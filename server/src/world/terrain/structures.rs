use std::collections::HashMap;

use valence::prelude::{BlockState, Chunk, ChunkPos, PropName, PropValue, UnloadedChunk};

use super::{column, raster::TerrainProvider, spatial::ChunkBounds};

const RUIN_PILLAR_PROFILE: RuinPillarProfile = RuinPillarProfile {
    biome_matches: super::raster::ColumnSample::is_wastes_biome,
    seed_spacing: 72,
    offset_margin: 12,
    max_extent: 12,
    min_surface_y: 68,
    max_surface_y: 118,
    max_slope: 7,
    min_ruin_density: 0.6,
    min_neg_pressure: 0.18,
    chance_base: 0.22,
    chance_ruin_scale: 0.55,
    chance_feature_scale: 0.18,
    chance_boundary_penalty: 0.18,
};

const BROKEN_ALTAR_PROFILE: BrokenAltarProfile = BrokenAltarProfile {
    biome_matches: super::raster::ColumnSample::is_wastes_biome,
    seed_spacing: 96,
    offset_margin: 16,
    max_extent: 16,
    min_surface_y: 70,
    max_surface_y: 116,
    max_slope: 6,
    min_neg_pressure: 0.5,
    min_ruin_density: 0.42,
    chance_base: 0.12,
    chance_neg_pressure_scale: 0.72,
    chance_ruin_scale: 0.18,
    chance_boundary_penalty: 0.22,
};

const SPIRIT_ORE_PROFILE: SpiritOreProfile = SpiritOreProfile {
    biome_matches: super::raster::ColumnSample::is_peaks_biome,
    seed_spacing: 88,
    offset_margin: 14,
    max_extent: 14,
    min_surface_y: 180,
    max_surface_y: 360,
    max_slope: 10,
    min_feature_mask: 0.8,
    chance_base: 0.16,
    chance_feature_scale: 0.52,
    chance_boundary_penalty: 0.20,
};

const BONE_PILE_PROFILE: BonePileProfile = BonePileProfile {
    biome_matches: super::raster::ColumnSample::is_wastes_biome,
    seed_spacing: 48,
    offset_margin: 8,
    max_extent: 8,
    min_surface_y: 64,
    max_surface_y: 130,
    max_slope: 8,
    min_ruin_density: 0.3,
    chance_base: 0.28,
    chance_ruin_scale: 0.45,
    chance_boundary_penalty: 0.15,
};

// Spawn portal: at most one per 2000-block grid cell. Because the spawn zone
// is well under 2000 blocks across, this yields exactly 0 or 1 portal per
// spawn biome in practice. It is NOT a hard uniqueness guarantee — if the
// spawn biome ever spans multiple cells, duplicates are possible.
const SPAWN_PORTAL_PROFILE: SpawnPortalProfile = SpawnPortalProfile {
    biome_matches: super::raster::ColumnSample::is_spawn_biome,
    seed_spacing: 2000,
    offset_margin: 200,
    max_extent: 12,
    min_surface_y: 60,
    max_surface_y: 200,
    max_slope: 5,
    chance_base: 1.0,
    chance_feature_scale: 0.0,
    chance_boundary_penalty: 0.0,
};

const RIFT_BRIDGE_PROFILE: RiftBridgeProfile = RiftBridgeProfile {
    biome_matches: super::raster::ColumnSample::is_rift_biome,
    seed_spacing: 104,
    offset_margin: 20,
    max_extent: 18,
    min_surface_y: 36,
    max_surface_y: 128,
    max_slope: 12,
    rift_axis_min: 0.86,
    rift_axis_max: 1.14,
    chance_base: 0.18,
    chance_feature_scale: 0.34,
    chance_rim_scale: 0.28,
    chance_boundary_penalty: 0.14,
};

pub(super) fn decorate_chunk(
    chunk: &mut UnloadedChunk,
    pos: ChunkPos,
    min_y: i32,
    terrain: &TerrainProvider,
) {
    let bounds = ChunkBounds::from_chunk_pos(pos);
    let world_height = chunk.height() as i32;

    let cell_min_x = (bounds.min_x - RUIN_PILLAR_PROFILE.max_extent)
        .div_euclid(RUIN_PILLAR_PROFILE.seed_spacing);
    let cell_max_x = (bounds.max_x + RUIN_PILLAR_PROFILE.max_extent)
        .div_euclid(RUIN_PILLAR_PROFILE.seed_spacing);
    let cell_min_z = (bounds.min_z - RUIN_PILLAR_PROFILE.max_extent)
        .div_euclid(RUIN_PILLAR_PROFILE.seed_spacing);
    let cell_max_z = (bounds.max_z + RUIN_PILLAR_PROFILE.max_extent)
        .div_euclid(RUIN_PILLAR_PROFILE.seed_spacing);

    for cell_z in cell_min_z..=cell_max_z {
        for cell_x in cell_min_x..=cell_max_x {
            let Some(instance) =
                instantiate_ruin_pillar(cell_x, cell_z, min_y, world_height, terrain)
            else {
                continue;
            };
            if !instance.bounds.intersects_chunk(&bounds) {
                continue;
            }
            place_ruin_pillar_in_chunk(chunk, min_y, &bounds, &instance);
        }
    }

    let altar_min_x = (bounds.min_x - BROKEN_ALTAR_PROFILE.max_extent)
        .div_euclid(BROKEN_ALTAR_PROFILE.seed_spacing);
    let altar_max_x = (bounds.max_x + BROKEN_ALTAR_PROFILE.max_extent)
        .div_euclid(BROKEN_ALTAR_PROFILE.seed_spacing);
    let altar_min_z = (bounds.min_z - BROKEN_ALTAR_PROFILE.max_extent)
        .div_euclid(BROKEN_ALTAR_PROFILE.seed_spacing);
    let altar_max_z = (bounds.max_z + BROKEN_ALTAR_PROFILE.max_extent)
        .div_euclid(BROKEN_ALTAR_PROFILE.seed_spacing);

    for cell_z in altar_min_z..=altar_max_z {
        for cell_x in altar_min_x..=altar_max_x {
            let Some(instance) =
                instantiate_broken_altar(cell_x, cell_z, min_y, world_height, terrain)
            else {
                continue;
            };
            if !instance.bounds.intersects_chunk(&bounds) {
                continue;
            }
            place_broken_altar_in_chunk(chunk, min_y, &bounds, &instance);
        }
    }

    let ore_min_x =
        (bounds.min_x - SPIRIT_ORE_PROFILE.max_extent).div_euclid(SPIRIT_ORE_PROFILE.seed_spacing);
    let ore_max_x =
        (bounds.max_x + SPIRIT_ORE_PROFILE.max_extent).div_euclid(SPIRIT_ORE_PROFILE.seed_spacing);
    let ore_min_z =
        (bounds.min_z - SPIRIT_ORE_PROFILE.max_extent).div_euclid(SPIRIT_ORE_PROFILE.seed_spacing);
    let ore_max_z =
        (bounds.max_z + SPIRIT_ORE_PROFILE.max_extent).div_euclid(SPIRIT_ORE_PROFILE.seed_spacing);

    for cell_z in ore_min_z..=ore_max_z {
        for cell_x in ore_min_x..=ore_max_x {
            let Some(instance) =
                instantiate_spirit_ore_vein(cell_x, cell_z, min_y, world_height, terrain)
            else {
                continue;
            };
            if !instance.bounds.intersects_chunk(&bounds) {
                continue;
            }
            place_spirit_ore_vein_in_chunk(chunk, min_y, &bounds, &instance);
        }
    }

    let bridge_min_x = (bounds.min_x - RIFT_BRIDGE_PROFILE.max_extent)
        .div_euclid(RIFT_BRIDGE_PROFILE.seed_spacing);
    let bridge_max_x = (bounds.max_x + RIFT_BRIDGE_PROFILE.max_extent)
        .div_euclid(RIFT_BRIDGE_PROFILE.seed_spacing);
    let bridge_min_z = (bounds.min_z - RIFT_BRIDGE_PROFILE.max_extent)
        .div_euclid(RIFT_BRIDGE_PROFILE.seed_spacing);
    let bridge_max_z = (bounds.max_z + RIFT_BRIDGE_PROFILE.max_extent)
        .div_euclid(RIFT_BRIDGE_PROFILE.seed_spacing);

    for cell_z in bridge_min_z..=bridge_max_z {
        for cell_x in bridge_min_x..=bridge_max_x {
            let Some(instance) =
                instantiate_rift_bridge_remnant(cell_x, cell_z, min_y, world_height, terrain)
            else {
                continue;
            };
            if !instance.bounds.intersects_chunk(&bounds) {
                continue;
            }
            place_rift_bridge_remnant_in_chunk(chunk, min_y, &bounds, &instance);
        }
    }

    let bone_min_x =
        (bounds.min_x - BONE_PILE_PROFILE.max_extent).div_euclid(BONE_PILE_PROFILE.seed_spacing);
    let bone_max_x =
        (bounds.max_x + BONE_PILE_PROFILE.max_extent).div_euclid(BONE_PILE_PROFILE.seed_spacing);
    let bone_min_z =
        (bounds.min_z - BONE_PILE_PROFILE.max_extent).div_euclid(BONE_PILE_PROFILE.seed_spacing);
    let bone_max_z =
        (bounds.max_z + BONE_PILE_PROFILE.max_extent).div_euclid(BONE_PILE_PROFILE.seed_spacing);

    for cell_z in bone_min_z..=bone_max_z {
        for cell_x in bone_min_x..=bone_max_x {
            let Some(instance) =
                instantiate_bone_pile(cell_x, cell_z, min_y, world_height, terrain)
            else {
                continue;
            };
            if !instance.bounds.intersects_chunk(&bounds) {
                continue;
            }
            place_bone_pile_in_chunk(chunk, min_y, &bounds, &instance);
        }
    }

    let portal_min_x = (bounds.min_x - SPAWN_PORTAL_PROFILE.max_extent)
        .div_euclid(SPAWN_PORTAL_PROFILE.seed_spacing);
    let portal_max_x = (bounds.max_x + SPAWN_PORTAL_PROFILE.max_extent)
        .div_euclid(SPAWN_PORTAL_PROFILE.seed_spacing);
    let portal_min_z = (bounds.min_z - SPAWN_PORTAL_PROFILE.max_extent)
        .div_euclid(SPAWN_PORTAL_PROFILE.seed_spacing);
    let portal_max_z = (bounds.max_z + SPAWN_PORTAL_PROFILE.max_extent)
        .div_euclid(SPAWN_PORTAL_PROFILE.seed_spacing);

    for cell_z in portal_min_z..=portal_max_z {
        for cell_x in portal_min_x..=portal_max_x {
            let Some(instance) =
                instantiate_spawn_portal(cell_x, cell_z, min_y, world_height, terrain)
            else {
                continue;
            };
            if !instance.bounds.intersects_chunk(&bounds) {
                continue;
            }
            place_spawn_portal_in_chunk(chunk, min_y, &bounds, &instance);
        }
    }
}

