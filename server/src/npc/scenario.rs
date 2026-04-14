use bevy_transform::components::{GlobalTransform, Transform};
use big_brain::prelude::{FirstToScore, Thinker, ThinkerBuilder};
use valence::entity::zombie::ZombieEntityBundle;
use valence::prelude::{
    bevy_ecs, App, ChunkLayer, Commands, Component, DVec3, Despawned, Entity, EntityKind,
    EntityLayer, EntityLayerId, Position, Query, ResMut, Resource, Update, With,
};

use crate::npc::brain::{
    canonical_npc_id, ChaseAction, ChaseTargetScorer, DashAction, DashScorer, FleeAction,
    MeleeAttackAction, MeleeRangeScorer, PlayerProximityScorer, PROXIMITY_THRESHOLD,
};
use crate::npc::movement::{MovementCapabilities, MovementController, MovementCooldowns};
use crate::npc::navigator::Navigator;
use crate::npc::patrol::NpcPatrol;
use crate::npc::spawn::{DuelTarget, NpcBlackboard, NpcMarker};
use crate::world::zone::DEFAULT_SPAWN_ZONE_NAME;
use crate::{
    combat::components::{CombatState, DerivedAttrs, Lifecycle, Stamina, Wounds},
    cultivation::components::{Contamination, Cultivation, MeridianSystem},
};

/// Marker component for NPCs spawned by the `!npc_scenario` command.
/// Used for bulk cleanup on `!npc_scenario clear`.
#[derive(Clone, Copy, Debug, Component)]
pub struct ScenarioNpc;

/// Scenario types available via `!npc_scenario`.
#[derive(Clone, Copy, Debug)]
pub enum ScenarioType {
    /// NPC chases the nearest player.
    Chase,
    /// NPC flees from the nearest player (default brain).
    Flee,
    /// NPC chases then attacks in melee range.
    Fight,
    /// NPC maintains distance: flees when close, chases when far.
    Kite,
    /// 3 NPCs all chase + fight the player.
    Swarm,
    /// 2 NPCs fight each other for observation.
    Duel,
    /// Despawn all scenario NPCs.
    Clear,
}

impl ScenarioType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "chase" => Some(Self::Chase),
            "flee" => Some(Self::Flee),
            "fight" => Some(Self::Fight),
            "kite" => Some(Self::Kite),
            "swarm" => Some(Self::Swarm),
            "duel" => Some(Self::Duel),
            "clear" => Some(Self::Clear),
            _ => None,
        }
    }
}

/// Resource that queues a scenario spawn request from the chat command.
#[derive(Default)]
pub struct PendingScenario {
    pub request: Option<(ScenarioType, DVec3)>,
}

impl Resource for PendingScenario {}

pub fn register(app: &mut App) {
    app.insert_resource(PendingScenario::default())
        .add_systems(Update, process_pending_scenarios);
}

fn process_pending_scenarios(
    mut commands: Commands,
    mut pending: ResMut<PendingScenario>,
    layers: Query<Entity, (With<ChunkLayer>, With<EntityLayer>)>,
    scenario_npcs: Query<Entity, With<ScenarioNpc>>,
) {
    let Some((scenario, player_pos)) = pending.request.take() else {
        return;
    };

    let Ok(layer) = layers.get_single() else {
        tracing::warn!("[bong][npc] no layer found for scenario spawn");
        return;
    };

    // Always clear existing scenario NPCs first.
    for entity in &scenario_npcs {
        commands.entity(entity).insert(Despawned);
    }

    if matches!(scenario, ScenarioType::Clear) {
        tracing::info!("[bong][npc] cleared all scenario NPCs");
        return;
    }

    let spawn_count = match scenario {
        ScenarioType::Swarm => 4,
        ScenarioType::Duel => 2,
        _ => 1,
    };

    let mut spawned_entities = Vec::new();

    for i in 0..spawn_count {
        let offset = scenario_offset(i, spawn_count);
        let spawn_pos = player_pos + offset;

        let thinker = build_thinker(&scenario);

        let entity = commands
            .spawn((
                ZombieEntityBundle {
                    kind: EntityKind::ZOMBIE,
                    layer: EntityLayerId(layer),
                    position: Position::new([spawn_pos.x, spawn_pos.y, spawn_pos.z]),
                    ..Default::default()
                },
                Transform::from_xyz(spawn_pos.x as f32, spawn_pos.y as f32, spawn_pos.z as f32),
                GlobalTransform::default(),
                NpcMarker,
                NpcBlackboard::default(),
                Navigator::new(),
                MovementController::new(),
                scenario_capabilities(&scenario),
                MovementCooldowns::default(),
                NpcPatrol::new(
                    DEFAULT_SPAWN_ZONE_NAME,
                    DVec3::new(spawn_pos.x, spawn_pos.y, spawn_pos.z),
                ),
                ScenarioNpc,
                thinker,
            ))
            .id();

        commands.entity(entity).insert((
            Cultivation::default(),
            MeridianSystem::default(),
            Contamination::default(),
            Wounds::default(),
            Stamina::default(),
            CombatState::default(),
            DerivedAttrs::default(),
            Lifecycle {
                character_id: canonical_npc_id(entity),
                ..Default::default()
            },
        ));

        spawned_entities.push(entity);
    }

    // Cross-link duel targets so they fight each other instead of a player.
    if matches!(scenario, ScenarioType::Duel) && spawned_entities.len() == 2 {
        commands
            .entity(spawned_entities[0])
            .insert(DuelTarget(spawned_entities[1]));
        commands
            .entity(spawned_entities[1])
            .insert(DuelTarget(spawned_entities[0]));
    }

    tracing::info!("[bong][npc] spawned {spawn_count} scenario NPC(s) ({scenario:?}) near player");
}

