use valence::prelude::{Chunk, UnloadedChunk};

use super::raster::TerrainProvider;

pub fn fill_chunk_biomes(
    chunk: &mut UnloadedChunk,
    chunk_x: i32,
    chunk_z: i32,
    world_height: u32,
    terrain: &TerrainProvider,
) {
    let biome_height = world_height / 4;

    for biome_y in 0..biome_height {
        for bz in 0..4 {
            for bx in 0..4 {
                let world_x = chunk_x * 16 + bx as i32 * 4 + 2;
                let world_z = chunk_z * 16 + bz as i32 * 4 + 2;
                let sample = terrain.sample(world_x, world_z);
                chunk.set_biome(bx, biome_y, bz, sample.biome);
            }
        }
    }
}
