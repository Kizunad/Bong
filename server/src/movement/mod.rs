use uuid::Uuid;
use valence::entity::attributes::{EntityAttribute, EntityAttributes};
use valence::entity::{Look, OnGround, Velocity};
use valence::prelude::{
    bevy_ecs, Added, App, BlockPos, BlockState, Changed, ChunkLayer, Client, Commands, Component,
    DVec3, Entity, Event, EventReader, EventWriter, IntoSystemConfigs, Or, Position, Query, Res,
    UniqueId, Update, Vec3, With, Without,
};

use crate::combat::body_mass::BodyMass;
use crate::combat::components::{
    ActiveStatusEffect, DerivedAttrs, Stamina, StaminaState, StatusEffects, Wounds,
};
use crate::combat::events::StatusEffectKind;
use crate::combat::status::remove_status_effect;
use crate::combat::CombatClock;
use crate::cultivation::components::{Cultivation, MeridianId, Realm};
use crate::cultivation::known_techniques::KnownTechniques;
use crate::cultivation::meridian::severed::{
    check_meridian_dependencies, MeridianSeveredPermanent, SkillMeridianDependencies,
};
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::audio_event_emit::{
    AudioRecipient, PlaySoundRecipeRequest, AUDIO_BROADCAST_RADIUS,
};
use crate::network::gameplay_vfx;
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::movement::{
    MovementActionRequestV1, MovementActionV1, MovementStateV1, MovementZoneKindV1,
};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
use crate::schema::vfx_event::VfxEventPayloadV1;
use crate::world::dimension::{CurrentDimension, DimensionKind, DimensionLayers};
use crate::world::events::EVENT_REALM_COLLAPSE;
use crate::world::zone::{Zone, ZoneRegistry};

pub mod armor_weight;
pub mod dash_proficiency;
pub mod leg_wound;
pub mod player_knockback;

pub const BASE_MOVE_SPEED_MULTIPLIER: f32 = 0.75;
pub const EXHAUSTED_SPEED_MULTIPLIER: f32 = 0.6;
pub const LOW_STAMINA_THRESHOLD: f32 = 10.0;
pub const LOW_STAMINA_HUD_RATIO: f32 = 0.30;
pub const DASH_DURATION_TICKS: u64 = 4;
pub const DASH_ATTACK_BONUS_MULTIPLIER: f32 = 1.20;
pub const DASH_ATTACK_BONUS_WINDOW_TICKS: u64 = 10;
const LEG_STRAIN_REFRESH_TICKS: u64 = 40;
const MOVEMENT_SPEED_ATTRIBUTE_UUID: Uuid = Uuid::from_u128(0x426f_6e67_4d6f_7665_6d65_6e74_5631);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MovementAction {
    None,
    Dashing,
}

impl Default for MovementAction {
    fn default() -> Self {
        Self::None
    }
}

impl From<MovementAction> for MovementActionV1 {
    fn from(value: MovementAction) -> Self {
        match value {
            MovementAction::None => Self::None,
            MovementAction::Dashing => Self::Dashing,
        }
    }
}

