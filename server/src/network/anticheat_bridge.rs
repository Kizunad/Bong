use valence::prelude::{EventReader, Res};

use crate::combat::anticheat::AntiCheatViolationEvent;
use crate::network::redis_bridge::RedisOutbound;

use super::RedisBridgeResource;

pub fn publish_anticheat_violation_events(
    mut events: EventReader<AntiCheatViolationEvent>,
    redis: Res<RedisBridgeResource>,
) {
    for event in events.read() {
        if let Err(error) = redis
            .tx_outbound
            .send(RedisOutbound::AntiCheatReport(event.report.clone()))
        {
            tracing::warn!("[bong][anticheat] failed to enqueue Redis report: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use crossbeam_channel::unbounded;
    use valence::prelude::{App, Update};

    use crate::combat::anticheat::AntiCheatViolationEvent;
    use crate::network::RedisBridgeResource;
    use crate::schema::anticheat::{AntiCheatReportV1, ViolationKindV1};

    use super::*;

    #[test]
    fn publishes_anticheat_events_to_redis_outbound() {
        let mut app = App::new();
        let (tx_outbound, rx_outbound) = unbounded();
        let (_tx_inbound, rx_inbound) = unbounded();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.add_event::<AntiCheatViolationEvent>();
        app.add_systems(Update, publish_anticheat_violation_events);

        app.world_mut().send_event(AntiCheatViolationEvent {
            report: AntiCheatReportV1::new(
                "offline:Azure",
                42,
                1200,
                ViolationKindV1::ReachExceeded,
                10,
                "reach: target_distance=6.200 server_max=4.000",
            ),
        });
        app.update();

        let outbound = rx_outbound
            .try_recv()
            .expect("anticheat event should publish");
        let RedisOutbound::AntiCheatReport(report) = outbound else {
            panic!("expected anticheat report outbound");
        };
        assert_eq!(report.char_id, "offline:Azure");
        assert_eq!(report.kind, ViolationKindV1::ReachExceeded);
    }
}
