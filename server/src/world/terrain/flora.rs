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
//! Placement (where / whether / which variant) is decided by the cluster +
//! density gate below — **unchanged** by worldgen-v4 P6. P6 only swaps the
//! *geometry source*: once a decoration carries authored NBT variants
//! (`Decoration::is_nbt_driven`), the chosen-by-seed variant is **stamped** from
//! the resident [`DecorationNbtRegistry`] (memcpy-level, no runtime gzip — §8.1
//! #10) instead of built procedurally. A decoration with no templates keeps its
//! procedural geometry (the §8.1 #9 fallback), so the two paths share the exact
//! same gate and produce the same placement positions for a given seed.
//!
//! Each NBT-less `DecorationSpec.kind` keeps a small procedural geometry:
//!
//!   tree      — trunk column of blocks[0] with blocks[1] canopy sphere at top
//!   shrub     — 1..3 block tall cluster, blocks[0] primary, blocks[1] accent
//!   boulder   — half-dome of blocks[0] with blocks[1] flecks
//!   crystal   — vertical pillar of blocks[0] tipped with blocks[1], blocks[2] stubs
//!   mushroom  — blocks[1] stem + blocks[0] cap disc, blocks[2] accent
//!   flower    — single blocks[0] plant (typical ground-cover form; never NBT)
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

use valence::prelude::{BlockPos, BlockState, Chunk, ChunkPos, PropName, PropValue, UnloadedChunk};

use super::blocks::block_from_name;
use super::column;
use super::nbt_registry::{DecorationAnchor, DecorationNbtRegistry, Rotation};
use super::raster::{ColumnSample, Decoration, TerrainProvider};

