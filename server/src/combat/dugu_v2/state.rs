use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component, Entity};

use crate::cultivation::components::QiColor;

use super::events::{DuguSkillId, TaintTier};

#[derive(Debug, Clone, Component, Serialize, Deserialize)]
pub struct DuguState {
    pub insidious_color_percent: f32,
    pub morphology_percent: f32,
    pub self_revealed: bool,
    pub self_cure_hours_today: f32,
    pub self_cure_day: u64,
}

impl Default for DuguState {
    fn default() -> Self {
        Self {
            insidious_color_percent: 0.0,
            morphology_percent: 0.0,
            self_revealed: false,
            self_cure_hours_today: 0.0,
            self_cure_day: 0,
        }
    }
}

impl DuguState {
    pub fn reset_daily_if_needed(&mut self, day: u64) {
        if self.self_cure_day != day {
            self.self_cure_day = day;
            self.self_cure_hours_today = 0.0;
        }
    }
}

#[derive(Debug, Clone, Component, Serialize, Deserialize)]
pub struct TaintMark {
    pub caster: Entity,
    pub intensity: f32,
    pub since_tick: u64,
    pub expires_at_tick: Option<u64>,
    pub tier: TaintTier,
    pub temporary_qi_max_loss: f32,
    pub permanent_decay_rate_per_min: f32,
    pub returned_zone_qi: f32,
}

impl TaintMark {
    pub fn is_permanent(&self) -> bool {
        self.tier == TaintTier::Permanent
    }
}

#[derive(Debug, Clone, Component, Serialize, Deserialize)]
pub struct ShroudActive {
    pub skill: DuguSkillId,
    pub strength: f32,
    pub fake_qi_color: QiColor,
    pub started_at_tick: u64,
    pub expires_at_tick: u64,
    pub permanent_until_cancelled: bool,
    pub maintain_qi_per_tick: f64,
}

#[derive(Debug, Clone, Component, Serialize, Deserialize)]
pub struct ReverseAftermathCloud {
    pub caster: Entity,
    pub intensity: f32,
    pub radius_blocks: f32,
    pub expires_at_tick: u64,
}
