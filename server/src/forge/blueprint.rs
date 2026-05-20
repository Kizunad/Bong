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
            validate_forge_or_item_material(path, &bp.id, required.material.as_str(), minerals)?;
        }
        for carrier in &profile.optional_carriers {
            validate_forge_or_item_material(path, &bp.id, carrier.material.as_str(), minerals)?;
        }
    }
    Ok(())
}

fn validate_forge_or_item_material(
    path: &Path,
    blueprint_id: &str,
    material: &str,
    minerals: &MineralRegistry,
) -> Result<(), BlueprintLoadError> {
    if minerals.is_valid_mineral_id(material) {
        return validate_forge_material(path, blueprint_id, material, minerals);
    }
    if is_allowed_item_material(material) {
        return Ok(());
    }
    Err(BlueprintLoadError::InvalidMaterial {
        path: path.to_path_buf(),
        blueprint_id: blueprint_id.to_string(),
        material: material.to_string(),
        reason: "unknown mineral_id or forge item material".to_string(),
    })
}

pub fn is_allowed_item_material(material: &str) -> bool {
    matches!(
        material,
        "ling_mu_gun"
            | "ling_mu_ban"
            | "ling_mu_jing"
            | "fake_spirit_hide"
            | "feng_he_gu"
            | "yi_shou_gu"
            | "grass_fiber"
            | "xuan_gen_wei"
            | "yuan_ni_hong_yu"
            | "lie_yuan_tai"
            | "bei_wen_zhi"
    )
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
        assert_eq!(
            reg.len(),
            20,
            "expected 3 weapon + 7 tool + 1 spiritwood + 4 iron armor + 4 bone armor + 1 bone dagger blueprint"
        );
        assert!(reg.get("iron_sword_v0").is_some());
        assert!(reg.get("qing_feng_v0").is_some());
        assert!(reg.get("ling_feng_v0").is_some());
        for id in [
            "tool_cai_yao_dao_v0",
            "tool_bao_chu_v0",
            "tool_cao_lian_v0",
            "tool_dun_qi_jia_v0",
            "tool_gua_dao_v0",
            "tool_gu_hai_qian_v0",
            "tool_bing_jia_shou_tao_v0",
            "ling_xia_v1",
        ] {
            assert!(reg.get(id).is_some(), "missing tool blueprint `{id}`");
        }
    }

    #[test]
    fn iron_helmet_v0_blueprint_loads() {
        let reg = BlueprintRegistry::load_dir(DEFAULT_BLUEPRINTS_DIR).unwrap();
        let bp = reg
            .get("iron_helmet_v0")
            .expect("iron_helmet_v0 should load");
        assert_eq!(bp.name, "凡铁兜鍪");
        assert_eq!(bp.station_tier_min, 1);
        assert_eq!(bp.tier_cap, 1);
    }

    #[test]
    fn iron_chestplate_v0_blueprint_loads() {
        let reg = BlueprintRegistry::load_dir(DEFAULT_BLUEPRINTS_DIR).unwrap();
        assert!(
            reg.get("iron_chestplate_v0").is_some(),
            "iron_chestplate_v0 should be in registry"
        );
    }

    #[test]
    fn iron_leggings_v0_blueprint_loads() {
        let reg = BlueprintRegistry::load_dir(DEFAULT_BLUEPRINTS_DIR).unwrap();
        assert!(
            reg.get("iron_leggings_v0").is_some(),
            "iron_leggings_v0 should be in registry"
        );
    }

    #[test]
    fn iron_boots_v0_blueprint_loads() {
        let reg = BlueprintRegistry::load_dir(DEFAULT_BLUEPRINTS_DIR).unwrap();
        assert!(
            reg.get("iron_boots_v0").is_some(),
            "iron_boots_v0 should be in registry"
        );
    }

    #[test]
    fn iron_helmet_v0_only_needs_fan_tie_x2() {
        let reg = BlueprintRegistry::load_dir(DEFAULT_BLUEPRINTS_DIR).unwrap();
        let bp = reg.get("iron_helmet_v0").unwrap();
        assert_eq!(bp.steps.len(), 1, "helmet is single-step billet");
        let Some(StepSpec::Billet { profile }) = bp.steps.first() else {
            panic!("iron_helmet first step should be billet");
        };
        assert_eq!(profile.required.len(), 1);
        assert_eq!(profile.required[0].material, "fan_tie");
        assert_eq!(profile.required[0].count, 2);
    }

    #[test]
    fn iron_chestplate_v0_needs_fan_tie_x4() {
        let reg = BlueprintRegistry::load_dir(DEFAULT_BLUEPRINTS_DIR).unwrap();
        let bp = reg.get("iron_chestplate_v0").unwrap();
        let Some(StepSpec::Billet { profile }) = bp.steps.first() else {
            panic!("iron_chestplate first step should be billet");
        };
        assert_eq!(
            profile.required[0].count, 4,
            "chestplate is the biggest piece, needs 4 fan_tie"
        );
    }

    #[test]
    fn iron_armor_blueprints_produce_correct_outcomes() {
        let reg = BlueprintRegistry::load_dir(DEFAULT_BLUEPRINTS_DIR).unwrap();
        for (bp_id, weapon_id) in [
            ("iron_helmet_v0", "armor_iron_helmet"),
            ("iron_chestplate_v0", "armor_iron_chestplate"),
            ("iron_leggings_v0", "armor_iron_leggings"),
            ("iron_boots_v0", "armor_iron_boots"),
        ] {
            let bp = reg.get(bp_id).unwrap_or_else(|| panic!("missing {bp_id}"));
            assert_eq!(
                bp.outcomes.perfect.as_ref().map(|o| o.weapon.as_str()),
                Some(weapon_id),
                "{bp_id} perfect outcome should produce {weapon_id}"
            );
            let perfect_q = bp.outcomes.perfect.as_ref().unwrap().quality;
            let flawed_q = bp.outcomes.flawed.as_ref().unwrap().quality;
            assert!(
                (perfect_q - 1.0).abs() < f32::EPSILON,
                "{bp_id} perfect quality should be 1.0"
            );
            assert!(
                (flawed_q - 0.5).abs() < f32::EPSILON,
                "{bp_id} flawed quality should be 0.5"
            );
        }
    }

    #[test]
    fn ling_xia_recipe_uses_spiritwood_and_feng_he_gu_materials() {
        let reg = BlueprintRegistry::load_dir(DEFAULT_BLUEPRINTS_DIR).unwrap();
        let bp = reg.get("ling_xia_v1").expect("ling_xia blueprint exists");
        let Some(StepSpec::Billet { profile }) = bp.steps.first() else {
            panic!("ling_xia first step should be billet");
        };

        assert!(profile
            .required
            .iter()
            .any(|stack| stack.material == "ling_mu_ban" && stack.count == 2));
        assert!(profile
            .required
            .iter()
            .any(|stack| stack.material == "feng_he_gu" && stack.count == 2));
        assert_eq!(
            bp.outcomes
                .good
                .as_ref()
                .map(|outcome| outcome.weapon.as_str()),
            Some("ling_xia")
        );
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
    fn ling_feng_accepts_botany_v2_carriers() {
        let reg = BlueprintRegistry::load_dir(DEFAULT_BLUEPRINTS_DIR).unwrap();
        let bp = reg.get("ling_feng_v0").unwrap();
        let Some(StepSpec::Billet { profile }) = bp.steps.first() else {
            panic!("ling_feng first step should be billet");
        };
        assert!(profile
            .optional_carriers
            .iter()
            .any(|carrier| { carrier.material == "xuan_gen_wei" && carrier.unlocks_tier == 3 }));
        assert!(profile
            .optional_carriers
            .iter()
            .any(|carrier| { carrier.material == "yuan_ni_hong_yu" && carrier.unlocks_tier == 4 }));
    }

    #[test]
    fn tool_blueprints_are_single_step_fanqi_outputs() {
        let reg = BlueprintRegistry::load_dir(DEFAULT_BLUEPRINTS_DIR).unwrap();

        for (id, item_id) in [
            ("tool_cai_yao_dao_v0", "cai_yao_dao"),
            ("tool_bao_chu_v0", "bao_chu"),
            ("tool_cao_lian_v0", "cao_lian"),
            ("tool_dun_qi_jia_v0", "dun_qi_jia"),
            ("tool_gua_dao_v0", "gua_dao"),
            ("tool_gu_hai_qian_v0", "gu_hai_qian"),
            ("tool_bing_jia_shou_tao_v0", "bing_jia_shou_tao"),
        ] {
            let bp = reg.get(id).unwrap_or_else(|| panic!("missing {id}"));
            assert_eq!(
                bp.steps.len(),
                1,
                "tool blueprint `{id}` should be one-step"
            );
            assert_eq!(bp.steps[0].kind(), StepKind::Billet);
            assert_eq!(bp.tier_cap, 1);
            assert_eq!(
                bp.outcomes
                    .good
                    .as_ref()
                    .map(|outcome| outcome.weapon.as_str()),
                Some(item_id),
                "tool blueprint `{id}` should produce `{item_id}`"
            );
        }
    }

    #[test]
    fn iron_armor_blueprint_scrolls_parse_and_link_correctly() {
        let item_reg = crate::inventory::load_item_registry().expect("item registry should load");
        for (item_id, blueprint_id) in [
            ("blueprint_scroll_iron_helmet", "iron_helmet_v0"),
            ("blueprint_scroll_iron_chestplate", "iron_chestplate_v0"),
            ("blueprint_scroll_iron_leggings", "iron_leggings_v0"),
            ("blueprint_scroll_iron_boots", "iron_boots_v0"),
        ] {
            let item = item_reg
                .get(item_id)
                .unwrap_or_else(|| panic!("expected {item_id} in item registry"));
            let scroll = item
                .blueprint_scroll_spec
                .as_ref()
                .unwrap_or_else(|| panic!("expected blueprint_scroll_spec for {item_id}"));
            assert_eq!(
                scroll.blueprint_id, blueprint_id,
                "{item_id} should map to forge blueprint {blueprint_id}, got {}",
                scroll.blueprint_id
            );
        }
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

    // ─── bone armor & dagger blueprint tests (plan-depth-loop-v1 P4) ───

    #[test]
    fn bone_helmet_v0_blueprint_loads() {
        let reg = BlueprintRegistry::load_dir(DEFAULT_BLUEPRINTS_DIR).unwrap();
        let bp = reg
            .get("bone_helmet_v0")
            .expect("bone_helmet_v0 should load");
        assert_eq!(bp.name, "骨兜鍪");
        assert_eq!(bp.station_tier_min, 1);
        assert_eq!(bp.tier_cap, 1);
    }

    #[test]
    fn bone_chestplate_v0_blueprint_loads() {
        let reg = BlueprintRegistry::load_dir(DEFAULT_BLUEPRINTS_DIR).unwrap();
        assert!(
            reg.get("bone_chestplate_v0").is_some(),
            "bone_chestplate_v0 should be in registry"
        );
    }

    #[test]
    fn bone_dagger_v0_blueprint_loads() {
        let reg = BlueprintRegistry::load_dir(DEFAULT_BLUEPRINTS_DIR).unwrap();
        let bp = reg
            .get("bone_dagger_v0")
            .expect("bone_dagger_v0 should load");
        assert_eq!(bp.name, "骨匕首");
        assert_eq!(bp.station_tier_min, 1);
        let Some(StepSpec::Billet { profile }) = bp.steps.first() else {
            panic!("bone_dagger first step should be billet");
        };
        assert_eq!(profile.required.len(), 2, "bone dagger needs yi_shou_gu + grass_fiber");
        assert!(
            profile.required.iter().any(|s| s.material == "yi_shou_gu" && s.count == 2),
            "bone dagger should need 2 yi_shou_gu"
        );
        assert!(
            profile.required.iter().any(|s| s.material == "grass_fiber" && s.count == 1),
            "bone dagger should need 1 grass_fiber"
        );
    }

    #[test]
    fn bone_armor_blueprints_produce_correct_outcomes() {
        let reg = BlueprintRegistry::load_dir(DEFAULT_BLUEPRINTS_DIR).unwrap();
        for (bp_id, weapon_id) in [
            ("bone_helmet_v0", "armor_bone_helmet"),
            ("bone_chestplate_v0", "armor_bone_chestplate"),
            ("bone_leggings_v0", "armor_bone_leggings"),
            ("bone_boots_v0", "armor_bone_boots"),
        ] {
            let bp = reg.get(bp_id).unwrap_or_else(|| panic!("missing {bp_id}"));
            assert_eq!(
                bp.outcomes.perfect.as_ref().map(|o| o.weapon.as_str()),
                Some(weapon_id),
                "{bp_id} perfect outcome should produce {weapon_id}"
            );
            let explode = bp.outcomes.explode.as_ref().unwrap();
            assert!(
                (explode.damage - 3.0).abs() < f32::EPSILON,
                "{bp_id} explode damage should be 3.0 (lower than iron), got {}",
                explode.damage
            );
            assert!(
                (explode.station_wear - 0.01).abs() < f32::EPSILON,
                "{bp_id} station_wear should be 0.01 (lower than iron), got {}",
                explode.station_wear
            );
        }
    }

    #[test]
    fn bone_helmet_v0_needs_yi_shou_gu_x2() {
        let reg = BlueprintRegistry::load_dir(DEFAULT_BLUEPRINTS_DIR).unwrap();
        let bp = reg.get("bone_helmet_v0").unwrap();
        let Some(StepSpec::Billet { profile }) = bp.steps.first() else {
            panic!("bone_helmet first step should be billet");
        };
        assert_eq!(profile.required.len(), 1);
        assert_eq!(profile.required[0].material, "yi_shou_gu");
        assert_eq!(profile.required[0].count, 2);
    }

    #[test]
    fn bone_chestplate_v0_needs_yi_shou_gu_x4() {
        let reg = BlueprintRegistry::load_dir(DEFAULT_BLUEPRINTS_DIR).unwrap();
        let bp = reg.get("bone_chestplate_v0").unwrap();
        let Some(StepSpec::Billet { profile }) = bp.steps.first() else {
            panic!("bone_chestplate first step should be billet");
        };
        assert_eq!(
            profile.required.len(), 1,
            "bone_chestplate_v0 should have exactly 1 required material, got {}",
            profile.required.len()
        );
        assert_eq!(
            profile.required[0].material, "yi_shou_gu",
            "bone chestplate material should be yi_shou_gu, got {}",
            profile.required[0].material
        );
        assert_eq!(
            profile.required[0].count, 4,
            "bone chestplate is the biggest piece, needs 4 yi_shou_gu"
        );
    }

    #[test]
    fn bone_dagger_v0_outcome_quality_coefficients() {
        let reg = BlueprintRegistry::load_dir(DEFAULT_BLUEPRINTS_DIR).unwrap();
        let bp = reg.get("bone_dagger_v0").unwrap();
        let perfect = bp.outcomes.perfect.as_ref().unwrap();
        assert!(
            (perfect.quality - 1.0).abs() < f32::EPSILON,
            "bone_dagger perfect quality should be 1.0, got {}",
            perfect.quality
        );
        let good = bp.outcomes.good.as_ref().unwrap();
        assert!(
            (good.quality - 0.8).abs() < f32::EPSILON,
            "bone_dagger good quality should be 0.8, got {}",
            good.quality
        );
        let flawed = bp.outcomes.flawed.as_ref().unwrap();
        assert!(
            (flawed.quality - 0.5).abs() < f32::EPSILON,
            "bone_dagger flawed quality should be 0.5, got {}",
            flawed.quality
        );
    }

    #[test]
    fn bone_armor_blueprint_scrolls_parse_and_link_correctly() {
        let item_reg = crate::inventory::load_item_registry().expect("item registry should load");
        for (item_id, blueprint_id) in [
            ("blueprint_scroll_bone_helmet", "bone_helmet_v0"),
            ("blueprint_scroll_bone_chestplate", "bone_chestplate_v0"),
            ("blueprint_scroll_bone_leggings", "bone_leggings_v0"),
            ("blueprint_scroll_bone_boots", "bone_boots_v0"),
        ] {
            let item = item_reg
                .get(item_id)
                .unwrap_or_else(|| panic!("expected {item_id} in item registry"));
            let scroll = item
                .blueprint_scroll_spec
                .as_ref()
                .unwrap_or_else(|| panic!("expected blueprint_scroll_spec for {item_id}"));
            assert_eq!(
                scroll.blueprint_id, blueprint_id,
                "{item_id} should map to forge blueprint {blueprint_id}, got {}",
                scroll.blueprint_id
            );
        }
    }
}
