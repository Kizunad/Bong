//! plan-skill-v1 §10 alchemy 钩子 —— pure helper 层。
//!
//! 本文件**只**定义 skill Lv → alchemy 可观察量 的映射（plan §6.2 / §7.2），
//! 不绑定到 ECS system。当 alchemy 的 resolver/session ECS driver（plan-alchemy-v1 P5）
//! 实装后，相应 system 读取这里的 pure 函数即可完成接入。
//!
//! 嵌入点提示：
//!   - §7.2 XP：resolver::resolve 的 bucket 结算结束后，`xp_for_bucket(bucket)` → `SkillXpGain`
//!   - §6.2 火候容差：session::classify 前把 `FireProfile.tolerance.temp_band / duration_band`
//!     分别乘 `tolerance_scale(effective_lv)`
//!   - §6.2 残缺池权重：outcome::pick_side_effect 前把坏效果 weight × `side_effect_weight_scale`
//!   - §6.2 丹毒抗性：cultivation::contamination_tick 的 purge_rate 叠加 `purge_rate_bonus`
//!
//! `effective_lv` 由调用方算好（`skill::curve::effective_lv(real_lv, cap)`；cap 见
//! `cultivation::breakthrough::skill_cap_for_realm`）。

use crate::alchemy::outcome::OutcomeBucket;
use crate::skill::curve::interp;

/// plan §7.2 炼丹 XP 表 source-of-truth。
pub fn xp_for_bucket(bucket: OutcomeBucket) -> u32 {
    match bucket {
        OutcomeBucket::Perfect => 6,
        OutcomeBucket::Good => 3,
        OutcomeBucket::Flawed => 2,
        OutcomeBucket::Explode => 1, // 炸炉 +1：失败付代价同时给 1 XP
        OutcomeBucket::Waste => 0,   // 投错乱搞不给
    }
}

/// plan §7.2 "读懂丹方残卷（学习新方）" 的 XP 一次性奖励。
pub const LEARN_RECIPE_XP: u32 = 1;

/// plan §6.2 火候容差乘子：Lv 插值到 [1.0, 1.50]。
pub fn tolerance_scale(effective_lv: u8) -> f32 {
    interp(
        effective_lv,
        &[
            (0, 1.00),
            (1, 1.05),
            (3, 1.15),
            (5, 1.25),
            (7, 1.35),
            (10, 1.50),
        ],
    )
}

/// plan §6.2 残缺池"坏" side_effect 权重乘子：Lv 插值到 [1.0, 0.40]。高 Lv 减坏效果。
pub fn side_effect_weight_scale(effective_lv: u8) -> f32 {
    interp(
        effective_lv,
        &[
            (0, 1.00),
            (1, 0.95),
            (3, 0.85),
            (5, 0.75),
            (7, 0.60),
            (10, 0.40),
        ],
    )
}

/// plan §6.2 丹毒抗性加成：Lv 插值到 [0.0, 0.25]，叠加到 `Contamination.purge_rate`。
pub fn purge_rate_bonus(effective_lv: u8) -> f32 {
    interp(
        effective_lv,
        &[
            (0, 0.00),
            (1, 0.02),
            (3, 0.05),
            (5, 0.10),
            (7, 0.15),
            (10, 0.25),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// plan §7.2 锚点：五个桶精确数值。
    #[test]
    fn xp_for_bucket_matches_plan_section_seven_two() {
        assert_eq!(xp_for_bucket(OutcomeBucket::Perfect), 6);
        assert_eq!(xp_for_bucket(OutcomeBucket::Good), 3);
        assert_eq!(xp_for_bucket(OutcomeBucket::Flawed), 2);
        assert_eq!(xp_for_bucket(OutcomeBucket::Explode), 1);
        assert_eq!(xp_for_bucket(OutcomeBucket::Waste), 0);
        assert_eq!(LEARN_RECIPE_XP, 1);
    }

    /// plan §6.2 火候容差锚点：端点值必须精确命中。
    #[test]
    fn tolerance_scale_matches_plan_endpoints() {
        let eps = 1e-5_f32;
        assert!((tolerance_scale(0) - 1.00).abs() < eps);
        assert!((tolerance_scale(1) - 1.05).abs() < eps);
        assert!((tolerance_scale(3) - 1.15).abs() < eps);
        assert!((tolerance_scale(5) - 1.25).abs() < eps);
        assert!((tolerance_scale(7) - 1.35).abs() < eps);
        assert!((tolerance_scale(10) - 1.50).abs() < eps);
        // 插值：Lv.2 应在 1.05 → 1.15 之间（大约 1.10）
        assert!((tolerance_scale(2) - 1.10).abs() < eps);
    }

    #[test]
    fn side_effect_weight_scale_endpoints_monotonic() {
        let eps = 1e-5_f32;
        assert!((side_effect_weight_scale(0) - 1.00).abs() < eps);
        assert!((side_effect_weight_scale(10) - 0.40).abs() < eps);
        // 单调递减
        assert!(side_effect_weight_scale(3) > side_effect_weight_scale(7));
    }

    #[test]
    fn purge_rate_bonus_endpoints() {
        let eps = 1e-5_f32;
        assert!((purge_rate_bonus(0) - 0.00).abs() < eps);
        assert!((purge_rate_bonus(10) - 0.25).abs() < eps);
        // 插值：Lv.2 应在 0.02 → 0.05 之间（大约 0.035）
        assert!((purge_rate_bonus(2) - 0.035).abs() < eps);
    }
}
