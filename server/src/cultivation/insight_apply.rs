//! InsightChosen 效果应用（plan §5.5 最后一步）。
//!
//! 对 Cultivation / MeridianSystem / QiColor / LifeRecord 具体修改。部分
//! "解锁感知"类效果只是在 perception set 里登记，由客户端在 inspect UI 决定如何展示。

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component};

use super::components::{Cultivation, MeridianSystem, QiColor};
use super::insight::{InsightChoice, InsightEffect};
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
    /// 解锁的实践/流派
    pub practices: HashSet<String>,
}

impl InsightModifiers {
    pub fn new() -> Self {
        Self {
            qi_regen_mul: 1.0,
            composure_recover_mul: 1.0,
            next_breakthrough_bonus: 0.0,
            hunyuan_threshold_mul: 1.0,
            chaotic_tolerance_add: 0.0,
            practices: HashSet::new(),
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn apply_choice(
    choice: &InsightChoice,
    cultivation: &mut Cultivation,
    meridians: &mut MeridianSystem,
    _qi_color: &mut QiColor,
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
        UnlockPractice { name } => {
            modifiers.practices.insert(name.clone());
        }
        UnlockPerception { kind } => {
            perceptions.set.insert(kind.clone());
        }
    }

    // 写生平
    life_record.insights_taken.push(TakenInsight {
        trigger_id: trigger_id.to_string(),
        choice: format!("{:?}", choice.effect),
        magnitude: choice.effect.magnitude(),
        flavor: choice.flavor.clone(),
        taken_at: tick_now,
        realm_at_time: cultivation.realm,
    });
    life_record.biography.push(BiographyEntry::InsightTaken {
        trigger: trigger_id.to_string(),
        choice: format!("{:?}", choice.effect),
        tick: tick_now,
    });
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
        let choice = InsightChoice {
            category: InsightCategory::Meridian,
            effect: InsightEffect::MeridianRate {
                id: MeridianId::Lung,
                mul: 1.05,
            },
            flavor: "x".into(),
        };
        let before = ms.get(MeridianId::Lung).flow_rate;
        apply_choice(
            &choice, &mut c, &mut ms, &mut qc, &mut perc, &mut mods, &mut lr, "t", 100,
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
        let choice = InsightChoice {
            category: InsightCategory::Perception,
            effect: InsightEffect::UnlockPerception {
                kind: "zone_qi_density".into(),
            },
            flavor: "".into(),
        };
        apply_choice(
            &choice, &mut c, &mut ms, &mut qc, &mut perc, &mut mods, &mut lr, "t", 0,
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
        let choice = InsightChoice {
            category: InsightCategory::Qi,
            effect: InsightEffect::QiRegenFactor { mul: 1.05 },
            flavor: "".into(),
        };
        apply_choice(
            &choice, &mut c, &mut ms, &mut qc, &mut perc, &mut mods, &mut lr, "t", 0,
        );
        assert!((mods.qi_regen_mul - 1.05).abs() < 1e-9);
    }
}