#[derive(Clone, Copy)]
struct RuinPillarProfile {
    biome_matches: fn(&super::raster::ColumnSample) -> bool,
    seed_spacing: i32,
    offset_margin: i32,
    max_extent: i32,
    min_surface_y: i32,
    max_surface_y: i32,
    max_slope: i32,
    min_ruin_density: f32,
    min_neg_pressure: f32,
    chance_base: f64,
    chance_ruin_scale: f64,
    chance_feature_scale: f64,
    chance_boundary_penalty: f64,
}

#[derive(Clone, Copy)]
struct BrokenAltarProfile {
    biome_matches: fn(&super::raster::ColumnSample) -> bool,
    seed_spacing: i32,
    offset_margin: i32,
    max_extent: i32,
    min_surface_y: i32,
    max_surface_y: i32,
    max_slope: i32,
    min_neg_pressure: f32,
    min_ruin_density: f32,
    chance_base: f64,
    chance_neg_pressure_scale: f64,
    chance_ruin_scale: f64,
    chance_boundary_penalty: f64,
}

#[derive(Clone, Copy)]
struct SpiritOreProfile {
    biome_matches: fn(&super::raster::ColumnSample) -> bool,
    seed_spacing: i32,
    offset_margin: i32,
    max_extent: i32,
    min_surface_y: i32,
    max_surface_y: i32,
    max_slope: i32,
    min_feature_mask: f32,
    chance_base: f64,
    chance_feature_scale: f64,
    chance_boundary_penalty: f64,
}

#[derive(Clone, Copy)]
struct RiftBridgeProfile {
    biome_matches: fn(&super::raster::ColumnSample) -> bool,
    seed_spacing: i32,
    offset_margin: i32,
    max_extent: i32,
    min_surface_y: i32,
    max_surface_y: i32,
    max_slope: i32,
    rift_axis_min: f32,
    rift_axis_max: f32,
    chance_base: f64,
    chance_feature_scale: f64,
    chance_rim_scale: f64,
    chance_boundary_penalty: f64,
}

#[derive(Clone, Copy)]
struct BonePileProfile {
    biome_matches: fn(&super::raster::ColumnSample) -> bool,
    seed_spacing: i32,
    offset_margin: i32,
    max_extent: i32,
    min_surface_y: i32,
    max_surface_y: i32,
    max_slope: i32,
    min_ruin_density: f32,
    chance_base: f64,
    chance_ruin_scale: f64,
    chance_boundary_penalty: f64,
}

#[derive(Clone, Copy)]
struct SpawnPortalProfile {
    biome_matches: fn(&super::raster::ColumnSample) -> bool,
    seed_spacing: i32,
    offset_margin: i32,
    max_extent: i32,
    min_surface_y: i32,
    max_surface_y: i32,
    max_slope: i32,
    chance_base: f64,
    chance_feature_scale: f64,
    chance_boundary_penalty: f64,
}

#[derive(Clone, Copy)]
struct StructureBounds {
    min_x: i32,
    max_x: i32,
    min_z: i32,
    max_z: i32,
}

impl StructureBounds {
    fn intersects_chunk(self, chunk: &ChunkBounds) -> bool {
        self.max_x >= chunk.min_x
            && self.min_x <= chunk.max_x
            && self.max_z >= chunk.min_z
            && self.min_z <= chunk.max_z
    }
}

#[derive(Clone, Copy)]
struct RuinPillarInstance {
    origin_x: i32,
    origin_z: i32,
    base_y: i32,
    seed: u64,
    footprint_radius: i32,
    column_height: i32,
    section_radius: i32,
    crown_height: i32,
    crown_overhang: i32,
    fragment_count: usize,
    bounds: StructureBounds,
}

#[derive(Clone, Copy)]
struct BrokenAltarInstance {
    origin_x: i32,
    origin_z: i32,
    base_y: i32,
    seed: u64,
    platform_radius: i32,
    dais_radius: i32,
    corner_post_height: i32,
    corner_post_radius: i32,
    bounds: StructureBounds,
}

#[derive(Clone, Copy)]
struct SpiritOreVeinInstance {
    origin_x: i32,
    origin_z: i32,
    base_y: i32,
    seed: u64,
    radius: i32,
    shard_height: i32,
    shard_count: usize,
    bounds: StructureBounds,
}

#[derive(Clone, Copy)]
struct RiftBridgeRemnantInstance {
    origin_x: i32,
    origin_z: i32,
    base_y: i32,
    seed: u64,
    span_half: i32,
    deck_half_width: i32,
    pylon_height: i32,
    axis: BridgeAxis,
    bounds: StructureBounds,
}

#[derive(Clone, Copy)]
struct BonePileInstance {
    origin_x: i32,
    origin_z: i32,
    base_y: i32,
    seed: u64,
    pile_radius: i32,
    pile_height: i32,
    skull_count: usize,
    bounds: StructureBounds,
}

#[derive(Clone, Copy)]
struct SpawnPortalInstance {
    origin_x: i32,
    origin_z: i32,
    base_y: i32,
    seed: u64,
    platform_radius: i32,
    pillar_height: i32,
    bounds: StructureBounds,
}

#[derive(Clone, Copy)]
enum BridgeAxis {
    X,
    Z,
}

#[derive(Clone, Copy)]
struct Placement {
    block: BlockState,
    priority: u8,
}

fn instantiate_ruin_pillar(
    cell_x: i32,
    cell_z: i32,
    min_y: i32,
    world_height: i32,
    terrain: &TerrainProvider,
) -> Option<RuinPillarInstance> {
    let base_seed = hash_coords(cell_x, cell_z, 0xC0DE_1201);
    let span = RUIN_PILLAR_PROFILE.seed_spacing - RUIN_PILLAR_PROFILE.offset_margin * 2;
    if span <= 0 {
        return None;
    }

    let seed_x = cell_x * RUIN_PILLAR_PROFILE.seed_spacing
        + RUIN_PILLAR_PROFILE.offset_margin
        + range_i32(base_seed, 0, span - 1);
    let seed_z = cell_z * RUIN_PILLAR_PROFILE.seed_spacing
        + RUIN_PILLAR_PROFILE.offset_margin
        + range_i32(base_seed.rotate_left(17), 0, span - 1);

    let sample = terrain.sample(seed_x, seed_z);
    if !(RUIN_PILLAR_PROFILE.biome_matches)(&sample) {
        return None;
    }

    let surface_y = column::surface_y_for_sample(&sample, min_y, world_height);
    if !(RUIN_PILLAR_PROFILE.min_surface_y..=RUIN_PILLAR_PROFILE.max_surface_y).contains(&surface_y)
    {
        return None;
    }
    if sample.water_level >= 0.0 && surface_y <= sample.water_level.round() as i32 {
        return None;
    }
    if sample.ruin_density < RUIN_PILLAR_PROFILE.min_ruin_density
        || sample.neg_pressure < RUIN_PILLAR_PROFILE.min_neg_pressure
    {
        return None;
    }
    if surface_slope(seed_x, seed_z, min_y, world_height, terrain) > RUIN_PILLAR_PROFILE.max_slope {
        return None;
    }

    let chance = (RUIN_PILLAR_PROFILE.chance_base
        + f64::from(sample.ruin_density - RUIN_PILLAR_PROFILE.min_ruin_density)
            * RUIN_PILLAR_PROFILE.chance_ruin_scale
        + f64::from(sample.feature_mask) * RUIN_PILLAR_PROFILE.chance_feature_scale
        - f64::from(sample.boundary_weight) * RUIN_PILLAR_PROFILE.chance_boundary_penalty)
        .clamp(0.0, 0.95);
    if sample_u01(base_seed.rotate_left(29)) > chance {
        return None;
    }

    let footprint_radius = range_i32(base_seed.rotate_left(5), 1, 2);
    let section_radius = if sample_u01(base_seed.rotate_left(11)) < 0.68 {
        0
    } else {
        1
    };
    let column_height = range_i32(base_seed.rotate_left(23), 7, 15);
    let crown_height = range_i32(base_seed.rotate_left(31), 1, 2);
    let crown_overhang = range_i32(base_seed.rotate_left(37), 1, 2);
    let fragment_count = range_i32(base_seed.rotate_left(43), 4, 8) as usize;
    let horizontal_extent = footprint_radius + crown_overhang + 6;

    Some(RuinPillarInstance {
        origin_x: seed_x,
        origin_z: seed_z,
        base_y: surface_y + 1,
        seed: base_seed,
        footprint_radius,
        column_height,
        section_radius,
        crown_height,
        crown_overhang,
        fragment_count,
        bounds: StructureBounds {
            min_x: seed_x - horizontal_extent,
            max_x: seed_x + horizontal_extent,
            min_z: seed_z - horizontal_extent,
            max_z: seed_z + horizontal_extent,
        },
    })
}

fn place_ruin_pillar_in_chunk(
    chunk: &mut UnloadedChunk,
    min_y: i32,
    chunk_bounds: &ChunkBounds,
    instance: &RuinPillarInstance,
) {
    let mut placements = HashMap::new();
    rasterize_foundation(&mut placements, chunk_bounds, instance);
    rasterize_column(&mut placements, chunk_bounds, instance);
    rasterize_crown(&mut placements, chunk_bounds, instance);
    rasterize_fragments(&mut placements, chunk_bounds, instance);

    for ((world_x, world_y, world_z), placement) in placements {
        if !chunk_bounds.contains(world_x, world_z) {
            continue;
        }
        let local_y = world_y - min_y;
        if local_y < 0 || local_y >= chunk.height() as i32 {
            continue;
        }
        let local_x = (world_x - chunk_bounds.min_x) as u32;
        let local_z = (world_z - chunk_bounds.min_z) as u32;
        let existing = chunk.block_state(local_x, local_y as u32, local_z);
        if !can_replace(existing, placement.block, placement.priority) {
            continue;
        }
        chunk.set_block_state(local_x, local_y as u32, local_z, placement.block);
    }
}

