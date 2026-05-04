use serde::{Deserialize, Serialize};

use crate::cultivation::components::ColorKind;

const BASE_LOSS_PER_BLOCK: f32 = 0.06;

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
    let distance = distance_blocks.max(0.0);
    let retention = 1.0 - BASE_LOSS_PER_BLOCK * distance
        + color_bonus_per_block(color) * distance
        + carrier_bonus_per_block(grade) * distance;
    retention.clamp(0.0, 1.0)
}

fn color_bonus_per_block(color: ColorKind) -> f32 {
    match color {
        ColorKind::Solid => 0.024,
        ColorKind::Light => 0.021,
        ColorKind::Sharp => 0.018,
        ColorKind::Mellow => 0.0,
        ColorKind::Heavy => -0.006,
        _ => 0.0,
    }
}

fn carrier_bonus_per_block(grade: CarrierGrade) -> f32 {
    match grade {
        CarrierGrade::Mundane => 0.0,
        CarrierGrade::Bone => 0.018,
        CarrierGrade::Beast => 0.032,
        CarrierGrade::Spirit => 0.038,
        CarrierGrade::Relic => 0.046,
    }
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
            0.4,
        );
        approx_eq(
            hit_qi_ratio(50.0, ColorKind::Solid, CarrierGrade::Beast),
            0.8,
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
            0.0,
        );
        approx_eq(
            hit_qi_ratio(100.0, ColorKind::Solid, CarrierGrade::Relic),
            1.0,
        );
    }
}
