use super::constants::{
    QI_DENSITY_GAZE_THRESHOLD, QI_REGION_STARVATION_THRESHOLD, QI_TIANDAO_DECAY_PER_ERA_MAX,
    QI_TIANDAO_DECAY_PER_ERA_MIN,
};
use super::env::EnvField;
use super::ledger::WorldQiBudget;
use super::{finite_non_negative, QiPhysicsError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TribulationCause {
    DensityGaze,
    RegionStarvation,
}

pub fn tribulation_trigger(env: &EnvField) -> Option<TribulationCause> {
    if env.local_zone_qi >= QI_DENSITY_GAZE_THRESHOLD {
        Some(TribulationCause::DensityGaze)
    } else if env.local_zone_qi <= QI_REGION_STARVATION_THRESHOLD {
        Some(TribulationCause::RegionStarvation)
    } else {
        None
    }
}

pub fn collapse_redistribute_qi(
    stored_qi: f64,
    surrounding_zones: &[(String, f64)],
) -> Result<Vec<(String, f64)>, QiPhysicsError> {
    let stored_qi = finite_non_negative(stored_qi, "rift.stored_qi")?;
    if stored_qi == 0.0 || surrounding_zones.is_empty() {
        return Ok(Vec::new());
    }

    let weights: Vec<f64> = surrounding_zones
        .iter()
        .map(|(_, qi)| (1.0 - qi.clamp(0.0, 1.0)).max(0.01))
        .collect();
    let total_weight: f64 = weights.iter().sum();

    Ok(surrounding_zones
        .iter()
        .zip(weights.iter())
        .map(|((name, _), weight)| (name.clone(), stored_qi * *weight / total_weight))
        .collect())
}

pub fn era_decay_step(budget: &mut WorldQiBudget, era_factor: f64) -> Result<f64, QiPhysicsError> {
    let era_factor = finite_non_negative(era_factor, "era_factor")?.clamp(0.0, 1.0);
    let ratio = QI_TIANDAO_DECAY_PER_ERA_MIN
        + (QI_TIANDAO_DECAY_PER_ERA_MAX - QI_TIANDAO_DECAY_PER_ERA_MIN) * era_factor;
    budget.apply_era_decay(ratio)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn density_gaze_triggers_at_high_qi() {
        assert_eq!(
            tribulation_trigger(&EnvField::new(0.95)),
            Some(TribulationCause::DensityGaze)
        );
    }

    #[test]
    fn starvation_triggers_at_low_qi() {
        assert_eq!(
            tribulation_trigger(&EnvField::new(0.01)),
            Some(TribulationCause::RegionStarvation)
        );
    }

    #[test]
    fn normal_zone_does_not_trigger() {
        assert_eq!(tribulation_trigger(&EnvField::new(0.5)), None);
    }

    #[test]
    fn collapse_redistributes_more_to_low_pressure_zone() {
        let out =
            collapse_redistribute_qi(10.0, &[("low".to_string(), 0.1), ("high".to_string(), 0.9)])
                .unwrap();
        assert!(out[0].1 > out[1].1);
        assert!((out[0].1 + out[1].1 - 10.0).abs() < 1e-9);
    }

    #[test]
    fn era_decay_step_uses_min_to_max_range() {
        let mut budget = WorldQiBudget::from_total(100.0);
        let decay = era_decay_step(&mut budget, 1.0).unwrap();
        assert_eq!(decay, 3.0);
        assert_eq!(budget.current_total, 97.0);
    }
}
