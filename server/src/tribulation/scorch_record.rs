//! plan-terrain-tribulation-scorch-v1 P3 — 实时天劫落点写回焦土标记的纯模型。
//!
//! 真正的块写入由后续 world persistence 消费；这里先锁定事件命中 zone 后应生成
//! `glass_fulgurite` 记号的契约，避免把天劫地理后果散落到 narration 或天气层。

pub const GLASS_FULGURITE_MARKER_ID: &str = "glass_fulgurite";
pub const TRIBULATION_SCORCH_EVENT: &str = "tribulation_scorch";

use valence::prelude::{DVec3, EventReader, Position, Query, Res, ResMut, Resource};

use crate::combat::CombatClock;
use crate::cultivation::tribulation::TribulationSettled;
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::zone::ZoneRegistry;

#[derive(Debug, Clone, PartialEq)]
pub struct ScorchRecord {
    pub zone_id: String,
    pub marker_id: String,
    pub pos_xyz: [f64; 3],
    pub created_at_tick: u64,
    pub source_event: String,
}

#[derive(Debug, Clone, Default)]
pub struct TribulationScorchRecords {
    records: Vec<ScorchRecord>,
}

impl Resource for TribulationScorchRecords {}

impl TribulationScorchRecords {
    pub fn push(&mut self, record: ScorchRecord) {
        self.records.push(record);
    }

    pub fn records(&self) -> &[ScorchRecord] {
        self.records.as_slice()
    }
}

pub fn record_tribulation_scorch_system(
    mut settled: EventReader<TribulationSettled>,
    zones: Option<Res<ZoneRegistry>>,
    clock: Option<Res<CombatClock>>,
    actors: Query<(&Position, Option<&CurrentDimension>)>,
    mut records: ResMut<TribulationScorchRecords>,
) {
    let Some(zones) = zones else {
        return;
    };
    let created_at_tick = clock.as_deref().map_or(0, |clock| clock.tick);
    for event in settled.read() {
        let Ok((position, dimension)) = actors.get(event.entity) else {
            continue;
        };
        let pos = position.get();
        let dimension = dimension
            .map(|value| value.0)
            .unwrap_or(DimensionKind::Overworld);
        let Some(zone) = zones.find_zone(dimension, pos) else {
            continue;
        };
        if let Some(record) = build_scorch_record(
            zone.name.as_str(),
            zone.active_events.as_slice(),
            Some(dvec3_to_xyz(pos)),
            created_at_tick,
        ) {
            records.push(record);
        }
    }
}

pub fn build_scorch_record(
    zone_id: &str,
    zone_active_events: &[String],
    epicenter: Option<[f64; 3]>,
    created_at_tick: u64,
) -> Option<ScorchRecord> {
    if !is_tribulation_scorch_zone(zone_id, zone_active_events) {
        return None;
    }
    let pos_xyz = epicenter?;
    if !pos_xyz.iter().all(|value| value.is_finite()) {
        return None;
    }
    Some(ScorchRecord {
        zone_id: zone_id.to_string(),
        marker_id: GLASS_FULGURITE_MARKER_ID.to_string(),
        pos_xyz,
        created_at_tick,
        source_event: TRIBULATION_SCORCH_EVENT.to_string(),
    })
}

pub fn is_tribulation_scorch_zone(zone_id: &str, zone_active_events: &[String]) -> bool {
    zone_id.contains("scorch")
        || zone_active_events
            .iter()
            .any(|event| event == TRIBULATION_SCORCH_EVENT)
}

