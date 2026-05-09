//! 骨币经济指数：统计世界骨币真元供给，并给 NPC 交易估价提供节律乘数。
//!
//! 本模块只做 economy plan 自己的价格指数，不重新定义真元逸散物理；骨币残量由
//! `inventory::ItemInstance.spirit_quality` 与 qi_physics/shelflife 链路维护。

use valence::prelude::{App, Query, Res, ResMut, Resource, Update};

use crate::cultivation::tick::CultivationClock;
use crate::inventory::{ItemInstance, PlayerInventory};
use crate::network::{redis_bridge::RedisOutbound, RedisBridgeResource};
use crate::qi_physics::constants::{
    QI_RHYTHM_ACTIVE, QI_RHYTHM_NEUTRAL, QI_RHYTHM_TURBULENT_RANGE,
};
use crate::schema::economy::{BoneCoinTickV1, PriceIndexV1, PriceSampleV1};
use crate::world::season::{query_season, Season, WorldSeasonState, VANILLA_DAY_TICKS};

pub const ECONOMY_TELEMETRY_INTERVAL_TICKS: u64 = 30 * VANILLA_DAY_TICKS;
const DEFAULT_DEMAND_PER_PLAYER_SPIRIT_QI: f64 = 50.0;
const MARKET_FACTOR_MIN: f64 = 0.5;
const MARKET_FACTOR_MAX: f64 = 2.0;
const MARKET_LOG_SENSITIVITY: f64 = 0.25;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BoneCoinSupply {
    pub active_coin_count: u64,
    pub rotten_coin_count: u64,
    pub legacy_scalar_count: u64,
    pub total_face_value: f64,
    pub total_spirit_qi: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EconomyPriceIndex {
    pub tick: u64,
    pub season: Season,
    pub total_spirit_qi: f64,
    pub total_face_value: f64,
    pub active_coin_count: u64,
    pub rotten_coin_count: u64,
    pub legacy_scalar_count: u64,
    pub demand_spirit_qi: f64,
    pub rhythm_multiplier: f64,
    pub market_factor: f64,
    pub price_multiplier: f64,
}

#[derive(Debug, Default)]
pub struct EconomyTelemetryState {
    last_emitted_tick: Option<u64>,
}

impl Resource for EconomyTelemetryState {}

pub fn register(app: &mut App) {
    app.insert_resource(EconomyTelemetryState::default())
        .add_systems(Update, publish_economy_telemetry_system);
}

pub fn bone_coin_face_value(template_id: &str) -> Option<f64> {
    match template_id {
        "bone_coin_5" => Some(5.0),
        "bone_coin_15" => Some(15.0),
        "bone_coin_40" => Some(40.0),
        "fengling_bone_coin" => Some(10.0),
        _ => None,
    }
}

pub fn is_rotten_bone_coin(template_id: &str) -> bool {
    template_id == "rotten_bone_coin"
}

pub fn collect_bone_coin_supply<'a>(
    inventories: impl IntoIterator<Item = &'a PlayerInventory>,
) -> BoneCoinSupply {
    let mut supply = BoneCoinSupply::default();
    for inventory in inventories {
        supply.legacy_scalar_count = supply
            .legacy_scalar_count
            .saturating_add(inventory.bone_coins);
        for item in inventory_items(inventory) {
            accumulate_item_supply(&mut supply, item);
        }
    }
    supply
}

pub fn demand_for_player_count(player_count: usize) -> f64 {
    (player_count.max(1) as f64) * DEFAULT_DEMAND_PER_PLAYER_SPIRIT_QI
}

pub fn rhythm_multiplier_for_season(season: Season, tick: u64) -> f64 {
    match season {
        Season::Summer => QI_RHYTHM_ACTIVE,
        Season::Winter => QI_RHYTHM_NEUTRAL,
        Season::SummerToWinter | Season::WinterToSummer => turbulent_rhythm_multiplier(tick),
    }
}

pub fn market_factor_for_supply(supply_spirit_qi: f64, demand_spirit_qi: f64) -> f64 {
    let ratio = supply_spirit_qi.max(1.0) / demand_spirit_qi.max(1.0);
    (1.0 + ratio.ln() * MARKET_LOG_SENSITIVITY).clamp(MARKET_FACTOR_MIN, MARKET_FACTOR_MAX)
}

