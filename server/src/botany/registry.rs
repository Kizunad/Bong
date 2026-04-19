//! plan-botany-v1 §1 — `PlantKindRegistry` 资源 + TOML loader。

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::Deserialize;
use valence::prelude::Resource;

use super::plant_kind::{PlantId, PlantKind};

const DEFAULT_PLANTS_PATH: &str = "assets/botany/plants.toml";

#[derive(Debug, Default)]
pub struct PlantKindRegistry {
    plants: HashMap<PlantId, PlantKind>,
}

impl Resource for PlantKindRegistry {}

impl PlantKindRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, plant: PlantKind) -> Result<(), String> {
        if self.plants.contains_key(&plant.id) {
            return Err(format!("duplicate plant id: {}", plant.id));
        }
        self.plants.insert(plant.id.clone(), plant);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&PlantKind> {
        self.plants.get(id)
    }

    pub fn len(&self) -> usize {
        self.plants.len()
    }

    pub fn is_empty(&self) -> bool {
        self.plants.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&PlantId, &PlantKind)> {
        self.plants.iter()
    }

    /// plan-lingtian-v1 §4 — `SeedRegistry` 由可种植子集派生。
    pub fn cultivable_ids(&self) -> impl Iterator<Item = &PlantId> {
        self.plants
            .iter()
            .filter_map(|(id, kind)| kind.cultivable.then_some(id))
    }
}

#[derive(Debug, Deserialize)]
struct PlantsFile {
    #[serde(default)]
    plant: Vec<PlantKind>,
}

pub fn load_plant_kind_registry() -> Result<PlantKindRegistry, String> {
    load_plant_kind_registry_from(Path::new(DEFAULT_PLANTS_PATH))
}

pub fn load_plant_kind_registry_from(path: &Path) -> Result<PlantKindRegistry, String> {
    let raw = fs::read_to_string(path).map_err(|e| format!("read {}: {}", path.display(), e))?;
    let parsed: PlantsFile =
        toml::from_str(&raw).map_err(|e| format!("parse {}: {}", path.display(), e))?;
    let mut registry = PlantKindRegistry::new();
    for plant in parsed.plant {
        registry.insert(plant)?;
    }
    Ok(registry)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_default_assets_with_test_trio() {
        let registry = load_plant_kind_registry().expect("default plants.toml should load");
        assert!(
            registry.get("ci_she_hao").is_some(),
            "ci_she_hao 是 §3.1 测试三作物之一"
        );
        assert!(registry.get("ning_mai_cao").is_some());
        assert!(registry.get("ling_mu_miao").is_some());
    }

    #[test]
    fn cultivable_filter_excludes_wild_only() {
        let registry = load_plant_kind_registry().unwrap();
        let cultivable: Vec<_> = registry.cultivable_ids().cloned().collect();
        assert!(cultivable.iter().any(|id| id == "ci_she_hao"));
        if let Some(plant) = registry.get("shi_mai_gen") {
            assert!(!plant.cultivable, "shi_mai_gen 必须 cultivable=false");
        }
    }
}
