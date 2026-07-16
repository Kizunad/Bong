//! 玩家加入时一次性推送 forge 快照（plan-forge-v1 §4 vertical slice）。
//!
//! 从真实 ECS `BlueprintRegistry` / `WeaponForgeStation` / `ForgeSessions`
//! 读取数据构建 snapshot（非 mock）。

#![allow(dead_code)]

use valence::prelude::{Added, Client, Entity, EventReader, Query, Res, Username, With};

use crate::forge::blueprint::{Blueprint, BlueprintRegistry, StepSpec};
use crate::forge::events::{
    ConsecrationInject, ForgeBucket, ForgeOutcomeEvent, ForgeStartAccepted,
    InscriptionScrollApplied, StepAdvance, TemperingHit,
};
use crate::forge::learned::LearnedBlueprints;
use crate::forge::session::{ForgeSession, ForgeSessionId, ForgeSessions, ForgeStep, StepState};
use crate::forge::station::WeaponForgeStation;
use crate::inventory::PlayerInventory;
use crate::network::send_server_data_payload;
use crate::schema::forge::{
    ForgeBlueprintBookDataV1, ForgeBlueprintEntryV1, ForgeSessionDataV1, ForgeStepStateDataV1,
    ForgeStepV1, WeaponForgeStationDataV1,
};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
use crate::skill::components::SkillSet;

type JoinedClientQueryItem<'a> = (Entity, &'a mut Client, &'a Username);

pub fn emit_join_forge_snapshots(
    #[allow(unused)] mut joined_clients: Query<
        JoinedClientQueryItem<'_>,
        (With<Client>, Added<PlayerInventory>),
    >,
    _registry: Res<BlueprintRegistry>,
    _stations: Query<&WeaponForgeStation>,
    _sessions: Res<ForgeSessions>,
    _learned_q: Query<&LearnedBlueprints>,
    _caster_names: Query<&Username>,
    _skill_q: Query<&SkillSet>,
) {
    // join hydration placeholder — real snapshots sent via send_forge_snapshots_to_player
    // when the player opens the forge screen.
}

/// Send forge snapshots for a specific player when they open a forge screen.
pub fn send_forge_snapshots_to_player(
    client: &mut Client,
    station: &WeaponForgeStation,
    owner_name: &str,
    session: Option<(&ForgeSession, &str)>, // (session, blueprint_name)
    learned: Option<(&LearnedBlueprints, &BlueprintRegistry)>,
) {
    // ── station ──
    send_station_snapshot_to_player(client, station, owner_name);

    // ── session ──
    if let Some((session, bp_name)) = session {
        let blueprint = learned.and_then(|(_, registry)| registry.get(session.blueprint.as_str()));
        send_session_snapshot_to_player(client, session, bp_name, blueprint);
    }

    // ── blueprint book ──
    if let Some((lb, registry)) = learned {
        send_blueprint_book_to_player(client, lb, registry);
    }
}

/// plan-forge-session-entry-wiring-v1 §4.1#3/P2 — 放砧成功后的专属回执（对齐 alchemy
/// `open_furnace` 模式：重要动作必有回执，之前只有 inventory 扣除能间接推断放砧成功）。
/// 也被 `send_forge_snapshots_to_player` 复用为其 station 分支。
pub fn send_station_snapshot_to_player(
    client: &mut Client,
    station: &WeaponForgeStation,
    owner_name: &str,
) {
    let payload = ServerDataV1::new(ServerDataPayloadV1::ForgeStation(Box::new(
        build_station_data(station, owner_name),
    )));
    let Ok(bytes) = crate::network::agent_bridge::serialize_server_data_payload(&payload) else {
        return;
    };
    send_server_data_payload(client, bytes.as_slice());
}

/// P2 — 单独推 session 快照（不附带 station/blueprint book），供淬炼击键 / 铭文 / 开光
/// 等单步交互事件后回推实时进度，避免每次交互都重发完整三件套。
pub fn send_session_snapshot_to_player(
    client: &mut Client,
    session: &ForgeSession,
    bp_name: &str,
    blueprint: Option<&Blueprint>,
) {
    let payload = ServerDataV1::new(ServerDataPayloadV1::ForgeSession(Box::new(
        build_session_data(session, bp_name, blueprint),
    )));
    let Ok(bytes) = crate::network::agent_bridge::serialize_server_data_payload(&payload) else {
        return;
    };
    send_server_data_payload(client, bytes.as_slice());
}