impl From<MovementActionRequestV1> for MovementAction {
    fn from(value: MovementActionRequestV1) -> Self {
        match value {
            MovementActionRequestV1::Dash => Self::Dashing,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MovementZoneKind {
    Normal,
    Dead,
    Negative,
    ResidueAsh,
}

impl Default for MovementZoneKind {
    fn default() -> Self {
        Self::Normal
    }
}

impl From<MovementZoneKind> for MovementZoneKindV1 {
    fn from(value: MovementZoneKind) -> Self {
        match value {
            MovementZoneKind::Normal => Self::Normal,
            MovementZoneKind::Dead => Self::Dead,
            MovementZoneKind::Negative => Self::Negative,
            MovementZoneKind::ResidueAsh => Self::ResidueAsh,
        }
    }
}

#[derive(Debug, Clone, Event)]
pub struct MovementActionIntent {
    pub entity: Entity,
    pub action: MovementAction,
    pub yaw_degrees: Option<f32>,
}

#[derive(Debug, Clone, Component)]
pub struct MovementState {
    pub current_speed_multiplier: f32,
    pub leg_wound_factor: f32,
    pub zone_kind: MovementZoneKind,
    pub action: MovementAction,
    pub active_until_tick: u64,
    pub dash_ready_at_tick: u64,
    pub dash_attack_bonus_until_tick: u64,
    pub hitbox_height_blocks: f32,
    pub last_grounded: bool,
    pub stamina_cost_active: bool,
    pub last_action_tick: Option<u64>,
    pub rejected_action: Option<MovementActionRequestV1>,
}

impl Default for MovementState {
    fn default() -> Self {
        Self {
            current_speed_multiplier: BASE_MOVE_SPEED_MULTIPLIER,
            leg_wound_factor: 1.0,
            zone_kind: MovementZoneKind::Normal,
            action: MovementAction::None,
            active_until_tick: 0,
            dash_ready_at_tick: 0,
            dash_attack_bonus_until_tick: 0,
            hitbox_height_blocks: 1.8,
            last_grounded: true,
            stamina_cost_active: false,
            last_action_tick: None,
            rejected_action: None,
        }
    }
}

pub fn register(app: &mut App) {
    app.add_event::<MovementActionIntent>();
    app.add_systems(
        Update,
        (
            attach_movement_state_to_joined_clients,
            sync_stamina_regen_from_realm,
            player_knockback::apply_pending_player_knockback_system,
            player_knockback::tick_active_player_knockback_system
                .after(player_knockback::apply_pending_player_knockback_system),
            tick_movement_actions.after(player_knockback::tick_active_player_knockback_system),
            apply_movement_speed_system.after(crate::combat::status::attribute_aggregate_tick),
            handle_movement_action_intents
                .after(crate::network::client_request_handler::handle_client_request_payloads),
            emit_movement_state_payloads
                .after(apply_movement_speed_system)
                .after(tick_movement_actions),
        ),
    );
}

type JoinedClientWithoutMovementFilter = (Added<Client>, Without<MovementState>);

fn attach_movement_state_to_joined_clients(
    mut commands: Commands,
    joined: Query<Entity, JoinedClientWithoutMovementFilter>,
) {
    for entity in &joined {
        commands.entity(entity).insert(MovementState::default());
    }
}

fn sync_stamina_regen_from_realm(mut players: Query<(&Cultivation, &mut Stamina), With<Client>>) {
    for (cultivation, mut stamina) in &mut players {
        let next = stamina_regen_rate(cultivation.realm);
        if (stamina.recover_per_sec - next).abs() > f32::EPSILON {
            stamina.recover_per_sec = next;
        }
    }
}

type MovementSpeedQueryItem<'a> = (
    &'a mut MovementState,
    Option<&'a Cultivation>,
    Option<&'a Stamina>,
    Option<&'a Position>,
    Option<&'a CurrentDimension>,
    Option<&'a mut DerivedAttrs>,
    Option<&'a mut EntityAttributes>,
    Option<&'a Wounds>,
    Option<&'a BodyMass>,
    Option<&'a mut StatusEffects>,
    Option<&'a player_knockback::ActivePlayerKnockback>,
);

fn apply_movement_speed_system(
    clock: Res<CombatClock>,
    zones: Option<Res<ZoneRegistry>>,
    dimension_layers: Option<Res<DimensionLayers>>,
    layers: Query<&ChunkLayer>,
    mut players: Query<MovementSpeedQueryItem<'_>, With<Client>>,
) {
    for (
        mut movement,
        cultivation,
        stamina,
        position,
        dimension,
        derived_attrs,
        entity_attrs,
        wounds,
        body_mass,
        status_effects,
        active_knockback,
    ) in &mut players
    {
        let realm = cultivation
            .map(|cultivation| cultivation.realm)
            .unwrap_or(Realm::Awaken);
        let zone_kind = runtime_zone_kind(
            position,
            dimension,
            zones.as_deref(),
            dimension_layers.as_deref(),
            &layers,
        );
        let stamina_current = stamina.map(|stamina| stamina.current).unwrap_or(100.0);
        let leg_wound_factor = wounds.map(leg_wound::combined_leg_factor).unwrap_or(1.0);
        let armor_weight_factor = body_mass
            .map(|mass| armor_weight::armor_weight_to_speed(mass.armor_mass))
            .unwrap_or(1.0);
        let knockback_recovery_factor = active_knockback
            .filter(|knockback| knockback.is_recovery_only())
            .map(|_| player_knockback::KNOCKBACK_RECOVERY_SPEED_MULTIPLIER)
            .unwrap_or(1.0);
        let multiplier = speed_multiplier_with_factors(
            realm,
            zone_kind,
            clock.tick,
            stamina_current,
            leg_wound_factor,
            armor_weight_factor,
            knockback_recovery_factor,
        );
        if movement.zone_kind != zone_kind {
            movement.zone_kind = zone_kind;
        }
        if (movement.current_speed_multiplier - multiplier).abs() > f32::EPSILON {
            movement.current_speed_multiplier = multiplier;
        }
        if (movement.leg_wound_factor - leg_wound_factor).abs() > f32::EPSILON {
            movement.leg_wound_factor = leg_wound_factor;
        }
        sync_leg_strain_status(status_effects, leg_wound_factor);

        if let Some(mut attrs) = derived_attrs {
            attrs.move_speed_multiplier =
                (attrs.move_speed_multiplier * multiplier).clamp(0.03, 2.0);
            attrs.attack_power = (attrs.attack_power
                * dash_attack_multiplier(&movement, clock.tick))
            .clamp(0.0, 8.0);
        }

        if let Some(mut attributes) = entity_attrs {
            apply_movement_attribute_modifier(&mut attributes, multiplier);
        }
    }
}

fn apply_movement_attribute_modifier(attributes: &mut EntityAttributes, multiplier: f32) {
    attributes.set_multiply_total_modifier(
        EntityAttribute::GenericMovementSpeed,
        MOVEMENT_SPEED_ATTRIBUTE_UUID,
        f64::from(multiplier - 1.0),
    );
}

type MovementActionQueryItem<'a> = (
    &'a mut Client,
    &'a mut MovementState,
    &'a mut Stamina,
    &'a Position,
    &'a Look,
    Option<&'a Cultivation>,
    Option<&'a mut Velocity>,
    Option<&'a UniqueId>,
    Option<&'a mut KnownTechniques>,
    Option<&'a MeridianSeveredPermanent>,
    Option<&'a Wounds>,
    Option<&'a player_knockback::ActivePlayerKnockback>,
);

fn handle_movement_action_intents(
    clock: Res<CombatClock>,
    mut intents: EventReader<MovementActionIntent>,
    skill_meridian_deps: Option<Res<SkillMeridianDependencies>>,
    mut players: Query<MovementActionQueryItem<'_>, With<Client>>,
    mut vfx: EventWriter<VfxEventRequest>,
    mut audio: EventWriter<PlaySoundRecipeRequest>,
) {
    for intent in intents.read() {
        let Ok((
            mut client,
            mut movement,
            mut stamina,
            position,
            look,
            cultivation,
            velocity,
            unique_id,
            known_techniques,
            severed,
            wounds,
            active_knockback,
        )) = players.get_mut(intent.entity)
        else {
            continue;
        };
        let now = clock.tick;
        let realm = cultivation
            .map(|cultivation| cultivation.realm)
            .unwrap_or(Realm::Awaken);
        let dir = intent
            .yaw_degrees
            .and_then(horizontal_direction_from_yaw)
            .unwrap_or_else(|| horizontal_direction(*look));
        let origin = position.get();
        let action = intent.action;
        let mut velocity = velocity;
        let dash_proficiency = known_techniques
            .as_deref()
            .map(dash_proficiency::known_dash_proficiency)
            .unwrap_or_default();

        if let Some(reason) = reject_reason(
            action,
            MovementRejectContext {
                movement: &movement,
                stamina: &stamina,
                dash_proficiency,
                now,
                active_knockback,
                leg_wound_factor: leg_wound::combined_leg_factor_from_optional(wounds),
                skill_meridian_deps: skill_meridian_deps.as_deref(),
                severed,
            },
        ) {
            movement.rejected_action = movement_action_request(action);
            movement.stamina_cost_active = false;
            tracing::debug!(
                "[bong][movement] rejected action={action:?} entity={:?} reason={reason}",
                intent.entity
            );
            continue;
        }

        movement.rejected_action = None;
        movement.stamina_cost_active = true;
        movement.last_action_tick = Some(now);
        spend_stamina(
            &mut stamina,
            movement_action_cost_with_proficiency(action, dash_proficiency),
            now,
        );

        match action {
            MovementAction::None => {}
            MovementAction::Dashing => {
                if let Some(mut known_techniques) = known_techniques {
                    dash_proficiency::record_dash_use(
                        &mut known_techniques,
                        combat_dash_bonus_active(&stamina),
                        false,
                    );
                }
                let impulse = dash_distance_for_runtime(dash_proficiency, &movement);
                apply_client_horizontal_impulse(&mut client, velocity.as_deref_mut(), dir, impulse);
                movement.action = MovementAction::Dashing;
                movement.active_until_tick = now.saturating_add(DASH_DURATION_TICKS);
                movement.dash_ready_at_tick = now.saturating_add(dash_cooldown_for_runtime(
                    realm,
                    dash_proficiency,
                    &movement,
                ));
                movement.dash_attack_bonus_until_tick =
                    now.saturating_add(DASH_ATTACK_BONUS_WINDOW_TICKS);
            }
        }

        emit_action_feedback(action, origin, dir, unique_id, &mut vfx, &mut audio);
    }
}

type MovementTickItem<'a> = (&'a mut MovementState, Option<&'a OnGround>);

fn tick_movement_actions(
    clock: Res<CombatClock>,
    mut players: Query<MovementTickItem<'_>, With<Client>>,
) {
    for (mut movement, on_ground) in &mut players {
        let now = clock.tick;
        let grounded = on_ground.map(|on_ground| on_ground.0).unwrap_or(true);
        movement.last_grounded = grounded;
        if movement.active_until_tick <= now {
            movement.action = MovementAction::None;
            movement.stamina_cost_active = false;
        }
    }
}

type MovementStateEmitFilter = (
    With<Client>,
    Or<(
        Added<MovementState>,
        Changed<MovementState>,
        Changed<Stamina>,
    )>,
);

fn emit_movement_state_payloads(
    clock: Res<CombatClock>,
    mut players: Query<(&mut Client, &MovementState, Option<&Stamina>), MovementStateEmitFilter>,
) {
    for (mut client, movement, stamina) in &mut players {
        let payload = ServerDataV1::new(ServerDataPayloadV1::MovementState(
            movement.to_payload(clock.tick, stamina),
        ));
        let payload_type = payload_type_label(payload.payload_type());
        let payload_bytes = match serialize_server_data_payload(&payload) {
            Ok(bytes) => bytes,
            Err(error) => {
                log_payload_build_error(payload_type, &error);
                continue;
            }
        };
        send_server_data_payload(&mut client, payload_bytes.as_slice());
        tracing::debug!(
            "[bong][movement] sent {} {} payload action={:?} speed={:.3}",
            SERVER_DATA_CHANNEL,
            payload_type,
            movement.action,
            movement.current_speed_multiplier
        );
    }
}

impl MovementState {
    fn to_payload(&self, now_tick: u64, stamina: Option<&Stamina>) -> MovementStateV1 {
        let (stamina_current, stamina_max) = stamina
            .map(|stamina| (stamina.current.max(0.0), stamina.max.max(1.0)))
            .unwrap_or((0.0, 1.0));
        MovementStateV1 {
            current_speed_multiplier: self.current_speed_multiplier,
            stamina_cost_active: self.stamina_cost_active,
            movement_action: self.action.into(),
            zone_kind: self.zone_kind.into(),
            dash_cooldown_remaining_ticks: self.dash_ready_at_tick.saturating_sub(now_tick),
            hitbox_height_blocks: self.hitbox_height_blocks,
            stamina_current,
            stamina_max,
            low_stamina: stamina_current / stamina_max <= LOW_STAMINA_HUD_RATIO,
            last_action_tick: self.last_action_tick,
            rejected_action: self.rejected_action,
        }
    }
}

struct MovementRejectContext<'a> {
    movement: &'a MovementState,
    stamina: &'a Stamina,
    dash_proficiency: f32,
    now: u64,
    active_knockback: Option<&'a player_knockback::ActivePlayerKnockback>,
    leg_wound_factor: f32,
    skill_meridian_deps: Option<&'a SkillMeridianDependencies>,
    severed: Option<&'a MeridianSeveredPermanent>,
}

