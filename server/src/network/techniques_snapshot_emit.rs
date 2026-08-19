use valence::prelude::{Added, Changed, Client, Entity, Query, Username, With};

use crate::combat::sword_basics::sword_proficiency_label;
use crate::cultivation::known_techniques::{KnownTechniques, TECHNIQUE_DEFINITIONS};
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::combat_hud::{
    TechniqueEntryV1, TechniqueRequiredMeridianV1, TechniquesSnapshotV1,
};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};

type TechniquesSnapshotFilter = (With<Client>, Changed<KnownTechniques>);
type JoinTechniquesSnapshotFilter = (With<Client>, Added<KnownTechniques>);
type TechniquesSnapshotQueryItem<'a> = (Entity, &'a mut Client, &'a Username, &'a KnownTechniques);

pub fn emit_techniques_snapshot_payloads(
    mut clients: Query<TechniquesSnapshotQueryItem<'_>, TechniquesSnapshotFilter>,
) {
    for (entity, mut client, username, known) in &mut clients {
        send_techniques_snapshot_to_client(entity, &mut client, username.0.as_str(), known);
    }
}

pub fn emit_join_techniques_snapshot_payloads(
    mut clients: Query<TechniquesSnapshotQueryItem<'_>, JoinTechniquesSnapshotFilter>,
) {
    for (entity, mut client, username, known) in &mut clients {
        send_techniques_snapshot_to_client(entity, &mut client, username.0.as_str(), known);
    }
}

pub fn send_techniques_snapshot_to_client(
    entity: Entity,
    client: &mut Client,
    username: &str,
    known: &KnownTechniques,
) {
    let snapshot = TechniquesSnapshotV1 {
        entries: known
            .entries
            .iter()
            .filter_map(|known| {
                let definition = TECHNIQUE_DEFINITIONS
                    .iter()
                    .find(|definition| definition.id == known.id)?;
                Some(TechniqueEntryV1 {
                    id: definition.id.to_string(),
                    display_name: definition.display_name.to_string(),
                    grade: definition.grade.to_string(),
                    proficiency: known.proficiency.clamp(0.0, 1.0),
                    proficiency_label: sword_proficiency_label(known.proficiency).to_string(),
                    active: known.active,
                    description: definition.description.to_string(),
                    required_realm: definition.required_realm.to_string(),
                    required_meridians: definition
                        .required_meridians
                        .iter()
                        .map(|required| TechniqueRequiredMeridianV1 {
                            channel: required.channel.to_string(),
                            min_health: required.min_health,
                        })
                        .collect(),
                    qi_cost: definition.qi_cost,
                    stamina_cost: definition.stamina_cost,
                    cast_ticks: definition.cast_ticks,
                    cooldown_ticks: definition.cooldown_ticks,
                    range: definition.range,
                })
            })
            .collect(),
    };
    let payload = ServerDataV1::new(ServerDataPayloadV1::TechniquesSnapshot(snapshot));
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
        "[bong][network] sent {} {} payload to entity {entity:?} for `{username}`",
        SERVER_DATA_CHANNEL,
        payload_type
    );
}
