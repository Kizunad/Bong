use bevy_transform::components::{GlobalTransform, Transform};
use big_brain::prelude::{FirstToScore, Thinker, ThinkerBuilder};
use valence::entity::marker::MarkerEntityBundle;
use valence::prelude::{Commands, DVec3, Entity, EntityLayerId, Position};

use crate::fauna::components::{fauna_spawn_seed, fauna_tag_for_beast_spawn};
use crate::fauna::visual::{entity_kind_for_beast, visual_kind_for_beast};
use crate::npc::brain::{
    AgeingScorer, ChaseAction, ChaseTargetScorer, GoToPoiState, MeleeAttackAction,
    MeleeRangeScorer, RestState, RetireAction, StallState, WanderAction, WanderScorer, WanderState,
};
use crate::npc::hunger::Hunger;
use crate::npc::lifecycle::{npc_runtime_bundle_with_age, NpcArchetype};
use crate::npc::lod::NpcLodTier;
use crate::npc::movement::{MovementController, MovementCooldowns};
use crate::npc::navigator::Navigator;
use crate::npc::patrol::NpcPatrol;
use crate::npc::schedule::{home_base_for_archetype, NpcDailySchedule};
use crate::npc::territory::{
    HuntAction, HuntState, ProtectYoungAction, ProtectYoungScorer, ProtectYoungState, Territory,
    TerritoryIntruderScorer, TerritoryPatrolAction, TerritoryPatrolState,
};

use super::common::{
    schedule_seed_for_entity, NpcBlackboard, NpcCombatLoadout, NpcMarker, NpcMeleeArchetype,
};

// ---------------------------------------------------------------------------
// Thinker
// ---------------------------------------------------------------------------

pub(crate) fn beast_npc_thinker() -> ThinkerBuilder {
    Thinker::build()
        .picker(FirstToScore { threshold: 0.05 })
        .when(AgeingScorer, RetireAction)
        .when(ProtectYoungScorer, ProtectYoungAction)
        .when(TerritoryIntruderScorer, HuntAction)
        .when(MeleeRangeScorer, MeleeAttackAction)
        .when(ChaseTargetScorer, ChaseAction)
        .when(WanderScorer, TerritoryPatrolAction)
        .when(WanderScorer, WanderAction)
}

// ---------------------------------------------------------------------------
// Spawn
// ---------------------------------------------------------------------------

/// Spawn a Beast (妖兽) NPC. 视觉 shell 走 fauna custom EntityKind，由 client GeckoLib renderer 区分种类。
/// `territory` 决定领地中心 + 半径；容量由 `Territory::capacity()` 派生。
/// `initial_age_ticks` 控制年龄（繁衍出来的幼崽传 0.0）。
pub fn spawn_beast_npc_at(
    commands: &mut Commands,
    layer: Entity,
    home_zone: &str,
    spawn_position: DVec3,
    territory: Territory,
    initial_age_ticks: f64,
) -> Entity {
    let loadout = NpcCombatLoadout::fighter(NpcMeleeArchetype::Brawler);
    let fauna_seed = fauna_spawn_seed(home_zone, spawn_position.x, spawn_position.z);
    let fauna_tag = fauna_tag_for_beast_spawn(home_zone, fauna_seed);
    let visual_kind = visual_kind_for_beast(fauna_tag.beast_kind);
    let entity = commands
        .spawn(MarkerEntityBundle {
            kind: entity_kind_for_beast(fauna_tag.beast_kind),
            layer: EntityLayerId(layer),
            position: Position::new([spawn_position.x, spawn_position.y, spawn_position.z]),
            ..Default::default()
        })
        .insert((
            Transform::from_xyz(
                spawn_position.x as f32,
                spawn_position.y as f32,
                spawn_position.z as f32,
            ),
            GlobalTransform::default(),
            NpcMarker,
            NpcBlackboard::default(),
            loadout.clone(),
            loadout.melee_archetype,
            loadout.melee_profile(),
            NpcArchetype::Beast,
            fauna_tag,
        ))
        .insert((
            Navigator::new(),
            MovementController::new(),
            loadout.movement_capabilities,
            MovementCooldowns::default(),
            NpcPatrol::new(home_zone, territory.center),
        ))
        .id();

    commands.entity(entity).insert((
        NpcDailySchedule::for_archetype(NpcArchetype::Beast, schedule_seed_for_entity(entity)),
        home_base_for_archetype(NpcArchetype::Beast, territory.center),
        GoToPoiState::default(),
        StallState::default(),
        RestState::default(),
        NpcLodTier::Dormant,
        Hunger::default(),
        WanderState::default(),
        territory,
        TerritoryPatrolState::default(),
        HuntState::default(),
        ProtectYoungState::default(),
        beast_npc_thinker(),
    ));

    let mut runtime = npc_runtime_bundle_with_age(entity, NpcArchetype::Beast, initial_age_ticks);
    let hp = fauna_tag.beast_kind.health_max();
    runtime.wounds.health_current = hp;
    runtime.wounds.health_max = hp;
    commands.entity(entity).insert(runtime);
    if let Some(visual_kind) = visual_kind {
        commands.entity(entity).insert(visual_kind);
    }

    entity
}
