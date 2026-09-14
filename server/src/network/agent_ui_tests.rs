use super::*;

// ── Bevy App 级系统测试（驱动真 system，断言 Redis emit payload）───────────

use crossbeam_channel::unbounded;
use valence::prelude::{App, IntoSystemConfigs, Update};
use valence::testing::create_mock_client;

use crate::cultivation::components::{Cultivation, Realm};
use crate::schema::agent_ui::AgentUiRequestCommandV1;
use crate::schema::common::MAX_PAYLOAD_BYTES;

/// 构造最小 App：RedisBridgeResource + 所有 agent_ui 系统 + event。
fn build_agent_ui_app() -> (App, crossbeam_channel::Receiver<RedisOutbound>) {
    let mut app = App::new();
    let (tx_outbound, rx_outbound) = unbounded();
    let (_tx_inbound, rx_inbound) = unbounded::<crate::network::redis_bridge::RedisInbound>();
    app.insert_resource(RedisBridgeResource {
        tx_outbound,
        rx_inbound,
    });
    app.add_event::<AgentUiCmdEvent>();
    app.add_event::<AgentUiResponseEvent>();
    app.init_resource::<AgentUiSessionStore>();
    app.init_resource::<CurrentTickResource>();
    app.add_systems(
        Update,
        (
            increment_current_tick_system,
            receive_agent_ui_cmd_system.after(increment_current_tick_system),
            agent_ui_tick_system.after(increment_current_tick_system),
            receive_agent_ui_response_system,
            receive_player_disconnect_system,
        ),
    );
    (app, rx_outbound)
}

/// 在 app 中 spawn 一个带 Client / Cultivation / Username 的测试玩家。
fn spawn_test_player(app: &mut App, username: &str, realm: Realm) -> Entity {
    let (bundle, _helper) = create_mock_client(username);
    let entity = app.world_mut().spawn(bundle).id();
    let cultivation = Cultivation {
        realm,
        ..Cultivation::default()
    };
    app.world_mut().entity_mut(entity).insert(cultivation);
    entity
}

fn make_cmd(request_id: &str, target: &str, realm_gate: u8) -> AgentUiRequestCommandV1 {
    // XML 使用与 agent xmlTemplates.ts 真实产出一致的 <owo-ui><components> 包裹形，
    // 确保 xml_sanitize 对真实流量的测试不会通过裸 flow-layout 蒙混（BLOCKER 修复）。
    //
    // target_player 使用 canonical_player_id 格式（"offline:<name>"），
    // 与 agent 侧 uiRenderer.ts 发送 `target_player: targetPlayer.uuid` 对齐
    // （uuid 由 world-state canonical_player_id 赋值，格式为 "offline:<username>"）。
    AgentUiRequestCommandV1 {
        request_id: request_id.to_string(),
        target_player: format!("offline:{target}"),
        xml: r#"<owo-ui><components><flow-layout direction="vertical" gap="4"><button id="btn_a">确认</button></flow-layout></components></owo-ui>"#.to_string(),
        timeout_ticks: 600,
        realm_gate,
        allowed_button_ids: vec!["btn_a".to_string(), "btn_b".to_string()],
    }
}

/// 共享 wire fixture 必须由生产 encoder 精确产出，并锁定专属 channel ID。
#[test]
fn agent_ui_close_channel_wire_fixture_matches_production_encoder() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../agent/packages/schema/samples/agent-ui-close.channel-wire.sample.json"
    ))
    .expect("agent_ui_close 共享 wire fixture 应是合法 JSON");

    assert_eq!(
        fixture["channel"].as_str(),
        Some(AGENT_UI_CLOSE_CHANNEL),
        "共享 fixture channel 必须与 server 生产 channel 常量一致"
    );
    let cases = fixture["cases"]
        .as_array()
        .expect("agent_ui_close 共享 fixture 应包含 cases 数组");
    assert_eq!(
        cases.len(),
        3,
        "共享 fixture 应覆盖 Replaced 与两种错误 reason"
    );

    for case in cases {
        let name = case["name"].as_str().expect("fixture case.name 应为字符串");
        let request_id = case["request_id"]
            .as_str()
            .expect("fixture case.request_id 应为字符串");
        let reason = case.get("reason").and_then(serde_json::Value::as_str);
        let expected_utf8 = case["payload_utf8"]
            .as_str()
            .expect("fixture case.payload_utf8 应为字符串");

        let actual = encode_agent_ui_close_payload(request_id, reason)
            .unwrap_or_else(|err| panic!("{name} 生产 close payload 编码失败：{err}"));
        assert_eq!(
            actual,
            expected_utf8.as_bytes(),
            "{name} fixture 必须与 server 生产 encoder 的原始 bytes 完全一致"
        );

        let parsed: AgentUiClosePayloadV1 = serde_json::from_slice(&actual).unwrap_or_else(|err| {
            panic!("{name} 生产 bytes 无法按 AgentUiClosePayloadV1 解析：{err}")
        });
        assert_eq!(parsed.request_id, request_id, "{name} request_id 漂移");
        assert_eq!(parsed.reason.as_deref(), reason, "{name} reason 漂移");
    }
}

/// 真实 realm gate producer 必须生成只指向目标玩家的拒绝 response。
#[test]
fn system_realm_gate_rejected_emits_private_response() {
    let (mut app, rx) = build_agent_ui_app();
    let target_username = "E2EPlayer";
    // 引气 rank=2，凝脉 gate=3 → 拒绝。
    spawn_test_player(&mut app, target_username, Realm::Induce);
    let cmd = make_cmd(
        "req-realm-gate-private-response",
        target_username,
        Realm::Condense.rank(),
    );
    let expected_target = cmd.target_player.clone();
    app.world_mut().send_event(AgentUiCmdEvent(cmd));
    app.update();

    let resp = match rx.try_recv().expect("realm_gate 拒绝应发 Redis response") {
        RedisOutbound::AgentUiResponse(r) => r,
        other => panic!("期望 AgentUiResponse，实为 {other:?}"),
    };
    assert!(matches!(resp.action, AgentUiActionType::Error));
    assert_eq!(resp.request_id, "req-realm-gate-private-response");
    assert_eq!(
        resp.target_player.as_deref(),
        Some(expected_target.as_str())
    );
    assert_eq!(
        resp.params.get("reason").map(String::as_str),
        Some("realm_gate_rejected")
    );
    assert_eq!(
        resp.params.get("player_realm").map(String::as_str),
        Some("2")
    );
    assert_eq!(
        resp.params.get("required_realm").map(String::as_str),
        Some("3")
    );
    assert_eq!(
        resp.params.len(),
        3,
        "realm gate producer 不得夹带未约定字段"
    );
    assert!(
        rx.try_recv().is_err(),
        "一次 realm gate 拒绝必须只生成一条 response"
    );
}

