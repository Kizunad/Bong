use serde::{Deserialize, Serialize};

use crate::cultivation::components::ColorKind;
use crate::qi_physics;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CarrierGrade {
    Mundane,
    Bone,
    Beast,
    Spirit,
    Relic,
}

/// plan-anqi-v1 §3.1.D: shared ranged qi retention curve.
pub fn hit_qi_ratio(distance_blocks: f32, color: ColorKind, grade: CarrierGrade) -> f32 {
    let medium = qi_physics::MediumKind {
        color,
        carrier: match grade {
            CarrierGrade::Mundane => qi_physics::CarrierGrade::BareQi,
            CarrierGrade::Bone | CarrierGrade::Beast => qi_physics::CarrierGrade::SpiritWeapon,
            CarrierGrade::Spirit | CarrierGrade::Relic => qi_physics::CarrierGrade::AncientRelic,
        },
    };
    qi_physics::qi_distance_atten(1.0, f64::from(distance_blocks), medium) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(left: f32, right: f32) {
        assert!(
            (left - right).abs() <= 0.001,
            "expected {left:.3} ~= {right:.3}"
        );
    }

    #[test]
    fn fits_worldview_anchor_points() {
        approx_eq(
            hit_qi_ratio(0.0, ColorKind::Mellow, CarrierGrade::Mundane),
            1.0,
        );
        approx_eq(
            hit_qi_ratio(10.0, ColorKind::Mellow, CarrierGrade::Mundane),
            0.737,
        );
        approx_eq(
            hit_qi_ratio(50.0, ColorKind::Solid, CarrierGrade::Relic),
            0.494,
        );
    }

    #[test]
    fn clamps_out_of_range_distances() {
        approx_eq(
            hit_qi_ratio(-5.0, ColorKind::Mellow, CarrierGrade::Mundane),
            1.0,
        );
        approx_eq(
            hit_qi_ratio(100.0, ColorKind::Mellow, CarrierGrade::Mundane),
            0.048,
        );
        approx_eq(
            hit_qi_ratio(100.0, ColorKind::Solid, CarrierGrade::Relic),
            0.244,
        );
    }

    #[test]
    fn better_carriers_retain_more_qi() {
        let mundane = hit_qi_ratio(30.0, ColorKind::Mellow, CarrierGrade::Mundane);
        let beast = hit_qi_ratio(30.0, ColorKind::Mellow, CarrierGrade::Beast);
        let relic = hit_qi_ratio(30.0, ColorKind::Mellow, CarrierGrade::Relic);
        assert!(mundane < beast && beast < relic);
    }
}
