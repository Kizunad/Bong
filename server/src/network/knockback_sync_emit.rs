use valence::prelude::{Client, Entity, EventReader, Query, Username};

use crate::combat::knockback::KnockbackEvent;
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::server_data::{KnockbackSyncV1, ServerDataPayloadV1, ServerDataV1};

pub fn emit_knockback_sync_to_client(
    mut knockback_reader: EventReader<KnockbackEvent>,
    mut clients: Query<(Entity, &Username, &mut Client)>,
) {
    for event in knockback_reader.read() {
        let sync = KnockbackSyncV1 {
            distance_blocks: event.distance_blocks,
            velocity_blocks_per_tick: event.velocity_blocks_per_tick,
            duration_ticks: event.duration_ticks,
            kinetic_energy: event.kinetic_energy,
            collision_damage: event.collision_damage,
            chain_depth: event.chain_depth,
            block_broken: event.block_broken,
        };
        send_knockback_sync(&mut clients, event.target, sync);
    }
}

fn send_knockback_sync(
    clients: &mut Query<(Entity, &Username, &mut Client)>,
    target: Entity,
    sync: KnockbackSyncV1,
) {
    let Ok((_ent, username, mut client)) = clients.get_mut(target) else {
        return;
    };

    let payload = ServerDataV1::new(ServerDataPayloadV1::KnockbackSync(sync));
    let payload_type = payload_type_label(payload.payload_type());
    let payload_bytes = match serialize_server_data_payload(&payload) {
        Ok(bytes) => bytes,
        Err(error) => {
            log_payload_build_error(payload_type, &error);
            return;
        }
    };

    send_server_data_payload(&mut client, payload_bytes.as_slice());
    tracing::debug!(
        "[bong][network] sent {} {} payload to `{}`",
        SERVER_DATA_CHANNEL,
        payload_type,
        username.0
    );
}
