//! plan-mineral-v1 §M2 — worldgen 固定矿脉锚点 → runtime OreNode。

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use valence::prelude::{bevy_ecs, BlockPos, Commands, Res, ResMut, Resource};

use super::components::{MineralOreIndex, MineralOreNode};
use super::persistence::ExhaustedMineralsLog;
use super::registry::MineralRegistry;
use super::types::MineralId;
use crate::world::dimension::DimensionKind;
use crate::world::terrain::{FossilBbox, TerrainProvider, TerrainProviders};

const DEFAULT_ANCHORS_PATH: &str = "../worldgen/blueprint/mineral_anchors.json";
const MIN_WORLD_Y: i32 = -64;

#[derive(Debug, Clone, Resource)]
pub struct MineralAnchorConfig {
    pub path: PathBuf,
}

impl Default for MineralAnchorConfig {
    fn default() -> Self {
        Self {
            path: Path::new(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_ANCHORS_PATH),
        }
    }
}

impl MineralAnchorConfig {
    #[cfg(test)]
    fn with_path(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MineralAnchor {
    pub zone: String,
    pub mineral_id: MineralId,
    pub center: BlockPos,
    pub radius: i32,
    pub max_units: u32,
}

#[derive(Debug, Deserialize)]
struct RawAnchorFile {
    version: u32,
    #[serde(default)]
    anchors: Vec<RawAnchor>,
}

#[derive(Debug, Deserialize)]
struct RawAnchor {
    zone: String,
    mineral_id: String,
    position: [i32; 3],
    radius: i32,
    max_units: u32,
}

pub fn spawn_mineral_anchor_nodes(
    mut commands: Commands,
    config: Res<MineralAnchorConfig>,
    registry: Res<MineralRegistry>,
    exhausted: Res<ExhaustedMineralsLog>,
    mut index: ResMut<MineralOreIndex>,
    providers: Option<Res<TerrainProviders>>,
) {
    let Some(providers) = providers else {
        tracing::info!(
            target: "bong::mineral",
            "skipping mineral anchor materialization: raster terrain provider is not loaded"
        );
        return;
    };

    let anchors = match load_mineral_anchors(&config.path, &registry) {
        Ok(anchors) => anchors,
        Err(error) => {
            tracing::warn!(
                target: "bong::mineral",
                "failed to load mineral anchors from {}: {error}",
                config.path.display()
            );
            return;
        }
    };

    let exhausted_positions = exhausted
        .entries()
        .iter()
        .filter_map(|entry| {
            MineralId::from_str(&entry.mineral_id)
                .map(|id| (id, BlockPos::new(entry.x, entry.y, entry.z)))
        })
        .collect::<HashSet<_>>();

    let mut spawned = 0usize;
    for anchor in &anchors {
        for pos in positions_for_anchor(anchor, &providers.overworld) {
            if exhausted_positions.contains(&(anchor.mineral_id, pos))
                || index.lookup(DimensionKind::Overworld, pos).is_some()
            {
                continue;
            }
            let entity = commands
                .spawn(MineralOreNode::new(anchor.mineral_id, pos))
                .id();
            index.insert(DimensionKind::Overworld, pos, entity);
            spawned += 1;
        }
    }

    let fossil_spawned = spawn_fossil_mineral_nodes(
        &mut commands,
        &providers.overworld,
        &exhausted_positions,
        index.as_mut(),
    );
    spawned += fossil_spawned;

    tracing::info!(
        target: "bong::mineral",
        "materialized {spawned} mineral ore nodes from {} anchor(s) and {} whalefall fossil node(s)",
        anchors.len(),
        fossil_spawned
    );
}

fn spawn_fossil_mineral_nodes(
    commands: &mut Commands,
    terrain: &TerrainProvider,
    exhausted_positions: &HashSet<(MineralId, BlockPos)>,
    index: &mut MineralOreIndex,
) -> usize {
    let mut spawned = 0usize;
    for fossil in terrain.fossil_bboxes() {
        for (mineral_id, pos) in fossil_mineral_positions(fossil, terrain) {
            if exhausted_positions.contains(&(mineral_id, pos))
                || index.lookup(DimensionKind::Overworld, pos).is_some()
            {
                continue;
            }
            let entity = commands.spawn(MineralOreNode::new(mineral_id, pos)).id();
            index.insert(DimensionKind::Overworld, pos, entity);
            spawned += 1;
        }
    }
    spawned
}

fn fossil_mineral_positions(
    fossil: &FossilBbox,
    terrain: &TerrainProvider,
) -> Vec<(MineralId, BlockPos)> {
    let masks = (fossil.min_x..=fossil.max_x).step_by(4).flat_map(|x| {
        (fossil.min_z..=fossil.max_z)
            .step_by(4)
            .map(move |z| (x, z, terrain.sample_fossil_bbox(x, z)))
    });
    fossil_mineral_positions_from_masks(fossil, masks)
}

fn fossil_mineral_positions_from_masks(
    fossil: &FossilBbox,
    masks: impl IntoIterator<Item = (i32, i32, u8)>,
) -> Vec<(MineralId, BlockPos)> {
    let mut candidates = Vec::new();
    let max_units = if fossil.max_units == 0 {
        180
    } else {
        fossil.max_units
    } as usize;
    for (x, z, mask) in masks {
        if mask == 0 {
            continue;
        }
        let mineral_id = fossil_mineral_for_mask(mask, stable_fossil_hash(fossil, x, z));
        let y_offset = (stable_fossil_hash(fossil, z, x) % 9) as i32 - 4;
        let pos = BlockPos::new(x, fossil.center_y + y_offset, z);
        candidates.push((stable_pos_hash(pos, mineral_id), mineral_id, pos));
    }
    candidates.sort_by_key(|(hash, _, _)| *hash);
    candidates
        .into_iter()
        .take(max_units)
        .map(|(_, mineral_id, pos)| (mineral_id, pos))
        .collect()
}

fn fossil_mineral_for_mask(mask: u8, hash: u64) -> MineralId {
    if mask >= 2 {
        match hash % 10 {
            0 => MineralId::LingShiYi,
            1 | 2 => MineralId::LingShiShang,
            3 | 4 => MineralId::LingJing,
            _ => MineralId::SuiTie,
        }
    } else if hash % 3 == 0 {
        MineralId::LingJing
    } else {
        MineralId::YuSui
    }
}

fn stable_fossil_hash(fossil: &FossilBbox, x: i32, z: i32) -> u64 {
    let mut value = 0xcbf29ce484222325u64;
    for byte in fossil.name.as_bytes() {
        value = (value ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
    }
    value ^= (x as i64 as u64).wrapping_mul(0x9e3779b97f4a7c15);
    value ^= (z as i64 as u64).wrapping_mul(0xbf58476d1ce4e5b9);
    splitmix64(value)
}

pub fn load_mineral_anchors(
    path: impl AsRef<Path>,
    registry: &MineralRegistry,
) -> Result<Vec<MineralAnchor>, String> {
    let path = path.as_ref();
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("read {} failed: {error}", path.display()))?;
    let file: RawAnchorFile = serde_json::from_str(&raw)
        .map_err(|error| format!("parse {} failed: {error}", path.display()))?;
    if file.version != 1 {
        return Err(format!(
            "unsupported mineral anchor manifest version {}",
            file.version
        ));
    }

    file.anchors
        .into_iter()
        .enumerate()
        .map(|(index, raw)| parse_anchor(index, raw, registry))
        .collect()
}

fn parse_anchor(
    index: usize,
    raw: RawAnchor,
    registry: &MineralRegistry,
) -> Result<MineralAnchor, String> {
    let mineral_id = MineralId::from_str(&raw.mineral_id)
        .ok_or_else(|| format!("anchors[{index}] unknown mineral_id `{}`", raw.mineral_id))?;
    if registry.get(mineral_id).is_none() {
        return Err(format!(
            "anchors[{index}] mineral_id `{mineral_id}` is not registered"
        ));
    }
    if raw.radius <= 0 {
        return Err(format!("anchors[{index}] radius must be positive"));
    }
    if raw.max_units == 0 {
        return Err(format!("anchors[{index}] max_units must be positive"));
    }

    Ok(MineralAnchor {
        zone: raw.zone,
        mineral_id,
        center: BlockPos::new(raw.position[0], raw.position[1], raw.position[2]),
        radius: raw.radius,
        max_units: raw.max_units,
    })
}

fn positions_for_anchor(anchor: &MineralAnchor, terrain: &TerrainProvider) -> Vec<BlockPos> {
    let radius = anchor.radius;
    let radius_sq = radius * radius;
    let mut candidates = Vec::new();

    for dx in -radius..=radius {
        for dy in -radius..=radius {
            let y = anchor.center.y + dy;
            if y < MIN_WORLD_Y {
                continue;
            }
            for dz in -radius..=radius {
                let distance_sq = dx * dx + dy * dy + dz * dz;
                if distance_sq > radius_sq {
                    continue;
                }
                let pos = BlockPos::new(anchor.center.x + dx, y, anchor.center.z + dz);
                candidates.push((stable_pos_hash(pos, anchor.mineral_id), pos));
            }
        }
    }

    candidates.sort_by_key(|(hash, _)| *hash);
    let mut seen = HashSet::new();
    // snap → dedup → take(max_units)：浅 anchor 多个上半 candidate 会塌到同
    // 一格，先 dedup 保证 max_units 真的拿到 N 个独立位置，再裁剪。否则
    // .take() 在 dedup 前会让重复条目吃掉配额，最终少于 max_units。
    candidates
        .into_iter()
        .filter_map(|(_, pos)| {
            // 把矿石压到地表或地下 —— 防止 anchor 球体上半部漂浮在 air 里。
            // 用每列 height 作为 surface_y；矿石 y = min(原 y, surface_y)，
            // 这样深矿脉保持地下分布，浅 anchor 自然贴地形成"露头"。
            let surface_y = terrain.sample(pos.x, pos.z).height.round() as i32;
            let snapped_y = pos.y.min(surface_y);
            if snapped_y < MIN_WORLD_Y {
                return None;
            }
            Some(BlockPos::new(pos.x, snapped_y, pos.z))
        })
        .filter(|snapped| seen.insert((snapped.x, snapped.y, snapped.z)))
        .take(anchor.max_units as usize)
        .collect()
}

fn stable_pos_hash(pos: BlockPos, mineral_id: MineralId) -> u64 {
    let mut value = mineral_id
        .as_str()
        .bytes()
        .fold(0xcbf29ce484222325, |acc, b| {
            (acc ^ u64::from(b)).wrapping_mul(0x100000001b3)
        });
    value ^= (pos.x as i64 as u64).wrapping_mul(0x9e3779b97f4a7c15);
    value ^= (pos.y as i64 as u64).wrapping_mul(0xbf58476d1ce4e5b9);
    value ^= (pos.z as i64 as u64).wrapping_mul(0x94d049bb133111eb);
    splitmix64(value)
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e3779b97f4a7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use super::super::persistence::ExhaustedEntry;
    use super::super::registry::build_default_registry;
    use super::*;
    use std::env;
    use valence::prelude::{App, Startup};

    fn unique_tmp_path(name: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        env::temp_dir().join(format!("bong-mineral-anchor-{stamp}-{name}.json"))
    }

    #[test]
    fn load_manifest_parses_registered_anchor() {
        let path = unique_tmp_path("valid");
        fs::write(
            &path,
            r#"{"version":1,"anchors":[{"zone":"spawn","mineral_id":"fan_tie","position":[1,64,2],"radius":3,"max_units":5}]}"#,
        )
        .unwrap();

        let anchors = load_mineral_anchors(&path, &build_default_registry()).unwrap();
        assert_eq!(anchors.len(), 1);
        assert_eq!(anchors[0].mineral_id, MineralId::FanTie);
        assert_eq!(anchors[0].center, BlockPos::new(1, 64, 2));
        assert_eq!(anchors[0].max_units, 5);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn positions_are_limited_to_max_units_and_radius() {
        let anchor = MineralAnchor {
            zone: "spawn".into(),
            mineral_id: MineralId::FanTie,
            center: BlockPos::new(0, 64, 0),
            radius: 4,
            max_units: 12,
        };

        let terrain = TerrainProvider::empty_for_tests();
        let positions = positions_for_anchor(&anchor, &terrain);
        assert_eq!(positions.len(), 12);
        for pos in positions {
            let dx = pos.x - anchor.center.x;
            let dy = pos.y - anchor.center.y;
            let dz = pos.z - anchor.center.z;
            assert!(dx * dx + dy * dy + dz * dz <= anchor.radius * anchor.radius);
        }
    }

    #[test]
    fn shallow_anchor_dedups_before_max_units_cut() {
        // Regression: 浅 anchor 上半 candidate snap 到同一 surface_y 会塌成
        // 重复 (x, y, z)。dedup 必须在 take(max_units) 之前，否则重复条目
        // 吃掉配额，最终少于 max_units。本测试触发 bug 的关键：anchor
        // center.y 远高于 wilderness 高度（~111），radius 大、max_units 大，
        // 整个上半球都会 snap 到同一 y，多个 dy 落入同一 (x, z) → 重复。
        let anchor = MineralAnchor {
            zone: "test".into(),
            mineral_id: MineralId::FanTie,
            center: BlockPos::new(0, 200, 0),
            radius: 8,
            max_units: 30,
        };
        let terrain = TerrainProvider::empty_for_tests();
        let positions = positions_for_anchor(&anchor, &terrain);

        assert_eq!(
            positions.len(),
            30,
            "max_units 必须真的拿到 30 个（修复前 dedup 在 take 后会少于 30）"
        );
        let mut seen = HashSet::new();
        for pos in &positions {
            assert!(
                seen.insert((pos.x, pos.y, pos.z)),
                "返回的 positions 不应有重复 (x,y,z)"
            );
        }
    }

    #[test]
    fn fossil_core_and_outer_masks_use_expected_mineral_sets() {
        for hash in 0..30 {
            assert!(matches!(
                fossil_mineral_for_mask(2, hash),
                MineralId::SuiTie
                    | MineralId::LingJing
                    | MineralId::LingShiShang
                    | MineralId::LingShiYi
            ));
            assert!(matches!(
                fossil_mineral_for_mask(1, hash),
                MineralId::YuSui | MineralId::LingJing
            ));
        }
    }

    #[test]
    fn fossil_candidates_are_deterministically_limited() {
        let fossil = FossilBbox {
            zone: "north_wastes".into(),
            name: "鲸坠骸骨".into(),
            center_xz: [0, 0],
            center_y: 76,
            min_x: -16,
            max_x: 16,
            min_z: -16,
            max_z: 16,
            max_units: 7,
        };
        let points = (-16..=16)
            .step_by(4)
            .flat_map(|x| (-16..=16).step_by(4).map(move |z| (x, z, 2)))
            .collect::<Vec<_>>();

        let first = fossil_mineral_positions_from_masks(&fossil, points.iter().copied());
        let second = fossil_mineral_positions_from_masks(&fossil, points.iter().copied());

        assert_eq!(first, second);
        assert_eq!(first.len(), 7);
        assert!(first.iter().all(|(_, pos)| pos.y >= 72 && pos.y <= 80));
    }

    #[test]
    fn startup_spawns_index_entries_and_skips_exhausted_positions() {
        let path = unique_tmp_path("startup");
        fs::write(
            &path,
            r#"{"version":1,"anchors":[{"zone":"spawn","mineral_id":"fan_tie","position":[0,64,0],"radius":1,"max_units":7}]}"#,
        )
        .unwrap();

        let anchor = MineralAnchor {
            zone: "spawn".into(),
            mineral_id: MineralId::FanTie,
            center: BlockPos::new(0, 64, 0),
            radius: 1,
            max_units: 7,
        };
        let terrain = TerrainProvider::empty_for_tests();
        let exhausted_pos = positions_for_anchor(&anchor, &terrain)[0];
        let mut exhausted = ExhaustedMineralsLog::default();
        exhausted.record(ExhaustedEntry {
            mineral_id: "fan_tie".into(),
            x: exhausted_pos.x,
            y: exhausted_pos.y,
            z: exhausted_pos.z,
            tick: 1,
        });

        let mut app = App::new();
        app.insert_resource(MineralAnchorConfig::with_path(&path));
        app.insert_resource(build_default_registry());
        app.insert_resource(exhausted);
        app.insert_resource(MineralOreIndex::default());
        app.insert_resource(TerrainProviders {
            overworld: crate::world::terrain::TerrainProvider::empty_for_tests(),
            tsy: None,
        });
        app.add_systems(Startup, spawn_mineral_anchor_nodes);

        app.update();

        let index = app.world().resource::<MineralOreIndex>();
        assert_eq!(index.len(), 6);
        assert_eq!(index.lookup(DimensionKind::Overworld, exhausted_pos), None);
        let _ = fs::remove_file(path);
    }
}
