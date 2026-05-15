use std::collections::HashMap;

use serde_json::json;
use valence::prelude::{
    bevy_ecs, Commands, Component, DVec3, Entity, Event, EventReader, EventWriter, Position, Query,
    Res, ResMut, Resource, With,
};

use crate::cultivation::tick::CultivationClock;
use crate::fauna::components::FaunaTag;
use crate::network::audio_event_emit::{AudioRecipient, PlaySoundRecipeRequest};
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::npc::lifecycle::NpcArchetype;
use crate::npc::lod::NpcLodTier;
use crate::npc::navigator::Navigator;
use crate::npc::spawn::NpcMarker;
use crate::schema::agent_command::Command;
use crate::schema::common::CommandType;
use crate::schema::vfx_event::VfxEventPayloadV1;
use crate::world::events::{ActiveEventsResource, EVENT_BEAST_TIDE};
use crate::world::zone::{Zone, ZoneRegistry};

pub const MIGRATION_THRESHOLD: f64 = 0.05;
pub const MIGRATION_SUSTAIN_TICKS: u64 = 600;
pub const MIGRATION_MIN_DURATION_TICKS: u32 = 6_000;
pub const MIGRATION_MAX_DURATION_TICKS: u32 = 12_000;
pub const MIGRATION_VISUAL_EVENT_ID: &str = "bong:migration_visual";
pub const MIGRATION_RUMBLE_RECIPE_ID: &str = "beast_migration_rumble";
pub const MIGRATION_BEAST_TIDE_THRESHOLD: usize = 10;
const MIGRATION_VFX_DURATION_TICKS: u16 = 200;
const MIGRATION_RUMBLE_RADIUS_BLOCKS: f64 = 100.0;
const MIGRATION_NEAR_STEP_BLOCKS: f64 = 0.6;
const MIGRATION_FAR_STEP_BLOCKS: f64 = 5.0;
const MIGRATION_REACH_DISTANCE: f64 = 2.0;

#[derive(Debug, Clone, PartialEq, Event)]
pub struct ZoneQiCriticalEvent {
    pub zone_id: String,
    pub spirit_qi: f64,
    pub neighbors: Vec<(String, f64)>,
}

#[derive(Debug, Clone, PartialEq, Event)]
pub struct MigrationEvent {
    pub zone_id: String,
    pub target_zone: String,
    pub direction: [f64; 3],
    pub duration_ticks: u32,
    pub started_at_tick: u64,
}

#[derive(Debug, Clone, Component, PartialEq)]
pub struct MigrationTarget {
    pub origin_zone: String,
    pub target_zone: String,
    pub target_pos: DVec3,
    pub speed_multiplier: f64,
    pub started_at_tick: u64,
}

#[derive(Debug, Clone, Default, Resource)]
pub struct FaunaMigrationState {
    critical_ticks_by_zone: HashMap<String, u64>,
    active_until_by_zone: HashMap<String, u64>,
    last_tick: Option<u64>,
}

type MigrationTriggerNpcQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Position,
        Option<&'static FaunaTag>,
        Option<&'static NpcArchetype>,
    ),
    With<NpcMarker>,
>;

type MigrationMoveQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut Position,
        &'static MigrationTarget,
        Option<&'static NpcLodTier>,
        Option<&'static mut Navigator>,
    ),
    With<NpcMarker>,
>;

