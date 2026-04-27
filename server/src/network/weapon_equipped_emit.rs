//! plan-weapon-v1 §8：装备槽变更 / 武器损坏推送。
//!
//! v1.1 channel 契约：物理 CustomPayload channel 固定为 `bong:server_data`，
//! 再由 JSON `type=weapon_equipped|weapon_broken` 分发；不注册独立
//! `bong:combat/weapon_*` channel。
//!
//! 两条管线：
//! 1. [`emit_weapon_equipped_payloads`]：对 `Changed<PlayerInventory>` 的玩家推送
//!    `main_hand / off_hand / two_hand` 三槽 snapshot。这样即使 v1 的 runtime
//!    `Weapon` component 只保留一个当前战斗武器，HUD 仍能拿到三槽装备态。
//! 2. [`emit_weapon_broken_payloads`]：消费 [`WeaponBroken`] 事件推送
//!    `WeaponBrokenV1 { instance_id, template_id }`。

use valence::prelude::{Changed, Client, Entity, EventReader, Query, Res, With};

use crate::combat::weapon::{WeaponBroken, WeaponKind};
use crate::inventory::{ItemRegistry, PlayerInventory};
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::combat_hud::{WeaponBrokenV1, WeaponEquippedV1, WeaponViewV1};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};

type WeaponSlotUpdate = (String, Option<WeaponViewV1>);
type WeaponClientUpdate = (Entity, Vec<WeaponSlotUpdate>);

fn slot_wire_name(slot: crate::combat::weapon::EquipSlot) -> &'static str {
    match slot {
        crate::combat::weapon::EquipSlot::MainHand => "main_hand",
        crate::combat::weapon::EquipSlot::OffHand => "off_hand",
        crate::combat::weapon::EquipSlot::TwoHand => "two_hand",
    }
}

fn item_to_view(
    item: &crate::inventory::ItemInstance,
    spec: &crate::inventory::WeaponSpec,
) -> WeaponViewV1 {
    WeaponViewV1 {
        instance_id: item.instance_id,
        template_id: item.template_id.clone(),
        weapon_kind: weapon_kind_str(spec.weapon_kind).to_string(),
        durability_current: (item.durability as f32) * spec.durability_max,
        durability_max: spec.durability_max,
        quality_tier: spec.quality_tier,
    }
}

fn weapon_kind_str(k: WeaponKind) -> &'static str {
    match k {
        WeaponKind::Sword => "sword",
        WeaponKind::Saber => "saber",
        WeaponKind::Staff => "staff",
        WeaponKind::Fist => "fist",
        WeaponKind::Spear => "spear",
        WeaponKind::Dagger => "dagger",
        WeaponKind::Bow => "bow",
    }
}

