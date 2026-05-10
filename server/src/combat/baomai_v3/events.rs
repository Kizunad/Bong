use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Entity, Event};

use crate::cultivation::components::MeridianId;

pub const BAOMAI_BENG_QUAN_SKILL_ID: &str = "baomai.beng_quan";
pub const BAOMAI_FULL_POWER_CHARGE_SKILL_ID: &str = "baomai.full_power_charge";
pub const BAOMAI_FULL_POWER_RELEASE_SKILL_ID: &str = "baomai.full_power_release";
pub const BAOMAI_MOUNTAIN_SHAKE_SKILL_ID: &str = "baomai.mountain_shake";
pub const BAOMAI_BLOOD_BURN_SKILL_ID: &str = "baomai.blood_burn";
pub const BAOMAI_DISPERSE_SKILL_ID: &str = "baomai.disperse";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BaomaiSkillId {
    BengQuan,
    FullPowerCharge,
    FullPowerRelease,
    MountainShake,
    BloodBurn,
    Disperse,
}

impl BaomaiSkillId {
    #[cfg(test)]
    pub const ALL: [Self; 6] = [
        Self::BengQuan,
        Self::FullPowerCharge,
        Self::FullPowerRelease,
        Self::MountainShake,
        Self::BloodBurn,
        Self::Disperse,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BengQuan => BAOMAI_BENG_QUAN_SKILL_ID,
            Self::FullPowerCharge => BAOMAI_FULL_POWER_CHARGE_SKILL_ID,
            Self::FullPowerRelease => BAOMAI_FULL_POWER_RELEASE_SKILL_ID,
            Self::MountainShake => BAOMAI_MOUNTAIN_SHAKE_SKILL_ID,
            Self::BloodBurn => BAOMAI_BLOOD_BURN_SKILL_ID,
            Self::Disperse => BAOMAI_DISPERSE_SKILL_ID,
        }
    }

    pub const fn wire_kind(self) -> &'static str {
        match self {
            Self::BengQuan => "beng_quan",
            Self::FullPowerCharge => "full_power_charge",
            Self::FullPowerRelease => "full_power_release",
            Self::MountainShake => "mountain_shake",
            Self::BloodBurn => "blood_burn",
            Self::Disperse => "disperse",
        }
    }

    pub const fn practice_xp(self) -> u32 {
        match self {
            Self::BengQuan | Self::FullPowerCharge => 1,
            Self::FullPowerRelease | Self::MountainShake | Self::BloodBurn => 2,
            Self::Disperse => 5,
        }
    }
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct BaomaiSkillEvent {
    pub skill: BaomaiSkillId,
    pub caster: Entity,
    pub target: Option<Entity>,
    pub tick: u64,
    pub qi_invested: f64,
    pub damage: f32,
    pub radius_blocks: Option<f32>,
    pub blood_multiplier: f32,
    pub flow_rate_multiplier: f64,
    pub meridian_dependencies: Vec<MeridianId>,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct MountainShakeEvent {
    pub caster: Entity,
    pub affected: Vec<Entity>,
    pub tick: u64,
    pub qi_spent: f64,
    pub radius_blocks: f32,
    pub shock_damage: f32,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct BloodBurnEvent {
    pub caster: Entity,
    pub tick: u64,
    pub hp_burned: f32,
    pub qi_multiplier: f32,
    pub active_until_tick: u64,
    pub ended_in_near_death: bool,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct DispersedQiEvent {
    pub caster: Entity,
    pub tick: u64,
    pub qi_max_before: f64,
    pub qi_max_after: f64,
    pub flow_rate_multiplier: f64,
    pub active_until_tick: Option<u64>,
    pub failed_reason: Option<String>,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct OverloadMeridianRippleEvent {
    pub caster: Entity,
    pub tick: u64,
    pub skill: BaomaiSkillId,
    pub severity_delta: f64,
    pub total_severity: f64,
    pub meridian_ids: Vec<MeridianId>,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct BodyTranscendenceExpiredEvent {
    pub caster: Entity,
    pub tick: u64,
}
