//! Per-column flora decoration placement.
//!
//! Two parallel layers, both reading from the same global decoration palette
//! but driven by independent raster channels:
//!
//! - **flora_variant_id** (`flora_density`) — sparse feature decorations
//!   (tree / shrub / boulder / crystal / mushroom). One per column max.
//! - **ground_cover_id** (`ground_cover_density`) — dense ground cover
//!   (kind="flower" specs: short grass / dandelion / fern / dead_bush). Also
//!   one per column max, but independent from flora — a column can host both
//!   a tree AND meadow grass.
//!
//! Each `DecorationSpec.kind` maps to a small procedural geometry:
//!
//!   tree      — trunk column of blocks[0] with blocks[1] canopy sphere at top
//!   shrub     — 1..3 block tall cluster, blocks[0] primary, blocks[1] accent
//!   boulder   — half-dome of blocks[0] with blocks[1] flecks
//!   crystal   — vertical pillar of blocks[0] tipped with blocks[1], blocks[2] stubs
//!   mushroom  — blocks[1] stem + blocks[0] cap disc, blocks[2] accent
//!   flower    — single blocks[0] plant (typical ground-cover form)
//!
//! Both layers share an 8×8 + 16×16 cluster gate so flora and ground cover
//! cluster naturally instead of dusting uniformly across the world. Feature
//! decorations gate harder (≥70 ⇒ skip, 30% bald patches), ground cover
//! gates lighter (≥85, 15% bald patches) so meadows feel continuous while
//! tree groves still feel grouped.
//!
//! Placements are chunk-local (no cross-chunk book-keeping): anything poking
//! out of the current chunk simply gets clipped. Mega-scale trees remain the
//! domain of `mega_tree.rs`.

use valence::prelude::{BlockState, Chunk, ChunkPos, PropName, PropValue, UnloadedChunk};

use super::blocks::block_from_name;
use super::column;
use super::raster::{ColumnSample, Decoration, TerrainProvider};

const CHUNK_SIZE: i32 = 16;
/// Minimum flora_density before we even roll placement. Mirrors the 0..1
/// clamp applied in the worldgen profiles.
const MIN_DENSITY: f32 = 0.05;
/// Threshold below which a variant is dropped (catches stray <=0 entries).
const DENSITY_PRECISION: u32 = 10_000;
/// Small trees and petrified stumps read well as landmarks, but dense profile
/// masks make the generic tree primitive crowd the surface too quickly.
const TREE_DENSITY_SCALE: f32 = 0.4;

/// Cluster gate threshold for feature decorations (flora_variant_id). Cells
/// scoring ≥ this value skip the feature loop entirely → ~30% of 8×8 patches
/// are bald, so groves cluster instead of dusting uniformly.
const FEATURE_CLUSTER_MAX: u32 = 70;
/// Cluster gate threshold for ground cover. Looser than feature gate so
/// meadows feel continuous (~15% bald patches).
const GROUND_COVER_CLUSTER_MAX: u32 = 85;

