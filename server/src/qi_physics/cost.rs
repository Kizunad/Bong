//! 真元成本算子。
//!
//! 技能模块只提供玩法参数（例如成本比例），实际的真元成本公式统一在这里计算，
//! 这样边界校验与其它 qi_physics 算子保持一致。

use super::{finite_non_negative, QiPhysicsError};

/// 按当前真元计算比例成本。
///
/// `ratio` 可以大于 1.0（例如需要投入超过当前池的仪式），由调用方的真元门
/// 决定是否允许施法；本函数只负责校验输入并保证乘积可表示。
pub fn proportional_qi_cost(qi_current: f64, ratio: f64) -> Result<f64, QiPhysicsError> {
    let qi_current = finite_non_negative(qi_current, "qi_current")?;
    let ratio = finite_non_negative(ratio, "qi_cost_ratio")?;
    let cost = qi_current * ratio;
    if !cost.is_finite() {
        return Err(QiPhysicsError::InvalidAmount {
            field: "qi_cost",
            value: cost,
        });
    }
    Ok(cost)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proportional_cost_scales_current_qi() {
        assert_eq!(proportional_qi_cost(100.0, 0.4).unwrap(), 40.0);
    }

    #[test]
    fn proportional_cost_allows_zero_and_ratios_above_one() {
        assert_eq!(proportional_qi_cost(0.0, 0.4).unwrap(), 0.0);
        assert_eq!(proportional_qi_cost(100.0, 0.0).unwrap(), 0.0);
        assert_eq!(proportional_qi_cost(100.0, 1.5).unwrap(), 150.0);
    }

    #[test]
    fn proportional_cost_rejects_invalid_inputs() {
        for (qi_current, ratio, field) in [
            (-1.0, 0.4, "qi_current"),
            (100.0, -0.1, "qi_cost_ratio"),
            (f64::NAN, 0.4, "qi_current"),
            (100.0, f64::INFINITY, "qi_cost_ratio"),
        ] {
            assert!(
                matches!(
                    proportional_qi_cost(qi_current, ratio),
                    Err(QiPhysicsError::InvalidAmount { field: actual, .. }) if actual == field
                ),
                "invalid proportional cost input ({qi_current}, {ratio}) must identify {field}"
            );
        }
    }

    #[test]
    fn proportional_cost_rejects_unrepresentable_product() {
        assert!(matches!(
            proportional_qi_cost(f64::MAX, 2.0),
            Err(QiPhysicsError::InvalidAmount {
                field: "qi_cost",
                ..
            })
        ));
    }
}
