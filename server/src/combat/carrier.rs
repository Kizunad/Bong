use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use valence::prelude::{
    bevy_ecs, App, Commands, DVec3, Entity, Event, EventReader, EventWriter, GameMode,
    IntoSystemConfigs, Position, Query, Res, ResMut, UniqueId, Update, With, Without,
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
    ColorKind, ContamSource, Contamination, Cultivation, MeridianId, QiColor, Realm,
};
use crate::cultivation::life_record::{BiographyEntry, LifeRecord};
use crate::cultivation::meridian::severed::{
    check_meridian_dependencies, MeridianSeveredPermanent,
};
use crate::cultivation::skill_registry::{CastRejectReason, CastResult, SkillRegistry};
use crate::forge::artifact_meridian::artifact_resonance_for_inventory;
use crate::forge::resonance::carrier_seal_efficiency_multiplier;
use crate::inventory::{
    bump_revision, ItemInstance, ItemRegistry, PlayerInventory, EQUIP_SLOT_MAIN_HAND,
    EQUIP_SLOT_OFF_HAND,
};
use crate::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;
use crate::qi_physics::ledger::{QiAccountId, QiTransfer, QiTransferReason};
use crate::qi_physics::release::qi_release_to_zone;
use crate::world::dimension::DimensionKind;
use crate::world::zone::ZoneRegistry;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CarrierKind {
    BoneChip,
    YibianShougu,
    LingmuArrow,
    DyedBone,
    FenglingheBone,
    ShangguBone,
}

impl CarrierKind {
    pub const fn grade(self) -> CarrierGrade {
        match self {
            Self::BoneChip => CarrierGrade::Bone,
            Self::YibianShougu => CarrierGrade::Beast,
            Self::LingmuArrow => CarrierGrade::Spirit,
            Self::DyedBone | Self::FenglingheBone | Self::ShangguBone => CarrierGrade::Relic,
        }
    }

    pub const fn half_life_min(self) -> f32 {
        match self {
            Self::BoneChip => 45.0,
            Self::YibianShougu => 120.0,
            Self::LingmuArrow => 90.0,
            Self::DyedBone => 180.0,
            Self::FenglingheBone => 240.0,
            Self::ShangguBone => 360.0,
        }
    }

