//! 顿悟静态选项池（plan §5.6）— agent 失败或不可用时兜底。
//!
//! 每个触发 ID 必须提供 ≥ 3 条选项。数值取上限 60% 作为"保底但不强"。

use super::components::{ColorKind, MeridianId, Realm};
use super::insight::{InsightCategory, InsightChoice, InsightEffect};

/// 按触发 ID 返回静态选项。未知触发返回空 Vec（上层可报错或跳过）。
pub fn fallback_for(trigger_id: &str) -> Vec<InsightChoice> {
    match trigger_id {
        "first_breakthrough_to_Induce"
        | "first_breakthrough_to_Condense"
        | "first_breakthrough_to_Solidify"
        | "first_breakthrough_to_Spirit"
        | "first_breakthrough_to_Void" => breakthrough_first_set(),
        "breakthrough_failed_recovered" => breakthrough_failed_set(),
        "meridian_forge_tier_milestone" => forge_milestone_set(),
        "first_tribulation_survived" => tribulation_survived_set(),
        "survived_negative_zone" => negative_zone_set(),
        "practice_dedication_milestone" => practice_milestone_set(),
        "chaotic_to_hunyuan_pivot" => color_pivot_set(),
        "witnessed_xuhua_tribulation"
        | "killed_higher_realm"
        | "killed_by_higher_realm_survived"
        | "post_rebirth_clarity" => generic_set(),
        _ => Vec::new(),
    }
}

fn breakthrough_first_set() -> Vec<InsightChoice> {
    vec![
        InsightChoice {
            category: InsightCategory::Breakthrough,
            effect: InsightEffect::NextBreakthroughBonus { add: 0.03 },
            flavor: "你已知冲关时神识凝聚的诀窍，下次心会更稳".into(),
        },
        InsightChoice {
            category: InsightCategory::Qi,
            effect: InsightEffect::QiRegenFactor { mul: 1.03 },
            flavor: "你的呼吸与天地灵气节奏更贴合".into(),
        },
        InsightChoice {
            category: InsightCategory::Composure,
            effect: InsightEffect::ComposureRecover { mul: 1.06 },
            flavor: "经此一遭，心如止水的速度略快".into(),
        },
    ]
}

fn breakthrough_failed_set() -> Vec<InsightChoice> {
    vec![
        InsightChoice {
            category: InsightCategory::Composure,
            effect: InsightEffect::ComposureShockDiscount {
                event: "BreakthroughFailure".into(),
                mul: 0.92,
            },
            flavor: "你已不畏走火，再经一次，心已无澜".into(),
        },
        InsightChoice {
            category: InsightCategory::Qi,
            effect: InsightEffect::UnfreezeQiMax { mul: 0.97 },
            flavor: "过去过载的旧伤略有松动，真元池微微扩展".into(),
        },
        InsightChoice {
            category: InsightCategory::Breakthrough,
            effect: InsightEffect::NextBreakthroughBonus { add: 0.03 },
            flavor: "你已识得翻车的感觉，下次会避开".into(),
        },
    ]
}

fn forge_milestone_set() -> Vec<InsightChoice> {
    vec![
        InsightChoice {
            category: InsightCategory::Meridian,
            effect: InsightEffect::MeridianForgeDiscount {
                id: MeridianId::Lung,
                discount: 0.05,
            },
            flavor: "你看清了这条经脉的脉络走向".into(),
        },
        InsightChoice {
            category: InsightCategory::Style,
            effect: InsightEffect::DualForgeDiscount {
                id: MeridianId::Lung,
                mul: 0.90,
            },
            flavor: "你看懂了双修的损耗节律".into(),
        },
        InsightChoice {
            category: InsightCategory::Meridian,
            effect: InsightEffect::MeridianOverloadTolerance {
                id: MeridianId::Lung,
                add: 0.03,
            },
            flavor: "你能感觉到经脉在过载边缘的颤动".into(),
        },
    ]
}

fn tribulation_survived_set() -> Vec<InsightChoice> {
    vec![
        InsightChoice {
            category: InsightCategory::Breakthrough,
            effect: InsightEffect::TribulationPredictionWindow,
            flavor: "你能在劫云聚拢前，听见第一道雷的脉搏".into(),
        },
        InsightChoice {
            category: InsightCategory::Composure,
            effect: InsightEffect::ComposureRecover { mul: 1.06 },
            flavor: "扛过劫后，你的心境恢复更快".into(),
        },
        InsightChoice {
            category: InsightCategory::Perception,
            effect: InsightEffect::UnlockPerception {
                kind: "tribulation_first_wave_preview".into(),
            },
            flavor: "劫云中的第一道雷形已在你识海预演".into(),
        },
    ]
}

