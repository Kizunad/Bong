use serde::{Deserialize, Serialize};
use valence::prelude::{App, Entity, EventReader, EventWriter, IntoSystemConfigs, Query, Update};

use super::components::{GuardianKind, HouseGuardian, IntrusionRecord, SpiritNiche, Tick};
use super::events::{
    NicheGuardianBroken, NicheGuardianFatigue, NicheIntrusionAttempt, NicheIntrusionEvent,
    SpiritNicheActivateGuardianRequest,
};
use crate::combat::components::TICKS_PER_SECOND;
use crate::cultivation::realm_taint::{ApplyRealmTaint, RealmTaintedKind};
use crate::inventory::{attach_lingering_owner_qi_by_instance, PlayerInventory};

pub const NICHE_INTRUSION_TAINT_DELTA: f32 = 0.20;
pub const LINGERING_OWNER_QI_TICKS: u64 = 8 * 60 * 60 * TICKS_PER_SECOND;
pub const PUPPET_BEAST_BONE_ITEM_ID: &str = "yi_shou_gu";
pub const PUPPET_ARRAY_STONE_ITEM_ID: &str = "zhen_shi_zhong";
pub const BASIC_TRAP_STONE_ITEM_ID: &str = "zhen_shi_chu";
pub const MIDDLE_TRAP_STONE_ITEM_ID: &str = "zhen_shi_zhong";
pub const ADVANCED_TRAP_STONE_ITEM_ID: &str = "zhen_shi_gao";
pub const BONDED_DAOXIANG_REMAINS_ITEM_ID: &str = "daoxiang_remains";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardianActivationError {
    OwnerMismatch,
    NichePositionMismatch,
    InstanceLimitReached,
    MissingMaterial(&'static str),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IntrusionDefenseOutcome {
    pub record: IntrusionRecord,
    pub guardian_fatigues: Vec<(GuardianKind, u8)>,
    pub guardian_breaks: Vec<GuardianKind>,
    pub taint_delta: f32,
}

pub fn register(app: &mut App) {
    app.add_event::<SpiritNicheActivateGuardianRequest>();
    app.add_event::<NicheIntrusionAttempt>();
    app.add_event::<NicheIntrusionEvent>();
    app.add_event::<NicheGuardianFatigue>();
    app.add_event::<NicheGuardianBroken>();
    app.add_systems(
        Update,
        (
            handle_spirit_niche_activate_guardian_requests,
            handle_niche_intrusion_attempts.after(handle_spirit_niche_activate_guardian_requests),
        ),
    );
}

pub fn activate_guardian(
    niche: &mut SpiritNiche,
    owner: &str,
    niche_pos: [i32; 3],
    guardian_kind: GuardianKind,
    materials: &[String],
    now_tick: Tick,
) -> Result<HouseGuardian, GuardianActivationError> {
    if niche.owner != owner {
        return Err(GuardianActivationError::OwnerMismatch);
    }
    if niche.pos != niche_pos {
        return Err(GuardianActivationError::NichePositionMismatch);
    }
    if niche
        .guardians
        .iter()
        .filter(|guardian| guardian.kind == guardian_kind && guardian.active)
        .count()
        >= guardian_kind.max_instances()
    {
        return Err(GuardianActivationError::InstanceLimitReached);
    }

    require_materials(guardian_kind, materials)?;
    let guardian_id = next_guardian_id(niche, now_tick);
    let guardian = HouseGuardian::new(
        guardian_id,
        guardian_kind,
        niche.owner.clone(),
        niche.pos,
        now_tick,
    );
    niche.guardians.push(guardian.clone());
    Ok(guardian)
}

pub fn resolve_intrusion(
    niche: &mut SpiritNiche,
    intruder: Entity,
    intruder_char_id: String,
    items_taken: Vec<u64>,
    intruder_qi_fraction: f32,
    intruder_back_turned: bool,
    now_tick: Tick,
) -> Option<IntrusionDefenseOutcome> {
    if niche.owner == intruder_char_id {
        return None;
    }

    let mut triggered = Vec::new();
    let mut fatigues = Vec::new();
    let mut breaks = Vec::new();
    for guardian in niche.guardians.iter_mut() {
        if !guardian.can_trigger_for(intruder_char_id.as_str(), now_tick) {
            continue;
        }
        if guardian.kind == GuardianKind::BondedDaoxiang
            && !intruder_back_turned
            && intruder_qi_fraction > 0.20
        {
            continue;
        }
        if guardian.consume_charge() {
            triggered.push(guardian.kind);
            fatigues.push((guardian.kind, guardian.charges_remaining));
            if guardian.charges_remaining == 0 {
                breaks.push(guardian.kind);
            }
        }
    }

    let taint_delta = if items_taken.is_empty() {
        0.0
    } else {
        NICHE_INTRUSION_TAINT_DELTA
    };
    if triggered.is_empty() && taint_delta == 0.0 {
        return None;
    }

    let record = IntrusionRecord {
        intruder,
        intruder_char_id,
        owner: niche.owner.clone(),
        time: now_tick,
        niche_pos: niche.pos,
        items_taken,
        guardian_kinds_triggered: triggered,
    };
    Some(IntrusionDefenseOutcome {
        record,
        guardian_fatigues: fatigues,
        guardian_breaks: breaks,
        taint_delta,
    })
}

fn handle_spirit_niche_activate_guardian_requests(
    mut events: EventReader<SpiritNicheActivateGuardianRequest>,
    mut niches: Query<(&mut SpiritNiche, &crate::combat::components::Lifecycle)>,
) {
    for event in events.read() {
        let Ok((mut niche, lifecycle)) = niches.get_mut(event.player) else {
            continue;
        };
        if let Err(error) = activate_guardian(
            &mut niche,
            lifecycle.character_id.as_str(),
            event.niche_pos,
            event.guardian_kind,
            event.materials.as_slice(),
            event.tick,
        ) {
            tracing::warn!(
                "[bong][social][niche-defense] guardian activation rejected for `{}`: {:?}",
                lifecycle.character_id,
                error
            );
        }
    }
}

fn handle_niche_intrusion_attempts(
    mut attempts: EventReader<NicheIntrusionAttempt>,
    mut niches: Query<&mut SpiritNiche>,
    mut inventories: Query<&mut PlayerInventory>,
    mut intrusions: EventWriter<NicheIntrusionEvent>,
    mut fatigues: EventWriter<NicheGuardianFatigue>,
    mut broken: EventWriter<NicheGuardianBroken>,
    mut taints: EventWriter<ApplyRealmTaint>,
) {
    for attempt in attempts.read() {
        let Some(mut niche) = niches
            .iter_mut()
            .find(|niche| niche.owner == attempt.niche_owner && niche.pos == attempt.niche_pos)
        else {
            continue;
        };
        let Some(outcome) = resolve_intrusion(
            &mut niche,
            attempt.intruder,
            attempt.intruder_char_id.clone(),
            attempt.items_taken.clone(),
            attempt.intruder_qi_fraction,
            attempt.intruder_back_turned,
            attempt.tick,
        ) else {
            continue;
        };
        for (guardian_kind, charges_remaining) in &outcome.guardian_fatigues {
            fatigues.send(NicheGuardianFatigue {
                niche_owner: outcome.record.owner.clone(),
                guardian_kind: *guardian_kind,
                charges_remaining: *charges_remaining,
                tick: attempt.tick,
            });
        }
        for guardian_kind in &outcome.guardian_breaks {
            broken.send(NicheGuardianBroken {
                niche_owner: outcome.record.owner.clone(),
                guardian_kind: *guardian_kind,
                intruder: attempt.intruder,
                intruder_char_id: attempt.intruder_char_id.clone(),
                tick: attempt.tick,
            });
        }
        if !outcome.record.items_taken.is_empty() {
            if let Ok(mut inventory) = inventories.get_mut(attempt.intruder) {
                let expire_at = attempt.tick.saturating_add(LINGERING_OWNER_QI_TICKS);
                for instance_id in &outcome.record.items_taken {
                    attach_lingering_owner_qi_by_instance(
                        &mut inventory,
                        *instance_id,
                        outcome.record.owner.clone(),
                        expire_at,
                    );
                }
            }
        }
        if outcome.taint_delta > 0.0 {
            taints.send(ApplyRealmTaint {
                target: attempt.intruder,
                kind: RealmTaintedKind::NicheIntrusion,
                delta: outcome.taint_delta,
                tick: attempt.tick,
            });
        }
        intrusions.send(NicheIntrusionEvent {
            niche_owner: outcome.record.owner,
            intruder: attempt.intruder,
            intruder_char_id: attempt.intruder_char_id.clone(),
            niche_pos: outcome.record.niche_pos,
            items_taken: outcome.record.items_taken,
            taint_delta: outcome.taint_delta,
            guardian_kinds_triggered: outcome.record.guardian_kinds_triggered,
            tick: attempt.tick,
        });
    }
}

fn require_materials(
    guardian_kind: GuardianKind,
    materials: &[String],
) -> Result<(), GuardianActivationError> {
    match guardian_kind {
        GuardianKind::Puppet => {
            require_count(materials, PUPPET_BEAST_BONE_ITEM_ID, 3)?;
            require_count(materials, PUPPET_ARRAY_STONE_ITEM_ID, 1)?;
        }
        GuardianKind::ZhenfaTrap => {
            if !materials.iter().any(|material| {
                matches!(
                    material.as_str(),
                    BASIC_TRAP_STONE_ITEM_ID
                        | MIDDLE_TRAP_STONE_ITEM_ID
                        | ADVANCED_TRAP_STONE_ITEM_ID
                )
            }) {
                return Err(GuardianActivationError::MissingMaterial(
                    BASIC_TRAP_STONE_ITEM_ID,
                ));
            }
        }
        GuardianKind::BondedDaoxiang => {
            require_count(materials, BONDED_DAOXIANG_REMAINS_ITEM_ID, 1)?;
            require_count(materials, ADVANCED_TRAP_STONE_ITEM_ID, 1)?;
        }
    }
    Ok(())
}

fn require_count(
    materials: &[String],
    required: &'static str,
    count: usize,
) -> Result<(), GuardianActivationError> {
    let actual = materials
        .iter()
        .filter(|material| material.as_str() == required)
        .count();
    if actual < count {
        return Err(GuardianActivationError::MissingMaterial(required));
    }
    Ok(())
}

fn next_guardian_id(niche: &SpiritNiche, now_tick: Tick) -> u64 {
    niche
        .guardians
        .iter()
        .map(|guardian| guardian.id)
        .max()
        .unwrap_or(now_tick)
        .saturating_add(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn niche() -> SpiritNiche {
        SpiritNiche {
            owner: "char:owner".to_string(),
            pos: [10, 64, 10],
            placed_at_tick: 1,
            revealed: false,
            revealed_by: None,
            guardians: Vec::new(),
        }
    }

    #[test]
    fn puppet_activation_rejects_missing_materials() {
        let mut niche = niche();
        let err = activate_guardian(
            &mut niche,
            "char:owner",
            [10, 64, 10],
            GuardianKind::Puppet,
            &["yi_shou_gu".to_string()],
            100,
        )
        .expect_err("missing array stone should reject puppet activation");
        assert_eq!(
            err,
            GuardianActivationError::MissingMaterial(PUPPET_BEAST_BONE_ITEM_ID)
        );
        assert!(niche.guardians.is_empty());
    }

    #[test]
    fn puppet_activation_succeeds_and_limits_to_one() {
        let mut niche = niche();
        let materials = vec![
            PUPPET_BEAST_BONE_ITEM_ID.to_string(),
            PUPPET_BEAST_BONE_ITEM_ID.to_string(),
            PUPPET_BEAST_BONE_ITEM_ID.to_string(),
            PUPPET_ARRAY_STONE_ITEM_ID.to_string(),
        ];
        let guardian = activate_guardian(
            &mut niche,
            "char:owner",
            [10, 64, 10],
            GuardianKind::Puppet,
            &materials,
            100,
        )
        .expect("puppet should activate with three beast bones and one array stone");
        assert_eq!(guardian.kind, GuardianKind::Puppet);
        assert_eq!(guardian.charges_remaining, 5);

        let err = activate_guardian(
            &mut niche,
            "char:owner",
            [10, 64, 10],
            GuardianKind::Puppet,
            &materials,
            101,
        )
        .expect_err("second puppet should hit same-kind limit");
        assert_eq!(err, GuardianActivationError::InstanceLimitReached);
    }

    #[test]
    fn intrusion_consumes_guardian_charge_and_marks_taint_for_taken_items() {
        let mut niche = niche();
        niche.guardians.push(HouseGuardian::new(
            1,
            GuardianKind::Puppet,
            "char:owner".to_string(),
            [10, 64, 10],
            10,
        ));
        let outcome = resolve_intrusion(
            &mut niche,
            Entity::from_raw(7),
            "char:intruder".to_string(),
            vec![42],
            0.8,
            false,
            11,
        )
        .expect("intrusion should trigger puppet and taint");
        assert_eq!(outcome.guardian_fatigues, vec![(GuardianKind::Puppet, 4)]);
        assert_eq!(outcome.taint_delta, NICHE_INTRUSION_TAINT_DELTA);
        assert_eq!(niche.guardians[0].charges_remaining, 4);
    }

    #[test]
    fn bonded_daoxiang_waits_for_back_or_low_qi_trigger() {
        let mut niche = niche();
        niche.guardians.push(HouseGuardian::new(
            1,
            GuardianKind::BondedDaoxiang,
            "char:owner".to_string(),
            [10, 64, 10],
            10,
        ));
        let no_trigger = resolve_intrusion(
            &mut niche,
            Entity::from_raw(7),
            "char:intruder".to_string(),
            Vec::new(),
            0.9,
            false,
            11,
        );
        assert!(no_trigger.is_none());

        let trigger = resolve_intrusion(
            &mut niche,
            Entity::from_raw(7),
            "char:intruder".to_string(),
            Vec::new(),
            0.9,
            true,
            12,
        )
        .expect("back-turned intruder should trigger bonded daoxiang");
        assert_eq!(
            trigger.record.guardian_kinds_triggered,
            vec![GuardianKind::BondedDaoxiang]
        );
    }
}
