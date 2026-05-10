use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DuguV2SkillIdV1 {
    Eclipse,
    SelfCure,
    Penetrate,
    Shroud,
    Reverse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DuguTaintTierV1 {
    Immediate,
    Temporary,
    Permanent,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DuguV2SkillCastV1 {
    pub caster: String,
    pub target: Option<String>,
    pub skill: DuguV2SkillIdV1,
    pub tick: u64,
    pub taint_tier: Option<DuguTaintTierV1>,
    pub hp_loss: f32,
    pub qi_loss: f32,
    pub qi_max_loss: f32,
    pub permanent_decay_rate_per_min: f32,
    pub returned_zone_qi: f32,
    pub reveal_probability: f32,
    pub animation_id: String,
    pub particle_id: String,
    pub sound_recipe_id: String,
    pub icon_texture: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DuguSelfCureProgressV1 {
    pub caster: String,
    pub hours_used: f32,
    pub daily_hours_after: f32,
    pub gain_percent: f32,
    pub insidious_color_percent: f32,
    pub morphology_percent: f32,
    pub self_revealed: bool,
    pub tick: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DuguReverseTriggeredV1 {
    pub caster: String,
    pub affected_targets: u32,
    pub burst_damage: f32,
    pub returned_zone_qi: f32,
    pub juebi_delay_ticks: Option<u64>,
    pub center: [f64; 3],
    pub tick: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dugu_v2_skill_cast_serializes_snake_case_skill_and_tier() {
        let payload = DuguV2SkillCastV1 {
            caster: "player:a".to_string(),
            target: Some("player:b".to_string()),
            skill: DuguV2SkillIdV1::Eclipse,
            tick: 7,
            taint_tier: Some(DuguTaintTierV1::Permanent),
            hp_loss: 20.0,
            qi_loss: 40.0,
            qi_max_loss: 0.0,
            permanent_decay_rate_per_min: 0.001,
            returned_zone_qi: 39.6,
            reveal_probability: 0.03,
            animation_id: "bong:dugu_needle_throw".to_string(),
            particle_id: "bong:dugu_taint_pulse".to_string(),
            sound_recipe_id: "dugu_needle_hiss".to_string(),
            icon_texture: "bong:textures/gui/skill/dugu_eclipse.png".to_string(),
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains(r#""skill":"eclipse""#));
        assert!(json.contains(r#""taint_tier":"permanent""#));
    }
}
