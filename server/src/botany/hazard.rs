use valence::prelude::{
    Client, Commands, DVec3, Entity, EventReader, Events, Position, Query, Res, ResMut, With,
};

use super::components::{BotanyAttractsMobsEvent, HarvestSessionStore, Plant};
use super::registry::{BotanyKindRegistry, FaunaKind, HarvestHazard, WoundLevel};
use crate::combat::components::{BodyPart, Wound, WoundKind, Wounds};
use crate::cultivation::components::{ColorKind, ContamSource, Contamination, Cultivation};
use crate::cultivation::death_hooks::release_qi_amount_to_zone;
use crate::cultivation::life_record::LifeRecord;
use crate::fauna::components::{BeastKind, FaunaTag};
use crate::npc::spawn::spawn_beast_npc_at;
use crate::npc::territory::Territory;
use crate::qi_physics::constants::QI_EPSILON;
use crate::qi_physics::ledger::QiTransfer;
use crate::tools::{has_required_tool, ToolKind};
use crate::world::dimension::{CurrentDimension, DimensionKind, DimensionLayers, OverworldLayer};
use crate::world::era::WorldEraState;
use crate::world::mob_spawn::{era_beast_spawn_gate, spawn_natural_mob_at, NaturalMobKind};
use crate::world::zone::ZoneRegistry;

pub fn hazard_hints_for_kind(
    kind_id: super::registry::BotanyPlantId,
    registry: &BotanyKindRegistry,
) -> Vec<String> {
    let Some(kind) = registry.get(kind_id) else {
        return Vec::new();
    };
    let Some(spec) = kind.v2_spec() else {
        return Vec::new();
    };
    spec.harvest_hazards
        .iter()
        .map(|hazard| match hazard {
            HarvestHazard::QiDrainOnApproach { drain_per_sec, .. } => {
                format!("靠近 -{drain_per_sec:.1} 真元/s 叠加")
            }
            HarvestHazard::WoundOnBareHand {
                required_tool: None,
                ..
            } => "无工具采空 100%".to_string(),
            HarvestHazard::WoundOnBareHand {
                required_tool: Some(tool),
                ..
            } => {
                format!("需工具 {}，否则受伤", tool.display_name())
            }
            HarvestHazard::DispersalOnFail { dispersal_chance } => {
                format!("失败散气 {:.0}%", dispersal_chance * 100.0)
            }
            HarvestHazard::ResonanceVision { duration_secs, .. } => {
                format!("采成触发怨念幻视 {duration_secs}s")
            }
            HarvestHazard::SeasonRequired { .. } => "相位未合则反吸".to_string(),
            HarvestHazard::AttractsMobs {
                mob_kind,
                min_count,
                max_count,
            } => {
                format!("可能引来 {mob_kind:?} {min_count}-{max_count} 只")
            }
        })
        .collect()
}

