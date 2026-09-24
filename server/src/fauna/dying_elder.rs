//! plan-dying-elder-v1 P0/P1 — 垂死大能核心数据结构、spawn 触发逻辑、给丹交互、夺舍系统。
//!
//! 垂死大能：困于坍缩渊的化虚修士，真元被持续消耗，向玩家求助换取传承。
//! 若玩家累计给丹 ≥5 颗大能依概率翻脸夺舍（永久 qi_max 减损）或自裁（zone 真元大释放）。
//! 正解（worldview §七「算计至上」）：拖延看其自毙后舔包。
//!
//! ## P0 交付物
//!
//! - [`DyingElderState`] — 四态状态机（Plea / Recovering / Betrayal / Dead）。
//! - [`DyingElderBlackboard`] — 个体行为帧（betray_probability / qi_max_cache / offered_skill_id /
//!   dan_received / spawn_tick）。
//! - [`DyingElderSpawnTimer`] — 全服级 spawn 计时 Resource（30 in-game days 周期）。
//! - [`DyingElderSpawnSystem`] — spawn 系统（gate: TSY zone + spirit_qi < -0.4 + 全服上限 1）。
//! - [`EARTH_GRADE_TECHNIQUE_POOL`] — 地阶功法池（spawn 时随机选 offered_skill_id）。
//! - P0 单测 ≥8 条。
//!
//! ## 守恒红线
//!
//! 1. **给丹**（P1）：丹 qi_gain 走 `QiTransfer{TradeDan}`；丹从 inventory 真删。
//! 2. **drain 衰减**（P2）：用 `compute_drain_per_tick` + `QiTransfer{RiftCollapse}`。
//! 3. **死亡释放**（P2）：`release_qi_amount_to_zone` 全额（化虚级 ~500），zone spirit_qi 跃升。
//! 4. **夺舍**（P1）：player qi_current → elder `QiTransfer{SoulSeize}`；
//!    qi_max 永久 debuff 是容量变化，**不** 重复计入 transfer。

use bevy_transform::components::{GlobalTransform, Transform};
use serde::{Deserialize, Serialize};
use valence::client::ClientMarker;
use valence::entity::marker::MarkerEntityBundle;
use valence::entity::EntityId;
use valence::prelude::{
    bevy_ecs, App, Commands, Component, DVec3, Entity, EntityKind, EntityLayerId, EventReader,
    EventWriter, IntoSystemConfigs, Position, Query, Res, ResMut, Resource, Update, With, Without,
};

use crate::cultivation::components::Cultivation;
use crate::inventory::freshness::GAME_DAY_TICKS;
use crate::inventory::{
    consume_item_instance_once, inventory_item_by_instance_borrow, DroppedLootRegistry,
    InventoryInstanceIdAllocator, ItemEffect, ItemInstance, ItemRegistry, PlayerInventory,
};
use crate::network::redis_bridge::RedisOutbound;
use crate::network::RedisBridgeResource;
use crate::npc::lifecycle::{npc_runtime_bundle, NpcArchetype};
use crate::npc::movement::GameTick;
use crate::npc::spawn::NpcMarker;
use crate::qi_physics::constants::{QI_EPSILON, QI_ZONE_UNIT_CAPACITY};
use crate::qi_physics::ledger::{
    dying_elder_dan_excess_account, dying_elder_release_overflow_account,
    transfer_external_qi_to_ledger, QiAccountId, QiTransfer, QiTransferReason, WorldQiAccount,
};
use crate::qi_physics::release::qi_release_to_zone;
use crate::schema::elder_encounter::{ElderEncounterEventKindV1, ElderEncounterEventV1};
use crate::social::components::Renown;
use crate::world::dimension::{CurrentDimension, DimensionKind, DimensionLayers};
use crate::world::tsy_drain::compute_drain_per_tick;
use crate::world::zone::ZoneRegistry;

// ── 常数 ─────────────────────────────────────────────────────────────────────

/// 垂死大能 spawn 周期：30 in-game days（稀有化，worldview §七「极度稀有」）。
/// `GAME_DAY_TICKS = 24_000`（server/src/inventory/freshness.rs:12）。
pub const DYING_ELDER_SPAWN_INTERVAL_TICKS: u64 = 30 * GAME_DAY_TICKS;

/// 全服同时存在的垂死大能上限（1 个）。
/// 稀有遭遇：同时只允许 1 个大能存在，进一步压低频率。
pub const DYING_ELDER_GLOBAL_CAP: usize = 1;

/// spawn gate：zone.spirit_qi 须低于此阈值（坍缩渊灵气严重匮乏）。
/// -0.4 是坍缩渊特征值，保证只在深度负灵域 spawn。
pub const DYING_ELDER_SPIRIT_QI_THRESHOLD: f64 = -0.4;

/// 垂死大能初始真元：化虚境界大能（worldview §三:78 化虚稀缺）。
/// 化虚境界 qi_max ≈ 500（spawn 时按此值初始化 qi_current）。
pub const DYING_ELDER_INITIAL_QI: f64 = 500.0;

/// 大能翻脸概率下界（随机分布的最小值）。
pub const DYING_ELDER_BETRAY_PROB_MIN: f64 = 0.30;

/// 大能翻脸概率上界（随机分布的最大值）。
pub const DYING_ELDER_BETRAY_PROB_MAX: f64 = 0.95;

/// 玩家声名阈值：fame > 此值时大能 betray_probability -= 0.2。
pub const DYING_ELDER_RENOWN_THRESHOLD: i32 = 300;

/// 声名加成对 betray_probability 的减量。
pub const DYING_ELDER_RENOWN_BETRAY_REDUCTION: f64 = 0.2;

/// 给丹 threshold：累计 ≥ 此值触发结局判定（守信自裁 or 翻脸夺舍）。
pub const DYING_ELDER_DAN_THRESHOLD: u32 = 5;

/// 夺舍 qi_max 减损比例（永久 debuff，worldview「高代价」）。
/// 被夺舍玩家的 qi_max 永久减少 10%（以大能 qi_max_cache 的 10% 为量）。
pub const DYING_ELDER_SOUL_SEIZE_RATIO: f64 = 0.10;

// ── 地阶功法池 ─────────────────────────────────────────────────────────────────

/// 地阶功法池：spawn 时随机选一门作为 offered_skill_id。
///
/// 技法来源（server/src/cultivation/known_techniques.rs grep）：
/// - `woliu.heart` — 无流心诀（地阶心法）
/// - `woliu.turbulence_burst` — 无流湍爆（地阶杀招）
/// - `anqi.echo_fractal` — 暗器回声裂变（地阶辅助）
/// - `sword_path.heaven_gate` — 剑道天门（地阶剑式）
pub const EARTH_GRADE_TECHNIQUE_POOL: &[&str] = &[
    "woliu.heart",
    "woliu.turbulence_burst",
    "anqi.echo_fractal",
    "sword_path.heaven_gate",
];

// ── 状态机 ────────────────────────────────────────────────────────────────────

/// 垂死大能四态状态机。
///
/// - `Plea`：乞求态，大能向玩家求助（初始态）。负灵域持续消耗真元。
/// - `Recovering(u32)`：恢复态，已收到丹，inner = 已收到丹数量。
///   每次给丹递增；累计 ≥ DYING_ELDER_DAN_THRESHOLD 时触发结局判定。
/// - `Betrayal`：翻脸夺舍态，大能夺舍玩家（emit SoulSeizeEvent）。
/// - `Dead`：死亡态，由死亡系统处理 qi 释放 + loot 生成。
///
/// 状态转换路径：
/// - `Plea` → `Recovering(n)` ：玩家给丹
/// - `Plea` → `Dead` ：真元耗尽自然死亡 / 被玩家击杀
/// - `Recovering(n)` → `Betrayal` ：累计 ≥5 丹 + rand < betray_probability
/// - `Recovering(n)` → `Dead` ：累计 ≥5 丹 + rand ≥ betray_probability（守信自裁）
/// - `Betrayal` → `Dead` ：夺舍完成后大能力竭死亡
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Component, Default)]
#[serde(rename_all = "snake_case")]
pub enum DyingElderState {
    /// 乞求态：求助玩家，等待给丹。负灵域 drain 持续消耗真元。
    #[default]
    Plea,
    /// 恢复态：已收到 N 颗丹，inner 为累计丹数。真元有所恢复。
    /// 累计 ≥ DYING_ELDER_DAN_THRESHOLD 时触发结局判定。
    Recovering {
        /// 已累计收到的丹数（0..=DYING_ELDER_DAN_THRESHOLD）。
        dan_received: u32,
    },
    /// 翻脸夺舍态：rand < betray_probability 时进入，emit SoulSeizeEvent。
    /// 守恒：player qi_current → elder via QiTransfer{SoulSeize}。
    Betrayal,
    /// 死亡态：自然死亡 / 守信自裁 / 被击杀 / 夺舍力竭。
    /// dead_by_betrayal = true 时 loot 质量稍差（玩家被算计的代价）。
    Dead {
        /// 是否死于背叛路线（夺舍力竭）。
        dead_by_betrayal: bool,
    },
}

// ── Blackboard ────────────────────────────────────────────────────────────────

