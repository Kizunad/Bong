//! plan-weapon-v1 §8：装备槽变更 / 武器损坏 推送。
//!
//! 两条管线：
//! 1. [`emit_weapon_equipped_payloads`]：对 `Changed<Weapon>` 的玩家推送
//!    `WeaponEquippedV1 { slot, weapon: Some(view) }`；对 `RemovedComponents<Weapon>`
//!    推 `weapon: None`。触发时机覆盖 `sync_weapon_component_from_equipped` 插入 /
//!    移除 / 属性变更（耐久扣减）三种情况。
//! 2. [`emit_weapon_broken_payloads`]：消费 [`WeaponBroken`] 事件推送
//!    `WeaponBrokenV1 { instance_id, template_id }`。

use valence::prelude::{Changed, Client, Entity, EventReader, Query, RemovedComponents, With};

use crate::combat::weapon::{Weapon, WeaponBroken, WeaponKind};
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::combat_hud::{SoulBondV1, WeaponBrokenV1, WeaponEquippedV1, WeaponViewV1};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};

const MAIN_HAND_SLOT: &str = "main_hand";

/// 把 runtime `Weapon` → wire `WeaponViewV1`。
fn weapon_to_view(w: &Weapon) -> WeaponViewV1 {
    WeaponViewV1 {
        instance_id: w.instance_id,
        template_id: w.template_id.clone(),
        weapon_kind: weapon_kind_str(w.weapon_kind).to_string(),
        durability_current: w.durability,
        durability_max: w.durability_max,
        quality_tier: w.quality_tier,
        soul_bond: w.soul_bond.as_ref().map(|b| SoulBondV1 {
            character_id: b.character_id.clone(),
            bond_level: b.bond_level,
            bond_progress: b.bond_progress,
        }),
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
/// 三类触发：
/// - `Changed<Weapon>` 包含 `Added<Weapon>` 和后续字段变动 → push with Some
/// - `RemovedComponents<Weapon>` → push with None（卸下 / broken）
///
/// 先收集 snapshots 释放 query 借用，再分别 write 到同一 `clients` query。
pub fn emit_weapon_equipped_payloads(
    changed_weapons: Query<(Entity, &Weapon), Changed<Weapon>>,
    mut clients: Query<&mut Client, With<Client>>,
    mut removed: RemovedComponents<Weapon>,
) {
    let updates: Vec<(Entity, WeaponViewV1)> = changed_weapons
        .iter()
        .map(|(e, w)| (e, weapon_to_view(w)))
        .collect();
    let removed_entities: Vec<Entity> = removed.read().collect();

    for (entity, view) in updates {
        if let Ok(mut client) = clients.get_mut(entity) {
            send_weapon_equipped(&mut client, MAIN_HAND_SLOT, Some(view));
        }
    }
    for entity in removed_entities {
        if let Ok(mut client) = clients.get_mut(entity) {
            send_weapon_equipped(&mut client, MAIN_HAND_SLOT, None);
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
