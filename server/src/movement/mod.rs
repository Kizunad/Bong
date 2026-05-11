use uuid::Uuid;
use valence::entity::attributes::{EntityAttribute, EntityAttributes};
use valence::entity::entity::Pose as PoseComponent;
use valence::entity::{Look, OnGround, Pose, Velocity};
use valence::math::Aabb;
use valence::prelude::{
    bevy_ecs, Added, App, BlockPos, BlockState, Changed, ChunkLayer, Client, Commands, Component,
    DVec3, Entity, Event, EventReader, EventWriter, IntoSystemConfigs, Or, Position, Query, Res,
    UniqueId, Update, With, Without,
};

use crate::combat::components::{
    BodyPart, DerivedAttrs, Stamina, StaminaState, Wound, WoundKind, Wounds,
};
use crate::combat::events::{AttackSource, CombatEvent, DeathEvent};
use crate::combat::CombatClock;
use crate::cultivation::components::{Cultivation, Realm};
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

pub const BASE_MOVE_SPEED_MULTIPLIER: f32 = 0.75;
pub const EXHAUSTED_SPEED_MULTIPLIER: f32 = 0.6;
pub const LOW_STAMINA_THRESHOLD: f32 = 10.0;
pub const LOW_STAMINA_HUD_RATIO: f32 = 0.30;
pub const DASH_DISTANCE_BLOCKS: f64 = 4.0;
pub const DASH_DURATION_TICKS: u64 = 4;
pub const DASH_STAMINA_COST: f32 = 15.0;
pub const DASH_ATTACK_BONUS_MULTIPLIER: f32 = 1.20;
pub const DASH_ATTACK_BONUS_WINDOW_TICKS: u64 = 10;
pub const SLIDE_DISTANCE_BLOCKS: f64 = 3.0;
pub const SLIDE_DURATION_TICKS: u64 = 8;
pub const SLIDE_STAND_TRANSITION_TICKS: u64 = 6;
pub const SLIDE_STAMINA_COST: f32 = 12.0;
pub const SLIDE_COOLDOWN_TICKS: u64 = 60;
pub const SLIDE_CONTACT_DAMAGE: f32 = 8.0;
pub const SLIDE_HITBOX_HEIGHT_BLOCKS: f32 = 1.0;
pub const DOUBLE_JUMP_STAMINA_COST: f32 = 20.0;
pub const DOUBLE_JUMP_DURATION_TICKS: u64 = 4;
pub const DOUBLE_JUMP_BASE_VERTICAL_VELOCITY: f32 = 6.4;
pub const DOUBLE_JUMP_DIRECTIONAL_NUDGE_BLOCKS: f64 = 0.8;
pub const PLAYER_COLLISION_WIDTH_BLOCKS: f64 = 0.6;
pub const MOVEMENT_SWEEP_STEP_BLOCKS: f64 = 0.2;
const MOVEMENT_SPEED_ATTRIBUTE_UUID: Uuid = Uuid::from_u128(0x426f_6e67_4d6f_7665_6d65_6e74_5631);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MovementAction {
    None,
    Dashing,
    Sliding,
    DoubleJumping,
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
            MovementAction::Sliding => Self::Sliding,
            MovementAction::DoubleJumping => Self::DoubleJumping,
        }
    }
}

