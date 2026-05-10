//! plan-zone-weather-v1 P1 — map WeatherEvent to EnvironmentEffect bundles.

use valence::prelude::{Res, ResMut};

use crate::lingtian::weather::{ActiveWeather, WeatherEvent};
use crate::lingtian::weather_profile::ZoneWeatherProfile;
use crate::lingtian::ZoneWeatherProfileRegistry;
use crate::world::environment::{EnvironmentEffect, ZoneEnvironmentRegistry};
use crate::world::zone::{Zone, ZoneRegistry};

/// Fixed visual bundle for a weather event in one zone.
pub fn weather_to_environment_bundle(
    event: WeatherEvent,
    zone: &Zone,
    profile: &ZoneWeatherProfile,
) -> Vec<EnvironmentEffect> {
    let (min, max) = aabb_arrays(zone);
    let center = center_array(zone);
    match event {
        WeatherEvent::Thunderstorm => vec![
            EnvironmentEffect::LightningPillar {
                center,
                radius: 10.0,
                strike_rate_per_min: profile.lightning_strike_per_min(),
            },
            EnvironmentEffect::EmberDrift {
                aabb_min: min,
                aabb_max: max,
                density: 0.3,
                glow: 0.5,
            },
            EnvironmentEffect::FogVeil {
                aabb_min: min,
                aabb_max: max,
                tint_rgb: [60, 60, 70],
                density: 0.4,
            },
        ],
        WeatherEvent::DroughtWind => vec![
            EnvironmentEffect::DustDevil {
                center,
                radius: 8.0,
                height: 30.0,
            },
            EnvironmentEffect::HeatHaze {
                aabb_min: min,
                aabb_max: max,
                distortion_strength: 0.4,
            },
            EnvironmentEffect::FogVeil {
                aabb_min: min,
                aabb_max: max,
                tint_rgb: [180, 150, 100],
                density: 0.2,
            },
        ],
        WeatherEvent::Blizzard => vec![
            EnvironmentEffect::SnowDrift {
                aabb_min: min,
                aabb_max: max,
                density: 0.8,
                wind_dir: [0.7, 0.0, -0.25],
            },
            EnvironmentEffect::FogVeil {
                aabb_min: min,
                aabb_max: max,
                tint_rgb: [200, 220, 230],
                density: 0.7,
            },
        ],
        WeatherEvent::HeavyHaze => vec![
            EnvironmentEffect::FogVeil {
                aabb_min: min,
                aabb_max: max,
                tint_rgb: [90, 90, 95],
                density: 0.85,
            },
            EnvironmentEffect::AshFall {
                aabb_min: min,
                aabb_max: max,
                density: 0.1,
            },
        ],
        WeatherEvent::LingMist => vec![EnvironmentEffect::FogVeil {
            aabb_min: min,
            aabb_max: max,
            tint_rgb: [180, 220, 230],
            density: 0.5,
        }],
    }
}

/// Runtime sync entrypoint: ZoneRegistry + ActiveWeather + profile registry → registry state.
pub fn weather_environment_sync_system(
    zones: Option<Res<ZoneRegistry>>,
    weather: Option<Res<ActiveWeather>>,
    profiles: Option<Res<ZoneWeatherProfileRegistry>>,
    registry: ResMut<ZoneEnvironmentRegistry>,
) {
    crate::world::environment::sync_zone_environment_effects(zones, weather, profiles, registry);
}

fn center_array(zone: &Zone) -> [f64; 3] {
    let center = zone.center();
    [center.x, center.y, center.z]
}

fn aabb_arrays(zone: &Zone) -> ([f64; 3], [f64; 3]) {
    let (min, max) = zone.bounds;
    ([min.x, min.y, min.z], [max.x, max.y, max.z])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use valence::prelude::DVec3;

    fn zone(name: &str) -> Zone {
        Zone {
            name: name.to_string(),
            dimension: crate::world::dimension::DimensionKind::Overworld,
            bounds: (DVec3::new(0.0, 60.0, 0.0), DVec3::new(100.0, 90.0, 100.0)),
            spirit_qi: 0.3,
            danger_level: 3,
            active_events: Vec::new(),
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
        }
    }

    fn kinds(effects: &[EnvironmentEffect]) -> HashSet<&'static str> {
        effects.iter().map(EnvironmentEffect::kind).collect()
    }

    #[test]
    fn weather_to_environment_thunderstorm_bundle() {
        let effects = weather_to_environment_bundle(
            WeatherEvent::Thunderstorm,
            &zone("scorch"),
            &ZoneWeatherProfile {
                lightning_strike_per_min_override: Some(3.0),
                ..Default::default()
            },
        );

        let kinds = kinds(&effects);
        assert!(kinds.contains("lightning_pillar"));
        assert!(kinds.contains("ember_drift"));
        assert!(kinds.contains("fog_veil"));
        assert!(effects.iter().any(|effect| matches!(
            effect,
            EnvironmentEffect::LightningPillar {
                strike_rate_per_min,
                ..
            } if (*strike_rate_per_min - 3.0).abs() < 1e-6
        )));
    }

    #[test]
    fn weather_to_environment_drought_wind_bundle() {
        let effects = weather_to_environment_bundle(
            WeatherEvent::DroughtWind,
            &zone("spawn"),
            &ZoneWeatherProfile::default(),
        );

        let kinds = kinds(&effects);
        assert!(kinds.contains("dust_devil"));
        assert!(kinds.contains("heat_haze"));
        assert!(kinds.contains("fog_veil"));
    }

    #[test]
    fn weather_to_environment_blizzard_bundle() {
        let effects = weather_to_environment_bundle(
            WeatherEvent::Blizzard,
            &zone("north_wastes"),
            &ZoneWeatherProfile::default(),
        );

        let kinds = kinds(&effects);
        assert!(kinds.contains("snow_drift"));
        assert!(kinds.contains("fog_veil"));
    }

    #[test]
    fn weather_to_environment_heavy_haze_bundle() {
        let effects = weather_to_environment_bundle(
            WeatherEvent::HeavyHaze,
            &zone("blood_valley"),
            &ZoneWeatherProfile::default(),
        );

        let kinds = kinds(&effects);
        assert!(kinds.contains("fog_veil"));
        assert!(kinds.contains("ash_fall"));
    }

    #[test]
    fn weather_to_environment_ling_mist_bundle() {
        let effects = weather_to_environment_bundle(
            WeatherEvent::LingMist,
            &zone("lingquan_marsh"),
            &ZoneWeatherProfile::default(),
        );

        assert_eq!(kinds(&effects), HashSet::from(["fog_veil"]));
    }
}
