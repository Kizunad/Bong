//! P5 §6.1 + §8.1 #4 — 化虚大衍丹体：体内炼丹。
//!
//! 复用 AlchemyAutoProfile 的 feed_stage/tick/classify 链路。
//! is_internal 分支跳过 FurnaceQiReserve → 直接扣 player.qi_current。
//! qi 消耗走 QiTransfer。

use crate::cultivation::components::Realm;

use super::components::DandaoStyle;
use super::mutation::MutationState;
use super::progression::{GREAT_TRANSMUTATION_PENALTY_PER_USE, GREAT_TRANSMUTATION_TOXIN_COST};

/// 大衍丹体是否可用（前置条件检查）。
/// 条件：化虚境界 + 变异阶段 >= 3 + DandaoStyle 已初始化。
pub fn can_internal_brew(
    realm: Realm,
    mutation_stage: u8,
    _dandao_style: &DandaoStyle,
) -> InternalBrewCheck {
    if realm != Realm::Void {
        return InternalBrewCheck::RealmTooLow;
    }
    if mutation_stage < 3 {
        return InternalBrewCheck::MutationStageTooLow;
    }
    InternalBrewCheck::Ok
}

/// 前置检查结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InternalBrewCheck {
    Ok,
    RealmTooLow,
    MutationStageTooLow,
}

/// 内炼 qi 消耗计算。
/// §8.1 #4: 直接扣 player.qi_current，不走 FurnaceQiReserve。
/// 消耗 = capacity_for_tier(5) × 10%（化虚炼丹的固定比例）。
pub const INTERNAL_BREW_QI_RATIO: f64 = 0.10;

/// 计算内炼 qi 消耗。
pub fn internal_brew_qi_cost(realm: Realm) -> f64 {
    use crate::cultivation::components::Meridian;
    let tier = match realm {
        Realm::Awaken => 0,
        Realm::Induce => 1,
        Realm::Condense => 2,
        Realm::Solidify => 3,
        Realm::Spirit => 4,
        Realm::Void => 5,
    };
    Meridian::capacity_for_tier(tier) * INTERNAL_BREW_QI_RATIO
}

/// 内炼 cast 时长（ticks）。
pub const INTERNAL_BREW_CAST_TICKS: u64 = 100; // 5s × 20 tps

/// 应用内炼代价。
/// 返回 (new_cumulative_toxin_delta, new_meridian_penalty_delta)。
pub fn apply_internal_brew_cost(style: &mut DandaoStyle, state: &mut MutationState) -> (f64, f64) {
    let toxin_delta = GREAT_TRANSMUTATION_TOXIN_COST;
    let penalty_delta = GREAT_TRANSMUTATION_PENALTY_PER_USE;

    style.cumulative_toxin += toxin_delta;
    state.meridian_penalty += penalty_delta;

    (toxin_delta, penalty_delta)
}

#[cfg(test)]
mod internal_brew_tests {
    use super::*;
    use crate::cultivation::components::Realm;
    use crate::dandao::components::DandaoStyle;
    use crate::dandao::components::MutationStage;
    use crate::dandao::mutation::MutationState;
    use crate::dandao::progression::{
        GREAT_TRANSMUTATION_PENALTY_PER_USE, GREAT_TRANSMUTATION_TOXIN_COST,
    };

    fn default_dandao() -> DandaoStyle {
        DandaoStyle {
            cumulative_toxin: 300.0,
            mutation_stage: 3,
            ..DandaoStyle::default()
        }
    }

    // ── 前置检查 ──────────────────────────────

    #[test]
    fn can_internal_brew_ok() {
        let style = default_dandao();
        assert_eq!(
            can_internal_brew(Realm::Void, 3, &style),
            InternalBrewCheck::Ok,
            "化虚 + 变异阶段 3 应可内炼"
        );
    }

    #[test]
    fn can_internal_brew_ok_stage_4() {
        let style = default_dandao();
        assert_eq!(
            can_internal_brew(Realm::Void, 4, &style),
            InternalBrewCheck::Ok,
            "化虚 + 变异阶段 4 应可内炼"
        );
    }

    #[test]
    fn rejects_non_void_realm() {
        let style = default_dandao();
        for realm in [
            Realm::Awaken,
            Realm::Induce,
            Realm::Condense,
            Realm::Solidify,
            Realm::Spirit,
        ] {
            assert_eq!(
                can_internal_brew(realm, 4, &style),
                InternalBrewCheck::RealmTooLow,
                "{realm:?} 境界不应能内炼"
            );
        }
    }

