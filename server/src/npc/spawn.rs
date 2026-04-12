use bevy_transform::components::{GlobalTransform, Transform};
use big_brain::prelude::{FirstToScore, Thinker};
use valence::entity::zombie::ZombieEntityBundle;
use valence::prelude::{
    bevy_ecs, App, ChunkLayer, Commands, Component, DVec3, Entity, EntityKind, EntityLayer,
    EntityLayerId, IntoSystemConfigs, Position, PostStartup, Query, With,
};

use crate::npc::brain::{FleeAction, PlayerProximityScorer, PROXIMITY_THRESHOLD};
use crate::npc::movement::{MovementCapabilities, MovementController, MovementCooldowns};
use crate::npc::navigator::Navigator;
use crate::npc::patrol::NpcPatrol;
use crate::world::zone::DEFAULT_SPAWN_ZONE_NAME;

const NPC_SPAWN_POSITION: [f64; 3] = [14.0, 66.0, 14.0];

#[derive(Clone, Copy, Debug, Default, Component)]
pub struct NpcMarker;

#[derive(Clone, Copy, Debug, Component)]
#[allow(dead_code, unfulfilled_lint_expectations)]
pub struct NpcBlackboard {
    pub nearest_player: Option<Entity>,
    pub player_distance: f32,
    /// Cached world position of the current target (player or duel opponent).
    pub target_position: Option<DVec3>,
    /// GameTick of the last melee attack (for cooldown tracking).
    pub last_melee_tick: u32,
}

impl Default for NpcBlackboard {
    fn default() -> Self {
        Self {
            nearest_player: None,
            player_distance: f32::INFINITY,
            target_position: None,
            last_melee_tick: 0,
        }
    }
}

/// Override target for NPC-vs-NPC scenarios (e.g. duel).
/// When present, the NPC targets this entity instead of the nearest player.
#[derive(Clone, Copy, Debug, Component)]
pub struct DuelTarget(pub Entity);

pub fn register(app: &mut App) {
    tracing::info!("[bong][npc] registering startup spawn systems");
    app.add_systems(
        PostStartup,
        (
            spawn_single_zombie_npc_on_startup,
            log_npc_marker_count.after(spawn_single_zombie_npc_on_startup),
        ),
    );
}

fn spawn_single_zombie_npc_on_startup(
    mut commands: Commands,
    layers: Query<Entity, (With<ChunkLayer>, With<EntityLayer>)>,
) {
    let layer = layers.single();
    let npc_entity = spawn_single_zombie_npc(&mut commands, layer);

    tracing::info!(
        "[bong][npc] spawned zombie npc entity {npc_entity:?} at [{}, {}, {}]",
        NPC_SPAWN_POSITION[0],
        NPC_SPAWN_POSITION[1],
        NPC_SPAWN_POSITION[2]
    );
}

fn spawn_single_zombie_npc(commands: &mut Commands, layer: Entity) -> Entity {
    commands
        .spawn((
            ZombieEntityBundle {
                kind: EntityKind::ZOMBIE,
                layer: EntityLayerId(layer),
                position: Position::new(NPC_SPAWN_POSITION),
                ..Default::default()
            },
            Transform::from_xyz(
                NPC_SPAWN_POSITION[0] as f32,
                NPC_SPAWN_POSITION[1] as f32,
                NPC_SPAWN_POSITION[2] as f32,
            ),
            GlobalTransform::default(),
            NpcMarker,
            NpcBlackboard::default(),
            Navigator::new(),
            MovementController::new(),
            MovementCapabilities::default(),
            MovementCooldowns::default(),
            NpcPatrol::new(
                DEFAULT_SPAWN_ZONE_NAME,
                DVec3::new(
                    NPC_SPAWN_POSITION[0],
                    NPC_SPAWN_POSITION[1],
                    NPC_SPAWN_POSITION[2],
                ),
            ),
            Thinker::build()
                .picker(FirstToScore {
                    threshold: PROXIMITY_THRESHOLD,
                })
                .when(PlayerProximityScorer, FleeAction),
        ))
        .id()
}

