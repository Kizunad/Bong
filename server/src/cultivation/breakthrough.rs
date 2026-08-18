//! 境界突破（plan §3.1 / §3.2）。
//!
//! 支持 5 条升阶路径：Awaken→Induce→Condense→Solidify→Spirit→Void。
//! 成功率公式（plan §3.1）：
//!   `success = base × meridian_integrity × composure × completeness × (1 + bonus)`
//! 辅助材料 bonus 封顶 +0.30。
//!
//! 化虚渡劫为特殊流程（§3.2）：不走本 system 的 try_breakthrough，而是
//! `tribulation.rs::initiate_tribulation` 分发天劫事件。

use std::collections::HashMap;

use valence::prelude::{
    bevy_ecs, bevy_ecs::system::SystemParam, BlockPos, Commands, Component, Entity, Event,
    EventReader, EventWriter, Events, Position, Query, Res, ResMut, Username,
};

use crate::combat::components::StatusEffects;
use crate::combat::status::{
    clear_breakthrough_boost, clear_du_jie_dan_damage_reduction, sum_breakthrough_boost,
};
use crate::network::gameplay_vfx;
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::npc::spawn::NpcMarker;
use crate::player::gameplay::PendingGameplayNarrations;
use crate::qi_physics::{QiAccountId, QiPhysicsError, QiTransferReason, WorldQiAccount};
use crate::schema::common::NarrationStyle;
use crate::skill::components::SkillId;
use crate::skill::events::{SkillCapChanged, SkillXpGain, XpGainSource};
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::karma::{KarmaWeightStore, KARMA_WEIGHT_MAX};
use crate::world::season::{query_season, Season};
use crate::world::spirit_eye::{SpiritEyeId, SpiritEyeRegistry, SpiritEyeUsedForBreakthroughEvent};
use crate::world::zone::ZoneRegistry;

use super::components::{CrackCause, Cultivation, MeridianCrack, MeridianSystem, Realm};
use super::death_hooks::{CultivationDeathCause, CultivationDeathTrigger};
use super::life_record::{BiographyEntry, LifeRecord};
use super::meridian_open::MIN_ZONE_QI_TO_OPEN;
use super::tick::CultivationClock;

pub const RAPID_BREAKTHROUGH_KARMA_WINDOW_TICKS: u64 = 30 * 24 * 60 * 60 * 20;
pub const RAPID_BREAKTHROUGH_KARMA_WEIGHT_DELTA: f32 = KARMA_WEIGHT_MAX;
pub const MIN_ZONE_QI_TO_BREAKTHROUGH: f64 = MIN_ZONE_QI_TO_OPEN;
pub const MIN_ZONE_QI_TO_GUYUAN: f64 = 0.80;
pub const SPIRIT_EYE_BREAKTHROUGH_SUCCESS_BONUS: f64 = 0.30;
pub const BLOOD_VALLEY_BREAKTHROUGH_SUCCESS_BONUS: f64 = 0.50;
/// 突破失败时 `qi_max_frozen` 累计上限（占 qi_max 的比例）。
///
/// 与 `overload.rs` 的 0.5 上限对齐：多次连续失败不会把有效真元上限打到 0，
/// 玩家始终保留至少 50% 的 qi_max 可用额度，避免永久废人。
pub const BREAKTHROUGH_FAIL_FROZEN_CAP_RATIO: f64 = 0.5;

/// 每境界的基础成功率（未叠心境/完整度/材料）。
pub fn base_success_rate(next: Realm) -> f64 {
    match next {
        Realm::Awaken => 1.0,
        Realm::Induce => 0.90,
        Realm::Condense => 0.80,
        Realm::Solidify => 0.70,
        Realm::Spirit => 0.55,
        Realm::Void => 0.30,
    }
}

/// 各境界的 qi 消耗门槛。
pub fn breakthrough_qi_cost(next: Realm) -> f64 {
    match next {
        Realm::Awaken => 0.0,
        Realm::Induce => 8.0,
        Realm::Condense => 25.0,
        Realm::Solidify => 80.0,
        Realm::Spirit => 250.0,
        Realm::Void => 800.0,
    }
}

/// 下一境界（与 try_breakthrough 内部 match 一致）。Void 返回 None。
pub fn next_realm(r: Realm) -> Option<Realm> {
    match r {
        Realm::Awaken => Some(Realm::Induce),
        Realm::Induce => Some(Realm::Condense),
        Realm::Condense => Some(Realm::Solidify),
        Realm::Solidify => Some(Realm::Spirit),
        Realm::Spirit => Some(Realm::Void),
        Realm::Void => None,
    }
}

/// qi_max 乘数（突破后真元池扩张）。
pub fn qi_max_multiplier(next: Realm) -> f64 {
    match next {
        Realm::Awaken => 1.0,
        Realm::Induce => 2.0,
        Realm::Condense => 2.5,
        Realm::Solidify => 3.0,
        Realm::Spirit => 3.5,
        Realm::Void => 5.0,
    }
}

/// NPC spawn 时按境界直接确定 `qi_max`（容量上限，非当前真元）。
///
/// plan-npc-realm-distribution-v1 §8.1 #2 决议：全仓不存在可组合出这六个数值的
/// 干净递推公式（纯乘链 / 递推加法式均与正典不吻合），因此直接把 worldview
/// §三:195 权威表转写为查表——这是决议明确允许的兜底实现，六个输出值必须与
/// 正典表逐一相等（10 / 40 / 150 / 540 / 2100 / 10700）。
///
/// 只决定容量上限；`qi_current` 由调用方显式保持 `0.0`（不满灵），真元靠
/// `apply_dormant_regen_with_multiplier` 从 zone 逐步吸收，不撞 qi_physics 守恒红线。
pub fn qi_max_for_realm(realm: Realm) -> f64 {
    match realm {
        Realm::Awaken => 10.0,
        Realm::Induce => 40.0,
        Realm::Condense => 150.0,
        Realm::Solidify => 540.0,
        Realm::Spirit => 2100.0,
        Realm::Void => 10700.0,
    }
}

/// plan-skill-v1 §4 境界软挂钩：每个境界压制 skill 的 `effective_lv = min(real_lv, cap)`。
///
/// 数值表（plan §4）：醒灵=3 · 引气=5 · 凝脉=7 · 固元=8 · 通灵=9 · 化虚=10。
/// 代码 Realm 枚举的中文对照见 `components.rs`（Awaken=醒灵 / Induce=引气 / Condense=凝脉 /
/// Solidify=固元 / Spirit=通灵 / Void=化虚）。
pub fn skill_cap_for_realm(realm: Realm) -> u8 {
    match realm {
        Realm::Awaken => 3,
        Realm::Induce => 5,
        Realm::Condense => 7,
        Realm::Solidify => 8,
        Realm::Spirit => 9,
        Realm::Void => 10,
    }
}

#[derive(Debug, Clone, Event)]
pub struct BreakthroughRequest {
    pub entity: Entity,
    pub material_bonus: f64, // 0.0..=0.30
}

#[derive(Debug, Clone, Event)]
pub struct BreakthroughOutcome {
    pub entity: Entity,
    pub from: Realm,
    pub result: Result<BreakthroughSuccess, BreakthroughError>,
}

