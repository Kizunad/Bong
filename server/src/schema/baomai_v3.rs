use serde::{Deserialize, Serialize};

pub const BAOMAI_SKILL_EVENT_TYPE: &str = "baomai_skill_event";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BaomaiSkillIdV1 {
    BengQuan,
    FullPowerCharge,
    FullPowerRelease,
    MountainShake,
    BloodBurn,
    Disperse,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaomaiSkillEventV1 {
    pub v: u8,
    #[serde(rename = "type")]
    pub event_type: String,
    pub skill_id: BaomaiSkillIdV1,
    pub caster_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_id: Option<String>,
    pub tick: u64,
    pub qi_invested: f64,
    pub damage: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub radius_blocks: Option<f32>,
    pub blood_multiplier: f32,
    pub flow_rate_multiplier: f64,
    pub meridian_ids: Vec<String>,
}

impl BaomaiSkillEventV1 {
    pub fn new(skill_id: BaomaiSkillIdV1, caster_id: String, tick: u64) -> Self {
        Self {
            v: 1,
            event_type: BAOMAI_SKILL_EVENT_TYPE.to_string(),
            skill_id,
            caster_id,
            target_id: None,
            tick,
            qi_invested: 0.0,
            damage: 0.0,
            radius_blocks: None,
            blood_multiplier: 1.0,
            flow_rate_multiplier: 1.0,
            meridian_ids: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baomai_skill_event_serializes_snake_case_skill_and_type() {
        let mut event =
            BaomaiSkillEventV1::new(BaomaiSkillIdV1::Disperse, "char:7".to_string(), 42);
        event.flow_rate_multiplier = 10.0;
        event.meridian_ids = vec!["Ren".to_string(), "Du".to_string()];

        let json = serde_json::to_value(&event).expect("serialize baomai event");

        assert_eq!(json["type"], BAOMAI_SKILL_EVENT_TYPE);
        assert_eq!(json["skill_id"], "disperse");
        assert_eq!(json["flow_rate_multiplier"], 10.0);
        assert_eq!(json["meridian_ids"][0], "Ren");
    }
}
