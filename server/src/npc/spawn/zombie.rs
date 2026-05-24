use bevy_transform::components::{GlobalTransform, Transform};
use big_brain::prelude::{FirstToScore, Thinker, ThinkerBuilder};
use valence::entity::zombie::ZombieEntityBundle;
use valence::prelude::{Commands, DVec3, Entity, EntityKind, EntityLayerId, Position};

use crate::npc::brain::{
    AgeingScorer, ChaseAction, ChaseTargetScorer, DashAction, DashScorer, MeleeAttackAction,
    MeleeRangeScorer, RetireAction,
};
use crate::npc::lifecycle::{npc_runtime_bundle, NpcArchetype};
use crate::npc::movement::{MovementController, MovementCooldowns};
use crate::npc::navigator::Navigator;
use crate::npc::patrol::NpcPatrol;
use crate::world::zone::DEFAULT_SPAWN_ZONE_NAME;

use super::common::{NpcBlackboard, NpcCombatLoadout, NpcMarker};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// 仅用于测试 —— 生产启动路径已移除 (plan-npc-overhaul-v1 §P1.4)。
#[allow(dead_code)]
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

/// 仅用于测试 —— 生产启动路径已移除 (plan-npc-overhaul-v1 §P1.4)。
#[allow(dead_code)]
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

// plan-npc-overhaul-v1 §P1.4 — spawn_single_zombie_npc_on_startup 和
// log_npc_marker_count 已移除。启动时不再自动生成僵尸 NPC。
