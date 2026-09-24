//! plan-dying-elder-v1 B1 修复 — S2C `bong:elder_encounter` payload 推送。
//!
//! server 在 P3 Redis 广播的同时，也向同 zone 在场玩家推送
//! `bong:elder_encounter` S2C CustomPayload，驱动 client HUD 显示。
//!
//! ## 触发时机
//! - 大能出现（appeared）：监听 `DyingElderSpawnRequest`
//! - 收丹（dan_received）：只监听 `DyingElderDanAcceptedEvent` 权威提交快照；原始
//!   `GiveDanToElderIntent` 可能被库存/状态/真元门禁拒绝，不能驱动客户端反馈
//! - 死亡（betrayal / dead_natural）：监听 Dead 状态变化（在 death_system 之前，
//!   与 `dying_elder_p3_emit_death_event_system` 相同时序）
//!
//! ## payload 格式（与 client DyingElderEncounterHandler 对齐）
//! ```json
//! {
//!   "zone_name":          string,
//!   "elder_entity_id":    i32,      // MC protocol entity_id（Valence EntityId::get()，从 1 起分配）
//!   "event_kind":         string,   // snake_case
//!   "betray_probability": f64,
//!   "dan_count":          u32,
//!   "offered_skill_id":   string,
//!   "qi_fraction":        f32,      // M2 修复：真实 qi_current/qi_max_cache
//!   "server_tick":        u64
//! }
//! ```
//!
//! ## 守恒红线
//! 本模块 **只** 推送显示层数据，不修改任何 gameplay 数值（qi、血量、状态）。

use valence::client::ClientMarker;
use valence::entity::EntityId;
use valence::ident;
use valence::prelude::{
    bevy_ecs, App, Client, Commands, Component, Entity, EventReader, IntoSystemConfigs, Position,
    Query, Res, ResMut, Resource, Update, With, Without,
};

use crate::fauna::dying_elder::{
    DyingElderAppearedEvent, DyingElderBlackboard, DyingElderDanAcceptedEvent, DyingElderState,
};
use crate::npc::lifecycle::{NpcDeathReason, NpcTerminalSettlementSucceeded, NpcTerminalSystemSet};
use crate::npc::movement::GameTick;
use crate::npc::spawn::NpcMarker;
use crate::schema::channels::CH_ELDER_ENCOUNTER;
use crate::schema::common::MAX_PAYLOAD_BYTES;
use crate::schema::elder_encounter::{ElderEncounterEventKindV1, ElderEncounterEventV1};
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::zone::ZoneRegistry;

// ── 辅助函数 ──────────────────────────────────────────────────────────────────

/// 序列化 `ElderEncounterEventV1` → JSON bytes；失败返回 None 并记录 warn。
fn to_json_bytes(event: &ElderEncounterEventV1) -> Option<Vec<u8>> {
    let bytes = match serde_json::to_vec(event) {
        Ok(bytes) => bytes,
        Err(e) => {
            tracing::warn!(
                "[bong][elder_encounter_emit] failed to serialize ElderEncounterEventV1: {e}"
            );
            return None;
        }
    };
    if bytes.len() > MAX_PAYLOAD_BYTES {
        tracing::error!(
            "[bong][elder_encounter_emit] payload for {} rejected as oversize: {} > {}",
            CH_ELDER_ENCOUNTER,
            bytes.len(),
            MAX_PAYLOAD_BYTES,
        );
        return None;
    }
    Some(bytes)
}

/// 向 `players` Query 中所有在 `zone_name` 且维度为 Overworld 的玩家发送 S2C payload。
///
/// `players`：(Entity, &mut Client, &Position, Option<&CurrentDimension>)
fn send_to_players_in_zone<'w>(
    players: &mut Query<
        'w,
        '_,
        (&mut Client, &Position, Option<&CurrentDimension>),
        With<ClientMarker>,
    >,
    zone_name: &str,
    zones: &ZoneRegistry,
    bytes: &[u8],
) -> usize {
    let mut sent = 0;
    for (mut client, pos, current_dim) in players.iter_mut() {
        let dim = current_dim.map(|d| d.0).unwrap_or(DimensionKind::Overworld);
        // 仅推送给同 zone 内玩家（按 AABB 最小匹配）
        if let Some(zone) = zones.find_zone(dim, pos.get()) {
            if zone.name == zone_name {
                client.send_custom_payload(ident!("bong:elder_encounter"), bytes);
                sent += 1;
            }
        }
    }
    sent
}