#[allow(clippy::type_complexity)]
pub fn tick_harvest_hazards(
    gameplay_tick: Option<Res<crate::player::gameplay::GameplayTick>>,
    store: Res<HarvestSessionStore>,
    kind_registry: Res<BotanyKindRegistry>,
    plants: Query<&Plant, With<Plant>>,
    positions: Query<
        (
            Entity,
            &Position,
            Option<&CurrentDimension>,
            Option<&LifeRecord>,
            &mut Cultivation,
        ),
        With<Client>,
    >,
    mut zones: Option<ResMut<ZoneRegistry>>,
    mut qi_transfer_events: Option<ResMut<Events<QiTransfer>>>,
) {
    let Some(_gameplay_tick) = gameplay_tick else {
        return;
    };

    let mut positions = positions;
    for session in store.iter() {
        let Some(kind) = kind_registry.get(session.target_plant) else {
            continue;
        };
        let Some(spec) = kind.v2_spec() else {
            continue;
        };
        let Some((radius_blocks, drain_per_sec)) = spec.harvest_hazards.iter().find_map(|hazard| {
            if let HarvestHazard::QiDrainOnApproach {
                radius_blocks,
                drain_per_sec,
            } = hazard
            {
                Some((*radius_blocks, *drain_per_sec))
            } else {
                None
            }
        }) else {
            continue;
        };
        let Some(target_pos) = session
            .target_entity
            .and_then(|entity| plants.get(entity).ok().map(|plant| plant.position))
        else {
            continue;
        };
        let Ok((entity, position, current_dimension, life_record, mut cultivation)) =
            positions.get_mut(session.client_entity)
        else {
            continue;
        };
        let player_pos = position.get();
        let dx = player_pos.x - target_pos[0];
        let dy = player_pos.y - target_pos[1];
        let dz = player_pos.z - target_pos[2];
        let radius = f64::from(radius_blocks);
        if dx * dx + dy * dy + dz * dz > radius * radius {
            continue;
        }
        let drain_per_tick = f64::from(drain_per_sec) / 20.0;
        let actual_drain = drain_per_tick.min(cultivation.qi_current.max(0.0));
        if actual_drain <= QI_EPSILON {
            continue;
        }

        cultivation.qi_current = (cultivation.qi_current - actual_drain).max(0.0);
        release_qi_amount_to_zone(
            entity,
            actual_drain,
            Some(position),
            current_dimension,
            life_record,
            zones.as_deref_mut(),
            qi_transfer_events.as_deref_mut(),
            "botany_harvest_hazard",
        );
    }
}

pub fn apply_completion_hazards(
    kind_id: super::registry::BotanyPlantId,
    registry: &BotanyKindRegistry,
    cultivation: Option<&mut Cultivation>,
    contamination: Option<&mut Contamination>,
    wounds: Option<&mut Wounds>,
    actual_tool: Option<ToolKind>,
    now_tick: u64,
) {
    let Some(kind) = registry.get(kind_id) else {
        return;
    };
    let Some(spec) = kind.v2_spec() else {
        return;
    };
    let mut cultivation = cultivation;
    let mut contamination = contamination;
    let mut wounds = wounds;
    for hazard in spec.harvest_hazards {
        match hazard {
            HarvestHazard::ResonanceVision { composure_loss, .. } => {
                if let Some(cultivation) = cultivation.as_deref_mut() {
                    cultivation.composure =
                        (cultivation.composure - f64::from(*composure_loss)).max(0.0);
                }
            }
            HarvestHazard::WoundOnBareHand {
                wound,
                required_tool,
                ..
            } if !has_required_tool(actual_tool, *required_tool) => {
                if let Some(wounds) = wounds.as_deref_mut() {
                    wounds.entries.push(Wound {
                        location: BodyPart::ArmR,
                        kind: wound_kind(*wound),
                        severity: wound_severity(*wound),
                        bleeding_per_sec: 0.0,
                        created_at_tick: now_tick,
                        inflicted_by: Some("botany_v2_hazard".to_string()),
                    });
                }
                if let Some(contamination) = contamination.as_deref_mut() {
                    contamination.entries.push(ContamSource {
                        amount: contamination_amount(*wound),
                        color: ColorKind::Insidious,
                        meridian_id: None,
                        attacker_id: Some("botany_v2_hazard".to_string()),
                        introduced_at: now_tick,
                    });
                }
            }
            _ => {}
        }
    }
}

pub fn attracts_mobs_hazards_for_kind(
    kind_id: super::registry::BotanyPlantId,
    registry: &BotanyKindRegistry,
) -> Vec<(FaunaKind, u8, u8)> {
    let Some(kind) = registry.get(kind_id) else {
        return Vec::new();
    };
    let Some(spec) = kind.v2_spec() else {
        return Vec::new();
    };
    spec.harvest_hazards
        .iter()
        .filter_map(|hazard| match hazard {
            HarvestHazard::AttractsMobs {
                mob_kind,
                min_count,
                max_count,
            } => Some((*mob_kind, *min_count, *max_count)),
            _ => None,
        })
        .collect()
}

