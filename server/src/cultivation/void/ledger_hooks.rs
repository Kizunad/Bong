use std::collections::VecDeque;

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Commands, Entity, Events, Query, Res, ResMut, Resource};

use crate::cultivation::components::Cultivation;
use crate::cultivation::tick::CultivationClock;
use crate::qi_physics::constants::QI_EPSILON;
use crate::qi_physics::{
    QiAccountId, QiPhysicsError, QiTransfer, QiTransferReason, WorldQiAccount, WorldQiBudget,
};
use crate::world::zone::{Zone, ZoneRegistry};

use super::components::{
    BarrierField, VoidActionKind, BARRIER_QI_COST, EXPLODE_ZONE_DECAY_TICKS, EXPLODE_ZONE_QI_COST,
    TICKS_PER_DAY,
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
    let cultivation_balance = cultivation.qi_current.max(0.0);
    let account_balance = accounts.balance(&from);
    if !accounts.has_account(&from) || (account_balance - cultivation_balance).abs() > QI_EPSILON {
        accounts.set_balance(from.clone(), cultivation_balance)?;
    }
    let transfer = QiTransfer::new(from.clone(), target, amount, QiTransferReason::VoidAction)?;
    accounts.transfer(transfer.clone())?;
    cultivation.qi_current = accounts.balance(&from).max(0.0);
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
) -> Result<ScheduledQiReturn, QiPhysicsError> {
    let borrow_amount = EXPLODE_ZONE_QI_COST + zone.spirit_qi.max(0.0);
    if budget.current_total + QI_EPSILON < borrow_amount {
        return Err(QiPhysicsError::InsufficientQi {
            account: "WorldQiBudget.current_total".to_string(),
            available: budget.current_total,
            requested: borrow_amount,
        });
    }
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
    Ok(scheduled)
}

pub fn apply_due_qi_returns(
    budget: &mut WorldQiBudget,
    accounts: &mut WorldQiAccount,
    zones: &mut [Zone],
    due: Vec<ScheduledQiReturn>,
) -> usize {
    let (applied, _) = apply_due_qi_returns_collect_failures(budget, accounts, zones, due);
    applied
}

fn apply_due_qi_returns_collect_failures(
    budget: &mut WorldQiBudget,
    accounts: &mut WorldQiAccount,
    zones: &mut [Zone],
    due: Vec<ScheduledQiReturn>,
) -> (usize, Vec<ScheduledQiReturn>) {
    let mut applied = 0;
    let mut failed = Vec::new();
    let mut zones = Some(zones);
    for entry in due {
        match apply_due_qi_return(budget, accounts, zones.as_deref_mut(), entry) {
            Ok(()) => applied += 1,
            Err(entry) => failed.push(entry),
        }
    }
    (applied, failed)
}

