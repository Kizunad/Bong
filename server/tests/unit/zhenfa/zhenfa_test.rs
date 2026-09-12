#![allow(dead_code)]

use std::collections::HashMap;

use bong_server::combat::components::*;
use bong_server::combat::events::*;
use bong_server::combat::CombatClock;
use bong_server::cultivation::color::PracticeLog;
use bong_server::cultivation::components::*;
use bong_server::cultivation::insight_apply::InsightModifiers;
use bong_server::cultivation::meridian::severed::MeridianSeveredPermanent;
use bong_server::cultivation::tribulation::JueBiTriggerEvent;
use bong_server::fauna::components::*;
use bong_server::inventory::*;
use bong_server::lingtian::*;
use bong_server::network::gameplay_vfx;
use bong_server::network::vfx_event_emit::VfxEventRequest;
use bong_server::npc::spawn::DecoyTarget;
use bong_server::player::gameplay::PendingGameplayNarrations;
use bong_server::player::state::canonical_player_id;
use bong_server::qi_physics::constants::*;
use bong_server::qi_physics::*;
use bong_server::schema::common::NarrationStyle;
use bong_server::schema::social::RelationshipKindV1;
use bong_server::social::components::{Relationships, Renown};
use bong_server::world::dimension::OverworldLayer;
use bong_server::world::zone::{ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};
use bong_server::zhenfa::trap_content;
use bong_server::zhenfa::*;
use valence::prelude::{
    App, BlockPos, BlockState, ChunkLayer, Entity, Events, Position, PropName, PropValue, UniqueId,
    UnloadedChunk, Username,
};
use valence::testing::ScenarioSingleClient;

// These IDs and costs are the public item-input contract exercised by the
// external tests. The production constants remain private; no test-only seam
// is opened merely to import them.
const ZHENFA_FLAG_ITEM_ID: &str = "array_flag";
const QI_SCATTER_BEAD_ITEM_ID: &str = "qi_scatter_bead";
const NETWORK_ARRAY_FLAG_ITEM_ID: &str = "array_flag_basic";
const NETWORK_ARRAY_EYE_ITEM_ID: &str = "array_eye_basic";
const SCATTER_DISTURBANCE_EVENT: &str = "scatter_disturbance";
const DECEIVE_HEAVEN_SPIRITWOOD_ITEM_ID: &str = "ling_mu_ban";
const DECEIVE_HEAVEN_SPIRITWOOD_COST: u32 = 2;
const DECEIVE_HEAVEN_BEAST_BONE_ITEM_ID: &str = "yi_shou_gu";
const DECEIVE_HEAVEN_BEAST_BONE_COST: u32 = 4;
const DECEIVE_HEAVEN_BONE_COIN_COST: u64 = 10;
const ANIM_RUNE_DRAW: &str = "bong:rune_draw";
const COMBAT_PRIORITY: u16 = 1000;

fn add_zhenfa_test_support(app: &mut App) {
    app.insert_resource(CombatClock::default());
    app.insert_resource(PendingGameplayNarrations::default());
    bong_server::zhenfa::register(app);
    // register() owns the zhenfa runtime events; these four are consumed by
    // optional combat/feedback branches exercised by the public test contract.
    app.add_event::<JueBiTriggerEvent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<ApplyStatusEffectIntent>();
}

fn app_with_zhenfa() -> App {
    let mut app = App::new();
    add_zhenfa_test_support(&mut app);
    app
}

fn app_with_zhenfa_layer() -> (App, Entity) {
    let scenario = ScenarioSingleClient::new();
    let layer = scenario.layer;
    let mut app = scenario.app;
    app.world_mut().entity_mut(layer).insert(OverworldLayer);
    app.world_mut()
        .get_mut::<ChunkLayer>(layer)
        .expect("test layer should carry ChunkLayer")
        .insert_chunk([0, 0], UnloadedChunk::new());
    add_zhenfa_test_support(&mut app);
    (app, layer)
}

fn app_with_loaded_zhenfa() -> App {
    let (app, _) = app_with_zhenfa_layer();
    app
}

fn app_with_zhenfa_unloaded_layer() -> (App, Entity) {
    let scenario = ScenarioSingleClient::new();
    let layer = scenario.layer;
    let mut app = scenario.app;
    app.world_mut().entity_mut(layer).insert(OverworldLayer);
    add_zhenfa_test_support(&mut app);
    (app, layer)
}

fn zhenfa_eye_state(charged: bool) -> BlockState {
    BlockState::BONG_ZHENFA_EYE.set(
        PropName::Charged,
        if charged {
            PropValue::True
        } else {
            PropValue::False
        },
    )
}

fn block_pos_from_array(pos: [i32; 3]) -> BlockPos {
    BlockPos::new(pos[0], pos[1], pos[2])
}

fn layer_block_state(app: &App, layer_entity: Entity, pos: [i32; 3]) -> Option<BlockState> {
    app.world()
        .get::<ChunkLayer>(layer_entity)
        .and_then(|layer| {
            layer
                .block(block_pos_from_array(pos))
                .map(|block| block.state)
        })
}

fn spawn_player(app: &mut App, name: &str, pos: [f64; 3]) -> Entity {
    spawn_player_with_inventory(app, name, pos, zhenfa_flag_inventory())
}

fn spawn_player_with_inventory(
    app: &mut App,
    name: &str,
    pos: [f64; 3],
    inventory: PlayerInventory,
) -> Entity {
    app.world_mut()
        .spawn((
            Username(name.to_string()),
            UniqueId::default(),
            Position::new(pos),
            Cultivation {
                realm: Realm::Induce,
                qi_current: 100.0,
                qi_max: 100.0,
                ..Default::default()
            },
            QiColor::default(),
            PracticeLog::default(),
            Wounds::default(),
            Contamination::default(),
            MeridianSystem::default(),
            inventory,
        ))
        .id()
}

fn spawn_plot(app: &mut App, pos: [i32; 3], cap: f32) -> Entity {
    let mut plot = LingtianPlot::new(block_pos_from_array(pos), None);
    plot.plot_qi_cap = cap;
    app.world_mut().spawn(plot).id()
}

fn plot_cap(app: &mut App, pos: [i32; 3]) -> f32 {
    app.world_mut()
        .query::<&LingtianPlot>()
        .iter(app.world())
        .find(|plot| plot.pos == block_pos_from_array(pos))
        .map(|plot| plot.plot_qi_cap)
        .expect("test plot should exist")
}

fn send_lingju_place(app: &mut App, player: Entity, pos: [i32; 3], tick: u64) {
    app.world_mut().send_event(ZhenfaPlaceRequest {
        player,
        pos,
        kind: ZhenfaKind::Lingju,
        carrier: ZhenfaCarrierKind::BeastCoreInlaid,
        qi_invest_ratio: 0.30,
        trigger: None,
        item_instance_id: None,
        target_face: None,
        requested_at_tick: tick,
    });
}