fn instantiate_broken_altar(
    cell_x: i32,
    cell_z: i32,
    min_y: i32,
    world_height: i32,
    terrain: &TerrainProvider,
) -> Option<BrokenAltarInstance> {
    let base_seed = hash_coords(cell_x, cell_z, 0xA17A_2026);
    let span = BROKEN_ALTAR_PROFILE.seed_spacing - BROKEN_ALTAR_PROFILE.offset_margin * 2;
    if span <= 0 {
        return None;
    }

    let seed_x = cell_x * BROKEN_ALTAR_PROFILE.seed_spacing
        + BROKEN_ALTAR_PROFILE.offset_margin
        + range_i32(base_seed, 0, span - 1);
    let seed_z = cell_z * BROKEN_ALTAR_PROFILE.seed_spacing
        + BROKEN_ALTAR_PROFILE.offset_margin
        + range_i32(base_seed.rotate_left(17), 0, span - 1);

    let sample = terrain.sample(seed_x, seed_z);
    if !(BROKEN_ALTAR_PROFILE.biome_matches)(&sample) {
        return None;
    }

    let surface_y = column::surface_y_for_sample(&sample, min_y, world_height);
    if !(BROKEN_ALTAR_PROFILE.min_surface_y..=BROKEN_ALTAR_PROFILE.max_surface_y)
        .contains(&surface_y)
    {
        return None;
    }
    if sample.water_level >= 0.0 && surface_y <= sample.water_level.round() as i32 {
        return None;
    }
    if sample.neg_pressure < BROKEN_ALTAR_PROFILE.min_neg_pressure
        || sample.ruin_density < BROKEN_ALTAR_PROFILE.min_ruin_density
    {
        return None;
    }
    if surface_slope(seed_x, seed_z, min_y, world_height, terrain) > BROKEN_ALTAR_PROFILE.max_slope
    {
        return None;
    }

    let chance = (BROKEN_ALTAR_PROFILE.chance_base
        + f64::from(sample.neg_pressure - BROKEN_ALTAR_PROFILE.min_neg_pressure)
            * BROKEN_ALTAR_PROFILE.chance_neg_pressure_scale
        + f64::from(sample.ruin_density - BROKEN_ALTAR_PROFILE.min_ruin_density)
            * BROKEN_ALTAR_PROFILE.chance_ruin_scale
        - f64::from(sample.boundary_weight) * BROKEN_ALTAR_PROFILE.chance_boundary_penalty)
        .clamp(0.0, 0.95);
    if sample_u01(base_seed.rotate_left(29)) > chance {
        return None;
    }

    let platform_radius = range_i32(base_seed.rotate_left(7), 2, 3);
    let dais_radius = range_i32(base_seed.rotate_left(13), 1, 2).min(platform_radius);
    let corner_post_height = range_i32(base_seed.rotate_left(23), 2, 4);
    let corner_post_radius = if platform_radius >= 3 && sample_u01(base_seed.rotate_left(31)) < 0.35
    {
        1
    } else {
        0
    };
    let horizontal_extent = platform_radius + corner_post_radius + 5;

    Some(BrokenAltarInstance {
        origin_x: seed_x,
        origin_z: seed_z,
        base_y: surface_y + 1,
        seed: base_seed,
        platform_radius,
        dais_radius,
        corner_post_height,
        corner_post_radius,
        bounds: StructureBounds {
            min_x: seed_x - horizontal_extent,
            max_x: seed_x + horizontal_extent,
            min_z: seed_z - horizontal_extent,
            max_z: seed_z + horizontal_extent,
        },
    })
}

fn place_broken_altar_in_chunk(
    chunk: &mut UnloadedChunk,
    min_y: i32,
    chunk_bounds: &ChunkBounds,
    instance: &BrokenAltarInstance,
) {
    let mut placements = HashMap::new();
    rasterize_altar_platform(&mut placements, chunk_bounds, instance);
    rasterize_altar_dais(&mut placements, chunk_bounds, instance);
    rasterize_altar_posts(&mut placements, chunk_bounds, instance);
    rasterize_altar_core(&mut placements, chunk_bounds, instance);
    rasterize_altar_rubble(&mut placements, chunk_bounds, instance);

    for ((world_x, world_y, world_z), placement) in placements {
        if !chunk_bounds.contains(world_x, world_z) {
            continue;
        }
        let local_y = world_y - min_y;
        if local_y < 0 || local_y >= chunk.height() as i32 {
            continue;
        }
        let local_x = (world_x - chunk_bounds.min_x) as u32;
        let local_z = (world_z - chunk_bounds.min_z) as u32;
        let existing = chunk.block_state(local_x, local_y as u32, local_z);
        if !can_replace(existing, placement.block, placement.priority) {
            continue;
        }
        chunk.set_block_state(local_x, local_y as u32, local_z, placement.block);
    }
}

fn instantiate_spirit_ore_vein(
    cell_x: i32,
    cell_z: i32,
    min_y: i32,
    world_height: i32,
    terrain: &TerrainProvider,
) -> Option<SpiritOreVeinInstance> {
    let base_seed = hash_coords(cell_x, cell_z, 0x0AE5_2026);
    let span = SPIRIT_ORE_PROFILE.seed_spacing - SPIRIT_ORE_PROFILE.offset_margin * 2;
    if span <= 0 {
        return None;
    }

    let seed_x = cell_x * SPIRIT_ORE_PROFILE.seed_spacing
        + SPIRIT_ORE_PROFILE.offset_margin
        + range_i32(base_seed, 0, span - 1);
    let seed_z = cell_z * SPIRIT_ORE_PROFILE.seed_spacing
        + SPIRIT_ORE_PROFILE.offset_margin
        + range_i32(base_seed.rotate_left(17), 0, span - 1);

    let sample = terrain.sample(seed_x, seed_z);
    if !(SPIRIT_ORE_PROFILE.biome_matches)(&sample) {
        return None;
    }

    let surface_y = column::surface_y_for_sample(&sample, min_y, world_height);
    if !(SPIRIT_ORE_PROFILE.min_surface_y..=SPIRIT_ORE_PROFILE.max_surface_y).contains(&surface_y) {
        return None;
    }
    if sample.feature_mask < SPIRIT_ORE_PROFILE.min_feature_mask {
        return None;
    }
    if sample.water_level >= 0.0 && surface_y <= sample.water_level.round() as i32 {
        return None;
    }
    if surface_slope(seed_x, seed_z, min_y, world_height, terrain) > SPIRIT_ORE_PROFILE.max_slope {
        return None;
    }

    let chance = (SPIRIT_ORE_PROFILE.chance_base
        + f64::from(sample.feature_mask - SPIRIT_ORE_PROFILE.min_feature_mask)
            * SPIRIT_ORE_PROFILE.chance_feature_scale
        - f64::from(sample.boundary_weight) * SPIRIT_ORE_PROFILE.chance_boundary_penalty)
        .clamp(0.0, 0.95);
    if sample_u01(base_seed.rotate_left(29)) > chance {
        return None;
    }

    let radius = range_i32(base_seed.rotate_left(7), 2, 3);
    let shard_height = range_i32(base_seed.rotate_left(13), 3, 6);
    let shard_count = range_i32(base_seed.rotate_left(19), 3, 6) as usize;
    let horizontal_extent = radius + 6;

    Some(SpiritOreVeinInstance {
        origin_x: seed_x,
        origin_z: seed_z,
        base_y: surface_y + 1,
        seed: base_seed,
        radius,
        shard_height,
        shard_count,
        bounds: StructureBounds {
            min_x: seed_x - horizontal_extent,
            max_x: seed_x + horizontal_extent,
            min_z: seed_z - horizontal_extent,
            max_z: seed_z + horizontal_extent,
        },
    })
}

fn place_spirit_ore_vein_in_chunk(
    chunk: &mut UnloadedChunk,
    min_y: i32,
    chunk_bounds: &ChunkBounds,
    instance: &SpiritOreVeinInstance,
) {
    let mut placements = HashMap::new();
    rasterize_ore_outcrop(&mut placements, chunk_bounds, instance);
    rasterize_ore_shards(&mut placements, chunk_bounds, instance);
    rasterize_ore_scatter(&mut placements, chunk_bounds, instance);

    for ((world_x, world_y, world_z), placement) in placements {
        if !chunk_bounds.contains(world_x, world_z) {
            continue;
        }
        let local_y = world_y - min_y;
        if local_y < 0 || local_y >= chunk.height() as i32 {
            continue;
        }
        let local_x = (world_x - chunk_bounds.min_x) as u32;
        let local_z = (world_z - chunk_bounds.min_z) as u32;
        let existing = chunk.block_state(local_x, local_y as u32, local_z);
        if !can_replace(existing, placement.block, placement.priority) {
            continue;
        }
        chunk.set_block_state(local_x, local_y as u32, local_z, placement.block);
    }
}

