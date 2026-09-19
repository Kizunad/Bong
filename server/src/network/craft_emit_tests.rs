#![allow(dead_code, unused_imports)]

use super::*;
use crate::craft::{
    register_basic_processing_recipes, register_examples, CraftCategory, CraftRecipe,
    CraftRequirements, CraftSession, RecipeId, RecipeUnlockState, UnlockSource,
};
use crate::cultivation::tick::CultivationClock;
use crate::inventory::{
    ContainerState, DroppedLootRegistry, InventoryInstanceIdAllocator, InventoryRevision,
    ItemCategory, ItemInstance, ItemRarity, ItemRegistry, ItemTemplate, PlacedItemState,
    JS_SAFE_INTEGER_MAX,
};
use crate::persistence::bootstrap_sqlite;
use crate::player::state::{load_player_slices, save_player_state, PlayerState};
use crate::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;
use crate::qi_physics::ledger::{pending_inflow_account, QiAccountId, QiTransferReason};
use crate::world::events::ActiveEventsResource;
use crate::world::heartbeat;
use crate::world::zone::{Zone, ZoneRegistry};
use crate::worldgen::pseudo_vein::TICKS_PER_MINUTE;
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
                cast_duration_ms: crate::inventory::DEFAULT_CAST_DURATION_MS,
                cooldown_ms: crate::inventory::DEFAULT_COOLDOWN_MS,
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
                wearer_race: crate::body_plan::types::RaceGateOwned::default(),
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
fn build_session_state_inactive() {
    let state = build_session_state_payload("offline:Alice", None);
    assert!(!state.active);
    assert!(state.recipe_id.is_none());
    assert_eq!(state.elapsed_ticks, 0);
    assert_eq!(state.total_ticks, 0);
}

#[test]
fn build_session_state_active_reflects_elapsed() {
    let session = CraftSession {
        recipe_id: RecipeId::new("craft.test.x"),
        started_at_tick: 0,
        remaining_ticks: 30,
        total_ticks: 100,
        owner_player_id: "offline:Alice".into(),
        qi_paid: 5.0,
        quantity_total: 1,
        completed_count: 0,
    };
    let state = build_session_state_payload("offline:Alice", Some(&session));
    assert!(state.active);
    assert_eq!(state.recipe_id.as_deref(), Some("craft.test.x"));
    assert_eq!(state.elapsed_ticks, 70);
    assert_eq!(state.total_ticks, 100);
    assert_eq!(state.completed_count, 0);
    assert_eq!(state.total_count, 1);
}

#[test]
fn build_session_state_completed_session_shows_full_elapsed() {
    let session = CraftSession {
        recipe_id: RecipeId::new("craft.test.y"),
        started_at_tick: 0,
        remaining_ticks: 0,
        total_ticks: 100,
        owner_player_id: "offline:Bob".into(),
        qi_paid: 5.0,
        quantity_total: 1,
        completed_count: 0,
    };
    let state = build_session_state_payload("offline:Bob", Some(&session));
    assert_eq!(state.elapsed_ticks, 100);
    assert_eq!(state.total_ticks, 100);
    assert_eq!(state.completed_count, 0);
    assert_eq!(state.total_count, 1);
}

#[test]
fn map_failure_reason_covers_all_variants() {
    assert_eq!(
        map_failure_reason(CraftFailureReason::PlayerCancelled),
        CraftFailureReasonV1::PlayerCancelled
    );
    assert_eq!(
        map_failure_reason(CraftFailureReason::PlayerDied),
        CraftFailureReasonV1::PlayerDied
    );
    assert_eq!(
        map_failure_reason(CraftFailureReason::InternalError),
        CraftFailureReasonV1::InternalError
    );
}

#[test]
fn refund_ground_context_falls_back_without_position_or_dimension() {
    let absent = refund_ground_context(None);
    assert_eq!(
        absent.pos, DEFAULT_REFUND_GROUND_POS,
        "缺少 Position 时应使用固定退款落地点，不能依赖无效玩家坐标"
    );
    assert_eq!(
        absent.dimension,
        DimensionKind::default(),
        "缺少 Position 时 dimension 也应回退到默认维度"
    );

    let pos = Position::new([12.0, 66.0, -9.0]);
    let missing_dimension = refund_ground_context(Some((&pos, None)));
    assert_eq!(
        missing_dimension.pos,
        [12.0, 66.0, -9.0],
        "有 Position 时应保留玩家坐标作为退款落地点"
    );
    assert_eq!(
        missing_dimension.dimension,
        DimensionKind::default(),
        "缺少 CurrentDimension 时应使用默认维度"
    );

    let dimension = CurrentDimension(DimensionKind::Tsy);
    let populated = refund_ground_context(Some((&pos, Some(&dimension))));
    assert_eq!(
        populated.pos,
        [12.0, 66.0, -9.0],
        "Position 与 CurrentDimension 齐全时应原样使用玩家坐标作为退款落地点"
    );
    assert_eq!(
        populated.dimension,
        DimensionKind::Tsy,
        "Position 与 CurrentDimension 齐全时应原样使用玩家所在维度"
    );
}

