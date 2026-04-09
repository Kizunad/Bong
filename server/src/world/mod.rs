pub mod events;
pub mod zone;

use std::fs;
use std::path::PathBuf;

use valence::anvil::AnvilLevel;
use valence::prelude::{
    ident, App, BiomeRegistry, BlockState, Commands, DimensionTypeRegistry, LayerBundle, Res,
    Server, Startup, UnloadedChunk,
};

const TEST_AREA_CHUNKS: i32 = 16;
const CHUNK_WIDTH: i32 = 16;
const BEDROCK_Y: i32 = 64;
const GRASS_Y: i32 = BEDROCK_Y + 1;
pub(crate) const TEST_AREA_BLOCK_EXTENT: i32 = TEST_AREA_CHUNKS * CHUNK_WIDTH;
const WORLD_PATH_ENV_VAR: &str = "BONG_WORLD_PATH";
const ANVIL_REGION_DIR_NAME: &str = "region";

#[derive(Debug, Clone, PartialEq, Eq)]
enum WorldBootstrap {
    FallbackFlat(FallbackFlatBootstrap),
    AnvilIfPresent(AnvilBootstrapConfig),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FallbackFlatBootstrap {
    reason: FallbackFlatReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FallbackFlatReason {
    NoWorldPathConfigured,
    WorldPathMissing(PathBuf),
    WorldPathNotDirectory(PathBuf),
    WorldPathUnreadable { world_path: PathBuf, error: String },
    RegionDirMissing(PathBuf),
    RegionDirEmpty(PathBuf),
    RegionDirInvalid(PathBuf),
    RegionDirUnreadable { region_dir: PathBuf, error: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AnvilBootstrapConfig {
    world_path: PathBuf,
    region_dir: PathBuf,
}

pub fn register(app: &mut App) {
    tracing::info!("[bong][world] registering world setup systems");
    zone::register(app);
    events::register(app);
    app.add_systems(Startup, setup_world);
}

fn setup_world(
    commands: Commands,
    server: Res<Server>,
    dimensions: Res<DimensionTypeRegistry>,
    biomes: Res<BiomeRegistry>,
) {
    match select_world_bootstrap() {
        WorldBootstrap::FallbackFlat(fallback) => {
            log_fallback_flat_selection(&fallback.reason);
            tracing::info!("[bong][world] starting fallback flat world bootstrap");
            spawn_fallback_flat_world(commands, server, dimensions, biomes);
        }
        WorldBootstrap::AnvilIfPresent(anvil) => {
            tracing::info!(
                "[bong][world] selected anvil bootstrap from {} with region dir {}, starting Anvil-backed world bootstrap",
                anvil.world_path.display(),
                anvil.region_dir.display()
            );
            spawn_anvil_world(commands, server, dimensions, biomes, anvil);
        }
    }
}

fn select_world_bootstrap() -> WorldBootstrap {
    select_world_bootstrap_from_configured_path(configured_world_path())
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

fn select_world_bootstrap_from_configured_path(world_path: Option<PathBuf>) -> WorldBootstrap {
    let Some(world_path) = world_path else {
        return fallback_flat(FallbackFlatReason::NoWorldPathConfigured);
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
        FallbackFlatReason::NoWorldPathConfigured => {
            tracing::info!(
                "[bong][world] no world path configured via {}, selecting fallback flat bootstrap",
                WORLD_PATH_ENV_VAR
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
    mut commands: Commands,
    server: Res<Server>,
    dimensions: Res<DimensionTypeRegistry>,
    biomes: Res<BiomeRegistry>,
    anvil: AnvilBootstrapConfig,
) {
    tracing::info!(
        "[bong][world] creating overworld layer backed by Anvil terrain at {}",
        anvil.world_path.display()
    );

    let layer = LayerBundle::new(ident!("overworld"), &dimensions, &biomes, &server);
    let anvil_level = AnvilLevel::new(&anvil.world_path, &biomes);

    commands.spawn((layer, anvil_level));
}

fn spawn_fallback_flat_world(
    mut commands: Commands,
    server: Res<Server>,
    dimensions: Res<DimensionTypeRegistry>,
    biomes: Res<BiomeRegistry>,
) {
    tracing::info!("[bong][world] creating overworld test area (16x16 chunks)");

    let mut layer = LayerBundle::new(ident!("overworld"), &dimensions, &biomes, &server);

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

    commands.spawn(layer);
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::zone::{default_spawn_bounds, ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};
    use super::{
        select_world_bootstrap, select_world_bootstrap_from_configured_path, AnvilBootstrapConfig,
        FallbackFlatBootstrap, FallbackFlatReason, WorldBootstrap, ANVIL_REGION_DIR_NAME,
        WORLD_PATH_ENV_VAR,
    };
    use valence::prelude::DVec3;

    #[test]
    fn fallback_spawn_zone_exists() {
        let registry = ZoneRegistry::fallback();
        let spawn_zone = registry
            .find_zone(DVec3::new(8.0, 66.0, 8.0))
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
            .find_zone(DVec3::new(8.0, 66.0, 8.0))
            .expect("missing zones.json should fall back to the spawn zone");

        println!(
            "missing zones config at {} -> using fallback zone {}",
            missing_path.display(),
            spawn_zone.name
        );

        assert_eq!(registry.zones.len(), 1);
        assert_eq!(spawn_zone.name, DEFAULT_SPAWN_ZONE_NAME);
        assert_eq!(spawn_zone.bounds, default_spawn_bounds());
    }

    #[test]
    fn falls_back_when_anvil_missing() {
        let world_path = unique_temp_dir("bong-world-bootstrap-without-region");
        fs::create_dir_all(&world_path).expect("test world path should be creatable");

        let selection = select_world_bootstrap_from_configured_path(Some(world_path.clone()));

        println!(
            "configured world path {} without region dir -> {:?}",
            world_path.display(),
            selection
        );

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
        let selection = select_world_bootstrap_from_configured_path(None);

        assert_eq!(
            selection,
            WorldBootstrap::FallbackFlat(FallbackFlatBootstrap {
                reason: FallbackFlatReason::NoWorldPathConfigured,
            })
        );
    }

    #[test]
    fn selects_fallback_with_empty_region_dir() {
        let world_path = unique_temp_dir("bong-world-bootstrap-empty-region");
        let region_dir = world_path.join(ANVIL_REGION_DIR_NAME);
        fs::create_dir_all(&region_dir).expect("empty region dir should be creatable");

        let selection = select_world_bootstrap_from_configured_path(Some(world_path));

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

        let selection = select_world_bootstrap_from_configured_path(Some(world_path.clone()));

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

        let selection = select_world_bootstrap_from_configured_path(Some(world_path));

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

        let _guard = ScopedWorldPathEnvVar::set(Some(world_path.clone()));
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

        let _guard = ScopedWorldPathEnvVar::set(Some(world_path.clone()));
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

    struct ScopedWorldPathEnvVar {
        previous: Option<std::ffi::OsString>,
    }

    impl ScopedWorldPathEnvVar {
        fn set(path: Option<PathBuf>) -> Self {
            let previous = std::env::var_os(WORLD_PATH_ENV_VAR);

            if let Some(path) = path {
                std::env::set_var(WORLD_PATH_ENV_VAR, path);
            } else {
                std::env::remove_var(WORLD_PATH_ENV_VAR);
            }

            Self { previous }
        }
    }

    impl Drop for ScopedWorldPathEnvVar {
        fn drop(&mut self) {
            if let Some(previous) = self.previous.take() {
                std::env::set_var(WORLD_PATH_ENV_VAR, previous);
            } else {
                std::env::remove_var(WORLD_PATH_ENV_VAR);
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