// ── System 类型别名 ────────────────────────────────────────────────────────────

type PlayerQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Client,
        &'static Position,
        Option<&'static CurrentDimension>,
    ),
    With<ClientMarker>,
>;

type DyingElderQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static EntityId,
        &'static DyingElderBlackboard,
        &'static DyingElderState,
    ),
    (With<NpcMarker>, Without<ClientMarker>),
>;

/// 已向在场客户端推送过垂死大能死亡事件。
///
/// 真元释放失败时 `DyingElderDeathProcessed` 会故意保持缺失以允许下一 tick 重试；
/// 客户端通知必须使用独立 marker，避免同一死亡每 tick 重播。
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct DyingElderDeathS2cBroadcast;

/// appeared 事件可能早于 Valence 为新 marker 分配 EntityId；保留事件到下一帧重试，
/// 避免客户端永远收不到 encounter HUD，也让协议 entity_id 与后续请求使用同一权威值。
#[derive(Resource, Default)]
struct PendingElderAppearS2c {
    events: Vec<DyingElderAppearedEvent>,
}

// ── S2C appeared 系统 ─────────────────────────────────────────────────────────

/// plan-dying-elder-v1 B1 Bug2 修复 — 大能出现时向同 zone 玩家推送 `bong:elder_encounter` appeared 事件。
///
/// 改为监听 `DyingElderAppearedEvent`（由 `dying_elder_apply_spawn_system` 在 entity 创建后 emit），
/// 通过 `elder_id_query` 取 Valence `EntityId::get()`（MC protocol entity_id，i32，从 1 起分配）。
/// qi_fraction = 1.0（spawn 时真元满值）。
#[allow(clippy::type_complexity)]
pub(crate) fn elder_encounter_s2c_appear_system(
    mut appeared_events: EventReader<DyingElderAppearedEvent>,
    elder_id_query: Query<&EntityId, (With<NpcMarker>, Without<ClientMarker>)>,
    elder_entities: Query<(), (With<NpcMarker>, Without<ClientMarker>)>,
    mut players: PlayerQuery<'_, '_>,
    zones: Option<Res<ZoneRegistry>>,
    mut pending: ResMut<PendingElderAppearS2c>,
) {
    let Some(zones) = zones else { return };

    pending.events.extend(appeared_events.read().cloned());
    let events = std::mem::take(&mut pending.events);

    for ev in events {
        let Ok(entity_id) = elder_id_query.get(ev.elder) else {
            if elder_entities.get(ev.elder).is_err() {
                tracing::debug!(
                    "[bong][elder_encounter_emit] drop appeared for despawned elder {:?}",
                    ev.elder
                );
                continue;
            }
            tracing::warn!(
                "[bong][elder_encounter_emit] S2C appeared: no EntityId for elder {:?}, retrying",
                ev.elder
            );
            pending.events.push(ev.clone());
            continue;
        };
        let protocol_id = entity_id.get();
        let event = ElderEncounterEventV1 {
            event_id: None,
            zone_name: ev.zone_name.clone(),
            elder_entity_id: protocol_id, // MC protocol entity_id（非 ECS index）
            event_kind: ElderEncounterEventKindV1::Appeared,
            betray_probability: ev.blackboard.betray_probability,
            dan_count: 0,
            offered_skill_id: ev.blackboard.offered_skill_id.to_string(),
            qi_fraction: 1.0_f32,
            server_tick: ev.tick,
        };
        let Some(bytes) = to_json_bytes(&event) else {
            continue;
        };
        let sent = send_to_players_in_zone(&mut players, &ev.zone_name, &zones, &bytes);
        if sent == 0 {
            // The player can enter the selected TSY layer immediately after the spawn tick.
            // Keep the event until a same-zone client exists instead of silently dropping it.
            pending.events.push(ev.clone());
        }
        if sent > 0 {
            tracing::info!(
                "[bong][elder_encounter_emit] S2C appeared → protocol_id={} zone='{}' players={} betray_prob={:.3} tick={}",
                protocol_id,
                ev.zone_name,
                sent,
                ev.blackboard.betray_probability,
                ev.tick,
            );
        }
    }
}

// ── S2C dan_received 系统 ─────────────────────────────────────────────────────

