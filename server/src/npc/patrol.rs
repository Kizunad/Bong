use std::collections::HashSet;

use big_brain::prelude::{ActionState, Actor};
use pathfinding::prelude::astar;
use valence::prelude::{
    bevy_ecs, App, Component, DVec3, Entity, Position, Query, Res, ResMut, Resource, Update, With,
};

use crate::npc::brain::FleeAction;
use crate::npc::spawn::NpcMarker;
use crate::world::zone::{Zone, ZoneRegistry};

const PATROL_STEP_DISTANCE: f64 = 0.2;
const PATROL_TARGET_EPSILON: f64 = 0.25;
const PATROL_REPATH_INTERVAL_TICKS: u16 = 10;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct PatrolTile {
    x: i32,
    z: i32,
}

impl PatrolTile {
    fn heuristic_cost(self, target: Self) -> u32 {
        self.x.abs_diff(target.x) + self.z.abs_diff(target.z)
    }
}

#[derive(Debug)]
struct PatrolGrid<'a> {
    zone: &'a Zone,
    min_x: i32,
    max_x: i32,
    min_z: i32,
    max_z: i32,
    blocked_tiles: HashSet<PatrolTile>,
}

impl<'a> PatrolGrid<'a> {
    fn new(zone: &'a Zone) -> Option<Self> {
        let (min, max) = zone.bounds;
        let min_x = min.x.ceil() as i32;
        let max_x = max.x.floor() as i32;
        let min_z = min.z.ceil() as i32;
        let max_z = max.z.floor() as i32;

        if min_x > max_x || min_z > max_z {
            return None;
        }

        let blocked_tiles = zone
            .blocked_tiles
            .iter()
            .map(|&(x, z)| PatrolTile { x, z })
            .collect();

        Some(Self {
            zone,
            min_x,
            max_x,
            min_z,
            max_z,
            blocked_tiles,
        })
    }

    fn clamp_tile(&self, position: DVec3) -> PatrolTile {
        PatrolTile {
            x: (position.x.floor() as i32).clamp(self.min_x, self.max_x),
            z: (position.z.floor() as i32).clamp(self.min_z, self.max_z),
        }
    }

    fn is_within_bounds(&self, tile: PatrolTile) -> bool {
        tile.x >= self.min_x && tile.x <= self.max_x && tile.z >= self.min_z && tile.z <= self.max_z
    }

    fn is_walkable(&self, tile: PatrolTile) -> bool {
        self.is_within_bounds(tile) && !self.blocked_tiles.contains(&tile)
    }

    fn tile_position(&self, tile: PatrolTile, y: f64) -> DVec3 {
        self.zone
            .clamp_position(DVec3::new(f64::from(tile.x), y, f64::from(tile.z)))
    }

    fn successors(&self, tile: PatrolTile, start_tile: PatrolTile) -> Vec<(PatrolTile, u32)> {
        [(-1, 0), (1, 0), (0, -1), (0, 1)]
            .into_iter()
            .filter_map(|(dx, dz)| {
                let next = PatrolTile {
                    x: tile.x + dx,
                    z: tile.z + dz,
                };

                if self.is_within_bounds(next) && (next == start_tile || self.is_walkable(next)) {
                    Some((next, 1))
                } else {
                    None
                }
            })
            .collect()
    }
}

