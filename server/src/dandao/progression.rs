//! plan-dandao-path-v1 P5 — 境界递进 + 流派平衡。
//!
//! 定义醒灵→化虚各境界解锁的丹道能力 + 与七流派的互动关系。

use crate::cultivation::components::Realm;

use super::components::MutationStage;

/// 各境界解锁的丹道能力 ID。
pub fn abilities_unlocked_at(realm: Realm) -> &'static [&'static str] {
    match realm {
        Realm::Awaken => &["dandao.pill_rush"],
        Realm::Induce => &["dandao.pill_rush", "dandao.pill_bomb"],
        Realm::Condense => &["dandao.pill_rush", "dandao.pill_bomb", "dandao.pill_mist"],
        Realm::Solidify => &[
            "dandao.pill_rush",
            "dandao.pill_bomb",
            "dandao.pill_mist",
            "dandao.pill_resonance",
        ],
        Realm::Spirit => &[
            "dandao.pill_rush",
            "dandao.pill_bomb",
            "dandao.pill_mist",
            "dandao.pill_resonance",
            "dandao.pill_to_blood",
        ],
        Realm::Void => &[
            "dandao.pill_rush",
            "dandao.pill_bomb",
            "dandao.pill_mist",
            "dandao.pill_resonance",
            "dandao.pill_to_blood",
            "dandao.great_transmutation",
        ],
    }
}

/// 固元被动「丹体共鸣」：自服丹效率加成。
pub const PILL_RESONANCE_EFFICIENCY_BONUS: f64 = 0.30;

/// 通灵「化丹为血」：1 pill → qi_max × 此比例。
pub const PILL_TO_BLOOD_QI_RATIO: f64 = 0.05;
/// 化丹为血代价：cumulative_toxin += 此值。
pub const PILL_TO_BLOOD_TOXIN_COST: f64 = 10.0;

/// 化虚「大衍丹体」：体内炼丹代价。
pub const GREAT_TRANSMUTATION_TOXIN_COST: f64 = 15.0;
/// 大衍丹体每次内炼的永久经脉效率惩罚。
pub const GREAT_TRANSMUTATION_PENALTY_PER_USE: f64 = 0.01;

/// 经脉惩罚对各流派的影响系数。
/// 丹道变异体的经脉惩罚会乘以此系数影响各流派效率。
/// 1.0 = 完全受影响；0.5 = 半影响；0.0 = 不受影响。
pub fn meridian_penalty_factor_for_style(style: &str) -> f64 {
    match style {
        "baomai" => 1.2,   // 体修依赖经脉流量极高，惩罚放大
        "woliu" => 1.1,    // 涡流依赖经脉持续输出
        "anqi" => 0.8,     // 暗器部分依赖载体而非经脉
        "zhenmai" => 0.9,  // 截脉依赖接触面不依赖整体流量
        "tuike" => 0.5,    // 替尸纯物资派，经脉影响小
        "dugu" => 1.0,     // 毒蛊依赖经脉精确控制
        "zhenfa" => 0.7,   // 阵法预埋不依赖实时经脉
        "dandao" => 0.6,   // 丹道自身已适应变异
        _ => 1.0,
    }
}

/// 变异体 vs 七流派的核心优劣势（数据查询接口）。
#[derive(Debug, Clone, Copy)]
pub struct StyleInteraction {
    pub advantage: &'static str,
    pub disadvantage: &'static str,
}

pub fn dandao_vs_style(style: &str) -> StyleInteraction {
    match style {
        "baomai" => StyleInteraction {
            advantage: "变异体质+50%更能扛过载撕裂",
            disadvantage: "经脉惩罚×1.2降低爆脉效率",
        },
        "anqi" => StyleInteraction {
            advantage: "丹药弹=新型载体不损耗真元",
            disadvantage: "丹药有保质期(shelflife)",
        },
        "zhenfa" => StyleInteraction {
            advantage: "丹雾=区域控制叠加",
            disadvantage: "丹雾暴露位置",
        },
        "dugu" => StyleInteraction {
            advantage: "丹毒≈蛊毒同源可叠加",
            disadvantage: "双重经脉惩罚",
        },
        "zhenmai" => StyleInteraction {
            advantage: "变异甲壳增加截脉触发面积",
            disadvantage: "甲壳区域无法精确截脉",
        },
        "tuike" => StyleInteraction {
            advantage: "变异壳+伪皮双层叠加",
            disadvantage: "变异壳不可蜕(永久)",
        },
        "woliu" => StyleInteraction {
            advantage: "丹药辅助真元恢复延长涡流",
            disadvantage: "变异体真元效率低",
        },
        _ => StyleInteraction {
            advantage: "通用体质加成",
            disadvantage: "经脉效率永久下降",
        },
    }
}