#[test]
fn refund_manifest_full_inventory_drops_to_ground() {
    let registry = registry_with_templates(&[("fan_tie", 64)]);
    let mut inventory = inv_with(&[("occupant", 1)]);
    clamp_main_pack_to_grid(&mut inventory, 1, 1);
    let mut allocator = InventoryInstanceIdAllocator::new(20);
    let mut dropped_loot = DroppedLootRegistry::default();
    let ground_pos = [7.0, 65.0, -3.0];

    let summary = grant_refund_manifest_to_inventory_or_ground(
        &mut inventory,
        &registry,
        &mut allocator,
        Some(&mut dropped_loot),
        vec![("fan_tie".to_string(), 1)],
        123,
        RefundGroundTarget {
            pos: ground_pos,
            dimension: DimensionKind::Tsy,
        },
    );

    assert_eq!(summary.material_returned, 1);
    assert_eq!(summary.granted_count, 0);
    assert_eq!(summary.dropped_count, 1);
    assert!(
        summary.errors.is_empty(),
        "满包但 DroppedLootRegistry 可用时不应出现结构性错误：{:?}",
        summary.errors
    );
    assert_eq!(dropped_loot.entries.len(), 1);
    let entry = dropped_loot.entries.values().next().unwrap();
    assert_eq!(entry.item.template_id, "fan_tie");
    assert_eq!(entry.item.stack_count, 1);
    assert_eq!(entry.world_pos, ground_pos);
    assert_eq!(entry.dimension, DimensionKind::Tsy);
    assert!(
        inventory.containers[0]
            .items
            .iter()
            .all(|placed| placed.instance.template_id != "fan_tie"),
        "满包退款应落地，不应写进已满容器"
    );
}

#[test]
fn refund_manifest_mixed_grant_and_drop_counts_actual_returned() {
    let registry = registry_with_templates(&[("fan_tie", 64), ("zhu_pi", 64)]);
    let mut inventory = inv_with(&[("fan_tie", 63), ("occupant", 1)]);
    clamp_main_pack_to_grid(&mut inventory, 2, 1);
    let mut allocator = InventoryInstanceIdAllocator::new(30);
    let mut dropped_loot = DroppedLootRegistry::default();

    let summary = grant_refund_manifest_to_inventory_or_ground(
        &mut inventory,
        &registry,
        &mut allocator,
        Some(&mut dropped_loot),
        vec![("fan_tie".to_string(), 1), ("zhu_pi".to_string(), 1)],
        77,
        RefundGroundTarget {
            pos: [0.0, 70.0, 0.0],
            dimension: DimensionKind::Overworld,
        },
    );

    assert_eq!(
        summary.material_returned, 2,
        "实际返还数必须统计成功入包 + 成功落地"
    );
    assert_eq!(summary.granted_count, 1);
    assert_eq!(summary.dropped_count, 1);
    assert!(summary.errors.is_empty());
    let fan_tie_stack = inventory.containers[0]
        .items
        .iter()
        .find(|placed| placed.instance.template_id == "fan_tie")
        .expect("fan_tie refund should merge into existing stack");
    assert_eq!(fan_tie_stack.instance.stack_count, 64);
    assert_eq!(dropped_loot.entries.len(), 1);
    let dropped = dropped_loot.entries.values().next().unwrap();
    assert_eq!(dropped.item.template_id, "zhu_pi");
    assert_eq!(dropped.item.stack_count, 1);
}

#[test]
fn refund_manifest_full_inventory_without_registry_reports_error_without_counting_returned() {
    let registry = registry_with_templates(&[("fan_tie", 64)]);
    let mut inventory = inv_with(&[("occupant", 1)]);
    clamp_main_pack_to_grid(&mut inventory, 1, 1);
    let mut allocator = InventoryInstanceIdAllocator::new(40);

    let summary = grant_refund_manifest_to_inventory_or_ground(
        &mut inventory,
        &registry,
        &mut allocator,
        None,
        vec![("fan_tie".to_string(), 1)],
        1,
        RefundGroundTarget {
            pos: [0.0, 64.0, 0.0],
            dimension: DimensionKind::Overworld,
        },
    );

    assert_eq!(
        summary.material_returned, 0,
        "无 DroppedLootRegistry 且背包满时不能虚报已返还"
    );
    assert_eq!(summary.granted_count, 0);
    assert_eq!(summary.dropped_count, 0);
    assert_eq!(summary.errors.len(), 1);
    assert!(
        summary.errors[0].contains("no DroppedLootRegistry"),
        "错误必须暴露缺少落地兜底的结构问题，实际={:?}",
        summary.errors
    );
}

#[test]
fn refund_manifest_unknown_template_does_not_create_ground_drop() {
    let registry = registry_with_templates(&[("fan_tie", 64)]);
    let mut inventory = inv_with(&[]);
    clamp_main_pack_to_grid(&mut inventory, 1, 1);
    let mut allocator = InventoryInstanceIdAllocator::new(50);
    let mut dropped_loot = DroppedLootRegistry::default();

    let summary = grant_refund_manifest_to_inventory_or_ground(
        &mut inventory,
        &registry,
        &mut allocator,
        Some(&mut dropped_loot),
        vec![("missing_template".to_string(), 1)],
        1,
        RefundGroundTarget {
            pos: [0.0, 64.0, 0.0],
            dimension: DimensionKind::Overworld,
        },
    );

    assert_eq!(summary.material_returned, 0);
    assert_eq!(summary.granted_count, 0);
    assert_eq!(summary.dropped_count, 0);
    assert_eq!(summary.errors.len(), 1);
    assert!(
        summary.errors[0].contains("unknown item template id"),
        "unknown template 是配置错误，不应伪装成满包落地：{:?}",
        summary.errors
    );
    assert!(
        dropped_loot.entries.is_empty(),
        "结构性错误不能产生 DroppedLootRegistry 条目"
    );
    assert!(inventory.containers[0].items.is_empty());
}

