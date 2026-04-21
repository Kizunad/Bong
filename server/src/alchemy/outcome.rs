//! 结果分桶（plan-alchemy-v1 §1.3）— 结束时按偏差分桶到 outcomes.{perfect,good,flawed,waste,explode}。
//!
//! 纯函数 + deterministic；side_effect_pool 抽取用显式 seed（便于测试）。

use crate::cultivation::components::ColorKind;

use super::recipe::{FireProfile, PillOutcome, Recipe, SideEffect};

/// 偏差汇总（每 tick 累积，结束时快照）。
#[derive(Debug, Clone, Default, Copy)]
pub struct DeviationSummary {
    /// 温度偏差累积（|cur - target| 积分，归一化到 tolerance.temp_band）。
    pub temp_deviation: f64,
    /// 时长偏差：abs(elapsed - target_duration) / tolerance.duration_band
    pub duration_deviation: f64,
    /// 是否错过中途投料窗口
    pub missed_stage: bool,
    /// qi 是否不足
    pub qi_deficit: bool,
    /// 过热（远超 target + tolerance.temp_band），可能触发炸炉
    pub severe_overheat: bool,
}

/// plan §1.3 偏差分桶逻辑（精确匹配路径）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutcomeBucket {
    Perfect,
    Good,
    Flawed,
    Waste,
    Explode,
}

/// 纯函数：把偏差映射到桶。阈值刻意简单，便于单测。
/// - severe_overheat → Explode
/// - qi_deficit → Waste（不炸，但毁材料）
/// - 完全在 tolerance 内 → Perfect
/// - 温度/时长其一溢出 1x tolerance → Good
/// - 溢出 2x tolerance 或 missed_stage → Flawed
/// - 溢出 3x tolerance → Waste
pub fn classify_precise(summary: &DeviationSummary) -> OutcomeBucket {
    if summary.severe_overheat {
        return OutcomeBucket::Explode;
    }
    if summary.qi_deficit {
        return OutcomeBucket::Waste;
    }
    let score = summary.temp_deviation.max(summary.duration_deviation);
    if summary.missed_stage {
        // 错过窗口 = 直接走 Flawed 或更差
        if score > 2.0 {
            return OutcomeBucket::Waste;
        }
        return OutcomeBucket::Flawed;
    }
    if score <= 1.0 {
        OutcomeBucket::Perfect
    } else if score <= 2.0 {
        OutcomeBucket::Good
    } else if score <= 3.0 {
        OutcomeBucket::Flawed
    } else {
        OutcomeBucket::Waste
    }
}

/// 残缺匹配路径产出（plan §1.3）：
/// - 丹效 ×(1 - missing_ratio * 0.7 clamp 0.3..0.6)
/// - toxin ×1.5
/// - 必抽一条 side_effect（由 seed 决定）
#[derive(Debug, Clone)]
pub struct FlawedResult {
    pub pill: String,
    pub quality: f64,
    pub toxin_amount: f64,
    pub toxin_color: ColorKind,
    pub side_effect: Option<SideEffect>,
}

/// 决定论抽取：按 weight 做加权选择，seed 决定落点。
pub fn pick_side_effect(pool: &[SideEffect], seed: u64) -> Option<SideEffect> {
    if pool.is_empty() {
        return None;
    }
    let total: u64 = pool.iter().map(|s| s.weight.max(1) as u64).sum();
    if total == 0 {
        return None;
    }
    let mut pick = seed % total;
    for s in pool {
        let w = s.weight.max(1) as u64;
        if pick < w {
            return Some(s.clone());
        }
        pick -= w;
    }
    pool.last().cloned()
}

/// 基于 base outcome 计算残缺版（注：base 是 recipe.outcomes.flawed 或构造的 default）。
pub fn build_flawed_result(
    recipe: &Recipe,
    base_toxin_color: ColorKind,
    missing_ratio: f64,
    seed: u64,
) -> Option<FlawedResult> {
    let fallback = recipe.flawed_fallback.as_ref()?;
    // missing_ratio: 0 → 丹效 ×0.6, 1 → 丹效 ×0.3（线性缩放到 [0.3, 0.6]）
    let eff_scale = (0.6 - missing_ratio * 0.3).clamp(0.3, 0.6);
    let base_quality = recipe
        .outcomes
        .flawed
        .as_ref()
        .map(|o| o.quality)
        .unwrap_or(0.4);
    let base_toxin = recipe
        .outcomes
        .flawed
        .as_ref()
        .map(|o| o.toxin_amount)
        .unwrap_or(0.6);
    let quality = base_quality * fallback.quality_scale * (eff_scale / 0.5);
    let toxin_amount = base_toxin * fallback.toxin_scale;
    let side = pick_side_effect(&fallback.side_effect_pool, seed);

    Some(FlawedResult {
        pill: fallback.pill.clone(),
        quality,
        toxin_amount,
        toxin_color: base_toxin_color,
        side_effect: side,
    })
}