pub fn spawn_attracted_mobs_from_harvest(
    mut commands: Commands,
    mut events: EventReader<BotanyAttractsMobsEvent>,
    dimension_layers: Option<Res<DimensionLayers>>,
    overworld_layers: Query<Entity, With<OverworldLayer>>,
    zone_registry: Option<Res<ZoneRegistry>>,
    // plan-era-state-v1 M1 — 时代兽密度门控：灾劫×1.5 / 演绎×0.8 / Unknown×1.0。
    world_era: Option<Res<WorldEraState>>,
) {
    let beast_density_mul = world_era
        .as_deref()
        .map(|e| e.current_modifiers().beast_density_mul)
        .unwrap_or(1.0);

    for event in events.read() {
        let Some(dimension) = harvest_zone_dimension(event, zone_registry.as_deref()) else {
            continue;
        };
        let Some(layer) = dimension_layers
            .as_deref()
            .map(|layers| layers.entity_for(dimension))
            .or_else(|| {
                (dimension == DimensionKind::Overworld)
                    .then(|| overworld_layers.iter().next())
                    .flatten()
            })
        else {
            continue;
        };

        let count = attracted_mob_count(event.min_count, event.max_count, event_seed(event));
        let patrol_center = DVec3::new(
            event.target_pos[0],
            event.target_pos[1],
            event.target_pos[2],
        );
        for idx in 0..count {
            // plan-era-state-v1 M1 — 时代兽密度门控：每只 spawn 独立采样，保证 Deduction 时代
            // 不会无脑抑制所有 spawn（概率降低，不是截断）。
            let spawn_seed = event_seed(event).wrapping_add(idx as u64);
            if !era_beast_spawn_gate(beast_density_mul, spawn_seed) {
                tracing::debug!(
                    "[bong][botany] era_beast_spawn_gate blocked spawn idx={} zone={} beast_density_mul={:.2}",
                    idx, event.zone_name, beast_density_mul
                );
                continue;
            }

            let spawn_pos = attracted_mob_position(event.target_pos, event_seed(event), idx);
            // 拟态灰烬蛛（FaunaKind::MimicSpider）走 spawn_natural_mob_at，
            // 附带完整 MimicSpiderBlackboard / SpiderDisguiseState 组件。
            // 其他 FaunaKind 回退到通用 spawn_beast_npc_at 路径。
            if event.mob_kind == FaunaKind::MimicSpider {
                spawn_natural_mob_at(
                    &mut commands,
                    layer,
                    NaturalMobKind::AshSpider,
                    event.zone_name.as_str(),
                    spawn_pos,
                    patrol_center,
                );
            } else {
                let entity = spawn_beast_npc_at(
                    &mut commands,
                    layer,
                    event.zone_name.as_str(),
                    spawn_pos,
                    Territory::new(patrol_center, 12.0),
                    0.0,
                );
                commands
                    .entity(entity)
                    .insert(FaunaTag::new(beast_kind_for_botany(event.mob_kind)));
            }
        }
    }
}

fn harvest_zone_dimension(
    event: &BotanyAttractsMobsEvent,
    zone_registry: Option<&ZoneRegistry>,
) -> Option<DimensionKind> {
    let Some(registry) = zone_registry else {
        tracing::warn!(
            "[bong][botany] drop AttractsMobs spawn for `{}`: ZoneRegistry missing",
            event.zone_name
        );
        return None;
    };
    let Some(zone) = registry.find_zone_by_name(event.zone_name.as_str()) else {
        tracing::warn!(
            "[bong][botany] drop AttractsMobs spawn for `{}`: zone not found",
            event.zone_name
        );
        return None;
    };
    Some(zone.dimension)
}

fn beast_kind_for_botany(kind: FaunaKind) -> BeastKind {
    match kind {
        FaunaKind::SpiritMice => BeastKind::Rat,
        FaunaKind::MimicSpider => BeastKind::Spider,
    }
}

fn attracted_mob_count(min_count: u8, max_count: u8, seed: u64) -> u8 {
    let min = min_count.max(1);
    let max = max_count.max(min);
    let span = u64::from(max - min + 1);
    min + (splitmix(seed) % span) as u8
}