/// 锻造结算后推 outcome payload 给对应 player。
pub fn send_forge_outcome_to_player(
    client: &mut Client,
    outcome: &crate::forge::events::ForgeOutcomeEvent,
    flawed_path: bool,
) {
    use crate::schema::forge::{ForgeOutcomeBucketV1, ForgeOutcomeDataV1};
    let data = ForgeOutcomeDataV1 {
        session_id: outcome.session.0,
        blueprint_id: outcome.blueprint.clone(),
        bucket: ForgeOutcomeBucketV1::from(outcome.bucket),
        weapon_item: outcome.weapon_item.clone(),
        quality: outcome.quality,
        color: outcome.color,
        side_effects: outcome.side_effects.clone(),
        achieved_tier: outcome.achieved_tier as u32,
        flawed_path,
    };
    let payload = ServerDataV1::new(ServerDataPayloadV1::ForgeOutcome(Box::new(data)));
    let Ok(bytes) = crate::network::agent_bridge::serialize_server_data_payload(&payload) else {
        return;
    };
    send_server_data_payload(client, bytes.as_slice());
}

/// plan-forge-session-entry-wiring-v1 §4.1#2 — 翻页（server 权威）后把新页码回推给
/// 发起翻页的 client。只推 blueprint book 这一个 payload（不像 `send_forge_snapshots_to_player`
/// 那样一并推 station/session——翻页时未必站在砧前）。
pub fn send_blueprint_book_to_player(
    client: &mut Client,
    learned: &LearnedBlueprints,
    registry: &BlueprintRegistry,
) {
    let payload = ServerDataV1::new(ServerDataPayloadV1::ForgeBlueprintBook(Box::new(
        build_blueprint_book(learned, registry),
    )));
    let Ok(bytes) = crate::network::agent_bridge::serialize_server_data_payload(&payload) else {
        return;
    };
    send_server_data_payload(client, bytes.as_slice());
}

// ══════════════════════════ P2 — S2C 回执真实调用点 ══════════════════════════
//
// plan-forge-session-entry-wiring-v1 P2：起炉受理 / 单步交互 / 结算全部曾经零 S2C
// 回执（引擎只改状态，client 无从得知）。以下 4 个系统把上面已实装的 send_* 函数
// 接上生产事件源，全部注册在 `forge::register`（forge/mod.rs），排在对应引擎处理
// 系统之后（`.after(...)`），保证读到的是处理完毕的最新状态。

/// 起炉受理（`ForgeStartAccepted`）后，把 station + session + blueprint_book 三件套
/// 推给起炉的 caster——对齐 alchemy `open_furnace` 一次性回执模式。
/// `send_forge_snapshots_to_player` 在此获得第一个真实生产调用点。
pub fn push_forge_start_snapshot_on_accept(
    mut ev: EventReader<ForgeStartAccepted>,
    sessions: Res<ForgeSessions>,
    registry: Res<BlueprintRegistry>,
    stations: Query<&WeaponForgeStation>,
    learned_q: Query<&LearnedBlueprints>,
    mut clients: Query<(&Username, &mut Client)>,
) {
    for accepted in ev.read() {
        let Ok(station) = stations.get(accepted.station) else {
            tracing::warn!(
                "[bong][network][forge] start snapshot skipped: station={:?} missing",
                accepted.station
            );
            continue;
        };
        let Ok((username, mut client)) = clients.get_mut(accepted.caster) else {
            continue;
        };
        let owner_name = username.0.clone();
        let session_and_name = sessions.get(accepted.session).map(|session| {
            let bp_name = registry
                .get(session.blueprint.as_str())
                .map(|bp| bp.name.as_str())
                .unwrap_or(session.blueprint.as_str());
            (session, bp_name)
        });
        let learned_and_registry = learned_q
            .get(accepted.caster)
            .ok()
            .map(|learned| (learned, &*registry));
        send_forge_snapshots_to_player(
            &mut client,
            station,
            owner_name.as_str(),
            session_and_name,
            learned_and_registry,
        );
    }
}

/// 淬炼击键 / 铭文投入 / 开光注真元——每次单步交互后回推最新 session 快照，
/// 让 `ForgeScreen` 实时反映进度（§4.1 决议原文「每步事件后→推更新的 session 快照」）。
/// 只推 session，不重发 station/blueprint_book（避免每次击键都重发三件套）。
pub fn push_forge_session_snapshot_on_interaction(
    mut tempering_hits: EventReader<TemperingHit>,
    mut scroll_applied: EventReader<InscriptionScrollApplied>,
    mut consecration_injects: EventReader<ConsecrationInject>,
    sessions: Res<ForgeSessions>,
    registry: Res<BlueprintRegistry>,
    mut clients: Query<&mut Client>,
) {
    let mut touched: Vec<ForgeSessionId> = Vec::new();
    for hit in tempering_hits.read() {
        touched.push(hit.session);
    }
    for applied in scroll_applied.read() {
        touched.push(applied.session);
    }
    for inject in consecration_injects.read() {
        touched.push(inject.session);
    }
    for session_id in touched {
        let Some(session) = sessions.get(session_id) else {
            continue;
        };
        let Ok(mut client) = clients.get_mut(session.caster) else {
            continue;
        };
        let bp_name = registry
            .get(session.blueprint.as_str())
            .map(|bp| bp.name.as_str())
            .unwrap_or(session.blueprint.as_str());
        let blueprint = registry.get(session.blueprint.as_str());
        send_session_snapshot_to_player(&mut client, session, bp_name, blueprint);
    }
}

