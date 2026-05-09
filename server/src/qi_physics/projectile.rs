use crate::cultivation::components::Realm;

use super::env::CarrierGrade;
use super::{finite_non_negative, QiPhysicsError};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConeDispersionShot {
    pub index: usize,
    pub yaw_degrees: f64,
    pub pitch_degrees: f64,
    pub distance_blocks: f64,
    pub tracking_degrees: f64,
}

pub fn cone_dispersion(
    projectile_count: usize,
    cone_degrees: f64,
    max_distance_blocks: f64,
    mastery: u8,
) -> Result<Vec<ConeDispersionShot>, QiPhysicsError> {
    let cone_degrees = finite_non_negative(cone_degrees, "cone.cone_degrees")?;
    let distance = finite_non_negative(max_distance_blocks, "cone.max_distance_blocks")?;
    if projectile_count == 0 {
        return Ok(Vec::new());
    }

    let mastery_ratio = f64::from(mastery.min(100)) / 100.0;
    let narrowed_cone = cone_degrees * (1.0 - 0.5 * mastery_ratio);
    let tracking = 5.0 * mastery_ratio;
    let center = (projectile_count - 1) as f64 / 2.0;
    let step = if projectile_count <= 1 {
        0.0
    } else {
        narrowed_cone / (projectile_count - 1) as f64
    };

    Ok((0..projectile_count)
        .map(|index| {
            let offset = index as f64 - center;
            ConeDispersionShot {
                index,
                yaw_degrees: offset * step,
                pitch_degrees: if index % 2 == 0 { 1.5 } else { -1.5 } * mastery_ratio,
                distance_blocks: distance,
                tracking_degrees: tracking,
            }
        })
        .collect())
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HighDensityInjectionOutcome {
    pub payload_qi: f64,
    pub wound_qi: f64,
    pub contamination_qi: f64,
    pub overload_ratio: f64,
    pub triggers_overload_tear: bool,
}

pub fn high_density_inject(
    payload_qi: f64,
    caster_qi_max: f64,
    color_matched: bool,
    mastery: u8,
) -> Result<HighDensityInjectionOutcome, QiPhysicsError> {
    let payload = finite_non_negative(payload_qi, "inject.payload_qi")?;
    let qi_max = finite_non_negative(caster_qi_max, "inject.caster_qi_max")?;
    let mastery_ratio = f64::from(mastery.min(100)) / 100.0;
    let wound_ratio = 1.5 + 0.3 * mastery_ratio;
    let color_multiplier = if color_matched { 1.3 } else { 0.6 };
    let overload_ratio = if qi_max > f64::EPSILON {
        payload / qi_max
    } else {
        0.0
    };

    Ok(HighDensityInjectionOutcome {
        payload_qi: payload,
        wound_qi: payload * wound_ratio * color_multiplier,
        contamination_qi: payload * 0.3,
        overload_ratio,
        triggers_overload_tear: overload_ratio > 0.30,
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArmorPenetrationOutcome {
    pub base_damage: f64,
    pub ignored_defense_ratio: f64,
    pub effective_damage: f64,
    pub carrier_shatter_probability: f64,
}

pub fn armor_penetrate(
    base_damage: f64,
    target_defense_ratio: f64,
    caster_realm: Realm,
    mastery: u8,
    carrier_grade: CarrierGrade,
) -> Result<ArmorPenetrationOutcome, QiPhysicsError> {
    let base = finite_non_negative(base_damage, "armor.base_damage")?;
    let defense =
        finite_non_negative(target_defense_ratio, "armor.target_defense_ratio")?.clamp(0.0, 1.0);
    let mastery_ratio = f64::from(mastery.min(100)) / 100.0;
    let void_master = caster_realm == Realm::Void && mastery >= 100;
    let ignored = if void_master { 0.90 } else { 0.75 };
    let multiplier = if void_master { 2.5 } else { 1.8 };
    let carrier_bonus = match carrier_grade {
        CarrierGrade::AncientRelic => 0.10,
        CarrierGrade::SpiritWeapon => 0.05,
        CarrierGrade::PhysicalWeapon | CarrierGrade::BareQi => 0.0,
    };
    let shatter = (0.50 - 0.35 * mastery_ratio - carrier_bonus).clamp(0.05, 0.50);

    Ok(ArmorPenetrationOutcome {
        base_damage: base,
        ignored_defense_ratio: ignored,
        effective_damage: base * multiplier * (1.0 - defense * (1.0 - ignored)),
        carrier_shatter_probability: shatter,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cone_dispersion_places_five_projectiles_symmetrically() {
        let shots = cone_dispersion(5, 60.0, 30.0, 0).unwrap();
        assert_eq!(shots.len(), 5);
        assert_eq!(shots[0].yaw_degrees, -30.0);
        assert_eq!(shots[2].yaw_degrees, 0.0);
        assert_eq!(shots[4].yaw_degrees, 30.0);
    }

    #[test]
    fn mastery_narrows_cone_and_adds_tracking() {
        let novice = cone_dispersion(5, 60.0, 30.0, 0).unwrap();
        let master = cone_dispersion(5, 60.0, 30.0, 100).unwrap();
        assert!(master[4].yaw_degrees < novice[4].yaw_degrees);
        assert_eq!(master[4].tracking_degrees, 5.0);
    }

    #[test]
    fn high_density_injection_tracks_color_match_and_overload() {
        let matched = high_density_inject(40.0, 100.0, true, 100).unwrap();
        let mismatched = high_density_inject(40.0, 100.0, false, 100).unwrap();
        assert!(matched.wound_qi > mismatched.wound_qi);
        assert!(matched.triggers_overload_tear);
        assert_eq!(matched.contamination_qi, 12.0);
    }

    #[test]
    fn armor_penetration_void_master_reaches_ninety_percent() {
        let out = armor_penetrate(20.0, 0.8, Realm::Void, 100, CarrierGrade::AncientRelic).unwrap();
        assert_eq!(out.ignored_defense_ratio, 0.90);
        assert!(out.effective_damage > 40.0);
        assert!((out.carrier_shatter_probability - 0.05).abs() <= 1e-9);
    }
}
