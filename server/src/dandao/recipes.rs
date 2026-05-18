//! P2 §3.2 — 丹道专属丹药配方（6 种）。
//!
//! 蜕骨丹、兽心丹、龙鳞散、续元丹、化形大丹、净毒丸。
//! 定义配方规格，JSON 配方文件由本模块数据对齐。

/// 丹道专属丹药配方 ID（与 server/assets/alchemy/recipes/*.json 文件名对齐）。
pub const RECIPE_TUI_GU_DAN: &str = "tui_gu_dan_v1";
pub const RECIPE_SHOU_XIN_DAN: &str = "shou_xin_dan_v1";
pub const RECIPE_LONG_LIN_SAN: &str = "long_lin_san_v1";
pub const RECIPE_XU_YUAN_DAN: &str = "xu_yuan_dan_v1";
pub const RECIPE_HUA_XING_DA_DAN: &str = "hua_xing_da_dan_v1";
pub const RECIPE_JING_DU_WAN: &str = "jing_du_wan_v1";

/// 丹药出品 ID（pill item_id）。
pub const PILL_TUI_GU_DAN: &str = "dandao_tui_gu_dan";
pub const PILL_SHOU_XIN_DAN: &str = "dandao_shou_xin_dan";
pub const PILL_LONG_LIN_SAN: &str = "dandao_long_lin_san";
pub const PILL_XU_YUAN_DAN: &str = "dandao_xu_yuan_dan";
pub const PILL_HUA_XING_DA_DAN: &str = "dandao_hua_xing_da_dan";
pub const PILL_JING_DU_WAN: &str = "dandao_jing_du_wan";

/// 丹道配方规格（plan §3.2 表对齐）。
#[derive(Debug, Clone, PartialEq)]
pub struct DandaoRecipeSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub pill_id: &'static str,
    /// ContamSource amount（丹毒量）。
    pub toxin_amount: f64,
    /// 最低炉阶。
    pub furnace_tier_min: u8,
    /// 品阶范围 [min, max]。
    pub quality_range: (u8, u8),
    /// 核心材料列表 (material_id, count)。
    pub ingredients: &'static [(&'static str, u32)],
    /// 效果描述。
    pub effect: &'static str,
}

/// 6 种丹道专属丹药配方。
pub fn dandao_recipes() -> [DandaoRecipeSpec; 6] {
    [
        DandaoRecipeSpec {
            id: RECIPE_TUI_GU_DAN,
            name: "蜕骨丹",
            pill_id: PILL_TUI_GU_DAN,
            toxin_amount: 5.0,
            furnace_tier_min: 2,
            quality_range: (2, 4),
            ingredients: &[("tui_gu_teng", 2), ("beast_bone", 1)],
            effect: "加速骨系变异进度 (+5.0 cumulative_toxin)",
        },
        DandaoRecipeSpec {
            id: RECIPE_SHOU_XIN_DAN,
            name: "兽心丹",
            pill_id: PILL_SHOU_XIN_DAN,
            toxin_amount: 3.0,
            furnace_tier_min: 2,
            quality_range: (2, 4),
            ingredients: &[("shou_xin_cao", 2), ("mutation_core", 1)],
            effect: "体质增幅 HP +10% 持续 1h + 肌肉系变异进度",
        },
        DandaoRecipeSpec {
            id: RECIPE_LONG_LIN_SAN,
            name: "龙鳞散",
            pill_id: PILL_LONG_LIN_SAN,
            toxin_amount: 4.0,
            furnace_tier_min: 3,
            quality_range: (3, 5),
            ingredients: &[("long_lin_tai", 3), ("ling_tie_fen", 1)],
            effect: "天然护甲增幅 30min + 鳞甲系变异进度",
        },
        DandaoRecipeSpec {
            id: RECIPE_XU_YUAN_DAN,
            name: "续元丹",
            pill_id: PILL_XU_YUAN_DAN,
            toxin_amount: 1.5,
            furnace_tier_min: 3,
            quality_range: (3, 5),
            ingredients: &[("xu_yuan_rui", 1), ("hui_yuan_zhi", 2)],
            effect: "寿元恢复 +50 年",
        },
        DandaoRecipeSpec {
            id: RECIPE_HUA_XING_DA_DAN,
            name: "化形大丹",
            pill_id: PILL_HUA_XING_DA_DAN,
            toxin_amount: 30.0,
            furnace_tier_min: 4,
            quality_range: (4, 5),
            ingredients: &[
                ("hua_xing_gen", 1),
                ("tui_gu_teng", 1),
                ("shou_xin_cao", 1),
                ("long_lin_tai", 1),
            ],
            effect: "直接触发下一变异阶段",
        },
        DandaoRecipeSpec {
            id: RECIPE_JING_DU_WAN,
            name: "净毒丸",
            pill_id: PILL_JING_DU_WAN,
            toxin_amount: 0.0,
            furnace_tier_min: 1,
            quality_range: (1, 3),
            ingredients: &[("hui_yuan_zhi", 3), ("gu_yuan_gen", 2)],
            effect: "降低当前 contamination 50%（不降 cumulative_toxin）",
        },
    ]
}

