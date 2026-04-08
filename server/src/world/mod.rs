pub mod zone;

pub use zone::ZoneRegistry;

use std::{
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
};

use valence::anvil::AnvilLevel;
use valence::prelude::{
    ident, App, BiomeRegistry, BlockState, Commands, DimensionTypeRegistry, Entity, LayerBundle,
    Res, Resource, Server, Startup, UnloadedChunk,
};

const TEST_AREA_CHUNKS: i32 = 16;
const CHUNK_WIDTH: i32 = 16;
const BEDROCK_Y: i32 = 64;
const GRASS_Y: i32 = BEDROCK_Y + 1;
const DEFAULT_ZONE_MAX_Y: f64 = 255.0;
const TEST_AREA_BLOCK_WIDTH: i32 = TEST_AREA_CHUNKS * CHUNK_WIDTH;
const DEFAULT_WORLD_PATH: &str = "world";
const WORLD_REGION_DIR: &str = "region";

pub const DEFAULT_SPAWN_ZONE: &str = "spawn";
pub const DEFAULT_SPAWN_POSITION: [f64; 3] = [8.0, 66.0, 8.0];
pub const DEFAULT_SPAWN_BOUNDS_MIN: [f64; 3] = [0.0, BEDROCK_Y as f64, 0.0];
pub const DEFAULT_SPAWN_BOUNDS_MAX: [f64; 3] = [
    TEST_AREA_BLOCK_WIDTH as f64,
    DEFAULT_ZONE_MAX_Y,
    TEST_AREA_BLOCK_WIDTH as f64,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldBootstrapMode {
    FallbackFlat,
    AnvilIfPresent,
}

impl WorldBootstrapMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::FallbackFlat => "FallbackFlat",
            Self::AnvilIfPresent => "AnvilIfPresent",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldBootstrapFallbackReason {
    ConfiguredFallbackFlat,
    MissingWorldPath,
    MissingRegionDirectory,
    MissingReadableRegionFile,
    RegionProbeFailed,
    AnvilRegionDetected,
}

impl WorldBootstrapFallbackReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::ConfiguredFallbackFlat => "configured fallback flat mode",
            Self::MissingWorldPath => "world path missing or not a directory",
            Self::MissingRegionDirectory => "region directory missing",
            Self::MissingReadableRegionFile => "region directory has no readable .mca files",
            Self::RegionProbeFailed => "failed to probe region directory for readable .mca file",
            Self::AnvilRegionDetected => "readable anvil region file detected",
        }
    }

    fn should_warn(self) -> bool {
        matches!(
            self,
            Self::MissingWorldPath
                | Self::MissingRegionDirectory
                | Self::MissingReadableRegionFile
                | Self::RegionProbeFailed
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldBootstrapConfig {
    pub preferred_mode: WorldBootstrapMode,
    pub world_path: PathBuf,
}

impl Default for WorldBootstrapConfig {
    fn default() -> Self {
        Self {
            preferred_mode: WorldBootstrapMode::AnvilIfPresent,
            world_path: PathBuf::from(DEFAULT_WORLD_PATH),
        }
    }
}

impl Resource for WorldBootstrapConfig {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldBootstrapState {
    pub preferred_mode: WorldBootstrapMode,
    pub selected_mode: WorldBootstrapMode,
    pub world_path: PathBuf,
    pub region_dir: PathBuf,
    pub fallback_reason: WorldBootstrapFallbackReason,
}

impl Resource for WorldBootstrapState {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveWorldLayer(pub Entity);

impl Resource for ActiveWorldLayer {}

pub fn register(app: &mut App) {
    tracing::info!("[bong][world] registering world setup system");
    app.init_resource::<WorldBootstrapConfig>();
    app.insert_resource(ZoneRegistry::from_optional_zones(None));
    app.add_systems(Startup, bootstrap_world);
}

fn bootstrap_world(
    mut commands: Commands,
    server: Res<Server>,
    dimensions: Res<DimensionTypeRegistry>,
    biomes: Res<BiomeRegistry>,
    bootstrap_config: Res<WorldBootstrapConfig>,
) {
    let bootstrap_state = resolve_world_bootstrap(bootstrap_config.as_ref());

    log_world_bootstrap(&bootstrap_state);

    let layer_entity = match bootstrap_state.selected_mode {
        WorldBootstrapMode::FallbackFlat => {
            let layer = spawn_fallback_flat_world(&server, &dimensions, &biomes);
            commands.spawn(layer).id()
        }
        WorldBootstrapMode::AnvilIfPresent => {
            let (layer, level) =
                spawn_anvil_world(&bootstrap_state.world_path, &server, &dimensions, &biomes);
            commands.spawn((layer, level)).id()
        }
    };

    commands.insert_resource(bootstrap_state);
    commands.insert_resource(ActiveWorldLayer(layer_entity));
}

fn log_world_bootstrap(bootstrap_state: &WorldBootstrapState) {
    if bootstrap_state.fallback_reason.should_warn() {
        tracing::warn!(
            "[bong][world] fallback bootstrap: using {} because {} (preferred: {}, world_path: {}, region_dir: {})",
            bootstrap_state.selected_mode.as_str(),
            bootstrap_state.fallback_reason.as_str(),
            bootstrap_state.preferred_mode.as_str(),
            bootstrap_state.world_path.display(),
            bootstrap_state.region_dir.display(),
        );
    }

    tracing::info!(
        "[bong][world] selected bootstrap mode: {} (preferred: {}, world_path: {}, region_dir: {}, reason: {})",
        bootstrap_state.selected_mode.as_str(),
        bootstrap_state.preferred_mode.as_str(),
        bootstrap_state.world_path.display(),
        bootstrap_state.region_dir.display(),
        bootstrap_state.fallback_reason.as_str(),
    );
}

fn resolve_world_bootstrap(config: &WorldBootstrapConfig) -> WorldBootstrapState {
    let region_dir = config.world_path.join(WORLD_REGION_DIR);
    let (selected_mode, fallback_reason) = match config.preferred_mode {
        WorldBootstrapMode::FallbackFlat => (
            WorldBootstrapMode::FallbackFlat,
            WorldBootstrapFallbackReason::ConfiguredFallbackFlat,
        ),
        WorldBootstrapMode::AnvilIfPresent if !config.world_path.is_dir() => (
            WorldBootstrapMode::FallbackFlat,
            WorldBootstrapFallbackReason::MissingWorldPath,
        ),
        WorldBootstrapMode::AnvilIfPresent if !region_dir.is_dir() => (
            WorldBootstrapMode::FallbackFlat,
            WorldBootstrapFallbackReason::MissingRegionDirectory,
        ),
        WorldBootstrapMode::AnvilIfPresent => match probe_region_for_anvil(&region_dir) {
            Ok(Some(_)) => (
                WorldBootstrapMode::AnvilIfPresent,
                WorldBootstrapFallbackReason::AnvilRegionDetected,
            ),
            Ok(None) => (
                WorldBootstrapMode::FallbackFlat,
                WorldBootstrapFallbackReason::MissingReadableRegionFile,
            ),
            Err(error) => {
                tracing::warn!(
                    "[bong][world] failed probing anvil region files in {}: {error}",
                    region_dir.display()
                );
                (
                    WorldBootstrapMode::FallbackFlat,
                    WorldBootstrapFallbackReason::RegionProbeFailed,
                )
            }
        },
    };

    WorldBootstrapState {
        preferred_mode: config.preferred_mode,
        selected_mode,
        world_path: config.world_path.clone(),
        region_dir,
        fallback_reason,
    }
}

fn probe_region_for_anvil(region_dir: &Path) -> io::Result<Option<PathBuf>> {
    let mut candidates = vec![];

    for entry in fs::read_dir(region_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension() == Some(OsStr::new("mca")) {
            candidates.push(path);
        }
    }

    candidates.sort();

    let mut first_error: Option<io::Error> = None;

    for candidate in candidates {
        match is_readable_region_file(&candidate) {
            Ok(true) => return Ok(Some(candidate)),
            Ok(false) => continue,
            Err(error) => {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
    }

    if let Some(error) = first_error {
        Err(error)
    } else {
        Ok(None)
    }
}

fn is_readable_region_file(path: &Path) -> io::Result<bool> {
    let metadata = fs::metadata(path)?;

    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("anvil region candidate is not a file: {}", path.display()),
        ));
    }

    fs::File::open(path)?;
    Ok(true)
}

fn spawn_anvil_world(
    world_path: &Path,
    server: &Server,
    dimensions: &DimensionTypeRegistry,
    biomes: &BiomeRegistry,
) -> (LayerBundle, AnvilLevel) {
    tracing::info!(
        "[bong][world] creating anvil-backed world from {}",
        world_path.display()
    );

    let layer = LayerBundle::new(ident!("overworld"), dimensions, biomes, server);
    let level = AnvilLevel::new(world_path, biomes);

    (layer, level)
}

fn spawn_fallback_flat_world(
    server: &Server,
    dimensions: &DimensionTypeRegistry,
    biomes: &BiomeRegistry,
) -> LayerBundle {
    tracing::info!("[bong][world] creating overworld test area (16x16 chunks)");

    let mut layer = LayerBundle::new(ident!("overworld"), dimensions, biomes, server);

    for chunk_z in 0..TEST_AREA_CHUNKS {
        for chunk_x in 0..TEST_AREA_CHUNKS {
            layer
                .chunk
                .insert_chunk([chunk_x, chunk_z], UnloadedChunk::new());
        }
    }

    for z in 0..TEST_AREA_BLOCK_WIDTH {
        for x in 0..TEST_AREA_BLOCK_WIDTH {
            layer
                .chunk
                .set_block([x, BEDROCK_Y, z], BlockState::BEDROCK);
            layer
                .chunk
                .set_block([x, GRASS_Y, z], BlockState::GRASS_BLOCK);
        }
    }

    layer
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use valence::prelude::DVec3;

    struct TempWorldDir {
        path: PathBuf,
    }

    impl TempWorldDir {
        fn new(test_name: &str) -> Self {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "bong-world-bootstrap-{test_name}-{}-{timestamp}",
                std::process::id()
            ));

            fs::create_dir_all(&path).expect("temporary world directory should be creatable");

            Self { path }
        }
    }

    impl Drop for TempWorldDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn fallback_spawn_zone_exists() {
        let registry = ZoneRegistry::from_optional_zones(None);
        let spawn = registry
            .get_zone(DEFAULT_SPAWN_ZONE)
            .expect("fallback registry should include the spawn zone");

        assert_eq!(registry.default_zone().name, DEFAULT_SPAWN_ZONE);
        assert_eq!(spawn.name, DEFAULT_SPAWN_ZONE);
        assert!(spawn.bounds.contains(DVec3::new(
            DEFAULT_SPAWN_POSITION[0],
            DEFAULT_SPAWN_POSITION[1],
            DEFAULT_SPAWN_POSITION[2],
        )));
    }

    #[test]
    fn missing_zone_config_uses_spawn_fallback() {
        let registry = ZoneRegistry::from_optional_zones(None);
        let spawn = registry.find_zone_or_default(DVec3::new(
            DEFAULT_SPAWN_POSITION[0],
            DEFAULT_SPAWN_POSITION[1],
            DEFAULT_SPAWN_POSITION[2],
        ));

        assert_eq!(spawn.name, DEFAULT_SPAWN_ZONE);
        assert_eq!(spawn.spawn_position, DEFAULT_SPAWN_POSITION);
        assert_eq!(
            spawn.bounds,
            zone::ZoneAabb::new(DEFAULT_SPAWN_BOUNDS_MIN, DEFAULT_SPAWN_BOUNDS_MAX)
        );
    }

    #[test]
    fn selects_fallback_without_region_dir() {
        let world_dir = TempWorldDir::new("missing-region");
        let config = WorldBootstrapConfig {
            preferred_mode: WorldBootstrapMode::AnvilIfPresent,
            world_path: world_dir.path.clone(),
        };

        let bootstrap = resolve_world_bootstrap(&config);

        assert_eq!(bootstrap.preferred_mode, WorldBootstrapMode::AnvilIfPresent);
        assert_eq!(bootstrap.selected_mode, WorldBootstrapMode::FallbackFlat);
        assert_eq!(
            bootstrap.fallback_reason,
            WorldBootstrapFallbackReason::MissingRegionDirectory
        );
        assert_eq!(bootstrap.region_dir, world_dir.path.join(WORLD_REGION_DIR));
    }

    #[test]
    fn anvil_path_is_selected_when_region_file_exists() {
        let world_dir = TempWorldDir::new("anvil-ready");
        let region_dir = world_dir.path.join(WORLD_REGION_DIR);
        fs::create_dir_all(&region_dir).expect("region directory should be creatable");

        let region_file = region_dir.join("r.0.0.mca");
        fs::write(&region_file, [])
            .expect("region test file should be writable and readable for probe");

        let config = WorldBootstrapConfig {
            preferred_mode: WorldBootstrapMode::AnvilIfPresent,
            world_path: world_dir.path.clone(),
        };

        let bootstrap = resolve_world_bootstrap(&config);

        assert_eq!(bootstrap.selected_mode, WorldBootstrapMode::AnvilIfPresent);
        assert_eq!(
            bootstrap.fallback_reason,
            WorldBootstrapFallbackReason::AnvilRegionDetected
        );
    }

    #[test]
    fn anvil_path_falls_back_on_error() {
        let world_dir = TempWorldDir::new("anvil-probe-error");
        let region_dir = world_dir.path.join(WORLD_REGION_DIR);

        fs::create_dir_all(region_dir.join("r.0.0.mca"))
            .expect("directory candidate should be creatable");

        let config = WorldBootstrapConfig {
            preferred_mode: WorldBootstrapMode::AnvilIfPresent,
            world_path: world_dir.path.clone(),
        };

        let bootstrap = resolve_world_bootstrap(&config);

        assert_eq!(bootstrap.selected_mode, WorldBootstrapMode::FallbackFlat);
        assert_eq!(
            bootstrap.fallback_reason,
            WorldBootstrapFallbackReason::RegionProbeFailed
        );
    }

    #[test]
    fn selects_fallback_without_world_path() {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let world_path = std::env::temp_dir().join(format!(
            "bong-world-bootstrap-missing-path-{}-{timestamp}",
            std::process::id()
        ));
        let config = WorldBootstrapConfig {
            preferred_mode: WorldBootstrapMode::AnvilIfPresent,
            world_path: world_path.clone(),
        };

        let bootstrap = resolve_world_bootstrap(&config);

        assert_eq!(bootstrap.selected_mode, WorldBootstrapMode::FallbackFlat);
        assert_eq!(
            bootstrap.fallback_reason,
            WorldBootstrapFallbackReason::MissingWorldPath
        );
        assert_eq!(bootstrap.world_path, world_path);
        assert_eq!(
            bootstrap.region_dir,
            bootstrap.world_path.join(WORLD_REGION_DIR)
        );
    }

    #[test]
    fn flat_path_still_boots() {
        let config = WorldBootstrapConfig {
            preferred_mode: WorldBootstrapMode::FallbackFlat,
            world_path: PathBuf::from("world"),
        };

        let bootstrap = resolve_world_bootstrap(&config);

        assert_eq!(bootstrap.preferred_mode, WorldBootstrapMode::FallbackFlat);
        assert_eq!(bootstrap.selected_mode, WorldBootstrapMode::FallbackFlat);
        assert_eq!(
            bootstrap.fallback_reason,
            WorldBootstrapFallbackReason::ConfiguredFallbackFlat
        );
    }

    #[test]
    fn missing_world_inputs_require_fallback_warning() {
        assert!(WorldBootstrapFallbackReason::MissingWorldPath.should_warn());
        assert!(WorldBootstrapFallbackReason::MissingRegionDirectory.should_warn());
        assert!(WorldBootstrapFallbackReason::MissingReadableRegionFile.should_warn());
        assert!(WorldBootstrapFallbackReason::RegionProbeFailed.should_warn());
        assert!(!WorldBootstrapFallbackReason::ConfiguredFallbackFlat.should_warn());
        assert!(!WorldBootstrapFallbackReason::AnvilRegionDetected.should_warn());
    }
}
