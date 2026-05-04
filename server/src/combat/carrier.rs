use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use valence::prelude::{
    bevy_ecs, App, Commands, DVec3, Entity, Event, EventReader, EventWriter, IntoSystemConfigs,
    Position, Query, Res, UniqueId, Update, With, Without,
};

use crate::combat::components::{
    BodyPart, Lifecycle, LifecycleState, Stamina, Wound, WoundKind, Wounds, TICKS_PER_SECOND,
};
use crate::combat::decay::{hit_qi_ratio, CarrierGrade};
use crate::combat::events::CombatEvent;
use crate::combat::projectile::{
    residual_qi_after_miss, segment_point_distance, AnqiProjectileFlight, ProjectileDespawnReason,
    QiProjectile,
};
use crate::combat::{CombatClock, CombatSystemSet};
use crate::cultivation::components::{
    ColorKind, ContamSource, Contamination, Cultivation, QiColor, Realm,
};
use crate::cultivation::life_record::{BiographyEntry, LifeRecord};
use crate::cultivation::skill_registry::{CastRejectReason, CastResult, SkillRegistry};
use crate::inventory::{
    bump_revision, ItemInstance, ItemRegistry, PlayerInventory, EQUIP_SLOT_MAIN_HAND,
    EQUIP_SLOT_OFF_HAND,
};

pub const ANQI_CHARGE_SKILL_ID: &str = "anqi.charge_carrier";
pub const ANQI_MATERIAL_TEMPLATE_ID: &str = "anqi_yibian_shougu";
pub const ANQI_CHARGED_TEMPLATE_ID: &str = "anqi_yibian_shougu_charged";
pub const CHARGE_DURATION_TICKS: u64 = 20 * TICKS_PER_SECOND;
pub const ANQI_THROW_STAMINA_COST: f32 = 5.0;
pub const ANQI_PROJECTILE_MAX_DISTANCE: f32 = 80.0;
pub const ANQI_HITBOX_INFLATION: f32 = 0.4;
pub const NATURAL_DECAY_BREAK_RATIO: f32 = 0.05;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CarrierSlot {
    MainHand,
    OffHand,
}

