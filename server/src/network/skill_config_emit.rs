use std::collections::{HashMap, HashSet};

use valence::prelude::{Client, Entity, Local, Query, Res, Username, With};

use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::player::state::canonical_player_id;
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
use crate::skill::config::{SkillConfigSnapshot, SkillConfigStore};

pub fn emit_skill_config_snapshots(
    store: Option<Res<SkillConfigStore>>,
    mut sent: Local<HashMap<Entity, (String, SkillConfigSnapshot)>>,
    mut clients: Query<(Entity, &mut Client, &Username), With<Client>>,
) {
    let Some(store) = store.as_deref() else {
        return;
    };
    let mut active_clients = HashSet::new();
    for (entity, mut client, username) in &mut clients {
        active_clients.insert(entity);
        let player_id = canonical_player_id(username.0.as_str());
        let snapshot = store.snapshot_for_player(player_id.as_str());
        if sent.get(&entity).is_some_and(|(cached_player_id, cached)| {
            cached_player_id == &player_id && cached == &snapshot
        }) {
            continue;
        }

        if send_skill_config_snapshot_to_client(
            &mut client,
            snapshot.clone(),
            entity,
            username.0.as_str(),
        ) {
            sent.insert(entity, (player_id, snapshot));
        }
    }
    sent.retain(|entity, _| active_clients.contains(entity));
}

pub(crate) fn send_skill_config_snapshot_to_client(
    client: &mut Client,
    snapshot: SkillConfigSnapshot,
    entity: Entity,
    username: &str,
) -> bool {
    let payload = ServerDataV1::new(ServerDataPayloadV1::SkillConfigSnapshot(snapshot));
    let payload_type = payload_type_label(payload.payload_type());
    let payload_bytes = match serialize_server_data_payload(&payload) {
        Ok(bytes) => bytes,
        Err(error) => {
            log_payload_build_error(payload_type, &error);
            return false;
        }
    };
    send_server_data_payload(client, payload_bytes.as_slice());
    tracing::debug!(
        "[bong][network] sent {} {} payload to entity {entity:?} for `{}`",
        SERVER_DATA_CHANNEL,
        payload_type,
        username
    );
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
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

    fn collect_skill_config_snapshots(helper: &mut MockClientHelper) -> Vec<SkillConfigSnapshot> {
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
                    ServerDataPayloadV1::SkillConfigSnapshot(snapshot) => Some(snapshot),
                    _ => None,
                }
            })
            .collect()
    }

    #[test]
    fn reconnect_with_same_player_id_receives_fresh_snapshot() {
        let mut app = App::new();
        app.init_resource::<SkillConfigStore>();
        app.add_systems(Update, emit_skill_config_snapshots);
        app.world_mut()
            .resource_mut::<SkillConfigStore>()
            .set_config(
                "offline:Azure",
                "zhenmai.sever_chain",
                crate::skill::config::SkillConfig::new(std::collections::BTreeMap::from([(
                    "backfire_kind".to_string(),
                    json!("array"),
                )])),
            );

        let (client_bundle, mut first_helper) = create_mock_client("Azure");
        let first_entity = app.world_mut().spawn(client_bundle).id();
        app.update();
        flush_client_packets(&mut app);
        assert_eq!(collect_skill_config_snapshots(&mut first_helper).len(), 1);

        app.update();
        flush_client_packets(&mut app);
        assert!(collect_skill_config_snapshots(&mut first_helper).is_empty());

        app.world_mut().despawn(first_entity);
        let (client_bundle, mut second_helper) = create_mock_client("Azure");
        app.world_mut().spawn(client_bundle);
        app.update();
        flush_client_packets(&mut app);

        let snapshots = collect_skill_config_snapshots(&mut second_helper);
        assert_eq!(snapshots.len(), 1);
        assert!(snapshots[0].configs.contains_key("zhenmai.sever_chain"));
    }
}
