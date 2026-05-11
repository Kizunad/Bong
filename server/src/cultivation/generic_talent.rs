//! 通用顿悟天赋注册表：JSON 数据驱动的 gain/cost 对轴。

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use serde::Deserialize;

use super::components::{ColorKind, MeridianId};
use super::insight::{
    InsightAlignment, InsightCategory, InsightCost, InsightEffect, InsightTradeoff,
};

pub const ALL_COLORS: [ColorKind; 10] = [
    ColorKind::Sharp,
    ColorKind::Heavy,
    ColorKind::Mellow,
    ColorKind::Solid,
    ColorKind::Light,
    ColorKind::Intricate,
    ColorKind::Gentle,
    ColorKind::Insidious,
    ColorKind::Violent,
    ColorKind::Turbid,
];

#[derive(Debug, Clone, Deserialize)]
pub struct GenericTalentFile {
    pub version: u32,
    pub talents: Vec<GenericTalentDef>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GenericTalentDef {
    pub id: String,
    pub category: String,
    pub color_affinity: Vec<String>,
    pub alignment: String,
    pub gain: StatModifier,
    pub cost: StatModifier,
    pub gain_flavor: String,
    pub cost_flavor: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StatModifier {
    pub stat: String,
    pub op: String,
    pub base_value: f64,
    #[serde(default)]
    pub meridian_group: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GenericTalentRegistry {
    talents: Vec<GenericTalentDef>,
}

impl GenericTalentRegistry {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let text = fs::read_to_string(path.as_ref())
            .map_err(|error| format!("读取 {} 失败: {error}", path.as_ref().display()))?;
        Self::from_json(text.as_str())
    }

    pub fn builtin() -> Result<Self, String> {
        Self::from_json(include_str!("../../assets/insight/generic_talents.json"))
    }

    pub fn from_json(text: &str) -> Result<Self, String> {
        let file: GenericTalentFile = serde_json::from_str(text)
            .map_err(|error| format!("generic_talents JSON 无效: {error}"))?;
        if file.version != 1 {
            return Err(format!("generic_talents version={} 不受支持", file.version));
        }
        let registry = Self {
            talents: file.talents,
        };
        registry.validate()?;
        Ok(registry)
    }

    pub fn talents(&self) -> &[GenericTalentDef] {
        &self.talents
    }

    pub fn query(
        &self,
        color_affinity: ColorKind,
        alignment: InsightAlignment,
    ) -> Vec<&GenericTalentDef> {
        let mut matches: Vec<_> = self
            .talents
            .iter()
            .filter(|talent| alignment_matches(talent.alignment.as_str(), alignment))
            .filter(|talent| affinity_matches(talent.color_affinity.as_slice(), color_affinity))
            .collect();
        matches.sort_by_key(|talent| talent.alignment != alignment.code());
        matches
    }

    pub fn to_insight_tradeoff(
        &self,
        def: &GenericTalentDef,
        alignment: InsightAlignment,
        main_color: ColorKind,
        diverge_color: ColorKind,
    ) -> Result<InsightTradeoff, String> {
        let coeff = alignment_coeff(alignment);
        let gain = effect_from_modifier(&def.gain, coeff, main_color, diverge_color)
            .map_err(|error| format!("{} gain 无效: {error}", def.id))?;
        let gain_magnitude = gain.magnitude();
        let mut cost = cost_from_modifier(&def.cost, main_color, diverge_color)
            .map_err(|error| format!("{} cost 无效: {error}", def.id))?;
        let min_cost = gain_magnitude * 0.5;
        if cost.magnitude() < min_cost {
            cost = amplify_cost(cost, min_cost);
        }
        let cost_magnitude = cost.magnitude();
        let target_color = match alignment {
            InsightAlignment::Converge => Some(main_color),
            InsightAlignment::Neutral => None,
            InsightAlignment::Diverge => Some(diverge_color),
        };

        Ok(InsightTradeoff {
            alignment,
            gain,
            gain_magnitude,
            cost,
            cost_magnitude,
            gain_flavor: resolve_flavor_template(
                def.gain_flavor.as_str(),
                main_color,
                target_color.unwrap_or(diverge_color),
                gain_magnitude,
            ),
            cost_flavor: resolve_flavor_template(
                def.cost_flavor.as_str(),
                main_color,
                target_color.unwrap_or(diverge_color),
                cost_magnitude,
            ),
            target_color,
        })
    }

    fn validate(&self) -> Result<(), String> {
        let mut seen = HashSet::new();
        for talent in &self.talents {
            if talent.id.trim().is_empty() {
                return Err("generic talent id 不能为空".to_string());
            }
            if !seen.insert(talent.id.as_str()) {
                return Err(format!("generic talent id 重复: {}", talent.id));
            }
            parse_category(talent.category.as_str())
                .ok_or_else(|| format!("{} category 不在白名单: {}", talent.id, talent.category))?;
            if !matches!(
                talent.alignment.as_str(),
                "converge" | "neutral" | "diverge" | "any"
            ) {
                return Err(format!(
                    "{} alignment 无效: {}",
                    talent.id, talent.alignment
                ));
            }
            if talent.color_affinity.is_empty() {
                return Err(format!("{} color_affinity 不能为空", talent.id));
            }
            for color in &talent.color_affinity {
                if color != "*" && parse_color_kind(color.as_str()).is_none() {
                    return Err(format!("{} color_affinity 无效: {color}", talent.id));
                }
            }
            validate_stat(&talent.gain, true)
                .map_err(|error| format!("{} gain {error}", talent.id))?;
            validate_stat(&talent.cost, false)
                .map_err(|error| format!("{} cost {error}", talent.id))?;
        }
        Ok(())
    }
}

pub fn parse_color_kind(value: &str) -> Option<ColorKind> {
    match value.to_ascii_lowercase().as_str() {
        "sharp" => Some(ColorKind::Sharp),
        "heavy" => Some(ColorKind::Heavy),
        "mellow" => Some(ColorKind::Mellow),
        "solid" => Some(ColorKind::Solid),
        "light" => Some(ColorKind::Light),
        "intricate" => Some(ColorKind::Intricate),
        "gentle" => Some(ColorKind::Gentle),
        "insidious" => Some(ColorKind::Insidious),
        "violent" => Some(ColorKind::Violent),
        "turbid" => Some(ColorKind::Turbid),
        _ => None,
    }
}

pub fn color_kind_to_chinese(color: ColorKind) -> &'static str {
    match color {
        ColorKind::Sharp => "锋锐",
        ColorKind::Heavy => "沉重",
        ColorKind::Mellow => "温润",
        ColorKind::Solid => "凝实",
        ColorKind::Light => "飘逸",
        ColorKind::Intricate => "缜密",
        ColorKind::Gentle => "柔和",
        ColorKind::Insidious => "阴诡",
        ColorKind::Violent => "暴烈",
        ColorKind::Turbid => "浊乱",
    }
}

pub fn resolve_flavor_template(
    template: &str,
    main_color: ColorKind,
    target_color: ColorKind,
    magnitude: f64,
) -> String {
    template
        .replace("{color_name}", color_kind_to_chinese(main_color))
        .replace("{target_color_name}", color_kind_to_chinese(target_color))
        .replace("{gain_pct}", pct(magnitude).as_str())
        .replace("{cost_pct}", pct(magnitude).as_str())
}

fn pct(value: f64) -> String {
    let mut text = format!("{:.1}", value * 100.0);
    if text.ends_with(".0") {
        text.truncate(text.len() - 2);
    }
    text
}

fn alignment_matches(value: &str, alignment: InsightAlignment) -> bool {
    value == "any" || value == alignment.code()
}

fn affinity_matches(values: &[String], color: ColorKind) -> bool {
    values
        .iter()
        .any(|value| value == "*" || parse_color_kind(value.as_str()) == Some(color))
}

fn parse_category(value: &str) -> Option<InsightCategory> {
    match value.to_ascii_lowercase().as_str() {
        "meridian" => Some(InsightCategory::Meridian),
        "qi" => Some(InsightCategory::Qi),
        "composure" => Some(InsightCategory::Composure),
        "coloring" => Some(InsightCategory::Coloring),
        "breakthrough" => Some(InsightCategory::Breakthrough),
        "style" => Some(InsightCategory::Style),
        "perception" => Some(InsightCategory::Perception),
        _ => None,
    }
}

fn validate_stat(stat: &StatModifier, gain: bool) -> Result<(), String> {
    if !matches!(stat.op.as_str(), "mul" | "add" | "sub") {
        return Err(format!("op 无效: {}", stat.op));
    }
    let Some((min, max)) = stat_range(stat.stat.as_str(), gain) else {
        return Err(format!("stat 不在白名单: {}", stat.stat));
    };
    if !stat.base_value.is_finite() || stat.base_value < min || stat.base_value > max {
        return Err(format!(
            "{} base_value={} 超出范围 [{}, {}]",
            stat.stat, stat.base_value, min, max
        ));
    }
    Ok(())
}

fn stat_range(stat: &str, gain: bool) -> Option<(f64, f64)> {
    if gain {
        match stat {
            "qi_regen_factor" => Some((1.01, 1.10)),
            "composure_recover" => Some((1.01, 1.15)),
            "breakthrough_bonus" => Some((0.01, 0.05)),
            "meridian_flow_rate" => Some((1.01, 1.08)),
            "overload_tolerance" => Some((0.01, 0.05)),
            "color_cap" => Some((0.01, 0.05)),
            "chaotic_tolerance" => Some((0.01, 0.05)),
            "hunyuan_threshold" => Some((0.95, 0.99)),
            _ => None,
        }
    } else {
        match stat {
            "qi_volatility" => Some((0.01, 0.05)),
            "shock_sensitivity" => Some((0.01, 0.05)),
            "opposite_color_penalty" => Some((0.05, 0.20)),
            "main_color_penalty" => Some((0.05, 0.15)),
            "overload_fragility" => Some((0.01, 0.05)),
            "meridian_heal_slowdown" => Some((0.85, 0.95)),
            "breakthrough_failure_penalty" => Some((1.05, 1.20)),
            "sense_exposure" => Some((0.01, 0.05)),
            "reaction_window_shrink" => Some((0.90, 0.97)),
            "chaotic_tolerance_loss" => Some((0.01, 0.03)),
            _ => None,
        }
    }
}

fn alignment_coeff(alignment: InsightAlignment) -> f64 {
    match alignment {
        InsightAlignment::Converge => 1.2,
        InsightAlignment::Neutral => 1.0,
        InsightAlignment::Diverge => 0.9,
    }
}

fn effect_from_modifier(
    stat: &StatModifier,
    coeff: f64,
    main_color: ColorKind,
    diverge_color: ColorKind,
) -> Result<InsightEffect, String> {
    let value = scaled_gain_value(stat, coeff);
    Ok(match stat.stat.as_str() {
        "qi_regen_factor" => InsightEffect::QiRegenFactor { mul: value },
        "composure_recover" => InsightEffect::ComposureRecover { mul: value },
        "breakthrough_bonus" => InsightEffect::NextBreakthroughBonus { add: value },
        "meridian_flow_rate" => InsightEffect::MeridianRate {
            id: meridian_group_to_id(stat.meridian_group.as_deref()),
            mul: value,
        },
        "overload_tolerance" => InsightEffect::MeridianOverloadTolerance {
            id: meridian_group_to_id(stat.meridian_group.as_deref()),
            add: value,
        },
        "color_cap" => InsightEffect::ColorCapAdd {
            color: resolve_color_token(stat.color.as_deref(), main_color, diverge_color)?,
            add: value,
        },
        "chaotic_tolerance" => InsightEffect::ChaoticTolerance { add: value },
        "hunyuan_threshold" => InsightEffect::HunyuanThreshold { mul: value },
        other => return Err(format!("未知 gain stat: {other}")),
    })
}

fn scaled_gain_value(stat: &StatModifier, coeff: f64) -> f64 {
    match stat.op.as_str() {
        "mul" if stat.base_value < 1.0 => 1.0 - (1.0 - stat.base_value) * coeff,
        "mul" => 1.0 + (stat.base_value - 1.0) * coeff,
        _ => stat.base_value * coeff,
    }
}

fn cost_from_modifier(
    stat: &StatModifier,
    main_color: ColorKind,
    diverge_color: ColorKind,
) -> Result<InsightCost, String> {
    Ok(match stat.stat.as_str() {
        "qi_volatility" => InsightCost::QiVolatility {
            add: stat.base_value,
        },
        "shock_sensitivity" => InsightCost::ShockSensitivity {
            add: stat.base_value,
        },
        "opposite_color_penalty" => InsightCost::OppositeColorPenalty {
            color: super::color_affinity::opposite_color(main_color),
            penalty: stat.base_value,
        },
        "main_color_penalty" => InsightCost::MainColorPenalty {
            color: main_color,
            penalty: stat.base_value,
        },
        "overload_fragility" => InsightCost::OverloadFragility {
            add: stat.base_value,
        },
        "meridian_heal_slowdown" => InsightCost::MeridianHealSlowdown {
            mul: stat.base_value,
        },
        "breakthrough_failure_penalty" => InsightCost::BreakthroughFailurePenalty {
            mul: stat.base_value,
        },
        "sense_exposure" => InsightCost::SenseExposure {
            add: stat.base_value,
        },
        "reaction_window_shrink" => InsightCost::ReactionWindowShrink {
            mul: stat.base_value,
        },
        "chaotic_tolerance_loss" => InsightCost::ChaoticToleranceLoss {
            sub: stat.base_value,
        },
        "main_color_token" => InsightCost::MainColorPenalty {
            color: resolve_color_token(stat.color.as_deref(), main_color, diverge_color)?,
            penalty: stat.base_value,
        },
        other => return Err(format!("未知 cost stat: {other}")),
    })
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
            mul: (1.0 - required).max(0.01),
        },
        InsightCost::BreakthroughFailurePenalty { .. } => InsightCost::BreakthroughFailurePenalty {
            mul: 1.0 + required,
        },
        InsightCost::SenseExposure { .. } => InsightCost::SenseExposure { add: required },
        InsightCost::ReactionWindowShrink { .. } => InsightCost::ReactionWindowShrink {
            mul: (1.0 - required).max(0.01),
        },
        InsightCost::ChaoticToleranceLoss { .. } => {
            InsightCost::ChaoticToleranceLoss { sub: required }
        }
    }
}

