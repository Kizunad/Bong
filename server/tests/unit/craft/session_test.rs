#![allow(dead_code, unused_imports)]

use std::collections::HashMap;

use bong_server::craft::events::{
    CraftCompletedEvent, CraftFailedEvent, CraftFailureReason, CraftStartedEvent, InsightTrigger,
    UnlockEventSource,
};
use bong_server::craft::recipe::{
    CraftCategory, CraftRecipe, CraftRequirements, CraftStationKind, RecipeId, UnlockSource,
};
use bong_server::craft::registry::CraftRegistry;
use bong_server::craft::session::*;
use bong_server::craft::unlock::{RecipeUnlockState, UnlockOutcome};
use bong_server::cultivation::components::{ColorKind, Cultivation, QiColor, Realm};
use bong_server::inventory::{
    ContainerState, InventoryRevision, ItemInstance, ItemRarity, PlacedItemState, PlayerInventory,
};
use bong_server::qi_physics::ledger::{
    pending_inflow_account, QiAccountId, QiTransferReason, WorldQiAccount,
};
use bong_server::skill::components::SkillSet;
use valence::prelude::{App, Entity};

fn make_inventory(items: &[(&str, u32)]) -> PlayerInventory {
    let placed: Vec<PlacedItemState> = items
        .iter()
        .enumerate()
        .map(|(idx, (template, n))| PlacedItemState {
            row: idx as u8,
            col: 0,
            instance: ItemInstance {
                instance_id: idx as u64 + 1,
                template_id: (*template).into(),
                display_name: (*template).into(),
                grid_w: 1,
                grid_h: 1,
                weight: 1.0,
                rarity: ItemRarity::Common,
                description: String::new(),
                stack_count: *n,
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
            },
        })
        .collect();
    PlayerInventory {
        material_preparation: Default::default(),
        triggered_treasures: Vec::new(),
        revision: InventoryRevision(1),
        containers: vec![ContainerState {
            quick_access: false,
            id: "main_pack".into(),
            name: "main".into(),
            rows: 16,
            cols: 1,
            items: placed,
            owner_instance_id: None,
        }],
        equipped: HashMap::new(),
        hotbar: Default::default(),
        bone_coins: 0,
        max_weight: 100.0,
    }
}

fn simple_recipe(id: &str) -> CraftRecipe {
    CraftRecipe {
        id: RecipeId::new(id),
        category: CraftCategory::Misc,
        display_name: id.into(),
        materials: vec![("herb_a".into(), 2), ("iron_needle".into(), 3)],
        qi_cost: 5.0,
        time_ticks: 100,
        output: ("test_pill".into(), 1),
        requirements: CraftRequirements::default(),
        unlock_sources: vec![UnlockSource::Scroll {
            item_template: "scroll_x".into(),
        }],
        station: None,
    }
}

fn prepared_inventory(recipe_id: &str, items: &[(&str, u32)]) -> PlayerInventory {
    let mut inventory = make_inventory(items);
    let recipe = simple_recipe(recipe_id);
    let ids: Vec<_> = inventory.containers[0]
        .items
        .iter()
        .map(|entry| entry.instance.instance_id)
        .collect();
    for id in ids {
        bong_server::craft::preparation::stage_material(&mut inventory, &recipe, id).unwrap();
    }
    inventory
}

fn ok_deps_for_player<'a>(
    registry: &'a CraftRegistry,
    unlock: &'a RecipeUnlockState,
    inventory: &'a mut PlayerInventory,
    cultivation: &'a mut Cultivation,
    color: &'a QiColor,
    ledger: &'a mut WorldQiAccount,
    skill_set: Option<&'a SkillSet>,
) -> StartCraftDeps<'a> {
    StartCraftDeps {
        registry,
        unlock_state: unlock,
        inventory,
        cultivation,
        qi_color: color,
        ledger,
        existing_session: None,
        skill_set,
        has_nearby_workbench: true, // 默认近处有制作台（手搓配方不需要）
    }
}

fn make_world() -> (
    CraftRegistry,
    RecipeUnlockState,
    Cultivation,
    QiColor,
    WorldQiAccount,
) {
    let mut registry = CraftRegistry::new();
    registry.register(simple_recipe("a")).unwrap();
    let mut unlock = RecipeUnlockState::new();
    unlock.unlock("offline:Alice", RecipeId::new("a"));
    let cultivation = Cultivation {
        qi_current: 50.0,
        qi_max: 80.0,
        ..Default::default()
    };
    let color = QiColor::default();
    let ledger = WorldQiAccount::default();
    (registry, unlock, cultivation, color, ledger)
}

fn caster_entity() -> Entity {
    // 在测试 App 内 spawn empty 拿 entity id（其他 fn 不需要真 App）
    let mut app = App::new();
    app.world_mut().spawn_empty().id()
}

// ============= 材料统计 =============

#[test]
fn count_template_aggregates_containers_and_hotbar() {
    let mut inv = make_inventory(&[("herb_a", 5), ("herb_a", 3), ("iron_needle", 2)]);
    // hotbar 内再放 4 个 herb_a
    inv.hotbar[0] = Some(ItemInstance {
        instance_id: 99,
        template_id: "herb_a".into(),
        display_name: "herb_a".into(),
        grid_w: 1,
        grid_h: 1,
        weight: 1.0,
        rarity: ItemRarity::Common,
        description: String::new(),
        stack_count: 4,
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
    });
    assert_eq!(count_template_in_inventory(&inv, "herb_a"), 5 + 3 + 4);
    assert_eq!(count_template_in_inventory(&inv, "iron_needle"), 2);
    assert_eq!(count_template_in_inventory(&inv, "absent"), 0);
}

#[test]
fn consume_materials_drains_in_order_and_drops_empty_stacks() {
    let mut inv = make_inventory(&[("herb_a", 5), ("herb_a", 3)]);
    consume_materials_from_inventory(&mut inv, "herb_a", 6).unwrap();
    // 第一个 stack 被吃完移除，第二个剩 2
    let remaining: Vec<_> = inv.containers[0]
        .items
        .iter()
        .map(|p| p.instance.stack_count)
        .collect();
    assert_eq!(remaining, vec![2]);
}

#[test]
fn consume_materials_bumps_revision_when_inventory_changes() {
    let mut inv = make_inventory(&[("herb_a", 5)]);
    let before = inv.revision;

    consume_materials_from_inventory(&mut inv, "herb_a", 2).unwrap();

    assert!(
        inv.revision.0 > before.0,
        "craft material consumption must bump inventory revision so client snapshots cannot look stale"
    );
}

#[test]
fn consume_materials_zero_count_is_noop() {
    let mut inv = make_inventory(&[("herb_a", 5)]);
    let before = inv.revision;

    consume_materials_from_inventory(&mut inv, "herb_a", 0).unwrap();

    assert_eq!(count_template_in_inventory(&inv, "herb_a"), 5);
    assert_eq!(
        inv.revision, before,
        "zero-count material consumption should not bump revision"
    );
}

#[test]
fn consume_materials_returns_err_on_underflow() {
    let mut inv = make_inventory(&[("herb_a", 1)]);
    let err = consume_materials_from_inventory(&mut inv, "herb_a", 5).unwrap_err();
    assert_eq!(err.template_id, "herb_a");
    assert_eq!(err.need, 4);
}

// ============= start_craft =============

