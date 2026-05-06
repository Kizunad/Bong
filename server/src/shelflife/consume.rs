//! plan-shelflife-v1 §5 / M5a — 消费侧 helper + event 基础设施。
//!
//! 提供三条路径的纯函数判定 + 两个 Bevy event。各消费 plan（alchemy / forge /
//! cultivation / food / 骨币交易）在自家入口前调对应 helper，按返回结果决定
//! 折算 / 警告 / 加成；event 走 Bevy 事件总线广播给观察侧（HUD / 命令行 /
//! 试药史记录等）。
//!
//! 落地阶段：
//! - **M5a** ← 本文件 — 三 helper + 两 event + 单测，**零侵入** alchemy / pill
//! - M5b — `consume_pill` 接入 spoil_check
//! - M5c — alchemy session staged 接入 decay_current_qi_factor
//! - M5d — pill consume 接入 age_peak_check + AgeBonusRoll
//!
//! 设计要点：
//! - 三 helper **均不发 event**（保持纯函数 + 可单测）；event 由调用侧根据返回值发
//! - `*Outcome` enum 的 `NotApplicable` 分支让调用侧能用统一签名兜底，不必预先 dispatch path
//! - 严格 `<` 阈值语义（plan §6.3）— Outcome 边界判定与 `compute_track_state` 一致

use valence::prelude::{bevy_ecs, Entity, Event};

use super::compute::{
    compute_current_qi, compute_current_qi_with_season, compute_track_state_with_season,
};
use super::types::{DecayProfile, Freshness, TrackState};
use crate::world::season::Season;

/// plan §5.2 — Spoil 路径"极低拒消费"的阈值比例。
/// `current < CRITICAL_BLOCK_RATIO × spoil_threshold` 时返回 `CriticalBlock`，
/// 调用侧应拒绝自动消费 + 弹二次确认（像吃屎）。
pub const CRITICAL_BLOCK_RATIO: f32 = 0.1;

// ============== Events ==============

/// plan §5.2 — Spoil 路径消费时的危险警告。
///
/// 调用侧（如 `consume_pill`）在 `spoil_check` 返回 `Warn` / `CriticalBlock` 时
/// 写本事件，供 HUD / chat / 试药史记录消费。
#[derive(Debug, Clone, Event)]
pub struct SpoilConsumeWarning {
    pub player: Entity,
    pub instance_id: u64,
    pub severity: SpoilSeverity,
    pub current_qi: f32,
    pub spoil_threshold: f32,
}

/// `SpoilConsumeWarning.severity` — 与 `SpoilCheckOutcome::{Warn,CriticalBlock}` 对应。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpoilSeverity {
    /// `current < spoil_threshold` 但 `≥ 0.1 × spoil_threshold`：触发 contam（Sharp 档丹毒）。
    Sharp,
    /// `current < 0.1 × spoil_threshold`：拒绝自动消费，需玩家二次确认。
    CriticalBlock,
}

/// plan §5.3 — Age 路径峰值消费的加成触发。
///
/// 调用侧（如 `consume_pill`）在 `age_peak_check` 返回 `Peaking` 时写本事件，
/// 供 alchemy 成丹率 / quality / 修炼吸收效率等消费方按 `bonus_strength` 放大。
#[derive(Debug, Clone, Event)]
pub struct AgeBonusRoll {
    pub player: Entity,
    pub instance_id: u64,
    /// PeakAndFall 的 `peak_bonus`（0.5 = 峰值为 initial × 1.5）。
    /// 消费侧据此放大效果（如 alchemy quality × (1 + bonus_strength)）。
    pub bonus_strength: f32,
}

// ============== Helper outcomes ==============

/// plan §5.2 — `spoil_check` 的判定结果。
#[derive(Debug, Clone, PartialEq)]
pub enum SpoilCheckOutcome {
    /// 非 Spoil 路径（Decay / Age）— 调用侧无需做 Spoil 校验，直接放行。
    NotApplicable,
    /// `current ≥ spoil_threshold`（**严格**：边界值 `==` 也算 Safe，plan §6.3）。
    Safe { current_qi: f32 },
    /// `current < spoil_threshold` 且 `≥ 0.1 × spoil_threshold` — 触发 contam。
    Warn {
        current_qi: f32,
        spoil_threshold: f32,
    },
    /// `current < 0.1 × spoil_threshold` — 拒绝自动消费。
    CriticalBlock {
        current_qi: f32,
        spoil_threshold: f32,
    },
}