pub fn fauna_migration_system(
    zones: Option<Res<ZoneRegistry>>,
    clock: Option<Res<CultivationClock>>,
    mut state: ResMut<FaunaMigrationState>,
    mut critical_events: EventWriter<ZoneQiCriticalEvent>,
    mut migration_events: EventWriter<MigrationEvent>,
    mut vfx_events: EventWriter<VfxEventRequest>,
    mut audio_events: EventWriter<PlaySoundRecipeRequest>,
) {
    let Some(zones) = zones else {
        state.critical_ticks_by_zone.clear();
        state.last_tick = None;
        return;
    };
    let now = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();
    let elapsed = state.elapsed_ticks(now);

    for zone in &zones.zones {
        if zone.spirit_qi >= MIGRATION_THRESHOLD {
            state.critical_ticks_by_zone.remove(zone.name.as_str());
            continue;
        }

        let active_until = state
            .active_until_by_zone
            .get(zone.name.as_str())
            .copied()
            .unwrap_or_default();
        if now < active_until {
            continue;
        }

        let low_ticks = state
            .critical_ticks_by_zone
            .entry(zone.name.clone())
            .or_default();
        *low_ticks = low_ticks.saturating_add(elapsed);
        if *low_ticks < MIGRATION_SUSTAIN_TICKS {
            continue;
        }

        let Some(target_zone) = select_migration_target_zone(zone, &zones.zones) else {
            continue;
        };
        let duration = migration_duration_ticks(zone);
        state
            .active_until_by_zone
            .insert(zone.name.clone(), now.saturating_add(duration as u64));
        state.critical_ticks_by_zone.remove(zone.name.as_str());

        let critical_event = ZoneQiCriticalEvent {
            zone_id: zone.name.clone(),
            spirit_qi: zone.spirit_qi,
            neighbors: migration_neighbors(zone, &zones.zones),
        };
        let migration_event = MigrationEvent {
            zone_id: zone.name.clone(),
            target_zone: target_zone.name.clone(),
            direction: refuge_direction(zone, target_zone),
            duration_ticks: duration,
            started_at_tick: now,
        };

        critical_events.send(critical_event);
        migration_events.send(migration_event.clone());
        vfx_events.send(migration_vfx_request(zone, &migration_event));
        audio_events.send(migration_rumble_request(zone));
    }
}

pub fn migration_trigger_system(
    mut commands: Commands,
    clock: Option<Res<CultivationClock>>,
    mut critical_events: EventReader<ZoneQiCriticalEvent>,
    zone_registry: Option<Res<ZoneRegistry>>,
    npcs: MigrationTriggerNpcQuery<'_, '_>,
) {
    let Some(zone_registry) = zone_registry else {
        return;
    };

    for event in critical_events.read() {
        let Some(source_zone) = zone_registry.find_zone_by_name(event.zone_id.as_str()) else {
            continue;
        };
        let Some(target_zone) = event
            .neighbors
            .iter()
            .max_by(|left, right| left.1.total_cmp(&right.1))
            .and_then(|(zone_id, _)| zone_registry.find_zone_by_name(zone_id.as_str()))
        else {
            continue;
        };
        let started_at_tick = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();
        for (entity, position, fauna_tag, archetype) in &npcs {
            if !source_zone.contains(position.get()) {
                continue;
            }
            let Some(speed_multiplier) = migration_speed_multiplier(fauna_tag, archetype) else {
                continue;
            };
            commands.entity(entity).insert(MigrationTarget {
                origin_zone: event.zone_id.clone(),
                target_zone: target_zone.name.clone(),
                target_pos: target_zone.center(),
                speed_multiplier,
                started_at_tick,
            });
        }
    }
}

pub fn migration_move_system(
    mut commands: Commands,
    clock: Option<Res<CultivationClock>>,
    mut migrating: MigrationMoveQuery<'_, '_>,
) {
    let now = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();
    for (entity, mut position, target, lod_tier, navigator) in &mut migrating {
        let current = position.get();
        if current.distance(target.target_pos) <= MIGRATION_REACH_DISTANCE {
            commands.entity(entity).remove::<MigrationTarget>();
            continue;
        }

        match lod_tier.copied().unwrap_or_default() {
            NpcLodTier::Dormant => {
                position.set(target.target_pos);
            }
            NpcLodTier::Far => {
                if now % 1_200 == 0 {
                    position.set(step_toward_xz_preserving_y(
                        current,
                        target.target_pos,
                        MIGRATION_FAR_STEP_BLOCKS,
                    ));
                }
            }
            NpcLodTier::Near => {
                if let Some(mut navigator) = navigator {
                    navigator.set_goal(target.target_pos, target.speed_multiplier);
                }
            }
        }
    }
}

