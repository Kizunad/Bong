//! plan-tsy-extract-v1 §4.1 server_data bridge for TSY extraction HUD.

use valence::prelude::{
    Added, Client, Entity, EventReader, Position, Query, RemovedComponents, Res, Username,
};

use crate::combat::CombatClock;
use crate::network::agent_bridge::{payload_type_label, serialize_server_data_payload};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::player::state::canonical_player_id;
use crate::schema::server_data::{
    ExtractAbortedReasonV1, ExtractAbortedV1, ExtractCompletedV1, ExtractFailedReasonV1,
    ExtractFailedV1, ExtractProgressV1, ExtractStartedV1, RiftPortalDirectionV1, RiftPortalKindV1,
    RiftPortalRemovedV1, RiftPortalStateV1, ServerDataPayloadV1, ServerDataV1,
    TsyCollapseStartedIpcV1,
};
use crate::world::extract_system::{
    ExtractAbortReason, ExtractAborted, ExtractCompleted, ExtractFailed, ExtractFailureReason,
    ExtractProgressPulse, ExtractRejectionReason, StartExtractResult,
};
use crate::world::rift_portal::RiftPortal;
use crate::world::tsy::{PortalDirection, RiftKind};
use crate::world::tsy_lifecycle::{TsyCollapseStarted, COLLAPSE_DURATION_TICKS};

pub fn emit_extract_started_payloads(
    mut events: EventReader<StartExtractResult>,
    clock: Res<CombatClock>,
    portals: Query<&RiftPortal>,
    mut clients: Query<(&Username, &mut Client)>,
) {
    for event in events.read() {
        match event {
            StartExtractResult::Started {
                player,
                portal,
                required_ticks,
            } => {
                let Ok(portal_data) = portals.get(*portal) else {
                    continue;
                };
                let Some(player_id) = player_id(&clients, *player) else {
                    continue;
                };
                push_to_client(
                    &mut clients,
                    *player,
                    ServerDataPayloadV1::ExtractStarted(ExtractStartedV1 {
                        player_id,
                        portal_entity_id: portal.to_bits(),
                        portal_kind: portal_kind_wire(portal_data.kind),
                        required_ticks: *required_ticks,
                        at_tick: clock.tick,
                    }),
                );
            }
            StartExtractResult::Rejected {
                player,
                portal: _,
                reason,
            } => {
                let Some(player_id) = player_id(&clients, *player) else {
                    continue;
                };
                push_to_client(
                    &mut clients,
                    *player,
                    ServerDataPayloadV1::ExtractAborted(ExtractAbortedV1 {
                        player_id,
                        reason: reject_reason_wire(*reason),
                    }),
                );
            }
        }
    }
}

pub fn emit_extract_progress_payloads(
    mut events: EventReader<ExtractProgressPulse>,
    mut clients: Query<(&Username, &mut Client)>,
) {
    for event in events.read() {
        let Some(player_id) = player_id(&clients, event.player) else {
            continue;
        };
        push_to_client(
            &mut clients,
            event.player,
            ServerDataPayloadV1::ExtractProgress(ExtractProgressV1 {
                player_id,
                portal_entity_id: event.portal.to_bits(),
                elapsed_ticks: event.elapsed_ticks,
                required_ticks: event.required_ticks,
            }),
        );
    }
}

pub fn emit_extract_completed_payloads(
    mut events: EventReader<ExtractCompleted>,
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
            ServerDataPayloadV1::ExtractCompleted(ExtractCompletedV1 {
                player_id,
                portal_kind: portal_kind_wire(event.portal_kind),
                family_id: event.family_id.clone(),
                exit_world_pos: event.exit_world_pos,
                at_tick: clock.tick,
            }),
        );
    }
}

pub fn emit_extract_aborted_payloads(
    mut events: EventReader<ExtractAborted>,
    mut clients: Query<(&Username, &mut Client)>,
) {
    for event in events.read() {
        let Some(player_id) = player_id(&clients, event.player) else {
            continue;
        };
        push_to_client(
            &mut clients,
            event.player,
            ServerDataPayloadV1::ExtractAborted(ExtractAbortedV1 {
                player_id,
                reason: abort_reason_wire(event.reason),
            }),
        );
    }
}

pub fn emit_extract_failed_payloads(
    mut events: EventReader<ExtractFailed>,
    mut clients: Query<(&Username, &mut Client)>,
) {
    for event in events.read() {
        let Some(player_id) = player_id(&clients, event.player) else {
            continue;
        };
        push_to_client(
            &mut clients,
            event.player,
            ServerDataPayloadV1::ExtractFailed(ExtractFailedV1 {
                player_id,
                reason: failure_reason_wire(event.reason),
            }),
        );
    }
}

pub fn emit_rift_portal_state_payloads(
    portals: Query<(Entity, &RiftPortal, &Position), Added<RiftPortal>>,
    mut clients: Query<&mut Client>,
) {
    let payloads = portal_state_payloads(portals.iter());

    if payloads.is_empty() {
        return;
    }
    for mut client in &mut clients {
        for payload in &payloads {
            send_server_data_payload(&mut client, payload.as_slice());
        }
    }
}