pub fn price_index_for_supply(
    tick: u64,
    season: Season,
    supply: BoneCoinSupply,
    demand_spirit_qi: f64,
) -> EconomyPriceIndex {
    let rhythm_multiplier = rhythm_multiplier_for_season(season, tick);
    let market_factor = market_factor_for_supply(supply.total_spirit_qi, demand_spirit_qi);
    EconomyPriceIndex {
        tick,
        season,
        total_spirit_qi: supply.total_spirit_qi,
        total_face_value: supply.total_face_value,
        active_coin_count: supply.active_coin_count,
        rotten_coin_count: supply.rotten_coin_count,
        legacy_scalar_count: supply.legacy_scalar_count,
        demand_spirit_qi: demand_spirit_qi.max(1.0),
        rhythm_multiplier,
        market_factor,
        price_multiplier: rhythm_multiplier * market_factor,
    }
}

pub fn neutral_price_index() -> EconomyPriceIndex {
    price_index_for_supply(
        0,
        Season::Winter,
        BoneCoinSupply {
            total_spirit_qi: DEFAULT_DEMAND_PER_PLAYER_SPIRIT_QI,
            total_face_value: DEFAULT_DEMAND_PER_PLAYER_SPIRIT_QI,
            ..Default::default()
        },
        DEFAULT_DEMAND_PER_PLAYER_SPIRIT_QI,
    )
}

pub fn estimate_item_price_with_index(
    item: &ItemInstance,
    base_price: u64,
    index: &EconomyPriceIndex,
) -> u64 {
    let quality_mult = 1.0 + item.spirit_quality.clamp(0.0, 1.0) * 0.5;
    let durability_mult = if item.durability.clamp(0.0, 1.0) <= 0.2 {
        0.5
    } else {
        1.0
    };
    let per = base_price as f64 * quality_mult * durability_mult * index.price_multiplier;
    (per * item.stack_count.max(1) as f64).round().max(1.0) as u64
}

fn publish_economy_telemetry_system(
    clock: Option<Res<CultivationClock>>,
    season_state: Option<Res<WorldSeasonState>>,
    redis: Option<Res<RedisBridgeResource>>,
    mut state: ResMut<EconomyTelemetryState>,
    inventories: Query<&PlayerInventory>,
) {
    let tick = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();
    if !state.should_emit(tick) {
        return;
    }

    let Some(redis) = redis else {
        return;
    };

    let mut supply = BoneCoinSupply::default();
    let mut player_count = 0usize;
    for inventory in inventories.iter() {
        player_count += 1;
        supply.legacy_scalar_count = supply
            .legacy_scalar_count
            .saturating_add(inventory.bone_coins);
        for item in inventory_items(inventory) {
            accumulate_item_supply(&mut supply, item);
        }
    }

    let season = season_state
        .as_deref()
        .map(|state| state.current.season)
        .unwrap_or_else(|| query_season("", tick).season);
    let index = price_index_for_supply(tick, season, supply, demand_for_player_count(player_count));
    let sample_prices = sample_prices_for_index(&index);

    let bone_coin_tick = BoneCoinTickV1::from_index(&index);
    let price_index = PriceIndexV1::from_index(&index, sample_prices);

    if let Err(error) = redis
        .tx_outbound
        .send(RedisOutbound::BoneCoinTick(bone_coin_tick))
    {
        tracing::warn!("[bong][economy] failed to enqueue bone coin tick telemetry: {error}");
        return;
    }
    if let Err(error) = redis
        .tx_outbound
        .send(RedisOutbound::PriceIndex(price_index))
    {
        tracing::warn!("[bong][economy] failed to enqueue price index telemetry: {error}");
        return;
    }

    state.mark_emitted(tick);
}

fn sample_prices_for_index(index: &EconomyPriceIndex) -> Vec<PriceSampleV1> {
    [
        ("common_good", 4_u64),
        ("uncommon_good", 12_u64),
        ("rare_good", 40_u64),
    ]
    .into_iter()
    .map(|(item_id, base_price)| PriceSampleV1 {
        item_id: item_id.to_string(),
        base_price,
        final_price: ((base_price as f64) * index.price_multiplier)
            .round()
            .max(1.0) as u64,
    })
    .collect()
}

fn accumulate_item_supply(supply: &mut BoneCoinSupply, item: &ItemInstance) {
    let stack = item.stack_count.max(1) as u64;
    if is_rotten_bone_coin(item.template_id.as_str()) {
        supply.rotten_coin_count = supply.rotten_coin_count.saturating_add(stack);
        return;
    }

    let Some(face_value) = bone_coin_face_value(item.template_id.as_str()) else {
        return;
    };
    let stack_f64 = stack as f64;
    supply.active_coin_count = supply.active_coin_count.saturating_add(stack);
    supply.total_face_value += face_value * stack_f64;
    supply.total_spirit_qi += face_value * item.spirit_quality.clamp(0.0, 1.0) * stack_f64;
}

