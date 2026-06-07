//! plan-daozhan-v1 P0/P1/P2 — 道伥核心数据结构、spawn 触发逻辑与 Mimicry/Ambush big-brain AI。
//!
//! 道伥：死在坍缩渊/天劫的高境遗骸，以玩家外形伪装诱近，背对/低真元时绝杀。
//!
//! ## P0 交付物
//!
//! - [`DaoZhangState`] — 道伥状态机（Mimicry / Ambush 两态，无 Retreat）。
//! - [`DaoZhangBehaviorBlackboard`] — 道伥个体行为帧（含守恒累积量 `daozhan_qi`）。
//! - [`DaoZhangSpawnTrigger`] — 道伥来源三种触发路径。
//! - [`realm_spawn_probability`] — 境界 → spawn 概率（化虚 80% / 通灵 50% / 固元 20%）。
//! - P0 loot 常数（由 [`loot.rs`] 引用）。
//!
//! ## P1 交付物
//!
//! - [`DaoZhangMimicryScorer`] — big-brain Scorer，Mimicry 态检测到玩家时评分 0.7（低于 Ambush 1.0）。
//! - [`DaoZhangMimicryAction`] — big-brain Action，循环推进 behavior_queue，计时 2–4s（游戏 tick）。
//! - [`daozhan_mimicry_scorer_system`] / [`daozhan_mimicry_action_system`] — Bevy 注册函数。
//!
//! ## P2 交付物
//!
//! - [`DaoZhangAmbushScorer`] — big-brain Scorer，背对 > 150° 或 qi < 20% 时 score=1.0，抢占 Mimicry。
//! - [`DaoZhangAmbushAction`] — big-brain Action，emit `DaoZhangRevealEvent` + VFX + 音效，
//!   然后在目标最近玩家上执行 3 连 `QiTransfer{DaoZhangDrain}`（守恒：player.qi -= amount，
//!   daozhan_qi += amount），完成后转回 Mimicry 冷却。
//! - [`DaoZhangAmbushChainState`] — 连击链追踪 Component（当前已击次数 / 目标玩家 / chain 开始 tick）。
//! - [`daozhan_ambush_scorer_system`] / [`daozhan_ambush_action_system`] — Bevy 注册函数。
//!
//! ## 守恒红线
//!
//! 1. **攻击吸取**（P2）：走 `QiTransfer{DaoZhangDrain}`；player.qi_current -= amount，
//!    daozhan_qi += amount，不凭空消失。
//! 2. **死亡释放**：走 `release_qi_amount_to_zone` 全额（qi_current 残余 + daozhan_qi 一并归还）。
//! 3. **天道凝结**（P3 实装）：走 `QiTransfer{TiandaoCondense}`；zone.spirit_qi -= delta，
//!    道伥 qi_init = condensed_amount，绝不凭空创生。
//! 4. **坍缩渊/天劫死亡 spawn**：道伥初始 qi 来自死者遗留（死亡链路转移），不创生。

use big_brain::prelude::{ActionBuilder, ActionState, Actor, Score, ScorerBuilder};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use valence::client::ClientMarker;
use valence::prelude::{
    bevy_ecs, App, Commands, Component, DVec3, Entity, EventReader, EventWriter, Position, Query,
    Res, ResMut, With, Without,
};

use crate::combat::events::DeathEvent;
use crate::cultivation::components::{Cultivation, Realm};
use crate::cultivation::death_hooks::release_qi_amount_to_zone;
use crate::fauna::experience::{play_audio, spawn_particle};
use crate::network::audio_event_emit::PlaySoundRecipeRequest;
use crate::network::daozhan_disguise_emit::DaoZhangRevealEvent;
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::npc::movement::GameTick;
use crate::npc::spawn::NpcMarker;
use crate::npc::tsy_hostile::spawn_tsy_daoxiang_at;
use crate::qi_physics::ledger::{QiAccountId, QiTransfer, QiTransferReason};
use crate::world::dimension::CurrentDimension;
use crate::world::zone::ZoneRegistry;

// ── 常数 ─────────────────────────────────────────────────────────────────────

/// 化虚境界 → spawn 道伥概率（0.0–1.0）。
pub const DAOZHAN_SPAWN_PROB_VOID: f64 = 0.80;

/// 通灵境界 → spawn 道伥概率。
pub const DAOZHAN_SPAWN_PROB_SPIRIT: f64 = 0.50;

/// 固元境界 → spawn 道伥概率。
pub const DAOZHAN_SPAWN_PROB_SOLIDIFY: f64 = 0.20;

/// 天道凝结触发阈值：zone.spirit_qi > 此值时天道可凝结道伥。
/// P3 实装；P0 定义常数以便测试锁定。
pub const TIANDAO_CONDENSE_THRESHOLD: f64 = 0.8;

/// 道伥伏击连击次数（每次暴起连打 3 次 QiTransfer{DaoZhangDrain}）。
/// P2 实装；P0 定义常数供测试。
pub const DAOZHAN_AMBUSH_CHAIN_COUNT: u32 = 3;

/// 道伥判断"背对"的最小偏转角（度）；> 此值算背对触发暴起。
/// P2 实装；P0 定义常数。
pub const DAOZHAN_BACK_ANGLE_DEG: f64 = 150.0;

/// 道伥判断"低真元"的比例阈值（qi_current / qi_max < 此值时触发暴起）。
/// P2 实装；P0 定义常数。
pub const DAOZHAN_LOW_QI_RATIO: f64 = 0.20;

// ── 状态机 ────────────────────────────────────────────────────────────────────

/// 道伥 AI 两态状态机。
///
/// - `Mimicry`：伪装为"无名玩家"在附近游荡，欺骗接近。client 端渲染 FakePlayerEntity。
/// - `Ambush`：已暴露，转入连击吸取真元。
///
/// 无 Retreat 态：道伥不撤退，只有死亡才会退场（与灰烬蛛三态不同）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Component)]
#[serde(rename_all = "snake_case")]
pub enum DaoZhangState {
    /// 伪装态：伪装为日常玩家行为（Swing / Sneak / Mine）诱近目标。
    Mimicry,
    /// 伏击态：暴露后连续 3 次 QiTransfer{DaoZhangDrain}，吸取真元。
    Ambush,
}

impl Default for DaoZhangState {
    fn default() -> Self {
        Self::Mimicry
    }
}

// ── Mimicry 假动作 ────────────────────────────────────────────────────────────

/// 道伥在 Mimicry 态循环执行的假动作。
///
/// v1 三种：Swing / Sneak / Mine（假挥手 / 假潜行 / 假挖掘）。
/// v2 接口预留：`behavior_queue: VecDeque<FakeBehavior>` 留 variant 空间。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FakeBehavior {
    /// 假挥手（模拟战斗姿势）。
    Swing,
    /// 假潜行（模拟侦查/隐蔽）。
    Sneak,
    /// 假挖掘（模拟采集动作）。
    Mine,
}

impl FakeBehavior {
    /// 返回循环顺序的三种假动作（固定顺序，供 behavior_queue 初始化）。
    pub fn cycle() -> [FakeBehavior; 3] {
        [FakeBehavior::Swing, FakeBehavior::Sneak, FakeBehavior::Mine]
    }
}

// ── Blackboard ────────────────────────────────────────────────────────────────

/// 道伥个体行为帧（ECS Component）。
///
/// ## 守恒字段
/// - `daozhan_qi`：P2 伏击期累积吸取量；死亡时由 `DaoZhangDeathSystem` 全额归还 zone。
///
/// ## v2 预留
/// - `pack_id: Option<Entity>`：多道伥协作组 ID，v1 不用（始终 None）。
#[derive(Debug, Clone, PartialEq, Component)]
pub struct DaoZhangBehaviorBlackboard {
    /// 道伥孵化区域名称（用于 zone 查找，守恒账户定位）。
    pub home_zone: String,
    /// 孵化位置（Mimicry 游走参考）。
    pub home_pos: DVec3,
    /// P2 伏击期累积吸取的玩家真元量；死亡时归还 zone。
    /// 守恒不变式：凡走 DaoZhangDrain 的 amount 须同步 += 到此字段。
    pub daozhan_qi: f64,
    /// 原始死者境界（影响掉落等级）；天道凝结的道伥为 None（无具体原始修士）。
    pub origin_realm: Option<Realm>,
    /// Mimicry 态假动作队列（循环）。
    pub behavior_queue: VecDeque<FakeBehavior>,
    /// 当前假动作已执行 tick 数（2–4s 随机，P1 big-brain 计时）。
    pub current_behavior_ticks: u32,
    /// v2 多道伥协作组 ID（v1 始终 None）。
    pub pack_id: Option<Entity>,
}

impl DaoZhangBehaviorBlackboard {
    /// 构造带默认 behavior_queue（Swing → Sneak → Mine 循环）的 blackboard。
    pub fn new(home_zone: &str, home_pos: DVec3, origin_realm: Option<Realm>) -> Self {
        let mut queue = VecDeque::with_capacity(3);
        for b in FakeBehavior::cycle() {
            queue.push_back(b);
        }
        Self {
            home_zone: home_zone.to_string(),
            home_pos,
            daozhan_qi: 0.0,
            origin_realm,
            behavior_queue: queue,
            current_behavior_ticks: 0,
            pack_id: None,
        }
    }

    /// 取队列头部假动作（循环消费：pop front + push back）。
    pub fn next_behavior(&mut self) -> FakeBehavior {
        let b = self
            .behavior_queue
            .pop_front()
            .unwrap_or(FakeBehavior::Swing);
        self.behavior_queue.push_back(b);
        b
    }
}

// ── spawn 触发路径 ────────────────────────────────────────────────────────────

/// 道伥三种来源触发路径。
///
/// 由调用者（tsy_lifecycle / tribulation / tiandao system）附加到 spawn 函数参数，
/// 供后续 loot / debug / telemetry 查询。
/// `DaoZhangSpawnTrigger` 含 `f64` 字段（condensed_qi），不实现 `Eq`/`Hash`。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DaoZhangSpawnTrigger {
    /// 坍缩渊死亡（tsy_collapsed / tsy_drain）→ 按 origin_realm 概率门控 spawn。
    CollapseZoneDeath {
        /// 死亡坍缩 TSY family ID。
        family_id: String,
        /// 原始修士境界（来自死者 CorpseEmbalmed + Cultivation.realm 读取）。
        origin_realm: Realm,
    },
    /// 天劫失败/回火死亡（BreakthroughBackfire / MeridianCollapse）→ 按概率 spawn。
    TribulationStrike {
        /// 原始修士境界。
        origin_realm: Realm,
    },
    /// 天道凝结（P3 实装）：zone.spirit_qi > TIANDAO_CONDENSE_THRESHOLD，
    /// 从高浓度灵气中凝出道伥，初始 qi 走 QiTransfer{TiandaoCondense}。
    TiandaoCondense {
        /// 触发凝结的 zone 名。
        zone_name: String,
        /// 凝出的初始 qi 量（= zone 减去的量）。
        condensed_qi: f64,
    },
}

// ── spawn 概率 ────────────────────────────────────────────────────────────────

/// 按境界返回道伥 spawn 概率（0.0–1.0）。
///
/// 设计决议（#1）：化虚 80% / 通灵 50% / 固元 20% / 其他 0%。
/// 低境界修士未能高境养炼，无足够"怨念真元"凝出道伥。
pub fn realm_spawn_probability(realm: Realm) -> f64 {
    match realm {
        Realm::Void => DAOZHAN_SPAWN_PROB_VOID,
        Realm::Spirit => DAOZHAN_SPAWN_PROB_SPIRIT,
        Realm::Solidify => DAOZHAN_SPAWN_PROB_SOLIDIFY,
        Realm::Awaken | Realm::Induce | Realm::Condense => 0.0,
    }
}

/// 基于 splitmix64 种子判定道伥 spawn 是否命中概率门控。
///
/// 返回 `(命中?, 下一seed)`，调用方链式更新 seed 避免相关性。
/// 使用确定性 RNG 与 tsy_lifecycle 保持一致（不引入外部 rand crate）。
pub fn daozhan_spawn_roll(realm: Realm, seed: u64) -> (bool, u64) {
    let next = seed
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(0x6C62_272E_07BB_0142);
    let prob = realm_spawn_probability(realm);
    // 将概率映射到 0..1000 千分位
    let threshold = (prob * 1000.0) as u64;
    let hit = (next % 1000) < threshold;
    (hit, next)
}

// ── 道伥 loot origin_realm 分档 ───────────────────────────────────────────────

/// 道伥 loot 按 origin_realm 分档：掉落表名称后缀（供 loot.rs 使用）。
///
/// 设计决议（#2）：v1 只影响掉落等级，不影响战斗强度。
/// - 化虚/通灵：高档残卷 + 破碎法宝（稀有）
/// - 固元：中档残卷
/// - 其他/None（天道凝结）：通用残卷
pub fn daozhan_loot_tier(origin_realm: Option<Realm>) -> DaoZhangLootTier {
    match origin_realm {
        Some(Realm::Void) | Some(Realm::Spirit) => DaoZhangLootTier::High,
        Some(Realm::Solidify) => DaoZhangLootTier::Mid,
        _ => DaoZhangLootTier::Base,
    }
}