#[test]
fn refund_manifest_structural_error_rolls_back_earlier_grants_atomically() {
    let registry = registry_with_templates(&[("fan_tie", 64)]);
    let mut inventory = inv_with(&[("fan_tie", 63)]);
    clamp_main_pack_to_grid(&mut inventory, 1, 1);
    let original_revision = inventory.revision;
    let mut allocator = InventoryInstanceIdAllocator::new(60);
    let mut dropped_loot = DroppedLootRegistry::default();

    let summary = grant_refund_manifest_to_inventory_or_ground(
        &mut inventory,
        &registry,
        &mut allocator,
        Some(&mut dropped_loot),
        vec![
            ("fan_tie".to_string(), 1),
            ("missing_template".to_string(), 1),
        ],
        1,
        RefundGroundTarget {
            pos: [0.0, 64.0, 0.0],
            dimension: DimensionKind::Overworld,
        },
    );

    assert_eq!(
        summary.material_returned, 0,
        "manifest 后项结构错误时前项也不得提交，否则重试会复制已成功项"
    );
    assert_eq!(summary.errors.len(), 1, "应保留唯一结构错误供调用方诊断");
    assert_eq!(
        inventory.containers[0].items[0].instance.stack_count, 63,
        "事务失败必须回滚先前已合并到背包的退款"
    );
    assert_eq!(
        inventory.revision, original_revision,
        "事务失败不应留下虚假的 inventory revision 变化"
    );
    assert!(
        dropped_loot.entries.is_empty(),
        "事务失败不得留下部分地面掉落"
    );
    assert_eq!(
        allocator.next_id().unwrap(),
        60,
        "事务失败必须回滚 instance id allocator，避免无效退款消耗 id"
    );
}

#[test]
fn refund_manifest_allocator_boundary_rolls_back_without_drop_id_collision() {
    let registry = registry_with_templates(&[("fan_tie", 64), ("zhu_pi", 64)]);
    let mut inventory = inv_with(&[("occupant", 1)]);
    clamp_main_pack_to_grid(&mut inventory, 1, 1);
    let mut allocator = InventoryInstanceIdAllocator::new(JS_SAFE_INTEGER_MAX);
    let mut dropped_loot = DroppedLootRegistry::default();

    let summary = grant_refund_manifest_to_inventory_or_ground(
        &mut inventory,
        &registry,
        &mut allocator,
        Some(&mut dropped_loot),
        vec![("fan_tie".to_string(), 1), ("zhu_pi".to_string(), 1)],
        1,
        RefundGroundTarget {
            pos: [0.0, 64.0, 0.0],
            dimension: DimensionKind::Overworld,
        },
    );

    assert_eq!(
        summary.material_returned, 0,
        "第二个掉落无法分配安全 ID 时整批退款必须回滚"
    );
    assert_eq!(summary.errors.len(), 1, "allocator 越界应作为结构错误暴露");
    assert!(
        dropped_loot.entries.is_empty(),
        "allocator 边界失败不得让相同 ID 的后项覆盖前项并虚报两份退款"
    );
    assert_eq!(
        allocator.next_id().unwrap(),
        JS_SAFE_INTEGER_MAX,
        "失败事务应回滚 allocator，保留原始可分配边界 ID"
    );
}

#[test]
fn refund_manifest_rejects_existing_drop_id_collision_without_overwrite() {
    let registry = registry_with_templates(&[("fan_tie", 64), ("zhu_pi", 64)]);
    let mut inventory = inv_with(&[("occupant", 1)]);
    clamp_main_pack_to_grid(&mut inventory, 1, 1);
    let mut dropped_loot = DroppedLootRegistry::default();
    let target = RefundGroundTarget {
        pos: [0.0, 64.0, 0.0],
        dimension: DimensionKind::Overworld,
    };
    let mut allocator = InventoryInstanceIdAllocator::new(60);
    let seeded = grant_refund_manifest_to_inventory_or_ground(
        &mut inventory,
        &registry,
        &mut allocator,
        Some(&mut dropped_loot),
        vec![("fan_tie".to_string(), 1)],
        1,
        target,
    );
    assert_eq!(seeded.dropped_count, 1, "夹具应先落地 instance_id=60");

    let mut colliding_allocator = InventoryInstanceIdAllocator::new(60);
    let summary = grant_refund_manifest_to_inventory_or_ground(
        &mut inventory,
        &registry,
        &mut colliding_allocator,
        Some(&mut dropped_loot),
        vec![("zhu_pi".to_string(), 1)],
        2,
        target,
    );

    assert_eq!(
        summary.material_returned, 0,
        "已有掉落 ID 冲突时不能虚报新退款已返还"
    );
    assert!(
        summary
            .errors
            .iter()
            .any(|error| error.contains("instance id collision")),
        "碰撞错误应保留可诊断原因，实际={:?}",
        summary.errors
    );
    assert_eq!(
        dropped_loot.entries.len(),
        1,
        "碰撞不得新增或覆盖 registry 条目"
    );
    assert_eq!(
        dropped_loot.entries.get(&60).unwrap().item.template_id,
        "fan_tie",
        "新退款不得覆盖已有同 ID 掉落并吞掉旧物品"
    );
}

