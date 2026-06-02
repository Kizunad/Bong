//! plan-dandao-runtime-wiring-v1 P1 — `bong:mutation_visual` S2C CustomPayload emitter.
//!
//! 读取 [`MutationAdvanceEvent`] 事件，从 `MutationState` + `DandaoStyle.cumulative_toxin`
//! 组装 [`MutationVisualSyncPayload`]，通过 `send_custom_payload` 推送给对应玩家 client。
//!
//! # 大小写契约
//! - `kind`：CamelCase（如 "GoldenIris"），client `MutationKind.fromServerName` 依赖此格式。
//! - `body_slot`：PascalCase（如 "Head"），client `translateBodySlot` 会 `toUpperCase()`。
//! - `cumulative_toxin`：f64，client `MutationPayloadHandler` 直接读取此 JSON key。

use valence::prelude::{ident, Client, EventReader, Query, With};

use crate::dandao::components::DandaoStyle;
use crate::dandao::mutation::{MutationAdvanceEvent, MutationState};
use crate::dandao::visual_sync::MutationVisualSyncPayload;
use crate::schema::common::MAX_PAYLOAD_BYTES;

/// 监听 [`MutationAdvanceEvent`]，对触发变异推进的玩家 entity 发送
/// `bong:mutation_visual` CustomPayload。
///
/// 系统签名：event reader + per-player 查询（`MutationState` / `DandaoStyle`）+ client 写。
pub fn mutation_visual_emit_system(
    mut events: EventReader<MutationAdvanceEvent>,
    player_query: Query<(&MutationState, &DandaoStyle)>,
    mut clients: Query<&mut Client, With<Client>>,
) {
    for event in events.read() {
        let entity = event.entity;

        let Ok((state, style)) = player_query.get(entity) else {
            // entity 不再存活或尚未持有 MutationState（理论上不应发生，因为
            // mutation_advance_system 会先 insert，但防御性跳过）
            tracing::warn!(
                "[mutation-visual] entity {:?} fired MutationAdvanceEvent but lacks MutationState/DandaoStyle",
                entity
            );
            continue;
        };

        let entity_id = format!("entity:{}", entity.to_bits());
        let payload =
            MutationVisualSyncPayload::from_state(&entity_id, state, style.cumulative_toxin);

        let bytes = match serde_json::to_vec(&payload) {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!("[mutation-visual] serialize failed for {entity_id}: {e}");
                continue;
            }
        };

        if bytes.len() > MAX_PAYLOAD_BYTES {
            tracing::warn!(
                "[mutation-visual] payload oversize for {entity_id}: {} > {}",
                bytes.len(),
                MAX_PAYLOAD_BYTES
            );
            continue;
        }

        let Ok(mut client) = clients.get_mut(entity) else {
            // entity 当前不是在线玩家（离线 / NPC），跳过
            continue;
        };

        client.send_custom_payload(ident!("bong:mutation_visual"), &bytes);

        tracing::debug!(
            "[mutation-visual] sent bong:mutation_visual to {entity_id}: stage={} toxin={:.1}",
            payload.stage,
            payload.cumulative_toxin
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use valence::prelude::{App, Entity, Update};
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::create_mock_client;

    use crate::dandao::components::MutationStage;
    use crate::dandao::mutation::{ActiveMutation, BodySlot, MutationAdvanceEvent, MutationKind};
    use crate::dandao::visual_sync::MUTATION_VISUAL_CHANNEL;

    /// 辅助：收集 MockClientHelper 收到的所有 bong:mutation_visual 包的 JSON。
    fn drain_mutation_visual_payloads(
        helper: &mut valence::testing::MockClientHelper,
    ) -> Vec<serde_json::Value> {
        let mut out = Vec::new();
        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };
            if packet.channel.as_str() != MUTATION_VISUAL_CHANNEL {
                continue;
            }
            let val: serde_json::Value =
                serde_json::from_slice(packet.data.0 .0).expect("valid json payload");
            out.push(val);
        }
        out
    }

    fn flush_clients(app: &mut App) {
        let world = app.world_mut();
        let mut q = world.query::<&mut Client>();
        for mut c in q.iter_mut(world) {
            c.flush_packets().expect("mock flush");
        }
    }

    fn spawn_player_with_mutation(
        app: &mut App,
        toxin: f64,
    ) -> (Entity, valence::testing::MockClientHelper) {
        let (bundle, helper) = create_mock_client("Player");
        let state = MutationState {
            stage: MutationStage::Subtle,
            slots: vec![ActiveMutation {
                kind: MutationKind::GoldenIris,
                slot: BodySlot::Head,
                level: 1,
                acquired_tick: 10,
            }],
            meridian_penalty: 0.03,
        };
        let style = DandaoStyle {
            cumulative_toxin: toxin,
            ..DandaoStyle::default()
        };
        let entity = app.world_mut().spawn((bundle, state, style)).id();
        (entity, helper)
    }

    // --- happy path ---

    #[test]
    fn advance_event_sends_one_payload() {
        let mut app = App::new();
        app.add_event::<MutationAdvanceEvent>();
        app.add_systems(Update, mutation_visual_emit_system);

        let (entity, mut helper) = spawn_player_with_mutation(&mut app, 35.0);

        app.world_mut().send_event(MutationAdvanceEvent {
            entity,
            from_stage: MutationStage::None,
            to_stage: MutationStage::Subtle,
        });
        app.update();
        flush_clients(&mut app);

        let payloads = drain_mutation_visual_payloads(&mut helper);
        assert_eq!(
            payloads.len(),
            1,
            "MutationAdvanceEvent 应触发恰好 1 条 bong:mutation_visual payload"
        );
    }

    #[test]
    fn no_event_no_payload() {
        let mut app = App::new();
        app.add_event::<MutationAdvanceEvent>();
        app.add_systems(Update, mutation_visual_emit_system);

        let (_entity, mut helper) = spawn_player_with_mutation(&mut app, 35.0);

        // 不发送事件
        app.update();
        flush_clients(&mut app);

        let payloads = drain_mutation_visual_payloads(&mut helper);
        assert!(
            payloads.is_empty(),
            "无 MutationAdvanceEvent 时不应发 payload"
        );
    }

    // --- 契约字段验证 ---

    #[test]
    fn payload_contains_cumulative_toxin() {
        let mut app = App::new();
        app.add_event::<MutationAdvanceEvent>();
        app.add_systems(Update, mutation_visual_emit_system);

        let (entity, mut helper) = spawn_player_with_mutation(&mut app, 123.4);
        app.world_mut().send_event(MutationAdvanceEvent {
            entity,
            from_stage: MutationStage::None,
            to_stage: MutationStage::Subtle,
        });
        app.update();
        flush_clients(&mut app);

        let payloads = drain_mutation_visual_payloads(&mut helper);
        assert_eq!(payloads.len(), 1);
        let toxin = payloads[0]["cumulative_toxin"]
            .as_f64()
            .expect("cumulative_toxin 字段应存在且为 f64");
        assert!(
            (toxin - 123.4).abs() < 1e-6,
            "cumulative_toxin 应为 DandaoStyle 的值 123.4, got {toxin}"
        );
    }

    #[test]
    fn payload_kind_is_camel_case() {
        let mut app = App::new();
        app.add_event::<MutationAdvanceEvent>();
        app.add_systems(Update, mutation_visual_emit_system);

        let (entity, mut helper) = spawn_player_with_mutation(&mut app, 35.0);
        app.world_mut().send_event(MutationAdvanceEvent {
            entity,
            from_stage: MutationStage::None,
            to_stage: MutationStage::Subtle,
        });
        app.update();
        flush_clients(&mut app);

        let payloads = drain_mutation_visual_payloads(&mut helper);
        let kind = payloads[0]["slots"][0]["kind"]
            .as_str()
            .expect("slots[0].kind 应存在");
        assert_eq!(
            kind, "GoldenIris",
            "kind 必须是 CamelCase 'GoldenIris'，client fromServerName 依赖此格式"
        );
    }

    #[test]
    fn payload_body_slot_is_pascal_case() {
        let mut app = App::new();
        app.add_event::<MutationAdvanceEvent>();
        app.add_systems(Update, mutation_visual_emit_system);

        let (entity, mut helper) = spawn_player_with_mutation(&mut app, 35.0);
        app.world_mut().send_event(MutationAdvanceEvent {
            entity,
            from_stage: MutationStage::None,
            to_stage: MutationStage::Subtle,
        });
        app.update();
        flush_clients(&mut app);

        let payloads = drain_mutation_visual_payloads(&mut helper);
        let body_slot = payloads[0]["slots"][0]["body_slot"]
            .as_str()
            .expect("slots[0].body_slot 应存在");
        assert_eq!(
            body_slot, "Head",
            "body_slot 必须是 PascalCase 'Head'，client translateBodySlot(.toUpperCase()) 依赖此值"
        );
    }

    // --- 边界 ---

    #[test]
    fn payload_channel_is_correct() {
        assert_eq!(
            MUTATION_VISUAL_CHANNEL, "bong:mutation_visual",
            "channel ID 必须与 client BongNetworkHandler 注册值匹配"
        );
    }

    #[test]
    fn player_without_mutation_state_skips_gracefully() {
        // entity 没有 MutationState / DandaoStyle，系统不应 panic
        let mut app = App::new();
        app.add_event::<MutationAdvanceEvent>();
        app.add_systems(Update, mutation_visual_emit_system);

        let (bundle, mut helper) = create_mock_client("NoState");
        let entity = app.world_mut().spawn(bundle).id();

        app.world_mut().send_event(MutationAdvanceEvent {
            entity,
            from_stage: MutationStage::None,
            to_stage: MutationStage::Subtle,
        });
        app.update();
        flush_clients(&mut app);

        // 没有 MutationState → 系统应跳过，不发包
        let payloads = drain_mutation_visual_payloads(&mut helper);
        assert!(
            payloads.is_empty(),
            "缺少 MutationState 应跳过，不发 payload"
        );
    }

    #[test]
    fn multiple_events_send_multiple_payloads() {
        let mut app = App::new();
        app.add_event::<MutationAdvanceEvent>();
        app.add_systems(Update, mutation_visual_emit_system);

        let (entity, mut helper) = spawn_player_with_mutation(&mut app, 300.0);

        // 同一 tick 内两条事件（理论上不会发生，但系统应正确处理）
        app.world_mut().send_event(MutationAdvanceEvent {
            entity,
            from_stage: MutationStage::None,
            to_stage: MutationStage::Subtle,
        });
        app.world_mut().send_event(MutationAdvanceEvent {
            entity,
            from_stage: MutationStage::Subtle,
            to_stage: MutationStage::Visible,
        });
        app.update();
        flush_clients(&mut app);

        let payloads = drain_mutation_visual_payloads(&mut helper);
        assert_eq!(payloads.len(), 2, "两条事件应发两条 payload");
    }

    #[test]
    fn cumulative_toxin_zero_still_present_in_json() {
        let mut app = App::new();
        app.add_event::<MutationAdvanceEvent>();
        app.add_systems(Update, mutation_visual_emit_system);

        let (entity, mut helper) = spawn_player_with_mutation(&mut app, 0.0);
        app.world_mut().send_event(MutationAdvanceEvent {
            entity,
            from_stage: MutationStage::None,
            to_stage: MutationStage::Subtle,
        });
        app.update();
        flush_clients(&mut app);

        let payloads = drain_mutation_visual_payloads(&mut helper);
        assert_eq!(payloads.len(), 1);
        assert!(
            payloads[0].get("cumulative_toxin").is_some(),
            "cumulative_toxin=0 时字段仍应出现在 JSON 中（client fallback 0.0 依赖 key 存在）"
        );
    }

    #[test]
    fn player_without_dandao_style_skips_gracefully() {
        // entity 有 Client + MutationState，但缺 DandaoStyle → player_query 元组
        // (&MutationState, &DandaoStyle) 失配 → 系统跳过、不发包（emit 前置条件分支）。
        let mut app = App::new();
        app.add_event::<MutationAdvanceEvent>();
        app.add_systems(Update, mutation_visual_emit_system);

        let (bundle, mut helper) = create_mock_client("NoStyle");
        let state = MutationState {
            stage: MutationStage::Subtle,
            slots: vec![ActiveMutation {
                kind: MutationKind::GoldenIris,
                slot: BodySlot::Head,
                level: 1,
                acquired_tick: 10,
            }],
            meridian_penalty: 0.03,
        };
        // 仅 (bundle, state)，刻意不插 DandaoStyle
        let entity = app.world_mut().spawn((bundle, state)).id();

        app.world_mut().send_event(MutationAdvanceEvent {
            entity,
            from_stage: MutationStage::None,
            to_stage: MutationStage::Subtle,
        });
        app.update();
        flush_clients(&mut app);

        let payloads = drain_mutation_visual_payloads(&mut helper);
        assert!(
            payloads.is_empty(),
            "缺少 DandaoStyle 时元组查询失配，应跳过不发 payload，实际发了 {} 条",
            payloads.len()
        );
    }

    #[test]
    fn non_client_entity_with_mutation_sends_nothing() {
        // entity 有 MutationState + DandaoStyle 但不是在线 Client → payload 构建后
        // clients.get_mut(entity) 失配 → 跳过 send（emit 的第二个前置条件分支）。
        let mut app = App::new();
        app.add_event::<MutationAdvanceEvent>();
        app.add_systems(Update, mutation_visual_emit_system);

        // 旁观在线 client（事件不发给它，用于断言无 spurious 发包）
        let (_bystander, mut bystander_helper) = spawn_player_with_mutation(&mut app, 10.0);

        // 目标：有 state+style 但无 Client 组件
        let state = MutationState {
            stage: MutationStage::Subtle,
            slots: vec![ActiveMutation {
                kind: MutationKind::GoldenIris,
                slot: BodySlot::Head,
                level: 1,
                acquired_tick: 10,
            }],
            meridian_penalty: 0.03,
        };
        let style = DandaoStyle {
            cumulative_toxin: 50.0,
            ..DandaoStyle::default()
        };
        let non_client = app.world_mut().spawn((state, style)).id();

        app.world_mut().send_event(MutationAdvanceEvent {
            entity: non_client,
            from_stage: MutationStage::None,
            to_stage: MutationStage::Subtle,
        });
        app.update();
        flush_clients(&mut app);

        // 目标非 Client → 不发包；旁观 client 也不应收到任何包
        let payloads = drain_mutation_visual_payloads(&mut bystander_helper);
        assert!(
            payloads.is_empty(),
            "非 Client 实体触发的事件不应向任何 client 发包，旁观 client 实际收到 {} 条",
            payloads.len()
        );
    }
}
