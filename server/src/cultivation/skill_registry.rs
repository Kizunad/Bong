use std::collections::HashMap;

use valence::prelude::{Entity, Resource};

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
    MeridianSevered,
    QiInsufficient,
    OnCooldown,
    InvalidTarget,
    InRecovery,
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
    crate::cultivation::burst_meridian::register_skills(&mut registry);
    crate::combat::woliu::register_skills(&mut registry);
    registry
}