/// 玩家离线（target_player 不存在）→ Redis emit {action:error, reason:player_offline}
#[test]
fn system_player_offline_emits_error_response() {
    let (mut app, rx) = build_agent_ui_app();
    // 不 spawn 任何玩家
    let cmd = make_cmd("req-offline", "GhostPlayer", 0);
    app.world_mut().send_event(AgentUiCmdEvent(cmd));
    app.update();

    let resp = match rx.try_recv().expect("离线拒绝应发 Redis response") {
        RedisOutbound::AgentUiResponse(r) => r,
        other => panic!("期望 AgentUiResponse，实为 {other:?}"),
    };
    assert!(
        matches!(resp.action, AgentUiActionType::Error),
        "action 应为 Error，实为 {:?}",
        resp.action
    );
    assert_eq!(
        resp.params.get("reason").map(|s| s.as_str()),
        Some("player_offline"),
        "reason 应为 player_offline，实为 {:?}",
        resp.params.get("reason")
    );
}

/// BLOCKER-身份键契约 pin 测试：agent 发 target_player="offline:Kiz"（canonical_player_id 格式），
/// server 能正确解析到 username="Kiz" 的 ECS entity，session 成功创建（不误判 player_offline）。
#[test]
fn system_canonical_player_id_format_resolves_correctly() {
    let (mut app, rx) = build_agent_ui_app();
    spawn_test_player(&mut app, "Kiz", Realm::Condense);

    // agent 发送 canonical id（"offline:Kiz"）
    let cmd = AgentUiRequestCommandV1 {
        request_id: "req-canonical".to_string(),
        target_player: "offline:Kiz".to_string(),
        xml: r#"<owo-ui><components><flow-layout><label>test</label></flow-layout></components></owo-ui>"#.to_string(),
        timeout_ticks: 600,
        realm_gate: 0,
        allowed_button_ids: vec![],
    };
    app.world_mut().send_event(AgentUiCmdEvent(cmd));
    app.update();

    let msgs: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
    // 不应有 player_offline 错误
    let player_offline = msgs.iter().any(|m| {
        if let RedisOutbound::AgentUiResponse(r) = m {
            r.params.get("reason").map(|s| s.as_str()) == Some("player_offline")
        } else {
            false
        }
    });
    assert!(
        !player_offline,
        "canonical id 'offline:Kiz' 应能找到 username=Kiz 的玩家，不应返回 player_offline，实际 msgs={msgs:?}"
    );
}

/// 对应负例：裸 username 格式（不加 offline: 前缀）不应匹配到在线玩家（验证旧 bug 已修）。
#[test]
fn system_bare_username_format_returns_player_offline() {
    let (mut app, rx) = build_agent_ui_app();
    spawn_test_player(&mut app, "Kiz", Realm::Condense);

    // 旧 bug：agent 发裸 "Kiz" 而非 "offline:Kiz"，会被 server 误匹配；
    // 修复后 canonical_player_id 比较，"Kiz" ≠ "offline:Kiz" → player_offline。
    let cmd = AgentUiRequestCommandV1 {
        request_id: "req-bare-username".to_string(),
        target_player: "Kiz".to_string(), // 裸 username，非 canonical
        xml: r#"<owo-ui><components><flow-layout><label>test</label></flow-layout></components></owo-ui>"#.to_string(),
        timeout_ticks: 600,
        realm_gate: 0,
        allowed_button_ids: vec![],
    };
    app.world_mut().send_event(AgentUiCmdEvent(cmd));
    app.update();

    let msgs: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
    let player_offline = msgs.iter().any(|m| {
        if let RedisOutbound::AgentUiResponse(r) = m {
            r.params.get("reason").map(|s| s.as_str()) == Some("player_offline")
        } else {
            false
        }
    });
    assert!(
        player_offline,
        "裸 username 'Kiz' 不应匹配 canonical 'offline:Kiz' → 应返回 player_offline，实际 msgs={msgs:?}"
    );
}

/// Replaced：新 cmd 替换旧 Open session → Redis emit {action:replaced} for old request_id
#[test]
fn system_replaced_session_emits_replaced_response() {
    let (mut app, rx) = build_agent_ui_app();
    spawn_test_player(&mut app, "TestPlayer", Realm::Condense);

    // 第一条 cmd
    let cmd1 = make_cmd("req-old", "TestPlayer", 0);
    app.world_mut().send_event(AgentUiCmdEvent(cmd1));
    app.update();
    // 收掉第一轮可能的消息（req-old 不应有 error；只有 session 创建成功）
    while rx.try_recv().is_ok() {}

    // 第二条 cmd → 替换
    let cmd2 = make_cmd("req-new", "TestPlayer", 0);
    app.world_mut().send_event(AgentUiCmdEvent(cmd2));
    app.update();

    // 应有 replaced response for req-old
    let resps: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
    let replaced = resps.iter().find(|r| {
        if let RedisOutbound::AgentUiResponse(r) = r {
            matches!(r.action, AgentUiActionType::Replaced)
        } else {
            false
        }
    });
    assert!(
        replaced.is_some(),
        "替换旧 session 应发 replaced response，实际 resps: {resps:?}"
    );
    if let Some(RedisOutbound::AgentUiResponse(r)) = replaced {
        assert_eq!(
            r.request_id, "req-old",
            "replaced response 的 request_id 应为 req-old，实为 {}",
            r.request_id
        );
    }
}

