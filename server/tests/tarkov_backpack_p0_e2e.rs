//! plan-tarkov-backpack-v1 P0 — e2e（交付物 #4 红线锁定）。
//!
//! 经生产接线 seam `rebuild_and_drop_overflow`（`handle_inventory_move` 卸 worn-pack 分支
//! 调用的同一函数）真路径卸下非空背包：
//!   detach worn 背包件（apply_inventory_move 真路径） → rebuild 触发 → spill 进存活容器、
//!   overflow 转掉落物（写入 DroppedLootRegistry）。
//!
//! **不绕过该 seam 直测 apply_inventory_move 内部**——本 e2e 锁住「卸非空背包 overflow 不丢失」
//! 的接线（rebuild + overflow→掉落），任何把 rebuild/overflow→掉落从 move 路径摘掉的回归立即撞红。
//!
//! 注意：生产 dispatch `handle_inventory_move` 是私有 fn 且依赖 Valence `Client` 组件 / 整套
//! Bevy SystemParam，无法在 `server/tests/` 外部 crate 构造；本 e2e 经其调用的**唯一公共
//! 接线 seam** `rebuild_and_drop_overflow` + 公共 `apply_inventory_move` 走端到端真路径。

use std::collections::HashMap;

use bong_server::inventory::{
    apply_inventory_move, container_id_for_worn_pack, rebuild_and_drop_overflow, ContainerSpec,
    ContainerState, DroppedLootRegistry, InventoryRevision, ItemCategory, ItemInstance, ItemRarity,
    ItemRegistry, ItemTemplate, PlacedItemState, PlayerInventory, SlotContents, EQUIP_SLOT_CHEST,
};
use bong_server::schema::inventory::{EquipSlotV1, EquipStateV1, InventoryLocationV1};
use bong_server::world::dimension::DimensionKind;

fn container_template(id: &str, rows: u8, cols: u8, weight_capacity: f64) -> ItemTemplate {
    ItemTemplate {
        id: id.to_string(),
        display_name: id.to_string(),
        category: ItemCategory::Container,
        placeable: None,
        max_stack_count: 1,
        grid_w: 2,
        grid_h: 3,
        base_weight: 0.5,
        rarity: ItemRarity::Common,
        spirit_quality_initial: 1.0,
        description: "e2e backpack".to_string(),
        effect: None,
        cast_duration_ms: 0,
        cooldown_ms: 0,
        weapon_spec: None,
        forge_station_spec: None,
        blueprint_scroll_spec: None,
        inscription_scroll_spec: None,
        technique_scroll_spec: None,
        recipe_fragment_spec: None,
        container_spec: Some(ContainerSpec {
            rows,
            cols,
            weight_capacity,
            equip_slot: EQUIP_SLOT_CHEST.to_string(),
            durability_cost_per_op: 0.0,
            attrition_exempt: false,
            accept_filter: None,
        }),
        shield_spec: None,
        shelflife_profile: None,
        shelflife_track: None,
    }
}

