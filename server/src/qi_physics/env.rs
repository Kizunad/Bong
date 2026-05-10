use crate::cultivation::components::ColorKind;

use super::constants::{
    QI_RHYTHM_NEUTRAL, QI_TSY_DRAIN_FACTOR, QI_TSY_DRAIN_NONLINEAR_EXPONENT,
    VORTEX_TURBULENCE_ABSORPTION_MULTIPLIER, VORTEX_TURBULENCE_CAST_PRECISION_MULTIPLIER,
    VORTEX_TURBULENCE_DEFENSE_DRAIN_BONUS, VORTEX_TURBULENCE_SHELFLIFE_MULTIPLIER,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContainerKind {
    SealedInBone,
    LooseInPill,
    WieldedInWeapon,
    AmbientField,
    TurbulentField,
    SealedAncientRelic,
}

impl ContainerKind {
    pub fn seal_multiplier(self) -> f64 {
        match self {
            Self::SealedAncientRelic => 0.02,
            Self::SealedInBone => 0.12,
            Self::WieldedInWeapon => 0.35,
            Self::LooseInPill => 0.55,
            Self::AmbientField => 1.0,
            Self::TurbulentField => 1.0,
        }
    }

    pub fn allows_reverse_pressure(self) -> bool {
        matches!(self, Self::AmbientField | Self::TurbulentField)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CarrierGrade {
    BareQi,
    PhysicalWeapon,
    SpiritWeapon,
    AncientRelic,
}

impl CarrierGrade {
    pub fn loss_bonus_per_block(self) -> f64 {
        match self {
            Self::AncientRelic => -0.012,
            Self::SpiritWeapon => -0.006,
            Self::PhysicalWeapon => 0.008,
            Self::BareQi => 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MediumKind {
    pub color: ColorKind,
    pub carrier: CarrierGrade,
}

impl MediumKind {
    pub fn bare(color: ColorKind) -> Self {
        Self {
            color,
            carrier: CarrierGrade::BareQi,
        }
    }

    pub fn loss_bonus_per_block(self) -> f64 {
        let color_bonus = match self.color {
            ColorKind::Sharp => 0.012,
            ColorKind::Heavy => 0.004,
            ColorKind::Mellow => 0.0,
            ColorKind::Solid => -0.004,
            ColorKind::Light => 0.018,
            ColorKind::Intricate => 0.01,
            ColorKind::Gentle => -0.002,
            ColorKind::Insidious => 0.014,
            ColorKind::Violent => 0.02,
            ColorKind::Turbid => 0.024,
        };
        color_bonus + self.carrier.loss_bonus_per_block()
    }
}

impl Default for MediumKind {
    fn default() -> Self {
        Self::bare(ColorKind::Mellow)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnvField {
    pub local_zone_qi: f64,
    pub rhythm_multiplier: f64,
    pub tsy_intensity: f64,
    pub ambient_pressure: f64,
    pub turbulence_intensity: f64,
    pub law_disruption: f64,
}

impl EnvField {
    pub fn new(local_zone_qi: f64) -> Self {
        Self {
            local_zone_qi: local_zone_qi.clamp(0.0, 1.0),
            ..Self::default()
        }
    }

    pub fn dead_zone() -> Self {
        Self {
            local_zone_qi: 0.0,
            tsy_intensity: 1.0,
            ..Self::default()
        }
    }

    pub fn with_turbulence(mut self, intensity: f64) -> Self {
        self.turbulence_intensity = if intensity.is_finite() {
            intensity.clamp(0.0, 1.0)
        } else {
            0.0
        };
        self
    }

    pub fn with_law_disruption(mut self, intensity: f64) -> Self {
        self.law_disruption = if intensity.is_finite() {
            intensity.clamp(0.0, 1.0)
        } else {
            0.0
        };
        self
    }

    pub fn rhythm_factor(self) -> f64 {
        if self.rhythm_multiplier.is_finite() && self.rhythm_multiplier > 0.0 {
            self.rhythm_multiplier
        } else {
            QI_RHYTHM_NEUTRAL
        }
    }

    pub fn tsy_drain_factor(self) -> f64 {
        let intensity = self.tsy_intensity.clamp(0.0, 1.0);
        1.0 + QI_TSY_DRAIN_FACTOR * intensity.powf(QI_TSY_DRAIN_NONLINEAR_EXPONENT)
    }

    pub fn turbulence_shelflife_factor(self) -> f64 {
        1.0 + self.turbulence_intensity.clamp(0.0, 1.0)
            * (VORTEX_TURBULENCE_SHELFLIFE_MULTIPLIER - 1.0)
    }

    pub fn turbulence_absorption_factor(self) -> f64 {
        1.0 - self.turbulence_intensity.clamp(0.0, 1.0)
            * (1.0 - VORTEX_TURBULENCE_ABSORPTION_MULTIPLIER)
    }

    pub fn turbulence_cast_precision_factor(self) -> f64 {
        1.0 - self.turbulence_intensity.clamp(0.0, 1.0)
            * (1.0 - VORTEX_TURBULENCE_CAST_PRECISION_MULTIPLIER)
    }

    pub fn turbulence_defense_drain_factor(self) -> f64 {
        1.0 + self.turbulence_intensity.clamp(0.0, 1.0) * VORTEX_TURBULENCE_DEFENSE_DRAIN_BONUS
    }

    pub fn law_disruption_backfire_fraction(self) -> f64 {
        self.law_disruption.clamp(0.0, 1.0) * 0.4
    }

    pub fn law_disruption_channeling_multiplier(self) -> f64 {
        1.0 + self.law_disruption.clamp(0.0, 1.0) * 2.0
    }

    pub fn law_disruption_distance_multiplier(self) -> f64 {
        1.0 + self.law_disruption.clamp(0.0, 1.0)
    }
}

impl Default for EnvField {
    fn default() -> Self {
        Self {
            local_zone_qi: 0.5,
            rhythm_multiplier: QI_RHYTHM_NEUTRAL,
            tsy_intensity: 0.0,
            ambient_pressure: 0.0,
            turbulence_intensity: 0.0,
            law_disruption: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ancient_relic_leaks_less_than_bone() {
        assert!(
            ContainerKind::SealedAncientRelic.seal_multiplier()
                < ContainerKind::SealedInBone.seal_multiplier()
        );
    }

    #[test]
    fn ambient_field_allows_reverse_pressure() {
        assert!(ContainerKind::AmbientField.allows_reverse_pressure());
        assert!(ContainerKind::TurbulentField.allows_reverse_pressure());
        assert!(!ContainerKind::SealedInBone.allows_reverse_pressure());
    }

    #[test]
    fn medium_loss_combines_color_and_carrier() {
        let bare = MediumKind::bare(ColorKind::Violent).loss_bonus_per_block();
        let relic = MediumKind {
            color: ColorKind::Violent,
            carrier: CarrierGrade::AncientRelic,
        }
        .loss_bonus_per_block();
        assert!(relic < bare);
    }

    #[test]
    fn env_clamps_local_zone_qi() {
        assert_eq!(EnvField::new(2.0).local_zone_qi, 1.0);
        assert_eq!(EnvField::new(-0.5).local_zone_qi, 0.0);
    }

    #[test]
    fn default_env_is_neutral_not_starved() {
        assert_eq!(EnvField::default().local_zone_qi, 0.5);
    }

    #[test]
    fn turbulence_field_applies_woliu_v2_multipliers() {
        let env = EnvField::default().with_turbulence(1.0);
        assert_eq!(env.turbulence_shelflife_factor(), 3.0);
        assert_eq!(env.turbulence_absorption_factor(), 0.0);
        assert_eq!(env.turbulence_cast_precision_factor(), 0.5);
        assert_eq!(env.turbulence_defense_drain_factor(), 1.2);
    }

    #[test]
    fn law_disruption_exposes_juebi_multipliers() {
        let env = EnvField::default().with_law_disruption(1.0);
        assert_eq!(env.law_disruption_backfire_fraction(), 0.4);
        assert_eq!(env.law_disruption_channeling_multiplier(), 3.0);
        assert_eq!(env.law_disruption_distance_multiplier(), 2.0);
    }
}
