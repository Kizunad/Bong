//! plan-sword-path-v1 — 器修·剑道流派模块。
//!
//! 人剑共生 + 剑品阶 + 五招 + 天道盲区。
//! plan-sword-path-v2 接入 ECS：bond tracking / shatter / heaven gate cast /
//! 盲区 tick 等 Bevy system；招式 cast 走 `skill_register`。

pub mod bond;
pub mod grade;
pub mod heaven_gate;
pub mod shatter;
pub mod skill_register;
pub mod systems;
pub mod techniques;
pub mod tiandao_blind;
pub mod upgrade;

use valence::prelude::*;

pub fn register(app: &mut App) {
    app.add_event::<bond::SwordBondFormedEvent>()
        .add_event::<bond::SwordShatterEvent>()
        .add_event::<heaven_gate::HeavenGateCastEvent>()
        .init_resource::<heaven_gate::TiandaoBlindZoneRegistry>()
        .add_systems(
            Update,
            (
                systems::sword_bond_tracking_system,
                systems::sword_shatter_system,
                systems::tiandao_blind_zone_tick_system,
                skill_register::heaven_gate_cast_system,
            ),
        );
}