/// 垂死大能个体行为帧（ECS Component）。
///
/// ## 守恒字段
/// - `qi_current`：大能当前真元，P2 每 tick 被 `compute_drain_per_tick` 扣减；
///   死亡时由 `DyingElderDeathSystem` 全额 `release_qi_amount_to_zone`。
/// - `qi_max_cache`：spawn 时记录的初始 qi_max（用于 SoulSeize drain 计算）。
///
/// ## 设计决议
/// - `betray_probability`：spawn 时 [DYING_ELDER_BETRAY_PROB_MIN, DYING_ELDER_BETRAY_PROB_MAX]
///   随机，声名 fame > 300 时 -= 0.2。
/// - `offered_skill_id`：spawn 时从 EARTH_GRADE_TECHNIQUE_POOL 随机选一门。
#[derive(Debug, Clone, PartialEq, Component)]
pub struct DyingElderBlackboard {
    /// 孵化区域名称（TSY zone name，守恒账户定位）。
    pub home_zone: String,
    /// 孵化位置（用于 VFX / loot 掉落定位）。
    pub home_pos: DVec3,
    /// 当前真元（spawn 时 = DYING_ELDER_INITIAL_QI，每 tick 被 drain 扣减）。
    /// 死亡时全额归还 zone via release_qi_amount_to_zone。
    pub qi_current: f64,
    /// 初始 qi_max（spawn 时固定，用于 SoulSeize 计算 10% drain）。
    pub qi_max_cache: f64,
    /// 翻脸概率 [0.0, 1.0]，spawn 时随机初始化，声名调整后限制在 [0.05, 0.95]。
    pub betray_probability: f64,
    /// 本次遭遇承诺传授的地阶功法 ID（来自 EARTH_GRADE_TECHNIQUE_POOL）。
    pub offered_skill_id: &'static str,
    /// spawn tick（用于 log / 审计）。
    pub spawn_tick: u64,
}

impl DyingElderBlackboard {
    /// 用确定性 splitmix64 seed 初始化 blackboard。
    ///
    /// seed 由调用方（DyingElderSpawnSystem）基于 zone + tick 构造，保证跨重启稳定。
    pub fn new(home_zone: &str, home_pos: DVec3, seed: u64, spawn_tick: u64) -> Self {
        // splitmix64 第一步：生成 betray_probability
        let s1 = seed
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .wrapping_add(0x6C62_272E_07BB_0142);
        let prob_raw = (s1 % 1000) as f64 / 1000.0;
        let betray_probability = DYING_ELDER_BETRAY_PROB_MIN
            + prob_raw * (DYING_ELDER_BETRAY_PROB_MAX - DYING_ELDER_BETRAY_PROB_MIN);

        // splitmix64 第二步：选 offered_skill_id
        let s2 = s1
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .wrapping_add(0x6C62_272E_07BB_0142);
        let skill_idx = (s2 % EARTH_GRADE_TECHNIQUE_POOL.len() as u64) as usize;
        let offered_skill_id = EARTH_GRADE_TECHNIQUE_POOL[skill_idx];

        Self {
            home_zone: home_zone.to_string(),
            home_pos,
            qi_current: DYING_ELDER_INITIAL_QI,
            qi_max_cache: DYING_ELDER_INITIAL_QI,
            betray_probability,
            offered_skill_id,
            spawn_tick,
        }
    }

    /// 应用声名调整：fame > DYING_ELDER_RENOWN_THRESHOLD 时 betray_probability -= 0.2。
    /// 概率限制在 [0.05, 0.95] 防止极端值。
    pub fn apply_renown_adjustment(&mut self, fame: i32) {
        if fame > DYING_ELDER_RENOWN_THRESHOLD {
            self.betray_probability =
                (self.betray_probability - DYING_ELDER_RENOWN_BETRAY_REDUCTION).clamp(0.05, 0.95);
        }
    }
}

// ── 全服 Spawn 计时器 ─────────────────────────────────────────────────────────

/// 全服级垂死大能 spawn 计时 Resource。
///
/// 每 `DYING_ELDER_SPAWN_INTERVAL_TICKS`（30 in-game days = 720_000 ticks）尝试 spawn 一次。
/// 全服上限 1 个（`DYING_ELDER_GLOBAL_CAP`）—— 稀有遭遇，稀少才显珍贵。
#[derive(Debug, Default, Resource)]
pub struct DyingElderSpawnTimer {
    /// 上次 spawn 尝试的 tick（0 = 未曾尝试）。
    pub last_spawn_attempt_tick: u64,
    /// 累计 spawn 次数（统计 / 审计用）。
    pub total_spawned: u32,
}

// ── Spawn 系统 ────────────────────────────────────────────────────────────────

/// 全服垂死大能 spawn 系统。
///
/// 每 `DYING_ELDER_SPAWN_INTERVAL_TICKS` tick 检查一次：
/// 1. gate: TSY zone（`zone.is_tsy()`）
/// 2. gate: `zone.spirit_qi < DYING_ELDER_SPIRIT_QI_THRESHOLD`（-0.4）
/// 3. gate: 全服现存垂死大能 < `DYING_ELDER_GLOBAL_CAP`（上限 1）
/// 4. 从满足条件的 TSY zone 中选第一个 spawn（P2 可加权随机）
/// 5. spawn 事件（P0 仅记录 spawn request；实际 spawn entity 留 P1 完善）
///
/// **不实际创建 ECS Entity**：P0 系统只做 gate 判断 + emit `DyingElderSpawnRequest`；
/// 实际 entity 创建留 P1（attach big-brain / Position / NpcMarker 完整 bundle）。
#[allow(clippy::too_many_arguments)]
pub(crate) fn dying_elder_spawn_system(
    zones: Option<Res<ZoneRegistry>>,
    game_tick: Option<Res<GameTick>>,
    mut spawn_timer: Option<ResMut<DyingElderSpawnTimer>>,
    existing_elders: Query<&DyingElderBlackboard, (With<NpcMarker>, Without<ClientMarker>)>,
    mut spawn_requests: EventWriter<DyingElderSpawnRequest>,
) {
    let tick = game_tick.as_deref().map(|t| t.0 as u64).unwrap_or(0);
    let Some(zones) = zones else { return };
    let Some(ref mut timer) = spawn_timer else {
        return;
    };

    // 频率 gate：距上次尝试不足一个周期，跳过
    if tick.saturating_sub(timer.last_spawn_attempt_tick) < DYING_ELDER_SPAWN_INTERVAL_TICKS {
        return;
    }

    // 全服上限 gate：已存在垂死大能，跳过
    let existing_count = existing_elders.iter().count();
    if existing_count >= DYING_ELDER_GLOBAL_CAP {
        timer.last_spawn_attempt_tick = tick;
        return;
    }

    // 寻找满足条件的 TSY zone（is_tsy + spirit_qi < -0.4）
    let candidate = zones
        .zones
        .iter()
        .find(|z| z.is_tsy() && z.spirit_qi < DYING_ELDER_SPIRIT_QI_THRESHOLD);

    if let Some(zone) = candidate {
        // 用 zone name hash ^ tick 作为确定性 seed
        let seed = zone
            .name
            .bytes()
            .fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64))
            ^ tick;

        let spawn_pos = zone.center();
        let bb = DyingElderBlackboard::new(&zone.name, spawn_pos, seed, tick);

        timer.last_spawn_attempt_tick = tick;
        timer.total_spawned += 1;

        spawn_requests.send(DyingElderSpawnRequest {
            zone_name: zone.name.clone(),
            spawn_pos,
            blackboard: bb,
            tick,
        });
    } else {
        // 无满足条件的 TSY zone，更新计时器（避免同 tick 反复检查）
        timer.last_spawn_attempt_tick = tick;
    }
}

// ── Spawn 请求事件 ─────────────────────────────────────────────────────────────

/// 垂死大能 spawn 请求事件（P0 emit，P1 消费实际创建 entity）。
///
/// 解耦系统间依赖：spawn 判断与 entity 创建分离，避免 DyingElderSpawnSystem 直接持有
/// Commands（与 ZoneRegistry 的不可变借用冲突）。
#[derive(Debug, Clone, valence::prelude::Event)]
pub struct DyingElderSpawnRequest {
    /// 目标 TSY zone 名（大能 home_zone）。
    pub zone_name: String,
    /// spawn 坐标（zone 中心）。
    pub spawn_pos: DVec3,
    /// 初始化好的 Blackboard。
    pub blackboard: DyingElderBlackboard,
    /// spawn 触发 tick（审计用）。
    pub tick: u64,
}

// ── 大能已出现事件 ─────────────────────────────────────────────────────────────

/// plan-dying-elder-v1 Bug2 修复 — 大能 entity 真正创建后 emit 的事件。
///
/// `DyingElderSpawnRequest` 在 P0 emit，但 entity 由 P1 的 `dying_elder_apply_spawn_system`
/// 创建；二者之间 entity id 未知。本事件在 entity 创建后立即 emit，携带 ECS Entity，
/// 供 P3/S2C appear event 系统通过查询 `EntityId` 组件取得 MC protocol entity_id。
#[derive(Debug, Clone, valence::prelude::Event)]
pub struct DyingElderAppearedEvent {
    /// 真实 elder ECS entity（P3/S2C 系统通过此值查询 &EntityId 取得 MC protocol entity_id）。
    pub elder: Entity,
    /// TSY zone 名称（来自 spawn request）。
    pub zone_name: String,
    /// 初始化好的 Blackboard（含 betray_probability / offered_skill_id）。
    pub blackboard: DyingElderBlackboard,
    /// spawn 触发 tick。
    pub tick: u64,
}

// ── Bevy 注册 ──────────────────────────────────────────────────────────────────

/// Bevy 注册：P0 spawn timer resource + spawn 系统。
pub fn register_p0(app: &mut App) {
    app.add_event::<DyingElderSpawnRequest>();
    app.insert_resource(DyingElderSpawnTimer::default());
    app.add_systems(Update, dying_elder_spawn_system);
}

// ── Spawn apply 系统 ────────────────────────────────────────────────────────────

