//! plan-weapon-v1 §8：Treasure 装备槽变更推送。

use valence::prelude::{Changed, Client, Entity, Query, Res, With};

use crate::inventory::{
    ItemCategory, ItemRegistry, PlayerInventory, EQUIP_SLOT_OFF_HAND, EQUIP_SLOT_TREASURE_BELT_0,
    EQUIP_SLOT_TREASURE_BELT_1, EQUIP_SLOT_TREASURE_BELT_2, EQUIP_SLOT_TREASURE_BELT_3,
};
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::combat_hud::{TreasureEquippedV1, TreasureViewV1};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};

type TreasureSlotUpdate = (String, Option<TreasureViewV1>);
type TreasureClientUpdate = (Entity, Vec<TreasureSlotUpdate>);

fn send_treasure_equipped(client: &mut Client, slot: &str, treasure: Option<TreasureViewV1>) {
    let payload = ServerDataV1::new(ServerDataPayloadV1::TreasureEquipped(TreasureEquippedV1 {
        slot: slot.to_string(),
        treasure,
    }));
    let type_label = payload_type_label(payload.payload_type());
    let bytes = match serialize_server_data_payload(&payload) {
        Ok(bytes) => bytes,
        Err(err) => {
            log_payload_build_error(type_label, &err);
            return;
        }
    };
    send_server_data_payload(client, bytes.as_slice());
    tracing::debug!(
        "[bong][network] sent {} {} payload slot={slot}",
        SERVER_DATA_CHANNEL,
        type_label
    );
}

fn treasure_view(item: &crate::inventory::ItemInstance) -> TreasureViewV1 {
    TreasureViewV1 {
        instance_id: item.instance_id,
        template_id: item.template_id.clone(),
        display_name: item.display_name.clone(),
    }
}

pub fn emit_treasure_equipped_payloads(
    registry: Res<ItemRegistry>,
    changed_inventories: Query<(Entity, &PlayerInventory), Changed<PlayerInventory>>,
    mut clients: Query<&mut Client, With<Client>>,
) {
    let slots = [
        EQUIP_SLOT_OFF_HAND,
        EQUIP_SLOT_TREASURE_BELT_0,
        EQUIP_SLOT_TREASURE_BELT_1,
        EQUIP_SLOT_TREASURE_BELT_2,
        EQUIP_SLOT_TREASURE_BELT_3,
    ];

    let updates: Vec<TreasureClientUpdate> = changed_inventories
        .iter()
        .map(|(entity, inventory)| {
            let views = slots
                .into_iter()
                .map(|slot| {
                    let view = inventory.equipped.get(slot).and_then(|item| {
                        registry
                            .get(&item.template_id)
                            .filter(|tpl| matches!(tpl.category, ItemCategory::Treasure))
                            .map(|_| treasure_view(item))
                    });
                    (slot.to_string(), view)
                })
                .collect();
            (entity, views)
        })
        .collect();

    for (entity, slots) in updates {
        if let Ok(mut client) = clients.get_mut(entity) {
            for (slot, view) in slots {
                send_treasure_equipped(&mut client, &slot, view);
            }
        }
    }
}