fn instantiate_rift_bridge_remnant(
    cell_x: i32,
    cell_z: i32,
    min_y: i32,
    world_height: i32,
    terrain: &TerrainProvider,
) -> Option<RiftBridgeRemnantInstance> {
    let base_seed = hash_coords(cell_x, cell_z, 0xB21D_6E26);
    let span = RIFT_BRIDGE_PROFILE.seed_spacing - RIFT_BRIDGE_PROFILE.offset_margin * 2;
    if span <= 0 {
        return None;
    }

    let seed_x = cell_x * RIFT_BRIDGE_PROFILE.seed_spacing
        + RIFT_BRIDGE_PROFILE.offset_margin
        + range_i32(base_seed, 0, span - 1);
    let seed_z = cell_z * RIFT_BRIDGE_PROFILE.seed_spacing
        + RIFT_BRIDGE_PROFILE.offset_margin
        + range_i32(base_seed.rotate_left(17), 0, span - 1);

    let sample = terrain.sample(seed_x, seed_z);
    if !(RIFT_BRIDGE_PROFILE.biome_matches)(&sample) {
        return None;
    }

    let surface_y = column::surface_y_for_sample(&sample, min_y, world_height);
    if !(RIFT_BRIDGE_PROFILE.min_surface_y..=RIFT_BRIDGE_PROFILE.max_surface_y).contains(&surface_y)
    {
        return None;
    }
    if sample.rift_axis_sdf < RIFT_BRIDGE_PROFILE.rift_axis_min
        || sample.rift_axis_sdf > RIFT_BRIDGE_PROFILE.rift_axis_max
    {
        return None;
    }
    if surface_slope(seed_x, seed_z, min_y, world_height, terrain) > RIFT_BRIDGE_PROFILE.max_slope {
        return None;
    }

    let chance = (RIFT_BRIDGE_PROFILE.chance_base
        + f64::from(sample.feature_mask) * RIFT_BRIDGE_PROFILE.chance_feature_scale
        + f64::from(sample.rim_edge_mask) * RIFT_BRIDGE_PROFILE.chance_rim_scale
        - f64::from(sample.boundary_weight) * RIFT_BRIDGE_PROFILE.chance_boundary_penalty)
        .clamp(0.0, 0.95);
    if sample_u01(base_seed.rotate_left(29)) > chance {
        return None;
    }

    let axis = if sample_u01(base_seed.rotate_left(7)) < 0.5 {
        BridgeAxis::X
    } else {
        BridgeAxis::Z
    };
    let span_half = range_i32(base_seed.rotate_left(13), 4, 7);
    let deck_half_width = range_i32(base_seed.rotate_left(19), 1, 2);
    let pylon_height = range_i32(base_seed.rotate_left(23), 3, 5);
    let horizontal_extent = span_half + deck_half_width + 5;

    Some(RiftBridgeRemnantInstance {
        origin_x: seed_x,
        origin_z: seed_z,
        base_y: surface_y + 2,
        seed: base_seed,
        span_half,
        deck_half_width,
        pylon_height,
        axis,
        bounds: StructureBounds {
            min_x: seed_x - horizontal_extent,
            max_x: seed_x + horizontal_extent,
            min_z: seed_z - horizontal_extent,
            max_z: seed_z + horizontal_extent,
        },
    })
}

fn place_rift_bridge_remnant_in_chunk(
    chunk: &mut UnloadedChunk,
    min_y: i32,
    chunk_bounds: &ChunkBounds,
    instance: &RiftBridgeRemnantInstance,
) {
    let mut placements = HashMap::new();
    rasterize_bridge_pylons(&mut placements, chunk_bounds, instance);
    rasterize_bridge_deck(&mut placements, chunk_bounds, instance);
    rasterize_bridge_chains(&mut placements, chunk_bounds, instance);
    rasterize_bridge_rubble(&mut placements, chunk_bounds, instance);

    for ((world_x, world_y, world_z), placement) in placements {
        if !chunk_bounds.contains(world_x, world_z) {
            continue;
        }
        let local_y = world_y - min_y;
        if local_y < 0 || local_y >= chunk.height() as i32 {
            continue;
        }
        let local_x = (world_x - chunk_bounds.min_x) as u32;
        let local_z = (world_z - chunk_bounds.min_z) as u32;
        let existing = chunk.block_state(local_x, local_y as u32, local_z);
        if !can_replace(existing, placement.block, placement.priority) {
            continue;
        }
        chunk.set_block_state(local_x, local_y as u32, local_z, placement.block);
    }
}

fn rasterize_ore_outcrop(
    placements: &mut HashMap<(i32, i32, i32), Placement>,
    chunk_bounds: &ChunkBounds,
    instance: &SpiritOreVeinInstance,
) {
    for dz in -instance.radius..=instance.radius {
        for dx in -instance.radius..=instance.radius {
            let dist = dx.abs() + dz.abs();
            if dist > instance.radius + 1 {
                continue;
            }
            let world_y = instance.base_y - 1 + ((instance.radius - dist).max(0) / 2);
            upsert_block(
                placements,
                chunk_bounds,
                instance.origin_x + dx,
                world_y,
                instance.origin_z + dz,
                ore_matrix_block(instance.seed, dx, dz),
                5,
            );

            if sample_u01(hash_coords(dx, dz, instance.seed.rotate_left(33))) < 0.42 {
                upsert_block(
                    placements,
                    chunk_bounds,
                    instance.origin_x + dx,
                    world_y + 1,
                    instance.origin_z + dz,
                    ore_node_block(instance.seed, dx, dz),
                    6,
                );
            }
        }
    }
}

fn rasterize_ore_shards(
    placements: &mut HashMap<(i32, i32, i32), Placement>,
    chunk_bounds: &ChunkBounds,
    instance: &SpiritOreVeinInstance,
) {
    for index in 0..instance.shard_count {
        let angle =
            sample_u01(instance.seed.rotate_left(index as u32 + 41)) * std::f64::consts::TAU;
        let distance = 1
            + (sample_u01(instance.seed.rotate_left(index as u32 + 47)) * instance.radius as f64)
                .round() as i32;
        let base_x = instance.origin_x + (angle.cos() * distance as f64).round() as i32;
        let base_z = instance.origin_z + (angle.sin() * distance as f64).round() as i32;
        let height = (instance.shard_height - (index as i32 % 2)).max(2);
        for dy in 0..height {
            let world_y = instance.base_y + dy;
            if dy == height - 1
                && sample_u01(hash_coords(base_x, base_z, instance.seed + dy as u64)) < 0.6
            {
                upsert_block(
                    placements,
                    chunk_bounds,
                    base_x,
                    world_y,
                    base_z,
                    BlockState::BUDDING_AMETHYST,
                    7,
                );
            } else {
                upsert_block(
                    placements,
                    chunk_bounds,
                    base_x,
                    world_y,
                    base_z,
                    ore_shard_block(instance.seed, index, dy),
                    6,
                );
            }
        }
    }
}

fn rasterize_ore_scatter(
    placements: &mut HashMap<(i32, i32, i32), Placement>,
    chunk_bounds: &ChunkBounds,
    instance: &SpiritOreVeinInstance,
) {
    for index in 0..(instance.shard_count + 2) {
        let angle =
            sample_u01(instance.seed.rotate_left(index as u32 + 55)) * std::f64::consts::TAU;
        let distance = instance.radius
            + 1
            + (sample_u01(instance.seed.rotate_left(index as u32 + 59)) * 3.0).round() as i32;
        let world_x = instance.origin_x + (angle.cos() * distance as f64).round() as i32;
        let world_z = instance.origin_z + (angle.sin() * distance as f64).round() as i32;
        let world_y = instance.base_y - 1;
        let hash = hash_coords(world_x, world_z, instance.seed + index as u64);
        let block = if sample_u01(hash) < 0.5 {
            BlockState::CALCITE
        } else if sample_u01(hash.rotate_left(9)) < 0.6 {
            BlockState::AMETHYST_BLOCK
        } else {
            BlockState::DEEPSLATE_EMERALD_ORE
        };
        upsert_block(
            placements,
            chunk_bounds,
            world_x,
            world_y,
            world_z,
            block,
            3,
        );
    }
}

fn rasterize_bridge_pylons(
    placements: &mut HashMap<(i32, i32, i32), Placement>,
    chunk_bounds: &ChunkBounds,
    instance: &RiftBridgeRemnantInstance,
) {
    let ends = bridge_endpoints(instance);
    for (index, (x, z)) in ends.into_iter().enumerate() {
        let broken = sample_u01(instance.seed.rotate_left(index as u32 + 61)) < 0.4;
        let height = if broken {
            (instance.pylon_height - 1).max(2)
        } else {
            instance.pylon_height
        };
        for dy in 0..height {
            upsert_block(
                placements,
                chunk_bounds,
                x,
                instance.base_y + dy,
                z,
                bridge_support_block(instance.seed, index, dy),
                5,
            );
        }

        if !broken {
            upsert_block(
                placements,
                chunk_bounds,
                x,
                instance.base_y + height,
                z,
                BlockState::SMOOTH_BASALT,
                4,
            );
        }
    }
}

fn rasterize_bridge_deck(
    placements: &mut HashMap<(i32, i32, i32), Placement>,
    chunk_bounds: &ChunkBounds,
    instance: &RiftBridgeRemnantInstance,
) {
    for along in -instance.span_half..=instance.span_half {
        let missing = sample_u01(hash_coords(
            along,
            instance.deck_half_width,
            instance.seed.rotate_left(67),
        )) < 0.26;
        if missing {
            continue;
        }
        for cross in -instance.deck_half_width..=instance.deck_half_width {
            let (world_x, world_z) = match instance.axis {
                BridgeAxis::X => (instance.origin_x + along, instance.origin_z + cross),
                BridgeAxis::Z => (instance.origin_x + cross, instance.origin_z + along),
            };
            let deck_y = instance.base_y + instance.pylon_height - (along.abs() / 4);
            upsert_block(
                placements,
                chunk_bounds,
                world_x,
                deck_y,
                world_z,
                bridge_deck_block(instance.seed, along, cross),
                4,
            );

            if cross.abs() == instance.deck_half_width
                && sample_u01(hash_coords(world_x, world_z, instance.seed.rotate_left(71))) < 0.45
            {
                upsert_block(
                    placements,
                    chunk_bounds,
                    world_x,
                    deck_y + 1,
                    world_z,
                    if sample_u01(hash_coords(world_x, world_z, instance.seed.rotate_left(73)))
                        < 0.5
                    {
                        BlockState::IRON_BARS
                    } else {
                        BlockState::CHAIN
                    },
                    3,
                );
            }
        }
    }
}

