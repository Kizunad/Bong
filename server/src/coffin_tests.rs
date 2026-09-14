#![allow(dead_code, unused_imports)]

use super::*;

// ───────────────────────── plan-coffin-tiers-v1 P2 ─────────────────────────

use crate::craft::{CraftRegistry, ReclaimMode};
use crate::world::dimension::OverworldLayer;
use crate::world::entity_model::{
    BongVisualEntity, COFFIN_BRONZE_ENTITY_KIND, COFFIN_JADE_ENTITY_KIND,
    COFFIN_MUNDANE_ENTITY_KIND, COFFIN_STONE_ENTITY_KIND,
};
use valence::prelude::{App, EntityKind, Update};

const ALL_GRADES: [CoffinGrade; 4] = [
    CoffinGrade::Mundane,
    CoffinGrade::Jade,
    CoffinGrade::Stone,
    CoffinGrade::Bronze,
];

fn craft_registry_with_coffin() -> CraftRegistry {
    let mut registry = CraftRegistry::new();
    register_craft_recipes(&mut registry).expect("coffin mundane recipe should register");
    // P4: jade/stone/bronze 配方由 craft 数据资产加载
    crate::craft::register_workbench_recipes(&mut registry)
        .expect("workbench recipes should register (includes P4 coffin tiers)");
    registry
}

fn expected_entity_kind(grade: CoffinGrade) -> EntityKind {
    match grade {
        CoffinGrade::Mundane => COFFIN_MUNDANE_ENTITY_KIND,
        CoffinGrade::Jade => COFFIN_JADE_ENTITY_KIND,
        CoffinGrade::Stone => COFFIN_STONE_ENTITY_KIND,
        CoffinGrade::Bronze => COFFIN_BRONZE_ENTITY_KIND,
    }
}

/// 构造一个跑 recovery 系统的 app + 一个打了 OverworldLayer 标记的 layer 实体。
fn app_with_recovery() -> (App, Entity) {
    let mut app = App::new();
    app.add_systems(Update, rebuild_missing_coffin_markers);
    app.insert_resource(CoffinRegistry::default());
    // 裸 layer 实体 + OverworldLayer 标记（marker spawn 只需 layer Entity id）。
    let layer = app.world_mut().spawn(OverworldLayer).id();
    (app, layer)
}

#[test]
fn registry_set_marker_entity_updates_both_indices() {
    let mut registry = CoffinRegistry::default();
    let lower = BlockPos::new(4, 64, 4);
    let upper = coffin_upper_half(lower);
    assert!(registry.insert(lower, 0, CoffinGrade::Mundane));
    // 初始无 marker。
    assert_eq!(registry.lookup(lower).unwrap().marker_entity, None);

    let marker = Entity::from_raw(99);
    assert!(registry.set_marker_entity(lower, Some(marker)));
    // lower / upper 两索引都应更新到同一 marker。
    assert_eq!(registry.lookup(lower).unwrap().marker_entity, Some(marker));
    assert_eq!(registry.lookup(upper).unwrap().marker_entity, Some(marker));
}

#[test]
fn coffin_recipe_id_matches_register_namespace() {
    // coffin_recipe_id(Mundane) 必须命中 register_craft_recipes 注册的真实 id。
    let registry = craft_registry_with_coffin();
    assert!(
        registry
            .get(&coffin_recipe_id(CoffinGrade::Mundane))
            .is_some(),
        "coffin_recipe_id(Mundane) 应命中 `coffin.mundane_coffin` 配方"
    );
}

#[test]
fn compute_reclaim_drops_mundane_break_returns_partial_materials() {
    // Mundane 有配方 → Break 返还 ⊆ 配方原料，且数量 ∈ [0,count]。
    let registry = craft_registry_with_coffin();
    let recipe = registry
        .get(&coffin_recipe_id(CoffinGrade::Mundane))
        .expect("mundane recipe");
    for seed in 0u64..50 {
        let drops =
            compute_coffin_reclaim_drops(&registry, CoffinGrade::Mundane, ReclaimMode::Break, seed);
        for (template, count) in &drops {
            let original = recipe
                .materials
                .iter()
                .find(|(t, _)| t == template)
                .map(|(_, c)| *c)
                .unwrap_or_else(|| panic!("drop {template} 不在 mundane 配方原料中"));
            assert!(
                *count >= 1 && *count <= original,
                "Break 返还 {template}={count} 应在 [1,{original}]（已过滤 0），seed={seed}"
            );
        }
    }
}

#[test]
fn compute_reclaim_drops_mundane_reclaim_returns_full_materials() {
    // Reclaim = full 返还，逐项等于 mundane 配方原料。
    let registry = craft_registry_with_coffin();
    let recipe = registry
        .get(&coffin_recipe_id(CoffinGrade::Mundane))
        .expect("mundane recipe")
        .clone();
    let drops =
        compute_coffin_reclaim_drops(&registry, CoffinGrade::Mundane, ReclaimMode::Reclaim, 1);
    assert_eq!(
        drops, recipe.materials,
        "Reclaim 应 full 返还 mundane 配方原料（含顺序）"
    );
}

#[test]
fn compute_reclaim_drops_all_grades_have_recipes_after_p4() {
    // plan-coffin-tiers-v1 P4 完成后，jade/stone/bronze 均已注册配方 →
    // Reclaim 模式返还非空；Break 模式可能随机返还空，但不 panic。
    let registry = craft_registry_with_coffin();
    for grade in [CoffinGrade::Jade, CoffinGrade::Stone, CoffinGrade::Bronze] {
        // 配方应存在
        assert!(
            registry.get(&coffin_recipe_id(grade)).is_some(),
            "{grade:?} 配方应在 P4 注册，registry 中找不到"
        );
        // Reclaim（全量）返还非空
        let drops_reclaim =
            compute_coffin_reclaim_drops(&registry, grade, ReclaimMode::Reclaim, 42);
        assert!(
            !drops_reclaim.is_empty(),
            "{grade:?} Reclaim 模式应返还非空（配方已注册），实得空"
        );
        // Break（随机部分）不 panic（不校验非空，随机可能 0 项）
        let _ = compute_coffin_reclaim_drops(&registry, grade, ReclaimMode::Break, 42);
    }
}

#[test]
fn reclaim_seed_is_deterministic_for_same_pos_and_tick() {
    let lower = BlockPos::new(3, 64, 7);
    assert_eq!(
        coffin_reclaim_seed(lower, 100),
        coffin_reclaim_seed(lower, 100),
        "同棺位 + 同 tick 的 reclaim seed 必须确定性一致"
    );
    assert_ne!(
        coffin_reclaim_seed(lower, 100),
        coffin_reclaim_seed(lower, 101),
        "不同 tick 应产生不同 seed（破坏返还有抖动）"
    );
}

#[test]
fn recovery_rebuilds_marker_for_each_grade_with_aligned_visual_kind() {
    // 四档：注册棺（marker_entity=None）→ 跑 recovery → marker 被重建且 EntityKind
    // 与 grade 对齐（161/162/163/164）。这同时锁定 放置→marker 的 grade→视觉链。
    for grade in ALL_GRADES {
        let (mut app, _layer) = app_with_recovery();
        let lower = BlockPos::new(0, 64, 0);
        {
            let mut registry = app.world_mut().resource_mut::<CoffinRegistry>();
            registry.insert(lower, 0, grade);
            assert_eq!(registry.lookup(lower).unwrap().marker_entity, None);
        }

        app.update();

        let marker = app
            .world()
            .resource::<CoffinRegistry>()
            .lookup(lower)
            .unwrap()
            .marker_entity
            .unwrap_or_else(|| panic!("{grade:?}: recovery 应重建 marker 并回写 registry"));
        assert_eq!(
            app.world().get::<EntityKind>(marker).copied(),
            Some(expected_entity_kind(grade)),
            "{grade:?}: marker 的 EntityKind 应对齐档级 raw_id"
        );
        // BongVisualEntity.kind 也应对齐 grade.visual_kind()。
        let visual = app
            .world()
            .get::<BongVisualEntity>(marker)
            .expect("marker should carry BongVisualEntity");
        assert_eq!(
            visual.kind,
            grade.visual_kind(),
            "{grade:?}: BongVisualEntity.kind 应等于 grade.visual_kind()"
        );
    }
}

#[test]
fn recovery_is_idempotent_does_not_duplicate_markers() {
    // 已有存活 marker 的棺，再跑 recovery 不应重建（marker id 不变、不新增实体）。
    let (mut app, _layer) = app_with_recovery();
    let lower = BlockPos::new(2, 64, 2);
    app.world_mut()
        .resource_mut::<CoffinRegistry>()
        .insert(lower, 0, CoffinGrade::Jade);
    app.update();
    let first = app
        .world()
        .resource::<CoffinRegistry>()
        .lookup(lower)
        .unwrap()
        .marker_entity
        .expect("first recovery should spawn marker");

    let marker_count_before = app
        .world_mut()
        .query::<&BongVisualEntity>()
        .iter(app.world())
        .count();

    app.update();

    let second = app
        .world()
        .resource::<CoffinRegistry>()
        .lookup(lower)
        .unwrap()
        .marker_entity
        .expect("marker should remain after second update");
    assert_eq!(
        first, second,
        "marker 存活时 recovery 应幂等：marker id 不变"
    );
    let marker_count_after = app
        .world_mut()
        .query::<&BongVisualEntity>()
        .iter(app.world())
        .count();
    assert_eq!(
            marker_count_before, marker_count_after,
            "幂等 recovery 不应新增 marker 实体（before={marker_count_before}, after={marker_count_after}）"
        );
}