fn array_flag_item(instance_id: u64) -> ItemInstance {
    ItemInstance {
        instance_id,
        template_id: ZHENFA_FLAG_ITEM_ID.to_string(),
        display_name: "阵旗".to_string(),
        grid_w: 1,
        grid_h: 2,
        weight: 0.6,
        rarity: ItemRarity::Uncommon,
        description: "地师用来牵引阵眼气机的短旗。".to_string(),
        stack_count: 1,
        spirit_quality: 0.8,
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

fn trap_item(instance_id: u64, template_id: &str, display_name: &str) -> ItemInstance {
    ItemInstance {
        instance_id,
        template_id: template_id.to_string(),
        display_name: display_name.to_string(),
        grid_w: 1,
        grid_h: 1,
        weight: 0.1,
        rarity: ItemRarity::Common,
        description: display_name.to_string(),
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

fn material_item(instance_id: u64, template_id: &str, stack_count: u32) -> ItemInstance {
    let mut item = trap_item(instance_id, template_id, template_id);
    item.stack_count = stack_count;
    item
}

fn scatter_bead_item(instance_id: u64) -> ItemInstance {
    trap_item(instance_id, QI_SCATTER_BEAD_ITEM_ID, "散灵珠")
}

fn network_array_item(instance_id: u64, template_id: &str) -> ItemInstance {
    let mut item = trap_item(instance_id, template_id, template_id);
    item.spirit_quality = match template_id {
        NETWORK_ARRAY_EYE_ITEM_ID => 0.5,
        NETWORK_ARRAY_FLAG_ITEM_ID => 0.0,
        _ => item.spirit_quality,
    };
    item
}

fn network_array_inventory(items: &[(u64, &str)]) -> PlayerInventory {
    let mut inventory = zhenfa_flag_inventory();
    for (slot, (instance_id, template_id)) in items.iter().enumerate() {
        inventory.containers[0].items.push(PlacedItemState {
            row: 0,
            col: slot as u8,
            instance: network_array_item(*instance_id, template_id),
        });
    }
    inventory
}

fn network_array_test_inventory() -> PlayerInventory {
    network_array_inventory(&[
        (8101, NETWORK_ARRAY_FLAG_ITEM_ID),
        (8102, NETWORK_ARRAY_FLAG_ITEM_ID),
        (8103, NETWORK_ARRAY_FLAG_ITEM_ID),
        (8201, NETWORK_ARRAY_EYE_ITEM_ID),
    ])
}

fn send_network_array_place(
    app: &mut App,
    player: Entity,
    pos: [i32; 3],
    item_instance_id: u64,
    tick: u64,
) {
    app.world_mut().send_event(ZhenfaPlaceRequest {
        player,
        pos,
        kind: ZhenfaKind::NetworkArray,
        carrier: ZhenfaCarrierKind::CommonStone,
        qi_invest_ratio: 0.0,
        trigger: None,
        item_instance_id: Some(item_instance_id),
        target_face: None,
        requested_at_tick: tick,
    });
}

fn place_network_array_node(
    app: &mut App,
    player: Entity,
    pos: [i32; 3],
    item_instance_id: u64,
    tick: u64,
) {
    send_network_array_place(app, player, pos, item_instance_id, tick);
    app.update();
}

fn place_basic_network_array(app: &mut App, owner: Entity, start_tick: u64) {
    place_network_array_node(app, owner, [0, 64, 0], 8101, start_tick);
    place_network_array_node(app, owner, [6, 64, 0], 8102, start_tick + 1);
    place_network_array_node(app, owner, [0, 64, 6], 8103, start_tick + 2);
    place_network_array_node(app, owner, [1, 64, 1], 8201, start_tick + 3);
}

fn inventory_still_has_item(app: &App, player: Entity, instance_id: u64) -> bool {
    app.world()
        .get::<PlayerInventory>(player)
        .and_then(|inventory| inventory_item_by_instance_borrow(inventory, instance_id))
        .is_some()
}

fn released_zhenfa_qi_total(transfers: &Events<QiTransfer>) -> f64 {
    transfers
        .iter_current_update_events()
        .filter(|transfer| {
            transfer.reason == QiTransferReason::ReleaseToZone
                && transfer.from.kind == QiAccountKind::Container
                && transfer.from.id.starts_with("zhenfa_trap:")
        })
        .map(|transfer| transfer.amount)
        .sum()
}

fn zhenfa_flag_inventory() -> PlayerInventory {
    let mut inventory = empty_inventory();
    inventory.equipped.insert(
        EQUIP_SLOT_MAIN_HAND.to_string(),
        SlotContents::held_single(array_flag_item(9001)),
    );
    inventory
}

fn deceive_heaven_material_inventory() -> PlayerInventory {
    let mut inventory = zhenfa_flag_inventory();
    inventory.bone_coins = DECEIVE_HEAVEN_BONE_COIN_COST;
    inventory.containers[0].items.push(PlacedItemState {
        row: 0,
        col: 0,
        instance: material_item(
            9101,
            DECEIVE_HEAVEN_SPIRITWOOD_ITEM_ID,
            DECEIVE_HEAVEN_SPIRITWOOD_COST,
        ),
    });
    inventory.containers[0].items.push(PlacedItemState {
        row: 0,
        col: 1,
        instance: material_item(
            9102,
            DECEIVE_HEAVEN_BEAST_BONE_ITEM_ID,
            DECEIVE_HEAVEN_BEAST_BONE_COST,
        ),
    });
    inventory
}

fn ordinary_trap_inventory(item: ItemInstance) -> PlayerInventory {
    let mut inventory = empty_inventory();
    inventory.equipped.insert(
        EQUIP_SLOT_MAIN_HAND.to_string(),
        SlotContents::held_single(item),
    );
    inventory
}

fn scatter_bead_inventory(instance_id: u64) -> PlayerInventory {
    ordinary_trap_inventory(scatter_bead_item(instance_id))
}

fn empty_inventory() -> PlayerInventory {
    PlayerInventory {
        triggered_treasures: Vec::new(),
        revision: InventoryRevision(0),
        containers: vec![ContainerState {
            quick_access: false,
            id: MAIN_PACK_CONTAINER_ID.to_string(),
            name: "main".to_string(),
            rows: 4,
            cols: 6,
            items: Vec::new(),
            owner_instance_id: None,
        }],
        equipped: HashMap::new(),
        hotbar: Default::default(),
        bone_coins: 0,
        max_weight: 45.0,
    }
}

fn drain_rune_draw_anims(app: &mut App) -> Vec<(String, u16)> {
    app.world_mut()
        .resource_mut::<Events<VfxEventRequest>>()
        .drain()
        .filter_map(|request| match request.payload {
            bong_server::schema::vfx_event::VfxEventPayloadV1::PlayAnim {
                target_player,
                anim_id,
                priority,
                ..
            } if anim_id == ANIM_RUNE_DRAW => Some((target_player, priority)),
            _ => None,
        })
        .collect()
}

fn assert_runtime_trap_place_rejected(
    kind: ZhenfaKind,
    item_id: &str,
    request_item_instance_id: Option<u64>,
    request_face: Option<trap_content::TrapTargetFace>,
    inventory_item_instance_id: u64,
    pos: [i32; 3],
    case_label: &str,
) {
    let mut app = app_with_loaded_zhenfa();
    let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
    app.world_mut()
        .entity_mut(owner)
        .insert(ordinary_trap_inventory(trap_item(
            inventory_item_instance_id,
            item_id,
            item_id,
        )));

    app.world_mut().send_event(ZhenfaPlaceRequest {
        player: owner,
        pos,
        kind,
        carrier: ZhenfaCarrierKind::CommonStone,
        qi_invest_ratio: 1.0,
        trigger: None,
        item_instance_id: request_item_instance_id,
        target_face: request_face,
        requested_at_tick: 10,
    });
    app.update();

    assert!(
        app.world()
            .resource::<ZhenfaRegistry>()
            .find_at(pos)
            .is_none(),
        "{kind:?} placement must reject {case_label} before creating a registry entry"
    );
    assert_eq!(
        app.world().get::<Cultivation>(owner).unwrap().qi_current,
        100.0,
        "{kind:?} rejected placement for {case_label} must not debit qi"
    );
    let inventory = app.world().get::<PlayerInventory>(owner).unwrap();
    assert!(
        inventory_item_by_instance_borrow(inventory, inventory_item_instance_id).is_some(),
        "{kind:?} rejected placement for {case_label} must keep the trap item in inventory"
    );
}

fn send_bait_attack(app: &mut App, attacker: Entity, target: Entity, tick: u64) {
    app.world_mut().send_event(AttackIntent {
        attacker,
        target: Some(target),
        issued_at_tick: tick,
        reach: FIST_REACH,
        qi_invest: 0.0,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::Melee,
        debug_command: None,
    });
}

#[test]
fn lingju_tick_applies_cap_bonus_inside_radius_only() {
    let mut app = app_with_loaded_zhenfa();
    let owner = spawn_player(&mut app, "Alice", [0.5, 64.0, 0.5]);
    spawn_plot(&mut app, [20, 64, 0], PLOT_QI_CAP_BASE);
    spawn_plot(&mut app, [21, 64, 0], PLOT_QI_CAP_BASE);

    send_lingju_place(&mut app, owner, [0, 64, 0], 1);
    app.update();
    let instance = app
        .world()
        .resource::<ZhenfaRegistry>()
        .find_at([0, 64, 0])
        .expect("Lingju 应成功放置");
    assert_eq!(
        instance.effect_radius, 20,
        "Lingju 必须使用 profile radius，不能沿用旧 trap_effect_radius 的 0-2 格半径"
    );

    app.world_mut().resource_mut::<CombatClock>().tick = 2;
    app.update();

    assert!(
        (plot_cap(&mut app, [20, 64, 0]) - (PLOT_QI_CAP_BASE + QI_LINGJU_ARRAY_CAP_BONUS)).abs()
            < 1e-6,
        "恰好在 Lingju 半径边缘的 plot 应获得 +QI_LINGJU_ARRAY_CAP_BONUS cap"
    );
    assert!(
        (plot_cap(&mut app, [21, 64, 0]) - PLOT_QI_CAP_BASE).abs() < 1e-6,
        "半径外 1 格 plot 不应被 Lingju 影响"
    );
}

#[test]
fn lingju_cap_bonus_is_clamped_to_plot_qi_cap_max() {
    let mut app = app_with_loaded_zhenfa();
    let owner = spawn_player(&mut app, "Alice", [0.5, 64.0, 0.5]);
    let near_max_cap = PLOT_QI_CAP_MAX - (QI_LINGJU_ARRAY_CAP_BONUS * 0.5);
    spawn_plot(&mut app, [0, 64, 0], near_max_cap);

    send_lingju_place(&mut app, owner, [0, 64, 0], 1);
    app.update();
    app.world_mut().resource_mut::<CombatClock>().tick = 2;
    app.update();

    let actual = plot_cap(&mut app, [0, 64, 0]);
    assert!(
        (actual - PLOT_QI_CAP_MAX).abs() < 1e-6,
        "expected cap={} because Lingju bonus must clamp at PLOT_QI_CAP_MAX; actual={}",
        PLOT_QI_CAP_MAX,
        actual
    );
}

#[test]
fn lingju_decay_clears_cap_bonus_and_emits_decay_event() {
    let mut app = app_with_loaded_zhenfa();
    let owner = spawn_player(&mut app, "Alice", [0.5, 64.0, 0.5]);
    spawn_plot(&mut app, [0, 64, 0], PLOT_QI_CAP_BASE);

    send_lingju_place(&mut app, owner, [0, 64, 0], 1);
    app.update();
    app.world_mut().resource_mut::<CombatClock>().tick = 2;
    app.update();
    assert!((plot_cap(&mut app, [0, 64, 0]) - 2.0).abs() < 1e-6);

    let expires_at_tick = app
        .world()
        .resource::<ZhenfaRegistry>()
        .find_at([0, 64, 0])
        .expect("Lingju 应仍在 registry 中")
        .expires_at_tick;
    app.world_mut().resource_mut::<CombatClock>().tick = expires_at_tick;
    app.update();

    assert!(
        (plot_cap(&mut app, [0, 64, 0]) - PLOT_QI_CAP_BASE).abs() < 1e-6,
        "Lingju decay 后 plot cap 必须恢复原值"
    );
    assert!(app
        .world()
        .resource::<Events<ArrayDecayEvent>>()
        .iter_current_update_events()
        .any(|event| event.kind == ZhenfaKind::Lingju));
}

#[test]
fn lingju_force_break_clears_cap_bonus() {
    let mut app = app_with_loaded_zhenfa();
    let owner = spawn_player(&mut app, "Alice", [0.5, 64.0, 0.5]);
    spawn_plot(&mut app, [0, 64, 0], PLOT_QI_CAP_BASE);

    send_lingju_place(&mut app, owner, [0, 64, 0], 1);
    app.update();
    app.world_mut().resource_mut::<CombatClock>().tick = 2;
    app.update();
    assert!(
        (plot_cap(&mut app, [0, 64, 0]) - (PLOT_QI_CAP_BASE + QI_LINGJU_ARRAY_CAP_BONUS)).abs()
            < 1e-6
    );

    app.world_mut().send_event(ZhenfaDisarmRequest {
        player: owner,
        pos: [0, 64, 0],
        mode: ZhenfaDisarmMode::ForceBreak,
        requested_at_tick: 3,
    });
    app.update();

    assert!(
        (plot_cap(&mut app, [0, 64, 0]) - PLOT_QI_CAP_BASE).abs() < 1e-6,
        "Lingju force break 后 plot cap 必须恢复原值"
    );
}

#[test]
fn overlapping_lingju_arrays_use_boolean_or_not_stacking() {
    let (mut app, layer_entity) = app_with_zhenfa_layer();
    app.world_mut()
        .get_mut::<ChunkLayer>(layer_entity)
        .expect("test layer should carry ChunkLayer")
        .insert_chunk([1, 0], UnloadedChunk::new());
    let owner = spawn_player(&mut app, "Alice", [0.5, 64.0, 0.5]);
    spawn_plot(&mut app, [10, 64, 0], PLOT_QI_CAP_BASE);

    send_lingju_place(&mut app, owner, [0, 64, 0], 1);
    app.update();
    send_lingju_place(&mut app, owner, [20, 64, 0], 2);
    app.update();
    app.world_mut().resource_mut::<CombatClock>().tick = 3;
    app.update();

    let boosted = PLOT_QI_CAP_BASE + QI_LINGJU_ARRAY_CAP_BONUS;
    assert!(
        (plot_cap(&mut app, [10, 64, 0]) - boosted).abs() < 1e-6,
        "双 Lingju 覆盖同一 plot 只能取 OR/max，不能叠加到 +2.0"
    );

    app.world_mut().send_event(ZhenfaDisarmRequest {
        player: owner,
        pos: [0, 64, 0],
        mode: ZhenfaDisarmMode::ForceBreak,
        requested_at_tick: 4,
    });
    app.update();
    assert!(
        (plot_cap(&mut app, [10, 64, 0]) - boosted).abs() < 1e-6,
        "拆掉一个 Lingju 后，仍被另一个覆盖的 plot 应保持 boosted"
    );

    app.world_mut()
        .entity_mut(owner)
        .insert(Position::new([20.5, 64.0, 0.5]));
    app.world_mut().send_event(ZhenfaDisarmRequest {
        player: owner,
        pos: [20, 64, 0],
        mode: ZhenfaDisarmMode::ForceBreak,
        requested_at_tick: 5,
    });
    app.update();
    assert!(
        (plot_cap(&mut app, [10, 64, 0]) - PLOT_QI_CAP_BASE).abs() < 1e-6,
        "最后一个 Lingju 清除后 plot cap 才恢复基线"
    );
}

#[test]
fn network_array_and_full_lingju_use_max_bonus_not_stacking() {
    let mut app = app_with_loaded_zhenfa();
    let owner = spawn_player_with_inventory(
        &mut app,
        "Alice",
        [0.5, 64.0, 0.5],
        network_array_test_inventory(),
    );
    spawn_plot(&mut app, [2, 64, 2], PLOT_QI_CAP_BASE);

    send_lingju_place(&mut app, owner, [8, 64, 8], 1);
    app.update();
    place_basic_network_array(&mut app, owner, 2);
    app.world_mut().resource_mut::<CombatClock>().tick = 10;
    app.update();

    let expected = PLOT_QI_CAP_BASE + QI_LINGJU_ARRAY_CAP_BONUS;
    assert!(
        (plot_cap(&mut app, [2, 64, 2]) - expected).abs() < 1e-6,
        "Full Lingju + NetworkArray 覆盖同 plot 必须取 max(+1.0)，不能叠加到 +1.5"
    );
}

#[test]
fn lingju_deploy_event_feedback_vfx_and_narration_are_emitted() {
    let mut app = app_with_loaded_zhenfa();
    app.insert_resource(ZoneRegistry::fallback());
    app.add_event::<VfxEventRequest>();
    let owner = spawn_player(&mut app, "Alice", [0.5, 64.0, 0.5]);

    send_lingju_place(&mut app, owner, [0, 64, 0], 1);
    app.update();

    let ling_events = app.world().resource::<Events<LingArrayDeployEvent>>();
    let deploy = ling_events
        .iter_current_update_events()
        .find(|event| event.owner == owner)
        .expect("Lingju 放置应继续发 LingArrayDeployEvent");
    assert!(
        deploy.tiandao_gaze_weight > 0.0,
        "LingArrayDeployEvent.tiandao_gaze_weight 必须为正，给天道 gaze 审计消费"
    );

    let vfx_events = app.world().resource::<Events<VfxEventRequest>>();
    assert!(vfx_events
        .iter_current_update_events()
        .any(|event| matches!(
            &event.payload,
            bong_server::schema::vfx_event::VfxEventPayloadV1::SpawnParticle { event_id, .. }
                if event_id == gameplay_vfx::LINGJU_ACTIVATE
        )));

    let narrations = app
        .world_mut()
        .resource_mut::<PendingGameplayNarrations>()
        .drain();
    assert_eq!(
        narrations.len(),
        3,
        "Lingju 激活应入队两条感知 + 一条天道叙事"
    );
    assert!(narrations.iter().all(|narration| matches!(
        narration.scope,
        bong_server::schema::common::NarrationScope::Zone
    )));
    assert!(narrations
        .iter()
        .any(|narration| narration.style == NarrationStyle::Narration));
}

#[test]
fn non_lingju_place_does_not_emit_lingju_feedback() {
    let mut app = app_with_loaded_zhenfa();
    app.insert_resource(ZoneRegistry::fallback());
    app.add_event::<VfxEventRequest>();
    let owner = spawn_player(&mut app, "Alice", [0.5, 64.0, 0.5]);

    app.world_mut().send_event(ZhenfaPlaceRequest {
        player: owner,
        pos: [0, 64, 0],
        kind: ZhenfaKind::Ward,
        carrier: ZhenfaCarrierKind::LingqiBlock,
        qi_invest_ratio: 0.20,
        trigger: None,
        item_instance_id: None,
        target_face: None,
        requested_at_tick: 1,
    });
    app.update();

    assert!(
        app.world()
            .resource::<ZhenfaRegistry>()
            .find_at([0, 64, 0])
            .is_some(),
        "expected Ward place to succeed because this test must exercise a real non-Lingju place path"
    );
    let vfx: Vec<&VfxEventRequest> = app
        .world()
        .resource::<Events<VfxEventRequest>>()
        .iter_current_update_events()
        .collect();
    assert!(
        !vfx.iter().any(|request| matches!(
            &request.payload,
            bong_server::schema::vfx_event::VfxEventPayloadV1::SpawnParticle { event_id, .. }
                if event_id == gameplay_vfx::LINGJU_ACTIVATE
        )),
        "expected no LINGJU_ACTIVATE particle because non-Lingju place must not run \
         Lingju feedback, got {vfx:?}"
    );
    // plan-skill-av-relink-v1 P1 — 落阵成功（不分 kind）发 rune_draw 画符动画；
    // 它与 Lingju 专属反馈无关，本测试只锁"非 Lingju 不发 Lingju 反馈"契约。
    assert!(
        vfx.iter().any(|request| matches!(
            &request.payload,
            bong_server::schema::vfx_event::VfxEventPayloadV1::PlayAnim { anim_id, .. }
                    if anim_id == ANIM_RUNE_DRAW
        )),
        "expected rune_draw PlayAnim on successful non-Lingju place \
         (plan-skill-av-relink-v1 P1), got {vfx:?}"
    );
    let narrations = app
        .world_mut()
        .resource_mut::<PendingGameplayNarrations>()
        .drain();
    assert!(
        narrations.is_empty(),
        "expected no PendingGameplayNarrations because non-Lingju place must not run Lingju feedback; actual={:?}",
        narrations
    );
}

#[test]
fn scatter_bead_full_zone_routes_overflow_without_changing_zone() {
    let mut app = app_with_loaded_zhenfa();
    let mut zones = ZoneRegistry::fallback();
    zones.zones[0].spirit_qi = 1.0;
    app.insert_resource(zones);
    let owner = spawn_player_with_inventory(
        &mut app,
        "Alice",
        [0.5, 64.0, 0.5],
        scatter_bead_inventory(7002),
    );

    app.world_mut().send_event(ScatterBeadUseRequest {
        player: owner,
        item_instance_id: 7002,
        bury_pos: None,
        requested_at_tick: 1,
    });
    app.update();

    let zone = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
        .expect("spawn zone should exist");
    assert_eq!(zone.spirit_qi, 1.0, "满 cap zone 不应继续升高");

    let ledger = app.world().resource::<WorldQiAccount>();
    let overflow_total: f64 = ledger
        .transfers()
        .iter()
        .filter(|transfer| transfer.to.kind == bong_server::qi_physics::QiAccountKind::Overflow)
        .map(|transfer| transfer.amount)
        .sum();
    assert!(
        (overflow_total - QI_SCATTER_BEAD_CAPACITY).abs() < 1e-9,
        "zone 已满时散灵珠真元必须进入 overflow account，不能凭空消失"
    );
}

#[test]
fn scatter_bead_repeated_instance_is_rejected_after_first_consume() {
    let mut app = app_with_loaded_zhenfa();
    app.insert_resource(ZoneRegistry::fallback());
    let owner = spawn_player_with_inventory(
        &mut app,
        "Alice",
        [0.5, 64.0, 0.5],
        scatter_bead_inventory(7003),
    );

    for tick in [1, 2] {
        app.world_mut().send_event(ScatterBeadUseRequest {
            player: owner,
            item_instance_id: 7003,
            bury_pos: None,
            requested_at_tick: tick,
        });
        app.world_mut().resource_mut::<CombatClock>().tick = tick;
        app.update();
    }

    let ledger = app.world().resource::<WorldQiAccount>();
    assert_eq!(
        ledger.transfers().len(),
        1,
        "第二次使用同一已消耗散灵珠 instance 必须被拒绝，不能重复转账"
    );
}

#[test]
fn activate_emits_vfx() {
    let mut app = app_with_zhenfa();
    app.add_event::<VfxEventRequest>();
    let owner = spawn_player(&mut app, "owner", [0.0, 64.0, 0.0]);
    let _target = spawn_player(&mut app, "intruder", [1.5, 64.0, 0.5]);
    let anchor_entity = app.world_mut().spawn_empty().id();
    let id = app
        .world_mut()
        .resource_mut::<ZhenfaRegistry>()
        .insert(ZhenfaInstance {
            id: 0,
            kind: ZhenfaKind::Trap,
            owner,
            owner_player_id: "player:owner".to_string(),
            pos: [1, 64, 0],
            carrier: ZhenfaCarrierKind::CommonStone,
            qi_invest_ratio: 0.2,
            qi_invest_amount: 20.0,
            realm_at_cast: Realm::Induce,
            mastery_at_cast: 0.0,
            effect_radius: 1,
            ward_radius: 1,
            placed_at_tick: 1,
            expires_at_tick: 100,
            triggered_at: None,
            trigger: None,
            color_main: ColorKind::Intricate,
            color_secondary: None,
            anchor_entity,
        })
        .expect("insert trap");

    app.world_mut().send_event(ZhenfaTriggerRequest {
        player: owner,
        instance_id: Some(id),
        requested_at_tick: 10,
    });
    app.update();

    let events = app.world().resource::<Events<VfxEventRequest>>();
    let emitted = events
        .iter_current_update_events()
        .next()
        .expect("zhenfa trigger should emit vfx");
    match &emitted.payload {
        bong_server::schema::vfx_event::VfxEventPayloadV1::SpawnParticle { event_id, .. } => {
            assert_eq!(event_id, gameplay_vfx::ZHENFA_TRAP);
        }
        other => panic!("expected SpawnParticle, got {other:?}"),
    }
}

#[test]
fn placement_clamps_to_carrier_cap_and_debits_qi() {
    let mut app = app_with_loaded_zhenfa();
    let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);

    app.world_mut().send_event(ZhenfaPlaceRequest {
        player: owner,
        pos: [1, 64, 1],
        kind: ZhenfaKind::Trap,
        carrier: ZhenfaCarrierKind::CommonStone,
        qi_invest_ratio: 0.80,
        trigger: Some("proximity".to_string()),
        item_instance_id: None,
        target_face: None,
        requested_at_tick: 10,
    });
    app.update();

    let cultivation = app.world().get::<Cultivation>(owner).unwrap();
    assert_eq!(cultivation.qi_current, 90.0);
    let registry = app.world().resource::<ZhenfaRegistry>();
    let instance = registry.find_at([1, 64, 1]).unwrap();
    assert_eq!(instance.qi_invest_ratio, 0.10);
    assert_eq!(instance.effect_radius, 0);
    assert_eq!(registry.len(), 1);
}

#[test]
fn trap_place_success_emits_rune_draw_animation_for_owner() {
    let mut app = app_with_loaded_zhenfa();
    app.add_event::<VfxEventRequest>();
    let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
    let owner_uuid = app.world().get::<UniqueId>(owner).unwrap().0.to_string();

    app.world_mut().send_event(ZhenfaPlaceRequest {
        player: owner,
        pos: [1, 64, 1],
        kind: ZhenfaKind::Trap,
        carrier: ZhenfaCarrierKind::CommonStone,
        qi_invest_ratio: 0.80,
        trigger: Some("proximity".to_string()),
        item_instance_id: None,
        target_face: None,
        requested_at_tick: 10,
    });
    app.update();

    assert_eq!(
        app.world().resource::<ZhenfaRegistry>().len(),
        1,
        "前置条件破坏：Trap 落阵应成功（本测试要走真实成功路径）"
    );
    let anims = drain_rune_draw_anims(&mut app);
    assert_eq!(
        anims.len(),
        1,
        "落阵成功应恰发一条 rune_draw 画符动画，实际 {anims:?}"
    );
    assert_eq!(
        anims[0].0, owner_uuid,
        "rune_draw 应发给落阵者本人（target_player = owner uuid）"
    );
    assert_eq!(
        anims[0].1, COMBAT_PRIORITY,
        "rune_draw 优先级应为战斗动作档"
    );
}

#[test]
fn each_successful_place_emits_its_own_rune_draw() {
    let mut app = app_with_loaded_zhenfa();
    app.add_event::<VfxEventRequest>();
    let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);

    for (tick, pos) in [(10_u64, [1, 64, 1]), (20, [3, 64, 3])] {
        app.world_mut().send_event(ZhenfaPlaceRequest {
            player: owner,
            pos,
            kind: ZhenfaKind::Trap,
            carrier: ZhenfaCarrierKind::CommonStone,
            qi_invest_ratio: 0.20,
            trigger: Some("proximity".to_string()),
            item_instance_id: None,
            target_face: None,
            requested_at_tick: tick,
        });
    }
    app.update();

    assert_eq!(
        app.world().resource::<ZhenfaRegistry>().len(),
        2,
        "前置条件破坏：两次不同位置的落阵都应成功"
    );
    assert_eq!(
        drain_rune_draw_anims(&mut app).len(),
        2,
        "每次成功落阵各配一次 rune_draw（1:1）"
    );
}

#[test]
fn rejected_place_does_not_emit_rune_draw() {
    let mut app = app_with_loaded_zhenfa();
    app.add_event::<VfxEventRequest>();
    let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
    app.world_mut().get_mut::<Cultivation>(owner).unwrap().realm = Realm::Solidify;

    app.world_mut().send_event(ZhenfaPlaceRequest {
        player: owner,
        pos: [1, 64, 1],
        kind: ZhenfaKind::DeceiveHeaven,
        carrier: ZhenfaCarrierKind::BeastCoreInlaid,
        qi_invest_ratio: 0.80,
        trigger: None,
        item_instance_id: None,
        target_face: None,
        requested_at_tick: 10,
    });
    app.update();

    assert_eq!(
        app.world().resource::<ZhenfaRegistry>().len(),
        0,
        "前置条件破坏：缺料的 DeceiveHeaven 落阵必须被拒绝"
    );
    assert!(
        drain_rune_draw_anims(&mut app).is_empty(),
        "落阵被拒绝时不应发 rune_draw 画符动画"
    );
}

#[test]
fn place_on_unloaded_chunk_does_not_emit_rune_draw() {
    let (mut app, _layer) = app_with_zhenfa_unloaded_layer();
    app.add_event::<VfxEventRequest>();
    let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);

    app.world_mut().send_event(ZhenfaPlaceRequest {
        player: owner,
        pos: [1, 64, 1],
        kind: ZhenfaKind::Trap,
        carrier: ZhenfaCarrierKind::CommonStone,
        qi_invest_ratio: 0.80,
        trigger: Some("proximity".to_string()),
        item_instance_id: None,
        target_face: None,
        requested_at_tick: 10,
    });
    app.update();

    assert_eq!(
        app.world().resource::<ZhenfaRegistry>().len(),
        0,
        "前置条件破坏：未加载 chunk 上的落阵必须失败回滚"
    );
    assert!(
        drain_rune_draw_anims(&mut app).is_empty(),
        "落阵失败回滚时不应发 rune_draw 画符动画"
    );
}

#[test]
fn deceive_heaven_rejects_when_material_cost_is_missing() {
    let mut app = app_with_loaded_zhenfa();
    let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
    app.world_mut().get_mut::<Cultivation>(owner).unwrap().realm = Realm::Solidify;

    app.world_mut().send_event(ZhenfaPlaceRequest {
        player: owner,
        pos: [1, 64, 1],
        kind: ZhenfaKind::DeceiveHeaven,
        carrier: ZhenfaCarrierKind::BeastCoreInlaid,
        qi_invest_ratio: 0.80,
        trigger: None,
        item_instance_id: None,
        target_face: None,
        requested_at_tick: 10,
    });
    app.update();

    assert_eq!(app.world().resource::<ZhenfaRegistry>().len(), 0);
    let cultivation = app.world().get::<Cultivation>(owner).unwrap();
    assert_eq!(cultivation.qi_current, 100.0);
}

#[test]
fn deceive_heaven_rejects_equipped_materials() {
    let mut app = app_with_loaded_zhenfa();
    let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
    app.world_mut().get_mut::<Cultivation>(owner).unwrap().realm = Realm::Solidify;

    let mut inventory = empty_inventory();
    inventory.bone_coins = DECEIVE_HEAVEN_BONE_COIN_COST;
    inventory.equipped.insert(
        EQUIP_SLOT_MAIN_HAND.to_string(),
        SlotContents::held_single(array_flag_item(9001)),
    );
    inventory.equipped.insert(
        EQUIP_SLOT_OFF_HAND.to_string(),
        SlotContents::held_single(material_item(
            9101,
            DECEIVE_HEAVEN_SPIRITWOOD_ITEM_ID,
            DECEIVE_HEAVEN_SPIRITWOOD_COST,
        )),
    );
    inventory.equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents::worn_single(material_item(
            9102,
            DECEIVE_HEAVEN_BEAST_BONE_ITEM_ID,
            DECEIVE_HEAVEN_BEAST_BONE_COST,
        )),
    );
    app.world_mut().entity_mut(owner).insert(inventory);

    app.world_mut().send_event(ZhenfaPlaceRequest {
        player: owner,
        pos: [1, 64, 1],
        kind: ZhenfaKind::DeceiveHeaven,
        carrier: ZhenfaCarrierKind::BeastCoreInlaid,
        qi_invest_ratio: 0.80,
        trigger: None,
        item_instance_id: None,
        target_face: None,
        requested_at_tick: 10,
    });
    app.update();

    assert_eq!(
        app.world().resource::<ZhenfaRegistry>().len(),
        0,
        "expected no DeceiveHeaven instance because equipped materials are not valid material inputs"
    );
    let cultivation = app.world().get::<Cultivation>(owner).unwrap();
    assert_eq!(
        cultivation.qi_current, 100.0,
        "expected qi_current 100.0 because rejected placement must not invest qi, actual {}",
        cultivation.qi_current
    );
    let inventory = app.world().get::<PlayerInventory>(owner).unwrap();
    assert_eq!(
        inventory.bone_coins, DECEIVE_HEAVEN_BONE_COIN_COST,
        "expected bone_coins {} because rejected placement must not spend material cost, actual {}",
        DECEIVE_HEAVEN_BONE_COIN_COST, inventory.bone_coins
    );
    assert_eq!(
        inventory
            .equipped
            .get(EQUIP_SLOT_OFF_HAND)
            .and_then(|slot| slot.held.as_ref())
            .map(|item| (item.instance_id, item.stack_count)),
        Some((9101, DECEIVE_HEAVEN_SPIRITWOOD_COST)),
        "expected off-hand spiritwood instance/stack unchanged because rejected placement must not mutate equipped slots"
    );
    assert_eq!(
        inventory
            .equipped
            .get(EQUIP_SLOT_CHEST)
            .and_then(|slot| slot.worn.first())
            .map(|item| (item.instance_id, item.stack_count)),
        Some((9102, DECEIVE_HEAVEN_BEAST_BONE_COST)),
        "expected chest beast-bone instance/stack unchanged because rejected placement must not mutate equipped slots"
    );
}

