use std::collections::HashMap;

use bong_server::combat::components::{CombatState, Wound, WoundKind, Wounds};
use bong_server::combat::CombatClock;
use bong_server::inventory::ancient_relics::AncientRelicPool;
use bong_server::inventory::spirit_treasure::SpiritTreasureRegistry;
use bong_server::inventory::{
    ContainerState, InventoryInstanceIdAllocator, ItemInstance, ItemRegistry, PlacedItemState,
    PlayerInventory, MAIN_PACK_CONTAINER_ID,
};
use bong_server::network::audio_event_emit::PlaySoundRecipeRequest;
use bong_server::network::qi_attrition_emit::AttritionAppliedEvent;
use bong_server::network::vfx_event_emit::VfxEventRequest;
use bong_server::qi_physics::ledger::QiTransfer;
use bong_server::world::loot_pool::{LootEntry, LootPool, LootPoolRegistry};
use bong_server::world::tsy_container::{ContainerKind, KeyKind, LootContainer, SearchProgress};
use bong_server::world::tsy_container_search::*;
use bong_server::world::zone::ZoneRegistry;
use valence::prelude::{App, Entity, Events, Position, Update, Username};
use valence::testing::ScenarioSingleClient;

fn make_inv() -> PlayerInventory {
    PlayerInventory {
        triggered_treasures: Vec::new(),
        revision: bong_server::inventory::InventoryRevision(0),
        containers: vec![ContainerState {
            quick_access: false,
            id: MAIN_PACK_CONTAINER_ID.to_string(),
            name: "主背包".to_string(),
            rows: 4,
            cols: 5,
            items: Vec::new(),
            owner_instance_id: None,
        }],
        equipped: Default::default(),
        hotbar: Default::default(),
        bone_coins: 0,
        max_weight: 100.0,
    }
}

