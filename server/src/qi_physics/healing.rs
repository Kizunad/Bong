//! 医道治疗算子（plan-yidao-v1）。
//!
//! 本模块只放纯物理公式：接经、排异、急救、续命、群体接经的数值边界。
//! ECS 读写、schema 与表现事件留在 `combat::yidao`。

use crate::cultivation::components::Realm;

use super::{finite_non_negative, QiPhysicsError};

pub const PEACE_COLOR_CONTAM_PURGE_MULTIPLIER: f64 = 3.0;
pub const PEACE_COLOR_CAST_TIME_MULTIPLIER: f64 = 0.8;
pub const PEACE_COLOR_LIFE_KARMA_MULTIPLIER: f64 = 0.9;
pub const PEACE_COLOR_MASS_CAP_BONUS: u32 = 5;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeridianRepairOutcome {
    pub qi_cost: f64,
    pub success_threshold: f64,
    pub medic_karma_on_failure: f64,
    pub patient_karma_on_failure: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContamPurgeOutcome {
    pub qi_cost: f64,
    pub purge_amount: f64,
    pub residual_total: f64,
    pub post_cast_natural_purge_multiplier: f64,
    pub post_cast_duration_ticks: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EmergencyStabilizeOutcome {
    pub qi_cost: f64,
    pub hp_restore: f32,
    pub dying_window_ticks: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LifeExtendOutcome {
    pub qi_cost: f64,
    pub medic_karma_delta: f64,
    pub medic_qi_max_loss_ratio: f64,
    pub patient_qi_max_loss_ratio: f64,
    pub patient_realm_regress_chance: f64,
    pub revive_hp_fraction: f32,
    pub window_ticks: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MassMeridianRepairOutcome {
    pub capacity: u32,
    pub qi_cost_ratio: f64,
    pub medic_qi_max_loss_ratio_per_patient: f64,
    pub medic_karma_delta_per_patient: f64,
    pub success_threshold: f64,
}

pub fn meridian_repair(
    medic_qi_max: f64,
    realm: Realm,
    mastery: f64,
    peace_color: bool,
) -> Result<MeridianRepairOutcome, QiPhysicsError> {
    let medic_qi_max = finite_non_negative(medic_qi_max, "medic_qi_max")?;
    let mastery = mastery_unit(mastery)?;
    let cost_ratio = lerp(0.8, 0.5, mastery);
    let mut success_threshold = lerp(realm_base_success(realm), 0.99, mastery);
    if peace_color {
        success_threshold = (success_threshold + 0.04).min(0.99);
    }
    let failure_karma = lerp(1.0, 0.5, mastery);
    Ok(MeridianRepairOutcome {
        qi_cost: medic_qi_max * cost_ratio,
        success_threshold,
        medic_karma_on_failure: failure_karma,
        patient_karma_on_failure: failure_karma,
    })
}

pub fn contam_purge(
    medic_qi_max: f64,
    contamination_total: f64,
    realm: Realm,
    mastery: f64,
    peace_color: bool,
) -> Result<ContamPurgeOutcome, QiPhysicsError> {
    let medic_qi_max = finite_non_negative(medic_qi_max, "medic_qi_max")?;
    let contamination_total = finite_non_negative(contamination_total, "contamination_total")?;
    let mastery = mastery_unit(mastery)?;
    let cost_ratio = lerp(0.5, 0.3, mastery);
    let realm_bonus = match realm {
        Realm::Awaken | Realm::Induce => 0.0,
        Realm::Condense => 0.05,
        Realm::Solidify => 0.1,
        Realm::Spirit => 0.15,
        Realm::Void => 0.2,
    };
    let mut purge_ratio = (lerp(0.3, 0.6, mastery) + realm_bonus).clamp(0.0, 0.85);
    if peace_color {
        purge_ratio = (purge_ratio * PEACE_COLOR_CONTAM_PURGE_MULTIPLIER).min(1.0);
    }
    let purge_amount = contamination_total * purge_ratio;
    Ok(ContamPurgeOutcome {
        qi_cost: medic_qi_max * cost_ratio,
        purge_amount,
        residual_total: (contamination_total - purge_amount).max(0.0),
        post_cast_natural_purge_multiplier: if mastery >= 1.0 { 2.0 } else { 1.0 },
        post_cast_duration_ticks: (30.0 * 20.0 * mastery).round() as u64,
    })
}

pub fn emergency_stabilize(
    medic_qi_max: f64,
    patient_hp_max: f32,
    mastery: f64,
) -> Result<EmergencyStabilizeOutcome, QiPhysicsError> {
    let medic_qi_max = finite_non_negative(medic_qi_max, "medic_qi_max")?;
    let mastery = mastery_unit(mastery)?;
    let patient_hp_max =
        finite_non_negative(f64::from(patient_hp_max), "patient_hp_max")?.max(1.0) as f32;
    let restore_fraction = lerp(0.3, 0.5, mastery) as f32;
    Ok(EmergencyStabilizeOutcome {
        qi_cost: medic_qi_max * lerp(0.3, 0.1, mastery),
        hp_restore: patient_hp_max * restore_fraction,
        dying_window_ticks: (lerp(60.0, 90.0, mastery) * 20.0).round() as u64,
    })
}

pub fn life_extend(
    medic_qi_max: f64,
    mastery: f64,
    peace_color: bool,
) -> Result<LifeExtendOutcome, QiPhysicsError> {
    let medic_qi_max = finite_non_negative(medic_qi_max, "medic_qi_max")?;
    let mastery = mastery_unit(mastery)?;
    let mut karma = lerp(5.0, 2.5, mastery);
    if peace_color {
        karma *= PEACE_COLOR_LIFE_KARMA_MULTIPLIER;
    }
    Ok(LifeExtendOutcome {
        qi_cost: medic_qi_max * lerp(1.5, 1.0, mastery),
        medic_karma_delta: karma,
        medic_qi_max_loss_ratio: 0.10,
        patient_qi_max_loss_ratio: 0.10,
        patient_realm_regress_chance: lerp(0.5, 0.25, mastery),
        revive_hp_fraction: 0.5,
        window_ticks: (lerp(30.0, 60.0, mastery) * 20.0).round() as u64,
    })
}

pub fn mass_meridian_repair(
    local_qi_density: f64,
    realm: Realm,
    mastery: f64,
    peace_color: bool,
) -> Result<MassMeridianRepairOutcome, QiPhysicsError> {
    let local_qi_density = finite_non_negative(local_qi_density, "local_qi_density")?;
    let mastery = mastery_unit(mastery)?;
    let threshold = lerp(0.5, 0.2, mastery);
    let mut capacity = (local_qi_density / threshold).floor().max(0.0) as u32;
    if peace_color {
        capacity = capacity.saturating_add(PEACE_COLOR_MASS_CAP_BONUS);
    }
    Ok(MassMeridianRepairOutcome {
        capacity: if matches!(realm, Realm::Void) {
            capacity
        } else {
            0
        },
        qi_cost_ratio: 1.0,
        medic_qi_max_loss_ratio_per_patient: lerp(0.02, 0.01, mastery),
        medic_karma_delta_per_patient: lerp(0.1, 0.05, mastery),
        success_threshold: meridian_repair(1.0, realm, mastery, peace_color)?.success_threshold,
    })
}

pub fn yidao_cast_ticks(
    base_ticks: u64,
    mastery: f64,
    peace_color: bool,
) -> Result<u64, QiPhysicsError> {
    let mastery = mastery_unit(mastery)?;
    let multiplier = if peace_color {
        PEACE_COLOR_CAST_TIME_MULTIPLIER
    } else {
        1.0
    };
    Ok(
        ((base_ticks as f64) * lerp(1.0, 0.34, mastery) * multiplier)
            .round()
            .max(1.0) as u64,
    )
}

fn mastery_unit(mastery: f64) -> Result<f64, QiPhysicsError> {
    if mastery.is_finite() && (0.0..=100.0).contains(&mastery) {
        Ok(mastery / 100.0)
    } else {
        Err(QiPhysicsError::InvalidAmount {
            field: "mastery",
            value: mastery,
        })
    }
}

fn realm_base_success(realm: Realm) -> f64 {
    match realm {
        Realm::Awaken => 0.30,
        Realm::Induce => 0.42,
        Realm::Condense => 0.55,
        Realm::Solidify => 0.70,
        Realm::Spirit => 0.84,
        Realm::Void => 0.92,
    }
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meridian_repair_matches_awaken_and_void_bounds() {
        let low = meridian_repair(100.0, Realm::Awaken, 0.0, false).unwrap();
        assert_eq!(low.qi_cost, 80.0);
        assert_eq!(low.success_threshold, 0.30);
        assert_eq!(low.medic_karma_on_failure, 1.0);

        let high = meridian_repair(100.0, Realm::Void, 100.0, true).unwrap();
        assert_eq!(high.qi_cost, 50.0);
        assert_eq!(high.success_threshold, 0.99);
        assert_eq!(high.patient_karma_on_failure, 0.5);
    }

    #[test]
    fn contam_purge_peace_color_triples_then_clamps() {
        let normal = contam_purge(100.0, 30.0, Realm::Induce, 0.0, false).unwrap();
        assert_eq!(normal.qi_cost, 50.0);
        assert!((normal.purge_amount - 9.0).abs() < f64::EPSILON);

        let peace = contam_purge(100.0, 30.0, Realm::Void, 100.0, true).unwrap();
        assert_eq!(peace.purge_amount, 30.0);
        assert_eq!(peace.residual_total, 0.0);
        assert_eq!(peace.post_cast_natural_purge_multiplier, 2.0);
        assert_eq!(peace.post_cast_duration_ticks, 600);
    }

    #[test]
    fn emergency_stabilize_scales_hp_and_window() {
        let out = emergency_stabilize(80.0, 120.0, 100.0).unwrap();
        assert_eq!(out.qi_cost, 8.0);
        assert_eq!(out.hp_restore, 60.0);
        assert_eq!(out.dying_window_ticks, 1800);
    }

    #[test]
    fn emergency_stabilize_rejects_non_finite_patient_hp_max() {
        let err = emergency_stabilize(80.0, f32::NAN, 50.0).unwrap_err();
        assert!(
            matches!(err, QiPhysicsError::InvalidAmount { field, .. } if field == "patient_hp_max")
        );
    }

    #[test]
    fn life_extend_keeps_permanent_costs_and_reduces_karma_by_mastery() {
        let out = life_extend(200.0, 100.0, true).unwrap();
        assert_eq!(out.qi_cost, 200.0);
        assert_eq!(out.medic_karma_delta, 2.25);
        assert_eq!(out.medic_qi_max_loss_ratio, 0.10);
        assert_eq!(out.patient_qi_max_loss_ratio, 0.10);
        assert_eq!(out.patient_realm_regress_chance, 0.25);
    }

    #[test]
    fn mass_repair_is_void_only_and_uses_density_threshold() {
        let blocked = mass_meridian_repair(9.0, Realm::Spirit, 100.0, false).unwrap();
        assert_eq!(blocked.capacity, 0);

        let out = mass_meridian_repair(9.0, Realm::Void, 100.0, true).unwrap();
        assert_eq!(out.capacity, 50);
        assert_eq!(out.medic_qi_max_loss_ratio_per_patient, 0.01);
        assert_eq!(out.medic_karma_delta_per_patient, 0.05);
    }

    #[test]
    fn invalid_mastery_is_rejected() {
        let err = life_extend(100.0, 101.0, false).unwrap_err();
        assert!(matches!(
            err,
            QiPhysicsError::InvalidAmount {
                field: "mastery",
                ..
            }
        ));
    }
}
