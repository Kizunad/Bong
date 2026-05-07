//! # NPC Navigator — block-level pathfinding & movement
//!
//! Architecture inspired by Pumpkin-MC/Pumpkin's `Navigator` struct
//! (itself a Rust reimplementation of vanilla Minecraft's `EntityNavigation`).
//!
//! ## Three-layer separation (vanilla MC / Pumpkin pattern)
//!
//! ```text
//! ┌─────────────────────────────────────────────┐
//! │  Goals (big_brain scorers/actions)           │
//! │  brain.rs: "chase player" / "flee" / "idle"  │
//! │  → only sets navigator.set_goal(target, spd) │
//! └──────────────────┬──────────────────────────┘
//!                    ▼
//! ┌─────────────────────────────────────────────┐
//! │  Navigator (this module)                     │
//! │  - A* pathfinding at block resolution        │
//! │  - Path following with stuck detection       │
//! │  - Block-level collision (ChunkLayer query)  │
//! │  - Writes Position + Look + HeadYaw          │
//! └──────────────────┬──────────────────────────┘
//!                    ▼
//! ┌─────────────────────────────────────────────┐
//! │  Sync (sync.rs)                              │
//! │  - Position → Transform one-way sync         │
//! └─────────────────────────────────────────────┘
//! ```
//!
//! ## Key differences from Pumpkin
//!
//! - Uses `SurfaceProvider` + `ChunkLayer` instead of Pumpkin's `World` access
//! - A* via the `pathfinding` crate, not a hand-rolled open set
//! - Integrated with big_brain's Scorer/Action model (not Pumpkin's Goal trait)
//! - Zone bounds clamping from our zone system

use std::collections::HashMap;

use bevy_transform::components::Transform;
use pathfinding::prelude::astar;
use valence::entity::{HeadYaw, Look};
use valence::prelude::{
    bevy_ecs, App, BlockState, Chunk, ChunkLayer, ChunkPos, Component, DVec3, Position, Query, Res,
    Update, With,
};

use crate::npc::movement::MovementController;
use crate::npc::spawn::NpcMarker;
use crate::world::terrain::{TerrainProvider, TerrainProviders};

// ---------------------------------------------------------------------------
// Constants — tuned to feel like vanilla MC zombie movement
// ---------------------------------------------------------------------------

/// Step distance per tick. Multiplied by the goal's `speed` factor.
const BASE_STEP_DISTANCE: f64 = 0.2;

/// When within this XZ distance of a waypoint, advance to the next node.
/// Pumpkin uses separate XZ and Y thresholds; we merge for simplicity.
const NODE_REACH_XZ: f64 = 0.65;

/// A* goal tolerance in blocks (XZ only). The NPC is a ground mob — it
/// considers the destination "reached" when it's within this many blocks
/// horizontally, regardless of Y difference (player may be jumping/flying).
const GOAL_REACH_XZ: i32 = 2;

/// Max Y-step the navigator considers walkable between adjacent blocks.
/// Vanilla MC uses 1.0 (one block); we allow 1 for more natural movement.
const MAX_STEP_HEIGHT: i32 = 1;

/// A* iteration budget. Pumpkin uses 560; we use 400 to handle routing
/// around trees and decorations placed after terrain gen.
const MAX_PATH_ITERS: usize = 400;

/// If the NPC hasn't moved at least this far in `STUCK_CHECK_INTERVAL` ticks,
/// clear the path and recompute.
const STUCK_DISTANCE_THRESHOLD: f64 = 1.5;

/// How often (in ticks) to run the stuck check.
const STUCK_CHECK_INTERVAL: u32 = 60;

/// How often to recompute the path if the target moved.
const REPATH_INTERVAL_TICKS: u32 = 20;

/// Fallback surface Y when no terrain is loaded.
const FALLBACK_SURFACE_Y: f64 = 66.0;

/// How far down from the NPC's current Y to scan for ground in ChunkLayer.
const GROUND_SCAN_DEPTH: i32 = 16;

/// How far up from the NPC's current Y to scan for ground (for uphill steps).
const GROUND_SCAN_UP: i32 = 4;

// ---------------------------------------------------------------------------
// PathType — block classification for pathfinding penalties
// ---------------------------------------------------------------------------

/// Block classification for pathfinding cost, inspired by vanilla MC's
/// `PathNodeType` / Pumpkin's `PathType`.
///
/// A penalty of `f32::NEG_INFINITY` (or any negative value) means impassable.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[allow(dead_code)]
pub enum PathType {
    Walkable,
    Open,    // air above ground — free to traverse
    Blocked, // solid block at feet/head level
    Water,
    Lava,
    DangerFire,
    Fence, // fence/wall — blocks movement
}