fn resolve_color_token(
    value: Option<&str>,
    main_color: ColorKind,
    diverge_color: ColorKind,
) -> Result<ColorKind, String> {
    match value.unwrap_or("$main") {
        "$main" => Ok(main_color),
        "$diverge" => Ok(diverge_color),
        other => parse_color_kind(other).ok_or_else(|| format!("未知 color token: {other}")),
    }
}

fn meridian_group_to_id(group: Option<&str>) -> MeridianId {
    match group.unwrap_or("arm_yin") {
        "arm_yang" => MeridianId::LargeIntestine,
        "leg_yin" => MeridianId::Spleen,
        "leg_yang" => MeridianId::Stomach,
        "ren_du" => MeridianId::Ren,
        _ => MeridianId::Lung,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_json_valid() {
        let registry = GenericTalentRegistry::builtin().expect("generic_talents.json should load");
        assert!(registry.talents().len() >= 6);
    }

    #[test]
    fn load_json_rejects_unknown_stat() {
        let json = r#"{
          "version": 1,
          "talents": [{
            "id": "bad", "category": "qi", "color_affinity": ["*"], "alignment": "neutral",
            "gain": {"stat": "missing", "op": "add", "base_value": 0.03},
            "cost": {"stat": "qi_volatility", "op": "add", "base_value": 0.02},
            "gain_flavor": "x", "cost_flavor": "y"
          }]
        }"#;
        assert!(GenericTalentRegistry::from_json(json).is_err());
    }

    #[test]
    fn load_json_rejects_out_of_range() {
        let json = r#"{
          "version": 1,
          "talents": [{
            "id": "bad", "category": "qi", "color_affinity": ["*"], "alignment": "neutral",
            "gain": {"stat": "qi_regen_factor", "op": "mul", "base_value": 2.0},
            "cost": {"stat": "qi_volatility", "op": "add", "base_value": 0.02},
            "gain_flavor": "x", "cost_flavor": "y"
          }]
        }"#;
        assert!(GenericTalentRegistry::from_json(json).is_err());
    }

    #[test]
    fn query_by_color_and_alignment() {
        let registry = GenericTalentRegistry::builtin().unwrap();
        let sharp = registry.query(ColorKind::Sharp, InsightAlignment::Converge);
        assert!(sharp.iter().any(|talent| talent.alignment == "converge"));
    }

    #[test]
    fn wildcard_affinity_matches_all() {
        let registry = GenericTalentRegistry::builtin().unwrap();
        for color in ALL_COLORS {
            assert!(!registry.query(color, InsightAlignment::Neutral).is_empty());
        }
    }

    #[test]
    fn resolve_flavor_template_replaces_tokens() {
        let text = resolve_flavor_template(
            "{color_name}->{target_color_name}:{gain_pct}/{cost_pct}",
            ColorKind::Sharp,
            ColorKind::Heavy,
            0.036,
        );
        assert_eq!(text, "锋锐->沉重:3.6/3.6");
    }
}