fn rasterize_bridge_chains(
    placements: &mut HashMap<(i32, i32, i32), Placement>,
    chunk_bounds: &ChunkBounds,
    instance: &RiftBridgeRemnantInstance,
) {
    let ends = bridge_endpoints(instance);
    for (index, (x, z)) in ends.into_iter().enumerate() {
        if sample_u01(instance.seed.rotate_left(index as u32 + 79)) > 0.58 {
            continue;
        }
        for dy in 1..=instance.pylon_height {
            let (chain_x, chain_z) = match instance.axis {
                BridgeAxis::X => (
                    x,
                    z + if index % 2 == 0 {
                        -instance.deck_half_width
                    } else {
                        instance.deck_half_width
                    },
                ),
                BridgeAxis::Z => (
                    x + if index % 2 == 0 {
                        -instance.deck_half_width
                    } else {
                        instance.deck_half_width
                    },
                    z,
                ),
            };
            upsert_block(
                placements,
                chunk_bounds,
                chain_x,
                instance.base_y + dy,
                chain_z,
                BlockState::CHAIN.set(PropName::Axis, PropValue::Y),
                3,
            );
        }
    }
}

fn rasterize_bridge_rubble(
    placements: &mut HashMap<(i32, i32, i32), Placement>,
    chunk_bounds: &ChunkBounds,
    instance: &RiftBridgeRemnantInstance,
) {
    for index in 0..6 {
        let angle =
            sample_u01(instance.seed.rotate_left(index as u32 + 83)) * std::f64::consts::TAU;
        let distance = instance.deck_half_width
            + 2
            + (sample_u01(instance.seed.rotate_left(index as u32 + 89)) * 5.0).round() as i32;
        let world_x = instance.origin_x + (angle.cos() * distance as f64).round() as i32;
        let world_z = instance.origin_z + (angle.sin() * distance as f64).round() as i32;
        let world_y = instance.base_y - 1;
        let hash = hash_coords(world_x, world_z, instance.seed + index as u64);
        let block = match hash % 5 {
            0 => BlockState::POLISHED_BASALT,
            1 => BlockState::BASALT,
            2 => BlockState::DARK_OAK_SLAB.set(PropName::Type, PropValue::Bottom),
            3 => BlockState::SPRUCE_SLAB.set(PropName::Type, PropValue::Bottom),
            _ => BlockState::BLACKSTONE,
        };
        upsert_block(
            placements,
            chunk_bounds,
            world_x,
            world_y,
            world_z,
            block,
            2,
        );
    }
}

fn instantiate_bone_pile(
    cell_x: i32,
    cell_z: i32,
    min_y: i32,
    world_height: i32,
    terrain: &TerrainProvider,
) -> Option<BonePileInstance> {
    let base_seed = hash_coords(cell_x, cell_z, 0xB04E_2026);
    let span = BONE_PILE_PROFILE.seed_spacing - BONE_PILE_PROFILE.offset_margin * 2;
    if span <= 0 {
        return None;
    }

    let seed_x = cell_x * BONE_PILE_PROFILE.seed_spacing
        + BONE_PILE_PROFILE.offset_margin
        + range_i32(base_seed, 0, span - 1);
    let seed_z = cell_z * BONE_PILE_PROFILE.seed_spacing
        + BONE_PILE_PROFILE.offset_margin
        + range_i32(base_seed.rotate_left(17), 0, span - 1);

    let sample = terrain.sample(seed_x, seed_z);
    if !(BONE_PILE_PROFILE.biome_matches)(&sample) {
        return None;
    }

    let surface_y = column::surface_y_for_sample(&sample, min_y, world_height);
    if !(BONE_PILE_PROFILE.min_surface_y..=BONE_PILE_PROFILE.max_surface_y).contains(&surface_y) {
        return None;
    }
    if sample.water_level >= 0.0 && surface_y <= sample.water_level.round() as i32 {
        return None;
    }
    if sample.ruin_density < BONE_PILE_PROFILE.min_ruin_density {
        return None;
    }
    if surface_slope(seed_x, seed_z, min_y, world_height, terrain) > BONE_PILE_PROFILE.max_slope {
        return None;
    }

    let chance = (BONE_PILE_PROFILE.chance_base
        + f64::from(sample.ruin_density - BONE_PILE_PROFILE.min_ruin_density)
            * BONE_PILE_PROFILE.chance_ruin_scale
        - f64::from(sample.boundary_weight) * BONE_PILE_PROFILE.chance_boundary_penalty)
        .clamp(0.0, 0.95);
    if sample_u01(base_seed.rotate_left(29)) > chance {
        return None;
    }

    let pile_radius = range_i32(base_seed.rotate_left(5), 1, 3);
    let pile_height = range_i32(base_seed.rotate_left(11), 1, 3);
    let skull_count = range_i32(base_seed.rotate_left(19), 1, 3) as usize;
    let horizontal_extent = pile_radius + 3;

    Some(BonePileInstance {
        origin_x: seed_x,
        origin_z: seed_z,
        base_y: surface_y + 1,
        seed: base_seed,
        pile_radius,
        pile_height,
        skull_count,
        bounds: StructureBounds {
            min_x: seed_x - horizontal_extent,
            max_x: seed_x + horizontal_extent,
            min_z: seed_z - horizontal_extent,
            max_z: seed_z + horizontal_extent,
        },
    })
}

fn place_bone_pile_in_chunk(
    chunk: &mut UnloadedChunk,
    min_y: i32,
    chunk_bounds: &ChunkBounds,
    instance: &BonePileInstance,
) {
    let mut placements = HashMap::new();

    for dz in -instance.pile_radius..=instance.pile_radius {
        for dx in -instance.pile_radius..=instance.pile_radius {
            let dist = dx.abs() + dz.abs();
            if dist > instance.pile_radius + 1 {
                continue;
            }
            let local_height = (instance.pile_height - dist).max(0);
            for dy in 0..=local_height {
                let hash = hash_coords(dx + dy, dz - dy, instance.seed.rotate_left(3));
                let block = match hash % 7 {
                    0 => BlockState::BONE_BLOCK,
                    1 => BlockState::BONE_BLOCK,
                    2 => BlockState::BONE_BLOCK,
                    3 => BlockState::SOUL_SAND,
                    4 => BlockState::SOUL_SOIL,
                    5 => BlockState::GRAVEL,
                    _ => BlockState::COBBLESTONE,
                };
                upsert_block(
                    &mut placements,
                    chunk_bounds,
                    instance.origin_x + dx,
                    instance.base_y + dy,
                    instance.origin_z + dz,
                    block,
                    5,
                );
            }
        }
    }

    for index in 0..instance.skull_count {
        let angle =
            sample_u01(instance.seed.rotate_left(index as u32 + 31)) * std::f64::consts::TAU;
        let distance = (sample_u01(instance.seed.rotate_left(index as u32 + 37))
            * instance.pile_radius as f64)
            .round() as i32;
        let sx = instance.origin_x + (angle.cos() * distance as f64).round() as i32;
        let sz = instance.origin_z + (angle.sin() * distance as f64).round() as i32;
        let dist_from_center = (sx - instance.origin_x).abs() + (sz - instance.origin_z).abs();
        let skull_y = instance.base_y + (instance.pile_height - dist_from_center).max(0) + 1;
        let rotation = (instance.seed.rotate_left(index as u32 + 41) % 16) as u16;
        upsert_block(
            &mut placements,
            chunk_bounds,
            sx,
            skull_y,
            sz,
            skeleton_skull_state(rotation),
            6,
        );
    }

    for index in 0..(instance.pile_radius as usize + 2) {
        let angle =
            sample_u01(instance.seed.rotate_left(index as u32 + 51)) * std::f64::consts::TAU;
        let distance = instance.pile_radius
            + 1
            + (sample_u01(instance.seed.rotate_left(index as u32 + 57)) * 2.0).round() as i32;
        let world_x = instance.origin_x + (angle.cos() * distance as f64).round() as i32;
        let world_z = instance.origin_z + (angle.sin() * distance as f64).round() as i32;
        let hash = hash_coords(world_x, world_z, instance.seed + index as u64);
        let block = match hash % 4 {
            0 => BlockState::BONE_BLOCK,
            1 => BlockState::SOUL_SAND,
            2 => BlockState::SOUL_SOIL,
            _ => BlockState::GRAVEL,
        };
        upsert_block(
            &mut placements,
            chunk_bounds,
            world_x,
            instance.base_y - 1,
            world_z,
            block,
            3,
        );
    }

    for ((world_x, world_y, world_z), placement) in placements {
        if !chunk_bounds.contains(world_x, world_z) {
            continue;
        }
        let local_y = world_y - min_y;
        if local_y < 0 || local_y >= chunk.height() as i32 {
            continue;
        }
        let local_x = (world_x - chunk_bounds.min_x) as u32;
        let local_z = (world_z - chunk_bounds.min_z) as u32;
        let existing = chunk.block_state(local_x, local_y as u32, local_z);
        if !can_replace(existing, placement.block, placement.priority) {
            continue;
        }
        chunk.set_block_state(local_x, local_y as u32, local_z, placement.block);
    }
}

fn instantiate_spawn_portal(
    cell_x: i32,
    cell_z: i32,
    min_y: i32,
    world_height: i32,
    terrain: &TerrainProvider,
) -> Option<SpawnPortalInstance> {
    let base_seed = hash_coords(cell_x, cell_z, 0x5FA0_2026);
    let span = SPAWN_PORTAL_PROFILE.seed_spacing - SPAWN_PORTAL_PROFILE.offset_margin * 2;
    if span <= 0 {
        return None;
    }

    let seed_x = cell_x * SPAWN_PORTAL_PROFILE.seed_spacing
        + SPAWN_PORTAL_PROFILE.offset_margin
        + range_i32(base_seed, 0, span - 1);
    let seed_z = cell_z * SPAWN_PORTAL_PROFILE.seed_spacing
        + SPAWN_PORTAL_PROFILE.offset_margin
        + range_i32(base_seed.rotate_left(17), 0, span - 1);

    let sample = terrain.sample(seed_x, seed_z);
    if !(SPAWN_PORTAL_PROFILE.biome_matches)(&sample) {
        return None;
    }

    let surface_y = column::surface_y_for_sample(&sample, min_y, world_height);
    if !(SPAWN_PORTAL_PROFILE.min_surface_y..=SPAWN_PORTAL_PROFILE.max_surface_y)
        .contains(&surface_y)
    {
        return None;
    }
    if sample.water_level >= 0.0 && surface_y <= sample.water_level.round() as i32 {
        return None;
    }
    if surface_slope(seed_x, seed_z, min_y, world_height, terrain) > SPAWN_PORTAL_PROFILE.max_slope
    {
        return None;
    }

    let chance = (SPAWN_PORTAL_PROFILE.chance_base
        + f64::from(sample.feature_mask) * SPAWN_PORTAL_PROFILE.chance_feature_scale
        - f64::from(sample.boundary_weight) * SPAWN_PORTAL_PROFILE.chance_boundary_penalty)
        .clamp(0.0, 0.99);
    if sample_u01(base_seed.rotate_left(29)) > chance {
        return None;
    }

    let platform_radius = 5;
    let pillar_height = range_i32(base_seed.rotate_left(7), 4, 6);
    let horizontal_extent = platform_radius + 4;

    Some(SpawnPortalInstance {
        origin_x: seed_x,
        origin_z: seed_z,
        base_y: surface_y + 1,
        seed: base_seed,
        platform_radius,
        pillar_height,
        bounds: StructureBounds {
            min_x: seed_x - horizontal_extent,
            max_x: seed_x + horizontal_extent,
            min_z: seed_z - horizontal_extent,
            max_z: seed_z + horizontal_extent,
        },
    })
}

