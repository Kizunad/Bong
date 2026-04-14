use big_brain::prelude::{
    ActionBuilder, ActionState, Actor, BigBrainPlugin, BigBrainSet, Score, ScorerBuilder,
};
use std::collections::HashMap;
use valence::client::ClientMarker;
use valence::prelude::{
    bevy_ecs, App, Commands, Component, DVec3, Entity, EntityKind, EventReader, EventWriter,
    IntoSystemConfigs, Position, PreUpdate, Query, Res, Resource, With, Without,
};

use crate::combat::events::AttackIntent;
use crate::npc::movement::{
    activate_dash, activate_sprint, GameTick, MovementCapabilities, MovementController,
    MovementCooldowns, MovementMode,
};
use crate::npc::navigator::Navigator;
use crate::npc::patrol::NpcPatrol;
use crate::npc::spawn::{DuelTarget, NpcBlackboard, NpcMarker};
use crate::world::zone::ZoneRegistry;

pub const DEFAULT_FLEE_THRESHOLD: f32 = 0.6;
pub(crate) const PROXIMITY_THRESHOLD: f32 = DEFAULT_FLEE_THRESHOLD;
const FLEE_SUCCESS_DISTANCE: f64 = 16.0;
const FLEE_SPEED_FACTOR: f64 = 1.2;
const CHASE_RANGE: f32 = 32.0;
const CHASE_SPEED_FACTOR: f64 = 1.0;
const CHASE_ARRIVE_DISTANCE: f32 = 3.0;
const MELEE_RANGE: f32 = 3.5;
/// How far ahead of the NPC to place the flee waypoint.
const FLEE_WAYPOINT_DISTANCE: f64 = 8.0;

/// Sprint activates when chasing and player is farther than this.
const SPRINT_ACTIVATE_DISTANCE: f32 = 10.0;
/// Sprint deactivates when player is closer than this.
const SPRINT_DEACTIVATE_DISTANCE: f32 = 5.0;
/// Cooldown when sprint is manually cancelled (shorter than natural expiry).
const SPRINT_DEACTIVATE_COOLDOWN: u32 = 30;

/// Dash is considered when player distance is in this range.
const DASH_MIN_DISTANCE: f32 = 5.0;
const DASH_MAX_DISTANCE: f32 = 14.0;

/// Melee attack fires every N ticks (20 tps → 1.5 seconds).
const MELEE_ATTACK_COOLDOWN_TICKS: u32 = 30;

#[derive(Clone, Copy, Debug, Component)]
pub struct PlayerProximityScorer;

/// Scores high when a player is within [`CHASE_RANGE`] blocks.
#[derive(Clone, Copy, Debug, Component)]
pub struct ChaseTargetScorer;

/// Scores high (1.0) when a player is within [`MELEE_RANGE`] blocks.
#[derive(Clone, Copy, Debug, Component)]
pub struct MeleeRangeScorer;

#[derive(Clone, Copy, Debug, Component)]
pub struct FleeAction;

/// Moves the NPC toward the nearest player.
#[derive(Clone, Copy, Debug, Component)]
pub struct ChaseAction;

/// NPC stands still "attacking" while the player is in melee range.
#[derive(Clone, Copy, Debug, Component)]
pub struct MeleeAttackAction;

/// Scores high when the player is within dash range and dash is off cooldown.
#[derive(Clone, Copy, Debug, Component)]
pub struct DashScorer;

/// Activates a dash toward the player (Override movement).
#[derive(Clone, Copy, Debug, Component)]
pub struct DashAction;

#[derive(Clone, Debug)]
pub struct NpcBehaviorConfig {
    pub default_flee_threshold: f32,
    flee_threshold_overrides: HashMap<String, f32>,
}

impl Default for NpcBehaviorConfig {
    fn default() -> Self {
        Self {
            default_flee_threshold: DEFAULT_FLEE_THRESHOLD,
            flee_threshold_overrides: HashMap::new(),
        }
    }
}

impl Resource for NpcBehaviorConfig {}

pub fn canonical_npc_id(entity: Entity) -> String {
    format!("npc_{}v{}", entity.index(), entity.generation())
}

