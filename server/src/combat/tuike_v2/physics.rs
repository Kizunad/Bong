use crate::cultivation::color::PracticeLog;
use crate::cultivation::components::{ColorKind, Realm};
use crate::qi_physics;

use super::state::{FalseSkinLayer, FalseSkinTier, StackedFalseSkins};

pub use crate::qi_physics::constants::TUIKE_BETA;
pub const SHED_CURRENT_QI_RATIO: f64 = 0.05;
pub const ACTIVE_SHED_COOLDOWN_TICKS: u64 = 8 * crate::combat::components::TICKS_PER_SECOND;
pub const TRANSFER_STANDARD_COOLDOWN_TICKS: u64 = 5 * crate::combat::components::TICKS_PER_SECOND;
pub const TRANSFER_PERMANENT_COOLDOWN_TICKS: u64 = 30 * crate::combat::components::TICKS_PER_SECOND;
pub const NAKED_DEFENSE_DAMAGE_MULTIPLIER: f32 = 1.5;
pub const RESIDUE_DECAY_MIN_TICKS: u64 = 10 * 60 * crate::combat::components::TICKS_PER_SECOND;
pub const RESIDUE_DECAY_MAX_TICKS: u64 = 30 * 60 * crate::combat::components::TICKS_PER_SECOND;

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub struct ShedToCarrierOutcome {
    pub damage_absorbed: f64,
    pub damage_overflow: f64,
    pub contam_absorbed: f64,
    pub contam_overflow: f64,
    pub depleted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransferTaintOutcome {
    pub contam_moved_percent: f64,
    pub qi_cost: f64,
    pub backflow_percent: f64,
    pub permanent_absorbed: f64,
}

pub fn shed_start_cost(qi_current: f64) -> f64 {
    if qi_current.is_finite() {
        qi_current.max(0.0) * SHED_CURRENT_QI_RATIO * TUIKE_BETA
    } else {
        0.0
    }
}

pub fn transfer_cooldown_ticks(permanent_absorbed: f64) -> u64 {
    if permanent_absorbed > f64::EPSILON {
        TRANSFER_PERMANENT_COOLDOWN_TICKS
    } else {
        TRANSFER_STANDARD_COOLDOWN_TICKS
    }
}

pub fn naked_defense_damage_multiplier(stack: Option<&StackedFalseSkins>, now_tick: u64) -> f32 {
    if stack.is_some_and(|stack| stack.is_empty() && now_tick < stack.naked_until_tick) {
        NAKED_DEFENSE_DAMAGE_MULTIPLIER
    } else {
        1.0
    }
}

pub fn realm_rank(realm: Realm) -> u8 {
    match realm {
        Realm::Awaken => 0,
        Realm::Induce => 1,
        Realm::Condense => 2,
        Realm::Solidify => 3,
        Realm::Spirit => 4,
        Realm::Void => 5,
    }
}

pub fn max_layers_for_realm(realm: Realm) -> usize {
    match realm {
        Realm::Awaken | Realm::Induce | Realm::Condense => 1,
        Realm::Solidify | Realm::Spirit => 2,
        Realm::Void => 3,
    }
}

#[allow(dead_code)]
pub fn max_tier_for_realm(realm: Realm) -> FalseSkinTier {
    match realm {
        Realm::Awaken | Realm::Induce => FalseSkinTier::Fan,
        Realm::Condense => FalseSkinTier::Light,
        Realm::Solidify => FalseSkinTier::Mid,
        Realm::Spirit => FalseSkinTier::Heavy,
        Realm::Void => FalseSkinTier::Ancient,
    }
}

pub fn can_wear_tier(realm: Realm, tier: FalseSkinTier) -> bool {
    realm_rank(realm) >= realm_rank(tier.min_realm())
}

pub fn transfer_qi_per_contam_percent(realm: Realm) -> f64 {
    match realm {
        Realm::Awaken => 15.0,
        Realm::Induce => 13.0,
        Realm::Condense => 11.0,
        Realm::Solidify => 10.0,
        Realm::Spirit => 9.0,
        Realm::Void => 7.0,
    }
}

pub fn transfer_limit_percent(realm: Realm) -> f64 {
    match realm {
        Realm::Awaken => 1.0,
        Realm::Induce => 2.0,
        Realm::Condense => 3.0,
        Realm::Solidify => 5.0,
        Realm::Spirit => 8.0,
        Realm::Void => 15.0,
    }
}

pub fn can_absorb_permanent_taint(realm: Realm, tier: FalseSkinTier) -> bool {
    realm == Realm::Void && tier == FalseSkinTier::Ancient
}

pub fn solid_color_share(log: Option<&PracticeLog>) -> f64 {
    let Some(log) = log else {
        return 0.0;
    };
    let total = log.total();
    if total <= f64::EPSILON {
        return 0.0;
    }
    log.weights.get(&ColorKind::Solid).copied().unwrap_or(0.0) / total
}

pub fn maintenance_discount(log: Option<&PracticeLog>) -> f64 {
    if solid_color_share(log) >= 0.30 {
        0.5
    } else {
        1.0
    }
}

pub fn maintenance_qi_per_sec(stack: &StackedFalseSkins, log: Option<&PracticeLog>) -> f64 {
    stack
        .layers
        .iter()
        .map(|layer| layer.tier.maintain_qi_per_sec())
        .sum::<f64>()
        * maintenance_discount(log)
}

pub fn residue_decay_ticks_for_tier(tier: FalseSkinTier) -> u64 {
    match tier {
        FalseSkinTier::Fan | FalseSkinTier::Light => RESIDUE_DECAY_MIN_TICKS,
        FalseSkinTier::Mid | FalseSkinTier::Heavy => {
            (RESIDUE_DECAY_MIN_TICKS + RESIDUE_DECAY_MAX_TICKS) / 2
        }
        FalseSkinTier::Ancient => RESIDUE_DECAY_MAX_TICKS,
    }
}

#[allow(dead_code)]
pub fn shed_to_carrier(
    layer: &mut FalseSkinLayer,
    incoming_damage: f64,
    incoming_contam_percent: f64,
) -> ShedToCarrierOutcome {
    let physics = qi_physics::field::shed_to_carrier(
        layer.remaining_damage_capacity(),
        incoming_damage,
        incoming_contam_percent,
    );
    layer.damage_taken += physics.damage_absorbed;
    let contam_before = layer.contam_load;
    let incoming_contam = if incoming_contam_percent.is_finite() {
        incoming_contam_percent.max(0.0)
    } else {
        0.0
    };
    layer.contam_load =
        (contam_before + physics.contam_absorbed).clamp(0.0, layer.contam_capacity_percent());
    let actual_contam_absorbed = (layer.contam_load - contam_before).max(0.0);
    let actual_contam_overflow = (incoming_contam - actual_contam_absorbed).max(0.0);
    ShedToCarrierOutcome {
        damage_absorbed: physics.damage_absorbed,
        damage_overflow: physics.damage_overflow,
        contam_absorbed: actual_contam_absorbed,
        contam_overflow: actual_contam_overflow,
        depleted: layer.remaining_damage_capacity() <= f64::EPSILON
            || layer.remaining_contam_capacity_percent() <= f64::EPSILON,
    }
}

pub fn transfer_taint_to_outer_skin(
    stack: &mut StackedFalseSkins,
    realm: Realm,
    available_contam_percent: f64,
    qi_current: f64,
    permanent_decay_amount: Option<f64>,
) -> Option<TransferTaintOutcome> {
    let layer = stack.outer_mut()?;
    let limit = transfer_limit_percent(realm);
    let requested = available_contam_percent.max(0.0).min(limit);
    let capacity = layer.remaining_contam_capacity_percent();
    let rate = transfer_qi_per_contam_percent(realm).max(f64::EPSILON);
    let qi_limited = (qi_current.max(0.0) / rate).max(0.0);
    let moved = requested.min(capacity).min(qi_limited);
    let backflow = (requested - capacity).clamp(0.0, 5.0);
    let qi_cost = moved * rate;
    layer.contam_load = (layer.contam_load + moved).clamp(0.0, layer.contam_capacity_percent());

    let permanent_absorbed = if permanent_decay_amount.is_some_and(|amount| amount > 0.0)
        && can_absorb_permanent_taint(realm, layer.tier)
    {
        let amount = permanent_decay_amount.unwrap_or_default();
        layer.permanent_taint_load += amount;
        amount
    } else {
        0.0
    };

    Some(TransferTaintOutcome {
        contam_moved_percent: moved,
        qi_cost,
        backflow_percent: backflow,
        permanent_absorbed,
    })
}
