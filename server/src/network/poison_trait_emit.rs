use valence::prelude::{Client, Entity, EventReader, Query, Res, With};

use crate::cultivation::poison_trait::{
    DigestionLoad, PoisonDoseEvent, PoisonOverdoseEvent, PoisonToxicity,
};
use crate::cultivation::tick::CultivationClock;
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::redis_bridge::RedisOutbound;
use crate::network::{log_payload_build_error, send_server_data_payload, RedisBridgeResource};
use crate::schema::poison_trait::{
    PoisonDoseEventV1, PoisonOverdoseEventV1, PoisonOverdoseSeverityV1, PoisonTraitStateV1,
};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};

const POISON_STATE_EMIT_INTERVAL_TICKS: u64 = 20;

pub fn publish_poison_dose_events(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<PoisonDoseEvent>,
    mut clients: Query<&mut Client>,
    states: Query<(&PoisonToxicity, &DigestionLoad)>,
) {
    for event in events.read() {
        let payload = dose_payload(event);
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::PoisonDoseEvent(payload.clone()));
        send_payload_to_entity(
            &mut clients,
            event.player,
            ServerDataPayloadV1::PoisonDoseEvent(payload),
        );
        if let Ok((toxicity, digestion)) = states.get(event.player) {
            send_payload_to_entity(
                &mut clients,
                event.player,
                ServerDataPayloadV1::PoisonTraitState(state_payload(
                    event.player,
                    toxicity,
                    digestion,
                )),
            );
        }
    }
}

pub fn publish_poison_overdose_events(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<PoisonOverdoseEvent>,
    mut clients: Query<&mut Client>,
) {
    for event in events.read() {
        let payload = overdose_payload(event);
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::PoisonOverdoseEvent(payload.clone()));
        send_payload_to_entity(
            &mut clients,
            event.player,
            ServerDataPayloadV1::PoisonOverdoseEvent(payload),
        );
    }
}

pub fn emit_poison_trait_state_payloads(
    clock: Res<CultivationClock>,
    mut clients: Query<(Entity, &mut Client, &PoisonToxicity, &DigestionLoad), With<Client>>,
) {
    if !clock.tick.is_multiple_of(POISON_STATE_EMIT_INTERVAL_TICKS) {
        return;
    }

    for (entity, mut client, toxicity, digestion) in &mut clients {
        send_payload(
            &mut client,
            ServerDataPayloadV1::PoisonTraitState(state_payload(entity, toxicity, digestion)),
        );
    }
}

fn dose_payload(event: &PoisonDoseEvent) -> PoisonDoseEventV1 {
    PoisonDoseEventV1 {
        v: 1,
        player_entity_id: event.player.to_bits(),
        dose_amount: event.dose_amount,
        side_effect_tag: event.side_effect_tag.into(),
        poison_level_after: event.poison_level_after,
        digestion_after: event.digestion_after,
        at_tick: event.at_tick,
    }
}

fn overdose_payload(event: &PoisonOverdoseEvent) -> PoisonOverdoseEventV1 {
    PoisonOverdoseEventV1 {
        v: 1,
        player_entity_id: event.player.to_bits(),
        severity: PoisonOverdoseSeverityV1::from(event.severity),
        overflow: event.overflow,
        lifespan_penalty_years: event.lifespan_penalty_years,
        micro_tear_probability: event.micro_tear_probability,
        at_tick: event.at_tick,
    }
}

fn state_payload(
    entity: Entity,
    toxicity: &PoisonToxicity,
    digestion: &DigestionLoad,
) -> PoisonTraitStateV1 {
    let toxicity = toxicity.normalized();
    let digestion = digestion.normalized();
    PoisonTraitStateV1 {
        v: 1,
        player_entity_id: entity.to_bits(),
        poison_toxicity: toxicity.level,
        digestion_current: digestion.current,
        digestion_capacity: digestion.capacity,
        toxicity_tier_unlocked: toxicity.toxicity_tier_unlocked,
    }
}

fn send_payload_to_entity(
    clients: &mut Query<&mut Client>,
    entity: Entity,
    payload: ServerDataPayloadV1,
) {
    let Ok(mut client) = clients.get_mut(entity) else {
        return;
    };
    send_payload(&mut client, payload);
}

