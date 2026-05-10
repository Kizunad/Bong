use valence::prelude::{bevy_ecs, Client, Entity, Event, EventReader, Query, UniqueId, Username};

use crate::cultivation::components::{Cultivation, QiColor, Realm};
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::player::state::canonical_player_id;
use crate::schema::server_data::{QiColorObservedV1, ServerDataPayloadV1, ServerDataV1};

#[derive(Debug, Clone, Event, PartialEq, Eq)]
pub struct QiColorInspectRequest {
    pub observer: Entity,
    pub observed: Entity,
    pub requested_at_tick: u64,
}

pub fn emit_qi_color_observed_payloads(
    mut requests: EventReader<QiColorInspectRequest>,
    mut clients: Query<&mut Client>,
    cultivations: Query<&Cultivation>,
    qi_colors: Query<&QiColor>,
    identities: Query<(Option<&Username>, Option<&UniqueId>)>,
) {
    for request in requests.read() {
        let Ok(observer_cultivation) = cultivations.get(request.observer) else {
            continue;
        };
        let Ok(observed_cultivation) = cultivations.get(request.observed) else {
            continue;
        };
        let Ok(observed_color) = qi_colors.get(request.observed) else {
            continue;
        };

        let realm_diff = i32::from(realm_rank(observer_cultivation.realm))
            - i32::from(realm_rank(observed_cultivation.realm));
        if realm_diff <= 0 {
            continue;
        }

        let payload = if realm_diff >= 2 {
            QiColorObservedV1 {
                observer: entity_wire_id(&identities, request.observer),
                observed: entity_wire_id(&identities, request.observed),
                main: observed_color.main,
                secondary: observed_color.secondary,
                is_chaotic: observed_color.is_chaotic,
                is_hunyuan: observed_color.is_hunyuan,
                realm_diff,
            }
        } else {
            QiColorObservedV1 {
                observer: entity_wire_id(&identities, request.observer),
                observed: entity_wire_id(&identities, request.observed),
                main: observed_color.main,
                secondary: None,
                is_chaotic: false,
                is_hunyuan: false,
                realm_diff,
            }
        };

        let envelope = ServerDataV1::new(ServerDataPayloadV1::QiColorObserved(payload));
        let label = payload_type_label(envelope.payload_type());
        let bytes = match serialize_server_data_payload(&envelope) {
            Ok(bytes) => bytes,
            Err(error) => {
                tracing::warn!(
                    "[bong][network] failed to serialize {label} for {:?}: {error:?}",
                    request.observer
                );
                continue;
            }
        };

        let Ok(mut client) = clients.get_mut(request.observer) else {
            continue;
        };
        let _ = SERVER_DATA_CHANNEL;
        client.send_custom_payload(valence::ident!("bong:server_data"), &bytes);
    }
}

fn entity_wire_id(
    identities: &Query<(Option<&Username>, Option<&UniqueId>)>,
    entity: Entity,
) -> String {
    if let Ok((username, unique_id)) = identities.get(entity) {
        if let Some(username) = username {
            return canonical_player_id(username.0.as_str());
        }
        if let Some(unique_id) = unique_id {
            return format!("player:{}", unique_id.0);
        }
    }
    format!("entity_bits:{}", entity.to_bits())
}

fn realm_rank(realm: Realm) -> u8 {
    match realm {
        Realm::Awaken => 0,
        Realm::Induce => 1,
        Realm::Condense => 2,
        Realm::Solidify => 3,
        Realm::Spirit => 4,
        Realm::Void => 5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::{App, Position, Update};
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    use crate::cultivation::components::ColorKind;

    fn spawn_observer(
        app: &mut App,
        name: &str,
        realm: Realm,
        color: QiColor,
    ) -> (Entity, MockClientHelper) {
        let (mut client_bundle, helper) = create_mock_client(name);
        client_bundle.player.position = Position::new([0.0, 64.0, 0.0]);
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                Cultivation {
                    realm,
                    ..Default::default()
                },
                color,
            ))
            .id();
        (entity, helper)
    }

    fn spawn_observed(app: &mut App, name: &str, realm: Realm, color: QiColor) -> Entity {
        app.world_mut()
            .spawn((
                Username(name.to_string()),
                Position::new([0.0, 64.0, 0.0]),
                Cultivation {
                    realm,
                    ..Default::default()
                },
                color,
            ))
            .id()
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
    fn full_visibility_requires_two_realm_advantage() {
        let mut app = App::new();
        app.add_event::<QiColorInspectRequest>();
        app.add_systems(Update, emit_qi_color_observed_payloads);
        let (observer, mut helper) =
            spawn_observer(&mut app, "Observer", Realm::Spirit, QiColor::default());
        let observed = spawn_observed(
            &mut app,
            "Observed",
            Realm::Induce,
            QiColor {
                main: ColorKind::Intricate,
                secondary: Some(ColorKind::Heavy),
                is_chaotic: true,
                is_hunyuan: false,
                ..Default::default()
            },
        );

        app.world_mut().send_event(QiColorInspectRequest {
            observer,
            observed,
            requested_at_tick: 42,
        });
        app.update();
        flush_all_client_packets(&mut app);

        let payloads = drained_payloads(&mut helper);
        assert_eq!(payloads.len(), 1);
        assert_eq!(payloads[0]["type"], "qi_color_observed");
        assert_eq!(payloads[0]["observer"], "offline:Observer");
        assert_eq!(payloads[0]["observed"], "offline:Observed");
        assert_eq!(payloads[0]["main"], "Intricate");
        assert_eq!(payloads[0]["secondary"], "Heavy");
        assert_eq!(payloads[0]["is_chaotic"], true);
        assert_eq!(payloads[0]["realm_diff"], 3);
    }

    #[test]
    fn one_realm_advantage_only_emits_main_color() {
        let mut app = App::new();
        app.add_event::<QiColorInspectRequest>();
        app.add_systems(Update, emit_qi_color_observed_payloads);
        let (observer, mut helper) =
            spawn_observer(&mut app, "Observer", Realm::Condense, QiColor::default());
        let observed = spawn_observed(
            &mut app,
            "Observed",
            Realm::Induce,
            QiColor {
                main: ColorKind::Insidious,
                secondary: Some(ColorKind::Solid),
                is_chaotic: true,
                is_hunyuan: false,
                ..Default::default()
            },
        );

        app.world_mut().send_event(QiColorInspectRequest {
            observer,
            observed,
            requested_at_tick: 42,
        });
        app.update();
        flush_all_client_packets(&mut app);

        let payloads = drained_payloads(&mut helper);
        assert_eq!(payloads.len(), 1);
        assert_eq!(payloads[0]["main"], "Insidious");
        assert!(payloads[0].get("secondary").is_none());
        assert_eq!(payloads[0]["is_chaotic"], false);
        assert_eq!(payloads[0]["realm_diff"], 1);
    }

    #[test]
    fn equal_or_lower_realm_emits_nothing() {
        let mut app = App::new();
        app.add_event::<QiColorInspectRequest>();
        app.add_systems(Update, emit_qi_color_observed_payloads);
        let (observer, mut helper) =
            spawn_observer(&mut app, "Observer", Realm::Induce, QiColor::default());
        let observed = spawn_observed(
            &mut app,
            "Observed",
            Realm::Condense,
            QiColor {
                main: ColorKind::Heavy,
                ..Default::default()
            },
        );

        app.world_mut().send_event(QiColorInspectRequest {
            observer,
            observed,
            requested_at_tick: 42,
        });
        app.update();

        flush_all_client_packets(&mut app);

        assert!(drained_payloads(&mut helper).is_empty());
    }
}
