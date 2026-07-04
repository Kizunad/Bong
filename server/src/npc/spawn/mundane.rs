//! plan-mundane-fauna-v1 P0 — 凡兽 spawn：Rail A 原生 bundle + 被动 AI 三件套 +
//! `CorneredScorer` 反抗链。照 `zombie.rs:52`/`beast.rs:51-120` 范式组件清单穷举，
//! 不走 `beast.rs:64` 的 `MarkerEntityBundle` custom visual 路数。

use bevy_transform::components::{GlobalTransform, Transform};
use big_brain::prelude::{FirstToScore, Thinker, ThinkerBuilder};
use valence::entity::chicken::ChickenEntityBundle;
use valence::entity::cow::CowEntityBundle;
use valence::entity::fox::FoxEntityBundle;
use valence::entity::frog::FrogEntityBundle;
use valence::entity::goat::GoatEntityBundle;
use valence::entity::pig::PigEntityBundle;
use valence::entity::rabbit::RabbitEntityBundle;
use valence::entity::sheep::SheepEntityBundle;
use valence::entity::wolf::WolfEntityBundle;
use valence::prelude::{Commands, DVec3, Entity, EntityLayerId, Position};

use crate::fauna::mundane::{entity_kind_for_mundane, MundaneFaunaKind, MundaneFaunaSpecies};
use crate::npc::brain::{
    CorneredScorer, FarmAction, FleeAction, FleeThreatScorer, GoToPoiAction, GoToPoiState,
    HungerScorer, MeleeAttackAction, WanderScorer,
};
use crate::npc::hunger::Hunger;
use crate::npc::lifecycle::{npc_runtime_bundle_with_age, NpcArchetype};
use crate::npc::lod::NpcLodTier;
use crate::npc::movement::{MovementController, MovementCooldowns};
use crate::npc::navigator::Navigator;
use crate::npc::patrol::NpcPatrol;
use crate::npc::schedule::{home_base_for_archetype, NpcDailySchedule};

use super::common::{schedule_seed_for_entity, NpcBlackboard, NpcCombatLoadout, NpcMarker};

// ---------------------------------------------------------------------------
// Thinker — 4 分支威胁谱系反抗链（[[feedback_threat_spectrum]] 硬约束）
// ---------------------------------------------------------------------------

/// `FirstToScore` 按注册顺序取第一个过阈值的分支——`CorneredScorer` 必须排在
/// `FleeThreatScorer` 前，否则"被逼急了咬一口"永远轮不到（picker 顺序即优先级）。
pub(crate) fn mundane_fauna_thinker() -> ThinkerBuilder {
    Thinker::build()
        .picker(FirstToScore { threshold: 0.05 })
        .when(CorneredScorer, MeleeAttackAction)
        .when(FleeThreatScorer, FleeAction)
        .when(HungerScorer, FarmAction)
        .when(WanderScorer, GoToPoiAction::default())
}

// ---------------------------------------------------------------------------
// Spawn
// ---------------------------------------------------------------------------

