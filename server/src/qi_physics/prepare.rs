//! plan-race-system-v1 P5 / PR-6a — `prepare_transfer`：双端账本预校验，产出**保证**
//! `apply` 阶段不会失败的转账计划。
//!
//! 与 [`super::release::qi_release_to_zone`] 的区别：`qi_release_to_zone` 只校验
//! **目标端**容量（信任调用方已经从来源扣减/持有对应余额），本函数**额外**校验
//! **来源端**余额是否充足（`amount <= from_balance`）——供 `cultivation::race_change`
//! 这类"两阶段事务：预检必须先证明两端都成立，`commit` 再确定性 apply、不留失败
//! 分支"的消费点使用。
//!
//! `prepare_transfer` 本身是纯函数、无 ECS/世界依赖：调用方负责把 `to_capacity`
//! 算成目标账户的**剩余容量**（如 `zone_cap - zone_current`，或溢出账户传
//! `f64::MAX` 表示"无容量上限，全部照单全收"），返回的 [`TransferPlan`] 里
//! `accepted`（保证 ≤ `to_capacity`）+ `overflow`（`to_capacity` 装不下的余量，
//! 调用方应再路由到一个无上限的 overflow 账户，模式见
//! `qi_physics::ledger::dying_elder_release_overflow_account` /
//! `cultivation::death_hooks::release_qi_amount_to_zone` 的既有 overflow 兜底）保证
//! `accepted + overflow == amount`，两段分别 apply 到目标账户 / overflow 账户都不会
//! 因为容量或余额不足而失败。

use super::{finite_non_negative, QiPhysicsError};

/// 双端预校验产出的转账计划——`accepted` 一定能装进 `to_capacity`，`overflow` 是
/// 目标容量装不下、调用方需另路由（通常是无上限 overflow 账户）的余量。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransferPlan {
    /// 目标账户实际能接收的量（`<= to_capacity`）。
    pub accepted: f64,
    /// 目标容量装不下、需路由到别处（如 overflow 账户）的余量。
    pub overflow: f64,
    /// 来源账户扣除 `amount` 后的余额。
    pub from_balance_after: f64,
    /// 目标账户接收 `accepted` 后剩余的容量。
    pub to_capacity_after: f64,
}

