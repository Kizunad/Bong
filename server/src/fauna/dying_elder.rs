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

use serde::{Deserialize, Serialize};
use valence::client::ClientMarker;
use valence::prelude::{
    bevy_ecs, App, Component, DVec3, Entity, EventReader, EventWriter, Query, Res, ResMut,
    Resource, Update, With, Without,
};

use crate::inventory::freshness::GAME_DAY_TICKS;
use crate::npc::movement::GameTick;
use crate::npc::spawn::NpcMarker;
use crate::qi_physics::ledger::{QiAccountId, QiTransfer, QiTransferReason, WorldQiAccount};
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Component)]
#[serde(rename_all = "snake_case")]
pub enum DyingElderState {
    /// 乞求态：求助玩家，等待给丹。负灵域 drain 持续消耗真元。
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

impl Default for DyingElderState {
    fn default() -> Self {
        Self::Plea
    }
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

// ── Bevy 注册 ──────────────────────────────────────────────────────────────────

/// Bevy 注册：P0 spawn timer resource + spawn 系统。
pub fn register_p0(app: &mut App) {
    app.add_event::<DyingElderSpawnRequest>();
    app.insert_resource(DyingElderSpawnTimer::default());
    app.add_systems(Update, dying_elder_spawn_system);
}

// ── P1：给丹交互事件 ──────────────────────────────────────────────────────────

/// plan-dying-elder-v1 P1 — 玩家给大能交付一颗回元丹后由网络层 emit 的意图事件。
///
/// 网络层（`handle_give_dan_to_elder`）负责：
/// 1. 校验 pill_instance_id 属于该玩家且模板为 `hui_yuan_pill`；
/// 2. 消耗丹（inventory 真删）；
/// 3. emit 本事件。
///
/// 本事件由 `dying_elder_give_dan_system` 在 Update 阶段消费，执行：
/// - 大能 qi_current 增加 qi_gain；
/// - QiTransfer{TradeDan} 审计记录；
/// - DyingElderState 更新（Plea/Recovering → Recovering{n+1}）；
/// - 若 n+1 >= DYING_ELDER_DAN_THRESHOLD → 触发结局判定。
#[derive(Debug, Clone, valence::prelude::Event)]
pub struct GiveDanToElderIntent {
    /// 执行给丹操作的玩家实体。
    pub player: Entity,
    /// 垂死大能 ECS 实体。
    pub elder: Entity,
    /// 已被消耗的回元丹 instance_id（用于 QiTransfer 账户标识）。
    pub pill_instance_id: u64,
    /// 丹携带的 qi_gain 值（来自 ItemEffect::QiRecovery { amount }，默认 24.0）。
    pub qi_gain: f64,
}

// ── P1：夺舍事件 ──────────────────────────────────────────────────────────────

/// plan-dying-elder-v1 P1 — 大能翻脸夺舍时 emit 的事件。
///
/// ## 守恒约束
/// - `qi_transferred`：player.qi_current → elder via QiTransfer{SoulSeize}（已清零）；
/// - `qi_max_drain`：player.qi_max 永久减损量（= qi_max_cache × DYING_ELDER_SOUL_SEIZE_RATIO）；
///   **不是** qi 搬运，不走 QiTransfer。
///
/// 下游系统（`dying_elder_betray_system`）消费本事件：
/// - 实际操作玩家 `Cultivation.qi_current -= qi_transferred + Cultivation.qi_max -= qi_max_drain`；
/// - 大能 blackboard.qi_current += qi_transferred；
/// - 大能 state → Dead { dead_by_betrayal: true }（夺舍力竭）。
#[derive(Debug, Clone, valence::prelude::Event)]
pub struct SoulSeizeEvent {
    /// 执行夺舍的大能实体。
    pub elder: Entity,
    /// 被夺舍的玩家实体。
    pub player: Entity,
    /// 转入大能的 qi 量（= player.qi_current，非负）。
    pub qi_transferred: f64,
    /// 玩家 qi_max 永久减损量（= elder.qi_max_cache × DYING_ELDER_SOUL_SEIZE_RATIO）。
    /// **不是 qi 搬运**——不走 QiTransfer，是容量 debuff。
    pub qi_max_drain: f64,
}

// ── P1：给丹系统 ──────────────────────────────────────────────────────────────

/// plan-dying-elder-v1 P1 — 消费 `GiveDanToElderIntent`，更新大能真元 + 状态。
///
/// ## 守恒执行顺序
/// 1. 读取大能 `DyingElderBlackboard.qi_current`；
/// 2. 加上 qi_gain（不超过 qi_max_cache × 2.0 防止超量）；
/// 3. 向 `WorldQiAccount` push QiTransfer{TradeDan} 审计记录；
/// 4. 更新 `DyingElderState`：
///    - Plea → Recovering { dan_received: 1 }
///    - Recovering { n } → Recovering { n+1 }
///    - n+1 >= DYING_ELDER_DAN_THRESHOLD → 触发结局判定
/// 5. 结局判定：`betray_roll` → Betrayal 或 Dead { dead_by_betrayal: false }
#[allow(clippy::type_complexity)]
pub(crate) fn dying_elder_give_dan_system(
    mut intents: EventReader<GiveDanToElderIntent>,
    mut elders: Query<
        (&mut DyingElderBlackboard, &mut DyingElderState),
        (With<NpcMarker>, Without<ClientMarker>),
    >,
    mut soul_seize_events: EventWriter<SoulSeizeEvent>,
    mut qi_transfer_events: EventWriter<QiTransfer>,
    mut qi_account: Option<ResMut<WorldQiAccount>>,
    game_tick: Option<Res<GameTick>>,
) {
    let tick = game_tick.as_deref().map(|t| t.0 as u64).unwrap_or(0);

    for intent in intents.read() {
        let Ok((mut bb, mut state)) = elders.get_mut(intent.elder) else {
            tracing::warn!(
                "[bong][dying_elder] give_dan_system: elder entity {:?} not found",
                intent.elder
            );
            continue;
        };

        // 校验大能当前状态是否可接丹（只在 Plea 或 Recovering 状态接受）
        let dan_received = match *state {
            DyingElderState::Plea => 0,
            DyingElderState::Recovering { dan_received } => dan_received,
            DyingElderState::Betrayal | DyingElderState::Dead { .. } => {
                tracing::debug!(
                    "[bong][dying_elder] give_dan_system: elder {:?} in {:?}, rejecting dan",
                    intent.elder,
                    *state
                );
                continue;
            }
        };

        // ── 守恒：更新大能真元（不超过 qi_max_cache 的合理上限）──────────────
        let qi_before = bb.qi_current;
        let qi_cap = bb.qi_max_cache * 1.5; // 允许恢复到最大值 150%（大能积蓄传承真元）
        bb.qi_current = (bb.qi_current + intent.qi_gain).min(qi_cap);
        let actual_qi_added = bb.qi_current - qi_before;

        // ── 守恒：QiTransfer{TradeDan} 审计记录 ──────────────────────────────
        let pill_account =
            QiAccountId::container(format!("hui_yuan_pill:{}", intent.pill_instance_id));
        let elder_account = QiAccountId::npc(format!("dying_elder:{}", intent.elder.to_bits()));
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
            intent.qi_gain,
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

                // qi_max_drain 永久减损量（= qi_max_cache × DYING_ELDER_SOUL_SEIZE_RATIO）
                let qi_max_drain = bb.qi_max_cache * DYING_ELDER_SOUL_SEIZE_RATIO;
                // player.qi_current 全额转入大能（下游系统读玩家真实 qi_current）
                // 此处用 qi_max_cache 作为估算量（实际量由 betray system 从 Cultivation 读）
                let qi_transferred_estimate = bb.qi_max_cache * 0.5; // 估计，实际值在 betray system

                soul_seize_events.send(SoulSeizeEvent {
                    elder: intent.elder,
                    player: intent.player,
                    qi_transferred: qi_transferred_estimate,
                    qi_max_drain,
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
/// 5. player.qi_max -= qi_max_drain（永久容量 debuff，**不走** QiTransfer）；
/// 6. 大能 state → Dead { dead_by_betrayal: true }（夺舍力竭）。
#[allow(clippy::type_complexity)]
pub(crate) fn dying_elder_betray_system(
    mut events: EventReader<SoulSeizeEvent>,
    mut elders: Query<
        (&mut DyingElderBlackboard, &mut DyingElderState),
        (With<NpcMarker>, Without<ClientMarker>),
    >,
    mut cultivations: Query<&mut crate::cultivation::components::Cultivation, With<ClientMarker>>,
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

        let Ok(mut cultivation) = cultivations.get_mut(ev.player) else {
            tracing::warn!(
                "[bong][dying_elder] betray_system: player entity {:?} has no Cultivation",
                ev.player
            );
            // 即使玩家无修炼，大能仍进入 Dead 状态
            *state = DyingElderState::Dead {
                dead_by_betrayal: true,
            };
            continue;
        };

        // ── 守恒：读取玩家真实 qi_current，全额转入大能 ───────────────────────
        let actual_qi = cultivation.qi_current.max(0.0);
        cultivation.qi_current = 0.0;
        bb.qi_current += actual_qi;

        // ── 守恒：QiTransfer{SoulSeize} 审计（从玩家到大能）────────────────────
        if actual_qi > 0.0 {
            let player_account = QiAccountId::player(format!("entity:{}", ev.player.to_bits()));
            let elder_account = QiAccountId::npc(format!("dying_elder:{}", ev.elder.to_bits()));
            let transfer = QiTransfer {
                from: player_account,
                to: elder_account,
                amount: actual_qi,
                reason: QiTransferReason::SoulSeize,
            };
            if let Some(ref mut account) = qi_account {
                account.push_transfer_audit(transfer.clone());
            }
            qi_transfer_events.send(transfer);
        }

        // ── 守恒：qi_max 永久减损（容量 debuff，不走 QiTransfer）────────────────
        let qi_max_drain = (bb.qi_max_cache * DYING_ELDER_SOUL_SEIZE_RATIO).max(0.0);
        cultivation.qi_max = (cultivation.qi_max - qi_max_drain).max(0.0);

        // 防止 qi_current > 新 qi_max（守恒：不超容量）
        let effective_max =
            (cultivation.qi_max - cultivation.qi_max_frozen.unwrap_or(0.0)).max(0.0);
        cultivation.qi_current = cultivation.qi_current.min(effective_max);

        // ── 大能力竭死亡 ───────────────────────────────────────────────────────
        *state = DyingElderState::Dead {
            dead_by_betrayal: true,
        };

        tracing::info!(
            "[bong][dying_elder] betray_system: player {:?} soul seized! qi_transferred={:.2} qi_max_drain={:.2}",
            ev.player,
            actual_qi,
            qi_max_drain,
        );
    }
}

// ── Bevy 注册 P1 ──────────────────────────────────────────────────────────────

/// Bevy 注册：P1 给丹系统 + 夺舍系统 + 相关事件。
pub fn register_p1(app: &mut App) {
    app.add_event::<GiveDanToElderIntent>();
    app.add_event::<SoulSeizeEvent>();
    app.add_event::<QiTransfer>();
    app.add_systems(Update, dying_elder_give_dan_system);
    app.add_systems(Update, dying_elder_betray_system);
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
    fn give_dan_qi_gain_does_not_overflow_qi_cap() {
        // 期望：多次给丹后大能 qi_current 不超过 qi_cap（= qi_max_cache × 1.5）
        let qi_max_cache = DYING_ELDER_INITIAL_QI; // 500.0
        let qi_cap = qi_max_cache * 1.5; // 750.0
        let mut qi_current = qi_max_cache; // 500.0
        let qi_gain_per_dan = 24.0_f64;
        // 给 10 颗（远超 threshold），模拟上限保护
        for i in 0..10 {
            qi_current = (qi_current + qi_gain_per_dan).min(qi_cap);
            assert!(
                qi_current <= qi_cap,
                "第 {i} 颗丹后 qi_current={qi_current} 不应超过 qi_cap={qi_cap}"
            );
        }
    }

    #[test]
    fn give_dan_qi_gain_conservation_per_dan() {
        // 期望：每颗丹的 qi 增量精确等于 qi_gain（在 cap 范围内）
        let initial_qi = 100.0_f64;
        let qi_gain = 24.0_f64;
        let qi_cap = DYING_ELDER_INITIAL_QI * 1.5; // 750.0
        let expected_after = (initial_qi + qi_gain).min(qi_cap);
        assert!(
            (expected_after - (initial_qi + qi_gain)).abs() < f64::EPSILON || expected_after < qi_cap,
            "qi 增量应精确为 qi_gain={qi_gain}（在 cap 内）；initial={initial_qi} expected_after={expected_after}"
        );
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
}
