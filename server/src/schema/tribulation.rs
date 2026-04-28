use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TribulationKindV1 {
    DuXu,
    ZoneCollapse,
    Targeted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum TribulationPhaseV1 {
    Omen,
    Lock,
    Wave { wave: u32 },
    HeartDemon,
    Settle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DuXuOutcomeV1 {
    Ascended,
    HalfStep,
    Failed,
    Killed,
    Fled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DuXuResultV1 {
    pub char_id: String,
    pub outcome: DuXuOutcomeV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub killer: Option<String>,
    pub waves_survived: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TribulationEventV1 {
    pub v: u8,
    pub kind: TribulationKindV1,
    pub phase: TribulationPhaseV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub char_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub epicenter: Option<[f64; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wave_current: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wave_total: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<DuXuResultV1>,
}

impl TribulationEventV1 {
    pub fn du_xu(
        phase: TribulationPhaseV1,
        char_id: Option<String>,
        actor_name: Option<String>,
        epicenter: Option<[f64; 3]>,
        wave_current: Option<u32>,
        wave_total: Option<u32>,
        result: Option<DuXuResultV1>,
    ) -> Self {
        Self {
            v: 1,
            kind: TribulationKindV1::DuXu,
            phase,
            char_id,
            actor_name,
            zone: None,
            epicenter,
            wave_current,
            wave_total,
            result,
        }
    }
}
