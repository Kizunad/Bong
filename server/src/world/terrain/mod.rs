mod biome;
mod column;
mod decoration;
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
    DimensionTypeRegistry, Query, Res, ResMut, Resource, Server, UnloadedChunk, Update, View, With,
};

pub use raster::{raster_dir_from_manifest_path, TerrainProvider};

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
    mut commands: Commands,
    server: Res<Server>,
    mut dimensions: ResMut<DimensionTypeRegistry>,
    biomes: Res<BiomeRegistry>,
    config: RasterBootstrapConfig,
) {
    let provider = TerrainProvider::load(&config.manifest_path, &config.raster_dir, &biomes)
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

    let layer =
        valence::prelude::LayerBundle::new(ident!("overworld"), &dimensions, &biomes, &server);
    commands.spawn(layer);
    commands.insert_resource(provider);
}

fn generate_chunks_around_players(
    mut layers: Query<&mut ChunkLayer>,
    clients: Query<View, With<Client>>,
    terrain: Option<Res<TerrainProvider>>,
    mut generated: ResMut<GeneratedChunks>,
) {
    let Some(terrain) = terrain else {
        return;
    };
    let terrain = terrain.into_inner();
    let generated = generated.as_mut();

    let mut layer = match layers.get_single_mut() {
        Ok(layer) => layer,
        Err(_) => return,
    };

    for view in &clients {
        for pos in view.get().iter() {
            ensure_chunk_generated(&mut layer, pos, terrain, &mut generated.loaded);
        }
    }
}

fn remove_unviewed_chunks(
    mut layers: Query<&mut ChunkLayer>,
    terrain: Option<Res<TerrainProvider>>,
    mut generated: ResMut<GeneratedChunks>,
) {
    if terrain.is_none() {
        return;
    }
    let generated = generated.as_mut();

    let Ok(mut layer) = layers.get_single_mut() else {
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
    structures::decorate_chunk(&mut chunk, pos, min_y, terrain);
    biome::fill_chunk_biomes(&mut chunk, pos.x, pos.z, WORLD_HEIGHT, terrain);
    layer.insert_chunk(pos, chunk);
    generated.insert(pos);
}