impl PathType {
    /// Default pathfinding penalty. Negative = impassable.
    pub fn default_penalty(self) -> f32 {
        match self {
            Self::Walkable | Self::Open => 0.0,
            Self::Blocked => -1.0,
            Self::Water => 8.0,
            Self::Lava => -1.0,
            Self::DangerFire => 16.0,
            Self::Fence => -1.0,
        }
    }
}

// ---------------------------------------------------------------------------
// NavigatorGoal — what the Navigator is trying to reach
// ---------------------------------------------------------------------------

/// A navigation request set by a brain action (chase, flee, patrol, etc.).
#[derive(Clone, Copy, Debug)]
pub struct NavigatorGoal {
    /// World-space destination.
    pub destination: DVec3,
    /// Speed multiplier (1.0 = base speed, 1.5 = fast, 0.6 = slow).
    pub speed: f64,
}

// ---------------------------------------------------------------------------
// Navigator component
// ---------------------------------------------------------------------------

/// Per-NPC navigation state. Attached as an ECS Component.
///
/// Brain actions call [`Navigator::set_goal`] to request movement;
/// the `navigator_system` does the rest (pathfinding, stepping, yaw).
#[derive(Clone, Debug, Component)]
pub struct Navigator {
    /// Current navigation target. `None` = idle.
    current_goal: Option<NavigatorGoal>,

    /// Computed A* path (block-level positions, Y = feet).
    path: Vec<DVec3>,
    /// Index into `path` for the next waypoint.
    path_index: usize,

    /// Ticks since last repath.
    repath_countdown: u32,
    /// Destination at the time the current path was computed.
    last_pathed_destination: Option<DVec3>,

    // -- Stuck detection (Pumpkin: ticks_on_current_node / total_ticks) --
    /// Position when we started measuring stuck-ness.
    stuck_check_pos: DVec3,
    /// Tick counter for periodic stuck checks.
    stuck_check_ticks: u32,

    /// Per-mob penalty overrides. E.g. an aquatic mob could set Water → 0.
    path_type_overrides: HashMap<PathType, f32>,
}

impl Default for Navigator {
    fn default() -> Self {
        Self {
            current_goal: None,
            path: Vec::new(),
            path_index: 0,
            repath_countdown: 0,
            last_pathed_destination: None,
            stuck_check_pos: DVec3::ZERO,
            stuck_check_ticks: 0,
            path_type_overrides: HashMap::new(),
        }
    }
}

impl Navigator {
    /// Create a new idle navigator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a new navigation goal. The navigator will compute a path on the
    /// next tick. This is the **only** method brain actions should call.
    ///
    /// Mirrors Pumpkin's `Navigator::set_progress()`.
    pub fn set_goal(&mut self, destination: DVec3, speed: f64) {
        self.current_goal = Some(NavigatorGoal { destination, speed });
        // Force repath on next tick.
        self.repath_countdown = 0;
    }

    /// Stop navigating and clear the current path.
    pub fn stop(&mut self) {
        self.current_goal = None;
        self.path.clear();
        self.path_index = 0;
    }

    /// Whether the navigator is currently idle (no goal).
    pub fn is_idle(&self) -> bool {
        self.current_goal.is_none()
    }

    /// Override the pathfinding penalty for a given [`PathType`].
    /// Use negative values to make a type impassable.
    #[allow(dead_code)]
    pub fn set_pathfinding_penalty(&mut self, path_type: PathType, penalty: f32) {
        self.path_type_overrides.insert(path_type, penalty);
    }

    fn penalty(&self, pt: PathType) -> f32 {
        self.path_type_overrides
            .get(&pt)
            .copied()
            .unwrap_or_else(|| pt.default_penalty())
    }
}

// ---------------------------------------------------------------------------
// ECS system
// ---------------------------------------------------------------------------

pub fn register(app: &mut App) {
    tracing::info!("[bong][npc] registering navigator system");
    app.add_systems(Update, navigator_tick_system);
}

