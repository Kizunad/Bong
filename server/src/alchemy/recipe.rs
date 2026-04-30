//! 配方系统（plan-alchemy-v1 §1.1）— 纯数据，JSON 加载。
//!
//! 启动期扫 `server/assets/alchemy/recipes/*.json` → `RecipeRegistry` resource。
//! NPC / agent 未来可读同一张表做自动炼丹。

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use valence::prelude::Resource;

use crate::cultivation::components::ColorKind;
use crate::inventory::ItemInstance;
use crate::mineral::{build_default_registry as build_default_mineral_registry, MineralRegistry};

/// 单个配方 ID — 与 JSON `id` 字段一致。
pub type RecipeId = String;

/// 配方段（plan §1.3 中途投料）。`at_tick=0` 即起炉投料。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecipeStage {
    pub at_tick: u32,
    pub required: Vec<IngredientSpec>,
    /// window=0 表示该段必须在首 tick（起炉时）投入；否则在 `[at_tick, at_tick+window]` 内投。
    #[serde(default)]
    pub window: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct IngredientSpec {
    pub material: String,
    pub count: u32,
    /// plan-mineral-v1 §6 — 矿物辅料字段。当 `Some(...)` 时该 ingredient 为矿物，
    /// consume 时必须用 inventory item NBT `mineral_id == 此值` 的物品；
    /// 同时 `material` 字段仍参与精确匹配（用于配方查找的 key），但视作矿物 alias。
    /// `None` 保持现有 botany 草药 / 凡俗物品行为不变。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mineral_id: Option<String>,
}

impl IngredientSpec {
    /// plan-mineral-v1 §6 — 校验 inventory item NBT mineral_id 是否满足本 ingredient。
    /// `None` ingredient.mineral_id → 任何 item 都通过（不要求矿物来源）。
    /// `Some(req)` → item.mineral_id 必须等于 req。
    pub fn matches_mineral(&self, item_mineral_id: Option<&str>) -> bool {
        match (&self.mineral_id, item_mineral_id) {
            (None, _) => true,
            (Some(_), None) => false,
            (Some(req), Some(got)) => req == got,
        }
    }