#[test]
fn unplaced_stock_cannot_start_or_spend_qi() {
    let (registry, unlock, mut cult, color, mut ledger) = make_world();
    let mut inventory = make_inventory(&[("herb_a", 5), ("iron_needle", 5)]);
    let before = serde_json::to_value(&inventory).unwrap();
    let result = start_craft(
        StartCraftRequest {
            caster: caster_entity(),
            player_id: "offline:Alice",
            recipe_id: &RecipeId::new("a"),
            current_tick: 0,
            quantity: 1,
        },
        ok_deps_for_player(
            &registry,
            &unlock,
            &mut inventory,
            &mut cult,
            &color,
            &mut ledger,
            None,
        ),
    );
    assert!(
        matches!(result, Err(StartCraftError::MissingMaterials(_))),
        "背包有材料不等于制作区已备料"
    );
    assert_eq!(serde_json::to_value(&inventory).unwrap(), before);
    assert_eq!(cult.qi_current, 50.0);
    assert!(ledger.transfers().is_empty());
}

#[test]
fn preparation_roundtrip_returns_exact_instance_and_counts_carried_weight() {
    use bong_server::craft::preparation::{return_materials, stage_material};
    use bong_server::inventory::{calculate_current_weight, ItemRegistry};
    let mut inventory = make_inventory(&[("herb_a", 5)]);
    inventory.containers[0].items[0].instance.durability = 0.43;
    inventory.containers[0].items[0].instance.forge_quality = Some(0.72);
    let original = inventory.containers[0].items[0].clone();
    let weight = calculate_current_weight(&inventory);
    stage_material(
        &mut inventory,
        &simple_recipe("a"),
        original.instance.instance_id,
    )
    .unwrap();
    assert!(
        inventory.containers[0].items.is_empty(),
        "移入必须真实离开背包"
    );
    assert_eq!(
        calculate_current_weight(&inventory),
        weight,
        "材料区不免除携带负重"
    );
    assert!(
        stage_material(
            &mut inventory,
            &simple_recipe("a"),
            original.instance.instance_id
        )
        .is_err(),
        "重复移入不可复制物品"
    );
    let json = serde_json::to_string(&inventory).unwrap();
    let mut loaded: PlayerInventory = serde_json::from_str(&json).unwrap();
    let drops = return_materials(
        &mut loaded,
        &ItemRegistry::from_map(HashMap::new()),
        None,
        [0.0, 64.0, 0.0],
        Default::default(),
    )
    .unwrap();
    assert!(drops.is_empty());
    assert_eq!(
        loaded.containers[0].items,
        vec![original],
        "重载后应返还原位置、实例和属性"
    );
    assert!(loaded.material_preparation.materials.is_empty());
}

#[test]
fn returning_to_full_inventory_drops_original_instance_without_loss() {
    use bong_server::craft::preparation::{return_materials, stage_material};
    use bong_server::inventory::ItemRegistry;
    let mut inventory = make_inventory(&[("herb_a", 5)]);
    inventory.containers[0].rows = 1;
    let original = inventory.containers[0].items[0].instance.clone();
    stage_material(&mut inventory, &simple_recipe("a"), original.instance_id).unwrap();
    let mut obstacle = make_inventory(&[("iron_needle", 1)]).containers[0]
        .items
        .remove(0);
    obstacle.instance.instance_id = 20;
    inventory.containers[0].items.push(obstacle);
    let drops = return_materials(
        &mut inventory,
        &ItemRegistry::from_map(HashMap::new()),
        None,
        [1.0, 65.0, 2.0],
        Default::default(),
    )
    .unwrap();
    assert_eq!(drops.len(), 1);
    assert_eq!(
        drops[0].item, original,
        "满包兜底不得按模板重新生成或损耗物品"
    );
    assert_eq!(drops[0].world_pos, [1.0, 65.0, 2.0]);
    assert!(inventory.material_preparation.materials.is_empty());
}

#[test]
fn start_craft_happy_path_writes_ledger_and_session() {
    let (registry, unlock, mut cult, color, mut ledger) = make_world();
    let mut inv = prepared_inventory("a", &[("herb_a", 5), ("iron_needle", 5)]);
    let caster = caster_entity();

    let result = start_craft(
        StartCraftRequest {
            caster,
            player_id: "offline:Alice",
            recipe_id: &RecipeId::new("a"),
            current_tick: 1000,
            quantity: 1,
        },
        ok_deps_for_player(
            &registry,
            &unlock,
            &mut inv,
            &mut cult,
            &color,
            &mut ledger,
            None,
        ),
    )
    .unwrap();

    // session 形态
    assert_eq!(result.session.recipe_id.as_str(), "a");
    assert_eq!(result.session.started_at_tick, 1000);
    assert_eq!(result.session.remaining_ticks, 100);
    assert_eq!(result.session.qi_paid, 5.0);

    // 材料扣减
    assert_eq!(inv.material_preparation.count("a", "herb_a"), 3);
    assert_eq!(inv.material_preparation.count("a", "iron_needle"), 2);

    // qi 守恒：cultivation 扣 5，ledger 待分配池余额 +5
    assert_eq!(
        cult.qi_current, 45.0,
        "制作预付 5 点真元后玩家应从 50 降至 45"
    );
    let pending_balance = ledger.balance(&pending_inflow_account());
    assert_eq!(
        pending_balance, 5.0,
        "制作预付的 5 点真元应完整进入待分配池"
    );

    // 守恒律观察：qi_paid 与 ledger transfer 等同
    assert_eq!(result.session.qi_paid, 5.0);
    assert_eq!(result.event.qi_paid, 5.0);
}

#[test]
fn start_craft_batch_reserves_all_materials_and_qi_upfront() {
    let (registry, unlock, mut cult, color, mut ledger) = make_world();
    let mut inv = prepared_inventory("a", &[("herb_a", 8), ("iron_needle", 10)]);
    let result = start_craft(
        StartCraftRequest {
            caster: caster_entity(),
            player_id: "offline:Alice",
            recipe_id: &RecipeId::new("a"),
            current_tick: 1000,
            quantity: 3,
        },
        ok_deps_for_player(
            &registry,
            &unlock,
            &mut inv,
            &mut cult,
            &color,
            &mut ledger,
            None,
        ),
    )
    .unwrap();

    assert_eq!(result.session.quantity_total, 3);
    assert_eq!(result.session.completed_count, 0);
    assert_eq!(result.session.qi_paid, 15.0);
    assert_eq!(inv.material_preparation.count("a", "herb_a"), 2);
    assert_eq!(inv.material_preparation.count("a", "iron_needle"), 1);
    assert_eq!(cult.qi_current, 35.0);
    assert_eq!(ledger.balance(&pending_inflow_account()), 15.0);
}

#[test]
fn start_craft_rejects_quantity_above_limit_before_cost_checks() {
    let (registry, unlock, mut cult, color, mut ledger) = make_world();
    let mut inv = prepared_inventory("a", &[("herb_a", 8), ("iron_needle", 10)]);
    let err = start_craft(
        StartCraftRequest {
            caster: caster_entity(),
            player_id: "offline:Alice",
            recipe_id: &RecipeId::new("a"),
            current_tick: 1000,
            quantity: MAX_CRAFT_QUANTITY + 1,
        },
        ok_deps_for_player(
            &registry,
            &unlock,
            &mut inv,
            &mut cult,
            &color,
            &mut ledger,
            None,
        ),
    )
    .unwrap_err();

    assert_eq!(
        err,
        StartCraftError::QuantityTooLarge {
            requested: MAX_CRAFT_QUANTITY + 1,
            max: MAX_CRAFT_QUANTITY,
        }
    );
    assert_eq!(inv.material_preparation.count("a", "herb_a"), 8);
    assert_eq!(cult.qi_current, 50.0);
}