fn place_spawn_portal_in_chunk(
    chunk: &mut UnloadedChunk,
    min_y: i32,
    chunk_bounds: &ChunkBounds,
    instance: &SpawnPortalInstance,
) {
    let mut placements = HashMap::new();
    let r = instance.platform_radius;

    for dz in -r..=r {
        for dx in -r..=r {
            let dist_sq = dx * dx + dz * dz;
            if dist_sq > r * r + r {
                continue;
            }
            let ring = dx.abs().max(dz.abs());
            let block = if ring <= 1 {
                portal_core_block(instance.seed, dx, dz)
            } else if (dx.abs() + dz.abs()) % 2 == 0 {
                BlockState::END_STONE_BRICKS
            } else {
                BlockState::PURPUR_BLOCK
            };
            upsert_block(
                &mut placements,
                chunk_bounds,
                instance.origin_x + dx,
                instance.base_y - 1,
                instance.origin_z + dz,
                block,
                6,
            );

            if ring >= 2 && sample_u01(hash_coords(dx, dz, instance.seed.rotate_left(3))) < 0.12 {
                upsert_block(
                    &mut placements,
                    chunk_bounds,
                    instance.origin_x + dx,
                    instance.base_y,
                    instance.origin_z + dz,
                    candle_state(
                        1 + (hash_coords(dx, dz, instance.seed.rotate_left(5)) % 3) as u16,
                        true,
                    ),
                    4,
                );
            }
        }
    }

    upsert_block(
        &mut placements,
        chunk_bounds,
        instance.origin_x,
        instance.base_y,
        instance.origin_z,
        BlockState::LODESTONE,
        7,
    );
    upsert_block(
        &mut placements,
        chunk_bounds,
        instance.origin_x,
        instance.base_y + 1,
        instance.origin_z,
        BlockState::END_ROD,
        7,
    );

    let pillar_offsets = [(-r, -r), (r, -r), (-r, r), (r, r)];
    for (index, (px, pz)) in pillar_offsets.into_iter().enumerate() {
        for dy in 0..instance.pillar_height {
            let block = if dy == 0 {
                BlockState::CHISELED_POLISHED_BLACKSTONE
            } else {
                portal_pillar_block(instance.seed, index, dy)
            };
            upsert_block(
                &mut placements,
                chunk_bounds,
                instance.origin_x + px,
                instance.base_y + dy,
                instance.origin_z + pz,
                block,
                6,
            );
        }
        upsert_block(
            &mut placements,
            chunk_bounds,
            instance.origin_x + px,
            instance.base_y + instance.pillar_height,
            instance.origin_z + pz,
            BlockState::SOUL_LANTERN,
            5,
        );
    }

    let cardinal = [(0, -r - 1), (0, r + 1), (-r - 1, 0), (r + 1, 0)];
    for (index, (cx, cz)) in cardinal.into_iter().enumerate() {
        let broken = sample_u01(instance.seed.rotate_left(index as u32 + 11)) < 0.3;
        if broken {
            continue;
        }
        upsert_block(
            &mut placements,
            chunk_bounds,
            instance.origin_x + cx,
            instance.base_y - 1,
            instance.origin_z + cz,
            BlockState::CRYING_OBSIDIAN,
            5,
        );
        upsert_block(
            &mut placements,
            chunk_bounds,
            instance.origin_x + cx,
            instance.base_y,
            instance.origin_z + cz,
            BlockState::END_ROD,
            5,
        );
    }

    for ((world_x, world_y, world_z), placement) in placements {
        if !chunk_bounds.contains(world_x, world_z) {
            continue;
        }
        let local_y = world_y - min_y;
        if local_y < 0 || local_y >= chunk.height() as i32 {
            continue;
        }
        let local_x = (world_x - chunk_bounds.min_x) as u32;
        let local_z = (world_z - chunk_bounds.min_z) as u32;
        let existing = chunk.block_state(local_x, local_y as u32, local_z);
        if !can_replace(existing, placement.block, placement.priority) {
            continue;
        }
        chunk.set_block_state(local_x, local_y as u32, local_z, placement.block);
    }
}

fn portal_core_block(seed: u64, dx: i32, dz: i32) -> BlockState {
    let hash = hash_coords(dx, dz, seed.rotate_left(71));
    if dx == 0 && dz == 0 {
        BlockState::CRYING_OBSIDIAN
    } else {
        match hash % 5 {
            0 => BlockState::CRYING_OBSIDIAN,
            1 => BlockState::OBSIDIAN,
            2 => BlockState::AMETHYST_BLOCK,
            _ => BlockState::END_STONE_BRICKS,
        }
    }
}

fn portal_pillar_block(seed: u64, index: usize, dy: i32) -> BlockState {
    let hash = hash_coords(index as i32, dy, seed.rotate_left(77));
    match hash % 5 {
        0 => BlockState::POLISHED_BLACKSTONE,
        1 => BlockState::CHISELED_POLISHED_BLACKSTONE,
        2 => BlockState::CRYING_OBSIDIAN,
        _ => BlockState::POLISHED_BLACKSTONE_BRICKS,
    }
}

fn skeleton_skull_state(rotation: u16) -> BlockState {
    let rot_value = match rotation % 16 {
        0 => PropValue::_0,
        1 => PropValue::_1,
        2 => PropValue::_2,
        3 => PropValue::_3,
        4 => PropValue::_4,
        5 => PropValue::_5,
        6 => PropValue::_6,
        7 => PropValue::_7,
        8 => PropValue::_8,
        9 => PropValue::_9,
        10 => PropValue::_10,
        11 => PropValue::_11,
        12 => PropValue::_12,
        13 => PropValue::_13,
        14 => PropValue::_14,
        _ => PropValue::_15,
    };
    BlockState::SKELETON_SKULL
        .set(PropName::Rotation, rot_value)
        .set(PropName::Powered, PropValue::False)
}

fn ore_matrix_block(seed: u64, dx: i32, dz: i32) -> BlockState {
    let hash = hash_coords(dx, dz, seed.rotate_left(91));
    match hash % 6 {
        0 => BlockState::CALCITE,
        1 => BlockState::SMOOTH_BASALT,
        2 => BlockState::DEEPSLATE,
        _ => BlockState::STONE,
    }
}

fn ore_node_block(seed: u64, dx: i32, dz: i32) -> BlockState {
    let hash = hash_coords(dx, dz, seed.rotate_left(97));
    match hash % 6 {
        0 => BlockState::AMETHYST_BLOCK,
        1 => BlockState::BUDDING_AMETHYST,
        2 => BlockState::DEEPSLATE_EMERALD_ORE,
        3 => BlockState::EMERALD_ORE,
        _ => BlockState::CALCITE,
    }
}

fn ore_shard_block(seed: u64, index: usize, dy: i32) -> BlockState {
    let hash = hash_coords(index as i32, dy, seed.rotate_left(101));
    match hash % 5 {
        0 => BlockState::AMETHYST_BLOCK,
        1 => BlockState::BUDDING_AMETHYST,
        2 => BlockState::CALCITE,
        _ => BlockState::DEEPSLATE_EMERALD_ORE,
    }
}

fn bridge_endpoints(instance: &RiftBridgeRemnantInstance) -> [(i32, i32); 2] {
    match instance.axis {
        BridgeAxis::X => [
            (instance.origin_x - instance.span_half, instance.origin_z),
            (instance.origin_x + instance.span_half, instance.origin_z),
        ],
        BridgeAxis::Z => [
            (instance.origin_x, instance.origin_z - instance.span_half),
            (instance.origin_x, instance.origin_z + instance.span_half),
        ],
    }
}

fn bridge_support_block(seed: u64, index: usize, dy: i32) -> BlockState {
    let hash = hash_coords(index as i32, dy, seed.rotate_left(107));
    match hash % 6 {
        0 => BlockState::POLISHED_BASALT,
        1 => BlockState::BLACKSTONE,
        2 => BlockState::BASALT,
        3 => BlockState::STONE_BRICKS,
        _ => BlockState::POLISHED_BLACKSTONE,
    }
}

fn bridge_deck_block(seed: u64, along: i32, cross: i32) -> BlockState {
    let hash = hash_coords(along, cross, seed.rotate_left(113));
    match hash % 7 {
        0 => BlockState::SPRUCE_PLANKS,
        1 => BlockState::DARK_OAK_PLANKS,
        2 => BlockState::SMOOTH_BASALT,
        3 => BlockState::POLISHED_BASALT,
        _ => BlockState::BASALT,
    }
}

fn rasterize_altar_platform(
    placements: &mut HashMap<(i32, i32, i32), Placement>,
    chunk_bounds: &ChunkBounds,
    instance: &BrokenAltarInstance,
) {
    let radius = instance.platform_radius;
    let floor_y = instance.base_y - 1;
    for dz in -radius..=radius {
        for dx in -radius..=radius {
            let ring = dx.abs().max(dz.abs());
            if ring == radius
                && sample_u01(hash_coords(dx, dz, instance.seed.rotate_left(3))) > 0.92
            {
                continue;
            }

            upsert_block(
                placements,
                chunk_bounds,
                instance.origin_x + dx,
                floor_y,
                instance.origin_z + dz,
                altar_floor_block(instance.seed, dx, dz),
                5,
            );

            if ring == radius {
                let side = edge_side(dx, dz);
                if let Some(side) = side {
                    let facing = facing_for_side(side);
                    if sample_u01(hash_coords(dx, dz, instance.seed.rotate_left(5))) < 0.72 {
                        upsert_block(
                            placements,
                            chunk_bounds,
                            instance.origin_x + dx,
                            instance.base_y,
                            instance.origin_z + dz,
                            altar_edge_stair(facing),
                            4,
                        );
                    }
                }
            }
        }
    }
}

