use valence::prelude::{bevy_ecs, Client, Position, Query, Username, With};

use crate::inventory::DroppedItemEvent;
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::inventory_snapshot_emit::item_view_from_instance;
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::inventory::{ContainerIdV1, InventoryEventV1, InventoryLocationV1};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};

pub fn emit_dropped_item_inventory_events(
    mut dropped_events: bevy_ecs::event::EventReader<DroppedItemEvent>,
    mut clients: Query<(&Username, &mut Client, &Position), With<Client>>,
) {
    for ev in dropped_events.read() {
        let Ok((_username, mut client, position)) = clients.get_mut(ev.entity) else {
            continue;
        };

        for (idx, dropped) in ev.dropped.iter().enumerate() {
            let base = position.0;
            let world_pos = [base.x + 0.35 + idx as f64 * 0.1, base.y, base.z + 0.35];
            let payload = ServerDataV1::new(ServerDataPayloadV1::InventoryEvent(
                InventoryEventV1::Dropped {
                    revision: ev.revision.0,
                    instance_id: dropped.instance.instance_id,
                    from: InventoryLocationV1::Container {
                        container_id: match dropped.container_id.as_str() {
                            "main_pack" => ContainerIdV1::MainPack,
                            "small_pouch" => ContainerIdV1::SmallPouch,
                            "front_satchel" => ContainerIdV1::FrontSatchel,
                            _ => ContainerIdV1::MainPack,
                        },
                        row: u64::from(dropped.row),
                        col: u64::from(dropped.col),
                    },
                    world_pos,
                    item: item_view_from_instance(&dropped.instance),
                },
            ));
            let payload_type = payload_type_label(payload.payload_type());
            let payload_bytes = match serialize_server_data_payload(&payload) {
                Ok(bytes) => bytes,
                Err(error) => {
                    log_payload_build_error(payload_type, &error);
                    continue;
                }
            };

            send_server_data_payload(&mut client, payload_bytes.as_slice());
            tracing::info!(
                "[bong][network] sent {} {} payload to client entity {:?} (dropped instance={})",
                SERVER_DATA_CHANNEL,
                payload_type,
                ev.entity,
                dropped.instance.instance_id,
            );
        }
    }
}