/// allowed_button_ids 非法 → Redis emit error + client close(reason=invalid_button_id)，session 结束
#[test]
fn system_invalid_button_id_emits_error_response_and_close_s2c() {
    let (mut app, rx) = build_agent_ui_app();
    let (bundle, mut helper) = create_mock_client("TestPlayer");
    let entity = app.world_mut().spawn(bundle).id();
    app.world_mut().entity_mut(entity).insert(Cultivation {
        realm: Realm::Induce,
        ..Cultivation::default()
    });

    // 建立 Open session
    let cmd = make_cmd("req-btn", "TestPlayer", 0);
    app.world_mut().send_event(AgentUiCmdEvent(cmd));
    app.update();
    while rx.try_recv().is_ok() {} // 清掉 session 创建时的消息
    flush_all_clients(&mut app);
    let _ = helper.collect_received(); // 清掉初始 agent_ui_request S2C

    // 发非法 button_id
    app.world_mut().send_event(AgentUiResponseEvent {
        player: entity,
        request_id: "req-btn".to_string(),
        action: AgentUiActionType::ButtonClick,
        params: [("button_id".to_string(), "invalid_btn".to_string())]
            .into_iter()
            .collect(),
    });
    app.update();

    let resp = match rx
        .try_recv()
        .expect("非法 button_id 应发 Redis error response")
    {
        RedisOutbound::AgentUiResponse(r) => r,
        other => panic!("期望 AgentUiResponse，实为 {other:?}"),
    };
    assert!(
        matches!(resp.action, AgentUiActionType::Error),
        "action 应为 Error（invalid_button_id），实为 {:?}",
        resp.action
    );
    assert_eq!(
        resp.params.get("reason").map(|s| s.as_str()),
        Some("invalid_button_id"),
        "reason 应为 invalid_button_id，实为 {:?}",
        resp.params.get("reason")
    );
    let payloads = collect_agent_ui_close_payloads(&mut app, &mut helper);
    assert_eq!(
        payloads.len(),
        1,
        "invalid_button_id 时 client 应收 1 条 bong:agent_ui_close S2C，实际 {}",
        payloads.len()
    );
    assert_eq!(
        payloads[0]["request_id"].as_str(),
        Some("req-btn"),
        "AgentUiClose 的 request_id 应为 req-btn，实为 {}",
        payloads[0]["request_id"]
    );
    assert_eq!(
        payloads[0]["reason"].as_str(),
        Some("invalid_button_id"),
        "AgentUiClose 的 reason 应为 invalid_button_id，实为 {}",
        payloads[0]["reason"]
    );

    // invalid_button_id 是 Error 终态，不能残留 Open session 等待 timeout。
    let store = app.world().resource::<AgentUiSessionStore>();
    assert!(
        store.get(entity).is_none(),
        "invalid_button_id 后 session 应已终止并从 store 移除"
    );
}

/// timeout：tick 推进 >= expire_tick → TimedOut + Redis{action:timeout}
#[test]
fn system_timeout_emits_timeout_response() {
    let (mut app, rx) = build_agent_ui_app();
    let entity = spawn_test_player(&mut app, "TestPlayer", Realm::Awaken);

    // 直接在 store 插入 Open session，expire_tick=20
    // 然后把 CurrentTickResource 设为 19 → 下次 update 后 increment 到 20 → timeout 触发
    {
        let mut store = app.world_mut().resource_mut::<AgentUiSessionStore>();
        store.upsert(
            entity,
            AgentUiSession {
                request_id: "req-timeout".to_string(),
                state: AgentUiSessionState::Open,
                expire_tick: 20,
                allowed_button_ids: vec![],
            },
        );
    }
    app.world_mut().resource_mut::<CurrentTickResource>().0 = 19;

    // 一次 update：increment → tick=20，agent_ui_tick_system 判 20>=20 → TimedOut
    app.update();

    let all_resps: Vec<RedisOutbound> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
    let found_timeout = all_resps.iter().any(|r| {
        if let RedisOutbound::AgentUiResponse(r) = r {
            matches!(r.action, AgentUiActionType::Timeout)
        } else {
            false
        }
    });
    assert!(
        found_timeout,
        "tick(20) >= expire_tick(20) 后应发 timeout response，实际 resps: {all_resps:?}"
    );
    // session 应已被清除
    let store = app.world().resource::<AgentUiSessionStore>();
    assert!(
        store.get(entity).is_none(),
        "timeout 后 session 应已被清除，实为 {:?}",
        store.get(entity)
    );
}

/// 玩家离线（Client 组件移除）→ Redis{action:dismissed}
#[test]
fn system_player_disconnect_emits_dismissed_response() {
    let (mut app, rx) = build_agent_ui_app();
    let entity = spawn_test_player(&mut app, "TestPlayer", Realm::Awaken);

    // 建立 Open session（手动插入，绕过 cmd 系统，保证 session 存在）
    {
        let mut store = app.world_mut().resource_mut::<AgentUiSessionStore>();
        store.upsert(
            entity,
            AgentUiSession {
                request_id: "req-disconnect".to_string(),
                state: AgentUiSessionState::Open,
                expire_tick: 99999,
                allowed_button_ids: vec![],
            },
        );
    }

    // 移除 Client 组件触发 receive_player_disconnect_system
    use valence::prelude::Client as ValenceClient;
    app.world_mut().entity_mut(entity).remove::<ValenceClient>();
    app.update();

    let resp = match rx.try_recv().expect("玩家断线应发 dismissed response") {
        RedisOutbound::AgentUiResponse(r) => r,
        other => panic!("期望 AgentUiResponse，实为 {other:?}"),
    };
    assert!(
        matches!(resp.action, AgentUiActionType::Dismissed),
        "action 应为 Dismissed，实为 {:?}",
        resp.action
    );
    assert_eq!(
        resp.request_id, "req-disconnect",
        "request_id 应为 req-disconnect，实为 {}",
        resp.request_id
    );
}

// ── AgentUiSessionState ──────────────────────────────────────────────────

#[test]
fn session_state_open_not_terminal() {
    assert!(
        !AgentUiSessionState::Open.is_terminal(),
        "Open 状态不应为 terminal"
    );
}

#[test]
fn session_state_all_terminal_variants() {
    let terminals = [
        AgentUiSessionState::Completed,
        AgentUiSessionState::Dismissed,
        AgentUiSessionState::TimedOut,
        AgentUiSessionState::Replaced,
        AgentUiSessionState::Error,
    ];
    for state in &terminals {
        assert!(
            state.is_terminal(),
            "{state:?} 应为 terminal，但 is_terminal() 返回 false"
        );
    }
}

// ── AgentUiSessionStore ──────────────────────────────────────────────────

fn make_session(request_id: &str, expire_tick: u64) -> AgentUiSession {
    AgentUiSession {
        request_id: request_id.to_string(),
        state: AgentUiSessionState::Open,
        expire_tick,
        allowed_button_ids: vec!["btn_a".to_string(), "btn_b".to_string()],
    }
}

fn dummy_entity(n: u32) -> Entity {
    // In tests we create raw entity ids — use from_bits as documented
    Entity::from_bits((n as u64) | ((1u64) << 32))
}

#[test]
fn session_store_upsert_new_returns_none() {
    let mut store = AgentUiSessionStore::default();
    let entity = dummy_entity(1);
    let result = store.upsert(entity, make_session("req-1", 100));
    assert!(
        result.is_none(),
        "新插入 session 时 upsert 应返回 None（无旧 session），实为 {result:?}"
    );
}

#[test]
fn session_store_upsert_replaces_open_returns_old() {
    let mut store = AgentUiSessionStore::default();
    let entity = dummy_entity(2);
    store.upsert(entity, make_session("req-1", 100));
    let old = store.upsert(entity, make_session("req-2", 200));
    let old = old.expect("替换 Open session 时 upsert 应返回旧 session");
    assert_eq!(
        old.request_id, "req-1",
        "返回的旧 session request_id 应为 req-1，实为 {}",
        old.request_id
    );
    // 新 session 应已替换
    assert_eq!(
        store.get(entity).unwrap().request_id,
        "req-2",
        "upsert 后 store 中应为新 session req-2"
    );
}

