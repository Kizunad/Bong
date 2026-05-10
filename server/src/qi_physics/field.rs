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
pub struct InverseDiffusionOutcome {
    pub stored_qi: f64,
    pub efficiency: f64,
    pub retained_qi: f64,
    pub returned_to_zone_qi: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DensityAmplifierOutcome {
    pub base_density: f64,
    pub multiplier: f64,
    pub amplified_density: f64,
    pub triggers_tiandao_gaze: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TiandaoSignalDistortionOutcome {
    pub self_visible_weight: f64,
    pub target_visible_weight: f64,
    pub visible_qi_current: f64,
    pub visible_qi_max: f64,
    pub conservation_delta: f64,
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

/// 阵法真元逆逸散：mastery 越高，阵眼封存真元越不容易回漏到 zone。
pub fn inverse_diffusion(
    stored_qi: f64,
    mastery_efficiency: f64,
    elapsed_ratio: f64,
) -> Result<InverseDiffusionOutcome, QiPhysicsError> {
    let stored_qi = finite_non_negative(stored_qi, "zhenfa.inverse_diffusion.stored_qi")?;
    let efficiency = finite_non_negative(
        mastery_efficiency,
        "zhenfa.inverse_diffusion.mastery_efficiency",
    )?
    .clamp(0.0, 1.0);
    let elapsed_ratio =
        finite_non_negative(elapsed_ratio, "zhenfa.inverse_diffusion.elapsed_ratio")?
            .clamp(0.0, 1.0);
    let leak_ratio = elapsed_ratio * (1.0 - efficiency);
    let returned_to_zone_qi = stored_qi * leak_ratio;
    Ok(InverseDiffusionOutcome {
        stored_qi,
        efficiency,
        retained_qi: stored_qi - returned_to_zone_qi,
        returned_to_zone_qi,
    })
}

/// 聚灵阵密度放大：只改环境密度读数，不凭空给 world qi ledger 增发真元。
pub fn density_amplifier(
    base_density: f64,
    multiplier: f64,
    gaze_threshold: f64,
) -> Result<DensityAmplifierOutcome, QiPhysicsError> {
    let base_density = finite_non_negative(base_density, "zhenfa.density.base_density")?;
    let multiplier = finite_non_negative(multiplier, "zhenfa.density.multiplier")?;
    let gaze_threshold = finite_non_negative(gaze_threshold, "zhenfa.density.gaze_threshold")?;
    let amplified_density = base_density * multiplier.max(1.0);
    Ok(DensityAmplifierOutcome {
        base_density,
        multiplier: multiplier.max(1.0),
        amplified_density,
        triggers_tiandao_gaze: amplified_density > gaze_threshold,
    })
}

/// 欺天阵只扭曲天道 agent 看到的 zone 快照字段；真实账本不改。
#[allow(clippy::too_many_arguments)]
pub fn tiandao_signal_distort(
    self_weight: f64,
    target_weight: f64,
    qi_current: f64,
    qi_max: f64,
    self_multiplier: f64,
    target_multiplier: f64,
    qi_visible_ratio: f64,
) -> Result<TiandaoSignalDistortionOutcome, QiPhysicsError> {
    let self_weight = finite_non_negative(self_weight, "zhenfa.deceive.self_weight")?;
    let target_weight = finite_non_negative(target_weight, "zhenfa.deceive.target_weight")?;
    let qi_current = finite_non_negative(qi_current, "zhenfa.deceive.qi_current")?;
    let qi_max = finite_non_negative(qi_max, "zhenfa.deceive.qi_max")?;
    if qi_current > qi_max {
        return Err(QiPhysicsError::InvalidAmount {
            field: "zhenfa.deceive.qi_current",
            value: qi_current,
        });
    }
    let self_multiplier = finite_non_negative(self_multiplier, "zhenfa.deceive.self_multiplier")?;
    let target_multiplier =
        finite_non_negative(target_multiplier, "zhenfa.deceive.target_multiplier")?;
    let qi_visible_ratio =
        finite_non_negative(qi_visible_ratio, "zhenfa.deceive.qi_visible_ratio")?.clamp(0.0, 1.0);
    let visible_qi_current = qi_current * qi_visible_ratio;
    let visible_qi_max = qi_max * qi_visible_ratio;
    Ok(TiandaoSignalDistortionOutcome {
        self_visible_weight: self_weight * self_multiplier,
        target_visible_weight: target_weight * target_multiplier,
        visible_qi_current,
        visible_qi_max,
        conservation_delta: (qi_current + qi_max) - (visible_qi_current + visible_qi_max),
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShedToCarrierOutcome {
    pub damage_absorbed: f64,
    pub damage_overflow: f64,
    pub contam_absorbed: f64,
    pub contam_overflow: f64,
}

/// 替尸影论承伤算子：伪皮先吃本次伤害和同源污染，超出单层上限才回流真身。
pub fn shed_to_carrier(
    carrier_remaining_capacity: f64,
    incoming_damage: f64,
    incoming_contam_percent: f64,
) -> ShedToCarrierOutcome {
    let capacity = if carrier_remaining_capacity.is_finite() {
        carrier_remaining_capacity.max(0.0)
    } else {
        0.0
    };
    let damage = if incoming_damage.is_finite() {
        incoming_damage.max(0.0)
    } else {
        0.0
    };
    let contam = if incoming_contam_percent.is_finite() {
        incoming_contam_percent.max(0.0)
    } else {
        0.0
    };
    let damage_absorbed = damage.min(capacity);
    let damage_overflow = (damage - damage_absorbed).max(0.0);
    let contam_capacity = capacity.min(100.0);
    let contam_absorbed = contam.min(contam_capacity);
    let contam_overflow = (contam - contam_absorbed).max(0.0);

    ShedToCarrierOutcome {
        damage_absorbed,
        damage_overflow,
        contam_absorbed,
        contam_overflow,
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
    fn inverse_diffusion_retains_more_qi_at_high_mastery() {
        let low = inverse_diffusion(100.0, 0.2, 0.5).unwrap();
        let high = inverse_diffusion(100.0, 0.8, 0.5).unwrap();
        assert!(high.retained_qi > low.retained_qi);
        assert!((high.retained_qi + high.returned_to_zone_qi - 100.0).abs() < 1e-9);
    }

    #[test]
    fn density_amplifier_marks_tiandao_gaze_threshold() {
        let out = density_amplifier(4.1, 1.5, 6.0).unwrap();
        assert!(out.triggers_tiandao_gaze);
        assert!((out.amplified_density - 6.15).abs() < 1e-9);
    }

    #[test]
    fn tiandao_signal_distort_lowers_self_and_boosts_target_snapshot_only() {
        let out = tiandao_signal_distort(10.0, 10.0, 80.0, 100.0, 0.5, 1.5, 0.25).unwrap();
        assert_eq!(out.self_visible_weight, 5.0);
        assert_eq!(out.target_visible_weight, 15.0);
        assert_eq!(out.visible_qi_current, 20.0);
        assert_eq!(out.visible_qi_max, 25.0);
        assert_eq!(out.conservation_delta, 135.0);
    }

    #[test]
    fn tiandao_signal_distort_rejects_current_above_max() {
        assert!(matches!(
            tiandao_signal_distort(10.0, 10.0, 120.0, 100.0, 0.5, 1.5, 0.25),
            Err(QiPhysicsError::InvalidAmount {
                field: "zhenfa.deceive.qi_current",
                value: 120.0,
            })
        ));
    }

    #[test]
    fn reverse_burst_all_marks_returns_dirty_qi_to_zone_budget() {
        let out = reverse_burst_all_marks([2.0, f64::NAN, -1.0, 3.0]);
        assert_eq!(out.mark_count, 2);
        assert_eq!(out.total_taint_intensity, 5.0);
        assert_eq!(out.burst_damage, 60.0);
        assert!((out.returned_zone_qi - 4.95).abs() < 1e-9);
    }

    #[test]
    fn shed_to_carrier_absorbs_until_capacity_then_overflows() {
        let out = shed_to_carrier(50.0, 80.0, 12.0);
        assert_eq!(out.damage_absorbed, 50.0);
        assert_eq!(out.damage_overflow, 30.0);
        assert_eq!(out.contam_absorbed, 12.0);
        assert_eq!(out.contam_overflow, 0.0);
    }

    #[test]
    fn shed_to_carrier_sanitizes_non_finite_input() {
        let out = shed_to_carrier(f64::NAN, f64::INFINITY, -1.0);
        assert_eq!(out.damage_absorbed, 0.0);
        assert_eq!(out.damage_overflow, 0.0);
        assert_eq!(out.contam_absorbed, 0.0);
        assert_eq!(out.contam_overflow, 0.0);
    }
}
