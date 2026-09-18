#![allow(dead_code, unused_imports)]

use bong_server::combat::CombatClock;
use bong_server::craft::{
    count_template_in_inventory, CraftCancelIntent, CraftCompletedEvent, CraftFailedEvent,
    CraftFailureReason, CraftRegistry, CraftStartIntent, CraftStartedEvent,
};
use bong_server::cultivation::components::{Cultivation, QiColor};
use bong_server::inventory::PlayerInventory;
use bong_server::network::agent_bridge::{serialize_server_data_payload, SERVER_DATA_CHANNEL};
use bong_server::network::craft_emit::*;
use bong_server::player::gameplay::PendingGameplayNarrations;
use bong_server::player::state::{canonical_player_id, PlayerStatePersistence};
use bong_server::qi_physics::ledger::WorldQiAccount;
use bong_server::schema::craft::{CraftRequirementsV1, CraftSessionStateV1, RecipeListV1};
use bong_server::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
use bong_server::skill::components::SkillSet;
use bong_server::world::dimension::{CurrentDimension, DimensionKind};
use std::time::{SystemTime, UNIX_EPOCH};
use valence::prelude::{Client, Position};

use bong_server::craft::{
    register_basic_processing_recipes, CraftCategory, CraftRecipe, CraftRequirements, CraftSession,
    RecipeId, RecipeUnlockState, UnlockSource,
};
use bong_server::cultivation::tick::CultivationClock;
use bong_server::inventory::{
    ContainerState, DroppedLootRegistry, InventoryInstanceIdAllocator, InventoryRevision,
    ItemCategory, ItemInstance, ItemRarity, ItemRegistry, ItemTemplate, PlacedItemState,
    JS_SAFE_INTEGER_MAX,
};
use bong_server::persistence::bootstrap_sqlite;
use bong_server::player::state::{load_player_slices, save_player_state, PlayerState};
use bong_server::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;
use bong_server::qi_physics::ledger::{pending_inflow_account, QiAccountId, QiTransferReason};
use bong_server::world::events::ActiveEventsResource;
use bong_server::world::heartbeat;
use bong_server::world::zone::{Zone, ZoneRegistry};
use bong_server::worldgen::pseudo_vein::TICKS_PER_MINUTE;
use std::collections::HashMap;
use std::path::PathBuf;
use valence::prelude::{App, DVec3, Events, Update};
use valence::protocol::packets::play::CustomPayloadS2c;
use valence::testing::{create_mock_client, MockClientHelper};