fn reject_reason(action: MovementAction, ctx: MovementRejectContext<'_>) -> Option<String> {
    if action == MovementAction::None {
        return Some("none_action".to_string());
    }
    if ctx
        .active_knockback
        .is_some_and(player_knockback::ActivePlayerKnockback::is_displacing)
    {
        return Some("knockback".to_string());
    }
    if ctx.stamina.current <= 0.0 {
        return Some("stamina_depleted".to_string());
    }
    let cost = movement_action_cost_with_proficiency(action, ctx.dash_proficiency);
    if ctx.stamina.current < cost {
        return Some("stamina_insufficient".to_string());
    }
    if ctx.movement.action != MovementAction::None && ctx.movement.active_until_tick > ctx.now {
        return Some("movement_action_active".to_string());
    }
    match action {
        MovementAction::None => None,
        MovementAction::Dashing if ctx.now < ctx.movement.dash_ready_at_tick => {
            Some("dash_cooldown".to_string())
        }
        MovementAction::Dashing if ctx.leg_wound_factor <= f32::EPSILON => {
            Some("leg_severed".to_string())
        }
        MovementAction::Dashing
            if dash_meridian_blocker(ctx.skill_meridian_deps, ctx.severed).is_some() =>
        {
            Some("dash_meridian_severed".to_string())
        }
        _ => None,
    }
}