    pub fn validate_item(&self, item: &ItemInstance) -> Result<(), IngredientMismatch> {
        if self.matches_mineral(item.mineral_id.as_deref()) {
            Ok(())
        } else if item.mineral_id.is_none() {
            Err(IngredientMismatch::MissingMineralId {
                material: self.material.clone(),
            })
        } else {
            Err(IngredientMismatch::WrongMineralId {
                material: self.material.clone(),
                expected: self.mineral_id.clone().unwrap_or_default(),
                got: item.mineral_id.clone().unwrap_or_default(),
            })
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngredientMismatch {
    MissingMineralId {
        material: String,
    },
    WrongMineralId {
        material: String,
        expected: String,
        got: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToleranceSpec {
    pub temp_band: f64,
    pub duration_band: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FireProfile {
    pub target_temp: f64,
    pub target_duration_ticks: u32,
    pub qi_cost: f64,
    pub tolerance: ToleranceSpec,
}

/// 五结果桶中的一个成丹桶（perfect/good/flawed 共用此 shape）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PillOutcome {
    pub pill: String,
    pub quality: f64,
    pub toxin_amount: f64,
    pub toxin_color: ColorKind,
    /// 可选：服下后恢复的 qi（plan §3.2 回元丹用）。
    #[serde(default)]
    pub qi_gain: Option<f64>,
}

/// 炸炉后果。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExplodeOutcome {
    pub damage: f64,
    pub meridian_crack: f64,
}

/// 完整 outcomes 桶 — waste/explode 是 null-able 的（plan §1.3）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Outcomes {
    #[serde(default)]
    pub perfect: Option<PillOutcome>,
    #[serde(default)]
    pub good: Option<PillOutcome>,
    #[serde(default)]
    pub flawed: Option<PillOutcome>,
    /// waste 始终 null（配方里也写 null）— 材料全失，无产出。
    #[serde(default)]
    pub waste: Option<serde_json::Value>,
    #[serde(default)]
    pub explode: Option<ExplodeOutcome>,
}

/// 副作用池条目（plan §1.3 残缺匹配抽取）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SideEffect {
    pub tag: String,
    /// duration=0 表示瞬发 / 一次性。
    #[serde(default)]
    pub duration_s: u32,
    #[serde(default)]
    pub weight: u32,
    /// 永久效果标记（如 qi_cap_perm_minus_1）。
    #[serde(default)]
    pub perm: bool,
    /// 可选：指定施加的真元色（如 random_color_shift）。
    #[serde(default)]
    pub color: Option<ColorKind>,
    #[serde(default)]
    pub amount: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FlawedFallback {
    pub pill: String,
    pub quality_scale: f64,
    pub toxin_scale: f64,
    #[serde(default)]
    pub side_effect_pool: Vec<SideEffect>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Recipe {
    pub id: RecipeId,
    pub name: String,
    pub furnace_tier_min: u8,
    pub stages: Vec<RecipeStage>,
    pub fire_profile: FireProfile,
    pub outcomes: Outcomes,
    #[serde(default)]
    pub flawed_fallback: Option<FlawedFallback>,
}

impl Recipe {
    /// 配方首 tick（at_tick=0）所需材料的聚合视图。
    /// 投料精确匹配用。
    pub fn stage0_ingredients(&self) -> HashMap<String, u32> {
        let mut map = HashMap::new();
        if let Some(stage) = self.stages.iter().find(|s| s.at_tick == 0) {
            for ing in &stage.required {
                *map.entry(ing.material.clone()).or_insert(0) += ing.count;
            }
        }
        map
    }

    /// 全阶段材料总需求（plan §1.3 残缺匹配用）。
    pub fn all_ingredients(&self) -> HashMap<String, u32> {
        let mut map = HashMap::new();
        for stage in &self.stages {
            for ing in &stage.required {
                *map.entry(ing.material.clone()).or_insert(0) += ing.count;
            }
        }
        map
    }
}

#[derive(Debug, Default)]
pub struct RecipeRegistry {
    recipes: HashMap<RecipeId, Recipe>,
}

impl Resource for RecipeRegistry {}

impl RecipeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, recipe: Recipe) -> Result<(), String> {
        let id = recipe.id.clone();
        if self.recipes.insert(id.clone(), recipe).is_some() {
            return Err(format!("duplicate recipe id `{id}`"));
        }
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&Recipe> {
        self.recipes.get(id)
    }

    pub fn len(&self) -> usize {
        self.recipes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.recipes.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Recipe> {
        self.recipes.values()
    }

    /// plan §1.3 精确匹配：材料集合 == stage0 需求。
    pub fn match_exact(&self, staged: &HashMap<String, u32>) -> Option<&Recipe> {
        self.recipes
            .values()
            .find(|r| r.stage0_ingredients() == *staged)
    }

    /// plan §1.3 残缺匹配：投入是配方 stage0 的严格子集，且存在 flawed_fallback。
    /// 返回命中的 recipe + 缺失比例（0..1，越大越残缺）。
    pub fn match_flawed(&self, staged: &HashMap<String, u32>) -> Option<(&Recipe, f64)> {
        if staged.is_empty() {
            return None;
        }
        let mut best: Option<(&Recipe, f64)> = None;
        for recipe in self.recipes.values() {
            if recipe.flawed_fallback.is_none() {
                continue;
            }
            let need = recipe.stage0_ingredients();
            if need.is_empty() {
                continue;
            }
            // staged 必须是 need 的 (材料, count) 子集
            let mut is_subset = true;
            let mut matched_count: u32 = 0;
            let mut total_need: u32 = 0;
            for (mat, &n) in &need {
                total_need += n;
                match staged.get(mat) {
                    Some(&s) if s > n => {
                        is_subset = false;
                        break;
                    }
                    Some(&s) => matched_count += s,
                    None => {}
                }
            }
            if !is_subset {
                continue;
            }
            // staged 里不能有 need 之外的材料
            if staged.keys().any(|k| !need.contains_key(k)) {
                continue;
            }
            if matched_count == total_need {
                // 实际是精确匹配，不走残缺
                continue;
            }
            let missing_ratio = 1.0 - (matched_count as f64 / total_need as f64);
            match best {
                None => best = Some((recipe, missing_ratio)),
                Some((_, cur)) if missing_ratio < cur => best = Some((recipe, missing_ratio)),
                _ => {}
            }
        }
        best
    }
}

const DEFAULT_RECIPES_DIR: &str = "assets/alchemy/recipes";

pub fn load_recipe_registry() -> Result<RecipeRegistry, String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_RECIPES_DIR);
    load_recipe_registry_from_dir(path)
}

pub fn load_recipe_registry_from_dir(path: impl AsRef<Path>) -> Result<RecipeRegistry, String> {
    let minerals = build_default_mineral_registry();
    load_recipe_registry_from_dir_with_minerals(path, Some(&minerals))
}

pub fn load_recipe_registry_from_dir_with_minerals(
    path: impl AsRef<Path>,
    minerals: Option<&MineralRegistry>,
) -> Result<RecipeRegistry, String> {
    let path = path.as_ref();
    let entries = fs::read_dir(path).map_err(|error| {
        format!(
            "failed to read alchemy recipe directory {}: {error}",
            path.display()
        )
    })?;

    let mut json_paths: Vec<PathBuf> = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let file_path = entry.path();
            let is_json = file_path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("json"));
            is_json.then_some(file_path)
        })
        .collect();
    json_paths.sort();

    if json_paths.is_empty() {
        return Err(format!(
            "alchemy recipe directory {} contains no *.json files",
            path.display()
        ));
    }

    let mut registry = RecipeRegistry::new();
    for json_path in json_paths {
        let content = fs::read_to_string(&json_path)
            .map_err(|error| format!("failed to read {}: {error}", json_path.display()))?;
        let recipe: Recipe = serde_json::from_str(&content)
            .map_err(|error| format!("failed to parse recipe {}: {error}", json_path.display()))?;
        if let Some(minerals) = minerals {
            validate_recipe_minerals(&recipe, minerals).map_err(|error| {
                format!(
                    "failed to validate recipe minerals from {}: {error}",
                    json_path.display()
                )
            })?;
        }
        registry.insert(recipe).map_err(|error| {
            format!(
                "failed to register recipe from {}: {error}",
                json_path.display()
            )
        })?;
    }
    Ok(registry)
}

