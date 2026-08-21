use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Deserialize;
use valence::prelude::{ChunkPos, ChunkView, DVec3};

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
    distribution: Cow<'a, [SpawnDistributionAnchor]>,
    materialized_chunks: Cow<'a, BTreeSet<ChunkPos>>,
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
        let distribution = distribution_from_zone_patrol_anchors(registry);
        Self {
            registry: Some(registry),
            materialized_chunks: Cow::Owned(crate::world::materialized_fallback_chunks(
                registry,
                &distribution,
            )),
            distribution: Cow::Owned(distribution),
        }
    }

    #[cfg(test)]
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
            materialized_chunks: Cow::Owned(crate::world::materialized_fallback_chunks(
                registry,
                &distribution,
            )),
            distribution: Cow::Owned(distribution),
        }
    }

    #[cfg(test)]
    pub fn fallback() -> Self {
        Self {
            registry: None,
            distribution: Cow::Owned(Vec::new()),
            materialized_chunks: Cow::Owned(BTreeSet::new()),
        }
    }

    fn from_snapshot(snapshot: &'a FallbackSpawnSnapshot) -> Self {
        Self {
            registry: Some(&snapshot.registry),
            distribution: Cow::Borrowed(&snapshot.distribution),
            materialized_chunks: Cow::Borrowed(&snapshot.materialized_chunks),
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

        // review finding：blocked_at 对 zone.blocked_tiles 线性扫描，螺旋回退每次访问
        // 都是 O(blocked_count)——最坏 zone（~65k blocked tile）下每次 spawn 选择退化
        // 到 O(zone tile 数 × blocked tile 数)。先建成 HashSet 索引，每次访问 O(1)。
        let blocked_tile_index: HashSet<(i32, i32)> = zone.blocked_tiles.iter().copied().collect();
        let blocked_at = |pos: DVec3| {
            let tx = pos.x.floor() as i32;
            let tz = pos.z.floor() as i32;
            blocked_tile_index.contains(&(tx, tz))
        };
        // fallback bootstrap 只物化出生分布的本地视域 union，并额外覆盖固定
        // emergency view。一个结果 chunk 只有在自己的完整运行时视域也被 producer
        // 物化时才安全；扫描会反复命中同一 chunk，因此缓存这个 expensive decision，
        // 避免每个 tile 都重新做约 2,025 次 BTreeSet 查询。
        let full_view_cache = RefCell::new(BTreeMap::<ChunkPos, bool>::new());
        let fully_materialized_at = |chunk: ChunkPos| {
            let cached = full_view_cache.borrow().get(&chunk).copied();
            if let Some(covered) = cached {
                return covered;
            }
            let covered = ChunkView::new(chunk, crate::world::FALLBACK_VIEW_DISTANCE_CHUNKS)
                .iter()
                .all(|view_chunk| self.materialized_chunks.contains(&view_chunk));
            full_view_cache.borrow_mut().insert(chunk, covered);
            covered
        };
        // 把 producer 返回的 exact chunk set 与 blocked tile 合并为同一个拒绝谓词，
        // 保证候选、簇中心、emergency 直返和螺旋扫描都不会把玩家送进未物化的
        // fallback 世界或其视域边缘之外。
        let unavailable_at =
            |pos: DVec3| blocked_at(pos) || !fully_materialized_at(ChunkPos::from(pos));

        let clamped = zone.clamp_position(candidate);
        if unavailable_at(clamped) {
            let fallback = zone.clamp_position(DVec3::new(
                selected.anchor.x,
                selected.safe_y,
                selected.anchor.z,
            ));
            // clamp_position 只保证 AABB 内；簇中心仍须同时满足 blocked_tiles 与
            // materialized union，避免配置中的分布簇与实际 eager chunk 集漂移。
            if unavailable_at(fallback) {
                // emergency 位置本身也可能被 blocked_tiles 排除，或不在本轮 fallback
                // union 内；只有通过同一拒绝谓词才允许直接返回。
                let emergency = zone.clamp_position(DVec3::from_array(EMERGENCY_SPAWN_POSITION));
                if !unavailable_at(emergency) {
                    tracing::warn!(
                        "[bong][player] blocked/未物化的簇中心 ({}, {}) 回退到可用 emergency \
                         tile ({}, {})（钳制后）",
                        fallback.x.floor() as i32,
                        fallback.z.floor() as i32,
                        emergency.x.floor() as i32,
                        emergency.z.floor() as i32,
                    );
                    return [emergency.x, emergency.y, emergency.z];
                }
                // emergency tile 也不可用：在 zone AABB 内扫描最近的、同时属于已物化
                // fallback union 且未被 blocked_tiles 排除的 tile。
                tracing::warn!(
                    "[bong][player] 候选、簇中心与 emergency tile ({}, {}) 均不可用；\
                     扫描已物化 fallback union 内最近空闲 tile 作为最后回退",
                    emergency.x.floor() as i32,
                    emergency.z.floor() as i32,
                );
                let start = (emergency.x.floor() as i32, emergency.z.floor() as i32);
                let (free_x, free_z) = nearest_unblocked_tile(
                    zone,
                    start,
                    &unavailable_at,
                    EMERGENCY_SCAN_WORK_BUDGET,
                )
                .unwrap_or_else(|| {
                    panic!(
                        "[bong][player] spawn zone `{}` 在扫描预算内没有可用空闲 \
                         tile （blocked、未物化或预算耗尽），无法生成出生点",
                        zone.name
                    )
                });
                return [free_x, fallback.y, free_z];
            }
            return [fallback.x, fallback.y, fallback.z];
        }

        [clamped.x, clamped.y, clamped.z]
    }
}