/// 造一个最小 PlayerInventory，main_pack 内放指定 (template, count)。
fn inv_with(items: &[(&str, u32)]) -> PlayerInventory {
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

fn registry_with_templates(templates: &[(&str, u32)]) -> ItemRegistry {
    let templates = templates
        .iter()
        .map(|(id, max_stack_count)| {
            let template = ItemTemplate {
                quick_use: false,
                id: (*id).to_string(),
                display_name: (*id).to_string(),
                category: ItemCategory::Misc,
                placeable: None,
                max_stack_count: *max_stack_count,
                grid_w: 1,
                grid_h: 1,
                base_weight: 1.0,
                rarity: ItemRarity::Common,
                spirit_quality_initial: 0.0,
                description: String::new(),
                effect: None,
                cast_duration_ms: bong_server::inventory::DEFAULT_CAST_DURATION_MS,
                cooldown_ms: bong_server::inventory::DEFAULT_COOLDOWN_MS,
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
                wearer_race: bong_server::body_plan::types::RaceGateOwned::default(),
            };
            ((*id).to_string(), template)
        })
        .collect();
    ItemRegistry::from_map(templates)
}

fn clamp_main_pack_to_grid(inventory: &mut PlayerInventory, rows: u8, cols: u8) {
    inventory.containers[0].rows = rows;
    inventory.containers[0].cols = cols;
}

fn craft_refund_test_app(recipe: CraftRecipe, templates: &[(&str, u32)], clock_tick: u64) -> App {
    let mut app = App::new();
    let mut registry = CraftRegistry::new();
    registry.register(recipe).unwrap();
    app.insert_resource(registry);
    app.insert_resource(RecipeUnlockState::new());
    app.insert_resource(WorldQiAccount::default());
    app.insert_resource(registry_with_templates(templates));
    app.insert_resource(InventoryInstanceIdAllocator::new(100));
    app.insert_resource(DroppedLootRegistry::default());
    app.insert_resource(CombatClock { tick: clock_tick });
    app.add_event::<CraftStartIntent>();
    app.add_event::<CraftCancelIntent>();
    app.add_event::<CraftStartedEvent>();
    app.add_event::<CraftCompletedEvent>();
    app.add_event::<CraftFailedEvent>();
    app
}

fn craft_test_persistence(test_name: &str) -> (PlayerStatePersistence, PathBuf) {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let data_dir = std::env::temp_dir().join(format!(
        "bong-craft-emit-{test_name}-{}-{suffix}",
        std::process::id()
    ));
    let db_path = data_dir.join("bong.db");
    bootstrap_sqlite(&db_path, &format!("craft-emit-{test_name}"))
        .expect("sqlite bootstrap should succeed");
    let persistence = PlayerStatePersistence::with_db_path(&data_dir, &db_path);
    save_player_state(&persistence, "Azure", &PlayerState::default())
        .expect("test player should initialize");
    (persistence, data_dir)
}

fn current_failed_events(app: &App) -> Vec<CraftFailedEvent> {
    app.world()
        .resource::<Events<CraftFailedEvent>>()
        .iter_current_update_events()
        .cloned()
        .collect()
}

#[test]
fn material_transfer_persists_once_and_failed_return_keeps_custody() {
    use bong_server::craft::events::MaterialMoveIntent;
    use bong_server::network::craft_materials::apply_craft_material_intents;
    let recipe_id = RecipeId::new("craft.tool.workbench");
    let mut app = craft_refund_test_app(
        make_recipe(recipe_id.as_str(), &[("fan_tie", 1)], vec![]),
        &[("fan_tie", 64)],
        10,
    );
    app.add_event::<MaterialMoveIntent>();
    app.add_systems(Update, apply_craft_material_intents);
    app.add_systems(Update, apply_craft_cancel_intents);
    let (persistence, data_dir) = craft_test_persistence("material-custody");
    app.insert_resource(persistence.clone());
    let original = inv_with(&[("fan_tie", 2)]);
    let original_item = original.containers[0].items[0].clone();
    let (bundle, _helper) = create_mock_client("Azure");
    let player = app
        .world_mut()
        .spawn(bundle)
        .insert(original)
        .insert(PlayerState::default())
        .insert(Cultivation::default())
        .insert(Position::new([0.0, 64.0, 0.0]))
        .id();
    let intent = MaterialMoveIntent {
        caster: player,
        recipe_id,
        instance_id: Some(1),
        station_pos: None,
        returning: false,
        expected_revision: 1,
    };
    app.world_mut().send_event(intent.clone());
    app.world_mut().send_event(intent.clone());
    app.update();
    let saved = load_player_slices(&persistence, "Azure").inventory.unwrap();
    assert!(saved.containers[0].items.is_empty());
    assert_eq!(
        saved.material_preparation.materials.len(),
        1,
        "重复请求不能重复托管"
    );
    assert_eq!(
        saved.material_preparation.materials[0].item,
        original_item.instance
    );

    let connection = rusqlite::Connection::open(persistence.db_path()).unwrap();
    connection
        .execute_batch(
            "CREATE TRIGGER fail_material_return BEFORE UPDATE ON inventories
        BEGIN SELECT RAISE(FAIL, 'forced material return failure'); END;",
        )
        .unwrap();
    app.world_mut().send_event(MaterialMoveIntent {
        returning: true,
        expected_revision: saved.revision.0,
        ..intent
    });
    app.update();
    let current = app.world().get::<PlayerInventory>(player).unwrap();
    assert!(
        current.containers[0].items.is_empty(),
        "落盘失败不得提前把材料放回背包"
    );
    assert_eq!(current.material_preparation, saved.material_preparation);
    assert!(app
        .world()
        .resource::<DroppedLootRegistry>()
        .entries
        .is_empty());

    connection
        .execute_batch("DROP TRIGGER fail_material_return;")
        .unwrap();
    app.world_mut()
        .send_event(CraftCancelIntent { caster: player });
    app.update();
    let saved = load_player_slices(&persistence, "Azure").inventory.unwrap();
    assert_eq!(
        saved.containers[0].items,
        vec![original_item],
        "未开工关闭时全额原样返还"
    );
    assert!(saved.material_preparation.materials.is_empty());
    drop(connection);
    drop(app);
    std::fs::remove_dir_all(data_dir).unwrap();
}

#[test]
fn forge_material_custody_checks_station_authority_and_survives_station_loss() {
    use bong_server::craft::events::MaterialMoveIntent;
    use bong_server::forge::{
        blueprint::BlueprintRegistry, learned::LearnedBlueprints, station::WeaponForgeStation,
    };
    use bong_server::network::craft_materials::apply_craft_material_intents;
    let mut app = craft_refund_test_app(
        make_recipe("unrelated", &[("fan_tie", 1)], vec![]),
        &[("fan_tie", 64)],
        10,
    );
    app.insert_resource(BlueprintRegistry::load_dir("assets/forge/blueprints").unwrap());
    app.add_event::<MaterialMoveIntent>();
    app.add_systems(Update, apply_craft_material_intents);
    let (bundle, _helper) = create_mock_client("Azure");
    let original = inv_with(&[("fan_tie", 3)]);
    let original_item = original.containers[0].items[0].clone();
    let player = app
        .world_mut()
        .spawn(bundle)
        .insert((
            original,
            PlayerState::default(),
            Cultivation::default(),
            Position::new([1.0, 64.0, 0.0]),
            CurrentDimension(DimensionKind::Overworld),
            LearnedBlueprints {
                ids: vec!["iron_sword_v0".into()],
                current_index: 0,
            },
        ))
        .id();
    let other = app.world_mut().spawn_empty().id();
    let station = app
        .world_mut()
        .spawn(WeaponForgeStation::placed(
            valence::prelude::BlockPos::new(2, 64, 0),
            1,
            other,
        ))
        .id();
    let intent = MaterialMoveIntent {
        caster: player,
        recipe_id: RecipeId::new("iron_sword_v0"),
        instance_id: Some(1),
        station_pos: Some((2, 64, 0)),
        returning: false,
        expected_revision: 1,
    };
    app.world_mut().send_event(intent.clone());
    app.update();
    assert!(
        app.world()
            .get::<PlayerInventory>(player)
            .unwrap()
            .material_preparation
            .materials
            .is_empty(),
        "不能向他人的工位投入材料"
    );
    app.world_mut()
        .get_mut::<WeaponForgeStation>(station)
        .unwrap()
        .owner = Some(player);
    app.world_mut()
        .get_mut::<Position>(player)
        .unwrap()
        .set([20.0, 64.0, 0.0]);
    app.world_mut().send_event(intent.clone());
    app.update();
    assert!(
        app.world()
            .get::<PlayerInventory>(player)
            .unwrap()
            .material_preparation
            .materials
            .is_empty(),
        "不能隔空投料"
    );
    app.world_mut()
        .get_mut::<Position>(player)
        .unwrap()
        .set([1.0, 64.0, 0.0]);
    app.world_mut().send_event(intent.clone());
    app.update();
    assert_eq!(
        app.world()
            .get::<PlayerInventory>(player)
            .unwrap()
            .material_preparation
            .materials
            .len(),
        1
    );
    app.world_mut().despawn(station);
    app.world_mut().send_event(MaterialMoveIntent {
        returning: true,
        instance_id: None,
        ..intent
    });
    app.update();
    let inventory = app.world().get::<PlayerInventory>(player).unwrap();
    assert_eq!(
        inventory.containers[0].items,
        vec![original_item],
        "未开炉材料在工位消失后仍可原样取回"
    );
    assert!(inventory.material_preparation.materials.is_empty());
}

fn current_completed_events(app: &App) -> Vec<CraftCompletedEvent> {
    app.world()
        .resource::<Events<CraftCompletedEvent>>()
        .iter_current_update_events()
        .cloned()
        .collect()
}

/// 造一个 craft 配方，指定 id / 原料 / 解锁来源（空 vec = 材料发现路径）。
fn make_recipe(id: &str, materials: &[(&str, u32)], sources: Vec<UnlockSource>) -> CraftRecipe {
    CraftRecipe {
        id: RecipeId::new(id),
        category: CraftCategory::Tool,
        display_name: id.into(),
        materials: materials
            .iter()
            .map(|(t, c)| ((*t).to_string(), *c))
            .collect(),
        qi_cost: 0.0,
        time_ticks: 60,
        output: ("out".into(), 1),
        requirements: CraftRequirements::default(),
        unlock_sources: sources,
        station: None,
    }
}

/// 解锁 registry 内全部配方（构造"全解锁"基线，用于列表排序 / 体积上限测试）。
fn unlock_all(unlock_state: &mut RecipeUnlockState, player: &str, registry: &CraftRegistry) {
    for r in registry.iter() {
        unlock_state.unlock(player.to_string(), r.id.clone());
    }
}

fn flush_client_packets(app: &mut App) {
    let world = app.world_mut();
    let mut query = world.query::<&mut Client>();
    for mut client in query.iter_mut(world) {
        client
            .flush_packets()
            .expect("mock client packets should flush");
    }
}

fn collect_recipe_lists(helper: &mut MockClientHelper) -> Vec<RecipeListV1> {
    let mut out = Vec::new();
    for frame in helper.collect_received().0 {
        let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
            continue;
        };
        if packet.channel.as_str() != SERVER_DATA_CHANNEL {
            continue;
        }
        let payload = serde_json::from_slice::<serde_json::Value>(packet.data.0 .0)
            .expect("server_data payload should decode as JSON");
        if payload.get("type").and_then(|v| v.as_str()) == Some("craft_recipe_list") {
            let mut list_payload = payload;
            if let Some(object) = list_payload.as_object_mut() {
                object.remove("type");
            }
            let list = serde_json::from_value::<RecipeListV1>(list_payload)
                .expect("craft_recipe_list payload should decode");
            out.push(list);
        }
    }
    out
}

