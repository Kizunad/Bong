#![allow(dead_code, unused_imports)]

use std::collections::HashMap;

use super::super::events::{InsightTrigger, UnlockEventSource};
use super::super::recipe::{CraftCategory, CraftRequirements, CraftStationKind, UnlockSource};
use super::*;
use crate::cultivation::components::Cultivation;
use crate::inventory::{
    ContainerState, InventoryRevision, ItemInstance, ItemRarity, PlacedItemState,
};
use crate::qi_physics::ledger::QiAccountId;
use valence::prelude::App;

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

fn prepared_inventory(recipe: &CraftRecipe, items: &[(&str, u32)]) -> PlayerInventory {
    let mut inventory = make_inventory(items);
    let ids: Vec<_> = inventory.containers[0]
        .items
        .iter()
        .map(|entry| entry.instance.instance_id)
        .collect();
    for id in ids {
        crate::craft::preparation::stage_material(&mut inventory, recipe, id).unwrap();
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
fn start_craft_baseline_workbench_passes_unlock_gate_with_empty_state() {
    // 基线常显豁免（unlock::BASELINE_RECIPES）：制作台自身配方对空 unlock
    // state 的新玩家必须直接可做 —— 不经材料发现、不经三渠道。
    let mut registry = CraftRegistry::new();
    crate::craft::register_workbench_recipes(&mut registry).unwrap();
    let unlock = RecipeUnlockState::new(); // 从未解锁过任何配方
    let mut inv = prepared_inventory(
        registry
            .get(&RecipeId::new("craft.tool.workbench"))
            .unwrap(),
        &[("spirit_wood", 4), ("iron_ingot", 2), ("shu_gu", 2)],
    );
    let mut cult = Cultivation {
        qi_current: 50.0,
        qi_max: 80.0,
        ..Default::default()
    };
    let color = QiColor::default();
    let mut ledger = WorldQiAccount::default();

    let mut deps = ok_deps_for_player(
        &registry,
        &unlock,
        &mut inv,
        &mut cult,
        &color,
        &mut ledger,
        None,
    );
    // 制作台自身是手搓配方（station: None），附近没有制作台也必须能做 ——
    // 否则"造第一张制作台"死锁。
    deps.has_nearby_workbench = false;

    let success = start_craft(
        StartCraftRequest {
            caster: caster_entity(),
            player_id: "offline:Alice",
            recipe_id: &RecipeId::new("craft.tool.workbench"),
            current_tick: 0,
            quantity: 1,
        },
        deps,
    )
    .unwrap_or_else(|err| {
        panic!(
            "期望基线豁免让空 unlock state 玩家直接开工制作台，因为它是 workbench \
             配方树的入口配方；实际报错 {err:?}"
        )
    });
    assert_eq!(
        success.session.recipe_id,
        RecipeId::new("craft.tool.workbench")
    );
    // 材料照常扣除（豁免只绕 unlock 门，不绕材料校验）
    assert_eq!(
        inv.material_preparation
            .count("craft.tool.workbench", "spirit_wood"),
        0
    );
    assert_eq!(
        inv.material_preparation
            .count("craft.tool.workbench", "iron_ingot"),
        0
    );
    assert_eq!(
        inv.material_preparation
            .count("craft.tool.workbench", "shu_gu"),
        0
    );
}

#[test]
fn start_craft_baseline_exemption_does_not_leak_to_other_workbench_recipes() {
    // 对照组：同一注册表里其他空源配方（如石镐）对空 unlock state 仍应 NotUnlocked
    // —— 豁免名单精确到 craft.tool.workbench，不是放开整棵 workbench 树。
    let mut registry = CraftRegistry::new();
    crate::craft::register_workbench_recipes(&mut registry).unwrap();
    let unlock = RecipeUnlockState::new();
    let mut inv = prepared_inventory(
        registry
            .get(&RecipeId::new("workbench.tool.stone_pickaxe"))
            .unwrap(),
        &[("stone_chunk", 3), ("wood_handle", 1)],
    );
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
            recipe_id: &RecipeId::new("workbench.tool.stone_pickaxe"),
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
    assert!(
        matches!(err, StartCraftError::NotUnlocked(_)),
        "期望石镐对空 unlock state 仍 NotUnlocked（基线豁免只覆盖制作台自身），\
         实际={err:?}"
    );
}
