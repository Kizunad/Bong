use bevy_transform::components::{GlobalTransform, Transform};
use big_brain::prelude::{FirstToScore, Thinker, ThinkerBuilder};
use valence::entity::zombie::ZombieEntityBundle;
use valence::prelude::{
    Commands, DVec3, Entity, EntityKind, EntityLayerId, EventWriter, Position, Query, Res, With,
};

use crate::npc::brain::{
    AgeingScorer, ChaseAction, ChaseTargetScorer, DashAction, DashScorer, MeleeAttackAction,
    MeleeRangeScorer, RetireAction,
};
use crate::npc::lifecycle::{npc_runtime_bundle, NpcArchetype, NpcSpawnNotice, NpcSpawnSource};
use crate::npc::movement::{MovementController, MovementCooldowns};
use crate::npc::navigator::Navigator;
use crate::npc::patrol::NpcPatrol;
use crate::world::zone::DEFAULT_SPAWN_ZONE_NAME;

use super::common::{spawn_notice, NpcBlackboard, NpcCombatLoadout, NpcMarker};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

pub(crate) const NPC_SPAWN_POSITION: [f64; 3] = [14.0, 66.0, 14.0];

// ---------------------------------------------------------------------------
// Thinker
// ---------------------------------------------------------------------------

pub(crate) fn startup_npc_thinker() -> ThinkerBuilder {
    Thinker::build()
        .picker(FirstToScore { threshold: 0.05 })
        .when(AgeingScorer, RetireAction)
        .when(MeleeRangeScorer, MeleeAttackAction)
        .when(DashScorer, DashAction)
        .when(ChaseTargetScorer, ChaseAction)
}

// ---------------------------------------------------------------------------
// Spawn
// ---------------------------------------------------------------------------

pub fn spawn_zombie_npc_at(
    commands: &mut Commands,
    layer: Entity,
    home_zone: &str,
    spawn_position: DVec3,
    patrol_target: DVec3,
) -> Entity {
    let entity = commands
        .spawn((
            ZombieEntityBundle {
                kind: EntityKind::ZOMBIE,
                layer: EntityLayerId(layer),
                position: Position::new([spawn_position.x, spawn_position.y, spawn_position.z]),
                ..Default::default()
            },
            Transform::from_xyz(
                spawn_position.x as f32,
                spawn_position.y as f32,
                spawn_position.z as f32,
            ),
            GlobalTransform::default(),
            NpcMarker,
            NpcBlackboard::default(),
            NpcCombatLoadout::default(),
            NpcCombatLoadout::default().melee_archetype,
            NpcCombatLoadout::default().melee_profile(),
            NpcArchetype::Zombie,
            Navigator::new(),
            MovementController::new(),
            NpcCombatLoadout::default().movement_capabilities,
            MovementCooldowns::default(),
            NpcPatrol::new(home_zone, patrol_target),
            startup_npc_thinker(),
        ))
        .id();

    commands
        .entity(entity)
        .insert(npc_runtime_bundle(entity, NpcArchetype::Zombie));

    entity
}

pub(crate) fn spawn_single_zombie_npc(commands: &mut Commands, layer: Entity) -> Entity {
    spawn_zombie_npc_at(
        commands,
        layer,
        DEFAULT_SPAWN_ZONE_NAME,
        DVec3::new(
            NPC_SPAWN_POSITION[0],
            NPC_SPAWN_POSITION[1],
            NPC_SPAWN_POSITION[2],
        ),
        DVec3::new(
            NPC_SPAWN_POSITION[0],
            NPC_SPAWN_POSITION[1],
            NPC_SPAWN_POSITION[2],
        ),
    )
}

pub(crate) fn spawn_single_zombie_npc_on_startup(
    mut commands: Commands,
    dimension_layers: Option<Res<crate::world::dimension::DimensionLayers>>,
    mut notices: EventWriter<NpcSpawnNotice>,
) {
    let Some(dimension_layers) = dimension_layers else {
        return;
    };
    let layer = dimension_layers.overworld;
    let npc_entity = spawn_single_zombie_npc(&mut commands, layer);
    notices.send(spawn_notice(
        npc_entity,
        NpcArchetype::Zombie,
        NpcSpawnSource::Startup,
        DEFAULT_SPAWN_ZONE_NAME,
        DVec3::new(
            NPC_SPAWN_POSITION[0],
            NPC_SPAWN_POSITION[1],
            NPC_SPAWN_POSITION[2],
        ),
        0.0,
    ));

    tracing::info!(
        "[bong][npc] spawned zombie npc entity {npc_entity:?} at [{}, {}, {}]",
        NPC_SPAWN_POSITION[0],
        NPC_SPAWN_POSITION[1],
        NPC_SPAWN_POSITION[2]
    );
}

pub(crate) fn log_npc_marker_count(query: Query<Entity, With<NpcMarker>>) {
    tracing::info!(
        "[bong][npc] startup marker count with NpcMarker: {}",
        query.iter().count()
    );
}