/// `ForgeStepAdvance` 处理后（forge step advance 阶段之后）回推最新快照：未完成则和
/// 起炉受理一样推 station+session+blueprint_book 三件套（`send_forge_snapshots_to_player`
/// 的第二个真实调用点）；已到 Done 则只推 station（`has_session` 已被引擎清 false）+
/// blueprint_book，不带 session（结算内容由 `push_forge_outcome_on_event` 的
/// `forge_outcome` payload 承载，避免重复/过期的 session 快照）。
pub fn push_forge_session_snapshot_on_step_advance(
    mut ev: EventReader<StepAdvance>,
    sessions: Res<ForgeSessions>,
    registry: Res<BlueprintRegistry>,
    stations: Query<&WeaponForgeStation>,
    learned_q: Query<&LearnedBlueprints>,
    mut clients: Query<(&Username, &mut Client)>,
) {
    for advance in ev.read() {
        let Some(session) = sessions.get(advance.session) else {
            continue;
        };
        let Ok(station) = stations.get(session.station) else {
            continue;
        };
        let Ok((username, mut client)) = clients.get_mut(session.caster) else {
            continue;
        };
        let owner_name = username.0.clone();
        let session_and_name = (!session.is_done()).then(|| {
            let bp_name = registry
                .get(session.blueprint.as_str())
                .map(|bp| bp.name.as_str())
                .unwrap_or(session.blueprint.as_str());
            (session, bp_name)
        });
        let learned_and_registry = learned_q
            .get(session.caster)
            .ok()
            .map(|learned| (learned, &*registry));
        send_forge_snapshots_to_player(
            &mut client,
            station,
            owner_name.as_str(),
            session_and_name,
            learned_and_registry,
        );
    }
}

/// 锻造结算（`ForgeOutcomeEvent`，涵盖正常收尾 + billet-Waste 早退两条路径）后推
/// outcome payload——`send_forge_outcome_to_player` 在此获得真实生产调用点。
/// `flawed_path` 对齐 `ForgeBucket::Flawed`（走了 `flawed_fallback` 残缺匹配路径）。
pub fn push_forge_outcome_on_event(
    mut ev: EventReader<ForgeOutcomeEvent>,
    mut clients: Query<&mut Client>,
) {
    for outcome in ev.read() {
        let Ok(mut client) = clients.get_mut(outcome.caster) else {
            continue;
        };
        let flawed_path = matches!(outcome.bucket, ForgeBucket::Flawed);
        send_forge_outcome_to_player(&mut client, outcome, flawed_path);
    }
}

fn build_station_data(station: &WeaponForgeStation, owner_name: &str) -> WeaponForgeStationDataV1 {
    WeaponForgeStationDataV1 {
        station_id: format!("forge_station_{}", owner_name),
        tier: station.tier,
        integrity: station.integrity,
        owner_name: owner_name.to_string(),
        has_session: station.session.is_some(),
        // plan-forge-session-entry-wiring-v1 §4.1#3 — 正常放砧路径 pos 恒 Some
        // （station::handle_place_station_request 经 `WeaponForgeStation::placed` 构造）；
        // 无 pos 只可能出现在测试 fixture，defensive 落 (0,0,0)。
        station_pos_x: station.pos.map(|p| p.0).unwrap_or(0),
        station_pos_y: station.pos.map(|p| p.1).unwrap_or(0),
        station_pos_z: station.pos.map(|p| p.2).unwrap_or(0),
    }
}

fn build_session_data(
    session: &ForgeSession,
    bp_name: &str,
    blueprint: Option<&Blueprint>,
) -> ForgeSessionDataV1 {
    ForgeSessionDataV1 {
        session_id: session.id.0,
        blueprint_id: session.blueprint.clone(),
        blueprint_name: bp_name.to_string(),
        active: !session.is_done(),
        current_step: forge_step_to_v1(session.current_step),
        step_index: session.step_index as u32,
        achieved_tier: session.achieved_tier as u32,
        step_state: build_step_state(session, blueprint),
    }
}

