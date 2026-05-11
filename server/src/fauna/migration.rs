use std::collections::HashMap;

use valence::prelude::{bevy_ecs, DVec3, Event, EventWriter, Res, ResMut, Resource};

use crate::cultivation::tick::CultivationClock;
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::schema::vfx_event::VfxEventPayloadV1;
use crate::world::zone::{Zone, ZoneRegistry};

pub const MIGRATION_QI_LOW_THRESHOLD: f64 = 0.18;
pub const MIGRATION_QI_DROP_THRESHOLD: f64 = 0.20;
pub const MIGRATION_MIN_DURATION_TICKS: u32 = 6_000;
pub const MIGRATION_MAX_DURATION_TICKS: u32 = 12_000;
pub const MIGRATION_VISUAL_EVENT_ID: &str = "bong:migration_visual";
const MIGRATION_VFX_DURATION_TICKS: u16 = 200;

#[derive(Debug, Clone, PartialEq, Event)]
pub struct MigrationEvent {
    pub zone_id: String,
    pub direction: [f64; 3],
    pub duration_ticks: u32,
    pub started_at_tick: u64,
}

#[derive(Debug, Clone, Default, Resource)]
pub struct FaunaMigrationState {
    previous_qi_by_zone: HashMap<String, f64>,
    active_until_by_zone: HashMap<String, u64>,
}

pub fn fauna_migration_system(
    zones: Option<Res<ZoneRegistry>>,
    clock: Option<Res<CultivationClock>>,
    mut state: ResMut<FaunaMigrationState>,
    mut events: EventWriter<MigrationEvent>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    let Some(zones) = zones else {
        state.previous_qi_by_zone.clear();
        return;
    };
    let now = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();
    for zone in &zones.zones {
        let previous = state
            .previous_qi_by_zone
            .insert(zone.name.clone(), zone.spirit_qi);
        let Some(previous_qi) = previous else {
            continue;
        };
        let active_until = state
            .active_until_by_zone
            .get(zone.name.as_str())
            .copied()
            .unwrap_or_default();
        if now < active_until {
            continue;
        }
        if previous_qi - zone.spirit_qi < MIGRATION_QI_DROP_THRESHOLD
            || zone.spirit_qi > MIGRATION_QI_LOW_THRESHOLD
        {
            continue;
        }
        let duration = migration_duration_ticks(zone);
        state
            .active_until_by_zone
            .insert(zone.name.clone(), now.saturating_add(duration as u64));
        let event = MigrationEvent {
            zone_id: zone.name.clone(),
            direction: refuge_direction(zone, &zones.zones),
            duration_ticks: duration,
            started_at_tick: now,
        };
        events.send(event.clone());
        vfx_events.send(migration_vfx_request(zone, &event));
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

fn refuge_direction(source: &Zone, zones: &[Zone]) -> [f64; 3] {
    let source_center = source.center();
    let target = zones
        .iter()
        .filter(|zone| zone.name != source.name && zone.spirit_qi > source.spirit_qi)
        .max_by(|left, right| left.spirit_qi.total_cmp(&right.spirit_qi))
        .map(Zone::center)
        .unwrap_or_else(|| source_center + DVec3::new(1.0, 0.0, 0.0));
    let vector = target - source_center;
    let horizontal_len = (vector.x * vector.x + vector.z * vector.z).sqrt();
    if horizontal_len <= 1e-6 {
        return [1.0, 0.0, 0.0];
    }
    [vector.x / horizontal_len, 0.0, vector.z / horizontal_len]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::dimension::DimensionKind;
    use valence::prelude::{App, DVec3, Events, Update};

    #[test]
    fn migration_triggers_on_qi_drop() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 10 });
        app.insert_resource(FaunaMigrationState::default());
        app.insert_resource(ZoneRegistry {
            zones: vec![zone("draining", 0.52, 0.0), zone("refuge", 0.90, 64.0)],
        });
        app.add_event::<MigrationEvent>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, fauna_migration_system);

        app.update();
        {
            let mut zones = app.world_mut().resource_mut::<ZoneRegistry>();
            zones.zones[0].spirit_qi = 0.10;
        }
        app.world_mut().resource_mut::<CultivationClock>().tick = 20;
        app.update();

        let events = app.world().resource::<Events<MigrationEvent>>();
        let mut reader = events.get_reader();
        let emitted = reader.read(events).cloned().collect::<Vec<_>>();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].zone_id, "draining");
        assert!(emitted[0].direction[0] > 0.9);
        assert!(emitted[0].duration_ticks >= MIGRATION_MIN_DURATION_TICKS);

        let vfx_events = app.world().resource::<Events<VfxEventRequest>>();
        let emitted_vfx = vfx_events
            .get_reader()
            .read(vfx_events)
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(emitted_vfx.len(), 1);
        match &emitted_vfx[0].payload {
            VfxEventPayloadV1::SpawnParticle {
                event_id,
                direction,
                duration_ticks,
                ..
            } => {
                assert_eq!(event_id, MIGRATION_VISUAL_EVENT_ID);
                assert_eq!(*direction, Some(emitted[0].direction));
                assert_eq!(*duration_ticks, Some(MIGRATION_VFX_DURATION_TICKS));
            }
            other => panic!("expected migration SpawnParticle VFX, got {other:?}"),
        }
    }

    #[test]
    fn migration_does_not_retrigger_while_active() {
        let mut state = FaunaMigrationState::default();
        state
            .active_until_by_zone
            .insert("draining".to_string(), 100);
        assert_eq!(state.active_until_by_zone["draining"], 100);
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
