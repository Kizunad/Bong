//! plan-shelflife-v1 M6 — server tick boundary 200 sweep。
//!
//! plan §6.1 第 7 条 access-time：每 200 tick（与 worldstate publish 同节拍），
//! 全局扫描所有 PlayerInventory，对 track_state 边界跨越的 item 做 ID 变体切换。
//!
//! 本系统仅在 sweep 时修改 item — 不对 snapshot emit / probe / consume 读路径产生副作用。

use valence::prelude::{Position, Query, Res, ResMut, Update};

use crate::inventory::{bump_revision, ItemRegistry, PlayerInventory};
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::season::query_season;
use crate::world::zone::ZoneRegistry;

use super::compute::zone_multiplier_lookup;
use super::registry::DecayProfileRegistry;
use super::variant::apply_variant_switch_with_season;

/// plan §6.1 第 7 条：每 200 tick sweep 所有玩家 inventory，
/// 对 `TrackState::Dead` / `AgePostPeakSpoiled` 的 item 执行变体切换。
/// 切换后 bump revision 通知客户端。
pub fn sweep_shelflife_variants(
    mut inventories: Query<(&Position, Option<&CurrentDimension>, &mut PlayerInventory)>,
    zones: Option<Res<ZoneRegistry>>,
    profile_registry: Res<DecayProfileRegistry>,
    item_registry: Res<ItemRegistry>,
    mut tick_counter: ResMut<ShelflifeSweepTick>,
) {
    tick_counter.0 = tick_counter.0.wrapping_add(1);
    if tick_counter.0 % 200 != 0 {
        return;
    }

    for (position, current_dim, mut inventory) in inventories.iter_mut() {
        let mut any_switched = false;
        let zone_multiplier = zone_multiplier_for_position(zones.as_deref(), position, current_dim);
        let season = query_season("", tick_counter.0).season;

        for container in &mut inventory.containers {
            for placed in &mut container.items {
                let entropy_seed = placed.instance.instance_id;
                if apply_variant_switch_with_season(
                    &mut placed.instance,
                    &profile_registry,
                    &item_registry,
                    tick_counter.0,
                    zone_multiplier,
                    season,
                    entropy_seed,
                ) {
                    any_switched = true;
                }
            }
        }

        for item in inventory.equipped.values_mut() {
            let entropy_seed = item.instance_id;
            if apply_variant_switch_with_season(
                item,
                &profile_registry,
                &item_registry,
                tick_counter.0,
                zone_multiplier,
                season,
                entropy_seed,
            ) {
                any_switched = true;
            }
        }

        for item in inventory.hotbar.iter_mut().flatten() {
            let entropy_seed = item.instance_id;
            if apply_variant_switch_with_season(
                item,
                &profile_registry,
                &item_registry,
                tick_counter.0,
                zone_multiplier,
                season,
                entropy_seed,
            ) {
                any_switched = true;
            }
        }

        if any_switched {
            bump_revision(&mut inventory);
        }
    }
}

fn zone_multiplier_for_position(
    zones: Option<&ZoneRegistry>,
    position: &Position,
    current_dim: Option<&CurrentDimension>,
) -> f32 {
    let Some(zones) = zones else {
        return 1.0;
    };
    let dim = current_dim.map(|c| c.0).unwrap_or(DimensionKind::Overworld);
    zones
        .find_zone(dim, position.0)
        .map(|zone| zone_multiplier_lookup(zone.spirit_qi))
        .unwrap_or(1.0)
}

/// Sweep 节拍计数器 — 用 u64 回绕保证 infinite server uptime。
#[derive(Debug, Default)]
pub struct ShelflifeSweepTick(pub u64);

impl valence::prelude::Resource for ShelflifeSweepTick {}

/// 注册 sweep 系统 + tick counter resource。
pub fn register_sweep(app: &mut valence::prelude::App) {
    app.insert_resource(ShelflifeSweepTick::default());
    app.add_systems(Update, sweep_shelflife_variants);
}
