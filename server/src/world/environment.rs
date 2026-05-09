use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use valence::prelude::{
    bevy_ecs, App, Event, EventWriter, IntoSystemConfigs, Resource, Startup, Update,
};

use crate::lingtian::weather::{ActiveWeather, WeatherEvent};
use crate::world::zone::{Zone, ZoneRegistry};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EnvironmentEffect {
    TornadoColumn {
        center: [f64; 3],
        radius: f64,
        height: f64,
        particle_density: f32,
    },
    LightningPillar {
        center: [f64; 3],
        radius: f64,
        strike_rate_per_min: f32,
    },
    AshFall {
        aabb_min: [f64; 3],
        aabb_max: [f64; 3],
        density: f32,
    },
    FogVeil {
        aabb_min: [f64; 3],
        aabb_max: [f64; 3],
        tint_rgb: [u8; 3],
        density: f32,
    },
    DustDevil {
        center: [f64; 3],
        radius: f64,
        height: f64,
    },
    EmberDrift {
        aabb_min: [f64; 3],
        aabb_max: [f64; 3],
        density: f32,
        glow: f32,
    },
    HeatHaze {
        aabb_min: [f64; 3],
        aabb_max: [f64; 3],
        distortion_strength: f32,
    },
    SnowDrift {
        aabb_min: [f64; 3],
        aabb_max: [f64; 3],
        density: f32,
        wind_dir: [f32; 3],
    },
}

impl EnvironmentEffect {
    #[cfg(test)]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::TornadoColumn { .. } => "tornado_column",
            Self::LightningPillar { .. } => "lightning_pillar",
            Self::AshFall { .. } => "ash_fall",
            Self::FogVeil { .. } => "fog_veil",
            Self::DustDevil { .. } => "dust_devil",
            Self::EmberDrift { .. } => "ember_drift",
            Self::HeatHaze { .. } => "heat_haze",
            Self::SnowDrift { .. } => "snow_drift",
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Event)]
pub enum ZoneEnvironmentLifecycleEvent {
    EffectAdded { zone: String, index: usize },
    EffectRemoved { zone: String, index: usize },
    Replaced { zone: String },
}

#[derive(Debug, Clone, Default, Resource)]
pub struct ZoneEnvironmentRegistry {
    by_zone: HashMap<String, Vec<EnvironmentEffect>>,
    generation_by_zone: HashMap<String, u64>,
    dirty: HashSet<String>,
    lifecycle: Vec<ZoneEnvironmentLifecycleEvent>,
}

