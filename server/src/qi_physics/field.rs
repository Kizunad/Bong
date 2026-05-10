use super::collision::reverse_clamp;
use super::constants::DUGU_DIRTY_QI_ZONE_RETURN_RATIO;
use super::{finite_non_negative, QiPhysicsError};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EchoFractalOutcome {
    pub local_qi_density: f64,
    pub threshold: f64,
    pub echo_count: u32,
    pub damage_per_echo: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DuguReverseBurstOutcome {
    pub mark_count: u32,
    pub total_taint_intensity: f64,
    pub burst_damage: f64,
    pub returned_zone_qi: f64,
}

pub fn density_echo(
    local_qi_density: f64,
    base_threshold: f64,
    base_damage: f64,
    mastery: u8,
) -> Result<EchoFractalOutcome, QiPhysicsError> {
    let density = finite_non_negative(local_qi_density, "echo.local_qi_density")?;
    let threshold = finite_non_negative(base_threshold, "echo.base_threshold")?;
    let damage = finite_non_negative(base_damage, "echo.base_damage")?;
    let mastery_ratio = f64::from(mastery.min(100)) / 100.0;
    let effective_threshold = (threshold - (threshold - 0.1).max(0.0) * mastery_ratio).max(0.01);
    let echo_count = (density / effective_threshold).floor().max(1.0) as u32;
    let total_damage = damage * (2.0 + 0.5 * mastery_ratio);

    Ok(EchoFractalOutcome {
        local_qi_density: density,
        threshold: effective_threshold,
        echo_count,
        damage_per_echo: total_damage / f64::from(echo_count),
    })
}

/// 截脉多点接触的音论分散算子。
///
/// `points` 当前只作为调用侧的物理语义输入保留：v2 数值表已经把多点分散后的
/// `k_drain` 降到低于单点弹反，因此这里不再次按点数折减，避免重复惩罚。
pub fn multi_point_dispersion(
    incoming_qi: f64,
    k_drain: f64,
    style_weight: f64,
    beta: f64,
    _points: u8,
) -> f64 {
    reverse_clamp(incoming_qi, k_drain, style_weight, beta)
}

/// 主动 SEVERED 经脉换反震效率的破例算子。
pub fn sever_meridian(normal_clamp: f64, amplification_multiplier: f64) -> f64 {
    if !normal_clamp.is_finite() || !amplification_multiplier.is_finite() {
        return 0.0;
    }
    (normal_clamp.max(0.0) * amplification_multiplier.max(0.0)).max(0.0)
}

/// 毒蛊倒蚀只负责把已种入的永久标记一次性清算；标记扫描由 combat 层完成。
pub fn reverse_burst_all_marks<I>(mark_intensities: I) -> DuguReverseBurstOutcome
where
    I: IntoIterator<Item = f64>,
{
    let mut mark_count = 0_u32;
    let mut total_taint_intensity = 0.0;
    for intensity in mark_intensities {
        if intensity.is_finite() && intensity > 0.0 {
            mark_count = mark_count.saturating_add(1);
            total_taint_intensity += intensity;
        }
    }
    let burst_damage = total_taint_intensity * 12.0;
    DuguReverseBurstOutcome {
        mark_count,
        total_taint_intensity,
        burst_damage,
        returned_zone_qi: total_taint_intensity * DUGU_DIRTY_QI_ZONE_RETURN_RATIO,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn void_density_nine_at_threshold_point_three_yields_thirty_echoes() {
        let out = density_echo(9.0, 0.3, 60.0, 0).unwrap();
        assert_eq!(out.echo_count, 30);
        assert_eq!(out.threshold, 0.3);
    }

    #[test]
    fn mastery_lowers_threshold_to_point_one() {
        let out = density_echo(9.0, 0.3, 60.0, 100).unwrap();
        assert_eq!(out.echo_count, 90);
        assert!((out.threshold - 0.1).abs() < 1e-9);
    }

    #[test]
    fn multipoint_dispersion_reuses_reverse_clamp_without_second_point_penalty() {
        assert!((multi_point_dispersion(50.0, 0.2, 0.5, 0.6, 5) - 3.0).abs() < 1e-6);
    }

    #[test]
    fn sever_meridian_triples_normal_half_clamp_for_void_zhenmai() {
        assert_eq!(sever_meridian(0.5, 3.0), 1.5);
    }

    #[test]
    fn reverse_burst_all_marks_returns_dirty_qi_to_zone_budget() {
        let out = reverse_burst_all_marks([2.0, f64::NAN, -1.0, 3.0]);
        assert_eq!(out.mark_count, 2);
        assert_eq!(out.total_taint_intensity, 5.0);
        assert_eq!(out.burst_damage, 60.0);
        assert!((out.returned_zone_qi - 4.95).abs() < 1e-9);
    }
}
