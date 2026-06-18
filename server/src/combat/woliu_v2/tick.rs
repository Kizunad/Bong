use valence::prelude::{
    Commands, DVec3, Entity, EventWriter, Position, Query, Res, ResMut, With, Without,
};

use crate::combat::components::TICKS_PER_SECOND;
use crate::combat::CombatClock;
use crate::cultivation::components::{Cultivation, MeridianSystem, Realm};
use crate::cultivation::tribulation::{JueBiTriggerEvent, JueBiTriggerSource};
use crate::qi_physics::constants::{QI_EPSILON, QI_ZONE_UNIT_CAPACITY};
use crate::qi_physics::{
    qi_release_to_zone, QiAccountId, QiAccountKind, QiTransfer, QiTransferReason, WorldQiAccount,
};
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::zone::ZoneRegistry;

use super::backfire::apply_backfire_to_hand_meridians;
use super::events::{
    BackfireCauseV2, BackfireLevel, TurbulenceFieldDecayed, VortexBackfireEventV2, WoliuSkillId,
};
use super::physics::turbulence_decay_step;
use super::state::{PassiveVortex, TurbulenceExposure, TurbulenceField, VortexV2State};

const VOID_HEART_TRIBULATION_TICKS: u64 = 30 * TICKS_PER_SECOND;

pub fn turbulence_decay_tick(
    clock: Res<CombatClock>,
    mut commands: Commands,
    mut fields: Query<(valence::prelude::Entity, &mut TurbulenceField)>,
    mut decayed_events: EventWriter<TurbulenceFieldDecayed>,
    mut qi_transfers: EventWriter<QiTransfer>,
    mut zones: Option<ResMut<ZoneRegistry>>,
    mut qi_account: Option<ResMut<WorldQiAccount>>,
) {
    for (entity, mut field) in &mut fields {
        let elapsed_ticks = clock.tick.saturating_sub(field.last_decay_tick);
        if elapsed_ticks == 0 {
            continue;
        }
        field.last_decay_tick = clock.tick;
        let elapsed_seconds = elapsed_ticks as f64 / TICKS_PER_SECOND as f64;
        let remaining_before = f64::from(field.remaining_swirl_qi);
        let (_decayed, remaining) = turbulence_decay_step(
            f64::from(field.remaining_swirl_qi),
            f64::from(field.decay_rate_per_second),
            elapsed_seconds,
        );
        field.remaining_swirl_qi = remaining as f32;
        let decayed =
            (remaining_before - f64::from(field.remaining_swirl_qi)).clamp(0.0, remaining_before);
        if decayed > f64::EPSILON {
            release_decayed_turbulence_qi(
                &field,
                decayed,
                zones.as_deref_mut(),
                qi_account.as_deref_mut(),
                &mut qi_transfers,
            );
            decayed_events.send(TurbulenceFieldDecayed {
                caster: field.caster,
                radius: field.radius,
                decayed_qi: decayed as f32,
                remaining_swirl_qi: field.remaining_swirl_qi,
                tick: clock.tick,
            });
        }
        if field.remaining_swirl_qi <= f32::EPSILON {
            commands.entity(entity).remove::<TurbulenceField>();
        }
    }
}