impl CarrierSlot {
    pub fn equip_key(self) -> &'static str {
        match self {
            Self::MainHand => EQUIP_SLOT_MAIN_HAND,
            Self::OffHand => EQUIP_SLOT_OFF_HAND,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BondKind {
    HandheldCarrier,
    EmbeddedTrap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CarrierKind {
    YibianShougu,
}

impl CarrierKind {
    pub const fn grade(self) -> CarrierGrade {
        match self {
            Self::YibianShougu => CarrierGrade::Beast,
        }
    }

    pub const fn half_life_min(self) -> f32 {
        match self {
            Self::YibianShougu => 120.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CarrierImprint {
    pub qi_amount: f32,
    pub qi_amount_initial: f32,
    pub qi_color: ColorKind,
    pub source_realm: Realm,
    pub half_life_min: f32,
    pub decay_started_at_tick: u64,
    pub bond_kind: BondKind,
}

#[derive(Debug, Clone, PartialEq, Default, bevy_ecs::component::Component)]
pub struct CarrierStore {
    pub imprints_by_instance: HashMap<u64, CarrierImprint>,
}

#[derive(Debug, Clone, Copy, PartialEq, bevy_ecs::component::Component)]
pub struct CarrierCharging {
    pub slot: CarrierSlot,
    pub instance_id: u64,
    pub qi_target: f32,
    pub prepaid_qi: f32,
    pub started_at_tick: u64,
    pub start_pos: DVec3,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct ChargeCarrierIntent {
    pub carrier: Entity,
    pub slot: Option<CarrierSlot>,
    pub qi_target: Option<f32>,
    pub issued_at_tick: u64,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct ThrowCarrierIntent {
    pub thrower: Entity,
    pub slot: CarrierSlot,
    pub dir_unit: [f32; 3],
    pub power: f32,
    pub issued_at_tick: u64,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct CarrierChargedEvent {
    pub carrier: Entity,
    pub instance_id: u64,
    pub qi_amount: f32,
    pub qi_color: ColorKind,
    pub full_charge: bool,
    pub tick: u64,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct CarrierImpactEvent {
    pub attacker: Entity,
    pub target: Entity,
    pub carrier_kind: CarrierKind,
    pub hit_distance: f32,
    pub sealed_qi_initial: f32,
    pub hit_qi: f32,
    pub wound_damage: f32,
    pub contam_amount: f32,
    pub tick: u64,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct ProjectileDespawnedEvent {
    pub owner: Option<Entity>,
    pub projectile: Entity,
    pub reason: ProjectileDespawnReason,
    pub distance: f32,
    pub qi_evaporated: f32,
    pub residual_qi: f32,
    pub pos: [f64; 3],
    pub tick: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InjectProfile {
    pub wound_ratio: f32,
    pub contam_ratio: f32,
}

pub fn register(app: &mut App) {
    app.add_event::<ChargeCarrierIntent>();
    app.add_event::<ThrowCarrierIntent>();
    app.add_event::<CarrierChargedEvent>();
    app.add_event::<CarrierImpactEvent>();
    app.add_event::<ProjectileDespawnedEvent>();
    app.add_systems(
        Update,
        (
            begin_charge_carrier.in_set(CombatSystemSet::Intent),
            charge_carrier_tick.in_set(CombatSystemSet::Physics),
            carry_decay_tick.in_set(CombatSystemSet::Physics),
            throw_carrier_intents.in_set(CombatSystemSet::Intent),
            projectile_tick_system.in_set(CombatSystemSet::Resolve),
        ),
    );
}

pub fn register_skills(registry: &mut SkillRegistry) {
    registry.register(ANQI_CHARGE_SKILL_ID, resolve_anqi_charge_skill);
}

pub fn resolve_anqi_charge_skill(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    _target: Option<Entity>,
) -> CastResult {
    let now_tick = world
        .get_resource::<CombatClock>()
        .map(|clock| clock.tick)
        .unwrap_or_default();
    if world
        .get::<crate::combat::components::SkillBarBindings>(caster)
        .is_some_and(|bindings| bindings.is_on_cooldown(slot, now_tick))
    {
        return CastResult::Rejected {
            reason: CastRejectReason::OnCooldown,
        };
    }

    let qi_target = world
        .get::<Cultivation>(caster)
        .map(default_qi_target)
        .unwrap_or(0.0);
    if qi_target <= f32::EPSILON {
        return CastResult::Rejected {
            reason: CastRejectReason::QiInsufficient,
        };
    }
    let Some(mut events) = world.get_resource_mut::<bevy_ecs::event::Events<ChargeCarrierIntent>>()
    else {
        return CastResult::Rejected {
            reason: CastRejectReason::InvalidTarget,
        };
    };
    events.send(ChargeCarrierIntent {
        carrier: caster,
        slot: None,
        qi_target: Some(qi_target),
        issued_at_tick: now_tick,
    });
    if let Some(mut bindings) = world.get_mut::<crate::combat::components::SkillBarBindings>(caster)
    {
        bindings.set_cooldown(slot, now_tick.saturating_add(CHARGE_DURATION_TICKS));
    }
    CastResult::Started {
        cooldown_ticks: CHARGE_DURATION_TICKS,
        anim_duration_ticks: CHARGE_DURATION_TICKS as u32,
    }
}

pub fn default_qi_target(cultivation: &Cultivation) -> f32 {
    ((cultivation.qi_max as f32) * 0.3).min(80.0)
}

type BeginChargeActor<'a> = (
    Entity,
    &'a mut Cultivation,
    Option<&'a QiColor>,
    Option<&'a Lifecycle>,
    &'a Position,
    &'a PlayerInventory,
    Option<&'a CarrierCharging>,
);

type ChargingActor<'a> = (
    Entity,
    &'a mut Cultivation,
    Option<&'a QiColor>,
    &'a Position,
    &'a mut PlayerInventory,
    &'a mut CarrierStore,
    &'a CarrierCharging,
);

pub fn anqi_carrier_profile(kind: CarrierKind) -> InjectProfile {
    match kind {
        CarrierKind::YibianShougu => InjectProfile {
            wound_ratio: 0.5,
            contam_ratio: 0.5,
        },
    }
}

fn begin_charge_carrier(
    clock: Res<CombatClock>,
    mut intents: EventReader<ChargeCarrierIntent>,
    mut commands: Commands,
    mut actors: Query<BeginChargeActor<'_>>,
) {
    for intent in intents.read() {
        let Ok((entity, mut cultivation, _qi_color, lifecycle, position, inventory, charging)) =
            actors.get_mut(intent.carrier)
        else {
            continue;
        };
        if charging.is_some() || !lifecycle_allows_charge(lifecycle) {
            continue;
        }
        let qi_target = intent
            .qi_target
            .unwrap_or_else(|| default_qi_target(&cultivation));
        if qi_target <= 0.0 || qi_target > default_qi_target(&cultivation) + f32::EPSILON {
            continue;
        }
        if cultivation.qi_current + f64::EPSILON < f64::from(qi_target) {
            continue;
        }
        let Some((slot, item)) = find_chargeable_hand(inventory, intent.slot) else {
            continue;
        };
        let prepaid = qi_target * 0.5;
        cultivation.qi_current =
            (cultivation.qi_current - f64::from(prepaid)).clamp(0.0, cultivation.qi_max);
        commands.entity(entity).insert(CarrierCharging {
            slot,
            instance_id: item.instance_id,
            qi_target,
            prepaid_qi: prepaid,
            started_at_tick: intent.issued_at_tick.max(clock.tick),
            start_pos: position.get(),
        });
    }
}

fn lifecycle_allows_charge(lifecycle: Option<&Lifecycle>) -> bool {
    !lifecycle.is_some_and(|lifecycle| {
        matches!(
            lifecycle.state,
            LifecycleState::NearDeath | LifecycleState::Terminated
        )
    })
}

fn find_chargeable_hand(
    inventory: &PlayerInventory,
    requested: Option<CarrierSlot>,
) -> Option<(CarrierSlot, &ItemInstance)> {
    let slots = match requested {
        Some(CarrierSlot::MainHand) => &[CarrierSlot::MainHand][..],
        Some(CarrierSlot::OffHand) => &[CarrierSlot::OffHand][..],
        None => &[CarrierSlot::MainHand, CarrierSlot::OffHand][..],
    };
    slots.iter().find_map(|slot| {
        let item = inventory.equipped.get(slot.equip_key())?;
        (item.template_id == ANQI_MATERIAL_TEMPLATE_ID
            || item.template_id == ANQI_CHARGED_TEMPLATE_ID)
            .then_some((*slot, item))
    })
}

fn charge_carrier_tick(
    clock: Res<CombatClock>,
    registry: Res<ItemRegistry>,
    mut commands: Commands,
    mut actors: Query<ChargingActor<'_>>,
    mut events: EventWriter<CarrierChargedEvent>,
) {
    for (entity, mut cultivation, qi_color, position, mut inventory, mut store, charging) in
        &mut actors
    {
        let moved_too_far = position.get().distance(charging.start_pos) > 1.0;
        let elapsed = clock.tick.saturating_sub(charging.started_at_tick);
        if moved_too_far {
            finish_charge(
                &registry,
                &mut commands,
                entity,
                &mut inventory,
                &mut store,
                charging,
                qi_color,
                &cultivation,
                clock.tick,
                false,
                (elapsed as f32 / CHARGE_DURATION_TICKS as f32).clamp(0.0, 1.0),
                &mut events,
            );
            continue;
        }
        if elapsed < CHARGE_DURATION_TICKS {
            continue;
        }
        let remaining = charging.qi_target - charging.prepaid_qi;
        if cultivation.qi_current + f64::EPSILON < f64::from(remaining) {
            continue;
        }
        cultivation.qi_current =
            (cultivation.qi_current - f64::from(remaining)).clamp(0.0, cultivation.qi_max);
        finish_charge(
            &registry,
            &mut commands,
            entity,
            &mut inventory,
            &mut store,
            charging,
            qi_color,
            &cultivation,
            clock.tick,
            true,
            1.0,
            &mut events,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn finish_charge(
    registry: &ItemRegistry,
    commands: &mut Commands,
    entity: Entity,
    inventory: &mut PlayerInventory,
    store: &mut CarrierStore,
    charging: &CarrierCharging,
    qi_color: Option<&QiColor>,
    cultivation: &Cultivation,
    tick: u64,
    full_charge: bool,
    progress_ratio: f32,
    events: &mut EventWriter<CarrierChargedEvent>,
) {
    let qi_amount = if full_charge {
        charging.qi_target
    } else {
        charging.qi_target * progress_ratio * 0.5
    };
    if qi_amount <= f32::EPSILON {
        commands.entity(entity).remove::<CarrierCharging>();
        return;
    }
    if transform_equipped_item(inventory, registry, charging.slot, ANQI_CHARGED_TEMPLATE_ID) {
        store.imprints_by_instance.insert(
            charging.instance_id,
            CarrierImprint {
                qi_amount,
                qi_amount_initial: qi_amount,
                qi_color: qi_color
                    .map(|color| color.main)
                    .unwrap_or(ColorKind::Mellow),
                source_realm: cultivation.realm,
                half_life_min: CarrierKind::YibianShougu.half_life_min(),
                decay_started_at_tick: tick,
                bond_kind: BondKind::HandheldCarrier,
            },
        );
        events.send(CarrierChargedEvent {
            carrier: entity,
            instance_id: charging.instance_id,
            qi_amount,
            qi_color: qi_color
                .map(|color| color.main)
                .unwrap_or(ColorKind::Mellow),
            full_charge,
            tick,
        });
    }
    commands.entity(entity).remove::<CarrierCharging>();
}

fn transform_equipped_item(
    inventory: &mut PlayerInventory,
    registry: &ItemRegistry,
    slot: CarrierSlot,
    template_id: &str,
) -> bool {
    let Some(template) = registry.get(template_id) else {
        return false;
    };
    let Some(item) = inventory.equipped.get_mut(slot.equip_key()) else {
        return false;
    };
    item.template_id = template.id.clone();
    item.display_name = template.display_name.clone();
    item.grid_w = template.grid_w;
    item.grid_h = template.grid_h;
    item.weight = template.base_weight;
    item.rarity = template.rarity;
    item.description = template.description.clone();
    item.stack_count = item.stack_count.min(template.max_stack_count).max(1);
    item.spirit_quality = template.spirit_quality_initial;
    bump_revision(inventory);
    true
}

fn carry_decay_tick(
    clock: Res<CombatClock>,
    registry: Res<ItemRegistry>,
    mut actors: Query<(&mut PlayerInventory, &mut CarrierStore)>,
) {
    if !clock.tick.is_multiple_of(TICKS_PER_SECOND) {
        return;
    }
    for (mut inventory, mut store) in &mut actors {
        let mut expired = Vec::new();
        for (instance_id, imprint) in &mut store.imprints_by_instance {
            if imprint.bond_kind != BondKind::HandheldCarrier {
                continue;
            }
            let elapsed_min = clock.tick.saturating_sub(imprint.decay_started_at_tick) as f32
                / TICKS_PER_SECOND as f32
                / 60.0;
            let half_lives = elapsed_min / imprint.half_life_min.max(0.001);
            imprint.qi_amount = imprint.qi_amount_initial * 0.5_f32.powf(half_lives);
            if imprint.qi_amount / imprint.qi_amount_initial.max(f32::EPSILON)
                < NATURAL_DECAY_BREAK_RATIO
            {
                expired.push(*instance_id);
            }
        }
        for instance_id in expired {
            store.imprints_by_instance.remove(&instance_id);
            degrade_equipped_instance(&mut inventory, &registry, instance_id);
        }
    }
}

fn degrade_equipped_instance(
    inventory: &mut PlayerInventory,
    registry: &ItemRegistry,
    instance_id: u64,
) -> bool {
    let Some(slot) = [CarrierSlot::MainHand, CarrierSlot::OffHand]
        .into_iter()
        .find(|slot| {
            inventory
                .equipped
                .get(slot.equip_key())
                .is_some_and(|item| item.instance_id == instance_id)
        })
    else {
        return false;
    };
    transform_equipped_item(inventory, registry, slot, ANQI_MATERIAL_TEMPLATE_ID)
}

fn throw_carrier_intents(
    clock: Res<CombatClock>,
    mut commands: Commands,
    mut intents: EventReader<ThrowCarrierIntent>,
    mut actors: Query<(
        &Position,
        &mut PlayerInventory,
        &mut CarrierStore,
        Option<&mut Stamina>,
    )>,
) {
    for intent in intents.read() {
        let Ok((position, mut inventory, mut store, stamina)) = actors.get_mut(intent.thrower)
        else {
            continue;
        };
        let Some(item) = inventory.equipped.get(intent.slot.equip_key()) else {
            continue;
        };
        let Some(imprint) = store.imprints_by_instance.remove(&item.instance_id) else {
            continue;
        };
        let dir = normalized_dir(intent.dir_unit);
        if dir.length_squared() <= f64::EPSILON {
            continue;
        }
        if let Some(mut stamina) = stamina {
            if stamina.current + f32::EPSILON < ANQI_THROW_STAMINA_COST {
                continue;
            }
            stamina.current = (stamina.current - ANQI_THROW_STAMINA_COST).clamp(0.0, stamina.max);
            stamina.last_drain_tick = Some(clock.tick.max(intent.issued_at_tick));
        }
        inventory.equipped.remove(intent.slot.equip_key());
        bump_revision(&mut inventory);

        let spawn_pos = position.get() + DVec3::new(0.0, 1.62, 0.0) + dir * 0.5;
        let speed = 60.0 + 30.0 * f64::from(intent.power.clamp(0.0, 1.0));
        commands.spawn((
            Position::new(spawn_pos),
            QiProjectile {
                owner: Some(intent.thrower),
                qi_payload: imprint.qi_amount,
            },
            AnqiProjectileFlight {
                carrier_kind: CarrierKind::YibianShougu,
                qi_color: imprint.qi_color,
                carrier_grade: CarrierKind::YibianShougu.grade(),
                spawn_pos,
                prev_pos: spawn_pos,
                velocity: dir * speed,
                max_distance: ANQI_PROJECTILE_MAX_DISTANCE,
                hitbox_inflation: ANQI_HITBOX_INFLATION,
            },
        ));
    }
}

fn normalized_dir(dir: [f32; 3]) -> DVec3 {
    let raw = DVec3::new(f64::from(dir[0]), f64::from(dir[1]), f64::from(dir[2]));
    if raw.length_squared() <= f64::EPSILON {
        DVec3::ZERO
    } else {
        raw.normalize()
    }
}

type ProjectileItem<'a> = (
    Entity,
    &'a mut Position,
    &'a mut QiProjectile,
    &'a mut AnqiProjectileFlight,
);
type TargetItem<'a> = (
    Entity,
    &'a Position,
    &'a mut Wounds,
    &'a mut Contamination,
    Option<&'a mut LifeRecord>,
);

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn projectile_tick_system(
    clock: Res<CombatClock>,
    mut commands: Commands,
    mut projectiles: Query<ProjectileItem<'_>>,
    mut targets: Query<TargetItem<'_>, (With<Wounds>, Without<AnqiProjectileFlight>)>,
    unique_ids: Query<&UniqueId>,
    mut combat_events: EventWriter<CombatEvent>,
    mut impacts: EventWriter<CarrierImpactEvent>,
    mut despawned: EventWriter<ProjectileDespawnedEvent>,
) {
    let dt = 1.0 / TICKS_PER_SECOND as f64;
    for (projectile_entity, mut position, mut projectile, mut flight) in &mut projectiles {
        if projectile.qi_payload <= f32::EPSILON {
            emit_projectile_despawn(
                &mut commands,
                &mut despawned,
                ProjectileDespawnArgs {
                    projectile_entity,
                    projectile: &projectile,
                    flight: &flight,
                    reason: ProjectileDespawnReason::NaturalDecay,
                    pos: position.get(),
                    tick: clock.tick,
                },
            );
            continue;
        }

        let current = position.get();
        let next = current + flight.velocity * dt;
        let traveled = next.distance(flight.spawn_pos) as f32;
        if traveled > flight.max_distance {
            emit_projectile_despawn(
                &mut commands,
                &mut despawned,
                ProjectileDespawnArgs {
                    projectile_entity,
                    projectile: &projectile,
                    flight: &flight,
                    reason: ProjectileDespawnReason::OutOfRange,
                    pos: next,
                    tick: clock.tick,
                },
            );
            continue;
        }

        let mut hit: Option<(Entity, f32)> = None;
        for (target_entity, target_pos, _, _, _) in &mut targets {
            if projectile.owner == Some(target_entity) {
                continue;
            }
            let distance_to_segment =
                segment_point_distance(current, next, target_pos.get() + DVec3::new(0.0, 1.0, 0.0));
            if distance_to_segment <= f64::from(0.3 + flight.hitbox_inflation) {
                hit = Some((
                    target_entity,
                    target_pos.get().distance(flight.spawn_pos) as f32,
                ));
                break;
            }
        }

        if let Some((target_entity, hit_distance)) = hit {
            let Ok((_, _, mut wounds, mut contamination, life_record)) =
                targets.get_mut(target_entity)
            else {
                continue;
            };
            let ratio = hit_qi_ratio(hit_distance, flight.qi_color, flight.carrier_grade);
            let hit_qi = projectile.qi_payload * ratio;
            if hit_qi <= f32::EPSILON {
                emit_projectile_despawn(
                    &mut commands,
                    &mut despawned,
                    ProjectileDespawnArgs {
                        projectile_entity,
                        projectile: &projectile,
                        flight: &flight,
                        reason: ProjectileDespawnReason::HitTarget,
                        pos: next,
                        tick: clock.tick,
                    },
                );
                continue;
            }
            let profile = anqi_carrier_profile(flight.carrier_kind);
            let wound_damage = hit_qi * profile.wound_ratio;
            let contam_amount = hit_qi * profile.contam_ratio;
            let attacker_id = projectile
                .owner
                .map(|owner| entity_wire_id(unique_ids.get(owner).ok(), owner))
                .unwrap_or_else(|| "entity:unknown".to_string());
            wounds.health_current =
                (wounds.health_current - wound_damage).clamp(0.0, wounds.health_max);
            wounds.entries.push(Wound {
                location: BodyPart::Chest,
                kind: WoundKind::Pierce,
                severity: wound_damage,
                bleeding_per_sec: wound_damage * 0.05,
                created_at_tick: clock.tick,
                inflicted_by: Some(attacker_id.clone()),
            });
            contamination.entries.push(ContamSource {
                amount: f64::from(contam_amount),
                color: flight.qi_color,
                attacker_id: Some(attacker_id.clone()),
                introduced_at: clock.tick,
            });
            if let Some(mut life_record) = life_record {
                life_record.push(BiographyEntry::AnqiSniped {
                    attacker_id: attacker_id.clone(),
                    distance_blocks: hit_distance,
                    sealed_qi: projectile.qi_payload,
                    hit_qi,
                    tick: clock.tick,
                });
            }
            let attacker = projectile.owner.unwrap_or(projectile_entity);
            combat_events.send(CombatEvent {
                attacker,
                target: target_entity,
                resolved_at_tick: clock.tick,
                body_part: BodyPart::Chest,
                wound_kind: WoundKind::Pierce,
                damage: wound_damage,
                contam_delta: f64::from(contam_amount),
                description: format!(
                    "anqi_carrier {attacker_id} -> entity:{} hit at {:.1} blocks (hit_qi {:.1})",
                    target_entity.to_bits(),
                    hit_distance,
                    hit_qi
                ),
                defense_kind: None,
                defense_effectiveness: None,
                defense_contam_reduced: None,
                defense_wound_severity: None,
            });
            impacts.send(CarrierImpactEvent {
                attacker,
                target: target_entity,
                carrier_kind: flight.carrier_kind,
                hit_distance,
                sealed_qi_initial: projectile.qi_payload,
                hit_qi,
                wound_damage,
                contam_amount,
                tick: clock.tick,
            });
            projectile.qi_payload = 0.0;
            emit_projectile_despawn(
                &mut commands,
                &mut despawned,
                ProjectileDespawnArgs {
                    projectile_entity,
                    projectile: &projectile,
                    flight: &flight,
                    reason: ProjectileDespawnReason::HitTarget,
                    pos: next,
                    tick: clock.tick,
                },
            );
            continue;
        }

        flight.prev_pos = current;
        position.set(next);
    }
}

struct ProjectileDespawnArgs<'a> {
    projectile_entity: Entity,
    projectile: &'a QiProjectile,
    flight: &'a AnqiProjectileFlight,
    reason: ProjectileDespawnReason,
    pos: DVec3,
    tick: u64,
}

fn emit_projectile_despawn(
    commands: &mut Commands,
    despawned: &mut EventWriter<ProjectileDespawnedEvent>,
    args: ProjectileDespawnArgs<'_>,
) {
    let distance = args.pos.distance(args.flight.spawn_pos) as f32;
    let qi_at_despawn = args.projectile.qi_payload
        * hit_qi_ratio(distance, args.flight.qi_color, args.flight.carrier_grade);
    let (qi_evaporated, residual_qi) = if args.reason == ProjectileDespawnReason::HitTarget {
        (qi_at_despawn, 0.0)
    } else {
        residual_qi_after_miss(qi_at_despawn)
    };
    despawned.send(ProjectileDespawnedEvent {
        owner: args.projectile.owner,
        projectile: args.projectile_entity,
        reason: args.reason,
        distance,
        qi_evaporated,
        residual_qi,
        pos: [args.pos.x, args.pos.y, args.pos.z],
        tick: args.tick,
    });
    commands.entity(args.projectile_entity).despawn();
}

fn entity_wire_id(unique_id: Option<&UniqueId>, entity: Entity) -> String {
    crate::combat::woliu::entity_wire_id(unique_id, entity)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{InventoryRevision, ItemCategory, ItemRarity, ItemTemplate, WeaponSpec};

    fn template(id: &str, name: &str, max_stack_count: u32) -> ItemTemplate {
        ItemTemplate {
            id: id.to_string(),
            display_name: name.to_string(),
            category: ItemCategory::Misc,
            max_stack_count,
            grid_w: 1,
            grid_h: 1,
            base_weight: 0.2,
            rarity: ItemRarity::Uncommon,
            spirit_quality_initial: 1.0,
            description: name.to_string(),
            effect: None,
            cast_duration_ms: 0,
            cooldown_ms: 0,
            weapon_spec: None::<WeaponSpec>,
            forge_station_spec: None,
            blueprint_scroll_spec: None,
            inscription_scroll_spec: None,
        }
    }

    fn registry() -> ItemRegistry {
        ItemRegistry::from_map(HashMap::from([
            (
                ANQI_MATERIAL_TEMPLATE_ID.to_string(),
                template(ANQI_MATERIAL_TEMPLATE_ID, "异变兽骨", 16),
            ),
            (
                ANQI_CHARGED_TEMPLATE_ID.to_string(),
                template(ANQI_CHARGED_TEMPLATE_ID, "封元异变兽骨", 1),
            ),
        ]))
    }

    fn item(instance_id: u64, template_id: &str) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: template_id.to_string(),
            display_name: template_id.to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.2,
            rarity: ItemRarity::Uncommon,
            description: template_id.to_string(),
            stack_count: 1,
            spirit_quality: 1.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
        }
    }

    fn inventory_with_main_hand(template_id: &str) -> PlayerInventory {
        let mut equipped = HashMap::new();
        equipped.insert(EQUIP_SLOT_MAIN_HAND.to_string(), item(7, template_id));
        PlayerInventory {
            revision: InventoryRevision(1),
            containers: Vec::new(),
            equipped,
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 45.0,
        }
    }

    #[test]
    fn default_qi_target_caps_at_thirty_percent_or_eighty() {
        assert_eq!(
            default_qi_target(&Cultivation {
                qi_max: 150.0,
                ..Default::default()
            }),
            45.0
        );
        assert_eq!(
            default_qi_target(&Cultivation {
                qi_max: 540.0,
                ..Default::default()
            }),
            80.0
        );
    }

    #[test]
    fn transform_charged_carrier_is_non_stackable_and_bumps_revision() {
        let registry = registry();
        let mut inventory = inventory_with_main_hand(ANQI_MATERIAL_TEMPLATE_ID);

        assert!(transform_equipped_item(
            &mut inventory,
            &registry,
            CarrierSlot::MainHand,
            ANQI_CHARGED_TEMPLATE_ID
        ));

        let item = inventory.equipped.get(EQUIP_SLOT_MAIN_HAND).unwrap();
        assert_eq!(item.template_id, ANQI_CHARGED_TEMPLATE_ID);
        assert_eq!(item.stack_count, 1);
        assert_eq!(inventory.revision.0, 2);
    }

    #[test]
    fn natural_decay_uses_half_life_curve() {
        let mut imprint = CarrierImprint {
            qi_amount: 40.0,
            qi_amount_initial: 40.0,
            qi_color: ColorKind::Solid,
            source_realm: Realm::Condense,
            half_life_min: 120.0,
            decay_started_at_tick: 0,
            bond_kind: BondKind::HandheldCarrier,
        };
        let elapsed_min = 120.0;
        let half_lives = elapsed_min / imprint.half_life_min;
        imprint.qi_amount = imprint.qi_amount_initial * 0.5_f32.powf(half_lives);
        assert!((imprint.qi_amount - 20.0).abs() <= 0.001);
    }

    #[test]
    fn profile_splits_yibian_bone_half_wound_half_contam() {
        let profile = anqi_carrier_profile(CarrierKind::YibianShougu);
        assert_eq!(profile.wound_ratio, 0.5);
        assert_eq!(profile.contam_ratio, 0.5);
    }
}
