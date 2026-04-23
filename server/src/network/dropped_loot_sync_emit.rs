use valence::prelude::{Added, Client, Entity, Query, Username, With};

use crate::inventory::{dropped_loot_snapshot, DroppedLootEntry, DroppedLootRegistry};
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::inventory_snapshot_emit::item_view_from_instance;
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::server_data::{DroppedLootEntryV1, ServerDataPayloadV1, ServerDataV1};

type JoinedDropSyncClient<'a> = (Entity, &'a Username, &'a mut Client);
type JoinedDropSyncClientFilter = (With<Client>, Added<Client>);

pub fn send_dropped_loot_sync_to_client(
    entity: Entity,
    client: &mut Client,
    registry: &DroppedLootRegistry,
) {
    let drops = dropped_loot_snapshot(registry, entity)
        .into_iter()
        .map(to_wire_entry)
        .collect::<Vec<_>>();

    let payload = ServerDataV1::new(ServerDataPayloadV1::DroppedLootSync(drops));
    let payload_type = payload_type_label(payload.payload_type());
    let payload_bytes = match serialize_server_data_payload(&payload) {
        Ok(bytes) => bytes,
        Err(error) => {
            log_payload_build_error(payload_type, &error);
            return;
        }
    };
    send_server_data_payload(client, payload_bytes.as_slice());
    tracing::info!(
        "[bong][network] sent {} {} payload to client entity {:?}",
        SERVER_DATA_CHANNEL,
        payload_type,
        entity,
    );
}

pub fn emit_join_dropped_loot_syncs(
    registry: valence::prelude::Res<DroppedLootRegistry>,
    mut clients: Query<JoinedDropSyncClient<'_>, JoinedDropSyncClientFilter>,
) {
    for (entity, _username, mut client) in &mut clients {
        send_dropped_loot_sync_to_client(entity, &mut client, &registry);
    }
}

fn to_wire_entry(entry: DroppedLootEntry) -> DroppedLootEntryV1 {
    DroppedLootEntryV1 {
        instance_id: entry.instance_id,
        source_container_id: entry.source_container_id,
        source_row: u64::from(entry.source_row),
        source_col: u64::from(entry.source_col),
        world_pos: entry.world_pos,
        item: item_view_from_instance(&entry.item),
    }
}
