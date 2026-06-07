//! plan-daozhan-v1 P0/P1 — 道伥核心数据结构、spawn 触发逻辑与 Mimicry big-brain AI。
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
//! ## 守恒红线
//!
//! 1. **攻击吸取**（P2 实装）：走 `QiTransfer{DaoZhangDrain}`；player.qi_current -= amount，
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
    bevy_ecs, App, Commands, Component, DVec3, Entity, Position, Query, With, Without,
};

use crate::cultivation::components::Realm;
use crate::npc::movement::GameTick;
use crate::npc::spawn::NpcMarker;

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
}
