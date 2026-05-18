//! P2 §3.3 — 变异催化炉 tier 4 扩展。
//!
//! 扩展 AlchemyFurnace tier 系统，tier 4 = 变异催化炉。
//! 变异丹配方在 tier 4 炉中炼制时成功率 +20%。

use super::recipes::is_mutation_recipe;

/// 变异催化炉 tier。
pub const CATALYST_FURNACE_TIER: u8 = 4;

/// 变异催化炉对变异丹配方的成功率加成。
pub const CATALYST_MUTATION_SUCCESS_BONUS: f64 = 0.20;

/// 计算变异催化炉对给定配方的成功率加成。
/// - tier 4 炉 + 变异丹配方 → 0.20 加成
/// - tier 4 炉 + 非变异丹配方 → 0.0 加成
/// - 非 tier 4 炉 → 0.0 加成
pub fn catalyst_furnace_bonus(furnace_tier: u8, recipe_id: &str) -> f64 {
    if furnace_tier >= CATALYST_FURNACE_TIER && is_mutation_recipe(recipe_id) {
        CATALYST_MUTATION_SUCCESS_BONUS
    } else {
        0.0
    }
}

/// 变异催化炉是否可以炼制给定 tier 的配方。
/// tier 4 炉可以炼 tier 1-4 配方。
pub fn can_catalyst_furnace_brew(furnace_tier: u8, recipe_tier_min: u8) -> bool {
    furnace_tier >= recipe_tier_min
}

#[cfg(test)]
mod catalyst_tests {
    use super::*;
    use crate::dandao::recipes::*;

    #[test]
    fn catalyst_tier_is_4() {
        assert_eq!(CATALYST_FURNACE_TIER, 4, "变异催化炉应为 tier 4");
    }

    #[test]
    fn catalyst_bonus_for_mutation_recipe() {
        let bonus = catalyst_furnace_bonus(4, RECIPE_TUI_GU_DAN);
        assert!(
            (bonus - 0.20).abs() < f64::EPSILON,
            "tier 4 + 蜕骨丹应有 20% 加成, got {bonus}"
        );
    }

    #[test]
    fn catalyst_bonus_for_all_mutation_recipes() {
        for id in [
            RECIPE_TUI_GU_DAN,
            RECIPE_SHOU_XIN_DAN,
            RECIPE_LONG_LIN_SAN,
            RECIPE_HUA_XING_DA_DAN,
        ] {
            let bonus = catalyst_furnace_bonus(4, id);
            assert!(
                (bonus - 0.20).abs() < f64::EPSILON,
                "tier 4 + {id} 应有 20% 加成"
            );
        }
    }

    #[test]
    fn catalyst_no_bonus_for_non_mutation_recipe() {
        let bonus = catalyst_furnace_bonus(4, RECIPE_XU_YUAN_DAN);
        assert!(
            bonus.abs() < f64::EPSILON,
            "tier 4 + 续元丹不应有加成: got {bonus}"
        );
        let bonus2 = catalyst_furnace_bonus(4, RECIPE_JING_DU_WAN);
        assert!(
            bonus2.abs() < f64::EPSILON,
            "tier 4 + 净毒丸不应有加成: got {bonus2}"
        );
    }

    #[test]
    fn no_bonus_for_lower_tier_furnace() {
        for tier in 1..=3 {
            let bonus = catalyst_furnace_bonus(tier, RECIPE_TUI_GU_DAN);
            assert!(
                bonus.abs() < f64::EPSILON,
                "tier {tier} 不应有变异催化加成: got {bonus}"
            );
        }
    }

    #[test]
    fn tier_5_also_gets_bonus() {
        let bonus = catalyst_furnace_bonus(5, RECIPE_TUI_GU_DAN);
        assert!(
            (bonus - 0.20).abs() < f64::EPSILON,
            "tier 5 也应有变异催化加成（高阶兼容低阶）"
        );
    }

    #[test]
    fn can_brew_tier_check() {
        assert!(can_catalyst_furnace_brew(4, 1));
        assert!(can_catalyst_furnace_brew(4, 2));
        assert!(can_catalyst_furnace_brew(4, 3));
        assert!(can_catalyst_furnace_brew(4, 4));
        assert!(
            !can_catalyst_furnace_brew(4, 5),
            "tier 4 不能炼 tier 5 配方"
        );
        assert!(
            !can_catalyst_furnace_brew(3, 4),
            "tier 3 不能炼 tier 4 配方"
        );
    }

    #[test]
    fn bonus_for_unknown_recipe_is_zero() {
        let bonus = catalyst_furnace_bonus(4, "unknown_recipe");
        assert!(bonus.abs() < f64::EPSILON, "未知配方不应有加成");
    }
}
