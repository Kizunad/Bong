use big_brain::prelude::{ActionBuilder, ActionState, Actor};
use valence::client::ClientMarker;
use valence::prelude::{
    bevy_ecs, Commands, Component, DVec3, Entity, EntityKind, EventWriter, Position, Query, Res,
    With, Without,
};

use crate::combat::components::{Lifecycle, LifecycleState, StatusEffects};
use crate::combat::events::{AttackIntent, AttackSource, DefenseIntent, StatusEffectKind};
use crate::combat::jiemai::jiemai_qi_cost_for_realm;
use crate::combat::status::has_active_status;
use crate::combat::CombatClock;
use crate::cultivation::components::Cultivation;
use crate::cultivation::technique_scroll::realm_rank;
use crate::npc::movement::{
    activate_dash, activate_sprint, GameTick, MovementCapabilities, MovementController,
    MovementCooldowns, MovementMode,
};
use crate::npc::navigator::Navigator;
use crate::npc::patrol::NpcPatrol;
use crate::npc::spawn::{NpcBlackboard, NpcMarker, NpcMeleeProfile};
use crate::world::zone::ZoneRegistry;

use super::{
    CHASE_SPEED_FACTOR, FLEE_SPEED_FACTOR, FLEE_SUCCESS_DISTANCE, FLEE_WAYPOINT_DISTANCE,
    MELEE_ATTACK_COOLDOWN_TICKS, SPRINT_ACTIVATE_DISTANCE, SPRINT_DEACTIVATE_COOLDOWN,
    SPRINT_DEACTIVATE_DISTANCE,
};

type NpcGoalQueryItem<'a> = (
    &'a Position,
    &'a NpcBlackboard,
    &'a NpcPatrol,
    &'a NpcMeleeProfile,
    &'a mut Navigator,
);
type NpcGoalQueryFilter = (With<NpcMarker>, With<EntityKind>, Without<ClientMarker>);
type NpcFleeQueryItem<'a> = (
    &'a Position,
    &'a NpcBlackboard,
    &'a NpcPatrol,
    &'a mut Navigator,
);

// ---------------------------------------------------------------------------
// FleeAction
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Component)]
pub struct FleeAction;

impl ActionBuilder for FleeAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("FleeAction")
    }
}

