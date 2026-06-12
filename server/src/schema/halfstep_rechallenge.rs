//! plan-halfstep-rechallenge-integration-v1 P1 — HalfStepRechallengeTriggerPayloadV1 IPC schema.
//!
//! server → agent Redis payload（`bong:tribulation/halfstep_rechallenge`，`CH_HALFSTEP_RECHALLENGE`）。
//!
//! agent 订阅后按 `zone_halfstep_count` 路由：
//! - player scope（常规）：「你感到曾遭封压的经脉微微松动，或许时机已到。」
//! - zone scope（仅 zone_halfstep_count >= 2）：「虚空中某处的修士收到了相同的消息。」

use serde::{Deserialize, Serialize};

/// 半步化虚重渡触发 payload（server → agent，`bong:tribulation/halfstep_rechallenge`）。
///
/// ## 字段说明
/// - `char_id`：触发方玩家/NPC 的 char_id（agent 用于 player-scope narration target）
/// - `zone_name`：触发方当前所在 zone 名称（agent 用于 zone-scope narration target）
/// - `zone_halfstep_count`：当前同 zone 内 HalfStep 修士总数（含本次触发方；
///   agent 据此决定是否 emit zone echo narration，门槛 >= 2）
/// - `at_tick`：server 游戏 tick（agent 时序审计用）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HalfStepRechallengeTriggerPayloadV1 {
    /// 触发方 char_id（玩家 = "offline:<name>"；dormant NPC = char_id string）。
    pub char_id: String,
    /// 触发方所在 zone 名称（ZoneRegistry zone.name；无法解析时 fallback "spawn"）。
    pub zone_name: String,
    /// 同 zone 当前 HalfStep 修士总数（含本次触发方，agent 门槛判断用）。
    pub zone_halfstep_count: u32,
    /// server 游戏 tick（时序审计）。
    pub at_tick: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn halfstep_rechallenge_trigger_payload_serializes_correctly() {
        let payload = HalfStepRechallengeTriggerPayloadV1 {
            char_id: "offline:Azure".to_string(),
            zone_name: "qingyun_peaks".to_string(),
            zone_halfstep_count: 1,
            at_tick: 12345,
        };
        let json = serde_json::to_string(&payload).expect("serialize should succeed");
        let deserialized: HalfStepRechallengeTriggerPayloadV1 =
            serde_json::from_str(&json).expect("deserialize should succeed");
        assert_eq!(deserialized, payload);
    }

    #[test]
    fn halfstep_rechallenge_trigger_payload_deny_unknown_fields() {
        // serde deny_unknown_fields：额外字段应拒绝
        let bad_json = r#"{"char_id":"offline:Azure","zone_name":"spawn","zone_halfstep_count":1,"at_tick":1,"extra_field":"should_fail"}"#;
        let result = serde_json::from_str::<HalfStepRechallengeTriggerPayloadV1>(bad_json);
        assert!(
            result.is_err(),
            "deny_unknown_fields should reject extra fields in HalfStepRechallengeTriggerPayloadV1"
        );
    }

    #[test]
    fn halfstep_rechallenge_trigger_payload_zone_halfstep_count_boundary() {
        // zone_halfstep_count = 0（极端边界，理论上不会发生但 schema 不约束）
        let payload = HalfStepRechallengeTriggerPayloadV1 {
            char_id: "offline:Azure".to_string(),
            zone_name: "spawn".to_string(),
            zone_halfstep_count: 0,
            at_tick: 0,
        };
        let json = serde_json::to_string(&payload).expect("serialize should succeed");
        let back: HalfStepRechallengeTriggerPayloadV1 =
            serde_json::from_str(&json).expect("deserialize should succeed");
        assert_eq!(back.zone_halfstep_count, 0);
    }
}