fn item_instance(instance_id: u64, template_id: &str) -> ItemInstance {
    ItemInstance {
        instance_id,
        template_id: template_id.to_string(),
        display_name: template_id.to_string(),
        grid_w: 1,
        grid_h: 1,
        weight: 0.1,
        rarity: ItemRarity::Common,
        description: String::new(),
        stack_count: 1,
        spirit_quality: 1.0,
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

/// e2e — 卸非空背包：apply_inventory_move 真路径卸下 worn 背包件 → rebuild_and_drop_overflow
/// 触发 → overflow 进 DroppedLootRegistry（守恒、非空、不丢失）。
#[test]
fn e2e_unequip_nonempty_pack_drops_overflow_not_lost() {
    // ── 场景：唯一存活容器极小（tiny 1×1），背包内含 3 件 → 1 件 spill、2 件 overflow 掉落。──
    let registry = ItemRegistry::from_map(HashMap::from([(
        "e2e_pack".to_string(),
        container_template("e2e_pack", 3, 3, 10.0),
    )]));

    let pack_id = container_id_for_worn_pack(4242);
    let mut inventory = PlayerInventory {
        triggered_treasures: Vec::new(),
        revision: InventoryRevision(1),
        containers: vec![
            // body_pocket（2×3=6 格）预填满——rebuild 兜底会用它接 spill，填满后不再吸收，
            // 迫使内含物只能去 tiny（1 格），其余 overflow 掉落。
            ContainerState {
                id: "body_pocket".to_string(),
                name: "贴身口袋".to_string(),
                rows: 2,
                cols: 3,
                items: (0..6)
                    .map(|i| PlacedItemState {
                        row: (i / 3) as u8,
                        col: (i % 3) as u8,
                        instance: item_instance(100 + i as u64, "filler"),
                    })
                    .collect(),
                owner_instance_id: None,
            },
            // 卸下目标容器：stash 恰好 2×3=6 格 = 背包件 footprint，卸入后被填满，
            // 不再有空间接 spill（迫使 spill 只能去 tiny）。
            ContainerState {
                id: "stash".to_string(),
                name: "stash".to_string(),
                rows: 3,
                cols: 2,
                items: Vec::new(),
                owner_instance_id: None,
            },
            // spill 容器：tiny 1×1（只容得下 1 件 spill，其余 overflow 掉落）。
            ContainerState {
                id: "tiny".to_string(),
                name: "tiny".to_string(),
                rows: 1,
                cols: 1,
                items: Vec::new(),
                owner_instance_id: None,
            },
            // 背包件 4242 的内含容器，预置 3 件 1×1。
            ContainerState {
                id: pack_id.clone(),
                name: "e2e_pack".to_string(),
                rows: 3,
                cols: 3,
                items: vec![
                    PlacedItemState {
                        row: 0,
                        col: 0,
                        instance: item_instance(31, "loot_a"),
                    },
                    PlacedItemState {
                        row: 0,
                        col: 1,
                        instance: item_instance(32, "loot_b"),
                    },
                    PlacedItemState {
                        row: 0,
                        col: 2,
                        instance: item_instance(33, "loot_c"),
                    },
                ],
                owner_instance_id: Some(4242),
            },
        ],
        equipped: {
            let mut e = HashMap::new();
            // 背包件穿在 chest worn 层。
            let mut pack_item = item_instance(4242, "e2e_pack");
            pack_item.grid_w = 2;
            pack_item.grid_h = 3;
            e.insert(
                EQUIP_SLOT_CHEST.to_string(),
                SlotContents::worn_single(pack_item),
            );
            e
        },
        hotbar: Default::default(),
        bone_coins: 0,
        max_weight: 100.0,
    };

    // ── 第一步：apply_inventory_move 真路径卸下背包件 chest worn → stash 容器（0,0）。──
    // 背包件 2×3 恰好填满 stash（3×2），背包件离 worn 层 ⇒ pack_4242 变孤儿（待 rebuild 清理）。
    // 非空拒卸硬门已移除，本移动应成功（这是塔科夫式「连货整体卸下」的入口）。
    let from = InventoryLocationV1::Equip {
        slot: EquipSlotV1::Chest,
        state: EquipStateV1::Worn,
    };
    let to = InventoryLocationV1::Container {
        container_id: "stash".to_string(),
        row: 0,
        col: 0,
    };
    apply_inventory_move(&mut inventory, &registry, 4242, &from, &to).expect(
        "apply_inventory_move 卸下非空 worn 背包件到 stash 容器应成功（非空拒卸硬门已移除）",
    );

    // 背包件已离 chest worn 层。
    assert!(
        inventory
            .equipped
            .get(EQUIP_SLOT_CHEST)
            .map(|s| s.worn.is_empty())
            .unwrap_or(true),
        "卸下后 chest worn 层应为空"
    );

    // ── 第二步：经生产接线 seam（handle_inventory_move 卸包分支调用的同一函数）触发 rebuild + overflow→掉落。──
    let mut dropped = DroppedLootRegistry::default();
    let dropped_ids = rebuild_and_drop_overflow(
        &mut inventory,
        &registry,
        &mut dropped,
        [10.0, 64.0, -7.0],
        DimensionKind::Overworld,
    );

    // tiny 仅 1 格 → 1 件 spill 进 tiny，2 件 overflow 掉落（守恒：3 = 1 + 2）。
    assert_eq!(
        dropped_ids.len(),
        2,
        "tiny 容器仅 1 格，3 件内含物中 1 件 spill、2 件应转掉落物（禁止静默丢失）；实际掉落 {dropped_ids:?}"
    );
    assert_eq!(
        dropped.entries.len(),
        2,
        "DroppedLootRegistry 应含 2 条掉落条目（overflow 全部入世界，不丢失）"
    );

    // 孤儿 pack 容器已被 rebuild 移除（不可 access，杜绝 Bug C）。
    assert!(
        !inventory.containers.iter().any(|c| c.id == pack_id),
        "卸下背包后其孤儿 `{pack_id}` 容器必须被 rebuild 移除（不留可 access 孤儿）"
    );

    // 守恒：spill（tiny 内）+ 掉落（registry）= 原 3 件，instance 不增不减不蒸发。
    let tiny = inventory
        .containers
        .iter()
        .find(|c| c.id == "tiny")
        .expect("tiny 容器存在");
    let mut all_ids: Vec<u64> = tiny.items.iter().map(|p| p.instance.instance_id).collect();
    all_ids.extend(dropped.entries.keys().copied());
    all_ids.sort_unstable();
    assert_eq!(
        all_ids,
        vec![31, 32, 33],
        "spill + 掉落必须守恒覆盖全部 3 件原内含物（31/32/33），禁止静默丢失；实际并集 = {all_ids:?}"
    );

    // 掉落条目保留原 instance（不分配新 id）+ dimension 正确。
    for id in &dropped_ids {
        let entry = dropped
            .entries
            .get(id)
            .unwrap_or_else(|| panic!("掉落 instance {id} 应在 DroppedLootRegistry"));
        assert_eq!(
            entry.item.instance_id, *id,
            "掉落条目 item.instance_id 应与 key 一致（保留原 instance）"
        );
        assert_eq!(
            entry.dimension,
            DimensionKind::Overworld,
            "掉落物 dimension 应为玩家所在维度（Overworld）"
        );
    }
}
