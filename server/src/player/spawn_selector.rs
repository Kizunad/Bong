use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

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
pub(crate) struct SpawnDistributionAnchor {
    anchor: DVec3,
    radius: f64,
    weight: u32,
    safe_y: f64,
}

impl SpawnDistributionAnchor {
    pub(crate) fn cluster(&self) -> (DVec3, f64) {
        (self.anchor, self.radius)
    }

    pub(crate) fn anchor(&self) -> DVec3 {
        self.anchor
    }
}

#[cfg(test)]
pub(crate) fn spawn_distribution_anchor_for_test(
    anchor: DVec3,
    radius: f64,
) -> SpawnDistributionAnchor {
    SpawnDistributionAnchor {
        anchor,
        radius,
        weight: 1,
        safe_y: anchor.y,
    }
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
            let fallback = zone.clamp_position(DVec3::new(
                selected.anchor.x,
                selected.safe_y,
                selected.anchor.z,
            ));
            return [fallback.x, fallback.y, fallback.z];
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

/// 启动时一次性加载的不可变快照：zone 注册表 + 钳制后的出生分布。
/// 世界构建（chunk union）与出生选择（fallback_spawn）都从这一个权威来源读取，
/// 避免两条路径各自读 zones.json 导致 chunk 分配与 spawn 选择不一致（finding 3/4/5）。
pub(crate) struct FallbackSpawnSnapshot {
    pub(crate) registry: ZoneRegistry,
    pub(crate) distribution: Vec<SpawnDistributionAnchor>,
}

static FALLBACK_SPAWN_SNAPSHOT: OnceLock<FallbackSpawnSnapshot> = OnceLock::new();

pub(crate) fn fallback_spawn_snapshot() -> &'static FallbackSpawnSnapshot {
    FALLBACK_SPAWN_SNAPSHOT.get_or_init(FallbackSpawnSnapshot::load)
}

impl FallbackSpawnSnapshot {
    fn load() -> Self {
        let registry = ZoneRegistry::load();
        let distribution = clamp_distribution_to_spawn_zone(
            &registry,
            effective_default_spawn_distribution(&registry),
        );
        Self {
            registry,
            distribution,
        }
    }
}

/// 把分布锚点钳制进出生 zone 的 AABB，保证任何锚点派生出的 spawn 都落在
/// `fallback_spawn_chunk_union` 已分配的 chunk 内。当前 zones.json 全部在界内
/// （钳制是 no-op），这是对未来配置错误的防御。
fn clamp_distribution_to_spawn_zone(
    registry: &ZoneRegistry,
    distribution: Vec<SpawnDistributionAnchor>,
) -> Vec<SpawnDistributionAnchor> {
    let Some(spawn_zone) = registry.find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME) else {
        return distribution;
    };
    let (zone_min, zone_max) = spawn_zone.bounds;
    let mut clamped_changes = 0;
    let clamped = distribution
        .into_iter()
        .map(|anchor| {
            let clamped_anchor = spawn_zone.clamp_position(anchor.anchor);
            let clamped_safe_y = anchor.safe_y.clamp(zone_min.y, zone_max.y);
            if clamped_anchor != anchor.anchor || clamped_safe_y != anchor.safe_y {
                clamped_changes += 1;
            }
            SpawnDistributionAnchor {
                anchor: clamped_anchor,
                radius: anchor.radius,
                weight: anchor.weight,
                safe_y: clamped_safe_y,
            }
        })
        .collect::<Vec<_>>();
    if clamped_changes > 0 {
        tracing::warn!(
            clamped = clamped_changes,
            total = clamped.len(),
            "[bong][player] spawn_distribution 有 {clamped_changes} 个锚点在出生 zone AABB 之外，已钳制进界"
        );
    }
    clamped
}

pub fn fallback_spawn(seed: &str, purpose: SpawnPurpose) -> [f64; 3] {
    let snapshot = fallback_spawn_snapshot();
    PlayerSpawnSelector::with_distribution(&snapshot.registry, snapshot.distribution.clone())
        .select(seed, purpose)
}

pub fn emergency_spawn_position() -> [f64; 3] {
    EMERGENCY_SPAWN_POSITION
}

fn default_spawn_distribution_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(crate::world::zone::DEFAULT_ZONES_PATH)
}

