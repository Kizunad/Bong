//! plan-agent-ui-data-v1 P0 — 天道 UI-as-Data Rust serde 镜像。
//!
//! 与 TypeScript `agent/packages/schema/src/payloads/agent-ui.ts` 1:1 对应。
//! 字段不混用：三个 schema 各自独立（AgentUiRequestCommandV1 / AgentUiRequestPayloadV1 /
//! AgentUiResponsePayloadV1）。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ─── Action 字面联合 ─────────────────────────────────────────────────────────

/// 天道 UI 面板交互动作枚举，与 TypeScript `AgentUiActionType` 1:1。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentUiActionType {
    /// 玩家点击了一个按钮。
    ButtonClick,
    /// 玩家主动关闭面板（ESC）。
    Dismissed,
    /// server ticker 超时（权威来源）。
    Timeout,
    /// 同一玩家的新请求替换了此 session。
    Replaced,
    /// 校验错误（allowed_button_ids / 离线 / realm_gate）。
    Error,
    /// client OwoUI 解析 XML 失败。
    ParseError,
}

// ─── Schema 1：Agent → Server（Redis bong:agent_ui_cmd）──────────────────────

/// 天道 Agent 向 server 发布的 UI 指令（Redis bong:agent_ui_cmd）。
///
/// 安全字段 `realm_gate` / `allowed_button_ids` 仅存在于此 schema，
/// 绝不下发给 client（见 `AgentUiRequestPayloadV1`）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentUiRequestCommandV1 {
    pub request_id: String,
    pub target_player: String,
    pub xml: String,
    /// 超时时间（ticks），范围 20..=2400，默认 600。
    pub timeout_ticks: u32,
    /// 境界门控（1-indexed rank）：0=不门控，1=醒灵+，..，5=通灵+。
    pub realm_gate: u8,
    /// 允许的按钮 ID 白名单，最多 16 条。不下发给 client。
    pub allowed_button_ids: Vec<String>,
}

impl AgentUiRequestCommandV1 {
    /// 校验：realm_gate ∈ [0,5] && allowed_button_ids.len() ≤ 16 && timeout_ticks ∈ [20,2400]
    pub fn validate(&self) -> Result<(), String> {
        if self.realm_gate > 5 {
            return Err(format!("realm_gate={} 超出范围 0..=5", self.realm_gate));
        }
        if self.allowed_button_ids.len() > 16 {
            return Err(format!(
                "allowed_button_ids 长度 {} 超出上限 16",
                self.allowed_button_ids.len()
            ));
        }
        if self.timeout_ticks < 20 || self.timeout_ticks > 2400 {
            return Err(format!(
                "timeout_ticks={} 不在范围 20..=2400",
                self.timeout_ticks
            ));
        }
        if self.xml.len() > 8192 {
            return Err(format!("xml 长度 {} 超出 8192 字节上限", self.xml.len()));
        }
        Ok(())
    }
}

// ─── Schema 2：Server → Client（bong:server_data 变体 agent_ui_request）───────

/// server 下发给 client 的动态 UI 面板请求（bong:server_data 变体 agent_ui_request）。
///
/// 不含 `realm_gate` / `allowed_button_ids`（安全字段仅在 server 内部处理）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentUiRequestPayloadV1 {
    pub request_id: String,
    pub target_player: String,
    pub xml: String,
    /// 超时时间（ticks），范围 20..=2400。
    pub timeout_ticks: u32,
}

// ─── Schema 3：Client → Server CustomPayload + Server → Agent Redis ───────────

/// 玩家面板交互响应（C2S CustomPayload + server→agent Redis）。
///
/// `params` 为 `HashMap<String,String>` 以保留可扩展性：
///   - `button_click` → `params["button_id"] = "<id>"`
///   - `error` → `params["reason"] = "realm_gate_rejected"` 等
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentUiResponsePayloadV1 {
    pub request_id: String,
    pub action: AgentUiActionType,
    pub params: HashMap<String, String>,
}

