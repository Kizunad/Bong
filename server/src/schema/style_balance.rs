use serde::{Deserialize, Serialize};

use crate::cultivation::components::ColorKind;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StyleTelemetryColorSnapshotV1 {
    pub main: ColorKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secondary: Option<ColorKind>,
    pub is_chaotic: bool,
    pub is_hunyuan: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StyleBalanceTelemetryEventV1 {
    pub v: u8,
    pub attacker_player_id: String,
    pub defender_player_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attacker_color: Option<StyleTelemetryColorSnapshotV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defender_color: Option<StyleTelemetryColorSnapshotV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attacker_style: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defender_style: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attacker_rejection_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defender_resistance: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defender_drain_affinity: Option<f64>,
    /// Injected qi before distance attenuation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attacker_qi: Option<f64>,
    /// Collision distance in blocks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distance_blocks: Option<f64>,
    /// Post-rejection hit value before defender mitigation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_hit: Option<f64>,
    /// Qi lost by the defender after mitigation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defender_lost: Option<f64>,
    /// Qi absorbed by defender drain affinity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defender_absorbed: Option<f64>,
    pub cause: String,
    pub resolved_at_tick: u64,
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::*;

    #[test]
    fn serializes_optional_physics_fields_without_breaking_color_snapshots() {
        let payload = StyleBalanceTelemetryEventV1 {
            v: 1,
            attacker_player_id: "offline:Azure".to_string(),
            defender_player_id: "offline:Crimson".to_string(),
            attacker_color: Some(StyleTelemetryColorSnapshotV1 {
                main: ColorKind::Heavy,
                secondary: Some(ColorKind::Solid),
                is_chaotic: false,
                is_hunyuan: true,
            }),
            defender_color: None,
            attacker_style: Some("baomai".to_string()),
            defender_style: Some("jiemai".to_string()),
            attacker_rejection_rate: Some(0.65),
            defender_resistance: Some(0.95),
            defender_drain_affinity: Some(0.2),
            attacker_qi: Some(20.0),
            distance_blocks: Some(3.0),
            effective_hit: Some(11.8),
            defender_lost: Some(0.59),
            defender_absorbed: Some(0.12),
            cause: "attack_intent:offline:Azure".to_string(),
            resolved_at_tick: 404,
        };

        let value = serde_json::to_value(payload).expect("style telemetry should serialize");
        assert_eq!(value["attacker_color"]["main"], json!("Heavy"));
        assert_eq!(value["attacker_style"], json!("baomai"));
        assert_eq!(value["attacker_rejection_rate"], json!(0.65));
        assert_eq!(value["defender_resistance"], json!(0.95));
    }

    #[test]
    fn skips_optional_physics_fields_when_absent() {
        let payload = StyleBalanceTelemetryEventV1 {
            v: 1,
            attacker_player_id: "offline:Azure".to_string(),
            defender_player_id: "offline:Crimson".to_string(),
            attacker_color: None,
            defender_color: None,
            attacker_style: None,
            defender_style: None,
            attacker_rejection_rate: None,
            defender_resistance: None,
            defender_drain_affinity: None,
            attacker_qi: None,
            distance_blocks: None,
            effective_hit: None,
            defender_lost: None,
            defender_absorbed: None,
            cause: "attack_intent:offline:Azure".to_string(),
            resolved_at_tick: 404,
        };

        let value = serde_json::to_value(payload).expect("style telemetry should serialize");
        let object = value.as_object().expect("payload should be an object");
        assert!(!object.contains_key("attacker_style"));
        assert!(!object.contains_key("attacker_rejection_rate"));
        assert!(!object.contains_key("defender_lost"));
        assert_eq!(
            Value::Object(object.clone())["cause"],
            json!("attack_intent:offline:Azure")
        );
    }
}
