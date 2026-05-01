use valence::prelude::{
    Added, Client, Commands, Entity, EventReader, Query, Res, ViewDistance, With,
};

use crate::cultivation::breakthrough::BreakthroughOutcome;
use crate::cultivation::components::Cultivation;
use crate::cultivation::death_hooks::PlayerRevived;
use crate::cultivation::tick::CultivationClock;
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::realm_vision::RealmVisionParamsV1;
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};

use super::planner::compute_base_params;
use super::view_distance_ramp::begin_view_distance_ramp;

type RealmVisionClientQueryItem<'a> = (
    Entity,
    &'a mut Client,
    &'a Cultivation,
    &'a mut ViewDistance,
);
type JoinedRealmVisionClientFilter = (With<Client>, Added<Cultivation>);

pub fn push_initial_realm_vision(
    mut commands: Commands,
    clock: Res<CultivationClock>,
    mut clients: Query<RealmVisionClientQueryItem<'_>, JoinedRealmVisionClientFilter>,
) {
    for (entity, mut client, cultivation, mut view_distance) in &mut clients {
        push_params_and_ramp(
            &mut commands,
            entity,
            &mut client,
            &mut view_distance,
            compute_base_params(cultivation.realm),
            clock.tick,
            "join",
        );
    }
}

pub fn push_realm_vision_on_breakthrough(
    mut commands: Commands,
    clock: Res<CultivationClock>,
    mut events: EventReader<BreakthroughOutcome>,
    mut clients: Query<(Entity, &mut Client, &mut ViewDistance), With<Client>>,
) {
    for event in events.read() {
        let Ok(success) = &event.result else {
            continue;
        };
        let Ok((entity, mut client, mut view_distance)) = clients.get_mut(event.entity) else {
            continue;
        };
        push_params_and_ramp(
            &mut commands,
            entity,
            &mut client,
            &mut view_distance,
            compute_base_params(success.to),
            clock.tick,
            "breakthrough",
        );
    }
}

pub fn push_realm_vision_on_revive(
    mut commands: Commands,
    clock: Res<CultivationClock>,
    mut events: EventReader<PlayerRevived>,
    mut clients: Query<RealmVisionClientQueryItem<'_>, With<Client>>,
) {
    for event in events.read() {
        let Ok((entity, mut client, cultivation, mut view_distance)) =
            clients.get_mut(event.entity)
        else {
            continue;
        };
        push_params_and_ramp(
            &mut commands,
            entity,
            &mut client,
            &mut view_distance,
            compute_base_params(cultivation.realm),
            clock.tick,
            "revive",
        );
    }
}

pub fn push_params_and_ramp(
    commands: &mut Commands,
    entity: Entity,
    client: &mut Client,
    view_distance: &mut ViewDistance,
    params: RealmVisionParamsV1,
    now_tick: u64,
    reason: &str,
) {
    begin_view_distance_ramp(
        commands,
        entity,
        view_distance,
        params.server_view_distance_chunks,
        now_tick,
    );
    send_realm_vision_params(client, params, reason);
}

pub fn send_realm_vision_params(client: &mut Client, params: RealmVisionParamsV1, reason: &str) {
    let payload = ServerDataV1::new(ServerDataPayloadV1::RealmVisionParams(params));
    let payload_type = payload_type_label(payload.payload_type());
    let bytes = match serialize_server_data_payload(&payload) {
        Ok(bytes) => bytes,
        Err(error) => {
            log_payload_build_error(payload_type, &error);
            return;
        }
    };
    send_server_data_payload(client, bytes.as_slice());
    tracing::debug!(
        "[bong][realm_vision] sent {} {} payload ({reason})",
        SERVER_DATA_CHANNEL,
        payload_type
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::breakthrough::BreakthroughSuccess;
    use crate::cultivation::components::Realm;
    use valence::prelude::{App, Events, Update};
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    fn flush_client_packets(app: &mut App) {
        let world = app.world_mut();
        let mut query = world.query::<&mut Client>();
        for mut client in query.iter_mut(world) {
            client
                .flush_packets()
                .expect("mock client packets should flush");
        }
    }

    fn collect_server_data_frames(helper: &mut MockClientHelper) -> Vec<serde_json::Value> {
        let mut frames = Vec::new();
        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };
            if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                continue;
            }
            frames.push(
                serde_json::from_slice(packet.data.0 .0)
                    .expect("server_data custom payload should decode as JSON"),
            );
        }
        frames
    }

    #[test]
    fn push_on_breakthrough() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 40 });
        app.add_event::<BreakthroughOutcome>();
        app.add_systems(Update, push_realm_vision_on_breakthrough);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .resource_mut::<Events<BreakthroughOutcome>>()
            .send(BreakthroughOutcome {
                entity,
                from: Realm::Awaken,
                result: Ok(BreakthroughSuccess {
                    to: Realm::Void,
                    success_rate: 1.0,
                    used_qi: 1.0,
                }),
            });

        app.update();
        flush_client_packets(&mut app);

        let frames = collect_server_data_frames(&mut helper);
        let payload = frames
            .iter()
            .find(|payload| {
                payload.get("type").and_then(|v| v.as_str()) == Some("realm_vision_params")
            })
            .expect("realm vision payload should be sent");
        assert_eq!(
            payload.get("fog_start").and_then(|v| v.as_f64()),
            Some(240.0)
        );
        assert_eq!(
            payload
                .get("server_view_distance_chunks")
                .and_then(|v| v.as_u64()),
            Some(20)
        );
    }

    #[test]
    fn push_on_revive() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 80 });
        app.add_event::<PlayerRevived>();
        app.add_systems(Update, push_realm_vision_on_revive);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                Cultivation {
                    realm: Realm::Condense,
                    ..Default::default()
                },
            ))
            .id();
        app.world_mut()
            .resource_mut::<Events<PlayerRevived>>()
            .send(PlayerRevived { entity });

        app.update();
        flush_client_packets(&mut app);

        let frames = collect_server_data_frames(&mut helper);
        let payload = frames
            .iter()
            .find(|payload| {
                payload.get("type").and_then(|v| v.as_str()) == Some("realm_vision_params")
            })
            .expect("realm vision payload should be sent");
        assert_eq!(
            payload.get("fog_start").and_then(|v| v.as_f64()),
            Some(80.0)
        );
        assert_eq!(
            payload
                .get("server_view_distance_chunks")
                .and_then(|v| v.as_u64()),
            Some(8)
        );
    }
}
