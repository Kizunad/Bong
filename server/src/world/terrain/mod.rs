mod biome;
mod blocks;
mod column;
mod decoration;
mod flora;
mod mega_tree;
mod noise;
mod raster;
mod spatial;
mod structures;
mod wilderness;

use std::collections::HashSet;
use std::path::PathBuf;

use valence::prelude::{
    ident, App, BiomeRegistry, BlockState, ChunkLayer, ChunkPos, Client, Commands,
    DimensionTypeRegistry, Entity, Query, Res, ResMut, Resource, Server, UnloadedChunk, Update,
    View, VisibleChunkLayer, With,
};

use crate::world::dimension::{DimensionLayers, OverworldLayer};

pub use raster::{raster_dir_from_manifest_path, TerrainProvider, TerrainProviders};

const WORLD_HEIGHT: u32 = 512;
pub const MIN_Y: i32 = -64;

/// Surface information for a single world column, used by NPC navigation.
#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub struct SurfaceInfo {
    /// The Y coordinate of the top solid block.
    pub y: i32,
    /// Whether an NPC can stand on this column (no deep water or lava).
    pub passable: bool,
}

/// Trait for querying terrain surface height and walkability.
///
/// Implemented by [`TerrainProvider`] for production use.  Tests can supply
/// lightweight mocks (flat plane, slope, cliff, etc.) without touching raster
/// files.
pub trait SurfaceProvider {
    fn query_surface(&self, world_x: i32, world_z: i32) -> SurfaceInfo;
}