/// The core navigator system. Runs every tick for each NPC with a Navigator.
///
/// Mirrors Pumpkin's `Navigator::tick()`:
/// 1. If MovementController has an Override active → yield (don't write Position).
/// 2. If no goal → stop.
/// 3. If path is stale or missing → A* recompute.
/// 4. Advance along path, check stuck.
/// 5. Set Position + Look + HeadYaw.
#[allow(clippy::type_complexity)]
pub fn navigator_tick_system(
    mut npcs: Query<
        (
            &mut Position,
            &mut Transform,
            &mut Look,
            &mut HeadYaw,
            &mut Navigator,
            Option<&MovementController>,
        ),
        With<NpcMarker>,
    >,
    providers: Option<Res<TerrainProviders>>,
    layers: Query<&ChunkLayer, With<crate::world::dimension::OverworldLayer>>,
) {
    let layer = layers.get_single().ok();
    let terrain = providers.as_deref().map(|p| &p.overworld);

    for (mut position, mut transform, mut look, mut head_yaw, mut nav, movement_ctrl) in &mut npcs {
        // If an Override ability (Dash, Leap, etc.) is active, it owns Position
        // this tick. Navigator must not interfere.
        let movement_ctrl = movement_ctrl.cloned().unwrap_or_default();
        if movement_ctrl.navigator_should_yield() {
            continue;
        }

        let Some(goal) = nav.current_goal else {
            if let Some(layer_ref) = layer {
                let current = position.get();
                let snapped = snap_to_ground(current, Some(layer_ref));
                if (snapped.y - current.y).abs() > 1e-4 {
                    position.set(snapped);
                    transform.translation.y = snapped.y as f32;
                }
            }
            continue;
        };

        let current_pos = position.get();

        // -- Repath if needed --
        let destination_moved = nav
            .last_pathed_destination
            .map(|d| d.distance_squared(goal.destination) > 4.0)
            .unwrap_or(true);

        if nav.repath_countdown == 0 || destination_moved || nav.path_index >= nav.path.len() {
            let new_path = compute_path(current_pos, goal.destination, &nav, terrain, layer);
            nav.path = new_path;
            nav.path_index = 0;
            nav.repath_countdown = REPATH_INTERVAL_TICKS;
            nav.last_pathed_destination = Some(goal.destination);
            nav.stuck_check_pos = current_pos;
            nav.stuck_check_ticks = 0;
        } else {
            nav.repath_countdown = nav.repath_countdown.saturating_sub(1);
        }

        // -- Stuck detection (Pumpkin: every STUCK_CHECK_INTERVAL ticks) --
        nav.stuck_check_ticks += 1;
        if nav.stuck_check_ticks >= STUCK_CHECK_INTERVAL {
            let moved = current_pos.distance(nav.stuck_check_pos);
            if moved < STUCK_DISTANCE_THRESHOLD {
                // Stuck — clear path so we recompute next tick.
                nav.path.clear();
                nav.path_index = 0;
                nav.repath_countdown = 0;
            }
            nav.stuck_check_pos = current_pos;
            nav.stuck_check_ticks = 0;
        }

        // -- Advance along path --
        // Skip nodes we're already close to.
        while let Some(waypoint) = nav.path.get(nav.path_index).copied() {
            let dx = waypoint.x - current_pos.x;
            let dz = waypoint.z - current_pos.z;
            if dx * dx + dz * dz > NODE_REACH_XZ * NODE_REACH_XZ {
                break;
            }
            nav.path_index += 1;
        }

        let target_pos = if let Some(waypoint) = nav.path.get(nav.path_index).copied() {
            waypoint
        } else {
            // Path exhausted or A* failed — don't blindly walk into obstacles.
            // Force repath on the next tick and stay put for now.
            nav.repath_countdown = 0;
            continue;
        };

        // -- Step toward target --
        // Apply ability speed scale (Sprint multiplier, or 1.0 for normal).
        let step = goal.speed * movement_ctrl.speed_scale() * BASE_STEP_DISTANCE;
        let next_pos = step_toward_with_collision(current_pos, target_pos, step, layer);

        // -- Update facing (Pumpkin: Navigator sets yaw/head_yaw) --
        let dx = next_pos.x - current_pos.x;
        let dz = next_pos.z - current_pos.z;
        if dx * dx + dz * dz > 1e-8 {
            let yaw = (dz.atan2(dx).to_degrees() - 90.0) as f32;
            look.yaw = yaw;
            look.pitch = 0.0;
            head_yaw.0 = yaw;
        }

        // -- Write Position + Transform --
        position.set(next_pos);
        transform.translation.x = next_pos.x as f32;
        transform.translation.y = next_pos.y as f32;
        transform.translation.z = next_pos.z as f32;
    }
}

// ---------------------------------------------------------------------------
// A* pathfinding at block resolution
// ---------------------------------------------------------------------------

/// A* node: block-level world coordinate.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct PathNode {
    x: i32,
    y: i32,
    z: i32,
}

impl PathNode {
    fn to_dvec3(self) -> DVec3 {
        DVec3::new(
            f64::from(self.x) + 0.5,
            f64::from(self.y),
            f64::from(self.z) + 0.5,
        )
    }

