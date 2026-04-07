use std::collections::HashSet;
use std::fs;
use std::path::Path;

use serde::Deserialize;
use valence::prelude::{App, Commands, DVec3, Resource, Startup};

use super::TEST_AREA_BLOCK_EXTENT;

pub const DEFAULT_ZONES_PATH: &str = "zones.json";
pub const DEFAULT_SPAWN_ZONE_NAME: &str = "spawn";

const DEFAULT_SPAWN_BOUNDS_MIN: [f64; 3] = [0.0, 64.0, 0.0];
const DEFAULT_SPAWN_BOUNDS_MAX_Y: f64 = 80.0;
const DEFAULT_SPAWN_SPIRIT_QI: f64 = 0.9;
const DEFAULT_SPAWN_PATROL_ANCHORS: [[f64; 3]; 1] = [[14.0, 66.0, 14.0]];
const MAX_ZONE_DANGER_LEVEL: u8 = 5;

#[derive(Clone, Debug, PartialEq)]
pub struct Zone {
    pub name: String,
    pub bounds: (DVec3, DVec3),
    pub spirit_qi: f64,
    pub danger_level: u8,
    pub active_events: Vec<String>,
    pub patrol_anchors: Vec<DVec3>,
}

impl Zone {
    fn spawn() -> Self {
        Self {
            name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            bounds: default_spawn_bounds(),
            spirit_qi: DEFAULT_SPAWN_SPIRIT_QI,
            danger_level: 0,
            active_events: Vec::new(),
            patrol_anchors: DEFAULT_SPAWN_PATROL_ANCHORS
                .into_iter()
                .map(dvec3_from_array)
                .collect(),
        }
    }

    pub fn contains(&self, pos: DVec3) -> bool {
        let (min, max) = self.bounds;

        pos.x >= min.x
            && pos.x <= max.x
            && pos.y >= min.y
            && pos.y <= max.y
            && pos.z >= min.z
            && pos.z <= max.z
    }

    pub fn clamp_position(&self, pos: DVec3) -> DVec3 {
        let (min, max) = self.bounds;

        DVec3::new(
            pos.x.clamp(min.x, max.x),
            pos.y.clamp(min.y, max.y),
            pos.z.clamp(min.z, max.z),
        )
    }

    pub fn center(&self) -> DVec3 {
        let (min, max) = self.bounds;
        DVec3::new(
            (min.x + max.x) * 0.5,
            (min.y + max.y) * 0.5,
            (min.z + max.z) * 0.5,
        )
    }

