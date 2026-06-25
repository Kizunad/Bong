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

const STYLE_BALANCE_TELEMETRY_VERSION: u8 = 1;
const STYLE_BALANCE_UNIT_MIN: f64 = 0.0;
const STYLE_BALANCE_UNIT_MAX: f64 = 1.0;

impl StyleBalanceTelemetryEventV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.v != STYLE_BALANCE_TELEMETRY_VERSION {
            return Err(format!(
                "StyleBalanceTelemetryEventV1.v must be {STYLE_BALANCE_TELEMETRY_VERSION}, got {}",
                self.v
            ));
        }
        if self.attacker_player_id.is_empty() {
            return Err("StyleBalanceTelemetryEventV1.attacker_player_id must be non-empty".into());
        }
        if self.defender_player_id.is_empty() {
            return Err("StyleBalanceTelemetryEventV1.defender_player_id must be non-empty".into());
        }
        if self.cause.is_empty() {
            return Err("StyleBalanceTelemetryEventV1.cause must be non-empty".into());
        }

        validate_optional_non_empty(
            "attacker_style",
            self.attacker_style.as_deref(),
            "StyleBalanceTelemetryEventV1",
        )?;
        validate_optional_non_empty(
            "defender_style",
            self.defender_style.as_deref(),
            "StyleBalanceTelemetryEventV1",
        )?;
        validate_optional_unit(
            "attacker_rejection_rate",
            self.attacker_rejection_rate,
            "StyleBalanceTelemetryEventV1",
        )?;
        validate_optional_unit(
            "defender_resistance",
            self.defender_resistance,
            "StyleBalanceTelemetryEventV1",
        )?;
        validate_optional_unit(
            "defender_drain_affinity",
            self.defender_drain_affinity,
            "StyleBalanceTelemetryEventV1",
        )?;
        validate_optional_non_negative(
            "attacker_qi",
            self.attacker_qi,
            "StyleBalanceTelemetryEventV1",
        )?;
        validate_optional_non_negative(
            "distance_blocks",
            self.distance_blocks,
            "StyleBalanceTelemetryEventV1",
        )?;
        validate_optional_non_negative(
            "effective_hit",
            self.effective_hit,
            "StyleBalanceTelemetryEventV1",
        )?;
        validate_optional_non_negative(
            "defender_lost",
            self.defender_lost,
            "StyleBalanceTelemetryEventV1",
        )?;
        validate_optional_non_negative(
            "defender_absorbed",
            self.defender_absorbed,
            "StyleBalanceTelemetryEventV1",
        )?;

        Ok(())
    }
}

fn validate_optional_non_empty(
    field: &str,
    value: Option<&str>,
    context: &str,
) -> Result<(), String> {
    if let Some(value) = value {
        if !value.is_empty() {
            return Ok(());
        }

        return Err(format!("{context}.{field} must be non-empty when present"));
    }

    Ok(())
}

fn validate_optional_unit(field: &str, value: Option<f64>, context: &str) -> Result<(), String> {
    if let Some(value) = value {
        if !(value.is_finite()
            && (STYLE_BALANCE_UNIT_MIN..=STYLE_BALANCE_UNIT_MAX).contains(&value))
        {
            return Err(format!(
                "{context}.{field} must be finite and {STYLE_BALANCE_UNIT_MIN}..={STYLE_BALANCE_UNIT_MAX}, got {value}"
            ));
        }
    }

    Ok(())
}

