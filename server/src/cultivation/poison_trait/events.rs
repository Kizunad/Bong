use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Entity};

use super::components::{PoisonPillKind, PoisonPowderKind, PoisonSideEffectTag};

#[derive(Debug, Clone, bevy_ecs::event::Event, PartialEq, Serialize, Deserialize)]
pub struct ConsumePoisonPillIntent {
    pub entity: Entity,
    pub pill: PoisonPillKind,
    pub issued_at_tick: u64,
}

#[derive(Debug, Clone, bevy_ecs::event::Event, PartialEq, Serialize, Deserialize)]
pub struct PoisonDoseEvent {
    pub player: Entity,
    pub dose_amount: f32,
    pub side_effect_tag: PoisonSideEffectTag,
    pub poison_level_after: f32,
    pub digestion_after: f32,
    pub at_tick: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PoisonOverdoseSeverity {
    Mild,
    Moderate,
    Severe,
}

impl PoisonOverdoseSeverity {
    pub fn lifespan_penalty_years(self) -> f32 {
        match self {
            Self::Mild => 0.1,
            Self::Moderate => 1.0,
            Self::Severe => 5.0,
        }
    }

    pub fn micro_tear_probability(self) -> f32 {
        match self {
            Self::Mild => 0.0,
            Self::Moderate => 0.10,
            Self::Severe => 0.30,
        }
    }

    pub fn micro_tear_severity(self) -> f64 {
        match self {
            Self::Mild => 0.0,
            Self::Moderate => 0.08,
            Self::Severe => 0.18,
        }
    }
}

#[derive(Debug, Clone, bevy_ecs::event::Event, PartialEq, Serialize, Deserialize)]
pub struct PoisonOverdoseEvent {
    pub player: Entity,
    pub severity: PoisonOverdoseSeverity,
    pub overflow: f32,
    pub lifespan_penalty_years: f32,
    pub micro_tear_probability: f32,
    pub at_tick: u64,
}

#[derive(Debug, Clone, bevy_ecs::event::Event, PartialEq, Serialize, Deserialize)]
pub struct DigestionOverloadEvent {
    pub player: Entity,
    pub current: f32,
    pub capacity: f32,
    pub overflow: f32,
    pub at_tick: u64,
}

#[derive(Debug, Clone, bevy_ecs::event::Event, PartialEq, Serialize, Deserialize)]
pub struct PoisonPowderConsumedEvent {
    pub player: Entity,
    pub powder: PoisonPowderKind,
    pub target: Option<Entity>,
    pub at_tick: u64,
}
