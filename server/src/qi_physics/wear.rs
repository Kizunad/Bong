use super::constants::{QI_TARGETED_ITEM_WEAR_MAX_FRACTION, QI_TARGETED_ITEM_WEAR_MIN_FRACTION};

pub fn qi_targeted_item_wear_fraction(instance_id: u64, actor_id: &str, karma_weight: f32) -> f64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    hash = fnv1a64(hash, &instance_id.to_le_bytes());
    hash = fnv1a64(hash, actor_id.as_bytes());
    hash = fnv1a64(hash, &karma_weight.to_bits().to_le_bytes());
    let bucket = hash % 10_000;
    let unit = bucket as f64 / 9_999.0;
    QI_TARGETED_ITEM_WEAR_MIN_FRACTION
        + (QI_TARGETED_ITEM_WEAR_MAX_FRACTION - QI_TARGETED_ITEM_WEAR_MIN_FRACTION) * unit
}

fn fnv1a64(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash
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