fn forge_step_to_v1(step: ForgeStep) -> ForgeStepV1 {
    match step {
        ForgeStep::Billet => ForgeStepV1::Billet,
        ForgeStep::Tempering => ForgeStepV1::Tempering,
        ForgeStep::Inscription => ForgeStepV1::Inscription,
        ForgeStep::Consecration => ForgeStepV1::Consecration,
        ForgeStep::Done => ForgeStepV1::Done,
    }
}

fn build_step_state(session: &ForgeSession, blueprint: Option<&Blueprint>) -> ForgeStepStateDataV1 {
    let step_spec = blueprint.and_then(|bp| bp.steps.get(session.step_index));
    match &session.step_state {
        StepState::Billet(state) => ForgeStepStateDataV1::Billet {
            materials_in: state
                .materials_in
                .iter()
                .map(|(k, v)| (k.clone(), *v))
                .collect(),
            active_carrier: state.active_carrier.clone(),
            resolved_tier_cap: state.resolved_tier_cap as u32,
        },
        StepState::Tempering(state) => ForgeStepStateDataV1::Tempering {
            pattern: match step_spec {
                Some(StepSpec::Tempering { profile }) => profile
                    .pattern
                    .iter()
                    .copied()
                    .map(crate::schema::forge::TemperBeatV1::from)
                    .collect(),
                _ => vec![],
            },
            beat_cursor: state.beat_cursor as u32,
            hits: state.hits,
            misses: state.misses,
            deviation: state.deviation,
            qi_spent: state.qi_spent,
        },
        StepState::Inscription(state) => ForgeStepStateDataV1::Inscription {
            filled_slots: state.filled_slots as u32,
            max_slots: match step_spec {
                Some(StepSpec::Inscription { profile }) => profile.slots as u32,
                _ => state.filled_slots as u32,
            },
            failed: state.failed,
        },
        StepState::Consecration(state) => ForgeStepStateDataV1::Consecration {
            qi_injected: state.qi_injected,
            qi_required: state.qi_required,
            color_imprint: state.color_imprint,
            min_realm: match step_spec {
                Some(StepSpec::Consecration { profile }) => Some(profile.min_realm),
                _ => None,
            },
        },
        StepState::None => ForgeStepStateDataV1::None,
    }
}

