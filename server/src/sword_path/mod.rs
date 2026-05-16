//! plan-sword-path-v1 — 器修·剑道流派模块。
//!
//! 人剑共生 + 剑品阶 + 五招 + 天道盲区。

pub mod bond;
pub mod grade;
pub mod shatter;
pub mod techniques;
pub mod tiandao_blind;

use valence::prelude::*;

pub fn register(app: &mut App) {
    app.add_event::<bond::SwordBondFormedEvent>()
        .add_event::<bond::SwordShatterEvent>();
}
