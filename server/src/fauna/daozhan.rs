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
use crate::cultivation::components::{
    release_external_qi_to_zone, transfer_cultivation_to_external_owner, ActorQiIdentity,
    ActorQiKind, Cultivation, Realm,
};
use crate::cultivation::life_record::LifeRecord;
use crate::fauna::drop::{build_fauna_item_instance, fauna_drop_seed, jittered_drop_pos};
use crate::fauna::experience::{play_audio, spawn_particle};
use crate::inventory::{
    DroppedLootEntry, DroppedLootRegistry, InventoryInstanceIdAllocator, ItemRegistry,
};
use crate::network::audio_event_emit::PlaySoundRecipeRequest;
use crate::network::daozhan_disguise_emit::DaoZhangRevealEvent;
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::npc::loot::{daozhan_loot_for_tier, roll_loot, NpcLootTable};
use crate::npc::movement::GameTick;
use crate::npc::spawn::NpcMarker;
use crate::npc::tsy_hostile::spawn_tsy_daoxiang_at;
use crate::qi_physics::ledger::{QiAccountId, QiTransfer, QiTransferReason, WorldQiAccount};
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
#[derive(Default)]
pub enum DaoZhangState {
    /// 伪装态：伪装为日常玩家行为（Swing / Sneak / Mine）诱近目标。
    #[default]
    Mimicry,
    /// 伏击态：暴露后连续 3 次 QiTransfer{DaoZhangDrain}，吸取真元。
    Ambush,
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
/// 测试直接测本函数（契约：dot(facing, to_npc_norm) < cos(150°) ≈ -0.866）。
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
    // 背对判断：玩家面向与"玩家→道伥"方向的夹角 > 150°
    // 等价于 dot(facing, to_npc_norm) < cos(150°) = -sqrt(3)/2 ≈ -0.866
    // 注意：不取反，cos(150°) 本身就是负数。
    let cos_threshold = DAOZHAN_BACK_ANGLE_DEG.to_radians().cos(); // ≈ -0.866
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
        Entity,
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
    (
        Entity,
        &'static Position,
        &'static LifeRecord,
        &'static mut Cultivation,
    ),
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
    mut ledger: ResMut<WorldQiAccount>,
    mut commands: Commands,
    game_tick: Option<Res<GameTick>>,
) {
    let tick = game_tick.as_deref().map(|t| t.0).unwrap_or(0);

    // Collect action updates to avoid borrow conflicts with mutable player query.
    // (player_entity, daozhan_actor_index)
    let mut pending_drains: Vec<(Entity, u32)> = Vec::new();

    for (Actor(actor), mut action_state) in &mut actions {
        let Ok((_entity, pos, mut state, bb, mut chain_opt)) = daozhan.get_mut(*actor) else {
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
                    .min_by(|(_, pa, _, _), (_, pb, _, _)| {
                        npc_pos
                            .distance(pa.get())
                            .partial_cmp(&npc_pos.distance(pb.get()))
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .map(|(e, _, _, _)| e);

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

                // 真元事务成功后才推进连击计数和下一击时钟；失败路径保持本次 hit 可重试。
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

    // 执行 pending drain：player Cultivation → 道伥 blackboard，两端物理 owner 与 audit 同步提交。
    for (player_entity, actor_idx) in pending_drains {
        let daozhan_entity = daozhan
            .iter_mut()
            .find_map(|(entity, _, _, _, _)| (entity.index() == actor_idx).then_some(entity));
        let Some(daozhan_entity) = daozhan_entity else {
            continue;
        };

        let committed = {
            let Ok((_, _, player_life_record, mut player_cultivation)) =
                players.get_mut(player_entity)
            else {
                continue;
            };
            let Ok((_, _, _, mut daozhan_blackboard, _)) = daozhan.get_mut(daozhan_entity) else {
                continue;
            };

            let drained = DAOZHAN_DRAIN_AMOUNT_PER_HIT.min(player_cultivation.qi_current.max(0.0));
            if drained <= 0.0 {
                true
            } else {
                let source = match ActorQiIdentity::from_life_record(
                    player_life_record,
                    ActorQiKind::Player,
                ) {
                    Ok(source) => source,
                    Err(error) => {
                        tracing::warn!(
                            ?error,
                            "[bong][fauna] daozhan drain missing canonical player identity"
                        );
                        continue;
                    }
                };
                let target = match ActorQiIdentity::from_canonical_npc_id(
                    crate::npc::brain::canonical_npc_id(daozhan_entity),
                ) {
                    Ok(target) => target,
                    Err(error) => {
                        tracing::warn!(
                            ?error,
                            "[bong][fauna] daozhan drain invalid target identity"
                        );
                        continue;
                    }
                };
                match transfer_cultivation_to_external_owner(
                    &mut player_cultivation,
                    &mut daozhan_blackboard.daozhan_qi,
                    &mut ledger,
                    &source,
                    &target,
                    drained,
                    QiTransferReason::DaoZhangDrain,
                ) {
                    Ok(outcome) => {
                        for transfer in outcome.transfers {
                            qi_transfer_events.send(transfer);
                        }
                        true
                    }
                    Err(error) => {
                        tracing::warn!(?error, "[bong][fauna] daozhan drain failed closed");
                        false
                    }
                }
            }
        };

        if committed {
            if let Ok((_, _, _, _, Some(mut chain_state))) = daozhan.get_mut(daozhan_entity) {
                chain_state.hits_done = chain_state.hits_done.saturating_add(1);
                chain_state.next_hit_at_tick =
                    tick as u64 + DAOZHAN_AMBUSH_HIT_INTERVAL_TICKS as u64;
            }
        }
    }
}

/// `QiAccountId.id` is a public field — this alias helps tests reference the field cleanly.
#[allow(dead_code)]
pub(crate) fn _qi_account_id_str(id: &QiAccountId) -> &str {
    &id.id
}

/// P2 辅助事件：保留既有 wire / 测试契约；生产伏击路径直接走 typed owner transaction。
#[derive(Debug, Clone, valence::prelude::Event)]
pub struct DaoZhangDrainAccumulate {
    /// 道伥 entity raw index。
    pub daozhan_idx: u32,
    /// 本次实际吸取量。
    pub amount: f64,
}

// ── P2: Bevy 注册 ─────────────────────────────────────────────────────────────

/// Bevy 注册：P2 Ambush 评分器 + 行动 system；生产吸取在行动 system 内同步提交两端 owner 与 audit。
pub fn register_p2(app: &mut App) {
    use big_brain::prelude::BigBrainSet;
    use valence::prelude::{IntoSystemConfigs, PreUpdate};

    app.add_event::<DaoZhangDrainAccumulate>();
    app.add_systems(
        PreUpdate,
        daozhan_ambush_scorer_system.in_set(BigBrainSet::Scorers),
    );
    app.add_systems(
        PreUpdate,
        daozhan_ambush_action_system.in_set(BigBrainSet::Actions),
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
    technique_registry: Res<crate::cultivation::known_techniques::TechniqueRegistry>,
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
            &technique_registry,
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

/// 道伥死亡时将累积 `daozhan_qi` 全额归还 zone。
///
/// ## 守恒红线
/// - 全额归还（非 1%）：道伥死亡即阴质瓦解，积蓄真元即时散逸回灵气环境。
/// - blackboard 是外部物理 owner；通过 typed external-owner transaction 原子扣减，不能伪造
///   玩家 `LifeRecord`，也不能只 emit `QiTransfer`。
/// - durable NPC identity 使用 canonical `npc_<index>v<generation>`，不使用 `Entity` Debug。
pub(crate) fn daozhan_death_qi_release_system(
    mut deaths: EventReader<DeathEvent>,
    mut daozhan_q: Query<
        (
            &Position,
            Option<&CurrentDimension>,
            &mut DaoZhangBehaviorBlackboard,
        ),
        With<NpcMarker>,
    >,
    mut zones: Option<ResMut<ZoneRegistry>>,
    mut ledger: ResMut<WorldQiAccount>,
    mut qi_transfers: ResMut<bevy_ecs::event::Events<QiTransfer>>,
) {
    for death in deaths.read() {
        let Ok((position, dimension, mut blackboard)) = daozhan_q.get_mut(death.target) else {
            continue;
        };
        let release_amount = blackboard.daozhan_qi;
        if release_amount <= 0.0 {
            continue;
        }
        let zone = match (dimension, zones.as_deref_mut()) {
            (Some(dimension), Some(zones)) => {
                let zone_name = zones
                    .find_zone(dimension.0, position.0)
                    .map(|zone| zone.name.clone());
                zone_name.and_then(|zone_name| zones.find_zone_mut(zone_name.as_str()))
            }
            _ => None,
        };
        let outcome = release_external_qi_to_zone(
            &mut blackboard.daozhan_qi,
            QiAccountId::npc(crate::npc::brain::canonical_npc_id(death.target)),
            zone,
            &mut ledger,
            release_amount,
            QiTransferReason::ReleaseToZone,
        );
        match outcome {
            Ok(outcome) => {
                for transfer in outcome.transfers {
                    qi_transfers.send(transfer);
                }
            }
            Err(error) => {
                tracing::warn!(
                    ?error,
                    target = ?death.target,
                    "[bong][daozhan] death qi release failed closed"
                );
            }
        }
    }
}

// ── P3: 死亡掉落系统（M2 fix）─────────────────────────────────────────────────

/// 道伥死亡时按 origin_realm 分档产出 loot，接通 npc::loot::daozhan_loot_for_tier。
///
/// 走 `crate::inventory::DroppedLootRegistry` + `InventoryInstanceIdAllocator`，
/// 与 fauna_drop_system 相同的掉落登记路径，保证 pickup 逻辑正常识别。
pub(crate) fn daozhan_death_loot_system(
    mut deaths: EventReader<DeathEvent>,
    daozhan_q: Query<
        (
            &Position,
            Option<&CurrentDimension>,
            &DaoZhangBehaviorBlackboard,
        ),
        With<NpcMarker>,
    >,
    item_registry: Option<Res<ItemRegistry>>,
    decay_profiles: Option<Res<crate::shelflife::DecayProfileRegistry>>,
    mut allocator: Option<ResMut<InventoryInstanceIdAllocator>>,
    mut loot_registry: Option<ResMut<DroppedLootRegistry>>,
) {
    let (Some(item_registry), Some(allocator), Some(loot_registry)) = (
        item_registry.as_deref(),
        allocator.as_deref_mut(),
        loot_registry.as_deref_mut(),
    ) else {
        return;
    };
    for death in deaths.read() {
        let Ok((pos, dimension, blackboard)) = daozhan_q.get(death.target) else {
            continue;
        };
        let tier = daozhan_loot_tier(blackboard.origin_realm);
        let loot_entries = daozhan_loot_for_tier(tier);
        if loot_entries.is_empty() {
            continue;
        }
        let seed = fauna_drop_seed(death.target, death.at_tick);
        let dim = dimension
            .map(|d| d.0)
            .unwrap_or(crate::world::dimension::DimensionKind::Tsy);
        // roll_loot 已处理 chance × stack range，不重复实现 RNG
        let table = NpcLootTable::new(crate::npc::lifecycle::NpcArchetype::Daoxiang, loot_entries);
        let rolled = roll_loot(&table, seed);
        for (idx, loot) in rolled.into_iter().enumerate() {
            let Ok(item) = build_fauna_item_instance(
                loot.template_id.as_str(),
                loot.stack,
                death.at_tick,
                item_registry,
                decay_profiles.as_deref(),
                allocator,
            ) else {
                tracing::warn!(
                    "[bong][daozhan] loot drop `{}` skipped: template missing",
                    loot.template_id
                );
                continue;
            };
            let world_pos = jittered_drop_pos(pos.get(), seed, idx as u64);
            loot_registry.entries.insert(
                item.instance_id,
                DroppedLootEntry {
                    instance_id: item.instance_id,
                    source_container_id: format!("daozhan_death:{:?}", blackboard.origin_realm),
                    source_row: 0,
                    source_col: 0,
                    world_pos,
                    dimension: dim,
                    item,
                },
            );
        }
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
            // M2 fix：道伥死亡 loot 掉落（接通 npc::loot::daozhan_loot_for_tier）
            daozhan_death_loot_system.after(daozhan_death_qi_release_system),
        ),
    );
}

#[cfg(test)]
#[path = "daozhan_tests.rs"]
mod tests;
