//! plan-race-system-v1 P4 — 易形状态快照 S2C（proto field 142 `morph_state`）。
//!
//! 只保证服务端 emit 侧的契约（本 PR 的验收面）——client 渲染消费（PR-5b）不在本
//! 模块范围。仿 `race_gate_meta_emit.rs` 的 join-once 节流模式，叠加
//! `cultivation_detail_emit.rs` 的周期性全量重发节流（`emit_interval_ticks`）。
//!
//! **简化说明**：规格原文额外要求"变形解除瞬间 64 格半径 delta 广播"（`mode="delta"`，
//! `active=false` 立即通知视距内玩家）；本 PR 只交付 `mode="full"`（join 首帧 +
//! 周期性全量重发，client 全量替换本地缓存）。由于生产 `races.json.morph_pairs`
//! 当前为空数组（P4 无真实可易形目标），`MorphState` 组件在生产环境永远不会被插入，
//! 这条简化在 P4 阶段没有可观测差异；delta 广播留给 PR-5b 或后续加固批次一并接入
//! （周期性全量重发已经能在 `MORPH_STATE_SYNC_INTERVAL_TICKS` 内让 client 收敛到
//! 正确状态，只是不是"瞬间"）。

use valence::entity::EntityId;
use valence::prelude::{
    bevy_ecs, Client, Commands, Component, Entity, Query, Res, ResMut, Resource, With,
};

use crate::body_plan::MorphState;
use crate::cultivation::tick::CultivationClock;
use crate::network::agent_bridge::{payload_type_label, serialize_server_data_payload};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::server_data::{
    MorphStateEntryV1, MorphStateV1, ServerDataPayloadV1, ServerDataV1,
};

/// 周期性全量重发间隔（tick）——与 `cultivation_detail_emit::EMIT_INTERVAL_TICKS`
/// 同数量级（~1s @ 20TPS）。
const MORPH_STATE_SYNC_INTERVAL_TICKS: u64 = 20;

/// plan-race-system-v1 P4 —— 标记已给该客户端下发过 join 首帧 `morph_state`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct LastSentMorphStateJoin;

#[derive(Default, Resource)]
pub struct MorphStateEmitState {
    last_emit_tick: u64,
}

/// 从全部当前处于 `MorphState` 的实体构建一份 `mode="full"` 快照——`active` 恒为
/// `true`（未易形实体压根不出现在表里，client 用"表里没有=未易形"的缺省语义，
/// 与 `race_gate_meta` 的"表里没有=Any 放行"同一惯例）。
pub fn build_morph_state_full(morphed: impl Iterator<Item = (i32, MorphState)>) -> MorphStateV1 {
    let mut entries: Vec<MorphStateEntryV1> = morphed
        .map(|(entity_id, state)| MorphStateEntryV1 {
            entity_id,
            model_kind: u32::from(state.model_kind),
            form_race_id: state.form.as_str().to_string(),
            form_body_plan_id: state.form.as_str().to_string(),
            active: true,
        })
        .collect();
    entries.sort_by_key(|e| e.entity_id);
    MorphStateV1 {
        mode: "full".to_string(),
        entries,
    }
}

type MorphStateEmitClientItem<'a> = (Entity, &'a mut Client, Option<&'a LastSentMorphStateJoin>);

