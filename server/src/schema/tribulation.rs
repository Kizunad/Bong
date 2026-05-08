use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TribulationKindV1 {
    DuXu,
    ZoneCollapse,
    Targeted,
    AscensionQuotaOpen,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub occupied_slots: Option<u32>,
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
            occupied_slots: None,
        }
    }

    pub fn zone_collapse(
        phase: TribulationPhaseV1,
        zone: Option<String>,
        epicenter: Option<[f64; 3]>,
    ) -> Self {
        Self {
            v: 1,
            kind: TribulationKindV1::ZoneCollapse,
            phase,
            char_id: None,
            actor_name: None,
            zone,
            epicenter,
            wave_current: None,
            wave_total: None,
            result: None,
            occupied_slots: None,
        }
    }

    pub fn targeted(
        phase: TribulationPhaseV1,
        zone: Option<String>,
        epicenter: Option<[f64; 3]>,
    ) -> Self {
        Self {
            v: 1,
            kind: TribulationKindV1::Targeted,
            phase,
            char_id: None,
            actor_name: None,
            zone,
            epicenter,
            wave_current: None,
            wave_total: None,
            result: None,
            occupied_slots: None,
        }
    }

    pub fn ascension_quota_open(occupied_slots: Option<u32>) -> Self {
        Self {
            v: 1,
            kind: TribulationKindV1::AscensionQuotaOpen,
            phase: TribulationPhaseV1::Settle,
            char_id: None,
            actor_name: None,
            zone: None,
            epicenter: None,
            wave_current: None,
            wave_total: None,
            result: None,
            occupied_slots,
        }
    }
}
