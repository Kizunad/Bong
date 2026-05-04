//! plan-alchemy-v2 P1 — 丹方残卷与残缺学习路径。

use serde::{Deserialize, Serialize};

use super::recipe::{Recipe, RecipeId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecipeFragment {
    pub recipe_id: RecipeId,
    pub known_stages: Vec<u8>,
    pub max_quality_tier: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartialRecipeKnowledge {
    pub recipe_id: RecipeId,
    pub known_stages: Vec<u8>,
    pub max_quality_tier: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FragmentCompleteness {
    UsablePartial,
    SeverelyDamaged,
}

impl RecipeFragment {
    pub fn normalized(mut self, recipe: &Recipe) -> Self {
        self.known_stages
            .retain(|stage| usize::from(*stage) < recipe.stages.len());
        self.known_stages.sort_unstable();
        self.known_stages.dedup();
        self.max_quality_tier = self.max_quality_tier.clamp(1, 3);
        self
    }

    pub fn completeness_for_recipe(&self, recipe: &Recipe) -> FragmentCompleteness {
        let total = recipe.stages.len().max(1);
        if self.known_stages.len() * 2 >= total {
            FragmentCompleteness::UsablePartial
        } else {
            FragmentCompleteness::SeverelyDamaged
        }
    }

    pub fn learned_quality_cap(&self, recipe: &Recipe) -> u8 {
        match self.completeness_for_recipe(recipe) {
            FragmentCompleteness::UsablePartial => self.max_quality_tier.clamp(1, 3),
            FragmentCompleteness::SeverelyDamaged => 1,
        }
    }

    pub fn into_knowledge(self, recipe: &Recipe) -> PartialRecipeKnowledge {
        let normalized = self.normalized(recipe);
        let max_quality_tier = normalized.learned_quality_cap(recipe);
        PartialRecipeKnowledge {
            recipe_id: normalized.recipe_id,
            known_stages: normalized.known_stages,
            max_quality_tier,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alchemy::recipe::{
        FireProfile, IngredientSpec, Outcomes, RecipeStage, ToleranceSpec,
    };

    fn recipe_with_stage_count(count: usize) -> Recipe {
        Recipe {
            id: "kai_mai_pill_v0".to_string(),
            name: "开脉丹".to_string(),
            furnace_tier_min: 1,
            stages: (0..count)
                .map(|idx| RecipeStage {
                    at_tick: idx as u32,
                    required: vec![IngredientSpec {
                        material: format!("herb_{idx}"),
                        count: 1,
                        mineral_id: None,
                    }],
                    window: 0,
                })
                .collect(),
            fire_profile: FireProfile {
                target_temp: 0.5,
                target_duration_ticks: 20,
                qi_cost: 1.0,
                tolerance: ToleranceSpec {
                    temp_band: 0.1,
                    duration_band: 5,
                },
            },
            outcomes: Outcomes {
                perfect: None,
                good: None,
                flawed: None,
                waste: None,
                explode: None,
            },
            flawed_fallback: None,
        }
    }

    #[test]
    fn normalized_fragment_drops_unknown_stage_and_clamps_quality() {
        let recipe = recipe_with_stage_count(3);
        let fragment = RecipeFragment {
            recipe_id: recipe.id.clone(),
            known_stages: vec![2, 9, 2, 0],
            max_quality_tier: 9,
        }
        .normalized(&recipe);

        assert_eq!(fragment.known_stages, vec![0, 2]);
        assert_eq!(fragment.max_quality_tier, 3);
    }

    #[test]
    fn fragment_with_at_least_half_stages_keeps_partial_quality_cap() {
        let recipe = recipe_with_stage_count(4);
        let fragment = RecipeFragment {
            recipe_id: recipe.id.clone(),
            known_stages: vec![0, 1],
            max_quality_tier: 3,
        };

        assert_eq!(
            fragment.completeness_for_recipe(&recipe),
            FragmentCompleteness::UsablePartial
        );
        assert_eq!(fragment.learned_quality_cap(&recipe), 3);
    }

    #[test]
    fn fragment_below_half_stages_is_capped_to_tier_one() {
        let recipe = recipe_with_stage_count(4);
        let fragment = RecipeFragment {
            recipe_id: recipe.id.clone(),
            known_stages: vec![0],
            max_quality_tier: 3,
        };

        assert_eq!(
            fragment.completeness_for_recipe(&recipe),
            FragmentCompleteness::SeverelyDamaged
        );
        assert_eq!(fragment.learned_quality_cap(&recipe), 1);
    }
}
