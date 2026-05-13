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
    bevy_ecs, App, BlockPos, BlockState, Chunk, ChunkLayer, ChunkPos, Client, Commands, Component,
    DVec3, Entity, EventWriter, GameMode, HeadYaw, IntoSystemConfigs, Look, ParamSet, Position,
    Query, Res, ResMut, Resource, Update, With, Without,
};

use crate::combat::body_mass::BodyMass;
use crate::combat::components::{BodyPart, Wound, WoundKind, Wounds};
use crate::combat::events::AttackSource;
use crate::combat::knockback::KnockbackEvent;
use crate::npc::spawn::NpcMarker;
use crate::qi_physics::{
    entity_collision, wall_collision, EntityCollisionInput, KnockbackResult, WallCollisionInput,
    MAX_BLOCK_PENETRATION,
};

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

// ---------------------------------------------------------------------------
// MovementMode — who owns Position this tick
// ---------------------------------------------------------------------------

/// What kind of movement is active right now.
#[derive(Clone, Debug, Default)]
pub enum MovementMode {
    /// Navigator handles ground A* pathfinding. Default state.
    #[default]
    GroundNav,
    /// A short burst of speed — Navigator still drives, but speed is scaled.
    Sprinting(SprintState),
    /// An override ability has taken over Position writing.
    Override(ActiveOverride),
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
    Knockback(KnockbackState),
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

#[derive(Clone, Debug)]
pub struct KnockbackState {
    /// Original attacker when known; collision chains keep this attribution.
    pub attacker: Option<Entity>,
    pub source: AttackSource,
    /// Normalized XZ direction of the forced movement.
    pub direction: DVec3,
    /// Total distance to cover.
    pub total_distance: f64,
    /// Distance covered so far.
    pub distance_covered: f64,
    /// Distance per tick.
    pub speed_per_tick: f64,
    /// Y level to maintain during forced movement.
    pub ground_y: f64,
    /// Kinetic energy carried by this knockback step.
    pub kinetic_energy: f64,
    /// Remaining entity collision chain depth.
    pub chain_depth: u8,
    /// Number of blocks already pierced by this knockback.
    pub blocks_broken: u8,
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
    pub attacker: Option<Entity>,
    pub source: AttackSource,
    /// Direction the target is pushed (attacker → target).
    pub direction: DVec3,
    pub distance_blocks: f64,
    pub duration_ticks: u32,
    pub velocity_blocks_per_tick: f64,
    pub kinetic_energy: f64,
    pub chain_depth: u8,
}

impl PendingKnockback {
    pub fn from_result(
        attacker: Entity,
        source: AttackSource,
        direction: DVec3,
        result: KnockbackResult,
        chain_depth: u8,
    ) -> Self {
        Self {
            attacker: Some(attacker),
            source,
            direction,
            distance_blocks: result.distance_blocks,
            duration_ticks: result.duration_ticks,
            velocity_blocks_per_tick: result.velocity_blocks_per_tick,
            kinetic_energy: result.kinetic_energy,
            chain_depth,
        }
    }

    pub fn from_collision(
        attacker: Option<Entity>,
        source: AttackSource,
        direction: DVec3,
        distance_blocks: f64,
        velocity_blocks_per_tick: f64,
        kinetic_energy: f64,
        chain_depth: u8,
    ) -> Self {
        let duration_ticks = if velocity_blocks_per_tick <= f64::EPSILON {
            1
        } else {
            (distance_blocks / velocity_blocks_per_tick)
                .ceil()
                .clamp(1.0, 30.0) as u32
        };
        Self {
            attacker,
            source,
            direction,
            distance_blocks,
            duration_ticks,
            velocity_blocks_per_tick,
            kinetic_energy,
            chain_depth,
        }
    }

