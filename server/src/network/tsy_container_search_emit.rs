//! plan-input-binding-v1 §4 — TSY container search server_data bridge.

use std::collections::HashMap;

use valence::prelude::{
    bevy_ecs::query::QueryFilter, Added, Changed, Client, Entity, EventReader, Or, ParamSet,
    Position, Query, Res, Username, With,
};

use crate::combat::CombatClock;
use crate::network::agent_bridge::{payload_type_label, serialize_server_data_payload};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::player::state::canonical_player_id;
use crate::schema::server_data::{
    ContainerKindV1, ContainerStateV1, KeyKindV1, LootPreviewItemV1, SearchAbortReasonV1,
    SearchAbortedV1, SearchCompletedV1, SearchProgressV1, SearchStartedV1, ServerDataPayloadV1,
    ServerDataV1,
};
use crate::world::tsy_container::{ContainerKind, KeyKind, LootContainer, SearchProgress};
use crate::world::tsy_container_search::{
    SearchAbortReason, SearchAborted, SearchCompleted, StartSearchResult,
};

type ContainerStateQueryItem<'a> = (Entity, &'a LootContainer, &'a Position);
type ChangedContainerFilter = Or<(Added<LootContainer>, Changed<LootContainer>)>;

pub fn emit_container_state_payloads(
    containers: Query<ContainerStateQueryItem, ChangedContainerFilter>,
    mut clients: Query<(Entity, &Username, &mut Client)>,
) {
    let player_ids = player_ids(
        clients
            .iter()
            .map(|(entity, username, _)| (entity, username)),
    );
    let payloads = container_state_payloads(containers.iter(), &player_ids);
    broadcast(&mut clients, &payloads);
}

#[allow(clippy::type_complexity)]
pub fn emit_container_state_payloads_to_joined_clients(
    containers: Query<(Entity, &LootContainer, &Position)>,
    mut clients: ParamSet<(
        Query<(Entity, &Username), With<Client>>,
        Query<(Entity, &Username, &mut Client), Added<Client>>,
    )>,
) {
    let player_ids = player_ids(clients.p0().iter());
    let payloads = container_state_payloads(containers.iter(), &player_ids);
    let mut joined_clients = clients.p1();
    broadcast(&mut joined_clients, &payloads);
}

pub fn emit_search_started_payloads(
    mut events: EventReader<StartSearchResult>,
    clock: Res<CombatClock>,
    mut clients: Query<(&Username, &mut Client)>,
) {
    for event in events.read() {
        let StartSearchResult::Started {
            player,
            container,
            required_ticks,
        } = event
        else {
            continue;
        };
        let Some(player_id) = player_id(&clients, *player) else {
            continue;
        };
        push_to_client(
            &mut clients,
            *player,
            ServerDataPayloadV1::SearchStarted(SearchStartedV1 {
                player_id,
                container_entity_id: container.to_bits(),
                required_ticks: *required_ticks,
                at_tick: clock.tick,
            }),
        );
    }
}

pub fn emit_search_progress_payloads(
    players: Query<(Entity, &SearchProgress), With<Client>>,
    mut clients: Query<(&Username, &mut Client)>,
) {
    for (player, progress) in &players {
        if progress.elapsed_ticks == 0 || progress.elapsed_ticks % 5 != 0 {
            continue;
        }
        let Some(player_id) = player_id(&clients, player) else {
            continue;
        };
        let elapsed_ticks = progress.elapsed_ticks.min(progress.required_ticks);
        push_to_client(
            &mut clients,
            player,
            ServerDataPayloadV1::SearchProgress(SearchProgressV1 {
                player_id,
                container_entity_id: progress.container.to_bits(),
                elapsed_ticks,
                required_ticks: progress.required_ticks,
            }),
        );
    }
}

pub fn emit_search_completed_payloads(
    mut events: EventReader<SearchCompleted>,
    clock: Res<CombatClock>,
    mut clients: Query<(&Username, &mut Client)>,
) {
    for event in events.read() {
        let Some(player_id) = player_id(&clients, event.player) else {
            continue;
        };
        let loot_preview = event
            .loot
            .iter()
            .map(|item| LootPreviewItemV1 {
                template_id: item.template_id.clone(),
                display_name: item.display_name.clone(),
                stack_count: item.stack_count,
            })
            .collect();
        push_to_client(
            &mut clients,
            event.player,
            ServerDataPayloadV1::SearchCompleted(SearchCompletedV1 {
                player_id,
                container_entity_id: event.container.to_bits(),
                family_id: event.family_id.clone(),
                loot_preview,
                at_tick: clock.tick,
            }),
        );
    }
}

pub fn emit_search_aborted_payloads(
    mut events: EventReader<SearchAborted>,
    clock: Res<CombatClock>,
    mut clients: Query<(&Username, &mut Client)>,
) {
    for event in events.read() {
        let Some(player_id) = player_id(&clients, event.player) else {
            continue;
        };
        push_to_client(
            &mut clients,
            event.player,
            ServerDataPayloadV1::SearchAborted(SearchAbortedV1 {
                player_id,
                container_entity_id: event.container.to_bits(),
                reason: abort_reason_wire(event.reason),
                at_tick: clock.tick,
            }),
        );
    }
}

