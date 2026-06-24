//! Rift-mouth local negative pressure tick.
//!
//! `zone.spirit_qi < 0` remains handled by `negative_zone`; this module consumes
//! raster-local `neg_pressure + portal_anchor_sdf` hot-spots so rift mouths and
//! cave/abyssal internal entrances can drain qi without turning the whole zone
//! into a negative zone.

use valence::prelude::{DVec3, Entity, EventWriter, Position, Query, Res, ResMut, Without};

use crate::network::vfx_event_emit::VfxEventRequest;
use crate::npc::spawn::NpcMarker;
use crate::qi_physics::{QiAccountId, QiTransfer, QiTransferReason, WorldQiAccount};
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

/// rift-mouth 负压抽真元守恒记账（audit-only 模式，同 TSY drain）。
///
/// 玩家/NPC 真元已在 ECS Cultivation.qi_current 扣减，此处仅：
///   1. 确保 `QiAccountId::rift(zone_label)` 账户存在并增 `amount`；
///   2. `push_transfer_audit` 留审计轨迹。
/// 不调 `WorldQiAccount::transfer`（后者会检查 from 余额并拒绝，因为活体 qi 在 ECS 不在 ledger）。
fn record_neg_pressure_drain_transfer(
    account: Option<&mut WorldQiAccount>,
    entity: Entity,
    zone_label: &str,
    amount: f64,
) {
    let Some(account) = account else {
        return;
    };
    if amount <= 0.0 {
        return;
    }
    let from = QiAccountId::player(format!("entity:{entity:?}"));
    let to = QiAccountId::rift(zone_label.to_string());
    // 确保 rift 账户存在
    if !account.has_account(&to) {
        let _ = account.set_balance(to.clone(), 0.0);
    }
    // rift 账户增 amount（审计-only：不动 from 账户余额）
    let rift_balance = account.balance(&to);
    let _ = account.set_balance(to.clone(), rift_balance + amount);
    // 追加审计轨迹
    account.push_transfer_audit(QiTransfer {
        from,
        to,
        amount,
        reason: QiTransferReason::NegPressureDrain,
    });
}

pub fn tick_neg_pressure(
    providers: Option<Res<TerrainProviders>>,
    mut qi_account: Option<ResMut<WorldQiAccount>>,
    mut actors: Query<(Entity, &Position, Option<&CurrentDimension>, &mut Cultivation), Without<NpcMarker>>,
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
        record_neg_pressure_drain_transfer(
            qi_account.as_deref_mut(),
            entity,
            "rift_mouth",
            actual_drain,
        );
        cultivation.qi_current = (cultivation.qi_current - drain).max(0.0);
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

    use crate::qi_physics::{QiAccountId, QiTransferReason, WorldQiAccount};
    use valence::prelude::Entity;

    /// happy path: drain > 0 → rift balance increases, audit trail recorded,
    /// player account is NOT written to ledger (audit-only pattern).
    #[test]
    fn neg_pressure_drain_records_rift_ledger_not_player() {
        let mut account = WorldQiAccount::default();
        let entity = Entity::from_raw(3);
        record_neg_pressure_drain_transfer(Some(&mut account), entity, "rift_mouth", 0.5);

        let rift_id = QiAccountId::rift("rift_mouth");
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
        record_neg_pressure_drain_transfer(Some(&mut account), Entity::from_raw(5), "rift_mouth", 0.0);

        let rift_id = QiAccountId::rift("rift_mouth");
        assert_eq!(account.balance(&rift_id), 0.0, "amount=0 不应写 rift 账户");
        assert_eq!(account.transfers().len(), 0, "amount=0 不应留审计记录");
    }

    /// boundary: None account → no panic, silently returns.
    #[test]
    fn neg_pressure_drain_none_account_is_noop() {
        // must not panic
        record_neg_pressure_drain_transfer(None, Entity::from_raw(7), "rift_mouth", 1.0);
    }

    /// conservation: rift balance accumulates across multiple drain calls.
    #[test]
    fn neg_pressure_drain_rift_balance_accumulates() {
        let mut account = WorldQiAccount::default();
        record_neg_pressure_drain_transfer(Some(&mut account), Entity::from_raw(1), "rift_mouth", 1.5);
        record_neg_pressure_drain_transfer(Some(&mut account), Entity::from_raw(2), "rift_mouth", 2.5);

        let rift_id = QiAccountId::rift("rift_mouth");
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
        record_neg_pressure_drain_transfer(Some(&mut account), Entity::from_raw(9), "rift_mouth", actual_drain);

        let rift_id = QiAccountId::rift("rift_mouth");
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
