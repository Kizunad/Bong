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
    ident, App, BiomeRegistry, BlockState, Chunk, ChunkLayer, ChunkPos, Client, Commands,
    DimensionTypeRegistry, Entity, Query, Res, ResMut, Resource, Server, UnloadedChunk, Update,
    View, VisibleChunkLayer, With,
};

use crate::mineral::{MineralOreIndex, MineralOreNode};
use crate::world::dimension::{DimensionKind, DimensionLayers, OverworldLayer};

#[allow(unused_imports)]
pub use raster::{raster_dir_from_manifest_path, FossilBbox, TerrainProvider, TerrainProviders};

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
    mineral_index: Option<Res<MineralOreIndex>>,
    mineral_nodes: Query<&MineralOreNode>,
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
            ensure_chunk_generated(
                &mut layer,
                pos,
                terrain,
                &mut generated.loaded,
                mineral_index.as_deref(),
                &mineral_nodes,
            );
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
    mineral_index: Option<&MineralOreIndex>,
    mineral_nodes: &Query<&MineralOreNode>,
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
    overlay_mineral_ores(&mut chunk, pos, min_y, mineral_index, mineral_nodes);
    biome::fill_chunk_biomes(&mut chunk, pos.x, pos.z, WORLD_HEIGHT, terrain);
    layer.insert_chunk(pos, chunk);
    generated.insert(pos);
}

fn overlay_mineral_ores(
    chunk: &mut UnloadedChunk,
    pos: ChunkPos,
    min_y: i32,
    mineral_index: Option<&MineralOreIndex>,
    mineral_nodes: &Query<&MineralOreNode>,
) {
    let Some(mineral_index) = mineral_index else {
        return;
    };

    for (dimension, block_pos, entity) in mineral_index.iter() {
        if dimension != DimensionKind::Overworld {
            continue;
        }
        if block_pos.x.div_euclid(16) != pos.x || block_pos.z.div_euclid(16) != pos.z {
            continue;
        }
        let Ok(node) = mineral_nodes.get(entity) else {
            continue;
        };
        set_mineral_block(chunk, block_pos, min_y, node.mineral_id);
    }
}

fn set_mineral_block(
    chunk: &mut UnloadedChunk,
    block_pos: valence::prelude::BlockPos,
    min_y: i32,
    mineral_id: crate::mineral::MineralId,
) {
    let local_y = block_pos.y - min_y;
    if !(0..WORLD_HEIGHT as i32).contains(&local_y) {
        return;
    }
    let local_x = block_pos.x.rem_euclid(16) as u32;
    let local_z = block_pos.z.rem_euclid(16) as u32;
    chunk.set_block_state(
        local_x,
        local_y as u32,
        local_z,
        mineral_block_state(mineral_id),
    );
}

fn mineral_block_state(mineral_id: crate::mineral::MineralId) -> BlockState {
    match mineral_id.vanilla_block() {
        "iron_ore" => BlockState::IRON_ORE,
        "deepslate_iron_ore" => BlockState::DEEPSLATE_IRON_ORE,
        "copper_ore" => BlockState::COPPER_ORE,
        "redstone_ore" => BlockState::REDSTONE_ORE,
        "ancient_debris" => BlockState::ANCIENT_DEBRIS,
        "obsidian" => BlockState::OBSIDIAN,
        "gold_ore" => BlockState::GOLD_ORE,
        "emerald_ore" => BlockState::EMERALD_ORE,
        "lapis_ore" => BlockState::LAPIS_ORE,
        "coal_ore" => BlockState::COAL_ORE,
        "nether_gold_ore" => BlockState::NETHER_GOLD_ORE,
        "nether_quartz_ore" => BlockState::NETHER_QUARTZ_ORE,
        "diamond_ore" => BlockState::DIAMOND_ORE,
        _ => BlockState::STONE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mineral::MineralId;
    use valence::prelude::BlockPos;

    #[test]
    fn set_mineral_block_writes_matching_vanilla_block() {
        let pos = BlockPos::new(3, -12, 5);
        let mut chunk = UnloadedChunk::with_height(WORLD_HEIGHT);

        set_mineral_block(&mut chunk, pos, MIN_Y, MineralId::ZaGang);

        assert_eq!(
            chunk.block_state(3, (pos.y - MIN_Y) as u32, 5),
            BlockState::COPPER_ORE
        );
    }
}