// ─── Schema 4：Server → Client close 信号（bong:server_data 变体 agent_ui_close）──

/// server 向 client 发送关闭信号（bong:server_data 变体 agent_ui_close）。
///
/// `reason` 为 None = Replaced（client 关闭不发任何 response）；
/// `reason` = "invalid_button_id" | "session_expired" 时 client 显示提示后关闭。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentUiClosePayloadV1 {
    pub request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

// ─── 单测 ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── AgentUiRequestCommandV1 roundtrip ────────────────────────────────────

    #[test]
    fn agent_ui_request_command_roundtrip() {
        let json = r#"{
            "request_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
            "target_player": "b2c3d4e5-f6a7-8901-bcde-f01234567891",
            "xml": "<flow-layout><button id=\"enter_realm\">踏入</button></flow-layout>",
            "timeout_ticks": 600,
            "realm_gate": 3,
            "allowed_button_ids": ["enter_realm", "dismiss"]
        }"#;
        let cmd: AgentUiRequestCommandV1 =
            serde_json::from_str(json).expect("AgentUiRequestCommandV1 应能反序列化正样本");
        assert_eq!(cmd.request_id, "a1b2c3d4-e5f6-7890-abcd-ef1234567890");
        assert_eq!(cmd.realm_gate, 3);
        assert_eq!(cmd.allowed_button_ids, vec!["enter_realm", "dismiss"]);
        assert!(
            cmd.validate().is_ok(),
            "正样本应通过 validate()，错误：{:?}",
            cmd.validate()
        );
    }

    #[test]
    fn agent_ui_request_command_realm_gate_zero_passes() {
        let json = r#"{
            "request_id": "test",
            "target_player": "test",
            "xml": "x",
            "timeout_ticks": 600,
            "realm_gate": 0,
            "allowed_button_ids": []
        }"#;
        let cmd: AgentUiRequestCommandV1 = serde_json::from_str(json).unwrap();
        assert!(
            cmd.validate().is_ok(),
            "realm_gate=0 应通过校验，实为不门控"
        );
    }

    #[test]
    fn agent_ui_request_command_realm_gate_five_passes() {
        let json = r#"{
            "request_id": "test",
            "target_player": "test",
            "xml": "x",
            "timeout_ticks": 600,
            "realm_gate": 5,
            "allowed_button_ids": ["dismiss"]
        }"#;
        let cmd: AgentUiRequestCommandV1 = serde_json::from_str(json).unwrap();
        assert!(cmd.validate().is_ok(), "realm_gate=5（通灵+）应通过校验");
    }

    #[test]
    fn agent_ui_request_command_realm_gate_six_rejected() {
        let json = r#"{
            "request_id": "test",
            "target_player": "test",
            "xml": "x",
            "timeout_ticks": 600,
            "realm_gate": 6,
            "allowed_button_ids": []
        }"#;
        let cmd: AgentUiRequestCommandV1 = serde_json::from_str(json).unwrap();
        let err = cmd
            .validate()
            .expect_err("realm_gate=6 超出范围，应被 validate() 拒绝");
        assert!(
            err.contains("realm_gate=6"),
            "错误信息应包含 realm_gate=6，实为：{err}"
        );
    }

    #[test]
    fn agent_ui_request_command_allowed_button_ids_16_passes() {
        let ids: Vec<String> = (0..16).map(|i| format!("btn_{i}")).collect();
        let cmd = AgentUiRequestCommandV1 {
            request_id: "test".into(),
            target_player: "test".into(),
            xml: "x".into(),
            timeout_ticks: 600,
            realm_gate: 0,
            allowed_button_ids: ids,
        };
        assert!(
            cmd.validate().is_ok(),
            "allowed_button_ids 16 条（边界最大值）应通过 validate()"
        );
    }

    #[test]
    fn agent_ui_request_command_allowed_button_ids_17_rejected() {
        let ids: Vec<String> = (0..17).map(|i| format!("btn_{i}")).collect();
        let cmd = AgentUiRequestCommandV1 {
            request_id: "test".into(),
            target_player: "test".into(),
            xml: "x".into(),
            timeout_ticks: 600,
            realm_gate: 0,
            allowed_button_ids: ids,
        };
        let err = cmd
            .validate()
            .expect_err("allowed_button_ids 第 17 条应被 validate() 拒绝");
        assert!(err.contains("17"), "错误信息应包含长度 17，实为：{err}");
    }

    #[test]
    fn agent_ui_request_command_timeout_ticks_boundary() {
        let make = |timeout_ticks: u32| AgentUiRequestCommandV1 {
            request_id: "test".into(),
            target_player: "test".into(),
            xml: "x".into(),
            timeout_ticks,
            realm_gate: 0,
            allowed_button_ids: vec![],
        };
        assert!(
            make(20).validate().is_ok(),
            "timeout_ticks=20（边界最小）应通过"
        );
        assert!(
            make(2400).validate().is_ok(),
            "timeout_ticks=2400（边界最大）应通过"
        );
        assert!(
            make(19).validate().is_err(),
            "timeout_ticks=19 应被拒绝（低于 minimum:20）"
        );
        assert!(
            make(2401).validate().is_err(),
            "timeout_ticks=2401 应被拒绝（超出 maximum:2400）"
        );
    }

    #[test]
    fn agent_ui_request_command_xml_too_large_rejected() {
        let big_xml = "x".repeat(8193);
        let cmd = AgentUiRequestCommandV1 {
            request_id: "test".into(),
            target_player: "test".into(),
            xml: big_xml,
            timeout_ticks: 600,
            realm_gate: 0,
            allowed_button_ids: vec![],
        };
        assert!(
            cmd.validate().is_err(),
            "xml 超 8192 字节应被 validate() 拒绝"
        );
    }

    // ── AgentUiResponsePayloadV1 roundtrip ───────────────────────────────────

    #[test]
    fn agent_ui_response_button_click_roundtrip() {
        let json = r#"{
            "request_id": "a1b2c3d4",
            "action": "button_click",
            "params": {"button_id": "enter_realm"}
        }"#;
        let resp: AgentUiResponsePayloadV1 =
            serde_json::from_str(json).expect("button_click 应能反序列化");
        assert!(
            matches!(resp.action, AgentUiActionType::ButtonClick),
            "action 应为 ButtonClick，实为 {:?}",
            resp.action
        );
        assert_eq!(
            resp.params.get("button_id"),
            Some(&"enter_realm".to_string()),
            "params.button_id 应为 enter_realm"
        );
    }

    #[test]
    fn agent_ui_response_dismissed_roundtrip() {
        let json = r#"{"request_id":"x","action":"dismissed","params":{}}"#;
        let resp: AgentUiResponsePayloadV1 =
            serde_json::from_str(json).expect("dismissed 应能反序列化");
        assert!(
            matches!(resp.action, AgentUiActionType::Dismissed),
            "action 应为 Dismissed，实为 {:?}",
            resp.action
        );
    }

    #[test]
    fn agent_ui_response_all_actions_roundtrip() {
        let actions = [
            ("button_click", AgentUiActionType::ButtonClick),
            ("dismissed", AgentUiActionType::Dismissed),
            ("timeout", AgentUiActionType::Timeout),
            ("replaced", AgentUiActionType::Replaced),
            ("error", AgentUiActionType::Error),
            ("parse_error", AgentUiActionType::ParseError),
        ];
        for (wire, expected) in &actions {
            let json = format!(r#"{{"request_id":"x","action":"{wire}","params":{{}}}}"#);
            let resp: AgentUiResponsePayloadV1 = serde_json::from_str(&json)
                .unwrap_or_else(|e| panic!("action={wire} 应能反序列化，错误：{e}"));
            assert_eq!(
                resp.action, *expected,
                "action={wire} 反序列化后应等于 {expected:?}"
            );
        }
    }

    #[test]
    fn agent_ui_response_error_realm_gate_rejected_params() {
        let json = r#"{
            "request_id": "test",
            "action": "error",
            "params": {
                "reason": "realm_gate_rejected",
                "player_realm": "2",
                "required_realm": "3"
            }
        }"#;
        let resp: AgentUiResponsePayloadV1 =
            serde_json::from_str(json).expect("error+realm_gate_rejected 应能反序列化");
        assert_eq!(
            resp.params.get("reason"),
            Some(&"realm_gate_rejected".to_string()),
            "params.reason 应为 realm_gate_rejected"
        );
        assert_eq!(
            resp.params.get("player_realm"),
            Some(&"2".to_string()),
            "params.player_realm 应为 2"
        );
    }

    #[test]
    fn agent_ui_response_invalid_action_rejected() {
        let json = r#"{"request_id":"x","action":"invalid_action","params":{}}"#;
        let result: Result<AgentUiResponsePayloadV1, _> = serde_json::from_str(json);
        assert!(
            result.is_err(),
            "非法 action 字面量应反序列化失败，得到 Ok 变体"
        );
    }

    // ── AgentUiClosePayloadV1 roundtrip ──────────────────────────────────────

    #[test]
    fn agent_ui_close_with_reason_roundtrip() {
        let json = r#"{"request_id":"x","reason":"invalid_button_id"}"#;
        let close: AgentUiClosePayloadV1 =
            serde_json::from_str(json).expect("close+reason 应能反序列化");
        assert_eq!(
            close.reason,
            Some("invalid_button_id".to_string()),
            "reason 应为 invalid_button_id"
        );
        // round-trip 应含 reason 字段
        let serialized = serde_json::to_string(&close).unwrap();
        let val: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert_eq!(
            val["reason"],
            serde_json::Value::String("invalid_button_id".into()),
            "序列化结果应含 reason 字段"
        );
    }

    #[test]
    fn agent_ui_close_replaced_no_reason_roundtrip() {
        let json = r#"{"request_id":"x"}"#;
        let close: AgentUiClosePayloadV1 =
            serde_json::from_str(json).expect("Replaced（无 reason）应能反序列化");
        assert!(
            close.reason.is_none(),
            "reason 省略时应为 None，实为 {:?}",
            close.reason
        );
        // round-trip 应不含 reason 字段（skip_serializing_if = "Option::is_none"）
        let serialized = serde_json::to_string(&close).unwrap();
        let val: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert!(
            val.get("reason").is_none(),
            "Replaced close 序列化结果不应含 reason 字段，实为：{serialized}"
        );
    }

    // ── AgentUiActionType 变体枚举 pin 测试 ─────────────────────────────────

    #[test]
    fn agent_ui_action_type_wire_format_pin() {
        // 锁定每个 enum 变体的 wire 字符串，防止 rename 导致双端不对齐。
        let cases: &[(&str, AgentUiActionType)] = &[
            ("button_click", AgentUiActionType::ButtonClick),
            ("dismissed", AgentUiActionType::Dismissed),
            ("timeout", AgentUiActionType::Timeout),
            ("replaced", AgentUiActionType::Replaced),
            ("error", AgentUiActionType::Error),
            ("parse_error", AgentUiActionType::ParseError),
        ];
        for (wire, variant) in cases {
            // 序列化
            let serialized = serde_json::to_string(variant).unwrap();
            assert_eq!(
                serialized,
                format!("\"{}\"", wire),
                "AgentUiActionType::{variant:?} 序列化应为 \"{wire}\"，实为 {serialized}"
            );
            // 反序列化
            let deserialized: AgentUiActionType = serde_json::from_str(&format!("\"{}\"", wire))
                .unwrap_or_else(|e| panic!("\"{}\" 应能反序列化：{e}", wire));
            assert_eq!(
                deserialized, *variant,
                "\"{}\" 反序列化应为 {:?}",
                wire, variant
            );
        }
    }
}
