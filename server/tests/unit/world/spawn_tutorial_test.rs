use bong_server::alchemy::learned::LearnedRecipes;
use bong_server::alchemy::recipe_fragment::PartialRecipeKnowledge;
use bong_server::forge::learned::LearnedBlueprints;
use bong_server::inventory::{
ContainerState, InventoryInstanceIdAllocator, InventoryRevision, ItemCategory, ItemInstance,
ItemRarity, ItemRegistry, ItemTemplate, PlacedItemState, PlayerInventory,
MAIN_PACK_CONTAINER_ID,
};
use bong_server::world::spawn_tutorial::*;
use std::collections::HashMap;

fn registry_with_spirit_niche_base() -> ItemRegistry {
    let mut templates = HashMap::new();
    templates.insert(
        SPIRIT_NICHE_BASE_TEMPLATE_ID.to_string(),
        ItemTemplate {
            id: SPIRIT_NICHE_BASE_TEMPLATE_ID.to_string(),
            display_name: "灵龛基座".to_string(),
            category: ItemCategory::Misc,
            placeable: None,
            max_stack_count: 1,
            grid_w: 2,
            grid_h: 2,
            base_weight: 6.0,
            rarity: ItemRarity::Rare,
            spirit_quality_initial: 0.5,
            description: "龛石灵铁木台组合的永久复活点基座。".to_string(),
            effect: None,
            cast_duration_ms: 1500,
            cooldown_ms: 1500,
            weapon_spec: None,
            forge_station_spec: None,
            blueprint_scroll_spec: None,
            inscription_scroll_spec: None,
            technique_scroll_spec: None,
            readable_scroll_spec: None,
            recipe_fragment_spec: None,
            container_spec: None,
            shelflife_profile: None,
            shield_spec: None,
            shelflife_track: None,
            wearer_race: bong_server::body_plan::types::RaceGateOwned::default(),
        },
    );
    ItemRegistry::from_map(templates)
}

/// plan-scroll-reading-v1 P0 — registry 仅含 `MERIDIAN_PRIMER_TEMPLATE_ID`（挂了
/// `readable_scroll_spec`），供 grant 幂等测试使用。
fn registry_with_meridian_primer() -> ItemRegistry {
    let mut templates = HashMap::new();
    templates.insert(
        MERIDIAN_PRIMER_TEMPLATE_ID.to_string(),
        ItemTemplate {
            id: MERIDIAN_PRIMER_TEMPLATE_ID.to_string(),
            display_name: "《经脉浅述·残卷》".to_string(),
            category: ItemCategory::Scroll,
            placeable: None,
            max_stack_count: 1,
            grid_w: 1,
            grid_h: 2,
            base_weight: 0.05,
            rarity: ItemRarity::Common,
            spirit_quality_initial: 0.3,
            description: "残破的入门讲义，记述经脉通行之序。翻开可读。".to_string(),
            effect: None,
            cast_duration_ms: 1500,
            cooldown_ms: 1500,
            weapon_spec: None,
            forge_station_spec: None,
            blueprint_scroll_spec: None,
            inscription_scroll_spec: None,
            technique_scroll_spec: None,
            readable_scroll_spec: Some(bong_server::inventory::ReadableScrollSpec {
                title: "《经脉浅述·残卷》".to_string(),
                body_pages: vec!["第一页".to_string(), "第二页".to_string()],
                anim_id: Some("bong:read_scroll".to_string()),
            }),
            recipe_fragment_spec: None,
            container_spec: None,
            shelflife_profile: None,
            shield_spec: None,
            shelflife_track: None,
            wearer_race: bong_server::body_plan::types::RaceGateOwned::default(),
        },
    );
    ItemRegistry::from_map(templates)
}

/// registry 缺 `MERIDIAN_PRIMER_TEMPLATE_ID`（配置错误场景）——只含无关模板。
fn registry_missing_meridian_primer() -> ItemRegistry {
    ItemRegistry::from_map(HashMap::new())
}

fn empty_inventory() -> PlayerInventory {
    PlayerInventory {
        triggered_treasures: Vec::new(),
        revision: InventoryRevision(1),
        containers: vec![ContainerState {
            quick_access: false,
            id: MAIN_PACK_CONTAINER_ID.to_string(),
            name: "主背包".to_string(),
            rows: 3,
            cols: 3,
            items: Vec::new(),
            owner_instance_id: None,
        }],
        equipped: HashMap::new(),
        hotbar: Default::default(),
        bone_coins: 0,
        max_weight: 45.0,
    }
}

