//! plan-skill-v1 §10 forge 钩子 —— pure helper 层。
//!
//! 嵌入点：
//!   - §7.3 XP：`finalize_outcome` 发 `ForgeOutcomeEvent` 时用 `xp_for_outcome` 算 amount
//!   - §6.3 Tempering 命中窗口：`apply_tempering_hit` / `resolve_tempering` 时把 profile.window
//!     的 tick 范围按 `tempering_window_bonus_ticks(effective_lv)` 扩展
//!   - §6.3 允许失误 +次：tempering profile.allowed_misses 加 `allowed_miss_bonus(effective_lv)`
//!   - §6.3 铭文失败率 -%：inscription 内部 roll 时乘 `(1 - inscription_failure_rate_reduction)`
//!
//! `effective_lv = min(real_lv, cap)`，cap 由 `cultivation::breakthrough::skill_cap_for_realm`。

use crate::forge::events::ForgeBucket;
use crate::skill::curve::interp;

/// plan §7.3 分步累加 XP：
///   * billet 默认 +1（plan 约束 step[0] 必 billet，能走到 finalize 意味着 billet 已成）
///   * tempering perfect +4 / good +2
///   * inscription 成 +3（bucket 非 Flawed/Waste 才算成）
///   * consecration 成 +5（同上）
///   * 废品 +0，炸砧 +1
///
/// Flawed 时仅保留 billet 的 +1（其他步骤视为"学到了但没做好"，不额外奖）。
pub fn xp_for_outcome(
    bucket: ForgeBucket,
    has_tempering: bool,
    has_inscription: bool,
    has_consecration: bool,
) -> u32 {
    match bucket {
        ForgeBucket::Waste => 0,
        ForgeBucket::Explode => 1,
        ForgeBucket::Flawed => 1, // 仅 billet +1
        ForgeBucket::Good => {
            let mut xp = 1; // billet
            if has_tempering {
                xp += 2; // good 淬炼
            }
            if has_inscription {
                xp += 3;
            }
            if has_consecration {
                xp += 5;
            }
            xp
        }
        ForgeBucket::Perfect => {
            let mut xp = 1; // billet
            if has_tempering {
                xp += 4; // perfect 淬炼
            }
            if has_inscription {
                xp += 3;
            }
            if has_consecration {
                xp += 5;
            }
            xp
        }
    }
}

/// plan §6.3 Tempering 命中窗口额外 tick：Lv 插值到 [0, 8]。
pub fn tempering_window_bonus_ticks(effective_lv: u8) -> u32 {
    interp(
        effective_lv,
        &[(0, 0.0), (1, 1.0), (3, 3.0), (5, 5.0), (7, 6.0), (10, 8.0)],
    )
    .round() as u32
}

/// plan §6.3 Tempering 允许失误数加成：Lv 插值到 [0, 3]。
pub fn allowed_miss_bonus(effective_lv: u8) -> u32 {
    interp(
        effective_lv,
        &[(0, 0.0), (1, 0.0), (3, 1.0), (5, 1.0), (7, 2.0), (10, 3.0)],
    )
    .round() as u32
}

/// plan §6.3 铭文失败率减免：Lv 插值到 [0, 0.30]（0.30 意味失败率 × 0.70）。
pub fn inscription_failure_rate_reduction(effective_lv: u8) -> f32 {
    interp(
        effective_lv,
        &[
            (0, 0.00),
            (1, 0.03),
            (3, 0.10),
            (5, 0.15),
            (7, 0.22),
            (10, 0.30),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// plan §7.3 锚点：一个 blueprint 有全部四步且 Perfect 时 XP = 1+4+3+5 = 13。
    #[test]
    fn xp_perfect_full_blueprint_sums_to_thirteen() {
        assert_eq!(xp_for_outcome(ForgeBucket::Perfect, true, true, true), 13);
    }

    /// Good 全步：1+2+3+5 = 11。
    #[test]
    fn xp_good_full_blueprint_sums_to_eleven() {
        assert_eq!(xp_for_outcome(ForgeBucket::Good, true, true, true), 11);
    }

    /// 只有 billet 的最简 blueprint（无 tempering/inscription/consecration）：
    /// Perfect 或 Good 都只给 +1（billet）。
    #[test]
    fn xp_billet_only_blueprint_gives_one() {
        assert_eq!(xp_for_outcome(ForgeBucket::Perfect, false, false, false), 1);
        assert_eq!(xp_for_outcome(ForgeBucket::Good, false, false, false), 1);
    }

    #[test]
    fn xp_flawed_explode_waste_match_plan() {
        // Flawed 无论几步都只 +1（billet 学到）
        assert_eq!(xp_for_outcome(ForgeBucket::Flawed, true, true, true), 1);
        // Explode 固定 +1
        assert_eq!(xp_for_outcome(ForgeBucket::Explode, true, true, true), 1);
        // Waste 0
        assert_eq!(xp_for_outcome(ForgeBucket::Waste, true, true, true), 0);
    }

    /// plan §6.3 Tempering 窗口锚点。
    #[test]
    fn tempering_window_bonus_matches_plan_endpoints() {
        assert_eq!(tempering_window_bonus_ticks(0), 0);
        assert_eq!(tempering_window_bonus_ticks(1), 1);
        assert_eq!(tempering_window_bonus_ticks(3), 3);
        assert_eq!(tempering_window_bonus_ticks(5), 5);
        assert_eq!(tempering_window_bonus_ticks(7), 6);
        assert_eq!(tempering_window_bonus_ticks(10), 8);
    }

    #[test]
    fn allowed_miss_and_failure_reduction_endpoints() {
        assert_eq!(allowed_miss_bonus(0), 0);
        assert_eq!(allowed_miss_bonus(5), 1);
        assert_eq!(allowed_miss_bonus(10), 3);
        let eps = 1e-5_f32;
        assert!((inscription_failure_rate_reduction(0) - 0.00).abs() < eps);
        assert!((inscription_failure_rate_reduction(10) - 0.30).abs() < eps);
    }
}