/// plan §5.3 — `age_peak_check` 的判定结果。
#[derive(Debug, Clone, PartialEq)]
pub enum AgePeakCheck {
    /// 非 Age 路径（Decay / Spoil）— 调用侧无需做峰值检查。
    NotApplicable,
    /// Age 路径但当前不在 Peaking 窗口（Fresh / PastPeak / AgePostPeakSpoiled）。
    NotPeaking,
    /// Age 路径且处于 `peak_at_ticks ± peak_window_ratio` 窗口 — 触发 bonus。
    Peaking {
        /// 来自 profile 的 `peak_bonus`，传给消费侧放大效果。
        bonus_strength: f32,
    },
}

// ============== Helpers ==============

/// plan §5.1 — Decay 路径折算因子：`current_qi / initial_qi` 比率。
///
/// 消费侧（alchemy session / forge / 修炼吸收 / 骨币交易）按"原始贡献度 × factor"
/// 折算当下贡献。`factor < 1.0` 表已损耗，`factor == 0.0` 表死物（current ≤ floor）。
///
/// **路径兼容**：传入 Decay/Spoil 都返回 ratio；Age 路径**不应**走本 helper（PeakAndFall
/// 可能 > 1.0，语义不符），但为防御性兜底也返回 `current/initial`，调用侧用 §5.3 的
/// `age_peak_check` 替代。
///
/// 返回值 `[0.0, +∞)`，正常 Decay/Spoil 在 `[0.0, 1.0]`。
pub fn decay_current_qi_factor(
    freshness: &Freshness,
    profile: &DecayProfile,
    now_tick: u64,
    storage_multiplier: f32,
) -> f32 {
    let current = compute_current_qi(freshness, profile, now_tick, storage_multiplier);
    let initial = freshness.initial_qi;
    if initial <= f32::EPSILON {
        return 0.0;
    }
    (current / initial).max(0.0)
}

pub fn decay_current_qi_factor_with_season(
    freshness: &Freshness,
    profile: &DecayProfile,
    now_tick: u64,
    storage_multiplier: f32,
    season: Season,
    entropy_seed: u64,
) -> f32 {
    let current = compute_current_qi_with_season(
        freshness,
        profile,
        now_tick,
        storage_multiplier,
        season,
        entropy_seed,
    );
    let initial = freshness.initial_qi;
    if initial <= f32::EPSILON {
        return 0.0;
    }
    (current / initial).max(0.0)
}

/// plan §5.2 — Spoil 路径消费前置校验。
///
/// 非 Spoil profile 直接返回 `NotApplicable`；Spoil profile 按 `current` 与
/// `spoil_threshold` 比较：
/// - `current ≥ threshold` → `Safe`（含边界 `==`，plan §6.3 严格 `<`）
/// - `0.1 × threshold ≤ current < threshold` → `Warn`
/// - `current < 0.1 × threshold` → `CriticalBlock`
///
/// 调用侧拿 `Warn`/`CriticalBlock` 时应写 `SpoilConsumeWarning` event。
pub fn spoil_check(
    freshness: &Freshness,
    profile: &DecayProfile,
    now_tick: u64,
    storage_multiplier: f32,
) -> SpoilCheckOutcome {
    let DecayProfile::Spoil {
        spoil_threshold, ..
    } = profile
    else {
        return SpoilCheckOutcome::NotApplicable;
    };
    let current = compute_current_qi(freshness, profile, now_tick, storage_multiplier);
    let threshold = *spoil_threshold;
    if current >= threshold {
        SpoilCheckOutcome::Safe {
            current_qi: current,
        }
    } else if current < CRITICAL_BLOCK_RATIO * threshold {
        SpoilCheckOutcome::CriticalBlock {
            current_qi: current,
            spoil_threshold: threshold,
        }
    } else {
        SpoilCheckOutcome::Warn {
            current_qi: current,
            spoil_threshold: threshold,
        }
    }
}

