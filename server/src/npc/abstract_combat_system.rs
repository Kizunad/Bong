use valence::prelude::{
    bevy_ecs, App, Commands, Component, Entity, EventWriter, Query, Res, ResMut, Resource, Update,
    With, Without,
};

use crate::combat::components::{CombatState, DerivedAttrs, Wounds};
use crate::cultivation::components::Cultivation;
use crate::cultivation::known_techniques::KnownTechniques;
use crate::npc::abstract_combat::{
    abstract_combat_resolve, apply_outcome_to_loser, apply_qi_cost, AbstractCombatOutcome,
};
use crate::npc::combat_power::compute_combat_power;
use crate::npc::equipment::NpcEquipment;
use crate::npc::faction::{FactionId, FactionMembership, FactionStore};
use crate::npc::lod::NpcLodTier;
use crate::npc::movement::GameTick;
use crate::npc::patrol::NpcPatrol;
use crate::npc::spawn::{DuelTarget, NpcBlackboard, NpcMarker};
use crate::qi_physics::ledger::{QiAccountId, QiTransfer, QiTransferReason};

pub const ABSTRACT_COMBAT_INTERVAL_TICKS: u32 = 200;
pub const FAR_TIER_ENCOUNTER_RADIUS_SQ: f64 = 64.0 * 64.0;

#[derive(Debug, Clone, Copy, Default, Resource)]
pub struct AbstractCombatSeed(pub u64);