fn emit_action_feedback(
    action: MovementAction,
    origin: DVec3,
    dir: DVec3,
    unique_id: Option<&UniqueId>,
    vfx: &mut EventWriter<VfxEventRequest>,
    audio: &mut EventWriter<PlaySoundRecipeRequest>,
) {
    let (event_id, color, count, duration, recipe_id, anim_id) = match action {
        MovementAction::None => return,
        MovementAction::Dashing => (
            gameplay_vfx::MOVEMENT_DASH,
            "#DDE6EE",
            10,
            10,
            "movement_dash",
            "bong:dash_forward",
        ),
    };

    vfx.send(gameplay_vfx::spawn_request(
        event_id,
        origin,
        Some([dir.x, dir.y, dir.z]),
        color,
        0.75,
        count,
        duration,
    ));
    if let Some(unique_id) = unique_id {
        vfx.send(VfxEventRequest::new(
            origin,
            VfxEventPayloadV1::PlayAnim {
                target_player: unique_id.0.to_string(),
                anim_id: anim_id.to_string(),
                priority: 1450,
                fade_in_ticks: Some(2),
            },
        ));
    }
    audio.send(PlaySoundRecipeRequest {
        recipe_id: recipe_id.to_string(),
        instance_id: 0,
        pos: Some([
            origin.x.floor() as i32,
            origin.y.floor() as i32,
            origin.z.floor() as i32,
        ]),
        flag: None,
        volume_mul: 1.0,
        pitch_shift: 0.0,
        recipient: AudioRecipient::Radius {
            origin,
            radius: AUDIO_BROADCAST_RADIUS,
        },
    });
}