#[test]
fn start_craft_rejects_unknown_recipe() {
    let (registry, unlock, mut cult, color, mut ledger) = make_world();
    let mut inv = prepared_inventory("missing", &[("herb_a", 5), ("iron_needle", 5)]);
    let err = start_craft(
        StartCraftRequest {
            caster: caster_entity(),
            player_id: "offline:Alice",
            recipe_id: &RecipeId::new("missing"),
            current_tick: 0,
            quantity: 1,
        },
        ok_deps_for_player(
            &registry,
            &unlock,
            &mut inv,
            &mut cult,
            &color,
            &mut ledger,
            None,
        ),
    )
    .unwrap_err();
    assert!(matches!(err, StartCraftError::UnknownRecipe(_)));
}

#[test]
fn start_craft_rejects_locked_recipe() {
    let (registry, _unlock, mut cult, color, mut ledger) = make_world();
    let unlock = RecipeUnlockState::new(); // 空 unlock state
    let mut inv = prepared_inventory("a", &[("herb_a", 5), ("iron_needle", 5)]);
    let err = start_craft(
        StartCraftRequest {
            caster: caster_entity(),
            player_id: "offline:Alice",
            recipe_id: &RecipeId::new("a"),
            current_tick: 0,
            quantity: 1,
        },
        ok_deps_for_player(
            &registry,
            &unlock,
            &mut inv,
            &mut cult,
            &color,
            &mut ledger,
            None,
        ),
    )
    .unwrap_err();
    assert!(matches!(err, StartCraftError::NotUnlocked(_)));
    // 失败时无副作用：材料仍在
    assert_eq!(inv.material_preparation.count("a", "herb_a"), 5);
}

#[test]
fn start_craft_rejects_when_session_already_exists() {
    let (registry, unlock, mut cult, color, mut ledger) = make_world();
    let mut inv = prepared_inventory("a", &[("herb_a", 5), ("iron_needle", 5)]);
    let existing = CraftSession {
        recipe_id: RecipeId::new("a"),
        started_at_tick: 0,
        remaining_ticks: 50,
        total_ticks: 100,
        owner_player_id: "offline:Alice".into(),
        qi_paid: 5.0,
        quantity_total: 1,
        completed_count: 0,
    };
    let mut deps = ok_deps_for_player(
        &registry,
        &unlock,
        &mut inv,
        &mut cult,
        &color,
        &mut ledger,
        None,
    );
    deps.existing_session = Some(&existing);
    let err = start_craft(
        StartCraftRequest {
            caster: caster_entity(),
            player_id: "offline:Alice",
            recipe_id: &RecipeId::new("a"),
            current_tick: 0,
            quantity: 1,
        },
        deps,
    )
    .unwrap_err();
    assert_eq!(err, StartCraftError::AlreadyHasSession);
}

#[test]
fn start_craft_rejects_missing_materials_with_full_deficit_list() {
    let (registry, unlock, mut cult, color, mut ledger) = make_world();
    let mut inv = prepared_inventory("a", &[("herb_a", 1)]); // need 2 + iron_needle 3
    let err = start_craft(
        StartCraftRequest {
            caster: caster_entity(),
            player_id: "offline:Alice",
            recipe_id: &RecipeId::new("a"),
            current_tick: 0,
            quantity: 1,
        },
        ok_deps_for_player(
            &registry,
            &unlock,
            &mut inv,
            &mut cult,
            &color,
            &mut ledger,
            None,
        ),
    )
    .unwrap_err();
    match err {
        StartCraftError::MissingMaterials(deficits) => {
            assert_eq!(deficits.len(), 2);
            let herb = deficits.iter().find(|d| d.template_id == "herb_a").unwrap();
            assert_eq!(herb.have, 1);
            assert_eq!(herb.need, 2);
            let iron = deficits
                .iter()
                .find(|d| d.template_id == "iron_needle")
                .unwrap();
            assert_eq!(iron.have, 0);
            assert_eq!(iron.need, 3);
        }
        other => panic!("expected MissingMaterials, got {other:?}"),
    }
    // 失败时不扣材料
    assert_eq!(inv.material_preparation.count("a", "herb_a"), 1);
}

#[test]
fn start_craft_rejects_insufficient_qi() {
    let (registry, unlock, mut _ignored, color, mut ledger) = make_world();
    let mut cult = Cultivation {
        qi_current: 2.0, // recipe 要 5
        qi_max: 80.0,
        ..Default::default()
    };
    let mut inv = prepared_inventory("a", &[("herb_a", 5), ("iron_needle", 5)]);
    let err = start_craft(
        StartCraftRequest {
            caster: caster_entity(),
            player_id: "offline:Alice",
            recipe_id: &RecipeId::new("a"),
            current_tick: 0,
            quantity: 1,
        },
        ok_deps_for_player(
            &registry,
            &unlock,
            &mut inv,
            &mut cult,
            &color,
            &mut ledger,
            None,
        ),
    )
    .unwrap_err();
    assert!(matches!(
        err,
        StartCraftError::InsufficientQi {
            have: 2.0,
            need: 5.0
        }
    ));
    // 失败时不扣材料
    assert_eq!(inv.material_preparation.count("a", "herb_a"), 5);
}

#[test]
fn start_craft_rejects_realm_too_low() {
    let mut registry = CraftRegistry::new();
    let mut recipe = simple_recipe("a");
    recipe.requirements.realm_min = Some(Realm::Solidify);
    registry.register(recipe).unwrap();

    let mut unlock = RecipeUnlockState::new();
    unlock.unlock("offline:Alice", RecipeId::new("a"));
    let mut cult = Cultivation {
        qi_current: 50.0,
        qi_max: 80.0,
        realm: Realm::Awaken,
        ..Default::default()
    };
    let color = QiColor::default();
    let mut ledger = WorldQiAccount::default();
    let mut inv = prepared_inventory("a", &[("herb_a", 5), ("iron_needle", 5)]);

    let err = start_craft(
        StartCraftRequest {
            caster: caster_entity(),
            player_id: "offline:Alice",
            recipe_id: &RecipeId::new("a"),
            current_tick: 0,
            quantity: 1,
        },
        ok_deps_for_player(
            &registry,
            &unlock,
            &mut inv,
            &mut cult,
            &color,
            &mut ledger,
            None,
        ),
    )
    .unwrap_err();
    assert!(matches!(
        err,
        StartCraftError::RealmTooLow {
            required: Realm::Solidify,
            current: Realm::Awaken
        }
    ));
}

#[test]
fn start_craft_rejects_qi_color_mismatch() {
    let mut registry = CraftRegistry::new();
    let mut recipe = simple_recipe("a");
    recipe.requirements.qi_color_min = Some((ColorKind::Insidious, 0.05));
    registry.register(recipe).unwrap();
    let mut unlock = RecipeUnlockState::new();
    unlock.unlock("offline:Alice", RecipeId::new("a"));
    let mut cult = Cultivation {
        qi_current: 50.0,
        qi_max: 80.0,
        ..Default::default()
    };
    let color = QiColor {
        main: ColorKind::Mellow, // 不是 Insidious
        ..Default::default()
    };
    let mut ledger = WorldQiAccount::default();
    let mut inv = prepared_inventory("a", &[("herb_a", 5), ("iron_needle", 5)]);
    let err = start_craft(
        StartCraftRequest {
            caster: caster_entity(),
            player_id: "offline:Alice",
            recipe_id: &RecipeId::new("a"),
            current_tick: 0,
            quantity: 1,
        },
        ok_deps_for_player(
            &registry,
            &unlock,
            &mut inv,
            &mut cult,
            &color,
            &mut ledger,
            None,
        ),
    )
    .unwrap_err();
    assert!(matches!(
        err,
        StartCraftError::QiColorMismatch {
            required: ColorKind::Insidious,
            current: ColorKind::Mellow
        }
    ));
}