#[test]
fn recovery_respawns_marker_after_despawn() {
    // 状态转换：marker 被 despawn（模拟区块卸载 / 手动 despawn）→ 下次 recovery 重建。
    let (mut app, _layer) = app_with_recovery();
    let lower = BlockPos::new(5, 64, 5);
    app.world_mut()
        .resource_mut::<CoffinRegistry>()
        .insert(lower, 0, CoffinGrade::Stone);
    app.update();
    let first = app
        .world()
        .resource::<CoffinRegistry>()
        .lookup(lower)
        .unwrap()
        .marker_entity
        .expect("first marker");

    // despawn marker，但 registry 仍指向旧（已死）实体。
    app.world_mut().despawn(first);
    app.update();

    let second = app
        .world()
        .resource::<CoffinRegistry>()
        .lookup(lower)
        .unwrap()
        .marker_entity
        .expect("recovery should rebuild after despawn");
    assert_ne!(
        first, second,
        "despawn 后 recovery 应 spawn 新 marker（id 不同于已死的旧 marker）"
    );
    assert!(
        app.world().get::<EntityKind>(second).is_some(),
        "重建的 marker 应是有效实体"
    );
}

#[test]
fn recovery_no_overworld_layer_is_noop() {
    // 错误分支：无 OverworldLayer（极早期 / 测试未建 layer）→ recovery 跳过，不 panic。
    let mut app = App::new();
    app.add_systems(Update, rebuild_missing_coffin_markers);
    app.insert_resource(CoffinRegistry::default());
    // 不 spawn OverworldLayer 实体。
    app.world_mut().resource_mut::<CoffinRegistry>().insert(
        BlockPos::new(0, 64, 0),
        0,
        CoffinGrade::Mundane,
    );

    app.update();

    assert_eq!(
        app.world()
            .resource::<CoffinRegistry>()
            .lookup(BlockPos::new(0, 64, 0))
            .unwrap()
            .marker_entity,
        None,
        "无 layer 时 recovery 应跳过，marker_entity 保持 None"
    );
}

#[test]
fn despawn_coffin_marker_none_is_noop() {
    // 幂等：marker=None 时 despawn 不 panic（命令式调用守门）。
    let mut app = App::new();
    // 用一个临时系统驱动 despawn(None)，验证不 panic。
    fn drive(mut commands: Commands) {
        super::despawn_coffin_marker(&mut commands, None);
    }
    app.add_systems(Update, drive);
    app.update();
}

#[test]
fn registry_registers_and_removes_both_halves() {
    let mut registry = CoffinRegistry::default();
    let lower = BlockPos::new(8, 64, 8);
    let upper = coffin_upper_half(lower);

    assert!(registry.insert(lower, 10, CoffinGrade::Mundane));
    assert_eq!(registry.lookup(lower).unwrap().lower, lower);
    assert_eq!(registry.lookup(upper).unwrap().upper, upper);

    let removed = registry.remove_by_pos(upper).expect("coffin should remove");
    assert_eq!(removed.lower, lower);
    assert!(registry.lookup(lower).is_none());
    assert!(registry.lookup(upper).is_none());
}

#[test]
fn coffin_lower_restores_from_pinned_player_position() {
    let lower = BlockPos::new(8, 64, 8);
    assert_eq!(
        coffin_lower_from_player_position(coffin_player_position(lower)),
        lower
    );
}

// ─────────────────── ECS 集成测试：破坏 / 菜单回收完整链路 ───────────────────
//
// 这两个测试锁定 P2 实质重写的核心链路：
//   破坏/回收 → despawn marker 实体 + registry 移除 + inventory grant
// 纯函数 compute_coffin_reclaim_drops 已在上面单元测试中覆盖，
// 这里只断言可观察的 ECS 副作用（实体存活 / registry 状态 / inventory 内容）。

use crate::inventory::{
    ContainerState, InventoryInstanceIdAllocator, InventoryRevision, ItemCategory, ItemRarity,
    ItemRegistry, ItemTemplate, PlayerInventory, DEFAULT_CAST_DURATION_MS, DEFAULT_COOLDOWN_MS,
    MAIN_PACK_CONTAINER_ID,
};
use std::collections::HashMap;
use valence::protocol::packets::play::GameMessageS2c;
use valence::testing::{MockClientHelper, ScenarioSingleClient};

/// 构造只含 ling_mu_ban 和 ling_mu_gun 的 ItemRegistry（mundane 棺配方原料）。
fn make_coffin_item_registry() -> ItemRegistry {
    let mut templates = HashMap::new();
    for template_id in &["ling_mu_ban", "ling_mu_gun"] {
        templates.insert(
            template_id.to_string(),
            ItemTemplate {
                quick_use: false,
                id: template_id.to_string(),
                display_name: template_id.to_string(),
                category: ItemCategory::Misc,
                placeable: None,
                max_stack_count: 64,
                grid_w: 1,
                grid_h: 1,
                base_weight: 0.1,
                rarity: ItemRarity::Common,
                spirit_quality_initial: 1.0,
                description: "test".to_string(),
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
                shelflife_profile: None,
                shield_spec: None,
                shelflife_track: None,
                wearer_race: crate::body_plan::types::RaceGateOwned::default(),
            },
        );
    }
    ItemRegistry::from_map(templates)
}

/// 构造一个含 main_pack（4×4 格，宽松容量）的空背包组件。
fn empty_player_inventory() -> PlayerInventory {
    PlayerInventory {
        triggered_treasures: Vec::new(),
        revision: InventoryRevision(0),
        containers: vec![ContainerState {
            quick_access: false,
            id: MAIN_PACK_CONTAINER_ID.to_string(),
            name: "主背包".to_string(),
            rows: 4,
            cols: 4,
            items: vec![],
            owner_instance_id: None,
        }],
        equipped: HashMap::new(),
        hotbar: Default::default(),
        bone_coins: 0,
        max_weight: 9999.0,
    }
}

/// 在 registry 里注册一格 mundane 棺，spawn 一个带真实 `BongVisualEntity` 组件的
/// marker 实体（与生产路径 `spawn_visual_marker` 一致），并把 marker id 写回 registry。
/// 测试断言的是"despawn 了真实 BongVisual marker"而非裸实体，覆盖 charge-1。
/// 返回 (lower, marker_entity)。
fn register_mundane_coffin_with_marker(
    world: &mut bevy_ecs::world::World,
    lower: BlockPos,
) -> (BlockPos, Entity) {
    let marker = world
        .spawn(BongVisualEntity {
            kind: CoffinGrade::Mundane.visual_kind(),
            source: None,
        })
        .id();
    world
        .resource_mut::<CoffinRegistry>()
        .insert(lower, 0, CoffinGrade::Mundane);
    world
        .resource_mut::<CoffinRegistry>()
        .set_marker_entity(lower, Some(marker));
    (lower, marker)
}

/// 构造含 handle_coffin_breaks 系统 + 所有必要 event / resource 的 App。
/// 玩家实体附带 PlayerInventory + Username。
fn app_with_break_system() -> (App, Entity) {
    let scenario = ScenarioSingleClient::new();
    let mut app = scenario.app;
    let client_entity = scenario.client;

    // 必要 events（系统读/写）。
    app.add_event::<CoffinBreakRequest>();
    app.add_event::<CoffinStateChanged>();
    app.add_event::<PlaySoundRecipeRequest>();

    // CoffinRegistry 和 craft 配方。
    app.insert_resource(CoffinRegistry::default());
    let mut craft = CraftRegistry::new();
    register_craft_recipes(&mut craft).expect("coffin recipes should register");
    app.insert_resource(craft);

    // ItemRegistry + allocator（供 grant_reclaim_drops_to_inventory）。
    app.insert_resource(make_coffin_item_registry());
    app.insert_resource(InventoryInstanceIdAllocator::default());

    // 给客户端实体追加 PlayerInventory（Username 已由 ClientBundle 携带）。
    app.world_mut()
        .entity_mut(client_entity)
        .insert(empty_player_inventory());
    // plan-bughunt-coffin-dimension-gate-v1：这些 helper 构造的场景默认代表「玩家在
    // 主世界操作主世界的棺」这一 happy path；跨维拒绝场景由各测试显式覆盖/移除该组件。
    app.world_mut()
        .entity_mut(client_entity)
        .insert(CurrentDimension(DimensionKind::Overworld));

    // 注册被测系统（裸加，无 .after 约束）。
    app.add_systems(Update, handle_coffin_breaks);

    (app, client_entity)
}

/// 构造含 handle_coffin_menu_reclaim 系统 + 所有必要 event / resource 的 App。
fn app_with_reclaim_system() -> (App, Entity) {
    let scenario = ScenarioSingleClient::new();
    let mut app = scenario.app;
    let client_entity = scenario.client;

    app.add_event::<CoffinMenuReclaimRequest>();
    app.add_event::<CoffinStateChanged>();
    app.add_event::<PlaySoundRecipeRequest>();

    app.insert_resource(CoffinRegistry::default());
    let mut craft = CraftRegistry::new();
    register_craft_recipes(&mut craft).expect("coffin recipes should register");
    app.insert_resource(craft);

    app.insert_resource(make_coffin_item_registry());
    app.insert_resource(InventoryInstanceIdAllocator::default());

    app.world_mut()
        .entity_mut(client_entity)
        .insert(empty_player_inventory());
    // plan-bughunt-coffin-dimension-gate-v1：这些 helper 构造的场景默认代表「玩家在
    // 主世界操作主世界的棺」这一 happy path；跨维拒绝场景由各测试显式覆盖/移除该组件。
    app.world_mut()
        .entity_mut(client_entity)
        .insert(CurrentDimension(DimensionKind::Overworld));

    app.add_systems(Update, handle_coffin_menu_reclaim);

    (app, client_entity)
}

