use super::constants::{QI_AMBIENT_EXCRETION_PER_SEC, QI_PER_ZONE_UNIT, QI_REGEN_COEF};
use super::env::{ContainerKind, EnvField};

pub fn qi_excretion(
    initial: f64,
    container: ContainerKind,
    elapsed_secs: f64,
    env: EnvField,
) -> f64 {
    if !initial.is_finite() {
        return env.local_zone_qi.max(0.0);
    }
    if initial <= env.local_zone_qi {
        return initial.max(0.0);
    }
    if !elapsed_secs.is_finite() || elapsed_secs <= 0.0 {
        return initial.max(env.local_zone_qi);
    }

    let pressure_delta = initial - env.local_zone_qi;
    let rate = QI_AMBIENT_EXCRETION_PER_SEC
        * container.seal_multiplier()
        * env.rhythm_factor()
        * env.tsy_drain_factor();
    let leaked_ratio = 1.0 - (-rate * elapsed_secs).exp();
    let leaked = pressure_delta * leaked_ratio.clamp(0.0, 1.0);

    (initial - leaked).max(env.local_zone_qi)
}

pub fn qi_excretion_loss(
    initial: f64,
    container: ContainerKind,
    elapsed_secs: f64,
    env: EnvField,
) -> f64 {
    (initial - qi_excretion(initial, container, elapsed_secs, env)).max(0.0)
}

pub fn regen_from_zone(zone_qi: f64, rate: f64, integrity: f64, room: f64) -> (f64, f64) {
    if !zone_qi.is_finite() || zone_qi <= 0.0 || !room.is_finite() || room <= 0.0 {
        return (0.0, 0.0);
    }
    let rate = if rate.is_finite() { rate.max(0.0) } else { 0.0 };
    let integrity = if integrity.is_finite() {
        integrity.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let raw_gain = zone_qi * rate * integrity * QI_REGEN_COEF;
    let capped_gain = raw_gain.min(room);
    let drain = capped_gain / QI_PER_ZONE_UNIT;
    if drain > zone_qi {
        let actual_drain = zone_qi;
        let actual_gain = actual_drain * QI_PER_ZONE_UNIT;
        (actual_gain, actual_drain)
    } else {
        (capped_gain, drain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn high_concentration_leaks_toward_zone() {
        let env = EnvField::new(0.2);
        let remaining = qi_excretion(1.0, ContainerKind::AmbientField, 60.0, env);
        assert!(remaining < 1.0);
        assert!(remaining > 0.2);
    }

    #[test]
    fn equal_pressure_does_not_leak() {
        let env = EnvField::new(0.7);
        assert_eq!(
            qi_excretion(0.7, ContainerKind::SealedInBone, 3600.0, env),
            0.7
        );
    }

    #[test]
    fn below_zone_pressure_stays_silent() {
        let env = EnvField::new(0.8);
        assert_eq!(
            qi_excretion(0.4, ContainerKind::LooseInPill, 3600.0, env),
            0.4
        );
    }

    #[test]
    fn dead_zone_allows_true_zero_approach() {
        let env = EnvField::dead_zone();
        let remaining = qi_excretion(0.4, ContainerKind::AmbientField, 100_000.0, env);
        assert!(remaining < 0.001);
    }

    #[test]
    fn sealed_relic_leaks_less_than_loose_pill() {
        let env = EnvField::new(0.0);
        let relic = qi_excretion(1.0, ContainerKind::SealedAncientRelic, 10_000.0, env);
        let pill = qi_excretion(1.0, ContainerKind::LooseInPill, 10_000.0, env);
        assert!(relic > pill);
    }

    #[test]
    fn elapsed_zero_keeps_initial() {
        assert_eq!(
            qi_excretion(1.0, ContainerKind::AmbientField, 0.0, EnvField::new(0.1)),
            1.0
        );
    }

    #[test]
    fn loss_reports_initial_minus_remaining() {
        let env = EnvField::new(0.0);
        let remaining = qi_excretion(1.0, ContainerKind::AmbientField, 10.0, env);
        let loss = qi_excretion_loss(1.0, ContainerKind::AmbientField, 10.0, env);
        assert!((1.0 - remaining - loss).abs() < 1e-9);
    }

    #[test]
    fn regen_from_zone_is_zero_sum_scaled() {
        let (gain, drain) = regen_from_zone(0.5, 2.0, 1.0, 99.0);
        assert_eq!(gain, 0.01);
        assert!(gain > 0.0);
        assert!((gain / QI_PER_ZONE_UNIT - drain).abs() < 1e-9);
    }

    #[test]
    fn regen_respects_available_room() {
        let (gain, drain) = regen_from_zone(1.0, 1_000.0, 1.0, 3.0);
        assert_eq!(gain, 3.0);
        assert_eq!(drain, 3.0 / QI_PER_ZONE_UNIT);
    }

    #[test]
    fn regen_caps_drain_to_available_zone_qi() {
        let (gain, drain) = regen_from_zone(0.001, 1_000_000.0, 1.0, 1_000_000.0);
        assert_eq!(drain, 0.001);
        assert_eq!(gain, 0.001 * QI_PER_ZONE_UNIT);
    }
}