fn validate_optional_non_negative(
    field: &str,
    value: Option<f64>,
    context: &str,
) -> Result<(), String> {
    if let Some(value) = value {
        if !(value.is_finite() && value >= 0.0) {
            return Err(format!(
                "{context}.{field} must be finite and >= 0, got {value}"
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::*;

    fn valid_style_balance_telemetry() -> StyleBalanceTelemetryEventV1 {
        StyleBalanceTelemetryEventV1 {
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
        }
    }

    fn assert_style_balance_invalid(
        label: &str,
        mutate: impl FnOnce(&mut StyleBalanceTelemetryEventV1),
    ) {
        let mut event = valid_style_balance_telemetry();
        mutate(&mut event);

        assert!(
            event.validate().is_err(),
            "{label} should violate TypeScript StyleBalanceTelemetryEventV1 contract"
        );
    }

    #[test]
    fn serializes_optional_physics_fields_without_breaking_color_snapshots() {
        let payload = valid_style_balance_telemetry();

        let value = serde_json::to_value(payload).expect("style telemetry should serialize");
        assert_eq!(value["attacker_color"]["main"], json!("Heavy"));
        assert_eq!(value["attacker_style"], json!("baomai"));
        assert_eq!(value["attacker_rejection_rate"], json!(0.65));
        assert_eq!(value["defender_resistance"], json!(0.95));
    }

    #[test]
    fn skips_optional_physics_fields_when_absent() {
        let mut payload = valid_style_balance_telemetry();
        payload.attacker_color = None;
        payload.defender_color = None;
        payload.attacker_style = None;
        payload.defender_style = None;
        payload.attacker_rejection_rate = None;
        payload.defender_resistance = None;
        payload.defender_drain_affinity = None;
        payload.attacker_qi = None;
        payload.distance_blocks = None;
        payload.effective_hit = None;
        payload.defender_lost = None;
        payload.defender_absorbed = None;

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

    #[test]
    fn validates_style_balance_telemetry_contract() {
        valid_style_balance_telemetry()
            .validate()
            .expect("sample style telemetry should pass validate()");
    }

    #[test]
    fn accepts_style_balance_telemetry_numeric_boundaries() {
        for value in [STYLE_BALANCE_UNIT_MIN, STYLE_BALANCE_UNIT_MAX] {
            let mut event = valid_style_balance_telemetry();
            event.attacker_rejection_rate = Some(value);
            event.defender_resistance = Some(value);
            event.defender_drain_affinity = Some(value);

            event
                .validate()
                .expect("TypeScript StyleBalanceTelemetryEventV1 unit boundaries should pass");
        }

        let mut non_negative = valid_style_balance_telemetry();
        non_negative.attacker_qi = Some(0.0);
        non_negative.distance_blocks = Some(0.0);
        non_negative.effective_hit = Some(0.0);
        non_negative.defender_lost = Some(0.0);
        non_negative.defender_absorbed = Some(0.0);
        non_negative
            .validate()
            .expect("TypeScript StyleBalanceTelemetryEventV1 non-negative boundaries should pass");
    }

    #[test]
    fn rejects_style_balance_telemetry_contract_violations() {
        assert_style_balance_invalid("version", |event| event.v = 2);
        assert_style_balance_invalid("attacker_player_id", |event| {
            event.attacker_player_id.clear()
        });
        assert_style_balance_invalid("defender_player_id", |event| {
            event.defender_player_id.clear()
        });
        assert_style_balance_invalid("cause", |event| event.cause.clear());
        assert_style_balance_invalid("attacker_style", |event| {
            event.attacker_style = Some(String::new());
        });
        assert_style_balance_invalid("defender_style", |event| {
            event.defender_style = Some(String::new());
        });
        assert_style_balance_invalid("attacker_rejection_rate below", |event| {
            event.attacker_rejection_rate = Some(-0.1);
        });
        assert_style_balance_invalid("attacker_rejection_rate above", |event| {
            event.attacker_rejection_rate = Some(1.1);
        });
        assert_style_balance_invalid("defender_resistance below", |event| {
            event.defender_resistance = Some(-0.1);
        });
        assert_style_balance_invalid("defender_resistance above", |event| {
            event.defender_resistance = Some(1.1);
        });
        assert_style_balance_invalid("defender_drain_affinity below", |event| {
            event.defender_drain_affinity = Some(-0.1);
        });
        assert_style_balance_invalid("defender_drain_affinity above", |event| {
            event.defender_drain_affinity = Some(1.1);
        });
        assert_style_balance_invalid("attacker_qi below", |event| event.attacker_qi = Some(-0.1));
        assert_style_balance_invalid("distance_blocks below", |event| {
            event.distance_blocks = Some(-0.1);
        });
        assert_style_balance_invalid("effective_hit below", |event| {
            event.effective_hit = Some(-0.1);
        });
        assert_style_balance_invalid("defender_lost below", |event| {
            event.defender_lost = Some(-0.1);
        });
        assert_style_balance_invalid("defender_absorbed below", |event| {
            event.defender_absorbed = Some(-0.1);
        });
        assert_style_balance_invalid("attacker_qi nan", |event| {
            event.attacker_qi = Some(f64::NAN)
        });
        assert_style_balance_invalid("distance_blocks inf", |event| {
            event.distance_blocks = Some(f64::INFINITY);
        });
    }
}