pub fn register(app: &mut App) {
    app.insert_resource(AbstractCombatSeed::default())
        .add_systems(
            Update,
            (abstract_combat_system, lod_transition_cleanup_system),
        );
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn abstract_combat_system(
    game_tick: Res<GameTick>,
    mut seed_res: ResMut<AbstractCombatSeed>,
    faction_store: Res<FactionStore>,
    mut npcs: Query<
        (
            Entity,
            &NpcLodTier,
            &mut Wounds,
            &mut Cultivation,
            &DerivedAttrs,
            &KnownTechniques,
            Option<&NpcEquipment>,
            Option<&FactionMembership>,
            Option<&DuelTarget>,
            &NpcPatrol,
            &mut NpcBlackboard,
        ),
        With<NpcMarker>,
    >,
    mut qi_events: EventWriter<QiTransfer>,
    mut commands: Commands,
) {
    if game_tick.0 % ABSTRACT_COMBAT_INTERVAL_TICKS != 0 {
        return;
    }

    let pairs = collect_far_combat_pairs(&npcs, &faction_store);

    for (attacker_entity, defender_entity) in pairs {
        let Ok(
            [(
                _,
                _,
                ref mut a_wounds,
                ref mut a_cult,
                a_derived,
                a_tech,
                a_equip,
                _,
                _,
                a_patrol,
                ref mut a_bb,
            ), (
                _,
                _,
                ref mut d_wounds,
                ref mut d_cult,
                d_derived,
                d_tech,
                d_equip,
                _,
                _,
                d_patrol,
                ref mut d_bb,
            )],
        ) = npcs.get_many_mut([attacker_entity, defender_entity])
        else {
            if let Ok((_, _, _, _, _, _, _, _, _, _, ref mut bb)) = npcs.get_mut(attacker_entity) {
                bb.retaliation_target = None;
            }
            if let Ok((_, _, _, _, _, _, _, _, _, _, ref mut bb)) = npcs.get_mut(defender_entity) {
                bb.retaliation_target = None;
            }
            continue;
        };

        let attacker_power = compute_combat_power(
            a_cult.realm,
            a_cult.as_ref(),
            a_wounds.as_ref(),
            a_derived,
            a_tech,
            a_equip,
        );
        let defender_power = compute_combat_power(
            d_cult.realm,
            d_cult.as_ref(),
            d_wounds.as_ref(),
            d_derived,
            d_tech,
            d_equip,
        );

        seed_res.0 = seed_res.0.wrapping_add(1);
        let combat_seed = seed_res
            .0
            .wrapping_mul(0x517C_C1B7_2722_0A95)
            .wrapping_add(game_tick.0 as u64);

        let outcome = abstract_combat_resolve(
            attacker_power,
            defender_power,
            a_cult.realm,
            d_cult.realm,
            combat_seed,
        );

        match outcome {
            AbstractCombatOutcome::AttackerWins {
                damage_ratio,
                attacker_qi_cost,
                defender_qi_cost,
            } => {
                apply_outcome_to_loser(d_wounds.as_mut(), damage_ratio);
                let a_actual = apply_qi_cost(a_cult.as_mut(), attacker_qi_cost);
                let d_actual = apply_qi_cost(d_cult.as_mut(), defender_qi_cost);

                emit_qi_transfer(
                    &mut qi_events,
                    attacker_entity,
                    &a_patrol.home_zone,
                    a_actual,
                );
                emit_qi_transfer(
                    &mut qi_events,
                    defender_entity,
                    &d_patrol.home_zone,
                    d_actual,
                );
            }
            AbstractCombatOutcome::DefenderWins {
                damage_ratio,
                attacker_qi_cost,
                defender_qi_cost,
            } => {
                apply_outcome_to_loser(a_wounds.as_mut(), damage_ratio);
                let a_actual = apply_qi_cost(a_cult.as_mut(), attacker_qi_cost);
                let d_actual = apply_qi_cost(d_cult.as_mut(), defender_qi_cost);

                emit_qi_transfer(
                    &mut qi_events,
                    attacker_entity,
                    &a_patrol.home_zone,
                    a_actual,
                );
                emit_qi_transfer(
                    &mut qi_events,
                    defender_entity,
                    &d_patrol.home_zone,
                    d_actual,
                );
            }
        }

        a_bb.retaliation_target = Some((defender_entity, game_tick.0 as u64 + 400));
        d_bb.retaliation_target = Some((attacker_entity, game_tick.0 as u64 + 400));

        if !npcs.contains(attacker_entity) {
            commands.entity(defender_entity).remove::<DuelTarget>();
        }
        if !npcs.contains(defender_entity) {
            commands.entity(attacker_entity).remove::<DuelTarget>();
        }
    }
}

#[allow(clippy::type_complexity)]
fn collect_far_combat_pairs(
    npcs: &Query<
        (
            Entity,
            &NpcLodTier,
            &mut Wounds,
            &mut Cultivation,
            &DerivedAttrs,
            &KnownTechniques,
            Option<&NpcEquipment>,
            Option<&FactionMembership>,
            Option<&DuelTarget>,
            &NpcPatrol,
            &mut NpcBlackboard,
        ),
        With<NpcMarker>,
    >,
    faction_store: &FactionStore,
) -> Vec<(Entity, Entity)> {
    let mut pairs = Vec::new();
    let mut processed = std::collections::HashSet::new();

    let far_npcs: Vec<(Entity, Option<FactionId>, Option<Entity>)> = npcs
        .iter()
        .filter(|(_, tier, _, _, _, _, _, _, _, _, _)| **tier == NpcLodTier::Far)
        .map(|(entity, _, _, _, _, _, _, membership, duel, _, _)| {
            (entity, membership.map(|m| m.faction_id), duel.map(|d| d.0))
        })
        .collect();

    for (entity, faction, duel_target) in &far_npcs {
        if processed.contains(entity) {
            continue;
        }

        if let Some(target) = duel_target {
            if far_npcs.iter().any(|(e, _, _)| *e == *target) && !processed.contains(target) {
                pairs.push((*entity, *target));
                processed.insert(*entity);
                processed.insert(*target);
            }
            continue;
        }

        let Some(my_faction) = faction else {
            continue;
        };

        for (other_entity, other_faction, _) in &far_npcs {
            if *other_entity == *entity || processed.contains(other_entity) {
                continue;
            }
            let Some(other_fid) = other_faction else {
                continue;
            };
            if faction_store.is_hostile_pair(*my_faction, *other_fid) {
                pairs.push((*entity, *other_entity));
                processed.insert(*entity);
                processed.insert(*other_entity);
                break;
            }
        }
    }

    pairs
}

fn emit_qi_transfer(
    qi_events: &mut EventWriter<QiTransfer>,
    entity: Entity,
    zone_name: &str,
    amount: f64,
) {
    if amount <= 0.0 {
        return;
    }
    let from = QiAccountId::npc(format!("npc_{}v{}", entity.index(), entity.generation()));
    let to = QiAccountId::zone(zone_name);
    if let Ok(transfer) = QiTransfer::new(from, to, amount, QiTransferReason::AbstractCombat) {
        qi_events.send(transfer);
    }
}

#[allow(clippy::type_complexity)]
fn lod_transition_cleanup_system(
    mut npcs: Query<
        (Entity, &NpcLodTier, &mut CombatState, &mut NpcBlackboard),
        (With<NpcMarker>, Without<LodTransitionCleaned>),
    >,
    cleaned: Query<(Entity, &NpcLodTier), (With<NpcMarker>, With<LodTransitionCleaned>)>,
    mut commands: Commands,
) {
    for (entity, tier, mut combat_state, mut blackboard) in &mut npcs {
        match tier {
            NpcLodTier::Far | NpcLodTier::Dormant => {
                combat_state.incoming_window = None;
                if matches!(tier, NpcLodTier::Dormant) {
                    blackboard.retaliation_target = None;
                }
                commands.entity(entity).insert(LodTransitionCleaned);
            }
            NpcLodTier::Near => {}
        }
    }

    for (entity, tier) in &cleaned {
        if *tier == NpcLodTier::Near {
            commands.entity(entity).remove::<LodTransitionCleaned>();
        }
    }
}

#[derive(Component, Debug, Clone, Copy)]
struct LodTransitionCleaned;

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::{App, Events, Update};

    use crate::combat::components::{DerivedAttrs, Wounds};
    use crate::cultivation::components::{Cultivation, Realm};
    use crate::cultivation::known_techniques::KnownTechniques;
    use crate::npc::faction::{FactionMembership, FactionRank, FactionStore, Reputation};
    use crate::npc::lod::NpcLodTier;
    use crate::npc::movement::GameTick;
    use crate::npc::patrol::NpcPatrol;
    use crate::npc::spawn::{DuelTarget, NpcBlackboard, NpcMarker};

    fn build_test_app() -> App {
        let mut app = App::new();
        app.insert_resource(GameTick(0));
        app.insert_resource(FactionStore::default());
        app.insert_resource(AbstractCombatSeed::default());
        app.add_event::<QiTransfer>();
        app.add_systems(
            Update,
            (abstract_combat_system, lod_transition_cleanup_system),
        );
        app
    }

    fn spawn_far_npc(app: &mut App, realm: Realm, faction: FactionId, qi_current: f64) -> Entity {
        app.world_mut()
            .spawn((
                NpcMarker,
                NpcLodTier::Far,
                Wounds {
                    entries: Vec::new(),
                    health_current: 100.0,
                    health_max: 100.0,
                },
                Cultivation {
                    realm,
                    qi_current,
                    qi_max: 100.0,
                    ..Default::default()
                },
                DerivedAttrs {
                    attack_power: 5.0,
                    defense_power: 3.0,
                    ..Default::default()
                },
                KnownTechniques {
                    entries: Vec::new(),
                },
                FactionMembership {
                    faction_id: faction,
                    rank: FactionRank::Disciple,
                    reputation: Reputation::default(),
                    lineage: None,
                    mission_queue: Default::default(),
                },
                NpcPatrol::new("spawn", valence::prelude::DVec3::ZERO),
                NpcBlackboard::default(),
                CombatState::default(),
            ))
            .id()
    }

    fn spawn_near_npc(app: &mut App, faction: FactionId) -> Entity {
        app.world_mut()
            .spawn((
                NpcMarker,
                NpcLodTier::Near,
                Wounds {
                    entries: Vec::new(),
                    health_current: 100.0,
                    health_max: 100.0,
                },
                Cultivation {
                    realm: Realm::Induce,
                    qi_current: 50.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
                DerivedAttrs::default(),
                KnownTechniques {
                    entries: Vec::new(),
                },
                FactionMembership {
                    faction_id: faction,
                    rank: FactionRank::Disciple,
                    reputation: Reputation::default(),
                    lineage: None,
                    mission_queue: Default::default(),
                },
                NpcPatrol::new("spawn", valence::prelude::DVec3::ZERO),
                NpcBlackboard::default(),
                CombatState::default(),
            ))
            .id()
    }

    #[test]
    fn far_hostile_pair_triggers_abstract_combat() {
        let mut app = build_test_app();
        app.insert_resource(GameTick(ABSTRACT_COMBAT_INTERVAL_TICKS));

        let attacker = spawn_far_npc(&mut app, Realm::Induce, FactionId::Attack, 50.0);
        let defender = spawn_far_npc(&mut app, Realm::Induce, FactionId::Defend, 50.0);

        app.update();

        let a_wounds = app.world().get::<Wounds>(attacker).unwrap();
        let d_wounds = app.world().get::<Wounds>(defender).unwrap();
        let a_cult = app.world().get::<Cultivation>(attacker).unwrap();
        let d_cult = app.world().get::<Cultivation>(defender).unwrap();

        let hp_changed = a_wounds.health_current < 100.0 || d_wounds.health_current < 100.0;
        assert!(
            hp_changed,
            "at least one NPC should take damage; attacker hp={}, defender hp={}",
            a_wounds.health_current, d_wounds.health_current
        );

        let qi_changed = a_cult.qi_current < 50.0 || d_cult.qi_current < 50.0;
        assert!(
            qi_changed,
            "at least one NPC should lose qi; attacker qi={}, defender qi={}",
            a_cult.qi_current, d_cult.qi_current
        );
    }

    #[test]
    fn near_npcs_not_affected_by_abstract_combat() {
        let mut app = build_test_app();
        app.insert_resource(GameTick(ABSTRACT_COMBAT_INTERVAL_TICKS));

        let attacker = spawn_near_npc(&mut app, FactionId::Attack);
        let defender = spawn_near_npc(&mut app, FactionId::Defend);

        app.update();

        let a_wounds = app.world().get::<Wounds>(attacker).unwrap();
        let d_wounds = app.world().get::<Wounds>(defender).unwrap();
        assert!(
            (a_wounds.health_current - 100.0).abs() < 1e-4
                && (d_wounds.health_current - 100.0).abs() < 1e-4,
            "Near NPCs should NOT be affected by abstract combat"
        );
    }

    #[test]
    fn qi_transfer_emitted_after_abstract_combat() {
        let mut app = build_test_app();
        app.insert_resource(GameTick(ABSTRACT_COMBAT_INTERVAL_TICKS));

        spawn_far_npc(&mut app, Realm::Induce, FactionId::Attack, 50.0);
        spawn_far_npc(&mut app, Realm::Induce, FactionId::Defend, 50.0);

        app.update();

        let events = app.world().resource::<Events<QiTransfer>>();
        let transfers: Vec<_> = events.iter_current_update_events().collect();
        assert!(
            !transfers.is_empty(),
            "QiTransfer events should be emitted after abstract combat"
        );
        for transfer in &transfers {
            assert_eq!(
                transfer.reason,
                QiTransferReason::AbstractCombat,
                "transfer reason should be AbstractCombat"
            );
            assert!(
                transfer.amount > 0.0,
                "transfer amount should be positive, got {}",
                transfer.amount
            );
        }
    }

    #[test]
    fn same_faction_does_not_fight() {
        let mut app = build_test_app();
        app.insert_resource(GameTick(ABSTRACT_COMBAT_INTERVAL_TICKS));

        let a = spawn_far_npc(&mut app, Realm::Induce, FactionId::Attack, 50.0);
        let b = spawn_far_npc(&mut app, Realm::Induce, FactionId::Attack, 50.0);

        app.update();

        let a_wounds = app.world().get::<Wounds>(a).unwrap();
        let b_wounds = app.world().get::<Wounds>(b).unwrap();
        assert!(
            (a_wounds.health_current - 100.0).abs() < 1e-4,
            "same faction NPC should not take damage"
        );
        assert!(
            (b_wounds.health_current - 100.0).abs() < 1e-4,
            "same faction NPC should not take damage"
        );
    }

    #[test]
    fn system_skips_non_interval_ticks() {
        let mut app = build_test_app();
        app.insert_resource(GameTick(ABSTRACT_COMBAT_INTERVAL_TICKS + 1));

        let a = spawn_far_npc(&mut app, Realm::Induce, FactionId::Attack, 50.0);
        let d = spawn_far_npc(&mut app, Realm::Induce, FactionId::Defend, 50.0);

        app.update();

        let a_wounds = app.world().get::<Wounds>(a).unwrap();
        let d_wounds = app.world().get::<Wounds>(d).unwrap();
        assert!(
            (a_wounds.health_current - 100.0).abs() < 1e-4
                && (d_wounds.health_current - 100.0).abs() < 1e-4,
            "system should not run on non-interval ticks"
        );
    }

    #[test]
    fn dormant_npcs_not_affected() {
        let mut app = build_test_app();
        app.insert_resource(GameTick(ABSTRACT_COMBAT_INTERVAL_TICKS));

        let a = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcLodTier::Dormant,
                Wounds {
                    entries: Vec::new(),
                    health_current: 100.0,
                    health_max: 100.0,
                },
                Cultivation {
                    realm: Realm::Induce,
                    qi_current: 50.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
                DerivedAttrs::default(),
                KnownTechniques {
                    entries: Vec::new(),
                },
                FactionMembership {
                    faction_id: FactionId::Attack,
                    rank: FactionRank::Disciple,
                    reputation: Reputation::default(),
                    lineage: None,
                    mission_queue: Default::default(),
                },
                NpcPatrol::new("spawn", valence::prelude::DVec3::ZERO),
                NpcBlackboard::default(),
                CombatState::default(),
            ))
            .id();
        let d = spawn_far_npc(&mut app, Realm::Induce, FactionId::Defend, 50.0);

        app.update();

        let a_wounds = app.world().get::<Wounds>(a).unwrap();
        let d_wounds = app.world().get::<Wounds>(d).unwrap();
        assert!(
            (a_wounds.health_current - 100.0).abs() < 1e-4
                && (d_wounds.health_current - 100.0).abs() < 1e-4,
            "Dormant NPCs should NOT be affected by abstract combat"
        );
    }

    #[test]
    fn lod_transition_cleanup_clears_incoming_window() {
        let mut app = App::new();
        app.add_systems(Update, lod_transition_cleanup_system);

        let entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcLodTier::Far,
                CombatState {
                    in_combat_until_tick: Some(1000),
                    last_attack_at_tick: Some(500),
                    incoming_window: Some(crate::combat::components::DefenseWindow {
                        opened_at_tick: 400,
                        duration_ms: 200,
                    }),
                },
                NpcBlackboard {
                    retaliation_target: Some((Entity::PLACEHOLDER, 999)),
                    ..Default::default()
                },
            ))
            .id();

        app.update();

        let combat_state = app.world().get::<CombatState>(entity).unwrap();
        assert!(
            combat_state.incoming_window.is_none(),
            "incoming_window should be cleared when NPC transitions to Far"
        );
        let bb = app.world().get::<NpcBlackboard>(entity).unwrap();
        assert!(
            bb.retaliation_target.is_some(),
            "retaliation_target should NOT be cleared for Far tier (only Dormant)"
        );
    }

    #[test]
    fn lod_transition_dormant_clears_retaliation() {
        let mut app = App::new();
        app.add_systems(Update, lod_transition_cleanup_system);

        let entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcLodTier::Dormant,
                CombatState {
                    in_combat_until_tick: Some(1000),
                    last_attack_at_tick: Some(500),
                    incoming_window: Some(crate::combat::components::DefenseWindow {
                        opened_at_tick: 400,
                        duration_ms: 200,
                    }),
                },
                NpcBlackboard {
                    retaliation_target: Some((Entity::PLACEHOLDER, 999)),
                    ..Default::default()
                },
            ))
            .id();

        app.update();

        let combat_state = app.world().get::<CombatState>(entity).unwrap();
        assert!(
            combat_state.incoming_window.is_none(),
            "incoming_window should be cleared for Dormant tier"
        );
        let bb = app.world().get::<NpcBlackboard>(entity).unwrap();
        assert!(
            bb.retaliation_target.is_none(),
            "retaliation_target should be cleared for Dormant tier"
        );
    }

    #[test]
    fn lod_near_does_not_clear_combat_state() {
        let mut app = App::new();
        app.add_systems(Update, lod_transition_cleanup_system);

        let entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcLodTier::Near,
                CombatState {
                    in_combat_until_tick: Some(1000),
                    last_attack_at_tick: Some(500),
                    incoming_window: Some(crate::combat::components::DefenseWindow {
                        opened_at_tick: 400,
                        duration_ms: 200,
                    }),
                },
                NpcBlackboard {
                    retaliation_target: Some((Entity::PLACEHOLDER, 999)),
                    ..Default::default()
                },
            ))
            .id();

        app.update();

        let combat_state = app.world().get::<CombatState>(entity).unwrap();
        assert!(
            combat_state.incoming_window.is_some(),
            "Near tier should NOT clear incoming_window"
        );
    }

    #[test]
    fn duel_target_pair_fights_regardless_of_faction() {
        let mut app = build_test_app();
        app.insert_resource(GameTick(ABSTRACT_COMBAT_INTERVAL_TICKS));

        let a = spawn_far_npc(&mut app, Realm::Induce, FactionId::Attack, 50.0);
        let b = spawn_far_npc(&mut app, Realm::Induce, FactionId::Attack, 50.0);

        app.world_mut().entity_mut(a).insert(DuelTarget(b));
        app.world_mut().entity_mut(b).insert(DuelTarget(a));

        app.update();

        let a_wounds = app.world().get::<Wounds>(a).unwrap();
        let b_wounds = app.world().get::<Wounds>(b).unwrap();
        let hp_changed = a_wounds.health_current < 100.0 || b_wounds.health_current < 100.0;
        assert!(
            hp_changed,
            "DuelTarget pair should fight even with same faction; a hp={}, b hp={}",
            a_wounds.health_current, b_wounds.health_current
        );
    }

    #[test]
    fn despawn_target_does_not_panic() {
        let mut app = build_test_app();
        app.insert_resource(GameTick(ABSTRACT_COMBAT_INTERVAL_TICKS));

        let a = spawn_far_npc(&mut app, Realm::Induce, FactionId::Attack, 50.0);
        let fake_target = Entity::from_raw(9999);
        app.world_mut()
            .entity_mut(a)
            .insert(DuelTarget(fake_target));

        app.update();

        let a_wounds = app.world().get::<Wounds>(a).unwrap();
        assert!(
            (a_wounds.health_current - 100.0).abs() < 1e-4,
            "NPC with despawned target should not take damage"
        );
        let a_bb = app.world().get::<NpcBlackboard>(a).unwrap();
        assert!(
            a_bb.retaliation_target.is_none(),
            "retaliation_target should be cleared when target entity is invalid"
        );
    }

    #[test]
    fn neutral_faction_does_not_trigger_combat() {
        let mut app = build_test_app();
        app.insert_resource(GameTick(ABSTRACT_COMBAT_INTERVAL_TICKS));

        let a = spawn_far_npc(&mut app, Realm::Induce, FactionId::Neutral, 50.0);
        let b = spawn_far_npc(&mut app, Realm::Induce, FactionId::Attack, 50.0);

        app.update();

        let a_wounds = app.world().get::<Wounds>(a).unwrap();
        let b_wounds = app.world().get::<Wounds>(b).unwrap();
        assert!(
            (a_wounds.health_current - 100.0).abs() < 1e-4
                && (b_wounds.health_current - 100.0).abs() < 1e-4,
            "Neutral faction should not fight"
        );
    }
}