pub fn migration_to_beast_tide_system(
    mut active_events: ResMut<ActiveEventsResource>,
    mut zone_registry: Option<ResMut<ZoneRegistry>>,
    migrating_fauna: Query<(&Position, &MigrationTarget, &FaunaTag), With<NpcMarker>>,
) {
    let Some(zone_registry) = zone_registry.as_deref_mut() else {
        return;
    };

    let mut arrivals_by_target: HashMap<String, usize> = HashMap::new();
    for (position, target, _tag) in &migrating_fauna {
        let Some(target_zone) = zone_registry.find_zone_by_name(target.target_zone.as_str()) else {
            continue;
        };
        if target_zone.contains(position.get()) {
            *arrivals_by_target
                .entry(target.target_zone.clone())
                .or_default() += 1;
        }
    }

    for (target_zone, count) in arrivals_by_target {
        if count < MIGRATION_BEAST_TIDE_THRESHOLD
            || active_events.contains(target_zone.as_str(), EVENT_BEAST_TIDE)
        {
            continue;
        }
        let command = migration_beast_tide_command(target_zone.as_str(), count);
        active_events.enqueue_from_spawn_command_with_karma(
            &command,
            Some(&mut *zone_registry),
            None,
            None,
        );
    }
}

impl FaunaMigrationState {
    fn elapsed_ticks(&mut self, now: u64) -> u64 {
        let elapsed = self
            .last_tick
            .map(|last_tick| now.saturating_sub(last_tick).max(1))
            .unwrap_or(1);
        self.last_tick = Some(now);
        elapsed
    }
}

fn migration_speed_multiplier(
    fauna_tag: Option<&FaunaTag>,
    archetype: Option<&NpcArchetype>,
) -> Option<f64> {
    if fauna_tag.is_some() || archetype == Some(&NpcArchetype::Beast) {
        Some(1.5)
    } else if archetype.is_some() {
        Some(1.2)
    } else {
        None
    }
}

fn migration_beast_tide_command(target_zone: &str, beast_count: usize) -> Command {
    Command {
        command_type: CommandType::SpawnEvent,
        target: target_zone.to_string(),
        params: HashMap::from([
            ("event".to_string(), json!(EVENT_BEAST_TIDE)),
            ("tide_kind".to_string(), json!("wandering")),
            (
                "intensity".to_string(),
                json!((beast_count as f64 / 20.0).clamp(0.5, 1.0)),
            ),
            ("duration_ticks".to_string(), json!(24_000_u64)),
            ("reason".to_string(), json!("migration_arrival")),
        ]),
    }
}

fn migration_vfx_request(zone: &Zone, event: &MigrationEvent) -> VfxEventRequest {
    let center = zone.center();
    VfxEventRequest::new(
        center,
        VfxEventPayloadV1::SpawnParticle {
            event_id: MIGRATION_VISUAL_EVENT_ID.to_string(),
            origin: [center.x, center.y, center.z],
            direction: Some(event.direction),
            color: Some("#B08A5A".to_string()),
            strength: Some((1.0 - zone.spirit_qi).clamp(0.20, 1.0) as f32),
            count: Some(migration_visual_count(zone)),
            duration_ticks: Some(MIGRATION_VFX_DURATION_TICKS),
        },
    )
}

fn migration_rumble_request(zone: &Zone) -> PlaySoundRecipeRequest {
    let center = zone.center();
    PlaySoundRecipeRequest {
        recipe_id: MIGRATION_RUMBLE_RECIPE_ID.to_string(),
        instance_id: 0,
        pos: Some([
            center.x.floor() as i32,
            center.y.floor() as i32,
            center.z.floor() as i32,
        ]),
        flag: None,
        volume_mul: (0.75 + (1.0 - zone.spirit_qi).clamp(0.0, 1.0) * 0.35) as f32,
        pitch_shift: -0.15,
        recipient: AudioRecipient::Radius {
            origin: center,
            radius: MIGRATION_RUMBLE_RADIUS_BLOCKS,
        },
    }
}

fn migration_visual_count(zone: &Zone) -> u16 {
    let signal = ((1.0 - zone.spirit_qi).clamp(0.0, 1.0) * 64.0).round() as u16;
    signal.clamp(8, crate::schema::vfx_event::VFX_PARTICLE_COUNT_MAX)
}

