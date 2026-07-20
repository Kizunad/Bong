//! plan-mineral-v1 §M2 — worldgen 固定矿脉锚点 → runtime OreNode。

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use valence::prelude::{bevy_ecs, BlockPos, Commands, DVec3, Res, ResMut, Resource};

use super::components::{MineralOreIndex, MineralOreNode};
use super::persistence::ExhaustedMineralsLog;
use super::registry::MineralRegistry;
use super::types::MineralId;
use crate::gathering::session::Gatherable;
use crate::gathering::tools::{base_time_ticks, GatheringTargetKind};
use crate::world::dimension::DimensionKind;
use crate::world::terrain::{FossilBbox, TerrainProvider, TerrainProviders};
use crate::world::zone::ZoneRegistry;

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
    zones: Res<ZoneRegistry>,
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

    let anchors = match load_mineral_anchors(&config.path, &registry, &zones) {
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

    let prepared_positions =
        match prepare_mineral_anchor_positions(&anchors, &providers.overworld, zones.as_ref()) {
            Ok(prepared) => prepared,
            Err(error) => {
                tracing::warn!(
                    target: "bong::mineral",
                    "failed to preflight mineral anchors from {}: {error}",
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
    for (anchor, positions) in anchors.iter().zip(prepared_positions) {
        for pos in positions {
            if exhausted_positions.contains(&(anchor.mineral_id, pos))
                || index.lookup(DimensionKind::Overworld, pos).is_some()
            {
                continue;
            }
            let entity = commands
                .spawn((
                    MineralOreNode::new(anchor.mineral_id, pos),
                    mineral_gatherable(anchor.mineral_id, registry.as_ref()),
                ))
                .id();
            index.insert(DimensionKind::Overworld, pos, entity);
            spawned += 1;
        }
    }

    let fossil_spawned = spawn_fossil_mineral_nodes(
        &mut commands,
        &providers.overworld,
        &exhausted_positions,
        registry.as_ref(),
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
    registry: &MineralRegistry,
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
            let entity = commands
                .spawn((
                    MineralOreNode::new(mineral_id, pos),
                    mineral_gatherable(mineral_id, registry),
                ))
                .id();
            index.insert(DimensionKind::Overworld, pos, entity);
            spawned += 1;
        }
    }
    spawned
}

/// plan-cultivation-pacing-v1 P1.9 — `pub(crate)` 而非 module-private：respawn.rs
/// 复用同一套 Gatherable 元数据构造逻辑，重生的 OreNode 必须和启动期物化的
/// OreNode 拥有完全一致的 gathering 元数据（不能各写一份漂移）。
pub(crate) fn mineral_gatherable(mineral_id: MineralId, registry: &MineralRegistry) -> Gatherable {
    let mineral_key = mineral_id.as_str();
    let display_name = registry
        .get(mineral_id)
        .map(|entry| entry.display_name_zh.to_string())
        .unwrap_or_else(|| format!("{mineral_id:?}"));
    Gatherable {
        target: GatheringTargetKind::Ore,
        base_time_ticks: base_time_ticks(GatheringTargetKind::Ore),
        loot_table: format!("mineral:{mineral_key}"),
        display_name,
    }
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
    } else if hash.is_multiple_of(3) {
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
    zones: &ZoneRegistry,
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

    let anchors = file
        .anchors
        .into_iter()
        .enumerate()
        .map(|(index, raw)| parse_anchor(index, raw, registry))
        .collect::<Result<Vec<_>, _>>()?;
    validate_anchor_zones(&anchors, zones)?;
    Ok(anchors)
}

#[derive(Clone, Copy)]
enum AnchorZoneValidationContext {
    Center,
    FinalCandidates,
}

fn validate_anchor_zone_contract(
    anchor_index: usize,
    anchor: &MineralAnchor,
    positions: &[BlockPos],
    zones: &ZoneRegistry,
    context: AnchorZoneValidationContext,
) -> Result<(), String> {
    let declared_zone = zones.find_zone_by_name(&anchor.zone).ok_or_else(|| match context {
        AnchorZoneValidationContext::Center => format!(
            "anchors[{anchor_index}] mineral `{}` declares unknown runtime zone `{}`",
            anchor.mineral_id.as_str(),
            anchor.zone
        ),
        AnchorZoneValidationContext::FinalCandidates => format!(
            "anchors[{anchor_index}] mineral `{}` declares unknown runtime zone `{}` during final-candidate preflight",
            anchor.mineral_id.as_str(),
            anchor.zone
        ),
    })?;

    if declared_zone.dimension != DimensionKind::Overworld {
        return Err(match context {
            AnchorZoneValidationContext::Center => format!(
                "anchors[{anchor_index}] mineral `{}` declares zone `{}` in dimension {:?}, but fixed mineral anchors materialize in Overworld",
                anchor.mineral_id.as_str(),
                anchor.zone,
                declared_zone.dimension
            ),
            AnchorZoneValidationContext::FinalCandidates => format!(
                "anchors[{anchor_index}] mineral `{}` declares zone `{}` in dimension {:?}, but fixed mineral anchors materialize in Overworld during final-candidate preflight",
                anchor.mineral_id.as_str(),
                anchor.zone,
                declared_zone.dimension
            ),
        });
    }

    for (position_index, pos) in positions.iter().copied().enumerate() {
        let point = DVec3::new(f64::from(pos.x), f64::from(pos.y), f64::from(pos.z));
        let describe_position = || match context {
            AnchorZoneValidationContext::Center => format!("center {pos:?}"),
            AnchorZoneValidationContext::FinalCandidates => format!(
                "final candidate[{position_index}] {pos:?} after surface snap/dedup/max_units"
            ),
        };

        if !declared_zone.contains(point) {
            return Err(format!(
                "anchors[{anchor_index}] mineral `{}` {} lies outside declared zone `{}` AABB {:?}",
                anchor.mineral_id.as_str(),
                describe_position(),
                anchor.zone,
                declared_zone.bounds
            ));
        }

        let actual_zone = zones
            .find_zone(DimensionKind::Overworld, point)
            .ok_or_else(|| {
                format!(
                    "anchors[{anchor_index}] mineral `{}` {} does not resolve to any Overworld runtime zone",
                    anchor.mineral_id.as_str(),
                    describe_position()
                )
            })?;
        if actual_zone.name != anchor.zone {
            let capture_target = match context {
                AnchorZoneValidationContext::Center => "anchor",
                AnchorZoneValidationContext::FinalCandidates => "ore node",
            };
            return Err(format!(
                "anchors[{anchor_index}] mineral `{}` {} resolves to runtime zone `{}`, not declared `{}`; a more specific or overlapping zone would capture this {capture_target}",
                anchor.mineral_id.as_str(),
                describe_position(),
                actual_zone.name,
                anchor.zone
            ));
        }
    }

    Ok(())
}

fn validate_anchor_zones(anchors: &[MineralAnchor], zones: &ZoneRegistry) -> Result<(), String> {
    for (index, anchor) in anchors.iter().enumerate() {
        validate_anchor_zone_contract(
            index,
            anchor,
            std::slice::from_ref(&anchor.center),
            zones,
            AnchorZoneValidationContext::Center,
        )?;
    }

    Ok(())
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
            // worldgen-v4 P0 §8.1 #1: 用每列 span 顶面作为 surface_y；矿石
            // y = min(原 y, surface_y)，深矿脉保持地下分布，浅 anchor 贴地"露头"。
            let surface_y = terrain.sample(pos.x, pos.z).surface_y();
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

fn prepare_mineral_anchor_positions(
    anchors: &[MineralAnchor],
    terrain: &TerrainProvider,
    zones: &ZoneRegistry,
) -> Result<Vec<Vec<BlockPos>>, String> {
    // Generate every anchor's stable, surface-snapped, deduplicated and truncated
    // candidate set before validating any of them. The caller does not mutate
    // Commands or MineralOreIndex until this whole batch returns Ok.
    let prepared = anchors
        .iter()
        .map(|anchor| positions_for_anchor(anchor, terrain))
        .collect::<Vec<_>>();

    validate_final_anchor_positions(anchors, &prepared, zones)?;
    Ok(prepared)
}

fn validate_final_anchor_positions(
    anchors: &[MineralAnchor],
    prepared: &[Vec<BlockPos>],
    zones: &ZoneRegistry,
) -> Result<(), String> {
    debug_assert_eq!(anchors.len(), prepared.len());

    for (anchor_index, (anchor, positions)) in anchors.iter().zip(prepared).enumerate() {
        validate_anchor_zone_contract(
            anchor_index,
            anchor,
            positions,
            zones,
            AnchorZoneValidationContext::FinalCandidates,
        )?;
    }

    Ok(())
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
    use crate::world::zone::{Zone, ZoneRegistryStartupSet};
    use std::any::TypeId;
    use std::env;
    use valence::prelude::bevy_ecs::schedule::{NodeId, ScheduleGraph};
    use valence::prelude::{App, DVec3, IntoSystemConfigs, Startup, SystemSet};

    fn unique_tmp_path(name: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        env::temp_dir().join(format!("bong-mineral-anchor-{stamp}-{name}.json"))
    }

    fn write_single_anchor_manifest(name: &str, zone: &str, position: [i32; 3]) -> PathBuf {
        write_anchor_manifest(name, &[(zone, "fan_tie", position, 3, 5)])
    }

    fn write_anchor_manifest(name: &str, anchors: &[(&str, &str, [i32; 3], i32, u32)]) -> PathBuf {
        let path = unique_tmp_path(name);
        let anchors_json = anchors
            .iter()
            .map(|(zone, mineral_id, position, radius, max_units)| {
                format!(
                    r#"{{"zone":"{zone}","mineral_id":"{mineral_id}","position":[{},{},{}],"radius":{radius},"max_units":{max_units}}}"#,
                    position[0], position[1], position[2]
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        fs::write(
            &path,
            format!(r#"{{"version":1,"anchors":[{anchors_json}]}}"#),
        )
        .unwrap();
        path
    }

    fn test_anchor(zone: &str, center: BlockPos, radius: i32, max_units: u32) -> MineralAnchor {
        MineralAnchor {
            zone: zone.into(),
            mineral_id: MineralId::FanTie,
            center,
            radius,
            max_units,
        }
    }

    fn test_zone(name: &str, dimension: DimensionKind, bounds: (DVec3, DVec3)) -> Zone {
        Zone {
            name: name.to_string(),
            dimension,
            bounds,
            spirit_qi: 0.0,
            danger_level: 1,
            active_events: Vec::new(),
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
            qi_equilibrium: 0.0,
            qi_inflow_per_min: 0.0,
        }
    }

    fn materializer_app(
        manifest_path: &Path,
        zones: ZoneRegistry,
        terrain: TerrainProvider,
    ) -> App {
        let mut app = App::new();
        app.insert_resource(MineralAnchorConfig::with_path(manifest_path.to_path_buf()));
        app.insert_resource(build_default_registry());
        app.insert_resource(zones);
        app.insert_resource(ExhaustedMineralsLog::default());
        app.insert_resource(MineralOreIndex::default());
        app.insert_resource(TerrainProviders {
            overworld: terrain,
            tsy: None,
        });
        app.add_systems(Startup, spawn_mineral_anchor_nodes);
        app
    }

    fn system_node(graph: &ScheduleGraph, expected_name: &str) -> (NodeId, TypeId) {
        graph
            .systems()
            .find_map(|(node, system, _)| {
                (system.name().as_ref() == expected_name).then(|| (node, system.type_id()))
            })
            .unwrap_or_else(|| panic!("Startup schedule must contain system `{expected_name}`"))
    }

    fn system_type_set_node(graph: &ScheduleGraph, system_type: TypeId) -> NodeId {
        graph
            .system_sets()
            .find_map(|(node, set, _)| (set.system_type() == Some(system_type)).then_some(node))
            .expect("Startup schedule must expose the producer system's automatic SystemTypeSet")
    }

    fn concrete_set_node<S>(graph: &ScheduleGraph, expected: S) -> NodeId
    where
        S: SystemSet,
    {
        graph
            .system_sets()
            .find_map(|(node, set, _)| set.as_dyn_eq().dyn_eq(expected.as_dyn_eq()).then_some(node))
            .expect("Startup schedule must contain the expected concrete system set")
    }

    fn assert_production_startup_dependencies(app: &App) {
        let schedule = app
            .get_schedule(Startup)
            .expect("production registration must create the Startup schedule");
        let graph = schedule.graph();
        let (setup_world, setup_world_type) = system_node(
            graph,
            std::any::type_name_of_val(&crate::world::setup_world),
        );
        let (materializer, _) = system_node(
            graph,
            std::any::type_name_of_val(&spawn_mineral_anchor_nodes),
        );
        let setup_world_set = system_type_set_node(graph, setup_world_type);
        let zone_registry_set = concrete_set_node(graph, ZoneRegistryStartupSet);

        assert!(
            graph
                .hierarchy()
                .graph()
                .contains_edge(setup_world_set, setup_world),
            "fixture must bind the real setup_world system to the exact SystemTypeSet targeted by .after(setup_world)"
        );
        assert!(
            graph
                .dependency()
                .graph()
                .contains_edge(setup_world_set, materializer),
            "production mineral Startup must run after world::setup_world so deferred TerrainProviders exist before the one-shot materializer"
        );
        assert!(
            graph
                .dependency()
                .graph()
                .contains_edge(zone_registry_set, materializer),
            "production mineral Startup must run after ZoneRegistryStartupSet so deferred ZoneRegistry exists before the one-shot materializer"
        );
    }

    #[test]
    fn load_manifest_parses_registered_anchor() {
        let path = unique_tmp_path("valid");
        fs::write(
            &path,
            r#"{"version":1,"anchors":[{"zone":"spawn","mineral_id":"fan_tie","position":[1,64,2],"radius":3,"max_units":5}]}"#,
        )
        .unwrap();

        let anchors =
            load_mineral_anchors(&path, &build_default_registry(), &ZoneRegistry::fallback())
                .unwrap();
        assert_eq!(anchors.len(), 1);
        assert_eq!(anchors[0].mineral_id, MineralId::FanTie);
        assert_eq!(anchors[0].center, BlockPos::new(1, 64, 2));
        assert_eq!(anchors[0].max_units, 5);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn load_manifest_rejects_unknown_runtime_zone() {
        let path = write_single_anchor_manifest("unknown-zone", "missing_zone", [0, 64, 0]);
        let error =
            load_mineral_anchors(&path, &build_default_registry(), &ZoneRegistry::fallback())
                .unwrap_err();
        let _ = fs::remove_file(path);

        assert!(
            error.contains("declares unknown runtime zone `missing_zone`"),
            "unknown zone error must name the rejected zone so startup logs contain a repair clue; actual: {error}"
        );
    }

    #[test]
    fn load_manifest_rejects_center_outside_declared_zone_aabb() {
        let path = write_single_anchor_manifest("outside-zone", "spawn", [1000, 64, 1000]);
        let error =
            load_mineral_anchors(&path, &build_default_registry(), &ZoneRegistry::fallback())
                .unwrap_err();
        let _ = fs::remove_file(path);

        assert!(
            error.contains("lies outside declared zone `spawn` AABB"),
            "AABB rejection must name the declared zone and boundary failure; actual: {error}"
        );
    }

    #[test]
    fn load_manifest_rejects_more_specific_runtime_zone_capture() {
        let path = write_single_anchor_manifest("nested-zone", "outer", [50, 50, 50]);
        let zones = ZoneRegistry {
            zones: vec![
                test_zone(
                    "outer",
                    DimensionKind::Overworld,
                    (DVec3::ZERO, DVec3::splat(100.0)),
                ),
                test_zone(
                    "inner",
                    DimensionKind::Overworld,
                    (DVec3::splat(40.0), DVec3::splat(60.0)),
                ),
            ],
        };
        let error = load_mineral_anchors(&path, &build_default_registry(), &zones).unwrap_err();
        let _ = fs::remove_file(path);

        assert!(
            error.contains("resolves to runtime zone `inner`, not declared `outer`"),
            "runtime resolution must pin ZoneRegistry::find_zone smallest-AABB semantics; actual: {error}"
        );
    }

    #[test]
    fn load_manifest_rejects_non_overworld_declared_zone() {
        let path = write_single_anchor_manifest("tsy-zone", "tsy_test", [0, 64, 0]);
        let zones = ZoneRegistry {
            zones: vec![test_zone(
                "tsy_test",
                DimensionKind::Tsy,
                (DVec3::new(-10.0, 0.0, -10.0), DVec3::new(10.0, 100.0, 10.0)),
            )],
        };
        let error = load_mineral_anchors(&path, &build_default_registry(), &zones).unwrap_err();
        let _ = fs::remove_file(path);

        assert!(
            error.contains("dimension Tsy") && error.contains("materialize in Overworld"),
            "fixed anchors must fail before an Overworld materializer consumes a TSY zone; actual: {error}"
        );
    }

    #[test]
    fn final_candidates_accept_radius_exactly_on_declared_aabb_boundary() {
        let anchor = test_anchor("boundary", BlockPos::new(1, 64, 1), 1, 7);
        let zones = ZoneRegistry {
            zones: vec![test_zone(
                "boundary",
                DimensionKind::Overworld,
                (DVec3::new(0.0, 0.0, 0.0), DVec3::new(2.0, 320.0, 2.0)),
            )],
        };

        let prepared = prepare_mineral_anchor_positions(
            std::slice::from_ref(&anchor),
            &TerrainProvider::empty_for_tests(),
            &zones,
        )
        .expect("inclusive AABB boundary must accept the radius-one sphere");

        assert_eq!(prepared.len(), 1);
        assert_eq!(
            prepared[0].len(),
            7,
            "radius-one anchor with max_units=7 must retain every discrete sphere point"
        );
        assert!(
            prepared[0]
                .iter()
                .any(|pos| pos.x == 0 || pos.x == 2 || pos.z == 0 || pos.z == 2),
            "fixture must exercise at least one candidate exactly on an inclusive AABB face"
        );
    }

    #[test]
    fn final_candidates_reject_radius_one_block_outside_declared_aabb() {
        let anchor = test_anchor("boundary", BlockPos::new(1, 64, 1), 1, 7);
        let zones = ZoneRegistry {
            zones: vec![test_zone(
                "boundary",
                DimensionKind::Overworld,
                (DVec3::new(1.0, 0.0, 0.0), DVec3::new(2.0, 320.0, 2.0)),
            )],
        };

        let error = prepare_mineral_anchor_positions(
            std::slice::from_ref(&anchor),
            &TerrainProvider::empty_for_tests(),
            &zones,
        )
        .unwrap_err();

        assert!(
            error.contains("final candidate")
                && error.contains("lies outside declared zone `boundary` AABB"),
            "center remains legal, so the one-block radius overflow must be rejected only after final candidate generation; actual: {error}"
        );
    }

    #[test]
    fn final_candidates_reject_surface_snap_below_declared_aabb() {
        let terrain = TerrainProvider::empty_for_tests();
        let surface_y = terrain.sample(0, 0).surface_y();
        let anchor_y = surface_y + 10;
        let anchor = test_anchor("high_zone", BlockPos::new(0, anchor_y, 0), 1, 7);
        let zones = ZoneRegistry {
            zones: vec![test_zone(
                "high_zone",
                DimensionKind::Overworld,
                (
                    DVec3::new(-2.0, f64::from(anchor_y - 1), -2.0),
                    DVec3::new(2.0, f64::from(anchor_y + 1), 2.0),
                ),
            )],
        };

        let error =
            prepare_mineral_anchor_positions(std::slice::from_ref(&anchor), &terrain, &zones)
                .unwrap_err();

        assert!(
            error.contains("final candidate")
                && error.contains("after surface snap/dedup/max_units")
                && error.contains("lies outside declared zone `high_zone` AABB"),
            "raw sphere and center are inside the zone, but surface snap to y={surface_y} must be revalidated; actual: {error}"
        );
    }

    #[test]
    fn final_candidates_reject_more_specific_zone_capture_at_radius_edge() {
        let anchor = test_anchor("outer", BlockPos::new(5, 64, 5), 1, 7);
        let zones = ZoneRegistry {
            zones: vec![
                test_zone(
                    "outer",
                    DimensionKind::Overworld,
                    (DVec3::new(0.0, 0.0, 0.0), DVec3::new(10.0, 320.0, 10.0)),
                ),
                test_zone(
                    "inner",
                    DimensionKind::Overworld,
                    (DVec3::new(5.5, 0.0, 4.0), DVec3::new(6.5, 320.0, 6.0)),
                ),
            ],
        };

        let center = DVec3::new(5.0, 64.0, 5.0);
        assert_eq!(
            zones
                .find_zone(DimensionKind::Overworld, center)
                .expect("outer zone contains center")
                .name,
            "outer",
            "fixture center must remain owned by outer so only a final radius-edge point fails"
        );

        let error = prepare_mineral_anchor_positions(
            std::slice::from_ref(&anchor),
            &TerrainProvider::empty_for_tests(),
            &zones,
        )
        .unwrap_err();

        assert!(
            error.contains("final candidate")
                && error.contains("resolves to runtime zone `inner`, not declared `outer`"),
            "a nested zone that captures only a radius-edge candidate must fail the whole anchor; actual: {error}"
        );
    }

    #[test]
    fn mineral_gatherable_uses_stable_loot_table_and_display_name() {
        let registry = build_default_registry();
        let gatherable = mineral_gatherable(MineralId::FanTie, &registry);

        assert_eq!(
            gatherable.target,
            GatheringTargetKind::Ore,
            "mineral anchors must expose ore target metadata for gathering HUD and tool matching"
        );
        assert_eq!(
            gatherable.base_time_ticks,
            base_time_ticks(GatheringTargetKind::Ore),
            "mineral gatherable base time should follow the shared ore timing"
        );
        assert_eq!(
            gatherable.loot_table, "mineral:fan_tie",
            "mineral loot table key must use stable snake_case mineral id, not Debug formatting"
        );
        assert_eq!(
            gatherable.display_name, "凡铁",
            "mineral gatherable display name should come from the mineral registry"
        );
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
    fn production_registration_runs_mineral_startup_after_world_and_zone_registry() {
        let path = write_anchor_manifest(
            "production-startup-order",
            &[("spawn", "fan_tie", [16, 64, 16], 1, 1)],
        );
        let mut app = App::new();
        app.insert_resource(TerrainProviders {
            overworld: TerrainProvider::empty_for_tests(),
            tsy: None,
        });

        // Register both real producer identities after the real mineral consumer.
        // The setup system stays disabled because its full world bootstrap is not
        // part of this focused regression; the raw Startup graph still has to
        // carry both production dependencies before the schedule is run.
        crate::mineral::register(&mut app);
        crate::world::zone::register(&mut app);
        app.add_systems(Startup, crate::world::setup_world.run_if(|| false));
        app.insert_resource(MineralAnchorConfig::with_path(&path));
        app.insert_resource(ExhaustedMineralsLog::default());

        assert_production_startup_dependencies(&app);
        app.world_mut().run_schedule(Startup);

        let zones = app
            .world()
            .get_resource::<ZoneRegistry>()
            .expect("real zone startup register must initialize ZoneRegistry first");
        let spawn = zones
            .find_zone_by_name("spawn")
            .expect("runtime zone registry must contain the production spawn zone");
        assert!(
            spawn.contains(DVec3::new(16.0, 64.0, 16.0)),
            "production spawn zone must own the fixture anchor center"
        );

        let index_entries = app
            .world()
            .resource::<MineralOreIndex>()
            .iter()
            .collect::<Vec<_>>();
        assert_eq!(
            index_entries.len(),
            1,
            "real mineral register must materialize exactly one fixture node after zone startup"
        );
        let (dimension, indexed_pos, indexed_entity) = index_entries[0];
        assert_eq!(dimension, DimensionKind::Overworld);
        let actual_owner = zones
            .find_zone(
                dimension,
                DVec3::new(
                    f64::from(indexed_pos.x),
                    f64::from(indexed_pos.y),
                    f64::from(indexed_pos.z),
                ),
            )
            .expect("materialized node must resolve to a runtime zone");
        assert_eq!(
            actual_owner.name, "spawn",
            "materialized node must remain owned by its declared runtime zone"
        );

        let node = app
            .world()
            .get::<MineralOreNode>(indexed_entity)
            .cloned()
            .expect("indexed entity must carry the production ore component");
        let gatherable = app
            .world()
            .get::<Gatherable>(indexed_entity)
            .cloned()
            .expect("indexed entity must carry the production gathering component");
        assert_eq!(node.mineral_id, MineralId::FanTie);
        assert_eq!(node.position, indexed_pos);
        assert_eq!(node.remaining_units, 1);
        assert_eq!(gatherable.loot_table, "mineral:fan_tie");
        assert_eq!(
            app.world()
                .resource::<MineralOreIndex>()
                .lookup(DimensionKind::Overworld, node.position),
            Some(indexed_entity),
            "production entity and MineralOreIndex must agree on the materialized position"
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn startup_spawns_index_entries_and_skips_exhausted_positions() {
        let path = unique_tmp_path("startup");
        fs::write(
            &path,
            r#"{"version":1,"anchors":[{"zone":"spawn","mineral_id":"fan_tie","position":[0,65,0],"radius":1,"max_units":7}]}"#,
        )
        .unwrap();

        let anchor = MineralAnchor {
            zone: "spawn".into(),
            mineral_id: MineralId::FanTie,
            center: BlockPos::new(0, 65, 0),
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
            respawn_at_tick: None,
        });

        let mut app = App::new();
        app.insert_resource(MineralAnchorConfig::with_path(&path));
        app.insert_resource(build_default_registry());
        app.insert_resource(ZoneRegistry::fallback());
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
        let mut query = app.world_mut().query::<&Gatherable>();
        let gatherables = query.iter(app.world()).cloned().collect::<Vec<_>>();
        assert_eq!(
            gatherables.len(),
            6,
            "each spawned non-exhausted mineral node should carry Gatherable metadata"
        );
        for gatherable in &gatherables {
            assert_eq!(
                gatherable.target,
                GatheringTargetKind::Ore,
                "spawned mineral gatherable should be typed as ore"
            );
            assert_eq!(
                gatherable.base_time_ticks,
                base_time_ticks(GatheringTargetKind::Ore),
                "spawned mineral gatherable should use shared ore gather timing"
            );
            assert_eq!(
                gatherable.loot_table, "mineral:fan_tie",
                "spawned mineral gatherable should keep stable snake_case loot key"
            );
            assert_eq!(
                gatherable.display_name, "凡铁",
                "spawned mineral gatherable should expose registry display name"
            );
        }
        let _ = fs::remove_file(path);
    }

    #[test]
    fn startup_fails_closed_before_materializing_invalid_anchor() {
        let path = write_single_anchor_manifest("startup-invalid", "spawn", [1000, 64, 1000]);
        let mut app = App::new();
        app.insert_resource(MineralAnchorConfig::with_path(&path));
        app.insert_resource(build_default_registry());
        app.insert_resource(ZoneRegistry::fallback());
        app.insert_resource(ExhaustedMineralsLog::default());
        app.insert_resource(MineralOreIndex::default());
        app.insert_resource(TerrainProviders {
            overworld: TerrainProvider::empty_for_tests(),
            tsy: None,
        });
        app.add_systems(Startup, spawn_mineral_anchor_nodes);

        app.update();

        assert_eq!(
            app.world().resource::<MineralOreIndex>().len(),
            0,
            "invalid anchor manifest must fail closed before any ore index entry is materialized"
        );
        let mut query = app.world_mut().query::<&MineralOreNode>();
        assert_eq!(
            query.iter(app.world()).count(),
            0,
            "invalid anchor manifest must not leave spawned ore entities outside the index"
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn startup_keeps_ordinary_and_fossil_materialization_atomic_when_later_candidate_fails() {
        let success_path = write_anchor_manifest(
            "startup-atomic-success-baseline",
            &[("outer", "fan_tie", [-4, 64, 0], 1, 1)],
        );
        let invalid_path = write_anchor_manifest(
            "startup-atomic-final-candidate",
            &[
                ("outer", "fan_tie", [-4, 64, 0], 1, 1),
                ("outer", "za_gang", [5, 64, 5], 1, 7),
            ],
        );
        let zones = ZoneRegistry {
            zones: vec![
                test_zone(
                    "outer",
                    DimensionKind::Overworld,
                    (DVec3::new(-10.0, 0.0, -10.0), DVec3::new(10.0, 320.0, 10.0)),
                ),
                test_zone(
                    "inner",
                    DimensionKind::Overworld,
                    (DVec3::new(5.5, 0.0, 4.0), DVec3::new(6.5, 320.0, 6.0)),
                ),
            ],
        };
        let fossil = FossilBbox {
            zone: "outer".into(),
            name: "atomicity-fixture".into(),
            center_xz: [0, 0],
            center_y: 64,
            min_x: 0,
            max_x: 0,
            min_z: 0,
            max_z: 0,
            max_units: 1,
        };

        let success_terrain = TerrainProvider::with_fossil_for_tests(fossil.clone(), 2);
        let expected_fossil_nodes = fossil_mineral_positions(&fossil, &success_terrain);
        assert_eq!(
            expected_fossil_nodes.len(),
            1,
            "fixture must produce one real fossil candidate before testing batch atomicity"
        );
        let (expected_fossil_id, expected_fossil_pos) = expected_fossil_nodes[0];
        let ordinary_anchor = test_anchor("outer", BlockPos::new(-4, 64, 0), 1, 1);
        let expected_ordinary_positions = positions_for_anchor(&ordinary_anchor, &success_terrain);
        assert_eq!(
            expected_ordinary_positions.len(),
            1,
            "success baseline must produce one ordinary anchor candidate"
        );
        let expected_ordinary_pos = expected_ordinary_positions[0];
        assert_ne!(
            expected_ordinary_pos, expected_fossil_pos,
            "ordinary and fossil fixtures must exercise two independent index positions"
        );

        let mut success_app = materializer_app(&success_path, zones.clone(), success_terrain);
        success_app.update();

        assert_eq!(
            success_app.world().resource::<MineralOreIndex>().len(),
            2,
            "valid batch must materialize one ordinary node and one non-vacuous fossil node"
        );
        let ordinary_entity = success_app
            .world()
            .resource::<MineralOreIndex>()
            .lookup(DimensionKind::Overworld, expected_ordinary_pos)
            .expect("valid batch must index the ordinary anchor node");
        assert_eq!(
            success_app
                .world()
                .get::<MineralOreNode>(ordinary_entity)
                .expect("ordinary index entry must point to an ore node")
                .mineral_id,
            MineralId::FanTie
        );
        let fossil_entity = success_app
            .world()
            .resource::<MineralOreIndex>()
            .lookup(DimensionKind::Overworld, expected_fossil_pos)
            .expect("valid batch must index the mmap-backed fossil node");
        assert_eq!(
            success_app
                .world()
                .get::<MineralOreNode>(fossil_entity)
                .expect("fossil index entry must point to an ore node")
                .mineral_id,
            expected_fossil_id
        );
        let mut success_query = success_app.world_mut().query::<&MineralOreNode>();
        assert_eq!(
            success_query.iter(success_app.world()).count(),
            2,
            "success baseline must expose both ordinary and fossil entities"
        );

        let invalid_terrain = TerrainProvider::with_fossil_for_tests(fossil, 2);
        assert_eq!(
            fossil_mineral_positions(
                invalid_terrain
                    .fossil_bboxes()
                    .first()
                    .expect("invalid fixture must retain its fossil bbox"),
                &invalid_terrain,
            )
            .len(),
            1,
            "failure fixture must remain non-vacuous before the later anchor is rejected"
        );
        let mut invalid_app = materializer_app(&invalid_path, zones, invalid_terrain);
        invalid_app.update();

        assert_eq!(
            invalid_app.world().resource::<MineralOreIndex>().len(),
            0,
            "a valid first anchor must not enter the index when a later anchor fails final-candidate preflight"
        );
        assert_eq!(
            invalid_app
                .world()
                .resource::<MineralOreIndex>()
                .lookup(DimensionKind::Overworld, expected_ordinary_pos),
            None,
            "ordinary materialization must remain untouched until every final candidate passes"
        );
        assert_eq!(
            invalid_app
                .world()
                .resource::<MineralOreIndex>()
                .lookup(DimensionKind::Overworld, expected_fossil_pos),
            None,
            "fossil materialization must remain untouched when ordinary-anchor preflight fails"
        );
        let mut invalid_query = invalid_app.world_mut().query::<&MineralOreNode>();
        let invalid_nodes = invalid_query
            .iter(invalid_app.world())
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(
            invalid_nodes
                .iter()
                .filter(|node| node.position == expected_fossil_pos)
                .count(),
            0,
            "expected fossil position must have no spawned MineralOreNode when ordinary-anchor preflight fails"
        );
        assert_eq!(
            invalid_nodes.len(),
            0,
            "Commands::spawn must remain untouched for both ordinary and fossil nodes"
        );

        let _ = fs::remove_file(success_path);
        let _ = fs::remove_file(invalid_path);
    }

    // ─── plan-bughunt-mineral-anchor-position-drift-v1 ──────────────
    // 回归契约：`worldgen/blueprint/mineral_anchors.json` 的每条固定矿脉
    // anchor 必须（1）声明一个当前 runtime zone 表里真实存在的 zone，
    // （2）position 落在该 zone 的 AABB 内。此前 qingyun_peaks / blood_valley /
    // lingquan_marsh 的 9 条 anchor 全部用旧世界坐标，实际都落在 spawn AABB
    // 内；`rift_valley` 更是不存在的旧 zone id。加这条契约测试防止未来 zone
    // 坐标迁移时同类漂移静默复发。

    #[test]
    fn manifest_anchors_declare_zones_that_exist_in_runtime_registry() {
        let registry = build_default_registry();
        let zones = ZoneRegistry::load();
        let anchors =
            load_mineral_anchors(MineralAnchorConfig::default().path, &registry, &zones).unwrap();

        assert_eq!(
            anchors.len(),
            10,
            "默认 manifest 必须保留 10 条固定矿脉 anchor；删除任一远端矿点或重复一条配置都应撞红"
        );
        let expected_zone_minerals = HashSet::from([
            ("qingyun_peaks", "fan_tie"),
            ("qingyun_peaks", "za_gang"),
            ("qingyun_peaks", "ling_jing"),
            ("blood_valley", "ling_tie"),
            ("blood_valley", "wu_yao"),
            ("blood_valley", "zhu_sha"),
            ("blood_valley", "cu_tie"),
            ("lingquan_marsh", "yu_sui"),
            ("lingquan_marsh", "dan_sha"),
            ("spawn", "fan_tie"),
        ]);
        let actual_zone_minerals = anchors
            .iter()
            .map(|anchor| (anchor.zone.as_str(), anchor.mineral_id.as_str()))
            .collect::<HashSet<_>>();
        assert_eq!(
            actual_zone_minerals, expected_zone_minerals,
            "默认 manifest 的 zone/mineral 组合必须精确保持；跨区替换不能只靠总数 10 蒙混过关"
        );

        let prepared =
            prepare_mineral_anchor_positions(&anchors, &TerrainProvider::empty_for_tests(), &zones)
                .expect("默认十条 anchor 的 surface-snap 最终候选必须全部通过生产 preflight");
        assert_eq!(
            prepared.len(),
            10,
            "生产 preflight 必须为默认十条 anchor 各返回一组最终候选"
        );
        for (anchor, positions) in anchors.iter().zip(&prepared) {
            assert!(
                !positions.is_empty(),
                "默认 anchor `{}`/`{}` 不应在 snap/dedup/max_units 后退化为空",
                anchor.zone,
                anchor.mineral_id.as_str()
            );
            for pos in positions {
                let point = DVec3::new(f64::from(pos.x), f64::from(pos.y), f64::from(pos.z));
                let actual_zone = zones
                    .find_zone(DimensionKind::Overworld, point)
                    .expect("preflight-passed final candidate must resolve to a runtime zone");
                assert_eq!(
                    actual_zone.name,
                    anchor.zone,
                    "默认 anchor `{}`/`{}` 最终候选 {:?} 必须仍由声明 zone 拥有",
                    anchor.zone,
                    anchor.mineral_id.as_str(),
                    pos
                );
            }
        }

        for anchor in &anchors {
            let zone = zones.find_zone_by_name(&anchor.zone).unwrap_or_else(|| {
                panic!(
                    "mineral anchor `{}`({:?}) 声明的 zone `{}` 在当前 runtime zone 表(server/zones.json)中不存在——\
                     旧 zone id 迁移后必须同步更新 anchor 的 zone 字段",
                    anchor.mineral_id, anchor.center, anchor.zone
                )
            });
            let pos = valence::prelude::DVec3::new(
                anchor.center.x as f64,
                anchor.center.y as f64,
                anchor.center.z as f64,
            );
            assert!(
                zone.contains(pos),
                "mineral anchor `{}`({:?}) 声明 zone `{}`，但 position 不在该 zone 的 AABB {:?} 内——\
                 anchor 坐标已随 zone 坐标迁移漂移（远端矿脉门槛被压低到 spawn 附近）",
                anchor.mineral_id,
                anchor.center,
                anchor.zone,
                zone.bounds
            );
            let actual_zone = zones
                .find_zone(DimensionKind::Overworld, pos)
                .expect("declared Overworld zone contains the anchor center");
            assert_eq!(
                actual_zone.name, anchor.zone,
                "mineral anchor `{}`({:?}) center is captured by more-specific runtime zone `{}` instead of declared `{}`",
                anchor.mineral_id, anchor.center, actual_zone.name, anchor.zone
            );
        }
    }

    #[test]
    fn manifest_only_spawn_anchor_is_the_teaching_fan_tie_vein() {
        let registry = build_default_registry();
        let zones = ZoneRegistry::load();
        let anchors =
            load_mineral_anchors(MineralAnchorConfig::default().path, &registry, &zones).unwrap();

        let spawn_anchors: Vec<_> = anchors.iter().filter(|a| a.zone == "spawn").collect();
        assert_eq!(
            spawn_anchors.len(),
            1,
            "spawn 区应只保留教学用凡铁矿脉一条 anchor，其余矿点必须归属各自远端 zone"
        );
        assert_eq!(
            spawn_anchors[0].mineral_id,
            MineralId::FanTie,
            "spawn 区唯一保留的 anchor 必须是教学凡铁矿，不是任何远端稀有矿"
        );
    }

    #[test]
    fn manifest_no_longer_references_nonexistent_rift_valley_zone() {
        let registry = build_default_registry();
        let zones = ZoneRegistry::load();
        let anchors =
            load_mineral_anchors(MineralAnchorConfig::default().path, &registry, &zones).unwrap();

        assert!(
            anchors.iter().all(|a| a.zone != "rift_valley"),
            "`rift_valley` 不是当前 runtime zone 表中的合法 zone id（血谷 zone 名已是 \
             `blood_valley`），manifest 不应再引用这个旧 id"
        );
    }
}