/// Spread NPCs in a circle ~12 blocks from the player.
fn scenario_offset(index: usize, total: usize) -> DVec3 {
    let angle = std::f64::consts::TAU * (index as f64) / (total as f64);
    DVec3::new(angle.cos() * 12.0, 0.0, angle.sin() * 12.0)
}

fn build_thinker(scenario: &ScenarioType) -> ThinkerBuilder {
    match scenario {
        ScenarioType::Chase => Thinker::build()
            .picker(FirstToScore { threshold: 0.05 })
            .when(ChaseTargetScorer, ChaseAction),

        ScenarioType::Flee => Thinker::build()
            .picker(FirstToScore {
                threshold: PROXIMITY_THRESHOLD,
            })
            .when(PlayerProximityScorer, FleeAction),

        ScenarioType::Fight | ScenarioType::Swarm | ScenarioType::Duel => Thinker::build()
            .picker(FirstToScore { threshold: 0.05 })
            .when(MeleeRangeScorer, MeleeAttackAction)
            .when(DashScorer, DashAction)
            .when(ChaseTargetScorer, ChaseAction),

        ScenarioType::Kite => Thinker::build()
            .picker(FirstToScore { threshold: 0.05 })
            .when(PlayerProximityScorer, FleeAction)
            .when(ChaseTargetScorer, ChaseAction),

        ScenarioType::Clear => {
            // Clear is handled before we get here, but provide a default.
            Thinker::build()
                .picker(FirstToScore { threshold: 0.8 })
                .when(PlayerProximityScorer, FleeAction)
        }
    }
}

/// Movement capabilities per scenario type.
/// Fight/Swarm NPCs get dash; others only get sprint.
fn scenario_capabilities(scenario: &ScenarioType) -> MovementCapabilities {
    match scenario {
        ScenarioType::Fight | ScenarioType::Swarm | ScenarioType::Duel => MovementCapabilities {
            can_sprint: true,
            can_dash: true,
        },
        _ => MovementCapabilities::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::combat::components::{Lifecycle, Stamina, Wounds};
    use crate::cultivation::components::{Contamination, Cultivation, MeridianSystem};
    use valence::prelude::{Entity, Update, With};
    use valence::testing::ScenarioSingleClient;

    #[test]
    fn scenario_spawned_npcs_include_shared_combat_target_components() {
        let scenario = ScenarioSingleClient::new();
        let mut app = scenario.app;
        app.insert_resource(PendingScenario {
            request: Some((ScenarioType::Duel, DVec3::new(8.0, 66.0, 8.0))),
        });
        app.add_systems(Update, process_pending_scenarios);

        app.update();

        let scenario_npcs = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<ScenarioNpc>>();
            query.iter(world).collect::<Vec<_>>()
        };

        assert_eq!(
            scenario_npcs.len(),
            2,
            "duel scenario should spawn two NPCs for coverage"
        );

        for npc in scenario_npcs {
            let entity_ref = app.world().entity(npc);
            assert!(
                entity_ref.get::<Cultivation>().is_some(),
                "scenario NPC should include Cultivation for shared resolver"
            );
            assert!(
                entity_ref.get::<MeridianSystem>().is_some(),
                "scenario NPC should include MeridianSystem for shared resolver"
            );
            assert!(
                entity_ref.get::<Contamination>().is_some(),
                "scenario NPC should include Contamination for shared resolver"
            );
            assert!(
                entity_ref.get::<Wounds>().is_some(),
                "scenario NPC should include Wounds for shared resolver"
            );
            assert!(
                entity_ref.get::<Stamina>().is_some(),
                "scenario NPC should include Stamina for shared resolver"
            );
            let lifecycle = entity_ref
                .get::<Lifecycle>()
                .expect("scenario NPC should include Lifecycle identity component");
            assert_eq!(
                lifecycle.character_id,
                canonical_npc_id(npc),
                "scenario NPC Lifecycle should use canonical npc identity"
            );
        }
    }
}