/// 精确匹配路径 → 取桶对应的 PillOutcome（或 explode/waste 返回 None 让调用侧处理特例）。
pub fn pick_pill_outcome(recipe: &Recipe, bucket: OutcomeBucket) -> Option<&PillOutcome> {
    match bucket {
        OutcomeBucket::Perfect => recipe.outcomes.perfect.as_ref(),
        OutcomeBucket::Good => recipe.outcomes.good.as_ref(),
        OutcomeBucket::Flawed => recipe.outcomes.flawed.as_ref(),
        OutcomeBucket::Waste | OutcomeBucket::Explode => None,
    }
}

/// plan §1.3 累积 temp_deviation 的纯函数：给一组 (tick, temp) 轨迹和 profile，计算归一化偏差。
pub fn compute_temp_deviation(track: &[(u32, f64)], profile: &FireProfile) -> f64 {
    if track.is_empty() {
        return f64::INFINITY;
    }
    let band = profile.tolerance.temp_band.max(1e-6);
    let mut sum = 0.0;
    for (_, t) in track {
        let over = (t - profile.target_temp).abs();
        let norm = (over / band - 1.0).max(0.0); // 只在超出 tolerance 时计分
        sum += norm;
    }
    sum / track.len() as f64
}

pub fn compute_duration_deviation(elapsed: u32, profile: &FireProfile) -> f64 {
    let band = profile.tolerance.duration_band.max(1) as f64;
    let diff = (elapsed as i64 - profile.target_duration_ticks as i64).unsigned_abs() as f64;
    (diff / band - 1.0).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_profile() -> FireProfile {
        FireProfile {
            target_temp: 0.5,
            target_duration_ticks: 100,
            qi_cost: 10.0,
            tolerance: crate::alchemy::recipe::ToleranceSpec {
                temp_band: 0.1,
                duration_band: 10,
            },
        }
    }

    #[test]
    fn classify_perfect_within_tolerance() {
        let s = DeviationSummary::default();
        assert_eq!(classify_precise(&s), OutcomeBucket::Perfect);
    }

    #[test]
    fn classify_good_when_single_overflow() {
        let s = DeviationSummary {
            temp_deviation: 1.5,
            ..Default::default()
        };
        assert_eq!(classify_precise(&s), OutcomeBucket::Good);
    }

    #[test]
    fn classify_flawed_when_double_overflow() {
        let s = DeviationSummary {
            duration_deviation: 2.5,
            ..Default::default()
        };
        assert_eq!(classify_precise(&s), OutcomeBucket::Flawed);
    }

    #[test]
    fn classify_waste_when_severe() {
        let s = DeviationSummary {
            temp_deviation: 5.0,
            ..Default::default()
        };
        assert_eq!(classify_precise(&s), OutcomeBucket::Waste);
    }

    #[test]
    fn classify_explode_on_severe_overheat() {
        let s = DeviationSummary {
            severe_overheat: true,
            ..Default::default()
        };
        assert_eq!(classify_precise(&s), OutcomeBucket::Explode);
    }

    #[test]
    fn classify_waste_on_qi_deficit_even_if_temp_is_fine() {
        let s = DeviationSummary {
            qi_deficit: true,
            ..Default::default()
        };
        assert_eq!(classify_precise(&s), OutcomeBucket::Waste);
    }

    #[test]
    fn classify_missed_stage_goes_flawed() {
        let s = DeviationSummary {
            missed_stage: true,
            ..Default::default()
        };
        assert_eq!(classify_precise(&s), OutcomeBucket::Flawed);
    }

    #[test]
    fn pick_side_effect_deterministic_with_seed() {
        let pool = vec![
            SideEffect {
                tag: "a".into(),
                duration_s: 0,
                weight: 3,
                perm: false,
                color: None,
                amount: None,
            },
            SideEffect {
                tag: "b".into(),
                duration_s: 0,
                weight: 1,
                perm: false,
                color: None,
                amount: None,
            },
        ];
        // seed=0 → pick=0, weight_cum 3 → "a"
        assert_eq!(pick_side_effect(&pool, 0).unwrap().tag, "a");
        // seed=3 → pick=3, rolls over to "b"
        assert_eq!(pick_side_effect(&pool, 3).unwrap().tag, "b");
    }

    #[test]
    fn pick_side_effect_empty_returns_none() {
        assert!(pick_side_effect(&[], 42).is_none());
    }

    #[test]
    fn compute_temp_deviation_zero_when_on_target() {
        let track: Vec<(u32, f64)> = (0..10).map(|i| (i, 0.5)).collect();
        assert_eq!(compute_temp_deviation(&track, &mk_profile()), 0.0);
    }

    #[test]
    fn compute_temp_deviation_positive_when_out_of_band() {
        let track: Vec<(u32, f64)> = (0..10).map(|i| (i, 0.8)).collect(); // 偏 0.3, band 0.1 → norm 2.0
        let d = compute_temp_deviation(&track, &mk_profile());
        assert!((d - 2.0).abs() < 1e-9);
    }

    #[test]
    fn compute_duration_deviation_band_normalized() {
        let p = mk_profile(); // target=100, band=10
        assert_eq!(compute_duration_deviation(100, &p), 0.0);
        assert_eq!(compute_duration_deviation(110, &p), 0.0);
        let d = compute_duration_deviation(130, &p);
        // diff=30, diff/band=3, minus 1 → 2.0
        assert!((d - 2.0).abs() < 1e-9);
    }
}
