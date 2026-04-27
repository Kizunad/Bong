//! plan-weapon-v1 §8：Treasure 装备槽变更推送。
//!
//! v1.1 channel 契约：物理 CustomPayload channel 固定为 `bong:server_data`，
//! 再由 JSON `type=treasure_equipped` 分发；不注册独立
//! `bong:combat/treasure_equipped` channel。

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use valence::prelude::{App, Update};
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    use crate::inventory::{
        ContainerState, InventoryRevision, ItemRarity, ItemTemplate, WeaponSpec,
    };

    fn treasure_template() -> ItemTemplate {
        ItemTemplate {
            id: "starter_talisman".to_string(),
            display_name: "启程护符".to_string(),
            category: ItemCategory::Treasure,
            grid_w: 1,
            grid_h: 1,
            base_weight: 0.2,
            rarity: ItemRarity::Uncommon,
            spirit_quality_initial: 0.76,
            description: String::new(),
            effect: None,
            cast_duration_ms: 0,
            cooldown_ms: 0,
            weapon_spec: None::<WeaponSpec>,
        }
    }

    fn treasure_instance(instance_id: u64) -> crate::inventory::ItemInstance {
        crate::inventory::ItemInstance {
            instance_id,
            template_id: "starter_talisman".to_string(),
            display_name: "启程护符".to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.2,
            rarity: ItemRarity::Uncommon,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 0.76,
            durability: 0.93,
            freshness: None,
            mineral_id: None,
            charges: None,
        }
    }

    fn empty_inventory() -> PlayerInventory {
        PlayerInventory {
            revision: InventoryRevision(1),
            containers: vec![ContainerState {
                id: "main_pack".to_string(),
                name: "main_pack".to_string(),
                rows: 5,
                cols: 7,
                items: Vec::new(),
            }],
            equipped: Default::default(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 50.0,
        }
    }

    fn flush_client_packets(app: &mut App) {
        let world = app.world_mut();
        let mut query = world.query::<&mut Client>();
        for mut client in query.iter_mut(world) {
            client
                .flush_packets()
                .expect("mock client packets should flush");
        }
    }

    fn collect_server_data_frames(
        helper: &mut MockClientHelper,
    ) -> Vec<(String, serde_json::Value)> {
        let mut frames = Vec::new();
        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };
            let value: serde_json::Value = serde_json::from_slice(packet.data.0 .0)
                .expect("server_data custom payload should decode as JSON");
            frames.push((packet.channel.as_str().to_string(), value));
        }
        frames
    }

    #[test]
    fn treasure_equipped_uses_server_data_channel_and_type() {
        let mut app = App::new();
        app.insert_resource(ItemRegistry::from_map(HashMap::from([(
            "starter_talisman".to_string(),
            treasure_template(),
        )])));
        app.add_systems(Update, emit_treasure_equipped_payloads);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let mut inventory = empty_inventory();
        inventory.equipped.insert(
            EQUIP_SLOT_TREASURE_BELT_0.to_string(),
            treasure_instance(88),
        );
        app.world_mut().spawn((client_bundle, inventory));

        app.update();
        flush_client_packets(&mut app);

        let frames = collect_server_data_frames(&mut helper);
        let (channel, payload) = frames
            .iter()
            .find(|(_, payload)| {
                payload.get("type").and_then(|v| v.as_str()) == Some("treasure_equipped")
                    && payload.get("slot").and_then(|v| v.as_str())
                        == Some(EQUIP_SLOT_TREASURE_BELT_0)
            })
            .expect("treasure_equipped payload should be sent");
        assert_eq!(channel, SERVER_DATA_CHANNEL);
        assert_eq!(
            payload.get("slot").and_then(|v| v.as_str()),
            Some(EQUIP_SLOT_TREASURE_BELT_0)
        );
        assert_eq!(
            payload
                .get("treasure")
                .and_then(|v| v.get("template_id"))
                .and_then(|v| v.as_str()),
            Some("starter_talisman")
        );
    }
}