    pub fn patrol_target(&self, anchor_index: usize) -> DVec3 {
        if self.patrol_anchors.is_empty() {
            self.center()
        } else {
            self.patrol_anchors[anchor_index % self.patrol_anchors.len()]
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ZoneRegistry {
    pub zones: Vec<Zone>,
}

impl Resource for ZoneRegistry {}

impl Default for ZoneRegistry {
    fn default() -> Self {
        Self::fallback()
    }
}

impl ZoneRegistry {
    pub fn fallback() -> Self {
        Self {
            zones: vec![Zone::spawn()],
        }
    }

    pub fn load() -> Self {
        Self::load_from_path(DEFAULT_ZONES_PATH)
    }

    pub fn load_from_path(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();

        let contents = match fs::read_to_string(path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                tracing::info!(
                    "[bong][world] no zones config at {}, using fallback spawn zone",
                    path.display()
                );
                return Self::fallback();
            }
            Err(error) => {
                tracing::warn!(
                    "[bong][world] failed to read {} as zones config, using fallback spawn zone: {error}",
                    path.display()
                );
                return Self::fallback();
            }
        };

        let registry = match serde_json::from_str::<ZonesFileConfig>(&contents) {
            Ok(config) => match ZoneRegistry::try_from(config) {
                Ok(registry) => registry,
                Err(error) => {
                    tracing::warn!(
                        "[bong][world] invalid zones config at {}, using fallback spawn zone: {error}",
                        path.display()
                    );
                    return Self::fallback();
                }
            },
            Err(error) => {
                tracing::warn!(
                    "[bong][world] failed to parse {} as zones config, using fallback spawn zone: {error}",
                    path.display()
                );
                return Self::fallback();
            }
        };

        tracing::info!(
            "[bong][world] loaded {} authoritative zone(s) from {}",
            registry.zones.len(),
            path.display()
        );

        registry
    }

    pub fn find_zone_by_name(&self, name: &str) -> Option<&Zone> {
        self.zones.iter().find(|zone| zone.name == name)
    }

    pub fn find_zone(&self, pos: DVec3) -> Option<&Zone> {
        self.zones.iter().find(|zone| zone.contains(pos))
    }

    pub fn find_zone_mut(&mut self, name: &str) -> Option<&mut Zone> {
        self.zones.iter_mut().find(|zone| zone.name == name)
    }
}

#[derive(Debug, Deserialize)]
struct ZonesFileConfig {
    zones: Vec<ZoneConfig>,
}

#[derive(Debug, Deserialize)]
struct ZoneConfig {
    name: String,
    aabb: ZoneAabbConfig,
    spirit_qi: f64,
    danger_level: u8,
    #[serde(default)]
    active_events: Vec<String>,
    #[serde(default)]
    patrol_anchors: Vec<[f64; 3]>,
}

#[derive(Debug, Deserialize)]
struct ZoneAabbConfig {
    min: [f64; 3],
    max: [f64; 3],
}

impl TryFrom<ZonesFileConfig> for ZoneRegistry {
    type Error = String;

    fn try_from(config: ZonesFileConfig) -> Result<Self, Self::Error> {
        if config.zones.is_empty() {
            return Err("zones list cannot be empty".to_string());
        }

        let mut seen_names = HashSet::new();
        let mut saw_spawn = false;
        let mut zones = Vec::with_capacity(config.zones.len());

        for zone_config in config.zones {
            let zone = validate_zone(zone_config, &mut seen_names)?;
            if zone.name == DEFAULT_SPAWN_ZONE_NAME {
                saw_spawn = true;
            }
            zones.push(zone);
        }

        if !saw_spawn {
            return Err(format!(
                "zones config must include a `{DEFAULT_SPAWN_ZONE_NAME}` zone to preserve spawn fallback semantics"
            ));
        }

        Ok(Self { zones })
    }
}

fn validate_zone(zone: ZoneConfig, seen_names: &mut HashSet<String>) -> Result<Zone, String> {
    let name = zone.name.trim();
    if name.is_empty() {
        return Err("zone name cannot be empty".to_string());
    }

    if !seen_names.insert(name.to_string()) {
        return Err(format!("duplicate zone name `{name}`"));
    }

    if !zone.spirit_qi.is_finite() || !(0.0..=1.0).contains(&zone.spirit_qi) {
        return Err(format!(
            "zone `{name}` spirit_qi must be a finite value within [0.0, 1.0]"
        ));
    }

    if zone.danger_level > MAX_ZONE_DANGER_LEVEL {
        return Err(format!(
            "zone `{name}` danger_level must be within [0, {MAX_ZONE_DANGER_LEVEL}]"
        ));
    }

    let min = validate_dvec3(zone.aabb.min, format!("zone `{name}` aabb.min"))?;
    let max = validate_dvec3(zone.aabb.max, format!("zone `{name}` aabb.max"))?;
    if min.x > max.x || min.y > max.y || min.z > max.z {
        return Err(format!(
            "zone `{name}` has invalid aabb bounds: min must not exceed max"
        ));
    }

    for event_name in &zone.active_events {
        if event_name.trim().is_empty() {
            return Err(format!("zone `{name}` contains an empty active event name"));
        }
    }

    let mut patrol_anchors = Vec::with_capacity(zone.patrol_anchors.len());
    for (index, anchor) in zone.patrol_anchors.into_iter().enumerate() {
        let anchor = validate_dvec3(anchor, format!("zone `{name}` patrol_anchors[{index}]"))?;
        if !contains_bounds((min, max), anchor) {
            return Err(format!(
                "zone `{name}` patrol_anchors[{index}] must stay within the zone aabb"
            ));
        }
        patrol_anchors.push(anchor);
    }

    Ok(Zone {
        name: name.to_string(),
        bounds: (min, max),
        spirit_qi: zone.spirit_qi,
        danger_level: zone.danger_level,
        active_events: zone.active_events,
        patrol_anchors,
    })
}

fn validate_dvec3(value: [f64; 3], field_name: String) -> Result<DVec3, String> {
    if !value.into_iter().all(f64::is_finite) {
        return Err(format!("{field_name} must contain only finite numbers"));
    }

    Ok(dvec3_from_array(value))
}

fn contains_bounds(bounds: (DVec3, DVec3), pos: DVec3) -> bool {
    let (min, max) = bounds;

    pos.x >= min.x
        && pos.x <= max.x
        && pos.y >= min.y
        && pos.y <= max.y
        && pos.z >= min.z
        && pos.z <= max.z
}

pub fn default_spawn_bounds() -> (DVec3, DVec3) {
    (
        dvec3_from_array(DEFAULT_SPAWN_BOUNDS_MIN),
        DVec3::new(
            f64::from(TEST_AREA_BLOCK_EXTENT),
            DEFAULT_SPAWN_BOUNDS_MAX_Y,
            f64::from(TEST_AREA_BLOCK_EXTENT),
        ),
    )
}

pub fn register(app: &mut App) {
    tracing::info!("[bong][world] registering zone registry startup system");
    app.add_systems(Startup, initialize_zone_registry);
}

fn initialize_zone_registry(mut commands: Commands) {
    let registry = ZoneRegistry::load();

    tracing::info!(
        "[bong][world] initialized zone registry with {} zone(s)",
        registry.zones.len()
    );

    commands.insert_resource(registry);
}
fn dvec3_from_array(value: [f64; 3]) -> DVec3 {
    DVec3::new(value[0], value[1], value[2])
}

#[cfg(test)]
mod zone_tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};
    use valence::prelude::DVec3;

