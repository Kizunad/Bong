//! plan-dandao-path-v1 — 丹道流派底盘 + 变异系统。
//!
//! P0: 累计丹毒追踪 + 三基础招式（服丹急行/投丹/丹雾）+ PracticeLog 温润色。
//! P1: 变异系统（MutationState + 阶段推进 + 顿悟 + 社会惩罚）。

pub mod boss;
pub mod components;
pub mod mutation;
pub mod progression;
mod skills;
mod toxin_tracker;

#[cfg(test)]
mod tests;

pub use skills::{
    DANDAO_PILL_BOMB_SKILL_ID, DANDAO_PILL_MIST_SKILL_ID, DANDAO_PILL_RUSH_SKILL_ID,
};

use valence::prelude::*;

use crate::cultivation::skill_registry::SkillRegistry;

pub fn register(app: &mut App) {
    // P0: toxin tracking
    app.add_event::<toxin_tracker::PillIntakeTracked>();
    app.add_systems(Update, toxin_tracker::track_pill_intake_system);
    // P1: mutation advancement
    app.add_event::<mutation::MutationAdvanceEvent>();
    app.add_systems(Update, mutation::mutation_advance_system);
}

pub fn register_skills(registry: &mut SkillRegistry) {
    registry.register(DANDAO_PILL_RUSH_SKILL_ID, skills::resolve_pill_rush);
    registry.register(DANDAO_PILL_BOMB_SKILL_ID, skills::resolve_pill_bomb);
    registry.register(DANDAO_PILL_MIST_SKILL_ID, skills::resolve_pill_mist);
}
