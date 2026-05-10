use super::constants::QI_DECAY_PER_BLOCK;
use super::env::{EnvField, MediumKind};

pub fn qi_distance_atten(initial: f64, distance_blocks: f64, medium: MediumKind) -> f64 {
    if !initial.is_finite() || initial <= 0.0 {
        return 0.0;
    }
    if !distance_blocks.is_finite() || distance_blocks <= 0.0 {
        return initial;
    }

    let loss_per_block = (QI_DECAY_PER_BLOCK + medium.loss_bonus_per_block()).clamp(0.0, 0.95);
    initial * (1.0 - loss_per_block).powf(distance_blocks)
}

pub fn qi_distance_atten_in_env(
    initial: f64,
    distance_blocks: f64,
    medium: MediumKind,
    env: &EnvField,
) -> f64 {
    qi_distance_atten(
        initial,
        distance_blocks * env.law_disruption_distance_multiplier(),
        medium,
    )
}

#[cfg(test)]
mod tests {
    use crate::cultivation::components::ColorKind;

    use super::*;
    use crate::qi_physics::env::{CarrierGrade, MediumKind};

    #[test]
    fn zero_distance_keeps_initial_qi() {
        assert_eq!(qi_distance_atten(10.0, 0.0, MediumKind::default()), 10.0);
    }

    #[test]
    fn negative_initial_returns_zero() {
        assert_eq!(qi_distance_atten(-1.0, 4.0, MediumKind::default()), 0.0);
    }

    #[test]
    fn finite_distance_loses_qi() {
        let out = qi_distance_atten(10.0, 3.0, MediumKind::default());
        assert!(out > 0.0 && out < 10.0);
    }

    #[test]
    fn far_distance_approaches_zero() {
        let out = qi_distance_atten(10.0, 1_000.0, MediumKind::default());
        assert!(out < 0.001);
    }

    #[test]
    fn ancient_relic_carries_farther_than_bare_qi() {
        let bare = qi_distance_atten(10.0, 20.0, MediumKind::bare(ColorKind::Mellow));
        let relic = qi_distance_atten(
            10.0,
            20.0,
            MediumKind {
                color: ColorKind::Mellow,
                carrier: CarrierGrade::AncientRelic,
            },
        );
        assert!(relic > bare);
    }

    #[test]
    fn violent_color_loses_more_than_solid_color() {
        let violent = qi_distance_atten(10.0, 10.0, MediumKind::bare(ColorKind::Violent));
        let solid = qi_distance_atten(10.0, 10.0, MediumKind::bare(ColorKind::Solid));
        assert!(violent < solid);
    }

    #[test]
    fn law_disruption_offsets_effective_hit_distance() {
        let calm =
            qi_distance_atten_in_env(10.0, 10.0, MediumKind::default(), &EnvField::default());
        let disrupted = qi_distance_atten_in_env(
            10.0,
            10.0,
            MediumKind::default(),
            &EnvField::default().with_law_disruption(1.0),
        );
        assert!(disrupted < calm);
    }
}
