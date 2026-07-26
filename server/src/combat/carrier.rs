use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use valence::entity::Look;
use valence::prelude::{
    bevy_ecs, App, Commands, DVec3, Entity, Event, EventReader, EventWriter, GameMode,
    IntoSystemConfigs, Position, Query, Res, ResMut, UniqueId, Update, With, Without,
};

use crate::body_plan::{
    resolve_body_plan_for_target, BodyPlanPurpose, BodyPlanRegistry, BodyPlanResolveInputs,
    RaceRegistry,
};
use crate::combat::components::{
    Lifecycle, LifecycleState, Stamina, Wound, WoundKind, Wounds, TICKS_PER_SECOND,
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

/// plan-skill-anim-fidelity-v1 P2 后半 —— 封骨充能开始（`begin_charge_carrier`
/// 成功插入 `CarrierCharging` 时发出）。纯观察事件：驱动循环蓄力段动画
/// `anqi_charge_carrier_loop` 的 PlayAnim（vfx_animation_trigger 消费），不参与
/// 任何数值结算。
#[derive(Debug, Clone, Event, PartialEq)]
pub struct CarrierChargeBeganEvent {
    pub carrier: Entity,
    pub tick: u64,
}

/// plan-skill-anim-fidelity-v1 P2 后半 —— 封骨充能结束（`finish_charge` **全部**
/// 退出路径，含密封失败 / 密封量≈0 早退分支）。循环动画停止路径的权威信号
/// （§8.1 #3 红线：任何退出路径都必须停循环段，`CarrierChargedEvent` 在早退
/// 分支不发出、不能兜底）：
/// - `full_charge=true` 充能完成 → StopAnim(循环段) + PlayAnim(release 收势)
/// - `full_charge=false` 移动打断 → 仅 StopAnim（打断不奖励收势）
#[derive(Debug, Clone, Event, PartialEq)]
pub struct CarrierChargeEndedEvent {
    pub carrier: Entity,
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
    app.add_event::<CarrierChargeBeganEvent>();
    app.add_event::<CarrierChargeEndedEvent>();
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
    _slot: u8,
    _target: Option<Entity>,
) -> CastResult {
    let now_tick = world
        .get_resource::<CombatClock>()
        .map(|clock| clock.tick)
        .unwrap_or_default();
    if world
        .get::<crate::combat::components::SkillBarBindings>(caster)
        .is_some_and(|bindings| bindings.is_on_cooldown(ANQI_CHARGE_SKILL_ID, now_tick))
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
        bindings.set_cooldown(
            ANQI_CHARGE_SKILL_ID,
            now_tick.saturating_add(CHARGE_DURATION_TICKS),
        );
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
    mut began_events: EventWriter<CarrierChargeBeganEvent>,
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
        // P2 后半：充能开始 → 循环蓄力段动画信号（纯观察，AV 消费在
        // vfx_animation_trigger::emit_anqi_visual_triggers）。
        began_events.send(CarrierChargeBeganEvent {
            carrier: entity,
            tick: clock.tick,
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

#[allow(clippy::too_many_arguments)]
fn charge_carrier_tick(
    clock: Res<CombatClock>,
    registry: Res<ItemRegistry>,
    mut zones: Option<ResMut<ZoneRegistry>>,
    mut commands: Commands,
    mut actors: Query<ChargingActor<'_>>,
    mut events: EventWriter<CarrierChargedEvent>,
    mut ended_events: EventWriter<CarrierChargeEndedEvent>,
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
                &mut ended_events,
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
            &mut ended_events,
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
    ended_events: &mut EventWriter<CarrierChargeEndedEvent>,
) {
    // P2 后半：充能结束信号在**所有**退出路径发出（循环动画停止路径红线
    // §8.1 #3——早退分支无 CarrierChargedEvent，循环段必须由本事件停止）。
    ended_events.send(CarrierChargeEndedEvent {
        carrier: entity,
        full_charge,
        tick,
    });
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
    // plan-race-system-v1 P0c —— 投射物命中部位分类按目标实体分派：`resolve_body_plan`
    // 的玩家身份权威真源。不查 `BeastKind`（同 `combat::resolve::body_part_multipliers`
    // 注释：races.json 现阶段所有 BeastKind 派生种族的 body_plan_id 均为 "humanoid"，
    // `beast_kind: None` 落进 Tier2/Tier3 分支得到完全相同的 humanoid 解析结果）。
    Option<&'a Cultivation>,
    // plan-race-system-v1 P0 review r2（BLOCKING-1 收口）—— `PartBoxes` 命中几何分类
    // 需要目标朝向把世界系命中点变换到局部系（见下方 `classify_body_part` 调用）；
    // `HeightBands` 分支忽略这个值。
    Option<&'a Look>,
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
    // plan-race-system-v1 P0c —— `Option<Res<...>>` 同 `body_part_multipliers` 既有
    // 约定：大量既有单测未插入这两个资源，缺失时 `classify_body_part` 的目标 plan
    // 解析优雅退化到 `humanoid_plan_static()`（生产环境 `body_plan::register()` 恒
    // 装载，这条退化分支不会在真实部署触发）。
    body_plan_registry: Option<Res<BodyPlanRegistry>>,
    race_registry: Option<Res<RaceRegistry>>,
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
        for (target_entity, target_pos, _, _, _, target_cultivation, _) in &mut targets {
            if projectile.owner == Some(target_entity) {
                continue;
            }
            // plan-race-system-v1 P5/PR-6c —— 粗筛半径按目标当前 body_plan 动态派生
            // （`body_plan::geometry::bounding_radius`），替换写死的 humanoid 专属
            // `0.3`（`STANDING_HALF_WIDTH`）：whale 等横长非人构型的真实体积远大于
            // 人形，固定 0.3 会让弹道在肉眼可见的"打中了"情况下被判定为未命中。
            // humanoid 目标：`bounding_radius` 对 `HeightBands` 原样吐出
            // `aabb.half_width`（0.3），与换轨前 bit-for-bit 相同，不回归。
            let target_body_plan = resolve_body_plan_for_target(
                target_entity,
                BodyPlanPurpose::Intrinsic,
                BodyPlanResolveInputs {
                    cultivation: target_cultivation,
                    beast_kind: None,
                    morph_state: None,
                },
                body_plan_registry.as_deref(),
                race_registry.as_deref(),
            );
            let target_radius =
                crate::body_plan::geometry::bounding_radius(&target_body_plan.hit_geometry);
            let distance_to_segment =
                segment_point_distance(current, next, target_pos.get() + DVec3::new(0.0, 1.0, 0.0));
            if distance_to_segment <= f64::from(flight.hitbox_inflation) + target_radius {
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
            let Ok((_, target_pos, mut wounds, mut contamination, life_record, cultivation, look)) =
                targets.get_mut(target_entity)
            else {
                continue;
            };
            let target_yaw_radians = look
                .map(|look| f64::from(look.yaw).to_radians())
                .unwrap_or(0.0);
            // plan-combat-hit-location-v1 P2（决议 §8.1 旁路桶 #2）—— 暗器/凝气弹投射命中
            // 部位应按弹道终点几何算，而非硬编 Chest：复用 `classify_body_part` 同一套
            // 几何分类（与近战 `raycast_humanoid` 共享阈值/语义），以本 tick 飞行段
            // （`current` → `next`）上离目标中心最近的点作为命中点、`flight.spawn_pos`
            // 作为攻方几何原点（弹道起点，决定 lateral 判定的参照方向）。
            // plan-race-system-v1 P0c —— `classify_body_part` 现在按**目标实体**分派：
            // 经 `resolve_body_plan_for_target`（与近战 `combat::resolve` 消费点同款
            // 兜底链路）解析出目标的 `&BodyPlan` 再传入，不再无条件读
            // `humanoid_plan_static()` 单例。
            let target_body_plan = resolve_body_plan_for_target(
                target_entity,
                BodyPlanPurpose::Intrinsic,
                BodyPlanResolveInputs {
                    cultivation,
                    beast_kind: None,
                    morph_state: None,
                },
                body_plan_registry.as_deref(),
                race_registry.as_deref(),
            );
            let target_feet = target_pos.get();
            let target_center = target_feet + DVec3::new(0.0, 1.0, 0.0);
            let segment = next - current;
            // plan-race-system-v1 P0 review r3（blocker 收口）—— PartBoxes 目标不再用
            // "已知命中点（离目标中心最近的线段投影点）+ classify_part_boxes_point 的
            // 就近回退"反推部位：命中点落在盒间空隙时，就近回退会把空隙伪造成有效命中
            // （语义缺陷）。改为对本 tick 弹道线段（`current` → `next`，即"上一位置→
            // 命中位置"）做真实射线-盒求交 `body_plan::geometry::raycast_part_boxes`
            // ——与 `raycast_humanoid` 的 `PartBoxes` 分支（近战统一入口）完全同一个几何
            // 原语，以首次相交的 part_id 为权威；线段穿过空隙、未与任何盒相交时
            // 忠实返回 `None`（不伪造命中部位），由下方 Wound 构造处显式处理。
            // `HeightBands` 目标路径 bit-for-bit 不变：仍是"线段→目标中心最近点投影 +
            // `classify_body_part`"这条既有实现，未做任何改动。
            let body_part: Option<crate::body_plan::BodyPartId> = match &target_body_plan
                .hit_geometry
            {
                crate::body_plan::HitGeometry::HeightBands { .. } => {
                    let segment_len_sq = segment.length_squared();
                    let t = if segment_len_sq <= f64::EPSILON {
                        0.0
                    } else {
                        ((target_center - current).dot(segment) / segment_len_sq).clamp(0.0, 1.0)
                    };
                    let projectile_hit_point = current + segment * t;
                    Some(crate::combat::raycast::classify_body_part(
                        target_body_plan,
                        projectile_hit_point,
                        target_feet,
                        flight.spawn_pos,
                        target_yaw_radians,
                    ))
                }
                crate::body_plan::HitGeometry::PartBoxes { boxes } => {
                    crate::body_plan::geometry::raycast_part_boxes(
                        current,
                        segment,
                        segment.length(),
                        target_feet,
                        target_yaw_radians,
                        boxes,
                    )
                    .map(|part_hit| part_hit.part_id)
                }
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
            // plan-race-system-v1 P0 review r3 —— PartBoxes 目标弹道穿过部位间空隙时
            // （粗筛 capsule 判定"够近"，但精细求交没有命中任何具体局部盒），显式策略：
            // 跳过 Wound 构造（不伪造命中部位）；伤害/沾染/事件仍照常结算（粗筛已确认
            // 真实物理接触）——carrier 本身没有部位伤害倍率概念，`wound_damage` 不因
            // 缺失部位而打折/加成，等价于"部位倍率中性"。`tracing::debug` 记录，行为由
            // `projectile_through_partboxes_gap_skips_wound_but_still_applies_damage`
            // 等专属测试锁死。
            match &body_part {
                Some(part_id) => {
                    wounds.entries.push(Wound {
                        location: part_id.clone(),
                        kind: WoundKind::Pierce,
                        severity: wound_damage,
                        bleeding_per_sec: wound_damage * 0.05,
                        created_at_tick: clock.tick,
                        inflicted_by: Some(attacker_id.clone()),
                    });
                }
                None => {
                    tracing::debug!(
                        "[bong][carrier] projectile {:?} 弹道穿过目标 {:?} 的 PartBoxes 空隙\
                         （未与任何局部盒相交）——跳过 Wound 构造，伤害/沾染/事件仍照常结算",
                        projectile_entity,
                        target_entity
                    );
                }
            }
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
                // humanoid-only boundary（P0 决议，边界①，同 combat::resolve 同名分支）：
                // `CombatEvent.body_part` 是 legacy 8 值枚举，非人形部位 id 落回 Chest
                // 占位 + warn（不是静默默认）。plan-race-system-v1 P0 review r3 —— 弹道
                // 穿过 PartBoxes 空隙（`body_part == None`）同样落回 Chest 占位，但用
                // `debug`（非 `warn`）记录：这是几何上合法的空隙场景，不是数据缺陷。
                body_part: match &body_part {
                    Some(part_id) => crate::body_plan::id_to_legacy_body_part(part_id)
                        .unwrap_or_else(|| {
                            tracing::warn!(
                                "[bong][body_plan] carrier CombatEvent wire: part id {} has no \
                                 legacy BodyPart mapping — emitting BodyPart::Chest as an \
                                 explicit placeholder (not a silent default)",
                                part_id
                            );
                            crate::combat::components::BodyPart::Chest
                        }),
                    None => {
                        tracing::debug!(
                            "[bong][body_plan] carrier CombatEvent wire: PartBoxes 目标本次弹道\
                             未命中任何局部盒 —— emitting BodyPart::Chest as an explicit \
                             placeholder (gap hit, not a silent default)"
                        );
                        crate::combat::components::BodyPart::Chest
                    }
                },
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
    use crate::body_plan::BodyPartId;
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
            readable_scroll_spec: None,
            recipe_fragment_spec: None,
            container_spec: None,
            shelflife_profile: None,
            shield_spec: None,
            shelflife_track: None,
            wearer_race: crate::body_plan::types::RaceGateOwned::default(),
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
        app.add_event::<CarrierChargeBeganEvent>();
        app.add_event::<CarrierChargeEndedEvent>();
        app.add_event::<QiTransfer>();
        app.add_systems(Update, (begin_charge_carrier, charge_carrier_tick));
        app
    }

    fn drain_charge_began(app: &mut App) -> Vec<CarrierChargeBeganEvent> {
        app.world_mut()
            .resource_mut::<Events<CarrierChargeBeganEvent>>()
            .drain()
            .collect()
    }

    fn drain_charge_ended(app: &mut App) -> Vec<CarrierChargeEndedEvent> {
        app.world_mut()
            .resource_mut::<Events<CarrierChargeEndedEvent>>()
            .drain()
            .collect()
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

    // ── P2 后半：充能开始/结束观察事件（循环蓄力段动画停止路径 §8.1 #3）────────

    /// 充能成功开始 → 恰发 1 条 CarrierChargeBeganEvent（循环段 PlayAnim 信号），
    /// 此刻不得有任何 Ended（循环不能未播先停）。
    #[test]
    fn begin_charge_emits_charge_began_event_without_ended() {
        let mut app = charge_app();
        let actor = spawn_charge_actor(&mut app);

        app.world_mut().send_event(ChargeCarrierIntent {
            carrier: actor,
            slot: Some(CarrierSlot::MainHand),
            qi_target: Some(60.0),
            issued_at_tick: 0,
        });
        app.update();

        let began = drain_charge_began(&mut app);
        assert_eq!(
            began.len(),
            1,
            "充能开始应恰发 1 条 CarrierChargeBeganEvent（循环蓄力段动画信号），实际 {} 条",
            began.len()
        );
        assert_eq!(began[0].carrier, actor, "Began 事件应指向充能者本人");
        assert!(
            drain_charge_ended(&mut app).is_empty(),
            "充能刚开始不得发 CarrierChargeEndedEvent（循环段不能未播先停）"
        );
    }

    /// 充能被拒（qi_target 超上限 → begin 静默 continue）→ 不发 Began：
    /// 没开始的充能不能触发循环动画。
    #[test]
    fn rejected_begin_charge_emits_no_began_event() {
        let mut app = charge_app();
        let actor = spawn_charge_actor(&mut app);

        app.world_mut().send_event(ChargeCarrierIntent {
            carrier: actor,
            slot: Some(CarrierSlot::MainHand),
            // default_qi_target(qi_max=200)=60，超出即拒。
            qi_target: Some(90.0),
            issued_at_tick: 0,
        });
        app.update();

        assert!(
            app.world().get::<CarrierCharging>(actor).is_none(),
            "前置：超上限 qi_target 应被拒、不插 CarrierCharging"
        );
        assert!(
            drain_charge_began(&mut app).is_empty(),
            "被拒的充能不得发 CarrierChargeBeganEvent（否则循环动画凭空开播）"
        );
    }

    /// 自然完成 → Ended{full_charge:true}（StopAnim+release 信号）且与
    /// CarrierChargedEvent(full) 同拍。
    #[test]
    fn full_charge_completion_emits_ended_full() {
        let mut app = charge_app();
        let actor = spawn_charge_actor(&mut app);

        app.world_mut().send_event(ChargeCarrierIntent {
            carrier: actor,
            slot: Some(CarrierSlot::MainHand),
            qi_target: Some(60.0),
            issued_at_tick: 0,
        });
        app.update();
        drain_charge_ended(&mut app);

        app.world_mut().resource_mut::<CombatClock>().tick = CHARGE_DURATION_TICKS;
        app.update();

        let ended = drain_charge_ended(&mut app);
        assert_eq!(
            ended.len(),
            1,
            "充能自然完成应恰发 1 条 CarrierChargeEndedEvent，实际 {} 条",
            ended.len()
        );
        assert!(
            ended[0].full_charge,
            "自然完成的 Ended 必须 full_charge=true（驱动 StopAnim+release 收势）"
        );
        assert_eq!(ended[0].carrier, actor);
        let charged: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<CarrierChargedEvent>>()
            .drain()
            .collect();
        assert!(
            charged.iter().any(|event| event.full_charge),
            "前置：自然完成应同拍发出 CarrierChargedEvent(full_charge=true)"
        );
    }

    /// 半程移动打断 → Ended{full_charge:false}（仅 StopAnim，打断不奖励收势）。
    #[test]
    fn movement_interrupt_emits_ended_not_full() {
        let mut app = charge_app();
        let actor = spawn_charge_actor(&mut app);

        app.world_mut().send_event(ChargeCarrierIntent {
            carrier: actor,
            slot: Some(CarrierSlot::MainHand),
            qi_target: Some(60.0),
            issued_at_tick: 0,
        });
        app.update();
        drain_charge_ended(&mut app);

        app.world_mut()
            .entity_mut(actor)
            .insert(Position::new([2.0, 66.0, 0.0]));
        app.world_mut().resource_mut::<CombatClock>().tick = CHARGE_DURATION_TICKS / 2;
        app.update();

        let ended = drain_charge_ended(&mut app);
        assert_eq!(
            ended.len(),
            1,
            "移动打断应恰发 1 条 CarrierChargeEndedEvent，实际 {} 条",
            ended.len()
        );
        assert!(
            !ended[0].full_charge,
            "移动打断的 Ended 必须 full_charge=false（仅 StopAnim，不播 release）"
        );
    }

    /// 零进度立即打断（progress≈0 → 密封量≈0 走 finish_charge 早退分支，无
    /// CarrierChargedEvent）→ **仍必须**发 Ended：早退路径漏发 = 循环动画永卡
    /// （§8.1 #3 全退出路径覆盖的关键锚点）。
    #[test]
    fn zero_progress_interrupt_still_emits_ended_despite_no_charged_event() {
        let mut app = charge_app();
        let actor = spawn_charge_actor(&mut app);

        app.world_mut().send_event(ChargeCarrierIntent {
            carrier: actor,
            slot: Some(CarrierSlot::MainHand),
            qi_target: Some(60.0),
            issued_at_tick: 0,
        });
        app.update();
        drain_charge_ended(&mut app);

        // clock 仍为 0：elapsed=0 → progress_ratio=0 → qi_amount=0 → 早退分支。
        app.world_mut()
            .entity_mut(actor)
            .insert(Position::new([2.0, 66.0, 0.0]));
        app.update();

        assert!(
            app.world().get::<CarrierCharging>(actor).is_none(),
            "前置：零进度移动打断也应结束 CarrierCharging"
        );
        let charged: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<CarrierChargedEvent>>()
            .drain()
            .collect();
        assert!(
            charged.is_empty(),
            "前置：零进度打断走早退分支，不应发 CarrierChargedEvent"
        );
        let ended = drain_charge_ended(&mut app);
        assert_eq!(
            ended.len(),
            1,
            "早退分支（密封量≈0）也必须发 Ended——漏发 = 循环蓄力段动画永卡在玩家身上"
        );
        assert!(
            !ended[0].full_charge,
            "零进度打断的 Ended 必须 full_charge=false"
        );
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

    // ══════════════════════════════════════════════════════════════════════════
    // plan-combat-hit-location-v1 P2（决议 §8.1 旁路桶 #2）— 投射命中部位几何化 pin
    // ══════════════════════════════════════════════════════════════════════════

    /// 构造一发沿 X 轴飞行、经过给定绝对 Y 高度的暗器投射，命中站在原点的目标。
    /// `flight_y` 决定投射穿过目标 hitbox 时的高度，从而驱动 `classify_body_part`
    /// 落到不同部位——用来证明命中部位不再恒为 `BodyPart::Chest`。
    fn projectile_hit_body_part_at_height(flight_y: f64) -> crate::body_plan::BodyPartId {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 10 });
        app.add_event::<CombatEvent>();
        app.add_event::<CarrierImpactEvent>();
        app.add_event::<ProjectileDespawnedEvent>();
        app.add_systems(Update, projectile_tick_system);

        app.world_mut().spawn((
            Position::new([-1.0, flight_y, 0.0]),
            QiProjectile {
                owner: None,
                qi_payload: 20.0,
            },
            AnqiProjectileFlight {
                carrier_kind: CarrierKind::BoneChip,
                qi_color: ColorKind::Sharp,
                carrier_grade: CarrierKind::BoneChip.grade(),
                spawn_pos: DVec3::new(-1.0, flight_y, 0.0),
                prev_pos: DVec3::new(-1.0, flight_y, 0.0),
                velocity: DVec3::new(20.0, 0.0, 0.0),
                max_distance: ANQI_PROJECTILE_MAX_DISTANCE,
                hitbox_inflation: ANQI_HITBOX_INFLATION,
            },
        ));
        // 目标 `Position` 是脚底坐标（`classify_body_part` 的 `target_feet_position`
        // 约定，见 `raycast.rs::standing_humanoid_aabb`）；无 `GameMode` 组件即视为
        // 可被伤害（`is_damageable` 默认 true）。
        let target = app
            .world_mut()
            .spawn((
                Position::new([0.0, 0.0, 0.0]),
                Wounds::default(),
                Contamination::default(),
            ))
            .id();

        app.update();

        let wounds = app.world().entity(target).get::<Wounds>().unwrap();
        assert_eq!(
            wounds.entries.len(),
            1,
            "flight_y={flight_y} 应命中目标产生恰好一条 Wound，实测 {} 条 —— \
             若为 0 说明本次高度没有几何相交，测试几何参数需要调整",
            wounds.entries.len()
        );
        let combat_events: Vec<_> = app
            .world()
            .resource::<Events<CombatEvent>>()
            .iter_current_update_events()
            .collect();
        assert_eq!(combat_events.len(), 1);
        // `Wound.location`（`BodyPartId`）与 `CombatEvent.body_part`（legacy `BodyPart`，
        // 边界①转换）必须是同一次 `classify_body_part` 调用结果——humanoid 部位全部能
        // 干净转换回 legacy，转换失败（非人形，本测试不涉及）会走 Chest 占位而非本断言
        // 覆盖的路径。
        assert_eq!(
            wounds.entries[0].location,
            crate::body_plan::legacy_body_part_to_id(combat_events[0].body_part),
            "Wound.location 与 CombatEvent.body_part 必须是同一个 classify_body_part \
             调用结果，实测 Wound={:?} CombatEvent={:?} 不一致",
            wounds.entries[0].location,
            combat_events[0].body_part
        );
        wounds.entries[0].location.clone()
    }

    #[test]
    fn projectile_hit_at_head_height_classifies_head_not_chest() {
        // 目标脚底 y=0，头部阈值 rel_y>0.88 → y>1.584；投射沿 y=1.65 平飞穿过目标中心线
        // （命中判定半径 0.3+0.4=0.7，|1.65-1.0|=0.65 留够浮点误差余量）。
        let part = projectile_hit_body_part_at_height(1.65);
        assert_eq!(
            part,
            BodyPartId::new("head"),
            "投射沿头部高度（y=1.65，脚底 y=0）飞行应命中 head，实测 {part:?} —— \
             若又是 chest 说明 P2 旁路清理被回退成硬编胸口了"
        );
    }

    #[test]
    fn projectile_hit_at_leg_height_classifies_leg_not_chest() {
        // 腿部阈值 rel_y<=0.53 → y<=0.954；投射沿 y=0.5 平飞穿过目标中心线
        // （|0.5-1.0|=0.5，同样留够命中半径 0.7 的浮点误差余量）。
        let part = projectile_hit_body_part_at_height(0.5);
        assert!(
            part == BodyPartId::new("leg_l") || part == BodyPartId::new("leg_r"),
            "投射沿腿部高度（y=0.5，脚底 y=0）飞行应命中 leg_l/leg_r，实测 {part:?} —— \
             若是 chest 说明命中部位仍是恒定胸口而非按弹道几何算出"
        );
    }

    #[test]
    fn projectile_hit_at_chest_height_still_classifies_chest() {
        // 对照组：胸口高度（rel_y≈0.556，在 0.55~0.88 之间且 lateral 落在阈值内）仍应判 Chest，
        // 证明这不是"再也不会出现 Chest"而是"部位随几何真实变化，胸口只是其中一种可能"。
        let part = projectile_hit_body_part_at_height(1.0);
        assert_eq!(
            part,
            BodyPartId::new("chest"),
            "投射沿胸口高度（y=1.0，脚底 y=0）飞行应命中 chest，实测 {part:?}"
        );
    }

    // ══════════════════════════════════════════════════════════════════════════
    // plan-race-system-v1 P0 review r3（blocker+major 收口）—— carrier 投射物对
    // PartBoxes 目标改用弹道线段真求交（`body_plan::geometry::raycast_part_boxes`），
    // 取代此前"已知命中点 + classify_part_boxes_point 就近回退"的语义缺陷（盒间空隙
    // 会被伪造成有效命中）。以下测试全部走真实 `projectile_tick_system`
    // 生产系统（不直接调用几何纯函数），合成非人形 PartBoxes 构型 + 真实
    // BodyPlanRegistry/RaceRegistry，覆盖：①前后部位遮挡（近盒挡远盒）②平移后仍
    // 正确命中 ③射线穿过空隙时跳过 Wound 构造但伤害/事件仍照常结算。
    // ══════════════════════════════════════════════════════════════════════════
    mod partboxes_carrier_production_integration_tests {
        use super::*;
        use crate::body_plan::race_registry::RaceEntry;
        use crate::body_plan::types::{BodyPartDef, HitGeometry, PartBox, PartConsequence};
        use crate::body_plan::{BodyPlanRegistry, RaceRegistry};
        use crate::cultivation::components::Cultivation;
        use std::collections::HashMap as StdHashMap;

        /// 双盒合成构型：`near_part`/`far_part` 沿局部 +X（=世界 +X，target yaw=0 时
        /// 局部系与世界系重合）前后排列，`near_part` 更靠近射线起点。
        fn two_box_plan(near_part: &str, far_part: &str) -> crate::body_plan::BodyPlan {
            crate::body_plan::BodyPlan {
                id: format!("test_carrier_two_box_{near_part}_{far_part}").into(),
                display_name: "测试用 carrier 双盒构型".to_string(),
                is_humanoid: false,
                parts: vec![
                    BodyPartDef {
                        id: near_part.into(),
                        damage_mul: 1.0,
                        contam_mul: 1.0,
                        bleed_mul: 1.0,
                        consequence: PartConsequence::Core,
                    },
                    BodyPartDef {
                        id: far_part.into(),
                        damage_mul: 1.0,
                        contam_mul: 1.0,
                        bleed_mul: 1.0,
                        consequence: PartConsequence::Core,
                    },
                ],
                hit_geometry: HitGeometry::PartBoxes {
                    boxes: vec![
                        // 局部 offset y=1.0 对齐粗筛 capsule 判定用的 target_center
                        // （`target_pos + (0,1,0)`），确保粗筛与精细求交在同一高度。
                        PartBox {
                            part_id: near_part.into(),
                            offset: [0.0, 1.0, 0.0],
                            half_extents: [0.3, 0.3, 0.3],
                            priority: 0,
                        },
                        PartBox {
                            part_id: far_part.into(),
                            offset: [1.5, 1.0, 0.0],
                            half_extents: [0.3, 0.3, 0.3],
                            priority: 0,
                        },
                    ],
                },
                equip_slots: vec![],
                meridian_profile: None,
                mutation_slot_mapping: StdHashMap::new(),
            }
        }

        /// 空隙构型：唯一的盒偏移远离射线路径（局部 x=5.0，射线只走到 x≈2.0 就结束），
        /// 粗筛 capsule（只判定到 target_center 点的距离，与盒位置无关）仍会命中，
        /// 但精细 PartBoxes 求交必须落空。
        fn gap_plan(part_id: &str) -> crate::body_plan::BodyPlan {
            crate::body_plan::BodyPlan {
                id: format!("test_carrier_gap_{part_id}").into(),
                display_name: "测试用 carrier 空隙构型".to_string(),
                is_humanoid: false,
                parts: vec![BodyPartDef {
                    id: part_id.into(),
                    damage_mul: 1.0,
                    contam_mul: 1.0,
                    bleed_mul: 1.0,
                    consequence: PartConsequence::Core,
                }],
                hit_geometry: HitGeometry::PartBoxes {
                    boxes: vec![PartBox {
                        part_id: part_id.into(),
                        offset: [5.0, 1.0, 0.0],
                        half_extents: [0.3, 0.3, 0.3],
                        priority: 0,
                    }],
                },
                equip_slots: vec![],
                meridian_profile: None,
                mutation_slot_mapping: StdHashMap::new(),
            }
        }

        fn registries_for(plan: crate::body_plan::BodyPlan) -> (BodyPlanRegistry, RaceRegistry) {
            let plan_id = plan.id.clone();
            let body_plans = BodyPlanRegistry::from_plans(vec![plan])
                .expect("synthetic carrier plan must validate");
            let races = RaceRegistry::from_parts_for_test(
                vec![RaceEntry {
                    id: crate::body_plan::RaceId::new(crate::body_plan::HUMAN_RACE_ID),
                    display_name: "carrier PartBoxes 测试替身".to_string(),
                    body_plan_id: plan_id,
                    beast_kinds: vec![],
                }],
                vec![],
                &body_plans,
            )
            .expect("races fixture must validate");
            (body_plans, races)
        }

        /// 组装最小 App：合成 registries + `projectile_tick_system` + 一发沿世界 +X
        /// 飞行的投射物 + 一个携带 `Cultivation::default()`（race 解析到合成 plan）的
        /// 目标。`target_feet` 允许任意平移，验证生产链路的世界→局部变换真的用了
        /// 目标的实际位置，而不是隐式假设原点。
        fn run_projectile_at_target(
            plan: crate::body_plan::BodyPlan,
            target_feet: DVec3,
        ) -> (Wounds, Vec<CombatEvent>, Vec<ProjectileDespawnedEvent>) {
            let (body_plans, races) = registries_for(plan);
            let mut app = App::new();
            app.insert_resource(CombatClock { tick: 900 });
            app.insert_resource(body_plans);
            app.insert_resource(races);
            app.add_event::<CombatEvent>();
            app.add_event::<CarrierImpactEvent>();
            app.add_event::<ProjectileDespawnedEvent>();
            app.add_systems(Update, projectile_tick_system);

            // 射线沿世界 +X：spawn 于 target 局部 x=-3（射线起点），一 tick 内飞抵
            // 局部 x=+2（速度 100，dt=1/20s，单 tick 位移 5.0 blocks），覆盖两盒
            // 构型的 near(x∈[-0.3,0.3])/far(x∈[1.2,1.8]) 与空隙构型的射线终点(x=2)
            // 均落在 gap 盒(x∈[4.7,5.3])之外。
            let spawn_pos = target_feet + DVec3::new(-3.0, 1.0, 0.0);
            app.world_mut().spawn((
                Position::new([spawn_pos.x, spawn_pos.y, spawn_pos.z]),
                QiProjectile {
                    owner: None,
                    qi_payload: 20.0,
                },
                AnqiProjectileFlight {
                    carrier_kind: CarrierKind::BoneChip,
                    qi_color: ColorKind::Sharp,
                    carrier_grade: CarrierKind::BoneChip.grade(),
                    spawn_pos,
                    prev_pos: spawn_pos,
                    velocity: DVec3::new(100.0, 0.0, 0.0),
                    max_distance: ANQI_PROJECTILE_MAX_DISTANCE,
                    hitbox_inflation: ANQI_HITBOX_INFLATION,
                },
            ));
            let target = app
                .world_mut()
                .spawn((
                    Position::new([target_feet.x, target_feet.y, target_feet.z]),
                    Wounds::default(),
                    Contamination::default(),
                    Cultivation::default(),
                ))
                .id();

            app.update();

            let wounds = app.world().entity(target).get::<Wounds>().unwrap().clone();
            let combat_events: Vec<CombatEvent> = app
                .world()
                .resource::<Events<CombatEvent>>()
                .iter_current_update_events()
                .cloned()
                .collect();
            let despawns: Vec<ProjectileDespawnedEvent> = app
                .world()
                .resource::<Events<ProjectileDespawnedEvent>>()
                .iter_current_update_events()
                .cloned()
                .collect();
            (wounds, combat_events, despawns)
        }

        #[test]
        fn near_box_occludes_far_box_at_origin() {
            let plan = two_box_plan("near_plate", "far_plate");
            let (wounds, _events, _despawns) =
                run_projectile_at_target(plan, DVec3::new(0.0, 64.0, 0.0));
            assert_eq!(
                wounds.entries.len(),
                1,
                "PartBoxes 真求交应恰好命中一个部位，实测 {:?}",
                wounds.entries
            );
            assert_eq!(
                wounds.entries[0].location,
                BodyPartId::new("near_plate"),
                "两盒都在射线路径上时，near_plate（离投射起点更近）必须遮挡 far_plate，\
                 实测命中 {:?}",
                wounds.entries[0].location
            );
        }

        #[test]
        fn near_box_occludes_far_box_after_target_translation() {
            // 与上一测试几何完全相同，唯一变量是目标整体平移到远离原点的坐标——
            // 证明生产链路的世界→局部变换用的是目标实际位置，不是隐式硬编码原点。
            let plan = two_box_plan("near_plate", "far_plate");
            let (wounds, _events, _despawns) =
                run_projectile_at_target(plan, DVec3::new(437.0, 64.0, -812.0));
            assert_eq!(wounds.entries.len(), 1);
            assert_eq!(
                wounds.entries[0].location,
                BodyPartId::new("near_plate"),
                "平移后仍应命中 near_plate（局部系不变性在生产链路中成立），实测 {:?}",
                wounds.entries[0].location
            );
        }

        #[test]
        fn ray_through_partboxes_gap_skips_wound_but_still_applies_damage() {
            let plan = gap_plan("shell");
            let (wounds, combat_events, despawns) =
                run_projectile_at_target(plan, DVec3::new(0.0, 64.0, 0.0));

            assert!(
                wounds.entries.is_empty(),
                "弹道穿过 PartBoxes 空隙必须跳过 Wound 构造，不伪造命中部位，实测 {:?}",
                wounds.entries
            );
            assert!(
                wounds.health_current < Wounds::default().health_max,
                "即便跳过 Wound 构造，粗筛已确认的真实物理接触仍应照常结算伤害（health_current \
                 应低于满血），实测 {}",
                wounds.health_current
            );
            assert_eq!(
                combat_events.len(),
                1,
                "空隙命中仍应发出恰好一条 CombatEvent（伤害/事件照常结算），实测 {combat_events:?}"
            );
            assert_eq!(
                combat_events[0].body_part,
                crate::combat::components::BodyPart::Chest,
                "空隙命中的 CombatEvent.body_part 应落回 Chest 占位（显式 fallback，非静默默认）"
            );
            assert_eq!(
                despawns.len(),
                1,
                "空隙命中仍应作为 HitTarget 消耗投射物（despawn 恰好一次），实测 {despawns:?}"
            );
            assert_eq!(despawns[0].reason, ProjectileDespawnReason::HitTarget);
        }

        // ══════════════════════════════════════════════════════════════════════
        // plan-race-system-v1 P5/PR-6c —— 粗筛半径按目标 body_plan 动态派生，用真实
        // whale.json 目标验证：换轨前写死的 `0.3+ANQI_HITBOX_INFLATION=0.7` 判定不到
        // 的横向偏移，换轨后（whale 半径 ≈5.74）必须能命中；humanoid 目标半径不回归。
        // ══════════════════════════════════════════════════════════════════════

        /// 加载真实磁盘 `assets/body_plans/plans/*.json` + `races.json`（而非合成
        /// fixture）——本组测试要断言的是真实落盘 whale.json 数据驱动出的粗筛半径，
        /// 不是任意手搓的 PartBoxes 构型。
        fn real_registries() -> (BodyPlanRegistry, RaceRegistry) {
            let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
            let plans_dir = manifest_dir.join(crate::body_plan::registry::DEFAULT_BODY_PLANS_DIR);
            let races_path = manifest_dir.join(crate::body_plan::race_registry::DEFAULT_RACES_PATH);
            let body_plans =
                BodyPlanRegistry::load_dir(&plans_dir).expect("real plans/ should load");
            let races = RaceRegistry::load_file(&races_path, &body_plans)
                .expect("real races.json should load");
            (body_plans, races)
        }

        /// 组装一发沿世界 +X 飞行、在 `lateral_z_offset` 处经过目标的投射，目标携带
        /// 给定 `race` 的 `Cultivation`（走真实 registries 解析 body_plan）。
        fn run_lateral_projectile_at_real_target(
            race: crate::body_plan::RaceId,
            lateral_z_offset: f64,
        ) -> (Wounds, Vec<CombatEvent>, Vec<ProjectileDespawnedEvent>) {
            let (body_plans, races) = real_registries();
            let mut app = App::new();
            app.insert_resource(CombatClock { tick: 900 });
            app.insert_resource(body_plans);
            app.insert_resource(races);
            app.add_event::<CombatEvent>();
            app.add_event::<CarrierImpactEvent>();
            app.add_event::<ProjectileDespawnedEvent>();
            app.add_systems(Update, projectile_tick_system);

            let target_feet = DVec3::new(0.0, 64.0, 0.0);
            // 粗筛 capsule 的固定参照点是 `target_pos + (0,1,0)`，横向偏移全部落在 z
            // 轴（射线沿世界 +X 飞行，不与该参照点在 x 上重合，只在 z 上偏移）。
            let spawn_pos = DVec3::new(-3.0, target_feet.y + 1.0, lateral_z_offset);
            app.world_mut().spawn((
                Position::new([spawn_pos.x, spawn_pos.y, spawn_pos.z]),
                QiProjectile {
                    owner: None,
                    qi_payload: 20.0,
                },
                AnqiProjectileFlight {
                    carrier_kind: CarrierKind::BoneChip,
                    qi_color: ColorKind::Sharp,
                    carrier_grade: CarrierKind::BoneChip.grade(),
                    spawn_pos,
                    prev_pos: spawn_pos,
                    velocity: DVec3::new(100.0, 0.0, 0.0),
                    max_distance: ANQI_PROJECTILE_MAX_DISTANCE,
                    hitbox_inflation: ANQI_HITBOX_INFLATION,
                },
            ));
            let target = app
                .world_mut()
                .spawn((
                    Position::new([target_feet.x, target_feet.y, target_feet.z]),
                    Wounds::default(),
                    Contamination::default(),
                    Cultivation {
                        race,
                        ..Default::default()
                    },
                ))
                .id();

            app.update();

            let wounds = app.world().entity(target).get::<Wounds>().unwrap().clone();
            let combat_events: Vec<CombatEvent> = app
                .world()
                .resource::<Events<CombatEvent>>()
                .iter_current_update_events()
                .cloned()
                .collect();
            let despawns: Vec<ProjectileDespawnedEvent> = app
                .world()
                .resource::<Events<ProjectileDespawnedEvent>>()
                .iter_current_update_events()
                .cloned()
                .collect();
            (wounds, combat_events, despawns)
        }

        #[test]
        fn humanoid_target_bounding_radius_does_not_regress_beyond_legacy_fixed_value() {
            // 换轨前写死的判定半径是 0.3（STANDING_HALF_WIDTH 字面量）+
            // ANQI_HITBOX_INFLATION(0.4) = 0.7。z 偏移 0.6 应命中（<0.7），0.8 应未命中
            // （>0.7）——humanoid 目标必须与换轨前 bit-for-bit 一致的判定边界。
            let (_, _, despawns_hit) = run_lateral_projectile_at_real_target(
                crate::body_plan::RaceId::new(crate::body_plan::HUMAN_RACE_ID),
                0.6,
            );
            assert_eq!(
                despawns_hit.first().map(|d| d.reason),
                Some(ProjectileDespawnReason::HitTarget),
                "humanoid 目标 z 偏移 0.6（< 换轨前固定阈值 0.7）必须命中，实测 {despawns_hit:?}"
            );

            let (_, _, despawns_miss) = run_lateral_projectile_at_real_target(
                crate::body_plan::RaceId::new(crate::body_plan::HUMAN_RACE_ID),
                0.8,
            );
            assert!(
                despawns_miss
                    .iter()
                    .all(|d| d.reason != ProjectileDespawnReason::HitTarget),
                "humanoid 目标 z 偏移 0.8（> 换轨前固定阈值 0.7）单 tick 内不应产生 HitTarget \
                 despawn，实测 {despawns_miss:?}"
            );
        }

        #[test]
        fn whale_target_bounding_radius_catches_offsets_that_would_miss_the_legacy_fixed_value() {
            // whale.json 粗筛半径 ≈5.74（tail_fin 局部 z=-3.74±half 2.0）远大于换轨前
            // 写死的 0.3——z 偏移 2.0 远超换轨前固定阈值 0.7（必定会被误判为未命中），
            // 换轨后 whale 目标必须能命中。
            let (wounds, combat_events, despawns) =
                run_lateral_projectile_at_real_target(crate::body_plan::RaceId::new("whale"), 2.0);
            assert_eq!(
                despawns.first().map(|d| d.reason),
                Some(ProjectileDespawnReason::HitTarget),
                "whale 目标 z 偏移 2.0（换轨前固定阈值 0.7 判不到，换轨后动态半径应判到）必须命中，\
                 实测 {despawns:?}"
            );
            assert_eq!(
                combat_events.len(),
                1,
                "whale 目标命中仍应发出恰好一条 CombatEvent，实测 {combat_events:?}"
            );
            assert!(
                wounds.health_current < Wounds::default().health_max,
                "whale 目标命中应造成伤害（health_current 低于满血），实测 {}",
                wounds.health_current
            );
        }
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

        // 再验冷却未设置：anqi.charge_carrier 应仍处于 ready 状态（tick=10）。
        let bindings = world.get::<SkillBarBindings>(caster).unwrap();
        assert!(
            !bindings.is_on_cooldown(ANQI_CHARGE_SKILL_ID, 10),
            "期望真元不足拒绝时不设置冷却（slot={slot} 应 ready），\
             实际冷却被烧掉了"
        );
    }
}