#[test]
fn crafting_pending_then_heartbeat_zone_inflow_preserves_total_and_skips_full_zone() {
    let mut recipe = make_recipe("craft.tool.workbench", &[("fan_tie", 1)], vec![]);
    recipe.qi_cost = 5.0;
    let mut app = craft_refund_test_app(recipe, &[("fan_tie", 64)], 10);
    app.insert_resource(CultivationClock { tick: 0 });
    app.insert_resource(ActiveEventsResource::default());
    app.add_event::<crate::cultivation::breakthrough::BreakthroughOutcome>();
    app.add_event::<crate::world::events::ZoneCollapsedEvent>();
    heartbeat::register(&mut app);
    crate::network::register_craft_start_runtime_system(&mut app);
    let initial_sink_qi = 0.10;
    let expected_sink_qi = initial_sink_qi + 1.0 / QI_ZONE_UNIT_CAPACITY;
    app.insert_resource(ZoneRegistry {
        spatial_revision: 0,
        zones: vec![
            Zone {
                name: "full_zone".to_string(),
                dimension: DimensionKind::Overworld,
                bounds: (DVec3::new(-50.0, 60.0, -50.0), DVec3::new(50.0, 90.0, 50.0)),
                spirit_qi: 0.25,
                danger_level: 0,
                active_events: Vec::new(),
                patrol_anchors: Vec::new(),
                blocked_tiles: Vec::new(),
                qi_equilibrium: 0.25,
                qi_inflow_per_min: 100.0,
            },
            Zone {
                name: "craft_sink".to_string(),
                dimension: DimensionKind::Overworld,
                bounds: (
                    DVec3::new(100.0, 60.0, -50.0),
                    DVec3::new(200.0, 90.0, 50.0),
                ),
                spirit_qi: initial_sink_qi,
                danger_level: 0,
                active_events: Vec::new(),
                patrol_anchors: Vec::new(),
                blocked_tiles: Vec::new(),
                qi_equilibrium: 0.30,
                qi_inflow_per_min: 1.0,
            },
        ],
    });
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
        crate::craft::preparation::stage_material(&mut inventory, &recipe, 1).unwrap();
    }
    let player_account = QiAccountId::player(canonical_player_id("Azure"));
    let full_account = QiAccountId::zone("full_zone");
    let sink_account = QiAccountId::zone("craft_sink");
    let observed_before = app.world().get::<Cultivation>(player).unwrap().qi_current
        + app.world().resource::<WorldQiAccount>().total()
        + app
            .world()
            .resource::<ZoneRegistry>()
            .zones
            .iter()
            .map(|zone| zone.spirit_qi * QI_ZONE_UNIT_CAPACITY)
            .sum::<f64>();

    app.world_mut().send_event(CraftStartIntent {
        caster: player,
        recipe_id: RecipeId::new("craft.tool.workbench"),
        quantity: 1,
    });
    app.update();

    {
        let ledger = app.world().resource::<WorldQiAccount>();
        assert!(
            !ledger.has_account(&player_account),
            "制作阶段不得留下在线玩家的长期 ledger 镜像"
        );
        assert_eq!(
            ledger.balance(&pending_inflow_account()),
            5.0,
            "heartbeat 前制作消耗应全部停留在待分配池"
        );
        assert!(
            !ledger.has_account(&full_account),
            "制作阶段不得为已满 zone 创建长期 ledger mirror"
        );
        assert!(
            !ledger.has_account(&sink_account),
            "制作阶段不得为目标 zone 创建长期 ledger mirror"
        );
        let cultivation_after = app.world().get::<Cultivation>(player).unwrap();
        let zone_qi = app
            .world()
            .resource::<ZoneRegistry>()
            .zones
            .iter()
            .map(|zone| zone.spirit_qi * QI_ZONE_UNIT_CAPACITY)
            .sum::<f64>();
        assert!(
            (cultivation_after.qi_current + ledger.total() + zone_qi - observed_before).abs()
                < 1e-9,
            "ECS player qi + Zone field + pending 制作阶段必须保持观察总量守恒"
        );
        assert_eq!(
            ledger.transfers().len(),
            1,
            "heartbeat 前账本应只有一笔 Crafting 转账"
        );
        let crafting = &ledger.transfers()[0];
        assert_eq!(
            crafting.from, player_account,
            "第一笔转账必须从制作玩家账户发出"
        );
        assert_eq!(
            crafting.to,
            pending_inflow_account(),
            "第一笔转账必须写入待分配池"
        );
        assert_eq!(
            crafting.reason,
            QiTransferReason::Crafting,
            "第一笔转账必须标记为 Crafting"
        );
        assert_eq!(
            crafting.amount, 5.0,
            "Crafting 转账金额必须等于配方 qi_cost"
        );
    }
    assert!(
        app.world().get::<CraftSession>(player).is_some(),
        "制作阶段成功后必须保留 CraftSession"
    );
    assert!(
        current_failed_events(&app).is_empty(),
        "完整回流链的制作阶段不得发出 CraftFailedEvent"
    );

    app.world_mut().resource_mut::<CultivationClock>().tick = TICKS_PER_MINUTE;
    app.update();

    let zones = app.world().resource::<ZoneRegistry>();
    let full_zone = zones
        .find_zone_by_name("full_zone")
        .expect("full-zone fixture should remain registered");
    let sink_zone = zones
        .find_zone_by_name("craft_sink")
        .expect("sink-zone fixture should remain registered");
    assert_eq!(
        full_zone.spirit_qi, 0.25,
        "已达 equilibrium 的 zone 即使速率很高也不得消费 pending"
    );
    assert!(
        (sink_zone.spirit_qi - expected_sink_qi).abs() < 1e-9,
        "1 分钟 heartbeat 应把 1.0 绝对真元从 pending 滴灌到 craft_sink"
    );

    let ledger = app.world().resource::<WorldQiAccount>();
    assert_eq!(
        ledger.balance(&pending_inflow_account()),
        4.0,
        "一分钟 heartbeat 应从待分配池消费恰好 1 点真元"
    );
    assert!(
        !ledger.has_account(&full_account),
        "heartbeat 不得为已满 zone 创建长期 ledger mirror"
    );
    assert!(
        !ledger.has_account(&sink_account),
        "heartbeat 不得为目标 zone 创建长期 ledger mirror"
    );
    let zone_qi = app
        .world()
        .resource::<ZoneRegistry>()
        .zones
        .iter()
        .map(|zone| zone.spirit_qi * QI_ZONE_UNIT_CAPACITY)
        .sum::<f64>();
    assert!(
        (app.world().get::<Cultivation>(player).unwrap().qi_current + ledger.total() + zone_qi
            - observed_before)
            .abs()
            < 1e-9,
        "Crafting → pending → ZoneInflow 全链路必须保持 ECS、Zone field 与账本总量守恒"
    );
    assert_eq!(
        ledger.transfers().len(),
        2,
        "heartbeat 后账本应恰有 Crafting 与 ZoneInflow 两笔转账"
    );
    let inflow = &ledger.transfers()[1];
    assert_eq!(
        inflow.from,
        pending_inflow_account(),
        "ZoneInflow 必须从待分配池发出"
    );
    assert_eq!(
        inflow.to, sink_account,
        "ZoneInflow 必须写入未达 equilibrium 的目标 zone"
    );
    assert_eq!(
        inflow.reason,
        QiTransferReason::ZoneInflow,
        "heartbeat 回流转账必须标记为 ZoneInflow"
    );
    assert_eq!(
        inflow.amount, 1.0,
        "一分钟 heartbeat 应按 qi_inflow_per_min 转入 1 点真元"
    );
    assert!(
        ledger.transfers().iter().all(|transfer| {
            transfer.reason != QiTransferReason::ZoneInflow || transfer.to != full_account
        }),
        "容量门禁不得给已达 equilibrium 的 full_zone 生成 ZoneInflow"
    );
}

