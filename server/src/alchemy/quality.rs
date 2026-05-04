//! plan-alchemy-v2 P2 — 丹药品阶与开光的纯函数。

use crate::cultivation::components::Realm;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PillQualityProfile {
    pub quality_tier: u8,
    pub effect_multiplier: f64,
    pub consecrated: bool,
}

pub fn quality_tier_from_quality(quality: f64) -> u8 {
    let q = if quality.is_finite() { quality } else { 0.0 }.clamp(0.0, 1.0);
    if q < 0.30 {
        1
    } else if q < 0.50 {
        2
    } else if q < 0.70 {
        3
    } else if q < 0.90 {
        4
    } else {
        5
    }
}

pub fn effect_multiplier_for_tier(tier: u8) -> f64 {
    match tier.clamp(1, 5) {
        1 => 0.70,
        2 => 0.85,
        3 => 1.00,
        4 => 1.20,
        5 => 1.50,
        _ => unreachable!(),
    }
}

pub fn profile_for_pill_quality(quality: f64, flawed_path: bool) -> PillQualityProfile {
    let tier = quality_tier_from_quality(quality);
    let quality_tier = if flawed_path { tier.min(3) } else { tier };
    PillQualityProfile {
        quality_tier,
        effect_multiplier: effect_multiplier_for_tier(quality_tier),
        consecrated: false,
    }
}

pub fn consecrate_if_allowed(
    profile: PillQualityProfile,
    officiant_realm: Realm,
) -> PillQualityProfile {
    if profile.quality_tier == 5 && officiant_realm == Realm::Void {
        return PillQualityProfile {
            consecrated: true,
            effect_multiplier: profile.effect_multiplier * 2.0,
            ..profile
        };
    }
    profile
}

pub fn auto_profile_quality_cap(best_manual_quality: f64) -> f64 {
    if !best_manual_quality.is_finite() || best_manual_quality <= 0.0 {
        return 0.0;
    }
    (best_manual_quality * 0.85).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quality_tier_maps_quality_into_five_bands() {
        assert_eq!(quality_tier_from_quality(0.0), 1);
        assert_eq!(quality_tier_from_quality(0.30), 2);
        assert_eq!(quality_tier_from_quality(0.50), 3);
        assert_eq!(quality_tier_from_quality(0.70), 4);
        assert_eq!(quality_tier_from_quality(0.90), 5);
    }

    #[test]
    fn flawed_path_caps_quality_tier_to_three() {
        let profile = profile_for_pill_quality(0.95, true);

        assert_eq!(profile.quality_tier, 3);
        assert_eq!(profile.effect_multiplier, 1.0);
    }

    #[test]
    fn void_realm_can_consecrate_tier_five_pill() {
        let base = profile_for_pill_quality(0.95, false);
        let consecrated = consecrate_if_allowed(base, Realm::Void);

        assert!(consecrated.consecrated);
        assert_eq!(consecrated.quality_tier, 5);
        assert!((consecrated.effect_multiplier - 3.0).abs() < 1e-9);
    }

    #[test]
    fn auto_profile_quality_cap_is_eighty_five_percent_of_manual_best() {
        assert!((auto_profile_quality_cap(0.8) - 0.68).abs() < 1e-9);
        assert_eq!(auto_profile_quality_cap(9.0), 1.0);
    }
}
