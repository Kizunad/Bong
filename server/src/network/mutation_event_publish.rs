//! plan-dandao-runtime-wiring-v1 P2 — `bong:mutation_event` Redis 发布 system。
//!
//! 读取 [`MutationAdvanceEvent`]，组装对齐 agent TypeBox `MutationEventV1` 的 wire payload，
//! 通过 Redis 发布 `bong:mutation_event`，触发天道 `MutationNarrationRuntime` 叙事。
//!
//! # Wire 字段（对齐 agent TypeBox `additionalProperties:false`）
//! ```text
//! { v:1, entity_id:u64, from_stage, to_stage, cumulative_toxin:f64, at_tick:u64 }
//! ```
//! - `entity_id` = `Entity::to_bits()`
//! - `cumulative_toxin` 从 `DandaoStyle` 组件读取（`MutationAdvanceEvent` 不持有此值）
//! - `at_tick` 来自 `CultivationClock`
//!
//! # 范式
//! 仿 `network/poison_trait_emit.rs`：EventReader → 查询组件 → serialize → Redis send。

use valence::prelude::{Entity, EventReader, Query, Res};

use crate::cultivation::tick::CultivationClock;
use crate::dandao::components::{DandaoStyle, MutationStage};
use crate::dandao::mutation::MutationAdvanceEvent;
use crate::network::redis_bridge::RedisOutbound;
use crate::network::RedisBridgeResource;
use crate::schema::dandao::{MutationEventV1, MutationStageV1};

/// 将 [`MutationStage`]（内部枚举）转换为 wire enum [`MutationStageV1`]（serde snake_case）。
fn stage_to_wire(stage: MutationStage) -> MutationStageV1 {
    match stage {
        MutationStage::None => MutationStageV1::None,
        MutationStage::Subtle => MutationStageV1::Subtle,
        MutationStage::Visible => MutationStageV1::Visible,
        MutationStage::Heavy => MutationStageV1::Heavy,
        MutationStage::Bestial => MutationStageV1::Bestial,
    }
}

