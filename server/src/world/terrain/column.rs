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
    let sky_top_y = column.sky_island.map(|span| span.top_y).unwrap_or(min_y);
    let max_y = column
        .top_y
        .max(column.water_top)
        .max(column.bedrock_y)
        .max(sky_top_y);

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

/// Walkable surface from the sample's spans alone (no block styling). This is
/// the authoritative §8.1 #2 surface = ceiling of the surface span, clamped to
/// the world Y budget. `resolve_column().top_y` agrees with this; this helper
/// is the cheap path for callers that only need the Y.
#[allow(dead_code)]
pub(super) fn span_surface_y(sample: &ColumnSample, min_y: i32, world_height: i32) -> i32 {
    clamp_world_y(sample.surface_y(), min_y, world_height)
}

#[derive(Clone, Copy)]
pub(super) struct SkyIslandSpan {
    pub bottom_y: i32,
    pub top_y: i32,
}

pub(super) fn sky_island_span_for_sample(
    sample: &ColumnSample,
    min_y: i32,
    world_height: i32,
) -> Option<SkyIslandSpan> {
    // worldgen-v4 P0 §8.1 #1/#12: the floating isle is now a span above the
    // surface span (the Python shim appends it when sky_island_mask>=0.2 &&
    // base_y<9000 && thickness>=4). The sky_island_mask gate still applies so a
    // stray high span without mask never reads as an isle.
    if sample.sky_island_mask < 0.2 {
        return None;
    }
    let (base_y, top_raw) = sample.sky_island_span()?;
    let bottom_y = clamp_world_y(base_y, min_y, world_height);
    let top_y = clamp_world_y(top_raw, min_y, world_height);
    if top_y <= bottom_y {
        return None;
    }

    Some(SkyIslandSpan { bottom_y, top_y })
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
    sky_island: Option<SkyIslandSpan>,
}