#[test]
fn deceive_heaven_instance_lasts_exactly_thirty_minutes_without_duration_bonuses() {
    let mut app = app_with_loaded_zhenfa();
    let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
    app.world_mut().entity_mut(owner).insert(Cultivation {
        realm: Realm::Solidify,
        qi_current: 100.0,
        qi_max: 100.0,
        ..Default::default()
    });
    app.world_mut().entity_mut(owner).insert(QiColor {
        main: ColorKind::Solid,
        is_chaotic: false,
        ..Default::default()
    });
    let mut modifiers = InsightModifiers::new();
    modifiers.zhenfa_concealment = 10.0;
    app.world_mut().entity_mut(owner).insert(modifiers);
    app.world_mut()
        .entity_mut(owner)
        .insert(deceive_heaven_material_inventory());

    app.world_mut().send_event(ZhenfaPlaceRequest {
        player: owner,
        pos: [1, 64, 1],
        kind: ZhenfaKind::DeceiveHeaven,
        carrier: ZhenfaCarrierKind::BeastCoreInlaid,
        qi_invest_ratio: 0.80,
        trigger: None,
        item_instance_id: None,
        target_face: None,
        requested_at_tick: 10,
    });
    app.update();

    let instance = app
        .world()
        .resource::<ZhenfaRegistry>()
        .find_at([1, 64, 1])
        .expect("欺天阵应成功放置");
    assert_eq!(
        instance.expires_at_tick - instance.placed_at_tick,
        DECEIVE_HEAVEN_DURATION_TICKS,
        "欺天阵生产实例必须固定 30 分钟，不吃专精/颜色时长加成"
    );
}

