use valence::prelude::{Client, With};

use crate::cultivation::burst_meridian::BurstMeridianEvent;
use crate::network::agent_bridge::{payload_type_label, serialize_server_data_payload};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::server_data::ServerDataV1;

pub fn emit_burst_meridian_events(world: &mut valence::prelude::bevy_ecs::world::World) {
    let events: Vec<BurstMeridianEvent> = world
        .resource_mut::<valence::prelude::Events<BurstMeridianEvent>>()
        .drain()
        .collect();
    if events.is_empty() {
        return;
    }

    let payloads: Vec<Vec<u8>> = events
        .iter()
        .filter_map(|event| {
            let payload = ServerDataV1::new(event.to_payload(world));
            let payload_type = payload_type_label(payload.payload_type());
            match serialize_server_data_payload(&payload) {
                Ok(bytes) => Some(bytes),
                Err(error) => {
                    log_payload_build_error(payload_type, &error);
                    None
                }
            }
        })
        .collect();

    let mut clients = world.query_filtered::<&mut Client, With<Client>>();
    for mut client in clients.iter_mut(world) {
        for payload in &payloads {
            send_server_data_payload(&mut client, payload.as_slice());
        }
    }
}
