//! plan-dandao-path-v1 P0 — 丹道流派底盘。
//!
//! 横向辅助流派：累计丹毒追踪 + 三基础招式（服丹急行/投丹/丹雾）+ PracticeLog 温润色。

mod components;
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
    app.add_event::<toxin_tracker::PillIntakeTracked>();
    app.add_systems(Update, toxin_tracker::track_pill_intake_system);
}

pub fn register_skills(registry: &mut SkillRegistry) {
    registry.register(DANDAO_PILL_RUSH_SKILL_ID, skills::resolve_pill_rush);
    registry.register(DANDAO_PILL_BOMB_SKILL_ID, skills::resolve_pill_bomb);
    registry.register(DANDAO_PILL_MIST_SKILL_ID, skills::resolve_pill_mist);
}