#[test]
fn deceive_heaven_expiry_releases_sealed_qi_to_zone() {
    let mut app = app_with_loaded_zhenfa();
    app.insert_resource(ZoneRegistry::fallback());
    let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
    app.world_mut().entity_mut(owner).insert(Cultivation {
        realm: Realm::Solidify,
        qi_current: 100.0,
        qi_max: 100.0,
        ..Default::default()
    });
    app.world_mut()
        .entity_mut(owner)
        .insert(deceive_heaven_material_inventory());

    app.world_mut().send_event(ZhenfaPlaceRequest {
        player: owner,
        pos: [1, 64, 1],
        kind: ZhenfaKind::DeceiveHeaven,
        carrier: ZhenfaCarrierKind::BeastCoreInlaid,
        qi_invest_ratio: 0.80,
        trigger: None,
        item_instance_id: None,
        target_face: None,
        requested_at_tick: 10,
    });
    app.update();
    app.world_mut().resource_mut::<CombatClock>().tick = 10 + DECEIVE_HEAVEN_DURATION_TICKS;
    app.update();

    assert!(app
        .world()
        .resource::<ZhenfaRegistry>()
        .find_at([1, 64, 1])
        .is_none());
    assert!(
        (released_zhenfa_qi_total(app.world().resource::<Events<QiTransfer>>()) - 80.0).abs()
            < f64::EPSILON
    );
}

#[test]
fn placement_writes_and_disarm_removes_custom_block() {
    let (mut app, layer_entity) = app_with_zhenfa_layer();
    let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
    let pos = [1, 64, 1];

    app.world_mut().send_event(ZhenfaPlaceRequest {
        player: owner,
        pos,
        kind: ZhenfaKind::Trap,
        carrier: ZhenfaCarrierKind::LingqiBlock,
        qi_invest_ratio: 0.10,
        trigger: None,
        item_instance_id: None,
        target_face: None,
        requested_at_tick: 10,
    });
    app.update();

    assert_eq!(
        layer_block_state(&app, layer_entity, pos),
        Some(zhenfa_eye_state(false))
    );

    app.world_mut().send_event(ZhenfaDisarmRequest {
        player: owner,
        pos,
        mode: ZhenfaDisarmMode::ForceBreak,
        requested_at_tick: 11,
    });
    app.update();

    assert_eq!(
        layer_block_state(&app, layer_entity, pos),
        Some(BlockState::AIR)
    );
    assert!(app
        .world()
        .resource::<ZhenfaRegistry>()
        .find_at(pos)
        .is_none());
}

#[test]
fn shrine_ward_writes_charged_custom_block() {
    let (mut app, layer_entity) = app_with_zhenfa_layer();
    let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
    let pos = [0, 64, 0];

    app.world_mut().send_event(ZhenfaPlaceRequest {
        player: owner,
        pos,
        kind: ZhenfaKind::ShrineWard,
        carrier: ZhenfaCarrierKind::LingqiBlock,
        qi_invest_ratio: 0.20,
        trigger: None,
        item_instance_id: None,
        target_face: None,
        requested_at_tick: 10,
    });
    app.update();

    assert_eq!(
        layer_block_state(&app, layer_entity, pos),
        Some(zhenfa_eye_state(true))
    );
    assert!(app
        .world()
        .resource::<ZhenfaRegistry>()
        .find_at(pos)
        .is_some());
}

#[test]
fn placement_rejects_unloaded_chunk_without_qi_debit_or_registry_entry() {
    let (mut app, layer_entity) = app_with_zhenfa_unloaded_layer();
    let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
    let pos = [1, 64, 1];

    app.world_mut().send_event(ZhenfaPlaceRequest {
        player: owner,
        pos,
        kind: ZhenfaKind::Trap,
        carrier: ZhenfaCarrierKind::LingqiBlock,
        qi_invest_ratio: 0.20,
        trigger: None,
        item_instance_id: None,
        target_face: None,
        requested_at_tick: 10,
    });
    app.update();

    assert_eq!(
        app.world().get::<Cultivation>(owner).unwrap().qi_current,
        100.0
    );
    assert_eq!(app.world().resource::<ZhenfaRegistry>().len(), 0);
    assert_eq!(layer_block_state(&app, layer_entity, pos), None);
}

