use serde::{Deserialize, Serialize};

use super::death_insight::DeathInsightPositionV1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SpiritEyePositionV1 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SpiritEyeMigrateReasonV1 {
    UsagePressure,
    PeriodicDrift,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SpiritEyeMigrateV1 {
    pub v: u8,
    pub eye_id: String,
    pub from: SpiritEyePositionV1,
    pub to: SpiritEyePositionV1,
    pub reason: SpiritEyeMigrateReasonV1,
    pub usage_pressure: f64,
    pub tick: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SpiritEyeDiscoveredV1 {
    pub v: u8,
    pub eye_id: String,
    pub character_id: String,
    pub pos: SpiritEyePositionV1,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub zone: Option<String>,
    pub qi_concentration: f64,
    pub discovered_at_tick: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SpiritEyeUsedForBreakthroughV1 {
    pub v: u8,
    pub eye_id: String,
    pub character_id: String,
    pub realm_from: String,
    pub realm_to: String,
    pub usage_pressure: f64,
    pub tick: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SpiritEyeCoordinateNoteV1 {
    pub v: u8,
    pub eye_id: String,
    pub owner_character_id: String,
    pub pos: SpiritEyePositionV1,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub zone: Option<String>,
    pub qi_concentration: f64,
    pub discovered_at_tick: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DeathInsightSpiritEyeV1 {
    pub eye_id: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub zone: Option<String>,
    pub pos: DeathInsightPositionV1,
    pub qi_concentration: f64,
}

impl From<[f64; 3]> for SpiritEyePositionV1 {
    fn from(pos: [f64; 3]) -> Self {
        Self {
            x: pos[0],
            y: pos[1],
            z: pos[2],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spirit_eye_migrate_contract_serializes_snake_case_reason() {
        let payload = SpiritEyeMigrateV1 {
            v: 1,
            eye_id: "spirit_eye:spawn:0".to_string(),
            from: [0.0, 66.0, 0.0].into(),
            to: [640.0, 66.0, 0.0].into(),
            reason: SpiritEyeMigrateReasonV1::UsagePressure,
            usage_pressure: 1.0,
            tick: 72,
        };

        let json = serde_json::to_value(payload).expect("serialize");

        assert_eq!(json["reason"], "usage_pressure");
        assert_eq!(json["eye_id"], "spirit_eye:spawn:0");
    }
}