fn collect_craft_session_states(helper: &mut MockClientHelper) -> Vec<CraftSessionStateV1> {
    let mut out = Vec::new();
    for frame in helper.collect_received().0 {
        let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
            continue;
        };
        if packet.channel.as_str() != SERVER_DATA_CHANNEL {
            continue;
        }
        let payload = serde_json::from_slice::<serde_json::Value>(packet.data.0 .0)
            .expect("server_data payload should decode as JSON");
        if payload.get("type").and_then(|v| v.as_str()) == Some("craft_session_state") {
            let mut state_payload = payload;
            if let Some(object) = state_payload.as_object_mut() {
                object.remove("type");
            }
            let state = serde_json::from_value::<CraftSessionStateV1>(state_payload)
                .expect("craft_session_state payload should decode");
            out.push(state);
        }
    }
    out
}

#[test]
fn cancel_intent_missing_caster_is_noop_without_failed_event() {
    let recipe = make_recipe(
        "craft.test.cancel_missing_caster",
        &[("fan_tie", 2)],
        vec![],
    );
    let mut app = craft_refund_test_app(recipe, &[("fan_tie", 64)], 11);
    app.add_systems(Update, apply_craft_cancel_intents);

    let caster_without_inventory = app.world_mut().spawn_empty().id();
    app.world_mut().send_event(CraftCancelIntent {
        caster: caster_without_inventory,
    });
    app.update();

    assert!(
        current_failed_events(&app).is_empty(),
        "caster 查找失败应只跳过本 intent，不能伪造 Failed 事件"
    );
    assert!(
        app.world()
            .get::<CraftSession>(caster_without_inventory)
            .is_none(),
        "缺少 inventory/session 的 caster 不应被 cancel 路径补写 CraftSession"
    );
}

#[test]
fn production_skill_gate_reads_caster_skill_set() {
    let mut recipe = make_recipe("craft.skill.integration", &[("fan_tie", 1)], vec![]);
    recipe.requirements.skill_lv_min = Some(2);
    let mut app = craft_refund_test_app(recipe, &[("fan_tie", 64)], 10);
    app.add_systems(Update, apply_craft_start_intents);

    let (client_bundle, _helper) = create_mock_client("Azure");
    let player = app
        .world_mut()
        .spawn(client_bundle)
        .insert(inv_with(&[("fan_tie", 1)]))
        .insert(Cultivation::default())
        .insert(QiColor::default())
        .insert(SkillSet::default())
        .insert(Position::new([0.0, 64.0, 0.0]))
        .id();
    app.world_mut()
        .resource_mut::<RecipeUnlockState>()
        .unlock("offline:Azure", RecipeId::new("craft.skill.integration"));
    app.world_mut()
        .get_mut::<SkillSet>(player)
        .unwrap()
        .skills
        .insert(
            bong_server::skill::components::SkillId::Alchemy,
            bong_server::skill::components::SkillEntry {
                lv: 1,
                ..Default::default()
            },
        );
    app.world_mut().send_event(CraftStartIntent {
        caster: player,
        recipe_id: RecipeId::new("craft.skill.integration"),
        quantity: 1,
    });

    app.update();

    assert!(
        app.world().get::<CraftSession>(player).is_none(),
        "production bridge must reject a caster below loaded skill requirement"
    );
    assert_eq!(
        count_template_in_inventory(
            app.world().get::<PlayerInventory>(player).unwrap(),
            "fan_tie"
        ),
        1,
        "production skill rejection must not consume materials"
    );
    assert!(
        !current_failed_events(&app).is_empty(),
        "production skill rejection must emit the observable craft failure"
    );
}

#[test]
fn duplicate_start_intents_same_frame_consume_materials_only_once() {
    let recipe = make_recipe("craft.tool.workbench", &[("fan_tie", 2)], vec![]);
    let mut app = craft_refund_test_app(recipe, &[("fan_tie", 64)], 10);
    app.add_systems(Update, apply_craft_start_intents);

    let (client_bundle, _helper) = create_mock_client("Azure");
    let player = app
        .world_mut()
        .spawn(client_bundle)
        .insert(inv_with(&[("fan_tie", 4)]))
        .insert(Cultivation::default())
        .insert(QiColor::default())
        .insert(Position::new([0.0, 64.0, 0.0]))
        .id();
    {
        let recipe = app
            .world()
            .resource::<CraftRegistry>()
            .get(&RecipeId::new("craft.tool.workbench"))
            .unwrap()
            .clone();
        let mut inventory = app.world_mut().get_mut::<PlayerInventory>(player).unwrap();
        bong_server::craft::preparation::stage_material(&mut inventory, &recipe, 1).unwrap();
    }
    for _ in 0..2 {
        app.world_mut().send_event(CraftStartIntent {
            caster: player,
            recipe_id: RecipeId::new("craft.tool.workbench"),
            quantity: 1,
        });
    }

    app.update();

    let started: Vec<_> = app
        .world()
        .resource::<Events<CraftStartedEvent>>()
        .iter_current_update_events()
        .collect();
    assert_eq!(
        started.len(),
        1,
        "同帧重复 start 只能创建一个 session/Started 事件，实际={started:?}"
    );
    let inventory = app.world().get::<PlayerInventory>(player).unwrap();
    assert_eq!(
        inventory
            .material_preparation
            .count("craft.tool.workbench", "fan_tie"),
        2,
        "同帧重复 start 只能预扣一次 fan_tie x2，不能在 deferred insert 前重复扣料"
    );
    assert!(
        app.world().get::<CraftSession>(player).is_some(),
        "首个 start 成功后应保留唯一 CraftSession"
    );
}