#[test]
fn ecs_coffin_break_despawns_marker_and_grants_partial_materials() {
    // 端到端 ECS 链路：玩家破坏 mundane 棺 →
    //   (a) marker 实体被 despawn；
    //   (b) registry 移除该棺；
    //   (c) 玩家背包收到 Break 返还（材料种类 ⊆ 配方，每种数量 ≤ 配方量）。
    // 棺贴近玩家出生点 (0,0,0)，通过 coffin_target_is_close 近距校验。
    let (mut app, client_entity) = app_with_break_system();
    let lower = BlockPos::new(0, 0, 0);

    let (_lower, marker) = register_mundane_coffin_with_marker(app.world_mut(), lower);

    // CoffinBreakRequest 是显式破坏意图（替代旧 DiggingEvent，marker 实体无方块不可挖）。
    app.world_mut().send_event(CoffinBreakRequest {
        player: client_entity,
        pos: lower,
        tick: 0,
    });

    app.update();

    // (a) marker 实体已被 despawn（world 里取不到）。
    assert!(
        app.world().get_entity(marker).is_none(),
        "破坏后 marker 实体应被 despawn，但 world 里仍可查到 {marker:?}；\
             期望 handle_coffin_breaks 调用 despawn_coffin_marker"
    );

    // (b) registry 里该棺已移除（lower 和 upper 两索引均为 None）。
    let upper = coffin_upper_half(lower);
    assert!(
        app.world()
            .resource::<CoffinRegistry>()
            .lookup(lower)
            .is_none(),
        "破坏后 registry 应移除 lower={lower:?} 索引，但仍存在"
    );
    assert!(
        app.world()
            .resource::<CoffinRegistry>()
            .lookup(upper)
            .is_none(),
        "破坏后 registry 应移除 upper={upper:?} 索引，但仍存在"
    );

    // (c) inventory 收到 Break 返还：种类 ⊆ {ling_mu_ban, ling_mu_gun}，
    //     每种数量 ∈ [0, 配方量]（Break 随机，允许 0 或全量，不断言固定数）。
    let valid_materials: HashMap<&str, u32> = [("ling_mu_ban", 6u32), ("ling_mu_gun", 2u32)]
        .iter()
        .copied()
        .collect();
    let inventory = app
        .world()
        .get::<PlayerInventory>(client_entity)
        .expect("client entity should carry PlayerInventory after break");
    for container in &inventory.containers {
        for placed in &container.items {
            let template_id = placed.instance.template_id.as_str();
            let recipe_max = valid_materials.get(template_id).copied().unwrap_or(0);
            assert!(
                recipe_max > 0,
                "Break 返还了配方外材料 {template_id}；期望仅含 ling_mu_ban / ling_mu_gun"
            );
            assert!(
                placed.instance.stack_count <= recipe_max,
                "Break 返还 {template_id}×{} 超过配方上限 ×{recipe_max}",
                placed.instance.stack_count
            );
        }
    }
}

#[test]
fn ecs_coffin_menu_reclaim_despawns_marker_and_grants_full_materials() {
    // 端到端 ECS 链路：玩家菜单回收 mundane 棺 →
    //   (a) marker 实体被 despawn；
    //   (b) registry 移除该棺；
    //   (c) 玩家背包精确收到 Reclaim 全量（ling_mu_ban×6 + ling_mu_gun×2），
    //       锁定「较全返还」语义（与 Break 随机部分返还形成对比）。
    let (mut app, client_entity) = app_with_reclaim_system();
    // ScenarioSingleClient 玩家默认在原点 (0,0,0)；棺紧贴原点（center ≈ 0.5），
    // 距离远小于共享 6 格欧氏 reach profile，通过近距校验。
    let lower = BlockPos::new(0, 0, 0);

    let (_lower, marker) = register_mundane_coffin_with_marker(app.world_mut(), lower);

    app.world_mut().send_event(CoffinMenuReclaimRequest {
        player: client_entity,
        pos: lower,
        tick: 0,
    });

    app.update();

    // (a) marker 实体已被 despawn。
    assert!(
        app.world().get_entity(marker).is_none(),
        "菜单回收后 marker 实体应被 despawn，但 world 里仍可查到 {marker:?}；\
             期望 handle_coffin_menu_reclaim 调用 despawn_coffin_marker"
    );

    // (b) registry 里该棺已移除。
    let upper = coffin_upper_half(lower);
    assert!(
        app.world()
            .resource::<CoffinRegistry>()
            .lookup(lower)
            .is_none(),
        "回收后 registry 应移除 lower={lower:?}，但仍存在"
    );
    assert!(
        app.world()
            .resource::<CoffinRegistry>()
            .lookup(upper)
            .is_none(),
        "回收后 registry 应移除 upper={upper:?}，但仍存在"
    );

    // (c) 背包精确等于 mundane 配方全量：ling_mu_ban×6 + ling_mu_gun×2。
    let mut totals: HashMap<String, u32> = HashMap::new();
    let inventory = app
        .world()
        .get::<PlayerInventory>(client_entity)
        .expect("client entity should carry PlayerInventory after reclaim");
    for container in &inventory.containers {
        for placed in &container.items {
            *totals
                .entry(placed.instance.template_id.clone())
                .or_insert(0) += placed.instance.stack_count;
        }
    }
    assert_eq!(
        totals.get("ling_mu_ban").copied().unwrap_or(0),
        6,
        "Reclaim 应全量返还 ling_mu_ban×6（较全返还语义）；实得 {:?}",
        totals.get("ling_mu_ban")
    );
    assert_eq!(
        totals.get("ling_mu_gun").copied().unwrap_or(0),
        2,
        "Reclaim 应全量返还 ling_mu_gun×2（较全返还语义）；实得 {:?}",
        totals.get("ling_mu_gun")
    );
    assert_eq!(
        totals.len(),
        2,
        "Reclaim 应恰好返还 2 种材料（ling_mu_ban + ling_mu_gun），实得 {totals:?}"
    );
}

#[test]
fn ecs_coffin_break_rejected_when_player_too_far() {
    // 安全门控回归：玩家距棺超出共享 6 格欧氏 reach profile 时，
    // handle_coffin_breaks 必须拒绝——
    //   (a) registry 仍保有该棺；
    //   (b) marker 实体仍存在；
    //   (c) 背包无返还材料。
    let (mut app, client_entity) = app_with_break_system();
    let lower = BlockPos::new(0, 64, 0);

    let (_lower, marker) = register_mundane_coffin_with_marker(app.world_mut(), lower);

    // 给玩家实体注入 Position，置于远处（x=100，距棺中心约 100 格，>> sqrt(36)=6）。
    app.world_mut()
        .entity_mut(client_entity)
        .insert(Position::new([100.0, 64.0, 100.0]));

    app.world_mut().send_event(CoffinBreakRequest {
        player: client_entity,
        pos: lower,
        tick: 0,
    });

    app.update();

    // (a) registry 里该棺仍在。
    assert!(
        app.world()
            .resource::<CoffinRegistry>()
            .lookup(lower)
            .is_some(),
        "远程 break 被拒后 registry 应仍保有 lower={lower:?}，\
             期望：coffin_target_is_close 返 false → continue 跳过 remove_by_pos"
    );

    // (b) marker 实体仍存在。
    assert!(
        app.world().get_entity(marker).is_some(),
        "远程 break 被拒后 marker 实体应仍存在（{marker:?}），\
             期望：despawn_coffin_marker 未被调用"
    );

    // (c) 背包仍为空（无返还材料）。
    let inventory = app
        .world()
        .get::<PlayerInventory>(client_entity)
        .expect("client entity should have PlayerInventory");
    let total_items: usize = inventory.containers.iter().map(|c| c.items.len()).sum();
    assert_eq!(
        total_items, 0,
        "远程 break 被拒后背包应无任何返还材料，实得 {total_items} 件；\
             期望：break 被 continue 拦截，materials 路径未执行"
    );
}

#[test]
fn ecs_coffin_break_without_inventory_still_removes_coffin() {
    // 边界测试：PlayerInventory=None（玩家背包缺失）时，
    // handle_coffin_breaks 仍应正常 despawn marker + 移除 registry，不 panic，
    // 仅材料静默丢弃（intentional design）。
    let (mut app, client_entity) = app_with_break_system();
    let lower = BlockPos::new(0, 0, 0);

    let (_lower, marker) = register_mundane_coffin_with_marker(app.world_mut(), lower);

    // 移除 PlayerInventory，模拟背包缺失边界。
    app.world_mut()
        .entity_mut(client_entity)
        .remove::<PlayerInventory>();

    app.world_mut().send_event(CoffinBreakRequest {
        player: client_entity,
        pos: lower,
        tick: 0,
    });

    // 断言不 panic（app.update() 不抛出 = 通过）。
    app.update();

    // (a) marker 已 despawn。
    assert!(
        app.world().get_entity(marker).is_none(),
        "无 inventory 时 break 仍应 despawn marker {marker:?}；\
             期望：despawn_coffin_marker 在 inventory 检查前执行"
    );

    // (b) registry 已移除。
    assert!(
        app.world()
            .resource::<CoffinRegistry>()
            .lookup(lower)
            .is_none(),
        "无 inventory 时 break 仍应移除 registry lower={lower:?}；\
             期望：remove_by_pos 在 inventory 检查前执行"
    );
}

// ─── plan-coffin-tiers-v1 P3 E 非静默修复：grant_reclaim_drops_to_inventory ────

