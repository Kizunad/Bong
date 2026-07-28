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

use crate::cultivation::components::Realm;
use crate::fauna::mundane::{entity_kind_for_mundane, MundaneFaunaKind, MundaneFaunaSpecies};
use crate::npc::brain::{
    CorneredScorer, FarmAction, FleeAction, FleeThreatScorer, GoToPoiAction, GoToPoiState,
    HungerScorer, MeleeAttackAction, MundaneHuntAction, MundaneHuntScorer, MundaneHuntState,
    WanderScorer,
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
///
/// **P2 追加**：`kind.is_predator()`（狐/狼）额外挂 `MundaneHuntScorer → MundaneHuntAction`
/// 分支，排在 `CorneredScorer` 之后、`FleeThreatScorer` 之前——预掠者发现猎物时优先追猎，
/// 不因附近有玩家/修士就放弃（T2/T2.5"掠食骚扰/群体掠食"语义：狼狐不是易惊的猎物动物）。
/// **v1 范围限定**（诚实记录，非阉割）：本 thinker 不含"主动猎杀低境界玩家"
/// （T2.5 完整语义的一部分）与"群体协同（≥3 成型）"——两者都需要区分"威胁感知"与
/// "猎物感知"两套独立信号 + 群体计数基建，超出本次 P2 实现预算，留作后续 tuning 向小 PR；
/// 本次交付的是可测、可验证的"凡兽内部食物链"核心（狼/狐猎 T0/T1/鼠）。
pub(crate) fn mundane_fauna_thinker(kind: MundaneFaunaKind) -> ThinkerBuilder {
    let mut builder = Thinker::build()
        .picker(FirstToScore { threshold: 0.05 })
        .when(CorneredScorer, MeleeAttackAction);
    if kind.is_predator() {
        builder = builder.when(MundaneHuntScorer, MundaneHuntAction);
    }
    builder
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
        mundane_fauna_thinker(kind),
    ));

    // P2 — 狼/狐掠食者专属运行态（`MundaneHuntAction` 硬性读 `&mut MundaneHuntState`，
    // 非 predator 不挂——thinker 本就不含 Hunt 分支，挂了也是死组件，不挂更干净）。
    if kind.is_predator() {
        commands.entity(entity).insert(MundaneHuntState::default());
    }

    // 凡兽无灵、不修炼——`Realm::Awaken`（醒灵，境界地板）作为惰性占位值传入。
    // 共享的 `NpcRuntimeBundle` 自 #857 起强制携带 `Realm`，但凡兽从不参与修炼系统，
    // 此值只为满足类型约束，与旧 `Cultivation::default().realm`（同为 Awaken）行为一致。
    let mut runtime =
        npc_runtime_bundle_with_age(entity, NpcArchetype::Mundane, Realm::Awaken, 0.0);
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

    // -----------------------------------------------------------------
    // 4-thinker 分支真实触发 pin（plan §P0 测试第 4 条——只写"复用 Action"不算数，
    // 必须验证 Scorer 真的能把 picker 带到对应 Action）。全程走真实
    // `crate::npc::brain::register` 管线（BigBrainPlugin + update_npc_blackboard +
    // 全部 scorer/action 系统），不是手搓 Score 断言。
    // -----------------------------------------------------------------

    use big_brain::prelude::Actor;
    use valence::client::ClientMarker;
    use valence::prelude::DVec3;

    fn mundane_thinker_scenario() -> (App, Entity) {
        mundane_thinker_scenario_with_kind(MundaneFaunaKind::Rabbit)
    }

    fn mundane_thinker_scenario_with_kind(kind: MundaneFaunaKind) -> (App, Entity) {
        let mut app = App::new();
        app.insert_resource(crate::qi_physics::WorldQiAccount::default());
        app.add_event::<crate::qi_physics::QiTransfer>();
        app.insert_resource(
            crate::cultivation::known_techniques::TechniqueRegistry::load_for_tests(),
        );
        crate::npc::lifecycle::register(&mut app);
        crate::npc::brain::register(&mut app);
        app.add_event::<crate::combat::events::AttackIntent>();
        app.insert_resource(crate::npc::movement::GameTick(0));
        let layer = app.world_mut().spawn_empty().id();
        let actor = {
            let mut commands = app.world_mut().commands();
            spawn_mundane_fauna_at(
                &mut commands,
                layer,
                DEFAULT_SPAWN_ZONE_NAME,
                DVec3::new(0.0, 64.0, 0.0),
                DVec3::new(0.0, 64.0, 0.0),
                kind,
            )
        };
        app.world_mut().flush();
        // 生产 spawn 恒以 NpcLodTier::Dormant 起步（同 beast.rs/commoner.rs 先例，由独立
        // LOD 距离系统事后唤醒）——`lod_gated_score` 对 Dormant 恒强制 Score=0.0
        // （`is_dormant` 分支），不推进到 Near 会让 Hunger/WanderScorer 永远算不出真实值，
        // 与本测试组"验证 thinker 分支接线"的目标无关（LOD 距离唤醒逻辑已在
        // `npc::lod` 自己的测试套件里覆盖）。直接推到 Near，隔离出 thinker 本身的行为。
        *app.world_mut()
            .get_mut::<crate::npc::lod::NpcLodTier>(actor)
            .expect("should have NpcLodTier") = crate::npc::lod::NpcLodTier::Near;
        (app, actor)
    }

    /// 推进 `GameTick`（movement/patrol 冷却读取）与 `CultivationClock`
    /// （`npc/brain/scorers_survival.rs` 里 `wander_scorer_system`/`hunger_scorer_system`
    /// 真正读的 `clock_tick`——**不是** `GameTick`：`crate::npc::brain::register` 从不插入
    /// 会自增的 `CultivationClock` 系统，冻结在默认 0 上不推进，纯推 `GameTick` 对
    /// `scheduled_wander_score` 的 `(schedule.seed, tick)` 加权随机抽样毫无影响）并逐 tick
    /// `app.update()`。`tick` 冻结不变会让抽样结果恒定为单次固定值（凡兽日程新增 Forage
    /// 权重与 Wander 分摊同一时段后，该固定值不落在 Wander 分支的概率不可忽略——
    /// 本测试组曾因此在批量跑时真实撞红，见 P0 commit 后续修复）。这里显式推进
    /// `CultivationClock.tick`，让 `(seed, tick)` 每帧换一组独立抽样，40 次内命中 Wander
    /// 的概率趋近 1，而不是冻结在 tick=0 制造只在特定 seed 下才复现的假死锁。
    fn run_thinker_ticks(app: &mut App, ticks: u32) {
        for i in 0..ticks {
            app.insert_resource(crate::npc::movement::GameTick(i));
            app.insert_resource(crate::cultivation::tick::CultivationClock { tick: i as u64 });
            app.update();
        }
    }

    /// big_brain 只在某个 `.when()` 分支**真正被 picker 选中**时才懒惰 spawn 对应的
    /// action 实体（未选中的候选分支压根没有实体，见 `big_brain::thinker::thinker_system`
    /// 的 `exec_picked_action`/`spawn_action` 调用点——不是"实体常在、状态恒 Init"）。
    /// 故断言口径是"当前存在哪个候选实体"而非"读某个候选的 ActionState"：返回本 tick
    /// 挂在 `actor` 身上的候选标签集合（正常稳态应恰好一个）。
    fn active_action_labels(app: &mut App, actor: Entity) -> Vec<&'static str> {
        let world = app.world_mut();
        let mut labels = Vec::new();

        let mut cornered_q =
            world.query_filtered::<&Actor, valence::prelude::With<MeleeAttackAction>>();
        if cornered_q.iter(world).any(|a| a.0 == actor) {
            labels.push("cornered_melee");
        }

        let mut flee_q = world.query_filtered::<&Actor, valence::prelude::With<FleeAction>>();
        if flee_q.iter(world).any(|a| a.0 == actor) {
            labels.push("flee");
        }

        let mut farm_q = world.query_filtered::<&Actor, valence::prelude::With<FarmAction>>();
        if farm_q.iter(world).any(|a| a.0 == actor) {
            labels.push("farm");
        }

        let mut poi_q = world.query_filtered::<&Actor, valence::prelude::With<GoToPoiAction>>();
        if poi_q.iter(world).any(|a| a.0 == actor) {
            labels.push("go_to_poi");
        }

        let mut hunt_q =
            world.query_filtered::<&Actor, valence::prelude::With<MundaneHuntAction>>();
        if hunt_q.iter(world).any(|a| a.0 == actor) {
            labels.push("hunt");
        }

        labels
    }

    #[test]
    fn thinker_branch_wander_when_no_threat_and_not_hungry() {
        // 无玩家/无饥饿信号 → WanderScorer 基线兜底，GoToPoiAction 被选中。
        //
        // `scheduled_wander_score`（`npc/schedule.rs`）走 `activity_for` 的加权随机抽样
        // （不像 `schedule_multiplier` 是纯确定性查表），命中/不命中 Wander 逐 tick 独立
        // 摇骰；本测试只断言"最终态"，若恰好在被检查的那一 tick 摇到非 Wander 分支，
        // 已选中的 GoToPoiAction 会被 picker 取消、留下真空态——与本用例"验证 thinker
        // 接线"的目标无关的概率噪音（`mundane_schedule_weights` 本身的数值健康度已由
        // 独立的生产代码注释 + 阈值算术保证，见该函数文档）。这里显式把 schedule 的
        // `phase_weights` 钉死成"任何阶段都 100% Wander"（同 `fallback_schedule_weights`
        // 的确定性写法），把 `scheduled_wander_score` 的加权随机抽样收窄成恒定
        // `baseline * 1.0`，隔离出本用例真正要测的东西：CorneredScorer/FleeThreatScorer/
        // HungerScorer 都不响应时，WanderScorer 确实兜底把 picker 带到 GoToPoiAction。
        let (mut app, actor) = mundane_thinker_scenario();
        {
            let mut schedule = app
                .world_mut()
                .get_mut::<NpcDailySchedule>(actor)
                .expect("should have NpcDailySchedule");
            schedule.phase_weights = std::collections::HashMap::from([
                (
                    crate::npc::schedule::DayPhase::Dawn,
                    vec![(crate::npc::schedule::ScheduleActivity::Wander, 1.0)],
                ),
                (
                    crate::npc::schedule::DayPhase::Day,
                    vec![(crate::npc::schedule::ScheduleActivity::Wander, 1.0)],
                ),
                (
                    crate::npc::schedule::DayPhase::Dusk,
                    vec![(crate::npc::schedule::ScheduleActivity::Wander, 1.0)],
                ),
                (
                    crate::npc::schedule::DayPhase::Night,
                    vec![(crate::npc::schedule::ScheduleActivity::Wander, 1.0)],
                ),
            ]);
        }
        run_thinker_ticks(&mut app, 40);
        assert_eq!(
            active_action_labels(&mut app, actor),
            vec!["go_to_poi"],
            "无威胁/无饥饿时应恰好选中游荡（WanderScorer 基线兜底），不应有其它分支或零分支"
        );
    }

    #[test]
    fn thinker_branch_farm_when_hungry_and_no_threat() {
        // 无玩家在场，但 Hunger 低于阈值 → HungerScorer 压过 WanderScorer，FarmAction 被选中。
        let (mut app, actor) = mundane_thinker_scenario();
        app.world_mut()
            .get_mut::<crate::npc::hunger::Hunger>(actor)
            .expect("should have Hunger")
            .set(0.05);
        run_thinker_ticks(&mut app, 40);
        assert_eq!(
            active_action_labels(&mut app, actor),
            vec!["farm"],
            "低 Hunger + 无威胁应恰好选中进食（HungerScorer 排在 WanderScorer 前）"
        );
    }

    #[test]
    fn thinker_branch_flee_when_player_within_range_and_not_cornered() {
        // 玩家在 FleeThreatScorer 感知半径(20)内但远超 CorneredScorer 近战距离(4)，
        // 且未被击中（无 retaliation_target）→ FleeThreatScorer 触发 FleeAction。
        let (mut app, actor) = mundane_thinker_scenario();
        app.world_mut()
            .spawn((ClientMarker, Position::new([10.0, 64.0, 0.0])));
        run_thinker_ticks(&mut app, 40);
        assert_eq!(
            active_action_labels(&mut app, actor),
            vec!["flee"],
            "10 格内玩家（未被击中，非近战逼近）应恰好选中逃跑"
        );
    }

    #[test]
    fn thinker_branch_cornered_beats_flee_when_retaliation_active_and_close() {
        // 模拟"被追打至逼入死角"：玩家近战距离内 + retaliation_target 命中窗口未过期 →
        // CorneredScorer 排在 FleeThreatScorer 前，picker 选中 MeleeAttackAction（反击）
        // 而非继续 FleeAction（[[feedback_threat_spectrum]] 硬约束：最低档也能反抗）。
        let (mut app, actor) = mundane_thinker_scenario();
        let player = app
            .world_mut()
            .spawn((ClientMarker, Position::new([2.0, 64.0, 0.0])))
            .id();
        app.world_mut()
            .get_mut::<NpcBlackboard>(actor)
            .expect("should have NpcBlackboard")
            .retaliation_target = Some((player, 10_000));
        run_thinker_ticks(&mut app, 40);
        assert_eq!(
            active_action_labels(&mut app, actor),
            vec!["cornered_melee"],
            "被逼入死角（近战距离 + retaliation_target 命中）应恰好选中反击，\
             而非继续逃跑——CorneredScorer 必须排在 FleeThreatScorer 前"
        );
    }

    // -----------------------------------------------------------------
    // P2 — 狼/狐 Hunt 分支组件清单 + thinker 接线 pin
    // -----------------------------------------------------------------

    #[test]
    fn predator_species_carry_mundane_hunt_state_non_predators_do_not() {
        for (kind, expect_hunt_state) in [
            (MundaneFaunaKind::Wolf, true),
            (MundaneFaunaKind::Fox, true),
            (MundaneFaunaKind::Chicken, false),
            (MundaneFaunaKind::Cow, false),
        ] {
            let (mut app, layer) = make_app_with_layer();
            let entity = {
                let mut commands = app.world_mut().commands();
                spawn_mundane_fauna_at(
                    &mut commands,
                    layer,
                    DEFAULT_SPAWN_ZONE_NAME,
                    DVec3::new(0.0, 64.0, 0.0),
                    DVec3::new(0.0, 64.0, 0.0),
                    kind,
                )
            };
            app.world_mut().flush();
            assert_eq!(
                app.world().get::<MundaneHuntState>(entity).is_some(),
                expect_hunt_state,
                "{kind:?} 的 MundaneHuntState 挂载状态应为 {expect_hunt_state}\
                 （非 predator 挂了也是死组件，故不挂更干净）"
            );
        }
    }

    #[test]
    fn thinker_branch_hunt_when_predator_finds_prey_even_with_player_nearby() {
        // 狼发现猎物时优先追猎，即便附近有玩家（FleeThreatScorer 会响应）——Hunt 分支
        // 排在 FleeThreatScorer 前，狼不是易惊的猎物动物（T2.5"群体掠食"语义）。
        let (mut app, actor) = mundane_thinker_scenario_with_kind(MundaneFaunaKind::Wolf);
        // 猎物：一只兔子在狼的 Hunt 搜索半径内。
        app.world_mut().spawn((
            Position::new([3.0, 64.0, 0.0]),
            MundaneFaunaSpecies(MundaneFaunaKind::Rabbit),
        ));
        // 附近玩家（会让 FleeThreatScorer 打分，验证 Hunt 排序压过它）。
        app.world_mut()
            .spawn((ClientMarker, Position::new([10.0, 64.0, 0.0])));
        run_thinker_ticks(&mut app, 40);
        assert_eq!(
            active_action_labels(&mut app, actor),
            vec!["hunt"],
            "狼在猎物+玩家同时在场时应选中 Hunt（排在 FleeThreatScorer 前），\
             而非逃跑"
        );
    }

    #[test]
    fn thinker_branch_wander_when_predator_has_no_prey_in_range() {
        // 狼周围没有合法猎物时，Hunt 分支评分应为 0，thinker 正常兜底到 Wander。
        let (mut app, actor) = mundane_thinker_scenario_with_kind(MundaneFaunaKind::Wolf);
        {
            let mut schedule = app
                .world_mut()
                .get_mut::<NpcDailySchedule>(actor)
                .expect("should have NpcDailySchedule");
            schedule.phase_weights = std::collections::HashMap::from([
                (
                    crate::npc::schedule::DayPhase::Dawn,
                    vec![(crate::npc::schedule::ScheduleActivity::Wander, 1.0)],
                ),
                (
                    crate::npc::schedule::DayPhase::Day,
                    vec![(crate::npc::schedule::ScheduleActivity::Wander, 1.0)],
                ),
                (
                    crate::npc::schedule::DayPhase::Dusk,
                    vec![(crate::npc::schedule::ScheduleActivity::Wander, 1.0)],
                ),
                (
                    crate::npc::schedule::DayPhase::Night,
                    vec![(crate::npc::schedule::ScheduleActivity::Wander, 1.0)],
                ),
            ]);
        }
        run_thinker_ticks(&mut app, 40);
        assert_eq!(
            active_action_labels(&mut app, actor),
            vec!["go_to_poi"],
            "无猎物在场时狼应兜底走游荡，不应恒卡在 Hunt（pick 返回 None → score 0）"
        );
    }
}