#[test]
fn placement_rejects_missing_overworld_layer_without_qi_debit_or_registry_entry() {
    let mut app = app_with_zhenfa();
    let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);

    app.world_mut().send_event(ZhenfaPlaceRequest {
        player: owner,
        pos: [1, 64, 1],
        kind: ZhenfaKind::Trap,
        carrier: ZhenfaCarrierKind::LingqiBlock,
        qi_invest_ratio: 0.20,
        trigger: None,
        item_instance_id: None,
        target_face: None,
        requested_at_tick: 10,
    });
    app.update();

    assert_eq!(
        app.world().get::<Cultivation>(owner).unwrap().qi_current,
        100.0
    );
    assert_eq!(app.world().resource::<ZhenfaRegistry>().len(), 0);
}

#[test]
fn duplicate_same_block_is_rejected_without_second_qi_debit() {
    let mut app = app_with_loaded_zhenfa();
    let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);

    for tick in [1, 2] {
        app.world_mut().send_event(ZhenfaPlaceRequest {
            player: owner,
            pos: [2, 64, 2],
            kind: ZhenfaKind::Trap,
            carrier: ZhenfaCarrierKind::LingqiBlock,
            qi_invest_ratio: 0.10,
            trigger: None,
            item_instance_id: None,
            target_face: None,
            requested_at_tick: tick,
        });
    }
    app.update();

    assert_eq!(app.world().resource::<ZhenfaRegistry>().len(), 1);
    assert_eq!(
        app.world().get::<Cultivation>(owner).unwrap().qi_current,
        90.0
    );
}

#[test]
fn placement_requires_array_flag() {
    let mut app = app_with_zhenfa();
    let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
    app.world_mut().entity_mut(owner).insert(empty_inventory());

    app.world_mut().send_event(ZhenfaPlaceRequest {
        player: owner,
        pos: [2, 64, 2],
        kind: ZhenfaKind::Trap,
        carrier: ZhenfaCarrierKind::LingqiBlock,
        qi_invest_ratio: 0.10,
        trigger: None,
        item_instance_id: None,
        target_face: None,
        requested_at_tick: 1,
    });
    app.update();

    assert_eq!(app.world().resource::<ZhenfaRegistry>().len(), 0);
    assert_eq!(
        app.world().get::<Cultivation>(owner).unwrap().qi_current,
        100.0
    );
}

#[test]
fn place_and_disarm_trap_runtime_p0_variants() {
    let cases = [
        (
            ZhenfaKind::BeastTrap,
            trap_content::BEAST_TRAP_ITEM_ID,
            trap_content::TrapTargetFace::North,
        ),
        (
            ZhenfaKind::TripWire,
            trap_content::TRIP_WIRE_ITEM_ID,
            trap_content::TrapTargetFace::North,
        ),
        (
            ZhenfaKind::DecoyStake,
            trap_content::BAIT_STAKE_ITEM_ID,
            trap_content::TrapTargetFace::Top,
        ),
    ];

    for (idx, (kind, item_id, target_face)) in cases.into_iter().enumerate() {
        let mut app = app_with_loaded_zhenfa();
        let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
        let item_instance_id = 9200 + idx as u64;
        let pos = [1 + idx as i32, 64, 1];
        app.world_mut()
            .entity_mut(owner)
            .insert(ordinary_trap_inventory(trap_item(
                item_instance_id,
                item_id,
                item_id,
            )));

        app.world_mut().send_event(ZhenfaPlaceRequest {
            player: owner,
            pos,
            kind,
            carrier: ZhenfaCarrierKind::CommonStone,
            qi_invest_ratio: 1.0,
            trigger: None,
            item_instance_id: Some(item_instance_id),
            target_face: Some(target_face),
            requested_at_tick: 10,
        });
        app.update();

        let registry = app.world().resource::<ZhenfaRegistry>();
        let instance = registry.find_at(pos).expect("P0 trap should be placed");
        assert_eq!(instance.kind, kind);
        assert_eq!(
            instance.qi_invest_amount, 0.0,
            "{kind:?} is a mundane trap and must not seal qi"
        );
        assert_eq!(
            app.world().get::<Cultivation>(owner).unwrap().qi_current,
            100.0,
            "{kind:?} placement must not debit qi"
        );
        let inventory = app.world().get::<PlayerInventory>(owner).unwrap();
        assert!(inventory_item_by_instance_borrow(inventory, item_instance_id).is_none());

        let qi_before_disarm = app.world().get::<Cultivation>(owner).unwrap().qi_current;
        let wounds_before_disarm = app.world().get::<Wounds>(owner).unwrap().clone();
        let contam_count_before_disarm = app
            .world()
            .get::<Contamination>(owner)
            .unwrap()
            .entries
            .len();
        let lung_integrity_before_disarm = app
            .world()
            .get::<MeridianSystem>(owner)
            .unwrap()
            .get(MeridianId::Lung)
            .integrity;
        app.world_mut().send_event(ZhenfaDisarmRequest {
            player: owner,
            pos,
            mode: ZhenfaDisarmMode::ForceBreak,
            requested_at_tick: 20,
        });
        app.update();

        assert!(
            app.world()
                .resource::<ZhenfaRegistry>()
                .find_at(pos)
                .is_none(),
            "{kind:?} must be removable through ZhenfaDisarm"
        );
        let wounds_after_disarm = app.world().get::<Wounds>(owner).unwrap();
        assert_eq!(
            app.world().get::<Cultivation>(owner).unwrap().qi_current,
            qi_before_disarm,
            "{kind:?} ForceBreak is a mundane mechanical cleanup and must not debit qi"
        );
        assert_eq!(
            wounds_after_disarm.health_current, wounds_before_disarm.health_current,
            "{kind:?} ForceBreak must not apply zhenfa backlash health loss"
        );
        assert_eq!(
            wounds_after_disarm.entries.len(),
            wounds_before_disarm.entries.len(),
            "{kind:?} ForceBreak must not append backlash wounds"
        );
        assert_eq!(
            app.world()
                .get::<Contamination>(owner)
                .unwrap()
                .entries
                .len(),
            contam_count_before_disarm,
            "{kind:?} ForceBreak must not append contamination entries"
        );
        assert_eq!(
            app.world()
                .get::<MeridianSystem>(owner)
                .unwrap()
                .get(MeridianId::Lung)
                .integrity,
            lung_integrity_before_disarm,
            "{kind:?} ForceBreak must not damage Lung meridian integrity"
        );
    }
}

#[test]
fn runtime_p0_traps_reject_bad_item_face_and_missing_item() {
    let cases = [
        (
            ZhenfaKind::BeastTrap,
            trap_content::BEAST_TRAP_ITEM_ID,
            trap_content::TrapTargetFace::North,
            trap_content::TrapTargetFace::Bottom,
        ),
        (
            ZhenfaKind::TripWire,
            trap_content::TRIP_WIRE_ITEM_ID,
            trap_content::TrapTargetFace::North,
            trap_content::TrapTargetFace::Bottom,
        ),
        (
            ZhenfaKind::DecoyStake,
            trap_content::BAIT_STAKE_ITEM_ID,
            trap_content::TrapTargetFace::Top,
            trap_content::TrapTargetFace::North,
        ),
    ];

    for (idx, (kind, item_id, valid_face, invalid_face)) in cases.into_iter().enumerate() {
        let base_id = 9300 + (idx as u64 * 10);
        assert_runtime_trap_place_rejected(
            kind,
            item_id,
            Some(base_id + 99),
            Some(valid_face),
            base_id,
            [10 + idx as i32, 64, 1],
            "wrong item_instance_id",
        );
        assert_runtime_trap_place_rejected(
            kind,
            item_id,
            Some(base_id),
            Some(invalid_face),
            base_id,
            [20 + idx as i32, 64, 1],
            "wrong target_face",
        );
        assert_runtime_trap_place_rejected(
            kind,
            item_id,
            None,
            Some(valid_face),
            base_id,
            [30 + idx as i32, 64, 1],
            "missing item_instance_id",
        );
    }
}

#[test]
fn beast_trap_ignores_non_fauna_and_high_tier_beasts() {
    let mut app = app_with_loaded_zhenfa();
    let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
    let bob = spawn_player(&mut app, "Bob", [1.5, 64.0, 1.5]);
    let high_tier_beast = app
        .world_mut()
        .spawn((
            Position::new([1.5, 64.0, 1.5]),
            Wounds::default(),
            FaunaTag::new(BeastKind::BlueSpider),
        ))
        .id();
    app.world_mut()
        .entity_mut(owner)
        .insert(ordinary_trap_inventory(trap_item(
            9402,
            trap_content::BEAST_TRAP_ITEM_ID,
            trap_content::BEAST_TRAP_ITEM_ID,
        )));

    app.world_mut().send_event(ZhenfaPlaceRequest {
        player: owner,
        pos: [1, 64, 1],
        kind: ZhenfaKind::BeastTrap,
        carrier: ZhenfaCarrierKind::CommonStone,
        qi_invest_ratio: 1.0,
        trigger: None,
        item_instance_id: Some(9402),
        target_face: Some(trap_content::TrapTargetFace::North),
        requested_at_tick: 10,
    });
    app.update();
    app.world_mut().resource_mut::<CombatClock>().tick = 11;
    app.update();

    assert!(
        app.world()
            .resource::<ZhenfaRegistry>()
            .find_at([1, 64, 1])
            .is_some(),
        "BeastTrap must stay armed when only players or high-tier FaunaTag beasts enter"
    );
    assert!(
        app.world()
            .resource::<Events<ApplyStatusEffectIntent>>()
            .iter_current_update_events()
            .next()
            .is_none(),
        "non-beast and high-tier beast entries must not receive Immobilized"
    );
    assert!(
        app.world().get::<Wounds>(bob).unwrap().entries.is_empty()
            && app
                .world()
                .get::<Wounds>(high_tier_beast)
                .unwrap()
                .entries
                .is_empty(),
        "non-targets must not receive BeastTrap wound entries"
    );
}

#[test]
fn trip_wire_owner_alone_does_not_self_alert() {
    let mut app = app_with_loaded_zhenfa();
    let owner = spawn_player(&mut app, "Alice", [1.5, 64.0, 1.5]);
    app.world_mut()
        .entity_mut(owner)
        .insert(ordinary_trap_inventory(trap_item(
            9404,
            trap_content::TRIP_WIRE_ITEM_ID,
            trap_content::TRIP_WIRE_ITEM_ID,
        )));

    app.world_mut().send_event(ZhenfaPlaceRequest {
        player: owner,
        pos: [1, 64, 1],
        kind: ZhenfaKind::TripWire,
        carrier: ZhenfaCarrierKind::CommonStone,
        qi_invest_ratio: 1.0,
        trigger: None,
        item_instance_id: Some(9404),
        target_face: Some(trap_content::TrapTargetFace::North),
        requested_at_tick: 10,
    });
    app.update();
    app.world_mut().resource_mut::<CombatClock>().tick = 11;
    app.update();

    assert!(
        app.world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain()
            .is_empty(),
        "TripWire must not alert on its owner standing inside the trigger column"
    );
}

