//! 拟态灰烬蛛：死域边缘伏击权重。

pub const ASH_SPIDER_LAIR_FLORA_VARIANT_ID: u8 = 4;
pub const ASH_SPIDER_LAIR_WEIGHT_MULTIPLIER: f64 = 8.0;
pub const ASH_SPIDER_EDGE_WIDTH_BLOCKS: f64 = 50.0;

pub fn ash_spider_spawn_weight(
    distance_to_dead_zone_edge_blocks: f64,
    flora_variant_id: u8,
    base_weight: f64,
) -> f64 {
    if !(0.0..=ASH_SPIDER_EDGE_WIDTH_BLOCKS).contains(&distance_to_dead_zone_edge_blocks)
        || base_weight <= 0.0
    {
        return 0.0;
    }
    if flora_variant_id == ASH_SPIDER_LAIR_FLORA_VARIANT_ID {
        base_weight * ASH_SPIDER_LAIR_WEIGHT_MULTIPLIER
    } else {
        base_weight
    }
}

pub fn ash_spider_ambush_count_per_100(
    distance_to_dead_zone_edge_blocks: f64,
    flora_variant_id: u8,
) -> u32 {
    let weight = ash_spider_spawn_weight(distance_to_dead_zone_edge_blocks, flora_variant_id, 1.0);
    if weight <= 0.0 {
        return 0;
    }
    let edge_factor =
        1.0 - (distance_to_dead_zone_edge_blocks / ASH_SPIDER_EDGE_WIDTH_BLOCKS).clamp(0.0, 1.0);
    (3.0 + edge_factor * weight).round().clamp(3.0, 15.0) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lair_variant_multiplies_spawn_weight_by_eight() {
        assert_eq!(
            ash_spider_spawn_weight(10.0, ASH_SPIDER_LAIR_FLORA_VARIANT_ID, 2.0),
            16.0
        );
        assert_eq!(ash_spider_spawn_weight(10.0, 1, 2.0), 2.0);
    }

    #[test]
    fn ambush_rate_stays_inside_statistical_acceptance_band() {
        let near_lair = ash_spider_ambush_count_per_100(0.0, ASH_SPIDER_LAIR_FLORA_VARIANT_ID);
        let far_edge = ash_spider_ambush_count_per_100(49.0, ASH_SPIDER_LAIR_FLORA_VARIANT_ID);

        assert!((3..=15).contains(&near_lair));
        assert!((3..=15).contains(&far_edge));
        assert!(near_lair > far_edge);
    }

    #[test]
    fn outside_edge_ring_does_not_spawn() {
        assert_eq!(
            ash_spider_spawn_weight(51.0, ASH_SPIDER_LAIR_FLORA_VARIANT_ID, 1.0),
            0.0
        );
        assert_eq!(
            ash_spider_ambush_count_per_100(51.0, ASH_SPIDER_LAIR_FLORA_VARIANT_ID),
            0
        );
    }
}