fn release_decayed_turbulence_qi(
    field: &TurbulenceField,
    decayed: f64,
    zones: Option<&mut ZoneRegistry>,
    qi_account: Option<&mut WorldQiAccount>,
    qi_transfers: &mut EventWriter<QiTransfer>,
) {
    if decayed <= QI_EPSILON {
        return;
    }

    let from = QiAccountId::rift(format!(
        "woliu_turbulence:entity:{}:{}",
        field.caster.to_bits(),
        field.spawned_at_tick
    ));
    let Some(zones) = zones else {
        send_turbulence_overflow(
            qi_transfers,
            qi_account,
            from,
            decayed,
            format!(
                "woliu_turbulence_missing_zone:{}:entity:{}",
                field.source_zone,
                field.caster.to_bits()
            ),
        );
        return;
    };
    let Some(zone) = zones.find_zone_mut(field.source_zone.as_str()) else {
        send_turbulence_overflow(
            qi_transfers,
            qi_account,
            from,
            decayed,
            format!(
                "woliu_turbulence_missing_zone:{}:entity:{}",
                field.source_zone,
                field.caster.to_bits()
            ),
        );
        return;
    };
    if zone.dimension != field.dimension {
        send_turbulence_overflow(
            qi_transfers,
            qi_account,
            from,
            decayed,
            format!(
                "woliu_turbulence_dimension_mismatch:{}:entity:{}",
                field.source_zone,
                field.caster.to_bits()
            ),
        );
        return;
    }

    let to = QiAccountId::zone(zone.name.clone());
    let zone_current = zone.spirit_qi * QI_ZONE_UNIT_CAPACITY;
    match qi_release_to_zone(
        decayed,
        from.clone(),
        to,
        zone_current,
        QI_ZONE_UNIT_CAPACITY,
    ) {
        Ok(outcome) => {
            zone.spirit_qi = (outcome.zone_after / QI_ZONE_UNIT_CAPACITY).clamp(-1.0, 1.0);
            if let Some(transfer) = outcome.transfer {
                qi_transfers.send(transfer);
            }
            if outcome.overflow > QI_EPSILON {
                send_turbulence_overflow(
                    qi_transfers,
                    qi_account,
                    from,
                    outcome.overflow,
                    format!(
                        "woliu_turbulence_overflow:{}:entity:{}",
                        field.source_zone,
                        field.caster.to_bits()
                    ),
                );
            }
        }
        Err(error) => {
            tracing::warn!(
                ?error,
                "[bong][woliu_v2] invalid turbulence qi release; routing to overflow"
            );
            send_turbulence_overflow(
                qi_transfers,
                qi_account,
                from,
                decayed,
                format!("woliu_turbulence_error:entity:{}", field.caster.to_bits()),
            );
        }
    }
}

fn send_turbulence_overflow(
    qi_transfers: &mut EventWriter<QiTransfer>,
    qi_account: Option<&mut WorldQiAccount>,
    from: QiAccountId,
    amount: f64,
    overflow_id: String,
) {
    if amount <= QI_EPSILON {
        return;
    }
    let Ok(transfer) = QiTransfer::new(
        from,
        QiAccountId::overflow(overflow_id),
        amount,
        QiTransferReason::ReleaseToZone,
    ) else {
        return;
    };
    credit_turbulence_overflow(qi_account, &transfer);
    qi_transfers.send(transfer);
}

fn credit_turbulence_overflow(qi_account: Option<&mut WorldQiAccount>, transfer: &QiTransfer) {
    if transfer.to.kind != QiAccountKind::Overflow || transfer.amount <= QI_EPSILON {
        return;
    }
    let Some(accounts) = qi_account else {
        return;
    };
    let next_balance = accounts.balance(&transfer.to) + transfer.amount;
    match accounts.set_balance(transfer.to.clone(), next_balance) {
        Ok(()) => accounts.push_transfer_audit(transfer.clone()),
        Err(error) => tracing::warn!(
            ?error,
            "[bong][woliu_v2] failed to credit turbulence overflow qi account"
        ),
    }
}

type TurbulenceTargetItem<'a> = (
    Entity,
    &'a Position,
    Option<&'a CurrentDimension>,
    Option<&'a TurbulenceExposure>,
);

type TurbulenceFieldItem<'a> = (Entity, &'a TurbulenceField);

