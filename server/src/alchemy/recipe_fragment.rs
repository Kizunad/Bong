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