fn resolve_column(sample: &ColumnSample, min_y: i32, world_height: i32) -> ResolvedColumn {
    let bedrock_y = min_y;
    // worldgen-v4 P0 §8.1 #1/#2: the walkable surface is the surface span's
    // ceiling. The spans are now the single source of vertical truth, so the
    // old height-mutating deformations (rift / fracture / neg_pressure / entrance
    // sinks) no longer move the surface — those layers only restyle the surface
    // blocks below. This is what makes query_surface() agree with the Python
    // golden (surface == round(height)) instead of double-carving.
    let top_y = clamp_world_y(sample.surface_y(), min_y, world_height);
    let water_top = if sample.water_level < 0.0 {
        -1
    } else {
        clamp_world_y(sample.water_level.round() as i32, min_y, world_height)
    };

    let mut surface_block = sample.surface_block;
    let mut filler_block = sample.subsurface_block;
    let mut deep_block_bias = subsurface_hash(sample);

    let mut filler_depth = 4;
    // worldgen-v4 P0 §8.1 #1: the cave void is the carved gap between the
    // surface span and a floor remnant below it, derived straight from spans.
    let (carve_floor, carve_ceiling) = match sample.cave_carve() {
        Some((floor, ceiling)) => {
            let floor = clamp_world_y(floor, min_y, world_height).max(bedrock_y + 1);
            let ceiling = clamp_world_y(ceiling, min_y, world_height);
            if ceiling >= floor {
                deep_block_bias = deep_block_bias.wrapping_add(37);
                (Some(floor), Some(ceiling))
            } else {
                (None, None)
            }
        }
        None => (None, None),
    };
    let sky_island = sky_island_span_for_sample(sample, min_y, world_height);

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

    // worldgen-v4 P0 §8.1 #1/#2: the rift / fracture / neg_pressure semantic
    // layers are RETAINED, but their old terrain-depth mutations (`top_y -=`)
    // are gone — the span already encodes the final surface height (the Python
    // `height` for these profiles already shapes the valley / scar). These
    // blocks now only restyle the surface/filler material, never move the
    // walkable top, so the rendered surface stays byte-equal to the span ceiling
    // (== round(height), the frozen v3 golden).
    if sample.rift_axis_sdf < 0.9 {
        if sample.rift_axis_sdf < 0.42 {
            surface_block = BlockState::BLACKSTONE;
        }
        if sample.rift_axis_sdf < 0.65 {
            filler_block = BlockState::BASALT;
        }
        filler_depth = filler_depth.max(8);
    }

    if sample.fracture_mask > 0.7 {
        if sample.fracture_mask > 0.88 {
            surface_block = BlockState::LAVA;
        } else if sample.surface_block != BlockState::CRIMSON_NYLIUM {
            surface_block = BlockState::MAGMA_BLOCK;
        }
        filler_block = BlockState::BLACKSTONE;
        filler_depth = filler_depth.max(8);
    }

    if sample.neg_pressure > 0.18 {
        if sample.neg_pressure > 0.42 {
            filler_block = BlockState::GRAVEL;
        }
        if sample.ruin_density > 0.5 {
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

    let top_y = top_y.clamp(bedrock_y + 2, min_y + world_height - 2);

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
        sky_island,
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

    if let Some(span) = column.sky_island {
        if world_y >= span.bottom_y && world_y <= span.top_y {
            return sky_island_block_at(world_y, span, column);
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

fn sky_island_block_at(world_y: i32, span: SkyIslandSpan, column: &ResolvedColumn) -> BlockState {
    if world_y == span.top_y {
        return column.surface_block;
    }

    if world_y >= span.top_y - column.filler_depth.min(4) {
        return column.filler_block;
    }

    if world_y <= span.bottom_y + 1 && column.deep_block_bias.is_multiple_of(3) {
        return BlockState::CALCITE;
    }

    BlockState::STONE
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
    // worldgen-v4 P0 §8.1 #1: height / cave_mask are gone; derive the deep-block
    // bias from the span surface Y and the carved-cave flag instead so the deep
    // stone/deepslate mix stays deterministic per column.
    let mut bits = (sample.surface_y() as f32).to_bits();
    bits ^= sample.water_level.to_bits().rotate_left(7);
    bits ^= sample.feature_mask.to_bits().rotate_left(13);
    bits ^= sample.boundary_weight.to_bits().rotate_left(19);
    bits ^= sample.rift_axis_sdf.to_bits().rotate_left(3);
    bits ^= u32::from(sample.has_carved_cave()).rotate_left(11);
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
    // block reaches the absolute ceiling of the runtime dimension.
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
    use super::{deep_block_at, sky_island_span_for_sample, smoothstep, subsurface_hash};
    use crate::world::terrain::raster::ColumnSample;
    use valence::prelude::{BiomeId, BlockState};

    fn sample() -> ColumnSample {
        ColumnSample {
            // Single solid span: bedrock (-64) → surface 72 (replaces height=72).
            spans: smallvec::smallvec![(-64, 72)],
            surface_block: BlockState::GRASS_BLOCK,
            subsurface_block: BlockState::DIRT,
            biome_id: 0,
            biome: BiomeId::default(),
            water_level: -1.0,
            feature_mask: 0.15,
            boundary_weight: 0.0,
            rift_axis_sdf: 99.0,
            portal_anchor_sdf: 999.0,
            rim_edge_mask: 0.0,
            fracture_mask: 0.0,
            neg_pressure: 0.0,
            ruin_density: 0.0,
            qi_density: 0.12,
            mofa_decay: 0.40,
            qi_vein_flow: 0.0,
            spirit_eye_candidates: 0,
            realm_collapse_mask: 0,
            sky_island_mask: 0.0,
            underground_tier: 0,
            flora_density: 0.0,
            flora_variant_id: 0,
            ground_cover_density: 0.0,
            ground_cover_id: 0,
            zongmen_origin_id: 0,
            mineral_density: 0.0,
            mineral_kind: 0,
            fossil_bbox: 0,
            anomaly_intensity: 0.0,
            anomaly_kind: 0,
            tsy_presence: 0,
            tsy_origin_id: 0,
            tsy_depth_tier: 0,
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

    #[test]
    fn sky_island_span_uses_manifest_vertical_layers() {
        // worldgen-v4 P0: the isle is a span above the surface span (base 260,
        // top 272) gated by sky_island_mask. surface span (-64..72) stays span[0].
        let mut sample = sample();
        sample.sky_island_mask = 0.5;
        sample.spans = smallvec::smallvec![(-64, 72), (260, 272)];

        let span = sky_island_span_for_sample(&sample, -64, 512).unwrap();
        assert_eq!(
            span.bottom_y, 260,
            "isle bottom should be the high span floor"
        );
        assert_eq!(span.top_y, 272, "isle top should be the high span ceiling");
    }

    #[test]
    fn sky_island_span_requires_mask_gate() {
        // A high span with no sky_island_mask must NOT read as an isle (the mask
        // is the §8.1 #12 semantic gate retained alongside spans).
        let mut sample = sample();
        sample.sky_island_mask = 0.0;
        sample.spans = smallvec::smallvec![(-64, 72), (260, 272)];
        assert!(
            sky_island_span_for_sample(&sample, -64, 512).is_none(),
            "no mask → no isle even with a high span present"
        );
    }

    #[test]
    fn cave_carve_recovered_from_span_gap() {
        // surface cap (70..74) over a floor remnant (-64..40): the void between
        // them is the carve (41..69). Mirrors the Python shim's cave folding.
        let mut sample = sample();
        sample.spans = smallvec::smallvec![(70, 74), (-64, 40)];
        let cave = super::cave_span_for_sample(&sample, -64, 512).expect("carved cave span");
        assert_eq!(
            (cave.carve_floor, cave.carve_ceiling),
            (41, 69),
            "carve void = (remnant_ceiling+1, surface_floor-1); expected (41,69) \
             from remnant ceiling 40 and surface floor 70"
        );
    }
}
