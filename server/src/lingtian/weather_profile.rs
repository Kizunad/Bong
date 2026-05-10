//! plan-zone-weather-v1 P0 — zone-scoped weather profile.
//!
//! Profile 是 server-side world config：只影响本地天气生成概率和物理强度，
//! 不新增 Redis wire schema。

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Resource};

use super::weather::WeatherEvent;
use crate::world::season::Season;

pub const DEFAULT_WEATHER_PROFILES_PATH: &str = "weather_profiles.json";
pub const DEFAULT_LIGHTNING_STRIKE_PER_MIN: f32 = 1.0;
pub const DEFAULT_DUST_DEVIL_PUSH_STRENGTH: f32 = 0.5;
pub const DEFAULT_VISION_OBSCURE_RADIUS: f32 = 16.0;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ZoneWeatherProfile {
    pub thunderstorm_multiplier: Option<f32>,
    pub drought_wind_multiplier: Option<f32>,
    pub blizzard_multiplier: Option<f32>,
    pub heavy_haze_multiplier: Option<f32>,
    pub ling_mist_multiplier: Option<f32>,
    pub force_event: Option<WeatherEvent>,
    pub lightning_strike_per_min_override: Option<f32>,
    pub push_velocity_strength: Option<f32>,
    pub vision_obscure_radius: Option<f32>,
}

impl ZoneWeatherProfile {
    pub fn multiplier_for(&self, event: WeatherEvent) -> f32 {
        let value = match event {
            WeatherEvent::Thunderstorm => self.thunderstorm_multiplier,
            WeatherEvent::DroughtWind => self.drought_wind_multiplier,
            WeatherEvent::Blizzard => self.blizzard_multiplier,
            WeatherEvent::HeavyHaze => self.heavy_haze_multiplier,
            WeatherEvent::LingMist => self.ling_mist_multiplier,
        };
        sanitize_non_negative(value.unwrap_or(1.0))
    }

    pub fn effective_probability(&self, event: WeatherEvent, season: Season) -> f32 {
        (event.daily_probability(season) * self.multiplier_for(event)).clamp(0.0, 1.0)
    }

    pub fn lightning_strike_per_min(&self) -> f32 {
        sanitize_positive(
            self.lightning_strike_per_min_override
                .unwrap_or(DEFAULT_LIGHTNING_STRIKE_PER_MIN),
            DEFAULT_LIGHTNING_STRIKE_PER_MIN,
        )
    }

    pub fn dust_devil_push_strength(&self) -> f32 {
        sanitize_positive(
            self.push_velocity_strength
                .unwrap_or(DEFAULT_DUST_DEVIL_PUSH_STRENGTH),
            DEFAULT_DUST_DEVIL_PUSH_STRENGTH,
        )
    }

    pub fn vision_obscure_radius(&self) -> f32 {
        sanitize_positive(
            self.vision_obscure_radius
                .unwrap_or(DEFAULT_VISION_OBSCURE_RADIUS),
            DEFAULT_VISION_OBSCURE_RADIUS,
        )
    }

    fn validate(&self) -> Result<(), String> {
        for (field, value) in [
            ("thunderstorm_multiplier", self.thunderstorm_multiplier),
            ("drought_wind_multiplier", self.drought_wind_multiplier),
            ("blizzard_multiplier", self.blizzard_multiplier),
            ("heavy_haze_multiplier", self.heavy_haze_multiplier),
            ("ling_mist_multiplier", self.ling_mist_multiplier),
        ] {
            let Some(value) = value else {
                continue;
            };
            if !value.is_finite() || value < 0.0 {
                return Err(format!("{field} must be finite and >= 0"));
            }
        }
        for (field, value) in [
            (
                "lightning_strike_per_min_override",
                self.lightning_strike_per_min_override,
            ),
            ("push_velocity_strength", self.push_velocity_strength),
            ("vision_obscure_radius", self.vision_obscure_radius),
        ] {
            let Some(value) = value else {
                continue;
            };
            if !value.is_finite() || value <= 0.0 {
                return Err(format!("{field} must be finite and > 0"));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Resource)]
pub struct ZoneWeatherProfileRegistry {
    by_zone: HashMap<String, ZoneWeatherProfile>,
}

impl ZoneWeatherProfileRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_map(by_zone: HashMap<String, ZoneWeatherProfile>) -> Result<Self, String> {
        let mut registry = Self::new();
        for (zone, profile) in by_zone {
            registry.insert(zone, profile)?;
        }
        Ok(registry)
    }

    pub fn load_default() -> Result<Self, String> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_WEATHER_PROFILES_PATH);
        Self::load_from_path(path)
    }

    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(format!(
                    "weather profile file not found: {}",
                    path.display()
                ));
            }
            Err(error) => return Err(format!("failed to read {}: {error}", path.display())),
        };
        let parsed: ZoneWeatherProfilesFile = serde_json::from_str(text.as_str())
            .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
        Self::from_map(parsed.profiles)
    }

    pub fn insert(
        &mut self,
        zone: impl Into<String>,
        profile: ZoneWeatherProfile,
    ) -> Result<(), String> {
        let zone = normalize_zone(zone.into())?;
        profile
            .validate()
            .map_err(|reason| format!("invalid weather profile for zone `{zone}`: {reason}"))?;
        self.by_zone.insert(zone, profile);
        Ok(())
    }

    pub fn get(&self, zone: &str) -> Option<&ZoneWeatherProfile> {
        self.by_zone.get(zone.trim())
    }

    pub fn profile_for(&self, zone: &str) -> ZoneWeatherProfile {
        self.get(zone).cloned().unwrap_or_default()
    }

    pub fn len(&self) -> usize {
        self.by_zone.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_zone.is_empty()
    }
}

