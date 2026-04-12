use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

use valence::prelude::{BlockState, Chunk, ChunkPos, DVec3, UnloadedChunk};

use super::{column, raster::TerrainProvider, spatial::ChunkBounds};

const TREE_CACHE_CAPACITY: usize = 24;

const TREE_PROFILES: [MegaTreeProfile; 4] = [
    // 灵木 — spawn 唯一地标巨树（世界树级别）
    MegaTreeProfile {
        kind: MegaTreeKind::SpiritWood,
        biome_matches: super::raster::ColumnSample::is_spawn_biome,
        seed_spacing: 2000,
        offset_margin: 200,
        min_surface_y: 60,
        max_surface_y: 250,
        max_slope: 20,
        max_extent: 120,
        dry_surface_only: true,
        chance_base: 0.95,
        chance_feature_scale: 0.05,
        chance_boundary_penalty: 0.0,
        trunk_height: (140, 180),
        crown_radius: (45, 60),
        crown_height: (50, 65),
        attraction_count: 800,
        step_size: 3.5,
        trunk_base_radius: (8.0, 12.0),
        trunk_taper: 0.78,
        branch_radius_ratio: 0.72,
        min_branch_radius: 1.2,
        leaf_radius: (4, 7),
        leaf_density: 0.82,
        upward_bias: 0.08,
        crown_shape: CrownShape::Ellipsoid,
        bare: false,
        root_count: 12,
    },
    MegaTreeProfile {
        kind: MegaTreeKind::AncientPine,
        biome_matches: super::raster::ColumnSample::is_peaks_biome,
        seed_spacing: 112,
        offset_margin: 20,
        min_surface_y: 96,
        max_surface_y: 220,
        max_slope: 14,
        max_extent: 42,
        dry_surface_only: true,
        chance_base: 0.14,
        chance_feature_scale: 0.40,
        chance_boundary_penalty: 0.30,
        trunk_height: (44, 64),
        crown_radius: (14, 22),
        crown_height: (28, 40),
        attraction_count: 400,
        step_size: 2.4,
        trunk_base_radius: (2.6, 4.2),
        trunk_taper: 0.71,
        branch_radius_ratio: 0.70,
        min_branch_radius: 0.75,
        leaf_radius: (3, 5),
        leaf_density: 0.68,
        upward_bias: 0.30,
        crown_shape: CrownShape::Cone,
        bare: false,
        root_count: 4,
    },
    // 枯木 — north_wastes 唯一地标巨树（巨大枯死古木）
    MegaTreeProfile {
        kind: MegaTreeKind::Deadwood,
        biome_matches: super::raster::ColumnSample::is_wastes_biome,
        seed_spacing: 2000,
        offset_margin: 200,
        min_surface_y: 60,
        max_surface_y: 250,
        max_slope: 20,
        max_extent: 80,
        dry_surface_only: true,
        chance_base: 0.95,
        chance_feature_scale: 0.05,
        chance_boundary_penalty: 0.0,
        trunk_height: (80, 110),
        crown_radius: (30, 45),
        crown_height: (35, 50),
        attraction_count: 350,
        step_size: 3.0,
        trunk_base_radius: (5.0, 8.0),
        trunk_taper: 0.70,
        branch_radius_ratio: 0.68,
        min_branch_radius: 0.90,
        leaf_radius: (0, 0),
        leaf_density: 0.0,
        upward_bias: 0.10,
        crown_shape: CrownShape::Ellipsoid,
        bare: true,
        root_count: 8,
    },
    MegaTreeProfile {
        kind: MegaTreeKind::SwampCypress,
        biome_matches: super::raster::ColumnSample::is_marsh_biome,
        seed_spacing: 96,
        offset_margin: 18,
        min_surface_y: 62,
        max_surface_y: 130,
        max_slope: 6,
        max_extent: 26,
        dry_surface_only: true,
        chance_base: 0.18,
        chance_feature_scale: 0.42,
        chance_boundary_penalty: 0.20,
        trunk_height: (28, 42),
        crown_radius: (10, 16),
        crown_height: (20, 30),
        attraction_count: 320,
        step_size: 2.2,
        trunk_base_radius: (2.4, 3.8),
        trunk_taper: 0.72,
        branch_radius_ratio: 0.72,
        min_branch_radius: 0.70,
        leaf_radius: (3, 5),
        leaf_density: 0.72,
        upward_bias: 0.22,
        crown_shape: CrownShape::Column,
        bare: false,
        root_count: 5,
    },
];