fn build_blueprint_book(
    learned: &LearnedBlueprints,
    registry: &BlueprintRegistry,
) -> ForgeBlueprintBookDataV1 {
    let entries: Vec<ForgeBlueprintEntryV1> = learned
        .ids
        .iter()
        .filter_map(|id| {
            registry.get(id).map(|bp| ForgeBlueprintEntryV1 {
                id: bp.id.clone(),
                display_name: bp.name.clone(),
                tier_cap: bp.tier_cap,
                step_count: bp.steps.len() as u32,
            })
        })
        .collect();
    ForgeBlueprintBookDataV1 {
        learned: entries,
        current_index: learned.current_index as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::Realm;
    use crate::forge::blueprint::{StepKind, TemperBeat};
    use crate::forge::learned::LearnedBlueprints;
    use crate::forge::session::{
        ConsecrationState, ForgeSessionId, InscriptionState, TemperingState,
    };
    use crate::network::agent_bridge::SERVER_DATA_CHANNEL;
    use valence::prelude::{App, BlockPos, Entity, Update};
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    fn qing_feng() -> Blueprint {
        serde_json::from_str(include_str!(
            "../../assets/forge/blueprints/qing_feng_v0.json"
        ))
        .expect("qing_feng_v0 blueprint should parse")
    }

    fn ling_feng() -> Blueprint {
        serde_json::from_str(include_str!(
            "../../assets/forge/blueprints/ling_feng_v0.json"
        ))
        .expect("ling_feng_v0 blueprint should parse")
    }

    fn session_at(blueprint: &Blueprint, kind: StepKind, state: StepState) -> ForgeSession {
        let step_index = blueprint
            .step_index(kind)
            .expect("blueprint should contain requested step");
        let mut session = ForgeSession::new(
            ForgeSessionId(7),
            blueprint.id.clone(),
            Entity::from_raw(1),
            Entity::from_raw(2),
        );
        session.step_index = step_index;
        session.current_step = ForgeStep::from_kind(kind);
        session.step_state = state;
        session
    }

    #[test]
    fn tempering_snapshot_includes_blueprint_pattern() {
        let blueprint = qing_feng();
        let session = session_at(
            &blueprint,
            StepKind::Tempering,
            StepState::Tempering(TemperingState {
                beat_cursor: 1,
                hits: 1,
                ..Default::default()
            }),
        );

        let data = build_session_data(&session, blueprint.name.as_str(), Some(&blueprint));

        match data.step_state {
            ForgeStepStateDataV1::Tempering {
                pattern,
                beat_cursor,
                ..
            } => {
                assert_eq!(beat_cursor, 1);
                assert_eq!(pattern.len(), 10);
                assert_eq!(
                    &pattern[0..3],
                    &[
                        crate::schema::forge::TemperBeatV1::Light,
                        crate::schema::forge::TemperBeatV1::Light,
                        crate::schema::forge::TemperBeatV1::Heavy,
                    ]
                );
            }
            other => panic!("expected tempering state, got {other:?}"),
        }
    }

    #[test]
    fn inscription_snapshot_uses_blueprint_max_slots() {
        let blueprint = ling_feng();
        let session = session_at(
            &blueprint,
            StepKind::Inscription,
            StepState::Inscription(InscriptionState {
                scrolls_in: vec!["frost_edge".to_string()],
                filled_slots: 1,
                failed: false,
            }),
        );

        let data = build_session_data(&session, blueprint.name.as_str(), Some(&blueprint));

        match data.step_state {
            ForgeStepStateDataV1::Inscription {
                filled_slots,
                max_slots,
                failed,
            } => {
                assert_eq!(filled_slots, 1);
                assert_eq!(max_slots, 2);
                assert!(!failed);
            }
            other => panic!("expected inscription state, got {other:?}"),
        }
    }

    #[test]
    fn consecration_snapshot_includes_blueprint_min_realm() {
        let blueprint = ling_feng();
        let session = session_at(
            &blueprint,
            StepKind::Consecration,
            StepState::Consecration(ConsecrationState {
                qi_injected: 12.5,
                qi_required: 80.0,
                color_imprint: None,
            }),
        );

        let data = build_session_data(&session, blueprint.name.as_str(), Some(&blueprint));

        match data.step_state {
            ForgeStepStateDataV1::Consecration {
                qi_injected,
                qi_required,
                min_realm,
                ..
            } => {
                assert!((qi_injected - 12.5).abs() < f64::EPSILON);
                assert!((qi_required - 80.0).abs() < f64::EPSILON);
                assert_eq!(min_realm, Some(Realm::Spirit));
            }
            other => panic!("expected consecration state, got {other:?}"),
        }
    }

    // ══════════════════ P2 — S2C 回执真实调用点：饱和测试 ══════════════════

    fn flush_all_client_packets(app: &mut App) {
        let world = app.world_mut();
        let mut query = world.query::<&mut Client>();
        for mut client in query.iter_mut(world) {
            client
                .flush_packets()
                .expect("mock client packets should flush successfully");
        }
    }

    /// 从 `MockClientHelper` 收到的包里解出所有 `bong:server_data` payload
    /// （测试构建走 JSON 分支，见 `serialize_server_data_payload` 的 `#[cfg(test)]` 分支）。
    fn collect_server_data_payloads(helper: &mut MockClientHelper) -> Vec<ServerDataPayloadV1> {
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
                Some(payload.payload)
            })
            .collect()
    }

    fn has_variant(payloads: &[ServerDataPayloadV1], want: &str) -> bool {
        payloads.iter().any(|p| match p {
            ServerDataPayloadV1::ForgeStation(_) => want == "forge_station",
            ServerDataPayloadV1::ForgeSession(_) => want == "forge_session",
            ServerDataPayloadV1::ForgeOutcome(_) => want == "forge_outcome",
            ServerDataPayloadV1::ForgeBlueprintBook(_) => want == "forge_blueprint_book",
            _ => false,
        })
    }

    fn spawn_station(app: &mut App, tier: u8, owner: Entity) -> Entity {
        app.world_mut()
            .spawn(WeaponForgeStation::placed(
                BlockPos::new(4, 64, 4),
                tier,
                owner,
            ))
            .id()
    }

    fn insert_qing_feng_session(
        app: &mut App,
        session_id: ForgeSessionId,
        station: Entity,
        caster: Entity,
        current_step: ForgeStep,
        step_state: StepState,
    ) {
        let mut sessions = app
            .world_mut()
            .remove_resource::<ForgeSessions>()
            .unwrap_or_default();
        let mut session =
            ForgeSession::new(session_id, "qing_feng_v0".to_string(), station, caster);
        session.current_step = current_step;
        session.step_state = step_state;
        sessions.insert(session);
        app.world_mut().insert_resource(sessions);
    }

    fn registry_with_qing_feng() -> BlueprintRegistry {
        let mut registry = BlueprintRegistry::new();
        registry
            .insert(qing_feng())
            .expect("qing_feng_v0 should insert into fresh registry");
        registry
    }

    // ── push_forge_start_snapshot_on_accept ─────────────────────────────

    #[test]
    fn start_accept_pushes_station_session_and_blueprint_book() {
        let mut app = App::new();
        app.add_event::<ForgeStartAccepted>();
        app.insert_resource(registry_with_qing_feng());
        app.add_systems(Update, push_forge_start_snapshot_on_accept);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let mut learned = LearnedBlueprints::new();
        learned.learn("qing_feng_v0".into());
        let caster = app.world_mut().spawn((client_bundle, learned)).id();
        let station = spawn_station(&mut app, 1, caster);
        insert_qing_feng_session(
            &mut app,
            ForgeSessionId(1),
            station,
            caster,
            ForgeStep::Billet,
            StepState::Billet(Default::default()),
        );

        app.world_mut().send_event(ForgeStartAccepted {
            session: ForgeSessionId(1),
            station,
            caster,
            blueprint: "qing_feng_v0".to_string(),
            materials: vec![("fan_tie".to_string(), 4), ("za_gang".to_string(), 1)],
        });
        app.update();
        flush_all_client_packets(&mut app);

        let payloads = collect_server_data_payloads(&mut helper);
        assert!(
            has_variant(&payloads, "forge_station"),
            "起炉受理应推 forge_station（send_forge_snapshots_to_player 真实调用点之一），实际={payloads:?}"
        );
        assert!(
            has_variant(&payloads, "forge_session"),
            "起炉受理应推 forge_session，实际={payloads:?}"
        );
        assert!(
            has_variant(&payloads, "forge_blueprint_book"),
            "已学图谱不为空时起炉受理应一并推 forge_blueprint_book，实际={payloads:?}"
        );
        let session_id = payloads.iter().find_map(|p| match p {
            ServerDataPayloadV1::ForgeSession(data) => Some(data.session_id),
            _ => None,
        });
        assert_eq!(
            session_id,
            Some(1),
            "forge_session.session_id 应等于受理的 session id"
        );
    }

    #[test]
    fn start_accept_omits_blueprint_book_when_caster_has_not_learned_anything() {
        // LearnedBlueprints 组件懒插入——未学过任何图谱时 caster 上根本没有这个
        // component，learned_q.get 应返回 Err，不应 panic 也不应推 blueprint_book。
        let mut app = App::new();
        app.add_event::<ForgeStartAccepted>();
        app.insert_resource(registry_with_qing_feng());
        app.add_systems(Update, push_forge_start_snapshot_on_accept);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let caster = app.world_mut().spawn(client_bundle).id();
        let station = spawn_station(&mut app, 1, caster);
        insert_qing_feng_session(
            &mut app,
            ForgeSessionId(2),
            station,
            caster,
            ForgeStep::Billet,
            StepState::Billet(Default::default()),
        );

        app.world_mut().send_event(ForgeStartAccepted {
            session: ForgeSessionId(2),
            station,
            caster,
            blueprint: "qing_feng_v0".to_string(),
            materials: vec![],
        });
        app.update();
        flush_all_client_packets(&mut app);

        let payloads = collect_server_data_payloads(&mut helper);
        assert!(has_variant(&payloads, "forge_station"));
        assert!(has_variant(&payloads, "forge_session"));
        assert!(
            !has_variant(&payloads, "forge_blueprint_book"),
            "无 LearnedBlueprints component 时不应推 forge_blueprint_book，实际={payloads:?}"
        );
    }

    #[test]
    fn start_accept_skips_when_station_entity_missing() {
        // station 实体在受理与回执之间被 despawn（理论边界）——不应 panic，不应发包。
        let mut app = App::new();
        app.add_event::<ForgeStartAccepted>();
        app.insert_resource(registry_with_qing_feng());
        app.insert_resource(ForgeSessions::new());
        app.add_systems(Update, push_forge_start_snapshot_on_accept);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let caster = app.world_mut().spawn(client_bundle).id();
        let ghost_station = Entity::from_raw(999_999);

        app.world_mut().send_event(ForgeStartAccepted {
            session: ForgeSessionId(3),
            station: ghost_station,
            caster,
            blueprint: "qing_feng_v0".to_string(),
            materials: vec![],
        });
        app.update();
        flush_all_client_packets(&mut app);

        assert!(
            collect_server_data_payloads(&mut helper).is_empty(),
            "station 实体缺失时不应发任何 forge payload"
        );
    }

    // ── push_forge_session_snapshot_on_interaction ──────────────────────

    #[test]
    fn tempering_hit_pushes_session_only_snapshot() {
        let mut app = App::new();
        app.add_event::<TemperingHit>();
        app.add_event::<InscriptionScrollApplied>();
        app.add_event::<ConsecrationInject>();
        app.insert_resource(registry_with_qing_feng());
        app.insert_resource(ForgeSessions::new());
        app.add_systems(Update, push_forge_session_snapshot_on_interaction);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let caster = app.world_mut().spawn(client_bundle).id();
        let station = Entity::from_raw(1);
        insert_qing_feng_session(
            &mut app,
            ForgeSessionId(5),
            station,
            caster,
            ForgeStep::Tempering,
            StepState::Tempering(TemperingState {
                hits: 1,
                ..Default::default()
            }),
        );

        app.world_mut().send_event(TemperingHit {
            session: ForgeSessionId(5),
            beat: TemperBeat::Light,
            ticks_remaining: 4,
        });
        app.update();
        flush_all_client_packets(&mut app);

        let payloads = collect_server_data_payloads(&mut helper);
        assert_eq!(
            payloads.len(),
            1,
            "单步交互只应推 1 条 forge_session payload（不重发 station/blueprint_book），实际={payloads:?}"
        );
        assert!(has_variant(&payloads, "forge_session"));
        assert!(!has_variant(&payloads, "forge_station"));
        assert!(!has_variant(&payloads, "forge_blueprint_book"));
    }

    #[test]
    fn consecration_inject_pushes_session_snapshot() {
        let mut app = App::new();
        app.add_event::<TemperingHit>();
        app.add_event::<InscriptionScrollApplied>();
        app.add_event::<ConsecrationInject>();
        app.insert_resource(registry_with_qing_feng());
        app.insert_resource(ForgeSessions::new());
        app.add_systems(Update, push_forge_session_snapshot_on_interaction);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let caster = app.world_mut().spawn(client_bundle).id();
        insert_qing_feng_session(
            &mut app,
            ForgeSessionId(6),
            Entity::from_raw(1),
            caster,
            ForgeStep::Consecration,
            StepState::Consecration(Default::default()),
        );

        app.world_mut().send_event(ConsecrationInject {
            session: ForgeSessionId(6),
            qi_amount: 3.0,
        });
        app.update();
        flush_all_client_packets(&mut app);

        assert!(has_variant(
            &collect_server_data_payloads(&mut helper),
            "forge_session"
        ));
    }

    #[test]
    fn interaction_snapshot_skips_unknown_session_without_panic() {
        let mut app = App::new();
        app.add_event::<TemperingHit>();
        app.add_event::<InscriptionScrollApplied>();
        app.add_event::<ConsecrationInject>();
        app.insert_resource(registry_with_qing_feng());
        app.insert_resource(ForgeSessions::new());
        app.add_systems(Update, push_forge_session_snapshot_on_interaction);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        app.world_mut().spawn(client_bundle);

        app.world_mut().send_event(TemperingHit {
            session: ForgeSessionId(404),
            beat: TemperBeat::Heavy,
            ticks_remaining: 0,
        });
        app.update();
        flush_all_client_packets(&mut app);

        assert!(
            collect_server_data_payloads(&mut helper).is_empty(),
            "未知 session_id 不应发包也不应 panic"
        );
    }

    // ── push_forge_session_snapshot_on_step_advance ─────────────────────

    #[test]
    fn step_advance_not_done_pushes_full_snapshot() {
        let mut app = App::new();
        app.add_event::<StepAdvance>();
        app.insert_resource(registry_with_qing_feng());
        app.add_systems(Update, push_forge_session_snapshot_on_step_advance);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let mut learned = LearnedBlueprints::new();
        learned.learn("qing_feng_v0".into());
        let caster = app.world_mut().spawn((client_bundle, learned)).id();
        let station = spawn_station(&mut app, 1, caster);
        insert_qing_feng_session(
            &mut app,
            ForgeSessionId(9),
            station,
            caster,
            ForgeStep::Tempering,
            StepState::Tempering(TemperingState {
                hits: 3,
                ..Default::default()
            }),
        );

        app.world_mut().send_event(StepAdvance {
            session: ForgeSessionId(9),
            from_step: ForgeStep::Tempering,
        });
        app.update();
        flush_all_client_packets(&mut app);

        let payloads = collect_server_data_payloads(&mut helper);
        assert!(
            has_variant(&payloads, "forge_station"),
            "未完成的 step 推进应推 station（send_forge_snapshots_to_player 第二个调用点），实际={payloads:?}"
        );
        assert!(
            has_variant(&payloads, "forge_session"),
            "未完成时应带上最新 session 快照，实际={payloads:?}"
        );
        assert!(has_variant(&payloads, "forge_blueprint_book"));
    }

    #[test]
    fn step_advance_done_omits_session_snapshot() {
        let mut app = App::new();
        app.add_event::<StepAdvance>();
        app.insert_resource(registry_with_qing_feng());
        app.add_systems(Update, push_forge_session_snapshot_on_step_advance);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let caster = app.world_mut().spawn(client_bundle).id();
        let station = spawn_station(&mut app, 1, caster);
        insert_qing_feng_session(
            &mut app,
            ForgeSessionId(10),
            station,
            caster,
            ForgeStep::Done,
            StepState::None,
        );

        app.world_mut().send_event(StepAdvance {
            session: ForgeSessionId(10),
            from_step: ForgeStep::Consecration,
        });
        app.update();
        flush_all_client_packets(&mut app);

        let payloads = collect_server_data_payloads(&mut helper);
        assert!(
            has_variant(&payloads, "forge_station"),
            "Done 后仍应回推 station（反映 has_session 已清 false），实际={payloads:?}"
        );
        assert!(
            !has_variant(&payloads, "forge_session"),
            "Done 后不应再带 session 快照（结算内容由 forge_outcome 承载），实际={payloads:?}"
        );
    }

    #[test]
    fn step_advance_skips_when_station_missing_without_panic() {
        let mut app = App::new();
        app.add_event::<StepAdvance>();
        app.insert_resource(registry_with_qing_feng());
        app.add_systems(Update, push_forge_session_snapshot_on_step_advance);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let caster = app.world_mut().spawn(client_bundle).id();
        let ghost_station = Entity::from_raw(999_998);
        insert_qing_feng_session(
            &mut app,
            ForgeSessionId(11),
            ghost_station,
            caster,
            ForgeStep::Tempering,
            StepState::Tempering(Default::default()),
        );

        app.world_mut().send_event(StepAdvance {
            session: ForgeSessionId(11),
            from_step: ForgeStep::Tempering,
        });
        app.update();
        flush_all_client_packets(&mut app);

        assert!(collect_server_data_payloads(&mut helper).is_empty());
    }

    // ── push_forge_outcome_on_event ──────────────────────────────────────

    fn outcome_event(caster: Entity, bucket: ForgeBucket) -> ForgeOutcomeEvent {
        ForgeOutcomeEvent {
            session: ForgeSessionId(20),
            caster,
            blueprint: "qing_feng_v0".to_string(),
            bucket,
            weapon_item: Some("qing_feng_sword".to_string()),
            quality: 0.8,
            color: None,
            side_effects: vec![],
            achieved_tier: 2,
            consecration_qi_amount: 0.0,
        }
    }

    #[test]
    fn outcome_perfect_pushes_payload_with_flawed_path_false() {
        let mut app = App::new();
        app.add_event::<ForgeOutcomeEvent>();
        app.add_systems(Update, push_forge_outcome_on_event);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let caster = app.world_mut().spawn(client_bundle).id();

        app.world_mut()
            .send_event(outcome_event(caster, ForgeBucket::Perfect));
        app.update();
        flush_all_client_packets(&mut app);

        let payloads = collect_server_data_payloads(&mut helper);
        let outcome = payloads
            .iter()
            .find_map(|p| match p {
                ServerDataPayloadV1::ForgeOutcome(data) => Some(data.as_ref()),
                _ => None,
            })
            .expect("forge_outcome payload should be pushed");
        assert!(
            !outcome.flawed_path,
            "Perfect bucket 不是 flawed_fallback 路径，flawed_path 应为 false"
        );
    }

    #[test]
    fn outcome_flawed_pushes_payload_with_flawed_path_true() {
        let mut app = App::new();
        app.add_event::<ForgeOutcomeEvent>();
        app.add_systems(Update, push_forge_outcome_on_event);

        let (client_bundle, mut helper) = create_mock_client("Azure");
        let caster = app.world_mut().spawn(client_bundle).id();

        app.world_mut()
            .send_event(outcome_event(caster, ForgeBucket::Flawed));
        app.update();
        flush_all_client_packets(&mut app);

        let payloads = collect_server_data_payloads(&mut helper);
        let outcome = payloads
            .iter()
            .find_map(|p| match p {
                ServerDataPayloadV1::ForgeOutcome(data) => Some(data.as_ref()),
                _ => None,
            })
            .expect("forge_outcome payload should be pushed");
        assert!(
            outcome.flawed_path,
            "Flawed bucket 走了 flawed_fallback 路径，flawed_path 应为 true"
        );
    }

    #[test]
    fn outcome_skips_when_caster_has_no_client_without_panic() {
        // billet-Waste 早退路径的 caster 理论上恒有 Client（玩家在线才能起炉），
        // 但防御纵深仍要求实体缺 Client 时不 panic、不发包。
        let mut app = App::new();
        app.add_event::<ForgeOutcomeEvent>();
        app.add_systems(Update, push_forge_outcome_on_event);

        let headless_caster = app.world_mut().spawn_empty().id();
        app.world_mut()
            .send_event(outcome_event(headless_caster, ForgeBucket::Waste));
        app.update();
        // 无 client 可 flush；只需确认不 panic 即通过。
    }
}
