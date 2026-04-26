//! plan-armor-v1 §1 — 装备护甲数据模型与注册表。
//!
//! 本模块只负责：
//! - `ArmorProfile` 数据结构（slot/覆盖部位/按 WoundKind 的减免系数/耐久）
//! - `ArmorProfileRegistry` resource（template_id -> ArmorProfile blueprint）
//! - 启动期从 `server/assets/combat/armor_profiles/*.json` 扫描加载。
//!
//! 设计意图：
//! - 运行时结算侧只读 `DerivedAttrs.defense_profile`（二维矩阵），不直接依赖 inventory。
//! - registry 以 `template_id` 为 key，避免与运行时分配的 `instance_id` 耦合。
//! - 单件护甲的运行时耐久使用 inventory 侧 `ItemInstance.durability` (0..=1) 表示；
//!   破损判定亦以此为准。

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use valence::prelude::Resource;

use crate::combat::components::{BodyPart, WoundKind};
use crate::schema::inventory::EquipSlotV1;

pub const DEFAULT_ARMOR_PROFILES_DIR: &str = "assets/combat/armor_profiles";
pub const ARMOR_MITIGATION_CAP: f32 = 0.85;
pub const ARMOR_BROKEN_MULTIPLIER_DEFAULT: f32 = 0.3;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ArmorProfile {
    pub slot: EquipSlotV1,
    pub body_coverage: Vec<BodyPart>,
    pub kind_mitigation: HashMap<WoundKind, f32>,
    pub durability_max: u32,
    #[serde(default = "default_broken_multiplier")]
    pub broken_multiplier: f32,
}

fn default_broken_multiplier() -> f32 {
    ARMOR_BROKEN_MULTIPLIER_DEFAULT
}

impl ArmorProfile {
    pub fn validate(&self) -> Result<(), String> {
        if self.body_coverage.is_empty() {
            return Err("body_coverage must not be empty".to_string());
        }
        if self.durability_max == 0 {
            return Err("durability_max must be >= 1".to_string());
        }
        if !self.broken_multiplier.is_finite() || !(0.0..=1.0).contains(&self.broken_multiplier) {
            return Err(format!(
                "broken_multiplier {} must be finite in [0, 1]",
                self.broken_multiplier
            ));
        }

        // EquipSlotV1 有很多槽位；MVP 只允许护甲槽写入，避免把武器塞进来导致
        // coverage 矩阵污染。
        match self.slot {
            EquipSlotV1::Head | EquipSlotV1::Chest | EquipSlotV1::Legs | EquipSlotV1::Feet => {}
            _ => {
                return Err(format!(
                    "slot {:?} is not an armor slot (expected head/chest/legs/feet)",
                    self.slot
                ));
            }
        }

        // coverage 不允许重复。
        let mut seen = HashSet::new();
        for part in &self.body_coverage {
            if !seen.insert(*part) {
                return Err(format!("duplicate body_coverage entry: {:?}", part));
            }
        }

        for (kind, mitigation) in &self.kind_mitigation {
            if !mitigation.is_finite() {
                return Err(format!("mitigation for {:?} must be finite", kind));
            }
            if !(0.0..=ARMOR_MITIGATION_CAP).contains(mitigation) {
                return Err(format!(
                    "mitigation for {:?}={} out of range [0, {}]",
                    kind, mitigation, ARMOR_MITIGATION_CAP
                ));
            }
        }

        Ok(())
    }

