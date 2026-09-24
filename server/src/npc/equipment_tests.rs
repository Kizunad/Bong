use super::*;

// === P0.1 quality_multiplier all tiers ===

#[test]
fn quality_multiplier_all_tiers() {
    assert!((quality_multiplier(0) - 1.00).abs() < 1e-5);
    assert!((quality_multiplier(1) - 1.15).abs() < 1e-5);
    assert!((quality_multiplier(2) - 1.35).abs() < 1e-5);
    assert!((quality_multiplier(3) - 1.60).abs() < 1e-5);
    assert!((quality_multiplier(255) - 1.60).abs() < 1e-5);
}