fn rasterize_altar_dais(
    placements: &mut HashMap<(i32, i32, i32), Placement>,
    chunk_bounds: &ChunkBounds,
    instance: &BrokenAltarInstance,
) {
    let radius = instance.dais_radius;
    for dz in -radius..=radius {
        for dx in -radius..=radius {
            if dx.abs() == radius
                && dz.abs() == radius
                && sample_u01(hash_coords(dx, dz, instance.seed.rotate_left(11))) > 0.35
            {
                continue;
            }

            upsert_block(
                placements,
                chunk_bounds,
                instance.origin_x + dx,
                instance.base_y,
                instance.origin_z + dz,
                altar_dais_block(instance.seed, dx, dz),
                6,
            );
        }
    }
}

fn rasterize_altar_posts(
    placements: &mut HashMap<(i32, i32, i32), Placement>,
    chunk_bounds: &ChunkBounds,
    instance: &BrokenAltarInstance,
) {
    let offset = instance.platform_radius + instance.corner_post_radius;
    let post_positions = [
        (-offset, -offset),
        (offset, -offset),
        (-offset, offset),
        (offset, offset),
    ];
    for (index, (x, z)) in post_positions.into_iter().enumerate() {
        let post_hash = instance.seed.rotate_left(index as u32 + 7);
        let broken = sample_u01(post_hash) < 0.55;
        let height = if broken {
            (instance.corner_post_height - 1).max(1)
        } else {
            instance.corner_post_height
        };
        for dy in 0..height {
            let world_y = instance.base_y + 1 + dy;
            for dz in -instance.corner_post_radius..=instance.corner_post_radius {
                for dx in -instance.corner_post_radius..=instance.corner_post_radius {
                    upsert_block(
                        placements,
                        chunk_bounds,
                        instance.origin_x + x + dx,
                        world_y,
                        instance.origin_z + z + dz,
                        altar_post_block(post_hash, dx, dz, dy),
                        5,
                    );
                }
            }
        }

        if !broken {
            upsert_block(
                placements,
                chunk_bounds,
                instance.origin_x + x,
                instance.base_y + 1 + height,
                instance.origin_z + z,
                BlockState::POLISHED_BLACKSTONE_BRICK_SLAB.set(PropName::Type, PropValue::Bottom),
                4,
            );
        }
    }
}

fn rasterize_altar_core(
    placements: &mut HashMap<(i32, i32, i32), Placement>,
    chunk_bounds: &ChunkBounds,
    instance: &BrokenAltarInstance,
) {
    let core_y = instance.base_y + 1;
    upsert_block(
        placements,
        chunk_bounds,
        instance.origin_x,
        core_y,
        instance.origin_z,
        if sample_u01(instance.seed.rotate_left(41)) < 0.45 {
            BlockState::CRYING_OBSIDIAN
        } else {
            BlockState::OBSIDIAN
        },
        7,
    );

    for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        let hash = hash_coords(dx, dz, instance.seed.rotate_left(47));
        if sample_u01(hash) > 0.7 {
            continue;
        }
        upsert_block(
            placements,
            chunk_bounds,
            instance.origin_x + dx,
            instance.base_y + 2,
            instance.origin_z + dz,
            candle_state(1 + (hash % 2) as u16, sample_u01(hash.rotate_left(9)) < 0.5),
            4,
        );
    }

    if sample_u01(instance.seed.rotate_left(53)) < 0.35 {
        upsert_block(
            placements,
            chunk_bounds,
            instance.origin_x,
            instance.base_y + 2,
            instance.origin_z,
            BlockState::STONE_PRESSURE_PLATE,
            4,
        );
    }
}

fn rasterize_altar_rubble(
    placements: &mut HashMap<(i32, i32, i32), Placement>,
    chunk_bounds: &ChunkBounds,
    instance: &BrokenAltarInstance,
) {
    let radius = instance.platform_radius + 4;
    for index in 0..(4 + instance.platform_radius as usize) {
        let angle =
            sample_u01(instance.seed.rotate_left(index as u32 + 19)) * std::f64::consts::TAU;
        let distance = 2
            + (sample_u01(instance.seed.rotate_left(index as u32 + 29)) * radius as f64).round()
                as i32;
        let world_x = instance.origin_x + (angle.cos() * distance as f64).round() as i32;
        let world_z = instance.origin_z + (angle.sin() * distance as f64).round() as i32;
        let world_y = instance.base_y - 1;
        let hash = hash_coords(world_x, world_z, instance.seed + index as u64);
        let block = match hash % 6 {
            0 => BlockState::BLACKSTONE,
            1 => BlockState::POLISHED_BLACKSTONE,
            2 => BlockState::POLISHED_BLACKSTONE_BRICK_SLAB.set(PropName::Type, PropValue::Bottom),
            3 => BlockState::POLISHED_BLACKSTONE_BRICK_WALL,
            _ => BlockState::COBBLESTONE,
        };
        upsert_block(
            placements,
            chunk_bounds,
            world_x,
            world_y,
            world_z,
            block,
            2,
        );
    }
}

fn rasterize_foundation(
    placements: &mut HashMap<(i32, i32, i32), Placement>,
    chunk_bounds: &ChunkBounds,
    instance: &RuinPillarInstance,
) {
    let radius = instance.footprint_radius;
    for dz in -radius..=radius {
        for dx in -radius..=radius {
            let ring = dx.abs().max(dz.abs());
            let floor_y = instance.base_y - 1;
            upsert_block(
                placements,
                chunk_bounds,
                instance.origin_x + dx,
                floor_y,
                instance.origin_z + dz,
                base_block(instance.seed, dx, dz, ring),
                4,
            );

            if ring == radius && (dx.abs() != dz.abs() || radius == 1) {
                upsert_block(
                    placements,
                    chunk_bounds,
                    instance.origin_x + dx,
                    instance.base_y,
                    instance.origin_z + dz,
                    parapet_block(instance.seed, dx, dz),
                    3,
                );
            }
        }
    }
}

fn rasterize_column(
    placements: &mut HashMap<(i32, i32, i32), Placement>,
    chunk_bounds: &ChunkBounds,
    instance: &RuinPillarInstance,
) {
    let radius = instance.section_radius;
    for y_offset in 0..instance.column_height {
        let world_y = instance.base_y + y_offset;
        let chipped_side = range_i32(instance.seed.rotate_left((y_offset as u32) & 31), 0, 3);
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                if radius > 0 && dx.abs() == radius && dz.abs() == radius {
                    continue;
                }
                if should_skip_for_damage(dx, dz, radius, chipped_side, y_offset, instance) {
                    continue;
                }
                upsert_block(
                    placements,
                    chunk_bounds,
                    instance.origin_x + dx,
                    world_y,
                    instance.origin_z + dz,
                    shaft_block(instance.seed, dx, dz, y_offset),
                    5,
                );
            }
        }
    }
}

fn rasterize_crown(
    placements: &mut HashMap<(i32, i32, i32), Placement>,
    chunk_bounds: &ChunkBounds,
    instance: &RuinPillarInstance,
) {
    let crown_base_y = instance.base_y + instance.column_height;
    let crown_radius = instance.section_radius + instance.crown_overhang;
    for layer in 0..instance.crown_height {
        let world_y = crown_base_y + layer;
        for dz in -crown_radius..=crown_radius {
            for dx in -crown_radius..=crown_radius {
                if dx.abs() == crown_radius
                    && dz.abs() == crown_radius
                    && sample_u01(hash_coords(
                        dx,
                        dz,
                        instance.seed.rotate_left(7) + layer as u64,
                    )) > 0.25
                {
                    continue;
                }
                if sample_u01(hash_coords(
                    dx,
                    dz,
                    instance.seed.rotate_left(13) + layer as u64,
                )) > 0.86
                {
                    continue;
                }
                upsert_block(
                    placements,
                    chunk_bounds,
                    instance.origin_x + dx,
                    world_y,
                    instance.origin_z + dz,
                    crown_block(instance.seed, dx, dz, layer),
                    4,
                );
            }
        }
    }

    for arm in 0..4 {
        if sample_u01(instance.seed.rotate_left(19 + arm)) > 0.55 {
            continue;
        }
        let (dx, dz, facing) = match arm {
            0 => (0, -(crown_radius + 1), PropValue::North),
            1 => (0, crown_radius + 1, PropValue::South),
            2 => (-(crown_radius + 1), 0, PropValue::West),
            _ => (crown_radius + 1, 0, PropValue::East),
        };
        upsert_block(
            placements,
            chunk_bounds,
            instance.origin_x + dx,
            crown_base_y,
            instance.origin_z + dz,
            stair_cap(facing),
            3,
        );
    }
}

fn rasterize_fragments(
    placements: &mut HashMap<(i32, i32, i32), Placement>,
    chunk_bounds: &ChunkBounds,
    instance: &RuinPillarInstance,
) {
    let radius = instance.footprint_radius + instance.crown_overhang + 3;
    for index in 0..instance.fragment_count {
        let angle =
            sample_u01(instance.seed.rotate_left((index as u32) + 9)) * std::f64::consts::TAU;
        let distance = 2
            + (sample_u01(instance.seed.rotate_left((index as u32) + 23)) * radius as f64).round()
                as i32;
        let world_x = instance.origin_x + (angle.cos() * distance as f64).round() as i32;
        let world_z = instance.origin_z + (angle.sin() * distance as f64).round() as i32;
        let world_y = instance.base_y - 1;
        let hash = hash_coords(world_x, world_z, instance.seed + index as u64);
        let block = if sample_u01(hash) < 0.4 {
            BlockState::COBBLESTONE_SLAB.set(PropName::Type, PropValue::Bottom)
        } else if sample_u01(hash.rotate_left(7)) < 0.35 {
            COBBLESTONE_WALL
        } else {
            rubble_block(hash)
        };
        upsert_block(
            placements,
            chunk_bounds,
            world_x,
            world_y,
            world_z,
            block,
            2,
        );

        if sample_u01(hash.rotate_left(17)) < 0.22 {
            upsert_block(
                placements,
                chunk_bounds,
                world_x,
                world_y + 1,
                world_z,
                BlockState::MOSSY_COBBLESTONE,
                2,
            );
        }
    }
}