pub fn spoil_check_with_season(
    freshness: &Freshness,
    profile: &DecayProfile,
    now_tick: u64,
    storage_multiplier: f32,
    season: Season,
    entropy_seed: u64,
) -> SpoilCheckOutcome {
    let DecayProfile::Spoil {
        spoil_threshold, ..
    } = profile
    else {
        return SpoilCheckOutcome::NotApplicable;
    };
    let current = compute_current_qi_with_season(
        freshness,
        profile,
        now_tick,
        storage_multiplier,
        season,
        entropy_seed,
    );
    let threshold = *spoil_threshold;
    if current >= threshold {
        SpoilCheckOutcome::Safe {
            current_qi: current,
        }
    } else if current < CRITICAL_BLOCK_RATIO * threshold {
        SpoilCheckOutcome::CriticalBlock {
            current_qi: current,
            spoil_threshold: threshold,
        }
    } else {
        SpoilCheckOutcome::Warn {
            current_qi: current,
            spoil_threshold: threshold,
        }
    }
}

/// plan §5.3 — Age 路径峰值检查。
///
/// 复用 `compute_track_state` 的 `Peaking` 判定，避免逻辑重复 / 漂移。返回
/// `Peaking` 时调用侧应写 `AgeBonusRoll` event。
///
/// 非 Age profile 直接返回 `NotApplicable`。
pub fn age_peak_check(
    freshness: &Freshness,
    profile: &DecayProfile,
    now_tick: u64,
    storage_multiplier: f32,
) -> AgePeakCheck {
    let DecayProfile::Age { peak_bonus, .. } = profile else {
        return AgePeakCheck::NotApplicable;
    };
    let state =
        super::compute::compute_track_state(freshness, profile, now_tick, storage_multiplier);
    if state == TrackState::Peaking {
        AgePeakCheck::Peaking {
            bonus_strength: *peak_bonus,
        }
    } else {
        AgePeakCheck::NotPeaking
    }
}

pub fn age_peak_check_with_season(
    freshness: &Freshness,
    profile: &DecayProfile,
    now_tick: u64,
    storage_multiplier: f32,
    season: Season,
    entropy_seed: u64,
) -> AgePeakCheck {
    let DecayProfile::Age { peak_bonus, .. } = profile else {
        return AgePeakCheck::NotApplicable;
    };
    let state = compute_track_state_with_season(
        freshness,
        profile,
        now_tick,
        storage_multiplier,
        season,
        entropy_seed,
    );
    if state == TrackState::Peaking {
        AgePeakCheck::Peaking {
            bonus_strength: *peak_bonus,
        }
    } else {
        AgePeakCheck::NotPeaking
    }
}

#[cfg(test)]
mod tests {
    use super::super::types::{DecayFormula, DecayProfileId};
    use super::*;

    fn decay_exp(half_life: u64, floor: f32) -> DecayProfile {
        DecayProfile::Decay {
            id: DecayProfileId::new("t_decay"),
            formula: DecayFormula::Exponential {
                half_life_ticks: half_life,
            },
            floor_qi: floor,
        }
    }

    fn spoil_exp(half_life: u64, threshold: f32) -> DecayProfile {
        DecayProfile::Spoil {
            id: DecayProfileId::new("t_spoil"),
            formula: DecayFormula::Exponential {
                half_life_ticks: half_life,
            },
            spoil_threshold: threshold,
        }
    }

    fn age_p(peak: u64, bonus: f32, post_half: u64, spoil_th: f32) -> DecayProfile {
        DecayProfile::Age {
            id: DecayProfileId::new("t_age"),
            peak_at_ticks: peak,
            peak_bonus: bonus,
            peak_window_ratio: 0.1,
            post_peak_half_life_ticks: post_half,
            post_peak_spoil_threshold: spoil_th,
            post_peak_spoil_profile: DecayProfileId::new("t_age_spoil"),
        }
    }

    fn fresh(profile: &DecayProfile, initial: f32, created: u64) -> Freshness {
        Freshness::new(created, initial, profile)
    }

    // ============== decay_current_qi_factor ==============

    #[test]
    fn decay_factor_at_creation_is_one() {
        let p = decay_exp(1000, 0.0);
        let f = fresh(&p, 100.0, 0);
        let r = decay_current_qi_factor(&f, &p, 0, 1.0);
        assert!((r - 1.0).abs() < 1e-3);
    }

    #[test]
    fn decay_factor_at_one_half_life_is_half() {
        let p = decay_exp(1000, 0.0);
        let f = fresh(&p, 100.0, 0);
        let r = decay_current_qi_factor(&f, &p, 1000, 1.0);
        assert!((r - 0.5).abs() < 1e-3);
    }