#[derive(Debug, Clone, Copy)]
pub struct BreakthroughSuccess {
    pub to: Realm,
    pub success_rate: f64,
    pub used_qi: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BreakthroughError {
    AtMaxRealm,
    RequiresTribulation, // Spirit→Void 必须走 tribulation 流程
    NotEnoughMeridians {
        need: usize,
        have: usize,
    },
    NotEnoughRegularMeridians {
        need: usize,
        have: usize,
    },
    NotEnoughExtraordinaryMeridians {
        need: usize,
        have: usize,
    },
    NotEnoughQi {
        need: f64,
        have: f64,
    },
    ZoneTooWeak {
        need: f64,
        have: f64,
    },
    EnvInsufficient {
        need: f64,
        have: f64,
        in_spirit_eye: bool,
    },
    LedgerUnavailable,
    RolledFailure {
        severity: f64,
    }, // 骰子输了
    /// review r2 major-2 收口：目标实体成功解析出一个真实（非 humanoid 兜底）
    /// `BodyPlan`，但该 plan 未声明 `meridian_profile`——fail-closed 拒绝突破，
    /// 不静默借用 humanoid 1/3/6/12/16/20 曲线（见
    /// `body_plan::MeridianProfileMissingError` 文档）。
    RaceProfileIncomplete,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum BreakthroughLedgerError {
    MissingStableActorId { is_npc: bool },
    QiPhysics(QiPhysicsError),
}

impl From<QiPhysicsError> for BreakthroughLedgerError {
    fn from(error: QiPhysicsError) -> Self {
        Self::QiPhysics(error)
    }
}

fn breakthrough_error_message(error: &BreakthroughError) -> String {
    match error {
        BreakthroughError::AtMaxRealm => "突破未成：你已抵达当前最高境界。".to_string(),
        BreakthroughError::RequiresTribulation => {
            "突破未成：通灵至化虚必须先走渡虚劫。".to_string()
        }
        BreakthroughError::NotEnoughMeridians { need, have } => {
            format!("突破未成：需先打通 {need} 条经脉（当前 {have}）。")
        }
        BreakthroughError::NotEnoughRegularMeridians { need, have } => {
            format!("突破未成：需先打通 {need} 条正经（当前 {have}）。")
        }
        BreakthroughError::NotEnoughExtraordinaryMeridians { need, have } => {
            format!("突破未成：需先打通 {need} 条奇经（当前 {have}）。")
        }
        BreakthroughError::NotEnoughQi { need, have } => {
            format!("突破未成：真元不足（需 {need:.1}，当前 {have:.1}）。")
        }
        BreakthroughError::ZoneTooWeak { need, have } => {
            format!("突破未成：此地灵气不足（需 {need:.2}，当前 {have:.2}）。")
        }
        BreakthroughError::EnvInsufficient {
            need,
            have,
            in_spirit_eye,
        } => {
            if *in_spirit_eye {
                format!("突破未成：灵眼扰动未稳（需 {need:.2}，当前 {have:.2}）。")
            } else {
                format!("突破未成：固元须在灵气浓处或灵眼内（需 {need:.2}，当前 {have:.2}）。")
            }
        }
        BreakthroughError::LedgerUnavailable => "突破未成：真元账本未就绪，仪式暂缓。".to_string(),
        BreakthroughError::RolledFailure { severity } => {
            format!("突破失败：气机反噬，伤势强度 {severity:.2}。")
        }
        BreakthroughError::RaceProfileIncomplete => {
            "突破未成：此身构型的经脉档案不完整，无法判定突破配额。".to_string()
        }
    }
}

/// 计算修正后的成功率 — plan §3.1 公式。
pub fn compute_success_rate(
    next: Realm,
    meridian_integrity_avg: f64,
    composure: f64,
    completeness: f64,
    material_bonus: f64,
) -> f64 {
    let base = base_success_rate(next);
    let bonus = material_bonus.clamp(0.0, 0.30);
    let raw = base * meridian_integrity_avg * composure * completeness * (1.0 + bonus);
    raw.clamp(0.0, 1.0)
}

pub fn compute_success_rate_with_env_bonus(
    next: Realm,
    meridian_integrity_avg: f64,
    composure: f64,
    completeness: f64,
    material_bonus: f64,
    env_bonus: f64,
) -> f64 {
    let material = material_bonus.clamp(0.0, 0.30);
    let env_bonus = env_bonus.clamp(0.0, 0.50);
    let raw = base_success_rate(next)
        * meridian_integrity_avg
        * composure
        * completeness
        * (1.0 + material)
        * (1.0 + env_bonus);
    raw.clamp(0.0, 1.0)
}

pub fn season_success_modifier(season: Season) -> f64 {
    match season {
        Season::Summer => 1.05,
        Season::Winter => 0.95,
        Season::SummerToWinter | Season::WinterToSummer => 0.85,
    }
}

pub fn compute_success_rate_with_env_and_season_bonus(
    next: Realm,
    meridian_integrity_avg: f64,
    composure: f64,
    completeness: f64,
    material_bonus: f64,
    env_bonus: f64,
    season: Season,
) -> f64 {
    let material = material_bonus.clamp(0.0, 0.30);
    let env_bonus = env_bonus.clamp(0.0, 0.50);
    let raw = base_success_rate(next)
        * meridian_integrity_avg
        * composure
        * completeness
        * (1.0 + material)
        * (1.0 + env_bonus)
        * season_success_modifier(season);
    raw.clamp(0.0, 1.0)
}

pub fn add_pending_material_bonus(cultivation: &mut Cultivation, magnitude: f64) -> f64 {
    let delta = magnitude.clamp(0.0, 0.30);
    cultivation.pending_material_bonus =
        (cultivation.pending_material_bonus + delta).clamp(0.0, 0.30);
    cultivation.pending_material_bonus
}

/// 按目标实体的 `MeridianProfile` 判定突破前置条件（配额 / 子配额 / qi 消耗）——
/// plan-race-system-v1 P1 对抗审查 M2/M3：非 humanoid 构型（P1 合成样本 / P5 whale
/// 等）走本函数即可拿到正确判定，不再假设 humanoid 曲线。
///
/// P5 换轨：production 消费点（`breakthrough_system` / `try_breakthrough_with_profile`）
/// 均已改走本函数——原先"调用方拿不到实体时"的零参 humanoid 保底包装
/// `breakthrough_precondition_error` 因此不再有调用点，已随本轮换轨移除；
/// `try_breakthrough_with_env_season_bonus`（既有测试/调用点用的 humanoid 便捷入口）
/// 显式传入 humanoid profile 调用本函数，行为 bit-for-bit 不变。
pub(crate) fn breakthrough_precondition_error_for_profile(
    cultivation: &Cultivation,
    meridians: &MeridianSystem,
    profile: &crate::body_plan::MeridianProfile,
) -> Option<BreakthroughError> {
    let next = match cultivation.realm {
        Realm::Awaken => Realm::Induce,
        Realm::Induce => Realm::Condense,
        Realm::Condense => Realm::Solidify,
        Realm::Solidify => Realm::Spirit,
        Realm::Spirit => return Some(BreakthroughError::RequiresTribulation),
        Realm::Void => return Some(BreakthroughError::AtMaxRealm),
    };
    let req = profile.realm_requirements[next.rank() as usize - 1];
    let need = req.total as usize;
    let have = meridians.opened_count();
    if have < need {
        return Some(BreakthroughError::NotEnoughMeridians { need, have });
    }

    let regular_have = meridians.regular_opened_count();
    let extraordinary_have = meridians.extraordinary_opened_count();
    let regular_need = req.regular_min as usize;
    let extraordinary_need = req.extraordinary_min as usize;
    if regular_have < regular_need {
        return Some(BreakthroughError::NotEnoughRegularMeridians {
            need: regular_need,
            have: regular_have,
        });
    }
    if extraordinary_have < extraordinary_need {
        return Some(BreakthroughError::NotEnoughExtraordinaryMeridians {
            need: extraordinary_need,
            have: extraordinary_have,
        });
    }

    let cost = breakthrough_qi_cost(next);
    if cultivation.qi_current < cost {
        return Some(BreakthroughError::NotEnoughQi {
            need: cost,
            have: cultivation.qi_current,
        });
    }
    None
}

fn breakthrough_environment_error(
    position: &Position,
    dimension: DimensionKind,
    zones: Option<&ZoneRegistry>,
    spirit_eyes: Option<&SpiritEyeRegistry>,
    from: Realm,
) -> Option<BreakthroughError> {
    let zone_qi = zones
        .and_then(|zones| zones.find_zone(dimension, position.get()))
        .map(|zone| zone.spirit_qi)
        .unwrap_or(0.0);
    let in_spirit_eye = spirit_eyes
        .and_then(|registry| registry.spirit_eye_qi_at(dimension, position.get()))
        .is_some();

    if next_realm(from) == Some(Realm::Solidify) {
        if zone_qi >= MIN_ZONE_QI_TO_GUYUAN || in_spirit_eye {
            None
        } else {
            Some(BreakthroughError::EnvInsufficient {
                need: MIN_ZONE_QI_TO_GUYUAN,
                have: zone_qi,
                in_spirit_eye,
            })
        }
    } else if zone_qi < MIN_ZONE_QI_TO_BREAKTHROUGH {
        Some(BreakthroughError::ZoneTooWeak {
            need: MIN_ZONE_QI_TO_BREAKTHROUGH,
            have: zone_qi,
        })
    } else {
        None
    }
}

/// 随机骰子抽象 — 测试时可注入确定值。
pub trait RollSource {
    fn roll_unit(&mut self) -> f64;
}

/// break review finding（major-1）：突破 roll 流跨 Update 持久化所需的每实体容器。
///
/// 历史上 `breakthrough_system` 每个 Update 都用固定种子重建 `XorshiftRoll`，导致
/// 一次双连发若被 socket 读批拆到两个 tick，两条请求各自消费 r1（=0.8597…）——
/// Solidify→Spirit 的成功率顶到全态夏季也只有 0.75075 < r1，拆批就永远过不去。
/// 本组件把 roll 流状态存到实体上，随每笔真实尝试推进；同 tick 与 1/tick 拆批的
/// 请求都消费到**连续**的 roll 值，任意拆批双连发都收敛（findings 的确定性控制见
/// 下方 `breakthrough_roll_state_*` 单测）。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct BreakthroughRollState(pub u64);

/// 突破 roll 流种子（历史上每 Update 重建 XorshiftRoll 用的同一常量，行为保持向后
/// 兼容：新玩家首笔请求仍消费 r1=0.8597…）。
pub const BREAKTHROUGH_ROLL_SEED: u64 = 0x9e3779b97f4a7c15;

/// 默认 roll：PRNG 的简单 xorshift（可重现，无需引 rand 依赖）。
pub struct XorshiftRoll(pub u64);
impl RollSource for XorshiftRoll {
    fn roll_unit(&mut self) -> f64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        ((x as f64) / (u64::MAX as f64)).clamp(0.0, 1.0)
    }
}

/// 纯函数：尝试突破。`roll` 可由调用方注入以方便测试（<= success_rate 则成功）。
pub fn try_breakthrough<R: RollSource>(
    cultivation: &mut Cultivation,
    meridians: &mut MeridianSystem,
    material_bonus: f64,
    roll: &mut R,
) -> Result<BreakthroughSuccess, BreakthroughError> {
    try_breakthrough_with_env_bonus(cultivation, meridians, material_bonus, 0.0, roll)
}

pub fn attempt_breakthrough_guyuan<R: RollSource>(
    cultivation: &mut Cultivation,
    meridians: &mut MeridianSystem,
    material_bonus: f64,
    spirit_eye_bonus: f64,
    roll: &mut R,
) -> Result<BreakthroughSuccess, BreakthroughError> {
    try_breakthrough_with_env_bonus(
        cultivation,
        meridians,
        material_bonus,
        spirit_eye_bonus,
        roll,
    )
}

pub fn try_breakthrough_with_env_bonus<R: RollSource>(
    cultivation: &mut Cultivation,
    meridians: &mut MeridianSystem,
    material_bonus: f64,
    env_bonus: f64,
    roll: &mut R,
) -> Result<BreakthroughSuccess, BreakthroughError> {
    try_breakthrough_with_env_season_bonus(
        cultivation,
        meridians,
        material_bonus,
        env_bonus,
        None,
        roll,
    )
}

pub fn try_breakthrough_with_env_season_bonus<R: RollSource>(
    cultivation: &mut Cultivation,
    meridians: &mut MeridianSystem,
    material_bonus: f64,
    env_bonus: f64,
    season: Option<Season>,
    roll: &mut R,
) -> Result<BreakthroughSuccess, BreakthroughError> {
    let profile = crate::body_plan::humanoid_plan_static()
        .meridian_profile
        .as_ref()
        .expect(
            "humanoid body plan must declare meridian_profile from plan-race-system-v1 P1 \
             onward — validate_body_plan should have rejected a humanoid plan missing it",
        );
    try_breakthrough_with_profile(
        cultivation,
        meridians,
        material_bonus,
        env_bonus,
        season,
        profile,
        roll,
    )
}

/// plan-race-system-v1 P5 —— 按**目标实体**解析出的 `body_plan::MeridianProfile` 尝试
/// 突破，供非 humanoid 战斗构型（whale 等易形/种族玩家）走通突破链路。调用方经
/// [`crate::body_plan::meridian_profile_for_target`] 解析出 `profile` 后传入——不再
/// 无条件绑死 humanoid 曲线。`try_breakthrough_with_env_season_bonus` 是本函数的
/// humanoid 保底包装（换轨前后 bit-for-bit 一致，见其函数体），既有调用点无需改动。
#[allow(clippy::too_many_arguments)]
pub fn try_breakthrough_with_profile<R: RollSource>(
    cultivation: &mut Cultivation,
    meridians: &mut MeridianSystem,
    material_bonus: f64,
    env_bonus: f64,
    season: Option<Season>,
    profile: &crate::body_plan::MeridianProfile,
    roll: &mut R,
) -> Result<BreakthroughSuccess, BreakthroughError> {
    let from = cultivation.realm;
    if let Some(error) =
        breakthrough_precondition_error_for_profile(cultivation, meridians, profile)
    {
        return Err(error);
    }
    let next = next_realm(from).expect("precondition check rejects max realm");
    let need = profile.realm_requirements[next.rank() as usize - 1].total as usize;
    let have = meridians.opened_count();
    let cost = breakthrough_qi_cost(next);

    let n = meridians.iter().count() as f64;
    let integrity_avg = if n > 0.0 {
        meridians.iter().map(|m| m.integrity).sum::<f64>() / n
    } else {
        1.0
    };
    // completeness：刚好达标 = 1.0，超额每多一条 +0.05（封顶 1.3）
    let completeness = 1.0 + 0.05 * (have as f64 - need as f64);
    let completeness = completeness.clamp(0.8, 1.3);

    let effective_material_bonus =
        (material_bonus + cultivation.pending_material_bonus).clamp(0.0, 0.30);

    let success_rate = match season {
        Some(season) => compute_success_rate_with_env_and_season_bonus(
            next,
            integrity_avg,
            cultivation.composure,
            completeness,
            effective_material_bonus,
            env_bonus,
            season,
        ),
        None => compute_success_rate_with_env_bonus(
            next,
            integrity_avg,
            cultivation.composure,
            completeness,
            effective_material_bonus,
            env_bonus,
        ),
    };

    // 扣费（不论成败）
    cultivation.qi_current -= cost;
    cultivation.pending_material_bonus = 0.0;

    let r = roll.roll_unit();
    if r <= success_rate {
        cultivation.realm = next;
        cultivation.qi_max *= qi_max_multiplier(next);
        cultivation.composure = (cultivation.composure - 0.1).max(0.0);
        Ok(BreakthroughSuccess {
            to: next,
            success_rate,
            used_qi: cost,
        })
    } else {
        // 失败：严重度由 success_rate 反推（越高越惨烈的翻车更罕见）
        let severity = (1.0 - success_rate).clamp(0.1, 0.9);
        // 给 integrity 最高 2 条经脉上裂痕
        let mut targets: Vec<_> = meridians.iter_mut().filter(|m| m.opened).collect();
        targets
            .sort_by_key(|meridian| std::cmp::Reverse(meridian.rate_tier + meridian.capacity_tier));
        for m in targets.into_iter().take(2) {
            m.cracks.push(MeridianCrack {
                severity,
                healing_progress: 0.0,
                cause: CrackCause::Backfire,
                created_at: 0,
            });
            m.integrity = (m.integrity - severity * 0.2).max(0.0);
        }
        // 突破失败：真元上限冻结。severity ∈ [0.1, 0.9]，每次加 severity * 10.0（即 1.0..9.0）。
        // 无 cap 时多次失败可致 qi_max_frozen ≥ qi_max → 有效上限归零 → 玩家永久废人。
        // 与 overload.rs 对齐：冻结量不超过 qi_max * BREAKTHROUGH_FAIL_FROZEN_CAP_RATIO (0.5)。
        let new_frozen = (cultivation.qi_max_frozen.unwrap_or(0.0) + severity * 10.0)
            .min(cultivation.qi_max * BREAKTHROUGH_FAIL_FROZEN_CAP_RATIO);
        cultivation.qi_max_frozen = Some(new_frozen);
        cultivation.composure = (cultivation.composure - 0.3).max(0.0);
        Err(BreakthroughError::RolledFailure { severity })
    }
}

fn breakthrough_season(
    position_context: Option<(&Position, DimensionKind)>,
    zones: Option<&ZoneRegistry>,
    tick: u64,
) -> Season {
    let zone_name = position_context
        .and_then(|(position, dimension)| {
            zones.and_then(|zones| {
                zones
                    .find_zone(dimension, position.get())
                    .map(|zone| zone.name.as_str())
            })
        })
        .unwrap_or("");
    query_season(zone_name, tick).season
}

fn spirit_eye_env_bonus_for(from: Realm, blood_valley: Option<bool>) -> f64 {
    if next_realm(from) != Some(Realm::Solidify) {
        return 0.0;
    }

    match blood_valley {
        Some(true) => BLOOD_VALLEY_BREAKTHROUGH_SUCCESS_BONUS,
        Some(false) => SPIRIT_EYE_BREAKTHROUGH_SUCCESS_BONUS,
        None => 0.0,
    }
}

pub(crate) fn breakthrough_actor_account_id(
    life_record: Option<&LifeRecord>,
    is_npc: bool,
) -> Result<QiAccountId, BreakthroughLedgerError> {
    let id = life_record
        .and_then(|life_record| {
            let id = life_record.character_id.trim();
            (!id.is_empty()).then(|| life_record.character_id.clone())
        })
        .ok_or(BreakthroughLedgerError::MissingStableActorId { is_npc })?;
    if is_npc {
        Ok(QiAccountId::npc(id))
    } else {
        Ok(QiAccountId::player(id))
    }
}

/// plan-zone-qi-economy-v1 P0 §8.1 决议 #1：突破消耗回充**独立待分配池**
/// （`qi_physics::ledger::credit_pending_inflow`），不再注水 audit-only 的
/// `zone:<name>` 账户（会被 `apply_dormant_regen_with_multiplier` 整体覆写、且从不
/// 写回 `zone.spirit_qi`——记账蒸发 bug 本身）。失败仍透传 `BreakthroughLedgerError`
/// （经 `From<QiPhysicsError>` 自动转换），调用方保留 `LedgerUnavailable` 回滚分支。
pub(crate) fn credit_active_breakthrough_cost(
    account: &mut WorldQiAccount,
    zone_name: &str,
    from: QiAccountId,
    amount: f64,
) -> Result<(), BreakthroughLedgerError> {
    crate::qi_physics::credit_pending_inflow(
        account,
        zone_name,
        from,
        amount,
        QiTransferReason::Breakthrough,
    )?;
    Ok(())
}

#[derive(SystemParam)]
pub(crate) struct BreakthroughResources<'w> {
    zones: Option<Res<'w, ZoneRegistry>>,
    spirit_eyes: Option<ResMut<'w, SpiritEyeRegistry>>,
    pending_narrations: Option<ResMut<'w, PendingGameplayNarrations>>,
    spirit_eye_used_events: Option<ResMut<'w, Events<SpiritEyeUsedForBreakthroughEvent>>>,
    skill_xp_events: Option<ResMut<'w, Events<SkillXpGain>>>,
    qi_account: Option<ResMut<'w, WorldQiAccount>>,
    /// plan-race-system-v1 P5 —— 突破配额换轨：按 `req.entity` 解析目标实体的
    /// `body_plan::MeridianProfile`（`crate::body_plan::meridian_profile_for_target`），
    /// 不再无条件绑死 humanoid 曲线。缺失时（大量既有单测未插入这两个资源）优雅退化
    /// 到 humanoid，行为 bit-for-bit 不变。
    body_plans: Option<Res<'w, crate::body_plan::BodyPlanRegistry>>,
    races: Option<Res<'w, crate::body_plan::RaceRegistry>>,
}

