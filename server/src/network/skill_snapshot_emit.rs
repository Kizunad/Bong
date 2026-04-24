use valence::prelude::{bevy_ecs, Added, Client, Entity, Query, Username, With};

use crate::cultivation::breakthrough::skill_cap_for_realm;
use crate::cultivation::components::Cultivation;
use crate::cultivation::death_hooks::PlayerRevived;
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
use crate::schema::skill::SkillSnapshotPayloadV1;
use crate::skill::components::SkillSet;

type JoinedClientQueryItem<'a> = (
    Entity,
    &'a mut Client,
    &'a Username,
    &'a SkillSet,
    &'a Cultivation,
);

pub fn emit_join_skill_snapshots(
    mut joined_clients: Query<JoinedClientQueryItem<'_>, (With<Client>, Added<SkillSet>)>,
) {
    for (entity, mut client, username, skill_set, cultivation) in &mut joined_clients {
        send_skill_snapshot_to_client(
            entity,
            &mut client,
            username.0.as_str(),
            skill_set,
            cultivation,
            "join",
        );
    }
}

pub fn emit_revive_skill_resyncs(
    mut revived: bevy_ecs::event::EventReader<PlayerRevived>,
    mut clients: Query<(&mut Client, &Username, &SkillSet, &Cultivation), With<Client>>,
) {
    for event in revived.read() {
        let Ok((mut client, username, skill_set, cultivation)) = clients.get_mut(event.entity)
        else {
            continue;
        };
        send_skill_snapshot_to_client(
            event.entity,
            &mut client,
            username.0.as_str(),
            skill_set,
            cultivation,
            "revive_resync",
        );
    }
}

pub(crate) fn send_skill_snapshot_to_client(
    entity: Entity,
    client: &mut Client,
    username: &str,
    skill_set: &SkillSet,
    cultivation: &Cultivation,
    reason: &str,
) {
    let snapshot = SkillSnapshotPayloadV1::from_runtime(entity.to_bits(), skill_set, |_| {
        skill_cap_for_realm(cultivation.realm)
    });
    let payload = ServerDataV1::new(ServerDataPayloadV1::SkillSnapshot(Box::new(snapshot)));
    let payload_type = payload_type_label(payload.payload_type());
    let payload_bytes = match serialize_server_data_payload(&payload) {
        Ok(bytes) => bytes,
        Err(error) => {
            log_payload_build_error(payload_type, &error);
            return;
        }
    };

    send_server_data_payload(client, payload_bytes.as_slice());
    tracing::info!(
        "[bong][network] sent {} {} payload to client entity {entity:?} for `{}` ({reason})",
        SERVER_DATA_CHANNEL,
        payload_type,
        username,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::Realm;
    use crate::skill::components::{SkillEntry, SkillId};

    #[test]
    fn snapshot_uses_cultivation_cap_for_all_skills() {
        let mut set = SkillSet::default();
        set.skills.insert(
            SkillId::Forging,
            SkillEntry {
                lv: 7,
                xp: 120,
                total_xp: 10_000,
                last_action_at: 1,
                recent_repeat_count: 0,
            },
        );
        let cult = Cultivation {
            realm: Realm::Induce,
            ..Default::default()
        };

        let snapshot =
            SkillSnapshotPayloadV1::from_runtime(42, &set, |_| skill_cap_for_realm(cult.realm));
        assert_eq!(snapshot.skills.get("forging").unwrap().cap, 5);
        assert_eq!(snapshot.skills.get("forging").unwrap().lv, 7);
    }
}
