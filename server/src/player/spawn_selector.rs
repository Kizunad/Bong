use std::fs;
use std::path::Path;

use serde::Deserialize;
use valence::prelude::DVec3;

use crate::world::zone::{ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};

pub const EMERGENCY_SPAWN_POSITION: [f64; 3] = [8.0, 150.0, 8.0];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpawnPurpose {
    InitialLogin,
    NewLifeBirth,
    DevSpawnCommand,
    FallRecovery,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerSpawnSelector<'a> {
    registry: Option<&'a ZoneRegistry>,
    distribution: Vec<SpawnDistributionAnchor>,
}

#[derive(Debug, Clone, PartialEq)]
struct SpawnDistributionAnchor {
    anchor: DVec3,
    radius: f64,
    weight: u32,
    safe_y: f64,
}

#[derive(Debug, Deserialize)]
struct SpawnDistributionFile {
    zones: Vec<SpawnDistributionZone>,
}

#[derive(Debug, Deserialize)]
struct SpawnDistributionZone {
    name: String,
    #[serde(default)]
    spawn_distribution: Vec<SpawnDistributionAnchorConfig>,
}

#[derive(Debug, Deserialize)]
struct SpawnDistributionAnchorConfig {
    anchor: [f64; 3],
    radius: f64,
    weight: u32,
    safe_y: f64,
}

impl<'a> PlayerSpawnSelector<'a> {
    pub fn new(registry: &'a ZoneRegistry) -> Self {
        Self {
            registry: Some(registry),
            distribution: distribution_from_zone_patrol_anchors(registry),
        }
    }

    fn with_distribution(
        registry: &'a ZoneRegistry,
        distribution: Vec<SpawnDistributionAnchor>,
    ) -> Self {
        let distribution = if distribution.is_empty() {
            distribution_from_zone_patrol_anchors(registry)
        } else {
            distribution
        };
        Self {
            registry: Some(registry),
            distribution,
        }
    }

    #[cfg(test)]
    pub fn fallback() -> Self {
        Self {
            registry: None,
            distribution: Vec::new(),
        }
    }

    pub fn select(&self, seed: &str, purpose: SpawnPurpose) -> [f64; 3] {
        let Some(registry) = self.registry else {
            tracing::warn!(
                "[bong][player] spawn selector has no ZoneRegistry; using emergency fallback spawn"
            );
            return EMERGENCY_SPAWN_POSITION;
        };
        let Some(zone) = registry.find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME) else {
            tracing::warn!(
                "[bong][player] spawn selector cannot find `{DEFAULT_SPAWN_ZONE_NAME}` zone; using emergency fallback spawn"
            );
            return EMERGENCY_SPAWN_POSITION;
        };
        if self.distribution.is_empty() {
            tracing::warn!(
                "[bong][player] `{DEFAULT_SPAWN_ZONE_NAME}` has no spawn_distribution; using emergency fallback spawn"
            );
            return EMERGENCY_SPAWN_POSITION;
        }

        let hash = stable_hash(seed, purpose);
        let total_weight: u64 = self
            .distribution
            .iter()
            .map(|anchor| u64::from(anchor.weight))
            .sum::<u64>()
            .max(1);
        let mut pick = hash % total_weight.max(1);
        let selected = self
            .distribution
            .iter()
            .find(|anchor| {
                let weight = u64::from(anchor.weight);
                if pick < weight {
                    true
                } else {
                    pick -= weight;
                    false
                }
            })
            .unwrap_or(&self.distribution[0]);

        let radius_bits = hash.rotate_left(17);
        let angle_bits = hash.rotate_left(41);
        let radius_fraction = (radius_bits & 0xffff) as f64 / 65_535.0;
        let angle_fraction = (angle_bits & 0xffff) as f64 / 65_535.0;
        let radius = selected.radius * radius_fraction.sqrt();
        let angle = angle_fraction * std::f64::consts::TAU;
        let candidate = DVec3::new(
            selected.anchor.x + radius * angle.cos(),
            selected.safe_y,
            selected.anchor.z + radius * angle.sin(),
        );

        let clamped = zone.clamp_position(candidate);
        if zone
            .blocked_tiles
            .iter()
            .any(|(x, z)| *x == clamped.x.floor() as i32 && *z == clamped.z.floor() as i32)
        {
            return [selected.anchor.x, selected.safe_y, selected.anchor.z];
        }

        [clamped.x, clamped.y, clamped.z]
    }
}

fn stable_hash(seed: &str, purpose: SpawnPurpose) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in format!("{purpose:?}:{seed}").bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

pub fn fallback_spawn(seed: &str, purpose: SpawnPurpose) -> [f64; 3] {
    let registry = ZoneRegistry::load();
    let distribution = load_default_spawn_distribution().unwrap_or_else(|error| {
        tracing::warn!(
            "[bong][player] failed to load spawn_distribution from zones.json: {error}; using patrol anchor fallback"
        );
        Vec::new()
    });
    if distribution.is_empty() {
        PlayerSpawnSelector::new(&registry).select(seed, purpose)
    } else {
        PlayerSpawnSelector::with_distribution(&registry, distribution).select(seed, purpose)
    }
}