/// 天道注视概率加权（plan §6.4）。
/// 变异阶段 3+: 天道注视概率 +20%。
pub fn tiandao_attention_weight(stage: MutationStage) -> f64 {
    match stage {
        MutationStage::None | MutationStage::Subtle | MutationStage::Visible => 0.0,
        MutationStage::Heavy => 0.20,
        MutationStage::Bestial => 0.35,
    }
}

#[cfg(test)]
mod progression_tests {
    use super::*;

    #[test]
    fn abilities_grow_with_realm() {
        let a = abilities_unlocked_at(Realm::Awaken).len();
        let b = abilities_unlocked_at(Realm::Induce).len();
        let c = abilities_unlocked_at(Realm::Condense).len();
        let d = abilities_unlocked_at(Realm::Solidify).len();
        let e = abilities_unlocked_at(Realm::Spirit).len();
        let f = abilities_unlocked_at(Realm::Void).len();
        assert!(a <= b && b <= c && c <= d && d <= e && e <= f,
            "能力数量应随境界递增: {a}/{b}/{c}/{d}/{e}/{f}");
    }

    #[test]
    fn void_realm_has_all_six_abilities() {
        assert_eq!(abilities_unlocked_at(Realm::Void).len(), 6);
    }

    #[test]
    fn pill_rush_available_from_awaken() {
        assert!(abilities_unlocked_at(Realm::Awaken).contains(&"dandao.pill_rush"));
    }

    #[test]
    fn meridian_penalty_factors_in_valid_range() {
        for style in ["baomai", "woliu", "anqi", "zhenmai", "tuike", "dugu", "zhenfa", "dandao"] {
            let f = meridian_penalty_factor_for_style(style);
            assert!(
                (0.0..=2.0).contains(&f),
                "style={style} factor={f} 应在 0.0-2.0"
            );
        }
    }

    #[test]
    fn baomai_has_highest_penalty_factor() {
        let baomai = meridian_penalty_factor_for_style("baomai");
        for style in ["anqi", "zhenmai", "tuike", "dugu", "zhenfa", "dandao"] {
            assert!(
                baomai >= meridian_penalty_factor_for_style(style),
                "体修应是最高惩罚系数 style={style}"
            );
        }
    }

    #[test]
    fn tuike_has_lowest_penalty_factor() {
        let tuike = meridian_penalty_factor_for_style("tuike");
        for style in ["baomai", "woliu", "anqi", "zhenmai", "dugu", "zhenfa"] {
            assert!(
                tuike <= meridian_penalty_factor_for_style(style),
                "替尸应是最低惩罚系数 style={style}"
            );
        }
    }

    #[test]
    fn tiandao_attention_only_heavy_plus() {
        assert_eq!(tiandao_attention_weight(MutationStage::None), 0.0);
        assert_eq!(tiandao_attention_weight(MutationStage::Subtle), 0.0);
        assert_eq!(tiandao_attention_weight(MutationStage::Visible), 0.0);
        assert!(tiandao_attention_weight(MutationStage::Heavy) > 0.0);
        assert!(tiandao_attention_weight(MutationStage::Bestial) > tiandao_attention_weight(MutationStage::Heavy));
    }

    #[test]
    fn style_interaction_has_content() {
        for style in ["baomai", "woliu", "anqi", "zhenmai", "tuike", "dugu", "zhenfa"] {
            let interaction = dandao_vs_style(style);
            assert!(!interaction.advantage.is_empty(), "style={style} advantage 不应为空");
            assert!(!interaction.disadvantage.is_empty(), "style={style} disadvantage 不应为空");
        }
    }

    #[test]
    fn constants_are_positive() {
        let vals: &[f64] = &[
            PILL_RESONANCE_EFFICIENCY_BONUS,
            PILL_TO_BLOOD_QI_RATIO,
            PILL_TO_BLOOD_TOXIN_COST,
            GREAT_TRANSMUTATION_TOXIN_COST,
            GREAT_TRANSMUTATION_PENALTY_PER_USE,
        ];
        for (i, &v) in vals.iter().enumerate() {
            assert!(v > 0.0, "常量 index={i} 应为正数, got {v}");
        }
    }
}
