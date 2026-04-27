//! plan-skill-v1 §10 botany 钩子 —— pure helper 层。
//!
//! botany 当前仍使用连续 `spirit_quality: f64` 表示掉落品相，尚未引入独立四档品阶结构。
//! 这里先把 plan §6.1 的四档分布偏移投影为连续品质 bonus，保证 herbalism 已经实打实
//! 影响野外采集结果，并与现有 inventory schema 保持一致。

use crate::skill::curve::interp;

const BASE_QUALITY_DISTRIBUTION: [f32; 4] = [20.0, 30.0, 40.0, 10.0];
const EXTRA_EXTREME_BONUS_POINTS_AT_LV10: f32 = 5.0;

/// plan §6.1 herbalism 品质偏移，按关键 Lv 线性插值，并向下取整到整百分点，
/// 对齐文档中的 `Lv.4 -> +12%` 示例。
pub fn quality_bias_points(effective_lv: u8) -> f32 {
    interp(
        effective_lv,
        &[
            (0, 0.0),
            (1, 5.0),
            (3, 10.0),
            (5, 15.0),
            (7, 20.0),
            (10, 30.0),
        ],
    )
    .floor()
}

/// 按 plan §6.1 基准四档 `[劣/普/良/极]` 计算 skill 修饰后的分布。
pub fn adjusted_quality_distribution(effective_lv: u8) -> [f32; 4] {
    let bias = quality_bias_points(effective_lv);
    let mut poor = (BASE_QUALITY_DISTRIBUTION[0] - bias * 0.6).max(0.0);
    let mut normal = (BASE_QUALITY_DISTRIBUTION[1] - bias * 0.3).max(0.0);
    let good = BASE_QUALITY_DISTRIBUTION[2] + bias * 0.7;
    let mut extreme = BASE_QUALITY_DISTRIBUTION[3] + bias * 0.2;

    if effective_lv >= 10 {
        // Lv.10 额外 +5% 极：先尽量均分从劣/普扣，某一档不够时由另一档补足剩余。
        let poor_take = poor.min(EXTRA_EXTREME_BONUS_POINTS_AT_LV10 / 2.0);
        poor -= poor_take;
        let normal_take = normal.min(EXTRA_EXTREME_BONUS_POINTS_AT_LV10 - poor_take);
        normal -= normal_take;
        extreme += poor_take + normal_take;
    }

    [poor, normal, good, extreme]
}

pub fn spirit_quality_bonus(effective_lv: u8) -> f64 {
    let base = expected_quality_score(BASE_QUALITY_DISTRIBUTION);
    let adjusted = expected_quality_score(adjusted_quality_distribution(effective_lv));
    f64::from((adjusted - base).max(0.0))
}

fn expected_quality_score(distribution: [f32; 4]) -> f32 {
    (distribution[1] / 3.0 + distribution[2] * (2.0 / 3.0) + distribution[3]) / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn herbalism_lv4_distribution_matches_plan_example() {
        let [poor, normal, good, extreme] = adjusted_quality_distribution(4);
        let eps = 1e-5_f32;

        assert!((poor - 12.8).abs() < eps);
        assert!((normal - 26.4).abs() < eps);
        assert!((good - 48.4).abs() < eps);
        assert!((extreme - 12.4).abs() < eps);
    }

    #[test]
    fn lv10_distribution_adds_extra_extreme_bonus_after_bias_shift() {
        let [poor, normal, good, extreme] = adjusted_quality_distribution(10);
        let eps = 1e-5_f32;

        assert!((poor - 0.0).abs() < eps);
        assert!((normal - 18.0).abs() < eps);
        assert!((good - 61.0).abs() < eps);
        assert!((extreme - 21.0).abs() < eps);
    }

    #[test]
    fn spirit_quality_bonus_tracks_distribution_uplift() {
        let eps = 1e-6_f64;
        assert!((spirit_quality_bonus(0) - 0.0).abs() < eps);
        assert!((spirit_quality_bonus(3) - 0.056_666_67).abs() < eps);
        assert!((spirit_quality_bonus(10) - 0.21).abs() < eps);
    }
}
