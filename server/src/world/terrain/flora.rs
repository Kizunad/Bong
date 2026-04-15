//! Per-column flora decoration placement.
//!
//! Reads `flora_density` + `flora_variant_id` from the raster (see
//! `raster::ColumnSample`) and realizes the referenced `Decoration` (see
//! `raster::TerrainProvider::decoration`) as an actual block structure in the
//! chunk. Each `DecorationSpec.kind` maps to a small procedural geometry:
//!
//!   tree      — trunk column of blocks[0] with blocks[1] canopy sphere at top
//!   shrub     — 1..3 block tall cluster, blocks[0] primary, blocks[1] accent
//!   boulder   — half-dome of blocks[0] with blocks[1] flecks
//!   crystal   — vertical pillar of blocks[0] tipped with blocks[1], blocks[2] stubs
//!   mushroom  — blocks[1] stem + blocks[0] cap disc, blocks[2] accent
//!   flower    — single blocks[0] plant
//!
//! Placements are chunk-local (no cross-chunk book-keeping): anything poking
//! out of the current chunk simply gets clipped. Mega-scale trees remain the
//! domain of `mega_tree.rs`; this module handles the dense, smaller
//! decorations that make each biome feel distinct.

use valence::prelude::{BlockState, Chunk, ChunkPos, UnloadedChunk};

use super::blocks::block_from_name;
use super::raster::{Decoration, TerrainProvider};

const CHUNK_SIZE: i32 = 16;
/// Minimum flora_density before we even roll placement. Mirrors the 0..1
/// clamp applied in the worldgen profiles.
const MIN_DENSITY: f32 = 0.05;
/// Threshold below which a variant is dropped (catches stray <=0 entries).
const DENSITY_PRECISION: u32 = 10_000;