#[test]
fn grant_reclaim_drops_to_full_inventory_returns_false_and_does_not_panic() {
    // 满背包时 grant_reclaim_drops_to_inventory 应返回 false（无一成功发还）且不 panic。
    // 填满背包：1×1 格 * 16 格 = 16 个 item，各占满 stack。
    let item_registry = make_coffin_item_registry();
    let mut allocator = InventoryInstanceIdAllocator::default();

    // 构造一个只有 1 格的背包，然后填满它（stack 上限 64）。
    let mut inventory = PlayerInventory {
        triggered_treasures: Vec::new(),
        revision: InventoryRevision(0),
        containers: vec![ContainerState {
            quick_access: false,
            id: MAIN_PACK_CONTAINER_ID.to_string(),
            name: "主背包".to_string(),
            rows: 1,
            cols: 1,
            items: vec![],
            owner_instance_id: None,
        }],
        equipped: HashMap::new(),
        hotbar: Default::default(),
        bone_coins: 0,
        max_weight: 9999.0,
    };
    // 先填满唯一的格（塞满 ling_mu_ban×64）。
    add_item_to_player_inventory(
        &mut inventory,
        &item_registry,
        &mut allocator,
        "ling_mu_ban",
        64,
        0,
    )
    .expect("should fit into empty slot");

    // 再尝试发还更多 ling_mu_ban → 格已满，应失败。
    let drops = vec![("ling_mu_ban".to_string(), 6u32)];
    let granted = grant_reclaim_drops_to_inventory(
        &mut inventory,
        &item_registry,
        &mut allocator,
        "test_player",
        &drops,
        None, // 无 client，只测 return value + 不 panic
    );
    assert!(
        !granted,
        "背包满时 grant_reclaim_drops_to_inventory 应返回 false（无一成功发还），\
             期望：ling_mu_ban 格已满（64/64），追加返回 Err，granted_any 保持 false"
    );
}

#[test]
fn grant_reclaim_drops_empty_drops_returns_false() {
    // 边界：空 drops slice → 返回 false（没有任何成功发还）。
    let item_registry = make_coffin_item_registry();
    let mut allocator = InventoryInstanceIdAllocator::default();
    let mut inventory = empty_player_inventory();
    let granted = grant_reclaim_drops_to_inventory(
        &mut inventory,
        &item_registry,
        &mut allocator,
        "test_player",
        &[],
        None,
    );
    assert!(
        !granted,
        "空 drops 时 grant_reclaim_drops_to_inventory 应返回 false（无成功项），\
             期望：循环体未执行，granted_any 保持初始 false"
    );
}

#[test]
fn grant_reclaim_drops_to_empty_inventory_returns_true() {
    // happy path：空背包足够容纳 drops → 返回 true。
    let item_registry = make_coffin_item_registry();
    let mut allocator = InventoryInstanceIdAllocator::default();
    let mut inventory = empty_player_inventory();
    let drops = vec![("ling_mu_ban".to_string(), 3u32)];
    let granted = grant_reclaim_drops_to_inventory(
        &mut inventory,
        &item_registry,
        &mut allocator,
        "test_player",
        &drops,
        None,
    );
    assert!(
        granted,
        "空背包发还 ling_mu_ban×3 应成功，grant_reclaim_drops_to_inventory 应返回 true"
    );
}

/// app_with_reclaim_system 的变体：同时返回 MockClientHelper 以便断言 S2C chat 消息。
fn app_with_reclaim_system_and_helper() -> (
    valence::prelude::App,
    valence::prelude::Entity,
    MockClientHelper,
) {
    let scenario = ScenarioSingleClient::new();
    let mut app = scenario.app;
    let client_entity = scenario.client;
    let helper = scenario.helper;

    app.add_event::<CoffinMenuReclaimRequest>();
    app.add_event::<CoffinStateChanged>();
    app.add_event::<PlaySoundRecipeRequest>();

    app.insert_resource(CoffinRegistry::default());
    let mut craft = CraftRegistry::new();
    register_craft_recipes(&mut craft).expect("coffin recipes should register");
    app.insert_resource(craft);

    app.insert_resource(make_coffin_item_registry());
    app.insert_resource(InventoryInstanceIdAllocator::default());

    app.world_mut()
        .entity_mut(client_entity)
        .insert(empty_player_inventory());
    // plan-bughunt-coffin-dimension-gate-v1：这些 helper 构造的场景默认代表「玩家在
    // 主世界操作主世界的棺」这一 happy path；跨维拒绝场景由各测试显式覆盖/移除该组件。
    app.world_mut()
        .entity_mut(client_entity)
        .insert(CurrentDimension(DimensionKind::Overworld));

    app.add_systems(valence::prelude::Update, handle_coffin_menu_reclaim);

    (app, client_entity, helper)
}

// ─── CodeRabbit major B: 满包 → chat 提示非静默路径 ────────────────────────────

#[test]
fn ecs_coffin_reclaim_full_inventory_sends_chat_message_and_removes_coffin() {
    // 满背包回收 mundane 棺：
    //   (a) 棺仍被移除（marker despawn + registry 清除）；
    //   (b) 客户端收到 chat 提示（§e[棺] 背包已满…）——锁定"非静默丢失"契约；
    //   (c) 不 panic。
    let lower = BlockPos::new(0, 0, 0);
    let (mut app, client_entity, mut helper) = app_with_reclaim_system_and_helper();

    let (_lower, marker) = register_mundane_coffin_with_marker(app.world_mut(), lower);

    // 把背包填满：用一个只有 1 格的容器并塞满它（ling_mu_ban×64 占满唯一格）。
    // 然后换掉 empty_player_inventory，让 reclaim drops 无处落脚。
    let item_registry = make_coffin_item_registry();
    let mut allocator = InventoryInstanceIdAllocator::default();
    let mut full_inv = PlayerInventory {
        triggered_treasures: Vec::new(),
        revision: InventoryRevision(0),
        containers: vec![ContainerState {
            quick_access: false,
            id: MAIN_PACK_CONTAINER_ID.to_string(),
            name: "主背包".to_string(),
            rows: 1,
            cols: 1,
            items: vec![],
            owner_instance_id: None,
        }],
        equipped: HashMap::new(),
        hotbar: Default::default(),
        bone_coins: 0,
        max_weight: 9999.0,
    };
    crate::inventory::add_item_to_player_inventory(
        &mut full_inv,
        &item_registry,
        &mut allocator,
        "ling_mu_ban",
        64,
        0,
    )
    .expect("should fill the single slot");
    app.world_mut().entity_mut(client_entity).insert(full_inv);

    app.world_mut().send_event(CoffinMenuReclaimRequest {
        player: client_entity,
        pos: lower,
        tick: 0,
    });

    app.update();

    // (a) 棺 marker 已 despawn，不因满包中断。
    assert!(
        app.world().get_entity(marker).is_none(),
        "满包回收后 marker 实体仍应被 despawn；期望：满包只影响 grant，不阻断 despawn 路径"
    );
    // registry 也已移除。
    assert!(
        app.world()
            .resource::<CoffinRegistry>()
            .lookup(lower)
            .is_none(),
        "满包回收后 registry 仍应移除 lower={lower:?}；期望：remove_by_pos 先于 inventory 检查"
    );

    // (b) 客户端收到聊天提示。
    let messages: Vec<String> = helper
        .collect_received()
        .0
        .into_iter()
        .filter_map(|frame| {
            frame
                .decode::<GameMessageS2c>()
                .ok()
                .map(|p| p.chat.to_legacy_lossy())
        })
        .collect();
    assert!(
        messages
            .iter()
            .any(|m| m.contains("[棺]") && m.contains("背包已满")),
        "满包回收时客户端应收到包含 '[棺]' 和 '背包已满' 的 chat 提示，\
             锁定非静默材料丢失契约；实际 messages={messages:?}"
    );
}

#[test]
fn ecs_coffin_reclaim_without_inventory_still_removes_coffin() {
    // 边界测试：PlayerInventory=None 时，handle_coffin_menu_reclaim 仍应
    // despawn marker + 移除 registry，不 panic，材料静默丢弃。
    let (mut app, client_entity) = app_with_reclaim_system();
    let lower = BlockPos::new(0, 0, 0);

    let (_lower, marker) = register_mundane_coffin_with_marker(app.world_mut(), lower);

    app.world_mut()
        .entity_mut(client_entity)
        .remove::<PlayerInventory>();

    app.world_mut().send_event(CoffinMenuReclaimRequest {
        player: client_entity,
        pos: lower,
        tick: 0,
    });

    app.update();

    // (a) marker 已 despawn。
    assert!(
        app.world().get_entity(marker).is_none(),
        "无 inventory 时 reclaim 仍应 despawn marker {marker:?}；\
             期望：despawn_coffin_marker 在 inventory 检查前执行"
    );

    // (b) registry 已移除。
    assert!(
        app.world()
            .resource::<CoffinRegistry>()
            .lookup(lower)
            .is_none(),
        "无 inventory 时 reclaim 仍应移除 registry lower={lower:?}；\
             期望：remove_by_pos 在 inventory 检查前执行"
    );
}

// ─────────────────── plan-coffin-tiers-v1 P4 tests ──────────────────────

/// P4 §配方注册：3 档新配方全部可从 registry 查到，id 对齐 coffin_recipe_id。
#[test]
fn p4_jade_stone_bronze_recipes_registered() {
    let registry = craft_registry_with_coffin();
    for grade in [CoffinGrade::Jade, CoffinGrade::Stone, CoffinGrade::Bronze] {
        let recipe_id = coffin_recipe_id(grade);
        assert!(
            registry.get(&recipe_id).is_some(),
            "P4 配方 `{recipe_id}` 应已注册（grade={grade:?}）"
        );
    }
}

