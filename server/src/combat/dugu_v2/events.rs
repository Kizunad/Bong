use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, DVec3, Entity, Event};

use crate::cultivation::components::Realm;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DuguSkillId {
    Eclipse,
    SelfCure,
    Penetrate,
    Shroud,
    Reverse,
}

impl DuguSkillId {
    #[cfg(test)]
    pub const ALL: [Self; 5] = [
        Self::Eclipse,
        Self::SelfCure,
        Self::Penetrate,
        Self::Shroud,
        Self::Reverse,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Eclipse => "dugu.eclipse",
            Self::SelfCure => "dugu.self_cure",
            Self::Penetrate => "dugu.penetrate",
            Self::Shroud => "dugu.shroud",
            Self::Reverse => "dugu.reverse",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaintTier {
    Immediate,
    Temporary,
    Permanent,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DuguSkillVisual {
    pub animation_id: &'static str,
    pub particle_id: &'static str,
    pub sound_recipe_id: &'static str,
    pub hud_hint: &'static str,
    pub icon_texture: &'static str,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct EclipseNeedleEvent {
    pub caster: Entity,
    pub target: Entity,
    pub target_realm: Realm,
    pub tier: TaintTier,
    pub injected_qi: f32,
    pub hp_loss: f32,
    pub qi_loss: f32,
    pub qi_max_loss: f32,
    pub permanent_decay_rate_per_min: f32,
    pub returned_zone_qi: f32,
    pub reveal_probability: f32,
    pub tick: u64,
    pub visual: DuguSkillVisual,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct SelfCureProgressEvent {
    pub caster: Entity,
    pub hours_used: f32,
    pub daily_hours_after: f32,
    pub gain_percent: f32,
    pub insidious_color_percent: f32,
    pub morphology_percent: f32,
    pub self_revealed: bool,
    pub tick: u64,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct PenetrateChainEvent {
    pub caster: Entity,
    pub target: Entity,
    pub taint_tier: TaintTier,
    pub multiplier: f32,
    pub affected_targets: u32,
    pub permanent_decay_rate_per_min: f32,
    pub reveal_probability: f32,
    pub tick: u64,
    pub visual: DuguSkillVisual,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct ShroudActivatedEvent {
    pub caster: Entity,
    pub strength: f32,
    pub expires_at_tick: u64,
    pub tick: u64,
    pub visual: DuguSkillVisual,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct ReverseTriggeredEvent {
    pub caster: Entity,
    pub affected_targets: u32,
    pub burst_damage: f32,
    pub returned_zone_qi: f32,
    pub juebi_delay_ticks: Option<u64>,
    pub tick: u64,
    pub center: DVec3,
    pub visual: DuguSkillVisual,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct PermanentQiMaxDecayApplied {
    pub target: Entity,
    pub caster: Entity,
    pub loss: f32,
    pub qi_max_after: f32,
    pub tick: u64,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct DuguSelfRevealedEvent {
    pub caster: Entity,
    pub insidious_color_percent: f32,
    pub morphology_percent: f32,
    pub tick: u64,
}