fn send_weapon_equipped(client: &mut Client, slot: &str, weapon: Option<WeaponViewV1>) {
    let payload = ServerDataV1::new(ServerDataPayloadV1::WeaponEquipped(WeaponEquippedV1 {
        slot: slot.to_string(),
        weapon,
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

fn send_weapon_broken(client: &mut Client, instance_id: u64, template_id: &str) {
    let payload = ServerDataV1::new(ServerDataPayloadV1::WeaponBroken(WeaponBrokenV1 {
        instance_id,
        template_id: template_id.to_string(),
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
    tracing::info!(
        "[bong][network] sent {} {} payload instance={instance_id} template={template_id}",
        SERVER_DATA_CHANNEL,
        type_label
    );
}

/// plan-weapon-v1 §8.1：推送 `weapon_equipped` payload。
///
/// 对 inventory 的每次 revision 变化，推三槽 snapshot。
pub fn emit_weapon_equipped_payloads(
    registry: Res<ItemRegistry>,
    changed_inventories: Query<(Entity, &PlayerInventory), Changed<PlayerInventory>>,
    mut clients: Query<&mut Client, With<Client>>,
) {
    let updates: Vec<WeaponClientUpdate> = changed_inventories
        .iter()
        .map(|(entity, inventory)| {
            let slots = [
                (crate::combat::weapon::EquipSlot::MainHand, "main_hand"),
                (crate::combat::weapon::EquipSlot::OffHand, "off_hand"),
                (crate::combat::weapon::EquipSlot::TwoHand, "two_hand"),
            ]
            .into_iter()
            .map(|(slot, key)| {
                let view = inventory.equipped.get(key).and_then(|item| {
                    registry
                        .get(&item.template_id)
                        .and_then(|tpl| tpl.weapon_spec.as_ref())
                        .map(|spec| item_to_view(item, spec))
                });
                (slot_wire_name(slot).to_string(), view)
            })
            .collect();
            (entity, slots)
        })
        .collect();

    for (entity, slots) in updates {
        if let Ok(mut client) = clients.get_mut(entity) {
            for (slot, view) in slots {
                send_weapon_equipped(&mut client, &slot, view);
            }
        }
    }
}

/// plan-weapon-v1 §6.3：消费 [`WeaponBroken`] 事件并推送到对应玩家 client。
pub fn emit_weapon_broken_payloads(
    mut events: EventReader<WeaponBroken>,
    mut clients: Query<&mut Client, With<Client>>,
) {
    let broken: Vec<WeaponBroken> = events.read().cloned().collect();
    for ev in broken {
        if let Ok(mut client) = clients.get_mut(ev.entity) {
            send_weapon_broken(&mut client, ev.instance_id, &ev.template_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use valence::prelude::{App, Events, Update};
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    use crate::combat::weapon::EquipSlot;
    use crate::inventory::{
        ContainerState, InventoryRevision, ItemCategory, ItemInstance, ItemRarity, ItemTemplate,
        WeaponSpec,
    };

    fn weapon_template() -> ItemTemplate {
        ItemTemplate {
            id: "iron_sword".to_string(),
            display_name: "铁剑".to_string(),
            category: ItemCategory::Weapon,
            grid_w: 1,
            grid_h: 3,
            base_weight: 2.6,
            rarity: ItemRarity::Common,
            spirit_quality_initial: 0.2,
            description: String::new(),
            effect: None,
            cast_duration_ms: 0,
            cooldown_ms: 0,
            weapon_spec: Some(WeaponSpec {
                weapon_kind: WeaponKind::Sword,
                base_attack: 8.0,
                quality_tier: 1,
                durability_max: 200.0,
                qi_cost_mul: 1.0,
            }),
        }
    }

    fn weapon_instance(instance_id: u64) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: "iron_sword".to_string(),
            display_name: "铁剑".to_string(),
            grid_w: 1,
            grid_h: 3,
            weight: 2.6,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 0.2,
            durability: 0.925,
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
    fn weapon_equipped_uses_server_data_channel_and_type() {
        let mut app = App::new();
        app.insert_resource(ItemRegistry::from_map(HashMap::from([(
            "iron_sword".to_string(),
            weapon_template(),
        )])));
        app.add_systems(Update, emit_weapon_equipped_payloads);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let mut inventory = empty_inventory();
        inventory
            .equipped
            .insert("main_hand".to_string(), weapon_instance(42));
        app.world_mut().spawn((client_bundle, inventory));

        app.update();
        flush_client_packets(&mut app);

        let frames = collect_server_data_frames(&mut helper);
        let (channel, payload) = frames
            .iter()
            .find(|(_, payload)| {
                payload.get("type").and_then(|v| v.as_str()) == Some("weapon_equipped")
            })
            .expect("weapon_equipped payload should be sent");
        assert_eq!(channel, SERVER_DATA_CHANNEL);
        assert_eq!(
            payload.get("slot").and_then(|v| v.as_str()),
            Some("main_hand")
        );
        assert_eq!(
            payload
                .get("weapon")
                .and_then(|v| v.get("template_id"))
                .and_then(|v| v.as_str()),
            Some("iron_sword")
        );
    }

    #[test]
    fn weapon_broken_uses_server_data_channel_and_type() {
        let mut app = App::new();
        app.add_event::<WeaponBroken>();
        app.add_systems(Update, emit_weapon_broken_payloads);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .resource_mut::<Events<WeaponBroken>>()
            .send(WeaponBroken {
                entity,
                instance_id: 77,
                template_id: "iron_sword".to_string(),
            });

        app.update();
        flush_client_packets(&mut app);

        let frames = collect_server_data_frames(&mut helper);
        let (channel, payload) = frames
            .iter()
            .find(|(_, payload)| {
                payload.get("type").and_then(|v| v.as_str()) == Some("weapon_broken")
            })
            .expect("weapon_broken payload should be sent");
        assert_eq!(channel, SERVER_DATA_CHANNEL);
        assert_eq!(
            payload.get("instance_id").and_then(|v| v.as_u64()),
            Some(77)
        );
        assert_eq!(
            payload.get("template_id").and_then(|v| v.as_str()),
            Some("iron_sword")
        );
    }

    #[test]
    fn weapon_equipped_payload_label_matches_wire_type() {
        let weapon = WeaponViewV1 {
            instance_id: 42,
            template_id: "iron_sword".to_string(),
            weapon_kind: "sword".to_string(),
            durability_current: 185.0,
            durability_max: 200.0,
            quality_tier: 1,
        };
        let payload = ServerDataV1::new(ServerDataPayloadV1::WeaponEquipped(WeaponEquippedV1 {
            slot: "main_hand".to_string(),
            weapon: Some(weapon),
        }));
        let label = payload_type_label(payload.payload_type());
        let value: serde_json::Value = serde_json::to_value(&payload).expect("serialize");
        assert_eq!(label, "weapon_equipped");
        assert_eq!(value.get("type").and_then(|v| v.as_str()), Some(label));
    }

    #[test]
    fn equip_slot_wire_name_stays_server_data_payload_field() {
        assert_eq!(slot_wire_name(EquipSlot::MainHand), "main_hand");
        assert_eq!(slot_wire_name(EquipSlot::OffHand), "off_hand");
        assert_eq!(slot_wire_name(EquipSlot::TwoHand), "two_hand");
    }
}
