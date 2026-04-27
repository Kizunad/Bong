//! plan-shelflife-v1 M6 — server tick boundary 200 sweep。
//!
//! plan §6.1 第 7 条 access-time：每 200 tick（与 worldstate publish 同节拍），
//! 全局扫描所有 PlayerInventory，对 track_state 边界跨越的 item 做 ID 变体切换。
//!
//! 本系统仅在 sweep 时修改 item — 不对 snapshot emit / probe / consume 读路径产生副作用。

use valence::prelude::{Query, Res, ResMut, Update};

use crate::inventory::{bump_revision, ItemRegistry, PlayerInventory};

use super::registry::DecayProfileRegistry;
use super::variant::apply_variant_switch;

/// plan §6.1 第 7 条：每 200 tick sweep 所有玩家 inventory，
/// 对 `TrackState::Dead` / `AgePostPeakSpoiled` 的 item 执行变体切换。
/// 切换后 bump revision 通知客户端。
pub fn sweep_shelflife_variants(
    mut inventories: Query<&mut PlayerInventory>,
    profile_registry: Res<DecayProfileRegistry>,
    item_registry: Res<ItemRegistry>,
    mut tick_counter: ResMut<ShelflifeSweepTick>,
) {
    tick_counter.0 = tick_counter.0.wrapping_add(1);
    if tick_counter.0 % 200 != 0 {
        return;
    }

    for mut inventory in inventories.iter_mut() {
        let mut any_switched = false;

        for container in &mut inventory.containers {
            for placed in &mut container.items {
                if apply_variant_switch(
                    &mut placed.instance,
                    &profile_registry,
                    &item_registry,
                    tick_counter.0,
                ) {
                    any_switched = true;
                }
            }
        }

        for item in inventory.equipped.values_mut() {
            if apply_variant_switch(item, &profile_registry, &item_registry, tick_counter.0) {
                any_switched = true;
            }
        }

        for item in inventory.hotbar.iter_mut().flatten() {
            if apply_variant_switch(item, &profile_registry, &item_registry, tick_counter.0) {
                any_switched = true;
            }
        }

        if any_switched {
            bump_revision(&mut inventory);
        }
    }
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
