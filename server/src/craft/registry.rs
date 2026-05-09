//! plan-craft-v1 §3 — `CraftRegistry` resource。
//!
//! 全局配方注册表。各流派 plan（dugu-v2 / tuike-v2 / zhenfa-v2 / tools-v1）
//! 在自己 P0 阶段调 `register` 注入；本 plan 内 `mod_default::register_examples`
//! 注册 5 个示例（P1 验收基线）。
//!
//! 与 `alchemy::RecipeRegistry` 命名空间隔离：手搓走 craft 命名空间，
//! 炼丹走 alchemy 命名空间，两边 RecipeRegistry resource 都存在不冲突。

use std::collections::HashMap;

use valence::prelude::Resource;

use super::recipe::{CraftCategory, CraftRecipe, RecipeId, RecipeValidationError};

#[derive(Debug, Default)]
pub struct CraftRegistry {
    recipes: HashMap<RecipeId, CraftRecipe>,
}

impl Resource for CraftRegistry {}

#[derive(Debug, Clone, PartialEq)]
pub enum RegistryError {
    /// 配方 id 已存在 — 注册顺序由 plugin build 阶段决定，重名是 plan 间命名冲突
    DuplicateId(RecipeId),
    /// 配方自身校验失败（材料 / qi / time / unlock 等）
    Invalid(RecipeValidationError),
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateId(id) => write!(f, "duplicate recipe id `{id}`"),
            Self::Invalid(err) => write!(f, "invalid recipe: {err}"),
        }
    }
}

impl CraftRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, recipe: CraftRecipe) -> Result<(), RegistryError> {
        recipe.validate().map_err(RegistryError::Invalid)?;
        if self.recipes.contains_key(&recipe.id) {
            return Err(RegistryError::DuplicateId(recipe.id));
        }
        self.recipes.insert(recipe.id.clone(), recipe);
        Ok(())
    }

    pub fn get(&self, id: &RecipeId) -> Option<&CraftRecipe> {
        self.recipes.get(id)
    }

    pub fn len(&self) -> usize {
        self.recipes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.recipes.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &CraftRecipe> {
        self.recipes.values()
    }

    /// 按类别筛选 — UI 左列表分组的服务端起点。
    pub fn by_category(&self, category: CraftCategory) -> impl Iterator<Item = &CraftRecipe> {
        self.recipes
            .values()
            .filter(move |r| r.category == category)
    }

    /// 按 §5 决策门 #2 = A，返回排序后的 (category, recipes) 分组列表。
    /// category 顺序固定（CraftCategory::ALL），category 内按 RecipeId 字母升序。
    pub fn grouped_for_ui(&self) -> Vec<(CraftCategory, Vec<&CraftRecipe>)> {
        CraftCategory::ALL
            .iter()
            .map(|cat| {
                let mut recipes: Vec<&CraftRecipe> = self.by_category(*cat).collect();
                recipes.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
                (*cat, recipes)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::super::events::InsightTrigger;
    use super::super::recipe::{CraftCategory, CraftRecipe, CraftRequirements, UnlockSource};
    use super::*;

    fn recipe_for(id: &str, cat: CraftCategory) -> CraftRecipe {
        CraftRecipe {
            id: RecipeId::new(id),
            category: cat,
            display_name: id.into(),
            materials: vec![("herb_a".into(), 1)],
            qi_cost: 1.0,
            time_ticks: 100,
            output: ("test_out".into(), 1),
            requirements: CraftRequirements::default(),
            unlock_sources: vec![UnlockSource::Scroll {
                item_template: "scroll_x".into(),
            }],
        }
    }

    #[test]
    fn empty_registry_is_empty() {
        let r = CraftRegistry::new();
        assert!(r.is_empty());
        assert_eq!(r.len(), 0);
    }

    #[test]
    fn register_inserts_and_get_returns() {
        let mut r = CraftRegistry::new();
        let recipe = recipe_for("a", CraftCategory::Misc);
        r.register(recipe.clone()).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r.get(&RecipeId::new("a")), Some(&recipe));
    }

    #[test]
    fn register_rejects_duplicate_id() {
        let mut r = CraftRegistry::new();
        r.register(recipe_for("a", CraftCategory::Misc)).unwrap();
        let err = r
            .register(recipe_for("a", CraftCategory::Tool))
            .expect_err("duplicate must reject");
        assert!(matches!(err, RegistryError::DuplicateId(id) if id.as_str() == "a"));
    }

    #[test]
    fn register_rejects_invalid_recipe() {
        let mut r = CraftRegistry::new();
        let mut bad = recipe_for("bad", CraftCategory::Misc);
        bad.materials.clear(); // 触发 NoMaterials
        let err = r.register(bad).expect_err("invalid must reject");
        assert!(matches!(err, RegistryError::Invalid(_)));
    }

    #[test]
    fn by_category_filters_correctly() {
        let mut r = CraftRegistry::new();
        r.register(recipe_for("a1", CraftCategory::AnqiCarrier))
            .unwrap();
        r.register(recipe_for("a2", CraftCategory::AnqiCarrier))
            .unwrap();
        r.register(recipe_for("t1", CraftCategory::Tool)).unwrap();

        let anqi: Vec<&CraftRecipe> = r.by_category(CraftCategory::AnqiCarrier).collect();
        assert_eq!(anqi.len(), 2);

        let zhenfa: Vec<&CraftRecipe> = r.by_category(CraftCategory::ZhenfaTrap).collect();
        assert!(zhenfa.is_empty());
    }

    #[test]
    fn grouped_for_ui_orders_categories_then_alpha() {
        let mut r = CraftRegistry::new();
        // 故意逆序注册 + 同 category 内逆序
        r.register(recipe_for("z", CraftCategory::Tool)).unwrap();
        r.register(recipe_for("a", CraftCategory::Tool)).unwrap();
        r.register(recipe_for("anqi_x", CraftCategory::AnqiCarrier))
            .unwrap();

        let groups = r.grouped_for_ui();
        assert_eq!(groups.len(), CraftCategory::ALL.len());
        // 第 1 组应是 AnqiCarrier（CraftCategory::ALL 排第一）
        assert_eq!(groups[0].0, CraftCategory::AnqiCarrier);
        assert_eq!(groups[0].1.len(), 1);
        // Tool 在 ALL 第 5 位
        let tool_group = &groups[4];
        assert_eq!(tool_group.0, CraftCategory::Tool);
        // category 内字母升序：a 在 z 之前
        assert_eq!(tool_group.1[0].id.as_str(), "a");
        assert_eq!(tool_group.1[1].id.as_str(), "z");
    }

    #[test]
    fn unlock_source_insight_carries_trigger() {
        // 配合验证 UnlockSource::Insight 可被 register（不被当作"未绑定 trigger"误判）
        let mut r = CraftRegistry::new();
        let mut recipe = recipe_for("insight_test", CraftCategory::Misc);
        recipe.unlock_sources = vec![UnlockSource::Insight {
            trigger: InsightTrigger::Breakthrough,
        }];
        r.register(recipe).unwrap();
        assert_eq!(r.len(), 1);
    }
}
