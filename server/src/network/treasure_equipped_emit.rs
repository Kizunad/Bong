//! plan-weapon-v1 §8：Treasure 装备槽变更推送。
//!
//! v1.1 channel 契约：物理 CustomPayload channel 固定为 `bong:server_data`，
//! 再由 JSON `type=treasure_equipped` 分发；不注册独立
//! `bong:combat/treasure_equipped` channel。

use valence::prelude::{Changed, Client, Entity, Query, Res, With};

use crate::inventory::{
    ItemCategory, ItemRegistry, PlayerInventory, EQUIP_SLOT_OFF_HAND, TREASURE_TRIGGER_CAP,
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

/// plan-layered-equip-v1 P4（决议 #8）— 触发位 slot 命名约定：`trigger_0..trigger_(CAP-1)`。
/// 与装备槽 wire 名（off_hand 等）正交，client `TreasurePanelSync` / `WeaponHotbarHudPlanner`
/// 从这些 key 拉激活态法宝。
pub fn trigger_slot_key(index: usize) -> String {
    format!("trigger_{index}")
}

pub fn emit_treasure_equipped_payloads(
    registry: Res<ItemRegistry>,
    changed_inventories: Query<(Entity, &PlayerInventory), Changed<PlayerInventory>>,
    mut clients: Query<&mut Client, With<Client>>,
) {
    // plan-layered-equip-v1 P0.2/P4（决议 #8 / #17）— treasure_belt 装备槽取消，法宝激活态由
    // 灵宝 UI 触发位承载（trigger_0..trigger_(CAP-1)）。off_hand held treasure 仍下发作装备态展示。
    let updates: Vec<TreasureClientUpdate> = changed_inventories
        .iter()
        .map(|(entity, inventory)| {
            let mut views: Vec<TreasureSlotUpdate> = Vec::with_capacity(1 + TREASURE_TRIGGER_CAP);

            // off_hand held treasure（装备态展示，与触发位激活态正交，决议 #16）。
            let off_hand_view = inventory
                .equipped
                .get(EQUIP_SLOT_OFF_HAND)
                .and_then(|s| s.held.as_ref())
                .and_then(|item| {
                    registry
                        .get(&item.template_id)
                        .filter(|tpl| matches!(tpl.category, ItemCategory::Treasure))
                        .map(|_| treasure_view(item))
                });
            views.push((EQUIP_SLOT_OFF_HAND.to_string(), off_hand_view));

            // 触发位激活态法宝（决议 #8）：固定 CAP 个槽，空槽下发 None 以清除。
            for index in 0..TREASURE_TRIGGER_CAP {
                let view = inventory.triggered_treasures.get(index).map(treasure_view);
                views.push((trigger_slot_key(index), view));
            }

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
            placeable: None,
            max_stack_count: 1,
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
            forge_station_spec: None,
            blueprint_scroll_spec: None,
            inscription_scroll_spec: None,
            technique_scroll_spec: None,
            recipe_fragment_spec: None,
            container_spec: None,
            shelflife_profile: None,
            shield_spec: None,
            shelflife_track: None,
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
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        }
    }

    fn empty_inventory() -> PlayerInventory {
        PlayerInventory {
            triggered_treasures: Vec::new(),
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
        // plan-layered-equip-v1 决议 #8/#17：treasure_belt 槽已删，法宝以 off_hand held 形式下发。
        let mut app = App::new();
        app.insert_resource(ItemRegistry::from_map(HashMap::from([(
            "starter_talisman".to_string(),
            treasure_template(),
        )])));
        app.add_systems(Update, emit_treasure_equipped_payloads);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let mut inventory = empty_inventory();
        inventory.equipped.insert(
            EQUIP_SLOT_OFF_HAND.to_string(),
            crate::inventory::SlotContents::held_single(treasure_instance(88)),
        );
        app.world_mut().spawn((client_bundle, inventory));

        app.update();
        flush_client_packets(&mut app);

        let frames = collect_server_data_frames(&mut helper);
        let (channel, payload) = frames
            .iter()
            .find(|(_, payload)| {
                payload.get("type").and_then(|v| v.as_str()) == Some("treasure_equipped")
                    && payload.get("slot").and_then(|v| v.as_str()) == Some(EQUIP_SLOT_OFF_HAND)
            })
            .expect("off_hand treasure_equipped payload should be sent");
        assert_eq!(channel, SERVER_DATA_CHANNEL);
        assert_eq!(
            payload.get("slot").and_then(|v| v.as_str()),
            Some(EQUIP_SLOT_OFF_HAND)
        );
        assert_eq!(
            payload
                .get("treasure")
                .and_then(|v| v.get("template_id"))
                .and_then(|v| v.as_str()),
            Some("starter_talisman")
        );
    }

    // plan-layered-equip-v1 P4（决议 #8）— 触发位法宝以 trigger_<idx> slot 下发；
    // 占用槽带 treasure view，空槽下发 None（清除）。
    #[test]
    fn trigger_slot_treasure_emitted_per_index_with_empty_slots_cleared() {
        let mut app = App::new();
        app.insert_resource(ItemRegistry::from_map(HashMap::from([(
            "starter_talisman".to_string(),
            treasure_template(),
        )])));
        app.add_systems(Update, emit_treasure_equipped_payloads);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let mut inventory = empty_inventory();
        // 触发位放一件（index 0），其余槽空。
        inventory.triggered_treasures.push(treasure_instance(77));
        app.world_mut().spawn((client_bundle, inventory));

        app.update();
        flush_client_packets(&mut app);

        let frames = collect_server_data_frames(&mut helper);

        // index 0 槽应带 treasure view。
        let slot0 = frames
            .iter()
            .find(|(_, p)| {
                p.get("type").and_then(|v| v.as_str()) == Some("treasure_equipped")
                    && p.get("slot").and_then(|v| v.as_str()) == Some(trigger_slot_key(0).as_str())
            })
            .expect("trigger_0 treasure_equipped payload should be sent");
        assert_eq!(
            slot0
                .1
                .get("treasure")
                .and_then(|v| v.get("instance_id"))
                .and_then(|v| v.as_u64()),
            Some(77),
            "trigger slot 0 should carry the activated treasure instance"
        );

        // 全部 CAP 个触发位槽都应有 payload（空槽 treasure=None 清除）。
        for index in 0..TREASURE_TRIGGER_CAP {
            let key = trigger_slot_key(index);
            let frame = frames.iter().find(|(_, p)| {
                p.get("type").and_then(|v| v.as_str()) == Some("treasure_equipped")
                    && p.get("slot").and_then(|v| v.as_str()) == Some(key.as_str())
            });
            assert!(
                frame.is_some(),
                "trigger slot {key} must emit a payload (occupied or cleared)"
            );
            if index != 0 {
                let treasure = frame.unwrap().1.get("treasure");
                assert!(
                    treasure.is_none() || treasure == Some(&serde_json::Value::Null),
                    "empty trigger slot {key} should clear (treasure=None), got {treasure:?}"
                );
            }
        }
    }
}
