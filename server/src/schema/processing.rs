//! plan-lingtian-process-v1 §5.3 — 作物加工 IPC 镜像。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessingKindV1 {
    Drying,
    Grinding,
    ForgingAlchemy,
    Extraction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessingSessionDataV1 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<bool>,
    pub session_id: String,
    pub kind: ProcessingKindV1,
    pub recipe_id: String,
    pub progress_ticks: u32,
    pub duration_ticks: u32,
    pub player_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FreshnessUpdateV1 {
    pub item_uuid: String,
    pub freshness: f32,
    pub profile_name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn processing_session_data_roundtrip_uses_snake_case_kind() {
        let payload = ProcessingSessionDataV1 {
            active: Some(true),
            session_id: "processing:1".to_string(),
            kind: ProcessingKindV1::ForgingAlchemy,
            recipe_id: "forge_ci_she_hao".to_string(),
            progress_ticks: 10,
            duration_ticks: 6_000,
            player_id: "offline:Azure".to_string(),
        };
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["kind"], "forging_alchemy");
        let decoded: ProcessingSessionDataV1 = serde_json::from_value(json).unwrap();
        assert_eq!(decoded, payload);
    }

    #[test]
    fn freshness_update_bounds_are_runtime_validated_by_callers() {
        let payload = FreshnessUpdateV1 {
            item_uuid: "item:7".to_string(),
            freshness: 0.42,
            profile_name: "drying_v1".to_string(),
        };
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["profile_name"], "drying_v1");
    }
}
