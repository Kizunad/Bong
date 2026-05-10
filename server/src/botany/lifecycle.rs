use std::collections::{HashMap, HashSet};

use valence::prelude::{Commands, Entity, Events, Query, Res, ResMut, With};

use crate::network::vfx_event_emit::VfxEventRequest;
use crate::schema::vfx_event::VfxEventPayloadV1;
use crate::world::terrain::TerrainProviders;
use crate::world::zone::{BotanyZoneTag, Zone, ZoneRegistry};

use super::components::{
    BotanyVariantRoll, Plant, PlantLifecycleClock, PlantStaticPoint, PlantStaticPointStore,
};
use super::registry::{
    zone_supports, BotanyKindRegistry, BotanyPlantId, BotanySpawnMode, PlantVariant, SurvivalMode,
};

const LIFECYCLE_INTERVAL_TICKS: u64 = 100;
const BOTANY_AURA_INTERVAL_TICKS: u64 = 200;
const BOTANY_AURA_EVENT_ID: &str = "bong:botany_aura";

#[allow(dead_code)]
pub fn spawn_static_points_for_zone(zone: &Zone) -> Vec<PlantStaticPoint> {
    // MVP 单点：zone 中心近似，Y 贴 zone 顶部（缺地面拾取的近似）
    let position = zone_center_position(zone);
    vec![PlantStaticPoint {
        id: 1,
        zone_name: zone.name.clone(),
        position,
        preferred_plant: BotanyPlantId::GuYuanGen,
        last_spawn_tick: None,
        regen_ticks: 7_200,
        bound_entity: None,
    }]
}

fn zone_center_position(zone: &Zone) -> [f64; 3] {
    [
        (zone.bounds.0.x + zone.bounds.1.x) * 0.5,
        zone.bounds.1.y,
        (zone.bounds.0.z + zone.bounds.1.z) * 0.5,
    ]
}

/// 在 zone 水平范围里用 splitmix 伪随机取一个 XZ 坐标，Y 贴 zone 顶部。
/// seed 建议拼入 kind + tick + spawn 序号，保证可观测的非趋同分布且便于测试。
fn zone_sampled_position(seed: u64, zone: &Zone) -> [f64; 3] {
    let x_span = (zone.bounds.1.x - zone.bounds.0.x).max(0.0);
    let z_span = (zone.bounds.1.z - zone.bounds.0.z).max(0.0);
    let (hx, hz) = (
        splitmix(seed),
        splitmix(seed.wrapping_add(0xA2D9_D6E1_4CA5_A73F)),
    );
    let fx = (hx % 10_000) as f64 / 10_000.0;
    let fz = (hz % 10_000) as f64 / 10_000.0;
    [
        zone.bounds.0.x + fx * x_span,
        zone.bounds.1.y,
        zone.bounds.0.z + fz * z_span,
    ]
}

fn splitmix(seed: u64) -> u64 {
    let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn spawn_seed(now_tick: u64, kind: BotanyPlantId, spawn_idx: u32) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    kind.as_str().hash(&mut hasher);
    let kind_seed = hasher.finish();
    now_tick.wrapping_mul(0xA342_3F3A_1E8B_C11D)
        ^ kind_seed.wrapping_mul(0xD1B5_4A32_D192_ED03)
        ^ u64::from(spawn_idx)
}

fn v2_env_locks_hold(
    kind: &super::registry::BotanyPlantKind,
    position: [f64; 3],
    zone: &Zone,
    terrain_providers: Option<&TerrainProviders>,
) -> bool {
    let Some(terrain) =
        terrain_providers.and_then(|providers| providers.for_dimension(zone.dimension))
    else {
        return false;
    };
    super::env_lock::check_env_locks(
        kind,
        position[0].round() as i32,
        position[2].round() as i32,
        terrain,
        zone,
    )
}

