use valence::prelude::{BlockState, Chunk, ChunkPos, PropName, PropValue, UnloadedChunk};

use super::{column, raster::TerrainProvider};

pub fn decorate_chunk(
    chunk: &mut UnloadedChunk,
    pos: ChunkPos,
    min_y: i32,
    terrain: &TerrainProvider,
    top_y_by_column: &[[i32; 16]; 16],
) {
    for (local_z, row) in top_y_by_column.iter().enumerate() {
        for (local_x, &top_y) in row.iter().enumerate() {
            let world_x = pos.x * 16 + local_x as i32;
            let world_z = pos.z * 16 + local_z as i32;
            let sample = terrain.sample(world_x, world_z);
            let plant_y = top_y + 1;
            let density = decoration_hash(world_x, world_z, 17) % 1000;

            decorate_cave_column(chunk, local_x, local_z, min_y, world_x, world_z, &sample);

            if decorate_water_column(
                chunk, local_x, local_z, min_y, world_x, world_z, top_y, density, &sample,
            ) {
                continue;
            }

            if !can_place_above_surface(chunk, local_x as u32, plant_y, local_z as u32, min_y) {
                continue;
            }

            // 簇生 gate：把 8x8 与 16x16 两层 cell hash 平均，让花草成片而非均匀撒。
            // 双层 0–99 取平均 → 三角分布峰在 50；阈值 61 → P(score >= 61) ≈ 30% bare cell。
            // bare 标志下沉到各 helper：草/花/蕨/枯灌木分支按 bare skip，但树/竹保留按
            // 原 density 摆放（避免簇生 gate 误伤大型植被分布）。
            let cluster_a =
                decoration_hash(world_x.div_euclid(8), world_z.div_euclid(8), 31) % 100;
            let cluster_b =
                decoration_hash(world_x.div_euclid(16), world_z.div_euclid(16), 33) % 100;
            let bare_cell = (cluster_a + cluster_b) / 2 >= 61;

            if sample.is_wilderness_biome() {
                place_wilderness_vegetation(
                    chunk, local_x, local_z, plant_y, min_y, density, bare_cell,
                );
            } else if sample.is_peaks_biome() {
                place_peaks_vegetation(
                    chunk, local_x, local_z, plant_y, min_y, top_y, density, bare_cell,
                );
            } else if sample.is_marsh_biome() {
                place_marsh_vegetation(
                    chunk, local_x, local_z, plant_y, min_y, density, bare_cell,
                );
            } else if sample.is_spawn_biome() {
                place_spawn_vegetation(
                    chunk, local_x, local_z, plant_y, top_y, density, world_x, world_z, min_y,
                    bare_cell,
                );
            } else if sample.is_wastes_biome() {
                place_wastes_vegetation(
                    chunk, local_x, local_z, plant_y, min_y, density, bare_cell,
                );
            }
        }
    }

    super::mega_tree::decorate_chunk(chunk, pos, min_y, terrain);
}

#[allow(clippy::too_many_arguments)]
fn decorate_water_column(
    chunk: &mut UnloadedChunk,
    local_x: usize,
    local_z: usize,
    min_y: i32,
    world_x: i32,
    world_z: i32,
    top_y: i32,
    density: u32,
    sample: &super::raster::ColumnSample,
) -> bool {
    if sample.water_level < 0.0 {
        return false;
    }

    let water_top = sample.water_level.round() as i32;
    if top_y >= water_top {
        return false;
    }

    let water_depth = water_top - top_y;

    if sample.is_marsh_biome() {
        place_marsh_water_plants(
            chunk,
            local_x,
            local_z,
            min_y,
            top_y,
            water_top,
            water_depth,
            world_x,
            world_z,
        );
    }

    if sample.is_rift_biome() {
        place_rift_magma(
            chunk,
            local_x,
            local_z,
            min_y,
            top_y,
            water_depth,
            world_x,
            world_z,
            sample,
        );
    }

    maybe_place_lily_pad(chunk, local_x, local_z, min_y, water_top, density, sample);

    true
}