    /// Manhattan distance on XZ only — the NPC is a ground mob, so Y
    /// differences shouldn't inflate the heuristic (they're resolved by
    /// terrain scanning, not vertical movement).
    fn heuristic_xz(self, target: Self) -> u32 {
        let dx = self.x.abs_diff(target.x);
        let dz = self.z.abs_diff(target.z);
        dx + dz
    }
}

fn compute_path(
    start: DVec3,
    destination: DVec3,
    nav: &Navigator,
    terrain: Option<&TerrainProvider>,
    layer: Option<&ChunkLayer>,
) -> Vec<DVec3> {
    let start_node = PathNode {
        x: start.x.floor() as i32,
        y: start.y.floor() as i32,
        z: start.z.floor() as i32,
    };
    // Target node uses ground-level Y, not the player's airborne Y.
    // This way A* can always reach the goal even when the player is jumping.
    let target_x = destination.x.floor() as i32;
    let target_z = destination.z.floor() as i32;
    let target_ground_y = resolve_ground_y_from_chunk(target_x, target_z, start_node.y, layer)
        .map(|gy| gy + 1)
        .unwrap_or_else(|| resolve_surface_y(target_x, target_z, terrain) + 1);

    let target_node = PathNode {
        x: target_x,
        y: target_ground_y,
        z: target_z,
    };

    if start_node.x.abs_diff(target_node.x) <= GOAL_REACH_XZ as u32
        && start_node.z.abs_diff(target_node.z) <= GOAL_REACH_XZ as u32
    {
        return Vec::new(); // already close enough
    }

    // A* with iteration cap.
    let mut iters = 0usize;

    let result = astar(
        &start_node,
        |node| {
            iters += 1;
            if iters > MAX_PATH_ITERS {
                return Vec::new();
            }
            block_successors(*node, start_node, nav, terrain, layer)
        },
        |node| node.heuristic_xz(target_node),
        // Goal reached when within GOAL_REACH_XZ blocks horizontally.
        |node| {
            node.x.abs_diff(target_node.x) <= GOAL_REACH_XZ as u32
                && node.z.abs_diff(target_node.z) <= GOAL_REACH_XZ as u32
        },
    );

    result
        .map(|(path, _cost)| path.into_iter().skip(1).map(|n| n.to_dvec3()).collect())
        .unwrap_or_default()
}

/// Generate walkable neighbors for a block-level A* node.
///
/// Checks 4 cardinal directions (no diagonals — matches vanilla MC ground nav).
/// For each neighbor:
/// 1. Resolve surface Y from ChunkLayer (scanning down for real ground including trees).
/// 2. Fall back to TerrainProvider heightmap if chunk not loaded.
/// 3. Check cliff height vs `MAX_STEP_HEIGHT`.
/// 4. Check block solidity at feet + head via ChunkLayer.
/// 5. Apply PathType penalties.
fn block_successors(
    node: PathNode,
    start_node: PathNode,
    nav: &Navigator,
    terrain: Option<&TerrainProvider>,
    layer: Option<&ChunkLayer>,
) -> Vec<(PathNode, u32)> {
    let mut result = Vec::with_capacity(4);

    for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
        let nx = node.x + dx;
        let nz = node.z + dz;

        // Resolve surface Y at the neighbor — prefer ChunkLayer (sees trees/decorations),
        // fall back to TerrainProvider heightmap (raw terrain only).
        let feet_y = if let Some(gy) = resolve_ground_y_from_chunk(nx, nz, node.y, layer) {
            gy + 1 // feet stand on top of the ground block
        } else {
            resolve_surface_y(nx, nz, terrain) + 1
        };

        // Cliff check.
        let dy = (feet_y - node.y).abs();
        if dy > MAX_STEP_HEIGHT {
            continue;
        }

        // Classify the block at feet and head level.
        let feet_type = classify_block(nx, feet_y, nz, layer);
        let head_type = classify_block(nx, feet_y + 1, nz, layer);

        let feet_penalty = nav.penalty(feet_type);
        let head_penalty = nav.penalty(head_type);

        // Impassable if either penalty is negative.
        if feet_penalty < 0.0 || head_penalty < 0.0 {
            // Allow stepping onto the start node (NPC may be standing there).
            let neighbor = PathNode {
                x: nx,
                y: feet_y,
                z: nz,
            };
            if neighbor != start_node {
                continue;
            }
        }

        let cost = 1 + dy as u32 + (feet_penalty.max(0.0) + head_penalty.max(0.0)) as u32;
        result.push((
            PathNode {
                x: nx,
                y: feet_y,
                z: nz,
            },
            cost,
        ));
    }

    result
}