fn should_skip_for_damage(
    dx: i32,
    dz: i32,
    radius: i32,
    chipped_side: i32,
    y_offset: i32,
    instance: &RuinPillarInstance,
) -> bool {
    if y_offset < instance.column_height - 3 {
        return false;
    }
    let on_edge = if radius == 0 {
        true
    } else {
        dx.abs() == radius || dz.abs() == radius
    };
    if !on_edge {
        return false;
    }
    match chipped_side {
        0 => dz < 0,
        1 => dz > 0,
        2 => dx < 0,
        _ => dx > 0,
    }
}

fn shaft_block(seed: u64, dx: i32, dz: i32, y_offset: i32) -> BlockState {
    let hash = hash_coords(dx + y_offset, dz - y_offset, seed.rotate_left(3));
    match hash % 11 {
        0 => BlockState::MOSSY_STONE_BRICKS,
        1 | 2 => BlockState::CRACKED_STONE_BRICKS,
        3 => CHISELED_STONE_BRICKS,
        4 => BlockState::COBBLESTONE,
        _ => BlockState::STONE_BRICKS,
    }
}

fn base_block(seed: u64, dx: i32, dz: i32, ring: i32) -> BlockState {
    let hash = hash_coords(dx + ring, dz - ring, seed.rotate_left(5));
    if ring == 0 && hash % 5 == 0 {
        CHISELED_STONE_BRICKS
    } else if hash % 4 == 0 {
        BlockState::MOSSY_COBBLESTONE
    } else {
        rubble_block(hash)
    }
}

fn parapet_block(seed: u64, dx: i32, dz: i32) -> BlockState {
    let hash = hash_coords(dx, dz, seed.rotate_left(9));
    if hash % 5 == 0 {
        COBBLESTONE_WALL
    } else if hash % 3 == 0 {
        BlockState::MOSSY_COBBLESTONE
    } else {
        BlockState::STONE_BRICKS
    }
}

fn crown_block(seed: u64, dx: i32, dz: i32, layer: i32) -> BlockState {
    let hash = hash_coords(dx + layer, dz - layer, seed.rotate_left(15));
    if layer == 0 && hash % 4 == 0 {
        BlockState::STONE_BRICK_SLAB.set(PropName::Type, PropValue::Bottom)
    } else {
        shaft_block(seed.rotate_left(21), dx, dz, layer)
    }
}

fn rubble_block(hash: u64) -> BlockState {
    match hash % 5 {
        0 => BlockState::MOSSY_COBBLESTONE,
        1 => BlockState::COBBLESTONE,
        2 => BlockState::MOSSY_STONE_BRICKS,
        _ => BlockState::STONE_BRICKS,
    }
}

fn stair_cap(facing: PropValue) -> BlockState {
    BlockState::STONE_BRICK_STAIRS
        .set(PropName::Facing, facing)
        .set(PropName::Half, PropValue::Bottom)
        .set(PropName::Shape, PropValue::Straight)
        .set(PropName::Waterlogged, PropValue::False)
}

fn altar_edge_stair(facing: PropValue) -> BlockState {
    BlockState::POLISHED_BLACKSTONE_BRICK_STAIRS
        .set(PropName::Facing, facing)
        .set(PropName::Half, PropValue::Bottom)
        .set(PropName::Shape, PropValue::Straight)
        .set(PropName::Waterlogged, PropValue::False)
}

fn altar_floor_block(seed: u64, dx: i32, dz: i32) -> BlockState {
    let hash = hash_coords(dx, dz, seed.rotate_left(3));
    match hash % 7 {
        0 => BlockState::BLACKSTONE,
        1 => BlockState::POLISHED_BLACKSTONE,
        2 => BlockState::CRACKED_POLISHED_BLACKSTONE_BRICKS,
        3 => BlockState::CHISELED_POLISHED_BLACKSTONE,
        _ => BlockState::POLISHED_BLACKSTONE_BRICKS,
    }
}

fn altar_dais_block(seed: u64, dx: i32, dz: i32) -> BlockState {
    let hash = hash_coords(dx, dz, seed.rotate_left(13));
    if hash % 5 == 0 {
        BlockState::CHISELED_POLISHED_BLACKSTONE
    } else if hash % 3 == 0 {
        BlockState::CRACKED_POLISHED_BLACKSTONE_BRICKS
    } else {
        BlockState::POLISHED_BLACKSTONE_BRICKS
    }
}

fn altar_post_block(seed: u64, dx: i32, dz: i32, dy: i32) -> BlockState {
    let hash = hash_coords(dx + dy, dz - dy, seed.rotate_left(21));
    match hash % 6 {
        0 => BlockState::GILDED_BLACKSTONE,
        1 => BlockState::CRACKED_POLISHED_BLACKSTONE_BRICKS,
        2 => BlockState::CHISELED_POLISHED_BLACKSTONE,
        _ => BlockState::POLISHED_BLACKSTONE_BRICKS,
    }
}

fn edge_side(dx: i32, dz: i32) -> Option<u8> {
    if dz < 0 && dz.abs() >= dx.abs() {
        Some(0)
    } else if dz > 0 && dz.abs() >= dx.abs() {
        Some(1)
    } else if dx < 0 {
        Some(2)
    } else if dx > 0 {
        Some(3)
    } else {
        None
    }
}

fn facing_for_side(side: u8) -> PropValue {
    match side {
        0 => PropValue::North,
        1 => PropValue::South,
        2 => PropValue::West,
        _ => PropValue::East,
    }
}

fn candle_state(count: u16, lit: bool) -> BlockState {
    let candles = match count.clamp(1, 4) {
        1 => PropValue::_1,
        2 => PropValue::_2,
        3 => PropValue::_3,
        _ => PropValue::_4,
    };
    BlockState::CANDLE
        .set(PropName::Candles, candles)
        .set(
            PropName::Lit,
            if lit {
                PropValue::True
            } else {
                PropValue::False
            },
        )
        .set(PropName::Waterlogged, PropValue::False)
}

fn surface_slope(
    world_x: i32,
    world_z: i32,
    min_y: i32,
    world_height: i32,
    terrain: &TerrainProvider,
) -> i32 {
    let offsets = [(0, 0), (4, 0), (-4, 0), (0, 4), (0, -4)];
    let mut min_surface = i32::MAX;
    let mut max_surface = i32::MIN;
    for (dx, dz) in offsets {
        let sample = terrain.sample(world_x + dx, world_z + dz);
        let surface = column::surface_y_for_sample(&sample, min_y, world_height);
        min_surface = min_surface.min(surface);
        max_surface = max_surface.max(surface);
    }
    max_surface - min_surface
}

fn upsert_block(
    placements: &mut HashMap<(i32, i32, i32), Placement>,
    chunk_bounds: &ChunkBounds,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    block: BlockState,
    priority: u8,
) {
    if !chunk_bounds.contains_with_margin(world_x, world_z, 18) {
        return;
    }

    let entry = placements
        .entry((world_x, world_y, world_z))
        .or_insert(Placement { block, priority });
    if priority >= entry.priority {
        *entry = Placement { block, priority };
    }
}

fn can_replace(existing: BlockState, _incoming: BlockState, priority: u8) -> bool {
    if existing == BlockState::BEDROCK || existing == BlockState::WATER {
        return false;
    }

    // Never overwrite logs, leaves, or mega-tree blocks — these are expensive to generate
    if is_protected_natural(existing) {
        return false;
    }

    if priority >= 4 {
        return !is_structure_block(existing);
    }

    existing.is_air()
        || matches!(
            existing,
            BlockState::DEAD_BUSH
                | BlockState::FERN
                | BlockState::GRASS
                | BlockState::POPPY
                | BlockState::DANDELION
                | BlockState::MOSS_CARPET
        )
        || existing.wall_block_id().is_some()
}

fn is_protected_natural(block: BlockState) -> bool {
    matches!(
        block,
        BlockState::OAK_LOG
            | BlockState::SPRUCE_LOG
            | BlockState::DARK_OAK_LOG
            | BlockState::OAK_LEAVES
            | BlockState::SPRUCE_LEAVES
            | BlockState::DARK_OAK_LEAVES
            | BlockState::BAMBOO
    )
}

fn is_structure_block(block: BlockState) -> bool {
    matches!(
        block,
        BlockState::LODESTONE
            | BlockState::CRYING_OBSIDIAN
            | BlockState::OBSIDIAN
            | BlockState::END_ROD
            | BlockState::SOUL_LANTERN
            | BlockState::END_STONE_BRICKS
            | BlockState::PURPUR_BLOCK
            | BlockState::BUDDING_AMETHYST
            | BlockState::AMETHYST_BLOCK
    )
}

fn hash_coords(x: i32, z: i32, salt: u64) -> u64 {
    let mut value = (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    value ^= (z as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    value ^= salt.wrapping_mul(0x1656_67B1_9E37_79F9);
    value ^= value >> 33;
    value = value.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    value ^= value >> 33;
    value = value.wrapping_mul(0xC4CE_B9FE_1A85_EC53);
    value ^ (value >> 33)
}

fn sample_u01(seed: u64) -> f64 {
    let mantissa = seed >> 11;
    mantissa as f64 / ((1u64 << 53) as f64)
}

fn range_i32(seed: u64, min: i32, max: i32) -> i32 {
    if max <= min {
        return min;
    }
    let span = (max - min + 1) as u64;
    min + (seed % span) as i32
}

const COBBLESTONE_WALL: BlockState = BlockState::COBBLESTONE_WALL
    .set(PropName::Up, PropValue::True)
    .set(PropName::North, PropValue::None)
    .set(PropName::East, PropValue::None)
    .set(PropName::South, PropValue::None)
    .set(PropName::West, PropValue::None)
    .set(PropName::Waterlogged, PropValue::False);

const CHISELED_STONE_BRICKS: BlockState = BlockState::CHISELED_STONE_BRICKS;