/// 监听 [`MutationAdvanceEvent`]，对每个变异推进事件向 Redis `bong:mutation_event` 发布
/// [`MutationEventV1`] payload，供天道 `MutationNarrationRuntime` 消费。
///
/// `redis` 用 `Option` 以兼容无 Redis 环境（如 P0 集成测试场景），缺 Resource 时静默跳过。
pub fn mutation_event_publish_system(
    redis: Option<Res<RedisBridgeResource>>,
    mut events: EventReader<MutationAdvanceEvent>,
    dandao_query: Query<(Entity, &DandaoStyle)>,
    clock: Option<Res<CultivationClock>>,
) {
    let Some(redis) = redis else {
        // 无 Redis resource（测试环境或初始化未完成），读完事件并静默跳过。
        events.clear();
        return;
    };

    let at_tick = clock.as_ref().map(|c| c.tick).unwrap_or(0);

    for event in events.read() {
        let entity = event.entity;

        // cumulative_toxin 来自 DandaoStyle 组件；若 entity 无 DandaoStyle 则跳过。
        let cumulative_toxin = match dandao_query.get(entity) {
            Ok((_, style)) => style.cumulative_toxin,
            Err(_) => {
                tracing::warn!(
                    "[bong][mutation_event] entity {entity:?} has MutationAdvanceEvent \
                     but no DandaoStyle; skipping publish"
                );
                continue;
            }
        };

        let payload = MutationEventV1 {
            v: 1,
            entity_id: entity.to_bits(),
            from_stage: stage_to_wire(event.from_stage),
            to_stage: stage_to_wire(event.to_stage),
            cumulative_toxin,
            at_tick,
        };

        let _ = redis
            .tx_outbound
            .send(RedisOutbound::MutationEvent(payload));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crossbeam_channel::TryRecvError;
    use valence::prelude::{App, Update};

    use crate::dandao::components::{DandaoStyle, MutationStage};
    use crate::dandao::mutation::MutationAdvanceEvent;

    fn build_test_app() -> (App, crossbeam_channel::Receiver<RedisOutbound>) {
        let mut app = App::new();
        let (tx_outbound, rx_outbound) = crossbeam_channel::unbounded();
        let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.add_event::<MutationAdvanceEvent>();
        app.add_systems(Update, mutation_event_publish_system);
        (app, rx_outbound)
    }

    // ─── happy-path ─────────────────────────────────────────────────────────

    #[test]
    fn mutation_advance_publishes_redis_event() {
        let (mut app, rx_outbound) = build_test_app();
        app.insert_resource(CultivationClock { tick: 12345 });

        let entity = app
            .world_mut()
            .spawn(DandaoStyle {
                cumulative_toxin: 105.0,
                ..DandaoStyle::default()
            })
            .id();

        app.world_mut().send_event(MutationAdvanceEvent {
            entity,
            from_stage: MutationStage::Subtle,
            to_stage: MutationStage::Visible,
        });
        app.update();

        match rx_outbound
            .try_recv()
            .expect("mutation event should be published to Redis")
        {
            RedisOutbound::MutationEvent(payload) => {
                assert_eq!(payload.v, 1, "v 字段必须为 1");
                assert_eq!(
                    payload.entity_id,
                    entity.to_bits(),
                    "entity_id 必须为 entity bits"
                );
                assert_eq!(
                    payload.from_stage,
                    MutationStageV1::Subtle,
                    "from_stage 必须对齐"
                );
                assert_eq!(
                    payload.to_stage,
                    MutationStageV1::Visible,
                    "to_stage 必须对齐"
                );
                assert_eq!(
                    payload.cumulative_toxin, 105.0,
                    "cumulative_toxin 应从 DandaoStyle 读取"
                );
                assert_eq!(payload.at_tick, 12345, "at_tick 应来自 CultivationClock");
            }
            other => panic!("expected MutationEvent outbound, got {other:?}"),
        }
        assert!(
            matches!(rx_outbound.try_recv(), Err(TryRecvError::Empty)),
            "只应发布 1 条事件"
        );
    }

    #[test]
    fn mutation_advance_wire_payload_has_correct_field_names() {
        // 验证序列化后 JSON 字段名与 agent TypeBox 完全对齐。
        let (mut app, rx_outbound) = build_test_app();
        app.insert_resource(CultivationClock { tick: 9999 });

        let entity = app
            .world_mut()
            .spawn(DandaoStyle {
                cumulative_toxin: 300.0,
                ..DandaoStyle::default()
            })
            .id();

        app.world_mut().send_event(MutationAdvanceEvent {
            entity,
            from_stage: MutationStage::Heavy,
            to_stage: MutationStage::Bestial,
        });
        app.update();

        match rx_outbound.try_recv().expect("should publish") {
            RedisOutbound::MutationEvent(payload) => {
                let json =
                    serde_json::to_string(&payload).expect("MutationEventV1 should serialize");
                let value: serde_json::Value =
                    serde_json::from_str(&json).expect("should be valid JSON");

                // agent TypeBox strict fields
                assert_eq!(value["v"], 1);
                assert!(
                    value["entity_id"].is_number(),
                    "entity_id 应为 JSON number（Integer）"
                );
                assert_eq!(value["from_stage"], "heavy");
                assert_eq!(value["to_stage"], "bestial");
                assert_eq!(value["cumulative_toxin"], 300.0);
                assert_eq!(value["at_tick"], 9999);
                // 旧字段不应出现
                assert!(value.get("entity").is_none());
                assert!(value.get("server_tick").is_none());
                assert!(value.get("new_meridian_penalty").is_none());
            }
            other => panic!("expected MutationEvent, got {other:?}"),
        }
    }

    // ─── 每个 MutationStage 迁移各一条 ────────────────────────────────────

    #[test]
    fn all_mutation_stage_transitions_publish() {
        let transitions = [
            (
                MutationStage::None,
                MutationStage::Subtle,
                MutationStageV1::None,
                MutationStageV1::Subtle,
            ),
            (
                MutationStage::Subtle,
                MutationStage::Visible,
                MutationStageV1::Subtle,
                MutationStageV1::Visible,
            ),
            (
                MutationStage::Visible,
                MutationStage::Heavy,
                MutationStageV1::Visible,
                MutationStageV1::Heavy,
            ),
            (
                MutationStage::Heavy,
                MutationStage::Bestial,
                MutationStageV1::Heavy,
                MutationStageV1::Bestial,
            ),
        ];

        for (from, to, expected_from, expected_to) in transitions {
            let (mut app, rx_outbound) = build_test_app();
            app.insert_resource(CultivationClock { tick: 1 });

            let entity = app
                .world_mut()
                .spawn(DandaoStyle {
                    cumulative_toxin: 50.0,
                    ..DandaoStyle::default()
                })
                .id();

            app.world_mut().send_event(MutationAdvanceEvent {
                entity,
                from_stage: from,
                to_stage: to,
            });
            app.update();

            match rx_outbound
                .try_recv()
                .expect("should publish for transition")
            {
                RedisOutbound::MutationEvent(p) => {
                    assert_eq!(
                        p.from_stage, expected_from,
                        "from_stage 不匹配 {from:?}→{to:?}"
                    );
                    assert_eq!(p.to_stage, expected_to, "to_stage 不匹配 {from:?}→{to:?}");
                }
                other => panic!("expected MutationEvent for {from:?}→{to:?}, got {other:?}"),
            }
        }
    }

    // ─── 无 DandaoStyle → 跳过（不 panic，不发布） ──────────────────────────

    #[test]
    fn no_dandao_style_skips_publish() {
        let (mut app, rx_outbound) = build_test_app();
        app.insert_resource(CultivationClock { tick: 1 });

        // entity 没有 DandaoStyle 组件
        let entity = app.world_mut().spawn(()).id();
        app.world_mut().send_event(MutationAdvanceEvent {
            entity,
            from_stage: MutationStage::None,
            to_stage: MutationStage::Subtle,
        });
        app.update();

        assert!(
            matches!(rx_outbound.try_recv(), Err(TryRecvError::Empty)),
            "无 DandaoStyle 时不应发布事件"
        );
    }

    // ─── 无 event → 不发布 ─────────────────────────────────────────────────

    #[test]
    fn no_event_no_publish() {
        let (mut app, rx_outbound) = build_test_app();
        app.insert_resource(CultivationClock { tick: 1 });
        app.update();

        assert!(
            matches!(rx_outbound.try_recv(), Err(TryRecvError::Empty)),
            "无事件时 Redis 应保持静默"
        );
    }

    // ─── 多事件 → 多条发布 ──────────────────────────────────────────────────

    #[test]
    fn multiple_events_publish_multiple() {
        let (mut app, rx_outbound) = build_test_app();
        app.insert_resource(CultivationClock { tick: 1 });

        let e1 = app
            .world_mut()
            .spawn(DandaoStyle {
                cumulative_toxin: 50.0,
                ..DandaoStyle::default()
            })
            .id();
        let e2 = app
            .world_mut()
            .spawn(DandaoStyle {
                cumulative_toxin: 200.0,
                ..DandaoStyle::default()
            })
            .id();

        app.world_mut().send_event(MutationAdvanceEvent {
            entity: e1,
            from_stage: MutationStage::None,
            to_stage: MutationStage::Subtle,
        });
        app.world_mut().send_event(MutationAdvanceEvent {
            entity: e2,
            from_stage: MutationStage::Visible,
            to_stage: MutationStage::Heavy,
        });
        app.update();

        let mut count = 0;
        while let Ok(msg) = rx_outbound.try_recv() {
            assert!(matches!(msg, RedisOutbound::MutationEvent(_)));
            count += 1;
        }
        assert_eq!(count, 2, "2 个事件应发布 2 条 Redis 消息");
    }

    // ─── at_tick 来自 clock，无 clock 时 fallback 0 ─────────────────────────

    #[test]
    fn at_tick_fallback_zero_when_no_clock() {
        let (mut app, rx_outbound) = build_test_app();
        // 不 insert CultivationClock

        let entity = app
            .world_mut()
            .spawn(DandaoStyle {
                cumulative_toxin: 10.0,
                ..DandaoStyle::default()
            })
            .id();
        app.world_mut().send_event(MutationAdvanceEvent {
            entity,
            from_stage: MutationStage::None,
            to_stage: MutationStage::Subtle,
        });
        app.update();

        match rx_outbound.try_recv().expect("should publish") {
            RedisOutbound::MutationEvent(p) => {
                assert_eq!(
                    p.at_tick, 0,
                    "无 CultivationClock 时 at_tick 应 fallback 为 0"
                );
            }
            other => panic!("expected MutationEvent, got {other:?}"),
        }
    }
}
