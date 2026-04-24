//! plan-armor-v1 §1 — 装备护甲数据模型与注册表。
//!
//! 本模块只负责：
//! - `ArmorProfile` 数据结构（slot/覆盖部位/按 WoundKind 的减免系数/耐久）
//! - `ArmorProfileRegistry` resource（instance_id -> ArmorProfile）
//! - 启动期从 `server/assets/combat/armor_profiles/*.json` 扫描加载。
//!
//! 设计意图：
//! - 运行时结算侧只读 `DerivedAttrs.defense_profile`（二维矩阵），不直接依赖 inventory。
//! - registry 以 `instance_id` 为 key，避免给 ItemInstance 增肥；同时允许同模板不同 instance
//!   有不同耐久。

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
    pub durability_cur: u32,
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
        if self.durability_cur > self.durability_max {
            return Err(format!(
                "durability_cur {} exceeds durability_max {}",
                self.durability_cur, self.durability_max
            ));
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

    pub fn is_broken(&self) -> bool {
        self.durability_cur == 0
    }

    pub fn effective_multiplier(&self) -> f32 {
        if self.is_broken() {
            self.broken_multiplier
        } else {
            1.0
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct ArmorProfileRegistry {
    by_instance_id: HashMap<u64, ArmorProfile>,
}

impl Resource for ArmorProfileRegistry {}

#[derive(Debug)]
pub enum ArmorProfileLoadError {
    Io(std::io::Error),
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    DuplicateInstanceId(u64),
    Invalid {
        path: PathBuf,
        instance_id: u64,
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
            ArmorProfileLoadError::DuplicateInstanceId(id) => {
                write!(f, "duplicate armor profile instance_id {id}")
            }
            ArmorProfileLoadError::Invalid {
                path,
                instance_id,
                reason,
            } => write!(
                f,
                "invalid armor profile {} (instance_id={instance_id}): {reason}",
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
    #[serde(deserialize_with = "deserialize_instance_id")]
    instance_id: u64,
    profile: ArmorProfile,
}

fn deserialize_instance_id<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    // 与 inventory schema 里的 JS safe integer 约束保持一致。
    const JS_SAFE_INTEGER_MAX: u64 = 9_007_199_254_740_991;

    let id = u64::deserialize(deserializer)?;
    if id == 0 {
        return Err(serde::de::Error::custom("instance_id must be >= 1"));
    }
    if id > JS_SAFE_INTEGER_MAX {
        return Err(serde::de::Error::custom(format!(
            "instance_id {id} exceeds JS safe integer max {JS_SAFE_INTEGER_MAX}"
        )));
    }
    Ok(id)
}

impl ArmorProfileRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.by_instance_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_instance_id.is_empty()
    }

    pub fn get(&self, instance_id: u64) -> Option<&ArmorProfile> {
        self.by_instance_id.get(&instance_id)
    }

    pub fn insert(&mut self, instance_id: u64, profile: ArmorProfile) -> Result<(), String> {
        profile.validate()?;
        self.by_instance_id.insert(instance_id, profile);
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
            if reg.by_instance_id.contains_key(&parsed.instance_id) {
                return Err(ArmorProfileLoadError::DuplicateInstanceId(
                    parsed.instance_id,
                ));
            }
            if let Err(reason) = parsed.profile.validate() {
                return Err(ArmorProfileLoadError::Invalid {
                    path: path.clone(),
                    instance_id: parsed.instance_id,
                    reason,
                });
            }
            tracing::info!(
                "[bong][combat][armor] loaded armor profile instance_id={} slot={:?}",
                parsed.instance_id,
                parsed.profile.slot
            );
            reg.by_instance_id.insert(parsed.instance_id, parsed.profile);
        }

        Ok(reg)
    }

    #[cfg(test)]
    pub fn from_map(map: HashMap<u64, ArmorProfile>) -> Self {
        Self {
            by_instance_id: map,
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
        assert!(r.get(1).is_none());
    }

    #[test]
    fn armor_profile_validate_rejects_non_armor_slot() {
        let profile = ArmorProfile {
            slot: EquipSlotV1::MainHand,
            body_coverage: vec![BodyPart::Chest],
            kind_mitigation: HashMap::from([(WoundKind::Cut, 0.1)]),
            durability_cur: 1,
            durability_max: 1,
            broken_multiplier: 0.3,
        };
        assert!(profile.validate().is_err());
    }
}