#[test]
fn coffin_open_grants_spirit_niche_base_once_per_player_state() {
    let registry = registry_with_spirit_niche_base();
    let mut allocator = InventoryInstanceIdAllocator::new(100);
    let mut state = TutorialState::new(0);
    let mut inventory = empty_inventory();

    let first = grant_coffin_reward_once(
        &mut state,
        &mut inventory,
        &registry,
        &mut allocator,
        [0, 69, 0],
    );
    assert!(matches!(
        first,
        CoffinGrantOutcome::Granted { instance_id: 100 }
    ));
    assert_eq!(inventory.containers[0].items.len(), 1);
    assert_eq!(
        inventory.containers[0].items[0].instance.template_id, SPIRIT_NICHE_BASE_TEMPLATE_ID,
        "coffin reward must be the placeable niche base, not the old material"
    );
    assert!(state.has(TutorialHook::CoffinOpened));

    let second = grant_coffin_reward_once(
        &mut state,
        &mut inventory,
        &registry,
        &mut allocator,
        [0, 69, 0],
    );
    assert_eq!(second, CoffinGrantOutcome::AlreadyOpened);
    assert_eq!(inventory.containers[0].items.len(), 1);
}

// ── plan-scroll-reading-v1 P0 §8.1 #1：grant_meridian_primer_once ──────────

#[test]
fn grant_meridian_primer_once_happy_path() {
    let registry = registry_with_meridian_primer();
    let mut allocator = InventoryInstanceIdAllocator::new(200);
    let mut state = TutorialState::new(0);
    let mut inventory = empty_inventory();

    let outcome =
        grant_meridian_primer_once(&mut state, &mut inventory, &registry, &mut allocator);

    assert!(matches!(
        outcome,
        MeridianPrimerGrantOutcome::Granted { instance_id: 200 }
    ));
    assert_eq!(inventory.containers[0].items.len(), 1);
    assert_eq!(
        inventory.containers[0].items[0].instance.template_id,
        MERIDIAN_PRIMER_TEMPLATE_ID
    );
    assert!(
        state.has(TutorialHook::MeridianPrimerGranted),
        "successful grant must trigger the hook"
    );
}

#[test]
fn grant_meridian_primer_once_does_not_regrant_when_hook_already_set() {
    // 正常收敛路径：hook 已打（例如上次 join 已发放），重复调用必须是 no-op。
    let registry = registry_with_meridian_primer();
    let mut allocator = InventoryInstanceIdAllocator::new(200);
    let mut state = TutorialState::new(0);
    state.trigger(TutorialHook::MeridianPrimerGranted);
    let mut inventory = empty_inventory();

    let outcome =
        grant_meridian_primer_once(&mut state, &mut inventory, &registry, &mut allocator);

    assert_eq!(outcome, MeridianPrimerGrantOutcome::AlreadyGranted);
    assert!(
        inventory.containers[0].items.is_empty(),
        "hook already set must short-circuit before touching inventory at all"
    );
}

#[test]
fn grant_meridian_primer_once_reconnect_does_not_regrant() {
    // 模拟"重连"：同一 state/inventory 上连续两次调用（第一次真发放，第二次是重连判定）。
    let registry = registry_with_meridian_primer();
    let mut allocator = InventoryInstanceIdAllocator::new(200);
    let mut state = TutorialState::new(0);
    let mut inventory = empty_inventory();

    let first =
        grant_meridian_primer_once(&mut state, &mut inventory, &registry, &mut allocator);
    assert!(matches!(first, MeridianPrimerGrantOutcome::Granted { .. }));

    let second =
        grant_meridian_primer_once(&mut state, &mut inventory, &registry, &mut allocator);
    assert_eq!(
        second,
        MeridianPrimerGrantOutcome::AlreadyGranted,
        "reconnect (second call on same state) must not grant a second copy"
    );
    assert_eq!(
        inventory.containers[0].items.len(),
        1,
        "exactly one scroll instance must exist after a granted + a reconnect call"
    );
}

