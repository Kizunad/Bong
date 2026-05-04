use valence::prelude::{Client, Entity, Query, Res, UniqueId, Username, With};

use crate::combat::components::TICKS_PER_SECOND;
use crate::combat::woliu::entity_wire_id;
use crate::combat::CombatClock;
use crate::cultivation::components::{Cultivation, MeridianSystem};
use crate::cultivation::dugu::{poison_state_payload, DuguPoisonState};
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};

type DuguStateClientItem<'a> = (
    Entity,
    &'a mut Client,
    &'a Username,
    Option<&'a UniqueId>,
    Option<&'a DuguPoisonState>,
    Option<&'a MeridianSystem>,
    Option<&'a Cultivation>,
);

pub fn emit_dugu_poison_state_payloads(
    clock: Res<CombatClock>,
    mut clients: Query<DuguStateClientItem<'_>, With<Client>>,
) {
    if !clock.tick.is_multiple_of(TICKS_PER_SECOND) {
        return;
    }

    for (entity, mut client, username, unique_id, poison, meridians, cultivation) in &mut clients {
        let payload =
            ServerDataV1::new(ServerDataPayloadV1::DuguPoisonState(poison_state_payload(
                entity_wire_id(unique_id, entity),
                poison,
                meridians,
                cultivation,
                clock.tick,
            )));
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
            "[bong][network] sent {} {} payload to entity {entity:?} for `{}` active={}",
            SERVER_DATA_CHANNEL,
            payload_type,
            username.0,
            poison.is_some()
        );
    }
}