pub fn update_turbulence_exposure_tick(
    clock: Res<CombatClock>,
    mut commands: Commands,
    fields: Query<TurbulenceFieldItem<'_>>,
    targets: Query<TurbulenceTargetItem<'_>, With<Cultivation>>,
) {
    for (target, position, target_dim, current_exposure) in &targets {
        let target_dim = dimension_kind(target_dim);
        let mut strongest: Option<(Entity, f32)> = None;
        for (field_entity, field) in &fields {
            if target == field.caster || target == field_entity {
                continue;
            }
            if field.remaining_swirl_qi <= f32::EPSILON {
                continue;
            }
            if field.dimension != target_dim {
                continue;
            }
            if !within_radius(position.get(), field.center, field.radius) {
                continue;
            }
            if strongest
                .map(|(_, intensity)| field.intensity > intensity)
                .unwrap_or(true)
            {
                strongest = Some((field.caster, field.intensity));
            }
        }

        match (strongest, current_exposure) {
            (Some((source, intensity)), _) if intensity > f32::EPSILON => {
                commands.entity(target).insert(TurbulenceExposure::new(
                    source,
                    intensity,
                    clock.tick.saturating_add(1),
                ));
            }
            (None, Some(_)) => {
                commands.entity(target).remove::<TurbulenceExposure>();
            }
            _ => {}
        }
    }
}

pub fn heart_active_backfire_tick(
    clock: Res<CombatClock>,
    mut states: Query<(
        Entity,
        &mut VortexV2State,
        &Cultivation,
        &mut MeridianSystem,
        Option<&CurrentDimension>,
    )>,
    mut events: EventWriter<VortexBackfireEventV2>,
    mut juebi_triggers: EventWriter<JueBiTriggerEvent>,
) {
    for (entity, mut state, cultivation, mut meridians, dimension) in &mut states {
        if state.active_skill_kind != WoliuSkillId::Heart
            || cultivation.realm != Realm::Void
            || state.backfire_level.is_some()
            || dimension_kind(dimension) == DimensionKind::Tsy
        {
            continue;
        }
        if clock.tick.saturating_sub(state.started_at_tick) < VOID_HEART_TRIBULATION_TICKS {
            continue;
        }
        apply_backfire_to_hand_meridians(&mut meridians, BackfireLevel::Severed);
        state.backfire_level = Some(BackfireLevel::Severed);
        events.send(VortexBackfireEventV2 {
            caster: entity,
            skill: WoliuSkillId::Heart,
            level: BackfireLevel::Severed,
            cause: BackfireCauseV2::VoidHeartTribulation,
            overflow_qi: 0.0,
            tick: clock.tick,
        });
        juebi_triggers.send(JueBiTriggerEvent {
            entity,
            source: JueBiTriggerSource::WoliuVortexHeart,
            delay_ticks: VOID_HEART_TRIBULATION_TICKS,
            triggered_at_tick: clock.tick,
            epicenter: None,
        });
    }
}

pub fn vortex_v2_state_lifecycle_tick(
    clock: Res<CombatClock>,
    mut commands: Commands,
    active_states: Query<(Entity, &VortexV2State)>,
    orphan_passives: Query<Entity, (With<PassiveVortex>, Without<VortexV2State>)>,
) {
    for (entity, state) in &active_states {
        let active_expired = clock.tick >= state.active_until_tick;
        let state_expired = clock.tick >= state.active_until_tick.max(state.cooldown_until_tick);
        if !active_expired && !state_expired {
            continue;
        }
        let mut entity_commands = commands.entity(entity);
        if active_expired && state.active_skill_kind == WoliuSkillId::Heart {
            entity_commands.remove::<PassiveVortex>();
        }
        if state_expired {
            entity_commands.remove::<VortexV2State>();
        }
    }
    for entity in &orphan_passives {
        commands.entity(entity).remove::<PassiveVortex>();
    }
}

fn within_radius(target: DVec3, center: DVec3, radius: f32) -> bool {
    let radius = f64::from(radius.max(0.0));
    target.distance_squared(center) <= radius * radius
}

fn dimension_kind(dimension: Option<&CurrentDimension>) -> DimensionKind {
    dimension.map(|dimension| dimension.0).unwrap_or_default()
}
