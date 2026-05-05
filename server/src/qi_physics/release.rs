use super::ledger::{QiAccountId, QiTransfer, QiTransferReason};
use super::{finite_non_negative, QiPhysicsError};

#[derive(Debug, Clone, PartialEq)]
pub struct ZoneReleaseOutcome {
    pub zone_after: f64,
    pub accepted: f64,
    pub overflow: f64,
    pub transfer: Option<QiTransfer>,
}

pub fn qi_release_to_zone(
    amount: f64,
    from: QiAccountId,
    zone: QiAccountId,
    zone_current: f64,
    zone_cap: f64,
) -> Result<ZoneReleaseOutcome, QiPhysicsError> {
    let amount = finite_non_negative(amount, "release.amount")?;
    if !zone_current.is_finite() {
        return Err(QiPhysicsError::InvalidAmount {
            field: "zone_current",
            value: zone_current,
        });
    }
    let zone_cap = finite_non_negative(zone_cap, "zone_cap")?;
    let room = (zone_cap - zone_current).max(0.0);
    let accepted = amount.min(room);
    let overflow = amount - accepted;
    let transfer = if accepted > 0.0 {
        Some(QiTransfer::new(
            from,
            zone,
            accepted,
            QiTransferReason::ReleaseToZone,
        )?)
    } else {
        None
    };

    Ok(ZoneReleaseOutcome {
        zone_after: zone_current + accepted,
        accepted,
        overflow,
        transfer,
    })
}

pub fn accumulate_zone_release(zone_current: f64, releases: &[f64], zone_cap: f64) -> f64 {
    let mut zone = zone_current.max(0.0);
    let cap = zone_cap.max(0.0);
    for amount in releases.iter().copied() {
        if amount.is_finite() && amount > 0.0 {
            zone = (zone + amount).min(cap);
        }
    }
    zone
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qi_physics::ledger::QiAccountKind;

    #[test]
    fn release_writes_zone_transfer() {
        let outcome = qi_release_to_zone(
            3.0,
            QiAccountId::player("p1"),
            QiAccountId::zone("spawn"),
            4.0,
            10.0,
        )
        .unwrap();
        assert_eq!(outcome.zone_after, 7.0);
        assert_eq!(outcome.accepted, 3.0);
        assert_eq!(
            outcome.transfer.as_ref().unwrap().to.kind,
            QiAccountKind::Zone
        );
    }

    #[test]
    fn release_clamps_to_zone_cap() {
        let outcome = qi_release_to_zone(
            5.0,
            QiAccountId::player("p1"),
            QiAccountId::zone("spawn"),
            8.0,
            10.0,
        )
        .unwrap();
        assert_eq!(outcome.accepted, 2.0);
        assert_eq!(outcome.overflow, 3.0);
        assert_eq!(outcome.zone_after, 10.0);
    }

    #[test]
    fn full_zone_accepts_nothing() {
        let outcome = qi_release_to_zone(
            5.0,
            QiAccountId::player("p1"),
            QiAccountId::zone("spawn"),
            10.0,
            10.0,
        )
        .unwrap();
        assert!(outcome.transfer.is_none());
        assert_eq!(outcome.overflow, 5.0);
    }

    #[test]
    fn simultaneous_deaths_are_order_independent_adds() {
        let a = accumulate_zone_release(1.0, &[2.0, 3.0, 4.0], 20.0);
        let b = accumulate_zone_release(1.0, &[4.0, 2.0, 3.0], 20.0);
        assert_eq!(a, b);
        assert_eq!(a, 10.0);
    }

    #[test]
    fn invalid_release_amount_is_rejected() {
        let err = qi_release_to_zone(
            f64::NAN,
            QiAccountId::player("p1"),
            QiAccountId::zone("spawn"),
            0.0,
            1.0,
        )
        .expect_err("nan should fail");
        assert!(matches!(err, QiPhysicsError::InvalidAmount { .. }));
    }

    #[test]
    fn release_accepts_negative_zone_qi() {
        let outcome = qi_release_to_zone(
            0.4,
            QiAccountId::player("p1"),
            QiAccountId::zone("dead_edge"),
            -0.6,
            1.0,
        )
        .unwrap();
        assert_eq!(outcome.accepted, 0.4);
        assert_eq!(outcome.zone_after, -0.19999999999999996);
        assert_eq!(outcome.transfer.unwrap().amount, 0.4);
    }
}