    pub const fn material_template_id(self) -> &'static str {
        match self {
            Self::BoneChip => "anqi_bone_chip",
            Self::YibianShougu => ANQI_MATERIAL_TEMPLATE_ID,
            Self::LingmuArrow => "anqi_lingmu_arrow",
            Self::DyedBone => "anqi_dyed_bone",
            Self::FenglingheBone => "anqi_fenglinghe_bone",
            Self::ShangguBone => "anqi_shanggu_bone",
        }
    }

    pub const fn charged_template_id(self) -> &'static str {
        match self {
            Self::BoneChip => "anqi_bone_chip_charged",
            Self::YibianShougu => ANQI_CHARGED_TEMPLATE_ID,
            Self::LingmuArrow => "anqi_lingmu_arrow_charged",
            Self::DyedBone => "anqi_dyed_bone_charged",
            Self::FenglingheBone => "anqi_fenglinghe_bone_charged",
            Self::ShangguBone => "anqi_shanggu_bone_charged",
        }
    }

    pub fn from_template_id(template_id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| {
            template_id == kind.material_template_id() || template_id == kind.charged_template_id()
        })
    }

    pub const ALL: [Self; 6] = [
        Self::BoneChip,
        Self::YibianShougu,
        Self::LingmuArrow,
        Self::DyedBone,
        Self::FenglingheBone,
        Self::ShangguBone,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InjectionKind {
    Snipe,
    MultiShot,
    SoulInject,
    ArmorPierce,
    EchoFractal,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CarrierImprint {
    pub carrier_kind: CarrierKind,
    pub qi_amount: f32,
    pub qi_amount_initial: f32,
    pub qi_color: ColorKind,
    pub source_realm: Realm,
    pub half_life_min: f32,
    pub decay_started_at_tick: u64,
    pub bond_kind: BondKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub injection_kind: Option<InjectionKind>,
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
            // qc-P0：投射物 miss/expire despawn 残真元守恒 → 落点 zone。
            // 与 Redis 桥接系统（publish_projectile_despawned_events）并行；
            // 须在 Resolve 之后运行（ProjectileDespawnedEvent 由 projectile_tick_system 发出）。
            projectile_miss_qi_release_system
                .in_set(CombatSystemSet::Emit)
                .after(projectile_tick_system),
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

    // plan-meridian-severed-v1 §3 强约束：充能需要肺经（手太阴）导引真元灌注暗器。
    // 断肺经 → 真元无法通过肺经注入充能通道 → 拒绝充能（worldview §四:286）。
    const CHARGE_MERIDIAN_DEPS: &[MeridianId] = &[MeridianId::Lung];
    if let Err(blocked) = check_meridian_dependencies(
        CHARGE_MERIDIAN_DEPS,
        world.get::<MeridianSeveredPermanent>(caster),
    ) {
        return CastResult::Rejected {
            reason: CastRejectReason::MeridianSevered(Some(blocked)),
        };
    }

    let Some(cultivation) = world.get::<Cultivation>(caster) else {
        return CastResult::Rejected {
            reason: CastRejectReason::QiInsufficient,
        };
    };
    let qi_target = default_qi_target(cultivation);
    if qi_target <= f32::EPSILON {
        return CastResult::Rejected {
            reason: CastRejectReason::QiInsufficient,
        };
    }
    // Guard: player must have enough qi_current to cover the charge cost.
    // Without this check, resolve returns Started and sets the cooldown even
    // when the player has qi_current=0; begin_charge_carrier then silently
    // skips (line 403) — burning the cooldown with no charge effect.
    if cultivation.qi_current + f64::EPSILON < f64::from(qi_target) {
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
        CarrierKind::BoneChip => InjectProfile {
            wound_ratio: 0.45,
            contam_ratio: 0.35,
        },
        CarrierKind::YibianShougu => InjectProfile {
            wound_ratio: 0.5,
            contam_ratio: 0.5,
        },
        CarrierKind::LingmuArrow => InjectProfile {
            wound_ratio: 0.55,
            contam_ratio: 0.25,
        },
        CarrierKind::DyedBone => InjectProfile {
            wound_ratio: 0.65,
            contam_ratio: 0.30,
        },
        CarrierKind::FenglingheBone => InjectProfile {
            wound_ratio: 0.75,
            contam_ratio: 0.20,
        },
        CarrierKind::ShangguBone => InjectProfile {
            wound_ratio: 0.70,
            contam_ratio: 0.15,
        },
    }
}

fn begin_charge_carrier(
    clock: Res<CombatClock>,
    mut intents: EventReader<ChargeCarrierIntent>,
    mut commands: Commands,
    mut actors: Query<BeginChargeActor<'_>>,
    mut qi_transfers: EventWriter<QiTransfer>,
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
        let Some((slot, item, _kind)) = find_chargeable_hand(inventory, intent.slot) else {
            continue;
        };
        let prepaid = qi_target * 0.5;
        cultivation.qi_current =
            (cultivation.qi_current - f64::from(prepaid)).clamp(0.0, cultivation.qi_max);
        emit_carrier_channeling_transfer(
            &mut qi_transfers,
            entity,
            item.instance_id,
            f64::from(prepaid),
        );
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
) -> Option<(CarrierSlot, &ItemInstance, CarrierKind)> {
    let slots = match requested {
        Some(CarrierSlot::MainHand) => &[CarrierSlot::MainHand][..],
        Some(CarrierSlot::OffHand) => &[CarrierSlot::OffHand][..],
        None => &[CarrierSlot::MainHand, CarrierSlot::OffHand][..],
    };
    slots.iter().find_map(|slot| {
        let item = inventory
            .equipped
            .get(slot.equip_key())
            .and_then(|s| s.held.as_ref())?;
        let kind = CarrierKind::from_template_id(&item.template_id)?;
        Some((*slot, item, kind))
    })
}

fn charge_carrier_tick(
    clock: Res<CombatClock>,
    registry: Res<ItemRegistry>,
    mut zones: Option<ResMut<ZoneRegistry>>,
    mut commands: Commands,
    mut actors: Query<ChargingActor<'_>>,
    mut events: EventWriter<CarrierChargedEvent>,
    mut qi_transfers: EventWriter<QiTransfer>,
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
                position.get(),
                zones.as_deref_mut(),
                &mut qi_transfers,
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
        emit_carrier_channeling_transfer(
            &mut qi_transfers,
            entity,
            charging.instance_id,
            f64::from(remaining),
        );
        finish_charge(
            &registry,
            &mut commands,
            entity,
            &mut inventory,
            &mut store,
            charging,
            qi_color,
            &cultivation,
            position.get(),
            zones.as_deref_mut(),
            &mut qi_transfers,
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
    position: DVec3,
    zones: Option<&mut ZoneRegistry>,
    qi_transfers: &mut EventWriter<QiTransfer>,
    tick: u64,
    full_charge: bool,
    progress_ratio: f32,
    events: &mut EventWriter<CarrierChargedEvent>,
) {
    let total_deducted = if full_charge {
        charging.qi_target
    } else {
        charging.prepaid_qi
    };
    let base_qi_amount = if full_charge {
        charging.qi_target
    } else {
        charging.qi_target * progress_ratio * 0.5
    };
    let resonance = artifact_resonance_for_inventory(inventory, charging.instance_id, qi_color);
    let qi_amount = carrier_sealed_qi_amount(base_qi_amount, resonance);
    if qi_amount <= f32::EPSILON {
        release_unsealed_carrier_qi(
            zones,
            qi_transfers,
            entity,
            charging.instance_id,
            position,
            f64::from(total_deducted),
        );
        commands.entity(entity).remove::<CarrierCharging>();
        return;
    }
    let carrier_kind = CarrierKind::from_template_id(
        inventory
            .equipped
            .get(charging.slot.equip_key())
            .and_then(|s| s.held.as_ref())
            .map(|item| item.template_id.as_str())
            .unwrap_or_default(),
    )
    .unwrap_or(CarrierKind::YibianShougu);
    let mut sealed_base_qi = 0.0_f32;
    if transform_equipped_item(
        inventory,
        registry,
        charging.slot,
        carrier_kind.charged_template_id(),
    ) {
        sealed_base_qi = base_qi_amount;
        store.imprints_by_instance.insert(
            charging.instance_id,
            CarrierImprint {
                carrier_kind,
                qi_amount,
                qi_amount_initial: qi_amount,
                qi_color: qi_color
                    .map(|color| color.main)
                    .unwrap_or(ColorKind::Mellow),
                source_realm: cultivation.realm,
                half_life_min: carrier_kind.half_life_min(),
                decay_started_at_tick: tick,
                bond_kind: BondKind::HandheldCarrier,
                injection_kind: None,
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
    release_unsealed_carrier_qi(
        zones,
        qi_transfers,
        entity,
        charging.instance_id,
        position,
        f64::from((total_deducted - sealed_base_qi).max(0.0)),
    );
    commands.entity(entity).remove::<CarrierCharging>();
}

fn carrier_qi_account(owner: Entity, instance_id: u64) -> QiAccountId {
    QiAccountId::container(format!("anqi_carrier:{owner:?}:{instance_id}"))
}

fn emit_carrier_channeling_transfer(
    qi_transfers: &mut EventWriter<QiTransfer>,
    owner: Entity,
    instance_id: u64,
    amount: f64,
) {
    if amount <= f64::EPSILON {
        return;
    }
    if let Ok(transfer) = QiTransfer::new(
        QiAccountId::player(format!("entity:{owner:?}")),
        carrier_qi_account(owner, instance_id),
        amount,
        QiTransferReason::Channeling,
    ) {
        qi_transfers.send(transfer);
    }
}

fn release_unsealed_carrier_qi(
    zones: Option<&mut ZoneRegistry>,
    qi_transfers: &mut EventWriter<QiTransfer>,
    owner: Entity,
    instance_id: u64,
    pos: DVec3,
    amount: f64,
) {
    if amount <= f64::EPSILON {
        return;
    }
    release_account_to_zone(
        zones,
        qi_transfers,
        carrier_qi_account(owner, instance_id),
        DimensionKind::Overworld,
        pos,
        amount,
        "anqi_carrier_charge_unsealed",
        instance_id,
    );
}

fn carrier_sealed_qi_amount(base_qi_amount: f32, resonance: Option<f64>) -> f32 {
    base_qi_amount
        * resonance
            .map(carrier_seal_efficiency_multiplier)
            .unwrap_or(1.0)
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
    let Some(item) = inventory
        .equipped
        .get_mut(slot.equip_key())
        .and_then(|s| s.held.as_mut())
    else {
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
                .and_then(|s| s.held.as_ref())
                .is_some_and(|item| item.instance_id == instance_id)
        })
    else {
        return false;
    };
    let material_template = inventory
        .equipped
        .get(slot.equip_key())
        .and_then(|s| s.held.as_ref())
        .and_then(|item| CarrierKind::from_template_id(&item.template_id))
        .unwrap_or(CarrierKind::YibianShougu)
        .material_template_id();
    transform_equipped_item(inventory, registry, slot, material_template)
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
        let Some(item) = inventory
            .equipped
            .get(intent.slot.equip_key())
            .and_then(|s| s.held.as_ref())
        else {
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
        if let Some(s) = inventory.equipped.get_mut(intent.slot.equip_key()) {
            s.held = None;
        }
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
                carrier_kind: imprint.carrier_kind,
                qi_color: imprint.qi_color,
                carrier_grade: imprint.carrier_kind.grade(),
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
    game_modes: Query<&GameMode>,
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
            if !crate::combat::is_damageable(target_entity, &game_modes) {
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
                meridian_id: None,
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
                source: crate::combat::events::AttackSource::Melee,
                debug_command: false,
                physical_damage: 0.0,
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

/// qc-P0：anqi 投射物 miss / OutOfRange / HitBlock / NaturalDecay despawn 时，
/// 把 residual_qi 经 qi_release_to_zone 归还落点 zone。
///
/// HitTarget 分支在 `emit_projectile_despawn` 内已将 `residual_qi` 置为 0.0，
/// 因此此处只需判断 `residual_qi > ε` 即可安全门控，不会重复释放。
///
/// 维度：anqi 投射物目前只存在于主世界（Overworld），无跨维度飞行路径。
pub fn projectile_miss_qi_release_system(
    mut events: EventReader<ProjectileDespawnedEvent>,
    mut zones: ResMut<ZoneRegistry>,
    mut qi_transfers: EventWriter<QiTransfer>,
) {
    for event in events.read() {
        let residual = f64::from(event.residual_qi);
        if residual <= f64::EPSILON {
            continue;
        }
        let pos = DVec3::new(event.pos[0], event.pos[1], event.pos[2]);
        release_residual_to_zone(
            &mut zones,
            &mut qi_transfers,
            DimensionKind::Overworld,
            pos,
            residual,
            "anqi_projectile_miss",
            event.projectile.to_bits(),
        );
    }
}

/// Shared helper: locate the zone at `pos`, apply `qi_release_to_zone`, and emit `QiTransfer`.
/// On zone-not-found or overflow, routes to an overflow account (qi never disappears).
/// This is `pub` so `needle.rs` can reuse the same conservation path without duplicating logic.
pub fn release_residual_to_zone(
    zones: &mut ZoneRegistry,
    qi_transfers: &mut EventWriter<QiTransfer>,
    dim: DimensionKind,
    pos: DVec3,
    residual: f64,
    context: &str,
    entity_bits: u64,
) {
    let from = QiAccountId::player(format!("{context}:entity:{entity_bits}"));
    release_account_to_zone(
        Some(zones),
        qi_transfers,
        from,
        dim,
        pos,
        residual,
        context,
        entity_bits,
    );
}

/// Shared helper for returning qi from an explicit source account to a zone or overflow.
#[allow(clippy::too_many_arguments)]
fn release_account_to_zone(
    zones: Option<&mut ZoneRegistry>,
    qi_transfers: &mut EventWriter<QiTransfer>,
    from: QiAccountId,
    dim: DimensionKind,
    pos: DVec3,
    residual: f64,
    context: &str,
    entity_bits: u64,
) {
    if residual <= f64::EPSILON {
        return;
    }

    // Look up zone name first (immutable borrow), then mutably update.
    let Some(zones) = zones else {
        let overflow_to =
            QiAccountId::overflow(format!("{context}_no_zone_registry:{entity_bits}"));
        if let Ok(t) = QiTransfer::new(from, overflow_to, residual, QiTransferReason::ReleaseToZone)
        {
            qi_transfers.send(t);
        }
        return;
    };

    let zone_name = zones.find_zone(dim, pos).map(|z| z.name.clone());

    if let Some(zone_name) = zone_name {
        let to = QiAccountId::zone(zone_name.clone());
        // Safe: we just found the zone by name, find_zone_mut should succeed.
        if let Some(zone) = zones.find_zone_mut(&zone_name) {
            let zone_current = zone.spirit_qi.max(0.0) * QI_ZONE_UNIT_CAPACITY;
            match qi_release_to_zone(
                residual,
                from.clone(),
                to,
                zone_current,
                QI_ZONE_UNIT_CAPACITY,
            ) {
                Ok(outcome) => {
                    zone.spirit_qi = (outcome.zone_after / QI_ZONE_UNIT_CAPACITY).clamp(-1.0, 1.0);
                    if let Some(t) = outcome.transfer {
                        qi_transfers.send(t);
                    }
                    if outcome.overflow > f64::EPSILON {
                        let overflow_to = QiAccountId::overflow(format!(
                            "{context}_overflow:entity:{entity_bits}"
                        ));
                        if let Ok(t) = QiTransfer::new(
                            from,
                            overflow_to,
                            outcome.overflow,
                            QiTransferReason::ReleaseToZone,
                        ) {
                            qi_transfers.send(t);
                        }
                    }
                }
                Err(err) => {
                    tracing::warn!(
                        ?err,
                        context,
                        entity_bits,
                        residual,
                        "[bong][qc_p0] qi release error; routing to overflow"
                    );
                    let overflow_to = QiAccountId::overflow(format!(
                        "{context}_err_overflow:entity:{entity_bits}"
                    ));
                    if let Ok(t) = QiTransfer::new(
                        from,
                        overflow_to,
                        residual,
                        QiTransferReason::ReleaseToZone,
                    ) {
                        qi_transfers.send(t);
                    }
                }
            }
        } else {
            // find_zone returned Some but find_zone_mut returned None — very unlikely but safe.
            let overflow_to =
                QiAccountId::overflow(format!("{context}_no_mut_zone:entity:{entity_bits}"));
            if let Ok(t) =
                QiTransfer::new(from, overflow_to, residual, QiTransferReason::ReleaseToZone)
            {
                qi_transfers.send(t);
            }
        }
    } else {
        // No zone at despawn position — overflow fallback.
        let overflow_to = QiAccountId::overflow(format!("{context}_no_zone:entity:{entity_bits}"));
        if let Ok(t) = QiTransfer::new(from, overflow_to, residual, QiTransferReason::ReleaseToZone)
        {
            qi_transfers.send(t);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{InventoryRevision, ItemCategory, ItemRarity, ItemTemplate, WeaponSpec};
    use valence::prelude::{App, Events, Position, Update};

    fn template(id: &str, name: &str, max_stack_count: u32) -> ItemTemplate {
        ItemTemplate {
            id: id.to_string(),
            display_name: name.to_string(),
            category: ItemCategory::Misc,
            placeable: None,
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
            technique_scroll_spec: None,
            recipe_fragment_spec: None,
            container_spec: None,
            shelflife_profile: None,
            shield_spec: None,
            shelflife_track: None,
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
            lingering_owner_qi: None,
        }
    }

    fn inventory_with_main_hand(template_id: &str) -> PlayerInventory {
        use crate::inventory::SlotContents;
        let mut equipped = HashMap::new();
        equipped.insert(
            EQUIP_SLOT_MAIN_HAND.to_string(),
            SlotContents::held_single(item(7, template_id)),
        );
        PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(1),
            containers: Vec::new(),
            equipped,
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 45.0,
        }
    }

    fn charge_app() -> App {
        use crate::world::zone::ZoneRegistry;

        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 0 });
        app.insert_resource(registry());
        app.insert_resource(ZoneRegistry::default());
        app.add_event::<ChargeCarrierIntent>();
        app.add_event::<CarrierChargedEvent>();
        app.add_event::<QiTransfer>();
        app.add_systems(Update, (begin_charge_carrier, charge_carrier_tick));
        app
    }

    fn spawn_charge_actor(app: &mut App) -> Entity {
        app.world_mut()
            .spawn((
                Cultivation {
                    qi_current: 100.0,
                    qi_max: 200.0,
                    ..Default::default()
                },
                Position::new([0.0, 66.0, 0.0]),
                inventory_with_main_hand(ANQI_MATERIAL_TEMPLATE_ID),
                CarrierStore::default(),
            ))
            .id()
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

        let item = inventory
            .equipped
            .get(EQUIP_SLOT_MAIN_HAND)
            .unwrap()
            .held
            .as_ref()
            .unwrap();
        assert_eq!(item.template_id, ANQI_CHARGED_TEMPLATE_ID);
        assert_eq!(item.stack_count, 1);
        assert_eq!(inventory.revision.0, 2);
    }

    #[test]
    fn begin_charge_channels_prepaid_qi_into_carrier_account() {
        let mut app = charge_app();
        let actor = spawn_charge_actor(&mut app);

        app.world_mut().send_event(ChargeCarrierIntent {
            carrier: actor,
            slot: Some(CarrierSlot::MainHand),
            qi_target: Some(60.0),
            issued_at_tick: 0,
        });
        app.update();

        let cultivation = app.world().get::<Cultivation>(actor).unwrap();
        assert!((cultivation.qi_current - 70.0).abs() < f64::EPSILON);

        let transfers = app.world().resource::<Events<QiTransfer>>();
        let transfer = transfers
            .iter_current_update_events()
            .find(|transfer| {
                transfer.reason == QiTransferReason::Channeling
                    && transfer.to == carrier_qi_account(actor, 7)
            })
            .expect("暗器开始充能扣 prepaid_qi 后必须把真元封入 carrier container");
        assert_eq!(
            transfer.from,
            QiAccountId::player(format!("entity:{actor:?}"))
        );
        assert!((transfer.amount - 30.0).abs() < f64::EPSILON);
    }

    #[test]
    fn interrupted_charge_releases_unsealed_prepaid_qi_to_zone() {
        let mut app = charge_app();
        let actor = spawn_charge_actor(&mut app);
        app.world_mut()
            .resource_mut::<crate::world::zone::ZoneRegistry>()
            .find_zone_mut("spawn")
            .unwrap()
            .spirit_qi = 0.0;

        app.world_mut().send_event(ChargeCarrierIntent {
            carrier: actor,
            slot: Some(CarrierSlot::MainHand),
            qi_target: Some(60.0),
            issued_at_tick: 0,
        });
        app.update();

        app.world_mut()
            .entity_mut(actor)
            .insert(Position::new([2.0, 66.0, 0.0]));
        app.world_mut().resource_mut::<CombatClock>().tick = CHARGE_DURATION_TICKS / 2;
        app.update();

        assert!(
            app.world().get::<CarrierCharging>(actor).is_none(),
            "移动中断后 CarrierCharging 必须结束"
        );
        let store = app.world().get::<CarrierStore>(actor).unwrap();
        let imprint = store
            .imprints_by_instance
            .get(&7)
            .expect("半程中断应保留已封入暗器的部分真元");
        assert!((imprint.qi_amount - 15.0).abs() < f32::EPSILON);

        let transfers = app.world().resource::<Events<QiTransfer>>();
        let transfer = transfers
            .iter_current_update_events()
            .find(|transfer| {
                transfer.reason == QiTransferReason::ReleaseToZone
                    && transfer.from == carrier_qi_account(actor, 7)
            })
            .expect("移动中断时未封存的 prepaid_qi 必须释放回 zone，不能吞真元");
        assert_eq!(transfer.to, QiAccountId::zone("spawn".to_string()));
        assert!((transfer.amount - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn projectile_hit_despawns_without_damage_or_impact_on_creative_target() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 10 });
        app.add_event::<CombatEvent>();
        app.add_event::<CarrierImpactEvent>();
        app.add_event::<ProjectileDespawnedEvent>();
        app.add_systems(Update, projectile_tick_system);

        app.world_mut().spawn((
            Position::new([0.0, 65.0, 0.0]),
            QiProjectile {
                owner: None,
                qi_payload: 20.0,
            },
            AnqiProjectileFlight {
                carrier_kind: CarrierKind::BoneChip,
                qi_color: ColorKind::Sharp,
                carrier_grade: CarrierKind::BoneChip.grade(),
                spawn_pos: DVec3::new(0.0, 65.0, 0.0),
                prev_pos: DVec3::new(0.0, 65.0, 0.0),
                velocity: DVec3::new(20.0, 0.0, 0.0),
                max_distance: ANQI_PROJECTILE_MAX_DISTANCE,
                hitbox_inflation: ANQI_HITBOX_INFLATION,
            },
        ));
        let target = app
            .world_mut()
            .spawn((
                Position::new([0.5, 64.0, 0.0]),
                Wounds::default(),
                Contamination::default(),
                GameMode::Creative,
            ))
            .id();
        let before = app
            .world()
            .entity(target)
            .get::<Wounds>()
            .unwrap()
            .health_current;

        app.update();

        let wounds = app.world().entity(target).get::<Wounds>().unwrap();
        assert_eq!(wounds.health_current, before);
        assert!(wounds.entries.is_empty());
        assert!(app.world().resource::<Events<CombatEvent>>().is_empty());
        assert!(app
            .world()
            .resource::<Events<CarrierImpactEvent>>()
            .is_empty());
        assert_eq!(
            app.world()
                .resource::<Events<ProjectileDespawnedEvent>>()
                .len(),
            1
        );
    }

    #[test]
    fn natural_decay_uses_half_life_curve() {
        let mut imprint = CarrierImprint {
            carrier_kind: CarrierKind::YibianShougu,
            qi_amount: 40.0,
            qi_amount_initial: 40.0,
            qi_color: ColorKind::Solid,
            source_realm: Realm::Condense,
            half_life_min: 120.0,
            decay_started_at_tick: 0,
            bond_kind: BondKind::HandheldCarrier,
            injection_kind: None,
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

    #[test]
    fn carrier_charge_qi_uses_artifact_resonance_efficiency() {
        assert_eq!(carrier_sealed_qi_amount(50.0, None), 50.0);
        assert!((carrier_sealed_qi_amount(50.0, Some(0.0)) - 40.0).abs() <= 0.001);
        assert!((carrier_sealed_qi_amount(50.0, Some(1.0)) - 60.0).abs() <= 0.001);
    }

    // ── qc-P0 守恒测试：projectile_miss_qi_release_system ──────────────────────────

    /// 辅助：构建带 ZoneRegistry + QiTransfer 事件的 App 并注册 miss-release 系统。
    fn miss_release_app() -> App {
        use crate::qi_physics::ledger::QiTransfer;
        use crate::world::zone::ZoneRegistry;

        let mut app = App::new();
        app.add_event::<ProjectileDespawnedEvent>();
        app.add_event::<QiTransfer>();
        app.insert_resource(ZoneRegistry::default()); // 含默认 spawn zone
        app.add_systems(Update, projectile_miss_qi_release_system);
        app
    }

    fn spawn_entity(app: &mut App) -> Entity {
        app.world_mut().spawn_empty().id()
    }

    fn make_despawn_event(
        projectile: Entity,
        owner: Option<Entity>,
        residual_qi: f32,
        reason: ProjectileDespawnReason,
    ) -> ProjectileDespawnedEvent {
        // spawn zone 在 DEFAULT_SPAWN_BOUNDS_MIN = [-128, 64, -128] 到 [128, 80, 128]
        // 落点 [0, 66, 0] 在 spawn zone 内。
        ProjectileDespawnedEvent {
            owner,
            projectile,
            reason,
            distance: 5.0,
            qi_evaporated: 0.7 * residual_qi / 0.3,
            residual_qi,
            pos: [0.0, 66.0, 0.0],
            tick: 10,
        }
    }

    #[test]
    fn miss_despawn_residual_goes_to_zone_qi_increases() {
        // 期望：OutOfRange despawn，residual_qi=3.0 → spawn zone.spirit_qi 上升，
        // 因为真元从投射物归还到 zone（player cast 时已扣，此处归还 zone）。
        let mut app = miss_release_app();
        let projectile = spawn_entity(&mut app);

        let zone_before = app
            .world()
            .resource::<crate::world::zone::ZoneRegistry>()
            .find_zone_by_name("spawn")
            .unwrap()
            .spirit_qi;

        app.world_mut().send_event(make_despawn_event(
            projectile,
            None,
            3.0, // residual_qi
            ProjectileDespawnReason::OutOfRange,
        ));
        app.update();

        let zone_after = app
            .world()
            .resource::<crate::world::zone::ZoneRegistry>()
            .find_zone_by_name("spawn")
            .unwrap()
            .spirit_qi;

        assert!(
            zone_after > zone_before,
            "期望 miss despawn 后 spawn zone.spirit_qi 上升（真元归还 zone），\
             实际 before={zone_before:.6} after={zone_after:.6}"
        );
    }

    #[test]
    fn hit_target_despawn_does_not_release_to_zone() {
        // 期望：HitTarget despawn residual_qi=0.0（由 carrier.rs:924 保证）→
        // miss-release 系统门控 ε 后不触发 zone 更新。
        let mut app = miss_release_app();
        let projectile = spawn_entity(&mut app);

        let zone_before = app
            .world()
            .resource::<crate::world::zone::ZoneRegistry>()
            .find_zone_by_name("spawn")
            .unwrap()
            .spirit_qi;

        app.world_mut().send_event(make_despawn_event(
            projectile,
            None,
            0.0, // HitTarget 已置 residual_qi=0.0
            ProjectileDespawnReason::HitTarget,
        ));
        app.update();

        let zone_after = app
            .world()
            .resource::<crate::world::zone::ZoneRegistry>()
            .find_zone_by_name("spawn")
            .unwrap()
            .spirit_qi;

        assert_eq!(
            zone_before, zone_after,
            "期望 HitTarget despawn 不改变 zone.spirit_qi（residual=0，无双重释放），\
             实际 before={zone_before:.6} after={zone_after:.6}"
        );
    }

    #[test]
    fn zero_residual_is_noop() {
        // 期望：residual_qi=0 → 不更新 zone，不 emit QiTransfer。
        use crate::qi_physics::ledger::QiTransfer;

        let mut app = miss_release_app();
        let projectile = spawn_entity(&mut app);

        let zone_before = app
            .world()
            .resource::<crate::world::zone::ZoneRegistry>()
            .find_zone_by_name("spawn")
            .unwrap()
            .spirit_qi;

        app.world_mut().send_event(make_despawn_event(
            projectile,
            None,
            0.0,
            ProjectileDespawnReason::NaturalDecay,
        ));
        app.update();

        let zone_after = app
            .world()
            .resource::<crate::world::zone::ZoneRegistry>()
            .find_zone_by_name("spawn")
            .unwrap()
            .spirit_qi;

        assert_eq!(
            zone_before, zone_after,
            "residual=0 时 zone.spirit_qi 应不变（期望 noop），实际改变了"
        );

        let transfers = app.world().resource::<Events<QiTransfer>>();
        assert!(
            transfers.is_empty(),
            "residual=0 时不应 emit QiTransfer，实际 emit 了 {} 条",
            transfers.len()
        );
    }

    #[test]
    fn no_zone_at_position_routes_to_overflow_transfer() {
        // 期望：落点在 spawn zone 范围外（无 zone 覆盖）→
        // 仍 emit QiTransfer（overflow 路径），真元不蒸发。
        use crate::qi_physics::ledger::QiTransfer;

        let mut app = miss_release_app();
        let projectile = spawn_entity(&mut app);

        // 落点 [9999, 66, 9999] 不在任何注册 zone 内
        app.world_mut().send_event(ProjectileDespawnedEvent {
            owner: None,
            projectile,
            reason: ProjectileDespawnReason::OutOfRange,
            distance: 80.0,
            qi_evaporated: 7.0,
            residual_qi: 3.0,
            pos: [9999.0, 66.0, 9999.0],
            tick: 10,
        });
        app.update();

        let transfers = app.world().resource::<Events<QiTransfer>>();
        assert!(
            !transfers.is_empty(),
            "落点无 zone 时仍须 emit overflow QiTransfer（真元不蒸发），实际无 transfer"
        );
    }

    #[test]
    fn conservation_invariant_residual_equals_transfer_total() {
        // 期望：residual_qi = Σ transfer.amount（守恒等式）。
        // zone 有足够容量吸收全部 residual。
        use crate::qi_physics::ledger::QiTransfer;

        let mut app = miss_release_app();
        let projectile = spawn_entity(&mut app);
        let residual: f32 = 5.0;

        app.world_mut().send_event(make_despawn_event(
            projectile,
            None,
            residual,
            ProjectileDespawnReason::HitBlock,
        ));
        app.update();

        let events = app.world().resource::<Events<QiTransfer>>();
        let mut reader = events.get_reader();
        let total: f64 = reader.read(events).map(|t| t.amount).sum();

        assert!(
            (total - f64::from(residual)).abs() < 1e-9,
            "守恒不变式：transfer 总量应等于 residual_qi（期望 {residual}），实际 {total}"
        );
    }

    // ── 经脉门测试：charge_carrier meridian gate ─────────────────────────────────────

    /// 验证 anqi.charge_carrier 在 SkillMeridianDependencies 中已声明肺经依赖。
    /// 断肺经 → charge 被通用 check_meridian_dependencies 拦截（worldview §四:286）。
    #[test]
    fn charge_carrier_declared_in_skill_meridian_dependencies_with_lung() {
        use crate::cultivation::meridian::severed::SkillMeridianDependencies;

        let mut deps = SkillMeridianDependencies::default();
        crate::combat::anqi_v2::declare_meridian_dependencies(&mut deps);

        assert!(
            deps.is_declared(ANQI_CHARGE_SKILL_ID),
            "期望 anqi.charge_carrier 已在 SkillMeridianDependencies 声明（plan-meridian-severed-v1 §3 强约束），\
             实际未声明 → 断肺经的玩家仍可充能"
        );
        let declared = deps.lookup(ANQI_CHARGE_SKILL_ID);
        assert!(
            declared.contains(&MeridianId::Lung),
            "期望 charge_carrier 依赖 MeridianId::Lung（肺经，真元注入暗器的主导引脉），\
             实际声明的依赖为 {declared:?}"
        );
    }

    /// 验证 resolve_anqi_charge_skill 在施法前检查经脉门：肺经 SEVERED → 返回 MeridianSevered。
    #[test]
    fn charge_carrier_cast_rejected_when_lung_severed() {
        use crate::combat::components::SkillBarBindings;
        use crate::cultivation::components::Cultivation;
        use crate::cultivation::meridian::severed::{MeridianSeveredPermanent, SeveredSource};

        let mut world = bevy_ecs::world::World::new();
        world.insert_resource(CombatClock { tick: 1 });
        world.insert_resource(bevy_ecs::event::Events::<ChargeCarrierIntent>::default());

        let mut severed = MeridianSeveredPermanent::default();
        severed.insert(MeridianId::Lung, SeveredSource::CombatWound, 1);

        let caster = world
            .spawn((
                Cultivation {
                    qi_current: 100.0,
                    qi_max: 200.0,
                    ..Default::default()
                },
                SkillBarBindings::default(),
                severed,
            ))
            .id();

        let result = resolve_anqi_charge_skill(&mut world, caster, 0, None);

        assert!(
            matches!(
                result,
                CastResult::Rejected {
                    reason: CastRejectReason::MeridianSevered(Some(MeridianId::Lung))
                }
            ),
            "期望肺经 SEVERED 时 resolve_anqi_charge_skill 返回 \
             CastRejectReason::MeridianSevered(Some(Lung))（真元无法经肺经注入暗器），\
             实际返回 {result:?}"
        );
    }

    /// 验证 resolve_anqi_charge_skill 在肺经完好（无 SEVERED component）时正常施法。
    #[test]
    fn charge_carrier_cast_allowed_when_lung_intact() {
        use crate::combat::components::SkillBarBindings;
        use crate::cultivation::components::Cultivation;

        let mut world = bevy_ecs::world::World::new();
        world.insert_resource(CombatClock { tick: 1 });
        world.insert_resource(bevy_ecs::event::Events::<ChargeCarrierIntent>::default());

        // 无 MeridianSeveredPermanent component → 肺经视为 INTACT，充能应通过经脉门
        let caster = world
            .spawn((
                Cultivation {
                    qi_current: 100.0,
                    qi_max: 200.0,
                    ..Default::default()
                },
                SkillBarBindings::default(),
            ))
            .id();

        let result = resolve_anqi_charge_skill(&mut world, caster, 0, None);

        // qi_target > 0 且无经脉阻断 → 应进入 Started（充能 intent 已 emit）
        assert!(
            matches!(result, CastResult::Started { .. }),
            "期望肺经完好时 resolve_anqi_charge_skill 返回 CastResult::Started（经脉门放行），\
             实际返回 {result:?}"
        );
    }

    // ── qi 门测试：resolve_anqi_charge_skill 真元不足时提前拒绝 ────────────────────

    /// 核心回归：qi_current=0 时 resolve 必须拒绝，而非启动冷却又无充能效果。
    #[test]
    fn charge_carrier_rejected_when_qi_current_is_zero() {
        use crate::combat::components::SkillBarBindings;

        let mut world = bevy_ecs::world::World::new();
        world.insert_resource(CombatClock { tick: 1 });
        world.insert_resource(bevy_ecs::event::Events::<ChargeCarrierIntent>::default());

        // qi_max=300 → qi_target=(300*0.3).min(80)=80; qi_current=0 < 80 → 应拒绝
        let caster = world
            .spawn((
                Cultivation {
                    qi_current: 0.0,
                    qi_max: 300.0,
                    ..Default::default()
                },
                SkillBarBindings::default(),
            ))
            .id();

        let result = resolve_anqi_charge_skill(&mut world, caster, 0, None);

        assert!(
            matches!(
                result,
                CastResult::Rejected {
                    reason: CastRejectReason::QiInsufficient
                }
            ),
            "期望 qi_current=0 时 resolve_anqi_charge_skill 返回 QiInsufficient（\
             阻止空冷却 bug），实际返回 {result:?}"
        );
    }

    /// 边界：qi_current 正好等于 qi_target 时应允许施法（临界 >= 等号成立）。
    #[test]
    fn charge_carrier_allowed_when_qi_current_exactly_equals_qi_target() {
        use crate::combat::components::SkillBarBindings;

        let mut world = bevy_ecs::world::World::new();
        world.insert_resource(CombatClock { tick: 1 });
        world.insert_resource(bevy_ecs::event::Events::<ChargeCarrierIntent>::default());

        // qi_max=200 → qi_target=(200*0.3) f32 ≈ 60.000004（非整数 60，f32 0.3 不精确）。
        // qi_current 取**真实** qi_target 值以精确测「==」临界，避免硬编码 60.0 因 f32 imprecision
        // 被 guard 误判 < 而拒绝（workflow agent 原测试 bug）。
        let cult_for_target = Cultivation {
            qi_max: 200.0,
            ..Default::default()
        };
        let qi_target_val = f64::from(default_qi_target(&cult_for_target));
        let caster = world
            .spawn((
                Cultivation {
                    qi_current: qi_target_val,
                    qi_max: 200.0,
                    ..Default::default()
                },
                SkillBarBindings::default(),
            ))
            .id();

        let result = resolve_anqi_charge_skill(&mut world, caster, 0, None);

        assert!(
            matches!(result, CastResult::Started { .. }),
            "期望 qi_current 精确等于 qi_target({qi_target_val}) 时允许充能（\
             临界 >= 成立），实际返回 {result:?}"
        );
    }

    /// 边界：qi_current 比 qi_target 少 1 时必须拒绝。
    #[test]
    fn charge_carrier_rejected_when_qi_current_one_below_qi_target() {
        use crate::combat::components::SkillBarBindings;

        let mut world = bevy_ecs::world::World::new();
        world.insert_resource(CombatClock { tick: 1 });
        world.insert_resource(bevy_ecs::event::Events::<ChargeCarrierIntent>::default());

        // qi_max=200 → qi_target=60; qi_current=59 < 60 → 应拒绝
        let caster = world
            .spawn((
                Cultivation {
                    qi_current: 59.0,
                    qi_max: 200.0,
                    ..Default::default()
                },
                SkillBarBindings::default(),
            ))
            .id();

        let result = resolve_anqi_charge_skill(&mut world, caster, 0, None);

        assert!(
            matches!(
                result,
                CastResult::Rejected {
                    reason: CastRejectReason::QiInsufficient
                }
            ),
            "期望 qi_current=59 < qi_target=60 时返回 QiInsufficient（单位以下拒绝），\
             实际返回 {result:?}"
        );
    }

    /// 高 qi_max 玩家（qi_target 触顶 80）qi_current 低于 80 时必须拒绝。
    #[test]
    fn charge_carrier_rejected_for_high_qi_max_player_with_low_qi_current() {
        use crate::combat::components::SkillBarBindings;

        let mut world = bevy_ecs::world::World::new();
        world.insert_resource(CombatClock { tick: 1 });
        world.insert_resource(bevy_ecs::event::Events::<ChargeCarrierIntent>::default());

        // qi_max=600 → qi_target=(600*0.3).min(80)=80; qi_current=50 < 80 → 应拒绝
        // 这是 bug 报告的典型场景：qi_max 高但战斗中 qi_current 被消耗
        let caster = world
            .spawn((
                Cultivation {
                    qi_current: 50.0,
                    qi_max: 600.0,
                    ..Default::default()
                },
                SkillBarBindings::default(),
            ))
            .id();

        let result = resolve_anqi_charge_skill(&mut world, caster, 0, None);

        assert!(
            matches!(
                result,
                CastResult::Rejected {
                    reason: CastRejectReason::QiInsufficient
                }
            ),
            "期望高 qi_max(600) 低 qi_current(50) 玩家被拒绝（qi_target 触顶 80，\
             qi_current<80），实际返回 {result:?}"
        );
    }

    /// 验证低真元拒绝时不会设置冷却（不应烧冷却）。
    #[test]
    fn charge_carrier_rejected_qi_insufficient_does_not_set_cooldown() {
        use crate::combat::components::SkillBarBindings;

        let mut world = bevy_ecs::world::World::new();
        world.insert_resource(CombatClock { tick: 10 });
        world.insert_resource(bevy_ecs::event::Events::<ChargeCarrierIntent>::default());

        let caster = world
            .spawn((
                Cultivation {
                    qi_current: 0.0,
                    qi_max: 300.0,
                    ..Default::default()
                },
                SkillBarBindings::default(),
            ))
            .id();

        let slot: u8 = 2;
        let result = resolve_anqi_charge_skill(&mut world, caster, slot, None);

        // 先验 Rejected
        assert!(
            matches!(
                result,
                CastResult::Rejected {
                    reason: CastRejectReason::QiInsufficient
                }
            ),
            "期望 qi_current=0 被拒绝，实际 {result:?}"
        );

        // 再验冷却未设置：slot 应仍处于 ready 状态（tick=10）
        let bindings = world.get::<SkillBarBindings>(caster).unwrap();
        assert!(
            !bindings.is_on_cooldown(slot, 10),
            "期望真元不足拒绝时不设置冷却（slot={slot} 应 ready），\
             实际 slot 被置为冷却中 — 冷却被烧掉了"
        );
    }
}