fn resolve_surface_y(wx: i32, wz: i32, terrain: Option<&TerrainProvider>) -> i32 {
    use crate::world::terrain::SurfaceProvider;
    terrain
        .map(|t| t.query_surface(wx, wz).y)
        .unwrap_or(FALLBACK_SURFACE_Y as i32)
}

/// Scan the actual ChunkLayer downward (and slightly upward) from `ref_y` to
/// find the topmost solid block at `(wx, wz)`.
///
/// Unlike `resolve_surface_y` (which reads the heightmap), this sees **all**
/// blocks including trees, structures, and decorations placed after terrain gen.
///
/// Returns `None` if the chunk is not loaded (caller should fall back to
/// TerrainProvider).
fn resolve_ground_y_from_chunk(
    wx: i32,
    wz: i32,
    ref_y: i32,
    layer: Option<&ChunkLayer>,
) -> Option<i32> {
    let layer = layer?;
    let min_y = layer.min_y();
    let max_y = min_y + layer.height() as i32 - 1;

    let chunk_pos = ChunkPos::new(wx.div_euclid(16), wz.div_euclid(16));
    let chunk = layer.chunk(chunk_pos)?;

    let lx = wx.rem_euclid(16) as u32;
    let lz = wz.rem_euclid(16) as u32;

    // Scan upward first (for climbing steps/slopes).
    let scan_top = (ref_y + GROUND_SCAN_UP).min(max_y);
    // Scan down to find the ground.
    let scan_bottom = (ref_y - GROUND_SCAN_DEPTH).max(min_y);

    // From scan_top downward, find first solid block with air/passable above.
    for y in (scan_bottom..=scan_top).rev() {
        let local_y = (y - min_y) as u32;
        let block = chunk.block_state(lx, local_y, lz);
        if is_solid_for_ground(block) {
            // Verify there's room to stand (feet + head above must be passable).
            let above1 = if y < max_y {
                chunk.block_state(lx, (y + 1 - min_y) as u32, lz)
            } else {
                BlockState::AIR
            };
            let above2 = if y + 1 < max_y {
                chunk.block_state(lx, (y + 2 - min_y) as u32, lz)
            } else {
                BlockState::AIR
            };
            if !is_solid_for_ground(above1) && !is_solid_for_ground(above2) {
                return Some(y);
            }
        }
    }

    None
}

/// Whether a block counts as "solid ground" that an NPC can stand on.
/// Excludes passthrough blocks (grass, flowers), fluids, and leaves
/// (so NPCs don't try to walk on tree canopies).
fn is_solid_for_ground(block: BlockState) -> bool {
    if block == BlockState::AIR || block == BlockState::CAVE_AIR {
        return false;
    }
    if block == BlockState::WATER || block == BlockState::LAVA {
        return false;
    }
    if is_passthrough_block(block) {
        return false;
    }
    if is_leaf_block(block) {
        return false;
    }
    true
}

// ---------------------------------------------------------------------------
// Block classification
// ---------------------------------------------------------------------------

/// Classify a world block as a [`PathType`] for pathfinding penalties.
///
/// Queries the loaded ChunkLayer. If the chunk is not loaded, assumes
/// `Walkable` (optimistic — the NPC shouldn't be pathfinding into unloaded
/// chunks in practice).
fn classify_block(wx: i32, wy: i32, wz: i32, layer: Option<&ChunkLayer>) -> PathType {
    let Some(layer) = layer else {
        return PathType::Open;
    };

    let min_y = layer.min_y();
    let local_y = wy - min_y;
    if local_y < 0 || local_y >= layer.height() as i32 {
        return PathType::Open;
    }

    let chunk_pos = ChunkPos::new(wx.div_euclid(16), wz.div_euclid(16));
    let Some(chunk) = layer.chunk(chunk_pos) else {
        return PathType::Open;
    };

    let lx = wx.rem_euclid(16) as u32;
    let ly = local_y as u32;
    let lz = wz.rem_euclid(16) as u32;
    let block = chunk.block_state(lx, ly, lz);

    classify_block_state(block)
}

/// Map a BlockState to a PathType.
fn classify_block_state(block: BlockState) -> PathType {
    if block == BlockState::AIR || block == BlockState::CAVE_AIR {
        return PathType::Open;
    }
    if block == BlockState::WATER {
        return PathType::Water;
    }
    if block == BlockState::LAVA || block == BlockState::MAGMA_BLOCK {
        return PathType::Lava;
    }
    if block == BlockState::FIRE || block == BlockState::SOUL_FIRE {
        return PathType::DangerFire;
    }
    if is_passthrough_block(block) {
        return PathType::Open;
    }
    // Fences, walls, etc.
    if is_fence_like(block) {
        return PathType::Fence;
    }

    PathType::Blocked
}