#[test]
fn refund_structural_error_does_not_mask_config_bug() {
    // ── Part A：直接命中生产退款入口，锁死精确错误文案与整批回滚 ──────────
    let registry = registry_with_templates(&[("fan_tie", 64)]);
    let mut inventory_no_containers = inv_with(&[]);
    inventory_no_containers.containers.clear();
    let original_revision = inventory_no_containers.revision;
    let mut allocator = InventoryInstanceIdAllocator::new(70);
    let mut dropped_loot = DroppedLootRegistry::default();

    let summary = grant_refund_manifest_to_inventory_or_ground(
        &mut inventory_no_containers,
        &registry,
        &mut allocator,
        Some(&mut dropped_loot),
        vec![("fan_tie".to_string(), 1)],
        1,
        RefundGroundTarget {
            pos: [0.0, 64.0, 0.0],
            dimension: DimensionKind::Overworld,
        },
    );

    assert_eq!(
        summary.material_returned, 0,
        "no containers 是结构错误而非满包成功，绝不能虚报已返还，实际={}",
        summary.material_returned
    );
    assert_eq!(
        summary.errors.len(),
        1,
        "应保留唯一结构错误供调用方诊断，实际={:?}",
        summary.errors
    );
    assert!(
        summary.errors[0].contains("player inventory has no containers"),
        "no containers 必须原样透传为结构错误，不能被 `inventory full:` fallback 判据吞掉，实际={:?}",
        summary.errors
    );
    assert!(
        dropped_loot.entries.is_empty(),
        "no containers 结构错误绝不能产生 DroppedLootEntry——否则配置 bug 会被掩盖成『满包已掉地上』"
    );
    assert!(
        inventory_no_containers.containers.is_empty(),
        "no containers 分支不应凭空补写容器"
    );
    assert_eq!(
        inventory_no_containers.revision, original_revision,
        "结构错误必须整批回滚：clone staging 不得发布，revision 不能变化"
    );
    assert_eq!(
        allocator.next_id().unwrap(),
        70,
        "结构错误分支必须在触发前就失败，不能消耗有效 instance id"
    );

    // ── Part B：真实 CraftCancelIntent → apply_craft_cancel_intents 生产系统，
    //    no containers 玩家与真正满包玩家同批处理，验证两者被正确区分 ──────────
    let recipe = make_recipe(
        "craft.test.refund_structural_error",
        &[("fan_tie", 2)],
        vec![],
    );
    let mut app = craft_refund_test_app(recipe, &[("fan_tie", 64)], 789);
    app.add_systems(Update, apply_craft_cancel_intents);

    let mut inventory_no_containers_ecs = inv_with(&[]);
    inventory_no_containers_ecs.containers.clear();
    let ecs_original_revision = inventory_no_containers_ecs.revision;
    let player_no_containers = app
        .world_mut()
        .spawn(inventory_no_containers_ecs)
        .insert(Position::new([1.0, 65.0, 1.0]))
        .insert(CraftSession {
            recipe_id: RecipeId::new("craft.test.refund_structural_error"),
            started_at_tick: 700,
            remaining_ticks: 20,
            total_ticks: 40,
            owner_player_id: canonical_player_id("Azure"),
            qi_paid: 0.0,
            quantity_total: 1,
            completed_count: 0,
        })
        .id();

    // 对照组：真正的满包（有容器、格子占满），同一入口必须走地面掉落成功。
    let mut inventory_full = inv_with(&[("occupant", 1)]);
    clamp_main_pack_to_grid(&mut inventory_full, 1, 1);
    let player_full = app
        .world_mut()
        .spawn(inventory_full)
        .insert(Position::new([2.0, 65.0, 2.0]))
        .insert(CraftSession {
            recipe_id: RecipeId::new("craft.test.refund_structural_error"),
            started_at_tick: 700,
            remaining_ticks: 20,
            total_ticks: 40,
            owner_player_id: canonical_player_id("Bob"),
            qi_paid: 0.0,
            quantity_total: 1,
            completed_count: 0,
        })
        .id();

    app.world_mut().send_event(CraftCancelIntent {
        caster: player_no_containers,
    });
    app.world_mut().send_event(CraftCancelIntent {
        caster: player_full,
    });
    app.update();

    let failed = current_failed_events(&app);
    assert_eq!(
        failed.len(),
        1,
        "两个 caster 里只有真正满包的对照组应完成退款事务并发布 Failed(PlayerCancelled)，\
         no containers 那个绝不能被误判成功，实际事件={:?}",
        failed
    );
    assert_eq!(
        failed[0].caster, player_full,
        "唯一发布的 Failed 事件必须属于满包对照组玩家，不能是 no containers 结构错误玩家"
    );
    assert_eq!(
        failed[0].material_returned, 1,
        "满包对照组按 70% 取整应实际返还 1 个材料"
    );

    assert!(
        app.world()
            .get::<CraftSession>(player_no_containers)
            .is_some(),
        "no containers 结构错误退款事务未提交，必须保留 CraftSession 作为可重试凭证，不能被误删"
    );
    assert!(
        app.world().get::<CraftSession>(player_full).is_none(),
        "满包对照组退款成功提交后 session 应正常结束"
    );

    let inventory_after = app
        .world()
        .get::<PlayerInventory>(player_no_containers)
        .unwrap();
    assert_eq!(
        inventory_after.revision, ecs_original_revision,
        "no containers 分支的 clone staging 绝不能发布到真实 PlayerInventory"
    );
    assert!(
        inventory_after.containers.is_empty(),
        "no containers 分支不应被结构错误路径意外补写容器"
    );

    let dropped = app.world().resource::<DroppedLootRegistry>();
    assert_eq!(
        dropped.entries.len(),
        1,
        "只有满包对照组的退款才允许落地，no containers 绝不能贡献 DroppedLootEntry，实际={:?}",
        dropped.entries
    );
    let entry = dropped.entries.values().next().unwrap();
    assert_eq!(
        entry.world_pos,
        [2.0, 65.0, 2.0],
        "唯一的地面掉落必须来自满包对照组玩家坐标，证明 no containers 玩家没有偷偷贡献掉落"
    );
    assert_eq!(entry.item.template_id, "fan_tie");
}

