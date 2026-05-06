use valence::prelude::{EventReader, Res};

use crate::lingtian::events::ZonePressureCrossed;
use crate::lingtian::pressure::PressureLevel;
use crate::schema::lingtian::{LingtianZonePressureLevelV1, LingtianZonePressureV1};

use super::redis_bridge::RedisOutbound;
use super::RedisBridgeResource;

pub fn publish_lingtian_zone_pressure_events(
    mut events: EventReader<ZonePressureCrossed>,
    redis: Res<RedisBridgeResource>,
) {
    for event in events.read() {
        let Some(level) = pressure_level_to_wire(event.level) else {
            continue;
        };
        let payload =
            LingtianZonePressureV1::new(event.zone.clone(), level, event.raw_pressure, event.tick);
        if let Err(error) = redis
            .tx_outbound
            .send(RedisOutbound::LingtianZonePressure(payload))
        {
            tracing::warn!("[bong][lingtian] dropped zone pressure Redis event: {error}");
        }
    }
}

fn pressure_level_to_wire(level: PressureLevel) -> Option<LingtianZonePressureLevelV1> {
    match level {
        PressureLevel::None => None,
        PressureLevel::Low => Some(LingtianZonePressureLevelV1::Low),
        PressureLevel::Mid => Some(LingtianZonePressureLevelV1::Mid),
        PressureLevel::High => Some(LingtianZonePressureLevelV1::High),
    }
}

#[cfg(test)]
mod tests {
    use crossbeam_channel::Receiver;
    use valence::prelude::{App, Update};

    use super::*;
    use crate::network::redis_bridge::RedisOutbound;

    fn setup_app() -> (App, Receiver<RedisOutbound>) {
        let mut app = App::new();
        let (tx, rx) = crossbeam_channel::unbounded();
        let (_in_tx, in_rx) = crossbeam_channel::unbounded();
        app.insert_resource(RedisBridgeResource {
            tx_outbound: tx,
            rx_inbound: in_rx,
        });
        app.add_event::<ZonePressureCrossed>();
        app.add_systems(Update, publish_lingtian_zone_pressure_events);
        (app, rx)
    }

    #[test]
    fn publishes_high_pressure_crossing_to_redis() {
        let (mut app, rx) = setup_app();
        app.world_mut().send_event(ZonePressureCrossed {
            zone: "starter_zone".to_string(),
            level: PressureLevel::High,
            raw_pressure: 1.1,
            tick: 1440,
        });

        app.update();

        match rx.try_recv().expect("expected one pressure event") {
            RedisOutbound::LingtianZonePressure(payload) => {
                assert_eq!(payload.v, 1);
                assert_eq!(payload.zone, "starter_zone");
                assert_eq!(payload.level, LingtianZonePressureLevelV1::High);
                assert!((payload.raw_pressure - 1.1).abs() < f32::EPSILON);
                assert_eq!(payload.tick, 1440);
            }
            other => panic!("expected LingtianZonePressure, got {other:?}"),
        }
    }

    #[test]
    fn skips_none_pressure_level() {
        let (mut app, rx) = setup_app();
        app.world_mut().send_event(ZonePressureCrossed {
            zone: "starter_zone".to_string(),
            level: PressureLevel::None,
            raw_pressure: 0.0,
            tick: 1440,
        });

        app.update();

        assert!(rx.try_recv().is_err());
    }
}
