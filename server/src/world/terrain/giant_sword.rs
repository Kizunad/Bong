//! Giant Sword Sea (巨剑沧海) structure generator.
//!
//! Generates ancient swords of varying sizes embedded in the terrain — some
//! standing upright, some tilted, some broken and fallen. The zone is meant
//! to evoke a vast battlefield where countless blades remain planted in the
//! earth after an apocalyptic clash in the Age of Gods.

use std::collections::HashMap;

use valence::prelude::{BlockState, Chunk, ChunkPos, PropName, PropValue, UnloadedChunk};

use super::raster::TerrainProvider;
use super::spatial::ChunkBounds;
use super::{column, structures::StructureBounds};

const SWORD_SEA_MIN_X: i32 = 3800;
const SWORD_SEA_MAX_X: i32 = 5400;
const SWORD_SEA_MIN_Z: i32 = 800;
const SWORD_SEA_MAX_Z: i32 = 2400;

pub fn is_in_sword_sea(world_x: i32, world_z: i32) -> bool {
    (SWORD_SEA_MIN_X..=SWORD_SEA_MAX_X).contains(&world_x)
        && (SWORD_SEA_MIN_Z..=SWORD_SEA_MAX_Z).contains(&world_z)
}

fn chunk_overlaps_sword_sea(pos: ChunkPos) -> bool {
    let cx = pos.x * 16;
    let cz = pos.z * 16;
    cx + 16 >= SWORD_SEA_MIN_X
        && cx <= SWORD_SEA_MAX_X
        && cz + 16 >= SWORD_SEA_MIN_Z
        && cz <= SWORD_SEA_MAX_Z
}

#[derive(Clone, Copy, PartialEq)]
enum SwordVariant {
    Colossal,
    Large,
    Medium,
    Small,
    Broken,
    Fallen,
    Wooden,
}

#[derive(Clone, Copy)]
enum SwordOrientation {
    Upright,
    TiltedNorth,
    TiltedSouth,
    TiltedEast,
    TiltedWest,
    FallenX,
    FallenZ,
}

#[derive(Clone, Copy)]
struct SwordInstance {
    origin_x: i32,
    origin_z: i32,
    base_y: i32,
    seed: u64,
    variant: SwordVariant,
    orientation: SwordOrientation,
    blade_length: i32,
    blade_width: i32,
    guard_width: i32,
    grip_length: i32,
    bounds: StructureBounds,
}

pub fn decorate_chunk(
    chunk: &mut UnloadedChunk,
    pos: ChunkPos,
    min_y: i32,
    terrain: &TerrainProvider,
) {
    if !chunk_overlaps_sword_sea(pos) {
        return;
    }

    let bounds = ChunkBounds::from_chunk_pos(pos);
    let world_height = chunk.height() as i32;

    // Grid tiers: small/medium/wooden dense, large moderate, colossal sparse+huge
    for &(spacing, salt, variant_pool) in &[
        (18i32, 0xBEEF_5A0D_u64, 0u8),
        (40, 0xCAFE_5B0D, 1),
        (220, 0xDEAD_5C0D, 2),
    ] {
        let extent = spacing;
        let cell_min_x = (bounds.min_x - extent).div_euclid(spacing);
        let cell_max_x = (bounds.max_x + extent).div_euclid(spacing);
        let cell_min_z = (bounds.min_z - extent).div_euclid(spacing);
        let cell_max_z = (bounds.max_z + extent).div_euclid(spacing);

        for cell_z in cell_min_z..=cell_max_z {
            for cell_x in cell_min_x..=cell_max_x {
                let Some(instance) = instantiate_sword(
                    cell_x,
                    cell_z,
                    spacing,
                    salt,
                    variant_pool,
                    min_y,
                    world_height,
                    terrain,
                ) else {
                    continue;
                };
                if !instance.bounds.intersects_chunk(&bounds) {
                    continue;
                }
                place_sword_in_chunk(chunk, min_y, &bounds, &instance);
            }
        }
    }

    // Wind scatter: dead bushes, chains
    scatter_wind_debris(chunk, pos, min_y, terrain);


    // Unique landmark: giant crater with grey broken sword at zone center
    place_crater_sword_if_overlaps(chunk, pos, min_y, terrain);
}

