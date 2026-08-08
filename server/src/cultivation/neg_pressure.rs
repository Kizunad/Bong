//! Rift-mouth local negative pressure tick.
//!
//! `zone.spirit_qi < 0` remains handled by `negative_zone`; this module consumes
//! raster-local `neg_pressure + portal_anchor_sdf` hot-spots so rift mouths and
//! cave/abyssal internal entrances can drain qi without turning the whole zone
//! into a negative zone.

use valence::prelude::{DVec3, Entity, EventWriter, Position, Query, Res, ResMut, Without};

use crate::network::vfx_event_emit::VfxEventRequest;
use crate::npc::spawn::NpcMarker;
use crate::qi_physics::{
    rift_drain_account, QiAccountId, QiTransfer, QiTransferReason, WorldQiAccount,
};
use crate::schema::vfx_event::VfxEventPayloadV1;
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::terrain::TerrainProviders;

use super::components::{Cultivation, Realm};

pub const HOTSPOT_RADIUS_BLOCKS: f32 = 30.0;
pub const FULL_PULL_NEG_PRESSURE: f32 = 0.8;
pub const TICKS_PER_SECOND: f64 = 20.0;
pub const FROST_BREATH_EVENT_ID: &str = "bong:frost_breath";

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NegPressureField {
    pub center: [f64; 2],
    pub max_pull: f32,
    pub falloff: f32,
}

impl NegPressureField {
    pub fn strength_at(&self, x: f64, z: f64) -> f32 {
        let dx = x - self.center[0];
        let dz = z - self.center[1];
        let distance = (dx * dx + dz * dz).sqrt() as f32;
        if distance > self.falloff || self.falloff <= 0.0 {
            return 0.0;
        }
        let t = 1.0 - distance / self.falloff;
        (self.max_pull * t).clamp(0.0, self.max_pull)
    }
}

pub fn qi_drain_per_sec(realm: Realm) -> f64 {
    match realm {
        Realm::Awaken => 0.0,
        Realm::Induce => 2.0,
        Realm::Condense => 5.0,
        Realm::Solidify => 10.0,
        Realm::Spirit => 25.0,
        Realm::Void => 60.0,
    }
}

pub fn drain_per_tick(realm: Realm, neg_pressure: f32, portal_anchor_sdf: f32) -> f64 {
    if portal_anchor_sdf > HOTSPOT_RADIUS_BLOCKS || neg_pressure <= 0.0 {
        return 0.0;
    }
    let pressure_scale = (neg_pressure / FULL_PULL_NEG_PRESSURE).clamp(0.0, 1.0) as f64;
    qi_drain_per_sec(realm) * pressure_scale / TICKS_PER_SECOND
}

pub fn frost_breath_payload(origin: DVec3, strength: f32) -> VfxEventPayloadV1 {
    VfxEventPayloadV1::SpawnParticle {
        event_id: FROST_BREATH_EVENT_ID.to_string(),
        origin: [origin.x, origin.y + 1.6, origin.z],
        direction: Some([0.0, 1.0, 0.0]),
        color: Some("#CFEFFF".to_string()),
        strength: Some(strength.clamp(0.15, 1.0)),
        count: Some(4),
        duration_ticks: Some(20),
    }
}

/// Rift-mouth 负压抽真元守恒记账。
///
/// 活体真元由 ECS `Cultivation.qi_current` 持有；本函数把外部物理来源的实际抽取量
/// 原子转入可持久恢复的固定 `rift_drain` 池，并保留 canonical actor 审计来源。
/// 调用者只有在本函数成功后才能提交 ECS debit。
fn record_neg_pressure_drain_transfer(
    account: &mut WorldQiAccount,
    entity: Entity,
    amount: f64,
) -> Result<(), crate::qi_physics::QiPhysicsError> {
    let amount = crate::qi_physics::finite_non_negative(amount, "transfer.amount")?;
    if amount == 0.0 {
        return Ok(());
    }

    let from = QiAccountId::player(format!("entity:{entity:?}"));
    let transfer = QiTransfer::new(
        from,
        rift_drain_account(),
        amount,
        QiTransferReason::NegPressureDrain,
    )?;
    let destination = account.balance(&transfer.to);
    let destination_after = destination + amount;
    if !destination_after.is_finite() || destination_after == destination {
        return Err(crate::qi_physics::QiPhysicsError::InvalidAmount {
            field: "destination_balance",
            value: destination_after,
        });
    }

    account.set_balance(transfer.to.clone(), destination_after)?;
    account.push_transfer_audit(transfer);
    Ok(())
}