#[test]
fn start_craft_zero_qi_recipe_skips_ledger_transfer() {
    let mut registry = CraftRegistry::new();
    let mut recipe = simple_recipe("a");
    recipe.qi_cost = 0.0;
    registry.register(recipe).unwrap();
    let mut unlock = RecipeUnlockState::new();
    unlock.unlock("offline:Alice", RecipeId::new("a"));
    let mut cult = Cultivation {
        qi_current: 0.0, // 零 qi 也能起手
        qi_max: 80.0,
        ..Default::default()
    };
    let color = QiColor::default();
    let mut ledger = WorldQiAccount::default();
    let mut inv = prepared_inventory("a", &[("herb_a", 5), ("iron_needle", 5)]);
    let result = start_craft(
        StartCraftRequest {
            caster: caster_entity(),
            player_id: "offline:Alice",
            recipe_id: &RecipeId::new("a"),
            current_tick: 0,
            quantity: 1,
        },
        ok_deps_for_player(
            &registry,
            &unlock,
            &mut inv,
            &mut cult,
            &color,
            &mut ledger,
            None,
        ),
    )
    .unwrap();
    assert_eq!(result.session.qi_paid, 0.0);
    // ledger 无 transfer 落地
    assert_eq!(ledger.transfers().len(), 0);
    assert_eq!(cult.qi_current, 0.0);
}

// ============= tick_session =============

#[test]
fn tick_session_decrements_remaining() {
    let mut session = CraftSession {
        recipe_id: RecipeId::new("a"),
        started_at_tick: 0,
        remaining_ticks: 100,
        total_ticks: 100,
        owner_player_id: "offline:Alice".into(),
        qi_paid: 5.0,
        quantity_total: 1,
        completed_count: 0,
    };
    let done = tick_session(&mut session, 30);
    assert!(!done);
    assert_eq!(session.remaining_ticks, 70);
}

#[test]
fn tick_session_completes_at_zero() {
    let mut session = CraftSession {
        recipe_id: RecipeId::new("a"),
        started_at_tick: 0,
        remaining_ticks: 5,
        total_ticks: 100,
        owner_player_id: "offline:Alice".into(),
        qi_paid: 0.0,
        quantity_total: 1,
        completed_count: 0,
    };
    let done = tick_session(&mut session, 5);
    assert!(done);
    assert_eq!(session.remaining_ticks, 0);
}

#[test]
fn tick_session_overshoot_clamps_to_zero() {
    let mut session = CraftSession {
        recipe_id: RecipeId::new("a"),
        started_at_tick: 0,
        remaining_ticks: 5,
        total_ticks: 100,
        owner_player_id: "offline:Alice".into(),
        qi_paid: 0.0,
        quantity_total: 1,
        completed_count: 0,
    };
    let done = tick_session(&mut session, 100);
    assert!(done);
    assert_eq!(session.remaining_ticks, 0);
}

#[test]
fn tick_session_with_zero_amount_is_noop() {
    let mut session = CraftSession {
        recipe_id: RecipeId::new("a"),
        started_at_tick: 0,
        remaining_ticks: 50,
        total_ticks: 100,
        owner_player_id: "offline:Alice".into(),
        qi_paid: 0.0,
        quantity_total: 1,
        completed_count: 0,
    };
    let done = tick_session(&mut session, 0);
    assert!(!done);
    assert_eq!(session.remaining_ticks, 50);
}

#[test]
fn tick_session_already_complete_is_idempotent() {
    let mut session = CraftSession {
        recipe_id: RecipeId::new("a"),
        started_at_tick: 0,
        remaining_ticks: 0,
        total_ticks: 100,
        owner_player_id: "offline:Alice".into(),
        qi_paid: 0.0,
        quantity_total: 1,
        completed_count: 0,
    };
    let done = tick_session(&mut session, 50);
    assert!(done);
    assert_eq!(session.remaining_ticks, 0);
}

// ============= cancel_craft =============

#[test]
fn cancel_craft_returns_70pct_floor() {
    let recipe = simple_recipe("a"); // herb_a×2, iron_needle×3
    let session = CraftSession {
        recipe_id: RecipeId::new("a"),
        started_at_tick: 0,
        remaining_ticks: 50,
        total_ticks: 100,
        owner_player_id: "offline:Alice".into(),
        qi_paid: 5.0,
        quantity_total: 1,
        completed_count: 0,
    };
    let outcome = cancel_craft(
        &session,
        &recipe,
        caster_entity(),
        CraftFailureReason::PlayerCancelled,
    );
    // herb_a: floor(2 * 0.7) = 1
    // iron_needle: floor(3 * 0.7) = 2
    let map: HashMap<&str, u32> = outcome
        .refund_manifest
        .iter()
        .map(|(t, n)| (t.as_str(), *n))
        .collect();
    assert_eq!(map.get("herb_a"), Some(&1));
    assert_eq!(map.get("iron_needle"), Some(&2));
    assert_eq!(outcome.event.material_returned, 3);
    assert_eq!(outcome.event.qi_refunded, 0.0); // §5 决策门 #3
}

#[test]
fn cancel_craft_filters_zero_refund_entries() {
    let mut recipe = simple_recipe("a");
    recipe.materials = vec![("herb_a".into(), 1)]; // floor(1 * 0.7) = 0
    let session = CraftSession {
        recipe_id: RecipeId::new("a"),
        started_at_tick: 0,
        remaining_ticks: 50,
        total_ticks: 100,
        owner_player_id: "offline:Alice".into(),
        qi_paid: 0.0,
        quantity_total: 1,
        completed_count: 0,
    };
    let outcome = cancel_craft(
        &session,
        &recipe,
        caster_entity(),
        CraftFailureReason::PlayerCancelled,
    );
    assert!(outcome.refund_manifest.is_empty());
    assert_eq!(outcome.event.material_returned, 0);
}

#[test]
fn cancel_craft_batch_refunds_unfinished_quantity() {
    let recipe = simple_recipe("a"); // herb_a×2, iron_needle×3
    let session = CraftSession {
        recipe_id: RecipeId::new("a"),
        started_at_tick: 0,
        remaining_ticks: 50,
        total_ticks: 100,
        owner_player_id: "offline:Alice".into(),
        qi_paid: 15.0,
        quantity_total: 3,
        completed_count: 1,
    };
    let outcome = cancel_craft(
        &session,
        &recipe,
        caster_entity(),
        CraftFailureReason::PlayerCancelled,
    );
    let map: HashMap<&str, u32> = outcome
        .refund_manifest
        .iter()
        .map(|(t, n)| (t.as_str(), *n))
        .collect();
    // 剩余 2 件：herb_a floor(2*2*0.7)=2；iron_needle floor(3*2*0.7)=4
    assert_eq!(map.get("herb_a"), Some(&2));
    assert_eq!(map.get("iron_needle"), Some(&4));
    assert_eq!(outcome.event.material_returned, 6);
}

#[test]
fn cancel_craft_propagates_player_died_reason() {
    let recipe = simple_recipe("a");
    let session = CraftSession {
        recipe_id: RecipeId::new("a"),
        started_at_tick: 0,
        remaining_ticks: 50,
        total_ticks: 100,
        owner_player_id: "offline:Alice".into(),
        qi_paid: 5.0,
        quantity_total: 1,
        completed_count: 0,
    };
    let outcome = cancel_craft(
        &session,
        &recipe,
        caster_entity(),
        CraftFailureReason::PlayerDied,
    );
    assert_eq!(outcome.event.reason, CraftFailureReason::PlayerDied);
}

#[test]
fn cancel_craft_propagates_internal_error_reason() {
    let recipe = simple_recipe("a");
    let session = CraftSession {
        recipe_id: RecipeId::new("a"),
        started_at_tick: 0,
        remaining_ticks: 50,
        total_ticks: 100,
        owner_player_id: "offline:Alice".into(),
        qi_paid: 0.0,
        quantity_total: 1,
        completed_count: 0,
    };
    let outcome = cancel_craft(
        &session,
        &recipe,
        caster_entity(),
        CraftFailureReason::InternalError,
    );
    assert_eq!(outcome.event.reason, CraftFailureReason::InternalError);
}