fn log_npc_marker_count(query: Query<Entity, With<NpcMarker>>) {
    tracing::info!(
        "[bong][npc] startup marker count with NpcMarker: {}",
        query.iter().count()
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use big_brain::prelude::{BigBrainPlugin, HasThinker, ThinkerBuilder};
    use valence::prelude::{
        bevy_ecs, App, Commands, DVec3, Entity, EntityKind, EntityLayerId, PreUpdate, Res, Resource,
    };

    #[derive(Clone, Copy, Resource)]
    struct TestLayer(Entity);

    fn setup_test_layer(mut commands: Commands) {
        let layer = commands.spawn_empty().id();
        commands.insert_resource(TestLayer(layer));
    }

    fn spawn_test_npc(mut commands: Commands, layer: Res<TestLayer>) {
        spawn_single_zombie_npc(&mut commands, layer.0);
    }

    #[test]
    fn spawn_npc_creates_single_zombie_with_expected_components() {
        let mut app = App::new();
        app.add_plugins(BigBrainPlugin::new(PreUpdate));
        app.add_systems(
            valence::prelude::Startup,
            (setup_test_layer, spawn_test_npc.after(setup_test_layer)),
        );

        app.update();
        app.update();

        let npc_entities = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
            query.iter(world).collect::<Vec<_>>()
        };

        assert_eq!(
            npc_entities.len(),
            1,
            "expected exactly one NPC marker entity"
        );

        let npc_entity = npc_entities[0];

        let kind = app
            .world()
            .get::<EntityKind>(npc_entity)
            .expect("NPC should have EntityKind component");
        assert_eq!(*kind, EntityKind::ZOMBIE);

        let position = app
            .world()
            .get::<Position>(npc_entity)
            .expect("NPC should have Position component");
        assert_eq!(position.get(), DVec3::new(14.0, 66.0, 14.0));

        let transform = app
            .world()
            .get::<Transform>(npc_entity)
            .expect("NPC should have Transform component");
        assert_eq!(transform.translation.x, 14.0);
        assert_eq!(transform.translation.y, 66.0);
        assert_eq!(transform.translation.z, 14.0);

        let _global_transform = app
            .world()
            .get::<GlobalTransform>(npc_entity)
            .expect("NPC should have GlobalTransform component");

        let blackboard = app
            .world()
            .get::<NpcBlackboard>(npc_entity)
            .expect("NPC should have NpcBlackboard component");
        assert_eq!(blackboard.nearest_player, None);
        assert!(
            blackboard.player_distance.is_infinite(),
            "NpcBlackboard.player_distance should default to infinity"
        );

        let patrol = app
            .world()
            .get::<NpcPatrol>(npc_entity)
            .expect("NPC should have a patrol component");
        assert_eq!(patrol.home_zone, DEFAULT_SPAWN_ZONE_NAME);
        assert_eq!(patrol.current_target, DVec3::new(14.0, 66.0, 14.0));

        let layer_id = app
            .world()
            .get::<EntityLayerId>(npc_entity)
            .expect("NPC should have EntityLayerId component");
        assert_ne!(
            layer_id.0,
            Entity::PLACEHOLDER,
            "NPC should be assigned to a non-placeholder layer"
        );

        let _thinker_builder = app
            .world()
            .get::<ThinkerBuilder>(npc_entity)
            .expect("NPC should have a Thinker builder attached at spawn time");

        let has_thinker = app
            .world()
            .get::<HasThinker>(npc_entity)
            .expect("BigBrain should attach HasThinker to NPC");

        let _thinker = app
            .world()
            .get::<Thinker>(has_thinker.entity())
            .expect("BigBrain thinker entity should contain Thinker component");
    }
}
