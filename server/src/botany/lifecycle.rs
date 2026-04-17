use std::collections::{HashMap, HashSet};

use valence::prelude::{Commands, Entity, Query, Res, ResMut, With};

use crate::world::zone::ZoneRegistry;

use super::components::{Plant, PlantLifecycleClock, PlantStaticPoint, PlantStaticPointStore};
use super::registry::{zone_supports, BotanyKindRegistry, BotanyPlantId, BotanySpawnMode};

const LIFECYCLE_INTERVAL_TICKS: u64 = 100;

#[allow(dead_code)]
pub fn spawn_static_points_for_zone(zone_name: &str) -> Vec<PlantStaticPoint> {
    vec![PlantStaticPoint {
        id: 1,
        zone_name: zone_name.to_string(),
        preferred_plant: BotanyPlantId::GuYuanGen,
        last_spawn_tick: None,
        regen_ticks: 7_200,
        bound_entity: None,
    }]
}

pub fn initialize_static_points_from_zones(
    mut static_points: ResMut<PlantStaticPointStore>,
    registry: Res<BotanyKindRegistry>,
    zone_registry: Option<Res<ZoneRegistry>>,
) {
    if static_points.is_initialized() {
        return;
    }

    let Some(zone_registry) = zone_registry else {
        return;
    };

    let mut next_id = 1_u64;
    for zone in &zone_registry.zones {
        for mut point in spawn_static_points_for_zone(zone.name.as_str()) {
            let Some(kind) = registry.get(point.preferred_plant) else {
                continue;
            };
            if kind.spawn_mode != BotanySpawnMode::StaticPoint || !zone_supports(kind, zone) {
                continue;
            }

            point.id = next_id;
            point.regen_ticks = kind.regen_ticks;
            point.last_spawn_tick = None;
            point.bound_entity = None;
            static_points.upsert(point);
            next_id = next_id.saturating_add(1);
        }
    }

    static_points.mark_initialized();
}

#[allow(clippy::too_many_arguments)]
pub fn run_botany_lifecycle_tick(
    mut commands: Commands,
    mut lifecycle_clock: ResMut<PlantLifecycleClock>,
    registry: Res<BotanyKindRegistry>,
    mut zone_registry: Option<ResMut<ZoneRegistry>>,
    mut static_points: ResMut<PlantStaticPointStore>,
    plants: Query<(Entity, &Plant), With<Plant>>,
) {
    lifecycle_clock.tick = lifecycle_clock.tick.saturating_add(1);
    if !lifecycle_clock
        .tick
        .is_multiple_of(LIFECYCLE_INTERVAL_TICKS)
    {
        return;
    }

    let Some(zone_registry) = zone_registry.as_deref_mut() else {
        return;
    };

    let now_tick = lifecycle_clock.tick;

    let mut restore_ops: Vec<(String, f64)> = Vec::new();
    let mut wither_targets: Vec<Entity> = Vec::new();
    let mut active_counts: HashMap<(String, BotanyPlantId), u32> = HashMap::new();
    let existing_entities: HashSet<Entity> = plants.iter().map(|(entity, _)| entity).collect();

    for point in static_points.iter_mut() {
        if let Some(bound_entity) = point.bound_entity {
            if !existing_entities.contains(&bound_entity) {
                point.bound_entity = None;
                point.last_spawn_tick.get_or_insert(now_tick);
            }
        }
    }

    for (entity, plant) in plants.iter() {
        let Some(zone) = zone_registry.find_zone_by_name(plant.zone_name.as_str()) else {
            continue;
        };
        let Some(kind) = registry.get(plant.id) else {
            continue;
        };

        if let Some(source_point) = plant.source_point {
            if let Some(point) = static_points.get_mut(source_point) {
                if point.bound_entity != Some(entity) {
                    point.bound_entity = Some(entity);
                }
            }
        }

        let age = lifecycle_clock.tick.saturating_sub(plant.planted_at_tick);
        let wither_due_harvest = plant.harvested;
        let wither_due_unsupported = !zone_supports(kind, zone);
        let wither_due_low_qi = zone.spirit_qi < f64::from(kind.survive_threshold);
        let wither_due_age = age >= kind.max_age_ticks;
        if wither_due_harvest || wither_due_unsupported || wither_due_low_qi || wither_due_age {
            wither_targets.push(entity);
            if !plant.harvested {
                restore_ops.push((
                    plant.zone_name.clone(),
                    f64::from(kind.growth_cost * kind.restore_ratio),
                ));
            }
            if let Some(source_point) = plant.source_point {
                if let Some(point) = static_points.get_mut(source_point) {
                    point.bound_entity = None;
                    point.last_spawn_tick = Some(now_tick);
                }
            }
            continue;
        }

        *active_counts
            .entry((plant.zone_name.clone(), plant.id))
            .or_default() += 1;
    }

    for target in wither_targets {
        commands.entity(target).despawn();
    }

    for (zone_name, restore) in restore_ops {
        if let Some(zone) = zone_registry.find_zone_mut(zone_name.as_str()) {
            zone.spirit_qi = (zone.spirit_qi + restore).clamp(-1.0, 1.0);
        }
    }

    for zone in &mut zone_registry.zones {
        if zone.spirit_qi <= 0.0 {
            continue;
        }

        for kind in registry.iter() {
            if kind.spawn_mode != BotanySpawnMode::ZoneRefresh {
                continue;
            }
            if !zone_supports(kind, zone) {
                continue;
            }

            let spawn_threshold = f64::from(kind.growth_cost.max(kind.survive_threshold));
            if zone.spirit_qi < spawn_threshold {
                continue;
            }

            let target_count =
                (zone.spirit_qi.max(0.0) as f32 * kind.density_factor).floor() as u32;
            if target_count == 0 {
                continue;
            }

            let count_key = (zone.name.clone(), kind.id);
            let current_count = active_counts.get(&count_key).copied().unwrap_or(0);
            for _ in current_count..target_count {
                if zone.spirit_qi < spawn_threshold {
                    break;
                }

                commands.spawn(Plant {
                    id: kind.id,
                    zone_name: zone.name.clone(),
                    planted_at_tick: now_tick,
                    wither_progress: 0,
                    source_point: None,
                    harvested: false,
                });
                zone.spirit_qi = (zone.spirit_qi - f64::from(kind.growth_cost)).clamp(-1.0, 1.0);
            }
        }
    }

    for point in static_points.iter_mut() {
        if point.bound_entity.is_some() {
            continue;
        }

        let Some(zone) = zone_registry.find_zone_mut(point.zone_name.as_str()) else {
            continue;
        };
        let Some(kind) = registry.get(point.preferred_plant) else {
            continue;
        };
        if kind.spawn_mode != BotanySpawnMode::StaticPoint || !zone_supports(kind, zone) {
            continue;
        }

        let cooldown_ready = point
            .last_spawn_tick
            .map(|last_spawn_tick| now_tick.saturating_sub(last_spawn_tick) >= point.regen_ticks)
            .unwrap_or(true);
        let spawn_threshold = f64::from(kind.growth_cost.max(kind.survive_threshold));
        if !cooldown_ready || zone.spirit_qi < spawn_threshold {
            continue;
        }

        let entity = commands
            .spawn(Plant {
                id: point.preferred_plant,
                zone_name: point.zone_name.clone(),
                planted_at_tick: now_tick,
                wither_progress: 0,
                source_point: Some(point.id),
                harvested: false,
            })
            .id();
        point.bound_entity = Some(entity);
        zone.spirit_qi = (zone.spirit_qi - f64::from(kind.growth_cost)).clamp(-1.0, 1.0);
    }
}