#[test]
fn apply_craft_start_intents_without_spatial_context_credits_pending_never_spawn() {
    let mut recipe = make_recipe("craft.tool.workbench", &[("fan_tie", 1)], vec![]);
    recipe.qi_cost = 5.0;
    let mut app = craft_refund_test_app(recipe, &[("fan_tie", 64)], 10);
    app.add_systems(Update, apply_craft_start_intents);

    let (client_bundle, _helper) = create_mock_client("Azure");
    let mut cultivation = Cultivation::default();
    cultivation.qi_current = 10.0;
    cultivation.qi_max = cultivation.qi_max.max(10.0);
    let mut player_entity = app.world_mut().spawn(client_bundle);
    player_entity
        .insert(inv_with(&[("fan_tie", 1)]))
        .insert(cultivation)
        .insert(QiColor::default())
        .remove::<Position>();
    let player = player_entity.id();
    {
        let recipe = app
            .world()
            .resource::<CraftRegistry>()
            .get(&RecipeId::new("craft.tool.workbench"))
            .unwrap()
            .clone();
        let mut inventory = app.world_mut().get_mut::<PlayerInventory>(player).unwrap();
        bong_server::craft::preparation::stage_material(&mut inventory, &recipe, 1).unwrap();
    }
    let player_account = QiAccountId::player(canonical_player_id("Azure"));
    let observed_before = app.world().get::<Cultivation>(player).unwrap().qi_current
        + app.world().resource::<WorldQiAccount>().total();

    app.world_mut().send_event(CraftStartIntent {
        caster: player,
        recipe_id: RecipeId::new("craft.tool.workbench"),
        quantity: 1,
    });
    app.update();

    let ledger = app.world().resource::<WorldQiAccount>();
    assert!(
        !ledger.has_account(&player_account),
        "在线玩家真元权威在 ECS，制作后不得留下长期 player ledger 镜像"
    );
    assert_eq!(
        ledger.balance(&pending_inflow_account()),
        5.0,
        "缺少空间组件时制作消耗仍应完整进入待分配池"
    );
    assert_eq!(
        ledger.balance(&QiAccountId::zone("spawn")),
        0.0,
        "制作消耗不得再落入陈旧的硬编码 spawn 账户"
    );
    let cultivation_after = app.world().get::<Cultivation>(player).unwrap();
    assert!(
        (cultivation_after.qi_current + ledger.total() - observed_before).abs() < 1e-9,
        "ECS player qi + ledger 在制作前后必须守恒"
    );
    let transfer = ledger
        .transfers()
        .last()
        .expect("正 qi_cost 制作必须留下审计 transfer");
    assert_eq!(
        transfer.from, player_account,
        "制作审计转账的来源必须是当前玩家账户"
    );
    assert_eq!(
        transfer.to,
        pending_inflow_account(),
        "制作审计转账的目标必须是待分配池"
    );
    assert_eq!(
        transfer.reason,
        QiTransferReason::Crafting,
        "制作审计转账必须标记为 Crafting"
    );
    assert_eq!(transfer.amount, 5.0, "制作审计金额必须等于配方 qi_cost");
    assert_eq!(
        cultivation_after.qi_current, 5.0,
        "制作后应从 ECS 玩家真元扣除 5 点"
    );
    assert!(
        app.world().get::<CraftSession>(player).is_some(),
        "成功预付真元后必须创建制作会话"
    );
    assert!(
        current_failed_events(&app).is_empty(),
        "成功制作起手不得发出 CraftFailedEvent"
    );
}

#[test]
fn start_persistence_failure_keeps_inventory_qi_ledger_and_session_at_pre_state() {
    let mut recipe = make_recipe("craft.tool.workbench", &[("fan_tie", 2)], vec![]);
    recipe.qi_cost = 5.0;
    let mut app = craft_refund_test_app(recipe, &[("fan_tie", 64)], 10);
    app.add_systems(Update, apply_craft_start_intents);
    let (persistence, data_dir) = craft_test_persistence("start-rollback");
    let connection = rusqlite::Connection::open(persistence.db_path())
        .expect("sqlite should open for failure injection");
    connection
        .execute_batch(
            "
            CREATE TRIGGER fail_craft_session_insert
            BEFORE INSERT ON player_craft_sessions
            BEGIN
                SELECT RAISE(FAIL, 'forced craft session failure');
            END;
            ",
        )
        .expect("failure trigger should install");
    app.insert_resource(persistence.clone());

    let (client_bundle, _helper) = create_mock_client("Azure");
    let mut cultivation = Cultivation::default();
    cultivation.qi_current = 10.0;
    cultivation.qi_max = cultivation.qi_max.max(10.0);
    let player = app
        .world_mut()
        .spawn(client_bundle)
        .insert(inv_with(&[("fan_tie", 2)]))
        .insert(cultivation)
        .insert(QiColor::default())
        .insert(Position::new([0.0, 64.0, 0.0]))
        .id();
    {
        let recipe = app
            .world()
            .resource::<CraftRegistry>()
            .get(&RecipeId::new("craft.tool.workbench"))
            .unwrap()
            .clone();
        let mut inventory = app.world_mut().get_mut::<PlayerInventory>(player).unwrap();
        bong_server::craft::preparation::stage_material(&mut inventory, &recipe, 1).unwrap();
    }
    let player_account = QiAccountId::player(canonical_player_id("Azure"));
    app.world_mut().send_event(CraftStartIntent {
        caster: player,
        recipe_id: RecipeId::new("craft.tool.workbench"),
        quantity: 1,
    });

    app.update();

    let inventory = app.world().get::<PlayerInventory>(player).unwrap();
    assert_eq!(
        inventory
            .material_preparation
            .count("craft.tool.workbench", "fan_tie"),
        2,
        "failed durable start must not publish the staged material debit"
    );
    assert_eq!(
        app.world().get::<Cultivation>(player).unwrap().qi_current,
        10.0,
        "failed durable start must not publish the staged qi debit"
    );
    assert!(
        app.world().get::<CraftSession>(player).is_none(),
        "failed durable start must not create an in-memory session"
    );
    let ledger = app.world().resource::<WorldQiAccount>();
    assert!(
        !ledger.has_account(&player_account),
        "持久化拒绝后不得发布临时 player ledger 影子账户"
    );
    assert_eq!(
        ledger.balance(&pending_inflow_account()),
        0.0,
        "持久化拒绝后不得发布 staged pending 入账"
    );
    assert!(
        ledger.transfers().is_empty(),
        "failed durable start must not publish a transfer audit entry"
    );
    assert_eq!(
        current_failed_events(&app).len(),
        1,
        "persistence rejection should produce one client-visible failure"
    );
    let reloaded = load_player_slices(&persistence, "Azure");
    assert!(reloaded.inventory.is_none());
    assert!(reloaded.craft_session.is_none());
    std::fs::remove_dir_all(data_dir).ok();
}