/// 查找配方 by ID。
pub fn find_recipe(id: &str) -> Option<&'static DandaoRecipeSpec> {
    // Use leaked static for ergonomic lifetime.
    // In practice the recipes are compile-time constant.
    static RECIPES: std::sync::OnceLock<[DandaoRecipeSpec; 6]> = std::sync::OnceLock::new();
    let recipes = RECIPES.get_or_init(dandao_recipes);
    recipes.iter().find(|r| r.id == id)
}

/// 检查是否为丹道变异丹（享受变异催化炉加成的配方）。
pub fn is_mutation_recipe(recipe_id: &str) -> bool {
    matches!(
        recipe_id,
        RECIPE_TUI_GU_DAN | RECIPE_SHOU_XIN_DAN | RECIPE_LONG_LIN_SAN | RECIPE_HUA_XING_DA_DAN
    )
}

#[cfg(test)]
mod recipe_tests {
    use super::*;

    #[test]
    fn all_6_recipes_present() {
        let recipes = dandao_recipes();
        assert_eq!(recipes.len(), 6, "丹道专属丹药应有 6 种");
    }

    #[test]
    fn recipe_ids_unique() {
        let recipes = dandao_recipes();
        let ids: Vec<&str> = recipes.iter().map(|r| r.id).collect();
        let unique: std::collections::HashSet<&str> = ids.iter().copied().collect();
        assert_eq!(ids.len(), unique.len(), "配方 ID 不应重复");
    }

    #[test]
    fn pill_ids_unique() {
        let recipes = dandao_recipes();
        let ids: Vec<&str> = recipes.iter().map(|r| r.pill_id).collect();
        let unique: std::collections::HashSet<&str> = ids.iter().copied().collect();
        assert_eq!(ids.len(), unique.len(), "丹药 ID 不应重复");
    }

    #[test]
    fn toxin_amounts_match_plan() {
        let recipes = dandao_recipes();
        // plan §3.2: 蜕骨丹 5.0, 兽心丹 3.0, 龙鳞散 4.0, 续元丹 1.5, 化形大丹 30.0, 净毒丸 0.0
        assert!(
            (recipes[0].toxin_amount - 5.0).abs() < f64::EPSILON,
            "蜕骨丹 toxin=5.0"
        );
        assert!(
            (recipes[1].toxin_amount - 3.0).abs() < f64::EPSILON,
            "兽心丹 toxin=3.0"
        );
        assert!(
            (recipes[2].toxin_amount - 4.0).abs() < f64::EPSILON,
            "龙鳞散 toxin=4.0"
        );
        assert!(
            (recipes[3].toxin_amount - 1.5).abs() < f64::EPSILON,
            "续元丹 toxin=1.5"
        );
        assert!(
            (recipes[4].toxin_amount - 30.0).abs() < f64::EPSILON,
            "化形大丹 toxin=30.0"
        );
        assert!(
            (recipes[5].toxin_amount - 0.0).abs() < f64::EPSILON,
            "净毒丸 toxin=0.0"
        );
    }

    #[test]
    fn jing_du_wan_has_zero_toxin() {
        let recipes = dandao_recipes();
        let jdw = recipes.iter().find(|r| r.id == RECIPE_JING_DU_WAN).unwrap();
        assert!(
            jdw.toxin_amount.abs() < f64::EPSILON,
            "净毒丸不应有丹毒: got {}",
            jdw.toxin_amount
        );
    }

