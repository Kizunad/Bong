//! 医道 v1 跨栈契约。

use serde::{Deserialize, Serialize};

use crate::cultivation::components::MeridianId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum YidaoSkillIdV1 {
    MeridianRepair,
    ContamPurge,
    EmergencyResuscitate,
    LifeExtension,
    MassMeridianRepair,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum YidaoEventKindV1 {
    MeridianHeal,
    ContamPurge,
    EmergencyResuscitate,
    LifeExtension,
    MassHeal,
    KarmaAccumulation,
    MedicalContract,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MedicalContractStateV1 {
    Stranger,
    Patient,
    LongTermPatient,
    Bonded,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct YidaoEventV1 {
    pub v: u8,
    pub kind: YidaoEventKindV1,
    pub tick: u64,
    pub medic_id: String,
    pub patient_ids: Vec<String>,
    pub skill: YidaoSkillIdV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meridian_id: Option<MeridianId>,
    pub success_count: u32,
    pub failure_count: u32,
    pub qi_transferred: f64,
    pub contam_reduced: f64,
    pub hp_restored: f32,
    pub karma_delta: f64,
    pub medic_qi_max_delta: f64,
    pub patient_qi_max_delta: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contract_state: Option<MedicalContractStateV1>,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HealerNpcAiStateV1 {
    pub healer_id: String,
    pub active_action: String,
    pub queue_len: u32,
    pub reputation: i32,
    pub retreating: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct YidaoHudStateV1 {
    pub healer_id: String,
    pub reputation: i32,
    pub peace_mastery: f32,
    pub karma: f64,
    pub active_skill: Option<YidaoSkillIdV1>,
    pub patient_ids: Vec<String>,
    pub patient_hp_percent: Option<f32>,
    pub patient_contam_total: Option<f64>,
    pub severed_meridian_count: u32,
    pub contract_count: u32,
    pub mass_preview_count: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yidao_event_roundtrips_with_no_unknown_fields() {
        let event = YidaoEventV1 {
            v: 1,
            kind: YidaoEventKindV1::MassHeal,
            tick: 42,
            medic_id: "offline:Healer".to_string(),
            patient_ids: vec![
                "offline:PatientA".to_string(),
                "offline:PatientB".to_string(),
            ],
            skill: YidaoSkillIdV1::MassMeridianRepair,
            meridian_id: Some(MeridianId::Lung),
            success_count: 2,
            failure_count: 0,
            qi_transferred: 120.0,
            contam_reduced: 0.0,
            hp_restored: 0.0,
            karma_delta: 0.2,
            medic_qi_max_delta: -0.04,
            patient_qi_max_delta: 0.0,
            contract_state: None,
            detail: "mass repair".to_string(),
        };
        let text = serde_json::to_string(&event).expect("serialize");
        assert!(text.contains("\"mass_meridian_repair\""));
        let back: YidaoEventV1 = serde_json::from_str(&text).expect("deserialize");
        assert_eq!(back.kind, YidaoEventKindV1::MassHeal);
        assert_eq!(back.patient_ids.len(), 2);
    }

    #[test]
    fn yidao_hud_state_serializes_active_skill() {
        let state = YidaoHudStateV1 {
            healer_id: "npc:doctor".to_string(),
            reputation: 12,
            peace_mastery: 48.0,
            karma: 3.5,
            active_skill: Some(YidaoSkillIdV1::LifeExtension),
            patient_ids: vec!["offline:NearDeath".to_string()],
            patient_hp_percent: Some(0.5),
            patient_contam_total: Some(1.25),
            severed_meridian_count: 1,
            contract_count: 2,
            mass_preview_count: 0,
        };
        let value = serde_json::to_value(state).expect("serialize");
        assert_eq!(value["active_skill"], "life_extension");
    }
}
