//! plan-anticheat-v1 IPC schema. 与 `agent/packages/schema/src/anticheat.ts` 对齐。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViolationKindV1 {
    ReachExceeded,
    CooldownBypassed,
    QiInvestExceeded,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AntiCheatReportV1 {
    pub v: u8,
    #[serde(rename = "type")]
    pub message_type: String,
    pub char_id: String,
    pub entity_id: u64,
    pub at_tick: u64,
    pub kind: ViolationKindV1,
    pub count: u32,
    pub details: String,
}

impl AntiCheatReportV1 {
    pub fn new(
        char_id: impl Into<String>,
        entity_id: u64,
        at_tick: u64,
        kind: ViolationKindV1,
        count: u32,
        details: impl Into<String>,
    ) -> Self {
        Self {
            v: 1,
            message_type: "anticheat_report".to_string(),
            char_id: char_id.into(),
            entity_id,
            at_tick,
            kind,
            count,
            details: details.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anticheat_report_sample_roundtrips() {
        let report: AntiCheatReportV1 = serde_json::from_str(include_str!(
            "../../../agent/packages/schema/samples/anticheat-report.sample.json"
        ))
        .expect("sample should deserialize");

        assert_eq!(report.v, 1);
        assert_eq!(report.message_type, "anticheat_report");
        assert_eq!(report.char_id, "offline:Azure");
        assert_eq!(report.entity_id, 42);
        assert_eq!(report.kind, ViolationKindV1::ReachExceeded);

        let json = serde_json::to_string(&report).expect("serialize anticheat report");
        let back: AntiCheatReportV1 =
            serde_json::from_str(json.as_str()).expect("deserialize anticheat report");
        assert_eq!(back, report);
    }
}
