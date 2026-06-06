//! 神识感知保鲜 S2C 回执发送系统
//!
//! 消费 `FreshnessProbeResponse` 事件：
//! - `ProbeResult::Precise` → 构建 `FreshnessUpdateV1`（复用既有 `freshness_update` 类型串）
//!   并用 inventory query 反查 `freshness.profile`；
//! - `ProbeResult::Denied` → 不发 freshness_update（避免污染 FreshnessStore），
//!   改发 `EventAlert` 灰字提示（info 级别，3.5 s toast）。
//!
//! 模板：`mineral_probe_emit.rs` / `tribulation_heart_demon_offer_emit.rs`（已验证范本）。

use valence::prelude::{Client, Entity, EventReader, Query, With};

use crate::inventory::{inventory_item_by_instance_borrow, PlayerInventory};
use crate::network::agent_bridge::{payload_type_label, serialize_server_data_payload};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::common::EventKind;
use crate::schema::processing::FreshnessUpdateV1;
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
use crate::shelflife::probe::{FreshnessProbeResponse, ProbeDenialReason, ProbeResult};

/// 每帧排空 `FreshnessProbeResponse` 事件队列，为每个触发者发送 S2C 回执。
///
/// - Precise → 发 `freshness_update`（复用既有 handler，不新增 client 处理逻辑）
/// - Denied → 发 `event_alert`（info 级别，不污染 FreshnessStore）
///
/// 仅发给 `ev.player` 对应的 entity，旁观者不收包。
pub fn emit_freshness_probe_results(
    mut clients: Query<(Entity, &mut Client), With<Client>>,
    inventories: Query<&PlayerInventory>,
    mut responses: EventReader<FreshnessProbeResponse>,
) {
    for ev in responses.read() {
        let payload = match &ev.result {
            ProbeResult::Precise {
                current_qi,
                initial_qi,
                ..
            } => {
                // 从触发者 inventory 反查 profile_name（Precise 结果不携带 profile，
                // 必须二次查 inventory NBT；inventory 查询安全因为 probe 刚刚用相同 instance_id 通过）。
                let profile_name = inventories
                    .get(ev.player)
                    .ok()
                    .and_then(|inv| inventory_item_by_instance_borrow(inv, ev.instance_id))
                    .and_then(|item| item.freshness.as_ref())
                    .map(|f| f.profile.0.clone())
                    .unwrap_or_default();

                let freshness = if *initial_qi > 0.0 {
                    (current_qi / initial_qi).clamp(0.0, 1.0)
                } else {
                    0.0
                };

                ServerDataV1::new(ServerDataPayloadV1::FreshnessUpdate(FreshnessUpdateV1 {
                    item_uuid: ev.instance_id.to_string(),
                    freshness,
                    profile_name,
                }))
            }
            ProbeResult::Denied { reason } => {
                let message = freshness_denied_message(reason);
                ServerDataV1::new(ServerDataPayloadV1::EventAlert {
                    event: EventKind::Generic,
                    message: message.to_string(),
                    zone: None,
                    duration_ticks: Some(70), // ~3.5 s @ 20 tps
                })
            }
        };

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

/// `ProbeDenialReason` → 界面灰字提示文案。
fn freshness_denied_message(reason: &ProbeDenialReason) -> &'static str {
    match reason {
        ProbeDenialReason::RealmTooLow => "神识未及，凝脉方可感知保鲜",
        ProbeDenialReason::ItemNotFound => "此物已离手，无法感知",
        ProbeDenialReason::NoFreshness => "此物无时气流转，无从感知",
        ProbeDenialReason::ProfileNotRegistered => "保鲜谱系模糊，难以辨别",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::inventory::{
        ContainerState, ItemInstance, ItemRarity, PlacedItemState, PlayerInventory,
    };
    use crate::network::agent_bridge::SERVER_DATA_CHANNEL;
    use crate::schema::server_data::ServerDataPayloadV1;
    use crate::shelflife::probe::ProbeResult;
    use crate::shelflife::{DecayFormula, DecayProfile, DecayProfileId, Freshness};
    use std::collections::HashMap;
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

    fn sample_decay_profile() -> DecayProfile {
        DecayProfile::Decay {
            id: DecayProfileId::new("test_decay"),
            formula: DecayFormula::Exponential {
                half_life_ticks: 1000,
            },
            floor_qi: 0.0,
        }
    }

    fn make_item_with_freshness(instance_id: u64, profile: &DecayProfile) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: "ling_cao_test".to_string(),
            display_name: "灵草".to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.1,
            rarity: ItemRarity::Common,
            description: "test".to_string(),
            stack_count: 1,
            spirit_quality: 0.8,
            durability: 1.0,
            freshness: Some(Freshness::new(0, 100.0, profile)),
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        }
    }

    fn make_inventory_with_item(item: ItemInstance) -> PlayerInventory {
        let container = ContainerState {
            id: "main_pack".to_string(),
            name: "main_pack".to_string(),
            rows: 4,
            cols: 4,
            items: vec![PlacedItemState {
                row: 0,
                col: 0,
                instance: item,
            }],
        };
        PlayerInventory {
            revision: crate::inventory::InventoryRevision(1),
            containers: vec![container],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 100.0,
        }
    }

    /// 一次性从 helper 收包，分别返回 (freshness_updates, event_alert_messages)。
    /// `collect_received()` 消耗一次即空，必须在同一次调用中分拣两种 payload。
    fn collect_payloads(helper: &mut MockClientHelper) -> (Vec<FreshnessUpdateV1>, Vec<String>) {
        let mut updates = Vec::new();
        let mut alerts = Vec::new();
        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };
            if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                continue;
            }
            let payload: ServerDataV1 = serde_json::from_slice(packet.data.0 .0)
                .expect("server data payload should decode");
            match payload.payload {
                ServerDataPayloadV1::FreshnessUpdate(data) => updates.push(data),
                ServerDataPayloadV1::EventAlert { message, .. } => alerts.push(message),
                _ => {}
            }
        }
        (updates, alerts)
    }

    fn setup_app() -> App {
        let mut app = App::new();
        app.add_event::<FreshnessProbeResponse>();
        app.add_systems(Update, emit_freshness_probe_results);
        app
    }

    // ─────────────────────────────────────────────────────────────────
    // P1 测试矩阵：≥7 条
    // ─────────────────────────────────────────────────────────────────

    /// Precise → FreshnessUpdateV1 字段映射（Decay track）：
    /// freshness = current_qi / initial_qi；item_uuid = instance_id；profile_name 反查正确。
    #[test]
    fn precise_decay_freshness_update_fields_correct() {
        let mut app = setup_app();
        let profile = sample_decay_profile();
        let (player, mut helper) = spawn_mock_client(&mut app, "Kiz");
        // 把 inventory 挂到 player entity 上
        app.world_mut()
            .entity_mut(player)
            .insert(make_inventory_with_item(make_item_with_freshness(
                42, &profile,
            )));

        app.world_mut().send_event(FreshnessProbeResponse {
            player,
            instance_id: 42,
            result: ProbeResult::Precise {
                track: crate::shelflife::DecayTrack::Decay,
                current_qi: 75.0,
                initial_qi: 100.0,
                track_state: crate::shelflife::types::TrackState::Declining,
                predicted_event_ticks: None,
            },
        });

        app.update();
        flush_all_client_packets(&mut app);

        let (updates, _) = collect_payloads(&mut helper);
        assert_eq!(updates.len(), 1, "期望收到 1 条 freshness_update");
        let u = &updates[0];
        assert_eq!(u.item_uuid, "42", "item_uuid 应为 instance_id.to_string()");
        assert!(
            (u.freshness - 0.75).abs() < 1e-5,
            "freshness=0.75 时 current_qi/initial_qi=75/100，实际 {}",
            u.freshness
        );
        assert_eq!(
            u.profile_name, "test_decay",
            "profile_name 应从 inventory 反查"
        );
    }

    /// Precise → Spoil track：同样映射正确。
    #[test]
    fn precise_spoil_freshness_update_fields_correct() {
        let mut app = setup_app();
        let profile = DecayProfile::Spoil {
            id: DecayProfileId::new("test_spoil"),
            formula: DecayFormula::Exponential {
                half_life_ticks: 500,
            },
            spoil_threshold: 10.0,
        };
        let (player, mut helper) = spawn_mock_client(&mut app, "Spoil");
        app.world_mut()
            .entity_mut(player)
            .insert(make_inventory_with_item(make_item_with_freshness(
                99, &profile,
            )));

        app.world_mut().send_event(FreshnessProbeResponse {
            player,
            instance_id: 99,
            result: ProbeResult::Precise {
                track: crate::shelflife::DecayTrack::Spoil,
                current_qi: 50.0,
                initial_qi: 100.0,
                track_state: crate::shelflife::types::TrackState::Declining,
                predicted_event_ticks: None,
            },
        });

        app.update();
        flush_all_client_packets(&mut app);

        let (updates, _) = collect_payloads(&mut helper);
        assert_eq!(updates.len(), 1);
        let u = &updates[0];
        assert_eq!(u.item_uuid, "99");
        assert!(
            (u.freshness - 0.5).abs() < 1e-5,
            "freshness=0.5，实际 {}",
            u.freshness
        );
        assert_eq!(u.profile_name, "test_spoil");
    }

    /// Precise → Age track：同样映射正确。
    #[test]
    fn precise_age_freshness_update_fields_correct() {
        let mut app = setup_app();
        let profile = DecayProfile::Age {
            id: DecayProfileId::new("test_age"),
            peak_at_ticks: 1000,
            peak_bonus: 0.5,
            peak_window_ratio: 0.1,
            post_peak_half_life_ticks: 500,
            post_peak_spoil_threshold: 30.0,
            post_peak_spoil_profile: DecayProfileId::new("test_age_spoil"),
        };
        let (player, mut helper) = spawn_mock_client(&mut app, "Age");
        app.world_mut()
            .entity_mut(player)
            .insert(make_inventory_with_item(make_item_with_freshness(
                7, &profile,
            )));

        app.world_mut().send_event(FreshnessProbeResponse {
            player,
            instance_id: 7,
            result: ProbeResult::Precise {
                track: crate::shelflife::DecayTrack::Age,
                current_qi: 80.0,
                initial_qi: 80.0,
                track_state: crate::shelflife::types::TrackState::Fresh,
                predicted_event_ticks: None,
            },
        });

        app.update();
        flush_all_client_packets(&mut app);

        let (updates, _) = collect_payloads(&mut helper);
        assert_eq!(updates.len(), 1);
        let u = &updates[0];
        assert_eq!(u.item_uuid, "7");
        assert!(
            (u.freshness - 1.0).abs() < 1e-5,
            "当前=初始时 freshness=1.0，实际 {}",
            u.freshness
        );
        assert_eq!(u.profile_name, "test_age");
    }

    /// Denied / realm_too_low → 不发 freshness_update，改发 event_alert。
    #[test]
    fn denied_realm_too_low_sends_event_alert_not_freshness_update() {
        let mut app = setup_app();
        let (player, mut helper) = spawn_mock_client(&mut app, "Low");

        app.world_mut().send_event(FreshnessProbeResponse {
            player,
            instance_id: 1,
            result: ProbeResult::Denied {
                reason: ProbeDenialReason::RealmTooLow,
            },
        });

        app.update();
        flush_all_client_packets(&mut app);

        let (updates, alerts) = collect_payloads(&mut helper);
        assert!(updates.is_empty(), "Denied 不应发 freshness_update");
        assert_eq!(
            alerts.len(),
            1,
            "Denied 应发 1 条 event_alert，实际 {:?}",
            alerts
        );
        assert!(
            alerts[0].contains("凝脉"),
            "realm_too_low 文案应含「凝脉」，实际：{}",
            alerts[0]
        );
    }

    /// Denied / item_not_found → 不发 freshness_update，改发 event_alert。
    #[test]
    fn denied_item_not_found_sends_event_alert() {
        let mut app = setup_app();
        let (player, mut helper) = spawn_mock_client(&mut app, "Ghost");

        app.world_mut().send_event(FreshnessProbeResponse {
            player,
            instance_id: 999,
            result: ProbeResult::Denied {
                reason: ProbeDenialReason::ItemNotFound,
            },
        });

        app.update();
        flush_all_client_packets(&mut app);

        let (updates, alerts) = collect_payloads(&mut helper);
        assert!(updates.is_empty(), "Denied 不应发 freshness_update");
        assert_eq!(alerts.len(), 1);
    }

    /// Denied / no_freshness → event_alert。
    #[test]
    fn denied_no_freshness_sends_event_alert() {
        let mut app = setup_app();
        let (player, mut helper) = spawn_mock_client(&mut app, "NoFresh");

        app.world_mut().send_event(FreshnessProbeResponse {
            player,
            instance_id: 2,
            result: ProbeResult::Denied {
                reason: ProbeDenialReason::NoFreshness,
            },
        });

        app.update();
        flush_all_client_packets(&mut app);

        let (updates, alerts) = collect_payloads(&mut helper);
        assert!(updates.is_empty());
        assert_eq!(alerts.len(), 1);
    }

    /// Denied / profile_not_registered → event_alert。
    #[test]
    fn denied_profile_not_registered_sends_event_alert() {
        let mut app = setup_app();
        let (player, mut helper) = spawn_mock_client(&mut app, "NoProfile");

        app.world_mut().send_event(FreshnessProbeResponse {
            player,
            instance_id: 3,
            result: ProbeResult::Denied {
                reason: ProbeDenialReason::ProfileNotRegistered,
            },
        });

        app.update();
        flush_all_client_packets(&mut app);

        let (updates, alerts) = collect_payloads(&mut helper);
        assert!(updates.is_empty());
        assert_eq!(alerts.len(), 1);
    }

    /// 仅发给触发玩家，旁观者收不到包。
    #[test]
    fn freshness_update_sent_only_to_probe_player() {
        let mut app = setup_app();
        let profile = sample_decay_profile();

        let (player, mut player_helper) = spawn_mock_client(&mut app, "Player");
        let (_spectator, mut spectator_helper) = spawn_mock_client(&mut app, "Spectator");

        app.world_mut()
            .entity_mut(player)
            .insert(make_inventory_with_item(make_item_with_freshness(
                10, &profile,
            )));

        app.world_mut().send_event(FreshnessProbeResponse {
            player,
            instance_id: 10,
            result: ProbeResult::Precise {
                track: crate::shelflife::DecayTrack::Decay,
                current_qi: 60.0,
                initial_qi: 100.0,
                track_state: crate::shelflife::types::TrackState::Declining,
                predicted_event_ticks: None,
            },
        });

        app.update();
        flush_all_client_packets(&mut app);

        let (player_updates, _) = collect_payloads(&mut player_helper);
        let (spectator_updates, _) = collect_payloads(&mut spectator_helper);

        assert_eq!(player_updates.len(), 1, "触发玩家应收到 1 包");
        assert_eq!(spectator_updates.len(), 0, "旁观者不应收到任何包");
    }

    /// freshness=current/initial clamp [0,1]：initial_qi=0 时不 panic，返回 0.0。
    #[test]
    fn freshness_zero_initial_qi_clamps_to_zero_no_panic() {
        let r = ProbeResult::Precise {
            track: crate::shelflife::DecayTrack::Decay,
            current_qi: 0.0,
            initial_qi: 0.0, // edge: division by zero 防护
            track_state: crate::shelflife::types::TrackState::Dead,
            predicted_event_ticks: None,
        };
        // 计算逻辑单元测试（不走 Bevy app）
        let freshness = match &r {
            ProbeResult::Precise {
                current_qi,
                initial_qi,
                ..
            } => {
                if *initial_qi > 0.0 {
                    (current_qi / initial_qi).clamp(0.0, 1.0)
                } else {
                    0.0
                }
            }
            _ => unreachable!(),
        };
        assert_eq!(
            freshness, 0.0,
            "initial_qi=0 时 freshness 应为 0.0 不 panic"
        );
    }

    /// 类型串 pin：FreshnessUpdate → "freshness_update"（防 wire/label 不一致）。
    #[test]
    fn freshness_update_payload_type_label_pin() {
        use crate::network::agent_bridge::payload_type_label;
        use crate::schema::server_data::ServerDataType;

        let label = payload_type_label(ServerDataType::FreshnessUpdate);
        assert_eq!(
            label, "freshness_update",
            "payload_type_label 应返回 freshness_update，实际 {label}"
        );
    }
}
