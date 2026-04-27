//! plan-mineral-v1 §2.2 — `MineralDropEvent` 消费者。
//!
//! break_handler 发 `MineralDropEvent`（player / mineral_id / position）后，本系统
//! 按 `MineralRegistry` 合成 `ItemInstance`（NBT 写 `mineral_id`）塞进玩家 main_pack。
//!
//! 设计要点：
//!  * **不依赖 ItemRegistry**：矿物 item 没有 TOML 模板（避免 18 条重复登记），
//!    display_name / rarity / 默认 grid 由 `MineralRegistry` 提供。
//!  * **template_id = "mineral_{canonical}"**：给 client tooltip / schema layer 留正典锚，
//!    不需要在 ItemRegistry 注册就能区分（mineral_id NBT 才是权威）。
//!  * 找不到 inventory / registry miss / allocator 耗尽 → warn + skip（不 panic，
//!    丢失一次 drop 不阻塞服务器）。

use valence::prelude::{EventReader, Query, Res, ResMut};

use super::events::MineralDropEvent;
use super::registry::{MineralEntry, MineralRegistry};
use super::types::MineralRarity;
use crate::inventory::{
    InventoryInstanceIdAllocator, ItemInstance, ItemRarity, PlacedItemState, PlayerInventory,
    MAIN_PACK_CONTAINER_ID,
};

/// Mineral ore drop 的默认堆数 —— 一次挖方块产一枚（与 vanilla 一致）。
const DEFAULT_DROP_STACK_COUNT: u32 = 1;

/// plan-mineral-v1 §2.2 consumer —— 把 MineralDropEvent 写成 PlayerInventory 的 ItemInstance。
pub fn consume_mineral_drops_into_inventory(
    mut events: EventReader<MineralDropEvent>,
    registry: Res<MineralRegistry>,
    mut allocator: ResMut<InventoryInstanceIdAllocator>,
    mut inventories: Query<&mut PlayerInventory>,
) {
    for event in events.read() {
        let Ok(mut inventory) = inventories.get_mut(event.player) else {
            tracing::warn!(
                target: "bong::mineral",
                "MineralDropEvent for entity {:?} but no PlayerInventory — drop skipped",
                event.player
            );
            continue;
        };

        let Some(entry) = registry.get(event.mineral_id) else {
            tracing::warn!(
                target: "bong::mineral",
                "MineralDropEvent carries unregistered mineral_id {} — drop skipped",
                event.mineral_id
            );
            continue;
        };

        let instance_id = match allocator.next_id() {
            Ok(id) => id,
            Err(err) => {
                tracing::warn!(
                    target: "bong::mineral",
                    "inventory allocator exhausted on mineral drop: {err}"
                );
                continue;
            }
        };

        let instance = build_mineral_item_instance(instance_id, entry, DEFAULT_DROP_STACK_COUNT);

        let Some(main_pack) = inventory
            .containers
            .iter_mut()
            .find(|c| c.id == MAIN_PACK_CONTAINER_ID)
        else {
            tracing::warn!(
                target: "bong::mineral",
                "player {:?} missing main_pack container — mineral drop lost",
                event.player
            );
            continue;
        };

        main_pack.items.push(PlacedItemState {
            row: 0,
            col: 0,
            instance,
        });

        inventory.revision.0 = inventory.revision.0.saturating_add(1);
    }
}

fn build_mineral_item_instance(
    instance_id: u64,
    entry: &MineralEntry,
    stack_count: u32,
) -> ItemInstance {
    ItemInstance {
        instance_id,
        template_id: format!("mineral_{}", entry.canonical_name),
        display_name: entry.display_name_zh.to_string(),
        grid_w: 1,
        grid_h: 1,
        weight: 0.5,
        rarity: rarity_from_mineral(entry.rarity),
        description: String::new(),
        stack_count,
        spirit_quality: 0.0,
        durability: 1.0,
        freshness: None,
        mineral_id: Some(entry.canonical_name.to_string()),
        charges: None,
    }
}

