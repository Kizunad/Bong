use std::collections::{HashMap, HashSet};

use bong_server::inventory::*;
use bong_server::world::dimension::DimensionKind;

fn test_template(
    template_id: &str,
    category: ItemCategory,
    grid_w: u8,
    grid_h: u8,
    max_stack_count: u32,
) -> ItemTemplate {
    ItemTemplate {
        id: template_id.to_string(),
        display_name: template_id.to_string(),
        category,
        placeable: None,
        max_stack_count,
        grid_w,
        grid_h,
        base_weight: 0.1,
        rarity: ItemRarity::Common,
        spirit_quality_initial: 1.0,
        description: "test template".to_string(),
        effect: None,
        cast_duration_ms: DEFAULT_CAST_DURATION_MS,
        cooldown_ms: DEFAULT_COOLDOWN_MS,
        weapon_spec: None,
        forge_station_spec: None,
        blueprint_scroll_spec: None,
        inscription_scroll_spec: None,
        technique_scroll_spec: None,
        readable_scroll_spec: None,
        recipe_fragment_spec: None,
        container_spec: None,
        shield_spec: None,
        shelflife_profile: None,
        shelflife_track: None,
        wearer_race: Default::default(),
    }
}

fn registry_from_templates(templates: Vec<ItemTemplate>) -> ItemRegistry {
    ItemRegistry::from_map(
        templates
            .into_iter()
            .map(|template| (template.id.clone(), template))
            .collect(),
    )
}

fn empty_inventory(rows: u8, cols: u8) -> PlayerInventory {
    PlayerInventory {
        triggered_treasures: Vec::new(),
        revision: InventoryRevision(0),
        containers: vec![ContainerState {
            quick_access: false,
            id: MAIN_PACK_CONTAINER_ID.to_string(),
            name: "主背包".to_string(),
            rows,
            cols,
            items: Vec::new(),
            owner_instance_id: None,
        }],
        equipped: HashMap::new(),
        hotbar: Default::default(),
        bone_coins: 0,
        max_weight: 99.0,
    }
}

