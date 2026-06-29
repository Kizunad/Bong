//! plan-sword-path-v1 — 器修·剑道流派模块。
//!
//! 人剑共生 + 剑品阶 + 五招 + 天道盲区。
//! plan-sword-path-v2 接入 ECS：bond tracking / shatter / heaven gate cast /
//! 盲区 tick 等 Bevy system；招式 cast 走 `skill_register`。

pub mod av_event;
pub mod bond;
pub mod grade;
pub mod heaven_gate;
pub mod shatter;
pub mod skill_register;
pub mod sword_intent_entity;
pub mod systems;
pub mod techniques;
pub mod tiandao_blind;
pub mod upgrade;

use valence::prelude::*;

pub fn register(app: &mut App) {
    app.add_event::<bond::SwordBondFormedEvent>()
        .add_event::<bond::SwordShatterEvent>()
        .add_event::<heaven_gate::HeavenGateCastEvent>()
        .add_event::<av_event::SwordPathSkillCastEvent>()
        .init_resource::<heaven_gate::TiandaoBlindZoneRegistry>()
        .add_systems(
            Update,
            (
                systems::sword_bond_tracking_system,
                systems::sword_shatter_system,
                systems::tiandao_blind_zone_tick_system,
                // legacy event-driven settlement（保留向后兼容，cast_heaven_gate 不再触发它）
                skill_register::heaven_gate_cast_system,
                // §D 四阶段 phase system（天门 HeavenGateChanneling → elapsed 推进）
                skill_register::heaven_gate_phase_system,
                // §C 剑意追踪实体 tick system
                sword_intent_entity::sword_intent_tracking_system,
            ),
        );
}
