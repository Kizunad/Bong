//! plan-spirit-treasure-v1：灵宝状态与器灵对话 server_data 推送。

use valence::message::SendMessage;
use valence::prelude::{Added, Changed, Client, Entity, Or, Position, Query, Res, Username, With};

use crate::combat::components::StatusEffects;
use crate::inventory::spirit_treasure::{
    state_payload_for_active_treasures, sync_passive_status_effects, ActiveSpiritTreasures,
    SpiritTreasureRegistry,
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
    treasure_holders: &mut Query<
        (&ActiveSpiritTreasures, Option<&mut StatusEffects>),
        With<Client>,
    >,
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

    let state_payload_bytes = registry
        .apply_affinity_delta(&dialogue.treasure_id, dialogue.affinity_delta)
        .and_then(|_| {
            refresh_target_treasure_state_payload(registry, target_entity, treasure_holders)
        });

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
            if let Some(bytes) = state_payload_bytes.as_deref() {
                send_server_data_payload(&mut client, bytes);
            }
        }
    }
}

fn refresh_target_treasure_state_payload(
    registry: &SpiritTreasureRegistry,
    target_entity: Entity,
    treasure_holders: &mut Query<
        (&ActiveSpiritTreasures, Option<&mut StatusEffects>),
        With<Client>,
    >,
) -> Option<Vec<u8>> {
    let Ok((active, status_effects)) = treasure_holders.get_mut(target_entity) else {
        return None;
    };

    if let Some(mut statuses) = status_effects {
        sync_passive_status_effects(registry, active.treasures.as_slice(), &mut statuses);
    }

    let payload = ServerDataV1::new(ServerDataPayloadV1::SpiritTreasureState(
        state_payload_for_active_treasures(registry, active),
    ));
    let payload_type = payload_type_label(payload.payload_type());
    match serialize_server_data_payload(&payload) {
        Ok(bytes) => Some(bytes),
        Err(error) => {
            log_payload_build_error(payload_type, &error);
            None
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::events::StatusEffectKind;
    use crate::inventory::spirit_treasure::{
        affinity_scale, sync_passive_status_effects, ActiveTreasureEntry, JIZHAOJING_TEMPLATE_ID,
    };
    use crate::network::agent_bridge::SERVER_DATA_CHANNEL;
    use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
    use crate::schema::spirit_treasure::{SpiritTreasureDialogueToneV1, SpiritTreasureDialogueV1};
    use valence::prelude::{bevy_ecs, App, Resource, Update};
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    #[derive(Resource, Default)]
    struct DialogueInbox(Option<SpiritTreasureDialogueV1>);

    fn process_dialogue_from_inbox(
        mut inbox: valence::prelude::ResMut<DialogueInbox>,
        zone_registry: Option<Res<ZoneRegistry>>,
        mut registry: valence::prelude::ResMut<SpiritTreasureRegistry>,
        mut clients: Query<(Entity, &mut Client, &Username, &Position), With<Client>>,
        mut treasure_holders: Query<
            (&ActiveSpiritTreasures, Option<&mut StatusEffects>),
            With<Client>,
        >,
    ) {
        let Some(dialogue) = inbox.0.take() else {
            return;
        };
        process_spirit_treasure_dialogue(
            dialogue,
            zone_registry.as_deref(),
            &mut registry,
            &mut clients,
            &mut treasure_holders,
        );
    }

    fn flush_all_client_packets(app: &mut App) {
        let world = app.world_mut();
        let mut query = world.query::<&mut Client>();

        for mut client in query.iter_mut(world) {
            client
                .flush_packets()
                .expect("mock client packets should flush successfully");
        }
    }

    fn collect_server_data_payloads(helper: &mut MockClientHelper) -> Vec<ServerDataV1> {
        let mut payloads = Vec::new();

        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };
            if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                continue;
            }

            payloads.push(
                serde_json::from_slice(packet.data.0 .0)
                    .expect("typed payload should decode as ServerDataV1"),
            );
        }

        payloads
    }

    fn status_magnitude(statuses: &StatusEffects, kind: StatusEffectKind) -> Option<f32> {
        statuses
            .active
            .iter()
            .find(|effect| effect.kind == kind)
            .map(|effect| effect.magnitude)
    }

    #[test]
    fn dialogue_affinity_delta_refreshes_status_and_state_payload() {
        let mut app = App::new();
        app.insert_resource(DialogueInbox::default());
        app.add_systems(Update, process_dialogue_from_inbox);

        let (mut client_bundle, mut helper) = create_mock_client("Alice");
        client_bundle.player.position = Position::new([8.0, 66.0, 8.0]);
        let active = ActiveSpiritTreasures {
            treasures: vec![ActiveTreasureEntry {
                template_id: JIZHAOJING_TEMPLATE_ID.to_string(),
                instance_id: 88,
                equipped: true,
                passive_active: true,
            }],
        };
        let entity = app
            .world_mut()
            .spawn((client_bundle, active.clone(), StatusEffects::default()))
            .id();

        let mut registry = SpiritTreasureRegistry::default();
        registry.ensure_player_holder(JIZHAOJING_TEMPLATE_ID, 88, entity, 0);
        registry
            .active
            .get_mut(JIZHAOJING_TEMPLATE_ID)
            .expect("state exists")
            .affinity = 0.3;

        let mut initial_statuses = StatusEffects::default();
        sync_passive_status_effects(
            &registry,
            active.treasures.as_slice(),
            &mut initial_statuses,
        );
        app.world_mut().entity_mut(entity).insert(initial_statuses);
        app.insert_resource(registry);
        app.world_mut().resource_mut::<DialogueInbox>().0 = Some(SpiritTreasureDialogueV1 {
            v: 1,
            request_id: "test-dialogue".to_string(),
            character_id: "Alice".to_string(),
            treasure_id: JIZHAOJING_TEMPLATE_ID.to_string(),
            text: "镜面转暗。".to_string(),
            tone: SpiritTreasureDialogueToneV1::Cold,
            affinity_delta: -0.1,
        });

        app.update();
        flush_all_client_packets(&mut app);

        let statuses = app
            .world()
            .entity(entity)
            .get::<StatusEffects>()
            .expect("client should keep StatusEffects");
        let expected = 0.30 * affinity_scale(0.2);
        let actual = status_magnitude(statuses, StatusEffectKind::SpiritTreasurePerception)
            .expect("active treasure should keep perception passive");
        assert!(
            (actual - expected).abs() < 1e-6,
            "dialogue affinity_delta 后应立即重算 status，actual={actual}, expected={expected}"
        );

        let payloads = collect_server_data_payloads(&mut helper);
        assert!(
            payloads.iter().any(|payload| matches!(
                payload.payload,
                ServerDataPayloadV1::SpiritTreasureDialogue(_)
            )),
            "目标玩家仍应收到器灵对话 payload"
        );
        let state_payload = payloads
            .iter()
            .find_map(|payload| match &payload.payload {
                ServerDataPayloadV1::SpiritTreasureState(state) => Some(state),
                _ => None,
            })
            .expect("affinity_delta 后应主动推送 spirit_treasure_state");
        let treasure = state_payload
            .treasures
            .iter()
            .find(|treasure| treasure.template_id == JIZHAOJING_TEMPLATE_ID)
            .expect("state payload should include jizhaojing");
        assert!(
            (treasure.affinity - 0.2).abs() < 1e-9,
            "state payload affinity 应是最新值，实际 {}",
            treasure.affinity
        );
        assert!(
            treasure.sleeping,
            "affinity <= 0.2 时 state payload 应同步沉睡"
        );
    }
}