/// Spawn a mundane fauna (凡兽) NPC. Rail A 原生 `<X>EntityBundle`——client 零改动，
/// vanilla renderer 免费渲染原版模型/贴图/音效（同 `spawn_zombie_npc_at` 先例）。
///
/// 位置 snap 到地表由**调用方**负责（`snap_spawn_y_to_surface`，同
/// `spawn_beast_npc_at`/`spawn_commoner_npc_at` 先例——spawn 函数本身不管地形查询）。
pub fn spawn_mundane_fauna_at(
    commands: &mut Commands,
    layer: Entity,
    home_zone: &str,
    spawn_position: DVec3,
    patrol_target: DVec3,
    kind: MundaneFaunaKind,
) -> Entity {
    let loadout = NpcCombatLoadout::civilian();
    let layer_id = EntityLayerId(layer);
    let position = Position::new([spawn_position.x, spawn_position.y, spawn_position.z]);
    let entity_kind = entity_kind_for_mundane(kind);

    let mut entity_commands = commands.spawn_empty();
    match kind {
        MundaneFaunaKind::Cow => {
            entity_commands.insert(CowEntityBundle {
                kind: entity_kind,
                layer: layer_id,
                position,
                ..Default::default()
            });
        }
        MundaneFaunaKind::Pig => {
            entity_commands.insert(PigEntityBundle {
                kind: entity_kind,
                layer: layer_id,
                position,
                ..Default::default()
            });
        }
        MundaneFaunaKind::Sheep => {
            entity_commands.insert(SheepEntityBundle {
                kind: entity_kind,
                layer: layer_id,
                position,
                ..Default::default()
            });
        }
        MundaneFaunaKind::Chicken => {
            entity_commands.insert(ChickenEntityBundle {
                kind: entity_kind,
                layer: layer_id,
                position,
                ..Default::default()
            });
        }
        MundaneFaunaKind::Rabbit => {
            entity_commands.insert(RabbitEntityBundle {
                kind: entity_kind,
                layer: layer_id,
                position,
                ..Default::default()
            });
        }
        MundaneFaunaKind::Goat => {
            entity_commands.insert(GoatEntityBundle {
                kind: entity_kind,
                layer: layer_id,
                position,
                ..Default::default()
            });
        }
        MundaneFaunaKind::Frog => {
            entity_commands.insert(FrogEntityBundle {
                kind: entity_kind,
                layer: layer_id,
                position,
                ..Default::default()
            });
        }
        MundaneFaunaKind::Fox => {
            entity_commands.insert(FoxEntityBundle {
                kind: entity_kind,
                layer: layer_id,
                position,
                ..Default::default()
            });
        }
        MundaneFaunaKind::Wolf => {
            entity_commands.insert(WolfEntityBundle {
                kind: entity_kind,
                layer: layer_id,
                position,
                ..Default::default()
            });
        }
    }

    let entity = entity_commands
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
            NpcArchetype::Mundane,
            MundaneFaunaSpecies(kind),
        ))
        .insert((
            Navigator::new(),
            MovementController::new(),
            loadout.movement_capabilities,
            MovementCooldowns::default(),
            NpcPatrol::new(home_zone, patrol_target),
        ))
        .id();

    commands.entity(entity).insert((
        NpcDailySchedule::for_archetype(NpcArchetype::Mundane, schedule_seed_for_entity(entity)),
        home_base_for_archetype(NpcArchetype::Mundane, patrol_target),
        GoToPoiState::default(),
        NpcLodTier::Dormant,
        Hunger::default(),
        mundane_fauna_thinker(),
    ));

    let mut runtime = npc_runtime_bundle_with_age(entity, NpcArchetype::Mundane, 0.0);
    let hp = kind.health_max();
    runtime.wounds.health_current = hp;
    runtime.wounds.health_max = hp;
    commands.entity(entity).insert(runtime);

    entity
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::components::{CombatState, Lifecycle, Wounds};
    use crate::npc::lifecycle::NpcLifespan;
    use crate::npc::movement::MovementCapabilities;
    use crate::world::zone::DEFAULT_SPAWN_ZONE_NAME;
    use valence::prelude::{App, EntityKind};

    fn make_app_with_layer() -> (App, Entity) {
        let mut app = App::new();
        let layer = app.world_mut().spawn_empty().id();
        (app, layer)
    }

    #[test]
    fn spawns_all_nine_kinds_with_matching_entity_kind() {
        let expected = [
            (MundaneFaunaKind::Cow, EntityKind::COW),
            (MundaneFaunaKind::Pig, EntityKind::PIG),
            (MundaneFaunaKind::Sheep, EntityKind::SHEEP),
            (MundaneFaunaKind::Chicken, EntityKind::CHICKEN),
            (MundaneFaunaKind::Rabbit, EntityKind::RABBIT),
            (MundaneFaunaKind::Goat, EntityKind::GOAT),
            (MundaneFaunaKind::Frog, EntityKind::FROG),
            (MundaneFaunaKind::Fox, EntityKind::FOX),
            (MundaneFaunaKind::Wolf, EntityKind::WOLF),
        ];
        for (kind, expected_entity_kind) in expected {
            let (mut app, layer) = make_app_with_layer();
            let entity = {
                let mut commands = app.world_mut().commands();
                spawn_mundane_fauna_at(
                    &mut commands,
                    layer,
                    DEFAULT_SPAWN_ZONE_NAME,
                    DVec3::new(10.0, 64.0, 10.0),
                    DVec3::new(10.0, 64.0, 10.0),
                    kind,
                )
            };
            app.world_mut().flush();
            assert_eq!(
                app.world().get::<EntityKind>(entity).copied(),
                Some(expected_entity_kind),
                "{kind:?} 应产出原生 {expected_entity_kind:?} 实体"
            );
        }
    }

    #[test]
    fn spawned_entity_carries_full_component_checklist() {
        // 组件清单 pin（plan §P0"通用底盘"穷举）：漏挂任一项会让对应 thinker 分支静默孤岛
        // （query 不满足只让 Action 恒 Failure，不 panic，肉眼从游戏内看不出来）。
        let (mut app, layer) = make_app_with_layer();
        let entity = {
            let mut commands = app.world_mut().commands();
            spawn_mundane_fauna_at(
                &mut commands,
                layer,
                DEFAULT_SPAWN_ZONE_NAME,
                DVec3::new(5.0, 64.0, 5.0),
                DVec3::new(5.0, 64.0, 5.0),
                MundaneFaunaKind::Cow,
            )
        };
        app.world_mut().flush();

        macro_rules! assert_has {
            ($ty:ty, $label:expr) => {
                assert!(
                    app.world().get::<$ty>(entity).is_some(),
                    "凡兽实体缺 {}，对应 thinker 分支会静默孤岛",
                    $label
                );
            };
        }

        assert_has!(NpcMarker, "NpcMarker");
        assert_has!(NpcBlackboard, "NpcBlackboard");
        assert_has!(Navigator, "Navigator");
        assert_has!(MovementController, "MovementController");
        assert_has!(MovementCapabilities, "MovementCapabilities");
        assert_has!(MovementCooldowns, "MovementCooldowns");
        assert_has!(NpcPatrol, "NpcPatrol");
        assert_has!(crate::npc::spawn::NpcMeleeProfile, "NpcMeleeProfile");
        assert_has!(NpcArchetype, "NpcArchetype");
        assert_has!(MundaneFaunaSpecies, "MundaneFaunaSpecies");
        assert_has!(Wounds, "Wounds");
        assert_has!(CombatState, "CombatState");
        assert_has!(Lifecycle, "Lifecycle");
        assert_has!(NpcLifespan, "NpcLifespan");
        assert_has!(GoToPoiState, "GoToPoiState");
        assert_has!(NpcLodTier, "NpcLodTier");
        assert_has!(Hunger, "Hunger");
        assert_has!(ThinkerBuilder, "ThinkerBuilder");

        let archetype = *app.world().get::<NpcArchetype>(entity).unwrap();
        assert_eq!(archetype, NpcArchetype::Mundane);

        let species = *app.world().get::<MundaneFaunaSpecies>(entity).unwrap();
        assert_eq!(species, MundaneFaunaSpecies(MundaneFaunaKind::Cow));
    }

    #[test]
    fn health_max_is_set_per_species_not_shared_default() {
        // 威胁谱系差异化：鸡与狼必须走各自的 health_max，不共享全局默认值。
        for (kind, expected_hp) in [
            (
                MundaneFaunaKind::Chicken,
                MundaneFaunaKind::Chicken.health_max(),
            ),
            (MundaneFaunaKind::Wolf, MundaneFaunaKind::Wolf.health_max()),
        ] {
            let (mut app, layer) = make_app_with_layer();
            let entity = {
                let mut commands = app.world_mut().commands();
                spawn_mundane_fauna_at(
                    &mut commands,
                    layer,
                    DEFAULT_SPAWN_ZONE_NAME,
                    DVec3::new(1.0, 64.0, 1.0),
                    DVec3::new(1.0, 64.0, 1.0),
                    kind,
                )
            };
            app.world_mut().flush();
            let wounds = app
                .world()
                .get::<Wounds>(entity)
                .expect("should have Wounds");
            assert_eq!(
                wounds.health_max, expected_hp,
                "{kind:?} health_max 应为 {expected_hp}，实际 {}",
                wounds.health_max
            );
            assert_eq!(
                wounds.health_current, expected_hp,
                "{kind:?} health_current 应从满血起步"
            );
        }
        // 鸡 < 狼 的差异化断言（防止两者意外相等仍通过上面的独立断言）。
        assert!(MundaneFaunaKind::Chicken.health_max() < MundaneFaunaKind::Wolf.health_max());
    }

    #[test]
    fn spawn_position_is_faithfully_applied() {
        // spawn 落地：Position 组件必须等于传入坐标（吸附由调用方 snap_spawn_y_to_surface
        // 负责，此处验证 spawn 函数忠实使用传入的（可能已被调用方 snap 过的）坐标，不做
        // 自己的二次修改）。
        let (mut app, layer) = make_app_with_layer();
        let target = DVec3::new(12.5, 70.0, -8.25);
        let entity = {
            let mut commands = app.world_mut().commands();
            spawn_mundane_fauna_at(
                &mut commands,
                layer,
                DEFAULT_SPAWN_ZONE_NAME,
                target,
                target,
                MundaneFaunaKind::Frog,
            )
        };
        app.world_mut().flush();
        let position = app
            .world()
            .get::<Position>(entity)
            .expect("should have Position");
        assert!((position.get() - target).length() < 1e-9);
    }
}