pub fn validate_recipe_minerals(recipe: &Recipe, minerals: &MineralRegistry) -> Result<(), String> {
    for stage in &recipe.stages {
        for ingredient in &stage.required {
            let Some(mineral_id) = ingredient.mineral_id.as_deref() else {
                continue;
            };
            if !minerals.is_valid_mineral_id(mineral_id) {
                return Err(format!(
                    "recipe `{}` ingredient `{}` references unknown mineral_id `{}`",
                    recipe.id, ingredient.material, mineral_id
                ));
            }
        }
    }
    Ok(())
}

pub fn validate_stage_mineral_items<'a>(
    ingredients: impl IntoIterator<Item = &'a IngredientSpec>,
    items: impl IntoIterator<Item = &'a ItemInstance>,
) -> Result<(), IngredientMismatch> {
    let mut remaining: Vec<&ItemInstance> = items.into_iter().collect();
    for ingredient in ingredients {
        if ingredient.mineral_id.is_none() {
            continue;
        }
        let Some(index) = remaining
            .iter()
            .position(|item| item.template_id == ingredient.material || item.mineral_id.is_some())
        else {
            return Err(IngredientMismatch::MissingMineralId {
                material: ingredient.material.clone(),
            });
        };
        let item = remaining.remove(index);
        ingredient.validate_item(item)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_recipe() -> Recipe {
        Recipe {
            id: "test_pill".into(),
            name: "测试丹".into(),
            furnace_tier_min: 1,
            stages: vec![RecipeStage {
                at_tick: 0,
                required: vec![
                    IngredientSpec {
                        material: "a".into(),
                        count: 2,
                        mineral_id: None,
                    },
                    IngredientSpec {
                        material: "b".into(),
                        count: 1,
                        mineral_id: None,
                    },
                ],
                window: 0,
            }],
            fire_profile: FireProfile {
                target_temp: 0.5,
                target_duration_ticks: 100,
                qi_cost: 10.0,
                tolerance: ToleranceSpec {
                    temp_band: 0.1,
                    duration_band: 10,
                },
            },
            outcomes: Outcomes {
                perfect: Some(PillOutcome {
                    pill: "test_pill".into(),
                    quality: 1.0,
                    toxin_amount: 0.2,
                    toxin_color: ColorKind::Mellow,
                    qi_gain: None,
                }),
                good: None,
                flawed: None,
                waste: None,
                explode: None,
            },
            flawed_fallback: Some(FlawedFallback {
                pill: "test_pill_flawed".into(),
                quality_scale: 0.5,
                toxin_scale: 1.5,
                side_effect_pool: vec![],
            }),
        }
    }

    #[test]
    fn registry_insert_rejects_duplicate_id() {
        let mut r = RecipeRegistry::new();
        r.insert(sample_recipe()).unwrap();
        assert!(r.insert(sample_recipe()).is_err());
    }

    #[test]
    fn match_exact_hits_when_ingredients_equal() {
        let mut r = RecipeRegistry::new();
        r.insert(sample_recipe()).unwrap();
        let mut staged = HashMap::new();
        staged.insert("a".to_string(), 2);
        staged.insert("b".to_string(), 1);
        assert_eq!(
            r.match_exact(&staged).map(|x| x.id.as_str()),
            Some("test_pill")
        );
    }

    #[test]
    fn match_exact_misses_on_missing_ingredient() {
        let mut r = RecipeRegistry::new();
        r.insert(sample_recipe()).unwrap();
        let mut staged = HashMap::new();
        staged.insert("a".to_string(), 2);
        assert!(r.match_exact(&staged).is_none());
    }

    #[test]
    fn match_flawed_hits_when_subset() {
        let mut r = RecipeRegistry::new();
        r.insert(sample_recipe()).unwrap();
        let mut staged = HashMap::new();
        staged.insert("a".to_string(), 2); // missing "b"
        let (hit, ratio) = r.match_flawed(&staged).expect("should match flawed");
        assert_eq!(hit.id, "test_pill");
        assert!(ratio > 0.0 && ratio < 1.0);
    }

    #[test]
    fn match_flawed_rejects_superset() {
        let mut r = RecipeRegistry::new();
        r.insert(sample_recipe()).unwrap();
        let mut staged = HashMap::new();
        staged.insert("a".to_string(), 5); // 超量
        staged.insert("b".to_string(), 1);
        assert!(r.match_flawed(&staged).is_none());
    }

    #[test]
    fn match_flawed_rejects_unknown_material() {
        let mut r = RecipeRegistry::new();
        r.insert(sample_recipe()).unwrap();
        let mut staged = HashMap::new();
        staged.insert("a".to_string(), 1);
        staged.insert("z".to_string(), 1); // unknown
        assert!(r.match_flawed(&staged).is_none());
    }

    #[test]
    fn load_registry_from_default_dir() {
        let registry = load_recipe_registry()
            .expect("production recipes under server/assets/alchemy/recipes must load");
        // plan §3.2 — three test recipes shipped
        assert!(registry.len() >= 3);
        assert!(registry.get("kai_mai_pill_v0").is_some());
        assert!(registry.get("hui_yuan_pill_v0").is_some());
        assert!(registry.get("du_ming_san_v0").is_some());
    }

    #[test]
    fn du_ming_san_has_three_stages() {
        let registry = load_recipe_registry().unwrap();
        let recipe = registry.get("du_ming_san_v0").unwrap();
        assert_eq!(recipe.stages.len(), 3);
        assert_eq!(recipe.stages[2].at_tick, 160);
    }

    // =========== plan-mineral-v1 §6 — IngredientSpec.mineral_id ===========

    #[test]
    fn ingredient_mineral_id_legacy_recipe_json_omits_field() {
        let json = r#"{ "material": "bai_cao", "count": 2 }"#;
        let ing: IngredientSpec = serde_json::from_str(json).expect("legacy ingredient must parse");
        assert!(ing.mineral_id.is_none());
    }

    #[test]
    fn ingredient_mineral_id_new_recipe_json_carries_field() {
        let json = r#"{ "material": "dan_sha_aux", "count": 1, "mineral_id": "dan_sha" }"#;
        let ing: IngredientSpec = serde_json::from_str(json).expect("aux ingredient must parse");
        assert_eq!(ing.mineral_id.as_deref(), Some("dan_sha"));
    }

    #[test]
    fn ingredient_mineral_id_serialization_omits_when_none() {
        let ing = IngredientSpec {
            material: "bai_cao".into(),
            count: 2,
            mineral_id: None,
        };
        let json = serde_json::to_string(&ing).unwrap();
        assert!(
            !json.contains("mineral_id"),
            "None mineral_id should be skipped: {json}"
        );
    }

    #[test]
    fn ingredient_matches_mineral_when_no_constraint() {
        let ing = IngredientSpec {
            material: "bai_cao".into(),
            count: 1,
            mineral_id: None,
        };
        // 无约束 — 任何 item 都通过（包括没 mineral_id 的凡俗物品）
        assert!(ing.matches_mineral(None));
        assert!(ing.matches_mineral(Some("fan_tie")));
    }

    #[test]
    fn ingredient_matches_mineral_requires_match_when_constrained() {
        let ing = IngredientSpec {
            material: "dan_sha_aux".into(),
            count: 1,
            mineral_id: Some("dan_sha".into()),
        };
        assert!(ing.matches_mineral(Some("dan_sha")));
        assert!(!ing.matches_mineral(Some("zhu_sha")));
        // 没 mineral_id NBT 的物品不可冒充矿物来源（plan §2.2 极端情况第 5 条）
        assert!(!ing.matches_mineral(None));
    }

    #[test]
    fn default_recipes_include_mineral_auxiliary_specs() {
        let registry = load_recipe_registry().unwrap();
        let jie_du = registry.get("jie_du_dan_v1").expect("jie_du_dan_v1");
        assert!(jie_du
            .stages
            .iter()
            .flat_map(|stage| stage.required.iter())
            .any(|ingredient| ingredient.mineral_id.as_deref() == Some("dan_sha")));
        let pei_yuan = registry
            .get("pei_yuan_dan_zhu_sha_v1")
            .expect("pei_yuan_dan_zhu_sha_v1");
        assert!(pei_yuan
            .stages
            .iter()
            .flat_map(|stage| stage.required.iter())
            .any(|ingredient| ingredient.mineral_id.as_deref() == Some("zhu_sha")));
    }

    #[test]
    fn recipe_mineral_validation_rejects_unknown_id() {
        let minerals = build_default_mineral_registry();
        let mut recipe = sample_recipe();
        recipe.stages[0].required[0].mineral_id = Some("unknown_ore".into());
        let err = validate_recipe_minerals(&recipe, &minerals).unwrap_err();
        assert!(err.contains("unknown_ore"));
    }

    #[test]
    fn validate_stage_mineral_items_rejects_wrong_mineral() {
        let ing = IngredientSpec {
            material: "dan_sha_aux".into(),
            count: 1,
            mineral_id: Some("dan_sha".into()),
        };
        let item = ItemInstance {
            instance_id: 1,
            template_id: "dan_sha_aux".into(),
            display_name: "灵铁".into(),
            grid_w: 1,
            grid_h: 1,
            weight: 1.0,
            rarity: crate::inventory::ItemRarity::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 0.0,
            durability: 1.0,
            freshness: None,
            mineral_id: Some("ling_tie".into()),
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
        };
        let err = validate_stage_mineral_items([&ing], [&item]).unwrap_err();
        assert!(matches!(
            err,
            IngredientMismatch::WrongMineralId { expected, got, .. }
                if expected == "dan_sha" && got == "ling_tie"
        ));
    }
}
