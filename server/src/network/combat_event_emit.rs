use valence::prelude::{Client, Entity, EventReader, Query, Username};
use valence::protocol::encode::WritePacket;
use valence::protocol::packets::play::DamageTiltS2c;
use valence::protocol::VarInt;

use crate::combat::events::{CombatEvent, DefenseKind};
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::server_data::{
    CombatEventFloaterEntryV1, CombatEventFloaterV1, ServerDataPayloadV1, ServerDataV1,
};

pub fn emit_combat_event_to_client(
    mut combat_reader: EventReader<CombatEvent>,
    mut clients: Query<(Entity, &Username, &mut Client)>,
) {
    for ev in combat_reader.read() {
        let amount = ev.damage + ev.physical_damage;
        if amount <= 0.0 {
            continue;
        }

        let kind = wire_kind(ev);
        let text = format_amount(amount);

        let entry = CombatEventFloaterEntryV1 {
            kind: kind.clone(),
            amount,
            text,
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };

        send_floater(&mut clients, ev.target, &entry);

        if ev.attacker != ev.target {
            send_floater(&mut clients, ev.attacker, &entry);
        }

        send_damage_tilt(&mut clients, ev.target);
    }
}

fn wire_kind(ev: &CombatEvent) -> String {
    if ev.defense_kind == Some(DefenseKind::JieMai) {
        return "block".to_string();
    }
    "hit".to_string()
}

fn format_amount(amount: f32) -> String {
    let rounded = amount.round() as i64;
    if (amount - rounded as f32).abs() < 0.1 {
        rounded.to_string()
    } else {
        format!("{:.1}", amount)
    }
}

fn send_floater(
    clients: &mut Query<(Entity, &Username, &mut Client)>,
    entity: Entity,
    entry: &CombatEventFloaterEntryV1,
) {
    let Ok((_ent, username, mut client)) = clients.get_mut(entity) else {
        return;
    };

    let payload = ServerDataV1::new(ServerDataPayloadV1::CombatEventFloater(
        CombatEventFloaterV1 {
            events: vec![entry.clone()],
        },
    ));
    let payload_type = payload_type_label(payload.payload_type());
    let payload_bytes = match serialize_server_data_payload(&payload) {
        Ok(bytes) => bytes,
        Err(error) => {
            log_payload_build_error(payload_type, &error);
            return;
        }
    };

    send_server_data_payload(&mut client, payload_bytes.as_slice());
    tracing::debug!(
        "[bong][network] sent {} {} payload to `{}`",
        SERVER_DATA_CHANNEL,
        payload_type,
        username.0
    );
}

fn send_damage_tilt(clients: &mut Query<(Entity, &Username, &mut Client)>, target: Entity) {
    let Ok((_ent, _username, mut client)) = clients.get_mut(target) else {
        return;
    };
    client.write_packet(&DamageTiltS2c {
        entity_id: VarInt(0),
        yaw: 0.0,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::components::{BodyPart, WoundKind};
    use crate::combat::events::{AttackSource, CombatEvent};

    #[test]
    fn wire_kind_hit_for_normal_attack() {
        let ev = make_event(None);
        assert_eq!(wire_kind(&ev), "hit");
    }

    #[test]
    fn wire_kind_block_for_jiemai() {
        let ev = make_event(Some(DefenseKind::JieMai));
        assert_eq!(wire_kind(&ev), "block");
    }

    #[test]
    fn format_amount_integer() {
        assert_eq!(format_amount(12.0), "12");
        assert_eq!(format_amount(12.04), "12");
    }

    #[test]
    fn format_amount_decimal() {
        assert_eq!(format_amount(12.5), "12.5");
    }

    #[test]
    fn floater_amount_uses_physical_damage_for_mundane_hits() {
        let ev = CombatEvent {
            physical_damage: 7.0,
            damage: 0.0,
            ..make_event(None)
        };
        let amount = ev.damage + ev.physical_damage;

        assert_eq!(amount, 7.0);
        assert_eq!(format_amount(amount), "7");
    }

    #[test]
    fn combat_event_floater_serializes_as_combat_event_type() {
        let payload = ServerDataV1::new(ServerDataPayloadV1::CombatEventFloater(
            CombatEventFloaterV1 {
                events: vec![CombatEventFloaterEntryV1 {
                    kind: "hit".to_string(),
                    amount: 12.0,
                    text: "12".to_string(),
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                }],
            },
        ));
        let bytes = payload.to_json_bytes_checked().unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["type"], "combat_event");
        assert!(json["events"].is_array());
        assert_eq!(json["events"][0]["kind"], "hit");
        assert_eq!(json["events"][0]["amount"], 12.0);
    }

    fn make_event(defense: Option<DefenseKind>) -> CombatEvent {
        CombatEvent {
            attacker: Entity::PLACEHOLDER,
            target: Entity::PLACEHOLDER,
            resolved_at_tick: 0,
            body_part: BodyPart::Chest,
            wound_kind: WoundKind::Cut,
            source: AttackSource::Melee,
            debug_command: false,
            physical_damage: 0.0,
            damage: 10.0,
            contam_delta: 0.0,
            description: String::new(),
            defense_kind: defense,
            defense_effectiveness: None,
            defense_contam_reduced: None,
            defense_wound_severity: None,
        }
    }
}
