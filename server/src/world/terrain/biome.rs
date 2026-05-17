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

    // Sword sea override: force stony_peaks biome for dark grey sky
    let sword_sea_biome = if super::giant_sword::is_in_sword_sea(chunk_x * 16 + 8, chunk_z * 16 + 8)
    {
        terrain.biome_palette.get(1).copied()
    } else {
        None
    };

    for biome_y in 0..biome_height {
        for bz in 0..4 {
            for bx in 0..4 {
                let world_x = chunk_x * 16 + bx as i32 * 4 + 2;
                let world_z = chunk_z * 16 + bz as i32 * 4 + 2;
                let biome = if let Some(override_biome) = sword_sea_biome {
                    if super::giant_sword::is_in_sword_sea(world_x, world_z) {
                        override_biome
                    } else {
                        terrain.sample(world_x, world_z).biome
                    }
                } else {
                    terrain.sample(world_x, world_z).biome
                };
                chunk.set_biome(bx, biome_y, bz, biome);
            }
        }
    }
}
