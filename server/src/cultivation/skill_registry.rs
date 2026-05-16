use std::collections::HashMap;

use valence::prelude::{Entity, Resource};

use crate::cultivation::components::MeridianId;

#[derive(Debug, Clone, PartialEq)]
pub enum CastResult {
    Started {
        cooldown_ticks: u64,
        anim_duration_ticks: u32,
    },
    Rejected {
        reason: CastRejectReason,
    },
    Interrupted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CastRejectReason {
    RealmTooLow,
    /// 招式依赖经脉永久 SEVERED 或当前 integrity 不可用。`Option<MeridianId>` 标
    /// 出首条不满足的依赖经脉；旧调用点未携带具体 id 时为 `None`（plan-meridian-
    /// severed-v1 上线后逐步收口）。
    MeridianSevered(Option<MeridianId>),
    QiInsufficient,
    OnCooldown,
    InvalidTarget,
    InRecovery,
}

impl CastRejectReason {
    /// 兼容旧调用点 —— 不带 meridian_id 的 `MeridianSevered`。
    pub const MERIDIAN_SEVERED: CastRejectReason = CastRejectReason::MeridianSevered(None);
}

pub type SkillFn = fn(
    &mut valence::prelude::bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
) -> CastResult;

#[derive(Default)]
pub struct SkillRegistry {
    entries: HashMap<&'static str, SkillFn>,
}

impl Resource for SkillRegistry {}

impl SkillRegistry {
    pub fn register(&mut self, skill_id: &'static str, skill_fn: SkillFn) {
        self.entries.insert(skill_id, skill_fn);
    }

    pub fn lookup(&self, skill_id: &str) -> Option<SkillFn> {
        self.entries.get(skill_id).copied()
    }
}

pub fn init_registry() -> SkillRegistry {
    let mut registry = SkillRegistry::default();
    crate::combat::carrier::register_skills(&mut registry);
    crate::combat::anqi_v2::register_skills(&mut registry);
    crate::cultivation::burst_meridian::register_skills(&mut registry);
    crate::combat::jiemai::register_skills(&mut registry);
    crate::combat::zhenmai_v2::register_skills(&mut registry);
    crate::combat::woliu::register_skills(&mut registry);
    crate::combat::yidao::register_skills(&mut registry);
    crate::combat::woliu_v2::register_skills(&mut registry);
    crate::combat::dugu_v2::register_skills(&mut registry);
    crate::combat::baomai_v3::register_skills(&mut registry);
    crate::combat::tuike_v2::register_skills(&mut registry);
    crate::combat::sword_basics::register_skills(&mut registry);
    crate::cultivation::dugu::register_skills(&mut registry);
    crate::cultivation::full_power_strike::register_skills(&mut registry);
    crate::dandao::register_skills(&mut registry);
    registry
}