#[cfg(test)]
fn load_default_spawn_distribution() -> Result<Vec<SpawnDistributionAnchor>, String> {
    load_spawn_distribution_from_path(default_spawn_distribution_path())
}

pub(crate) fn effective_default_spawn_distribution(
    registry: &ZoneRegistry,
) -> Vec<SpawnDistributionAnchor> {
    effective_spawn_distribution_from_path(registry, default_spawn_distribution_path())
}

fn effective_spawn_distribution_from_path(
    registry: &ZoneRegistry,
    path: impl AsRef<Path>,
) -> Vec<SpawnDistributionAnchor> {
    match load_spawn_distribution_from_path(path) {
        Ok(distribution) if !distribution.is_empty() => distribution,
        Ok(_) => {
            tracing::warn!(
                "[bong][player] spawn_distribution in zones.json is empty; using patrol anchor fallback"
            );
            distribution_from_zone_patrol_anchors(registry)
        }
        Err(error) => {
            tracing::warn!(
                "[bong][player] failed to load spawn_distribution from zones.json: {error}; using patrol anchor fallback"
            );
            distribution_from_zone_patrol_anchors(registry)
        }
    }
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

fn emergency_spawn_distribution() -> Vec<SpawnDistributionAnchor> {
    let anchor = DVec3::from_array(EMERGENCY_SPAWN_POSITION);
    vec![SpawnDistributionAnchor {
        anchor,
        radius: 0.0,
        weight: 1,
        safe_y: anchor.y,
    }]
}

fn distribution_from_zone_patrol_anchors(registry: &ZoneRegistry) -> Vec<SpawnDistributionAnchor> {
    let distribution: Vec<SpawnDistributionAnchor> = registry
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
        .unwrap_or_default();
    if distribution.is_empty() {
        emergency_spawn_distribution()
    } else {
        distribution
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::zone::{Zone, ZoneRegistry};

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

    #[test]
    fn effective_distribution_falls_back_to_patrol_anchors_on_load_error() {
        let registry = ZoneRegistry::fallback();
        let missing_path = std::env::temp_dir().join(format!(
            "bong-missing-spawn-distribution-{}-{}.json",
            std::process::id(),
            stable_hash("missing", SpawnPurpose::DevSpawnCommand),
        ));

        let distribution = effective_spawn_distribution_from_path(&registry, missing_path);
        let patrol_anchor = registry
            .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
            .and_then(|zone| zone.patrol_anchors.first())
            .expect("fallback spawn zone should declare a patrol anchor");

        assert_eq!(distribution.len(), 1);
        assert_eq!(distribution[0].anchor, *patrol_anchor);
        assert_eq!(distribution[0].radius, 64.0);
        assert_eq!(distribution[0].weight, 1);
        assert_eq!(distribution[0].safe_y, patrol_anchor.y + 2.0);
    }

    #[test]
    fn effective_distribution_falls_back_to_patrol_anchors_for_valid_empty_file() {
        let registry = ZoneRegistry::fallback();
        let path = std::env::temp_dir().join(format!(
            "bong-empty-spawn-distribution-{}-{}.json",
            std::process::id(),
            stable_hash("empty", SpawnPurpose::DevSpawnCommand),
        ));
        fs::write(
            &path,
            r#"{"zones":[{"name":"spawn","spawn_distribution":[]}]}"#,
        )
        .expect("empty spawn distribution fixture should be written");

        let distribution = effective_spawn_distribution_from_path(&registry, &path);
        fs::remove_file(path).expect("empty spawn distribution fixture should be removed");
        let patrol_anchor = registry
            .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
            .and_then(|zone| zone.patrol_anchors.first())
            .expect("fallback spawn zone should declare a patrol anchor");

        assert_eq!(distribution.len(), 1);
        assert_eq!(distribution[0].anchor, *patrol_anchor);
        assert_eq!(distribution[0].radius, 64.0);
    }

    fn synthetic_registry(bounds: (DVec3, DVec3), blocked_tiles: Vec<(i32, i32)>) -> ZoneRegistry {
        ZoneRegistry {
            zones: vec![Zone {
                name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
                dimension: crate::world::dimension::DimensionKind::Overworld,
                bounds,
                spirit_qi: 0.9,
                danger_level: 0,
                active_events: Vec::new(),
                patrol_anchors: Vec::new(),
                blocked_tiles,
                qi_equilibrium: 0.0,
                qi_inflow_per_min: 0.0,
            }],
        }
    }

    #[test]
    fn blocked_tile_fallback_returns_clamped_cluster_center() {
        let registry = synthetic_registry(
            (
                DVec3::new(-750.0, -64.0, -750.0),
                DVec3::new(750.0, 320.0, 750.0),
            ),
            vec![(750, 0)],
        );
        let selector = PlayerSpawnSelector::with_distribution(
            &registry,
            vec![SpawnDistributionAnchor {
                anchor: DVec3::new(10_000.0, 72.0, 0.0),
                radius: 0.0,
                weight: 1,
                safe_y: 72.0,
            }],
        );

        let pos = selector.select("blocked-edge", SpawnPurpose::InitialLogin);

        assert_eq!(
            pos,
            [750.0, 72.0, 0.0],
            "候选点 (10000,72,0) 钳制到 (750,72,0) 后撞上 blocked tile (750,0)，回退必须返回再钳制的簇中心"
        );
        assert!(
            registry.zones[0].contains(DVec3::new(pos[0], pos[1], pos[2])),
            "blocked-tile 回退必须落在出生 zone 内，绝不返回界外原始锚点"
        );
    }

    #[test]
    fn snapshot_clamps_out_of_bounds_anchors_into_spawn_zone() {
        let registry = synthetic_registry(
            (
                DVec3::new(-750.0, -64.0, -750.0),
                DVec3::new(750.0, 320.0, 750.0),
            ),
            Vec::new(),
        );
        let distribution = vec![
            SpawnDistributionAnchor {
                anchor: DVec3::new(10_000.0, 72.0, 0.0),
                radius: 0.0,
                weight: 1,
                safe_y: 72.0,
            },
            SpawnDistributionAnchor {
                anchor: DVec3::new(-100.0, 5.0, 200.0),
                radius: 30.0,
                weight: 1,
                safe_y: -999.0,
            },
        ];

        let clamped = clamp_distribution_to_spawn_zone(&registry, distribution);

        assert_eq!(clamped[0].anchor, DVec3::new(750.0, 72.0, 0.0));
        assert_eq!(clamped[0].safe_y, 72.0);
        assert_eq!(clamped[1].anchor, DVec3::new(-100.0, 5.0, 200.0));
        assert_eq!(clamped[1].safe_y, -64.0);
        assert_eq!(clamped[1].radius, 30.0);
        assert_eq!(clamped[1].weight, 1);
    }

    #[test]
    fn fallback_spawn_snapshot_is_shared_immutable_single_authority() {
        let first = fallback_spawn_snapshot();
        let second = fallback_spawn_snapshot();
        assert!(
            std::ptr::eq(first, second),
            "启动快照必须全局唯一（OnceLock），chunk 分配与出生选择才能读同一份权威数据"
        );
        let spawn_zone = first
            .registry
            .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
            .expect("spawn zone should load");
        assert!(!first.distribution.is_empty());
        for anchor in &first.distribution {
            assert!(
                spawn_zone.contains(anchor.anchor),
                "快照分布锚点必须全部落在出生 zone 内"
            );
            assert!(
                spawn_zone.contains(DVec3::new(anchor.anchor.x, anchor.safe_y, anchor.anchor.z)),
                "快照分布 safe_y 必须落在出生 zone 内"
            );
        }
    }

    #[test]
    fn empty_patrol_anchors_produce_nonempty_emergency_distribution() {
        let registry = ZoneRegistry::fallback();
        let mut empty = registry
            .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
            .expect("fallback spawn zone should exist")
            .clone();
        empty.patrol_anchors.clear();
        let registry = ZoneRegistry { zones: vec![empty] };

        let distribution = distribution_from_zone_patrol_anchors(&registry);

        assert_eq!(distribution.len(), 1);
        assert_eq!(distribution[0].anchor.to_array(), EMERGENCY_SPAWN_POSITION);
        assert_eq!(distribution[0].radius, 0.0);
        assert_eq!(distribution[0].weight, 1);
    }
}
