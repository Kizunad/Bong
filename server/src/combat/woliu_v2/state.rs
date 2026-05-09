use valence::prelude::{bevy_ecs, DVec3, Entity};

use crate::qi_physics::constants::VORTEX_TURBULENCE_DECAY_PER_SEC;
#[cfg(test)]
use crate::qi_physics::constants::{
    VORTEX_TURBULENCE_CAST_PRECISION_MULTIPLIER, VORTEX_TURBULENCE_DEFENSE_DRAIN_BONUS,
    VORTEX_TURBULENCE_SHELFLIFE_MULTIPLIER,
};

use super::events::{BackfireLevel, WoliuSkillId};

#[derive(bevy_ecs::component::Component, Debug, Clone, Copy, PartialEq)]
pub struct VortexV2State {
    pub active_skill_kind: WoliuSkillId,
    pub heart_passive_enabled: bool,
    pub lethal_radius: f32,
    pub influence_radius: f32,
    pub turbulence_radius: f32,
    pub turbulence_intensity: f32,
    pub backfire_level: Option<BackfireLevel>,
    pub started_at_tick: u64,
    pub cooldown_until_tick: u64,
}

#[derive(bevy_ecs::component::Component, Debug, Clone, Copy, PartialEq)]
pub struct TurbulenceField {
    pub caster: Entity,
    pub center: DVec3,
    pub radius: f32,
    pub intensity: f32,
    pub decay_rate_per_second: f32,
    pub spawned_at_tick: u64,
    pub last_decay_tick: u64,
    pub remaining_swirl_qi: f32,
}

impl TurbulenceField {
    pub fn new(
        caster: Entity,
        center: DVec3,
        radius: f32,
        intensity: f32,
        swirl_qi: f32,
        tick: u64,
    ) -> Self {
        Self {
            caster,
            center,
            radius: radius.max(0.0),
            intensity: intensity.clamp(0.0, 1.0),
            decay_rate_per_second: VORTEX_TURBULENCE_DECAY_PER_SEC as f32,
            spawned_at_tick: tick,
            last_decay_tick: tick,
            remaining_swirl_qi: swirl_qi.max(0.0),
        }
    }

    #[cfg(test)]
    pub fn absorption_multiplier(self) -> f32 {
        if self.intensity > 0.0 {
            0.0
        } else {
            1.0
        }
    }

    #[cfg(test)]
    pub fn cast_precision_multiplier(self) -> f32 {
        1.0 - self.intensity * (1.0 - VORTEX_TURBULENCE_CAST_PRECISION_MULTIPLIER as f32)
    }

    #[cfg(test)]
    pub fn shelflife_multiplier(self) -> f32 {
        1.0 + self.intensity * (VORTEX_TURBULENCE_SHELFLIFE_MULTIPLIER as f32 - 1.0)
    }

    #[cfg(test)]
    pub fn defense_drain_multiplier(self) -> f32 {
        1.0 + self.intensity * VORTEX_TURBULENCE_DEFENSE_DRAIN_BONUS as f32
    }
}

#[derive(bevy_ecs::component::Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct PassiveVortex {
    pub enabled: bool,
    pub toggled_at_tick: u64,
}
