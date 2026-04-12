use valence::prelude::{bevy_ecs, App, Component, DVec3, Entity, Position, Query, Res, ResMut, Resource, Update, With};

use crate::npc::navigator::Navigator;
use crate::npc::spawn::NpcMarker;
use crate::world::zone::{Zone, ZoneRegistry};

/// Distance the NPC must be within to consider a waypoint "reached".
const PATROL_TARGET_EPSILON: f64 = 2.0;

/// Speed factor for patrol movement (slower than chase).
const PATROL_SPEED_FACTOR: f64 = 0.6;

// ---------------------------------------------------------------------------
// NpcPatrol component
// ---------------------------------------------------------------------------

/// Patrol state: which zone the NPC belongs to and which anchor it's heading for.
///
/// The actual pathfinding and movement is handled by [`Navigator`].
/// This component just manages the high-level "cycle through patrol anchors" logic.
#[derive(Clone, Debug, Component)]
pub struct NpcPatrol {
    pub home_zone: String,
    pub anchor_index: usize,
    pub current_target: DVec3,
    warned_missing_zone: bool,
}

#[derive(Clone, Debug, Default)]
pub struct PatrolWarningsResource {
    warned_missing_registry: bool,
}

impl Resource for PatrolWarningsResource {}

impl NpcPatrol {
    pub fn new(home_zone: impl Into<String>, current_target: DVec3) -> Self {
        Self {
            home_zone: home_zone.into(),
            anchor_index: 0,
            current_target,
            warned_missing_zone: false,
        }
    }
}

// ---------------------------------------------------------------------------
// System
// ---------------------------------------------------------------------------

pub fn register(app: &mut App) {
    tracing::info!("[bong][npc] registering patrol system");
    app.insert_resource(PatrolWarningsResource::default());
    app.add_systems(Update, patrol_npcs);
}

/// Patrol system: for each NPC whose Navigator is idle, set the next patrol
/// anchor as the navigation goal.
///
/// If the Navigator already has a goal (chase/flee/etc.), patrol does nothing —
/// the Navigator is "owned" by whatever brain action set the goal.
fn patrol_npcs(
    zone_registry: Option<Res<ZoneRegistry>>,
    mut patrol_warnings: ResMut<PatrolWarningsResource>,
    mut npcs: Query<(Entity, &Position, &mut NpcPatrol, &mut Navigator), With<NpcMarker>>,
) {
    if zone_registry.is_none() && !patrol_warnings.warned_missing_registry {
        tracing::warn!(
            "[bong][npc] ZoneRegistry resource is missing; patrol will degrade to fallback spawn zone"
        );
        patrol_warnings.warned_missing_registry = true;
    } else if zone_registry.is_some() {
        patrol_warnings.warned_missing_registry = false;
    }

    let zone_registry = zone_registry.as_deref();

    for (_entity, position, mut patrol, mut navigator) in &mut npcs {
        // If the navigator already has a goal (chase/flee), don't override it.
        if !navigator.is_idle() {
            continue;
        }

        let zone = resolve_patrol_zone(zone_registry, patrol.as_mut(), position.get());
        let current_pos = zone.clamp_position(position.get());
        let target = zone.clamp_position(patrol.current_target);

        // Check if we've reached the current target.
        let dx = current_pos.x - target.x;
        let dz = current_pos.z - target.z;
        if dx * dx + dz * dz <= PATROL_TARGET_EPSILON * PATROL_TARGET_EPSILON {
            // Advance to the next anchor.
            let next_target = zone.clamp_position(zone.patrol_target(patrol.anchor_index));
            patrol.anchor_index = next_anchor_index(&zone, patrol.anchor_index);
            patrol.current_target = next_target;
        }

        // Set the navigator goal to the current patrol target.
        let clamped_target = zone.clamp_position(patrol.current_target);
        navigator.set_goal(clamped_target, PATROL_SPEED_FACTOR);
    }
}

fn resolve_patrol_zone(
    zone_registry: Option<&ZoneRegistry>,
    patrol: &mut NpcPatrol,
    current_position: DVec3,
) -> Zone {
    if let Some(zone_registry) = zone_registry {
        if let Some(zone) = zone_registry.find_zone_by_name(&patrol.home_zone) {
            patrol.warned_missing_zone = false;
            return zone.clone();
        }

        if let Some(zone) = zone_registry.find_zone(current_position) {
            if !patrol.warned_missing_zone {
                tracing::warn!(
                    "[bong][npc] patrol home zone `{}` missing; re-homing NPC patrol to zone `{}`",
                    patrol.home_zone,
                    zone.name
                );
            }

            patrol.home_zone = zone.name.clone();
            patrol.warned_missing_zone = false;
            return zone.clone();
        }
    }

    if !patrol.warned_missing_zone {
        tracing::warn!(
            "[bong][npc] patrol home zone `{}` is unavailable; using fallback spawn zone",
            patrol.home_zone
        );
        patrol.warned_missing_zone = true;
    }

    let zone = fallback_spawn_zone();
    patrol.home_zone = zone.name.clone();
    zone
}