/// P4 §配方字段：材料 / qi_cost / time_ticks / station 按 plan §P4 表正确。
#[test]
fn p4_recipe_fields_match_plan_spec() {
    let registry = craft_registry_with_coffin();

    // 寒玉棺
    let jade = registry
        .get(&coffin_recipe_id(CoffinGrade::Jade))
        .expect("jade recipe");
    assert_eq!(jade.qi_cost, 2.0, "jade qi_cost");
    assert_eq!(jade.time_ticks, 120 * TICKS_PER_SECOND, "jade time");
    assert_eq!(
        jade.station,
        Some(crate::craft::CraftStationKind::Workbench),
        "jade station"
    );
    assert!(
        jade.materials
            .iter()
            .any(|(id, cnt)| id == "yu_sui" && *cnt == 3),
        "jade 配方应含 yu_sui×3"
    );
    assert!(
        jade.materials
            .iter()
            .any(|(id, cnt)| id == "xue_po_lian" && *cnt == 2),
        "jade 配方应含 xue_po_lian×2"
    );
    assert!(
        jade.materials
            .iter()
            .any(|(id, cnt)| id == "ling_mu_ban" && *cnt == 4),
        "jade 配方应含 ling_mu_ban×4（主材，锁住别被改回不可得材料）"
    );
    assert!(
        jade.unlock_sources.iter().any(|u| matches!(
            u,
            UnlockSource::Scroll { item_template } if item_template == "scroll_jade_coffin"
        )),
        "jade 解锁源应含 Scroll(scroll_jade_coffin)"
    );

    // 玄石棺
    let stone = registry
        .get(&coffin_recipe_id(CoffinGrade::Stone))
        .expect("stone recipe");
    assert_eq!(stone.qi_cost, 4.0, "stone qi_cost");
    assert_eq!(stone.time_ticks, 150 * TICKS_PER_SECOND, "stone time");
    assert_eq!(
        stone.station,
        Some(crate::craft::CraftStationKind::Workbench),
        "stone station"
    );
    assert!(
        stone
            .materials
            .iter()
            .any(|(id, cnt)| id == "wu_yao" && *cnt == 2),
        "stone 配方应含 wu_yao×2"
    );
    assert!(
        stone
            .materials
            .iter()
            .any(|(id, cnt)| id == "xuan_iron" && *cnt == 4),
        "stone 配方应含 xuan_iron×4"
    );
    assert!(
        stone
            .materials
            .iter()
            .any(|(id, cnt)| id == "zhen_shi_chu" && *cnt == 2),
        "stone 配方应含 zhen_shi_chu×2（fix-round-1 替换 zhen_shi_zhong，锁住可得材料）"
    );
    assert!(
        stone.requirements.realm_min.is_none(),
        "stone 配方无境界门控（realm_min=None）"
    );
    assert!(
        stone.unlock_sources.iter().any(|u| matches!(
            u,
            UnlockSource::Scroll { item_template } if item_template == "scroll_stone_coffin"
        )),
        "stone 解锁源应含 Scroll(scroll_stone_coffin)"
    );
    assert!(
        stone.unlock_sources.iter().any(|u| matches!(
            u,
            UnlockSource::Mentor { npc_archetype } if npc_archetype == "array_scribe"
        )),
        "stone 解锁源应含 Mentor(array_scribe)"
    );

    // 青铜棺
    let bronze = registry
        .get(&coffin_recipe_id(CoffinGrade::Bronze))
        .expect("bronze recipe");
    assert_eq!(bronze.qi_cost, 6.0, "bronze qi_cost");
    assert_eq!(bronze.time_ticks, 180 * TICKS_PER_SECOND, "bronze time");
    assert_eq!(
        bronze.station,
        Some(crate::craft::CraftStationKind::Workbench),
        "bronze station"
    );
    assert!(
        bronze
            .materials
            .iter()
            .any(|(id, cnt)| id == "gu_tong_pian" && *cnt == 4),
        "bronze 配方应含 gu_tong_pian×4"
    );
    assert!(
        bronze
            .materials
            .iter()
            .any(|(id, cnt)| id == "xuan_iron" && *cnt == 3),
        "bronze 配方应含 xuan_iron×3"
    );
    assert!(
        bronze
            .materials
            .iter()
            .any(|(id, cnt)| id == "ling_mu_ban" && *cnt == 3),
        "bronze 配方应含 ling_mu_ban×3（fix-round-1 替换 ling_mu_jing，锁住可得材料）"
    );
    assert!(
        bronze
            .materials
            .iter()
            .any(|(id, cnt)| id == "zhen_shi_chu" && *cnt == 2),
        "bronze 配方应含 zhen_shi_chu×2（fix-round-1 替换 zhen_shi_gao，锁住可得材料）"
    );
    assert_eq!(
        bronze.requirements.realm_min,
        Some(crate::cultivation::components::Realm::Induce),
        "bronze 配方 realm_min 应为 Induce（引气境门控）"
    );
    assert!(
        bronze.unlock_sources.iter().any(|u| matches!(
            u,
            UnlockSource::Scroll { item_template } if item_template == "scroll_bronze_coffin"
        )),
        "bronze 解锁源应含 Scroll(scroll_bronze_coffin)"
    );
    assert!(
        bronze.unlock_sources.iter().any(|u| matches!(
            u,
            UnlockSource::Mentor { npc_archetype } if npc_archetype == "hermit_builder"
        )),
        "bronze 解锁源应含 Mentor(hermit_builder)（炼器流 archetype 防漂移）"
    );
}

/// P4 §门控单调梯度：倍率/qi_cost/time_ticks 全部严格递增（难度梯度自洽）。
#[test]
fn p4_grade_monotonicity() {
    // 倍率递减（越高档寿元消耗越少）
    let factors = [
        CoffinGrade::Mundane.lifespan_factor(),
        CoffinGrade::Jade.lifespan_factor(),
        CoffinGrade::Stone.lifespan_factor(),
        CoffinGrade::Bronze.lifespan_factor(),
    ];
    for i in 0..3 {
        assert!(
            factors[i] > factors[i + 1],
            "lifespan_factor 应严格递减：{:.1} > {:.1} (index {} vs {})",
            factors[i],
            factors[i + 1],
            i,
            i + 1
        );
    }

    // qi_cost 递增
    let registry = craft_registry_with_coffin();
    let qi_costs = [
        registry
            .get(&coffin_recipe_id(CoffinGrade::Mundane))
            .map(|r| r.qi_cost)
            .unwrap_or(0.0),
        registry
            .get(&coffin_recipe_id(CoffinGrade::Jade))
            .map(|r| r.qi_cost)
            .unwrap_or(0.0),
        registry
            .get(&coffin_recipe_id(CoffinGrade::Stone))
            .map(|r| r.qi_cost)
            .unwrap_or(0.0),
        registry
            .get(&coffin_recipe_id(CoffinGrade::Bronze))
            .map(|r| r.qi_cost)
            .unwrap_or(0.0),
    ];
    for i in 0..3 {
        assert!(
            qi_costs[i] < qi_costs[i + 1],
            "qi_cost 应严格递增：{} < {} (grade index {} vs {})",
            qi_costs[i],
            qi_costs[i + 1],
            i,
            i + 1
        );
    }

    // time_ticks 递增
    let times = [
        registry
            .get(&coffin_recipe_id(CoffinGrade::Mundane))
            .map(|r| r.time_ticks)
            .unwrap_or(0),
        registry
            .get(&coffin_recipe_id(CoffinGrade::Jade))
            .map(|r| r.time_ticks)
            .unwrap_or(0),
        registry
            .get(&coffin_recipe_id(CoffinGrade::Stone))
            .map(|r| r.time_ticks)
            .unwrap_or(0),
        registry
            .get(&coffin_recipe_id(CoffinGrade::Bronze))
            .map(|r| r.time_ticks)
            .unwrap_or(0),
    ];
    for i in 0..3 {
        assert!(
            times[i] < times[i + 1],
            "time_ticks 应严格递增：{} < {} (grade index {} vs {})",
            times[i],
            times[i + 1],
            i,
            i + 1
        );
    }
}

/// P4 §守恒：3 档配方 output spirit_quality = 0.0，守恒律 trivially 通过。

#[test]
fn p4_registry_set_grade_updates_both_indices() {
    let mut registry = CoffinRegistry::default();
    let lower = BlockPos::new(10, 64, 10);
    let upper = coffin_upper_half(lower);
    registry.insert(lower, 0, CoffinGrade::Mundane);

    let ok = registry.set_grade(lower, CoffinGrade::Bronze);
    assert!(ok, "set_grade 对已存在的棺应返回 true");
    assert_eq!(
        registry.lookup(lower).unwrap().grade,
        CoffinGrade::Bronze,
        "lower 索引档级应更新为 Bronze"
    );
    assert_eq!(
        registry.lookup(upper).unwrap().grade,
        CoffinGrade::Bronze,
        "upper 索引档级应更新为 Bronze（双索引同步）"
    );
}

/// P4 §set_grade：对不存在的棺返回 false。

#[test]
fn p4_reclaim_drops_non_empty_for_new_grades() {
    let registry = craft_registry_with_coffin();
    for grade in [CoffinGrade::Jade, CoffinGrade::Stone, CoffinGrade::Bronze] {
        // Reclaim 模式（全量返还）：至少有 1 项
        let drops_reclaim =
            compute_coffin_reclaim_drops(&registry, grade, ReclaimMode::Reclaim, 42);
        assert!(
            !drops_reclaim.is_empty(),
            "{grade:?} Reclaim drops should be non-empty now that recipe is registered"
        );
        // 返还物种类 ⊆ 配方原料
        let recipe = registry
            .get(&coffin_recipe_id(grade))
            .expect("recipe exists");
        let material_ids: std::collections::HashSet<&str> =
            recipe.materials.iter().map(|(id, _)| id.as_str()).collect();
        for (tid, _) in &drops_reclaim {
            assert!(
                material_ids.contains(tid.as_str()),
                "{grade:?} Reclaim 返还了配方外材料 `{tid}`"
            );
        }
    }
}

