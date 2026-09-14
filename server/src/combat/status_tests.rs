#![allow(dead_code, unused_imports)]

use super::*;

#[test]
fn health_regen_boost_multiplier_caps_runaway_stack() {
    let status_effects = StatusEffects {
        active: vec![
            crate::combat::components::ActiveStatusEffect {
                kind: StatusEffectKind::HealthRegenBoost,
                magnitude: 9.0,
                remaining_ticks: 20,
                source_pill: None,
            },
            crate::combat::components::ActiveStatusEffect {
                kind: StatusEffectKind::HealthRegenBoost,
                magnitude: 9.0,
                remaining_ticks: 20,
                source_pill: None,
            },
        ],
    };

    assert_eq!(
        health_regen_boost_multiplier(&status_effects),
        MAX_HEALTH_REGEN_BOOST_MULTIPLIER
    );
}


