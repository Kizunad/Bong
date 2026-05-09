//! 化虚专属 action（plan-void-actions-v1）。
//!
//! 本模块只承接 Realm::Void 之后的世界级行为，不复用 SkillRegistry，也不改
//! 渡虚劫 / quota 门槛。真元扣减必须经过 qi_physics ledger，寿元扣减走
//! LifespanComponent，公开叙事通过 `bong:void_action/*` fanout。

pub mod actions;
pub mod components;
pub mod ledger_hooks;
pub mod legacy;

use valence::prelude::{App, IntoSystemConfigs, Update};

use actions::{apply_barrier_dispel_system, resolve_void_action_intents};
use components::{BarrierDispelHistory, VoidActionCooldowns};
use ledger_hooks::{
    apply_due_void_qi_returns_system, despawn_expired_barriers_system, VoidQiReturnSchedule,
};

pub fn register(app: &mut App) {
    app.init_resource::<VoidActionCooldowns>();
    app.init_resource::<BarrierDispelHistory>();
    app.init_resource::<VoidQiReturnSchedule>();
    app.add_event::<actions::VoidActionIntent>();
    app.add_event::<actions::VoidActionBroadcast>();
    app.add_systems(
        Update,
        (
            resolve_void_action_intents.after(crate::cultivation::lifespan::lifespan_aging_tick),
            apply_barrier_dispel_system,
            apply_due_void_qi_returns_system,
            despawn_expired_barriers_system,
        )
            .chain(),
    );
}
