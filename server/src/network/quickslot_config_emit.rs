//! plan-HUD-v1 §10.4 / §11.4 server-side emit for `quickslot_config` payload。
//!
//! 监听 `Changed<QuickSlotBindings>`，覆盖三种触发：玩家拖物品到 F 槽、
//! cast 完成 / 中断后冷却写入、`set_cooldown` 调用。
//! 触发时把完整 `QuickSlotConfigV1`（含 instance→template 反查 +
//! cooldown_until_ms 折算）推给该 client。
//!
//! 当前 v1 限制：cast_duration_ms / cooldown_ms / icon_texture 是占位常量
//! （后续扩展 ItemTemplate 让 schema 真实匹配 plan §10.4）；只发给本人，不广播。

use std::time::{SystemTime, UNIX_EPOCH};

use valence::prelude::{Changed, Client, Entity, Query, Res, Username, With};

use crate::combat::components::QuickSlotBindings;
use crate::combat::CombatClock;
use crate::inventory::{
    ItemRegistry, PlayerInventory, DEFAULT_CAST_DURATION_MS as TEMPLATE_DEFAULT_CAST_MS,
    DEFAULT_COOLDOWN_MS as TEMPLATE_DEFAULT_COOLDOWN_MS,
};
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::combat_hud::{QuickSlotConfigV1, QuickSlotEntryV1};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};

const TICK_MS: u64 = 50;

type QuickSlotEmitFilter = (With<Client>, Changed<QuickSlotBindings>);

pub fn emit_quickslot_config_payloads(
    clock: Res<CombatClock>,
    item_registry: Res<ItemRegistry>,
    mut clients: Query<
        (
            Entity,
            &mut Client,
            &Username,
            &QuickSlotBindings,
            &PlayerInventory,
        ),
        QuickSlotEmitFilter,
    >,
) {
    let now_ms = current_unix_millis();
    let now_tick = clock.tick;

    for (entity, mut client, username, bindings, inventory) in &mut clients {
        let mut slots = Vec::with_capacity(QuickSlotBindings::SLOT_COUNT);
        let mut cooldown_until_ms = Vec::with_capacity(QuickSlotBindings::SLOT_COUNT);
        for i in 0..QuickSlotBindings::SLOT_COUNT {
            let slot_idx = i as u8;
            let entry = bindings.get(slot_idx).and_then(|instance_id| {
                let template_id = lookup_template_id(inventory, instance_id)?;
                let template = item_registry.get(&template_id);
                let display_name = template
                    .map(|t| t.display_name.clone())
                    .unwrap_or_else(|| template_id.clone());
                let cast_duration_ms = template
                    .map(|t| t.cast_duration_ms)
                    .unwrap_or(TEMPLATE_DEFAULT_CAST_MS);
                let cooldown_ms = template
                    .map(|t| t.cooldown_ms)
                    .unwrap_or(TEMPLATE_DEFAULT_COOLDOWN_MS);
                Some(QuickSlotEntryV1 {
                    item_id: template_id,
                    display_name,
                    cast_duration_ms,
                    cooldown_ms,
                    icon_texture: String::new(),
                })
            });
            slots.push(entry);
            // tick → unix ms 折算：cooldown_until_tick > now → 还在冷却。
            let cd_tick = bindings.cooldown_until_tick[i];
            let cd_until_ms = if cd_tick > now_tick {
                let delta_ticks = cd_tick - now_tick;
                now_ms.saturating_add(delta_ticks.saturating_mul(TICK_MS))
            } else {
                0
            };
            cooldown_until_ms.push(cd_until_ms);
        }

        let payload = ServerDataV1::new(ServerDataPayloadV1::QuickSlotConfig(QuickSlotConfigV1 {
            slots,
            cooldown_until_ms,
        }));
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
            "[bong][network] sent {} {} payload to entity {entity:?} for `{}`",
            SERVER_DATA_CHANNEL,
            payload_type,
            username.0
        );
    }
}

fn lookup_template_id(inv: &PlayerInventory, instance_id: u64) -> Option<String> {
    for c in &inv.containers {
        if let Some(p) = c
            .items
            .iter()
            .find(|p| p.instance.instance_id == instance_id)
        {
            return Some(p.instance.template_id.clone());
        }
    }
    if let Some(item) = inv
        .equipped
        .values()
        .find(|item| item.instance_id == instance_id)
    {
        return Some(item.template_id.clone());
    }
    inv.hotbar
        .iter()
        .flatten()
        .find(|item| item.instance_id == instance_id)
        .map(|item| item.template_id.clone())
}

fn current_unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
