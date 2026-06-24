use serde::{Deserialize, Serialize};
use valence::prelude::{
    bevy_ecs, Commands, DVec3, Entity, Event, EventReader, EventWriter, IntoSystemConfigs,
    ParamSet, Position, Query, Res, ResMut, UniqueId,
};

use crate::combat::components::{
    ActiveStatusEffect, DerivedAttrs, SkillBarBindings, StatusEffects, TICKS_PER_SECOND,
};
use crate::combat::events::{ApplyStatusEffectIntent, StatusEffectKind};
use crate::combat::projectile::QiProjectile;
use crate::combat::status::{has_active_status, remove_status_effect, upsert_status_effect};
use crate::combat::{CombatClock, CombatSystemSet};
use crate::cultivation::color::{record_style_practice, PracticeLog};
use crate::cultivation::components::{
    ColorKind, Cultivation, MeridianId, MeridianSystem, QiColor, Realm,
};
use crate::cultivation::life_record::{BiographyEntry, LifeRecord};
use crate::cultivation::skill_registry::{CastRejectReason, CastResult, SkillRegistry};
use crate::qi_physics::constants::{QI_EPSILON, QI_ZONE_UNIT_CAPACITY};
use crate::qi_physics::{
    qi_negative_field_drain_ratio, qi_release_to_zone, qi_woliu_vortex_field_strength_for_realm,
    MediumKind, QiAccountId, QiPhysicsError, QiTransfer, QiTransferReason, StyleAttack,
    ZoneReleaseOutcome,
};
use crate::schema::cultivation::meridian_id_to_string;
use crate::schema::woliu::{
    ProjectileQiDrainedEventV1, VortexBackfireCauseV1, VortexBackfireEventV1, VortexFieldStateV1,
};
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::zone::ZoneRegistry;