fn negative_zone_set() -> Vec<InsightChoice> {
    vec![
        InsightChoice {
            category: InsightCategory::Perception,
            effect: InsightEffect::UnlockPerception {
                kind: "zone_qi_density".into(),
            },
            flavor: "你能感知方圆百米灵气浓淡，再不会盲目静坐于枯地".into(),
        },
        InsightChoice {
            category: InsightCategory::Qi,
            effect: InsightEffect::UnfreezeQiMax { mul: 0.97 },
            flavor: "穿过虚无的经历让真元池意外扩展".into(),
        },
        InsightChoice {
            category: InsightCategory::Composure,
            effect: InsightEffect::ComposureRecover { mul: 1.06 },
            flavor: "在虚无中见本心，归来后心境愈固".into(),
        },
    ]
}

fn practice_milestone_set() -> Vec<InsightChoice> {
    vec![
        InsightChoice {
            category: InsightCategory::Coloring,
            effect: InsightEffect::ColorCapAdd {
                color: ColorKind::Sharp,
                add: 0.03,
            },
            flavor: "你的本色更纯，再练可达更深之境".into(),
        },
        InsightChoice {
            category: InsightCategory::Coloring,
            effect: InsightEffect::ChaoticTolerance { add: 0.03 },
            flavor: "你能在多修之间保持本心".into(),
        },
        InsightChoice {
            category: InsightCategory::Qi,
            effect: InsightEffect::QiRegenFactor { mul: 1.03 },
            flavor: "长年修习，你的吐纳已近自然".into(),
        },
    ]
}

fn color_pivot_set() -> Vec<InsightChoice> {
    vec![
        InsightChoice {
            category: InsightCategory::Coloring,
            effect: InsightEffect::HunyuanThreshold { mul: 0.97 },
            flavor: "你已窥见万法归一的门径".into(),
        },
        InsightChoice {
            category: InsightCategory::Style,
            effect: InsightEffect::UnlockPractice {
                name: "三色调和".into(),
            },
            flavor: "你领悟了三色相济的法门".into(),
        },
        InsightChoice {
            category: InsightCategory::Composure,
            effect: InsightEffect::ComposureRecover { mul: 1.06 },
            flavor: "混元之初，心境一新".into(),
        },
    ]
}

fn generic_set() -> Vec<InsightChoice> {
    vec![
        InsightChoice {
            category: InsightCategory::Qi,
            effect: InsightEffect::QiRegenFactor { mul: 1.03 },
            flavor: "你对此事有所感悟".into(),
        },
        InsightChoice {
            category: InsightCategory::Composure,
            effect: InsightEffect::ComposureRecover { mul: 1.06 },
            flavor: "心境因此更稳".into(),
        },
        InsightChoice {
            category: InsightCategory::Breakthrough,
            effect: InsightEffect::NextBreakthroughBonus { add: 0.03 },
            flavor: "此等经历，让你更接近下一关".into(),
        },
    ]
}

#[allow(dead_code)]
fn _realm_referenced(r: Realm) -> Realm {
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::insight::validate_offer;
    use crate::cultivation::insight::InsightQuota;

    #[test]
    fn every_known_trigger_has_at_least_three_options() {
        let ids = [
            "first_breakthrough_to_Induce",
            "breakthrough_failed_recovered",
            "meridian_forge_tier_milestone",
            "first_tribulation_survived",
            "survived_negative_zone",
            "practice_dedication_milestone",
            "chaotic_to_hunyuan_pivot",
            "witnessed_xuhua_tribulation",
        ];
        for id in ids {
            let opts = fallback_for(id);
            assert!(opts.len() >= 3, "{id} had {} options", opts.len());
        }
    }

    #[test]
    fn all_fallback_options_pass_arbiter() {
        let quota = InsightQuota::default();
        let ids = [
            "first_breakthrough_to_Induce",
            "breakthrough_failed_recovered",
            "meridian_forge_tier_milestone",
            "first_tribulation_survived",
            "survived_negative_zone",
            "practice_dedication_milestone",
            "chaotic_to_hunyuan_pivot",
        ];
        for id in ids {
            for choice in fallback_for(id) {
                validate_offer(&quota, &choice, Realm::Induce)
                    .unwrap_or_else(|e| panic!("fallback for {id} failed arbiter: {e:?}"));
            }
        }
    }

    #[test]
    fn unknown_trigger_yields_empty() {
        assert!(fallback_for("unknown_thing_xyz").is_empty());
    }
}