#[test]
fn session_store_upsert_non_open_old_returns_none() {
    let mut store = AgentUiSessionStore::default();
    let entity = dummy_entity(3);
    // 插入 terminal session
    let mut old_session = make_session("req-1", 100);
    old_session.state = AgentUiSessionState::Completed;
    store.upsert(entity, old_session);
    // 替换 terminal session → 不返回旧 session（terminal 不需要 close 信号）
    let result = store.upsert(entity, make_session("req-2", 200));
    assert!(
        result.is_none(),
        "替换 terminal session 时 upsert 应返回 None，实为 {result:?}"
    );
}

#[test]
fn session_store_take_if_match_correct_id() {
    let mut store = AgentUiSessionStore::default();
    let entity = dummy_entity(4);
    store.upsert(entity, make_session("req-correct", 100));
    let taken = store.take_if_match(entity, "req-correct");
    assert!(
        taken.is_some(),
        "request_id 匹配时 take_if_match 应返回 Some"
    );
    assert!(
        store.get(entity).is_none(),
        "take_if_match 后 store 中不应再有该 entity 的 session"
    );
}

#[test]
fn session_store_take_if_match_wrong_id_returns_none() {
    let mut store = AgentUiSessionStore::default();
    let entity = dummy_entity(5);
    store.upsert(entity, make_session("req-correct", 100));
    let result = store.take_if_match(entity, "req-wrong");
    assert!(
        result.is_none(),
        "request_id 不匹配时 take_if_match 应返回 None，实为 {result:?}"
    );
    // session 应仍在 store 中
    assert!(
        store.get(entity).is_some(),
        "take_if_match 失败后 store 中应仍有该 session"
    );
}

#[test]
fn session_store_take_if_match_idempotent() {
    let mut store = AgentUiSessionStore::default();
    let entity = dummy_entity(6);
    store.upsert(entity, make_session("req-1", 100));
    let first = store.take_if_match(entity, "req-1");
    let second = store.take_if_match(entity, "req-1");
    assert!(first.is_some(), "第一次 take_if_match 应返回 Some");
    assert!(second.is_none(), "第二次 take_if_match 应返回 None（幂等）");
}

// ── xml_sanitize ─────────────────────────────────────────────────────────

/// 跨端契约 pin 测试：agent xmlTemplates.ts 真实输出的 <owo-ui><components> 包裹形
/// 必须通过 xml_sanitize。此测试锁死 server 接受 client 所需的包裹结构，
/// 任何对 ALLOWED_TAGS 的破坏性变更会立刻撞红此用例（BLOCKER 修复 cross-stack contract）。
#[test]
fn xml_sanitize_accepts_owo_ui_wrapped_form_cross_stack_contract() {
    // 与 agent TSY_DISCOVERY_TEMPLATE 结构对齐（精简版）
    let xml_tsy = r#"<owo-ui><components><flow-layout direction="vertical" gap="4"><label style="color:#C8A060">【活坍缩渊】测试地带</label><button id="enter_realm">踏入探寻</button><button id="dismiss">离开</button></flow-layout></components></owo-ui>"#;
    let result = xml_sanitize(xml_tsy);
    assert!(
        result.is_ok(),
        "agent TSY_DISCOVERY_TEMPLATE 包裹形应通过 xml_sanitize（owo-ui+components 必须在白名单），错误：{:?}",
        result.err()
    );

    // 与 agent TIANDAO_REVELATION_TEMPLATE 结构对齐
    let xml_tiandao = r#"<owo-ui><components><flow-layout direction="vertical" gap="6"><label style="color:#8888FF">天意</label><label style="color:#CCCCCC">天道降示：真元将竭，速寻灵源</label><button id="dismiss" style="color:#AAAAAA">闭目冥思</button></flow-layout></components></owo-ui>"#;
    let result2 = xml_sanitize(xml_tiandao);
    assert!(
        result2.is_ok(),
        "agent TIANDAO_REVELATION_TEMPLATE 包裹形应通过 xml_sanitize，错误：{:?}",
        result2.err()
    );

    // DTD/entity 仍须禁（即使包裹在 owo-ui 内）
    let xml_with_doctype = r#"<!DOCTYPE evil><owo-ui><components><flow-layout><label>x</label></flow-layout></components></owo-ui>"#;
    let result3 = xml_sanitize(xml_with_doctype);
    assert_eq!(
        result3,
        Err(XmlSanitizeError::DoctypeOrEntity),
        "owo-ui 包裹形内嵌 DOCTYPE 仍应被拒绝"
    );
}

#[test]
fn xml_sanitize_happy_path() {
    // owo-ui 根节点是 client UIModel.load 的强制要求，合法 XML 必须以 <owo-ui> 开头。
    let xml = r#"<owo-ui><components><flow-layout><label>踏入世界</label><button id="enter">确认</button></flow-layout></components></owo-ui>"#;
    let result = xml_sanitize(xml);
    assert!(
        result.is_ok(),
        "合法 owo-ui 包裹 XML 应通过清洗，错误：{:?}",
        result.err()
    );
}

#[test]
fn xml_sanitize_rejects_bare_xml_missing_owo_ui_root() {
    // 裸 XML 通过 sanitize 但 client UIModel.load 必 fallback，server 提前拦截。
    let xml =
        r#"<flow-layout><label>踏入世界</label><button id="enter">确认</button></flow-layout>"#;
    let result = xml_sanitize(xml);
    assert_eq!(
        result,
        Err(XmlSanitizeError::MissingOwoUiRoot),
        "缺 <owo-ui> 根节点的裸 XML 应返回 MissingOwoUiRoot，实为 {result:?}"
    );
}

#[test]
fn xml_sanitize_empty_string_rejects_missing_owo_ui_root() {
    // 空字符串无 owo-ui 根节点，应被拒绝。
    let result = xml_sanitize("");
    assert_eq!(
        result,
        Err(XmlSanitizeError::MissingOwoUiRoot),
        "空字符串缺 <owo-ui> 根节点应返回 MissingOwoUiRoot，实为 {result:?}"
    );
}

#[test]
fn xml_sanitize_rejects_doctype() {
    let xml = r#"<!DOCTYPE foo [<!ENTITY xxe "evil">]><label>test</label>"#;
    let result = xml_sanitize(xml);
    assert_eq!(
        result,
        Err(XmlSanitizeError::DoctypeOrEntity),
        "含 DOCTYPE 的 XML 应被拒绝"
    );
}

#[test]
fn xml_sanitize_rejects_entity() {
    let xml = r#"<!ENTITY foo "bar"><label>test</label>"#;
    let result = xml_sanitize(xml);
    assert_eq!(
        result,
        Err(XmlSanitizeError::DoctypeOrEntity),
        "含 ENTITY 的 XML 应被拒绝"
    );
}