#[cfg(test)]
mod tests {
    use valence::prelude::{App, Position, Update};

    use super::*;
    use crate::botany::components::{PlantLifecycleClock, PlantStaticPointStore};
    use crate::botany::registry::BotanyPlantId;
    use crate::world::zone::{Zone, ZoneRegistry};

    #[test]
    fn zone_refresh_consumes_spirit_qi_when_kind_is_supported() {
        let mut app = App::new();
        app.insert_resource(BotanyKindRegistry::default());
        app.insert_resource(PlantStaticPointStore::default());
        app.insert_resource(PlantLifecycleClock {
            tick: LIFECYCLE_INTERVAL_TICKS - 1,
        });
        app.insert_resource(ZoneRegistry {
            zones: vec![Zone {
                name: "spawn".to_string(),
                bounds: (
                    Position::new([0.0, 0.0, 0.0]).get(),
                    Position::new([1.0, 1.0, 1.0]).get(),
                ),
                spirit_qi: 0.6,
                danger_level: 1,
                active_events: vec![],
                patrol_anchors: vec![],
                blocked_tiles: vec![],
            }],
        });
        app.add_systems(Update, run_botany_lifecycle_tick);

        app.update();

        let zone_registry = app.world().resource::<ZoneRegistry>();
        let after = zone_registry.zones[0].spirit_qi;
        assert!(after < 0.6, "zone refresh should spend spirit_qi");
    }

    #[test]
    fn natural_wither_restores_spirit_qi() {
        let mut app = App::new();
        app.insert_resource(BotanyKindRegistry::default());
        app.insert_resource(PlantStaticPointStore::default());
        app.insert_resource(PlantLifecycleClock {
            tick: LIFECYCLE_INTERVAL_TICKS - 1,
        });
        app.insert_resource(ZoneRegistry {
            zones: vec![Zone {
                name: "spawn".to_string(),
                bounds: (
                    Position::new([0.0, 0.0, 0.0]).get(),
                    Position::new([1.0, 1.0, 1.0]).get(),
                ),
                spirit_qi: 0.1,
                danger_level: 1,
                active_events: vec![],
                patrol_anchors: vec![],
                blocked_tiles: vec![],
            }],
        });

        app.world_mut().spawn(Plant {
            id: BotanyPlantId::NingMaiCao,
            zone_name: "spawn".to_string(),
            planted_at_tick: 0,
            wither_progress: 0,
            source_point: None,
            harvested: false,
        });

        app.add_systems(Update, run_botany_lifecycle_tick);
        app.update();

        let zone_registry = app.world().resource::<ZoneRegistry>();
        assert!(
            zone_registry.zones[0].spirit_qi > 0.1,
            "natural wither should restore spirit_qi"
        );
    }