impl SurfaceProvider for TerrainProvider {
    fn query_surface(&self, world_x: i32, world_z: i32) -> SurfaceInfo {
        let sample = self.sample(world_x, world_z);
        let y = column::surface_y_for_sample(&sample, MIN_Y, WORLD_HEIGHT as i32);
        let water_top = if sample.water_level < 0.0 {
            MIN_Y - 1
        } else {
            sample.water_level.round() as i32
        };
        let passable = water_top <= y && sample.surface_block != BlockState::LAVA;
        SurfaceInfo { y, passable }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RasterBootstrapConfig {
    pub manifest_path: PathBuf,
    pub raster_dir: PathBuf,
}

#[derive(Default)]
struct GeneratedChunks {
    loaded: HashSet<ChunkPos>,
}

impl Resource for GeneratedChunks {}

pub fn register(app: &mut App) {
    app.insert_resource(GeneratedChunks::default())
        .add_systems(Update, generate_chunks_around_players)
        .add_systems(Update, remove_unviewed_chunks);
}

pub fn spawn_raster_world(
    commands: &mut Commands,
    server: &Server,
    dimensions: &mut DimensionTypeRegistry,
    biomes: &BiomeRegistry,
    config: RasterBootstrapConfig,
) -> Entity {
    let provider = TerrainProvider::load(&config.manifest_path, &config.raster_dir, biomes)
        .unwrap_or_else(|error| panic!("failed to bootstrap raster terrain: {error}"));
    tracing::info!(
        "[bong][world] loaded {} terrain tiles / {} POIs / {} decorations from {}",
        provider.tile_count(),
        provider.pois().len(),
        provider.decoration_count(),
        config.manifest_path.display()
    );

    if let Some((_, _, dim)) = dimensions
        .iter_mut()
        .find(|(_, name, _)| *name == ident!("overworld").as_str_ident())
    {
        dim.height = WORLD_HEIGHT as i32;
        dim.logical_height = WORLD_HEIGHT as i32;
    }

    let layer = valence::prelude::LayerBundle::new(ident!("overworld"), dimensions, biomes, server);
    let entity = commands.spawn((layer, OverworldLayer)).id();

    // plan-tsy-worldgen-v1 §6.1 — optional TSY raster manifest from
    // BONG_TSY_RASTER_PATH; absent → tsy=None (legacy behaviour).
    let tsy_provider = load_tsy_provider_from_env(biomes);

    commands.insert_resource(TerrainProviders {
        overworld: provider,
        tsy: tsy_provider,
    });
    entity
}

const TSY_RASTER_PATH_ENV_VAR: &str = "BONG_TSY_RASTER_PATH";

fn load_tsy_provider_from_env(biomes: &BiomeRegistry) -> Option<TerrainProvider> {
    let raw = std::env::var_os(TSY_RASTER_PATH_ENV_VAR)?;
    if raw.is_empty() {
        return None;
    }
    let manifest_path = PathBuf::from(raw);
    let raster_dir = match raster_dir_from_manifest_path(&manifest_path) {
        Ok(path) => path,
        Err(error) => {
            tracing::warn!(
                "[bong][world] BONG_TSY_RASTER_PATH={} unreadable: {error}",
                manifest_path.display()
            );
            return None;
        }
    };
    match TerrainProvider::load(&manifest_path, &raster_dir, biomes) {
        Ok(provider) => {
            tracing::info!(
                "[bong][world] loaded TSY {} terrain tiles / {} POIs from {}",
                provider.tile_count(),
                provider.pois().len(),
                manifest_path.display()
            );
            Some(provider)
        }
        Err(error) => {
            tracing::warn!(
                "[bong][world] failed to load TSY raster {}: {error}",
                manifest_path.display()
            );
            None
        }
    }
}

fn generate_chunks_around_players(
    mut layers: Query<&mut ChunkLayer>,
    clients: Query<(View, &VisibleChunkLayer), With<Client>>,
    providers: Option<Res<TerrainProviders>>,
    dimension_layers: Option<Res<DimensionLayers>>,
    mut generated: ResMut<GeneratedChunks>,
) {
    let Some(providers) = providers else {
        return;
    };
    let terrain = &providers.overworld;
    let generated = generated.as_mut();

    // For now we only generate raster-backed chunks for the overworld layer.
    // TSY chunk routing arrives with `plan-tsy-worldgen-v1`.
    let Some(dimension_layers) = dimension_layers else {
        return;
    };
    let overworld_layer_entity = dimension_layers.overworld;

    let Ok(mut layer) = layers.get_mut(overworld_layer_entity) else {
        return;
    };

    for (view, visible_chunk_layer) in &clients {
        if visible_chunk_layer.0 != overworld_layer_entity {
            continue;
        }
        for pos in view.get().iter() {
            ensure_chunk_generated(&mut layer, pos, terrain, &mut generated.loaded);
        }
    }
}

fn remove_unviewed_chunks(
    mut layers: Query<&mut ChunkLayer>,
    providers: Option<Res<TerrainProviders>>,
    dimension_layers: Option<Res<DimensionLayers>>,
    mut generated: ResMut<GeneratedChunks>,
) {
    if providers.is_none() {
        return;
    }
    let Some(dimension_layers) = dimension_layers else {
        return;
    };
    let generated = generated.as_mut();

    let Ok(mut layer) = layers.get_mut(dimension_layers.overworld) else {
        return;
    };

    generated.loaded.retain(|pos| layer.chunk(*pos).is_some());

    let mut removed = Vec::new();
    layer.retain_chunks(|pos, chunk| {
        let keep = chunk.viewer_count_mut() > 0;
        if !keep {
            removed.push(pos);
        }
        keep
    });

    for pos in removed {
        generated.loaded.remove(&pos);
    }
}

fn ensure_chunk_generated(
    layer: &mut ChunkLayer,
    pos: ChunkPos,
    terrain: &TerrainProvider,
    generated: &mut HashSet<ChunkPos>,
) {
    if generated.contains(&pos) || layer.chunk(pos).is_some() {
        return;
    }

    let min_y = layer.min_y();
    let mut chunk = UnloadedChunk::with_height(WORLD_HEIGHT);
    let mut top_y_by_column = [[min_y; 16]; 16];
    for local_z in 0..16 {
        for local_x in 0..16 {
            let world_x = pos.x * 16 + local_x;
            let world_z = pos.z * 16 + local_z;
            let sample = terrain.sample(world_x, world_z);
            top_y_by_column[local_z as usize][local_x as usize] =
                column::fill_column(&mut chunk, local_x as u32, local_z as u32, min_y, &sample);
        }
    }

    decoration::decorate_chunk(&mut chunk, pos, min_y, terrain, &top_y_by_column);
    flora::decorate_chunk(&mut chunk, pos, min_y, terrain, &top_y_by_column);
    structures::decorate_chunk(&mut chunk, pos, min_y, terrain);
    biome::fill_chunk_biomes(&mut chunk, pos.x, pos.z, WORLD_HEIGHT, terrain);
    layer.insert_chunk(pos, chunk);
    generated.insert(pos);
}