/// plan-dying-elder-v1 P1 — 消费 `DyingElderSpawnRequest`，创建携带完整组件 bundle 的大能 entity。
///
/// P0 spawn 系统只负责 gate 判断 + emit `DyingElderSpawnRequest`；
/// 本系统（独立 Bevy event reader）在同一帧内消费该事件，真正将大能 entity 插入 ECS World。
///
/// ## 创建的组件 bundle（Bug3 修复：补齐 Valence 客户端可见所需组件）
/// - [`MarkerEntityBundle`]：`EntityKind::VILLAGER` + `EntityLayerId(tsy)` + `Position`
///   → Valence 凭此向客户端发送 entity 数据包（`EntityKind` 是客户端渲染必要条件）
/// - [`Transform`] + [`GlobalTransform`]：Bevy 变换组件（NPC 系统依赖）
/// - [`DyingElderState::Plea`]：初始乞求态
/// - [`DyingElderBlackboard`]：从 spawn request 内联 blackboard（含 betray_probability / offered_skill_id）
/// - [`NpcMarker`]：标记为 NPC entity（全服系统依赖此 marker 定向查询）
/// - [`NpcArchetype::DyingElder`]（通过 `npc_runtime_bundle` 包含）
/// - [`Cultivation`]：化虚境界初始真元（qi_current = qi_max = DYING_ELDER_INITIAL_QI）
///
/// ## Bug2 修复：emit DyingElderAppearedEvent 携带真实 ECS Entity
/// entity 创建后立即 emit `DyingElderAppearedEvent`，供 P3/S2C appear 系统
/// 查询 `&EntityId` 取 MC protocol entity_id（替代原来的 0 占位）。
pub(crate) fn dying_elder_apply_spawn_system(
    mut commands: Commands,
    mut spawn_requests: EventReader<DyingElderSpawnRequest>,
    dimension_layers: Option<Res<DimensionLayers>>,
    mut appeared_events: EventWriter<DyingElderAppearedEvent>,
    game_tick: Option<Res<GameTick>>,
) {
    let tick = game_tick.as_deref().map(|t| t.0 as u64).unwrap_or(0);
    // Bug3：TSY layer entity 用于 EntityLayerId（让 Valence 向客户端广播大能实体）
    // 大能固定 spawn 在 TSY zone（gate 已确保），使用 dimension_layers.tsy
    let tsy_layer = dimension_layers.as_deref().map(|dl| dl.tsy);

    for req in spawn_requests.read() {
        let bb = req.blackboard.clone();
        let pos = req.spawn_pos;

        // ── 构建大能 Cultivation（化虚境界，qi_current = qi_max = DYING_ELDER_INITIAL_QI）
        let mut cultivation = Cultivation::default();
        cultivation.realm = crate::cultivation::components::Realm::Void;
        cultivation.qi_current = DYING_ELDER_INITIAL_QI;
        cultivation.qi_max = DYING_ELDER_INITIAL_QI;

        // ── Bug3 修复：MarkerEntityBundle 提供 EntityKind + EntityLayerId，让 Valence 向客户端发包
        // EntityKind::VILLAGER 在 MC 1.20.1 是 120，用于标识大能外观（P4 可换自定义 skin）
        let entity = if let Some(layer) = tsy_layer {
            commands
                .spawn((
                    MarkerEntityBundle {
                        kind: EntityKind::VILLAGER,
                        layer: EntityLayerId(layer),
                        position: Position::new([pos.x, pos.y, pos.z]),
                        ..Default::default()
                    },
                    Transform::from_xyz(pos.x as f32, pos.y as f32, pos.z as f32),
                    GlobalTransform::default(),
                    NpcMarker,
                    CurrentDimension(DimensionKind::Tsy),
                    DyingElderState::Plea,
                    bb.clone(),
                    NpcArchetype::DyingElder,
                ))
                .id()
        } else {
            // DimensionLayers 未注册（测试环境）：退化路径，无 MarkerEntityBundle
            commands
                .spawn((
                    NpcMarker,
                    CurrentDimension(DimensionKind::Tsy),
                    Position::new([pos.x, pos.y, pos.z]),
                    DyingElderState::Plea,
                    bb.clone(),
                    NpcArchetype::DyingElder,
                ))
                .id()
        };

        // ── 覆盖 npc_runtime_bundle 中的 Cultivation（化虚级 qi）
        // realm 实参必须是真实 Realm::Void——npc_runtime_bundle 内部用它经
        // npc_meridian_system_for_realm(realm) 派生 meridian_system（20 条经脉），
        // 这个字段不受下方 `runtime.cultivation = cultivation` 覆盖（cultivation 和
        // meridian_system 是 NpcRuntimeBundle 两个独立字段）。此前误传占位
        // Realm::Awaken 导致 meridian_system 只开 1 脉，与落地 cultivation.realm=Void
        // （required_meridians=20）双源矛盾（realm↔经脉双源 bug，P0 回归锁见
        // lifecycle.rs + 下方 apply_spawn_system_keeps_void_realm_and_full_qi_regression_lock）。
        let mut runtime = npc_runtime_bundle(
            entity,
            NpcArchetype::DyingElder,
            crate::cultivation::components::Realm::Void,
        );
        runtime.cultivation = cultivation;
        commands.entity(entity).insert(runtime);

        tracing::info!(
            "[bong][dying_elder] apply_spawn: created entity {:?} (idx={}) at {:?} zone='{}' betray_prob={:.3} layer={:?}",
            entity,
            entity.index(),
            pos,
            req.zone_name,
            req.blackboard.betray_probability,
            tsy_layer,
        );

        // ── Bug2 修复：emit DyingElderAppearedEvent 携带真实 entity idx ──────
        appeared_events.send(DyingElderAppearedEvent {
            elder: entity,
            zone_name: req.zone_name.clone(),
            blackboard: req.blackboard.clone(),
            tick,
        });
    }
}

// ── P1：给丹交互事件 ──────────────────────────────────────────────────────────

/// plan-dying-elder-v1 P1 — 玩家给大能交付一颗回元丹后由网络层 emit 的意图事件。
///
/// 网络层（`handle_give_dan_to_elder`）负责：
/// 1. 校验 pill_instance_id 属于该玩家且模板为 `huiyuan_pill`（pills.toml id，无下划线）；
/// 2. 解析大能实体并 emit 本事件；网络层不消费库存。
///
/// 本事件由 `dying_elder_give_dan_system` 在 Update 阶段消费，执行：
/// - 权威重验 instance/template/effect，并按 EventReader 顺序消费库存；
/// - 大能 qi_current 增加 ItemRegistry 中的 canonical qi_gain；
/// - QiTransfer{TradeDan} 审计记录；
/// - DyingElderState 更新（Plea/Recovering → Recovering{n+1}）；
/// - 若 n+1 >= DYING_ELDER_DAN_THRESHOLD → 触发结局判定。
#[derive(Debug, Clone, valence::prelude::Event)]
pub struct GiveDanToElderIntent {
    /// 执行给丹操作的玩家实体。
    pub player: Entity,
    /// 垂死大能 ECS 实体。
    pub elder: Entity,
    /// 待权威校验并消费的回元丹 instance_id（用于库存事务与 QiTransfer 账户标识）。
    pub pill_instance_id: u64,
}

/// 给丹事务完成后发出的权威事件。只有实例/模板/effect 校验、库存消费和真元提交全部
/// 成功才会 emit；Redis/S2C 必须监听本事件，不能监听可能被拒绝的原始 intent。
#[derive(Debug, Clone, valence::prelude::Event)]
pub struct DyingElderDanAcceptedEvent {
    pub player: Entity,
    pub elder: Entity,
    pub pill_instance_id: u64,
    pub qi_gain: f64,
    /// 本次事务提交后的顺序计数；同 tick 多颗丹也必须保留逐笔顺序。
    pub dan_count: u32,
    /// 本次事务提交后的真元比例快照，避免下游读到同 tick 最后一颗丹的最终态。
    pub qi_fraction: f32,
}

// ── P1：夺舍事件 ──────────────────────────────────────────────────────────────

/// plan-dying-elder-v1 P1 — 大能翻脸夺舍时 emit 的事件。
///
/// ## 守恒约束
/// 事件只定位 elder/player；执行系统提交时必须重读双方 `Cultivation` 与
/// `DyingElderBlackboard::qi_max_cache`。`qi_transferred` / `qi_max_drain` 是旧协议兼容
/// 字段，retry 会填 0，均不得作为物理权威。
///
/// 下游系统（`dying_elder_betray_system`）消费本事件：
/// - 从玩家真实 `Cultivation.qi_current` 全额转入大能；
/// - 按大能真实 `qi_max_cache` 计算玩家 qi_max 永久减损；
/// - 大能 state → Dead { dead_by_betrayal: true }（夺舍力竭）。
#[derive(Debug, Clone, valence::prelude::Event)]
pub struct SoulSeizeEvent {
    /// 执行夺舍的大能实体。
    pub elder: Entity,
    /// 被夺舍的玩家实体。
    pub player: Entity,
    /// 旧事件兼容字段，非权威；执行系统忽略并重读 player Cultivation。
    pub qi_transferred: f64,
    /// 旧事件兼容字段，非权威；执行系统忽略并由 elder qi_max_cache 重算。
    pub qi_max_drain: f64,
}

/// 尚未成功提交的夺舍事务。`SoulSeizeEvent` 是单帧事件；非法/缺失组件导致提交失败时，
/// 本组件保留 victim，下一 tick 由 retry system 重新发出事件，直到完整事务成功。
#[derive(Debug, Clone, Copy, Component, PartialEq, Eq)]
pub struct PendingSoulSeize {
    pub victim: Entity,
}

// ── P1：给丹系统 ──────────────────────────────────────────────────────────────

