use std::collections::HashMap;

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
    mut sent: Local<HashMap<String, SkillConfigSnapshot>>,
    mut clients: Query<(Entity, &mut Client, &Username), With<Client>>,
) {
    let Some(store) = store.as_deref() else {
        return;
    };
    for (entity, mut client, username) in &mut clients {
        let player_id = canonical_player_id(username.0.as_str());
        let snapshot = store.snapshot_for_player(player_id.as_str());
        if sent.get(player_id.as_str()) == Some(&snapshot) {
            continue;
        }

        let payload = ServerDataV1::new(ServerDataPayloadV1::SkillConfigSnapshot(snapshot.clone()));
        let payload_type = payload_type_label(payload.payload_type());
        let payload_bytes = match serialize_server_data_payload(&payload) {
            Ok(bytes) => bytes,
            Err(error) => {
                log_payload_build_error(payload_type, &error);
                continue;
            }
        };
        send_server_data_payload(&mut client, payload_bytes.as_slice());
        sent.insert(player_id, snapshot);
        tracing::debug!(
            "[bong][network] sent {} {} payload to entity {entity:?} for `{}`",
            SERVER_DATA_CHANNEL,
            payload_type,
            username.0
        );
    }
}