fn make_test_item_instance(instance_id: u64, template_id: &str) -> ItemInstance {
    ItemInstance {
        instance_id,
        template_id: template_id.to_string(),
        display_name: template_id.to_string(),
        grid_w: 1,
        grid_h: 1,
        weight: 0.1,
        rarity: ItemRarity::Common,
        description: "test".to_string(),
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

// 本批测试中的 fixture 均为 1×1 物品；用坐标唯一性验证“不重叠”契约，
// 不复制生产侧 footprint 判定实现。
fn assert_container_has_no_overlaps(container: &ContainerState) {
    let mut occupied = HashSet::new();
    for placed in &container.items {
        assert!(
            occupied.insert((placed.row, placed.col)),
            "items at ({}, {}) should not overlap",
            placed.row,
            placed.col
        );
    }
}
#[test]
fn allocator_rejects_values_above_js_safe_integer_max() {
    let mut allocator = InventoryInstanceIdAllocator::new(JS_SAFE_INTEGER_MAX);
    assert_eq!(
        allocator.next_id().expect("max id should be allocatable"),
        JS_SAFE_INTEGER_MAX
    );

    let error = allocator
        .next_id()
        .expect_err("allocator should fail after JS safe integer max");
    assert!(error.contains("exceeds JS safe integer max"));
}

#[test]
fn instantiated_inventory_uses_allocator_ids_within_js_safe_bound() {
    let registry = load_item_registry().expect("item registry should load");
    let loadout = load_default_loadout(&registry).expect("default loadout should load");
    let mut allocator = InventoryInstanceIdAllocator::new(1);

    let player_inventory = instantiate_inventory_from_loadout(&loadout, &mut allocator, &registry)
        .expect("inventory should instantiate from loadout");

    assert_eq!(player_inventory.revision, InventoryRevision(1));
    assert_eq!(player_inventory.bone_coins, loadout.bone_coins);
    assert!(
        (player_inventory.max_weight - loadout.max_weight).abs() < f64::EPSILON,
        "expected instantiated max_weight {} to match loadout {}",
        player_inventory.max_weight,
        loadout.max_weight
    );

    for item in player_inventory
        .containers
        .iter()
        .flat_map(|container| container.items.iter().map(|entry| &entry.instance))
        .chain(
            player_inventory
                .equipped
                .values()
                .flat_map(|s| s.iter_all()),
        )
        .chain(player_inventory.hotbar.iter().flatten())
    {
        assert!(item.instance_id <= JS_SAFE_INTEGER_MAX);
        assert!(!item.display_name.trim().is_empty());
    }
}

#[test]
fn find_free_slot_returns_top_left_for_empty_container() {
    let inventory = empty_inventory(5, 7);
    let main_pack = &inventory.containers[0];

    assert_eq!(find_free_slot(main_pack, 1, 1), Some((0, 0)));
    assert_eq!(find_free_slot(main_pack, 2, 2), Some((0, 0)));
}

#[test]
fn find_free_slot_scans_row_major_and_respects_multicell_bounds() {
    let registry =
        registry_from_templates(vec![test_template("wide", ItemCategory::Misc, 2, 2, 1)]);
    let mut inventory = empty_inventory(3, 3);
    let mut allocator = InventoryInstanceIdAllocator::new(1);

    add_item_to_player_inventory(&mut inventory, &registry, &mut allocator, "wide", 1, 0)
        .expect("first wide item should fit at top-left");

    let main_pack = &inventory.containers[0];
    assert_eq!(
        find_free_slot(main_pack, 1, 1),
        Some((0, 2)),
        "row-major scan should skip the occupied 2x2 footprint"
    );
    assert_eq!(
        find_free_slot(main_pack, 2, 2),
        None,
        "remaining space cannot hold a second 2x2 footprint"
    );
}

#[test]
fn find_free_slot_finds_fragmented_hole_and_returns_none_when_full() {
    let registry = registry_from_templates(vec![test_template("one", ItemCategory::Misc, 1, 1, 1)]);
    let mut inventory = empty_inventory(2, 3);
    let mut allocator = InventoryInstanceIdAllocator::new(1);

    for _ in 0..5 {
        add_item_to_player_inventory(&mut inventory, &registry, &mut allocator, "one", 1, 0)
            .expect("first five one-cell items should fit");
    }

    let main_pack = &inventory.containers[0];
    assert_eq!(find_free_slot(main_pack, 1, 1), Some((1, 2)));
    assert_eq!(find_free_slot(main_pack, 2, 2), None);

    add_item_to_player_inventory(&mut inventory, &registry, &mut allocator, "one", 1, 0)
        .expect("last one-cell slot should fit");
    assert_eq!(find_free_slot(&inventory.containers[0], 1, 1), None);
}

#[test]
fn runtime_grant_increments_revision_and_creates_instance() {
    let registry = load_item_registry().expect("item registry should load");
    let loadout = load_default_loadout(&registry).expect("default loadout should load");
    let mut allocator = InventoryInstanceIdAllocator::new(1);
    let mut inventory = instantiate_inventory_from_loadout(&loadout, &mut allocator, &registry)
        .expect("inventory should instantiate from loadout");

    let baseline_revision = inventory.revision;
    let receipt = add_item_to_player_inventory(
        &mut inventory,
        &registry,
        &mut allocator,
        "ci_she_hao",
        2,
        0,
    )
    .expect("runtime inventory grant should succeed for canonical herb");

    assert_eq!(receipt.template_id, "ci_she_hao");
    assert_eq!(receipt.stack_count, 2);
    assert!(receipt.instance_id >= 1);
    assert_eq!(receipt.created_instance_ids, vec![receipt.instance_id]);
    assert!(receipt.merged_instance_ids.is_empty());
    assert_eq!(inventory.revision.0, baseline_revision.0.saturating_add(1));

    // plan-backpack-equip-v1 P2 — 新 loadout 无 main_pack，检查 back_pack（首个非 body_pocket 容器）。
    let primary_pack = inventory
        .containers
        .iter()
        .find(|container| container.id != BODY_POCKET_CONTAINER_ID)
        .expect("primary pack should exist");
    assert!(
        primary_pack
            .items
            .iter()
            .any(|entry| entry.instance.template_id == "ci_she_hao"),
        "runtime grant should materialize in primary pack; got: {:?}",
        primary_pack
            .items
            .iter()
            .map(|p| &p.instance.template_id)
            .collect::<Vec<_>>()
    );
}

#[test]
fn runtime_grant_falls_back_to_body_pocket_when_primary_pack_is_full() {
    let registry = registry_from_templates(vec![test_template("one", ItemCategory::Misc, 1, 1, 1)]);
    let mut inventory = empty_inventory(1, 1);
    inventory.containers[0].items.push(PlacedItemState {
        row: 0,
        col: 0,
        instance: make_test_item_instance(77, "filler"),
    });
    inventory.containers.push(ContainerState {
        quick_access: false,
        id: BODY_POCKET_CONTAINER_ID.to_string(),
        name: "贴身口袋".to_string(),
        rows: 2,
        cols: 3,
        items: Vec::new(),
        owner_instance_id: None,
    });
    let mut allocator = InventoryInstanceIdAllocator::new(100);

    let receipt =
        add_item_to_player_inventory(&mut inventory, &registry, &mut allocator, "one", 1, 0)
            .expect("body_pocket should receive runtime grant when primary pack is full");

    assert_eq!(
        receipt.created_instance_ids,
        vec![100],
        "expected runtime grant to create instance 100 because allocator starts at 100 and no stack merge is possible, actual {:?}",
        receipt.created_instance_ids
    );
    assert_eq!(
        inventory.containers[0].items.len(),
        1,
        "expected primary pack to keep only the original filler because it was full, actual items {:?}",
        inventory.containers[0]
            .items
            .iter()
            .map(|placed| &placed.instance.template_id)
            .collect::<Vec<_>>()
    );
    let body_pocket = inventory
        .containers
        .iter()
        .find(|container| container.id == BODY_POCKET_CONTAINER_ID)
        .unwrap_or_else(|| {
            panic!(
                "expected `{BODY_POCKET_CONTAINER_ID}` to exist because fallback grants need a final carried container, actual container ids {:?}",
                inventory
                    .containers
                    .iter()
                    .map(|container| &container.id)
                    .collect::<Vec<_>>()
            )
        });
    assert_eq!(
        body_pocket.items.len(),
        1,
        "expected body_pocket to receive the grant because primary pack was full, actual items {:?}",
        body_pocket
            .items
            .iter()
            .map(|placed| &placed.instance.template_id)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        body_pocket.items[0].instance.template_id, "one",
        "expected body_pocket item template to be `one` because that was the granted template, actual `{}`",
        body_pocket.items[0].instance.template_id
    );
}

fn filtered_pack_inventory_fixture() -> (ItemRegistry, PlayerInventory, String, String) {
    let mut rejecting_pack_template =
        test_template("mineral_pack", ItemCategory::Container, 1, 1, 1);
    rejecting_pack_template.container_spec = Some(ContainerSpec {
        quick_access: false,
        rows: 2,
        cols: 2,
        weight_capacity: 10.0,
        equip_slot: EQUIP_SLOT_CHEST.to_string(),
        durability_cost_per_op: 0.0,
        attrition_exempt: false,
        accept_filter: Some(vec![ContainerAcceptFilter::Category(ItemCategory::Mineral)]),
    });
    let mut accepting_pack_template =
        test_template("general_pack", ItemCategory::Container, 1, 1, 1);
    accepting_pack_template.container_spec = Some(ContainerSpec {
        quick_access: false,
        rows: 2,
        cols: 2,
        weight_capacity: 10.0,
        equip_slot: EQUIP_SLOT_CHEST.to_string(),
        durability_cost_per_op: 0.0,
        attrition_exempt: false,
        accept_filter: None,
    });
    let registry = registry_from_templates(vec![
        test_template("one", ItemCategory::Misc, 1, 1, 1),
        rejecting_pack_template,
        accepting_pack_template,
    ]);
    let rejecting_pack_item = make_test_item_instance(10, "mineral_pack");
    let accepting_pack_item = make_test_item_instance(20, "general_pack");
    let rejecting_pack_id = container_id_for_worn_pack(rejecting_pack_item.instance_id);
    let accepting_pack_id = container_id_for_worn_pack(accepting_pack_item.instance_id);
    let inventory = PlayerInventory {
        triggered_treasures: Vec::new(),
        revision: InventoryRevision(0),
        containers: vec![
            ContainerState {
                quick_access: false,
                id: rejecting_pack_id.clone(),
                name: "矿物袋".to_string(),
                rows: 2,
                cols: 2,
                items: Vec::new(),
                owner_instance_id: Some(rejecting_pack_item.instance_id),
            },
            ContainerState {
                quick_access: false,
                id: accepting_pack_id.clone(),
                name: "通用包".to_string(),
                rows: 2,
                cols: 2,
                items: Vec::new(),
                owner_instance_id: Some(accepting_pack_item.instance_id),
            },
        ],
        equipped: HashMap::from([(
            EQUIP_SLOT_CHEST.to_string(),
            SlotContents {
                worn: vec![rejecting_pack_item, accepting_pack_item],
                held: None,
            },
        )]),
        hotbar: Default::default(),
        bone_coins: 0,
        max_weight: 99.0,
    };

    (registry, inventory, rejecting_pack_id, accepting_pack_id)
}

#[test]
fn runtime_grant_skips_non_body_pack_when_accept_filter_rejects_item() {
    let (registry, mut inventory, rejecting_pack_id, accepting_pack_id) =
        filtered_pack_inventory_fixture();
    let mut allocator = InventoryInstanceIdAllocator::new(200);

    let receipt =
        add_item_to_player_inventory(&mut inventory, &registry, &mut allocator, "one", 1, 0)
            .expect("general pack should receive runtime grant after filtered pack rejects it");

    assert_eq!(
        receipt.created_instance_ids,
        vec![200],
        "expected grant to create instance 200 in accepting pack because rejecting pack filter only accepts minerals, actual {:?}",
        receipt.created_instance_ids
    );
    let rejecting_pack = inventory
        .containers
        .iter()
        .find(|container| container.id == rejecting_pack_id)
        .expect("rejecting pack should still exist");
    assert!(
        rejecting_pack.items.is_empty(),
        "expected rejecting pack to stay empty because its accept_filter rejects `one`, actual items {:?}",
        rejecting_pack
            .items
            .iter()
            .map(|placed| &placed.instance.template_id)
            .collect::<Vec<_>>()
    );
    let accepting_pack = inventory
        .containers
        .iter()
        .find(|container| container.id == accepting_pack_id)
        .expect("accepting pack should still exist");
    assert_eq!(
        accepting_pack.items.len(),
        1,
        "expected accepting pack to receive one granted item because it has no accept_filter, actual items {:?}",
        accepting_pack
            .items
            .iter()
            .map(|placed| &placed.instance.template_id)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        accepting_pack.items[0].instance.template_id, "one",
        "expected accepting pack item template to be `one` because that was the granted template, actual `{}`",
        accepting_pack.items[0].instance.template_id
    );
}

#[test]
fn existing_item_grant_skips_non_body_pack_when_accept_filter_rejects_item() {
    let (registry, mut inventory, rejecting_pack_id, accepting_pack_id) =
        filtered_pack_inventory_fixture();

    let mut item = make_test_item_instance(300, "one");
    item.spirit_quality = 0.75;
    add_existing_item_to_player_inventory(&mut inventory, &registry, item)
        .expect("existing item should land in accepting pack after filtered pack rejects it");

    let rejecting_pack = inventory
        .containers
        .iter()
        .find(|container| container.id == rejecting_pack_id)
        .expect("rejecting pack should still exist");
    assert!(
        rejecting_pack.items.is_empty(),
        "expected rejecting pack to stay empty because its accept_filter rejects `one`, actual items {:?}",
        rejecting_pack
            .items
            .iter()
            .map(|placed| &placed.instance.template_id)
            .collect::<Vec<_>>()
    );
    let accepting_pack = inventory
        .containers
        .iter()
        .find(|container| container.id == accepting_pack_id)
        .expect("accepting pack should still exist");
    assert_eq!(
        accepting_pack.items.len(),
        1,
        "expected accepting pack to receive one existing loot item because it has no accept_filter, actual items {:?}",
        accepting_pack
            .items
            .iter()
            .map(|placed| &placed.instance.template_id)
            .collect::<Vec<_>>()
    );
    let placed = &accepting_pack.items[0].instance;
    assert_eq!(
        placed.instance_id, 300,
        "expected existing item grant to preserve caller-allocated instance_id 300, actual {}",
        placed.instance_id
    );
    assert!(
        (placed.spirit_quality - 0.75).abs() < f64::EPSILON,
        "expected existing item grant to preserve spirit_quality 0.75, actual {}",
        placed.spirit_quality
    );
}

#[test]
fn runtime_grant_places_multiple_non_stack_items_without_overlap() {
    let registry =
        registry_from_templates(vec![test_template("stone", ItemCategory::Misc, 1, 1, 1)]);
    let mut inventory = empty_inventory(2, 2);
    let mut allocator = InventoryInstanceIdAllocator::new(1);

    let receipt =
        add_item_to_player_inventory(&mut inventory, &registry, &mut allocator, "stone", 4, 0)
            .expect("four non-stack one-cell items should exactly fill a 2x2 pack");

    assert_eq!(receipt.stack_count, 4);
    let main_pack = &inventory.containers[0];
    let positions: Vec<_> = main_pack
        .items
        .iter()
        .map(|placed| (placed.row, placed.col, placed.instance.stack_count))
        .collect();
    assert_eq!(positions, vec![(0, 0, 1), (0, 1, 1), (1, 0, 1), (1, 1, 1)]);
    assert_container_has_no_overlaps(main_pack);

    let error =
        add_item_to_player_inventory(&mut inventory, &registry, &mut allocator, "stone", 1, 0)
            .expect_err("full pack should reject another non-stack item");
    assert!(error.contains("inventory full: stone"));
}

// ─────────────────────────────────────────────────────────────────
// plan-botany-harvest-full-inventory-loss-v1 §P0 — add_item_to_player_inventory_or_ground
// ─────────────────────────────────────────────────────────────────

#[test]
fn grant_or_ground_grants_normally_when_space_available() {
    let registry =
        registry_from_templates(vec![test_template("stone", ItemCategory::Misc, 1, 1, 1)]);
    let mut inventory = empty_inventory(2, 2);
    let mut allocator = InventoryInstanceIdAllocator::new(1);
    let mut dropped_loot = DroppedLootRegistry::default();

    let outcome = add_item_to_player_inventory_or_ground(
        &mut inventory,
        &registry,
        &mut allocator,
        Some(&mut dropped_loot),
        "stone",
        1,
        0,
        [1.0, 2.0, 3.0],
        DimensionKind::Overworld,
        None,
    )
    .expect("space available should grant normally, not fall back to ground");

    match outcome {
        GrantOrGroundOutcome::Granted(receipt) => {
            assert_eq!(
                receipt.revision,
                InventoryRevision(1),
                "successful grant should bump inventory revision to 1"
            );
        }
        GrantOrGroundOutcome::DroppedToGround(entry) => {
            panic!("expected Granted with free space available, got DroppedToGround({entry:?})")
        }
    }
    assert!(
        dropped_loot.entries.is_empty(),
        "no overflow entry should be created when a normal grant succeeds"
    );
}

#[test]
fn grant_or_ground_drops_to_ground_when_inventory_full() {
    let registry = registry_from_templates(vec![test_template(
        "ci_she_hao",
        ItemCategory::Herb,
        1,
        1,
        64,
    )]);
    let mut inventory = empty_inventory(1, 1);
    inventory.containers[0].items.push(PlacedItemState {
        row: 0,
        col: 0,
        instance: make_test_item_instance(1, "occupant"),
    });
    let mut allocator = InventoryInstanceIdAllocator::new(2);
    let mut dropped_loot = DroppedLootRegistry::default();
    let ground_pos = [5.0, 64.0, 5.0];

    let outcome = add_item_to_player_inventory_or_ground(
        &mut inventory,
        &registry,
        &mut allocator,
        Some(&mut dropped_loot),
        "ci_she_hao",
        1,
        0,
        ground_pos,
        DimensionKind::Overworld,
        None,
    )
    .expect(
        "full inventory with a DroppedLootRegistry available should fall back to ground, not error",
    );

    match outcome {
        GrantOrGroundOutcome::DroppedToGround(entry) => {
            assert_eq!(
                entry.world_pos, ground_pos,
                "dropped entry world_pos should equal the caller-provided ground_pos"
            );
            assert_eq!(entry.item.template_id, "ci_she_hao");
        }
        GrantOrGroundOutcome::Granted(receipt) => panic!(
            "expected DroppedToGround when the only cell is occupied, got Granted({receipt:?})"
        ),
    }
    assert_eq!(
        dropped_loot.entries.len(),
        1,
        "overflow item should be recorded exactly once in DroppedLootRegistry"
    );
    assert!(
        inventory.containers[0]
            .items
            .iter()
            .all(|placed| placed.instance.template_id != "ci_she_hao"),
        "ci_she_hao must not silently appear in the full container"
    );
}

#[test]
fn grant_or_ground_errors_when_full_and_no_registry_available() {
    let registry = registry_from_templates(vec![test_template(
        "ci_she_hao",
        ItemCategory::Herb,
        1,
        1,
        64,
    )]);
    let mut inventory = empty_inventory(1, 1);
    inventory.containers[0].items.push(PlacedItemState {
        row: 0,
        col: 0,
        instance: make_test_item_instance(1, "occupant"),
    });
    let mut allocator = InventoryInstanceIdAllocator::new(2);

    let error = add_item_to_player_inventory_or_ground(
        &mut inventory,
        &registry,
        &mut allocator,
        None,
        "ci_she_hao",
        1,
        0,
        [0.0, 0.0, 0.0],
        DimensionKind::Overworld,
        None,
    )
    .expect_err(
        "full inventory with no DroppedLootRegistry must surface an observable error, not panic",
    );

    assert!(
        error.contains("no DroppedLootRegistry"),
        "error should explain the missing ground fallback, got {error:?}"
    );
}

#[test]
fn grant_or_ground_passes_through_unknown_template_error_without_dropping() {
    let registry =
        registry_from_templates(vec![test_template("stone", ItemCategory::Misc, 1, 1, 1)]);
    let mut inventory = empty_inventory(2, 2);
    let mut allocator = InventoryInstanceIdAllocator::new(1);
    let mut dropped_loot = DroppedLootRegistry::default();

    let error = add_item_to_player_inventory_or_ground(
        &mut inventory,
        &registry,
        &mut allocator,
        Some(&mut dropped_loot),
        "nonexistent_item",
        1,
        0,
        [0.0, 0.0, 0.0],
        DimensionKind::Overworld,
        None,
    )
    .expect_err(
        "unknown template id is a structural error and must not be masked as ground overflow",
    );

    assert!(
        error.contains("unknown item template id"),
        "structural error should mention unknown template id, got {error:?}"
    );
    assert!(
        dropped_loot.entries.is_empty(),
        "structural errors must not create a ground drop entry"
    );
}

#[test]
fn runtime_grant_merges_existing_stack_before_allocating_new_slot() {
    let registry = registry_from_templates(vec![test_template(
        "ci_she_hao",
        ItemCategory::Herb,
        1,
        1,
        64,
    )]);
    let mut inventory = empty_inventory(2, 2);
    let mut allocator = InventoryInstanceIdAllocator::new(10);

    add_item_to_player_inventory(
        &mut inventory,
        &registry,
        &mut allocator,
        "ci_she_hao",
        10,
        0,
    )
    .expect("initial herb stack should fit");
    let first_instance_id = inventory.containers[0].items[0].instance.instance_id;

    let receipt = add_item_to_player_inventory(
        &mut inventory,
        &registry,
        &mut allocator,
        "ci_she_hao",
        5,
        0,
    )
    .expect("second herb grant should merge into existing stack");

    assert_eq!(receipt.instance_id, 0);
    assert!(receipt.created_instance_ids.is_empty());
    assert_eq!(receipt.merged_instance_ids, vec![first_instance_id]);
    assert_eq!(inventory.containers[0].items.len(), 1);
    assert_eq!(inventory.containers[0].items[0].instance.stack_count, 15);
}

#[test]
fn runtime_grant_merges_same_block_template_stack() {
    let registry = registry_from_templates(vec![test_template(
        "earth_crumb",
        ItemCategory::Block,
        1,
        1,
        64,
    )]);
    let mut inventory = empty_inventory(2, 2);
    let mut allocator = InventoryInstanceIdAllocator::new(100);

    add_item_to_player_inventory(
        &mut inventory,
        &registry,
        &mut allocator,
        "earth_crumb",
        10,
        0,
    )
    .expect("initial block stack should fit");
    let first_instance_id = inventory.containers[0].items[0].instance.instance_id;

    let receipt = add_item_to_player_inventory(
        &mut inventory,
        &registry,
        &mut allocator,
        "earth_crumb",
        5,
        0,
    )
    .expect("same block template should merge into existing stack");

    assert_eq!(receipt.instance_id, 0);
    assert!(receipt.created_instance_ids.is_empty());
    assert_eq!(receipt.merged_instance_ids, vec![first_instance_id]);
    assert_eq!(inventory.containers[0].items.len(), 1);
    assert_eq!(inventory.containers[0].items[0].instance.stack_count, 15);
}

#[test]
fn runtime_grant_keeps_different_block_templates_in_separate_stacks() {
    let registry = registry_from_templates(vec![
        test_template("earth_crumb", ItemCategory::Block, 1, 1, 64),
        test_template("barren_sand", ItemCategory::Block, 1, 1, 64),
    ]);
    let mut inventory = empty_inventory(2, 2);
    let mut allocator = InventoryInstanceIdAllocator::new(110);

    add_item_to_player_inventory(
        &mut inventory,
        &registry,
        &mut allocator,
        "earth_crumb",
        1,
        0,
    )
    .expect("earth_crumb block stack should fit");
    add_item_to_player_inventory(
        &mut inventory,
        &registry,
        &mut allocator,
        "barren_sand",
        1,
        0,
    )
    .expect("barren_sand block stack should fit");

    let main_pack = &inventory.containers[0];
    assert_eq!(main_pack.items.len(), 2);
    assert_eq!(main_pack.items[0].instance.template_id, "earth_crumb");
    assert_eq!(main_pack.items[0].instance.stack_count, 1);
    assert_eq!(main_pack.items[1].instance.template_id, "barren_sand");
    assert_eq!(main_pack.items[1].instance.stack_count, 1);
}

#[test]
fn runtime_grant_repeated_herb_harvests_merge_into_one_stack() {
    let registry = registry_from_templates(vec![test_template(
        "ci_she_hao",
        ItemCategory::Herb,
        1,
        1,
        64,
    )]);
    let mut inventory = empty_inventory(5, 7);
    let mut allocator = InventoryInstanceIdAllocator::new(30);

    for _ in 0..5 {
        let receipt = add_item_to_player_inventory(
            &mut inventory,
            &registry,
            &mut allocator,
            "ci_she_hao",
            1,
            0,
        )
        .expect("batch herb harvest grant should merge into existing stack");
        if receipt.merged_instance_ids.is_empty() {
            assert_eq!(receipt.created_instance_ids.len(), 1);
        } else {
            assert_eq!(receipt.instance_id, 0);
            assert!(receipt.created_instance_ids.is_empty());
        }
    }

    let main_pack = &inventory.containers[0];
    assert_eq!(main_pack.items.len(), 1);
    assert_eq!(main_pack.items[0].row, 0);
    assert_eq!(main_pack.items[0].col, 0);
    assert_eq!(main_pack.items[0].instance.stack_count, 5);
    assert_eq!(inventory.revision.0, 5);
}

#[test]
fn runtime_grant_caps_stack_and_places_remainder_in_new_slot() {
    let registry = registry_from_templates(vec![test_template(
        "ci_she_hao",
        ItemCategory::Herb,
        1,
        1,
        64,
    )]);
    let mut inventory = empty_inventory(2, 2);
    let mut allocator = InventoryInstanceIdAllocator::new(20);

    add_item_to_player_inventory(
        &mut inventory,
        &registry,
        &mut allocator,
        "ci_she_hao",
        63,
        0,
    )
    .expect("initial herb stack should fit");
    let receipt = add_item_to_player_inventory(
        &mut inventory,
        &registry,
        &mut allocator,
        "ci_she_hao",
        3,
        0,
    )
    .expect("overflow should create a second stack");

    let main_pack = &inventory.containers[0];
    assert_eq!(main_pack.items.len(), 2);
    assert_eq!(main_pack.items[0].instance.stack_count, 64);
    assert_eq!(main_pack.items[1].row, 0);
    assert_eq!(main_pack.items[1].col, 1);
    assert_eq!(main_pack.items[1].instance.stack_count, 2);
    assert_eq!(receipt.instance_id, main_pack.items[1].instance.instance_id);
    assert_eq!(receipt.created_instance_ids, vec![receipt.instance_id]);
    assert_eq!(
        receipt.merged_instance_ids,
        vec![main_pack.items[0].instance.instance_id]
    );
    assert_container_has_no_overlaps(main_pack);
}

#[test]
fn find_mergeable_stack_respects_capacity_boundaries() {
    let registry = registry_from_templates(vec![test_template(
        "ci_she_hao",
        ItemCategory::Herb,
        1,
        1,
        64,
    )]);
    let mut inventory = empty_inventory(2, 2);
    let mut allocator = InventoryInstanceIdAllocator::new(40);

    add_item_to_player_inventory(
        &mut inventory,
        &registry,
        &mut allocator,
        "ci_she_hao",
        1,
        0,
    )
    .expect("initial herb stack should fit");

    assert!(
        find_mergeable_stack(&mut inventory.containers[0], "ci_she_hao", 1).is_none(),
        "max_stack_count=1 must disable stack merging"
    );

    inventory.containers[0].items[0].instance.stack_count = 64;
    assert!(
        find_mergeable_stack(&mut inventory.containers[0], "ci_she_hao", 64).is_none(),
        "full stack must not be mergeable"
    );
}

#[test]
fn runtime_grant_does_not_merge_customized_stack_with_default_grant() {
    let registry = registry_from_templates(vec![test_template(
        "ci_she_hao",
        ItemCategory::Herb,
        1,
        1,
        64,
    )]);
    let mut inventory = empty_inventory(2, 2);
    let mut allocator = InventoryInstanceIdAllocator::new(50);

    add_customized_item_to_player_inventory(
        &mut inventory,
        &registry,
        &mut allocator,
        "ci_she_hao",
        1,
        0,
        |instance| {
            instance.display_name = format!("雷 · {}", instance.display_name);
            instance.spirit_quality = (instance.spirit_quality + 0.1).clamp(0.0, 1.0);
        },
    )
    .expect("customized herb stack should fit");
    let receipt = add_item_to_player_inventory(
        &mut inventory,
        &registry,
        &mut allocator,
        "ci_she_hao",
        1,
        0,
    )
    .expect("default herb should fit beside customized stack");

    let main_pack = &inventory.containers[0];
    assert_eq!(main_pack.items.len(), 2);
    assert!(receipt.merged_instance_ids.is_empty());
    assert_eq!(receipt.created_instance_ids, vec![receipt.instance_id]);
    assert_eq!(main_pack.items[0].instance.stack_count, 1);
    assert_eq!(main_pack.items[1].instance.stack_count, 1);
    assert_ne!(
        main_pack.items[0].instance.display_name,
        main_pack.items[1].instance.display_name
    );
}

// ─── apply_inventory_move ───────────────────────────────────────────────

fn make_test_inventory_with_one_item() -> PlayerInventory {
    let item = ItemInstance {
        instance_id: 42,
        template_id: "rat_tail".to_string(),
        display_name: "噬元鼠尾".to_string(),
        grid_w: 1,
        grid_h: 1,
        weight: 0.2,
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
    };
    PlayerInventory {
        triggered_treasures: Vec::new(),
        revision: InventoryRevision(7),
        containers: vec![
            ContainerState {
                quick_access: false,
                id: MAIN_PACK_CONTAINER_ID.to_string(),
                name: "主背包".to_string(),
                rows: 5,
                cols: 7,
                items: vec![PlacedItemState {
                    row: 0,
                    col: 0,
                    instance: item,
                }],

                owner_instance_id: None,
            },
            ContainerState {
                quick_access: false,
                id: SMALL_POUCH_CONTAINER_ID.to_string(),
                name: "小口袋".to_string(),
                rows: 3,
                cols: 3,
                items: Vec::new(),
                owner_instance_id: None,
            },
            ContainerState {
                quick_access: false,
                id: FRONT_SATCHEL_CONTAINER_ID.to_string(),
                name: "前挂包".to_string(),
                rows: 3,
                cols: 4,
                items: Vec::new(),
                owner_instance_id: None,
            },
        ],
        equipped: HashMap::new(),
        hotbar: Default::default(),
        bone_coins: 0,
        max_weight: 50.0,
    }
}

#[test]
fn apply_move_grid_to_hotbar_succeeds_and_bumps_revision() {
    use bong_server::schema::inventory::InventoryLocationV1;
    let registry = load_item_registry().expect("item registry should load");
    let mut inv = make_test_inventory_with_one_item();
    let outcome = apply_inventory_move(
        &mut inv,
        &registry,
        42,
        &InventoryLocationV1::Container {
            container_id: "main_pack".to_string(),
            row: 0,
            col: 0,
        },
        &InventoryLocationV1::Hotbar { index: 3 },
        false,
    )
    .expect("move should succeed");

    assert_eq!(
        outcome,
        InventoryMoveOutcome::Moved {
            revision: InventoryRevision(8)
        }
    );
    assert!(inv.containers[0].items.is_empty());
    assert_eq!(inv.hotbar[3].as_ref().unwrap().instance_id, 42);
}

#[test]
fn apply_move_rejects_when_from_does_not_match() {
    use bong_server::schema::inventory::InventoryLocationV1;
    let registry = load_item_registry().expect("item registry should load");
    let mut inv = make_test_inventory_with_one_item();
    let result = apply_inventory_move(
        &mut inv,
        &registry,
        42,
        // Wrong from cell.
        &InventoryLocationV1::Container {
            container_id: "main_pack".to_string(),
            row: 1,
            col: 1,
        },
        &InventoryLocationV1::Hotbar { index: 3 },
        false,
    );

    assert!(result.is_err());
    // Inventory unchanged.
    assert_eq!(inv.revision, InventoryRevision(7));
    assert_eq!(inv.containers[0].items.len(), 1);
    assert!(inv.hotbar[3].is_none());
}

#[test]
fn apply_move_swaps_when_target_occupied_with_same_footprint() {
    use bong_server::schema::inventory::InventoryLocationV1;
    let registry = load_item_registry().expect("item registry should load");
    let mut inv = make_test_inventory_with_one_item();
    // Pre-populate hotbar slot 3 with a 1×1 item.
    inv.hotbar[3] = Some(ItemInstance {
        instance_id: 99,
        template_id: "blocker".to_string(),
        display_name: "占位物".to_string(),
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
    });

    let outcome = apply_inventory_move(
        &mut inv,
        &registry,
        42,
        &InventoryLocationV1::Container {
            container_id: "main_pack".to_string(),
            row: 0,
            col: 0,
        },
        &InventoryLocationV1::Hotbar { index: 3 },
        false,
    )
    .expect("swap should succeed");

    assert_eq!(
        outcome,
        InventoryMoveOutcome::Swapped {
            revision: InventoryRevision(8),
            displaced_instance_id: 99,
        }
    );
    // Dragged is now at hotbar(3); displaced is at container(0,0).
    assert_eq!(inv.hotbar[3].as_ref().unwrap().instance_id, 42);
    assert_eq!(inv.containers[0].items.len(), 1);
    assert_eq!(inv.containers[0].items[0].instance.instance_id, 99);
    assert_eq!(inv.containers[0].items[0].row, 0);
    assert_eq!(inv.containers[0].items[0].col, 0);
}

#[test]
fn apply_move_rejects_swap_when_footprints_differ() {
    use bong_server::schema::inventory::InventoryLocationV1;
    let registry = load_item_registry().expect("item registry should load");
    let mut inv = make_test_inventory_with_one_item();
    // Add a 2×2 occupant at container (2,2).
    inv.containers[0].items.push(PlacedItemState {
        row: 2,
        col: 2,
        instance: ItemInstance {
            instance_id: 200,
            template_id: "big".to_string(),
            display_name: "大物".to_string(),
            grid_w: 2,
            grid_h: 2,
            weight: 0.5,
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
        },
    });

    // Try to drop 1×1 (#42) onto the 2×2 anchor — overlap, mismatched footprint → reject.
    let result = apply_inventory_move(
        &mut inv,
        &registry,
        42,
        &InventoryLocationV1::Container {
            container_id: "main_pack".to_string(),
            row: 0,
            col: 0,
        },
        &InventoryLocationV1::Container {
            container_id: "main_pack".to_string(),
            row: 2,
            col: 2,
        },
        false,
    );

    assert!(result.is_err());
    assert_eq!(inv.revision, InventoryRevision(7));
    // Both items remain in their original positions.
    assert_eq!(inv.containers[0].items.len(), 2);
}

#[test]
fn apply_move_within_grid_succeeds() {
    use bong_server::schema::inventory::InventoryLocationV1;
    let registry = load_item_registry().expect("item registry should load");
    let mut inv = make_test_inventory_with_one_item();
    let _ = apply_inventory_move(
        &mut inv,
        &registry,
        42,
        &InventoryLocationV1::Container {
            container_id: "main_pack".to_string(),
            row: 0,
            col: 0,
        },
        &InventoryLocationV1::Container {
            container_id: "main_pack".to_string(),
            row: 2,
            col: 3,
        },
        false,
    )
    .expect("intra-grid move should succeed");

    assert_eq!(inv.containers[0].items.len(), 1);
    let placed = &inv.containers[0].items[0];
    assert_eq!(placed.instance.instance_id, 42);
    assert_eq!(placed.row, 2);
    assert_eq!(placed.col, 3);
}

// ─── plan-rotate-v1 — apply_inventory_move rotated 落位 ────────────────

/// 测试辅助：把 helper 库存里的 #42 改成 2x1 footprint（旋转测试主角）。
fn make_test_inventory_with_2x1_item() -> PlayerInventory {
    let mut inv = make_test_inventory_with_one_item();
    inv.containers[0].items[0].instance.grid_w = 2;
    inv.containers[0].items[0].instance.grid_h = 1;
    inv
}

/// 测试辅助：构造一个占位 ItemInstance（占位物不过 registry 校验，模板可为假）。
fn blocker_instance(instance_id: u64, grid_w: u8, grid_h: u8) -> ItemInstance {
    ItemInstance {
        instance_id,
        template_id: "blocker".to_string(),
        display_name: "占位物".to_string(),
        grid_w,
        grid_h,
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

fn main_pack_loc(row: u64, col: u64) -> bong_server::schema::inventory::InventoryLocationV1 {
    bong_server::schema::inventory::InventoryLocationV1::Container {
        container_id: "main_pack".to_string(),
        row,
        col,
    }
}

/// 2x1 旋转落位成 1x2：dims 互换写回，再旋转移回恢复 2x1（奇偶往返）。
#[test]
fn apply_move_rotated_2x1_lands_as_1x2_then_rotates_back() {
    let registry = load_item_registry().expect("item registry should load");
    let mut inv = make_test_inventory_with_2x1_item();

    let outcome = apply_inventory_move(
        &mut inv,
        &registry,
        42,
        &main_pack_loc(0, 0),
        &main_pack_loc(2, 3),
        true,
    )
    .expect("rotated move should succeed");
    assert_eq!(
        outcome,
        InventoryMoveOutcome::Moved {
            revision: InventoryRevision(8)
        }
    );
    let placed = &inv.containers[0].items[0];
    assert_eq!((placed.row, placed.col), (2, 3));
    assert_eq!(
        (placed.instance.grid_w, placed.instance.grid_h),
        (1, 2),
        "旋转落位后 grid_w/grid_h 应互换为 1x2，实际 {}x{}",
        placed.instance.grid_w,
        placed.instance.grid_h
    );

    // 再次旋转移回原位 → 恢复原朝向 2x1（连按两次 R 的服务端等价）。
    apply_inventory_move(
        &mut inv,
        &registry,
        42,
        &main_pack_loc(2, 3),
        &main_pack_loc(0, 0),
        true,
    )
    .expect("second rotated move should succeed");
    let placed = &inv.containers[0].items[0];
    assert_eq!(
        (placed.instance.grid_w, placed.instance.grid_h),
        (2, 1),
        "二次旋转应恢复原朝向 2x1"
    );
}

/// 2x1 转成 1x2 后撞容器底边（行溢出）→ TargetOutOfBounds；原件朝向/位置无脏状态。
/// 不旋转时同一目标 (4,0) 是合法的（2x1 在最底行放得下），拒绝完全由旋转引起。
#[test]
fn apply_move_rotated_rejects_row_overflow_and_leaves_state_clean() {
    let registry = load_item_registry().expect("item registry should load");
    let mut inv = make_test_inventory_with_2x1_item();

    let error = apply_inventory_move(
        &mut inv,
        &registry,
        42,
        &main_pack_loc(0, 0),
        &main_pack_loc(4, 0),
        true,
    )
    .expect_err("rotated 1x2 at bottom row must overflow 5-row container");
    assert!(
        matches!(error, InventoryMoveRejectReason::TargetOutOfBounds),
        "expected TargetOutOfBounds（行 4 + 高 2 > 5 行），got: {error:?}"
    );
    assert_eq!(
        inv.revision,
        InventoryRevision(7),
        "拒绝后 revision 不得变化"
    );
    let placed = &inv.containers[0].items[0];
    assert_eq!((placed.row, placed.col), (0, 0), "拒绝后物品必须留在原位");
    assert_eq!(
        (placed.instance.grid_w, placed.instance.grid_h),
        (2, 1),
        "拒绝后必须保持原朝向 2x1（不得留下已互换的脏状态）"
    );
}

/// 1x2 转成 2x1 后撞容器右边（列溢出）→ TargetOutOfBounds（镜像方向的越界分支）。
#[test]
fn apply_move_rotated_rejects_col_overflow() {
    let registry = load_item_registry().expect("item registry should load");
    let mut inv = make_test_inventory_with_one_item();
    inv.containers[0].items[0].instance.grid_w = 1;
    inv.containers[0].items[0].instance.grid_h = 2;

    let error = apply_inventory_move(
        &mut inv,
        &registry,
        42,
        &main_pack_loc(0, 0),
        &main_pack_loc(0, 6),
        true,
    )
    .expect_err("rotated 2x1 at rightmost col must overflow 7-col container");
    assert!(
        matches!(error, InventoryMoveRejectReason::TargetOutOfBounds),
        "expected TargetOutOfBounds（列 6 + 宽 2 > 7 列），got: {error:?}"
    );
    let placed = &inv.containers[0].items[0];
    assert_eq!(
        (placed.instance.grid_w, placed.instance.grid_h),
        (1, 2),
        "拒绝后必须保持原朝向 1x2"
    );
}

/// 旋转后的 footprint 撞到别人（非锚点重叠）→ TargetOccupied；无脏状态。
/// 不旋转时同一目标 (2,3) 合法（2x1 横放不碰 (3,3) 的占位物），拒绝完全由旋转引起。
#[test]
fn apply_move_rotated_rejects_collision_and_leaves_state_clean() {
    let registry = load_item_registry().expect("item registry should load");
    let mut inv = make_test_inventory_with_2x1_item();
    inv.containers[0].items.push(PlacedItemState {
        row: 3,
        col: 3,
        instance: blocker_instance(300, 1, 1),
    });

    let error = apply_inventory_move(
        &mut inv,
        &registry,
        42,
        &main_pack_loc(0, 0),
        &main_pack_loc(2, 3),
        true,
    )
    .expect_err("rotated 1x2 at (2,3) overlaps blocker at (3,3)");
    assert!(
        matches!(
            error,
            InventoryMoveRejectReason::TargetOccupied { instance_id: 300 }
        ),
        "expected TargetOccupied by #300, got: {error:?}"
    );
    assert_eq!(inv.revision, InventoryRevision(7));
    let placed = inv.containers[0]
        .items
        .iter()
        .find(|p| p.instance.instance_id == 42)
        .expect("item #42 must remain in container");
    assert_eq!((placed.row, placed.col), (0, 0));
    assert_eq!(
        (placed.instance.grid_w, placed.instance.grid_h),
        (2, 1),
        "拒绝后必须保持原朝向 2x1"
    );
}

/// 旋转后 footprint 与目标位占用者一致 → 走 swap；占用者弹回原位，旋转件落新位。
#[test]
fn apply_move_rotated_swap_succeeds_when_rotated_footprint_matches() {
    let registry = load_item_registry().expect("item registry should load");
    let mut inv = make_test_inventory_with_2x1_item();
    inv.containers[0].items.push(PlacedItemState {
        row: 2,
        col: 2,
        instance: blocker_instance(200, 1, 2),
    });

    let outcome = apply_inventory_move(
        &mut inv,
        &registry,
        42,
        &main_pack_loc(0, 0),
        &main_pack_loc(2, 2),
        true,
    )
    .expect("rotated swap should succeed（旋转后 1x2 与占用者 footprint 相同）");
    assert_eq!(
        outcome,
        InventoryMoveOutcome::Swapped {
            revision: InventoryRevision(8),
            displaced_instance_id: 200,
        }
    );
    let moved = inv.containers[0]
        .items
        .iter()
        .find(|p| p.instance.instance_id == 42)
        .expect("#42 present");
    assert_eq!((moved.row, moved.col), (2, 2));
    assert_eq!((moved.instance.grid_w, moved.instance.grid_h), (1, 2));
    let displaced = inv.containers[0]
        .items
        .iter()
        .find(|p| p.instance.instance_id == 200)
        .expect("#200 present");
    assert_eq!((displaced.row, displaced.col), (0, 0));
}

/// 旋转 swap 中占用者放不回原位 → 拒绝，且回滚必须恢复「原朝向」的件
/// （若错误回滚旋转后的件，1x2 在 (0,0) 会与 (1,0) 的占位物重叠 = 脏状态）。
#[test]
fn apply_move_rotated_swap_restore_keeps_original_orientation() {
    let registry = load_item_registry().expect("item registry should load");
    let mut inv = make_test_inventory_with_2x1_item();
    // 目标位占用者：1x2（与旋转后的 #42 footprint 相同 → 触发 swap 分支）。
    inv.containers[0].items.push(PlacedItemState {
        row: 2,
        col: 2,
        instance: blocker_instance(200, 1, 2),
    });
    // (1,0) 占位物：让 1x2 的 #200 放不回 (0,0)（rows 0-1 col 0 与其重叠）。
    // 原朝向 2x1 的 #42（row 0, cols 0-1）与它不重叠。
    inv.containers[0].items.push(PlacedItemState {
        row: 1,
        col: 0,
        instance: blocker_instance(300, 1, 1),
    });

    let error = apply_inventory_move(
        &mut inv,
        &registry,
        42,
        &main_pack_loc(0, 0),
        &main_pack_loc(2, 2),
        true,
    )
    .expect_err("swap must reject：#200 (1x2) 放不回 (0,0)");
    assert!(
        matches!(
            error,
            InventoryMoveRejectReason::TargetOccupied { instance_id: 300 }
        ),
        "expected TargetOccupied by #300, got: {error:?}"
    );
    assert_eq!(inv.revision, InventoryRevision(7));
    assert_eq!(inv.containers[0].items.len(), 3, "三件必须全部原样保留");
    let restored = inv.containers[0]
        .items
        .iter()
        .find(|p| p.instance.instance_id == 42)
        .expect("#42 must be restored");
    assert_eq!((restored.row, restored.col), (0, 0));
    assert_eq!(
        (restored.instance.grid_w, restored.instance.grid_h),
        (2, 1),
        "swap 回滚必须放回原朝向 2x1（回滚旋转件会与 #300 重叠）"
    );
    let occupant = inv.containers[0]
        .items
        .iter()
        .find(|p| p.instance.instance_id == 200)
        .expect("#200 must be restored");
    assert_eq!((occupant.row, occupant.col), (2, 2));
}

/// 1x1 物品 rotated=true 是 no-op：移动照常成功，dims 不变。
#[test]
fn apply_move_rotated_1x1_is_noop() {
    let registry = load_item_registry().expect("item registry should load");
    let mut inv = make_test_inventory_with_one_item();

    apply_inventory_move(
        &mut inv,
        &registry,
        42,
        &main_pack_loc(0, 0),
        &main_pack_loc(2, 3),
        true,
    )
    .expect("1x1 rotated move should succeed as plain move");
    let placed = &inv.containers[0].items[0];
    assert_eq!((placed.row, placed.col), (2, 3));
    assert_eq!(
        (placed.instance.grid_w, placed.instance.grid_h),
        (1, 1),
        "1x1 物品旋转是 no-op，dims 不得变化"
    );
}

/// 2x2 正方形物品 rotated=true 同样 no-op（互换恒等，直接跳过）。
#[test]
fn apply_move_rotated_square_2x2_is_noop() {
    let registry = load_item_registry().expect("item registry should load");
    let mut inv = make_test_inventory_with_one_item();
    inv.containers[0].items[0].instance.grid_w = 2;
    inv.containers[0].items[0].instance.grid_h = 2;

    apply_inventory_move(
        &mut inv,
        &registry,
        42,
        &main_pack_loc(0, 0),
        &main_pack_loc(2, 3),
        true,
    )
    .expect("2x2 rotated move should succeed as plain move");
    let placed = &inv.containers[0].items[0];
    assert_eq!(
        (placed.instance.grid_w, placed.instance.grid_h),
        (2, 2),
        "正方形物品旋转是 no-op，dims 不得变化"
    );
}

/// 非网格目标（hotbar）rotated=true 被忽略：落位成功且保持原朝向。
#[test]
fn apply_move_rotated_ignored_for_hotbar_target() {
    use bong_server::schema::inventory::InventoryLocationV1;
    let registry = load_item_registry().expect("item registry should load");
    let mut inv = make_test_inventory_with_2x1_item();

    apply_inventory_move(
        &mut inv,
        &registry,
        42,
        &main_pack_loc(0, 0),
        &InventoryLocationV1::Hotbar { index: 3 },
        true,
    )
    .expect("hotbar move with rotated flag should succeed");
    let item = inv.hotbar[3].as_ref().expect("#42 in hotbar");
    assert_eq!(
        (item.grid_w, item.grid_h),
        (2, 1),
        "非网格目标必须忽略旋转标志，保持原朝向 2x1"
    );
}

/// rotated=false 全兼容旧行为：2x1 移动后仍是 2x1。
#[test]
fn apply_move_rotated_false_keeps_orientation() {
    let registry = load_item_registry().expect("item registry should load");
    let mut inv = make_test_inventory_with_2x1_item();

    apply_inventory_move(
        &mut inv,
        &registry,
        42,
        &main_pack_loc(0, 0),
        &main_pack_loc(2, 3),
        false,
    )
    .expect("plain move should succeed");
    let placed = &inv.containers[0].items[0];
    assert_eq!(
        (placed.instance.grid_w, placed.instance.grid_h),
        (2, 1),
        "rotated=false 必须保持原朝向（旧行为兼容）"
    );
}

#[test]
fn apply_move_allows_weapon_to_main_hand() {
    use bong_server::schema::inventory::{EquipSlotV1, InventoryLocationV1};

    let registry = load_item_registry().expect("item registry should load");
    let mut inv = make_test_inventory_with_one_item();
    inv.containers[0].items[0].instance.template_id = "iron_sword".to_string();
    inv.containers[0].items[0].instance.display_name = "铁剑".to_string();
    inv.containers[0].items[0].instance.grid_h = 2;

    let outcome = apply_inventory_move(
        &mut inv,
        &registry,
        42,
        &InventoryLocationV1::Container {
            container_id: "main_pack".to_string(),
            row: 0,
            col: 0,
        },
        &InventoryLocationV1::Equip {
            slot: EquipSlotV1::MainHand,
            state: bong_server::schema::inventory::EquipStateV1::Held,
        },
        false,
    )
    .expect("weapon should equip to main_hand");

    assert_eq!(
        outcome,
        InventoryMoveOutcome::Moved {
            revision: InventoryRevision(8)
        }
    );
    assert_eq!(
        inv.equipped
            .get(EQUIP_SLOT_MAIN_HAND)
            .and_then(|s| s.held.as_ref())
            .map(|item| item.template_id.as_str()),
        Some("iron_sword")
    );
}

#[test]
fn apply_move_allows_tool_to_main_hand() {
    use bong_server::schema::inventory::{EquipSlotV1, InventoryLocationV1};

    let registry = load_item_registry().expect("item registry should load");
    let mut inv = make_test_inventory_with_one_item();
    inv.containers[0].items[0].instance.template_id = "dun_qi_jia".to_string();
    inv.containers[0].items[0].instance.display_name = "钝气夹".to_string();

    let outcome = apply_inventory_move(
        &mut inv,
        &registry,
        42,
        &InventoryLocationV1::Container {
            container_id: "main_pack".to_string(),
            row: 0,
            col: 0,
        },
        &InventoryLocationV1::Equip {
            slot: EquipSlotV1::MainHand,
            state: bong_server::schema::inventory::EquipStateV1::Held,
        },
        false,
    )
    .expect("tool should equip to main_hand");

    assert_eq!(
        outcome,
        InventoryMoveOutcome::Moved {
            revision: InventoryRevision(8)
        }
    );
    assert_eq!(
        inv.equipped
            .get(EQUIP_SLOT_MAIN_HAND)
            .and_then(|s| s.held.as_ref())
            .map(|item| item.template_id.as_str()),
        Some("dun_qi_jia")
    );
}

#[test]
fn apply_move_allows_tool_to_off_hand() {
    // 用户反馈：工具双手都要能装。off_hand 现也放行 Tool/Hoe（与 client InventoryEquipRules
    // OFF_HAND 同步）。此前 off_hand 只收 dagger/fist/treasure/shield，工具被拒。
    use bong_server::schema::inventory::{EquipSlotV1, InventoryLocationV1};

    let registry = load_item_registry().expect("item registry should load");
    let mut inv = make_test_inventory_with_one_item();
    inv.containers[0].items[0].instance.template_id = "stone_pickaxe".to_string();
    inv.containers[0].items[0].instance.display_name = "石镐".to_string();

    let outcome = apply_inventory_move(
        &mut inv,
        &registry,
        42,
        &InventoryLocationV1::Container {
            container_id: "main_pack".to_string(),
            row: 0,
            col: 0,
        },
        &InventoryLocationV1::Equip {
            slot: EquipSlotV1::OffHand,
            state: bong_server::schema::inventory::EquipStateV1::Held,
        },
        false,
    )
    .expect("tool should equip to off_hand");

    assert_eq!(
        outcome,
        InventoryMoveOutcome::Moved {
            revision: InventoryRevision(8)
        }
    );
    assert_eq!(
        inv.equipped
            .get(EQUIP_SLOT_OFF_HAND)
            .and_then(|s| s.held.as_ref())
            .map(|item| item.template_id.as_str()),
        Some("stone_pickaxe")
    );
}

#[test]
fn apply_move_rejects_block_to_main_hand() {
    use bong_server::schema::inventory::{EquipSlotV1, InventoryLocationV1};

    let registry = load_item_registry().expect("item registry should load");
    let mut inv = make_test_inventory_with_one_item();
    inv.containers[0].items[0].instance.template_id = "earth_crumb".to_string();
    inv.containers[0].items[0].instance.display_name = "土屑".to_string();

    let error = apply_inventory_move(
        &mut inv,
        &registry,
        42,
        &InventoryLocationV1::Container {
            container_id: "main_pack".to_string(),
            row: 0,
            col: 0,
        },
        &InventoryLocationV1::Equip {
            slot: EquipSlotV1::MainHand,
            state: bong_server::schema::inventory::EquipStateV1::Held,
        },
        false,
    )
    .expect_err("block items must not equip to main_hand");

    assert!(
        matches!(error, InventoryMoveRejectReason::EquipCategoryMismatch),
        "expected main_hand category rejection, got: {error:?}"
    );
    assert!(!inv.equipped.contains_key(EQUIP_SLOT_MAIN_HAND));
}