pub(super) fn decorate_chunk(
    chunk: &mut UnloadedChunk,
    pos: ChunkPos,
    min_y: i32,
    terrain: &TerrainProvider,
) {
    let bounds = ChunkBounds::from_chunk_pos(pos);
    let world_height = chunk.height() as i32;

    for profile in TREE_PROFILES {
        let cell_min_x = (bounds.min_x - profile.max_extent).div_euclid(profile.seed_spacing);
        let cell_max_x = (bounds.max_x + profile.max_extent).div_euclid(profile.seed_spacing);
        let cell_min_z = (bounds.min_z - profile.max_extent).div_euclid(profile.seed_spacing);
        let cell_max_z = (bounds.max_z + profile.max_extent).div_euclid(profile.seed_spacing);

        for cell_z in cell_min_z..=cell_max_z {
            for cell_x in cell_min_x..=cell_max_x {
                let Some(instance) =
                    instantiate_tree(profile, cell_x, cell_z, min_y, world_height, terrain)
                else {
                    continue;
                };
                if !instance.bounds.intersects_chunk(&bounds) {
                    continue;
                }
                place_tree_in_chunk(chunk, min_y, &bounds, &instance);
            }
        }
    }
}

#[derive(Clone, Copy)]
struct MegaTreeProfile {
    kind: MegaTreeKind,
    biome_matches: fn(&super::raster::ColumnSample) -> bool,
    seed_spacing: i32,
    offset_margin: i32,
    min_surface_y: i32,
    max_surface_y: i32,
    max_slope: i32,
    max_extent: i32,
    dry_surface_only: bool,
    chance_base: f64,
    chance_feature_scale: f64,
    chance_boundary_penalty: f64,
    trunk_height: (i32, i32),
    crown_radius: (i32, i32),
    crown_height: (i32, i32),
    attraction_count: usize,
    step_size: f64,
    trunk_base_radius: (f64, f64),
    trunk_taper: f64,
    branch_radius_ratio: f64,
    min_branch_radius: f64,
    leaf_radius: (i32, i32),
    leaf_density: f64,
    upward_bias: f64,
    crown_shape: CrownShape,
    bare: bool,
    root_count: usize,
}

#[derive(Clone, Copy)]
enum MegaTreeKind {
    SpiritWood,
    AncientPine,
    Deadwood,
    SwampCypress,
}

#[derive(Clone, Copy)]
enum CrownShape {
    Ellipsoid,
    Cone,
    Column,
}

impl CrownShape {
    fn contains(self, x: f64, y: f64, z: f64) -> bool {
        match self {
            Self::Ellipsoid => x * x + y * y + z * z <= 1.0,
            Self::Cone => {
                let ny = (y + 1.0) * 0.5;
                if !(0.0..=1.0).contains(&ny) {
                    return false;
                }
                let radius = 1.05 - ny * 0.85;
                x * x + z * z <= radius * radius
            }
            Self::Column => {
                let radial = x * x + z * z;
                radial <= 0.42 && y.abs() <= 1.0
            }
        }
    }
}

#[derive(Clone, Copy)]
struct MegaTreeParams {
    kind: MegaTreeKind,
    trunk_height: i32,
    crown_center_y: i32,
    crown_radii: (i32, i32),
    attraction_count: usize,
    influence_radius: f64,
    kill_distance: f64,
    step_size: f64,
    max_iterations: usize,
    trunk_base_radius: f64,
    trunk_taper: f64,
    branch_radius_ratio: f64,
    min_branch_radius: f64,
    leaf_radius: i32,
    leaf_density: f64,
    upward_bias: f64,
    crown_shape: CrownShape,
    bare: bool,
    root_count: usize,
}

struct MegaTreeInstance {
    base: WorldPos,
    seed: u64,
    params: MegaTreeParams,
    bounds: TreeBounds,
}

#[derive(Clone, Copy)]
struct WorldPos {
    x: i32,
    y: i32,
    z: i32,
}