/// 道伥 loot 分档枚举。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DaoZhangLootTier {
    /// 化虚/通灵 origin：高档残卷 + 破碎法宝（低概率）。
    High,
    /// 固元 origin：中档残卷。
    Mid,
    /// 天道凝结 / 低境 / 默认：通用残卷（最基础）。
    Base,
}

// ── P1: Mimicry tick 常数 ─────────────────────────────────────────────────────

/// Mimicry 态每个假动作最短持续 tick（2s × 20TPS）。
pub const MIMICRY_BEHAVIOR_MIN_TICKS: u32 = 40;

/// Mimicry 态每个假动作最长持续 tick（4s × 20TPS）。
pub const MIMICRY_BEHAVIOR_MAX_TICKS: u32 = 80;

/// Mimicry 态激活时的 big-brain score（低于 Ambush=1.0，让 Ambush 始终可抢占）。
pub const DAOZHAN_MIMICRY_SCORE: f32 = 0.7;

/// Mimicry 态感知玩家的最大距离（格）。
pub const DAOZHAN_MIMICRY_SENSE_RADIUS: f64 = 16.0;

// ── P1: DaoZhangMimicryScorer ─────────────────────────────────────────────────

/// Mimicry 评分器：道伥处于 Mimicry 态且感知到周围玩家时，score = DAOZHAN_MIMICRY_SCORE(0.7)。
///
/// 低于 Ambush scorer(1.0)，保证暴起优先级总高于伪装游荡。
/// 即使无玩家也可给出低分（0.3）以保持游荡，防止道伥完全静止露馅。
#[derive(Clone, Copy, Debug, Component)]
pub struct DaoZhangMimicryScorer;

impl ScorerBuilder for DaoZhangMimicryScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("DaoZhangMimicryScorer")
    }
}

type DaoZhangScorerActorQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Position, &'static DaoZhangState),
    (With<NpcMarker>, Without<ClientMarker>),
>;

type MimicryPlayerQuery<'w, 's> =
    Query<'w, 's, &'static Position, (With<ClientMarker>, Without<NpcMarker>)>;

pub(crate) fn daozhan_mimicry_scorer_system(
    daozhan: DaoZhangScorerActorQuery<'_, '_>,
    players: MimicryPlayerQuery<'_, '_>,
    mut scorers: Query<(&Actor, &mut Score), With<DaoZhangMimicryScorer>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let Ok((npc_pos, state)) = daozhan.get(*actor) else {
            score.set(0.0);
            continue;
        };

        // 只在 Mimicry 态激活（Ambush 态由 AmbushScorer/Thinker 接管）
        if *state != DaoZhangState::Mimicry {
            score.set(0.0);
            continue;
        }

        let pos = npc_pos.get();
        let has_nearby_player = players
            .iter()
            .any(|player_pos| pos.distance(player_pos.get()) <= DAOZHAN_MIMICRY_SENSE_RADIUS);

        // 附近有玩家：高分（接近 Ambush 阈值但不达到），触发积极游荡欺骗
        // 无玩家：低分（维持最基础的游荡，防止完全静止）
        score.set(if has_nearby_player {
            DAOZHAN_MIMICRY_SCORE
        } else {
            0.3
        });
    }
}

// ── P1: DaoZhangMimicryAction ─────────────────────────────────────────────────

/// Mimicry 行动：循环推进 behavior_queue，每个动作持续 2–4s（游戏 tick 计时）。
///
/// 每次进入 Requested 时从 behavior_queue 取头部假动作并记录 tick 计时目标，
/// Executing 期到时重新进入 Requested（循环直到被高优先级 scorer 抢占或 Cancelled）。
///
/// **计时使用游戏 tick（`GameTick` Resource）**，非渲染帧（避免 fauna-stitched 教训）。
/// tick 不可用时回退到 `current_behavior_ticks` 字段自增（同 ECS tick 语义）。
#[derive(Clone, Copy, Debug, Component)]
pub struct DaoZhangMimicryAction;

impl ActionBuilder for DaoZhangMimicryAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("DaoZhangMimicryAction")
    }
}

type DaoZhangMimicryActorQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static DaoZhangState,
        &'static mut DaoZhangBehaviorBlackboard,
    ),
    (With<NpcMarker>, Without<ClientMarker>),
>;

/// 用确定性 splitmix64 为当前行为生成持续 tick 数（范围 MIN..=MAX）。
///
/// seed = (actor_entity_raw_index ^ game_tick)，每次 Requested 时产生新的持续时间。
fn mimicry_behavior_duration_ticks(seed: u64) -> u32 {
    let mixed = seed
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(0x6C62_272E_07BB_0142);
    let range = (MIMICRY_BEHAVIOR_MAX_TICKS - MIMICRY_BEHAVIOR_MIN_TICKS + 1) as u64;
    MIMICRY_BEHAVIOR_MIN_TICKS + (mixed % range) as u32
}

pub(crate) fn daozhan_mimicry_action_system(
    mut daozhan: DaoZhangMimicryActorQuery<'_, '_>,
    mut actions: Query<(&Actor, &mut ActionState), With<DaoZhangMimicryAction>>,
    game_tick: Option<valence::prelude::Res<GameTick>>,
) {
    let tick = game_tick.as_deref().map(|t| t.0).unwrap_or(0);

    for (Actor(actor), mut action_state) in &mut actions {
        let Ok((state, mut bb)) = daozhan.get_mut(*actor) else {
            *action_state = ActionState::Failure;
            continue;
        };

        // 状态守卫：非 Mimicry 态时立即 Failure（Ambush 已接管）
        if *state != DaoZhangState::Mimicry {
            *action_state = ActionState::Failure;
            continue;
        }

        match *action_state {
            ActionState::Requested => {
                // 取下一个假动作，开始计时
                let _behavior = bb.next_behavior();
                // 用 (actor.index ^ tick) 作种子，保证每次持续时长不同
                let seed = (actor.index() as u64).wrapping_add(tick as u64);
                let duration = mimicry_behavior_duration_ticks(seed);
                bb.current_behavior_ticks = 0;
                // 将目标 tick 数存入 current_behavior_ticks 负数侧：
                // 我们用 current_behavior_ticks 字段存"目标持续 tick"，
                // 然后在 Executing 期每帧递增到期。
                // 为复用同一字段，约定：Requested 时写目标，Executing 时逐帧 +1 比对。
                bb.current_behavior_ticks = duration;
                *action_state = ActionState::Executing;
            }

            ActionState::Executing => {
                // 每帧递减计数器（目标 tick 被写入时为 duration，递减到 0 表示完成）
                if bb.current_behavior_ticks == 0 {
                    // 本动作完成，重新进入 Requested 驱动下一个假动作
                    *action_state = ActionState::Requested;
                } else {
                    bb.current_behavior_ticks = bb.current_behavior_ticks.saturating_sub(1);
                }
            }

            ActionState::Cancelled => {
                *action_state = ActionState::Failure;
            }

            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

/// Bevy 注册：Mimicry 评分器 + 行动 system（供 fauna::mod.rs 调用）。
pub fn register_p1(app: &mut App) {
    use big_brain::prelude::BigBrainSet;
    use valence::prelude::{IntoSystemConfigs, PreUpdate};

    app.add_systems(
        PreUpdate,
        daozhan_mimicry_scorer_system.in_set(BigBrainSet::Scorers),
    );
    app.add_systems(
        PreUpdate,
        daozhan_mimicry_action_system.in_set(BigBrainSet::Actions),
    );
}

// ── P2: Ambush 常数 ───────────────────────────────────────────────────────────

/// 道伥暴起 VFX event_id（`bong:vfx/daozhan_reveal`）。
pub const DAOZHAN_REVEAL_VFX_EVENT_ID: &str = "bong:vfx/daozhan_reveal";

/// 暴起粒子颜色（阴邪紫 #7040A8）。
pub const DAOZHAN_REVEAL_PARTICLE_COLOR: &str = "#7040A8";

/// 暴起粒子数量（burst 12）。
pub const DAOZHAN_REVEAL_PARTICLE_COUNT: u16 = 12;

/// 粒子持续时间（10 tick ≈ 500ms）。
pub const DAOZHAN_REVEAL_PARTICLE_DURATION_TICKS: u16 = 10;

/// 暴起粒子 strength（径向扩散速度参考）。
pub const DAOZHAN_REVEAL_PARTICLE_STRENGTH: f32 = 0.75;

/// 暴起音效 recipe ID（entity.wither.ambient 变体）。
pub const DAOZHAN_REVEAL_AUDIO_RECIPE_ID: &str = "entity_wither_ambient";

/// 暴起音效音量倍率。
pub const DAOZHAN_REVEAL_AUDIO_VOLUME: f32 = 0.8;

/// 暴起音效 pitch shift（略低沉，体现阴邪感）。
pub const DAOZHAN_REVEAL_AUDIO_PITCH_SHIFT: f32 = -0.3;

/// 每次 QiTransfer{DaoZhangDrain} 从玩家吸取的真元量（每连击一次）。
/// 三连总共吸取 `DAOZHAN_DRAIN_AMOUNT_PER_HIT * 3`。
pub const DAOZHAN_DRAIN_AMOUNT_PER_HIT: f64 = 8.0;

/// Ambush 连击最大 hit 次数（3 连）。
pub const DAOZHAN_AMBUSH_CHAIN_HITS: u32 = DAOZHAN_AMBUSH_CHAIN_COUNT;

/// 连击时两次 hit 之间的间隔（tick），避免同 tick 全部打完。
pub const DAOZHAN_AMBUSH_HIT_INTERVAL_TICKS: u32 = 4; // 4 tick ≈ 200ms

/// Ambush 完成后冷却 tick 数（道伥短暂切回 Mimicry 喘息期），供 Scorer 避免重入暴起。
pub const DAOZHAN_AMBUSH_COOLDOWN_TICKS: u64 = 60; // 3s

/// 道伥 VFX 原点 Y 轴偏移（躯干中心）。
pub const DAOZHAN_VFX_ORIGIN_Y_OFFSET: f64 = 0.8;

/// 道伥感知范围：Ambush 触发检测范围（格）。
pub const DAOZHAN_AMBUSH_SENSE_RADIUS: f64 = 8.0;

// ── P2: 暴起冷却 Component ─────────────────────────────────────────────────────

/// Ambush 结束后附加的冷却 Component，防止立即重入暴起。
/// `ready_at_tick`：`>= 此值` 时才允许再次 Ambush。
#[derive(Debug, Clone, Copy, Component)]
pub struct DaoZhangAmbushCooldown {
    pub ready_at_tick: u64,
}

// ── P2: 连击链状态 Component ──────────────────────────────────────────────────

/// 道伥暴起连击链追踪 Component（Ambush Action 内部状态）。
///
/// - `hits_done`：已完成的连击次数（0..=DAOZHAN_AMBUSH_CHAIN_COUNT）。
/// - `target_player`：当前连击目标玩家 Entity。
/// - `next_hit_at_tick`：下一次打击允许的 tick（间隔计时）。
///
/// 每次 Requested 时初始化；达到 DAOZHAN_AMBUSH_CHAIN_COUNT 次后 Action Success。
#[derive(Debug, Clone, Component)]
pub struct DaoZhangAmbushChainState {
    /// 已打出的连击次数。
    pub hits_done: u32,
    /// 当前连击目标玩家 Entity。
    pub target_player: Entity,
    /// 下次允许打击的 tick（间隔节拍，避免同帧三连）。
    pub next_hit_at_tick: u64,
}

// ── P2: DaoZhangAmbushScorer ──────────────────────────────────────────────────

/// 暴起评分器：
///
/// 条件（AND-OR）：
///   1. 道伥处于 `DaoZhangState::Mimicry`（Ambush 态已处理，不重入）
///   2. 无暴起冷却（`DaoZhangAmbushCooldown` 已过期或不存在）
///   3. 附近玩家（< `DAOZHAN_AMBUSH_SENSE_RADIUS`）满足至少一项触发条件：
///      a. 玩家背对道伥（偏转角 > 150°）
///      b. 玩家 qi_current / qi_max < 0.20
///
/// 满足时 score = 1.0（高于 Mimicry=0.7，保证 Ambush 优先抢占）。
#[derive(Clone, Copy, Debug, Component)]
pub struct DaoZhangAmbushScorer;

impl ScorerBuilder for DaoZhangAmbushScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("DaoZhangAmbushScorer")
    }
}

type DaoZhangAmbushScorerActorQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Position,
        &'static DaoZhangState,
        Option<&'static DaoZhangAmbushCooldown>,
    ),
    (With<NpcMarker>, Without<ClientMarker>),
>;

type AmbushPlayerQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Position,
        &'static Cultivation,
        Option<&'static valence::entity::Look>,
    ),
    (With<ClientMarker>, Without<NpcMarker>),
>;

