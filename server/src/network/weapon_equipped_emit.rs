//! plan-weapon-v1 §8：装备槽变更 / 武器损坏 推送。
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
