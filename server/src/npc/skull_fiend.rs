//! plan-skull-fiend-v1 — 骨煞直线冲撞 AI、命中真元抽吸与表现事件。

use bevy_transform::components::Transform;
use big_brain::prelude::{
    ActionBuilder, ActionState, Actor, BigBrainSet, FirstToScore, Score, ScorerBuilder, Thinker,
    ThinkerBuilder,
};
use valence::prelude::{
    bevy_ecs, App, BlockState, Chunk, ChunkLayer, ChunkPos, Commands, Component, DVec3, Entity,
    EntityLayerId, EventWriter, HeadYaw, IntoSystemConfigs, Look, Position, PreUpdate, Query, Res,
    With, Without,
};

use crate::combat::body_mass::BodyMass;
use crate::combat::components::{WoundKind, Wounds};
use crate::combat::events::{AttackIntent, AttackReach, AttackSource};
use crate::combat::knockback::DEFAULT_CHAIN_DEPTH;
use crate::cultivation::components::Cultivation;
use crate::fauna::experience::play_audio;
use crate::network::audio_event_emit::PlaySoundRecipeRequest;
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::npc::brain::{AgeingScorer, RetireAction};
use crate::npc::movement::{GameTick, PendingKnockback};
use crate::npc::navigator::Navigator;
use crate::npc::spawn::{NpcBlackboard, NpcMarker};
use crate::schema::vfx_event::VfxEventPayloadV1;

const DEFAULT_DETECTION_RANGE_BLOCKS: f32 = 24.0;
const DEFAULT_LOCK_TICKS: u32 = 30;
const DEFAULT_ENRAGED_LOCK_TICKS: u32 = 16;
const DEFAULT_CHARGE_SPEED_BLOCKS_PER_TICK: f64 = 16.0 / 20.0;
const DEFAULT_ENRAGED_CHARGE_SPEED_BLOCKS_PER_TICK: f64 = 22.0 / 20.0;
const DEFAULT_MAX_CHARGE_DISTANCE_BLOCKS: f64 = 32.0;
const DEFAULT_HIT_RADIUS_BLOCKS: f64 = 1.5;
const DEFAULT_VERTICAL_HIT_TOLERANCE_BLOCKS: f64 = 4.0;
const DEFAULT_IMPACT_DAMAGE: f32 = 8.0;
const DEFAULT_QI_DRAIN: f64 = 5.0;
const DEFAULT_STUNNED_TICKS: u32 = 40;
const DEFAULT_ENRAGED_STUNNED_TICKS: u32 = 20;
const DEFAULT_WALL_SELF_DAMAGE_RATIO: f32 = 0.10;
const DEFAULT_ENRAGED_WALL_SELF_DAMAGE_RATIO: f32 = 0.20;
const ENRAGE_HEALTH_RATIO: f32 = 0.30;
const SKULL_FIEND_LOCK_VFX: &str = "bong:skull_fiend_locking";
const SKULL_FIEND_TRAIL_VFX: &str = "bong:skull_fiend_trail";
const SKULL_FIEND_IMPACT_VFX: &str = "bong:skull_fiend_impact";
const SKULL_FIEND_STUNNED_VFX: &str = "bong:skull_fiend_stunned";

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct SkullFiendMarker {
    pub family_id: String,
}

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct SkullFiendConfig {
    pub detection_range_blocks: f32,
    pub lock_ticks: u32,
    pub enraged_lock_ticks: u32,
    pub charge_speed_blocks_per_tick: f64,
    pub enraged_charge_speed_blocks_per_tick: f64,
    pub max_charge_distance_blocks: f64,
    pub hit_radius_blocks: f64,
    pub vertical_hit_tolerance_blocks: f64,
    pub impact_damage: f32,
    pub qi_drain: f64,
    pub stunned_ticks: u32,
    pub enraged_stunned_ticks: u32,
    pub wall_self_damage_ratio: f32,
    pub enraged_wall_self_damage_ratio: f32,
}