// ============= finalize_craft =============

#[test]
fn finalize_craft_returns_output_manifest() {
    let mut recipe = simple_recipe("a");
    recipe.output = ("eclipse_needle_iron".into(), 3);
    let session = CraftSession {
        recipe_id: RecipeId::new("a"),
        started_at_tick: 100,
        remaining_ticks: 0,
        total_ticks: 100,
        owner_player_id: "offline:Alice".into(),
        qi_paid: 5.0,
        quantity_total: 1,
        completed_count: 0,
    };
    let outcome = finalize_craft(&session, &recipe, caster_entity(), 200);
    assert_eq!(outcome.event.completed_at_tick, 200);
    assert_eq!(outcome.event.output_template, "eclipse_needle_iron");
    assert_eq!(outcome.event.output_count, 3);
    assert_eq!(outcome.output_manifest, ("eclipse_needle_iron".into(), 3));
}

// ============= 守恒律端到端 =============

#[test]
fn start_craft_ledger_amount_matches_session_qi_paid() {
    // 守恒律观察值断言 — qi_paid 必须等同 ledger transfer amount
    let (registry, unlock, mut cult, color, mut ledger) = make_world();
    let mut inv = prepared_inventory("a", &[("herb_a", 5), ("iron_needle", 5)]);

    let result = start_craft(
        StartCraftRequest {
            caster: caster_entity(),
            player_id: "offline:Alice",
            recipe_id: &RecipeId::new("a"),
            current_tick: 0,
            quantity: 1,
        },
        ok_deps_for_player(
            &registry,
            &unlock,
            &mut inv,
            &mut cult,
            &color,
            &mut ledger,
            None,
        ),
    )
    .unwrap();

    // 找最近一次 transfer
    let last_transfer = ledger
        .transfers()
        .last()
        .expect("ledger should have transfer");
    assert_eq!(last_transfer.amount, result.session.qi_paid);
    assert_eq!(last_transfer.reason, QiTransferReason::Crafting);
    assert_eq!(last_transfer.from, QiAccountId::player("offline:Alice"));
    assert_eq!(last_transfer.to, pending_inflow_account());
}

#[test]
fn start_craft_unlock_via_insight_then_run() {
    // 集成：先用 insight 解锁，然后 start_craft 跑通
    let mut registry = CraftRegistry::new();
    let mut recipe = simple_recipe("a");
    recipe.unlock_sources = vec![UnlockSource::Insight {
        trigger: InsightTrigger::Breakthrough,
    }];
    registry.register(recipe).unwrap();
    let mut unlock = RecipeUnlockState::new();
    let recipe_ref = registry.get(&RecipeId::new("a")).unwrap();
    let outcome = bong_server::craft::unlock::unlock_via_insight(
        &mut unlock,
        "offline:Alice",
        recipe_ref,
        InsightTrigger::Breakthrough,
    );
    assert!(matches!(
        outcome,
        UnlockOutcome::Newly {
            source: UnlockEventSource::Insight {
                trigger: InsightTrigger::Breakthrough
            }
        }
    ));

    let mut cult = Cultivation {
        qi_current: 50.0,
        qi_max: 80.0,
        ..Default::default()
    };
    let color = QiColor::default();
    let mut ledger = WorldQiAccount::default();
    let mut inv = prepared_inventory("a", &[("herb_a", 5), ("iron_needle", 5)]);
    let success = start_craft(
        StartCraftRequest {
            caster: caster_entity(),
            player_id: "offline:Alice",
            recipe_id: &RecipeId::new("a"),
            current_tick: 0,
            quantity: 1,
        },
        ok_deps_for_player(
            &registry,
            &unlock,
            &mut inv,
            &mut cult,
            &color,
            &mut ledger,
            None,
        ),
    )
    .unwrap();
    assert_eq!(success.session.qi_paid, 5.0);
}

// ============= 守恒 / ECS 外部真元源不变量 =============

#[test]
fn start_craft_moves_external_player_qi_without_leaving_player_mirror() {
    let (registry, unlock, mut cult, color, mut ledger) = make_world();
    let mut inv = prepared_inventory("a", &[("herb_a", 5), ("iron_needle", 5)]);
    let qi_before = cult.qi_current;
    let ledger_before = ledger.total();
    let pending_before = ledger.balance(&pending_inflow_account());
    let player_account = QiAccountId::player("offline:Alice");
    assert!(!ledger.has_account(&player_account));

    start_craft(
        StartCraftRequest {
            caster: caster_entity(),
            player_id: "offline:Alice",
            recipe_id: &RecipeId::new("a"),
            current_tick: 0,
            quantity: 1,
        },
        ok_deps_for_player(
            &registry,
            &unlock,
            &mut inv,
            &mut cult,
            &color,
            &mut ledger,
            None,
        ),
    )
    .unwrap();

    // recipe.qi_cost = 5.0（make_world / simple_recipe）
    let qi_paid = 5.0_f64;
    assert_eq!(
        cult.qi_current,
        qi_before - qi_paid,
        "制作应从 ECS 玩家真元中精确扣除 qi_paid"
    );
    assert!(
        !ledger.has_account(&player_account),
        "玩家真元权威在 ECS，制作转账后不得留下长期 player ledger 镜像"
    );
    let pending_after = ledger.balance(&pending_inflow_account());
    assert_eq!(
        pending_after,
        pending_before + qi_paid,
        "pending inflow account must gain exactly qi_cost"
    );
    let observed_before = qi_before + ledger_before;
    let observed_after = cult.qi_current + ledger.total();
    assert!(
        (observed_before - observed_after).abs() < 1e-9,
        "ECS player qi + ledger before {observed_before} must equal after {observed_after}"
    );
}

#[test]
fn start_craft_conserves_external_player_qi_plus_ledger_total() {
    let (registry, unlock, mut cult, color, mut ledger) = make_world();
    let mut inv = prepared_inventory("a", &[("herb_a", 5), ("iron_needle", 5)]);
    let observed_before = cult.qi_current + ledger.total();

    start_craft(
        StartCraftRequest {
            caster: caster_entity(),
            player_id: "offline:Alice",
            recipe_id: &RecipeId::new("a"),
            current_tick: 0,
            quantity: 1,
        },
        ok_deps_for_player(
            &registry,
            &unlock,
            &mut inv,
            &mut cult,
            &color,
            &mut ledger,
            None,
        ),
    )
    .unwrap();

    let observed_after = cult.qi_current + ledger.total();
    assert!(
        (observed_before - observed_after).abs() < 1e-9,
        "ECS player qi + ledger before {observed_before} must equal after {observed_after}"
    );
}

#[test]
fn start_craft_works_without_player_ledger_sync() {
    let mut registry = CraftRegistry::new();
    registry.register(simple_recipe("a")).unwrap();
    let mut unlock = RecipeUnlockState::new();
    unlock.unlock("offline:Alice", RecipeId::new("a"));
    let mut cult = Cultivation {
        qi_current: 50.0,
        qi_max: 80.0,
        ..Default::default()
    };
    let color = QiColor::default();
    let mut ledger = WorldQiAccount::default();
    let mut inv = prepared_inventory("a", &[("herb_a", 5), ("iron_needle", 5)]);

    start_craft(
        StartCraftRequest {
            caster: caster_entity(),
            player_id: "offline:Alice",
            recipe_id: &RecipeId::new("a"),
            current_tick: 0,
            quantity: 1,
        },
        ok_deps_for_player(
            &registry,
            &unlock,
            &mut inv,
            &mut cult,
            &color,
            &mut ledger,
            None,
        ),
    )
    .expect("生产态没有 player ledger 镜像时也必须能制作");
    assert_eq!(
        cult.qi_current, 45.0,
        "无 player ledger 镜像的生产态制作仍应从 ECS 扣除 5 点真元"
    );
    assert_eq!(
        ledger.balance(&pending_inflow_account()),
        5.0,
        "无 player ledger 镜像时制作真元仍应完整进入 pending"
    );
    assert!(!ledger.has_account(&QiAccountId::player("offline:Alice")));
}

