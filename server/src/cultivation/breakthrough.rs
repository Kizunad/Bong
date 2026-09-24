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
use super::overload::FREEZE_FACTOR;
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
        // 突破失败：真元上限冻结。severity ∈ [0.1, 0.9]，与过载路径使用同一冻结系数。
        // 无 cap 时多次失败可致 qi_max_frozen ≥ qi_max → 有效上限归零 → 玩家永久废人。
        // 与 overload.rs 对齐：冻结量不超过 qi_max * BREAKTHROUGH_FAIL_FROZEN_CAP_RATIO (0.5)。
        let new_frozen = (cultivation.qi_max_frozen.unwrap_or(0.0) + severity * FREEZE_FACTOR)
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
#[path = "breakthrough_tests.rs"]
mod tests;
