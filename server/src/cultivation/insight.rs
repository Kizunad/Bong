//! 顿悟系统（plan §5）— 核心数据结构、7 类白名单、quota、触发点。
//!
//! 子模块 `insight_fallback` 提供静态选项池（agent 失败兜底）。
//! `insight_apply` 负责将选中的效果应用到 Cultivation/MeridianSystem/QiColor/LifeRecord。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use valence::prelude::{bevy_ecs, Component, Event, Resource};

use super::components::{ColorKind, MeridianId, Realm};

/// 7 类顿悟类别（plan §5.2 A-G）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InsightCategory {
    Meridian,     // A
    Qi,           // B
    Composure,    // C
    Coloring,     // D（对齐 TS schema InsightCategory 字面量）
    Breakthrough, // E
    Style,        // F（对齐 TS schema InsightCategory 字面量）
    Perception,   // G
}

impl InsightCategory {
    /// 单次效果幅度上限（plan §5.2 括号内数值）。
    pub fn single_cap(self) -> f64 {
        match self {
            InsightCategory::Meridian => 0.05,
            InsightCategory::Qi => 0.05,
            InsightCategory::Composure => 0.10,
            InsightCategory::Coloring => 0.05,
            InsightCategory::Breakthrough => 0.05,
            InsightCategory::Style => 0.15,     // 流派类效果更大
            InsightCategory::Perception => 1.0, // 解锁类无幅度
        }
    }

    /// 同类累计上限。
    pub fn cumulative_cap(self) -> f64 {
        match self {
            InsightCategory::Meridian => 0.20,
            InsightCategory::Qi => 0.25,
            InsightCategory::Composure => 0.30,
            InsightCategory::Coloring => 0.15,
            InsightCategory::Breakthrough => 0.30,
            InsightCategory::Style => 1.00,
            InsightCategory::Perception => f64::INFINITY,
        }
    }
}

/// 各境界的顿悟额度（plan §5.3）。
pub fn realm_quota(r: Realm) -> u8 {
    match r {
        Realm::Awaken => 1,
        Realm::Induce => 2,
        Realm::Condense => 3,
        Realm::Solidify => 4,
        Realm::Spirit => 5,
        Realm::Void => 6,
    }
}

/// 具体效果 payload（plan §5.2 白名单子项）。
/// 使用枚举列出所有合法变体，Arbiter 校验即对应 match。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum InsightEffect {
    // A 经脉类
    MeridianRate {
        id: MeridianId,
        mul: f64,
    },
    MeridianForgeDiscount {
        id: MeridianId,
        discount: f64,
    },
    MeridianOverloadTolerance {
        id: MeridianId,
        add: f64,
    },
    // B 真元类
    QiRegenFactor {
        mul: f64,
    },
    PurgeEfficiency {
        color: ColorKind,
        mul: f64,
    },
    UnfreezeQiMax {
        mul: f64,
    },
    // C 心境类
    ComposureRecover {
        mul: f64,
    },
    ComposureShockDiscount {
        event: String,
        mul: f64,
    },
    ComposureImmuneDuringBreakthrough,
    // D 染色类
    ColorCapAdd {
        color: ColorKind,
        add: f64,
    },
    ChaoticTolerance {
        add: f64,
    },
    HunyuanThreshold {
        mul: f64,
    },
    // E 突破类
    NextBreakthroughBonus {
        add: f64,
    },
    BreakthroughEventConditionDrop {
        realm: Realm,
    },
    TribulationPredictionWindow,
    // F 流派类
    DualForgeDiscount {
        id: MeridianId,
        mul: f64,
    },
    ColorMaterialAffinity {
        color: ColorKind,
        material: String,
        add: f64,
    },
    ZhenfaConcealment {
        add: f64,
    },
    ZhenfaDisenchant {
        add: f64,
    },
    UnlockPractice {
        name: String,
    },
    // G 感知类
    UnlockPerception {
        kind: String,
    },
    LifespanExtensionEnlightenment,
}

impl InsightEffect {
    pub fn category(&self) -> InsightCategory {
        use InsightEffect::*;
        match self {
            MeridianRate { .. }
            | MeridianForgeDiscount { .. }
            | MeridianOverloadTolerance { .. } => InsightCategory::Meridian,
            QiRegenFactor { .. } | PurgeEfficiency { .. } | UnfreezeQiMax { .. } => {
                InsightCategory::Qi
            }
            ComposureRecover { .. }
            | ComposureShockDiscount { .. }
            | ComposureImmuneDuringBreakthrough => InsightCategory::Composure,
            ColorCapAdd { .. } | ChaoticTolerance { .. } | HunyuanThreshold { .. } => {
                InsightCategory::Coloring
            }
            NextBreakthroughBonus { .. }
            | BreakthroughEventConditionDrop { .. }
            | TribulationPredictionWindow => InsightCategory::Breakthrough,
            DualForgeDiscount { .. }
            | ColorMaterialAffinity { .. }
            | ZhenfaConcealment { .. }
            | ZhenfaDisenchant { .. }
            | UnlockPractice { .. } => InsightCategory::Style,
            UnlockPerception { .. } | LifespanExtensionEnlightenment => InsightCategory::Perception,
        }
    }