fn spend_stamina(stamina: &mut Stamina, cost: f32, now: u64) {
    stamina.current = (stamina.current - cost).max(0.0);
    stamina.last_drain_tick = Some(now);
    if stamina.current <= 0.0 {
        stamina.state = StaminaState::Exhausted;
    }
}

pub fn movement_action_cost_with_proficiency(action: MovementAction, dash_proficiency: f32) -> f32 {
    match action {
        MovementAction::None => 0.0,
        MovementAction::Dashing => dash_proficiency::dash_stamina_cost(dash_proficiency),
    }
}

fn sync_leg_strain_status(status_effects: Option<MutStatusEffects<'_>>, leg_wound_factor: f32) {
    let Some(mut status_effects) = status_effects else {
        return;
    };
    let magnitude = leg_wound::leg_strain_magnitude(leg_wound_factor);
    if magnitude <= f32::EPSILON {
        remove_status_effect(&mut status_effects, StatusEffectKind::LegStrain);
        return;
    }
    if let Some(effect) = status_effects
        .active
        .iter_mut()
        .find(|effect| effect.kind == StatusEffectKind::LegStrain)
    {
        effect.magnitude = magnitude;
        effect.remaining_ticks = LEG_STRAIN_REFRESH_TICKS;
        return;
    }
    status_effects.active.push(ActiveStatusEffect {
        kind: StatusEffectKind::LegStrain,
        magnitude,
        remaining_ticks: LEG_STRAIN_REFRESH_TICKS,
    });
}

type MutStatusEffects<'a> = valence::prelude::Mut<'a, StatusEffects>;

fn dash_meridian_blocker(
    deps: Option<&SkillMeridianDependencies>,
    severed: Option<&MeridianSeveredPermanent>,
) -> Option<MeridianId> {
    let deps = deps.map(|deps| deps.lookup(dash_proficiency::DASH_TECHNIQUE_ID))?;
    check_meridian_dependencies(deps, severed).err()
}

