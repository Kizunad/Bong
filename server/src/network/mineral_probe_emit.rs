//! 神识感知矿脉 S2C 回执发送系统
//!
//! 消费 `MineralProbeResponse` 事件，将结果序列化为 `ServerDataPayloadV1::MineralProbeResult`
//! 并通过 `SERVER_DATA_CHANNEL` 发送给触发探针的玩家（仅发给触发者，不广播）。
//!
//! 模板：`tribulation_heart_demon_offer_emit.rs`（唯一已验证可用范本）。

use valence::prelude::{Client, Entity, EventReader, Query, With};

use crate::mineral::events::{MineralProbeDenialReason, MineralProbeResponse, MineralProbeResult};
use crate::network::agent_bridge::{payload_type_label, serialize_server_data_payload};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::server_data::{MineralProbeResultV1, ServerDataPayloadV1, ServerDataV1};

/// 每帧排空 `MineralProbeResponse` 事件队列，为每个触发者发送 S2C 回执。
///
/// 仅发给 `ev.player` 对应的 entity，旁观者不收包。
pub fn emit_mineral_probe_results(
    mut clients: Query<(Entity, &mut Client), With<Client>>,
    mut responses: EventReader<MineralProbeResponse>,
) {
    for ev in responses.read() {
        let v1 = mineral_probe_result_v1(&ev.result);
        let payload = ServerDataV1::new(ServerDataPayloadV1::MineralProbeResult(v1));
        let payload_type = payload_type_label(payload.payload_type());
        let payload_bytes = match serialize_server_data_payload(&payload) {
            Ok(bytes) => bytes,
            Err(error) => {
                log_payload_build_error(payload_type, &error);
                continue;
            }
        };
        for (entity, mut client) in &mut clients {
            if entity == ev.player {
                send_server_data_payload(&mut client, payload_bytes.as_slice());
            }
        }
    }
}

/// `MineralProbeResult` → `MineralProbeResultV1` 映射。
fn mineral_probe_result_v1(result: &MineralProbeResult) -> MineralProbeResultV1 {
    match result {
        MineralProbeResult::Found {
            mineral_id,
            remaining_units,
            display_name_zh,
        } => MineralProbeResultV1 {
            kind: "found".to_string(),
            mineral_id: Some(mineral_id.to_string()),
            remaining_units: Some(*remaining_units),
            display_name_zh: Some(display_name_zh.clone()),
            denial_reason: None,
        },
        MineralProbeResult::Denied { reason } => MineralProbeResultV1 {
            kind: "denied".to_string(),
            mineral_id: None,
            remaining_units: None,
            display_name_zh: None,
            denial_reason: Some(denial_reason_snake_case(reason)),
        },
    }
}

