use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component};

use crate::cultivation::components::MeridianId;

use super::events::BaomaiSkillId;

#[derive(Debug, Clone, Default, Component, Serialize, Deserialize)]
pub struct BaomaiMastery {
    values: HashMap<BaomaiSkillId, f32>,
}

impl BaomaiMastery {
    pub fn level(&self, skill: BaomaiSkillId) -> u8 {
        self.values
            .get(&skill)
            .copied()
            .unwrap_or_default()
            .clamp(0.0, 100.0)
            .round() as u8
    }

    #[cfg(test)]
    pub fn set_level(&mut self, skill: BaomaiSkillId, value: f32) {
        self.values.insert(skill, value.clamp(0.0, 100.0));
    }

    pub fn grant_cast_xp(&mut self, skill: BaomaiSkillId) -> f32 {
        let current = self.values.entry(skill).or_insert(0.0);
        let gain = mastery_gain(*current);
        *current = (*current + gain).clamp(0.0, 100.0);
        *current
    }
}

pub fn mastery_gain(current: f32) -> f32 {
    if current < 50.0 {
        0.5
    } else if current < 80.0 {
        0.2
    } else {
        0.05
    }
}

#[derive(Debug, Clone, Component, Serialize, Deserialize, PartialEq)]
pub struct BloodBurnActive {
    pub started_at_tick: u64,
    pub active_until_tick: u64,
    pub hp_burned: f32,
    pub qi_multiplier: f32,
    pub cooldown_until_tick: u64,
}

impl BloodBurnActive {
    pub fn is_active_at(&self, tick: u64) -> bool {
        tick < self.active_until_tick
    }
}

#[derive(Debug, Clone, Component, Serialize, Deserialize, PartialEq)]
pub struct BodyTranscendence {
    pub started_at_tick: u64,
    pub active_until_tick: u64,
    pub flow_rate_multiplier: f64,
    pub qi_max_lost: f64,
    pub cooldowns_reset: bool,
    pub overload_tear_suppressed: bool,
    pub original_flow_rates: Vec<(MeridianId, f64)>,
}

impl BodyTranscendence {
    pub fn is_active_at(&self, tick: u64) -> bool {
        tick < self.active_until_tick
    }
}

#[derive(Debug, Clone, Default, Component, Serialize, Deserialize, PartialEq)]
pub struct MeridianRippleScar {
    pub severity: f64,
    pub accumulated_overloads: u32,
    pub last_updated_tick: u64,
}

#[derive(Debug, Clone, Default, Component, Serialize, Deserialize, PartialEq)]
pub struct DisperseCastHistory {
    pub cast_ticks: Vec<u64>,
}

impl DisperseCastHistory {
    pub fn record_and_count_recent(&mut self, tick: u64, window_ticks: u64) -> usize {
        self.cast_ticks
            .retain(|prior| tick.saturating_sub(*prior) <= window_ticks);
        self.cast_ticks.push(tick);
        self.cast_ticks.len()
    }
}
