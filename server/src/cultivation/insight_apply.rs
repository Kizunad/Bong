//! InsightChosen 效果应用（plan §5.5 最后一步）。
//!
//! 对 Cultivation / MeridianSystem / QiColor / LifeRecord 具体修改。部分
//! "解锁感知"类效果只是在 perception set 里登记，由客户端在 inspect UI 决定如何展示。

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component};

use super::color::PracticeLog;
use super::components::{Cultivation, MeridianSystem, QiColor};
use super::insight::{InsightAlignment, InsightChoice, InsightCost, InsightEffect};
use super::life_record::{BiographyEntry, LifeRecord, TakenInsight};

/// 玩家解锁的感知能力集合（InsightEffect::UnlockPerception 写入）。
#[derive(Debug, Clone, Default, Component, Serialize, Deserialize)]
pub struct UnlockedPerceptions {
    pub set: HashSet<String>,
}

/// 用于 ComposureShockDiscount / BreakthroughEventConditionDrop 等"修饰器"。
#[derive(Debug, Clone, Default, Component, Serialize, Deserialize)]
pub struct InsightModifiers {
    pub qi_regen_mul: f64,
    pub composure_recover_mul: f64,
    pub next_breakthrough_bonus: f64,
    pub hunyuan_threshold_mul: f64,
    pub chaotic_tolerance_add: f64,
    /// 地师·阵法流：藏阵 / 破阵对立路径的累计等级。
    pub zhenfa_concealment: f64,
    pub zhenfa_disenchant: f64,
    #[serde(default)]
    pub opposite_color_efficiency_penalty: f64,
    #[serde(default)]
    pub qi_volatility_add: f64,
    #[serde(default)]
    pub shock_sensitivity_add: f64,
    #[serde(default)]
    pub main_color_efficiency_penalty: f64,
    #[serde(default)]
    pub reaction_window_penalty: f64,
    #[serde(default = "default_one")]
    pub breakthrough_failure_penalty_mul: f64,
    #[serde(default)]
    pub sense_exposure_add: f64,
    #[serde(default)]
    pub overload_fragility_add: f64,
    #[serde(default = "default_one")]
    pub meridian_heal_slowdown_mul: f64,
    #[serde(default)]
    pub chaotic_tolerance_loss: f64,
    /// 解锁的实践/流派
    pub practices: HashSet<String>,
}

fn default_one() -> f64 {
    1.0
}

