//! plan-HUD-v1 §3.3 / §3.4 server-side emit for `derived_attrs_sync` payload.
//!
//! 监听 `Changed<DerivedAttrs>` / `Changed<TribulationState>`（含 join 首次 attach），
//! 把替尸伪皮层数、绝灵涡流状态与天劫锁定态推给 client HUD；飞行/虚化字段在 v1
//! 先保留默认值，后续对应系统接入时直接填充。

use std::collections::HashSet;

use valence::prelude::{
    Changed, Client, Entity, Or, ParamSet, Query, RemovedComponents, Username, With,
};

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
type DerivedAttrsEmitFilter = (
    With<Client>,
    Or<(Changed<DerivedAttrs>, Changed<TribulationState>)>,
);
type DerivedAttrsEmitQuery<'w, 's> =
    Query<'w, 's, DerivedAttrsEmitQueryItem<'static>, DerivedAttrsEmitFilter>;
type DerivedAttrsAnyClientQuery<'w, 's> =
    Query<'w, 's, DerivedAttrsEmitQueryItem<'static>, With<Client>>;

pub fn emit_derived_attrs_sync_payloads(
    mut clients: ParamSet<(
        DerivedAttrsEmitQuery<'_, '_>,
        DerivedAttrsAnyClientQuery<'_, '_>,
    )>,
    mut removed_tribulations: RemovedComponents<TribulationState>,
) {
    let mut emitted = HashSet::new();
    {
        let mut changed_clients = clients.p0();
        for (entity, mut client, username, attrs, tribulation) in &mut changed_clients {
            send_derived_attrs_sync_payload(entity, &mut client, username, attrs, tribulation);
            emitted.insert(entity);
        }
    }

    let removed_entities: Vec<_> = removed_tribulations.read().collect();
    if removed_entities.is_empty() {
        return;
    }

    let mut all_clients = clients.p1();
    for entity in removed_entities {
        if emitted.contains(&entity) {
            continue;
        }
        let Ok((entity, mut client, username, attrs, tribulation)) = all_clients.get_mut(entity)
        else {
            continue;
        };
        send_derived_attrs_sync_payload(entity, &mut client, username, attrs, tribulation);
    }
}

fn send_derived_attrs_sync_payload(
    entity: Entity,
    client: &mut Client,
    username: &Username,
    attrs: &DerivedAttrs,
    tribulation: Option<&TribulationState>,
) {
    let payload = ServerDataV1::new(ServerDataPayloadV1::DerivedAttrsSync(DerivedAttrsSyncV1 {
        flying: false,
        flying_qi_remaining: 0.0,
        flying_force_descent_at_ms: 0,
        phasing: false,
        phasing_until_ms: 0,
        tribulation_locked: tribulation.is_some(),
        tribulation_stage: String::new(),
        throughput_peak_norm: 0.0,
        tuike_layers: attrs.tuike_layers,
        vortex_active: attrs.vortex_active,
    }));
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
        "[bong][network] sent {} {} payload to entity {entity:?} for `{}` (tuike_layers={} vortex_active={})",
        SERVER_DATA_CHANNEL,
        payload_type,
        username.0,
        attrs.tuike_layers,
        attrs.vortex_active
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::{App, Update};
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    fn flush_client_packets(app: &mut App) {
        let world = app.world_mut();
        let mut query = world.query::<&mut Client>();
        for mut client in query.iter_mut(world) {
            client
                .flush_packets()
                .expect("mock client packets should flush");
        }
    }

    fn collect_derived_attrs_payloads(helper: &mut MockClientHelper) -> Vec<DerivedAttrsSyncV1> {
        helper
            .collect_received()
            .0
            .into_iter()
            .filter_map(|frame| {
                let packet = frame.decode::<CustomPayloadS2c>().ok()?;
                if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                    return None;
                }
                let payload = serde_json::from_slice::<ServerDataV1>(packet.data.0 .0).ok()?;
                match payload.payload {
                    ServerDataPayloadV1::DerivedAttrsSync(attrs) => Some(attrs),
                    _ => None,
                }
            })
            .collect()
    }

    #[test]
    fn emits_when_tribulation_state_changes_without_derived_attrs_change() {
        let mut app = App::new();
        app.add_systems(Update, emit_derived_attrs_sync_payloads);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                DerivedAttrs {
                    tuike_layers: 2,
                    vortex_active: true,
                    ..DerivedAttrs::default()
                },
            ))
            .id();

        app.update();
        flush_client_packets(&mut app);
        let initial = collect_derived_attrs_payloads(&mut helper);
        assert_eq!(initial.len(), 1);
        assert!(!initial[0].tribulation_locked);

        app.world_mut()
            .entity_mut(entity)
            .insert(TribulationState::restored(1, 3, 10));
        app.update();
        flush_client_packets(&mut app);

        let changed = collect_derived_attrs_payloads(&mut helper);
        assert_eq!(changed.len(), 1);
        assert!(changed[0].tribulation_locked);
        assert_eq!(changed[0].tuike_layers, 2);
        assert!(changed[0].vortex_active);
    }

    #[test]
    fn emits_clear_when_tribulation_state_is_removed() {
        let mut app = App::new();
        app.add_systems(Update, emit_derived_attrs_sync_payloads);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                DerivedAttrs::default(),
                TribulationState::restored(1, 3, 10),
            ))
            .id();

        app.update();
        flush_client_packets(&mut app);
        let initial = collect_derived_attrs_payloads(&mut helper);
        assert_eq!(initial.len(), 1);
        assert!(initial[0].tribulation_locked);

        app.world_mut()
            .entity_mut(entity)
            .remove::<TribulationState>();
        app.update();
        flush_client_packets(&mut app);

        let cleared = collect_derived_attrs_payloads(&mut helper);
        assert_eq!(cleared.len(), 1);
        assert!(!cleared[0].tribulation_locked);
    }
}