#[test]
fn bait_stake_breaks_after_four_attacks_and_emits_break_vfx() {
    let mut app = app_with_loaded_zhenfa();
    app.add_event::<VfxEventRequest>();
    let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
    let attacker = spawn_player(&mut app, "Bob", [1.5, 64.0, 1.5]);
    app.world_mut()
        .entity_mut(owner)
        .insert(ordinary_trap_inventory(trap_item(
            9406,
            trap_content::BAIT_STAKE_ITEM_ID,
            trap_content::BAIT_STAKE_ITEM_ID,
        )));

    app.world_mut().send_event(ZhenfaPlaceRequest {
        player: owner,
        pos: [1, 64, 1],
        kind: ZhenfaKind::DecoyStake,
        carrier: ZhenfaCarrierKind::CommonStone,
        qi_invest_ratio: 1.0,
        trigger: None,
        item_instance_id: Some(9406),
        target_face: Some(trap_content::TrapTargetFace::Top),
        requested_at_tick: 10,
    });
    app.update();
    let anchor = app
        .world()
        .resource::<ZhenfaRegistry>()
        .find_at([1, 64, 1])
        .expect("bait_stake should be placed before attack test")
        .anchor_entity;

    for tick in 11..14 {
        send_bait_attack(&mut app, attacker, anchor, tick);
        app.update();
    }
    assert!(
        app.world()
            .resource::<ZhenfaRegistry>()
            .find_at([1, 64, 1])
            .is_some(),
        "bait_stake must survive the first three attacks"
    );
    assert_eq!(
        app.world()
            .get::<BaitDurability>(anchor)
            .map(|durability| durability.remaining_hits),
        Some(1),
        "bait_stake durability should decrement once per attack intent"
    );

    send_bait_attack(&mut app, attacker, anchor, 14);
    app.update();

    assert!(
        app.world()
            .resource::<ZhenfaRegistry>()
            .find_at([1, 64, 1])
            .is_none(),
        "fourth attack must remove the bait_stake zhenfa instance"
    );
    assert!(
        app.world().get::<DecoyTarget>(anchor).is_none(),
        "broken bait_stake anchor entity must be despawned so blackboard decoy query clears next tick"
    );
    assert!(
        app.world()
            .resource::<Events<VfxEventRequest>>()
            .iter_current_update_events()
            .any(|event| {
                matches!(
                    &event.payload,
                    bong_server::schema::vfx_event::VfxEventPayloadV1::SpawnParticle {
                        event_id,
                        count,
                        duration_ticks,
                        ..
                    } if event_id == gameplay_vfx::DECOY_BREAK
                        && *count == Some(10)
                        && *duration_ticks == Some(18)
                )
            }),
        "bait_stake break must emit decoy_break with the pinned straw scatter burst shape"
    );
}

#[test]
fn trap_runtime_audio_recipes_are_pinned() {
    let beast: serde_json::Value = serde_json::from_str(include_str!(
        "../../../assets/audio/recipes/beast_trap_snap.json"
    ))
    .expect("beast_trap_snap audio recipe must parse");
    assert_eq!(beast["id"], "beast_trap_snap");
    assert_eq!(beast["layers"][0]["sound"], "minecraft:block.chain.break");
    assert_eq!(beast["layers"][1]["sound"], "minecraft:entity.wolf.hurt");
    assert_eq!(beast["attenuation"], "world_3d");
    assert_eq!(beast["bus"], "COMBAT");

    let trip: serde_json::Value = serde_json::from_str(include_str!(
        "../../../assets/audio/recipes/trip_wire_trigger.json"
    ))
    .expect("trip_wire_trigger audio recipe must parse");
    assert_eq!(trip["id"], "trip_wire_trigger");
    assert_eq!(
        trip["layers"][0]["sound"],
        "minecraft:block.tripwire.click_on"
    );
    assert_eq!(trip["layers"][1]["sound"], "minecraft:block.note_block.hat");
    assert_eq!(trip["attenuation"], "world_3d");
    assert_eq!(trip["bus"], "ENVIRONMENT");

    let bait: serde_json::Value = serde_json::from_str(include_str!(
        "../../../assets/audio/recipes/bait_stake_break.json"
    ))
    .expect("bait_stake_break audio recipe must parse");
    assert_eq!(bait["id"], "bait_stake_break");
    assert_eq!(bait["layers"][0]["sound"], "minecraft:block.wood.break");
    assert_eq!(bait["layers"][0]["volume"], 0.8);
    assert_eq!(bait["layers"][0]["pitch"], 0.7);
    assert_eq!(bait["layers"][1]["sound"], "minecraft:block.bamboo.break");
    assert_eq!(bait["layers"][1]["delay_ticks"], 1);
    assert_eq!(bait["attenuation"], "world_3d");
    assert_eq!(bait["bus"], "ENVIRONMENT");
}

#[test]
fn place_blast_rejects_bottom_face_without_consuming_item() {
    let mut app = app_with_loaded_zhenfa();
    let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
    app.world_mut()
        .entity_mut(owner)
        .insert(ordinary_trap_inventory(trap_item(
            9102,
            trap_content::BLAST_TRAP_ITEM_ID,
            "爆阵符",
        )));

    app.world_mut().send_event(ZhenfaPlaceRequest {
        player: owner,
        pos: [1, 64, 1],
        kind: ZhenfaKind::BlastTrap,
        carrier: ZhenfaCarrierKind::CommonStone,
        qi_invest_ratio: 1.0,
        trigger: None,
        item_instance_id: Some(9102),
        target_face: Some(trap_content::TrapTargetFace::Bottom),
        requested_at_tick: 10,
    });
    app.update();

    assert_eq!(app.world().resource::<ZhenfaRegistry>().len(), 0);
    assert_eq!(
        app.world().get::<Cultivation>(owner).unwrap().qi_current,
        100.0
    );
    let inventory = app.world().get::<PlayerInventory>(owner).unwrap();
    assert!(inventory_item_by_instance_borrow(inventory, 9102).is_some());
}

#[test]
fn place_rejects_ordinary_trap_when_qi_is_insufficient() {
    let mut app = app_with_loaded_zhenfa();
    let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
    app.world_mut()
        .entity_mut(owner)
        .insert(ordinary_trap_inventory(trap_item(
            9103,
            trap_content::WARNING_TRAP_ITEM_ID,
            "警示符",
        )));
    app.world_mut()
        .get_mut::<Cultivation>(owner)
        .unwrap()
        .qi_current = 1.0;

    app.world_mut().send_event(ZhenfaPlaceRequest {
        player: owner,
        pos: [1, 64, 1],
        kind: ZhenfaKind::WarningTrap,
        carrier: ZhenfaCarrierKind::CommonStone,
        qi_invest_ratio: 0.0,
        trigger: None,
        item_instance_id: Some(9103),
        target_face: Some(trap_content::TrapTargetFace::Top),
        requested_at_tick: 10,
    });
    app.update();

    assert_eq!(app.world().resource::<ZhenfaRegistry>().len(), 0);
    let inventory = app.world().get::<PlayerInventory>(owner).unwrap();
    assert!(inventory_item_by_instance_borrow(inventory, 9103).is_some());
}

#[test]
fn place_rejected_chunk_density_exceeded() {
    let mut app = app_with_loaded_zhenfa();
    let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
    app.world_mut()
        .entity_mut(owner)
        .insert(ordinary_trap_inventory(trap_item(
            9104,
            trap_content::BLAST_TRAP_ITEM_ID,
            "爆阵符",
        )));
    let anchor_entity = app.world_mut().spawn_empty().id();
    app.world_mut()
        .resource_mut::<ZhenfaRegistry>()
        .insert(ZhenfaInstance {
            id: 0,
            kind: ZhenfaKind::BlastTrap,
            owner,
            owner_player_id: "offline:Alice".to_string(),
            pos: [2, 64, 2],
            carrier: ZhenfaCarrierKind::CommonStone,
            qi_invest_ratio: 0.6,
            qi_invest_amount: 60.0,
            realm_at_cast: Realm::Induce,
            mastery_at_cast: 0.0,
            effect_radius: 2,
            ward_radius: 1,
            placed_at_tick: 1,
            expires_at_tick: 1_000,
            triggered_at: None,
            trigger: None,
            color_main: ColorKind::Intricate,
            color_secondary: None,
            anchor_entity,
        })
        .expect("seed existing trap");

    app.world_mut().send_event(ZhenfaPlaceRequest {
        player: owner,
        pos: [3, 64, 3],
        kind: ZhenfaKind::BlastTrap,
        carrier: ZhenfaCarrierKind::CommonStone,
        qi_invest_ratio: 1.0,
        trigger: None,
        item_instance_id: Some(9104),
        target_face: Some(trap_content::TrapTargetFace::North),
        requested_at_tick: 10,
    });
    app.update();

    assert!(app
        .world()
        .resource::<ZhenfaRegistry>()
        .find_at([3, 64, 3])
        .is_none());
    let inventory = app.world().get::<PlayerInventory>(owner).unwrap();
    assert!(inventory_item_by_instance_borrow(inventory, 9104).is_some());
}

#[test]
fn warning_detects_above_three_blocks_and_keeps_node() {
    let mut app = app_with_loaded_zhenfa();
    let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
    let _intruder = spawn_player(&mut app, "Bob", [0.5, 66.5, 0.5]);
    app.world_mut()
        .entity_mut(owner)
        .insert(ordinary_trap_inventory(trap_item(
            9105,
            trap_content::WARNING_TRAP_ITEM_ID,
            "警示符",
        )));

    app.world_mut().send_event(ZhenfaPlaceRequest {
        player: owner,
        pos: [0, 64, 0],
        kind: ZhenfaKind::WarningTrap,
        carrier: ZhenfaCarrierKind::CommonStone,
        qi_invest_ratio: 0.0,
        trigger: None,
        item_instance_id: Some(9105),
        target_face: Some(trap_content::TrapTargetFace::Top),
        requested_at_tick: 1,
    });
    app.update();
    app.world_mut().resource_mut::<CombatClock>().tick = 2;
    app.update();

    assert!(app
        .world()
        .resource::<ZhenfaRegistry>()
        .find_at([0, 64, 0])
        .is_some());
    let narrations = app
        .world_mut()
        .resource_mut::<PendingGameplayNarrations>()
        .drain();
    assert_eq!(narrations.len(), 1);
    assert_eq!(narrations[0].target.as_deref(), Some("offline:Alice"));
}

#[test]
fn warning_ignores_placer() {
    let mut app = app_with_loaded_zhenfa();
    let owner = spawn_player(&mut app, "Alice", [0.5, 66.5, 0.5]);
    app.world_mut()
        .entity_mut(owner)
        .insert(ordinary_trap_inventory(trap_item(
            9108,
            trap_content::WARNING_TRAP_ITEM_ID,
            "警示符",
        )));

    app.world_mut().send_event(ZhenfaPlaceRequest {
        player: owner,
        pos: [0, 64, 0],
        kind: ZhenfaKind::WarningTrap,
        carrier: ZhenfaCarrierKind::CommonStone,
        qi_invest_ratio: 0.0,
        trigger: None,
        item_instance_id: Some(9108),
        target_face: Some(trap_content::TrapTargetFace::Top),
        requested_at_tick: 1,
    });
    app.update();
    app.world_mut().resource_mut::<CombatClock>().tick = 2;
    app.update();

    assert!(app
        .world()
        .resource::<ZhenfaRegistry>()
        .find_at([0, 64, 0])
        .is_some());
    assert!(app
        .world_mut()
        .resource_mut::<PendingGameplayNarrations>()
        .drain()
        .is_empty());
}