    #[test]
    fn loads_zones_json_with_fallback() {
        let valid_path = unique_temp_path("bong-zones-valid", ".json");
        fs::write(
            &valid_path,
            r#"{
  "zones": [
    {
      "name": "spawn",
      "aabb": {
        "min": [0.0, 64.0, 0.0],
        "max": [32.0, 80.0, 32.0]
      },
      "spirit_qi": 0.9,
      "danger_level": 0,
      "active_events": [],
      "patrol_anchors": [
        [14.0, 66.0, 14.0],
        [18.0, 66.0, 18.0]
      ]
    },
    {
      "name": "blood_valley",
      "aabb": {
        "min": [100.0, 64.0, 100.0],
        "max": [120.0, 80.0, 120.0]
      },
      "spirit_qi": 0.35,
      "danger_level": 4,
      "active_events": ["beast_tide"],
      "patrol_anchors": [
        [104.0, 66.0, 104.0]
      ]
    }
  ]
}"#,
        )
        .expect("valid zones.json fixture should be writable");

        let registry = ZoneRegistry::load_from_path(&valid_path);
        let spawn = registry
            .find_zone(DVec3::new(14.0, 66.0, 14.0))
            .expect("valid config should load spawn zone");
        let blood_valley = registry
            .find_zone(DVec3::new(110.0, 66.0, 110.0))
            .expect("valid config should load blood_valley zone");

        assert_eq!(registry.zones.len(), 2);
        assert_eq!(spawn.name, DEFAULT_SPAWN_ZONE_NAME);
        assert_eq!(spawn.patrol_anchors.len(), 2);
        assert_eq!(spawn.patrol_anchors[0], DVec3::new(14.0, 66.0, 14.0));
        assert_eq!(blood_valley.name, "blood_valley");
        assert_eq!(blood_valley.spirit_qi, 0.35);
        assert_eq!(blood_valley.danger_level, 4);
        assert_eq!(blood_valley.active_events, vec!["beast_tide".to_string()]);
        assert_eq!(
            blood_valley.patrol_anchors,
            vec![DVec3::new(104.0, 66.0, 104.0)]
        );

        let fallback_path = unique_temp_path("bong-zones-missing", ".json");
        let fallback_registry = ZoneRegistry::load_from_path(&fallback_path);
        assert_eq!(fallback_registry.zones.len(), 1);
        assert_eq!(fallback_registry.zones[0].name, DEFAULT_SPAWN_ZONE_NAME);
    }

    fn unique_temp_path(prefix: &str, suffix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("current time should be after unix epoch")
            .as_nanos();

        std::env::temp_dir().join(format!("{prefix}-{nanos}{suffix}"))
    }
}