impl NpcBehaviorConfig {
    pub fn threshold_for_npc(&self, npc: Entity) -> f32 {
        let npc_id = canonical_npc_id(npc);
        self.threshold_for_npc_id(npc_id.as_str())
    }

    pub fn threshold_for_npc_id(&self, npc_id: &str) -> f32 {
        self.flee_threshold_overrides
            .get(npc_id)
            .copied()
            .unwrap_or(self.default_flee_threshold)
    }

    pub fn set_threshold_for_npc_id(&mut self, npc_id: impl Into<String>, flee_threshold: f32) {
        self.flee_threshold_overrides
            .insert(npc_id.into(), flee_threshold.clamp(0.0, 1.0));
    }
}

type NpcGoalQueryItem<'a> = (
    &'a Position,
    &'a NpcBlackboard,
    &'a NpcPatrol,
    &'a mut Navigator,
);
type NpcGoalQueryFilter = (With<NpcMarker>, With<EntityKind>, Without<ClientMarker>);

impl ScorerBuilder for PlayerProximityScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("PlayerProximityScorer")
    }
}

impl ActionBuilder for FleeAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("FleeAction")
    }
}

impl ScorerBuilder for ChaseTargetScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("ChaseTargetScorer")
    }
}

impl ScorerBuilder for MeleeRangeScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("MeleeRangeScorer")
    }
}

impl ActionBuilder for ChaseAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("ChaseAction")
    }
}

impl ActionBuilder for MeleeAttackAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("MeleeAttackAction")
    }
}

impl ScorerBuilder for DashScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("DashScorer")
    }
}

impl ActionBuilder for DashAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("DashAction")
    }
}

pub fn register(app: &mut App) {
    tracing::info!("[bong][npc] registering brain systems");
    app.insert_resource(NpcBehaviorConfig::default())
        .add_plugins(BigBrainPlugin::new(PreUpdate))
        .add_systems(
            PreUpdate,
            update_npc_blackboard.before(BigBrainSet::Scorers),
        )
        .add_systems(
            PreUpdate,
            (
                player_proximity_scorer_system,
                chase_target_scorer_system,
                melee_range_scorer_system,
                dash_scorer_system,
            )
                .in_set(BigBrainSet::Scorers),
        )
        .add_systems(
            PreUpdate,
            (
                flee_action_system,
                chase_action_system,
                melee_attack_action_system,
                dash_action_system,
            )
                .in_set(BigBrainSet::Actions),
        );
}

pub fn update_npc_blackboard(
    mut npc_query: Query<(&Position, &mut NpcBlackboard, Option<&DuelTarget>), With<NpcMarker>>,
    player_query: Query<(Entity, &Position), With<ClientMarker>>,
    all_positions: Query<&Position>,
) {
    for (npc_position, mut blackboard, duel_target) in &mut npc_query {
        let npc_pos = npc_position.get();

        // Duel override: target a specific entity instead of nearest player.
        if let Some(DuelTarget(target_entity)) = duel_target {
            if let Ok(target_pos) = all_positions.get(*target_entity) {
                let dist = npc_pos.distance(target_pos.get());
                blackboard.nearest_player = Some(*target_entity);
                blackboard.player_distance = dist as f32;
                blackboard.target_position = Some(target_pos.get());
                continue;
            }
        }

        let mut nearest_player = None;
        let mut nearest_distance = f64::INFINITY;
        let mut nearest_pos = None;

        for (player_entity, player_position) in &player_query {
            let distance = npc_pos.distance(player_position.get());
            if distance < nearest_distance {
                nearest_distance = distance;
                nearest_player = Some(player_entity);
                nearest_pos = Some(player_position.get());
            }
        }

        blackboard.nearest_player = nearest_player;
        blackboard.player_distance = nearest_distance as f32;
        blackboard.target_position = nearest_pos;
    }
}

