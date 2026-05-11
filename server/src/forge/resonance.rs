//! plan-qixiu-depth-v1 P3 — 法器共鸣。

use crate::cultivation::components::QiColor;
use crate::forge::artifact_color::ArtifactColor;

pub fn compute_resonance(
    artifact_color: &ArtifactColor,
    user_color: &QiColor,
    groove_total_depth: f64,
    groove_depth_cap: f64,
) -> f64 {
    let maturity = if groove_depth_cap > 0.0 {
        (groove_total_depth / groove_depth_cap).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let color_match = if artifact_color.is_uncolored() {
        0.5
    } else if artifact_color.main == user_color.main {
        1.0
    } else if user_color.secondary == Some(artifact_color.main)
        || artifact_color.secondary == Some(user_color.main)
    {
        0.6
    } else {
        0.2
    };
    (color_match * maturity).clamp(0.0, 1.0)
}

pub fn damage_resonance_multiplier(resonance: f64) -> f32 {
    (0.7 + 0.6 * resonance.clamp(0.0, 1.0)) as f32
}

pub fn carrier_seal_efficiency_multiplier(resonance: f64) -> f32 {
    (0.8 + 0.4 * resonance.clamp(0.0, 1.0)) as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::{ColorKind, QiColor};
    use crate::forge::artifact_color::ArtifactColor;

    fn qi(main: ColorKind, secondary: Option<ColorKind>) -> QiColor {
        QiColor {
            main,
            secondary,
            ..Default::default()
        }
    }

    #[test]
    fn same_color_max_resonance() {
        let artifact = ArtifactColor::from_initial_color(ColorKind::Solid, 100.0);
        assert_eq!(
            compute_resonance(&artifact, &qi(ColorKind::Solid, None), 100.0, 100.0),
            1.0
        );
    }

    #[test]
    fn no_color_half_resonance() {
        let artifact = ArtifactColor::uncolored();
        assert_eq!(
            compute_resonance(&artifact, &qi(ColorKind::Sharp, None), 50.0, 100.0),
            0.25
        );
    }

    #[test]
    fn different_color_low_resonance() {
        let artifact = ArtifactColor::from_initial_color(ColorKind::Sharp, 100.0);
        assert_eq!(
            compute_resonance(&artifact, &qi(ColorKind::Heavy, None), 100.0, 100.0),
            0.2
        );
    }

    #[test]
    fn secondary_color_partial_resonance() {
        let artifact = ArtifactColor::from_initial_color(ColorKind::Sharp, 100.0);
        assert_eq!(
            compute_resonance(
                &artifact,
                &qi(ColorKind::Heavy, Some(ColorKind::Sharp)),
                100.0,
                100.0,
            ),
            0.6
        );
    }

    #[test]
    fn damage_with_zero_resonance_is_70_pct() {
        assert!((damage_resonance_multiplier(0.0) - 0.7).abs() < 1e-6);
    }

    #[test]
    fn damage_with_full_resonance_is_130_pct() {
        assert!((damage_resonance_multiplier(1.0) - 1.3).abs() < 1e-6);
    }

    #[test]
    fn carrier_seal_efficiency_scales_with_resonance() {
        assert!((carrier_seal_efficiency_multiplier(0.0) - 0.8).abs() < 1e-6);
        assert!((carrier_seal_efficiency_multiplier(0.5) - 1.0).abs() < 1e-6);
        assert!((carrier_seal_efficiency_multiplier(1.0) - 1.2).abs() < 1e-6);
    }

    #[test]
    fn resonance_clamps_non_finite_depth_cap_to_zero() {
        let artifact = ArtifactColor::from_initial_color(ColorKind::Solid, 100.0);
        assert_eq!(
            compute_resonance(&artifact, &qi(ColorKind::Solid, None), 100.0, 0.0),
            0.0
        );
    }
}
