use std::collections::HashMap;

use valence::prelude::{Client, Entity, EventReader, Local, Position, Query, With};

use crate::cultivation::tribulation::{
    TribulationAnnounce, TribulationLocked, TribulationSettled, TribulationWaveCleared,
};
use crate::network::agent_bridge::{payload_type_label, serialize_server_data_payload};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1, TribulationBroadcastV1};

const BROADCAST_LIFETIME_MS: u64 = 60_000;
const SPECTATE_INVITE_RADIUS: f64 = 50.0;

pub fn emit_tribulation_broadcast_payloads(
    mut clients: Query<(&mut Client, Option<&Position>), With<Client>>,
    mut active_broadcasts: Local<HashMap<Entity, TribulationBroadcastV1>>,
    mut announce: EventReader<TribulationAnnounce>,
    mut locked: EventReader<TribulationLocked>,
    mut cleared: EventReader<TribulationWaveCleared>,
    mut settled: EventReader<TribulationSettled>,
) {
    for ev in announce.read() {
        let data = TribulationBroadcastV1::active(
            ev.actor_name.clone(),
            "warn",
            ev.epicenter[0],
            ev.epicenter[2],
            BROADCAST_LIFETIME_MS,
        );
        active_broadcasts.insert(ev.entity, data.clone());
        broadcast(&mut clients, data);
    }
    for ev in locked.read() {
        let data = active_broadcasts.entry(ev.entity).or_insert_with(|| {
            TribulationBroadcastV1::active(
                ev.actor_name.clone(),
                "locked",
                ev.epicenter[0],
                ev.epicenter[2],
                BROADCAST_LIFETIME_MS,
            )
        });
        data.stage = "locked".to_string();
        data.refresh(BROADCAST_LIFETIME_MS);
        broadcast(&mut clients, data.clone());
    }
    for ev in cleared.read() {
        let stage = if ev.wave == 0 { "warn" } else { "striking" };
        let data = active_broadcasts.entry(ev.entity).or_insert_with(|| {
            TribulationBroadcastV1::active("", stage, 0.0, 0.0, BROADCAST_LIFETIME_MS)
        });
        data.stage = stage.to_string();
        data.refresh(BROADCAST_LIFETIME_MS);
        broadcast(&mut clients, data.clone());
    }
    for ev in settled.read() {
        active_broadcasts.remove(&ev.entity);
        broadcast(&mut clients, TribulationBroadcastV1::clear());
    }
}

fn broadcast(
    clients: &mut Query<(&mut Client, Option<&Position>), With<Client>>,
    data: TribulationBroadcastV1,
) {
    for (mut client, position) in clients.iter_mut() {
        let payload = ServerDataV1::new(ServerDataPayloadV1::TribulationBroadcast(
            data.for_client(position),
        ));
        let payload_type = payload_type_label(payload.payload_type());
        let payload_bytes = match serialize_server_data_payload(&payload) {
            Ok(payload) => payload,
            Err(error) => {
                log_payload_build_error(payload_type, &error);
                continue;
            }
        };
        send_server_data_payload(&mut client, payload_bytes.as_slice());
    }
}

trait TribulationBroadcastClientView {
    fn for_client(&self, position: Option<&Position>) -> TribulationBroadcastV1;
}

impl TribulationBroadcastClientView for TribulationBroadcastV1 {
    fn for_client(&self, position: Option<&Position>) -> TribulationBroadcastV1 {
        let mut data = self.clone();
        if !data.active {
            return data;
        }
        let Some(position) = position else {
            data.spectate_invite = false;
            data.spectate_distance = 0.0;
            return data;
        };
        let pos = position.get();
        let dx = pos.x - data.world_x;
        let dz = pos.z - data.world_z;
        let distance = (dx * dx + dz * dz).sqrt();
        data.spectate_distance = distance;
        data.spectate_invite = distance <= SPECTATE_INVITE_RADIUS;
        data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::cultivation::tribulation::TribulationAnnounce;
    use crate::network::agent_bridge::SERVER_DATA_CHANNEL;
    use valence::prelude::{App, Update};
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    fn spawn_mock_client_at(app: &mut App, name: &str, pos: [f64; 3]) -> MockClientHelper {
        let (mut bundle, helper) = create_mock_client(name);
        bundle.player.position = Position::new(pos);
        app.world_mut().spawn(bundle);
        helper
    }

    fn flush_all_client_packets(app: &mut App) {
        let world = app.world_mut();
        let mut query = world.query::<&mut Client>();
        for mut client in query.iter_mut(world) {
            client
                .flush_packets()
                .expect("mock client packets should flush");
        }
    }

    fn collect_tribulation_broadcasts(
        helper: &mut MockClientHelper,
    ) -> Vec<TribulationBroadcastV1> {
        let mut payloads = Vec::new();
        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };
            if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                continue;
            }
            let payload: ServerDataV1 = serde_json::from_slice(packet.data.0 .0)
                .expect("server data payload should decode");
            if let ServerDataPayloadV1::TribulationBroadcast(data) = payload.payload {
                payloads.push(data);
            }
        }
        payloads
    }

    #[test]
    fn broadcast_fills_distance_per_client() {
        let mut app = App::new();
        app.add_event::<TribulationAnnounce>();
        app.add_event::<TribulationLocked>();
        app.add_event::<TribulationWaveCleared>();
        app.add_event::<TribulationSettled>();
        app.add_systems(Update, emit_tribulation_broadcast_payloads);

        let mut near = spawn_mock_client_at(&mut app, "Near", [30.0, 66.0, 40.0]);
        let mut far = spawn_mock_client_at(&mut app, "Far", [300.0, 66.0, 400.0]);
        app.world_mut().send_event(TribulationAnnounce {
            entity: Entity::PLACEHOLDER,
            char_id: "offline:Azure".to_string(),
            actor_name: "Azure".to_string(),
            epicenter: [0.0, 66.0, 0.0],
            waves_total: 3,
        });

        app.update();
        flush_all_client_packets(&mut app);

        let near_payloads = collect_tribulation_broadcasts(&mut near);
        let far_payloads = collect_tribulation_broadcasts(&mut far);
        assert_eq!(near_payloads.len(), 1);
        assert_eq!(far_payloads.len(), 1);
        assert!(near_payloads[0].spectate_invite);
        assert_eq!(near_payloads[0].spectate_distance, 50.0);
        assert!(!far_payloads[0].spectate_invite);
        assert_eq!(far_payloads[0].spectate_distance, 500.0);
        assert_eq!(near_payloads[0].world_x, 0.0);
        assert_eq!(near_payloads[0].world_z, 0.0);
        assert_eq!(near_payloads[0].actor_name, "Azure");
    }
}
