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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AoeGroundWaveOutcome {
    pub qi_spent: f64,
    pub radius_blocks: f32,
    pub shock_damage: f32,
    pub stagger_ticks: u64,
    pub knockback_blocks: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BloodBurnConversionOutcome {
    pub hp_burned: f32,
    pub qi_multiplier: f32,
    pub duration_ticks: u64,
    pub ends_in_near_death: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BodyTranscendenceOutcome {
    pub qi_max_before: f64,
    pub qi_max_after: f64,
    pub qi_max_lost: f64,
    pub flow_rate_multiplier: f64,
    pub duration_ticks: u64,
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

/// 爆脉 v3 撼山：把真元沿地表传导为短半径震波。
pub fn aoe_ground_wave(
    qi_spent: f64,
    radius_blocks: f32,
    shock_damage: f32,
) -> Result<AoeGroundWaveOutcome, QiPhysicsError> {
    let qi_spent = finite_non_negative(qi_spent, "ground_wave.qi_spent")?;
    let radius = finite_non_negative(f64::from(radius_blocks), "ground_wave.radius")? as f32;
    let damage = finite_non_negative(f64::from(shock_damage), "ground_wave.damage")? as f32;
    Ok(AoeGroundWaveOutcome {
        qi_spent,
        radius_blocks: radius,
        shock_damage: damage,
        stagger_ticks: 10,
        knockback_blocks: 0.75,
    })
}

/// 爆脉 v3 焚血：把当前生命直接换成短时真元投入倍率。
pub fn blood_burn_conversion(
    hp_current: f32,
    hp_burn: f32,
    qi_multiplier: f32,
    duration_ticks: u64,
) -> Result<BloodBurnConversionOutcome, QiPhysicsError> {
    let hp_current = finite_non_negative(f64::from(hp_current), "blood_burn.hp_current")? as f32;
    let hp_burned = finite_non_negative(f64::from(hp_burn), "blood_burn.hp_burn")? as f32;
    if hp_burned > hp_current {
        return Err(QiPhysicsError::InvalidAmount {
            field: "blood_burn.hp_burn",
            value: f64::from(hp_burned),
        });
    }
    let multiplier =
        finite_non_negative(f64::from(qi_multiplier), "blood_burn.qi_multiplier")? as f32;
    Ok(BloodBurnConversionOutcome {
        hp_burned,
        qi_multiplier: multiplier.max(1.0),
        duration_ticks,
        ends_in_near_death: hp_current - hp_burned <= hp_current.max(1.0) * 0.10,
    })
}

/// 爆脉 v3 散功：永久烧 qi_max，换取短时 flow_rate 放大窗口。
pub fn body_transcendence(
    qi_max: f64,
    loss_ratio: f64,
    flow_rate_multiplier: f64,
    duration_ticks: u64,
) -> Result<BodyTranscendenceOutcome, QiPhysicsError> {
    let qi_max = finite_non_negative(qi_max, "body_transcendence.qi_max")?;
    let loss_ratio =
        finite_non_negative(loss_ratio, "body_transcendence.loss_ratio")?.clamp(0.0, 1.0);
    let multiplier = finite_non_negative(
        flow_rate_multiplier,
        "body_transcendence.flow_rate_multiplier",
    )?
    .max(1.0);
    let qi_max_lost = qi_max * loss_ratio;
    Ok(BodyTranscendenceOutcome {
        qi_max_before: qi_max,
        qi_max_after: (qi_max - qi_max_lost).max(0.0),
        qi_max_lost,
        flow_rate_multiplier: multiplier,
        duration_ticks,
    })
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

    #[test]
    fn aoe_ground_wave_preserves_input_damage_and_control_window() {
        let out = aoe_ground_wave(35.0, 6.0, 90.0).unwrap();
        assert_eq!(out.qi_spent, 35.0);
        assert_eq!(out.radius_blocks, 6.0);
        assert_eq!(out.shock_damage, 90.0);
        assert_eq!(out.stagger_ticks, 10);
    }

    #[test]
    fn blood_burn_conversion_marks_near_death_boundary() {
        let out = blood_burn_conversion(100.0, 91.0, 2.5, 500).unwrap();
        assert!(out.ends_in_near_death);
        assert_eq!(out.qi_multiplier, 2.5);
    }

    #[test]
    fn blood_burn_conversion_rejects_more_hp_than_available() {
        let err = blood_burn_conversion(20.0, 21.0, 2.0, 500).unwrap_err();
        assert!(matches!(
            err,
            QiPhysicsError::InvalidAmount {
                field: "blood_burn.hp_burn",
                ..
            }
        ));
    }

    #[test]
    fn body_transcendence_burns_qi_max_without_immunity_field() {
        let out = body_transcendence(10_700.0, 0.5, 10.0, 100).unwrap();
        assert_eq!(out.qi_max_before, 10_700.0);
        assert_eq!(out.qi_max_after, 5_350.0);
        assert_eq!(out.flow_rate_multiplier, 10.0);
    }
}
