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
use crate::combat::guard_log::GuardLogDedup;
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
    app.init_resource::<GuardLogDedup>();
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
            LifecycleState::AwaitingRevival | LifecycleState::Terminated
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
    mut stores: Query<(Entity, &mut CarrierStore)>,
    mut inventories: Query<&mut PlayerInventory>,
) {
    if !clock.tick.is_multiple_of(TICKS_PER_SECOND) {
        return;
    }
    for (entity, mut store) in &mut stores {
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
        if expired.is_empty() {
            continue;
        }
        for instance_id in &expired {
            store.imprints_by_instance.remove(instance_id);
        }
        if let Ok(mut inventory) = inventories.get_mut(entity) {
            for instance_id in expired {
                degrade_equipped_instance(&mut inventory, &registry, instance_id);
            }
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

type ThrowCarrierActorQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Position,
        &'static mut PlayerInventory,
        &'static mut CarrierStore,
        Option<&'static mut Stamina>,
        Option<&'static UniqueId>,
    ),
>;

fn throw_carrier_intents(
    clock: Res<CombatClock>,
    mut commands: Commands,
    mut intents: EventReader<ThrowCarrierIntent>,
    mut actors: ThrowCarrierActorQuery<'_, '_>,
    // 护栏 guard info! 按 (carrier, reason) 去重：e2e 场景只需一条关联标记，
    // 而任意连接可反复发 throw_carrier 空手请求——若每条都写 info 日志，
    // 无操作请求就被转换成无界日志输出。去重经共享资源 GuardLogDedup 按 tick
    // 窗口过期：窗口内每个 carrier×reason 至多一条（输出不随请求量增长），
    // 窗口外自动剪除（内存不随历史玩家无限增长）。review findings [major]：
    // 客户端可控抛射请求制造无界 info 日志路径 / dedup 表随历史玩家无限增长。
    mut guard_log: ResMut<GuardLogDedup>,
) {
    for intent in intents.read() {
        let Ok((position, mut inventory, mut store, stamina, unique_id)) =
            actors.get_mut(intent.thrower)
        else {
            continue;
        };
        let wire_id = entity_wire_id(unique_id, intent.thrower);
        let Some(item) = inventory
            .equipped
            .get(intent.slot.equip_key())
            .and_then(|s| s.held.as_ref())
        else {
            // e2e 空手护栏的正向证据：消费者系统（throw_carrier_intents）在「手槽
            // 无载体」早退处发信号。carrier=player:{uuid} 是该 bot 的线缆 id，场景
            // 用它把这条日志归属到自己的请求——不再依赖不唯一的 payload 字节数
            // （review findings [major]：throw 场景缺消费者信号 / 日志相关性）。
            // 系统未注册时此日志不出现，场景据此区分「护栏走了」与「消费者断线」。
            // 每个 (carrier, no_carrier_item) 只在窗口内发一次：场景只需一条，
            // 恶意客户端反复请求不能把 info 日志喂成无界输出（review finding
            // [major]）。
            if guard_log.should_emit(&wire_id, "no_carrier_item", clock.tick) {
                tracing::info!(
                    "[bong][combat] throw_carrier guard carrier={} slot={:?} reason=no_carrier_item",
                    wire_id,
                    intent.slot
                );
            }
            continue;
        };
        let Some(imprint) = store.imprints_by_instance.remove(&item.instance_id) else {
            // 与上同构：手槽有非暗器物品（新手村 fixture 主手通常是 iron_sword）
            // 但无 anqi 印记——空手护栏的另一条实测路径。去重语义同上。
            if guard_log.should_emit(&wire_id, "no_anqi_imprint", clock.tick) {
                tracing::info!(
                    "[bong][combat] throw_carrier guard carrier={} slot={:?} reason=no_anqi_imprint",
                    wire_id,
                    intent.slot
                );
            }
            continue;
        };
        let dir = normalized_dir(intent.dir_unit);
        // bughunt-20260726 carrier-throw-dir-nan-leak：`intent.dir_unit` 来自
        // C2S `ClientRequestV1::ThrowCarrier` 的 `[f32; 3]`，plain serde_json
        // 反序列化对越界字面量（如 JSON `1e40`）会 `as f32` 饱和成
        // `f32::INFINITY`，没有 parse 错误、没有范围校验。`normalized_dir` 对
        // (inf,0,0) 算出 `length_squared()=inf`（不是 NaN，`inf<=EPSILON` 为
        // false）从而跳过零向量早退，再 `normalize()` 内部 `inf * (1/inf)` =
        // `inf * 0.0` = NaN——`dir` 变成 (NaN,0,0)。此时 `NaN <= EPSILON`
        // 同样恒为 false（IEEE-754 NaN 比较全假），下面原有的零向量守卫被
        // 绕过而非触发，NaN 就此流入 velocity/spawn_pos，产生一个永不消亡、
        // 每 tick 写 NaN 位置的幽灵投射物（`projectile_tick_system` 里
        // `traveled > max_distance`、`qi_payload <= EPSILON` 两个退出判定
        // 全部因 NaN 比较恒假而失效）。显式拒绝非有限 `dir`，与零向量共用
        // 同一条 `continue` 分支（沿用既有守卫的既有语义，两者是同一处
        // "退化输入" 早退，不新引入行为分支）。
        if !dir.is_finite() || dir.length_squared() <= f64::EPSILON {
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
        // bughunt-20260726 carrier-throw-dir-nan-leak：防御性兜底。`dir` 现在
        // 已在 `throw_carrier_intents` 里被拒绝非有限值，本分支正常情况下
        // 不会触发；但保留它是为了防止任何未来直接构造
        // `AnqiProjectileFlight`（绕开 throw_carrier_intents）的生产者重新
        // 引入非有限 velocity/position 时，制造出同样的永生 NaN 实体——
        // `traveled > flight.max_distance` 和上面的 `qi_payload <= EPSILON`
        // 两个退出判定在 NaN 面前都会因 IEEE-754 比较恒假而失效，只有显式
        // `is_finite()` 检查能兜底。用 `current`（本 tick 前最后一个已知
        // 有限位置）而非 `next`/`traveled` 本身作为 despawn 的 `pos`：避免
        // 把 NaN 传进 `emit_projectile_despawn` 的距离/衰减计算——NaN 落点
        // 会让 `ZoneRegistry::find_zone` 的 AABB 比较恒假从而找不到任何
        // zone，把真元错误地转入 overflow 账户（真元本身不会凭空消失，但
        // 会丢失真实落点归属，且 NaN 还会被序列化进
        // `ProjectileDespawnedEvent.pos` 传给下游 Redis 桥接消费者）。
        if !next.is_finite() || !traveled.is_finite() {
            emit_projectile_despawn(
                &mut commands,
                &mut despawned,
                ProjectileDespawnArgs {
                    projectile_entity,
                    projectile: &projectile,
                    flight: &flight,
                    reason: ProjectileDespawnReason::OutOfRange,
                    pos: current,
                    tick: clock.tick,
                },
            );
            continue;
        }
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
#[path = "carrier_tests.rs"]
mod tests;