pub fn tick_neg_pressure(
    providers: Option<Res<TerrainProviders>>,
    mut qi_account: Option<ResMut<WorldQiAccount>>,
    mut actors: Query<
        (
            Entity,
            &Position,
            Option<&CurrentDimension>,
            &mut Cultivation,
        ),
        Without<NpcMarker>,
    >,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    let Some(providers) = providers else {
        return;
    };

    for (entity, pos, current_dimension, mut cultivation) in actors.iter_mut() {
        let dimension = current_dimension
            .map(|current| current.0)
            .unwrap_or(DimensionKind::Overworld);
        let Some(provider) = providers.for_dimension(dimension) else {
            continue;
        };

        let sample = provider.sample(pos.0.x.floor() as i32, pos.0.z.floor() as i32);
        let drain = drain_per_tick(
            cultivation.realm,
            sample.neg_pressure,
            sample.portal_anchor_sdf,
        );
        if drain <= 0.0 {
            continue;
        }

        let before_qi = cultivation.qi_current.max(0.0);
        let actual_drain = drain.min(before_qi);
        let Some(account) = qi_account.as_deref_mut() else {
            tracing::warn!(
                "[bong][neg_pressure] WorldQiAccount missing for actor {:?}; keep qi unchanged",
                entity,
            );
            continue;
        };
        if let Err(error) = record_neg_pressure_drain_transfer(account, entity, actual_drain) {
            tracing::warn!(
                "[bong][neg_pressure] rift ledger failed actor={:?} amount={} error={error}; keep qi unchanged",
                entity,
                actual_drain,
            );
            continue;
        }
        cultivation.qi_current = before_qi - actual_drain;
        let strength = (sample.neg_pressure / FULL_PULL_NEG_PRESSURE).clamp(0.15, 1.0);
        vfx_events.send(VfxEventRequest::new(
            pos.0,
            frost_breath_payload(pos.0, strength),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn realm_drain_table_matches_rift_mouth_plan() {
        assert_eq!(qi_drain_per_sec(Realm::Awaken), 0.0);
        assert_eq!(qi_drain_per_sec(Realm::Induce), 2.0);
        assert_eq!(qi_drain_per_sec(Realm::Condense), 5.0);
        assert_eq!(qi_drain_per_sec(Realm::Solidify), 10.0);
        assert_eq!(qi_drain_per_sec(Realm::Spirit), 25.0);
        assert_eq!(qi_drain_per_sec(Realm::Void), 60.0);
    }

    #[test]
    fn drain_per_tick_uses_full_pull_at_point_eight_pressure() {
        let drain = drain_per_tick(Realm::Solidify, 0.8, 0.0);
        assert!((drain - 0.5).abs() < 1e-9);
    }

    #[test]
    fn drain_per_tick_gates_outside_portal_anchor_radius() {
        assert_eq!(drain_per_tick(Realm::Void, 0.8, 30.1), 0.0);
        assert_eq!(drain_per_tick(Realm::Void, 0.0, 0.0), 0.0);
    }

    #[test]
    fn neg_pressure_field_falls_off_from_center() {
        let field = NegPressureField {
            center: [0.0, 0.0],
            max_pull: 0.8,
            falloff: 30.0,
        };
        assert_eq!(field.strength_at(0.0, 0.0), 0.8);
        assert_eq!(field.strength_at(30.1, 0.0), 0.0);
        assert!(field.strength_at(15.0, 0.0) > 0.0);
    }

    #[test]
    fn frost_breath_payload_uses_dedicated_event_id() {
        let payload = frost_breath_payload(DVec3::new(1.0, 64.0, 2.0), 0.8);
        match payload {
            VfxEventPayloadV1::SpawnParticle {
                event_id,
                origin,
                color,
                ..
            } => {
                assert_eq!(event_id, FROST_BREATH_EVENT_ID);
                assert_eq!(origin, [1.0, 65.6, 2.0]);
                assert_eq!(color.as_deref(), Some("#CFEFFF"));
            }
            other => panic!("expected SpawnParticle, got {other:?}"),
        }
    }

    // ── QS-01 ledger accounting tests ───────────────────────────────────────

    use crate::qi_physics::{rift_drain_account, QiAccountId, QiTransferReason, WorldQiAccount};
    use valence::prelude::Entity;

    /// happy path: drain > 0 → rift balance increases, audit trail recorded,
    /// player account is NOT persisted in ledger (external-owner transaction pattern).
    #[test]
    fn neg_pressure_drain_records_rift_ledger_not_player() {
        let mut account = WorldQiAccount::default();
        let entity = Entity::from_raw(3);
        record_neg_pressure_drain_transfer(&mut account, entity, 0.5)
            .expect("valid drain should commit");

        let rift_id = rift_drain_account();
        let player_id = QiAccountId::player(format!("entity:{entity:?}"));

        assert_eq!(
            account.balance(&rift_id),
            0.5,
            "rift 账户应增加 actual_drain（0.5），期望 0.5，实际 {}",
            account.balance(&rift_id)
        );
        assert!(
            !account.has_account(&player_id),
            "玩家账户不应写入 ledger（ECS 是真元的唯一来源）"
        );
        assert_eq!(
            account.total(),
            0.5,
            "ledger total 应等于 rift drain 量（0.5），不应虚增"
        );
        assert_eq!(
            account.transfers().len(),
            1,
            "应有恰好 1 条审计记录，实际 {}",
            account.transfers().len()
        );
        assert_eq!(
            account.transfers()[0].reason,
            QiTransferReason::NegPressureDrain,
            "审计记录 reason 应为 NegPressureDrain"
        );
    }

    /// boundary: amount=0 → no rift account created, no audit trail.
    #[test]
    fn neg_pressure_drain_zero_amount_is_noop() {
        let mut account = WorldQiAccount::default();
        record_neg_pressure_drain_transfer(&mut account, Entity::from_raw(5), 0.0)
            .expect("zero drain should be a valid no-op");

        let rift_id = rift_drain_account();
        assert_eq!(account.balance(&rift_id), 0.0, "amount=0 不应写 rift 账户");
        assert_eq!(account.transfers().len(), 0, "amount=0 不应留审计记录");
    }

    /// error boundary: destination cannot advance beyond f64::MAX, so no balance or audit mutates.
    #[test]
    fn neg_pressure_drain_destination_overflow_is_atomic() {
        let mut account = WorldQiAccount::default();
        account
            .set_balance(rift_drain_account(), f64::MAX)
            .expect("max finite fixture should be valid");
        let before = account.clone();

        let error = record_neg_pressure_drain_transfer(&mut account, Entity::from_raw(7), 1.0)
            .expect_err("destination overflow must reject the drain");

        assert!(matches!(
            error,
            crate::qi_physics::QiPhysicsError::InvalidAmount {
                field: "destination_balance",
                ..
            }
        ));
        assert_eq!(
            account.balance(&rift_drain_account()),
            before.balance(&rift_drain_account())
        );
        assert_eq!(account.transfers(), before.transfers());
    }

    /// conservation: rift balance accumulates across multiple drain calls.
    #[test]
    fn neg_pressure_drain_rift_balance_accumulates() {
        let mut account = WorldQiAccount::default();
        record_neg_pressure_drain_transfer(&mut account, Entity::from_raw(1), 1.5)
            .expect("first drain should commit");
        record_neg_pressure_drain_transfer(&mut account, Entity::from_raw(2), 2.5)
            .expect("second drain should commit");

        let rift_id = rift_drain_account();
        assert_eq!(
            account.balance(&rift_id),
            4.0,
            "rift 余额应累积两次 drain（1.5+2.5=4.0），实际 {}",
            account.balance(&rift_id)
        );
        assert_eq!(
            account.transfers().len(),
            2,
            "应有 2 条审计记录，实际 {}",
            account.transfers().len()
        );
    }

    /// overdraft guard: actual_drain = drain.min(before_qi) so rift never exceeds
    /// what the player actually lost.  Validate the helper correctly records only
    /// actual_drain, not an unclamped larger value.
    #[test]
    fn neg_pressure_drain_records_actual_drain_not_unclamped() {
        // Simulate: player has 0.1 qi, drain formula produces 0.5/tick.
        // actual_drain = min(0.5, 0.1) = 0.1.
        let actual_drain = 0.5_f64.min(0.1_f64);
        let mut account = WorldQiAccount::default();
        record_neg_pressure_drain_transfer(&mut account, Entity::from_raw(9), actual_drain)
            .expect("clamped drain should commit");

        let rift_id = rift_drain_account();
        assert!(
            account.balance(&rift_id) <= 0.1 + 1e-12,
            "rift balance ({}) must not exceed before_qi (0.1) — \
             ledger must record actual_drain (clamped), not unclamped drain",
            account.balance(&rift_id)
        );
    }

    /// pin: NegPressureDrain reason is distinct from RiftCollapse (different semantic).
    #[test]
    fn neg_pressure_drain_reason_distinct_from_rift_collapse() {
        assert_ne!(
            QiTransferReason::NegPressureDrain,
            QiTransferReason::RiftCollapse,
            "NegPressureDrain and RiftCollapse must be distinct variants for audit trail clarity"
        );
    }
}
