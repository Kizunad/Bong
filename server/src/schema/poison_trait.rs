use serde::{de::Error as _, Deserialize, Deserializer, Serialize};

const POISON_TRAIT_SCHEMA_VERSION: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PoisonSideEffectTagV1 {
    #[serde(rename = "qi_focus_drift_2h")]
    QiFocusDrift2h,
    #[serde(rename = "rage_burst_30min")]
    RageBurst30m,
    #[serde(rename = "hallucin_tint_6h")]
    HallucinTint6h,
    #[serde(rename = "digest_lock_6h")]
    DigestLock6h,
    ToxicityTierUnlock,
}

impl From<crate::cultivation::poison_trait::PoisonSideEffectTag> for PoisonSideEffectTagV1 {
    fn from(tag: crate::cultivation::poison_trait::PoisonSideEffectTag) -> Self {
        match tag {
            crate::cultivation::poison_trait::PoisonSideEffectTag::QiFocusDrift2h => {
                Self::QiFocusDrift2h
            }
            crate::cultivation::poison_trait::PoisonSideEffectTag::RageBurst30m => {
                Self::RageBurst30m
            }
            crate::cultivation::poison_trait::PoisonSideEffectTag::HallucinTint6h => {
                Self::HallucinTint6h
            }
            crate::cultivation::poison_trait::PoisonSideEffectTag::DigestLock6h => {
                Self::DigestLock6h
            }
            crate::cultivation::poison_trait::PoisonSideEffectTag::ToxicityTierUnlock => {
                Self::ToxicityTierUnlock
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PoisonOverdoseSeverityV1 {
    Mild,
    Moderate,
    Severe,
}

impl From<crate::cultivation::poison_trait::PoisonOverdoseSeverity> for PoisonOverdoseSeverityV1 {
    fn from(severity: crate::cultivation::poison_trait::PoisonOverdoseSeverity) -> Self {
        match severity {
            crate::cultivation::poison_trait::PoisonOverdoseSeverity::Mild => Self::Mild,
            crate::cultivation::poison_trait::PoisonOverdoseSeverity::Moderate => Self::Moderate,
            crate::cultivation::poison_trait::PoisonOverdoseSeverity::Severe => Self::Severe,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PoisonDoseEventV1 {
    #[serde(deserialize_with = "deserialize_v1_version")]
    pub v: u8,
    pub player_entity_id: u64,
    pub dose_amount: f32,
    pub side_effect_tag: PoisonSideEffectTagV1,
    pub poison_level_after: f32,
    pub digestion_after: f32,
    pub at_tick: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PoisonOverdoseEventV1 {
    #[serde(deserialize_with = "deserialize_v1_version")]
    pub v: u8,
    pub player_entity_id: u64,
    pub severity: PoisonOverdoseSeverityV1,
    pub overflow: f32,
    pub lifespan_penalty_years: f32,
    pub micro_tear_probability: f32,
    pub at_tick: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PoisonTraitStateV1 {
    #[serde(deserialize_with = "deserialize_v1_version")]
    pub v: u8,
    pub player_entity_id: u64,
    pub poison_toxicity: f32,
    pub digestion_current: f32,
    pub digestion_capacity: f32,
    pub toxicity_tier_unlocked: bool,
}

fn deserialize_v1_version<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: Deserializer<'de>,
{
    let version = u8::deserialize(deserializer)?;
    if version == POISON_TRAIT_SCHEMA_VERSION {
        Ok(version)
    } else {
        Err(D::Error::custom(format!(
            "poison_trait v must be {POISON_TRAIT_SCHEMA_VERSION}, got {version}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dose_event_serde_uses_snake_case_tag() {
        let payload = PoisonDoseEventV1 {
            v: 1,
            player_entity_id: 7,
            dose_amount: 5.0,
            side_effect_tag: PoisonSideEffectTagV1::QiFocusDrift2h,
            poison_level_after: 5.0,
            digestion_after: 20.0,
            at_tick: 10,
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("\"qi_focus_drift_2h\""));
        let back: PoisonDoseEventV1 = serde_json::from_str(&json).unwrap();
        assert_eq!(back, payload);
    }

    #[test]
    fn overdose_event_rejects_unknown_fields() {
        let json = r#"{"v":1,"player_entity_id":7,"severity":"mild","overflow":1.0,"lifespan_penalty_years":0.1,"micro_tear_probability":0.0,"at_tick":10,"extra":true}"#;
        assert!(serde_json::from_str::<PoisonOverdoseEventV1>(json).is_err());
    }

    #[test]
    fn dose_event_rejects_wrong_version() {
        let json = r#"{"v":2,"player_entity_id":7,"dose_amount":5.0,"side_effect_tag":"qi_focus_drift_2h","poison_level_after":5.0,"digestion_after":20.0,"at_tick":10}"#;
        assert!(serde_json::from_str::<PoisonDoseEventV1>(json).is_err());
    }

    #[test]
    fn state_rejects_wrong_version() {
        let json = r#"{"v":0,"player_entity_id":7,"poison_toxicity":5.0,"digestion_current":20.0,"digestion_capacity":100.0,"toxicity_tier_unlocked":false}"#;
        assert!(serde_json::from_str::<PoisonTraitStateV1>(json).is_err());
    }
}