#[test]
fn xml_sanitize_rejects_doctype_lowercase() {
    // 大小写不敏感
    let xml = r#"<!doctype foo><label>test</label>"#;
    let result = xml_sanitize(xml);
    assert_eq!(
        result,
        Err(XmlSanitizeError::DoctypeOrEntity),
        "小写 doctype 应被拒绝"
    );
}

#[test]
fn xml_sanitize_rejects_too_large() {
    let xml = "<label>".to_string() + &"x".repeat(8192) + "</label>";
    let result = xml_sanitize(&xml);
    assert!(
        matches!(result, Err(XmlSanitizeError::TooLarge(_))),
        "超 8192 字节的 XML 应返回 TooLarge 错误，实为 {result:?}"
    );
}

#[test]
fn xml_sanitize_accepts_exactly_8192_bytes() {
    // 构造正好 8192 字节的合法 owo-ui 包裹 xml
    // 框架：<owo-ui><label>CONTENT</label></owo-ui>
    let wrapper_overhead = "<owo-ui><label>".len() + "</label></owo-ui>".len(); // 15 + 17 = 32
    let content = "x".repeat(8192 - wrapper_overhead);
    let xml = format!("<owo-ui><label>{content}</label></owo-ui>");
    assert_eq!(xml.len(), 8192, "测试数据构造错误");
    let result = xml_sanitize(&xml);
    assert!(
        result.is_ok(),
        "恰好 8192 字节的 owo-ui 包裹 XML 应通过清洗，错误：{:?}",
        result.err()
    );
}

#[test]
fn xml_sanitize_rejects_non_whitelisted_tag() {
    // 非白名单标签必须带 owo-ui 根节点（先通过根节点检查，再检查内容）。
    let xml = r#"<owo-ui><components><flow-layout><script>alert(1)</script></flow-layout></components></owo-ui>"#;
    let result = xml_sanitize(xml);
    assert!(
        matches!(result, Err(XmlSanitizeError::NonWhitelistedTag(ref t)) if t == "script"),
        "非白名单标签 <script> 应返回 NonWhitelistedTag(\"script\")，实为 {result:?}"
    );
}

#[test]
fn xml_sanitize_allows_all_whitelisted_tags() {
    let xml = r#"<owo-ui><components><flow-layout><grid-layout><label>x</label><button id="b">y</button><texture id="t"/></grid-layout></flow-layout></components></owo-ui>"#;
    let result = xml_sanitize(xml);
    assert!(
        result.is_ok(),
        "所有白名单标签应通过清洗，错误：{:?}",
        result.err()
    );
}

#[test]
fn xml_sanitize_rejects_too_deep() {
    // owo-ui 根 + 6 层内部嵌套 = 7 层（超出 6 层上限）
    let xml = "<owo-ui><flow-layout><flow-layout><flow-layout><flow-layout><flow-layout><label>deep</label></flow-layout></flow-layout></flow-layout></flow-layout></flow-layout></owo-ui>";
    let result = xml_sanitize(xml);
    assert!(
        matches!(result, Err(XmlSanitizeError::TooDeep)),
        "嵌套深度 >6 应返回 TooDeep 错误，实为 {result:?}"
    );
}

#[test]
fn xml_sanitize_accepts_depth_6() {
    // owo-ui 根 + 5 层内部嵌套 = 6 层（正好达到上限）
    let xml = "<owo-ui><flow-layout><flow-layout><flow-layout><flow-layout><label>ok</label></flow-layout></flow-layout></flow-layout></flow-layout></owo-ui>";
    let result = xml_sanitize(xml);
    assert!(
        result.is_ok(),
        "嵌套深度正好 6 层应通过清洗，错误：{:?}",
        result.err()
    );
}

#[test]
fn xml_sanitize_rejects_too_many_nodes() {
    // owo-ui(1) + flow-layout(1) + 63 labels = 65 节点（超出 64 上限）
    let inner: String = (0..63).map(|i| format!("<label>{i}</label>")).collect();
    let xml = format!("<owo-ui><flow-layout>{inner}</flow-layout></owo-ui>");
    let result = xml_sanitize(&xml);
    assert!(
        matches!(result, Err(XmlSanitizeError::TooManyNodes)),
        "节点数 65 应返回 TooManyNodes 错误，实为 {result:?}"
    );
}

#[test]
fn xml_sanitize_accepts_64_nodes() {
    // owo-ui(1) + flow-layout(1) + 62 labels = 64 节点（正好达到上限）
    let inner: String = (0..62).map(|i| format!("<label>{i}</label>")).collect();
    let xml = format!("<owo-ui><flow-layout>{inner}</flow-layout></owo-ui>");
    let result = xml_sanitize(&xml);
    assert!(
        result.is_ok(),
        "节点数正好 64 应通过清洗，错误：{:?}",
        result.err()
    );
}

// ── Realm::rank() ─────────────────────────────────────────────────────────

#[test]
fn realm_rank_1_indexed() {
    use crate::cultivation::components::Realm;
    assert_eq!(Realm::Awaken.rank(), 1, "醒灵 rank 应为 1");
    assert_eq!(Realm::Induce.rank(), 2, "引气 rank 应为 2");
    assert_eq!(Realm::Condense.rank(), 3, "凝脉 rank 应为 3");
    assert_eq!(Realm::Solidify.rank(), 4, "固元 rank 应为 4");
    assert_eq!(Realm::Spirit.rank(), 5, "通灵 rank 应为 5");
    assert_eq!(Realm::Void.rank(), 6, "化虚 rank 应为 6");
}

#[test]
fn realm_rank_realm_gate_0_always_passes() {
    use crate::cultivation::components::Realm;
    // realm_gate=0 表示不门控；server 侧判断为 `cmd.realm_gate == 0 || rank >= realm_gate`
    // 此处验证所有境界的 rank 均为正整数（>= 1），确保门控判断条件成立
    for realm in [
        Realm::Awaken,
        Realm::Induce,
        Realm::Condense,
        Realm::Solidify,
        Realm::Spirit,
        Realm::Void,
    ] {
        assert!(
            realm.rank() >= 1,
            "所有境界的 rank 应 >= 1，当 realm_gate=0 时 server 直接跳过比较，实为 {}",
            realm.rank()
        );
    }
}

#[test]
fn realm_rank_gate_3_condense_passes() {
    use crate::cultivation::components::Realm;
    // 凝脉 rank=3 >= realm_gate=3 → 通过
    assert!(
        Realm::Condense.rank() >= 3,
        "凝脉 rank=3 应满足 realm_gate=3"
    );
}

