use serde::{Deserialize, Serialize};
use valence::prelude::{
    bevy_ecs, Commands, Entity, Event, EventReader, EventWriter, Events, Position, Query, Res,
    ResMut, Username, With,
};

use crate::cultivation::components::{Cultivation, Realm};
use crate::cultivation::death_hooks::{CultivationDeathCause, CultivationDeathTrigger};
use crate::cultivation::life_record::{BiographyEntry, LifeRecord};
use crate::cultivation::lifespan::LifespanComponent;
use crate::cultivation::tick::CultivationClock;
use crate::network::{redis_bridge::RedisOutbound, RedisBridgeResource};
use crate::npc::tsy_hostile::{DaoxiangInstinctCooldown, TsyHostileMarker};
use crate::persistence::{persist_void_action_cooldown, PersistenceSettings};
use crate::qi_physics::{
    constants::QI_EPSILON, QiAccountId, QiTransfer, WorldQiAccount, WorldQiBudget,
};
use crate::schema::void_actions::{VoidActionBroadcastV1, VoidActionRequestV1};
use crate::world::tsy_lifecycle::{TsyLifecycle, TsyZoneStateRegistry};
use crate::world::zone::ZoneRegistry;

use super::components::{
    BarrierDispelHistory, BarrierField, VoidActionCooldowns, VoidActionKind, VoidActionLogEntry,
    DAOXIANG_SUPPRESS_EXTENSION_TICKS,
};
use super::ledger_hooks::{
    borrow_explode_zone_qi, debit_caster_qi_to_account, schedule_barrier_return,
    VoidQiReturnSchedule,
};
use super::legacy::{apply_legacy_assignment, persist_legacy_letterbox, LegacyLetterbox};

#[derive(Debug, Clone, Event)]
pub struct VoidActionIntent {
    pub caster: Entity,
    pub request: VoidActionRequestV1,
    pub requested_at_tick: u64,
}