fn next_anchor_index(zone: &Zone, anchor_index: usize) -> usize {
    if zone.patrol_anchors.is_empty() {
        0
    } else {
        (anchor_index + 1) % zone.patrol_anchors.len()
    }
}

fn fallback_spawn_zone() -> Zone {
    ZoneRegistry::fallback()
        .zones
        .into_iter()
        .next()
        .expect("fallback registry should always include a spawn zone")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod patrol_tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::npc::navigator::Navigator;
    use crate::world::zone::{Zone, DEFAULT_SPAWN_ZONE_NAME};

    fn small_zone() -> Zone {
        Zone {
            name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            bounds: (DVec3::new(0.0, 0.0, 0.0), DVec3::new(80.0, 320.0, 80.0)),
            spirit_qi: 0.9,
            danger_level: 0,
            active_events: Vec::new(),
            patrol_anchors: vec![DVec3::new(4.0, 66.0, 4.0), DVec3::new(72.0, 66.0, 72.0)],
            blocked_tiles: Vec::new(),
        }
    }

    fn test_zone_registry() -> ZoneRegistry {
        ZoneRegistry {
            zones: vec![small_zone()],
        }
    }

    #[test]
    fn patrol_sets_navigator_goal_when_idle() {
        let mut app = App::new();
        app.insert_resource(test_zone_registry());
        app.insert_resource(PatrolWarningsResource::default());
        app.add_systems(Update, patrol_npcs);

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([4.0, 66.0, 4.0]),
                NpcPatrol::new(DEFAULT_SPAWN_ZONE_NAME, DVec3::new(72.0, 66.0, 72.0)),
                Navigator::new(),
            ))
            .id();

        app.update();

        let nav = app
            .world()
            .get::<Navigator>(npc)
            .expect("Navigator should exist");
        assert!(
            !nav.is_idle(),
            "patrol should have set a navigator goal"
        );
    }

    #[test]
    fn patrol_does_not_override_active_navigator() {
        let mut app = App::new();
        app.insert_resource(test_zone_registry());
        app.insert_resource(PatrolWarningsResource::default());
        app.add_systems(Update, patrol_npcs);

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([4.0, 66.0, 4.0]),
                NpcPatrol::new(DEFAULT_SPAWN_ZONE_NAME, DVec3::new(72.0, 66.0, 72.0)),
                Navigator::new(),
            ))
            .id();

        // Pre-set a navigator goal (simulating chase/flee).
        {
            let mut nav = app
                .world_mut()
                .get_mut::<Navigator>(npc)
                .unwrap();
            nav.set_goal(DVec3::new(999.0, 67.0, 999.0), 2.0);
        }

        app.update();

        // Patrol should NOT have overridden the existing goal.
        let nav = app.world().get::<Navigator>(npc).unwrap();
        assert!(!nav.is_idle());
    }

    #[test]
    fn patrol_degrades_gracefully_without_zone_registry() {
        let mut app = App::new();
        app.insert_resource(PatrolWarningsResource::default());
        app.add_systems(Update, patrol_npcs);

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([14.0, 67.0, 14.0]),
                NpcPatrol::new(DEFAULT_SPAWN_ZONE_NAME, DVec3::new(14.0, 67.0, 14.0)),
                Navigator::new(),
            ))
            .id();

        app.update();

        let patrol = app
            .world()
            .get::<NpcPatrol>(npc)
            .expect("NPC should keep a patrol component");
        assert_eq!(patrol.home_zone, DEFAULT_SPAWN_ZONE_NAME);
    }

    #[test]
    fn invalid_zones_file_uses_spawn_fallback() {
        let invalid_path = unique_temp_path("bong-zones-invalid", ".json");
        fs::write(
            &invalid_path,
            r#"{
  "zones": [
    {
      "name": "spawn",
      "aabb": {
        "min": [0.0, 64.0, 0.0],
        "max": [10.0, 80.0, 10.0]
      },
      "spirit_qi": 0.9,
      "danger_level": 0,
      "active_events": [],
      "patrol_anchors": [
        [999.0, 66.0, 999.0]
      ]
    }
  ]
}"#,
        )
        .expect("invalid zones.json fixture should be writable");

        let registry = ZoneRegistry::load_from_path(&invalid_path);
        assert_eq!(registry.zones.len(), 1);
        assert_eq!(registry.zones[0].name, DEFAULT_SPAWN_ZONE_NAME);
    }

    fn unique_temp_path(prefix: &str, suffix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("current time should be after unix epoch")
            .as_nanos();

        std::env::temp_dir().join(format!("{prefix}-{nanos}{suffix}"))
    }
}