// ───────────────────────── F21 — persist_in_coffin 断连兜底 ─────────────────────────

use crate::player::state::PlayerStatePersistence;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn f21_temp_persistence(test_name: &str) -> (PlayerStatePersistence, PathBuf) {
    let unique_suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let data_dir = std::env::temp_dir().join(format!(
        "bong-coffin-persist-{test_name}-{}-{unique_suffix}",
        std::process::id()
    ));
    let db_path = data_dir.join("bong.db");
    crate::persistence::bootstrap_sqlite(&db_path, &format!("coffin-persist-{test_name}"))
        .expect("sqlite bootstrap should succeed");
    (
        PlayerStatePersistence::with_db_path(&data_dir, &db_path),
        data_dir,
    )
}

fn f21_seed_in_coffin_row(persistence: &PlayerStatePersistence, username: &str) {
    let lifespan = LifespanComponent {
        born_at_tick: 0,
        years_lived: 4.0,
        cap_by_realm: 100,
        offline_pause_tick: None,
    };
    crate::player::state::save_player_lifespan_slice_with_coffin(
        persistence,
        username,
        &lifespan,
        Some(CoffinGrade::Jade),
    )
    .expect("seeding an in-coffin row should succeed");
}

fn f21_read_in_coffin(persistence: &PlayerStatePersistence, username: &str) -> Option<i64> {
    let conn = rusqlite::Connection::open(persistence.db_path()).expect("db should open");
    conn.query_row(
        "SELECT in_coffin FROM player_lifespan WHERE username = ?1",
        rusqlite::params![username],
        |row| row.get(0),
    )
    .ok()
}

#[test]
fn persist_in_coffin_falls_back_to_narrow_clear_when_lifespan_missing() {
    let (persistence, data_dir) = f21_temp_persistence("fallback-happy-path");
    f21_seed_in_coffin_row(&persistence, "Azure");
    assert_eq!(
        f21_read_in_coffin(&persistence, "Azure"),
        Some(1),
        "sanity check: seed row must start in_coffin=1"
    );

    let username = Username("Azure".to_string());
    // grade=None (exit path) + persistence/username 都在，但 lifespan=None（已被移除）
    // —— 这正是 F21 要兜住的组合。
    persist_in_coffin(Some(&persistence), Some(&username), None, None);

    assert_eq!(
        f21_read_in_coffin(&persistence, "Azure"),
        Some(0),
        "F21: missing LifespanComponent must no longer leave in_coffin=1 stuck in SQLite \
             when persistence + username are both available to key the fallback UPDATE on"
    );

    let _ = std::fs::remove_dir_all(&data_dir);
}

#[test]
fn persist_in_coffin_grade_none_with_lifespan_present_still_uses_normal_path() {
    let (persistence, data_dir) = f21_temp_persistence("normal-path-regression");
    f21_seed_in_coffin_row(&persistence, "Azure");

    let username = Username("Azure".to_string());
    let lifespan = LifespanComponent {
        born_at_tick: 0,
        years_lived: 4.0,
        cap_by_realm: 100,
        offline_pause_tick: None,
    };
    // 三者都在 —— 应该走原有的 save_player_lifespan_slice_with_coffin 路径，不碰 F21 分支。
    persist_in_coffin(Some(&persistence), Some(&username), Some(&lifespan), None);

    assert_eq!(
        f21_read_in_coffin(&persistence, "Azure"),
        Some(0),
        "regression check: the pre-existing all-components-present exit path must still \
             clear in_coffin exactly as before F21"
    );

    let _ = std::fs::remove_dir_all(&data_dir);
}

#[test]
fn persist_in_coffin_missing_username_leaves_other_rows_untouched() {
    let (persistence, data_dir) = f21_temp_persistence("missing-username-noop");
    f21_seed_in_coffin_row(&persistence, "Bystander");

    // username=None：既不能走正常路径也不能走 F21 fallback（无法定位是谁），
    // 必须保持原地 no-op + warn，不得误清任何行。
    persist_in_coffin(Some(&persistence), None, None, None);

    assert_eq!(
        f21_read_in_coffin(&persistence, "Bystander"),
        Some(1),
        "F21: missing username must stay a no-op — must not accidentally clear an unrelated \
             row"
    );

    let _ = std::fs::remove_dir_all(&data_dir);
}

#[test]
fn persist_in_coffin_missing_persistence_does_not_panic() {
    let username = Username("Azure".to_string());
    // persistence=None：没有 DB 句柄可写，必须原地 no-op + warn，且绝不 panic。
    persist_in_coffin(None, Some(&username), None, None);
}

#[test]
fn persist_in_coffin_grade_some_with_lifespan_missing_does_not_use_f21_fallback() {
    // F21 fallback 只在 grade.is_none() 时触发（那才是"清空"语义）；grade=Some 但
    // lifespan=None 属于另一种缺组件场景（例如刚进棺就断连），必须维持原有 no-op+warn，
    // 不能被 F21 误当成"清空"请求而抹掉 in_coffin。
    let (persistence, data_dir) = f21_temp_persistence("grade-some-lifespan-missing");
    f21_seed_in_coffin_row(&persistence, "Azure");

    let username = Username("Azure".to_string());
    persist_in_coffin(
        Some(&persistence),
        Some(&username),
        None,
        Some(CoffinGrade::Stone),
    );

    assert_eq!(
        f21_read_in_coffin(&persistence, "Azure"),
        Some(1),
        "grade=Some + lifespan=None must remain a no-op (F21 only covers the grade=None \
             clearing path), so the seeded in_coffin=1 row must be untouched"
    );

    let _ = std::fs::remove_dir_all(&data_dir);
}

// ───────────── plan-bughunt-coffin-dimension-gate-v1 — 维度门禁回归 ─────────────
//
// 四类请求（place / enter / break / menu_reclaim）在异维玩家 + 裸坐标数值巧合命中
// 主世界棺位时必须拒绝（含 CurrentDimension 组件缺失的 fail-closed 分支）；
// 同维度 + 近距时必须放行——只测拒绝不够，必须证明门禁没有连正常操作一起拦掉。

fn collect_chat_messages(helper: &mut MockClientHelper) -> Vec<String> {
    helper
        .collect_received()
        .0
        .into_iter()
        .filter_map(|frame| {
            frame
                .decode::<GameMessageS2c>()
                .ok()
                .map(|packet| packet.chat.to_legacy_lossy())
        })
        .collect()
}

/// 构造 handle_coffin_place_requests 测试场景：玩家背包已持有一个 `mundane_coffin`
/// 物品实例（返回其 item_instance_id），layer 实体打上 `OverworldLayer` 标记。
fn app_with_place_system_holding_coffin() -> (App, Entity, u64, MockClientHelper) {
    let scenario = ScenarioSingleClient::new();
    let mut app = scenario.app;
    let client_entity = scenario.client;
    let layer_entity = scenario.layer;
    let helper = scenario.helper;

    let item_registry = crate::inventory::load_item_registry().expect("item registry should load");
    let mut allocator = InventoryInstanceIdAllocator::default();
    let mut inventory = empty_player_inventory();
    let receipt = add_item_to_player_inventory(
        &mut inventory,
        &item_registry,
        &mut allocator,
        MUNDANE_COFFIN_ITEM_ID,
        1,
        0,
    )
    .expect("granting mundane_coffin should succeed");
    let item_instance_id = receipt.instance_id;

    app.add_event::<CoffinPlaceRequest>();
    app.insert_resource(CoffinRegistry::default());
    app.insert_resource(item_registry);
    app.insert_resource(allocator);

    app.world_mut()
        .entity_mut(layer_entity)
        .insert(OverworldLayer);
    app.world_mut().entity_mut(client_entity).insert(inventory);
    // `handle_coffin_place_requests` 的玩家查询要求 `&PlayerState`（非 Option），缺了会在
    // 取玩家那步就整条早退——那样连维度分支都进不去，拒绝侧用例会因为「压根没跑到门禁」
    // 而假绿，放行侧则永远注册不上。必须显式补上。
    app.world_mut()
        .entity_mut(client_entity)
        .insert(PlayerState::default());
    // 默认场景 = 玩家在主世界（happy path）；跨维测试显式覆盖/移除该组件。
    app.world_mut()
        .entity_mut(client_entity)
        .insert(CurrentDimension(DimensionKind::Overworld));

    app.add_systems(Update, handle_coffin_place_requests);

    (app, client_entity, item_instance_id, helper)
}

#[test]
fn ecs_coffin_place_allowed_in_overworld_near_target() {
    // 放行侧：主世界 + 近距 → 棺应成功注册，随身棺材物品被消费。
    let (mut app, client_entity, item_instance_id, _helper) =
        app_with_place_system_holding_coffin();
    let target = BlockPos::new(0, 0, 0);

    app.world_mut().send_event(CoffinPlaceRequest {
        player: client_entity,
        pos: target,
        item_instance_id,
        tick: 0,
    });
    app.update();

    assert!(
        app.world()
            .resource::<CoffinRegistry>()
            .lookup(target)
            .is_some(),
        "主世界 + 近距放置应成功注册棺于 {target:?}，但 registry 中找不到"
    );
    let inventory = app
        .world()
        .get::<PlayerInventory>(client_entity)
        .expect("client should carry PlayerInventory");
    assert!(
        inventory_item_by_instance(inventory, item_instance_id).is_none(),
        "放置成功后随身棺材物品实例应被消费，但仍能在背包中找到"
    );
}