fn apply_due_qi_return(
    budget: &mut WorldQiBudget,
    accounts: &mut WorldQiAccount,
    zones: Option<&mut [Zone]>,
    entry: ScheduledQiReturn,
) -> Result<(), ScheduledQiReturn> {
    match entry.kind {
        VoidActionKind::ExplodeZone => {
            budget.current_total += entry.amount;
            if let Some(zones) = zones {
                if let Some(zone) = zones.iter_mut().find(|zone| zone.name == entry.zone_id) {
                    zone.spirit_qi = 0.0;
                }
            }
        }
        VoidActionKind::Barrier => {
            let from = QiAccountId::zone(format!("barrier:{}", entry.zone_id));
            let to = QiAccountId::zone(entry.zone_id.clone());
            let transfer = QiTransfer::new(from, to, entry.amount, QiTransferReason::VoidAction);
            if let Err(error) = transfer.and_then(|transfer| accounts.transfer(transfer)) {
                tracing::warn!(
                    "[bong][void-action] failed to return barrier qi for zone {}: {error}",
                    entry.zone_id
                );
                return Err(entry);
            }
        }
        VoidActionKind::SuppressTsy | VoidActionKind::LegacyAssign => {}
    }
    Ok(())
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
    mut accounts: ResMut<WorldQiAccount>,
    mut schedule: ResMut<VoidQiReturnSchedule>,
    mut zones: Option<ResMut<ZoneRegistry>>,
) {
    let due = schedule.drain_due(clock.tick);
    if due.is_empty() {
        return;
    }
    let mut failed = Vec::new();
    if let Some(zones) = zones.as_deref_mut() {
        failed = apply_due_qi_returns_collect_failures(
            &mut budget,
            &mut accounts,
            zones.zones.as_mut_slice(),
            due,
        )
        .1;
    } else {
        for entry in due {
            if let Err(entry) = apply_due_qi_return(&mut budget, &mut accounts, None, entry) {
                failed.push(entry);
            }
        }
    }
    for mut entry in failed {
        entry.due_tick = clock.tick.saturating_add(TICKS_PER_DAY);
        schedule.push(entry);
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
    fn debit_caster_qi_resyncs_existing_account_from_cultivation_view() {
        let mut accounts = WorldQiAccount::default();
        let mut c = cultivation(500.0);
        accounts
            .set_balance(QiAccountId::player("offline:Void"), 700.0)
            .unwrap();

        debit_caster_qi_to_account(
            Entity::PLACEHOLDER,
            "offline:Void",
            QiAccountId::zone("spawn"),
            &mut c,
            &mut accounts,
            None,
            150.0,
        )
        .expect("void-action bridge should resync the player ledger before transfer");

        assert_eq!(c.qi_current, 350.0);
        assert_eq!(
            accounts.balance(&QiAccountId::player("offline:Void")),
            350.0
        );
        assert_eq!(accounts.balance(&QiAccountId::zone("spawn")), 150.0);
    }

    #[test]
    fn debit_caster_qi_allows_initial_account_seed() {
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
        .expect("missing account may be seeded from cultivation view in v1");

        assert_eq!(c.qi_current, 350.0);
        assert_eq!(
            accounts.balance(&QiAccountId::player("offline:Void")),
            350.0
        );
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
        let entry = borrow_explode_zone_qi(&mut budget, &mut z, "offline:Void", 10, &mut schedule)
            .expect("budget can cover explode borrow");
        assert_eq!(entry.amount, 300.5);
        assert_eq!(budget.current_total, 699.5);
    }

    #[test]
    fn explode_zone_sets_qi_to_peak() {
        let mut budget = WorldQiBudget::from_total(1_000.0);
        let mut z = zone("spawn", 0.2);
        let mut schedule = VoidQiReturnSchedule::default();
        borrow_explode_zone_qi(&mut budget, &mut z, "offline:Void", 10, &mut schedule)
            .expect("budget can cover explode borrow");
        assert_eq!(z.spirit_qi, 1.0);
    }

    #[test]
    fn explode_zone_schedules_six_month_return() {
        let mut budget = WorldQiBudget::from_total(1_000.0);
        let mut z = zone("spawn", 0.0);
        let mut schedule = VoidQiReturnSchedule::default();
        let entry = borrow_explode_zone_qi(&mut budget, &mut z, "offline:Void", 10, &mut schedule)
            .expect("budget can cover explode borrow");
        assert_eq!(entry.due_tick, 10 + EXPLODE_ZONE_DECAY_TICKS);
    }

    #[test]
    fn explode_zone_rejects_budget_underflow_without_mutation() {
        let mut budget = WorldQiBudget::from_total(100.0);
        let mut z = zone("spawn", 0.5);
        let mut schedule = VoidQiReturnSchedule::default();

        let error = borrow_explode_zone_qi(&mut budget, &mut z, "offline:Void", 10, &mut schedule)
            .expect_err("budget underflow must reject");

        assert!(matches!(error, QiPhysicsError::InsufficientQi { .. }));
        assert_eq!(budget.current_total, 100.0);
        assert_eq!(z.spirit_qi, 0.5);
        assert!(schedule.is_empty());
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
        let mut accounts = WorldQiAccount::default();
        budget.current_total = 700.0;
        let mut zones = vec![zone("spawn", 1.0)];
        let applied = apply_due_qi_returns(
            &mut budget,
            &mut accounts,
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
        let mut accounts = WorldQiAccount::default();
        let mut zones = vec![zone("spawn", 1.0)];
        apply_due_qi_returns(
            &mut budget,
            &mut accounts,
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
    fn due_barrier_return_does_not_mint_world_budget() {
        let mut budget = WorldQiBudget::from_total(1_000.0);
        let mut accounts = WorldQiAccount::default();
        accounts
            .set_balance(QiAccountId::zone("barrier:spawn"), BARRIER_QI_COST)
            .unwrap();
        let mut zones = vec![zone("spawn", 0.5)];

        let applied = apply_due_qi_returns(
            &mut budget,
            &mut accounts,
            &mut zones,
            vec![ScheduledQiReturn {
                kind: VoidActionKind::Barrier,
                owner_id: "a".to_string(),
                zone_id: "spawn".to_string(),
                amount: BARRIER_QI_COST,
                due_tick: 20,
            }],
        );

        assert_eq!(applied, 1);
        assert_eq!(budget.current_total, 1_000.0);
        assert_eq!(accounts.balance(&QiAccountId::zone("barrier:spawn")), 0.0);
        assert_eq!(
            accounts.balance(&QiAccountId::zone("spawn")),
            BARRIER_QI_COST
        );
        assert_eq!(zones[0].spirit_qi, 0.5);
    }

    #[test]
    fn due_barrier_return_failure_is_retriable() {
        let mut budget = WorldQiBudget::from_total(1_000.0);
        let mut accounts = WorldQiAccount::default();
        let mut zones = vec![zone("spawn", 0.5)];
        let entry = ScheduledQiReturn {
            kind: VoidActionKind::Barrier,
            owner_id: "a".to_string(),
            zone_id: "spawn".to_string(),
            amount: BARRIER_QI_COST,
            due_tick: 20,
        };

        let (applied, failed) = apply_due_qi_returns_collect_failures(
            &mut budget,
            &mut accounts,
            &mut zones,
            vec![entry.clone()],
        );

        assert_eq!(applied, 0);
        assert_eq!(failed, vec![entry]);
        assert_eq!(accounts.balance(&QiAccountId::zone("spawn")), 0.0);
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
