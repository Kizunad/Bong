//! 飞行中立巨型生物 —— spawn 路径与运行时组件。
//!
//! 渲染契约：客户端 Fabric mod (`com.bong.client.whale`) 在
//! `BongClient.onInitializeClient` 第 N 个注册的 EntityType 是 `bong:whale`。
//! Vanilla MC 1.20.1 有 124 entity_type（id 0..=123）；BongClient 注册顺序
//! `BotanyPlantRenderBootstrap`（→ 124） → `WhaleRenderBootstrap`（→ **125**）。
//! 因此 server 端用 `EntityKind::new(125)` 与 Fabric 注册位号对齐。
//!
//! ⚠️ FRAGILE：这个 ID 依赖于 Fabric mod 注册顺序。任何向 Bong client 加入新
//! EntityType 注册都必须确保排在 `WhaleRenderBootstrap.register()` 之后，
//! 否则 ID 会偏移、协议错位、客户端会把鲸当成另一种实体（或抛出 unknown
//! type ID 的协议异常）。Phase B-2 的"跨进程协议对齐"问题暂未彻底解决，
//! 后续考虑用 registry sync custom payload 替代硬编码。

use bevy_transform::components::{GlobalTransform, Transform};
use big_brain::prelude::{FirstToScore, Thinker, ThinkerBuilder};
use valence::entity::entity::NoGravity;
use valence::entity::phantom::PhantomEntityBundle;
use valence::prelude::{
    bevy_ecs, Commands, Component, DVec3, Entity, EntityKind, EntityLayerId, Look, Position,
};

use crate::fauna::components::{BeastKind, FaunaTag};
use crate::npc::brain_whale::{WhaleDriftAction, WhaleDriftScorer};
use crate::npc::lifecycle::{npc_runtime_bundle, NpcArchetype};
use crate::npc::lod::NpcLodTier;
use crate::npc::spawn::{NpcBlackboard, NpcMarker};
use crate::npc::whale_narration::WhaleSpawnNarrationPending;

/// Fabric 客户端为 `bong:whale` 注册得到的 raw id。详见模块顶部 FRAGILE 注释。
pub const WHALE_ENTITY_KIND: EntityKind = EntityKind::new(125);

/// Thinker 的 score 阈值。当前只一种行为（drift），阈值放低保证一直触发。
const WHALE_THINKER_THRESHOLD: f32 = 0.01;

/// 默认游荡半径（块）。鲸在 home 周围这个范围内自由飘。
pub const DEFAULT_WANDER_RADIUS_XZ: f64 = 96.0;
/// 默认 Y 轴震荡幅度（绕 `home_altitude` 上下波动，单位块）。
/// ±1.25 块 = 总 2.5 块视觉摆幅，飞鲸应该是稳重悬停感而非过山车。
pub const DEFAULT_Y_OSCILLATION_AMPLITUDE: f64 = 1.25;
/// Y 轴震荡周期（tick）。20 tick = 1s，所以 200 tick = 10s 一次完整正弦。
pub const DEFAULT_Y_OSCILLATION_PERIOD_TICKS: u64 = 200;
/// 鲸的巡航速度（block/tick）。0.15 = 3 block/sec，慢飘但仍肉眼可见的位移。
/// 历史：0.05 太慢只感知到 Y 震荡 → 0.5 偏过山车 → 收到 0.15 平衡。
pub const DEFAULT_CRUISE_SPEED: f64 = 0.15;
/// 到达目标判定半径（块）。距离 ≤ 此值视为到达，触发选新目标。
pub const ARRIVAL_RADIUS: f64 = 4.0;
/// 飞鲸 HP 上限。化虚境界单击 ~210，800 HP 让化虚 4-5 击杀；
/// 通灵峰值单击 ~170 也能堆死，但 qi_max=170 须连续投全 qi 才行（恢复时间长，
/// 实操不易）。固元及以下 qi_max ≤ 130 → 单击峰值 130 < 800 / 击杀次数过多
/// 不现实。HP 800 = 神兽级耐打但非无敌的设计目标。
pub const WHALE_HP_MAX: f32 = 800.0;
/// 飞鲸 max_age_ticks。Beast 默认 80_000 tick (~67min ≈ 1.1 in-game 年) 太短。
/// 神兽级长寿给到 100 in-game 年 = 100 × `LIFESPAN_TICKS_PER_YEAR` (72000 tick/年)
/// = 7_200_000 tick = 100 现实小时连续在线时间（远超玩家会观察到的窗口，
/// 实际"自然死亡"几乎不会触发）。
pub const WHALE_LIFESPAN_MAX_TICKS: f64 = 7_200_000.0;

