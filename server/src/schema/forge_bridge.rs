//! 炼器（武器）server→agent Redis bridge payload。

use serde::{Deserialize, Serialize};

use super::forge::ForgeOutcomeBucketV1;
use crate::cultivation::components::ColorKind;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForgeMaterialStackV1 {
    pub material: String,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForgeStartPayloadV1 {
    pub v: u8,
    pub session_id: u64,
    pub blueprint_id: String,
    pub station_id: String,
    pub caster_id: String,
    pub materials: Vec<ForgeMaterialStackV1>,
    pub ts: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForgeOutcomePayloadV1 {
    pub v: u8,
    pub session_id: u64,
    pub blueprint_id: String,
    pub bucket: ForgeOutcomeBucketV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weapon_item: Option<String>,
    pub quality: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<ColorKind>,
    pub side_effects: Vec<String>,
    pub achieved_tier: u8,
    pub caster_id: String,
    pub ts: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forge_start_payload_sample_roundtrip() {
        let payload: ForgeStartPayloadV1 = serde_json::from_str(include_str!(
            "../../../agent/packages/schema/samples/forge-start-payload.sample.json"
        ))
        .expect("forge start sample should deserialize");

        assert_eq!(payload.v, 1);
        assert_eq!(payload.blueprint_id, "qing_feng_v0");
        assert_eq!(payload.materials[0].material, "fan_tie");
    }

    #[test]
    fn forge_outcome_payload_samples_roundtrip() {
        for sample in [
            include_str!(
                "../../../agent/packages/schema/samples/forge-outcome-payload-perfect.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/forge-outcome-payload-flawed.sample.json"
            ),
        ] {
            let payload: ForgeOutcomePayloadV1 =
                serde_json::from_str(sample).expect("forge outcome sample should deserialize");
            assert_eq!(payload.v, 1);
            assert!((0.0..=1.0).contains(&payload.quality));
        }
    }
}