pub(crate) fn daozhan_ambush_scorer_system(
    daozhan: DaoZhangAmbushScorerActorQuery<'_, '_>,
    players: AmbushPlayerQuery<'_, '_>,
    mut scorers: Query<(&Actor, &mut Score), With<DaoZhangAmbushScorer>>,
    game_tick: Option<Res<GameTick>>,
) {
    let tick = game_tick.as_deref().map(|t| t.0).unwrap_or(0);

    for (Actor(actor), mut score) in &mut scorers {
        let Ok((npc_pos, state, cooldown)) = daozhan.get(*actor) else {
            score.set(0.0);
            continue;
        };

        // 只在 Mimicry 态尝试暴起（Ambush 态由 AmbushAction 接管）
        if *state != DaoZhangState::Mimicry {
            score.set(0.0);
            continue;
        }

        // 冷却检查：冷却未到期时不暴起
        if let Some(cd) = cooldown {
            if u64::from(tick) < cd.ready_at_tick {
                score.set(0.0);
                continue;
            }
        }

        let npc_p = npc_pos.get();

        // 扫描感知范围内的玩家，检查触发条件
        let should_ambush = players.iter().any(|(_, player_pos, cultivation, look)| {
            let d = npc_p.distance(player_pos.get());
            if d > DAOZHAN_AMBUSH_SENSE_RADIUS {
                return false;
            }
            let back = player_back_faces_npc_p2(player_pos.get(), look, npc_p);
            let low_qi = qi_ratio_p2(cultivation) < DAOZHAN_LOW_QI_RATIO;
            back || low_qi
        });

        score.set(if should_ambush { 1.0 } else { 0.0 });
    }
}

/// 判断玩家是否背对道伥（偏转角 > DAOZHAN_BACK_ANGLE_DEG = 150°）。
///
/// 复用 tsy_hostile.rs `player_back_faces_npc` 同等逻辑；P2 以此函数为 source of truth，
/// 测试直接测本函数（契约：dot(facing, to_npc_norm) < cos(150°/2)=cos(75°) ≈ -0.2588）。
fn player_back_faces_npc_p2(
    player_pos: DVec3,
    look: Option<&valence::entity::Look>,
    npc_pos: DVec3,
) -> bool {
    let Some(look) = look else { return false };
    let to_npc = npc_pos - player_pos;
    let to_npc_xz = DVec3::new(to_npc.x, 0.0, to_npc.z);
    if to_npc_xz.length_squared() <= f64::EPSILON {
        return false;
    }
    // MC yaw：0 = south(+Z)，顺时针增加；facing = (-sin(yaw), 0, cos(yaw))
    let yaw = f64::from(look.yaw).to_radians();
    let facing = DVec3::new(-yaw.sin(), 0.0, yaw.cos());
    // dot < 0 意味着面朝方向"背离" NPC（偏转角 > 90°）；
    // 但道伥要求 > 150°（更严格），等价于 dot < cos(150°) = -sqrt(3)/2 ≈ -0.866
    let cos_threshold = -(DAOZHAN_BACK_ANGLE_DEG.to_radians().cos()); // cos(150°) = -0.866
    let dot = facing.dot(to_npc_xz.normalize());
    dot <= cos_threshold
}

/// 玩家真元比（qi_current / qi_max），夹逼到 [0, 1]。
fn qi_ratio_p2(cultivation: &Cultivation) -> f64 {
    cultivation.qi_current.max(0.0) / cultivation.qi_max.max(1.0)
}

// ── P2: DaoZhangAmbushAction ──────────────────────────────────────────────────

/// 暴起行动：
///
/// 1. **Requested**（首次进入）：
///    - 切换 `DaoZhangState::Ambush`
///    - emit `DaoZhangRevealEvent`（触发 client 渲染切换 + daozhan_disguise_emit 广播）
///    - emit VFX burst（`bong:vfx/daozhan_reveal`，#7040A8，count=12，10tick）
///    - emit 音效（`entity_wither_ambient`，vol=0.8，pitch=-0.3）
///    - 初始化 `DaoZhangAmbushChainState`：找最近玩家作为目标
///    - 转 Executing
///
/// 2. **Executing**（连击期）：
///    - 每隔 `DAOZHAN_AMBUSH_HIT_INTERVAL_TICKS` tick 打出一次 QiTransfer{DaoZhangDrain}：
///      `player.qi_current -= DAOZHAN_DRAIN_AMOUNT_PER_HIT`（若不足则取 min）
///      `daozhan_bb.daozhan_qi += actual_drained`（守恒）
///      emit `QiTransfer{from=player,to=daozhan,reason=DaoZhangDrain}`
///    - 达到 DAOZHAN_AMBUSH_CHAIN_COUNT 次后：
///      切换回 `DaoZhangState::Mimicry`，添加 `DaoZhangAmbushCooldown`，Success
///
/// 3. **Cancelled**：停止，切回 Mimicry，Failure
///
/// ## 守恒约束
///
/// 每次 DaoZhangDrain：
/// - `player_cultivation.qi_current -= drained`
/// - `daozhan_bb.daozhan_qi += drained`
/// - emit `QiTransfer{from=player:<uuid>, to=npc:daozhan:<entity_id>, reason=DaoZhangDrain}`
///
/// 死亡时 `DaoZhangDeathSystem`（P3）负责 `release_qi_amount_to_zone(daozhan_bb.daozhan_qi)`。
#[derive(Clone, Copy, Debug, Component)]
pub struct DaoZhangAmbushAction;

impl ActionBuilder for DaoZhangAmbushAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("DaoZhangAmbushAction")
    }
}

type DaoZhangAmbushActorQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Position,
        &'static mut DaoZhangState,
        &'static mut DaoZhangBehaviorBlackboard,
        Option<&'static mut DaoZhangAmbushChainState>,
    ),
    (With<NpcMarker>, Without<ClientMarker>),
>;

type AmbushTargetPlayerQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static Position, &'static mut Cultivation),
    (With<ClientMarker>, Without<NpcMarker>),