/// 鲸的 brain blackboard：home 锚点、漫游半径、retarget seed 状态。
#[derive(Debug, Clone, Component)]
pub struct WhaleBlackboard {
    /// 鲸的 "家" 中心点。drift 目标都在 home 周围 wander_radius_xz 内挑选。
    pub home_position: DVec3,
    /// 偏好的飞行高度（Y）。drift target 的 Y 锁死等于此值，
    /// 让基础 Y 不漂移；视觉 Y 起伏全部由 flight system 的 sin 震荡贡献。
    pub home_altitude: f64,
    /// 水平游荡半径（XZ 平面）。
    pub wander_radius_xz: f64,
    /// retarget 用的种子。每次选新目标 +1，保证 deterministic + 不重复。
    pub retarget_seq: u64,
}

impl WhaleBlackboard {
    pub fn new(home_position: DVec3, wander_radius_xz: f64) -> Self {
        Self {
            home_position,
            home_altitude: home_position.y,
            wander_radius_xz,
            retarget_seq: 0,
        }
    }
}

/// 鲸的飞行运动控制器。Action 写入 `target`，flight system 读 `target` 推进 `Position`。
///
/// 不复用 `Navigator`：Navigator 是 A* 地面寻路，飞行无寻路需求，直接走
/// 朝向插值 + 正弦 Y 震荡更符合"鲸飘在天上"的视觉直觉。
///
/// **baseline_y 与 visible Y 的关系**：`Position.y` 是叠加 sin 震荡后的可见高度；
/// 如果用 `Position.y` 反推 baseline 会把上轮 sin 偏移当成漂移误差，造成累积偏移
/// （issue：当 amp > cruise_speed 时 baseline 会单方向爬出 target.y 数十块）。
/// 所以单独存 `baseline_y`：flight system 用它做 baseline lerp，最终 `Position.y =
/// baseline_y + sin(phase) * amp` —— 震荡只影响表象，不污染收敛。
#[derive(Debug, Clone, Component)]
pub struct WhaleFlightController {
    /// 当前 drift 目标（XZ 平面 + 基础 Y）。None 时 system 静止。
    pub target: Option<DVec3>,
    /// 巡航速度（block/tick）。
    pub cruise_speed: f64,
    /// Y 轴正弦震荡相位累加器（tick），独立于 target Y。
    pub oscillation_phase_ticks: u64,
    /// Y 震荡幅度（块）。
    pub y_oscillation_amplitude: f64,
    /// Y 震荡周期（tick）。
    pub y_oscillation_period_ticks: u64,
    /// 不含 sin 震荡的"基础"Y 高度，flight system 持续推进它收敛到 target.y。
    /// 可见 `Position.y = baseline_y + sin(phase) * amp`。
    pub baseline_y: f64,
}

impl Default for WhaleFlightController {
    fn default() -> Self {
        Self {
            target: None,
            cruise_speed: DEFAULT_CRUISE_SPEED,
            oscillation_phase_ticks: 0,
            y_oscillation_amplitude: DEFAULT_Y_OSCILLATION_AMPLITUDE,
            y_oscillation_period_ticks: DEFAULT_Y_OSCILLATION_PERIOD_TICKS,
            baseline_y: 0.0,
        }
    }
}

/// Splitmix64 → unit f64，给 retarget 选 deterministic 随机角度/距离。
/// 与 `npc/loot.rs::splitmix64_unit` 是同一变体，但出 f64 给 trig 用。
pub(crate) fn splitmix64_unit_f64(seed: u64) -> f64 {
    let mut x = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    let bits = (x >> 11) & ((1u64 << 53) - 1);
    bits as f64 / (1u64 << 53) as f64
}

/// 在 home 周围 wander_radius_xz 内挑一个 deterministic 随机点。
/// XZ 走极坐标采样，Y **锁死在 home_altitude**（飞鲸要稳重悬停感，
/// 不能让 retarget 把基础 Y 来回拉，否则叠加 sin 震荡会变过山车）。
pub fn pick_drift_target(blackboard: &WhaleBlackboard, _controller_amp: f64) -> DVec3 {
    let seed = blackboard.retarget_seq;
    // angle in [0, 2π)
    let angle =
        splitmix64_unit_f64(seed.wrapping_mul(0xA5A5_5A5A_A5A5_5A5A)) * std::f64::consts::TAU;
    // radius bias toward outer ring：sqrt 让分布在圆盘上更均匀
    let r_unit = splitmix64_unit_f64(seed.wrapping_mul(0x1234_5678_9ABC_DEF0)).sqrt();
    let r = r_unit * blackboard.wander_radius_xz;
    let dx = angle.cos() * r;
    let dz = angle.sin() * r;
    DVec3::new(
        blackboard.home_position.x + dx,
        blackboard.home_altitude,
        blackboard.home_position.z + dz,
    )
}