#[allow(clippy::too_many_arguments)] // Bevy system signature; one Query/EventWriter per concern.
#[allow(clippy::type_complexity)] // players Query carries 5 optional/owned token tuple elements
pub fn breakthrough_system(
    clock: Res<CultivationClock>,
    mut commands: Commands,
    mut requests: EventReader<BreakthroughRequest>,
    mut outcomes: EventWriter<BreakthroughOutcome>,
    mut deaths: EventWriter<CultivationDeathTrigger>,
    mut players: Query<(
        &mut Cultivation,
        &mut MeridianSystem,
        &mut LifeRecord,
        Option<&NpcMarker>,
        Option<&mut BreakthroughRollState>,
    )>,
    mut status_effects_q: Query<&mut StatusEffects>,
    positions: Query<&Position>,
    usernames: Query<&Username>,
    current_dimensions: Query<&CurrentDimension>,
    mut vfx_events: EventWriter<VfxEventRequest>,
    mut skill_cap_events: EventWriter<SkillCapChanged>,
    mut resources: BreakthroughResources,
) {
    // fix review finding major-1：roll 流不再每 Update 重建，而是按实体持久（组件
    // BreakthroughRollState），同 tick 与拆批到多个 Update 的请求都消费**连续**的
    // roll 值——Solidify→Spirit 双连发在 1/tick 拆批下也收敛（r1 失败后 next tick 的
    // r2 必胜）。roll_streams 是本 Update 内的实体级续接缓冲。
    let mut roll_streams: HashMap<Entity, u64> = HashMap::new();
    let now = clock.tick;
    for req in requests.read() {
        let Ok((mut cultivation, mut meridians, mut life, npc_marker, roll_state)) =
            players.get_mut(req.entity)
        else {
            // §15.2 可观察性：静默丢请求 = 玩家永远不知道为什么没反应。
            // 组件缺失属服务端接线问题，必须在 log 留痕。
            tracing::warn!(
                "[bong][cultivation] breakthrough request dropped: entity {:?} missing \
                 Cultivation/MeridianSystem/LifeRecord (attach chain broken?)",
                req.entity
            );
            continue;
        };
        let from = cultivation.realm;
        if let Some(target) = next_realm(from) {
            life.push(BiographyEntry::BreakthroughStarted {
                realm_target: target,
                tick: now,
            });
        }
        let character_id = life.character_id.clone();
        let username = usernames.get(req.entity).ok().map(|name| name.0.clone());

        // plan-race-system-v1 P5 换轨：突破配额（need）按目标实体解析出的 body plan
        // 派生，不再无条件绑死 humanoid 曲线——whale 等非人构型走此系统时用自己的
        // `MeridianProfile.realm_requirements`。`BeastKind` 不查（本系统查询要求携带
        // `Cultivation`/`MeridianSystem`/`LifeRecord`，携带这三者的 NPC 是"修士"身份，
        // 不是纯兽类 fauna，与既有 `resolve_meridian_topology_for_target` 消费点同款
        // 简化——见 `npc::brain::actions_life::cultivate_action_system`）。
        //
        // review r2 major-2 收口：resolve **成功**但 plan 缺 `meridian_profile` 时
        // `meridian_profile_for_target` 返回 `Err`——fail-closed 直接拒绝本次突破，
        // 不落入下面借 humanoid 曲线顶上的旧行为（resolve 本身失败/资源缺失仍在该函数
        // 内部退化到 humanoid，不受影响，见其文档）。
        let profile = match crate::body_plan::meridian_profile_for_target(
            req.entity,
            crate::body_plan::BodyPlanPurpose::Intrinsic,
            crate::body_plan::BodyPlanResolveInputs {
                cultivation: Some(&cultivation),
                beast_kind: None,
                morph_state: None,
            },
            resources.body_plans.as_deref(),
            resources.races.as_deref(),
        ) {
            Ok(profile) => profile,
            Err(error) => {
                tracing::warn!(
                    "[bong][cultivation] breakthrough rejected entity={:?} fail-closed: {error}",
                    req.entity
                );
                if let Some(narrations) = resources.pending_narrations.as_deref_mut() {
                    if let Some(username) = username.as_deref() {
                        narrations.push_player(
                            username,
                            breakthrough_error_message(&BreakthroughError::RaceProfileIncomplete),
                            NarrationStyle::SystemWarning,
                        );
                    }
                }
                outcomes.send(BreakthroughOutcome {
                    entity: req.entity,
                    from,
                    result: Err(BreakthroughError::RaceProfileIncomplete),
                });
                continue;
            }
        };

        // plan §3.1：material_bonus = req.material_bonus（手动传入，默认 0）
        //   ⊕ 服用突破辅助丹药挂在 StatusEffects 的 BreakthroughBoost buff 聚合值。
        //   最终 clamp 由 compute_success_rate 内部处理。
        let buff_bonus = status_effects_q
            .get(req.entity)
            .map(|se| sum_breakthrough_boost(se) as f64)
            .unwrap_or(0.0);
        let material_bonus = req.material_bonus + buff_bonus;

        let position_context = positions.get(req.entity).ok().map(|position| {
            let dimension = current_dimensions
                .get(req.entity)
                .map(|current| current.0)
                .unwrap_or_default();
            (position, dimension)
        });
        let zone_snapshot: Option<(String, f64)> =
            position_context.and_then(|(position, dimension)| {
                resources.zones.as_deref().and_then(|registry| {
                    registry
                        .find_zone(dimension, position.get())
                        .map(|zone| (zone.name.clone(), zone.spirit_qi))
                })
            });
        let spirit_eye_snapshot: Option<(SpiritEyeId, Option<String>, bool)> = position_context
            .and_then(|(position, dimension)| {
                resources
                    .spirit_eyes
                    .as_deref()
                    .and_then(|registry| registry.eye_at(dimension, position.get()))
                    .map(|eye| (eye.id.clone(), eye.zone_name.clone(), eye.blood_valley))
            });
        let env_bonus = spirit_eye_env_bonus_for(
            from,
            spirit_eye_snapshot
                .as_ref()
                .map(|(_, _, blood_valley)| *blood_valley),
        );
        let season = breakthrough_season(position_context, resources.zones.as_deref(), now);

        let zone_error = position_context.and_then(|(position, dimension)| {
            breakthrough_environment_error(
                position,
                dimension,
                resources.zones.as_deref(),
                resources.spirit_eyes.as_deref(),
                from,
            )
        });
        let ledger_error = if zone_error.is_none()
            && (resources.qi_account.is_none() || zone_snapshot.is_none())
            && breakthrough_precondition_error_for_profile(&cultivation, &meridians, profile)
                .is_none()
        {
            Some(BreakthroughError::LedgerUnavailable)
        } else {
            None
        };

        // 本 Update 内实体级续接：先查本 Update 已消费到的 roll 状态，否则读持久组件
        // （无组件则用固定种子，向后兼容：新玩家首笔请求仍消费 r1）。
        let entity_roll = roll_streams
            .get(&req.entity)
            .copied()
            .unwrap_or_else(|| roll_state.map_or(BREAKTHROUGH_ROLL_SEED, |state| state.0));
        let mut roll = XorshiftRoll(entity_roll);

        let res = zone_error
            .or_else(|| breakthrough_precondition_error_for_profile(&cultivation, &meridians, profile))
            .or(ledger_error)
            .map_or_else(
                || {
                    let actor_account = breakthrough_actor_account_id(
                        Some(&life),
                        npc_marker.is_some(),
                    )
                    .map_err(|error| {
                        tracing::warn!(
                            "[bong][cultivation] breakthrough ledger actor id unavailable entity={:?} error={:?}",
                            req.entity,
                            error
                        );
                        BreakthroughError::LedgerUnavailable
                    });
                    let Ok(actor_account) = actor_account else {
                        return Err(BreakthroughError::LedgerUnavailable);
                    };
                    let cultivation_before = cultivation.clone();
                    let meridians_before = meridians.clone();
                    let before_qi = cultivation.qi_current.max(0.0);
                    let result = try_breakthrough_with_profile(
                        &mut cultivation,
                        &mut meridians,
                        material_bonus,
                        env_bonus,
                        Some(season),
                        profile,
                        &mut roll,
                    );
                    let used_qi = (before_qi - cultivation.qi_current.max(0.0)).max(0.0);
                    if let (Some(account), Some((zone_name, _zone_qi))) =
                        (resources.qi_account.as_deref_mut(), zone_snapshot.as_ref())
                    {
                        if let Err(error) = credit_active_breakthrough_cost(
                            account,
                            zone_name.as_str(),
                            actor_account,
                            used_qi,
                        ) {
                            tracing::warn!(
                                "[bong][cultivation] breakthrough ledger credit failed entity={:?} zone={} amount={} error={:?}",
                                req.entity,
                                zone_name,
                                used_qi,
                                error
                            );
                            *cultivation = cultivation_before;
                            *meridians = meridians_before;
                            return Err(BreakthroughError::LedgerUnavailable);
                        }
                    }
                    result
                },
                Err,
            );

        // 消费点（try_breakthrough 内部）只在本 Update 真正突破尝试时推进 roll；因前置错误
        // 拒绝的请求不推进（roll 保持原值，写入同值无害）。持久化到组件跨 Update 续接。
        roll_streams.insert(req.entity, roll.0);

        match &res {
            Ok(success) => {
                life.push(BiographyEntry::BreakthroughSucceeded {
                    realm: success.to,
                    tick: now,
                });
                // plan-skill-v1 §4 境界软挂钩：突破到新境界 → 三个 MVP skill 的 cap 全部上调。
                // Client / agent 订阅 SkillCapChanged 做 narration / inspect 面板 effective_lv 展示。
                let new_cap = skill_cap_for_realm(success.to);
                for skill in SkillId::ALL {
                    skill_cap_events.send(SkillCapChanged {
                        char_entity: req.entity,
                        skill,
                        new_cap,
                    });
                }
                if let Some(skill_xp_events) = resources.skill_xp_events.as_deref_mut() {
                    skill_xp_events.send(SkillXpGain {
                        char_entity: req.entity,
                        skill: SkillId::Cultivation,
                        amount: 3,
                        source: XpGainSource::Action {
                            plan_id: "cultivation",
                            action: "breakthrough_success",
                        },
                    });
                }
                if from == Realm::Condense && success.to == Realm::Solidify {
                    if let Some((eye_id, zone_name, _blood_valley)) = spirit_eye_snapshot.as_ref() {
                        if let Some(payload) =
                            resources.spirit_eyes.as_deref_mut().and_then(|registry| {
                                registry.record_breakthrough_use_by_id(
                                    eye_id,
                                    character_id.as_str(),
                                    from,
                                    success.to,
                                    now,
                                )
                            })
                        {
                            life.push(BiographyEntry::SpiritEyeBreakthrough {
                                eye_id: payload.eye_id.clone(),
                                zone: zone_name.clone(),
                                tick: now,
                            });
                            if let Some(spirit_eye_used_events) =
                                resources.spirit_eye_used_events.as_deref_mut()
                            {
                                spirit_eye_used_events
                                    .send(SpiritEyeUsedForBreakthroughEvent { payload });
                            }
                            if let Some(narrations) = resources.pending_narrations.as_deref_mut() {
                                narrations.push_broadcast(
                                    "某处灵机结作一线，旋又归于沉寂。",
                                    NarrationStyle::Narration,
                                );
                            }
                        }
                    }
                }
                // plan-particle-system-v1 §4.4：突破成功发 breakthrough_pillar 光柱。
                if let Ok(pos) = positions.get(req.entity) {
                    let p = pos.get();
                    vfx_events.send(gameplay_vfx::spawn_request(
                        gameplay_vfx::BREAKTHROUGH_PILLAR,
                        p,
                        None,
                        "#FFE8A0",
                        1.0,
                        12,
                        60,
                    ));
                }
            }
            Err(BreakthroughError::RolledFailure { severity }) => {
                if let Some(target) = next_realm(from) {
                    life.push(BiographyEntry::BreakthroughFailed {
                        realm_target: target,
                        severity: *severity,
                        tick: now,
                    });
                }
                if let Ok(pos) = positions.get(req.entity) {
                    let p = pos.get();
                    vfx_events.send(gameplay_vfx::spawn_request(
                        gameplay_vfx::BREAKTHROUGH_FAIL,
                        p,
                        None,
                        "#FF3344",
                        (*severity as f32).clamp(0.35, 1.0),
                        (8.0 + *severity as f32 * 16.0).round() as u32,
                        60,
                    ));
                }
            }
            Err(_) => {}
        }

        if let Err(error) = &res {
            // 拒绝原因必须可观察（§15.2）：narration 是玩家面反馈，log 是排障面。
            // 此前 narration 资源/username 缺失时双双静默——留 log 兜底。
            tracing::info!(
                "[bong][cultivation] breakthrough rejected entity={:?} from={:?} error={:?}",
                req.entity,
                from,
                error
            );
            if let (Some(narrations), Some(username)) = (
                resources.pending_narrations.as_deref_mut(),
                username.as_deref(),
            ) {
                narrations.push_player(
                    username,
                    breakthrough_error_message(error),
                    NarrationStyle::SystemWarning,
                );
            } else {
                tracing::warn!(
                    "[bong][cultivation] breakthrough rejection feedback UNDELIVERABLE \
                     entity={:?} narrations_present={} username_present={}",
                    req.entity,
                    resources.pending_narrations.is_some(),
                    username.is_some()
                );
            }
        }

        if let Err(BreakthroughError::RolledFailure { severity }) = &res {
            if *severity >= 0.7 {
                // 严重失败 → 走火入魔
                deaths.send(CultivationDeathTrigger {
                    entity: req.entity,
                    cause: CultivationDeathCause::BreakthroughBackfire,
                    context: serde_json::json!({
                        "from": format!("{:?}", from),
                        "severity": severity,
                    }),
                });
            }
        }

        // 不论成败，一次性消费 BreakthroughBoost buff（plan §3.1：辅助丹药为突破"仪式"消耗）。
        // bughunt r4-P2#7：同步清除渡劫丹来源的 DamageReduction(u64::MAX)，防止永久减伤泄漏。
        if let Ok(mut se) = status_effects_q.get_mut(req.entity) {
            clear_breakthrough_boost(&mut se);
            clear_du_jie_dan_damage_reduction(&mut se);
        }

        outcomes.send(BreakthroughOutcome {
            entity: req.entity,
            from,
            result: res,
        });
    }

    // 把每个实体本 Update 消费后的 roll 流状态写回组件（deferred Commands 可见性：
    // 同 Update 内靠 roll_streams 续接，下一 Update 靠组件续接）。
    for (entity, roll_state) in roll_streams.drain() {
        commands
            .entity(entity)
            .insert(BreakthroughRollState(roll_state));
    }
}