#[test]
fn ecs_coffin_place_rejected_when_player_too_far() {
    // 远距拒绝侧：放置不得登记棺，也不得在 reach 门之前消费背包物品。
    let (mut app, client_entity, item_instance_id, _helper) =
        app_with_place_system_holding_coffin();
    let target = BlockPos::new(0, 0, 0);
    app.world_mut()
        .entity_mut(client_entity)
        .insert(Position::new([100.0, 0.0, 100.0]));

    app.world_mut().send_event(CoffinPlaceRequest {
        player: client_entity,
        pos: target,
        item_instance_id,
        tick: 0,
    });
    app.update();

    assert!(
        app.world()
            .resource::<CoffinRegistry>()
            .lookup(target)
            .is_none(),
        "远距放置被拒后 registry 不应登记棺于 {target:?}"
    );
    let inventory = app
        .world()
        .get::<PlayerInventory>(client_entity)
        .expect("client should carry PlayerInventory");
    assert!(
        inventory_item_by_instance(inventory, item_instance_id).is_some(),
        "远距放置被拒必须发生在消费物品之前，棺材物品实例应仍在背包中"
    );
}

#[test]
fn ecs_coffin_place_rejected_sends_place_rejected_chat() {
    // review finding [2]：放置被拒必须推请求特定的 chat 回执「[棺] 放置被拒」——
    // bot/验证据此判定放置结果，不再依赖 /give 包序快照屏障。伪造 instance 落在
    // 缺实例分支（主世界 + 近距，走 instance 校验），必须发该回执。
    let (mut app, client_entity, _item_instance_id, mut helper) =
        app_with_place_system_holding_coffin();
    let target = BlockPos::new(0, 0, 0);

    app.world_mut().send_event(CoffinPlaceRequest {
        player: client_entity,
        pos: target,
        item_instance_id: 999_999, // 不存在的实例 → missing item instance 被拒
        tick: 0,
    });
    app.update();

    let messages = collect_chat_messages(&mut helper);
    assert!(
            messages
                .iter()
                .any(|m| m.contains("[棺]") && m.contains("放置被拒")),
            "放置被拒必须下发显式 chat 回执（COFFIN_PLACE_REJECTION_MESSAGE_PREFIX）；             实际 messages={messages:?}"
        );
    assert!(
        app.world()
            .resource::<CoffinRegistry>()
            .lookup(target)
            .is_none(),
        "被拒放棺不得注册棺于 {target:?}"
    );
}

#[test]
fn ecs_coffin_place_allowed_does_not_send_place_rejected_chat() {
    // 放行侧对照：成功放置不得误发「放置被拒」回执（二者互斥，bot 靠互斥收口）。
    let (mut app, client_entity, item_instance_id, mut helper) =
        app_with_place_system_holding_coffin();
    let target = BlockPos::new(0, 0, 0);

    app.world_mut().send_event(CoffinPlaceRequest {
        player: client_entity,
        pos: target,
        item_instance_id,
        tick: 0,
    });
    app.update();

    assert!(
        app.world()
            .resource::<CoffinRegistry>()
            .lookup(target)
            .is_some(),
        "主世界 + 近距放置应成功注册棺于 {target:?}"
    );
    let messages = collect_chat_messages(&mut helper);
    assert!(
            !messages.iter().any(|m| m.contains("放置被拒")),
            "成功放置不应发送「放置被拒」回执（接受=coffin_place_consumed 快照，             拒绝=该 chat，二者互斥）；实际 messages={messages:?}"
        );
}

#[test]
fn ecs_coffin_place_rejected_in_tsy_same_numeric_pos() {
    // 拒绝侧：坍缩渊玩家 + 与主世界棺位数值相同的坐标 → 必须拒绝，且带 chat 反馈。
    let (mut app, client_entity, item_instance_id, mut helper) =
        app_with_place_system_holding_coffin();
    app.world_mut()
        .entity_mut(client_entity)
        .insert(CurrentDimension(DimensionKind::Tsy));
    let target = BlockPos::new(0, 0, 0);

    app.world_mut().send_event(CoffinPlaceRequest {
        player: client_entity,
        pos: target,
        item_instance_id,
        tick: 0,
    });
    app.update();

    assert!(
        app.world()
            .resource::<CoffinRegistry>()
            .lookup(target)
            .is_none(),
        "坍缩渊玩家靠裸坐标数值巧合不得在主世界注册棺，但 registry 中找到了 {target:?}"
    );
    let inventory = app
        .world()
        .get::<PlayerInventory>(client_entity)
        .expect("client should carry PlayerInventory");
    assert!(
        inventory_item_by_instance(inventory, item_instance_id).is_some(),
        "维度拒绝必须发生在消费物品之前，随身棺材物品应仍在背包中"
    );
    let messages = collect_chat_messages(&mut helper);
    assert!(
        messages
            .iter()
            .any(|m| m.contains("[棺]") && m.contains("主世界")),
        "维度拒绝必须下发 chat 反馈；实际 messages={messages:?}"
    );
}

#[test]
fn ecs_coffin_place_rejected_missing_dimension() {
    // fail-closed：CurrentDimension 组件缺失时同样拒绝，不得隐式当作主世界处理。
    let (mut app, client_entity, item_instance_id, mut helper) =
        app_with_place_system_holding_coffin();
    app.world_mut()
        .entity_mut(client_entity)
        .remove::<CurrentDimension>();
    let target = BlockPos::new(0, 0, 0);

    app.world_mut().send_event(CoffinPlaceRequest {
        player: client_entity,
        pos: target,
        item_instance_id,
        tick: 0,
    });
    app.update();

    assert!(
        app.world()
            .resource::<CoffinRegistry>()
            .lookup(target)
            .is_none(),
        "缺失 CurrentDimension 时应 fail-closed 拒绝，但棺仍被注册于 {target:?}"
    );
    let messages = collect_chat_messages(&mut helper);
    assert!(
        messages
            .iter()
            .any(|m| m.contains("[棺]") && m.contains("主世界")),
        "维度组件缺失同样必须下发拒绝 chat 反馈；实际 messages={messages:?}"
    );
}

/// 构造 handle_coffin_enter_requests 测试场景：registry 中已注册一口未占用的
/// mundane 棺，坐标紧贴 ScenarioSingleClient 玩家默认原点。
fn app_with_enter_system_and_registered_coffin() -> (App, Entity, BlockPos, MockClientHelper) {
    let scenario = ScenarioSingleClient::new();
    let mut app = scenario.app;
    let client_entity = scenario.client;
    let helper = scenario.helper;
    let lower = BlockPos::new(0, 0, 0);

    app.add_event::<CoffinEnterRequest>();
    app.add_event::<CoffinStateChanged>();
    app.add_event::<PlaySoundRecipeRequest>();

    let mut registry = CoffinRegistry::default();
    registry.insert(lower, 0, CoffinGrade::Mundane);
    app.insert_resource(registry);

    // 默认场景 = 玩家在主世界（happy path）；跨维测试显式覆盖/移除该组件。
    app.world_mut()
        .entity_mut(client_entity)
        .insert(CurrentDimension(DimensionKind::Overworld));

    app.add_systems(Update, handle_coffin_enter_requests);

    (app, client_entity, lower, helper)
}

#[test]
fn ecs_coffin_enter_allowed_in_overworld_near_target() {
    // 放行侧：主世界 + 近距 → 玩家应成功进棺（registry 标记占用 + CoffinComponent 挂载）。
    let (mut app, client_entity, lower, _helper) = app_with_enter_system_and_registered_coffin();

    app.world_mut().send_event(CoffinEnterRequest {
        player: client_entity,
        pos: lower,
        tick: 0,
    });
    app.update();

    assert_eq!(
        app.world()
            .resource::<CoffinRegistry>()
            .lookup(lower)
            .unwrap()
            .occupied_by,
        Some(client_entity),
        "主世界 + 近距进棺应成功标记 registry 占用，但未生效"
    );
    assert!(
        app.world().get::<CoffinComponent>(client_entity).is_some(),
        "主世界 + 近距进棺后玩家实体应挂载 CoffinComponent"
    );
}

#[test]
fn ecs_coffin_enter_rejected_when_player_too_far() {
    // 远距拒绝侧：进棺不得占用 registry，也不得写入玩家的 CoffinComponent。
    let (mut app, client_entity, lower, _helper) = app_with_enter_system_and_registered_coffin();
    app.world_mut()
        .entity_mut(client_entity)
        .insert(Position::new([100.0, 0.0, 100.0]));

    app.world_mut().send_event(CoffinEnterRequest {
        player: client_entity,
        pos: lower,
        tick: 0,
    });
    app.update();

    assert!(
        app.world()
            .resource::<CoffinRegistry>()
            .lookup(lower)
            .expect("registered coffin should remain available")
            .occupied_by
            .is_none(),
        "远距进棺被拒后 registry 不应标记为已占用"
    );
    assert!(
        app.world().get::<CoffinComponent>(client_entity).is_none(),
        "远距进棺被拒后玩家实体不应挂载 CoffinComponent"
    );
}

#[test]
fn ecs_coffin_enter_rejected_in_tsy_same_numeric_pos() {
    // 拒绝侧：坍缩渊玩家 + 与主世界棺位数值相同的坐标 → 必须拒绝，且带 chat 反馈。
    let (mut app, client_entity, lower, mut helper) = app_with_enter_system_and_registered_coffin();
    app.world_mut()
        .entity_mut(client_entity)
        .insert(CurrentDimension(DimensionKind::Tsy));

    app.world_mut().send_event(CoffinEnterRequest {
        player: client_entity,
        pos: lower,
        tick: 0,
    });
    app.update();

    assert!(
        app.world()
            .resource::<CoffinRegistry>()
            .lookup(lower)
            .unwrap()
            .occupied_by
            .is_none(),
        "坍缩渊玩家靠裸坐标数值巧合不得进入主世界棺，但 registry 显示已被占用"
    );
    assert!(
        app.world().get::<CoffinComponent>(client_entity).is_none(),
        "维度拒绝后玩家实体不应挂载 CoffinComponent"
    );
    let messages = collect_chat_messages(&mut helper);
    assert!(
        messages
            .iter()
            .any(|m| m.contains("[棺]") && m.contains("主世界")),
        "维度拒绝必须下发 chat 反馈；实际 messages={messages:?}"
    );
}