/// Blocks NPCs can walk through (vegetation, flowers, etc.).
fn is_passthrough_block(block: BlockState) -> bool {
    block == BlockState::GRASS
        || block == BlockState::TALL_GRASS
        || block == BlockState::FERN
        || block == BlockState::LARGE_FERN
        || block == BlockState::POPPY
        || block == BlockState::DANDELION
        || block == BlockState::DEAD_BUSH
        || block == BlockState::LILY_PAD
        || block == BlockState::SNOW
        || block == BlockState::VINE
        || block == BlockState::TORCH
        || block == BlockState::WALL_TORCH
        || block == BlockState::RAIL
        || block == BlockState::REDSTONE_WIRE
}

fn is_fence_like(block: BlockState) -> bool {
    block == BlockState::OAK_FENCE
        || block == BlockState::SPRUCE_FENCE
        || block == BlockState::BIRCH_FENCE
        || block == BlockState::COBBLESTONE_WALL
}

/// Leaf blocks — solid for NPC collision (NPCs cannot walk through tree canopies).
/// Listed explicitly because leaves fall through to `Blocked` anyway, but this
/// makes the intent clear and helps `is_solid_for_ground` treat them correctly
/// (leaves can be ground only if nothing else is below — e.g. on top of a tree).
fn is_leaf_block(block: BlockState) -> bool {
    block == BlockState::OAK_LEAVES
        || block == BlockState::SPRUCE_LEAVES
        || block == BlockState::BIRCH_LEAVES
        || block == BlockState::JUNGLE_LEAVES
        || block == BlockState::ACACIA_LEAVES
        || block == BlockState::DARK_OAK_LEAVES
        || block == BlockState::AZALEA_LEAVES
        || block == BlockState::FLOWERING_AZALEA_LEAVES
        || block == BlockState::CHERRY_LEAVES
        || block == BlockState::MANGROVE_LEAVES
}

// ---------------------------------------------------------------------------
// Movement with block-level collision
// ---------------------------------------------------------------------------

/// Step from `current` toward `target` by at most `max_step`, checking block
/// solidity at the destination and resolving the correct ground Y.
///
/// If the direct step is blocked, tries ±45° and ±90° offsets.
/// Returns `current` if all directions are blocked.
fn step_toward_with_collision(
    current: DVec3,
    target: DVec3,
    max_step: f64,
    layer: Option<&ChunkLayer>,
) -> DVec3 {
    let delta = DVec3::new(target.x - current.x, 0.0, target.z - current.z);
    let dist = delta.length();

    if dist <= f64::EPSILON {
        // Even if not moving in XZ, snap to ground (gravity).
        return snap_to_ground(current, layer);
    }

    let dir = delta / dist;
    let step = dist.min(max_step);

    let candidates = [
        dir,
        rotate_y(dir, std::f64::consts::FRAC_PI_4),
        rotate_y(dir, -std::f64::consts::FRAC_PI_4),
        rotate_y(dir, std::f64::consts::FRAC_PI_2),
        rotate_y(dir, -std::f64::consts::FRAC_PI_2),
    ];

    let ref_y = current.y.floor() as i32;

    for candidate in &candidates {
        let tentative = current + *candidate * step;
        let wx = tentative.x.floor() as i32;
        let wz = tentative.z.floor() as i32;

        // Resolve real ground Y at the target XZ (sees trees, decorations).
        let ground_y = resolve_ground_y_from_chunk(wx, wz, ref_y, layer).unwrap_or(ref_y - 1);
        let feet_y = ground_y + 1;

        // Cliff check — don't jump too high or fall too far per step.
        let dy = (feet_y - ref_y).abs();
        if dy > MAX_STEP_HEIGHT {
            continue;
        }

        if let Some(chunk_layer) = layer {
            // Check feet and head aren't solid.
            let feet_type = classify_block(wx, feet_y, wz, Some(chunk_layer));
            let head_type = classify_block(wx, feet_y + 1, wz, Some(chunk_layer));

            if feet_type == PathType::Blocked
                || feet_type == PathType::Fence
                || head_type == PathType::Blocked
                || head_type == PathType::Fence
            {
                continue;
            }
        }

        // Move in XZ and set Y to the resolved ground level (feet on ground).
        return DVec3::new(tentative.x, f64::from(feet_y), tentative.z);
    }

    // All blocked — stay put but still apply gravity.
    snap_to_ground(current, layer)
}