fn migration_duration_ticks(zone: &Zone) -> u32 {
    let (min, max) = zone.bounds;
    let area = ((max.x - min.x).abs() * (max.z - min.z).abs()).max(1.0);
    let scaled = MIGRATION_MIN_DURATION_TICKS as f64 + area.sqrt() * 20.0;
    scaled.round().clamp(
        MIGRATION_MIN_DURATION_TICKS as f64,
        MIGRATION_MAX_DURATION_TICKS as f64,
    ) as u32
}

fn select_migration_target_zone<'a>(source: &Zone, zones: &'a [Zone]) -> Option<&'a Zone> {
    zones
        .iter()
        .filter(|zone| zone.name != source.name && zone.spirit_qi > source.spirit_qi)
        .max_by(|left, right| left.spirit_qi.total_cmp(&right.spirit_qi))
}

fn migration_neighbors(source: &Zone, zones: &[Zone]) -> Vec<(String, f64)> {
    zones
        .iter()
        .filter(|zone| zone.name != source.name)
        .map(|zone| (zone.name.clone(), zone.spirit_qi))
        .collect()
}

fn refuge_direction(source: &Zone, target: &Zone) -> [f64; 3] {
    let source_center = source.center();
    let target_center = target.center();
    let vector = target_center - source_center;
    let horizontal_len = (vector.x * vector.x + vector.z * vector.z).sqrt();
    if horizontal_len <= 1e-6 {
        return [1.0, 0.0, 0.0];
    }
    [vector.x / horizontal_len, 0.0, vector.z / horizontal_len]
}

fn step_toward(current: DVec3, target: DVec3, max_step: f64) -> DVec3 {
    let delta = target - current;
    let distance = delta.length();
    if distance <= max_step || distance <= f64::EPSILON {
        return target;
    }
    current + delta / distance * max_step
}