#[test]
fn start_craft_preserves_preexisting_player_ledger_balance() {
    let (registry, unlock, mut cult, color, mut ledger) = make_world();
    let mut inv = prepared_inventory("a", &[("herb_a", 5), ("iron_needle", 5)]);
    let player_account = QiAccountId::player("offline:Alice");
    ledger.set_balance(player_account.clone(), 7.0).unwrap();
    let observed_before = cult.qi_current + ledger.total();

    start_craft(
        StartCraftRequest {
            caster: caster_entity(),
            player_id: "offline:Alice",
            recipe_id: &RecipeId::new("a"),
            current_tick: 0,
            quantity: 1,
        },
        ok_deps_for_player(
            &registry,
            &unlock,
            &mut inv,
            &mut cult,
            &color,
            &mut ledger,
            None,
        ),
    )
    .unwrap();

    assert_eq!(
        ledger.balance(&player_account),
        7.0,
        "临时 source 影子转账后必须精确恢复既有 player ledger 余额"
    );
    assert_eq!(
        ledger.balance(&pending_inflow_account()),
        5.0,
        "制作真元应等额进入 pending"
    );
    assert_eq!(
        cult.qi_current, 45.0,
        "制作应从 ECS 玩家真元中精确扣除 5 点"
    );
    assert!(
        (cult.qi_current + ledger.total() - observed_before).abs() < 1e-9,
        "存在既有 player ledger 余额时 ECS 真元与 ledger 总量仍必须守恒"
    );
}

#[test]
fn start_craft_ledger_failure_keeps_external_qi_and_materials_unchanged() {
    let mut registry = CraftRegistry::new();
    let mut recipe = simple_recipe("a");
    recipe.qi_cost = f64::MAX;
    registry.register(recipe).unwrap();
    let mut unlock = RecipeUnlockState::new();
    unlock.unlock("offline:Alice", RecipeId::new("a"));
    let mut cult = Cultivation {
        qi_current: f64::MAX,
        qi_max: f64::MAX,
        ..Default::default()
    };
    let color = QiColor::default();
    let mut ledger = WorldQiAccount::default();
    ledger
        .set_balance(pending_inflow_account(), f64::MAX)
        .unwrap();
    let mut inv = prepared_inventory("a", &[("herb_a", 5), ("iron_needle", 5)]);

    let err = start_craft(
        StartCraftRequest {
            caster: caster_entity(),
            player_id: "offline:Alice",
            recipe_id: &RecipeId::new("a"),
            current_tick: 0,
            quantity: 1,
        },
        ok_deps_for_player(
            &registry,
            &unlock,
            &mut inv,
            &mut cult,
            &color,
            &mut ledger,
            None,
        ),
    )
    .unwrap_err();
    assert!(matches!(err, StartCraftError::LedgerError(_)));
    assert_eq!(
        cult.qi_current,
        f64::MAX,
        "pending 溢出拒绝后不得扣除 ECS 玩家真元"
    );
    assert_eq!(
        ledger.balance(&pending_inflow_account()),
        f64::MAX,
        "pending 溢出拒绝后目标余额必须保持原值"
    );
    assert!(!ledger.has_account(&QiAccountId::player("offline:Alice")));
    assert_eq!(inv.material_preparation.count("a", "herb_a"), 5);
    assert_eq!(inv.material_preparation.count("a", "iron_needle"), 5);
    assert!(ledger.transfers().is_empty());
}

#[test]
fn start_craft_rejects_empty_source_recipe_before_material_discovery() {
    // plan-craft-material-discovery：空 unlock_sources 不再"默认解锁"。
    // 即使背包里有原料、其它前置都满足，未经材料发现写入 unlock_state 前
    // start_craft 必须 reject（材料发现解锁由 craft_emit 系统在 tick 中完成）。
    let mut registry = CraftRegistry::new();
    let mut recipe = simple_recipe("default_unlocked");
    recipe.unlock_sources = vec![];
    recipe.qi_cost = 0.0;
    registry.register(recipe).unwrap();

    let unlock = RecipeUnlockState::new();
    let mut inv = prepared_inventory("default_unlocked", &[("herb_a", 5), ("iron_needle", 5)]);
    let mut cult = Cultivation {
        qi_current: 50.0,
        qi_max: 80.0,
        ..Default::default()
    };
    let color = QiColor::default();
    let mut ledger = WorldQiAccount::default();

    let err = start_craft(
        StartCraftRequest {
            caster: caster_entity(),
            player_id: "offline:Alice",
            recipe_id: &RecipeId::new("default_unlocked"),
            current_tick: 0,
            quantity: 1,
        },
        ok_deps_for_player(
            &registry,
            &unlock,
            &mut inv,
            &mut cult,
            &color,
            &mut ledger,
            None,
        ),
    )
    .expect_err("empty-source recipe must be locked until material-discovery unlock");
    assert!(
        matches!(err, StartCraftError::NotUnlocked(_)),
        "期望 NotUnlocked（空源配方需先经材料发现解锁），实际={err:?}"
    );
    // reject 不应扣材料
    assert_eq!(
        inv.material_preparation.count("default_unlocked", "herb_a"),
        5
    );
}

#[test]
fn start_craft_allows_empty_source_recipe_after_material_unlock() {
    // 材料发现解锁后（unlock_state 已写入），空源配方应正常可造。
    let mut registry = CraftRegistry::new();
    let mut recipe = simple_recipe("default_unlocked");
    recipe.unlock_sources = vec![];
    recipe.qi_cost = 0.0;
    registry.register(recipe).unwrap();

    let mut unlock = RecipeUnlockState::new();
    // 模拟 apply_material_discovery_unlock 已把该配方解锁
    unlock.unlock("offline:Alice", RecipeId::new("default_unlocked"));
    let mut inv = prepared_inventory("default_unlocked", &[("herb_a", 5), ("iron_needle", 5)]);
    let mut cult = Cultivation {
        qi_current: 50.0,
        qi_max: 80.0,
        ..Default::default()
    };
    let color = QiColor::default();
    let mut ledger = WorldQiAccount::default();

    let result = start_craft(
        StartCraftRequest {
            caster: caster_entity(),
            player_id: "offline:Alice",
            recipe_id: &RecipeId::new("default_unlocked"),
            current_tick: 0,
            quantity: 1,
        },
        ok_deps_for_player(
            &registry,
            &unlock,
            &mut inv,
            &mut cult,
            &color,
            &mut ledger,
            None,
        ),
    );
    assert!(result.is_ok(), "材料发现解锁后空源配方应可造: {result:?}");
}

// ============= station validation =============