/// `MineralProbeDenialReason` → snake_case 字串（与 plan P0 视听规格一致）。
fn denial_reason_snake_case(reason: &MineralProbeDenialReason) -> String {
    match reason {
        MineralProbeDenialReason::RealmTooLow => "realm_too_low",
        MineralProbeDenialReason::OutOfRange => "out_of_range",
        MineralProbeDenialReason::NotMineralOre => "not_mineral_ore",
        MineralProbeDenialReason::StaleOreIndex => "stale_ore_index",
        MineralProbeDenialReason::MineralNotRegistered => "mineral_not_registered",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::mineral::events::MineralProbeDenialReason;
    use crate::network::agent_bridge::SERVER_DATA_CHANNEL;
    use valence::prelude::{App, Update};
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    fn spawn_mock_client(app: &mut App, name: &str) -> (Entity, MockClientHelper) {
        let (bundle, helper) = create_mock_client(name);
        let entity = app.world_mut().spawn(bundle).id();
        (entity, helper)
    }

    fn flush_all_client_packets(app: &mut App) {
        let world = app.world_mut();
        let mut query = world.query::<&mut Client>();
        for mut client in query.iter_mut(world) {
            client
                .flush_packets()
                .expect("mock client packets should flush");
        }
    }

    fn collect_mineral_probe_results(helper: &mut MockClientHelper) -> Vec<MineralProbeResultV1> {
        let mut payloads = Vec::new();
        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };
            if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                continue;
            }
            let payload: ServerDataV1 = serde_json::from_slice(packet.data.0 .0)
                .expect("server data payload should decode");
            if let ServerDataPayloadV1::MineralProbeResult(data) = payload.payload {
                payloads.push(data);
            }
        }
        payloads
    }

    // ─────────────────────────────────────────────────────────────────
    // P0 测试矩阵：≥6 条
    // ─────────────────────────────────────────────────────────────────

    /// found 回执：payload 字段对拍（mineral_id / remaining_units / display_name_zh 都在）。
    #[test]
    fn mineral_probe_found_payload_fields() {
        let mut app = App::new();
        app.add_event::<MineralProbeResponse>();
        app.add_systems(Update, emit_mineral_probe_results);

        let (player, mut player_helper) = spawn_mock_client(&mut app, "Kiz");

        use crate::mineral::types::MineralId;
        use valence::prelude::BlockPos;

        app.world_mut().send_event(MineralProbeResponse {
            player,
            position: BlockPos::new(0, 64, 0),
            result: MineralProbeResult::Found {
                mineral_id: MineralId::CuTie,
                remaining_units: 42,
                display_name_zh: "赤铜矿脉".to_string(),
            },
        });

        app.update();
        flush_all_client_packets(&mut app);

        let results = collect_mineral_probe_results(&mut player_helper);
        assert_eq!(results.len(), 1, "期望收到 1 条 found 回执");
        let r = &results[0];
        assert_eq!(r.kind, "found");
        assert_eq!(r.mineral_id.as_deref(), Some("cu_tie"));
        assert_eq!(r.remaining_units, Some(42));
        assert_eq!(r.display_name_zh.as_deref(), Some("赤铜矿脉"));
        assert!(
            r.denial_reason.is_none(),
            "found 时 denial_reason 应为 None"
        );
    }

    /// 仅发给触发玩家，旁观者收不到包（镜像 heart_demon `*_is_sent_only_to_tribulator`）。
    #[test]
    fn mineral_probe_result_is_sent_only_to_probe_player() {
        let mut app = App::new();
        app.add_event::<MineralProbeResponse>();
        app.add_systems(Update, emit_mineral_probe_results);

        let (player, mut player_helper) = spawn_mock_client(&mut app, "Kiz");
        let (_spectator, mut spectator_helper) = spawn_mock_client(&mut app, "Spectator");

        use crate::mineral::types::MineralId;
        use valence::prelude::BlockPos;

        app.world_mut().send_event(MineralProbeResponse {
            player,
            position: BlockPos::new(0, 64, 0),
            result: MineralProbeResult::Found {
                mineral_id: MineralId::CuTie,
                remaining_units: 10,
                display_name_zh: "赤铜矿脉".to_string(),
            },
        });

        app.update();
        flush_all_client_packets(&mut app);

        let player_results = collect_mineral_probe_results(&mut player_helper);
        let spectator_results = collect_mineral_probe_results(&mut spectator_helper);

        assert_eq!(player_results.len(), 1, "触发玩家应收到 1 包");
        assert_eq!(spectator_results.len(), 0, "旁观者不应收到任何包");
    }

    /// denied / realm_too_low：字段映射正确，kind=denied，denial_reason=realm_too_low。
    #[test]
    fn mineral_probe_denied_realm_too_low() {
        let mut app = App::new();
        app.add_event::<MineralProbeResponse>();
        app.add_systems(Update, emit_mineral_probe_results);

        let (player, mut player_helper) = spawn_mock_client(&mut app, "Kiz");

        use valence::prelude::BlockPos;
        app.world_mut().send_event(MineralProbeResponse {
            player,
            position: BlockPos::new(0, 64, 0),
            result: MineralProbeResult::Denied {
                reason: MineralProbeDenialReason::RealmTooLow,
            },
        });

        app.update();
        flush_all_client_packets(&mut app);

        let results = collect_mineral_probe_results(&mut player_helper);
        assert_eq!(results.len(), 1);
        let r = &results[0];
        assert_eq!(r.kind, "denied");
        assert_eq!(r.denial_reason.as_deref(), Some("realm_too_low"));
        assert!(r.mineral_id.is_none());
        assert!(r.remaining_units.is_none());
        assert!(r.display_name_zh.is_none());
    }

    /// denied / out_of_range：denial_reason 正确。
    #[test]
    fn mineral_probe_denied_out_of_range() {
        let r = mineral_probe_result_v1(&MineralProbeResult::Denied {
            reason: MineralProbeDenialReason::OutOfRange,
        });
        assert_eq!(r.kind, "denied");
        assert_eq!(r.denial_reason.as_deref(), Some("out_of_range"));
    }

    /// denied / not_mineral_ore：denial_reason 正确。
    #[test]
    fn mineral_probe_denied_not_mineral_ore() {
        let r = mineral_probe_result_v1(&MineralProbeResult::Denied {
            reason: MineralProbeDenialReason::NotMineralOre,
        });
        assert_eq!(r.denial_reason.as_deref(), Some("not_mineral_ore"));
    }

    /// denied / stale_ore_index：denial_reason 正确。
    #[test]
    fn mineral_probe_denied_stale_ore_index() {
        let r = mineral_probe_result_v1(&MineralProbeResult::Denied {
            reason: MineralProbeDenialReason::StaleOreIndex,
        });
        assert_eq!(r.denial_reason.as_deref(), Some("stale_ore_index"));
    }

    /// denied / mineral_not_registered：denial_reason 正确。
    #[test]
    fn mineral_probe_denied_mineral_not_registered() {
        let r = mineral_probe_result_v1(&MineralProbeResult::Denied {
            reason: MineralProbeDenialReason::MineralNotRegistered,
        });
        assert_eq!(r.denial_reason.as_deref(), Some("mineral_not_registered"));
    }

    /// proto/wire round-trip：`MineralProbeResultV1` 序列化后反序列化字段不变。
    #[test]
    fn mineral_probe_result_v1_wire_roundtrip() {
        let v1 = MineralProbeResultV1 {
            kind: "found".to_string(),
            mineral_id: Some("yu_sui".to_string()),
            remaining_units: Some(7),
            display_name_zh: Some("玉髓矿脉".to_string()),
            denial_reason: None,
        };
        let payload = ServerDataV1::new(ServerDataPayloadV1::MineralProbeResult(v1.clone()));
        let bytes = serde_json::to_vec(&payload).expect("序列化应成功");
        let decoded: ServerDataV1 = serde_json::from_slice(&bytes).expect("反序列化应成功");

        if let ServerDataPayloadV1::MineralProbeResult(decoded_v1) = decoded.payload {
            assert_eq!(decoded_v1.kind, "found");
            assert_eq!(decoded_v1.mineral_id.as_deref(), Some("yu_sui"));
            assert_eq!(decoded_v1.remaining_units, Some(7));
            assert_eq!(decoded_v1.display_name_zh.as_deref(), Some("玉髓矿脉"));
            assert!(decoded_v1.denial_reason.is_none());
        } else {
            panic!("期望 MineralProbeResult 变体，实际 {:?}", decoded.payload);
        }
    }

    /// payload_type_label pin：MineralProbeResult → "mineral_probe_result"（防 wire/label 不一致）。
    #[test]
    fn mineral_probe_result_payload_type_label_pin() {
        use crate::network::agent_bridge::payload_type_label;
        use crate::schema::server_data::ServerDataType;

        let label = payload_type_label(ServerDataType::MineralProbeResult);
        assert_eq!(
            label, "mineral_probe_result",
            "payload_type_label 应返回 mineral_probe_result，实际 {label}"
        );
    }

    /// 多条事件：同一帧两个不同玩家的回执各自独立送达。
    #[test]
    fn mineral_probe_two_players_independent_results() {
        let mut app = App::new();
        app.add_event::<MineralProbeResponse>();
        app.add_systems(Update, emit_mineral_probe_results);

        let (player_a, mut helper_a) = spawn_mock_client(&mut app, "Alice");
        let (player_b, mut helper_b) = spawn_mock_client(&mut app, "Bob");

        use crate::mineral::types::MineralId;
        use valence::prelude::BlockPos;

        app.world_mut().send_event(MineralProbeResponse {
            player: player_a,
            position: BlockPos::new(0, 64, 0),
            result: MineralProbeResult::Found {
                mineral_id: MineralId::CuTie,
                remaining_units: 5,
                display_name_zh: "赤铜矿脉".to_string(),
            },
        });
        app.world_mut().send_event(MineralProbeResponse {
            player: player_b,
            position: BlockPos::new(10, 64, 0),
            result: MineralProbeResult::Denied {
                reason: MineralProbeDenialReason::OutOfRange,
            },
        });

        app.update();
        flush_all_client_packets(&mut app);

        let results_a = collect_mineral_probe_results(&mut helper_a);
        let results_b = collect_mineral_probe_results(&mut helper_b);

        assert_eq!(results_a.len(), 1, "Alice 应收到 1 条");
        assert_eq!(results_a[0].kind, "found");
        assert_eq!(results_b.len(), 1, "Bob 应收到 1 条");
        assert_eq!(results_b[0].kind, "denied");
        assert_eq!(results_b[0].denial_reason.as_deref(), Some("out_of_range"));
    }
}