#[test]
fn cancel_intent_without_session_is_noop_without_failed_event() {
    let recipe = make_recipe(
        "craft.test.cancel_without_session",
        &[("fan_tie", 2)],
        vec![],
    );
    let mut app = craft_refund_test_app(recipe, &[("fan_tie", 64)], 12);
    app.add_systems(Update, apply_craft_cancel_intents);

    let player = app.world_mut().spawn(inv_with(&[("fan_tie", 1)])).id();
    app.world_mut()
        .send_event(CraftCancelIntent { caster: player });
    app.update();

    assert!(
        current_failed_events(&app).is_empty(),
        "无 CraftSession 的取消 intent 应 debug/noop，不应通知 client 失败"
    );
    let inventory = app.world().get::<PlayerInventory>(player).unwrap();
    assert_eq!(
        inventory.containers[0].items.len(),
        1,
        "无 session 的 cancel 不应改动玩家背包"
    );
    assert!(
        app.world().get::<CraftSession>(player).is_none(),
        "无 session 的 cancel 不应插入或保留 CraftSession"
    );
}

#[test]
fn cancel_intent_unknown_recipe_preserves_session_without_terminal_event() {
    let recipe = make_recipe("craft.test.cancel_known_recipe", &[("fan_tie", 2)], vec![]);
    let mut app = craft_refund_test_app(recipe, &[("fan_tie", 64)], 13);
    app.add_systems(Update, apply_craft_cancel_intents);

    let player = app
        .world_mut()
        .spawn(inv_with(&[("fan_tie", 1)]))
        .insert(CraftSession {
            recipe_id: RecipeId::new("craft.test.cancel_missing_recipe"),
            started_at_tick: 10,
            remaining_ticks: 20,
            total_ticks: 40,
            owner_player_id: "offline:Azure".into(),
            qi_paid: 0.0,
            quantity_total: 1,
            completed_count: 0,
        })
        .id();

    app.world_mut()
        .send_event(CraftCancelIntent { caster: player });
    app.update();

    assert!(
        current_failed_events(&app).is_empty(),
        "未知 recipe 尚未终止 session，不能发布语义为终结的 CraftFailedEvent"
    );
    assert!(
        app.world().get::<CraftSession>(player).is_some(),
        "未知 recipe 时无法重建退款 manifest，必须保留 session 避免吞掉预扣材料"
    );
}

#[test]
fn cancel_refund_missing_drop_registry_preserves_session_then_retries_once() {
    let recipe = make_recipe("craft.test.refund_retry", &[("fan_tie", 2)], vec![]);
    let mut app = craft_refund_test_app(recipe, &[("fan_tie", 64)], 455);
    app.add_systems(Update, apply_craft_cancel_intents);
    app.world_mut().remove_resource::<DroppedLootRegistry>();

    let mut inventory = inv_with(&[("occupant", 1)]);
    clamp_main_pack_to_grid(&mut inventory, 1, 1);
    let player = app
        .world_mut()
        .spawn(inventory)
        .insert(Position::new([3.0, 65.0, 4.0]))
        .insert(CraftSession {
            recipe_id: RecipeId::new("craft.test.refund_retry"),
            started_at_tick: 400,
            remaining_ticks: 20,
            total_ticks: 40,
            owner_player_id: canonical_player_id("Azure"),
            qi_paid: 0.0,
            quantity_total: 1,
            completed_count: 0,
        })
        .id();

    app.world_mut()
        .send_event(CraftCancelIntent { caster: player });
    app.update();

    assert!(
        current_failed_events(&app).is_empty(),
        "退款事务未提交时不能下发已取消 outcome"
    );
    assert!(
        app.world().get::<CraftSession>(player).is_some(),
        "缺少落地 registry 时必须保留 session 作为可重试退款凭证"
    );

    app.world_mut()
        .insert_resource(DroppedLootRegistry::default());
    app.world_mut()
        .send_event(CraftCancelIntent { caster: player });
    app.update();

    let failed = current_failed_events(&app);
    assert_eq!(failed.len(), 1, "依赖恢复后重试应只产生一次已提交退款事件");
    assert_eq!(
        failed[0].material_returned, 1,
        "重试应返还配方约定的一个材料"
    );
    assert!(
        app.world().get::<CraftSession>(player).is_none(),
        "退款成功提交后才可移除 session"
    );
    let dropped = app.world().resource::<DroppedLootRegistry>();
    assert_eq!(
        dropped.entries.len(),
        1,
        "跨帧重试只能落地一次，不能复制首次失败的退款"
    );
}