#[test]
fn start_craft_handcraft_passes_without_nearby_workbench() {
    // station: None = 手搓配方，不需要制作台
    let mut registry = CraftRegistry::new();
    let mut recipe = simple_recipe("handcraft");
    recipe.station = None;
    recipe.qi_cost = 0.0;
    registry.register(recipe).unwrap();

    let mut unlock = RecipeUnlockState::new();
    unlock.unlock("offline:Alice", RecipeId::new("handcraft"));
    let mut inv = prepared_inventory("handcraft", &[("herb_a", 5), ("iron_needle", 5)]);
    let mut cult = Cultivation {
        qi_current: 50.0,
        qi_max: 80.0,
        ..Default::default()
    };
    let color = QiColor::default();
    let mut ledger = WorldQiAccount::default();

    let result = start_craft(
        StartCraftRequest {
            caster: caster_entity(),
            player_id: "offline:Alice",
            recipe_id: &RecipeId::new("handcraft"),
            current_tick: 0,
            quantity: 1,
        },
        StartCraftDeps {
            registry: &registry,
            unlock_state: &unlock,
            inventory: &mut inv,
            cultivation: &mut cult,
            qi_color: &color,
            ledger: &mut ledger,
            existing_session: None,
            skill_set: None,
            has_nearby_workbench: false, // 附近没有制作台
        },
    );
    assert!(
        result.is_ok(),
        "handcraft recipe (station: None) must succeed even without nearby workbench: {result:?}"
    );
}

#[test]
fn start_craft_rejects_skill_level_below_loaded_requirement_without_side_effects() {
    let (mut registry, mut unlock, mut cult, color, mut ledger) = make_world();
    let mut recipe = simple_recipe("skill_gate");
    recipe.requirements.skill_lv_min = Some(2);
    registry.register(recipe).unwrap();
    unlock.unlock("offline:Alice", RecipeId::new("skill_gate"));
    let mut inventory = prepared_inventory("skill_gate", &[("herb_a", 5), ("iron_needle", 5)]);
    let before_inventory = inventory.material_preparation.count("skill_gate", "herb_a");
    let mut skills = SkillSet::default();
    skills.skills.insert(
        bong_server::skill::components::SkillId::Herbalism,
        bong_server::skill::components::SkillEntry {
            lv: 1,
            ..Default::default()
        },
    );
    let err = start_craft(
        StartCraftRequest {
            caster: caster_entity(),
            player_id: "offline:Alice",
            recipe_id: &RecipeId::new("skill_gate"),
            current_tick: 0,
            quantity: 1,
        },
        ok_deps_for_player(
            &registry,
            &unlock,
            &mut inventory,
            &mut cult,
            &color,
            &mut ledger,
            Some(&skills),
        ),
    )
    .unwrap_err();
    assert_eq!(
        err,
        StartCraftError::SkillTooLow {
            required: 2,
            current: 1
        }
    );
    assert_eq!(
        inventory.material_preparation.count("skill_gate", "herb_a"),
        before_inventory,
        "skill rejection must not consume materials"
    );
    assert_eq!(
        cult.qi_current, 50.0,
        "skill rejection must not transfer qi"
    );
}

#[test]
fn start_craft_accepts_loaded_skill_requirement_at_boundary() {
    let (mut registry, mut unlock, mut cult, color, mut ledger) = make_world();
    let mut recipe = simple_recipe("skill_gate_boundary");
    recipe.requirements.skill_lv_min = Some(2);
    registry.register(recipe).unwrap();
    unlock.unlock("offline:Alice", RecipeId::new("skill_gate_boundary"));
    let mut inventory =
        prepared_inventory("skill_gate_boundary", &[("herb_a", 5), ("iron_needle", 5)]);
    let mut skills = SkillSet::default();
    skills.skills.insert(
        bong_server::skill::components::SkillId::Herbalism,
        bong_server::skill::components::SkillEntry {
            lv: 2,
            ..Default::default()
        },
    );
    let result = start_craft(
        StartCraftRequest {
            caster: caster_entity(),
            player_id: "offline:Alice",
            recipe_id: &RecipeId::new("skill_gate_boundary"),
            current_tick: 0,
            quantity: 1,
        },
        ok_deps_for_player(
            &registry,
            &unlock,
            &mut inventory,
            &mut cult,
            &color,
            &mut ledger,
            Some(&skills),
        ),
    );
    assert!(
        result.is_ok(),
        "skill level exactly at requirement must permit craft: {result:?}"
    );
}

#[test]
fn start_craft_uses_highest_non_herbalism_skill() {
    let (mut registry, mut unlock, mut cult, color, mut ledger) = make_world();
    let mut recipe = simple_recipe("skill_gate_any_skill");
    recipe.requirements.skill_lv_min = Some(2);
    registry.register(recipe).unwrap();
    unlock.unlock("offline:Alice", RecipeId::new("skill_gate_any_skill"));
    let mut inventory =
        prepared_inventory("skill_gate_any_skill", &[("herb_a", 5), ("iron_needle", 5)]);
    let mut skills = SkillSet::default();
    skills.skills.insert(
        bong_server::skill::components::SkillId::Alchemy,
        bong_server::skill::components::SkillEntry {
            lv: 2,
            ..Default::default()
        },
    );
    let result = start_craft(
        StartCraftRequest {
            caster: caster_entity(),
            player_id: "offline:Alice",
            recipe_id: &RecipeId::new("skill_gate_any_skill"),
            current_tick: 0,
            quantity: 1,
        },
        ok_deps_for_player(
            &registry,
            &unlock,
            &mut inventory,
            &mut cult,
            &color,
            &mut ledger,
            Some(&skills),
        ),
    );
    assert!(
        result.is_ok(),
        "any learned skill at the requirement must permit craft: {result:?}"
    );
}

#[test]
fn start_craft_applies_realm_skill_cap_before_comparison() {
    let (mut registry, mut unlock, mut cult, color, mut ledger) = make_world();
    cult.realm = Realm::Awaken;
    let mut recipe = simple_recipe("skill_gate_cap");
    recipe.requirements.skill_lv_min = Some(4);
    registry.register(recipe).unwrap();
    unlock.unlock("offline:Alice", RecipeId::new("skill_gate_cap"));
    let mut inventory = prepared_inventory("skill_gate_cap", &[("herb_a", 5), ("iron_needle", 5)]);
    let mut skills = SkillSet::default();
    skills.skills.insert(
        bong_server::skill::components::SkillId::Alchemy,
        bong_server::skill::components::SkillEntry {
            lv: 5,
            ..Default::default()
        },
    );
    let error = start_craft(
        StartCraftRequest {
            caster: caster_entity(),
            player_id: "offline:Alice",
            recipe_id: &RecipeId::new("skill_gate_cap"),
            current_tick: 0,
            quantity: 1,
        },
        ok_deps_for_player(
            &registry,
            &unlock,
            &mut inventory,
            &mut cult,
            &color,
            &mut ledger,
            Some(&skills),
        ),
    )
    .unwrap_err();
    assert_eq!(
        error,
        StartCraftError::SkillTooLow {
            required: 4,
            current: bong_server::cultivation::breakthrough::skill_cap_for_realm(Realm::Awaken),
        },
        "real skill level above realm cap must compare using effective level"
    );
}
#[test]
fn start_craft_missing_skill_set_treats_level_as_zero_without_side_effects() {
    // verdict-1906-r2 major #8 回归：SkillSet 组件缺失时 skill 门槛按 0 处理
    // （fail-closed：达不到任何 skill_lv_min），且不产生任何真元/材料副作用。
    let (mut registry, mut unlock, mut cult, color, mut ledger) = make_world();
    let mut recipe = simple_recipe("skill_gate_no_set");
    recipe.requirements.skill_lv_min = Some(1);
    registry.register(recipe).unwrap();
    unlock.unlock("offline:Alice", RecipeId::new("skill_gate_no_set"));
    let mut inventory =
        prepared_inventory("skill_gate_no_set", &[("herb_a", 5), ("iron_needle", 5)]);
    let before_inventory = inventory
        .material_preparation
        .count("skill_gate_no_set", "herb_a");

    let err = start_craft(
        StartCraftRequest {
            caster: caster_entity(),
            player_id: "offline:Alice",
            recipe_id: &RecipeId::new("skill_gate_no_set"),
            current_tick: 0,
            quantity: 1,
        },
        ok_deps_for_player(
            &registry,
            &unlock,
            &mut inventory,
            &mut cult,
            &color,
            &mut ledger,
            None, // 组件缺失
        ),
    )
    .unwrap_err();
    assert_eq!(
        err,
        StartCraftError::SkillTooLow {
            required: 1,
            current: 0
        },
        "missing SkillSet must fail closed with current=0"
    );
    assert_eq!(
        inventory
            .material_preparation
            .count("skill_gate_no_set", "herb_a"),
        before_inventory,
        "missing-skill rejection must not consume materials"
    );
    assert_eq!(
        cult.qi_current, 50.0,
        "missing-skill rejection must not move qi"
    );
    assert!(
        ledger.transfers().is_empty(),
        "missing-skill rejection must not write any qi ledger transfer"
    );
}