#[test]
fn grant_meridian_primer_once_crash_window_item_present_hook_missing_does_not_duplicate() {
    // 模拟"物品已发 hook 未持久化"的崩溃窗口重登：inventory 里已经有一份
    // MERIDIAN_PRIMER_TEMPLATE_ID（上次 add_item 已落盘），但 TutorialState 的 hook
    // 没跟着落盘（进程在两次写之间崩溃）。重登后必须：不重发物品 + 补打 hook 收敛。
    let registry = registry_with_meridian_primer();
    let mut allocator = InventoryInstanceIdAllocator::new(200);
    let mut state = TutorialState::new(0); // hook 未设置——模拟未持久化
    let mut inventory = inventory_with_items(vec![(MERIDIAN_PRIMER_TEMPLATE_ID, 777)]);

    let outcome =
        grant_meridian_primer_once(&mut state, &mut inventory, &registry, &mut allocator);

    assert_eq!(
        outcome,
        MeridianPrimerGrantOutcome::AlreadyGranted,
        "existing inventory instance must be detected even though the hook was never set"
    );
    assert_eq!(
        inventory.containers[0].items.len(),
        1,
        "must not add a second copy on top of the crash-window survivor instance"
    );
    assert!(
        state.has(TutorialHook::MeridianPrimerGranted),
        "hook must be backfilled so this player converges and stops being re-checked forever"
    );
}

#[test]
fn grant_meridian_primer_once_old_player_backfill_when_never_granted() {
    // 存量老玩家：TutorialState 有真实历史内容（非新建 tick=0），但从未拿到过残卷。
    let registry = registry_with_meridian_primer();
    let mut allocator = InventoryInstanceIdAllocator::new(200);
    let mut state = TutorialState::new(500);
    state.trigger(TutorialHook::CoffinOpened);
    state.trigger(TutorialHook::Moved200Blocks);
    let mut inventory = empty_inventory();

    let outcome =
        grant_meridian_primer_once(&mut state, &mut inventory, &registry, &mut allocator);

    assert!(
        matches!(outcome, MeridianPrimerGrantOutcome::Granted { .. }),
        "old players who never received the scroll must be backfilled exactly once, got {outcome:?}"
    );
    assert_eq!(inventory.containers[0].items.len(), 1);
}

#[test]
fn grant_meridian_primer_once_missing_template_reports_error_without_panicking() {
    let registry = registry_missing_meridian_primer();
    let mut allocator = InventoryInstanceIdAllocator::new(200);
    let mut state = TutorialState::new(0);
    let mut inventory = empty_inventory();

    let outcome =
        grant_meridian_primer_once(&mut state, &mut inventory, &registry, &mut allocator);

    assert!(
        matches!(
            outcome,
            MeridianPrimerGrantOutcome::MissingItemTemplate { .. }
        ),
        "missing registry template must report an error, not panic, got {outcome:?}"
    );
    assert!(
        !state.has(TutorialHook::MeridianPrimerGranted),
        "hook must not be set when the grant itself failed"
    );
    assert!(inventory.containers[0].items.is_empty());
}



#[test]
fn moved_200_blocks_uses_spawn_anchor_not_last_position() {
    let mut state = TutorialState::new(0);
    state.spawn_position = Some([8.0, 70.0, 8.0]);
    state.last_position = Some([180.0, 70.0, 8.0]);

    assert!(!moved_at_least_200_blocks(&state, [190.0, 70.0, 8.0]));
    assert!(moved_at_least_200_blocks(&state, [210.0, 70.0, 8.0]));
}









#[test]
fn rat_swarm_requires_coffin_first_meridian_and_movement_toward_lingquan() {
    let mut state = TutorialState::new(0);
    state.trigger(TutorialHook::CoffinOpened);
    state.trigger(TutorialHook::FirstMeridianOpened);
    state.last_position = Some([0.0, 70.0, 90.0]);
    state.first_lingquan_pos = Some([0.0, 70.0, 0.0]);

    assert!(should_spawn_rat_swarm(&state, [0.0, 70.0, 70.0]));
    assert!(!should_spawn_rat_swarm(&state, [0.0, 70.0, 110.0]));

    state.rat_swarm_spawned_at_tick = Some(12);
    assert!(!should_spawn_rat_swarm(&state, [0.0, 70.0, 60.0]));
}

#[test]
fn telemetry_rate_handles_zero_and_completed_counts() {
    let mut telemetry = TutorialTelemetry::default();
    assert_eq!(telemetry.completion_rate_30min(), 0.0);
    telemetry.started = 4;
    telemetry.completed_within_30min = 3;
    assert_eq!(telemetry.completion_rate_30min(), 0.75);
}

// ── test helper ──────────────────────────────────────────────