    #[test]
    fn rejects_low_mutation_stage() {
        let style = default_dandao();
        for stage in 0..3 {
            assert_eq!(
                can_internal_brew(Realm::Void, stage, &style),
                InternalBrewCheck::MutationStageTooLow,
                "变异阶段 {stage} 不应能内炼"
            );
        }
    }

    // ── qi 消耗 ──────────────────────────────

    #[test]
    fn internal_brew_qi_cost_positive() {
        let cost = internal_brew_qi_cost(Realm::Void);
        assert!(cost > 0.0, "内炼 qi 消耗应为正数: got {cost}");
    }

    #[test]
    fn internal_brew_qi_cost_scales_with_realm() {
        let cost_condense = internal_brew_qi_cost(Realm::Condense);
        let cost_void = internal_brew_qi_cost(Realm::Void);
        assert!(
            cost_void > cost_condense,
            "化虚消耗({cost_void})应大于凝脉({cost_condense})"
        );
    }

    // ── cast 时长 ─────────────────────────────

    #[test]
    fn cast_duration_is_5s() {
        assert_eq!(INTERNAL_BREW_CAST_TICKS, 100, "内炼 cast = 5s = 100 ticks");
    }

    // ── 内炼代价 ──────────────────────────────

    #[test]
    fn apply_cost_increases_toxin() {
        let mut style = default_dandao();
        let mut state = MutationState {
            stage: MutationStage::Heavy,
            meridian_penalty: 0.15,
            ..MutationState::default()
        };
        let before_toxin = style.cumulative_toxin;
        let (toxin_delta, _) = apply_internal_brew_cost(&mut style, &mut state);

        assert!(
            (toxin_delta - GREAT_TRANSMUTATION_TOXIN_COST).abs() < f64::EPSILON,
            "toxin_delta 应为 {GREAT_TRANSMUTATION_TOXIN_COST}, got {toxin_delta}"
        );
        assert!(
            (style.cumulative_toxin - (before_toxin + GREAT_TRANSMUTATION_TOXIN_COST)).abs()
                < f64::EPSILON,
            "cumulative_toxin 应增加 {GREAT_TRANSMUTATION_TOXIN_COST}"
        );
    }

    #[test]
    fn apply_cost_increases_penalty() {
        let mut style = default_dandao();
        let mut state = MutationState {
            stage: MutationStage::Heavy,
            meridian_penalty: 0.15,
            ..MutationState::default()
        };
        let before_penalty = state.meridian_penalty;
        let (_, penalty_delta) = apply_internal_brew_cost(&mut style, &mut state);

        assert!(
            (penalty_delta - GREAT_TRANSMUTATION_PENALTY_PER_USE).abs() < f64::EPSILON,
            "penalty_delta 应为 {GREAT_TRANSMUTATION_PENALTY_PER_USE}"
        );
        assert!(
            (state.meridian_penalty - (before_penalty + GREAT_TRANSMUTATION_PENALTY_PER_USE)).abs()
                < f64::EPSILON,
            "meridian_penalty 应增加 {GREAT_TRANSMUTATION_PENALTY_PER_USE}"
        );
    }

    #[test]
    fn apply_cost_accumulates() {
        let mut style = default_dandao();
        let mut state = MutationState {
            stage: MutationStage::Heavy,
            meridian_penalty: 0.15,
            ..MutationState::default()
        };

        let initial_toxin = style.cumulative_toxin;
        let initial_penalty = state.meridian_penalty;

        // 3 次内炼
        for _ in 0..3 {
            apply_internal_brew_cost(&mut style, &mut state);
        }

        let expected_toxin = initial_toxin + 3.0 * GREAT_TRANSMUTATION_TOXIN_COST;
        let expected_penalty = initial_penalty + 3.0 * GREAT_TRANSMUTATION_PENALTY_PER_USE;

        assert!(
            (style.cumulative_toxin - expected_toxin).abs() < f64::EPSILON,
            "3 次内炼后 toxin 应为 {expected_toxin}, got {}",
            style.cumulative_toxin
        );
        assert!(
            (state.meridian_penalty - expected_penalty).abs() < f64::EPSILON,
            "3 次内炼后 penalty 应为 {expected_penalty}, got {}",
            state.meridian_penalty
        );
    }

    #[test]
    fn great_transmutation_toxin_cost_is_15() {
        assert!(
            (GREAT_TRANSMUTATION_TOXIN_COST - 15.0).abs() < f64::EPSILON,
            "大衍丹体每次内炼 toxin 代价应为 15.0"
        );
    }

    #[test]
    fn great_transmutation_penalty_is_001() {
        assert!(
            (GREAT_TRANSMUTATION_PENALTY_PER_USE - 0.01).abs() < f64::EPSILON,
            "大衍丹体每次内炼经脉惩罚应为 0.01 (1%)"
        );
    }
}