/// join 首帧 + 周期性全量重发（同一 tick 内给"首帧未发过"和"到周期"的客户端都补发）。
pub fn emit_morph_state_payloads(
    mut commands: Commands,
    clock: Res<CultivationClock>,
    mut state: ResMut<MorphStateEmitState>,
    morphed_q: Query<(&EntityId, &MorphState)>,
    mut clients: Query<MorphStateEmitClientItem<'_>, With<Client>>,
) {
    let due_for_periodic_resync =
        clock.tick.saturating_sub(state.last_emit_tick) >= MORPH_STATE_SYNC_INTERVAL_TICKS;
    if due_for_periodic_resync {
        state.last_emit_tick = clock.tick;
    }

    // 无客户端待发（既非首帧也未到周期）时提前跳过，避免每 tick 都重算快照。
    let any_first_frame = clients.iter().any(|(_, _, last_sent)| last_sent.is_none());
    if !due_for_periodic_resync && !any_first_frame {
        return;
    }

    let snapshot = build_morph_state_full(
        morphed_q
            .iter()
            .map(|(entity_id, morph_state)| (entity_id.get(), morph_state.clone())),
    );

    for (entity, mut client, last_sent) in &mut clients {
        if last_sent.is_none() {
            // fallthrough — 首帧必发。
        } else if !due_for_periodic_resync {
            continue;
        }

        let payload = ServerDataV1::new(ServerDataPayloadV1::MorphState(snapshot.clone()));
        let payload_type = payload_type_label(payload.payload_type());
        match serialize_server_data_payload(&payload) {
            Ok(bytes) => {
                send_server_data_payload(&mut client, bytes.as_slice());
                if last_sent.is_none() {
                    commands.entity(entity).insert(LastSentMorphStateJoin);
                }
            }
            Err(error) => log_payload_build_error(payload_type, &error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body_plan::types::RaceId;
    use valence::prelude::{App, Update};
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    fn flush_client_packets(app: &mut App) {
        let world = app.world_mut();
        let mut query = world.query::<&mut Client>();
        for mut client in query.iter_mut(world) {
            client
                .flush_packets()
                .expect("mock client packets should flush");
        }
    }

    fn collect_morph_state_payloads(helper: &mut MockClientHelper) -> Vec<MorphStateV1> {
        helper
            .collect_received()
            .0
            .into_iter()
            .filter_map(|frame| {
                let packet = frame.decode::<CustomPayloadS2c>().ok()?;
                let payload = serde_json::from_slice::<ServerDataV1>(packet.data.0 .0).ok()?;
                match payload.payload {
                    ServerDataPayloadV1::MorphState(state) => Some(state),
                    _ => None,
                }
            })
            .collect()
    }

    #[test]
    fn build_morph_state_full_empty_when_no_morphed_entities() {
        let snapshot = build_morph_state_full(std::iter::empty());
        assert_eq!(snapshot.mode, "full");
        assert!(snapshot.entries.is_empty());
    }

    #[test]
    fn build_morph_state_full_maps_fields_and_sorts_by_entity_id() {
        let snapshot = build_morph_state_full(
            vec![
                (99, MorphState::new(RaceId::new("whale"), 2, 10)),
                (5, MorphState::new(RaceId::new("beetle"), 1, 20)),
            ]
            .into_iter(),
        );
        assert_eq!(snapshot.entries.len(), 2);
        assert_eq!(
            snapshot.entries[0].entity_id, 5,
            "必须按 entity_id 升序排列"
        );
        assert_eq!(snapshot.entries[0].form_race_id, "beetle");
        assert_eq!(snapshot.entries[0].model_kind, 1);
        assert!(snapshot.entries[0].active);
        assert_eq!(snapshot.entries[1].entity_id, 99);
        assert_eq!(snapshot.entries[1].form_race_id, "whale");
    }

    #[test]
    fn join_first_frame_sends_full_snapshot_and_marks_client() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 0 });
        app.init_resource::<MorphStateEmitState>();
        app.add_systems(Update, emit_morph_state_payloads);
        let (client_bundle, mut helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();

        app.update();
        flush_client_packets(&mut app);

        let payloads = collect_morph_state_payloads(&mut helper);
        assert_eq!(payloads.len(), 1, "join 首帧必须恰好发一条 morph_state");
        assert_eq!(payloads[0].mode, "full");
        assert!(payloads[0].entries.is_empty(), "无易形实体时表应为空");
        assert!(
            app.world().get::<LastSentMorphStateJoin>(entity).is_some(),
            "join 首帧发送后必须标记 LastSentMorphStateJoin"
        );
    }

    #[test]
    fn already_sent_client_not_due_for_resync_gets_nothing() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 0 });
        app.init_resource::<MorphStateEmitState>();
        app.add_systems(Update, emit_morph_state_payloads);
        let (client_bundle, mut helper) = create_mock_client("Azure");
        app.world_mut()
            .spawn((client_bundle, LastSentMorphStateJoin));

        app.update();
        flush_client_packets(&mut app);

        let payloads = collect_morph_state_payloads(&mut helper);
        assert!(
            payloads.is_empty(),
            "已发过 join 首帧、且未到周期重发窗口的客户端不应再收到 morph_state"
        );
    }

    #[test]
    fn periodic_resync_resends_to_already_sent_clients_after_interval() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 0 });
        app.init_resource::<MorphStateEmitState>();
        app.add_systems(Update, emit_morph_state_payloads);
        let (client_bundle, mut helper) = create_mock_client("Azure");
        app.world_mut()
            .spawn((client_bundle, LastSentMorphStateJoin));

        app.insert_resource(CultivationClock {
            tick: MORPH_STATE_SYNC_INTERVAL_TICKS,
        });
        app.update();
        flush_client_packets(&mut app);

        let payloads = collect_morph_state_payloads(&mut helper);
        assert_eq!(
            payloads.len(),
            1,
            "到达周期重发窗口后，即便已发过 join 首帧也必须重发一条全量快照"
        );
        assert_eq!(payloads[0].mode, "full");
    }

    // 注：ECS 层"真实易形实体出现在下发快照里"的场景由 `build_morph_state_full`
    // 的纯函数测试覆盖字段映射/排序正确性（见上）；`EntityId` 是 valence
    // `EntityManager` 内部分配的私有字段元组，测试无法脱离真实 spawn 流程手搓
    // 任意值，故 ECS 集成层只锁"有/无实体待发"两个节流分支（上方三条 join/resync
    // 测试），不重复在此处手搓 EntityId。
}