#[allow(clippy::type_complexity)]
pub fn rapid_breakthrough_karma_mark_system(
    clock: Res<CultivationClock>,
    mut outcomes: EventReader<BreakthroughOutcome>,
    mut weights: Option<ResMut<KarmaWeightStore>>,
    players: Query<(
        &LifeRecord,
        Option<&Username>,
        &Position,
        Option<&CurrentDimension>,
    )>,
    zones: Option<Res<ZoneRegistry>>,
) {
    let Some(weights) = weights.as_deref_mut() else {
        return;
    };
    let now = clock.tick;

    for outcome in outcomes.read() {
        if outcome.result.is_err() {
            continue;
        }
        let Ok((life_record, username, position, current_dimension)) = players.get(outcome.entity)
        else {
            continue;
        };
        if !has_rapid_breakthrough_karma_trigger(life_record, now) {
            continue;
        }

        let dimension = current_dimension
            .map(|current| current.0)
            .unwrap_or(DimensionKind::Overworld);
        let position_vec = position.get();
        let zone_name = zones.as_deref().and_then(|registry| {
            registry
                .find_zone(dimension, position_vec)
                .map(|zone| zone.name.clone())
        });
        let player_id = username
            .map(|name| name.0.clone())
            .unwrap_or_else(|| life_record.character_id.clone());

        weights.mark_player(
            player_id,
            zone_name,
            block_pos_from_position(position),
            RAPID_BREAKTHROUGH_KARMA_WEIGHT_DELTA,
            now,
        );
    }
}

fn has_rapid_breakthrough_karma_trigger(life_record: &LifeRecord, now: u64) -> bool {
    life_record
        .biography
        .iter()
        .filter(|entry| matches_recent_breakthrough_success(entry, now))
        .take(2)
        .count()
        >= 2
}

fn matches_recent_breakthrough_success(entry: &BiographyEntry, now: u64) -> bool {
    matches!(
        entry,
        BiographyEntry::BreakthroughSucceeded { tick, .. }
            if *tick <= now && now - *tick <= RAPID_BREAKTHROUGH_KARMA_WINDOW_TICKS
    )
}

