use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Deserialize;
use valence::prelude::DVec3;

use crate::world::zone::{Zone, ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};

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

        let blocked_at = |pos: DVec3| {
            zone.blocked_tiles
                .iter()
                .any(|(x, z)| *x == pos.x.floor() as i32 && *z == pos.z.floor() as i32)
        };

        let clamped = zone.clamp_position(candidate);
        if blocked_at(clamped) {
            let fallback = zone.clamp_position(DVec3::new(
                selected.anchor.x,
                selected.safe_y,
                selected.anchor.z,
            ));
            // review finding：钳制后的簇中心必须再过一遍同一 blocked_tiles 谓词 ——
            // clamp_position 只保证 AABB 内、不清除 blocked 状态，中心本身撞上 blocked
            // tile 时回退到已知有效 emergency 位置，绝不把玩家生到 zone 显式排除的坐标。
            if blocked_at(fallback) {
                // review finding：emergency 位置本身也可能被 blocked_tiles 显式排除 ——
                // 必须先过同一 clamp_position 与 blocked_at 谓词，否则会把玩家生到 zone
                // 禁止的坐标上。clamp_position 只保证 AABB 内、不清除 blocked 状态。
                let emergency = zone.clamp_position(DVec3::from_array(EMERGENCY_SPAWN_POSITION));
                if !blocked_at(emergency) {
                    tracing::warn!(
                        "[bong][player] blocked-tile fallback 的簇中心 ({}, {}) 本身也在 \
                         blocked_tiles 上；emergency tile ({}, {}) 空闲，回退到 emergency \
                         spawn（钳制后）",
                        fallback.x.floor() as i32,
                        fallback.z.floor() as i32,
                        emergency.x.floor() as i32,
                        emergency.z.floor() as i32,
                    );
                    return [emergency.x, emergency.y, emergency.z];
                }
                // emergency tile 也被排除：在 zone AABB 内螺旋扫描最近空闲 tile。
                tracing::warn!(
                    "[bong][player] 候选、簇中心与 emergency tile ({}, {}) 均在 \
                     blocked_tiles 上；扫描 zone 内最近空闲 tile 作为最后回退",
                    emergency.x.floor() as i32,
                    emergency.z.floor() as i32,
                );
                let start = (emergency.x.floor() as i32, emergency.z.floor() as i32);
                let (free_x, free_z) =
                    nearest_unblocked_tile(zone, start, &blocked_at).unwrap_or_else(|| {
                        panic!(
                            "[bong][player] spawn zone `{}` 的全部 tile 均被 blocked_tiles \
                             排除，无法生成出生点",
                            zone.name
                        )
                    });
                return [free_x as f64, fallback.y, free_z as f64];
            }
            return [fallback.x, fallback.y, fallback.z];
        }

        [clamped.x, clamped.y, clamped.z]
    }
}

/// 在 zone AABB 的 floor-tile 范围内从 `start` 螺旋向外扫描，返回最近未被
/// `blocked` 排除的 tile。圈半径超过 [`MAX_EMERGENCY_SCAN_RADIUS`] 仍无空闲 tile
/// 时返回 `None`，由调用方 fail-closed。扫描结果严格限制在 zone 内，绝不越界。
const MAX_EMERGENCY_SCAN_RADIUS: i32 = 128;