#[derive(Debug, Deserialize)]
struct ZoneWeatherProfilesFile {
    profiles: HashMap<String, ZoneWeatherProfile>,
}

fn normalize_zone(zone: String) -> Result<String, String> {
    let zone = zone.trim();
    if zone.is_empty() {
        Err("zone id cannot be empty".to_string())
    } else {
        Ok(zone.to_string())
    }
}

fn sanitize_non_negative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

fn sanitize_positive(value: f32, fallback: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_serde_round_trip() {
        let profile = ZoneWeatherProfile {
            thunderstorm_multiplier: Some(5.0),
            drought_wind_multiplier: Some(2.0),
            blizzard_multiplier: None,
            heavy_haze_multiplier: Some(0.5),
            ling_mist_multiplier: Some(0.0),
            force_event: Some(WeatherEvent::Thunderstorm),
            lightning_strike_per_min_override: Some(3.0),
            push_velocity_strength: Some(0.8),
            vision_obscure_radius: Some(12.0),
        };

        let json = serde_json::to_string(&profile).expect("serialize");
        let parsed: ZoneWeatherProfile = serde_json::from_str(json.as_str()).expect("deserialize");

        assert_eq!(parsed, profile);
    }

    #[test]
    fn multiplier_zero_means_never_rolls() {
        let profile = ZoneWeatherProfile {
            thunderstorm_multiplier: Some(0.0),
            ..Default::default()
        };

        assert_eq!(
            profile.effective_probability(WeatherEvent::Thunderstorm, Season::Summer),
            0.0
        );
    }

    #[test]
    fn multiplier_doubles_probability_within_rng_resolution() {
        let profile = ZoneWeatherProfile {
            thunderstorm_multiplier: Some(2.0),
            ..Default::default()
        };

        assert!(
            (profile.effective_probability(WeatherEvent::Thunderstorm, Season::Summer) - 0.06)
                .abs()
                < 1e-6
        );
    }

    #[test]
    fn default_profile_equivalent_to_unmodified_baseline() {
        let profile = ZoneWeatherProfile::default();
        for event in WeatherEvent::all() {
            assert!(
                (profile.effective_probability(event, Season::Summer)
                    - event.daily_probability(Season::Summer))
                .abs()
                    < 1e-6
            );
        }
    }

    #[test]
    fn registry_rejects_invalid_negative_values() {
        let mut registry = ZoneWeatherProfileRegistry::new();
        let result = registry.insert(
            "scorch",
            ZoneWeatherProfile {
                lightning_strike_per_min_override: Some(-1.0),
                ..Default::default()
            },
        );

        assert!(result.is_err());
    }

    #[test]
    fn registry_rejects_zero_positive_override_values() {
        for (field, profile) in [
            (
                "lightning_strike_per_min_override",
                ZoneWeatherProfile {
                    lightning_strike_per_min_override: Some(0.0),
                    ..Default::default()
                },
            ),
            (
                "push_velocity_strength",
                ZoneWeatherProfile {
                    push_velocity_strength: Some(0.0),
                    ..Default::default()
                },
            ),
            (
                "vision_obscure_radius",
                ZoneWeatherProfile {
                    vision_obscure_radius: Some(0.0),
                    ..Default::default()
                },
            ),
        ] {
            let mut registry = ZoneWeatherProfileRegistry::new();
            let err = registry
                .insert("scorch", profile)
                .expect_err("zero override should be rejected");
            assert!(
                err.contains(field),
                "error should name invalid field `{field}`: {err}"
            );
        }
    }

    #[test]
    fn registry_returns_default_for_unknown_zone() {
        let registry = ZoneWeatherProfileRegistry::new();

        assert_eq!(
            registry.profile_for("missing"),
            ZoneWeatherProfile::default()
        );
    }

    #[test]
    fn registry_load_missing_profile_file_errors() {
        let path = std::env::temp_dir().join(format!(
            "bong-missing-weather-profiles-{}-{}.json",
            std::process::id(),
            line!()
        ));

        let err = ZoneWeatherProfileRegistry::load_from_path(&path)
            .expect_err("missing profile file should surface config error");

        assert!(err.contains("weather profile file not found"));
        assert!(err.contains(path.to_string_lossy().as_ref()));
    }

    #[test]
    fn default_profile_file_loads_scorch_overrides() {
        let registry =
            ZoneWeatherProfileRegistry::load_default().expect("default profile file should load");

        for zone in [
            "blood_valley_east_scorch",
            "north_waste_east_scorch",
            "drift_scorch_001",
        ] {
            let profile = registry.get(zone).expect("scorch profile should exist");
            assert_eq!(profile.thunderstorm_multiplier, Some(5.0));
            assert_eq!(profile.drought_wind_multiplier, Some(2.0));
            assert!(
                profile.lightning_strike_per_min() >= 1.0,
                "scorch lightning rate must stay active"
            );
        }
    }
}
