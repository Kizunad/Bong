use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WoliuSkillIdV1 {
    Hold,
    Burst,
    Mouth,
    Pull,
    Heart,
    VacuumPalm,
    VortexShield,
    VacuumLock,
    VortexResonance,
    TurbulenceBurst,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WoliuBackfireLevelV1 {
    Sensation,
    MicroTear,
    Torn,
    Severed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WoliuSkillCastV1 {
    pub caster: String,
    pub skill: WoliuSkillIdV1,
    pub tick: u64,
    pub lethal_radius: f32,
    pub influence_radius: f32,
    pub turbulence_radius: f32,
    pub absorbed_qi: f32,
    pub swirl_qi: f32,
    pub animation_id: String,
    pub particle_id: String,
    pub sound_recipe_id: String,
    pub icon_texture: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WoliuBackfireV1 {
    pub caster: String,
    pub skill: WoliuSkillIdV1,
    pub level: WoliuBackfireLevelV1,
    pub cause: String,
    pub overflow_qi: f32,
    pub tick: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TurbulenceFieldV1 {
    pub caster: String,
    pub skill: WoliuSkillIdV1,
    pub center: [f64; 3],
    pub radius: f32,
    pub intensity: f32,
    pub swirl_qi: f32,
    pub tick: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WoliuPullDisplaceV1 {
    pub caster: String,
    pub target: String,
    pub displacement_blocks: f32,
    pub tick: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn woliu_skill_cast_serializes_snake_case_skill() {
        let payload = WoliuSkillCastV1 {
            caster: "player:a".to_string(),
            skill: WoliuSkillIdV1::VacuumPalm,
            tick: 7,
            lethal_radius: 3.0,
            influence_radius: 30.0,
            turbulence_radius: 10.0,
            absorbed_qi: 0.1,
            swirl_qi: 12.0,
            animation_id: "bong:vortex_palm_open".to_string(),
            particle_id: "bong:vortex_spiral".to_string(),
            sound_recipe_id: "vortex_qi_siphon".to_string(),
            icon_texture: "bong:textures/gui/skill/woliu_mouth.png".to_string(),
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains(r#""skill":"vacuum_palm""#));
        assert!(json.contains("vortex_qi_siphon"));
    }
}
