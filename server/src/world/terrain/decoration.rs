use valence::prelude::{BlockState, Chunk, ChunkPos, PropName, PropValue, UnloadedChunk};

use super::raster::TerrainProvider;

pub fn decorate_chunk(
    chunk: &mut UnloadedChunk,
    pos: ChunkPos,
    min_y: i32,
    terrain: &TerrainProvider,
    top_y_by_column: &[[i32; 16]; 16],
) {
    for local_z in 0..16 {
        for local_x in 0..16 {
            let world_x = pos.x * 16 + local_x as i32;
            let world_z = pos.z * 16 + local_z as i32;
            let sample = terrain.sample(world_x, world_z);
            let top_y = top_y_by_column[local_z][local_x];
            let plant_y = top_y + 1;

            if !can_place_above_surface(chunk, local_x as u32, plant_y, local_z as u32, min_y) {
                continue;
            }

            let density = decoration_hash(world_x, world_z, 17) % 1000;
            match sample.biome_id {
                0 => place_wilderness_vegetation(chunk, local_x, local_z, plant_y, density),
                1 => place_peaks_vegetation(chunk, local_x, local_z, plant_y, top_y, density),
                2 => place_marsh_vegetation(
                    chunk, local_x, local_z, plant_y, top_y, density, &sample,
                ),
                3 => {}
                4 => place_spawn_vegetation(
                    chunk, local_x, local_z, plant_y, top_y, density, world_x, world_z, min_y,
                ),
                5 => {}
                6 => place_wastes_vegetation(chunk, local_x, local_z, plant_y, density),
                _ => {}
            }
        }
    }
}

fn place_wilderness_vegetation(
    chunk: &mut UnloadedChunk,
    local_x: usize,
    local_z: usize,
    plant_y: i32,
    density: u32,
) {
    if density < 28 {
        set_block(chunk, local_x, plant_y, local_z, BlockState::FERN);
    } else if density < 55 {
        set_block(chunk, local_x, plant_y, local_z, BlockState::GRASS);
    }
}

fn place_peaks_vegetation(
    chunk: &mut UnloadedChunk,
    local_x: usize,
    local_z: usize,
    plant_y: i32,
    top_y: i32,
    density: u32,
) {
    if top_y < 200 && density < 20 {
        set_block(chunk, local_x, plant_y, local_z, BlockState::FERN);
    }
}

fn place_marsh_vegetation(
    chunk: &mut UnloadedChunk,
    local_x: usize,
    local_z: usize,
    plant_y: i32,
    top_y: i32,
    density: u32,
    sample: &super::raster::ColumnSample,
) {
    if sample.water_level >= 0.0 && top_y < sample.water_level.round() as i32 && density < 36 {
        set_block(
            chunk,
            local_x,
            sample.water_level.round() as i32 + 1,
            local_z,
            BlockState::LILY_PAD,
        );
    } else if density < 80 {
        set_block(chunk, local_x, plant_y, local_z, BlockState::GRASS);
    } else if density < 120 {
        set_block(chunk, local_x, plant_y, local_z, BlockState::FERN);
    }
}

fn place_spawn_vegetation(
    chunk: &mut UnloadedChunk,
    local_x: usize,
    local_z: usize,
    plant_y: i32,
    top_y: i32,
    density: u32,
    world_x: i32,
    world_z: i32,
    min_y: i32,
) {
    if density < 30 {
        set_block(chunk, local_x, plant_y, local_z, BlockState::DANDELION);
    } else if density < 60 {
        set_block(chunk, local_x, plant_y, local_z, BlockState::POPPY);
    } else if density < 110 {
        set_block(chunk, local_x, plant_y, local_z, BlockState::GRASS);
    } else if density < 132 {
        set_block(chunk, local_x, plant_y, local_z, BlockState::FERN);
    } else if density < 144 {
        place_simple_oak(
            chunk,
            local_x,
            local_z,
            top_y + 1,
            min_y,
            decoration_hash(world_x, world_z, 91),
        );
    }
}

fn place_wastes_vegetation(
    chunk: &mut UnloadedChunk,
    local_x: usize,
    local_z: usize,
    plant_y: i32,
    density: u32,
) {
    if density < 8 {
        set_block(chunk, local_x, plant_y, local_z, BlockState::DEAD_BUSH);
    }
}

fn place_simple_oak(
    chunk: &mut UnloadedChunk,
    local_x: usize,
    local_z: usize,
    trunk_base_y: i32,
    min_y: i32,
    hash: u32,
) {
    let trunk_height = 4 + (hash % 3) as i32;
    let top_y = trunk_base_y + trunk_height;

    for y in trunk_base_y..top_y {
        set_block(chunk, local_x, y, local_z, BlockState::OAK_LOG);
    }

    for canopy_y in (top_y - 2)..=top_y {
        for dz in -2_i32..=2_i32 {
            for dx in -2_i32..=2_i32 {
                if dx.abs() == 2 && dz.abs() == 2 {
                    continue;
                }
                let x = local_x as i32 + dx;
                let z = local_z as i32 + dz;
                if !(0..16).contains(&x) || !(0..16).contains(&z) {
                    continue;
                }
                let local_y = canopy_y - min_y;
                if local_y < 0 || local_y >= chunk.height() as i32 {
                    continue;
                }
                if chunk
                    .block_state(x as u32, local_y as u32, z as u32)
                    .is_air()
                {
                    chunk.set_block_state(
                        x as u32,
                        local_y as u32,
                        z as u32,
                        BlockState::OAK_LEAVES,
                    );
                }
            }
        }
    }
}

fn can_place_above_surface(
    chunk: &UnloadedChunk,
    local_x: u32,
    world_y: i32,
    local_z: u32,
    min_y: i32,
) -> bool {
    let local_y = world_y - min_y;
    if local_y < 0 || local_y >= chunk.height() as i32 {
        return false;
    }
    chunk.block_state(local_x, local_y as u32, local_z).is_air()
}

fn set_block(
    chunk: &mut UnloadedChunk,
    local_x: usize,
    world_y: i32,
    local_z: usize,
    block: BlockState,
) {
    let min_y = -64;
    let local_y = world_y - min_y;
    if !(0..16).contains(&(local_x as i32)) || !(0..16).contains(&(local_z as i32)) {
        return;
    }
    if local_y < 0 || local_y >= chunk.height() as i32 {
        return;
    }
    chunk.set_block_state(local_x as u32, local_y as u32, local_z as u32, block);
}

fn decoration_hash(world_x: i32, world_z: i32, salt: u32) -> u32 {
    let mut value = world_x as u32;
    value ^= (world_z as u32).wrapping_mul(0x9E37_79B9);
    value ^= salt.wrapping_mul(0x85EB_CA6B);
    value ^= value >> 16;
    value = value.wrapping_mul(0x7FEB_352D);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846C_A68B);
    value ^= value >> 16;
    value
}

#[allow(dead_code)]
fn tall_grass_lower() -> BlockState {
    BlockState::TALL_GRASS.set(PropName::Half, PropValue::Lower)
}