impl InsightModifiers {
    pub fn new() -> Self {
        Self {
            qi_regen_mul: 1.0,
            composure_recover_mul: 1.0,
            next_breakthrough_bonus: 0.0,
            hunyuan_threshold_mul: 1.0,
            chaotic_tolerance_add: 0.0,
            zhenfa_concealment: 0.0,
            zhenfa_disenchant: 0.0,
            opposite_color_efficiency_penalty: 0.0,
            qi_volatility_add: 0.0,
            shock_sensitivity_add: 0.0,
            main_color_efficiency_penalty: 0.0,
            reaction_window_penalty: 0.0,
            breakthrough_failure_penalty_mul: 1.0,
            sense_exposure_add: 0.0,
            overload_fragility_add: 0.0,
            meridian_heal_slowdown_mul: 1.0,
            chaotic_tolerance_loss: 0.0,
            practices: HashSet::new(),
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn apply_choice(
    choice: &InsightChoice,
    cultivation: &mut Cultivation,
    meridians: &mut MeridianSystem,
    qi_color: &mut QiColor,
    practice_log: Option<&mut PracticeLog>,
    perceptions: &mut UnlockedPerceptions,
    modifiers: &mut InsightModifiers,
    life_record: &mut LifeRecord,
    trigger_id: &str,
    tick_now: u64,
) {
    use InsightEffect::*;
    match &choice.effect {
        MeridianRate { id, mul } => {
            let m = meridians.get_mut(*id);
            m.flow_rate *= mul;
        }
        MeridianForgeDiscount { .. } => {
            // 记到 modifiers（本 plan 未细化锻造折扣接口，留作后续）
        }
        MeridianOverloadTolerance { id, add } => {
            let m = meridians.get_mut(*id);
            m.flow_rate *= 1.0 + add; // 用 flow_rate 代理 overload 阈值
        }
        QiRegenFactor { mul } => {
            modifiers.qi_regen_mul *= mul;
        }
        PurgeEfficiency { .. } => {
            // 留给 ContaminationTick 读 modifiers 未来版本
        }
        UnfreezeQiMax { mul } => {
            if let Some(frozen) = cultivation.qi_max_frozen {
                cultivation.qi_max_frozen = Some(frozen * mul);
            }
        }
        ComposureRecover { mul } => {
            cultivation.composure_recover_rate *= mul;
            modifiers.composure_recover_mul *= mul;
        }
        ComposureShockDiscount { .. } => {
            // 未来在 ComposureTick 外的冲击事件处理器读取
        }
        ComposureImmuneDuringBreakthrough => {
            modifiers
                .practices
                .insert("composure_immune_during_breakthrough".into());
        }
        ColorCapAdd { .. } | ChaoticTolerance { .. } => {
            if let ChaoticTolerance { add } = &choice.effect {
                modifiers.chaotic_tolerance_add += *add;
            }
            // ColorCapAdd 未在 QiColor 中建模具体 cap，留到染色战斗加成切片
        }
        HunyuanThreshold { mul } => {
            modifiers.hunyuan_threshold_mul *= mul;
        }
        NextBreakthroughBonus { add } => {
            modifiers.next_breakthrough_bonus += add;
        }
        BreakthroughEventConditionDrop { .. } | TribulationPredictionWindow => {
            modifiers.practices.insert("breakthrough_hint".into());
        }
        DualForgeDiscount { .. } | ColorMaterialAffinity { .. } => {
            modifiers.practices.insert("forge_specialization".into());
        }
        ZhenfaConcealment { add } => {
            modifiers.zhenfa_concealment = (modifiers.zhenfa_concealment + add).max(0.0);
            modifiers.zhenfa_disenchant = (modifiers.zhenfa_disenchant - add * 0.5).max(0.0);
            modifiers.practices.insert("zhenfa:concealment".into());
        }
        ZhenfaDisenchant { add } => {
            modifiers.zhenfa_disenchant = (modifiers.zhenfa_disenchant + add).max(0.0);
            modifiers.zhenfa_concealment = (modifiers.zhenfa_concealment - add * 0.5).max(0.0);
            modifiers.practices.insert("zhenfa:disenchant".into());
        }
        UnlockPractice { name } => {
            modifiers.practices.insert(name.clone());
        }
        UnlockPerception { kind } => {
            perceptions.set.insert(kind.clone());
        }
        LifespanExtensionEnlightenment => {
            modifiers
                .practices
                .insert("lifespan_extension:enlightenment_used".into());
        }
    }
    apply_tradeoff_cost(&choice.cost, modifiers);
    apply_alignment_side_effects(choice, qi_color, practice_log, life_record, tick_now);

    // 写生平
    life_record.insights_taken.push(TakenInsight {
        trigger_id: trigger_id.to_string(),
        choice: format!("{:?}", choice.effect),
        magnitude: choice.effect.magnitude(),
        flavor: choice.flavor.clone(),
        alignment: Some(choice.alignment.code().to_string()),
        cost_kind: Some(choice.cost.kind().to_string()),
        taken_at: tick_now,
        realm_at_time: cultivation.realm,
    });
    life_record.biography.push(BiographyEntry::InsightTaken {
        trigger: trigger_id.to_string(),
        choice: format!("{:?}", choice.effect),
        alignment: Some(choice.alignment.code().to_string()),
        cost_kind: Some(choice.cost.kind().to_string()),
        tick: tick_now,
    });
}

fn apply_tradeoff_cost(cost: &InsightCost, modifiers: &mut InsightModifiers) {
    match cost {
        InsightCost::OppositeColorPenalty { penalty, .. } => {
            modifiers.opposite_color_efficiency_penalty += penalty;
        }
        InsightCost::QiVolatility { add } => modifiers.qi_volatility_add += add,
        InsightCost::ShockSensitivity { add } => modifiers.shock_sensitivity_add += add,
        InsightCost::MainColorPenalty { penalty, .. } => {
            modifiers.main_color_efficiency_penalty += penalty;
        }
        InsightCost::OverloadFragility { add } => modifiers.overload_fragility_add += add,
        InsightCost::MeridianHealSlowdown { mul } => modifiers.meridian_heal_slowdown_mul *= mul,
        InsightCost::BreakthroughFailurePenalty { mul } => {
            modifiers.breakthrough_failure_penalty_mul *= mul;
        }
        InsightCost::SenseExposure { add } => modifiers.sense_exposure_add += add,
        InsightCost::ReactionWindowShrink { mul } => {
            modifiers.reaction_window_penalty += 1.0 - mul;
        }
        InsightCost::ChaoticToleranceLoss { sub } => modifiers.chaotic_tolerance_loss += sub,
    }
}

fn apply_alignment_side_effects(
    choice: &InsightChoice,
    qi_color: &QiColor,
    practice_log: Option<&mut PracticeLog>,
    life_record: &mut LifeRecord,
    tick_now: u64,
) {
    let Some(log) = practice_log else {
        return;
    };
    match (choice.alignment, choice.target_color) {
        (InsightAlignment::Diverge, Some(target)) => {
            let amount = if qi_color.is_hunyuan { 5.0 } else { 2.0 };
            log.add(target, amount);
            life_record.push(BiographyEntry::InsightDiverge {
                from_color: qi_color.main,
                to_color: target,
                tick: tick_now,
            });
        }
        (InsightAlignment::Converge, Some(target)) if qi_color.is_chaotic => {
            log.add(target, 2.0);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::MeridianId;
    use crate::cultivation::insight::InsightCategory;

    #[test]
    fn meridian_rate_choice_applies_multiplier() {
        let mut c = Cultivation::default();
        let mut ms = MeridianSystem::default();
        let mut qc = QiColor::default();
        let mut perc = UnlockedPerceptions::default();
        let mut mods = InsightModifiers::new();
        let mut lr = LifeRecord::default();
        let choice = InsightChoice::neutral(
            InsightCategory::Meridian,
            InsightEffect::MeridianRate {
                id: MeridianId::Lung,
                mul: 1.05,
            },
            "x",
        );
        let before = ms.get(MeridianId::Lung).flow_rate;
        apply_choice(
            &choice, &mut c, &mut ms, &mut qc, None, &mut perc, &mut mods, &mut lr, "t", 100,
        );
        assert!((ms.get(MeridianId::Lung).flow_rate - before * 1.05).abs() < 1e-9);
        assert_eq!(lr.insights_taken.len(), 1);
        assert_eq!(lr.biography.len(), 1);
    }

    #[test]
    fn perception_choice_unlocks() {
        let mut c = Cultivation::default();
        let mut ms = MeridianSystem::default();
        let mut qc = QiColor::default();
        let mut perc = UnlockedPerceptions::default();
        let mut mods = InsightModifiers::new();
        let mut lr = LifeRecord::default();
        let choice = InsightChoice::neutral(
            crate::cultivation::insight::InsightCategory::Perception,
            InsightEffect::UnlockPerception {
                kind: "zone_qi_density".into(),
            },
            "",
        );
        apply_choice(
            &choice, &mut c, &mut ms, &mut qc, None, &mut perc, &mut mods, &mut lr, "t", 0,
        );
        assert!(perc.set.contains("zone_qi_density"));
    }

    #[test]
    fn qi_regen_factor_multiplies_modifier() {
        let mut c = Cultivation::default();
        let mut ms = MeridianSystem::default();
        let mut qc = QiColor::default();
        let mut perc = UnlockedPerceptions::default();
        let mut mods = InsightModifiers::new();
        let mut lr = LifeRecord::default();
        let choice = InsightChoice::neutral(
            InsightCategory::Qi,
            InsightEffect::QiRegenFactor { mul: 1.05 },
            "",
        );
        apply_choice(
            &choice, &mut c, &mut ms, &mut qc, None, &mut perc, &mut mods, &mut lr, "t", 0,
        );
        assert!((mods.qi_regen_mul - 1.05).abs() < 1e-9);
    }

    #[test]
    fn lifespan_extension_choice_marks_enlightenment_use() {
        let mut c = Cultivation::default();
        let mut ms = MeridianSystem::default();
        let mut qc = QiColor::default();
        let mut perc = UnlockedPerceptions::default();
        let mut mods = InsightModifiers::new();
        let mut lr = LifeRecord::default();
        let choice = InsightChoice::neutral(
            crate::cultivation::insight::InsightCategory::Perception,
            crate::cultivation::insight::InsightEffect::LifespanExtensionEnlightenment,
            "",
        );
        apply_choice(
            &choice, &mut c, &mut ms, &mut qc, None, &mut perc, &mut mods, &mut lr, "t", 0,
        );
        assert!(mods
            .practices
            .contains("lifespan_extension:enlightenment_used"));
    }

    #[test]
    fn diverge_choice_injects_practice_log_and_cost() {
        let mut c = Cultivation::default();
        let mut ms = MeridianSystem::default();
        let mut qc = QiColor {
            main: crate::cultivation::components::ColorKind::Sharp,
            ..QiColor::default()
        };
        let mut log = PracticeLog::default();
        let mut perc = UnlockedPerceptions::default();
        let mut mods = InsightModifiers::new();
        let mut lr = LifeRecord::default();
        let mut choice = InsightChoice::neutral(
            InsightCategory::Coloring,
            InsightEffect::ColorCapAdd {
                color: crate::cultivation::components::ColorKind::Light,
                add: 0.03,
            },
            "转向飘逸",
        );
        choice.alignment = InsightAlignment::Diverge;
        choice.target_color = Some(crate::cultivation::components::ColorKind::Light);
        choice.cost = InsightCost::MainColorPenalty {
            color: crate::cultivation::components::ColorKind::Sharp,
            penalty: 0.10,
        };
        choice.cost_magnitude = 0.10;
        apply_choice(
            &choice,
            &mut c,
            &mut ms,
            &mut qc,
            Some(&mut log),
            &mut perc,
            &mut mods,
            &mut lr,
            "t",
            0,
        );
        assert_eq!(
            log.weights
                .get(&crate::cultivation::components::ColorKind::Light)
                .copied(),
            Some(2.0)
        );
        assert!((mods.main_color_efficiency_penalty - 0.10).abs() < 1e-9);
        assert!(lr
            .biography
            .iter()
            .any(|entry| matches!(entry, BiographyEntry::InsightDiverge { .. })));
    }
}