fn player_proximity_scorer_system(
    npcs: Query<&NpcBlackboard, With<NpcMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<PlayerProximityScorer>>,
    npc_behavior: Option<Res<NpcBehaviorConfig>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let flee_threshold = npc_behavior
            .as_deref()
            .map(|behavior| behavior.threshold_for_npc(*actor))
            .unwrap_or(DEFAULT_FLEE_THRESHOLD)
            .clamp(0.0, 1.0);

        let value = if let Ok(blackboard) = npcs.get(*actor) {
            score_for_flee_threshold(proximity_score(blackboard.player_distance), flee_threshold)
        } else {
            0.0
        };

        score.set(value);
    }
}

fn score_for_flee_threshold(score: f32, flee_threshold: f32) -> f32 {
    if score >= flee_threshold {
        1.0
    } else {
        0.0
    }
}

// ---------------------------------------------------------------------------
// Flee action — sets Navigator goal away from player
// ---------------------------------------------------------------------------

fn flee_action_system(
    mut npcs: Query<NpcGoalQueryItem<'_>, NpcGoalQueryFilter>,
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
fn compute_flee_target(
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

fn proximity_score(distance: f32) -> f32 {
    if !distance.is_finite() {
        return 0.0;
    }

    ((8.0 - distance) / 8.0).clamp(0.0, 1.0)
}

#[cfg(test)]
fn should_flee_from_score(score: f32) -> bool {
    score >= PROXIMITY_THRESHOLD
}

// ---------------------------------------------------------------------------
// Chase action — sets Navigator goal toward the player
// ---------------------------------------------------------------------------

fn chase_target_scorer_system(
    npcs: Query<&NpcBlackboard, With<NpcMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<ChaseTargetScorer>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let value = if let Ok(bb) = npcs.get(*actor) {
            chase_score(bb.player_distance)
        } else {
            0.0
        };
        score.set(value);
    }
}

fn chase_score(distance: f32) -> f32 {
    if !distance.is_finite() || distance > CHASE_RANGE {
        return 0.0;
    }
    ((CHASE_RANGE - distance) / CHASE_RANGE).clamp(0.0, 1.0)
}

fn chase_action_system(
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
        let Ok((_npc_position, blackboard, _patrol, mut navigator)) = npcs.get_mut(*actor) else {
            *state = ActionState::Failure;
            continue;
        };

        match *state {
            ActionState::Requested => {
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                if blackboard.player_distance <= CHASE_ARRIVE_DISTANCE {
                    navigator.stop();
                    *state = ActionState::Success;
                    continue;
                }

                let Some(target_pos) = blackboard.target_position else {
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
// Melee attack — NPC stands still
// ---------------------------------------------------------------------------

fn melee_range_scorer_system(
    npcs: Query<&NpcBlackboard, With<NpcMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<MeleeRangeScorer>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let value = if let Ok(bb) = npcs.get(*actor) {
            if bb.player_distance <= MELEE_RANGE {
                1.0
            } else {
                0.0
            }
        } else {
            0.0
        };
        score.set(value);
    }
}

fn melee_attack_action_system(
    mut actions: Query<(&Actor, &mut ActionState), With<MeleeAttackAction>>,
    mut npcs: Query<(&Position, &mut NpcBlackboard, &mut Navigator), With<NpcMarker>>,
    mut attack_intents: EventWriter<AttackIntent>,
    game_tick: Option<Res<GameTick>>,
) {
    let tick = game_tick.map(|t| t.0).unwrap_or(0);

    for (Actor(actor), mut state) in &mut actions {
        match *state {
            ActionState::Requested => {
                if let Ok((_, _, mut nav)) = npcs.get_mut(*actor) {
                    nav.stop();
                }
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                let Ok((_npc_pos, mut bb, _)) = npcs.get_mut(*actor) else {
                    continue;
                };

                if bb.player_distance > MELEE_RANGE * 1.5 {
                    *state = ActionState::Success;
                    continue;
                }

                // Attack on cooldown — emit AttackIntent into shared combat resolver.
                if tick.wrapping_sub(bb.last_melee_tick) >= MELEE_ATTACK_COOLDOWN_TICKS {
                    bb.last_melee_tick = tick;

                    if let Some(target_entity) = bb.nearest_player {
                        if target_entity != *actor {
                            attack_intents.send(AttackIntent {
                                attacker: *actor,
                                target: Some(target_entity),
                                issued_at_tick: u64::from(tick),
                                reach: MELEE_RANGE,
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
// Dash — short-range burst toward the player (Override movement)
// ---------------------------------------------------------------------------

fn dash_scorer_system(
    npcs: Query<
        (
            &NpcBlackboard,
            &MovementCapabilities,
            &MovementCooldowns,
            &MovementController,
        ),
        With<NpcMarker>,
    >,
    mut scorers: Query<(&Actor, &mut Score), With<DashScorer>>,
    game_tick: Option<Res<GameTick>>,
) {
    let tick = game_tick.map(|t| t.0).unwrap_or(0);

    for (Actor(actor), mut score) in &mut scorers {
        let value = if let Ok((bb, caps, cooldowns, ctrl)) = npcs.get(*actor) {
            dash_score(bb, caps, cooldowns, ctrl, tick)
        } else {
            0.0
        };
        score.set(value);
    }
}

fn dash_score(
    bb: &NpcBlackboard,
    caps: &MovementCapabilities,
    cooldowns: &MovementCooldowns,
    ctrl: &MovementController,
    current_tick: u32,
) -> f32 {
    if !caps.can_dash {
        return 0.0;
    }
    if current_tick < cooldowns.dash_ready_at {
        return 0.0;
    }
    if ctrl.navigator_should_yield() {
        return 0.0; // already in an override
    }
    if !bb.player_distance.is_finite() {
        return 0.0;
    }
    if bb.player_distance < DASH_MIN_DISTANCE || bb.player_distance > DASH_MAX_DISTANCE {
        return 0.0;
    }

    // Score high — dash should take priority over regular chase when available.
    0.9
}

#[allow(clippy::type_complexity)]
fn dash_action_system(
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

                // Stop the navigator — dash takes over.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::events::AttackIntent;
    use crate::npc::movement::{MovementCapabilities, MovementController, MovementCooldowns};
    use crate::npc::navigator::Navigator;
    use crate::npc::patrol::NpcPatrol;
    use crate::world::zone::DEFAULT_SPAWN_ZONE_NAME;
    use bevy_transform::components::Transform;
    use big_brain::prelude::{FirstToScore, Thinker};
    use valence::prelude::{App, Position};

    #[derive(Default)]
    struct CapturedAttackIntents(Vec<AttackIntent>);

    impl valence::prelude::Resource for CapturedAttackIntents {}

    fn capture_attack_intents(
        mut events: EventReader<AttackIntent>,
        mut captured: valence::prelude::ResMut<CapturedAttackIntents>,
    ) {
        captured.0.extend(events.read().cloned());
    }

    #[test]
    fn player_proximity_scorer_thresholds() {
        let score_at_just_inside_threshold_distance = proximity_score(3.2);
        let score_at_exact_threshold_distance = proximity_score(3.2);
        let score_just_outside_threshold_distance = proximity_score(3.3);
        let score_out_of_range = proximity_score(8.0);

        assert!(
            should_flee_from_score(score_at_just_inside_threshold_distance),
            "3.2 blocks should meet threshold"
        );
        assert!(
            should_flee_from_score(score_at_exact_threshold_distance),
            "exact threshold score should trigger flee"
        );
        assert!(
            !should_flee_from_score(score_just_outside_threshold_distance),
            "3.3 blocks should fall under threshold"
        );
        assert_eq!(score_out_of_range, 0.0, "8+ blocks should score 0");

        let thinker = Thinker::build()
            .picker(FirstToScore {
                threshold: PROXIMITY_THRESHOLD,
            })
            .when(PlayerProximityScorer, FleeAction);
        let mut app = App::new();
        app.world_mut().spawn(thinker);
        assert_eq!(PROXIMITY_THRESHOLD, 0.6);
        assert!((proximity_score(3.2) - 0.6).abs() < 1e-6);
    }

    #[test]
    fn npc_behavior_config_defaults_to_proximity_threshold() {
        let config = NpcBehaviorConfig::default();
        assert_eq!(config.default_flee_threshold, PROXIMITY_THRESHOLD);
        assert_eq!(config.threshold_for_npc_id("npc_1v1"), PROXIMITY_THRESHOLD);
    }

    #[test]
    fn npc_behavior_config_applies_per_npc_override() {
        let mut config = NpcBehaviorConfig::default();
        config.set_threshold_for_npc_id("npc_7v3", 0.2);

        assert_eq!(config.threshold_for_npc_id("npc_7v3"), 0.2);
        assert_eq!(config.threshold_for_npc_id("npc_8v3"), PROXIMITY_THRESHOLD);
    }

    #[test]
    fn canonical_npc_id_is_generation_aware() {
        let mut app = App::new();
        let entity = app.world_mut().spawn_empty().id();

        assert_eq!(
            canonical_npc_id(entity),
            format!("npc_{}v{}", entity.index(), entity.generation())
        );
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
    fn chase_score_within_range() {
        assert!(chase_score(10.0) > 0.0);
        assert!(chase_score(32.0) > -f32::EPSILON);
        assert_eq!(chase_score(33.0), 0.0);
        assert_eq!(chase_score(f32::INFINITY), 0.0);
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
    fn bridge_less_no_player_behavior() {
        let mut app = App::new();
        app.add_systems(PreUpdate, update_npc_blackboard);

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([14.0, 66.0, 14.0]),
                NpcBlackboard::default(),
            ))
            .id();

        app.update();

        let blackboard = app
            .world()
            .get::<NpcBlackboard>(npc)
            .expect("NPC blackboard should exist");

        assert!(
            blackboard.nearest_player.is_none(),
            "without players, nearest_player must remain None"
        );
        assert!(
            blackboard.player_distance.is_infinite(),
            "without players, distance must remain infinity"
        );
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
        assert_eq!(captured.len(), 1, "melee cooldown should emit one AttackIntent");
        assert_eq!(captured[0].attacker, npc);
        assert_eq!(captured[0].target, Some(target));
        assert_eq!(captured[0].reach, MELEE_RANGE);
        assert_eq!(captured[0].debug_command, None);

        assert!(
            app.world().get::<crate::npc::movement::PendingKnockback>(target).is_none(),
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
    fn dash_score_zero_without_capability() {
        let bb = NpcBlackboard {
            player_distance: 8.0,
            ..Default::default()
        };
        let caps = MovementCapabilities {
            can_sprint: true,
            can_dash: false,
        };
        let cd = MovementCooldowns::default();
        let ctrl = MovementController::new();

        assert_eq!(dash_score(&bb, &caps, &cd, &ctrl, 0), 0.0);
    }

    #[test]
    fn dash_score_positive_in_range_with_capability() {
        let bb = NpcBlackboard {
            player_distance: 8.0, // within DASH_MIN..DASH_MAX
            ..Default::default()
        };
        let caps = MovementCapabilities {
            can_sprint: true,
            can_dash: true,
        };
        let cd = MovementCooldowns::default();
        let ctrl = MovementController::new();

        assert!(dash_score(&bb, &caps, &cd, &ctrl, 0) > 0.0);
    }

    #[test]
    fn dash_score_zero_on_cooldown() {
        let bb = NpcBlackboard {
            player_distance: 8.0,
            ..Default::default()
        };
        let caps = MovementCapabilities {
            can_sprint: true,
            can_dash: true,
        };
        let cd = MovementCooldowns {
            sprint_ready_at: 0,
            dash_ready_at: 100, // cooldown active
        };
        let ctrl = MovementController::new();

        assert_eq!(dash_score(&bb, &caps, &cd, &ctrl, 50), 0.0);
    }

    #[test]
    fn dash_score_zero_outside_range() {
        let bb_too_close = NpcBlackboard {
            player_distance: 3.0, // < DASH_MIN_DISTANCE
            ..Default::default()
        };
        let bb_too_far = NpcBlackboard {
            player_distance: 20.0, // > DASH_MAX_DISTANCE
            ..Default::default()
        };
        let caps = MovementCapabilities {
            can_sprint: true,
            can_dash: true,
        };
        let cd = MovementCooldowns::default();
        let ctrl = MovementController::new();

        assert_eq!(dash_score(&bb_too_close, &caps, &cd, &ctrl, 0), 0.0);
        assert_eq!(dash_score(&bb_too_far, &caps, &cd, &ctrl, 0), 0.0);
    }
}
