//! plan-dandao-path-v1 — 丹道流派底盘 + 变异系统。
//!
//! P0: 累计丹毒追踪 + 三基础招式（服丹急行/投丹/丹雾）+ PracticeLog 温润色。
//! P1: 变异系统（MutationState + 阶段推进 + 顿悟 + 社会惩罚）。
//! P2: 丹道专属物品（5 灵草 + 6 丹药配方 + 变异催化炉 + 变异锻造）。
//! P3: 变异视觉同步 payload + HUD schema placeholder。
//! P4: 暴龙王 BOSS AI（三阶段 Scorer 链）。
//! P5: 境界递进 + 化虚大衍丹体（内炼）。

pub mod boss;
pub mod boss_ai;
pub mod boss_spawn;
pub mod catalyst_furnace;
pub mod components;
pub mod herbs;
pub mod internal_brew;
pub mod mutation;
pub mod mutation_forge;
pub mod progression;
pub mod recipes;
mod skills;
pub mod toxin_tracker;
pub mod visual_sync;

#[cfg(test)]
mod tests;

pub use skills::{DANDAO_PILL_BOMB_SKILL_ID, DANDAO_PILL_MIST_SKILL_ID, DANDAO_PILL_RUSH_SKILL_ID};

use valence::prelude::*;

use crate::cultivation::skill_registry::SkillRegistry;

pub fn register(app: &mut App) {
    // P0: toxin tracking
    app.add_event::<toxin_tracker::PillIntakeTracked>();
    app.add_systems(Update, toxin_tracker::track_pill_intake_system);
    // P1: mutation advancement
    app.add_event::<mutation::MutationAdvanceEvent>();
    app.add_systems(Update, mutation::mutation_advance_system);
    // P1 (plan-dandao-runtime-wiring-v1): 变异视觉 server→client emit
    // mutation_advance_system 先推进 MutationState + emit event，emit system 再读 event 发包。
    app.add_systems(
        Update,
        crate::network::mutation_visual_emit::mutation_visual_emit_system
            .after(mutation::mutation_advance_system),
    );
    // P2 (plan-dandao-runtime-wiring-v1): 变异叙事 server→agent Redis publish
    // 读 MutationAdvanceEvent → 组 MutationEventV1 → publish bong:mutation_event。
    app.add_systems(
        Update,
        crate::network::mutation_event_publish::mutation_event_publish_system
            .after(mutation::mutation_advance_system),
    );
    // P4 (plan-dandao-runtime-wiring-v1): 暴龙王 BOSS 遭遇战系统注册
    // big-brain 集成层（scorer/action）+ 真元吸取光环 + 死亡掉落 + 天道旁白。
    boss_spawn::register(app);
}

pub fn register_skills(registry: &mut SkillRegistry) {
    registry.register(DANDAO_PILL_RUSH_SKILL_ID, skills::resolve_pill_rush);
    registry.register(DANDAO_PILL_BOMB_SKILL_ID, skills::resolve_pill_bomb);
    registry.register(DANDAO_PILL_MIST_SKILL_ID, skills::resolve_pill_mist);
}