/// plan-dying-elder-v1 B1 — 玩家给丹后向同 zone 玩家推送 `bong:elder_encounter` dan_received 事件。
///
/// 在 `dying_elder_give_dan_system` 之后运行，只消费权威 accepted 快照。
#[allow(clippy::type_complexity)]
pub(crate) fn elder_encounter_s2c_dan_received_system(
    mut accepted_events: EventReader<DyingElderDanAcceptedEvent>,
    elders: DyingElderQuery<'_, '_>,
    mut players: PlayerQuery<'_, '_>,
    zones: Option<Res<ZoneRegistry>>,
    game_tick: Option<Res<GameTick>>,
) {
    let Some(zones) = zones else { return };
    let tick = game_tick.as_deref().map(|t| t.0 as u64).unwrap_or(0);

    for accepted in accepted_events.read() {
        let Ok((_entity, entity_id, bb, _state)) = elders.get(accepted.elder) else {
            continue;
        };

        let event = ElderEncounterEventV1 {
            event_id: None,
            zone_name: bb.home_zone.clone(),
            elder_entity_id: entity_id.get(), // MC protocol entity_id（非 ECS index）
            event_kind: ElderEncounterEventKindV1::DanReceived,
            betray_probability: 0.0,
            dan_count: accepted.dan_count,
            offered_skill_id: bb.offered_skill_id.to_string(),
            qi_fraction: accepted.qi_fraction,
            server_tick: tick,
        };
        let Some(bytes) = to_json_bytes(&event) else {
            continue;
        };
        send_to_players_in_zone(&mut players, &bb.home_zone, &zones, &bytes);
    }
}

// ── S2C death 系统 ────────────────────────────────────────────────────────────

/// plan-dying-elder-v1 B1 — 大能死亡时向同 zone 玩家推送 `bong:elder_encounter` 死亡事件。
///
/// 在 `dying_elder_death_system`（含 `DyingElderDeathProcessed` 标记）之前运行，
/// 检测本 tick 刚进入 Dead 状态且尚未推送死亡通知的大能。
#[allow(clippy::type_complexity)]
pub(crate) fn elder_encounter_s2c_death_system(
    mut commands: Commands,
    mut settlements: EventReader<NpcTerminalSettlementSucceeded>,
    elders: Query<
        (&EntityId, &DyingElderBlackboard, &DyingElderState),
        (
            With<NpcMarker>,
            Without<ClientMarker>,
            Without<DyingElderDeathS2cBroadcast>,
        ),
    >,
    mut players: PlayerQuery<'_, '_>,
    zones: Option<Res<ZoneRegistry>>,
) {
    let Some(zones) = zones else { return };

    for settlement in settlements.read() {
        let Ok((entity_id, bb, state)) = elders.get(settlement.entity) else {
            continue;
        };
        let dead_by_betrayal = settlement.reason == NpcDeathReason::DuoShe
            || matches!(
                *state,
                DyingElderState::Dead {
                    dead_by_betrayal: true
                }
            )
            || settlement.cause == "dying_elder_betrayal";

        let event_kind = if dead_by_betrayal {
            ElderEncounterEventKindV1::Betrayal
        } else {
            ElderEncounterEventKindV1::DeadNatural
        };

        let event = ElderEncounterEventV1 {
            event_id: None,
            zone_name: bb.home_zone.clone(),
            elder_entity_id: entity_id.get(), // MC protocol entity_id（非 ECS index）
            event_kind,
            betray_probability: 0.0,
            dan_count: 0,
            offered_skill_id: String::new(),
            qi_fraction: 0.0,
            server_tick: settlement.at_tick,
        };
        let Some(bytes) = to_json_bytes(&event) else {
            continue;
        };
        send_to_players_in_zone(&mut players, &bb.home_zone, &zones, &bytes);
        commands
            .entity(settlement.entity)
            .insert(DyingElderDeathS2cBroadcast);
        tracing::info!(
            "[bong][elder_encounter_emit] S2C death → entity={:?} protocol_id={} zone='{}' kind={:?} tick={}",
            settlement.entity,
            entity_id.get(),
            bb.home_zone,
            event_kind,
            settlement.at_tick,
        );
    }
}

// ── Bevy 注册 ─────────────────────────────────────────────────────────────────