/// plan-dying-elder-v1 P1 — 消费 `GiveDanToElderIntent`，更新大能真元 + 状态。
///
/// ## 守恒执行顺序
/// 1. 校验 elder state、`Cultivation.qi_current` 与 cap；
/// 2. 权威重验玩家 inventory instance / `huiyuan_pill` template / ItemRegistry effect；
/// 3. 在本系统的 EventReader 顺序内真实消费丹，保证同 tick 第 5/6 颗只扣成功事务；
/// 4. cap 内写入大能，cap 外真实转入稳定 overflow；ledger 失败则完整 qi 暂存大能；
/// 5. 同步 Blackboard mirror、记录 QiTransfer，并 emit `DyingElderDanAcceptedEvent`；
/// 6. 更新 `DyingElderState`：
///    - Plea → Recovering { dan_received: 1 }
///    - Recovering { n } → Recovering { n+1 }
///    - n+1 >= DYING_ELDER_DAN_THRESHOLD → 触发结局判定
/// 7. 结局判定：`betray_roll` → Betrayal 或 Dead { dead_by_betrayal: false }
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn dying_elder_give_dan_system(
    mut commands: Commands,
    mut intents: EventReader<GiveDanToElderIntent>,
    mut elders: Query<
        (
            &mut DyingElderBlackboard,
            &mut DyingElderState,
            &mut Cultivation,
        ),
        (With<NpcMarker>, Without<ClientMarker>),
    >,
    mut player_inventories: Query<&mut PlayerInventory, With<ClientMarker>>,
    item_registry: Option<Res<ItemRegistry>>,
    player_renowns: Query<&Renown, With<ClientMarker>>,
    mut soul_seize_events: EventWriter<SoulSeizeEvent>,
    mut accepted_events: EventWriter<DyingElderDanAcceptedEvent>,
    mut qi_transfer_events: EventWriter<QiTransfer>,
    mut qi_account: Option<ResMut<WorldQiAccount>>,
    game_tick: Option<Res<GameTick>>,
) {
    let tick = game_tick.as_deref().map(|t| t.0 as u64).unwrap_or(0);

    for intent in intents.read() {
        let Ok((mut bb, mut state, mut cultivation)) = elders.get_mut(intent.elder) else {
            tracing::warn!(
                "[bong][dying_elder] give_dan_system: elder entity {:?} missing blackboard/state/cultivation",
                intent.elder
            );
            continue;
        };

        // 校验大能当前状态是否可接丹（只在 Plea 或 Recovering 状态接受）。必须在
        // inventory 消费前执行，确保同 tick 已达阈值后的后续 intent 不再扣物品。
        let (dan_received, first_dan) = match *state {
            DyingElderState::Plea => (0, true),
            DyingElderState::Recovering { dan_received }
                if dan_received < DYING_ELDER_DAN_THRESHOLD =>
            {
                (dan_received, false)
            }
            DyingElderState::Recovering { .. }
            | DyingElderState::Betrayal
            | DyingElderState::Dead { .. } => {
                tracing::debug!(
                    "[bong][dying_elder] give_dan_system: elder {:?} in {:?}, rejecting dan",
                    intent.elder,
                    *state
                );
                continue;
            }
        };

        // Cultivation.qi_current 是物理权威；Blackboard 只做 encounter/UI 镜像。
        let qi_before = cultivation.qi_current;
        let qi_cap = bb.qi_max_cache * 1.5;
        if !qi_before.is_finite()
            || qi_before < 0.0
            || !bb.qi_max_cache.is_finite()
            || bb.qi_max_cache < 0.0
            || !qi_cap.is_finite()
        {
            tracing::warn!(
                "[bong][dying_elder] give_dan_system: invalid qi state elder={:?} current={} max_cache={} cap={}; keep inventory/state unchanged",
                intent.elder,
                qi_before,
                bb.qi_max_cache,
                qi_cap,
            );
            continue;
        }

        // 网络层只做 preflight；这里按 EventReader 顺序权威重验并消费 inventory。
        let Some(item_registry) = item_registry.as_deref() else {
            tracing::warn!(
                "[bong][dying_elder] give_dan_system: ItemRegistry missing; keep pill/state unchanged"
            );
            continue;
        };
        let Ok(mut inventory) = player_inventories.get_mut(intent.player) else {
            tracing::warn!(
                "[bong][dying_elder] give_dan_system: player {:?} missing PlayerInventory; keep state unchanged",
                intent.player,
            );
            continue;
        };
        let Some(pill) = inventory_item_by_instance_borrow(&inventory, intent.pill_instance_id)
        else {
            tracing::warn!(
                "[bong][dying_elder] give_dan_system: pill instance {} missing for player {:?}; keep state unchanged",
                intent.pill_instance_id,
                intent.player,
            );
            continue;
        };
        if pill.template_id != "huiyuan_pill" {
            tracing::warn!(
                "[bong][dying_elder] give_dan_system: instance {} template '{}' is not huiyuan_pill; keep inventory/state unchanged",
                intent.pill_instance_id,
                pill.template_id,
            );
            continue;
        }
        let Some(ItemEffect::QiRecovery { amount: qi_gain }) = item_registry
            .get("huiyuan_pill")
            .and_then(|template| template.effect.as_ref())
        else {
            tracing::warn!(
                "[bong][dying_elder] give_dan_system: huiyuan_pill registry effect is not QiRecovery; keep inventory/state unchanged"
            );
            continue;
        };
        let qi_gain = *qi_gain;
        let fallback_qi_after = qi_before + qi_gain;
        let fallback_qi_gain = fallback_qi_after - qi_before;
        if !qi_gain.is_finite()
            || qi_gain <= 0.0
            || !fallback_qi_after.is_finite()
            || (fallback_qi_gain - qi_gain).abs() > QI_EPSILON
        {
            tracing::warn!(
                "[bong][dying_elder] give_dan_system: invalid canonical pill qi={} or unrepresentable fallback sum={} actual_gain={} elder={:?}; keep inventory/state unchanged",
                qi_gain,
                fallback_qi_after,
                fallback_qi_gain,
                intent.elder,
            );
            continue;
        }
        if let Err(error) = consume_item_instance_once(&mut inventory, intent.pill_instance_id) {
            tracing::warn!(
                "[bong][dying_elder] give_dan_system: consume pill {} failed for player {:?}: {error}; keep state unchanged",
                intent.pill_instance_id,
                intent.player,
            );
            continue;
        }

        // 首颗丹的声名调整也延迟到消费成功后，前置拒绝不得污染 encounter。
        if first_dan {
            if let Ok(renown) = player_renowns.get(intent.player) {
                bb.apply_renown_adjustment(renown.fame);
                tracing::debug!(
                    "[bong][dying_elder] give_dan_system: player {:?} fame={} applied renown adjustment → betray_prob={:.3}",
                    intent.player,
                    renown.fame,
                    bb.betray_probability,
                );
            }
        }

        // ── 守恒：优先把 cap 内真元写入大能；cap 外部分真实进入稳定 overflow ──
        let room = (qi_cap - qi_before).max(0.0);
        let capped_qi_added = qi_gain.min(room);
        let excess_qi = (qi_gain - capped_qi_added).max(0.0);

        let pill_account =
            QiAccountId::container(format!("hui_yuan_pill:{}", intent.pill_instance_id));
        let elder_account = QiAccountId::npc(format!("dying_elder:{}", intent.elder.to_bits()));
        let mut actual_qi_added = capped_qi_added;

        if excess_qi > QI_EPSILON {
            let overflow_account = dying_elder_dan_excess_account();
            let overflow_result = match qi_account.as_deref_mut() {
                Some(account) => transfer_external_qi_to_ledger(
                    account,
                    pill_account.clone(),
                    overflow_account,
                    excess_qi,
                    QiTransferReason::TradeDan,
                )
                .map_err(|error| error.to_string()),
                None => Err("WorldQiAccount missing".to_string()),
            };

            match overflow_result {
                Ok(Some(transfer)) => {
                    qi_transfer_events.send(transfer);
                }
                Ok(None) => {}
                Err(error) => {
                    // 丹已由本事务消费，无法回滚 item。ledger 不可用时把 full qi 留在
                    // Cultivation 物理权威中（允许临时越 cap），绝不丢弃 cap 外部分。
                    actual_qi_added = qi_gain;
                    tracing::warn!(
                        "[bong][dying_elder] give_dan_system: excess qi ledger failed elder={:?} excess={} error={}; keep full pill qi in elder",
                        intent.elder,
                        excess_qi,
                        error,
                    );
                }
            }
        }

        cultivation.qi_current = qi_before + actual_qi_added;
        bb.qi_current = cultivation.qi_current;

        // ── 守恒：进入大能物理池的部分用 TradeDan audit + event 留痕 ─────────
        if actual_qi_added > 0.0 {
            let transfer = QiTransfer {
                from: pill_account,
                to: elder_account,
                amount: actual_qi_added,
                reason: QiTransferReason::TradeDan,
            };
            if let Some(ref mut account) = qi_account {
                account.push_transfer_audit(transfer.clone());
            }
            qi_transfer_events.send(transfer);
        }

        // ── 状态更新：dan_received + 1 → 检查是否达到阈值 ───────────────────
        let new_dan_received = dan_received + 1;
        tracing::info!(
            "[bong][dying_elder] give_dan_system: elder {:?} received dan #{}/{} (qi_gain={:.2} actual={:.2}) tick={tick}",
            intent.elder,
            new_dan_received,
            DYING_ELDER_DAN_THRESHOLD,
            qi_gain,
            actual_qi_added,
        );

        if new_dan_received >= DYING_ELDER_DAN_THRESHOLD {
            // ── 结局判定 ──────────────────────────────────────────────────────
            // 用 (player entity bits ^ elder entity bits ^ tick) 作为确定性 seed
            let seed = intent.player.to_bits()
                ^ intent.elder.to_bits()
                ^ tick.wrapping_mul(0x517C_C1B7_2722_0A95);
            let betrayal = betray_roll(bb.betray_probability, seed);

            if betrayal {
                // 翻脸夺舍
                *state = DyingElderState::Betrayal;

                // 先留下可重试权威，再发本帧事件。若 betray system 因非法/缺失组件
                // fail-closed，PendingSoulSeize 会在下一 tick 重新驱动同一事务。
                commands.entity(intent.elder).insert(PendingSoulSeize {
                    victim: intent.player,
                });

                // qi_max_drain 永久减损量（= qi_max_cache × DYING_ELDER_SOUL_SEIZE_RATIO）
                let qi_max_drain = bb.qi_max_cache * DYING_ELDER_SOUL_SEIZE_RATIO;

                soul_seize_events.send(SoulSeizeEvent {
                    elder: intent.elder,
                    player: intent.player,
                    // 兼容字段不携权威数值，betray system 必须重读双方组件。
                    qi_transferred: 0.0,
                    qi_max_drain: 0.0,
                });

                tracing::info!(
                    "[bong][dying_elder] give_dan_system: BETRAYAL! elder {:?} → player {:?} soul seize qi_max_drain={:.2}",
                    intent.elder,
                    intent.player,
                    qi_max_drain,
                );
            } else {
                // 守信自裁
                *state = DyingElderState::Dead {
                    dead_by_betrayal: false,
                };
                tracing::info!(
                    "[bong][dying_elder] give_dan_system: HONORABLE DEATH elder {:?} self-destructs after {new_dan_received} dan",
                    intent.elder,
                );
            }
        } else {
            *state = DyingElderState::Recovering {
                dan_received: new_dan_received,
            };
        }

        let qi_fraction = if bb.qi_max_cache > 0.0 {
            (bb.qi_current / bb.qi_max_cache).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
        accepted_events.send(DyingElderDanAcceptedEvent {
            player: intent.player,
            elder: intent.elder,
            pill_instance_id: intent.pill_instance_id,
            qi_gain,
            dan_count: new_dan_received,
            qi_fraction,
        });
    }
}

/// 每 tick 重发上一帧未完成的夺舍事务。只为仍处于 Betrayal 的大能发事件；成功路径
/// 由 `dying_elder_betray_system` 移除 [`PendingSoulSeize`]，因此不会重复转移。
#[allow(clippy::type_complexity)]
pub(crate) fn dying_elder_retry_pending_soul_seize_system(
    pending: Query<
        (Entity, &PendingSoulSeize, &DyingElderState),
        (With<NpcMarker>, Without<ClientMarker>),
    >,
    mut events: EventWriter<SoulSeizeEvent>,
) {
    for (elder, pending, state) in &pending {
        if !matches!(*state, DyingElderState::Betrayal) {
            continue;
        }
        events.send(SoulSeizeEvent {
            elder,
            player: pending.victim,
            // 生产提交始终从双方 Cultivation 读取权威值；这两个字段仅保留旧事件契约。
            qi_transferred: 0.0,
            qi_max_drain: 0.0,
        });
    }
}

// ── P1：夺舍执行系统 ─────────────────────────────────────────────────────────

/// plan-dying-elder-v1 P1 — 消费 `SoulSeizeEvent`，执行真元夺舍 + qi_max 永久减损。
///
/// ## 守恒执行
/// 1. 读取玩家 `Cultivation.qi_current`（真实当前值）；
/// 2. player.qi_current → 0（全额转移给大能）；
/// 3. 大能 bb.qi_current += 实际转移量；
/// 4. 向 WorldQiAccount push QiTransfer{SoulSeize} 审计记录；
/// 5. player.qi_max 减去由 elder qi_max_cache 重算的永久容量 debuff（**不走** QiTransfer）；
/// 6. 大能 state → Dead { dead_by_betrayal: true }（夺舍力竭）。
#[allow(clippy::type_complexity)]
pub(crate) fn dying_elder_betray_system(
    mut commands: Commands,
    mut events: EventReader<SoulSeizeEvent>,
    mut elders: Query<
        (&mut DyingElderBlackboard, &mut DyingElderState),
        (With<NpcMarker>, Without<ClientMarker>),
    >,
    mut cultivations: Query<&mut Cultivation>,
    mut qi_transfer_events: EventWriter<QiTransfer>,
    mut qi_account: Option<ResMut<WorldQiAccount>>,
) {
    for ev in events.read() {
        let Ok((mut bb, mut state)) = elders.get_mut(ev.elder) else {
            tracing::warn!(
                "[bong][dying_elder] betray_system: elder entity {:?} not found",
                ev.elder
            );
            continue;
        };

        // 确认大能仍在 Betrayal 状态（避免重复处理）
        if !matches!(*state, DyingElderState::Betrayal) {
            continue;
        }

        // 一次取得玩家与大能两个物理权威，任一缺失都 fail-closed，避免半边先扣后另一边失败。
        let Ok([mut elder_cultivation, mut cultivation]) =
            cultivations.get_many_mut([ev.elder, ev.player])
        else {
            tracing::warn!(
                "[bong][dying_elder] betray_system: elder {:?} or player {:?} missing Cultivation; keep Betrayal pending",
                ev.elder,
                ev.player,
            );
            continue;
        };
        // 在任何组件/审计写入前先把完整新状态算完并校验。NaN/Inf/负真元或求和溢出
        // 一律 fail-closed，保持 Betrayal 供修复后重试，绝不先清玩家再毒化大能。
        let elder_qi_before = elder_cultivation.qi_current;
        let player_qi_before = cultivation.qi_current;
        let elder_qi_after = elder_qi_before + player_qi_before;
        if !elder_qi_before.is_finite()
            || elder_qi_before < 0.0
            || !player_qi_before.is_finite()
            || player_qi_before < 0.0
            || !elder_qi_after.is_finite()
        {
            tracing::warn!(
                "[bong][dying_elder] betray_system: invalid qi elder={:?} elder_qi={} player={:?} player_qi={} sum={}; keep unchanged",
                ev.elder,
                elder_qi_before,
                ev.player,
                player_qi_before,
                elder_qi_after,
            );
            continue;
        }

        // qi_max debuff 是容量变化，但也必须与真元提交同一原子边界，避免非法容量输入
        // 在玩家真元已清零后才产生 NaN/Inf。
        let frozen_qi_max = cultivation.qi_max_frozen.unwrap_or(0.0);
        let qi_max_drain = bb.qi_max_cache * DYING_ELDER_SOUL_SEIZE_RATIO;
        let raw_qi_max_after = cultivation.qi_max - qi_max_drain;
        if !bb.qi_max_cache.is_finite()
            || bb.qi_max_cache < 0.0
            || !cultivation.qi_max.is_finite()
            || cultivation.qi_max < 0.0
            || !frozen_qi_max.is_finite()
            || frozen_qi_max < 0.0
            || !qi_max_drain.is_finite()
            || qi_max_drain < 0.0
            || !raw_qi_max_after.is_finite()
        {
            tracing::warn!(
                "[bong][dying_elder] betray_system: invalid capacity elder={:?} cache={} player={:?} qi_max={} frozen={} drain={}; keep unchanged",
                ev.elder,
                bb.qi_max_cache,
                ev.player,
                cultivation.qi_max,
                frozen_qi_max,
                qi_max_drain,
            );
            continue;
        }
        let player_qi_max_after = raw_qi_max_after.max(0.0);

        // 所有可失败计算已完成；从这里开始一次提交双方物理权威、mirror 与状态。
        cultivation.qi_current = 0.0;
        cultivation.qi_max = player_qi_max_after;
        elder_cultivation.qi_current = elder_qi_after;
        bb.qi_current = elder_qi_after;

        // ── 守恒：QiTransfer{SoulSeize} 审计（从玩家到大能）────────────────────
        if player_qi_before > 0.0 {
            let player_account = QiAccountId::player(format!("entity:{}", ev.player.to_bits()));
            let elder_account = QiAccountId::npc(format!("dying_elder:{}", ev.elder.to_bits()));
            let transfer = QiTransfer {
                from: player_account,
                to: elder_account,
                amount: player_qi_before,
                reason: QiTransferReason::SoulSeize,
            };
            if let Some(ref mut account) = qi_account {
                account.push_transfer_audit(transfer.clone());
            }
            qi_transfer_events.send(transfer);
        }

        // ── 大能力竭死亡 ───────────────────────────────────────────────────────
        *state = DyingElderState::Dead {
            dead_by_betrayal: true,
        };
        commands.entity(ev.elder).remove::<PendingSoulSeize>();

        tracing::info!(
            "[bong][dying_elder] betray_system: player {:?} soul seized! qi_transferred={:.2} qi_max_drain={:.2}",
            ev.player,
            player_qi_before,
            qi_max_drain,
        );
    }
}

// ── Bevy 注册 P1 ──────────────────────────────────────────────────────────────

/// Bevy 注册：P1 给丹系统 + 夺舍系统 + spawn apply 系统 + 相关事件。
pub fn register_p1(app: &mut App) {
    app.add_event::<GiveDanToElderIntent>();
    app.add_event::<DyingElderDanAcceptedEvent>();
    app.add_event::<SoulSeizeEvent>();
    app.add_event::<QiTransfer>();
    // Bug2 修复：注册 DyingElderAppearedEvent（entity 创建后 emit，携带真实 entity idx）
    app.add_event::<DyingElderAppearedEvent>();
    // spawn apply：消费 P0 emit 的 DyingElderSpawnRequest，真正创建大能 entity。
    // 在 give_dan_system 之前注册（ordering 保证：同帧内先 spawn 再允许交互，但遭遇流程不依赖同帧）
    app.add_systems(Update, dying_elder_apply_spawn_system);
    // 固定 retry → give → betray：旧 pending 先重发，本帧新 give 再判定，最后统一提交。
    app.add_systems(
        Update,
        (
            dying_elder_retry_pending_soul_seize_system.before(dying_elder_give_dan_system),
            dying_elder_give_dan_system
                .after(crate::network::client_request_handler::handle_client_request_payloads)
                .after(dying_elder_retry_pending_soul_seize_system),
            dying_elder_betray_system.after(dying_elder_give_dan_system),
        ),
    );
}

// ── 纯函数工具 ────────────────────────────────────────────────────────────────

/// 用 splitmix64 生成 [0, 1) 的 f64（用于 betray 判定 roll）。
/// 返回 `(value, next_seed)`，调用方链式更新 seed。
pub fn splitmix64_f64(seed: u64) -> (f64, u64) {
    let next = seed
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(0x6C62_272E_07BB_0142);
    let value = (next % 1_000_000) as f64 / 1_000_000.0;
    (value, next)
}

/// 判断 betray_probability roll 是否命中（roll < betray_probability → 翻脸）。
/// 用 splitmix64 保证确定性（测试可重现）。
pub fn betray_roll(betray_probability: f64, seed: u64) -> bool {
    let (roll, _) = splitmix64_f64(seed);
    roll < betray_probability
}

// ── P2：死亡标记 Component ─────────────────────────────────────────────────────

/// 垂死大能已处理死亡（避免 `DyingElderDeathSystem` 重复触发 qi release + loot）。
///
/// 设计：ECS Component（非 Event）—— 死亡结算是 push 型，在 Dead 态被检测到后
/// 立即插入本 Component，后续 tick 直接跳过该 entity，直到 entity 被 despawn。
#[derive(Debug, Clone, Copy, Component)]
pub struct DyingElderDeathProcessed;

/// 垂死大能死亡叙事已广播。
///
/// 叙事发送与真元/loot 结算是两个独立副作用：结算失败时允许下一 tick 重试，
/// 但死亡叙事只应对外广播一次。
#[derive(Debug, Clone, Copy, Component)]
pub struct DyingElderDeathBroadcast;

// ── P2：offered_skill_id → scroll template_id 映射 ────────────────────────────

/// 将地阶功法 skill_id 映射到对应的功法残卷 template_id。
///
/// 用于 DyingElderDeathSystem 生成 loot（大能传承残卷掉落）。
///
/// ## 映射关系（与 EARTH_GRADE_TECHNIQUE_POOL 一一对应）
/// - `woliu.heart` → `scroll_woliu_heart`（无流心诀，woliu_scrolls.toml:69）
/// - `woliu.turbulence_burst` → `scroll_woliu_turbulence_burst`（无流湍爆，woliu_scrolls.toml:134）
/// - `anqi.echo_fractal` → `scroll_anqi_echo_fractal`（暗器回声裂变，anqi.toml 新增）
/// - `sword_path.heaven_gate` → `scroll_sword_heaven_gate`（剑道天门禁忌，sword_materials.toml:176）
///
/// 返回 `None` 表示未知 skill_id（测试中已锁全部 4 条映射，运行时 warn + 跳过掉落）。
pub fn skill_id_to_scroll_template(skill_id: &str) -> Option<&'static str> {
    match skill_id {
        "woliu.heart" => Some("scroll_woliu_heart"),
        "woliu.turbulence_burst" => Some("scroll_woliu_turbulence_burst"),
        "anqi.echo_fractal" => Some("scroll_anqi_echo_fractal"),
        "sword_path.heaven_gate" => Some("scroll_sword_heaven_gate"),
        _ => None,
    }
}