#[test]
fn build_recipe_list_payload_hides_empty_source_recipes_until_unlocked() {
    // plan-craft-material-discovery：空源配方不再"默认下发"。空 unlock_state 下
    // 列表应为空；写入 unlock_state（模拟材料发现）后对应配方才出现且 unlocked。
    let mut registry = CraftRegistry::new();
    register_examples(&mut registry).unwrap();

    let empty = RecipeUnlockState::new();
    let payload = build_recipe_list_payload("offline:Alice", &registry, &empty);
    assert_eq!(payload.player_id, "offline:Alice");
    assert!(
        payload.recipes.is_empty(),
        "未解锁任何配方时列表应为空（空源配方不再默认下发），实际={:?}",
        payload.recipes.iter().map(|r| &r.id).collect::<Vec<_>>()
    );

    let mut unlock_state = RecipeUnlockState::new();
    unlock_state.unlock(
        "offline:Alice",
        RecipeId::new("craft.example.eclipse_needle.iron"),
    );
    let payload = build_recipe_list_payload("offline:Alice", &registry, &unlock_state);
    assert_eq!(payload.recipes.len(), 1);
    assert!(payload
        .recipes
        .iter()
        .any(|r| r.id == "craft.example.eclipse_needle.iron" && r.unlocked));
    // 未解锁的其它配方仍不下发
    assert!(payload
        .recipes
        .iter()
        .all(|r| r.id != "craft.example.poison_decoction.fan"));
}

#[test]
fn build_recipe_list_payload_reflects_partial_unlocks() {
    let mut registry = CraftRegistry::new();
    register_examples(&mut registry).unwrap();
    let mut unlock_state = RecipeUnlockState::new();
    unlock_state.unlock(
        "offline:Alice",
        RecipeId::new("craft.example.fake_skin.light"),
    );
    let payload = build_recipe_list_payload("offline:Alice", &registry, &unlock_state);
    let unlocked = payload
        .recipes
        .iter()
        .find(|r| r.id == "craft.example.fake_skin.light")
        .expect("fake skin recipe should be included");
    assert!(unlocked.unlocked);
}