#[derive(Debug, Clone, Event)]
pub struct VoidActionBroadcast {
    pub payload: VoidActionBroadcastV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoidActionError {
    RealmTooLow,
    QiInsufficient,
    LifespanInsufficient,
    OnCooldown { ready_at_tick: u64 },
    ZoneNotFound,
    TargetNotTsy,
    TsyStateRejected,
    InvalidBarrierGeometry,
    LegacyAlreadyAssigned,
    LegacyPersistFailed,
    LedgerRejected,
}

impl VoidActionError {
    pub fn wire_reason(&self) -> &'static str {
        match self {
            Self::RealmTooLow => "realm_too_low",
            Self::QiInsufficient => "qi_insufficient",
            Self::LifespanInsufficient => "lifespan_insufficient",
            Self::OnCooldown { .. } => "on_cooldown",
            Self::ZoneNotFound => "zone_not_found",
            Self::TargetNotTsy => "target_not_tsy",
            Self::TsyStateRejected => "tsy_state_rejected",
            Self::InvalidBarrierGeometry => "invalid_barrier_geometry",
            Self::LegacyAlreadyAssigned => "legacy_already_assigned",
            Self::LegacyPersistFailed => "legacy_persist_failed",
            Self::LedgerRejected => "ledger_rejected",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoidPrecheckInput {
    pub realm: Realm,
    pub qi_current: f64,
    pub lifespan_remaining_years: f64,
    pub ready_at_tick: u64,
    pub now_tick: u64,
}

pub fn precheck_void_action(
    kind: VoidActionKind,
    input: VoidPrecheckInput,
) -> Result<(), VoidActionError> {
    if input.realm != Realm::Void {
        return Err(VoidActionError::RealmTooLow);
    }
    if input.qi_current + QI_EPSILON < kind.qi_cost() {
        return Err(VoidActionError::QiInsufficient);
    }
    if input.lifespan_remaining_years < kind.lifespan_cost_years() as f64 {
        return Err(VoidActionError::LifespanInsufficient);
    }
    if input.now_tick < input.ready_at_tick {
        return Err(VoidActionError::OnCooldown {
            ready_at_tick: input.ready_at_tick,
        });
    }
    Ok(())
}

pub fn deduct_lifespan_for_void_action(
    lifespan: &mut LifespanComponent,
    kind: VoidActionKind,
) -> bool {
    let cost = kind.lifespan_cost_years() as f64;
    if cost <= 0.0 {
        return false;
    }
    lifespan.years_lived = (lifespan.years_lived + cost).min(lifespan.cap_by_realm as f64);
    lifespan.remaining_years() <= f64::EPSILON
}

pub fn suppress_lifecycle(lifecycle: TsyLifecycle) -> Result<TsyLifecycle, VoidActionError> {
    match lifecycle {
        TsyLifecycle::Collapsing => Ok(TsyLifecycle::Declining),
        TsyLifecycle::Active | TsyLifecycle::Declining | TsyLifecycle::New | TsyLifecycle::Dead => {
            Err(VoidActionError::TsyStateRejected)
        }
    }
}

pub fn extend_daoxiang_cooldown(
    cooldown: &mut DaoxiangInstinctCooldown,
    now_tick: u64,
    extension_ticks: u64,
) {
    let target = now_tick
        .saturating_add(extension_ticks)
        .min(u32::MAX as u64) as u32;
    cooldown.ready_at_tick = cooldown.ready_at_tick.max(target);
}

pub fn barrier_dispel_qi(daoxiang_qi: f64) -> f64 {
    (daoxiang_qi * 0.5).max(0.0)
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn resolve_void_action_intents(
    mut commands: Commands,
    settings: Option<Res<PersistenceSettings>>,
    clock: Res<CultivationClock>,
    redis: Option<Res<RedisBridgeResource>>,
    mut intents: EventReader<VoidActionIntent>,
    mut cooldowns: ResMut<VoidActionCooldowns>,
    mut accounts: ResMut<WorldQiAccount>,
    mut budget: ResMut<WorldQiBudget>,
    mut return_schedule: ResMut<VoidQiReturnSchedule>,
    mut zones: Option<ResMut<ZoneRegistry>>,
    mut tsy_states: Option<ResMut<TsyZoneStateRegistry>>,
    mut qi_transfers: Option<ResMut<Events<QiTransfer>>>,
    mut deaths: EventWriter<CultivationDeathTrigger>,
    mut broadcasts: EventWriter<VoidActionBroadcast>,
    mut actors: Query<(
        &mut Cultivation,
        &mut LifespanComponent,
        &mut LifeRecord,
        Option<&Username>,
        Option<&Position>,
    )>,
    mut daoxiang: Query<(&TsyHostileMarker, Option<&mut DaoxiangInstinctCooldown>)>,
) {
    for intent in intents.read() {
        let Ok((mut cultivation, mut lifespan, mut life_record, username, position)) =
            actors.get_mut(intent.caster)
        else {
            continue;
        };
        let kind = intent.request.kind();
        let actor_id = life_record.character_id.clone();
        let actor_name = username
            .map(|username| username.0.to_string())
            .unwrap_or_else(|| actor_id.clone());
        let now_tick = intent.requested_at_tick.max(clock.tick);

        let precheck = precheck_void_action(
            kind,
            VoidPrecheckInput {
                realm: cultivation.realm,
                qi_current: cultivation.qi_current,
                lifespan_remaining_years: lifespan.remaining_years(),
                ready_at_tick: cooldowns.ready_at(&actor_id, kind),
                now_tick,
            },
        );
        if let Err(error) = precheck {
            tracing::warn!(
                "[bong][void-action] rejected {:?} by {}: {}",
                kind,
                actor_id,
                error.wire_reason()
            );
            continue;
        }

        let result = match &intent.request {
            VoidActionRequestV1::SuppressTsy { zone_id } => cast_suppress_tsy(
                intent.caster,
                &actor_id,
                &actor_name,
                &mut cultivation,
                &mut lifespan,
                &mut life_record,
                zone_id,
                now_tick,
                zones.as_deref_mut(),
                tsy_states.as_deref_mut(),
                &mut accounts,
                qi_transfers.as_deref_mut(),
                &mut daoxiang,
            ),
            VoidActionRequestV1::ExplodeZone { zone_id } => cast_explode_zone(
                intent.caster,
                &actor_id,
                &actor_name,
                &mut cultivation,
                &mut lifespan,
                &mut life_record,
                zone_id,
                now_tick,
                zones.as_deref_mut(),
                &mut accounts,
                &mut budget,
                &mut return_schedule,
                qi_transfers.as_deref_mut(),
            ),
            VoidActionRequestV1::Barrier { zone_id, geometry } => cast_barrier(
                intent.caster,
                &actor_id,
                &actor_name,
                &mut cultivation,
                &mut lifespan,
                &mut life_record,
                zone_id,
                *geometry,
                now_tick,
                position,
                &mut commands,
                &mut accounts,
                &mut return_schedule,
                qi_transfers.as_deref_mut(),
            ),
            VoidActionRequestV1::LegacyAssign {
                inheritor_id,
                item_instance_ids,
                message,
            } => cast_legacy_assign(
                &actor_id,
                &actor_name,
                &mut life_record,
                inheritor_id,
                item_instance_ids.clone(),
                message.clone(),
                now_tick,
                settings.as_deref(),
            ),
        };

        match result {
            Ok(outcome) => {
                cooldowns.set_used(&actor_id, kind, now_tick);
                let ready_at_tick = cooldowns.ready_at(&actor_id, kind);
                if ready_at_tick > now_tick {
                    if let Some(settings) = settings.as_deref() {
                        if let Err(error) =
                            persist_void_action_cooldown(settings, &actor_id, kind, ready_at_tick)
                        {
                            tracing::warn!(
                                "[bong][void-action] failed to persist cooldown for {actor_id}/{:?}: {error}",
                                kind
                            );
                        }
                    } else {
                        tracing::warn!(
                            "[bong][void-action] no persistence settings; cooldown for {actor_id}/{:?} remains memory-only",
                            kind
                        );
                    }
                }
                let payload = VoidActionBroadcastV1::new(
                    kind,
                    actor_id.clone(),
                    actor_name,
                    intent.request.target_label(),
                    now_tick,
                    outcome.public_text,
                );
                if let Some(redis) = redis.as_deref() {
                    let _ = redis
                        .tx_outbound
                        .send(RedisOutbound::VoidAction(payload.clone()));
                }
                broadcasts.send(VoidActionBroadcast { payload });
                if outcome.caused_death {
                    deaths.send(CultivationDeathTrigger {
                        entity: intent.caster,
                        cause: CultivationDeathCause::VoidActionBacklash,
                        context: serde_json::json!({
                            "kind": kind.wire_name(),
                            "at_tick": now_tick,
                        }),
                    });
                }
            }
            Err(error) => tracing::warn!(
                "[bong][void-action] failed {:?} by {}: {}",
                kind,
                actor_id,
                error.wire_reason()
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct VoidActionOutcome {
    public_text: String,
    caused_death: bool,
}

#[allow(clippy::too_many_arguments)]
fn cast_suppress_tsy(
    caster: Entity,
    actor_id: &str,
    actor_name: &str,
    cultivation: &mut Cultivation,
    lifespan: &mut LifespanComponent,
    life_record: &mut LifeRecord,
    zone_id: &str,
    now_tick: u64,
    zones: Option<&mut ZoneRegistry>,
    tsy_states: Option<&mut TsyZoneStateRegistry>,
    accounts: &mut WorldQiAccount,
    qi_transfer_events: Option<&mut Events<QiTransfer>>,
    daoxiang: &mut Query<(&TsyHostileMarker, Option<&mut DaoxiangInstinctCooldown>)>,
) -> Result<VoidActionOutcome, VoidActionError> {
    let family_id = {
        let zones = zones.ok_or(VoidActionError::ZoneNotFound)?;
        let zone = zones
            .zones
            .iter()
            .find(|zone| zone.name == zone_id)
            .ok_or(VoidActionError::ZoneNotFound)?;
        zone.tsy_family_id().ok_or(VoidActionError::TargetNotTsy)?
    };
    let states = tsy_states.ok_or(VoidActionError::TargetNotTsy)?;
    let state = states
        .by_family
        .get_mut(&family_id)
        .ok_or(VoidActionError::TargetNotTsy)?;
    let next_lifecycle = suppress_lifecycle(state.lifecycle)?;

    debit_caster_qi_to_account(
        caster,
        actor_id,
        QiAccountId::zone(zone_id),
        cultivation,
        accounts,
        qi_transfer_events,
        VoidActionKind::SuppressTsy.qi_cost(),
    )
    .map_err(|_| VoidActionError::LedgerRejected)?;

    state.lifecycle = next_lifecycle;
    state.collapsing_started_at_tick = None;

    for (marker, cooldown) in daoxiang.iter_mut() {
        if marker.family_id == family_id {
            if let Some(mut cooldown) = cooldown {
                extend_daoxiang_cooldown(
                    &mut cooldown,
                    now_tick,
                    DAOXIANG_SUPPRESS_EXTENSION_TICKS,
                );
            }
        }
    }

    let caused_death = deduct_lifespan_for_void_action(lifespan, VoidActionKind::SuppressTsy);
    let entry = VoidActionLogEntry::accepted(
        VoidActionKind::SuppressTsy,
        zone_id,
        now_tick,
        "collapsing_to_declining",
    );
    life_record.void_actions.push(entry);
    life_record.push(BiographyEntry::VoidAction {
        kind: VoidActionKind::SuppressTsy,
        target: zone_id.to_string(),
        qi_cost: VoidActionKind::SuppressTsy.qi_cost(),
        lifespan_cost_years: VoidActionKind::SuppressTsy.lifespan_cost_years(),
        tick: now_tick,
    });
    Ok(VoidActionOutcome {
        public_text: format!("{actor_name} 镇住 {zone_id}，坍缩渊退回衰竭。"),
        caused_death,
    })
}

#[allow(clippy::too_many_arguments)]
fn cast_explode_zone(
    caster: Entity,
    actor_id: &str,
    actor_name: &str,
    cultivation: &mut Cultivation,
    lifespan: &mut LifespanComponent,
    life_record: &mut LifeRecord,
    zone_id: &str,
    now_tick: u64,
    zones: Option<&mut ZoneRegistry>,
    accounts: &mut WorldQiAccount,
    budget: &mut WorldQiBudget,
    return_schedule: &mut VoidQiReturnSchedule,
    qi_transfer_events: Option<&mut Events<QiTransfer>>,
) -> Result<VoidActionOutcome, VoidActionError> {
    let zones = zones.ok_or(VoidActionError::ZoneNotFound)?;
    let zone = zones
        .zones
        .iter_mut()
        .find(|zone| zone.name == zone_id)
        .ok_or(VoidActionError::ZoneNotFound)?;
    let borrow_amount = super::components::EXPLODE_ZONE_QI_COST + zone.spirit_qi.max(0.0);
    if budget.current_total < borrow_amount {
        return Err(VoidActionError::LedgerRejected);
    }
    debit_caster_qi_to_account(
        caster,
        actor_id,
        QiAccountId::zone(zone_id),
        cultivation,
        accounts,
        qi_transfer_events,
        VoidActionKind::ExplodeZone.qi_cost(),
    )
    .map_err(|_| VoidActionError::LedgerRejected)?;
    borrow_explode_zone_qi(budget, zone, actor_id, now_tick, return_schedule)
        .map_err(|_| VoidActionError::LedgerRejected)?;
    let caused_death = deduct_lifespan_for_void_action(lifespan, VoidActionKind::ExplodeZone);
    life_record.void_actions.push(VoidActionLogEntry::accepted(
        VoidActionKind::ExplodeZone,
        zone_id,
        now_tick,
        "zone_qi_peak_then_decay",
    ));
    life_record.push(BiographyEntry::VoidAction {
        kind: VoidActionKind::ExplodeZone,
        target: zone_id.to_string(),
        qi_cost: VoidActionKind::ExplodeZone.qi_cost(),
        lifespan_cost_years: VoidActionKind::ExplodeZone.lifespan_cost_years(),
        tick: now_tick,
    });
    Ok(VoidActionOutcome {
        public_text: format!("{actor_name} 引爆 {zone_id}，灵机暴涨后六月归零。"),
        caused_death,
    })
}

#[allow(clippy::too_many_arguments)]
fn cast_barrier(
    caster: Entity,
    actor_id: &str,
    actor_name: &str,
    cultivation: &mut Cultivation,
    lifespan: &mut LifespanComponent,
    life_record: &mut LifeRecord,
    zone_id: &str,
    geometry: super::components::BarrierGeometry,
    now_tick: u64,
    position: Option<&Position>,
    commands: &mut Commands,
    accounts: &mut WorldQiAccount,
    return_schedule: &mut VoidQiReturnSchedule,
    qi_transfer_events: Option<&mut Events<QiTransfer>>,
) -> Result<VoidActionOutcome, VoidActionError> {
    if geometry.radius() <= 0.0 {
        return Err(VoidActionError::InvalidBarrierGeometry);
    }
    if let Some(position) = position {
        let pos = position.get();
        if !geometry.contains([pos.x, pos.y, pos.z]) {
            tracing::debug!(
                "[bong][void-action] barrier geometry does not contain caster position; still accepted as remote boundary"
            );
        }
    }
    debit_caster_qi_to_account(
        caster,
        actor_id,
        QiAccountId::zone(format!("barrier:{zone_id}")),
        cultivation,
        accounts,
        qi_transfer_events,
        VoidActionKind::Barrier.qi_cost(),
    )
    .map_err(|_| VoidActionError::LedgerRejected)?;
    let field = BarrierField::new(actor_id, zone_id, geometry, now_tick);
    schedule_barrier_return(&field, return_schedule);
    commands.spawn(field);
    let caused_death = deduct_lifespan_for_void_action(lifespan, VoidActionKind::Barrier);
    life_record.void_actions.push(VoidActionLogEntry::accepted(
        VoidActionKind::Barrier,
        zone_id,
        now_tick,
        "barrier_field_spawned",
    ));
    life_record.push(BiographyEntry::VoidAction {
        kind: VoidActionKind::Barrier,
        target: zone_id.to_string(),
        qi_cost: VoidActionKind::Barrier.qi_cost(),
        lifespan_cost_years: VoidActionKind::Barrier.lifespan_cost_years(),
        tick: now_tick,
    });
    Ok(VoidActionOutcome {
        public_text: format!("{actor_name} 在 {zone_id} 立下化虚障，道伥过线折其半气。"),
        caused_death,
    })
}

#[allow(clippy::too_many_arguments)]
fn cast_legacy_assign(
    actor_id: &str,
    actor_name: &str,
    life_record: &mut LifeRecord,
    inheritor_id: &str,
    item_instance_ids: Vec<u64>,
    message: Option<String>,
    now_tick: u64,
    settings: Option<&PersistenceSettings>,
) -> Result<VoidActionOutcome, VoidActionError> {
    if life_record.legacy_letterbox.is_some() {
        return Err(VoidActionError::LegacyAlreadyAssigned);
    }
    let letterbox =
        LegacyLetterbox::new(actor_id, inheritor_id, item_instance_ids, message, now_tick);
    let settings = settings.ok_or(VoidActionError::LegacyPersistFailed)?;
    if let Err(error) = persist_legacy_letterbox(settings, &letterbox) {
        tracing::warn!(
            "[bong][void-action] failed to persist legacy letterbox for {actor_id}: {error}"
        );
        return Err(VoidActionError::LegacyPersistFailed);
    }
    apply_legacy_assignment(life_record, letterbox);
    life_record.push(BiographyEntry::VoidAction {
        kind: VoidActionKind::LegacyAssign,
        target: inheritor_id.to_string(),
        qi_cost: 0.0,
        lifespan_cost_years: 0,
        tick: now_tick,
    });
    Ok(VoidActionOutcome {
        public_text: format!("{actor_name} 留下临终遗令，道统指向 {inheritor_id}。"),
        caused_death: false,
    })
}

pub fn apply_barrier_dispel_system(
    mut history: ResMut<BarrierDispelHistory>,
    barriers: Query<(Entity, &BarrierField)>,
    mut daoxiang: Query<(Entity, &Position, &mut Cultivation), With<TsyHostileMarker>>,
) {
    let mut active_barriers = std::collections::HashSet::new();
    for (barrier_entity, field) in &barriers {
        active_barriers.insert(barrier_entity);
        for (hostile_entity, position, mut cultivation) in &mut daoxiang {
            let pos = position.get();
            if !field.geometry.contains([pos.x, pos.y, pos.z]) {
                continue;
            }
            if !history.mark_once(barrier_entity, hostile_entity) {
                continue;
            }
            cultivation.qi_current = barrier_dispel_qi(cultivation.qi_current)
                .min(cultivation.qi_max)
                .max(0.0);
            tracing::debug!(
                "[bong][void-action] barrier {} dispelled daoxiang {:?}",
                field.zone_id,
                hostile_entity
            );
        }
    }
    history.retain_active_barriers(&active_barriers);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> VoidPrecheckInput {
        VoidPrecheckInput {
            realm: Realm::Void,
            qi_current: 500.0,
            lifespan_remaining_years: 500.0,
            ready_at_tick: 0,
            now_tick: 10,
        }
    }

    #[test]
    fn precheck_accepts_void_with_costs() {
        assert_eq!(
            precheck_void_action(VoidActionKind::Barrier, input()),
            Ok(())
        );
    }

    #[test]
    fn precheck_rejects_non_void() {
        let mut input = input();
        input.realm = Realm::Spirit;
        assert_eq!(
            precheck_void_action(VoidActionKind::Barrier, input),
            Err(VoidActionError::RealmTooLow)
        );
    }

    #[test]
    fn precheck_rejects_low_qi() {
        let mut input = input();
        input.qi_current = 10.0;
        assert_eq!(
            precheck_void_action(VoidActionKind::Barrier, input),
            Err(VoidActionError::QiInsufficient)
        );
    }

    #[test]
    fn precheck_allows_equal_lifespan_cost() {
        let mut input = input();
        input.lifespan_remaining_years = 30.0;
        assert_eq!(precheck_void_action(VoidActionKind::Barrier, input), Ok(()));
    }

    #[test]
    fn precheck_rejects_lower_lifespan() {
        let mut input = input();
        input.lifespan_remaining_years = 29.9;
        assert_eq!(
            precheck_void_action(VoidActionKind::Barrier, input),
            Err(VoidActionError::LifespanInsufficient)
        );
    }

    #[test]
    fn precheck_rejects_cooldown() {
        let mut input = input();
        input.ready_at_tick = 11;
        assert_eq!(
            precheck_void_action(VoidActionKind::Barrier, input),
            Err(VoidActionError::OnCooldown { ready_at_tick: 11 })
        );
    }

    #[test]
    fn precheck_allows_at_ready_tick() {
        let mut input = input();
        input.ready_at_tick = 10;
        assert_eq!(precheck_void_action(VoidActionKind::Barrier, input), Ok(()));
    }

    #[test]
    fn precheck_legacy_allows_zero_cost() {
        let input = VoidPrecheckInput {
            qi_current: 0.0,
            lifespan_remaining_years: 1.0,
            ..input()
        };
        assert_eq!(
            precheck_void_action(VoidActionKind::LegacyAssign, input),
            Ok(())
        );
    }

    #[test]
    fn legacy_assign_requires_persistence_settings_before_mutating_life_record() {
        let mut life_record = LifeRecord::new("offline:Void");

        let result = cast_legacy_assign(
            "offline:Void",
            "Void",
            &mut life_record,
            "offline:Heir",
            vec![1001],
            Some("留给后来人".to_string()),
            42,
            None,
        );

        assert_eq!(result, Err(VoidActionError::LegacyPersistFailed));
        assert!(life_record.legacy_letterbox.is_none());
        assert!(life_record.biography.is_empty());
    }

    #[test]
    fn deduct_lifespan_adds_years_lived() {
        let mut lifespan = LifespanComponent::new(200);
        deduct_lifespan_for_void_action(&mut lifespan, VoidActionKind::Barrier);
        assert_eq!(lifespan.years_lived, 30.0);
    }

    #[test]
    fn deduct_lifespan_reports_no_death_when_remaining() {
        let mut lifespan = LifespanComponent::new(200);
        assert!(!deduct_lifespan_for_void_action(
            &mut lifespan,
            VoidActionKind::Barrier
        ));
    }

    #[test]
    fn deduct_lifespan_reports_death_at_cap() {
        let mut lifespan = LifespanComponent::new(30);
        assert!(deduct_lifespan_for_void_action(
            &mut lifespan,
            VoidActionKind::Barrier
        ));
    }

    #[test]
    fn suppress_lifecycle_rewinds_collapsing() {
        assert_eq!(
            suppress_lifecycle(TsyLifecycle::Collapsing),
            Ok(TsyLifecycle::Declining)
        );
    }

    #[test]
    fn suppress_lifecycle_rejects_active() {
        assert_eq!(
            suppress_lifecycle(TsyLifecycle::Active),
            Err(VoidActionError::TsyStateRejected)
        );
    }

    #[test]
    fn suppress_lifecycle_rejects_declining() {
        assert_eq!(
            suppress_lifecycle(TsyLifecycle::Declining),
            Err(VoidActionError::TsyStateRejected)
        );
    }

    #[test]
    fn suppress_lifecycle_rejects_dead() {
        assert_eq!(
            suppress_lifecycle(TsyLifecycle::Dead),
            Err(VoidActionError::TsyStateRejected)
        );
    }

    #[test]
    fn daoxiang_cooldown_extends_forward() {
        let mut cooldown = DaoxiangInstinctCooldown { ready_at_tick: 10 };
        extend_daoxiang_cooldown(&mut cooldown, 20, 100);
        assert_eq!(cooldown.ready_at_tick, 120);
    }

    #[test]
    fn daoxiang_cooldown_never_shortens() {
        let mut cooldown = DaoxiangInstinctCooldown { ready_at_tick: 200 };
        extend_daoxiang_cooldown(&mut cooldown, 20, 100);
        assert_eq!(cooldown.ready_at_tick, 200);
    }

    #[test]
    fn daoxiang_cooldown_saturates_to_u32() {
        let mut cooldown = DaoxiangInstinctCooldown { ready_at_tick: 0 };
        extend_daoxiang_cooldown(&mut cooldown, u64::MAX - 1, 100);
        assert_eq!(cooldown.ready_at_tick, u32::MAX);
    }

    #[test]
    fn barrier_dispel_halves_qi() {
        assert_eq!(barrier_dispel_qi(80.0), 40.0);
    }

    #[test]
    fn barrier_dispel_clamps_negative_qi() {
        assert_eq!(barrier_dispel_qi(-10.0), 0.0);
    }

    #[test]
    fn error_reasons_are_snake_case() {
        assert_eq!(
            VoidActionError::LedgerRejected.wire_reason(),
            "ledger_rejected"
        );
    }

    #[test]
    fn barrier_dispel_system_halves_daoxiang_qi_once() {
        use super::super::components::BarrierGeometry;
        use valence::prelude::{App, Update};

        let mut app = App::new();
        app.init_resource::<BarrierDispelHistory>();
        app.add_systems(Update, apply_barrier_dispel_system);
        app.world_mut().spawn(BarrierField::new(
            "offline:Void",
            "spawn",
            BarrierGeometry::circle([0.0, 64.0, 0.0], 8.0),
            10,
        ));
        let hostile = app
            .world_mut()
            .spawn((
                TsyHostileMarker {
                    family_id: "tsy".to_string(),
                },
                Position::new([4.0, 64.0, 0.0]),
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 80.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
            ))
            .id();

        app.update();
        assert_eq!(
            app.world()
                .entity(hostile)
                .get::<Cultivation>()
                .unwrap()
                .qi_current,
            40.0
        );
        app.update();
        assert_eq!(
            app.world()
                .entity(hostile)
                .get::<Cultivation>()
                .unwrap()
                .qi_current,
            40.0
        );
    }
}
