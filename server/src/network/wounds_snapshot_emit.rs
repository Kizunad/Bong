//! plan-HUD-v1 §2.1 wounds_snapshot emit。
//!
//! 监听 `Changed<Wounds>` 把伤口列表推给客户端 mini body 红点 + 检视屏伤口层。
//! server BodyPart 是 7 段（粗），client BodyPart 是 16 段（细）；按代表性映射，
//! 后续 server 细化时直接换 wire 名即可。

use std::time::{SystemTime, UNIX_EPOCH};

use valence::prelude::{Changed, Client, Entity, Or, Query, Username, With};

use crate::combat::components::{BodyPart, Lifecycle, LifecycleState, Wound, WoundKind, Wounds};
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::combat_hud::{WoundEntryV1, WoundsSnapshotV1};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};

type WoundsEmitFilter = (With<Client>, Or<(Changed<Wounds>, Changed<Lifecycle>)>);

pub fn emit_wounds_snapshot_payloads(
    mut clients: Query<
        (Entity, &mut Client, &Username, &Wounds, Option<&Lifecycle>),
        WoundsEmitFilter,
    >,
) {
    let now_ms = current_unix_millis();

    for (entity, mut client, username, wounds, lifecycle) in &mut clients {
        let wire_wounds = wounds_to_wire(wounds, lifecycle, now_ms);
        let payload = ServerDataV1::new(ServerDataPayloadV1::WoundsSnapshot(WoundsSnapshotV1 {
            wounds: wire_wounds,
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
            "[bong][network] sent {} {} payload to entity {entity:?} for `{}` ({} wounds)",
            SERVER_DATA_CHANNEL,
            payload_type,
            username.0,
            wounds.entries.len()
        );
    }
}

fn wounds_to_wire(
    wounds: &Wounds,
    lifecycle: Option<&Lifecycle>,
    now_ms: u64,
) -> Vec<WoundEntryV1> {
    if lifecycle.is_some_and(|lifecycle| lifecycle.state != LifecycleState::Alive) {
        return Vec::new();
    }
    wounds
        .entries
        .iter()
        .map(|wound| wound_to_wire(wound, now_ms))
        .collect()
}

fn wound_to_wire(wound: &Wound, now_ms: u64) -> WoundEntryV1 {
    WoundEntryV1 {
        part: body_part_wire(wound.location).to_string(),
        kind: wound_kind_wire(wound.kind).to_string(),
        severity: wound.severity,
        state: if wound.bleeding_per_sec > 0.0 {
            "bleeding".to_string()
        } else {
            "stable".to_string()
        },
        // server 暂无感染/疤痕模型；占位，后续接入。
        infection: 0.0,
        scar: false,
        updated_at_ms: now_ms,
    }
}

fn body_part_wire(part: BodyPart) -> &'static str {
    // 粗 7 段映射到 client 细 16 段中的代表位（plan-combat-v1 阶段性）。
    match part {
        BodyPart::Head => "head",
        BodyPart::Chest => "chest",
        BodyPart::Abdomen => "abdomen",
        BodyPart::ArmL => "left_upper_arm",
        BodyPart::ArmR => "right_upper_arm",
        BodyPart::LegL => "left_thigh",
        BodyPart::LegR => "right_thigh",
    }
}

fn wound_kind_wire(kind: WoundKind) -> &'static str {
    match kind {
        WoundKind::Cut => "cut",
        WoundKind::Blunt => "blunt",
        WoundKind::Pierce => "pierce",
        WoundKind::Burn => "burn",
        WoundKind::Concussion => "concussion",
    }
}

fn current_unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wounds_to_wire_clears_entries_for_near_death_lifecycle() {
        let wounds = Wounds {
            health_current: 5.0,
            health_max: 100.0,
            entries: vec![Wound {
                location: BodyPart::Chest,
                kind: WoundKind::Blunt,
                severity: 3.0,
                bleeding_per_sec: 0.0,
                created_at_tick: 12,
                inflicted_by: Some("test".to_string()),
            }],
        };
        let mut lifecycle = Lifecycle::default();
        lifecycle.enter_near_death(20);

        let wire = wounds_to_wire(&wounds, Some(&lifecycle), 123);

        assert!(
            wire.is_empty(),
            "death/near-death clients should receive an empty wounds snapshot"
        );
    }

    #[test]
    fn wounds_to_wire_keeps_entries_for_alive_lifecycle() {
        let wounds = Wounds {
            health_current: 90.0,
            health_max: 100.0,
            entries: vec![Wound {
                location: BodyPart::Chest,
                kind: WoundKind::Blunt,
                severity: 3.0,
                bleeding_per_sec: 0.0,
                created_at_tick: 12,
                inflicted_by: Some("test".to_string()),
            }],
        };
        let lifecycle = Lifecycle::default();

        let wire = wounds_to_wire(&wounds, Some(&lifecycle), 123);

        assert_eq!(wire.len(), 1);
        assert_eq!(wire[0].part, "chest");
    }
}
