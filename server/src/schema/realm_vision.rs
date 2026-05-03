use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum FogShapeV1 {
    Cylinder,
    Sphere,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RealmVisionParamsV1 {
    pub fog_start: f64,
    pub fog_end: f64,
    pub fog_color_rgb: u32,
    pub fog_shape: FogShapeV1,
    pub vignette_alpha: f64,
    pub tint_color_argb: u32,
    pub particle_density: f64,
    pub transition_ticks: u32,
    pub server_view_distance_chunks: u8,
    pub post_fx_sharpen: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum SenseKindV1 {
    LivingQi,
    AmbientLeyline,
    CultivatorRealm,
    HeavenlyGaze,
    CrisisPremonition,
    ZhenfaArray,
    ZhenfaWardAlert,
    SpiritEye,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SenseEntryV1 {
    pub kind: SenseKindV1,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub intensity: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SpiritualSenseTargetsV1 {
    pub entries: Vec<SenseEntryV1>,
    pub generation: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn realm_vision_sample_roundtrip() {
        let samples = [
            include_str!("../../../agent/packages/schema/samples/realm-vision-awaken.sample.json"),
            include_str!("../../../agent/packages/schema/samples/realm-vision-induce.sample.json"),
            include_str!(
                "../../../agent/packages/schema/samples/realm-vision-condense.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/realm-vision-solidify.sample.json"
            ),
            include_str!("../../../agent/packages/schema/samples/realm-vision-spirit.sample.json"),
            include_str!("../../../agent/packages/schema/samples/realm-vision-void.sample.json"),
        ];

        for json in samples {
            let params: RealmVisionParamsV1 =
                serde_json::from_str(json).expect("realm vision sample should deserialize");
            assert!(params.fog_end >= params.fog_start);
            assert!(params.server_view_distance_chunks <= 32);
            let roundtrip = serde_json::to_string(&params).expect("serialize realm vision sample");
            let decoded: RealmVisionParamsV1 =
                serde_json::from_str(&roundtrip).expect("realm vision sample should roundtrip");
            assert_eq!(params, decoded);
        }
    }

    #[test]
    fn spiritual_sense_sample_roundtrip() {
        let samples = [
            include_str!(
                "../../../agent/packages/schema/samples/spiritual-sense-induce.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/spiritual-sense-condense.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/spiritual-sense-solidify.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/spiritual-sense-spirit.sample.json"
            ),
            include_str!("../../../agent/packages/schema/samples/spiritual-sense-void.sample.json"),
        ];

        for json in samples {
            let targets: SpiritualSenseTargetsV1 =
                serde_json::from_str(json).expect("spiritual sense sample should deserialize");
            assert!(targets
                .entries
                .iter()
                .all(|entry| (0.0..=1.0).contains(&entry.intensity)));
            let roundtrip =
                serde_json::to_string(&targets).expect("serialize spiritual sense sample");
            let decoded: SpiritualSenseTargetsV1 =
                serde_json::from_str(&roundtrip).expect("spiritual sense sample should roundtrip");
            assert_eq!(targets, decoded);
        }
    }
}