#[test]
fn cancel_refund_full_inventory_drops_to_ground_and_reports_actual_returned() {
    let recipe = make_recipe("craft.test.refund_cancel", &[("fan_tie", 2)], vec![]);
    let mut app = craft_refund_test_app(recipe, &[("fan_tie", 64)], 456);
    app.add_systems(Update, apply_craft_cancel_intents);

    let (client_bundle, _helper) = create_mock_client("Azure");
    let mut inventory = inv_with(&[("occupant", 1)]);
    clamp_main_pack_to_grid(&mut inventory, 1, 1);
    inventory.bone_coins = 77;
    let player = app
        .world_mut()
        .spawn(client_bundle)
        .insert(inventory)
        .insert(Cultivation::default())
        .insert(QiColor::default())
        .insert(Position::new([11.0, 65.0, -2.0]))
        .insert(CurrentDimension(DimensionKind::Tsy))
        .insert(CraftSession {
            recipe_id: RecipeId::new("craft.test.refund_cancel"),
            started_at_tick: 400,
            remaining_ticks: 20,
            total_ticks: 40,
            owner_player_id: canonical_player_id("Azure"),
            qi_paid: 0.0,
            quantity_total: 1,
            completed_count: 0,
        })
        .id();

    app.world_mut()
        .send_event(CraftCancelIntent { caster: player });
    app.update();

    let failed = current_failed_events(&app);
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].reason, CraftFailureReason::PlayerCancelled);
    assert_eq!(
        failed[0].material_returned, 1,
        "material_returned 必须按实际入包 + 落地成功数统计，不能沿用预估数虚报"
    );
    assert_eq!(failed[0].qi_refunded, 0.0, "craft 取消不退真元");
    assert!(
        app.world().get::<CraftSession>(player).is_none(),
        "退款落地成功后 session 应正常结束"
    );

    let inventory = app.world().get::<PlayerInventory>(player).unwrap();
    assert_eq!(inventory.bone_coins, 77, "退款材料不得改动骨币");
    assert!(
        inventory.containers[0]
            .items
            .iter()
            .all(|placed| placed.instance.template_id != "fan_tie"),
        "背包满时 fan_tie 不应被伪造进背包"
    );

    let dropped = app.world().resource::<DroppedLootRegistry>();
    assert_eq!(dropped.entries.len(), 1);
    let entry = dropped.entries.values().next().unwrap();
    assert_eq!(entry.item.template_id, "fan_tie");
    assert_eq!(entry.item.stack_count, 1);
    assert_eq!(entry.world_pos, [11.0, 65.0, -2.0]);
    assert_eq!(entry.dimension, DimensionKind::Tsy);
}

/// plan-bughunt-craft-refund-full-inventory-loss-v1 P4 — `no containers` 是配置/结构
/// 错误（`carried_container_candidate_indices(...).is_empty()`），绝不能被
/// `add_item_to_player_inventory_or_ground` 当作 `inventory full:` 满包成功 fallback 到
/// 地面掉落。Part A 直接命中生产退款入口 `grant_refund_manifest_to_inventory_or_ground`
/// 断言精确错误文案；Part B 走真实 `CraftCancelIntent → apply_craft_cancel_intents`
/// 生产系统，与同批次真正满包的对照玩家一起处理，证明两种错误在同一入口下被正确区分。
#[test]
fn duplicate_cancel_intents_same_frame_refund_only_once() {
    let recipe = make_recipe(
        "craft.test.refund_cancel_duplicate",
        &[("fan_tie", 2)],
        vec![],
    );
    let mut app = craft_refund_test_app(recipe, &[("fan_tie", 64)], 457);
    app.add_systems(Update, apply_craft_cancel_intents);

    let (client_bundle, _helper) = create_mock_client("Azure");
    let mut inventory = inv_with(&[("occupant", 1)]);
    clamp_main_pack_to_grid(&mut inventory, 1, 1);
    let player = app
        .world_mut()
        .spawn(client_bundle)
        .insert(inventory)
        .insert(Position::new([11.0, 65.0, -2.0]))
        .insert(CurrentDimension(DimensionKind::Tsy))
        .insert(CraftSession {
            recipe_id: RecipeId::new("craft.test.refund_cancel_duplicate"),
            started_at_tick: 400,
            remaining_ticks: 20,
            total_ticks: 40,
            owner_player_id: canonical_player_id("Azure"),
            qi_paid: 0.0,
            quantity_total: 1,
            completed_count: 0,
        })
        .id();

    app.world_mut()
        .send_event(CraftCancelIntent { caster: player });
    app.world_mut()
        .send_event(CraftCancelIntent { caster: player });
    app.update();

    let failed = current_failed_events(&app);
    assert_eq!(
        failed.len(),
        1,
        "同帧重复 cancel 只能产生一条失败事件，避免 deferred remove 前重复退款，实际={failed:?}"
    );
    assert_eq!(
        failed[0].material_returned, 1,
        "同帧重复 cancel 的 material_returned 只能统计第一次真实落地退款"
    );
    assert!(
        app.world().get::<CraftSession>(player).is_none(),
        "同帧重复 cancel 处理后 session 应结束"
    );

    let dropped = app.world().resource::<DroppedLootRegistry>();
    assert_eq!(
        dropped.entries.len(),
        1,
        "同帧重复 cancel 满包退款只能产生一个地面掉落，避免复制材料"
    );
    let entry = dropped.entries.values().next().unwrap();
    assert_eq!(entry.item.template_id, "fan_tie");
    assert_eq!(entry.item.stack_count, 1);
}

#[test]
fn finalize_failure_refund_full_inventory_drops_to_ground_without_bone_coin_drift() {
    let recipe = make_recipe("craft.test.refund_finalize", &[("fan_tie", 2)], vec![]);
    let mut app = craft_refund_test_app(recipe, &[("fan_tie", 64), ("out", 64)], 789);
    app.add_systems(Update, tick_craft_sessions);

    let (client_bundle, _helper) = create_mock_client("Azure");
    let mut inventory = inv_with(&[("occupant", 1)]);
    clamp_main_pack_to_grid(&mut inventory, 1, 1);
    inventory.bone_coins = 88;
    let player = app
        .world_mut()
        .spawn(client_bundle)
        .insert(inventory)
        .insert(Position::new([-4.0, 66.0, 9.0]))
        .insert(CurrentDimension(DimensionKind::Overworld))
        .insert(CraftSession {
            recipe_id: RecipeId::new("craft.test.refund_finalize"),
            started_at_tick: 700,
            remaining_ticks: 1,
            total_ticks: 1,
            owner_player_id: canonical_player_id("Azure"),
            qi_paid: 0.0,
            quantity_total: 1,
            completed_count: 0,
        })
        .id();

    app.update();

    assert!(
        current_completed_events(&app).is_empty(),
        "产物入包失败不能发 Completed"
    );
    let failed = current_failed_events(&app);
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].reason, CraftFailureReason::InternalError);
    assert_eq!(
        failed[0].material_returned, 1,
        "产物入包失败后的剩余批次退款也必须统计实际落地成功数"
    );
    assert!(
        app.world().get::<CraftSession>(player).is_none(),
        "finalize 失败退款落地后 session 应结束，不能保留僵尸状态"
    );

    let inventory = app.world().get::<PlayerInventory>(player).unwrap();
    assert_eq!(inventory.bone_coins, 88, "材料退款不得增减骨币");
    assert!(
        inventory.containers[0]
            .items
            .iter()
            .all(|placed| placed.instance.template_id != "fan_tie"
                && placed.instance.template_id != "out"),
        "满包时产物和退款材料都不应被写进已满容器"
    );

    let dropped = app.world().resource::<DroppedLootRegistry>();
    assert_eq!(dropped.entries.len(), 1);
    let entry = dropped.entries.values().next().unwrap();
    assert_eq!(
        entry.item.template_id, "fan_tie",
        "落地兜底只用于退款材料；产物 grant 失败仍按失败事件处理"
    );
    assert_eq!(entry.item.stack_count, 1);
    assert_eq!(entry.world_pos, [-4.0, 66.0, 9.0]);
    assert_eq!(entry.dimension, DimensionKind::Overworld);
}