    pub fn from_distance(
        direction: DVec3,
        distance_blocks: f64,
        target_mass: f64,
        chain_depth: u8,
    ) -> Self {
        let result = KnockbackResult::from_distance(distance_blocks, target_mass);
        Self {
            attacker: None,
            source: AttackSource::Melee,
            direction,
            distance_blocks: result.distance_blocks,
            duration_ticks: result.duration_ticks,
            velocity_blocks_per_tick: result.velocity_blocks_per_tick,
            kinetic_energy: result.kinetic_energy,
            chain_depth,
        }
    }
}

/// Force-activate a knockback override. Ignores capabilities and cooldowns.
fn activate_knockback(
    controller: &mut MovementController,
    knockback: &PendingKnockback,
    ground_y: f64,
) {
    let dir = DVec3::new(knockback.direction.x, 0.0, knockback.direction.z);
    let len = dir.length();
    if len < 1e-6 {
        return;
    }
    let dir = dir / len;

    let duration_ticks = knockback.duration_ticks.max(1);
    let distance_blocks = knockback.distance_blocks.max(0.0);

    controller.mode = MovementMode::Override(ActiveOverride::Knockback(KnockbackState {
        attacker: knockback.attacker,
        source: knockback.source,
        direction: dir,
        total_distance: distance_blocks,
        distance_covered: 0.0,
        speed_per_tick: if knockback.velocity_blocks_per_tick > 0.0 {
            knockback.velocity_blocks_per_tick
        } else {
            distance_blocks / f64::from(duration_ticks)
        },
        ground_y,
        kinetic_energy: knockback.kinetic_energy,
        chain_depth: knockback.chain_depth,
        blocks_broken: 0,
    }));
}

type StalePendingKnockbackFilter = (
    With<PendingKnockback>,
    Without<MovementController>,
    Without<Client>,
);

fn apply_pending_knockback_system(
    mut commands: Commands,
    mut controllable: Query<(
        Entity,
        &Position,
        &PendingKnockback,
        &mut MovementController,
    )>,
    stale: Query<Entity, StalePendingKnockbackFilter>,
) {
    // Apply knockback to entities that have a MovementController (NPCs).
    for (entity, position, knockback, mut ctrl) in &mut controllable {
        activate_knockback(&mut ctrl, knockback, position.get().y);
        commands.entity(entity).remove::<PendingKnockback>();
    }
    // Clean up PendingKnockback on non-player entities without MovementController. Players are
    // consumed by movement::player_knockback so Valence velocity can carry the hit reaction.
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
    mut npcs: ParamSet<(
        Query<
            (
                Entity,
                &mut Position,
                &mut Transform,
                &mut Look,
                &mut HeadYaw,
                &mut MovementController,
                &mut MovementCooldowns,
                Option<&BodyMass>,
                Option<&mut Wounds>,
            ),
            With<NpcMarker>,
        >,
        Query<(Entity, &Position, Option<&BodyMass>), With<NpcMarker>>,
    )>,
    mut layers: Query<&mut ChunkLayer, With<crate::world::dimension::OverworldLayer>>,
    mut commands: Commands,
    mut knockback_events: EventWriter<KnockbackEvent>,
    game_tick: Res<GameTick>,
) {
    let collision_targets = {
        let targets = npcs.p1();
        targets
            .iter()
            .map(|(entity, position, body_mass)| CollisionTargetSnapshot {
                entity,
                position: position.get(),
                mass: body_mass.copied().unwrap_or_default().total_mass(),
            })
            .collect::<Vec<_>>()
    };
    let mut layer = layers.get_single_mut().ok();
    let tick = game_tick.0;

    for (
        entity,
        mut position,
        mut transform,
        mut look,
        mut head_yaw,
        mut ctrl,
        mut cooldowns,
        body_mass,
        mut wounds,
    ) in &mut npcs.p0()
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
                    layer.as_deref(),
                );
                dash.ground_y = ground_y;
                let tentative = DVec3::new(next_x, ground_y, next_z);

                if is_blocked_at(tentative, layer.as_deref()) {
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
                    layer.as_deref(),
                );
                kb.ground_y = ground_y;
                let tentative = DVec3::new(next_x, ground_y, next_z);

