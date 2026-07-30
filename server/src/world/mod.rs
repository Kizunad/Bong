pub mod block_break;
pub mod block_drop;
pub mod block_place;
pub mod bong_blocks;
pub mod calamity;
pub mod container_block;
pub mod container_open;
pub mod dimension;
pub mod dimension_transfer;
pub mod entity_model;
pub mod environment;
pub mod environment_overlay;
pub mod era;
pub mod event_rhythm;
pub mod events;
pub mod extract_system;
pub mod furniture;
pub mod heartbeat;
pub mod karma;
pub mod loot_pool;
pub mod mob_spawn;
pub mod poi_mutant_nest;
pub mod poi_novice;
#[cfg(test)]
mod poi_novice_scatter_integration_test;
pub mod poi_respawn_tick;
pub mod pseudo_vein_runtime;
pub mod rift_portal;
pub mod risk_heatmap;
pub mod risk_signals;
pub mod season;
pub mod spawn_tutorial;
pub mod spirit_eye;
pub mod terrain;
pub mod territory;
pub mod territory_narration;
pub mod territory_perks;
pub mod territory_rumor;
pub mod tiandao_hunt;
pub mod tsy;
pub mod tsy_container;
pub mod tsy_container_search;
pub mod tsy_container_spawn;
pub mod tsy_dev_command;
pub mod tsy_drain;
pub mod tsy_filter;
#[cfg(test)]
mod tsy_integration_test;
pub mod tsy_lifecycle;
#[cfg(test)]
mod tsy_lifecycle_integration_test;
pub mod tsy_origin;
pub mod tsy_poi_consumer;
pub mod tsy_portal;
#[allow(dead_code)]
pub mod wangyintai_atmosphere;
pub mod weather_physics;
pub mod weather_to_environment;
pub mod zone;

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use valence::anvil::AnvilLevel;
use valence::prelude::{
    ident, App, BiomeRegistry, BlockState, ChunkPos, Commands, DimensionTypeRegistry, Entity,
    IntoSystemConfigs, LayerBundle, Res, ResMut, Server, Startup, UnloadedChunk, Update,
};

use self::dimension::{DimensionLayers, OverworldLayer, TsyLayer};

use crate::combat::CombatSystemSet;

const TEST_AREA_CHUNKS: i32 = 16;
const CHUNK_WIDTH: i32 = 16;
const NEW_PLAYER_VIEW_DISTANCE_CHUNKS: u8 =
    crate::cultivation::realm_vision::planner::AWAKEN_VIEW_DISTANCE_CHUNKS;
const FALLBACK_FLAT_MAX_CHUNKS: usize = 4_096;
const BEDROCK_Y: i32 = 64;
const GRASS_Y: i32 = BEDROCK_Y + 1;
pub(crate) const TEST_AREA_BLOCK_EXTENT: i32 = TEST_AREA_CHUNKS * CHUNK_WIDTH;
const TERRAIN_RASTER_PATH_ENV_VAR: &str = "BONG_TERRAIN_RASTER_PATH";
const WORLD_PATH_ENV_VAR: &str = "BONG_WORLD_PATH";
const ANVIL_REGION_DIR_NAME: &str = "region";