#[test]
fn finalize_missing_recipe_preserves_completed_session_without_terminal_event() {
    let recipe = make_recipe("craft.test.known", &[("fan_tie", 2)], vec![]);
    let mut app = craft_refund_test_app(recipe, &[("fan_tie", 64)], 790);
    app.add_systems(Update, tick_craft_sessions);

    let (client_bundle, _helper) = create_mock_client("Azure");
    let player = app
        .world_mut()
        .spawn(client_bundle)
        .insert(inv_with(&[]))
        .insert(CraftSession {
            recipe_id: RecipeId::new("craft.test.missing"),
            started_at_tick: 700,
            remaining_ticks: 1,
            total_ticks: 1,
            owner_player_id: canonical_player_id("Azure"),
            qi_paid: 0.0,
            quantity_total: 2,
            completed_count: 0,
        })
        .id();

    app.update();

    assert!(
        current_failed_events(&app).is_empty(),
        "配方缺失时退款 manifest 不可重建，不能发布终止事件后吞掉预扣材料"
    );
    assert!(
        current_completed_events(&app).is_empty(),
        "配方缺失时不能伪造完成事件"
    );
    let session = app.world().get::<CraftSession>(player).unwrap();
    assert_eq!(
        session.remaining_ticks, 0,
        "完成边界应被持久化为可恢复的 remaining_ticks=0 session"
    );
    assert!(
        app.world()
            .get::<CraftSessionPersistenceDirty>(player)
            .is_some(),
        "缺失配方的完成 session 必须标脏等待持久化，不能只留易失 ECS 状态"
    );
}

#[test]
fn build_recipe_list_payload_hides_empty_source_basic_recipe_until_material_unlock() {
    let mut registry = CraftRegistry::new();
    register_basic_processing_recipes(&mut registry).unwrap();

    // 空 unlock_state：空源基础配方不下发。
    let empty = RecipeUnlockState::new();
    let payload = build_recipe_list_payload("offline:Alice", &registry, &empty);
    assert!(
        payload.recipes.iter().all(|r| r.id != "basic.wood_handle"),
        "材料发现解锁前，空源 basic.wood_handle 不应下发"
    );

    // 模拟材料发现解锁后：出现且 unlocked，字段正确。
    let mut unlock_state = RecipeUnlockState::new();
    unlock_state.unlock("offline:Alice", RecipeId::new("basic.wood_handle"));
    let payload = build_recipe_list_payload("offline:Alice", &registry, &unlock_state);
    let wood_handle = payload
        .recipes
        .iter()
        .find(|r| r.id == "basic.wood_handle")
        .expect("解锁后 basic.wood_handle 应出现在列表");
    assert!(wood_handle.unlocked);
    assert_eq!(wood_handle.display_name, "削木柄");
    assert_eq!(wood_handle.materials, vec![("crude_wood".to_string(), 2)]);
    assert_eq!(wood_handle.output, ("wood_handle".to_string(), 2));
}

#[test]
fn material_discoverable_recipe_list_fits_server_data_budget() {
    // 本 PR 关心的不变式：材料发现可达集合 = 全部空源配方。一个靠采集解锁了
    // 所有 gather-able 配方的玩家，其 CraftRecipeList 仍能单包下发（不超预算）。
    //
    // 注（既有限制，不在本 PR 范围）：把残卷/师承/顿悟门控配方也全部解锁后的
    // "终态全表"目前约 41KB，超过单包 MAX_PAYLOAD_BYTES(32KB)，需要 CraftRecipeList
    // 分页 / 增量下发来根治。这是 plan-craft-v1 既有的 payload 设计待办，材料发现
    // 改动并未抬高这一上限（终态全表集合与改动前一致）。
    let mut app = App::new();
    app.insert_resource(
        bong_server::inventory::load_item_registry()
            .expect("craft emission test requires ItemRegistry"),
    );
    bong_server::craft::register(&mut app);
    let registry = app.world().resource::<CraftRegistry>();
    let mut unlock_state = RecipeUnlockState::new();
    for r in registry.iter() {
        if r.unlock_sources.is_empty() {
            unlock_state.unlock("offline:Alice", r.id.clone());
        }
    }
    let payload = ServerDataV1::new(ServerDataPayloadV1::CraftRecipeList(Box::new(
        build_recipe_list_payload("offline:Alice", registry, &unlock_state),
    )));

    let bytes = serialize_server_data_payload(&payload)
        .expect("material-discoverable craft recipe list must fit server_data budget");
    assert!(
        bytes.len() <= bong_server::schema::common::MAX_PAYLOAD_BYTES,
        "空源（材料发现）配方全解锁后的列表应在单包预算内，实际 {} 字节 > {}",
        bytes.len(),
        bong_server::schema::common::MAX_PAYLOAD_BYTES
    );
}

#[test]
fn requirements_v1_default_omits_optional_fields_in_payload() {
    let r = CraftRequirementsV1 {
        realm_min: None,
        qi_color_min: None,
        skill_lv_min: None,
    };
    let s = serde_json::to_string(&r).unwrap();
    assert!(!s.contains("realm_min"));
    assert!(!s.contains("qi_color_min"));
    assert!(!s.contains("skill_lv_min"));
    // sanity：requirements 即使全 None 也应该序列化干净
    let _: CraftRequirementsV1 = serde_json::from_str(&s).unwrap();
    // unused 静默
    let _ = CraftRequirements::default;
}

// ── plan-craft-material-discovery — apply_material_discovery_unlock 系统 ──