// ── P2：DyingElderDrainSystem ─────────────────────────────────────────────────

/// plan-dying-elder-v1 P2 — 每 tick 对 Plea/Recovering 态大能执行坍缩渊真元消耗。
///
/// ## 守恒执行
/// 1. 用生产 `Cultivation` 物理权威计算本 tick 扣减量；
/// 2. 先把 `actual_drain` 真实转入 `rift:<zone_name>` ledger；
/// 3. ledger 成功后才扣 `Cultivation.qi_current`，并同步 Blackboard mirror；
/// 4. `qi_current <= 0` → state 变 `Dead { dead_by_betrayal: false }`（自然力竭）；
///
/// **注意**：本系统仅针对 Plea/Recovering 状态；Betrayal/Dead 态不受此系统管辖。
#[allow(clippy::type_complexity)]
pub(crate) fn dying_elder_drain_system(
    mut elders: Query<
        (
            Entity,
            &mut DyingElderBlackboard,
            &mut DyingElderState,
            &mut Cultivation,
        ),
        (
            With<NpcMarker>,
            Without<ClientMarker>,
            Without<DyingElderDeathProcessed>,
        ),
    >,
    zones: Option<Res<ZoneRegistry>>,
    mut qi_transfer_events: EventWriter<QiTransfer>,
    mut qi_account: Option<ResMut<WorldQiAccount>>,
    game_tick: Option<Res<GameTick>>,
) {
    let tick = game_tick.as_deref().map(|t| t.0 as u64).unwrap_or(0);
    let Some(zones) = zones else { return };

    for (entity, mut bb, mut state, mut cultivation) in &mut elders {
        // 只在 Plea / Recovering 状态 drain（Betrayal/Dead 不走此系统）
        match *state {
            DyingElderState::Plea | DyingElderState::Recovering { .. } => {}
            DyingElderState::Betrayal | DyingElderState::Dead { .. } => continue,
        }

        // 查找大能所在 TSY zone（用 home_zone 名称精确查找）
        let Some(zone) = zones.zones.iter().find(|z| z.name == bb.home_zone) else {
            tracing::warn!(
                "[bong][dying_elder] drain_system: elder {:?} home_zone '{}' not found in registry tick={tick}",
                entity,
                bb.home_zone
            );
            continue;
        };

        if !cultivation.qi_current.is_finite() {
            tracing::warn!(
                "[bong][dying_elder] drain_system: elder {:?} invalid Cultivation.qi_current={}; keep unchanged",
                entity,
                cultivation.qi_current,
            );
            continue;
        }

        let drain = compute_drain_per_tick(zone, &cultivation);
        if !drain.is_finite() {
            tracing::warn!(
                "[bong][dying_elder] drain_system: elder {:?} computed non-finite drain={drain}; keep unchanged",
                entity,
            );
            continue;
        }
        if drain <= 0.0 {
            bb.qi_current = cultivation.qi_current;
            continue;
        }

        let before_qi = cultivation.qi_current.max(0.0);
        let actual_drain = drain.min(before_qi);

        // ── 守恒：先真实 credit rift，失败时双组件均不扣、下一 tick 重试 ──────
        if actual_drain > QI_EPSILON {
            let elder_account = QiAccountId::npc(format!("dying_elder:{}", entity.to_bits()));
            let rift_account = QiAccountId::rift(bb.home_zone.clone());
            let Some(account) = qi_account.as_deref_mut() else {
                tracing::warn!(
                    "[bong][dying_elder] drain_system: WorldQiAccount missing for elder {:?}; keep qi unchanged",
                    entity,
                );
                continue;
            };
            match transfer_external_qi_to_ledger(
                account,
                elder_account,
                rift_account,
                actual_drain,
                QiTransferReason::RiftCollapse,
            ) {
                Ok(Some(transfer)) => {
                    qi_transfer_events.send(transfer);
                }
                Ok(None) => {}
                Err(error) => {
                    tracing::warn!(
                        "[bong][dying_elder] drain_system: rift ledger failed elder={:?} amount={} error={error}; keep qi unchanged",
                        entity,
                        actual_drain,
                    );
                    continue;
                }
            }
        }

        cultivation.qi_current = (before_qi - actual_drain).max(0.0);
        bb.qi_current = cultivation.qi_current;

        // ── qi 耗尽 → 自然死亡 ──────────────────────────────────────────────
        if cultivation.qi_current <= 0.0 {
            *state = DyingElderState::Dead {
                dead_by_betrayal: false,
            };
            tracing::info!(
                "[bong][dying_elder] drain_system: elder {:?} qi exhausted → Dead(natural) tick={tick}",
                entity,
            );
        }
    }
}

