use valence::prelude::{BlockState, Chunk, UnloadedChunk};

use super::raster::ColumnSample;

const SEA_LEVEL: i32 = 64;

pub fn fill_column(
    chunk: &mut UnloadedChunk,
    local_x: u32,
    local_z: u32,
    min_y: i32,
    sample: &ColumnSample,
) -> i32 {
    let column = resolve_column(sample, min_y, chunk.height() as i32);
    let max_y = column.top_y.max(column.water_top).max(column.bedrock_y);

    for world_y in min_y..=max_y {
        let Some(local_y) = local_y(world_y, min_y, chunk.height() as i32) else {
            continue;
        };
        let block = block_at(world_y, &column);
        chunk.set_block_state(local_x, local_y, local_z, block);
    }

    column.top_y
}

struct ResolvedColumn {
    bedrock_y: i32,
    top_y: i32,
    water_top: i32,
    surface_block: BlockState,
    filler_block: BlockState,
    deep_block: BlockState,
    filler_depth: i32,
    carve_floor: Option<i32>,
    carve_ceiling: Option<i32>,
}

fn resolve_column(sample: &ColumnSample, min_y: i32, world_height: i32) -> ResolvedColumn {
    let bedrock_y = min_y;
    let mut top_y = clamp_world_y(sample.height.round() as i32, min_y, world_height);
    let water_top = if sample.water_level < 0.0 {
        -1
    } else {
        clamp_world_y(sample.water_level.round() as i32, min_y, world_height)
    };

    let mut surface_block = sample.surface_block;
    let mut filler_block = sample.subsurface_block;
    let mut deep_block = if top_y > 92 {
        BlockState::DEEPSLATE
    } else {
        BlockState::STONE
    };

    let mut filler_depth = 4;
    let mut carve_floor = None;
    let mut carve_ceiling = None;

    if sample.surface_block == BlockState::GRASS_BLOCK {
        filler_block = BlockState::DIRT;
        filler_depth = 5;
    } else if matches!(
        sample.surface_block,
        BlockState::MUD | BlockState::CLAY | BlockState::MOSS_BLOCK
    ) {
        filler_block = BlockState::MUD;
        filler_depth = 6;
    } else if matches!(
        sample.surface_block,
        BlockState::BLACKSTONE | BlockState::BASALT
    ) {
        filler_block = BlockState::BLACKSTONE;
        filler_depth = 6;
    } else if matches!(
        sample.surface_block,
        BlockState::SNOW_BLOCK | BlockState::PACKED_ICE
    ) {
        filler_block = BlockState::STONE;
        filler_depth = 3;
    } else if sample.surface_block == BlockState::SOUL_SAND {
        filler_block = BlockState::SOUL_SAND;
        filler_depth = 5;
    } else if matches!(
        sample.surface_block,
        BlockState::SAND | BlockState::RED_SANDSTONE | BlockState::TERRACOTTA
    ) {
        filler_block = sample.surface_block;
        filler_depth = 7;
    } else if sample.surface_block == BlockState::GRAVEL {
        filler_block = BlockState::GRAVEL;
        filler_depth = 5;
    }

    if sample.rift_axis_sdf < 0.9 {
        let carve_depth =
            ((1.0 - sample.rift_axis_sdf) * 22.0 + sample.rim_edge_mask * 4.0).round() as i32;
        top_y -= carve_depth;
        if sample.rift_axis_sdf < 0.42 {
            surface_block = BlockState::BLACKSTONE;
        }
        if sample.rift_axis_sdf < 0.65 {
            filler_block = BlockState::BASALT;
        }
        filler_depth = filler_depth.max(8);
    }

    if sample.fracture_mask > 0.7 {
        let crack_depth = ((sample.fracture_mask - 0.7) * 300.0) as i32;
        top_y = (bedrock_y + 6).max(top_y - crack_depth);
        surface_block = BlockState::MAGMA_BLOCK;
        filler_block = BlockState::BLACKSTONE;
        filler_depth = filler_depth.max(8);
    }

    if sample.neg_pressure > 0.18 {
        let sink = (sample.neg_pressure * 14.0).round() as i32;
        top_y -= sink;
        if sample.neg_pressure > 0.42 {
            filler_block = BlockState::GRAVEL;
        }
        if sample.ruin_density > 0.5 {
            surface_block = BlockState::GRAVEL;
        }
    }

    if sample.entrance_mask > 0.16 {
        top_y -= (sample.entrance_mask * 10.0).round() as i32;
    }

    if sample.cave_mask > 0.58 {
        carve_floor = Some((top_y - sample.ceiling_height.round() as i32).max(bedrock_y + 8));
        carve_ceiling = Some((top_y - 2).max(carve_floor.unwrap_or(bedrock_y + 12) + 4));
        deep_block = BlockState::DEEPSLATE;
        if sample.entrance_mask > 0.4 {
            surface_block = BlockState::GRAVEL;
        }
    }

    if sample.boundary_weight < 0.22 {
        filler_depth = filler_depth.max(3) - 1;
    }

    top_y = top_y.clamp(bedrock_y + 2, min_y + world_height - 2);

    ResolvedColumn {
        bedrock_y,
        top_y,
        water_top,
        surface_block,
        filler_block,
        deep_block,
        filler_depth,
        carve_floor,
        carve_ceiling,
    }
}

fn block_at(world_y: i32, column: &ResolvedColumn) -> BlockState {
    if world_y <= column.bedrock_y {
        return BlockState::BEDROCK;
    }

    if let (Some(carve_floor), Some(carve_ceiling)) = (column.carve_floor, column.carve_ceiling) {
        if world_y >= carve_floor && world_y <= carve_ceiling {
            if column.water_top >= 0 && world_y <= column.water_top && world_y > column.top_y {
                return BlockState::WATER;
            }
            return BlockState::AIR;
        }
    }

    if world_y > column.top_y {
        if column.water_top >= 0 && world_y <= column.water_top {
            return BlockState::WATER;
        }
        return BlockState::AIR;
    }

    if world_y == column.top_y {
        return column.surface_block;
    }

    if world_y >= column.top_y - column.filler_depth {
        return column.filler_block;
    }

    column.deep_block
}

fn clamp_world_y(world_y: i32, min_y: i32, world_height: i32) -> i32 {
    world_y.clamp(min_y, min_y + world_height - 1)
}

fn local_y(world_y: i32, min_y: i32, world_height: i32) -> Option<u32> {
    let y = world_y - min_y;
    if y < 0 || y >= world_height {
        None
    } else {
        Some(y as u32)
    }
}

#[allow(dead_code)]
pub fn sea_level() -> i32 {
    SEA_LEVEL
}