#[derive(Debug, Clone, PartialEq, Eq)]
enum WorldBootstrap {
    FallbackFlat(FallbackFlatBootstrap),
    TerrainRaster(terrain::RasterBootstrapConfig),
    AnvilIfPresent(AnvilBootstrapConfig),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FallbackFlatBootstrap {
    reason: FallbackFlatReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FallbackFlatReason {
    NoWorldBootstrapConfigured,
    TerrainManifestMissing(PathBuf),
    TerrainManifestNotFile(PathBuf),
    TerrainManifestUnreadable {
        manifest_path: PathBuf,
        error: String,
    },
    WorldPathMissing(PathBuf),
    WorldPathNotDirectory(PathBuf),
    WorldPathUnreadable {
        world_path: PathBuf,
        error: String,
    },
    RegionDirMissing(PathBuf),
    RegionDirEmpty(PathBuf),
    RegionDirInvalid(PathBuf),
    RegionDirUnreadable {
        region_dir: PathBuf,
        error: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AnvilBootstrapConfig {
    world_path: PathBuf,
    region_dir: PathBuf,
}

pub fn register(app: &mut App) {
    tracing::info!("[bong][world] registering world setup systems");
    dimension::register(app);
    dimension_transfer::register(app);
    // 默认方块破坏 apply（Creative Start / Survival Stop → set AIR）。各业务模块
    // 仍消费同一份 DiggingEvent 做 drop / 索引清理，本系统在 Update 阶段统一抹平
    // chunk —— 否则 vanilla / 普通方块挖了会"复原"。
    block_break::register(app);
    block_drop::register(app);
    container_block::register(app);
    container_open::register(app);
    block_place::register(app);
    zone::register(app);
    environment::register(app);
    weather_physics::register(app);
    season::register(app);
    calamity::register(app);
    era::register(app);
    tiandao_hunt::register(app);
    territory::register(app);
    territory_perks::register(app);
    pseudo_vein_runtime::register(app);
    spirit_eye::register(app);
    entity_model::register(app);
    furniture::register(app);
    app.insert_resource(karma::KarmaWeightStore::default());
    app.insert_resource(karma::QiDensityHeatmap::default());
    events::register(app);
    heartbeat::register(app);
    spawn_tutorial::register(app);
    terrain::register(app);
    // plan-tsy-zone-v1 §2.3 — drain tick 接到 combat::Physics set 内：
    // 同 tick 顺序为 wound_bleed_tick → tsy_drain_tick → death_arbiter_tick
    // （Physics 在 Resolve 之前，death_arbiter_tick 在 Resolve；Bevy 自动按 set
    // chain 排序，无需 .after 显式约束）
    app.add_systems(
        Update,
        tsy_drain::tsy_drain_tick.in_set(CombatSystemSet::Physics),
    );
    // plan-tsy-zone-v1 §3.3 / §3.4 — entry / exit portal tick；约束在
    // DimensionTransferSet 之前，让本 tick 内发的 DimensionTransferRequest 在
    // 同 tick 末由 apply_dimension_transfers 立即消费。
    tsy_portal::register(app);
    // plan-tsy-zone-v1 §3.1 — `/tsy_spawn` 调试命令的事件消费器
    tsy_dev_command::register(app);
    // plan-tsy-worldgen-v1 §1 — startup 期消费 TerrainProviders.pois() 把 POI 转 marker
    tsy_poi_consumer::register(app);
    // plan-poi-novice-v1 — startup 期消费 TerrainProviders.pois() 生成新手 POI registry。
    poi_novice::register(app);
    poi_respawn_tick::register(app);
    poi_mutant_nest::log_mutant_nest_contract();
    app.insert_resource(rift_portal::load_tsy_portals());
    // plan-tsy-lifecycle-v1 §1 — TSY 生命周期状态机 + 塌缩清理 + 道伥转化
    tsy_lifecycle::register(app);
    // plan-tsy-extract-v1 — TSY 定点撤离倒计时 + race-out 裂口。
    risk_heatmap::register(app);
    risk_signals::register(app);
    extract_system::register(app);
    // plan-tsy-container-v1 §2 — 搜刮 system + event 总线
    tsy_container_search::register(app);
    // plan-tsy-container-v1 §1.4 / §1.5 — loot pool + 容器 spawn 配置 resource
    let loot_pools = match loot_pool::load_loot_pool_registry() {
        Ok(reg) => reg,
        Err(err) => panic!("[bong][tsy-container] failed to load loot pools: {err}"),
    };
    let spawn_reg = match tsy_container_spawn::load_tsy_container_spawn_registry() {
        Ok(reg) => reg,
        Err(err) => panic!("[bong][tsy-container] failed to load tsy_containers.json: {err}"),
    };
    app.insert_resource(loot_pools);
    app.insert_resource(spawn_reg);
    app.add_systems(Startup, setup_world);
}

pub fn setup_world(
    mut commands: Commands,
    server: Res<Server>,
    mut dimensions: ResMut<DimensionTypeRegistry>,
    biomes: Res<BiomeRegistry>,
) {
    let overworld = match select_world_bootstrap() {
        WorldBootstrap::FallbackFlat(fallback) => {
            log_fallback_flat_selection(&fallback.reason);
            tracing::info!("[bong][world] starting fallback flat world bootstrap");
            spawn_fallback_flat_world(&mut commands, &server, &dimensions, &biomes)
        }
        WorldBootstrap::TerrainRaster(config) => {
            tracing::info!(
                "[bong][world] selected terrain raster bootstrap from {}",
                config.manifest_path.display()
            );
            terrain::spawn_raster_world(&mut commands, &server, &mut dimensions, &biomes, config)
        }
        WorldBootstrap::AnvilIfPresent(anvil) => {
            tracing::info!(
                "[bong][world] selected anvil bootstrap from {} with region dir {}, starting Anvil-backed world bootstrap",
                anvil.world_path.display(),
                anvil.region_dir.display()
            );
            spawn_anvil_world(&mut commands, &server, &dimensions, &biomes, anvil)
        }
    };

    let tsy = spawn_tsy_layer(&mut commands, &server, &dimensions, &biomes);
    tracing::info!("[bong][world] spawned tsy dimension layer (empty, awaits worldgen)");
    commands.insert_resource(DimensionLayers { overworld, tsy });
}

fn spawn_tsy_layer(
    commands: &mut Commands,
    server: &Server,
    dimensions: &DimensionTypeRegistry,
    biomes: &BiomeRegistry,
) -> Entity {
    let layer = LayerBundle::new(ident!("bong:tsy"), dimensions, biomes, server);
    commands.spawn((layer, TsyLayer)).id()
}

fn select_world_bootstrap() -> WorldBootstrap {
    select_world_bootstrap_from_configured_paths(
        configured_terrain_raster_path(),
        configured_world_path(),
    )
}

fn configured_terrain_raster_path() -> Option<PathBuf> {
    std::env::var_os(TERRAIN_RASTER_PATH_ENV_VAR).and_then(|value| {
        if value.is_empty() {
            None
        } else {
            Some(PathBuf::from(value))
        }
    })
}

fn configured_world_path() -> Option<PathBuf> {
    std::env::var_os(WORLD_PATH_ENV_VAR).and_then(|value| {
        if value.is_empty() {
            None
        } else {
            Some(PathBuf::from(value))
        }
    })
}

fn select_world_bootstrap_from_configured_paths(
    terrain_manifest_path: Option<PathBuf>,
    world_path: Option<PathBuf>,
) -> WorldBootstrap {
    if let Some(manifest_path) = terrain_manifest_path {
        match fs::metadata(&manifest_path) {
            Ok(metadata) if metadata.is_file() => {
                let raster_dir = match terrain::raster_dir_from_manifest_path(&manifest_path) {
                    Ok(path) => path,
                    Err(error) => {
                        return fallback_flat(FallbackFlatReason::TerrainManifestUnreadable {
                            manifest_path,
                            error,
                        });
                    }
                };
                return WorldBootstrap::TerrainRaster(terrain::RasterBootstrapConfig {
                    manifest_path,
                    raster_dir,
                });
            }
            Ok(_) => {
                return fallback_flat(FallbackFlatReason::TerrainManifestNotFile(manifest_path));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return fallback_flat(FallbackFlatReason::TerrainManifestMissing(manifest_path));
            }
            Err(error) => {
                return fallback_flat(FallbackFlatReason::TerrainManifestUnreadable {
                    manifest_path,
                    error: error.to_string(),
                });
            }
        }
    }

    let Some(world_path) = world_path else {
        return fallback_flat(FallbackFlatReason::NoWorldBootstrapConfigured);
    };

    match fs::metadata(&world_path) {
        Ok(metadata) if metadata.is_dir() => {}
        Ok(_) => {
            return fallback_flat(FallbackFlatReason::WorldPathNotDirectory(world_path));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return fallback_flat(FallbackFlatReason::WorldPathMissing(world_path));
        }
        Err(error) => {
            return fallback_flat(FallbackFlatReason::WorldPathUnreadable {
                world_path,
                error: error.to_string(),
            });
        }
    }

    let region_dir = world_path.join(ANVIL_REGION_DIR_NAME);

    match fs::read_dir(&region_dir) {
        Ok(entries) => select_world_bootstrap_from_region_entries(world_path, region_dir, entries),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fallback_flat(FallbackFlatReason::RegionDirMissing(region_dir))
        }
        Err(error) => fallback_flat(FallbackFlatReason::RegionDirUnreadable {
            region_dir,
            error: error.to_string(),
        }),
    }
}

fn fallback_flat(reason: FallbackFlatReason) -> WorldBootstrap {
    WorldBootstrap::FallbackFlat(FallbackFlatBootstrap { reason })
}

fn select_world_bootstrap_from_region_entries(
    world_path: PathBuf,
    region_dir: PathBuf,
    entries: fs::ReadDir,
) -> WorldBootstrap {
    let mut saw_any_entry = false;

    for entry_result in entries {
        saw_any_entry = true;

        let entry = match entry_result {
            Ok(entry) => entry,
            Err(error) => {
                return fallback_flat(FallbackFlatReason::RegionDirUnreadable {
                    region_dir,
                    error: error.to_string(),
                });
            }
        };

        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) => {
                return fallback_flat(FallbackFlatReason::RegionDirUnreadable {
                    region_dir,
                    error: error.to_string(),
                });
            }
        };

        if !file_type.is_file() {
            continue;
        }

        let file_name = entry.file_name();
        let Some(file_name) = file_name.to_str() else {
            continue;
        };

        if !is_anvil_region_file_name(file_name) {
            continue;
        }

        if let Err(error) = fs::File::open(entry.path()) {
            return fallback_flat(FallbackFlatReason::RegionDirUnreadable {
                region_dir,
                error: error.to_string(),
            });
        }

        return WorldBootstrap::AnvilIfPresent(AnvilBootstrapConfig {
            world_path,
            region_dir,
        });
    }

    if saw_any_entry {
        fallback_flat(FallbackFlatReason::RegionDirInvalid(region_dir))
    } else {
        fallback_flat(FallbackFlatReason::RegionDirEmpty(region_dir))
    }
}

fn is_anvil_region_file_name(file_name: &str) -> bool {
    let mut parts = file_name.split('.');

    matches!(parts.next(), Some("r"))
        && parts
            .next()
            .and_then(|region_x| region_x.parse::<i32>().ok())
            .is_some()
        && parts
            .next()
            .and_then(|region_z| region_z.parse::<i32>().ok())
            .is_some()
        && matches!(parts.next(), Some("mca"))
        && parts.next().is_none()
}

fn log_fallback_flat_selection(reason: &FallbackFlatReason) {
    match reason {
        FallbackFlatReason::NoWorldBootstrapConfigured => {
            tracing::info!(
                "[bong][world] no world bootstrap configured via {} or {}, selecting fallback flat bootstrap",
                TERRAIN_RASTER_PATH_ENV_VAR,
                WORLD_PATH_ENV_VAR
            );
        }
        FallbackFlatReason::TerrainManifestMissing(manifest_path) => {
            tracing::info!(
                "[bong][world] configured terrain manifest {} is missing, selecting fallback flat bootstrap",
                manifest_path.display()
            );
        }
        FallbackFlatReason::TerrainManifestNotFile(manifest_path) => {
            tracing::warn!(
                "[bong][world] configured terrain manifest {} is not a file, selecting fallback flat bootstrap",
                manifest_path.display()
            );
        }
        FallbackFlatReason::TerrainManifestUnreadable {
            manifest_path,
            error,
        } => {
            tracing::warn!(
                "[bong][world] failed to inspect configured terrain manifest {} for bootstrap selection, selecting fallback flat bootstrap: {error}",
                manifest_path.display()
            );
        }
        FallbackFlatReason::WorldPathMissing(world_path) => {
            tracing::info!(
                "[bong][world] configured world path {} is missing, selecting fallback flat bootstrap",
                world_path.display()
            );
        }
        FallbackFlatReason::WorldPathNotDirectory(world_path) => {
            tracing::warn!(
                "[bong][world] configured world path {} is not a directory, selecting fallback flat bootstrap",
                world_path.display()
            );
        }
        FallbackFlatReason::WorldPathUnreadable { world_path, error } => {
            tracing::warn!(
                "[bong][world] failed to inspect configured world path {} for bootstrap selection, selecting fallback flat bootstrap: {error}",
                world_path.display()
            );
        }
        FallbackFlatReason::RegionDirMissing(region_dir) => {
            tracing::info!(
                "[bong][world] no region directory at {}, selecting fallback flat bootstrap",
                region_dir.display()
            );
        }
        FallbackFlatReason::RegionDirEmpty(region_dir) => {
            tracing::info!(
                "[bong][world] region directory at {} is empty, selecting fallback flat bootstrap",
                region_dir.display()
            );
        }
        FallbackFlatReason::RegionDirInvalid(region_dir) => {
            tracing::warn!(
                "[bong][world] region directory at {} has no readable Anvil region assets (*.mca), selecting fallback flat bootstrap",
                region_dir.display()
            );
        }
        FallbackFlatReason::RegionDirUnreadable { region_dir, error } => {
            tracing::warn!(
                "[bong][world] failed to inspect region directory {} for bootstrap selection, selecting fallback flat bootstrap: {error}",
                region_dir.display()
            );
        }
    }
}

fn spawn_anvil_world(
    commands: &mut Commands,
    server: &Server,
    dimensions: &DimensionTypeRegistry,
    biomes: &BiomeRegistry,
    anvil: AnvilBootstrapConfig,
) -> Entity {
    tracing::info!(
        "[bong][world] creating overworld layer backed by Anvil terrain at {}",
        anvil.world_path.display()
    );

    let layer = LayerBundle::new(ident!("overworld"), dimensions, biomes, server);
    let anvil_level = AnvilLevel::new(&anvil.world_path, biomes);

    commands.spawn((layer, anvil_level, OverworldLayer)).id()
}

fn spawn_fallback_flat_world(
    commands: &mut Commands,
    server: &Server,
    dimensions: &DimensionTypeRegistry,
    biomes: &BiomeRegistry,
) -> Entity {
    let registry = zone::ZoneRegistry::load();
    let anchors = crate::player::spawn_selector::effective_default_spawn_distribution(&registry);
    let chunks = match fallback_spawn_chunk_union(&registry, &anchors) {
        Ok(chunks) => chunks,
        Err(chunk_count) => {
            tracing::error!(
                chunks = chunk_count,
                limit = FALLBACK_FLAT_MAX_CHUNKS,
                "[bong][world] fallback spawn chunk union exceeded safety limit during construction; refusing eager world allocation"
            );
            panic!(
                "fallback spawn chunk union has at least {chunk_count} chunks, above safety limit {FALLBACK_FLAT_MAX_CHUNKS}"
            );
        }
    };

    let mut layer = LayerBundle::new(ident!("overworld"), dimensions, biomes, server);

    for chunk_pos in &chunks {
        layer.chunk.insert_chunk(*chunk_pos, UnloadedChunk::new());
        let block_min_x = chunk_pos.x * CHUNK_WIDTH;
        let block_min_z = chunk_pos.z * CHUNK_WIDTH;
        for local_z in 0..CHUNK_WIDTH {
            for local_x in 0..CHUNK_WIDTH {
                let x = block_min_x + local_x;
                let z = block_min_z + local_z;
                layer
                    .chunk
                    .set_block([x, BEDROCK_Y, z], BlockState::BEDROCK);
                layer
                    .chunk
                    .set_block([x, GRASS_Y, z], BlockState::GRASS_BLOCK);
            }
        }
    }

    scatter_spawn_resources(&mut layer.chunk, &anchors);

    let layer_entity = commands.spawn((layer, OverworldLayer)).id();
    tracing::info!(
        "[bong][world] BOT_FALLBACK_FLAT_READY anchors={} chunks={} view_distance_chunks={}",
        anchors.len(),
        chunks.len(),
        NEW_PLAYER_VIEW_DISTANCE_CHUNKS,
    );
    layer_entity
}

fn fallback_spawn_chunk_union(
    registry: &zone::ZoneRegistry,
    anchors: &[crate::player::spawn_selector::SpawnDistributionAnchor],
) -> Result<BTreeSet<ChunkPos>, usize> {
    let Some(spawn_zone) = registry.find_zone_by_name(zone::DEFAULT_SPAWN_ZONE_NAME) else {
        return Ok(BTreeSet::new());
    };
    let (zone_min, zone_max) = spawn_zone.bounds;
    let mut chunks = BTreeSet::new();

    for anchor in anchors {
        let (anchor_pos, radius) = anchor.cluster();
        let min_spawn_x = (anchor_pos.x - radius).clamp(zone_min.x, zone_max.x);
        let max_spawn_x = (anchor_pos.x + radius).clamp(zone_min.x, zone_max.x);
        let min_spawn_z = (anchor_pos.z - radius).clamp(zone_min.z, zone_max.z);
        let max_spawn_z = (anchor_pos.z + radius).clamp(zone_min.z, zone_max.z);
        let min_spawn_chunk = ChunkPos::new(
            block_coord_to_chunk(min_spawn_x),
            block_coord_to_chunk(min_spawn_z),
        );
        let max_spawn_chunk = ChunkPos::new(
            block_coord_to_chunk(max_spawn_x),
            block_coord_to_chunk(max_spawn_z),
        );
        let min_view =
            valence::prelude::ChunkView::new(min_spawn_chunk, NEW_PLAYER_VIEW_DISTANCE_CHUNKS);
        let max_view =
            valence::prelude::ChunkView::new(max_spawn_chunk, NEW_PLAYER_VIEW_DISTANCE_CHUNKS);
        let (min_chunk, _) = min_view.bounding_box();
        let (_, max_chunk) = max_view.bounding_box();

        let rectangle_width = i128::from(max_chunk.x) - i128::from(min_chunk.x) + 1;
        let rectangle_height = i128::from(max_chunk.z) - i128::from(min_chunk.z) + 1;
        let rectangle_chunks = rectangle_width * rectangle_height;
        if rectangle_chunks > FALLBACK_FLAT_MAX_CHUNKS as i128 {
            return Err(usize::try_from(rectangle_chunks).unwrap_or(usize::MAX));
        }

        for chunk_z in min_chunk.z..=max_chunk.z {
            for chunk_x in min_chunk.x..=max_chunk.x {
                chunks.insert(ChunkPos::new(chunk_x, chunk_z));
                if chunks.len() > FALLBACK_FLAT_MAX_CHUNKS {
                    return Err(chunks.len());
                }
            }
        }
    }

    Ok(chunks)
}

fn block_coord_to_chunk(coord: f64) -> i32 {
    (coord.floor() as i32).div_euclid(CHUNK_WIDTH)
}

/// 在每个出生分布簇附近散布基础资源方块（树木、石头、铁矿），让新玩家裸手可采。
/// 这些方块不在 MineralOreIndex 里，由 block_drop 系统处理掉落。
fn scatter_spawn_resources(
    chunk_layer: &mut valence::prelude::ChunkLayer,
    anchors: &[crate::player::spawn_selector::SpawnDistributionAnchor],
) {
    let tree_y = GRASS_Y + 1;

    struct Deposit {
        dx: i32,
        dz: i32,
        block: BlockState,
        count: i32,
    }

    let deposits = [
        // 两棵橡木（相对簇中心东北方向）
        Deposit {
            dx: 12,
            dz: 8,
            block: BlockState::OAK_LOG,
            count: 4,
        },
        Deposit {
            dx: 20,
            dz: 14,
            block: BlockState::OAK_LOG,
            count: 5,
        },
        // 一棵白桦
        Deposit {
            dx: 6,
            dz: 22,
            block: BlockState::BIRCH_LOG,
            count: 3,
        },
        // 铁矿露头（3 处，每处 2-3 块，嵌在地面里）
        Deposit {
            dx: 30,
            dz: 10,
            block: BlockState::IRON_ORE,
            count: 3,
        },
        Deposit {
            dx: 35,
            dz: 18,
            block: BlockState::IRON_ORE,
            count: 2,
        },
        Deposit {
            dx: 28,
            dz: 26,
            block: BlockState::IRON_ORE,
            count: 2,
        },
        // 石头露头
        Deposit {
            dx: 18,
            dz: 30,
            block: BlockState::STONE,
            count: 4,
        },
        Deposit {
            dx: 40,
            dz: 6,
            block: BlockState::STONE,
            count: 3,
        },
    ];

    for anchor in anchors {
        let anchor_pos = anchor.anchor();
        let base_x = anchor_pos.x.floor() as i32;
        let base_z = anchor_pos.z.floor() as i32;
        for deposit in &deposits {
            let x = base_x + deposit.dx;
            let z = base_z + deposit.dz;
            if deposit.block == BlockState::OAK_LOG || deposit.block == BlockState::BIRCH_LOG {
                for dy in 0..deposit.count {
                    chunk_layer.set_block([x, tree_y + dy, z], deposit.block);
                }
                let leaves = match deposit.block {
                    BlockState::BIRCH_LOG => BlockState::BIRCH_LEAVES,
                    _ => BlockState::OAK_LEAVES,
                };
                chunk_layer.set_block([x, tree_y + deposit.count, z], leaves);
            } else {
                for i in 0..deposit.count {
                    let ox = i % 2;
                    let oz = i / 2;
                    chunk_layer.set_block([x + ox, GRASS_Y, z + oz], deposit.block);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::event_rhythm::{
        default_event_rhythm, event_trigger_timing_by_player_loop_phase as rhythm_timing_by_phase,
        PlayerLoopPhase, RhythmEventKind,
    };
    use super::zone::{default_spawn_bounds, Zone, ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};
    use super::{
        block_coord_to_chunk, fallback_spawn_chunk_union, select_world_bootstrap,
        select_world_bootstrap_from_configured_paths, terrain::RasterBootstrapConfig,
        AnvilBootstrapConfig, FallbackFlatBootstrap, FallbackFlatReason, WorldBootstrap,
        ANVIL_REGION_DIR_NAME, FALLBACK_FLAT_MAX_CHUNKS, GRASS_Y, NEW_PLAYER_VIEW_DISTANCE_CHUNKS,
        TERRAIN_RASTER_PATH_ENV_VAR, WORLD_PATH_ENV_VAR,
    };
    use valence::prelude::{
        BlockPos, BlockState, ChunkLayer, ChunkPos, ChunkView, DVec3, UnloadedChunk,
    };
    use valence::testing::ScenarioSingleClient;

    #[test]
    fn fallback_spawn_zone_exists() {
        let registry = ZoneRegistry::fallback();
        let spawn_zone = registry
            .find_zone(
                crate::world::dimension::DimensionKind::Overworld,
                DVec3::new(8.0, 66.0, 8.0),
            )
            .expect("fallback registry should always contain the spawn zone");

        assert_eq!(spawn_zone.name, DEFAULT_SPAWN_ZONE_NAME);
        assert_eq!(spawn_zone.bounds, default_spawn_bounds());
        assert_eq!(spawn_zone.spirit_qi, 0.9);
        assert_eq!(spawn_zone.danger_level, 0);
    }

    #[test]
    fn event_trigger_timing_by_player_loop_phase() {
        let config = default_event_rhythm();

        let pseudo = rhythm_timing_by_phase(
            config,
            RhythmEventKind::PseudoVein,
            PlayerLoopPhase::ReturnTrip,
        )
        .expect("pseudo_vein timing rule should exist");
        assert!(
            pseudo.is_preferred_phase,
            "伪灵脉应优先插在回程路上，而不是随机打断所有阶段"
        );
        assert!(
            pseudo
                .emotional_effects
                .iter()
                .any(|effect| effect == "temptation"),
            "伪灵脉节奏表必须声明诱惑情绪作用"
        );

        let beast = rhythm_timing_by_phase(
            config,
            RhythmEventKind::BeastTide,
            PlayerLoopPhase::DeepGathering,
        )
        .expect("beast_tide timing rule should exist");
        assert!(
            beast.is_preferred_phase,
            "兽潮应优先插在深处采集阶段，提供恐慌和逆流机会"
        );
        assert!(
            beast.timing.lead_ticks < pseudo.timing.lead_ticks,
            "深处采集的兽潮预警应比回程伪灵脉更短，形成当趟决策压力"
        );

        let tide = rhythm_timing_by_phase(
            config,
            RhythmEventKind::TideSkyOmen,
            PlayerLoopPhase::HomeOrganizing,
        )
        .expect("tide_sky_omen timing rule should exist");
        assert!(
            tide.is_preferred_phase,
            "汐转期天象应优先在灵龛整理阶段影响下一趟路线"
        );

        let collapse = rhythm_timing_by_phase(
            config,
            RhythmEventKind::RealmCollapse,
            PlayerLoopPhase::SafeShelter,
        )
        .expect("realm_collapse timing rule should exist");
        assert!(
            collapse.is_preferred_phase,
            "域崩应优先从安全区被感知，避免变成贴脸秒杀"
        );

        let tribulation = rhythm_timing_by_phase(
            config,
            RhythmEventKind::TribulationBroadcast,
            PlayerLoopPhase::OutboundSearch,
        )
        .expect("tribulation_broadcast timing rule should exist");
        assert!(
            tribulation
                .emotional_effects
                .iter()
                .any(|effect| effect == "spectator_impulse"),
            "天劫广播必须声明围观冲动，供下一趟目标选择消费"
        );
    }

    #[test]
    fn missing_zones_file_uses_spawn_fallback() {
        let missing_path = missing_zones_path();
        let registry = ZoneRegistry::load_from_path(&missing_path);
        let spawn_zone = registry
            .find_zone(
                crate::world::dimension::DimensionKind::Overworld,
                DVec3::new(8.0, 66.0, 8.0),
            )
            .expect("missing zones.json should fall back to the spawn zone");

        assert_eq!(registry.zones.len(), 1);
        assert_eq!(spawn_zone.name, DEFAULT_SPAWN_ZONE_NAME);
        assert_eq!(spawn_zone.bounds, default_spawn_bounds());
    }

    #[test]
    fn chunk_conversion_uses_euclidean_floor_for_negative_boundaries() {
        assert_eq!(block_coord_to_chunk(-0.1), -1);
        assert_eq!(block_coord_to_chunk(-1.0), -1);
        assert_eq!(block_coord_to_chunk(-16.0), -1);
        assert_eq!(block_coord_to_chunk(-16.1), -2);
        assert_eq!(block_coord_to_chunk(-17.0), -2);
        assert_eq!(block_coord_to_chunk(0.0), 0);
        assert_eq!(block_coord_to_chunk(15.9), 0);
        assert_eq!(block_coord_to_chunk(16.0), 1);
    }

    #[test]
    fn fallback_chunk_union_covers_each_real_spawn_view_without_global_bridge() {
        let registry = ZoneRegistry::load();
        let anchors =
            crate::player::spawn_selector::effective_default_spawn_distribution(&registry);
        let chunks = fallback_spawn_chunk_union(&registry, &anchors)
            .expect("configured fallback union should fit");

        assert!(anchors.len() >= 3, "zones.json 应提供多个出生分布簇");
        for anchor in &anchors {
            let (center, radius) = anchor.cluster();
            let spawn_min_x = block_coord_to_chunk(center.x - radius);
            let spawn_max_x = block_coord_to_chunk(center.x + radius);
            let spawn_min_z = block_coord_to_chunk(center.z - radius);
            let spawn_max_z = block_coord_to_chunk(center.z + radius);
            for spawn_chunk_z in spawn_min_z..=spawn_max_z {
                for spawn_chunk_x in spawn_min_x..=spawn_max_x {
                    let view = ChunkView::new(
                        ChunkPos::new(spawn_chunk_x, spawn_chunk_z),
                        crate::cultivation::realm_vision::planner::AWAKEN_VIEW_DISTANCE_CHUNKS,
                    );
                    assert!(
                        view.iter().all(|chunk| chunks.contains(&chunk)),
                        "出生簇 center=({},{}) radius={} 的候选 chunk=({},{}) 必须覆盖醒灵完整 Valence 视域",
                        center.x,
                        center.z,
                        radius,
                        spawn_chunk_x,
                        spawn_chunk_z,
                    );
                }
            }
        }

        let cluster_boxes = anchors
            .iter()
            .map(|anchor| {
                let (center, radius) = anchor.cluster();
                let min_view = ChunkView::new(
                    ChunkPos::new(
                        block_coord_to_chunk(center.x - radius),
                        block_coord_to_chunk(center.z - radius),
                    ),
                    crate::cultivation::realm_vision::planner::AWAKEN_VIEW_DISTANCE_CHUNKS,
                );
                let max_view = ChunkView::new(
                    ChunkPos::new(
                        block_coord_to_chunk(center.x + radius),
                        block_coord_to_chunk(center.z + radius),
                    ),
                    crate::cultivation::realm_vision::planner::AWAKEN_VIEW_DISTANCE_CHUNKS,
                );
                let (min_chunk, _) = min_view.bounding_box();
                let (_, max_chunk) = max_view.bounding_box();
                (min_chunk, max_chunk)
            })
            .collect::<Vec<_>>();
        let global_min_x = cluster_boxes
            .iter()
            .map(|(min_chunk, _)| min_chunk.x)
            .min()
            .expect("spawn distribution should not be empty");
        let global_min_z = cluster_boxes
            .iter()
            .map(|(min_chunk, _)| min_chunk.z)
            .min()
            .expect("spawn distribution should not be empty");
        let global_max_x = cluster_boxes
            .iter()
            .map(|(_, max_chunk)| max_chunk.x)
            .max()
            .expect("spawn distribution should not be empty");
        let global_max_z = cluster_boxes
            .iter()
            .map(|(_, max_chunk)| max_chunk.z)
            .max()
            .expect("spawn distribution should not be empty");
        let bridge_chunk = (global_min_z..=global_max_z).find_map(|chunk_z| {
            (global_min_x..=global_max_x).find_map(|chunk_x| {
                let candidate = ChunkPos::new(chunk_x, chunk_z);
                let inside_local_union = cluster_boxes.iter().any(|(min_chunk, max_chunk)| {
                    candidate.x >= min_chunk.x
                        && candidate.x <= max_chunk.x
                        && candidate.z >= min_chunk.z
                        && candidate.z <= max_chunk.z
                });
                (!inside_local_union).then_some(candidate)
            })
        });
        let Some(bridge_chunk) = bridge_chunk else {
            assert_eq!(
                cluster_boxes.len(),
                1,
                "多个出生簇若无桥接 gap，必须明确证明只剩单簇：{cluster_boxes:?}"
            );
            return;
        };
        assert!(
            !chunks.contains(&bridge_chunk),
            "局部出生簇 union 不得退化为跨远端 anchors 的全局 AABB 桥接；gap={bridge_chunk:?}"
        );
    }

    #[test]
    fn fallback_flat_chunk_count_guard_accepts_configured_union_and_rejects_excess() {
        let registry = ZoneRegistry::load();
        let anchors =
            crate::player::spawn_selector::effective_default_spawn_distribution(&registry);
        let configured = fallback_spawn_chunk_union(&registry, &anchors)
            .expect("当前 zones.json fallback union 必须留在显式 eager-allocation 上限内");
        assert!(
            configured.len() <= FALLBACK_FLAT_MAX_CHUNKS,
            "已受理的 fallback union 不得超过 eager-allocation 上限"
        );

        let oversized_registry = synthetic_spawn_registry((
            DVec3::new(-1_000_000.0, 0.0, -1_000_000.0),
            DVec3::new(1_000_000.0, 100.0, 1_000_000.0),
        ));
        let oversized_anchor = crate::player::spawn_selector::spawn_distribution_anchor_for_test(
            DVec3::ZERO,
            1_000_000.0,
        );
        let (oversized_center, oversized_radius) = oversized_anchor.cluster();
        let oversized_min_view = ChunkView::new(
            ChunkPos::new(
                block_coord_to_chunk(oversized_center.x - oversized_radius),
                block_coord_to_chunk(oversized_center.z - oversized_radius),
            ),
            NEW_PLAYER_VIEW_DISTANCE_CHUNKS,
        );
        let oversized_max_view = ChunkView::new(
            ChunkPos::new(
                block_coord_to_chunk(oversized_center.x + oversized_radius),
                block_coord_to_chunk(oversized_center.z + oversized_radius),
            ),
            NEW_PLAYER_VIEW_DISTANCE_CHUNKS,
        );
        let (oversized_min_chunk, _) = oversized_min_view.bounding_box();
        let (_, oversized_max_chunk) = oversized_max_view.bounding_box();
        let oversized_count = usize::try_from(
            (i128::from(oversized_max_chunk.x) - i128::from(oversized_min_chunk.x) + 1)
                * (i128::from(oversized_max_chunk.z) - i128::from(oversized_min_chunk.z) + 1),
        )
        .expect("synthetic fallback rectangle count should fit usize");
        let oversized_anchors = [oversized_anchor];
        assert_eq!(
            fallback_spawn_chunk_union(&oversized_registry, &oversized_anchors),
            Err(oversized_count),
            "单个巨大合法矩形必须在分配 BTreeSet 前 fail closed，并报告精确候选 chunk 数"
        );

        let disjoint_anchors = (0..=FALLBACK_FLAT_MAX_CHUNKS)
            .map(|index| {
                crate::player::spawn_selector::spawn_distribution_anchor_for_test(
                    DVec3::new(index as f64 * 16_000.0, 65.0, 0.0),
                    0.0,
                )
            })
            .collect::<Vec<_>>();
        let disjoint_registry = synthetic_spawn_registry((
            DVec3::new(-1.0, 0.0, -1.0),
            DVec3::new(FALLBACK_FLAT_MAX_CHUNKS as f64 * 16_000.0 + 1.0, 100.0, 1.0),
        ));
        assert_eq!(
            fallback_spawn_chunk_union(&disjoint_registry, &disjoint_anchors),
            Err(FALLBACK_FLAT_MAX_CHUNKS + 1),
            "多个小矩形的 union 累积越界也必须在第 4097 个唯一 chunk 处 fail closed"
        );
    }

    #[test]
    fn clamped_spawn_disk_and_emergency_position_remain_covered() {
        let registry = ZoneRegistry::load();
        let anchors =
            crate::player::spawn_selector::effective_default_spawn_distribution(&registry);
        let chunks = fallback_spawn_chunk_union(&registry, &anchors)
            .expect("configured fallback union should fit");
        let spawn_zone = registry
            .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
            .expect("spawn zone should load");
        let (zone_min, zone_max) = spawn_zone.bounds;

        for anchor in &anchors {
            let (center, radius) = anchor.cluster();
            for pos in [
                DVec3::new(center.x - radius, center.y, center.z - radius),
                DVec3::new(center.x + radius, center.y, center.z + radius),
            ] {
                let clamped = DVec3::new(
                    pos.x.clamp(zone_min.x, zone_max.x),
                    pos.y,
                    pos.z.clamp(zone_min.z, zone_max.z),
                );
                assert!(chunks.contains(&ChunkPos::from(clamped)));
            }
        }

        assert!(chunks.contains(&ChunkPos::from(DVec3::from(
            crate::player::spawn_selector::EMERGENCY_SPAWN_POSITION,
        ))));
    }

    #[test]
    fn selected_spawn_chunks_and_views_are_inside_fallback_union() {
        let registry = ZoneRegistry::load();
        let anchors =
            crate::player::spawn_selector::effective_default_spawn_distribution(&registry);
        let chunks = fallback_spawn_chunk_union(&registry, &anchors)
            .expect("configured fallback union should fit");

        for purpose in [
            crate::player::spawn_selector::SpawnPurpose::InitialLogin,
            crate::player::spawn_selector::SpawnPurpose::NewLifeBirth,
            crate::player::spawn_selector::SpawnPurpose::FallRecovery,
        ] {
            for seed in ["offline:Alice", "offline:Bob", "offline:Boundary-127"] {
                let pos = crate::player::spawn_selector::fallback_spawn(seed, purpose);
                let spawn_chunk = ChunkPos::from(DVec3::from(pos));
                let view = ChunkView::new(
                    spawn_chunk,
                    crate::cultivation::realm_vision::planner::AWAKEN_VIEW_DISTANCE_CHUNKS,
                );
                assert!(
                    view.iter().all(|chunk| chunks.contains(&chunk)),
                    "seed={seed} purpose={purpose:?} spawn={pos:?} 的完整醒灵视域必须在 fallback union 内"
                );
            }
        }
    }

    fn synthetic_spawn_registry(bounds: (DVec3, DVec3)) -> ZoneRegistry {
        ZoneRegistry {
            zones: vec![Zone {
                name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
                dimension: crate::world::dimension::DimensionKind::Overworld,
                bounds,
                spirit_qi: 0.9,
                danger_level: 0,
                active_events: Vec::new(),
                patrol_anchors: vec![DVec3::new(-0.1, 65.0, -0.1)],
                blocked_tiles: Vec::new(),
                qi_equilibrium: 0.0,
                qi_inflow_per_min: 0.0,
            }],
        }
    }

    #[test]
    fn negative_anchor_margin_has_exact_inclusive_chunk_bounds() {
        let registry = synthetic_spawn_registry((
            DVec3::new(-100.0, 0.0, -100.0),
            DVec3::new(100.0, 100.0, 100.0),
        ));
        let anchors = [
            crate::player::spawn_selector::spawn_distribution_anchor_for_test(
                DVec3::new(-0.1, 65.0, -0.1),
                0.0,
            ),
        ];
        let chunks = fallback_spawn_chunk_union(&registry, &anchors)
            .expect("configured fallback union should fit");

        let (min, max) =
            ChunkView::new(ChunkPos::new(-1, -1), NEW_PLAYER_VIEW_DISTANCE_CHUNKS).bounding_box();
        assert!(chunks.contains(&min));
        assert!(chunks.contains(&max));
        assert!(!chunks.contains(&ChunkPos::new(min.x - 1, min.z)));
        assert!(!chunks.contains(&ChunkPos::new(max.x + 1, max.z)));
    }

    #[test]
    fn scatter_spawn_resources_places_matching_tree_leaves_and_surface_clusters() {
        let scenario = ScenarioSingleClient::new();
        let mut app = scenario.app;
        let mut layer = app
            .world_mut()
            .get_mut::<ChunkLayer>(scenario.layer)
            .expect("test layer should carry ChunkLayer");
        let anchors = [
            crate::player::spawn_selector::spawn_distribution_anchor_for_test(
                DVec3::new(-32.2, 65.0, -48.1),
                80.0,
            ),
            crate::player::spawn_selector::spawn_distribution_anchor_for_test(
                DVec3::new(96.8, 65.0, 64.9),
                80.0,
            ),
        ];
        for anchor in &anchors {
            let base = anchor.anchor();
            for chunk_x in
                (base.x.floor() as i32).div_euclid(16)..=(base.x.floor() as i32 + 41).div_euclid(16)
            {
                for chunk_z in (base.z.floor() as i32).div_euclid(16)
                    ..=(base.z.floor() as i32 + 31).div_euclid(16)
                {
                    layer.insert_chunk([chunk_x, chunk_z], UnloadedChunk::new());
                }
            }
        }
        layer.insert_chunk([0, 0], UnloadedChunk::new());

        super::scatter_spawn_resources(&mut layer, &anchors);

        for anchor in &anchors {
            let base = anchor.anchor();
            let x = base.x.floor() as i32;
            let z = base.z.floor() as i32;
            assert_eq!(
                block_state(&layer, x + 12, GRASS_Y + 1, z + 8),
                Some(BlockState::OAK_LOG)
            );
            assert_eq!(
                block_state(&layer, x + 12, GRASS_Y + 5, z + 8),
                Some(BlockState::OAK_LEAVES)
            );
            assert_eq!(
                block_state(&layer, x + 6, GRASS_Y + 1, z + 22),
                Some(BlockState::BIRCH_LOG)
            );
            assert_eq!(
                block_state(&layer, x + 6, GRASS_Y + 4, z + 22),
                Some(BlockState::BIRCH_LEAVES),
                "expected birch trunk to receive birch leaves, actual mismatched leaves"
            );
            assert_eq!(
                block_state(&layer, x + 30, GRASS_Y, z + 10),
                Some(BlockState::IRON_ORE)
            );
            assert_eq!(
                block_state(&layer, x + 31, GRASS_Y, z + 10),
                Some(BlockState::IRON_ORE)
            );
            assert_eq!(
                block_state(&layer, x + 30, GRASS_Y, z + 11),
                Some(BlockState::IRON_ORE)
            );
            assert_eq!(
                block_state(&layer, x + 18, GRASS_Y, z + 30),
                Some(BlockState::STONE)
            );
            assert_eq!(
                block_state(&layer, x + 19, GRASS_Y, z + 31),
                Some(BlockState::STONE)
            );
        }
        assert_eq!(
            block_state(&layer, 12, GRASS_Y + 1, 8),
            Some(BlockState::AIR),
            "资源必须随出生簇平移，不得继续固定写在世界原点附近"
        );
    }

    fn block_state(layer: &ChunkLayer, x: i32, y: i32, z: i32) -> Option<BlockState> {
        layer.block(BlockPos::new(x, y, z)).map(|block| block.state)
    }

    #[test]
    fn falls_back_when_anvil_missing() {
        let world_path = unique_temp_dir("bong-world-bootstrap-without-region");
        fs::create_dir_all(&world_path).expect("test world path should be creatable");

        let selection =
            select_world_bootstrap_from_configured_paths(None, Some(world_path.clone()));

        assert_eq!(
            selection,
            WorldBootstrap::FallbackFlat(FallbackFlatBootstrap {
                reason: FallbackFlatReason::RegionDirMissing(
                    world_path.join(ANVIL_REGION_DIR_NAME)
                ),
            })
        );
    }

    #[test]
    fn selects_fallback_without_world_path() {
        let selection = select_world_bootstrap_from_configured_paths(None, None);

        assert_eq!(
            selection,
            WorldBootstrap::FallbackFlat(FallbackFlatBootstrap {
                reason: FallbackFlatReason::NoWorldBootstrapConfigured,
            })
        );
    }

    #[test]
    fn selects_fallback_with_empty_region_dir() {
        let world_path = unique_temp_dir("bong-world-bootstrap-empty-region");
        let region_dir = world_path.join(ANVIL_REGION_DIR_NAME);
        fs::create_dir_all(&region_dir).expect("empty region dir should be creatable");

        let selection = select_world_bootstrap_from_configured_paths(None, Some(world_path));

        assert_eq!(
            selection,
            WorldBootstrap::FallbackFlat(FallbackFlatBootstrap {
                reason: FallbackFlatReason::RegionDirEmpty(region_dir),
            })
        );
    }

    #[test]
    fn uses_anvil_when_region_exists() {
        let world_path = unique_temp_dir("bong-world-bootstrap-anvil");
        let region_dir = world_path.join(ANVIL_REGION_DIR_NAME);
        fs::create_dir_all(&region_dir).expect("region dir should be creatable");
        fs::write(region_dir.join("r.0.0.mca"), b"placeholder")
            .expect("region marker file should be creatable");

        let selection =
            select_world_bootstrap_from_configured_paths(None, Some(world_path.clone()));

        assert_eq!(
            selection,
            WorldBootstrap::AnvilIfPresent(AnvilBootstrapConfig {
                world_path,
                region_dir,
            })
        );
    }

    #[test]
    fn falls_back_when_region_assets_invalid() {
        let world_path = unique_temp_dir("bong-world-bootstrap-invalid-region");
        let region_dir = world_path.join(ANVIL_REGION_DIR_NAME);
        fs::create_dir_all(&region_dir).expect("region dir should be creatable");
        fs::write(region_dir.join("notes.txt"), b"not an anvil region")
            .expect("invalid region marker should be creatable");

        let selection = select_world_bootstrap_from_configured_paths(None, Some(world_path));

        assert_eq!(
            selection,
            WorldBootstrap::FallbackFlat(FallbackFlatBootstrap {
                reason: FallbackFlatReason::RegionDirInvalid(region_dir),
            })
        );
    }

    #[test]
    fn uses_anvil_when_region_exists_via_env_selection() {
        let world_path = unique_temp_dir("bong-world-bootstrap-env-anvil");
        let region_dir = world_path.join(ANVIL_REGION_DIR_NAME);
        fs::create_dir_all(&region_dir).expect("region dir should be creatable");
        fs::write(region_dir.join("r.0.0.mca"), b"placeholder")
            .expect("region marker file should be creatable");

        let _lock = env_lock();
        let _world_guard = ScopedEnvVar::set(WORLD_PATH_ENV_VAR, Some(world_path.clone()));
        let _raster_guard = ScopedEnvVar::set(TERRAIN_RASTER_PATH_ENV_VAR, None);
        let selection = select_world_bootstrap();

        assert_eq!(
            selection,
            WorldBootstrap::AnvilIfPresent(AnvilBootstrapConfig {
                world_path,
                region_dir,
            })
        );
    }

    #[test]
    fn falls_back_when_anvil_missing_via_env_selection() {
        let world_path = unique_temp_dir("bong-world-bootstrap-env-missing");
        fs::create_dir_all(&world_path).expect("test world path should be creatable");

        let _lock = env_lock();
        let _world_guard = ScopedEnvVar::set(WORLD_PATH_ENV_VAR, Some(world_path.clone()));
        let _raster_guard = ScopedEnvVar::set(TERRAIN_RASTER_PATH_ENV_VAR, None);
        let selection = select_world_bootstrap();

        assert_eq!(
            selection,
            WorldBootstrap::FallbackFlat(FallbackFlatBootstrap {
                reason: FallbackFlatReason::RegionDirMissing(
                    world_path.join(ANVIL_REGION_DIR_NAME)
                ),
            })
        );
    }

    #[test]
    fn prefers_raster_manifest_when_configured() {
        let raster_dir = unique_temp_dir("bong-world-bootstrap-raster");
        fs::create_dir_all(&raster_dir).expect("raster dir should be creatable");
        let manifest_path = raster_dir.join("manifest.json");
        fs::write(&manifest_path, "{}\n").expect("manifest file should be creatable");

        let selection =
            select_world_bootstrap_from_configured_paths(Some(manifest_path.clone()), None);

        assert_eq!(
            selection,
            WorldBootstrap::TerrainRaster(RasterBootstrapConfig {
                manifest_path,
                raster_dir,
            })
        );
    }

    #[test]
    fn raster_path_wins_over_anvil_path() {
        let raster_dir = unique_temp_dir("bong-world-bootstrap-raster-priority");
        fs::create_dir_all(&raster_dir).expect("raster dir should be creatable");
        let manifest_path = raster_dir.join("manifest.json");
        fs::write(&manifest_path, "{}\n").expect("manifest file should be creatable");

        let world_path = unique_temp_dir("bong-world-bootstrap-priority-anvil");
        let region_dir = world_path.join(ANVIL_REGION_DIR_NAME);
        fs::create_dir_all(&region_dir).expect("region dir should be creatable");
        fs::write(region_dir.join("r.0.0.mca"), b"placeholder")
            .expect("region marker file should be creatable");

        let selection = select_world_bootstrap_from_configured_paths(
            Some(manifest_path.clone()),
            Some(world_path),
        );

        assert_eq!(
            selection,
            WorldBootstrap::TerrainRaster(RasterBootstrapConfig {
                manifest_path,
                raster_dir,
            })
        );
    }

    #[test]
    fn uses_raster_via_env_selection() {
        let raster_dir = unique_temp_dir("bong-world-bootstrap-env-raster");
        fs::create_dir_all(&raster_dir).expect("raster dir should be creatable");
        let manifest_path = raster_dir.join("manifest.json");
        fs::write(&manifest_path, "{}\n").expect("manifest file should be creatable");

        let _lock = env_lock();
        let _world_guard = ScopedEnvVar::set(WORLD_PATH_ENV_VAR, None);
        let _raster_guard =
            ScopedEnvVar::set(TERRAIN_RASTER_PATH_ENV_VAR, Some(manifest_path.clone()));
        let selection = select_world_bootstrap();

        assert_eq!(
            selection,
            WorldBootstrap::TerrainRaster(RasterBootstrapConfig {
                manifest_path,
                raster_dir,
            })
        );
    }

    struct ScopedEnvVar {
        key: &'static str,
        previous: Option<std::ffi::OsString>,
    }

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    impl ScopedEnvVar {
        fn set(key: &'static str, path: Option<PathBuf>) -> Self {
            let previous = std::env::var_os(key);

            if let Some(path) = path {
                std::env::set_var(key, path);
            } else {
                std::env::remove_var(key);
            }

            Self { key, previous }
        }
    }

    impl Drop for ScopedEnvVar {
        fn drop(&mut self) {
            if let Some(previous) = self.previous.take() {
                std::env::set_var(self.key, previous);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }

    fn missing_zones_path() -> PathBuf {
        unique_temp_path("bong-missing-zones", ".json")
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        unique_temp_path(prefix, "")
    }

    fn unique_temp_path(prefix: &str, suffix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("current time should be after unix epoch")
            .as_nanos();

        std::env::temp_dir().join(format!("{prefix}-{nanos}{suffix}"))
    }
}