impl Default for SkullFiendConfig {
    fn default() -> Self {
        Self {
            detection_range_blocks: DEFAULT_DETECTION_RANGE_BLOCKS,
            lock_ticks: DEFAULT_LOCK_TICKS,
            enraged_lock_ticks: DEFAULT_ENRAGED_LOCK_TICKS,
            charge_speed_blocks_per_tick: DEFAULT_CHARGE_SPEED_BLOCKS_PER_TICK,
            enraged_charge_speed_blocks_per_tick: DEFAULT_ENRAGED_CHARGE_SPEED_BLOCKS_PER_TICK,
            max_charge_distance_blocks: DEFAULT_MAX_CHARGE_DISTANCE_BLOCKS,
            hit_radius_blocks: DEFAULT_HIT_RADIUS_BLOCKS,
            vertical_hit_tolerance_blocks: DEFAULT_VERTICAL_HIT_TOLERANCE_BLOCKS,
            impact_damage: DEFAULT_IMPACT_DAMAGE,
            qi_drain: DEFAULT_QI_DRAIN,
            stunned_ticks: DEFAULT_STUNNED_TICKS,
            enraged_stunned_ticks: DEFAULT_ENRAGED_STUNNED_TICKS,
            wall_self_damage_ratio: DEFAULT_WALL_SELF_DAMAGE_RATIO,
            enraged_wall_self_damage_ratio: DEFAULT_ENRAGED_WALL_SELF_DAMAGE_RATIO,
        }
    }
}