    pub fn effective_multiplier_for_durability_ratio(&self, durability_ratio: f64) -> f32 {
        if !durability_ratio.is_finite() {
            return self.broken_multiplier;
        }
        if durability_ratio <= 0.0 {
            self.broken_multiplier
        } else {
            1.0
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct ArmorProfileRegistry {
    by_template_id: HashMap<String, ArmorProfile>,
}

impl Resource for ArmorProfileRegistry {}

#[derive(Debug)]
pub enum ArmorProfileLoadError {
    Io(std::io::Error),
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    DuplicateTemplateId(String),
    Invalid {
        path: PathBuf,
        template_id: String,
        reason: String,
    },
}

impl std::fmt::Display for ArmorProfileLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArmorProfileLoadError::Io(e) => write!(f, "io: {e}"),
            ArmorProfileLoadError::Json { path, source } => {
                write!(f, "json: {}: {source}", path.display())
            }
            ArmorProfileLoadError::DuplicateTemplateId(id) => {
                write!(f, "duplicate armor profile template_id {id}")
            }
            ArmorProfileLoadError::Invalid {
                path,
                template_id,
                reason,
            } => write!(
                f,
                "invalid armor profile {} (template_id={template_id}): {reason}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for ArmorProfileLoadError {}

impl From<std::io::Error> for ArmorProfileLoadError {
    fn from(e: std::io::Error) -> Self {
        ArmorProfileLoadError::Io(e)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArmorProfileFile {
    template_id: String,
    profile: ArmorProfile,
}

impl ArmorProfileRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.by_template_id.len()
    }

    // TODO: plan-armor-v1 后续 milestone 接入后取消 allow（is_empty 供 registry 消费者使用）
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.by_template_id.is_empty()
    }

    pub fn get(&self, template_id: &str) -> Option<&ArmorProfile> {
        self.by_template_id.get(template_id)
    }

    // TODO: plan-armor-v1 后续 milestone 接入后取消 allow（insert 供装备 sync 手动装载用）
    #[allow(dead_code)]
    pub fn insert(&mut self, template_id: String, profile: ArmorProfile) -> Result<(), String> {
        let template_id = template_id.trim().to_string();
        if template_id.is_empty() {
            return Err("template_id must not be empty".to_string());
        }
        profile.validate()?;
        self.by_template_id.insert(template_id, profile);
        Ok(())
    }

    pub fn load_dir(path: impl AsRef<Path>) -> Result<Self, ArmorProfileLoadError> {
        let dir = path.as_ref();
        let mut reg = Self::new();
        if !dir.exists() {
            tracing::warn!(
                "[bong][combat][armor] armor_profiles dir {} does not exist — registry empty",
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
            let parsed: ArmorProfileFile =
                serde_json::from_str(&text).map_err(|e| ArmorProfileLoadError::Json {
                    path: path.clone(),
                    source: e,
                })?;
            let template_id = parsed.template_id.trim().to_string();
            if template_id.is_empty() {
                return Err(ArmorProfileLoadError::Invalid {
                    path: path.clone(),
                    template_id,
                    reason: "template_id must not be empty".to_string(),
                });
            }
            if reg.by_template_id.contains_key(&template_id) {
                return Err(ArmorProfileLoadError::DuplicateTemplateId(template_id));
            }
            if let Err(reason) = parsed.profile.validate() {
                return Err(ArmorProfileLoadError::Invalid {
                    path: path.clone(),
                    template_id,
                    reason,
                });
            }
            tracing::info!(
                "[bong][combat][armor] loaded armor profile template_id={} slot={:?}",
                template_id,
                parsed.profile.slot
            );
            reg.by_template_id.insert(template_id, parsed.profile);
        }

        Ok(reg)
    }

    #[cfg(test)]
    pub fn from_map(map: HashMap<String, ArmorProfile>) -> Self {
        Self {
            by_template_id: map,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_registry_is_empty() {
        let r = ArmorProfileRegistry::new();
        assert!(r.is_empty());
        assert_eq!(r.len(), 0);
        assert!(r.get("fake_spirit_hide").is_none());
    }

    #[test]
    fn armor_profile_validate_rejects_non_armor_slot() {
        let profile = ArmorProfile {
            slot: EquipSlotV1::MainHand,
            body_coverage: vec![BodyPart::Chest],
            kind_mitigation: HashMap::from([(WoundKind::Cut, 0.1)]),
            durability_max: 1,
            broken_multiplier: 0.3,
        };
        assert!(profile.validate().is_err());
    }

    #[test]
    fn loads_default_blueprint_json_from_assets() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_ARMOR_PROFILES_DIR);
        let registry = ArmorProfileRegistry::load_dir(dir).expect("load armor profile assets");
        let profile = registry
            .get("fake_spirit_hide")
            .expect("fake_spirit_hide armor profile should be loaded from blueprint JSON");

        assert_eq!(registry.len(), 1);
        assert_eq!(profile.slot, EquipSlotV1::Chest);
        assert_eq!(
            profile.body_coverage,
            vec![BodyPart::Chest, BodyPart::Abdomen]
        );
        assert_eq!(profile.kind_mitigation.get(&WoundKind::Cut), Some(&0.25));
        assert_eq!(profile.durability_max, 100);
        assert_eq!(profile.broken_multiplier, 0.3);
    }
}