fn nearest_unblocked_tile(
    zone: &Zone,
    start: (i32, i32),
    blocked: &impl Fn(DVec3) -> bool,
) -> Option<(i32, i32)> {
    let (min, max) = zone.bounds;
    let (min_tx, max_tx) = (min.x.floor() as i32, max.x.floor() as i32);
    let (min_tz, max_tz) = (min.z.floor() as i32, max.z.floor() as i32);
    let in_zone_tile = |tx: i32, tz: i32| {
        tx >= min_tx && tx <= max_tx && tz >= min_tz && tz <= max_tz
    };
    for radius in 0..=MAX_EMERGENCY_SCAN_RADIUS {
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                if dx.abs() != radius && dz.abs() != radius {
                    continue;
                }
                let (tx, tz) = (start.0 + dx, start.1 + dz);
                if !in_zone_tile(tx, tz) {
                    continue;
                }
                let pos = DVec3::new(tx as f64 + 0.5, 0.0, tz as f64 + 0.5);
                if !blocked(pos) {
                    return Some((tx, tz));
                }
            }
        }
    }
    None
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
            spatial_revision: 0,
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
    fn blocked_tile_fallback_uses_emergency_when_cluster_center_also_blocked() {
        // review finding：候选点 (10000,72,0) 钳制到 (750,72,0) 后撞上 blocked tile
        // (750,0)，而簇中心钳制后仍在同一 (750,0) 上 —— 旧实现直接返回这块被 zone 显式
        // 排除的坐标。修复后必须回退到已知有效 emergency 位置，绝不把玩家生到 blocked
        // tile 上（clamp_position 只保证 AABB 内，不清除 blocked 状态）。
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
            EMERGENCY_SPAWN_POSITION,
            "候选与簇中心都在 blocked tile (750,0) 上时，回退必须落到 emergency spawn"
        );
        assert!(
            registry.zones[0].contains(DVec3::new(pos[0], pos[1], pos[2])),
            "blocked-tile 回退必须落在出生 zone 内，绝不返回界外原始锚点"
        );
        assert!(
            !registry.zones[0].blocked_tiles.contains(&(
                pos[0].floor() as i32,
                pos[2].floor() as i32
            )),
            "回退位置不得仍是 blocked tile"
        );
    }

    #[test]
    fn blocked_emergency_tile_scans_for_nearest_free_tile() {
        // review finding：emergency 位置 (8,150,8) 的 floor tile (8,8) 也在 blocked_tiles
        // 时，旧实现直接返回该坐标 —— 玩家被生到 zone 显式排除的 tile 上。修复后必须先
        // 钳制 + 过同一 blocked_at 谓词，再在 zone AABB 内扫描最近空闲 tile，绝不返回
        // blocked 坐标。本 zone 同时阻塞候选 (750,0) 与 emergency (8,8)，且 (8,8) 附近
        // 有空闲 tile，应命中螺旋扫描分支。
        let registry = synthetic_registry(
            (
                DVec3::new(-750.0, -64.0, -750.0),
                DVec3::new(750.0, 320.0, 750.0),
            ),
            vec![(750, 0), (8, 8)],
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
        let tile = (pos[0].floor() as i32, pos[2].floor() as i32);

        assert_ne!(
            tile,
            (8, 8),
            "不得返回被排除的 emergency tile 本身（pos={pos:?}）"
        );
        assert!(
            !registry.zones[0].blocked_tiles.contains(&tile),
            "候选、簇中心与 emergency 全 blocked 时，回退位置不得仍是 blocked tile（pos={pos:?}）"
        );
        assert!(
            registry.zones[0].contains(DVec3::new(pos[0], pos[1], pos[2])),
            "螺旋扫描回退必须落在出生 zone AABB 内（pos={pos:?}）"
        );
    }

    #[test]
    fn blocked_tile_fallback_never_returns_a_blocked_tile_across_seeds() {
        // review finding：blocked-tile 回退必须对 fallback 位置应用同一谓词 —— 扫一组
        // 确定性 seed，断言 select 永不返回被 zone 显式排除的坐标（无论走 clamp 直返、
        // 簇中心回退还是 emergency 回退）。radius>250 使候选常钳制到 AABB 边界，边界
        // blocked tile 大量触发 fallback 分支（中心 (0,0) 空闲 → 走簇中心回退子分支）。
        let registry = synthetic_registry(
            (
                DVec3::new(-200.0, -64.0, -200.0),
                DVec3::new(200.0, 320.0, 200.0),
            ),
            vec![(200, 0), (-200, 0), (0, 200), (0, -200)],
        );
        let selector = PlayerSpawnSelector::with_distribution(
            &registry,
            vec![SpawnDistributionAnchor {
                anchor: DVec3::new(0.0, 72.0, 0.0),
                radius: 250.0,
                weight: 1,
                safe_y: 72.0,
            }],
        );

        for i in 0..64 {
            let pos = selector.select(&format!("blocked-sweep-{i}"), SpawnPurpose::InitialLogin);
            let tile = (pos[0].floor() as i32, pos[2].floor() as i32);
            assert!(
                !registry.zones[0].blocked_tiles.contains(&tile),
                "blocked-sweep-{i} 返回了 blocked tile {tile:?}（pos={pos:?}）"
            );
            assert!(
                registry.zones[0].contains(DVec3::new(pos[0], pos[1], pos[2])),
                "blocked-sweep-{i} 必须落在出生 zone 内（pos={pos:?}）"
            );
        }
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
        let registry = ZoneRegistry {
            spatial_revision: 0,
            zones: vec![empty],
        };

        let distribution = distribution_from_zone_patrol_anchors(&registry);

        assert_eq!(distribution.len(), 1);
        assert_eq!(distribution[0].anchor.to_array(), EMERGENCY_SPAWN_POSITION);
        assert_eq!(distribution[0].radius, 0.0);
        assert_eq!(distribution[0].weight, 1);
    }
}