pub fn emit_rift_portal_removed_payloads(
    mut removed: RemovedComponents<RiftPortal>,
    mut clients: Query<&mut Client>,
) {
    let payloads: Vec<_> = removed
        .read()
        .filter_map(|entity| {
            serialize_payload(ServerDataV1::new(ServerDataPayloadV1::RiftPortalRemoved(
                RiftPortalRemovedV1 {
                    entity_id: entity.to_bits(),
                },
            )))
        })
        .collect();

    if payloads.is_empty() {
        return;
    }
    for mut client in &mut clients {
        for payload in &payloads {
            send_server_data_payload(&mut client, payload.as_slice());
        }
    }
}

pub fn emit_rift_portal_state_payloads_to_joined_clients(
    portals: Query<(Entity, &RiftPortal, &Position)>,
    mut clients: Query<&mut Client, Added<Client>>,
) {
    let payloads = portal_state_payloads(portals.iter());

    if payloads.is_empty() {
        return;
    }
    for mut client in &mut clients {
        for payload in &payloads {
            send_server_data_payload(&mut client, payload.as_slice());
        }
    }
}

pub fn emit_tsy_collapse_portal_state_payloads(
    mut events: EventReader<TsyCollapseStarted>,
    portals: Query<(Entity, &RiftPortal, &Position)>,
    mut clients: Query<&mut Client>,
) {
    for event in events.read() {
        let payloads = portal_state_payloads(
            portals
                .iter()
                .filter(|(_, portal, _)| portal.family_id == event.family_id),
        );

        for mut client in &mut clients {
            for payload in &payloads {
                send_server_data_payload(&mut client, payload.as_slice());
            }
        }
    }
}

fn portal_state_payloads<'a>(
    portals: impl Iterator<Item = (Entity, &'a RiftPortal, &'a Position)>,
) -> Vec<Vec<u8>> {
    portals
        .map(|(entity, portal, position)| {
            ServerDataV1::new(ServerDataPayloadV1::RiftPortalState(RiftPortalStateV1 {
                entity_id: entity.to_bits(),
                kind: portal_kind_wire(portal.kind),
                direction: portal_direction_wire(portal.direction),
                family_id: portal.family_id.clone(),
                world_pos: [position.0.x, position.0.y, position.0.z],
                trigger_radius: portal.trigger_radius,
                current_extract_ticks: portal.current_extract_ticks,
                activation_window_end: portal.activation_window.map(|win| win.end_at_tick),
            }))
        })
        .filter_map(serialize_payload)
        .collect()
}

