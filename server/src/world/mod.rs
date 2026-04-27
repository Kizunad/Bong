pub mod dimension;
pub mod dimension_transfer;
pub mod events;
pub mod extract_system;
pub mod rift_portal;
pub mod terrain;
pub mod tsy;
pub mod tsy_dev_command;
pub mod tsy_drain;
pub mod tsy_filter;
#[cfg(test)]
mod tsy_integration_test;
pub mod tsy_lifecycle;
#[cfg(test)]
mod tsy_lifecycle_integration_test;
pub mod tsy_poi_consumer;
pub mod tsy_portal;
pub mod zone;

use std::fs;
use std::path::PathBuf;

use valence::anvil::AnvilLevel;
use valence::prelude::{
    ident, App, BiomeRegistry, BlockState, Commands, DimensionTypeRegistry, Entity,
    IntoSystemConfigs, LayerBundle, Res, ResMut, Server, Startup, UnloadedChunk, Update,
};

use self::dimension::{DimensionLayers, OverworldLayer, TsyLayer};

use crate::combat::CombatSystemSet;

const TEST_AREA_CHUNKS: i32 = 16;
const CHUNK_WIDTH: i32 = 16;
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
    zone::register(app);
    events::register(app);
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
    // plan-tsy-zone-v1 §3.1 — `!tsy-spawn` 调试命令的事件消费器
    tsy_dev_command::register(app);
    // plan-tsy-worldgen-v1 §1 — startup 期消费 TerrainProviders.pois() 把 POI 转 marker
    tsy_poi_consumer::register(app);
    app.insert_resource(rift_portal::load_tsy_portals());
    // plan-tsy-lifecycle-v1 §1 — TSY 生命周期状态机 + 塌缩清理 + 道伥转化
    tsy_lifecycle::register(app);
    // plan-tsy-extract-v1 — TSY 定点撤离倒计时 + race-out 裂口。
    extract_system::register(app);
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
    tracing::info!("[bong][world] creating overworld test area (16x16 chunks)");

    let mut layer = LayerBundle::new(ident!("overworld"), dimensions, biomes, server);

    for chunk_z in 0..TEST_AREA_CHUNKS {
        for chunk_x in 0..TEST_AREA_CHUNKS {
            layer
                .chunk
                .insert_chunk([chunk_x, chunk_z], UnloadedChunk::new());
        }
    }

    for z in 0..TEST_AREA_BLOCK_EXTENT {
        for x in 0..TEST_AREA_BLOCK_EXTENT {
            layer
                .chunk
                .set_block([x, BEDROCK_Y, z], BlockState::BEDROCK);
            layer
                .chunk
                .set_block([x, GRASS_Y, z], BlockState::GRASS_BLOCK);
        }
    }

    commands.spawn((layer, OverworldLayer)).id()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::zone::{default_spawn_bounds, ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};
    use super::{
        select_world_bootstrap, select_world_bootstrap_from_configured_paths,
        terrain::RasterBootstrapConfig, AnvilBootstrapConfig, FallbackFlatBootstrap,
        FallbackFlatReason, WorldBootstrap, ANVIL_REGION_DIR_NAME, TERRAIN_RASTER_PATH_ENV_VAR,
        WORLD_PATH_ENV_VAR,
    };
    use valence::prelude::DVec3;

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

        let _guard = ScopedEnvVar::set(WORLD_PATH_ENV_VAR, Some(world_path.clone()));
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

        let _guard = ScopedEnvVar::set(WORLD_PATH_ENV_VAR, Some(world_path.clone()));
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

        let _guard = ScopedEnvVar::set(TERRAIN_RASTER_PATH_ENV_VAR, Some(manifest_path.clone()));
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