fn dash_distance_for_runtime(proficiency: f32, movement: &MovementState) -> f32 {
    let leg_factor = movement.leg_wound_factor;
    let wound_adjusted = if leg_factor <= 0.4 {
        0.5
    } else if leg_factor <= 0.7 {
        0.8
    } else {
        1.0
    };
    dash_proficiency::dash_distance(proficiency) * wound_adjusted
}

fn dash_cooldown_for_runtime(realm: Realm, proficiency: f32, movement: &MovementState) -> u64 {
    let base =
        dash_cooldown_by_realm(realm).min(dash_proficiency::dash_cooldown_ticks(proficiency));
    let leg_factor = movement.leg_wound_factor;
    if leg_factor <= 0.4 {
        ((base as f32) * 1.5).ceil() as u64
    } else {
        base
    }
}

fn combat_dash_bonus_active(stamina: &Stamina) -> bool {
    matches!(stamina.state, StaminaState::Combat)
}

pub fn realm_speed_bonus(realm: Realm) -> f32 {
    match realm {
        Realm::Awaken => 0.00,
        Realm::Induce => 0.05,
        Realm::Condense => 0.10,
        Realm::Solidify => 0.15,
        Realm::Spirit => 0.20,
        Realm::Void => 0.25,
    }
}

pub fn stamina_regen_rate(realm: Realm) -> f32 {
    match realm {
        Realm::Awaken | Realm::Induce => 2.0,
        Realm::Condense => 3.0,
        Realm::Solidify => 4.0,
        Realm::Spirit => 5.0,
        Realm::Void => 6.0,
    }
}

pub fn zone_speed_modifier(kind: MovementZoneKind, tick: u64) -> f32 {
    match kind {
        MovementZoneKind::Normal => 1.0,
        MovementZoneKind::Dead => 0.8,
        MovementZoneKind::Negative => 0.9 + deterministic_tick_jitter(tick, 0.05),
        MovementZoneKind::ResidueAsh => 0.7,
    }
}

pub fn speed_multiplier_with_factors(
    realm: Realm,
    zone_kind: MovementZoneKind,
    tick: u64,
    stamina_current: f32,
    leg_wound_factor: f32,
    armor_weight_factor: f32,
    knockback_recovery_factor: f32,
) -> f32 {
    let stamina_penalty = if stamina_current < LOW_STAMINA_THRESHOLD {
        EXHAUSTED_SPEED_MULTIPLIER
    } else {
        1.0
    };
    (BASE_MOVE_SPEED_MULTIPLIER
        * (1.0 + realm_speed_bonus(realm))
        * zone_speed_modifier(zone_kind, tick)
        * stamina_penalty
        * leg_wound_factor.clamp(0.0, 1.0)
        * armor_weight_factor.clamp(0.0, 1.0)
        * knockback_recovery_factor.clamp(0.0, 1.0))
    .clamp(0.03, 2.0)
}

fn deterministic_tick_jitter(tick: u64, amplitude: f32) -> f32 {
    let mixed = tick
        .wrapping_mul(1_103_515_245)
        .wrapping_add(12_345)
        .rotate_left(13);
    let unit = (mixed % 10_001) as f32 / 10_000.0;
    (unit * 2.0 - 1.0) * amplitude
}

pub fn dash_cooldown_by_realm(realm: Realm) -> u64 {
    match realm {
        Realm::Spirit | Realm::Void => 20,
        Realm::Condense | Realm::Solidify => 30,
        Realm::Awaken | Realm::Induce => 40,
    }
}

pub fn movement_horizontal_impulse(dir: DVec3, impulse: f32) -> (f32, f32) {
    let normalized = normalize_horizontal(dir);
    (
        normalized.x as f32 * impulse.max(0.0),
        normalized.z as f32 * impulse.max(0.0),
    )
}

fn apply_client_horizontal_impulse(
    client: &mut Client,
    velocity: Option<&mut Velocity>,
    dir: DVec3,
    impulse: f32,
) {
    let (x, z) = movement_horizontal_impulse(dir, impulse);
    if let Some(velocity) = velocity {
        velocity.0.x += x;
        velocity.0.z += z;
        client.set_velocity(velocity.0);
    } else {
        client.set_velocity(Vec3::new(x, 0.0, z));
    }
}

pub fn dash_attack_multiplier(state: &MovementState, tick: u64) -> f32 {
    if state.dash_attack_bonus_until_tick > tick {
        DASH_ATTACK_BONUS_MULTIPLIER
    } else {
        1.0
    }
}