#[test]
fn realm_rank_gate_3_induce_rejected() {
    use crate::cultivation::components::Realm;
    // 引气 rank=2 < realm_gate=3 → 拒绝
    assert!(
        Realm::Induce.rank() < 3,
        "引气 rank=2 不应满足 realm_gate=3"
    );
}

// ── XmlSanitizeError Display ──────────────────────────────────────────────

#[test]
fn xml_sanitize_error_display_too_large() {
    let err = XmlSanitizeError::TooLarge(9000);
    let msg = err.to_string();
    assert!(
        msg.contains("9000"),
        "TooLarge Display 应含字节数，实为：{msg}"
    );
}

#[test]
fn xml_sanitize_error_display_doctype() {
    let err = XmlSanitizeError::DoctypeOrEntity;
    let msg = err.to_string();
    assert!(
        msg.contains("DOCTYPE") || msg.contains("ENTITY"),
        "DoctypeOrEntity Display 应含 DOCTYPE/ENTITY，实为：{msg}"
    );
}

#[test]
fn xml_sanitize_error_display_too_deep() {
    let err = XmlSanitizeError::TooDeep;
    let msg = err.to_string();
    assert!(
        msg.contains("depth") || msg.contains("6"),
        "TooDeep Display 应含 depth/6，实为：{msg}"
    );
}

#[test]
fn xml_sanitize_error_display_too_many_nodes() {
    let err = XmlSanitizeError::TooManyNodes;
    let msg = err.to_string();
    assert!(
        msg.contains("64") || msg.contains("node"),
        "TooManyNodes Display 应含 64/node，实为：{msg}"
    );
}

#[test]
fn xml_sanitize_error_display_missing_owo_ui_root() {
    let err = XmlSanitizeError::MissingOwoUiRoot;
    let msg = err.to_string();
    assert!(
        msg.contains("owo-ui"),
        "MissingOwoUiRoot Display 应含 owo-ui，实为：{msg}"
    );
}

// ── AgentUiSessionStore::take ─────────────────────────────────────────────

#[test]
fn session_store_take_existing() {
    let mut store = AgentUiSessionStore::default();
    let entity = dummy_entity(10);
    store.upsert(entity, make_session("req-1", 100));
    let taken = store.take(entity);
    assert!(taken.is_some(), "take 已存在 entity 的 session 应返回 Some");
    assert!(
        store.get(entity).is_none(),
        "take 后 store 中不应再有该 entity"
    );
}

#[test]
fn session_store_take_absent_returns_none() {
    let mut store = AgentUiSessionStore::default();
    let entity = dummy_entity(11);
    let result = store.take(entity);
    assert!(
        result.is_none(),
        "take 不存在的 entity 应返回 None，实为 {result:?}"
    );
}

// ── S2C 双向 emit 系统测试（fix-s2c-proto-panic：锁住 Redis + client-facing S2C 专属 channel）

use valence::protocol::packets::play::CustomPayloadS2c;
use valence::testing::MockClientHelper;

/// flush 所有 mock client packets（让 Client 内部缓冲写到 MockClientHelper）。
fn flush_all_clients(app: &mut App) {
    let world = app.world_mut();
    let mut q = world.query::<&mut valence::prelude::Client>();
    for mut c in q.iter_mut(world) {
        c.flush_packets().expect("mock client flush should succeed");
    }
}

/// 从 MockClientHelper 抽取 bong:agent_ui_request 专属 channel 的裸 JSON payloads。
///
/// payload 直接是 `AgentUiRequestPayloadV1` JSON（无 ServerDataV1 外层 envelope）。
/// 防回归：不从 bong:server_data channel 采集，确保修复后不走 proto 路径。
fn collect_agent_ui_request_payloads(
    app: &mut App,
    helper: &mut MockClientHelper,
) -> Vec<serde_json::Value> {
    flush_all_clients(app);
    let mut payloads = Vec::new();
    for frame in helper.collect_received().0 {
        let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
            continue;
        };
        if packet.channel.as_str() != AGENT_UI_REQUEST_CHANNEL {
            continue;
        }
        match serde_json::from_slice::<serde_json::Value>(packet.data.0 .0) {
            Ok(v) => payloads.push(v),
            Err(e) => panic!(
                "bong:agent_ui_request payload 无法反序列化为 JSON：{e}\nbytes={:?}",
                packet.data.0 .0
            ),
        }
    }
    payloads
}

/// 从 MockClientHelper 抽取 bong:agent_ui_request 专属 channel 的裸 payload 长度。
fn collect_agent_ui_request_payload_lengths(
    app: &mut App,
    helper: &mut MockClientHelper,
) -> Vec<usize> {
    flush_all_clients(app);
    let mut lengths = Vec::new();
    for frame in helper.collect_received().0 {
        let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
            continue;
        };
        if packet.channel.as_str() == AGENT_UI_REQUEST_CHANNEL {
            lengths.push(packet.data.0 .0.len());
        }
    }
    lengths
}

/// 从 MockClientHelper 抽取 bong:agent_ui_close 专属 channel 的裸 JSON payloads。
///
/// payload 直接是 `AgentUiClosePayloadV1` JSON（无 ServerDataV1 外层 envelope）。
fn collect_agent_ui_close_payloads(
    app: &mut App,
    helper: &mut MockClientHelper,
) -> Vec<serde_json::Value> {
    flush_all_clients(app);
    let mut payloads = Vec::new();
    for frame in helper.collect_received().0 {
        let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
            continue;
        };
        if packet.channel.as_str() != AGENT_UI_CLOSE_CHANNEL {
            continue;
        }
        match serde_json::from_slice::<serde_json::Value>(packet.data.0 .0) {
            Ok(v) => payloads.push(v),
            Err(e) => panic!(
                "bong:agent_ui_close payload 无法反序列化为 JSON：{e}\nbytes={:?}",
                packet.data.0 .0
            ),
        }
    }
    payloads
}

/// 防回归：AgentUiRequest/AgentUiClose 绝不通过 bong:server_data channel 发送。
/// 若未来代码意外地把 agent_ui 改回 server_data 路径，此测试立即撞红，
/// 防止 production proto_convert.rs unreachable!() panic 再次出现。
fn assert_no_agent_ui_on_server_data_channel(helper: &mut MockClientHelper) {
    use crate::network::agent_bridge::SERVER_DATA_CHANNEL;
    use crate::schema::proto_gen::bong;
    use prost::Message;

    for frame in helper.collect_received().0 {
        let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
            continue;
        };
        if packet.channel.as_str() != SERVER_DATA_CHANNEL {
            continue;
        }
        let envelope =
            bong::ServerDataEnvelope::decode(packet.data.0 .0).unwrap_or_else(|err| {
                panic!(
                    "bong:server_data channel 必须是 production proto envelope，不能依赖 test-only JSON decode；\
                     若这里出现 AgentUiRequest/AgentUiClose JSON，说明它们被错误路由到旧 server_data channel。\
                     decode error={err}; bytes={:?}",
                    packet.data.0 .0
                )
            });
        assert!(
            envelope.payload.is_some(),
            "bong:server_data proto envelope 不应为空；AgentUiRequest/AgentUiClose 必须走专属 \
             bong:agent_ui_request/bong:agent_ui_close channel"
        );
    }
}

