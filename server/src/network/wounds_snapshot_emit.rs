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
    use valence::prelude::{App, Update};
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    fn sample_wounds() -> Wounds {
        Wounds {
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
        }
    }

    fn flush_client_packets(app: &mut App) {
        let world = app.world_mut();
        let mut query = world.query::<&mut Client>();
        for mut client in query.iter_mut(world) {
            client
                .flush_packets()
                .expect("mock client packets should flush");
        }
    }

    fn collect_wounds_snapshot_payloads(helper: &mut MockClientHelper) -> Vec<WoundsSnapshotV1> {
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
                    ServerDataPayloadV1::WoundsSnapshot(snapshot) => Some(snapshot),
                    _ => None,
                }
            })
            .collect()
    }

    #[test]
    fn wounds_to_wire_clears_entries_for_near_death_lifecycle() {
        let wounds = sample_wounds();
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
        let wounds = sample_wounds();
        let lifecycle = Lifecycle::default();

        let wire = wounds_to_wire(&wounds, Some(&lifecycle), 123);

        assert_eq!(wire.len(), 1);
        assert_eq!(wire[0].part, "chest");
    }

    #[test]
    fn emits_empty_snapshot_when_lifecycle_changes_to_near_death() {
        assert_lifecycle_change_emits_empty_snapshot(
            "NearDeath",
            LifecycleState::NearDeath,
            |lifecycle| lifecycle.enter_near_death(20),
        );
    }

    #[test]
    fn emits_empty_snapshot_when_lifecycle_changes_to_awaiting_revival() {
        assert_lifecycle_change_emits_empty_snapshot(
            "AwaitingRevival",
            LifecycleState::AwaitingRevival,
            |lifecycle| {
                lifecycle.enter_near_death(20);
                assert_eq!(
                    lifecycle.state,
                    LifecycleState::NearDeath,
                    "test setup expected enter_near_death to move lifecycle into NearDeath"
                );
                lifecycle.await_revival_decision(
                    crate::combat::components::RevivalDecision::Fortune { chance: 1.0 },
                    40,
                );
            },
        );
    }

    #[test]
    fn emits_empty_snapshot_when_lifecycle_changes_to_terminated() {
        assert_lifecycle_change_emits_empty_snapshot(
            "Terminated",
            LifecycleState::Terminated,
            |lifecycle| lifecycle.terminate(20),
        );
    }

    fn assert_lifecycle_change_emits_empty_snapshot(
        state_name: &str,
        expected_state: LifecycleState,
        transition: impl FnOnce(&mut Lifecycle),
    ) {
        let mut app = App::new();
        app.add_systems(Update, emit_wounds_snapshot_payloads);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((client_bundle, sample_wounds(), Lifecycle::default()))
            .id();

        app.update();
        flush_client_packets(&mut app);
        let initial = collect_wounds_snapshot_payloads(&mut helper);
        assert_eq!(
            initial.len(),
            1,
            "expected initial Changed<Wounds>/Changed<Lifecycle> snapshot before testing lifecycle-only transition"
        );
        assert_eq!(
            initial[0].wounds.len(),
            1,
            "expected initial alive client snapshot to include existing wounds"
        );

        {
            let mut entity_mut = app.world_mut().entity_mut(entity);
            let mut lifecycle = entity_mut.get_mut::<Lifecycle>().unwrap();
            transition(&mut lifecycle);
            assert_eq!(
                lifecycle.state, expected_state,
                "test setup expected lifecycle transition into {state_name}"
            );
        }
        app.update();
        flush_client_packets(&mut app);

        let changed = collect_wounds_snapshot_payloads(&mut helper);
        assert_eq!(
            changed.len(),
            1,
            "expected Changed<Lifecycle> to trigger one wounds snapshot for {state_name}"
        );
        assert!(
            changed[0].wounds.is_empty(),
            "expected Changed<Lifecycle> {state_name} snapshot to clear client wounds, actual: {:?}",
            changed[0].wounds
        );
    }
}
