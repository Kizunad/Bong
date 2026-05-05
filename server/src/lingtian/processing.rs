//! plan-lingtian-process-v1 — 作物二级加工。
//!
//! 本模块只负责“原作物 → 加工成品”的单级转换。freshness 衰减本体在
//! `inventory::freshness`，forge/alchemy 只通过窄 helper 读取加工产物标签，
//! 避免把灵田加工逻辑扩散到其它子系统。

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component, Entity, Query, Resource};

pub const DRYING_TICKS: u32 = 24_000;
pub const GRINDING_TICKS: u32 = 600;
pub const FORGING_ALCHEMY_TICKS: u32 = 6_000;
pub const EXTRACTION_TICKS: u32 = 12_000;
pub const DEFAULT_RECIPES_DIR: &str = "assets/recipes/processing";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessingKind {
    Drying,
    Grinding,
    ForgingAlchemy,
    Extraction,
}

impl ProcessingKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ProcessingKind::Drying => "drying",
            ProcessingKind::Grinding => "grinding",
            ProcessingKind::ForgingAlchemy => "forging_alchemy",
            ProcessingKind::Extraction => "extraction",
        }
    }

    pub fn default_duration_ticks(self) -> u32 {
        match self {
            ProcessingKind::Drying => DRYING_TICKS,
            ProcessingKind::Grinding => GRINDING_TICKS,
            ProcessingKind::ForgingAlchemy => FORGING_ALCHEMY_TICKS,
            ProcessingKind::Extraction => EXTRACTION_TICKS,
        }
    }

    pub fn skill_focus(self) -> SkillFocus {
        match self {
            ProcessingKind::Drying | ProcessingKind::Grinding => SkillFocus::Herbalism,
            ProcessingKind::ForgingAlchemy => SkillFocus::Alchemy,
            ProcessingKind::Extraction => SkillFocus::Hybrid,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkillFocus {
    Herbalism,
    Alchemy,
    Hybrid,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ItemStack {
    pub item_id: String,
    pub count: u32,
    pub quality: f32,
    #[serde(default)]
    pub freshness: Option<f32>,
}

impl ItemStack {
    pub fn new(item_id: impl Into<String>, count: u32, quality: f32) -> Self {
        Self {
            item_id: item_id.into(),
            count,
            quality,
            freshness: None,
        }
    }

    pub fn with_freshness(mut self, freshness: f32) -> Self {
        self.freshness = Some(freshness.clamp(0.0, 1.0));
        self
    }
}

#[derive(Component, Debug, Clone, PartialEq)]
pub struct ProcessingSession {
    pub player: Entity,
    pub kind: ProcessingKind,
    pub input_items: Vec<ItemStack>,
    pub recipe_id: String,
    pub started_at_tick: u64,
    pub expected_duration_ticks: u32,
    pub progress_ticks: u32,
    pub freshness_frozen: bool,
}

impl ProcessingSession {
    pub fn new(
        player: Entity,
        kind: ProcessingKind,
        input_items: Vec<ItemStack>,
        recipe_id: impl Into<String>,
        started_at_tick: u64,
        expected_duration_ticks: u32,
    ) -> Self {
        Self {
            player,
            kind,
            input_items,
            recipe_id: recipe_id.into(),
            started_at_tick,
            expected_duration_ticks,
            progress_ticks: 0,
            freshness_frozen: true,
        }
    }

    pub fn progress_ratio(&self) -> f32 {
        if self.expected_duration_ticks == 0 {
            return 1.0;
        }
        (self.progress_ticks as f32 / self.expected_duration_ticks as f32).clamp(0.0, 1.0)
    }

    pub fn is_complete(&self) -> bool {
        self.progress_ticks >= self.expected_duration_ticks
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecipeInput {
    pub item_id: String,
    pub count: u32,
    #[serde(default)]
    pub min_freshness: Option<FreshnessRequirement>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FreshnessRequirement(pub f32);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecipeOutput {
    pub item_id: String,
    pub count: u32,
    pub quality_multiplier: f32,
    #[serde(default)]
    pub freshness_profile: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SkillRequirement {
    #[serde(default)]
    pub herbalism: u8,
    #[serde(default)]
    pub alchemy: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProcessingRecipe {
    pub id: String,
    pub kind: ProcessingKind,
    pub inputs: Vec<RecipeInput>,
    pub outputs: Vec<RecipeOutput>,
    pub duration_ticks: u32,
    pub skill_req: SkillRequirement,
    pub failure_rate: f32,
    #[serde(default)]
    pub failure_output: Option<RecipeOutput>,
    #[serde(default)]
    pub qi_cost: u32,
}

impl ProcessingRecipe {
    pub fn validate(&self) -> Result<(), String> {
        if self.id.trim().is_empty() {
            return Err("processing recipe id must not be empty".into());
        }
        if self.inputs.is_empty() {
            return Err(format!("processing recipe `{}` has no inputs", self.id));
        }
        if self.outputs.is_empty() {
            return Err(format!("processing recipe `{}` has no outputs", self.id));
        }
        if self.duration_ticks == 0 {
            return Err(format!(
                "processing recipe `{}` duration_ticks must be > 0",
                self.id
            ));
        }
        if !(0.0..=1.0).contains(&self.failure_rate) || !self.failure_rate.is_finite() {
            return Err(format!(
                "processing recipe `{}` failure_rate must be finite in 0..=1",
                self.id
            ));
        }
        for input in &self.inputs {
            if input.item_id.trim().is_empty() || input.count == 0 {
                return Err(format!(
                    "processing recipe `{}` input item_id/count invalid",
                    self.id
                ));
            }
            if let Some(req) = input.min_freshness {
                if !(0.0..=1.0).contains(&req.0) || !req.0.is_finite() {
                    return Err(format!(
                        "processing recipe `{}` min_freshness must be finite in 0..=1",
                        self.id
                    ));
                }
            }
        }
        for output in self.outputs.iter().chain(self.failure_output.iter()) {
            if output.item_id.trim().is_empty() || output.count == 0 {
                return Err(format!(
                    "processing recipe `{}` output item_id/count invalid",
                    self.id
                ));
            }
            if output.quality_multiplier < 0.0 || !output.quality_multiplier.is_finite() {
                return Err(format!(
                    "processing recipe `{}` output quality_multiplier invalid",
                    self.id
                ));
            }
        }
        Ok(())
    }
}

#[derive(Resource, Default, Clone, Debug)]
pub struct ProcessingRecipeRegistry {
    by_id: HashMap<String, ProcessingRecipe>,
}

impl ProcessingRecipeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, recipe: ProcessingRecipe) -> Result<(), String> {
        recipe.validate()?;
        self.by_id.insert(recipe.id.clone(), recipe);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&ProcessingRecipe> {
        self.by_id.get(id)
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    pub fn ids(&self) -> impl Iterator<Item = &String> {
        self.by_id.keys()
    }

    pub fn recipes_by_kind(&self, kind: ProcessingKind) -> impl Iterator<Item = &ProcessingRecipe> {
        self.by_id
            .values()
            .filter(move |recipe| recipe.kind == kind)
    }

    pub fn load_default() -> Result<Self, String> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_RECIPES_DIR);
        Self::load_dir(path)
    }

    pub fn load_dir(path: impl AsRef<Path>) -> Result<Self, String> {
        let mut toml_paths = Vec::new();
        collect_toml_paths(path.as_ref(), &mut toml_paths)?;
        toml_paths.sort();
        if toml_paths.is_empty() {
            return Err(format!(
                "processing recipe directory {} contains no *.toml files",
                path.as_ref().display()
            ));
        }

        let mut registry = Self::new();
        for toml_path in toml_paths {
            let content = fs::read_to_string(&toml_path)
                .map_err(|error| format!("failed to read {}: {error}", toml_path.display()))?;
            let parsed: ProcessingRecipesToml = toml::from_str(&content).map_err(|error| {
                format!(
                    "failed to parse {} as processing recipe TOML: {error}",
                    toml_path.display()
                )
            })?;
            for recipe in parsed.recipe {
                let id = recipe.id.clone();
                if registry.by_id.contains_key(&id) {
                    return Err(format!(
                        "duplicate processing recipe id `{id}` in {}",
                        toml_path.display()
                    ));
                }
                registry.insert(recipe)?;
            }
        }
        Ok(registry)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProcessingRecipesToml {
    recipe: Vec<ProcessingRecipe>,
}

fn collect_toml_paths(path: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(path).map_err(|error| {
        format!(
            "failed to read processing recipe directory {}: {error}",
            path.display()
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "failed to read processing recipe entry under {}: {error}",
                path.display()
            )
        })?;
        let file_path = entry.path();
        if file_path.is_dir() {
            collect_toml_paths(&file_path, out)?;
        } else if file_path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("toml"))
        {
            out.push(file_path);
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessingSkillLevels {
    pub herbalism: u8,
    pub alchemy: u8,
}

impl ProcessingSkillLevels {
    pub fn satisfies(self, req: &SkillRequirement) -> bool {
        self.herbalism >= req.herbalism && self.alchemy >= req.alchemy
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProcessingStartError {
    UnknownRecipe,
    KindMismatch {
        expected: ProcessingKind,
        got: ProcessingKind,
    },
    SkillLocked {
        required: SkillRequirement,
        actual: ProcessingSkillLevels,
    },
    MissingInput {
        item_id: String,
        required: u32,
        available: u32,
    },
    FreshnessTooLow {
        item_id: String,
        min: f32,
        got: f32,
    },
}

pub fn validate_processing_start(
    registry: &ProcessingRecipeRegistry,
    recipe_id: &str,
    kind: ProcessingKind,
    inputs: &[ItemStack],
    skills: ProcessingSkillLevels,
) -> Result<(), ProcessingStartError> {
    let recipe = registry
        .get(recipe_id)
        .ok_or(ProcessingStartError::UnknownRecipe)?;
    if recipe.kind != kind {
        return Err(ProcessingStartError::KindMismatch {
            expected: recipe.kind,
            got: kind,
        });
    }
    if !skills.satisfies(&recipe.skill_req) {
        return Err(ProcessingStartError::SkillLocked {
            required: recipe.skill_req.clone(),
            actual: skills,
        });
    }
    for required in &recipe.inputs {
        let mut matching_count = 0;
        let mut qualifying_count = 0;
        let mut best_freshness = None::<f32>;
        for stack in inputs
            .iter()
            .filter(|stack| stack.item_id == required.item_id)
        {
            matching_count += stack.count;
            let got = stack.freshness.unwrap_or(1.0);
            best_freshness = Some(best_freshness.map_or(got, |best| best.max(got)));
            if required.min_freshness.is_none_or(|min| got >= min.0) {
                qualifying_count += stack.count;
            }
        }
        if qualifying_count < required.count {
            if let Some(min) = required.min_freshness {
                if matching_count >= required.count || matching_count > 0 {
                    return Err(ProcessingStartError::FreshnessTooLow {
                        item_id: required.item_id.clone(),
                        min: min.0,
                        got: best_freshness.unwrap_or(0.0),
                    });
                }
            }
            return Err(ProcessingStartError::MissingInput {
                item_id: required.item_id.clone(),
                required: required.count,
                available: matching_count,
            });
        }
    }
    Ok(())
}

pub fn processing_session_tick_system(mut sessions: Query<&mut ProcessingSession>) {
    for mut session in &mut sessions {
        tick_processing_session(&mut session, 1);
    }
}

pub fn tick_processing_session(session: &mut ProcessingSession, ticks: u32) {
    if session.is_complete() {
        return;
    }
    session.progress_ticks = session
        .progress_ticks
        .saturating_add(ticks)
        .min(session.expected_duration_ticks);
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProcessingCompletion {
    pub succeeded: bool,
    pub outputs: Vec<ItemStack>,
    pub applied_quality_multiplier: f32,
    pub freshness_profile: Option<String>,
}

pub fn complete_processing_session(
    session: &ProcessingSession,
    recipe: &ProcessingRecipe,
    skills: ProcessingSkillLevels,
    roll_0_1: f32,
) -> ProcessingCompletion {
    let failure_rate = adjusted_failure_rate(recipe, skills);
    let input_quality = average_input_quality(&session.input_items);
    let succeeded = roll_0_1.clamp(0.0, 1.0) >= failure_rate;
    let selected_outputs: Vec<RecipeOutput> = if succeeded {
        recipe.outputs.clone()
    } else {
        recipe.failure_output.iter().cloned().collect()
    };
    let mut freshness_profile = None;
    let outputs = selected_outputs
        .iter()
        .map(|output| {
            freshness_profile = output
                .freshness_profile
                .clone()
                .or(freshness_profile.clone());
            ItemStack::new(
                output.item_id.clone(),
                output.count,
                input_quality * output.quality_multiplier,
            )
        })
        .collect();

    ProcessingCompletion {
        succeeded,
        outputs,
        applied_quality_multiplier: selected_outputs
            .first()
            .map(|output| output.quality_multiplier)
            .unwrap_or(0.0),
        freshness_profile,
    }
}

pub fn adjusted_failure_rate(recipe: &ProcessingRecipe, skills: ProcessingSkillLevels) -> f32 {
    let focus_level = match recipe.kind.skill_focus() {
        SkillFocus::Herbalism => skills.herbalism,
        SkillFocus::Alchemy => skills.alchemy,
        SkillFocus::Hybrid => skills.herbalism.min(skills.alchemy),
    };
    let reduction = match focus_level {
        0..=2 => 0.0,
        3..=4 => 0.20,
        5..=7 => 0.45,
        _ => 0.65,
    };
    (recipe.failure_rate * (1.0 - reduction)).clamp(0.0, 1.0)
}

pub fn average_input_quality(items: &[ItemStack]) -> f32 {
    let (weighted_sum, count) = items.iter().fold((0.0, 0), |(sum, count), item| {
        (sum + item.quality * item.count as f32, count + item.count)
    });
    if count == 0 {
        1.0
    } else {
        weighted_sum / count as f32
    }
}

pub fn processed_input_quality_bonus(item_id: &str, quality: f32) -> f32 {
    let kind_bonus = if item_id.starts_with("extract_") {
        0.20
    } else if item_id.starts_with("processed_") {
        0.12
    } else if item_id.starts_with("powder_") {
        0.08
    } else if item_id.starts_with("dry_") {
        0.04
    } else {
        0.0
    };
    (quality.clamp(0.0, 2.5) - 1.0).max(0.0) * 0.10 + kind_bonus
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExtractionYield {
    pub output_count: u32,
    pub quality_multiplier: f32,
}

pub fn extraction_yield(input_count: u32, skills: ProcessingSkillLevels) -> ExtractionYield {
    let base = (input_count / 3).max(1);
    let output_count = if skills.alchemy >= 6 {
        ((base as f32) * 1.5).ceil() as u32
    } else {
        base
    };
    ExtractionYield {
        output_count,
        quality_multiplier: 2.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::Entity;

    fn entity() -> Entity {
        Entity::from_raw(1)
    }

    fn dry_recipe() -> ProcessingRecipe {
        ProcessingRecipe {
            id: "dry_ci_she_hao".to_string(),
            kind: ProcessingKind::Drying,
            inputs: vec![RecipeInput {
                item_id: "ci_she_hao".to_string(),
                count: 5,
                min_freshness: Some(FreshnessRequirement(0.5)),
            }],
            outputs: vec![RecipeOutput {
                item_id: "dry_ci_she_hao".to_string(),
                count: 5,
                quality_multiplier: 0.9,
                freshness_profile: Some("drying_v1".to_string()),
            }],
            duration_ticks: DRYING_TICKS,
            skill_req: SkillRequirement {
                herbalism: 0,
                alchemy: 0,
            },
            failure_rate: 0.30,
            failure_output: Some(RecipeOutput {
                item_id: "withered_dry_ci_she_hao".to_string(),
                count: 3,
                quality_multiplier: 0.3,
                freshness_profile: Some("drying_v1".to_string()),
            }),
            qi_cost: 0,
        }
    }

    fn registry() -> ProcessingRecipeRegistry {
        let mut registry = ProcessingRecipeRegistry::new();
        registry.insert(dry_recipe()).unwrap();
        registry
            .insert(ProcessingRecipe {
                id: "grind_ci_she_hao".to_string(),
                kind: ProcessingKind::Grinding,
                inputs: vec![RecipeInput {
                    item_id: "dry_ci_she_hao".to_string(),
                    count: 2,
                    min_freshness: None,
                }],
                outputs: vec![RecipeOutput {
                    item_id: "powder_ci_she_hao".to_string(),
                    count: 2,
                    quality_multiplier: 1.0,
                    freshness_profile: Some("grinding_v1".to_string()),
                }],
                duration_ticks: GRINDING_TICKS,
                skill_req: SkillRequirement::default(),
                failure_rate: 0.10,
                failure_output: None,
                qi_cost: 0,
            })
            .unwrap();
        registry
    }

    #[test]
    fn processing_kind_enum_4_variants_distinct() {
        let variants = [
            ProcessingKind::Drying,
            ProcessingKind::Grinding,
            ProcessingKind::ForgingAlchemy,
            ProcessingKind::Extraction,
        ];
        assert_eq!(variants.len(), 4);
        assert_eq!(ProcessingKind::Drying.as_str(), "drying");
        assert_ne!(ProcessingKind::Drying, ProcessingKind::Grinding);
        assert_eq!(
            ProcessingKind::Extraction.default_duration_ticks(),
            EXTRACTION_TICKS
        );
    }

    #[test]
    fn processing_session_progress_tick_increments() {
        let mut session = ProcessingSession::new(
            entity(),
            ProcessingKind::Grinding,
            vec![],
            "grind_ci_she_hao",
            10,
            3,
        );
        tick_processing_session(&mut session, 1);
        assert_eq!(session.progress_ticks, 1);
        assert_eq!(session.progress_ratio(), 1.0 / 3.0);
    }

    #[test]
    fn processing_session_completion_at_duration() {
        let mut session = ProcessingSession::new(
            entity(),
            ProcessingKind::Grinding,
            vec![],
            "grind_ci_she_hao",
            10,
            3,
        );
        tick_processing_session(&mut session, 5);
        assert_eq!(session.progress_ticks, 3);
        assert!(session.is_complete());
    }

    #[test]
    fn processing_session_freezes_input_freshness() {
        let session = ProcessingSession::new(
            entity(),
            ProcessingKind::Drying,
            vec![ItemStack::new("ci_she_hao", 5, 1.0).with_freshness(0.8)],
            "dry_ci_she_hao",
            10,
            DRYING_TICKS,
        );
        assert!(session.freshness_frozen);
        assert_eq!(session.input_items[0].freshness, Some(0.8));
    }

    #[test]
    fn processing_recipe_registry_lookup_by_id() {
        let registry = registry();
        assert_eq!(registry.len(), 2);
        assert_eq!(
            registry.get("dry_ci_she_hao").unwrap().kind,
            ProcessingKind::Drying
        );
    }

    #[test]
    fn processing_recipe_registry_unknown_id_returns_none() {
        assert!(registry().get("missing").is_none());
    }

    #[test]
    fn drying_recipe_lv0_failure_rate_30_percent() {
        let recipe = dry_recipe();
        let rate = adjusted_failure_rate(
            &recipe,
            ProcessingSkillLevels {
                herbalism: 0,
                alchemy: 0,
            },
        );
        assert!((rate - 0.30).abs() < f32::EPSILON);
    }

    #[test]
    fn grinding_recipe_high_skill_lower_failure() {
        let recipe = registry().get("grind_ci_she_hao").unwrap().clone();
        let low = adjusted_failure_rate(
            &recipe,
            ProcessingSkillLevels {
                herbalism: 0,
                alchemy: 0,
            },
        );
        let high = adjusted_failure_rate(
            &recipe,
            ProcessingSkillLevels {
                herbalism: 8,
                alchemy: 0,
            },
        );
        assert!(high < low);
    }

    #[test]
    fn processing_session_offline_pause_resume() {
        let mut session = ProcessingSession::new(
            entity(),
            ProcessingKind::Drying,
            vec![],
            "dry_ci_she_hao",
            10,
            DRYING_TICKS,
        );
        tick_processing_session(&mut session, 100);
        let paused = session.progress_ticks;
        tick_processing_session(&mut session, 0);
        assert_eq!(session.progress_ticks, paused);
        tick_processing_session(&mut session, 1);
        assert_eq!(session.progress_ticks, paused + 1);
    }

    #[test]
    fn processing_session_input_quality_multiplier_applied() {
        let recipe = dry_recipe();
        let session = ProcessingSession::new(
            entity(),
            ProcessingKind::Drying,
            vec![ItemStack::new("ci_she_hao", 5, 1.2)],
            "dry_ci_she_hao",
            10,
            DRYING_TICKS,
        );
        let completion = complete_processing_session(
            &session,
            &recipe,
            ProcessingSkillLevels {
                herbalism: 9,
                alchemy: 0,
            },
            1.0,
        );
        assert!(completion.succeeded);
        assert!((completion.outputs[0].quality - 1.08).abs() < 1e-6);
    }

    #[test]
    fn processing_session_failure_produces_failure_output() {
        let recipe = dry_recipe();
        let session = ProcessingSession::new(
            entity(),
            ProcessingKind::Drying,
            vec![ItemStack::new("ci_she_hao", 5, 1.0)],
            "dry_ci_she_hao",
            10,
            DRYING_TICKS,
        );
        let completion = complete_processing_session(
            &session,
            &recipe,
            ProcessingSkillLevels {
                herbalism: 0,
                alchemy: 0,
            },
            0.0,
        );
        assert!(!completion.succeeded);
        assert_eq!(completion.outputs[0].item_id, "withered_dry_ci_she_hao");
    }

    #[test]
    fn processing_session_failure_no_output_when_none() {
        let recipe = registry().get("grind_ci_she_hao").unwrap().clone();
        let session = ProcessingSession::new(
            entity(),
            ProcessingKind::Grinding,
            vec![ItemStack::new("dry_ci_she_hao", 2, 1.0)],
            "grind_ci_she_hao",
            10,
            GRINDING_TICKS,
        );
        let completion = complete_processing_session(
            &session,
            &recipe,
            ProcessingSkillLevels {
                herbalism: 0,
                alchemy: 0,
            },
            0.0,
        );
        assert!(!completion.succeeded);
        assert!(completion.outputs.is_empty());
    }

    #[test]
    fn validate_start_rejects_low_freshness() {
        let err = validate_processing_start(
            &registry(),
            "dry_ci_she_hao",
            ProcessingKind::Drying,
            &[ItemStack::new("ci_she_hao", 5, 1.0).with_freshness(0.2)],
            ProcessingSkillLevels {
                herbalism: 0,
                alchemy: 0,
            },
        )
        .unwrap_err();
        assert!(matches!(err, ProcessingStartError::FreshnessTooLow { .. }));
    }

    #[test]
    fn validate_start_unknown_recipe_id_returns_unknown_recipe() {
        let err = validate_processing_start(
            &registry(),
            "missing_recipe",
            ProcessingKind::Drying,
            &[ItemStack::new("ci_she_hao", 5, 1.0).with_freshness(0.8)],
            ProcessingSkillLevels {
                herbalism: 9,
                alchemy: 9,
            },
        )
        .unwrap_err();
        assert_eq!(err, ProcessingStartError::UnknownRecipe);
    }

    #[test]
    fn validate_start_kind_mismatch_between_request_and_recipe() {
        let err = validate_processing_start(
            &registry(),
            "dry_ci_she_hao",
            ProcessingKind::Grinding,
            &[ItemStack::new("ci_she_hao", 5, 1.0).with_freshness(0.8)],
            ProcessingSkillLevels {
                herbalism: 9,
                alchemy: 9,
            },
        )
        .unwrap_err();
        assert_eq!(
            err,
            ProcessingStartError::KindMismatch {
                expected: ProcessingKind::Drying,
                got: ProcessingKind::Grinding,
            }
        );
    }

    #[test]
    fn validate_start_skill_locked_when_below_required_levels() {
        let mut registry = ProcessingRecipeRegistry::new();
        let mut locked = dry_recipe();
        locked.id = "locked_forge_ci_she_hao".to_string();
        locked.skill_req = SkillRequirement {
            herbalism: 5,
            alchemy: 3,
        };
        registry.insert(locked).unwrap();

        let err = validate_processing_start(
            &registry,
            "locked_forge_ci_she_hao",
            ProcessingKind::Drying,
            &[ItemStack::new("ci_she_hao", 5, 1.0).with_freshness(0.8)],
            ProcessingSkillLevels {
                herbalism: 4,
                alchemy: 3,
            },
        )
        .unwrap_err();
        assert_eq!(
            err,
            ProcessingStartError::SkillLocked {
                required: SkillRequirement {
                    herbalism: 5,
                    alchemy: 3,
                },
                actual: ProcessingSkillLevels {
                    herbalism: 4,
                    alchemy: 3,
                },
            }
        );
    }

    #[test]
    fn validate_start_missing_input_when_count_below_required() {
        let err = validate_processing_start(
            &registry(),
            "grind_ci_she_hao",
            ProcessingKind::Grinding,
            &[ItemStack::new("dry_ci_she_hao", 1, 1.0)],
            ProcessingSkillLevels {
                herbalism: 9,
                alchemy: 9,
            },
        )
        .unwrap_err();
        assert_eq!(
            err,
            ProcessingStartError::MissingInput {
                item_id: "dry_ci_she_hao".to_string(),
                required: 2,
                available: 1,
            }
        );
    }

    #[test]
    fn validate_start_ignores_stale_extra_stack_when_fresh_quantity_satisfies() {
        assert_eq!(
            validate_processing_start(
                &registry(),
                "dry_ci_she_hao",
                ProcessingKind::Drying,
                &[
                    ItemStack::new("ci_she_hao", 5, 1.0).with_freshness(0.8),
                    ItemStack::new("ci_she_hao", 1, 1.0).with_freshness(0.2),
                ],
                ProcessingSkillLevels {
                    herbalism: 0,
                    alchemy: 0,
                },
            ),
            Ok(())
        );
    }

    #[test]
    fn default_processing_recipe_assets_load_at_least_8_recipes() {
        let registry = ProcessingRecipeRegistry::load_default()
            .expect("processing recipes under assets/recipes/processing should load");
        assert!(registry.len() >= 8);
        assert_eq!(registry.recipes_by_kind(ProcessingKind::Drying).count(), 2);
        assert!(registry.get("extract_ci_she_hao").is_some());
    }

    #[test]
    fn extraction_session_high_quality_low_quantity() {
        let yield_spec = extraction_yield(
            3,
            ProcessingSkillLevels {
                herbalism: 6,
                alchemy: 3,
            },
        );
        assert_eq!(yield_spec.output_count, 1);
        assert_eq!(yield_spec.quality_multiplier, 2.0);
    }

    #[test]
    fn extraction_quality_x2_0_modifier() {
        let yield_spec = extraction_yield(
            6,
            ProcessingSkillLevels {
                herbalism: 6,
                alchemy: 6,
            },
        );
        assert_eq!(yield_spec.quality_multiplier, 2.0);
        assert_eq!(yield_spec.output_count, 3);
    }

    #[test]
    fn cross_skill_req_herbalism_5_alchemy_3_unlocks_extraction() {
        let req = SkillRequirement {
            herbalism: 6,
            alchemy: 3,
        };
        assert!(ProcessingSkillLevels {
            herbalism: 6,
            alchemy: 3
        }
        .satisfies(&req));
        assert!(!ProcessingSkillLevels {
            herbalism: 5,
            alchemy: 3
        }
        .satisfies(&req));
    }
}