#[test]
fn start_craft_empty_skill_set_treats_level_as_zero_without_side_effects() {
    // 同 major #8：SkillSet 存在但 skills 为空（新玩家未学任何技能）。
    let (mut registry, mut unlock, mut cult, color, mut ledger) = make_world();
    let mut recipe = simple_recipe("skill_gate_empty");
    recipe.requirements.skill_lv_min = Some(1);
    registry.register(recipe).unwrap();
    unlock.unlock("offline:Alice", RecipeId::new("skill_gate_empty"));
    let mut inventory =
        prepared_inventory("skill_gate_empty", &[("herb_a", 5), ("iron_needle", 5)]);
    let before_inventory = inventory
        .material_preparation
        .count("skill_gate_empty", "herb_a");
    let skills = SkillSet::default();

    let err = start_craft(
        StartCraftRequest {
            caster: caster_entity(),
            player_id: "offline:Alice",
            recipe_id: &RecipeId::new("skill_gate_empty"),
            current_tick: 0,
            quantity: 1,
        },
        ok_deps_for_player(
            &registry,
            &unlock,
            &mut inventory,
            &mut cult,
            &color,
            &mut ledger,
            Some(&skills),
        ),
    )
    .unwrap_err();
    assert_eq!(
        err,
        StartCraftError::SkillTooLow {
            required: 1,
            current: 0
        },
        "empty SkillSet must fail closed with current=0"
    );
    assert_eq!(
        inventory
            .material_preparation
            .count("skill_gate_empty", "herb_a"),
        before_inventory,
        "empty-skill rejection must not consume materials"
    );
    assert_eq!(
        cult.qi_current, 50.0,
        "empty-skill rejection must not move qi"
    );
    assert!(
        ledger.transfers().is_empty(),
        "empty-skill rejection must not write any qi ledger transfer"
    );
}

#[test]
fn start_craft_skill_set_below_requirement_reports_effective_level() {
    // AnySkill 门槛取所有已存在技能的最高 effective level；这里的 Forging Lv.1
    // 低于要求 2，因此拒绝时应报告当前等级 1，而不是错误地声称没有任何技能。
    let (mut registry, mut unlock, mut cult, color, mut ledger) = make_world();
    let mut recipe = simple_recipe("skill_gate_below_requirement");
    recipe.requirements.skill_lv_min = Some(2);
    registry.register(recipe).unwrap();
    unlock.unlock(
        "offline:Alice",
        RecipeId::new("skill_gate_below_requirement"),
    );
    let mut inventory = prepared_inventory(
        "skill_gate_below_requirement",
        &[("herb_a", 5), ("iron_needle", 5)],
    );
    let mut skills = SkillSet::default();
    skills.skills.insert(
        bong_server::skill::components::SkillId::Forging,
        bong_server::skill::components::SkillEntry {
            lv: 1,
            ..Default::default()
        },
    );

    let err = start_craft(
        StartCraftRequest {
            caster: caster_entity(),
            player_id: "offline:Alice",
            recipe_id: &RecipeId::new("skill_gate_below_requirement"),
            current_tick: 0,
            quantity: 1,
        },
        ok_deps_for_player(
            &registry,
            &unlock,
            &mut inventory,
            &mut cult,
            &color,
            &mut ledger,
            Some(&skills),
        ),
    )
    .unwrap_err();
    assert_eq!(
        err,
        StartCraftError::SkillTooLow {
            required: 2,
            current: 1
        },
        "存在 Lv.1 技能但要求 Lv.2 时必须报告 current=1"
    );
    assert!(
        ledger.transfers().is_empty(),
        "low-skill rejection must not write any qi ledger transfer"
    );
}

#[test]
fn start_craft_workbench_recipe_fails_without_nearby_workbench() {
    // station: Some(Workbench) 且 has_nearby_workbench: false → StationOutOfRange
    let mut registry = CraftRegistry::new();
    let mut recipe = simple_recipe("wb_tool");
    recipe.station = Some(CraftStationKind::Workbench);
    recipe.qi_cost = 0.0;
    registry.register(recipe).unwrap();

    let mut unlock = RecipeUnlockState::new();
    unlock.unlock("offline:Alice", RecipeId::new("wb_tool"));
    let mut inv = prepared_inventory("wb_tool", &[("herb_a", 5), ("iron_needle", 5)]);
    let mut cult = Cultivation {
        qi_current: 50.0,
        qi_max: 80.0,
        ..Default::default()
    };
    let color = QiColor::default();
    let mut ledger = WorldQiAccount::default();

    let err = start_craft(
        StartCraftRequest {
            caster: caster_entity(),
            player_id: "offline:Alice",
            recipe_id: &RecipeId::new("wb_tool"),
            current_tick: 0,
            quantity: 1,
        },
        StartCraftDeps {
            registry: &registry,
            unlock_state: &unlock,
            inventory: &mut inv,
            cultivation: &mut cult,
            qi_color: &color,
            ledger: &mut ledger,
            existing_session: None,
            skill_set: None,
            has_nearby_workbench: false,
        },
    )
    .unwrap_err();

    assert_eq!(
        err,
        StartCraftError::StationOutOfRange,
        "workbench recipe must fail with StationOutOfRange when no workbench is nearby"
    );
    assert_eq!(
        inv.material_preparation.count("wb_tool", "herb_a"),
        5,
        "materials must not be consumed on StationOutOfRange rejection"
    );
}
#[test]
fn start_craft_workbench_recipe_passes_with_nearby_workbench() {
    // station: Some(Workbench) 且 has_nearby_workbench: true → 正常通过
    let mut registry = CraftRegistry::new();
    let mut recipe = simple_recipe("wb_tool2");
    recipe.station = Some(CraftStationKind::Workbench);
    recipe.qi_cost = 0.0;
    registry.register(recipe).unwrap();

    let mut unlock = RecipeUnlockState::new();
    unlock.unlock("offline:Alice", RecipeId::new("wb_tool2"));
    let mut inv = prepared_inventory("wb_tool2", &[("herb_a", 5), ("iron_needle", 5)]);
    let mut cult = Cultivation {
        qi_current: 50.0,
        qi_max: 80.0,
        ..Default::default()
    };
    let color = QiColor::default();
    let mut ledger = WorldQiAccount::default();

    let result = start_craft(
        StartCraftRequest {
            caster: caster_entity(),
            player_id: "offline:Alice",
            recipe_id: &RecipeId::new("wb_tool2"),
            current_tick: 0,
            quantity: 1,
        },
        StartCraftDeps {
            registry: &registry,
            unlock_state: &unlock,
            inventory: &mut inv,
            cultivation: &mut cult,
            qi_color: &color,
            ledger: &mut ledger,
            existing_session: None,
            skill_set: None,
            has_nearby_workbench: true, // 附近有制作台
        },
    );
    assert!(
        result.is_ok(),
        "workbench recipe must succeed when workbench is nearby: {result:?}"
    );
}
