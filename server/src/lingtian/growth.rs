//! plan-lingtian-v1 §1.3 — 生长 / 灵气消耗 / 区域漏吸（混合模型）。
//!
//! 纯函数层：单 lingtian-tick 推一个 plot 一步，无 ECS 依赖。`systems.rs`
//! 负责按 1 lingtian-tick = 1200 Bevy tick（= 60s @ 20tps，plan §4 LingtianTick）
//! 周期调用本模块。
//!
//! 公式（plan §1.3 伪码）：
//! ```text
//! base_drain = crop.kind.growth_cost.drain_per_tick()
//! per_tick   = 1.0 / crop.kind.growth_duration_ticks   // 让 multiplier=1 时刚好满期成熟
//!
//! 若 plot_qi >= base_drain:
//!     growth      += per_tick × quality_multiplier(plot_qi / cap)
//!     quality_acc += quality_bonus(plot_qi / cap)
//!     plot_qi     -= base_drain
//! 否则若 zone_qi >= base_drain × ZONE_LEAK_RATIO:
//!     growth      += per_tick × ZONE_LEAK_GROWTH_FACTOR (0.3)
//!     zone_qi     -= base_drain × ZONE_LEAK_RATIO
//!     quality_acc 不增（漏吸不带品质增益）
//! 否则：
//!     growth 停滞
//! ```

use crate::botany::PlantKind;

use super::plot::LingtianPlot;

/// 区域漏吸比例：plot_qi 不足时按本系数从 zone qi 抽 base_drain × ratio。
pub const ZONE_LEAK_RATIO: f32 = 0.2;

/// 区域漏吸状态下生长速率衰减（plan §1.3 注释 "30%"）。
pub const ZONE_LEAK_GROWTH_FACTOR: f32 = 0.3;

/// plan §1.3 quality_multiplier — plot_qi 越满成长越快，封顶 1.5。
///
/// 设计选择：分段线性。0.0 → 0.8（贫，仍长但慢）；0.5 → 1.0（基线）；1.0 → 1.5（丰沛）。
pub fn quality_multiplier(qi_ratio: f32) -> f32 {
    let r = qi_ratio.clamp(0.0, 1.0);
    if r <= 0.5 {
        0.8 + 0.4 * r
    } else {
        1.0 + (r - 0.5)
    }
}

/// quality_accum 增量：plot_qi/cap >= 0.9 视为"丰沛期"，每 tick +0.001。
pub const FENGPEI_THRESHOLD: f32 = 0.9;
pub const FENGPEI_QUALITY_BONUS: f32 = 0.001;

pub fn quality_bonus(qi_ratio: f32) -> f32 {
    if qi_ratio.clamp(0.0, 1.0) >= FENGPEI_THRESHOLD {
        FENGPEI_QUALITY_BONUS
    } else {
        0.0
    }
}

/// 单 lingtian-tick 一步推进的结果。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GrowthOutcome {
    /// 走"plot_qi 充足"分支，plot_qi 减、growth 增。
    Grew {
        delta_growth: f32,
        plot_qi_consumed: f32,
    },
    /// 走"区域漏吸"分支，zone qi 减、growth 微增。
    LeakedFromZone {
        delta_growth: f32,
        zone_qi_consumed: f32,
    },
    /// plot_qi 与 zone qi 双双不够 → growth 停滞。
    Stalled,
    /// plot 没有 crop（空田，不应被推进；调用方过滤）。
    NoCrop,
}

impl GrowthOutcome {
    pub fn delta_growth(&self) -> f32 {
        match self {
            Self::Grew { delta_growth, .. } | Self::LeakedFromZone { delta_growth, .. } => {
                *delta_growth
            }
            Self::Stalled | Self::NoCrop => 0.0,
        }
    }
}