#[test]
fn blast_one_shot_removes_node_and_returns_qi_to_zone() {
    let mut app = app_with_loaded_zhenfa();
    app.insert_resource(ZoneRegistry::fallback());
    app.world_mut()
        .resource_mut::<ZoneRegistry>()
        .find_zone_mut(bong_server::world::zone::DEFAULT_SPAWN_ZONE_NAME)
        .expect("spawn zone should exist")
        .spirit_qi = 0.0;
    let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
    let intruder = spawn_player(&mut app, "Bob", [1.5, 64.0, 1.5]);
    app.world_mut()
        .entity_mut(owner)
        .insert(ordinary_trap_inventory(trap_item(
            9106,
            trap_content::BLAST_TRAP_ITEM_ID,
            "爆阵符",
        )));

    app.world_mut().send_event(ZhenfaPlaceRequest {
        player: owner,
        pos: [1, 64, 1],
        kind: ZhenfaKind::BlastTrap,
        carrier: ZhenfaCarrierKind::CommonStone,
        qi_invest_ratio: 1.0,
        trigger: None,
        item_instance_id: Some(9106),
        target_face: Some(trap_content::TrapTargetFace::North),
        requested_at_tick: 1,
    });
    app.update();
    app.world_mut().resource_mut::<CombatClock>().tick = 2;
    app.update();

    assert!(app
        .world()
        .resource::<ZhenfaRegistry>()
        .find_at([1, 64, 1])
        .is_none());
    let wounds = app.world().get::<Wounds>(intruder).unwrap();
    assert!((wounds.health_current - 82.0).abs() < f32::EPSILON);
    assert!(wounds.entries.iter().any(|wound| wound.location
        == bong_server::body_plan::legacy_body_part_to_id(BodyPart::Chest)
        && wound.kind == WoundKind::Cut));
    let transfers = app.world().resource::<Events<QiTransfer>>();
    let released = transfers.iter_current_update_events().find(|transfer| {
        transfer.to == QiAccountId::zone(bong_server::world::zone::DEFAULT_SPAWN_ZONE_NAME)
    });
    assert!(released.is_some_and(|transfer| (transfer.amount - 30.0).abs() < f64::EPSILON));
}

#[test]
fn blast_requires_los() {
    let (mut app, layer_entity) = app_with_zhenfa_layer();
    app.world_mut()
        .get_mut::<ChunkLayer>(layer_entity)
        .expect("test layer should exist")
        .set_block(block_pos_from_array([2, 64, 1]), BlockState::STONE);
    let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
    let intruder = spawn_player(&mut app, "Bob", [2.9, 64.0, 1.5]);
    app.world_mut()
        .entity_mut(owner)
        .insert(ordinary_trap_inventory(trap_item(
            9109,
            trap_content::BLAST_TRAP_ITEM_ID,
            "爆阵符",
        )));

    app.world_mut().send_event(ZhenfaPlaceRequest {
        player: owner,
        pos: [1, 64, 1],
        kind: ZhenfaKind::BlastTrap,
        carrier: ZhenfaCarrierKind::CommonStone,
        qi_invest_ratio: 1.0,
        trigger: None,
        item_instance_id: Some(9109),
        target_face: Some(trap_content::TrapTargetFace::North),
        requested_at_tick: 1,
    });
    app.update();
    app.world_mut().resource_mut::<CombatClock>().tick = 2;
    app.update();

    assert!(app
        .world()
        .resource::<ZhenfaRegistry>()
        .find_at([1, 64, 1])
        .is_some());
    let wounds = app.world().get::<Wounds>(intruder).unwrap();
    assert_eq!(wounds.health_current, wounds.health_max);
    assert!(wounds.entries.is_empty());
}

#[test]
fn blast_damage_resolution_keeps_los_filter_per_target() {
    let (mut app, layer_entity) = app_with_zhenfa_layer();
    app.world_mut()
        .get_mut::<ChunkLayer>(layer_entity)
        .expect("test layer should exist")
        .set_block(block_pos_from_array([2, 64, 1]), BlockState::STONE);
    let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
    let visible_intruder = spawn_player(&mut app, "Bob", [1.5, 64.0, 1.5]);
    let blocked_intruder = spawn_player(&mut app, "Chen", [2.9, 64.0, 1.5]);
    app.world_mut()
        .entity_mut(owner)
        .insert(ordinary_trap_inventory(trap_item(
            9110,
            trap_content::BLAST_TRAP_ITEM_ID,
            "爆阵符",
        )));

    app.world_mut().send_event(ZhenfaPlaceRequest {
        player: owner,
        pos: [1, 64, 1],
        kind: ZhenfaKind::BlastTrap,
        carrier: ZhenfaCarrierKind::CommonStone,
        qi_invest_ratio: 1.0,
        trigger: None,
        item_instance_id: Some(9110),
        target_face: Some(trap_content::TrapTargetFace::North),
        requested_at_tick: 1,
    });
    app.update();
    app.world_mut().resource_mut::<CombatClock>().tick = 2;
    app.update();

    let visible_wounds = app.world().get::<Wounds>(visible_intruder).unwrap();
    assert!(
        visible_wounds.health_current < visible_wounds.health_max,
        "expected visible intruder to take blast damage because LOS is clear; actual health={}",
        visible_wounds.health_current
    );
    let blocked_wounds = app.world().get::<Wounds>(blocked_intruder).unwrap();
    assert_eq!(
        blocked_wounds.health_current, blocked_wounds.health_max,
        "expected blocked intruder to avoid blast damage because wall blocks LOS; actual health={}",
        blocked_wounds.health_current
    );
    assert!(
        blocked_wounds.entries.is_empty(),
        "expected blocked intruder to receive no wound entries because LOS is blocked; actual={:?}",
        blocked_wounds.entries
    );
}

#[test]
fn slow_three_charges_then_remove() {
    let mut app = app_with_loaded_zhenfa();
    let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
    let intruder = spawn_player(&mut app, "Bob", [2.5, 64.0, 2.5]);
    app.world_mut()
        .entity_mut(owner)
        .insert(ordinary_trap_inventory(trap_item(
            9107,
            trap_content::SLOW_TRAP_ITEM_ID,
            "缓阵符",
        )));

    app.world_mut().send_event(ZhenfaPlaceRequest {
        player: owner,
        pos: [2, 64, 2],
        kind: ZhenfaKind::SlowTrap,
        carrier: ZhenfaCarrierKind::CommonStone,
        qi_invest_ratio: 0.0,
        trigger: None,
        item_instance_id: Some(9107),
        target_face: Some(trap_content::TrapTargetFace::Top),
        requested_at_tick: 1,
    });
    app.update();

    for (idx, tick) in [2_u64, 4, 6].into_iter().enumerate() {
        app.world_mut().resource_mut::<CombatClock>().tick = tick;
        app.world_mut()
            .entity_mut(intruder)
            .insert(Position::new([2.5, 64.0, 2.5]));
        app.update();
        if idx == 2 {
            break;
        }
        app.world_mut().resource_mut::<CombatClock>().tick = tick + 1;
        app.world_mut()
            .entity_mut(intruder)
            .insert(Position::new([20.0, 64.0, 20.0]));
        app.update();
    }

    assert!(app
        .world()
        .resource::<ZhenfaRegistry>()
        .find_at([2, 64, 2])
        .is_none());
    let status_events = app.world().resource::<Events<ApplyStatusEffectIntent>>();
    assert!(status_events
        .iter_current_update_events()
        .any(|event| event.kind == StatusEffectKind::QiRegenPaused));
}

#[test]
fn decay_removes_expired_array_eye() {
    let (mut app, layer_entity) = app_with_zhenfa_layer();
    app.insert_resource(ZoneRegistry::fallback());
    let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
    let pos = [3, 64, 3];
    app.world_mut().send_event(ZhenfaPlaceRequest {
        player: owner,
        pos,
        kind: ZhenfaKind::Trap,
        carrier: ZhenfaCarrierKind::CommonStone,
        qi_invest_ratio: 0.10,
        trigger: None,
        item_instance_id: None,
        target_face: None,
        requested_at_tick: 0,
    });
    app.update();

    let anchor_entity = app
        .world()
        .resource::<ZhenfaRegistry>()
        .find_at(pos)
        .unwrap()
        .anchor_entity;
    assert_eq!(
        layer_block_state(&app, layer_entity, pos),
        Some(zhenfa_eye_state(false))
    );
    app.world_mut().resource_mut::<CombatClock>().tick =
        carrier_spec(ZhenfaCarrierKind::CommonStone).duration_ticks + 1;
    app.update();

    assert!(app
        .world()
        .resource::<ZhenfaRegistry>()
        .find_at(pos)
        .is_none());
    assert!(app.world().get_entity(anchor_entity).is_none());
    assert_eq!(
        layer_block_state(&app, layer_entity, pos),
        Some(BlockState::AIR)
    );
    // QS-02 修复后：Trap（qi_invest_ratio=0.10）部署时封入的真元，在过期衰减时归还 zone（守恒）。
    // 旧断言「衰减无 QiTransfer」锁的是 bug 行为——Trap 部署扣了真元却不归还、过期蒸发。
    let transfers = app.world().resource::<Events<QiTransfer>>();
    let released = transfers
        .iter_current_update_events()
        .filter(|t| t.reason == QiTransferReason::ReleaseToZone)
        .count();
    assert!(
        released >= 1,
        "expected Trap 衰减把封存真元归还 zone（QS-02 守恒修复），实际无 ReleaseToZone transfer"
    );
}

#[test]
fn passive_trap_trigger_damages_legs_and_frees_array_eye() {
    let (mut app, layer_entity) = app_with_zhenfa_layer();
    let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
    let intruder = spawn_player(&mut app, "Bob", [5.5, 64.0, 5.5]);
    let pos = [5, 64, 5];
    app.world_mut().send_event(ZhenfaPlaceRequest {
        player: owner,
        pos,
        kind: ZhenfaKind::Trap,
        carrier: ZhenfaCarrierKind::LingqiBlock,
        qi_invest_ratio: 0.20,
        trigger: None,
        item_instance_id: None,
        target_face: None,
        requested_at_tick: 10,
    });
    app.update();

    let id = app
        .world()
        .resource::<ZhenfaRegistry>()
        .find_at(pos)
        .unwrap()
        .id;
    let anchor_entity = app
        .world()
        .resource::<ZhenfaRegistry>()
        .find_at(pos)
        .unwrap()
        .anchor_entity;
    assert_eq!(
        layer_block_state(&app, layer_entity, pos),
        Some(zhenfa_eye_state(false))
    );
    app.world_mut().resource_mut::<CombatClock>().tick = 11;
    app.update();

    let registry = app.world().resource::<ZhenfaRegistry>();
    assert!(registry.get(id).is_none());
    assert!(registry.find_at(pos).is_none());
    assert!(app.world().get_entity(anchor_entity).is_none());
    assert_eq!(
        layer_block_state(&app, layer_entity, pos),
        Some(BlockState::AIR)
    );
    let wounds = app.world().get::<Wounds>(intruder).unwrap();
    assert_eq!(
        wounds
            .entries
            .iter()
            .filter(|w| {
                w.location == bong_server::body_plan::legacy_body_part_to_id(BodyPart::LegL)
                    || w.location == bong_server::body_plan::legacy_body_part_to_id(BodyPart::LegR)
            })
            .count(),
        2
    );
    assert!(wounds.health_current < wounds.health_max);
    assert!(!app.world().resource::<Events<CombatEvent>>().is_empty());
    assert_eq!(
        app.world()
            .get::<PracticeLog>(owner)
            .unwrap()
            .weights
            .get(&ColorKind::Intricate)
            .copied(),
        Some(bong_server::cultivation::color::STYLE_PRACTICE_AMOUNT)
    );
}

