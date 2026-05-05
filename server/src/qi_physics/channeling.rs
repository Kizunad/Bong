use super::env::EnvField;
use super::ledger::{QiAccountId, QiTransfer, QiTransferReason};
use super::{finite_non_negative, QiPhysicsError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelDirection {
    Inject,
    Drain,
    Gather,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChannelingOutcome {
    pub requested: f64,
    pub moved: f64,
    pub direction: ChannelDirection,
}

pub fn qi_channeling(
    amount: f64,
    direction: ChannelDirection,
    efficiency: f64,
    env: EnvField,
) -> Result<ChannelingOutcome, QiPhysicsError> {
    let amount = finite_non_negative(amount, "channel.amount")?;
    let efficiency = finite_non_negative(efficiency, "channel.efficiency")?.clamp(0.0, 1.0);
    let pressure_factor = match direction {
        ChannelDirection::Inject => (1.0 - env.local_zone_qi).clamp(0.0, 1.0),
        ChannelDirection::Drain => env.local_zone_qi.clamp(0.0, 1.0),
        ChannelDirection::Gather => (env.local_zone_qi + env.ambient_pressure).clamp(0.0, 1.0),
    };
    let moved = amount * efficiency * env.rhythm_factor() * pressure_factor;
    Ok(ChannelingOutcome {
        requested: amount,
        moved,
        direction,
    })
}

pub fn qi_channeling_transfer(
    from: QiAccountId,
    to: QiAccountId,
    amount: f64,
    direction: ChannelDirection,
    efficiency: f64,
    env: EnvField,
) -> Result<QiTransfer, QiPhysicsError> {
    let outcome = qi_channeling(amount, direction, efficiency, env)?;
    QiTransfer::new(from, to, outcome.moved, QiTransferReason::Channeling)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inject_prefers_low_zone_pressure() {
        let low = qi_channeling(10.0, ChannelDirection::Inject, 1.0, EnvField::new(0.1)).unwrap();
        let high = qi_channeling(10.0, ChannelDirection::Inject, 1.0, EnvField::new(0.9)).unwrap();
        assert!(low.moved > high.moved);
    }

    #[test]
    fn drain_prefers_high_zone_pressure() {
        let low = qi_channeling(10.0, ChannelDirection::Drain, 1.0, EnvField::new(0.1)).unwrap();
        let high = qi_channeling(10.0, ChannelDirection::Drain, 1.0, EnvField::new(0.9)).unwrap();
        assert!(high.moved > low.moved);
    }

    #[test]
    fn gather_uses_ambient_pressure() {
        let mut env = EnvField::new(0.1);
        env.ambient_pressure = 0.4;
        let outcome = qi_channeling(10.0, ChannelDirection::Gather, 1.0, env).unwrap();
        assert_eq!(outcome.moved, 5.0);
    }

    #[test]
    fn channeling_efficiency_is_clamped() {
        let outcome =
            qi_channeling(10.0, ChannelDirection::Drain, 2.0, EnvField::new(1.0)).unwrap();
        assert_eq!(outcome.moved, 10.0);
    }

    #[test]
    fn channeling_transfer_uses_channeling_reason() {
        let transfer = qi_channeling_transfer(
            QiAccountId::zone("spawn"),
            QiAccountId::player("p"),
            10.0,
            ChannelDirection::Drain,
            1.0,
            EnvField::new(1.0),
        )
        .unwrap();
        assert_eq!(transfer.amount, 10.0);
        assert_eq!(transfer.reason, QiTransferReason::Channeling);
    }
}
