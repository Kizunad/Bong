pub fn armor_weight_to_speed(total_weight: f64) -> f32 {
    let factor = if total_weight < 5.0 {
        1.0
    } else if total_weight <= 15.0 {
        1.0 - (total_weight - 5.0) * 0.015
    } else {
        (0.85 - (total_weight - 15.0) * 0.01).max(0.65)
    };
    factor as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 1e-5,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn armor_weight_curve_matches_plan_table() {
        assert_close(armor_weight_to_speed(0.0), 1.0);
        assert_close(armor_weight_to_speed(3.0), 1.0);
        assert_close(armor_weight_to_speed(10.0), 0.925);
        assert_close(armor_weight_to_speed(20.0), 0.8);
        assert_close(armor_weight_to_speed(999.0), 0.65);
    }
}
