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

pub(super) fn surface_y_for_sample(sample: &ColumnSample, min_y: i32, world_height: i32) -> i32 {
    resolve_column(sample, min_y, world_height).top_y
}

#[derive(Clone, Copy)]
pub(super) struct CaveDecorationSpan {
    pub carve_floor: i32,
    pub carve_ceiling: i32,
}

pub(super) fn cave_span_for_sample(
    sample: &ColumnSample,
    min_y: i32,
    world_height: i32,
) -> Option<CaveDecorationSpan> {
    let column = resolve_column(sample, min_y, world_height);
    Some(CaveDecorationSpan {
        carve_floor: column.carve_floor?,
        carve_ceiling: column.carve_ceiling?,
    })
}

struct ResolvedColumn {
    bedrock_y: i32,
    top_y: i32,
    water_top: i32,
    surface_block: BlockState,
    filler_block: BlockState,
    deep_block_bias: u32,
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
    let mut deep_block_bias = subsurface_hash(sample);

    let mut filler_depth = 4;
    let mut carve_floor = None;
    let mut carve_ceiling = None;

    if matches!(
        sample.surface_block,
        BlockState::GRASS_BLOCK | BlockState::PODZOL | BlockState::ROOTED_DIRT
    ) {
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
        if sample.fracture_mask > 0.88 {
            surface_block = BlockState::LAVA;
        } else if sample.surface_block != BlockState::CRIMSON_NYLIUM {
            surface_block = BlockState::MAGMA_BLOCK;
        }
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
        deep_block_bias = deep_block_bias.wrapping_add(37);
        if sample.entrance_mask > 0.4 {
            surface_block = BlockState::GRAVEL;
        }
    }

    if sample.boundary_weight < 0.22 {
        filler_depth = filler_depth.max(3) - 1;
    }

    if (0.1..0.9).contains(&sample.boundary_weight) {
        let transition = smoothstep(sample.boundary_weight);
        let min_depth = (filler_depth.max(3) - 1).max(2);
        let max_depth = filler_depth + 2;
        filler_depth = min_depth + ((max_depth - min_depth) as f32 * transition).round() as i32;
    }

    top_y = top_y.clamp(bedrock_y + 2, min_y + world_height - 2);

    ResolvedColumn {
        bedrock_y,
        top_y,
        water_top,
        surface_block,
        filler_block,
        deep_block_bias,
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

    // Deep fracture lava pool: extend LAVA 2 blocks below surface
    if column.surface_block == BlockState::LAVA && world_y >= column.top_y - 2 {
        return BlockState::LAVA;
    }

    if world_y >= column.top_y - column.filler_depth {
        return column.filler_block;
    }

    deep_block_at(world_y, column.bedrock_y, column.deep_block_bias)
}

fn deep_block_at(world_y: i32, bedrock_y: i32, deep_block_bias: u32) -> BlockState {
    if world_y <= bedrock_y {
        return BlockState::BEDROCK;
    }
    if world_y > 8 {
        return BlockState::STONE;
    }
    if world_y <= -32 {
        return BlockState::DEEPSLATE;
    }

    let threshold = ((world_y + 32) * 255 / 40) as u32;
    if deep_block_bias & 0xFF <= threshold {
        BlockState::DEEPSLATE
    } else {
        BlockState::STONE
    }
}

fn subsurface_hash(sample: &ColumnSample) -> u32 {
    let mut bits = sample.height.to_bits();
    bits ^= sample.water_level.to_bits().rotate_left(7);
    bits ^= sample.feature_mask.to_bits().rotate_left(13);
    bits ^= sample.boundary_weight.to_bits().rotate_left(19);
    bits ^= sample.rift_axis_sdf.to_bits().rotate_left(3);
    bits ^= sample.cave_mask.to_bits().rotate_left(11);
    bits ^= sample.neg_pressure.to_bits().rotate_left(23);
    bits ^= u32::from(sample.biome_id).wrapping_mul(0x9E37_79B9);
    bits ^= bits >> 16;
    bits = bits.wrapping_mul(0x7FEB_352D);
    bits ^= bits >> 15;
    bits = bits.wrapping_mul(0x846C_A68B);
    bits ^ (bits >> 16)
}

fn smoothstep(value: f32) -> f32 {
    let t = value.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn clamp_world_y(world_y: i32, min_y: i32, world_height: i32) -> i32 {
    // Valence 0.2x still encodes chunk heightmaps with a fixed 9-bit budget.
    // Reserving the top two local Y values avoids overflow when a motion-blocking
    // block reaches the absolute ceiling of a 512-high dimension.
    world_y.clamp(min_y, min_y + world_height - 3)
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

#[cfg(test)]
mod tests {
    use super::{deep_block_at, smoothstep, subsurface_hash};
    use crate::world::terrain::raster::ColumnSample;
    use valence::prelude::{BiomeId, BlockState};

    fn sample() -> ColumnSample {
        ColumnSample {
            height: 72.0,
            surface_block: BlockState::GRASS_BLOCK,
            subsurface_block: BlockState::DIRT,
            biome_id: 0,
            biome: BiomeId::default(),
            water_level: -1.0,
            feature_mask: 0.15,
            boundary_weight: 0.0,
            rift_axis_sdf: 99.0,
            rim_edge_mask: 0.0,
            cave_mask: 0.0,
            ceiling_height: 0.0,
            entrance_mask: 0.0,
            fracture_mask: 0.0,
            neg_pressure: 0.0,
            ruin_density: 0.0,
        }
    }

    #[test]
    fn deep_blocks_use_expected_absolute_layers() {
        let bias = 255;
        assert_eq!(deep_block_at(-64, -64, bias), BlockState::BEDROCK);
        assert_eq!(deep_block_at(9, -64, bias), BlockState::STONE);
        assert_eq!(deep_block_at(-32, -64, bias), BlockState::DEEPSLATE);
    }

    #[test]
    fn transition_band_mix_is_deterministic() {
        let sample = sample();
        let bias = subsurface_hash(&sample);
        assert_eq!(deep_block_at(-12, -64, bias), deep_block_at(-12, -64, bias));
    }

    #[test]
    fn smoothstep_respects_endpoints_and_midpoint() {
        assert_eq!(smoothstep(0.0), 0.0);
        assert_eq!(smoothstep(1.0), 1.0);
        assert!((smoothstep(0.5) - 0.5).abs() < f32::EPSILON);
    }
}
