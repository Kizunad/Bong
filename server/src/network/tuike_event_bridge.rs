//! plan-tuike-v1 — ShedEvent → Redis 叙事事件。

use valence::prelude::{EventReader, Res};

use crate::combat::tuike::ShedEvent;
use crate::network::redis_bridge::RedisOutbound;
use crate::network::RedisBridgeResource;

pub fn publish_tuike_shed_events(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<ShedEvent>,
) {
    for event in events.read() {
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::TuikeShed(event.payload()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::Receiver;
    use valence::prelude::{App, Events, Update};

    fn setup_app() -> (App, Receiver<RedisOutbound>) {
        let (tx_outbound, rx_outbound) = crossbeam_channel::unbounded();
        let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded();
        let mut app = App::new();
        app.add_event::<ShedEvent>();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.add_systems(Update, publish_tuike_shed_events);
        (app, rx_outbound)
    }

    #[test]
    fn publishes_shed_event_payload() {
        let (mut app, rx) = setup_app();
        let target = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<Events<ShedEvent>>()
            .send(ShedEvent {
                target,
                attacker: None,
                target_id: "offline:Azure".to_string(),
                attacker_id: None,
                kind: crate::combat::tuike::FalseSkinKind::SpiderSilk,
                layers_shed: 1,
                layers_remaining: 0,
                contam_absorbed: 10.0,
                contam_overflow: 2.0,
                tick: 8,
            });

        app.update();

        let outbound = rx.try_recv().expect("expected outbound shed event");
        let RedisOutbound::TuikeShed(payload) = outbound else {
            panic!("expected TuikeShed, got {outbound:?}");
        };
        assert_eq!(payload.target_id, "offline:Azure");
        assert_eq!(payload.layers_shed, 1);
        assert_eq!(payload.contam_overflow, 2.0);
    }
}
