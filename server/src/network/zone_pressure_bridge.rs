use valence::prelude::{EventReader, Res};

use super::redis_bridge::RedisOutbound;
use super::RedisBridgeResource;
use crate::lingtian::{PressureLevel, ZonePressureCrossed};
use crate::npc::movement::GameTick;
use crate::schema::zone_pressure::ZonePressureCrossedV1;

const ZONE_PRESSURE_EVENT_VERSION: u8 = 1;

pub fn publish_zone_pressure_crossed_events(
    redis: Res<RedisBridgeResource>,
    game_tick: Option<Res<GameTick>>,
    mut events: EventReader<ZonePressureCrossed>,
) {
    let at_tick = game_tick.map(|tick| u64::from(tick.0)).unwrap_or_default();
    for event in events.read() {
        let wire = ZonePressureCrossedV1 {
            v: ZONE_PRESSURE_EVENT_VERSION,
            kind: "zone_pressure_crossed".to_string(),
            zone: event.zone.clone(),
            level: pressure_level_to_wire(event.level).to_string(),
            raw_pressure: event.raw_pressure,
            at_tick,
        };
        if let Err(error) = redis
            .tx_outbound
            .send(RedisOutbound::ZonePressureCrossed(wire))
        {
            tracing::warn!("[bong][zone_pressure_bridge] dropped ZonePressureCrossed: {error}");
        }
    }
}

fn pressure_level_to_wire(level: PressureLevel) -> &'static str {
    match level {
        PressureLevel::None => "none",
        PressureLevel::Low => "low",
        PressureLevel::Mid => "mid",
        PressureLevel::High => "high",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::{unbounded, Receiver};
    use valence::prelude::{App, Update};

    fn setup_app() -> (App, Receiver<RedisOutbound>) {
        let mut app = App::new();
        let (tx_outbound, rx_outbound) = unbounded();
        let (_tx_inbound, rx_inbound) = unbounded();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        (app, rx_outbound)
    }

    #[test]
    fn publishes_zone_pressure_crossing_to_redis_outbound() {
        let (mut app, rx) = setup_app();
        app.add_event::<ZonePressureCrossed>();
        app.insert_resource(GameTick(77));
        app.add_systems(Update, publish_zone_pressure_crossed_events);

        app.world_mut().send_event(ZonePressureCrossed {
            zone: "spawn".to_string(),
            level: PressureLevel::High,
            raw_pressure: 1.25,
        });
        app.update();

        let outbound = rx.try_recv().expect("expected zone pressure outbound");
        let RedisOutbound::ZonePressureCrossed(payload) = outbound else {
            panic!("expected ZonePressureCrossed outbound");
        };
        assert_eq!(payload.zone, "spawn");
        assert_eq!(payload.level, "high");
        assert_eq!(payload.raw_pressure, 1.25);
        assert_eq!(payload.at_tick, 77);
    }
}