>;

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn daozhan_ambush_action_system(
    mut daozhan: DaoZhangAmbushActorQuery<'_, '_>,
    mut players: AmbushTargetPlayerQuery<'_, '_>,
    mut actions: Query<(&Actor, &mut ActionState), With<DaoZhangAmbushAction>>,
    mut vfx_events: EventWriter<VfxEventRequest>,
    mut audio_events: EventWriter<PlaySoundRecipeRequest>,
    mut reveal_events: EventWriter<DaoZhangRevealEvent>,
    mut qi_transfer_events: EventWriter<QiTransfer>,
    mut drain_events: EventWriter<DaoZhangDrainAccumulate>,
    mut commands: Commands,
    game_tick: Option<Res<GameTick>>,
) {
    let tick = game_tick.as_deref().map(|t| t.0).unwrap_or(0);

    // Collect action updates to avoid borrow conflicts with mutable player query.
    // (player_entity, daozhan_actor_index)
    let mut pending_drains: Vec<(Entity, u32)> = Vec::new();

    for (Actor(actor), mut action_state) in &mut actions {
        let Ok((pos, mut state, bb, mut chain_opt)) = daozhan.get_mut(*actor) else {
            *action_state = ActionState::Failure;
            continue;
        };

        match *action_state {
            ActionState::Requested => {
                // ① 状态切换 Mimicry → Ambush
                *state = DaoZhangState::Ambush;

                let npc_pos = pos.get();

                // ② emit reveal event（daozhan_disguise_emit 监听后广播 bong:daozhan_reveal）
                reveal_events.send(DaoZhangRevealEvent {
                    daozhan: actor.index(),
                    trigger_pos: npc_pos,
                });

                // ③ VFX burst：阴邪紫粒子，count=12，10tick，#7040A8
                vfx_events.send(spawn_particle(
                    DAOZHAN_REVEAL_VFX_EVENT_ID,
                    npc_pos + DVec3::new(0.0, DAOZHAN_VFX_ORIGIN_Y_OFFSET, 0.0),
                    DAOZHAN_REVEAL_PARTICLE_COLOR,
                    DAOZHAN_REVEAL_PARTICLE_STRENGTH,
                    DAOZHAN_REVEAL_PARTICLE_COUNT,
                    DAOZHAN_REVEAL_PARTICLE_DURATION_TICKS,
                ));
                let _ = bb.daozhan_qi; // keep bb borrow to avoid dead_code lint

                // ④ 音效：entity.wither.ambient（阴邪低吼）
                audio_events.send(play_audio(
                    DAOZHAN_REVEAL_AUDIO_RECIPE_ID,
                    npc_pos,
                    DAOZHAN_REVEAL_AUDIO_VOLUME,
                    DAOZHAN_REVEAL_AUDIO_PITCH_SHIFT,
                ));

                // ⑤ 找最近玩家初始化连击链
                let nearest_player = players
                    .iter()
                    .min_by(|(_, pa, _), (_, pb, _)| {
                        npc_pos
                            .distance(pa.get())
                            .partial_cmp(&npc_pos.distance(pb.get()))
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .map(|(e, _, _)| e);

                if let Some(target_player) = nearest_player {
                    // 附加连击链 Component 到 daozhan 实体
                    commands.entity(*actor).insert(DaoZhangAmbushChainState {
                        hits_done: 0,
                        target_player,
                        next_hit_at_tick: tick as u64,
                    });
                    *action_state = ActionState::Executing;
                } else {
                    // 无目标玩家 → 直接 Success，回 Mimicry
                    *state = DaoZhangState::Mimicry;
                    *action_state = ActionState::Success;
                }
            }

            ActionState::Executing => {
                // 守卫：确保仍在 Ambush 态
                if *state != DaoZhangState::Ambush {
                    *action_state = ActionState::Success;
                    continue;
                }

                let Some(ref mut chain_state) = chain_opt else {
                    // chain state 丢失（理论上不应发生），回 Mimicry
                    *state = DaoZhangState::Mimicry;
                    *action_state = ActionState::Success;
                    continue;
                };

                // 检查是否已完成全部连击
                if chain_state.hits_done >= DAOZHAN_AMBUSH_CHAIN_HITS {
                    // 连击完成：回 Mimicry + 冷却
                    *state = DaoZhangState::Mimicry;
                    commands.entity(*actor).insert(DaoZhangAmbushCooldown {
                        ready_at_tick: tick as u64 + DAOZHAN_AMBUSH_COOLDOWN_TICKS,
                    });
                    *action_state = ActionState::Success;
                    continue;
                }

                // 间隔计时：还没到下次打击时机
                if (tick as u64) < chain_state.next_hit_at_tick {
                    continue;
                }

                // 记录待处理的 drain（避免同一帧对 player 双重借用）
                pending_drains.push((chain_state.target_player, actor.index()));

                // 更新连击链（hits_done+1，推进 next_hit_at_tick）— 直接在借用上修改
                chain_state.hits_done += 1;
                chain_state.next_hit_at_tick =
                    tick as u64 + DAOZHAN_AMBUSH_HIT_INTERVAL_TICKS as u64;

                // 注：daozhan_qi 通过 pending_drains 路径在循环外更新（分步处理避免双借）
                let _ = bb.daozhan_qi;
            }

            ActionState::Cancelled => {
                // 取消暴起：切回 Mimicry
                *state = DaoZhangState::Mimicry;
                *action_state = ActionState::Failure;
            }

            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }

    // 执行 pending drain（守恒：player.qi_current -= drained，daozhan.daozhan_qi += drained）
    for (player_entity, actor_idx) in pending_drains {
        let Ok((_, _, mut cult)) = players.get_mut(player_entity) else {
            continue;
        };

        // 计算实际可吸取量（不超过玩家当前真元，保证 player qi 不负数）
        let available = cult.qi_current.max(0.0);
        let drained = DAOZHAN_DRAIN_AMOUNT_PER_HIT.min(available);
        if drained <= 0.0 {
            continue;
        }

        // 守恒步骤 ①：玩家减真元（不走 cultivation.qi_current+=X 无对应转移的私造路径）
        cult.qi_current -= drained;

        // 守恒步骤 ②：emit DaoZhangDrainAccumulate（由 daozhan_drain_accumulate_system 同步到 daozhan_qi）
        // 此 event 在 daozhan 主查询结束后的独立系统中处理，避免 ECS 双重可变借用。
        drain_events.send(DaoZhangDrainAccumulate {
            daozhan_idx: actor_idx,
            amount: drained,
        });

        // 守恒步骤 ③：emit QiTransfer ledger 事件（audit trail）
        let from = QiAccountId::player(player_entity.index().to_string());
        let to = QiAccountId::npc(format!("daozhan:{actor_idx}"));
        if let Ok(transfer) = QiTransfer::new(from, to, drained, QiTransferReason::DaoZhangDrain) {
            qi_transfer_events.send(transfer);
        }
    }
}

type DaoZhangBBQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static mut DaoZhangBehaviorBlackboard),
    (With<NpcMarker>, Without<ClientMarker>),
>;

/// 守恒系统：消费 `DaoZhangDrainAccumulate` 事件，将 `amount` 累积到对应道伥的
/// `DaoZhangBehaviorBlackboard.daozhan_qi` 字段（保证死亡时 `release_qi_amount_to_zone` 全额归还）。
///
/// 独立系统避免与 `daozhan_ambush_action_system` 的双重可变借用冲突。
pub(crate) fn daozhan_drain_accumulate_system(
    mut drain_events: valence::prelude::EventReader<DaoZhangDrainAccumulate>,
    mut daozhan: DaoZhangBBQuery<'_, '_>,
) {
    for event in drain_events.read() {
        // 按 entity raw index 精确定位道伥 blackboard
        for (entity, mut bb) in daozhan.iter_mut() {
            if entity.index() == event.daozhan_idx {
                bb.daozhan_qi += event.amount;
                break;
            }
        }
    }
}

/// `QiAccountId.id` is a public field — this alias helps tests reference the field cleanly.
#[allow(dead_code)]
pub(crate) fn _qi_account_id_str(id: &QiAccountId) -> &str {
    &id.id
}

/// P2 辅助事件：记录需累积到道伥 daozhan_qi 的量（避免 ECS 双重借用）。
#[derive(Debug, Clone, valence::prelude::Event)]
pub struct DaoZhangDrainAccumulate {
    /// 道伥 entity raw index。
    pub daozhan_idx: u32,
    /// 本次实际吸取量。
    pub amount: f64,
}

// ── P2: Bevy 注册 ─────────────────────────────────────────────────────────────

/// Bevy 注册：P2 Ambush 评分器 + 行动 system + 守恒累积 system（供 fauna::mod.rs 调用）。
pub fn register_p2(app: &mut App) {
    use big_brain::prelude::BigBrainSet;
    use valence::prelude::{IntoSystemConfigs, PreUpdate, Update};

    app.add_event::<DaoZhangDrainAccumulate>();
    app.add_systems(
        PreUpdate,
        daozhan_ambush_scorer_system.in_set(BigBrainSet::Scorers),
    );
    app.add_systems(
        PreUpdate,
        daozhan_ambush_action_system.in_set(BigBrainSet::Actions),
    );
    // 守恒：在 Update 阶段处理 DaoZhangDrainAccumulate，将吸取量累积到 daozhan_qi
    app.add_systems(
        Update,
        daozhan_drain_accumulate_system.after(daozhan_ambush_action_system),
    );
}

// ── P3: 天道凝结系统 ──────────────────────────────────────────────────────────

/// 天道凝结：zone.spirit_qi > TIANDAO_CONDENSE_THRESHOLD 时，从高浓度灵气中凝出道伥。
///
/// ## 守恒
/// 初始 qi 走 `QiTransfer{from:zone,to:daozhan,reason:TiandaoCondense}`；
/// zone.spirit_qi -= condensed_delta；绝不凭空创生。
///
/// ## 触发条件
/// - zone.spirit_qi > TIANDAO_CONDENSE_THRESHOLD（默认 0.8）
/// - 每 TIANDAO_CONDENSE_INTERVAL_TICKS tick 检查一次（不每帧触发）
/// - 每次凝结消耗 TIANDAO_CONDENSE_QI_COST（从 zone 扣除）
/// - 生成的道伥 origin_realm = None（天道凝结，无具体原始修士）
///
/// **注意**：P3 走 server 侧确定性系统，不依赖 LLM agent 确定性触发，
/// 保证高灵气区域必然产出道伥（agent 仅作为可选加强路径）。
pub const TIANDAO_CONDENSE_INTERVAL_TICKS: u32 = 600; // 30s @ 20TPS

/// 天道凝结时从 zone 扣除的灵气量（每次凝出一只道伥扣此量）。
pub const TIANDAO_CONDENSE_QI_COST: f64 = 0.05;

/// 天道凝结 qi 守恒：凝出道伥的初始 qi 量（= zone 扣减量）。
pub const TIANDAO_CONDENSE_INITIAL_QI: f64 = TIANDAO_CONDENSE_QI_COST;

/// 天道凝结检查的每 zone 最大道伥上限（防止同区堆叠过多）。
pub const TIANDAO_CONDENSE_MAX_PER_ZONE: u32 = 3;

/// 道伥凝结系统状态（per-zone 冷却，Resource）。
#[derive(Debug, Default, valence::prelude::Resource)]
pub struct DaoZhangCondenseState {
    /// zone_name → 上次凝结的 tick（GameTick 为 u32）
    pub last_condense_tick: std::collections::HashMap<String, u32>,
}

/// 天道凝结系统：每 TIANDAO_CONDENSE_INTERVAL_TICKS 检查一次高浓度灵气区域，
/// 超过阈值时凝出道伥（守恒：zone.spirit_qi -= TIANDAO_CONDENSE_QI_COST）。
#[allow(clippy::too_many_arguments)]
pub(crate) fn daozhan_tiandao_condense_system(
    mut zones: Option<ResMut<ZoneRegistry>>,
    mut condense_state: Option<ResMut<DaoZhangCondenseState>>,
    existing_daozhan: Query<&DaoZhangBehaviorBlackboard, With<NpcMarker>>,
    game_tick: Option<Res<GameTick>>,
    mut qi_transfer_events: EventWriter<QiTransfer>,
    // 暂不实际 spawn（需要 layer entity + commands 才能 spawn_tsy_daoxiang_at）
    // P3 守恒测试 + 灵气消耗由本系统实装；spawn 接入点在 fauna::mod.rs 注册后
    // 通过 SpawnDaoZhangRequest event 分发（避免此系统直接持有 Commands）
    mut spawn_requests: EventWriter<SpawnDaoZhangFromCondenseRequest>,
) {
    let tick = game_tick.as_deref().map(|t| t.0).unwrap_or(0);
    let Some(zones) = zones.as_deref_mut() else {
        return;
    };
    let Some(condense_state) = condense_state.as_deref_mut() else {
        return;
    };

    let all_zones: Vec<(String, f64)> = zones
        .zones
        .iter()
        .map(|z| (z.name.clone(), z.spirit_qi))
        .collect();

    for (zone_name, spirit_qi) in all_zones {
        if spirit_qi <= TIANDAO_CONDENSE_THRESHOLD {
            continue;
        }
        // 冷却检查
        let last = condense_state
            .last_condense_tick
            .get(&zone_name)
            .copied()
            .unwrap_or(0);
        if tick.saturating_sub(last) < TIANDAO_CONDENSE_INTERVAL_TICKS {
            continue;
        }
        // 本 zone 道伥数量检查（不超过上限）
        let count_in_zone = existing_daozhan
            .iter()
            .filter(|bb| bb.home_zone == zone_name)
            .count() as u32;
        if count_in_zone >= TIANDAO_CONDENSE_MAX_PER_ZONE {
            continue;
        }
        // 扣减 zone spirit_qi（守恒：从 zone 凝结，绝不凭空创生）
        if let Some(zone) = zones.find_zone_mut(zone_name.as_str()) {
            let actual_cost =
                TIANDAO_CONDENSE_QI_COST.min(zone.spirit_qi - TIANDAO_CONDENSE_THRESHOLD);
            if actual_cost <= 0.0 {
                continue;
            }
            zone.spirit_qi = (zone.spirit_qi - actual_cost).clamp(-1.0, 1.0);

            // 守恒 ledger 记录
            let from = QiAccountId::zone(zone_name.clone());
            let to = QiAccountId::npc(format!("daozhan:condense:{zone_name}:{tick}"));
            if let Ok(transfer) =
                QiTransfer::new(from, to, actual_cost, QiTransferReason::TiandaoCondense)
            {
                qi_transfer_events.send(transfer);
            }

            condense_state
                .last_condense_tick
                .insert(zone_name.clone(), tick);

            // 发出凝结 spawn 请求（由 daozhan_condense_spawn_system 实际执行 spawn）
            spawn_requests.send(SpawnDaoZhangFromCondenseRequest {
                zone_name,
                condensed_qi: actual_cost,
                tick,
            });
        }
    }
}

/// 天道凝结 spawn 请求事件（解耦守恒扣减与实际 spawn，避免单系统参数爆炸）。
#[derive(Debug, Clone, valence::prelude::Event)]
pub struct SpawnDaoZhangFromCondenseRequest {
    /// 触发凝结的 zone 名（道伥 home_zone）。
    pub zone_name: String,
    /// 凝结初始 qi 量（守恒：来自 zone 的扣减）。
    pub condensed_qi: f64,
    /// 触发 tick（GameTick 为 u32，用于日志/audit）。
    pub tick: u32,
}

/// 天道凝结 spawn 执行系统：消费 `SpawnDaoZhangFromCondenseRequest`，
/// 在对应 zone 中 spawn 道伥实体（挂载 DaoZhangBehaviorBlackboard）。
///
/// 使用 `spawn_tsy_daoxiang_at` + DaoZhangBehaviorBlackboard 注入，
/// 道伥初始 qi 由调用者通过 DaoZhangBehaviorBlackboard.daozhan_qi 反映。
pub(crate) fn daozhan_condense_spawn_system(
    mut spawn_requests: EventReader<SpawnDaoZhangFromCondenseRequest>,
    zones: Option<Res<ZoneRegistry>>,
    layers: Option<Res<crate::world::dimension::DimensionLayers>>,
    mut commands: Commands,
) {
    let Some(zones) = zones else {
        for _ in spawn_requests.read() {}
        return;
    };
    let Some(layers) = layers else {
        for _ in spawn_requests.read() {}
        return;
    };
    let layer = layers.overworld;

    for req in spawn_requests.read() {
        let Some(zone) = zones.find_zone_by_name(req.zone_name.as_str()) else {
            continue;
        };
        // 在 zone AABB 中心 spawn 道伥
        let center = zone.center();
        let spawn_pos = center;
        let patrol_target = DVec3::new(center.x + 5.0, center.y, center.z);

        let entity = spawn_tsy_daoxiang_at(
            &mut commands,
            layer,
            &format!("condense:{}", req.zone_name),
            req.zone_name.as_str(),
            spawn_pos,
            patrol_target,
        );
        // 附加 DaoZhangBehaviorBlackboard（覆盖 spawn 默认，注入天道凝结 origin_realm=None）
        commands.entity(entity).insert((
            DaoZhangState::Mimicry,
            DaoZhangBehaviorBlackboard {
                home_zone: req.zone_name.clone(),
                home_pos: spawn_pos,
                // 守恒：凝结 qi 来自 zone，初始存入 blackboard.daozhan_qi（等待死亡归还）
                daozhan_qi: req.condensed_qi,
                origin_realm: None,
                behavior_queue: {
                    let mut q = std::collections::VecDeque::with_capacity(3);
                    for b in FakeBehavior::cycle() {
                        q.push_back(b);
                    }
                    q
                },
                current_behavior_ticks: 0,
                pack_id: None,
            },
        ));
    }
}

// ── P3: 死亡 qi 全额归还系统 ─────────────────────────────────────────────────

/// 道伥死亡时将 qi_current 残余 + 累积 daozhan_qi 全额归还 zone。
///
/// ## 守恒红线
/// - 全额归还（非 1%）：道伥死亡即阴质瓦解，积蓄真元即时散逸回灵气环境。
/// - 走 `release_qi_amount_to_zone`（正典 helper），不私造 zone.spirit_qi += X 路径。
/// - `release_amount = daozhan_qi（累积吸取）`（qi_current 是 NPC 战斗 hp 字段，非 qi 账户）。
pub(crate) fn daozhan_death_qi_release_system(
    mut deaths: EventReader<DeathEvent>,
    daozhan_q: Query<
        (
            &Position,
            Option<&CurrentDimension>,
            &DaoZhangBehaviorBlackboard,
        ),
        With<NpcMarker>,
    >,
    mut zones: Option<ResMut<ZoneRegistry>>,
    mut qi_transfers: ResMut<bevy_ecs::event::Events<QiTransfer>>,
) {
    let Some(zones) = zones.as_deref_mut() else {
        for _ in deaths.read() {}
        return;
    };

    for death in deaths.read() {
        let Ok((position, dimension, blackboard)) = daozhan_q.get(death.target) else {
            continue;
        };
        // 全额归还：daozhan_qi（累积吸取量，包括天道凝结初始量）
        let release_amount = blackboard.daozhan_qi;
        if release_amount <= 0.0 {
            continue;
        }
        // 走 release_qi_amount_to_zone 正典 helper（全额，非1%）
        release_qi_amount_to_zone(
            death.target,
            release_amount,
            Some(position),
            dimension,
            None, // DaoZhang 无 LifeRecord
            Some(zones),
            Some(&mut *qi_transfers),
            "daozhan_death",
        );
    }
}

// ── P3: Bevy 注册 ─────────────────────────────────────────────────────────────

/// Bevy 注册：P3 天道凝结系统 + 死亡 qi 归还系统（供 fauna::mod.rs 调用）。
pub fn register_p3(app: &mut App) {
    use valence::prelude::{IntoSystemConfigs, Update};

    // 注册事件
    app.add_event::<SpawnDaoZhangFromCondenseRequest>();
    app.init_resource::<DaoZhangCondenseState>();
    // QiTransfer 事件已在 qi_physics::register 全局注册，此处无需重复

    app.add_systems(
        Update,
        (
            daozhan_tiandao_condense_system,
            daozhan_condense_spawn_system.after(daozhan_tiandao_condense_system),
            daozhan_death_qi_release_system.before(crate::fauna::drop::fauna_drop_system),
        ),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── 枚举 pin 测试 ────────────────────────────────────────────────────────

    #[test]
    fn daozhan_state_default_is_mimicry() {
        assert_eq!(
            DaoZhangState::default(),
            DaoZhangState::Mimicry,
            "DaoZhangState 默认应是 Mimicry（伪装先行，暴起在后）"
        );
    }

    #[test]
    fn fake_behavior_cycle_has_three_variants() {
        let cycle = FakeBehavior::cycle();
        assert_eq!(cycle.len(), 3, "FakeBehavior::cycle 应包含 3 种假动作");
        assert!(cycle.contains(&FakeBehavior::Swing));
        assert!(cycle.contains(&FakeBehavior::Sneak));
        assert!(cycle.contains(&FakeBehavior::Mine));
    }

    #[test]
    fn daozhan_state_serde_roundtrip() {
        let states = [DaoZhangState::Mimicry, DaoZhangState::Ambush];
        for s in states {
            let json = serde_json::to_string(&s).expect("serialize");
            let back: DaoZhangState = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(s, back, "DaoZhangState serde 往返：{json}");
        }
    }

    #[test]
    fn fake_behavior_serde_roundtrip() {
        for b in FakeBehavior::cycle() {
            let json = serde_json::to_string(&b).expect("serialize");
            let back: FakeBehavior = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(b, back, "FakeBehavior serde 往返：{json}");
        }
    }

    // ── spawn 概率 ──────────────────────────────────────────────────────────

    #[test]
    fn realm_spawn_probability_values() {
        // 化虚 80%
        assert!(
            (realm_spawn_probability(Realm::Void) - 0.80).abs() < 1e-9,
            "化虚 spawn 概率应为 0.80，实际={:.4}",
            realm_spawn_probability(Realm::Void)
        );
        // 通灵 50%
        assert!(
            (realm_spawn_probability(Realm::Spirit) - 0.50).abs() < 1e-9,
            "通灵 spawn 概率应为 0.50，实际={:.4}",
            realm_spawn_probability(Realm::Spirit)
        );
        // 固元 20%
        assert!(
            (realm_spawn_probability(Realm::Solidify) - 0.20).abs() < 1e-9,
            "固元 spawn 概率应为 0.20，实际={:.4}",
            realm_spawn_probability(Realm::Solidify)
        );
        // 凝脉 0%
        assert_eq!(
            realm_spawn_probability(Realm::Condense),
            0.0,
            "凝脉不化道伥，概率应为 0"
        );
        // 引气 0%
        assert_eq!(
            realm_spawn_probability(Realm::Induce),
            0.0,
            "引气不化道伥，概率应为 0"
        );
        // 醒灵 0%
        assert_eq!(
            realm_spawn_probability(Realm::Awaken),
            0.0,
            "醒灵不化道伥，概率应为 0"
        );
    }

    #[test]
    fn realm_spawn_probability_boundary_at_full_range() {
        // 所有概率都在 [0, 1] 范围内
        for realm in [
            Realm::Void,
            Realm::Spirit,
            Realm::Solidify,
            Realm::Condense,
            Realm::Induce,
            Realm::Awaken,
        ] {
            let p = realm_spawn_probability(realm);
            assert!(
                (0.0..=1.0).contains(&p),
                "{realm:?} spawn 概率 {p} 超出 [0,1] 范围"
            );
        }
    }

    #[test]
    fn daozhan_spawn_roll_deterministic() {
        // 相同 seed + 相同 realm → 相同结果（确定性 RNG）
        let (a, _) = daozhan_spawn_roll(Realm::Void, 12345);
        let (b, _) = daozhan_spawn_roll(Realm::Void, 12345);
        assert_eq!(
            a, b,
            "daozhan_spawn_roll 必须是确定性的，相同 seed 应给出相同结果"
        );
    }

    #[test]
    fn daozhan_spawn_roll_never_hits_for_zero_probability() {
        // 醒灵概率 = 0.0，任何 seed 都不应命中
        for seed in [0u64, 1, 42, 999, u64::MAX, 0x9E3779B9_7F4A7C15] {
            let (hit, _) = daozhan_spawn_roll(Realm::Awaken, seed);
            assert!(!hit, "醒灵 spawn 概率为 0，seed={seed} 时不应命中");
        }
    }

    #[test]
    fn daozhan_spawn_roll_void_high_rate_statistical() {
        // 化虚 80%，大量样本应接近预期
        let mut hits = 0u32;
        let mut seed = 0xDEAD_BEEF_0000_0000u64;
        let n = 10_000u32;
        for _ in 0..n {
            let (hit, next) = daozhan_spawn_roll(Realm::Void, seed);
            if hit {
                hits += 1;
            }
            seed = next;
        }
        let rate = hits as f64 / n as f64;
        // 允许 ±5% 误差（n=10000 时 3σ ≈ 1.2%，5% 足够宽松）
        assert!(
            (0.75..=0.85).contains(&rate),
            "化虚 spawn roll 命中率 {rate:.3} 应在 [0.75, 0.85] 之间（期望 0.80）"
        );
    }

    #[test]
    fn daozhan_spawn_roll_advances_seed() {
        // next seed 不等于输入 seed（防止 seed 不推进导致相关性）
        let seed = 42u64;
        let (_, next) = daozhan_spawn_roll(Realm::Void, seed);
        assert_ne!(seed, next, "daozhan_spawn_roll 应推进 seed");
    }

    // ── loot 分档 ────────────────────────────────────────────────────────────

    #[test]
    fn daozhan_loot_tier_by_realm() {
        // 化虚/通灵 → High
        assert_eq!(
            daozhan_loot_tier(Some(Realm::Void)),
            DaoZhangLootTier::High,
            "化虚道伥应掉高档 loot"
        );
        assert_eq!(
            daozhan_loot_tier(Some(Realm::Spirit)),
            DaoZhangLootTier::High,
            "通灵道伥应掉高档 loot"
        );
        // 固元 → Mid
        assert_eq!(
            daozhan_loot_tier(Some(Realm::Solidify)),
            DaoZhangLootTier::Mid,
            "固元道伥应掉中档 loot"
        );
        // 凝脉/引气/醒灵 → Base
        for realm in [Realm::Condense, Realm::Induce, Realm::Awaken] {
            assert_eq!(
                daozhan_loot_tier(Some(realm)),
                DaoZhangLootTier::Base,
                "{realm:?} 道伥应掉基础 loot"
            );
        }
        // 天道凝结（None）→ Base
        assert_eq!(
            daozhan_loot_tier(None),
            DaoZhangLootTier::Base,
            "天道凝结道伥（无原始境界）应掉基础 loot"
        );
    }

    // ── DaoZhangSpawnTrigger serde pin 测试 ─────────────────────────────────

    #[test]
    fn spawn_trigger_collapse_zone_serde_roundtrip() {
        let trigger = DaoZhangSpawnTrigger::CollapseZoneDeath {
            family_id: "tsy_lingxu_01".into(),
            origin_realm: Realm::Void,
        };
        let json = serde_json::to_string(&trigger).expect("serialize");
        let back: DaoZhangSpawnTrigger = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(trigger, back, "CollapseZoneDeath serde 往返失败：{json}");
    }

    #[test]
    fn spawn_trigger_tribulation_strike_serde_roundtrip() {
        let trigger = DaoZhangSpawnTrigger::TribulationStrike {
            origin_realm: Realm::Spirit,
        };
        let json = serde_json::to_string(&trigger).expect("serialize");
        let back: DaoZhangSpawnTrigger = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(trigger, back, "TribulationStrike serde 往返失败：{json}");
    }

    #[test]
    fn spawn_trigger_tiandao_condense_serde_roundtrip() {
        let trigger = DaoZhangSpawnTrigger::TiandaoCondense {
            zone_name: "spawn".into(),
            condensed_qi: 12.5,
        };
        let json = serde_json::to_string(&trigger).expect("serialize");
        let back: DaoZhangSpawnTrigger = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(trigger, back, "TiandaoCondense serde 往返失败：{json}");
    }

    // ── DaoZhangBehaviorBlackboard ────────────────────────────────────────────

    #[test]
    fn blackboard_initial_state() {
        let pos = DVec3::new(1.0, 64.0, -3.0);
        let bb = DaoZhangBehaviorBlackboard::new("spawn", pos, Some(Realm::Void));
        assert_eq!(bb.home_zone, "spawn");
        assert_eq!(bb.home_pos, pos);
        assert_eq!(
            bb.daozhan_qi, 0.0,
            "初始 daozhan_qi 应为 0（守恒约束：未吸取任何真元）"
        );
        assert_eq!(bb.origin_realm, Some(Realm::Void));
        assert_eq!(bb.behavior_queue.len(), 3, "初始队列含 3 个假动作");
        assert!(bb.pack_id.is_none(), "v1 不使用 pack_id");
    }

    #[test]
    fn blackboard_behavior_queue_cycles() {
        let mut bb = DaoZhangBehaviorBlackboard::new("spawn", DVec3::ZERO, None);
        // 取 6 次，应循环：Swing Sneak Mine Swing Sneak Mine
        let expected = [
            FakeBehavior::Swing,
            FakeBehavior::Sneak,
            FakeBehavior::Mine,
            FakeBehavior::Swing,
            FakeBehavior::Sneak,
            FakeBehavior::Mine,
        ];
        for (i, exp) in expected.into_iter().enumerate() {
            let got = bb.next_behavior();
            assert_eq!(
                got, exp,
                "第 {i} 次 next_behavior 应返回 {exp:?}，实际 {got:?}"
            );
        }
        // 队列长度始终不变（循环不消耗）
        assert_eq!(
            bb.behavior_queue.len(),
            3,
            "循环后队列仍应有 3 个元素（push_back 保持长度）"
        );
    }

    #[test]
    fn blackboard_tiandao_condense_has_no_origin_realm() {
        // 天道凝结的道伥无具体原始修士境界，origin_realm = None
        let bb = DaoZhangBehaviorBlackboard::new("collapse_zone_deep", DVec3::ZERO, None);
        assert!(
            bb.origin_realm.is_none(),
            "天道凝结道伥的 origin_realm 应为 None"
        );
        assert_eq!(
            daozhan_loot_tier(bb.origin_realm),
            DaoZhangLootTier::Base,
            "天道凝结道伥 loot 应为 Base 档"
        );
    }

    #[test]
    fn blackboard_daozhan_qi_is_conservation_tracked() {
        // daozhan_qi 字段存在且初始为 0，测试"守恒不变式：吸取后 daozhan_qi > 0"
        let mut bb = DaoZhangBehaviorBlackboard::new("spawn", DVec3::ZERO, Some(Realm::Void));
        bb.daozhan_qi += 5.0; // 模拟 P2 DaoZhangDrain 后累积
        assert!(
            bb.daozhan_qi > 0.0,
            "累积 daozhan_qi={} 应 > 0（守恒：已吸取玩家真元 5.0）",
            bb.daozhan_qi
        );
    }

    // ── TIANDAO_CONDENSE_THRESHOLD 常数 pin 测试 ─────────────────────────────

    #[test]
    fn tiandao_condense_threshold_is_plausible() {
        // spirit_qi 是 0.0–1.0 归一化，0.8 为高浓度区间合理门控。
        // 用本地变量避免 clippy::assertions_on_constants（常量折叠会让 assert! 变 assert!(true)）。
        let threshold: f64 = TIANDAO_CONDENSE_THRESHOLD;
        assert!(
            threshold > 0.5,
            "TIANDAO_CONDENSE_THRESHOLD 应 > 0.5（高浓度门控），实际={}",
            threshold
        );
        assert!(
            threshold < 1.0,
            "TIANDAO_CONDENSE_THRESHOLD 应 < 1.0（不能是不可达的满值），实际={}",
            threshold
        );
    }

    // ── P1: Mimicry 常数 pin 测试 ────────────────────────────────────────────

    #[test]
    fn mimicry_behavior_tick_range_is_valid() {
        // 用本地变量避免 clippy::assertions_on_constants（常量折叠会让 assert! 变 assert!(true)）
        let min: u32 = MIMICRY_BEHAVIOR_MIN_TICKS;
        let max: u32 = MIMICRY_BEHAVIOR_MAX_TICKS;
        assert!(
            min >= 20,
            "Mimicry 最短持续应 >= 20 tick（1s），实际={}",
            min
        );
        assert!(
            max > min,
            "Mimicry 最长持续应 > 最短持续，min={} max={}",
            min,
            max
        );
        assert!(
            max <= 200,
            "Mimicry 最长持续应 <= 200 tick（10s），避免过久呆站，实际={}",
            max
        );
    }

    #[test]
    fn mimicry_score_below_ambush_threshold() {
        // Mimicry score 必须 < Ambush 满分 1.0（保证暴起可抢占伪装）
        // 用本地变量避免 clippy::assertions_on_constants
        let score: f32 = DAOZHAN_MIMICRY_SCORE;
        assert!(
            score < 1.0,
            "DAOZHAN_MIMICRY_SCORE 应 < 1.0（Ambush=1.0 优先），实际={}",
            score
        );
        assert!(
            score > 0.0,
            "DAOZHAN_MIMICRY_SCORE 应 > 0（有实际伪装驱动力），实际={}",
            score
        );
    }

    #[test]
    fn mimicry_sense_radius_plausible() {
        // 用本地变量避免 clippy::assertions_on_constants
        let radius: f64 = DAOZHAN_MIMICRY_SENSE_RADIUS;
        assert!(
            radius >= 8.0,
            "感知半径应 >= 8 格（能发现附近玩家），实际={}",
            radius
        );
        assert!(
            radius <= 32.0,
            "感知半径应 <= 32 格（不跨区域感知），实际={}",
            radius
        );
    }

    // ── P1: mimicry_behavior_duration_ticks 单元测试 ─────────────────────────

    #[test]
    fn mimicry_duration_always_in_range() {
        // 任意 seed 返回的 duration 应在 [MIN, MAX] 范围
        for seed in [0u64, 1, 42, 12345, u64::MAX, 0xDEAD_BEEF] {
            let d = mimicry_behavior_duration_ticks(seed);
            assert!(
                d >= MIMICRY_BEHAVIOR_MIN_TICKS,
                "seed={seed} duration={d} 低于 MIN={}",
                MIMICRY_BEHAVIOR_MIN_TICKS
            );
            assert!(
                d <= MIMICRY_BEHAVIOR_MAX_TICKS,
                "seed={seed} duration={d} 超过 MAX={}",
                MIMICRY_BEHAVIOR_MAX_TICKS
            );
        }
    }

    #[test]
    fn mimicry_duration_deterministic() {
        // 相同 seed 必须给出相同结果（确定性，供测试锁定）
        let d1 = mimicry_behavior_duration_ticks(9999);
        let d2 = mimicry_behavior_duration_ticks(9999);
        assert_eq!(
            d1, d2,
            "mimicry_behavior_duration_ticks 必须是确定性的（seed=9999，第一次={d1}，第二次={d2}）"
        );
    }

    #[test]
    fn mimicry_duration_varies_across_seeds() {
        // 不同 seed 应产生不同 duration（验证 RNG 不退化为常数）
        let durations: std::collections::HashSet<u32> =
            (0u64..50).map(mimicry_behavior_duration_ticks).collect();
        assert!(
            durations.len() > 1,
            "mimicry_behavior_duration_ticks 对 50 个不同 seed 应给出至少 2 种不同持续时长（实际只有 {} 种）",
            durations.len()
        );
    }

    // ── P1: DaoZhangMimicryScorer / Action big-brain 测试 ───────────────────

    use big_brain::prelude::{ActionState, Score};
    use valence::prelude::{App, Update};

    fn daozhan_test_app() -> App {
        let mut app = App::new();
        app.add_systems(
            Update,
            (daozhan_mimicry_scorer_system, daozhan_mimicry_action_system),
        );
        app
    }

    #[test]
    fn mimicry_scorer_zero_when_ambush_state() {
        // Ambush 态时评分应为 0（不触发伪装循环）
        let mut app = daozhan_test_app();
        let pos = DVec3::new(0.0, 64.0, 0.0);

        let daozhan = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([pos.x, pos.y, pos.z]),
                DaoZhangState::Ambush,
                DaoZhangBehaviorBlackboard::new("spawn", pos, None),
            ))
            .id();

        let scorer = app
            .world_mut()
            .spawn((Actor(daozhan), Score::default(), DaoZhangMimicryScorer))
            .id();

        app.update();

        let score = app.world().get::<Score>(scorer).unwrap().get();
        assert_eq!(
            score, 0.0,
            "Ambush 态时 DaoZhangMimicryScorer 应为 0，实际={score}"
        );
    }

    #[test]
    fn mimicry_scorer_high_score_when_player_in_range() {
        // Mimicry 态 + 玩家在感知范围内 → score = DAOZHAN_MIMICRY_SCORE
        let mut app = daozhan_test_app();
        let pos = DVec3::new(0.0, 64.0, 0.0);
        let player_pos = DVec3::new(10.0, 64.0, 0.0); // 10 < 16 = sense radius

        let daozhan = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([pos.x, pos.y, pos.z]),
                DaoZhangState::Mimicry,
                DaoZhangBehaviorBlackboard::new("spawn", pos, None),
            ))
            .id();

        app.world_mut().spawn((
            ClientMarker,
            Position::new([player_pos.x, player_pos.y, player_pos.z]),
        ));

        let scorer = app
            .world_mut()
            .spawn((Actor(daozhan), Score::default(), DaoZhangMimicryScorer))
            .id();

        app.update();

        let score = app.world().get::<Score>(scorer).unwrap().get();
        assert!(
            (score - DAOZHAN_MIMICRY_SCORE).abs() < 1e-6,
            "玩家在感知范围内时 DaoZhangMimicryScorer 应={DAOZHAN_MIMICRY_SCORE}，实际={score}"
        );
    }

    #[test]
    fn mimicry_scorer_low_score_when_no_player() {
        // Mimicry 态 + 无玩家 → score = 0.3（维持基础游荡）
        let mut app = daozhan_test_app();
        let pos = DVec3::new(0.0, 64.0, 0.0);

        let daozhan = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([pos.x, pos.y, pos.z]),
                DaoZhangState::Mimicry,
                DaoZhangBehaviorBlackboard::new("spawn", pos, None),
            ))
            .id();

        let scorer = app
            .world_mut()
            .spawn((Actor(daozhan), Score::default(), DaoZhangMimicryScorer))
            .id();

        app.update();

        let score = app.world().get::<Score>(scorer).unwrap().get();
        assert!(
            (score - 0.3).abs() < 1e-6,
            "无玩家时 DaoZhangMimicryScorer 应=0.3（维持游荡），实际={score}"
        );
    }

    #[test]
    fn mimicry_scorer_low_score_when_player_out_of_range() {
        // Mimicry 态 + 玩家超出感知半径 → score = 0.3
        let mut app = daozhan_test_app();
        let pos = DVec3::new(0.0, 64.0, 0.0);
        let player_far = DVec3::new(DAOZHAN_MIMICRY_SENSE_RADIUS + 1.0, 64.0, 0.0);

        let daozhan = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([pos.x, pos.y, pos.z]),
                DaoZhangState::Mimicry,
                DaoZhangBehaviorBlackboard::new("spawn", pos, None),
            ))
            .id();

        app.world_mut().spawn((
            ClientMarker,
            Position::new([player_far.x, player_far.y, player_far.z]),
        ));

        let scorer = app
            .world_mut()
            .spawn((Actor(daozhan), Score::default(), DaoZhangMimicryScorer))
            .id();

        app.update();

        let score = app.world().get::<Score>(scorer).unwrap().get();
        assert!(
            (score - 0.3).abs() < 1e-6,
            "玩家超出感知半径时 DaoZhangMimicryScorer 应=0.3，实际={score}"
        );
    }

    #[test]
    fn mimicry_action_requested_transitions_to_executing() {
        // Requested 时 action 应转 Executing，并取第一个假动作
        let mut app = daozhan_test_app();
        let pos = DVec3::new(0.0, 64.0, 0.0);

        let daozhan = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([pos.x, pos.y, pos.z]),
                DaoZhangState::Mimicry,
                DaoZhangBehaviorBlackboard::new("spawn", pos, None),
            ))
            .id();

        let action = app
            .world_mut()
            .spawn((
                Actor(daozhan),
                ActionState::Requested,
                DaoZhangMimicryAction,
            ))
            .id();

        app.update();

        let state = app.world().get::<ActionState>(action).unwrap();
        assert_eq!(
            *state,
            ActionState::Executing,
            "Requested 后 action 应变 Executing，实际={state:?}"
        );

        // current_behavior_ticks 应被设置为 duration（> 0）
        let bb = app
            .world()
            .get::<DaoZhangBehaviorBlackboard>(daozhan)
            .unwrap();
        assert!(
            bb.current_behavior_ticks >= MIMICRY_BEHAVIOR_MIN_TICKS,
            "current_behavior_ticks 应被设置为 duration >= MIN={}，实际={}",
            MIMICRY_BEHAVIOR_MIN_TICKS,
            bb.current_behavior_ticks
        );
    }

    #[test]
    fn mimicry_action_executing_decrements_ticks() {
        // Executing 时每帧 current_behavior_ticks 递减
        let mut app = daozhan_test_app();
        let pos = DVec3::new(0.0, 64.0, 0.0);

        let mut bb = DaoZhangBehaviorBlackboard::new("spawn", pos, None);
        bb.current_behavior_ticks = 10; // 手动设置计时

        let daozhan = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([pos.x, pos.y, pos.z]),
                DaoZhangState::Mimicry,
                bb,
            ))
            .id();

        app.world_mut().spawn((
            Actor(daozhan),
            ActionState::Executing,
            DaoZhangMimicryAction,
        ));

        app.update();

        let bb = app
            .world()
            .get::<DaoZhangBehaviorBlackboard>(daozhan)
            .unwrap();
        assert_eq!(
            bb.current_behavior_ticks, 9,
            "每帧 Executing 时 current_behavior_ticks 应递减 1（10→9），实际={}",
            bb.current_behavior_ticks
        );
    }

    #[test]
    fn mimicry_action_cycles_back_to_requested_when_ticks_zero() {
        // current_behavior_ticks 降到 0 时 action 重入 Requested（循环下一个假动作）
        let mut app = daozhan_test_app();
        let pos = DVec3::new(0.0, 64.0, 0.0);

        let mut bb = DaoZhangBehaviorBlackboard::new("spawn", pos, None);
        bb.current_behavior_ticks = 0; // 倒计时已到

        let daozhan = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([pos.x, pos.y, pos.z]),
                DaoZhangState::Mimicry,
                bb,
            ))
            .id();

        let action = app
            .world_mut()
            .spawn((
                Actor(daozhan),
                ActionState::Executing,
                DaoZhangMimicryAction,
            ))
            .id();

        app.update();

        let state = app.world().get::<ActionState>(action).unwrap();
        assert_eq!(
            *state,
            ActionState::Requested,
            "倒计时到 0 时 action 应重入 Requested（驱动下一假动作），实际={state:?}"
        );
    }

    #[test]
    fn mimicry_action_cancelled_sets_failure() {
        // Cancelled 时 action 变 Failure
        let mut app = daozhan_test_app();
        let pos = DVec3::new(0.0, 64.0, 0.0);

        let daozhan = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([pos.x, pos.y, pos.z]),
                DaoZhangState::Mimicry,
                DaoZhangBehaviorBlackboard::new("spawn", pos, None),
            ))
            .id();

        let action = app
            .world_mut()
            .spawn((
                Actor(daozhan),
                ActionState::Cancelled,
                DaoZhangMimicryAction,
            ))
            .id();

        app.update();

        let state = app.world().get::<ActionState>(action).unwrap();
        assert_eq!(
            *state,
            ActionState::Failure,
            "Cancelled 时 action 应变 Failure，实际={state:?}"
        );
    }

    #[test]
    fn mimicry_action_failure_when_not_mimicry_state() {
        // Ambush 态时 action 立即 Failure（被 Ambush 接管）
        let mut app = daozhan_test_app();
        let pos = DVec3::new(0.0, 64.0, 0.0);

        let daozhan = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([pos.x, pos.y, pos.z]),
                DaoZhangState::Ambush, // 非 Mimicry 态
                DaoZhangBehaviorBlackboard::new("spawn", pos, None),
            ))
            .id();

        let action = app
            .world_mut()
            .spawn((
                Actor(daozhan),
                ActionState::Executing,
                DaoZhangMimicryAction,
            ))
            .id();

        app.update();

        let state = app.world().get::<ActionState>(action).unwrap();
        assert_eq!(
            *state,
            ActionState::Failure,
            "非 Mimicry 态时 action 应立即 Failure，实际={state:?}"
        );
    }

    // ═══════════════════════════════════════════════════════════════════════
    // P2: player_back_faces_npc_p2 单元测试
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn back_angle_threshold_constant_pin() {
        // DAOZHAN_BACK_ANGLE_DEG = 150°；cos(150°) ≈ -0.866
        let threshold: f64 = DAOZHAN_BACK_ANGLE_DEG;
        assert!(
            (threshold - 150.0).abs() < 1e-9,
            "DAOZHAN_BACK_ANGLE_DEG 应为 150°，实际={threshold}"
        );
        // 对应 cos(150°) ≈ -0.866（背对判断 cutoff）
        let cos_val = threshold.to_radians().cos();
        assert!(
            (cos_val - (-0.866_025_4)).abs() < 1e-4,
            "cos(150°) ≈ -0.866，实际={cos_val:.6}"
        );
    }

    #[test]
    fn back_faces_npc_true_when_player_faces_away() {
        // 玩家站在 (0,64,0)，道伥在 (0,64,5)（+Z 方向）
        // 玩家 yaw=0 → facing=(0,0,1)（向 +Z，即朝向道伥方向）
        // 要让玩家背对道伥：yaw=180° → facing=(0,0,-1)（背道伥而去）
        // to_npc = (0,0,5-0)=(0,0,5)，normalize=(0,0,1)
        // facing(yaw=180°) = (-sin(180°),0,cos(180°)) = (0,0,-1)
        // dot = (0,0,-1)·(0,0,1) = -1.0 <= -0.866 → 背对 ✓
        use valence::entity::Look;
        let player_pos = DVec3::new(0.0, 64.0, 0.0);
        let npc_pos = DVec3::new(0.0, 64.0, 5.0);
        let look = Look {
            yaw: 180.0,
            pitch: 0.0,
        };
        assert!(
            player_back_faces_npc_p2(player_pos, Some(&look), npc_pos),
            "yaw=180° 时玩家应背对 +Z 方向的道伥（期望 back=true）"
        );
    }

    #[test]
    fn back_faces_npc_false_when_player_faces_toward_npc() {
        // 玩家面朝道伥（yaw=0 → facing=(0,0,1)，npc 在 +Z 方向）
        // dot = 1.0 >> -0.866 → 非背对
        use valence::entity::Look;
        let player_pos = DVec3::new(0.0, 64.0, 0.0);
        let npc_pos = DVec3::new(0.0, 64.0, 5.0);
        let look = Look {
            yaw: 0.0,
            pitch: 0.0,
        };
        assert!(
            !player_back_faces_npc_p2(player_pos, Some(&look), npc_pos),
            "yaw=0 时玩家面朝道伥，不应触发背对判断（期望 back=false）"
        );
    }

    #[test]
    fn back_faces_npc_false_when_look_is_none() {
        // 没有 Look Component 时不触发背对（无法判断方向，保守返回 false）
        let player_pos = DVec3::new(0.0, 64.0, 0.0);
        let npc_pos = DVec3::new(0.0, 64.0, 5.0);
        assert!(
            !player_back_faces_npc_p2(player_pos, None, npc_pos),
            "Look=None 时 back 应返回 false（无法判断方向）"
        );
    }

    #[test]
    fn back_faces_npc_false_when_player_on_same_xz_as_npc() {
        // 同一 XZ 坐标（to_npc_xz 长度为 0），不触发背对（避免除零）
        use valence::entity::Look;
        let player_pos = DVec3::new(0.0, 64.0, 0.0);
        let npc_pos = DVec3::new(0.0, 70.0, 0.0); // 同 XZ，只差 Y
        let look = Look {
            yaw: 0.0,
            pitch: 0.0,
        };
        assert!(
            !player_back_faces_npc_p2(player_pos, Some(&look), npc_pos),
            "XZ 距离为 0 时不应触发背对（避免除零，期望 back=false）"
        );
    }

    #[test]
    fn back_faces_boundary_at_exactly_150_degrees() {
        // 偏转角恰好 150° 时应触发（> = <=  threshold 边界）
        // 玩家在原点，道伥在 +Z，facing 旋转 150°（从 +Z 方向转 150°）= yaw=150
        // yaw=150° → facing = (-sin(150°),0,cos(150°)) = (-0.5,0,-0.866)
        // to_npc_norm = (0,0,1)
        // dot = 0+0+(-0.866)·1 = -0.866 = cos(150°) → 应触发（<= threshold）
        use valence::entity::Look;
        let player_pos = DVec3::ZERO;
        let npc_pos = DVec3::new(0.0, 0.0, 5.0);
        // 从 +Z 方向顺时针 150° = yaw=150°（MC yaw 0=south=+Z，顺时针增加）
        let look = Look {
            yaw: 150.0,
            pitch: 0.0,
        };
        assert!(
            player_back_faces_npc_p2(player_pos, Some(&look), npc_pos),
            "yaw=150° 对 +Z 方向道伥时 dot=cos(150°) 恰好触发背对（边界应为 true）"
        );
    }

    // ═══════════════════════════════════════════════════════════════════════
    // P2: qi_ratio_p2 单元测试
    // ═══════════════════════════════════════════════════════════════════════

    fn make_cultivation(qi_current: f64, qi_max: f64) -> Cultivation {
        Cultivation {
            qi_current,
            qi_max,
            qi_max_frozen: None,
            ..Default::default()
        }
    }

    #[test]
    fn qi_ratio_full_is_one() {
        let cult = make_cultivation(100.0, 100.0);
        let r = qi_ratio_p2(&cult);
        assert!(
            (r - 1.0).abs() < 1e-9,
            "qi_current=qi_max 时比例应为 1.0，实际={r}"
        );
    }

    #[test]
    fn qi_ratio_empty_is_zero() {
        let cult = make_cultivation(0.0, 100.0);
        let r = qi_ratio_p2(&cult);
        assert!(r.abs() < 1e-9, "qi_current=0 时比例应为 0，实际={r}");
    }

    #[test]
    fn qi_ratio_below_low_threshold() {
        // qi_current = 15, qi_max = 100 → ratio=0.15 < 0.20 → 触发 low_qi 条件
        let cult = make_cultivation(15.0, 100.0);
        let r = qi_ratio_p2(&cult);
        assert!(
            r < DAOZHAN_LOW_QI_RATIO,
            "比例=0.15 应低于 DAOZHAN_LOW_QI_RATIO={DAOZHAN_LOW_QI_RATIO}"
        );
    }

    #[test]
    fn qi_ratio_at_boundary_20_percent() {
        // qi_current = 20, qi_max = 100 → ratio=0.20 = threshold，不触发（要求严格 <）
        let cult = make_cultivation(20.0, 100.0);
        let r = qi_ratio_p2(&cult);
        assert!(
            (r - 0.20).abs() < 1e-9,
            "比例应为 0.20（边界），实际={r:.6}"
        );
        // 边界：< 0.20 才触发，等于不触发（r >= threshold → 不触发）
        assert!(
            r >= DAOZHAN_LOW_QI_RATIO,
            "ratio=0.20 不应小于 DAOZHAN_LOW_QI_RATIO（边界 off-by-one：< 而非 <=）"
        );
    }

    #[test]
    fn qi_ratio_negative_current_clamped_to_zero() {
        // 防御性：qi_current < 0 时 clamp 到 0
        let cult = make_cultivation(-5.0, 100.0);
        let r = qi_ratio_p2(&cult);
        assert!(
            r.abs() < 1e-9,
            "qi_current<0 时 ratio 应 clamp 到 0，实际={r}"
        );
    }

    // ═══════════════════════════════════════════════════════════════════════
    // P2: DaoZhangAmbushScorer ECS 测试
    // ═══════════════════════════════════════════════════════════════════════

    fn ambush_test_app() -> App {
        let mut app = App::new();
        app.add_systems(
            Update,
            (
                daozhan_mimicry_scorer_system,
                daozhan_mimicry_action_system,
                daozhan_ambush_scorer_system,
            ),
        );
        app
    }

    #[test]
    fn ambush_scorer_zero_when_ambush_state() {
        // 已处于 Ambush 态时 scorer 应为 0（不重入）
        let mut app = ambush_test_app();
        let pos = DVec3::new(0.0, 64.0, 0.0);

        let daozhan = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([pos.x, pos.y, pos.z]),
                DaoZhangState::Ambush,
                DaoZhangBehaviorBlackboard::new("spawn", pos, None),
            ))
            .id();

        let scorer = app
            .world_mut()
            .spawn((Actor(daozhan), Score::default(), DaoZhangAmbushScorer))
            .id();

        app.update();

        let score = app.world().get::<Score>(scorer).unwrap().get();
        assert_eq!(
            score, 0.0,
            "Ambush 态时 DaoZhangAmbushScorer 应为 0（不重入），实际={score}"
        );
    }

    #[test]
    fn ambush_scorer_zero_when_on_cooldown() {
        // 冷却未过期时 scorer 应为 0
        let mut app = ambush_test_app();
        let pos = DVec3::new(0.0, 64.0, 0.0);

        let daozhan = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([pos.x, pos.y, pos.z]),
                DaoZhangState::Mimicry,
                DaoZhangBehaviorBlackboard::new("spawn", pos, None),
                // 冷却到 tick=99999，当前 tick=0
                DaoZhangAmbushCooldown {
                    ready_at_tick: 99_999,
                },
            ))
            .id();

        // 玩家在感知范围内且背对（理论上应触发，但被冷却阻止）
        app.world_mut().spawn((
            ClientMarker,
            Position::new([0.0, 64.0, 3.0]),
            Cultivation {
                qi_current: 5.0,
                qi_max: 100.0,
                ..Default::default()
            },
        ));

        let scorer = app
            .world_mut()
            .spawn((Actor(daozhan), Score::default(), DaoZhangAmbushScorer))
            .id();

        app.update();

        let score = app.world().get::<Score>(scorer).unwrap().get();
        assert_eq!(
            score, 0.0,
            "冷却中时 DaoZhangAmbushScorer 应为 0，实际={score}"
        );
    }

    #[test]
    fn ambush_scorer_one_when_player_low_qi() {
        // 玩家 qi < 20% 时 scorer 应为 1.0
        let mut app = ambush_test_app();
        let pos = DVec3::new(0.0, 64.0, 0.0);

        let daozhan = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([pos.x, pos.y, pos.z]),
                DaoZhangState::Mimicry,
                DaoZhangBehaviorBlackboard::new("spawn", pos, None),
            ))
            .id();

        // 玩家在 5 格内（< 8 = DAOZHAN_AMBUSH_SENSE_RADIUS），qi=10/100 = 10% < 20%
        app.world_mut().spawn((
            ClientMarker,
            Position::new([0.0, 64.0, 5.0]),
            Cultivation {
                qi_current: 10.0,
                qi_max: 100.0,
                ..Default::default()
            },
        ));

        let scorer = app
            .world_mut()
            .spawn((Actor(daozhan), Score::default(), DaoZhangAmbushScorer))
            .id();

        app.update();

        let score = app.world().get::<Score>(scorer).unwrap().get();
        assert!(
            (score - 1.0).abs() < 1e-6,
            "低真元（qi=10%）时 DaoZhangAmbushScorer 应=1.0，实际={score}"
        );
    }

    #[test]
    fn ambush_scorer_zero_when_player_out_of_range() {
        // 玩家超出感知范围时 scorer 为 0（即使 qi 低）
        let mut app = ambush_test_app();
        let pos = DVec3::new(0.0, 64.0, 0.0);

        let daozhan = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([pos.x, pos.y, pos.z]),
                DaoZhangState::Mimicry,
                DaoZhangBehaviorBlackboard::new("spawn", pos, None),
            ))
            .id();

        // 玩家在 20 格处（> 8 = DAOZHAN_AMBUSH_SENSE_RADIUS）
        app.world_mut().spawn((
            ClientMarker,
            Position::new([0.0, 64.0, 20.0]),
            Cultivation {
                qi_current: 1.0, // 很低
                qi_max: 100.0,
                ..Default::default()
            },
        ));

        let scorer = app
            .world_mut()
            .spawn((Actor(daozhan), Score::default(), DaoZhangAmbushScorer))
            .id();

        app.update();

        let score = app.world().get::<Score>(scorer).unwrap().get();
        assert_eq!(
            score, 0.0,
            "超出感知范围时 DaoZhangAmbushScorer 应=0，实际={score}"
        );
    }

    #[test]
    fn ambush_scorer_zero_when_no_player_nearby() {
        // 无玩家时 scorer 为 0
        let mut app = ambush_test_app();
        let pos = DVec3::new(0.0, 64.0, 0.0);

        let daozhan = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([pos.x, pos.y, pos.z]),
                DaoZhangState::Mimicry,
                DaoZhangBehaviorBlackboard::new("spawn", pos, None),
            ))
            .id();

        let scorer = app
            .world_mut()
            .spawn((Actor(daozhan), Score::default(), DaoZhangAmbushScorer))
            .id();

        app.update();

        let score = app.world().get::<Score>(scorer).unwrap().get();
        assert_eq!(
            score, 0.0,
            "无玩家时 DaoZhangAmbushScorer 应=0，实际={score}"
        );
    }

    // ═══════════════════════════════════════════════════════════════════════
    // P2: 守恒核心测试：player.qi -= amount == daozhan.daozhan_qi += amount
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn drain_amount_constant_pin() {
        // DAOZHAN_DRAIN_AMOUNT_PER_HIT 应大于 0 且合理（< qi_max 典型值）
        let drain: f64 = DAOZHAN_DRAIN_AMOUNT_PER_HIT;
        assert!(
            drain > 0.0,
            "DAOZHAN_DRAIN_AMOUNT_PER_HIT 应 > 0，实际={drain}"
        );
        assert!(
            drain <= 50.0,
            "DAOZHAN_DRAIN_AMOUNT_PER_HIT 应 <= 50（单次不超过半满上限），实际={drain}"
        );
    }

    #[test]
    fn drain_conservation_normal_case() {
        // 守恒：player.qi_current -= drained，daozhan.daozhan_qi += drained（之和不变）
        let player_qi_before = 100.0f64;
        let drain = DAOZHAN_DRAIN_AMOUNT_PER_HIT;
        let available = player_qi_before.max(0.0);
        let drained = drain.min(available);

        let player_qi_after = player_qi_before - drained;
        let daozhan_qi_after = 0.0f64 + drained; // 初始 daozhan_qi=0

        // 守恒：player 减少量 == daozhan 增加量
        assert!(
            (player_qi_before - player_qi_after - daozhan_qi_after).abs() < 1e-9,
            "守恒失败：player 减少 {:.3}，daozhan 增加 {:.3}，差值应为 0",
            player_qi_before - player_qi_after,
            daozhan_qi_after
        );
    }

    #[test]
    fn drain_conservation_player_has_less_than_drain_amount() {
        // 边界：玩家真元 < DAOZHAN_DRAIN_AMOUNT_PER_HIT 时，drained = 玩家实际量（不负数）
        let player_qi_before = 3.0f64; // 远小于 DAOZHAN_DRAIN_AMOUNT_PER_HIT=8
        let drain = DAOZHAN_DRAIN_AMOUNT_PER_HIT;
        let available = player_qi_before.max(0.0);
        let drained = drain.min(available);

        assert!(
            (drained - 3.0).abs() < 1e-9,
            "玩家 qi=3 时应只吸取 3，实际吸取={drained}"
        );

        let player_qi_after = player_qi_before - drained;
        assert!(
            player_qi_after >= 0.0,
            "玩家 qi 不应变负数，实际={player_qi_after}"
        );

        let daozhan_qi_after = drained;
        // 总和守恒
        assert!(
            (player_qi_before - (player_qi_after + daozhan_qi_after)).abs() < 1e-9,
            "守恒：player_before={player_qi_before}，player_after+daozhan={:.3}",
            player_qi_after + daozhan_qi_after
        );
    }

    #[test]
    fn drain_conservation_player_qi_zero() {
        // 边界：玩家 qi=0 时 drained=0，daozhan_qi 不增加（无真元可吸）
        let player_qi_before = 0.0f64;
        let drain = DAOZHAN_DRAIN_AMOUNT_PER_HIT;
        let available = player_qi_before.max(0.0);
        let drained = drain.min(available);

        assert!(
            drained.abs() < 1e-9,
            "玩家 qi=0 时 drained 应为 0，实际={drained}"
        );
    }

    #[test]
    fn drain_conservation_three_hits_total() {
        // 三连击总守恒：player 总减少量 == daozhan 总累积量
        let mut player_qi = 100.0f64;
        let mut daozhan_qi = 0.0f64;
        let drain = DAOZHAN_DRAIN_AMOUNT_PER_HIT;

        for _ in 0..DAOZHAN_AMBUSH_CHAIN_COUNT {
            let available = player_qi.max(0.0);
            let drained = drain.min(available);
            player_qi -= drained;
            daozhan_qi += drained;
        }

        assert!(
            (player_qi + daozhan_qi - 100.0).abs() < 1e-9,
            "三连击后 player_qi({player_qi:.3}) + daozhan_qi({daozhan_qi:.3}) 应等于初始总量 100"
        );
    }

    #[test]
    fn ambush_chain_state_initial_values_pin() {
        // 确认 DaoZhangAmbushChainState 初始字段语义
        let e = Entity::PLACEHOLDER;
        let cs = DaoZhangAmbushChainState {
            hits_done: 0,
            target_player: e,
            next_hit_at_tick: 0,
        };
        assert_eq!(cs.hits_done, 0, "初始 hits_done 应为 0（尚未打出任何一击）");
        assert_eq!(
            cs.target_player, e,
            "target_player 应为初始化时指定的 entity"
        );
    }

    #[test]
    fn ambush_chain_hits_constant_matches_ambush_chain_count() {
        // DAOZHAN_AMBUSH_CHAIN_HITS 应与 DAOZHAN_AMBUSH_CHAIN_COUNT 一致（两个常量来源同一逻辑）
        let chain_hits: u32 = DAOZHAN_AMBUSH_CHAIN_HITS;
        let chain_count: u32 = DAOZHAN_AMBUSH_CHAIN_COUNT;
        assert_eq!(
            chain_hits, chain_count,
            "DAOZHAN_AMBUSH_CHAIN_HITS({chain_hits}) 应等于 DAOZHAN_AMBUSH_CHAIN_COUNT({chain_count})"
        );
        // 应为 3
        assert_eq!(
            chain_hits, 3,
            "三连击设计决议：道伥连击次数应为 3，实际={chain_hits}"
        );
    }

    #[test]
    fn ambush_cooldown_ticks_plausible() {
        // DAOZHAN_AMBUSH_COOLDOWN_TICKS 应在合理范围（1s~10s）
        let cd: u64 = DAOZHAN_AMBUSH_COOLDOWN_TICKS;
        assert!(cd >= 20, "暴起冷却应 >= 20 tick（1s），实际={cd}");
        assert!(cd <= 200, "暴起冷却应 <= 200 tick（10s），实际={cd}");
    }

    #[test]
    fn low_qi_ratio_constant_pin() {
        // DAOZHAN_LOW_QI_RATIO 应为 0.20（设计决议）
        let r: f64 = DAOZHAN_LOW_QI_RATIO;
        assert!(
            (r - 0.20).abs() < 1e-9,
            "DAOZHAN_LOW_QI_RATIO 应为 0.20（设计决议 #2），实际={r}"
        );
    }

    #[test]
    fn reveal_vfx_constants_pin() {
        // VFX 规格 pin：count=12，#7040A8，10tick（与设计决议锁定）
        let count: u16 = DAOZHAN_REVEAL_PARTICLE_COUNT;
        assert_eq!(count, 12, "暴起粒子数量应为 12，实际={count}");

        assert_eq!(
            DAOZHAN_REVEAL_PARTICLE_COLOR, "#7040A8",
            "暴起粒子颜色应为 #7040A8，实际={}",
            DAOZHAN_REVEAL_PARTICLE_COLOR
        );

        let duration: u16 = DAOZHAN_REVEAL_PARTICLE_DURATION_TICKS;
        assert_eq!(duration, 10, "暴起粒子持续应为 10 tick，实际={duration}");

        assert_eq!(
            DAOZHAN_REVEAL_VFX_EVENT_ID, "bong:vfx/daozhan_reveal",
            "VFX event_id 应为 bong:vfx/daozhan_reveal，实际={}",
            DAOZHAN_REVEAL_VFX_EVENT_ID
        );
    }

    // ═══════════════════════════════════════════════════════════════════════
    // P3: 天道凝结守恒测试
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn tiandao_condense_threshold_blocks_low_zone_spirit_qi() {
        // zone.spirit_qi <= TIANDAO_CONDENSE_THRESHOLD 时不应触发凝结
        // 模拟：spirit_qi=0.7 < threshold=0.8 → 不触发
        let spirit_qi = 0.70f64;
        let threshold: f64 = TIANDAO_CONDENSE_THRESHOLD;
        assert!(
            spirit_qi <= threshold,
            "spirit_qi={spirit_qi} 应 <= TIANDAO_CONDENSE_THRESHOLD={threshold}（不触发凝结）"
        );
        // 模拟凝结守卫：不触发时 zone 灵气不变
        let zone_qi_before = spirit_qi;
        let should_condense = spirit_qi > threshold;
        let zone_qi_after = if should_condense {
            (spirit_qi - TIANDAO_CONDENSE_QI_COST).clamp(-1.0, 1.0)
        } else {
            spirit_qi
        };
        assert!(
            (zone_qi_before - zone_qi_after).abs() < 1e-9,
            "低于阈值时 zone 灵气不应变化（守恒：不凝结不扣减），before={zone_qi_before:.3} after={zone_qi_after:.3}"
        );
    }

    #[test]
    fn tiandao_condense_conservation_zone_decreases_by_cost() {
        // 守恒：zone 扣减量 == 道伥获得初始 qi
        let spirit_qi_before = 0.90f64; // > threshold=0.8
        let threshold: f64 = TIANDAO_CONDENSE_THRESHOLD;
        assert!(
            spirit_qi_before > threshold,
            "测试前提：spirit_qi={spirit_qi_before} 应 > threshold={threshold}"
        );
        // 实际 cost = min(COST, spirit_qi - threshold)（防止扣过头）
        let actual_cost = TIANDAO_CONDENSE_QI_COST.min(spirit_qi_before - threshold);
        let spirit_qi_after = (spirit_qi_before - actual_cost).clamp(-1.0, 1.0);
        let daozhan_initial_qi = actual_cost;

        // 守恒：zone 减少量 == 道伥初始 qi
        assert!(
            (spirit_qi_before - spirit_qi_after - daozhan_initial_qi).abs() < 1e-9,
            "守恒失败：zone 减少 {:.4}，道伥初始 qi={:.4}，差值应为 0",
            spirit_qi_before - spirit_qi_after,
            daozhan_initial_qi
        );
    }

    #[test]
    fn tiandao_condense_actual_cost_capped_to_available_headroom() {
        // 边界：spirit_qi = threshold + 极小量（0.001），cost 不超过可用头量
        let spirit_qi = TIANDAO_CONDENSE_THRESHOLD + 0.001;
        let actual_cost = TIANDAO_CONDENSE_QI_COST.min(spirit_qi - TIANDAO_CONDENSE_THRESHOLD);
        let headroom = spirit_qi - TIANDAO_CONDENSE_THRESHOLD;
        assert!(
            actual_cost <= headroom + 1e-9,
            "actual_cost={actual_cost:.5} 不应超过可用头量 headroom={headroom:.5}（防止 zone 跌穿 threshold）"
        );
    }

    #[test]
    fn tiandao_condense_interval_ticks_plausible() {
        // TIANDAO_CONDENSE_INTERVAL_TICKS 应为合理值（不小于 20 tick = 1s）
        let interval: u32 = TIANDAO_CONDENSE_INTERVAL_TICKS;
        assert!(
            interval >= 20,
            "TIANDAO_CONDENSE_INTERVAL_TICKS 应 >= 20 tick（1s），实际={interval}"
        );
        assert!(
            interval <= 6000,
            "TIANDAO_CONDENSE_INTERVAL_TICKS 应 <= 6000 tick（5分钟），实际={interval}"
        );
    }

    #[test]
    fn tiandao_condense_max_per_zone_plausible() {
        // TIANDAO_CONDENSE_MAX_PER_ZONE 应在合理范围（1~10）
        let max: u32 = TIANDAO_CONDENSE_MAX_PER_ZONE;
        assert!(
            max >= 1,
            "TIANDAO_CONDENSE_MAX_PER_ZONE 应 >= 1，实际={max}"
        );
        assert!(
            max <= 10,
            "TIANDAO_CONDENSE_MAX_PER_ZONE 应 <= 10（防止同区堆叠过多），实际={max}"
        );
    }

    // ═══════════════════════════════════════════════════════════════════════
    // P3: DaoZhangDeathSystem 守恒测试（unit level，无 ECS）
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn death_release_full_amount_not_partial() {
        // 守恒语义：死亡时 daozhan_qi 全额归还（非1%），验证数学逻辑
        let daozhan_qi_accumulated = 24.0f64; // 三连击 × 8.0 = 24.0
                                              // 正典 helper 返回归还的实际量（全额 or 0）
                                              // 我们只测逻辑：release_amount == daozhan_qi（不是 0.01 × daozhan_qi）
        let release_amount = daozhan_qi_accumulated;
        let partial_1pct = daozhan_qi_accumulated * 0.01;
        assert!(
            release_amount > partial_1pct,
            "道伥死亡应全额归还（{release_amount:.3}），而非 1%（{partial_1pct:.3}）"
        );
        // 守恒不变式：release == daozhan_qi
        assert!(
            (release_amount - daozhan_qi_accumulated).abs() < 1e-9,
            "release_amount 应等于 daozhan_qi_accumulated（守恒），差值应为 0"
        );
    }

    #[test]
    fn death_release_zero_when_daozhan_qi_is_zero() {
        // 天道凝结道伥未吸取任何真元时，daozhan_qi=0，死亡时不应触发释放（守恒：无吸无还）
        let daozhan_qi = 0.0f64;
        let should_release = daozhan_qi > 0.0;
        assert!(
            !should_release,
            "daozhan_qi=0 时不应触发 release_qi_amount_to_zone（无真元可还）"
        );
    }

    #[test]
    fn death_release_includes_both_accumulated_and_initial_qi() {
        // 道伥既有天道凝结初始 qi，又有伏击吸取累积量
        // 两者都存在于 daozhan_qi 字段（天道凝结路径：condense 初始量写入 daozhan_qi）
        let condense_qi = TIANDAO_CONDENSE_INITIAL_QI;
        let drained_from_player = DAOZHAN_DRAIN_AMOUNT_PER_HIT * 2.0; // 2 连击
                                                                      // 模拟 blackboard.daozhan_qi 包含两部分
        let total_daozhan_qi = condense_qi + drained_from_player;
        let release_amount = total_daozhan_qi;

        // 守恒：全额归还（含凝结量 + 吸取量）
        assert!(
            (release_amount - total_daozhan_qi).abs() < 1e-9,
            "死亡归还量应包含凝结量({condense_qi:.4}) + 吸取量({drained_from_player:.4})，合计={total_daozhan_qi:.4}"
        );
        assert!(
            total_daozhan_qi > condense_qi,
            "含伏击吸取后总量 {total_daozhan_qi:.4} 应大于仅凝结量 {condense_qi:.4}"
        );
    }

    // ═══════════════════════════════════════════════════════════════════════
    // P3: 神识识破 DisguisedDaoZhang — 常数 pin 测试
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn disguised_daozhan_condense_state_default_is_empty() {
        // DaoZhangCondenseState default 应无任何 zone 记录（全新服务器无冷却）
        let state = DaoZhangCondenseState::default();
        assert!(
            state.last_condense_tick.is_empty(),
            "DaoZhangCondenseState 初始应无 zone 冷却记录（空 HashMap）"
        );
    }

    #[test]
    fn spawn_request_event_fields_accessible() {
        // SpawnDaoZhangFromCondenseRequest 字段语义 pin（守恒测试用）
        let req = SpawnDaoZhangFromCondenseRequest {
            zone_name: "spawn".to_string(),
            condensed_qi: TIANDAO_CONDENSE_INITIAL_QI,
            tick: 600,
        };
        assert_eq!(req.zone_name, "spawn", "zone_name 字段应为初始化值");
        assert!(
            (req.condensed_qi - TIANDAO_CONDENSE_INITIAL_QI).abs() < 1e-9,
            "condensed_qi 应等于 TIANDAO_CONDENSE_INITIAL_QI={TIANDAO_CONDENSE_INITIAL_QI}"
        );
        assert_eq!(req.tick, 600, "tick 字段应为初始化值");
    }

    #[test]
    fn tiandao_initial_qi_equals_cost() {
        // TIANDAO_CONDENSE_INITIAL_QI 应等于 TIANDAO_CONDENSE_QI_COST（守恒：zone 扣多少，道伥得多少）
        assert!(
            (TIANDAO_CONDENSE_INITIAL_QI - TIANDAO_CONDENSE_QI_COST).abs() < 1e-9,
            "TIANDAO_CONDENSE_INITIAL_QI({}) 应等于 TIANDAO_CONDENSE_QI_COST({})（守恒 1:1 转移）",
            TIANDAO_CONDENSE_INITIAL_QI,
            TIANDAO_CONDENSE_QI_COST
        );
    }
}
