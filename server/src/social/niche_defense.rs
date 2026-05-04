use serde::{Deserialize, Serialize};
use valence::prelude::{App, Entity, EventReader, EventWriter, IntoSystemConfigs, Query, Update};

use super::components::{
    GuardianKind, HouseGuardian, IntrusionRecord, SpiritNiche, Tick, ZhenfaTrapTier,
};
use super::events::{
    NicheGuardianBroken, NicheGuardianFatigue, NicheIntrusionAttempt, NicheIntrusionEvent,
    SpiritNicheActivateGuardianRequest,
};
use crate::combat::components::TICKS_PER_SECOND;
use crate::cultivation::realm_taint::{ApplyRealmTaint, RealmTaintedKind};
use crate::inventory::{
    attach_lingering_owner_qi_by_instance, consume_item_instance_once, PlayerInventory,
};

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
    MaterialConsumeFailed(&'static str),
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

#[cfg(test)]
pub fn activate_guardian(
    niche: &mut SpiritNiche,
    owner: &str,
    niche_pos: [i32; 3],
    guardian_kind: GuardianKind,
    materials: &[String],
    now_tick: Tick,
) -> Result<HouseGuardian, GuardianActivationError> {
    let recipe = activation_recipe(guardian_kind, materials)?;
    validate_guardian_activation(niche, owner, niche_pos, guardian_kind)?;
    Ok(push_guardian(
        niche,
        guardian_kind,
        recipe.trap_tier,
        now_tick,
    ))
}

pub fn activate_guardian_from_inventory(
    niche: &mut SpiritNiche,
    inventory: &mut PlayerInventory,
    owner: &str,
    niche_pos: [i32; 3],
    guardian_kind: GuardianKind,
    materials: &[String],
    now_tick: Tick,
) -> Result<HouseGuardian, GuardianActivationError> {
    let recipe = activation_recipe(guardian_kind, materials)?;
    validate_guardian_activation(niche, owner, niche_pos, guardian_kind)?;
    consume_required_materials(inventory, &recipe.requirements)?;
    Ok(push_guardian(
        niche,
        guardian_kind,
        recipe.trap_tier,
        now_tick,
    ))
}

fn validate_guardian_activation(
    niche: &SpiritNiche,
    owner: &str,
    niche_pos: [i32; 3],
    guardian_kind: GuardianKind,
) -> Result<(), GuardianActivationError> {
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
    Ok(())
}

fn push_guardian(
    niche: &mut SpiritNiche,
    guardian_kind: GuardianKind,
    trap_tier: ZhenfaTrapTier,
    now_tick: Tick,
) -> HouseGuardian {
    let guardian_id = next_guardian_id(niche, now_tick);
    let mut guardian = HouseGuardian::new(
        guardian_id,
        guardian_kind,
        niche.owner.clone(),
        niche.pos,
        now_tick,
    );
    guardian.trap_tier = trap_tier;
    niche.guardians.push(guardian.clone());
    guardian
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
    mut niches: Query<(
        &mut SpiritNiche,
        &crate::combat::components::Lifecycle,
        Option<&mut PlayerInventory>,
    )>,
) {
    for event in events.read() {
        let Ok((mut niche, lifecycle, inventory)) = niches.get_mut(event.player) else {
            continue;
        };
        let Some(mut inventory) = inventory else {
            tracing::warn!(
                "[bong][social][niche-defense] guardian activation rejected for `{}`: missing inventory",
                lifecycle.character_id
            );
            continue;
        };
        if let Err(error) = activate_guardian_from_inventory(
            &mut niche,
            &mut inventory,
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

#[derive(Debug, Clone, Copy)]
struct GuardianMaterialRequirement {
    item_id: &'static str,
    count: u32,
}

#[derive(Debug, Clone)]
struct GuardianActivationRecipe {
    requirements: Vec<GuardianMaterialRequirement>,
    trap_tier: ZhenfaTrapTier,
}

fn activation_recipe(
    guardian_kind: GuardianKind,
    materials: &[String],
) -> Result<GuardianActivationRecipe, GuardianActivationError> {
    let recipe = match guardian_kind {
        GuardianKind::Puppet => GuardianActivationRecipe {
            requirements: vec![
                GuardianMaterialRequirement {
                    item_id: PUPPET_BEAST_BONE_ITEM_ID,
                    count: 3,
                },
                GuardianMaterialRequirement {
                    item_id: PUPPET_ARRAY_STONE_ITEM_ID,
                    count: 1,
                },
            ],
            trap_tier: ZhenfaTrapTier::default(),
        },
        GuardianKind::ZhenfaTrap => {
            let Some((trap_stone, trap_tier)) = requested_trap_stone(materials) else {
                return Err(GuardianActivationError::MissingMaterial(
                    BASIC_TRAP_STONE_ITEM_ID,
                ));
            };
            GuardianActivationRecipe {
                requirements: vec![GuardianMaterialRequirement {
                    item_id: trap_stone,
                    count: 1,
                }],
                trap_tier,
            }
        }
        GuardianKind::BondedDaoxiang => GuardianActivationRecipe {
            requirements: vec![
                GuardianMaterialRequirement {
                    item_id: BONDED_DAOXIANG_REMAINS_ITEM_ID,
                    count: 1,
                },
                GuardianMaterialRequirement {
                    item_id: ADVANCED_TRAP_STONE_ITEM_ID,
                    count: 1,
                },
            ],
            trap_tier: ZhenfaTrapTier::default(),
        },
    };
    for requirement in &recipe.requirements {
        if material_count(materials, requirement.item_id) < requirement.count {
            return Err(GuardianActivationError::MissingMaterial(
                requirement.item_id,
            ));
        }
    }
    Ok(recipe)
}

fn requested_trap_stone(materials: &[String]) -> Option<(&'static str, ZhenfaTrapTier)> {
    materials
        .iter()
        .find_map(|material| match material.as_str() {
            BASIC_TRAP_STONE_ITEM_ID => Some((BASIC_TRAP_STONE_ITEM_ID, ZhenfaTrapTier::Basic)),
            MIDDLE_TRAP_STONE_ITEM_ID => Some((MIDDLE_TRAP_STONE_ITEM_ID, ZhenfaTrapTier::Middle)),
            ADVANCED_TRAP_STONE_ITEM_ID => {
                Some((ADVANCED_TRAP_STONE_ITEM_ID, ZhenfaTrapTier::Advanced))
            }
            _ => None,
        })
}

fn material_count(materials: &[String], required: &str) -> u32 {
    materials
        .iter()
        .filter(|material| material.as_str() == required)
        .count() as u32
}

fn consume_required_materials(
    inventory: &mut PlayerInventory,
    requirements: &[GuardianMaterialRequirement],
) -> Result<(), GuardianActivationError> {
    for requirement in requirements {
        if inventory_template_count(inventory, requirement.item_id) < requirement.count {
            return Err(GuardianActivationError::MissingMaterial(
                requirement.item_id,
            ));
        }
    }
    for requirement in requirements {
        for _ in 0..requirement.count {
            let Some(instance_id) = find_material_instance_id(inventory, requirement.item_id)
            else {
                return Err(GuardianActivationError::MissingMaterial(
                    requirement.item_id,
                ));
            };
            consume_item_instance_once(inventory, instance_id)
                .map_err(|_| GuardianActivationError::MaterialConsumeFailed(requirement.item_id))?;
        }
    }
    Ok(())
}

fn inventory_template_count(inventory: &PlayerInventory, template_id: &str) -> u32 {
    let container_count: u32 = inventory
        .containers
        .iter()
        .flat_map(|container| container.items.iter())
        .filter(|placed| placed.instance.template_id == template_id)
        .map(|placed| placed.instance.stack_count)
        .sum();
    let equipped_count: u32 = inventory
        .equipped
        .values()
        .filter(|item| item.template_id == template_id)
        .map(|item| item.stack_count)
        .sum();
    let hotbar_count: u32 = inventory
        .hotbar
        .iter()
        .flatten()
        .filter(|item| item.template_id == template_id)
        .map(|item| item.stack_count)
        .sum();
    container_count
        .saturating_add(equipped_count)
        .saturating_add(hotbar_count)
}

fn find_material_instance_id(inventory: &PlayerInventory, template_id: &str) -> Option<u64> {
    inventory
        .containers
        .iter()
        .flat_map(|container| container.items.iter())
        .find(|placed| {
            placed.instance.template_id == template_id && placed.instance.stack_count > 0
        })
        .map(|placed| placed.instance.instance_id)
        .or_else(|| {
            inventory
                .hotbar
                .iter()
                .flatten()
                .find(|item| item.template_id == template_id && item.stack_count > 0)
                .map(|item| item.instance_id)
        })
        .or_else(|| {
            inventory
                .equipped
                .values()
                .find(|item| item.template_id == template_id && item.stack_count > 0)
                .map(|item| item.instance_id)
        })
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
    use std::collections::HashMap;

    use crate::inventory::{
        ContainerState, InventoryRevision, ItemInstance, ItemRarity, PlacedItemState,
        MAIN_PACK_CONTAINER_ID,
    };

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

    fn item(template_id: &str, instance_id: u64, stack_count: u32) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: template_id.to_string(),
            display_name: template_id.to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 1.0,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count,
            spirit_quality: 0.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        }
    }

    fn inventory_with(items: Vec<ItemInstance>) -> PlayerInventory {
        PlayerInventory {
            revision: InventoryRevision(1),
            containers: vec![ContainerState {
                id: MAIN_PACK_CONTAINER_ID.to_string(),
                name: "Main Pack".to_string(),
                rows: 4,
                cols: 9,
                items: items
                    .into_iter()
                    .enumerate()
                    .map(|(idx, instance)| PlacedItemState {
                        row: 0,
                        col: idx as u8,
                        instance,
                    })
                    .collect(),
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 45.0,
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
    fn inventory_activation_rejects_forged_materials_without_server_items() {
        let mut niche = niche();
        let mut inventory = inventory_with(Vec::new());
        let materials = vec![
            PUPPET_BEAST_BONE_ITEM_ID.to_string(),
            PUPPET_BEAST_BONE_ITEM_ID.to_string(),
            PUPPET_BEAST_BONE_ITEM_ID.to_string(),
            PUPPET_ARRAY_STONE_ITEM_ID.to_string(),
        ];
        let err = activate_guardian_from_inventory(
            &mut niche,
            &mut inventory,
            "char:owner",
            [10, 64, 10],
            GuardianKind::Puppet,
            &materials,
            100,
        )
        .expect_err("client-provided material ids must not bypass server inventory");
        assert_eq!(
            err,
            GuardianActivationError::MissingMaterial(PUPPET_BEAST_BONE_ITEM_ID)
        );
        assert!(niche.guardians.is_empty());
        assert_eq!(inventory.revision, InventoryRevision(1));
    }

    #[test]
    fn inventory_activation_consumes_authoritative_material_stacks() {
        let mut niche = niche();
        let mut inventory = inventory_with(vec![
            item(PUPPET_BEAST_BONE_ITEM_ID, 1, 3),
            item(PUPPET_ARRAY_STONE_ITEM_ID, 2, 1),
        ]);
        let materials = vec![
            PUPPET_BEAST_BONE_ITEM_ID.to_string(),
            PUPPET_BEAST_BONE_ITEM_ID.to_string(),
            PUPPET_BEAST_BONE_ITEM_ID.to_string(),
            PUPPET_ARRAY_STONE_ITEM_ID.to_string(),
        ];
        let guardian = activate_guardian_from_inventory(
            &mut niche,
            &mut inventory,
            "char:owner",
            [10, 64, 10],
            GuardianKind::Puppet,
            &materials,
            100,
        )
        .expect("server inventory should satisfy and consume puppet materials");
        assert_eq!(guardian.kind, GuardianKind::Puppet);
        assert!(inventory.containers[0].items.is_empty());
        assert!(inventory.revision.0 > 1);
    }

    #[test]
    fn zhenfa_trap_activation_preserves_requested_tier() {
        let mut niche = niche();
        let mut inventory = inventory_with(vec![item(ADVANCED_TRAP_STONE_ITEM_ID, 3, 1)]);
        let guardian = activate_guardian_from_inventory(
            &mut niche,
            &mut inventory,
            "char:owner",
            [10, 64, 10],
            GuardianKind::ZhenfaTrap,
            &[ADVANCED_TRAP_STONE_ITEM_ID.to_string()],
            100,
        )
        .expect("advanced trap stone should activate advanced trap");
        assert_eq!(guardian.kind, GuardianKind::ZhenfaTrap);
        assert_eq!(guardian.trap_tier, ZhenfaTrapTier::Advanced);
        assert!(inventory.containers[0].items.is_empty());
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
