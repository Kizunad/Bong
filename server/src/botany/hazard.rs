use valence::prelude::{Commands, DVec3, Entity, EventReader, Position, Query, Res, With};

use super::components::{BotanyAttractsMobsEvent, HarvestSessionStore, Plant};
use super::registry::{BotanyKindRegistry, FaunaKind, HarvestHazard, WoundLevel};
use crate::combat::components::{BodyPart, Wound, WoundKind, Wounds};
use crate::cultivation::components::{ColorKind, ContamSource, Contamination, Cultivation};
use crate::fauna::components::{BeastKind, FaunaTag};
use crate::npc::spawn::spawn_beast_npc_at;
use crate::npc::territory::Territory;
use crate::tools::{has_required_tool, ToolKind};
use crate::world::dimension::{DimensionKind, DimensionLayers, OverworldLayer};
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

pub fn tick_harvest_hazards(
    gameplay_tick: Option<Res<crate::player::gameplay::GameplayTick>>,
    store: Res<HarvestSessionStore>,
    kind_registry: Res<BotanyKindRegistry>,
    plants: Query<&Plant, With<Plant>>,
    positions: Query<(Entity, &Position, &mut Cultivation), With<valence::prelude::Client>>,
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
        let Ok((_, position, mut cultivation)) = positions.get_mut(session.client_entity) else {
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
        cultivation.qi_current = (cultivation.qi_current - drain_per_tick).max(0.0);
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
) {
    for event in events.read() {
        let dimension = zone_registry
            .as_deref()
            .and_then(|registry| registry.find_zone_by_name(event.zone_name.as_str()))
            .map(|zone| zone.dimension)
            .unwrap_or(DimensionKind::Overworld);
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
        for idx in 0..count {
            let spawn_pos = attracted_mob_position(event.target_pos, event_seed(event), idx);
            let entity = spawn_beast_npc_at(
                &mut commands,
                layer,
                event.zone_name.as_str(),
                spawn_pos,
                Territory::new(
                    DVec3::new(
                        event.target_pos[0],
                        event.target_pos[1],
                        event.target_pos[2],
                    ),
                    12.0,
                ),
                0.0,
            );
            commands
                .entity(entity)
                .insert(FaunaTag::new(beast_kind_for_botany(event.mob_kind)));
        }
    }
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
    use crate::botany::registry::BotanyPlantId;

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
    fn attracts_mobs_event_spawns_fauna_tagged_beasts() {
        use crate::npc::lifecycle::NpcArchetype;
        use crate::npc::spawn::NpcMarker;
        use valence::prelude::{App, Update, With};

        let mut app = App::new();
        app.add_event::<BotanyAttractsMobsEvent>();
        app.add_systems(Update, spawn_attracted_mobs_from_harvest);
        app.world_mut().spawn(OverworldLayer);
        let client = app.world_mut().spawn_empty().id();

        app.world_mut().send_event(BotanyAttractsMobsEvent {
            client_entity: client,
            plant_kind: BotanyPlantId::BaiYanPeng,
            zone_name: "north_wastes".to_string(),
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
}
