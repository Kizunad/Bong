//! plan-race-system-v1 P4/PR-5b — 易形状态快照 S2C（proto field 142 `morph_state`）。
//!
//! `mode="full"`：仿 `race_gate_meta_emit.rs` 的 join-once 节流模式，叠加
//! `cultivation_detail_emit.rs` 的周期性全量重发节流（`emit_interval_ticks`），
//! join 首帧 + 每 `MORPH_STATE_SYNC_INTERVAL_TICKS` 全量重发给全部 client（无视距
//! 过滤——client 全量替换本地缓存，靠 client 侧渲染时再按可见性裁剪）。
//!
//! `mode="delta"`（PR-5b 补齐 P4 遗留简化）：`Added<MorphState>`（易形）/
//! `RemovedComponents<MorphState>`（解除——手动再 cast / 死亡 `combat::lifecycle` /
//! 下线 `player::mod` 三条触发路径共用 `body_plan::morph::release_morph_state`，本
//! 系统只订阅组件增删，不关心谁触发的移除）触发瞬间广播，`active=false` 表示删除。
//! 半径过滤复用 `disguise_sync::ids_visible_to_client`（按 client `ViewDistance` 动态
//! 推导的 XZ Chebyshev 半径，反作弊加固版），**不用固定 64 欧氏**——固定常数在大
//! ViewDistance/preview 模式下会把明明在视野内的玩家漏掉，`disguise_sync` 模块文档
//! 已把这条教训写死为跨模块惯例（daozhan/spider 伪装同款），本模块没有理由另起炉灶。

use std::collections::{HashMap, HashSet};

use valence::entity::EntityId;
use valence::prelude::{
    bevy_ecs, Added, Client, Commands, Component, Entity, Position, Query, RemovedComponents, Res,
    ResMut, Resource, ViewDistance, With,
};

use crate::body_plan::MorphState;
use crate::cultivation::tick::CultivationClock;
use crate::network::agent_bridge::{payload_type_label, serialize_server_data_payload};
use crate::network::disguise_sync::ids_visible_to_client;
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

/// plan-race-system-v1 PR-5b —— 追踪已易形实体的最近已知坐标，供
/// `RemovedComponents<MorphState>` 触发的移除瞬间提供半径过滤中心（组件移除后
/// 实体本身可能仍存活——手动 cast 切换——也可能同 tick 内已 despawn——下线——两种
/// 情形都要能定位广播半径中心，故坐标必须提前缓存而非移除后现查）。
#[derive(Default, Resource)]
pub struct MorphStateEntityCache {
    positions: HashMap<Entity, (i32, [f64; 3])>,
}

/// 一条 delta 变更（插入 or 移除）连同其广播半径中心坐标。
struct MorphStateDeltaChange {
    entry: MorphStateEntryV1,
    center: [f64; 3],
}

fn build_removed_entry(entity_id: i32) -> MorphStateEntryV1 {
    // 文档约定（`MorphStateEntryV1` doc）：`active=false` 时其余字段恒为空/0，
    // 仅 `entity_id`/`active` 有意义——client 侧只据此从本地缓存删除该 entity_id。
    MorphStateEntryV1 {
        entity_id,
        model_kind: 0,
        form_race_id: String::new(),
        form_body_plan_id: String::new(),
        active: false,
    }
}