fn send_payload(client: &mut Client, payload: ServerDataPayloadV1) {
    let envelope = ServerDataV1::new(payload);
    let payload_type = payload_type_label(envelope.payload_type());
    let payload_bytes = match serialize_server_data_payload(&envelope) {
        Ok(bytes) => bytes,
        Err(error) => {
            log_payload_build_error(payload_type, &error);
            return;
        }
    };
    send_server_data_payload(client, payload_bytes.as_slice());
    tracing::debug!(
        "[bong][network] sent {} {} payload",
        SERVER_DATA_CHANNEL,
        payload_type
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    use crossbeam_channel::TryRecvError;
    use valence::prelude::{App, Update};
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    use crate::cultivation::poison_trait::{PoisonOverdoseSeverity, PoisonSideEffectTag};

    fn spawn_client_with_poison(app: &mut App) -> (Entity, MockClientHelper) {
        let (client_bundle, helper) = create_mock_client("Poisoned");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                PoisonToxicity {
                    level: 12.0,
                    toxicity_tier_unlocked: true,
                    ..PoisonToxicity::default()
                },
                DigestionLoad {
                    current: 30.0,
                    capacity: 100.0,
                    ..DigestionLoad::default()
                },
            ))
            .id();
        (entity, helper)
    }

    fn flush_all_client_packets(app: &mut App) {
        let world = app.world_mut();
        let mut query = world.query::<&mut Client>();
        for mut client in query.iter_mut(world) {
            client
                .flush_packets()
                .expect("mock client flush should work");
        }
    }

    fn drained_payloads(helper: &mut MockClientHelper) -> Vec<serde_json::Value> {
        let mut payloads = Vec::new();
        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };
            if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                continue;
            }
            payloads.push(serde_json::from_slice::<serde_json::Value>(packet.data.0 .0).unwrap());
        }
        payloads
    }

    #[test]
    fn dose_event_publishes_redis_and_client_state() {
        let mut app = App::new();
        let (tx_outbound, rx_outbound) = crossbeam_channel::unbounded();
        let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.add_event::<PoisonDoseEvent>();
        app.add_systems(Update, publish_poison_dose_events);
        let (player, mut helper) = spawn_client_with_poison(&mut app);

        app.world_mut().send_event(PoisonDoseEvent {
            player,
            dose_amount: 5.0,
            side_effect_tag: PoisonSideEffectTag::QiFocusDrift2h,
            poison_level_after: 17.0,
            digestion_after: 50.0,
            at_tick: 100,
        });
        app.update();
        flush_all_client_packets(&mut app);

        match rx_outbound
            .try_recv()
            .expect("dose event should publish to Redis")
        {
            RedisOutbound::PoisonDoseEvent(payload) => {
                assert_eq!(payload.player_entity_id, player.to_bits());
                assert_eq!(payload.dose_amount, 5.0);
            }
            other => panic!("expected PoisonDoseEvent outbound, got {other:?}"),
        }
        assert!(matches!(rx_outbound.try_recv(), Err(TryRecvError::Empty)));

        let payloads = drained_payloads(&mut helper);
        assert_eq!(payloads.len(), 2);
        assert_eq!(payloads[0]["type"], "poison_dose_event");
        assert_eq!(payloads[1]["type"], "poison_trait_state");
        assert_eq!(payloads[1]["poison_toxicity"], 12.0);
        assert_eq!(payloads[1]["toxicity_tier_unlocked"], true);
    }

    #[test]
    fn overdose_event_publishes_redis_and_client_payload() {
        let mut app = App::new();
        let (tx_outbound, rx_outbound) = crossbeam_channel::unbounded();
        let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.add_event::<PoisonOverdoseEvent>();
        app.add_systems(Update, publish_poison_overdose_events);
        let (player, mut helper) = spawn_client_with_poison(&mut app);

        app.world_mut().send_event(PoisonOverdoseEvent {
            player,
            severity: PoisonOverdoseSeverity::Moderate,
            overflow: 30.0,
            lifespan_penalty_years: 1.0,
            micro_tear_probability: 0.1,
            at_tick: 120,
        });
        app.update();
        flush_all_client_packets(&mut app);

        match rx_outbound
            .try_recv()
            .expect("overdose event should publish to Redis")
        {
            RedisOutbound::PoisonOverdoseEvent(payload) => {
                assert_eq!(payload.player_entity_id, player.to_bits());
                assert_eq!(payload.lifespan_penalty_years, 1.0);
            }
            other => panic!("expected PoisonOverdoseEvent outbound, got {other:?}"),
        }

        let payloads = drained_payloads(&mut helper);
        assert_eq!(payloads.len(), 1);
        assert_eq!(payloads[0]["type"], "poison_overdose_event");
        assert_eq!(payloads[0]["severity"], "moderate");
    }

    #[test]
    fn periodic_state_payload_uses_current_poison_components() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 20 });
        app.add_systems(Update, emit_poison_trait_state_payloads);
        let (_player, mut helper) = spawn_client_with_poison(&mut app);

        app.update();
        flush_all_client_packets(&mut app);

        let payloads = drained_payloads(&mut helper);
        assert_eq!(payloads.len(), 1);
        assert_eq!(payloads[0]["type"], "poison_trait_state");
        assert_eq!(payloads[0]["digestion_capacity"], 100.0);
    }
}
