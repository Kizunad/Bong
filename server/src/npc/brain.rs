use bevy_transform::components::Transform;
use big_brain::prelude::{
    ActionBuilder, ActionState, Actor, BigBrainPlugin, BigBrainSet, Score, ScorerBuilder,
};
use std::collections::HashMap;
use valence::client::ClientMarker;
use valence::prelude::{
    bevy_ecs, App, Commands, Component, DVec3, Entity, EntityKind, IntoSystemConfigs, Position,
    PreUpdate, Query, Res, Resource, With, Without,
};

use crate::npc::spawn::{NpcBlackboard, NpcMarker};

pub const DEFAULT_FLEE_THRESHOLD: f32 = 0.6;
pub(crate) const PROXIMITY_THRESHOLD: f32 = DEFAULT_FLEE_THRESHOLD;
const FLEE_SUCCESS_DISTANCE: f64 = 16.0;
const FLEE_SPEED: f64 = 0.15;
const FALLBACK_FLEE_DIR: DVec3 = DVec3::new(1.0, 0.0, 0.0);
const PLATFORM_MIN_XZ: f64 = 0.5;
const PLATFORM_MAX_XZ: f64 = 255.5;
const NPC_FIXED_Y: f64 = 66.0;

#[derive(Clone, Copy, Debug, Component)]
pub struct PlayerProximityScorer;

#[derive(Clone, Copy, Debug, Component)]
pub struct FleeAction;

#[derive(Clone, Debug)]
pub struct NpcBehaviorConfig {
    pub default_flee_threshold: f32,
    flee_threshold_overrides: HashMap<u32, f32>,
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

impl NpcBehaviorConfig {
    pub fn threshold_for_npc_index(&self, npc_index: u32) -> f32 {
        self.flee_threshold_overrides
            .get(&npc_index)
            .copied()
            .unwrap_or(self.default_flee_threshold)
    }

    pub fn set_threshold_for_npc_index(&mut self, npc_index: u32, flee_threshold: f32) {
        self.flee_threshold_overrides
            .insert(npc_index, flee_threshold.clamp(0.0, 1.0));
    }
}

type NpcFleeQueryItem<'a> = (&'a mut Position, &'a mut Transform, &'a NpcBlackboard);
type NpcFleeQueryFilter = (With<NpcMarker>, With<EntityKind>, Without<ClientMarker>);

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
            player_proximity_scorer_system.in_set(BigBrainSet::Scorers),
        )
        .add_systems(PreUpdate, flee_action_system.in_set(BigBrainSet::Actions));
}

pub fn update_npc_blackboard(
    mut npc_query: Query<(&Position, &mut NpcBlackboard), With<NpcMarker>>,
    player_query: Query<(Entity, &Position), With<ClientMarker>>,
) {
    for (npc_position, mut blackboard) in &mut npc_query {
        let mut nearest_player = None;
        let mut nearest_distance = f64::INFINITY;

        let npc_pos = npc_position.get();

        for (player_entity, player_position) in &player_query {
            let distance = npc_pos.distance(player_position.get());
            if distance < nearest_distance {
                nearest_distance = distance;
                nearest_player = Some(player_entity);
            }
        }

        blackboard.nearest_player = nearest_player;
        blackboard.player_distance = nearest_distance as f32;
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
            .map(|behavior| behavior.threshold_for_npc_index(actor.index()))
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

fn flee_action_system(
    mut npcs: Query<NpcFleeQueryItem<'_>, NpcFleeQueryFilter>,
    players: Query<&Position, (With<ClientMarker>, Without<NpcMarker>)>,
    mut actions: Query<(&Actor, &mut ActionState), With<FleeAction>>,
) {
    for (Actor(actor), mut state) in &mut actions {
        let Ok((mut npc_position, mut npc_transform, blackboard)) = npcs.get_mut(*actor) else {
            *state = ActionState::Failure;
            continue;
        };

        match *state {
            ActionState::Requested => {
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                if blackboard.player_distance > FLEE_SUCCESS_DISTANCE as f32 {
                    *state = ActionState::Success;
                    continue;
                }

                let Some(player_entity) = blackboard.nearest_player else {
                    continue;
                };

                let Ok(player_position) = players.get(player_entity) else {
                    continue;
                };

                let next_pos = next_flee_position(npc_position.get(), player_position.get());
                npc_position.set(next_pos);
                npc_transform.translation.x = next_pos.x as f32;
                npc_transform.translation.y = next_pos.y as f32;
                npc_transform.translation.z = next_pos.z as f32;
            }
            ActionState::Cancelled => {
                *state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
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

fn next_flee_position(npc_pos: DVec3, player_pos: DVec3) -> DVec3 {
    let mut flee_dir = npc_pos - player_pos;
    flee_dir.y = 0.0;

    let movement_dir = if flee_dir.length_squared() <= f64::EPSILON {
        FALLBACK_FLEE_DIR
    } else {
        flee_dir.normalize()
    };

    let tentative = npc_pos + (movement_dir * FLEE_SPEED);
    DVec3::new(
        tentative.x.clamp(PLATFORM_MIN_XZ, PLATFORM_MAX_XZ),
        NPC_FIXED_Y,
        tentative.z.clamp(PLATFORM_MIN_XZ, PLATFORM_MAX_XZ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_transform::components::Transform;
    use big_brain::prelude::{FirstToScore, Thinker};
    use valence::client::ClientMarker;
    use valence::prelude::{App, Position};

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
        assert_eq!(config.threshold_for_npc_index(1), PROXIMITY_THRESHOLD);
    }

    #[test]
    fn npc_behavior_config_applies_per_npc_override() {
        let mut config = NpcBehaviorConfig::default();
        config.set_threshold_for_npc_index(7, 0.2);

        assert_eq!(config.threshold_for_npc_index(7), 0.2);
        assert_eq!(config.threshold_for_npc_index(8), PROXIMITY_THRESHOLD);
    }

    #[test]
    fn flee_action_handles_zero_vector() {
        let npc_pos = DVec3::new(14.0, 66.0, 14.0);
        let player_pos = DVec3::new(14.0, 66.0, 14.0);

        let next = next_flee_position(npc_pos, player_pos);

        assert!(
            (next.x - (14.0 + FLEE_SPEED)).abs() < 1e-9,
            "fallback direction should move +X by speed"
        );
        assert_eq!(next.y, NPC_FIXED_Y);
        assert_eq!(next.z, 14.0);
    }

    #[test]
    fn flee_action_clamps_to_platform_bounds() {
        let npc_near_max = DVec3::new(255.49, 66.0, 255.49);
        let player_same = DVec3::new(255.49, 66.0, 255.49);
        let next_max = next_flee_position(npc_near_max, player_same);

        assert!(next_max.x <= PLATFORM_MAX_XZ, "X should clamp to max");
        assert!(next_max.z <= PLATFORM_MAX_XZ, "Z should clamp to max");

        let npc_near_min = DVec3::new(0.51, 66.0, 0.51);
        let player_far_positive = DVec3::new(10.0, 66.0, 10.0);
        let next_min = next_flee_position(npc_near_min, player_far_positive);

        assert!(next_min.x >= PLATFORM_MIN_XZ, "X should clamp to min");
        assert!(next_min.z >= PLATFORM_MIN_XZ, "Z should clamp to min");
        assert_eq!(next_min.y, NPC_FIXED_Y, "Y should stay fixed");
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
                },
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
}