fn dvec3_to_xyz(pos: DVec3) -> [f64; 3] {
    [pos.x, pos.y, pos.z]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::tribulation::{TribulationKind, TribulationSettled};
    use crate::schema::tribulation::{DuXuOutcomeV1, DuXuResultV1};
    use crate::world::zone::Zone;
    use valence::prelude::{App, DVec3, Events, Position, Update};

    fn scorch_zone() -> Zone {
        Zone {
            name: "north_waste_east_scorch".to_string(),
            dimension: DimensionKind::Overworld,
            bounds: (
                DVec3::new(1500.0, 60.0, -8500.0),
                DVec3::new(2700.0, 100.0, -7500.0),
            ),
            spirit_qi: 0.28,
            danger_level: 7,
            active_events: vec![TRIBULATION_SCORCH_EVENT.to_string()],
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
        }
    }

    fn settled_event(entity: valence::prelude::Entity) -> TribulationSettled {
        TribulationSettled {
            entity,
            kind: TribulationKind::DuXu,
            source: None,
            result: DuXuResultV1 {
                char_id: "offline:Azure".to_string(),
                outcome: DuXuOutcomeV1::Killed,
                killer: None,
                waves_survived: 5,
                reason: Some("du_xu_tribulation".to_string()),
            },
        }
    }

    #[test]
    fn tribulation_hit_in_scorch_zone_records_glass_fulgurite_marker() {
        let record = build_scorch_record(
            "north_waste_east_scorch",
            &[TRIBULATION_SCORCH_EVENT.to_string()],
            Some([2100.0, 80.0, -8000.0]),
            42,
        )
        .expect("scorch zone should record terrain marker");

        assert_eq!(record.marker_id, GLASS_FULGURITE_MARKER_ID);
        assert_eq!(record.pos_xyz, [2100.0, 80.0, -8000.0]);
        assert_eq!(record.created_at_tick, 42);
    }

    #[test]
    fn non_scorch_zone_does_not_record_marker() {
        let record = build_scorch_record("spawn", &[], Some([0.0, 70.0, 0.0]), 1);

        assert!(record.is_none());
    }

    #[test]
    fn missing_or_invalid_epicenter_is_not_recorded() {
        assert!(build_scorch_record("drift_scorch_001", &[], None, 1).is_none());
        assert!(
            build_scorch_record("drift_scorch_001", &[], Some([f64::NAN, 70.0, 0.0]), 1).is_none()
        );
    }

    #[test]
    fn settled_tribulation_in_scorch_zone_records_runtime_glass_fulgurite() {
        let mut app = App::new();
        app.add_event::<TribulationSettled>();
        app.insert_resource(ZoneRegistry {
            zones: vec![scorch_zone()],
        });
        app.insert_resource(CombatClock { tick: 77 });
        app.insert_resource(TribulationScorchRecords::default());
        app.add_systems(Update, record_tribulation_scorch_system);
        let entity = app
            .world_mut()
            .spawn((
                Position::new([2100.0, 80.0, -8000.0]),
                CurrentDimension(DimensionKind::Overworld),
            ))
            .id();

        app.world_mut()
            .resource_mut::<Events<TribulationSettled>>()
            .send(settled_event(entity));
        app.update();

        let records = app.world().resource::<TribulationScorchRecords>();
        assert_eq!(records.records().len(), 1);
        assert_eq!(records.records()[0].zone_id, "north_waste_east_scorch");
        assert_eq!(records.records()[0].marker_id, GLASS_FULGURITE_MARKER_ID);
        assert_eq!(records.records()[0].pos_xyz, [2100.0, 80.0, -8000.0]);
        assert_eq!(records.records()[0].created_at_tick, 77);
    }

    #[test]
    fn settled_tribulation_outside_scorch_zone_does_not_record_runtime_marker() {
        let mut app = App::new();
        app.add_event::<TribulationSettled>();
        app.insert_resource(ZoneRegistry::fallback());
        app.insert_resource(CombatClock { tick: 77 });
        app.insert_resource(TribulationScorchRecords::default());
        app.add_systems(Update, record_tribulation_scorch_system);
        let entity = app
            .world_mut()
            .spawn((
                Position::new([0.0, 66.0, 0.0]),
                CurrentDimension(DimensionKind::Overworld),
            ))
            .id();

        app.world_mut()
            .resource_mut::<Events<TribulationSettled>>()
            .send(settled_event(entity));
        app.update();

        let records = app.world().resource::<TribulationScorchRecords>();
        assert!(records.records().is_empty());
    }
}
