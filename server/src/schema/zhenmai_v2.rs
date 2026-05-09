use serde::{Deserialize, Serialize};

pub const ZHENMAI_SKILL_EVENT_TYPE: &str = "zhenmai_skill_event";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZhenmaiSkillIdV1 {
    Parry,
    Neutralize,
    Multipoint,
    HardenMeridian,
    SeverChain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZhenmaiAttackKindV1 {
    RealYuan,
    PhysicalCarrier,
    TaintedYuan,
    Array,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZhenmaiSkillEventV1 {
    pub v: u8,
    #[serde(rename = "type")]
    pub event_type: String,
    pub skill_id: ZhenmaiSkillIdV1,
    pub caster_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meridian_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meridian_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attack_kind: Option<ZhenmaiAttackKindV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reflected_qi: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub k_drain: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub damage_multiplier: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub self_damage_multiplier: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grants_amplification: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at_tick: Option<u64>,
    pub tick: u64,
}

impl ZhenmaiSkillEventV1 {
    pub fn new(skill_id: ZhenmaiSkillIdV1, caster_id: String, tick: u64) -> Self {
        Self {
            v: 1,
            event_type: ZHENMAI_SKILL_EVENT_TYPE.to_string(),
            skill_id,
            caster_id,
            target_id: None,
            meridian_id: None,
            meridian_ids: None,
            attack_kind: None,
            reflected_qi: None,
            k_drain: None,
            damage_multiplier: None,
            self_damage_multiplier: None,
            grants_amplification: None,
            expires_at_tick: None,
            tick,
        }
    }
}