pub fn decorate_chunk(
    chunk: &mut UnloadedChunk,
    pos: ChunkPos,
    min_y: i32,
    terrain: &TerrainProvider,
    top_y_by_column: &[[i32; 16]; 16],
) {
    let world_height = chunk.height() as i32;
    // Track which columns took a feature decoration so the ground-cover loop
    // can skip them — otherwise a boulder's lower rim sits on top of the
    // ground-cover flower we just placed (visible "sand on dead bush").
    let mut feature_occupied = [[false; CHUNK_SIZE as usize]; CHUNK_SIZE as usize];

    for (local_z, row) in top_y_by_column.iter().enumerate() {
        for (local_x, &top_y) in row.iter().enumerate() {
            let world_x = pos.x * CHUNK_SIZE + local_x as i32;
            let world_z = pos.z * CHUNK_SIZE + local_z as i32;
            let sample = terrain.sample(world_x, world_z);

            // Cluster score combines 8×8 and 16×16 cell hashes. Averaging
            // softens the hard 8×8 cell edges while keeping the macro
            // bald-patch distribution from the 16×16 layer.
            let cluster_a = decoration_hash(world_x.div_euclid(8), world_z.div_euclid(8), 31) % 100;
            let cluster_b =
                decoration_hash(world_x.div_euclid(16), world_z.div_euclid(16), 33) % 100;
            let cluster_score = (cluster_a + cluster_b) / 2;

            // --- Layer 1: feature decoration (trees / shrubs / boulders) ---
            if cluster_score < FEATURE_CLUSTER_MAX
                && sample.flora_density >= MIN_DENSITY
                && sample.flora_variant_id != 0
            {
                if let Some(deco) = terrain.decoration(sample.flora_variant_id) {
                    if let Some(base_y) =
                        placement_base_y(deco, &sample, top_y, min_y, world_height)
                    {
                        // Sky-isle bottom hangs from above; everything else needs
                        // a block under base_y (carve / mega_tree / water can
                        // leave top_y empty otherwise → 浮空树/石/灌).
                        // Plant-like kinds (tree/shrub) want soil whitelist；
                        // 岩石/结构/菌类（boulder/crystal/mushroom/fallen_log/
                        // grave_mound）能落在 stone/deepslate/andesite 等任意
                        // 实心方块上，否则 broken_peaks / waste_plateau 整片
                        // feature 装饰会消失。
                        let needs_below_support = !is_sky_isle_bottom_flora(deco);
                        if needs_below_support {
                            let supported = if requires_plant_soil(deco) {
                                has_plant_support_below(
                                    chunk,
                                    local_x as i32,
                                    base_y,
                                    local_z as i32,
                                    min_y,
                                )
                            } else {
                                has_solid_support_below(
                                    chunk,
                                    local_x as i32,
                                    base_y,
                                    local_z as i32,
                                    min_y,
                                )
                            };
                            if !supported {
                                continue;
                            }
                        }
                        let roll = decoration_hash(world_x, world_z, 997) % DENSITY_PRECISION;
                        let target = (sample.flora_density
                            * deco.rarity.max(0.05)
                            * placement_density_scale(deco)
                            * DENSITY_PRECISION as f32) as u32;
                        if roll < target {
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
                            feature_occupied[local_z][local_x] = true;
                        }
                    }
                }
            }
        }
    }

    // --- Layer 2: ground cover (草/花/枯木) ---
    // 单独一遍循环，跳过被特征装饰占用的列；同时检查 base_y-1 是不是真正能
    // 承载植被的方块（防止 carve / mega_tree / 水位异常导致草浮空）。
    // Independent salt (1009 vs 997) so feature roll and ground-cover roll
    // don't lock-step — same column can win one and lose the other.
    for (local_z, row) in top_y_by_column.iter().enumerate() {
        for (local_x, &top_y) in row.iter().enumerate() {
            if feature_occupied[local_z][local_x] {
                continue;
            }
            let world_x = pos.x * CHUNK_SIZE + local_x as i32;
            let world_z = pos.z * CHUNK_SIZE + local_z as i32;
            let sample = terrain.sample(world_x, world_z);

            let cluster_a = decoration_hash(world_x.div_euclid(8), world_z.div_euclid(8), 31) % 100;
            let cluster_b =
                decoration_hash(world_x.div_euclid(16), world_z.div_euclid(16), 33) % 100;
            let cluster_score = (cluster_a + cluster_b) / 2;

            if cluster_score >= GROUND_COVER_CLUSTER_MAX
                || sample.ground_cover_density < MIN_DENSITY
                || sample.ground_cover_id == 0
            {
                continue;
            }

            let base_y = top_y + 1;
            // 下方支撑白名单：vanilla 草本类植物只在土质 / 沙质 / 苔藓类方块上稳定
            if !has_plant_support_below(chunk, local_x as i32, base_y, local_z as i32, min_y) {
                continue;
            }

            let Some(deco) = terrain.decoration(sample.ground_cover_id) else {
                continue;
            };
            let roll = decoration_hash(world_x, world_z, 1009) % DENSITY_PRECISION;
            let target = (sample.ground_cover_density
                * deco.rarity.max(0.05)
                * DENSITY_PRECISION as f32) as u32;
            if roll >= target {
                continue;
            }
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

/// Whether the block immediately under `base_y` is a vanilla "可放草本"
/// support: dirt 家族 / 砂 / 砂岩 / 苔藓 / clay / mud / gravel。
/// 排除 leaves / log / water / air / 矿物 等不该长草的。
/// 草本类（kind="tree" / "shrub"）需要 vanilla 草本支撑（土质 / 沙质 /
/// 苔藓）。其余 kind（boulder/crystal/mushroom/fallen_log/grave_mound/
/// flower）走 solid 通用支撑。flower 在 ground-cover loop 单独严格检查，
/// 这里不会走到。
fn requires_plant_soil(deco: &Decoration) -> bool {
    matches!(deco.kind.as_str(), "tree" | "shrub")
}

/// Generic solid-support check: 任何非空气、非液体的方块都算支撑，给
/// boulder / crystal / mushroom / fallen_log / grave_mound 用 —— 它们
/// 在石质 / 深板岩 / 安山岩等地形上也要能放，不能走 plant 白名单。
fn has_solid_support_below(
    chunk: &UnloadedChunk,
    local_x: i32,
    base_y: i32,
    local_z: i32,
    min_y: i32,
) -> bool {
    if !(0..CHUNK_SIZE).contains(&local_x) || !(0..CHUNK_SIZE).contains(&local_z) {
        return false;
    }
    let support_y = base_y - 1;
    let local_y = support_y - min_y;
    if local_y < 0 || local_y >= chunk.height() as i32 {
        return false;
    }
    let state = chunk.block_state(local_x as u32, local_y as u32, local_z as u32);
    !state.is_air() && !state.is_liquid()
}

fn has_plant_support_below(
    chunk: &UnloadedChunk,
    local_x: i32,
    base_y: i32,
    local_z: i32,
    min_y: i32,
) -> bool {
    if !(0..CHUNK_SIZE).contains(&local_x) || !(0..CHUNK_SIZE).contains(&local_z) {
        return false;
    }
    let support_y = base_y - 1;
    let local_y = support_y - min_y;
    if local_y < 0 || local_y >= chunk.height() as i32 {
        return false;
    }
    let state = chunk.block_state(local_x as u32, local_y as u32, local_z as u32);
    matches!(
        state,
        BlockState::GRASS_BLOCK
            | BlockState::PODZOL
            | BlockState::MYCELIUM
            | BlockState::DIRT
            | BlockState::COARSE_DIRT
            | BlockState::ROOTED_DIRT
            | BlockState::DIRT_PATH
            | BlockState::FARMLAND
            | BlockState::MOSS_BLOCK
            | BlockState::MUD
            | BlockState::MUDDY_MANGROVE_ROOTS
            | BlockState::CLAY
            | BlockState::GRAVEL
            | BlockState::SAND
            | BlockState::RED_SAND
            | BlockState::SANDSTONE
            | BlockState::RED_SANDSTONE
            | BlockState::TERRACOTTA
    )
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

    if is_sky_isle_bottom_flora(deco) {
        place_hanging_crystal(
            chunk, local_x, base_y, local_z, min_y, &blocks, size, world_x, world_z,
        );
        return;
    }

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
        "fallen_log" => place_fallen_log(
            chunk, local_x, base_y, local_z, min_y, &blocks, size, world_x, world_z,
        ),
        "grave_mound" => place_grave_mound(
            chunk, local_x, base_y, local_z, min_y, &blocks, size, world_x, world_z,
        ),
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

fn placement_base_y(
    deco: &Decoration,
    sample: &ColumnSample,
    ground_top_y: i32,
    min_y: i32,
    world_height: i32,
) -> Option<i32> {
    if is_sky_isle_top_flora(deco) {
        return column::sky_island_span_for_sample(sample, min_y, world_height)
            .map(|span| span.top_y + 1);
    }

    if is_sky_isle_bottom_flora(deco) {
        return column::sky_island_span_for_sample(sample, min_y, world_height)
            .map(|span| span.bottom_y - 1);
    }

    Some(ground_top_y + 1)
}

fn placement_density_scale(deco: &Decoration) -> f32 {
    if deco.kind == "tree" {
        TREE_DENSITY_SCALE
    } else {
        1.0
    }
}

fn is_sky_isle_top_flora(deco: &Decoration) -> bool {
    deco.profile == "sky_isle" && matches!(deco.name.as_str(), "ling_yu_tree" | "fei_yu_bamboo")
}

fn is_sky_isle_bottom_flora(deco: &Decoration) -> bool {
    deco.profile == "sky_isle" && deco.name == "tian_mai_crystal"
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

    // Oak-only vine drape: 35% per candidate cell on the canopy rim, then
    // hang 1–3 blocks down with diminishing odds (60% / 42% / 24%).
    if trunk == BlockState::OAK_LOG {
        drape_oak_vines(
            chunk, local_x, base_y, local_z, min_y, trunk_h, radius, world_x, world_z,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn drape_oak_vines(
    chunk: &mut UnloadedChunk,
    trunk_lx: i32,
    trunk_base_y: i32,
    trunk_lz: i32,
    min_y: i32,
    trunk_h: i32,
    radius: i32,
    world_x: i32,
    world_z: i32,
) {
    let canopy_top = trunk_base_y + trunk_h;
    let scan_y_start = canopy_top - 1;
    let scan_y_end = canopy_top + radius;
    let scan_r = radius + 1;

    for y in scan_y_start..=scan_y_end {
        for dx in -scan_r..=scan_r {
            for dz in -scan_r..=scan_r {
                let cx = trunk_lx + dx;
                let cz = trunk_lz + dz;
                if !(0..CHUNK_SIZE).contains(&cx) || !(0..CHUNK_SIZE).contains(&cz) {
                    continue;
                }
                let local_y = y - min_y;
                if local_y < 0 || local_y >= chunk.height() as i32 {
                    continue;
                }
                if !chunk
                    .block_state(cx as u32, local_y as u32, cz as u32)
                    .is_air()
                {
                    continue;
                }

                // 4 邻居方向：vine 把"该方向上有 oak 块"的面 set 为 True
                let n = is_oak_at(chunk, cx, y, cz - 1, min_y);
                let e = is_oak_at(chunk, cx + 1, y, cz, min_y);
                let s = is_oak_at(chunk, cx, y, cz + 1, min_y);
                let w = is_oak_at(chunk, cx - 1, y, cz, min_y);
                if !(n || e || s || w) {
                    continue;
                }

                let h = decoration_hash(world_x + dx, world_z + dz, 281)
                    .wrapping_add((y - min_y) as u32);
                if h % 100 >= 35 {
                    continue;
                }

                let mut vine = BlockState::VINE;
                if n {
                    vine = vine.set(PropName::North, PropValue::True);
                }
                if e {
                    vine = vine.set(PropName::East, PropValue::True);
                }
                if s {
                    vine = vine.set(PropName::South, PropValue::True);
                }
                if w {
                    vine = vine.set(PropName::West, PropValue::True);
                }
                set_block_if_air(chunk, cx, y, cz, min_y, vine);

                // 下垂藤：每格概率 60% / 42% / 24% 衰减
                let drape_state = vine;
                for ddy in 1..=3i32 {
                    let dy_world = y - ddy;
                    let dlocal = dy_world - min_y;
                    if dlocal < 0 || dlocal >= chunk.height() as i32 {
                        break;
                    }
                    if !chunk
                        .block_state(cx as u32, dlocal as u32, cz as u32)
                        .is_air()
                    {
                        break;
                    }
                    let dh = decoration_hash(world_x + dx, world_z + dz, 283 + ddy as u32);
                    let chance = match ddy {
                        1 => 60,
                        2 => 42,
                        _ => 24,
                    };
                    if dh % 100 >= chance {
                        break;
                    }
                    set_block_if_air(chunk, cx, dy_world, cz, min_y, drape_state);
                }
            }
        }
    }
}

fn is_oak_at(chunk: &UnloadedChunk, local_x: i32, world_y: i32, local_z: i32, min_y: i32) -> bool {
    if !(0..CHUNK_SIZE).contains(&local_x) || !(0..CHUNK_SIZE).contains(&local_z) {
        return false;
    }
    let local_y = world_y - min_y;
    if local_y < 0 || local_y >= chunk.height() as i32 {
        return false;
    }
    matches!(
        chunk.block_state(local_x as u32, local_y as u32, local_z as u32),
        BlockState::OAK_LOG | BlockState::OAK_LEAVES
    )
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
fn place_hanging_crystal(
    chunk: &mut UnloadedChunk,
    local_x: i32,
    anchor_y: i32,
    local_z: i32,
    min_y: i32,
    blocks: &[BlockState],
    size: i32,
    world_x: i32,
    world_z: i32,
) {
    let body = blocks.get(1).copied().unwrap_or(blocks[0]);
    let tip = blocks[0];
    let accent = blocks.get(2).copied();

    let h = size.max(3);
    for i in 0..h {
        set_block_if_air(chunk, local_x, anchor_y - i, local_z, min_y, body);
    }
    set_block_if_air(chunk, local_x, anchor_y - h, local_z, min_y, tip);

    if let Some(acc) = accent {
        for (i, (dx, dz)) in [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().enumerate() {
            let roll = decoration_hash(world_x, world_z, 111 + i as u32) % 8;
            if roll < 2 {
                set_block_if_air(chunk, local_x + dx, anchor_y, local_z + dz, min_y, acc);
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

/// Fallen log: 横躺的 log（带 axis 属性），随机 N/S/E/W 方向延伸 size 段。
#[allow(clippy::too_many_arguments)]
fn place_fallen_log(
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
    let direction = decoration_hash(world_x, world_z, 591) % 4;
    let (dx, dz, axis) = match direction {
        0 => (1_i32, 0_i32, PropValue::X),
        1 => (-1, 0, PropValue::X),
        2 => (0, 1, PropValue::Z),
        _ => (0, -1, PropValue::Z),
    };
    let log = blocks[0].set(PropName::Axis, axis);
    let length = size.clamp(3, 6);
    for i in 0..length {
        let cx = local_x + dx * i;
        let cz = local_z + dz * i;
        set_block_if_air(chunk, cx, base_y, cz, min_y, log);
    }
}

/// Grave mound: 半圆苔石 dome + 中央顶上立 sign 当碑。
/// blocks[0]=主体, [1]=表层苔石, [2]=sign（先放空牌，碑文 NBT 待后续阶段实现）。
/// 整体下沉 1 格（base_y - 1 起算，比地表低一格半埋），强制替换地表方块
/// 以制造"半埋古坟"质感，不是"地上叠石"。跨 chunk 时只有半个 dome ——
/// 因为 chunk-local 写入限制；要根治需要 anchor 系统跨 chunk 同步。
#[allow(clippy::too_many_arguments)]
fn place_grave_mound(
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
    let crust = blocks.get(1).copied().unwrap_or(body);
    let sign_block = blocks.get(2).copied();

    // 下沉 1 格：dome 起点比地表低一格，半埋
    let dome_base = base_y - 1;
    let radius = size.clamp(2, 5);
    let mound_h = radius - 1; // 半径=2 → 1 高，半径=5 → 4 高
    for dy in 0..=mound_h {
        let layer_r = radius - dy;
        let layer_r_sq = layer_r * layer_r;
        for dx in -layer_r..=layer_r {
            for dz in -layer_r..=layer_r {
                let d2 = dx * dx + dz * dz;
                if d2 > layer_r_sq {
                    continue;
                }
                // 顶层 + 外缘用 mossy_cobblestone（crust），内部用 cobblestone（body）
                let block = if dy == mound_h || d2 == layer_r_sq {
                    crust
                } else {
                    body
                };
                // 强制替换（不用 if_air）—— 制造半埋下沉的视觉，让 dome 切掉
                // 下方 dirt/grass_block 等。
                set_block_at_world(
                    chunk,
                    local_x + dx,
                    dome_base + dy,
                    local_z + dz,
                    min_y,
                    block,
                );
            }
        }
    }

    // 中央顶上立碑（sign 立在土堆顶面方块之上一格）
    if let Some(sign) = sign_block {
        let sign_y = dome_base + mound_h + 1;
        let rot = match decoration_hash(world_x, world_z, 597) % 16 {
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
        let sign_state = sign.set(PropName::Rotation, rot);
        set_block_if_air(chunk, local_x, sign_y, local_z, min_y, sign_state);
    }
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

/// 无条件覆盖（不检查 air）—— 用于 grave_mound 这种要"切下去 / 半埋"的几何，
/// 让 dome 强制替换地表 grass_block / dirt 制造下沉视觉。
fn set_block_at_world(
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
