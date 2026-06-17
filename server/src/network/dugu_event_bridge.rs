use valence::prelude::{EventReader, Query, Res, UniqueId};

use crate::cultivation::dugu::{
    antidote_payload, progress_payload, AntidoteResultEvent, DuguPoisonProgressEvent,
};
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

pub fn publish_antidote_result_events(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<AntidoteResultEvent>,
    unique_ids: Query<&UniqueId>,
) {
    for event in events.read() {
        let payload = antidote_payload(
            event,
            unique_ids.get(event.healer).ok(),
            unique_ids.get(event.target).ok(),
        );
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::AntidoteResult(payload));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use valence::prelude::{App, Update};

    use crate::cultivation::components::MeridianId;
    use crate::cultivation::dugu::AntidoteResult;
    use crate::schema::dugu::AntidoteResultV1;

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

    /// AntidoteResult::Failed — bridge publishes AntidoteResult outbound with correct payload.
    ///
    /// Expectation: emit AntidoteResultEvent with result=Failed →
    /// rx_outbound receives RedisOutbound::AntidoteResult whose result field
    /// serialises as AntidoteResultV1::Failed and whose healer/target carry
    /// "entity:" prefix (no UniqueId components attached).
    #[test]
    fn publishes_antidote_result_failed_to_redis_outbound() {
        let mut app = App::new();
        let (tx_outbound, rx_outbound) = crossbeam_channel::unbounded();
        let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.add_event::<AntidoteResultEvent>();
        app.add_systems(Update, publish_antidote_result_events);

        let healer = app.world_mut().spawn_empty().id();
        let target = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(AntidoteResultEvent {
            healer,
            target,
            result: AntidoteResult::Failed,
            meridian_id: MeridianId::Heart,
            qi_max_after: 95.0,
            tick: 3_000,
        });

        app.update();

        match rx_outbound
            .try_recv()
            .expect("antidote result (failed) should publish to outbound channel")
        {
            RedisOutbound::AntidoteResult(payload) => {
                assert_eq!(
                    payload.result,
                    AntidoteResultV1::Failed,
                    "expected result=Failed because event carried AntidoteResult::Failed"
                );
                assert_eq!(
                    payload.meridian_id, "Heart",
                    "expected meridian_id=Heart matching MeridianId::Heart debug repr"
                );
                assert_eq!(
                    payload.qi_max_after, 95.0,
                    "qi_max_after should pass through verbatim"
                );
                assert_eq!(payload.tick, 3_000, "tick should pass through verbatim");
                assert!(
                    payload.healer.starts_with("entity:"),
                    "healer without UniqueId should fall back to entity: prefix, got {}",
                    payload.healer
                );
                assert!(
                    payload.target.starts_with("entity:"),
                    "target without UniqueId should fall back to entity: prefix, got {}",
                    payload.target
                );
            }
            other => panic!("expected RedisOutbound::AntidoteResult, got {other:?}"),
        }
    }

    /// AntidoteResult::Success — bridge publishes with result=Success.
    ///
    /// Expectation: each AntidoteResult enum variant must have at least one
    /// dedicated case (饱和化测试 requirement). result=Success is the happy path.
    #[test]
    fn publishes_antidote_result_success_to_redis_outbound() {
        let mut app = App::new();
        let (tx_outbound, rx_outbound) = crossbeam_channel::unbounded();
        let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.add_event::<AntidoteResultEvent>();
        app.add_systems(Update, publish_antidote_result_events);

        let healer = app.world_mut().spawn_empty().id();
        let target = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(AntidoteResultEvent {
            healer,
            target,
            result: AntidoteResult::Success,
            meridian_id: MeridianId::Liver,
            qi_max_after: 200.0,
            tick: 42,
        });

        app.update();

        match rx_outbound
            .try_recv()
            .expect("antidote result (success) should publish to outbound channel")
        {
            RedisOutbound::AntidoteResult(payload) => {
                assert_eq!(
                    payload.result,
                    AntidoteResultV1::Success,
                    "expected result=Success because event carried AntidoteResult::Success"
                );
                assert_eq!(
                    payload.meridian_id, "Liver",
                    "expected meridian_id=Liver matching MeridianId::Liver debug repr"
                );
            }
            other => panic!("expected RedisOutbound::AntidoteResult (success), got {other:?}"),
        }
    }

    /// Multi-event batch: two AntidoteResultEvents in one tick → two outbound messages,
    /// no cross-contamination between healer/target ids.
    #[test]
    fn publishes_multiple_antidote_results_in_one_tick() {
        let mut app = App::new();
        let (tx_outbound, rx_outbound) = crossbeam_channel::unbounded();
        let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.add_event::<AntidoteResultEvent>();
        app.add_systems(Update, publish_antidote_result_events);

        let healer_a = app.world_mut().spawn_empty().id();
        let target_a = app.world_mut().spawn_empty().id();
        let healer_b = app.world_mut().spawn_empty().id();
        let target_b = app.world_mut().spawn_empty().id();

        app.world_mut().send_event(AntidoteResultEvent {
            healer: healer_a,
            target: target_a,
            result: AntidoteResult::Success,
            meridian_id: MeridianId::Heart,
            qi_max_after: 100.0,
            tick: 1,
        });
        app.world_mut().send_event(AntidoteResultEvent {
            healer: healer_b,
            target: target_b,
            result: AntidoteResult::Failed,
            meridian_id: MeridianId::Liver,
            qi_max_after: 50.0,
            tick: 2,
        });

        app.update();

        let msg_a = rx_outbound
            .try_recv()
            .expect("first antidote result should publish");
        let msg_b = rx_outbound
            .try_recv()
            .expect("second antidote result should publish");

        let (payload_a, payload_b) = match (msg_a, msg_b) {
            (RedisOutbound::AntidoteResult(a), RedisOutbound::AntidoteResult(b)) => (a, b),
            other => panic!("expected two AntidoteResult outbound messages, got {other:?}"),
        };

        assert_eq!(
            payload_a.result,
            AntidoteResultV1::Success,
            "first event should map to Success"
        );
        assert_eq!(
            payload_b.result,
            AntidoteResultV1::Failed,
            "second event should map to Failed"
        );
        assert_ne!(
            payload_a.healer, payload_b.healer,
            "healer ids must not be swapped across events"
        );
        assert_ne!(
            payload_a.target, payload_b.target,
            "target ids must not be swapped across events"
        );
        assert_eq!(payload_a.tick, 1, "first event tick should be 1");
        assert_eq!(payload_b.tick, 2, "second event tick should be 2");

        assert!(
            rx_outbound.try_recv().is_err(),
            "no further messages expected after two events"
        );
    }

    /// Regression: DuguPoisonProgress path still works after adding AntidoteResult variant.
    /// Expectation: emitting DuguPoisonProgressEvent still routes to DuguPoisonProgress outbound,
    /// not AntidoteResult (confirm enum dispatch is not broken by the new variant).
    #[test]
    fn dugu_poison_progress_still_routes_correctly_after_antidote_variant_added() {
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
            flow_capacity_after: 50.0,
            qi_max_after: 80.0,
            actual_loss_this_tick: 5.0,
            tick: 9_999,
        });

        app.update();

        match rx_outbound
            .try_recv()
            .expect("regression: dugu poison progress should still publish")
        {
            RedisOutbound::DuguPoisonProgress(payload) => {
                assert_eq!(
                    payload.tick, 9_999,
                    "regression check: tick should match for DuguPoisonProgress outbound"
                );
            }
            other => panic!(
                "regression: expected DuguPoisonProgress outbound after antidote variant added, got {other:?}"
            ),
        }
    }

    /// Serialize round-trip: AntidoteResultEventV1 serialises and deserialises without data loss.
    #[test]
    fn antidote_result_event_v1_serialize_roundtrip() {
        let evt = crate::schema::dugu::AntidoteResultEventV1 {
            healer: "player:alice".to_string(),
            target: "player:bob".to_string(),
            result: AntidoteResultV1::Failed,
            meridian_id: "Heart".to_string(),
            qi_max_after: 123.5,
            tick: 7_777,
        };
        let json = serde_json::to_string(&evt)
            .expect("AntidoteResultEventV1 should serialize to JSON without error");
        assert!(
            json.contains("\"failed\""),
            "expected snake_case 'failed' in JSON, got {json}"
        );
        let back: crate::schema::dugu::AntidoteResultEventV1 = serde_json::from_str(&json)
            .expect("AntidoteResultEventV1 should deserialize from JSON without error");
        assert_eq!(
            back, evt,
            "round-trip should produce identical struct — expected {:?}, got {:?}",
            evt, back
        );
    }
}
