use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CombatRealtimeKindV1 {
    CombatEvent,
    DeathEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CombatRealtimeEventV1 {
    pub v: u8,
    pub kind: CombatRealtimeKindV1,
    pub tick: u64,
    pub target_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attacker_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CombatSummaryV1 {
    pub v: u8,
    pub window_start_tick: u64,
    pub window_end_tick: u64,
    pub combat_event_count: u64,
    pub death_event_count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn realtime_and_summary_roundtrip() {
        let realtime = CombatRealtimeEventV1 {
            v: 1,
            kind: CombatRealtimeKindV1::CombatEvent,
            tick: 42,
            target_id: "offline:Crimson".to_string(),
            attacker_id: Some("offline:Azure".to_string()),
            description: Some("debug_attack_intent offline:Azure -> offline:Crimson".to_string()),
            cause: None,
        };
        let realtime_json = serde_json::to_string(&realtime).expect("serialize realtime");
        let realtime_back: CombatRealtimeEventV1 =
            serde_json::from_str(realtime_json.as_str()).expect("deserialize realtime");
        assert_eq!(realtime_back.v, 1);
        assert_eq!(realtime_back.tick, 42);

        let summary = CombatSummaryV1 {
            v: 1,
            window_start_tick: 201,
            window_end_tick: 400,
            combat_event_count: 12,
            death_event_count: 3,
        };
        let summary_json = serde_json::to_string(&summary).expect("serialize summary");
        let summary_back: CombatSummaryV1 =
            serde_json::from_str(summary_json.as_str()).expect("deserialize summary");
        assert_eq!(summary_back.window_start_tick, 201);
        assert_eq!(summary_back.window_end_tick, 400);
    }
}
