//! plan-terrain-jiuzong-ruin-v1 §7 P2 — zone-scoped 灵气紊乱场。

pub const QI_TURBULENCE_PERIOD_TICKS: u64 = 90 * 20;
pub const JIU_ZONG_BASE_QI: f64 = 0.40;
pub const JIU_ZONG_TURBULENCE_AMPLITUDE: f64 = 0.30;

#[derive(Debug, Clone, PartialEq)]
pub struct QiTurbulenceField {
    pub zone_id: String,
    pub base_qi: f64,
    pub amplitude: f64,
    pub period_ticks: u64,
    pub phase_seed: u64,
}

impl QiTurbulenceField {
    pub fn jiu_zong(zone_id: impl Into<String>, phase_seed: u64) -> Self {
        Self {
            zone_id: zone_id.into(),
            base_qi: JIU_ZONG_BASE_QI,
            amplitude: JIU_ZONG_TURBULENCE_AMPLITUDE,
            period_ticks: QI_TURBULENCE_PERIOD_TICKS,
            phase_seed,
        }
    }

    pub fn sample_qi(&self, current_tick: u64) -> f64 {
        let period = self.period_ticks.max(1);
        let phase = ((current_tick + self.phase_seed) % period) as f64 / period as f64;
        let wave = (phase * std::f64::consts::TAU).sin();
        round3((self.base_qi + wave * self.amplitude).clamp(0.0, 1.0))
    }

    pub fn is_breakthrough_stable(
        &self,
        start_tick: u64,
        duration_ticks: u64,
        threshold: f64,
    ) -> bool {
        if duration_ticks == 0 {
            return self.sample_qi(start_tick) >= threshold;
        }
        let steps = 6;
        (0..=steps).all(|idx| {
            let tick = start_tick + duration_ticks.saturating_mul(idx) / steps;
            self.sample_qi(tick) >= threshold
        })
    }
}

fn round3(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qi_turbulence_field_spans_plan_pinned_range() {
        let field = QiTurbulenceField::jiu_zong("jiuzong_bloodstream_ruin", 0);

        assert_eq!(field.sample_qi(0), 0.4);
        assert_eq!(field.sample_qi(QI_TURBULENCE_PERIOD_TICKS / 4), 0.7);
        assert_eq!(field.sample_qi(QI_TURBULENCE_PERIOD_TICKS / 2), 0.4);
        assert_eq!(field.sample_qi(QI_TURBULENCE_PERIOD_TICKS * 3 / 4), 0.1);
        assert_eq!(field.sample_qi(QI_TURBULENCE_PERIOD_TICKS), 0.4);
    }

    #[test]
    fn breakthrough_stability_fails_across_full_turbulence_period() {
        let field = QiTurbulenceField::jiu_zong("jiuzong_beiling_ruin", 0);

        assert!(!field.is_breakthrough_stable(0, 3 * 60 * 20, 0.5));
        assert!(field.is_breakthrough_stable(QI_TURBULENCE_PERIOD_TICKS / 4, 0, 0.5));
    }
}