impl ZoneEnvironmentRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(dead_code)]
    pub fn add(&mut self, zone: impl Into<String>, effect: EnvironmentEffect) {
        let zone = normalize_zone(zone.into());
        let entry = self.by_zone.entry(zone.clone()).or_default();
        let index = entry.len();
        entry.push(effect);
        self.lifecycle
            .push(ZoneEnvironmentLifecycleEvent::EffectAdded {
                zone: zone.clone(),
                index,
            });
        self.mark_dirty(zone);
    }

    #[allow(dead_code)]
    pub fn remove(
        &mut self,
        zone: &str,
        effect_match: impl Fn(&EnvironmentEffect) -> bool,
    ) -> usize {
        let zone = normalize_zone(zone);
        let removed_indices = {
            let Some(entry) = self.by_zone.get_mut(&zone) else {
                return 0;
            };
            let mut removed_indices = Vec::new();
            let mut kept = Vec::with_capacity(entry.len());
            for (index, effect) in entry.drain(..).enumerate() {
                if effect_match(&effect) {
                    removed_indices.push(index);
                } else {
                    kept.push(effect);
                }
            }
            *entry = kept;
            removed_indices
        };
        let removed = removed_indices.len();
        if removed > 0 {
            for index in removed_indices {
                self.lifecycle
                    .push(ZoneEnvironmentLifecycleEvent::EffectRemoved {
                        zone: zone.clone(),
                        index,
                    });
            }
            self.mark_dirty(zone);
        }
        removed
    }

    pub fn replace(&mut self, zone: impl Into<String>, effects: Vec<EnvironmentEffect>) {
        let zone = normalize_zone(zone.into());
        if self.by_zone.get(&zone) == Some(&effects) {
            return;
        }
        self.by_zone.insert(zone.clone(), effects);
        self.lifecycle
            .push(ZoneEnvironmentLifecycleEvent::Replaced { zone: zone.clone() });
        self.mark_dirty(zone);
    }

    pub fn current(&self, zone: &str) -> &[EnvironmentEffect] {
        let key = normalize_zone(zone);
        self.by_zone
            .get(key.as_str())
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn generation(&self, zone: &str) -> u64 {
        let key = normalize_zone(zone);
        self.generation_by_zone
            .get(key.as_str())
            .copied()
            .unwrap_or_default()
    }

    pub fn drain_dirty(&mut self) -> Vec<String> {
        let mut dirty: Vec<String> = self.dirty.drain().collect();
        dirty.sort();
        dirty
    }

    pub fn drain_lifecycle(&mut self) -> Vec<ZoneEnvironmentLifecycleEvent> {
        self.lifecycle.drain(..).collect()
    }

    fn mark_dirty(&mut self, zone: String) {
        let generation = self.generation_by_zone.entry(zone.clone()).or_default();
        *generation = generation.saturating_add(1).max(1);
        self.dirty.insert(zone);
    }
}

#[allow(dead_code)]
pub trait EnvironmentPhysicsHook: Send + Sync {
    fn on_effect_active(&self, effect: &EnvironmentEffect, world: &mut bevy_ecs::world::World);
}

pub fn register(app: &mut App) {
    app.insert_resource(ZoneEnvironmentRegistry::new());
    app.add_event::<ZoneEnvironmentLifecycleEvent>();
    app.add_systems(Startup, sync_zone_environment_effects);
    app.add_systems(
        Update,
        (
            sync_zone_environment_effects,
            publish_zone_environment_lifecycle_events.after(sync_zone_environment_effects),
        ),
    );
}

pub fn publish_zone_environment_lifecycle_events(
    mut registry: valence::prelude::ResMut<ZoneEnvironmentRegistry>,
    mut events: EventWriter<ZoneEnvironmentLifecycleEvent>,
) {
    for event in registry.drain_lifecycle() {
        events.send(event);
    }
}

pub fn sync_zone_environment_effects(
    zones: Option<valence::prelude::Res<ZoneRegistry>>,
    weather: Option<valence::prelude::Res<ActiveWeather>>,
    mut registry: valence::prelude::ResMut<ZoneEnvironmentRegistry>,
) {
    let Some(zones) = zones else {
        return;
    };
    for zone in &zones.zones {
        let active_weather = weather
            .as_ref()
            .and_then(|active| active.current(zone.name.as_str()));
        let effects = default_effects_for_zone(zone, active_weather);
        registry.replace(zone.name.clone(), effects);
    }
}

pub fn default_effects_for_zone(
    zone: &Zone,
    active_weather: Option<WeatherEvent>,
) -> Vec<EnvironmentEffect> {
    let mut effects = Vec::new();

    if is_scorch_zone(zone) {
        effects.extend(scorch_zone_effects(zone));
    }
    if is_tribulation_zone(zone) {
        effects.extend(tribulation_zone_effects(zone));
    }
    if zone.is_tsy() {
        effects.extend(tsy_zone_effects(zone));
    }
    if let Some(weather) = active_weather {
        effects.extend(weather_environment_effects(zone, weather));
    }

    effects
}

