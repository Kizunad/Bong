//! 邃渊柱 —— 通灵+ 级克苏鲁式游荡实体，spawn 路径与运行时组件。
//! 极缓慢移动（0.3m/s），触手攻击范围 8 格 + 真元 drain aura。

use bevy_transform::components::{GlobalTransform, Transform};
use big_brain::prelude::{FirstToScore, Thinker, ThinkerBuilder};
use valence::entity::marker::MarkerEntityBundle;
use valence::prelude::{bevy_ecs, Commands, Component, DVec3, Entity, EntityLayerId, Position};

use crate::fauna::components::{BeastKind, FaunaTag};
use crate::fauna::visual::{entity_kind_for_beast, visual_kind_for_beast};
use crate::npc::brain::{ChaseAction, ChaseTargetScorer, MeleeAttackAction, MeleeRangeScorer, WanderAction, WanderScorer, WanderState};
use crate::npc::lifecycle::{npc_runtime_bundle, NpcArchetype};
use crate::npc::lod::NpcLodTier;
use crate::npc::movement::{MovementController, MovementCooldowns};
use crate::npc::navigator::Navigator;
use crate::npc::spawn::{NpcBlackboard, NpcCombatLoadout, NpcMarker, NpcMeleeArchetype};

#[allow(dead_code)]
const PILLAR_THINKER_THRESHOLD: f32 = 0.05;
pub const PILLAR_LIFESPAN_MAX_TICKS: f64 = 7_200_000.0;

#[derive(Debug, Clone, Copy, Component)]
#[allow(dead_code)]
pub struct PillarDrainAura {
    pub radius_blocks: f32,
    pub drain_boost_multiplier: f64,
}

impl Default for PillarDrainAura {
    fn default() -> Self {
        Self {
            radius_blocks: 8.0,
            drain_boost_multiplier: 2.0,
        }
    }
}

#[allow(dead_code)]
fn pillar_npc_thinker() -> ThinkerBuilder {
    Thinker::build()
        .picker(FirstToScore {
            threshold: PILLAR_THINKER_THRESHOLD,
        })
        .when(MeleeRangeScorer, MeleeAttackAction)
        .when(ChaseTargetScorer, ChaseAction)
        .when(WanderScorer, WanderAction)
}

pub fn spawn_pillar_npc_at(
    commands: &mut Commands,
    layer: Entity,
    spawn_position: DVec3,
) -> Entity {
    let loadout = NpcCombatLoadout::fighter(NpcMeleeArchetype::Brawler);
    let entity = commands
        .spawn(MarkerEntityBundle {
            kind: entity_kind_for_beast(BeastKind::LivingPillar),
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
            FaunaTag::new(BeastKind::LivingPillar),
            NpcLodTier::Dormant,
            PillarDrainAura::default(),
        ))
        .insert((
            Navigator::new(),
            MovementController::new(),
            loadout.movement_capabilities,
            MovementCooldowns::default(),
            WanderState::default(),
        ))
        .id();

    if let Some(visual) = visual_kind_for_beast(BeastKind::LivingPillar) {
        commands.entity(entity).insert(visual);
    }

    let mut runtime = npc_runtime_bundle(entity, NpcArchetype::Beast);
    let hp = BeastKind::LivingPillar.health_max();
    runtime.wounds.health_current = hp;
    runtime.wounds.health_max = hp;
    runtime.lifespan.max_age_ticks = PILLAR_LIFESPAN_MAX_TICKS;
    commands
        .entity(entity)
        .insert((pillar_npc_thinker(), runtime));

    entity
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::testing::ScenarioSingleClient;

    #[test]
    fn spawn_pillar_attaches_fauna_tag_and_drain_aura() {
        let scenario = ScenarioSingleClient::new();
        let layer = scenario.layer;
        let mut app = scenario.app;
        let pillar = spawn_pillar_npc_at(
            &mut app.world_mut().commands(),
            layer,
            DVec3::new(0.0, 64.0, 0.0),
        );
        app.world_mut().flush();

        assert_eq!(
            app.world().get::<FaunaTag>(pillar).map(|t| t.beast_kind),
            Some(BeastKind::LivingPillar)
        );
        let wounds = app
            .world()
            .get::<crate::combat::components::Wounds>(pillar)
            .expect("pillar should have wounds");
        assert_eq!(wounds.health_max, BeastKind::LivingPillar.health_max());
        let aura = app
            .world()
            .get::<PillarDrainAura>(pillar)
            .expect("pillar should have drain aura");
        assert_eq!(aura.radius_blocks, 8.0);
        assert_eq!(aura.drain_boost_multiplier, 2.0);
    }
}