    #[test]
    fn decay_factor_floors_at_floor_qi_ratio() {
        // initial=100, floor=10 → factor 永远 ≥ 0.1
        let p = decay_exp(100, 10.0);
        let f = fresh(&p, 100.0, 0);
        let r = decay_current_qi_factor(&f, &p, 1_000_000, 1.0);
        assert!((r - 0.1).abs() < 1e-3);
    }

    #[test]
    fn decay_factor_zero_initial_returns_zero_safely() {
        let p = decay_exp(1000, 0.0);
        let f = fresh(&p, 0.0, 0);
        let r = decay_current_qi_factor(&f, &p, 500, 1.0);
        assert_eq!(r, 0.0);
    }

    #[test]
    fn decay_factor_works_for_spoil_path_too() {
        // §5.2 消费侧也可能要 ratio（如丹药 quality 折算），Spoil profile 应正常返回。
        let p = spoil_exp(1000, 20.0);
        let f = fresh(&p, 100.0, 0);
        let r = decay_current_qi_factor(&f, &p, 1000, 1.0);
        assert!((r - 0.5).abs() < 1e-3);
    }

    // ============== spoil_check ==============

    #[test]
    fn spoil_check_not_applicable_for_decay() {
        let p = decay_exp(1000, 0.0);
        let f = fresh(&p, 100.0, 0);
        assert_eq!(
            spoil_check(&f, &p, 5000, 1.0),
            SpoilCheckOutcome::NotApplicable
        );
    }

    #[test]
    fn spoil_check_not_applicable_for_age() {
        let p = age_p(1000, 0.5, 500, 30.0);
        let f = fresh(&p, 100.0, 0);
        assert_eq!(
            spoil_check(&f, &p, 1000, 1.0),
            SpoilCheckOutcome::NotApplicable
        );
    }

    #[test]
    fn spoil_check_safe_at_creation() {
        let p = spoil_exp(1000, 20.0);
        let f = fresh(&p, 100.0, 0);
        match spoil_check(&f, &p, 0, 1.0) {
            SpoilCheckOutcome::Safe { current_qi } => assert!((current_qi - 100.0).abs() < 1e-3),
            other => panic!("expected Safe, got {other:?}"),
        }
    }

    #[test]
    fn spoil_check_exact_threshold_is_safe_strict_lt() {
        // plan §6.3：边界值 `current == spoil_threshold` 不触发 Spoiled。
        // half_life=1000, threshold=50 → 1 half_life 后 current=50 = threshold → Safe
        let p = spoil_exp(1000, 50.0);
        let f = fresh(&p, 100.0, 0);
        let outcome = spoil_check(&f, &p, 1000, 1.0);
        assert!(
            matches!(outcome, SpoilCheckOutcome::Safe { .. }),
            "exactly == threshold should be Safe (strict `<`), got {outcome:?}"
        );
    }

    #[test]
    fn spoil_check_warn_below_threshold() {
        // half_life=1000, threshold=50 → 2 half_life 后 current=25 < 50 但 ≥ 5 → Warn
        let p = spoil_exp(1000, 50.0);
        let f = fresh(&p, 100.0, 0);
        let outcome = spoil_check(&f, &p, 2000, 1.0);
        match outcome {
            SpoilCheckOutcome::Warn {
                current_qi,
                spoil_threshold,
            } => {
                assert!((current_qi - 25.0).abs() < 1e-3);
                assert!((spoil_threshold - 50.0).abs() < 1e-3);
            }
            other => panic!("expected Warn, got {other:?}"),
        }
    }

    #[test]
    fn spoil_check_critical_block_at_extreme_decay() {
        // threshold=50, critical 阈值 = 5。half_life=1000:
        // current ≤ 5 → 0.5^n ≤ 0.05 → n ≥ 4.32 → tick ≥ 4322
        let p = spoil_exp(1000, 50.0);
        let f = fresh(&p, 100.0, 0);
        let outcome = spoil_check(&f, &p, 5000, 1.0);
        match outcome {
            SpoilCheckOutcome::CriticalBlock {
                current_qi,
                spoil_threshold,
            } => {
                assert!(current_qi < 5.0);
                assert!((spoil_threshold - 50.0).abs() < 1e-3);
            }
            other => panic!("expected CriticalBlock, got {other:?}"),
        }
    }

