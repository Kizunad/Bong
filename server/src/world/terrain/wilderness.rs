use valence::prelude::{BiomeId, BlockState};

use super::raster::ColumnSample;

pub fn sample(
    world_x: i32,
    world_z: i32,
    plains_biome: BiomeId,
    forest_biome: BiomeId,
    river_biome: BiomeId,
) -> ColumnSample {
    let x = world_x as f64;
    let z = world_z as f64;

    let continental =
        (x / 2400.0).sin() * 8.5 + (z / 2700.0).cos() * 7.2 + ((x + z) / 3600.0).sin() * 5.8;
    let ridge = (x / 680.0).sin() * 4.2 + (z / 760.0).cos() * 3.6 + ((x - z) / 940.0).sin() * 2.9;
    let mountain = (x / 1200.0).sin() * (z / 1400.0).cos() * 3.8 + ((x + z) / 1800.0).sin() * 2.4;
    let drainage = 0.5
        + (x / 520.0).sin() * (z / 610.0).cos() * 0.22
        + ((x - z) / 870.0).sin() * 0.16
        + ((x + z) / 1040.0).cos() * 0.12;
    let scar = 0.5
        + ((x + z) / 760.0).sin() * ((x - z) / 690.0).cos() * 0.2
        + (x / 430.0).sin() * (z / 470.0).cos() * 0.14;

    let mut height = 70.0 + continental * 4.0 + ridge * 3.5 + mountain * 6.0;
    if drainage < 0.12 {
        height -= (0.12 - drainage) * 8.0;
    }
    if scar > 0.82 {
        height += (scar - 0.82) * 9.5;
    }

    let roughness = ridge.abs() * 0.13 + (scar - 0.5).abs() * 0.08;
    let feature_mask = (0.09 + continental.abs() * 0.08 + roughness * 0.58).min(1.0);
    let biome_id = if drainage < 0.09 {
        8
    } else if feature_mask > 0.2 {
        7
    } else {
        0
    };
    let biome = match biome_id {
        7 => forest_biome,
        8 => river_biome,
        _ => plains_biome,
    };

    let surface_block = if height < 76.0 && drainage > 0.18 && scar < 0.72 {
        BlockState::GRASS_BLOCK
    } else if drainage < 0.06 || scar > 0.84 {
        BlockState::GRAVEL
    } else if roughness < 0.07 {
        BlockState::COARSE_DIRT
    } else {
        BlockState::STONE
    };

    ColumnSample {
        height: height as f32,
        surface_block,
        subsurface_block: BlockState::STONE,
        biome_id,
        biome,
        water_level: -1.0,
        feature_mask: feature_mask as f32,
        boundary_weight: 0.0,
        rift_axis_sdf: 99.0,
        rim_edge_mask: 0.0,
        cave_mask: 0.0,
        ceiling_height: 0.0,
        entrance_mask: 0.0,
        fracture_mask: 0.0,
        neg_pressure: 0.0,
        ruin_density: 0.0,
        // Mofa wilderness baseline: thin qi, moderate decay, no vein / isles /
        // deep caves / flora variant / anomalies. Matches Python wilderness defaults
        // (see worldgen/scripts/terrain_gen/profiles/wilderness.py).
        qi_density: 0.12,
        mofa_decay: 0.40,
        qi_vein_flow: 0.0,
        sky_island_mask: 0.0,
        sky_island_base_y: 9999.0,
        sky_island_thickness: 0.0,
        underground_tier: 0,
        cavern_floor_y: 9999.0,
        flora_density: 0.0,
        flora_variant_id: 0,
        anomaly_intensity: 0.0,
        anomaly_kind: 0,
    }
}
