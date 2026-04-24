use valence::prelude::{Client, Entity, EventReader, Query, With};

use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
use crate::schema::skill::{
    SkillCapChangedPayloadV1, SkillIdV1, SkillLvUpPayloadV1, SkillScrollUsedPayloadV1,
    SkillXpGainPayloadV1, XpGainSourceV1,
};
use crate::skill::components::SkillId;
use crate::skill::events::{
    SkillCapChanged, SkillLvUp, SkillScrollUsed, SkillXpGain, XpGainSource,
};

pub fn emit_skill_xp_gain_payloads(
    mut events: EventReader<SkillXpGain>,
    mut clients: Query<&mut Client, With<Client>>,
) {
    for event in events.read() {
        let Ok(mut client) = clients.get_mut(event.char_entity) else {
            continue;
        };

        let payload = ServerDataV1::new(ServerDataPayloadV1::SkillXpGain(Box::new(
            SkillXpGainPayloadV1::new(
                entity_wire_id(event.char_entity),
                skill_to_wire(event.skill),
                event.amount,
                source_to_wire(&event.source),
            ),
        )));
        send_payload(&mut client, payload);
    }
}

pub fn emit_skill_lv_up_payloads(
    mut events: EventReader<SkillLvUp>,
    mut clients: Query<&mut Client, With<Client>>,
) {
    for event in events.read() {
        let Ok(mut client) = clients.get_mut(event.char_entity) else {
            continue;
        };

        let payload = ServerDataV1::new(ServerDataPayloadV1::SkillLvUp(SkillLvUpPayloadV1::new(
            entity_wire_id(event.char_entity),
            skill_to_wire(event.skill),
            event.new_lv,
        )));
        send_payload(&mut client, payload);
    }
}

pub fn emit_skill_cap_changed_payloads(
    mut events: EventReader<SkillCapChanged>,
    mut clients: Query<&mut Client, With<Client>>,
) {
    for event in events.read() {
        let Ok(mut client) = clients.get_mut(event.char_entity) else {
            continue;
        };

        let payload = ServerDataV1::new(ServerDataPayloadV1::SkillCapChanged(
            SkillCapChangedPayloadV1::new(
                entity_wire_id(event.char_entity),
                skill_to_wire(event.skill),
                event.new_cap,
            ),
        ));
        send_payload(&mut client, payload);
    }
}

pub fn emit_skill_scroll_used_payloads(
    mut events: EventReader<SkillScrollUsed>,
    mut clients: Query<&mut Client, With<Client>>,
) {
    for event in events.read() {
        let Ok(mut client) = clients.get_mut(event.char_entity) else {
            continue;
        };

        let payload = ServerDataV1::new(ServerDataPayloadV1::SkillScrollUsed(Box::new(
            SkillScrollUsedPayloadV1::new(
                entity_wire_id(event.char_entity),
                event.scroll_id.as_str(),
                skill_to_wire(event.skill),
                event.xp_granted,
                event.was_duplicate,
            ),
        )));
        send_payload(&mut client, payload);
    }
}

fn send_payload(client: &mut Client, payload: ServerDataV1) {
    let payload_type = payload_type_label(payload.payload_type());
    let payload_bytes = match serialize_server_data_payload(&payload) {
        Ok(bytes) => bytes,
        Err(error) => {
            log_payload_build_error(payload_type, &error);
            return;
        }
    };

    send_server_data_payload(client, payload_bytes.as_slice());
    let _ = SERVER_DATA_CHANNEL;
}

fn entity_wire_id(entity: Entity) -> u64 {
    entity.to_bits()
}

fn skill_to_wire(skill: SkillId) -> SkillIdV1 {
    match skill {
        SkillId::Herbalism => SkillIdV1::Herbalism,
        SkillId::Alchemy => SkillIdV1::Alchemy,
        SkillId::Forging => SkillIdV1::Forging,
    }
}

fn source_to_wire(source: &XpGainSource) -> XpGainSourceV1 {
    match source {
        XpGainSource::Action { plan_id, action } => XpGainSourceV1::Action {
            plan_id: (*plan_id).to_string(),
            action: (*action).to_string(),
        },
        XpGainSource::Scroll {
            scroll_id,
            xp_grant,
        } => XpGainSourceV1::Scroll {
            scroll_id: scroll_id.as_str().to_string(),
            xp_grant: *xp_grant,
        },
        XpGainSource::RealmBreakthrough => XpGainSourceV1::RealmBreakthrough,
        XpGainSource::Mentor { mentor_char } => XpGainSourceV1::Mentor {
            mentor_char: *mentor_char,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill::components::ScrollId;
    use serde_json::Value;
    use valence::prelude::App;

    #[test]
    fn converts_xp_gain_source_action_to_wire() {
        let source = source_to_wire(&XpGainSource::Action {
            plan_id: "botany",
            action: "harvest_auto",
        });
        match source {
            XpGainSourceV1::Action { plan_id, action } => {
                assert_eq!(plan_id, "botany");
                assert_eq!(action, "harvest_auto");
            }
            other => panic!("expected action source, got {other:?}"),
        }
    }

    #[test]
    fn skill_payload_serializes_to_server_data_types() {
        let xp = ServerDataV1::new(ServerDataPayloadV1::SkillXpGain(Box::new(
            SkillXpGainPayloadV1::new(
                1001,
                SkillIdV1::Herbalism,
                5,
                XpGainSourceV1::Action {
                    plan_id: "botany".to_string(),
                    action: "harvest_auto".to_string(),
                },
            ),
        )));
        let lv = ServerDataV1::new(ServerDataPayloadV1::SkillLvUp(SkillLvUpPayloadV1::new(
            1001,
            SkillIdV1::Alchemy,
            3,
        )));
        let cap = ServerDataV1::new(ServerDataPayloadV1::SkillCapChanged(
            SkillCapChangedPayloadV1::new(1001, SkillIdV1::Forging, 7),
        ));
        let scroll = ServerDataV1::new(ServerDataPayloadV1::SkillScrollUsed(Box::new(
            SkillScrollUsedPayloadV1::new(
                1001,
                ScrollId::new("scroll:bai_cao_tu_kao_can").as_str(),
                SkillIdV1::Herbalism,
                500,
                false,
            ),
        )));

        for (payload, expected_type) in [
            (xp, "skill_xp_gain"),
            (lv, "skill_lv_up"),
            (cap, "skill_cap_changed"),
            (scroll, "skill_scroll_used"),
        ] {
            let bytes = serialize_server_data_payload(&payload).expect("serialize skill payload");
            let json: Value = serde_json::from_slice(&bytes).expect("decode json");
            assert_eq!(
                json.get("type").and_then(Value::as_str),
                Some(expected_type)
            );
        }
    }

    #[test]
    fn entity_wire_id_uses_bevy_entity_bits() {
        let mut app = App::new();
        let entity = app.world_mut().spawn_empty().id();
        assert_eq!(entity_wire_id(entity), entity.to_bits());
    }
}
