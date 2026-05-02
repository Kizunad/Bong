//! plan-HUD-v1 §3.3 / §3.4 server-side emit for `derived_attrs_sync` payload.
//!
//! 监听 `Changed<DerivedAttrs>`（含 join 首次 attach），把替尸伪皮层数与绝灵涡流状态
//! 推给 client HUD；飞行/虚化字段在 v1 先保留默认值，后续对应系统接入时直接填充。

use valence::prelude::{Changed, Client, Entity, Query, Username, With};

use crate::combat::components::DerivedAttrs;
use crate::cultivation::tribulation::TribulationState;
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::combat_hud::DerivedAttrsSyncV1;
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};

type DerivedAttrsEmitQueryItem<'a> = (
    Entity,
    &'a mut Client,
    &'a Username,
    &'a DerivedAttrs,
    Option<&'a TribulationState>,
);
type DerivedAttrsEmitFilter = (With<Client>, Changed<DerivedAttrs>);

pub fn emit_derived_attrs_sync_payloads(
    mut clients: Query<DerivedAttrsEmitQueryItem<'_>, DerivedAttrsEmitFilter>,
) {
    for (entity, mut client, username, attrs, tribulation) in &mut clients {
        let payload =
            ServerDataV1::new(ServerDataPayloadV1::DerivedAttrsSync(DerivedAttrsSyncV1 {
                flying: false,
                flying_qi_remaining: 0.0,
                flying_force_descent_at_ms: 0,
                phasing: false,
                phasing_until_ms: 0,
                tribulation_locked: tribulation.is_some(),
                tribulation_stage: String::new(),
                throughput_peak_norm: 0.0,
                vortex_fake_skin_layers: attrs.vortex_fake_skin_layers,
                vortex_ready: attrs.vortex_ready,
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
            "[bong][network] sent {} {} payload to entity {entity:?} for `{}` (fake_skin_layers={} vortex_ready={})",
            SERVER_DATA_CHANNEL,
            payload_type,
            username.0,
            attrs.vortex_fake_skin_layers,
            attrs.vortex_ready
        );
    }
}