fn key_item(template: &str, instance_id: u64) -> ItemInstance {
    ItemInstance {
        instance_id,
        template_id: template.to_string(),
        display_name: "key".to_string(),
        grid_w: 1,
        grid_h: 1,
        weight: 0.1,
        rarity: bong_server::inventory::ItemRarity::Common,
        description: String::new(),
        stack_count: 1,
        spirit_quality: 0.0,
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

fn place_test_loot(inv: &mut PlayerInventory, instance: ItemInstance) {
    let registry = ItemRegistry::from_map(HashMap::new());
    place_loot_in_carried_inventory(inv, &registry, instance);
}

fn placed_item_summaries(inv: &PlayerInventory) -> Vec<(String, u64, String, u8, u8)> {
    inv.containers
        .iter()
        .flat_map(|container| {
            container.items.iter().map(|placed| {
                (
                    container.id.clone(),
                    placed.instance.instance_id,
                    placed.instance.template_id.clone(),
                    placed.row,
                    placed.col,
                )
            })
        })
        .collect()
}

fn spirit_item(
    template: &str,
    instance_id: u64,
    spirit_quality: f64,
    stack_count: u32,
) -> ItemInstance {
    ItemInstance {
        spirit_quality,
        stack_count,
        ..key_item(template, instance_id)
    }
}

fn run_start_search_at_distance(distance: f64) -> StartSearchResult {
    let scenario = ScenarioSingleClient::new();
    let mut app = scenario.app;
    let player = scenario.client;
    app.add_event::<StartSearchRequest>();
    app.add_event::<StartSearchResult>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.insert_resource(CombatClock { tick: 11 });
    app.insert_resource(SurfaceStashPlayerLimit::default());
    app.add_systems(Update, start_search_container);

    let container = app
        .world_mut()
        .spawn((
            LootContainer::new(
                ContainerKind::DryCorpse,
                "tsy_range_test".to_string(),
                bong_server::world::zone::TsyDepth::Shallow,
                "range_pool".to_string(),
                0,
            ),
            Position::new([distance, 64.0, 0.0]),
        ))
        .id();
    app.world_mut().entity_mut(player).insert((
        Username("Azure".to_string()),
        Position::new([0.0, 64.0, 0.0]),
        CombatState::default(),
        make_inv(),
    ));
    app.world_mut()
        .resource_mut::<Events<StartSearchRequest>>()
        .send(StartSearchRequest { player, container });

    app.update();

    let events = app.world().resource::<Events<StartSearchResult>>();
    let mut reader = events.get_reader();
    let mut emitted: Vec<_> = reader.read(events).cloned().collect();
    assert_eq!(
        emitted.len(),
        1,
        "start_search_container should emit exactly one result at distance {distance}; actual={emitted:?}"
    );
    emitted.remove(0)
}

#[test]
fn start_search_allows_crosshair_range_within_five_blocks() {
    match run_start_search_at_distance(4.75) {
        StartSearchResult::Started { required_ticks, .. } => {
            assert_eq!(required_ticks, ContainerKind::DryCorpse.base_search_ticks());
        }
        other => panic!("expected Started for 4.75 block search range, got {other:?}"),
    }
}

#[test]
fn start_search_rejects_distance_beyond_five_blocks() {
    match run_start_search_at_distance(5.01) {
        StartSearchResult::Rejected { reason, .. } => {
            assert_eq!(reason, SearchRejectionReason::OutOfRange);
        }
        other => panic!("expected OutOfRange rejection beyond 5 blocks, got {other:?}"),
    }
}

#[test]
fn find_key_in_inventory_main_pack() {
    let mut inv = make_inv();
    inv.containers[0].items.push(PlacedItemState {
        row: 0,
        col: 0,
        instance: key_item("key_stone_casket", 42),
    });
    assert_eq!(
        find_key_in_inventory(&inv, KeyKind::StoneCasketKey),
        Some(42)
    );
    assert_eq!(find_key_in_inventory(&inv, KeyKind::JadeCoffinSeal), None);
}

#[test]
fn find_key_in_inventory_hotbar() {
    let mut inv = make_inv();
    inv.hotbar[0] = Some(key_item("key_array_core", 7));
    assert_eq!(
        find_key_in_inventory(&inv, KeyKind::ArrayCoreSigil),
        Some(7)
    );
}

#[test]
fn find_key_in_inventory_none() {
    let inv = make_inv();
    assert_eq!(find_key_in_inventory(&inv, KeyKind::StoneCasketKey), None);
}

#[test]
fn is_in_combat_recognises_active_window() {
    let mut s = CombatState::default();
    assert!(!is_in_combat(&s, 100));
    s.in_combat_until_tick = Some(150);
    assert!(is_in_combat(&s, 100));
    assert!(!is_in_combat(&s, 150)); // 等于不算（in_combat_until_tick > tick）
    assert!(!is_in_combat(&s, 200));
}

#[test]
fn damaged_this_tick_match() {
    let mut w = Wounds::default();
    assert!(!damaged_this_tick(&w, 50));
    w.entries.push(Wound {
        location: bong_server::body_plan::legacy_body_part_to_id(
            bong_server::combat::components::BodyPart::Chest,
        ),
        kind: WoundKind::Blunt,
        severity: 0.1,
        bleeding_per_sec: 0.0,
        created_at_tick: 50,
        inflicted_by: None,
    });
    assert!(damaged_this_tick(&w, 50));
    assert!(!damaged_this_tick(&w, 51));
}

#[test]
fn place_loot_in_carried_inventory_works_with_main_pack() {
    let mut inv = make_inv();
    let item = key_item("iron_sword", 99);
    place_test_loot(&mut inv, item);
    assert_eq!(
        inv.containers[0].items.len(),
        1,
        "expected one loot item because main_pack has space; actual items={:?}",
        placed_item_summaries(&inv)
    );
    assert_eq!(
        inv.containers[0].items[0].instance.instance_id,
        99,
        "expected inserted loot instance_id=99; actual items={:?}",
        placed_item_summaries(&inv)
    );
    assert_eq!(
        inv.revision.0, 1,
        "expected revision=1 because one loot item was inserted; actual revision={}",
        inv.revision.0
    );
}

#[test]
fn place_item_warns_without_main_pack() {
    let mut inv = make_inv();
    inv.containers.clear();
    // 不应 panic，仅警告
    place_test_loot(&mut inv, key_item("x", 1));
    assert!(
        inv.containers.is_empty(),
        "expected no containers to remain because the test cleared inventory before placement; actual containers={:?}",
        inv.containers
            .iter()
            .map(|container| container.id.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn place_item_with_default_runtime_pack_without_main_pack_is_not_lost() {
    let registry = bong_server::inventory::load_item_registry().expect("item registry should load");
    let loadout = bong_server::inventory::load_default_loadout(&registry)
        .expect("default loadout should load");
    let mut allocator = InventoryInstanceIdAllocator::new(70_000);
    let mut inv = bong_server::inventory::instantiate_inventory_from_loadout(
        &loadout,
        &mut allocator,
        &registry,
    )
    .expect("default loadout should instantiate");
    let runtime_pack_id = inv
        .containers
        .iter()
        .find_map(|container| {
            container
                .id
                .strip_prefix("pack_")
                .map(|_| container.id.clone())
        })
        .expect("default loadout should derive a runtime pack_<instance_id> container");
    assert!(
        inv.containers
            .iter()
            .all(|container| container.id != MAIN_PACK_CONTAINER_ID),
        "default loadout no longer creates `{MAIN_PACK_CONTAINER_ID}`; ids={:?}",
        inv.containers
            .iter()
            .map(|container| container.id.as_str())
            .collect::<Vec<_>>()
    );

    place_loot_in_carried_inventory(&mut inv, &registry, key_item("iron_sword", 99));

    let runtime_pack = inv
        .containers
        .iter()
        .find(|container| container.id == runtime_pack_id)
        .expect("runtime pack container should still exist");
    assert!(
        runtime_pack
            .items
            .iter()
            .any(|placed| placed.instance.instance_id == 99),
        "TSY container loot should land in default runtime pack `{runtime_pack_id}` when old `{MAIN_PACK_CONTAINER_ID}` is absent; runtime_pack_items={:?}",
        runtime_pack
            .items
            .iter()
            .map(|placed| (placed.instance.instance_id, placed.instance.template_id.as_str(), placed.row, placed.col))
            .collect::<Vec<_>>()
    );
}

#[test]
fn tick_search_progress_consumes_key_before_placing_loot_into_freed_slot() {
    let scenario = ScenarioSingleClient::new();
    let mut app = scenario.app;
    let player = scenario.client;
    app.add_event::<SearchCompleted>();
    app.add_event::<SearchAborted>();
    app.add_event::<RelicExtracted>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.insert_resource(CombatClock { tick: 7 });
    app.insert_resource(
        bong_server::inventory::load_item_registry().expect("item registry should load"),
    );
    app.insert_resource(AncientRelicPool::default());
    app.insert_resource(SpiritTreasureRegistry::default());
    app.insert_resource(InventoryInstanceIdAllocator::new(50_000));
    app.insert_resource(SurfaceStashPlayerLimit::default());
    app.insert_resource(LootPoolRegistry::from_pools(HashMap::from([(
        "single_key".to_string(),
        LootPool {
            rolls: (1, 1),
            entries: vec![LootEntry {
                template_id: "key_array_core".to_string(),
                weight: 1,
                count: (1, 1),
            }],
        },
    )])));
    app.add_systems(Update, tick_search_progress);

    let mut inv = PlayerInventory {
        containers: vec![ContainerState {
            quick_access: false,
            id: MAIN_PACK_CONTAINER_ID.to_string(),
            name: "tiny".to_string(),
            rows: 1,
            cols: 1,
            items: vec![PlacedItemState {
                row: 0,
                col: 0,
                instance: key_item(KeyKind::StoneCasketKey.template_id(), 42),
            }],
            owner_instance_id: None,
        }],
        ..make_inv()
    };
    inv.revision.0 = 0;

    let container = app
        .world_mut()
        .spawn((
            LootContainer::new(
                ContainerKind::StoneCasket,
                "tsy_key_order_test".to_string(),
                bong_server::world::zone::TsyDepth::Shallow,
                "single_key".to_string(),
                0,
            ),
            Position::new([0.0, 64.0, 0.0]),
        ))
        .id();
    app.world_mut().entity_mut(player).insert((
        Username("Azure".to_string()),
        Position::new([0.0, 64.0, 0.0]),
        CombatState::default(),
        Wounds::default(),
        SearchProgress {
            container,
            required_ticks: 1,
            elapsed_ticks: 0,
            started_at_tick: 7,
            started_pos: [0.0, 64.0, 0.0],
            key_item_instance_id: Some(42),
        },
        inv,
    ));
    app.world_mut()
        .entity_mut(container)
        .get_mut::<LootContainer>()
        .expect("container should exist")
        .searched_by = Some(player);

    app.update();

    let inv = app
        .world()
        .get::<PlayerInventory>(player)
        .expect("player inventory should remain attached");
    let items = placed_item_summaries(inv);
    assert!(
        items
            .iter()
            .any(|(_, _, template_id, row, col)| template_id == "key_array_core" && *row == 0 && *col == 0),
        "expected rolled loot to use the only freed slot after consuming key instance 42 first; actual items={items:?}"
    );
    assert!(
        items
            .iter()
            .all(|(_, instance_id, template_id, _, _)| {
                *instance_id != 42 && template_id != KeyKind::StoneCasketKey.template_id()
            }),
        "expected consumed key instance 42 to be removed before loot placement; actual items={items:?}"
    );
    let container_state = app
        .world()
        .get::<LootContainer>(container)
        .expect("container should remain attached");
    assert!(
        container_state.depleted,
        "expected search completion to mark container depleted; actual container={container_state:?}"
    );

    let completed = app.world().resource::<Events<SearchCompleted>>();
    let mut reader = completed.get_reader();
    let emitted: Vec<_> = reader.read(completed).cloned().collect();
    assert_eq!(
        emitted.len(),
        1,
        "expected exactly one SearchCompleted event after required_ticks reached; actual events={emitted:?}"
    );
    assert_eq!(
        emitted[0].player, player,
        "expected SearchCompleted player to match searched player; actual event={:?}",
        emitted[0]
    );
    assert_eq!(
        emitted[0].container, container,
        "expected SearchCompleted container to match searched container; actual event={:?}",
        emitted[0]
    );
    assert_eq!(
        emitted[0].loot.len(),
        1,
        "expected one rolled loot item from single_key pool; actual loot={:?}",
        emitted[0].loot
    );
}

// ——— loot 多件放不同槽位（bug fix: 不再全堆 (0,0)） ———

#[test]
fn place_loot_in_carried_inventory_multiple_items_land_in_distinct_slots() {
    // 期望：3 件 1x1 loot 各自落在不同 (row,col) 槽位，而非全压到 (0,0)。
    // 修复前：全部 push row=0,col=0（叠在同一格）。
    let mut inv = make_inv(); // 4x5 背包，初始空
    let item_a = key_item("herb_a", 1);
    let item_b = key_item("herb_b", 2);
    let item_c = key_item("herb_c", 3);

    place_test_loot(&mut inv, item_a);
    place_test_loot(&mut inv, item_b);
    place_test_loot(&mut inv, item_c);

    let items = &inv.containers[0].items;
    assert_eq!(
        items.len(),
        3,
        "期望背包中有 3 件 loot，实际 {}",
        items.len()
    );

    // 检查每件物品的 (row,col) 都不相同——修复前全为 (0,0)
    let slots: Vec<(u8, u8)> = items.iter().map(|p| (p.row, p.col)).collect();
    let unique: std::collections::HashSet<(u8, u8)> = slots.iter().cloned().collect();
    assert_eq!(
        unique.len(),
        3,
        "期望 3 件 loot 落在 3 个不同槽位（find_free_slot 分散放置），\
         实际只有 {} 个不同槽位（值为 {:?}）——\
         修复前所有物品都压到 (0,0)",
        unique.len(),
        slots
    );
}

#[test]
fn place_loot_in_carried_inventory_first_item_goes_to_top_left() {
    // 期望：空背包首件 loot 落在 (0,0)——find_free_slot 行主序扫描，空容器返回 (0,0)。
    let mut inv = make_inv();
    place_test_loot(&mut inv, key_item("herb_a", 1));

    let placed = &inv.containers[0].items[0];
    assert_eq!(
        (placed.row, placed.col),
        (0, 0),
        "期望空背包首件 loot 落在 (0,0)（行主序首个空位），实际 ({},{})",
        placed.row,
        placed.col
    );
}

#[test]
fn place_loot_in_carried_inventory_second_item_not_at_origin() {
    // 期望：第二件 1x1 loot 不落在 (0,0)（(0,0) 已被第一件占用）。
    let mut inv = make_inv();
    place_test_loot(&mut inv, key_item("item_a", 1));
    place_test_loot(&mut inv, key_item("item_b", 2));

    let items = &inv.containers[0].items;
    let second = &items[1];
    assert_ne!(
        (second.row, second.col),
        (0, 0),
        "期望第二件 loot 不在 (0,0)（已被第一件占据），\
         实际第二件仍在 (0,0)——这正是被修复的 bug"
    );
}

#[test]
fn place_loot_in_carried_inventory_full_pack_does_not_panic() {
    // 期望：背包已满（1x1 背包放了 1 件）时，再放第二件不 panic、不插入，revision 不再增加。
    use bong_server::inventory::ContainerState;
    let mut inv = PlayerInventory {
        triggered_treasures: Vec::new(),
        revision: bong_server::inventory::InventoryRevision(0),
        containers: vec![ContainerState {
            quick_access: false,
            id: MAIN_PACK_CONTAINER_ID.to_string(),
            name: "tiny".to_string(),
            rows: 1,
            cols: 1,
            items: Vec::new(),
            owner_instance_id: None,
        }],
        equipped: Default::default(),
        hotbar: Default::default(),
        bone_coins: 0,
        max_weight: 100.0,
    };
    place_test_loot(&mut inv, key_item("item_a", 1)); // 占满唯一格
    let rev_after_first = inv.revision.0;
    place_test_loot(&mut inv, key_item("item_b", 2)); // 背包已满，应跳过

    assert_eq!(
        inv.containers[0].items.len(),
        1,
        "期望背包满时第二件 loot 被丢弃（warn），实际插入了——背包只有 1 格"
    );
    assert_eq!(
        inv.revision.0, rev_after_first,
        "期望背包满时 revision 不再递增（无 bump），实际 revision 从 {} 变为 {}",
        rev_after_first, inv.revision.0
    );
}

#[test]
fn place_loot_in_carried_inventory_revision_incremented_per_item() {
    // 期望：每次成功放置 bump_revision，3 件物品后 revision=3。
    let mut inv = make_inv();
    assert_eq!(inv.revision.0, 0, "初始 revision 应为 0");
    place_test_loot(&mut inv, key_item("a", 1));
    assert_eq!(inv.revision.0, 1, "放第 1 件后 revision 应为 1");
    place_test_loot(&mut inv, key_item("b", 2));
    assert_eq!(inv.revision.0, 2, "放第 2 件后 revision 应为 2");
    place_test_loot(&mut inv, key_item("c", 3));
    assert_eq!(inv.revision.0, 3, "放第 3 件后 revision 应为 3");
}

#[test]
fn place_loot_in_carried_inventory_2x1_item_finds_free_slot() {
    // 期望：2x1 (grid_w=2, grid_h=1) 物品能正确调用 find_free_slot，
    // 不会因 grid_w>1 而塞到 col=0..0 导致溢出。
    use bong_server::inventory::ContainerState;
    let mut inv = PlayerInventory {
        triggered_treasures: Vec::new(),
        revision: bong_server::inventory::InventoryRevision(0),
        containers: vec![ContainerState {
            quick_access: false,
            id: MAIN_PACK_CONTAINER_ID.to_string(),
            name: "wide".to_string(),
            rows: 2,
            cols: 4,
            items: Vec::new(),
            owner_instance_id: None,
        }],
        equipped: Default::default(),
        hotbar: Default::default(),
        bone_coins: 0,
        max_weight: 100.0,
    };
    let wide_item = ItemInstance {
        grid_w: 2,
        grid_h: 1,
        ..key_item("wide_relic", 10)
    };
    place_test_loot(&mut inv, wide_item);
    assert_eq!(
        inv.containers[0].items.len(),
        1,
        "期望 2x1 物品被成功放置（背包有足够空间），实际未放入"
    );
    let placed = &inv.containers[0].items[0];
    assert_eq!(
        (placed.row, placed.col),
        (0, 0),
        "期望 2x1 物品首先落在 (0,0)（行主序首个合法空位），实际 ({},{})",
        placed.row,
        placed.col
    );
}

#[test]
fn apply_search_attrition_emits_qi_attrition_vfx_event() {
    let mut app = App::new();
    app.add_event::<SearchCompleted>();
    app.add_event::<QiTransfer>();
    app.add_event::<AttritionAppliedEvent>();
    let mut zones = ZoneRegistry::fallback();
    zones.zones[0].spirit_qi = 0.5;
    app.insert_resource(zones);
    app.add_systems(Update, apply_search_attrition);

    let mut inv = make_inv();
    let item = spirit_item("tsy_spirit_relic", 9001, 1.0, 10);
    inv.containers[0].items.push(PlacedItemState {
        row: 0,
        col: 0,
        instance: item.clone(),
    });
    let player = app
        .world_mut()
        .spawn((inv, Position::new([12.0, 70.0, -4.0])))
        .id();

    app.world_mut().send_event(SearchCompleted {
        player,
        container: Entity::from_raw(77),
        family_id: "test_stash".to_string(),
        loot: vec![item],
    });
    app.update();

    let events = app.world().resource::<Events<AttritionAppliedEvent>>();
    let mut reader = events.get_reader();
    let emitted: Vec<_> = reader.read(events).cloned().collect();
    assert_eq!(
        emitted.len(),
        1,
        "ContainerSearch 磨损应发出 1 条定向 VFX event"
    );
    assert_eq!(emitted[0].operator, player);
    assert_eq!(emitted[0].item_entity_id, 9001);
    assert!(
        (emitted[0].amount_lost - 0.5001).abs() < 1e-6,
        "ContainerSearch rate≈5.001%，stack=10 时损耗应约 0.5001，实际 {}",
        emitted[0].amount_lost
    );
    assert_eq!(emitted[0].world_pos, [12.0, 70.0, -4.0]);
}

// ——— plan-onboarding-loop-v1 P0: SurfaceStashPlayerLimit 测试 ———

#[test]
fn surface_stash_player_limit_allows_3_per_day() {
    let mut limit = SurfaceStashPlayerLimit::default();
    let now = 1_000_000u64;
    for i in 0..SURFACE_STASH_DAILY_LIMIT {
        assert!(
            limit.can_search("stash_0", "player_a", now),
            "第 {} 次搜索应被允许（上限 {}），但被拒绝",
            i + 1,
            SURFACE_STASH_DAILY_LIMIT
        );
        limit.record_search("stash_0", "player_a", now);
    }
}

#[test]
fn surface_stash_player_limit_blocks_4th_search() {
    let mut limit = SurfaceStashPlayerLimit::default();
    let now = 1_000_000u64;
    for _ in 0..SURFACE_STASH_DAILY_LIMIT {
        limit.record_search("stash_0", "player_a", now);
    }
    assert!(
        !limit.can_search("stash_0", "player_a", now),
        "第 {} 次搜索应被拒绝，但被允许",
        SURFACE_STASH_DAILY_LIMIT + 1
    );
}

#[test]
fn surface_stash_player_limit_resets_after_24h() {
    let mut limit = SurfaceStashPlayerLimit::default();
    let now = 1_000_000u64;
    for _ in 0..SURFACE_STASH_DAILY_LIMIT {
        limit.record_search("stash_0", "player_a", now);
    }
    assert!(!limit.can_search("stash_0", "player_a", now));

    // 24h 后重置
    let after_24h = now + 24 * 60 * 60;
    assert!(
        limit.can_search("stash_0", "player_a", after_24h),
        "24h 后限额应重置，但搜索仍被拒绝"
    );
}

#[test]
fn surface_stash_limit_24h_minus_1s_does_not_reset() {
    let mut limit = SurfaceStashPlayerLimit::default();
    let now = 1_000_000u64;
    for _ in 0..SURFACE_STASH_DAILY_LIMIT {
        limit.record_search("stash_0", "player_a", now);
    }
    // 24h - 1s：不应重置
    let almost_24h = now + 24 * 60 * 60 - 1;
    assert!(
        !limit.can_search("stash_0", "player_a", almost_24h),
        "24h-1s 时限额不应重置（off-by-one），但 can_search 返回了 true"
    );
}

#[test]
fn surface_stash_limit_poi_isolation() {
    let mut limit = SurfaceStashPlayerLimit::default();
    let now = 1_000_000u64;
    // 在 stash_0 用完配额
    for _ in 0..SURFACE_STASH_DAILY_LIMIT {
        limit.record_search("stash_0", "player_a", now);
    }
    assert!(
        !limit.can_search("stash_0", "player_a", now),
        "stash_0 配额用尽后应拒绝"
    );
    // stash_1 应独立计数，仍然可搜
    assert!(
        limit.can_search("stash_1", "player_a", now),
        "不同 poi（stash_1）的配额应独立于 stash_0，但被拒绝"
    );
}

#[test]
fn surface_stash_limit_player_isolation() {
    let mut limit = SurfaceStashPlayerLimit::default();
    let now = 1_000_000u64;
    // player_a 用完配额
    for _ in 0..SURFACE_STASH_DAILY_LIMIT {
        limit.record_search("stash_0", "player_a", now);
    }
    assert!(
        !limit.can_search("stash_0", "player_a", now),
        "player_a 配额用尽后应拒绝"
    );
    // player_b 应独立计数，仍然可搜
    assert!(
        limit.can_search("stash_0", "player_b", now),
        "不同玩家（player_b）的配额应独立于 player_a，但被拒绝"
    );
}

#[test]
fn surface_stash_limit_empty_state_allows_search() {
    let mut limit = SurfaceStashPlayerLimit::default();
    let now = 1_000_000u64;
    // 初始无记录时 can_search 应返回 true
    assert!(
        limit.can_search("stash_0", "player_a", now),
        "初始无记录时 can_search 应返回 true，但返回了 false"
    );
}

// ——— plan-tsy-search-cancel-v1 P0 §8.1 #3 — handle_cancel_search 回归测试 ———

fn collect_search_aborted(app: &App) -> Vec<SearchAborted> {
    let events = app.world().resource::<Events<SearchAborted>>();
    let mut reader = events.get_reader();
    reader.read(events).cloned().collect()
}

#[test]
fn handle_cancel_search_removes_progress_and_releases_lock() {
    // 玩家挂 SearchProgress + 容器 searched_by = Some(player)，发一条
    // CancelSearchRequest；期望：进度组件被摘、容器锁被释放、SearchAborted
    // 恰好 1 条且 reason == Cancelled，因为这是主动取消的 happy path。
    let mut app = App::new();
    app.add_event::<CancelSearchRequest>();
    app.add_event::<SearchAborted>();
    app.add_systems(Update, handle_cancel_search);

    let container = app
        .world_mut()
        .spawn(LootContainer::new(
            ContainerKind::StoneCasket,
            "tsy_cancel_test".to_string(),
            bong_server::world::zone::TsyDepth::Shallow,
            "single_key".to_string(),
            0,
        ))
        .id();

    let player = app
        .world_mut()
        .spawn((
            SearchProgress {
                container,
                required_ticks: 100,
                elapsed_ticks: 10,
                started_at_tick: 0,
                started_pos: [0.0, 64.0, 0.0],
                key_item_instance_id: None,
            },
            IsSearching,
        ))
        .id();

    app.world_mut()
        .entity_mut(container)
        .get_mut::<LootContainer>()
        .expect("container should exist")
        .searched_by = Some(player);

    app.world_mut().send_event(CancelSearchRequest { player });
    app.update();

    assert!(
        app.world().get::<SearchProgress>(player).is_none(),
        "expected SearchProgress to be removed from player after cancel, because \
         handle_cancel_search must free the player to search again; actual=present"
    );
    assert!(
        app.world().get::<IsSearching>(player).is_none(),
        "expected IsSearching marker to be removed from player after cancel, because \
         downstream qi-accel query filters on With<IsSearching>; actual=present"
    );

    let container_state = app
        .world()
        .get::<LootContainer>(container)
        .expect("container should remain attached");
    assert_eq!(
        container_state.searched_by, None,
        "expected container searched_by lock to be released after cancel, because a \
         cancelled search must not keep other players locked out; actual={:?}",
        container_state.searched_by
    );

    let emitted = collect_search_aborted(&app);
    assert_eq!(
        emitted.len(),
        1,
        "expected exactly one SearchAborted event for one CancelSearchRequest; actual={emitted:?}"
    );
    assert_eq!(
        emitted[0].player, player,
        "expected SearchAborted.player to match the cancelling player; actual={:?}",
        emitted[0]
    );
    assert_eq!(
        emitted[0].container, container,
        "expected SearchAborted.container to match the searched container; actual={:?}",
        emitted[0]
    );
    assert_eq!(
        emitted[0].reason,
        SearchAbortReason::Cancelled,
        "expected SearchAborted.reason to be Cancelled for a player-initiated cancel, \
         not Moved/Combat/Damaged; actual={:?}",
        emitted[0].reason
    );
}

#[test]
fn handle_cancel_search_is_noop_without_search_progress() {
    // 玩家没有 SearchProgress（例如客户端竞态下重复按取消键，或搜刮早已
    // 结束）时发 CancelSearchRequest；期望 system 不 panic 且不发
    // SearchAborted——这是幂等/误按分支，不应产生任何可观察副作用。
    let mut app = App::new();
    app.add_event::<CancelSearchRequest>();
    app.add_event::<SearchAborted>();
    app.add_systems(Update, handle_cancel_search);

    let player = app.world_mut().spawn_empty().id();

    app.world_mut().send_event(CancelSearchRequest { player });
    app.update();

    assert!(
        app.world().get::<SearchProgress>(player).is_none(),
        "player never had SearchProgress; expected it to remain absent, not spuriously \
         inserted by handle_cancel_search"
    );

    let emitted = collect_search_aborted(&app);
    assert_eq!(
        emitted.len(),
        0,
        "expected zero SearchAborted events when player has no SearchProgress, because \
         handle_cancel_search should `continue` on the missing-progress early-out; actual={emitted:?}"
    );
}

#[test]
fn handle_cancel_search_leaves_other_players_container_lock_untouched() {
    // player 持有指向 container 的 SearchProgress，但该 container 的锁实际被
    // other_player 占着（desync/竞态：player 的 progress 已过期，或指向的容器
    // 已被他人抢占）。取消时必须摘掉 player 自己的进度组件，但 handle_cancel_search
    // 的 owner 守卫（`if c.searched_by == Some(req.player)`）必须阻止 player 释放
    // other_player 的锁——这正是该守卫存在的意义。注意 player 必须真的挂着
    // SearchProgress，否则会在 `progress_q.get` 早退处 `continue`、根本走不到守卫，
    // 那样就退化成 is_noop_without_search_progress 的重复覆盖了。
    let mut app = App::new();
    app.add_event::<CancelSearchRequest>();
    app.add_event::<SearchAborted>();
    app.add_systems(Update, handle_cancel_search);

    let other_player = app.world_mut().spawn_empty().id();

    let container = app
        .world_mut()
        .spawn(LootContainer::new(
            ContainerKind::StoneCasket,
            "tsy_cancel_test_other".to_string(),
            bong_server::world::zone::TsyDepth::Shallow,
            "single_key".to_string(),
            0,
        ))
        .id();
    app.world_mut()
        .entity_mut(container)
        .get_mut::<LootContainer>()
        .expect("container should exist")
        .searched_by = Some(other_player);

    // player 的 SearchProgress 指向那个被 other_player 锁住的容器。
    let player = app
        .world_mut()
        .spawn((
            SearchProgress {
                container,
                required_ticks: 100,
                elapsed_ticks: 10,
                started_at_tick: 0,
                started_pos: [0.0, 64.0, 0.0],
                key_item_instance_id: None,
            },
            IsSearching,
        ))
        .id();

    app.world_mut().send_event(CancelSearchRequest { player });
    app.update();

    // 守卫命中（container.searched_by != Some(player)）：锁不能被误清。
    // 这条断言就是回归探针——若未来把守卫改成无条件 `c.searched_by = None`，
    // 这里会立刻从 Some(other_player) 变成 None 而撞红。
    let container_state = app
        .world()
        .get::<LootContainer>(container)
        .expect("container should remain attached");
    assert_eq!(
        container_state.searched_by,
        Some(other_player),
        "expected other_player's container lock to survive because the owner guard \
         `c.searched_by == Some(req.player)` is false for the cancelling player; \
         a regression to unconditional release would clear it; actual={:?}",
        container_state.searched_by
    );

    // player 自己的进度仍应被摘除（移除不受 owner 守卫门控，先于容器检查执行）。
    assert!(
        app.world().get::<SearchProgress>(player).is_none(),
        "expected the cancelling player's own SearchProgress to be removed even when the \
         container lock is held by someone else; actual=present"
    );
    assert!(
        app.world().get::<IsSearching>(player).is_none(),
        "expected the cancelling player's own IsSearching marker to be removed even when the \
         container lock is held by someone else; actual=present"
    );

    // player 发起了一次真实取消（有 progress），故应恰好 emit 1 条 Cancelled。
    let emitted = collect_search_aborted(&app);
    assert_eq!(
        emitted.len(),
        1,
        "expected exactly one SearchAborted for the cancelling player who held a SearchProgress; \
         actual={emitted:?}"
    );
    assert_eq!(
        emitted[0].player, player,
        "expected SearchAborted.player to be the cancelling player, not the lock owner; actual={:?}",
        emitted[0]
    );
    assert_eq!(
        emitted[0].reason,
        SearchAbortReason::Cancelled,
        "expected reason Cancelled for a player-initiated cancel; actual={:?}",
        emitted[0].reason
    );
}