/// `mode="delta"` 半径广播——`Added<MorphState>`（易形插入）/
/// `RemovedComponents<MorphState>`（易形解除，含手动 cast 切换 / 死亡 / 下线三条
/// 触发路径）触发瞬间广播给视距内 client（`disguise_sync::ids_visible_to_client`）。
///
/// 与 `emit_morph_state_payloads`（`mode="full"`）是两条独立系统——full 负责 join
/// 首帧兜底 + 周期性收敛漂移，delta 负责瞬时通知，二者共同保证最终一致性
/// （client `MorphStateStore` 消费两种 mode，见 PR-5b client 侧文档）。
pub fn emit_morph_state_delta_payloads(
    mut cache: ResMut<MorphStateEntityCache>,
    morphed_q: Query<(Entity, &EntityId, &Position, &MorphState)>,
    added_q: Query<Entity, Added<MorphState>>,
    mut removed: RemovedComponents<MorphState>,
    mut clients: Query<(&mut Client, &Position, &ViewDistance)>,
) {
    // 每 tick 刷新一遍全部当前已易形实体的缓存坐标——保证一旦该实体在未来某 tick
    // 被移除（可能与其他组件同 tick 一起 despawn），移除分支仍能取到"移除前最后
    // 一次刷新"的坐标，而不是要求坐标恰好在移除的同一 tick 内被更新。
    for (entity, entity_id, position, _) in &morphed_q {
        let p = position.get();
        cache
            .positions
            .insert(entity, (entity_id.get(), [p.x, p.y, p.z]));
    }

    let mut changes: Vec<MorphStateDeltaChange> = Vec::new();

    for entity in &added_q {
        let Ok((_, entity_id, position, state)) = morphed_q.get(entity) else {
            continue;
        };
        let p = position.get();
        changes.push(MorphStateDeltaChange {
            entry: MorphStateEntryV1 {
                entity_id: entity_id.get(),
                model_kind: u32::from(state.model_kind),
                form_race_id: state.form.as_str().to_string(),
                form_body_plan_id: state.form.as_str().to_string(),
                active: true,
            },
            center: [p.x, p.y, p.z],
        });
    }

    for entity in removed.read() {
        let Some((entity_id, center)) = cache.positions.remove(&entity) else {
            // 缓存里没有该实体——从未被本系统观察到过就被移除（例如测试直接构造
            // world 未经过任何 tick），没有半径中心可用，安全地跳过而非广播全图。
            continue;
        };
        changes.push(MorphStateDeltaChange {
            entry: build_removed_entry(entity_id),
            center,
        });
    }

    if changes.is_empty() {
        return;
    }

    let candidates: Vec<(i32, [f64; 3])> = changes
        .iter()
        .map(|change| (change.entry.entity_id, change.center))
        .collect();

    for (mut client, client_pos, view_distance) in &mut clients {
        let visible_ids: HashSet<i32> =
            ids_visible_to_client(&candidates, client_pos, view_distance)
                .into_iter()
                .collect();
        if visible_ids.is_empty() {
            continue;
        }
        let entries: Vec<MorphStateEntryV1> = changes
            .iter()
            .filter(|change| visible_ids.contains(&change.entry.entity_id))
            .map(|change| change.entry.clone())
            .collect();
        if entries.is_empty() {
            continue;
        }

        let payload = ServerDataV1::new(ServerDataPayloadV1::MorphState(MorphStateV1 {
            mode: "delta".to_string(),
            entries,
        }));
        let payload_type = payload_type_label(payload.payload_type());
        match serialize_server_data_payload(&payload) {
            Ok(bytes) => send_server_data_payload(&mut client, bytes.as_slice()),
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
    //
    // plan-race-system-v1 PR-5b —— `emit_morph_state_delta_payloads` 的 ECS 集成测试
    // 需要真实 `EntityId`（半径过滤按 `(entity_id, position)` pair 工作），走
    // `client_request_handler.rs` 既有先例：`app.add_plugins(EntityPlugin)` 后
    // spawn `EntityId::default()` 占位 + `app.update()`，valence `EntityPlugin`
    // 会把占位替换为真实分配的 protocol id。
    mod delta_tests {
        use super::*;
        use crate::body_plan::types::RaceId;
        use valence::entity::EntityPlugin;
        use valence::prelude::{DVec3, EntityKind, OldPosition};

        fn spawn_morphed_npc(app: &mut App, pos: DVec3) -> (Entity, i32) {
            let npc = app
                .world_mut()
                .spawn((
                    EntityKind::PLAYER,
                    EntityId::default(),
                    Position::new(pos),
                    OldPosition::new(pos),
                ))
                .id();
            app.update();
            let entity_id = app
                .world()
                .get::<EntityId>(npc)
                .expect("EntityPlugin must assign a protocol id")
                .get();
            (npc, entity_id)
        }

        fn spawn_client_at(app: &mut App, pos: DVec3) -> (Entity, MockClientHelper) {
            let (bundle, helper) = create_mock_client("Azure");
            let client = app.world_mut().spawn(bundle).id();
            app.world_mut()
                .entity_mut(client)
                .insert(Position::new(pos));
            (client, helper)
        }

        fn base_app() -> App {
            let mut app = App::new();
            app.add_plugins(EntityPlugin);
            app.init_resource::<MorphStateEntityCache>();
            app.add_systems(Update, emit_morph_state_delta_payloads);
            app
        }

        #[test]
        fn cast_insert_broadcasts_active_true_delta_to_visible_client() {
            let mut app = base_app();
            let (_client, mut helper) = spawn_client_at(&mut app, DVec3::new(0.0, 64.0, 0.0));
            let (npc, entity_id) = spawn_morphed_npc(&mut app, DVec3::new(5.0, 64.0, 5.0));

            app.world_mut()
                .entity_mut(npc)
                .insert(MorphState::new(RaceId::new("whale"), 0, 42));

            app.update();
            flush_client_packets(&mut app);

            let payloads = collect_morph_state_payloads(&mut helper);
            assert_eq!(
                payloads.len(),
                1,
                "视距内 client 应恰好收到一条 delta payload"
            );
            assert_eq!(payloads[0].mode, "delta");
            assert_eq!(payloads[0].entries.len(), 1);
            assert_eq!(payloads[0].entries[0].entity_id, entity_id);
            assert!(payloads[0].entries[0].active, "易形插入应下发 active=true");
            assert_eq!(payloads[0].entries[0].form_race_id, "whale");
        }

        #[test]
        fn cast_insert_not_broadcast_to_client_outside_radius() {
            let mut app = base_app();
            // vd 默认 = 2 chunks → 半径 (2+2)*16 = 64；把 client 放在 200 格外，明确出视距。
            let (_client, mut helper) = spawn_client_at(&mut app, DVec3::new(0.0, 64.0, 0.0));
            let (npc, _entity_id) = spawn_morphed_npc(&mut app, DVec3::new(300.0, 64.0, 0.0));

            app.world_mut()
                .entity_mut(npc)
                .insert(MorphState::new(RaceId::new("whale"), 0, 1));

            app.update();
            flush_client_packets(&mut app);

            let payloads = collect_morph_state_payloads(&mut helper);
            assert!(
                payloads.is_empty(),
                "视距外 client 不应收到 delta 广播，实际 {payloads:?}"
            );
        }

        #[test]
        fn release_removes_broadcasts_active_false_delta_using_cached_position() {
            let mut app = base_app();
            let (_client, mut helper) = spawn_client_at(&mut app, DVec3::new(0.0, 64.0, 0.0));
            let (npc, entity_id) = spawn_morphed_npc(&mut app, DVec3::new(5.0, 64.0, 5.0));

            app.world_mut()
                .entity_mut(npc)
                .insert(MorphState::new(RaceId::new("whale"), 0, 1));
            // 插入瞬间的 delta（active=true）先跑掉，避免污染下面对 active=false 那条的断言。
            app.update();
            flush_client_packets(&mut app);
            let _ = collect_morph_state_payloads(&mut helper);

            app.world_mut().entity_mut(npc).remove::<MorphState>();
            app.update();
            flush_client_packets(&mut app);

            let payloads = collect_morph_state_payloads(&mut helper);
            assert_eq!(
                payloads.len(),
                1,
                "解除易形应恰好广播一条 delta（用移除前最后一次缓存坐标定位半径）"
            );
            assert_eq!(payloads[0].mode, "delta");
            assert_eq!(payloads[0].entries.len(), 1);
            assert_eq!(payloads[0].entries[0].entity_id, entity_id);
            assert!(
                !payloads[0].entries[0].active,
                "解除易形应下发 active=false"
            );
            assert_eq!(
                payloads[0].entries[0].form_race_id, "",
                "active=false 时 form_race_id 恒空（文档约定），不残留最后已知形态"
            );
            assert_eq!(payloads[0].entries[0].model_kind, 0);
        }

        #[test]
        fn removal_never_observed_by_system_is_skipped_without_panic() {
            // 缓存里从未记录过该实体（例如系统从未跑过就直接移除组件），
            // 移除分支应安全跳过，不 panic、不广播一条没有半径中心的假 delta。
            let mut app = base_app();
            let (_client, mut helper) = spawn_client_at(&mut app, DVec3::new(0.0, 64.0, 0.0));

            // 直接在同一 tick 内插入又移除，系统从未观察到"已插入"的中间态。
            let npc = app
                .world_mut()
                .spawn((
                    EntityKind::PLAYER,
                    EntityId::default(),
                    Position::new(DVec3::new(5.0, 64.0, 5.0)),
                    OldPosition::new(DVec3::new(5.0, 64.0, 5.0)),
                    MorphState::new(RaceId::new("whale"), 0, 1),
                ))
                .id();
            app.world_mut().entity_mut(npc).remove::<MorphState>();

            app.update();
            flush_client_packets(&mut app);

            let payloads = collect_morph_state_payloads(&mut helper);
            assert!(
                payloads.is_empty(),
                "缓存未命中的移除必须安全跳过，不产生 payload，实际 {payloads:?}"
            );
        }

        #[test]
        fn no_changes_this_tick_emits_nothing() {
            let mut app = base_app();
            let (_client, mut helper) = spawn_client_at(&mut app, DVec3::new(0.0, 64.0, 0.0));

            app.update();
            flush_client_packets(&mut app);

            let payloads = collect_morph_state_payloads(&mut helper);
            assert!(
                payloads.is_empty(),
                "无任何易形增删时不应发送 delta payload"
            );
        }
    }

    #[test]
    fn build_removed_entry_defaults_inactive_fields_and_carries_entity_id() {
        let entry = build_removed_entry(77);
        assert_eq!(entry.entity_id, 77);
        assert!(!entry.active);
        assert_eq!(
            entry.model_kind, 0,
            "active=false 时 model_kind 恒 0（文档约定）"
        );
        assert_eq!(entry.form_race_id, "", "active=false 时 form_race_id 恒空");
        assert_eq!(
            entry.form_body_plan_id, "",
            "active=false 时 form_body_plan_id 恒空"
        );
    }
}
