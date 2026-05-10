//! plan-zone-weather-v1 P2 — cultivation style modifier for zone weather hits.

use super::components::{ColorKind, QiColor};
use crate::lingtian::weather::WeatherEvent;

pub const VIOLENT_LIGHTNING_MULTIPLIER: f32 = 0.7;
pub const METAL_ARMOR_LIGHTNING_MULTIPLIER: f32 = 1.5;

pub fn for_zone_weather(
    weather: WeatherEvent,
    qi_color: &QiColor,
    wearing_metal_armor: bool,
) -> f32 {
    if !matches!(weather, WeatherEvent::Thunderstorm) {
        return 1.0;
    }
    let mut multiplier = 1.0;
    if has_violent_qi(qi_color) {
        multiplier *= VIOLENT_LIGHTNING_MULTIPLIER;
    }
    if wearing_metal_armor {
        multiplier *= METAL_ARMOR_LIGHTNING_MULTIPLIER;
    }
    multiplier
}

fn has_violent_qi(qi_color: &QiColor) -> bool {
    qi_color.main == ColorKind::Violent || qi_color.secondary == Some(ColorKind::Violent)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn color(main: ColorKind, secondary: Option<ColorKind>) -> QiColor {
        QiColor {
            main,
            secondary,
            is_chaotic: false,
            is_hunyuan: false,
        }
    }

    #[test]
    fn style_modifier_lightning_strike_violent_dye_x07() {
        let multiplier = for_zone_weather(
            WeatherEvent::Thunderstorm,
            &color(ColorKind::Violent, None),
            false,
        );

        assert!((multiplier - 0.7).abs() < 1e-6);
    }

    #[test]
    fn style_modifier_lightning_strike_iron_armor_x15() {
        let multiplier = for_zone_weather(
            WeatherEvent::Thunderstorm,
            &color(ColorKind::Mellow, None),
            true,
        );

        assert!((multiplier - 1.5).abs() < 1e-6);
    }

    #[test]
    fn style_modifier_combines_violent_and_metal_armor() {
        let multiplier = for_zone_weather(
            WeatherEvent::Thunderstorm,
            &color(ColorKind::Mellow, Some(ColorKind::Violent)),
            true,
        );

        assert!((multiplier - 1.05).abs() < 1e-6);
    }

    #[test]
    fn style_modifier_ignores_non_lightning_weather() {
        let multiplier = for_zone_weather(
            WeatherEvent::LingMist,
            &color(ColorKind::Violent, None),
            true,
        );

        assert_eq!(multiplier, 1.0);
    }
}
