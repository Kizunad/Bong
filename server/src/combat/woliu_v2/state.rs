use valence::prelude::{bevy_ecs, DVec3, Entity};

use crate::qi_physics::constants::VORTEX_TURBULENCE_DECAY_PER_SEC;
use crate::qi_physics::EnvField;
use crate::world::dimension::DimensionKind;

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
    pub active_until_tick: u64,
    pub cooldown_until_tick: u64,
}

#[derive(bevy_ecs::component::Component, Debug, Clone, PartialEq)]
pub struct TurbulenceField {
    pub caster: Entity,
    pub center: DVec3,
    pub dimension: DimensionKind,
    pub source_zone: String,
    pub radius: f32,
    pub intensity: f32,
    pub decay_rate_per_second: f32,
    pub spawned_at_tick: u64,
    pub last_decay_tick: u64,
    pub remaining_swirl_qi: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurbulenceFieldOrigin {
    pub dimension: DimensionKind,
    pub source_zone: String,
}

impl TurbulenceFieldOrigin {
    pub fn new(dimension: DimensionKind, source_zone: impl Into<String>) -> Self {
        Self {
            dimension,
            source_zone: source_zone.into(),
        }
    }
}

impl TurbulenceField {
    pub fn new(
        caster: Entity,
        center: DVec3,
        origin: TurbulenceFieldOrigin,
        radius: f32,
        intensity: f32,
        swirl_qi: f32,
        tick: u64,
    ) -> Self {
        Self {
            caster,
            center,
            dimension: origin.dimension,
            source_zone: origin.source_zone,
            radius: radius.max(0.0),
            intensity: intensity.clamp(0.0, 1.0),
            decay_rate_per_second: VORTEX_TURBULENCE_DECAY_PER_SEC as f32,
            spawned_at_tick: tick,
            last_decay_tick: tick,
            remaining_swirl_qi: swirl_qi.max(0.0),
        }
    }
}

#[derive(bevy_ecs::component::Component, Debug, Clone, Copy, PartialEq)]
pub struct TurbulenceExposure {
    pub source: Entity,
    pub intensity: f32,
    pub until_tick: u64,
}

impl TurbulenceExposure {
    pub fn new(source: Entity, intensity: f32, until_tick: u64) -> Self {
        Self {
            source,
            intensity: intensity.clamp(0.0, 1.0),
            until_tick,
        }
    }

    pub fn env_field(self) -> EnvField {
        EnvField::default().with_turbulence(f64::from(self.intensity))
    }

    pub fn absorption_multiplier(self) -> f64 {
        self.env_field().turbulence_absorption_factor()
    }

    pub fn cast_precision_multiplier(self) -> f64 {
        self.env_field().turbulence_cast_precision_factor()
    }

    #[allow(dead_code)]
    pub fn defense_drain_multiplier(self) -> f64 {
        self.env_field().turbulence_defense_drain_factor()
    }
}

#[derive(bevy_ecs::component::Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct PassiveVortex {
    pub enabled: bool,
    pub toggled_at_tick: u64,
}

/// 涡流回响——被动延迟重播涡流招式。
///
/// 当 VortexCastEvent 触发时（且 `is_echo == false`），写入一条 ScheduledEcho。
/// tick system 到 `replay_at_tick` 后 emit 弱化版 VortexCastEvent（`is_echo = true`）。
#[allow(dead_code)]
#[derive(bevy_ecs::component::Component, Debug, Clone, PartialEq)]
pub struct ScheduledEcho {
    /// 原始施法 entity。
    pub caster: Entity,
    /// 原始招式。
    pub skill: WoliuSkillId,
    /// 原始施法位置。
    pub center: DVec3,
    /// 重播时刻。
    pub replay_at_tick: u64,
    /// 威力比（通常 0.4 = 40%）。
    pub power_ratio: f32,
    /// 原始 lethal_radius。
    pub original_lethal_radius: f32,
    /// 原始 influence_radius。
    pub original_influence_radius: f32,
    /// 原始 turbulence_radius。
    pub original_turbulence_radius: f32,
    /// 是否失控（方向随机化 / 可命中自己/队友）。
    pub misfired: bool,
}

/// 虚心坍缩态——追踪虚心持续状态的辅助 component。
///
/// 与 `StatusEffectKind::VoidCoreActive` 配合使用。StatusEffect 控制无敌/不可选中，
/// 此 component 追踪冲击波参数和回归时刻。
#[allow(dead_code)]
#[derive(bevy_ecs::component::Component, Debug, Clone, Copy, PartialEq)]
pub struct VoidCoreState {
    /// 回归冲击波时刻（= started_at + VOID_CORE_DURATION_TICKS）。
    pub shockwave_at_tick: u64,
    /// 冲击波伤害（预计算）。
    pub shockwave_damage: f64,
    /// 冲击波范围（格）。
    pub shockwave_radius: f32,
    /// 虚蚀阶段（用于 MeridianCrack / SEVERED 概率）。
    pub erosion_stage: super::erosion::VoidErosionStage,
}
