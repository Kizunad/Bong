use serde::{Deserialize, Serialize};
use valence::prelude::{DVec3, Resource};

use super::{
    DEFAULT_SPAWN_BOUNDS_MAX, DEFAULT_SPAWN_BOUNDS_MIN, DEFAULT_SPAWN_POSITION, DEFAULT_SPAWN_ZONE,
};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ZoneAabb {
    pub min: [f64; 3],
    pub max: [f64; 3],
}

impl ZoneAabb {
    pub const fn new(min: [f64; 3], max: [f64; 3]) -> Self {
        Self { min, max }
    }

    pub fn contains(&self, position: DVec3) -> bool {
        position.x >= self.min[0]
            && position.x <= self.max[0]
            && position.y >= self.min[1]
            && position.y <= self.max[1]
            && position.z >= self.min[2]
            && position.z <= self.max[2]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Zone {
    pub name: String,
    pub bounds: ZoneAabb,
    pub spawn_position: [f64; 3],
    pub spirit_qi: f64,
    pub danger_level: u8,
    #[serde(default)]
    pub active_events: Vec<String>,
}

impl Zone {
    fn fallback_spawn() -> Self {
        Self {
            name: DEFAULT_SPAWN_ZONE.to_string(),
            bounds: ZoneAabb::new(DEFAULT_SPAWN_BOUNDS_MIN, DEFAULT_SPAWN_BOUNDS_MAX),
            spawn_position: DEFAULT_SPAWN_POSITION,
            spirit_qi: 0.9,
            danger_level: 0,
            active_events: vec![],
        }
    }
}

#[derive(Debug, Clone)]
pub struct ZoneRegistry {
    zones: Vec<Zone>,
    default_zone_name: String,
}

impl Resource for ZoneRegistry {}

impl ZoneRegistry {
    pub fn from_optional_zones(zones: Option<Vec<Zone>>) -> Self {
        match zones {
            Some(zones) if !zones.is_empty() => Self::from_zones(zones),
            _ => Self::fallback(),
        }
    }

    pub fn fallback() -> Self {
        Self::from_zones(vec![Zone::fallback_spawn()])
    }

    fn from_zones(zones: Vec<Zone>) -> Self {
        let default_zone_name = zones
            .iter()
            .find(|zone| zone.name == DEFAULT_SPAWN_ZONE)
            .map(|zone| zone.name.clone())
            .unwrap_or_else(|| zones[0].name.clone());

        Self {
            zones,
            default_zone_name,
        }
    }

    pub fn zones(&self) -> &[Zone] {
        &self.zones
    }

    pub fn default_zone(&self) -> &Zone {
        self.get_zone(&self.default_zone_name)
            .expect("default zone should exist in the registry")
    }

    pub fn get_zone(&self, name: &str) -> Option<&Zone> {
        self.zones.iter().find(|zone| zone.name == name)
    }

    pub fn find_zone(&self, position: DVec3) -> Option<&Zone> {
        self.zones
            .iter()
            .find(|zone| zone.bounds.contains(position))
    }

    pub fn find_zone_or_default(&self, position: DVec3) -> &Zone {
        self.find_zone(position)
            .unwrap_or_else(|| self.default_zone())
    }

    #[allow(dead_code)]
    pub fn find_zone_mut(&mut self, name: &str) -> Option<&mut Zone> {
        self.zones.iter_mut().find(|zone| zone.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_has_single_zone() {
        let registry = ZoneRegistry::fallback();
        assert_eq!(registry.zones().len(), 1);
    }

    #[test]
    fn find_zone_returns_spawn_for_spawn_position() {
        let registry = ZoneRegistry::fallback();
        let spawn = registry.find_zone_or_default(DVec3::new(
            DEFAULT_SPAWN_POSITION[0],
            DEFAULT_SPAWN_POSITION[1],
            DEFAULT_SPAWN_POSITION[2],
        ));

        assert_eq!(spawn.name, DEFAULT_SPAWN_ZONE);
        assert_eq!(
            spawn.bounds,
            ZoneAabb::new(DEFAULT_SPAWN_BOUNDS_MIN, DEFAULT_SPAWN_BOUNDS_MAX)
        );
    }

    #[test]
    fn missing_zone_config_returns_spawn_fallback() {
        let registry = ZoneRegistry::from_optional_zones(None);

        assert_eq!(registry.default_zone().name, DEFAULT_SPAWN_ZONE);
        assert!(registry.get_zone(DEFAULT_SPAWN_ZONE).is_some());
    }
}
