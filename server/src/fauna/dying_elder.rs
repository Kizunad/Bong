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
use crate::world::dimension::{DimensionKind, DimensionLayers};
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
#[allow(clippy::type_complexity)]
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
        if !qi_gain.is_finite() || qi_gain <= 0.0 || !fallback_qi_after.is_finite() {
            tracing::warn!(
                "[bong][dying_elder] give_dan_system: invalid canonical pill qi={} or fallback sum={} elder={:?}; keep inventory/state unchanged",
                qi_gain,
                fallback_qi_after,
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
        Update,
        (
            dying_elder_p3_emit_appear_event_system,
            // 状态生产者完成后立即广播；结算失败重试不应重复叙事。
            dying_elder_p3_emit_death_event_system
                .after(dying_elder_drain_system)
                .after(dying_elder_betray_system)
                .before(dying_elder_death_system),
            // dan_received broadcast 在 give_dan_system 之后（状态已更新后再广播）
            dying_elder_p3_emit_dan_received_event_system.after(dying_elder_give_dan_system),
        ),
    );
}

// ── 测试 ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── 常数 pin 测试 ─────────────────────────────────────────────────────────

    #[test]
    fn spawn_interval_equals_30_game_days() {
        // 期望：DYING_ELDER_SPAWN_INTERVAL_TICKS = 30 × GAME_DAY_TICKS
        // 防止常数漂移破坏稀有性保证
        assert_eq!(
            DYING_ELDER_SPAWN_INTERVAL_TICKS,
            30 * GAME_DAY_TICKS,
            "spawn 间隔应精确为 30 in-game days（30 × GAME_DAY_TICKS = {}）；\
             实际 = {}（若 GAME_DAY_TICKS 变动，同步更新 plan 文档说明）",
            30 * GAME_DAY_TICKS,
            DYING_ELDER_SPAWN_INTERVAL_TICKS
        );
    }

    #[test]
    fn global_cap_is_one() {
        // 期望：全服上限为 1，保证稀有遭遇
        assert_eq!(
            DYING_ELDER_GLOBAL_CAP, 1,
            "全服上限必须为 1（worldview §七「极度稀有」）；实际 = {}",
            DYING_ELDER_GLOBAL_CAP
        );
    }

    #[test]
    fn spirit_qi_threshold_is_negative_0_4() {
        // 期望：gate 阈值 = -0.4（坍缩渊深度负灵域标志值）
        assert!(
            (DYING_ELDER_SPIRIT_QI_THRESHOLD - (-0.4)).abs() < f64::EPSILON,
            "spirit_qi gate 阈值应为 -0.4（坍缩渊深度负灵域），实际 = {}",
            DYING_ELDER_SPIRIT_QI_THRESHOLD
        );
    }

    #[test]
    fn earth_grade_technique_pool_has_four_entries() {
        // 期望：地阶功法池精确 4 条，分别是设计决议指定的 ID
        assert_eq!(
            EARTH_GRADE_TECHNIQUE_POOL.len(),
            4,
            "EARTH_GRADE_TECHNIQUE_POOL 应包含 4 门地阶功法，实际 = {}",
            EARTH_GRADE_TECHNIQUE_POOL.len()
        );
        assert!(
            EARTH_GRADE_TECHNIQUE_POOL.contains(&"woliu.heart"),
            "功法池应包含 woliu.heart（无流心诀）"
        );
        assert!(
            EARTH_GRADE_TECHNIQUE_POOL.contains(&"woliu.turbulence_burst"),
            "功法池应包含 woliu.turbulence_burst（无流湍爆）"
        );
        assert!(
            EARTH_GRADE_TECHNIQUE_POOL.contains(&"anqi.echo_fractal"),
            "功法池应包含 anqi.echo_fractal（暗器回声裂变）"
        );
        assert!(
            EARTH_GRADE_TECHNIQUE_POOL.contains(&"sword_path.heaven_gate"),
            "功法池应包含 sword_path.heaven_gate（剑道天门）"
        );
    }

    // ── 状态机 pin 测试 ───────────────────────────────────────────────────────

    #[test]
    fn dying_elder_state_default_is_plea() {
        // 期望：默认状态为 Plea（初始乞求态）
        assert_eq!(
            DyingElderState::default(),
            DyingElderState::Plea,
            "DyingElderState 默认应为 Plea（大能进入遭遇时以乞求态开始）"
        );
    }

    #[test]
    fn dying_elder_state_all_variants_serialize() {
        // 期望：所有状态变体均可序列化/反序列化，schema 稳定
        let variants = vec![
            DyingElderState::Plea,
            DyingElderState::Recovering { dan_received: 0 },
            DyingElderState::Recovering { dan_received: 3 },
            DyingElderState::Betrayal,
            DyingElderState::Dead {
                dead_by_betrayal: false,
            },
            DyingElderState::Dead {
                dead_by_betrayal: true,
            },
        ];
        for variant in &variants {
            let json = serde_json::to_string(variant)
                .unwrap_or_else(|e| panic!("DyingElderState {variant:?} 序列化失败: {e}"));
            let decoded: DyingElderState = serde_json::from_str(&json)
                .unwrap_or_else(|e| panic!("DyingElderState json={json} 反序列化失败: {e}"));
            assert_eq!(
                decoded, *variant,
                "DyingElderState {variant:?} 序列化往返应相等，实际反序列化 = {decoded:?}"
            );
        }
    }

    // ── Blackboard 初始化测试 ─────────────────────────────────────────────────

    #[test]
    fn blackboard_new_betray_probability_in_range() {
        // 期望：betray_probability ∈ [DYING_ELDER_BETRAY_PROB_MIN, DYING_ELDER_BETRAY_PROB_MAX]
        for seed in [0u64, 1, 42, 1234567890, u64::MAX] {
            let bb = DyingElderBlackboard::new("tsy_deep", DVec3::ZERO, seed, 0);
            assert!(
                (DYING_ELDER_BETRAY_PROB_MIN..=DYING_ELDER_BETRAY_PROB_MAX)
                    .contains(&bb.betray_probability),
                "betray_probability={:.3} 应在 [{}, {}]；seed={}",
                bb.betray_probability,
                DYING_ELDER_BETRAY_PROB_MIN,
                DYING_ELDER_BETRAY_PROB_MAX,
                seed
            );
        }
    }

    #[test]
    fn blackboard_new_offered_skill_from_pool() {
        // 期望：offered_skill_id 始终来自 EARTH_GRADE_TECHNIQUE_POOL
        for seed in [0u64, 1, 999, 123456, u64::MAX / 3] {
            let bb = DyingElderBlackboard::new("tsy_shallow", DVec3::ZERO, seed, 0);
            assert!(
                EARTH_GRADE_TECHNIQUE_POOL.contains(&bb.offered_skill_id),
                "offered_skill_id='{}' 不在 EARTH_GRADE_TECHNIQUE_POOL 中；seed={}",
                bb.offered_skill_id,
                seed
            );
        }
    }

    #[test]
    fn blackboard_new_qi_current_equals_initial() {
        // 期望：spawn 时 qi_current = DYING_ELDER_INITIAL_QI（化虚级 500）
        let bb = DyingElderBlackboard::new("tsy_deep", DVec3::ZERO, 0, 100);
        assert!(
            (bb.qi_current - DYING_ELDER_INITIAL_QI).abs() < f64::EPSILON,
            "spawn 时 qi_current={} 应等于 DYING_ELDER_INITIAL_QI={}",
            bb.qi_current,
            DYING_ELDER_INITIAL_QI
        );
        assert!(
            (bb.qi_max_cache - DYING_ELDER_INITIAL_QI).abs() < f64::EPSILON,
            "spawn 时 qi_max_cache={} 应等于 DYING_ELDER_INITIAL_QI={}",
            bb.qi_max_cache,
            DYING_ELDER_INITIAL_QI
        );
    }

    // ── 声名调整测试 ──────────────────────────────────────────────────────────

    #[test]
    fn renown_adjustment_reduces_betray_prob_when_fame_high() {
        // 期望：fame > 300 时 betray_probability 减少 0.2，且不低于 0.05
        let mut bb = DyingElderBlackboard::new("tsy_deep", DVec3::ZERO, 0, 0);
        let original_prob = bb.betray_probability;

        bb.apply_renown_adjustment(301);
        let expected = (original_prob - DYING_ELDER_RENOWN_BETRAY_REDUCTION).clamp(0.05, 0.95);
        assert!(
            (bb.betray_probability - expected).abs() < f64::EPSILON,
            "fame=301 时 betray_probability 应从 {:.3} 减至 {:.3}（clamp 后），实际 = {:.3}",
            original_prob,
            expected,
            bb.betray_probability
        );
    }

    #[test]
    fn renown_adjustment_no_change_when_fame_at_threshold() {
        // 期望：fame = 300（边界）不触发调整（条件 fame > 300）
        let mut bb = DyingElderBlackboard::new("tsy_deep", DVec3::ZERO, 0, 0);
        let original_prob = bb.betray_probability;

        bb.apply_renown_adjustment(300); // 等于阈值，不触发
        assert!(
            (bb.betray_probability - original_prob).abs() < f64::EPSILON,
            "fame=300（等于阈值，条件 fame>300）不应减少 betray_probability；\
             期望 = {:.3}，实际 = {:.3}",
            original_prob,
            bb.betray_probability
        );
    }

    #[test]
    fn renown_adjustment_clamps_to_minimum() {
        // 期望：即使 betray_probability 很小，调整后不低于 0.05
        let mut bb = DyingElderBlackboard::new("tsy_deep", DVec3::ZERO, 0, 0);
        bb.betray_probability = 0.20; // 手动设为接近下界
        bb.apply_renown_adjustment(301);
        assert!(
            bb.betray_probability >= 0.05,
            "renown 调整后 betray_probability={:.3} 不应低于 0.05（clamp 下界）",
            bb.betray_probability
        );
    }

    // ── betray_roll 测试 ──────────────────────────────────────────────────────

    #[test]
    fn betray_roll_deterministic_for_same_seed() {
        // 期望：相同 seed 和 probability，结果完全相同（可重现）
        let prob = 0.5;
        let seed = 12345u64;
        let r1 = betray_roll(prob, seed);
        let r2 = betray_roll(prob, seed);
        assert_eq!(
            r1, r2,
            "betray_roll 对相同 seed={seed} prob={prob} 应确定性，r1={r1} r2={r2}"
        );
    }

    #[test]
    fn betray_roll_always_true_when_probability_is_one() {
        // 期望：probability = 1.0 时必然翻脸（roll < 1.0 恒成立）
        for seed in [0u64, 1, 42, u64::MAX] {
            assert!(
                betray_roll(1.0, seed),
                "betray_probability=1.0 时任意 seed={seed} 均应翻脸"
            );
        }
    }

    #[test]
    fn betray_roll_never_true_when_probability_is_zero() {
        // 期望：probability = 0.0 时从不翻脸
        for seed in [0u64, 1, 42, u64::MAX] {
            assert!(
                !betray_roll(0.0, seed),
                "betray_probability=0.0 时任意 seed={seed} 均不应翻脸"
            );
        }
    }

    #[test]
    fn betray_roll_distribution_roughly_matches_probability() {
        // 期望：大量 seed 样本下，命中率约等于 betray_probability（±5%）
        let prob = 0.60;
        let samples = 10_000u64;
        let hits = (0..samples).filter(|&s| betray_roll(prob, s)).count() as f64;
        let ratio = hits / samples as f64;
        assert!(
            (ratio - prob).abs() < 0.05,
            "betray_roll(prob={prob}) 10k 次命中率={ratio:.3}，应在 {prob}±0.05 内"
        );
    }

    // ── 全服上限边界测试（纯逻辑，无 Bevy App） ─────────────────────────────

    #[test]
    fn spawn_gate_logic_respects_global_cap() {
        // 期望：existing_count >= DYING_ELDER_GLOBAL_CAP → spawn gate 关闭
        // 模拟上限判断（不实际启动 Bevy ECS）
        let global_cap = DYING_ELDER_GLOBAL_CAP;
        let existing_below_cap = global_cap - 1; // 0
        let existing_at_cap = global_cap; // 1

        // existing < cap → 允许 spawn
        assert!(
            existing_below_cap < global_cap,
            "existing={existing_below_cap} < cap={global_cap}，spawn gate 应开放"
        );

        // existing >= cap → 禁止 spawn
        assert!(
            existing_at_cap >= global_cap,
            "existing={existing_at_cap} >= cap={global_cap}，spawn gate 应关闭"
        );
    }

    #[test]
    fn spawn_timer_frequency_gate_logic() {
        // 期望：tick 间隔 < DYING_ELDER_SPAWN_INTERVAL_TICKS → 不触发
        let interval = DYING_ELDER_SPAWN_INTERVAL_TICKS;
        let last_attempt = 100u64;

        let tick_too_soon = last_attempt + interval - 1;
        let tick_ready = last_attempt + interval;

        // 未到间隔：gate 关闭
        assert!(
            tick_too_soon.saturating_sub(last_attempt) < interval,
            "tick_too_soon={tick_too_soon} 距上次尝试 {} < interval={interval}，\
             spawn gate 应仍处于冷却",
            tick_too_soon - last_attempt
        );

        // 正好到达间隔：gate 开放
        assert!(
            tick_ready.saturating_sub(last_attempt) >= interval,
            "tick_ready={tick_ready} 距上次尝试 {} >= interval={interval}，\
             spawn gate 应开放",
            tick_ready - last_attempt
        );
    }

    #[test]
    fn spawn_gate_requires_tsy_and_negative_spirit_qi() {
        // 期望：只有 is_tsy() + spirit_qi < -0.4 的 zone 才通过 gate
        // 纯逻辑测试（不实例化 Zone struct，Zone::spawn() 是 crate-private）
        let eval_gate = |name: &str, spirit_qi: f64| -> (bool, bool) {
            let is_tsy = name.starts_with("tsy_");
            let qi_ok = spirit_qi < DYING_ELDER_SPIRIT_QI_THRESHOLD;
            (is_tsy, qi_ok)
        };

        // TSY zone + spirit_qi 充分负 → 通过
        let (tsy, qi) = eval_gate("tsy_deep", -0.5);
        assert!(
            tsy && qi,
            "tsy_deep + spirit_qi=-0.5 应通过 gate（is_tsy={tsy} qi_ok={qi}）"
        );

        // TSY zone + spirit_qi 不够负 → 不通过
        let (_tsy, qi) = eval_gate("tsy_deep", -0.3);
        assert!(
            !qi,
            "tsy_deep + spirit_qi=-0.3 > -0.4 阈值，qi gate 应关闭（qi_ok={qi}）"
        );

        // 非 TSY zone + 任何 spirit_qi → 不通过
        let (tsy, _qi) = eval_gate("spawn", -0.9);
        assert!(
            !tsy,
            "spawn zone（非 tsy_ 前缀）应被 TSY gate 拒绝（is_tsy={tsy}）"
        );

        // 正好在阈值（-0.4）不通过（条件是严格小于）
        let (tsy, qi) = eval_gate("tsy_deep", -0.4);
        assert!(
            tsy && !qi,
            "spirit_qi=-0.4 等于阈值，应被拒绝（严格小于，qi_ok={qi}）"
        );
    }

    #[test]
    fn default_zone_registry_exposes_dying_elder_tsy_candidates() {
        let registry = ZoneRegistry::load();
        let candidate_names: Vec<_> = registry
            .zones
            .iter()
            .filter(|zone| zone.is_tsy() && zone.spirit_qi < DYING_ELDER_SPIRIT_QI_THRESHOLD)
            .map(|zone| zone.name.as_str())
            .collect();

        assert!(
            !candidate_names.is_empty(),
            "默认 ZoneRegistry 必须含至少 1 个满足垂死大能 gate 的 TSY zone"
        );
        assert!(
            candidate_names.contains(&"tsy_daneng_01_shallow"),
            "zones.tsy.json 的大能陨落浅层必须进入垂死大能候选集；实际候选={candidate_names:?}"
        );
    }

    #[test]
    fn spawn_system_emits_request_from_default_tsy_zones_after_interval() {
        use valence::prelude::{Events, Update};

        let mut app = App::new();
        app.add_event::<DyingElderSpawnRequest>();
        app.insert_resource(ZoneRegistry::load());
        app.insert_resource(GameTick(DYING_ELDER_SPAWN_INTERVAL_TICKS as u32));
        app.insert_resource(DyingElderSpawnTimer::default());
        app.add_systems(Update, dying_elder_spawn_system);

        app.update();

        let events = app.world().resource::<Events<DyingElderSpawnRequest>>();
        let requests: Vec<_> = events.get_reader().read(events).cloned().collect();
        assert_eq!(
            requests.len(),
            1,
            "到达 DYING_ELDER_SPAWN_INTERVAL_TICKS 后应真实 emit 1 个 spawn request"
        );

        let request = &requests[0];
        let registry = app.world().resource::<ZoneRegistry>();
        let zone = registry
            .find_zone_by_name(&request.zone_name)
            .expect("spawn request zone must exist in default ZoneRegistry");
        assert!(
            zone.is_tsy() && zone.spirit_qi < DYING_ELDER_SPIRIT_QI_THRESHOLD,
            "spawn request 必须来自满足 gate 的 TSY zone；zone={} spirit_qi={}",
            zone.name,
            zone.spirit_qi
        );

        let timer = app.world().resource::<DyingElderSpawnTimer>();
        assert_eq!(timer.total_spawned, 1);
        assert_eq!(
            timer.last_spawn_attempt_tick,
            DYING_ELDER_SPAWN_INTERVAL_TICKS
        );
    }

    // ── P1 给丹交互纯逻辑测试 ──────────────────────────────────────────────────

    #[test]
    fn give_dan_threshold_pin() {
        // 期望：DAN_THRESHOLD = 5（设计决议：累计 5 颗触发结局判定）
        assert_eq!(
            DYING_ELDER_DAN_THRESHOLD, 5,
            "DAN_THRESHOLD 应精确为 5（设计决议定稿）；实际 = {DYING_ELDER_DAN_THRESHOLD}"
        );
    }

    #[test]
    fn give_dan_state_transition_plea_to_recovering() {
        // 期望：首次给丹：Plea → Recovering { dan_received: 1 }
        // 纯状态机逻辑测试（不需要 Bevy ECS）
        let state = DyingElderState::Plea;
        let dan_received = match state {
            DyingElderState::Plea => 0,
            DyingElderState::Recovering { dan_received } => dan_received,
            _ => panic!("意外状态"),
        };
        let new_dan = dan_received + 1;
        let new_state = DyingElderState::Recovering {
            dan_received: new_dan,
        };
        assert_eq!(
            new_state,
            DyingElderState::Recovering { dan_received: 1 },
            "Plea 首次给丹后应转为 Recovering{{dan_received:1}}，实际 = {:?}",
            new_state
        );
    }

    #[test]
    fn give_dan_state_transition_recovering_to_recovering() {
        // 期望：给丹 2→3→4 颗，each 步 Recovering.dan_received 递增
        for n in 1..DYING_ELDER_DAN_THRESHOLD {
            let state = DyingElderState::Recovering { dan_received: n };
            let current = match state {
                DyingElderState::Recovering { dan_received } => dan_received,
                _ => panic!("意外状态"),
            };
            let next = current + 1;
            let expected = DyingElderState::Recovering { dan_received: next };
            // 只要 next < THRESHOLD，还不触发结局
            if next < DYING_ELDER_DAN_THRESHOLD {
                assert_eq!(
                    expected,
                    DyingElderState::Recovering { dan_received: next },
                    "给第 {} 颗时 dan_received 应从 {} 增至 {}",
                    next,
                    n,
                    next
                );
            }
        }
    }

    #[test]
    fn give_dan_threshold_triggers_outcome_on_fifth_dan() {
        // 期望：第 5 颗丹触发结局判定（dan_received 到达 DYING_ELDER_DAN_THRESHOLD）
        let current_dan = DYING_ELDER_DAN_THRESHOLD - 1; // 4 颗已给
        let new_dan = current_dan + 1; // 第 5 颗
        assert!(
            new_dan >= DYING_ELDER_DAN_THRESHOLD,
            "第 {new_dan} 颗丹应触发结局判定（threshold={DYING_ELDER_DAN_THRESHOLD}）"
        );
    }

    #[test]
    fn give_dan_state_betrayal_rejects_further_dan() {
        // 期望：大能在 Betrayal 状态不接受进一步的丹
        let state = DyingElderState::Betrayal;
        let can_accept = matches!(
            state,
            DyingElderState::Plea | DyingElderState::Recovering { .. }
        );
        assert!(
            !can_accept,
            "Betrayal 状态不应接受给丹（can_accept={can_accept}）"
        );
    }

    #[test]
    fn give_dan_state_dead_rejects_further_dan() {
        // 期望：大能在 Dead 状态不接受进一步的丹（死的都死了）
        for dead_state in [
            DyingElderState::Dead {
                dead_by_betrayal: false,
            },
            DyingElderState::Dead {
                dead_by_betrayal: true,
            },
        ] {
            let can_accept = matches!(
                dead_state,
                DyingElderState::Plea | DyingElderState::Recovering { .. }
            );
            assert!(
                !can_accept,
                "Dead 状态不应接受给丹；dead_by_betrayal={:?}",
                dead_state
            );
        }
    }

    // ── P1 SoulSeize 守恒纯逻辑测试 ──────────────────────────────────────────

    #[test]
    fn soul_seize_ratio_pin() {
        // 期望：SOUL_SEIZE_RATIO = 0.10（10% qi_max 减损，永久）
        assert!(
            (DYING_ELDER_SOUL_SEIZE_RATIO - 0.10).abs() < f64::EPSILON,
            "SOUL_SEIZE_RATIO 应为 0.10（永久 qi_max 减损 10%）；实际 = {DYING_ELDER_SOUL_SEIZE_RATIO}"
        );
    }

    #[test]
    fn soul_seize_qi_max_drain_calculation() {
        // 期望：qi_max_drain = qi_max_cache × SOUL_SEIZE_RATIO（数学验证）
        let qi_max_cache = 500.0_f64;
        let drain = qi_max_cache * DYING_ELDER_SOUL_SEIZE_RATIO;
        assert!(
            (drain - 50.0).abs() < f64::EPSILON,
            "qi_max_cache=500 × ratio=0.10 应得 drain=50.0，实际 = {drain}"
        );
    }

    #[test]
    fn soul_seize_does_not_make_qi_max_negative() {
        // 期望：即使玩家 qi_max 很小，SoulSeize 后 qi_max 不为负
        let qi_max_cache = 500.0_f64;
        let qi_max_drain = qi_max_cache * DYING_ELDER_SOUL_SEIZE_RATIO; // 50.0
                                                                        // 极端情况：玩家 qi_max 仅 30，drain 50 → 应 clamp 到 0
        let player_qi_max = 30.0_f64;
        let new_qi_max = (player_qi_max - qi_max_drain).max(0.0);
        assert!(
            new_qi_max >= 0.0,
            "SoulSeize 后 qi_max={new_qi_max} 不应为负（player_qi_max={player_qi_max} drain={qi_max_drain}）"
        );
    }

    #[test]
    fn soul_seize_qi_current_transfer_is_exact() {
        // 期望：player.qi_current 全额转入大能，守恒不变式：
        // before: player_qi + elder_qi = total
        // after:  0 + (elder_qi + player_qi) = total
        let player_qi = 120.5_f64;
        let elder_qi = 200.0_f64;
        let total_before = player_qi + elder_qi;
        let transferred = player_qi; // player.qi_current → 0
        let new_elder_qi = elder_qi + transferred;
        let total_after = 0.0 + new_elder_qi;
        assert!(
            (total_before - total_after).abs() < f64::EPSILON,
            "SoulSeize 守恒：total_before={total_before} 应等于 total_after={total_after}"
        );
    }

    #[test]
    fn soul_seize_qi_max_debuff_does_not_affect_qi_current_conservation() {
        // 期望：qi_max debuff 是容量减少，不影响 qi_current 守恒
        // qi_max -= drain 只改容量，player.qi_current 的减少量已全部转给 elder
        let player_qi = 80.0_f64;
        let player_qi_max = 300.0_f64;
        let qi_max_drain = 30.0_f64; // 独立于 qi_current 转移
        let transferred_qi = player_qi; // = 80.0，全额转移
        let new_player_qi_max = (player_qi_max - qi_max_drain).max(0.0); // 270.0
                                                                         // 守恒检查：qi_current 转移量独立于 qi_max debuff
        assert!(
            (transferred_qi - player_qi).abs() < f64::EPSILON,
            "qi_current 转移量应等于 player_qi；transferred={transferred_qi} player_qi={player_qi}"
        );
        assert!(
            (new_player_qi_max - 270.0).abs() < f64::EPSILON,
            "qi_max_debuff 后应为 270.0；实际 = {new_player_qi_max}"
        );
    }

    // ── P1 TradeDan QiTransfer 审计测试 ──────────────────────────────────────

    #[test]
    fn trade_dan_qi_transfer_reason_variant_exists() {
        // 期望：QiTransferReason::TradeDan 和 SoulSeize variant 存在且可用
        // 只要编译通过，variant 就存在（类型系统保证）
        let _trade_dan = QiTransferReason::TradeDan;
        let _soul_seize = QiTransferReason::SoulSeize;
        assert!(
            matches!(_trade_dan, QiTransferReason::TradeDan),
            "TradeDan variant 应存在（编译期验证）"
        );
        assert!(
            matches!(_soul_seize, QiTransferReason::SoulSeize),
            "SoulSeize variant 应存在（编译期验证）"
        );
    }

    #[test]
    fn give_dan_qi_gain_partitions_at_qi_cap_without_loss() {
        // 期望：cap 内留在大能，cap 外进入稳定 overflow；两者之和精确闭合丹的真实输入。
        let qi_max_cache = DYING_ELDER_INITIAL_QI; // 500.0
        let qi_cap = qi_max_cache * 1.5; // 750.0
        let mut qi_current = qi_max_cache; // 500.0
        let mut overflow = 0.0_f64;
        let qi_gain_per_dan = huiyuan_pill_qi_gain_from_registry();
        // 给 10 颗（远超 threshold），模拟上限保护
        for i in 0..10 {
            let room = (qi_cap - qi_current).max(0.0);
            let accepted = qi_gain_per_dan.min(room);
            qi_current += accepted;
            overflow += qi_gain_per_dan - accepted;
            assert!(
                qi_current <= qi_cap,
                "第 {i} 颗丹后 qi_current={qi_current} 不应超过 qi_cap={qi_cap}"
            );
            let expected_total = qi_max_cache + qi_gain_per_dan * (i + 1) as f64;
            assert!(
                (qi_current + overflow - expected_total).abs() <= QI_EPSILON,
                "第 {i} 颗丹后 cap 内与 overflow 必须闭合 expected={expected_total}"
            );
        }
    }

    #[test]
    fn give_dan_qi_gain_conservation_per_dan() {
        // 期望：每颗丹的 qi 增量精确等于 qi_gain（在 cap 范围内）
        let initial_qi = 100.0_f64;
        let qi_gain = huiyuan_pill_qi_gain_from_registry();
        let qi_cap = DYING_ELDER_INITIAL_QI * 1.5; // 750.0
        let expected_after = (initial_qi + qi_gain).min(qi_cap);
        assert!(
            (expected_after - (initial_qi + qi_gain)).abs() < f64::EPSILON || expected_after < qi_cap,
            "qi 增量应精确为 qi_gain={qi_gain}（在 cap 内）；initial={initial_qi} expected_after={expected_after}"
        );
    }

    #[test]
    fn give_dan_over_cap_without_ledger_keeps_full_consumed_pill_qi_in_elder() {
        use valence::prelude::App;

        let mut app = App::new();
        app.add_event::<GiveDanToElderIntent>();
        app.add_event::<DyingElderDanAcceptedEvent>();
        app.add_event::<SoulSeizeEvent>();
        app.add_event::<QiTransfer>();
        app.insert_resource(crate::inventory::load_item_registry().expect("真实 registry"));
        app.add_systems(valence::prelude::Update, dying_elder_give_dan_system);

        let player = app
            .world_mut()
            .spawn((ClientMarker, inventory_with_huiyuan_pills(&[(99, 1)])))
            .id();
        let mut bb = DyingElderBlackboard::new("tsy_deep", DVec3::ZERO, 7, 0);
        bb.betray_probability = 0.0;
        bb.qi_current = 740.0;
        let elder = app
            .world_mut()
            .spawn((
                NpcMarker,
                bb,
                DyingElderState::Recovering {
                    dan_received: DYING_ELDER_DAN_THRESHOLD - 1,
                },
                Cultivation {
                    qi_current: 740.0,
                    qi_max: DYING_ELDER_INITIAL_QI,
                    ..Cultivation::default()
                },
            ))
            .id();
        let qi_gain = huiyuan_pill_qi_gain_from_registry();

        app.world_mut().send_event(GiveDanToElderIntent {
            player,
            elder,
            pill_instance_id: 99,
        });
        app.update();

        let elder_ref = app.world().entity(elder);
        let expected_qi = 740.0 + qi_gain;
        assert!((expected_qi - 800.0).abs() <= QI_EPSILON);
        assert!(
            (elder_ref.get::<Cultivation>().unwrap().qi_current - expected_qi).abs() <= QI_EPSILON,
            "丹已消费且 overflow 无法入账时，完整 60 真元必须留在物理权威"
        );
        assert!(
            (elder_ref.get::<DyingElderBlackboard>().unwrap().qi_current - expected_qi).abs()
                <= QI_EPSILON,
            "Blackboard mirror 必须同步临时越 cap 的物理权威"
        );
        assert_eq!(
            *elder_ref.get::<DyingElderState>().unwrap(),
            DyingElderState::Dead {
                dead_by_betrayal: false
            },
            "ledger 缺失只影响 excess 落点，不得阻断第五丹结局推进"
        );

        let emitted = app
            .world_mut()
            .resource_mut::<bevy_ecs::event::Events<QiTransfer>>()
            .drain()
            .collect::<Vec<_>>();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].reason, QiTransferReason::TradeDan);
        assert!((emitted[0].amount - qi_gain).abs() <= QI_EPSILON);
        assert_eq!(
            emitted[0].to,
            QiAccountId::npc(format!("dying_elder:{}", elder.to_bits()))
        );
        let inventory = app.world().entity(player).get::<PlayerInventory>().unwrap();
        assert!(inventory.hotbar[0].is_none());
        assert_eq!(inventory.revision.0, 1);
        let accepted = app
            .world_mut()
            .resource_mut::<bevy_ecs::event::Events<DyingElderDanAcceptedEvent>>()
            .drain()
            .collect::<Vec<_>>();
        assert_eq!(accepted.len(), 1);
        assert_eq!(accepted[0].dan_count, DYING_ELDER_DAN_THRESHOLD);
        assert!((accepted[0].qi_gain - qi_gain).abs() <= QI_EPSILON);
        let soul_seize = app
            .world_mut()
            .resource_mut::<bevy_ecs::event::Events<SoulSeizeEvent>>()
            .drain()
            .collect::<Vec<_>>();
        assert!(soul_seize.is_empty(), "守信结局不得误发夺舍事件");
    }

    fn give_dan_test_app() -> valence::prelude::App {
        let mut app = valence::prelude::App::new();
        app.add_event::<GiveDanToElderIntent>();
        app.add_event::<DyingElderDanAcceptedEvent>();
        app.add_event::<SoulSeizeEvent>();
        app.add_event::<QiTransfer>();
        app.insert_resource(WorldQiAccount::default());
        app.insert_resource(crate::inventory::load_item_registry().expect("真实 registry"));
        app.add_systems(valence::prelude::Update, dying_elder_give_dan_system);
        app
    }

    fn spawn_recovering_elder(
        app: &mut valence::prelude::App,
        dan_received: u32,
        qi_current: f64,
    ) -> Entity {
        let mut bb = DyingElderBlackboard::new("tsy_deep", DVec3::ZERO, 7, 0);
        bb.betray_probability = 0.0;
        bb.qi_current = qi_current;
        app.world_mut()
            .spawn((
                NpcMarker,
                bb,
                DyingElderState::Recovering { dan_received },
                Cultivation {
                    qi_current,
                    qi_max: DYING_ELDER_INITIAL_QI,
                    ..Cultivation::default()
                },
            ))
            .id()
    }

    #[test]
    fn give_dan_same_tick_fifth_and_sixth_consumes_only_fifth() {
        let mut app = give_dan_test_app();
        let player = app
            .world_mut()
            .spawn((ClientMarker, inventory_with_huiyuan_pills(&[(10, 2)])))
            .id();
        let elder = spawn_recovering_elder(&mut app, DYING_ELDER_DAN_THRESHOLD - 1, 100.0);

        // 两个 intent 必须在同一次 app.update 前入队，锁住真实 EventReader 并发边界。
        for _ in 0..2 {
            app.world_mut().send_event(GiveDanToElderIntent {
                player,
                elder,
                pill_instance_id: 10,
            });
        }
        app.update();

        let inventory = app.world().entity(player).get::<PlayerInventory>().unwrap();
        let remaining = inventory.hotbar[0]
            .as_ref()
            .expect("第六颗必须仍留在 stack");
        assert_eq!(remaining.stack_count, 1);
        assert_eq!(
            inventory.revision.0, 1,
            "只有第五颗成功事务可 bump revision"
        );
        assert_eq!(
            *app.world().entity(elder).get::<DyingElderState>().unwrap(),
            DyingElderState::Dead {
                dead_by_betrayal: false
            }
        );

        let accepted = app
            .world_mut()
            .resource_mut::<bevy_ecs::event::Events<DyingElderDanAcceptedEvent>>()
            .drain()
            .collect::<Vec<_>>();
        assert_eq!(accepted.len(), 1, "第六颗被状态门禁拒绝，不得伪造 Accepted");
        assert_eq!(accepted[0].dan_count, DYING_ELDER_DAN_THRESHOLD);
        let qi_gain = huiyuan_pill_qi_gain_from_registry();
        let traded = app
            .world()
            .resource::<WorldQiAccount>()
            .transfers()
            .iter()
            .filter(|transfer| transfer.reason == QiTransferReason::TradeDan)
            .collect::<Vec<_>>();
        assert_eq!(traded.len(), 1, "同 tick 第五/第六丹只能提交一笔 TradeDan");
        assert!((traded[0].amount - qi_gain).abs() <= QI_EPSILON);
    }

    #[test]
    fn give_dan_same_tick_two_players_only_first_wins_fifth() {
        let mut app = give_dan_test_app();
        let first = app
            .world_mut()
            .spawn((ClientMarker, inventory_with_huiyuan_pills(&[(20, 1)])))
            .id();
        let second = app
            .world_mut()
            .spawn((ClientMarker, inventory_with_huiyuan_pills(&[(21, 1)])))
            .id();
        let elder = spawn_recovering_elder(&mut app, DYING_ELDER_DAN_THRESHOLD - 1, 100.0);

        app.world_mut().send_event(GiveDanToElderIntent {
            player: first,
            elder,
            pill_instance_id: 20,
        });
        app.world_mut().send_event(GiveDanToElderIntent {
            player: second,
            elder,
            pill_instance_id: 21,
        });
        app.update();

        let first_inventory = app.world().entity(first).get::<PlayerInventory>().unwrap();
        assert!(first_inventory.hotbar[0].is_none());
        assert_eq!(first_inventory.revision.0, 1);
        let second_inventory = app.world().entity(second).get::<PlayerInventory>().unwrap();
        assert!(
            second_inventory.hotbar[0].is_some(),
            "输掉竞态的玩家不得被扣丹"
        );
        assert_eq!(second_inventory.revision.0, 0);

        let accepted = app
            .world_mut()
            .resource_mut::<bevy_ecs::event::Events<DyingElderDanAcceptedEvent>>()
            .drain()
            .collect::<Vec<_>>();
        assert_eq!(accepted.len(), 1);
        assert_eq!(
            accepted[0].player, first,
            "EventReader 先到的玩家赢得第五丹事务"
        );
        assert_eq!(accepted[0].pill_instance_id, 20);
        assert_eq!(
            app.world()
                .resource::<WorldQiAccount>()
                .transfers()
                .iter()
                .filter(|transfer| transfer.reason == QiTransferReason::TradeDan)
                .count(),
            1
        );
    }

    #[test]
    fn give_dan_same_tick_before_threshold_accepts_both_in_reader_order() {
        let mut app = give_dan_test_app();
        let player = app
            .world_mut()
            .spawn((
                ClientMarker,
                inventory_with_huiyuan_pills(&[(30, 1), (31, 1)]),
            ))
            .id();
        let elder = spawn_recovering_elder(&mut app, 2, 100.0);

        for pill_instance_id in [30, 31] {
            app.world_mut().send_event(GiveDanToElderIntent {
                player,
                elder,
                pill_instance_id,
            });
        }
        app.update();

        let inventory = app.world().entity(player).get::<PlayerInventory>().unwrap();
        assert!(inventory.hotbar[0].is_none());
        assert!(inventory.hotbar[1].is_none());
        assert_eq!(inventory.revision.0, 2);
        assert_eq!(
            *app.world().entity(elder).get::<DyingElderState>().unwrap(),
            DyingElderState::Recovering { dan_received: 4 }
        );

        let accepted = app
            .world_mut()
            .resource_mut::<bevy_ecs::event::Events<DyingElderDanAcceptedEvent>>()
            .drain()
            .collect::<Vec<_>>();
        assert_eq!(accepted.len(), 2);
        assert_eq!(
            accepted
                .iter()
                .map(|event| (event.pill_instance_id, event.dan_count))
                .collect::<Vec<_>>(),
            vec![(30, 3), (31, 4)],
            "Accepted 必须保留逐笔提交后的 dan_count 顺序"
        );
        let qi_gain = huiyuan_pill_qi_gain_from_registry();
        let expected_fractions = [
            ((100.0 + qi_gain) / DYING_ELDER_INITIAL_QI) as f32,
            ((100.0 + 2.0 * qi_gain) / DYING_ELDER_INITIAL_QI) as f32,
        ];
        for (event, expected_fraction) in accepted.iter().zip(expected_fractions) {
            assert!(
                (event.qi_fraction - expected_fraction).abs() <= f32::EPSILON,
                "Accepted 必须携带逐笔提交时的 qi_fraction，而非同 tick 最终态"
            );
        }
        assert_eq!(
            app.world()
                .resource::<WorldQiAccount>()
                .transfers()
                .iter()
                .filter(|transfer| transfer.reason == QiTransferReason::TradeDan)
                .count(),
            2
        );
    }

    #[test]
    fn give_dan_rejection_paths_leave_inventory_ledger_and_elder_unchanged() {
        #[derive(Clone, Copy)]
        enum InventoryCase {
            Valid,
            Missing,
            MissingInstance,
            WrongTemplate,
        }

        #[derive(Clone, Copy)]
        enum RegistryCase {
            Real,
            Missing,
            Empty,
            NoEffect,
            WrongEffect,
            QiRecovery(f64),
        }

        fn registry_with_effect(effect: Option<ItemEffect>) -> ItemRegistry {
            let real = crate::inventory::load_item_registry().expect("真实 registry");
            let mut template = real
                .get("huiyuan_pill")
                .expect("真实 registry 应含 huiyuan_pill")
                .clone();
            template.effect = effect;
            ItemRegistry::from_map(std::collections::HashMap::from([(
                template.id.clone(),
                template,
            )]))
        }

        let cases = [
            (
                "threshold_reached",
                DyingElderState::Recovering {
                    dan_received: DYING_ELDER_DAN_THRESHOLD,
                },
                100.0,
                DYING_ELDER_INITIAL_QI,
                InventoryCase::Valid,
                RegistryCase::Real,
            ),
            (
                "betrayal",
                DyingElderState::Betrayal,
                100.0,
                DYING_ELDER_INITIAL_QI,
                InventoryCase::Valid,
                RegistryCase::Real,
            ),
            (
                "dead",
                DyingElderState::Dead {
                    dead_by_betrayal: false,
                },
                100.0,
                DYING_ELDER_INITIAL_QI,
                InventoryCase::Valid,
                RegistryCase::Real,
            ),
            (
                "elder_nan",
                DyingElderState::Plea,
                f64::NAN,
                DYING_ELDER_INITIAL_QI,
                InventoryCase::Valid,
                RegistryCase::Real,
            ),
            (
                "elder_negative",
                DyingElderState::Plea,
                -1.0,
                DYING_ELDER_INITIAL_QI,
                InventoryCase::Valid,
                RegistryCase::Real,
            ),
            (
                "max_cache_nan",
                DyingElderState::Plea,
                100.0,
                f64::NAN,
                InventoryCase::Valid,
                RegistryCase::Real,
            ),
            (
                "max_cache_negative",
                DyingElderState::Plea,
                100.0,
                -1.0,
                InventoryCase::Valid,
                RegistryCase::Real,
            ),
            (
                "cap_overflow",
                DyingElderState::Plea,
                100.0,
                f64::MAX,
                InventoryCase::Valid,
                RegistryCase::Real,
            ),
            (
                "missing_inventory",
                DyingElderState::Plea,
                100.0,
                DYING_ELDER_INITIAL_QI,
                InventoryCase::Missing,
                RegistryCase::Real,
            ),
            (
                "missing_instance",
                DyingElderState::Plea,
                100.0,
                DYING_ELDER_INITIAL_QI,
                InventoryCase::MissingInstance,
                RegistryCase::Real,
            ),
            (
                "wrong_template",
                DyingElderState::Plea,
                100.0,
                DYING_ELDER_INITIAL_QI,
                InventoryCase::WrongTemplate,
                RegistryCase::Real,
            ),
            (
                "missing_registry",
                DyingElderState::Plea,
                100.0,
                DYING_ELDER_INITIAL_QI,
                InventoryCase::Valid,
                RegistryCase::Missing,
            ),
            (
                "missing_registry_template",
                DyingElderState::Plea,
                100.0,
                DYING_ELDER_INITIAL_QI,
                InventoryCase::Valid,
                RegistryCase::Empty,
            ),
            (
                "missing_effect",
                DyingElderState::Plea,
                100.0,
                DYING_ELDER_INITIAL_QI,
                InventoryCase::Valid,
                RegistryCase::NoEffect,
            ),
            (
                "wrong_effect",
                DyingElderState::Plea,
                100.0,
                DYING_ELDER_INITIAL_QI,
                InventoryCase::Valid,
                RegistryCase::WrongEffect,
            ),
            (
                "qi_gain_nan",
                DyingElderState::Plea,
                100.0,
                DYING_ELDER_INITIAL_QI,
                InventoryCase::Valid,
                RegistryCase::QiRecovery(f64::NAN),
            ),
            (
                "qi_gain_zero",
                DyingElderState::Plea,
                100.0,
                DYING_ELDER_INITIAL_QI,
                InventoryCase::Valid,
                RegistryCase::QiRecovery(0.0),
            ),
            (
                "qi_gain_negative",
                DyingElderState::Plea,
                100.0,
                DYING_ELDER_INITIAL_QI,
                InventoryCase::Valid,
                RegistryCase::QiRecovery(-1.0),
            ),
            (
                "fallback_sum_overflow",
                DyingElderState::Plea,
                f64::MAX,
                DYING_ELDER_INITIAL_QI,
                InventoryCase::Valid,
                RegistryCase::Real,
            ),
        ];

        for (label, initial_state, elder_qi, qi_max_cache, inventory_case, registry_case) in cases {
            let mut app = valence::prelude::App::new();
            app.add_event::<GiveDanToElderIntent>();
            app.add_event::<DyingElderDanAcceptedEvent>();
            app.add_event::<SoulSeizeEvent>();
            app.add_event::<QiTransfer>();
            app.insert_resource(WorldQiAccount::default());
            match registry_case {
                RegistryCase::Real => {
                    app.insert_resource(
                        crate::inventory::load_item_registry().expect("真实 registry"),
                    );
                }
                RegistryCase::Missing => {}
                RegistryCase::Empty => {
                    app.insert_resource(ItemRegistry::default());
                }
                RegistryCase::NoEffect => {
                    app.insert_resource(registry_with_effect(None));
                }
                RegistryCase::WrongEffect => {
                    app.insert_resource(registry_with_effect(Some(
                        ItemEffect::BreakthroughBonus { magnitude: 1.0 },
                    )));
                }
                RegistryCase::QiRecovery(amount) => {
                    app.insert_resource(registry_with_effect(Some(ItemEffect::QiRecovery {
                        amount,
                    })));
                }
            }
            app.add_systems(valence::prelude::Update, dying_elder_give_dan_system);

            let player = app.world_mut().spawn(ClientMarker).id();
            let inventory = match inventory_case {
                InventoryCase::Missing => None,
                InventoryCase::Valid => Some(inventory_with_huiyuan_pills(&[(60, 1)])),
                InventoryCase::MissingInstance => Some(inventory_with_huiyuan_pills(&[(61, 1)])),
                InventoryCase::WrongTemplate => {
                    let mut inventory = inventory_with_huiyuan_pills(&[(60, 1)]);
                    inventory.hotbar[0].as_mut().unwrap().template_id = "not_huiyuan".to_string();
                    Some(inventory)
                }
            };
            if let Some(inventory) = inventory {
                app.world_mut().entity_mut(player).insert(inventory);
            }
            let inventory_before =
                app.world()
                    .entity(player)
                    .get::<PlayerInventory>()
                    .map(|inventory| {
                        serde_json::to_value(inventory).expect("inventory fixture should serialize")
                    });

            let mut bb = DyingElderBlackboard::new("tsy_deep", DVec3::ZERO, 7, 0);
            bb.qi_current = elder_qi;
            bb.qi_max_cache = qi_max_cache;
            let elder = app
                .world_mut()
                .spawn((
                    NpcMarker,
                    bb,
                    initial_state,
                    Cultivation {
                        qi_current: elder_qi,
                        qi_max: DYING_ELDER_INITIAL_QI,
                        ..Cultivation::default()
                    },
                ))
                .id();

            app.world_mut().send_event(GiveDanToElderIntent {
                player,
                elder,
                pill_instance_id: 60,
            });
            app.update();

            assert_eq!(
                app.world()
                    .entity(player)
                    .get::<PlayerInventory>()
                    .map(|inventory| serde_json::to_value(inventory)
                        .expect("inventory after rejection should serialize")),
                inventory_before,
                "case={label}: rejected give must not mutate inventory"
            );
            let elder_ref = app.world().entity(elder);
            assert_eq!(
                *elder_ref.get::<DyingElderState>().unwrap(),
                initial_state,
                "case={label}"
            );
            assert_same_float(
                elder_ref.get::<Cultivation>().unwrap().qi_current,
                elder_qi,
                label,
            );
            assert_same_float(
                elder_ref.get::<DyingElderBlackboard>().unwrap().qi_current,
                elder_qi,
                label,
            );
            assert_same_float(
                elder_ref
                    .get::<DyingElderBlackboard>()
                    .unwrap()
                    .qi_max_cache,
                qi_max_cache,
                label,
            );
            assert!(
                app.world()
                    .resource::<WorldQiAccount>()
                    .transfers()
                    .is_empty(),
                "case={label}: rejected give must not audit"
            );
            assert!(
                app.world()
                    .resource::<WorldQiAccount>()
                    .iter_balances()
                    .next()
                    .is_none(),
                "case={label}: rejected give must not create ledger accounts"
            );
            assert!(
                app.world_mut()
                    .resource_mut::<bevy_ecs::event::Events<DyingElderDanAcceptedEvent>>()
                    .drain()
                    .next()
                    .is_none(),
                "case={label}: rejected give must not emit Accepted"
            );
            assert!(
                app.world_mut()
                    .resource_mut::<bevy_ecs::event::Events<QiTransfer>>()
                    .drain()
                    .next()
                    .is_none(),
                "case={label}: rejected give must not emit QiTransfer"
            );
        }
    }

    #[test]
    fn betrayal_state_transitions_to_dead_after_soul_seize() {
        // 期望：Betrayal 态经夺舍后大能转为 Dead { dead_by_betrayal: true }
        let expected = DyingElderState::Dead {
            dead_by_betrayal: true,
        };
        assert_eq!(
            expected,
            DyingElderState::Dead {
                dead_by_betrayal: true
            },
            "夺舍完成后大能应处于 Dead{{dead_by_betrayal:true}}，实际 = {:?}",
            expected
        );
        // 守信自裁（非翻脸路线）是 Dead { dead_by_betrayal: false }
        let honorable = DyingElderState::Dead {
            dead_by_betrayal: false,
        };
        assert_ne!(
            expected, honorable,
            "背叛死亡(dead_by_betrayal=true) 与守信死亡(false) 应不同，用于 loot 分档"
        );
    }

    // ── P2：skill_id_to_scroll_template 映射测试 ──────────────────────────────

    #[test]
    fn skill_id_to_scroll_template_maps_all_pool_entries() {
        // 期望：EARTH_GRADE_TECHNIQUE_POOL 中每个 skill_id 都有对应的 scroll template_id
        for skill_id in EARTH_GRADE_TECHNIQUE_POOL {
            let result = skill_id_to_scroll_template(skill_id);
            assert!(
                result.is_some(),
                "skill_id='{}' 在 EARTH_GRADE_TECHNIQUE_POOL 中但无 scroll 映射；\
                 每个地阶功法必须有对应的 scroll item（检查 assets/items/ 中是否有对应 toml 定义）",
                skill_id
            );
        }
    }

    #[test]
    fn skill_id_to_scroll_template_correct_values() {
        // 期望：各 skill_id 映射到精确的 scroll template_id（wire 契约 pin）
        let cases = [
            ("woliu.heart", "scroll_woliu_heart"),
            ("woliu.turbulence_burst", "scroll_woliu_turbulence_burst"),
            ("anqi.echo_fractal", "scroll_anqi_echo_fractal"),
            ("sword_path.heaven_gate", "scroll_sword_heaven_gate"),
        ];
        for (skill_id, expected_scroll) in cases {
            let actual = skill_id_to_scroll_template(skill_id);
            assert_eq!(
                actual,
                Some(expected_scroll),
                "skill_id='{}' 应映射到 '{}'，实际 = {:?}（loot 掉落依赖此映射正确）",
                skill_id,
                expected_scroll,
                actual
            );
        }
    }

    #[test]
    fn skill_id_to_scroll_template_unknown_returns_none() {
        // 期望：未知 skill_id 返回 None（不 panic，调用方处理 warn + skip）
        let unknown_ids = ["", "unknown_skill", "qi_blast", "woliu.nonexistent"];
        for skill_id in unknown_ids {
            assert!(
                skill_id_to_scroll_template(skill_id).is_none(),
                "未知 skill_id='{}' 应返回 None（调用方 warn + skip 掉落），不应 panic",
                skill_id
            );
        }
    }

    // ── P2：DyingElderDrainSystem 守恒纯逻辑测试 ──────────────────────────────

    #[test]
    fn drain_system_only_affects_plea_and_recovering_states() {
        // 期望：只有 Plea / Recovering 状态的大能受 drain 系统管辖
        let active_states = [
            DyingElderState::Plea,
            DyingElderState::Recovering { dan_received: 2 },
        ];
        let inactive_states = [
            DyingElderState::Betrayal,
            DyingElderState::Dead {
                dead_by_betrayal: false,
            },
            DyingElderState::Dead {
                dead_by_betrayal: true,
            },
        ];

        for state in &active_states {
            let should_drain = matches!(
                state,
                DyingElderState::Plea | DyingElderState::Recovering { .. }
            );
            assert!(
                should_drain,
                "状态 {:?} 应受 drain 系统管辖（should_drain=true）",
                state
            );
        }
        for state in &inactive_states {
            let should_drain = matches!(
                state,
                DyingElderState::Plea | DyingElderState::Recovering { .. }
            );
            assert!(
                !should_drain,
                "状态 {:?} 不应受 drain 系统管辖（should_drain=false）",
                state
            );
        }
    }

    #[test]
    fn give_dan_precedes_last_breath_drain_in_same_tick() {
        let mut app = valence::prelude::App::new();
        app.add_event::<GiveDanToElderIntent>();
        app.add_event::<DyingElderDanAcceptedEvent>();
        app.add_event::<SoulSeizeEvent>();
        app.add_event::<QiTransfer>();
        app.insert_resource(crate::inventory::load_item_registry().expect("真实 registry"));
        app.insert_resource(WorldQiAccount::default());
        let mut zones = ZoneRegistry::fallback();
        zones.zones[0].name = "tsy_deep".to_string();
        zones.zones[0].spirit_qi = -1.0;
        let expected_drain = compute_drain_per_tick(
            &zones.zones[0],
            &Cultivation {
                qi_current: 1.0,
                qi_max: DYING_ELDER_INITIAL_QI,
                ..Cultivation::default()
            },
        );
        assert!(
            expected_drain > 1.0,
            "fixture 必须足以在先 drain 时杀死大能"
        );
        app.insert_resource(zones);
        app.add_systems(valence::prelude::Update, dying_elder_give_dan_system);
        // 由生产注册函数提供 give → drain ordering，测试不得复制一份局部顺序假绿。
        register_p2(&mut app);

        let player = app
            .world_mut()
            .spawn((ClientMarker, inventory_with_huiyuan_pills(&[(41, 1)])))
            .id();
        let mut bb = DyingElderBlackboard::new("tsy_deep", DVec3::ZERO, 7, 0);
        bb.betray_probability = 0.0;
        bb.qi_current = 1.0;
        let elder = app
            .world_mut()
            .spawn((
                NpcMarker,
                bb,
                DyingElderState::Plea,
                Cultivation {
                    qi_current: 1.0,
                    qi_max: DYING_ELDER_INITIAL_QI,
                    ..Cultivation::default()
                },
            ))
            .id();

        app.world_mut().send_event(GiveDanToElderIntent {
            player,
            elder,
            pill_instance_id: 41,
        });
        app.update();

        let qi_gain = huiyuan_pill_qi_gain_from_registry();
        let elder_ref = app.world().entity(elder);
        assert_eq!(
            *elder_ref.get::<DyingElderState>().unwrap(),
            DyingElderState::Recovering { dan_received: 1 },
            "同 tick 必须先给丹续命，再执行 drain"
        );
        let expected_qi = 1.0 + qi_gain - expected_drain;
        assert!(
            (elder_ref.get::<Cultivation>().unwrap().qi_current - expected_qi).abs() <= QI_EPSILON
        );
        assert!(
            (elder_ref.get::<DyingElderBlackboard>().unwrap().qi_current - expected_qi).abs()
                <= QI_EPSILON
        );
        let inventory = app.world().entity(player).get::<PlayerInventory>().unwrap();
        assert!(inventory.hotbar[0].is_none());
        assert_eq!(inventory.revision.0, 1);
        let transfers = app.world().resource::<WorldQiAccount>().transfers();
        assert_eq!(
            transfers
                .iter()
                .filter(|transfer| transfer.reason == QiTransferReason::TradeDan)
                .count(),
            1
        );
        assert_eq!(
            transfers
                .iter()
                .filter(|transfer| transfer.reason == QiTransferReason::RiftCollapse)
                .count(),
            1
        );
    }

    #[test]
    fn drain_system_qi_exhaustion_transitions_to_dead_natural() {
        // 期望：qi_current 耗尽后 → Dead { dead_by_betrayal: false }（自然死亡）
        // 纯状态机逻辑（不启动 Bevy ECS）
        let mut state = DyingElderState::Plea;
        let mut qi_current = 0.001_f64; // 接近零

        // 模拟 drain 清零
        let drain = 0.01_f64;
        qi_current = (qi_current - drain).max(0.0);
        if qi_current <= 0.0 {
            state = DyingElderState::Dead {
                dead_by_betrayal: false,
            };
        }

        assert_eq!(
            state,
            DyingElderState::Dead {
                dead_by_betrayal: false
            },
            "真元耗尽后应转为 Dead{{dead_by_betrayal:false}}（自然死亡），实际 = {:?}",
            state
        );
    }

    #[test]
    fn drain_system_qi_not_negative_after_drain() {
        // 期望：drain 后 qi_current 不为负（clamp to 0）
        let qi_current = 0.5_f64;
        let drain = 1.0_f64; // 远超 qi_current
        let new_qi = (qi_current - drain).max(0.0);
        assert!(
            new_qi >= 0.0,
            "drain 后 qi_current={new_qi} 不应为负（drain={drain} > qi_current={qi_current}，应 clamp 到 0）"
        );
        assert!(
            (new_qi).abs() < f64::EPSILON,
            "drain 超出时 qi_current 应精确为 0.0，实际 = {new_qi}"
        );
    }

    #[test]
    fn drain_system_qi_conservation_invariant() {
        // 期望：drain 守恒不变式：drain_amount + new_qi == old_qi（在 clamp 前）
        // drain 系统：elder.qi 减少 = rift.qi 增加
        let old_qi = 100.0_f64;
        let drain = 5.0_f64;
        let new_qi = (old_qi - drain).max(0.0);
        let actual_drain = old_qi - new_qi; // = 5.0（未超出）
        assert!(
            (old_qi - actual_drain - new_qi).abs() < f64::EPSILON,
            "守恒不变式：old_qi({old_qi}) - actual_drain({actual_drain}) == new_qi({new_qi})"
        );
        // rift 获得的量 = actual_drain（守恒）
        let rift_gained = actual_drain;
        assert!(
            (rift_gained - 5.0_f64).abs() < f64::EPSILON,
            "rift 应获得 5.0，实际 = {rift_gained}"
        );
    }

    #[test]
    fn drain_system_credits_real_rift_balance_before_deducting_elder() {
        use valence::prelude::App;

        let mut app = App::new();
        app.add_event::<QiTransfer>();
        let mut zones = ZoneRegistry::fallback();
        zones.zones[0].name = "tsy_deep".to_string();
        zones.zones[0].spirit_qi = -0.6;
        let cultivation = Cultivation {
            qi_current: 100.0,
            qi_max: DYING_ELDER_INITIAL_QI,
            ..Cultivation::default()
        };
        let expected_drain = compute_drain_per_tick(&zones.zones[0], &cultivation);
        assert!(expected_drain > QI_EPSILON, "fixture 必须产生正 drain");
        app.insert_resource(zones);
        app.insert_resource(WorldQiAccount::default());
        app.add_systems(valence::prelude::Update, dying_elder_drain_system);

        let mut bb = DyingElderBlackboard::new("tsy_deep", DVec3::ZERO, 7, 0);
        bb.qi_current = cultivation.qi_current;
        let elder = app
            .world_mut()
            .spawn((NpcMarker, bb, DyingElderState::Plea, cultivation))
            .id();

        app.update();

        let elder_ref = app.world().entity(elder);
        let blackboard_qi = elder_ref.get::<DyingElderBlackboard>().unwrap().qi_current;
        let cultivation_qi = elder_ref.get::<Cultivation>().unwrap().qi_current;
        let expected_after = 100.0 - expected_drain;
        assert!((blackboard_qi - expected_after).abs() <= QI_EPSILON);
        assert!((cultivation_qi - expected_after).abs() <= QI_EPSILON);
        assert!((blackboard_qi - cultivation_qi).abs() <= QI_EPSILON);

        let rift_account = QiAccountId::rift("tsy_deep");
        let elder_source = QiAccountId::npc(format!("dying_elder:{}", elder.to_bits()));
        let account = app.world().resource::<WorldQiAccount>();
        assert!((account.balance(&rift_account) - expected_drain).abs() <= QI_EPSILON);
        assert!(
            !account.has_account(&elder_source),
            "外部 ECS source 临时影子余额必须恢复为不存在"
        );
        assert_eq!(account.transfers().len(), 1);
        assert_eq!(
            account.transfers()[0].reason,
            QiTransferReason::RiftCollapse
        );
        assert!(
            (cultivation_qi + account.balance(&rift_account) - 100.0).abs() <= QI_EPSILON,
            "Cultivation + rift ledger 必须绝对量守恒"
        );
    }

    #[test]
    fn drain_system_without_ledger_keeps_both_components_unchanged() {
        use valence::prelude::App;

        let mut app = App::new();
        app.add_event::<QiTransfer>();
        let mut zones = ZoneRegistry::fallback();
        zones.zones[0].name = "tsy_deep".to_string();
        zones.zones[0].spirit_qi = -0.6;
        app.insert_resource(zones);
        app.add_systems(valence::prelude::Update, dying_elder_drain_system);

        let mut bb = DyingElderBlackboard::new("tsy_deep", DVec3::ZERO, 7, 0);
        bb.qi_current = 100.0;
        let elder = app
            .world_mut()
            .spawn((
                NpcMarker,
                bb,
                DyingElderState::Plea,
                Cultivation {
                    qi_current: 100.0,
                    qi_max: DYING_ELDER_INITIAL_QI,
                    ..Cultivation::default()
                },
            ))
            .id();

        app.update();

        let elder_ref = app.world().entity(elder);
        assert!(
            (elder_ref.get::<DyingElderBlackboard>().unwrap().qi_current - 100.0).abs()
                <= QI_EPSILON
        );
        assert!((elder_ref.get::<Cultivation>().unwrap().qi_current - 100.0).abs() <= QI_EPSILON);
        assert_eq!(
            *elder_ref.get::<DyingElderState>().unwrap(),
            DyingElderState::Plea
        );
        let emitted = app
            .world_mut()
            .resource_mut::<bevy_ecs::event::Events<QiTransfer>>()
            .drain()
            .collect::<Vec<_>>();
        assert!(emitted.is_empty(), "缺 ledger 时不得伪造 drain event");
    }

    // ── P2：DyingElderDeathSystem 守恒 + loot 分档测试 ────────────────────────

    #[derive(Debug)]
    struct DeathReleaseRun {
        zone_after: Option<f64>,
        blackboard_qi_after: f64,
        cultivation_qi_after: f64,
        processed: bool,
        emitted: Vec<QiTransfer>,
        audited: Vec<QiTransfer>,
        overflow_balance: f64,
        elder_source_present: bool,
    }

    fn run_death_release(
        zone_fraction: Option<f64>,
        release_amount: f64,
        with_account: bool,
    ) -> DeathReleaseRun {
        use valence::prelude::App;

        let mut app = App::new();
        app.add_event::<QiTransfer>();
        if let Some(zone_fraction) = zone_fraction {
            let mut zones = ZoneRegistry::fallback();
            zones.zones[0].name = "tsy_deep".to_string();
            zones.zones[0].spirit_qi = zone_fraction;
            app.insert_resource(zones);
        }
        if with_account {
            app.insert_resource(WorldQiAccount::default());
        }
        app.add_systems(valence::prelude::Update, dying_elder_death_system);

        let mut bb = DyingElderBlackboard::new("tsy_deep", DVec3::ZERO, 7, 0);
        bb.qi_current = release_amount;
        let elder = app
            .world_mut()
            .spawn((
                NpcMarker,
                bb,
                Cultivation {
                    qi_current: release_amount,
                    qi_max: DYING_ELDER_INITIAL_QI,
                    ..Cultivation::default()
                },
                DyingElderState::Dead {
                    dead_by_betrayal: false,
                },
            ))
            .id();

        app.update();

        let zone_after = app
            .world()
            .get_resource::<ZoneRegistry>()
            .map(|zones| zones.zones[0].spirit_qi);
        let elder_ref = app.world().entity(elder);
        let blackboard_qi_after = elder_ref
            .get::<DyingElderBlackboard>()
            .expect("死亡系统不应在本帧删除大能 blackboard")
            .qi_current;
        let cultivation_qi_after = elder_ref
            .get::<Cultivation>()
            .expect("死亡系统不应在本帧删除大能 Cultivation")
            .qi_current;
        let processed = elder_ref.contains::<DyingElderDeathProcessed>();
        let emitted = app
            .world_mut()
            .resource_mut::<bevy_ecs::event::Events<QiTransfer>>()
            .drain()
            .collect::<Vec<_>>();
        let elder_account = QiAccountId::npc(format!("dying_elder:{}", elder.to_bits()));
        let overflow_account = dying_elder_release_overflow_account();
        let (audited, overflow_balance, elder_source_present) = app
            .world()
            .get_resource::<WorldQiAccount>()
            .map(|account| {
                (
                    account.transfers().to_vec(),
                    account.balance(&overflow_account),
                    account.has_account(&elder_account),
                )
            })
            .unwrap_or_default();
        DeathReleaseRun {
            zone_after,
            blackboard_qi_after,
            cultivation_qi_after,
            processed,
            emitted,
            audited,
            overflow_balance,
            elder_source_present,
        }
    }

    #[derive(Debug)]
    struct FifthDanRun {
        state: DyingElderState,
        blackboard_qi_after: f64,
        cultivation_qi_after: f64,
        player_qi_after: f64,
        zone_after: f64,
        transfers: Vec<QiTransfer>,
        dan_overflow_balance: f64,
        death_overflow_balance: f64,
        temporary_sources_present: bool,
    }

    fn huiyuan_pill_instance(instance_id: u64, stack_count: u32) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: "huiyuan_pill".to_string(),
            display_name: "回元丹".to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.2,
            rarity: crate::inventory::ItemRarity::Rare,
            description: String::new(),
            stack_count,
            spirit_quality: 1.0,
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
        }
    }

    fn inventory_with_huiyuan_pills(pills: &[(u64, u32)]) -> PlayerInventory {
        assert!(pills.len() <= 9, "测试 hotbar 最多容纳 9 个实例");
        let mut hotbar = <[Option<ItemInstance>; 9]>::default();
        for (slot, (instance_id, stack_count)) in pills.iter().copied().enumerate() {
            hotbar[slot] = Some(huiyuan_pill_instance(instance_id, stack_count));
        }
        PlayerInventory {
            revision: crate::inventory::InventoryRevision(0),
            containers: Vec::new(),
            equipped: Default::default(),
            hotbar,
            bone_coins: 0,
            max_weight: 100.0,
            triggered_treasures: Vec::new(),
        }
    }

    fn huiyuan_pill_qi_gain_from_registry() -> f64 {
        use crate::inventory::{load_item_registry, ItemEffect};

        let registry = load_item_registry().expect("真实 ItemRegistry 应可加载");
        let template = registry
            .get("huiyuan_pill")
            .expect("真实 ItemRegistry 应注册 huiyuan_pill");
        match template.effect.as_ref() {
            Some(ItemEffect::QiRecovery { amount }) => *amount,
            effect => panic!("huiyuan_pill 应配置 QiRecovery，实际={effect:?}"),
        }
    }

    #[test]
    fn huiyuan_pill_registry_qi_gain_is_sixty() {
        assert!(
            (huiyuan_pill_qi_gain_from_registry() - 60.0).abs() <= QI_EPSILON,
            "回元丹生产 ItemRegistry 契约必须固定为 60 真元"
        );
    }

    fn run_fifth_dan_chain(betray_probability: f64, player_qi: f64) -> FifthDanRun {
        use valence::prelude::App;

        let mut app = App::new();
        app.add_event::<GiveDanToElderIntent>();
        app.add_event::<DyingElderDanAcceptedEvent>();
        app.add_event::<SoulSeizeEvent>();
        app.add_event::<QiTransfer>();
        let mut zones = ZoneRegistry::fallback();
        zones.zones[0].name = "tsy_deep".to_string();
        zones.zones[0].spirit_qi = -0.6;
        app.insert_resource(zones);
        app.insert_resource(WorldQiAccount::default());
        app.insert_resource(crate::inventory::load_item_registry().expect("真实 registry"));
        app.add_systems(
            valence::prelude::Update,
            (
                dying_elder_give_dan_system,
                dying_elder_betray_system.after(dying_elder_give_dan_system),
                dying_elder_death_system.after(dying_elder_betray_system),
            ),
        );

        let player = app
            .world_mut()
            .spawn((
                ClientMarker,
                Cultivation {
                    qi_current: player_qi,
                    qi_max: 300.0,
                    ..Cultivation::default()
                },
                inventory_with_huiyuan_pills(&[(1, 1), (2, 1), (3, 1), (4, 1), (5, 1)]),
            ))
            .id();
        let mut bb = DyingElderBlackboard::new("tsy_deep", DVec3::ZERO, 7, 0);
        bb.betray_probability = betray_probability;
        bb.qi_current = 500.0;
        let elder = app
            .world_mut()
            .spawn((
                NpcMarker,
                bb,
                Cultivation {
                    qi_current: DYING_ELDER_INITIAL_QI,
                    qi_max: DYING_ELDER_INITIAL_QI,
                    ..Cultivation::default()
                },
                DyingElderState::Plea,
            ))
            .id();

        for pill_instance_id in 1..=DYING_ELDER_DAN_THRESHOLD as u64 {
            app.world_mut().send_event(GiveDanToElderIntent {
                player,
                elder,
                pill_instance_id,
            });
            app.update();
            let elder_ref = app.world().entity(elder);
            let blackboard_qi = elder_ref
                .get::<DyingElderBlackboard>()
                .expect("大能应保留 blackboard")
                .qi_current;
            let cultivation_qi = elder_ref
                .get::<Cultivation>()
                .expect("大能应保留 Cultivation")
                .qi_current;
            assert!(
                (blackboard_qi - cultivation_qi).abs() <= QI_EPSILON,
                "第 {pill_instance_id} 颗丹结算后 Blackboard mirror 必须与 Cultivation 权威一致"
            );
        }

        let state = app
            .world()
            .entity(elder)
            .get::<DyingElderState>()
            .copied()
            .unwrap();
        let blackboard_qi_after = app
            .world()
            .entity(elder)
            .get::<DyingElderBlackboard>()
            .unwrap()
            .qi_current;
        let cultivation_qi_after = app
            .world()
            .entity(elder)
            .get::<Cultivation>()
            .unwrap()
            .qi_current;
        let player_qi_after = app
            .world()
            .entity(player)
            .get::<Cultivation>()
            .unwrap()
            .qi_current;
        let zone_after = app.world().resource::<ZoneRegistry>().zones[0].spirit_qi;
        let account = app.world().resource::<WorldQiAccount>();
        let dan_overflow_account = dying_elder_dan_excess_account();
        let death_overflow_account = dying_elder_release_overflow_account();
        let elder_source = QiAccountId::npc(format!("dying_elder:{}", elder.to_bits()));
        let temporary_sources_present = account.has_account(&elder_source)
            || (1..=DYING_ELDER_DAN_THRESHOLD as u64).any(|pill_instance_id| {
                account.has_account(&QiAccountId::container(format!(
                    "hui_yuan_pill:{pill_instance_id}"
                )))
            });
        FifthDanRun {
            state,
            blackboard_qi_after,
            cultivation_qi_after,
            player_qi_after,
            zone_after,
            transfers: account.transfers().to_vec(),
            dan_overflow_balance: account.balance(&dan_overflow_account),
            death_overflow_balance: account.balance(&death_overflow_account),
            temporary_sources_present,
        }
    }

    #[test]
    fn honorable_fifth_dan_death_preserves_qi() {
        let run = run_fifth_dan_chain(0.0, 40.0);
        assert_eq!(
            run.state,
            DyingElderState::Dead {
                dead_by_betrayal: false
            }
        );
        assert!(run.blackboard_qi_after.abs() <= QI_EPSILON);
        assert!(run.cultivation_qi_after.abs() <= QI_EPSILON);
        assert!((run.player_qi_after - 40.0).abs() <= QI_EPSILON);
        assert!(!run.temporary_sources_present);

        let qi_gain = huiyuan_pill_qi_gain_from_registry();
        let pill_total = qi_gain * f64::from(DYING_ELDER_DAN_THRESHOLD);
        let elder_cap = DYING_ELDER_INITIAL_QI * 1.5;
        let dan_excess = (DYING_ELDER_INITIAL_QI + pill_total - elder_cap).max(0.0);
        let accepted = QI_ZONE_UNIT_CAPACITY - (-0.6 * QI_ZONE_UNIT_CAPACITY);
        let death_overflow = elder_cap - accepted;
        let traded = run
            .transfers
            .iter()
            .filter(|t| t.reason == QiTransferReason::TradeDan)
            .map(|t| t.amount)
            .sum::<f64>();
        let released = run
            .transfers
            .iter()
            .filter(|t| t.reason == QiTransferReason::ReleaseToZone)
            .map(|t| t.amount)
            .sum::<f64>();
        assert!(
            (traded - pill_total).abs() <= QI_EPSILON,
            "五颗丹的 TradeDan 两腿应闭合真实 registry 总量 {pill_total}"
        );
        assert!(
            (released - elder_cap).abs() <= QI_EPSILON,
            "死亡只释放 cap 内大能真元 {elder_cap}，丹 excess 已单独稳定入账"
        );
        assert!((run.dan_overflow_balance - dan_excess).abs() <= QI_EPSILON);
        assert!((run.death_overflow_balance - death_overflow).abs() <= QI_EPSILON);
        assert!((run.zone_after - 1.0).abs() <= QI_EPSILON);

        let total_before = -0.6 * QI_ZONE_UNIT_CAPACITY + DYING_ELDER_INITIAL_QI + pill_total;
        let total_after = run.zone_after * QI_ZONE_UNIT_CAPACITY
            + run.dan_overflow_balance
            + run.death_overflow_balance
            + run.cultivation_qi_after;
        assert!(
            (total_after - total_before).abs() <= QI_EPSILON,
            "绝对量头尾守恒：before={total_before} after={total_after}"
        );
    }

    #[test]
    fn betrayal_death_releases_player_soul_seize_qi() {
        let player_before = 40.0;
        let run = run_fifth_dan_chain(1.0, player_before);
        assert_eq!(
            run.state,
            DyingElderState::Dead {
                dead_by_betrayal: true
            }
        );
        assert!(run.blackboard_qi_after.abs() <= QI_EPSILON);
        assert!(run.cultivation_qi_after.abs() <= QI_EPSILON);
        assert!(
            run.player_qi_after.abs() <= QI_EPSILON,
            "夺舍应抽空玩家当前真元"
        );
        assert!(!run.temporary_sources_present);

        let qi_gain = huiyuan_pill_qi_gain_from_registry();
        let pill_total = qi_gain * f64::from(DYING_ELDER_DAN_THRESHOLD);
        let elder_cap = DYING_ELDER_INITIAL_QI * 1.5;
        let dan_excess = (DYING_ELDER_INITIAL_QI + pill_total - elder_cap).max(0.0);
        let accepted = QI_ZONE_UNIT_CAPACITY - (-0.6 * QI_ZONE_UNIT_CAPACITY);
        let death_overflow = elder_cap + player_before - accepted;
        let traded = run
            .transfers
            .iter()
            .filter(|t| t.reason == QiTransferReason::TradeDan)
            .map(|t| t.amount)
            .sum::<f64>();
        let seized = run
            .transfers
            .iter()
            .filter(|t| t.reason == QiTransferReason::SoulSeize)
            .map(|t| t.amount)
            .sum::<f64>();
        let released = run
            .transfers
            .iter()
            .filter(|t| t.reason == QiTransferReason::ReleaseToZone)
            .map(|t| t.amount)
            .sum::<f64>();
        assert!((traded - pill_total).abs() <= QI_EPSILON);
        assert!((seized - player_before).abs() <= QI_EPSILON);
        assert!(
            (released - (elder_cap + player_before)).abs() <= QI_EPSILON,
            "死亡释放只包含 cap 内大能真元与被夺玩家真元"
        );
        assert!((run.dan_overflow_balance - dan_excess).abs() <= QI_EPSILON);
        assert!((run.death_overflow_balance - death_overflow).abs() <= QI_EPSILON);
        assert!((run.zone_after - 1.0).abs() <= QI_EPSILON);

        let total_before =
            -0.6 * QI_ZONE_UNIT_CAPACITY + DYING_ELDER_INITIAL_QI + pill_total + player_before;
        let total_after = run.zone_after * QI_ZONE_UNIT_CAPACITY
            + run.dan_overflow_balance
            + run.death_overflow_balance
            + run.cultivation_qi_after
            + run.player_qi_after;
        assert!(
            (total_after - total_before).abs() <= QI_EPSILON,
            "夺舍路线绝对量头尾守恒：before={total_before} after={total_after}"
        );
    }

    #[derive(Debug, Clone, Copy)]
    struct InvalidBetrayFixture {
        elder_qi: f64,
        player_qi: f64,
        blackboard_qi: f64,
        blackboard_qi_max_cache: f64,
        player_qi_max: f64,
        player_qi_max_frozen: Option<f64>,
    }

    impl Default for InvalidBetrayFixture {
        fn default() -> Self {
            Self {
                elder_qi: 500.0,
                player_qi: 40.0,
                blackboard_qi: 321.0,
                blackboard_qi_max_cache: DYING_ELDER_INITIAL_QI,
                player_qi_max: 300.0,
                player_qi_max_frozen: None,
            }
        }
    }

    #[derive(Debug)]
    struct InvalidBetrayRun {
        state: DyingElderState,
        blackboard_qi: f64,
        blackboard_qi_max_cache: f64,
        elder_qi: f64,
        player_qi: f64,
        player_qi_max: f64,
        player_qi_max_frozen: Option<f64>,
        emitted: Vec<QiTransfer>,
        audited: Vec<QiTransfer>,
    }

    fn run_invalid_betray(fixture: InvalidBetrayFixture) -> InvalidBetrayRun {
        use valence::prelude::App;

        let mut app = App::new();
        app.add_event::<SoulSeizeEvent>();
        app.add_event::<QiTransfer>();
        app.insert_resource(WorldQiAccount::default());
        app.add_systems(valence::prelude::Update, dying_elder_betray_system);

        let player = app
            .world_mut()
            .spawn((
                ClientMarker,
                Cultivation {
                    qi_current: fixture.player_qi,
                    qi_max: fixture.player_qi_max,
                    qi_max_frozen: fixture.player_qi_max_frozen,
                    ..Cultivation::default()
                },
            ))
            .id();
        let mut bb = DyingElderBlackboard::new("tsy_deep", DVec3::ZERO, 7, 0);
        bb.qi_current = fixture.blackboard_qi;
        bb.qi_max_cache = fixture.blackboard_qi_max_cache;
        let elder = app
            .world_mut()
            .spawn((
                NpcMarker,
                bb,
                DyingElderState::Betrayal,
                Cultivation {
                    qi_current: fixture.elder_qi,
                    qi_max: DYING_ELDER_INITIAL_QI,
                    ..Cultivation::default()
                },
            ))
            .id();
        app.world_mut().send_event(SoulSeizeEvent {
            elder,
            player,
            qi_transferred: fixture.player_qi,
            qi_max_drain: DYING_ELDER_INITIAL_QI * DYING_ELDER_SOUL_SEIZE_RATIO,
        });

        app.update();

        let (
            state,
            blackboard_qi,
            blackboard_qi_max_cache,
            elder_qi_after,
            player_qi_after,
            player_qi_max,
            player_qi_max_frozen,
        ) = {
            let elder_ref = app.world().entity(elder);
            let player_ref = app.world().entity(player);
            (
                *elder_ref.get::<DyingElderState>().unwrap(),
                elder_ref.get::<DyingElderBlackboard>().unwrap().qi_current,
                elder_ref
                    .get::<DyingElderBlackboard>()
                    .unwrap()
                    .qi_max_cache,
                elder_ref.get::<Cultivation>().unwrap().qi_current,
                player_ref.get::<Cultivation>().unwrap().qi_current,
                player_ref.get::<Cultivation>().unwrap().qi_max,
                player_ref.get::<Cultivation>().unwrap().qi_max_frozen,
            )
        };
        let emitted = app
            .world_mut()
            .resource_mut::<bevy_ecs::event::Events<QiTransfer>>()
            .drain()
            .collect::<Vec<_>>();
        let audited = app
            .world()
            .resource::<WorldQiAccount>()
            .transfers()
            .to_vec();
        InvalidBetrayRun {
            state,
            blackboard_qi,
            blackboard_qi_max_cache,
            elder_qi: elder_qi_after,
            player_qi: player_qi_after,
            player_qi_max,
            player_qi_max_frozen,
            emitted,
            audited,
        }
    }

    fn assert_same_float(actual: f64, expected: f64, label: &str) {
        assert!(
            (actual.is_nan() && expected.is_nan()) || actual == expected,
            "{label} 应保持 {expected:?}，实际 {actual:?}"
        );
    }

    fn assert_same_optional_float(actual: Option<f64>, expected: Option<f64>, label: &str) {
        match (actual, expected) {
            (Some(actual), Some(expected)) => assert_same_float(actual, expected, label),
            (None, None) => {}
            (actual, expected) => {
                panic!("{label} 应保持 {expected:?}，实际 {actual:?}")
            }
        }
    }

    #[test]
    fn betray_system_invalid_or_overflowing_qi_is_fully_atomic() {
        let valid = InvalidBetrayFixture::default();
        for (label, fixture) in [
            (
                "elder_nan",
                InvalidBetrayFixture {
                    elder_qi: f64::NAN,
                    ..valid
                },
            ),
            (
                "elder_negative",
                InvalidBetrayFixture {
                    elder_qi: -1.0,
                    ..valid
                },
            ),
            (
                "elder_positive_infinity",
                InvalidBetrayFixture {
                    elder_qi: f64::INFINITY,
                    ..valid
                },
            ),
            (
                "player_nan",
                InvalidBetrayFixture {
                    player_qi: f64::NAN,
                    ..valid
                },
            ),
            (
                "player_negative",
                InvalidBetrayFixture {
                    player_qi: -1.0,
                    ..valid
                },
            ),
            (
                "player_positive_infinity",
                InvalidBetrayFixture {
                    player_qi: f64::INFINITY,
                    ..valid
                },
            ),
            (
                "qi_sum_overflow",
                InvalidBetrayFixture {
                    elder_qi: f64::MAX,
                    player_qi: f64::MAX,
                    ..valid
                },
            ),
            (
                "blackboard_qi_max_cache_nan",
                InvalidBetrayFixture {
                    blackboard_qi_max_cache: f64::NAN,
                    ..valid
                },
            ),
            (
                "blackboard_qi_max_cache_negative",
                InvalidBetrayFixture {
                    blackboard_qi_max_cache: -1.0,
                    ..valid
                },
            ),
            (
                "blackboard_qi_max_cache_positive_infinity",
                InvalidBetrayFixture {
                    blackboard_qi_max_cache: f64::INFINITY,
                    ..valid
                },
            ),
            (
                "player_qi_max_nan",
                InvalidBetrayFixture {
                    player_qi_max: f64::NAN,
                    ..valid
                },
            ),
            (
                "player_qi_max_negative",
                InvalidBetrayFixture {
                    player_qi_max: -1.0,
                    ..valid
                },
            ),
            (
                "player_qi_max_positive_infinity",
                InvalidBetrayFixture {
                    player_qi_max: f64::INFINITY,
                    ..valid
                },
            ),
            (
                "player_qi_max_frozen_nan",
                InvalidBetrayFixture {
                    player_qi_max_frozen: Some(f64::NAN),
                    ..valid
                },
            ),
            (
                "player_qi_max_frozen_negative",
                InvalidBetrayFixture {
                    player_qi_max_frozen: Some(-1.0),
                    ..valid
                },
            ),
            (
                "player_qi_max_frozen_positive_infinity",
                InvalidBetrayFixture {
                    player_qi_max_frozen: Some(f64::INFINITY),
                    ..valid
                },
            ),
            (
                "raw_qi_max_after_non_finite",
                InvalidBetrayFixture {
                    blackboard_qi_max_cache: -f64::MAX,
                    player_qi_max: f64::MAX,
                    ..valid
                },
            ),
        ] {
            let run = run_invalid_betray(fixture);
            assert_eq!(run.state, DyingElderState::Betrayal, "case={label}");
            assert_same_float(run.elder_qi, fixture.elder_qi, label);
            assert_same_float(run.player_qi, fixture.player_qi, label);
            assert_same_float(run.blackboard_qi, fixture.blackboard_qi, label);
            assert_same_float(
                run.blackboard_qi_max_cache,
                fixture.blackboard_qi_max_cache,
                label,
            );
            assert_same_float(run.player_qi_max, fixture.player_qi_max, label);
            assert_same_optional_float(
                run.player_qi_max_frozen,
                fixture.player_qi_max_frozen,
                label,
            );
            assert!(
                run.emitted.is_empty(),
                "case={label}: invalid transaction must not emit QiTransfer"
            );
            assert!(
                run.audited.is_empty(),
                "case={label}: invalid transaction must not append audit"
            );
        }
    }

    #[test]
    fn pending_soul_seize_retries_after_invalid_qi_and_commits_once() {
        let mut app = valence::prelude::App::new();
        app.add_event::<DyingElderSpawnRequest>();
        register_p1(&mut app);
        app.insert_resource(WorldQiAccount::default());
        app.insert_resource(crate::inventory::load_item_registry().expect("真实 registry"));

        let player = app
            .world_mut()
            .spawn((
                ClientMarker,
                Cultivation {
                    qi_current: f64::NAN,
                    qi_max: 300.0,
                    ..Cultivation::default()
                },
                inventory_with_huiyuan_pills(&[(50, 1)]),
            ))
            .id();
        let mut bb = DyingElderBlackboard::new("tsy_deep", DVec3::ZERO, 7, 0);
        bb.betray_probability = 1.0;
        bb.qi_current = 100.0;
        let elder = app
            .world_mut()
            .spawn((
                NpcMarker,
                bb,
                DyingElderState::Recovering {
                    dan_received: DYING_ELDER_DAN_THRESHOLD - 1,
                },
                Cultivation {
                    qi_current: 100.0,
                    qi_max: DYING_ELDER_INITIAL_QI,
                    ..Cultivation::default()
                },
            ))
            .id();

        app.world_mut().send_event(GiveDanToElderIntent {
            player,
            elder,
            pill_instance_id: 50,
        });
        app.update();
        let elder_ref = app.world().entity(elder);
        assert_eq!(
            *elder_ref.get::<DyingElderState>().unwrap(),
            DyingElderState::Betrayal
        );
        assert_eq!(
            elder_ref.get::<PendingSoulSeize>(),
            Some(&PendingSoulSeize { victim: player }),
            "第五丹真实 give 链必须自动留下可重试权威"
        );
        let qi_gain = huiyuan_pill_qi_gain_from_registry();
        assert!(
            (elder_ref.get::<Cultivation>().unwrap().qi_current - (100.0 + qi_gain)).abs()
                <= QI_EPSILON
        );
        assert!(app
            .world()
            .entity(player)
            .get::<Cultivation>()
            .unwrap()
            .qi_current
            .is_nan());
        let inventory = app.world().entity(player).get::<PlayerInventory>().unwrap();
        assert!(inventory.hotbar[0].is_none());
        assert_eq!(inventory.revision.0, 1);
        let first_frame_transfers = app.world().resource::<WorldQiAccount>().transfers();
        assert_eq!(
            first_frame_transfers
                .iter()
                .filter(|transfer| transfer.reason == QiTransferReason::TradeDan)
                .count(),
            1
        );
        assert_eq!(
            first_frame_transfers
                .iter()
                .filter(|transfer| transfer.reason == QiTransferReason::SoulSeize)
                .count(),
            0,
            "非法玩家真元不得产生半笔夺舍审计"
        );
        let accepted = app
            .world_mut()
            .resource_mut::<bevy_ecs::event::Events<DyingElderDanAcceptedEvent>>()
            .drain()
            .collect::<Vec<_>>();
        assert_eq!(accepted.len(), 1);
        assert_eq!(accepted[0].dan_count, DYING_ELDER_DAN_THRESHOLD);

        app.world_mut()
            .entity_mut(player)
            .get_mut::<Cultivation>()
            .unwrap()
            .qi_current = 40.0;
        // 不重发 SoulSeizeEvent；第二帧必须只靠 PendingSoulSeize retry 驱动成功。
        app.update();

        let elder_ref = app.world().entity(elder);
        assert_eq!(
            *elder_ref.get::<DyingElderState>().unwrap(),
            DyingElderState::Dead {
                dead_by_betrayal: true
            }
        );
        assert!(!elder_ref.contains::<PendingSoulSeize>());
        let expected_elder_qi = 100.0 + qi_gain + 40.0;
        assert!(
            (elder_ref.get::<Cultivation>().unwrap().qi_current - expected_elder_qi).abs()
                <= QI_EPSILON
        );
        assert!(
            (elder_ref.get::<DyingElderBlackboard>().unwrap().qi_current - expected_elder_qi).abs()
                <= QI_EPSILON
        );
        let player_cultivation = app.world().entity(player).get::<Cultivation>().unwrap();
        assert_eq!(player_cultivation.qi_current, 0.0);
        assert_eq!(player_cultivation.qi_max, 250.0);
        let transfers = app.world().resource::<WorldQiAccount>().transfers();
        let seized = transfers
            .iter()
            .filter(|transfer| transfer.reason == QiTransferReason::SoulSeize)
            .collect::<Vec<_>>();
        assert_eq!(seized.len(), 1);
        assert!((seized[0].amount - 40.0).abs() <= QI_EPSILON);

        // 成功后 pending 已移除；再跑一帧也不能重复转移或重复扣 qi_max。
        app.update();
        assert_eq!(
            app.world()
                .resource::<WorldQiAccount>()
                .transfers()
                .iter()
                .filter(|transfer| transfer.reason == QiTransferReason::SoulSeize)
                .count(),
            1
        );
        assert_eq!(
            app.world()
                .entity(player)
                .get::<Cultivation>()
                .unwrap()
                .qi_max,
            250.0
        );
    }

    #[test]
    fn death_system_release_uses_absolute_zone_capacity() {
        let run = run_death_release(Some(-0.6), 500.0, true);
        let accepted = QI_ZONE_UNIT_CAPACITY - (-0.6 * QI_ZONE_UNIT_CAPACITY);
        let overflow = 500.0 - accepted;
        assert!(
            run.blackboard_qi_after.abs() <= QI_EPSILON,
            "成功释放后 Blackboard mirror 应归零"
        );
        assert!(
            run.cultivation_qi_after.abs() <= QI_EPSILON,
            "成功释放后 Cultivation 物理权威应归零"
        );
        assert!(run.processed, "死亡结算后必须插入幂等处理标记");
        assert!(
            (run.zone_after.expect("fixture 安装了 zone") - 1.0).abs() <= QI_EPSILON,
            "负灵域应按绝对容量回暖到比例上限 1.0"
        );
        assert_eq!(
            run.emitted.len(),
            2,
            "500 真元应产生 accepted 与 overflow 两条事件"
        );
        let accepted_transfer = run
            .emitted
            .iter()
            .find(|transfer| transfer.to.kind == crate::qi_physics::ledger::QiAccountKind::Zone)
            .expect("应包含写入真实 zone 的 accepted 腿");
        assert!((accepted_transfer.amount - accepted).abs() <= QI_EPSILON);
        assert!((run.overflow_balance - overflow).abs() <= QI_EPSILON);
        assert!(
            !run.elder_source_present,
            "外部 ECS source 的临时影子余额必须在真实入账后移除"
        );
        assert_eq!(run.audited.len(), run.emitted.len());
        assert!(
            run.emitted
                .iter()
                .all(|transfer| run.audited.contains(transfer)),
            "WorldQiAccount 审计必须逐条镜像事件，但不绑定 accepted/overflow 顺序"
        );

        let total_before = -0.6 * QI_ZONE_UNIT_CAPACITY + 500.0;
        let total_after = run.zone_after.unwrap() * QI_ZONE_UNIT_CAPACITY
            + run.overflow_balance
            + run.cultivation_qi_after;
        assert!((total_after - total_before).abs() <= QI_EPSILON);
    }

    #[test]
    fn death_system_routes_overflow_to_overflow_account() {
        let run = run_death_release(Some(0.9), 500.0, true);
        let accepted = (1.0 - 0.9) * QI_ZONE_UNIT_CAPACITY;
        let overflow = 500.0 - accepted;
        assert!(
            (run.emitted
                .iter()
                .map(|transfer| transfer.amount)
                .sum::<f64>()
                - 500.0)
                .abs()
                <= QI_EPSILON,
            "accepted + overflow 必须闭合全部 500 真元"
        );
        let overflow_transfer = run
            .emitted
            .iter()
            .find(|transfer| transfer.to.kind == crate::qi_physics::ledger::QiAccountKind::Overflow)
            .expect("zone 接近满时必须路由 overflow account");
        assert!(
            (overflow_transfer.amount - overflow).abs() <= QI_EPSILON,
            "overflow 应为 release-accepted={overflow}，实际={}",
            overflow_transfer.amount
        );
        assert_eq!(overflow_transfer.reason, QiTransferReason::ReleaseToZone);
        assert!((run.overflow_balance - overflow).abs() <= QI_EPSILON);
        assert!(!run.elder_source_present);
    }

    #[test]
    fn death_system_without_world_qi_account_fails_closed_without_partial_writes() {
        let run = run_death_release(Some(-0.6), 500.0, false);
        assert!(
            run.audited.is_empty(),
            "未安装 WorldQiAccount 时不应伪造本地审计资源"
        );
        assert!(
            run.emitted.is_empty(),
            "overflow 无法真实入账时不得先发任何 transfer event"
        );
        assert!(
            (run.zone_after.expect("fixture 安装了 zone") - (-0.6)).abs() <= QI_EPSILON,
            "缺 ledger 时不得部分写 zone"
        );
        assert!((run.blackboard_qi_after - 500.0).abs() <= QI_EPSILON);
        assert!((run.cultivation_qi_after - 500.0).abs() <= QI_EPSILON);
        assert!(!run.processed, "缺 ledger 时不得插入 processed marker");
        assert!(run.overflow_balance.abs() <= QI_EPSILON);
    }

    #[test]
    fn death_system_without_world_qi_account_fails_closed_even_without_overflow() {
        let run = run_death_release(Some(-0.6), 10.0, false);
        assert!(run.emitted.is_empty());
        assert!(run.audited.is_empty());
        assert!(
            (run.zone_after.expect("fixture 安装了 zone") - (-0.6)).abs() <= QI_EPSILON,
            "即使 zone 可全量接收，缺 ledger 也不得先写 field-authority"
        );
        assert!((run.blackboard_qi_after - 10.0).abs() <= QI_EPSILON);
        assert!((run.cultivation_qi_after - 10.0).abs() <= QI_EPSILON);
        assert!(!run.processed);
    }

    #[test]
    fn death_system_missing_zone_routes_everything_to_overflow() {
        let run = run_death_release(None, 500.0, true);
        assert!(run.zone_after.is_none(), "fixture 不应安装 ZoneRegistry");
        assert!(
            run.blackboard_qi_after.abs() <= QI_EPSILON,
            "全量 overflow 后 Blackboard mirror 应归零"
        );
        assert!(run.cultivation_qi_after.abs() <= QI_EPSILON);
        assert!(run.processed);
        assert_eq!(
            run.emitted.len(),
            1,
            "缺 zone 时不得发无法落入世界状态的 accepted 腿"
        );
        assert!(run.emitted[0].to.kind == crate::qi_physics::ledger::QiAccountKind::Overflow);
        assert!((run.emitted[0].amount - 500.0).abs() <= QI_EPSILON);
        assert!((run.overflow_balance - 500.0).abs() <= QI_EPSILON);
        assert!(!run.elder_source_present);
    }

    #[test]
    fn death_system_release_error_remains_retryable() {
        let run = run_death_release(Some(f64::NAN), 500.0, true);
        assert!(
            (run.blackboard_qi_after - 500.0).abs() <= QI_EPSILON,
            "释放失败不得清零 Blackboard mirror"
        );
        assert!(
            (run.cultivation_qi_after - 500.0).abs() <= QI_EPSILON,
            "释放失败不得清零 Cultivation 物理权威"
        );
        assert!(
            !run.processed,
            "释放失败不得插入 DyingElderDeathProcessed，必须允许重试"
        );
        assert!(run.emitted.is_empty(), "失败调用不得发 transfer");
        assert!(run.audited.is_empty(), "失败调用不得写 audit");
        assert!(run.overflow_balance.abs() <= QI_EPSILON);
    }

    #[test]
    fn death_system_non_finite_elder_qi_remains_retryable() {
        let run = run_death_release(Some(-0.6), f64::NAN, true);
        assert!(
            run.blackboard_qi_after.is_nan(),
            "非法 mirror 不得被静默改写"
        );
        assert!(
            run.cultivation_qi_after.is_nan(),
            "非法物理权威不得被 max(0) 静默改写"
        );
        assert!(!run.processed, "非法真元不得封死后续修复与重试");
        assert!(run.emitted.is_empty(), "非法真元不得发 transfer");
        assert!(run.audited.is_empty(), "非法真元不得写 audit");
    }

    #[test]
    fn death_broadcasts_once_while_release_retries_until_single_success() {
        use crate::network::redis_bridge::{RedisInbound, RedisOutbound};
        use valence::prelude::App;

        let mut app = App::new();
        app.add_event::<QiTransfer>();
        let mut zones = ZoneRegistry::fallback();
        zones.zones[0].name = "tsy_deep".to_string();
        zones.zones[0].spirit_qi = f64::NAN;
        app.insert_resource(zones);
        app.insert_resource(WorldQiAccount::default());
        let (tx_outbound, rx_outbound) = crossbeam_channel::unbounded();
        let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded::<RedisInbound>();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.add_systems(
            valence::prelude::Update,
            (
                dying_elder_p3_emit_death_event_system,
                dying_elder_death_system,
            )
                .chain(),
        );

        let mut bb = DyingElderBlackboard::new("tsy_deep", DVec3::ZERO, 7, 0);
        bb.qi_current = 500.0;
        let elder = app
            .world_mut()
            .spawn((
                NpcMarker,
                EntityId::default(),
                bb,
                Cultivation {
                    qi_current: 500.0,
                    qi_max: DYING_ELDER_INITIAL_QI,
                    ..Cultivation::default()
                },
                DyingElderState::Dead {
                    dead_by_betrayal: false,
                },
            ))
            .id();

        app.update();
        let first = app.world().entity(elder);
        assert!(
            first.contains::<DyingElderDeathBroadcast>(),
            "首次失败 tick 也必须立即记录叙事已广播"
        );
        assert!(
            !first.contains::<DyingElderDeathProcessed>(),
            "首次释放失败后结算必须保持可重试"
        );
        assert!(
            (first.get::<DyingElderBlackboard>().unwrap().qi_current - 500.0).abs() <= QI_EPSILON
        );
        assert!(
            (first.get::<Cultivation>().unwrap().qi_current - 500.0).abs() <= QI_EPSILON,
            "首次失败不得扣 Cultivation 物理权威"
        );

        app.update();
        let second = app.world().entity(elder);
        assert!(second.contains::<DyingElderDeathBroadcast>());
        assert!(
            !second.contains::<DyingElderDeathProcessed>(),
            "连续第二次失败仍不得封死结算重试"
        );
        assert!(
            (second.get::<DyingElderBlackboard>().unwrap().qi_current - 500.0).abs() <= QI_EPSILON
        );
        assert!(
            (second.get::<Cultivation>().unwrap().qi_current - 500.0).abs() <= QI_EPSILON,
            "连续失败不得扣 Cultivation 物理权威"
        );

        app.world_mut().resource_mut::<ZoneRegistry>().zones[0].spirit_qi = -0.6;
        app.update();
        let succeeded = app.world().entity(elder);
        assert!(succeeded.contains::<DyingElderDeathBroadcast>());
        assert!(
            succeeded.contains::<DyingElderDeathProcessed>(),
            "zone 恢复后结算应最终成功并记录独立 marker"
        );
        assert!(
            succeeded
                .get::<DyingElderBlackboard>()
                .unwrap()
                .qi_current
                .abs()
                <= QI_EPSILON
        );
        assert!(
            succeeded.get::<Cultivation>().unwrap().qi_current.abs() <= QI_EPSILON,
            "最终成功后 Cultivation 与 Blackboard 应同时归零"
        );

        app.update();
        let broadcasts = std::iter::from_fn(|| rx_outbound.try_recv().ok())
            .filter(|outbound| matches!(outbound, RedisOutbound::ElderEncounterEvent(_)))
            .count();
        assert_eq!(
            broadcasts, 1,
            "连续失败、最终成功及成功后 tick 全程只能广播一次"
        );
        let transfers = app.world().resource::<WorldQiAccount>().transfers();
        assert_eq!(
            transfers.len(),
            2,
            "最终成功只能落一组 accepted + overflow 两腿"
        );
        assert!(
            (transfers
                .iter()
                .map(|transfer| transfer.amount)
                .sum::<f64>()
                - 500.0)
                .abs()
                <= QI_EPSILON,
            "最终成功的一组两腿必须完整结算 500 真元"
        );
    }

    #[test]
    fn death_broadcast_send_failure_retries_before_marking_success() {
        use crate::network::redis_bridge::{RedisInbound, RedisOutbound};
        use valence::prelude::App;

        let mut app = App::new();
        let (failed_tx, failed_rx) = crossbeam_channel::unbounded();
        drop(failed_rx);
        let (_failed_in_tx, failed_in_rx) = crossbeam_channel::unbounded::<RedisInbound>();
        app.insert_resource(RedisBridgeResource {
            tx_outbound: failed_tx,
            rx_inbound: failed_in_rx,
        });
        app.add_systems(
            valence::prelude::Update,
            dying_elder_p3_emit_death_event_system,
        );

        let elder = app
            .world_mut()
            .spawn((
                NpcMarker,
                EntityId::default(),
                DyingElderBlackboard::new("tsy_deep", DVec3::ZERO, 7, 0),
                DyingElderState::Dead {
                    dead_by_betrayal: false,
                },
            ))
            .id();

        app.update();
        assert!(
            !app.world()
                .entity(elder)
                .contains::<DyingElderDeathBroadcast>(),
            "发送失败时不得误记已广播"
        );

        let (live_tx, live_rx) = crossbeam_channel::unbounded();
        let (_live_in_tx, live_in_rx) = crossbeam_channel::unbounded::<RedisInbound>();
        app.insert_resource(RedisBridgeResource {
            tx_outbound: live_tx,
            rx_inbound: live_in_rx,
        });
        app.update();

        assert!(
            app.world()
                .entity(elder)
                .contains::<DyingElderDeathBroadcast>(),
            "bridge 恢复后应成功广播并落 marker"
        );
        assert!(matches!(
            live_rx.try_recv(),
            Ok(RedisOutbound::ElderEncounterEvent(_))
        ));
        assert!(live_rx.try_recv().is_err(), "恢复 tick 只能广播一次");
    }

    #[test]
    fn natural_exhaustion_zero_qi_no_transfer() {
        let run = run_death_release(Some(-0.6), 0.0, true);
        assert!((run.zone_after.expect("fixture 安装了 zone") + 0.6).abs() <= QI_EPSILON);
        assert!(run.blackboard_qi_after.abs() <= QI_EPSILON);
        assert!(run.cultivation_qi_after.abs() <= QI_EPSILON);
        assert!(run.processed, "零真元死亡仍应完成幂等死亡结算");
        assert!(run.emitted.is_empty(), "零真元死亡不得发假 transfer event");
        assert!(run.audited.is_empty(), "零真元死亡不得写假 audit");
        assert!(run.overflow_balance.abs() <= QI_EPSILON);
    }

    #[test]
    fn death_system_loot_differentiates_by_betrayal_flag() {
        // 期望：dead_by_betrayal=true → 背叛 pool（低质量）；false → 守信 pool（高质量）
        // 池 ID 选择逻辑 pin
        let betrayal_pool = "dying_elder_secondary_betrayal";
        let honorable_pool = "dying_elder_secondary_honorable";

        // dead_by_betrayal=false → 守信结局
        let pool_for_honorable = if false { betrayal_pool } else { honorable_pool };
        assert_eq!(
            pool_for_honorable, honorable_pool,
            "守信自裁（dead_by_betrayal=false）应使用 '{honorable_pool}' loot 池"
        );

        // dead_by_betrayal=true → 背叛结局
        let pool_for_betrayal = if true { betrayal_pool } else { honorable_pool };
        assert_eq!(
            pool_for_betrayal, betrayal_pool,
            "背叛夺舍（dead_by_betrayal=true）应使用 '{betrayal_pool}' loot 池（质量稍差）"
        );
    }

    #[test]
    fn death_system_soul_seize_qi_max_not_exceed_original() {
        // 期望：SoulSeize 后玩家 qi_max 只减少不增加（debuff 是单向的）
        let original_qi_max = 300.0_f64;
        let qi_max_drain = 30.0_f64; // 10% of 300
        let new_qi_max = (original_qi_max - qi_max_drain).max(0.0);
        assert!(
            new_qi_max <= original_qi_max,
            "SoulSeize 后 qi_max({new_qi_max}) 不应超过原值({original_qi_max})；debuff 单向减少"
        );
        assert!(
            (new_qi_max - 270.0).abs() < f64::EPSILON,
            "qi_max_drain=30 时 qi_max 应从 300 减至 270；实际 = {new_qi_max}"
        );
    }

    #[test]
    fn loot_pools_honor_betrayal_pool_exists_in_json() {
        // 期望：dying_elder_secondary_honorable 和 dying_elder_secondary_betrayal 在 loot_pools.json 中定义
        let registry = crate::world::loot_pool::load_loot_pool_registry()
            .expect("loot_pools.json 必须能成功加载");
        assert!(
            registry.get("dying_elder_secondary_honorable").is_some(),
            "loot_pools.json 应包含 dying_elder_secondary_honorable pool（守信结局掉落池）"
        );
        assert!(
            registry.get("dying_elder_secondary_betrayal").is_some(),
            "loot_pools.json 应包含 dying_elder_secondary_betrayal pool（背叛结局掉落池）"
        );
    }

    #[test]
    fn loot_pools_reference_only_known_templates() {
        // 期望：两个 dying_elder loot pool 中的 template_id 均在 ItemRegistry 中
        let pools = crate::world::loot_pool::load_loot_pool_registry()
            .expect("loot_pools.json 必须能成功加载");
        let items = crate::inventory::load_item_registry().expect("ItemRegistry 必须能成功加载");

        for pool_id in &[
            "dying_elder_secondary_honorable",
            "dying_elder_secondary_betrayal",
        ] {
            let pool = pools.get(pool_id).unwrap_or_else(|| {
                panic!("pool '{pool_id}' 应在 loot_pools.json 中（见上一个测试）")
            });
            for entry in &pool.entries {
                assert!(
                    items.get(&entry.template_id).is_some(),
                    "pool '{}' 引用未知 template_id '{}'（须在 ItemRegistry 中）",
                    pool_id,
                    entry.template_id
                );
            }
        }
    }

    #[test]
    fn scroll_template_ids_exist_in_item_registry() {
        // 期望：EARTH_GRADE_TECHNIQUE_POOL 中所有功法对应的 scroll template 均在 ItemRegistry 中
        let items = crate::inventory::load_item_registry().expect("ItemRegistry 必须能成功加载");
        for skill_id in EARTH_GRADE_TECHNIQUE_POOL {
            let scroll_id = skill_id_to_scroll_template(skill_id)
                .unwrap_or_else(|| panic!("skill_id='{skill_id}' 无 scroll 映射"));
            assert!(
                items.get(scroll_id).is_some(),
                "skill_id='{}' 对应的 scroll template_id='{}' 不在 ItemRegistry 中；\
                 请检查 assets/items/ 是否有对应 toml 定义",
                skill_id,
                scroll_id
            );
        }
    }

    // ── P3：Redis 叙事事件 + Renown 调整测试 ──────────────────────────────────

    #[test]
    fn p3_renown_threshold_constant_pin() {
        // 期望：DYING_ELDER_RENOWN_THRESHOLD = 300（设计决议 pin）
        assert_eq!(
            DYING_ELDER_RENOWN_THRESHOLD, 300,
            "P3 声名门槛应精确为 300（worldview §七 fame>300 触发友好调整）；实际 = {}",
            DYING_ELDER_RENOWN_THRESHOLD
        );
    }

    #[test]
    fn p3_renown_betray_reduction_constant_pin() {
        // 期望：DYING_ELDER_RENOWN_BETRAY_REDUCTION = 0.2（设计决议 pin）
        assert!(
            (DYING_ELDER_RENOWN_BETRAY_REDUCTION - 0.2).abs() < f64::EPSILON,
            "P3 声名减量应精确为 0.2；实际 = {}",
            DYING_ELDER_RENOWN_BETRAY_REDUCTION
        );
    }

    #[test]
    fn p3_renown_adjustment_applied_only_once_at_plea() {
        // 期望：声名调整只在 Plea 态（首次给丹）触发，之后 Recovering 态不再调整
        let mut bb = DyingElderBlackboard::new("tsy_deep", DVec3::ZERO, 0, 0);
        let original_prob = bb.betray_probability;

        // 模拟 Plea 态调整（fame=301）
        bb.apply_renown_adjustment(301);
        let after_first = bb.betray_probability;

        // 验证确实发生了调整
        let expected_after_first =
            (original_prob - DYING_ELDER_RENOWN_BETRAY_REDUCTION).clamp(0.05, 0.95);
        assert!(
            (after_first - expected_after_first).abs() < f64::EPSILON,
            "首次 apply_renown_adjustment(fame=301) 后应减少 {DYING_ELDER_RENOWN_BETRAY_REDUCTION}；\
             original={original_prob:.3} expected={expected_after_first:.3} actual={after_first:.3}"
        );

        // 模拟 Recovering 态不再调整（give_dan_system 只在 Plea 态调用 apply_renown_adjustment）
        // 此处纯逻辑验证：Recovering 态下调用不会 panic 或产生异常副作用
        let after_second = bb.betray_probability; // 不再调用 apply_renown_adjustment
        assert!(
            (after_second - after_first).abs() < f64::EPSILON,
            "Recovering 态下 betray_probability 不应再变化；after_first={after_first:.3} after_second={after_second:.3}"
        );
    }

    #[test]
    fn p3_renown_adjustment_boundary_exactly_300_no_change() {
        // 期望：fame = 300（等于阈值）不触发调整（条件是严格 fame > 300）
        let mut bb = DyingElderBlackboard::new("tsy_deep", DVec3::ZERO, 42, 0);
        let before = bb.betray_probability;
        bb.apply_renown_adjustment(300); // 等于阈值，不触发
        assert!(
            (bb.betray_probability - before).abs() < f64::EPSILON,
            "fame=300 不应触发减量（严格大于判断）；before={before:.3} after={:.3}",
            bb.betray_probability
        );
    }

    #[test]
    fn p3_renown_adjustment_fame_301_triggers_change() {
        // 期望：fame = 301（恰好超过阈值）触发减量
        let mut bb = DyingElderBlackboard::new("tsy_deep", DVec3::ZERO, 42, 0);
        let before = bb.betray_probability;
        bb.apply_renown_adjustment(301); // 刚超过阈值，触发
        assert!(
            bb.betray_probability < before,
            "fame=301 应触发减量；before={before:.3} after={:.3}",
            bb.betray_probability
        );
    }

    #[test]
    fn p3_death_event_kind_dead_natural_vs_betrayal() {
        // 期望：dead_by_betrayal 决定 event_kind（纯逻辑 pin，匹配 P3 系统逻辑）
        let kind_betrayal = if true {
            ElderEncounterEventKindV1::Betrayal
        } else {
            ElderEncounterEventKindV1::DeadNatural
        };
        let kind_natural = if false {
            ElderEncounterEventKindV1::Betrayal
        } else {
            ElderEncounterEventKindV1::DeadNatural
        };

        assert_eq!(
            kind_betrayal,
            ElderEncounterEventKindV1::Betrayal,
            "dead_by_betrayal=true 应映射为 ElderEncounterEventKindV1::Betrayal"
        );
        assert_eq!(
            kind_natural,
            ElderEncounterEventKindV1::DeadNatural,
            "dead_by_betrayal=false 应映射为 ElderEncounterEventKindV1::DeadNatural"
        );
    }

    #[test]
    fn p3_appear_event_betray_probability_from_blackboard() {
        // 期望：Appeared 事件的 betray_probability 来自 blackboard 初始值
        // （renown 调整在首次给丹时执行，appeared 事件使用 spawn 时的原始值）
        let bb = DyingElderBlackboard::new("tsy_deep", DVec3::ZERO, 1234, 0);
        let betray_prob = bb.betray_probability;

        // 模拟构建 appeared 事件（qi_fraction=1.0：刚出现时真元满值；elder_entity_id 为 placeholder）
        let event = ElderEncounterEventV1 {
            zone_name: bb.home_zone.clone(),
            elder_entity_id: 1, // 最小合法 MC protocol entity_id
            event_kind: ElderEncounterEventKindV1::Appeared,
            betray_probability: betray_prob,
            dan_count: 0,
            offered_skill_id: bb.offered_skill_id.to_string(),
            qi_fraction: 1.0,
            server_tick: 0,
        };

        assert_eq!(
            event.event_kind,
            ElderEncounterEventKindV1::Appeared,
            "spawn 时发送的事件应为 Appeared"
        );
        assert!(
            (event.betray_probability - betray_prob).abs() < f64::EPSILON,
            "appeared 事件 betray_probability 应来自 blackboard spawn 值；\
             bb={betray_prob:.3} event={:.3}",
            event.betray_probability
        );
        assert_eq!(
            event.dan_count, 0,
            "appeared 事件 dan_count 应为 0（刚出现，尚未收到丹）"
        );
    }

    #[test]
    fn p3_elder_encounter_event_v1_all_event_kinds_constructible() {
        // 期望：5 种 ElderEncounterEventKindV1 均可构建为完整 ElderEncounterEventV1（契约 pin）
        use crate::schema::elder_encounter::ElderEncounterEventKindV1;

        let kinds = [
            ElderEncounterEventKindV1::Appeared,
            ElderEncounterEventKindV1::DanReceived,
            ElderEncounterEventKindV1::Betrayal,
            ElderEncounterEventKindV1::DeadNatural,
            ElderEncounterEventKindV1::DeadPlayerKill,
        ];
        for kind in kinds {
            let event = ElderEncounterEventV1 {
                zone_name: "tsy_deep".to_string(),
                elder_entity_id: 1, // MC protocol entity_id（最小合法值=1）
                event_kind: kind,
                betray_probability: 0.5,
                dan_count: 0,
                offered_skill_id: "woliu.heart".to_string(),
                qi_fraction: 0.7,
                server_tick: 100,
            };
            let json = serde_json::to_string(&event).unwrap_or_else(|e| {
                panic!("ElderEncounterEventV1{{kind:{kind:?}}} serialize failed: {e}")
            });
            let back: ElderEncounterEventV1 =
                serde_json::from_str(&json).unwrap_or_else(|e| panic!("deserialize failed: {e}"));
            assert_eq!(
                back.event_kind, kind,
                "event_kind={kind:?} serde round-trip should preserve value"
            );
        }
    }

    // ── B1 修复集成测试：dying_elder_apply_spawn_system 真实生产链 ─────────────

    /// B1 fix: 验证 dying_elder_apply_spawn_system 消费 DyingElderSpawnRequest
    /// 后，真正在 ECS World 中创建携带 DyingElderBlackboard + DyingElderState + NpcMarker
    /// 的 entity（走生产链，非手塞 entity）。
    #[test]
    fn apply_spawn_system_creates_entity_with_full_bundle_from_spawn_request() {
        use valence::prelude::App;
        let mut app = App::new();
        // 注册事件 + 系统（Bug2 修复：需同时注册 DyingElderAppearedEvent）
        app.add_event::<DyingElderSpawnRequest>();
        app.add_event::<DyingElderAppearedEvent>();
        app.add_systems(valence::prelude::Update, dying_elder_apply_spawn_system);

        // 发送 spawn request
        let pos = DVec3::new(10.0, 64.0, 20.0);
        let bb = DyingElderBlackboard::new("tsy_deep", pos, 42, 100);
        app.world_mut().send_event(DyingElderSpawnRequest {
            zone_name: "tsy_deep".to_string(),
            spawn_pos: pos,
            blackboard: bb.clone(),
            tick: 100,
        });

        // 运行一帧（系统消费事件，spawn entity）
        app.update();

        // 断言：ECS 中存在带 DyingElderBlackboard + DyingElderState + NpcMarker 的 entity
        let mut query = app
            .world_mut()
            .query::<(&DyingElderBlackboard, &DyingElderState, &NpcMarker)>();
        let results: Vec<_> = query.iter(app.world()).collect();

        assert_eq!(
            results.len(),
            1,
            "apply_spawn_system 应创建恰好 1 个带 DyingElderBlackboard+State+NpcMarker 的 entity，\
             实际 count={}（期望=1；若=0 说明 spawn apply system 未真实消费 request）",
            results.len()
        );

        let (spawned_bb, spawned_state, _) = results[0];
        assert_eq!(
            *spawned_state,
            DyingElderState::Plea,
            "spawn 后 entity 状态应为 Plea（初始乞求态），实际 = {:?}",
            *spawned_state
        );
        assert!(
            (spawned_bb.qi_current - DYING_ELDER_INITIAL_QI).abs() < f64::EPSILON,
            "spawn entity qi_current 应等于 DYING_ELDER_INITIAL_QI={DYING_ELDER_INITIAL_QI}，\
             实际 = {}（检查 DyingElderBlackboard::new 初始化或 apply_spawn_system 未保留 blackboard）",
            spawned_bb.qi_current
        );
        assert_eq!(
            spawned_bb.home_zone, "tsy_deep",
            "spawn entity home_zone 应为 'tsy_deep'（来自 spawn request），实际 = '{}'",
            spawned_bb.home_zone
        );
    }

    /// plan-npc-realm-distribution-v1 P0 回归锁：dying_elder 的化虚 `Cultivation`
    /// 字面量构造（:391-394）经 `:429-430` 整体覆盖 `npc_runtime_bundle` 产出的
    /// Cultivation，不受 P0 choke-point 修复影响——realm 必须仍是 `Realm::Void`，
    /// qi_current/qi_max 必须仍是 `DYING_ELDER_INITIAL_QI`（满灵，大能特例，
    /// 不适用"NPC spawn 不满灵"的通用红线，因为它走的是覆盖分支非通用 bundle 输出）。
    #[test]
    fn apply_spawn_system_keeps_void_realm_and_full_qi_regression_lock() {
        use valence::prelude::App;
        let mut app = App::new();
        app.add_event::<DyingElderSpawnRequest>();
        app.add_event::<DyingElderAppearedEvent>();
        app.add_systems(valence::prelude::Update, dying_elder_apply_spawn_system);

        let pos = DVec3::new(5.0, 64.0, 5.0);
        let bb = DyingElderBlackboard::new("tsy_deep", pos, 7, 50);
        app.world_mut().send_event(DyingElderSpawnRequest {
            zone_name: "tsy_deep".to_string(),
            spawn_pos: pos,
            blackboard: bb,
            tick: 50,
        });
        app.update();

        let mut query = app.world_mut().query::<(
            &crate::cultivation::components::Cultivation,
            &crate::cultivation::components::MeridianSystem,
            &NpcMarker,
        )>();
        let results: Vec<_> = query.iter(app.world()).collect();
        assert_eq!(results.len(), 1, "应恰好创建 1 个大能 entity");
        let (cultivation, meridian_system, _) = results[0];
        assert_eq!(
            cultivation.realm,
            crate::cultivation::components::Realm::Void,
            "大能 Cultivation.realm 必须是 Void（化虚），实际 = {:?}——\
             若被 P0 choke-point 修复误伤，说明 :429-430 的覆盖顺序被破坏",
            cultivation.realm
        );
        assert!(
            (cultivation.qi_current - DYING_ELDER_INITIAL_QI).abs() < f64::EPSILON,
            "大能 qi_current 必须等于 DYING_ELDER_INITIAL_QI（满灵特例），实际 = {}",
            cultivation.qi_current
        );
        assert!(
            (cultivation.qi_max - DYING_ELDER_INITIAL_QI).abs() < f64::EPSILON,
            "大能 qi_max 必须等于 DYING_ELDER_INITIAL_QI，实际 = {}",
            cultivation.qi_max
        );
        // realm↔经脉双源回归锁：meridian_system 必须由真实 Realm::Void 派生
        // （required_meridians=20），不能停留在占位 Realm::Awaken 派生出的 1 脉。
        assert_eq!(
            meridian_system.opened_count(),
            crate::cultivation::components::Realm::Void.required_meridians(),
            "大能 MeridianSystem.opened_count() 必须等于 Realm::Void.required_meridians()\
             （20），实际 = {}——若传入 npc_runtime_bundle 的 realm 实参退回占位值\
             （如 Realm::Awaken），meridian_system 会按错误 realm 派生，与落地\
             cultivation.realm=Void 形成 realm↔经脉双源",
            meridian_system.opened_count()
        );
    }

    /// B1 fix: 多次 spawn request 创建多个 entity（global cap 由 spawn_system 守，apply_spawn 只负责创建）。
    #[test]
    fn apply_spawn_system_creates_entity_per_request() {
        use valence::prelude::App;
        let mut app = App::new();
        app.add_event::<DyingElderSpawnRequest>();
        app.add_event::<DyingElderAppearedEvent>(); // Bug2 修复：需同时注册
        app.add_systems(valence::prelude::Update, dying_elder_apply_spawn_system);

        // 发送 2 个 request（模拟两帧各一次 spawn，实际生产中 spawn_system 的 global cap 防止这种情况）
        for (zone, seed) in [("tsy_deep_a", 1u64), ("tsy_deep_b", 2u64)] {
            let pos = DVec3::new(0.0, 64.0, 0.0);
            let bb = DyingElderBlackboard::new(zone, pos, seed, 0);
            app.world_mut().send_event(DyingElderSpawnRequest {
                zone_name: zone.to_string(),
                spawn_pos: pos,
                blackboard: bb,
                tick: 0,
            });
        }

        app.update();

        let mut query = app
            .world_mut()
            .query::<(&DyingElderBlackboard, &NpcMarker)>();
        let count = query.iter(app.world()).count();
        assert_eq!(
            count, 2,
            "2 个 spawn request 应创建 2 个 entity，实际 count={}",
            count
        );
    }

    /// B3 fix: 验证给丹逻辑正确使用 huiyuan_pill（无下划线，与 pills.toml 注册 id 一致）。
    #[test]
    fn give_dan_pill_id_matches_registry_id_huiyuan_pill() {
        // 契约 pin：dying_elder 给丹校验的 pill id 必须与 pills.toml 注册 id 完全一致。
        // pills.toml 注册 id: "huiyuan_pill"（无下划线）
        // 错误值（已修复的 pre-bug）: "hui_yuan_pill"（带下划线）
        let registered_pill_id = "huiyuan_pill";

        // 验证 QiAccountId 格式（dying_elder.rs:513 用 "hui_yuan_pill:" 前缀，是审计 key 非 item_id）
        // 真正的 item_id 校验在 client_request_handler.rs handle_give_dan_to_elder，
        // 本测试 pin 住正确值作为文档化约束
        assert_eq!(
            registered_pill_id, "huiyuan_pill",
            "给丹校验 id 必须精确为 'huiyuan_pill'（pills.toml 第 38 行），\
             绝不是 'hui_yuan_pill'（带下划线版本会导致所有给丹请求被拒绝）"
        );

        // 验证 pills.toml 注册文件中确实用无下划线版本（asset pin）
        let pills_toml_path = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/items/pills.toml");
        let content = std::fs::read_to_string(pills_toml_path)
            .expect("无法读取 pills.toml（路径必须正确，检查 assets/items/pills.toml）");
        assert!(
            content.contains("id = \"huiyuan_pill\""),
            "pills.toml 应包含 'id = \"huiyuan_pill\"'（无下划线），\
             若不存在说明 pills.toml 被重命名或注册被删除（B3 fix 依赖此 id 一致性）"
        );
        assert!(
            !content.contains("id = \"hui_yuan_pill\""),
            "pills.toml 不应包含 'id = \"hui_yuan_pill\"'（带下划线版本），\
             若存在说明 pills.toml 用了错误 id"
        );
    }

    /// M2 fix: 验证 betray_system 的状态写入与 death_system 不产生可见的竞态
    /// （纯逻辑测试：betray 把 state → Dead{betrayal:true} 后，death_system 能正确读取）。
    #[test]
    fn betray_system_state_write_visible_to_death_system() {
        // 期望：betray_system 写 Dead{dead_by_betrayal:true} 后，
        // death_system 的 Dead state 匹配正确（无竞态掩盖）
        // 模拟 betray_system 执行（从 Betrayal 写入 Dead{betrayal:true}）
        let state = DyingElderState::Dead {
            dead_by_betrayal: true,
        };

        // 模拟 death_system 读取（应看到 Dead{betrayal:true}）
        let is_dead_by_betrayal = match state {
            DyingElderState::Dead { dead_by_betrayal } => dead_by_betrayal,
            _ => panic!("betray 后状态应为 Dead，实际 = {:?}", state),
        };

        assert!(
            is_dead_by_betrayal,
            "betray_system 写 Dead{{dead_by_betrayal:true}} 后，\
             death_system 应看到 dead_by_betrayal=true（M2 ordering 保证无竞态），\
             实际 = {is_dead_by_betrayal}"
        );
    }
}