                if let Some((block_pos, block_state)) =
                    blocked_block_at(tentative, layer.as_deref())
                {
                    let moving_mass = body_mass.copied().unwrap_or_default().total_mass();
                    let collision = wall_collision(WallCollisionInput {
                        target_mass: moving_mass,
                        velocity_blocks_per_tick: kb.speed_per_tick,
                        block_hardness: block_hardness(block_state),
                        armor_mitigation: body_mass
                            .copied()
                            .map(collision_armor_mitigation)
                            .unwrap_or_default(),
                    })
                    .ok();
                    let mut block_broken = false;
                    if let Some(collision) = collision {
                        if collision.entity_damage > f64::EPSILON {
                            if let Some(wounds) = wounds.as_deref_mut() {
                                apply_collision_wound(
                                    wounds,
                                    collision.entity_damage as f32,
                                    u64::from(tick),
                                    kb.attacker.unwrap_or(entity),
                                );
                            }
                        }
                        if collision.block_broken && kb.blocks_broken < MAX_BLOCK_PENETRATION {
                            if let Some(layer) = layer.as_deref_mut() {
                                layer.set_block(block_pos, BlockState::AIR);
                            }
                            kb.blocks_broken = kb.blocks_broken.saturating_add(1);
                            block_broken = true;
                        }
                        knockback_events.send(KnockbackEvent {
                            attacker: kb.attacker.unwrap_or(entity),
                            target: entity,
                            source: kb.source,
                            distance_blocks: kb.total_distance,
                            velocity_blocks_per_tick: kb.speed_per_tick,
                            duration_ticks: duration_ticks_for_knockback(kb),
                            kinetic_energy: collision.kinetic_energy,
                            collision_damage: Some(collision.entity_damage as f32),
                            chain_depth: kb.chain_depth,
                            block_broken,
                        });
                    }
                    if !block_broken || kb.blocks_broken >= MAX_BLOCK_PENETRATION {
                        ctrl.mode = MovementMode::GroundNav;
                        continue;
                    }
                }