pub(crate) fn flee_action_system(
    mut npcs: Query<NpcFleeQueryItem<'_>, NpcGoalQueryFilter>,
    mut actions: Query<(&Actor, &mut ActionState), With<FleeAction>>,
    zone_registry: Option<Res<ZoneRegistry>>,
) {
    for (Actor(actor), mut state) in &mut actions {
        let Ok((npc_position, blackboard, patrol, mut navigator)) = npcs.get_mut(*actor) else {
            *state = ActionState::Failure;
            continue;
        };

        match *state {
            ActionState::Requested => {
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                if blackboard.player_distance > FLEE_SUCCESS_DISTANCE as f32 {
                    navigator.stop();
                    *state = ActionState::Success;
                    continue;
                }

                let Some(target_pos) = blackboard.target_position else {
                    navigator.stop();
                    *state = ActionState::Failure;
                    continue;
                };

                let flee_target = compute_flee_target(
                    npc_position.get(),
                    target_pos,
                    zone_registry.as_deref(),
                    &patrol.home_zone,
                );
                navigator.set_goal(flee_target, FLEE_SPEED_FACTOR);
            }
            ActionState::Cancelled => {
                navigator.stop();
                *state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

/// Compute a point FLEE_WAYPOINT_DISTANCE blocks away from the player,
/// clamped to zone bounds.
pub(crate) fn compute_flee_target(
    npc_pos: DVec3,
    player_pos: DVec3,
    zone_registry: Option<&ZoneRegistry>,
    home_zone: &str,
) -> DVec3 {
    let mut flee_dir = npc_pos - player_pos;
    flee_dir.y = 0.0;

    let dir = if flee_dir.length_squared() <= f64::EPSILON {
        DVec3::new(1.0, 0.0, 0.0)
    } else {
        flee_dir.normalize()
    };

    let target = npc_pos + dir * FLEE_WAYPOINT_DISTANCE;

    // Clamp to zone bounds.
    if let Some(zone) = zone_registry.and_then(|r| r.find_zone_by_name(home_zone)) {
        let (min, max) = zone.bounds;
        DVec3::new(
            target.x.clamp(min.x, max.x),
            target.y,
            target.z.clamp(min.z, max.z),
        )
    } else {
        target
    }
}

// ---------------------------------------------------------------------------
// ChaseAction
// ---------------------------------------------------------------------------

/// Moves the NPC toward the nearest player.
#[derive(Clone, Copy, Debug, Component)]
pub struct ChaseAction;

impl ActionBuilder for ChaseAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("ChaseAction")
    }
}

pub(crate) fn chase_action_system(
    mut npcs: Query<NpcGoalQueryItem<'_>, NpcGoalQueryFilter>,
    mut movement: Query<
        (
            &mut MovementController,
            &MovementCapabilities,
            &mut MovementCooldowns,
        ),
        With<NpcMarker>,
    >,
    mut actions: Query<(&Actor, &mut ActionState), With<ChaseAction>>,
    game_tick: Option<Res<GameTick>>,
) {
    let tick = game_tick.map(|t| t.0).unwrap_or(0);

    for (Actor(actor), mut state) in &mut actions {
        let Ok((_npc_position, blackboard, _patrol, melee_profile, mut navigator)) =
            npcs.get_mut(*actor)
        else {
            *state = ActionState::Failure;
            continue;
        };

        match *state {
            ActionState::Requested => {
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                if blackboard.player_distance <= melee_profile.preferred_distance {
                    navigator.stop();
                    *state = ActionState::Success;
                    continue;
                }

                let Some(target_pos) = blackboard.target_position else {
                    navigator.stop();
                    *state = ActionState::Failure;
                    continue;
                };

                navigator.set_goal(target_pos, CHASE_SPEED_FACTOR);

                // Sprint enhancement: activate sprint when chasing at medium range.
                if let Ok((mut ctrl, caps, mut cooldowns)) = movement.get_mut(*actor) {
                    if blackboard.player_distance > SPRINT_ACTIVATE_DISTANCE {
                        activate_sprint(&mut ctrl, caps, &mut cooldowns, tick);
                    } else if blackboard.player_distance < SPRINT_DEACTIVATE_DISTANCE
                        && matches!(ctrl.mode, MovementMode::Sprinting(_))
                    {
                        cooldowns.sprint_ready_at = tick + SPRINT_DEACTIVATE_COOLDOWN;
                        ctrl.reset_to_ground();
                    }
                }
            }
            ActionState::Cancelled => {
                navigator.stop();
                if let Ok((mut ctrl, _, mut cooldowns)) = movement.get_mut(*actor) {
                    if matches!(ctrl.mode, MovementMode::Sprinting(_)) {
                        cooldowns.sprint_ready_at = tick + SPRINT_DEACTIVATE_COOLDOWN;
                    }
                    ctrl.reset_to_ground();
                }
                *state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

// ---------------------------------------------------------------------------
// MeleeAttackAction
// ---------------------------------------------------------------------------

/// NPC stands still "attacking" while the player is in melee range.
#[derive(Clone, Copy, Debug, Component)]
pub struct MeleeAttackAction;

impl ActionBuilder for MeleeAttackAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("MeleeAttackAction")
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn melee_attack_action_system(
    mut actions: Query<(&Actor, &mut ActionState), With<MeleeAttackAction>>,
    mut npcs: Query<
        (
            &Position,
            &mut NpcBlackboard,
            &NpcMeleeProfile,
            &mut Navigator,
            Option<&Lifecycle>,
        ),
        With<NpcMarker>,
    >,
    mut attack_intents: EventWriter<AttackIntent>,
    game_tick: Option<Res<GameTick>>,
) {
    let tick = game_tick.map(|t| t.0).unwrap_or(0);

    for (Actor(actor), mut state) in &mut actions {
        match *state {
            ActionState::Requested => {
                if let Ok((_, _, _, mut nav, _)) = npcs.get_mut(*actor) {
                    nav.stop();
                }
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                let Ok((_npc_pos, mut bb, profile, _, lifecycle)) = npcs.get_mut(*actor) else {
                    *state = ActionState::Failure;
                    continue;
                };

                // NPC is not alive — abort attack action.
                if lifecycle.is_some_and(|lc| lc.state != LifecycleState::Alive) {
                    *state = ActionState::Failure;
                    continue;
                }

                if bb.player_distance > profile.disengage_distance {
                    *state = ActionState::Success;
                    continue;
                }

                if bb.player_distance > profile.reach.max {
                    continue;
                }

                // Attack on cooldown -- emit AttackIntent into shared combat resolver.
                if tick.wrapping_sub(bb.last_melee_tick) >= MELEE_ATTACK_COOLDOWN_TICKS {
                    bb.last_melee_tick = tick;

                    if let Some(target_entity) = bb.nearest_player {
                        if target_entity != *actor {
                            attack_intents.send(AttackIntent {
                                attacker: *actor,
                                target: Some(target_entity),
                                issued_at_tick: u64::from(tick),
                                reach: profile.reach,
                                qi_invest: 0.0,
                                wound_kind: profile.wound_kind,
                                source: AttackSource::Melee,
                                debug_command: None,
                            });
                        }
                    }
                }
            }
            ActionState::Cancelled => {
                *state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

// ---------------------------------------------------------------------------
// DashAction
// ---------------------------------------------------------------------------

/// Activates a dash toward the player (Override movement).
#[derive(Clone, Copy, Debug, Component)]
pub struct DashAction;

impl ActionBuilder for DashAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("DashAction")
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn dash_action_system(
    mut npcs: Query<
        (
            &Position,
            &NpcBlackboard,
            &mut Navigator,
            &mut MovementController,
            &MovementCapabilities,
            &mut MovementCooldowns,
        ),
        With<NpcMarker>,
    >,
    mut actions: Query<(&Actor, &mut ActionState), With<DashAction>>,
    game_tick: Option<Res<GameTick>>,
) {
    let tick = game_tick.map(|t| t.0).unwrap_or(0);

    for (Actor(actor), mut state) in &mut actions {
        let Ok((npc_pos, blackboard, mut navigator, mut ctrl, caps, mut cooldowns)) =
            npcs.get_mut(*actor)
        else {
            *state = ActionState::Failure;
            continue;
        };

        match *state {
            ActionState::Requested => {
                let Some(target) = blackboard.target_position else {
                    *state = ActionState::Failure;
                    continue;
                };

                let npc = npc_pos.get();
                let dir = DVec3::new(target.x - npc.x, 0.0, target.z - npc.z);

                // Stop the navigator -- dash takes over.
                navigator.stop();

                let activated = activate_dash(
                    &mut ctrl,
                    caps,
                    &mut cooldowns,
                    tick,
                    dir,
                    npc.y, // ground Y at current position
                );

                if activated {
                    *state = ActionState::Executing;
                } else {
                    *state = ActionState::Failure;
                }
            }
            ActionState::Executing => {
                // Dash is done when MovementController returns to GroundNav.
                if !ctrl.navigator_should_yield() {
                    *state = ActionState::Success;
                }
            }
            ActionState::Cancelled => {
                ctrl.reset_to_ground();
                *state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

// ---------------------------------------------------------------------------
// NpcDefenseAction
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, Component)]
pub struct NpcDefenseAction {
    pub last_defense_tick: Option<u64>,
}

impl ActionBuilder for NpcDefenseAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("NpcDefenseAction")
    }
}

pub(crate) fn defense_interval_range(realm: crate::cultivation::components::Realm) -> (u64, u64) {
    use crate::cultivation::components::Realm;
    match realm {
        Realm::Induce => (80, 120),
        Realm::Condense => (60, 80),
        Realm::Solidify => (40, 60),
        Realm::Spirit | Realm::Void => (20, 40),
        Realm::Awaken => (u64::MAX, u64::MAX),
    }
}

fn deterministic_interval(entity_index: u32, tick: u64, min: u64, max: u64) -> u64 {
    if min >= max {
        return min;
    }
    let seed = (entity_index as u64)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(tick.wrapping_mul(0xBF58_476D_1CE4_E5B9));
    let range = max - min + 1;
    min + (seed % range)
}

#[allow(clippy::type_complexity)]
pub(crate) fn npc_defense_action_system(
    mut actions: Query<(&Actor, &mut ActionState, &mut NpcDefenseAction)>,
    npcs: Query<(&Cultivation, Option<&StatusEffects>), With<NpcMarker>>,
    mut defense_intents: EventWriter<DefenseIntent>,
    combat_clock: Option<Res<CombatClock>>,
) {
    let now_tick = combat_clock.as_deref().map(|c| c.tick).unwrap_or(0);

    for (Actor(actor), mut state, mut defense_action) in &mut actions {
        let Ok((cultivation, statuses_opt)) = npcs.get(*actor) else {
            if matches!(*state, ActionState::Requested | ActionState::Executing) {
                *state = ActionState::Failure;
            }
            continue;
        };

        match *state {
            ActionState::Requested => {
                if realm_rank(cultivation.realm)
                    < realm_rank(crate::cultivation::components::Realm::Induce)
                {
                    *state = ActionState::Failure;
                    continue;
                }
                let Some(qi_cost) = jiemai_qi_cost_for_realm(cultivation.realm) else {
                    *state = ActionState::Failure;
                    continue;
                };
                if cultivation.qi_current < qi_cost {
                    *state = ActionState::Failure;
                    continue;
                }
                if let Some(statuses) = statuses_opt {
                    if has_active_status(statuses, StatusEffectKind::ParryRecovery) {
                        *state = ActionState::Failure;
                        continue;
                    }
                }
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                let (min, max) = defense_interval_range(cultivation.realm);
                let interval = deterministic_interval(actor.index(), now_tick, min, max);
                let can_fire = match defense_action.last_defense_tick {
                    Some(last) => now_tick.saturating_sub(last) >= interval,
                    None => true,
                };

                if can_fire {
                    defense_intents.send(DefenseIntent {
                        defender: *actor,
                        issued_at_tick: now_tick,
                    });
                    defense_action.last_defense_tick = Some(now_tick);
                    *state = ActionState::Success;
                }
            }
            ActionState::Cancelled => {
                *state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::events::AttackIntent;
    use crate::npc::movement::GameTick;
    use crate::npc::navigator::Navigator;
    use crate::npc::patrol::NpcPatrol;
    use crate::world::zone::DEFAULT_SPAWN_ZONE_NAME;
    use bevy_transform::components::Transform;
    use big_brain::prelude::BigBrainSet;
    use valence::prelude::{App, IntoSystemConfigs, PreUpdate, Resource};

    #[derive(Default)]
    struct CapturedAttackIntents(Vec<AttackIntent>);

    impl Resource for CapturedAttackIntents {}

    fn capture_attack_intents(
        mut events: valence::prelude::EventReader<AttackIntent>,
        mut captured: valence::prelude::ResMut<CapturedAttackIntents>,
    ) {
        captured.0.extend(events.read().cloned());
    }

    #[test]
    fn flee_target_computation_moves_away_from_player() {
        let npc = DVec3::new(10.0, 67.0, 10.0);
        let player = DVec3::new(15.0, 67.0, 10.0);
        let target = compute_flee_target(npc, player, None, DEFAULT_SPAWN_ZONE_NAME);

        // Should flee in -X direction.
        assert!(target.x < npc.x, "flee target should be away from player");
    }

    #[test]
    fn flee_target_same_position_uses_fallback() {
        let pos = DVec3::new(10.0, 67.0, 10.0);
        let target = compute_flee_target(pos, pos, None, DEFAULT_SPAWN_ZONE_NAME);

        // Fallback direction is +X.
        assert!(target.x > pos.x);
    }

    #[test]
    fn flee_action_completes_above_sixteen_blocks() {
        let mut app = App::new();
        app.add_systems(PreUpdate, flee_action_system.in_set(BigBrainSet::Actions));

        let player = app
            .world_mut()
            .spawn((ClientMarker, Position::new([0.0, 66.0, 0.0])))
            .id();

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                EntityKind::ZOMBIE,
                Position::new([30.0, 66.0, 0.0]),
                Transform::from_xyz(30.0, 66.0, 0.0),
                NpcBlackboard {
                    nearest_player: Some(player),
                    player_distance: 30.0,
                    target_position: Some(DVec3::new(0.0, 66.0, 0.0)),
                    ..Default::default()
                },
                NpcMeleeProfile::default(),
                NpcPatrol::new(DEFAULT_SPAWN_ZONE_NAME, DVec3::new(30.0, 66.0, 0.0)),
                Navigator::new(),
            ))
            .id();

        let action_entity = app
            .world_mut()
            .spawn((Actor(npc), FleeAction, ActionState::Requested))
            .id();

        app.update();
        app.update();

        let action_state = app
            .world()
            .get::<ActionState>(action_entity)
            .expect("flee action entity should still exist");
        assert_eq!(*action_state, ActionState::Success);
    }

    #[test]
    fn melee_attack_action_bridges_to_attack_intent_without_knockback_side_path() {
        let mut app = App::new();
        app.insert_resource(GameTick(120));
        app.insert_resource(CapturedAttackIntents::default());
        app.add_event::<AttackIntent>();
        app.add_systems(
            PreUpdate,
            (
                melee_attack_action_system,
                capture_attack_intents.after(melee_attack_action_system),
            ),
        );

        let target = app
            .world_mut()
            .spawn((ClientMarker, Position::new([12.0, 66.0, 10.0])))
            .id();
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([10.0, 66.0, 10.0]),
                NpcBlackboard {
                    nearest_player: Some(target),
                    player_distance: 2.0,
                    target_position: Some(DVec3::new(12.0, 66.0, 10.0)),
                    ..Default::default()
                },
                NpcMeleeProfile::spear(),
                Navigator::new(),
            ))
            .id();
        let action_entity = app
            .world_mut()
            .spawn((Actor(npc), MeleeAttackAction, ActionState::Requested))
            .id();

        app.update();
        app.update();

        let action_state = app
            .world()
            .get::<ActionState>(action_entity)
            .expect("melee action entity should still exist");
        assert_eq!(*action_state, ActionState::Executing);

        let captured = &app.world().resource::<CapturedAttackIntents>().0;
        assert_eq!(
            captured.len(),
            1,
            "melee cooldown should emit one AttackIntent"
        );
        assert_eq!(captured[0].attacker, npc);
        assert_eq!(captured[0].target, Some(target));
        assert_eq!(captured[0].reach, NpcMeleeProfile::spear().reach);
        assert_eq!(captured[0].qi_invest, 0.0);
        assert_eq!(captured[0].wound_kind, NpcMeleeProfile::spear().wound_kind);
        assert_eq!(captured[0].debug_command, None);

        assert!(
            app.world()
                .get::<crate::npc::movement::PendingKnockback>(target)
                .is_none(),
            "melee bridge should not rely on PendingKnockback as primary damage path"
        );
    }

    #[test]
    fn melee_attack_action_same_tick_does_not_emit_duplicate_attack_intents() {
        let mut app = App::new();
        app.insert_resource(GameTick(240));
        app.insert_resource(CapturedAttackIntents::default());
        app.add_event::<AttackIntent>();
        app.add_systems(
            PreUpdate,
            (
                melee_attack_action_system,
                capture_attack_intents.after(melee_attack_action_system),
            ),
        );

        let target = app
            .world_mut()
            .spawn((ClientMarker, Position::new([12.0, 66.0, 10.0])))
            .id();
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([10.0, 66.0, 10.0]),
                NpcBlackboard {
                    nearest_player: Some(target),
                    player_distance: 2.0,
                    target_position: Some(DVec3::new(12.0, 66.0, 10.0)),
                    ..Default::default()
                },
                NpcMeleeProfile::spear(),
                Navigator::new(),
            ))
            .id();
        app.world_mut()
            .spawn((Actor(npc), MeleeAttackAction, ActionState::Requested));

        app.update();
        app.update();
        app.update();

        let captured = &app.world().resource::<CapturedAttackIntents>().0;
        assert_eq!(
            captured.len(),
            1,
            "same GameTick should not produce duplicate melee AttackIntent"
        );
    }

    #[test]
    fn melee_action_waits_inside_disengage_band_without_swinging() {
        let mut app = App::new();
        app.insert_resource(GameTick(240));
        app.insert_resource(CapturedAttackIntents::default());
        app.add_event::<AttackIntent>();
        app.add_systems(
            PreUpdate,
            (
                melee_attack_action_system,
                capture_attack_intents.after(melee_attack_action_system),
            ),
        );

        let target = app
            .world_mut()
            .spawn((ClientMarker, Position::new([13.2, 66.0, 10.0])))
            .id();
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([10.0, 66.0, 10.0]),
                NpcBlackboard {
                    nearest_player: Some(target),
                    player_distance: 3.2,
                    target_position: Some(DVec3::new(13.2, 66.0, 10.0)),
                    ..Default::default()
                },
                NpcMeleeProfile::spear(),
                Navigator::new(),
            ))
            .id();
        let action_entity = app
            .world_mut()
            .spawn((Actor(npc), MeleeAttackAction, ActionState::Requested))
            .id();

        app.update();
        app.update();

        let action_state = app.world().get::<ActionState>(action_entity).unwrap();
        let captured = &app.world().resource::<CapturedAttackIntents>().0;

        assert_eq!(*action_state, ActionState::Executing);
        assert!(
            captured.is_empty(),
            "npc should hold range instead of swinging outside reach"
        );
    }

    // ─── NpcDefenseAction tests ─────────────────────────────────────────────

    #[derive(Default)]
    struct CapturedDefenseIntents(Vec<DefenseIntent>);

    impl Resource for CapturedDefenseIntents {}

    fn capture_defense_intents(
        mut events: valence::prelude::EventReader<DefenseIntent>,
        mut captured: valence::prelude::ResMut<CapturedDefenseIntents>,
    ) {
        captured.0.extend(events.read().cloned());
    }

    #[test]
    fn defense_action_fails_for_awaken_realm() {
        use crate::cultivation::components::Cultivation;

        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 100 });
        app.insert_resource(CapturedDefenseIntents::default());
        app.add_event::<DefenseIntent>();
        app.add_systems(
            PreUpdate,
            (
                npc_defense_action_system,
                capture_defense_intents.after(npc_defense_action_system),
            ),
        );

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Cultivation {
                    realm: crate::cultivation::components::Realm::Awaken,
                    qi_current: 100.0,
                    ..Default::default()
                },
            ))
            .id();

        let action_entity = app
            .world_mut()
            .spawn((
                Actor(npc),
                NpcDefenseAction::default(),
                ActionState::Requested,
            ))
            .id();

        app.update();

        assert_eq!(
            *app.world().get::<ActionState>(action_entity).unwrap(),
            ActionState::Failure,
            "Awaken NPC should fail defense action (realm too low)"
        );
        assert!(
            app.world()
                .resource::<CapturedDefenseIntents>()
                .0
                .is_empty(),
            "no DefenseIntent should be emitted for Awaken"
        );
    }

    #[test]
    fn defense_action_fails_with_insufficient_qi() {
        use crate::cultivation::components::Cultivation;

        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 100 });
        app.insert_resource(CapturedDefenseIntents::default());
        app.add_event::<DefenseIntent>();
        app.add_systems(
            PreUpdate,
            (
                npc_defense_action_system,
                capture_defense_intents.after(npc_defense_action_system),
            ),
        );

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Cultivation {
                    realm: crate::cultivation::components::Realm::Induce,
                    qi_current: 1.0,
                    ..Default::default()
                },
            ))
            .id();

        let action_entity = app
            .world_mut()
            .spawn((
                Actor(npc),
                NpcDefenseAction::default(),
                ActionState::Requested,
            ))
            .id();

        app.update();

        assert_eq!(
            *app.world().get::<ActionState>(action_entity).unwrap(),
            ActionState::Failure,
            "NPC with insufficient qi should fail defense action"
        );
    }

    #[test]
    fn defense_action_fails_during_parry_recovery() {
        use crate::combat::components::{ActiveStatusEffect, StatusEffects};
        use crate::cultivation::components::Cultivation;

        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 100 });
        app.insert_resource(CapturedDefenseIntents::default());
        app.add_event::<DefenseIntent>();
        app.add_systems(
            PreUpdate,
            (
                npc_defense_action_system,
                capture_defense_intents.after(npc_defense_action_system),
            ),
        );

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Cultivation {
                    realm: crate::cultivation::components::Realm::Solidify,
                    qi_current: 100.0,
                    ..Default::default()
                },
                StatusEffects {
                    active: vec![ActiveStatusEffect {
                        kind: StatusEffectKind::ParryRecovery,
                        magnitude: 1.0,
                        remaining_ticks: 20,
                        source_pill: None,
                    }],
                },
            ))
            .id();

        let action_entity = app
            .world_mut()
            .spawn((
                Actor(npc),
                NpcDefenseAction::default(),
                ActionState::Requested,
            ))
            .id();

        app.update();

        assert_eq!(
            *app.world().get::<ActionState>(action_entity).unwrap(),
            ActionState::Failure,
            "NPC in ParryRecovery should fail defense action"
        );
    }

    #[test]
    fn defense_action_emits_intent_on_first_fire() {
        use crate::cultivation::components::Cultivation;

        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 200 });
        app.insert_resource(CapturedDefenseIntents::default());
        app.add_event::<DefenseIntent>();
        app.add_systems(
            PreUpdate,
            (
                npc_defense_action_system,
                capture_defense_intents.after(npc_defense_action_system),
            ),
        );

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Cultivation {
                    realm: crate::cultivation::components::Realm::Condense,
                    qi_current: 100.0,
                    ..Default::default()
                },
            ))
            .id();

        let action_entity = app
            .world_mut()
            .spawn((
                Actor(npc),
                NpcDefenseAction::default(),
                ActionState::Requested,
            ))
            .id();

        app.update();
        app.update();

        let state = app
            .world()
            .get::<ActionState>(action_entity)
            .unwrap()
            .clone();
        let captured = &app.world().resource::<CapturedDefenseIntents>().0;

        assert_eq!(
            state,
            ActionState::Success,
            "defense action should succeed after emitting DefenseIntent"
        );
        assert_eq!(
            captured.len(),
            1,
            "exactly one DefenseIntent should be emitted"
        );
        assert_eq!(
            captured[0].defender, npc,
            "DefenseIntent.defender should be the NPC entity"
        );
        assert_eq!(
            captured[0].issued_at_tick, 200,
            "DefenseIntent.issued_at_tick should match CombatClock"
        );
    }

    #[test]
    fn defense_action_cancelled_becomes_failure() {
        use crate::cultivation::components::Cultivation;

        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 100 });
        app.insert_resource(CapturedDefenseIntents::default());
        app.add_event::<DefenseIntent>();
        app.add_systems(
            PreUpdate,
            (
                npc_defense_action_system,
                capture_defense_intents.after(npc_defense_action_system),
            ),
        );

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Cultivation {
                    realm: crate::cultivation::components::Realm::Solidify,
                    qi_current: 100.0,
                    ..Default::default()
                },
            ))
            .id();

        let action_entity = app
            .world_mut()
            .spawn((
                Actor(npc),
                NpcDefenseAction::default(),
                ActionState::Cancelled,
            ))
            .id();

        app.update();

        assert_eq!(
            *app.world().get::<ActionState>(action_entity).unwrap(),
            ActionState::Failure,
            "cancelled defense action should transition to Failure"
        );
    }

    #[test]
    fn defense_action_void_realm_fails_because_qi_cost_is_none() {
        use crate::cultivation::components::Cultivation;

        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 100 });
        app.insert_resource(CapturedDefenseIntents::default());
        app.add_event::<DefenseIntent>();
        app.add_systems(
            PreUpdate,
            (
                npc_defense_action_system,
                capture_defense_intents.after(npc_defense_action_system),
            ),
        );

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Cultivation {
                    realm: crate::cultivation::components::Realm::Void,
                    qi_current: 1000.0,
                    ..Default::default()
                },
            ))
            .id();

        let action_entity = app
            .world_mut()
            .spawn((
                Actor(npc),
                NpcDefenseAction::default(),
                ActionState::Requested,
            ))
            .id();

        app.update();

        assert_eq!(
            *app.world().get::<ActionState>(action_entity).unwrap(),
            ActionState::Failure,
            "Void realm has no qi cost for jiemai defense, so action should fail"
        );
    }

    // ─── defense_interval_range tests ───────────────────────────────────────

    #[test]
    fn defense_interval_induce_range() {
        let (min, max) = defense_interval_range(crate::cultivation::components::Realm::Induce);
        assert_eq!((min, max), (80, 120), "Induce interval should be 80..=120");
    }

    #[test]
    fn defense_interval_condense_range() {
        let (min, max) = defense_interval_range(crate::cultivation::components::Realm::Condense);
        assert_eq!((min, max), (60, 80), "Condense interval should be 60..=80");
    }

    #[test]
    fn defense_interval_solidify_range() {
        let (min, max) = defense_interval_range(crate::cultivation::components::Realm::Solidify);
        assert_eq!((min, max), (40, 60), "Solidify interval should be 40..=60");
    }

    #[test]
    fn defense_interval_spirit_and_void_range() {
        for realm in [
            crate::cultivation::components::Realm::Spirit,
            crate::cultivation::components::Realm::Void,
        ] {
            let (min, max) = defense_interval_range(realm);
            assert_eq!(
                (min, max),
                (20, 40),
                "realm {realm:?} interval should be 20..=40"
            );
        }
    }

    #[test]
    fn defense_interval_awaken_is_max() {
        let (min, max) = defense_interval_range(crate::cultivation::components::Realm::Awaken);
        assert_eq!(
            min,
            u64::MAX,
            "Awaken should have u64::MAX interval (never fires)"
        );
        assert_eq!(max, u64::MAX);
    }

    #[test]
    fn deterministic_interval_stays_within_range() {
        for entity_idx in 0..100u32 {
            for tick in [0u64, 50, 100, 500, 1000] {
                let interval = deterministic_interval(entity_idx, tick, 60, 80);
                assert!(
                    (60..=80).contains(&interval),
                    "entity {entity_idx} tick {tick}: interval {interval} out of range 60..=80"
                );
            }
        }
    }

    #[test]
    fn deterministic_interval_min_equals_max() {
        let interval = deterministic_interval(42, 100, 50, 50);
        assert_eq!(interval, 50, "when min == max, interval should equal min");
    }

    #[test]
    fn defense_action_respects_interval_cooldown() {
        use crate::cultivation::components::Cultivation;

        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 50 });
        app.insert_resource(CapturedDefenseIntents::default());
        app.add_event::<DefenseIntent>();
        app.add_systems(
            PreUpdate,
            (
                npc_defense_action_system,
                capture_defense_intents.after(npc_defense_action_system),
            ),
        );

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Cultivation {
                    realm: crate::cultivation::components::Realm::Solidify,
                    qi_current: 100.0,
                    ..Default::default()
                },
            ))
            .id();

        let action_entity = app
            .world_mut()
            .spawn((
                Actor(npc),
                NpcDefenseAction {
                    last_defense_tick: Some(49),
                },
                ActionState::Executing,
            ))
            .id();

        app.update();

        let state = app
            .world()
            .get::<ActionState>(action_entity)
            .unwrap()
            .clone();
        let captured = &app.world().resource::<CapturedDefenseIntents>().0;

        assert_eq!(
            state,
            ActionState::Executing,
            "defense action should remain Executing when interval not elapsed (50 - 49 = 1, need >= 40)"
        );
        assert!(
            captured.is_empty(),
            "no DefenseIntent should be emitted before interval expires"
        );
    }

    #[test]
    fn defense_action_fires_after_interval_elapsed() {
        use crate::cultivation::components::Cultivation;

        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 200 });
        app.insert_resource(CapturedDefenseIntents::default());
        app.add_event::<DefenseIntent>();
        app.add_systems(
            PreUpdate,
            (
                npc_defense_action_system,
                capture_defense_intents.after(npc_defense_action_system),
            ),
        );

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Cultivation {
                    realm: crate::cultivation::components::Realm::Solidify,
                    qi_current: 100.0,
                    ..Default::default()
                },
            ))
            .id();

        let action_entity = app
            .world_mut()
            .spawn((
                Actor(npc),
                NpcDefenseAction {
                    last_defense_tick: Some(100),
                },
                ActionState::Executing,
            ))
            .id();

        app.update();

        let state = app
            .world()
            .get::<ActionState>(action_entity)
            .unwrap()
            .clone();
        let captured = &app.world().resource::<CapturedDefenseIntents>().0;

        assert_eq!(
            state,
            ActionState::Success,
            "defense action should succeed when interval elapsed (200 - 100 = 100, need >= 40)"
        );
        assert_eq!(captured.len(), 1, "one DefenseIntent should be emitted");
    }

    #[test]
    fn defense_action_fails_when_npc_entity_missing() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 100 });
        app.insert_resource(CapturedDefenseIntents::default());
        app.add_event::<DefenseIntent>();
        app.add_systems(
            PreUpdate,
            (
                npc_defense_action_system,
                capture_defense_intents.after(npc_defense_action_system),
            ),
        );

        let phantom_entity = app.world_mut().spawn_empty().id();

        let action_entity = app
            .world_mut()
            .spawn((
                Actor(phantom_entity),
                NpcDefenseAction::default(),
                ActionState::Requested,
            ))
            .id();

        app.update();

        assert_eq!(
            *app.world().get::<ActionState>(action_entity).unwrap(),
            ActionState::Failure,
            "defense action should fail when actor entity lacks NpcMarker + Cultivation"
        );
    }

    #[test]
    fn defense_action_induce_emits_intent_for_valid_npc() {
        use crate::cultivation::components::Cultivation;

        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 300 });
        app.insert_resource(CapturedDefenseIntents::default());
        app.add_event::<DefenseIntent>();
        app.add_systems(
            PreUpdate,
            (
                npc_defense_action_system,
                capture_defense_intents.after(npc_defense_action_system),
            ),
        );

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Cultivation {
                    realm: crate::cultivation::components::Realm::Induce,
                    qi_current: 50.0,
                    ..Default::default()
                },
            ))
            .id();

        let action_entity = app
            .world_mut()
            .spawn((
                Actor(npc),
                NpcDefenseAction::default(),
                ActionState::Requested,
            ))
            .id();

        app.update();
        app.update();

        let state = app
            .world()
            .get::<ActionState>(action_entity)
            .unwrap()
            .clone();
        let captured = &app.world().resource::<CapturedDefenseIntents>().0;

        assert_eq!(
            state,
            ActionState::Success,
            "Induce NPC with sufficient qi should succeed"
        );
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].defender, npc);
    }
}