// ── P2：DyingElderDeathSystem ─────────────────────────────────────────────────

/// plan-dying-elder-v1 P2 — 统一处理垂死大能死亡（自然力竭 / 守信自裁 / 翻脸夺舍力竭）。
///
/// ## 两条死亡路线
/// - **守信 / 自然死亡**（`dead_by_betrayal = false`）：大能守约传承自裁 or 真元耗尽，
///   zone spirit_qi 瞬时跃升（全额 qi release），loot 质量较好（secondary_honorable 附加池）。
/// - **背叛路线**（`dead_by_betrayal = true`）：夺舍后力竭，loot 质量稍差（secondary_betrayal 池）。
///
/// ## 守恒执行
/// 1. `qi_release_to_zone(amount=elder.qi_current, from=npc:dying_elder:<id>, zone=zone:<home_zone>)`
///    → zone spirit_qi 瞬时跃升（化虚级 ~500 真元直接注入负灵域 → 区域灵气快速复苏）；
/// 2. 更新 ZoneRegistry 中对应 zone 的 spirit_qi；
/// 3. 生成 loot：
///    a. 地阶功法残卷（by offered_skill_id → scroll template_id）；
///    b. 通过 loot pool 生成附加掉落（dead_by_betrayal 分档）；
/// 4. 插入 `DyingElderDeathProcessed`（避免下一 tick 重复处理）。
///
/// **注意**：本系统在 `Update` 阶段运行，elder entity 不在本帧 despawn（由 NPC lifecycle 处理）。
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn dying_elder_death_system(
    mut commands: Commands,
    mut elders: Query<
        (
            Entity,
            &mut DyingElderBlackboard,
            &DyingElderState,
            &mut Cultivation,
        ),
        (
            With<NpcMarker>,
            Without<ClientMarker>,
            Without<DyingElderDeathProcessed>,
        ),
    >,
    mut zones: Option<ResMut<ZoneRegistry>>,
    item_registry: Option<Res<ItemRegistry>>,
    mut allocator: Option<ResMut<InventoryInstanceIdAllocator>>,
    mut loot_registry: Option<ResMut<DroppedLootRegistry>>,
    mut qi_transfer_events: EventWriter<QiTransfer>,
    mut qi_account: Option<ResMut<WorldQiAccount>>,
    game_tick: Option<Res<GameTick>>,
) {
    let tick = game_tick.as_deref().map(|t| t.0 as u64).unwrap_or(0);

    for (entity, mut bb, state, mut cultivation) in &mut elders {
        let dead_by_betrayal = match *state {
            DyingElderState::Dead { dead_by_betrayal } => dead_by_betrayal,
            _ => continue, // 只处理 Dead 态
        };

        // ── 守恒：qi_release_to_zone 全额释放大能真元 ────────────────────────
        if !cultivation.qi_current.is_finite() {
            tracing::warn!(
                "[bong][dying_elder] death_system: invalid Cultivation.qi_current={} for elder {:?}; keep pending for retry",
                cultivation.qi_current,
                entity,
            );
            continue;
        }
        let release_amount = cultivation.qi_current.max(0.0);
        if release_amount > 0.0 && qi_account.is_none() {
            tracing::warn!(
                "[bong][dying_elder] death_system: positive release={} but WorldQiAccount missing for elder {:?}; keep zone/components/marker unchanged",
                release_amount,
                entity,
            );
            continue;
        }
        let elder_account = QiAccountId::npc(format!("dying_elder:{}", entity.to_bits()));
        let zone_account = QiAccountId::zone(bb.home_zone.clone());

        // 只有真实存在的 zone 才能接收 accepted 腿；缺资源或 home_zone 漂移时，
        // 全量进入 overflow，绝不根据虚构浓度制造无法写回世界状态的 accepted。
        let zone_current_qi = zones
            .as_ref()
            .and_then(|zr| zr.zones.iter().find(|z| z.name == bb.home_zone))
            .map(|z| z.spirit_qi * QI_ZONE_UNIT_CAPACITY);

        let outcome = if release_amount > 0.0 {
            match qi_release_to_zone(
                release_amount,
                elder_account.clone(),
                zone_account.clone(),
                zone_current_qi.unwrap_or(QI_ZONE_UNIT_CAPACITY),
                QI_ZONE_UNIT_CAPACITY,
            ) {
                Ok(outcome) => Some(outcome),
                Err(e) => {
                    tracing::warn!(
                        "[bong][dying_elder] death_system: qi_release_to_zone error for elder {:?}: {e:?}",
                        entity
                    );
                    continue;
                }
            }
        } else {
            None
        };

        // overflow 没有 ZoneRegistry 字段承载，必须在任何 zone/组件提交前先真实入账。
        // 缺账本或 transfer 失败时，bb/cultivation/zone/processed 全量保持原样重试。
        let mut overflow_transfer = None;
        if let Some(ref outcome) = outcome {
            if outcome.overflow > QI_EPSILON {
                let Some(account) = qi_account.as_deref_mut() else {
                    tracing::warn!(
                        "[bong][dying_elder] death_system: overflow={} but WorldQiAccount missing for elder {:?}; keep pending",
                        outcome.overflow,
                        entity,
                    );
                    continue;
                };
                match transfer_external_qi_to_ledger(
                    account,
                    elder_account.clone(),
                    dying_elder_release_overflow_account(),
                    outcome.overflow,
                    QiTransferReason::ReleaseToZone,
                ) {
                    Ok(transfer) => overflow_transfer = transfer,
                    Err(error) => {
                        tracing::warn!(
                            "[bong][dying_elder] death_system: overflow ledger failed elder={:?} amount={} error={error}; keep pending",
                            entity,
                            outcome.overflow,
                        );
                        continue;
                    }
                }
            }
        }

        // 到这里所有可失败的守恒步骤都已完成，开始提交 field-authority 与双组件状态。
        if let Some(outcome) = outcome {
            if let Some(ref mut zr) = zones {
                if let Some(zone) = zr.zones.iter_mut().find(|z| z.name == bb.home_zone) {
                    zone.spirit_qi = outcome.zone_after / QI_ZONE_UNIT_CAPACITY;
                }
            }

            if let Some(transfer) = overflow_transfer {
                qi_transfer_events.send(transfer);
            }
            if let Some(transfer) = outcome.transfer {
                if let Some(ref mut account) = qi_account {
                    account.push_transfer_audit(transfer.clone());
                }
                qi_transfer_events.send(transfer);
            }
            tracing::info!(
                "[bong][dying_elder] death_system: elder {:?} released qi={:.2} to zone '{}' overflow={:.2} zone_after={:.4} tick={tick}",
                entity,
                outcome.accepted,
                bb.home_zone,
                outcome.overflow,
                outcome.zone_after / QI_ZONE_UNIT_CAPACITY,
            );
        }

        cultivation.qi_current = 0.0;
        bb.qi_current = cultivation.qi_current;

        // ── loot 生成 ──────────────────────────────────────────────────────
        let drop_pos: [f64; 3] = [bb.home_pos.x, bb.home_pos.y, bb.home_pos.z];
        let dim = DimensionKind::Tsy;

        if let (Some(item_reg), Some(allocator), Some(loot_reg)) = (
            item_registry.as_deref(),
            allocator.as_deref_mut(),
            loot_registry.as_deref_mut(),
        ) {
            // ── a. 地阶功法残卷（核心 loot，由 offered_skill_id 决定） ──────
            let scroll_template = skill_id_to_scroll_template(bb.offered_skill_id);
            if let Some(template_id) = scroll_template {
                if let Some(template) = item_reg.get(template_id) {
                    match allocator.next_id() {
                        Ok(instance_id) => {
                            let scroll = ItemInstance {
                                instance_id,
                                template_id: template.id.clone(),
                                display_name: template.display_name.clone(),
                                grid_w: template.grid_w,
                                grid_h: template.grid_h,
                                weight: template.base_weight,
                                rarity: template.rarity,
                                description: template.description.clone(),
                                stack_count: 1,
                                spirit_quality: template.spirit_quality_initial,
                                durability: 1.0,
                                freshness: None,
                                mineral_id: None,
                                charges: None,
                                forge_quality: None,
                                forge_color: None,
                                forge_side_effects: Vec::new(),
                                forge_achieved_tier: None,
                                alchemy: None,
                                lingering_owner_qi: None,
                            };
                            loot_reg.entries.insert(
                                instance_id,
                                crate::inventory::DroppedLootEntry {
                                    instance_id,
                                    source_container_id: format!(
                                        "dying_elder:{}",
                                        entity.to_bits()
                                    ),
                                    source_row: 0,
                                    source_col: 0,
                                    world_pos: drop_pos,
                                    dimension: dim,
                                    item: scroll,
                                },
                            );
                            tracing::info!(
                                "[bong][dying_elder] death_system: elder {:?} dropped scroll '{}' betrayal={dead_by_betrayal} tick={tick}",
                                entity,
                                template_id,
                            );
                        }
                        Err(e) => {
                            tracing::warn!(
                                "[bong][dying_elder] death_system: allocator overflow for scroll: {e}"
                            );
                        }
                    }
                } else {
                    tracing::warn!(
                        "[bong][dying_elder] death_system: scroll template '{}' not in ItemRegistry (offered_skill='{}')",
                        template_id,
                        bb.offered_skill_id,
                    );
                }
            } else {
                tracing::warn!(
                    "[bong][dying_elder] death_system: unknown offered_skill_id '{}' has no scroll mapping",
                    bb.offered_skill_id,
                );
            }

            // ── b. 附加掉落（dead_by_betrayal 分档） ──────────────────────────
            let secondary_pool_id = if dead_by_betrayal {
                "dying_elder_secondary_betrayal"
            } else {
                "dying_elder_secondary_honorable"
            };

            // 内联 loot pool 滚动（避免循环依赖 world::loot_pool，直接用 item_reg）
            // P2 简化：附加掉落统一从 jing_sui/jing_hun_yu 选一个，不依赖 LootPoolRegistry
            // （LootPoolRegistry 在生产路径通过 roll_loot_pool 使用，测试路径此处简化）
            let secondary_seed = entity
                .to_bits()
                .wrapping_add(tick)
                .wrapping_mul(0x517C_C1B7_2722_0A95);
            let (secondary_roll, _) = splitmix64_f64(secondary_seed);

            // 守信结局：60%机率掉 jing_sui（1-2个）+ 40%机率掉 jing_hun_yu（1个）
            // 背叛结局：80%机率掉 jing_sui（1个）+ 20%机率掉 jing_hun_yu（1个）
            let (secondary_template, secondary_count) = if !dead_by_betrayal {
                if secondary_roll < 0.60 {
                    ("jing_sui", 1u32)
                } else {
                    ("jing_hun_yu", 1)
                }
            } else if secondary_roll < 0.80 {
                ("jing_sui", 1u32)
            } else {
                ("jing_hun_yu", 1)
            };

            if let Some(template) = item_reg.get(secondary_template) {
                match allocator.next_id() {
                    Ok(instance_id) => {
                        let secondary_item = ItemInstance {
                            instance_id,
                            template_id: template.id.clone(),
                            display_name: template.display_name.clone(),
                            grid_w: template.grid_w,
                            grid_h: template.grid_h,
                            weight: template.base_weight,
                            rarity: template.rarity,
                            description: template.description.clone(),
                            stack_count: secondary_count,
                            spirit_quality: template.spirit_quality_initial,
                            durability: 1.0,
                            freshness: None,
                            mineral_id: None,
                            charges: None,
                            forge_quality: None,
                            forge_color: None,
                            forge_side_effects: Vec::new(),
                            forge_achieved_tier: None,
                            alchemy: None,
                            lingering_owner_qi: None,
                        };
                        loot_reg.entries.insert(
                            instance_id,
                            crate::inventory::DroppedLootEntry {
                                instance_id,
                                source_container_id: format!(
                                    "dying_elder_secondary:{}:{}",
                                    secondary_pool_id,
                                    entity.to_bits()
                                ),
                                source_row: 0,
                                source_col: 0,
                                world_pos: drop_pos,
                                dimension: dim,
                                item: secondary_item,
                            },
                        );
                        tracing::debug!(
                            "[bong][dying_elder] death_system: elder {:?} secondary loot '{}' ×{} pool={} tick={tick}",
                            entity,
                            secondary_template,
                            secondary_count,
                            secondary_pool_id,
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            "[bong][dying_elder] death_system: allocator overflow for secondary: {e}"
                        );
                    }
                }
            }
        }

        // ── 标记已处理（防重复） ──────────────────────────────────────────────
        commands.entity(entity).insert(DyingElderDeathProcessed);
    }
}