#[derive(Clone, Debug, Component)]
pub struct NpcPatrol {
    pub home_zone: String,
    pub anchor_index: usize,
    pub current_target: DVec3,
    warned_missing_zone: bool,
    path_tiles: Vec<PatrolTile>,
    path_index: usize,
    repath_ticks_remaining: u16,
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
            path_tiles: Vec::new(),
            path_index: 0,
            repath_ticks_remaining: 0,
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
    let previous_target = zone.clamp_position(patrol.current_target);
    let next_target = next_patrol_target(&zone, patrol, current_position);
    let target_changed = previous_target.distance_squared(next_target) > f64::EPSILON;
    let next_position =
        next_patrol_position(&zone, patrol, current_position, next_target, target_changed);

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

fn next_patrol_position(
    zone: &Zone,
    patrol: &mut NpcPatrol,
    current_position: DVec3,
    target_position: DVec3,
    target_changed: bool,
) -> DVec3 {
    let Some(grid) = PatrolGrid::new(zone) else {
        return step_toward(current_position, target_position, PATROL_STEP_DISTANCE);
    };

    refresh_patrol_path(
        &grid,
        patrol,
        current_position,
        target_position,
        target_changed,
    );
    advance_path_index(&grid, patrol, current_position, target_position.y);

    if let Some(next_tile) = patrol.path_tiles.get(patrol.path_index).copied() {
        let waypoint = grid.tile_position(next_tile, target_position.y);
        return step_toward(current_position, waypoint, PATROL_STEP_DISTANCE);
    }

    if grid.clamp_tile(current_position) == grid.clamp_tile(target_position) {
        return step_toward(current_position, target_position, PATROL_STEP_DISTANCE);
    }

    current_position
}

fn refresh_patrol_path(
    grid: &PatrolGrid<'_>,
    patrol: &mut NpcPatrol,
    current_position: DVec3,
    target_position: DVec3,
    target_changed: bool,
) {
    if patrol.repath_ticks_remaining > 0 {
        patrol.repath_ticks_remaining -= 1;
    }

    let needs_repath = target_changed
        || patrol.repath_ticks_remaining == 0
        || patrol.path_index >= patrol.path_tiles.len();
    if !needs_repath {
        return;
    }

    patrol.path_tiles = plan_patrol_path(grid, current_position, target_position);
    patrol.path_index = 0;
    patrol.repath_ticks_remaining = PATROL_REPATH_INTERVAL_TICKS;
}

fn plan_patrol_path(
    grid: &PatrolGrid<'_>,
    current_position: DVec3,
    target_position: DVec3,
) -> Vec<PatrolTile> {
    let start_tile = grid.clamp_tile(current_position);
    let target_tile = grid.clamp_tile(target_position);

    if start_tile == target_tile {
        return Vec::new();
    }

    astar(
        &start_tile,
        |tile| grid.successors(*tile, start_tile),
        |tile| tile.heuristic_cost(target_tile),
        |tile| *tile == target_tile,
    )
    .map(|(path, _cost)| path.into_iter().skip(1).collect())
    .unwrap_or_default()
}

fn advance_path_index(
    grid: &PatrolGrid<'_>,
    patrol: &mut NpcPatrol,
    current_position: DVec3,
    target_y: f64,
) {
    while let Some(next_tile) = patrol.path_tiles.get(patrol.path_index).copied() {
        let waypoint = grid.tile_position(next_tile, target_y);
        if current_position.distance_squared(waypoint)
            > PATROL_TARGET_EPSILON * PATROL_TARGET_EPSILON
        {
            break;
        }

        patrol.path_index += 1;
    }
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
                blocked_tiles: Vec::new(),
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

    #[test]
    fn npc_patrol_routes_around_blocked_tiles_with_a_star() {
        let blocked_tile = PatrolTile { x: 5, z: 5 };
        let zone = Zone {
            name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            bounds: (DVec3::new(0.0, 64.0, 0.0), DVec3::new(10.0, 80.0, 10.0)),
            spirit_qi: 0.9,
            danger_level: 0,
            active_events: Vec::new(),
            patrol_anchors: vec![DVec3::new(6.0, 66.0, 5.0)],
            blocked_tiles: vec![(5, 5)],
        };

        let grid = PatrolGrid::new(&zone).expect("test zone should produce a patrol grid");
        let planned_path = plan_patrol_path(
            &grid,
            DVec3::new(4.0, 66.0, 5.0),
            DVec3::new(6.0, 66.0, 5.0),
        );
        assert!(
            planned_path.iter().all(|tile| *tile != blocked_tile),
            "A* path should not route through blocked tiles"
        );
        assert!(
            planned_path.iter().any(|tile| tile.z != 5),
            "A* path should detour instead of returning the straight blocked line"
        );

        let mut app = App::new();
        app.insert_resource(ZoneRegistry { zones: vec![zone] });
        app.insert_resource(PatrolWarningsResource::default());
        app.add_systems(Update, patrol_npcs);

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([4.0, 66.0, 5.0]),
                NpcPatrol::new(DEFAULT_SPAWN_ZONE_NAME, DVec3::new(6.0, 66.0, 5.0)),
            ))
            .id();

        app.update();

        let world = app.world();
        let position = world
            .get::<Position>(npc)
            .expect("NPC should keep a position")
            .get();
        let patrol = world
            .get::<NpcPatrol>(npc)
            .expect("NPC should keep a patrol component");

        assert!(
            patrol.path_tiles.iter().all(|tile| *tile != blocked_tile),
            "cached patrol path should exclude blocked tiles"
        );
        assert!(
            patrol.path_tiles.iter().any(|tile| tile.z != 5),
            "cached patrol path should include a detour around the obstacle"
        );
        assert!(
            position.distance_squared(DVec3::new(5.0, 66.0, 5.0))
                > PATROL_STEP_DISTANCE * PATROL_STEP_DISTANCE,
            "first patrol step should not move toward the blocked straight-line tile"
        );
        assert!(
            position.distance_squared(DVec3::new(4.0, 66.0, 5.0)) > f64::EPSILON,
            "first patrol step should move somewhere along the detour path"
        );
    }

    fn unique_temp_path(prefix: &str, suffix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("current time should be after unix epoch")
            .as_nanos();

        std::env::temp_dir().join(format!("{prefix}-{nanos}{suffix}"))
    }
}