fn movement_action_request(action: MovementAction) -> Option<MovementActionRequestV1> {
    match action {
        MovementAction::None => None,
        MovementAction::Dashing => Some(MovementActionRequestV1::Dash),
    }
}

pub fn movement_zone_kind(zone: Option<&Zone>, on_residue_ash: bool) -> MovementZoneKind {
    if on_residue_ash {
        return MovementZoneKind::ResidueAsh;
    }
    let Some(zone) = zone else {
        return MovementZoneKind::Normal;
    };
    if zone
        .active_events
        .iter()
        .any(|event| event == EVENT_REALM_COLLAPSE)
        || (zone.danger_level >= 5 && zone.spirit_qi <= 0.1)
    {
        MovementZoneKind::Dead
    } else if zone.spirit_qi < -0.2 {
        MovementZoneKind::Negative
    } else {
        MovementZoneKind::Normal
    }
}

fn runtime_zone_kind(
    position: Option<&Position>,
    dimension: Option<&CurrentDimension>,
    zones: Option<&ZoneRegistry>,
    dimension_layers: Option<&DimensionLayers>,
    layers: &Query<&ChunkLayer>,
) -> MovementZoneKind {
    let (Some(position), Some(zones)) = (position, zones) else {
        return MovementZoneKind::Normal;
    };
    let dimension = dimension.map(|dimension| dimension.0).unwrap_or_default();
    let zone = zones.find_zone(dimension, position.get());
    let on_residue_ash = zone.is_some_and(|zone| {
        zone_allows_residue_ash_surface(zone)
            && on_residue_ash_surface(position.get(), dimension, dimension_layers, layers)
    });
    movement_zone_kind(zone, on_residue_ash)
}

fn horizontal_direction(look: Look) -> DVec3 {
    let v = look.vec();
    normalize_horizontal(DVec3::new(f64::from(v.x), 0.0, f64::from(v.z)))
}

fn horizontal_direction_from_yaw(yaw_degrees: f32) -> Option<DVec3> {
    if !yaw_degrees.is_finite() {
        return None;
    }
    let yaw = f64::from(yaw_degrees).to_radians();
    Some(normalize_horizontal(DVec3::new(-yaw.sin(), 0.0, yaw.cos())))
}

fn normalize_horizontal(dir: DVec3) -> DVec3 {
    let horizontal = DVec3::new(dir.x, 0.0, dir.z);
    if horizontal.length_squared() <= 1e-8 {
        DVec3::new(0.0, 0.0, 1.0)
    } else {
        horizontal.normalize()
    }
}

fn zone_allows_residue_ash_surface(zone: &Zone) -> bool {
    zone.name.contains("ash")
        || zone
            .active_events
            .iter()
            .any(|event| event == "no_cadence" || event == "tribulation_scorch")
}

fn on_residue_ash_surface(
    position: DVec3,
    dimension: DimensionKind,
    dimension_layers: Option<&DimensionLayers>,
    layers: &Query<&ChunkLayer>,
) -> bool {
    let Some(dimension_layers) = dimension_layers else {
        return false;
    };
    let Ok(layer) = layers.get(dimension_layers.entity_for(dimension)) else {
        return false;
    };
    let foot_y = (position.y - 0.05).floor() as i32;
    let below_y = (position.y - 1.0).floor() as i32;
    [foot_y, below_y].into_iter().any(|y| {
        let pos = BlockPos::new(position.x.floor() as i32, y, position.z.floor() as i32);
        layer
            .block(pos)
            .is_some_and(|block| is_residue_ash_block(block.state))
    })
}

