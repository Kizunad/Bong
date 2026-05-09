use valence::prelude::{
    Changed, Client, Entity, EventReader, EventWriter, Position, Query, UniqueId, With,
};

use crate::cultivation::full_power_strike::{
    ChargeInterruptedEvent, ChargingState, Exhausted, ExhaustedExpiredEvent, FullPowerReleasedEvent,
};
use crate::network::agent_bridge::{payload_type_label, serialize_server_data_payload};
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::server_data::{
    FullPowerChargingStateV1, FullPowerExhaustedStateV1, FullPowerReleaseV1, ServerDataPayloadV1,
    ServerDataV1,
};
use crate::schema::vfx_event::VfxEventPayloadV1;

const CHARGING_ORB_EVENT_ID: &str = "bong:charging_orb";
const RELEASE_LIGHTNING_EVENT_ID: &str = "bong:release_lightning";
const EXHAUSTED_GREY_MIST_EVENT_ID: &str = "bong:exhausted_grey_mist";

pub fn emit_full_power_charging_state_payloads(
    charging_q: Query<(Entity, &ChargingState, Option<&UniqueId>), Changed<ChargingState>>,
    position_q: Query<&Position>,
    mut clients: Query<&mut Client, With<Client>>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    for (entity, charging, unique_id) in &charging_q {
        broadcast_server_data(
            ServerDataPayloadV1::FullPowerChargingState(FullPowerChargingStateV1 {
                caster_uuid: actor_id(entity, unique_id),
                active: true,
                qi_committed: charging.qi_committed,
                target_qi: charging.target_qi,
                started_tick: charging.started_at_tick,
            }),
            &mut clients,
        );

        if let Ok(position) = position_q.get(entity) {
            let origin = position.get();
            vfx_events.send(VfxEventRequest::new(
                origin,
                VfxEventPayloadV1::SpawnParticle {
                    event_id: CHARGING_ORB_EVENT_ID.to_string(),
                    origin: [origin.x, origin.y + 1.0, origin.z],
                    direction: None,
                    color: Some("#C43CFF".to_string()),
                    strength: Some(charge_strength(charging)),
                    count: Some(10),
                    duration_ticks: Some(8),
                },
            ));
        }
    }
}

pub fn emit_full_power_charging_clear_payloads(
    mut released: EventReader<FullPowerReleasedEvent>,
    mut interrupted: EventReader<ChargeInterruptedEvent>,
    ids: Query<&UniqueId>,
    mut clients: Query<&mut Client, With<Client>>,
) {
    for event in released.read() {
        broadcast_charging_clear(event.caster, event.at_tick, &ids, &mut clients);
    }
    for event in interrupted.read() {
        broadcast_charging_clear(event.caster, event.at_tick, &ids, &mut clients);
    }
}

pub fn emit_full_power_release_payloads(
    mut released: EventReader<FullPowerReleasedEvent>,
    ids: Query<&UniqueId>,
    positions: Query<&Position>,
    mut clients: Query<&mut Client, With<Client>>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    for event in released.read() {
        let target_uuid = event
            .target
            .map(|target| actor_id(target, ids.get(target).ok()));
        broadcast_server_data(
            ServerDataPayloadV1::FullPowerRelease(FullPowerReleaseV1 {
                caster_uuid: actor_id(event.caster, ids.get(event.caster).ok()),
                target_uuid,
                qi_released: event.qi_released,
                tick: event.at_tick,
                hit_position: event.hit_position,
            }),
            &mut clients,
        );

        if let Ok(position) = positions.get(event.caster) {
            let origin = position.get();
            let direction = event.hit_position.map(|hit| {
                [
                    hit[0] - origin.x,
                    hit[1] - (origin.y + 1.0),
                    hit[2] - origin.z,
                ]
            });
            vfx_events.send(VfxEventRequest::new(
                origin,
                VfxEventPayloadV1::SpawnParticle {
                    event_id: RELEASE_LIGHTNING_EVENT_ID.to_string(),
                    origin: [origin.x, origin.y + 1.0, origin.z],
                    direction,
                    color: Some("#B445FF".to_string()),
                    strength: Some(1.0),
                    count: Some(18),
                    duration_ticks: Some(10),
                },
            ));
        }
    }
}

pub fn emit_full_power_exhausted_state_payloads(
    mut released: EventReader<FullPowerReleasedEvent>,
    mut expired: EventReader<ExhaustedExpiredEvent>,
    ids: Query<&UniqueId>,
    positions: Query<&Position>,
    exhausted_q: Query<&Exhausted>,
    mut clients: Query<&mut Client, With<Client>>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    for event in released.read() {
        let Ok(exhausted) = exhausted_q.get(event.caster) else {
            continue;
        };
        broadcast_server_data(
            ServerDataPayloadV1::FullPowerExhaustedState(FullPowerExhaustedStateV1 {
                caster_uuid: actor_id(event.caster, ids.get(event.caster).ok()),
                active: true,
                started_tick: exhausted.started_at_tick,
                recovery_at_tick: exhausted.recovery_at_tick,
            }),
            &mut clients,
        );

        if let Ok(position) = positions.get(event.caster) {
            let origin = position.get();
            vfx_events.send(VfxEventRequest::new(
                origin,
                VfxEventPayloadV1::SpawnParticle {
                    event_id: EXHAUSTED_GREY_MIST_EVENT_ID.to_string(),
                    origin: [origin.x, origin.y + 0.8, origin.z],
                    direction: None,
                    color: Some("#7D7782".to_string()),
                    strength: Some(0.65),
                    count: Some(12),
                    duration_ticks: Some(40),
                },
            ));
        }
    }

    for event in expired.read() {
        broadcast_server_data(
            ServerDataPayloadV1::FullPowerExhaustedState(FullPowerExhaustedStateV1 {
                caster_uuid: actor_id(event.entity, ids.get(event.entity).ok()),
                active: false,
                started_tick: event.at_tick,
                recovery_at_tick: event.at_tick,
            }),
            &mut clients,
        );
    }
}

fn broadcast_charging_clear(
    entity: Entity,
    tick: u64,
    ids: &Query<&UniqueId>,
    clients: &mut Query<&mut Client, With<Client>>,
) {
    broadcast_server_data(
        ServerDataPayloadV1::FullPowerChargingState(FullPowerChargingStateV1 {
            caster_uuid: actor_id(entity, ids.get(entity).ok()),
            active: false,
            qi_committed: 0.0,
            target_qi: 0.0,
            started_tick: tick,
        }),
        clients,
    );
}

fn broadcast_server_data(
    payload: ServerDataPayloadV1,
    clients: &mut Query<&mut Client, With<Client>>,
) {
    let payload = ServerDataV1::new(payload);
    let payload_type = payload_type_label(payload.payload_type());
    let payload_bytes = match serialize_server_data_payload(&payload) {
        Ok(bytes) => bytes,
        Err(error) => {
            log_payload_build_error(payload_type, &error);
            return;
        }
    };

    for mut client in clients.iter_mut() {
        send_server_data_payload(&mut client, payload_bytes.as_slice());
    }
}

fn actor_id(entity: Entity, unique_id: Option<&UniqueId>) -> String {
    unique_id
        .map(|id| id.0.to_string())
        .unwrap_or_else(|| format!("entity:{entity:?}"))
}

fn charge_strength(charging: &ChargingState) -> f32 {
    if charging.target_qi <= f64::EPSILON {
        return 0.1;
    }
    (charging.qi_committed / charging.target_qi).clamp(0.1, 1.0) as f32
}