// ── Bevy 注册 P2 ──────────────────────────────────────────────────────────────

/// Bevy 注册：P2 drain 系统 + 死亡结算系统。
///
/// ## System ordering
/// - `drain_system` → `death_system`（drain 先转换 Dead 态，death 再结算）
/// - `betray_system`（P1 注册）→ `death_system`：夺舍判定先于死亡结算，
///   确保同 tick 内 Betrayal → Dead 的路径能在死亡系统之前完成状态写入。
pub fn register_p2(app: &mut App) {
    app.add_systems(
        Update,
        (
            // 同 tick 给丹与最后一口 drain 必须稳定为先给丹、后 drain。
            dying_elder_drain_system.after(dying_elder_give_dan_system),
            // 死亡系统在 drain 系统之后运行，确保同 tick 内 drain → Dead 的状态能立即结算；
            // 同时也在 betray_system 之后（betray_system 在 P1 注册，此处跨 register 声明 ordering）
            dying_elder_death_system
                .after(dying_elder_drain_system)
                .after(dying_elder_betray_system),
        ),
    );
}

// ── P3：Redis 叙事事件发送 ──────────────────────────────────────────────────────

/// plan-dying-elder-v1 P3 Bug2 修复 — 消费 `DyingElderAppearedEvent`，向 agent 广播「大能出现」叙事事件。
///
/// 改为监听 `DyingElderAppearedEvent`（由 `dying_elder_apply_spawn_system` 在 entity 创建后 emit），
/// 通过 `elder_id_query` 取 Valence `EntityId::get()`（MC protocol entity_id）填入 payload。
///
/// `betray_probability` 字段使用 blackboard 初始值（renown 调整在首次给丹时执行）。
pub(crate) fn dying_elder_p3_emit_appear_event_system(
    mut appeared_events: EventReader<DyingElderAppearedEvent>,
    elder_id_query: Query<&EntityId, (With<NpcMarker>, Without<ClientMarker>)>,
    redis: Option<Res<RedisBridgeResource>>,
) {
    let Some(redis) = redis else { return };

    for ev in appeared_events.read() {
        let Ok(entity_id) = elder_id_query.get(ev.elder) else {
            tracing::warn!(
                "[bong][dying_elder] P3 emit appear: no EntityId for elder {:?}, skipping Redis event",
                ev.elder
            );
            continue;
        };
        let protocol_id = entity_id.get();
        // qi_fraction = 1.0：大能刚出现时真元满值（DYING_ELDER_INITIAL_QI / DYING_ELDER_INITIAL_QI）
        let qi_fraction = 1.0_f32;
        let event = ElderEncounterEventV1 {
            event_id: None,
            zone_name: ev.zone_name.clone(),
            elder_entity_id: protocol_id, // MC protocol entity_id（非 ECS index）
            event_kind: ElderEncounterEventKindV1::Appeared,
            betray_probability: ev.blackboard.betray_probability,
            dan_count: 0,
            offered_skill_id: ev.blackboard.offered_skill_id.to_string(),
            qi_fraction,
            server_tick: ev.tick,
        };
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::ElderEncounterEvent(event));
        tracing::info!(
            "[bong][dying_elder] P3 emit appear event: entity={:?} protocol_id={} zone='{}' betray_prob={:.3} tick={}",
            ev.elder,
            protocol_id,
            ev.zone_name,
            ev.blackboard.betray_probability,
            ev.tick,
        );
    }
}

