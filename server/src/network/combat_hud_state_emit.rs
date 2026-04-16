//! plan-HUD-v1 §11.4 server-side emit for `combat_hud_state` payload.
//!
//! Watches `Changed<Cultivation> | Changed<Stamina>` per client and pushes the
//! aggregated percentages so left-bottom mini body / qi-bar / stamina-bar refresh.
//! HP percent is currently fixed at 1.0 (Wounds aggregation TODO);
//! DerivedAttrFlags are all-false until Flying/Phasing/TribulationLocked
//! components land.

use valence::prelude::{Changed, Client, Entity, Or, Query, Username, With};

use crate::combat::components::{Stamina, Wounds};
use crate::cultivation::components::Cultivation;
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::combat_hud::{CombatHudStateV1, DerivedAttrFlagsV1};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};

type CombatHudEmitQueryItem<'a> = (
    Entity,
    &'a mut Client,
    &'a Username,
    &'a Cultivation,
    &'a Stamina,
    &'a Wounds,
);

type CombatHudEmitFilter = (
    With<Client>,
    Or<(Changed<Cultivation>, Changed<Stamina>, Changed<Wounds>)>,
);

pub fn emit_combat_hud_state_payloads(
    mut clients: Query<CombatHudEmitQueryItem<'_>, CombatHudEmitFilter>,
) {
    for (entity, mut client, username, cultivation, stamina, wounds) in &mut clients {
        let qi_percent = if cultivation.qi_max > 0.0 {
            (cultivation.qi_current / cultivation.qi_max).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
        let stamina_percent = if stamina.max > 0.0 {
            (stamina.current / stamina.max).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let hp_percent = if wounds.health_max > 0.0 {
            (wounds.health_current / wounds.health_max).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let payload = ServerDataV1::new(ServerDataPayloadV1::CombatHudState(CombatHudStateV1 {
            hp_percent,
            qi_percent,
            stamina_percent,
            // TODO: surface Flying/Phasing/TribulationLocked components when they exist.
            derived: DerivedAttrFlagsV1::default(),
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
            "[bong][network] sent {} {} payload to entity {entity:?} for `{}` (hp={:.2} qi={:.2} stam={:.2})",
            SERVER_DATA_CHANNEL,
            payload_type,
            username.0,
            hp_percent,
            qi_percent,
            stamina_percent,
        );
    }
}