fn decorate_cave_column(
    chunk: &mut UnloadedChunk,
    local_x: usize,
    local_z: usize,
    min_y: i32,
    world_x: i32,
    world_z: i32,
    sample: &super::raster::ColumnSample,
) {
    if sample.cave_mask <= 0.6 {
        return;
    }

    let Some(cave) = column::cave_span_for_sample(sample, min_y, chunk.height() as i32) else {
        return;
    };
    let cavity_height = cave.carve_ceiling - cave.carve_floor + 1;
    if cavity_height < 3 {
        return;
    }

    let ceiling_support_y = cave.carve_ceiling + 1;
    let floor_support_y = cave.carve_floor - 1;
    if !has_solid_support(chunk, local_x, ceiling_support_y, local_z, min_y)
        || !has_solid_support(chunk, local_x, floor_support_y, local_z, min_y)
    {
        return;
    }

    let stalactite_hash = decoration_hash(world_x, world_z, 201) % 1000;
    if stalactite_hash < 80 {
        let max_length = (cavity_height - 1).min(6);
        let length = 1 + (decoration_hash(world_x, world_z, 211) % max_length as u32) as i32;
        place_dripstone_column(
            chunk,
            local_x,
            local_z,
            min_y,
            cave.carve_ceiling,
            length,
            true,
        );
    }

    let lichen_hash = decoration_hash(world_x, world_z, 223) % 1000;
    if sample.cave_mask > 0.7 && lichen_hash < 40 {
        set_block_if_air(
            chunk,
            local_x,
            cave.carve_ceiling,
            local_z,
            min_y,
            glow_lichen_ceiling(),
        );
    }

    let moss_hash = decoration_hash(world_x, world_z, 227) % 1000;
    if sample.cave_mask > 0.65 && moss_hash < 60 {
        set_block_if_air(
            chunk,
            local_x,
            cave.carve_floor,
            local_z,
            min_y,
            BlockState::MOSS_CARPET,
        );
    }

    let stalagmite_hash = decoration_hash(world_x, world_z, 229) % 1000;
    if stalagmite_hash < 50 {
        let max_length = (cavity_height - 1).min(4);
        let length = 1 + (decoration_hash(world_x, world_z, 233) % max_length as u32) as i32;
        place_dripstone_column(
            chunk,
            local_x,
            local_z,
            min_y,
            cave.carve_floor,
            length,
            false,
        );
    }

    let mushroom_hash = decoration_hash(world_x, world_z, 237) % 1000;
    if mushroom_hash < 45 {
        let block = if mushroom_hash < 30 {
            BlockState::BROWN_MUSHROOM
        } else {
            BlockState::RED_MUSHROOM
        };
        set_block_if_air(chunk, local_x, cave.carve_floor, local_z, min_y, block);
    }

    let vine_hash = decoration_hash(world_x, world_z, 241) % 1000;
    if sample.cave_mask > 0.68 && vine_hash < 55 {
        let vine_length = 1 + (decoration_hash(world_x, world_z, 243) % 3) as i32;
        for offset in 0..vine_length {
            let vy = cave.carve_ceiling - offset;
            if vy <= cave.carve_floor {
                break;
            }
            let block = if offset == vine_length - 1 {
                cave_vine_tip(decoration_hash(world_x, world_z, 247))
            } else {
                cave_vine_body(decoration_hash(world_x, world_z, 249 + offset as u32))
            };
            set_block_if_air(chunk, local_x, vy, local_z, min_y, block);
        }
    }

    if cave.carve_floor < -20 {
        let amethyst_hash = decoration_hash(world_x, world_z, 251) % 1000;
        if amethyst_hash < 25 {
            set_block_if_air(
                chunk,
                local_x,
                cave.carve_floor,
                local_z,
                min_y,
                BlockState::AMETHYST_CLUSTER
                    .set(PropName::Facing, PropValue::Up)
                    .set(PropName::Waterlogged, PropValue::False),
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn place_wilderness_vegetation(
    chunk: &mut UnloadedChunk,
    local_x: usize,
    local_z: usize,
    plant_y: i32,
    min_y: i32,
    density: u32,
    bare_cell: bool,
) {
    // wilderness 仅花草蕨，bare 直接 return
    if bare_cell {
        return;
    }
    if density < 28 {
        set_block_if_air(chunk, local_x, plant_y, local_z, min_y, BlockState::FERN);
    } else if density < 55 {
        set_block_if_air(chunk, local_x, plant_y, local_z, min_y, BlockState::GRASS);
    }
}

#[allow(clippy::too_many_arguments)]
fn place_peaks_vegetation(
    chunk: &mut UnloadedChunk,
    local_x: usize,
    local_z: usize,
    plant_y: i32,
    min_y: i32,
    top_y: i32,
    density: u32,
    bare_cell: bool,
) {
    // peaks 仅蕨，bare 直接 return
    if bare_cell {
        return;
    }
    if top_y < 200 && density < 20 {
        set_block_if_air(chunk, local_x, plant_y, local_z, min_y, BlockState::FERN);
    }
}

#[allow(clippy::too_many_arguments)]
fn place_marsh_vegetation(
    chunk: &mut UnloadedChunk,
    local_x: usize,
    local_z: usize,
    plant_y: i32,
    min_y: i32,
    density: u32,
    bare_cell: bool,
) {
    // bare cell 跳过 grass/fern，但保留 marsh_tree（避免簇生 gate 削沼泽树分布）
    if !bare_cell && density < 80 {
        set_block_if_air(chunk, local_x, plant_y, local_z, min_y, BlockState::GRASS);
    } else if !bare_cell && density < 120 {
        set_block_if_air(chunk, local_x, plant_y, local_z, min_y, BlockState::FERN);
    } else if density < 130 {
        place_marsh_tree(chunk, local_x, local_z, plant_y, min_y, density);
    }
}

#[allow(clippy::too_many_arguments)]
fn place_marsh_water_plants(
    chunk: &mut UnloadedChunk,
    local_x: usize,
    local_z: usize,
    min_y: i32,
    top_y: i32,
    water_top: i32,
    water_depth: i32,
    world_x: i32,
    world_z: i32,
) {
    let floor_y = top_y + 1;
    if water_depth <= 0 {
        return;
    }

    let seagrass_hash = decoration_hash(world_x, world_z, 311) % 1000;
    if water_depth <= 2 && seagrass_hash < 260 {
        set_block_if_matches(
            chunk,
            local_x,
            floor_y,
            local_z,
            min_y,
            BlockState::WATER,
            BlockState::SEAGRASS,
        );
        return;
    }

    let kelp_hash = decoration_hash(world_x, world_z, 313) % 1000;
    if water_depth >= 3 && kelp_hash < 180 {
        let desired_height = 3 + (decoration_hash(world_x, world_z, 317) % 6) as i32;
        let kelp_height = desired_height.min(water_depth);
        place_kelp_column(
            chunk,
            local_x,
            local_z,
            min_y,
            floor_y,
            water_top,
            kelp_height,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn place_rift_magma(
    chunk: &mut UnloadedChunk,
    local_x: usize,
    local_z: usize,
    min_y: i32,
    top_y: i32,
    water_depth: i32,
    world_x: i32,
    world_z: i32,
    sample: &super::raster::ColumnSample,
) {
    if water_depth <= 0 || sample.rift_axis_sdf > 0.78 {
        return;
    }

    let magma_hash = decoration_hash(world_x, world_z, 331) % 1000;
    if magma_hash >= 180 {
        return;
    }

    set_block_at_world(
        chunk,
        local_x,
        top_y,
        local_z,
        min_y,
        BlockState::MAGMA_BLOCK,
    );
}

fn maybe_place_lily_pad(
    chunk: &mut UnloadedChunk,
    local_x: usize,
    local_z: usize,
    min_y: i32,
    water_top: i32,
    density: u32,
    sample: &super::raster::ColumnSample,
) {
    let threshold = if sample.is_marsh_biome() {
        92
    } else if sample.is_spawn_biome() {
        28
    } else {
        18
    };
    if density >= threshold {
        return;
    }

    let pad_y = water_top + 1;
    if !matches!(
        block_state_at_world(chunk, local_x, water_top, local_z, min_y),
        Some(state) if state == BlockState::WATER
    ) {
        return;
    }

    set_block_if_air(chunk, local_x, pad_y, local_z, min_y, BlockState::LILY_PAD);
}

fn place_kelp_column(
    chunk: &mut UnloadedChunk,
    local_x: usize,
    local_z: usize,
    min_y: i32,
    floor_y: i32,
    water_top: i32,
    kelp_height: i32,
) {
    if kelp_height <= 0 {
        return;
    }

    for offset in 0..kelp_height {
        let world_y = floor_y + offset;
        if world_y > water_top {
            break;
        }

        let block = if offset == kelp_height - 1 {
            kelp_top_state((offset % 25) as u16)
        } else {
            BlockState::KELP_PLANT
        };
        set_block_if_matches(
            chunk,
            local_x,
            world_y,
            local_z,
            min_y,
            BlockState::WATER,
            block,
        );
    }
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
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
    bare_cell: bool,
) {
    // bare cell 跳过 dandelion/poppy/grass/fern 花草分支，但保留 oak/bamboo 大型植被
    if !bare_cell && density < 30 {
        set_block_if_air(chunk, local_x, plant_y, local_z, min_y, BlockState::DANDELION);
    } else if !bare_cell && density < 60 {
        set_block_if_air(chunk, local_x, plant_y, local_z, min_y, BlockState::POPPY);
    } else if !bare_cell && density < 110 {
        set_block_if_air(chunk, local_x, plant_y, local_z, min_y, BlockState::GRASS);
    } else if !bare_cell && density < 132 {
        set_block_if_air(chunk, local_x, plant_y, local_z, min_y, BlockState::FERN);
    } else if density < 144 {
        place_simple_oak(
            chunk,
            local_x,
            local_z,
            top_y + 1,
            min_y,
            decoration_hash(world_x, world_z, 91),
        );
    } else if density < 162 {
        place_bamboo_cluster(chunk, local_x, local_z, plant_y, min_y, world_x, world_z);
    }
}

fn place_marsh_tree(
    chunk: &mut UnloadedChunk,
    local_x: usize,
    local_z: usize,
    trunk_base_y: i32,
    min_y: i32,
    hash: u32,
) {
    let trunk_height = 3 + (hash % 3) as i32;
    let top_y = trunk_base_y + trunk_height;

    for dy in -2..0 {
        let root_y = trunk_base_y + dy;
        for (rdx, rdz) in [(1_i32, 0_i32), (-1, 0), (0, 1), (0, -1)] {
            if hash
                .wrapping_mul((rdx.wrapping_add(7)) as u32)
                .is_multiple_of(3)
            {
                continue;
            }
            let rx = local_x as i32 + rdx;
            let rz = local_z as i32 + rdz;
            if (0..16).contains(&rx) && (0..16).contains(&rz) {
                let local_y = root_y - min_y;
                if local_y >= 0 && local_y < chunk.height() as i32 {
                    chunk.set_block_state(
                        rx as u32,
                        local_y as u32,
                        rz as u32,
                        BlockState::DARK_OAK_LOG,
                    );
                }
            }
        }
    }

    for y in trunk_base_y..top_y {
        set_block_at_world(chunk, local_x, y, local_z, min_y, BlockState::DARK_OAK_LOG);
    }

    for canopy_y in (top_y - 1)..=(top_y + 1) {
        let radius: i32 = if canopy_y == top_y + 1 { 1 } else { 2 };
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                if dx.abs() == radius && dz.abs() == radius {
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
                        BlockState::DARK_OAK_LEAVES,
                    );
                }
            }
        }
    }
}

fn place_bamboo_cluster(
    chunk: &mut UnloadedChunk,
    local_x: usize,
    local_z: usize,
    base_y: i32,
    min_y: i32,
    world_x: i32,
    world_z: i32,
) {
    let hash = decoration_hash(world_x, world_z, 401);
    let height = 3 + (hash % 5) as i32;
    for dy in 0..height {
        let leaves = if dy == 0 {
            PropValue::None
        } else if dy < height - 1 {
            PropValue::Small
        } else {
            PropValue::Large
        };
        set_block_if_air(
            chunk,
            local_x,
            base_y + dy,
            local_z,
            min_y,
            BlockState::BAMBOO
                .set(PropName::Age, PropValue::_0)
                .set(PropName::Leaves, leaves)
                .set(PropName::Stage, PropValue::_0),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn place_wastes_vegetation(
    chunk: &mut UnloadedChunk,
    local_x: usize,
    local_z: usize,
    plant_y: i32,
    min_y: i32,
    density: u32,
    bare_cell: bool,
) {
    // wastes 仅 dead_bush，bare 直接 return
    if bare_cell {
        return;
    }
    if density < 8 {
        set_block_if_air(chunk, local_x, plant_y, local_z, min_y, BlockState::DEAD_BUSH);
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
        set_block_at_world(chunk, local_x, y, local_z, min_y, BlockState::OAK_LOG);
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

fn place_dripstone_column(
    chunk: &mut UnloadedChunk,
    local_x: usize,
    local_z: usize,
    min_y: i32,
    anchor_y: i32,
    length: i32,
    hanging: bool,
) {
    for offset in 0..length {
        let world_y = if hanging {
            anchor_y - offset
        } else {
            anchor_y + offset
        };
        set_block_if_air(
            chunk,
            local_x,
            world_y,
            local_z,
            min_y,
            pointed_dripstone_state(hanging, offset, length),
        );
    }
}

fn set_block_if_air(
    chunk: &mut UnloadedChunk,
    local_x: usize,
    world_y: i32,
    local_z: usize,
    min_y: i32,
    block: BlockState,
) {
    if !matches!(block_state_at_world(chunk, local_x, world_y, local_z, min_y), Some(state) if state.is_air())
    {
        return;
    }
    set_block_at_world(chunk, local_x, world_y, local_z, min_y, block);
}

fn set_block_if_matches(
    chunk: &mut UnloadedChunk,
    local_x: usize,
    world_y: i32,
    local_z: usize,
    min_y: i32,
    expected: BlockState,
    block: BlockState,
) {
    if !matches!(
        block_state_at_world(chunk, local_x, world_y, local_z, min_y),
        Some(state) if state == expected
    ) {
        return;
    }
    set_block_at_world(chunk, local_x, world_y, local_z, min_y, block);
}

fn set_block_at_world(
    chunk: &mut UnloadedChunk,
    local_x: usize,
    world_y: i32,
    local_z: usize,
    min_y: i32,
    block: BlockState,
) {
    let local_y = world_y - min_y;
    if !(0..16).contains(&(local_x as i32)) || !(0..16).contains(&(local_z as i32)) {
        return;
    }
    if local_y < 0 || local_y >= chunk.height() as i32 {
        return;
    }
    chunk.set_block_state(local_x as u32, local_y as u32, local_z as u32, block);
}

fn block_state_at_world(
    chunk: &UnloadedChunk,
    local_x: usize,
    world_y: i32,
    local_z: usize,
    min_y: i32,
) -> Option<BlockState> {
    let local_y = world_y - min_y;
    if !(0..16).contains(&(local_x as i32)) || !(0..16).contains(&(local_z as i32)) {
        return None;
    }
    if local_y < 0 || local_y >= chunk.height() as i32 {
        return None;
    }
    Some(chunk.block_state(local_x as u32, local_y as u32, local_z as u32))
}

fn has_solid_support(
    chunk: &UnloadedChunk,
    local_x: usize,
    world_y: i32,
    local_z: usize,
    min_y: i32,
) -> bool {
    matches!(
        block_state_at_world(chunk, local_x, world_y, local_z, min_y),
        Some(state) if !state.is_air() && state != BlockState::WATER
    )
}

fn pointed_dripstone_state(hanging: bool, offset: i32, length: i32) -> BlockState {
    let thickness = if length == 1 || offset == length - 1 {
        PropValue::Tip
    } else if offset == length - 2 {
        PropValue::Frustum
    } else if offset == 0 {
        PropValue::Base
    } else {
        PropValue::Middle
    };
    let direction = if hanging {
        PropValue::Down
    } else {
        PropValue::Up
    };

    BlockState::POINTED_DRIPSTONE
        .set(PropName::Thickness, thickness)
        .set(PropName::VerticalDirection, direction)
        .set(PropName::Waterlogged, PropValue::False)
}

fn glow_lichen_ceiling() -> BlockState {
    BlockState::GLOW_LICHEN
        .set(PropName::Up, PropValue::True)
        .set(PropName::Waterlogged, PropValue::False)
}

fn kelp_top_state(age: u16) -> BlockState {
    let age_value = match age.min(25) {
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
        15 => PropValue::_15,
        16 => PropValue::_16,
        17 => PropValue::_17,
        18 => PropValue::_18,
        19 => PropValue::_19,
        20 => PropValue::_20,
        21 => PropValue::_21,
        22 => PropValue::_22,
        23 => PropValue::_23,
        24 => PropValue::_24,
        _ => PropValue::_25,
    };
    BlockState::KELP.set(PropName::Age, age_value)
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

fn cave_vine_tip(hash: u32) -> BlockState {
    let age = (hash % 26) as u16;
    let age_value = match age {
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
        15 => PropValue::_15,
        16 => PropValue::_16,
        17 => PropValue::_17,
        18 => PropValue::_18,
        19 => PropValue::_19,
        20 => PropValue::_20,
        21 => PropValue::_21,
        22 => PropValue::_22,
        23 => PropValue::_23,
        24 => PropValue::_24,
        _ => PropValue::_25,
    };
    let berries = hash.is_multiple_of(5);
    BlockState::CAVE_VINES
        .set(PropName::Age, age_value)
        .set(PropName::Berries, berries.into())
}

fn cave_vine_body(hash: u32) -> BlockState {
    let berries = hash.is_multiple_of(7);
    BlockState::CAVE_VINES_PLANT.set(PropName::Berries, berries.into())
}

#[allow(dead_code)]
fn tall_grass_lower() -> BlockState {
    BlockState::TALL_GRASS.set(PropName::Half, PropValue::Lower)
}