pub fn whale_npc_thinker() -> ThinkerBuilder {
    Thinker::build()
        .picker(FirstToScore {
            threshold: WHALE_THINKER_THRESHOLD,
        })
        .when(WhaleDriftScorer, WhaleDriftAction)
}

/// Spawn 一只飞鲸。`home_position` 是巡游中心，`wander_radius_xz` 是水平半径。
/// 实际 Y 高度直接取 home_position.y，Y 抖动靠 system 自动叠加。
pub fn spawn_whale_npc_at(
    commands: &mut Commands,
    layer: Entity,
    home_position: DVec3,
    wander_radius_xz: f64,
) -> Entity {
    let yaw_init = 0.0_f32;
    let entity = commands
        .spawn(PhantomEntityBundle {
            // ⚠️ 必须用自定义 EntityKind，不是 EntityKind::PHANTOM —— 否则
            // client 会把它当 vanilla phantom 渲染（不是 GeckoLib whale）。
            kind: WHALE_ENTITY_KIND,
            layer: EntityLayerId(layer),
            position: Position::new([home_position.x, home_position.y, home_position.z]),
            entity_no_gravity: NoGravity(true),
            look: Look::new(yaw_init, 0.0),
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
            // Beast archetype：与 rat / spider 同类，掉落表 / 寿命接现有妖兽逻辑。
            NpcArchetype::Beast,
            FaunaTag::new(BeastKind::Whale),
            NpcLodTier::Dormant,
            WhaleBlackboard::new(home_position, wander_radius_xz),
            WhaleFlightController {
                // baseline_y 必须等于初始 Y，否则首 tick visible Y 会跳到 0+sin(0)=0
                baseline_y: home_position.y,
                ..Default::default()
            },
            // 标记：narration system 下一 tick 读这个 → 广播 spawn 叙事 → 移除标记
            WhaleSpawnNarrationPending,
        ))
        .id();

    let mut runtime = npc_runtime_bundle(entity, NpcArchetype::Beast);
    // 神兽级数值覆写：放在 npc_runtime_bundle 之后、insert 之前修改，避免 ECS 后处理覆写。
    runtime.wounds.health_current = WHALE_HP_MAX;
    runtime.wounds.health_max = WHALE_HP_MAX;
    runtime.lifespan.max_age_ticks = WHALE_LIFESPAN_MAX_TICKS;
    commands
        .entity(entity)
        .insert((whale_npc_thinker(), runtime));

    entity
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::testing::ScenarioSingleClient;

    #[test]
    fn spawn_whale_attaches_fauna_whale_tag_and_blackboard() {
        let scenario = ScenarioSingleClient::new();
        let layer = scenario.layer;
        let mut app = scenario.app;
        let whale = spawn_whale_npc_at(
            &mut app.world_mut().commands(),
            layer,
            DVec3::new(0.0, 96.0, 0.0),
            DEFAULT_WANDER_RADIUS_XZ,
        );
        app.world_mut().flush();

        // FaunaTag::Whale 必须打上去（drop 表识别 / 妖兽 lifecycle 都看它）
        assert_eq!(
            app.world().get::<FaunaTag>(whale).map(|t| t.beast_kind),
            Some(BeastKind::Whale)
        );
        let bb = app
            .world()
            .get::<WhaleBlackboard>(whale)
            .expect("WhaleBlackboard must be attached");
        assert_eq!(bb.home_position, DVec3::new(0.0, 96.0, 0.0));
        assert_eq!(bb.home_altitude, 96.0);
        assert_eq!(bb.wander_radius_xz, DEFAULT_WANDER_RADIUS_XZ);
        assert_eq!(bb.retarget_seq, 0);
    }

    #[test]
    fn spawn_whale_uses_custom_entity_kind_not_phantom() {
        let scenario = ScenarioSingleClient::new();
        let layer = scenario.layer;
        let mut app = scenario.app;
        let whale = spawn_whale_npc_at(
            &mut app.world_mut().commands(),
            layer,
            DVec3::new(10.0, 100.0, 10.0),
            DEFAULT_WANDER_RADIUS_XZ,
        );
        app.world_mut().flush();

        // 鲸用 ID 125（自定义 EntityKind），绝不能等于 vanilla EntityKind::PHANTOM(71)
        let kind = app.world().get::<EntityKind>(whale).copied();
        assert_eq!(kind, Some(WHALE_ENTITY_KIND));
        assert_ne!(kind, Some(EntityKind::PHANTOM));
        assert_eq!(WHALE_ENTITY_KIND.get(), 125);
    }

    #[test]
    fn spawn_whale_hp_pool_is_neutral_giant_tier() {
        // 神兽级 HP：化虚单击 ~210 也要 4 击才打死，固元及以下基本无望
        let scenario = ScenarioSingleClient::new();
        let layer = scenario.layer;
        let mut app = scenario.app;
        let whale = spawn_whale_npc_at(
            &mut app.world_mut().commands(),
            layer,
            DVec3::new(0.0, 100.0, 0.0),
            DEFAULT_WANDER_RADIUS_XZ,
        );
        app.world_mut().flush();

        let wounds = app
            .world()
            .get::<crate::combat::components::Wounds>(whale)
            .expect("Wounds must be attached via npc_runtime_bundle");
        assert_eq!(
            wounds.health_max, WHALE_HP_MAX,
            "whale.health_max must be {WHALE_HP_MAX} (neutral giant tier), default 100 not enough"
        );
        assert_eq!(wounds.health_current, WHALE_HP_MAX, "whale spawns full HP");
        // 化虚 4 击杀 sanity：4 × 210 = 840 > 800
        assert!(
            4.0 * 210.0 >= WHALE_HP_MAX as f64,
            "化虚 4 击 (4×qi_max=840) 必须能打死 (HP={WHALE_HP_MAX})"
        );
        // 固元堆死不现实 sanity：12 击 × 130 qi_max = 1560，但实际玩家 qi 用一击 130 后要恢复，
        // 12 击需要 12 次满 qi 周期 → 战斗中达不到
        assert!(
            (WHALE_HP_MAX as f64) > 130.0 * 4.0,
            "固元 4 击 (4×130=520) 必须不足以打死 (HP={WHALE_HP_MAX})"
        );
    }

    #[test]
    fn spawn_whale_lifespan_is_long() {
        // 神兽级寿命：默认 Beast 8 万 tick (~67min) → 80 万 tick (~11h) 让玩家有充裕互动窗口
        let scenario = ScenarioSingleClient::new();
        let layer = scenario.layer;
        let mut app = scenario.app;
        let whale = spawn_whale_npc_at(
            &mut app.world_mut().commands(),
            layer,
            DVec3::new(0.0, 100.0, 0.0),
            DEFAULT_WANDER_RADIUS_XZ,
        );
        app.world_mut().flush();

        let lifespan = app
            .world()
            .get::<crate::npc::lifecycle::NpcLifespan>(whale)
            .expect("NpcLifespan must be attached");
        assert_eq!(
            lifespan.max_age_ticks, WHALE_LIFESPAN_MAX_TICKS,
            "whale.max_age_ticks must be {WHALE_LIFESPAN_MAX_TICKS} (= 100 in-game 年)"
        );
        assert_eq!(lifespan.age_ticks, 0.0, "whale spawns at age 0");
        // pin：100 年契约 (LIFESPAN_TICKS_PER_YEAR=72000)
        assert_eq!(
            WHALE_LIFESPAN_MAX_TICKS,
            100.0 * 72_000.0,
            "spec：100 in-game 年 = 100 × 72000 tick"
        );
    }

    #[test]
    fn whale_overrides_do_not_leak_to_default_beast_runtime_bundle() {
        // 回归保护：whale 覆写 HP/lifespan 不能影响其他 Beast (rat 等) 的默认值
        use bevy_ecs::entity::Entity;
        // 拿一个 placeholder entity id；npc_runtime_bundle 只读 entity_bits，不索引 ECS
        let ph = Entity::from_raw(99);
        let bundle = npc_runtime_bundle(ph, NpcArchetype::Beast);
        assert_eq!(
            bundle.wounds.health_max, 100.0,
            "默认 Beast HP 必须 100（whale override 是局部的）"
        );
        assert_eq!(
            bundle.lifespan.max_age_ticks, 80_000.0,
            "默认 Beast 寿命必须 8 万 tick（whale override 是局部的）"
        );
    }

    #[test]
    fn spawn_whale_starts_with_no_gravity_and_no_drift_target() {
        let scenario = ScenarioSingleClient::new();
        let layer = scenario.layer;
        let mut app = scenario.app;
        let whale = spawn_whale_npc_at(
            &mut app.world_mut().commands(),
            layer,
            DVec3::new(0.0, 80.0, 0.0),
            32.0,
        );
        app.world_mut().flush();

        // 飞行就靠 NoGravity；不设 client 会让它垂直坠落
        assert_eq!(app.world().get::<NoGravity>(whale).map(|g| g.0), Some(true));
        // FlightController 初始无目标（靠 brain action 第一帧填）
        let ctrl = app
            .world()
            .get::<WhaleFlightController>(whale)
            .expect("WhaleFlightController must be attached");
        assert!(
            ctrl.target.is_none(),
            "fresh whale must have no drift target"
        );
        assert_eq!(ctrl.cruise_speed, DEFAULT_CRUISE_SPEED);
        assert_eq!(ctrl.oscillation_phase_ticks, 0);
        // baseline_y 必须等于 home Y（80），否则首 tick visible Y 会从 0 跳到 home
        assert_eq!(
            ctrl.baseline_y, 80.0,
            "spawn 时 baseline_y 应该 = home_position.y，而不是 default 0"
        );
    }

    #[test]
    fn pick_drift_target_stays_within_wander_radius_xz() {
        // 饱和：1000 个 retarget seq 全都不能跑出 home 周围 wander_radius
        let mut bb = WhaleBlackboard::new(DVec3::new(100.0, 64.0, 200.0), 50.0);
        for seq in 0..1000 {
            bb.retarget_seq = seq;
            let target = pick_drift_target(&bb, DEFAULT_Y_OSCILLATION_AMPLITUDE);
            let dx = target.x - bb.home_position.x;
            let dz = target.z - bb.home_position.z;
            let r_xz = (dx * dx + dz * dz).sqrt();
            assert!(
                r_xz <= bb.wander_radius_xz + 1e-6,
                "seq {seq}: drift target XZ radius {r_xz} > wander {} (home {:?}, target {:?})",
                bb.wander_radius_xz,
                bb.home_position,
                target
            );
            // 飞鲸目标 Y 永远等于 home_altitude（pick 不引入 Y 抖动）
            assert!(
                (target.y - bb.home_altitude).abs() < 1e-9,
                "seq {seq}: target.y={} must equal home_altitude={}",
                target.y,
                bb.home_altitude
            );
        }
    }

    #[test]
    fn pick_drift_target_is_deterministic_given_seq() {
        let bb = WhaleBlackboard::new(DVec3::new(0.0, 64.0, 0.0), 32.0);
        let t1 = pick_drift_target(&bb, 6.0);
        let t2 = pick_drift_target(&bb, 6.0);
        assert_eq!(
            t1, t2,
            "same seq must produce same drift target (deterministic)"
        );
    }

    #[test]
    fn pick_drift_target_changes_with_seq() {
        // 饱和：相邻 seq 应给出不同目标（splitmix64 散列性质）
        let mut bb = WhaleBlackboard::new(DVec3::new(0.0, 64.0, 0.0), 32.0);
        bb.retarget_seq = 0;
        let t0 = pick_drift_target(&bb, 6.0);
        bb.retarget_seq = 1;
        let t1 = pick_drift_target(&bb, 6.0);
        assert_ne!(t0, t1, "seq+1 should yield a different drift target");
    }

    #[test]
    fn pick_drift_target_zero_radius_pins_to_home_xz() {
        // 边界：wander_radius=0 → XZ 锁 home，Y 也锁 home_altitude
        let bb = WhaleBlackboard::new(DVec3::new(50.0, 64.0, -30.0), 0.0);
        for seq in 0..50 {
            let mut bb = bb.clone();
            bb.retarget_seq = seq;
            let t = pick_drift_target(&bb, 6.0);
            assert!(
                (t.x - 50.0).abs() < 1e-9,
                "seq {seq}: X drift with radius 0"
            );
            assert!(
                (t.z - -30.0).abs() < 1e-9,
                "seq {seq}: Z drift with radius 0"
            );
            assert!(
                (t.y - 64.0).abs() < 1e-9,
                "seq {seq}: Y must lock to home_altitude"
            );
        }
    }

    #[test]
    fn splitmix64_unit_f64_is_in_unit_interval() {
        for s in 0..10_000u64 {
            let v = splitmix64_unit_f64(s);
            assert!(
                (0.0..1.0).contains(&v),
                "splitmix64_unit_f64({s}) = {v} outside [0,1)"
            );
        }
    }

    #[test]
    fn whale_entity_kind_constant_is_125() {
        // pin 测试：alignment with Fabric register ordering; 任何变更必须连同
        // client BongClient.java 的 register 顺序一并改，否则协议错位。
        assert_eq!(WHALE_ENTITY_KIND.get(), 125);
    }
}
