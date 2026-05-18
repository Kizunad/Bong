//! P2 §3.4 — 变异部位锻造。
//!
//! 角强化、鳞甲淬炼、多臂协调、尾刃化。
//! 走 `forge::station` 扩展，需要对应变异 slot 才能锻造。

use super::mutation::{MutationKind, MutationState};

/// 变异部位锻造配方。
#[derive(Debug, Clone, PartialEq)]
pub struct MutationForgeRecipe {
    pub id: &'static str,
    pub name: &'static str,
    /// 需要的变异 slot（任一匹配即可）。
    pub required_mutations: &'static [MutationKind],
    /// 材料列表 (material_id, count)。
    pub materials: &'static [(&'static str, u32)],
    /// 效果描述。
    pub effect: &'static str,
    /// 锻造所需 forge station 最低 tier。
    pub station_tier_min: u8,
}

/// 角强化配方。
pub const FORGE_HORN_ENHANCE: MutationForgeRecipe = MutationForgeRecipe {
    id: "dandao_forge_horn_enhance",
    name: "角强化",
    required_mutations: &[MutationKind::Horns],
    materials: &[("beast_bone", 3), ("ling_tie", 1)],
    effect: "角冲撞伤害 +30%",
    station_tier_min: 3,
};

/// 鳞甲淬炼配方。
pub const FORGE_SCALE_QUENCH: MutationForgeRecipe = MutationForgeRecipe {
    id: "dandao_forge_scale_quench",
    name: "鳞甲淬炼",
    required_mutations: &[MutationKind::ForearmScales, MutationKind::BackCarapace],
    materials: &[("long_lin_tai", 2), ("ore", 2)],
    effect: "鳞片护甲升一档",
    station_tier_min: 3,
};

/// 多臂协调配方。
pub const FORGE_MULTI_ARM_COORD: MutationForgeRecipe = MutationForgeRecipe {
    id: "dandao_forge_multi_arm_coord",
    name: "多臂协调",
    required_mutations: &[MutationKind::ExtraArms],
    materials: &[("shou_xin_cao", 2), ("gu_yuan_gen", 1)],
    effect: "多臂武器切换速度 -50% delay",
    station_tier_min: 4,
};

/// 尾刃化配方。
pub const FORGE_TAIL_BLADE: MutationForgeRecipe = MutationForgeRecipe {
    id: "dandao_forge_tail_blade",
    name: "尾刃化",
    required_mutations: &[MutationKind::Tail],
    materials: &[("ling_tie", 2), ("tui_gu_teng", 1)],
    effect: "尾击伤害 +20% + 附带真元",
    station_tier_min: 3,
};

/// 所有变异锻造配方。
pub fn all_mutation_forge_recipes() -> [&'static MutationForgeRecipe; 4] {
    [
        &FORGE_HORN_ENHANCE,
        &FORGE_SCALE_QUENCH,
        &FORGE_MULTI_ARM_COORD,
        &FORGE_TAIL_BLADE,
    ]
}

/// 检查玩家是否有对应变异 slot 可以进行该锻造。
pub fn can_forge_mutation(recipe: &MutationForgeRecipe, state: &MutationState) -> bool {
    recipe.required_mutations.iter().any(|required_kind| {
        state
            .slots
            .iter()
            .any(|active| active.kind == *required_kind)
    })
}

/// 查找变异锻造配方 by ID。
pub fn find_mutation_forge_recipe(id: &str) -> Option<&'static MutationForgeRecipe> {
    all_mutation_forge_recipes()
        .iter()
        .find(|r| r.id == id)
        .copied()
}

#[cfg(test)]
mod mutation_forge_tests {
    use super::*;
    #[allow(unused_imports)]
    use crate::dandao::components::MutationStage;
    use crate::dandao::mutation::{ActiveMutation, MutationKind, MutationState};

    fn state_with_slots(kinds: &[MutationKind]) -> MutationState {
        MutationState {
            stage: MutationStage::Bestial,
            slots: kinds
                .iter()
                .map(|k| ActiveMutation {
                    kind: *k,
                    slot: k.body_slot(),
                    level: 1,
                    acquired_tick: 0,
                })
                .collect(),
            meridian_penalty: 0.0,
        }
    }

