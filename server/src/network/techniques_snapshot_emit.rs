use std::collections::HashSet;

use valence::prelude::{Added, Changed, Client, Entity, Query, Res, Username, With};

use crate::combat::sword_basics::sword_proficiency_label;
use crate::cultivation::known_techniques::{KnownTechniques, TechniqueRegistry};
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, serialize_server_data_payload_proto,
    PayloadBuildError, SERVER_DATA_CHANNEL,
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
    registry: Res<TechniqueRegistry>,
    mut clients: Query<TechniquesSnapshotQueryItem<'_>, TechniquesSnapshotFilter>,
) {
    for (entity, mut client, username, known) in &mut clients {
        send_techniques_snapshot_to_client(
            &registry,
            entity,
            &mut client,
            username.0.as_str(),
            known,
        );
    }
}

pub fn emit_join_techniques_snapshot_payloads(
    registry: Res<TechniqueRegistry>,
    mut clients: Query<TechniquesSnapshotQueryItem<'_>, JoinTechniquesSnapshotFilter>,
) {
    for (entity, mut client, username, known) in &mut clients {
        send_techniques_snapshot_to_client(
            &registry,
            entity,
            &mut client,
            username.0.as_str(),
            known,
        );
    }
}

fn techniques_snapshot_payload(
    registry: &TechniqueRegistry,
    known: &KnownTechniques,
) -> ServerDataV1 {
    let mut seen_ids = HashSet::new();
    let snapshot = TechniquesSnapshotV1 {
        entries: known
            .entries
            .iter()
            .filter_map(|known| {
                if !seen_ids.insert(known.id.as_str()) {
                    return None;
                }
                let definition = registry.get(&known.id)?;
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
    ServerDataV1::new(ServerDataPayloadV1::TechniquesSnapshot(snapshot))
}

/// 启动期按“玩家学会完整 catalog”构造最坏情况快照，并强制走生产 protobuf 编码。
/// registry 启动后不可变，因此通过即保证任意 `KnownTechniques` 子集不会因整包超限被丢弃。
pub fn validate_techniques_snapshot_budget(
    registry: &TechniqueRegistry,
) -> Result<usize, PayloadBuildError> {
    let known = KnownTechniques {
        entries: registry
            .iter()
            .map(|definition| crate::cultivation::known_techniques::KnownTechnique {
                id: definition.id.clone(),
                proficiency: 1.0,
                active: true,
            })
            .collect(),
    };
    serialize_server_data_payload_proto(&techniques_snapshot_payload(registry, &known))
        .map(|bytes| bytes.len())
}

pub fn send_techniques_snapshot_to_client(
    registry: &TechniqueRegistry,
    entity: Entity,
    client: &mut Client,
    username: &str,
    known: &KnownTechniques,
) {
    let payload = techniques_snapshot_payload(registry, known);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::common::MAX_PAYLOAD_BYTES;
    use crate::schema::server_data::ServerDataBuildError;

    fn registry_with_description_len(len: usize) -> TechniqueRegistry {
        TechniqueRegistry::load_for_tests_with_override("movement.dash", |definition| {
            definition.description = "x".repeat(len);
        })
    }

    fn raw_full_snapshot_len(registry: &TechniqueRegistry) -> usize {
        let known = KnownTechniques::dev_default(registry);
        techniques_snapshot_payload(registry, &known)
            .to_proto_bytes()
            .len()
    }

    fn description_len_for_payload_size(target: usize) -> usize {
        let mut low = 0usize;
        let mut high = target * 2;
        while low <= high {
            let mid = low + (high - low) / 2;
            match raw_full_snapshot_len(&registry_with_description_len(mid)).cmp(&target) {
                std::cmp::Ordering::Less => low = mid + 1,
                std::cmp::Ordering::Greater => high = mid.saturating_sub(1),
                std::cmp::Ordering::Equal => return mid,
            }
        }
        panic!("could not construct a techniques snapshot with exact protobuf size {target}");
    }

    #[test]
    fn duplicate_persisted_ids_cannot_exceed_the_catalog_snapshot_budget() {
        let registry = TechniqueRegistry::load_for_tests();
        let known = KnownTechniques {
            entries: (0..10_000)
                .map(|_| crate::cultivation::known_techniques::KnownTechnique {
                    id: "movement.dash".to_string(),
                    proficiency: 1.0,
                    active: true,
                })
                .collect(),
        };
        let ServerDataPayloadV1::TechniquesSnapshot(snapshot) =
            techniques_snapshot_payload(&registry, &known).payload
        else {
            panic!("expected techniques snapshot payload");
        };
        assert_eq!(
            snapshot.entries.len(),
            1,
            "persisted duplicate ids must be canonicalized to a registry subset"
        );
    }

    #[test]
    fn checked_in_full_catalog_fits_real_protobuf_budget() {
        let registry = TechniqueRegistry::load_for_tests();
        let size = validate_techniques_snapshot_budget(&registry)
            .expect("checked-in full learned-technique snapshot must fit");
        assert!(size <= MAX_PAYLOAD_BYTES, "full catalog encoded to {size} bytes");
    }

    #[test]
    fn exact_protobuf_budget_is_accepted_and_one_byte_over_is_rejected() {
        let exact_registry = registry_with_description_len(description_len_for_payload_size(
            MAX_PAYLOAD_BYTES,
        ));
        assert_eq!(raw_full_snapshot_len(&exact_registry), MAX_PAYLOAD_BYTES);
        assert_eq!(
            validate_techniques_snapshot_budget(&exact_registry)
                .expect("exact MAX_PAYLOAD_BYTES snapshot must be accepted"),
            MAX_PAYLOAD_BYTES
        );

        let over_registry = registry_with_description_len(description_len_for_payload_size(
            MAX_PAYLOAD_BYTES + 1,
        ));
        assert_eq!(raw_full_snapshot_len(&over_registry), MAX_PAYLOAD_BYTES + 1);
        let error = validate_techniques_snapshot_budget(&over_registry)
            .expect_err("one-byte-over full snapshot must fail startup validation");
        assert!(matches!(
            error,
            ServerDataBuildError::Oversize { size, max }
                if size == MAX_PAYLOAD_BYTES + 1 && max == MAX_PAYLOAD_BYTES
        ));
    }
}