#[test]
fn chain_trigger_waits_six_ticks_and_does_not_loop() {
    let mut app = app_with_loaded_zhenfa();
    let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
    let _intruder = spawn_player(&mut app, "Bob", [5.5, 64.0, 5.5]);
    for (idx, pos) in [[5, 64, 5], [6, 64, 5], [7, 64, 5]].into_iter().enumerate() {
        app.world_mut().send_event(ZhenfaPlaceRequest {
            player: owner,
            pos,
            kind: ZhenfaKind::Trap,
            carrier: ZhenfaCarrierKind::LingqiBlock,
            qi_invest_ratio: 0.10,
            trigger: None,
            item_instance_id: None,
            target_face: None,
            requested_at_tick: idx as u64,
        });
    }
    app.update();

    app.world_mut().resource_mut::<CombatClock>().tick = 10;
    app.update();
    assert_eq!(app.world().resource::<ZhenfaRegistry>().len(), 2);

    app.world_mut().resource_mut::<CombatClock>().tick = 16;
    app.update();
    assert_eq!(app.world().resource::<ZhenfaRegistry>().len(), 1);

    app.world_mut().resource_mut::<CombatClock>().tick = 22;
    app.update();
    assert_eq!(app.world().resource::<ZhenfaRegistry>().len(), 0);
}

#[test]
fn active_trigger_picks_nearest_owned_untriggered_trap() {
    let (mut app, layer_entity) = app_with_zhenfa_layer();
    let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
    for (tick, pos) in [(1, [10, 64, 0]), (2, [3, 64, 0])] {
        app.world_mut().send_event(ZhenfaPlaceRequest {
            player: owner,
            pos,
            kind: ZhenfaKind::Trap,
            carrier: ZhenfaCarrierKind::LingqiBlock,
            qi_invest_ratio: 0.10,
            trigger: None,
            item_instance_id: None,
            target_face: None,
            requested_at_tick: tick,
        });
    }
    app.update();
    app.world_mut().send_event(ZhenfaTriggerRequest {
        player: owner,
        instance_id: None,
        requested_at_tick: 20,
    });
    app.update();

    let registry = app.world().resource::<ZhenfaRegistry>();
    assert!(registry.find_at([3, 64, 0]).is_none());
    assert!(registry.find_at([10, 64, 0]).is_some());
    assert_eq!(
        layer_block_state(&app, layer_entity, [3, 64, 0]),
        Some(BlockState::AIR)
    );
    assert_eq!(
        layer_block_state(&app, layer_entity, [10, 64, 0]),
        Some(zhenfa_eye_state(false))
    );
}

#[test]
fn force_break_applies_backlash_and_removes_eye() {
    let mut app = app_with_loaded_zhenfa();
    let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
    let breaker = spawn_player(&mut app, "Bob", [1.5, 64.0, 1.5]);
    app.world_mut().send_event(ZhenfaPlaceRequest {
        player: owner,
        pos: [1, 64, 1],
        kind: ZhenfaKind::Trap,
        carrier: ZhenfaCarrierKind::LingqiBlock,
        qi_invest_ratio: 0.10,
        trigger: None,
        item_instance_id: None,
        target_face: None,
        requested_at_tick: 1,
    });
    app.update();

    let anchor_entity = app
        .world()
        .resource::<ZhenfaRegistry>()
        .find_at([1, 64, 1])
        .unwrap()
        .anchor_entity;
    app.world_mut().send_event(ZhenfaDisarmRequest {
        player: breaker,
        pos: [1, 64, 1],
        mode: ZhenfaDisarmMode::ForceBreak,
        requested_at_tick: 2,
    });
    app.update();

    assert!(app
        .world()
        .resource::<ZhenfaRegistry>()
        .find_at([1, 64, 1])
        .is_none());
    assert!(app.world().get_entity(anchor_entity).is_none());
    assert!(!app
        .world()
        .get::<Wounds>(breaker)
        .unwrap()
        .entries
        .is_empty());
    assert_eq!(
        app.world()
            .get::<Contamination>(breaker)
            .unwrap()
            .entries
            .first()
            .unwrap()
            .amount,
        0.5
    );
}

#[test]
fn shrine_ward_deploy_emits_event_and_burns_intruder() {
    let mut app = app_with_loaded_zhenfa();
    let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
    let intruder = spawn_player(&mut app, "Bob", [4.5, 64.0, 0.5]);
    app.world_mut().send_event(ZhenfaPlaceRequest {
        player: owner,
        pos: [0, 64, 0],
        kind: ZhenfaKind::ShrineWard,
        carrier: ZhenfaCarrierKind::LingqiBlock,
        qi_invest_ratio: 0.20,
        trigger: None,
        item_instance_id: None,
        target_face: None,
        requested_at_tick: 1,
    });
    app.update();

    assert!(!app
        .world()
        .resource::<Events<WardArrayDeployEvent>>()
        .is_empty());
    app.world_mut().resource_mut::<CombatClock>().tick = 2;
    app.update();

    let wounds = app.world().get::<Wounds>(intruder).unwrap();
    assert!(wounds.health_current < wounds.health_max);
    assert!(wounds
        .entries
        .iter()
        .any(|w| w.inflicted_by.as_deref() == Some("zhenfa_shrine_ward:1")));
}

#[test]
fn shrine_ward_lethal_pressure_emits_death_event() {
    let mut app = app_with_loaded_zhenfa();
    let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
    let intruder = spawn_player(&mut app, "Bob", [4.5, 64.0, 0.5]);
    app.world_mut()
        .get_mut::<Wounds>(intruder)
        .unwrap()
        .health_current = 4.0;
    app.world_mut().send_event(ZhenfaPlaceRequest {
        player: owner,
        pos: [0, 64, 0],
        kind: ZhenfaKind::ShrineWard,
        carrier: ZhenfaCarrierKind::LingqiBlock,
        qi_invest_ratio: 0.20,
        trigger: None,
        item_instance_id: None,
        target_face: None,
        requested_at_tick: 1,
    });
    app.update();

    app.world_mut().resource_mut::<CombatClock>().tick = 2;
    app.update();

    let deaths: Vec<_> = app
        .world()
        .resource::<Events<DeathEvent>>()
        .get_reader()
        .read(app.world().resource::<Events<DeathEvent>>())
        .cloned()
        .collect();
    assert_eq!(deaths.len(), 1);
    assert_eq!(deaths[0].target, intruder);
    assert_eq!(deaths[0].attacker, Some(owner));
    assert_eq!(
        deaths[0].attacker_player_id.as_deref(),
        Some("offline:Alice")
    );
    assert_eq!(deaths[0].cause, "zhenfa_shrine_ward:offline:Bob");
}

#[test]
fn shrine_ward_allows_trusted_allies() {
    let mut app = app_with_loaded_zhenfa();
    let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
    let ally = spawn_player(&mut app, "Bob", [4.5, 64.0, 0.5]);
    app.world_mut().entity_mut(ally).insert((
        Lifecycle {
            character_id: "offline:Bob".to_string(),
            ..Default::default()
        },
        Relationships {
            edges: vec![bong_server::social::components::Relationship {
                kind: RelationshipKindV1::Pact,
                peer: canonical_player_id("Alice"),
                since_tick: 0,
                metadata: serde_json::Value::Null,
            }],
        },
        Renown {
            fame: 80,
            ..Default::default()
        },
    ));

    app.world_mut().send_event(ZhenfaPlaceRequest {
        player: owner,
        pos: [0, 64, 0],
        kind: ZhenfaKind::ShrineWard,
        carrier: ZhenfaCarrierKind::LingqiBlock,
        qi_invest_ratio: 0.20,
        trigger: None,
        item_instance_id: None,
        target_face: None,
        requested_at_tick: 1,
    });
    app.update();
    app.world_mut().resource_mut::<CombatClock>().tick = 2;
    app.update();

    let wounds = app.world().get::<Wounds>(ally).unwrap();
    assert_eq!(wounds.health_current, wounds.health_max);
    assert!(wounds.entries.is_empty());
}

#[test]
fn deceive_heaven_requires_solidify_or_higher() {
    let mut app = app_with_zhenfa();
    let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
    app.world_mut()
        .entity_mut(owner)
        .insert(deceive_heaven_material_inventory());
    app.world_mut().send_event(ZhenfaPlaceRequest {
        player: owner,
        pos: [0, 64, 0],
        kind: ZhenfaKind::DeceiveHeaven,
        carrier: ZhenfaCarrierKind::BeastCoreInlaid,
        qi_invest_ratio: 0.90,
        trigger: None,
        item_instance_id: None,
        target_face: None,
        requested_at_tick: 1,
    });
    app.update();

    assert_eq!(app.world().resource::<ZhenfaRegistry>().len(), 0);
}

#[test]
fn severed_kidney_blocks_lingju_array() {
    let mut app = app_with_zhenfa();
    let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
    let mut severed = MeridianSeveredPermanent::default();
    severed.insert(
        MeridianId::Kidney,
        bong_server::cultivation::meridian::severed::SeveredSource::CombatWound,
        1,
    );
    app.world_mut().entity_mut(owner).insert(severed);

    app.world_mut().send_event(ZhenfaPlaceRequest {
        player: owner,
        pos: [0, 64, 0],
        kind: ZhenfaKind::Lingju,
        carrier: ZhenfaCarrierKind::BeastCoreInlaid,
        qi_invest_ratio: 0.30,
        trigger: None,
        item_instance_id: None,
        target_face: None,
        requested_at_tick: 1,
    });
    app.update();

    assert_eq!(app.world().resource::<ZhenfaRegistry>().len(), 0);
}

#[test]
fn array_mastery_grows_on_cast_and_trigger() {
    let mut app = app_with_loaded_zhenfa();
    let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
    app.world_mut()
        .entity_mut(owner)
        .insert(ArrayMastery::default());
    app.world_mut().send_event(ZhenfaPlaceRequest {
        player: owner,
        pos: [1, 64, 1],
        kind: ZhenfaKind::Trap,
        carrier: ZhenfaCarrierKind::LingqiBlock,
        qi_invest_ratio: 0.10,
        trigger: None,
        item_instance_id: None,
        target_face: None,
        requested_at_tick: 1,
    });
    app.update();
    assert_eq!(app.world().get::<ArrayMastery>(owner).unwrap().trap, 0.3);

    app.world_mut().send_event(ZhenfaTriggerRequest {
        player: owner,
        instance_id: None,
        requested_at_tick: 2,
    });
    app.update();
    assert_eq!(app.world().get::<ArrayMastery>(owner).unwrap().trap, 1.3);
}

#[test]
fn zhenfa_instance_exposes_style_attack_and_defense() {
    let instance = ZhenfaInstance {
        id: 1,
        kind: ZhenfaKind::Ward,
        owner: Entity::from_raw(1),
        owner_player_id: "offline:Azure".to_string(),
        pos: [1, 64, 1],
        carrier: ZhenfaCarrierKind::LingqiBlock,
        qi_invest_ratio: 0.5,
        qi_invest_amount: 25.0,
        realm_at_cast: Realm::Induce,
        mastery_at_cast: 0.0,
        effect_radius: 2,
        ward_radius: 8,
        placed_at_tick: 1,
        expires_at_tick: 100,
        triggered_at: None,
        trigger: None,
        color_main: ColorKind::Intricate,
        color_secondary: Some(ColorKind::Solid),
        anchor_entity: Entity::from_raw(2),
    };

    assert_eq!(instance.style_color(), ColorKind::Intricate);
    assert_eq!(instance.injected_qi(), 25.0);
    assert_eq!(instance.rejection_rate(), 0.35);
    assert_eq!(instance.medium().carrier, CarrierGrade::SpiritWeapon);
    assert_eq!(instance.defense_color(), ColorKind::Solid);
    assert_eq!(instance.resistance(), 0.5);
}