/// happy path：session 创建 → 真向 client 发 AgentUiRequest S2C 包（专属 channel）。
///
/// 断言：
/// - Redis 无 error response
/// - S2C payload 经由 bong:agent_ui_request channel，request_id 对齐，xml 非空
/// - 防回归：bong:server_data channel 上无 agent_ui_request
#[test]
fn system_session_open_sends_agent_ui_request_s2c() {
    let (mut app, rx) = build_agent_ui_app();
    let (bundle, mut helper) = create_mock_client("Cultivator");
    let player = app.world_mut().spawn(bundle).id();
    app.world_mut().entity_mut(player).insert(Cultivation {
        realm: Realm::Condense,
        ..Cultivation::default()
    });

    let cmd = make_cmd("req-s2c-open", "Cultivator", 0);
    app.world_mut().send_event(AgentUiCmdEvent(cmd));
    app.update();

    // Redis 不应有 error（session 应成功创建）
    let redis_msgs: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
    let has_error = redis_msgs.iter().any(|m| {
        if let RedisOutbound::AgentUiResponse(r) = m {
            matches!(r.action, AgentUiActionType::Error)
        } else {
            false
        }
    });
    assert!(
        !has_error,
        "session 成功创建时不应有 Redis error response，实际 msgs={redis_msgs:?}"
    );

    // S2C：应经由 bong:agent_ui_request channel 发包（非 bong:server_data）
    let payloads = collect_agent_ui_request_payloads(&mut app, &mut helper);
    assert_eq!(
        payloads.len(),
        1,
        "session 创建后应向 client 发 1 条 bong:agent_ui_request S2C，实际 {} 条",
        payloads.len()
    );
    let req = &payloads[0];
    assert_eq!(
        req["request_id"].as_str(),
        Some("req-s2c-open"),
        "S2C agent_ui_request 的 request_id 应为 req-s2c-open，实为 {}",
        req["request_id"]
    );
    assert!(
        req["xml"].as_str().is_some_and(|x| !x.is_empty()),
        "S2C agent_ui_request 的 xml 字段应非空，实为 {}",
        req["xml"]
    );
    // 防回归：bong:server_data channel 上无 agent_ui_request
    assert_no_agent_ui_on_server_data_channel(&mut helper);
}

/// bug-hunt 回归：Rust 侧必须对齐 TS schema 的 request_id 长度上限。
///
/// 修复前 validate() 未检查 request_id 长度，超长 id 会进入真实 S2C 路径，
/// 在 bong:agent_ui_request 上生成超过 MAX_PAYLOAD_BYTES 的 JSON payload。
#[test]
fn system_rejects_oversized_request_id_before_agent_ui_request_s2c() {
    let (mut app, rx) = build_agent_ui_app();
    let (bundle, mut helper) = create_mock_client("OversizedId");
    let player = app.world_mut().spawn(bundle).id();
    app.world_mut().entity_mut(player).insert(Cultivation {
        realm: Realm::Condense,
        ..Cultivation::default()
    });

    let mut cmd = make_cmd(&"r".repeat(MAX_PAYLOAD_BYTES + 1), "OversizedId", 0);
    cmd.xml =
        r#"<owo-ui><components><flow-layout><label>ok</label></flow-layout></components></owo-ui>"#
            .to_string();
    app.world_mut().send_event(AgentUiCmdEvent(cmd));
    app.update();

    let s2c_lengths = collect_agent_ui_request_payload_lengths(&mut app, &mut helper);
    assert!(
        s2c_lengths.is_empty(),
        "超长 request_id 必须在 server validate 阶段拒绝，不应下发 bong:agent_ui_request；actual_lens={s2c_lengths:?}, max={MAX_PAYLOAD_BYTES}"
    );

    let redis_msgs: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
    let has_invalid_command = redis_msgs.iter().any(|msg| {
        matches!(
            msg,
            RedisOutbound::AgentUiResponse(resp)
                if matches!(resp.action, AgentUiActionType::Error)
                    && resp.params.get("reason").map(String::as_str) == Some("invalid_command")
        )
    });
    assert!(
        has_invalid_command,
        "超长 request_id 应返回 invalid_command，而不是静默丢弃；msgs={redis_msgs:?}"
    );
}

/// wire JSON 结构 pin：bong:agent_ui_request payload 是裸 AgentUiRequestPayloadV1 JSON，
/// 无 ServerDataV1 外层 envelope（无 "v"/"type" 包装字段）。
/// client 侧 BongNetworkHandler.registerAgentUiChannels() 直接解析这个结构。
#[test]
fn agent_ui_request_wire_json_is_bare_payload_v1() {
    let (mut app, _rx) = build_agent_ui_app();
    let (bundle, mut helper) = create_mock_client("WireTest");
    let player = app.world_mut().spawn(bundle).id();
    app.world_mut().entity_mut(player).insert(Cultivation {
        realm: Realm::Condense,
        ..Cultivation::default()
    });

    let cmd = make_cmd("req-wire-pin", "WireTest", 0);
    app.world_mut().send_event(AgentUiCmdEvent(cmd));
    app.update();

    let payloads = collect_agent_ui_request_payloads(&mut app, &mut helper);
    assert_eq!(
        payloads.len(),
        1,
        "应收到 1 条 bong:agent_ui_request payload"
    );
    let value = &payloads[0];

    // 裸 AgentUiRequestPayloadV1：应有 request_id/target_player/xml/timeout_ticks
    assert!(
        value.get("request_id").is_some(),
        "裸 AgentUiRequestPayloadV1 必须包含 'request_id'；当前 JSON={value}"
    );
    assert!(
        value.get("xml").is_some(),
        "裸 AgentUiRequestPayloadV1 必须包含 'xml'；当前 JSON={value}"
    );
    assert!(
        value.get("timeout_ticks").is_some(),
        "裸 AgentUiRequestPayloadV1 必须包含 'timeout_ticks'；当前 JSON={value}"
    );
    // 绝对不应有 ServerDataV1 外层 envelope 字段（否则 client 解析结构错误）
    assert!(
        value.get("v").is_none(),
        "payload 不应包含 ServerDataV1 envelope 的 'v' 版本字段；当前 JSON={value}"
    );
    assert!(
        value.get("type").is_none(),
        "payload 不应包含 ServerDataV1 envelope 的 'type' 字段；当前 JSON={value}"
    );
}

