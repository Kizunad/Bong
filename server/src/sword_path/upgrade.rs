//! plan-sword-path-v1 P1 — 灵剑品阶升级逻辑。

use crate::cultivation::components::Realm;

use super::grade::{SwordGrade, UpgradeQiCost};

fn realm_tier(r: Realm) -> u8 {
    match r {
        Realm::Awaken => 1,
        Realm::Induce => 2,
        Realm::Condense => 3,
        Realm::Solidify => 4,
        Realm::Spirit => 5,
        Realm::Void => 6,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct UpgradeRecipe {
    pub from: SwordGrade,
    pub to: SwordGrade,
    pub materials: &'static [(&'static str, u32)],
    pub qi_cost: UpgradeQiCost,
    pub time_ticks: u64,
    pub fail_chance: f32,
    pub required_station_tier: u8,
}

pub const UPGRADE_RECIPES: [UpgradeRecipe; 6] = [
    UpgradeRecipe {
        from: SwordGrade::Mortal,
        to: SwordGrade::Awakened,
        materials: &[("iron_ingot", 3), ("grass_rope", 2)],
        qi_cost: UpgradeQiCost::Fixed(0.0),
        time_ticks: 400,
        fail_chance: 0.0,
        required_station_tier: 1,
    },
    UpgradeRecipe {
        from: SwordGrade::Awakened,
        to: SwordGrade::Induced,
        materials: &[("refined_iron", 4), ("spirit_grass", 2)],
        qi_cost: UpgradeQiCost::Fixed(0.0),
        time_ticks: 800,
        fail_chance: 0.05,
        required_station_tier: 1,
    },
    UpgradeRecipe {
        from: SwordGrade::Induced,
        to: SwordGrade::Condensed,
        materials: &[
            ("xuan_iron", 5),
            ("spirit_spring_water", 3),
            ("beast_bone", 2),
        ],
        qi_cost: UpgradeQiCost::Fixed(5.0),
        time_ticks: 1600,
        fail_chance: 0.15,
        required_station_tier: 1,
    },
    UpgradeRecipe {
        from: SwordGrade::Condensed,
        to: SwordGrade::Solidified,
        materials: &[
            ("meteor_iron", 4),
            ("spirit_wood_core", 2),
            ("sword_embryo_shard", 1),
        ],
        qi_cost: UpgradeQiCost::Fixed(30.0),
        time_ticks: 3200,
        fail_chance: 0.25,
        required_station_tier: 2,
    },
    UpgradeRecipe {
        from: SwordGrade::Solidified,
        to: SwordGrade::Spirit,
        materials: &[
            ("star_iron", 3),
            ("ancient_sword_embryo", 1),
            ("spirit_spring_essence", 2),
        ],
        qi_cost: UpgradeQiCost::Fixed(150.0),
        time_ticks: 6400,
        fail_chance: 0.35,
        required_station_tier: 3,
    },
    UpgradeRecipe {
        from: SwordGrade::Spirit,
        to: SwordGrade::Void,
        materials: &[("sky_meteor_iron", 2), ("broken_sword_soul", 1)],
        qi_cost: UpgradeQiCost::All,
        time_ticks: 12800,
        fail_chance: 0.50,
        required_station_tier: 3,
    },
];

pub fn recipe_for(from: SwordGrade) -> Option<&'static UpgradeRecipe> {
    UPGRADE_RECIPES.iter().find(|r| r.from == from)
}

pub const FAIL_MATERIAL_LOSS_RATIO: f32 = 0.50;
pub const FAIL_DURABILITY_LOSS_RATIO: f32 = 0.30;

