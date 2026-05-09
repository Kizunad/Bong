//! 骨币经济 IPC schema（plan-economy-v1 P2/P3）。
//!
//! 与 `agent/packages/schema/src/economy.ts` 对拍。server 每 in-game month
//! 推送一次骨币真元供给与价格指数，Tiandao 只消费聚合值，不接逐枚骨币。

use serde::{Deserialize, Serialize};

use crate::economy::EconomyPriceIndex;

use super::world_state::SeasonV1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BoneCoinTickV1 {
    pub v: u8,
    pub tick: u64,
    pub season: SeasonV1,
    pub total_spirit_qi: f64,
    pub total_face_value: f64,
    pub active_coin_count: u64,
    pub rotten_coin_count: u64,
    pub legacy_scalar_count: u64,
    pub rhythm_multiplier: f64,
    pub market_factor: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PriceSampleV1 {
    pub item_id: String,
    pub base_price: u64,
    pub final_price: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PriceIndexV1 {
    pub v: u8,
    pub tick: u64,
    pub season: SeasonV1,
    pub supply_spirit_qi: f64,
    pub demand_spirit_qi: f64,
    pub rhythm_multiplier: f64,
    pub market_factor: f64,
    pub price_multiplier: f64,
    pub sample_prices: Vec<PriceSampleV1>,
}

impl BoneCoinTickV1 {
    pub fn from_index(index: &EconomyPriceIndex) -> Self {
        Self {
            v: 1,
            tick: index.tick,
            season: index.season.into(),
            total_spirit_qi: index.total_spirit_qi,
            total_face_value: index.total_face_value,
            active_coin_count: index.active_coin_count,
            rotten_coin_count: index.rotten_coin_count,
            legacy_scalar_count: index.legacy_scalar_count,
            rhythm_multiplier: index.rhythm_multiplier,
            market_factor: index.market_factor,
        }
    }
}

impl PriceIndexV1 {
    pub fn from_index(index: &EconomyPriceIndex, sample_prices: Vec<PriceSampleV1>) -> Self {
        Self {
            v: 1,
            tick: index.tick,
            season: index.season.into(),
            supply_spirit_qi: index.total_spirit_qi,
            demand_spirit_qi: index.demand_spirit_qi,
            rhythm_multiplier: index.rhythm_multiplier,
            market_factor: index.market_factor,
            price_multiplier: index.price_multiplier,
            sample_prices,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bone_coin_tick_v1_loads_sample_from_agent_packages_schema() {
        let raw = include_str!("../../../agent/packages/schema/samples/bone-coin-tick.sample.json");
        let parsed: BoneCoinTickV1 = serde_json::from_str(raw).expect("sample 应当通过 Rust serde");
        assert_eq!(parsed.v, 1);
        assert_eq!(parsed.season, SeasonV1::SummerToWinter);
        assert_eq!(parsed.active_coin_count, 3);
        assert_eq!(parsed.rotten_coin_count, 1);
    }

    #[test]
    fn price_index_v1_loads_sample_from_agent_packages_schema() {
        let raw = include_str!("../../../agent/packages/schema/samples/price-index.sample.json");
        let parsed: PriceIndexV1 = serde_json::from_str(raw).expect("sample 应当通过 Rust serde");
        assert_eq!(parsed.v, 1);
        assert_eq!(parsed.season, SeasonV1::SummerToWinter);
        assert_eq!(parsed.sample_prices.len(), 3);
        assert_eq!(parsed.sample_prices[0].item_id, "common_good");
    }
}
