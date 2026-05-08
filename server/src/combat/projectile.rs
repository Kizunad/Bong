use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, DVec3, Entity};

use crate::combat::carrier::CarrierKind;
use crate::combat::decay::CarrierGrade;
use crate::cultivation::components::ColorKind;
use crate::qi_physics::{CarrierGrade as QiCarrierGrade, MediumKind, StyleAttack};

#[derive(bevy_ecs::component::Component, Debug, Clone, Copy, PartialEq)]
pub struct QiProjectile {
    pub owner: Option<Entity>,
    pub qi_payload: f32,
}

#[derive(bevy_ecs::component::Component, Debug, Clone, Copy, PartialEq)]
pub struct AnqiProjectileFlight {
    pub carrier_kind: CarrierKind,
    pub qi_color: ColorKind,
    pub carrier_grade: CarrierGrade,
    pub spawn_pos: DVec3,
    pub prev_pos: DVec3,
    pub velocity: DVec3,
    pub max_distance: f32,
    pub hitbox_inflation: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnqiStyleAttack {
    pub color: ColorKind,
    pub qi_payload: f64,
    pub carrier_grade: CarrierGrade,
}

impl AnqiStyleAttack {
    pub fn new(flight: &AnqiProjectileFlight, projectile: &QiProjectile) -> Self {
        Self {
            color: flight.qi_color,
            qi_payload: f64::from(projectile.qi_payload.max(0.0)),
            carrier_grade: flight.carrier_grade,
        }
    }

    fn qi_carrier_grade(self) -> QiCarrierGrade {
        match self.carrier_grade {
            CarrierGrade::Mundane => QiCarrierGrade::BareQi,
            CarrierGrade::Bone | CarrierGrade::Beast => QiCarrierGrade::SpiritWeapon,
            CarrierGrade::Spirit | CarrierGrade::Relic => QiCarrierGrade::AncientRelic,
        }
    }
}

impl StyleAttack for AnqiStyleAttack {
    fn style_color(&self) -> ColorKind {
        self.color
    }

    fn injected_qi(&self) -> f64 {
        self.qi_payload
    }

    fn medium(&self) -> MediumKind {
        MediumKind {
            color: self.color,
            carrier: self.qi_carrier_grade(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectileDespawnReason {
    HitTarget,
    HitBlock,
    OutOfRange,
    NaturalDecay,
}

pub fn segment_point_distance(a: DVec3, b: DVec3, point: DVec3) -> f64 {
    let ab = b - a;
    let ab_len_sq = ab.length_squared();
    if ab_len_sq <= f64::EPSILON {
        return point.distance(a);
    }
    let t = ((point - a).dot(ab) / ab_len_sq).clamp(0.0, 1.0);
    let closest = a + ab * t;
    point.distance(closest)
}

pub fn residual_qi_after_miss(qi_at_despawn: f32) -> (f32, f32) {
    let qi = qi_at_despawn.max(0.0);
    (qi * 0.7, qi * 0.3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_distance_projects_inside_segment() {
        let d = segment_point_distance(
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(10.0, 0.0, 0.0),
            DVec3::new(5.0, 0.4, 0.0),
        );
        assert!((d - 0.4).abs() <= 0.001);
    }

    #[test]
    fn miss_residual_keeps_thirty_percent() {
        let (evaporated, residual) = residual_qi_after_miss(40.0);
        assert_eq!(evaporated, 28.0);
        assert_eq!(residual, 12.0);
    }

    #[test]
    fn anqi_style_attack_exposes_projectile_payload_to_qi_physics() {
        let flight = AnqiProjectileFlight {
            carrier_kind: CarrierKind::YibianShougu,
            qi_color: ColorKind::Sharp,
            carrier_grade: CarrierGrade::Relic,
            spawn_pos: DVec3::ZERO,
            prev_pos: DVec3::ZERO,
            velocity: DVec3::ZERO,
            max_distance: 32.0,
            hitbox_inflation: 0.1,
        };
        let projectile = QiProjectile {
            owner: None,
            qi_payload: 12.0,
        };

        let attack = AnqiStyleAttack::new(&flight, &projectile);

        assert_eq!(attack.style_color(), ColorKind::Sharp);
        assert_eq!(attack.injected_qi(), 12.0);
        assert_eq!(attack.medium().carrier, QiCarrierGrade::AncientRelic);
    }
}
