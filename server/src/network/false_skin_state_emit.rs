//! plan-tuike-v1 — server-side emit for `false_skin_state` HUD payload.

use valence::prelude::{
    Changed, Client, Entity, ParamSet, Query, RemovedComponents, Username, With,
};

use crate::combat::components::Lifecycle;
use crate::combat::tuike::{empty_false_skin_state, FalseSkin};
use crate::combat::tuike_v2::{FalseSkinTier, StackedFalseSkins};
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::npc::brain::canonical_npc_id;
use crate::npc::spawn::NpcMarker;
use crate::player::state::canonical_player_id;
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
use crate::schema::tuike::FalseSkinStateV1;

type ChangedFalseSkinClient<'a> = (
    Entity,
    &'a mut Client,
    &'a Username,
    &'a FalseSkin,
    Option<&'a Lifecycle>,
);
type AnyClient<'a> = (
    Entity,
    &'a mut Client,
    &'a Username,
    Option<&'a FalseSkin>,
    Option<&'a Lifecycle>,
);
type ChangedStackedFalseSkinClient<'a> = (
    Entity,
    &'a mut Client,
    &'a Username,
    &'a StackedFalseSkins,
    Option<&'a Lifecycle>,
);
type AnyStackedClient<'a> = (
    Entity,
    &'a mut Client,
    &'a Username,
    Option<&'a StackedFalseSkins>,
    Option<&'a Lifecycle>,
);

#[allow(clippy::type_complexity)] // Bevy ParamSet keeps changed/removed client paths in one system.
pub fn emit_false_skin_state_payloads(
    mut clients: ParamSet<(
        Query<ChangedFalseSkinClient<'static>, (With<Client>, Changed<FalseSkin>)>,
        Query<AnyClient<'static>, With<Client>>,
    )>,
    npc_markers: Query<(), With<NpcMarker>>,
    mut removed: RemovedComponents<FalseSkin>,
) {
    {
        let mut changed = clients.p0();
        for (entity, mut client, username, skin, lifecycle) in &mut changed {
            let target_id = target_id_for(entity, username, lifecycle, &npc_markers);
            send_false_skin_state(entity, &mut client, username, skin.state_payload(target_id));
        }
    }

    let removed_entities = removed.read().collect::<Vec<_>>();
    if removed_entities.is_empty() {
        return;
    }

    let mut all = clients.p1();
    for entity in removed_entities {
        let Ok((entity, mut client, username, skin, lifecycle)) = all.get_mut(entity) else {
            continue;
        };
        if skin.is_some() {
            continue;
        }
        let target_id = target_id_for(entity, username, lifecycle, &npc_markers);
        send_false_skin_state(
            entity,
            &mut client,
            username,
            empty_false_skin_state(target_id),
        );
    }
}

#[allow(clippy::type_complexity)]
pub fn emit_tuike_v2_false_skin_state_payloads(
    mut clients: ParamSet<(
        Query<ChangedStackedFalseSkinClient<'static>, (With<Client>, Changed<StackedFalseSkins>)>,
        Query<AnyStackedClient<'static>, With<Client>>,
    )>,
    npc_markers: Query<(), With<NpcMarker>>,
    mut removed: RemovedComponents<StackedFalseSkins>,
) {
    {
        let mut changed = clients.p0();
        for (entity, mut client, username, stack, lifecycle) in &mut changed {
            let target_id = target_id_for(entity, username, lifecycle, &npc_markers);
            send_false_skin_state(
                entity,
                &mut client,
                username,
                stacked_state_payload(target_id, stack),
            );
        }
    }

    let removed_entities = removed.read().collect::<Vec<_>>();
    if removed_entities.is_empty() {
        return;
    }

    let mut all = clients.p1();
    for entity in removed_entities {
        let Ok((entity, mut client, username, stack, lifecycle)) = all.get_mut(entity) else {
            continue;
        };
        if stack.is_some() {
            continue;
        }
        let target_id = target_id_for(entity, username, lifecycle, &npc_markers);
        send_false_skin_state(
            entity,
            &mut client,
            username,
            empty_false_skin_state(target_id),
        );
    }
}

fn send_false_skin_state(
    entity: Entity,
    client: &mut Client,
    username: &Username,
    state: FalseSkinStateV1,
) {
    let payload = ServerDataV1::new(ServerDataPayloadV1::FalseSkinState(state.clone()));
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
        "[bong][network] sent {} {} payload to entity {entity:?} for `{}` (layers={})",
        SERVER_DATA_CHANNEL,
        payload_type,
        username.0,
        state.layers_remaining,
    );
}

fn stacked_state_payload(target_id: String, stack: &StackedFalseSkins) -> FalseSkinStateV1 {
    let outer = stack.outer();
    FalseSkinStateV1 {
        target_id,
        kind: outer.map(|layer| false_skin_kind_for_tier(layer.tier)),
        layers_remaining: stack.layer_count().min(3) as u8,
        contam_capacity_per_layer: outer
            .map(|layer| layer.contam_capacity_percent())
            .unwrap_or(0.0),
        absorbed_contam: outer.map(|layer| layer.contam_load).unwrap_or(0.0),
        equipped_at_tick: outer.map(|layer| layer.equipped_at_tick).unwrap_or(0),
    }
}

fn false_skin_kind_for_tier(tier: FalseSkinTier) -> crate::schema::tuike::FalseSkinKindV1 {
    match tier {
        FalseSkinTier::Fan | FalseSkinTier::Light => {
            crate::schema::tuike::FalseSkinKindV1::SpiderSilk
        }
        FalseSkinTier::Mid | FalseSkinTier::Heavy | FalseSkinTier::Ancient => {
            crate::schema::tuike::FalseSkinKindV1::RottenWoodArmor
        }
    }
}

fn target_id_for(
    entity: Entity,
    username: &Username,
    lifecycle: Option<&Lifecycle>,
    npc_markers: &Query<(), With<NpcMarker>>,
) -> String {
    lifecycle
        .map(|lifecycle| lifecycle.character_id.clone())
        .or_else(|| {
            npc_markers
                .get(entity)
                .ok()
                .map(|_| canonical_npc_id(entity))
        })
        .unwrap_or_else(|| canonical_player_id(username.0.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::tuike::{FalseSkin, FalseSkinKind};
    use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
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

    fn collect_false_skin_payloads(helper: &mut MockClientHelper) -> Vec<FalseSkinStateV1> {
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
                    ServerDataPayloadV1::FalseSkinState(state) => Some(state),
                    _ => None,
                }
            })
            .collect()
    }

    #[test]
    fn emits_state_on_false_skin_change() {
        let mut app = App::new();
        app.add_systems(Update, emit_false_skin_state_payloads);
        let (client_bundle, mut helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                FalseSkin::fresh(7, FalseSkinKind::SpiderSilk, 3),
            ))
            .id();

        app.update();
        flush_client_packets(&mut app);

        let payloads = collect_false_skin_payloads(&mut helper);
        assert_eq!(payloads.len(), 1);
        assert_eq!(payloads[0].target_id, "offline:Azure");
        assert_eq!(payloads[0].layers_remaining, 1);

        app.world_mut().entity_mut(entity).remove::<FalseSkin>();
        app.update();
        flush_client_packets(&mut app);

        let cleared = collect_false_skin_payloads(&mut helper);
        assert_eq!(cleared.len(), 1);
        assert_eq!(cleared[0].layers_remaining, 0);
    }
}
