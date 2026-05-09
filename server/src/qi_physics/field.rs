use super::{finite_non_negative, QiPhysicsError};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EchoFractalOutcome {
    pub local_qi_density: f64,
    pub threshold: f64,
    pub echo_count: u32,
    pub damage_per_echo: f64,
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
}