#[test]
fn material_discovery_skips_player_without_relevant_ingredient() {
    let mut app = App::new();
    let mut registry = CraftRegistry::new();
    register_basic_processing_recipes(&mut registry).unwrap();
    app.insert_resource(registry);
    app.insert_resource(RecipeUnlockState::new());
    app.add_systems(Update, apply_material_discovery_unlock);

    let (client_bundle, mut helper) = create_mock_client("Bob");
    let entity = app.world_mut().spawn(client_bundle).id();
    app.world_mut()
        .entity_mut(entity)
        .insert(inv_with(&[("unobtainium_xyz", 1)]));
    app.update();
    flush_client_packets(&mut app);

    let player_id = canonical_player_id("Bob");
    let unlock_state = app.world().resource::<RecipeUnlockState>();
    assert_eq!(
        unlock_state.unlocked_count(&player_id),
        0,
        "背包无任何配方原料时不应解锁配方"
    );
    assert!(
        collect_recipe_lists(&mut helper).is_empty(),
        "无新解锁时不应重推配方列表"
    );
}

#[test]
fn material_discovery_does_not_unlock_explicit_source_recipe() {
    // 同一原料 fan_tie：空源配方解锁，scroll 门控的秘传配方不解锁（worldview §九）。
    let mut app = App::new();
    let mut registry = CraftRegistry::new();
    registry
        .register(make_recipe("craft.open.tool", &[("fan_tie", 1)], vec![]))
        .unwrap();
    registry
        .register(make_recipe(
            "craft.secret.tool",
            &[("fan_tie", 1)],
            vec![UnlockSource::Scroll {
                item_template: "scroll_secret".into(),
            }],
        ))
        .unwrap();
    app.insert_resource(registry);
    app.insert_resource(RecipeUnlockState::new());
    app.add_systems(Update, apply_material_discovery_unlock);

    let (client_bundle, _helper) = create_mock_client("Cleo");
    let entity = app.world_mut().spawn(client_bundle).id();
    app.world_mut()
        .entity_mut(entity)
        .insert(inv_with(&[("fan_tie", 5)]));
    app.update();

    let player_id = canonical_player_id("Cleo");
    let unlock_state = app.world().resource::<RecipeUnlockState>();
    assert!(
        unlock_state.is_unlocked(&player_id, &RecipeId::new("craft.open.tool")),
        "空源配方应被材料发现解锁"
    );
    assert!(
        !unlock_state.is_unlocked(&player_id, &RecipeId::new("craft.secret.tool")),
        "scroll 门控的秘传配方不应因持有原料而解锁"
    );
}

#[test]
fn material_discovery_isolates_unlocks_per_player() {
    // 两个在线玩家持不同原料，各自只解锁与自己原料匹配的空源配方，互不污染。
    let mut app = App::new();
    let mut registry = CraftRegistry::new();
    registry
        .register(make_recipe("craft.open.alpha", &[("fan_tie", 1)], vec![]))
        .unwrap();
    registry
        .register(make_recipe("craft.open.beta", &[("zhu_pi", 1)], vec![]))
        .unwrap();
    app.insert_resource(registry);
    app.insert_resource(RecipeUnlockState::new());
    app.add_systems(Update, apply_material_discovery_unlock);

    let (alice_bundle, _h1) = create_mock_client("Alice");
    let alice = app.world_mut().spawn(alice_bundle).id();
    app.world_mut()
        .entity_mut(alice)
        .insert(inv_with(&[("fan_tie", 2)]));
    let (bob_bundle, _h2) = create_mock_client("Bob");
    let bob = app.world_mut().spawn(bob_bundle).id();
    app.world_mut()
        .entity_mut(bob)
        .insert(inv_with(&[("zhu_pi", 2)]));

    app.update();

    let unlock_state = app.world().resource::<RecipeUnlockState>();
    let alice_id = canonical_player_id("Alice");
    let bob_id = canonical_player_id("Bob");
    assert!(
        unlock_state.is_unlocked(&alice_id, &RecipeId::new("craft.open.alpha")),
        "Alice 持 fan_tie 应解锁 alpha"
    );
    assert!(
        !unlock_state.is_unlocked(&alice_id, &RecipeId::new("craft.open.beta")),
        "Alice 无 zhu_pi 不应解锁 beta"
    );
    assert!(
        unlock_state.is_unlocked(&bob_id, &RecipeId::new("craft.open.beta")),
        "Bob 持 zhu_pi 应解锁 beta"
    );
    assert!(
        !unlock_state.is_unlocked(&bob_id, &RecipeId::new("craft.open.alpha")),
        "Bob 无 fan_tie 不应解锁 alpha"
    );
}

#[test]
fn material_discovery_narrates_multiple_recipes_in_one_frame() {
    // 边界：同帧背包同时持有多种空源配方原料 → 命中 newly.len() > 1 的
    // 多配方 narration 分支「悟得 N 种新制法：…」，合并为一条定向 narration。
    let mut app = App::new();
    let mut registry = CraftRegistry::new();
    registry
        .register(make_recipe("craft.open.alpha", &[("fan_tie", 1)], vec![]))
        .unwrap();
    registry
        .register(make_recipe("craft.open.beta", &[("zhu_pi", 1)], vec![]))
        .unwrap();
    app.insert_resource(registry);
    app.insert_resource(RecipeUnlockState::new());
    app.insert_resource(PendingGameplayNarrations::default());
    app.add_systems(Update, apply_material_discovery_unlock);

    let (client_bundle, _helper) = create_mock_client("Duke");
    let entity = app.world_mut().spawn(client_bundle).id();
    app.world_mut()
        .entity_mut(entity)
        .insert(inv_with(&[("fan_tie", 1), ("zhu_pi", 1)]));

    app.update();

    let narr = app
        .world_mut()
        .resource_mut::<PendingGameplayNarrations>()
        .drain();
    assert_eq!(narr.len(), 1, "同帧多配方解锁应合并为恰好一条 narration");
    assert_eq!(
        narr[0].target.as_deref(),
        Some("Duke"),
        "narration 应定向到该玩家"
    );
    let text = &narr[0].text;
    assert!(
        text.starts_with("悟得 2 种新制法："),
        "应走多配方分支并带数量，实际={text:?}"
    );
    // registry 迭代顺序不定，故只断言两个配方名都在（不绑定顺序）。
    assert!(
        text.contains("craft.open.alpha") && text.contains("craft.open.beta"),
        "应列出两个解锁配方名，实际={text:?}"
    );
    assert!(
        text.ends_with("。"),
        "narration 文案应以句号收尾，实际={text:?}"
    );
}