fn spawn_v2_plants_for_zone(
    commands: &mut Commands,
    zone: &mut Zone,
    registry: &BotanyKindRegistry,
    variant_roll: &BotanyVariantRoll,
    now_tick: u64,
    active_counts: &mut HashMap<(String, BotanyPlantId), u32>,
    terrain_providers: Option<&TerrainProviders>,
) {
    let Some(terrain) =
        terrain_providers.and_then(|providers| providers.for_dimension(zone.dimension))
    else {
        return;
    };

    for kind in registry.iter().filter(|kind| kind.is_v2()) {
        let Some(spec) = kind.v2_spec() else {
            continue;
        };
        let Some((position, growth_score)) =
            v2_candidate_position(kind.id, spec.survival_mode, zone, terrain, now_tick)
        else {
            continue;
        };
        if growth_score <= 0.0 {
            continue;
        }
        if !super::env_lock::check_env_locks(
            kind,
            position[0].round() as i32,
            position[2].round() as i32,
            terrain,
            zone,
        ) {
            continue;
        }

        let target_count = u32::from(growth_score >= 1.0);
        if target_count == 0 {
            continue;
        }
        let count_key = (zone.name.clone(), kind.id);
        let current_count = active_counts.get(&count_key).copied().unwrap_or(0);
        for spawn_idx in current_count..target_count {
            let seed = spawn_seed(now_tick, kind.id, spawn_idx);
            commands.spawn(Plant {
                id: kind.id,
                zone_name: zone.name.clone(),
                position,
                planted_at_tick: now_tick,
                wither_progress: 0,
                source_point: None,
                harvested: false,
                trampled: false,
                variant: roll_variant_for_zone(
                    zone,
                    seed.wrapping_mul(0x5851_F42D_4C95_7F2D),
                    variant_roll,
                ),
            });
            *active_counts.entry(count_key.clone()).or_default() += 1;
            zone.spirit_qi = (zone.spirit_qi - f64::from(kind.growth_cost)).clamp(-1.0, 1.0);
        }
    }
}

fn v2_candidate_position(
    kind: BotanyPlantId,
    survival_mode: SurvivalMode,
    zone: &Zone,
    terrain: &crate::world::terrain::TerrainProvider,
    now_tick: u64,
) -> Option<([f64; 3], f32)> {
    let seed = spawn_seed(now_tick, kind, 0);
    let mut position = zone_sampled_position(seed, zone);
    let x = position[0].round() as i32;
    let z = position[2].round() as i32;
    let surface = crate::world::terrain::SurfaceProvider::query_surface(terrain, x, z);
    let sample = terrain.sample(x, z);
    position[1] = if sample.cavern_floor_y < 9000.0 {
        f64::from(sample.cavern_floor_y)
    } else if sample.sky_island_base_y < 9000.0 {
        f64::from(sample.sky_island_base_y + sample.sky_island_thickness)
    } else {
        f64::from(surface.y + 1)
    };
    let score = match survival_mode {
        SurvivalMode::QiAbsorb => zone.spirit_qi.max(0.0) as f32 * 2.0,
        SurvivalMode::NegPressureFeed => sample.neg_pressure * 2.0,
        SurvivalMode::PressureDifferential => {
            sample.qi_vein_flow * (1.0 - sample.mofa_decay).max(0.0) * 3.0
        }
        SurvivalMode::SpiritCrystallize => sample.qi_density * 1.5,
        SurvivalMode::RuinResonance => sample.ruin_density * 2.0,
        SurvivalMode::ThermalConvection => (sample.underground_tier == 2) as u8 as f32,
        SurvivalMode::PortalSiphon => {
            if zone
                .active_events
                .iter()
                .any(|event| event == "portal_rift")
            {
                1.0
            } else {
                0.0
            }
        }
        SurvivalMode::DualMetabolism => sample.qi_density * 1.2 + sample.mofa_decay * 1.2,
        SurvivalMode::PhotoLuminance => 1.0,
        SurvivalMode::WaterPulse => {
            if zone
                .active_events
                .iter()
                .any(|event| event == "water_pulse_open")
            {
                1.0
            } else {
                0.0
            }
        }
    };
    Some((position, score))
}