impl WorldPos {
    fn as_dvec3(self) -> DVec3 {
        DVec3::new(self.x as f64, self.y as f64, self.z as f64)
    }
}

#[derive(Clone, Copy)]
struct TreeBounds {
    min_x: i32,
    max_x: i32,
    min_z: i32,
    max_z: i32,
}

impl TreeBounds {
    fn intersects_chunk(self, chunk: &ChunkBounds) -> bool {
        self.max_x >= chunk.min_x
            && self.min_x <= chunk.max_x
            && self.max_z >= chunk.min_z
            && self.min_z <= chunk.max_z
    }
}

#[derive(Clone)]
struct TreeNode {
    pos: DVec3,
    parent: Option<usize>,
    branch_depth: u8,
}

#[derive(Clone, Copy)]
struct Placement {
    block: BlockState,
    priority: u8,
}

fn instantiate_tree(
    profile: MegaTreeProfile,
    cell_x: i32,
    cell_z: i32,
    min_y: i32,
    world_height: i32,
    terrain: &TerrainProvider,
) -> Option<MegaTreeInstance> {
    let base_seed = hash_coords(cell_x, cell_z, profile.kind as u64 + 31);
    let span = profile.seed_spacing - profile.offset_margin * 2;
    if span <= 0 {
        return None;
    }

    let seed_x =
        cell_x * profile.seed_spacing + profile.offset_margin + range_i32(base_seed, 0, span - 1);
    let seed_z = cell_z * profile.seed_spacing
        + profile.offset_margin
        + range_i32(base_seed.rotate_left(17), 0, span - 1);

    let sample = terrain.sample(seed_x, seed_z);
    if !(profile.biome_matches)(&sample) {
        return None;
    }

    let surface_y = column::surface_y_for_sample(&sample, min_y, world_height);
    if surface_y < profile.min_surface_y || surface_y > profile.max_surface_y {
        return None;
    }

    if profile.dry_surface_only
        && sample.water_level >= 0.0
        && surface_y <= sample.water_level.round() as i32
    {
        return None;
    }

    if surface_slope(seed_x, seed_z, min_y, world_height, terrain) > profile.max_slope {
        return None;
    }

    let chance = (profile.chance_base
        + f64::from(sample.feature_mask) * profile.chance_feature_scale
        - f64::from(sample.boundary_weight) * profile.chance_boundary_penalty)
        .clamp(0.0, 0.95);
    if sample_u01(base_seed.rotate_left(29)) > chance {
        return None;
    }

    let trunk_height = range_i32(
        base_seed.rotate_left(7),
        profile.trunk_height.0,
        profile.trunk_height.1,
    );
    let crown_radius = range_i32(
        base_seed.rotate_left(13),
        profile.crown_radius.0,
        profile.crown_radius.1,
    );
    let crown_height = range_i32(
        base_seed.rotate_left(23),
        profile.crown_height.0,
        profile.crown_height.1,
    );
    let leaf_radius = range_i32(
        base_seed.rotate_left(31),
        profile.leaf_radius.0,
        profile.leaf_radius.1,
    );
    let trunk_base_radius = range_f64(
        base_seed.rotate_left(41),
        profile.trunk_base_radius.0,
        profile.trunk_base_radius.1,
    );

    let params = MegaTreeParams {
        kind: profile.kind,
        trunk_height,
        // Center crown at ~60% trunk height.  The lower 40% stays as bare
        // trunk; the upper 60% is where SCA branches grow outward.
        crown_center_y: trunk_height * 3 / 5,
        crown_radii: (crown_radius, crown_height),
        attraction_count: profile.attraction_count,
        // Derive influence_radius from the average spacing between attraction
        // points.  A multiplier of ~2.0 means each node "sees" a local
        // neighbourhood — small enough for distinct branches, large enough
        // to avoid dead zones.
        influence_radius: {
            let vol = crown_radius as f64
                * crown_height as f64
                * crown_radius as f64
                * std::f64::consts::FRAC_PI_6
                * 4.0;
            (vol / profile.attraction_count as f64).cbrt() * 2.0
        },
        kill_distance: 1.4,
        step_size: profile.step_size,
        max_iterations: profile.attraction_count.saturating_mul(4),
        trunk_base_radius,
        trunk_taper: profile.trunk_taper,
        branch_radius_ratio: profile.branch_radius_ratio,
        min_branch_radius: profile.min_branch_radius,
        leaf_radius,
        leaf_density: profile.leaf_density,
        upward_bias: profile.upward_bias,
        crown_shape: profile.crown_shape,
        bare: profile.bare,
        root_count: profile.root_count,
    };

    let horizontal_extent = crown_radius + leaf_radius + trunk_base_radius.ceil() as i32 + 10;
    Some(MegaTreeInstance {
        base: WorldPos {
            x: seed_x,
            y: surface_y + 1,
            z: seed_z,
        },
        seed: base_seed,
        params,
        bounds: TreeBounds {
            min_x: seed_x - horizontal_extent,
            max_x: seed_x + horizontal_extent,
            min_z: seed_z - horizontal_extent,
            max_z: seed_z + horizontal_extent,
        },
    })
}

