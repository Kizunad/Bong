//! plan-shelflife-v1 M6 — server tick boundary 200 sweep。
//!
//! plan §6.1 第 7 条 access-time：每 200 tick（与 worldstate publish 同节拍），
//! 全局扫描所有 PlayerInventory，对 track_state 边界跨越的 item 做 ID 变体切换。
//!
//! plan-food-v1 MAJOR1 — sweep 接入 ContainerFreshnessBehavior：
//! 若容器内存在 ice_cellar（food.container.ice_cellar），则对 Spoil track 物品
//! 应用 SpoilOnly { rate: 0.3 } 乘子，使冰窖 spoil 速率差异 ≥70% 真实生效。
//!
//! 本系统仅在 sweep 时修改 item — 不对 snapshot emit / probe / consume 读路径产生副作用。

use valence::prelude::{DetectChangesMut, Position, Query, Res, ResMut, Update};

use crate::inventory::{bump_revision, ItemRegistry, PlayerInventory};
use crate::spiritwood::item_freshness_behavior;
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::season::query_season;
use crate::world::zone::ZoneRegistry;

use super::compute::zone_multiplier_lookup;
use super::registry::DecayProfileRegistry;
use super::types::ContainerFreshnessBehavior;
use super::variant::{
    apply_variant_switch_with_season, apply_variant_switch_with_season_and_container,
};

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
    if !tick_counter.0.is_multiple_of(200) {
        return;
    }

    for (position, current_dim, mut inventory) in inventories.iter_mut() {
        let mut any_switched = false;
        let inventory_data = inventory.bypass_change_detection();
        let zone_multiplier = zone_multiplier_for_position(zones.as_deref(), position, current_dim);
        let season = query_season("", tick_counter.0).season;

        for container in &mut inventory_data.containers {
            // plan-food-v1 MAJOR1: 扫描容器内是否存在行为修改器物品（ice_cellar / ling_xia）。
            // 找到第一个非 Normal behavior 的容器物品，用其行为修改本容器内所有 item 的腐败速率。
            let container_behavior: ContainerFreshnessBehavior = container
                .items
                .iter()
                .find_map(|placed| {
                    let b = item_freshness_behavior(Some(&placed.instance));
                    if matches!(b, ContainerFreshnessBehavior::Normal) {
                        None
                    } else {
                        Some(b)
                    }
                })
                .unwrap_or(ContainerFreshnessBehavior::Normal);

            for placed in &mut container.items {
                let entropy_seed = placed.instance.instance_id;
                if apply_variant_switch_with_season_and_container(
                    &mut placed.instance,
                    &profile_registry,
                    &item_registry,
                    tick_counter.0,
                    zone_multiplier,
                    season,
                    entropy_seed,
                    &container_behavior,
                ) {
                    any_switched = true;
                }
            }
        }

        for item in inventory_data
            .equipped
            .values_mut()
            .flat_map(|s| s.iter_all_mut())
        {
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

        for item in inventory_data.hotbar.iter_mut().flatten() {
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
            bump_revision(inventory_data);
            inventory.set_changed();
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::cultivation::components::Cultivation;
    use crate::inventory::{
        ContainerState, InventoryRevision, ItemInstance, ItemRarity, ItemTemplate, PlacedItemState,
    };
    use crate::network::inventory_snapshot_emit::emit_changed_inventory_snapshots;
    use crate::player::state::PlayerState;
    use crate::shelflife::{DecayFormula, DecayProfile, DecayProfileId, DecayTrack, Freshness};
    use valence::prelude::{App, Client, IntoSystemConfigs, Username};
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::create_mock_client;

    fn dead_item() -> ItemInstance {
        ItemInstance {
            instance_id: 1,
            template_id: "mineral_ling_shi_fan".to_string(),
            display_name: "凡灵石".to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.5,
            rarity: ItemRarity::Uncommon,
            description: "alive".to_string(),
            stack_count: 1,
            spirit_quality: 0.8,
            durability: 0.7,
            freshness: Some(Freshness {
                created_at_tick: 0,
                initial_qi: 0.0,
                track: DecayTrack::Decay,
                profile: DecayProfileId::new("ling_shi_fan_v1"),
                frozen_accumulated: 0,
                frozen_since_tick: None,
            }),
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

    fn snapshot_count(helper: &mut valence::testing::MockClientHelper) -> usize {
        helper
            .collect_received()
            .0
            .into_iter()
            .filter(|frame| {
                let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                    return false;
                };
                if packet.channel.as_str() != "bong:server_data" {
                    return false;
                }
                serde_json::from_slice::<crate::schema::server_data::ServerDataV1>(packet.data.0 .0)
                    .is_ok_and(|payload| {
                        matches!(
                            payload.payload,
                            crate::schema::server_data::ServerDataPayloadV1::InventorySnapshot(_)
                        )
                    })
            })
            .count()
    }

    #[test]
    fn second_dead_template_sweep_does_not_bump_revision_or_flush_snapshot() {
        let mut profile_registry = DecayProfileRegistry::new();
        profile_registry
            .insert(DecayProfile::Decay {
                id: DecayProfileId::new("ling_shi_fan_v1"),
                formula: DecayFormula::Linear {
                    decay_per_tick: 1.0,
                },
                floor_qi: 0.0,
            })
            .unwrap();
        let mut template = ItemTemplate::minimal_for_test("dead_mineral_ling_shi_fan");
        template.display_name = "死灵石".to_string();
        template.description = "dead".to_string();
        let item_registry =
            ItemRegistry::from_map(HashMap::from([(template.id.clone(), template)]));

        let mut app = App::new();
        app.insert_resource(profile_registry);
        app.insert_resource(item_registry);
        app.insert_resource(ShelflifeSweepTick(198));
        app.add_systems(
            Update,
            (
                sweep_shelflife_variants,
                emit_changed_inventory_snapshots.after(sweep_shelflife_variants),
            ),
        );
        let (mut bundle, mut helper) = create_mock_client("Collector");
        bundle.player.position = Position::new([0.0, 64.0, 0.0]);
        let player = app.world_mut().spawn(bundle).id();
        app.world_mut().entity_mut(player).insert((
            PlayerInventory {
                revision: InventoryRevision(0),
                containers: vec![ContainerState {
                    id: "main_pack".to_string(),
                    name: "主背包".to_string(),
                    rows: 1,
                    cols: 1,
                    items: vec![PlacedItemState {
                        row: 0,
                        col: 0,
                        instance: dead_item(),
                    }],
                    owner_instance_id: None,
                    quick_access: false,
                }],
                equipped: Default::default(),
                hotbar: Default::default(),
                bone_coins: 0,
                max_weight: 45.0,
                triggered_treasures: Vec::new(),
            },
            PlayerState::default(),
            Cultivation::default(),
        ));

        // Clear Added<PlayerInventory> first; the changed emitter intentionally suppresses join hydration.
        app.update();
        app.update();
        app.world_mut()
            .get_mut::<Client>(player)
            .unwrap()
            .flush_packets()
            .unwrap();
        assert_eq!(snapshot_count(&mut helper), 1);
        assert_eq!(
            app.world().get::<PlayerInventory>(player).unwrap().revision,
            InventoryRevision(1)
        );

        app.world_mut().resource_mut::<ShelflifeSweepTick>().0 = 399;
        app.update();
        app.world_mut()
            .get_mut::<Client>(player)
            .unwrap()
            .flush_packets()
            .unwrap();

        assert_eq!(
            app.world().get::<PlayerInventory>(player).unwrap().revision,
            InventoryRevision(1),
            "second sweep over an already matching dead template must not bump revision"
        );
        assert_eq!(
            snapshot_count(&mut helper),
            0,
            "second no-op sweep must not mark inventory changed or flush a snapshot"
        );
        assert_eq!(
            app.world()
                .get::<PlayerInventory>(player)
                .unwrap()
                .containers[0]
                .items[0]
                .instance
                .template_id,
            "dead_mineral_ling_shi_fan"
        );
        assert_eq!(app.world().get::<Username>(player).unwrap().0, "Collector");
    }
}