fn inventory_items(inventory: &PlayerInventory) -> impl Iterator<Item = &ItemInstance> {
    inventory
        .containers
        .iter()
        .flat_map(|container| container.items.iter().map(|placed| &placed.instance))
        .chain(inventory.equipped.values())
        .chain(inventory.hotbar.iter().filter_map(|item| item.as_ref()))
}

fn turbulent_rhythm_multiplier(tick: u64) -> f64 {
    let (min, max) = QI_RHYTHM_TURBULENT_RANGE;
    let step = (tick % 97) as f64 / 96.0;
    min + (max - min) * step
}

impl EconomyTelemetryState {
    fn should_emit(&self, tick: u64) -> bool {
        self.last_emitted_tick
            .is_none_or(|last| tick.saturating_sub(last) >= ECONOMY_TELEMETRY_INTERVAL_TICKS)
    }

    fn mark_emitted(&mut self, tick: u64) {
        self.last_emitted_tick = Some(tick);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{
        ContainerState, InventoryRevision, ItemRarity, PlacedItemState, MAIN_PACK_CONTAINER_ID,
    };
    use std::collections::HashMap;

    fn item(template_id: &str, stack: u32, spirit_quality: f64) -> ItemInstance {
        ItemInstance {
            instance_id: 1,
            template_id: template_id.to_string(),
            display_name: template_id.to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.0,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: stack,
            spirit_quality,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        }
    }

    fn inventory(items: Vec<ItemInstance>) -> PlayerInventory {
        PlayerInventory {
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                id: MAIN_PACK_CONTAINER_ID.to_string(),
                name: "main".to_string(),
                rows: 3,
                cols: 9,
                items: items
                    .into_iter()
                    .enumerate()
                    .map(|(idx, instance)| PlacedItemState {
                        row: 0,
                        col: idx as u8,
                        instance,
                    })
                    .collect(),
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 7,
            max_weight: 45.0,
        }
    }

    #[test]
    fn bone_coin_supply_uses_spirit_quality_and_ignores_rotten_qi() {
        let inv = inventory(vec![
            item("bone_coin_5", 2, 0.5),
            item("bone_coin_40", 1, 0.25),
            item("rotten_bone_coin", 3, 1.0),
            item("spirit_grass", 4, 1.0),
        ]);

        let supply = collect_bone_coin_supply([&inv]);

        assert_eq!(supply.active_coin_count, 3);
        assert_eq!(supply.rotten_coin_count, 3);
        assert_eq!(supply.legacy_scalar_count, 7);
        assert_eq!(supply.total_face_value, 50.0);
        assert_eq!(supply.total_spirit_qi, 15.0);
    }

    #[test]
    fn market_factor_is_monotonic_and_clamped() {
        let scarce = market_factor_for_supply(1.0, 100.0);
        let balanced = market_factor_for_supply(100.0, 100.0);
        let abundant = market_factor_for_supply(10_000.0, 100.0);

        assert!(scarce < balanced);
        assert!(balanced < abundant);
        assert_eq!(scarce, MARKET_FACTOR_MIN);
        assert_eq!(abundant, MARKET_FACTOR_MAX);
    }

    #[test]
    fn rhythm_multiplier_uses_qi_physics_constants() {
        assert_eq!(
            rhythm_multiplier_for_season(Season::Summer, 0),
            QI_RHYTHM_ACTIVE
        );
        assert_eq!(
            rhythm_multiplier_for_season(Season::Winter, 0),
            QI_RHYTHM_NEUTRAL
        );

        let turbulent = rhythm_multiplier_for_season(Season::SummerToWinter, 48);
        assert!(turbulent >= QI_RHYTHM_TURBULENT_RANGE.0);
        assert!(turbulent <= QI_RHYTHM_TURBULENT_RANGE.1);
    }

    #[test]
    fn indexed_price_applies_market_and_rhythm_multiplier() {
        let item = item("spirit_grass", 2, 1.0);
        let index = price_index_for_supply(
            0,
            Season::Summer,
            BoneCoinSupply {
                total_spirit_qi: 100.0,
                total_face_value: 100.0,
                ..Default::default()
            },
            100.0,
        );

        assert_eq!(estimate_item_price_with_index(&item, 4, &index), 14);
    }
}