#[test]
fn ecs_coffin_enter_rejected_missing_dimension() {
    // fail-closed：CurrentDimension 组件缺失时同样拒绝，不得隐式当作主世界处理。
    let (mut app, client_entity, lower, mut helper) = app_with_enter_system_and_registered_coffin();
    app.world_mut()
        .entity_mut(client_entity)
        .remove::<CurrentDimension>();

    app.world_mut().send_event(CoffinEnterRequest {
        player: client_entity,
        pos: lower,
        tick: 0,
    });
    app.update();

    assert!(
        app.world()
            .resource::<CoffinRegistry>()
            .lookup(lower)
            .unwrap()
            .occupied_by
            .is_none(),
        "缺失 CurrentDimension 时应 fail-closed 拒绝，但 registry 显示已被占用"
    );
    let messages = collect_chat_messages(&mut helper);
    assert!(
        messages
            .iter()
            .any(|m| m.contains("[棺]") && m.contains("主世界")),
        "维度组件缺失同样必须下发拒绝 chat 反馈；实际 messages={messages:?}"
    );
}

/// app_with_break_system 的变体：同时返回 MockClientHelper，供 chat 反馈断言使用。
fn app_with_break_system_and_helper() -> (App, Entity, MockClientHelper) {
    let scenario = ScenarioSingleClient::new();
    let mut app = scenario.app;
    let client_entity = scenario.client;
    let helper = scenario.helper;

    app.add_event::<CoffinBreakRequest>();
    app.add_event::<CoffinStateChanged>();
    app.add_event::<PlaySoundRecipeRequest>();

    app.insert_resource(CoffinRegistry::default());
    let mut craft = CraftRegistry::new();
    register_craft_recipes(&mut craft).expect("coffin recipes should register");
    app.insert_resource(craft);

    app.insert_resource(make_coffin_item_registry());
    app.insert_resource(InventoryInstanceIdAllocator::default());

    app.world_mut()
        .entity_mut(client_entity)
        .insert(empty_player_inventory());
    app.world_mut()
        .entity_mut(client_entity)
        .insert(CurrentDimension(DimensionKind::Overworld));

    app.add_systems(Update, handle_coffin_breaks);

    (app, client_entity, helper)
}

#[test]
fn ecs_coffin_break_rejected_in_tsy_same_numeric_pos_with_chat_feedback() {
    // 拒绝侧：坍缩渊玩家 + 与主世界棺位数值相同的坐标 → 必须拒绝，且带 chat 反馈。
    // （放行侧已由既有 `ecs_coffin_break_despawns_marker_and_grants_partial_materials`
    // 覆盖：该测试场景现在默认玩家在主世界，是本门禁的放行回归。）
    let (mut app, client_entity, mut helper) = app_with_break_system_and_helper();
    let lower = BlockPos::new(0, 0, 0);
    let (_lower, marker) = register_mundane_coffin_with_marker(app.world_mut(), lower);
    app.world_mut()
        .entity_mut(client_entity)
        .insert(CurrentDimension(DimensionKind::Tsy));

    app.world_mut().send_event(CoffinBreakRequest {
        player: client_entity,
        pos: lower,
        tick: 0,
    });
    app.update();

    assert!(
        app.world()
            .resource::<CoffinRegistry>()
            .lookup(lower)
            .is_some(),
        "坍缩渊玩家靠裸坐标数值巧合不得破坏主世界棺，但 registry 中已找不到 {lower:?}"
    );
    assert!(
        app.world().get_entity(marker).is_some(),
        "维度拒绝后 marker 实体应仍存在（{marker:?}）"
    );
    let messages = collect_chat_messages(&mut helper);
    assert!(
        messages
            .iter()
            .any(|m| m.contains("[棺]") && m.contains("主世界")),
        "维度拒绝必须下发 chat 反馈；实际 messages={messages:?}"
    );
}

#[test]
fn ecs_coffin_break_rejected_missing_dimension() {
    // fail-closed：CurrentDimension 组件缺失时同样拒绝，不得隐式当作主世界处理。
    let (mut app, client_entity, mut helper) = app_with_break_system_and_helper();
    let lower = BlockPos::new(0, 0, 0);
    let (_lower, marker) = register_mundane_coffin_with_marker(app.world_mut(), lower);
    app.world_mut()
        .entity_mut(client_entity)
        .remove::<CurrentDimension>();

    app.world_mut().send_event(CoffinBreakRequest {
        player: client_entity,
        pos: lower,
        tick: 0,
    });
    app.update();

    assert!(
        app.world()
            .resource::<CoffinRegistry>()
            .lookup(lower)
            .is_some(),
        "缺失 CurrentDimension 时应 fail-closed 拒绝，但棺已被破坏移除"
    );
    assert!(
        app.world().get_entity(marker).is_some(),
        "缺失 CurrentDimension 时应 fail-closed 拒绝，但 marker 实体已被 despawn"
    );
    let messages = collect_chat_messages(&mut helper);
    assert!(
        messages
            .iter()
            .any(|m| m.contains("[棺]") && m.contains("主世界")),
        "维度组件缺失同样必须下发拒绝 chat 反馈；实际 messages={messages:?}"
    );
}

#[test]
fn ecs_coffin_reclaim_rejected_in_tsy_same_numeric_pos_with_chat_feedback() {
    // 拒绝侧：坍缩渊玩家 + 与主世界棺位数值相同的坐标 → 必须拒绝，且带 chat 反馈。
    // （放行侧已由既有 `ecs_coffin_menu_reclaim_despawns_marker_and_grants_full_materials`
    // 覆盖：该测试场景现在默认玩家在主世界，是本门禁的放行回归。）
    let (mut app, client_entity, mut helper) = app_with_reclaim_system_and_helper();
    let lower = BlockPos::new(0, 0, 0);
    let (_lower, marker) = register_mundane_coffin_with_marker(app.world_mut(), lower);
    app.world_mut()
        .entity_mut(client_entity)
        .insert(CurrentDimension(DimensionKind::Tsy));

    app.world_mut().send_event(CoffinMenuReclaimRequest {
        player: client_entity,
        pos: lower,
        tick: 0,
    });
    app.update();

    assert!(
        app.world()
            .resource::<CoffinRegistry>()
            .lookup(lower)
            .is_some(),
        "坍缩渊玩家靠裸坐标数值巧合不得回收主世界棺，但 registry 中已找不到 {lower:?}"
    );
    assert!(
        app.world().get_entity(marker).is_some(),
        "维度拒绝后 marker 实体应仍存在（{marker:?}）"
    );
    let messages = collect_chat_messages(&mut helper);
    assert!(
        messages
            .iter()
            .any(|m| m.contains("[棺]") && m.contains("主世界")),
        "维度拒绝必须下发 chat 反馈；实际 messages={messages:?}"
    );
}

#[test]
fn ecs_coffin_reclaim_rejected_when_player_too_far() {
    // 远距拒绝侧：菜单回收不得移除 registry/marker，也不得发放材料。
    let (mut app, client_entity) = app_with_reclaim_system();
    let lower = BlockPos::new(0, 0, 0);
    let (_lower, marker) = register_mundane_coffin_with_marker(app.world_mut(), lower);
    app.world_mut()
        .entity_mut(client_entity)
        .insert(Position::new([100.0, 0.0, 100.0]));

    app.world_mut().send_event(CoffinMenuReclaimRequest {
        player: client_entity,
        pos: lower,
        tick: 0,
    });
    app.update();

    assert!(
        app.world()
            .resource::<CoffinRegistry>()
            .lookup(lower)
            .is_some(),
        "远距回收被拒后 registry 应仍保有 {lower:?}"
    );
    assert!(
        app.world().get_entity(marker).is_some(),
        "远距回收被拒后 marker 实体应仍存在（{marker:?}）"
    );
    let inventory = app
        .world()
        .get::<PlayerInventory>(client_entity)
        .expect("client should carry PlayerInventory");
    let total_items: usize = inventory.containers.iter().map(|c| c.items.len()).sum();
    assert_eq!(
        total_items, 0,
        "远距回收被拒后背包不应收到任何材料，实得 {total_items} 件"
    );
}

#[test]
fn ecs_coffin_reclaim_rejected_missing_dimension() {
    // fail-closed：CurrentDimension 组件缺失时同样拒绝，不得隐式当作主世界处理。
    let (mut app, client_entity, mut helper) = app_with_reclaim_system_and_helper();
    let lower = BlockPos::new(0, 0, 0);
    let (_lower, marker) = register_mundane_coffin_with_marker(app.world_mut(), lower);
    app.world_mut()
        .entity_mut(client_entity)
        .remove::<CurrentDimension>();

    app.world_mut().send_event(CoffinMenuReclaimRequest {
        player: client_entity,
        pos: lower,
        tick: 0,
    });
    app.update();

    assert!(
        app.world()
            .resource::<CoffinRegistry>()
            .lookup(lower)
            .is_some(),
        "缺失 CurrentDimension 时应 fail-closed 拒绝，但棺已被回收移除"
    );
    assert!(
        app.world().get_entity(marker).is_some(),
        "缺失 CurrentDimension 时应 fail-closed 拒绝，但 marker 实体已被 despawn"
    );
    let messages = collect_chat_messages(&mut helper);
    assert!(
        messages
            .iter()
            .any(|m| m.contains("[棺]") && m.contains("主世界")),
        "维度组件缺失同样必须下发拒绝 chat 反馈；实际 messages={messages:?}"
    );
}