pub fn scorch_zone_effects(zone: &Zone) -> Vec<EnvironmentEffect> {
    let (min, max) = aabb_arrays(zone);
    let center = center_array(zone);
    vec![
        EnvironmentEffect::AshFall {
            aabb_min: min,
            aabb_max: max,
            density: 0.55,
        },
        EnvironmentEffect::EmberDrift {
            aabb_min: min,
            aabb_max: max,
            density: 0.28,
            glow: 0.65,
        },
        EnvironmentEffect::FogVeil {
            aabb_min: min,
            aabb_max: max,
            tint_rgb: [86, 38, 34],
            density: 0.34,
        },
        EnvironmentEffect::LightningPillar {
            center,
            radius: 18.0,
            strike_rate_per_min: 1.4,
        },
    ]
}

pub fn tribulation_zone_effects(zone: &Zone) -> Vec<EnvironmentEffect> {
    let (min, max) = aabb_arrays(zone);
    vec![
        EnvironmentEffect::LightningPillar {
            center: center_array(zone),
            radius: 12.0,
            strike_rate_per_min: 2.4,
        },
        EnvironmentEffect::FogVeil {
            aabb_min: min,
            aabb_max: max,
            tint_rgb: [91, 52, 132],
            density: 0.42,
        },
    ]
}

pub fn tsy_zone_effects(zone: &Zone) -> Vec<EnvironmentEffect> {
    let (min, max) = aabb_arrays(zone);
    vec![
        EnvironmentEffect::FogVeil {
            aabb_min: min,
            aabb_max: max,
            tint_rgb: [42, 43, 48],
            density: 0.58,
        },
        EnvironmentEffect::AshFall {
            aabb_min: min,
            aabb_max: max,
            density: 0.16,
        },
    ]
}

fn weather_environment_effects(zone: &Zone, weather: WeatherEvent) -> Vec<EnvironmentEffect> {
    let (min, max) = aabb_arrays(zone);
    let center = center_array(zone);
    match weather {
        WeatherEvent::Thunderstorm => vec![EnvironmentEffect::LightningPillar {
            center,
            radius: 10.0,
            strike_rate_per_min: 1.8,
        }],
        WeatherEvent::DroughtWind => vec![
            EnvironmentEffect::DustDevil {
                center,
                radius: 8.0,
                height: 32.0,
            },
            EnvironmentEffect::HeatHaze {
                aabb_min: min,
                aabb_max: max,
                distortion_strength: 0.35,
            },
        ],
        WeatherEvent::Blizzard => vec![EnvironmentEffect::SnowDrift {
            aabb_min: min,
            aabb_max: max,
            density: 0.5,
            wind_dir: [0.7, 0.0, -0.25],
        }],
        WeatherEvent::HeavyHaze => vec![EnvironmentEffect::FogVeil {
            aabb_min: min,
            aabb_max: max,
            tint_rgb: [74, 77, 82],
            density: 0.5,
        }],
        WeatherEvent::LingMist => vec![EnvironmentEffect::FogVeil {
            aabb_min: min,
            aabb_max: max,
            tint_rgb: [164, 207, 194],
            density: 0.36,
        }],
    }
}

fn is_scorch_zone(zone: &Zone) -> bool {
    zone.name.contains("scorch")
        || zone
            .active_events
            .iter()
            .any(|event| event == "tribulation_scorch" || event == "ash_fall")
}

fn is_tribulation_zone(zone: &Zone) -> bool {
    zone.name.contains("tribulation")
        || zone
            .active_events
            .iter()
            .any(|event| event.contains("tribulation") || event == "tianjie")
}

fn center_array(zone: &Zone) -> [f64; 3] {
    let center = zone.center();
    [center.x, center.y, center.z]
}

fn aabb_arrays(zone: &Zone) -> ([f64; 3], [f64; 3]) {
    let (min, max) = zone.bounds;
    ([min.x, min.y, min.z], [max.x, max.y, max.z])
}

