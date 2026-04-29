//! plan-forge-v1 §1.1 图谱系统
//!
//! JSON 定义 + 启动期扫目录加载 → `BlueprintRegistry` resource。
//! 结构与 alchemy recipe 对齐（未来可统一 `CraftingRegistry`）。

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use valence::prelude::Resource;

use crate::cultivation::components::{ColorKind, Realm};
use crate::mineral::MineralRegistry;

pub type BlueprintId = String;

pub const DEFAULT_BLUEPRINTS_DIR: &str = "assets/forge/blueprints";

/// 四步串行：坯料 → 淬炼 → 铭文 → 开光。
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StepSpec {
    Billet { profile: BilletProfile },
    Tempering { profile: TemperingProfile },
    Inscription { profile: InscriptionProfile },
    Consecration { profile: ConsecrationProfile },
}

impl StepSpec {
    pub fn kind(&self) -> StepKind {
        match self {
            StepSpec::Billet { .. } => StepKind::Billet,
            StepSpec::Tempering { .. } => StepKind::Tempering,
            StepSpec::Inscription { .. } => StepKind::Inscription,
            StepSpec::Consecration { .. } => StepKind::Consecration,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepKind {
    Billet,
    Tempering,
    Inscription,
    Consecration,
}

// ──────────────────────────────── Billet ────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct BilletProfile {
    pub required: Vec<MaterialStack>,
    #[serde(default)]
    pub optional_carriers: Vec<CarrierSpec>,
    #[serde(default)]
    pub tolerance: BilletTolerance,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MaterialStack {
    pub material: String,
    pub count: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CarrierSpec {
    pub material: String,
    pub unlocks_tier: u8,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct BilletTolerance {
    #[serde(default)]
    pub count_miss: u32,
}

// ──────────────────────────────── Tempering ────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct TemperingProfile {
    pub pattern: Vec<TemperBeat>,
    pub window_ticks: u32,
    pub qi_per_hit: f64,
    #[serde(default)]
    pub tolerance: TemperingTolerance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum TemperBeat {
    #[serde(rename = "L")]
    Light,
    #[serde(rename = "H")]
    Heavy,
    #[serde(rename = "F")]
    Fold,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct TemperingTolerance {
    #[serde(default)]
    pub miss_allowed: u32,
}

// ──────────────────────────────── Inscription ────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct InscriptionProfile {
    pub slots: u8,
    pub required_scroll_count: u8,
    #[serde(default)]
    pub tolerance: InscriptionTolerance,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct InscriptionTolerance {
    #[serde(default)]
    pub fail_chance: f32,
}

// ──────────────────────────────── Consecration ────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct ConsecrationProfile {
    pub qi_cost: f64,
    pub min_realm: Realm,
    #[serde(default)]
    pub tolerance: ConsecrationTolerance,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ConsecrationTolerance {
    #[serde(default)]
    pub qi_miss_ratio: f64,
}

// ──────────────────────────────── Outcomes ────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct OutcomesSpec {
    pub perfect: Option<WeaponOutcome>,
    pub good: Option<WeaponOutcome>,
    pub flawed: Option<WeaponOutcome>,
    #[serde(default)]
    pub waste: Option<WeaponOutcome>,
    pub explode: Option<ExplodeOutcome>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WeaponOutcome {
    pub weapon: String,
    pub quality: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExplodeOutcome {
    pub damage: f32,
    pub station_wear: f32,
}

// ──────────────────────────────── Fallback ────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct FlawedFallback {
    pub weapon: String,
    pub quality_scale: f32,
    #[serde(default)]
    pub side_effect_pool: Vec<SideEffectEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SideEffectEntry {
    pub tag: String,
    pub weight: u32,
    #[serde(default)]
    pub color: Option<ColorKind>,
    #[serde(default)]
    pub perm: bool,
}

// ──────────────────────────────── Blueprint ────────────────────────────────

fn default_station_tier_min() -> u8 {
    1
}

#[derive(Debug, Clone, Deserialize)]
pub struct Blueprint {
    pub id: BlueprintId,
    pub name: String,
    #[serde(default = "default_station_tier_min")]
    pub station_tier_min: u8,
    pub tier_cap: u8,
    pub steps: Vec<StepSpec>,
    pub outcomes: OutcomesSpec,
    #[serde(default)]
    pub flawed_fallback: Option<FlawedFallback>,
}

impl Blueprint {
    /// 找到第一个指定类型步骤的 index。
    pub fn step_index(&self, kind: StepKind) -> Option<usize> {
        self.steps.iter().position(|s| s.kind() == kind)
    }

    pub fn has_step(&self, kind: StepKind) -> bool {
        self.step_index(kind).is_some()
    }

    pub fn validate_with(
        &self,
        minerals: &MineralRegistry,
        station_tier: u8,
    ) -> Result<(), ForgeValidationError> {
        for step in &self.steps {
            let StepSpec::Billet { profile } = step else {
                continue;
            };
            for required in &profile.required {
                let Some(entry) = minerals.get_by_str(required.material.as_str()) else {
                    return Err(ForgeValidationError::UnknownMaterial {
                        material: required.material.clone(),
                    });
                };
                if entry.forge_tier_min == 0 {
                    return Err(ForgeValidationError::NotForgeMetal {
                        material: required.material.clone(),
                    });
                }
                if station_tier < entry.forge_tier_min {
                    return Err(ForgeValidationError::TierMismatch {
                        material: required.material.clone(),
                        material_name: entry.display_name_zh.to_string(),
                        station_tier,
                        required_tier: entry.forge_tier_min,
                    });
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForgeValidationError {
    UnknownMaterial {
        material: String,
    },
    NotForgeMetal {
        material: String,
    },
    TierMismatch {
        material: String,
        material_name: String,
        station_tier: u8,
        required_tier: u8,
    },
}

#[derive(Debug, Default)]
pub struct BlueprintRegistry {
    by_id: HashMap<BlueprintId, Blueprint>,
}

impl Resource for BlueprintRegistry {}

#[derive(Debug)]
pub enum BlueprintLoadError {
    Io(std::io::Error),
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    Duplicate(BlueprintId),
    InvalidMaterial {
        path: PathBuf,
        blueprint_id: BlueprintId,
        material: String,
        reason: String,
    },
}

impl std::fmt::Display for BlueprintLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlueprintLoadError::Io(e) => write!(f, "io: {e}"),
            BlueprintLoadError::Json { path, source } => {
                write!(f, "json: {}: {source}", path.display())
            }
            BlueprintLoadError::Duplicate(id) => write!(f, "duplicate blueprint id {id}"),
            BlueprintLoadError::InvalidMaterial {
                path,
                blueprint_id,
                material,
                reason,
            } => write!(
                f,
                "invalid forge material `{material}` in blueprint {blueprint_id} at {}: {reason}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for BlueprintLoadError {}

impl From<std::io::Error> for BlueprintLoadError {
    fn from(e: std::io::Error) -> Self {
        BlueprintLoadError::Io(e)
    }
}

impl BlueprintRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    pub fn get(&self, id: &str) -> Option<&Blueprint> {
        self.by_id.get(id)
    }

    pub fn ids(&self) -> impl Iterator<Item = &BlueprintId> {
        self.by_id.keys()
    }

    pub fn insert(&mut self, bp: Blueprint) -> Result<(), BlueprintLoadError> {
        if self.by_id.contains_key(&bp.id) {
            return Err(BlueprintLoadError::Duplicate(bp.id));
        }
        self.by_id.insert(bp.id.clone(), bp);
        Ok(())
    }

    /// 扫目录加载全部 *.json。
    pub fn load_dir(path: impl AsRef<Path>) -> Result<Self, BlueprintLoadError> {
        Self::load_dir_with_minerals(path, None)
    }

    pub fn load_dir_with_minerals(
        path: impl AsRef<Path>,
        minerals: Option<&MineralRegistry>,
    ) -> Result<Self, BlueprintLoadError> {
        let dir = path.as_ref();
        let mut reg = Self::new();
        if !dir.exists() {
            tracing::warn!(
                "[bong][forge] blueprints dir {} does not exist — registry empty",
                dir.display()
            );
            return Ok(reg);
        }
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let text = fs::read_to_string(&path)?;
            let bp: Blueprint =
                serde_json::from_str(&text).map_err(|e| BlueprintLoadError::Json {
                    path: path.clone(),
                    source: e,
                })?;
            if let Some(minerals) = minerals {
                validate_blueprint_minerals(&path, &bp, minerals)?;
            }
            tracing::info!("[bong][forge] loaded blueprint {} ({})", bp.id, bp.name);
            reg.insert(bp)?;
        }
        Ok(reg)
    }
}

pub fn validate_blueprint_minerals(
    path: &Path,
    bp: &Blueprint,
    minerals: &MineralRegistry,
) -> Result<(), BlueprintLoadError> {
    for step in &bp.steps {
        let StepSpec::Billet { profile } = step else {
            continue;
        };
        for required in &profile.required {
            validate_forge_material(path, &bp.id, required.material.as_str(), minerals)?;
        }
        for carrier in &profile.optional_carriers {
            if minerals.is_valid_mineral_id(&carrier.material) {
                validate_forge_material(path, &bp.id, carrier.material.as_str(), minerals)?;
            }
        }
    }
    Ok(())
}

pub fn validate_forge_material(
    path: &Path,
    blueprint_id: &str,
    material: &str,
    minerals: &MineralRegistry,
) -> Result<(), BlueprintLoadError> {
    let Some(entry) = minerals.get_by_str(material) else {
        return Err(BlueprintLoadError::InvalidMaterial {
            path: path.to_path_buf(),
            blueprint_id: blueprint_id.to_string(),
            material: material.to_string(),
            reason: "unknown mineral_id".to_string(),
        });
    };
    if entry.forge_tier_min == 0 {
        return Err(BlueprintLoadError::InvalidMaterial {
            path: path.to_path_buf(),
            blueprint_id: blueprint_id.to_string(),
            material: material.to_string(),
            reason: "mineral is not a forge metal".to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_default_blueprints() {
        let minerals = crate::mineral::build_default_registry();
        let reg =
            BlueprintRegistry::load_dir_with_minerals(DEFAULT_BLUEPRINTS_DIR, Some(&minerals))
                .expect("assets/forge/blueprints should load");
        assert_eq!(reg.len(), 3, "expected 3 test blueprints");
        assert!(reg.get("iron_sword_v0").is_some());
        assert!(reg.get("qing_feng_v0").is_some());
        assert!(reg.get("ling_feng_v0").is_some());
    }

    #[test]
    fn iron_sword_has_only_billet_step() {
        let reg = BlueprintRegistry::load_dir(DEFAULT_BLUEPRINTS_DIR).unwrap();
        let bp = reg.get("iron_sword_v0").unwrap();
        assert_eq!(bp.steps.len(), 1);
        assert_eq!(bp.steps[0].kind(), StepKind::Billet);
        assert_eq!(bp.tier_cap, 1);
    }

    #[test]
    fn qing_feng_has_billet_and_tempering() {
        let reg = BlueprintRegistry::load_dir(DEFAULT_BLUEPRINTS_DIR).unwrap();
        let bp = reg.get("qing_feng_v0").unwrap();
        assert_eq!(bp.steps.len(), 2);
        assert!(bp.has_step(StepKind::Billet));
        assert!(bp.has_step(StepKind::Tempering));
        assert!(!bp.has_step(StepKind::Inscription));
        assert!(bp.flawed_fallback.is_some());
    }

    #[test]
    fn ling_feng_has_all_four_steps() {
        let reg = BlueprintRegistry::load_dir(DEFAULT_BLUEPRINTS_DIR).unwrap();
        let bp = reg.get("ling_feng_v0").unwrap();
        assert_eq!(bp.steps.len(), 4);
        assert!(bp.has_step(StepKind::Billet));
        assert!(bp.has_step(StepKind::Tempering));
        assert!(bp.has_step(StepKind::Inscription));
        assert!(bp.has_step(StepKind::Consecration));
        assert_eq!(bp.tier_cap, 4);
    }

    #[test]
    fn duplicate_insert_errors() {
        let mut reg = BlueprintRegistry::new();
        let bp = Blueprint {
            id: "x".into(),
            name: "x".into(),
            station_tier_min: 1,
            tier_cap: 1,
            steps: vec![],
            outcomes: OutcomesSpec {
                perfect: None,
                good: None,
                flawed: None,
                waste: None,
                explode: None,
            },
            flawed_fallback: None,
        };
        reg.insert(bp.clone()).unwrap();
        let err = reg.insert(bp).unwrap_err();
        assert!(matches!(err, BlueprintLoadError::Duplicate(_)));
    }

    #[test]
    fn rejects_unknown_forge_material() {
        let minerals = crate::mineral::build_default_registry();
        let bp = Blueprint {
            id: "bad".into(),
            name: "bad".into(),
            station_tier_min: 1,
            tier_cap: 1,
            steps: vec![StepSpec::Billet {
                profile: BilletProfile {
                    required: vec![MaterialStack {
                        material: "iron_ingot".into(),
                        count: 1,
                    }],
                    optional_carriers: vec![],
                    tolerance: BilletTolerance::default(),
                },
            }],
            outcomes: OutcomesSpec {
                perfect: None,
                good: None,
                flawed: None,
                waste: None,
                explode: None,
            },
            flawed_fallback: None,
        };

        let err = validate_blueprint_minerals(Path::new("bad.json"), &bp, &minerals).unwrap_err();
        assert!(matches!(err, BlueprintLoadError::InvalidMaterial { .. }));
    }

    #[test]
    fn rejects_non_metal_forge_material() {
        let minerals = crate::mineral::build_default_registry();
        let err = validate_forge_material(Path::new("bad.json"), "bad", "dan_sha", &minerals)
            .unwrap_err();
        assert!(matches!(err, BlueprintLoadError::InvalidMaterial { .. }));
    }

    #[test]
    fn validate_with_rejects_station_tier_below_required_material() {
        let minerals = crate::mineral::build_default_registry();
        let reg = BlueprintRegistry::load_dir(DEFAULT_BLUEPRINTS_DIR).unwrap();
        let bp = reg.get("ling_feng_v0").expect("ling_feng fixture");
        let err = bp.validate_with(&minerals, 1).unwrap_err();
        assert!(matches!(
            err,
            ForgeValidationError::TierMismatch {
                material,
                station_tier: 1,
                required_tier: 3,
                ..
            } if material == "sui_tie"
        ));
    }

    #[test]
    fn validate_with_accepts_tier_three_rare_metals() {
        let minerals = crate::mineral::build_default_registry();
        let reg = BlueprintRegistry::load_dir(DEFAULT_BLUEPRINTS_DIR).unwrap();
        let ling_feng = reg.get("ling_feng_v0").expect("ling_feng fixture");
        ling_feng
            .validate_with(&minerals, 3)
            .expect("tier 3 station should accept sui_tie/rare metal blueprint");
        assert_eq!(
            minerals.get_by_str("ku_jin").unwrap().forge_tier_min,
            3,
            "plan-mineral-v2 P2: 稀铁炉可承接枯金"
        );
    }
}
