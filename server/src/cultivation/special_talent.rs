//! 特殊顿悟天赋池：无法用通用 stat/op/value 表达的硬编码效果。

use super::color_affinity::opposite_color;
use super::components::ColorKind;
use super::generic_talent::color_kind_to_chinese;
use super::insight::{InsightAlignment, InsightCost, InsightEffect, InsightTradeoff};

pub fn special_converge_pool(color: ColorKind) -> Vec<InsightTradeoff> {
    vec![tradeoff(
        InsightAlignment::Converge,
        InsightEffect::UnlockPractice {
            name: format!("{}专精", color_kind_to_chinese(color)),
        },
        InsightCost::OppositeColorPenalty {
            color: opposite_color(color),
            penalty: 0.15,
        },
        format!(
            "你把{}之路认作自身骨相，解锁对应专精实践。",
            color_kind_to_chinese(color)
        ),
        format!(
            "{}之道渐远——对立色效率 -15%",
            color_kind_to_chinese(opposite_color(color))
        ),
        Some(color),
    )]
}

pub fn special_neutral_pool() -> Vec<InsightTradeoff> {
    vec![tradeoff(
        InsightAlignment::Neutral,
        InsightEffect::UnlockPerception {
            kind: "qi_color_trace".to_string(),
        },
        InsightCost::SenseExposure { add: 0.03 },
        "你能看见真元染色留下的细痕，也更容易被同类灵识察觉。".to_string(),
        "灵识外放——被感知暴露度 +3%".to_string(),
        None,
    )]
}

pub fn special_diverge_pool(color: ColorKind, target: ColorKind) -> Vec<InsightTradeoff> {
    vec![tradeoff(
        InsightAlignment::Diverge,
        InsightEffect::UnlockPractice {
            name: format!("{}试修", color_kind_to_chinese(target)),
        },
        InsightCost::MainColorPenalty {
            color,
            penalty: 0.10,
        },
        format!(
            "你临摹{}之理，开出一条不属于旧路的练法。",
            color_kind_to_chinese(target)
        ),
        format!("{}之忆淡去——主色效率 -10%", color_kind_to_chinese(color)),
        Some(target),
    )]
}

pub fn tradeoff(
    alignment: InsightAlignment,
    gain: InsightEffect,
    mut cost: InsightCost,
    gain_flavor: String,
    cost_flavor: String,
    target_color: Option<ColorKind>,
) -> InsightTradeoff {
    let gain_magnitude = gain.magnitude();
    let min_cost = (gain_magnitude * 0.5).max(0.01);
    if cost.magnitude() < min_cost {
        cost = amplify_cost(cost, min_cost);
    }
    let cost_magnitude = cost.magnitude();
    InsightTradeoff {
        alignment,
        gain,
        gain_magnitude,
        cost,
        cost_magnitude,
        gain_flavor,
        cost_flavor,
        target_color,
    }
}

fn amplify_cost(cost: InsightCost, required: f64) -> InsightCost {
    match cost {
        InsightCost::OppositeColorPenalty { color, .. } => InsightCost::OppositeColorPenalty {
            color,
            penalty: required,
        },
        InsightCost::QiVolatility { .. } => InsightCost::QiVolatility { add: required },
        InsightCost::ShockSensitivity { .. } => InsightCost::ShockSensitivity { add: required },
        InsightCost::MainColorPenalty { color, .. } => InsightCost::MainColorPenalty {
            color,
            penalty: required,
        },
        InsightCost::OverloadFragility { .. } => InsightCost::OverloadFragility { add: required },
        InsightCost::MeridianHealSlowdown { .. } => InsightCost::MeridianHealSlowdown {
            mul: (1.0 - required).clamp(0.85, 0.95),
        },
        InsightCost::BreakthroughFailurePenalty { .. } => InsightCost::BreakthroughFailurePenalty {
            mul: 1.0 + required,
        },
        InsightCost::SenseExposure { .. } => InsightCost::SenseExposure { add: required },
        InsightCost::ReactionWindowShrink { .. } => InsightCost::ReactionWindowShrink {
            mul: (1.0 - required).clamp(0.90, 0.97),
        },
        InsightCost::ChaoticToleranceLoss { .. } => {
            InsightCost::ChaoticToleranceLoss { sub: required }
        }
        InsightCost::VortexBurstDamageMul { .. } => InsightCost::VortexBurstDamageMul {
            mul: (1.0 - required).clamp(0.80, 0.98),
        },
    }
}
