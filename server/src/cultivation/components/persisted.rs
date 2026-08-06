//! 持久化边界：运行时 [`Cultivation`] 不直接反序列化，所有快照必须经 qi invariant 校验。

use serde::{Deserialize, Serialize};

use crate::body_plan::RaceId;

use super::{Cultivation, CultivationQiInit, QiFlowError, Realm};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersistedCultivationV1 {
    pub realm: Realm,
    pub qi_current: f64,
    pub qi_max: f64,
    pub qi_max_frozen: Option<f64>,
    pub last_qi_zero_at: Option<u64>,
    pub pending_material_bonus: f64,
    pub composure: f64,
    pub composure_recover_rate: f64,
    #[serde(default = "default_persisted_race")]
    pub race: RaceId,
}

impl From<&Cultivation> for PersistedCultivationV1 {
    fn from(cultivation: &Cultivation) -> Self {
        Self {
            realm: cultivation.realm,
            qi_current: cultivation.qi_current,
            qi_max: cultivation.qi_max,
            qi_max_frozen: cultivation.qi_max_frozen,
            last_qi_zero_at: cultivation.last_qi_zero_at,
            pending_material_bonus: cultivation.pending_material_bonus,
            composure: cultivation.composure,
            composure_recover_rate: cultivation.composure_recover_rate,
            race: cultivation.race.clone(),
        }
    }
}

impl TryFrom<PersistedCultivationV1> for Cultivation {
    type Error = QiFlowError;

    fn try_from(persisted: PersistedCultivationV1) -> Result<Self, Self::Error> {
        let qi = CultivationQiInit {
            current: persisted.qi_current,
            max: persisted.qi_max,
            frozen: persisted.qi_max_frozen,
        };
        let mut cultivation = Cultivation {
            realm: persisted.realm,
            qi_current: 0.0,
            qi_max: 0.0,
            qi_max_frozen: None,
            last_qi_zero_at: persisted.last_qi_zero_at,
            pending_material_bonus: persisted.pending_material_bonus,
            composure: persisted.composure,
            composure_recover_rate: persisted.composure_recover_rate,
            race: persisted.race,
        };
        cultivation.set_for_init(qi)?;
        Ok(cultivation)
    }
}

pub fn decode_persisted_cultivation(value: serde_json::Value) -> Result<Cultivation, String> {
    let persisted = serde_json::from_value::<PersistedCultivationV1>(value)
        .map_err(|error| format!("invalid cultivation wire shape: {error}"))?;
    persisted
        .try_into()
        .map_err(|error| format!("invalid cultivation qi snapshot: {error}"))
}

pub fn encode_persisted_cultivation(cultivation: &Cultivation) -> PersistedCultivationV1 {
    PersistedCultivationV1::from(cultivation)
}

fn default_persisted_race() -> RaceId {
    RaceId::new(crate::body_plan::HUMAN_RACE_ID)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_persisted() -> PersistedCultivationV1 {
        PersistedCultivationV1 {
            realm: Realm::Condense,
            qi_current: 4.0,
            qi_max: 12.0,
            qi_max_frozen: Some(2.0),
            last_qi_zero_at: Some(42),
            pending_material_bonus: 1.5,
            composure: 0.75,
            composure_recover_rate: 0.002,
            race: RaceId::new("human"),
        }
    }

    #[test]
    fn persisted_roundtrip_preserves_every_field() {
        let cultivation = Cultivation::try_from(valid_persisted()).expect("valid snapshot");
        let encoded = PersistedCultivationV1::from(&cultivation);
        assert_eq!(encoded, valid_persisted());
    }

    #[test]
    fn legacy_snapshot_without_race_defaults_to_human() {
        let mut value = serde_json::to_value(valid_persisted()).expect("serialize fixture");
        value
            .as_object_mut()
            .expect("persisted cultivation is an object")
            .remove("race");

        let cultivation = decode_persisted_cultivation(value).expect("legacy snapshot is valid");
        assert_eq!(
            cultivation.race,
            RaceId::new(crate::body_plan::HUMAN_RACE_ID)
        );
    }

    #[test]
    fn every_invalid_qi_shape_is_rejected() {
        let invalid = [
            (f64::NAN, 12.0, None),
            (-1.0, 12.0, None),
            (13.0, 12.0, None),
            (1.0, f64::INFINITY, None),
            (1.0, -1.0, None),
            (1.0, 12.0, Some(f64::NAN)),
            (1.0, 12.0, Some(-1.0)),
            (1.0, 12.0, Some(6.01)),
        ];

        for (current, max, frozen) in invalid {
            let mut persisted = valid_persisted();
            persisted.qi_current = current;
            persisted.qi_max = max;
            persisted.qi_max_frozen = frozen;
            assert!(
                Cultivation::try_from(persisted).is_err(),
                "invalid qi snapshot current={current:?} max={max:?} frozen={frozen:?} must fail closed"
            );
        }
    }
}