pub fn decorate_chunk(
    chunk: &mut UnloadedChunk,
    pos: ChunkPos,
    min_y: i32,
    terrain: &TerrainProvider,
    top_y_by_column: &[[i32; 16]; 16],
) {
    for (local_z, row) in top_y_by_column.iter().enumerate() {
        for (local_x, &top_y) in row.iter().enumerate() {
            let world_x = pos.x * CHUNK_SIZE + local_x as i32;
            let world_z = pos.z * CHUNK_SIZE + local_z as i32;
            let sample = terrain.sample(world_x, world_z);

            if sample.flora_density < MIN_DENSITY || sample.flora_variant_id == 0 {
                continue;
            }
            let Some(deco) = terrain.decoration(sample.flora_variant_id) else {
                continue;
            };
            let base_y = top_y + 1;
            if base_y <= min_y {
                continue;
            }

            // Probability: density and rarity compound — dense regions fill up,
            // rare variants still feel sparse.
            let roll = decoration_hash(world_x, world_z, 997) % DENSITY_PRECISION;
            let target =
                (sample.flora_density * deco.rarity.max(0.05) * DENSITY_PRECISION as f32) as u32;
            if roll >= target {
                continue;
            }

            // Keep the top block underneath as support (don't overwrite with
            // air-only checks inside the geometry functions).
            place_decoration(
                chunk,
                local_x as i32,
                base_y,
                local_z as i32,
                min_y,
                deco,
                world_x,
                world_z,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn place_decoration(
    chunk: &mut UnloadedChunk,
    local_x: i32,
    base_y: i32,
    local_z: i32,
    min_y: i32,
    deco: &Decoration,
    world_x: i32,
    world_z: i32,
) {
    let blocks: Vec<BlockState> = deco
        .blocks
        .iter()
        .filter_map(|name| block_from_name(name))
        .collect();
    if blocks.is_empty() {
        return;
    }
    let size = sample_size(deco, world_x, world_z);

    match deco.kind.as_str() {
        "tree" => place_tree(
            chunk, local_x, base_y, local_z, min_y, &blocks, size, world_x, world_z,
        ),
        "shrub" => place_shrub(
            chunk, local_x, base_y, local_z, min_y, &blocks, size, world_x, world_z,
        ),
        "boulder" => place_boulder(
            chunk, local_x, base_y, local_z, min_y, &blocks, size, world_x, world_z,
        ),
        "crystal" => place_crystal(
            chunk, local_x, base_y, local_z, min_y, &blocks, size, world_x, world_z,
        ),
        "mushroom" => place_mushroom(
            chunk, local_x, base_y, local_z, min_y, &blocks, size, world_x, world_z,
        ),
        "flower" => place_flower(chunk, local_x, base_y, local_z, min_y, &blocks),
        // Unknown kind → primary block stump so something visible still appears.
        _ => {
            set_block_if_air(chunk, local_x, base_y, local_z, min_y, blocks[0]);
        }
    }
}

fn sample_size(deco: &Decoration, world_x: i32, world_z: i32) -> i32 {
    let [min, max] = deco.size_range;
    let min = min.max(1);
    if max <= min {
        return min;
    }
    let span = (max - min + 1) as u32;
    min + (decoration_hash(world_x, world_z, 13) % span) as i32
}

// ---------------------------------------------------------------------------
// Geometry primitives
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn place_tree(
    chunk: &mut UnloadedChunk,
    local_x: i32,
    base_y: i32,
    local_z: i32,
    min_y: i32,
    blocks: &[BlockState],
    size: i32,
    world_x: i32,
    world_z: i32,
) {
    let trunk = blocks[0];
    let canopy = blocks.get(1).copied().unwrap_or(trunk);
    let accent = blocks.get(2).copied();

    let trunk_h = size.max(3);
    for i in 0..trunk_h {
        set_block_if_air(chunk, local_x, base_y + i, local_z, min_y, trunk);
    }

    // Canopy: a forgiving sphere at the top of the trunk. Radius scales with
    // trunk height but stays modest to respect chunk boundaries.
    let canopy_top = base_y + trunk_h;
    let radius = (trunk_h / 4).clamp(2, 4);
    for dy in -1..=radius {
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                let d2 = dx * dx + dy * dy + dz * dz;
                let rr = radius * radius;
                if d2 > rr {
                    continue;
                }
                // Break a perfect sphere on the rim by culling some blocks
                // via hash — gives trees a naturally ragged silhouette.
                if d2 > (radius - 1) * (radius - 1) {
                    let h = decoration_hash(world_x + dx, world_z + dz, 37)
                        .wrapping_add((dy + radius) as u32);
                    if h.is_multiple_of(3) {
                        continue;
                    }
                }
                set_block_if_air(
                    chunk,
                    local_x + dx,
                    canopy_top + dy,
                    local_z + dz,
                    min_y,
                    canopy,
                );
            }
        }
    }

    // Accent: sparse highlights inside the canopy (lanterns, glow, etc.)
    if let Some(acc) = accent {
        for i in 0..3 {
            let ax = (decoration_hash(world_x, world_z, 51 + i) % (2 * radius as u32 + 1)) as i32
                - radius;
            let az = (decoration_hash(world_x, world_z, 53 + i) % (2 * radius as u32 + 1)) as i32
                - radius;
            let ay = (decoration_hash(world_x, world_z, 57 + i) % (radius as u32 + 1)) as i32 - 1;
            set_block_if_air(
                chunk,
                local_x + ax,
                canopy_top + ay,
                local_z + az,
                min_y,
                acc,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn place_shrub(
    chunk: &mut UnloadedChunk,
    local_x: i32,
    base_y: i32,
    local_z: i32,
    min_y: i32,
    blocks: &[BlockState],
    size: i32,
    world_x: i32,
    world_z: i32,
) {
    let primary = blocks[0];
    let accent = blocks.get(1).copied();
    let tertiary = blocks.get(2).copied();

    let h = size.clamp(1, 3);
    for i in 0..h {
        set_block_if_air(chunk, local_x, base_y + i, local_z, min_y, primary);
    }
    if let Some(a) = accent {
        for (i, (dx, dz)) in [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().enumerate() {
            let r = decoration_hash(world_x, world_z, 61 + i as u32) % 4;
            if r > 0 {
                set_block_if_air(chunk, local_x + dx, base_y, local_z + dz, min_y, a);
            }
        }
    }
    if let Some(t) = tertiary {
        // Crown the shrub with a tertiary accent half the time.
        if decoration_hash(world_x, world_z, 71).is_multiple_of(2) {
            set_block_if_air(chunk, local_x, base_y + h, local_z, min_y, t);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn place_boulder(
    chunk: &mut UnloadedChunk,
    local_x: i32,
    base_y: i32,
    local_z: i32,
    min_y: i32,
    blocks: &[BlockState],
    size: i32,
    world_x: i32,
    world_z: i32,
) {
    let primary = blocks[0];
    let secondary = blocks.get(1).copied();
    let tertiary = blocks.get(2).copied();

    // size encodes radius for boulders; clamp so we don't eat half a chunk.
    let r = size.clamp(2, 5);
    for dy in 0..r {
        for dx in -r..=r {
            for dz in -r..=r {
                let d2 = dx * dx + dy * dy + dz * dz;
                if d2 > r * r {
                    continue;
                }
                // Break upper rim so boulders aren't perfect hemispheres.
                if dy == r - 1 {
                    let h = decoration_hash(world_x + dx, world_z + dz, 83);
                    if h.is_multiple_of(4) {
                        continue;
                    }
                }
                let h = decoration_hash(world_x + dx, world_z + dz, 67);
                let block = if let Some(sec) = secondary {
                    if h.is_multiple_of(7) {
                        sec
                    } else if let Some(ter) = tertiary {
                        if h.is_multiple_of(19) {
                            ter
                        } else {
                            primary
                        }
                    } else {
                        primary
                    }
                } else {
                    primary
                };
                set_block_if_air(chunk, local_x + dx, base_y + dy, local_z + dz, min_y, block);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn place_crystal(
    chunk: &mut UnloadedChunk,
    local_x: i32,
    base_y: i32,
    local_z: i32,
    min_y: i32,
    blocks: &[BlockState],
    size: i32,
    world_x: i32,
    world_z: i32,
) {
    let body = blocks[0];
    let tip = blocks.get(1).copied().unwrap_or(body);
    let accent = blocks.get(2).copied();

    let h = size.max(3);
    for i in 0..h {
        set_block_if_air(chunk, local_x, base_y + i, local_z, min_y, body);
    }
    // Tip: one (or two) blocks of tip material on the very top.
    set_block_if_air(chunk, local_x, base_y + h, local_z, min_y, tip);
    if h > 4 {
        set_block_if_air(chunk, local_x, base_y + h + 1, local_z, min_y, tip);
    }

    // Accent: short stumps at the crystal's base give it a nesting feel.
    if let Some(acc) = accent {
        for (i, (dx, dz)) in [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().enumerate() {
            let roll = decoration_hash(world_x, world_z, 91 + i as u32) % 8;
            if roll < 3 {
                let stub_h = 1 + (roll as i32 % 2);
                for sy in 0..stub_h {
                    set_block_if_air(chunk, local_x + dx, base_y + sy, local_z + dz, min_y, acc);
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn place_mushroom(
    chunk: &mut UnloadedChunk,
    local_x: i32,
    base_y: i32,
    local_z: i32,
    min_y: i32,
    blocks: &[BlockState],
    size: i32,
    world_x: i32,
    world_z: i32,
) {
    let cap = blocks[0];
    let stem = blocks.get(1).copied().unwrap_or(cap);
    let accent = blocks.get(2).copied();

    let stem_h = size.clamp(2, 4);
    for i in 0..stem_h {
        set_block_if_air(chunk, local_x, base_y + i, local_z, min_y, stem);
    }

    let cap_y = base_y + stem_h;
    // Disc-shaped cap, radius 2. Slightly jagged by hash culling.
    let radius: i32 = 2;
    for dx in -radius..=radius {
        for dz in -radius..=radius {
            let d2 = dx * dx + dz * dz;
            if d2 > radius * radius {
                continue;
            }
            if d2 == radius * radius {
                let h = decoration_hash(world_x + dx, world_z + dz, 103);
                if h.is_multiple_of(3) {
                    continue;
                }
            }
            set_block_if_air(chunk, local_x + dx, cap_y, local_z + dz, min_y, cap);
        }
    }

    if let Some(acc) = accent {
        // Sparkle an accent block in the cap center.
        set_block_if_air(chunk, local_x, cap_y + 1, local_z, min_y, acc);
    }
}

fn place_flower(
    chunk: &mut UnloadedChunk,
    local_x: i32,
    base_y: i32,
    local_z: i32,
    min_y: i32,
    blocks: &[BlockState],
) {
    set_block_if_air(chunk, local_x, base_y, local_z, min_y, blocks[0]);
}

// ---------------------------------------------------------------------------
// Local helpers (self-contained — decoration.rs's equivalents are module-private)
// ---------------------------------------------------------------------------

fn set_block_if_air(
    chunk: &mut UnloadedChunk,
    local_x: i32,
    world_y: i32,
    local_z: i32,
    min_y: i32,
    block: BlockState,
) {
    if !(0..CHUNK_SIZE).contains(&local_x) || !(0..CHUNK_SIZE).contains(&local_z) {
        return;
    }
    let local_y = world_y - min_y;
    if local_y < 0 || local_y >= chunk.height() as i32 {
        return;
    }
    let state = chunk.block_state(local_x as u32, local_y as u32, local_z as u32);
    if !state.is_air() {
        return;
    }
    chunk.set_block_state(local_x as u32, local_y as u32, local_z as u32, block);
}

/// Same mix function as `decoration.rs::decoration_hash` but kept local so
/// flora placement stays independent of that module's private helpers.
fn decoration_hash(world_x: i32, world_z: i32, salt: u32) -> u32 {
    let mut h = salt.wrapping_mul(0x9E37_79B1);
    h = h.wrapping_add((world_x as u32).wrapping_mul(0x85EB_CA6B));
    h = h.wrapping_add((world_z as u32).wrapping_mul(0xC2B2_AE35));
    h ^= h >> 13;
    h = h.wrapping_mul(0xC2B2_AE3D);
    h ^= h >> 16;
    h
}