fn attracted_mob_position(target_pos: [f64; 3], seed: u64, idx: u8) -> DVec3 {
    let offset_seed = splitmix(seed ^ u64::from(idx).wrapping_mul(0x9E37_79B9));
    let dx = ((offset_seed & 0xFF) as f64 / 255.0 - 0.5) * 4.0;
    let dz = (((offset_seed >> 8) & 0xFF) as f64 / 255.0 - 0.5) * 4.0;
    DVec3::new(target_pos[0] + dx, target_pos[1], target_pos[2] + dz)
}

fn event_seed(event: &BotanyAttractsMobsEvent) -> u64 {
    event.issued_at_tick
        ^ u64::from(event.min_count).rotate_left(7)
        ^ u64::from(event.max_count).rotate_left(13)
        ^ event.client_entity.to_bits().rotate_left(23)
        ^ event.plant_kind.as_str().bytes().fold(0_u64, |acc, byte| {
            acc.wrapping_mul(0x100_0000_01B3)
                .wrapping_add(u64::from(byte))
        })
}

fn wound_kind(wound: WoundLevel) -> WoundKind {
    match wound {
        WoundLevel::Abrasion => WoundKind::Blunt,
        WoundLevel::Laceration => WoundKind::Cut,
        WoundLevel::Fracture => WoundKind::Concussion,
    }
}

fn wound_severity(wound: WoundLevel) -> f32 {
    match wound {
        WoundLevel::Abrasion => 0.12,
        WoundLevel::Laceration => 0.28,
        WoundLevel::Fracture => 0.45,
    }
}

fn contamination_amount(wound: WoundLevel) -> f64 {
    match wound {
        WoundLevel::Abrasion => 0.1,
        WoundLevel::Laceration => 0.2,
        WoundLevel::Fracture => 0.3,
    }
}

pub fn failure_dispersal_chance(
    kind_id: super::registry::BotanyPlantId,
    registry: &BotanyKindRegistry,
) -> f32 {
    let Some(kind) = registry.get(kind_id) else {
        return 0.0;
    };
    let Some(spec) = kind.v2_spec() else {
        return 0.0;
    };
    let mut chance = 0.0_f32;
    for hazard in spec.harvest_hazards {
        match hazard {
            HarvestHazard::DispersalOnFail { dispersal_chance } => {
                chance = chance.max(*dispersal_chance);
            }
            HarvestHazard::WoundOnBareHand {
                required_tool: None,
                ..
            } => {
                chance = chance.max(1.0);
            }
            _ => {}
        }
    }
    chance
}

pub fn should_disperse_on_fail(seed: u64, chance: f32) -> bool {
    if chance <= 0.0 {
        return false;
    }
    if chance >= 1.0 {
        return true;
    }
    let bucket = splitmix(seed) % 10_000;
    bucket < (chance * 10_000.0).round() as u64
}