/// plan-dying-elder-v1 P3 — 检测新进入 Dead 态的大能，向 agent 广播死亡叙事事件。
///
/// 本系统用 `DyingElderDeathBroadcast` 独立保证叙事幂等，不依赖死亡结算是否成功。
///
/// 广播的 `event_kind` 按死亡原因区分：
/// - `dead_by_betrayal = false` → `DeadNatural`（自然力竭 / 守信自裁）
/// - `dead_by_betrayal = true` → `Betrayal`（翻脸夺舍力竭）
///
/// **注意**：被玩家直接击杀（外部 kill system emit `Dead{dead_by_betrayal:false}`）在游戏中
/// 目前无专属路径区分，暂时统一归为 `DeadNatural`；后续如引入外部击杀标记可分档。
#[allow(clippy::type_complexity)]
pub(crate) fn dying_elder_p3_emit_death_event_system(
    mut commands: Commands,
    elders: Query<
        (Entity, &EntityId, &DyingElderBlackboard, &DyingElderState),
        (
            With<NpcMarker>,
            Without<ClientMarker>,
            Without<DyingElderDeathBroadcast>,
        ),
    >,
    redis: Option<Res<RedisBridgeResource>>,
    game_tick: Option<Res<GameTick>>,
) {
    let tick = game_tick.as_deref().map(|t| t.0 as u64).unwrap_or(0);
    let Some(redis) = redis else { return };

    for (entity, entity_id, bb, state) in elders.iter() {
        let dead_by_betrayal = match *state {
            DyingElderState::Dead { dead_by_betrayal } => dead_by_betrayal,
            _ => continue,
        };

        let event_kind = if dead_by_betrayal {
            ElderEncounterEventKindV1::Betrayal
        } else {
            ElderEncounterEventKindV1::DeadNatural
        };

        let event = ElderEncounterEventV1 {
            event_id: None,
            zone_name: bb.home_zone.clone(),
            elder_entity_id: entity_id.get(), // MC protocol entity_id（非 ECS index）
            event_kind,
            betray_probability: 0.0,
            dan_count: 0,
            offered_skill_id: String::new(),
            qi_fraction: 0.0, // 死亡时真元耗尽
            server_tick: tick,
        };
        match redis
            .tx_outbound
            .send(RedisOutbound::ElderEncounterEvent(event))
        {
            Ok(()) => {
                commands.entity(entity).insert(DyingElderDeathBroadcast);
                tracing::info!(
                    "[bong][dying_elder] P3 emit death event: entity={:?} zone='{}' kind={:?} tick={tick}",
                    entity,
                    bb.home_zone,
                    event_kind,
                );
            }
            Err(error) => {
                tracing::warn!(
                    "[bong][dying_elder] P3 death event send failed for entity {:?}; retry next tick: {error}",
                    entity,
                );
            }
        }
    }
}

/// plan-dying-elder-v1 P3 — 向 agent 广播「大能收丹」叙事事件（Recovering 态每次给丹后触发）。
///
/// 本系统只消费 `DyingElderDanAcceptedEvent`。原始 intent 被拒绝或同 tick 超阈值时
/// 不会产生假广播；事件内逐笔快照避免多颗丹都读到最终 state。
///
/// 发送 `DanReceived` 事件，携带当前大能 `dan_count`，供 agent 生成进度叙事。
#[allow(clippy::type_complexity)]
pub(crate) fn dying_elder_p3_emit_dan_received_event_system(
    mut accepted_events: EventReader<DyingElderDanAcceptedEvent>,
    elders: Query<
        (Entity, &EntityId, &DyingElderBlackboard, &DyingElderState),
        (With<NpcMarker>, Without<ClientMarker>),
    >,
    redis: Option<Res<RedisBridgeResource>>,
    game_tick: Option<Res<GameTick>>,
) {
    let tick = game_tick.as_deref().map(|t| t.0 as u64).unwrap_or(0);
    let Some(redis) = redis else { return };

    for accepted in accepted_events.read() {
        let Ok((entity, entity_id, bb, _state)) = elders.get(accepted.elder) else {
            continue;
        };
        let event = ElderEncounterEventV1 {
            event_id: None,
            zone_name: bb.home_zone.clone(),
            elder_entity_id: entity_id.get(), // MC protocol entity_id（非 ECS index）
            event_kind: ElderEncounterEventKindV1::DanReceived,
            betray_probability: 0.0,
            dan_count: accepted.dan_count,
            offered_skill_id: bb.offered_skill_id.to_string(),
            qi_fraction: accepted.qi_fraction,
            server_tick: tick,
        };
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::ElderEncounterEvent(event));
        tracing::debug!(
            "[bong][dying_elder] P3 emit dan_received event: entity={:?} zone='{}' dan_count={} tick={tick}",
            entity,
            bb.home_zone,
            accepted.dan_count,
        );
    }
}

// ── Bevy 注册 P3 ──────────────────────────────────────────────────────────────

/// Bevy 注册：P3 Redis 叙事事件系统（appear / death / dan_received broadcast）。
pub fn register_p3(app: &mut App) {
    app.add_systems(
        valence::prelude::PostUpdate,
        dying_elder_p3_emit_appear_event_system.after(valence::entity::InitEntitiesSet),
    );
    app.add_systems(
        Update,
        (
            // 第五颗丹同帧产生收丹与终态反馈：先广播收丹，终态必须最后到达，
            // 避免 client/agent 被后到的 DanReceived 覆盖死亡状态。
            dying_elder_p3_emit_death_event_system
                .after(dying_elder_drain_system)
                .after(dying_elder_betray_system)
                .after(dying_elder_p3_emit_dan_received_event_system)
                .before(dying_elder_death_system),
            // dan_received broadcast 在 give_dan_system 之后（状态已更新后再广播）
            dying_elder_p3_emit_dan_received_event_system.after(dying_elder_give_dan_system),
        ),
    );
}

#[cfg(test)]
#[path = "dying_elder_tests.rs"]
mod tests;