    #[test]
    fn all_4_recipes_present() {
        assert_eq!(
            all_mutation_forge_recipes().len(),
            4,
            "应有 4 种变异锻造配方"
        );
    }

    #[test]
    fn recipe_ids_unique() {
        let recipes = all_mutation_forge_recipes();
        let ids: Vec<&str> = recipes.iter().map(|r| r.id).collect();
        let unique: std::collections::HashSet<&str> = ids.iter().copied().collect();
        assert_eq!(ids.len(), unique.len(), "配方 ID 不应重复");
    }

    #[test]
    fn horn_enhance_requires_horns() {
        let with_horns = state_with_slots(&[MutationKind::Horns]);
        assert!(
            can_forge_mutation(&FORGE_HORN_ENHANCE, &with_horns),
            "有角应能锻造角强化"
        );

        let without = state_with_slots(&[MutationKind::Tail]);
        assert!(
            !can_forge_mutation(&FORGE_HORN_ENHANCE, &without),
            "无角不应能锻造角强化"
        );
    }

    #[test]
    fn scale_quench_accepts_either_scale_type() {
        let forearm = state_with_slots(&[MutationKind::ForearmScales]);
        assert!(
            can_forge_mutation(&FORGE_SCALE_QUENCH, &forearm),
            "前臂鳞应能淬炼"
        );

        let back = state_with_slots(&[MutationKind::BackCarapace]);
        assert!(
            can_forge_mutation(&FORGE_SCALE_QUENCH, &back),
            "背甲应能淬炼"
        );

        let neither = state_with_slots(&[MutationKind::Horns]);
        assert!(
            !can_forge_mutation(&FORGE_SCALE_QUENCH, &neither),
            "无鳞无甲不应能淬炼"
        );
    }

    #[test]
    fn multi_arm_coord_requires_extra_arms() {
        let arms = state_with_slots(&[MutationKind::ExtraArms]);
        assert!(can_forge_mutation(&FORGE_MULTI_ARM_COORD, &arms));

        let no_arms = state_with_slots(&[MutationKind::BodyEnlarge]);
        assert!(!can_forge_mutation(&FORGE_MULTI_ARM_COORD, &no_arms));
    }

    #[test]
    fn tail_blade_requires_tail() {
        let tail = state_with_slots(&[MutationKind::Tail]);
        assert!(can_forge_mutation(&FORGE_TAIL_BLADE, &tail));

        let no_tail = state_with_slots(&[MutationKind::Horns]);
        assert!(!can_forge_mutation(&FORGE_TAIL_BLADE, &no_tail));
    }

    #[test]
    fn empty_mutation_state_rejects_all() {
        let empty = MutationState::default();
        for recipe in all_mutation_forge_recipes() {
            assert!(
                !can_forge_mutation(recipe, &empty),
                "{} 不应通过空变异状态",
                recipe.id
            );
        }
    }

    #[test]
    fn find_recipe_works() {
        assert!(find_mutation_forge_recipe("dandao_forge_horn_enhance").is_some());
        assert!(find_mutation_forge_recipe("dandao_forge_tail_blade").is_some());
        assert!(find_mutation_forge_recipe("nonexistent").is_none());
    }

    #[test]
    fn all_recipes_have_materials() {
        for recipe in all_mutation_forge_recipes() {
            assert!(!recipe.materials.is_empty(), "{} 应有材料", recipe.id);
        }
    }

    #[test]
    fn station_tier_min_in_range() {
        for recipe in all_mutation_forge_recipes() {
            assert!(
                (1..=5).contains(&recipe.station_tier_min),
                "{} station_tier_min={} 应在 1-5",
                recipe.id,
                recipe.station_tier_min
            );
        }
    }

    #[test]
    fn multi_arm_requires_highest_tier() {
        assert_eq!(
            FORGE_MULTI_ARM_COORD.station_tier_min, 4,
            "多臂协调需要 tier 4 道砧"
        );
    }
}
