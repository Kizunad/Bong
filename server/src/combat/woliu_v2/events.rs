use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, DVec3, Entity, Event};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WoliuSkillId {
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

impl WoliuSkillId {
    #[cfg(test)]
    pub const ALL: [Self; 10] = [
        Self::Hold,
        Self::Burst,
        Self::Mouth,
        Self::Pull,
        Self::Heart,
        Self::VacuumPalm,
        Self::VortexShield,
        Self::VacuumLock,
        Self::VortexResonance,
        Self::TurbulenceBurst,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hold => "woliu.hold",
            Self::Burst => "woliu.burst",
            Self::Mouth => "woliu.mouth",
            Self::Pull => "woliu.pull",
            Self::Heart => "woliu.heart",
            Self::VacuumPalm => "woliu.vacuum_palm",
            Self::VortexShield => "woliu.vortex_shield",
            Self::VacuumLock => "woliu.vacuum_lock",
            Self::VortexResonance => "woliu.vortex_resonance",
            Self::TurbulenceBurst => "woliu.turbulence_burst",
        }
    }

    pub fn practice_xp(self) -> u32 {
        match self {
            Self::Hold => 1,
            Self::Burst | Self::Mouth => 2,
            Self::Pull => 3,
            Self::Heart => 5,
            Self::VacuumPalm | Self::VortexShield => 2,
            Self::VacuumLock => 3,
            Self::VortexResonance => 4,
            Self::TurbulenceBurst => 5,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackfireLevel {
    Sensation,
    MicroTear,
    Torn,
    Severed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackfireCauseV2 {
    MeridianOverflow,
    TsyNegativeField,
    VoidHeartTribulation,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WoliuSkillVisual {
    pub animation_id: &'static str,
    pub particle_id: &'static str,
    pub sound_recipe_id: &'static str,
    pub hud_hint: &'static str,
    pub icon_texture: &'static str,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct VortexCastEvent {
    pub caster: Entity,
    pub skill: WoliuSkillId,
    pub tick: u64,
    pub center: DVec3,
    pub lethal_radius: f32,
    pub influence_radius: f32,
    pub turbulence_radius: f32,
    pub absorbed_qi: f32,
    pub swirl_qi: f32,
    pub backfire_level: Option<BackfireLevel>,
    pub visual: WoliuSkillVisual,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct VortexBackfireEventV2 {
    pub caster: Entity,
    pub skill: WoliuSkillId,
    pub level: BackfireLevel,
    pub cause: BackfireCauseV2,
    pub overflow_qi: f32,
    pub tick: u64,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct TurbulenceFieldSpawned {
    pub caster: Entity,
    pub skill: WoliuSkillId,
    pub center: DVec3,
    pub radius: f32,
    pub intensity: f32,
    pub swirl_qi: f32,
    pub tick: u64,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct TurbulenceFieldDecayed {
    pub caster: Entity,
    pub radius: f32,
    pub decayed_qi: f32,
    pub remaining_swirl_qi: f32,
    pub tick: u64,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct EntityDisplacedByVortexPull {
    pub caster: Entity,
    pub target: Entity,
    pub displacement_blocks: f32,
    pub tick: u64,
}
