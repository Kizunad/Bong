use std::collections::HashSet;

use big_brain::prelude::{ActionState, Actor};
use valence::prelude::{
    bevy_ecs, App, Component, DVec3, Entity, Position, Query, Res, ResMut, Resource, Update, With,
};

use crate::npc::brain::FleeAction;
use crate::npc::spawn::NpcMarker;
use crate::world::zone::{Zone, ZoneRegistry};

const PATROL_STEP_DISTANCE: f64 = 0.2;
const PATROL_TARGET_EPSILON: f64 = 0.25;

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

pub fn register(app: &mut App) {
    tracing::info!("[bong][npc] registering patrol system");
    app.insert_resource(PatrolWarningsResource::default());
    app.add_systems(Update, patrol_npcs);
}

fn patrol_npcs(
    zone_registry: Option<Res<ZoneRegistry>>,
    mut patrol_warnings: ResMut<PatrolWarningsResource>,
    mut npcs: Query<(Entity, &mut Position, &mut NpcPatrol), With<NpcMarker>>,
    flee_actions: Query<(&Actor, &ActionState), With<FleeAction>>,
) {
    if zone_registry.is_none() && !patrol_warnings.warned_missing_registry {
        tracing::warn!(
            "[bong][npc] ZoneRegistry resource is missing; patrol will degrade to fallback spawn zone"
        );
        patrol_warnings.warned_missing_registry = true;
    } else if zone_registry.is_some() {
        patrol_warnings.warned_missing_registry = false;
    }

    let active_fleeing_npcs = active_fleeing_npcs(&flee_actions);
    let zone_registry = zone_registry.as_deref();

    for (entity, mut position, mut patrol) in &mut npcs {
        if active_fleeing_npcs.contains(&entity) {
            continue;
        }

        advance_patrol(position.as_mut(), patrol.as_mut(), zone_registry);
    }
}

fn advance_patrol(
    position: &mut Position,
    patrol: &mut NpcPatrol,
    zone_registry: Option<&ZoneRegistry>,
) {
    let zone = resolve_patrol_zone(zone_registry, patrol, position.get());
    let current_position = zone.clamp_position(position.get());
    let next_target = next_patrol_target(&zone, patrol, current_position);
    let next_position = step_toward(current_position, next_target, PATROL_STEP_DISTANCE);

    patrol.current_target = next_target;
    position.set(zone.clamp_position(next_position));
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

fn next_patrol_target(zone: &Zone, patrol: &mut NpcPatrol, current_position: DVec3) -> DVec3 {
    let current_target = zone.clamp_position(patrol.current_target);
    let target_reached = current_position.distance_squared(current_target)
        <= PATROL_TARGET_EPSILON * PATROL_TARGET_EPSILON;
    let target_outside_zone = !zone.contains(patrol.current_target);

    let next_target = if target_outside_zone || target_reached {
        let target = zone.clamp_position(zone.patrol_target(patrol.anchor_index));
        patrol.anchor_index = next_anchor_index(zone, patrol.anchor_index);
        target
    } else {
        current_target
    };

    zone.clamp_position(next_target)
}

fn next_anchor_index(zone: &Zone, anchor_index: usize) -> usize {
    if zone.patrol_anchors.is_empty() {
        0
    } else {
        (anchor_index + 1) % zone.patrol_anchors.len()
    }
}

fn step_toward(current: DVec3, target: DVec3, max_step: f64) -> DVec3 {
    if max_step <= 0.0 {
        return current;
    }

    let delta = target - current;
    let distance = delta.length();
    if distance <= max_step || distance <= f64::EPSILON {
        target
    } else {
        current + delta.normalize() * max_step
    }
}

fn active_fleeing_npcs(
    flee_actions: &Query<(&Actor, &ActionState), With<FleeAction>>,
) -> HashSet<Entity> {
    flee_actions
        .iter()
        .filter_map(|(actor, action_state)| match action_state {
            ActionState::Requested | ActionState::Executing => Some(actor.0),
            ActionState::Init
            | ActionState::Cancelled
            | ActionState::Success
            | ActionState::Failure => None,
        })
        .collect()
}

fn fallback_spawn_zone() -> Zone {
    ZoneRegistry::fallback()
        .zones
        .into_iter()
        .next()
        .expect("fallback registry should always include a spawn zone")
}

#[cfg(test)]
mod patrol_tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::world::zone::{Zone, DEFAULT_SPAWN_ZONE_NAME};

    fn test_zone_registry() -> ZoneRegistry {
        ZoneRegistry {
            zones: vec![Zone {
                name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
                bounds: (DVec3::new(0.0, 64.0, 0.0), DVec3::new(20.0, 80.0, 20.0)),
                spirit_qi: 0.9,
                danger_level: 0,
                active_events: Vec::new(),
                patrol_anchors: vec![DVec3::new(4.0, 66.0, 4.0), DVec3::new(18.0, 66.0, 18.0)],
            }],
        }
    }

    #[test]
    fn npc_patrol_stays_within_zone() {
        let mut app = App::new();
        app.insert_resource(test_zone_registry());
        app.insert_resource(PatrolWarningsResource::default());
        app.add_systems(Update, patrol_npcs);

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([19.9, 66.0, 19.9]),
                NpcPatrol::new(DEFAULT_SPAWN_ZONE_NAME, DVec3::new(999.0, 66.0, 999.0)),
            ))
            .id();

        for _ in 0..128 {
            app.update();

            let world = app.world();
            let zone_registry = world.resource::<ZoneRegistry>();
            let zone = zone_registry
                .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
                .expect("spawn zone should exist");
            let position = world
                .get::<Position>(npc)
                .expect("NPC should keep a position")
                .get();
            let patrol = world
                .get::<NpcPatrol>(npc)
                .expect("NPC should keep a patrol component");

            assert!(
                zone.contains(position),
                "patrol position should remain within zone bounds"
            );
            assert!(
                zone.contains(patrol.current_target),
                "patrol target should remain within zone bounds"
            );
        }
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

        let mut app = App::new();
        app.insert_resource(registry);
        app.insert_resource(PatrolWarningsResource::default());
        app.add_systems(Update, patrol_npcs);

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([14.0, 66.0, 14.0]),
                NpcPatrol::new("broken_zone", DVec3::new(999.0, 66.0, 999.0)),
            ))
            .id();

        app.update();

        let world = app.world();
        let zone = world
            .resource::<ZoneRegistry>()
            .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
            .expect("fallback spawn zone should exist");
        let position = world
            .get::<Position>(npc)
            .expect("NPC should keep a position")
            .get();
        let patrol = world
            .get::<NpcPatrol>(npc)
            .expect("NPC should keep a patrol component");

        assert_eq!(patrol.home_zone, DEFAULT_SPAWN_ZONE_NAME);
        assert!(zone.contains(position));
        assert!(zone.contains(patrol.current_target));
    }

    fn unique_temp_path(prefix: &str, suffix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("current time should be after unix epoch")
            .as_nanos();

        std::env::temp_dir().join(format!("{prefix}-{nanos}{suffix}"))
    }
}