    #[test]
    fn spoil_check_critical_block_strict_lt_at_ratio_boundary() {
        // current 恰好 = 0.1 × threshold 时，应走 Warn 而非 CriticalBlock
        // （strict `<` 语义 — `current < 0.1 × threshold` 才 critical）。
        // threshold=50, target current=5: half_life=1000, 0.5^n=0.05 → n≈4.3219
        // 直接构造 frozen-zero、current=5：用 Linear 公式更精确
        let p = DecayProfile::Spoil {
            id: DecayProfileId::new("t"),
            formula: DecayFormula::Linear {
                decay_per_tick: 0.95,
            },
            spoil_threshold: 50.0,
        };
        let f = fresh(&p, 100.0, 0);
        // dt=100 → current = 100 - 0.95*100 = 5.0 = 0.1 × 50 → 边界，应 Warn
        let outcome = spoil_check(&f, &p, 100, 1.0);
        assert!(
            matches!(outcome, SpoilCheckOutcome::Warn { .. }),
            "current == 0.1 × threshold should be Warn (strict `<`), got {outcome:?}"
        );
    }

    // ============== age_peak_check ==============

    #[test]
    fn age_peak_check_not_applicable_for_decay() {
        let p = decay_exp(1000, 0.0);
        let f = fresh(&p, 100.0, 0);
        assert_eq!(
            age_peak_check(&f, &p, 5000, 1.0),
            AgePeakCheck::NotApplicable
        );
    }

    #[test]
    fn age_peak_check_not_applicable_for_spoil() {
        let p = spoil_exp(1000, 20.0);
        let f = fresh(&p, 100.0, 0);
        assert_eq!(
            age_peak_check(&f, &p, 1000, 1.0),
            AgePeakCheck::NotApplicable
        );
    }

    #[test]
    fn age_peak_check_pre_peak_not_peaking() {
        let p = age_p(1000, 0.5, 500, 30.0);
        let f = fresh(&p, 100.0, 0);
        assert_eq!(age_peak_check(&f, &p, 100, 1.0), AgePeakCheck::NotPeaking);
    }

    #[test]
    fn age_peak_check_in_window_returns_bonus() {
        let p = age_p(1000, 0.5, 500, 30.0);
        let f = fresh(&p, 100.0, 0);
        // peak_window_ratio=0.1 → 窗口 [900, 1100]
        match age_peak_check(&f, &p, 1000, 1.0) {
            AgePeakCheck::Peaking { bonus_strength } => {
                assert!((bonus_strength - 0.5).abs() < 1e-3)
            }
            other => panic!("expected Peaking, got {other:?}"),
        }
    }

    #[test]
    fn age_peak_check_past_peak_not_peaking() {
        let p = age_p(1000, 0.5, 500, 30.0);
        let f = fresh(&p, 100.0, 0);
        // tick 2000：PastPeak，不发 bonus
        assert_eq!(age_peak_check(&f, &p, 2000, 1.0), AgePeakCheck::NotPeaking);
    }

    #[test]
    fn age_peak_check_after_spoil_migration_not_peaking() {
        let p = age_p(1000, 0.5, 500, 30.0);
        let f = fresh(&p, 100.0, 0);
        // tick 2500：current ≈ 18.75 < 30 → AgePostPeakSpoiled，仍非 Peaking
        assert_eq!(age_peak_check(&f, &p, 2500, 1.0), AgePeakCheck::NotPeaking);
    }

    #[test]
    fn age_peak_check_propagates_storage_multiplier() {
        // Halve 容器 (multiplier=0.5)：peak 推迟到 tick 2000；窗口 [1800, 2200]
        let p = age_p(1000, 0.5, 500, 30.0);
        let f = fresh(&p, 100.0, 0);
        assert_eq!(age_peak_check(&f, &p, 1000, 0.5), AgePeakCheck::NotPeaking);
        match age_peak_check(&f, &p, 2000, 0.5) {
            AgePeakCheck::Peaking { bonus_strength } => {
                assert!((bonus_strength - 0.5).abs() < 1e-3)
            }
            other => panic!("expected Peaking under storage_multiplier=0.5, got {other:?}"),
        }
    }
}