pub const WOLIU_VORTEX_SKILL_ID: &str = "woliu.vortex";
pub const VORTEX_CASTING_MAGNITUDE: f32 = 1.0;
pub const VORTEX_CASTING_DURATION_TICKS: u64 = u64::MAX;
pub const VORTEX_COOLDOWN_TICKS: u64 = TICKS_PER_SECOND;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VortexToggle {
    On,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VortexCastSource {
    SkillBar,
    System,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct VortexCastIntent {
    pub caster: Entity,
    pub toggle: VortexToggle,
    pub source: VortexCastSource,
    pub issued_at_tick: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackfireCause {
    EnvQiTooLow,
    ExceedMaintainMax,
    ExceedDeltaCap,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct VortexBackfireEvent {
    pub caster: Entity,
    pub cause: BackfireCause,
    pub meridian_severed: MeridianId,
    pub tick: u64,
    pub env_qi: f32,
    pub delta: f32,
    pub resisted: bool,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct ProjectileQiDrainedEvent {
    pub field_caster: Entity,
    pub projectile: Entity,
    pub owner: Option<Entity>,
    pub drained_amount: f32,
    pub remaining_payload: f32,
    pub delta: f32,
    pub tick: u64,
}

#[derive(bevy_ecs::component::Component, Debug, Clone, Copy, PartialEq)]
pub struct VortexField {
    pub center: DVec3,
    pub radius: f32,
    pub delta: f32,
    pub cast_at_tick: u64,
    pub maintain_max_ticks: u64,
    pub caster: Entity,
    pub env_qi_at_cast: f32,
    pub last_maintain_tick: u64,
}

impl StyleAttack for VortexField {
    fn style_color(&self) -> ColorKind {
        ColorKind::Intricate
    }

    fn injected_qi(&self) -> f64 {
        f64::from(self.delta.max(0.0))
    }

    fn purity(&self) -> f64 {
        (1.0 - f64::from(self.radius / 32.0).clamp(0.0, 0.5)).clamp(0.5, 1.0)
    }

    fn rejection_rate(&self) -> f64 {
        0.30
    }

    fn medium(&self) -> MediumKind {
        MediumKind::bare(ColorKind::Intricate)
    }
}

#[derive(bevy_ecs::component::Component, Debug, Clone, Copy, PartialEq)]
pub struct QiNeedle {
    pub owner: Option<Entity>,
    pub qi_payload: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VortexResolveOutcome {
    Activated(VortexField),
    Deactivated,
    Rejected(CastRejectReason),
    Backfired(BackfireCause),
}

type VortexActorItem<'a> = (
    &'a mut Cultivation,
    &'a mut StatusEffects,
    &'a mut DerivedAttrs,
    Option<&'a mut MeridianSystem>,
    Option<&'a mut LifeRecord>,
    Option<&'a CurrentDimension>,
    &'a Position,
    Option<&'a VortexField>,
);

pub fn register(app: &mut valence::prelude::App) {
    app.add_event::<VortexCastIntent>();
    app.add_event::<VortexBackfireEvent>();
    app.add_event::<ProjectileQiDrainedEvent>();
    app.add_event::<QiTransfer>();
    app.add_systems(
        valence::prelude::Update,
        (
            cast_vortex.in_set(CombatSystemSet::Intent),
            vortex_intercept_tick.in_set(CombatSystemSet::Physics),
            vortex_maintain_tick.in_set(CombatSystemSet::Physics),
        ),
    );
}

pub fn register_skills(registry: &mut SkillRegistry) {
    registry.register(WOLIU_VORTEX_SKILL_ID, resolve_woliu_vortex_skill);
}

pub fn declare_meridian_dependencies(
    dependencies: &mut crate::cultivation::meridian::severed::SkillMeridianDependencies,
) {
    dependencies.declare(WOLIU_VORTEX_SKILL_ID, vec![MeridianId::Lung]);
}

pub fn resolve_woliu_vortex_skill(
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
        .get::<SkillBarBindings>(caster)
        .is_some_and(|bindings| bindings.is_on_cooldown(slot, now_tick))
    {
        return rejected(CastRejectReason::OnCooldown);
    }

    // GAP-1 修复：check Lung 永久断脉 gate（woliu.vortex 依赖肺经调动涡流真元）。
    let severed =
        world.get::<crate::cultivation::meridian::severed::MeridianSeveredPermanent>(caster);
    if let Err(blocked) = crate::cultivation::meridian::severed::check_meridian_dependencies(
        &[MeridianId::Lung],
        severed,
    ) {
        return rejected(CastRejectReason::MeridianSevered(Some(blocked)));
    }

    let toggle = if world.get::<VortexField>(caster).is_some() {
        VortexToggle::Off
    } else {
        VortexToggle::On
    };
    let outcome = resolve_vortex_toggle_in_world(world, caster, toggle, now_tick);
    match outcome {
        VortexResolveOutcome::Activated(_) | VortexResolveOutcome::Deactivated => {
            if let Some(mut bindings) = world.get_mut::<SkillBarBindings>(caster) {
                bindings.set_cooldown(slot, now_tick.saturating_add(VORTEX_COOLDOWN_TICKS));
            }
            CastResult::Started {
                cooldown_ticks: VORTEX_COOLDOWN_TICKS,
                anim_duration_ticks: 1,
            }
        }
        VortexResolveOutcome::Rejected(reason) => rejected(reason),
        VortexResolveOutcome::Backfired(_) => {
            if let Some(mut bindings) = world.get_mut::<SkillBarBindings>(caster) {
                bindings.set_cooldown(slot, now_tick.saturating_add(VORTEX_COOLDOWN_TICKS));
            }
            CastResult::Started {
                cooldown_ticks: VORTEX_COOLDOWN_TICKS,
                anim_duration_ticks: 1,
            }
        }
    }
}

pub fn cast_vortex(
    clock: Res<CombatClock>,
    mut intents: EventReader<VortexCastIntent>,
    mut commands: Commands,
    zones: Option<Res<ZoneRegistry>>,
    mut actors: Query<VortexActorItem<'_>>,
    mut status_intents: EventWriter<ApplyStatusEffectIntent>,
    mut backfires: EventWriter<VortexBackfireEvent>,
) {
    for intent in intents.read() {
        let Ok((
            mut cultivation,
            mut statuses,
            mut attrs,
            meridians,
            life_record,
            dimension,
            position,
            field,
        )) = actors.get_mut(intent.caster)
        else {
            continue;
        };

        let outcome = resolve_vortex_toggle_parts(
            &mut commands,
            intent.caster,
            &mut cultivation,
            &mut statuses,
            &mut attrs,
            meridians,
            life_record,
            dimension,
            position,
            field,
            zones.as_deref(),
            intent.toggle,
            clock.tick,
            Some(&mut status_intents),
            Some(&mut backfires),
        );
        tracing::debug!(
            "[bong][woliu] intent caster={:?} toggle={:?} source={:?} outcome={:?}",
            intent.caster,
            intent.toggle,
            intent.source,
            outcome
        );
    }
}

pub fn resolve_vortex_toggle_in_world(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    toggle: VortexToggle,
    now_tick: u64,
) -> VortexResolveOutcome {
    let Some(position) = world.get::<Position>(caster).copied() else {
        return VortexResolveOutcome::Rejected(CastRejectReason::InvalidTarget);
    };
    let dimension = world.get::<CurrentDimension>(caster).copied();
    let existing_field = world.get::<VortexField>(caster).copied();
    let env_qi = current_env_qi(
        world.get_resource::<ZoneRegistry>(),
        dimension.as_ref(),
        &position,
    );

    if toggle == VortexToggle::Off {
        world.entity_mut(caster).remove::<VortexField>();
        if let Some(mut statuses) = world.get_mut::<StatusEffects>(caster) {
            remove_status_effect(&mut statuses, StatusEffectKind::VortexCasting);
        }
        if let Some(mut attrs) = world.get_mut::<DerivedAttrs>(caster) {
            attrs.vortex_active = false;
        }
        return VortexResolveOutcome::Deactivated;
    }

    if existing_field.is_some() {
        return VortexResolveOutcome::Rejected(CastRejectReason::InRecovery);
    }

    let Some(cultivation) = world.get::<Cultivation>(caster).cloned() else {
        return VortexResolveOutcome::Rejected(CastRejectReason::RealmTooLow);
    };
    let delta = vortex_delta_for_realm(cultivation.realm);
    if delta <= f32::EPSILON {
        return VortexResolveOutcome::Rejected(CastRejectReason::RealmTooLow);
    }

    if env_qi < delta {
        let meridian = pick_hand_meridian(cultivation.realm);
        if let Some(mut meridians) = world.get_mut::<MeridianSystem>(caster) {
            sever_meridian(&mut meridians, meridian);
        }
        let qi_max = 10.0
            + world
                .get::<MeridianSystem>(caster)
                .map(MeridianSystem::sum_capacity)
                .unwrap_or_default();
        if let Some(mut cultivation) = world.get_mut::<Cultivation>(caster) {
            cultivation.qi_max = qi_max;
            cultivation.qi_current = cultivation.qi_current.clamp(0.0, cultivation.qi_max);
        }
        if let Some(mut attrs) = world.get_mut::<DerivedAttrs>(caster) {
            attrs.vortex_active = false;
        }
        world.send_event(VortexBackfireEvent {
            caster,
            cause: BackfireCause::EnvQiTooLow,
            meridian_severed: meridian,
            tick: now_tick,
            env_qi,
            delta,
            resisted: false,
        });
        return VortexResolveOutcome::Backfired(BackfireCause::EnvQiTooLow);
    }

    let field = VortexField {
        center: position.get(),
        radius: vortex_radius_for_realm(cultivation.realm),
        delta,
        cast_at_tick: now_tick,
        maintain_max_ticks: vortex_maintain_max_ticks_for_realm(cultivation.realm),
        caster,
        env_qi_at_cast: env_qi,
        last_maintain_tick: now_tick,
    };
    if let Some(mut statuses) = world.get_mut::<StatusEffects>(caster) {
        upsert_status_effect(
            &mut statuses,
            ActiveStatusEffect {
                kind: StatusEffectKind::VortexCasting,
                magnitude: VORTEX_CASTING_MAGNITUDE,
                remaining_ticks: VORTEX_CASTING_DURATION_TICKS,
                source_pill: None,
            },
        );
    }
    if let Some(mut attrs) = world.get_mut::<DerivedAttrs>(caster) {
        attrs.vortex_active = true;
    }
    world.entity_mut(caster).insert(field);
    VortexResolveOutcome::Activated(field)
}

#[allow(clippy::too_many_arguments)]
fn resolve_vortex_toggle_parts(
    commands: &mut Commands,
    caster: Entity,
    cultivation: &mut Cultivation,
    statuses: &mut StatusEffects,
    attrs: &mut DerivedAttrs,
    meridians: Option<valence::prelude::Mut<'_, MeridianSystem>>,
    life_record: Option<valence::prelude::Mut<'_, LifeRecord>>,
    dimension: Option<&CurrentDimension>,
    position: &Position,
    field: Option<&VortexField>,
    zones: Option<&ZoneRegistry>,
    toggle: VortexToggle,
    now_tick: u64,
    mut status_intents: Option<&mut EventWriter<ApplyStatusEffectIntent>>,
    mut backfires: Option<&mut EventWriter<VortexBackfireEvent>>,
) -> VortexResolveOutcome {
    if toggle == VortexToggle::Off {
        if field.is_none() {
            return VortexResolveOutcome::Deactivated;
        }
        commands.entity(caster).remove::<VortexField>();
        attrs.vortex_active = false;
        remove_status_effect(statuses, StatusEffectKind::VortexCasting);
        if let Some(status_intents) = &mut status_intents {
            status_intents.send(ApplyStatusEffectIntent {
                target: caster,
                kind: StatusEffectKind::VortexCasting,
                magnitude: 0.0,
                duration_ticks: 0,
                issued_at_tick: now_tick,
            });
        }
        return VortexResolveOutcome::Deactivated;
    }

    if field.is_some() {
        return VortexResolveOutcome::Rejected(CastRejectReason::InRecovery);
    }

    let delta = vortex_delta_for_realm(cultivation.realm);
    if delta <= f32::EPSILON {
        return VortexResolveOutcome::Rejected(CastRejectReason::RealmTooLow);
    }

    let env_qi = current_env_qi(zones, dimension, position);
    if env_qi < delta {
        let meridian = pick_hand_meridian(cultivation.realm);
        if let Some(mut meridians) = meridians {
            sever_meridian(&mut meridians, meridian);
            cultivation.qi_max = 10.0 + meridians.sum_capacity();
            cultivation.qi_current = cultivation.qi_current.clamp(0.0, cultivation.qi_max);
        }
        attrs.vortex_active = false;
        if let Some(mut life_record) = life_record {
            record_vortex_backfire(&mut life_record, BackfireCause::EnvQiTooLow, now_tick);
        }
        let event = VortexBackfireEvent {
            caster,
            cause: BackfireCause::EnvQiTooLow,
            meridian_severed: meridian,
            tick: now_tick,
            env_qi,
            delta,
            resisted: false,
        };
        if let Some(backfires) = &mut backfires {
            backfires.send(event);
        }
        return VortexResolveOutcome::Backfired(BackfireCause::EnvQiTooLow);
    }

    let field = VortexField {
        center: position.get(),
        radius: vortex_radius_for_realm(cultivation.realm),
        delta,
        cast_at_tick: now_tick,
        maintain_max_ticks: vortex_maintain_max_ticks_for_realm(cultivation.realm),
        caster,
        env_qi_at_cast: env_qi,
        last_maintain_tick: now_tick,
    };
    upsert_status_effect(
        statuses,
        ActiveStatusEffect {
            kind: StatusEffectKind::VortexCasting,
            magnitude: VORTEX_CASTING_MAGNITUDE,
            remaining_ticks: VORTEX_CASTING_DURATION_TICKS,
            source_pill: None,
        },
    );
    attrs.vortex_active = true;
    if let Some(status_intents) = &mut status_intents {
        status_intents.send(ApplyStatusEffectIntent {
            target: caster,
            kind: StatusEffectKind::VortexCasting,
            magnitude: VORTEX_CASTING_MAGNITUDE,
            duration_ticks: VORTEX_CASTING_DURATION_TICKS,
            issued_at_tick: now_tick,
        });
    }
    commands.entity(caster).insert(field);
    VortexResolveOutcome::Activated(field)
}

type ProjectileDrainItem<'a> = (Entity, &'a Position, &'a mut QiProjectile);
type NeedleDrainItem<'a> = (Entity, &'a Position, &'a mut QiNeedle);

pub fn vortex_intercept_tick(
    clock: Res<CombatClock>,
    fields: Query<&VortexField>,
    mut projectiles: Query<ProjectileDrainItem<'_>>,
    mut needles: Query<NeedleDrainItem<'_>>,
    mut drain_events: ParamSet<(
        EventWriter<ProjectileQiDrainedEvent>,
        EventWriter<QiTransfer>,
    )>,
    mut life_records: Query<&mut LifeRecord>,
    mut practice_logs: Query<(&mut PracticeLog, Option<&QiColor>)>,
) {
    if fields.is_empty() {
        return;
    }

    for (projectile_entity, position, mut projectile) in &mut projectiles {
        if let Some((caster, delta, drain_ratio)) =
            vortex_aggregate_at(position.get(), fields.iter())
        {
            let drained = projected_qi_drain(projectile.qi_payload, drain_ratio);
            if drained > f32::EPSILON {
                if let Err(error) = record_vortex_qi_transfer(
                    &mut drain_events.p1(),
                    projectile_entity,
                    caster,
                    drained,
                ) {
                    tracing::warn!(
                        ?projectile_entity,
                        ?caster,
                        drained,
                        error = %error,
                        "[bong][woliu] failed to record vortex qi transfer"
                    );
                    continue;
                }
                drain_qi_payload(&mut projectile.qi_payload, drain_ratio);
                record_projectile_drain(
                    &mut life_records,
                    caster,
                    projectile_entity,
                    drained,
                    clock.tick,
                );
                record_vortex_practice(&mut practice_logs, caster);
                drain_events.p0().send(ProjectileQiDrainedEvent {
                    field_caster: caster,
                    projectile: projectile_entity,
                    owner: projectile.owner,
                    drained_amount: drained,
                    remaining_payload: projectile.qi_payload,
                    delta,
                    tick: clock.tick,
                });
            }
        }
    }

    for (needle_entity, position, mut needle) in &mut needles {
        if let Some((caster, delta, drain_ratio)) =
            vortex_aggregate_at(position.get(), fields.iter())
        {
            let drained = projected_qi_drain(needle.qi_payload, drain_ratio);
            if drained > f32::EPSILON {
                if let Err(error) = record_vortex_qi_transfer(
                    &mut drain_events.p1(),
                    needle_entity,
                    caster,
                    drained,
                ) {
                    tracing::warn!(
                        ?needle_entity,
                        ?caster,
                        drained,
                        error = %error,
                        "[bong][woliu] failed to record vortex qi transfer"
                    );
                    continue;
                }
                drain_qi_payload(&mut needle.qi_payload, drain_ratio);
                record_projectile_drain(
                    &mut life_records,
                    caster,
                    needle_entity,
                    drained,
                    clock.tick,
                );
                record_vortex_practice(&mut practice_logs, caster);
                drain_events.p0().send(ProjectileQiDrainedEvent {
                    field_caster: caster,
                    projectile: needle_entity,
                    owner: needle.owner,
                    drained_amount: drained,
                    remaining_payload: needle.qi_payload,
                    delta,
                    tick: clock.tick,
                });
            }
        }
    }
}

type MaintainActorItem<'a> = (
    Entity,
    &'a mut Cultivation,
    &'a mut MeridianSystem,
    &'a mut StatusEffects,
    &'a mut DerivedAttrs,
    &'a mut VortexField,
    Option<&'a mut LifeRecord>,
    Option<&'a Position>,
    Option<&'a CurrentDimension>,
);

pub fn vortex_maintain_tick(
    clock: Res<CombatClock>,
    mut commands: Commands,
    mut actors: Query<MaintainActorItem<'_>>,
    mut status_intents: EventWriter<ApplyStatusEffectIntent>,
    mut backfires: EventWriter<VortexBackfireEvent>,
    mut zones: Option<ResMut<ZoneRegistry>>,
    mut qi_transfers: EventWriter<QiTransfer>,
) {
    for (
        entity,
        mut cultivation,
        mut meridians,
        mut statuses,
        mut attrs,
        mut field,
        life_record,
        position,
        dimension,
    ) in &mut actors
    {
        if clock.tick.saturating_sub(field.last_maintain_tick) >= TICKS_PER_SECOND {
            let elapsed_seconds =
                clock.tick.saturating_sub(field.last_maintain_tick) / TICKS_PER_SECOND;
            field.last_maintain_tick = clock.tick;
            let cost = vortex_qi_cost_per_sec(cultivation.realm) * elapsed_seconds as f64;
            let before = cultivation.qi_current;
            cultivation.qi_current = (cultivation.qi_current - cost).clamp(0.0, cultivation.qi_max);
            let actual_cost = (before - cultivation.qi_current).max(0.0);
            if actual_cost > QI_EPSILON {
                vortex_maintain_release_to_zone(
                    entity,
                    actual_cost,
                    position.map(|p| p.get()),
                    dimension.map(|d| d.0).unwrap_or(DimensionKind::Overworld),
                    zones.as_deref_mut(),
                    &mut qi_transfers,
                );
            }
            if cultivation.qi_current <= f64::EPSILON {
                close_vortex_without_backfire(
                    &mut commands,
                    &mut status_intents,
                    entity,
                    &mut statuses,
                    &mut attrs,
                    clock.tick,
                );
                continue;
            }
        }

        if clock.tick.saturating_sub(field.cast_at_tick) > field.maintain_max_ticks {
            let meridian = pick_hand_meridian(cultivation.realm);
            let resisted = check_backfire_resistance(&statuses);
            if !resisted {
                sever_meridian(&mut meridians, meridian);
                cultivation.qi_max = 10.0 + meridians.sum_capacity();
                cultivation.qi_current = cultivation.qi_current.clamp(0.0, cultivation.qi_max);
            }
            if let Some(mut life_record) = life_record {
                record_vortex_backfire(
                    &mut life_record,
                    BackfireCause::ExceedMaintainMax,
                    clock.tick,
                );
            }
            close_vortex_without_backfire(
                &mut commands,
                &mut status_intents,
                entity,
                &mut statuses,
                &mut attrs,
                clock.tick,
            );
            backfires.send(VortexBackfireEvent {
                caster: entity,
                cause: BackfireCause::ExceedMaintainMax,
                meridian_severed: meridian,
                tick: clock.tick,
                env_qi: field.env_qi_at_cast,
                delta: field.delta,
                resisted,
            });
        }
    }
}

fn close_vortex_without_backfire(
    commands: &mut Commands,
    status_intents: &mut EventWriter<ApplyStatusEffectIntent>,
    entity: Entity,
    statuses: &mut StatusEffects,
    attrs: &mut DerivedAttrs,
    tick: u64,
) {
    commands.entity(entity).remove::<VortexField>();
    remove_status_effect(statuses, StatusEffectKind::VortexCasting);
    attrs.vortex_active = false;
    status_intents.send(ApplyStatusEffectIntent {
        target: entity,
        kind: StatusEffectKind::VortexCasting,
        magnitude: 0.0,
        duration_ticks: 0,
        issued_at_tick: tick,
    });
}

pub fn vortex_delta_for_realm(realm: Realm) -> f32 {
    qi_woliu_vortex_field_strength_for_realm(realm) as f32
}

pub fn vortex_radius_for_realm(realm: Realm) -> f32 {
    match realm {
        Realm::Awaken => 0.0,
        Realm::Induce => 1.0,
        Realm::Condense => 1.5,
        Realm::Solidify => 2.0,
        Realm::Spirit => 3.0,
        Realm::Void => 3.0,
    }
}

pub fn vortex_maintain_max_ticks_for_realm(realm: Realm) -> u64 {
    let seconds = match realm {
        Realm::Awaken => 0,
        Realm::Induce => 2,
        Realm::Condense => 5,
        Realm::Solidify => 8,
        Realm::Spirit => 12,
        Realm::Void => 12,
    };
    seconds * TICKS_PER_SECOND
}

pub fn vortex_qi_cost_per_sec(realm: Realm) -> f64 {
    match realm {
        Realm::Awaken => 0.0,
        Realm::Induce => 5.0,
        Realm::Condense => 6.0,
        Realm::Solidify => 8.0,
        Realm::Spirit => 10.0,
        Realm::Void => 10.0,
    }
}

pub fn vortex_aggregate_at<'a>(
    position: DVec3,
    fields: impl Iterator<Item = &'a VortexField>,
) -> Option<(Entity, f32, f32)> {
    fields
        .filter_map(|field| {
            let distance = position.distance(field.center);
            if distance > f64::from(field.radius) {
                return None;
            }
            let ratio = qi_negative_field_drain_ratio(f64::from(field.delta), distance);
            Some((field, distance, ratio))
        })
        .max_by(
            |(left, left_distance, left_ratio), (right, right_distance, right_ratio)| {
                left_ratio
                    .total_cmp(right_ratio)
                    .then_with(|| right_distance.total_cmp(left_distance))
                    .then_with(|| left.delta.total_cmp(&right.delta))
                    .then_with(|| right.caster.to_bits().cmp(&left.caster.to_bits()))
            },
        )
        .map(|(field, _, ratio)| (field.caster, field.delta, ratio as f32))
}

pub fn drain_qi_payload(qi_payload: &mut f32, drain_ratio: f32) -> f32 {
    if !qi_payload.is_finite() || *qi_payload <= 0.0 {
        *qi_payload = 0.0;
        return 0.0;
    }
    let drained = projected_qi_drain(*qi_payload, drain_ratio);
    *qi_payload = (*qi_payload - drained).max(0.0);
    drained
}

fn projected_qi_drain(qi_payload: f32, drain_ratio: f32) -> f32 {
    if !qi_payload.is_finite() || qi_payload <= 0.0 || !drain_ratio.is_finite() {
        return 0.0;
    }
    let ratio = drain_ratio.clamp(0.0, 1.0);
    (qi_payload * ratio).clamp(0.0, qi_payload)
}

pub fn ambient_qi_perception(
    previous_qi: f32,
    current_qi: f32,
    has_inspect_skill: bool,
) -> Option<String> {
    if has_inspect_skill {
        return Some(format!("灵气浓度: {current_qi:.2}"));
    }

    let ratio = current_qi / previous_qi.max(0.01);
    if ratio > 1.5 {
        Some("此地灵气骤然浓郁，呼吸间元气盈满".to_string())
    } else if ratio > 1.2 {
        Some("似觉灵气稍浓".to_string())
    } else if (0.8..=1.2).contains(&ratio) {
        None
    } else if ratio >= 0.5 {
        Some("灵气稀薄，引气如吸沙".to_string())
    } else {
        Some("灵气几近断绝，此地有不祥预感".to_string())
    }
}

pub fn pick_hand_meridian(_realm: Realm) -> MeridianId {
    MeridianId::Lung
}

pub fn sever_meridian(meridians: &mut MeridianSystem, id: MeridianId) {
    let meridian = meridians.get_mut(id);
    meridian.flow_capacity = 0.0;
    meridian.opened = false;
    meridian.integrity = 0.0;
}

pub fn check_backfire_resistance(statuses: &StatusEffects) -> bool {
    has_active_status(statuses, StatusEffectKind::AntiSpiritPressurePill)
}

pub fn entity_wire_id(unique_id: Option<&UniqueId>, entity: Entity) -> String {
    unique_id
        .map(|unique_id| format!("player:{}", unique_id.0))
        .unwrap_or_else(|| format!("entity:{}", entity.to_bits()))
}

pub fn backfire_cause_payload(cause: BackfireCause) -> VortexBackfireCauseV1 {
    match cause {
        BackfireCause::EnvQiTooLow => VortexBackfireCauseV1::EnvQiTooLow,
        BackfireCause::ExceedMaintainMax => VortexBackfireCauseV1::ExceedMaintainMax,
        BackfireCause::ExceedDeltaCap => VortexBackfireCauseV1::ExceedDeltaCap,
    }
}

pub fn vortex_field_state_payload(
    caster: String,
    field: Option<&VortexField>,
    now_tick: u64,
    intercepted_count: u32,
) -> VortexFieldStateV1 {
    let Some(field) = field else {
        return VortexFieldStateV1 {
            caster,
            active: false,
            center: [0.0, 0.0, 0.0],
            radius: 0.0,
            delta: 0.0,
            env_qi_at_cast: 0.0,
            maintain_remaining_ticks: 0,
            intercepted_count,
            active_skill_id: String::new(),
            charge_progress: 0.0,
            cooldown_until_ms: 0,
            backfire_level: String::new(),
            turbulence_radius: 0.0,
            turbulence_intensity: 0.0,
            turbulence_until_ms: 0,
        };
    };
    let elapsed = now_tick.saturating_sub(field.cast_at_tick);
    let remaining = field.maintain_max_ticks.saturating_sub(elapsed);
    VortexFieldStateV1 {
        caster,
        active: true,
        center: [field.center.x, field.center.y, field.center.z],
        radius: field.radius,
        delta: field.delta,
        env_qi_at_cast: field.env_qi_at_cast,
        maintain_remaining_ticks: remaining,
        intercepted_count,
        active_skill_id: String::new(),
        charge_progress: if remaining > 0 { 1.0 } else { 0.0 },
        cooldown_until_ms: 0,
        backfire_level: String::new(),
        turbulence_radius: 0.0,
        turbulence_intensity: 0.0,
        turbulence_until_ms: 0,
    }
}

pub fn vortex_backfire_payload(
    event: &VortexBackfireEvent,
    caster_unique_id: Option<&UniqueId>,
) -> VortexBackfireEventV1 {
    VortexBackfireEventV1 {
        caster: entity_wire_id(caster_unique_id, event.caster),
        cause: backfire_cause_payload(event.cause),
        meridian_severed: meridian_id_to_string(event.meridian_severed).to_string(),
        tick: event.tick,
        env_qi: event.env_qi,
        delta: event.delta,
        resisted: event.resisted,
    }
}

pub fn projectile_drained_payload(
    event: &ProjectileQiDrainedEvent,
    caster_unique_id: Option<&UniqueId>,
    projectile_unique_id: Option<&UniqueId>,
    owner_unique_id: Option<&UniqueId>,
) -> ProjectileQiDrainedEventV1 {
    ProjectileQiDrainedEventV1 {
        field_caster: entity_wire_id(caster_unique_id, event.field_caster),
        projectile: entity_wire_id(projectile_unique_id, event.projectile),
        owner: event
            .owner
            .map(|owner| entity_wire_id(owner_unique_id, owner)),
        drained_amount: event.drained_amount,
        remaining_payload: event.remaining_payload,
        delta: event.delta,
        tick: event.tick,
    }
}

fn current_env_qi(
    zones: Option<&ZoneRegistry>,
    dimension: Option<&CurrentDimension>,
    position: &Position,
) -> f32 {
    let dimension = dimension
        .map(|dimension| dimension.0)
        .unwrap_or(DimensionKind::Overworld);
    zones
        .and_then(|zones| zones.find_zone(dimension, position.get()))
        .map(|zone| zone.spirit_qi as f32)
        .unwrap_or(0.9)
}

fn record_projectile_drain(
    life_records: &mut Query<&mut LifeRecord>,
    caster: Entity,
    projectile: Entity,
    drained: f32,
    tick: u64,
) {
    let Ok(mut record) = life_records.get_mut(caster) else {
        return;
    };
    record.push(BiographyEntry::VortexProjectileDrained {
        projectile_id: format!("entity:{}", projectile.to_bits()),
        drained_amount: drained,
        tick,
    });
}

fn record_vortex_practice(
    practice_logs: &mut Query<(&mut PracticeLog, Option<&QiColor>)>,
    caster: Entity,
) {
    if let Ok((mut practice_log, qi_color)) = practice_logs.get_mut(caster) {
        record_style_practice(&mut practice_log, ColorKind::Intricate, qi_color);
    }
}

fn record_vortex_qi_transfer(
    transfers: &mut EventWriter<QiTransfer>,
    projectile: Entity,
    field_caster: Entity,
    drained: f32,
) -> Result<(), QiPhysicsError> {
    if let Some(transfer) = build_vortex_qi_transfer(projectile, field_caster, drained)? {
        transfers.send(transfer);
    }
    Ok(())
}

fn build_vortex_qi_transfer(
    projectile: Entity,
    field_caster: Entity,
    drained: f32,
) -> Result<Option<QiTransfer>, QiPhysicsError> {
    let amount = f64::from(drained);
    if amount <= f64::EPSILON {
        return Ok(None);
    }
    let transfer = QiTransfer::new(
        QiAccountId::container(format!("projectile:{}", projectile.to_bits())),
        QiAccountId::zone(format!("woliu_vortex:{}", field_caster.to_bits())),
        amount,
        QiTransferReason::Collision,
    )?;
    Ok(Some(transfer))
}

fn record_vortex_backfire(record: &mut LifeRecord, cause: BackfireCause, tick: u64) {
    record.push(BiographyEntry::VortexBackfired {
        cause: format!("{cause:?}"),
        tick,
    });
}

fn rejected(reason: CastRejectReason) -> CastResult {
    CastResult::Rejected { reason }
}

/// Release the per-tick vortex maintenance cost back to the caster's zone.
///
/// Mirrors the pattern established by `zhenmai_v2::drain_release_to_zone` — every
/// `qi_current` deduction inside a tick-driven system must emit a corresponding
/// [`QiTransfer`] with [`QiTransferReason::ReleaseToZone`] so the ledger stays
/// conserved.
fn vortex_maintain_release_to_zone(
    entity: Entity,
    cost: f64,
    position: Option<DVec3>,
    dimension: DimensionKind,
    zones: Option<&mut ZoneRegistry>,
    qi_transfers: &mut EventWriter<QiTransfer>,
) {
    if cost <= QI_EPSILON {
        return;
    }
    let from = QiAccountId::player(format!("entity:{}", entity.to_bits()));
    for transfer in
        build_vortex_maintain_zone_transfer(from, cost, position, dimension, zones, entity)
    {
        qi_transfers.send(transfer);
    }
}

/// 返回 `Vec`：zone 部分饱和时同时含 ① zone transfer（accepted）和 ② overflow transfer
/// （`outcome.overflow`，路由 overflow 账户）；无 zone / ZoneRegistry 缺失 / Err / zone 满时返回
/// 单条全额 overflow transfer。accepted+overflow==amount，任何 qi 都不蒸发（守恒，CodeRabbit #693）。
fn build_vortex_maintain_zone_transfer(
    from: QiAccountId,
    amount: f64,
    position: Option<DVec3>,
    dimension: DimensionKind,
    zones: Option<&mut ZoneRegistry>,
    entity: Entity,
) -> Vec<QiTransfer> {
    if amount <= QI_EPSILON {
        return Vec::new();
    }
    let overflow_account =
        || QiAccountId::overflow(format!("woliu.vortex_maintain:{}", entity.to_bits()));
    if let (Some(position), Some(zones)) = (position, zones) {
        let zone_name = zones
            .find_zone(dimension, position)
            .map(|zone| zone.name.clone());
        if let Some(zone_name) = zone_name {
            if let Some(zone) = zones.find_zone_mut(zone_name.as_str()) {
                let to = QiAccountId::zone(zone.name.clone());
                // 不 .max(0.0)：负灵域（spirit_qi<0）下当 0 会抹掉负缺口、凭空多 credit、破坏守恒。
                // 与规范 helper death_hooks::release_qi_amount_to_zone 一致用裸 spirit_qi*CAP。
                let zone_current = zone.spirit_qi * QI_ZONE_UNIT_CAPACITY;
                match qi_release_to_zone(
                    amount,
                    from.clone(),
                    to,
                    zone_current,
                    QI_ZONE_UNIT_CAPACITY,
                ) {
                    Ok(ZoneReleaseOutcome {
                        zone_after,
                        transfer: Some(transfer),
                        overflow,
                        ..
                    }) => {
                        zone.spirit_qi = (zone_after / QI_ZONE_UNIT_CAPACITY).clamp(-1.0, 1.0);
                        let mut transfers = vec![transfer];
                        // zone 部分饱和：剩余 overflow 显式入 overflow 账户，绝不静默丢弃。
                        if overflow > QI_EPSILON {
                            if let Ok(overflow_transfer) = QiTransfer::new(
                                from,
                                overflow_account(),
                                overflow,
                                QiTransferReason::ReleaseToZone,
                            ) {
                                transfers.push(overflow_transfer);
                            }
                        }
                        return transfers;
                    }
                    Ok(_) => {
                        // zone capped, overflow-only — fall through to overflow sink
                    }
                    Err(error) => {
                        tracing::warn!(
                            ?error,
                            "[bong][woliu] vortex maintain qi_release_to_zone error; routing to overflow"
                        );
                    }
                }
            }
        }
    }
    // Fallback: no zone / ZoneRegistry absent / Err / zone already at cap — route full amount to overflow.
    QiTransfer::new(
        from,
        overflow_account(),
        amount,
        QiTransferReason::ReleaseToZone,
    )
    .ok()
    .into_iter()
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::{App, Events, Update};

    use crate::combat::components::DerivedAttrs;
    use crate::world::zone::ZoneRegistry;

    fn spawn_actor(app: &mut App, realm: Realm, qi_current: f64) -> Entity {
        app.world_mut()
            .spawn((
                Cultivation {
                    realm,
                    qi_current,
                    qi_max: 100.0,
                    ..Default::default()
                },
                MeridianSystem::default(),
                StatusEffects::default(),
                DerivedAttrs::default(),
                Position::new([8.0, 66.0, 8.0]),
                CurrentDimension(DimensionKind::Overworld),
                SkillBarBindings::default(),
                PracticeLog::default(),
            ))
            .id()
    }

    fn app(tick: u64) -> App {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick });
        app.insert_resource(ZoneRegistry::fallback());
        app.add_event::<VortexBackfireEvent>();
        app.add_event::<ProjectileQiDrainedEvent>();
        app.add_event::<QiTransfer>();
        app.add_event::<ApplyStatusEffectIntent>();
        app
    }

    #[test]
    fn register_initializes_transfer_event_for_standalone_woliu_app() {
        let mut app = App::new();

        register(&mut app);

        assert!(app.world().contains_resource::<Events<QiTransfer>>());
    }

    #[test]
    fn realm_tables_match_plan_values() {
        assert_eq!(vortex_delta_for_realm(Realm::Awaken), 0.0);
        assert_eq!(vortex_delta_for_realm(Realm::Induce), 0.10);
        assert_eq!(vortex_delta_for_realm(Realm::Condense), 0.25);
        assert_eq!(vortex_delta_for_realm(Realm::Solidify), 0.45);
        assert_eq!(vortex_delta_for_realm(Realm::Spirit), 0.65);
        assert_eq!(vortex_delta_for_realm(Realm::Void), 0.80);
        assert_eq!(vortex_radius_for_realm(Realm::Induce), 1.0);
        assert_eq!(vortex_radius_for_realm(Realm::Condense), 1.5);
        assert_eq!(vortex_radius_for_realm(Realm::Solidify), 2.0);
        assert_eq!(vortex_radius_for_realm(Realm::Spirit), 3.0);
        assert_eq!(vortex_maintain_max_ticks_for_realm(Realm::Condense), 100);
        assert_eq!(vortex_qi_cost_per_sec(Realm::Spirit), 10.0);
    }

    #[test]
    fn resolve_skill_toggles_vortex_on_and_off() {
        let mut app = app(10);
        let actor = spawn_actor(&mut app, Realm::Condense, 100.0);

        let first = resolve_woliu_vortex_skill(app.world_mut(), actor, 0, None);
        assert!(matches!(first, CastResult::Started { .. }));
        assert!(app.world().get::<VortexField>(actor).is_some());
        assert!(
            app.world()
                .get::<DerivedAttrs>(actor)
                .unwrap()
                .vortex_active
        );
        assert!(has_active_status(
            app.world().get::<StatusEffects>(actor).unwrap(),
            StatusEffectKind::VortexCasting
        ));

        app.world_mut().resource_mut::<CombatClock>().tick = 40;
        let second = resolve_woliu_vortex_skill(app.world_mut(), actor, 0, None);
        assert!(matches!(second, CastResult::Started { .. }));
        assert!(app.world().get::<VortexField>(actor).is_none());
        assert!(
            !app.world()
                .get::<DerivedAttrs>(actor)
                .unwrap()
                .vortex_active
        );
    }

    #[test]
    fn awaken_rejects_vortex_without_mutation() {
        let mut app = app(10);
        let actor = spawn_actor(&mut app, Realm::Awaken, 100.0);

        let result = resolve_woliu_vortex_skill(app.world_mut(), actor, 0, None);

        assert_eq!(result, rejected(CastRejectReason::RealmTooLow));
        assert!(app.world().get::<VortexField>(actor).is_none());
        assert!(
            !app.world()
                .get::<DerivedAttrs>(actor)
                .unwrap()
                .vortex_active
        );
    }

    #[test]
    fn low_environment_qi_backfires_immediately() {
        let mut app = app(10);
        app.world_mut()
            .resource_mut::<ZoneRegistry>()
            .find_zone_mut("spawn")
            .unwrap()
            .spirit_qi = 0.05;
        let actor = spawn_actor(&mut app, Realm::Condense, 100.0);
        app.world_mut()
            .get_mut::<MeridianSystem>(actor)
            .unwrap()
            .get_mut(MeridianId::Lung)
            .opened = true;

        let result = resolve_woliu_vortex_skill(app.world_mut(), actor, 0, None);

        assert!(matches!(
            result,
            CastResult::Started {
                cooldown_ticks: VORTEX_COOLDOWN_TICKS,
                ..
            }
        ));
        let lung = app
            .world()
            .get::<MeridianSystem>(actor)
            .unwrap()
            .get(MeridianId::Lung);
        assert_eq!(lung.flow_capacity, 0.0);
        assert!(!lung.opened);
        let backfires = app.world().resource::<Events<VortexBackfireEvent>>();
        let backfire = backfires.iter_current_update_events().next().unwrap();
        assert_eq!(backfire.cause, BackfireCause::EnvQiTooLow);
    }

    #[test]
    fn maintain_tick_spends_qi_and_closes_when_depleted_without_backfire() {
        let mut app = app(TICKS_PER_SECOND);
        let actor = spawn_actor(&mut app, Realm::Condense, 6.0);
        app.world_mut().entity_mut(actor).insert(VortexField {
            center: DVec3::ZERO,
            radius: 1.5,
            delta: 0.25,
            cast_at_tick: 0,
            maintain_max_ticks: 100,
            caster: actor,
            env_qi_at_cast: 0.9,
            last_maintain_tick: 0,
        });
        upsert_status_effect(
            &mut app.world_mut().get_mut::<StatusEffects>(actor).unwrap(),
            ActiveStatusEffect {
                kind: StatusEffectKind::VortexCasting,
                magnitude: 1.0,
                remaining_ticks: u64::MAX,
                source_pill: None,
            },
        );
        app.add_systems(Update, vortex_maintain_tick);

        app.update();

        assert_eq!(
            app.world().get::<Cultivation>(actor).unwrap().qi_current,
            0.0
        );
        assert!(app.world().get::<VortexField>(actor).is_none());
        assert!(app
            .world()
            .resource::<Events<VortexBackfireEvent>>()
            .is_empty());
    }

    // ── vortex_maintain_tick qi-ledger conservation tests ──────────────────────
    //
    // These tests lock the fix for the qi-evaporation bug (bughunt r3-P3#1):
    // `vortex_maintain_tick` must emit a `QiTransfer { reason: ReleaseToZone }`
    // for every per-second qi deduction so the ledger stays conserved.

    #[test]
    fn maintain_tick_emits_qi_transfer_release_to_zone_on_cost() {
        // Happy path: one full second elapses, cost is deducted, a ReleaseToZone
        // transfer must be emitted covering the exact amount deducted.
        let mut app = app(TICKS_PER_SECOND);
        let actor = spawn_actor(&mut app, Realm::Condense, 100.0);
        app.world_mut().entity_mut(actor).insert(VortexField {
            center: DVec3::ZERO,
            radius: 1.5,
            delta: 0.25,
            cast_at_tick: 0,
            maintain_max_ticks: 500,
            caster: actor,
            env_qi_at_cast: 0.9,
            last_maintain_tick: 0,
        });
        // 清空 spawn zone 余量：fallback 默认 spirit_qi≈0.9，余量仅 ~5 不足以吸收整份 cost=6
        // → 会拆成 zone 部分 + overflow 部分（守恒仍成立但非单笔）。清零后整份归还落入 zone，
        // 锁住「全额回灌」契约。
        app.world_mut()
            .resource_mut::<ZoneRegistry>()
            .find_zone_mut("spawn")
            .unwrap()
            .spirit_qi = 0.0;
        app.add_systems(Update, vortex_maintain_tick);

        app.update();

        let expected_cost = vortex_qi_cost_per_sec(Realm::Condense); // 6.0
        let events = app.world().resource::<Events<QiTransfer>>();
        let releases: Vec<_> = events
            .iter_current_update_events()
            .filter(|t| t.reason == QiTransferReason::ReleaseToZone)
            .collect();
        assert!(
            !releases.is_empty(),
            "vortex_maintain_tick must emit a QiTransfer(ReleaseToZone) for the qi cost; \
             none emitted — ledger leak detected"
        );
        let total_released: f64 = releases.iter().map(|t| t.amount).sum();
        assert!(
            (total_released - expected_cost).abs() < 1e-6,
            "total ReleaseToZone amount must equal the deducted cost ({expected_cost}); \
             got {total_released}"
        );
        assert_eq!(
            app.world().get::<Cultivation>(actor).unwrap().qi_current,
            100.0 - expected_cost,
            "qi_current should be reduced by exactly the cost"
        );
    }

    /// 部分饱和守恒（CodeRabbit #693）：spawn zone 仅剩 room=5（spirit_qi=0.9）但 cost=6 →
    /// accepted=5 入 zone 账户、overflow=1 显式入 overflow 账户，两者之和 == cost，绝不静默丢弃。
    #[test]
    fn maintain_tick_partial_saturation_routes_overflow_not_discard() {
        use crate::qi_physics::ledger::QiAccountKind;
        let mut app = app(TICKS_PER_SECOND);
        let actor = spawn_actor(&mut app, Realm::Condense, 100.0);
        app.world_mut().entity_mut(actor).insert(VortexField {
            center: DVec3::ZERO,
            radius: 1.5,
            delta: 0.25,
            cast_at_tick: 0,
            maintain_max_ticks: 500,
            caster: actor,
            env_qi_at_cast: 0.9,
            last_maintain_tick: 0,
        });
        // 期望值从常量推导（QI_ZONE_UNIT_CAPACITY / vortex cost），容量或成本调整后测试不失真。
        let initial_spirit_qi = 0.9;
        let expected_cost = vortex_qi_cost_per_sec(Realm::Condense);
        // 部分饱和：room = (1.0 - spirit_qi) * CAP；accepted = min(room, cost)，overflow = 剩余。
        let expected_zone = ((1.0 - initial_spirit_qi) * QI_ZONE_UNIT_CAPACITY).min(expected_cost);
        let expected_overflow = expected_cost - expected_zone;
        app.world_mut()
            .resource_mut::<ZoneRegistry>()
            .find_zone_mut("spawn")
            .unwrap()
            .spirit_qi = initial_spirit_qi;
        app.add_systems(Update, vortex_maintain_tick);

        app.update();

        let events = app.world().resource::<Events<QiTransfer>>();
        let releases: Vec<_> = events
            .iter_current_update_events()
            .filter(|t| t.reason == QiTransferReason::ReleaseToZone)
            .collect();
        let zone_sum: f64 = releases
            .iter()
            .filter(|t| t.to.kind == QiAccountKind::Zone)
            .map(|t| t.amount)
            .sum();
        let overflow_sum: f64 = releases
            .iter()
            .filter(|t| t.to.kind == QiAccountKind::Overflow)
            .map(|t| t.amount)
            .sum();
        assert!(
            (zone_sum - expected_zone).abs() < 1e-6,
            "zone 仅接受 room={expected_zone}（spirit_qi {initial_spirit_qi}→1.0），实际入 zone 账户 {zone_sum}"
        );
        assert!(
            (overflow_sum - expected_overflow).abs() < 1e-6,
            "饱和溢出 {expected_overflow} 必须显式入 overflow 账户而非丢弃，实际 {overflow_sum}（#693）"
        );
        let total: f64 = releases.iter().map(|t| t.amount).sum();
        assert!(
            (total - expected_cost).abs() < 1e-6,
            "守恒：ReleaseToZone 总量应 == cost {expected_cost}（zone {expected_zone} + overflow {expected_overflow}），实际 {total}"
        );
    }

    /// 负灵域守恒（#681 同类，CodeRabbit 此前判 Critical）：spirit_qi=-0.5 时不得 .max(0.0) 当 0——
    /// 否则抹掉 -25 负缺口、凭空多 25 qi。修复后 zone_current=-25，cost=6 全额吸收，
    /// zone_after=(-25+6)/50=-0.38（而非 bug 的 (0+6)/50=0.12）。
    #[test]
    fn maintain_tick_negative_zone_no_phantom_credit() {
        let mut app = app(TICKS_PER_SECOND);
        let actor = spawn_actor(&mut app, Realm::Condense, 100.0);
        app.world_mut().entity_mut(actor).insert(VortexField {
            center: DVec3::ZERO,
            radius: 1.5,
            delta: 0.25,
            cast_at_tick: 0,
            maintain_max_ticks: 500,
            caster: actor,
            env_qi_at_cast: 0.9,
            last_maintain_tick: 0,
        });
        let initial_spirit_qi = -0.5;
        app.world_mut()
            .resource_mut::<ZoneRegistry>()
            .find_zone_mut("spawn")
            .unwrap()
            .spirit_qi = initial_spirit_qi;
        app.add_systems(Update, vortex_maintain_tick);

        app.update();

        let expected_cost = vortex_qi_cost_per_sec(Realm::Condense);
        let zone_after = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name("spawn")
            .unwrap()
            .spirit_qi;
        // 裸 spirit_qi*CAP：room=(1-spirit_qi)*CAP 充裕，cost 全额吸收 → zone_after=spirit_qi+cost/CAP。
        let expected = initial_spirit_qi + expected_cost / QI_ZONE_UNIT_CAPACITY;
        // bug 值（.max(0.0) 把负 zone 当 0）：zone_after 会变 cost/CAP，抹掉负缺口。
        let buggy_clamped = expected_cost / QI_ZONE_UNIT_CAPACITY;
        assert!(
            (zone_after - expected).abs() < 1e-6,
            "负灵域应按裸 spirit_qi*CAP 计：zone_after={zone_after:.4} 应={expected:.4}；\
             若 ≈{buggy_clamped:.4} 说明 .max(0.0) 抹掉了负缺口、凭空多出 qi（#681 同类）"
        );
        // room=75 > cost，全额入 zone，无 overflow。
        let total: f64 = app
            .world()
            .resource::<Events<QiTransfer>>()
            .iter_current_update_events()
            .filter(|t| t.reason == QiTransferReason::ReleaseToZone)
            .map(|t| t.amount)
            .sum();
        assert!(
            (total - expected_cost).abs() < 1e-6,
            "守恒：负灵域归还总量应 == cost {expected_cost}（全额入 zone），实际 {total}"
        );
    }

    /// 最大边界（zone 满，room=0）：spirit_qi=1.0 时 helper 承诺「单条全额 overflow transfer」。
    /// 断言 Zone 账户得 0、Overflow 账户得 expected_cost、总量 == 扣费，防 capped fallback 回归成
    /// 空转账或错误入 zone。
    #[test]
    fn maintain_tick_zone_full_routes_all_to_overflow() {
        use crate::qi_physics::ledger::QiAccountKind;
        let mut app = app(TICKS_PER_SECOND);
        let actor = spawn_actor(&mut app, Realm::Condense, 100.0);
        app.world_mut().entity_mut(actor).insert(VortexField {
            center: DVec3::ZERO,
            radius: 1.5,
            delta: 0.25,
            cast_at_tick: 0,
            maintain_max_ticks: 500,
            caster: actor,
            env_qi_at_cast: 0.9,
            last_maintain_tick: 0,
        });
        // spirit_qi=1.0 → zone_current=CAP, room=0：全额溢出（accepted=0 → 走 overflow fallback）。
        app.world_mut()
            .resource_mut::<ZoneRegistry>()
            .find_zone_mut("spawn")
            .unwrap()
            .spirit_qi = 1.0;
        app.add_systems(Update, vortex_maintain_tick);

        app.update();

        let expected_cost = vortex_qi_cost_per_sec(Realm::Condense);
        let events = app.world().resource::<Events<QiTransfer>>();
        let releases: Vec<_> = events
            .iter_current_update_events()
            .filter(|t| t.reason == QiTransferReason::ReleaseToZone)
            .collect();
        let zone_sum: f64 = releases
            .iter()
            .filter(|t| t.to.kind == QiAccountKind::Zone)
            .map(|t| t.amount)
            .sum();
        let overflow_sum: f64 = releases
            .iter()
            .filter(|t| t.to.kind == QiAccountKind::Overflow)
            .map(|t| t.amount)
            .sum();
        assert!(
            zone_sum.abs() < 1e-6,
            "zone 满（room=0）应 0 入 zone 账户，实际 {zone_sum}"
        );
        assert!(
            (overflow_sum - expected_cost).abs() < 1e-6,
            "zone 满时全额 {expected_cost} 必须入 overflow 账户（非空转账），实际 {overflow_sum}"
        );
        let total: f64 = releases.iter().map(|t| t.amount).sum();
        assert!(
            (total - expected_cost).abs() < 1e-6,
            "守恒：zone 满时 ReleaseToZone 总量应 == cost {expected_cost}（全 overflow），实际 {total}"
        );
    }

    #[test]
    fn maintain_tick_no_qi_transfer_when_interval_not_reached() {
        // Boundary: if fewer than TICKS_PER_SECOND ticks have elapsed since the
        // last maintain tick, no cost is charged and no QiTransfer emitted.
        let partial_tick = TICKS_PER_SECOND - 1;
        let mut app = app(partial_tick);
        let actor = spawn_actor(&mut app, Realm::Condense, 100.0);
        app.world_mut().entity_mut(actor).insert(VortexField {
            center: DVec3::ZERO,
            radius: 1.5,
            delta: 0.25,
            cast_at_tick: 0,
            maintain_max_ticks: 500,
            caster: actor,
            env_qi_at_cast: 0.9,
            last_maintain_tick: 0,
        });
        app.add_systems(Update, vortex_maintain_tick);

        app.update();

        let events = app.world().resource::<Events<QiTransfer>>();
        let release_count = events
            .iter_current_update_events()
            .filter(|t| t.reason == QiTransferReason::ReleaseToZone)
            .count();
        assert_eq!(
            release_count, 0,
            "no QiTransfer should be emitted when the per-second interval has not elapsed; \
             got {release_count} events"
        );
        assert_eq!(
            app.world().get::<Cultivation>(actor).unwrap().qi_current,
            100.0,
            "qi_current must not change before the interval elapses"
        );
    }

    #[test]
    fn maintain_tick_emits_overflow_transfer_when_no_zone_registry() {
        // Error branch: without a ZoneRegistry resource, the release must still
        // emit a QiTransfer (to the overflow account) so nothing disappears silently.
        let mut app = App::new();
        app.insert_resource(CombatClock {
            tick: TICKS_PER_SECOND,
        });
        // Deliberately do NOT insert ZoneRegistry.
        app.add_event::<VortexBackfireEvent>();
        app.add_event::<ProjectileQiDrainedEvent>();
        app.add_event::<QiTransfer>();
        app.add_event::<ApplyStatusEffectIntent>();
        let actor = app
            .world_mut()
            .spawn((
                Cultivation {
                    realm: Realm::Condense,
                    qi_current: 50.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
                MeridianSystem::default(),
                StatusEffects::default(),
                DerivedAttrs::default(),
                Position::new([8.0, 66.0, 8.0]),
                CurrentDimension(DimensionKind::Overworld),
                SkillBarBindings::default(),
                PracticeLog::default(),
                VortexField {
                    center: DVec3::ZERO,
                    radius: 1.5,
                    delta: 0.25,
                    cast_at_tick: 0,
                    maintain_max_ticks: 500,
                    caster: Entity::PLACEHOLDER,
                    env_qi_at_cast: 0.9,
                    last_maintain_tick: 0,
                },
            ))
            .id();
        app.add_systems(Update, vortex_maintain_tick);

        app.update();

        let events = app.world().resource::<Events<QiTransfer>>();
        let releases: Vec<_> = events
            .iter_current_update_events()
            .filter(|t| t.reason == QiTransferReason::ReleaseToZone)
            .collect();
        assert!(
            !releases.is_empty(),
            "vortex_maintain_tick must still emit a QiTransfer when ZoneRegistry is absent \
             (overflow fallback); none found — qi evaporated silently"
        );
        let total: f64 = releases.iter().map(|t| t.amount).sum();
        let expected = vortex_qi_cost_per_sec(Realm::Condense);
        assert!(
            (total - expected).abs() < 1e-6,
            "overflow transfer amount must equal cost ({expected}); got {total}"
        );
    }

    #[test]
    fn maintain_tick_depletion_emits_transfer_before_closing() {
        // Edge case: even when qi is fully depleted and vortex closes, the
        // partial cost actually drained must still be released to the zone.
        // Realm::Condense costs 6.0/s; start with 3.0 so only 3.0 is drained.
        let mut app = app(TICKS_PER_SECOND);
        let actor = spawn_actor(&mut app, Realm::Condense, 3.0);
        app.world_mut().entity_mut(actor).insert(VortexField {
            center: DVec3::ZERO,
            radius: 1.5,
            delta: 0.25,
            cast_at_tick: 0,
            maintain_max_ticks: 500,
            caster: actor,
            env_qi_at_cast: 0.9,
            last_maintain_tick: 0,
        });
        upsert_status_effect(
            &mut app.world_mut().get_mut::<StatusEffects>(actor).unwrap(),
            ActiveStatusEffect {
                kind: StatusEffectKind::VortexCasting,
                magnitude: 1.0,
                remaining_ticks: u64::MAX,
                source_pill: None,
            },
        );
        app.add_systems(Update, vortex_maintain_tick);

        app.update();

        // VortexField should be removed when qi hits 0.
        assert!(app.world().get::<VortexField>(actor).is_none());
        // But the 3.0 actually drained must still appear in the ledger.
        let events = app.world().resource::<Events<QiTransfer>>();
        let releases: Vec<_> = events
            .iter_current_update_events()
            .filter(|t| t.reason == QiTransferReason::ReleaseToZone)
            .collect();
        assert!(
            !releases.is_empty(),
            "QiTransfer(ReleaseToZone) must be emitted even when vortex closes on depletion; \
             none emitted — partial cost evaporated"
        );
        let total: f64 = releases.iter().map(|t| t.amount).sum();
        assert!(
            (total - 3.0).abs() < 1e-6,
            "released amount must equal actual drained (3.0, not full cost 6.0); got {total}"
        );
    }

    #[test]
    fn maintain_tick_timeout_does_not_emit_release_transfer() {
        // The timeout path (ExceedMaintainMax) does not deduct qi_current, so
        // no QiTransfer(ReleaseToZone) should be emitted for that branch.
        let mut app = app(101);
        let actor = spawn_actor(&mut app, Realm::Condense, 100.0);
        // last_maintain_tick == clock.tick so the cost branch does NOT fire.
        app.world_mut().entity_mut(actor).insert(VortexField {
            center: DVec3::ZERO,
            radius: 1.5,
            delta: 0.25,
            cast_at_tick: 0,
            maintain_max_ticks: 100,
            caster: actor,
            env_qi_at_cast: 0.9,
            last_maintain_tick: 101,
        });
        app.world_mut()
            .get_mut::<MeridianSystem>(actor)
            .unwrap()
            .get_mut(MeridianId::Lung)
            .opened = true;
        app.add_systems(Update, vortex_maintain_tick);

        app.update();

        let events = app.world().resource::<Events<QiTransfer>>();
        let release_count = events
            .iter_current_update_events()
            .filter(|t| t.reason == QiTransferReason::ReleaseToZone)
            .count();
        assert_eq!(
            release_count, 0,
            "timeout path must not emit ReleaseToZone because no qi was deducted; \
             got {release_count} events"
        );
    }

    #[test]
    fn maintain_tick_timeout_severs_lung_and_removes_vortex_state() {
        let mut app = app(101);
        let actor = spawn_actor(&mut app, Realm::Condense, 100.0);
        app.world_mut().entity_mut(actor).insert(VortexField {
            center: DVec3::ZERO,
            radius: 1.5,
            delta: 0.25,
            cast_at_tick: 0,
            maintain_max_ticks: 100,
            caster: actor,
            env_qi_at_cast: 0.9,
            last_maintain_tick: 100,
        });
        app.world_mut()
            .get_mut::<MeridianSystem>(actor)
            .unwrap()
            .get_mut(MeridianId::Lung)
            .opened = true;
        app.add_systems(Update, vortex_maintain_tick);

        app.update();

        let lung = app
            .world()
            .get::<MeridianSystem>(actor)
            .unwrap()
            .get(MeridianId::Lung);
        assert_eq!(lung.integrity, 0.0);
        assert!(app.world().get::<VortexField>(actor).is_none());
        assert!(
            !app.world()
                .get::<DerivedAttrs>(actor)
                .unwrap()
                .vortex_active
        );
        let backfire = app
            .world()
            .resource::<Events<VortexBackfireEvent>>()
            .iter_current_update_events()
            .next()
            .unwrap();
        assert_eq!(backfire.cause, BackfireCause::ExceedMaintainMax);
    }

    #[test]
    fn anti_spirit_pressure_pill_resists_timeout_severing() {
        let mut app = app(101);
        let actor = spawn_actor(&mut app, Realm::Condense, 100.0);
        app.world_mut().entity_mut(actor).insert(VortexField {
            center: DVec3::ZERO,
            radius: 1.5,
            delta: 0.25,
            cast_at_tick: 0,
            maintain_max_ticks: 100,
            caster: actor,
            env_qi_at_cast: 0.9,
            last_maintain_tick: 100,
        });
        {
            let mut statuses = app.world_mut().get_mut::<StatusEffects>(actor).unwrap();
            upsert_status_effect(
                &mut statuses,
                ActiveStatusEffect {
                    kind: StatusEffectKind::AntiSpiritPressurePill,
                    magnitude: 1.0,
                    remaining_ticks: 20,
                    source_pill: None,
                },
            );
        }
        app.add_systems(Update, vortex_maintain_tick);

        app.update();

        assert_eq!(
            app.world()
                .get::<MeridianSystem>(actor)
                .unwrap()
                .get(MeridianId::Lung)
                .integrity,
            1.0
        );
        let backfire = app
            .world()
            .resource::<Events<VortexBackfireEvent>>()
            .iter_current_update_events()
            .next()
            .unwrap();
        assert!(backfire.resisted);
    }

    #[test]
    fn projectile_drain_uses_strongest_inverse_square_field() {
        let low = VortexField {
            center: DVec3::ZERO,
            radius: 3.0,
            delta: 0.25,
            cast_at_tick: 0,
            maintain_max_ticks: 100,
            caster: Entity::from_raw(10),
            env_qi_at_cast: 0.9,
            last_maintain_tick: 0,
        };
        let high = VortexField {
            center: DVec3::new(3.0, 0.0, 0.0),
            delta: 0.80,
            caster: Entity::from_raw(11),
            ..low
        };
        let (caster, delta, drain_ratio) =
            vortex_aggregate_at(DVec3::new(0.5, 0.0, 0.0), [&low, &high].into_iter()).unwrap();
        assert_eq!(caster, Entity::from_raw(10));
        assert_eq!(delta, 0.25);
        assert!((drain_ratio - 0.25).abs() < 1e-6);

        let mut payload = 1.0;
        let drained = drain_qi_payload(&mut payload, drain_ratio);
        assert!((drained - 0.25).abs() < 1e-6);
        let drained_again = drain_qi_payload(&mut payload, 0.20);
        assert!((drained_again - 0.15).abs() < 1e-6);
        assert!((payload - 0.60).abs() < 1e-6);
    }

    #[test]
    fn projected_qi_drain_matches_mutating_drain_without_touching_payload() {
        let payload = 1.0;

        let projected = projected_qi_drain(payload, 0.25);
        let mut actual_payload = payload;
        let drained = drain_qi_payload(&mut actual_payload, 0.25);

        assert_eq!(payload, 1.0);
        assert!((projected - drained).abs() < 1e-6);
        assert!((actual_payload - 0.75).abs() < 1e-6);
        assert_eq!(projected_qi_drain(f32::NAN, 0.25), 0.0);
        assert_eq!(projected_qi_drain(1.0, f32::NAN), 0.0);
    }

    #[test]
    fn vortex_aggregate_tie_break_prefers_nearer_field_independent_of_iteration_order() {
        let near_weaker = VortexField {
            center: DVec3::new(1.0, 0.0, 0.0),
            radius: 3.0,
            delta: 0.25,
            cast_at_tick: 0,
            maintain_max_ticks: 100,
            caster: Entity::from_raw(30),
            env_qi_at_cast: 0.9,
            last_maintain_tick: 0,
        };
        let far_stronger = VortexField {
            center: DVec3::new(2.0, 0.0, 0.0),
            radius: 3.0,
            delta: 1.0,
            caster: Entity::from_raw(20),
            ..near_weaker
        };

        let forward =
            vortex_aggregate_at(DVec3::ZERO, [&near_weaker, &far_stronger].into_iter()).unwrap();
        let reverse =
            vortex_aggregate_at(DVec3::ZERO, [&far_stronger, &near_weaker].into_iter()).unwrap();

        assert_eq!(forward.0, near_weaker.caster);
        assert_eq!(reverse.0, near_weaker.caster);
        assert_eq!(forward.1, 0.25);
        assert!((forward.2 - 0.25).abs() < 1e-6);
    }

    #[test]
    fn vortex_qi_transfer_builder_surfaces_invalid_amounts() {
        let projectile = Entity::from_raw(1);
        let field_caster = Entity::from_raw(2);

        let transfer = build_vortex_qi_transfer(projectile, field_caster, 0.5)
            .unwrap()
            .unwrap();
        assert_eq!(
            transfer.from,
            QiAccountId::container(format!("projectile:{}", projectile.to_bits()))
        );
        assert_eq!(
            transfer.to,
            QiAccountId::zone(format!("woliu_vortex:{}", field_caster.to_bits()))
        );
        assert_eq!(transfer.reason, QiTransferReason::Collision);
        assert_eq!(transfer.amount, 0.5);

        assert!(build_vortex_qi_transfer(projectile, field_caster, 0.0)
            .unwrap()
            .is_none());

        let Err(QiPhysicsError::InvalidAmount { field, value }) =
            build_vortex_qi_transfer(projectile, field_caster, f32::NAN)
        else {
            panic!("expected invalid transfer amount for non-finite woliu drain");
        };
        assert_eq!(field, "transfer.amount");
        assert!(value.is_nan());
    }

    #[test]
    fn intercept_tick_chooses_near_weaker_field_and_records_transfer() {
        let mut app = app(10);
        let near_actor = spawn_actor(&mut app, Realm::Condense, 100.0);
        let far_actor = spawn_actor(&mut app, Realm::Void, 100.0);
        app.world_mut().entity_mut(near_actor).insert(VortexField {
            center: DVec3::ZERO,
            radius: 3.0,
            delta: 0.25,
            cast_at_tick: 0,
            maintain_max_ticks: 100,
            caster: near_actor,
            env_qi_at_cast: 0.9,
            last_maintain_tick: 0,
        });
        app.world_mut().entity_mut(far_actor).insert(VortexField {
            center: DVec3::new(3.0, 0.0, 0.0),
            radius: 3.0,
            delta: 0.80,
            cast_at_tick: 0,
            maintain_max_ticks: 100,
            caster: far_actor,
            env_qi_at_cast: 0.9,
            last_maintain_tick: 0,
        });
        let projectile = app
            .world_mut()
            .spawn((
                Position::new([0.5, 0.0, 0.0]),
                QiProjectile {
                    owner: None,
                    qi_payload: 1.0,
                },
            ))
            .id();
        app.add_systems(Update, vortex_intercept_tick);

        app.update();

        let drain = app
            .world()
            .resource::<Events<ProjectileQiDrainedEvent>>()
            .iter_current_update_events()
            .next()
            .unwrap();
        assert_eq!(drain.field_caster, near_actor);
        assert!((drain.drained_amount - 0.25).abs() < 1e-6);
        assert!((drain.remaining_payload - 0.75).abs() < 1e-6);

        let transfer = app
            .world()
            .resource::<Events<QiTransfer>>()
            .iter_current_update_events()
            .next()
            .unwrap();
        assert_eq!(
            transfer.from,
            QiAccountId::container(format!("projectile:{}", projectile.to_bits()))
        );
        assert_eq!(
            transfer.to,
            QiAccountId::zone(format!("woliu_vortex:{}", near_actor.to_bits()))
        );
        assert_eq!(transfer.reason, QiTransferReason::Collision);
        assert!((transfer.amount - f64::from(drain.drained_amount)).abs() < 1e-9);

        let remaining = app
            .world()
            .get::<QiProjectile>(projectile)
            .unwrap()
            .qi_payload;
        assert!((1.0 - (remaining + transfer.amount as f32)).abs() < 1e-6);
    }

    #[test]
    fn intercept_tick_records_intricate_practice_on_successful_drain() {
        let mut app = app(10);
        let actor = spawn_actor(&mut app, Realm::Condense, 100.0);
        app.world_mut().entity_mut(actor).insert(VortexField {
            center: DVec3::new(8.0, 66.0, 8.0),
            radius: 3.0,
            delta: 0.25,
            cast_at_tick: 0,
            maintain_max_ticks: 100,
            caster: actor,
            env_qi_at_cast: 0.9,
            last_maintain_tick: 0,
        });
        app.world_mut().spawn((
            Position::new([8.5, 66.0, 8.0]),
            QiProjectile {
                owner: None,
                qi_payload: 1.0,
            },
        ));
        app.add_systems(Update, vortex_intercept_tick);

        app.update();

        assert_eq!(
            app.world()
                .get::<PracticeLog>(actor)
                .unwrap()
                .weights
                .get(&ColorKind::Intricate)
                .copied(),
            Some(crate::cultivation::color::STYLE_PRACTICE_AMOUNT)
        );
        assert_eq!(
            app.world()
                .resource::<Events<ProjectileQiDrainedEvent>>()
                .iter_current_update_events()
                .count(),
            1
        );
    }

    #[test]
    fn vortex_rejected_when_lung_permanently_severed() {
        // GAP-1 修復テスト：MeridianSeveredPermanent に Lung が記録されているとき
        // resolve_woliu_vortex_skill が MeridianSevered(Lung) を返すことを確認する。
        use crate::cultivation::meridian::severed::{MeridianSeveredPermanent, SeveredSource};
        let mut app = app(10);
        let actor = spawn_actor(&mut app, Realm::Condense, 100.0);
        // Lung を永久断脈としてマークする。
        let mut severed = MeridianSeveredPermanent::default();
        severed.insert(MeridianId::Lung, SeveredSource::CombatWound, 5);
        app.world_mut().entity_mut(actor).insert(severed);

        let result = resolve_woliu_vortex_skill(app.world_mut(), actor, 0, None);

        assert_eq!(
            result,
            rejected(CastRejectReason::MeridianSevered(Some(MeridianId::Lung))),
            "woliu.vortex は Lung 永久断脈時に MeridianSevered(Lung) で拒絶されなければならない"
        );
        // エンティティが変更されていないこと。
        assert!(app.world().get::<VortexField>(actor).is_none());
    }

    #[test]
    fn vortex_allowed_when_lung_intact_despite_severed_component_present() {
        // Lung が SEVERED に含まれていなければ別経脈が SEVERED でも涡流は cast できる。
        use crate::cultivation::meridian::severed::{MeridianSeveredPermanent, SeveredSource};
        let mut app = app(10);
        let actor = spawn_actor(&mut app, Realm::Condense, 100.0);
        let mut severed = MeridianSeveredPermanent::default();
        severed.insert(MeridianId::Heart, SeveredSource::CombatWound, 5); // Lung 以外
        app.world_mut().entity_mut(actor).insert(severed);

        let result = resolve_woliu_vortex_skill(app.world_mut(), actor, 0, None);

        assert!(
            matches!(result, CastResult::Started { .. }),
            "Lung が intact なら woliu.vortex は cast できる: got {result:?}"
        );
    }

    #[test]
    fn ambient_qi_perception_keeps_non_inspect_relative() {
        assert_eq!(
            ambient_qi_perception(0.4, 0.9, false).as_deref(),
            Some("此地灵气骤然浓郁，呼吸间元气盈满")
        );
        assert_eq!(ambient_qi_perception(0.8, 0.78, false), None);
        assert_eq!(
            ambient_qi_perception(0.8, 0.2, true).as_deref(),
            Some("灵气浓度: 0.20")
        );
    }

    #[test]
    fn vortex_field_exposes_intricate_style_attack() {
        let field = VortexField {
            center: DVec3::ZERO,
            radius: 4.0,
            delta: 0.25,
            cast_at_tick: 0,
            maintain_max_ticks: 100,
            caster: Entity::from_raw(1),
            env_qi_at_cast: 0.9,
            last_maintain_tick: 0,
        };

        assert_eq!(field.style_color(), ColorKind::Intricate);
        assert_eq!(field.injected_qi(), 0.25);
        assert!(field.purity() >= 0.5);
        assert_eq!(field.rejection_rate(), 0.30);
    }
}