fn container_state_payloads<'a>(
    containers: impl Iterator<Item = (Entity, &'a LootContainer, &'a Position)>,
    player_ids: &HashMap<Entity, String>,
) -> Vec<Vec<u8>> {
    containers
        .map(|(entity, container, position)| {
            ServerDataV1::new(ServerDataPayloadV1::ContainerState(ContainerStateV1 {
                entity_id: entity.to_bits(),
                kind: container_kind_wire(container.kind),
                family_id: container.family_id.clone(),
                world_pos: [position.0.x, position.0.y, position.0.z],
                locked: container.locked.map(key_kind_wire),
                depleted: container.depleted,
                searched_by_player_id: container
                    .searched_by
                    .and_then(|player| player_ids.get(&player).cloned()),
            }))
        })
        .filter_map(serialize_payload)
        .collect()
}

fn broadcast<F: QueryFilter>(
    clients: &mut Query<(Entity, &Username, &mut Client), F>,
    payloads: &[Vec<u8>],
) {
    if payloads.is_empty() {
        return;
    }
    for (_, _, mut client) in clients.iter_mut() {
        for payload in payloads {
            send_server_data_payload(&mut client, payload.as_slice());
        }
    }
}

fn push_to_client(
    clients: &mut Query<(&Username, &mut Client)>,
    entity: Entity,
    payload: ServerDataPayloadV1,
) {
    let payload = ServerDataV1::new(payload);
    let Some(payload_bytes) = serialize_payload(payload) else {
        return;
    };
    if let Ok((_username, mut client)) = clients.get_mut(entity) {
        send_server_data_payload(&mut client, payload_bytes.as_slice());
    }
}

fn serialize_payload(payload: ServerDataV1) -> Option<Vec<u8>> {
    let payload_type = payload_type_label(payload.payload_type());
    match serialize_server_data_payload(&payload) {
        Ok(bytes) => Some(bytes),
        Err(error) => {
            log_payload_build_error(payload_type, &error);
            None
        }
    }
}

fn player_ids<'a>(
    clients: impl Iterator<Item = (Entity, &'a Username)>,
) -> HashMap<Entity, String> {
    clients
        .map(|(entity, username)| (entity, canonical_player_id(username.0.as_str())))
        .collect()
}

fn player_id(clients: &Query<(&Username, &mut Client)>, entity: Entity) -> Option<String> {
    clients
        .get(entity)
        .ok()
        .map(|(username, _)| canonical_player_id(username.0.as_str()))
}

fn container_kind_wire(kind: ContainerKind) -> ContainerKindV1 {
    match kind {
        ContainerKind::DryCorpse => ContainerKindV1::DryCorpse,
        ContainerKind::Skeleton => ContainerKindV1::Skeleton,
        ContainerKind::StoragePouch => ContainerKindV1::StoragePouch,
        ContainerKind::StoneCasket => ContainerKindV1::StoneCasket,
        ContainerKind::RelicCore => ContainerKindV1::RelicCore,
    }
}

fn key_kind_wire(kind: KeyKind) -> KeyKindV1 {
    match kind {
        KeyKind::StoneCasketKey => KeyKindV1::StoneCasketKey,
        KeyKind::JadeCoffinSeal => KeyKindV1::JadeCoffinSeal,
        KeyKind::ArrayCoreSigil => KeyKindV1::ArrayCoreSigil,
    }
}

fn abort_reason_wire(reason: SearchAbortReason) -> SearchAbortReasonV1 {
    match reason {
        SearchAbortReason::Moved => SearchAbortReasonV1::Moved,
        SearchAbortReason::Combat => SearchAbortReasonV1::Combat,
        SearchAbortReason::Damaged => SearchAbortReasonV1::Damaged,
        SearchAbortReason::Cancelled => SearchAbortReasonV1::Cancelled,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::server_data::ServerDataPayloadV1;
    use crate::world::zone::TsyDepth;
    use valence::math::DVec3;

    #[test]
    fn container_state_payload_preserves_active_searcher_id() {
        let player = Entity::from_raw(7);
        let container_entity = Entity::from_raw(42);
        let mut container = LootContainer::new(
            ContainerKind::StoragePouch,
            "tsy_lingxu_01".to_string(),
            TsyDepth::Shallow,
            "loot_pool".to_string(),
            100,
        );
        container.searched_by = Some(player);
        let position = Position(DVec3::new(8.0, 64.0, -4.0));
        let player_ids = HashMap::from([(player, "offline:Searcher".to_string())]);

        let payloads = container_state_payloads(
            vec![(container_entity, &container, &position)].into_iter(),
            &player_ids,
        );

        assert_eq!(payloads.len(), 1);
        let decoded: ServerDataV1 = serde_json::from_slice(&payloads[0]).expect("payload decodes");
        let ServerDataPayloadV1::ContainerState(state) = decoded.payload else {
            panic!("expected container_state payload");
        };
        assert_eq!(state.entity_id, container_entity.to_bits());
        assert_eq!(
            state.searched_by_player_id.as_deref(),
            Some("offline:Searcher")
        );
    }
}