fn place_tree_in_chunk(
    chunk: &mut UnloadedChunk,
    min_y: i32,
    chunk_bounds: &ChunkBounds,
    instance: &MegaTreeInstance,
) {
    let skeleton = cached_skeleton(instance);
    let mut child_count = vec![0usize; skeleton.len()];
    for node in &skeleton {
        if let Some(parent) = node.parent {
            child_count[parent] += 1;
        }
    }

    let mut placements = HashMap::new();
    rasterize_roots(&mut placements, chunk_bounds, instance, min_y);

    for (index, node) in skeleton.iter().enumerate() {
        let Some(parent_index) = node.parent else {
            continue;
        };
        let parent = &skeleton[parent_index];
        let radius = segment_radius(instance, node);
        rasterize_segment(
            &mut placements,
            chunk_bounds,
            parent.pos,
            node.pos,
            radius,
            log_block(instance.params.kind),
            2,
        );

        if !instance.params.bare
            && child_count[index] == 0
            && node.branch_depth >= 2
            && sample_u01(hash_coords(index as i32, instance.base.x, instance.seed))
                <= instance.params.leaf_density
        {
            rasterize_leaf_blob(&mut placements, chunk_bounds, node.pos, instance);
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
        if !can_replace(existing, placement.block) {
            continue;
        }
        chunk.set_block_state(local_x, local_y as u32, local_z, placement.block);
    }
}

fn generate_skeleton(instance: &MegaTreeInstance) -> Vec<TreeNode> {
    let mut rng = SimpleRng::new(instance.seed);
    let base = instance.base.as_dvec3();
    let params = instance.params;

    let trunk_steps = ((params.trunk_height as f64) / params.step_size).ceil() as usize;
    let mut nodes = Vec::with_capacity(trunk_steps + params.attraction_count / 2 + 1);
    nodes.push(TreeNode {
        pos: base,
        parent: None,
        branch_depth: 0,
    });

    let sway_x = rng.range_f64(-0.04, 0.04);
    let sway_z = rng.range_f64(-0.04, 0.04);
    for step in 1..=trunk_steps {
        let y = (step as f64 * params.step_size).min(params.trunk_height as f64);
        let drift =
            y * (params.trunk_height as f64 - y).max(0.0) / (params.trunk_height as f64 * 18.0);
        let pos = base
            + DVec3::new(
                drift * sway_x * params.trunk_height as f64,
                y,
                drift * sway_z * params.trunk_height as f64,
            );
        nodes.push(TreeNode {
            pos,
            parent: Some(nodes.len() - 1),
            branch_depth: 0,
        });
    }

    let mut attraction_points = sample_attraction_points(base, params, &mut rng);
    let min_distance_sq = (params.step_size * 0.75).powi(2);

    for _ in 0..params.max_iterations {
        let mut growth_vectors = vec![DVec3::ZERO; nodes.len()];
        let mut growth_counts = vec![0u16; nodes.len()];
        let mut has_influence = false;

        for point in &mut attraction_points {
            let mut nearest_index = None;
            let mut nearest_distance_sq = params.influence_radius * params.influence_radius;

            for (index, node) in nodes.iter().enumerate() {
                let delta = point.pos - node.pos;
                let distance_sq = delta.length_squared();
                if distance_sq <= params.kill_distance * params.kill_distance {
                    point.active = false;
                    nearest_index = None;
                    break;
                }
                if distance_sq < nearest_distance_sq {
                    nearest_distance_sq = distance_sq;
                    nearest_index = Some(index);
                }
            }

            if !point.active {
                continue;
            }

            let Some(nearest_index) = nearest_index else {
                continue;
            };
            let direction = normalize(point.pos - nodes[nearest_index].pos);
            if direction == DVec3::ZERO {
                continue;
            }
            growth_vectors[nearest_index] += direction;
            growth_counts[nearest_index] += 1;
            has_influence = true;
        }

        if !has_influence {
            break;
        }

        let previous_len = nodes.len();
        let mut spawned_positions = Vec::new();
        for index in 0..previous_len {
            if growth_counts[index] == 0 {
                continue;
            }

            let averaged = growth_vectors[index] / f64::from(growth_counts[index]);
            let growth_dir = normalize(
                averaged
                    + DVec3::new(
                        rng.range_f64(-0.08, 0.08),
                        params.upward_bias,
                        rng.range_f64(-0.08, 0.08),
                    ),
            );
            if growth_dir == DVec3::ZERO {
                continue;
            }

            let new_pos = nodes[index].pos + growth_dir * params.step_size;
            if nodes
                .iter()
                .any(|node| (node.pos - new_pos).length_squared() < min_distance_sq)
                || spawned_positions
                    .iter()
                    .any(|pos: &DVec3| (*pos - new_pos).length_squared() < min_distance_sq)
            {
                continue;
            }

            nodes.push(TreeNode {
                pos: new_pos,
                parent: Some(index),
                branch_depth: nodes[index].branch_depth.saturating_add(1),
            });
            spawned_positions.push(new_pos);
        }

        if nodes.len() == previous_len {
            break;
        }
        attraction_points.retain(|point| point.active);
        if attraction_points.is_empty() {
            break;
        }
    }

    nodes
}

#[derive(Clone, Copy)]
struct AttractionPoint {
    pos: DVec3,
    active: bool,
}

fn sample_attraction_points(
    base: DVec3,
    params: MegaTreeParams,
    rng: &mut SimpleRng,
) -> Vec<AttractionPoint> {
    let mut points = Vec::with_capacity(params.attraction_count);
    let mut attempts = 0usize;
    while points.len() < params.attraction_count && attempts < params.attraction_count * 16 {
        attempts += 1;
        let sample_x = rng.range_f64(-1.0, 1.0);
        let sample_y = rng.range_f64(-1.0, 1.0);
        let sample_z = rng.range_f64(-1.0, 1.0);
        if !params.crown_shape.contains(sample_x, sample_y, sample_z) {
            continue;
        }

        let world_pos = base
            + DVec3::new(
                sample_x * params.crown_radii.0 as f64,
                params.crown_center_y as f64 + sample_y * params.crown_radii.1 as f64,
                sample_z * params.crown_radii.0 as f64,
            );
        if world_pos.y <= base.y + params.step_size * 2.0 {
            continue;
        }
        points.push(AttractionPoint {
            pos: world_pos,
            active: true,
        });
    }
    points
}

fn rasterize_roots(
    placements: &mut HashMap<(i32, i32, i32), Placement>,
    chunk_bounds: &ChunkBounds,
    instance: &MegaTreeInstance,
    min_y: i32,
) {
    let base = instance.base.as_dvec3();
    let mut rng = SimpleRng::new(instance.seed.rotate_left(9));
    let root_radius = (instance.params.trunk_base_radius * 0.38).max(1.0);
    for index in 0..instance.params.root_count {
        let angle = (index as f64 / instance.params.root_count as f64) * std::f64::consts::TAU
            + rng.range_f64(-0.28, 0.28);
        let length = rng.range_f64(
            8.0 + instance.params.trunk_base_radius * 0.5,
            18.0 + instance.params.trunk_base_radius * 1.5,
        );
        let target = base
            + DVec3::new(
                angle.cos() * length,
                -rng.range_f64(3.0, 7.0 + instance.params.trunk_base_radius * 0.3),
                angle.sin() * length,
            );
        let block = if index % 2 == 0 {
            log_block(instance.params.kind)
        } else {
            BlockState::ROOTED_DIRT
        };
        rasterize_segment(
            placements,
            chunk_bounds,
            base + DVec3::new(0.0, 0.6, 0.0),
            target,
            root_radius,
            block,
            2,
        );
    }

    let local_y = instance.base.y - min_y;
    if local_y >= 0 {
        upsert_block(
            placements,
            chunk_bounds,
            instance.base.x,
            instance.base.y - 1,
            instance.base.z,
            BlockState::ROOTED_DIRT,
            2,
        );
    }
}

fn rasterize_leaf_blob(
    placements: &mut HashMap<(i32, i32, i32), Placement>,
    chunk_bounds: &ChunkBounds,
    center: DVec3,
    instance: &MegaTreeInstance,
) {
    let radius = instance.params.leaf_radius;
    if radius <= 0 {
        return;
    }

    for dy in -radius..=radius {
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                let dx_f = dx as f64 / radius as f64;
                let dy_f = dy as f64 / radius as f64;
                let dz_f = dz as f64 / radius as f64;
                let dist = dx_f * dx_f + dy_f * dy_f + dz_f * dz_f;
                if dist > 1.0 {
                    continue;
                }

                let world_x = center.x.round() as i32 + dx;
                let world_y = center.y.round() as i32 + dy;
                let world_z = center.z.round() as i32 + dz;
                if !chunk_bounds.contains_with_margin(world_x, world_z, radius + 2) {
                    continue;
                }
                let thinning = sample_u01(hash_coords(
                    world_x,
                    world_z,
                    world_y as u64 + instance.seed,
                ));
                if thinning > 0.75 - dist * 0.42 {
                    continue;
                }
                upsert_block(
                    placements,
                    chunk_bounds,
                    world_x,
                    world_y,
                    world_z,
                    leaf_block(instance.params.kind),
                    1,
                );
            }
        }
    }
}

