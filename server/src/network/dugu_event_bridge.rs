use valence::prelude::{EventReader, Query, Res, UniqueId};

use crate::cultivation::dugu::{progress_payload, DuguPoisonProgressEvent};
use crate::network::redis_bridge::RedisOutbound;
use crate::network::RedisBridgeResource;

pub fn publish_dugu_poison_progress_events(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<DuguPoisonProgressEvent>,
    unique_ids: Query<&UniqueId>,
) {
    for event in events.read() {
        let payload = progress_payload(
            event,
            unique_ids.get(event.target).ok(),
            unique_ids.get(event.attacker).ok(),
        );
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::DuguPoisonProgress(payload));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use valence::prelude::{App, Update};

    use crate::cultivation::components::MeridianId;

    #[test]
    fn publishes_poison_progress_to_redis_outbound() {
        let mut app = App::new();
        let (tx_outbound, rx_outbound) = crossbeam_channel::unbounded();
        let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.add_event::<DuguPoisonProgressEvent>();
        app.add_systems(Update, publish_dugu_poison_progress_events);

        let attacker = app.world_mut().spawn_empty().id();
        let target = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(DuguPoisonProgressEvent {
            target,
            attacker,
            meridian_id: MeridianId::Heart,
            flow_capacity_after: 98.0,
            qi_max_after: 108.0,
            actual_loss_this_tick: 2.0,
            tick: 6_000,
        });

        app.update();

        match rx_outbound
            .try_recv()
            .expect("dugu progress should publish")
        {
            RedisOutbound::DuguPoisonProgress(payload) => {
                assert_eq!(payload.meridian_id, "Heart");
                assert_eq!(payload.flow_capacity_after, 98.0);
                assert_eq!(payload.qi_max_after, 108.0);
                assert_eq!(payload.actual_loss_this_tick, 2.0);
                assert_eq!(payload.tick, 6_000);
                assert!(payload.target.starts_with("entity:"));
                assert!(payload.attacker.starts_with("entity:"));
            }
            other => panic!("expected dugu progress outbound, got {other:?}"),
        }
    }
}