/// 推一个 plot 一步。修改 `plot.crop.growth` / `plot.crop.quality_accum` /
/// `plot.plot_qi` / `zone_qi`。返回 outcome 用于上层日志 / 事件。
pub fn advance_one_lingtian_tick(
    plot: &mut LingtianPlot,
    kind: &PlantKind,
    zone_qi: &mut f32,
) -> GrowthOutcome {
    let contamination_multiplier = plot.contamination_quality_multiplier();
    let Some(crop) = plot.crop.as_mut() else {
        return GrowthOutcome::NoCrop;
    };
    if crop.is_ripe() {
        // 已熟；不再推进（让上层 harvest 流程接手）。
        return GrowthOutcome::Grew {
            delta_growth: 0.0,
            plot_qi_consumed: 0.0,
        };
    }

    let base_drain = kind.growth_cost.drain_per_tick();
    let duration = kind.growth_duration_ticks.max(1) as f32;
    let per_tick = 1.0 / duration;
    let qi_ratio = if plot.plot_qi_cap > 0.0 {
        plot.plot_qi / plot.plot_qi_cap
    } else {
        0.0
    };

    if plot.plot_qi >= base_drain {
        let mult = quality_multiplier(qi_ratio);
        let bonus = quality_bonus(qi_ratio) * contamination_multiplier;
        let delta = per_tick * mult;
        crop.growth = (crop.growth + delta).min(1.0);
        crop.quality_accum += bonus;
        plot.plot_qi -= base_drain;
        return GrowthOutcome::Grew {
            delta_growth: delta,
            plot_qi_consumed: base_drain,
        };
    }

    let leak_demand = base_drain * ZONE_LEAK_RATIO;
    if *zone_qi >= leak_demand {
        let delta = per_tick * ZONE_LEAK_GROWTH_FACTOR;
        crop.growth = (crop.growth + delta).min(1.0);
        *zone_qi -= leak_demand;
        return GrowthOutcome::LeakedFromZone {
            delta_growth: delta,
            zone_qi_consumed: leak_demand,
        };
    }

    GrowthOutcome::Stalled
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::botany::{GrowthCost, PlantKind, PlantRarity};
    use crate::lingtian::plot::CropInstance;
    use valence::prelude::BlockPos;

    fn ci_she_hao() -> PlantKind {
        PlantKind {
            id: "ci_she_hao".into(),
            display_name: "刺舌蒿".into(),
            cultivable: true,
            growth_cost: GrowthCost::Low,
            growth_duration_ticks: 480,
            rarity: PlantRarity::Common,
            description: String::new(),
        }
    }

    fn planted_plot(plot_qi: f32) -> LingtianPlot {
        let mut p = LingtianPlot::new(BlockPos::new(0, 64, 0), None);
        p.plot_qi = plot_qi;
        p.crop = Some(CropInstance::new("ci_she_hao".into()));
        p
    }

    #[test]
    fn quality_multiplier_endpoints() {
        assert!((quality_multiplier(0.0) - 0.8).abs() < 1e-6);
        assert!((quality_multiplier(0.5) - 1.0).abs() < 1e-6);
        assert!((quality_multiplier(1.0) - 1.5).abs() < 1e-6);
        // clamp 边界
        assert!((quality_multiplier(-0.5) - 0.8).abs() < 1e-6);
        assert!((quality_multiplier(2.0) - 1.5).abs() < 1e-6);
    }

    #[test]
    fn quality_bonus_only_at_fengpei() {
        assert_eq!(quality_bonus(0.0), 0.0);
        assert_eq!(quality_bonus(0.89), 0.0);
        assert_eq!(quality_bonus(0.9), FENGPEI_QUALITY_BONUS);
        assert_eq!(quality_bonus(1.0), FENGPEI_QUALITY_BONUS);
    }

    #[test]
    fn ci_she_hao_grows_when_plot_qi_full() {
        let kind = ci_she_hao();
        let mut plot = planted_plot(1.0);
        let mut zone_qi = 0.0; // 不需要 zone
        let out = advance_one_lingtian_tick(&mut plot, &kind, &mut zone_qi);
        match out {
            GrowthOutcome::Grew {
                delta_growth,
                plot_qi_consumed,
            } => {
                // duration=480 → per_tick = 1/480；mult @ ratio=1.0 → 1.5
                let expected_delta = (1.0_f32 / 480.0) * 1.5;
                assert!((delta_growth - expected_delta).abs() < 1e-6);
                assert_eq!(plot_qi_consumed, GrowthCost::Low.drain_per_tick());
            }
            other => panic!("expected Grew, got {other:?}"),
        }
        assert!(plot.plot_qi < 1.0);
        assert!(plot.crop.as_ref().unwrap().growth > 0.0);
        // 丰沛期累积 quality
        assert!((plot.crop.as_ref().unwrap().quality_accum - FENGPEI_QUALITY_BONUS).abs() < 1e-6);
    }

    #[test]
    fn falls_back_to_zone_leak_when_plot_qi_low() {
        let kind = ci_she_hao();
        let mut plot = planted_plot(0.0); // plot 全干
        let mut zone_qi = 1.0;
        let out = advance_one_lingtian_tick(&mut plot, &kind, &mut zone_qi);
        match out {
            GrowthOutcome::LeakedFromZone {
                delta_growth,
                zone_qi_consumed,
            } => {
                let per_tick = 1.0_f32 / 480.0;
                let expected_delta = per_tick * ZONE_LEAK_GROWTH_FACTOR;
                assert!((delta_growth - expected_delta).abs() < 1e-6);
                assert!(
                    (zone_qi_consumed - GrowthCost::Low.drain_per_tick() * ZONE_LEAK_RATIO).abs()
                        < 1e-6
                );
            }
            other => panic!("expected LeakedFromZone, got {other:?}"),
        }
        assert!(zone_qi < 1.0);
        assert_eq!(plot.plot_qi, 0.0);
        // 漏吸不增 quality
        assert_eq!(plot.crop.as_ref().unwrap().quality_accum, 0.0);
    }

    #[test]
    fn stalls_when_both_pools_empty() {
        let kind = ci_she_hao();
        let mut plot = planted_plot(0.0);
        let mut zone_qi = 0.0;
        let before_growth = plot.crop.as_ref().unwrap().growth;
        let out = advance_one_lingtian_tick(&mut plot, &kind, &mut zone_qi);
        assert_eq!(out, GrowthOutcome::Stalled);
        assert_eq!(plot.crop.as_ref().unwrap().growth, before_growth);
    }

    #[test]
    fn empty_plot_returns_no_crop() {
        let kind = ci_she_hao();
        let mut plot = LingtianPlot::new(BlockPos::new(0, 64, 0), None);
        let mut zone_qi = 1.0;
        let out = advance_one_lingtian_tick(&mut plot, &kind, &mut zone_qi);
        assert_eq!(out, GrowthOutcome::NoCrop);
    }

    #[test]
    fn ripe_crop_does_not_advance_growth_further() {
        let kind = ci_she_hao();
        let mut plot = planted_plot(1.0);
        plot.crop.as_mut().unwrap().growth = 1.0;
        let _ = advance_one_lingtian_tick(&mut plot, &kind, &mut 0.0);
        assert_eq!(plot.crop.as_ref().unwrap().growth, 1.0);
    }

    #[test]
    fn ci_she_hao_ripens_in_480_ticks_at_baseline_qi_ratio_05() {
        // baseline mult = 1.0（ratio=0.5 → quality_multiplier = 1.0）
        // per_tick = 1/480；走 480 tick 应 = 1.0 → 熟
        let kind = ci_she_hao();
        let mut plot = planted_plot(1000.0); // 假设无穷 plot_qi（不会枯）
        plot.plot_qi_cap = 2000.0;
        plot.plot_qi = 1000.0; // ratio 0.5 → mult 1.0
        let mut zone_qi = 0.0;
        for _ in 0..kind.growth_duration_ticks {
            // 维持 ratio = 0.5：补回 plot_qi 让模拟 baseline
            plot.plot_qi = 1000.0;
            advance_one_lingtian_tick(&mut plot, &kind, &mut zone_qi);
        }
        let crop = plot.crop.as_ref().unwrap();
        assert!(crop.is_ripe(), "growth = {}", crop.growth);
    }
}
