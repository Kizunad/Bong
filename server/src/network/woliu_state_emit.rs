use std::collections::HashMap;

use valence::prelude::{Client, Entity, EventReader, Local, Query, Res, UniqueId, Username, With};

use crate::combat::components::TICKS_PER_SECOND;
use crate::combat::woliu::{
    entity_wire_id, vortex_field_state_payload, ProjectileQiDrainedEvent, VortexField,
};
use crate::combat::CombatClock;
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};

#[derive(Default)]
pub struct VortexStateEmitCache {
    active: HashMap<Entity, bool>,
    intercepted_count: HashMap<Entity, u32>,
}

type VortexStateClientItem<'a> = (
    Entity,
    &'a mut Client,
    &'a Username,
    Option<&'a UniqueId>,
    Option<&'a VortexField>,
);

pub fn emit_vortex_state_payloads(
    clock: Res<CombatClock>,
    mut cache: Local<VortexStateEmitCache>,
    mut drained_events: EventReader<ProjectileQiDrainedEvent>,
    mut clients: Query<VortexStateClientItem<'_>, With<Client>>,
) {
    for event in drained_events.read() {
        let count = cache
            .intercepted_count
            .entry(event.field_caster)
            .or_default();
        *count = count.saturating_add(1);
    }

    let periodic = clock.tick.is_multiple_of(TICKS_PER_SECOND);
    for (entity, mut client, username, unique_id, field) in &mut clients {
        let active = field.is_some();
        let previously_active = cache.active.get(&entity).copied().unwrap_or(false);
        if !periodic && active == previously_active {
            continue;
        }

        cache.active.insert(entity, active);
        let intercepted_count = cache
            .intercepted_count
            .get(&entity)
            .copied()
            .unwrap_or_default();
        let payload = ServerDataV1::new(ServerDataPayloadV1::VortexState(
            vortex_field_state_payload(
                entity_wire_id(unique_id, entity),
                field,
                clock.tick,
                intercepted_count,
            ),
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
            "[bong][network] sent {} {} payload to entity {entity:?} for `{}` active={active}",
            SERVER_DATA_CHANNEL,
            payload_type,
            username.0
        );
    }
}