fn normalize_zone(zone: impl AsRef<str>) -> String {
    let normalized = zone.as_ref().trim();
    if normalized.is_empty() {
        "default".to_string()
    } else {
        normalized.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::{App, DVec3};

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

    fn all_effects() -> Vec<EnvironmentEffect> {
        vec![
            EnvironmentEffect::TornadoColumn {
                center: [1.0, 64.0, 2.0],
                radius: 8.0,
                height: 60.0,
                particle_density: 0.5,
            },
            EnvironmentEffect::LightningPillar {
                center: [1.0, 64.0, 2.0],
                radius: 3.0,
                strike_rate_per_min: 2.0,
            },
            EnvironmentEffect::AshFall {
                aabb_min: [0.0, 60.0, 0.0],
                aabb_max: [10.0, 90.0, 10.0],
                density: 0.4,
            },
            EnvironmentEffect::FogVeil {
                aabb_min: [0.0, 60.0, 0.0],
                aabb_max: [10.0, 90.0, 10.0],
                tint_rgb: [120, 130, 140],
                density: 0.3,
            },
            EnvironmentEffect::DustDevil {
                center: [1.0, 64.0, 2.0],
                radius: 4.0,
                height: 20.0,
            },
            EnvironmentEffect::EmberDrift {
                aabb_min: [0.0, 60.0, 0.0],
                aabb_max: [10.0, 90.0, 10.0],
                density: 0.4,
                glow: 0.6,
            },
            EnvironmentEffect::HeatHaze {
                aabb_min: [0.0, 60.0, 0.0],
                aabb_max: [10.0, 90.0, 10.0],
                distortion_strength: 0.2,
            },
            EnvironmentEffect::SnowDrift {
                aabb_min: [0.0, 60.0, 0.0],
                aabb_max: [10.0, 90.0, 10.0],
                density: 0.4,
                wind_dir: [1.0, 0.0, 0.0],
            },
        ]
    }

    #[test]
    fn effect_serde_round_trip_each_variant() {
        for effect in all_effects() {
            let json = serde_json::to_string(&effect).expect("serialize");
            let parsed: EnvironmentEffect = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(parsed, effect);
        }
    }

    #[test]
    fn effect_kind_wire_names_match_schema() {
        let kinds: Vec<&str> = all_effects().iter().map(EnvironmentEffect::kind).collect();
        assert_eq!(
            kinds,
            vec![
                "tornado_column",
                "lightning_pillar",
                "ash_fall",
                "fog_veil",
                "dust_devil",
                "ember_drift",
                "heat_haze",
                "snow_drift"
            ]
        );
    }

    #[test]
    fn registry_add_then_current() {
        let mut registry = ZoneEnvironmentRegistry::new();
        registry.add("spawn", all_effects()[0].clone());
        assert_eq!(registry.current("spawn"), &[all_effects()[0].clone()]);
        assert_eq!(registry.generation("spawn"), 1);
    }

    #[test]
    fn registry_remove_by_match_predicate() {
        let mut registry = ZoneEnvironmentRegistry::new();
        registry.replace("spawn", all_effects());
        let removed = registry.remove("spawn", |effect| effect.kind() == "fog_veil");
        assert_eq!(removed, 1);
        assert!(!registry
            .current("spawn")
            .iter()
            .any(|effect| effect.kind() == "fog_veil"));
    }

    #[test]
    fn registry_replace_overrides_existing() {
        let mut registry = ZoneEnvironmentRegistry::new();
        registry.add("spawn", all_effects()[0].clone());
        registry.replace("spawn", vec![all_effects()[3].clone()]);
        assert_eq!(registry.current("spawn"), &[all_effects()[3].clone()]);
    }

    #[test]
    fn registry_dirty_drain_idempotent() {
        let mut registry = ZoneEnvironmentRegistry::new();
        registry.add("spawn", all_effects()[0].clone());
        registry.add("spawn", all_effects()[1].clone());
        assert_eq!(registry.drain_dirty(), vec!["spawn".to_string()]);
        assert!(registry.drain_dirty().is_empty());
    }

    #[test]
    fn registry_cross_zone_isolation() {
        let mut registry = ZoneEnvironmentRegistry::new();
        registry.add("spawn", all_effects()[0].clone());
        registry.add("blood_valley", all_effects()[1].clone());
        assert_eq!(registry.current("spawn").len(), 1);
        assert_eq!(registry.current("blood_valley").len(), 1);
        assert_ne!(
            registry.current("spawn")[0],
            registry.current("blood_valley")[0]
        );
    }

    #[test]
    fn replace_same_effects_does_not_bump_generation() {
        let mut registry = ZoneEnvironmentRegistry::new();
        let effects = vec![all_effects()[0].clone()];
        registry.replace("spawn", effects.clone());
        registry.drain_dirty();
        registry.replace("spawn", effects);
        assert_eq!(registry.generation("spawn"), 1);
        assert!(registry.drain_dirty().is_empty());
    }

    #[test]
    fn lifecycle_event_is_registered_in_app() {
        let mut app = App::new();
        app.add_event::<ZoneEnvironmentLifecycleEvent>();
        app.world_mut()
            .send_event(ZoneEnvironmentLifecycleEvent::Replaced {
                zone: "spawn".to_string(),
            });
        app.update();
        assert!(
            app.world()
                .get_resource::<valence::prelude::Events<ZoneEnvironmentLifecycleEvent>>()
                .is_some(),
            "Bevy event resource should exist for lifecycle subscribers"
        );
    }

    #[test]
    fn lifecycle_event_added_removed_pair() {
        let mut registry = ZoneEnvironmentRegistry::new();
        registry.add("spawn", all_effects()[0].clone());
        assert_eq!(
            registry.drain_lifecycle(),
            vec![ZoneEnvironmentLifecycleEvent::EffectAdded {
                zone: "spawn".to_string(),
                index: 0,
            }]
        );

        registry.remove("spawn", |effect| effect.kind() == "tornado_column");
        assert_eq!(
            registry.drain_lifecycle(),
            vec![ZoneEnvironmentLifecycleEvent::EffectRemoved {
                zone: "spawn".to_string(),
                index: 0,
            }]
        );
    }

    #[test]
    fn scorch_zone_seed_contains_ash_ember_fog_and_lightning() {
        let effects = default_effects_for_zone(&zone("blood_valley_east_scorch"), None);
        let kinds: HashSet<&str> = effects.iter().map(EnvironmentEffect::kind).collect();
        assert!(kinds.contains("ash_fall"));
        assert!(kinds.contains("ember_drift"));
        assert!(kinds.contains("fog_veil"));
        assert!(kinds.contains("lightning_pillar"));
    }

    #[test]
    fn tsy_zone_seed_contains_dead_silence_fog() {
        let effects = default_effects_for_zone(&zone("tsy_lingxu_01_shallow"), None);
        assert!(effects.iter().any(|effect| effect.kind() == "fog_veil"));
    }

    #[test]
    fn weather_thunderstorm_adds_lightning_pillar() {
        let effects = default_effects_for_zone(&zone("spawn"), Some(WeatherEvent::Thunderstorm));
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0].kind(), "lightning_pillar");
    }

    #[test]
    fn weather_blizzard_adds_snow_drift() {
        let effects = default_effects_for_zone(&zone("spawn"), Some(WeatherEvent::Blizzard));
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0].kind(), "snow_drift");
    }

    #[test]
    fn physics_hook_trait_is_object_safe() {
        struct NoopHook;
        impl EnvironmentPhysicsHook for NoopHook {
            fn on_effect_active(
                &self,
                _effect: &EnvironmentEffect,
                _world: &mut bevy_ecs::world::World,
            ) {
            }
        }
        let _hook: Box<dyn EnvironmentPhysicsHook> = Box::new(NoopHook);
    }
}
