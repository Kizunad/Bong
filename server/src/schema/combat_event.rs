use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CombatBodyPartV1 {
    Head,
    Chest,
    Abdomen,
    ArmL,
    ArmR,
    LegL,
    LegR,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CombatWoundKindV1 {
    Cut,
    Blunt,
    Pierce,
    Burn,
    Concussion,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CombatRealtimeKindV1 {
    CombatEvent,
    DeathEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CombatDefenseKindV1 {
    JieMai,
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
    pub body_part: Option<CombatBodyPartV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wound_kind: Option<CombatWoundKindV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub damage: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contam_delta: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defense_kind: Option<CombatDefenseKindV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defense_effectiveness: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defense_contam_reduced: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defense_wound_severity: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CombatSummaryV1 {
    pub v: u8,
    pub window_start_tick: u64,
    pub window_end_tick: u64,
    pub combat_event_count: u64,
    pub death_event_count: u64,
    pub damage_total: f32,
    pub contam_delta_total: f64,
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
            body_part: Some(CombatBodyPartV1::Chest),
            wound_kind: Some(CombatWoundKindV1::Blunt),
            damage: Some(20.0),
            contam_delta: Some(5.0),
            description: Some("debug_attack_intent offline:Azure -> offline:Crimson".to_string()),
            cause: None,
            defense_kind: Some(CombatDefenseKindV1::JieMai),
            defense_effectiveness: Some(0.7),
            defense_contam_reduced: Some(4.0),
            defense_wound_severity: Some(0.4),
        };
        let realtime_json = serde_json::to_string(&realtime).expect("serialize realtime");
        let realtime_back: CombatRealtimeEventV1 =
            serde_json::from_str(realtime_json.as_str()).expect("deserialize realtime");
        assert_eq!(realtime_back.v, 1);
        assert_eq!(realtime_back.tick, 42);
        assert_eq!(realtime_back.body_part, Some(CombatBodyPartV1::Chest));
        assert_eq!(realtime_back.wound_kind, Some(CombatWoundKindV1::Blunt));
        assert_eq!(realtime_back.damage, Some(20.0));
        assert_eq!(
            realtime_back.defense_kind,
            Some(CombatDefenseKindV1::JieMai)
        );
        assert_eq!(realtime_back.defense_effectiveness, Some(0.7));

        let summary = CombatSummaryV1 {
            v: 1,
            window_start_tick: 201,
            window_end_tick: 400,
            combat_event_count: 12,
            death_event_count: 3,
            damage_total: 64.0,
            contam_delta_total: 12.5,
        };
        let summary_json = serde_json::to_string(&summary).expect("serialize summary");
        let summary_back: CombatSummaryV1 =
            serde_json::from_str(summary_json.as_str()).expect("deserialize summary");
        assert_eq!(summary_back.window_start_tick, 201);
        assert_eq!(summary_back.window_end_tick, 400);
        assert_eq!(summary_back.damage_total, 64.0);
        assert_eq!(summary_back.contam_delta_total, 12.5);
    }
}