/// plan §7：按 zone 环境决定植物变种。
/// - `Thunder`：zone.active_events 含 "thunder" / "tribulation"（不区分大小写）
/// - `Tainted`：zone.spirit_qi < 0 或 zone 带 NegativeField tag
///
/// 即使 zone 合格，也要通过 `BotanyVariantRoll` 概率掷骰才会变种（默认 1/3）。
pub(crate) fn roll_variant_for_zone(
    zone: &Zone,
    seed: u64,
    roll_cfg: &BotanyVariantRoll,
) -> PlantVariant {
    if roll_cfg.chance_inverse == 0 {
        return PlantVariant::None;
    }

    let thunder = zone.active_events.iter().any(|event| {
        let lower = event.to_ascii_lowercase();
        lower.contains("thunder") || lower.contains("tribulation")
    });
    let tainted = !thunder
        && (zone.spirit_qi < 0.0 || zone.supports_botany_tag(BotanyZoneTag::NegativeField));

    if !thunder && !tainted {
        return PlantVariant::None;
    }

    if splitmix(seed) % u64::from(roll_cfg.chance_inverse) != 0 {
        return PlantVariant::None;
    }

    if thunder {
        PlantVariant::Thunder
    } else {
        PlantVariant::Tainted
    }
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
        for mut point in spawn_static_points_for_zone(zone) {
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
    variant_roll: Res<BotanyVariantRoll>,
    terrain_providers: Option<Res<TerrainProviders>>,
    plants: Query<(Entity, &Plant), With<Plant>>,
    mut vfx_events: Option<ResMut<Events<VfxEventRequest>>>,
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
        let wither_due_trampled = plant.trampled;
        // EventTriggered 植物（如 kong_shou_hen）不检查 biome / spirit_qi 下限（plan §1.2.3）。
        let is_event_kind = kind.spawn_mode == BotanySpawnMode::EventTriggered;
        let v2_env_locked = kind.v2_spec().is_some_and(|_| {
            !v2_env_locks_hold(kind, plant.position, zone, terrain_providers.as_deref())
        });
        let wither_due_unsupported = !kind.is_v2() && !is_event_kind && !zone_supports(kind, zone);
        let wither_due_low_qi =
            !kind.is_v2() && !is_event_kind && zone.spirit_qi < f64::from(kind.survive_threshold);
        let wither_due_age = age >= kind.max_age_ticks;
        if wither_due_harvest
            || wither_due_trampled
            || wither_due_unsupported
            || wither_due_low_qi
            || v2_env_locked
            || wither_due_age
        {
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

        if now_tick.is_multiple_of(BOTANY_AURA_INTERVAL_TICKS) && zone.spirit_qi >= 0.5 {
            if let Some(vfx_events) = vfx_events.as_deref_mut() {
                emit_botany_aura_vfx(vfx_events, plant.position, zone.spirit_qi as f32);
            }
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
        spawn_v2_plants_for_zone(
            &mut commands,
            zone,
            &registry,
            variant_roll.as_ref(),
            now_tick,
            &mut active_counts,
            terrain_providers.as_deref(),
        );

        if zone.spirit_qi <= 0.0 {
            continue;
        }

        for kind in registry.iter() {
            if kind.is_v2() || kind.spawn_mode != BotanySpawnMode::ZoneRefresh {
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
            for spawn_idx in current_count..target_count {
                if zone.spirit_qi < spawn_threshold {
                    break;
                }

                let seed = spawn_seed(now_tick, kind.id, spawn_idx);
                let position = zone_sampled_position(seed, zone);
                let variant = roll_variant_for_zone(
                    zone,
                    seed.wrapping_mul(0x5851_F42D_4C95_7F2D),
                    variant_roll.as_ref(),
                );
                commands.spawn(Plant {
                    id: kind.id,
                    zone_name: zone.name.clone(),
                    position,
                    planted_at_tick: now_tick,
                    wither_progress: 0,
                    source_point: None,
                    harvested: false,
                    trampled: false,
                    variant,
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

        let static_variant = roll_variant_for_zone(
            zone,
            now_tick
                .wrapping_mul(0x94D0_49BB_1331_11EB)
                .wrapping_add(point.id),
            variant_roll.as_ref(),
        );
        let entity = commands
            .spawn(Plant {
                id: point.preferred_plant,
                zone_name: point.zone_name.clone(),
                position: point.position,
                planted_at_tick: now_tick,
                wither_progress: 0,
                source_point: Some(point.id),
                harvested: false,
                trampled: false,
                variant: static_variant,
            })
            .id();
        point.bound_entity = Some(entity);
        zone.spirit_qi = (zone.spirit_qi - f64::from(kind.growth_cost)).clamp(-1.0, 1.0);
    }
}

fn emit_botany_aura_vfx(
    vfx_events: &mut Events<VfxEventRequest>,
    position: [f64; 3],
    spirit_quality: f32,
) {
    let origin = [position[0], position[1] + 0.55, position[2]];
    vfx_events.send(VfxEventRequest::new(
        valence::prelude::DVec3::new(origin[0], origin[1], origin[2]),
        VfxEventPayloadV1::SpawnParticle {
            event_id: BOTANY_AURA_EVENT_ID.to_string(),
            origin,
            direction: Some([0.0, 1.0, 0.0]),
            color: Some(botany_aura_color(spirit_quality).to_string()),
            strength: Some(spirit_quality.clamp(0.5, 1.0)),
            count: Some(4),
            duration_ticks: Some(80),
        },
    ));
}

fn botany_aura_color(spirit_quality: f32) -> &'static str {
    if spirit_quality >= 0.9 {
        "#FFDD22"
    } else if spirit_quality >= 0.7 {
        "#22FF44"
    } else {
        "#88CC88"
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
        app.insert_resource(BotanyVariantRoll::default());
        app.insert_resource(PlantLifecycleClock {
            tick: LIFECYCLE_INTERVAL_TICKS - 1,
        });
        app.insert_resource(ZoneRegistry {
            zones: vec![Zone {
                name: "spawn".to_string(),
                dimension: crate::world::dimension::DimensionKind::Overworld,
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
        app.insert_resource(BotanyVariantRoll::default());
        app.insert_resource(PlantLifecycleClock {
            tick: LIFECYCLE_INTERVAL_TICKS - 1,
        });
        app.insert_resource(ZoneRegistry {
            zones: vec![Zone {
                name: "spawn".to_string(),
                dimension: crate::world::dimension::DimensionKind::Overworld,
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
            position: [0.5, 1.0, 0.5],
            planted_at_tick: 0,
            wither_progress: 0,
            source_point: None,
            harvested: false,
            trampled: false,
            variant: PlantVariant::None,
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
    fn mature_plant_emits_aura_vfx_on_cadence() {
        let mut app = App::new();
        app.insert_resource(BotanyKindRegistry::default());
        app.insert_resource(PlantStaticPointStore::default());
        app.insert_resource(BotanyVariantRoll::default());
        app.insert_resource(PlantLifecycleClock {
            tick: BOTANY_AURA_INTERVAL_TICKS - 1,
        });
        app.insert_resource(ZoneRegistry {
            zones: vec![Zone {
                name: "spawn".to_string(),
                dimension: crate::world::dimension::DimensionKind::Overworld,
                bounds: (
                    Position::new([0.0, 0.0, 0.0]).get(),
                    Position::new([1.0, 1.0, 1.0]).get(),
                ),
                spirit_qi: 0.8,
                danger_level: 1,
                active_events: vec![],
                patrol_anchors: vec![],
                blocked_tiles: vec![],
            }],
        });
        app.add_event::<VfxEventRequest>();
        app.world_mut().spawn(Plant {
            id: BotanyPlantId::NingMaiCao,
            zone_name: "spawn".to_string(),
            position: [0.5, 1.0, 0.5],
            planted_at_tick: 0,
            wither_progress: 0,
            source_point: None,
            harvested: false,
            trampled: false,
            variant: PlantVariant::None,
        });
        app.add_systems(Update, run_botany_lifecycle_tick);

        app.update();

        let emitted: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<VfxEventRequest>>()
            .drain()
            .collect();
        assert_eq!(emitted.len(), 1);
        match &emitted[0].payload {
            VfxEventPayloadV1::SpawnParticle {
                event_id,
                color,
                count,
                ..
            } => {
                assert_eq!(event_id, BOTANY_AURA_EVENT_ID);
                assert_eq!(color.as_deref(), Some("#22FF44"));
                assert_eq!(*count, Some(4));
            }
            other => panic!("expected botany aura SpawnParticle, got {other:?}"),
        }
    }

    #[test]
    fn static_points_seed_only_for_supported_zones() {
        let mut app = App::new();
        app.insert_resource(BotanyKindRegistry::default());
        app.insert_resource(PlantStaticPointStore::default());
        app.insert_resource(BotanyVariantRoll::default());
        app.insert_resource(ZoneRegistry {
            zones: vec![
                Zone {
                    name: "spawn".to_string(),
                    dimension: crate::world::dimension::DimensionKind::Overworld,
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
                    dimension: crate::world::dimension::DimensionKind::Overworld,
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
    fn trampled_plant_restores_spirit_qi_but_does_not_drop_item() {
        // 选 HuiYuanZhi（survive=0.35, growth_cost=0.003）在 lingquan_marsh；
        // spirit_qi 精准卡 0.35 —— 既不触发 low_qi wither（严格小于判定），
        // spawn loop 的 target_count 也为 0（0.35 * 1.5 = 0.525 floor = 0），
        // 仅留下 trampled 作为凋零成因，便于单独测量归还。
        let mut app = App::new();
        app.insert_resource(BotanyKindRegistry::default());
        app.insert_resource(PlantStaticPointStore::default());
        app.insert_resource(BotanyVariantRoll::default());
        app.insert_resource(PlantLifecycleClock {
            tick: LIFECYCLE_INTERVAL_TICKS - 1,
        });
        app.insert_resource(ZoneRegistry {
            zones: vec![Zone {
                name: "lingquan_marsh".to_string(),
                dimension: crate::world::dimension::DimensionKind::Overworld,
                bounds: (
                    Position::new([0.0, 0.0, 0.0]).get(),
                    Position::new([1.0, 1.0, 1.0]).get(),
                ),
                spirit_qi: 0.35,
                danger_level: 1,
                active_events: vec![],
                patrol_anchors: vec![],
                blocked_tiles: vec![],
            }],
        });

        app.world_mut().spawn(Plant {
            id: BotanyPlantId::HuiYuanZhi,
            zone_name: "lingquan_marsh".to_string(),
            position: [0.5, 1.0, 0.5],
            planted_at_tick: 0,
            wither_progress: 0,
            source_point: None,
            harvested: false,
            trampled: true,
            variant: PlantVariant::None,
        });

        app.add_systems(Update, run_botany_lifecycle_tick);
        app.update();

        let zone_registry = app.world().resource::<ZoneRegistry>();
        assert!(
            zone_registry.zones[0].spirit_qi > 0.35,
            "trampled plant should restore spirit_qi like natural wither, got {}",
            zone_registry.zones[0].spirit_qi
        );

        let world = app.world_mut();
        let mut plants = world.query::<&Plant>();
        assert_eq!(
            plants.iter(world).count(),
            0,
            "trampled plant should be despawned"
        );
    }

    #[test]
    fn harvested_plant_does_not_restore_spirit_qi() {
        // 对照组：harvested=true 的归途不回补（灵气随玩家离开 zone，plan §2）
        let mut app = App::new();
        app.insert_resource(BotanyKindRegistry::default());
        app.insert_resource(PlantStaticPointStore::default());
        app.insert_resource(BotanyVariantRoll::default());
        app.insert_resource(PlantLifecycleClock {
            tick: LIFECYCLE_INTERVAL_TICKS - 1,
        });
        app.insert_resource(ZoneRegistry {
            zones: vec![Zone {
                name: "lingquan_marsh".to_string(),
                dimension: crate::world::dimension::DimensionKind::Overworld,
                bounds: (
                    Position::new([0.0, 0.0, 0.0]).get(),
                    Position::new([1.0, 1.0, 1.0]).get(),
                ),
                spirit_qi: 0.35,
                danger_level: 1,
                active_events: vec![],
                patrol_anchors: vec![],
                blocked_tiles: vec![],
            }],
        });

        app.world_mut().spawn(Plant {
            id: BotanyPlantId::HuiYuanZhi,
            zone_name: "lingquan_marsh".to_string(),
            position: [0.5, 1.0, 0.5],
            planted_at_tick: 0,
            wither_progress: 0,
            source_point: None,
            harvested: true,
            trampled: false,
            variant: PlantVariant::None,
        });

        app.add_systems(Update, run_botany_lifecycle_tick);
        app.update();

        let zone_registry = app.world().resource::<ZoneRegistry>();
        assert!(
            (zone_registry.zones[0].spirit_qi - 0.35).abs() < 1e-9,
            "harvested plant should not restore spirit_qi, got {}",
            zone_registry.zones[0].spirit_qi
        );
    }

    #[test]
    fn roll_variant_produces_thunder_when_tribulation_active() {
        let zone = Zone {
            name: "qingyun_peaks".to_string(),
            dimension: crate::world::dimension::DimensionKind::Overworld,
            bounds: (
                Position::new([0.0, 0.0, 0.0]).get(),
                Position::new([1.0, 1.0, 1.0]).get(),
            ),
            spirit_qi: 0.5,
            danger_level: 7,
            active_events: vec!["thunder_tribulation".to_string()],
            patrol_anchors: vec![],
            blocked_tiles: vec![],
        };
        let cfg = BotanyVariantRoll { chance_inverse: 1 }; // 强制
        let v = roll_variant_for_zone(&zone, 42, &cfg);
        assert_eq!(v, PlantVariant::Thunder);
    }

    #[test]
    fn roll_variant_produces_tainted_when_spirit_qi_negative() {
        let zone = Zone {
            name: "negative_pocket".to_string(),
            dimension: crate::world::dimension::DimensionKind::Overworld,
            bounds: (
                Position::new([0.0, 0.0, 0.0]).get(),
                Position::new([1.0, 1.0, 1.0]).get(),
            ),
            spirit_qi: -0.3,
            danger_level: 9,
            active_events: vec![],
            patrol_anchors: vec![],
            blocked_tiles: vec![],
        };
        let cfg = BotanyVariantRoll { chance_inverse: 1 };
        assert_eq!(
            roll_variant_for_zone(&zone, 42, &cfg),
            PlantVariant::Tainted
        );
    }

    #[test]
    fn roll_variant_is_none_when_chance_zero() {
        let zone = Zone {
            name: "spawn".to_string(),
            dimension: crate::world::dimension::DimensionKind::Overworld,
            bounds: (
                Position::new([0.0, 0.0, 0.0]).get(),
                Position::new([1.0, 1.0, 1.0]).get(),
            ),
            spirit_qi: -0.5,
            danger_level: 1,
            active_events: vec!["thunder_tribulation".to_string()],
            patrol_anchors: vec![],
            blocked_tiles: vec![],
        };
        let cfg = BotanyVariantRoll { chance_inverse: 0 };
        assert_eq!(roll_variant_for_zone(&zone, 42, &cfg), PlantVariant::None);
    }

    #[test]
    fn roll_variant_is_none_in_neutral_zone() {
        let zone = Zone {
            name: "spawn".to_string(),
            dimension: crate::world::dimension::DimensionKind::Overworld,
            bounds: (
                Position::new([0.0, 0.0, 0.0]).get(),
                Position::new([1.0, 1.0, 1.0]).get(),
            ),
            spirit_qi: 0.6,
            danger_level: 1,
            active_events: vec![],
            patrol_anchors: vec![],
            blocked_tiles: vec![],
        };
        let cfg = BotanyVariantRoll { chance_inverse: 1 };
        assert_eq!(roll_variant_for_zone(&zone, 42, &cfg), PlantVariant::None);
    }

    #[test]
    fn harvested_static_point_unbinds_and_respawns_after_regen() {
        let mut app = App::new();
        app.insert_resource(BotanyKindRegistry::default());
        app.insert_resource(BotanyVariantRoll::default());
        app.insert_resource(PlantLifecycleClock {
            tick: LIFECYCLE_INTERVAL_TICKS - 1,
        });

        let marsh_zone = Zone {
            name: "lingquan_marsh".to_string(),
            dimension: crate::world::dimension::DimensionKind::Overworld,
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
                position: [0.5, 1.0, 0.5],
                planted_at_tick: 0,
                wither_progress: 0,
                source_point: Some(1),
                harvested: true,
                trampled: false,
                variant: PlantVariant::None,
            })
            .id();

        let mut static_points = PlantStaticPointStore::default();
        static_points.upsert(PlantStaticPoint {
            id: 1,
            zone_name: "lingquan_marsh".to_string(),
            position: [0.5, 1.0, 0.5],
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

    #[test]
    fn portal_rift_v2_spawn_ignores_negative_zone_qi_then_wither_when_event_closes() {
        let mut app = App::new();
        app.insert_resource(BotanyKindRegistry::default());
        app.insert_resource(PlantStaticPointStore::default());
        app.insert_resource(BotanyVariantRoll { chance_inverse: 0 });
        app.insert_resource(PlantLifecycleClock {
            tick: LIFECYCLE_INTERVAL_TICKS - 1,
        });
        app.insert_resource(TerrainProviders {
            overworld: crate::world::terrain::TerrainProvider::empty_for_tests(),
            tsy: None,
        });
        app.insert_resource(ZoneRegistry {
            zones: vec![Zone {
                name: "tsy_shallow".to_string(),
                dimension: crate::world::dimension::DimensionKind::Overworld,
                bounds: (
                    Position::new([0.0, 0.0, 0.0]).get(),
                    Position::new([16.0, 96.0, 16.0]).get(),
                ),
                spirit_qi: -0.4,
                danger_level: 8,
                active_events: vec!["portal_rift".to_string()],
                patrol_anchors: vec![],
                blocked_tiles: vec![],
            }],
        });
        app.add_systems(Update, run_botany_lifecycle_tick);

        app.update();

        {
            let world = app.world_mut();
            let mut plants = world.query::<&Plant>();
            let spawned = plants.iter(world).collect::<Vec<_>>();
            assert_eq!(spawned.len(), 1);
            assert_eq!(spawned[0].id, BotanyPlantId::LieYuanTai);
        }
        let after_spawn_qi = app.world().resource::<ZoneRegistry>().zones[0].spirit_qi;

        {
            let mut zones = app.world_mut().resource_mut::<ZoneRegistry>();
            zones.zones[0].active_events.clear();
        }
        {
            let mut clock = app.world_mut().resource_mut::<PlantLifecycleClock>();
            clock.tick = LIFECYCLE_INTERVAL_TICKS * 2 - 1;
        }

        app.update();

        {
            let world = app.world_mut();
            let mut plants = world.query::<&Plant>();
            assert_eq!(plants.iter(world).count(), 0);
        }
        let zone_registry = app.world().resource::<ZoneRegistry>();
        assert!(
            zone_registry.zones[0].spirit_qi > after_spawn_qi,
            "event-closed wither should restore spirit_qi"
        );
    }
}
