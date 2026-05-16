//! 毒龙 / 骨龙 —— 化虚级天空实体，spawn 路径与运行时组件。
//! 仿 spawn_whale 模式：高空飞行巡游，不主动攻击地面目标。

use bevy_transform::components::{GlobalTransform, Transform};
use big_brain::prelude::{FirstToScore, Thinker, ThinkerBuilder};
use valence::entity::entity::NoGravity;
use valence::entity::marker::MarkerEntityBundle;
use valence::prelude::{Commands, DVec3, Entity, EntityLayerId, Look, Position};

use crate::fauna::components::{BeastKind, FaunaTag};
use crate::fauna::visual::entity_kind_for_beast;
use crate::npc::brain_whale::{WhaleDriftAction, WhaleDriftScorer};
use crate::npc::lifecycle::{npc_runtime_bundle, NpcArchetype};
use crate::npc::lod::NpcLodTier;
use crate::npc::spawn::{NpcBlackboard, NpcMarker};
use crate::npc::spawn_whale::{WhaleBlackboard, WhaleFlightController};

#[allow(dead_code)]
const DRAGON_THINKER_THRESHOLD: f32 = 0.01;
pub const DRAGON_WANDER_RADIUS_XZ: f64 = 128.0;
pub const DRAGON_CRUISE_SPEED: f64 = 0.25;
pub const DRAGON_Y_OSCILLATION_AMPLITUDE: f64 = 2.0;
pub const DRAGON_LIFESPAN_MAX_TICKS: f64 = 7_200_000.0;

#[allow(dead_code)]
fn dragon_npc_thinker() -> ThinkerBuilder {
    Thinker::build()
        .picker(FirstToScore {
            threshold: DRAGON_THINKER_THRESHOLD,
        })
        .when(WhaleDriftScorer, WhaleDriftAction)
}

pub fn spawn_dragon_npc_at(
    commands: &mut Commands,
    layer: Entity,
    home_position: DVec3,
    kind: BeastKind,
) -> Entity {
    let entity = commands
        .spawn(MarkerEntityBundle {
            kind: entity_kind_for_beast(kind),
            layer: EntityLayerId(layer),
            position: Position::new([home_position.x, home_position.y, home_position.z]),
            entity_no_gravity: NoGravity(true),
            look: Look::new(0.0, 0.0),
            ..Default::default()
        })
        .insert((
            Transform::from_xyz(
                home_position.x as f32,
                home_position.y as f32,
                home_position.z as f32,
            ),
            GlobalTransform::default(),
            NpcMarker,
            NpcBlackboard::default(),
            NpcArchetype::Beast,
            FaunaTag::new(kind),
            NpcLodTier::Dormant,
            WhaleBlackboard::new(home_position, DRAGON_WANDER_RADIUS_XZ),
            WhaleFlightController {
                baseline_y: home_position.y,
                cruise_speed: DRAGON_CRUISE_SPEED,
                y_oscillation_amplitude: DRAGON_Y_OSCILLATION_AMPLITUDE,
                ..Default::default()
            },
        ))
        .id();

    if let Some(visual) = crate::fauna::visual::visual_kind_for_beast(kind) {
        commands.entity(entity).insert(visual);
    }

    let mut runtime = npc_runtime_bundle(entity, NpcArchetype::Beast);
    let hp = kind.health_max();
    runtime.wounds.health_current = hp;
    runtime.wounds.health_max = hp;
    runtime.lifespan.max_age_ticks = DRAGON_LIFESPAN_MAX_TICKS;
    commands
        .entity(entity)
        .insert((dragon_npc_thinker(), runtime));

    entity
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::testing::ScenarioSingleClient;

    #[test]
    fn spawn_poison_dragon_attaches_fauna_tag_and_flight_controller() {
        let scenario = ScenarioSingleClient::new();
        let layer = scenario.layer;
        let mut app = scenario.app;
        let dragon = spawn_dragon_npc_at(
            &mut app.world_mut().commands(),
            layer,
            DVec3::new(0.0, 140.0, 0.0),
            BeastKind::PoisonDragon,
        );
        app.world_mut().flush();

        assert_eq!(
            app.world().get::<FaunaTag>(dragon).map(|t| t.beast_kind),
            Some(BeastKind::PoisonDragon)
        );
        let wounds = app
            .world()
            .get::<crate::combat::components::Wounds>(dragon)
            .expect("dragon should have wounds");
        assert_eq!(wounds.health_max, BeastKind::PoisonDragon.health_max());
        assert!(app.world().get::<WhaleFlightController>(dragon).is_some());
    }

    #[test]
    fn spawn_bone_dragon_uses_correct_entity_kind() {
        let scenario = ScenarioSingleClient::new();
        let layer = scenario.layer;
        let mut app = scenario.app;
        let dragon = spawn_dragon_npc_at(
            &mut app.world_mut().commands(),
            layer,
            DVec3::new(100.0, 150.0, 100.0),
            BeastKind::BoneDragon,
        );
        app.world_mut().flush();

        assert_eq!(
            app.world().get::<FaunaTag>(dragon).map(|t| t.beast_kind),
            Some(BeastKind::BoneDragon)
        );
        assert_eq!(
            app.world()
                .get::<valence::prelude::EntityKind>(dragon)
                .copied(),
            Some(entity_kind_for_beast(BeastKind::BoneDragon))
        );
    }
}
