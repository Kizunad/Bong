use super::constants::{QI_ACOUSTIC_THRESHOLD, QI_DRAIN_CLAMP, QI_EXCRETION_BASE};
use super::distance::qi_distance_atten;
use super::env::EnvField;
use super::ledger::{QiAccountId, QiTransfer, QiTransferReason};
use super::traits::{StyleAttack, StyleDefense};

#[derive(Debug, Clone, PartialEq)]
pub struct CollisionOutcome {
    pub attenuated_qi: f64,
    pub effective_hit: f64,
    pub attacker_spent: f64,
    pub defender_lost: f64,
    pub defender_absorbed: f64,
    pub transfers: Vec<QiTransfer>,
}

pub fn qi_collision(
    atk: &dyn StyleAttack,
    def: &dyn StyleDefense,
    distance_blocks: f64,
    env: &EnvField,
) -> CollisionOutcome {
    let injected = atk.injected_qi().max(0.0);
    let attenuated = qi_distance_atten(injected, distance_blocks, atk.medium());
    let purity = atk.purity().clamp(0.0, 1.0);
    if purity < QI_ACOUSTIC_THRESHOLD {
        return CollisionOutcome {
            attenuated_qi: attenuated,
            effective_hit: 0.0,
            attacker_spent: injected,
            defender_lost: 0.0,
            defender_absorbed: 0.0,
            transfers: Vec::new(),
        };
    }

    let resistance = def.resistance().clamp(0.0, 1.0);
    let rejection = attenuated * QI_EXCRETION_BASE * (1.0 - purity + resistance * 0.5);
    let effective_hit = (attenuated - rejection).max(0.0) * env.rhythm_factor();
    let defender_lost = effective_hit * (1.0 - resistance);
    let defender_absorbed =
        (defender_lost * def.drain_affinity().clamp(0.0, 1.0)).min(injected * QI_DRAIN_CLAMP);

    let mut transfers = Vec::new();
    if defender_lost > 0.0 {
        if let Ok(transfer) = QiTransfer::new(
            QiAccountId::player("attacker"),
            QiAccountId::player("defender"),
            defender_lost,
            QiTransferReason::Collision,
        ) {
            transfers.push(transfer);
        }
    }
    if defender_absorbed > 0.0 {
        if let Ok(transfer) = QiTransfer::new(
            QiAccountId::player("defender"),
            QiAccountId::player("attacker"),
            defender_absorbed,
            QiTransferReason::Collision,
        ) {
            transfers.push(transfer);
        }
    }

    CollisionOutcome {
        attenuated_qi: attenuated,
        effective_hit,
        attacker_spent: injected,
        defender_lost,
        defender_absorbed,
        transfers,
    }
}

#[cfg(test)]
mod tests {
    use crate::cultivation::components::ColorKind;

    use super::*;
    use crate::qi_physics::traits::{SimpleStyleAttack, SimpleStyleDefense};

    #[test]
    fn collision_delivers_damage_after_distance() {
        let atk = SimpleStyleAttack::new(ColorKind::Sharp, 10.0);
        let def = SimpleStyleDefense::new(ColorKind::Solid, 0.2);
        let outcome = qi_collision(&atk, &def, 3.0, &EnvField::default());
        assert!(outcome.attenuated_qi < 10.0);
        assert!(outcome.defender_lost > 0.0);
    }

    #[test]
    fn low_purity_fails_acoustic_threshold() {
        let mut atk = SimpleStyleAttack::new(ColorKind::Sharp, 10.0);
        atk.purity = 0.1;
        let def = SimpleStyleDefense::new(ColorKind::Solid, 0.0);
        let outcome = qi_collision(&atk, &def, 1.0, &EnvField::default());
        assert_eq!(outcome.effective_hit, 0.0);
        assert!(outcome.transfers.is_empty());
    }

    #[test]
    fn strong_resistance_reduces_loss() {
        let atk = SimpleStyleAttack::new(ColorKind::Sharp, 10.0);
        let weak = SimpleStyleDefense::new(ColorKind::Solid, 0.1);
        let strong = SimpleStyleDefense::new(ColorKind::Solid, 0.8);
        let weak_out = qi_collision(&atk, &weak, 1.0, &EnvField::default());
        let strong_out = qi_collision(&atk, &strong, 1.0, &EnvField::default());
        assert!(strong_out.defender_lost < weak_out.defender_lost);
    }

    #[test]
    fn drain_affinity_is_clamped_to_half_spend() {
        let atk = SimpleStyleAttack::new(ColorKind::Sharp, 10.0);
        let mut def = SimpleStyleDefense::new(ColorKind::Solid, 0.0);
        def.drain_affinity = 1.0;
        let outcome = qi_collision(&atk, &def, 0.0, &EnvField::default());
        assert!(outcome.defender_absorbed <= 5.0);
    }

    #[test]
    fn collision_records_bidirectional_transfers_when_absorbed() {
        let atk = SimpleStyleAttack::new(ColorKind::Sharp, 10.0);
        let mut def = SimpleStyleDefense::new(ColorKind::Solid, 0.0);
        def.drain_affinity = 0.5;
        let outcome = qi_collision(&atk, &def, 0.0, &EnvField::default());
        assert_eq!(outcome.transfers.len(), 2);
    }

    #[test]
    fn active_rhythm_amplifies_effective_hit() {
        let atk = SimpleStyleAttack::new(ColorKind::Sharp, 10.0);
        let def = SimpleStyleDefense::new(ColorKind::Solid, 0.0);
        let base = qi_collision(&atk, &def, 0.0, &EnvField::default());
        let active = EnvField {
            rhythm_multiplier: 1.2,
            ..Default::default()
        };
        let boosted = qi_collision(&atk, &def, 0.0, &active);
        assert!(boosted.effective_hit > base.effective_hit);
    }

    #[test]
    fn zero_injected_qi_has_no_effect() {
        let atk = SimpleStyleAttack::new(ColorKind::Sharp, 0.0);
        let def = SimpleStyleDefense::new(ColorKind::Solid, 0.0);
        let outcome = qi_collision(&atk, &def, 0.0, &EnvField::default());
        assert_eq!(outcome.defender_lost, 0.0);
    }
}