/// 校验 `from_balance` 是否足以支付 `amount`（来源端），再把 `amount` 按
/// `to_capacity`（目标端剩余容量）clamp 出 `accepted`/`overflow`——两段都验证过后，
/// `commit` 阶段照此 plan 执行两次减法/加法，保证不会撞见"来源不够扣"或"目标超收"
/// 的运行时错误。
///
/// 失败仅两种：
/// - 任一入参非有限或为负（`QiPhysicsError::InvalidAmount`）；
/// - `amount > from_balance`（`QiPhysicsError::InsufficientQi`，`account` 固定为
///   `"from_balance"`，供调用方按此 field 精确匹配，不泄露具体账户 id——那由调用方
///   在自己的错误上下文里补充）。
pub fn prepare_transfer(
    from_balance: f64,
    to_capacity: f64,
    amount: f64,
) -> Result<TransferPlan, QiPhysicsError> {
    let from_balance = finite_non_negative(from_balance, "from_balance")?;
    let to_capacity = finite_non_negative(to_capacity, "to_capacity")?;
    let amount = finite_non_negative(amount, "amount")?;

    if amount > from_balance {
        return Err(QiPhysicsError::InsufficientQi {
            account: "from_balance".to_string(),
            available: from_balance,
            requested: amount,
        });
    }

    let accepted = amount.min(to_capacity);
    let overflow = amount - accepted;

    Ok(TransferPlan {
        accepted,
        overflow,
        from_balance_after: from_balance - amount,
        to_capacity_after: to_capacity - accepted,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── happy path ──────────────────────────────────────────────────────────

    #[test]
    fn fits_entirely_within_capacity() {
        let plan = prepare_transfer(10.0, 20.0, 6.0).expect("valid transfer");
        assert_eq!(plan.accepted, 6.0);
        assert_eq!(plan.overflow, 0.0);
        assert_eq!(plan.from_balance_after, 4.0);
        assert_eq!(plan.to_capacity_after, 14.0);
    }

    #[test]
    fn zero_amount_is_a_true_noop() {
        let plan = prepare_transfer(10.0, 20.0, 0.0).expect("zero amount is valid");
        assert_eq!(plan.accepted, 0.0);
        assert_eq!(plan.overflow, 0.0);
        assert_eq!(plan.from_balance_after, 10.0);
        assert_eq!(plan.to_capacity_after, 20.0);
    }

    // ─── boundary: exact match ───────────────────────────────────────────────

    #[test]
    fn amount_exactly_equals_from_balance() {
        let plan = prepare_transfer(5.0, 100.0, 5.0).expect("amount == from_balance is valid");
        assert_eq!(plan.from_balance_after, 0.0, "来源应精确清零，不留残留");
        assert_eq!(plan.accepted, 5.0);
    }

    #[test]
    fn amount_exactly_equals_to_capacity() {
        let plan = prepare_transfer(10.0, 5.0, 5.0).expect("amount == to_capacity is valid");
        assert_eq!(plan.accepted, 5.0);
        assert_eq!(plan.overflow, 0.0);
        assert_eq!(plan.to_capacity_after, 0.0, "目标容量应精确用尽");
    }

    // ─── overflow clamp ──────────────────────────────────────────────────────

    #[test]
    fn overflow_when_amount_exceeds_capacity() {
        let plan = prepare_transfer(50.0, 10.0, 30.0).expect("valid transfer");
        assert_eq!(plan.accepted, 10.0, "只能吃满目标剩余容量");
        assert_eq!(
            plan.overflow, 20.0,
            "超出部分必须完整报告，供调用方路由到别处"
        );
        assert_eq!(
            plan.accepted + plan.overflow,
            30.0,
            "accepted+overflow 必须精确等于请求 amount，不能凭空增减"
        );
        assert_eq!(plan.from_balance_after, 20.0);
        assert_eq!(plan.to_capacity_after, 0.0);
    }

    #[test]
    fn zero_capacity_routes_entire_amount_to_overflow() {
        let plan = prepare_transfer(10.0, 0.0, 4.0).expect("valid transfer");
        assert_eq!(plan.accepted, 0.0);
        assert_eq!(plan.overflow, 4.0);
    }

    #[test]
    fn unbounded_capacity_via_f64_max_never_overflows() {
        // overflow 账户"无容量上限"的调用惯例：调用方传 f64::MAX。
        let plan = prepare_transfer(1_000_000.0, f64::MAX, 999_999.0).expect("valid transfer");
        assert_eq!(plan.accepted, 999_999.0);
        assert_eq!(plan.overflow, 0.0);
    }

    // ─── insufficient source balance ─────────────────────────────────────────

    #[test]
    fn amount_greater_than_from_balance_is_rejected() {
        let err = prepare_transfer(3.0, 100.0, 5.0).expect_err("must reject overdraft");
        assert!(matches!(
            err,
            QiPhysicsError::InsufficientQi {
                available: 3.0,
                requested: 5.0,
                ..
            }
        ));
    }

    #[test]
    fn insufficient_error_reports_from_balance_account_label() {
        let err = prepare_transfer(1.0, 100.0, 2.0).expect_err("must reject overdraft");
        let QiPhysicsError::InsufficientQi { account, .. } = err else {
            panic!("expected InsufficientQi, got {err:?}");
        };
        assert_eq!(account, "from_balance");
    }

    // ─── invalid amount: each of the three params, negative + NaN ───────────

    #[test]
    fn negative_from_balance_rejected() {
        let err = prepare_transfer(-1.0, 10.0, 1.0).expect_err("negative from_balance invalid");
        assert!(matches!(
            err,
            QiPhysicsError::InvalidAmount {
                field: "from_balance",
                ..
            }
        ));
    }

    #[test]
    fn nan_from_balance_rejected() {
        let err = prepare_transfer(f64::NAN, 10.0, 1.0).expect_err("NaN from_balance invalid");
        assert!(matches!(
            err,
            QiPhysicsError::InvalidAmount {
                field: "from_balance",
                ..
            }
        ));
    }

    #[test]
    fn negative_to_capacity_rejected() {
        let err = prepare_transfer(10.0, -1.0, 1.0).expect_err("negative to_capacity invalid");
        assert!(matches!(
            err,
            QiPhysicsError::InvalidAmount {
                field: "to_capacity",
                ..
            }
        ));
    }

    #[test]
    fn nan_to_capacity_rejected() {
        let err = prepare_transfer(10.0, f64::NAN, 1.0).expect_err("NaN to_capacity invalid");
        assert!(matches!(
            err,
            QiPhysicsError::InvalidAmount {
                field: "to_capacity",
                ..
            }
        ));
    }

    #[test]
    fn negative_amount_rejected() {
        let err = prepare_transfer(10.0, 10.0, -1.0).expect_err("negative amount invalid");
        assert!(matches!(
            err,
            QiPhysicsError::InvalidAmount {
                field: "amount",
                ..
            }
        ));
    }

    #[test]
    fn nan_amount_rejected() {
        let err = prepare_transfer(10.0, 10.0, f64::NAN).expect_err("NaN amount invalid");
        assert!(matches!(
            err,
            QiPhysicsError::InvalidAmount {
                field: "amount",
                ..
            }
        ));
    }

    #[test]
    fn infinite_amount_rejected() {
        let err = prepare_transfer(10.0, 10.0, f64::INFINITY).expect_err("infinite amount invalid");
        assert!(matches!(
            err,
            QiPhysicsError::InvalidAmount {
                field: "amount",
                ..
            }
        ));
    }

    // ─── invalid-amount short-circuits before insufficient-balance check ────

    #[test]
    fn invalid_amount_precedes_insufficient_balance_check() {
        // from_balance=0 且 amount 非法：应报 InvalidAmount 而不是先误判 InsufficientQi。
        let err = prepare_transfer(0.0, 10.0, f64::NAN).expect_err("NaN must win over overdraft");
        assert!(matches!(
            err,
            QiPhysicsError::InvalidAmount {
                field: "amount",
                ..
            }
        ));
    }
}