#[derive(Debug, Clone, PartialEq)]
pub enum UpgradeCheckResult {
    Ok,
    NoRecipe,
    RealmTooLow,
    StationTierTooLow { need: u8, have: u8 },
    InsufficientQi { need: f64, have: f64 },
    MissingMaterials(Vec<(&'static str, u32)>),
}

pub fn check_upgrade(
    current_grade: SwordGrade,
    player_realm_tier: u8,
    station_tier: u8,
    qi_current: f64,
    has_material: impl Fn(&str) -> u32,
) -> UpgradeCheckResult {
    let Some(recipe) = recipe_for(current_grade) else {
        return UpgradeCheckResult::NoRecipe;
    };

    let required_realm_tier = realm_tier(recipe.to.required_realm());
    if player_realm_tier < required_realm_tier {
        return UpgradeCheckResult::RealmTooLow;
    }

    if station_tier < recipe.required_station_tier {
        return UpgradeCheckResult::StationTierTooLow {
            need: recipe.required_station_tier,
            have: station_tier,
        };
    }

    let qi_need = recipe.qi_cost.resolve(qi_current);
    if qi_current < qi_need {
        return UpgradeCheckResult::InsufficientQi {
            need: qi_need,
            have: qi_current,
        };
    }

    let mut missing = Vec::new();
    for &(mat_id, count) in recipe.materials {
        let have = has_material(mat_id);
        if have < count {
            missing.push((mat_id, count - have));
        }
    }
    if !missing.is_empty() {
        return UpgradeCheckResult::MissingMaterials(missing);
    }

    UpgradeCheckResult::Ok
}

#[derive(Debug, Clone, PartialEq)]
pub struct UpgradeOutcome {
    pub success: bool,
    pub new_grade: SwordGrade,
    pub materials_consumed: Vec<(&'static str, u32)>,
    pub qi_consumed: f64,
    pub stored_qi_lost: f64,
    pub durability_loss_ratio: f32,
}

pub fn resolve_upgrade(
    recipe: &UpgradeRecipe,
    qi_current: f64,
    stored_qi: f64,
    roll: f32,
) -> UpgradeOutcome {
    let qi_consumed = recipe.qi_cost.resolve(qi_current);
    let success = roll >= recipe.fail_chance;

    if success {
        UpgradeOutcome {
            success: true,
            new_grade: recipe.to,
            materials_consumed: recipe.materials.to_vec(),
            qi_consumed,
            stored_qi_lost: 0.0,
            durability_loss_ratio: 0.0,
        }
    } else {
        let partial_materials: Vec<(&str, u32)> = recipe
            .materials
            .iter()
            .map(|&(id, count)| {
                (
                    id,
                    ((count as f32 * FAIL_MATERIAL_LOSS_RATIO).ceil() as u32).max(1),
                )
            })
            .collect();
        UpgradeOutcome {
            success: false,
            new_grade: recipe.from,
            materials_consumed: partial_materials,
            qi_consumed,
            stored_qi_lost: stored_qi,
            durability_loss_ratio: FAIL_DURABILITY_LOSS_RATIO,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::realm_tier;
    use super::*;
    use crate::cultivation::components::Realm;

    #[test]
    fn all_six_recipes_exist() {
        assert_eq!(UPGRADE_RECIPES.len(), 6);
    }

    #[test]
    fn recipe_chain_covers_all_grades() {
        let mut grade = SwordGrade::Mortal;
        let mut count = 0;
        while let Some(recipe) = recipe_for(grade) {
            assert_eq!(recipe.from, grade);
            grade = recipe.to;
            count += 1;
        }
        assert_eq!(count, 6, "should chain through 6 upgrades");
        assert_eq!(grade, SwordGrade::Void, "should end at Void");
    }

    #[test]
    fn no_recipe_for_void() {
        assert!(recipe_for(SwordGrade::Void).is_none());
    }

    #[test]
    fn first_two_upgrades_zero_qi() {
        let r0 = recipe_for(SwordGrade::Mortal).unwrap();
        let r1 = recipe_for(SwordGrade::Awakened).unwrap();
        assert_eq!(
            r0.qi_cost,
            UpgradeQiCost::Fixed(0.0),
            "0→1 should cost 0 qi"
        );
        assert_eq!(
            r1.qi_cost,
            UpgradeQiCost::Fixed(0.0),
            "1→2 should cost 0 qi"
        );
    }

    #[test]
    fn solidify_upgrade_needs_tier_2_station() {
        let r = recipe_for(SwordGrade::Condensed).unwrap();
        assert_eq!(r.required_station_tier, 2);
    }

    #[test]
    fn spirit_upgrade_needs_tier_3_station() {
        let r = recipe_for(SwordGrade::Solidified).unwrap();
        assert_eq!(r.required_station_tier, 3);
    }

    #[test]
    fn void_upgrade_costs_all_qi() {
        let r = recipe_for(SwordGrade::Spirit).unwrap();
        assert_eq!(r.qi_cost, UpgradeQiCost::All);
    }

    #[test]
    fn fail_chance_monotonic() {
        let chances: Vec<f32> = UPGRADE_RECIPES.iter().map(|r| r.fail_chance).collect();
        for pair in chances.windows(2) {
            assert!(
                pair[1] >= pair[0],
                "fail_chance should be non-decreasing: {} -> {}",
                pair[0],
                pair[1]
            );
        }
    }

    #[test]
    fn time_ticks_monotonic() {
        let times: Vec<u64> = UPGRADE_RECIPES.iter().map(|r| r.time_ticks).collect();
        for pair in times.windows(2) {
            assert!(
                pair[1] >= pair[0],
                "time_ticks should be non-decreasing: {} -> {}",
                pair[0],
                pair[1]
            );
        }
    }

    #[test]
    fn check_upgrade_ok() {
        let result = check_upgrade(SwordGrade::Mortal, 1, 1, 100.0, |_| 10);
        assert_eq!(result, UpgradeCheckResult::Ok);
    }

    #[test]
    fn check_upgrade_no_recipe_for_void() {
        let result = check_upgrade(SwordGrade::Void, 5, 3, 100.0, |_| 10);
        assert_eq!(result, UpgradeCheckResult::NoRecipe);
    }

    #[test]
    fn check_upgrade_realm_too_low() {
        let result = check_upgrade(SwordGrade::Condensed, 2, 2, 100.0, |_| 10);
        assert_eq!(result, UpgradeCheckResult::RealmTooLow);
    }

    #[test]
    fn check_upgrade_station_too_low() {
        let result = check_upgrade(SwordGrade::Condensed, 4, 1, 100.0, |_| 10);
        assert_eq!(
            result,
            UpgradeCheckResult::StationTierTooLow { need: 2, have: 1 }
        );
    }

    #[test]
    fn check_upgrade_missing_materials() {
        let result = check_upgrade(SwordGrade::Mortal, 1, 1, 100.0, |id| {
            if id == "iron_ingot" {
                1
            } else {
                0
            }
        });
        match result {
            UpgradeCheckResult::MissingMaterials(missing) => {
                assert!(
                    missing.iter().any(|(id, _)| *id == "iron_ingot"),
                    "should report iron_ingot shortage"
                );
                assert!(
                    missing.iter().any(|(id, _)| *id == "grass_rope"),
                    "should report grass_rope shortage"
                );
            }
            other => panic!("expected MissingMaterials, got {other:?}"),
        }
    }

    #[test]
    fn check_upgrade_insufficient_qi() {
        let result = check_upgrade(SwordGrade::Induced, 3, 1, 2.0, |_| 10);
        assert_eq!(
            result,
            UpgradeCheckResult::InsufficientQi {
                need: 5.0,
                have: 2.0,
            },
            "引→凝 needs qi=5 but player has 2"
        );
    }

    #[test]
    fn resolve_upgrade_success() {
        let recipe = recipe_for(SwordGrade::Mortal).unwrap();
        let out = resolve_upgrade(recipe, 10.0, 0.0, 0.5);
        assert!(out.success);
        assert_eq!(out.new_grade, SwordGrade::Awakened);
        assert_eq!(out.durability_loss_ratio, 0.0);
        assert_eq!(out.stored_qi_lost, 0.0);
    }

    #[test]
    fn resolve_upgrade_fail() {
        let recipe = recipe_for(SwordGrade::Induced).unwrap();
        let out = resolve_upgrade(recipe, 100.0, 10.0, 0.05);
        assert!(!out.success, "roll 0.05 < fail_chance 0.15 should fail");
        assert_eq!(
            out.new_grade,
            SwordGrade::Induced,
            "grade should not change on fail"
        );
        assert!(
            (out.stored_qi_lost - 10.0).abs() < 1e-6,
            "stored_qi should be lost on fail"
        );
        assert!(
            (out.durability_loss_ratio - 0.30).abs() < 1e-6,
            "durability loss should be 30%"
        );
    }

    #[test]
    fn resolve_upgrade_fail_partial_materials() {
        let recipe = recipe_for(SwordGrade::Induced).unwrap();
        let out = resolve_upgrade(recipe, 100.0, 0.0, 0.01);
        assert!(!out.success);
        for (id, count) in &out.materials_consumed {
            let original = recipe
                .materials
                .iter()
                .find(|(mid, _)| mid == id)
                .unwrap()
                .1;
            let expected = ((original as f32 * 0.5).ceil() as u32).max(1);
            assert_eq!(
                *count, expected,
                "failed upgrade should consume ceil(50%) of {id}: expected {expected}, got {count}"
            );
        }
    }

    #[test]
    fn resolve_void_upgrade_consumes_all_qi() {
        let recipe = recipe_for(SwordGrade::Spirit).unwrap();
        let out = resolve_upgrade(recipe, 2100.0, 500.0, 0.8);
        assert!(out.success);
        assert!(
            (out.qi_consumed - 2100.0).abs() < 1e-6,
            "void upgrade should consume all qi: expected 2100, got {}",
            out.qi_consumed
        );
    }

    #[test]
    fn realm_tier_helper_values() {
        assert_eq!(realm_tier(Realm::Awaken), 1);
        assert_eq!(realm_tier(Realm::Induce), 2);
        assert_eq!(realm_tier(Realm::Condense), 3);
        assert_eq!(realm_tier(Realm::Solidify), 4);
        assert_eq!(realm_tier(Realm::Spirit), 5);
        assert_eq!(realm_tier(Realm::Void), 6);
    }
}