/// Snap a position to the ground at its current XZ.
/// Applies "gravity" — the NPC always stands on the topmost solid block.
fn snap_to_ground(pos: DVec3, layer: Option<&ChunkLayer>) -> DVec3 {
    let wx = pos.x.floor() as i32;
    let wz = pos.z.floor() as i32;
    let ref_y = pos.y.floor() as i32;

    if let Some(ground_y) = resolve_ground_y_from_chunk(wx, wz, ref_y, layer) {
        DVec3::new(pos.x, f64::from(ground_y + 1), pos.z)
    } else {
        pos
    }
}

fn rotate_y(dir: DVec3, angle: f64) -> DVec3 {
    let (sin, cos) = angle.sin_cos();
    DVec3::new(dir.x * cos - dir.z * sin, 0.0, dir.x * sin + dir.z * cos)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::Entity;

    use crate::world::terrain::{SurfaceInfo, SurfaceProvider};

    #[allow(dead_code)]
    struct FlatSurface(i32);
    impl SurfaceProvider for FlatSurface {
        fn query_surface(&self, _x: i32, _z: i32) -> SurfaceInfo {
            SurfaceInfo {
                y: self.0,
                passable: true,
            }
        }
    }

    #[test]
    fn navigator_default_is_idle() {
        let nav = Navigator::new();
        assert!(nav.is_idle());
    }

    #[test]
    fn set_goal_makes_navigator_non_idle() {
        let mut nav = Navigator::new();
        nav.set_goal(DVec3::new(10.0, 67.0, 10.0), 1.0);
        assert!(!nav.is_idle());
    }

    #[test]
    fn stop_clears_goal_and_path() {
        let mut nav = Navigator::new();
        nav.set_goal(DVec3::new(10.0, 67.0, 10.0), 1.0);
        nav.path = vec![DVec3::new(5.0, 67.0, 5.0)];
        nav.stop();
        assert!(nav.is_idle());
        assert!(nav.path.is_empty());
    }

    #[test]
    fn path_type_default_penalties() {
        assert_eq!(PathType::Walkable.default_penalty(), 0.0);
        assert!(PathType::Blocked.default_penalty() < 0.0);
        assert!(PathType::Lava.default_penalty() < 0.0);
        assert!(PathType::Water.default_penalty() > 0.0);
        assert!(PathType::DangerFire.default_penalty() > 0.0);
    }

    #[test]
    fn penalty_override_works() {
        let mut nav = Navigator::new();
        assert_eq!(nav.penalty(PathType::Water), 8.0);
        nav.set_pathfinding_penalty(PathType::Water, -1.0);
        assert_eq!(nav.penalty(PathType::Water), -1.0);
    }

    #[test]
    fn compute_path_finds_straight_line_without_terrain() {
        let nav = Navigator::new();
        let start = DVec3::new(0.5, 67.0, 0.5);
        let end = DVec3::new(5.5, 67.0, 0.5);
        let path = compute_path(start, end, &nav, None, None);
        assert!(!path.is_empty(), "should find path on open terrain");
    }

    #[test]
    fn classify_air_is_open() {
        assert_eq!(classify_block_state(BlockState::AIR), PathType::Open);
        assert_eq!(classify_block_state(BlockState::CAVE_AIR), PathType::Open);
    }

    #[test]
    fn classify_water_and_lava() {
        assert_eq!(classify_block_state(BlockState::WATER), PathType::Water);
        assert_eq!(classify_block_state(BlockState::LAVA), PathType::Lava);
    }

    #[test]
    fn classify_solid_is_blocked() {
        assert_eq!(classify_block_state(BlockState::STONE), PathType::Blocked);
        assert_eq!(classify_block_state(BlockState::OAK_LOG), PathType::Blocked);
        assert_eq!(
            classify_block_state(BlockState::OAK_PLANKS),
            PathType::Blocked
        );
    }

    #[test]
    fn classify_vegetation_is_open() {
        assert_eq!(classify_block_state(BlockState::GRASS), PathType::Open);
        assert_eq!(classify_block_state(BlockState::FERN), PathType::Open);
        assert_eq!(classify_block_state(BlockState::POPPY), PathType::Open);
    }

    #[test]
    fn step_toward_collision_stays_put_when_no_layer() {
        let current = DVec3::new(5.0, 67.0, 5.0);
        let target = DVec3::new(10.0, 67.0, 5.0);
        let result = step_toward_with_collision(current, target, 0.2, None);
        // Without a layer, no collision checks → should move.
        assert!(result.x > current.x);
    }

    #[test]
    fn path_node_heuristic_is_xz_manhattan() {
        let a = PathNode { x: 0, y: 0, z: 0 };
        let b = PathNode { x: 3, y: 1, z: 4 };
        // Y is ignored — ground mob heuristic.
        assert_eq!(a.heuristic_xz(b), 3 + 4);
    }

    // -- Bug #1: idle NPC gravity regression tests -------------------------

    fn make_navigator_app_with_ground(ground_y: i32) -> (App, Entity) {
        use valence::prelude::{BlockState, Chunk, UnloadedChunk};
        use valence::testing::ScenarioSingleClient;

        let scenario = ScenarioSingleClient::new();
        let mut app = scenario.app;
        crate::world::dimension::mark_test_layer_as_overworld(&mut app);
        let layer_entity = {
            let world = app.world_mut();
            let mut q = world.query_filtered::<Entity, With<ChunkLayer>>();
            q.iter(world).next().unwrap()
        };
        {
            let mut layer = app
                .world_mut()
                .get_mut::<ChunkLayer>(layer_entity)
                .unwrap();
            let mut chunk = UnloadedChunk::with_height(384);
            let min_y = layer.min_y();
            let local_y = (ground_y - min_y) as u32;
            for lx in 0..16u32 {
                for lz in 0..16u32 {
                    chunk.set_block_state(lx, local_y, lz, BlockState::STONE);
                }
            }
            layer.insert_chunk([0, 0], chunk);
        }
        app.add_systems(Update, navigator_tick_system);
        (app, layer_entity)
    }

    fn spawn_idle_npc(app: &mut App, y: f64) -> Entity {
        app.world_mut()
            .spawn((
                NpcMarker,
                Position::new([0.5, y, 0.5]),
                Transform::default(),
                Look::default(),
                HeadYaw::default(),
                Navigator::new(),
            ))
            .id()
    }

    #[test]
    fn idle_npc_in_air_falls_to_ground() {
        let (mut app, _) = make_navigator_app_with_ground(66);
        let npc = spawn_idle_npc(&mut app, 80.0);

        app.update();

        let pos = app.world().get::<Position>(npc).unwrap();
        assert!(
            (pos.get().y - 67.0).abs() < 0.01,
            "idle NPC at Y=80 should snap to ground_y+1=67, got Y={}",
            pos.get().y,
        );
    }

    #[test]
    fn idle_npc_already_on_ground_does_not_move() {
        let (mut app, _) = make_navigator_app_with_ground(66);
        let npc = spawn_idle_npc(&mut app, 67.0);

        app.update();

        let pos = app.world().get::<Position>(npc).unwrap();
        assert!(
            (pos.get().y - 67.0).abs() < 0.01,
            "idle NPC already at ground should stay at Y=67, got Y={}",
            pos.get().y,
        );
    }

    #[test]
    fn idle_npc_no_chunk_layer_does_not_panic() {
        let mut app = App::new();
        app.add_systems(Update, navigator_tick_system);
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([0.5, 80.0, 0.5]),
                Transform::default(),
                Look::default(),
                HeadYaw::default(),
                Navigator::new(),
            ))
            .id();

        app.update();

        let pos = app.world().get::<Position>(npc).unwrap();
        assert!(
            (pos.get().y - 80.0).abs() < 0.01,
            "idle NPC without chunk layer should stay at original Y=80, got Y={}",
            pos.get().y,
        );
    }

    #[test]
    fn idle_npc_transform_y_syncs_with_position() {
        let (mut app, _) = make_navigator_app_with_ground(66);
        let npc = spawn_idle_npc(&mut app, 80.0);

        app.update();

        let tf = app.world().get::<Transform>(npc).unwrap();
        assert!(
            (f64::from(tf.translation.y) - 67.0).abs() < 0.1,
            "idle gravity should also update Transform.translation.y, got {}",
            tf.translation.y,
        );
    }

    #[test]
    fn idle_to_active_transition_no_jitter() {
        let (mut app, _) = make_navigator_app_with_ground(66);
        let npc = spawn_idle_npc(&mut app, 80.0);

        app.update();
        let y_after_gravity = app.world().get::<Position>(npc).unwrap().get().y;
        assert!(
            (y_after_gravity - 67.0).abs() < 0.01,
            "first tick: idle gravity should snap to 67, got {}",
            y_after_gravity,
        );

        {
            let mut nav = app.world_mut().get_mut::<Navigator>(npc).unwrap();
            nav.set_goal(DVec3::new(5.0, 67.0, 0.5), 1.0);
        }
        app.update();

        let y_after_goal = app.world().get::<Position>(npc).unwrap().get().y;
        assert!(
            (y_after_goal - 67.0).abs() < 2.0,
            "second tick after set_goal: NPC should stay near ground, not jitter back to 80; got Y={}",
            y_after_goal,
        );
    }
}