/// Hash salt for picking which authored NBT variant a placement stamps. Distinct
/// from the placement-roll salt (997) so variant choice does not lock-step with
/// the place/skip decision — same column can win placement yet vary its variant.
const NBT_VARIANT_SALT: u32 = 619;
/// Hash salt for the quarter-turn rotation applied to a stamped NBT variant, so
/// a single authored template appears in four orientations across the world.
const NBT_ROTATION_SALT: u32 = 623;

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
    registry: &DecorationNbtRegistry,
) {
    // Sword sea zone: no vegetation — only bare stone and swords
    if super::giant_sword::is_in_sword_sea(pos.x * 16, pos.z * 16)
        && super::giant_sword::is_in_sword_sea(pos.x * 16 + 15, pos.z * 16 + 15)
    {
        return;
    }

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
                            // worldgen-v4 P6 §8.1 — footprint-aware anti-overlap.
                            // Authored NBT variants have multi-cell footprints (the
                            // rift_bridge spans up to 13–15 cells); two scatter points
                            // landing close used to interpenetrate because each feature
                            // marked only its anchor cell. Compute the cells this
                            // decoration will actually occupy and skip the whole column
                            // if any of them is already claimed by an earlier feature.
                            // Greedy / row-major so the result stays deterministic
                            // (same seed → same chunk).
                            //
                            // KNOWN RESIDUAL (P7): cells of a large footprint that fall
                            // in a NEIGHBOURING chunk are clipped here and not tracked,
                            // so cross-chunk feature overlap can still occur. This pass
                            // only resolves within-chunk feature-vs-feature overlap.
                            let footprint = decoration_footprint(
                                local_x as i32,
                                local_z as i32,
                                base_y,
                                deco,
                                world_x,
                                world_z,
                                registry,
                            );
                            // Greedy first-come reservation: place only if every
                            // footprint cell is free, then claim them all. Skipped
                            // features are NOT re-tried via procedural — the column
                            // simply yields to the earlier feature.
                            if !reserve_footprint(&mut feature_occupied, &footprint) {
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
                                registry,
                            );
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
                registry,
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

/// The set of in-chunk ground cells `(local_x, local_z)` this decoration will
/// occupy if placed at `(local_x, local_z, base_y)`. Used by the feature loop to
/// detect / reserve overlap so two authored NBT footprints never interpenetrate.
///
/// The prediction **mirrors the place path** so the check and the write agree:
/// * If [`place_decoration`] would take the NBT stamp path AND a variant
///   resolves, project the stamp's resolved cells (`registry.stamp` is pure, so
///   the writer re-resolving them yields the identical set).
/// * Otherwise (template-less / flower / unresolved → procedural geometry) fall
///   back to a conservative horizontal disc sized from the kind + size_range, so
///   the procedural path also participates in avoidance.
///
/// Cells outside the chunk (`local_x`/`local_z` not in `0..16`) are dropped:
/// those are clipped by the writer and are the P7 cross-chunk residual, not part
/// of this within-chunk pass. The anchor cell `(local_x, local_z)` is always
/// included so even a fully out-of-chunk-clipped stamp still reserves its column.
fn decoration_footprint(
    local_x: i32,
    local_z: i32,
    base_y: i32,
    deco: &Decoration,
    world_x: i32,
    world_z: i32,
    registry: &DecorationNbtRegistry,
) -> Vec<(i32, i32)> {
    let mut cells: Vec<(i32, i32)> = Vec::new();
    let push = |fx: i32, fz: i32, cells: &mut Vec<(i32, i32)>| {
        if (0..CHUNK_SIZE).contains(&fx)
            && (0..CHUNK_SIZE).contains(&fz)
            && !cells.contains(&(fx, fz))
        {
            cells.push((fx, fz));
        }
    };
    // The anchor column is always reserved even if every stamped cell is clipped
    // out of this chunk — otherwise the ground-cover loop would grow grass under
    // the (clipped) feature's column.
    push(local_x, local_z, &mut cells);

    let takes_nbt_path = deco.is_nbt_driven() && deco.kind != "flower";
    if takes_nbt_path {
        if let Some(stamp) =
            nbt_stamp_placements(local_x, base_y, local_z, deco, world_x, world_z, registry)
        {
            for (lx, lz, _world_y, _state) in stamp.placements {
                push(lx, lz, &mut cells);
            }
            return cells;
        }
        // NBT path was eligible but no variant resolved → falls through to
        // procedural in place_decoration; use the procedural disc below.
    }

    // Procedural fallback footprint: a conservative horizontal disc whose radius
    // bounds the kind's geometry spread. These match the radii the procedural
    // geometry primitives below can reach (canopy radius, boulder r, mushroom
    // cap, shrub/crystal accents, fallen-log length). Flowers are single-cell.
    let radius = procedural_footprint_radius(deco, world_x, world_z);
    for dz in -radius..=radius {
        for dx in -radius..=radius {
            if dx * dx + dz * dz > radius * radius {
                continue;
            }
            push(local_x + dx, local_z + dz, &mut cells);
        }
    }
    // Fallen logs extend linearly well past the disc; reserve the run too so a
    // log never lies across a neighbouring feature's footprint.
    if deco.kind == "fallen_log" {
        let direction = decoration_hash(world_x, world_z, 591) % 4;
        let (dx, dz) = match direction {
            0 => (1_i32, 0_i32),
            1 => (-1, 0),
            2 => (0, 1),
            _ => (0, -1),
        };
        let length = sample_size(deco, world_x, world_z).clamp(3, 6);
        for i in 0..length {
            push(local_x + dx * i, local_z + dz * i, &mut cells);
        }
    }
    cells
}

/// Conservative horizontal radius bounding the procedural geometry for `deco`'s
/// kind. Mirrors the spreads the geometry primitives below can reach so the
/// footprint never under-reserves (greedy avoidance prefers over-reserving).
fn procedural_footprint_radius(deco: &Decoration, world_x: i32, world_z: i32) -> i32 {
    let size = sample_size(deco, world_x, world_z);
    match deco.kind.as_str() {
        // place_tree: canopy radius = (trunk_h/4).clamp(2,4); vines reach +1.
        "tree" => (size.max(3) / 4).clamp(2, 4) + 1,
        // place_boulder: r = size.clamp(2,5).
        "boulder" => size.clamp(2, 5),
        // place_mushroom: cap disc radius 2.
        "mushroom" => 2,
        // place_shrub: accents at the 4 orthogonal neighbours (radius 1).
        "shrub" => 1,
        // place_crystal / hanging: base accents at the 4 neighbours (radius 1).
        "crystal" => 1,
        // place_grave_mound: dome radius = size.clamp(2,5).
        "grave_mound" => size.clamp(2, 5),
        // flowers occupy a single cell; fallen_log handled separately (linear).
        "flower" | "fallen_log" => 0,
        // Unknown kind → single stump block.
        _ => 0,
    }
}

/// Greedy footprint reservation against the per-chunk feature occupancy grid:
/// if **any** cell in `footprint` is already occupied, returns `false` and leaves
/// the grid untouched (the caller skips placement); otherwise marks every cell
/// occupied and returns `true`. First-come / row-major so the outcome is
/// deterministic for a given seed (same chunk → same reservations).
fn reserve_footprint(
    occupied: &mut [[bool; CHUNK_SIZE as usize]; CHUNK_SIZE as usize],
    footprint: &[(i32, i32)],
) -> bool {
    if footprint
        .iter()
        .any(|&(fx, fz)| occupied[fz as usize][fx as usize])
    {
        return false;
    }
    for &(fx, fz) in footprint {
        occupied[fz as usize][fx as usize] = true;
    }
    true
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
    registry: &DecorationNbtRegistry,
) {
    // worldgen-v4 P6 §8.1 — NBT-driven path. The placement decision (this column,
    // this base_y) is already made by the cluster + density gate above; here we
    // only choose the geometry SOURCE. A decoration carrying authored NBT
    // variants stamps one (chosen deterministically by seed) instead of running
    // procedural geometry; flowers stay procedural (single block, never NBT).
    //
    // If the stamp resolves we are done; otherwise (template missing / fully
    // clipped / unresolved palette) we fall through to procedural so the column
    // is never left bare.
    let takes_nbt_path = deco.is_nbt_driven() && deco.kind != "flower";
    if takes_nbt_path
        && stamp_nbt_decoration(
            chunk, local_x, base_y, local_z, min_y, deco, world_x, world_z, registry,
        )
    {
        return;
    }

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

/// Resolve which [`DecorationAnchor`] a decoration stamps with. Sky-isle hanging
/// crystals always hang regardless of the manifest field (the procedural path
/// already special-cases them via `is_sky_isle_bottom_flora`); otherwise the
/// manifest `anchor` drives it (Ground / Embedded for grave mounds).
fn stamp_anchor_for(deco: &Decoration) -> DecorationAnchor {
    if is_sky_isle_bottom_flora(deco) {
        DecorationAnchor::Hanging
    } else {
        deco.anchor
    }
}

/// Stamp an authored NBT variant for `deco` so it lands at the same anchor point
/// the procedural path would have used (`base_y` is that anchor — one block above
/// the surface for Ground, the underside block for Hanging). Returns `true` when
/// a variant was resident and at least one block was written; `false` when the
/// chosen template was missing / fully unresolved, so the caller can fall back to
/// procedural geometry.
///
/// **memcpy-level** (§8.1 #10): goes through [`DecorationNbtRegistry::stamp`],
/// which only walks the already-decompressed block list — no runtime gzip.
///
/// Write mode mirrors the procedural geometry it replaces:
/// * `Embedded` (grave mounds) overwrites unconditionally so the dome "sinks"
///   into the surface (matching `place_grave_mound`'s `set_block_at_world`).
/// * `Ground` / `Hanging` write air-only so a stamp never erases neighbouring
///   terrain / structures (matching `set_block_if_air`).
#[allow(clippy::too_many_arguments)]
fn stamp_nbt_decoration(
    chunk: &mut UnloadedChunk,
    local_x: i32,
    base_y: i32,
    local_z: i32,
    min_y: i32,
    deco: &Decoration,
    world_x: i32,
    world_z: i32,
    registry: &DecorationNbtRegistry,
) -> bool {
    let Some(stamp) =
        nbt_stamp_placements(local_x, base_y, local_z, deco, world_x, world_z, registry)
    else {
        return false;
    };

    let mut wrote_any = false;
    let overwrite = stamp.anchor == DecorationAnchor::Embedded;
    for (lx, lz, world_y, state) in stamp.placements {
        let wrote = if overwrite {
            set_block_at_world(chunk, lx, world_y, lz, min_y, state)
        } else {
            set_block_if_air(chunk, lx, world_y, lz, min_y, state)
        };
        wrote_any |= wrote;
    }
    wrote_any
}

/// The chunk-local placements an NBT stamp resolves to, shared by the writer
/// ([`stamp_nbt_decoration`]) and the footprint predictor
/// ([`decoration_footprint`]) so the anti-overlap check and the actual write
/// agree on exactly which cells a stamp touches. `registry.stamp` is a pure
/// deterministic function of (template, surface_pos, anchor, rotation), so
/// computing it twice (once to check, once to write) yields identical cells.
struct NbtStamp {
    anchor: DecorationAnchor,
    /// `(local_x, local_z, world_y, block_state)` — chunk-local horizontal,
    /// world-absolute vertical. Cells outside the chunk (`local_x`/`local_z`
    /// not in `0..16`) are retained here; the writer clips them via
    /// `set_block_*` and the footprint predictor filters them out.
    placements: Vec<(i32, i32, i32, BlockState)>,
}

/// Resolve the chunk-local placements for `deco`'s NBT stamp, or `None` when no
/// variant is resident / the template is missing (caller falls back to
/// procedural). Mirrors exactly the anchor + surface_y + rotation math the
/// writer used inline before this was extracted.
fn nbt_stamp_placements(
    local_x: i32,
    base_y: i32,
    local_z: i32,
    deco: &Decoration,
    world_x: i32,
    world_z: i32,
    registry: &DecorationNbtRegistry,
) -> Option<NbtStamp> {
    // Choose the variant + rotation deterministically from the placement seed so
    // re-generating the same chunk stamps the identical bytes.
    let variant_idx = decoration_hash(world_x, world_z, NBT_VARIANT_SALT);
    let template_id = pick_nbt_template(deco, variant_idx)?;
    let anchor = stamp_anchor_for(deco);

    // Map the procedural anchor `base_y` to the registry's `surface_pos` so the
    // stamped origin lands exactly where the procedural geometry started:
    //   Ground   — procedural first block sits AT base_y; registry Ground places
    //              template[0,0,0] at surface_pos.y + 1 ⇒ surface_pos.y = base_y - 1.
    //   Embedded — procedural grave dome base sits at base_y - 1; registry
    //              Embedded places template[0,0,0] at surface_pos.y ⇒
    //              surface_pos.y = base_y - 1.
    //   Hanging  — procedural top body sits at base_y; registry Hanging places the
    //              template top at surface_pos.y - 1 ⇒ surface_pos.y = base_y + 1.
    let surface_y = match anchor {
        DecorationAnchor::Ground | DecorationAnchor::Embedded => base_y - 1,
        DecorationAnchor::Hanging => base_y + 1,
    };
    // The placement loop passes true world_x/world_z; the chunk origin is just
    // world − local, so we can turn the stamp's absolute world positions back
    // into chunk-local coordinates.
    let surface_pos = BlockPos::new(world_x, surface_y, world_z);
    let chunk_origin_x = world_x - local_x;
    let chunk_origin_z = world_z - local_z;

    // Directional variety: a quarter-turn about the column. Templates whose look
    // does not hinge on a `facing` property (logs, boulders, mounds, crystals)
    // rotate correctly with positions alone.
    let rotation = Rotation::from_index(decoration_hash(world_x, world_z, NBT_ROTATION_SALT));

    let (placements, _unresolved) = registry.stamp(&template_id, surface_pos, anchor, rotation)?;

    let placements = placements
        .into_iter()
        .map(|(pos, state, _block_nbt)| {
            (pos.x - chunk_origin_x, pos.z - chunk_origin_z, pos.y, state)
        })
        .collect();
    Some(NbtStamp { anchor, placements })
}

/// Pick the NBT template id this placement stamps from `deco.nbt_templates`,
/// indexed deterministically by `hash`. `None` when the decoration carries no
/// templates (caller falls back to procedural).
fn pick_nbt_template(deco: &Decoration, hash: u32) -> Option<String> {
    if deco.nbt_templates.is_empty() {
        return None;
    }
    let idx = (hash as usize) % deco.nbt_templates.len();
    Some(deco.nbt_templates[idx].clone())
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

/// Write `block` only if the target cell is air and in-bounds. Returns `true`
/// when a block was actually written (used by the NBT-stamp path to know whether
/// a variant produced any visible geometry, so it can fall back to procedural if
/// every cell was clipped / occupied).
fn set_block_if_air(
    chunk: &mut UnloadedChunk,
    local_x: i32,
    world_y: i32,
    local_z: i32,
    min_y: i32,
    block: BlockState,
) -> bool {
    if !(0..CHUNK_SIZE).contains(&local_x) || !(0..CHUNK_SIZE).contains(&local_z) {
        return false;
    }
    let local_y = world_y - min_y;
    if local_y < 0 || local_y >= chunk.height() as i32 {
        return false;
    }
    let state = chunk.block_state(local_x as u32, local_y as u32, local_z as u32);
    if !state.is_air() {
        return false;
    }
    chunk.set_block_state(local_x as u32, local_y as u32, local_z as u32, block);
    true
}

/// 无条件覆盖（不检查 air）—— 用于 grave_mound 这种要"切下去 / 半埋"的几何，
/// 让 dome 强制替换地表 grass_block / dirt 制造下沉视觉。Returns `true` when the
/// cell was in-bounds and written.
fn set_block_at_world(
    chunk: &mut UnloadedChunk,
    local_x: i32,
    world_y: i32,
    local_z: i32,
    min_y: i32,
    block: BlockState,
) -> bool {
    if !(0..CHUNK_SIZE).contains(&local_x) || !(0..CHUNK_SIZE).contains(&local_z) {
        return false;
    }
    let local_y = world_y - min_y;
    if local_y < 0 || local_y >= chunk.height() as i32 {
        return false;
    }
    chunk.set_block_state(local_x as u32, local_y as u32, local_z as u32, block);
    true
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

#[cfg(test)]
mod nbt_stamp_tests {
    //! worldgen-v4 P6 §6.1 / §8.1 #9 — flora NBT stamp wiring.
    //!
    //! These lock the *swap of geometry source*: an NBT-driven decoration stamps
    //! its authored variant, a template-less one keeps procedural geometry, and a
    //! decoration whose template is missing falls back rather than leaving a bare
    //! column. They also pin anchor placement (Ground / Embedded / Hanging),
    //! variant determinism + spread, and that single-block flowers never stamp.
    use super::*;
    use crate::world::terrain::nbt_io::{
        write_structure_nbt, PaletteEntry, StructureBlockEntry, StructureNbt, DATA_VERSION,
    };
    use crate::world::terrain::{MIN_Y, WORLD_HEIGHT};
    use std::collections::HashSet;
    use std::fs;
    use std::path::{Path, PathBuf};

    const TEST_MIN_Y: i32 = MIN_Y;

    fn make_chunk() -> UnloadedChunk {
        UnloadedChunk::with_height(WORLD_HEIGHT)
    }

    fn block_at(chunk: &UnloadedChunk, local_x: i32, world_y: i32, local_z: i32) -> BlockState {
        let local_y = (world_y - TEST_MIN_Y) as u32;
        chunk.block_state(local_x as u32, local_y, local_z as u32)
    }

    /// A single-block template of `block_name` at template-local origin (so anchor
    /// landing is observable by one cell).
    fn single_block_template(block_name: &str) -> StructureNbt {
        StructureNbt {
            data_version: DATA_VERSION,
            size: [1, 1, 1],
            palette: vec![PaletteEntry {
                name: format!("minecraft:{block_name}"),
                properties: vec![],
            }],
            blocks: vec![StructureBlockEntry {
                pos: [0, 0, 0],
                state: 0,
                block_nbt: None,
            }],
            entities: vec![],
        }
    }

    /// A 1×3×1 column (y=0,1,2) of `block_name` — for hanging-anchor depth checks.
    fn column_template(block_name: &str) -> StructureNbt {
        StructureNbt {
            data_version: DATA_VERSION,
            size: [1, 3, 1],
            palette: vec![PaletteEntry {
                name: format!("minecraft:{block_name}"),
                properties: vec![],
            }],
            blocks: (0..3)
                .map(|y| StructureBlockEntry {
                    pos: [0, y, 0],
                    state: 0,
                    block_nbt: None,
                })
                .collect(),
            entities: vec![],
        }
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "bong_flora_stamp_{}_{}_{:p}",
            tag,
            std::process::id(),
            &tag as *const _
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_template(dir: &Path, rel: &str, s: &StructureNbt) {
        let path = dir.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        write_structure_nbt(s, &path).unwrap();
    }

    fn deco(name: &str, kind: &str, templates: &[&str], anchor: DecorationAnchor) -> Decoration {
        Decoration {
            global_id: 1,
            profile: "spawn".into(),
            local_id: 1,
            name: name.into(),
            kind: kind.into(),
            blocks: vec!["oak_log".into(), "oak_leaves".into(), "moss_block".into()],
            size_range: [3, 6],
            rarity: 0.5,
            notes: String::new(),
            nbt_templates: templates.iter().map(|t| t.to_string()).collect(),
            anchor,
        }
    }

    // ── ① NBT-driven path stamps the authored variant ───────────────────────

    #[test]
    fn nbt_driven_tree_stamps_template_block_at_ground_anchor() {
        let dir = temp_dir("ground_tree");
        write_template(
            &dir,
            "decorations/small_tree/oak_round_v1.nbt",
            &single_block_template("amethyst_block"),
        );
        let reg = DecorationNbtRegistry::load(&dir).unwrap();
        let d = deco(
            "elder_oak",
            "tree",
            &["decorations/small_tree/oak_round_v1.nbt"],
            DecorationAnchor::Ground,
        );

        let mut chunk = make_chunk();
        let base_y = 70; // ground anchor: procedural would place first trunk block AT base_y
        place_decoration(&mut chunk, 5, base_y, 5, TEST_MIN_Y, &d, 105, 205, &reg);

        // The single-block template's y=0 must land at base_y (not the procedural
        // oak_log trunk — the authored amethyst_block proves the NBT path ran).
        assert_eq!(
            block_at(&chunk, 5, base_y, 5),
            BlockState::AMETHYST_BLOCK,
            "Ground NBT stamp must place the authored template block at base_y, \
             proving the procedural trunk geometry was retired"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    // ── ② template-less decoration keeps procedural geometry (backward compat) ─

    #[test]
    fn template_less_decoration_runs_procedural_geometry() {
        // Empty registry + a deco with NO templates → procedural tree: an oak_log
        // trunk at base_y. This is the §8.1 #9 fallback / backward-compat path.
        let reg = DecorationNbtRegistry::empty();
        let d = deco("elder_oak", "tree", &[], DecorationAnchor::Ground);

        let mut chunk = make_chunk();
        let base_y = 70;
        place_decoration(&mut chunk, 5, base_y, 5, TEST_MIN_Y, &d, 105, 205, &reg);

        assert_eq!(
            block_at(&chunk, 5, base_y, 5),
            BlockState::OAK_LOG,
            "a template-less tree must still build the procedural oak trunk (the \
             §8.1 #9 procedural fallback is unchanged)"
        );
    }

    // ── ③ NBT-driven but template missing → procedural fallback (never bare) ──

    #[test]
    fn nbt_driven_with_missing_template_falls_back_to_procedural() {
        let reg = DecorationNbtRegistry::empty(); // no templates resident
        let d = deco(
            "elder_oak",
            "tree",
            &["decorations/small_tree/does_not_exist.nbt"],
            DecorationAnchor::Ground,
        );

        let mut chunk = make_chunk();
        let base_y = 70;
        place_decoration(&mut chunk, 5, base_y, 5, TEST_MIN_Y, &d, 105, 205, &reg);

        assert_eq!(
            block_at(&chunk, 5, base_y, 5),
            BlockState::OAK_LOG,
            "when a referenced template is not resident the column must fall back \
             to procedural geometry (never left bare)"
        );
    }

    // ── ④ stamp determinism — same seed/pos stamps the identical chunk twice ──

    #[test]
    fn stamp_is_deterministic_across_two_runs() {
        let dir = temp_dir("determinism");
        // Two distinct variants so the variant pick is observable.
        write_template(
            &dir,
            "decorations/boulder/a_v1.nbt",
            &single_block_template("cobblestone"),
        );
        write_template(
            &dir,
            "decorations/boulder/b_v2.nbt",
            &single_block_template("mossy_cobblestone"),
        );
        let reg = DecorationNbtRegistry::load(&dir).unwrap();
        let d = deco(
            "wayfarer_rock",
            "boulder",
            &[
                "decorations/boulder/a_v1.nbt",
                "decorations/boulder/b_v2.nbt",
            ],
            DecorationAnchor::Ground,
        );

        let stamp_once = || {
            let mut chunk = make_chunk();
            place_decoration(&mut chunk, 8, 72, 8, TEST_MIN_Y, &d, 312, -417, &reg);
            block_at(&chunk, 8, 72, 8)
        };
        let a = stamp_once();
        let b = stamp_once();
        assert_eq!(
            a, b,
            "the same (deco, world pos) must stamp the identical variant block \
             both times (deterministic variant pick)"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    // ── ⑤ anchor tri-state: Ground / Embedded / Hanging land correctly ───────

    #[test]
    fn embedded_anchor_sinks_one_block_below_base_y() {
        let dir = temp_dir("embedded");
        write_template(
            &dir,
            "decorations/grave/small_v1.nbt",
            &single_block_template("cobblestone"),
        );
        let reg = DecorationNbtRegistry::load(&dir).unwrap();
        let d = deco(
            "wayfarer_grave",
            "grave_mound",
            &["decorations/grave/small_v1.nbt"],
            DecorationAnchor::Embedded,
        );

        let mut chunk = make_chunk();
        let base_y = 70;
        place_decoration(&mut chunk, 4, base_y, 4, TEST_MIN_Y, &d, 304, 404, &reg);

        // Embedded: surface_pos.y = base_y - 1, template[0,0,0] lands AT surface_pos.y.
        assert_eq!(
            block_at(&chunk, 4, base_y - 1, 4),
            BlockState::COBBLESTONE,
            "Embedded grave must sink the dome base one block below base_y (matching \
             the procedural dome_base = base_y - 1)"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn embedded_anchor_overwrites_existing_surface_block() {
        // Embedded must overwrite (the half-buried look), not skip occupied cells.
        let dir = temp_dir("embedded_overwrite");
        write_template(
            &dir,
            "decorations/grave/small_v1.nbt",
            &single_block_template("cobblestone"),
        );
        let reg = DecorationNbtRegistry::load(&dir).unwrap();
        let d = deco(
            "wayfarer_grave",
            "grave_mound",
            &["decorations/grave/small_v1.nbt"],
            DecorationAnchor::Embedded,
        );

        let mut chunk = make_chunk();
        let base_y = 70;
        // Pre-fill the sink cell with dirt — Embedded must replace it.
        let sink_local_y = (base_y - 1 - TEST_MIN_Y) as u32;
        chunk.set_block_state(4, sink_local_y, 4, BlockState::DIRT);
        place_decoration(&mut chunk, 4, base_y, 4, TEST_MIN_Y, &d, 304, 404, &reg);

        assert_eq!(
            block_at(&chunk, 4, base_y - 1, 4),
            BlockState::COBBLESTONE,
            "Embedded stamp must overwrite the existing surface block (half-buried \
             dome), not be clipped by the occupied cell"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn hanging_anchor_grows_downward_from_underside() {
        let dir = temp_dir("hanging");
        write_template(
            &dir,
            "decorations/hanging_crystal/amethyst_stalactite_v1.nbt",
            &column_template("amethyst_block"),
        );
        let reg = DecorationNbtRegistry::load(&dir).unwrap();
        // tian_mai_crystal: sky_isle + crystal name triggers is_sky_isle_bottom_flora,
        // which forces the Hanging anchor regardless of the manifest field.
        let mut d = deco(
            "tian_mai_crystal",
            "crystal",
            &["decorations/hanging_crystal/amethyst_stalactite_v1.nbt"],
            DecorationAnchor::Hanging,
        );
        d.profile = "sky_isle".into();

        let mut chunk = make_chunk();
        let base_y = 200; // the underside anchor (procedural top body sits here)
        place_decoration(&mut chunk, 6, base_y, 6, TEST_MIN_Y, &d, 306, 406, &reg);

        // Hanging: surface_pos.y = base_y + 1; registry places the column top at
        // surface_pos.y - 1 = base_y, growing down to base_y - 2.
        for y in (base_y - 2)..=base_y {
            assert_eq!(
                block_at(&chunk, 6, y, 6),
                BlockState::AMETHYST_BLOCK,
                "Hanging column must occupy y={y} (top at base_y={base_y}, growing down)"
            );
        }
        assert!(
            block_at(&chunk, 6, base_y + 1, 6).is_air(),
            "Hanging stamp must not place anything above base_y (no block at base_y+1)"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn stamp_anchor_for_forces_hanging_on_sky_isle_crystal() {
        // Even if the manifest said Ground, the sky-isle bottom crystal must hang.
        let mut d = deco("tian_mai_crystal", "crystal", &[], DecorationAnchor::Ground);
        d.profile = "sky_isle".into();
        assert_eq!(
            stamp_anchor_for(&d),
            DecorationAnchor::Hanging,
            "tian_mai_crystal always hangs (sky-isle underside special case)"
        );
        // A normal ground crystal keeps its manifest anchor.
        let g = deco("xuan_jing_pillar", "crystal", &[], DecorationAnchor::Ground);
        assert_eq!(stamp_anchor_for(&g), DecorationAnchor::Ground);
    }

    // ── ⑥ variant coverage — many placements hit more than one variant ───────

    #[test]
    fn variant_pick_spreads_across_the_pool() {
        let dir = temp_dir("variant_spread");
        write_template(
            &dir,
            "decorations/boulder/a_v1.nbt",
            &single_block_template("cobblestone"),
        );
        write_template(
            &dir,
            "decorations/boulder/b_v2.nbt",
            &single_block_template("mossy_cobblestone"),
        );
        write_template(
            &dir,
            "decorations/boulder/c_v3.nbt",
            &single_block_template("stone"),
        );
        let reg = DecorationNbtRegistry::load(&dir).unwrap();
        let d = deco(
            "wayfarer_rock",
            "boulder",
            &[
                "decorations/boulder/a_v1.nbt",
                "decorations/boulder/b_v2.nbt",
                "decorations/boulder/c_v3.nbt",
            ],
            DecorationAnchor::Ground,
        );

        let mut seen: HashSet<BlockState> = HashSet::new();
        for i in 0..200 {
            let wx = 1000 + i * 7;
            let wz = -2000 + i * 13;
            let mut chunk = make_chunk();
            place_decoration(&mut chunk, 0, 72, 0, TEST_MIN_Y, &d, wx, wz, &reg);
            seen.insert(block_at(&chunk, 0, 72, 0));
        }
        assert!(
            seen.len() >= 2,
            "across 200 placements the variant pick must hit >=2 distinct variants \
             (it hit {}); a single-variant result means pick_nbt_template is stuck",
            seen.len()
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn pick_nbt_template_indexes_in_range_and_is_deterministic() {
        let d = deco(
            "x",
            "boulder",
            &[
                "decorations/boulder/a_v1.nbt",
                "decorations/boulder/b_v2.nbt",
            ],
            DecorationAnchor::Ground,
        );
        for hash in [0u32, 1, 2, 3, 99, u32::MAX] {
            let a = pick_nbt_template(&d, hash);
            let b = pick_nbt_template(&d, hash);
            assert_eq!(a, b, "pick must be deterministic for hash {hash}");
            let t = a.unwrap();
            assert!(
                d.nbt_templates.contains(&t),
                "pick {t:?} must be one of the deco's templates"
            );
        }
        // Empty templates → None.
        let none = deco("x", "boulder", &[], DecorationAnchor::Ground);
        assert!(pick_nbt_template(&none, 7).is_none());
    }

    // ── ⑦ flowers never stamp — single-block procedural always ───────────────

    #[test]
    fn flower_stays_procedural_even_with_templates() {
        // A flower spec that (wrongly) carries templates must still place its single
        // procedural block, never an NBT stamp. blocks[0] for a flower deco = poppy.
        let dir = temp_dir("flower_proc");
        write_template(
            &dir,
            "decorations/small_tree/oak_round_v1.nbt",
            &single_block_template("amethyst_block"),
        );
        let reg = DecorationNbtRegistry::load(&dir).unwrap();
        let mut d = deco(
            "meadow_poppy",
            "flower",
            &["decorations/small_tree/oak_round_v1.nbt"],
            DecorationAnchor::Ground,
        );
        d.blocks = vec!["poppy".into()];

        let mut chunk = make_chunk();
        place_decoration(&mut chunk, 2, 65, 2, TEST_MIN_Y, &d, 102, 202, &reg);
        assert_eq!(
            block_at(&chunk, 2, 65, 2),
            BlockState::POPPY,
            "flowers must always place their single procedural block (never an NBT \
             stamp), even if a template slipped onto the spec"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    // ── ⑨ geometry-source swap leaves the placement ANCHOR invariant ─────────

    #[test]
    fn swap_to_nbt_keeps_the_placement_anchor_at_base_y() {
        // The cluster + density gate (in decorate_chunk) decides the column and
        // base_y; place_decoration only chooses the geometry SOURCE. So both the
        // procedural and NBT path must anchor at the SAME base_y — proving the
        // swap does not shift where decorations land for a given seed. Procedural
        // tree → oak_log trunk AT base_y; NBT tree → template block AT base_y.
        let dir = temp_dir("anchor_invariant");
        write_template(
            &dir,
            "decorations/small_tree/oak_round_v1.nbt",
            &single_block_template("amethyst_block"),
        );
        let reg_nbt = DecorationNbtRegistry::load(&dir).unwrap();
        let reg_empty = DecorationNbtRegistry::empty();

        let base_y = 71;
        let proc_deco = deco("elder_oak", "tree", &[], DecorationAnchor::Ground);
        let nbt_deco = deco(
            "elder_oak",
            "tree",
            &["decorations/small_tree/oak_round_v1.nbt"],
            DecorationAnchor::Ground,
        );

        let mut proc_chunk = make_chunk();
        place_decoration(
            &mut proc_chunk,
            7,
            base_y,
            7,
            TEST_MIN_Y,
            &proc_deco,
            700,
            700,
            &reg_empty,
        );
        let mut nbt_chunk = make_chunk();
        place_decoration(
            &mut nbt_chunk,
            7,
            base_y,
            7,
            TEST_MIN_Y,
            &nbt_deco,
            700,
            700,
            &reg_nbt,
        );

        // Both anchor at base_y (just different block — geometry source swapped).
        assert!(
            !proc_chunk
                .block_state(7, (base_y - TEST_MIN_Y) as u32, 7)
                .is_air(),
            "procedural path must occupy the base_y anchor"
        );
        assert_eq!(
            block_at(&nbt_chunk, 7, base_y, 7),
            BlockState::AMETHYST_BLOCK,
            "NBT path must occupy the SAME base_y anchor (placement position invariant)"
        );
        assert_ne!(
            block_at(&proc_chunk, 7, base_y, 7),
            block_at(&nbt_chunk, 7, base_y, 7),
            "only the geometry source changed — the procedural trunk vs the authored \
             template block differ at the same anchor"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    // ── ⑩ §8.1 #9 retain — only NBT-driven non-flower kinds take the stamp path

    #[test]
    fn only_nbt_driven_non_flower_takes_stamp_path() {
        let dir = temp_dir("retain_gate");
        write_template(
            &dir,
            "decorations/boulder/a_v1.nbt",
            &single_block_template("amethyst_block"),
        );
        let reg = DecorationNbtRegistry::load(&dir).unwrap();

        // (a) NBT-driven boulder → stamps the template block.
        let boulder = deco(
            "wayfarer_rock",
            "boulder",
            &["decorations/boulder/a_v1.nbt"],
            DecorationAnchor::Ground,
        );
        let mut c1 = make_chunk();
        place_decoration(&mut c1, 1, 72, 1, TEST_MIN_Y, &boulder, 11, 11, &reg);
        assert_eq!(
            block_at(&c1, 1, 72, 1),
            BlockState::AMETHYST_BLOCK,
            "(a) NBT-driven boulder must stamp the authored template"
        );

        // (b) template-less boulder → procedural (cobblestone family at base_y).
        let proc_boulder = deco("wayfarer_rock", "boulder", &[], DecorationAnchor::Ground);
        let mut c2 = make_chunk();
        place_decoration(&mut c2, 1, 72, 1, TEST_MIN_Y, &proc_boulder, 11, 11, &reg);
        assert_ne!(
            block_at(&c2, 1, 72, 1),
            BlockState::AMETHYST_BLOCK,
            "(b) a template-less boulder must NOT stamp — it stays procedural"
        );

        // (c) flower with templates → still procedural single block (§8.1 #9 retain).
        let mut flower = deco(
            "wild_grass",
            "flower",
            &["decorations/boulder/a_v1.nbt"],
            DecorationAnchor::Ground,
        );
        flower.blocks = vec!["grass".into()];
        let mut c3 = make_chunk();
        place_decoration(&mut c3, 1, 65, 1, TEST_MIN_Y, &flower, 11, 11, &reg);
        assert_eq!(
            block_at(&c3, 1, 65, 1),
            BlockState::GRASS,
            "(c) flowers are §8.1 #9 retained-procedural — never stamp even with templates"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    // ── ⑧ stamp returns false (so caller can fall back) when fully clipped ───

    #[test]
    fn ground_stamp_falls_back_when_every_cell_is_occupied() {
        // If a Ground stamp's cells are all non-air, set_block_if_air writes nothing
        // and stamp_nbt_decoration returns false → caller falls back to procedural.
        let dir = temp_dir("clipped");
        write_template(
            &dir,
            "decorations/boulder/a_v1.nbt",
            &single_block_template("amethyst_block"),
        );
        let reg = DecorationNbtRegistry::load(&dir).unwrap();
        let d = deco(
            "wayfarer_rock",
            "boulder",
            &["decorations/boulder/a_v1.nbt"],
            DecorationAnchor::Ground,
        );

        let mut chunk = make_chunk();
        let base_y = 72;
        // Occupy the Ground stamp target (base_y) with stone — the air-only stamp
        // writes nothing there.
        let occ_y = (base_y - TEST_MIN_Y) as u32;
        chunk.set_block_state(3, occ_y, 3, BlockState::STONE);
        let wrote = stamp_nbt_decoration(&mut chunk, 3, base_y, 3, TEST_MIN_Y, &d, 303, 303, &reg);
        assert!(
            !wrote,
            "a fully-clipped Ground stamp must report no blocks written so the caller \
             falls back to procedural"
        );
        // Pre-existing block is untouched (air-only stamp).
        assert_eq!(block_at(&chunk, 3, base_y, 3), BlockState::STONE);
        let _ = fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod anti_overlap_tests {
    //! worldgen-v4 P6 §8.1 — footprint-aware anti-overlap.
    //!
    //! When NBT decorations replaced the old 1–3 cell procedural geometry their
    //! footprints grew to 5–15 cells (the rift_bridge spans ~13–15), so two
    //! scatter points landing close interpenetrated — each feature used to mark
    //! only its anchor cell. These lock the fix: a decoration reserves its WHOLE
    //! footprint; a second feature whose footprint touches it is skipped (not
    //! written, not fallen back to procedural); the reservation is deterministic;
    //! and the ground-cover loop reads the full footprint, not just the anchor.
    use super::*;
    use crate::world::terrain::nbt_io::{
        write_structure_nbt, PaletteEntry, StructureBlockEntry, StructureNbt, DATA_VERSION,
    };
    use std::collections::HashSet;
    use std::fs;
    use std::path::{Path, PathBuf};

    /// `(local_x, local_z)` chunk-ground cells. Aliased so `greedy_place`'s
    /// signature stays readable (and dodges clippy::type_complexity).
    type Cells = Vec<(i32, i32)>;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "bong_flora_overlap_{}_{}_{:p}",
            tag,
            std::process::id(),
            &tag as *const _
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_template(dir: &Path, rel: &str, s: &StructureNbt) {
        let path = dir.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        write_structure_nbt(s, &path).unwrap();
    }

    /// A solid `w × 1 × w` plate at template y=0 — a known multi-cell footprint
    /// that stands in for a large authored NBT (e.g. a bridge deck).
    fn plate_template(w: i32, block_name: &str) -> StructureNbt {
        let mut blocks = Vec::new();
        for z in 0..w {
            for x in 0..w {
                blocks.push(StructureBlockEntry {
                    pos: [x, 0, z],
                    state: 0,
                    block_nbt: None,
                });
            }
        }
        StructureNbt {
            data_version: DATA_VERSION,
            size: [w, 1, w],
            palette: vec![PaletteEntry {
                name: format!("minecraft:{block_name}"),
                properties: vec![],
            }],
            blocks,
            entities: vec![],
        }
    }

    fn deco(name: &str, kind: &str, templates: &[&str], anchor: DecorationAnchor) -> Decoration {
        Decoration {
            global_id: 1,
            profile: "spawn".into(),
            local_id: 1,
            name: name.into(),
            kind: kind.into(),
            blocks: vec!["oak_log".into(), "oak_leaves".into(), "moss_block".into()],
            size_range: [3, 6],
            rarity: 0.5,
            notes: String::new(),
            nbt_templates: templates.iter().map(|t| t.to_string()).collect(),
            anchor,
        }
    }

    /// Build a registry holding a single `w×1×w` plate variant for `kind`.
    fn plate_registry(dir: &Path, kind: &str, w: i32) -> DecorationNbtRegistry {
        write_template(
            dir,
            &format!("decorations/{kind}/plate_v1.nbt"),
            &plate_template(w, "stone"),
        );
        DecorationNbtRegistry::load(dir).unwrap()
    }

    // ── footprint cell-set correctness ───────────────────────────────────────

    #[test]
    fn nbt_footprint_covers_whole_plate_not_just_anchor() {
        // A 4×4 plate stamped at the chunk-interior anchor must reserve 16 cells
        // (the corner-anchored stamp grows +x/+z from the anchor). Rotation only
        // re-orients the same 16 cells, so the count is rotation-invariant.
        let dir = temp_dir("plate4");
        let reg = plate_registry(&dir, "boulder", 4);
        let d = deco(
            "wide_rock",
            "boulder",
            &["decorations/boulder/plate_v1.nbt"],
            DecorationAnchor::Ground,
        );
        // Anchor at (2,2) so the +x/+z plate stays inside the chunk for every
        // rotation? Rotations can push −x/−z; cells outside the chunk are clipped.
        // Put it near the centre and just assert it is a multi-cell footprint that
        // never includes an out-of-bounds cell and always contains the anchor.
        let fp = decoration_footprint(6, 6, 70, &d, 1006, 2006, &reg);
        assert!(
            fp.contains(&(6, 6)),
            "footprint must always include the anchor column (6,6); got {fp:?}"
        );
        assert!(
            fp.len() > 1,
            "a 4×4 NBT plate must reserve more than the single anchor cell \
             (the whole footprint) — got {} cell(s): {fp:?}",
            fp.len()
        );
        for &(fx, fz) in &fp {
            assert!(
                (0..CHUNK_SIZE).contains(&fx) && (0..CHUNK_SIZE).contains(&fz),
                "footprint must only contain in-chunk cells (cross-chunk clip is \
                 the P7 residual); got out-of-bounds ({fx},{fz})"
            );
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn footprint_clipped_out_of_chunk_still_reserves_anchor() {
        // A plate whose body extends off the +x edge: cells past x=15 are clipped,
        // but the anchor column must still be reserved so ground cover yields.
        let dir = temp_dir("clip_edge");
        let reg = plate_registry(&dir, "boulder", 5);
        let d = deco(
            "edge_rock",
            "boulder",
            &["decorations/boulder/plate_v1.nbt"],
            DecorationAnchor::Ground,
        );
        let fp = decoration_footprint(14, 8, 70, &d, 1014, 2008, &reg);
        assert!(
            fp.contains(&(14, 8)),
            "anchor (14,8) must be reserved even when most plate cells are clipped \
             off the chunk edge; got {fp:?}"
        );
        for &(fx, fz) in &fp {
            assert!((0..CHUNK_SIZE).contains(&fx) && (0..CHUNK_SIZE).contains(&fz));
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn procedural_footprint_is_a_disc_for_template_less_boulder() {
        // Template-less → procedural path → conservative disc. A boulder reserves
        // a radius≥2 disc, not just the anchor, so neighbouring procedural
        // features also avoid interpenetration.
        let reg = DecorationNbtRegistry::empty();
        let d = deco("rock", "boulder", &[], DecorationAnchor::Ground);
        let fp = decoration_footprint(8, 8, 70, &d, 108, 208, &reg);
        assert!(
            fp.contains(&(8, 8)) && fp.contains(&(8 + 2, 8)) && fp.contains(&(8, 8 + 2)),
            "a procedural boulder must reserve at least a radius-2 disc around the \
             anchor; got {fp:?}"
        );
    }

    #[test]
    fn flower_footprint_is_single_cell() {
        // Flowers never stamp and have no spread — exactly one reserved cell.
        let reg = DecorationNbtRegistry::empty();
        let mut d = deco("poppy", "flower", &[], DecorationAnchor::Ground);
        d.blocks = vec!["poppy".into()];
        let fp = decoration_footprint(5, 5, 65, &d, 105, 205, &reg);
        assert_eq!(
            fp,
            vec![(5, 5)],
            "a flower occupies exactly its own cell (single-cell footprint)"
        );
    }

    // ── greedy reservation contract (reserve_footprint) ──────────────────────

    #[test]
    fn reserve_succeeds_on_disjoint_and_marks_all_cells() {
        let mut grid = [[false; CHUNK_SIZE as usize]; CHUNK_SIZE as usize];
        let fp = vec![(1, 1), (2, 1), (1, 2)];
        assert!(
            reserve_footprint(&mut grid, &fp),
            "reservation must succeed when every cell is free"
        );
        for &(x, z) in &fp {
            assert!(
                grid[z as usize][x as usize],
                "cell ({x},{z}) must be marked occupied after a successful reservation"
            );
        }
        // A cell NOT in the footprint stays free.
        assert!(!grid[5][5], "untouched cells must remain free");
    }

    #[test]
    fn reserve_fails_and_leaves_grid_untouched_on_any_collision() {
        let mut grid = [[false; CHUNK_SIZE as usize]; CHUNK_SIZE as usize];
        grid[3][3] = true; // pre-occupied by an earlier feature
        let fp = vec![(2, 3), (3, 3), (4, 3)]; // middle cell collides
        assert!(
            !reserve_footprint(&mut grid, &fp),
            "reservation must fail when ANY footprint cell is already occupied"
        );
        // The free cells of the rejected footprint must NOT have been marked
        // (all-or-nothing) — only the pre-existing (3,3) stays set.
        assert!(
            !grid[3][2],
            "(2,3) must stay free after a rejected reservation"
        );
        assert!(
            !grid[3][4],
            "(4,3) must stay free after a rejected reservation"
        );
        assert!(grid[3][3], "the pre-existing occupant must remain");
    }

    // ── end-to-end: two adjacent large stamps cannot share a cell ────────────

    /// Simulate the feature loop's greedy reservation over a row-major sweep of
    /// columns, returning the placed anchors and the final occupancy grid. This
    /// mirrors `decorate_chunk`'s reserve/skip without needing a full
    /// `TerrainProvider` (the gate that decides candidate columns is unchanged;
    /// what we lock here is that reserved footprints never overlap).
    fn greedy_place(
        candidates: &[(i32, i32)],
        base_y: i32,
        d: &Decoration,
        reg: &DecorationNbtRegistry,
    ) -> (Cells, Cells) {
        let mut occupied = [[false; CHUNK_SIZE as usize]; CHUNK_SIZE as usize];
        let mut placed = Vec::new();
        for &(lx, lz) in candidates {
            let fp = decoration_footprint(lx, lz, base_y, d, 1000 + lx, 2000 + lz, reg);
            if reserve_footprint(&mut occupied, &fp) {
                placed.push((lx, lz));
            }
        }
        let mut occ_cells = Vec::new();
        for (z, row) in occupied.iter().enumerate() {
            for (x, &set) in row.iter().enumerate() {
                if set {
                    occ_cells.push((x as i32, z as i32));
                }
            }
        }
        (placed, occ_cells)
    }

    #[test]
    fn adjacent_large_footprints_never_share_a_cell() {
        // Three candidate anchors one cell apart along x — their 4×4 plates would
        // heavily overlap. After greedy reservation, the union of occupied cells
        // must equal the sum of each placed footprint's size (no shared cell).
        let dir = temp_dir("adjacent_big");
        let reg = plate_registry(&dir, "boulder", 4);
        let d = deco(
            "wide_rock",
            "boulder",
            &["decorations/boulder/plate_v1.nbt"],
            DecorationAnchor::Ground,
        );
        let candidates = [(2, 2), (3, 2), (4, 2)];
        let (placed, occ) = greedy_place(&candidates, 70, &d, &reg);

        // At least the first one places; overlapping neighbours are skipped.
        assert!(
            !placed.is_empty(),
            "at least the first candidate must place"
        );
        // The occupied set must have NO duplicates and each placed footprint's
        // cells must be pairwise disjoint — sum of footprint sizes == |union|.
        let mut expected_total = 0usize;
        for &(lx, lz) in &placed {
            expected_total +=
                decoration_footprint(lx, lz, 70, &d, 1000 + lx, 2000 + lz, &reg).len();
        }
        let unique: HashSet<(i32, i32)> = occ.iter().copied().collect();
        assert_eq!(
            unique.len(),
            occ.len(),
            "occupancy grid must contain no duplicate cells"
        );
        assert_eq!(
            occ.len(),
            expected_total,
            "placed footprints must be pairwise disjoint: |union of occupied| ({}) \
             must equal Σ|footprint| ({}). A smaller union means two stamps shared \
             a cell (interpenetration).",
            occ.len(),
            expected_total
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn second_stamp_whose_footprint_overlaps_the_first_is_skipped() {
        // The user's reported case: a large authored footprint (rift_bridge scale)
        // and a second feature whose footprint lands ON it. The second must be
        // skipped — not squeezed into the gaps (the old interpenetration bug).
        //
        // Rotation is seeded per world pos, so a fixed 1-cell offset does NOT
        // always collide (it may rotate clear — that's a legit disjoint case). So
        // we SCAN neighbour offsets for a pair whose footprints provably intersect,
        // assert the premise, then assert the second is skipped. This locks the
        // real invariant "overlap ⇒ skip" without baking in a rotation assumption.
        let dir = temp_dir("overlap_skip");
        let reg = plate_registry(&dir, "ruins_pillar", 4);
        let d = deco(
            "bridge_like",
            "ruins_pillar",
            &["decorations/ruins_pillar/plate_v1.nbt"],
            DecorationAnchor::Ground,
        );
        let first = (5, 5);
        let fp_first: HashSet<(i32, i32)> = decoration_footprint(
            first.0,
            first.1,
            70,
            &d,
            1000 + first.0,
            2000 + first.1,
            &reg,
        )
        .into_iter()
        .collect();

        // Find a neighbour anchor whose footprint shares a cell with the first.
        let mut overlapping_neighbour = None;
        'scan: for dz in -2..=2 {
            for dx in -2..=2 {
                if dx == 0 && dz == 0 {
                    continue;
                }
                let n = (first.0 + dx, first.1 + dz);
                let fp_n: HashSet<(i32, i32)> =
                    decoration_footprint(n.0, n.1, 70, &d, 1000 + n.0, 2000 + n.1, &reg)
                        .into_iter()
                        .collect();
                if fp_n.intersection(&fp_first).next().is_some() {
                    overlapping_neighbour = Some(n);
                    break 'scan;
                }
            }
        }
        let neighbour = overlapping_neighbour.expect(
            "test premise: a 4×4 plate must have at least one nearby anchor whose \
             footprint overlaps it (if none, the scenario can't reproduce overlap)",
        );

        // Greedy in first-then-neighbour order: only the first must place.
        let (placed, _occ) = greedy_place(&[first, neighbour], 70, &d, &reg);
        assert_eq!(
            placed,
            vec![first],
            "the first stamp claims its footprint; the neighbour {neighbour:?} whose \
             footprint overlaps it must be skipped (no interpenetration). placed={placed:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn far_apart_large_stamps_both_place() {
        // Sanity / no over-rejection: two 4×4 plates far enough apart that their
        // footprints are disjoint must BOTH place — the fix must not starve density.
        let dir = temp_dir("far_apart");
        let reg = plate_registry(&dir, "boulder", 4);
        let d = deco(
            "wide_rock",
            "boulder",
            &["decorations/boulder/plate_v1.nbt"],
            DecorationAnchor::Ground,
        );
        let (placed, _occ) = greedy_place(&[(1, 1), (10, 10)], 70, &d, &reg);
        assert_eq!(
            placed.len(),
            2,
            "two well-separated large stamps must both place (anti-overlap must not \
             reject non-colliding features); got placed={placed:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    // ── determinism ──────────────────────────────────────────────────────────

    #[test]
    fn footprint_is_deterministic_for_same_seed() {
        let dir = temp_dir("det");
        let reg = plate_registry(&dir, "boulder", 3);
        let d = deco(
            "rock",
            "boulder",
            &["decorations/boulder/plate_v1.nbt"],
            DecorationAnchor::Ground,
        );
        let a = decoration_footprint(7, 7, 70, &d, 1234, -5678, &reg);
        let b = decoration_footprint(7, 7, 70, &d, 1234, -5678, &reg);
        assert_eq!(
            a, b,
            "the same (deco, world pos) must yield the identical footprint cell \
             list both times (deterministic reservation)"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    // ── ground-cover avoidance reads the FULL footprint ──────────────────────

    #[test]
    fn ground_cover_loop_sees_full_footprint_marked() {
        // After a feature reserves a multi-cell footprint, every cell — not just
        // the anchor — is occupied, so the ground-cover loop (which skips occupied
        // cells) will not grow grass under any part of the feature.
        let dir = temp_dir("gc_avoid");
        let reg = plate_registry(&dir, "boulder", 3);
        let d = deco(
            "rock",
            "boulder",
            &["decorations/boulder/plate_v1.nbt"],
            DecorationAnchor::Ground,
        );
        let mut occupied = [[false; CHUNK_SIZE as usize]; CHUNK_SIZE as usize];
        let fp = decoration_footprint(4, 4, 70, &d, 1004, 2004, &reg);
        assert!(reserve_footprint(&mut occupied, &fp));
        // Every reserved cell reads as occupied (what the ground-cover loop tests).
        let non_anchor: Vec<_> = fp.iter().copied().filter(|&c| c != (4, 4)).collect();
        assert!(
            !non_anchor.is_empty(),
            "a 3×3 plate must reserve cells beyond the anchor"
        );
        for (x, z) in non_anchor {
            assert!(
                occupied[z as usize][x as usize],
                "ground cover must see the non-anchor footprint cell ({x},{z}) as \
                 occupied (so no grass grows under the feature body, not just its anchor)"
            );
        }
        let _ = fs::remove_dir_all(&dir);
    }
}