/// 在 zone AABB 的 floor-tile 范围内从 `start` 螺旋向外扫描，返回最近未被
/// `blocked` 排除的 tile 内实际坐标。通常返回 tile center；当边缘 tile center 落在
/// 分数 AABB 外时钳制到 AABB 边界，绝不返回可能越界的整数 tile 原点。扫描半径从
/// zone AABB 推导（`start` 到最远角落的 Chebyshev 距离），保证覆盖 zone 内全部候选
/// tile —— 固定半径会把「AABB 更大、空闲 tile 在固定圈之外」的 zone 误判成全 blocked
/// 而 panic（review finding）。zone 内全部 tile 均被 `blocked` 排除时返回 `None`，由
/// 调用方 fail-closed。
///
/// `work_budget` 是本次扫描允许的 tile 谓词求值次数上限（review finding：大 zone +
/// 稀疏空闲 tile 时扫描工作量随 AABB 面积增长，无界扫描会同步卡住每次受影响的登录）。
/// 预算耗尽且尚未找到空闲 tile 时同样返回 `None`（fail-closed，调用方 panic），绝不
/// 无界扫完整个 zone。
const EMERGENCY_SCAN_WORK_BUDGET: usize = 1 << 18; // 262,144 次谓词求值

fn nearest_unblocked_tile(
    zone: &Zone,
    start: (i32, i32),
    blocked: &impl Fn(DVec3) -> bool,
    work_budget: usize,
) -> Option<(f64, f64)> {
    let (min, max) = zone.bounds;
    let (min_tx, max_tx) = (min.x.floor() as i32, max.x.floor() as i32);
    let (min_tz, max_tz) = (min.z.floor() as i32, max.z.floor() as i32);
    let start_x = i64::from(start.0);
    let start_z = i64::from(start.1);
    let min_tx_i = i64::from(min_tx);
    let max_tx_i = i64::from(max_tx);
    let min_tz_i = i64::from(min_tz);
    let max_tz_i = i64::from(max_tz);
    // `start` 来自 clamp_position 后的 emergency tile，必在 zone 内。最远圈半径 =
    // start 到 zone AABB 四角的 Chebyshev 距离最大值，扫到该圈即扫完整个 zone。
    // i64 计算防 i32 减法/加法溢出（zone 边界可跨 >2^31 格）。
    let max_radius = (start_x - min_tx_i)
        .abs()
        .max((start_x - max_tx_i).abs())
        .max((start_z - min_tz_i).abs())
        .max((start_z - max_tz_i).abs());
    let mut work_left = work_budget;
    // Cell：visit 闭包独占 work_left 的可变借用，环循环需要共享读取"预算是否已耗尽"
    // —— Cell 让闭包写、循环读互不冲突。
    let exhausted = std::cell::Cell::new(false);
    let mut visit = |tx: i64, tz: i64| -> Option<(f64, f64)> {
        if work_left == 0 {
            // 预算耗尽：不再求值谓词，fail-closed。置 exhausted 标志让外层环循环立即
            // 终止（review finding：只返回 None 不终止循环的话，CPU 迭代数仍随 zone
            // 面积无界——预算只约束谓词调用，约束不住环枚举本身）。
            exhausted.set(true);
            return None;
        }
        work_left -= 1;
        if tx < min_tx_i || tx > max_tx_i || tz < min_tz_i || tz > max_tz_i {
            return None;
        }
        let pos = zone.clamp_position(DVec3::new(tx as f64 + 0.5, min.y, tz as f64 + 0.5));
        if !blocked(pos) {
            return Some((pos.x, pos.z));
        }
        None
    };
    for radius in 0..=max_radius {
        if exhausted.get() {
            break;
        }
        // Chebyshev 环边界 = max(|dx|,|dz|) == radius。半径 0 的圆盘只有起点本身；
        // radius>0 由四条边组成。逐边枚举 O(8R) 个边界 tile，避免对 (2R+1)² 全盘
        // 逐个跳过（O(R³)，大 zone 的 emergency 回退会退化到秒级）。
        if radius == 0 {
            if let Some(found) = visit(start_x, start_z) {
                return Some(found);
            }
            continue;
        }
        for dx in -radius..=radius {
            if exhausted.get() {
                break;
            }
            if let Some(found) = visit(start_x + dx, start_z - radius) {
                return Some(found);
            }
            if let Some(found) = visit(start_x + dx, start_z + radius) {
                return Some(found);
            }
        }
        for dz in -radius..radius {
            if exhausted.get() {
                break;
            }
            if let Some(found) = visit(start_x - radius, start_z + dz) {
                return Some(found);
            }
            if let Some(found) = visit(start_x + radius, start_z + dz) {
                return Some(found);
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
    pub(crate) materialized_chunks: BTreeSet<ChunkPos>,
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
        let materialized_chunks =
            crate::world::materialized_fallback_chunks(&registry, &distribution);
        Self {
            registry,
            distribution,
            materialized_chunks,
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
    PlayerSpawnSelector::from_snapshot(snapshot).select(seed, purpose)
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

    #[test]
    fn effective_distribution_falls_back_to_patrol_anchors_for_malformed_file() {
        // review finding：effective_spawn_distribution_from_path 声称每个 load 错误都
        // 回退 patrol anchor，但既有测试只覆盖「文件缺失」（I/O 错误）与「合法空分布」
        // 两个分支——present 但解析失败的 malformed 文件走的是 generic Err 分支，返回
        // 空分布/panic 的错误实现照样绿。本测试写入坏 JSON，断言必须回退到 patrol
        // anchor。
        let registry = ZoneRegistry::fallback();
        let path = std::env::temp_dir().join(format!(
            "bong-malformed-spawn-distribution-{}-{}.json",
            std::process::id(),
            stable_hash("malformed", SpawnPurpose::DevSpawnCommand),
        ));
        fs::write(&path, r#"{ "zones": [ "not-an-object" ] }"#)
            .expect("malformed spawn distribution fixture should be written");

        let distribution = effective_spawn_distribution_from_path(&registry, &path);
        fs::remove_file(path).expect("malformed spawn distribution fixture should be removed");
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
    fn effective_distribution_falls_back_to_patrol_anchors_for_invalid_distribution_entry() {
        // review finding：与 malformed JSON 同属 generic Err 分支的还有「JSON 合法但
        // 校验拒绝的分布条目」（本用例 weight=0 触发 load 的合法性校验错误）——错误
        // 实现若只对 I/O 错误回退、对校验错误 panic/返回空，fallback-world 启动会崩。
        // 断言校验错误同样必须回退 patrol anchor。
        let registry = ZoneRegistry::fallback();
        let path = std::env::temp_dir().join(format!(
            "bong-invalid-entry-spawn-distribution-{}-{}.json",
            std::process::id(),
            stable_hash("invalid-entry", SpawnPurpose::DevSpawnCommand),
        ));
        fs::write(
            &path,
            r#"{"zones":[{"name":"spawn","spawn_distribution":[{"anchor":[0.0,70.0,0.0],"radius":0.0,"weight":0,"safe_y":72.0}]}]}"#,
        )
        .expect("invalid-entry spawn distribution fixture should be written");

        let distribution = effective_spawn_distribution_from_path(&registry, &path);
        fs::remove_file(path).expect("invalid-entry fixture should be removed");
        let patrol_anchor = registry
            .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
            .and_then(|zone| zone.patrol_anchors.first())
            .expect("fallback spawn zone should declare a patrol anchor");

        assert_eq!(distribution.len(), 1);
        assert_eq!(distribution[0].anchor, *patrol_anchor);
        assert_eq!(distribution[0].radius, 64.0);
        assert_eq!(distribution[0].weight, 1);
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
    fn remote_distribution_blocked_fallback_stays_in_exact_materialized_union() {
        // 交叉配置回归：emergency 位于 chunk (0,0)，额外 zero-radius 分布簇位于
        // 正西一块。两者的 producer 矩形 union 横向扩展到 -23..22；扫描从 (8,8)
        // 出发时会先遇到 +1 chunk 中「已 materialize 但 full view 不安全」的 tile，
        // 再到达 -1 chunk 中 full-view-safe 的 tile。只检查 center chunk membership
        // 的实现会错误地停在 +1 chunk，无法通过这个 fixture。
        let view_distance = crate::world::FALLBACK_VIEW_DISTANCE_CHUNKS;
        let emergency_chunk = ChunkPos::from(DVec3::from_array(EMERGENCY_SPAWN_POSITION));
        assert_eq!(emergency_chunk, ChunkPos::new(0, 0));
        let safe_chunk = ChunkPos::new(-1, 0);
        let unsafe_edge_chunk = ChunkPos::new(1, 0);
        let anchor = DVec3::new(-8.0, 72.0, 8.0);
        let anchor_tile = (anchor.x.floor() as i32, anchor.z.floor() as i32);
        let emergency_tile = (
            EMERGENCY_SPAWN_POSITION[0].floor() as i32,
            EMERGENCY_SPAWN_POSITION[2].floor() as i32,
        );
        let representative_unsafe_tile = (16, emergency_tile.1);
        let representative_safe_tile = (-1, emergency_tile.1);
        assert_eq!(
            ChunkPos::from(DVec3::new(
                representative_unsafe_tile.0 as f64 + 0.5,
                72.0,
                representative_unsafe_tile.1 as f64 + 0.5,
            )),
            unsafe_edge_chunk,
            "representative +1 tile must map to the intended unsafe adjacent chunk"
        );
        assert_eq!(
            ChunkPos::from(DVec3::new(
                representative_safe_tile.0 as f64 + 0.5,
                72.0,
                representative_safe_tile.1 as f64 + 0.5,
            )),
            safe_chunk,
            "representative -1 tile must map to the intended safe adjacent chunk"
        );
        assert_eq!(
            (representative_unsafe_tile.0 - emergency_tile.0)
                .abs()
                .max((representative_unsafe_tile.1 - emergency_tile.1).abs()),
            8,
            "representative +1 tile must be reached on ring 8"
        );
        assert_eq!(
            (representative_safe_tile.0 - emergency_tile.0)
                .abs()
                .max((representative_safe_tile.1 - emergency_tile.1).abs()),
            9,
            "representative -1 tile must be reached on ring 9"
        );

        // The complete emergency center chunk is blocked. The anchor/candidate tile is also
        // blocked, forcing the selector through the emergency scan. No large synthetic overlay
        // is needed: the spiral reaches the representative +1 tile on ring 8, then the -1
        // adjacent chunk on ring 9, far below EMERGENCY_SCAN_WORK_BUDGET.
        let mut blocked_tiles = Vec::with_capacity(257);
        for tile_x in 0..16 {
            for tile_z in 0..16 {
                blocked_tiles.push((tile_x, tile_z));
            }
        }
        blocked_tiles.push(anchor_tile);

        let registry = synthetic_registry(
            (
                DVec3::new(-20_000.0, -64.0, -20_000.0),
                DVec3::new(20_000.0, 320.0, 20_000.0),
            ),
            blocked_tiles,
        );
        let selector = PlayerSpawnSelector::with_distribution(
            &registry,
            vec![SpawnDistributionAnchor {
                anchor,
                radius: 0.0,
                weight: 1,
                safe_y: 72.0,
            }],
        );
        let expected_union =
            crate::world::materialized_fallback_chunks(&registry, &selector.distribution);
        let full_view_is_materialized = |chunk: ChunkPos| {
            ChunkView::new(chunk, view_distance)
                .iter()
                .all(|view_chunk| expected_union.contains(&view_chunk))
        };

        assert_eq!(
            selector.materialized_chunks.as_ref(),
            &expected_union,
            "selector 必须保存同一 producer 计算出的 exact fallback union"
        );
        assert!(
            expected_union.contains(&unsafe_edge_chunk),
            "+1 adjacent chunk must be part of the materialized producer union"
        );
        assert!(
            !full_view_is_materialized(unsafe_edge_chunk),
            "+1 adjacent chunk must be materialized but not full-view-safe"
        );
        assert!(
            full_view_is_materialized(safe_chunk),
            "-1 distribution chunk must have a fully materialized view"
        );

        let pos = selector.select("remote-union", SpawnPurpose::InitialLogin);
        let result = DVec3::from(pos);
        let result_tile = (result.x.floor() as i32, result.z.floor() as i32);
        let result_chunk = ChunkPos::from(result);
        assert_eq!(
            result_chunk, safe_chunk,
            "full-view-safe scan must skip +1 and reach the full-view-safe -1 chunk"
        );
        assert!(
            !registry.zones[0].blocked_tiles.contains(&result_tile),
            "selected scan result must be unblocked (pos={pos:?})"
        );
        assert!(
            full_view_is_materialized(result_chunk),
            "selected scan result must have its complete runtime view in the producer union"
        );
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
            pos, EMERGENCY_SPAWN_POSITION,
            "候选与簇中心都在 blocked tile (750,0) 上时，回退必须落到 emergency spawn"
        );
        assert!(
            registry.zones[0].contains(DVec3::new(pos[0], pos[1], pos[2])),
            "blocked-tile 回退必须落在出生 zone 内，绝不返回界外原始锚点"
        );
        assert!(
            !registry.zones[0]
                .blocked_tiles
                .contains(&(pos[0].floor() as i32, pos[2].floor() as i32)),
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
    fn emergency_scan_finds_free_tile_at_exact_zone_max_radius() {
        // review finding：emergency 扫描的最大扫描圈（zone AABB 推导的最远角 Chebyshev
        // 距离）是**闭区间**边界 —— 唯一空闲 tile 恰好落在最后一圈时必须命中；
        // `0..max_radius` 式 off-by-one 会漏掉它、误判成全 blocked 而 panic。本 zone
        // tile 范围 [-128,128]²、start=(0,0)，最远圈半径=128，唯一空闲 tile 恰在
        // (128,0)（右边缘最后一圈）。
        let registry = synthetic_registry(
            (
                DVec3::new(-128.0, -64.0, -128.0),
                DVec3::new(128.0, 320.0, 128.0),
            ),
            Vec::new(),
        );
        let free = (128, 0);
        let blocked = |pos: DVec3| {
            let tx = pos.x.floor() as i32;
            let tz = pos.z.floor() as i32;
            tx != free.0 || tz != free.1
        };
        let found = nearest_unblocked_tile(
            &registry.zones[0],
            (0, 0),
            &blocked,
            EMERGENCY_SCAN_WORK_BUDGET,
        );
        assert_eq!(
            found,
            Some((128.0, 0.5)),
            "唯一空闲 tile 恰在最大扫描圈（Chebyshev 半径 128）上时必须被闭区间扫描命中；边缘 tile center 应钳制到 zone max"
        );
    }

    #[test]
    fn emergency_scan_selects_the_nearest_free_tile_when_several_are_free() {
        // review finding：blocked_emergency_tile_scans_for_nearest_free_tile 只断言
        // 「非 blocked + 在 zone 内」，放过了「有多个空闲 tile 时返回远处角落」的错误
        // 实现。本测试提供两个不同 Chebyshev 距离的空闲 tile（(1,0) 距离 1、(40,40)
        // 距离 40），断言最近者胜出 —— 返回远处 tile 的实现必红。
        let registry = synthetic_registry(
            (
                DVec3::new(-64.0, -64.0, -64.0),
                DVec3::new(64.0, 320.0, 64.0),
            ),
            Vec::new(),
        );
        let near = (1, 0);
        let far = (40, 40);
        let blocked = |pos: DVec3| {
            let tx = pos.x.floor() as i32;
            let tz = pos.z.floor() as i32;
            (tx != near.0 || tz != near.1) && (tx != far.0 || tz != far.1)
        };
        let found = nearest_unblocked_tile(
            &registry.zones[0],
            (0, 0),
            &blocked,
            EMERGENCY_SCAN_WORK_BUDGET,
        );
        assert_eq!(
            found,
            Some((1.5, 0.5)),
            "多个空闲 tile 时最近者（Chebyshev 距离 1）的 tile center 必须胜出，不得返回远处 (40,40)"
        );
    }

    #[test]
    fn emergency_scan_fails_closed_when_work_budget_exhausted() {
        // review finding：大 zone + 稀疏空闲 tile 时扫描工作量随 AABB 面积增长、每次
        // 受影响的 spawn 选择都重扫一遍。修复后扫描受 EMERGENCY_SCAN_WORK_BUDGET 显式
        // 资源约束：唯一空闲 tile 在预算之外时必须 fail-closed 返回 None（调用方 panic），
        // 绝不无界扫完整个 zone。本 zone 唯一空闲 tile 在半径 128，预算只够约 15 圈。
        let registry = synthetic_registry(
            (
                DVec3::new(-128.0, -64.0, -128.0),
                DVec3::new(128.0, 320.0, 128.0),
            ),
            Vec::new(),
        );
        let free = (128, 0);
        let blocked = |pos: DVec3| {
            let tx = pos.x.floor() as i32;
            let tz = pos.z.floor() as i32;
            tx != free.0 || tz != free.1
        };
        let found = nearest_unblocked_tile(&registry.zones[0], (0, 0), &blocked, 1_000);
        assert_eq!(
            found, None,
            "空闲 tile 在扫描预算之外时必须 fail-closed 返回 None，不得无界扫完整个 zone"
        );
    }

    #[test]
    fn emergency_scan_budget_caps_predicate_evaluations_on_pathological_zone() {
        // review finding：预算耗尽测试只用半径 128 的 zone，暴露不了「预算耗尽后环循环
        // 仍在继续枚举」的病态工作量。本 zone 半径 16384（全 blocked、唯一空闲 tile 在
        // 预算之外），谓词求值次数必须被预算精确封顶——预算语义是「最多 work_budget 次
        // blocked 谓词求值」，与 zone 面积无关。本测试能瞬时完成本身就依赖环循环在预算
        // 耗尽后立即终止（否则要跑完 ~10 亿次枚举）。
        let registry = synthetic_registry(
            (
                DVec3::new(-16_384.0, -64.0, -16_384.0),
                DVec3::new(16_384.0, 320.0, 16_384.0),
            ),
            Vec::new(),
        );
        let evaluations = std::cell::Cell::new(0usize);
        let blocked = |_pos: DVec3| {
            evaluations.set(evaluations.get() + 1);
            true
        };
        let found = nearest_unblocked_tile(&registry.zones[0], (0, 0), &blocked, 1_000);
        assert_eq!(
            found, None,
            "病态 zone 下预算耗尽必须 fail-closed 返回 None"
        );
        assert_eq!(
            evaluations.get(),
            1_000,
            "谓词求值次数必须被预算精确封顶（=1000），随 zone 面积增长的实现在此必红"
        );
    }

    #[test]
    fn emergency_scan_with_zero_budget_performs_no_predicate_evaluations() {
        // review finding：work_budget=0 的边界没有 pin——「预算=0 仍求值一次」的
        // off-by-one 实现在粗粒度测试下不红。契约是谓词求值次数上限：预算 0 必须
        // 一次都不求值、直接 fail-closed 返回 None。
        let registry = synthetic_registry(
            (
                DVec3::new(-64.0, -64.0, -64.0),
                DVec3::new(64.0, 320.0, 64.0),
            ),
            Vec::new(),
        );
        let evaluations = std::cell::Cell::new(0usize);
        let blocked = |_pos: DVec3| {
            evaluations.set(evaluations.get() + 1);
            false
        };
        let found = nearest_unblocked_tile(&registry.zones[0], (0, 0), &blocked, 0);
        assert_eq!(found, None, "work_budget=0 必须直接 fail-closed 返回 None");
        assert_eq!(
            evaluations.get(),
            0,
            "work_budget=0 不得执行任何谓词求值（多求值一次的 off-by-one 实现在此必红）"
        );
    }

    #[test]
    fn emergency_scan_accepts_free_tile_on_the_final_permitted_evaluation() {
        // 预算边界必须钉死到真实螺旋枚举顺序，而不是只写一个看似位于第五圈的
        // 坐标：ring0 的 (0,0) 是第 1 次；ring1 依次访问 (-1,-1)、(-1,1)、
        // (0,-1)、(0,1)，所以 (0,1) 才是第 5 次 predicate evaluation。
        let registry = synthetic_registry(
            (
                DVec3::new(-64.0, -64.0, -64.0),
                DVec3::new(64.0, 320.0, 64.0),
            ),
            Vec::new(),
        );
        let free = (0, 1);
        let evaluations = std::cell::Cell::new(0usize);
        let blocked = |pos: DVec3| {
            evaluations.set(evaluations.get() + 1);
            let tile = (pos.x.floor() as i32, pos.z.floor() as i32);
            tile != free
        };
        let found = nearest_unblocked_tile(&registry.zones[0], (0, 0), &blocked, 5);
        assert_eq!(
            found,
            Some((0.5, 1.5)),
            "唯一空闲 tile 恰在第 5 次（=预算上限）求值时命中，必须返回其 tile center"
        );
        assert_eq!(
            evaluations.get(),
            5,
            "第五次 predicate evaluation 命中后不得继续扫描或提前停止（actual={})",
            evaluations.get()
        );
    }

    #[test]
    fn emergency_scan_finds_free_tile_beyond_old_fixed_cap() {
        // review finding：旧实现把扫描半径硬编码为 128，zone AABB 更大时圈外空闲
        // tile 永远够不到 → 返回 None → select 在仍有合法出生 tile 时 panic。修复后
        // 扫描半径由 AABB 推导：本 zone tile 范围 [-200,200]²、唯一空闲 tile 在
        // (200,0)（Chebyshev 半径 200 > 128）也必须命中。
        let registry = synthetic_registry(
            (
                DVec3::new(-200.0, -64.0, -200.0),
                DVec3::new(200.0, 320.0, 200.0),
            ),
            Vec::new(),
        );
        let free = (200, 0);
        let blocked = |pos: DVec3| {
            let tx = pos.x.floor() as i32;
            let tz = pos.z.floor() as i32;
            tx != free.0 || tz != free.1
        };
        let found = nearest_unblocked_tile(
            &registry.zones[0],
            (0, 0),
            &blocked,
            EMERGENCY_SCAN_WORK_BUDGET,
        );
        assert_eq!(
            found,
            Some((200.0, 0.5)),
            "zone AABB 内、但超出旧固定半径 128 的空闲 tile 必须被 AABB 推导半径命中并钳制到 zone max"
        );
    }

    #[test]
    fn emergency_scan_returns_in_bounds_point_for_fractional_min_tile() {
        let registry = synthetic_registry(
            (
                DVec3::new(8.75, -64.0, -3.25),
                DVec3::new(10.25, 320.0, -1.25),
            ),
            Vec::new(),
        );
        let zone = &registry.zones[0];
        let found = nearest_unblocked_tile(zone, (8, -4), &|_pos: DVec3| false, 1)
            .expect("fractional-min tile is free and must be found");

        assert_eq!(
            found,
            (8.75, -3.25),
            "边缘 tile center 落在分数下界外时必须钳制到 AABB min，不得返回界外 tile 原点"
        );
        assert!(
            zone.contains(DVec3::new(found.0, 72.0, found.1)),
            "扫描返回的实际坐标必须位于 fractional AABB 内（found={found:?}）"
        );
        assert_eq!(
            (found.0.floor() as i32, found.1.floor() as i32),
            (8, -4),
            "钳制后的实际坐标仍必须属于被判定为空闲的同一 tile"
        );
    }

    #[test]
    fn emergency_scan_returns_none_when_whole_zone_blocked() {
        // fail-closed：整个 zone 全部被 blocked 排除时扫描返回 None —— select 对其
        // panic 是正确语义（此时确实不存在任何合法出生 tile）。
        let registry = synthetic_registry(
            (
                DVec3::new(-128.0, -64.0, -128.0),
                DVec3::new(128.0, 320.0, 128.0),
            ),
            Vec::new(),
        );
        let found = nearest_unblocked_tile(
            &registry.zones[0],
            (0, 0),
            &|_pos: DVec3| true,
            EMERGENCY_SCAN_WORK_BUDGET,
        );
        assert_eq!(
            found, None,
            "zone 全部 tile 均被 blocked 排除时，扫描必须 fail-closed 返回 None"
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

    /// 测试侧独立重算某个 seed 的原始候选 tile（select 的 hash→半径→角度→候选
    /// 计算），用于证明「候选确实落在 blocked tile 上」——即 blocked-tile 分支真的
    /// 被到达，而不是测试矩阵偶然避开。与 select 内联逻辑逐行对应，两者漂移即红。
    fn raw_candidate_tile_for_test(seed: &str, anchor: DVec3, radius: f64) -> (i32, i32) {
        let hash = stable_hash(seed, SpawnPurpose::InitialLogin);
        let radius_bits = hash.rotate_left(17);
        let angle_bits = hash.rotate_left(41);
        let radius_fraction = (radius_bits & 0xffff) as f64 / 65_535.0;
        let angle_fraction = (angle_bits & 0xffff) as f64 / 65_535.0;
        let r = radius * radius_fraction.sqrt();
        let angle = angle_fraction * std::f64::consts::TAU;
        let candidate = DVec3::new(
            anchor.x + r * angle.cos(),
            anchor.y,
            anchor.z + r * angle.sin(),
        );
        (candidate.x.floor() as i32, candidate.z.floor() as i32)
    }

    #[test]
    fn blocked_candidate_with_free_cluster_center_returns_the_center() {
        // review finding：select 有三条 blocked-tile 转移——候选被 block→返回钳制簇
        // 中心、中心被 block→返回 emergency、emergency 被 block→螺旋扫描。既有确定性
        // 测试只覆盖后两条，「候选被 block 而簇中心空闲→返回簇中心」只靠抽样 seed
        // 偶然覆盖，返回 blocked 候选的错误实现照样绿。本测试把 zone 内除中心 (0,0)
        // 外**全部** tile 标为 blocked：任何 seed 的候选（锚点半径 8 的圆盘内）都必然
        // 落在 blocked tile 上、簇中心必然空闲 → 该分支对每个 seed 确定性到达。断言
        // 一律返回簇中心，并用独立 oracle 证明至少一个 seed 的候选真实落在 blocked
        // tile 上（分支确实被走到，不是空转断言）。
        let mut blocked: Vec<(i32, i32)> = Vec::new();
        for tx in -16..=16 {
            for tz in -16..=16 {
                if (tx, tz) != (0, 0) {
                    blocked.push((tx, tz));
                }
            }
        }
        let registry = synthetic_registry(
            (
                DVec3::new(-16.0, -64.0, -16.0),
                DVec3::new(16.0, 320.0, 16.0),
            ),
            blocked,
        );
        let anchor = DVec3::new(0.0, 72.0, 0.0);
        let selector = PlayerSpawnSelector::with_distribution(
            &registry,
            vec![SpawnDistributionAnchor {
                anchor,
                radius: 8.0,
                weight: 1,
                safe_y: 72.0,
            }],
        );

        let mut saw_blocked_candidate = false;
        for i in 0..64 {
            let seed = format!("center-free-{i}");
            if raw_candidate_tile_for_test(&seed, anchor, 8.0) != (0, 0) {
                saw_blocked_candidate = true;
            }
            let pos = selector.select(&seed, SpawnPurpose::InitialLogin);
            let tile = (pos[0].floor() as i32, pos[2].floor() as i32);
            assert_eq!(
                tile,
                (0, 0),
                "{seed}：候选被 blocked 时 select 必须返回空闲簇中心，不得返回 blocked 候选（pos={pos:?}）"
            );
            assert!(
                !registry.zones[0].blocked_tiles.contains(&tile),
                "{seed} 返回位置不得仍是 blocked tile"
            );
        }
        assert!(
            saw_blocked_candidate,
            "测试矩阵中必须至少有一个 seed 的候选真实落在 blocked tile 上——否则本测试 \
             没有走到「候选被 block、簇中心空闲」分支，断言无效"
        );
    }

    #[test]
    #[should_panic(expected = "在扫描预算内没有可用空闲 tile")]
    fn select_fails_closed_when_emergency_scan_finds_no_free_tile() {
        // review finding：既有测试只直接调 nearest_unblocked_tile 断言 None，没有任何
        // 测试把该结果经 PlayerSpawnSelector::select 传播到调用方——「捕获/替换 None
        // 直接返回 blocked emergency 坐标」的错误 caller 实现会全部通过 helper 层测试。
        // 本测试把 select 推进到 emergency 扫描返回 None 的真实路径（整个 zone 全
        // blocked：候选、钳制簇中心、钳制 emergency 全部被排除），断言 fail-closed panic。
        let mut blocked = Vec::new();
        for tx in -16..=16 {
            for tz in -16..=16 {
                blocked.push((tx, tz));
            }
        }
        let registry = synthetic_registry(
            (
                DVec3::new(-16.0, -64.0, -16.0),
                DVec3::new(16.0, 320.0, 16.0),
            ),
            blocked,
        );
        let selector = PlayerSpawnSelector::with_distribution(
            &registry,
            vec![SpawnDistributionAnchor {
                anchor: DVec3::new(0.0, 72.0, 0.0),
                radius: 0.0,
                weight: 1,
                safe_y: 72.0,
            }],
        );

        selector.select("fully-blocked", SpawnPurpose::InitialLogin);
    }

    #[test]
    fn snapshot_clamps_out_of_bounds_anchors_into_spawn_zone() {
        // review finding：旧测试只覆盖 +X 一侧与 safe_y 下界。只钳部分轴/单侧边界的
        // 残缺实现会漏检。本矩阵覆盖 x/y/z 三轴两侧 + safe_y 上下边界 + 界内正对照。
        let registry = synthetic_registry(
            (
                DVec3::new(-750.0, -64.0, -750.0),
                DVec3::new(750.0, 320.0, 750.0),
            ),
            Vec::new(),
        );
        // (输入 anchor, 输入 safe_y, 期望 anchor, 期望 safe_y)
        let cases: [([f64; 3], f64, [f64; 3], f64); 9] = [
            ([10_000.0, 72.0, 0.0], 72.0, [750.0, 72.0, 0.0], 72.0), // +X 越界
            ([-10_000.0, 72.0, 0.0], 72.0, [-750.0, 72.0, 0.0], 72.0), // -X 越界
            ([0.0, 72.0, 10_000.0], 72.0, [0.0, 72.0, 750.0], 72.0), // +Z 越界
            ([0.0, 72.0, -10_000.0], 72.0, [0.0, 72.0, -750.0], 72.0), // -Z 越界
            ([0.0, 1_000.0, 0.0], 72.0, [0.0, 320.0, 0.0], 72.0),    // +Y 越界
            ([0.0, -500.0, 0.0], 72.0, [0.0, -64.0, 0.0], 72.0),     // -Y 越界
            ([0.0, 72.0, 0.0], 1_000.0, [0.0, 72.0, 0.0], 320.0),    // safe_y 上界
            ([0.0, 72.0, 0.0], -1_000.0, [0.0, 72.0, 0.0], -64.0),   // safe_y 下界
            ([100.0, 72.0, 200.0], 72.0, [100.0, 72.0, 200.0], 72.0), // 界内 no-op
        ];
        let distribution: Vec<SpawnDistributionAnchor> = cases
            .iter()
            .map(|(anchor, safe_y, _, _)| SpawnDistributionAnchor {
                anchor: DVec3::new(anchor[0], anchor[1], anchor[2]),
                radius: 30.0,
                weight: 1,
                safe_y: *safe_y,
            })
            .collect();

        let clamped = clamp_distribution_to_spawn_zone(&registry, distribution);

        for (index, (_, _, expected_anchor, expected_safe_y)) in cases.iter().enumerate() {
            assert_eq!(
                clamped[index].anchor,
                DVec3::new(expected_anchor[0], expected_anchor[1], expected_anchor[2]),
                "case[{index}] anchor 必须钳制到出生 zone AABB 内"
            );
            assert_eq!(
                clamped[index].safe_y, *expected_safe_y,
                "case[{index}] safe_y 必须钳制到出生 zone Y 范围内"
            );
            assert_eq!(
                clamped[index].radius, 30.0,
                "case[{index}] radius 必须原样保留"
            );
            assert_eq!(
                clamped[index].weight, 1,
                "case[{index}] weight 必须原样保留"
            );
        }
    }

    #[test]
    fn fallback_spawn_snapshot_is_shared_immutable_single_authority() {
        let first = fallback_spawn_snapshot();
        let second = fallback_spawn_snapshot();
        assert!(
            std::ptr::eq(first, second),
            "启动快照必须全局唯一（OnceLock），chunk 分配与出生选择才能读同一份权威数据"
        );
        let selector = PlayerSpawnSelector::from_snapshot(first);
        match &selector.distribution {
            Cow::Borrowed(distribution) => assert!(
                std::ptr::eq(*distribution, first.distribution.as_slice()),
                "出生选择器必须直接借用快照分布，登录热路径不得复制整个 Vec"
            ),
            Cow::Owned(_) => panic!("出生选择器不得复制快照分布"),
        }
        match &selector.materialized_chunks {
            Cow::Borrowed(materialized_chunks) => assert!(
                std::ptr::eq(*materialized_chunks, &first.materialized_chunks),
                "出生选择器必须直接借用快照 chunk 集，登录热路径不得复制整个 BTreeSet"
            ),
            Cow::Owned(_) => panic!("出生选择器不得复制快照 chunk 集"),
        }
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