fn step_toward_xz_preserving_y(current: DVec3, target: DVec3, max_step: f64) -> DVec3 {
    let horizontal_target = DVec3::new(target.x, current.y, target.z);
    step_toward(current, horizontal_target, max_step)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fauna::components::{BeastKind, FaunaTag};
    use crate::world::dimension::DimensionKind;
    use valence::prelude::{App, Events, IntoSystemConfigs, Update};

    #[test]
    fn critical_qi_triggers_migration() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 0 });
        app.insert_resource(FaunaMigrationState::default());
        app.insert_resource(ZoneRegistry {
            zones: vec![zone("draining", 0.52, 0.0), zone("refuge", 0.90, 64.0)],
        });
        app.add_event::<ZoneQiCriticalEvent>();
        app.add_event::<MigrationEvent>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(Update, fauna_migration_system);

        app.update();
        {
            let mut zones = app.world_mut().resource_mut::<ZoneRegistry>();
            zones.zones[0].spirit_qi = 0.04;
        }
        app.world_mut().resource_mut::<CultivationClock>().tick = MIGRATION_SUSTAIN_TICKS;
        app.update();

        let critical = drain_events::<ZoneQiCriticalEvent>(&app);
        assert_eq!(critical.len(), 1);
        assert_eq!(critical[0].zone_id, "draining");
        assert_eq!(critical[0].spirit_qi, 0.04);

        let migrations = drain_events::<MigrationEvent>(&app);
        assert_eq!(migrations.len(), 1);
        assert_eq!(migrations[0].target_zone, "refuge");
        assert!(migrations[0].direction[0] > 0.9);

        let vfx_events = drain_events::<VfxEventRequest>(&app);
        assert_eq!(vfx_events.len(), 1);
        match &vfx_events[0].payload {
            VfxEventPayloadV1::SpawnParticle {
                event_id,
                direction,
                duration_ticks,
                ..
            } => {
                assert_eq!(event_id, MIGRATION_VISUAL_EVENT_ID);
                assert_eq!(*direction, Some(migrations[0].direction));
                assert_eq!(*duration_ticks, Some(MIGRATION_VFX_DURATION_TICKS));
            }
            other => panic!("expected migration SpawnParticle VFX, got {other:?}"),
        }

        let audio_events = drain_events::<PlaySoundRecipeRequest>(&app);
        assert_eq!(audio_events.len(), 1);
        assert_eq!(audio_events[0].recipe_id, MIGRATION_RUMBLE_RECIPE_ID);
        assert_eq!(audio_events[0].pos, Some([8, 72, 8]));
    }

    #[test]
    fn migration_target_is_highest_qi_neighbor() {
        let zones = vec![
            zone("source", 0.02, 0.0),
            zone("weak", 0.20, 32.0),
            zone("rich", 0.90, 64.0),
        ];

        let target = select_migration_target_zone(&zones[0], &zones)
            .expect("migration should find richest neighboring zone");

        assert_eq!(target.name, "rich");
    }

    #[test]
    fn mass_arrival_triggers_beast_tide() {
        let mut app = App::new();
        app.insert_resource(ActiveEventsResource::default());
        app.insert_resource(ZoneRegistry {
            zones: vec![zone("source", 0.02, 0.0), zone("refuge", 0.90, 64.0)],
        });
        app.add_systems(Update, migration_to_beast_tide_system);

        for _ in 0..MIGRATION_BEAST_TIDE_THRESHOLD {
            app.world_mut().spawn((
                NpcMarker,
                FaunaTag::new(BeastKind::Rat),
                Position::new([70.0, 66.0, 8.0]),
                MigrationTarget {
                    origin_zone: "source".to_string(),
                    target_zone: "refuge".to_string(),
                    target_pos: DVec3::new(72.0, 72.0, 8.0),
                    speed_multiplier: 1.5,
                    started_at_tick: 0,
                },
            ));
        }

        app.update();

        assert!(
            app.world()
                .resource::<ActiveEventsResource>()
                .contains("refuge", EVENT_BEAST_TIDE),
            "10+ 迁徙兽群抵达应升级为既有兽潮状态机"
        );
    }

    #[test]
    fn npc_also_flees() {
        let mut app = App::new();
        app.insert_resource(ZoneRegistry {
            zones: vec![zone("source", 0.02, 0.0), zone("refuge", 0.90, 64.0)],
        });
        app.insert_resource(CultivationClock { tick: 77 });
        app.add_event::<ZoneQiCriticalEvent>();
        app.add_systems(Update, migration_trigger_system);
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcArchetype::Rogue,
                Position::new([8.0, 66.0, 8.0]),
            ))
            .id();

        app.world_mut()
            .resource_mut::<Events<ZoneQiCriticalEvent>>()
            .send(ZoneQiCriticalEvent {
                zone_id: "source".to_string(),
                spirit_qi: 0.02,
                neighbors: vec![("refuge".to_string(), 0.90)],
            });
        app.update();

        let target = app
            .world()
            .get::<MigrationTarget>(npc)
            .expect("NPC should receive MigrationTarget");
        assert_eq!(target.target_zone, "refuge");
        assert_eq!(target.speed_multiplier, 1.2);
        assert_eq!(target.started_at_tick, 77);
    }

    #[test]
    fn dormant_entities_teleport() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 1 });
        app.add_systems(Update, migration_move_system);
        let target_pos = DVec3::new(72.0, 66.0, 8.0);
        let entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcLodTier::Dormant,
                Position::new([8.0, 66.0, 8.0]),
                MigrationTarget {
                    origin_zone: "source".to_string(),
                    target_zone: "refuge".to_string(),
                    target_pos,
                    speed_multiplier: 1.5,
                    started_at_tick: 0,
                },
            ))
            .id();

        app.update();

        assert_eq!(
            app.world().get::<Position>(entity).unwrap().get(),
            target_pos,
            "Dormant 层不真实寻路，直接落到目标 zone 边缘"
        );
    }

    #[test]
    fn near_entities_delegate_migration_to_navigator_without_direct_position_step() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 1 });
        app.add_systems(Update, migration_move_system);
        let start_pos = DVec3::new(8.0, 80.0, 8.0);
        let target_pos = DVec3::new(72.0, 66.0, 8.0);
        let entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcLodTier::Near,
                Position::new([start_pos.x, start_pos.y, start_pos.z]),
                Navigator::new(),
                MigrationTarget {
                    origin_zone: "source".to_string(),
                    target_zone: "refuge".to_string(),
                    target_pos,
                    speed_multiplier: 1.5,
                    started_at_tick: 0,
                },
            ))
            .id();

        app.update();

        assert_eq!(
            app.world().get::<Position>(entity).unwrap().get(),
            start_pos,
            "Near 迁徙实体必须交给 Navigator，不能同 tick 裸写 Position 穿墙/飞行"
        );
        assert!(
            !app.world().get::<Navigator>(entity).unwrap().is_idle(),
            "Near 迁徙实体应设置 Navigator 目标"
        );
    }

    #[test]
    fn far_entities_preserve_altitude_during_coarse_migration_step() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 1_200 });
        app.add_systems(Update, migration_move_system);
        let start_pos = DVec3::new(8.0, 96.0, 8.0);
        let target_pos = DVec3::new(72.0, 66.0, 8.0);
        let entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcLodTier::Far,
                Position::new([start_pos.x, start_pos.y, start_pos.z]),
                MigrationTarget {
                    origin_zone: "source".to_string(),
                    target_zone: "refuge".to_string(),
                    target_pos,
                    speed_multiplier: 1.5,
                    started_at_tick: 0,
                },
            ))
            .id();

        app.update();

        let moved = app.world().get::<Position>(entity).unwrap().get();
        assert!(moved.x > start_pos.x, "Far 迁徙仍应低频向目标 XZ 推进");
        assert_eq!(
            moved.y, start_pos.y,
            "Far 迁徙的粗粒度模拟不能把实体沿 3D 直线拉成飞行"
        );
    }

    #[test]
    fn world_ecology_feedback_loop_low_qi_escalates_to_beast_tide() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 0 });
        app.insert_resource(FaunaMigrationState::default());
        app.insert_resource(ActiveEventsResource::default());
        app.insert_resource(ZoneRegistry {
            zones: vec![zone("draining", 0.04, 0.0), zone("refuge", 0.90, 64.0)],
        });
        app.add_event::<ZoneQiCriticalEvent>();
        app.add_event::<MigrationEvent>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(
            Update,
            (
                fauna_migration_system,
                migration_trigger_system.after(fauna_migration_system),
                migration_move_system.after(migration_trigger_system),
                migration_to_beast_tide_system.after(migration_move_system),
            ),
        );

        for _ in 0..MIGRATION_BEAST_TIDE_THRESHOLD {
            app.world_mut().spawn((
                NpcMarker,
                NpcLodTier::Dormant,
                FaunaTag::new(BeastKind::Rat),
                Position::new([8.0, 66.0, 8.0]),
            ));
        }

        app.update();
        app.world_mut().resource_mut::<CultivationClock>().tick = MIGRATION_SUSTAIN_TICKS;
        app.update();
        app.world_mut().resource_mut::<CultivationClock>().tick = MIGRATION_SUSTAIN_TICKS + 1;
        app.update();

        let events = app.world().resource::<ActiveEventsResource>();
        assert!(
            events.contains("refuge", EVENT_BEAST_TIDE),
            "低灵气持续阈值应触发迁徙，Dormant 兽群到达邻区后升级为兽潮"
        );
        assert_eq!(events.beast_tide_kind_for_zone("refuge"), Some("wandering"));
    }

    fn drain_events<T: Event + Clone>(app: &App) -> Vec<T> {
        let events = app.world().resource::<Events<T>>();
        events
            .get_reader()
            .read(events)
            .cloned()
            .collect::<Vec<_>>()
    }

    fn zone(name: &str, spirit_qi: f64, x: f64) -> Zone {
        Zone {
            name: name.to_string(),
            dimension: DimensionKind::Overworld,
            bounds: (DVec3::new(x, 64.0, 0.0), DVec3::new(x + 16.0, 80.0, 16.0)),
            spirit_qi,
            danger_level: 0,
            active_events: Vec::new(),
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
        }
    }
}
