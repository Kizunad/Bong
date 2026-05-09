use crate::cultivation::components::{Contamination, Realm};
use crate::qi_physics::constants::{
    QI_DRAIN_CLAMP, VORTEX_ABSORPTION_RATIO_BASE, VORTEX_SWIRL_RATIO_BASE,
};

#[cfg(test)]
use super::events::WoliuSkillId;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StirInput {
    pub total_drained: f64,
    pub realm: Realm,
    pub contamination_ratio: f64,
    pub meridian_flow_capacity: f64,
    pub dt_seconds: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StirOutcome {
    pub total_drained: f64,
    pub absorbed_raw: f64,
    pub actual_absorbed: f64,
    pub rotational_swirl: f64,
    pub overflow: f64,
    pub contamination_gain: f64,
}

impl StirOutcome {
    #[cfg(test)]
    pub fn total_output(self) -> f64 {
        self.actual_absorbed + self.rotational_swirl + self.overflow
    }
}

pub fn realm_absorption_rate(realm: Realm) -> f64 {
    match realm {
        Realm::Awaken => 0.001,
        Realm::Induce => 0.002,
        Realm::Condense => 0.004,
        Realm::Solidify => 0.006,
        Realm::Spirit => 0.008,
        Realm::Void => 0.010,
    }
}

pub fn contamination_ratio(contamination: Option<&Contamination>, qi_max: f64) -> f64 {
    let max = if qi_max.is_finite() && qi_max > 0.0 {
        qi_max
    } else {
        1.0
    };
    contamination
        .map(|contamination| {
            contamination
                .entries
                .iter()
                .map(|entry| entry.amount.max(0.0))
                .sum::<f64>()
                / max
        })
        .unwrap_or(0.0)
        .clamp(0.0, 1.0)
}

pub fn stir_99_1(input: StirInput) -> StirOutcome {
    let total_drained = sanitize_non_negative(input.total_drained);
    let contamination = sanitize_non_negative(input.contamination_ratio).clamp(0.0, 1.0);
    let dt = sanitize_non_negative(input.dt_seconds);
    let cap = sanitize_non_negative(input.meridian_flow_capacity) * (1.0 - contamination) * dt;
    let absorption_rate = realm_absorption_rate(input.realm) * (1.0 - contamination);
    let absorbed_raw = (total_drained * absorption_rate)
        .min(total_drained * QI_DRAIN_CLAMP)
        .max(0.0);
    let actual_absorbed = absorbed_raw.min(cap);
    let overflow = (absorbed_raw - actual_absorbed).max(0.0);
    let rotational_swirl = (total_drained * VORTEX_SWIRL_RATIO_BASE
        + total_drained * VORTEX_ABSORPTION_RATIO_BASE
        - absorbed_raw)
        .max(0.0);
    let contamination_gain = if overflow > 0.0 {
        overflow * 0.1
    } else {
        actual_absorbed * 0.01
    };
    StirOutcome {
        total_drained,
        absorbed_raw,
        actual_absorbed,
        rotational_swirl,
        overflow,
        contamination_gain,
    }
}

pub fn turbulence_decay_step(
    remaining_swirl_qi: f64,
    decay_rate_per_second: f64,
    dt_seconds: f64,
) -> (f64, f64) {
    let remaining = sanitize_non_negative(remaining_swirl_qi);
    let rate = sanitize_non_negative(decay_rate_per_second).clamp(0.0, 1.0);
    let dt = sanitize_non_negative(dt_seconds);
    if remaining <= f64::EPSILON || rate <= 0.0 || dt <= 0.0 {
        return (0.0, remaining);
    }
    let decayed = (remaining * (1.0 - (-rate * dt).exp())).min(remaining);
    (decayed, remaining - decayed)
}

#[cfg(test)]
pub fn lethal_and_influence_radius(skill: WoliuSkillId, realm: Realm) -> (f32, f32) {
    let spec = super::skills::skill_spec(skill, realm);
    (spec.lethal_radius, spec.influence_radius)
}

pub fn pull_displacement_blocks(caster_qi: f64, target_qi: f64, pull_force: f64) -> f32 {
    if !caster_qi.is_finite() || caster_qi <= 0.0 || !pull_force.is_finite() || pull_force <= 0.0 {
        return 0.0;
    }
    let denom = if target_qi.is_finite() && target_qi > 0.0 {
        target_qi
    } else {
        f64::INFINITY
    };
    (caster_qi * pull_force / denom).clamp(0.0, 128.0) as f32
}

fn sanitize_non_negative(value: f64) -> f64 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}
