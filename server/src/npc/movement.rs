// Ability layer is scaffolded — Brain integration will use these APIs.
#![allow(dead_code)]
//! # NPC Movement Ability Layer
//!
//! Sits between Brain (big-brain) and Navigator:
//!
//! ```text
//! Brain  →  MovementController  →  Navigator (ground A*)  →  Sync
//!               ↓ if Override
//!           ability_tick_system (writes Position directly)
//! ```
//!
//! ## Ability taxonomy
//!
//! | Kind     | Position writer | Example        |
//! |----------|-----------------|----------------|
//! | Modifier | Navigator       | Sprint         |
//! | Override | ability system  | Dash, Leap     |
//! | Instant  | one-shot        | Blink          |
//!
//! ## Invariant
//!
//! **Exactly one system writes Position per tick per NPC.** The Navigator
//! checks `MovementController::mode` and yields when an Override is active.

use bevy_transform::components::Transform;
use valence::prelude::{
    bevy_ecs, App, BlockState, Chunk, ChunkLayer, ChunkPos, Commands, Component, DVec3, Entity,
    HeadYaw, IntoSystemConfigs, Look, Position, Query, Res, ResMut, Resource, Update, With,
    Without,
};

use crate::npc::spawn::NpcMarker;

// ---------------------------------------------------------------------------
// GameTick — global frame counter for cooldown tracking
// ---------------------------------------------------------------------------

/// Monotonically increasing tick counter. Incremented once per Update.
#[derive(Clone, Debug, Default, Resource)]
pub struct GameTick(pub u32);

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default sprint speed multiplier (relative to Navigator base speed).
#[allow(dead_code)]
const SPRINT_SPEED_MULTIPLIER: f64 = 2.2;

/// Default sprint duration in ticks (20 tps → 2 seconds).
const SPRINT_DURATION_TICKS: u32 = 40;

/// Default sprint cooldown in ticks after sprint ends.
const SPRINT_COOLDOWN_TICKS: u32 = 60;

/// Default dash distance in blocks.
const DASH_DISTANCE: f64 = 8.0;

/// Default dash duration in ticks (very fast — ~0.3s).
const DASH_DURATION_TICKS: u32 = 6;

/// Default dash cooldown in ticks.
const DASH_COOLDOWN_TICKS: u32 = 80;

/// Collision sweep step size for dash (blocks per check).
const DASH_SWEEP_STEP: f64 = 0.5;

/// How far a melee knockback pushes the target (blocks).
const KNOCKBACK_DISTANCE: f64 = 4.0;

/// Duration of knockback in ticks.
const KNOCKBACK_DURATION_TICKS: u32 = 5;

// ---------------------------------------------------------------------------
// MovementMode — who owns Position this tick
// ---------------------------------------------------------------------------

/// What kind of movement is active right now.
#[derive(Clone, Debug)]
pub enum MovementMode {
    /// Navigator handles ground A* pathfinding. Default state.
    GroundNav,
    /// A short burst of speed — Navigator still drives, but speed is scaled.
    Sprinting(SprintState),
    /// An override ability has taken over Position writing.
    Override(ActiveOverride),
}

impl Default for MovementMode {
    fn default() -> Self {
        Self::GroundNav
    }
}

impl MovementMode {
    /// Whether the Navigator should yield (not write Position this tick).
    pub fn navigator_should_yield(&self) -> bool {
        matches!(self, Self::Override(_))
    }

