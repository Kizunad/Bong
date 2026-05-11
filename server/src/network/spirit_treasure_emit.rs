//! plan-spirit-treasure-v1：灵宝状态与器灵对话 server_data 推送。

use valence::message::SendMessage;
use valence::prelude::{Added, Changed, Client, Entity, Or, Position, Query, Res, Username, With};

use crate::inventory::spirit_treasure::{
    state_payload_for_active_treasures, ActiveSpiritTreasures, SpiritTreasureRegistry,
};
use crate::network::agent_bridge::{payload_type_label, serialize_server_data_payload};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::player::state::canonical_player_id;
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
use crate::schema::spirit_treasure::{SpiritTreasureDialoguePayloadV1, SpiritTreasureDialogueV1};
use crate::world::dimension::DimensionKind;
use crate::world::zone::{ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};

type SpiritTreasureStateClientFilter = (
    With<Client>,
    Or<(Added<ActiveSpiritTreasures>, Changed<ActiveSpiritTreasures>)>,
);

pub fn emit_spirit_treasure_state_payloads(
    registry: Res<SpiritTreasureRegistry>,
    mut clients: Query<(&mut Client, &ActiveSpiritTreasures), SpiritTreasureStateClientFilter>,
) {
    for (mut client, active) in &mut clients {
        let payload = ServerDataV1::new(ServerDataPayloadV1::SpiritTreasureState(
            state_payload_for_active_treasures(&registry, active),
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
    }
}

pub fn process_spirit_treasure_dialogue(
    dialogue: SpiritTreasureDialogueV1,
    zone_registry: Option<&ZoneRegistry>,
    registry: &mut SpiritTreasureRegistry,
    clients: &mut Query<(Entity, &mut Client, &Username, &Position), With<Client>>,
) {
    let display_name = registry
        .defs
        .get(&dialogue.treasure_id)
        .map(|def| def.display_name.clone())
        .unwrap_or_else(|| dialogue.treasure_id.clone());
    let text = normalized_dialogue_text(dialogue.text.as_str());
    let zone_registry = zone_registry
        .cloned()
        .unwrap_or_else(ZoneRegistry::fallback);
    let target = find_target_client(clients, dialogue.character_id.as_str(), &zone_registry);
    let Some((target_entity, zone)) = target else {
        tracing::warn!(
            "[bong][spirit-treasure] dialogue request={} character={} has no connected target",
            dialogue.request_id,
            dialogue.character_id
        );
        return;
    };

    registry.apply_affinity_delta(&dialogue.treasure_id, dialogue.affinity_delta);

    let payload = ServerDataV1::new(ServerDataPayloadV1::SpiritTreasureDialogue(
        SpiritTreasureDialoguePayloadV1 {
            dialogue: SpiritTreasureDialogueV1 {
                text: text.clone(),
                affinity_delta: dialogue.affinity_delta.clamp(-0.1, 0.1),
                ..dialogue.clone()
            },
            display_name: display_name.clone(),
            zone: zone.clone(),
        },
    ));
    let payload_type = payload_type_label(payload.payload_type());
    let payload_bytes = match serialize_server_data_payload(&payload) {
        Ok(bytes) => bytes,
        Err(error) => {
            log_payload_build_error(payload_type, &error);
            return;
        }
    };
    let public_message = format!("§b[{display_name}] §3{text}");

    for (entity, mut client, _, position) in clients.iter_mut() {
        if zone_name_for_position(&zone_registry, position.get()) == zone {
            client.send_chat_message(public_message.clone());
        }
        if entity == target_entity {
            send_server_data_payload(&mut client, payload_bytes.as_slice());
        }
    }
}

fn find_target_client(
    clients: &mut Query<(Entity, &mut Client, &Username, &Position), With<Client>>,
    character_id: &str,
    zone_registry: &ZoneRegistry,
) -> Option<(Entity, String)> {
    clients
        .iter_mut()
        .find(|(_, _, username, _)| {
            username.0 == character_id || canonical_player_id(username.0.as_str()) == character_id
        })
        .map(|(entity, _, _, position)| {
            (
                entity,
                zone_name_for_position(zone_registry, position.get()),
            )
        })
}

fn zone_name_for_position(
    zone_registry: &ZoneRegistry,
    position: valence::prelude::DVec3,
) -> String {
    zone_registry
        .find_zone(DimensionKind::Overworld, position)
        .map(|zone| zone.name.clone())
        .unwrap_or_else(|| DEFAULT_SPAWN_ZONE_NAME.to_string())
}

fn normalized_dialogue_text(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        "……".to_string()
    } else {
        trimmed.chars().take(180).collect()
    }
}