pub fn emit_tsy_collapse_started_payloads(
    mut events: EventReader<TsyCollapseStarted>,
    clock: Res<CombatClock>,
    portals: Query<(Entity, &RiftPortal)>,
    mut clients: Query<&mut Client>,
) {
    for event in events.read() {
        let collapse_tear_entity_ids = portals
            .iter()
            .filter_map(|(entity, portal)| {
                (portal.family_id == event.family_id && portal.kind == RiftKind::CollapseTear)
                    .then_some(entity.to_bits())
            })
            .collect();
        let payload = ServerDataV1::new(ServerDataPayloadV1::TsyCollapseStartedIpc(
            TsyCollapseStartedIpcV1 {
                family_id: event.family_id.clone(),
                at_tick: event.at_tick,
                remaining_ticks: COLLAPSE_DURATION_TICKS
                    .saturating_sub(clock.tick.saturating_sub(event.at_tick)),
                collapse_tear_entity_ids,
            },
        ));
        let Some(payload_bytes) = serialize_payload(payload) else {
            continue;
        };
        for mut client in &mut clients {
            send_server_data_payload(&mut client, payload_bytes.as_slice());
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

fn player_id(clients: &Query<(&Username, &mut Client)>, entity: Entity) -> Option<String> {
    clients
        .get(entity)
        .ok()
        .map(|(username, _)| canonical_player_id(username.0.as_str()))
}

fn portal_kind_wire(kind: RiftKind) -> RiftPortalKindV1 {
    match kind {
        RiftKind::MainRift => RiftPortalKindV1::MainRift,
        RiftKind::DeepRift => RiftPortalKindV1::DeepRift,
        RiftKind::CollapseTear => RiftPortalKindV1::CollapseTear,
    }
}

fn portal_direction_wire(direction: PortalDirection) -> RiftPortalDirectionV1 {
    match direction {
        PortalDirection::Entry => RiftPortalDirectionV1::Entry,
        PortalDirection::Exit => RiftPortalDirectionV1::Exit,
    }
}

fn abort_reason_wire(reason: ExtractAbortReason) -> ExtractAbortedReasonV1 {
    match reason {
        ExtractAbortReason::Moved => ExtractAbortedReasonV1::Moved,
        ExtractAbortReason::Combat => ExtractAbortedReasonV1::Combat,
        ExtractAbortReason::Damaged => ExtractAbortedReasonV1::Damaged,
        ExtractAbortReason::Cancelled => ExtractAbortedReasonV1::Cancelled,
        ExtractAbortReason::PortalExpired => ExtractAbortedReasonV1::PortalExpired,
    }
}

fn reject_reason_wire(reason: ExtractRejectionReason) -> ExtractAbortedReasonV1 {
    match reason {
        ExtractRejectionReason::OutOfRange => ExtractAbortedReasonV1::OutOfRange,
        ExtractRejectionReason::AlreadyBusy => ExtractAbortedReasonV1::AlreadyBusy,
        ExtractRejectionReason::InCombat => ExtractAbortedReasonV1::Combat,
        ExtractRejectionReason::NotInTsy => ExtractAbortedReasonV1::NotInTsy,
        ExtractRejectionReason::PortalExpired | ExtractRejectionReason::PortalCollapsed => {
            ExtractAbortedReasonV1::PortalExpired
        }
        ExtractRejectionReason::CannotExit => ExtractAbortedReasonV1::CannotExit,
        // plan-tsy-raceout-v1 §4 Q-RC4：CollapseTear 已被其他玩家占用。
        // 复用 AlreadyBusy IPC variant 避免 schema breaking change；
        // client HUD 文案由 nearestPortal 切换提示弥补 UX。
        ExtractRejectionReason::PortalOccupied => ExtractAbortedReasonV1::AlreadyBusy,
    }
}

fn failure_reason_wire(reason: ExtractFailureReason) -> ExtractFailedReasonV1 {
    match reason {
        ExtractFailureReason::SpiritQiDrained => ExtractFailedReasonV1::SpiritQiDrained,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::agent_bridge::SERVER_DATA_CHANNEL;
    use crate::world::dimension::DimensionKind;
    use crate::world::tsy::{DimensionAnchor, PortalDirection};
    use valence::prelude::{App, DVec3, Update};
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    #[test]
    fn portal_kind_serializes_to_plan_literal() {
        let payload = ServerDataV1::new(ServerDataPayloadV1::ExtractStarted(ExtractStartedV1 {
            player_id: "offline:Kiz".to_string(),
            portal_entity_id: 42,
            portal_kind: RiftPortalKindV1::CollapseTear,
            required_ticks: 60,
            at_tick: 10,
        }));
        let value = serde_json::to_value(payload).expect("serialize");
        assert_eq!(value["type"], "extract_started");
        assert_eq!(value["portal_kind"], "collapse_tear");
    }

    #[test]
    fn joined_client_receives_existing_rift_portal_state() {
        let mut app = App::new();
        app.add_systems(Update, emit_rift_portal_state_payloads_to_joined_clients);
        app.world_mut().spawn((
            Position::new([1.0, 2.0, 3.0]),
            RiftPortal {
                family_id: "tsy_lingxu_01".to_string(),
                target: DimensionAnchor {
                    dimension: DimensionKind::Overworld,
                    pos: DVec3::ZERO,
                },
                trigger_radius: 2.0,
                direction: PortalDirection::Exit,
                kind: RiftKind::MainRift,
                current_extract_ticks: 160,
                activation_window: None,
            },
        ));
        let (client_bundle, mut helper) = create_mock_client("Azure");
        app.world_mut().spawn(client_bundle);

        app.update();
        flush_all_client_packets(&mut app);

        let payloads = collect_server_data_payloads(&mut helper);
        assert!(payloads.iter().any(|payload| {
            matches!(
                &payload.payload,
                ServerDataPayloadV1::RiftPortalState(state)
                    if state.family_id == "tsy_lingxu_01" && state.world_pos == [1.0, 2.0, 3.0]
            )
        }));
    }

    #[test]
    fn removed_portal_broadcasts_cache_eviction() {
        let mut app = App::new();
        app.add_systems(Update, emit_rift_portal_removed_payloads);
        let portal = app
            .world_mut()
            .spawn(RiftPortal {
                family_id: "tsy_lingxu_01".to_string(),
                target: DimensionAnchor {
                    dimension: DimensionKind::Overworld,
                    pos: DVec3::ZERO,
                },
                trigger_radius: 2.0,
                direction: PortalDirection::Exit,
                kind: RiftKind::MainRift,
                current_extract_ticks: 160,
                activation_window: None,
            })
            .id();
        let (client_bundle, mut helper) = create_mock_client("Azure");
        app.world_mut().spawn(client_bundle);

        app.update();
        app.world_mut().entity_mut(portal).despawn();
        app.update();
        flush_all_client_packets(&mut app);

        let payloads = collect_server_data_payloads(&mut helper);
        assert!(payloads.iter().any(|payload| {
            matches!(
                &payload.payload,
                ServerDataPayloadV1::RiftPortalRemoved(removed)
                    if removed.entity_id == portal.to_bits()
            )
        }));
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
}
