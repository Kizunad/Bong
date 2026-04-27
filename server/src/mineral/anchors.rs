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
use crate::world::terrain::TerrainProviders;

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
    if providers.is_none() {
        tracing::info!(
            target: "bong::mineral",
            "skipping mineral anchor materialization: raster terrain provider is not loaded"
        );
        return;
    }

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
        for pos in positions_for_anchor(anchor) {
            if exhausted_positions.contains(&(anchor.mineral_id, pos))
                || index.lookup(pos).is_some()
            {
                continue;
            }
            let entity = commands
                .spawn(MineralOreNode::new(anchor.mineral_id, pos))
                .id();
            index.insert(pos, entity);
            spawned += 1;
        }
    }

    tracing::info!(
        target: "bong::mineral",
        "materialized {spawned} mineral ore nodes from {} anchor(s)",
        anchors.len()
    );
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

fn positions_for_anchor(anchor: &MineralAnchor) -> Vec<BlockPos> {
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
    candidates
        .into_iter()
        .take(anchor.max_units as usize)
        .map(|(_, pos)| pos)
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

        let positions = positions_for_anchor(&anchor);
        assert_eq!(positions.len(), 12);
        for pos in positions {
            let dx = pos.x - anchor.center.x;
            let dy = pos.y - anchor.center.y;
            let dz = pos.z - anchor.center.z;
            assert!(dx * dx + dy * dy + dz * dz <= anchor.radius * anchor.radius);
        }
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
        let exhausted_pos = positions_for_anchor(&anchor)[0];
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
        assert_eq!(index.lookup(exhausted_pos), None);
        let _ = fs::remove_file(path);
    }
}
