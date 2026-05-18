//! P0 常数可访问性测试。

use crate::qi_physics::constants::{
    RESONANCE_LOCK_INTEGRITY_DRAIN, RESONANCE_LOCK_RHO_OVERRIDE,
    RESONANCE_RETREAT_INTEGRITY_PENALTY, SCAR_CIRCUIT_INTEGRITY_MAX, SCAR_CIRCUIT_INTEGRITY_MIN,
};

use crate::combat::baomai_v4::constants::{
    CRACK_READING_DEEP_WINDOW_TICKS, CRACK_READING_DISPLAY_TICKS, CRACK_READING_RATE_CAP,
    CRACK_READING_RATE_PER_OVERLOAD, IRON_COCOON_THRESHOLDS, RESONANCE_LOCK_DURATION_TICKS,
    RESONANCE_LOCK_RANGE, RESONANCE_METER_HIT_BASE, RESONANCE_SCAR_THRESHOLD,
    SCAR_CIRCUIT_CHECK_INTERVAL_TICKS, SCAR_CIRCUIT_MIN_OVERLOADS, VOLUNTARY_SEVER_CAST_TICKS,
    VOLUNTARY_SEVER_COMBAT_COOLDOWN_TICKS, VOLUNTARY_SEVER_MAX_INTEGRITY,
    VOLUNTARY_SEVER_MIN_MASTERY, VOLUNTARY_SEVER_MIN_OVERLOADS,
};

#[test]
fn qi_physics_constants_exist() {
    // Verify all new qi_physics constants are accessible and have expected values.
    assert!(
        (SCAR_CIRCUIT_INTEGRITY_MIN - 0.50).abs() < 1e-6,
        "SCAR_CIRCUIT_INTEGRITY_MIN should be 0.50, got {}",
        SCAR_CIRCUIT_INTEGRITY_MIN
    );
    assert!(
        (SCAR_CIRCUIT_INTEGRITY_MAX - 0.85).abs() < 1e-6,
        "SCAR_CIRCUIT_INTEGRITY_MAX should be 0.85, got {}",
        SCAR_CIRCUIT_INTEGRITY_MAX
    );
    assert!(
        (RESONANCE_LOCK_RHO_OVERRIDE - 0.0).abs() < 1e-6,
        "RESONANCE_LOCK_RHO_OVERRIDE should be 0.0"
    );
    assert!(
        (RESONANCE_LOCK_INTEGRITY_DRAIN - 0.005).abs() < 1e-6,
        "RESONANCE_LOCK_INTEGRITY_DRAIN should be 0.005"
    );
    assert!(
        (RESONANCE_RETREAT_INTEGRITY_PENALTY - 0.08).abs() < 1e-6,
        "RESONANCE_RETREAT_INTEGRITY_PENALTY should be 0.08"
    );
}

#[test]
fn gameplay_constants_exist() {
    // Verify all gameplay constants are accessible and have expected values.
    assert_eq!(IRON_COCOON_THRESHOLDS, [50, 120, 250, 500]);
    assert!((RESONANCE_LOCK_RANGE - 2.0).abs() < 1e-6);
    assert!((RESONANCE_METER_HIT_BASE - 0.12).abs() < 1e-6);
    assert_eq!(RESONANCE_LOCK_DURATION_TICKS, 60);
    assert_eq!(RESONANCE_SCAR_THRESHOLD, 40);
    assert!((CRACK_READING_RATE_PER_OVERLOAD - 0.004).abs() < 1e-6);
    assert!((CRACK_READING_RATE_CAP - 0.50).abs() < 1e-6);
    assert_eq!(CRACK_READING_DISPLAY_TICKS, 60);
    assert_eq!(CRACK_READING_DEEP_WINDOW_TICKS, 60);
    assert_eq!(SCAR_CIRCUIT_MIN_OVERLOADS, 5);
    assert_eq!(SCAR_CIRCUIT_CHECK_INTERVAL_TICKS, 40);
    assert_eq!(VOLUNTARY_SEVER_MIN_OVERLOADS, 80);
    assert!((VOLUNTARY_SEVER_MIN_MASTERY - 60.0).abs() < 1e-6);
    assert!((VOLUNTARY_SEVER_MAX_INTEGRITY - 0.70).abs() < 1e-6);
    assert_eq!(VOLUNTARY_SEVER_CAST_TICKS, 100);
    assert_eq!(VOLUNTARY_SEVER_COMBAT_COOLDOWN_TICKS, 60);
}

#[test]
fn iron_cocoon_thresholds_monotonically_increasing() {
    for i in 1..IRON_COCOON_THRESHOLDS.len() {
        assert!(
            IRON_COCOON_THRESHOLDS[i] > IRON_COCOON_THRESHOLDS[i - 1],
            "Iron cocoon thresholds must be strictly increasing: {} should be > {}",
            IRON_COCOON_THRESHOLDS[i],
            IRON_COCOON_THRESHOLDS[i - 1]
        );
    }
}

#[test]
fn integrity_range_valid() {
    // Use variables to avoid clippy::assertions_on_constants (compile-time constant folding).
    let min = SCAR_CIRCUIT_INTEGRITY_MIN;
    let max = SCAR_CIRCUIT_INTEGRITY_MAX;
    assert!(
        min < max,
        "INTEGRITY_MIN ({}) must be < INTEGRITY_MAX ({})",
        min,
        max
    );
    assert!(
        min >= 0.0 && max <= 1.0,
        "Integrity range must be within [0.0, 1.0], got [{}, {}]",
        min,
        max
    );
}