fn scatter_wind_debris(
    chunk: &mut UnloadedChunk,
    pos: ChunkPos,
    min_y: i32,
    terrain: &TerrainProvider,
) {
    let world_height = chunk.height() as i32;
    for local_z in 0..16 {
        for local_x in 0..16 {
            let world_x = pos.x * 16 + local_x;
            let world_z = pos.z * 16 + local_z;
            if !is_in_sword_sea(world_x, world_z) {
                continue;
            }

            let sample = terrain.sample(world_x, world_z);
            let surface_y = column::surface_y_for_sample(&sample, min_y, world_height);
            let local_y = surface_y + 1 - min_y;
            if local_y < 0 || local_y >= world_height {
                continue;
            }

            let h = hash_coords(world_x, world_z, 0xA1AD_DE8B);
            let roll = h % 100;

            let block = if roll < 9 {
                // Dead bushes — windswept scrub
                BlockState::DEAD_BUSH
            } else if roll < 13 {
                // Chains — eerie wind chimes
                BlockState::CHAIN.set(PropName::Axis, PropValue::Y)
            } else {
                continue;
            };

            // Dead bush needs sand/dirt/terracotta below in vanilla,
            // but we place it raw — it renders fine visually even on deepslate
            let existing = chunk.block_state(local_x as u32, local_y as u32, local_z as u32);
            if existing.is_air() {
                chunk.set_block_state(local_x as u32, local_y as u32, local_z as u32, block);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn instantiate_sword(
    cell_x: i32,
    cell_z: i32,
    spacing: i32,
    salt: u64,
    variant_pool: u8,
    min_y: i32,
    world_height: i32,
    terrain: &TerrainProvider,
) -> Option<SwordInstance> {
    let base_seed = hash_coords(cell_x, cell_z, salt);
    let margin = spacing / 5;
    let span = spacing - margin * 2;
    if span <= 0 {
        return None;
    }

    let seed_x = cell_x * spacing + margin + range_i32(base_seed, 0, span - 1);
    let seed_z = cell_z * spacing + margin + range_i32(base_seed.rotate_left(17), 0, span - 1);

    if !is_in_sword_sea(seed_x, seed_z) {
        return None;
    }

    let sample = terrain.sample(seed_x, seed_z);
    let surface_y = column::surface_y_for_sample(&sample, min_y, world_height);
    if !(58..=120).contains(&surface_y) {
        return None;
    }

    // Placement chance varies by tier
    let chance = match variant_pool {
        0 => 0.75, // small/medium: very dense
        1 => 0.60, // large: moderate-dense
        _ => 0.40, // colossal: sparse but impressive
    };
    if sample_u01(base_seed.rotate_left(29)) > chance {
        return None;
    }

    let (variant, orientation) = choose_variant_and_orientation(base_seed, variant_pool);

    // blade_width = half-width on X-axis. Swords are narrow and flat.
    let (blade_length, blade_width, guard_width, grip_length) = match variant {
        SwordVariant::Colossal => (
            range_i32(base_seed.rotate_left(5), 90, 150),
            range_i32(base_seed.rotate_left(9), 2, 4),
            range_i32(base_seed.rotate_left(13), 18, 28),
            range_i32(base_seed.rotate_left(17), 15, 25),
        ),
        SwordVariant::Large => (
            range_i32(base_seed.rotate_left(5), 25, 45),
            range_i32(base_seed.rotate_left(9), 1, 3),
            range_i32(base_seed.rotate_left(13), 6, 10),
            range_i32(base_seed.rotate_left(17), 5, 9),
        ),
        SwordVariant::Medium => (
            range_i32(base_seed.rotate_left(5), 14, 24),
            range_i32(base_seed.rotate_left(9), 1, 2),
            range_i32(base_seed.rotate_left(13), 4, 6),
            range_i32(base_seed.rotate_left(17), 3, 5),
        ),
        SwordVariant::Small => (
            range_i32(base_seed.rotate_left(5), 6, 13),
            1,
            range_i32(base_seed.rotate_left(13), 2, 3),
            range_i32(base_seed.rotate_left(17), 2, 3),
        ),
        SwordVariant::Broken => (
            range_i32(base_seed.rotate_left(5), 10, 25),
            range_i32(base_seed.rotate_left(9), 1, 2),
            range_i32(base_seed.rotate_left(13), 3, 7),
            range_i32(base_seed.rotate_left(17), 2, 5),
        ),
        SwordVariant::Fallen => (
            range_i32(base_seed.rotate_left(5), 20, 50),
            range_i32(base_seed.rotate_left(9), 1, 3),
            range_i32(base_seed.rotate_left(13), 5, 9),
            range_i32(base_seed.rotate_left(17), 4, 7),
        ),
        SwordVariant::Wooden => (
            range_i32(base_seed.rotate_left(5), 8, 18),
            1,
            range_i32(base_seed.rotate_left(13), 2, 4),
            range_i32(base_seed.rotate_left(17), 3, 5),
        ),
    };

    let total_height = match orientation {
        SwordOrientation::FallenX | SwordOrientation::FallenZ => blade_width + 3,
        _ => blade_length + guard_width + grip_length + 2,
    };
    let horizontal_extent = match orientation {
        SwordOrientation::FallenX | SwordOrientation::FallenZ => {
            (blade_length + grip_length) / 2 + guard_width
        }
        _ => guard_width / 2 + blade_width + 2,
    };

    let _ = total_height;

    Some(SwordInstance {
        origin_x: seed_x,
        origin_z: seed_z,
        base_y: surface_y,
        seed: base_seed,
        variant,
        orientation,
        blade_length,
        blade_width,
        guard_width,
        grip_length,
        bounds: StructureBounds {
            min_x: seed_x - horizontal_extent,
            max_x: seed_x + horizontal_extent,
            min_z: seed_z - horizontal_extent,
            max_z: seed_z + horizontal_extent,
        },
    })
}

fn choose_variant_and_orientation(seed: u64, pool: u8) -> (SwordVariant, SwordOrientation) {
    let variant_roll = sample_u01(seed.rotate_left(41));
    let orient_roll = sample_u01(seed.rotate_left(47));

    let variant = match pool {
        0 => {
            // Small pool: small, medium, wooden, broken mix
            if variant_roll < 0.25 {
                SwordVariant::Small
            } else if variant_roll < 0.45 {
                SwordVariant::Medium
            } else if variant_roll < 0.65 {
                SwordVariant::Wooden
            } else if variant_roll < 0.82 {
                SwordVariant::Broken
            } else {
                SwordVariant::Fallen
            }
        }
        1 => {
            // Large pool
            if variant_roll < 0.40 {
                SwordVariant::Large
            } else if variant_roll < 0.60 {
                SwordVariant::Medium
            } else if variant_roll < 0.75 {
                SwordVariant::Wooden
            } else if variant_roll < 0.88 {
                SwordVariant::Broken
            } else {
                SwordVariant::Fallen
            }
        }
        _ => {
            // Colossal pool — only the biggest
            if variant_roll < 0.65 {
                SwordVariant::Colossal
            } else if variant_roll < 0.85 {
                SwordVariant::Large
            } else {
                SwordVariant::Fallen
            }
        }
    };

    let orientation = match variant {
        SwordVariant::Fallen => {
            if orient_roll < 0.5 {
                SwordOrientation::FallenX
            } else {
                SwordOrientation::FallenZ
            }
        }
        _ => {
            if orient_roll < 0.50 {
                SwordOrientation::Upright
            } else if orient_roll < 0.62 {
                SwordOrientation::TiltedNorth
            } else if orient_roll < 0.74 {
                SwordOrientation::TiltedSouth
            } else if orient_roll < 0.87 {
                SwordOrientation::TiltedEast
            } else {
                SwordOrientation::TiltedWest
            }
        }
    };

    (variant, orientation)
}

fn place_sword_in_chunk(
    chunk: &mut UnloadedChunk,
    min_y: i32,
    chunk_bounds: &ChunkBounds,
    instance: &SwordInstance,
) {
    let mut placements: HashMap<(i32, i32, i32), Placement> = HashMap::new();

    match instance.orientation {
        SwordOrientation::FallenX | SwordOrientation::FallenZ => {
            rasterize_fallen_sword(&mut placements, chunk_bounds, instance);
        }
        _ => {
            rasterize_standing_sword(&mut placements, chunk_bounds, instance);
        }
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
        if existing == BlockState::BEDROCK {
            continue;
        }
        chunk.set_block_state(local_x, local_y as u32, local_z, placement.block);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Standing sword (upright or tilted) — rendered vertically
// ─────────────────────────────────────────────────────────────────────────────

fn rasterize_standing_sword(
    placements: &mut HashMap<(i32, i32, i32), Placement>,
    chunk_bounds: &ChunkBounds,
    inst: &SwordInstance,
) {
    let tilt = match inst.orientation {
        SwordOrientation::TiltedNorth => (0, -1),
        SwordOrientation::TiltedSouth => (0, 1),
        SwordOrientation::TiltedEast => (1, 0),
        SwordOrientation::TiltedWest => (-1, 0),
        _ => (0, 0),
    };
    let tilt_period = if tilt == (0, 0) { i32::MAX } else { 5 };

    // Embed the blade partially underground for stability
    let embed_depth = (inst.blade_length / 4).max(2);
    let blade_start_y = inst.base_y - embed_depth;

    // --- Blade ---
    let blade_top_y = blade_start_y + inst.blade_length;
    for dy in 0..inst.blade_length {
        let y = blade_start_y + dy;
        let tilt_offset_x = if tilt_period < i32::MAX {
            (dy / tilt_period) * tilt.0
        } else {
            0
        };
        let tilt_offset_z = if tilt_period < i32::MAX {
            (dy / tilt_period) * tilt.1
        } else {
            0
        };
        let cx = inst.origin_x + tilt_offset_x;
        let cz = inst.origin_z + tilt_offset_z;

        // Blade narrows toward tip (last 30% tapers)
        let progress = dy as f32 / inst.blade_length as f32;
        let width = if progress > 0.7 {
            let taper = 1.0 - (progress - 0.7) / 0.3;
            ((inst.blade_width as f32 * taper).round() as i32).max(1)
        } else {
            inst.blade_width
        };

        // Blade is flat: 1 block thick (Z=0 only). Only colossal gets Z=-1..=1.
        let z_extent: i32 = if inst.variant == SwordVariant::Colossal { 1 } else { 0 };

        for dx in -width..=width {
            for dz in -z_extent..=z_extent {
                // Edges are always single-thick even on colossal
                if dz != 0 && dx.abs() >= width {
                    continue;
                }

                let block = if inst.variant == SwordVariant::Wooden {
                    wooden_blade_block(inst.seed, y, progress)
                } else {
                    blade_block(inst.seed, y, dx, progress)
                };
                upsert(placements, chunk_bounds, cx + dx, y, cz + dz, block);
            }
        }

        // Edge highlight: iron bars on the very edge of wider blades
        if width >= 2 && dy > 1 && progress < 0.8 {
            if sample_u01(hash_coords(cx + width + 1, y, inst.seed)) < 0.3 {
                upsert(
                    placements,
                    chunk_bounds,
                    cx + width + 1,
                    y,
                    cz,
                    BlockState::IRON_BARS,
                );
            }
            if sample_u01(hash_coords(cx - width - 1, y, inst.seed.rotate_left(3))) < 0.3 {
                upsert(
                    placements,
                    chunk_bounds,
                    cx - width - 1,
                    y,
                    cz,
                    BlockState::IRON_BARS,
                );
            }
        }
    }

    // --- Crossguard (tsuba / 锷) ---
    let guard_y = blade_top_y;
    let guard_tilt_x = if tilt_period < i32::MAX {
        (inst.blade_length / tilt_period) * tilt.0
    } else {
        0
    };
    let guard_tilt_z = if tilt_period < i32::MAX {
        (inst.blade_length / tilt_period) * tilt.1
    } else {
        0
    };
    let guard_cx = inst.origin_x + guard_tilt_x;
    let guard_cz = inst.origin_z + guard_tilt_z;

    // Guard: thin horizontal bar (1 block tall, tapered diamond on Z)
    let half_w = inst.guard_width / 2;
    for dx in -half_w..=half_w {
        // Taper: z-extent shrinks toward tips
        let dist_ratio = dx.abs() as f32 / half_w.max(1) as f32;
        let z_extent = if dist_ratio > 0.7 { 0 } else { 1 };
        for dz in -z_extent..=z_extent {
            let block = guard_block(inst.seed, dx, dz);
            upsert(
                placements,
                chunk_bounds,
                guard_cx + dx,
                guard_y,
                guard_cz + dz,
                block,
            );
        }
    }

    // --- Grip (handle / 柄) ---
    let grip_start_y = guard_y + 1;
    for dy in 0..inst.grip_length {
        let y = grip_start_y + dy;
        let offset_above_blade = inst.blade_length + dy + 2;
        let g_tilt_x = if tilt_period < i32::MAX {
            (offset_above_blade / tilt_period) * tilt.0
        } else {
            0
        };
        let g_tilt_z = if tilt_period < i32::MAX {
            (offset_above_blade / tilt_period) * tilt.1
        } else {
            0
        };
        let gx = inst.origin_x + g_tilt_x;
        let gz = inst.origin_z + g_tilt_z;

        let grip_radius = match inst.variant {
            SwordVariant::Colossal => 3,
            SwordVariant::Large => 1,
            _ => 0,
        };
        for dx in -grip_radius..=grip_radius {
            for dz in -grip_radius..=grip_radius {
                let block = grip_block(inst.seed, dy);
                upsert(placements, chunk_bounds, gx + dx, y, gz + dz, block);
            }
        }

        // Wrapping texture: occasional dark oak fences for tsuka-ito (柄糸)
        if dy % 2 == 0
            && grip_radius == 0
            && sample_u01(hash_coords(gx, y, inst.seed.rotate_left(53))) < 0.4
        {
            upsert(
                placements,
                chunk_bounds,
                gx + 1,
                y,
                gz,
                BlockState::DARK_OAK_FENCE,
            );
            upsert(
                placements,
                chunk_bounds,
                gx - 1,
                y,
                gz,
                BlockState::DARK_OAK_FENCE,
            );
        }
    }

    // --- Pommel (柄头) ---
    let pommel_y = grip_start_y + inst.grip_length;
    let pommel_offset = inst.blade_length + inst.grip_length + 2;
    let p_tilt_x = if tilt_period < i32::MAX {
        (pommel_offset / tilt_period) * tilt.0
    } else {
        0
    };
    let p_tilt_z = if tilt_period < i32::MAX {
        (pommel_offset / tilt_period) * tilt.1
    } else {
        0
    };
    let px = inst.origin_x + p_tilt_x;
    let pz = inst.origin_z + p_tilt_z;
    let pommel_r = if inst.variant == SwordVariant::Colossal {
        5
    } else if inst.variant == SwordVariant::Large {
        2
    } else if inst.blade_length > 20 {
        1
    } else {
        0
    };
    for dx in -pommel_r..=pommel_r {
        for dz in -pommel_r..=pommel_r {
            if dx * dx + dz * dz > pommel_r * pommel_r + 1 {
                continue;
            }
            upsert(
                placements,
                chunk_bounds,
                px + dx,
                pommel_y,
                pz + dz,
                pommel_block(inst.seed),
            );
        }
    }
    // Pommel cap
    upsert(
        placements,
        chunk_bounds,
        px,
        pommel_y + 1,
        pz,
        BlockState::LIGHTNING_ROD,
    );

    // --- Broken variant: cut the blade short and scatter debris ---
    if inst.variant == SwordVariant::Broken {
        rasterize_debris(placements, chunk_bounds, inst);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Fallen sword — rendered horizontally
// ─────────────────────────────────────────────────────────────────────────────

fn rasterize_fallen_sword(
    placements: &mut HashMap<(i32, i32, i32), Placement>,
    chunk_bounds: &ChunkBounds,
    inst: &SwordInstance,
) {
    let along_x = matches!(inst.orientation, SwordOrientation::FallenX);
    let half_blade = inst.blade_length / 2;
    let base_y = inst.base_y + 1;

    // --- Blade (horizontal) ---
    for along in -half_blade..=half_blade {
        let progress = (along + half_blade) as f32 / inst.blade_length as f32;
        let width = if progress > 0.8 {
            let taper = 1.0 - (progress - 0.8) / 0.2;
            ((inst.blade_width as f32 * taper).round() as i32).max(1)
        } else if progress < 0.1 {
            1
        } else {
            inst.blade_width
        };

        for cross in -width..=width {
            let (wx, wz) = if along_x {
                (inst.origin_x + along, inst.origin_z + cross)
            } else {
                (inst.origin_x + cross, inst.origin_z + along)
            };

            // Stack height: blade is 1-2 blocks tall when lying down
            let stack = if width >= 2 && cross.abs() < width {
                2
            } else {
                1
            };
            for dy in 0..stack {
                let block = blade_block(inst.seed, base_y + dy, along, progress);
                upsert(placements, chunk_bounds, wx, base_y + dy, wz, block);
            }
        }
    }

    // --- Crossguard (vertical, sticking up from the ground) ---
    let guard_along = half_blade + 1;
    let (gcx, gcz) = if along_x {
        (inst.origin_x + guard_along, inst.origin_z)
    } else {
        (inst.origin_x, inst.origin_z + guard_along)
    };
    let guard_half = inst.guard_width / 2;
    for cross in -guard_half..=guard_half {
        for dy in 0..3 {
            let (gx, gz) = if along_x {
                (gcx, gcz + cross)
            } else {
                (gcx + cross, gcz)
            };
            let block = guard_block(inst.seed, cross, dy);
            upsert(placements, chunk_bounds, gx, base_y + dy, gz, block);
        }
    }

    // --- Grip (horizontal, continuing past guard) ---
    let grip_start = guard_along + 2;
    for along in 0..inst.grip_length {
        let (gx, gz) = if along_x {
            (inst.origin_x + grip_start + along, inst.origin_z)
        } else {
            (inst.origin_x, inst.origin_z + grip_start + along)
        };
        let block = grip_block(inst.seed, along);
        upsert(placements, chunk_bounds, gx, base_y, gz, block);
        // Wrapping
        if along % 2 == 0 {
            let (wx, wz) = if along_x { (gx, gz + 1) } else { (gx + 1, gz) };
            upsert(
                placements,
                chunk_bounds,
                wx,
                base_y,
                wz,
                BlockState::DARK_OAK_FENCE,
            );
        }
    }

    // --- Pommel ---
    let pommel_along = grip_start + inst.grip_length;
    let (pmx, pmz) = if along_x {
        (inst.origin_x + pommel_along, inst.origin_z)
    } else {
        (inst.origin_x, inst.origin_z + pommel_along)
    };
    upsert(
        placements,
        chunk_bounds,
        pmx,
        base_y,
        pmz,
        pommel_block(inst.seed),
    );
    upsert(
        placements,
        chunk_bounds,
        pmx,
        base_y + 1,
        pmz,
        pommel_block(inst.seed),
    );

    // --- Ground impact crater around fallen blade ---
    rasterize_impact_crater(placements, chunk_bounds, inst);
}

fn rasterize_debris(
    placements: &mut HashMap<(i32, i32, i32), Placement>,
    chunk_bounds: &ChunkBounds,
    inst: &SwordInstance,
) {
    let debris_count = range_i32(inst.seed.rotate_left(61), 4, 10) as usize;
    for i in 0..debris_count {
        let angle = sample_u01(inst.seed.rotate_left(i as u32 + 63)) * std::f64::consts::TAU;
        let dist = 2.0 + sample_u01(inst.seed.rotate_left(i as u32 + 67)) * 8.0;
        let dx = (angle.cos() * dist).round() as i32;
        let dz = (angle.sin() * dist).round() as i32;
        let wx = inst.origin_x + dx;
        let wz = inst.origin_z + dz;

        let height = range_i32(inst.seed.rotate_left(i as u32 + 71), 1, 4);
        for dy in 0..height {
            let block = match hash_coords(wx, wz, inst.seed + i as u64) % 6 {
                0 => BlockState::BLACKSTONE,
                1 => BlockState::DEEPSLATE_BRICKS,
                2 => BlockState::POLISHED_DEEPSLATE,
                3 => BlockState::CHAIN.set(PropName::Axis, PropValue::Y),
                4 => BlockState::COBBLED_DEEPSLATE,
                _ => BlockState::CRACKED_DEEPSLATE_BRICKS,
            };
            upsert(
                placements,
                chunk_bounds,
                wx,
                inst.base_y + 1 + dy,
                wz,
                block,
            );
        }
    }
}

fn rasterize_impact_crater(
    placements: &mut HashMap<(i32, i32, i32), Placement>,
    chunk_bounds: &ChunkBounds,
    inst: &SwordInstance,
) {
    let crater_count = range_i32(inst.seed.rotate_left(73), 3, 6) as usize;
    for i in 0..crater_count {
        let angle = sample_u01(inst.seed.rotate_left(i as u32 + 77)) * std::f64::consts::TAU;
        let dist = (inst.blade_width as f64)
            + 1.0
            + sample_u01(inst.seed.rotate_left(i as u32 + 81)) * 4.0;
        let dx = (angle.cos() * dist).round() as i32;
        let dz = (angle.sin() * dist).round() as i32;
        let wx = inst.origin_x + dx;
        let wz = inst.origin_z + dz;

        let block = match hash_coords(wx, wz, inst.seed.rotate_left(83)) % 4 {
            0 => BlockState::COBBLED_DEEPSLATE,
            1 => BlockState::GRAVEL,
            2 => BlockState::DEEPSLATE,
            _ => BlockState::CRACKED_DEEPSLATE_BRICKS,
        };
        upsert(placements, chunk_bounds, wx, inst.base_y, wz, block);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Block palette
// ─────────────────────────────────────────────────────────────────────────────

fn blade_block(seed: u64, y: i32, offset: i32, progress: f32) -> BlockState {
    let h = hash_coords(offset, y, seed.rotate_left(91));
    if progress > 0.85 {
        // Blade tip: cold gleam breaking through corrosion
        match h % 6 {
            0 => BlockState::POLISHED_DEEPSLATE,
            1 => BlockState::SMOOTH_BASALT,
            2 => BlockState::IRON_BLOCK,
            3 => BlockState::POLISHED_DEEPSLATE,
            4 => BlockState::DEEPSLATE,
            _ => BlockState::SMOOTH_BASALT,
        }
    } else if progress > 0.5 {
        // Upper blade: dark steel with occasional iron glint
        match h % 8 {
            0 => BlockState::POLISHED_DEEPSLATE,
            1 => BlockState::DEEPSLATE,
            2 => BlockState::IRON_BLOCK,
            3 => BlockState::POLISHED_DEEPSLATE,
            4 => BlockState::SMOOTH_BASALT,
            5 => BlockState::DEEPSLATE,
            6 => BlockState::DEEPSLATE_BRICKS,
            _ => BlockState::POLISHED_DEEPSLATE,
        }
    } else {
        // Lower blade: ancient, heavily corroded
        match h % 10 {
            0 => BlockState::DEEPSLATE,
            1 => BlockState::DEEPSLATE_BRICKS,
            2 => BlockState::CRACKED_DEEPSLATE_BRICKS,
            3 => BlockState::COBBLED_DEEPSLATE,
            4 => BlockState::POLISHED_DEEPSLATE,
            5 => BlockState::DEEPSLATE,
            6 => BlockState::BLACKSTONE,
            7 => BlockState::CRACKED_DEEPSLATE_BRICKS,
            8 => BlockState::DEEPSLATE,
            _ => BlockState::COBBLED_DEEPSLATE,
        }
    }
}

fn wooden_blade_block(seed: u64, y: i32, progress: f32) -> BlockState {
    let h = hash_coords(0, y, seed.rotate_left(93));
    if progress > 0.8 {
        // Tip: charred/broken
        match h % 3 {
            0 => BlockState::DARK_OAK_LOG,
            1 => BlockState::STRIPPED_DARK_OAK_LOG,
            _ => BlockState::DARK_OAK_PLANKS,
        }
    } else {
        match h % 5 {
            0 => BlockState::DARK_OAK_LOG,
            1 => BlockState::DARK_OAK_LOG,
            2 => BlockState::STRIPPED_DARK_OAK_LOG,
            3 => BlockState::DARK_OAK_PLANKS,
            _ => BlockState::DARK_OAK_LOG,
        }
    }
}

fn guard_block(seed: u64, dx: i32, dz: i32) -> BlockState {
    let h = hash_coords(dx, dz, seed.rotate_left(97));
    match h % 6 {
        0 => BlockState::WEATHERED_COPPER,
        1 => BlockState::BLACKSTONE,
        2 => BlockState::POLISHED_BLACKSTONE,
        3 => BlockState::OXIDIZED_COPPER,
        4 => BlockState::BLACKSTONE,
        _ => BlockState::POLISHED_BLACKSTONE_BRICKS,
    }
}

fn grip_block(seed: u64, dy: i32) -> BlockState {
    let h = hash_coords(dy, 0, seed.rotate_left(101));
    match h % 4 {
        0 => BlockState::DARK_OAK_LOG,
        1 => BlockState::SPRUCE_LOG,
        2 => BlockState::DARK_OAK_LOG,
        _ => BlockState::STRIPPED_DARK_OAK_LOG,
    }
}

fn pommel_block(seed: u64) -> BlockState {
    let h = seed.rotate_left(107) % 5;
    match h {
        0 => BlockState::OXIDIZED_COPPER,
        1 => BlockState::WEATHERED_COPPER,
        2 => BlockState::BLACKSTONE,
        3 => BlockState::RAW_COPPER_BLOCK,
        _ => BlockState::POLISHED_BLACKSTONE,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unique landmark: giant impact crater + broken grey sword (zone centerpiece)
// ─────────────────────────────────────────────────────────────────────────────

const CRATER_RADIUS: i32 = 45;
const CRATER_DEPTH: i32 = 16;
const CRATER_RIM_HEIGHT: i32 = 6;
const CRATER_SWORD_HEIGHT: i32 = 80;
const CRATER_SWORD_WIDTH: i32 = 10;

fn find_crater_center(terrain: &TerrainProvider, min_y: i32, world_height: i32) -> (i32, i32, i32) {
    // Sample a grid across the zone, pick the lowest surface point
    let mut best_x = (SWORD_SEA_MIN_X + SWORD_SEA_MAX_X) / 2;
    let mut best_z = (SWORD_SEA_MIN_Z + SWORD_SEA_MAX_Z) / 2;
    let mut best_y = i32::MAX;

    let step = 64;
    let margin = CRATER_RADIUS + 20;
    let mut x = SWORD_SEA_MIN_X + margin;
    while x <= SWORD_SEA_MAX_X - margin {
        let mut z = SWORD_SEA_MIN_Z + margin;
        while z <= SWORD_SEA_MAX_Z - margin {
            let sample = terrain.sample(x, z);
            let sy = column::surface_y_for_sample(&sample, min_y, world_height);
            if sy < best_y {
                best_y = sy;
                best_x = x;
                best_z = z;
            }
            z += step;
        }
        x += step;
    }
    (best_x, best_z, best_y)
}

fn place_crater_sword_if_overlaps(
    chunk: &mut UnloadedChunk,
    pos: ChunkPos,
    min_y: i32,
    terrain: &TerrainProvider,
) {
    let world_height = chunk.height() as i32;
    let (crater_x, crater_z, ground_y) = find_crater_center(terrain, min_y, world_height);

    let chunk_min_x = pos.x * 16;
    let chunk_min_z = pos.z * 16;
    let chunk_max_x = chunk_min_x + 15;
    let chunk_max_z = chunk_min_z + 15;

    let extent = CRATER_RADIUS + CRATER_RIM_HEIGHT + 5;
    if chunk_max_x < crater_x - extent
        || chunk_min_x > crater_x + extent
        || chunk_max_z < crater_z - extent
        || chunk_min_z > crater_z + extent
    {
        return;
    }

    for local_z in 0..16i32 {
        for local_x in 0..16i32 {
            let wx = chunk_min_x + local_x;
            let wz = chunk_min_z + local_z;
            let dx = wx - crater_x;
            let dz = wz - crater_z;
            let dist_sq = dx * dx + dz * dz;
            let dist = (dist_sq as f32).sqrt();
            let r = CRATER_RADIUS as f32;

            if dist > r + 5.0 {
                continue;
            }

            if dist <= r {
                // Inside crater: bowl shape carved into ground
                let ratio = 1.0 - (dist / r);
                let depth = (ratio * ratio * CRATER_DEPTH as f32).round() as i32;
                let floor_y = ground_y - depth;

                // Clear everything above the floor up to ground level + rim
                for y in floor_y..=(ground_y + CRATER_RIM_HEIGHT) {
                    let local_y = y - min_y;
                    if local_y < 0 || local_y >= world_height {
                        continue;
                    }
                    chunk.set_block_state(
                        local_x as u32,
                        local_y as u32,
                        local_z as u32,
                        BlockState::AIR,
                    );
                }

                // Place scorched floor
                let floor_local_y = floor_y - min_y;
                if floor_local_y >= 0 && floor_local_y < world_height {
                    let h = hash_coords(wx, wz, 0xF100_8ED0);
                    let block = match h % 6 {
                        0 => BlockState::BLACKSTONE,
                        1 => BlockState::BASALT,
                        2 => BlockState::POLISHED_BLACKSTONE,
                        3 => BlockState::SMOOTH_BASALT,
                        4 => BlockState::BLACKSTONE,
                        _ => BlockState::POLISHED_BASALT,
                    };
                    chunk.set_block_state(
                        local_x as u32,
                        floor_local_y as u32,
                        local_z as u32,
                        block,
                    );
                }

                // Fill solid below the floor so it's not hollow
                for y in (min_y + 1)..floor_y {
                    let local_y = y - min_y;
                    if local_y >= world_height {
                        break;
                    }
                    let existing =
                        chunk.block_state(local_x as u32, local_y as u32, local_z as u32);
                    if existing.is_air() {
                        chunk.set_block_state(
                            local_x as u32,
                            local_y as u32,
                            local_z as u32,
                            BlockState::DEEPSLATE,
                        );
                    }
                }
            }

            // Crater rim: raised ring around edge
            if dist > r * 0.85 && dist <= r + 4.0 {
                let rim_ratio = 1.0 - ((dist - r * 0.85) / (r * 0.19)).min(1.0);
                let rim_h = (rim_ratio * CRATER_RIM_HEIGHT as f32).round() as i32;
                for dy in 0..rim_h {
                    let y = ground_y + 1 + dy;
                    let local_y = y - min_y;
                    if local_y < 0 || local_y >= world_height {
                        continue;
                    }
                    let h = hash_coords(wx + dy, wz, 0xC8A7_E800);
                    let block = match h % 5 {
                        0 => BlockState::CRACKED_DEEPSLATE_BRICKS,
                        1 => BlockState::COBBLED_DEEPSLATE,
                        2 => BlockState::BLACKSTONE,
                        3 => BlockState::DEEPSLATE,
                        _ => BlockState::POLISHED_BLACKSTONE,
                    };
                    chunk.set_block_state(local_x as u32, local_y as u32, local_z as u32, block);
                }
            }
        }
    }

    // The broken grey sword: tilted, rising from crater pit floor
    let pit_floor_y = ground_y - CRATER_DEPTH;
    let sword_base_y = pit_floor_y - 5; // embedded into pit floor
    let tilt_dx = 1;

    for dy in 0..CRATER_SWORD_HEIGHT {
        let y = sword_base_y + dy;
        let local_y = y - min_y;
        if local_y < 0 || local_y >= world_height {
            continue;
        }

        let tilt_x = crater_x + (dy / 8) * tilt_dx;
        let progress = dy as f32 / CRATER_SWORD_HEIGHT as f32;

        // Broken top: jagged termination at 75%
        let is_broken_top = progress > 0.75;
        if is_broken_top {
            let jagged = hash_coords(tilt_x, y, 0xBBBB_AAAA) % 4;
            if jagged == 0 {
                continue;
            }
        }

        let half_w = if progress > 0.70 {
            let taper = 1.0 - (progress - 0.70) / 0.30;
            ((CRATER_SWORD_WIDTH as f32 * taper).round() as i32).max(1)
        } else {
            CRATER_SWORD_WIDTH
        };

        for bx in -half_w..=half_w {
            for bz in -2..=2i32 {
                // Flat blade: 3 blocks thick at center, 1 at edges
                if bz.abs() > 1 && bx.abs() >= half_w - 1 {
                    continue;
                }
                if bz.abs() > 2 {
                    continue;
                }
                // Outer thickness only for wide section
                if bz.abs() == 2 && (bx.abs() > half_w / 2 || half_w < 4) {
                    continue;
                }

                let wx = tilt_x + bx;
                let wz = crater_z + bz;

                if wx < chunk_min_x || wx > chunk_max_x || wz < chunk_min_z || wz > chunk_max_z {
                    continue;
                }
                let lx = (wx - chunk_min_x) as u32;
                let lz = (wz - chunk_min_z) as u32;

                let h = hash_coords(bx, y, 0xDE4D_5A0D);
                let block = if is_broken_top {
                    match h % 4 {
                        0 => BlockState::SMOOTH_BASALT,
                        1 => BlockState::DEAD_BRAIN_CORAL_BLOCK,
                        2 => BlockState::POLISHED_DEEPSLATE,
                        _ => BlockState::SMOOTH_BASALT,
                    }
                } else if progress > 0.4 {
                    match h % 6 {
                        0 => BlockState::SMOOTH_BASALT,
                        1 => BlockState::POLISHED_DEEPSLATE,
                        2 => BlockState::SMOOTH_BASALT,
                        3 => BlockState::DEAD_BRAIN_CORAL_BLOCK,
                        4 => BlockState::SMOOTH_BASALT,
                        _ => BlockState::BASALT,
                    }
                } else {
                    match h % 6 {
                        0 => BlockState::SMOOTH_BASALT,
                        1 => BlockState::BASALT,
                        2 => BlockState::POLISHED_DEEPSLATE,
                        3 => BlockState::SMOOTH_BASALT,
                        4 => BlockState::DEAD_BRAIN_CORAL_BLOCK,
                        _ => BlockState::BASALT,
                    }
                };

                chunk.set_block_state(lx, local_y as u32, lz, block);
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers (local duplicates of structures.rs utilities to avoid circular deps)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
struct Placement {
    block: BlockState,
}

fn upsert(
    placements: &mut HashMap<(i32, i32, i32), Placement>,
    chunk_bounds: &ChunkBounds,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    block: BlockState,
) {
    if !chunk_bounds.contains_with_margin(world_x, world_z, 18) {
        return;
    }
    placements.insert((world_x, world_y, world_z), Placement { block });
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_in_sword_sea_boundaries() {
        assert!(is_in_sword_sea(4000, 1000));
        assert!(is_in_sword_sea(3800, 800));
        assert!(is_in_sword_sea(5400, 2400));
        assert!(!is_in_sword_sea(3799, 1000));
        assert!(!is_in_sword_sea(5401, 1000));
        assert!(!is_in_sword_sea(4000, 799));
        assert!(!is_in_sword_sea(4000, 2401));
    }

    #[test]
    fn variant_distribution_colossal_pool() {
        let mut colossal_count = 0;
        let mut large_count = 0;
        let mut fallen_count = 0;
        for i in 0..1000 {
            let seed = hash_coords(i, i * 7, 0xDE5D_5EED);
            let (variant, _) = choose_variant_and_orientation(seed, 2);
            match variant {
                SwordVariant::Colossal => colossal_count += 1,
                SwordVariant::Large => large_count += 1,
                SwordVariant::Fallen => fallen_count += 1,
                _ => {}
            }
        }
        assert!(
            colossal_count > 300,
            "Expected ~500 colossal, got {colossal_count}"
        );
        assert!(large_count > 150, "Expected ~250 large, got {large_count}");
        assert!(
            fallen_count > 100,
            "Expected ~250 fallen, got {fallen_count}"
        );
    }

    #[test]
    fn blade_block_varies_by_progress() {
        let seed = 0x1234_5678;
        let low = blade_block(seed, 5, 0, 0.2);
        let high = blade_block(seed, 30, 0, 0.9);
        // Both should be valid block states (not air)
        assert_ne!(low, BlockState::AIR);
        assert_ne!(high, BlockState::AIR);
    }

    #[test]
    fn guard_block_never_air() {
        for dx in -5..=5 {
            for dz in -2..=2 {
                let b = guard_block(0xABCD, dx, dz);
                assert_ne!(b, BlockState::AIR);
            }
        }
    }

    #[test]
    fn grip_block_is_wood() {
        for dy in 0..10 {
            let b = grip_block(0xFEED, dy);
            assert!(
                b == BlockState::DARK_OAK_LOG
                    || b == BlockState::SPRUCE_LOG
                    || b == BlockState::STRIPPED_DARK_OAK_LOG,
                "Unexpected grip block at dy={dy}: {b:?}"
            );
        }
    }

    #[test]
    fn pommel_block_is_dark_metal() {
        for seed in 0..20u64 {
            let b = pommel_block(seed);
            assert!(
                b == BlockState::OXIDIZED_COPPER
                    || b == BlockState::WEATHERED_COPPER
                    || b == BlockState::BLACKSTONE
                    || b == BlockState::RAW_COPPER_BLOCK
                    || b == BlockState::POLISHED_BLACKSTONE,
                "Unexpected pommel block: {b:?}"
            );
        }
    }
}
