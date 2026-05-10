//! `poison-trait-v1` — 毒性真元的泛型特性路径。
//!
//! 本模块只表达"吃毒丹积累特性 → 消化负荷 / 寿元 / 经脉代价 → 非毒蛊招式附毒"。
//! 毒蛊流派仍由 `combat::dugu_v2` 独立处理，不写 IdentityProfile，也不触发暴露事件。

pub mod attack_hook;
pub mod components;
pub mod events;
pub mod handlers;
pub mod recipes;
pub mod tick;

pub use components::{DigestionLoad, PoisonPillKind, PoisonSideEffectTag, PoisonToxicity};
pub use events::{
    ConsumePoisonPillIntent, DigestionOverloadEvent, PoisonDoseEvent, PoisonOverdoseEvent,
    PoisonOverdoseSeverity, PoisonPowderConsumedEvent,
};
pub use recipes::register_craft_recipes;
pub use tick::{
    apply_poison_overdose_costs, consume_poison_pill_system, digestion_load_decay_tick,
    poison_toxicity_decay_tick,
};

#[cfg(test)]
pub use attack_hook::{
    apply_poison_attack_modifier, poison_debuff_for_powder, poison_debuff_for_toxicity,
    PoisonAttackKind, PoisonDebuffTier,
};
#[cfg(test)]
pub use components::{digestion_capacity_for_realm, PoisonPowderKind};
#[cfg(test)]
pub use handlers::{
    calculate_overdose_severity, consume_poison_pill_now, poison_side_effect_tag_for_item,
    PoisonConsumeOutcome,
};
#[cfg(test)]
pub use recipes::poison_alchemy_recipe_ids;
#[cfg(test)]
pub use tick::{decay_digestion_load, decay_poison_toxicity, poison_micro_tear_roll};

#[cfg(test)]
mod matrix_tests;