    #[test]
    fn static_points_seed_only_for_supported_zones() {
        let mut app = App::new();
        app.insert_resource(BotanyKindRegistry::default());
        app.insert_resource(PlantStaticPointStore::default());
        app.insert_resource(ZoneRegistry {
            zones: vec![
                Zone {
                    name: "spawn".to_string(),
                    bounds: (
                        Position::new([0.0, 0.0, 0.0]).get(),
                        Position::new([1.0, 1.0, 1.0]).get(),
                    ),
                    spirit_qi: 0.9,
                    danger_level: 1,
                    active_events: vec![],
                    patrol_anchors: vec![],
                    blocked_tiles: vec![],
                },
                Zone {
                    name: "lingquan_marsh".to_string(),
                    bounds: (
                        Position::new([2.0, 0.0, 2.0]).get(),
                        Position::new([3.0, 1.0, 3.0]).get(),
                    ),
                    spirit_qi: 0.9,
                    danger_level: 2,
                    active_events: vec![],
                    patrol_anchors: vec![],
                    blocked_tiles: vec![],
                },
            ],
        });
        app.add_systems(Update, initialize_static_points_from_zones);

        app.update();

        let static_points = app.world().resource::<PlantStaticPointStore>();
        assert!(static_points.is_initialized());
        assert_eq!(static_points.len(), 1);
        let point = static_points
            .iter()
            .next()
            .expect("one static point should seed");
        assert_eq!(point.zone_name, "lingquan_marsh");
        assert_eq!(point.preferred_plant, BotanyPlantId::GuYuanGen);
    }

    #[test]
    fn harvested_static_point_unbinds_and_respawns_after_regen() {
        let mut app = App::new();
        app.insert_resource(BotanyKindRegistry::default());
        app.insert_resource(PlantLifecycleClock {
            tick: LIFECYCLE_INTERVAL_TICKS - 1,
        });

        let marsh_zone = Zone {
            name: "lingquan_marsh".to_string(),
            bounds: (
                Position::new([0.0, 0.0, 0.0]).get(),
                Position::new([1.0, 1.0, 1.0]).get(),
            ),
            spirit_qi: 0.9,
            danger_level: 2,
            active_events: vec![],
            patrol_anchors: vec![],
            blocked_tiles: vec![],
        };
        app.insert_resource(ZoneRegistry {
            zones: vec![marsh_zone],
        });

        let plant_entity = app
            .world_mut()
            .spawn(Plant {
                id: BotanyPlantId::GuYuanGen,
                zone_name: "lingquan_marsh".to_string(),
                planted_at_tick: 0,
                wither_progress: 0,
                source_point: Some(1),
                harvested: true,
            })
            .id();

        let mut static_points = PlantStaticPointStore::default();
        static_points.upsert(PlantStaticPoint {
            id: 1,
            zone_name: "lingquan_marsh".to_string(),
            preferred_plant: BotanyPlantId::GuYuanGen,
            last_spawn_tick: None,
            regen_ticks: LIFECYCLE_INTERVAL_TICKS,
            bound_entity: Some(plant_entity),
        });
        static_points.mark_initialized();
        app.insert_resource(static_points);
        app.add_systems(Update, run_botany_lifecycle_tick);

        app.update();
        app.update();

        {
            let static_points = app.world().resource::<PlantStaticPointStore>();
            let point = static_points
                .iter()
                .next()
                .expect("static point should remain in store");
            assert_eq!(point.bound_entity, None);
            assert_eq!(point.last_spawn_tick, Some(LIFECYCLE_INTERVAL_TICKS));
        }

        {
            let world = app.world_mut();
            let mut plants = world.query::<&Plant>();
            let remaining_static_plants = plants
                .iter(world)
                .filter(|plant| plant.source_point == Some(1))
                .count();
            assert_eq!(remaining_static_plants, 0);
        }

        {
            let mut zones = app.world_mut().resource_mut::<ZoneRegistry>();
            zones.zones[0].spirit_qi = 1.0;
        }
        {
            let mut clock = app.world_mut().resource_mut::<PlantLifecycleClock>();
            clock.tick = LIFECYCLE_INTERVAL_TICKS * 2 - 1;
        }

        app.update();
        app.update();

        let static_points = app.world().resource::<PlantStaticPointStore>();
        let point = static_points
            .iter()
            .next()
            .expect("static point should still exist after respawn");
        assert!(
            point.bound_entity.is_some(),
            "static point should respawn after cooldown"
        );

        let world = app.world_mut();
        let mut plants = world.query::<&Plant>();
        let respawned = plants
            .iter(world)
            .filter(|plant| plant.source_point == Some(1))
            .collect::<Vec<_>>();
        assert_eq!(respawned.len(), 1);
        assert_eq!(respawned[0].id, BotanyPlantId::GuYuanGen);
        assert_eq!(respawned[0].source_point, Some(1));
    }
}