impl From<MovementActionRequestV1> for MovementAction {
    fn from(value: MovementActionRequestV1) -> Self {
        match value {
            MovementActionRequestV1::Dash => Self::Dashing,
            MovementActionRequestV1::Slide => Self::Sliding,
            MovementActionRequestV1::DoubleJump => Self::DoubleJumping,
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
}

#[derive(Debug, Clone, Component)]
pub struct MovementState {
    pub current_speed_multiplier: f32,
    pub zone_kind: MovementZoneKind,
    pub action: MovementAction,
    pub active_until_tick: u64,
    pub stand_transition_until_tick: u64,
    pub dash_ready_at_tick: u64,
    pub slide_ready_at_tick: u64,
    pub dash_attack_bonus_until_tick: u64,
    pub double_jump_charges_remaining: u8,
    pub double_jump_charges_max: u8,
    pub hitbox_height_blocks: f32,
    pub slide_contact_damage_applied: bool,
    pub last_grounded: bool,
    pub stamina_cost_active: bool,
    pub last_action_tick: Option<u64>,
    pub rejected_action: Option<MovementActionRequestV1>,
}

impl Default for MovementState {
    fn default() -> Self {
        Self {
            current_speed_multiplier: BASE_MOVE_SPEED_MULTIPLIER,
            zone_kind: MovementZoneKind::Normal,
            action: MovementAction::None,
            active_until_tick: 0,
            stand_transition_until_tick: 0,
            dash_ready_at_tick: 0,
            slide_ready_at_tick: 0,
            dash_attack_bonus_until_tick: 0,
            double_jump_charges_remaining: 1,
            double_jump_charges_max: 1,
            hitbox_height_blocks: 1.8,
            slide_contact_damage_applied: false,
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
            tick_movement_actions.after(apply_slide_contact_damage_system),
            apply_movement_speed_system.after(crate::combat::status::attribute_aggregate_tick),
            handle_movement_action_intents
                .after(crate::network::client_request_handler::handle_client_request_payloads),
            apply_slide_contact_damage_system.after(handle_movement_action_intents),
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
    Option<&'a OnGround>,
    Option<&'a mut DerivedAttrs>,
    Option<&'a mut EntityAttributes>,
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
        on_ground,
        derived_attrs,
        entity_attrs,
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
        let multiplier = speed_multiplier(realm, zone_kind, clock.tick, stamina_current);
        let max_charges = double_jump_charges_by_realm(realm);
        let grounded = on_ground
            .map(|on_ground| on_ground.0)
            .unwrap_or(movement.last_grounded);
        let remaining_charges = double_jump_remaining_after_grounded(
            grounded,
            movement.double_jump_charges_remaining,
            max_charges,
        );

        if movement.zone_kind != zone_kind {
            movement.zone_kind = zone_kind;
        }
        if (movement.current_speed_multiplier - multiplier).abs() > f32::EPSILON {
            movement.current_speed_multiplier = multiplier;
        }
        if movement.double_jump_charges_max != max_charges {
            movement.double_jump_charges_max = max_charges;
        }
        if movement.double_jump_charges_remaining != remaining_charges {
            movement.double_jump_charges_remaining = remaining_charges;
        }

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
    &'a mut MovementState,
    &'a mut Stamina,
    &'a mut Position,
    &'a Look,
    Option<&'a Cultivation>,
    Option<&'a OnGround>,
    Option<&'a mut Velocity>,
    Option<&'a mut PoseComponent>,
    Option<&'a CurrentDimension>,
    Option<&'a UniqueId>,
);

fn handle_movement_action_intents(
    clock: Res<CombatClock>,
    mut intents: EventReader<MovementActionIntent>,
    dimension_layers: Option<Res<DimensionLayers>>,
    layers: Query<&ChunkLayer>,
    mut players: Query<MovementActionQueryItem<'_>, With<Client>>,
    mut vfx: EventWriter<VfxEventRequest>,
    mut audio: EventWriter<PlaySoundRecipeRequest>,
) {
    for intent in intents.read() {
        let Ok((
            mut movement,
            mut stamina,
            mut position,
            look,
            cultivation,
            on_ground,
            velocity,
            pose,
            dimension,
            unique_id,
        )) = players.get_mut(intent.entity)
        else {
            continue;
        };
        let now = clock.tick;
        let grounded = on_ground.map(|on_ground| on_ground.0).unwrap_or(true);
        let realm = cultivation
            .map(|cultivation| cultivation.realm)
            .unwrap_or(Realm::Awaken);
        let dir = horizontal_direction(*look);
        let origin = position.get();
        let action = intent.action;
        let dimension_kind = dimension.map(|dimension| dimension.0).unwrap_or_default();

        if let Some(reason) = reject_reason(action, &movement, &stamina, grounded, now) {
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
        spend_stamina(&mut stamina, movement_action_cost(action), now);

        match action {
            MovementAction::None => {}
            MovementAction::Dashing => {
                position.0 = movement_displacement_checked(
                    origin,
                    dir,
                    DASH_DISTANCE_BLOCKS,
                    movement.hitbox_height_blocks,
                    dimension_kind,
                    dimension_layers.as_deref(),
                    &layers,
                );
                movement.action = MovementAction::Dashing;
                movement.active_until_tick = now.saturating_add(DASH_DURATION_TICKS);
                movement.dash_ready_at_tick = now.saturating_add(dash_cooldown_by_realm(realm));
                movement.dash_attack_bonus_until_tick =
                    now.saturating_add(DASH_ATTACK_BONUS_WINDOW_TICKS);
            }
            MovementAction::Sliding => {
                position.0 = movement_displacement_checked(
                    origin,
                    dir,
                    SLIDE_DISTANCE_BLOCKS,
                    SLIDE_HITBOX_HEIGHT_BLOCKS,
                    dimension_kind,
                    dimension_layers.as_deref(),
                    &layers,
                );
                movement.action = MovementAction::Sliding;
                movement.active_until_tick = now.saturating_add(SLIDE_DURATION_TICKS);
                movement.stand_transition_until_tick = slide_stand_transition_end(now);
                movement.slide_ready_at_tick = now.saturating_add(SLIDE_COOLDOWN_TICKS);
                movement.hitbox_height_blocks = slide_hitbox_height(MovementAction::Sliding);
                movement.slide_contact_damage_applied = false;
                if let Some(mut pose) = pose {
                    *pose = PoseComponent(Pose::Swimming);
                }
            }
            MovementAction::DoubleJumping => {
                movement.double_jump_charges_remaining =
                    movement.double_jump_charges_remaining.saturating_sub(1);
                movement.action = MovementAction::DoubleJumping;
                movement.active_until_tick = now.saturating_add(DOUBLE_JUMP_DURATION_TICKS);
                let air_dir = velocity
                    .as_ref()
                    .map(|velocity| {
                        DVec3::new(f64::from(velocity.0.x), 0.0, f64::from(velocity.0.z))
                    })
                    .map(|current| double_jump_direction_after_air_turn(current, dir))
                    .unwrap_or(dir);
                position.0 = movement_displacement_checked(
                    origin,
                    air_dir,
                    DOUBLE_JUMP_DIRECTIONAL_NUDGE_BLOCKS,
                    movement.hitbox_height_blocks,
                    dimension_kind,
                    dimension_layers.as_deref(),
                    &layers,
                );
                if let Some(mut velocity) = velocity {
                    velocity.0.y =
                        DOUBLE_JUMP_BASE_VERTICAL_VELOCITY * double_jump_height_multiplier(realm);
                    velocity.0.x += air_dir.x as f32 * 1.8;
                    velocity.0.z += air_dir.z as f32 * 1.8;
                }
            }
        }

        emit_action_feedback(action, origin, dir, unique_id, &mut vfx, &mut audio);
    }
}

type SlideContactPlayerItem<'a> = (Entity, &'a mut MovementState, &'a Position);
type SlideContactTargetItem<'a> = (Entity, &'a Position, &'a mut Wounds);

fn apply_slide_contact_damage_system(
    clock: Res<CombatClock>,
    mut players: Query<SlideContactPlayerItem<'_>, With<Client>>,
    mut targets: Query<SlideContactTargetItem<'_>, (With<Wounds>, Without<Client>)>,
    mut combat_events: EventWriter<CombatEvent>,
    mut death_events: EventWriter<DeathEvent>,
) {
    for (player, mut movement, player_pos) in &mut players {
        if movement.action != MovementAction::Sliding
            || movement.active_until_tick <= clock.tick
            || movement.slide_contact_damage_applied
        {
            continue;
        }

        let mut hit_any = false;
        for (target, target_pos, mut wounds) in &mut targets {
            if !slide_contact_in_range(
                player_pos.get(),
                target_pos.get(),
                movement.hitbox_height_blocks,
            ) {
                continue;
            }
            hit_any = true;
            let was_alive = wounds.health_current > 0.0;
            let damage = apply_slide_contact_damage(&mut wounds, clock.tick);
            combat_events.send(CombatEvent {
                attacker: player,
                target,
                resolved_at_tick: clock.tick,
                body_part: BodyPart::LegL,
                wound_kind: WoundKind::Blunt,
                source: AttackSource::Melee,
                damage,
                contam_delta: 0.0,
                description: "滑铲撞击".to_string(),
                defense_kind: None,
                defense_effectiveness: None,
                defense_contam_reduced: None,
                defense_wound_severity: None,
            });
            if was_alive && wounds.health_current <= 0.0 {
                death_events.send(DeathEvent {
                    target,
                    cause: "slide_contact".to_string(),
                    attacker: Some(player),
                    attacker_player_id: None,
                    at_tick: clock.tick,
                });
            }
        }

        movement.slide_contact_damage_applied = true;
        if !hit_any {
            tracing::trace!(
                "[bong][movement] slide contact checked with no target player={player:?}"
            );
        }
    }
}

type MovementTickItem<'a> = (
    &'a mut MovementState,
    Option<&'a OnGround>,
    Option<&'a mut PoseComponent>,
);

fn tick_movement_actions(
    clock: Res<CombatClock>,
    mut players: Query<MovementTickItem<'_>, With<Client>>,
) {
    for (mut movement, on_ground, pose) in &mut players {
        let now = clock.tick;
        let grounded = on_ground.map(|on_ground| on_ground.0).unwrap_or(true);
        movement.last_grounded = grounded;
        if grounded {
            movement.double_jump_charges_remaining = movement.double_jump_charges_max;
        }
        if movement.active_until_tick <= now {
            movement.action = MovementAction::None;
            movement.stamina_cost_active = false;
        }
        if movement.stand_transition_until_tick != 0 && movement.stand_transition_until_tick <= now
        {
            movement.stand_transition_until_tick = 0;
            movement.hitbox_height_blocks = slide_hitbox_height(MovementAction::None);
            if let Some(mut pose) = pose {
                *pose = PoseComponent(Pose::Standing);
            }
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
            slide_cooldown_remaining_ticks: self.slide_ready_at_tick.saturating_sub(now_tick),
            double_jump_charges_remaining: self.double_jump_charges_remaining,
            double_jump_charges_max: self.double_jump_charges_max,
            hitbox_height_blocks: self.hitbox_height_blocks,
            stamina_current,
            stamina_max,
            low_stamina: stamina_current / stamina_max <= LOW_STAMINA_HUD_RATIO,
            last_action_tick: self.last_action_tick,
            rejected_action: self.rejected_action,
        }
    }
}

fn reject_reason(
    action: MovementAction,
    movement: &MovementState,
    stamina: &Stamina,
    grounded: bool,
    now: u64,
) -> Option<String> {
    if action == MovementAction::None {
        return Some("none_action".to_string());
    }
    if stamina.current <= 0.0 {
        return Some("stamina_depleted".to_string());
    }
    let cost = movement_action_cost(action);
    if stamina.current < cost {
        return Some("stamina_insufficient".to_string());
    }
    if movement.action != MovementAction::None && movement.active_until_tick > now {
        return Some("movement_action_active".to_string());
    }
    match action {
        MovementAction::None => None,
        MovementAction::Dashing if now < movement.dash_ready_at_tick => {
            Some("dash_cooldown".to_string())
        }
        MovementAction::Sliding if now < movement.slide_ready_at_tick => {
            Some("slide_cooldown".to_string())
        }
        MovementAction::Sliding if !slide_requires_running(stamina.state) => {
            Some("slide_requires_running".to_string())
        }
        MovementAction::Sliding if movement.stand_transition_until_tick > now => {
            Some("slide_stand_transition".to_string())
        }
        MovementAction::DoubleJumping
            if !double_jump_allowed(grounded, movement.double_jump_charges_remaining) =>
        {
            if grounded {
                Some("double_jump_requires_airborne".to_string())
            } else {
                Some("double_jump_no_charges".to_string())
            }
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
        MovementAction::Sliding => (
            gameplay_vfx::MOVEMENT_SLIDE,
            "#9B7653",
            12,
            12,
            "movement_slide",
            "bong:slide_low",
        ),
        MovementAction::DoubleJumping => (
            gameplay_vfx::MOVEMENT_DOUBLE_JUMP,
            "#CCCCFF",
            8,
            8,
            "movement_double_jump",
            "bong:double_jump",
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

pub fn movement_action_cost(action: MovementAction) -> f32 {
    match action {
        MovementAction::None => 0.0,
        MovementAction::Dashing => DASH_STAMINA_COST,
        MovementAction::Sliding => SLIDE_STAMINA_COST,
        MovementAction::DoubleJumping => DOUBLE_JUMP_STAMINA_COST,
    }
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

pub fn speed_multiplier(
    realm: Realm,
    zone_kind: MovementZoneKind,
    tick: u64,
    stamina_current: f32,
) -> f32 {
    let stamina_penalty = if stamina_current < LOW_STAMINA_THRESHOLD {
        EXHAUSTED_SPEED_MULTIPLIER
    } else {
        1.0
    };
    (BASE_MOVE_SPEED_MULTIPLIER
        * (1.0 + realm_speed_bonus(realm))
        * zone_speed_modifier(zone_kind, tick)
        * stamina_penalty)
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

pub fn double_jump_charges_by_realm(realm: Realm) -> u8 {
    match realm {
        Realm::Spirit | Realm::Void => 2,
        _ => 1,
    }
}

pub fn double_jump_remaining_after_grounded(
    grounded: bool,
    charges_remaining: u8,
    max_charges: u8,
) -> u8 {
    if grounded {
        max_charges
    } else {
        charges_remaining.min(max_charges)
    }
}

pub fn double_jump_height_multiplier(realm: Realm) -> f32 {
    match realm {
        Realm::Solidify | Realm::Spirit | Realm::Void => 1.0,
        Realm::Awaken | Realm::Induce | Realm::Condense => 0.8,
    }
}

pub fn slide_requires_running(state: StaminaState) -> bool {
    matches!(state, StaminaState::Sprinting)
}

pub fn slide_hitbox_height(action: MovementAction) -> f32 {
    if action == MovementAction::Sliding {
        SLIDE_HITBOX_HEIGHT_BLOCKS
    } else {
        1.8
    }
}

pub fn slide_stand_transition_end(start_tick: u64) -> u64 {
    start_tick
        .saturating_add(SLIDE_DURATION_TICKS)
        .saturating_add(SLIDE_STAND_TRANSITION_TICKS)
}

pub fn apply_slide_contact_damage(wounds: &mut Wounds, tick: u64) -> f32 {
    let damage = SLIDE_CONTACT_DAMAGE.min(wounds.health_current.max(0.0));
    wounds.health_current = (wounds.health_current - damage).clamp(0.0, wounds.health_max);
    wounds.entries.push(Wound {
        location: BodyPart::LegL,
        kind: WoundKind::Blunt,
        severity: damage,
        bleeding_per_sec: 0.0,
        created_at_tick: tick,
        inflicted_by: Some("movement_slide".to_string()),
    });
    damage
}

pub fn movement_displacement(origin: DVec3, dir: DVec3, distance: f64) -> DVec3 {
    origin + normalize_horizontal(dir) * distance
}

pub fn movement_displacement_swept(
    origin: DVec3,
    dir: DVec3,
    distance: f64,
    hitbox_height_blocks: f32,
    mut collides: impl FnMut(DVec3, f32) -> bool,
) -> DVec3 {
    let target = movement_displacement(origin, dir, distance);
    let delta = target - origin;
    let horizontal_distance = DVec3::new(delta.x, 0.0, delta.z).length();
    if horizontal_distance <= f64::EPSILON {
        return target;
    }
    let steps = (horizontal_distance / MOVEMENT_SWEEP_STEP_BLOCKS)
        .ceil()
        .max(1.0) as u32;
    let mut last_safe = origin;
    for step in 1..=steps {
        let candidate = origin + delta * (f64::from(step) / f64::from(steps));
        if collides(candidate, hitbox_height_blocks) {
            return last_safe;
        }
        last_safe = candidate;
    }
    target
}

fn movement_displacement_checked(
    origin: DVec3,
    dir: DVec3,
    distance: f64,
    hitbox_height_blocks: f32,
    dimension: DimensionKind,
    dimension_layers: Option<&DimensionLayers>,
    layers: &Query<&ChunkLayer>,
) -> DVec3 {
    movement_displacement_swept(
        origin,
        dir,
        distance,
        hitbox_height_blocks,
        |candidate, height| {
            player_collides_with_world(candidate, height, dimension, dimension_layers, layers)
        },
    )
}

fn player_collides_with_world(
    position: DVec3,
    hitbox_height_blocks: f32,
    dimension: DimensionKind,
    dimension_layers: Option<&DimensionLayers>,
    layers: &Query<&ChunkLayer>,
) -> bool {
    let Some(dimension_layers) = dimension_layers else {
        tracing::debug!(
            "[bong][movement] blocked displacement because DimensionLayers is unavailable"
        );
        return true;
    };
    let Ok(layer) = layers.get(dimension_layers.entity_for(dimension)) else {
        tracing::debug!(
            "[bong][movement] blocked displacement because ChunkLayer is unavailable for dimension={dimension:?}"
        );
        return true;
    };
    let player_aabb = player_collision_aabb(position, hitbox_height_blocks);
    player_aabb_intersects_blocks(layer, player_aabb)
}

fn player_collision_aabb(position: DVec3, hitbox_height_blocks: f32) -> Aabb {
    Aabb::from_bottom_size(
        position,
        DVec3::new(
            PLAYER_COLLISION_WIDTH_BLOCKS,
            f64::from(hitbox_height_blocks.max(0.1)),
            PLAYER_COLLISION_WIDTH_BLOCKS,
        ),
    )
}

fn player_aabb_intersects_blocks(layer: &ChunkLayer, player_aabb: Aabb) -> bool {
    let min = player_aabb.min();
    let max = player_aabb.max();
    for x in min.x.floor() as i32..=max.x.floor() as i32 {
        for y in min.y.floor() as i32..=max.y.floor() as i32 {
            for z in min.z.floor() as i32..=max.z.floor() as i32 {
                let pos = BlockPos::new(x, y, z);
                let Some(block) = layer.block(pos) else {
                    continue;
                };
                if block.state == BlockState::AIR {
                    continue;
                }
                for shape in block.state.collision_shapes() {
                    let block_aabb = shape + DVec3::new(f64::from(x), f64::from(y), f64::from(z));
                    if player_aabb.intersects(block_aabb) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

pub fn dash_attack_multiplier(state: &MovementState, tick: u64) -> f32 {
    if state.dash_attack_bonus_until_tick > tick {
        DASH_ATTACK_BONUS_MULTIPLIER
    } else {
        1.0
    }
}

pub fn double_jump_allowed(grounded: bool, charges_remaining: u8) -> bool {
    !grounded && charges_remaining > 0
}

fn movement_action_request(action: MovementAction) -> Option<MovementActionRequestV1> {
    match action {
        MovementAction::None => None,
        MovementAction::Dashing => Some(MovementActionRequestV1::Dash),
        MovementAction::Sliding => Some(MovementActionRequestV1::Slide),
        MovementAction::DoubleJumping => Some(MovementActionRequestV1::DoubleJump),
    }
}

pub fn double_jump_direction_after_air_turn(current: DVec3, requested: DVec3) -> DVec3 {
    let current = normalize_horizontal(current);
    let requested = normalize_horizontal(requested);
    let dot = current.dot(requested).clamp(-1.0, 1.0);
    let angle = dot.acos();
    let max = std::f64::consts::FRAC_PI_4;
    if angle <= max {
        return requested;
    }
    normalize_horizontal(current.lerp(requested, max / angle))
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

fn normalize_horizontal(dir: DVec3) -> DVec3 {
    let horizontal = DVec3::new(dir.x, 0.0, dir.z);
    if horizontal.length_squared() <= 1e-8 {
        DVec3::new(0.0, 0.0, 1.0)
    } else {
        horizontal.normalize()
    }
}

fn horizontal_distance_squared(a: DVec3, b: DVec3) -> f64 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    dx * dx + dz * dz
}

fn slide_contact_in_range(player_pos: DVec3, target_pos: DVec3, hitbox_height_blocks: f32) -> bool {
    let delta = target_pos - player_pos;
    if delta.y.abs() > f64::from(hitbox_height_blocks.max(0.1)) {
        return false;
    }
    horizontal_distance_squared(player_pos, target_pos) <= 1.44
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
            speed_multiplier(Realm::Awaken, MovementZoneKind::Normal, 0, 100.0),
            0.75,
        );
    }

    #[test]
    fn realm_bonus_stacks() {
        assert_close(
            speed_multiplier(Realm::Void, MovementZoneKind::Normal, 0, 100.0),
            0.9375,
        );
    }

    #[test]
    fn dead_zone_slows() {
        assert_close(
            speed_multiplier(Realm::Awaken, MovementZoneKind::Dead, 0, 100.0),
            0.6,
        );
    }

    #[test]
    fn exhausted_penalty() {
        assert_close(
            speed_multiplier(Realm::Awaken, MovementZoneKind::Normal, 0, 9.0),
            0.45,
        );
    }

    #[test]
    fn dash_distance_4_blocks() {
        let origin = DVec3::new(1.0, 64.0, 2.0);
        let end = movement_displacement(origin, DVec3::new(1.0, 0.0, 0.0), DASH_DISTANCE_BLOCKS);
        assert_eq!(end, DVec3::new(5.0, 64.0, 2.0));
    }

    #[test]
    fn dash_stamina_cost() {
        assert_close(movement_action_cost(MovementAction::Dashing), 15.0);
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
    fn slide_hitbox_reduction() {
        assert_close(slide_hitbox_height(MovementAction::Sliding), 1.0);
        assert_close(slide_hitbox_height(MovementAction::None), 1.8);
    }

    #[test]
    fn slide_contact_damage() {
        let mut wounds = Wounds::default();
        let damage = apply_slide_contact_damage(&mut wounds, 42);
        assert_close(damage, 8.0);
        assert_close(wounds.health_current, 92.0);
        assert_eq!(wounds.entries.len(), 1);
        assert_eq!(wounds.entries[0].kind, WoundKind::Blunt);
        assert_eq!(wounds.entries[0].created_at_tick, 42);
    }

    #[test]
    fn slide_contact_requires_vertical_overlap() {
        let player = DVec3::new(0.0, 64.0, 0.0);

        assert!(slide_contact_in_range(
            player,
            DVec3::new(0.9, 64.4, 0.0),
            SLIDE_HITBOX_HEIGHT_BLOCKS,
        ));
        assert!(!slide_contact_in_range(
            player,
            DVec3::new(0.9, 66.0, 0.0),
            SLIDE_HITBOX_HEIGHT_BLOCKS,
        ));
    }

    #[test]
    fn slide_requires_running() {
        assert!(super::slide_requires_running(StaminaState::Sprinting));
        assert!(!super::slide_requires_running(StaminaState::Walking));
    }

    #[test]
    fn slide_to_stand_transition() {
        assert_eq!(slide_stand_transition_end(10), 24);
    }

    #[test]
    fn double_jump_only_airborne() {
        assert!(double_jump_allowed(false, 1));
        assert!(!double_jump_allowed(true, 1));
        assert!(!double_jump_allowed(false, 0));
    }

    #[test]
    fn double_jump_resets_on_land() {
        let mut state = MovementState {
            double_jump_charges_max: 2,
            double_jump_charges_remaining: 0,
            ..Default::default()
        };
        state.last_grounded = true;
        state.double_jump_charges_remaining = double_jump_remaining_after_grounded(
            state.last_grounded,
            state.double_jump_charges_remaining,
            state.double_jump_charges_max,
        );
        assert_eq!(state.double_jump_charges_remaining, 2);
    }

    #[test]
    fn double_jump_airborne_does_not_refill_charges() {
        assert_eq!(double_jump_remaining_after_grounded(false, 0, 2), 0);
        assert_eq!(double_jump_remaining_after_grounded(false, 3, 2), 2);
    }

    #[test]
    fn double_jump_direction_change() {
        let current = DVec3::new(1.0, 0.0, 0.0);
        let requested = DVec3::new(0.0, 0.0, 1.0);
        let turned = double_jump_direction_after_air_turn(current, requested);
        let angle = normalize_horizontal(current)
            .dot(turned)
            .acos()
            .to_degrees();
        assert!(
            angle <= 45.0001,
            "air turn must clamp to 45 degrees, got {angle}"
        );
    }

    #[test]
    fn swept_displacement_stops_before_first_collision() {
        let origin = DVec3::new(0.0, 64.0, 0.0);
        let end = movement_displacement_swept(
            origin,
            DVec3::new(1.0, 0.0, 0.0),
            DASH_DISTANCE_BLOCKS,
            1.8,
            |candidate, _height| candidate.x >= 2.0,
        );

        assert!(
            end.x < 2.0,
            "movement should stop before blocking contact: {end:?}"
        );
        assert!(
            end.x > 1.5,
            "movement should advance to the last safe sample: {end:?}"
        );
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
    fn tongling_gets_2_charges() {
        assert_eq!(double_jump_charges_by_realm(Realm::Solidify), 1);
        assert_eq!(double_jump_charges_by_realm(Realm::Spirit), 2);
        assert_eq!(double_jump_charges_by_realm(Realm::Void), 2);
    }

    #[test]
    fn movement_matrix_covers_3_actions_6_realms_4_zones_2_stamina_states() {
        let actions = [
            MovementAction::Dashing,
            MovementAction::Sliding,
            MovementAction::DoubleJumping,
        ];
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
                        let speed = speed_multiplier(realm, zone, 123, stamina_current);
                        assert!(speed > 0.0, "{action:?} {realm:?} {zone:?}");
                        covered += 1;
                    }
                }
            }
        }
        assert_eq!(covered, 144);
    }
}