fn is_residue_ash_block(block: BlockState) -> bool {
    matches!(
        block,
        BlockState::COARSE_DIRT | BlockState::GRAVEL | BlockState::SAND | BlockState::SMOOTH_STONE
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 1e-5,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn base_speed_reduction() {
        assert_close(
            speed_multiplier_with_factors(
                Realm::Awaken,
                MovementZoneKind::Normal,
                0,
                100.0,
                1.0,
                1.0,
                1.0,
            ),
            0.75,
        );
    }

    #[test]
    fn realm_bonus_stacks() {
        assert_close(
            speed_multiplier_with_factors(
                Realm::Void,
                MovementZoneKind::Normal,
                0,
                100.0,
                1.0,
                1.0,
                1.0,
            ),
            0.9375,
        );
    }

    #[test]
    fn dead_zone_slows() {
        assert_close(
            speed_multiplier_with_factors(
                Realm::Awaken,
                MovementZoneKind::Dead,
                0,
                100.0,
                1.0,
                1.0,
                1.0,
            ),
            0.6,
        );
    }

    #[test]
    fn exhausted_penalty() {
        assert_close(
            speed_multiplier_with_factors(
                Realm::Awaken,
                MovementZoneKind::Normal,
                0,
                9.0,
                1.0,
                1.0,
                1.0,
            ),
            0.45,
        );
    }

    #[test]
    fn dash_impulse_uses_horizontal_direction() {
        let impulse = dash_proficiency::dash_distance(0.0);
        let (x, z) = movement_horizontal_impulse(DVec3::new(1.0, 2.0, 0.0), impulse);
        assert_close(x, impulse);
        assert_close(z, 0.0);
    }

    #[test]
    fn dash_direction_can_use_client_yaw_snapshot() {
        let impulse = dash_proficiency::dash_distance(0.0);
        let dir = horizontal_direction_from_yaw(90.0).expect("finite yaw should map to direction");
        let (x, z) = movement_horizontal_impulse(dir, impulse);
        assert_close(x, -impulse);
        assert_close(z, 0.0);
    }

    #[test]
    fn dash_stamina_cost() {
        assert_close(
            movement_action_cost_with_proficiency(MovementAction::Dashing, 0.0),
            15.0,
        );
    }

    #[test]
    fn dash_cooldown_by_realm() {
        assert_eq!(super::dash_cooldown_by_realm(Realm::Awaken), 40);
        assert_eq!(super::dash_cooldown_by_realm(Realm::Condense), 30);
        assert_eq!(super::dash_cooldown_by_realm(Realm::Spirit), 20);
    }

    #[test]
    fn dash_attack_bonus() {
        let state = MovementState {
            dash_attack_bonus_until_tick: 15,
            ..Default::default()
        };
        assert_close(dash_attack_multiplier(&state, 10), 1.2);
        assert_close(dash_attack_multiplier(&state, 15), 1.0);
    }

    #[test]
    fn residue_ash_speed_modifier_requires_zone_gate_and_surface_block() {
        let mut ash_zone = Zone {
            name: "south_ash_dead_zone".to_string(),
            dimension: DimensionKind::Overworld,
            bounds: (DVec3::ZERO, DVec3::new(10.0, 10.0, 10.0)),
            spirit_qi: 0.0,
            danger_level: 5,
            active_events: vec!["no_cadence".to_string()],
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
        };

        assert!(zone_allows_residue_ash_surface(&ash_zone));
        assert!(is_residue_ash_block(BlockState::COARSE_DIRT));
        assert_eq!(
            movement_zone_kind(Some(&ash_zone), true),
            MovementZoneKind::ResidueAsh
        );

        ash_zone.name = "spawn".to_string();
        ash_zone.active_events.clear();
        ash_zone.spirit_qi = 0.5;
        ash_zone.danger_level = 1;
        assert!(!zone_allows_residue_ash_surface(&ash_zone));
        assert_eq!(
            movement_zone_kind(Some(&ash_zone), false),
            MovementZoneKind::Normal
        );
    }

    #[test]
    fn movement_matrix_covers_dash_6_realms_4_zones_2_stamina_states() {
        let actions = [MovementAction::Dashing];
        let realms = [
            Realm::Awaken,
            Realm::Induce,
            Realm::Condense,
            Realm::Solidify,
            Realm::Spirit,
            Realm::Void,
        ];
        let zones = [
            MovementZoneKind::Normal,
            MovementZoneKind::Dead,
            MovementZoneKind::Negative,
            MovementZoneKind::ResidueAsh,
        ];
        let stamina = [100.0, 0.0];
        let mut covered = 0usize;
        for action in actions {
            for realm in realms {
                for zone in zones {
                    for stamina_current in stamina {
                        let speed = speed_multiplier_with_factors(
                            realm,
                            zone,
                            123,
                            stamina_current,
                            1.0,
                            1.0,
                            1.0,
                        );
                        assert!(speed > 0.0, "{action:?} {realm:?} {zone:?}");
                        covered += 1;
                    }
                }
            }
        }
        assert_eq!(covered, 48);
    }
}
