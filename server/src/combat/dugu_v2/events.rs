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
    /// Total qi drained from all affected targets' `qi_current` during this cast.
    /// Consumed by `penetrate_zone_credit_tick` to credit the target's zone.
    pub returned_zone_qi: f32,
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

/// bughunt r8 — Reverse（倒蚀）将受害者真元库清零时，实际被抹除的 qi_current 总量。
///
/// 守恒约束：victims 的 qi_current 被清零（累计 victim_qi_total），这部分真元必须归还到
/// 受害者所在 zone。此事件与 ReverseTriggeredEvent 并行发送，由
/// reverse_victim_qi_zone_credit_tick 消费，走 DuguReverseVictimQi 审计轨迹。
///
/// 注意：victim_qi_total 与 ReverseTriggeredEvent.returned_zone_qi（脏真元残留）是
/// **正交的**两条路径，不重复入账。
#[derive(Debug, Clone, Event, PartialEq)]
pub struct DuguReverseVictimQiEvent {
    pub caster: Entity,
    /// 所有受害者被清零的 qi_current 之和（≥ 0）
    pub victim_qi_total: f32,
    /// 受害者位置（用于 zone 查找，同 ReverseTriggeredEvent.center 逻辑）
    pub center: DVec3,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct DuguSelfRevealedEvent {
    pub caster: Entity,
    pub insidious_color_percent: f32,
    pub morphology_percent: f32,
    pub tick: u64,
}