    /// Speed multiplier that the Navigator should apply. 1.0 when not sprinting.
    pub fn speed_scale(&self) -> f64 {
        match self {
            Self::Sprinting(s) => s.multiplier,
            _ => 1.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Sprint state (Modifier)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct SprintState {
    pub multiplier: f64,
    pub remaining_ticks: u32,
}

// ---------------------------------------------------------------------------
// Override abilities
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub enum ActiveOverride {
    Dash(DashState),
    /// Involuntary pushback from a melee hit. Same physics as Dash, no cooldown.
    Knockback(DashState),
    // Future: Leap(LeapState), AirStep(AirStepState), Flight(FlightState)
}

#[derive(Clone, Debug)]
pub struct DashState {
    /// Normalized XZ direction of the dash.
    pub direction: DVec3,
    /// Total distance to cover.
    pub total_distance: f64,
    /// Distance covered so far.
    pub distance_covered: f64,
    /// Distance per tick.
    pub speed_per_tick: f64,
    /// Y level to maintain during dash (resolved at activation).
    pub ground_y: f64,
}

// ---------------------------------------------------------------------------
// MovementController — the single source of truth
// ---------------------------------------------------------------------------

/// Per-NPC movement state. Brain actions write here; Navigator and ability
/// systems read from here.
#[derive(Clone, Debug, Default, Component)]
pub struct MovementController {
    pub mode: MovementMode,
}

impl MovementController {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the Navigator should skip its tick for this NPC.
    pub fn navigator_should_yield(&self) -> bool {
        self.mode.navigator_should_yield()
    }

    /// Speed scale for the Navigator (Sprint multiplier or 1.0).
    pub fn speed_scale(&self) -> f64 {
        self.mode.speed_scale()
    }

    /// Return to default ground navigation.
    pub fn reset_to_ground(&mut self) {
        self.mode = MovementMode::GroundNav;
    }
}

// ---------------------------------------------------------------------------
// MovementCapabilities — what this NPC is allowed to do
// ---------------------------------------------------------------------------

/// Which movement abilities this NPC has unlocked.
/// A low-tier zombie might only have `can_sprint`; a Boss has everything.
#[derive(Clone, Debug, Component)]
pub struct MovementCapabilities {
    pub can_sprint: bool,
    pub can_dash: bool,
    // Future fields:
    // pub can_leap: bool,
    // pub can_air_step: bool,
    // pub can_fly: bool,
    // pub can_blink: bool,
}

impl Default for MovementCapabilities {
    fn default() -> Self {
        Self {
            can_sprint: true,
            can_dash: false,
        }
    }
}

// ---------------------------------------------------------------------------
// MovementCooldowns — tick-based cooldown tracking
// ---------------------------------------------------------------------------

/// Tracks when each ability becomes available again (game tick).
#[derive(Clone, Debug, Default, Component)]
pub struct MovementCooldowns {
    /// Tick at which sprint becomes available.
    pub sprint_ready_at: u32,
    /// Tick at which dash becomes available.
    pub dash_ready_at: u32,
}

// ---------------------------------------------------------------------------
// Activation helpers — called by Brain actions
// ---------------------------------------------------------------------------

/// Try to activate Sprint. Returns `true` if successful.
pub fn activate_sprint(
    controller: &mut MovementController,
    capabilities: &MovementCapabilities,
    cooldowns: &mut MovementCooldowns,
    current_tick: u32,
) -> bool {
    if !capabilities.can_sprint {
        return false;
    }
    if current_tick < cooldowns.sprint_ready_at {
        return false;
    }
    if controller.mode.navigator_should_yield() {
        return false; // don't interrupt an override
    }

    controller.mode = MovementMode::Sprinting(SprintState {
        multiplier: SPRINT_SPEED_MULTIPLIER,
        remaining_ticks: SPRINT_DURATION_TICKS,
    });
    true
}

/// Try to activate Dash. Returns `true` if successful.
///
/// `facing_dir` should be a normalized XZ direction (typically toward the
/// target or the NPC's current facing).
pub fn activate_dash(
    controller: &mut MovementController,
    capabilities: &MovementCapabilities,
    cooldowns: &mut MovementCooldowns,
    current_tick: u32,
    facing_dir: DVec3,
    ground_y: f64,
) -> bool {
    if !capabilities.can_dash {
        return false;
    }
    if current_tick < cooldowns.dash_ready_at {
        return false;
    }
    if controller.mode.navigator_should_yield() {
        return false;
    }

    let dir = DVec3::new(facing_dir.x, 0.0, facing_dir.z);
    let len = dir.length();
    if len < 1e-6 {
        return false;
    }
    let dir = dir / len;

    controller.mode = MovementMode::Override(ActiveOverride::Dash(DashState {
        direction: dir,
        total_distance: DASH_DISTANCE,
        distance_covered: 0.0,
        speed_per_tick: DASH_DISTANCE / DASH_DURATION_TICKS as f64,
        ground_y,
    }));
    true
}

// ---------------------------------------------------------------------------
// Knockback — involuntary pushback from melee hits
// ---------------------------------------------------------------------------

/// Insert on an entity to apply knockback next tick. The `apply_pending_knockback_system`
/// converts this into an Override.
#[derive(Clone, Debug, Component)]
pub struct PendingKnockback {
    /// Direction the target is pushed (attacker → target).
    pub direction: DVec3,
}

/// Force-activate a knockback override. Ignores capabilities and cooldowns.
fn activate_knockback(controller: &mut MovementController, direction: DVec3, ground_y: f64) {
    let dir = DVec3::new(direction.x, 0.0, direction.z);
    let len = dir.length();
    if len < 1e-6 {
        return;
    }
    let dir = dir / len;

    controller.mode = MovementMode::Override(ActiveOverride::Knockback(DashState {
        direction: dir,
        total_distance: KNOCKBACK_DISTANCE,
        distance_covered: 0.0,
        speed_per_tick: KNOCKBACK_DISTANCE / KNOCKBACK_DURATION_TICKS as f64,
        ground_y,
    }));
}

fn apply_pending_knockback_system(
    mut commands: Commands,
    mut controllable: Query<(
        Entity,
        &Position,
        &PendingKnockback,
        &mut MovementController,
    )>,
    stale: Query<Entity, (With<PendingKnockback>, Without<MovementController>)>,
) {
    // Apply knockback to entities that have a MovementController (NPCs).
    for (entity, position, knockback, mut ctrl) in &mut controllable {
        activate_knockback(&mut ctrl, knockback.direction, position.get().y);
        commands.entity(entity).remove::<PendingKnockback>();
    }
    // Clean up PendingKnockback on entities without MovementController (players).
    for entity in &stale {
        commands.entity(entity).remove::<PendingKnockback>();
    }
}

// ---------------------------------------------------------------------------
// ECS system — ticks Override abilities and expires Modifiers
// ---------------------------------------------------------------------------

pub fn register(app: &mut App) {
    tracing::info!("[bong][npc] registering movement ability system");
    app.insert_resource(GameTick::default()).add_systems(
        Update,
        (
            increment_game_tick,
            apply_pending_knockback_system.after(increment_game_tick),
            movement_ability_tick_system.after(apply_pending_knockback_system),
        )
            .before(crate::npc::navigator::navigator_tick_system),
    );
}

fn increment_game_tick(mut tick: ResMut<GameTick>) {
    tick.0 = tick.0.wrapping_add(1);
}

/// Ticks active movement abilities. Runs in `Update` **before** Navigator.
///
/// - **Sprint (Modifier)**: just counts down; Navigator reads the speed scale.
/// - **Dash (Override)**: writes Position directly, Navigator yields.
/// - When an ability expires, resets to `GroundNav` and writes cooldown.
#[allow(clippy::type_complexity)]
fn movement_ability_tick_system(
    mut npcs: Query<
        (
            Entity,
            &mut Position,
            &mut Transform,
            &mut Look,
            &mut HeadYaw,
            &mut MovementController,
            &mut MovementCooldowns,
        ),
        With<NpcMarker>,
    >,
    layers: Query<&ChunkLayer>,
    game_tick: Res<GameTick>,
) {
    let layer = layers.get_single().ok();
    let tick = game_tick.0;

    for (_entity, mut position, mut transform, mut look, mut head_yaw, mut ctrl, mut cooldowns) in
        &mut npcs
    {
        match &mut ctrl.mode {
            MovementMode::GroundNav => {
                // Nothing to do — Navigator handles it.
            }

            MovementMode::Sprinting(ref mut state) => {
                if state.remaining_ticks == 0 {
                    cooldowns.sprint_ready_at = tick + SPRINT_COOLDOWN_TICKS;
                    ctrl.mode = MovementMode::GroundNav;
                } else {
                    state.remaining_ticks -= 1;
                }
            }

            MovementMode::Override(ActiveOverride::Dash(ref mut dash)) => {
                if dash.distance_covered >= dash.total_distance {
                    cooldowns.dash_ready_at = tick + DASH_COOLDOWN_TICKS;
                    ctrl.mode = MovementMode::GroundNav;
                    continue;
                }

                let step = dash.speed_per_tick;
                let current = position.get();
                let next_x = current.x + dash.direction.x * step;
                let next_z = current.z + dash.direction.z * step;
                let ground_y = resolve_ground_y_at(
                    next_x.floor() as i32,
                    next_z.floor() as i32,
                    dash.ground_y as i32,
                    layer,
                );
                dash.ground_y = ground_y;
                let tentative = DVec3::new(next_x, ground_y, next_z);

                if is_blocked_at(tentative, layer) {
                    cooldowns.dash_ready_at = tick + DASH_COOLDOWN_TICKS;
                    ctrl.mode = MovementMode::GroundNav;
                    continue;
                }

                position.set(tentative);
                transform.translation.x = tentative.x as f32;
                transform.translation.y = tentative.y as f32;
                transform.translation.z = tentative.z as f32;

                let yaw = (dash.direction.z.atan2(dash.direction.x).to_degrees() - 90.0) as f32;
                look.yaw = yaw;
                look.pitch = 0.0;
                head_yaw.0 = yaw;

                dash.distance_covered += step;
            }

            MovementMode::Override(ActiveOverride::Knockback(ref mut kb)) => {
                if kb.distance_covered >= kb.total_distance {
                    ctrl.mode = MovementMode::GroundNav;
                    continue;
                }

                let step = kb.speed_per_tick;
                let current = position.get();
                let next_x = current.x + kb.direction.x * step;
                let next_z = current.z + kb.direction.z * step;
                let ground_y = resolve_ground_y_at(
                    next_x.floor() as i32,
                    next_z.floor() as i32,
                    kb.ground_y as i32,
                    layer,
                );
                kb.ground_y = ground_y;
                let tentative = DVec3::new(next_x, ground_y, next_z);

                if is_blocked_at(tentative, layer) {
                    ctrl.mode = MovementMode::GroundNav;
                    continue;
                }

                position.set(tentative);
                transform.translation.x = tentative.x as f32;
                transform.translation.y = tentative.y as f32;
                transform.translation.z = tentative.z as f32;

                // Don't change facing — NPC should still look at attacker.
                kb.distance_covered += step;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Collision helper — shared by all abilities
// ---------------------------------------------------------------------------

/// Resolve the ground Y (feet level) at a given XZ, scanning down from `ref_y`.
/// Returns the Y of the first air block above a solid block, or `ref_y` as fallback.
fn resolve_ground_y_at(wx: i32, wz: i32, ref_y: i32, layer: Option<&ChunkLayer>) -> f64 {
    let Some(layer) = layer else {
        return ref_y as f64;
    };
    let chunk_pos = ChunkPos::new(wx.div_euclid(16), wz.div_euclid(16));
    let Some(chunk) = layer.chunk(chunk_pos) else {
        return ref_y as f64;
    };

    let lx = wx.rem_euclid(16) as u32;
    let lz = wz.rem_euclid(16) as u32;
    let min_y = layer.min_y();
    let max_y = min_y + layer.height() as i32 - 1;
    let start = ref_y.clamp(min_y, max_y);

    for y in (min_y..=start).rev() {
        let ly = (y - min_y) as u32;
        let block = chunk.block_state(lx, ly, lz);
        if is_solid_block(block) {
            return (y + 1) as f64;
        }
    }
    ref_y as f64
}

/// Check if a position is blocked (solid block at feet or head level).
/// Used by Override abilities for collision detection.
fn is_blocked_at(pos: DVec3, layer: Option<&ChunkLayer>) -> bool {
    let Some(layer) = layer else {
        return false;
    };

    let wx = pos.x.floor() as i32;
    let wz = pos.z.floor() as i32;
    let feet_y = pos.y.floor() as i32;

    let min_y = layer.min_y();
    let max_y = min_y + layer.height() as i32 - 1;

    for y in [feet_y, feet_y + 1] {
        if y < min_y || y > max_y {
            continue;
        }

        let chunk_pos = ChunkPos::new(wx.div_euclid(16), wz.div_euclid(16));
        let Some(chunk) = layer.chunk(chunk_pos) else {
            continue;
        };

        let lx = wx.rem_euclid(16) as u32;
        let ly = (y - min_y) as u32;
        let lz = wz.rem_euclid(16) as u32;
        let block = chunk.block_state(lx, ly, lz);

        if is_solid_block(block) {
            return true;
        }
    }

    false
}

/// Whether a block is solid for NPC collision purposes.
fn is_solid_block(block: BlockState) -> bool {
    if block == BlockState::AIR || block == BlockState::CAVE_AIR {
        return false;
    }
    if block == BlockState::WATER || block == BlockState::LAVA {
        return false;
    }
    if is_passthrough(block) {
        return false;
    }
    true
}

/// Blocks NPCs can move through (vegetation, torches, etc.).
fn is_passthrough(block: BlockState) -> bool {
    block == BlockState::GRASS
        || block == BlockState::TALL_GRASS
        || block == BlockState::FERN
        || block == BlockState::LARGE_FERN
        || block == BlockState::POPPY
        || block == BlockState::DANDELION
        || block == BlockState::DEAD_BUSH
        || block == BlockState::SNOW
        || block == BlockState::VINE
        || block == BlockState::TORCH
        || block == BlockState::WALL_TORCH
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_controller_is_ground_nav() {
        let ctrl = MovementController::new();
        assert!(!ctrl.navigator_should_yield());
        assert_eq!(ctrl.speed_scale(), 1.0);
    }

    #[test]
    fn sprint_activation_respects_capability() {
        let mut ctrl = MovementController::new();
        let no_sprint = MovementCapabilities {
            can_sprint: false,
            can_dash: false,
        };
        let mut cd = MovementCooldowns::default();

        assert!(!activate_sprint(&mut ctrl, &no_sprint, &mut cd, 0));
        assert!(!ctrl.navigator_should_yield());
    }

    #[test]
    fn sprint_activation_sets_speed_scale() {
        let mut ctrl = MovementController::new();
        let caps = MovementCapabilities::default(); // can_sprint = true
        let mut cd = MovementCooldowns::default();

        assert!(activate_sprint(&mut ctrl, &caps, &mut cd, 0));
        assert!(!ctrl.navigator_should_yield()); // Sprint is Modifier, not Override
        assert!(ctrl.speed_scale() > 1.0);
    }

    #[test]
    fn sprint_does_not_interrupt_override() {
        let mut ctrl = MovementController::new();
        let caps = MovementCapabilities {
            can_sprint: true,
            can_dash: true,
        };
        let mut cd = MovementCooldowns::default();

        // Activate dash first.
        assert!(activate_dash(
            &mut ctrl,
            &caps,
            &mut cd,
            0,
            DVec3::new(1.0, 0.0, 0.0),
            66.0,
        ));
        assert!(ctrl.navigator_should_yield());

        // Sprint should fail — can't interrupt Override.
        assert!(!activate_sprint(&mut ctrl, &caps, &mut cd, 0));
    }

    #[test]
    fn dash_activation_respects_cooldown() {
        let mut ctrl = MovementController::new();
        let caps = MovementCapabilities {
            can_sprint: true,
            can_dash: true,
        };
        let mut cd = MovementCooldowns {
            sprint_ready_at: 0,
            dash_ready_at: 100,
        };

        // Current tick 50 < ready_at 100 → should fail.
        assert!(!activate_dash(
            &mut ctrl,
            &caps,
            &mut cd,
            50,
            DVec3::new(1.0, 0.0, 0.0),
            66.0,
        ));

        // Current tick 100 → should succeed.
        assert!(activate_dash(
            &mut ctrl,
            &caps,
            &mut cd,
            100,
            DVec3::new(1.0, 0.0, 0.0),
            66.0,
        ));
        assert!(ctrl.navigator_should_yield());
    }

    #[test]
    fn dash_state_has_correct_speed() {
        let mut ctrl = MovementController::new();
        let caps = MovementCapabilities {
            can_sprint: true,
            can_dash: true,
        };
        let mut cd = MovementCooldowns::default();

        activate_dash(
            &mut ctrl,
            &caps,
            &mut cd,
            0,
            DVec3::new(0.0, 0.0, 1.0),
            66.0,
        );

        if let MovementMode::Override(ActiveOverride::Dash(dash)) = &ctrl.mode {
            assert!((dash.direction.z - 1.0).abs() < 1e-6);
            assert!((dash.total_distance - DASH_DISTANCE).abs() < 1e-6);
            let expected_speed = DASH_DISTANCE / DASH_DURATION_TICKS as f64;
            assert!((dash.speed_per_tick - expected_speed).abs() < 1e-6);
        } else {
            panic!("expected Dash override");
        }
    }

    #[test]
    fn reset_to_ground_clears_mode() {
        let mut ctrl = MovementController::new();
        let caps = MovementCapabilities {
            can_sprint: true,
            can_dash: true,
        };
        let mut cd = MovementCooldowns::default();

        activate_dash(
            &mut ctrl,
            &caps,
            &mut cd,
            0,
            DVec3::new(1.0, 0.0, 0.0),
            66.0,
        );
        assert!(ctrl.navigator_should_yield());

        ctrl.reset_to_ground();
        assert!(!ctrl.navigator_should_yield());
        assert_eq!(ctrl.speed_scale(), 1.0);
    }
}