    #[test]
    fn hua_xing_da_dan_requires_all_4_herbs() {
        let recipes = dandao_recipes();
        let hxdd = recipes
            .iter()
            .find(|r| r.id == RECIPE_HUA_XING_DA_DAN)
            .unwrap();
        assert_eq!(hxdd.ingredients.len(), 4, "化形大丹需要 4 种材料");
        let materials: Vec<&str> = hxdd.ingredients.iter().map(|(m, _)| *m).collect();
        assert!(materials.contains(&"hua_xing_gen"), "化形大丹需要化形根");
        assert!(materials.contains(&"tui_gu_teng"), "化形大丹需要蜕骨藤");
        assert!(materials.contains(&"shou_xin_cao"), "化形大丹需要兽心草");
        assert!(materials.contains(&"long_lin_tai"), "化形大丹需要龙鳞苔");
    }

    #[test]
    fn hua_xing_da_dan_highest_toxin() {
        let recipes = dandao_recipes();
        let hxdd = recipes
            .iter()
            .find(|r| r.id == RECIPE_HUA_XING_DA_DAN)
            .unwrap();
        for recipe in &recipes {
            assert!(
                hxdd.toxin_amount >= recipe.toxin_amount,
                "化形大丹应是最高丹毒: {} vs {} ({})",
                hxdd.toxin_amount,
                recipe.toxin_amount,
                recipe.id
            );
        }
    }

    #[test]
    fn hua_xing_da_dan_requires_tier_4() {
        let recipes = dandao_recipes();
        let hxdd = recipes
            .iter()
            .find(|r| r.id == RECIPE_HUA_XING_DA_DAN)
            .unwrap();
        assert_eq!(hxdd.furnace_tier_min, 4, "化形大丹需要 tier 4 变异催化炉");
    }

    #[test]
    fn furnace_tier_min_in_valid_range() {
        for recipe in dandao_recipes() {
            assert!(
                (1..=5).contains(&recipe.furnace_tier_min),
                "{} furnace_tier_min={} 应在 1-5",
                recipe.id,
                recipe.furnace_tier_min
            );
        }
    }

    #[test]
    fn quality_range_min_leq_max() {
        for recipe in dandao_recipes() {
            assert!(
                recipe.quality_range.0 <= recipe.quality_range.1,
                "{} quality_range {:?} min > max",
                recipe.id,
                recipe.quality_range
            );
        }
    }

    #[test]
    fn all_recipes_have_ingredients() {
        for recipe in dandao_recipes() {
            assert!(
                !recipe.ingredients.is_empty(),
                "{} 应有至少 1 种材料",
                recipe.id
            );
        }
    }

    #[test]
    fn find_recipe_works() {
        assert!(find_recipe(RECIPE_TUI_GU_DAN).is_some());
        assert!(find_recipe(RECIPE_JING_DU_WAN).is_some());
        assert!(find_recipe("nonexistent").is_none());
    }

    #[test]
    fn is_mutation_recipe_correct() {
        assert!(is_mutation_recipe(RECIPE_TUI_GU_DAN), "蜕骨丹是变异丹");
        assert!(is_mutation_recipe(RECIPE_SHOU_XIN_DAN), "兽心丹是变异丹");
        assert!(is_mutation_recipe(RECIPE_LONG_LIN_SAN), "龙鳞散是变异丹");
        assert!(
            is_mutation_recipe(RECIPE_HUA_XING_DA_DAN),
            "化形大丹是变异丹"
        );
        assert!(!is_mutation_recipe(RECIPE_XU_YUAN_DAN), "续元丹不是变异丹");
        assert!(!is_mutation_recipe(RECIPE_JING_DU_WAN), "净毒丸不是变异丹");
    }

    #[test]
    fn xu_yuan_dan_uses_hui_yuan_zhi() {
        let recipes = dandao_recipes();
        let xyd = recipes.iter().find(|r| r.id == RECIPE_XU_YUAN_DAN).unwrap();
        let materials: Vec<&str> = xyd.ingredients.iter().map(|(m, _)| *m).collect();
        assert!(materials.contains(&"hui_yuan_zhi"), "续元丹需要回元芷");
        assert!(materials.contains(&"xu_yuan_rui"), "续元丹需要续元蕊");
    }

    #[test]
    fn jing_du_wan_uses_existing_herbs() {
        let recipes = dandao_recipes();
        let jdw = recipes.iter().find(|r| r.id == RECIPE_JING_DU_WAN).unwrap();
        let materials: Vec<&str> = jdw.ingredients.iter().map(|(m, _)| *m).collect();
        assert!(materials.contains(&"hui_yuan_zhi"), "净毒丸需要回元芷");
        assert!(materials.contains(&"gu_yuan_gen"), "净毒丸需要固元根");
    }
}
