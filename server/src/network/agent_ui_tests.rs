use super::*;

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