    /// 返回此效果的 magnitude（用于累计上限校验）。
    pub fn magnitude(&self) -> f64 {
        use InsightEffect::*;
        match self {
            MeridianRate { mul, .. }
            | QiRegenFactor { mul }
            | PurgeEfficiency { mul, .. }
            | HunyuanThreshold { mul }
            | ComposureRecover { mul }
            | DualForgeDiscount { mul, .. } => (mul - 1.0).abs(),
            MeridianForgeDiscount { discount, .. } => *discount,
            MeridianOverloadTolerance { add, .. }
            | ChaoticTolerance { add }
            | NextBreakthroughBonus { add }
            | ColorCapAdd { add, .. }
            | ColorMaterialAffinity { add, .. }
            | ZhenfaConcealment { add }
            | ZhenfaDisenchant { add } => *add,
            UnfreezeQiMax { mul } => (1.0 - mul).abs(),
            ComposureShockDiscount { mul, .. } => (1.0 - mul).abs(),
            ComposureImmuneDuringBreakthrough
            | TribulationPredictionWindow
            | UnlockPractice { .. }
            | UnlockPerception { .. }
            | LifespanExtensionEnlightenment
            | BreakthroughEventConditionDrop { .. } => 0.0,
        }
    }
}

/// 顿悟三轨：靠近当前色谱 / 中性安全牌 / 远离旧路。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InsightAlignment {
    Converge,
    Neutral,
    Diverge,
}

impl InsightAlignment {
    pub fn code(self) -> &'static str {
        match self {
            Self::Converge => "converge",
            Self::Neutral => "neutral",
            Self::Diverge => "diverge",
        }
    }
}

fn default_alignment() -> InsightAlignment {
    InsightAlignment::Neutral
}

fn default_cost() -> InsightCost {
    InsightCost::QiVolatility { add: 0.015 }
}

fn default_cost_magnitude() -> f64 {
    0.015
}

/// 顿悟代价。每个选项必须同时携带 gain + cost，不能再出现纯增益。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum InsightCost {
    OppositeColorPenalty { color: ColorKind, penalty: f64 },
    QiVolatility { add: f64 },
    ShockSensitivity { add: f64 },
    MainColorPenalty { color: ColorKind, penalty: f64 },
    OverloadFragility { add: f64 },
    MeridianHealSlowdown { mul: f64 },
    BreakthroughFailurePenalty { mul: f64 },
    SenseExposure { add: f64 },
    ReactionWindowShrink { mul: f64 },
    ChaoticToleranceLoss { sub: f64 },
}