fn rasterize_segment(
    placements: &mut HashMap<(i32, i32, i32), Placement>,
    chunk_bounds: &ChunkBounds,
    start: DVec3,
    end: DVec3,
    radius: f64,
    block: BlockState,
    priority: u8,
) {
    let min_x = start.x.min(end.x).floor() as i32 - radius.ceil() as i32 - 1;
    let max_x = start.x.max(end.x).ceil() as i32 + radius.ceil() as i32 + 1;
    let min_y = start.y.min(end.y).floor() as i32 - radius.ceil() as i32 - 1;
    let max_y = start.y.max(end.y).ceil() as i32 + radius.ceil() as i32 + 1;
    let min_z = start.z.min(end.z).floor() as i32 - radius.ceil() as i32 - 1;
    let max_z = start.z.max(end.z).ceil() as i32 + radius.ceil() as i32 + 1;

    for world_z in min_z..=max_z {
        for world_x in min_x..=max_x {
            if !chunk_bounds.contains_with_margin(world_x, world_z, radius.ceil() as i32 + 1) {
                continue;
            }
            for world_y in min_y..=max_y {
                let point = DVec3::new(
                    world_x as f64 + 0.5,
                    world_y as f64 + 0.5,
                    world_z as f64 + 0.5,
                );
                if distance_to_segment_sq(point, start, end) > (radius + 0.45) * (radius + 0.45) {
                    continue;
                }
                upsert_block(
                    placements,
                    chunk_bounds,
                    world_x,
                    world_y,
                    world_z,
                    block,
                    priority,
                );
            }
        }
    }
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

fn segment_radius(instance: &MegaTreeInstance, node: &TreeNode) -> f64 {
    let height_ratio = ((node.pos.y - instance.base.y as f64)
        / instance.params.trunk_height as f64)
        .clamp(0.0, 1.35);
    let trunk_radius =
        instance.params.trunk_base_radius * instance.params.trunk_taper.powf(height_ratio * 4.0);
    if node.branch_depth == 0 {
        trunk_radius.max(instance.params.min_branch_radius)
    } else {
        (trunk_radius
            * instance
                .params
                .branch_radius_ratio
                .powi(node.branch_depth as i32))
        .max(instance.params.min_branch_radius)
    }
}

fn surface_slope(
    world_x: i32,
    world_z: i32,
    min_y: i32,
    world_height: i32,
    terrain: &TerrainProvider,
) -> i32 {
    let offsets = [(0, 0), (5, 0), (-5, 0), (0, 5), (0, -5)];
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

fn can_replace(existing: BlockState, incoming: BlockState) -> bool {
    if is_leaf_block(incoming) {
        return existing.is_air()
            || matches!(
                existing,
                BlockState::WATER
                    | BlockState::GRASS
                    | BlockState::FERN
                    | BlockState::DANDELION
                    | BlockState::POPPY
                    | BlockState::LILY_PAD
            );
    }

    existing.is_air()
        || matches!(
            existing,
            BlockState::WATER
                | BlockState::GRASS
                | BlockState::FERN
                | BlockState::DANDELION
                | BlockState::POPPY
                | BlockState::LILY_PAD
                | BlockState::SEAGRASS
                | BlockState::KELP
                | BlockState::KELP_PLANT
                | BlockState::DEAD_BUSH
                | BlockState::MOSS_CARPET
        )
}

fn log_block(kind: MegaTreeKind) -> BlockState {
    match kind {
        MegaTreeKind::SpiritWood | MegaTreeKind::SwampCypress => BlockState::OAK_LOG,
        MegaTreeKind::AncientPine => BlockState::SPRUCE_LOG,
        MegaTreeKind::Deadwood => BlockState::OAK_LOG,
    }
}

fn leaf_block(kind: MegaTreeKind) -> BlockState {
    match kind {
        MegaTreeKind::SpiritWood => BlockState::OAK_LEAVES,
        MegaTreeKind::AncientPine => BlockState::SPRUCE_LEAVES,
        MegaTreeKind::Deadwood => BlockState::OAK_LEAVES,
        MegaTreeKind::SwampCypress => BlockState::MANGROVE_LEAVES,
    }
}

fn is_leaf_block(block: BlockState) -> bool {
    matches!(
        block,
        BlockState::OAK_LEAVES | BlockState::SPRUCE_LEAVES | BlockState::MANGROVE_LEAVES
    )
}

#[derive(Clone)]
struct CachedSkeletonEntry {
    seed: u64,
    nodes: Vec<TreeNode>,
}

fn cached_skeleton(instance: &MegaTreeInstance) -> Vec<TreeNode> {
    static CACHE: OnceLock<Mutex<Vec<CachedSkeletonEntry>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(Vec::with_capacity(TREE_CACHE_CAPACITY)));

    if let Ok(mut guard) = cache.lock() {
        if let Some(index) = guard.iter().position(|entry| entry.seed == instance.seed) {
            let entry = guard.remove(index);
            let nodes = entry.nodes.clone();
            guard.push(entry);
            return nodes;
        }
    }

    let nodes = generate_skeleton(instance);

    if let Ok(mut guard) = cache.lock() {
        if guard.len() >= TREE_CACHE_CAPACITY {
            guard.remove(0);
        }
        guard.push(CachedSkeletonEntry {
            seed: instance.seed,
            nodes: nodes.clone(),
        });
    }

    nodes
}

fn distance_to_segment_sq(point: DVec3, start: DVec3, end: DVec3) -> f64 {
    let segment = end - start;
    let length_sq = segment.length_squared();
    if length_sq <= f64::EPSILON {
        return (point - start).length_squared();
    }
    let t = ((point - start).dot(segment) / length_sq).clamp(0.0, 1.0);
    let closest = start + segment * t;
    (point - closest).length_squared()
}

fn normalize(vec: DVec3) -> DVec3 {
    let length_sq = vec.length_squared();
    if length_sq <= f64::EPSILON {
        DVec3::ZERO
    } else {
        vec / length_sq.sqrt()
    }
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

fn range_f64(seed: u64, min: f64, max: f64) -> f64 {
    if max <= min {
        return min;
    }
    min + (max - min) * sample_u01(seed)
}

struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn new(seed: u64) -> Self {
        Self {
            state: seed ^ 0xA076_1D64_78BD_642F,
        }
    }

    fn next_u64(&mut self) -> u64 {
        self.state ^= self.state >> 12;
        self.state ^= self.state << 25;
        self.state ^= self.state >> 27;
        self.state = self.state.wrapping_mul(0x2545_F491_4F6C_DD1D);
        self.state
    }

    fn range_f64(&mut self, min: f64, max: f64) -> f64 {
        range_f64(self.next_u64(), min, max)
    }
}