fn rarity_from_mineral(r: MineralRarity) -> ItemRarity {
    match r {
        MineralRarity::Fan => ItemRarity::Common,
        MineralRarity::Ling => ItemRarity::Uncommon,
        MineralRarity::Xi => ItemRarity::Rare,
        MineralRarity::Yi => ItemRarity::Legendary,
    }
}

#[cfg(test)]
mod tests {
    use super::super::registry::build_default_registry;
    use super::super::types::MineralId;
    use super::*;
    use crate::inventory::{
        ContainerState, InventoryRevision, PlayerInventory as Inv, MAIN_PACK_CONTAINER_ID as MAIN,
    };
    use std::collections::HashMap;
    use valence::prelude::{App, BlockPos, Events, Update};

    fn empty_inventory() -> Inv {
        Inv {
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                id: MAIN.to_string(),
                name: MAIN.to_string(),
                rows: 4,
                cols: 4,
                items: Vec::new(),
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 10.0,
        }
    }

    #[test]
    fn rarity_mapping_matches_mineral_tiers() {
        assert_eq!(rarity_from_mineral(MineralRarity::Fan), ItemRarity::Common);
        assert_eq!(
            rarity_from_mineral(MineralRarity::Ling),
            ItemRarity::Uncommon
        );
        assert_eq!(rarity_from_mineral(MineralRarity::Xi), ItemRarity::Rare);
        assert_eq!(
            rarity_from_mineral(MineralRarity::Yi),
            ItemRarity::Legendary
        );
    }

    #[test]
    fn build_mineral_item_instance_carries_canonical_mineral_id() {
        let reg = build_default_registry();
        let entry = reg.get(MineralId::SuiTie).unwrap();
        let item = build_mineral_item_instance(99, entry, 1);
        assert_eq!(item.mineral_id.as_deref(), Some("sui_tie"));
        assert_eq!(item.template_id, "mineral_sui_tie");
        assert_eq!(item.display_name, "髓铁");
        assert_eq!(item.rarity, ItemRarity::Rare);
    }

    #[test]
    fn drop_event_appends_instance_to_main_pack() {
        let mut app = App::new();
        app.add_event::<MineralDropEvent>();
        app.insert_resource(build_default_registry());
        app.insert_resource(InventoryInstanceIdAllocator::default());

        let player = app.world_mut().spawn(empty_inventory()).id();
        app.add_systems(Update, consume_mineral_drops_into_inventory);

        app.world_mut()
            .resource_mut::<Events<MineralDropEvent>>()
            .send(MineralDropEvent {
                player,
                mineral_id: MineralId::FanTie,
                position: BlockPos::new(1, 64, 2),
            });

        app.update();

        let inv = app.world().get::<Inv>(player).expect("player inventory");
        let main = inv
            .containers
            .iter()
            .find(|c| c.id == MAIN)
            .expect("main_pack present");
        assert_eq!(main.items.len(), 1);
        let item = &main.items[0].instance;
        assert_eq!(item.mineral_id.as_deref(), Some("fan_tie"));
        assert_eq!(item.template_id, "mineral_fan_tie");
        assert_eq!(inv.revision.0, 1);
    }

    #[test]
    fn drop_event_without_inventory_is_silently_skipped() {
        let mut app = App::new();
        app.add_event::<MineralDropEvent>();
        app.insert_resource(build_default_registry());
        app.insert_resource(InventoryInstanceIdAllocator::default());
        app.add_systems(Update, consume_mineral_drops_into_inventory);

        // 随便 spawn 一个没 inventory 的 entity
        let ghost = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<Events<MineralDropEvent>>()
            .send(MineralDropEvent {
                player: ghost,
                mineral_id: MineralId::LingTie,
                position: BlockPos::new(0, 0, 0),
            });

        // 不应 panic
        app.update();
    }
}
