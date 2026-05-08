use std::collections::VecDeque;

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Commands, Entity, Events, Query, Res, ResMut, Resource};

use crate::cultivation::components::Cultivation;
use crate::cultivation::tick::CultivationClock;
use crate::qi_physics::{
    QiAccountId, QiPhysicsError, QiTransfer, QiTransferReason, WorldQiAccount, WorldQiBudget,
};
use crate::world::zone::{Zone, ZoneRegistry};

use super::components::{
    BarrierField, VoidActionKind, BARRIER_QI_COST, EXPLODE_ZONE_DECAY_TICKS, EXPLODE_ZONE_QI_COST,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScheduledQiReturn {
    pub kind: VoidActionKind,
    pub owner_id: String,
    pub zone_id: String,
    pub amount: f64,
    pub due_tick: u64,
}

#[derive(Debug, Default, Clone, Resource)]
pub struct VoidQiReturnSchedule {
    returns: VecDeque<ScheduledQiReturn>,
}

impl VoidQiReturnSchedule {
    pub fn push(&mut self, scheduled: ScheduledQiReturn) {
        self.returns.push_back(scheduled);
    }

    pub fn len(&self) -> usize {
        self.returns.len()
    }

    pub fn is_empty(&self) -> bool {
        self.returns.is_empty()
    }

    pub fn drain_due(&mut self, now_tick: u64) -> Vec<ScheduledQiReturn> {
        let mut due = Vec::new();
        let mut pending = VecDeque::with_capacity(self.returns.len());
        while let Some(entry) = self.returns.pop_front() {
            if entry.due_tick <= now_tick {
                due.push(entry);
            } else {
                pending.push_back(entry);
            }
        }
        self.returns = pending;
        due
    }
}

pub fn debit_caster_qi_to_account(
    caster: Entity,
    actor_id: &str,
    target: QiAccountId,
    cultivation: &mut Cultivation,
    accounts: &mut WorldQiAccount,
    qi_transfer_events: Option<&mut Events<QiTransfer>>,
    amount: f64,
) -> Result<QiTransfer, QiPhysicsError> {
    let from = QiAccountId::player(actor_id);
    let available = accounts.balance(&from).max(cultivation.qi_current.max(0.0));
    accounts.set_balance(from.clone(), available)?;
    let transfer = QiTransfer::new(from.clone(), target, amount, QiTransferReason::VoidAction)?;
    accounts.transfer(transfer.clone())?;
    cultivation.qi_current = accounts.balance(&from).min(cultivation.qi_current).max(0.0);
    if let Some(events) = qi_transfer_events {
        events.send(transfer.clone());
    }
    tracing::debug!(
        "[bong][void-action] debited qi from {:?}/{} amount={amount}",
        caster,
        actor_id
    );
    Ok(transfer)
}

pub fn borrow_explode_zone_qi(
    budget: &mut WorldQiBudget,
    zone: &mut Zone,
    owner_id: &str,
    now_tick: u64,
    schedule: &mut VoidQiReturnSchedule,
) -> ScheduledQiReturn {
    let borrow_amount = EXPLODE_ZONE_QI_COST + zone.spirit_qi.max(0.0);
    budget.current_total = (budget.current_total - borrow_amount).max(0.0);
    zone.spirit_qi = 1.0;
    let scheduled = ScheduledQiReturn {
        kind: VoidActionKind::ExplodeZone,
        owner_id: owner_id.to_string(),
        zone_id: zone.name.clone(),
        amount: borrow_amount,
        due_tick: now_tick.saturating_add(EXPLODE_ZONE_DECAY_TICKS),
    };
    schedule.push(scheduled.clone());
    scheduled
}

pub fn apply_due_qi_returns(
    budget: &mut WorldQiBudget,
    zones: &mut [Zone],
    due: Vec<ScheduledQiReturn>,
) -> usize {
    let mut applied = 0;
    for entry in due {
        budget.current_total += entry.amount;
        if entry.kind == VoidActionKind::ExplodeZone {
            if let Some(zone) = zones.iter_mut().find(|zone| zone.name == entry.zone_id) {
                zone.spirit_qi = 0.0;
            }
        }
        applied += 1;
    }
    applied
}

pub fn schedule_barrier_return(
    field: &BarrierField,
    schedule: &mut VoidQiReturnSchedule,
) -> ScheduledQiReturn {
    let scheduled = ScheduledQiReturn {
        kind: VoidActionKind::Barrier,
        owner_id: field.owner_id.clone(),
        zone_id: field.zone_id.clone(),
        amount: BARRIER_QI_COST,
        due_tick: field.expires_at_tick,
    };
    schedule.push(scheduled.clone());
    scheduled
}

pub fn apply_due_void_qi_returns_system(
    clock: Res<CultivationClock>,
    mut budget: ResMut<WorldQiBudget>,
    mut schedule: ResMut<VoidQiReturnSchedule>,
    mut zones: Option<ResMut<ZoneRegistry>>,
) {
    let due = schedule.drain_due(clock.tick);
    if due.is_empty() {
        return;
    }
    if let Some(zones) = zones.as_deref_mut() {
        apply_due_qi_returns(&mut budget, zones.zones.as_mut_slice(), due);
    } else {
        for entry in due {
            budget.current_total += entry.amount;
        }
    }
}

pub fn despawn_expired_barriers_system(
    mut commands: Commands,
    clock: Res<CultivationClock>,
    barriers: Query<(Entity, &BarrierField)>,
) {
    for (entity, field) in &barriers {
        if field.expired(clock.tick) {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::cultivation::components::Realm;
    use crate::world::dimension::DimensionKind;
    use valence::prelude::DVec3;

    fn zone(name: &str, spirit_qi: f64) -> Zone {
        Zone {
            name: name.to_string(),
            dimension: DimensionKind::Overworld,
            bounds: (DVec3::ZERO, DVec3::new(10.0, 80.0, 10.0)),
            spirit_qi,
            danger_level: 0,
            active_events: Vec::new(),
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
        }
    }

    fn cultivation(qi: f64) -> Cultivation {
        Cultivation {
            realm: Realm::Void,
            qi_current: qi,
            qi_max: 500.0,
            ..Default::default()
        }
    }

    #[test]
    fn debit_caster_qi_uses_world_account_transfer() {
        let mut accounts = WorldQiAccount::default();
        let mut c = cultivation(500.0);
        let transfer = debit_caster_qi_to_account(
            Entity::PLACEHOLDER,
            "offline:Void",
            QiAccountId::zone("spawn"),
            &mut c,
            &mut accounts,
            None,
            200.0,
        )
        .expect("ledger transfer should succeed");
        assert_eq!(transfer.reason, QiTransferReason::VoidAction);
        assert_eq!(c.qi_current, 300.0);
    }

    #[test]
    fn debit_caster_qi_rejects_insufficient_account() {
        let mut accounts = WorldQiAccount::default();
        let mut c = cultivation(100.0);
        let error = debit_caster_qi_to_account(
            Entity::PLACEHOLDER,
            "offline:Void",
            QiAccountId::zone("spawn"),
            &mut c,
            &mut accounts,
            None,
            200.0,
        )
        .expect_err("insufficient qi should reject");
        assert!(matches!(error, QiPhysicsError::InsufficientQi { .. }));
    }

    #[test]
    fn debit_caster_qi_keeps_ledger_total_conserved() {
        let mut accounts = WorldQiAccount::default();
        let mut c = cultivation(500.0);
        debit_caster_qi_to_account(
            Entity::PLACEHOLDER,
            "offline:Void",
            QiAccountId::zone("spawn"),
            &mut c,
            &mut accounts,
            None,
            150.0,
        )
        .expect("ledger transfer should succeed");
        assert_eq!(accounts.total(), 500.0);
    }

    #[test]
    fn debit_caster_qi_records_transfer_in_account_history() {
        let mut accounts = WorldQiAccount::default();
        let mut c = cultivation(500.0);
        debit_caster_qi_to_account(
            Entity::PLACEHOLDER,
            "offline:Void",
            QiAccountId::zone("spawn"),
            &mut c,
            &mut accounts,
            None,
            150.0,
        )
        .expect("ledger transfer should succeed");
        assert_eq!(accounts.transfers().len(), 1);
    }

    #[test]
    fn explode_zone_borrows_from_world_budget() {
        let mut budget = WorldQiBudget::from_total(1_000.0);
        let mut z = zone("spawn", 0.5);
        let mut schedule = VoidQiReturnSchedule::default();
        let entry = borrow_explode_zone_qi(&mut budget, &mut z, "offline:Void", 10, &mut schedule);
        assert_eq!(entry.amount, 300.5);
        assert_eq!(budget.current_total, 699.5);
    }

    #[test]
    fn explode_zone_sets_qi_to_peak() {
        let mut budget = WorldQiBudget::from_total(1_000.0);
        let mut z = zone("spawn", 0.2);
        let mut schedule = VoidQiReturnSchedule::default();
        borrow_explode_zone_qi(&mut budget, &mut z, "offline:Void", 10, &mut schedule);
        assert_eq!(z.spirit_qi, 1.0);
    }

    #[test]
    fn explode_zone_schedules_six_month_return() {
        let mut budget = WorldQiBudget::from_total(1_000.0);
        let mut z = zone("spawn", 0.0);
        let mut schedule = VoidQiReturnSchedule::default();
        let entry = borrow_explode_zone_qi(&mut budget, &mut z, "offline:Void", 10, &mut schedule);
        assert_eq!(entry.due_tick, 10 + EXPLODE_ZONE_DECAY_TICKS);
    }

    #[test]
    fn return_schedule_drains_due_only() {
        let mut schedule = VoidQiReturnSchedule::default();
        schedule.push(ScheduledQiReturn {
            kind: VoidActionKind::ExplodeZone,
            owner_id: "a".to_string(),
            zone_id: "spawn".to_string(),
            amount: 1.0,
            due_tick: 10,
        });
        schedule.push(ScheduledQiReturn {
            kind: VoidActionKind::Barrier,
            owner_id: "a".to_string(),
            zone_id: "spawn".to_string(),
            amount: 1.0,
            due_tick: 20,
        });
        assert_eq!(schedule.drain_due(10).len(), 1);
        assert_eq!(schedule.len(), 1);
    }

    #[test]
    fn due_explode_return_refunds_budget() {
        let mut budget = WorldQiBudget::from_total(1_000.0);
        budget.current_total = 700.0;
        let mut zones = vec![zone("spawn", 1.0)];
        let applied = apply_due_qi_returns(
            &mut budget,
            &mut zones,
            vec![ScheduledQiReturn {
                kind: VoidActionKind::ExplodeZone,
                owner_id: "a".to_string(),
                zone_id: "spawn".to_string(),
                amount: 300.0,
                due_tick: 20,
            }],
        );
        assert_eq!(applied, 1);
        assert_eq!(budget.current_total, 1_000.0);
    }

    #[test]
    fn due_explode_return_sets_zone_to_zero() {
        let mut budget = WorldQiBudget::from_total(1_000.0);
        let mut zones = vec![zone("spawn", 1.0)];
        apply_due_qi_returns(
            &mut budget,
            &mut zones,
            vec![ScheduledQiReturn {
                kind: VoidActionKind::ExplodeZone,
                owner_id: "a".to_string(),
                zone_id: "spawn".to_string(),
                amount: 0.0,
                due_tick: 20,
            }],
        );
        assert_eq!(zones[0].spirit_qi, 0.0);
    }

    #[test]
    fn barrier_return_schedule_uses_expiry_tick() {
        let field = BarrierField::new(
            "offline:Void",
            "spawn",
            super::super::components::BarrierGeometry::circle([0.0, 64.0, 0.0], 5.0),
            11,
        );
        let mut schedule = VoidQiReturnSchedule::default();
        let entry = schedule_barrier_return(&field, &mut schedule);
        assert_eq!(entry.due_tick, field.expires_at_tick);
    }

    #[test]
    fn barrier_return_schedule_locks_barrier_cost() {
        let field = BarrierField::new(
            "offline:Void",
            "spawn",
            super::super::components::BarrierGeometry::circle([0.0, 64.0, 0.0], 5.0),
            11,
        );
        let mut schedule = VoidQiReturnSchedule::default();
        let entry = schedule_barrier_return(&field, &mut schedule);
        assert_eq!(entry.amount, BARRIER_QI_COST);
    }

    #[test]
    fn return_schedule_starts_empty() {
        assert!(VoidQiReturnSchedule::default().is_empty());
    }

    #[test]
    fn return_schedule_counts_inserted_entries() {
        let mut schedule = VoidQiReturnSchedule::default();
        schedule.push(ScheduledQiReturn {
            kind: VoidActionKind::Barrier,
            owner_id: "a".to_string(),
            zone_id: "spawn".to_string(),
            amount: 1.0,
            due_tick: 10,
        });
        assert_eq!(schedule.len(), 1);
    }
}
