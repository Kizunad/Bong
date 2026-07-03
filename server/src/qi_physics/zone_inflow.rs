//! plan-zone-qi-economy-v1 P1 — 平衡回流公式。
//!
//! 独立待分配池（`ledger::pending_inflow_account`）按各 zone 的 `qi_equilibrium` /
//! `qi_inflow_per_min` 配置滴灌回 `zone.spirit_qi`，把浓度钳制在低平衡点、绝不过冲。
//! 本模块只提供纯函数；系统 wiring（读写 `ZoneRegistry` / `WorldQiAccount` /
//! `ActiveEventsResource`）落在 `world::heartbeat`（守恒记账需要同时看到 ledger 与
//! zone 真实字段，和 `npc::dormant::apply_dormant_regen_with_multiplier` 同一分工）。

use super::constants::QI_ZONE_UNIT_CAPACITY;

/// 计算本次评估窗口内，应从独立待分配池向该 zone 注入的**绝对灵气点**数量
/// （`QI_ZONE_UNIT_CAPACITY` 换算前的原始单位，与 `qi_inflow_per_min` 同量纲）。
///
/// 返回值已经钳制到「补满 `equilibrium` 即停」——调用方仍需在实际扣减待分配池前，
/// 用池子的真实余额再做一次 `min`（余额不足则缩量，绝不透支，§8.1 #1 红线）。
///
/// 参数：
/// - `spirit_qi`：zone 当前浓度（0..=1，`MIN_ZONE_SPIRIT_QI..=MAX_ZONE_SPIRIT_QI`）；
/// - `equilibrium`：`Zone::qi_equilibrium`，`<= 0.0` 视为"不回流"（向后兼容默认值）；
/// - `inflow_per_min`：`Zone::qi_inflow_per_min`，`<= 0.0` 视为"不回流"；
/// - `dt_minutes`：本次评估窗口经过的游戏内分钟数（由 `CultivationClock` tick 差值换算）。
///
/// 显式返回 `0.0` 的分支：任何输入非有限值；`equilibrium <= 0.0`；`inflow_per_min <= 0.0`；
/// `dt_minutes <= 0.0`；`spirit_qi` 非有限或已 `>= equilibrium`（含负灵域 `spirit_qi < 0.0`
/// 时若上游误传入本函数，因 `spirit_qi < equilibrium` 仍会给出非零结果——**调用方必须先做
/// 负灵域 `continue` 判断**，本函数不重复该守恒/worldview 语义，只负责钳位算术）。
pub fn zone_equilibrium_inflow(
    spirit_qi: f64,
    equilibrium: f64,
    inflow_per_min: f64,
    dt_minutes: f64,
) -> f64 {
    if !spirit_qi.is_finite() {
        return 0.0;
    }
    if !equilibrium.is_finite() || equilibrium <= 0.0 {
        return 0.0;
    }
    if !inflow_per_min.is_finite() || inflow_per_min <= 0.0 {
        return 0.0;
    }
    if !dt_minutes.is_finite() || dt_minutes <= 0.0 {
        return 0.0;
    }
    if spirit_qi >= equilibrium {
        return 0.0;
    }

    let needed_absolute = (equilibrium - spirit_qi) * QI_ZONE_UNIT_CAPACITY;
    let desired_absolute = inflow_per_min * dt_minutes;
    desired_absolute.min(needed_absolute).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qi_physics::constants::QI_EPSILON;

    // ───────────────────────── happy path ─────────────────────────

    #[test]
    fn injects_proportional_to_dt_when_far_below_equilibrium() {
        // needed = (0.5 - 0.1) * 50 = 20.0 absolute; desired = 0.4/min * 1 min = 0.4 << 20.0
        let amount = zone_equilibrium_inflow(0.1, 0.5, 0.4, 1.0);
        assert!(
            (amount - 0.4).abs() < QI_EPSILON,
            "expected the full per-minute rate (0.4) to be injected because the zone is far \
             below equilibrium (needed=20.0 >> desired=0.4), got {amount}"
        );
    }

    #[test]
    fn scales_linearly_with_dt_minutes() {
        let one_min = zone_equilibrium_inflow(0.1, 0.5, 0.4, 1.0);
        let ten_min = zone_equilibrium_inflow(0.1, 0.5, 0.4, 10.0);
        assert!(
            (ten_min - one_min * 10.0).abs() < QI_EPSILON,
            "inflow must scale linearly with elapsed minutes (10x dt -> 10x amount), got \
             one_min={one_min} ten_min={ten_min}"
        );
    }

    // ───────────────────────── clamp at equilibrium (no overshoot) ─────────────────────────

    #[test]
    fn clamps_to_exactly_the_needed_amount_when_dt_would_overshoot() {
        // needed = (0.35 - 0.3) * 50 = 2.5 absolute; desired = 0.4/min * 100 min = 40.0 >> needed
        let amount = zone_equilibrium_inflow(0.3, 0.35, 0.4, 100.0);
        assert!(
            (amount - 2.5).abs() < QI_EPSILON,
            "must clamp to exactly the absolute amount needed to reach equilibrium (2.5), \
             not the uncapped desired amount (40.0) — overshoot would break the 'clamp at \
             equilibrium, never overshoot' P1 invariant, got {amount}"
        );
    }

    #[test]
    fn returns_zero_when_already_at_equilibrium() {
        let amount = zone_equilibrium_inflow(0.35, 0.35, 0.4, 10.0);
        assert_eq!(
            amount, 0.0,
            "a zone already sitting exactly at equilibrium must receive zero inflow (boundary \
             off-by-one: >= not >)"
        );
    }

    #[test]
    fn returns_zero_when_above_equilibrium() {
        let amount = zone_equilibrium_inflow(0.9, 0.35, 0.4, 10.0);
        assert_eq!(
            amount, 0.0,
            "a zone above equilibrium (e.g. topped up by other systems) must not be drained \
             back down by this function — it only injects, never removes"
        );
    }

    // ───────────────────────── opt-out / back-compat defaults ─────────────────────────

    #[test]
    fn zero_equilibrium_is_explicit_opt_out() {
        let amount = zone_equilibrium_inflow(0.1, 0.0, 0.4, 10.0);
        assert_eq!(
            amount, 0.0,
            "qi_equilibrium == 0.0 is the P1 back-compat default meaning 'do not inflow this \
             zone' — pre-P1 zones.json snapshots and test fixtures rely on this being a no-op"
        );
    }

    #[test]
    fn negative_equilibrium_is_rejected_as_opt_out_not_a_crash() {
        let amount = zone_equilibrium_inflow(0.1, -0.2, 0.4, 10.0);
        assert_eq!(
            amount, 0.0,
            "negative equilibrium must be treated as opt-out, not panic/NaN"
        );
    }

    #[test]
    fn zero_inflow_per_min_is_explicit_opt_out() {
        let amount = zone_equilibrium_inflow(0.1, 0.5, 0.0, 10.0);
        assert_eq!(
            amount, 0.0,
            "qi_inflow_per_min == 0.0 is the P1 back-compat default meaning 'no inflow rate \
             configured' even if equilibrium is set"
        );
    }

    #[test]
    fn negative_inflow_per_min_is_rejected() {
        let amount = zone_equilibrium_inflow(0.1, 0.5, -0.4, 10.0);
        assert_eq!(amount, 0.0);
    }

    // ───────────────────────── dt boundary ─────────────────────────

    #[test]
    fn zero_dt_minutes_yields_zero_amount() {
        let amount = zone_equilibrium_inflow(0.1, 0.5, 0.4, 0.0);
        assert_eq!(
            amount, 0.0,
            "an evaluation window of zero elapsed minutes (e.g. duplicate tick observation) \
             must not inject any qi"
        );
    }

    #[test]
    fn negative_dt_minutes_is_rejected() {
        let amount = zone_equilibrium_inflow(0.1, 0.5, 0.4, -5.0);
        assert_eq!(
            amount, 0.0,
            "a negative dt (clock went backwards) must not be treated as a valid window"
        );
    }

    // ───────────────────────── non-finite inputs ─────────────────────────

    #[test]
    fn non_finite_spirit_qi_yields_zero() {
        assert_eq!(zone_equilibrium_inflow(f64::NAN, 0.5, 0.4, 1.0), 0.0);
        assert_eq!(zone_equilibrium_inflow(f64::INFINITY, 0.5, 0.4, 1.0), 0.0);
    }

    #[test]
    fn non_finite_equilibrium_yields_zero() {
        assert_eq!(zone_equilibrium_inflow(0.1, f64::NAN, 0.4, 1.0), 0.0);
        assert_eq!(zone_equilibrium_inflow(0.1, f64::INFINITY, 0.4, 1.0), 0.0);
    }

    #[test]
    fn non_finite_inflow_per_min_yields_zero() {
        assert_eq!(zone_equilibrium_inflow(0.1, 0.5, f64::NAN, 1.0), 0.0);
        assert_eq!(zone_equilibrium_inflow(0.1, 0.5, f64::INFINITY, 1.0), 0.0);
    }

    #[test]
    fn non_finite_dt_minutes_yields_zero() {
        assert_eq!(zone_equilibrium_inflow(0.1, 0.5, 0.4, f64::NAN), 0.0);
        assert_eq!(zone_equilibrium_inflow(0.1, 0.5, 0.4, f64::INFINITY), 0.0);
    }

    // ───────────────────────── negative zone qi (defensive, caller still gates this) ─────────

    #[test]
    fn negative_spirit_qi_below_equilibrium_still_computes_an_amount() {
        // 本函数本身不做负灵域判断（这是调用方 heartbeat system 的职责，§8.1 #5）；
        // 这条 pin 住"纯算术函数不悄悄加她自己的负灵域例外"，防止未来有人重复实现该判断
        // 导致两处逻辑漂移。
        let amount = zone_equilibrium_inflow(-0.3, 0.35, 0.4, 1.0);
        assert!(
            amount > 0.0,
            "this pure function does not special-case negative spirit_qi — that gate lives in \
             the caller (heartbeat system, §8.1 #5); pin this so it isn't silently duplicated \
             here and drift from the caller's logic"
        );
    }
}
