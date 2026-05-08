use super::constants::{QI_TARGETED_ITEM_WEAR_MAX_FRACTION, QI_TARGETED_ITEM_WEAR_MIN_FRACTION};

pub fn qi_targeted_item_wear_fraction(instance_id: u64, actor_id: &str, karma_weight: f32) -> f64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    instance_id.hash(&mut hasher);
    actor_id.hash(&mut hasher);
    karma_weight.to_bits().hash(&mut hasher);
    let bucket = hasher.finish() % 10_000;
    let unit = bucket as f64 / 9_999.0;
    QI_TARGETED_ITEM_WEAR_MIN_FRACTION
        + (QI_TARGETED_ITEM_WEAR_MAX_FRACTION - QI_TARGETED_ITEM_WEAR_MIN_FRACTION) * unit
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targeted_item_wear_is_deterministic_and_bounded() {
        let first = qi_targeted_item_wear_fraction(42, "offline:Azure", 0.7);
        let second = qi_targeted_item_wear_fraction(42, "offline:Azure", 0.7);
        assert_eq!(first, second);
        assert!(
            (QI_TARGETED_ITEM_WEAR_MIN_FRACTION..=QI_TARGETED_ITEM_WEAR_MAX_FRACTION)
                .contains(&first)
        );
    }
}