impl SkullFiendConfig {
    pub fn profile_for(self, enraged: bool) -> SkullFiendChargeProfile {
        if enraged {
            SkullFiendChargeProfile {
                lock_ticks: self.enraged_lock_ticks,
                speed_blocks_per_tick: self.enraged_charge_speed_blocks_per_tick,
                stunned_ticks: self.enraged_stunned_ticks,
                wall_self_damage_ratio: self.enraged_wall_self_damage_ratio,
                enraged: true,
            }
        } else {
            SkullFiendChargeProfile {
                lock_ticks: self.lock_ticks,
                speed_blocks_per_tick: self.charge_speed_blocks_per_tick,
                stunned_ticks: self.stunned_ticks,
                wall_self_damage_ratio: self.wall_self_damage_ratio,
                enraged: false,
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkullFiendChargeProfile {
    pub lock_ticks: u32,
    pub speed_blocks_per_tick: f64,
    pub stunned_ticks: u32,
    pub wall_self_damage_ratio: f32,
    pub enraged: bool,
}

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub enum SkullFiendState {
    Idle,
    Locking {
        target: Entity,
        locked_target_pos: DVec3,
        locked_direction: DVec3,
        entered_at_tick: u32,
    },
    Charging {
        target: Entity,
        direction: DVec3,
        velocity: DVec3,
        origin: DVec3,
        distance_covered: f64,
    },
    Stunned {
        until_tick: u32,
        reason: SkullFiendStunReason,
    },
}

impl Default for SkullFiendState {
    fn default() -> Self {
        Self::Idle
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkullFiendStunReason {
    HitTarget,
    HitWall,
    MaxDistance,
    LostTarget,
}

#[derive(Component, Clone, Copy, Debug)]
pub struct SkullFiendAggroScorer;

#[derive(Component, Clone, Copy, Debug)]
pub struct SkullFiendChargeAction;

impl ScorerBuilder for SkullFiendAggroScorer {
    fn build(&self, cmd: &mut Commands, entity: Entity, _actor: Entity) {
        cmd.entity(entity).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("SkullFiendAggroScorer")
    }
}

impl ActionBuilder for SkullFiendChargeAction {
    fn build(&self, cmd: &mut Commands, entity: Entity, _actor: Entity) {
        cmd.entity(entity).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("SkullFiendChargeAction")
    }
}

pub fn register(app: &mut App) {
    app.add_systems(
        PreUpdate,
        skull_fiend_aggro_scorer_system.in_set(BigBrainSet::Scorers),
    )
    .add_systems(
        PreUpdate,
        skull_fiend_charge_action_system.in_set(BigBrainSet::Actions),
    );
}

pub fn skull_fiend_thinker() -> ThinkerBuilder {
    Thinker::build()
        .picker(FirstToScore { threshold: 0.05 })
        .when(AgeingScorer, RetireAction)
        .when(SkullFiendAggroScorer, SkullFiendChargeAction)
}

fn skull_fiend_aggro_scorer_system(
    npcs: Query<(&NpcBlackboard, &SkullFiendConfig, &SkullFiendState), With<SkullFiendMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<SkullFiendAggroScorer>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let value = npcs
            .get(*actor)
            .map(|(bb, config, state)| skull_fiend_aggro_score(bb, config, state))
            .unwrap_or(0.0);
        score.set(value);
    }
}

pub fn skull_fiend_aggro_score(
    bb: &NpcBlackboard,
    config: &SkullFiendConfig,
    state: &SkullFiendState,
) -> f32 {
    if !matches!(state, SkullFiendState::Idle) {
        return 0.0;
    }
    if bb.nearest_player.is_none() || !bb.player_distance.is_finite() {
        return 0.0;
    }
    if bb.player_distance > config.detection_range_blocks {
        return 0.0;
    }
    (1.0 - bb.player_distance / config.detection_range_blocks).clamp(0.25, 1.0)
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn skull_fiend_charge_action_system(
    mut commands: Commands,
    mut actions: Query<(&Actor, &mut ActionState), With<SkullFiendChargeAction>>,
    mut skulls: Query<
        (
            Entity,
            &mut Position,
            &mut Transform,
            Option<&mut Look>,
            Option<&mut HeadYaw>,
            &EntityLayerId,
            &mut SkullFiendState,
            &SkullFiendConfig,
            &NpcBlackboard,
            Option<&mut Navigator>,
            Option<&mut Wounds>,
        ),
        (With<NpcMarker>, With<SkullFiendMarker>),
    >,
    target_positions: Query<&Position, Without<SkullFiendMarker>>,
    target_body_masses: Query<&BodyMass>,
    mut cultivations: Query<&mut Cultivation>,
    layers: Query<&ChunkLayer>,
    mut attack_intents: EventWriter<AttackIntent>,
    mut vfx_events: EventWriter<VfxEventRequest>,
    mut audio_events: EventWriter<PlaySoundRecipeRequest>,
    game_tick: Option<Res<GameTick>>,
) {
    let tick = game_tick.as_deref().map(|tick| tick.0).unwrap_or(0);
    for (Actor(actor), mut action_state) in &mut actions {
        let Ok((
            entity,
            mut position,
            mut transform,
            look,
            head_yaw,
            layer_id,
            mut skull_state,
            config,
            bb,
            navigator,
            wounds,
        )) = skulls.get_mut(*actor)
        else {
            *action_state = ActionState::Failure;
            continue;
        };
        let mut skull_view = SkullFiendView {
            entity,
            position: &mut position,
            transform: &mut transform,
            look,
            head_yaw,
            layer_id,
            state: &mut skull_state,
            config: *config,
            blackboard: bb,
            navigator,
            wounds,
        };

        match *action_state {
            ActionState::Requested => {
                if begin_skull_fiend_charge(
                    &mut skull_view,
                    tick,
                    &target_positions,
                    &mut vfx_events,
                    &mut audio_events,
                ) {
                    *action_state = ActionState::Executing;
                } else {
                    *action_state = ActionState::Failure;
                }
            }
            ActionState::Executing => {
                if tick_skull_fiend_charge(
                    &mut commands,
                    &mut skull_view,
                    tick,
                    &target_positions,
                    &target_body_masses,
                    &mut cultivations,
                    &layers,
                    &mut attack_intents,
                    &mut vfx_events,
                    &mut audio_events,
                ) {
                    *action_state = ActionState::Success;
                }
            }
            ActionState::Cancelled => {
                *skull_view.state = SkullFiendState::Idle;
                *action_state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

struct SkullFiendView<'a> {
    entity: Entity,
    position: &'a mut Position,
    transform: &'a mut Transform,
    look: Option<bevy_ecs::change_detection::Mut<'a, Look>>,
    head_yaw: Option<bevy_ecs::change_detection::Mut<'a, HeadYaw>>,
    layer_id: &'a EntityLayerId,
    state: &'a mut SkullFiendState,
    config: SkullFiendConfig,
    blackboard: &'a NpcBlackboard,
    navigator: Option<bevy_ecs::change_detection::Mut<'a, Navigator>>,
    wounds: Option<bevy_ecs::change_detection::Mut<'a, Wounds>>,
}

fn begin_skull_fiend_charge(
    skull: &mut SkullFiendView<'_>,
    tick: u32,
    target_positions: &Query<&Position, Without<SkullFiendMarker>>,
    vfx_events: &mut EventWriter<VfxEventRequest>,
    audio_events: &mut EventWriter<PlaySoundRecipeRequest>,
) -> bool {
    if !matches!(*skull.state, SkullFiendState::Idle) {
        return matches!(
            *skull.state,
            SkullFiendState::Locking { .. }
                | SkullFiendState::Charging { .. }
                | SkullFiendState::Stunned { .. }
        );
    }
    let Some(target) = skull.blackboard.nearest_player else {
        return false;
    };
    if skull.blackboard.player_distance > skull.config.detection_range_blocks {
        return false;
    }
    let Ok(target_pos) = target_positions.get(target) else {
        return false;
    };
    let origin = skull.position.get();
    let Some(direction) = locked_charge_direction(origin, target_pos.get()) else {
        return false;
    };
    if let Some(navigator) = skull.navigator.as_mut() {
        navigator.stop();
    }
    face_direction(skull, direction);
    *skull.state = SkullFiendState::Locking {
        target,
        locked_target_pos: DVec3::new(target_pos.get().x, origin.y, target_pos.get().z),
        locked_direction: direction,
        entered_at_tick: tick,
    };
    emit_skull_fiend_vfx(
        vfx_events,
        SkullFiendVfx {
            event_id: SKULL_FIEND_LOCK_VFX,
            origin,
            direction: Some(direction),
            color: "#AA0022",
            strength: 0.9,
            count: 18,
            duration_ticks: 30,
        },
    );
    audio_events.send(play_audio("fauna_fuya_charge", origin, 1.1, 0.2));
    true
}

#[allow(clippy::too_many_arguments)]
fn tick_skull_fiend_charge(
    commands: &mut Commands,
    skull: &mut SkullFiendView<'_>,
    tick: u32,
    target_positions: &Query<&Position, Without<SkullFiendMarker>>,
    target_body_masses: &Query<&BodyMass>,
    cultivations: &mut Query<&mut Cultivation>,
    layers: &Query<&ChunkLayer>,
    attack_intents: &mut EventWriter<AttackIntent>,
    vfx_events: &mut EventWriter<VfxEventRequest>,
    audio_events: &mut EventWriter<PlaySoundRecipeRequest>,
) -> bool {
    let enraged = skull.wounds.as_deref().is_some_and(is_skull_fiend_enraged);
    let profile = skull.config.profile_for(enraged);

    match *skull.state {
        SkullFiendState::Idle => true,
        SkullFiendState::Locking {
            target,
            locked_direction,
            entered_at_tick,
            ..
        } => {
            if tick.wrapping_sub(entered_at_tick) < profile.lock_ticks {
                face_direction(skull, locked_direction);
                return false;
            }
            let velocity = locked_direction * profile.speed_blocks_per_tick;
            *skull.state = SkullFiendState::Charging {
                target,
                direction: locked_direction,
                velocity,
                origin: skull.position.get(),
                distance_covered: 0.0,
            };
            emit_skull_fiend_vfx(
                vfx_events,
                SkullFiendVfx {
                    event_id: SKULL_FIEND_TRAIL_VFX,
                    origin: skull.position.get(),
                    direction: Some(locked_direction),
                    color: if profile.enraged {
                        "#FF0033"
                    } else {
                        "#31004A"
                    },
                    strength: if profile.enraged { 1.0 } else { 0.75 },
                    count: if profile.enraged { 22 } else { 14 },
                    duration_ticks: 20,
                },
            );
            audio_events.send(play_audio(
                "fauna_fuya_charge",
                skull.position.get(),
                if profile.enraged { 1.25 } else { 1.0 },
                if profile.enraged { 0.35 } else { 0.0 },
            ));
            false
        }
        SkullFiendState::Charging {
            target,
            direction,
            velocity,
            origin,
            mut distance_covered,
        } => {
            let current = skull.position.get();
            let next = next_charge_position(current, velocity);
            let layer = layers.get(skull.layer_id.0).ok();
            if is_skull_fiend_blocked_at(next, layer) {
                apply_wall_self_damage(skull.wounds.as_deref_mut(), profile.wall_self_damage_ratio);
                enter_stunned(
                    skull,
                    tick,
                    profile.stunned_ticks,
                    SkullFiendStunReason::HitWall,
                    vfx_events,
                );
                return false;
            }

            set_skull_fiend_position(skull, next);
            face_direction(skull, direction);
            distance_covered += velocity.length();
            if tick.is_multiple_of(4) {
                emit_skull_fiend_vfx(
                    vfx_events,
                    SkullFiendVfx {
                        event_id: SKULL_FIEND_TRAIL_VFX,
                        origin: next,
                        direction: Some(direction),
                        color: if profile.enraged {
                            "#FF0033"
                        } else {
                            "#31004A"
                        },
                        strength: if profile.enraged { 0.9 } else { 0.65 },
                        count: if profile.enraged { 12 } else { 8 },
                        duration_ticks: 12,
                    },
                );
            }

            if let Ok(target_pos) = target_positions.get(target) {
                if charge_hit_target(next, target_pos.get(), &skull.config) {
                    if let Ok(mut cultivation) = cultivations.get_mut(target) {
                        drain_target_qi(&mut cultivation, skull.config.qi_drain);
                    }
                    attack_intents.send(AttackIntent {
                        attacker: skull.entity,
                        target: Some(target),
                        issued_at_tick: u64::from(tick),
                        reach: AttackReach::new(skull.config.hit_radius_blocks as f32, 0.0),
                        qi_invest: skull.config.impact_damage,
                        wound_kind: WoundKind::Blunt,
                        source: AttackSource::Melee,
                        debug_command: None,
                    });
                    commands
                        .entity(target)
                        .insert(PendingKnockback::from_distance(
                            velocity,
                            6.0,
                            target_body_masses
                                .get(target)
                                .ok()
                                .copied()
                                .unwrap_or_default()
                                .total_mass(),
                            DEFAULT_CHAIN_DEPTH,
                        ));
                    emit_skull_fiend_vfx(
                        vfx_events,
                        SkullFiendVfx {
                            event_id: SKULL_FIEND_IMPACT_VFX,
                            origin: next,
                            direction: Some(direction),
                            color: "#F2F2FF",
                            strength: 1.0,
                            count: 28,
                            duration_ticks: 18,
                        },
                    );
                    enter_stunned(
                        skull,
                        tick,
                        profile.stunned_ticks,
                        SkullFiendStunReason::HitTarget,
                        vfx_events,
                    );
                    return false;
                }
            } else {
                enter_stunned(
                    skull,
                    tick,
                    profile.stunned_ticks,
                    SkullFiendStunReason::LostTarget,
                    vfx_events,
                );
                return false;
            }

            if charge_exceeds_max_distance(origin, next, skull.config.max_charge_distance_blocks)
                || distance_covered >= skull.config.max_charge_distance_blocks
            {
                enter_stunned(
                    skull,
                    tick,
                    profile.stunned_ticks,
                    SkullFiendStunReason::MaxDistance,
                    vfx_events,
                );
                return false;
            }

            *skull.state = SkullFiendState::Charging {
                target,
                direction,
                velocity,
                origin,
                distance_covered,
            };
            false
        }
        SkullFiendState::Stunned { until_tick, .. } => {
            if tick >= until_tick {
                *skull.state = SkullFiendState::Idle;
                true
            } else {
                false
            }
        }
    }
}

pub fn locked_charge_direction(attacker_pos: DVec3, target_pos: DVec3) -> Option<DVec3> {
    let delta = DVec3::new(
        target_pos.x - attacker_pos.x,
        0.0,
        target_pos.z - attacker_pos.z,
    );
    let len = delta.length();
    (len > f64::EPSILON).then_some(delta / len)
}

pub fn next_charge_position(current: DVec3, velocity: DVec3) -> DVec3 {
    DVec3::new(current.x + velocity.x, current.y, current.z + velocity.z)
}

pub fn charge_hit_target(skull_pos: DVec3, target_pos: DVec3, config: &SkullFiendConfig) -> bool {
    let horizontal = DVec3::new(skull_pos.x - target_pos.x, 0.0, skull_pos.z - target_pos.z);
    horizontal.length() <= config.hit_radius_blocks
        && (skull_pos.y - target_pos.y).abs() <= config.vertical_hit_tolerance_blocks
}

pub fn charge_exceeds_max_distance(origin: DVec3, current: DVec3, max_distance: f64) -> bool {
    DVec3::new(current.x - origin.x, 0.0, current.z - origin.z).length() >= max_distance
}

pub fn drain_target_qi(cultivation: &mut Cultivation, amount: f64) -> f64 {
    let drain = cultivation.qi_current.max(0.0).min(amount.max(0.0));
    cultivation.qi_current -= drain;
    drain
}

pub fn is_skull_fiend_enraged(wounds: &Wounds) -> bool {
    wounds.health_max > f32::EPSILON
        && (wounds.health_current / wounds.health_max).clamp(0.0, 1.0) < ENRAGE_HEALTH_RATIO
}

fn enter_stunned(
    skull: &mut SkullFiendView<'_>,
    tick: u32,
    stunned_ticks: u32,
    reason: SkullFiendStunReason,
    vfx_events: &mut EventWriter<VfxEventRequest>,
) {
    *skull.state = SkullFiendState::Stunned {
        until_tick: tick.saturating_add(stunned_ticks),
        reason,
    };
    emit_skull_fiend_vfx(
        vfx_events,
        SkullFiendVfx {
            event_id: SKULL_FIEND_STUNNED_VFX,
            origin: skull.position.get(),
            direction: None,
            color: "#7C2BCB",
            strength: 0.75,
            count: 16,
            duration_ticks: stunned_ticks.min(40) as u16,
        },
    );
}

fn set_skull_fiend_position(skull: &mut SkullFiendView<'_>, pos: DVec3) {
    skull.position.set(pos);
    skull.transform.translation.x = pos.x as f32;
    skull.transform.translation.y = pos.y as f32;
    skull.transform.translation.z = pos.z as f32;
}

fn face_direction(skull: &mut SkullFiendView<'_>, direction: DVec3) {
    if direction.length_squared() <= f64::EPSILON {
        return;
    }
    let yaw = (direction.z.atan2(direction.x).to_degrees() - 90.0) as f32;
    if let Some(look) = skull.look.as_mut() {
        look.yaw = yaw;
        look.pitch = 0.0;
    }
    if let Some(head_yaw) = skull.head_yaw.as_mut() {
        head_yaw.0 = yaw;
    }
}

fn apply_wall_self_damage(wounds: Option<&mut Wounds>, ratio: f32) {
    let Some(wounds) = wounds else {
        return;
    };
    if wounds.health_max <= f32::EPSILON {
        return;
    }
    let damage = wounds.health_max * ratio.max(0.0);
    wounds.health_current = (wounds.health_current - damage).max(0.0);
}

struct SkullFiendVfx<'a> {
    event_id: &'a str,
    origin: DVec3,
    direction: Option<DVec3>,
    color: &'a str,
    strength: f32,
    count: u16,
    duration_ticks: u16,
}

fn emit_skull_fiend_vfx(vfx_events: &mut EventWriter<VfxEventRequest>, vfx: SkullFiendVfx<'_>) {
    vfx_events.send(VfxEventRequest::new(
        vfx.origin,
        VfxEventPayloadV1::SpawnParticle {
            event_id: vfx.event_id.to_string(),
            origin: [vfx.origin.x, vfx.origin.y, vfx.origin.z],
            direction: vfx.direction.map(|dir| [dir.x, dir.y, dir.z]),
            color: Some(vfx.color.to_string()),
            strength: Some(vfx.strength),
            count: Some(vfx.count),
            duration_ticks: Some(vfx.duration_ticks),
        },
    ));
}

fn is_skull_fiend_blocked_at(pos: DVec3, layer: Option<&ChunkLayer>) -> bool {
    let Some(layer) = layer else {
        return false;
    };
    let wx = pos.x.floor() as i32;
    let wz = pos.z.floor() as i32;
    let center_y = pos.y.floor() as i32;
    let min_y = layer.min_y();
    let max_y = min_y + layer.height() as i32 - 1;

    for y in [center_y - 1, center_y, center_y + 1] {
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
        if is_solid_skull_fiend_collision(chunk.block_state(lx, ly, lz)) {
            return true;
        }
    }
    false
}

fn is_solid_skull_fiend_collision(block: BlockState) -> bool {
    !matches!(
        block,
        BlockState::AIR
            | BlockState::CAVE_AIR
            | BlockState::WATER
            | BlockState::LAVA
            | BlockState::GRASS
            | BlockState::TALL_GRASS
            | BlockState::FERN
            | BlockState::LARGE_FERN
            | BlockState::POPPY
            | BlockState::DANDELION
            | BlockState::DEAD_BUSH
            | BlockState::SNOW
            | BlockState::VINE
            | BlockState::TORCH
            | BlockState::WALL_TORCH
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skull_fiend_config_defaults_match_plan_numbers() {
        let config = SkullFiendConfig::default();
        assert_eq!(config.detection_range_blocks, 24.0);
        assert_eq!(config.lock_ticks, 30);
        assert!((config.charge_speed_blocks_per_tick - 0.8).abs() < 1e-9);
        assert_eq!(config.max_charge_distance_blocks, 32.0);
        assert_eq!(config.impact_damage, 8.0);
        assert_eq!(config.qi_drain, 5.0);
        assert_eq!(config.stunned_ticks, 40);
    }

    #[test]
    fn skull_fiend_aggro_range_requires_idle_target_and_range() {
        let config = SkullFiendConfig::default();
        let target = Entity::from_raw(7);
        let mut bb = NpcBlackboard {
            nearest_player: Some(target),
            player_distance: 12.0,
            target_position: None,
            last_melee_tick: 0,
        };
        assert!(skull_fiend_aggro_score(&bb, &config, &SkullFiendState::Idle) > 0.4);
        bb.player_distance = 25.0;
        assert_eq!(
            skull_fiend_aggro_score(&bb, &config, &SkullFiendState::Idle),
            0.0
        );
        bb.player_distance = 12.0;
        assert_eq!(
            skull_fiend_aggro_score(
                &bb,
                &config,
                &SkullFiendState::Stunned {
                    until_tick: 10,
                    reason: SkullFiendStunReason::HitTarget,
                },
            ),
            0.0
        );
    }

    #[test]
    fn skull_fiend_no_steering_locks_xz_direction() {
        let dir = locked_charge_direction(DVec3::new(0.0, 72.0, 0.0), DVec3::new(3.0, 64.0, 4.0))
            .expect("direction should resolve");
        assert!((dir.x - 0.6).abs() < 1e-9);
        assert_eq!(dir.y, 0.0);
        assert!((dir.z - 0.8).abs() < 1e-9);
    }

    #[test]
    fn skull_fiend_charge_hit_covers_front_side_edge_and_vertical_miss() {
        let config = SkullFiendConfig::default();
        assert!(charge_hit_target(
            DVec3::new(1.5, 66.0, 0.0),
            DVec3::new(0.0, 64.0, 0.0),
            &config
        ));
        assert!(!charge_hit_target(
            DVec3::new(1.51, 66.0, 0.0),
            DVec3::new(0.0, 64.0, 0.0),
            &config
        ));
        assert!(!charge_hit_target(
            DVec3::new(0.0, 69.0, 0.0),
            DVec3::new(0.0, 64.0, 0.0),
            &config
        ));
    }

    #[test]
    fn skull_fiend_charge_step_preserves_locked_height() {
        let current = DVec3::new(2.0, 70.0, 2.0);
        let next = next_charge_position(current, DVec3::new(0.0, 0.5, 0.8));
        assert_eq!(next.y, 70.0);
        assert!((next.z - 2.8).abs() < 1e-9);
    }

    #[test]
    fn skull_fiend_charge_max_distance_uses_horizontal_span() {
        assert!(charge_exceeds_max_distance(
            DVec3::new(0.0, 80.0, 0.0),
            DVec3::new(32.0, 64.0, 0.0),
            32.0,
        ));
        assert!(!charge_exceeds_max_distance(
            DVec3::new(0.0, 80.0, 0.0),
            DVec3::new(20.0, 64.0, 0.0),
            32.0,
        ));
    }

    #[test]
    fn skull_fiend_qi_drain_clamps_at_available_pool() {
        let mut cultivation = Cultivation {
            qi_current: 3.0,
            qi_max: 10.0,
            ..Default::default()
        };
        let drained = drain_target_qi(&mut cultivation, 5.0);
        assert_eq!(drained, 3.0);
        assert_eq!(cultivation.qi_current, 0.0);
    }

    #[test]
    fn skull_fiend_enrage_profile_shortens_lock_and_raises_speed() {
        let config = SkullFiendConfig::default();
        let calm = Wounds {
            entries: Vec::new(),
            health_current: 40.0,
            health_max: 40.0,
        };
        let enraged = Wounds {
            entries: Vec::new(),
            health_current: 10.0,
            health_max: 40.0,
        };
        assert!(!is_skull_fiend_enraged(&calm));
        assert!(is_skull_fiend_enraged(&enraged));
        let calm_profile = config.profile_for(false);
        let enraged_profile = config.profile_for(true);
        assert!(enraged_profile.lock_ticks < calm_profile.lock_ticks);
        assert!(enraged_profile.speed_blocks_per_tick > calm_profile.speed_blocks_per_tick);
        assert!(enraged_profile.wall_self_damage_ratio > calm_profile.wall_self_damage_ratio);
    }
}