#[test]
fn build_recipe_list_payload_always_includes_baseline_workbench_recipe() {
    // 基线常显豁免（unlock::BASELINE_RECIPES）：制作台自身配方对空 unlock
    // state 的新玩家必须直接出现在列表里且 unlocked=true —— 它是 workbench
    // 配方树的入口，被材料发现藏住会让玩家不知道有制作台这条路。
    let mut registry = CraftRegistry::new();
    crate::craft::register_workbench_recipes(&mut registry).unwrap();

    let empty = RecipeUnlockState::new();
    let payload = build_recipe_list_payload("offline:Newbie", &registry, &empty);
    let workbench = payload
        .recipes
        .iter()
        .find(|r| r.id == "craft.tool.workbench")
        .unwrap_or_else(|| {
            panic!(
                "期望空 unlock state 下 craft.tool.workbench 仍被下发（基线常显），\
                 实际下发列表={:?}",
                payload.recipes.iter().map(|r| &r.id).collect::<Vec<_>>()
            )
        });
    assert!(
        workbench.unlocked,
        "基线配方下发时 unlocked 字段必须为 true"
    );
    assert_eq!(
        workbench.station, None,
        "制作台自身是手搓配方（station=None），客户端应把它分流到手搓台"
    );
    // 豁免不外溢：注册表里其余 100+ 空源 workbench 配方在空 state 下仍全部隐藏。
    assert_eq!(
        payload.recipes.len(),
        1,
        "空 unlock state 下应只下发基线配方本身，实际={:?}",
        payload.recipes.iter().map(|r| &r.id).collect::<Vec<_>>()
    );
}

#[test]
fn emit_recipe_list_sends_once_to_online_client() {
    let mut app = App::new();
    let mut registry = CraftRegistry::new();
    register_basic_processing_recipes(&mut registry).unwrap();
    // 预先解锁 wood_handle（模拟材料发现已完成），使 join 列表非空可断言。
    let mut unlock_state = RecipeUnlockState::new();
    unlock_state.unlock(
        canonical_player_id("Azure"),
        RecipeId::new("basic.wood_handle"),
    );
    app.insert_resource(registry);
    app.insert_resource(unlock_state);
    app.add_systems(Update, emit_recipe_list_on_join);

    let (client_bundle, mut helper) = create_mock_client("Azure");
    let entity = app.world_mut().spawn(client_bundle).id();
    app.world_mut()
        .entity_mut(entity)
        .insert(PlayerState::default());
    app.update();
    flush_client_packets(&mut app);

    let lists = collect_recipe_lists(&mut helper);
    assert_eq!(lists.len(), 1);
    assert!(lists[0].recipes.iter().any(|r| r.id == "basic.wood_handle"));

    app.update();
    flush_client_packets(&mut app);
    assert!(collect_recipe_lists(&mut helper).is_empty());
}

#[test]
fn emit_recipe_list_on_join_also_sends_idle_session_state_without_active_session() {
    let mut app = App::new();
    let mut registry = CraftRegistry::new();
    register_basic_processing_recipes(&mut registry).unwrap();
    let unlock_state = RecipeUnlockState::new();
    app.insert_resource(registry);
    app.insert_resource(unlock_state);
    app.add_systems(Update, emit_recipe_list_on_join);

    let (client_bundle, mut helper) = create_mock_client("Azure");
    let entity = app.world_mut().spawn(client_bundle).id();
    app.world_mut()
        .entity_mut(entity)
        .insert(PlayerState::default());
    app.update();
    flush_client_packets(&mut app);

    let states = collect_craft_session_states(&mut helper);
    assert_eq!(
        states.len(),
        1,
        "玩家 join 首包必须包含 idle craft_session_state；否则客户端 stale active session 只能靠自身断线清理自愈"
    );
    let state = &states[0];
    assert_eq!(state.player_id, canonical_player_id("Azure"));
    assert!(
        !state.active,
        "无 CraftSession 的新连接必须收到 active=false，实际 state={state:?}"
    );
    assert!(
        state.recipe_id.is_none(),
        "idle session state 不应携带 recipe_id，实际 state={state:?}"
    );
    assert_eq!(state.elapsed_ticks, 0);
    assert_eq!(state.total_ticks, 0);

    app.update();
    flush_client_packets(&mut app);
    assert!(
        collect_craft_session_states(&mut helper).is_empty(),
        "join hydration 已完成后不应每 tick 重复推 idle session state"
    );
}

#[test]
fn emit_recipe_list_on_join_waits_for_hydration_then_sends_active_session_state() {
    let mut app = App::new();
    let mut registry = CraftRegistry::new();
    register_basic_processing_recipes(&mut registry).unwrap();
    let unlock_state = RecipeUnlockState::new();
    app.insert_resource(registry);
    app.insert_resource(unlock_state);
    app.add_systems(Update, emit_recipe_list_on_join);

    let (client_bundle, mut helper) = create_mock_client("Azure");
    let entity = app.world_mut().spawn(client_bundle).id();
    app.update();
    flush_client_packets(&mut app);
    assert!(
        collect_recipe_lists(&mut helper).is_empty(),
        "PlayerState hydration 完成前不得发送 recipe/session 首包并缓存 idle 状态"
    );
    assert!(
        collect_craft_session_states(&mut helper).is_empty(),
        "PlayerState hydration 完成前不得发送错误的 idle craft_session_state"
    );

    app.world_mut().entity_mut(entity).insert((
        PlayerState::default(),
        CraftSession {
            recipe_id: RecipeId::new("basic.wood_handle"),
            started_at_tick: 0,
            remaining_ticks: 20,
            total_ticks: 60,
            owner_player_id: canonical_player_id("Azure"),
            qi_paid: 0.0,
            quantity_total: 3,
            completed_count: 1,
        },
    ));
    app.update();
    flush_client_packets(&mut app);

    let states = collect_craft_session_states(&mut helper);
    assert_eq!(
        states.len(),
        1,
        "玩家 join 首包必须携带当前 active craft session，避免 client 等待下一次 dirty/progress tick"
    );
    let state = &states[0];
    assert!(
        state.active,
        "已有 CraftSession 时 join hydration 必须 active=true"
    );
    assert_eq!(state.recipe_id.as_deref(), Some("basic.wood_handle"));
    assert_eq!(state.elapsed_ticks, 40);
    assert_eq!(state.total_ticks, 60);
    assert_eq!(state.completed_count, 1);
    assert_eq!(state.total_count, 3);
}

