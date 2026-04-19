//! plan-lingtian-v1 §1.2.4 / §4 — 种子 ↔ 植物 双向映射。
//!
//! 由 `botany::PlantKindRegistry` 派生（仅取 `cultivable=true` 子集），
//! 在 `register(app)` 时构建一次。运行时不变。
//!
//! 命名约定：`{plant_id}_seed`，与 `assets/items/seeds.toml` 的 item id 对齐。

use std::collections::HashMap;

use valence::prelude::{bevy_ecs, Resource};

use crate::botany::{PlantId, PlantKindRegistry};

#[derive(Debug, Default, Resource)]
pub struct SeedRegistry {
    seed_to_plant: HashMap<String, PlantId>,
    plant_to_seed: HashMap<PlantId, String>,
}

pub fn seed_id_for(plant_id: &str) -> String {
    format!("{plant_id}_seed")
}

impl SeedRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_plant_registry(registry: &PlantKindRegistry) -> Self {
        let mut s = Self::new();
        for plant_id in registry.cultivable_ids() {
            s.insert(plant_id.clone());
        }
        s
    }

    fn insert(&mut self, plant_id: PlantId) {
        let seed_id = seed_id_for(&plant_id);
        self.seed_to_plant.insert(seed_id.clone(), plant_id.clone());
        self.plant_to_seed.insert(plant_id, seed_id);
    }

    pub fn plant_for_seed(&self, seed_id: &str) -> Option<&PlantId> {
        self.seed_to_plant.get(seed_id)
    }

    pub fn seed_for_plant(&self, plant_id: &str) -> Option<&String> {
        self.plant_to_seed.get(plant_id)
    }

    pub fn len(&self) -> usize {
        self.seed_to_plant.len()
    }

    pub fn is_empty(&self) -> bool {
        self.seed_to_plant.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::botany::{GrowthCost, PlantKind, PlantKindRegistry, PlantRarity};

    fn kind(id: &str, cultivable: bool) -> PlantKind {
        PlantKind {
            id: id.into(),
            display_name: id.into(),
            cultivable,
            growth_cost: GrowthCost::Low,
            growth_duration_ticks: 100,
            rarity: PlantRarity::Common,
            description: String::new(),
        }
    }

    #[test]
    fn derives_from_cultivable_only() {
        let mut reg = PlantKindRegistry::new();
        reg.insert(kind("ci_she_hao", true)).unwrap();
        reg.insert(kind("shi_mai_gen", false)).unwrap();
        let seeds = SeedRegistry::from_plant_registry(&reg);
        assert_eq!(seeds.len(), 1);
        assert_eq!(
            seeds.plant_for_seed("ci_she_hao_seed"),
            Some(&"ci_she_hao".to_string())
        );
        assert!(seeds.plant_for_seed("shi_mai_gen_seed").is_none());
    }

    #[test]
    fn round_trip_lookup() {
        let mut reg = PlantKindRegistry::new();
        reg.insert(kind("ning_mai_cao", true)).unwrap();
        let seeds = SeedRegistry::from_plant_registry(&reg);
        let seed = seeds.seed_for_plant("ning_mai_cao").unwrap().clone();
        assert_eq!(seed, "ning_mai_cao_seed");
        assert_eq!(seeds.plant_for_seed(&seed), Some(&"ning_mai_cao".into()));
    }

    #[test]
    fn unknown_lookups_return_none() {
        let seeds = SeedRegistry::new();
        assert!(seeds.plant_for_seed("anything_seed").is_none());
        assert!(seeds.seed_for_plant("anything").is_none());
    }
}
