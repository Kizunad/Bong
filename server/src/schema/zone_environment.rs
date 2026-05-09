//! zone-scoped environment effect schema (`bong:zone_environment_update` / `bong:zone_environment`).

use serde::{Deserialize, Serialize};

use super::common::MAX_PAYLOAD_BYTES;
use crate::world::dimension::DimensionKind;
use crate::world::environment::EnvironmentEffect;

pub const ZONE_ENVIRONMENT_STATE_VERSION: u8 = 1;

pub type EnvironmentEffectV1 = EnvironmentEffect;

#[derive(Debug)]
pub enum ZoneEnvironmentBuildError {
    Json(serde_json::Error),
    Oversize { size: usize, max: usize },
    InvalidState(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ZoneEnvironmentStateV1 {
    pub v: u8,
    pub dimension: String,
    pub zone_id: String,
    pub effects: Vec<EnvironmentEffectV1>,
    pub generation: u64,
}

impl ZoneEnvironmentStateV1 {
    pub fn new(
        zone_id: impl Into<String>,
        effects: Vec<EnvironmentEffectV1>,
        generation: u64,
    ) -> Self {
        Self::new_with_dimension(
            DimensionKind::Overworld.ident_str(),
            zone_id,
            effects,
            generation,
        )
    }

    pub fn new_with_dimension(
        dimension: impl Into<String>,
        zone_id: impl Into<String>,
        effects: Vec<EnvironmentEffectV1>,
        generation: u64,
    ) -> Self {
        Self {
            v: ZONE_ENVIRONMENT_STATE_VERSION,
            dimension: dimension.into(),
            zone_id: zone_id.into(),
            effects,
            generation,
        }
    }

    pub fn validate(&self) -> Result<(), ZoneEnvironmentBuildError> {
        if self.v != ZONE_ENVIRONMENT_STATE_VERSION {
            return Err(ZoneEnvironmentBuildError::InvalidState(format!(
                "v must be {ZONE_ENVIRONMENT_STATE_VERSION}, got {}",
                self.v
            )));
        }
        if self.zone_id.trim().is_empty() {
            return Err(ZoneEnvironmentBuildError::InvalidState(
                "zone_id must not be blank".to_string(),
            ));
        }
        if self.dimension.trim().is_empty() {
            return Err(ZoneEnvironmentBuildError::InvalidState(
                "dimension must not be blank".to_string(),
            ));
        }
        Ok(())
    }

    pub fn to_json_bytes_checked(&self) -> Result<Vec<u8>, ZoneEnvironmentBuildError> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(ZoneEnvironmentBuildError::Json)?;
        if bytes.len() > MAX_PAYLOAD_BYTES {
            return Err(ZoneEnvironmentBuildError::Oversize {
                size: bytes.len(),
                max: MAX_PAYLOAD_BYTES,
            });
        }
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_state() -> ZoneEnvironmentStateV1 {
        ZoneEnvironmentStateV1::new(
            "spawn",
            vec![
                EnvironmentEffect::TornadoColumn {
                    center: [1.0, 70.0, 2.0],
                    radius: 9.0,
                    height: 48.0,
                    particle_density: 0.6,
                },
                EnvironmentEffect::FogVeil {
                    aabb_min: [0.0, 60.0, 0.0],
                    aabb_max: [32.0, 95.0, 32.0],
                    tint_rgb: [120, 132, 148],
                    density: 0.32,
                },
            ],
            7,
        )
    }

    #[test]
    fn state_serializes_tagged_effect_union() {
        let json = serde_json::to_value(sample_state()).expect("serialize");
        assert_eq!(json["v"], 1);
        assert_eq!(json["dimension"], "minecraft:overworld");
        assert_eq!(json["zone_id"], "spawn");
        assert_eq!(json["generation"], 7);
        assert_eq!(json["effects"][0]["kind"], "tornado_column");
        assert_eq!(json["effects"][1]["kind"], "fog_veil");
    }

    #[test]
    fn state_round_trips_via_json_bytes() {
        let state = sample_state();
        let bytes = state.to_json_bytes_checked().expect("checked bytes");
        let parsed: ZoneEnvironmentStateV1 =
            serde_json::from_slice(bytes.as_slice()).expect("deserialize");
        assert_eq!(parsed, state);
    }

    #[test]
    fn state_rejects_blank_zone_id() {
        let state = ZoneEnvironmentStateV1::new("", Vec::new(), 0);
        assert!(matches!(
            state.validate(),
            Err(ZoneEnvironmentBuildError::InvalidState(_))
        ));
    }

    #[test]
    fn state_rejects_blank_dimension() {
        let state = ZoneEnvironmentStateV1::new_with_dimension("", "spawn", Vec::new(), 0);
        assert!(matches!(
            state.validate(),
            Err(ZoneEnvironmentBuildError::InvalidState(_))
        ));
    }

    #[test]
    fn state_loads_sample_from_agent_schema_package() {
        let raw = include_str!(
            "../../../agent/packages/schema/samples/zone-environment-state.sample.json"
        );
        let parsed: ZoneEnvironmentStateV1 =
            serde_json::from_str(raw).expect("sample should pass Rust serde");
        assert_eq!(parsed.v, 1);
        assert_eq!(parsed.zone_id, "spawn");
        assert_eq!(parsed.effects.len(), 2);
        assert_eq!(parsed.effects[0].kind(), "tornado_column");
    }
}