/// 注册 S2C elder_encounter 推送系统。
///
/// - `elder_encounter_s2c_appear_system`：与 P3 appear event 同步运行
/// - `elder_encounter_s2c_dan_received_system`：在 `give_dan_system` 之后运行
/// - `elder_encounter_s2c_death_system`：收丹反馈与状态生产者之后、`death_system` 之前运行
pub fn register(app: &mut App) {
    app.init_resource::<PendingElderAppearS2c>();
    app.add_systems(
        valence::prelude::PostUpdate,
        elder_encounter_s2c_appear_system.after(valence::entity::InitEntitiesSet),
    );
    app.add_systems(
        Update,
        (
            elder_encounter_s2c_dan_received_system
                .after(crate::fauna::dying_elder::dying_elder_give_dan_system),
            elder_encounter_s2c_death_system
                .in_set(NpcTerminalSystemSet::PostCommit)
                .after(elder_encounter_s2c_dan_received_system),
        ),
    );
}

// ── 单元测试 ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::common::MAX_PAYLOAD_BYTES;
    use crate::schema::elder_encounter::ElderEncounterEventV1;

    fn flush_clients_and_collect_elder_payloads(
        app: &mut valence::prelude::App,
        helper: &mut valence::testing::MockClientHelper,
    ) -> Vec<ElderEncounterEventV1> {
        use valence::prelude::Client;
        use valence::protocol::packets::play::CustomPayloadS2c;

        let world = app.world_mut();
        let mut clients = world.query::<&mut Client>();
        for mut client in clients.iter_mut(world) {
            client
                .flush_packets()
                .expect("mock client packets should flush");
        }
        helper
            .collect_received()
            .0
            .into_iter()
            .filter_map(|frame| {
                let packet = frame.decode::<CustomPayloadS2c>().ok()?;
                (packet.channel.as_str() == CH_ELDER_ENCOUNTER).then(|| {
                    serde_json::from_slice::<ElderEncounterEventV1>(packet.data.0 .0)
                        .expect("elder encounter payload should decode")
                })
            })
            .collect()
    }

    // ── payload JSON 结构 pin 测试 ──────────────────────────────────────────

    #[test]
    fn appeared_payload_has_correct_json_structure() {
        // 期望：appeared 事件序列化包含所有 client handler 需要的字段
        let event = ElderEncounterEventV1 {
            event_id: None,
            zone_name: "tsy_deep".to_string(),
            elder_entity_id: 1, // MC protocol entity_id（最小合法值=1；Valence 从 1 起分配）
            event_kind: ElderEncounterEventKindV1::Appeared,
            betray_probability: 0.65,
            dan_count: 0,
            offered_skill_id: "woliu.heart".to_string(),
            qi_fraction: 1.0,
            server_tick: 1000,
        };
        let json = serde_json::to_string(&event).expect("serialize appeared event");

        assert!(
            json.contains("\"zone_name\""),
            "expected appeared payload to contain 'zone_name' field because client handler reads this, actual: {json}"
        );
        assert!(
            json.contains("\"event_kind\""),
            "expected appeared payload to contain 'event_kind' field because client handler routes on this, actual: {json}"
        );
        assert!(
            json.contains("\"appeared\""),
            "expected appeared payload event_kind to serialize as 'appeared', actual: {json}"
        );
        assert!(
            json.contains("\"betray_probability\""),
            "expected appeared payload to contain 'betray_probability' for client HUD display, actual: {json}"
        );
        assert!(
            json.contains("\"qi_fraction\""),
            "expected appeared payload to contain 'qi_fraction' (M2 fix: real qi display), actual: {json}"
        );
        assert!(
            json.contains("\"server_tick\""),
            "expected appeared payload to contain 'server_tick' for audit, actual: {json}"
        );
    }

    #[test]
    fn death_payload_event_kind_routes_correctly() {
        // 期望：dead_by_betrayal=true → Betrayal；false → DeadNatural
        let betrayal = ElderEncounterEventKindV1::Betrayal;
        let natural = ElderEncounterEventKindV1::DeadNatural;

        let json_b = serde_json::to_string(&betrayal).expect("serialize");
        let json_n = serde_json::to_string(&natural).expect("serialize");

        assert!(
            json_b.contains("betrayal"),
            "expected Betrayal kind to serialize as 'betrayal' for client routing, actual: {json_b}"
        );
        assert!(
            json_n.contains("dead_natural"),
            "expected DeadNatural kind to serialize as 'dead_natural' for client routing, actual: {json_n}"
        );
    }

    #[test]
    fn death_s2c_broadcasts_once_while_settlement_remains_unprocessed() {
        use valence::entity::EntityId;
        use valence::prelude::{App, DVec3, Position, Update};
        use valence::testing::create_mock_client;

        let mut app = App::new();
        app.add_event::<NpcTerminalSettlementSucceeded>();
        app.add_systems(Update, elder_encounter_s2c_death_system);
        let mut zones = ZoneRegistry::fallback();
        zones.zones[0].name = "tsy_deep".to_string();
        app.insert_resource(zones);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        app.world_mut()
            .spawn(client_bundle)
            .insert(Position::new([0.0, 64.0, 0.0]))
            .insert(CurrentDimension(DimensionKind::Overworld));

        let elder = app
            .world_mut()
            .spawn((
                NpcMarker,
                EntityId::default(),
                DyingElderBlackboard::new("tsy_deep", DVec3::ZERO, 7, 0),
                DyingElderState::Dead {
                    dead_by_betrayal: false,
                },
            ))
            .id();

        app.update();
        app.update();
        flush_clients_and_collect_elder_payloads(&mut app, &mut helper);
        assert!(
            !app.world()
                .entity(elder)
                .contains::<DyingElderDeathS2cBroadcast>(),
            "commit 前不得推送死亡 S2C 或插入幂等 marker"
        );

        app.world_mut().send_event(NpcTerminalSettlementSucceeded {
            entity: elder,
            at_tick: 77,
            cause: "dying_elder_death".to_string(),
            reason: NpcDeathReason::NaturalAging,
            attacker: None,
            attacker_player_id: None,
            authorize_loot: true,
            actor_qi_identity: crate::cultivation::components::ActorQiIdentity::from_life_record(
                &crate::cultivation::life_record::LifeRecord::new("npc:elder-s2c"),
                crate::cultivation::components::ActorQiKind::Npc,
            )
            .expect("fixture identity must be canonical"),
        });
        app.update();
        app.update();

        let payloads = flush_clients_and_collect_elder_payloads(&mut app, &mut helper);

        assert_eq!(payloads.len(), 1, "commit 后跨 tick 只能推送一次 S2C");
        assert_eq!(
            payloads[0].event_kind,
            ElderEncounterEventKindV1::DeadNatural
        );
        assert_eq!(payloads[0].server_tick, 77);
        let elder_ref = app.world().entity(elder);
        assert!(elder_ref.contains::<DyingElderDeathS2cBroadcast>());
    }

    #[test]
    fn fifth_dan_s2c_feedback_orders_terminal_event_last() {
        use valence::entity::EntityId;
        use valence::prelude::{App, Client, DVec3, Position};
        use valence::protocol::packets::play::CustomPayloadS2c;
        use valence::testing::create_mock_client;

        let mut app = App::new();
        app.add_event::<DyingElderAppearedEvent>();
        app.add_event::<DyingElderDanAcceptedEvent>();
        app.add_event::<NpcTerminalSettlementSucceeded>();
        let mut zones = ZoneRegistry::fallback();
        zones.zones[0].name = "tsy_deep".to_string();
        app.insert_resource(zones);
        register(&mut app);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        app.world_mut()
            .spawn(client_bundle)
            .insert(Position::new([0.0, 64.0, 0.0]))
            .insert(CurrentDimension(DimensionKind::Overworld));
        let player = app.world_mut().spawn_empty().id();
        let elder = app
            .world_mut()
            .spawn((
                NpcMarker,
                EntityId::default(),
                DyingElderBlackboard::new("tsy_deep", DVec3::ZERO, 7, 0),
                DyingElderState::Dead {
                    dead_by_betrayal: false,
                },
            ))
            .id();
        app.world_mut().send_event(DyingElderDanAcceptedEvent {
            player,
            elder,
            pill_instance_id: 5,
            qi_gain: 60.0,
            dan_count: crate::fauna::dying_elder::DYING_ELDER_DAN_THRESHOLD,
            qi_fraction: 1.0,
        });
        app.world_mut().send_event(NpcTerminalSettlementSucceeded {
            entity: elder,
            at_tick: 88,
            cause: "dying_elder_death".to_string(),
            reason: NpcDeathReason::NaturalAging,
            attacker: None,
            attacker_player_id: None,
            authorize_loot: true,
            actor_qi_identity: crate::cultivation::components::ActorQiIdentity::from_life_record(
                &crate::cultivation::life_record::LifeRecord::new("npc:elder-fifth-dan"),
                crate::cultivation::components::ActorQiKind::Npc,
            )
            .expect("fixture identity must be canonical"),
        });

        app.update();
        let world = app.world_mut();
        let mut clients = world.query::<&mut Client>();
        for mut client in clients.iter_mut(world) {
            client
                .flush_packets()
                .expect("mock client packets should flush");
        }

        let kinds = helper
            .collect_received()
            .0
            .into_iter()
            .filter_map(|frame| {
                let packet = frame.decode::<CustomPayloadS2c>().ok()?;
                if packet.channel.as_str() != CH_ELDER_ENCOUNTER {
                    return None;
                }
                Some(
                    serde_json::from_slice::<ElderEncounterEventV1>(packet.data.0 .0)
                        .expect("elder feedback payload should decode")
                        .event_kind,
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            kinds,
            vec![
                ElderEncounterEventKindV1::DanReceived,
                ElderEncounterEventKindV1::DeadNatural,
            ],
            "第五颗丹同帧 S2C 必须先收丹、后终态，确保 HUD 最终保持关闭"
        );
    }

    #[test]
    fn qi_fraction_zero_for_death_events() {
        // 期望：死亡事件 qi_fraction = 0.0（真元耗尽）
        let event = ElderEncounterEventV1 {
            event_id: None,
            zone_name: "tsy_deep".to_string(),
            elder_entity_id: 5,
            event_kind: ElderEncounterEventKindV1::DeadNatural,
            betray_probability: 0.0,
            dan_count: 0,
            offered_skill_id: String::new(),
            qi_fraction: 0.0,
            server_tick: 999,
        };
        assert_eq!(
            event.qi_fraction, 0.0,
            "expected qi_fraction=0.0 for death events because elder's qi is exhausted, actual: {}",
            event.qi_fraction
        );
        let json = serde_json::to_string(&event).expect("serialize");
        let back: ElderEncounterEventV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            back.qi_fraction, 0.0,
            "expected qi_fraction=0.0 to survive serde for death event, actual: {}",
            back.qi_fraction
        );
    }

    #[test]
    fn qi_fraction_computation_from_bb_values() {
        // 期望：qi_fraction = qi_current / qi_max_cache，clamp [0.0, 1.0]
        // 正常：qi_current=300, qi_max_cache=500 → fraction=0.6
        let qi_current = 300.0_f64;
        let qi_max_cache = 500.0_f64;
        let fraction = (qi_current / qi_max_cache).clamp(0.0, 1.0) as f32;
        assert!(
            (fraction - 0.6_f32).abs() < 1e-5,
            "expected qi_fraction=0.6 when qi_current=300/qi_max=500, actual: {fraction}"
        );

        // 超量（给丹后 > qi_max_cache）：clamp 到 1.0
        let qi_over = 600.0_f64;
        let fraction_over = (qi_over / qi_max_cache).clamp(0.0, 1.0) as f32;
        assert_eq!(
            fraction_over, 1.0,
            "expected qi_fraction clamped to 1.0 when qi > qi_max_cache, actual: {fraction_over}"
        );

        // qi_max_cache = 0（防止除零）
        let qi_zero_max = 0.0_f64;
        let fraction_zero = if qi_zero_max > 0.0 {
            (qi_current / qi_zero_max).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
        assert_eq!(
            fraction_zero, 0.0,
            "expected qi_fraction=0.0 when qi_max_cache=0 (zero-division guard), actual: {fraction_zero}"
        );
    }

    #[test]
    fn dan_received_payload_has_qi_fraction_and_dan_count() {
        // 期望：dan_received 事件包含正确 qi_fraction 和 dan_count
        let event = ElderEncounterEventV1 {
            event_id: None,
            zone_name: "tsy_abyss".to_string(),
            elder_entity_id: 42,
            event_kind: ElderEncounterEventKindV1::DanReceived,
            betray_probability: 0.0,
            dan_count: 3,
            offered_skill_id: "anqi.echo_fractal".to_string(),
            qi_fraction: 0.7,
            server_tick: 5000,
        };
        let json = serde_json::to_string(&event).expect("serialize");
        assert!(
            json.contains("\"dan_received\""),
            "expected dan_received payload event_kind to serialize as 'dan_received', actual: {json}"
        );
        assert!(
            json.contains("\"qi_fraction\""),
            "expected dan_received payload to include qi_fraction for client HUD progress bar, actual: {json}"
        );
        let back: ElderEncounterEventV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            back.dan_count, 3,
            "expected dan_count=3 to survive serde, actual: {}",
            back.dan_count
        );
        assert!(
            (back.qi_fraction - 0.7).abs() < 1e-5,
            "expected qi_fraction=0.7 to survive serde, actual: {}",
            back.qi_fraction
        );
    }

    #[test]
    fn channel_identifier_matches_client_handler() {
        // ident! 需要字面量；这里 pin schema 常量与实际发送 channel 保持完全一致。
        assert_eq!(
            CH_ELDER_ENCOUNTER, "bong:elder_encounter",
            "expected elder encounter channel to exactly match client handler identifier"
        );
    }

    fn elder_encounter_event_with_skill_len(skill_len: usize) -> ElderEncounterEventV1 {
        ElderEncounterEventV1 {
            event_id: None,
            zone_name: "spawn".to_string(),
            elder_entity_id: 1,
            event_kind: ElderEncounterEventKindV1::Appeared,
            betray_probability: 0.65,
            dan_count: 0,
            offered_skill_id: "x".repeat(skill_len),
            qi_fraction: 1.0,
            server_tick: 1000,
        }
    }

    fn encoded_elder_encounter_len(skill_len: usize) -> usize {
        serde_json::to_vec(&elder_encounter_event_with_skill_len(skill_len))
            .expect("test elder encounter payload should serialize")
            .len()
    }

    fn skill_len_for_payload_size(target_size: usize) -> usize {
        let mut lo = 0usize;
        let mut hi = target_size;
        while lo < hi {
            let mid = (lo + hi).div_ceil(2);
            if encoded_elder_encounter_len(mid) <= target_size {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        assert_eq!(
            encoded_elder_encounter_len(lo),
            target_size,
            "测试夹具应能构造 exactly target_size 的 elder_encounter payload"
        );
        lo
    }

    #[test]
    fn max_sized_elder_encounter_payload_is_accepted() {
        let skill_len = skill_len_for_payload_size(MAX_PAYLOAD_BYTES);
        let event = elder_encounter_event_with_skill_len(skill_len);
        let bytes = to_json_bytes(&event)
            .expect("刚好等于 MAX_PAYLOAD_BYTES 的 elder_encounter 应允许下发");

        assert_eq!(
            bytes.len(),
            MAX_PAYLOAD_BYTES,
            "测试前置条件失败：payload 应刚好等于 MAX_PAYLOAD_BYTES"
        );
    }

    #[test]
    fn oversized_elder_encounter_payload_is_rejected() {
        let event = elder_encounter_event_with_skill_len(MAX_PAYLOAD_BYTES);

        let encoded_len = serde_json::to_vec(&event)
            .expect("test elder encounter payload should serialize")
            .len();
        assert!(
            encoded_len > MAX_PAYLOAD_BYTES,
            "测试前置条件失败：payload 必须超出 MAX_PAYLOAD_BYTES；actual={encoded_len}, max={MAX_PAYLOAD_BYTES}"
        );
        assert!(
            to_json_bytes(&event).is_none(),
            "超出 MAX_PAYLOAD_BYTES 的 elder_encounter 不应生成可下发 bytes；actual_len={encoded_len}, max={MAX_PAYLOAD_BYTES}"
        );
    }

    #[test]
    fn max_plus_one_elder_encounter_payload_is_rejected() {
        let skill_len = skill_len_for_payload_size(MAX_PAYLOAD_BYTES + 1);
        let event = elder_encounter_event_with_skill_len(skill_len);

        let encoded_len = serde_json::to_vec(&event)
            .expect("test elder encounter payload should serialize")
            .len();
        assert_eq!(
            encoded_len,
            MAX_PAYLOAD_BYTES + 1,
            "测试前置条件失败：payload 应刚好等于 MAX_PAYLOAD_BYTES + 1"
        );
        assert!(
            to_json_bytes(&event).is_none(),
            "刚好超出 1 字节的 elder_encounter 不应生成可下发 bytes；actual_len={encoded_len}, max={MAX_PAYLOAD_BYTES}"
        );
    }
}