                if let Some(hit) = first_entity_collision(entity, tentative, &collision_targets) {
                    if kb.chain_depth > 0 {
                        let moving_mass = body_mass.copied().unwrap_or_default().total_mass();
                        if let Ok(collision) = entity_collision(EntityCollisionInput {
                            moving_mass,
                            hit_mass: hit.mass,
                            incoming_velocity: kb.speed_per_tick,
                            chain_decay: 0.5,
                        }) {
                            if collision.incoming_damage > f64::EPSILON {
                                if let Some(wounds) = wounds.as_deref_mut() {
                                    apply_collision_wound(
                                        wounds,
                                        collision.incoming_damage as f32,
                                        u64::from(tick),
                                        kb.attacker.unwrap_or(hit.entity),
                                    );
                                }
                            }
                            if collision.hit_damage > f64::EPSILON {
                                queue_collision_wound(
                                    &mut commands,
                                    hit.entity,
                                    collision.hit_damage as f32,
                                    u64::from(tick),
                                    entity,
                                );
                            }
                            if collision.transferred_distance >= 0.05 {
                                commands.entity(hit.entity).insert(
                                    PendingKnockback::from_collision(
                                        kb.attacker.or(Some(entity)),
                                        kb.source,
                                        kb.direction,
                                        collision.transferred_distance,
                                        collision.transferred_velocity,
                                        collision.kinetic_energy,
                                        kb.chain_depth.saturating_sub(1),
                                    ),
                                );
                            }
                            knockback_events.send(KnockbackEvent {
                                attacker: kb.attacker.unwrap_or(entity),
                                target: hit.entity,
                                source: kb.source,
                                distance_blocks: collision.transferred_distance,
                                velocity_blocks_per_tick: collision.transferred_velocity,
                                duration_ticks: duration_ticks_for_distance_and_velocity(
                                    collision.transferred_distance,
                                    collision.transferred_velocity,
                                ),
                                kinetic_energy: collision.kinetic_energy,
                                collision_damage: Some(collision.hit_damage as f32),
                                chain_depth: kb.chain_depth.saturating_sub(1),
                                block_broken: false,
                            });
                            ctrl.mode = MovementMode::GroundNav;
                            continue;
                        }
                    }
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

#[derive(Clone, Copy)]
struct CollisionTargetSnapshot {
    entity: Entity,
    position: DVec3,
    mass: f64,
}

fn duration_ticks_for_knockback(kb: &KnockbackState) -> u32 {
    duration_ticks_for_distance_and_velocity(kb.total_distance, kb.speed_per_tick)
}

fn duration_ticks_for_distance_and_velocity(distance: f64, velocity: f64) -> u32 {
    if velocity <= f64::EPSILON {
        1
    } else {
        (distance / velocity).ceil().clamp(1.0, 30.0) as u32
    }
}

fn first_entity_collision(
    moving: Entity,
    tentative: DVec3,
    targets: &[CollisionTargetSnapshot],
) -> Option<CollisionTargetSnapshot> {
    const ENTITY_COLLISION_RADIUS: f64 = 0.75;
    targets
        .iter()
        .copied()
        .filter(|target| target.entity != moving)
        .filter(|target| (target.position.y - tentative.y).abs() <= 1.5)
        .find(|target| {
            let dx = target.position.x - tentative.x;
            let dz = target.position.z - tentative.z;
            dx * dx + dz * dz <= ENTITY_COLLISION_RADIUS * ENTITY_COLLISION_RADIUS
        })
}

fn collision_armor_mitigation(body_mass: BodyMass) -> f64 {
    (body_mass.armor_mass / 60.0).clamp(0.0, 0.85)
}

fn apply_collision_wound(wounds: &mut Wounds, damage: f32, tick: u64, inflicted_by: Entity) {
    let damage = damage.max(0.0);
    if damage <= f32::EPSILON {
        return;
    }
    wounds.health_current = (wounds.health_current - damage).clamp(0.0, wounds.health_max);
    wounds.entries.push(Wound {
        location: BodyPart::Chest,
        kind: WoundKind::Blunt,
        severity: damage,
        bleeding_per_sec: damage * 0.02,
        created_at_tick: tick,
        inflicted_by: Some(format!("knockback:{inflicted_by:?}")),
    });
}

fn queue_collision_wound(
    commands: &mut Commands,
    target: Entity,
    damage: f32,
    tick: u64,
    inflicted_by: Entity,
) {
    commands.add(
        move |world: &mut valence::prelude::bevy_ecs::world::World| {
            if world
                .get::<GameMode>(target)
                .is_some_and(|game_mode| *game_mode != GameMode::Survival)
            {
                return;
            }
            if let Some(mut wounds) = world.get_mut::<Wounds>(target) {
                apply_collision_wound(&mut wounds, damage, tick, inflicted_by);
            }
        },
    );
}

fn blocked_block_at(pos: DVec3, layer: Option<&ChunkLayer>) -> Option<(BlockPos, BlockState)> {
    let layer = layer?;
    let wx = pos.x.floor() as i32;
    let wz = pos.z.floor() as i32;
    let feet_y = pos.y.floor() as i32;
    let min_y = layer.min_y();
    let max_y = min_y + layer.height() as i32 - 1;

    for y in [feet_y, feet_y + 1] {
        if y < min_y || y > max_y {
            continue;
        }
        let block_pos = BlockPos::new(wx, y, wz);
        let block = block_state_at(layer, block_pos)?;
        if is_solid_block(block) {
            return Some((block_pos, block));
        }
    }
    None
}

fn block_state_at(layer: &ChunkLayer, pos: BlockPos) -> Option<BlockState> {
    let chunk_pos = ChunkPos::new(pos.x.div_euclid(16), pos.z.div_euclid(16));
    let chunk = layer.chunk(chunk_pos)?;
    let lx = pos.x.rem_euclid(16) as u32;
    let ly = (pos.y - layer.min_y()) as u32;
    let lz = pos.z.rem_euclid(16) as u32;
    Some(chunk.block_state(lx, ly, lz))
}

fn block_hardness(block: BlockState) -> f64 {
    match block {
        BlockState::COARSE_DIRT
        | BlockState::GRAVEL
        | BlockState::SAND
        | BlockState::SMOOTH_STONE => 0.5,
        BlockState::GRASS_BLOCK | BlockState::DIRT | BlockState::DIRT_PATH => 1.0,
        BlockState::OAK_LOG
        | BlockState::STRIPPED_OAK_LOG
        | BlockState::DARK_OAK_LOG
        | BlockState::OAK_PLANKS => 2.0,
        BlockState::STONE | BlockState::COBBLESTONE | BlockState::STONE_BRICKS => 5.0,
        BlockState::OAK_LEAVES | BlockState::SPRUCE_LEAVES | BlockState::MANGROVE_LEAVES => 8.0,
        BlockState::IRON_BLOCK | BlockState::IRON_BARS | BlockState::IRON_ORE => 15.0,
        BlockState::OBSIDIAN | BlockState::CRYING_OBSIDIAN => 50.0,
        _ => 5.0,
    }
}

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

    #[test]
    fn pending_knockback_activates_dynamic_distance_and_speed() {
        let mut ctrl = MovementController::new();
        let pending = PendingKnockback::from_distance(DVec3::new(1.0, 0.0, 0.0), 6.0, 70.0, 3);

        activate_knockback(&mut ctrl, &pending, 64.0);

        let MovementMode::Override(ActiveOverride::Knockback(kb)) = ctrl.mode else {
            panic!("pending knockback should take over movement");
        };
        assert_eq!(kb.total_distance, 6.0);
        assert!(kb.speed_per_tick > 0.0);
        assert_eq!(kb.chain_depth, 3);
    }

    #[test]
    fn collision_wound_is_blunt_damage_and_reduces_health() {
        let mut wounds = Wounds::default();
        let before = wounds.health_current;

        apply_collision_wound(&mut wounds, 12.0, 7, Entity::from_raw(1));

        assert_eq!(wounds.health_current, before - 12.0);
        assert_eq!(wounds.entries.len(), 1);
        assert_eq!(wounds.entries[0].kind, WoundKind::Blunt);
        assert_eq!(wounds.entries[0].location, BodyPart::Chest);
    }

    #[test]
    fn queued_collision_wound_skips_creative_target() {
        let mut app = App::new();
        let target = app
            .world_mut()
            .spawn((Wounds::default(), GameMode::Creative))
            .id();
        let before = app
            .world()
            .entity(target)
            .get::<Wounds>()
            .unwrap()
            .health_current;
        app.add_systems(Update, move |mut commands: Commands| {
            queue_collision_wound(&mut commands, target, 12.0, 7, Entity::from_raw(1));
        });

        app.update();

        let wounds = app.world().entity(target).get::<Wounds>().unwrap();
        assert_eq!(wounds.health_current, before);
        assert!(wounds.entries.is_empty());
    }

    #[test]
    fn collision_target_scan_excludes_self_and_uses_horizontal_radius() {
        let moving = Entity::from_raw(1);
        let near = Entity::from_raw(2);
        let targets = [
            CollisionTargetSnapshot {
                entity: moving,
                position: DVec3::new(0.0, 64.0, 0.0),
                mass: 70.0,
            },
            CollisionTargetSnapshot {
                entity: near,
                position: DVec3::new(0.5, 64.0, 0.0),
                mass: 70.0,
            },
        ];

        let hit = first_entity_collision(moving, DVec3::new(0.0, 64.0, 0.0), &targets)
            .expect("nearby non-self entity should be detected");

        assert_eq!(hit.entity, near);
    }

    #[test]
    fn block_hardness_matches_knockback_plan_table() {
        assert_eq!(block_hardness(BlockState::DIRT), 1.0);
        assert_eq!(block_hardness(BlockState::OAK_PLANKS), 2.0);
        assert_eq!(block_hardness(BlockState::STONE), 5.0);
        assert_eq!(block_hardness(BlockState::IRON_BLOCK), 15.0);
        assert_eq!(block_hardness(BlockState::OBSIDIAN), 50.0);
    }
}