impl InsightCost {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::OppositeColorPenalty { .. } => "opposite_color_penalty",
            Self::QiVolatility { .. } => "qi_volatility",
            Self::ShockSensitivity { .. } => "shock_sensitivity",
            Self::MainColorPenalty { .. } => "main_color_penalty",
            Self::OverloadFragility { .. } => "overload_fragility",
            Self::MeridianHealSlowdown { .. } => "meridian_heal_slowdown",
            Self::BreakthroughFailurePenalty { .. } => "breakthrough_failure_penalty",
            Self::SenseExposure { .. } => "sense_exposure",
            Self::ReactionWindowShrink { .. } => "reaction_window_shrink",
            Self::ChaoticToleranceLoss { .. } => "chaotic_tolerance_loss",
        }
    }

    pub fn magnitude(&self) -> f64 {
        match self {
            Self::OppositeColorPenalty { penalty, .. } | Self::MainColorPenalty { penalty, .. } => {
                *penalty
            }
            Self::QiVolatility { add }
            | Self::ShockSensitivity { add }
            | Self::OverloadFragility { add }
            | Self::SenseExposure { add } => *add,
            Self::MeridianHealSlowdown { mul } | Self::ReactionWindowShrink { mul } => {
                (1.0 - *mul).abs()
            }
            Self::BreakthroughFailurePenalty { mul } => (*mul - 1.0).abs(),
            Self::ChaoticToleranceLoss { sub } => *sub,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsightTradeoff {
    pub alignment: InsightAlignment,
    pub gain: InsightEffect,
    pub gain_magnitude: f64,
    pub cost: InsightCost,
    pub cost_magnitude: f64,
    pub gain_flavor: String,
    pub cost_flavor: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_color: Option<ColorKind>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsightChoice {
    pub category: InsightCategory,
    #[serde(default = "default_alignment")]
    pub alignment: InsightAlignment,
    pub effect: InsightEffect,
    pub flavor: String,
    #[serde(default = "default_cost")]
    pub cost: InsightCost,
    #[serde(default = "default_cost_magnitude")]
    pub cost_magnitude: f64,
    #[serde(default)]
    pub cost_flavor: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_color: Option<ColorKind>,
}

impl InsightChoice {
    pub fn from_tradeoff(tradeoff: InsightTradeoff) -> Self {
        Self {
            category: tradeoff.gain.category(),
            alignment: tradeoff.alignment,
            effect: tradeoff.gain,
            flavor: tradeoff.gain_flavor,
            cost: tradeoff.cost,
            cost_magnitude: tradeoff.cost_magnitude,
            cost_flavor: tradeoff.cost_flavor,
            target_color: tradeoff.target_color,
        }
    }

    pub fn neutral(
        category: InsightCategory,
        effect: InsightEffect,
        flavor: impl Into<String>,
    ) -> Self {
        let cost = InsightCost::QiVolatility { add: 0.015 };
        Self {
            category,
            alignment: InsightAlignment::Neutral,
            effect,
            flavor: flavor.into(),
            cost_magnitude: cost.magnitude(),
            cost,
            cost_flavor: "灵气更活，战斗中真元挥发加速。".to_string(),
            target_color: None,
        }
    }
}

/// 顿悟请求事件（agent 消费）。
#[derive(Debug, Clone, Event)]
pub struct InsightRequest {
    pub entity: valence::prelude::Entity,
    pub trigger_id: String,
    pub realm: Realm,
}

/// 顿悟 Offer（agent 或 fallback 生成，发给客户端）。
#[derive(Debug, Clone, Event)]
pub struct InsightOffer {
    pub entity: valence::prelude::Entity,
    pub trigger_id: String,
    pub choices: Vec<InsightChoice>,
}

/// 玩家选择结果。
#[derive(Debug, Clone, Event)]
pub struct InsightChosen {
    pub entity: valence::prelude::Entity,
    pub trigger_id: String,
    pub choice_idx: Option<usize>, // None = 拒绝
}

/// 每玩家顿悟额度追踪。per-realm quota 消耗 + 累计效果幅度（防超 cumulative_cap）。
#[derive(Debug, Clone, Default, Component, Serialize, Deserialize)]
pub struct InsightQuota {
    /// 当前境界已用额度。
    pub used_this_realm: u8,
    /// 按类别累积的效果幅度（持久，不随境界刷新）。
    pub cumulative: HashMap<InsightCategory, f64>,
    /// 一次性触发 ID 记录（每境界 1 次等）。
    pub fired_triggers: Vec<String>,
}

impl InsightQuota {
    /// 境界突破时重置当前境界用量。
    pub fn reset_for_realm(&mut self) {
        self.used_this_realm = 0;
    }

    /// 此触发是否已达额度或重复？
    pub fn has_quota(&self, realm: Realm) -> bool {
        self.used_this_realm < realm_quota(realm)
    }

    pub fn apply_accumulation(&mut self, choice: &InsightChoice) {
        *self.cumulative.entry(choice.category).or_insert(0.0) += choice.effect.magnitude();
        self.used_this_realm = self.used_this_realm.saturating_add(1);
    }
}

/// 顿悟触发点登记（plan §5.4）— 可由 tick / breakthrough / forge 系统调用。
#[derive(Debug, Default, Resource)]
pub struct InsightTriggerRegistry {
    pub known_triggers: Vec<&'static str>,
}

impl InsightTriggerRegistry {
    pub fn with_defaults() -> Self {
        Self {
            known_triggers: vec![
                "first_breakthrough_to_Induce",
                "first_breakthrough_to_Condense",
                "first_breakthrough_to_Solidify",
                "first_breakthrough_to_Spirit",
                "first_breakthrough_to_Void",
                "breakthrough_failed_recovered",
                "meridian_forge_tier_milestone",
                "first_tribulation_survived",
                "witnessed_xuhua_tribulation",
                "survived_negative_zone",
                "practice_dedication_milestone",
                "chaotic_to_hunyuan_pivot",
                "killed_higher_realm",
                "killed_by_higher_realm_survived",
                "post_rebirth_clarity",
                "wind_candle_lifespan_extension",
            ],
        }
    }
}

/// Arbiter 校验（plan §5.5）— 白名单 + 单次/累计上限 + 引用合法性。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ArbiterError {
    SingleCapExceeded,
    CumulativeCapExceeded,
    QuotaExhausted,
    MissingCost,
    CostTooSmall,
}

pub fn validate_offer(
    quota: &InsightQuota,
    choice: &InsightChoice,
    realm: Realm,
) -> Result<(), ArbiterError> {
    if !quota.has_quota(realm) {
        return Err(ArbiterError::QuotaExhausted);
    }
    let cat = choice.effect.category();
    let mag = choice.effect.magnitude();
    if mag > cat.single_cap() + 1e-9 {
        return Err(ArbiterError::SingleCapExceeded);
    }
    let accumulated = quota.cumulative.get(&cat).copied().unwrap_or(0.0);
    if accumulated + mag > cat.cumulative_cap() + 1e-9 {
        return Err(ArbiterError::CumulativeCapExceeded);
    }
    if choice.cost_magnitude <= 0.0 || choice.cost.magnitude() <= 0.0 {
        return Err(ArbiterError::MissingCost);
    }
    if choice.cost_magnitude + 1e-9 < mag * 0.5 {
        return Err(ArbiterError::CostTooSmall);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_cap_enforced() {
        let quota = InsightQuota::default();
        let bad = InsightChoice {
            category: InsightCategory::Meridian,
            alignment: InsightAlignment::Neutral,
            effect: InsightEffect::MeridianRate {
                id: MeridianId::Lung,
                mul: 1.50,
            },
            flavor: "".into(),
            cost: InsightCost::QiVolatility { add: 0.75 },
            cost_magnitude: 0.75,
            cost_flavor: "真元挥发加速。".into(),
            target_color: None,
        };
        assert_eq!(
            validate_offer(&quota, &bad, Realm::Awaken),
            Err(ArbiterError::SingleCapExceeded)
        );
    }

    #[test]
    fn cumulative_cap_enforced() {
        let mut quota = InsightQuota::default();
        quota.cumulative.insert(InsightCategory::Meridian, 0.19);
        // 已累积 0.19，上限 0.20；再 +0.05 超限
        let c = InsightChoice {
            category: InsightCategory::Meridian,
            alignment: InsightAlignment::Neutral,
            effect: InsightEffect::MeridianRate {
                id: MeridianId::Lung,
                mul: 1.05,
            },
            flavor: "".into(),
            cost: InsightCost::QiVolatility { add: 0.03 },
            cost_magnitude: 0.03,
            cost_flavor: "真元挥发加速。".into(),
            target_color: None,
        };
        assert_eq!(
            validate_offer(&quota, &c, Realm::Induce),
            Err(ArbiterError::CumulativeCapExceeded)
        );
    }

    #[test]
    fn quota_exhausted_blocks_all() {
        let quota = InsightQuota {
            used_this_realm: 1,
            ..Default::default()
        };
        let c = InsightChoice {
            category: InsightCategory::Meridian,
            alignment: InsightAlignment::Neutral,
            effect: InsightEffect::MeridianRate {
                id: MeridianId::Lung,
                mul: 1.05,
            },
            flavor: "".into(),
            cost: InsightCost::QiVolatility { add: 0.03 },
            cost_magnitude: 0.03,
            cost_flavor: "真元挥发加速。".into(),
            target_color: None,
        };
        assert_eq!(
            validate_offer(&quota, &c, Realm::Awaken),
            Err(ArbiterError::QuotaExhausted)
        );
    }

    #[test]
    fn quota_resets_on_breakthrough() {
        let mut q = InsightQuota {
            used_this_realm: 2,
            ..Default::default()
        };
        q.reset_for_realm();
        assert_eq!(q.used_this_realm, 0);
    }

    #[test]
    fn realm_quota_matches_plan() {
        assert_eq!(realm_quota(Realm::Awaken), 1);
        assert_eq!(realm_quota(Realm::Void), 6);
    }

    #[test]
    fn perception_has_infinite_cumulative() {
        assert!(InsightCategory::Perception.cumulative_cap().is_infinite());
    }

    #[test]
    fn pure_gain_is_rejected() {
        let quota = InsightQuota::default();
        let c = InsightChoice {
            category: InsightCategory::Qi,
            alignment: InsightAlignment::Neutral,
            effect: InsightEffect::QiRegenFactor { mul: 1.03 },
            flavor: "".into(),
            cost: InsightCost::QiVolatility { add: 0.0 },
            cost_magnitude: 0.0,
            cost_flavor: "".into(),
            target_color: None,
        };
        assert_eq!(
            validate_offer(&quota, &c, Realm::Induce),
            Err(ArbiterError::MissingCost)
        );
    }
}
