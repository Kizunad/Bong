use std::collections::HashMap;

use valence::prelude::{Client, Entity, EventReader, Local, Query};

use crate::cultivation::tribulation::{
    TribulationAnnounce, TribulationSettled, TribulationWaveCleared,
};
use crate::network::agent_bridge::{payload_type_label, serialize_server_data_payload};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1, TribulationBroadcastV1};

const BROADCAST_LIFETIME_MS: u64 = 60_000;

pub fn emit_tribulation_broadcast_payloads(
    mut clients: Query<&mut Client>,
    mut active_broadcasts: Local<HashMap<Entity, TribulationBroadcastV1>>,
    mut announce: EventReader<TribulationAnnounce>,
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

fn broadcast(clients: &mut Query<&mut Client>, data: TribulationBroadcastV1) {
    let payload = ServerDataV1::new(ServerDataPayloadV1::TribulationBroadcast(data));
    let payload_type = payload_type_label(payload.payload_type());
    let payload_bytes = match serialize_server_data_payload(&payload) {
        Ok(payload) => payload,
        Err(error) => {
            log_payload_build_error(payload_type, &error);
            return;
        }
    };
    for mut client in clients.iter_mut() {
        send_server_data_payload(&mut client, payload_bytes.as_slice());
    }
}
