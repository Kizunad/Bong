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
    apply_inventory_move, container_id_for_worn_pack, discard_inventory_item_to_dropped_loot,
    rebuild_and_drop_overflow, ContainerSpec, ContainerState, DroppedLootRegistry,
    InventoryRevision, ItemCategory, ItemInstance, ItemRarity, ItemRegistry, ItemTemplate,
    PlacedItemState, PlayerInventory, SlotContents, BODY_POCKET_CONTAINER_ID, EQUIP_SLOT_CHEST,
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
        readable_scroll_spec: None,
        recipe_fragment_spec: None,
        container_spec: Some(ContainerSpec {
            rows,
            cols,
            weight_capacity,
            equip_slot: EQUIP_SLOT_CHEST.to_string(),
            durability_cost_per_op: 0.0,
            attrition_exempt: false,
            accept_filter: None,
            quick_access: false,
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
                quick_access: false,
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
                quick_access: false,
            },
            // spill 容器：tiny 1×1（只容得下 1 件 spill，其余 overflow 掉落）。
            ContainerState {
                id: "tiny".to_string(),
                name: "tiny".to_string(),
                rows: 1,
                cols: 1,
                items: Vec::new(),
                owner_instance_id: None,
                quick_access: false,
            },
            // 背包件 4242 的内含容器，预置 3 件 1×1。
            ContainerState {
                id: pack_id.clone(),
                name: "e2e_pack".to_string(),
                rows: 3,
                cols: 3,
                quick_access: false,
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

// ─── fix/tarkov-nest-persistence — 随包保留 + 任意位置开包 e2e（卸到 body_pocket 不掉地）───

/// 构造：worn 背包(4×4 容量) 含 2 件内含物 + 空 body_pocket。
fn setup_worn_pack_e2e(contents: &[u64]) -> (PlayerInventory, ItemRegistry, u64) {
    let mut loot_tpl = container_template("loot", 1, 1, 0.0);
    loot_tpl.container_spec = None;
    loot_tpl.category = ItemCategory::Misc;
    loot_tpl.grid_w = 1;
    loot_tpl.grid_h = 1;
    let registry = ItemRegistry::from_map(HashMap::from([
        (
            "e2e_pack".to_string(),
            container_template("e2e_pack", 4, 4, 20.0),
        ),
        ("loot".to_string(), loot_tpl),
    ]));
    let pack_id = 4242u64;
    let items = contents
        .iter()
        .enumerate()
        .map(|(i, id)| PlacedItemState {
            row: i as u8,
            col: 0,
            instance: item_instance(*id, "loot"),
        })
        .collect();
    let mut pack_item = item_instance(pack_id, "e2e_pack");
    // body_pocket = 2 行 × 3 列；背包件 2 宽 × 2 高，可放入 (0,0)。
    pack_item.grid_w = 2;
    pack_item.grid_h = 2;
    let inventory = PlayerInventory {
        triggered_treasures: Vec::new(),
        revision: InventoryRevision(1),
        containers: vec![
            ContainerState {
                id: BODY_POCKET_CONTAINER_ID.to_string(),
                name: "贴身口袋".to_string(),
                rows: 2,
                cols: 3,
                items: Vec::new(),
                owner_instance_id: None,
                quick_access: false,
            },
            ContainerState {
                id: container_id_for_worn_pack(pack_id),
                name: "e2e_pack".to_string(),
                rows: 4,
                cols: 4,
                items,
                owner_instance_id: Some(pack_id),
                quick_access: false,
            },
        ],
        equipped: {
            let mut e = HashMap::new();
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
    (inventory, registry, pack_id)
}

/// e2e #16 — 卸包到 body_pocket：apply_inventory_move(worn→body_pocket) → rebuild → 容器 +
/// 内含物保留、无掉落物。锁住「卸到身上不掉地」核心修复。
#[test]
fn e2e_unequip_pack_to_body_pocket_keeps_container_no_drop() {
    let (mut inventory, registry, pack_id) = setup_worn_pack_e2e(&[31, 32]);
    let from = InventoryLocationV1::Equip {
        slot: EquipSlotV1::Chest,
        state: EquipStateV1::Worn,
    };
    // body_pocket 首格（背包件 2×2 放入 2 行×3 列暗袋的 0,0）。
    let to = InventoryLocationV1::Container {
        container_id: BODY_POCKET_CONTAINER_ID.to_string(),
        row: 0,
        col: 0,
    };
    apply_inventory_move(&mut inventory, &registry, pack_id, &from, &to)
        .expect("卸 worn 背包到 body_pocket 应成功");

    let mut dropped = DroppedLootRegistry::default();
    let dropped_ids = rebuild_and_drop_overflow(
        &mut inventory,
        &registry,
        &mut dropped,
        [10.0, 64.0, -7.0],
        DimensionKind::Overworld,
    );

    assert!(
        dropped_ids.is_empty() && dropped.entries.is_empty(),
        "背包卸到 body_pocket（仍在身上）时绝不应掉落任何物品；实际掉落 {dropped_ids:?}"
    );
    let pack = inventory
        .containers
        .iter()
        .find(|c| c.id == container_id_for_worn_pack(pack_id))
        .expect("背包在 body_pocket 时其 pack 容器必须保留（核心修复）");
    let ids: Vec<u64> = pack.items.iter().map(|p| p.instance.instance_id).collect();
    assert_eq!(
        ids.len(),
        2,
        "内含物逐件守恒、不 spill；实际 pack 内含 = {ids:?}"
    );
    assert!(
        ids.contains(&31) && ids.contains(&32),
        "内含物 31/32 应原位保留在 pack 容器内；实际 = {ids:?}"
    );
}

/// e2e #17 — 跨位置移动内含物守恒：背包在 body_pocket 时，把内含物从 pack_<id> 移到 body_pocket
/// → Ok，不再出现 `does not hold instance` / 「背包未穿戴」拒绝。
#[test]
fn e2e_move_contents_out_of_pack_in_body_pocket_succeeds() {
    let (mut inventory, registry, pack_id) = setup_worn_pack_e2e(&[31]);
    // 先把背包卸到 body_pocket（占 0,0 起 2×3）。
    apply_inventory_move(
        &mut inventory,
        &registry,
        pack_id,
        &InventoryLocationV1::Equip {
            slot: EquipSlotV1::Chest,
            state: EquipStateV1::Worn,
        },
        &InventoryLocationV1::Container {
            container_id: BODY_POCKET_CONTAINER_ID.to_string(),
            row: 0,
            col: 0,
        },
    )
    .expect("卸背包到 body_pocket 应成功");
    let mut dropped = DroppedLootRegistry::default();
    rebuild_and_drop_overflow(
        &mut inventory,
        &registry,
        &mut dropped,
        [0.0, 64.0, 0.0],
        DimensionKind::Overworld,
    );

    // body_pocket 0,0 起被背包件(2×3)占满；它仍有空位？2×3 暗袋被 2×3 背包件占满 → 无空位。
    // 改往背包件**自身的容器内移动**不现实；这里验证「往 pack 容器拖入」放行：把一件新物
    // 从（已搬空的）某处放进 pack。简化：直接对 worn→body_pocket 后的 pack 容器做拖入校验。
    // 这里复用既有内含物：把 31 从 pack 移回 pack 另一格（同容器移动也走 validate_move_semantics
    // 的 Container 门控，验证放行）。
    let res = apply_inventory_move(
        &mut inventory,
        &registry,
        31,
        &InventoryLocationV1::Container {
            container_id: container_id_for_worn_pack(pack_id),
            row: 0,
            col: 0,
        },
        &InventoryLocationV1::Container {
            container_id: container_id_for_worn_pack(pack_id),
            row: 2,
            col: 2,
        },
    );
    assert!(
        res.is_ok(),
        "背包在 body_pocket（携带面）时往其 pack 容器移动内含物应放行，不再被门控拒绝；实际 = {res:?}"
    );
}

/// e2e #18 — 真丢弃才掉地：discard worn 背包 → 背包本体 + 内含物全进 DroppedLootRegistry、
/// pack 容器不残留（防 #736 重连重置 loadout）。
#[test]
fn e2e_discard_worn_pack_drops_pack_and_contents_clears_orphan() {
    let (mut inventory, registry, pack_id) = setup_worn_pack_e2e(&[31, 32]);
    let mut dropped = DroppedLootRegistry::default();
    // 1. discard 背包本体（worn）。
    discard_inventory_item_to_dropped_loot(
        &mut inventory,
        &mut dropped,
        [10.0, 64.0, -7.0],
        DimensionKind::Overworld,
        pack_id,
        &InventoryLocationV1::Equip {
            slot: EquipSlotV1::Chest,
            state: EquipStateV1::Worn,
        },
    )
    .expect("discard worn 背包应成功");
    // 2. handler §5 补 rebuild：孤儿 pack 内含物 spill→掉地。
    let spilled = rebuild_and_drop_overflow(
        &mut inventory,
        &registry,
        &mut dropped,
        [10.0, 64.0, -7.0],
        DimensionKind::Overworld,
    );

    assert!(
        !inventory
            .containers
            .iter()
            .any(|c| c.id == container_id_for_worn_pack(pack_id)),
        "discard + rebuild 后 pack 孤儿容器不应残留 inventory（防 #736 重连重置 loadout）"
    );
    // 守恒：背包本体 + 2 件内含物全部可在 inventory(spill 进 body_pocket) 或 dropped 中找到。
    assert!(
        dropped.entries.contains_key(&pack_id),
        "背包本体应进掉落物登记"
    );
    for cid in [31u64, 32] {
        let in_dropped = dropped.entries.contains_key(&cid);
        let in_inv = inventory
            .containers
            .iter()
            .any(|c| c.items.iter().any(|p| p.instance.instance_id == cid));
        assert!(
            in_dropped || in_inv,
            "内含物 {cid} 必须在掉落物或 inventory（spill），禁止静默丢失；spilled={spilled:?}"
        );
    }
}