#[test]
fn build_recipe_list_payload_grouped_by_category_for_ui_order() {
    let mut registry = CraftRegistry::new();
    register_examples(&mut registry).unwrap();
    // 全解锁后才能看到跨多类别的完整列表，验证类别分组连续（同类别不交错出现）。
    let mut unlock_state = RecipeUnlockState::new();
    unlock_all(&mut unlock_state, "offline:Charlie", &registry);
    let payload = build_recipe_list_payload("offline:Charlie", &registry, &unlock_state);
    let cats: Vec<CraftCategoryV1> = payload.recipes.iter().map(|r| r.category).collect();
    assert!(cats.len() >= 2, "示例配方应覆盖多个类别");
    // grouped_for_ui 不变式：同一 category 必须连续成段（一旦离开某类别不再回来）。
    let mut seen = std::collections::HashSet::new();
    let mut prev: Option<CraftCategoryV1> = None;
    for cat in &cats {
        if prev != Some(*cat) {
            assert!(
                seen.insert(*cat),
                "类别 {cat:?} 非连续出现，违反 grouped_for_ui 分组不变式：{cats:?}"
            );
            prev = Some(*cat);
        }
    }
}

#[test]
fn build_recipe_list_payload_preserves_requirements_qi_color_gate() {
    let mut registry = CraftRegistry::new();
    register_examples(&mut registry).unwrap();
    let mut unlock_state = RecipeUnlockState::new();
    unlock_state.unlock(
        "offline:Y",
        RecipeId::new("craft.example.eclipse_needle.iron"),
    );
    let payload = build_recipe_list_payload("offline:Y", &registry, &unlock_state);
    let needle = payload
        .recipes
        .iter()
        .find(|r| r.id == "craft.example.eclipse_needle.iron")
        .expect("eclipse_needle entry");
    assert!(needle.requirements.qi_color_min.is_some());
}

#[test]
fn material_discovery_unlocks_empty_source_recipe_and_repushes_list() {
    let mut app = App::new();
    let mut registry = CraftRegistry::new();
    // basic.wood_handle：空源、原料 crude_wood
    register_basic_processing_recipes(&mut registry).unwrap();
    app.insert_resource(registry);
    app.insert_resource(RecipeUnlockState::new());
    app.add_systems(Update, apply_material_discovery_unlock);

    let (client_bundle, mut helper) = create_mock_client("Azure");
    let entity = app.world_mut().spawn(client_bundle).id();
    app.world_mut()
        .entity_mut(entity)
        .insert(inv_with(&[("crude_wood", 3)]));
    app.update();
    flush_client_packets(&mut app);

    let player_id = canonical_player_id("Azure");
    let unlock_state = app.world().resource::<RecipeUnlockState>();
    assert!(
        unlock_state.is_unlocked(&player_id, &RecipeId::new("basic.wood_handle")),
        "持有 crude_wood 应被动解锁空源 basic.wood_handle"
    );
    let lists = collect_recipe_lists(&mut helper);
    assert!(
        lists.iter().any(|l| l
            .recipes
            .iter()
            .any(|r| r.id == "basic.wood_handle" && r.unlocked)),
        "材料发现后应重推一次含已解锁 wood_handle 的 CraftRecipeList，实际 lists={lists:?}"
    );
}

#[test]
fn material_discovery_idempotent_across_ticks_and_narrates_once() {
    // A→A 转移：每 tick 跑的系统，首帧解锁并推列表 + 一条 narration；
    // 第二帧已解锁，不应重复推 CraftRecipeList、也不应重复写 narration。
    let mut app = App::new();
    let mut registry = CraftRegistry::new();
    register_basic_processing_recipes(&mut registry).unwrap();
    app.insert_resource(registry);
    app.insert_resource(RecipeUnlockState::new());
    app.insert_resource(PendingGameplayNarrations::default());
    app.add_systems(Update, apply_material_discovery_unlock);

    let (client_bundle, mut helper) = create_mock_client("Azure");
    let entity = app.world_mut().spawn(client_bundle).id();
    app.world_mut()
        .entity_mut(entity)
        .insert(inv_with(&[("crude_wood", 3)]));

    // ── tick 1：首次解锁 ──
    app.update();
    flush_client_packets(&mut app);
    let lists_1 = collect_recipe_lists(&mut helper);
    assert_eq!(lists_1.len(), 1, "首帧应推一次 CraftRecipeList");
    let narr_1 = app
        .world_mut()
        .resource_mut::<PendingGameplayNarrations>()
        .drain();
    assert_eq!(narr_1.len(), 1, "首次解锁应恰好产生一条 narration");
    assert_eq!(
        narr_1[0].target.as_deref(),
        Some("Azure"),
        "narration 应定向到该玩家"
    );
    assert!(
        narr_1[0].text.contains("削木柄"),
        "narration 应点名解锁的配方，实际={:?}",
        narr_1[0].text
    );

    // ── tick 2：A→A，已解锁不重复 ──
    app.update();
    flush_client_packets(&mut app);
    assert!(
        collect_recipe_lists(&mut helper).is_empty(),
        "第二帧不应重复推送 CraftRecipeList"
    );
    let narr_2 = app
        .world_mut()
        .resource_mut::<PendingGameplayNarrations>()
        .drain();
    assert!(narr_2.is_empty(), "第二帧不应重复写 narration");
}