/// Replaced：新 cmd 替换旧 session → client 收 bong:agent_ui_close + Redis replaced；
/// 新 session → client 再收 bong:agent_ui_request（双向 emit 完整链路锁住）。
#[test]
fn system_replaced_sends_agent_ui_close_then_request_s2c() {
    let (mut app, rx) = build_agent_ui_app();
    let (bundle, mut helper) = create_mock_client("Cultivator2");
    let player = app.world_mut().spawn(bundle).id();
    app.world_mut().entity_mut(player).insert(Cultivation {
        realm: Realm::Condense,
        ..Cultivation::default()
    });

    // 第一条 cmd → 建立 Open session
    let cmd1 = make_cmd("req-old-s2c", "Cultivator2", 0);
    app.world_mut().send_event(AgentUiCmdEvent(cmd1));
    app.update();
    // 清掉第一轮 Redis 和 S2C
    while rx.try_recv().is_ok() {}
    // 消费掉第一轮 S2C（agent_ui_request）
    flush_all_clients(&mut app);
    let _ = helper.collect_received();

    // 第二条 cmd → Replaced
    let cmd2 = make_cmd("req-new-s2c", "Cultivator2", 0);
    app.world_mut().send_event(AgentUiCmdEvent(cmd2));
    app.update();

    // Redis 应有 replaced response for req-old-s2c
    let redis_msgs: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
    let replaced = redis_msgs.iter().find(|m| {
        if let RedisOutbound::AgentUiResponse(r) = m {
            r.request_id == "req-old-s2c" && matches!(r.action, AgentUiActionType::Replaced)
        } else {
            false
        }
    });
    assert!(
        replaced.is_some(),
        "Replaced 时 Redis 应有 request_id=req-old-s2c 的 replaced response，实为 {redis_msgs:?}"
    );

    // S2C：一次 flush + collect，同时检查 close 和 request（两者在同一 update 轮产生）。
    // 注意：collect_agent_ui_close_payloads/collect_agent_ui_request_payloads 各自会
    // flush + collect_received，消费性调用不可连用（第二次 collect 为空）。
    // 此处直接 flush 一次，收集所有帧，按 channel 分类。
    flush_all_clients(&mut app);
    let mut close_payloads: Vec<serde_json::Value> = Vec::new();
    let mut request_payloads: Vec<serde_json::Value> = Vec::new();
    for frame in helper.collect_received().0 {
        let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
            continue;
        };
        if packet.channel.as_str() == AGENT_UI_CLOSE_CHANNEL {
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(packet.data.0 .0) {
                close_payloads.push(v);
            }
        } else if packet.channel.as_str() == AGENT_UI_REQUEST_CHANNEL {
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(packet.data.0 .0) {
                request_payloads.push(v);
            }
        }
    }

    assert_eq!(
        close_payloads.len(),
        1,
        "Replaced 时 client 应收 1 条 bong:agent_ui_close S2C，实际 {}",
        close_payloads.len()
    );
    assert_eq!(
        close_payloads[0]["request_id"].as_str(),
        Some("req-old-s2c"),
        "AgentUiClose 的 request_id 应为 req-old-s2c（被替换的旧 session），实为 {}",
        close_payloads[0]["request_id"]
    );
    assert_eq!(
        request_payloads.len(),
        1,
        "Replaced 后 client 应收 1 条 bong:agent_ui_request S2C（新 session），实际 {}",
        request_payloads.len()
    );
    assert_eq!(
        request_payloads[0]["request_id"].as_str(),
        Some("req-new-s2c"),
        "新 AgentUiRequest 的 request_id 应为 req-new-s2c，实为 {}",
        request_payloads[0]["request_id"]
    );
}

/// session_expired：玩家响应时 session 已不存在 → client 收 bong:agent_ui_close S2C。
#[test]
fn system_session_expired_sends_agent_ui_close_s2c() {
    let (mut app, rx) = build_agent_ui_app();
    let (bundle, mut helper) = create_mock_client("Cultivator3");
    let player = app.world_mut().spawn(bundle).id();
    app.world_mut().entity_mut(player).insert(Cultivation {
        realm: Realm::Condense,
        ..Cultivation::default()
    });

    // 不建立 session，直接发 response → 触发 session_expired close
    app.world_mut().send_event(AgentUiResponseEvent {
        player,
        request_id: "req-expired".to_string(),
        action: AgentUiActionType::ButtonClick,
        params: [("button_id".to_string(), "dismiss".to_string())]
            .into_iter()
            .collect(),
    });
    app.update();

    // Redis 不应有任何 AgentUiResponse（session_expired 走 close，不 forward）
    let redis_msgs: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
    let has_response = redis_msgs
        .iter()
        .any(|m| matches!(m, RedisOutbound::AgentUiResponse(_)));
    assert!(
        !has_response,
        "session_expired 时 Redis 不应转发 response，实为 {redis_msgs:?}"
    );

    // S2C：client 应经由 bong:agent_ui_close channel 收到 close payload
    let payloads = collect_agent_ui_close_payloads(&mut app, &mut helper);
    assert_eq!(
        payloads.len(),
        1,
        "session_expired 时 client 应收 1 条 bong:agent_ui_close S2C，实际 {}",
        payloads.len()
    );
    let close = &payloads[0];
    assert_eq!(
        close["request_id"].as_str(),
        Some("req-expired"),
        "AgentUiClose 的 request_id 应为 req-expired，实为 {}",
        close["request_id"]
    );
    assert_eq!(
        close["reason"].as_str(),
        Some("session_expired"),
        "AgentUiClose 的 reason 应为 session_expired，实为 {}",
        close["reason"]
    );
    // 防回归：bong:server_data channel 上无 agent_ui_close
    assert_no_agent_ui_on_server_data_channel(&mut helper);
}

/// 防回归：agent_ui_request/close 不经由 bong:server_data channel 发送。
/// 若未来有人误将 AgentUi 改回 server_data 路径，此测试立即撞红，
/// 防止 production proto_convert.rs unreachable!() panic 再次出现。
#[test]
fn agent_ui_request_not_sent_on_server_data_channel() {
    let (mut app, _rx) = build_agent_ui_app();
    let (bundle, mut helper) = create_mock_client("AntiReg");
    let player = app.world_mut().spawn(bundle).id();
    app.world_mut().entity_mut(player).insert(Cultivation {
        realm: Realm::Condense,
        ..Cultivation::default()
    });

    let cmd = make_cmd("req-antireg", "AntiReg", 0);
    app.world_mut().send_event(AgentUiCmdEvent(cmd));
    app.update();
    flush_all_clients(&mut app);
    // assert_no_agent_ui_on_server_data_channel 是消费性的；
    // 确保此处独立 collect_received 而非依赖之前调用。
    assert_no_agent_ui_on_server_data_channel(&mut helper);
}