fn splitmix(seed: u64) -> u64 {
    let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::botany::components::{BotanyHarvestMode, BotanyPhase, HarvestSession};
    use crate::botany::registry::{BotanyPlantId, PlantVariant};
    use crate::cultivation::life_record::LifeRecord;
    use crate::player::gameplay::GameplayTick;
    use crate::player::state::canonical_player_id;
    use crate::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;
    use crate::qi_physics::ledger::QiTransferReason;
    use crate::world::dimension::{CurrentDimension, DimensionKind};
    use valence::prelude::{App, Update};
    use valence::testing::create_mock_client;

    fn make_hazard_plant(pos: [f64; 3]) -> Plant {
        Plant {
            id: BotanyPlantId::FuYuanJue,
            zone_name: "spawn".to_string(),
            position: pos,
            planted_at_tick: 0,
            wither_progress: 0,
            source_point: None,
            harvested: false,
            trampled: false,
            variant: PlantVariant::None,
        }
    }

    fn make_cultivation(qi_current: f64) -> Cultivation {
        Cultivation {
            qi_current,
            qi_max: 100.0,
            ..Default::default()
        }
    }

    fn make_hazard_app() -> App {
        let mut app = App::new();
        app.add_event::<QiTransfer>();
        app.add_systems(Update, tick_harvest_hazards);
        app.insert_resource(GameplayTick::default());
        app.insert_resource(HarvestSessionStore::default());
        app.insert_resource(BotanyKindRegistry::default());

        let mut zones = ZoneRegistry::fallback();
        zones.find_zone_mut("spawn").unwrap().spirit_qi = 0.0;
        app.insert_resource(zones);
        app
    }

    fn spawn_harvest_client(app: &mut App, name: &str, pos: [f64; 3], qi_current: f64) -> Entity {
        let (client_bundle, _helper) = create_mock_client(name);
        let client = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(client).insert((
            Position::new(pos),
            CurrentDimension(DimensionKind::Overworld),
            make_cultivation(qi_current),
            LifeRecord::new(canonical_player_id(name)),
        ));
        client
    }

    fn start_hazard_session(app: &mut App, client: Entity, plant: Entity, player_name: &str) {
        app.world_mut()
            .resource_mut::<HarvestSessionStore>()
            .upsert_session(HarvestSession {
                player_id: canonical_player_id(player_name),
                client_entity: client,
                target_entity: Some(plant),
                target_plant: BotanyPlantId::FuYuanJue,
                mode: BotanyHarvestMode::Manual,
                started_at_tick: 0,
                duration_ticks: 20,
                phase: BotanyPhase::InProgress,
                last_progress: 0.0,
                origin_position: [8.0, 64.0, 8.0],
            });
    }

    #[test]
    fn wound_stub_becomes_full_dispersal_chance() {
        let registry = BotanyKindRegistry::default();
        assert_eq!(
            failure_dispersal_chance(BotanyPlantId::JiaoMaiTeng, &registry),
            0.0
        );
    }

    #[test]
    fn fu_yuan_jue_reports_qi_drain_hint() {
        let registry = BotanyKindRegistry::default();
        let hints = hazard_hints_for_kind(BotanyPlantId::FuYuanJue, &registry);
        assert!(hints.iter().any(|hint| hint.contains("-0.4 真元/s")));
    }

    #[test]
    fn qi_drain_hazard_releases_actual_drain_to_zone() {
        let mut app = make_hazard_app();
        let plant = app
            .world_mut()
            .spawn(make_hazard_plant([8.0, 64.0, 8.0]))
            .id();
        let player = spawn_harvest_client(&mut app, "Azure", [8.0, 64.0, 8.0], 10.0);
        start_hazard_session(&mut app, player, plant, "Azure");

        app.update();

        let expected_drain = 0.4_f64 / 20.0;
        let cultivation = app.world().entity(player).get::<Cultivation>().unwrap();
        assert!(
            (cultivation.qi_current - (10.0 - expected_drain)).abs() < 1e-9,
            "采集靠近扣真元应只扣每 tick 实际值，期望 {}，实际 {}",
            10.0 - expected_drain,
            cultivation.qi_current
        );

        let zone_qi = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name("spawn")
            .unwrap()
            .spirit_qi
            * QI_ZONE_UNIT_CAPACITY;
        assert!(
            (zone_qi - expected_drain).abs() < 1e-9,
            "被扣真元必须回灌 zone，期望 zone 增量 {expected_drain}，实际 {zone_qi}"
        );

        let transfers: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<QiTransfer>>()
            .drain()
            .collect();
        assert_eq!(transfers.len(), 1);
        assert_eq!(transfers[0].reason, QiTransferReason::ReleaseToZone);
        assert!(
            (transfers[0].amount - expected_drain).abs() < 1e-9,
            "QiTransfer amount 应等于实际扣除量 {expected_drain}，实际 {}",
            transfers[0].amount
        );
    }

    #[test]
    fn qi_drain_hazard_clamps_release_to_remaining_qi() {
        let mut app = make_hazard_app();
        let plant = app
            .world_mut()
            .spawn(make_hazard_plant([8.0, 64.0, 8.0]))
            .id();
        let player = spawn_harvest_client(&mut app, "Bao", [8.0, 64.0, 8.0], 0.01);
        start_hazard_session(&mut app, player, plant, "Bao");

        app.update();

        let cultivation = app.world().entity(player).get::<Cultivation>().unwrap();
        assert!(
            cultivation.qi_current.abs() < 1e-9,
            "真元不足时应扣到 0，不应变负或释放超过剩余量，实际 {}",
            cultivation.qi_current
        );

        let transfers: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<QiTransfer>>()
            .drain()
            .collect();
        assert_eq!(transfers.len(), 1);
        assert!(
            (transfers[0].amount - 0.01).abs() < 1e-9,
            "QiTransfer amount 应 clamp 到剩余真元 0.01，实际 {}",
            transfers[0].amount
        );
    }

    #[test]
    fn attracts_mobs_maps_botany_kinds_to_fauna_beasts() {
        assert_eq!(beast_kind_for_botany(FaunaKind::SpiritMice), BeastKind::Rat);
        assert_eq!(
            beast_kind_for_botany(FaunaKind::MimicSpider),
            BeastKind::Spider
        );
    }

    #[test]
    fn bai_yan_peng_exposes_attracts_mobs_hazard() {
        let registry = BotanyKindRegistry::default();
        let hazards = attracts_mobs_hazards_for_kind(BotanyPlantId::BaiYanPeng, &registry);
        assert_eq!(hazards, vec![(FaunaKind::SpiritMice, 2, 5)]);
    }

    #[test]
    fn plant_without_attracts_mobs_exposes_no_mob_hazard() {
        let registry = BotanyKindRegistry::default();
        let hazards = attracts_mobs_hazards_for_kind(BotanyPlantId::FuYuanJue, &registry);
        assert!(hazards.is_empty());
    }

    #[test]
    fn attracts_mobs_event_spawns_fauna_tagged_beasts() {
        use crate::npc::lifecycle::NpcArchetype;
        use crate::npc::spawn::NpcMarker;
        use crate::world::zone::ZoneRegistry;
        use valence::prelude::{App, Update, With};

        let mut app = App::new();
        app.add_event::<BotanyAttractsMobsEvent>();
        app.add_systems(Update, spawn_attracted_mobs_from_harvest);
        app.insert_resource(ZoneRegistry::fallback());
        app.world_mut().spawn(OverworldLayer);
        let client = app.world_mut().spawn_empty().id();

        app.world_mut().send_event(BotanyAttractsMobsEvent {
            client_entity: client,
            plant_kind: BotanyPlantId::BaiYanPeng,
            zone_name: "spawn".to_string(),
            target_pos: [12.0, 66.0, 12.0],
            mob_kind: FaunaKind::SpiritMice,
            min_count: 2,
            max_count: 2,
            issued_at_tick: 99,
        });
        app.update();

        let world = app.world_mut();
        let mut query = world.query_filtered::<(&FaunaTag, &NpcArchetype), With<NpcMarker>>();
        let spawned = query.iter(world).collect::<Vec<_>>();
        assert_eq!(spawned.len(), 2);
        assert!(spawned
            .iter()
            .all(|(tag, archetype)| tag.beast_kind == BeastKind::Rat
                && **archetype == NpcArchetype::Beast));
    }

    #[test]
    fn attracts_mobs_event_with_unknown_zone_spawns_nothing() {
        use crate::npc::spawn::NpcMarker;
        use crate::world::zone::ZoneRegistry;
        use valence::prelude::{App, Update, With};

        let mut app = App::new();
        app.add_event::<BotanyAttractsMobsEvent>();
        app.add_systems(Update, spawn_attracted_mobs_from_harvest);
        app.insert_resource(ZoneRegistry::fallback());
        app.world_mut().spawn(OverworldLayer);
        let client = app.world_mut().spawn_empty().id();

        app.world_mut().send_event(BotanyAttractsMobsEvent {
            client_entity: client,
            plant_kind: BotanyPlantId::BaiYanPeng,
            zone_name: "missing_zone".to_string(),
            target_pos: [12.0, 66.0, 12.0],
            mob_kind: FaunaKind::SpiritMice,
            min_count: 2,
            max_count: 2,
            issued_at_tick: 99,
        });
        app.update();

        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
        assert_eq!(query.iter(world).count(), 0);
    }
}