fn block_pos_from_position(position: &Position) -> BlockPos {
    let p = position.get();
    BlockPos::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::MeridianId;
    use crate::npc::spawn::NpcMarker;
    use crate::qi_physics::{QiAccountId, QiTransferReason, WorldQiAccount};
    use crate::schema::common::NarrationScope;
    use crate::schema::vfx_event::VfxEventPayloadV1;
    use crate::world::karma::KarmaWeightStore;
    use crate::world::zone::ZoneRegistry;
    use valence::prelude::{App, Events, Update, Username};

    struct FixedRoll(f64);
    impl RollSource for FixedRoll {
        fn roll_unit(&mut self) -> f64 {
            self.0
        }
    }

    #[test]
    fn qi_max_for_realm_matches_worldview_table_exactly() {
        // plan-npc-realm-distribution-v1 §8.1 #2 决议：qi_max_for_realm 的六个输出
        // 必须与 worldview §三:195-203 权威表逐一相等（10/40/150/540/2100/10700）。
        // 严禁与 combat_power.rs:61 test-only fixture（10/30/60/120/200/400）混淆
        // ——那是完全不同的一套非正典数值，本测试专门守住不能被悄悄换成那套。
        assert_eq!(qi_max_for_realm(Realm::Awaken), 10.0, "醒灵进入时 qi_max");
        assert_eq!(qi_max_for_realm(Realm::Induce), 40.0, "引气进入时 qi_max");
        assert_eq!(
            qi_max_for_realm(Realm::Condense),
            150.0,
            "凝脉进入时 qi_max"
        );
        assert_eq!(
            qi_max_for_realm(Realm::Solidify),
            540.0,
            "固元进入时 qi_max"
        );
        assert_eq!(qi_max_for_realm(Realm::Spirit), 2100.0, "通灵进入时 qi_max");
        assert_eq!(qi_max_for_realm(Realm::Void), 10700.0, "化虚进入时 qi_max");
    }

    #[test]
    fn qi_max_for_realm_strictly_increasing_across_all_realm_transitions() {
        // 状态转换饱和覆盖：六境界依 rank 严格递增，不允许任何一档打平或倒退。
        let ordered = [
            Realm::Awaken,
            Realm::Induce,
            Realm::Condense,
            Realm::Solidify,
            Realm::Spirit,
            Realm::Void,
        ];
        for pair in ordered.windows(2) {
            let (prev, next) = (pair[0], pair[1]);
            assert!(
                qi_max_for_realm(prev) < qi_max_for_realm(next),
                "{:?}({}) 必须严格小于 {:?}({})",
                prev,
                qi_max_for_realm(prev),
                next,
                qi_max_for_realm(next)
            );
        }
    }

    fn setup_for_induce() -> (Cultivation, MeridianSystem) {
        let mut c = Cultivation {
            qi_current: 100.0,
            qi_max: 100.0,
            composure: 1.0,
            realm: Realm::Awaken,
            ..Default::default()
        };
        c.realm = Realm::Awaken;
        let mut m = MeridianSystem::default();
        open_regular(&mut m, 3);
        (c, m)
    }

    fn open_regular(meridians: &mut MeridianSystem, count: usize) {
        for id in MeridianId::REGULAR.iter().take(count) {
            meridians.get_mut(*id).opened = true;
        }
    }

    fn open_extraordinary(meridians: &mut MeridianSystem, count: usize) {
        for id in MeridianId::EXTRAORDINARY.iter().take(count) {
            meridians.get_mut(*id).opened = true;
        }
    }

    fn open_all_meridians(meridians: &mut MeridianSystem) {
        for id in MeridianId::REGULAR
            .iter()
            .chain(MeridianId::EXTRAORDINARY.iter())
        {
            meridians.get_mut(*id).opened = true;
        }
    }

    #[test]
    fn breakthrough_actor_account_id_uses_stable_life_record_id() {
        let player_life = LifeRecord::new("player_a");
        let npc_life = LifeRecord::new("npc_a");

        assert_eq!(
            breakthrough_actor_account_id(Some(&player_life), false)
                .expect("player life record with character_id should produce an account id"),
            QiAccountId::player("player_a"),
            "player breakthrough ledger id must come from stable LifeRecord.character_id"
        );
        assert_eq!(
            breakthrough_actor_account_id(Some(&npc_life), true)
                .expect("npc life record with character_id should produce an account id"),
            QiAccountId::npc("npc_a"),
            "npc breakthrough ledger id must come from stable LifeRecord.character_id"
        );
    }

    #[test]
    fn breakthrough_actor_account_id_rejects_missing_or_blank_id() {
        let blank_life = LifeRecord::new("   ");

        assert!(
            matches!(
                breakthrough_actor_account_id(None, true),
                Err(BreakthroughLedgerError::MissingStableActorId { is_npc: true })
            ),
            "npc breakthrough ledger id must reject missing LifeRecord instead of falling back to unstable Entity ids"
        );
        assert!(
            matches!(
                breakthrough_actor_account_id(Some(&blank_life), false),
                Err(BreakthroughLedgerError::MissingStableActorId { is_npc: false })
            ),
            "player breakthrough ledger id must reject blank LifeRecord.character_id"
        );
    }

    #[test]
    fn credit_active_breakthrough_cost_handles_boundaries() {
        let mut ledger = WorldQiAccount::default();
        let from = QiAccountId::player("player_a");
        // plan-zone-qi-economy-v1 P0 §8.1 决议 #1：目标是独立待分配池，不是
        // zone:<name>（那个 key 会被 dormant regen 整体覆写，credit 进去等于蒸发）。
        let pending_pool = crate::qi_physics::pending_inflow_account();

        credit_active_breakthrough_cost(&mut ledger, "spawn", from.clone(), 0.0)
            .expect("zero breakthrough cost should be a no-op");
        assert_eq!(
            ledger.balance(&pending_pool),
            0.0,
            "zero breakthrough cost should not create pending pool balance"
        );
        assert_eq!(
            ledger.balance(&QiAccountId::zone("spawn")),
            0.0,
            "zero breakthrough cost must never touch the zone:<name> ledger account"
        );
        assert!(
            ledger.transfers().is_empty(),
            "zero breakthrough cost should not append a transfer audit"
        );

        let err = credit_active_breakthrough_cost(&mut ledger, "spawn", from.clone(), -1.0)
            .expect_err("negative breakthrough cost must be rejected");
        assert!(
            matches!(
                err,
                BreakthroughLedgerError::QiPhysics(QiPhysicsError::InvalidAmount {
                    field: "transfer.amount",
                    ..
                })
            ),
            "negative breakthrough cost should surface the QiPhysics invalid amount error; got {err:?}"
        );

        credit_active_breakthrough_cost(&mut ledger, "spawn", from.clone(), 8.0)
            .expect("positive breakthrough cost should credit the pending inflow pool");
        assert_eq!(
            ledger.balance(&pending_pool),
            8.0,
            "first positive breakthrough cost should create the pending pool account and \
             credit the spent qi"
        );
        assert_eq!(
            ledger.balance(&QiAccountId::zone("spawn")),
            0.0,
            "positive breakthrough cost must still never touch the zone:<name> ledger account \
             (dormant regen owns that key and overwrites it wholesale from zone.spirit_qi)"
        );
        let transfer = ledger
            .transfers()
            .last()
            .expect("positive breakthrough cost should append one transfer audit");
        assert_eq!(
            transfer.from, from,
            "breakthrough audit transfer must preserve the stable actor account as source"
        );
        assert_eq!(
            transfer.to, pending_pool,
            "breakthrough audit transfer must target the independent pending inflow pool"
        );
        assert_eq!(
            transfer.reason,
            QiTransferReason::Breakthrough,
            "breakthrough audit transfer must use the dedicated reason"
        );
        assert_eq!(
            transfer.amount, 8.0,
            "breakthrough audit transfer amount must equal the spent qi"
        );
    }

    #[test]
    fn awaken_to_induce_always_succeeds_with_roll_zero() {
        let (mut c, mut m) = setup_for_induce();
        let out = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(0.0)).unwrap();
        assert_eq!(out.to, Realm::Induce);
        assert_eq!(c.realm, Realm::Induce);
    }

    #[test]
    fn awaken_to_induce_fails_with_high_roll() {
        let (mut c, mut m) = setup_for_induce();
        // base 0.9 * integrity 1.0 * composure 1.0 * completeness 1.0 = 0.9 → roll 0.99 fails
        let err = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(0.99)).unwrap_err();
        assert!(matches!(err, BreakthroughError::RolledFailure { .. }));
        assert_eq!(c.realm, Realm::Awaken);
        // qi 已扣
        assert!(c.qi_current < 100.0);
    }

    #[test]
    fn breakthrough_season_modifier_matches_four_phases() {
        assert_eq!(season_success_modifier(Season::Summer), 1.05);
        assert_eq!(season_success_modifier(Season::Winter), 0.95);
        assert_eq!(season_success_modifier(Season::SummerToWinter), 0.85);
        assert_eq!(season_success_modifier(Season::WinterToSummer), 0.85);
    }

    #[test]
    fn breakthrough_in_xizhuan_phase_has_lower_success_rate() {
        let summer = compute_success_rate_with_env_and_season_bonus(
            Realm::Induce,
            1.0,
            1.0,
            1.0,
            0.0,
            0.0,
            Season::Summer,
        );
        let xizhuan = compute_success_rate_with_env_and_season_bonus(
            Realm::Induce,
            1.0,
            1.0,
            1.0,
            0.0,
            0.0,
            Season::SummerToWinter,
        );

        assert!(xizhuan < summer);
        assert!((xizhuan - 0.765).abs() < 1e-9);
    }

    #[test]
    fn spirit_to_void_is_gated_by_tribulation() {
        let mut c = Cultivation {
            realm: Realm::Spirit,
            qi_current: 1000.0,
            qi_max: 1000.0,
            ..Default::default()
        };
        let mut m = MeridianSystem::default();
        for id in MeridianId::REGULAR
            .iter()
            .chain(MeridianId::EXTRAORDINARY.iter())
        {
            m.get_mut(*id).opened = true;
        }
        let err = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(0.0)).unwrap_err();
        assert_eq!(err, BreakthroughError::RequiresTribulation);
    }

    #[test]
    fn breakthrough_error_message_covers_all_error_variants() {
        let cases = [
            (
                BreakthroughError::AtMaxRealm,
                "突破未成：你已抵达当前最高境界。",
            ),
            (
                BreakthroughError::RequiresTribulation,
                "突破未成：通灵至化虚必须先走渡虚劫。",
            ),
            (
                BreakthroughError::NotEnoughMeridians { need: 16, have: 15 },
                "突破未成：需先打通 16 条经脉（当前 15）。",
            ),
            (
                BreakthroughError::NotEnoughRegularMeridians { need: 12, have: 8 },
                "突破未成：需先打通 12 条正经（当前 8）。",
            ),
            (
                BreakthroughError::NotEnoughExtraordinaryMeridians { need: 4, have: 3 },
                "突破未成：需先打通 4 条奇经（当前 3）。",
            ),
            (
                BreakthroughError::NotEnoughQi {
                    need: 100.0,
                    have: 42.5,
                },
                "突破未成：真元不足（需 100.0，当前 42.5）。",
            ),
            (
                BreakthroughError::ZoneTooWeak {
                    need: 0.8,
                    have: 0.4,
                },
                "突破未成：此地灵气不足（需 0.80，当前 0.40）。",
            ),
            (
                BreakthroughError::EnvInsufficient {
                    need: 0.7,
                    have: 0.3,
                    in_spirit_eye: true,
                },
                "突破未成：灵眼扰动未稳（需 0.70，当前 0.30）。",
            ),
            (
                BreakthroughError::EnvInsufficient {
                    need: 0.7,
                    have: 0.3,
                    in_spirit_eye: false,
                },
                "突破未成：固元须在灵气浓处或灵眼内（需 0.70，当前 0.30）。",
            ),
            (
                BreakthroughError::LedgerUnavailable,
                "突破未成：真元账本未就绪，仪式暂缓。",
            ),
            (
                BreakthroughError::RolledFailure { severity: 0.75 },
                "突破失败：气机反噬，伤势强度 0.75。",
            ),
        ];

        for (error, expected) in cases {
            assert_eq!(
                breakthrough_error_message(&error),
                expected,
                "expected stable breakthrough error text for {error:?}"
            );
        }
    }

    #[test]
    fn breakthrough_system_pushes_system_warning_on_precondition_error() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 10 });
        app.insert_resource(PendingGameplayNarrations::default());
        app.add_event::<BreakthroughRequest>();
        app.add_event::<BreakthroughOutcome>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<SkillCapChanged>();
        app.add_systems(Update, breakthrough_system);
        let player = app
            .world_mut()
            .spawn((
                Cultivation::default(),
                MeridianSystem::default(),
                LifeRecord::default(),
                Username("Azure".to_string()),
            ))
            .id();

        app.world_mut().send_event(BreakthroughRequest {
            entity: player,
            material_bonus: 0.0,
        });
        app.update();

        let narrations = app
            .world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain();
        assert_eq!(narrations.len(), 1);
        assert_eq!(narrations[0].scope, NarrationScope::Player);
        assert_eq!(narrations[0].target.as_deref(), Some("Azure"));
        assert_eq!(narrations[0].style, NarrationStyle::SystemWarning);
        assert!(
            narrations[0].text.contains("突破未成"),
            "expected system warning to include breakthrough failure reason, actual text={}",
            narrations[0].text
        );
    }

    /// plan-race-system-v1 P6b review major-5 收口：production 级集成测试——真实
    /// `Entity` + `Cultivation.race` + `BodyPlanRegistry`/`RaceRegistry` 资源驱动
    /// `breakthrough_system`（不是直接调用 `try_breakthrough_with_profile`，那只锁得住
    /// "传对了 profile 就能工作"，锁不住 `breakthrough_system` 内部 `meridian_profile_for_target`
    /// 接线是否真的把 `req.entity` 解析到了自己的 race 而不是 humanoid）。
    ///
    /// 合成非人构型 `test_breakthrough_synthetic_race` 的 Induce 门槛只需 1 条 channel
    /// （humanoid 需要 3 条），只开 1 条 channel：若系统内部悄悄用了 humanoid 曲线，
    /// 会因 `NotEnoughMeridians{need:3, have:1}` 拒绝；若真按自身构型解析，应该成功。
    #[test]
    fn breakthrough_system_uses_target_entity_own_race_profile_not_humanoid() {
        use crate::body_plan::race_registry::RaceEntry;
        use crate::body_plan::types::{
            BodyPartDef, ChannelDef, HeightBand, HeightBandAssignment, HitGeometry, MeridianFamily,
            MeridianProfile, PartConsequence, RealmMeridianReq, StandingAabbSpec,
        };
        use crate::body_plan::{BodyPlanId, BodyPlanRegistry, RaceId, RaceRegistry, HUMAN_RACE_ID};

        fn synthetic_race_plan() -> crate::body_plan::BodyPlan {
            crate::body_plan::BodyPlan {
                id: BodyPlanId::new("test_breakthrough_synthetic_race_plan"),
                display_name: "测试突破合成构型".to_string(),
                is_humanoid: false,
                parts: vec![BodyPartDef {
                    id: "body".into(),
                    damage_mul: 1.0,
                    contam_mul: 1.0,
                    bleed_mul: 1.0,
                    consequence: PartConsequence::Core,
                }],
                hit_geometry: HitGeometry::HeightBands {
                    aabb: StandingAabbSpec {
                        half_width: 2.0,
                        height: 3.0,
                    },
                    bands: vec![HeightBand {
                        min_rel_y: -1.0,
                        assignment: HeightBandAssignment::Single {
                            part: "body".into(),
                        },
                    }],
                    lateral_threshold: 0.5,
                },
                equip_slots: vec![],
                meridian_profile: Some(MeridianProfile {
                    channels: vec![
                        ChannelDef {
                            id: "chan_a".into(),
                            family: MeridianFamily::Regular,
                            body_part: None,
                            roles: vec![],
                        },
                        ChannelDef {
                            id: "chan_b".into(),
                            family: MeridianFamily::Regular,
                            body_part: None,
                            roles: vec![],
                        },
                        ChannelDef {
                            id: "chan_c".into(),
                            family: MeridianFamily::Regular,
                            body_part: None,
                            roles: vec![],
                        },
                    ],
                    topology_edges: vec![],
                    // Induce（index 1）只需 1 条——humanoid 同一档需要 3 条
                    // （`humanoid.json realm_requirements[1] = {total:3, regular_min:3}`）。
                    realm_requirements: [
                        RealmMeridianReq {
                            total: 0,
                            regular_min: 0,
                            extraordinary_min: 0,
                        },
                        RealmMeridianReq {
                            total: 1,
                            regular_min: 1,
                            extraordinary_min: 0,
                        },
                        RealmMeridianReq {
                            total: 2,
                            regular_min: 2,
                            extraordinary_min: 0,
                        },
                        RealmMeridianReq {
                            total: 3,
                            regular_min: 3,
                            extraordinary_min: 0,
                        },
                        RealmMeridianReq {
                            total: 3,
                            regular_min: 3,
                            extraordinary_min: 0,
                        },
                        RealmMeridianReq {
                            total: 3,
                            regular_min: 3,
                            extraordinary_min: 0,
                        },
                    ],
                    dugu_injection: vec![],
                }),
                mutation_slot_mapping: Default::default(),
            }
        }

        fn human_placeholder_plan() -> crate::body_plan::BodyPlan {
            crate::body_plan::BodyPlan {
                id: BodyPlanId::new("test_breakthrough_human_placeholder_plan"),
                display_name: "测试人族占位构型".to_string(),
                is_humanoid: false,
                parts: vec![BodyPartDef {
                    id: "body".into(),
                    damage_mul: 1.0,
                    contam_mul: 1.0,
                    bleed_mul: 1.0,
                    consequence: PartConsequence::Core,
                }],
                hit_geometry: HitGeometry::HeightBands {
                    aabb: StandingAabbSpec {
                        half_width: 2.0,
                        height: 3.0,
                    },
                    bands: vec![HeightBand {
                        min_rel_y: -1.0,
                        assignment: HeightBandAssignment::Single {
                            part: "body".into(),
                        },
                    }],
                    lateral_threshold: 0.5,
                },
                equip_slots: vec![],
                meridian_profile: None,
                mutation_slot_mapping: Default::default(),
            }
        }

        let body_plans =
            BodyPlanRegistry::from_plans(vec![synthetic_race_plan(), human_placeholder_plan()])
                .expect("synthetic race + human placeholder plans must validate");
        let races = RaceRegistry::from_parts_for_test(
            vec![
                RaceEntry {
                    id: RaceId::new(HUMAN_RACE_ID),
                    display_name: "人族".to_string(),
                    body_plan_id: BodyPlanId::new("test_breakthrough_human_placeholder_plan"),
                    beast_kinds: vec![],
                },
                RaceEntry {
                    id: RaceId::new("test_breakthrough_synthetic_race"),
                    display_name: "测试突破合成种族".to_string(),
                    body_plan_id: BodyPlanId::new("test_breakthrough_synthetic_race_plan"),
                    beast_kinds: vec![],
                },
            ],
            vec![],
            &body_plans,
        )
        .expect("synthetic race registry fixture must validate");

        let mut app = App::new();
        let mut zones = ZoneRegistry::fallback();
        zones.find_zone_mut("spawn").unwrap().spirit_qi = 0.9;
        app.insert_resource(CultivationClock { tick: 10 });
        app.insert_resource(zones);
        app.insert_resource(WorldQiAccount::default());
        app.insert_resource(body_plans);
        app.insert_resource(races);
        app.add_event::<BreakthroughRequest>();
        app.add_event::<BreakthroughOutcome>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<SkillCapChanged>();
        app.add_event::<SkillXpGain>();
        app.add_event::<SpiritEyeUsedForBreakthroughEvent>();
        app.add_systems(Update, breakthrough_system);

        let mut meridians =
            MeridianSystem::for_profile(synthetic_race_plan().meridian_profile.as_ref().unwrap());
        // 只打通 1 条 channel——humanoid 曲线（need=3）会拒绝，合成构型自己的曲线
        // （need=1）应该放行。
        meridians.regular[0].opened = true;
        let cultivation = Cultivation {
            realm: Realm::Awaken,
            qi_current: 100.0,
            qi_max: 100.0,
            composure: 1.0,
            race: RaceId::new("test_breakthrough_synthetic_race"),
            ..Default::default()
        };
        let entity = app
            .world_mut()
            .spawn((
                cultivation,
                meridians,
                LifeRecord::new("synthetic_race_char"),
                Position::new([8.0, 66.0, 8.0]),
            ))
            .id();

        app.world_mut().send_event(BreakthroughRequest {
            entity,
            material_bonus: 0.0,
        });
        app.update();

        // Roll-independent assertion: `try_breakthrough_with_profile` debits qi cost
        // "不论成败"(win or lose the roll) the moment the precondition check passes —
        // so a qi debit here is decisive proof that `have=1 >= need` was evaluated
        // against the *synthetic race's own* curve (need=1), not the humanoid curve
        // (need=3, which this 1-channel-opened entity would fail and leave qi
        // untouched). Asserting the exact realm outcome would flakily depend on the
        // system's internal fixed-seed roll instead of the profile wiring itself.
        let cultivation = app.world().get::<Cultivation>(entity).unwrap();
        assert_eq!(
            cultivation.qi_current, 92.0,
            "breakthrough_system must resolve the target entity's own race profile (need=1) \
             through meridian_profile_for_target and attempt the breakthrough (debiting the 8.0 \
             qi cost), not silently fall back to the humanoid curve (need=3) which would reject \
             this 1-channel-opened entity outright and leave qi_current untouched at 100.0 — \
             actual qi_current after the attempt: {}",
            cultivation.qi_current
        );
    }

    #[test]
    fn material_bonus_capped_at_30_percent() {
        let r = compute_success_rate(Realm::Induce, 1.0, 1.0, 1.0, 5.0);
        let r_cap = compute_success_rate(Realm::Induce, 1.0, 1.0, 1.0, 0.30);
        assert!((r - r_cap).abs() < 1e-9);
    }

    #[test]
    fn pending_material_bonus_accumulates_and_caps_at_30_percent() {
        let mut c = Cultivation::default();
        assert!((add_pending_material_bonus(&mut c, 0.12) - 0.12).abs() < 1e-9);
        assert!((add_pending_material_bonus(&mut c, 0.50) - 0.30).abs() < 1e-9);
        assert!((c.pending_material_bonus - 0.30).abs() < 1e-9);
    }

    #[test]
    fn completeness_bounded() {
        // 超额很多不会无限放大
        let r = compute_success_rate(Realm::Induce, 1.0, 1.0, 1.3, 0.0);
        assert!(r <= 1.0);
    }

    #[test]
    fn void_breakthrough_returns_max_realm_error() {
        let mut c = Cultivation {
            realm: Realm::Void,
            ..Default::default()
        };
        let mut m = MeridianSystem::default();
        let err = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(0.0)).unwrap_err();
        assert_eq!(err, BreakthroughError::AtMaxRealm);
    }

    #[test]
    fn pending_material_bonus_is_consumed_on_real_attempt() {
        let (mut c, mut m) = setup_for_induce();
        c.pending_material_bonus = 0.12;

        let out = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(0.0)).unwrap();

        let expected = compute_success_rate(Realm::Induce, 1.0, 1.0, 1.0, 0.12);
        assert!((out.success_rate - expected).abs() < 1e-9);
        assert_eq!(c.pending_material_bonus, 0.0);
    }

    #[test]
    fn pending_material_bonus_is_preserved_when_preconditions_fail() {
        let mut c = Cultivation {
            qi_current: 1.0,
            pending_material_bonus: 0.12,
            ..Default::default()
        };
        let mut m = MeridianSystem::default();
        open_regular(&mut m, 3);

        let err = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(0.0)).unwrap_err();

        assert!(matches!(err, BreakthroughError::NotEnoughQi { .. }));
        assert!((c.pending_material_bonus - 0.12).abs() < 1e-9);
    }

    #[test]
    fn induce_requires_three_regular_meridians_not_extraordinary_padding() {
        let mut c = Cultivation {
            realm: Realm::Awaken,
            qi_current: 100.0,
            qi_max: 100.0,
            composure: 1.0,
            ..Default::default()
        };
        let mut m = MeridianSystem::default();
        open_extraordinary(&mut m, 3);

        let err = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(0.0)).unwrap_err();

        assert_eq!(
            err,
            BreakthroughError::NotEnoughRegularMeridians { need: 3, have: 0 }
        );
        assert_eq!(c.realm, Realm::Awaken);
        assert_eq!(c.qi_current, 100.0);
    }

    #[test]
    fn solidify_requires_all_twelve_regular_meridians() {
        let mut c = Cultivation {
            realm: Realm::Condense,
            qi_current: 500.0,
            qi_max: 500.0,
            composure: 1.0,
            ..Default::default()
        };
        let mut m = MeridianSystem::default();
        open_regular(&mut m, 10);
        open_extraordinary(&mut m, 6);

        let err = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(0.0)).unwrap_err();

        assert_eq!(
            err,
            BreakthroughError::NotEnoughRegularMeridians { need: 12, have: 10 }
        );
        assert_eq!(c.realm, Realm::Condense);
        assert_eq!(c.qi_current, 500.0);
    }

    #[test]
    fn spirit_rejects_before_structure_when_total_meridians_are_too_few() {
        let mut c = Cultivation {
            realm: Realm::Solidify,
            qi_current: 1000.0,
            qi_max: 1000.0,
            composure: 1.0,
            ..Default::default()
        };
        let mut m = MeridianSystem::default();
        open_regular(&mut m, 12);
        open_extraordinary(&mut m, 3);

        let err = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(0.0)).unwrap_err();

        assert_eq!(
            err,
            BreakthroughError::NotEnoughMeridians { need: 16, have: 15 }
        );
        assert_eq!(c.realm, Realm::Solidify);
        assert_eq!(c.qi_current, 1000.0);
    }

    #[test]
    fn spirit_rejects_extraordinary_padding_without_regular_foundation() {
        let mut c = Cultivation {
            realm: Realm::Solidify,
            qi_current: 1000.0,
            qi_max: 1000.0,
            composure: 1.0,
            ..Default::default()
        };
        let mut m = MeridianSystem::default();
        open_regular(&mut m, 8);
        open_extraordinary(&mut m, 8);

        let err = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(0.0)).unwrap_err();

        assert_eq!(
            err,
            BreakthroughError::NotEnoughRegularMeridians { need: 12, have: 8 }
        );
        assert_eq!(c.realm, Realm::Solidify);
        assert_eq!(c.qi_current, 1000.0);
    }

    #[test]
    fn spirit_allows_twelve_regular_and_four_extraordinary_meridians() {
        let mut c = Cultivation {
            realm: Realm::Solidify,
            qi_current: 1000.0,
            qi_max: 1000.0,
            composure: 1.0,
            ..Default::default()
        };
        let mut m = MeridianSystem::default();
        open_regular(&mut m, 12);
        open_extraordinary(&mut m, 4);

        let out = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(0.0)).unwrap();

        assert_eq!(out.to, Realm::Spirit);
        assert_eq!(c.realm, Realm::Spirit);
    }

    #[test]
    fn breakthrough_rejects_when_zone_qi_too_weak() {
        let mut app = App::new();
        let mut zones = ZoneRegistry::fallback();
        zones.find_zone_mut("spawn").unwrap().spirit_qi = 0.0;
        app.insert_resource(CultivationClock { tick: 10 });
        app.insert_resource(zones);
        app.add_event::<BreakthroughRequest>();
        app.add_event::<BreakthroughOutcome>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<SkillCapChanged>();
        app.add_event::<SkillXpGain>();
        app.add_event::<SpiritEyeUsedForBreakthroughEvent>();
        app.add_systems(Update, breakthrough_system);

        let (mut cultivation, meridians) = setup_for_induce();
        cultivation.pending_material_bonus = 0.12;
        let player = app
            .world_mut()
            .spawn((
                cultivation,
                meridians,
                LifeRecord::default(),
                Position::new([8.0, 66.0, 8.0]),
            ))
            .id();

        app.world_mut().send_event(BreakthroughRequest {
            entity: player,
            material_bonus: 0.0,
        });
        app.update();

        let outcomes = app.world().resource::<Events<BreakthroughOutcome>>();
        let outcome = outcomes.iter_current_update_events().next().unwrap();
        assert!(matches!(
            outcome.result,
            Err(BreakthroughError::ZoneTooWeak { .. })
        ));
        let cultivation = app.world().get::<Cultivation>(player).unwrap();
        assert_eq!(cultivation.qi_current, 100.0);
        assert!((cultivation.pending_material_bonus - 0.12).abs() < 1e-9);
    }

    #[test]
    fn breakthrough_system_without_ledger_does_not_consume_qi() {
        let mut app = App::new();
        let mut zones = ZoneRegistry::fallback();
        zones.find_zone_mut("spawn").unwrap().spirit_qi = 0.9;
        app.insert_resource(CultivationClock { tick: 10 });
        app.insert_resource(zones);
        app.add_event::<BreakthroughRequest>();
        app.add_event::<BreakthroughOutcome>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<SkillCapChanged>();
        app.add_event::<SkillXpGain>();
        app.add_event::<SpiritEyeUsedForBreakthroughEvent>();
        app.add_systems(Update, breakthrough_system);

        let (cultivation, meridians) = setup_for_induce();
        let player = app
            .world_mut()
            .spawn((
                cultivation,
                meridians,
                LifeRecord::new("player_a"),
                Position::new([8.0, 66.0, 8.0]),
            ))
            .id();

        app.world_mut().send_event(BreakthroughRequest {
            entity: player,
            material_bonus: 0.0,
        });
        app.update();

        let outcomes = app.world().resource::<Events<BreakthroughOutcome>>();
        let outcome = outcomes.iter_current_update_events().next().unwrap();
        assert!(matches!(
            outcome.result,
            Err(BreakthroughError::LedgerUnavailable)
        ));
        let cultivation = app.world().get::<Cultivation>(player).unwrap();
        assert_eq!(cultivation.realm, Realm::Awaken);
        assert_eq!(cultivation.qi_current, 100.0);
    }

    #[test]
    fn breakthrough_success_credits_cost_to_pending_inflow_pool_not_zone_ledger() {
        let mut app = App::new();
        let mut zones = ZoneRegistry::fallback();
        zones.find_zone_mut("spawn").unwrap().spirit_qi = 0.9;
        app.insert_resource(CultivationClock { tick: 10 });
        app.insert_resource(zones);
        app.insert_resource(WorldQiAccount::default());
        app.add_event::<BreakthroughRequest>();
        app.add_event::<BreakthroughOutcome>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<SkillCapChanged>();
        app.add_event::<SkillXpGain>();
        app.add_event::<SpiritEyeUsedForBreakthroughEvent>();
        app.add_systems(Update, breakthrough_system);

        let (cultivation, meridians) = setup_for_induce();
        let player = app
            .world_mut()
            .spawn((
                cultivation,
                meridians,
                LifeRecord::new("player_a"),
                Position::new([8.0, 66.0, 8.0]),
            ))
            .id();

        app.world_mut().send_event(BreakthroughRequest {
            entity: player,
            material_bonus: 0.0,
        });
        app.update();

        let cultivation = app.world().get::<Cultivation>(player).unwrap();
        assert_eq!(
            cultivation.realm,
            Realm::Induce,
            "successful breakthrough should advance the player realm"
        );
        assert_eq!(
            cultivation.qi_current, 92.0,
            "successful breakthrough should spend exactly 8 qi from the player"
        );
        let ledger = app.world().resource::<WorldQiAccount>();
        let pending_pool = crate::qi_physics::pending_inflow_account();
        assert_eq!(
            ledger.balance(&pending_pool),
            8.0,
            "successful breakthrough should credit the spent 8 qi to the independent pending \
             inflow pool"
        );
        assert_eq!(
            ledger.balance(&QiAccountId::zone("spawn")),
            0.0,
            "successful breakthrough must never touch the zone:<name> ledger account (that key \
             is owned/overwritten wholesale by dormant regen from zone.spirit_qi)"
        );
        let transfer = ledger
            .transfers()
            .last()
            .expect("breakthrough should leave a QiTransfer audit");
        assert_eq!(
            transfer.reason,
            QiTransferReason::Breakthrough,
            "successful breakthrough audit should use the dedicated reason"
        );
        assert_eq!(
            transfer.from,
            QiAccountId::player("player_a"),
            "successful breakthrough audit should use stable player id as source"
        );
        assert_eq!(
            transfer.to, pending_pool,
            "successful breakthrough audit should target the independent pending inflow pool"
        );
        assert_eq!(
            transfer.amount, 8.0,
            "successful breakthrough audit amount should match spent qi"
        );
    }

    #[test]
    fn breakthrough_and_meridian_open_preserve_total_observed_qi_conservation() {
        // plan-zone-qi-economy-v1 P0 §10.3 — 开脉→突破全链路总量不变的端到端守恒对拍。
        // total_observed() = player_qi + zone_qi + container_qi + ledger_qi（含待分配池）。
        // 消耗 → 待分配池等额升，player_qi 等额降，total_observed() 必须严格不变
        // （无天道时代衰减，era_decay=0）。
        use crate::qi_physics::{assert_conservation, summarize_world_qi};

        let mut app = App::new();
        let mut zones = ZoneRegistry::fallback();
        zones.find_zone_mut("spawn").unwrap().spirit_qi = 0.9;
        app.insert_resource(CultivationClock { tick: 10 });
        app.insert_resource(zones);
        app.insert_resource(WorldQiAccount::default());
        app.add_event::<BreakthroughRequest>();
        app.add_event::<BreakthroughOutcome>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<SkillCapChanged>();
        app.add_event::<SkillXpGain>();
        app.add_event::<SpiritEyeUsedForBreakthroughEvent>();
        app.add_systems(Update, breakthrough_system);

        let (cultivation, meridians) = setup_for_induce();
        let player = app
            .world_mut()
            .spawn((
                cultivation,
                meridians,
                LifeRecord::new("player_a"),
                Position::new([8.0, 66.0, 8.0]),
            ))
            .id();

        let before = summarize_world_qi(app.world_mut());

        app.world_mut().send_event(BreakthroughRequest {
            entity: player,
            material_bonus: 0.0,
        });
        app.update();

        let cultivation = app.world().get::<Cultivation>(player).unwrap();
        assert_eq!(
            cultivation.realm,
            Realm::Induce,
            "sanity: breakthrough must actually succeed for this conservation test to be \
             meaningful (spent qi must leave the player)"
        );

        let after = summarize_world_qi(app.world_mut());
        assert_conservation(&before, &after, 0.0).unwrap_or_else(|error| {
            panic!(
                "breakthrough must conserve total_observed qi (player_qi + zone_qi + \
                 container_qi + ledger_qi) with zero era decay — got drift: {error} \
                 (before={before:?}, after={after:?}); a mismatch here means spent qi is \
                 vanishing (not reaching the pending inflow pool) or being double-counted"
            )
        });
        assert!(
            (before.total_observed() - after.total_observed()).abs() < 1e-9,
            "explicit total_observed equality check (belt-and-suspenders alongside \
             assert_conservation): before={}, after={}",
            before.total_observed(),
            after.total_observed()
        );
    }

    #[test]
    fn breakthrough_failure_also_credits_cost_to_pending_inflow_pool_not_zone_ledger() {
        let mut app = App::new();
        let mut zones = ZoneRegistry::fallback();
        zones.find_zone_mut("spawn").unwrap().spirit_qi = 0.9;
        app.insert_resource(CultivationClock { tick: 10 });
        app.insert_resource(zones);
        app.insert_resource(WorldQiAccount::default());
        app.add_event::<BreakthroughRequest>();
        app.add_event::<BreakthroughOutcome>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<SkillCapChanged>();
        app.add_event::<SkillXpGain>();
        app.add_event::<SpiritEyeUsedForBreakthroughEvent>();
        app.add_systems(Update, breakthrough_system);

        let (mut cultivation, meridians) = setup_for_induce();
        cultivation.composure = 0.0;
        let player = app
            .world_mut()
            .spawn((
                cultivation,
                meridians,
                LifeRecord::new("player_a"),
                Position::new([8.0, 66.0, 8.0]),
            ))
            .id();

        app.world_mut().send_event(BreakthroughRequest {
            entity: player,
            material_bonus: 0.0,
        });
        app.update();

        let cultivation = app.world().get::<Cultivation>(player).unwrap();
        assert_eq!(
            cultivation.realm,
            Realm::Awaken,
            "failed breakthrough should keep the player in the original realm"
        );
        assert_eq!(
            cultivation.qi_current, 92.0,
            "failed breakthrough should still spend exactly 8 qi"
        );
        let ledger = app.world().resource::<WorldQiAccount>();
        let pending_pool = crate::qi_physics::pending_inflow_account();
        assert_eq!(
            ledger.balance(&pending_pool),
            8.0,
            "failed breakthrough should still credit spent qi to the independent pending \
             inflow pool"
        );
        assert_eq!(
            ledger.balance(&QiAccountId::zone("spawn")),
            0.0,
            "failed breakthrough must never touch the zone:<name> ledger account either"
        );
        let transfer = ledger
            .transfers()
            .last()
            .expect("failed breakthrough should leave a QiTransfer audit");
        assert_eq!(
            transfer.reason,
            QiTransferReason::Breakthrough,
            "failed breakthrough audit should use the dedicated reason"
        );
        assert_eq!(
            transfer.to, pending_pool,
            "failed breakthrough audit should target the independent pending inflow pool"
        );
        assert_eq!(
            transfer.amount, 8.0,
            "failed breakthrough audit amount should match spent qi"
        );
    }

    #[test]
    fn npc_breakthrough_emits_vfx() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 10 });
        app.insert_resource(ZoneRegistry::fallback());
        app.insert_resource(WorldQiAccount::default());
        app.add_event::<BreakthroughRequest>();
        app.add_event::<BreakthroughOutcome>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<SkillCapChanged>();
        app.add_event::<SkillXpGain>();
        app.add_event::<SpiritEyeUsedForBreakthroughEvent>();
        app.add_systems(Update, breakthrough_system);

        let (cultivation, meridians) = setup_for_induce();
        let npc = app
            .world_mut()
            .spawn((
                cultivation,
                meridians,
                LifeRecord::new("npc_42v0"),
                Position::new([8.0, 66.0, 8.0]),
                NpcMarker,
            ))
            .id();

        app.world_mut().send_event(BreakthroughRequest {
            entity: npc,
            material_bonus: 0.0,
        });
        app.update();

        let vfx_events = app.world().resource::<Events<VfxEventRequest>>();
        let ids = vfx_events
            .iter_current_update_events()
            .filter_map(|event| match &event.payload {
                VfxEventPayloadV1::SpawnParticle { event_id, .. } => Some(event_id.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert!(ids.contains(&"bong:breakthrough_pillar"));
    }

    #[test]
    fn breakthrough_fail_emits_vfx() {
        let mut app = App::new();
        let mut zones = ZoneRegistry::fallback();
        zones.find_zone_mut("spawn").unwrap().spirit_qi = 0.9;
        app.insert_resource(CultivationClock { tick: 10 });
        app.insert_resource(zones);
        app.insert_resource(WorldQiAccount::default());
        app.add_event::<BreakthroughRequest>();
        app.add_event::<BreakthroughOutcome>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<SkillCapChanged>();
        app.add_event::<SkillXpGain>();
        app.add_event::<SpiritEyeUsedForBreakthroughEvent>();
        app.add_systems(Update, breakthrough_system);

        let (mut cultivation, meridians) = setup_for_induce();
        cultivation.composure = 0.0;
        let player = app
            .world_mut()
            .spawn((
                cultivation,
                meridians,
                LifeRecord::default(),
                Position::new([8.0, 66.0, 8.0]),
            ))
            .id();

        app.world_mut().send_event(BreakthroughRequest {
            entity: player,
            material_bonus: 0.0,
        });
        app.update();

        let events = app.world().resource::<Events<VfxEventRequest>>();
        let emitted = events
            .iter_current_update_events()
            .find(|event| {
                matches!(
                    &event.payload,
                    VfxEventPayloadV1::SpawnParticle { event_id, .. }
                        if event_id == gameplay_vfx::BREAKTHROUGH_FAIL
                )
            })
            .expect("rolled breakthrough failure should emit breakthrough_fail vfx");
        match &emitted.payload {
            VfxEventPayloadV1::SpawnParticle { event_id, .. } => {
                assert_eq!(event_id, gameplay_vfx::BREAKTHROUGH_FAIL);
            }
            other => panic!("expected SpawnParticle, got {other:?}"),
        }
    }

    /// 确定性 control（review finding major-1）——1/tick 拆批：两条请求分属两个 Update，
    /// 消费**连续**的 roll 值（r1 失败 → r2 必胜），Solidify→Spirit 拆批双连发收敛。
    /// 这是修复前的必死路径：per-Update 重建 XorshiftRoll 时，两条请求各自消费 r1=0.8597…，
    /// 而 Solidify→Spirit 顶到全态夏季也只有 0.693 < r1，永远过不去。
    #[test]
    fn breakthrough_roll_state_advances_across_updates_for_split_pair() {
        let mut app = App::new();
        let mut zones = ZoneRegistry::fallback();
        zones.find_zone_mut("spawn").unwrap().spirit_qi = 0.9;
        app.insert_resource(CultivationClock { tick: 10 });
        app.insert_resource(zones);
        app.insert_resource(WorldQiAccount::default());
        app.add_event::<BreakthroughRequest>();
        app.add_event::<BreakthroughOutcome>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<SkillCapChanged>();
        app.add_event::<SkillXpGain>();
        app.add_event::<SpiritEyeUsedForBreakthroughEvent>();
        app.add_systems(Update, breakthrough_system);

        let mut meridians = MeridianSystem::default();
        open_all_meridians(&mut meridians);
        let player = app
            .world_mut()
            .spawn((
                Cultivation {
                    realm: Realm::Solidify,
                    qi_current: 500.0,
                    qi_max: 500.0,
                    composure: 1.0,
                    ..Default::default()
                },
                meridians,
                LifeRecord::new("player_a"),
                Position::new([8.0, 66.0, 8.0]),
            ))
            .id();

        // tick N：只有第一条请求。r1=0.8597 > 全态夏季成功率 0.693 → 失败，境界停在 Solidify。
        app.world_mut().send_event(BreakthroughRequest {
            entity: player,
            material_bonus: 0.0,
        });
        app.update();
        let cultivation = app.world().get::<Cultivation>(player).unwrap();
        assert_eq!(
            cultivation.realm,
            Realm::Solidify,
            "1/tick 拆批首条请求消费 r1（高值）应失败，境界仍为 Solidify"
        );

        // tick N+1：第二条请求。roll 状态跨 Update 持久，消费 r2=0.3943 ≤ 成功率 → 成功。
        app.world_mut().send_event(BreakthroughRequest {
            entity: player,
            material_bonus: 0.0,
        });
        app.update();
        let cultivation = app.world().get::<Cultivation>(player).unwrap();
        assert_eq!(
            cultivation.realm,
            Realm::Spirit,
            "方案要求 1/tick 拆批的第二条请求消费连续 r2 并成功进阶 Spirit；"
            "若 roll 仍每 Update 重建（= 修复前），第二条请求会再消费 r1 而失败，境界停 Solidify"
        );
    }

    /// 确定性 control（review finding major-1）——同 Update 双连发：两条请求在同一 tick
    /// 消费 r1（败）、r2（胜），Solidify→Spirit 收敛。锁住"连续消费"的原有语义。
    #[test]
    fn breakthrough_roll_state_advances_within_same_update_for_paired_requests() {
        let mut app = App::new();
        let mut zones = ZoneRegistry::fallback();
        zones.find_zone_mut("spawn").unwrap().spirit_qi = 0.9;
        app.insert_resource(CultivationClock { tick: 10 });
        app.insert_resource(zones);
        app.insert_resource(WorldQiAccount::default());
        app.add_event::<BreakthroughRequest>();
        app.add_event::<BreakthroughOutcome>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<SkillCapChanged>();
        app.add_event::<SkillXpGain>();
        app.add_event::<SpiritEyeUsedForBreakthroughEvent>();
        app.add_systems(Update, breakthrough_system);

        let mut meridians = MeridianSystem::default();
        open_all_meridians(&mut meridians);
        let player = app
            .world_mut()
            .spawn((
                Cultivation {
                    realm: Realm::Solidify,
                    qi_current: 500.0,
                    qi_max: 500.0,
                    composure: 1.0,
                    ..Default::default()
                },
                meridians,
                LifeRecord::new("player_b"),
                Position::new([8.0, 66.0, 8.0]),
            ))
            .id();

        app.world_mut().send_event(BreakthroughRequest {
            entity: player,
            material_bonus: 0.0,
        });
        app.world_mut().send_event(BreakthroughRequest {
            entity: player,
            material_bonus: 0.0,
        });
        app.update();

        let cultivation = app.world().get::<Cultivation>(player).unwrap();
        assert_eq!(
            cultivation.realm,
            Realm::Spirit,
            "同 Update 双连发应连续消费 r1(败)/r2(胜) 并进阶 Spirit；"
            "若 roll 不随每笔请求推进，两条请求都消费 r1 而双败，境界停在 Solidify"
        );
    }

    #[test]
    fn guyuan_requires_high_qi_or_spirit_eye() {
        let mut zones = ZoneRegistry::fallback();
        zones.find_zone_mut("spawn").unwrap().spirit_qi = 0.6;
        let position = Position::new([8.0, 66.0, 8.0]);

        let err = breakthrough_environment_error(
            &position,
            DimensionKind::Overworld,
            Some(&zones),
            None,
            Realm::Condense,
        )
        .expect("low qi outside spirit eye should reject guyuan");

        assert_eq!(
            err,
            BreakthroughError::EnvInsufficient {
                need: MIN_ZONE_QI_TO_GUYUAN,
                have: 0.6,
                in_spirit_eye: false,
            }
        );
    }

    #[test]
    fn spirit_eye_bonus_raises_guyuan_success_rate() {
        let base = compute_success_rate_with_env_bonus(Realm::Solidify, 1.0, 1.0, 1.0, 0.0, 0.0);
        let boosted = compute_success_rate_with_env_bonus(
            Realm::Solidify,
            1.0,
            1.0,
            1.0,
            0.0,
            SPIRIT_EYE_BREAKTHROUGH_SUCCESS_BONUS,
        );

        assert!(boosted > base);
    }

    #[test]
    fn spirit_eye_bonus_is_gated_to_guyuan_breakthrough() {
        assert_eq!(
            spirit_eye_env_bonus_for(Realm::Condense, Some(false)),
            SPIRIT_EYE_BREAKTHROUGH_SUCCESS_BONUS
        );
        assert_eq!(
            spirit_eye_env_bonus_for(Realm::Condense, Some(true)),
            BLOOD_VALLEY_BREAKTHROUGH_SUCCESS_BONUS
        );
        assert_eq!(spirit_eye_env_bonus_for(Realm::Solidify, Some(false)), 0.0);
        assert_eq!(spirit_eye_env_bonus_for(Realm::Induce, Some(false)), 0.0);
        assert_eq!(spirit_eye_env_bonus_for(Realm::Condense, None), 0.0);
    }

    /// plan-skill-v1 §4 cap 表锚点：六境界分别对应 3/5/7/8/9/10。
    #[test]
    fn skill_cap_for_realm_matches_plan_section_four() {
        assert_eq!(skill_cap_for_realm(Realm::Awaken), 3);
        assert_eq!(skill_cap_for_realm(Realm::Induce), 5);
        assert_eq!(skill_cap_for_realm(Realm::Condense), 7);
        assert_eq!(skill_cap_for_realm(Realm::Solidify), 8);
        assert_eq!(skill_cap_for_realm(Realm::Spirit), 9);
        assert_eq!(skill_cap_for_realm(Realm::Void), 10);
    }

    fn setup_rapid_breakthrough_karma_app(now: u64) -> App {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: now });
        app.insert_resource(KarmaWeightStore::default());
        app.insert_resource(ZoneRegistry::fallback());
        app.add_event::<BreakthroughOutcome>();
        app.add_systems(Update, rapid_breakthrough_karma_mark_system);
        app
    }

    fn breakthrough_success_outcome(entity: Entity) -> BreakthroughOutcome {
        BreakthroughOutcome {
            entity,
            from: Realm::Awaken,
            result: Ok(BreakthroughSuccess {
                to: Realm::Induce,
                success_rate: 1.0,
                used_qi: 0.0,
            }),
        }
    }

    #[test]
    fn rapid_breakthrough_success_marks_hidden_karma_weight() {
        let now = RAPID_BREAKTHROUGH_KARMA_WINDOW_TICKS + 100;
        let mut app = setup_rapid_breakthrough_karma_app(now);
        let mut life = LifeRecord::new("offline:Azure");
        life.push(BiographyEntry::BreakthroughSucceeded {
            realm: Realm::Awaken,
            tick: now - 100,
        });
        life.push(BiographyEntry::BreakthroughSucceeded {
            realm: Realm::Induce,
            tick: now,
        });
        let entity = app
            .world_mut()
            .spawn((
                life,
                Username("Azure".to_string()),
                Position::new([8.8, 66.2, 8.1]),
            ))
            .id();

        app.world_mut()
            .send_event(breakthrough_success_outcome(entity));
        app.update();

        let weights = app.world().resource::<KarmaWeightStore>();
        let entry = weights
            .entry_for_player("Azure")
            .expect("rapid breakthroughs should mark hidden karma weight");
        assert_eq!(entry.weight, RAPID_BREAKTHROUGH_KARMA_WEIGHT_DELTA);
        assert_eq!(entry.zone.as_deref(), Some("spawn"));
        assert_eq!(entry.last_position, [8, 66, 8]);
        assert_eq!(entry.last_tick, now);
    }

    #[test]
    fn old_breakthrough_success_outside_window_does_not_mark_karma() {
        let now = RAPID_BREAKTHROUGH_KARMA_WINDOW_TICKS + 100;
        let mut app = setup_rapid_breakthrough_karma_app(now);
        let mut life = LifeRecord::new("offline:Azure");
        life.push(BiographyEntry::BreakthroughSucceeded {
            realm: Realm::Awaken,
            tick: now - RAPID_BREAKTHROUGH_KARMA_WINDOW_TICKS - 1,
        });
        life.push(BiographyEntry::BreakthroughSucceeded {
            realm: Realm::Induce,
            tick: now,
        });
        let entity = app
            .world_mut()
            .spawn((
                life,
                Username("Azure".to_string()),
                Position::new([8.0, 66.0, 8.0]),
            ))
            .id();

        app.world_mut()
            .send_event(breakthrough_success_outcome(entity));
        app.update();

        let weights = app.world().resource::<KarmaWeightStore>();
        assert!(weights.entry_for_player("Azure").is_none());
    }

    #[test]
    fn failed_breakthrough_outcome_does_not_mark_karma() {
        let now = RAPID_BREAKTHROUGH_KARMA_WINDOW_TICKS + 100;
        let mut app = setup_rapid_breakthrough_karma_app(now);
        let mut life = LifeRecord::new("offline:Azure");
        life.push(BiographyEntry::BreakthroughSucceeded {
            realm: Realm::Awaken,
            tick: now - 100,
        });
        life.push(BiographyEntry::BreakthroughSucceeded {
            realm: Realm::Induce,
            tick: now,
        });
        let entity = app
            .world_mut()
            .spawn((
                life,
                Username("Azure".to_string()),
                Position::new([8.0, 66.0, 8.0]),
            ))
            .id();

        app.world_mut().send_event(BreakthroughOutcome {
            entity,
            from: Realm::Awaken,
            result: Err(BreakthroughError::RolledFailure { severity: 0.2 }),
        });
        app.update();

        let weights = app.world().resource::<KarmaWeightStore>();
        assert!(weights.entry_for_player("Azure").is_none());
    }

    // ───────────────────────────────────────────────────────────────────────
    // qi_max_frozen cap: 突破失败不能永久废人
    // ───────────────────────────────────────────────────────────────────────

    /// 单次失败：qi_max_frozen 精确加上 severity * 10.0，且不超过 qi_max * 0.5。
    #[test]
    fn single_breakthrough_failure_freezes_qi_within_cap() {
        let (mut c, mut m) = setup_for_induce();
        // 强制失败：roll > base_success_rate(Induce)=0.90
        let _ = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(1.0));

        // severity = (1.0 - success_rate).clamp(0.1, 0.9)
        // success_rate = base × composure × integrity × completeness = 0.90 × 1.0 × 1.0 × 1.0 = 0.90
        // severity = 0.10, freeze_add = 0.10 * 10.0 = 1.0
        // cap = qi_max(100.0) * 0.5 = 50.0 → 1.0 < 50.0, no clamping
        let frozen = c
            .qi_max_frozen
            .expect("qi_max_frozen should be Some after failure");
        assert!(
            (frozen - 1.0).abs() < 1e-9,
            "期望 qi_max_frozen = 1.0（severity=0.10 × 10.0），实际 = {frozen}"
        );
        let effective = c.qi_max - frozen;
        assert!(
            effective > 0.0,
            "期望有效 qi_max > 0 以防玩家废人，实际 effective_qi_max = {effective}"
        );
    }

    /// 连续多次失败，qi_max_frozen 不超过 qi_max * 0.5 的硬上限。
    #[test]
    fn repeated_breakthrough_failures_frozen_capped_at_half_qi_max() {
        let qi_max = 100.0;
        let mut c = Cultivation {
            realm: Realm::Awaken,
            qi_current: qi_max,
            qi_max,
            composure: 0.0, // composure=0 → success_rate 极低 → severity 接近 0.9
            ..Default::default()
        };
        let mut m = MeridianSystem::default();
        open_regular(&mut m, 3);

        // 失败 20 次：无 cap 时 severity≈0.9, freeze_add≈9.0, 2 次即超 qi_max=100 × 0.5=50
        for _ in 0..20 {
            // qi_current 须 ≥ breakthrough_qi_cost(Induce)=8.0，补满避免 NotEnoughQi 前置错误
            c.qi_current = qi_max;
            let _ = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(1.0));
        }

        let frozen = c
            .qi_max_frozen
            .expect("qi_max_frozen should be Some after repeated failures");
        let cap = qi_max * BREAKTHROUGH_FAIL_FROZEN_CAP_RATIO;

        assert!(
            frozen <= cap,
            "期望 qi_max_frozen ≤ {cap}（qi_max×0.5，防废人），实际 qi_max_frozen = {frozen}"
        );

        let effective_qi_max = c.qi_max - frozen;
        assert!(
            effective_qi_max > 0.0,
            "期望有效真元上限 > 0（玩家不应被永久废），实际 effective_qi_max = {effective_qi_max}"
        );
    }

    /// 验证 cap 边界：pre-existing frozen 接近 cap 时，再叠一次不会超过 cap。
    #[test]
    fn breakthrough_failure_does_not_exceed_cap_when_already_near_cap() {
        let qi_max = 100.0;
        // 预填到接近 cap（40/100 = 0.4 × qi_max，距 0.5×qi_max=50 还差 10）
        let mut c = Cultivation {
            realm: Realm::Awaken,
            qi_current: qi_max,
            qi_max,
            composure: 0.0, // severity 接近 0.9 → freeze_add 接近 9.0，足够触碰 cap
            qi_max_frozen: Some(40.0),
            ..Default::default()
        };
        let mut m = MeridianSystem::default();
        open_regular(&mut m, 3);

        let _ = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(1.0));

        let frozen = c
            .qi_max_frozen
            .expect("qi_max_frozen should be Some after failure");
        let cap = qi_max * BREAKTHROUGH_FAIL_FROZEN_CAP_RATIO;

        assert!(
            frozen <= cap + 1e-9,
            "期望 qi_max_frozen ≤ {cap}（cap = qi_max×0.5），实际 qi_max_frozen = {frozen}（超 cap）"
        );
        // 有效上限仍须 > 0
        let effective = c.qi_max - frozen;
        assert!(
            effective >= qi_max * (1.0 - BREAKTHROUGH_FAIL_FROZEN_CAP_RATIO) - 1e-9,
            "期望有效 qi_max ≥ qi_max×0.5={cap}，实际 = {effective}"
        );
    }

    /// 成功突破不应修改 qi_max_frozen。
    #[test]
    fn successful_breakthrough_does_not_change_qi_max_frozen() {
        let (mut c, mut m) = setup_for_induce();
        c.qi_max_frozen = Some(5.0); // 预存冻结，验证成功路径不碰它

        let _ = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(0.0)).unwrap();

        assert_eq!(
            c.qi_max_frozen,
            Some(5.0),
            "期望成功突破不修改 qi_max_frozen（仍为 5.0），实际 = {:?}",
            c.qi_max_frozen
        );
    }
}