pub fn emergency_spawn_position() -> [f64; 3] {
    EMERGENCY_SPAWN_POSITION
}

fn load_default_spawn_distribution() -> Result<Vec<SpawnDistributionAnchor>, String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(crate::world::zone::DEFAULT_ZONES_PATH);
    load_spawn_distribution_from_path(path)
}

fn load_spawn_distribution_from_path(
    path: impl AsRef<Path>,
) -> Result<Vec<SpawnDistributionAnchor>, String> {
    let contents = fs::read_to_string(path.as_ref()).map_err(|error| error.to_string())?;
    let config: SpawnDistributionFile =
        serde_json::from_str(&contents).map_err(|error| error.to_string())?;
    let zone = config
        .zones
        .into_iter()
        .find(|zone| zone.name == DEFAULT_SPAWN_ZONE_NAME)
        .ok_or_else(|| format!("missing `{DEFAULT_SPAWN_ZONE_NAME}` zone"))?;

    let mut distribution = Vec::with_capacity(zone.spawn_distribution.len());
    for (index, anchor) in zone.spawn_distribution.into_iter().enumerate() {
        if !anchor.radius.is_finite() || anchor.radius < 0.0 {
            return Err(format!(
                "spawn_distribution[{index}].radius must be a finite non-negative value"
            ));
        }
        if anchor.weight == 0 {
            return Err(format!(
                "spawn_distribution[{index}].weight must be greater than 0"
            ));
        }
        if !anchor.safe_y.is_finite() || !anchor.anchor.into_iter().all(f64::is_finite) {
            return Err(format!(
                "spawn_distribution[{index}] must contain only finite coordinates"
            ));
        }
        distribution.push(SpawnDistributionAnchor {
            anchor: DVec3::new(anchor.anchor[0], anchor.anchor[1], anchor.anchor[2]),
            radius: anchor.radius,
            weight: anchor.weight,
            safe_y: anchor.safe_y,
        });
    }

    Ok(distribution)
}

fn distribution_from_zone_patrol_anchors(registry: &ZoneRegistry) -> Vec<SpawnDistributionAnchor> {
    registry
        .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
        .map(|zone| {
            zone.patrol_anchors
                .iter()
                .map(|anchor| SpawnDistributionAnchor {
                    anchor: *anchor,
                    radius: 64.0,
                    weight: 1,
                    safe_y: anchor.y + 2.0,
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::zone::ZoneRegistry;

    #[test]
    fn same_seed_and_purpose_are_stable() {
        let registry = ZoneRegistry::load();
        let selector = PlayerSpawnSelector::new(&registry);

        let first = selector.select("offline:Alice", SpawnPurpose::InitialLogin);
        let second = selector.select("offline:Alice", SpawnPurpose::InitialLogin);

        assert_eq!(first, second);
    }

    #[test]
    fn different_seeds_are_distributed_inside_spawn_zone() {
        let registry = ZoneRegistry::load();
        let selector = PlayerSpawnSelector::with_distribution(
            &registry,
            load_default_spawn_distribution().expect("spawn_distribution should load"),
        );
        let spawn_zone = registry
            .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
            .expect("spawn zone should load");

        let alice = selector.select("offline:Alice", SpawnPurpose::InitialLogin);
        let bob = selector.select("offline:Bob", SpawnPurpose::InitialLogin);

        assert_ne!(alice, bob);
        assert_ne!(alice, EMERGENCY_SPAWN_POSITION);
        assert!(spawn_zone.contains(DVec3::new(alice[0], alice[1], alice[2])));
        assert!(spawn_zone.contains(DVec3::new(bob[0], bob[1], bob[2])));
    }

    #[test]
    fn purpose_participates_in_seed() {
        let registry = ZoneRegistry::load();
        let selector = PlayerSpawnSelector::with_distribution(
            &registry,
            load_default_spawn_distribution().expect("spawn_distribution should load"),
        );

        let initial = selector.select("offline:Alice", SpawnPurpose::InitialLogin);
        let new_life = selector.select("offline:Alice:char-2", SpawnPurpose::NewLifeBirth);

        assert_ne!(initial, new_life);
    }

    #[test]
    fn missing_registry_uses_named_emergency_fallback() {
        let selector = PlayerSpawnSelector::fallback();

        assert_eq!(
            selector.select("offline:Alice", SpawnPurpose::InitialLogin),
            EMERGENCY_SPAWN_POSITION,
        );
    }

    #[test]
    fn zones_json_declares_spawn_distribution() {
        let distribution =
            load_default_spawn_distribution().expect("spawn_distribution should parse");

        assert!(distribution.len() >= 3);
        assert!(distribution.iter().all(|anchor| anchor.weight > 0));
    }
}
