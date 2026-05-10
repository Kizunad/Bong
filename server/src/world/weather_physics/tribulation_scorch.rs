//! 烬焰焦土雷雨物理的薄编排层。
//!
//! 这里只组合已落地的 `ZoneWeatherProfile::lightning_strike_per_min` 和
//! `cultivation::style_modifier::for_zone_weather`，不新增平行天气体系。

use crate::cultivation::components::QiColor;
use crate::cultivation::style_modifier;
use crate::lingtian::weather::WeatherEvent;
use crate::lingtian::weather_profile::ZoneWeatherProfile;

pub const TRIBULATION_SCORCH_QI_LEAK_MULTIPLIER: f32 = 1.3;

pub fn adjusted_lightning_rate_per_min(
    profile: &ZoneWeatherProfile,
    weather: WeatherEvent,
    qi_color: &QiColor,
    wearing_metal_armor: bool,
) -> f32 {
    if !matches!(weather, WeatherEvent::Thunderstorm) {
        return 0.0;
    }
    profile.lightning_strike_per_min()
        * style_modifier::for_zone_weather(weather, qi_color, wearing_metal_armor)
}

pub fn expected_lightning_strikes_in_window(
    profile: &ZoneWeatherProfile,
    weather: WeatherEvent,
    qi_color: &QiColor,
    wearing_metal_armor: bool,
    minutes: f32,
) -> f32 {
    if !minutes.is_finite() || minutes <= 0.0 {
        return 0.0;
    }
    adjusted_lightning_rate_per_min(profile, weather, qi_color, wearing_metal_armor) * minutes
}

pub fn qi_leak_multiplier_in_scorch_zone() -> f32 {
    TRIBULATION_SCORCH_QI_LEAK_MULTIPLIER
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::{ColorKind, QiColor};

    fn color(main: ColorKind) -> QiColor {
        QiColor {
            main,
            secondary: None,
            is_chaotic: false,
            is_hunyuan: false,
            ..Default::default()
        }
    }

    fn profile(rate: f32) -> ZoneWeatherProfile {
        ZoneWeatherProfile {
            lightning_strike_per_min_override: Some(rate),
            ..Default::default()
        }
    }

    #[test]
    fn scorch_thunderstorm_hits_five_minute_window_without_new_weather_stack() {
        let strikes = expected_lightning_strikes_in_window(
            &profile(2.0),
            WeatherEvent::Thunderstorm,
            &color(ColorKind::Mellow),
            false,
            5.0,
        );

        assert!((5.0..=15.0).contains(&strikes));
        assert!((strikes - 10.0).abs() < 1e-6);
    }

    #[test]
    fn violent_qi_reuses_style_modifier_for_x07_lightning_rate() {
        let base = adjusted_lightning_rate_per_min(
            &profile(3.0),
            WeatherEvent::Thunderstorm,
            &color(ColorKind::Mellow),
            false,
        );
        let violent = adjusted_lightning_rate_per_min(
            &profile(3.0),
            WeatherEvent::Thunderstorm,
            &color(ColorKind::Violent),
            false,
        );

        assert!((violent / base - 0.7).abs() < 1e-6);
    }

    #[test]
    fn metal_armor_reuses_style_modifier_for_x15_lightning_rate() {
        let base = adjusted_lightning_rate_per_min(
            &profile(3.0),
            WeatherEvent::Thunderstorm,
            &color(ColorKind::Mellow),
            false,
        );
        let metal = adjusted_lightning_rate_per_min(
            &profile(3.0),
            WeatherEvent::Thunderstorm,
            &color(ColorKind::Mellow),
            true,
        );

        assert!((metal / base - 1.5).abs() < 1e-6);
    }

    #[test]
    fn scorch_qi_leak_multiplier_applies_to_all_colors() {
        assert!((qi_leak_multiplier_in_scorch_zone() - 1.3).abs() < 1e-6);
        assert!((qi_leak_multiplier_in_scorch_zone() - 1.3).abs() < 1e-6);
    }

    #[test]
    fn non_thunderstorm_weather_does_not_spawn_scorch_lightning() {
        let strikes = expected_lightning_strikes_in_window(
            &profile(3.0),
            WeatherEvent::LingMist,
            &color(ColorKind::Violent),
            true,
            5.0,
        );

        assert_eq!(strikes, 0.0);
    }
}