fn test_item(instance_id: u64, template_id: &str) -> ItemInstance {
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

fn inventory_with_items(items: Vec<(&str, u64)>) -> PlayerInventory {
    let mut inv = empty_inventory();
    for (idx, (template_id, instance_id)) in items.iter().enumerate() {
        inv.containers[0].items.push(PlacedItemState {
            row: idx as u8,
            col: 0,
            instance: test_item(*instance_id, template_id),
        });
    }
    inv
}

// ── P2.1: CraftHintShown ─────────────────────────────────────

#[test]
fn craft_hint_shown_serde_roundtrip() {
    let hook = TutorialHook::CraftHintShown;
    let json = serde_json::to_string(&hook).expect("CraftHintShown should serialize");
    assert_eq!(
        json, "\"craft_hint_shown\"",
        "CraftHintShown serde rename_all=snake_case must produce 'craft_hint_shown', got {json}"
    );
    let back: TutorialHook =
        serde_json::from_str(&json).expect("CraftHintShown should deserialize");
    assert_eq!(back, hook);
}

#[test]
fn inventory_has_base_material_detects_fan_tie() {
    let inv = inventory_with_items(vec![("fan_tie", 1)]);
    assert!(
        inventory_has_base_material(&inv),
        "inventory with fan_tie should be detected as having base material"
    );
}

#[test]
fn inventory_has_base_material_ignores_non_base() {
    let inv = inventory_with_items(vec![("spirit_niche_stone", 1)]);
    assert!(
        !inventory_has_base_material(&inv),
        "spirit_niche_stone is not a base material, should not trigger"
    );
}

#[test]
fn inventory_has_base_material_empty_inventory() {
    let inv = empty_inventory();
    assert!(
        !inventory_has_base_material(&inv),
        "empty inventory should not have base materials"
    );
}

#[test]
fn craft_hint_fires_once_per_player() {
    let mut state = TutorialState::new(0);
    assert!(
        !state.has(TutorialHook::CraftHintShown),
        "fresh state should not have CraftHintShown"
    );
    assert!(
        state.trigger(TutorialHook::CraftHintShown),
        "first trigger should return true (newly inserted)"
    );
    assert!(
        state.has(TutorialHook::CraftHintShown),
        "state should now have CraftHintShown"
    );
    assert!(
        !state.trigger(TutorialHook::CraftHintShown),
        "second trigger should return false (already present)"
    );
}

#[test]
fn all_base_material_ids_trigger_detection() {
    for &material_id in BASE_MATERIAL_IDS {
        let inv = inventory_with_items(vec![(material_id, 42)]);
        assert!(
            inventory_has_base_material(&inv),
            "BASE_MATERIAL_IDS entry '{material_id}' should be detected in inventory"
        );
    }
}

// ── P2.2: recipe fragment learning flow (client_request plumbing) ──

#[test]
fn alchemy_learn_recipe_fragment_serde_roundtrip() {
    use bong_server::schema::client_request::ClientRequestV1;
    let json = r#"{"type":"alchemy_learn_recipe_fragment","v":1,"item_instance_id":4242}"#;
    let req: ClientRequestV1 = serde_json::from_str(json)
        .expect("AlchemyLearnRecipeFragment should deserialize from JSON");
    match req {
        ClientRequestV1::AlchemyLearnRecipeFragment {
            v,
            item_instance_id,
        } => {
            assert_eq!(v, 1, "version should be 1");
            assert_eq!(item_instance_id, 4242, "item_instance_id should be 4242");
        }
        other => panic!("expected AlchemyLearnRecipeFragment, got {other:?}"),
    }
}

#[test]
fn alchemy_learn_recipe_fragment_rejects_extra_fields() {
    use bong_server::schema::client_request::ClientRequestV1;
    let json = r#"{"type":"alchemy_learn_recipe_fragment","v":1,"item_instance_id":4242,"extra":true}"#;
    assert!(
        serde_json::from_str::<ClientRequestV1>(json).is_err(),
        "extra fields should be rejected by deny_unknown_fields"
    );
}

// ── P2.3: blueprint & recipe asset verification ──

#[test]
fn recipe_hui_yuan_pill_v0_loads_and_has_stages() {
    let registry = bong_server::alchemy::recipe::load_recipe_registry()
        .expect("recipe registry should load from assets");
    let recipe = registry
        .get("hui_yuan_pill_v0")
        .expect("hui_yuan_pill_v0 must exist in recipe registry");
    assert!(
        !recipe.stages.is_empty(),
        "hui_yuan_pill_v0 must have at least one stage"
    );
}

#[test]
fn blueprint_iron_sword_v0_loads_and_has_steps() {
    let registry = bong_server::forge::blueprint::BlueprintRegistry::load_dir(
        bong_server::forge::blueprint::DEFAULT_BLUEPRINTS_DIR,
    )
    .expect("blueprint registry should load from assets");
    let bp = registry
        .get("iron_sword_v0")
        .expect("iron_sword_v0 must exist in blueprint registry");
    assert!(
        !bp.steps.is_empty(),
        "iron_sword_v0 must have at least one step"
    );
}

#[test]
fn loot_pool_surface_stash_craft_contains_blueprint_and_fragment() {
    let registry = bong_server::world::loot_pool::load_loot_pool_registry()
        .expect("loot_pools.json should load");
    let pool = registry
        .get("surface_stash_craft")
        .expect("surface_stash_craft pool must exist in loot_pools.json");
    let has_blueprint = pool
        .entries
        .iter()
        .any(|entry| entry.template_id.contains("blueprint_scroll"));
    assert!(
        has_blueprint,
        "surface_stash_craft loot pool should contain at least one blueprint_scroll entry"
    );
    let has_fragment = pool
        .entries
        .iter()
        .any(|entry| entry.template_id.contains("fragment_alchemy"));
    assert!(
        has_fragment,
        "surface_stash_craft loot pool should contain at least one fragment_alchemy entry"
    );
}

// ── P2.4: first alchemy / forge hint logic ──

#[test]
fn first_alchemy_hint_serde_roundtrip() {
    let hook = TutorialHook::FirstAlchemyHint;
    let json = serde_json::to_string(&hook).expect("FirstAlchemyHint should serialize");
    assert_eq!(
        json, "\"first_alchemy_hint\"",
        "FirstAlchemyHint serde rename_all=snake_case must produce 'first_alchemy_hint', got {json}"
    );
    let back: TutorialHook =
        serde_json::from_str(&json).expect("FirstAlchemyHint should deserialize");
    assert_eq!(back, hook);
}

#[test]
fn first_forge_hint_serde_roundtrip() {
    let hook = TutorialHook::FirstForgeHint;
    let json = serde_json::to_string(&hook).expect("FirstForgeHint should serialize");
    assert_eq!(
        json, "\"first_forge_hint\"",
        "FirstForgeHint serde rename_all=snake_case must produce 'first_forge_hint', got {json}"
    );
    let back: TutorialHook =
        serde_json::from_str(&json).expect("FirstForgeHint should deserialize");
    assert_eq!(back, hook);
}

#[test]
fn alchemy_hint_needs_at_least_one_recipe() {
    // Simulates the logic from check_first_alchemy_hint:
    // learned recipes empty → hint should NOT fire even at correct realm
    let learned = LearnedRecipes::default();
    assert!(
        learned.ids.is_empty() && learned.partial.is_empty(),
        "default LearnedRecipes should be empty"
    );

    // With a partial recipe
    let mut learned_with_partial = LearnedRecipes::default();
    learned_with_partial.partial.push(PartialRecipeKnowledge {
        recipe_id: "hui_yuan_pill_v0".into(),
        known_stages: vec![0],
        max_quality_tier: 3,
    });
    assert!(
        !learned_with_partial.ids.is_empty() || !learned_with_partial.partial.is_empty(),
        "partial recipe should satisfy the has-recipe check"
    );

    // With a full recipe
    let mut learned_with_full = LearnedRecipes::default();
    learned_with_full.ids.push("hui_yuan_pill_v0".into());
    assert!(
        !learned_with_full.ids.is_empty(),
        "full recipe should satisfy the has-recipe check"
    );
}

#[test]
fn forge_hint_needs_at_least_one_blueprint() {
    // Empty
    let learned = LearnedBlueprints::default();
    assert!(
        learned.ids.is_empty(),
        "default LearnedBlueprints should be empty"
    );

    // With a blueprint
    let mut learned_with_bp = LearnedBlueprints::default();
    learned_with_bp.ids.push("iron_sword_v0".into());
    assert!(
        !learned_with_bp.ids.is_empty(),
        "learned blueprint should satisfy the has-blueprint check"
    );
}

// ── F9 跨层修复：send_tutorial_coffin_pos_on_join ──────────────



// ── plan-scroll-reading-v1 P0 §8.1 #1：grant_meridian_primer_on_join tick-poll ──
