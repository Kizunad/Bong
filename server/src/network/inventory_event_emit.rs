use valence::prelude::{bevy_ecs, Client, Position, Query, Username, With};

use crate::combat::armor::ArmorProfileRegistry;
use crate::inventory::{
    DroppedItemEvent, InventoryDurabilityChangedEvent, ItemInstance, PlayerInventory,
    EQUIP_SLOT_CHEST, EQUIP_SLOT_FEET, EQUIP_SLOT_HEAD, EQUIP_SLOT_LEGS,
};
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::inventory_snapshot_emit::item_view_from_instance;
use crate::network::redis_bridge::RedisOutbound;
use crate::network::RedisBridgeResource;
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::armor_event::ArmorDurabilityChangedV1;
use crate::schema::inventory::EquipSlotV1;
use crate::schema::inventory::{ContainerIdV1, InventoryEventV1, InventoryLocationV1};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};

pub fn emit_durability_changed_inventory_events(
    mut events: bevy_ecs::event::EventReader<InventoryDurabilityChangedEvent>,
    mut clients: Query<(&Username, &mut Client), With<Client>>,
) {
    for ev in events.read() {
        let Ok((_username, mut client)) = clients.get_mut(ev.entity) else {
            continue;
        };

        let payload = ServerDataV1::new(ServerDataPayloadV1::InventoryEvent(
            InventoryEventV1::DurabilityChanged {
                revision: ev.revision.0,
                instance_id: ev.instance_id,
                durability: ev.durability,
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
        tracing::debug!(
            "[bong][network] sent {} {} payload to client entity {:?} (durability instance={} value={})",
            SERVER_DATA_CHANNEL,
            payload_type,
            ev.entity,
            ev.instance_id,
            ev.durability
        );
    }
}

pub fn publish_armor_durability_changed_events(
    mut events: bevy_ecs::event::EventReader<InventoryDurabilityChangedEvent>,
    redis: bevy_ecs::system::Res<RedisBridgeResource>,
    armor_profiles: bevy_ecs::system::Res<ArmorProfileRegistry>,
    inventories: Query<&PlayerInventory>,
    usernames: Query<&Username, With<Client>>,
) {
    for ev in events.read() {
        let Ok(inventory) = inventories.get(ev.entity) else {
            continue;
        };
        let Some((slot, item)) = equipped_armor_for_instance(inventory, ev.instance_id) else {
            continue;
        };
        let Some(profile) = armor_profiles.get(item.template_id.as_str()) else {
            continue;
        };

        let max = f64::from(profile.durability_max);
        if max <= 0.0 {
            continue;
        }
        let ratio = ev.durability.clamp(0.0, 1.0);
        let cur = ratio * max;
        let entity_id = usernames
            .get(ev.entity)
            .map(|username| crate::player::state::canonical_player_id(username.0.as_str()))
            .unwrap_or_else(|_| format!("entity:{}", ev.entity.to_bits()));

        let payload = ArmorDurabilityChangedV1 {
            v: 1,
            entity_id,
            slot,
            instance_id: ev.instance_id,
            template_id: item.template_id.clone(),
            cur,
            max,
            durability_ratio: ratio,
            broken: ratio <= 0.0,
        };
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::ArmorDurabilityChanged(payload));
    }
}

fn equipped_armor_for_instance(
    inventory: &PlayerInventory,
    instance_id: u64,
) -> Option<(EquipSlotV1, &ItemInstance)> {
    [
        (EQUIP_SLOT_HEAD, EquipSlotV1::Head),
        (EQUIP_SLOT_CHEST, EquipSlotV1::Chest),
        (EQUIP_SLOT_LEGS, EquipSlotV1::Legs),
        (EQUIP_SLOT_FEET, EquipSlotV1::Feet),
    ]
    .into_iter()
    .find_map(|(slot_key, slot)| {
        let item = inventory.equipped.get(slot_key)?;
        (item.instance_id == instance_id).then_some((slot, item))
    })
}

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

#[cfg(test)]
mod tests {
    use super::*;

    use crossbeam_channel::unbounded;
    use std::collections::HashMap;
    use valence::prelude::{App, Update};

    use crate::combat::armor::ArmorProfile;
    use crate::combat::components::{BodyPart, WoundKind};
    use crate::inventory::{ContainerState, InventoryRevision, ItemRarity};

    fn make_item(instance_id: u64, template_id: &str, durability: f64) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: template_id.to_string(),
            display_name: template_id.to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 1.0,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 1.0,
            durability,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
        }
    }

    fn make_inventory(slot_key: &str, item: ItemInstance) -> PlayerInventory {
        PlayerInventory {
            revision: InventoryRevision(7),
            containers: vec![ContainerState {
                id: crate::inventory::MAIN_PACK_CONTAINER_ID.to_string(),
                name: "主背包".to_string(),
                rows: 5,
                cols: 7,
                items: vec![],
            }],
            equipped: HashMap::from([(slot_key.to_string(), item)]),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 50.0,
        }
    }

    fn armor_registry() -> ArmorProfileRegistry {
        ArmorProfileRegistry::from_map(HashMap::from([(
            "fake_spirit_hide".to_string(),
            ArmorProfile {
                slot: EquipSlotV1::Chest,
                body_coverage: vec![BodyPart::Chest, BodyPart::Abdomen],
                kind_mitigation: HashMap::from([(WoundKind::Blunt, 0.30)]),
                durability_max: 100,
                broken_multiplier: 0.3,
            },
        )]))
    }

    #[test]
    fn publishes_agent_armor_durability_event_for_equipped_armor() {
        let mut app = App::new();
        let (tx_outbound, rx_outbound) = unbounded();
        let (_tx_inbound, rx_inbound) = unbounded();

        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.insert_resource(armor_registry());
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, publish_armor_durability_changed_events);

        let entity = app
            .world_mut()
            .spawn(make_inventory(
                EQUIP_SLOT_CHEST,
                make_item(88, "fake_spirit_hide", 1.0),
            ))
            .id();

        app.world_mut().send_event(InventoryDurabilityChangedEvent {
            entity,
            revision: InventoryRevision(8),
            instance_id: 88,
            durability: 0.0,
        });

        app.update();

        let payload = rx_outbound.try_recv().expect("armor event should publish");
        match payload {
            RedisOutbound::ArmorDurabilityChanged(event) => {
                assert_eq!(event.v, 1);
                assert_eq!(event.slot, EquipSlotV1::Chest);
                assert_eq!(event.instance_id, 88);
                assert_eq!(event.template_id, "fake_spirit_hide");
                assert_eq!(event.cur, 0.0);
                assert_eq!(event.max, 100.0);
                assert_eq!(event.durability_ratio, 0.0);
                assert!(event.broken);
            }
            other => panic!("expected ArmorDurabilityChanged, got {other:?}"),
        }
    }

    #[test]
    fn skips_non_armor_durability_events_for_agent_channel() {
        let mut app = App::new();
        let (tx_outbound, rx_outbound) = unbounded();
        let (_tx_inbound, rx_inbound) = unbounded();

        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.insert_resource(armor_registry());
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, publish_armor_durability_changed_events);

        let entity = app
            .world_mut()
            .spawn(make_inventory(
                crate::inventory::EQUIP_SLOT_MAIN_HAND,
                make_item(99, "training_blade", 0.5),
            ))
            .id();

        app.world_mut().send_event(InventoryDurabilityChangedEvent {
            entity,
            revision: InventoryRevision(8),
            instance_id: 99,
            durability: 0.4,
        });

        app.update();

        assert!(rx_outbound.try_recv().is_err());
    }
}
