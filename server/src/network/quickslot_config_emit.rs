//! plan-HUD-v1 §10.4 / §11.4 server-side emit for `quickslot_config` payload。
//!
//! 监听 `Changed<QuickSlotBindings>`，覆盖三种触发：玩家拖物品到 F 槽、
//! cast 完成 / 中断后冷却写入、`set_cooldown` 调用。
//! 触发时把完整 `QuickSlotConfigV1`（含 instance→template 反查 +
//! cooldown_until_ms 折算）推给该 client。
//!
//! icon_texture 恒为空串是显式契约（plan-skill-av-relink-v1 P0 决议，修正 plan
//! 原文"从 technique_definition 取值"的前提错误）：quickslot 是纯 Item 槽
//! （`QuickSlotBindings` 只绑 instance_id→template，无 Skill 变体，
//! TechniqueRegistry 中不含 item template id），client 对空串按 item_id 走
//! ItemIconRegistry 富解析（tools/ 子目录映射、armor tint、存在性探测、
//! broken_artifact 兜底）；server 若回填 naive 模板路径，client 会走裸
//! texture() 分支绕过富解析，造成工具/护甲类图标回归。只发给本人，不广播。

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
        let config = build_quickslot_config(
            Some(bindings),
            Some(inventory),
            &item_registry,
            now_tick,
            now_ms,
            None,
            None,
        );
        send_quickslot_config_to_client(&mut client, config, entity, username.0.as_str());
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_quickslot_config(
    bindings: Option<&QuickSlotBindings>,
    inventory: Option<&PlayerInventory>,
    item_registry: &ItemRegistry,
    now_tick: u64,
    now_ms: u64,
    ack_request_id: Option<String>,
    bind_accepted: Option<bool>,
) -> QuickSlotConfigV1 {
    let mut slots = Vec::with_capacity(QuickSlotBindings::SLOT_COUNT);
    let mut cooldown_until_ms = Vec::with_capacity(QuickSlotBindings::SLOT_COUNT);
    for i in 0..QuickSlotBindings::SLOT_COUNT {
        let entry = bindings.and_then(|bindings| {
            bindings.get(i as u8).and_then(|instance_id| {
                let inventory = inventory?;
                let template_id = lookup_template_id(inventory, instance_id)?;
                let template = item_registry.get(&template_id);
                Some(QuickSlotEntryV1 {
                    display_name: template
                        .map(|template| template.display_name.clone())
                        .unwrap_or_else(|| template_id.clone()),
                    cast_duration_ms: template
                        .map(|template| template.cast_duration_ms)
                        .unwrap_or(TEMPLATE_DEFAULT_CAST_MS),
                    cooldown_ms: template
                        .map(|template| template.cooldown_ms)
                        .unwrap_or(TEMPLATE_DEFAULT_COOLDOWN_MS),
                    item_id: template_id,
                    // 契约：Item 槽 icon_texture 恒空串，client 按 item_id 走
                    // ItemIconRegistry 富解析；填路径会绕过它（见模块注释）。
                    icon_texture: String::new(),
                })
            })
        });
        slots.push(entry);
        let cd_tick = bindings
            .map(|bindings| bindings.cooldown_until_tick[i])
            .unwrap_or_default();
        cooldown_until_ms.push(if cd_tick > now_tick {
            now_ms.saturating_add((cd_tick - now_tick).saturating_mul(TICK_MS))
        } else {
            0
        });
    }
    QuickSlotConfigV1 {
        slots,
        cooldown_until_ms,
        ack_request_id,
        bind_accepted,
    }
}

pub(crate) fn send_quickslot_config_to_client(
    client: &mut Client,
    config: QuickSlotConfigV1,
    entity: Entity,
    username: &str,
) {
    let payload = ServerDataV1::new(ServerDataPayloadV1::QuickSlotConfig(config));
    let payload_type = payload_type_label(payload.payload_type());
    let payload_bytes = match serialize_server_data_payload(&payload) {
        Ok(bytes) => bytes,
        Err(error) => {
            log_payload_build_error(payload_type, &error);
            return;
        }
    };
    send_server_data_payload(client, payload_bytes.as_slice());
    tracing::debug!(
        "[bong][network] sent {} {} payload to entity {entity:?} for `{username}`",
        SERVER_DATA_CHANNEL,
        payload_type
    );
}

pub(crate) fn current_unix_millis_for_quickslot() -> u64 {
    current_unix_millis()
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
        .flat_map(|s| s.iter_all())
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
