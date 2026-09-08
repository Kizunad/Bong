use super::*;
use crate::inventory::{
    InventoryRevision, ItemInstance, ItemRarity, ItemTemplate, PlayerInventory,
    MAIN_PACK_CONTAINER_ID,
};
use crate::npc::spawn::NpcMarker;
use crate::world::dimension::{DimensionKind, DimensionLayers, OverworldLayer, TsyLayer};
use crate::world::dimension_transfer::{
    apply_dimension_transfers, DimensionTransferRequest, DimensionTransferSet,
};
use std::collections::HashMap;
use valence::prelude::{
    App, BlockPos, DVec3, EntityLayerId, IntoSystemConfigs, Update, VisibleChunkLayer,
    VisibleEntityLayers,
};

use super::super::events::{
    DrainQiCompleted, DyeContaminationWarning, HarvestCompleted, PlantingCompleted, RenewCompleted,
    ReplenishCompleted, StartDrainQiRequest, StartHarvestRequest, StartPlantingRequest,
    StartRenewRequest, StartReplenishRequest, StartTillRequest, TillCompleted,
};
use super::super::session::{
    ReplenishSource, SessionMode, DRAIN_QI_TICKS, HARVEST_MANUAL_TICKS, PLANTING_TICKS,
    RENEW_TICKS, REPLENISH_COOLDOWN_LINGTIAN_TICKS, TILL_MANUAL_TICKS,
};
use super::super::terrain::TerrainKind;
use crate::skill::events::XpGainSource;

fn make_hoe_instance(kind: HoeKind, durability: f64) -> ItemInstance {
    ItemInstance {
        instance_id: 1,
        template_id: kind.item_id().to_string(),
        display_name: kind.item_id().to_string(),
        grid_w: 1,
        grid_h: 2,
        weight: 1.5,
        rarity: ItemRarity::Common,
        description: String::new(),
        stack_count: 1,
        spirit_quality: 1.0,
        durability,
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

fn make_inventory_with_hoe(kind: HoeKind, durability: f64) -> PlayerInventory {
    let mut equipped = HashMap::new();
    equipped.insert(
        MAIN_HAND_SLOT.to_string(),
        crate::inventory::SlotContents::held_single(make_hoe_instance(kind, durability)),
    );
    PlayerInventory {
        triggered_treasures: Vec::new(),
        revision: InventoryRevision(0),
        containers: vec![],
        equipped,
        hotbar: Default::default(),
        bone_coins: 0,
        max_weight: 45.0,
    }
}

fn spawn_test_player<T: bevy_ecs::bundle::Bundle>(app: &mut App, components: T) -> Entity {
    let (client_bundle, _helper) = valence::testing::create_mock_client("LingtianTest");
    let player = app.world_mut().spawn(client_bundle).id();
    app.world_mut().entity_mut(player).insert((
        components,
        Position(DVec3::new(0.5, 64.5, 0.5)),
        CurrentDimension(crate::world::dimension::DimensionKind::Overworld),
    ));
    player
}

fn set_test_player_position(app: &mut App, player: Entity, target: BlockPos) {
    app.world_mut()
        .entity_mut(player)
        .insert(Position(DVec3::new(
            f64::from(target.x) + 0.5,
            f64::from(target.y) + 0.5,
            f64::from(target.z) + 0.5,
        )));
}

fn valid_test_player<T: bevy_ecs::bundle::Bundle>(
    app: &mut App,
    components: T,
    target: BlockPos,
) -> Entity {
    let player = spawn_test_player(app, components);
    set_test_player_position(app, player, target);
    player
}

fn build_app() -> App {
    let mut app = App::new();
    app.insert_resource(ActiveLingtianSessions::new())
        .insert_resource(SeedRegistry::new())
        .insert_resource(PlantKindRegistry::new())
        .insert_resource(ItemRegistry::default())
        .insert_resource(InventoryInstanceIdAllocator::default())
        .insert_resource(LingtianHarvestRng::default())
        .insert_resource(ZoneQiAccount::new())
        .insert_resource(LingtianClock::default())
        .insert_resource(CombatClock::default())
        .insert_resource(ActiveEventsResource::default())
        // fix-spec-1901-v2 §4.1 — C2S 请求持久队列（测试 app 也要 init）。
        .init_resource::<crate::lingtian::requests::PendingLingtianRequests>()
        .add_event::<StartTillRequest>()
        .add_event::<TillCompleted>()
        .add_event::<StartRenewRequest>()
        .add_event::<RenewCompleted>()
        .add_event::<StartPlantingRequest>()
        .add_event::<PlantingCompleted>()
        .add_event::<StartHarvestRequest>()
        .add_event::<HarvestCompleted>()
        .add_event::<StartReplenishRequest>()
        .add_event::<ReplenishCompleted>()
        .add_event::<DyeContaminationWarning>()
        .add_event::<StartDrainQiRequest>()
        .add_event::<DrainQiCompleted>()
        .add_event::<QiTransfer>()
        .add_event::<SkillXpGain>()
        .add_systems(
            Update,
            (
                validate_and_dispatch_lingtian_requests,
                handle_start_till,
                handle_start_renew,
                handle_start_harvest,
                handle_start_replenish,
                handle_start_drain_qi,
                tick_lingtian_sessions,
                apply_completed_sessions,
                record_dye_contamination_warning_recent_events,
            )
                .chain()
                .after(crate::world::movement_commit::AuthoritativePositionCommitSet),
        );
    app
}

#[test]
fn start_plot_index_preserves_all_plots_at_duplicate_position() {
    let pos = BlockPos::new(7, 64, -3);
    let mut first = LingtianPlot::new(pos, None);
    first.plot_qi = 0.25;
    let mut second = LingtianPlot::new(pos, None);
    second.plot_qi = 0.75;
    let plots = [first, second];

    let index = build_start_plot_index(plots.iter(), None);
    let candidates = index
        .get(&pos)
        .expect("duplicate-position fixture must be indexed");
    assert_eq!(candidates.len(), 2, "同一位置的 plot 候选不能在索引中丢失");
    let selected = candidates
        .first()
        .copied()
        .expect("duplicate-position fixture must preserve the first plot");

    assert!(
        std::ptr::eq(selected, &plots[0]),
        "duplicate BlockPos must preserve the first plot, matching the previous find semantics"
    );
    assert_eq!(selected.plot_qi, 0.25);
}

#[test]
fn renew_accepts_later_barren_duplicate_plot() {
    let mut app = build_app();
    let pos = BlockPos::new(8, 64, -3);
    let player = valid_test_player(
        &mut app,
        make_inventory_with_hoe(HoeKind::Xuantie, 1.0),
        pos,
    );
    let mut first_plot = LingtianPlot::new(pos, Some(player));
    first_plot.harvest_count = super::super::plot::N_RENEW - 1;
    let first_plot_entity = app.world_mut().spawn(first_plot).id();
    let mut barren = LingtianPlot::new(pos, Some(player));
    barren.harvest_count = super::super::plot::N_RENEW;
    let barren_plot_entity = app.world_mut().spawn(barren).id();

    app.world_mut().send_event(StartRenewRequest {
        player,
        pos,
        hoe_instance_id: 1,
    });
    for _ in 0..RENEW_TICKS {
        app.update();
    }

    assert_eq!(
        app.world().resource::<ActiveLingtianSessions>().len(),
        0,
        "Renew 结算后 session 应清理"
    );
    assert_eq!(
        app.world()
            .get::<LingtianPlot>(first_plot_entity)
            .unwrap()
            .harvest_count,
        super::super::plot::N_RENEW - 1,
        "Renew 不应修改同位置首个非贫瘠 plot"
    );
    assert_eq!(
        app.world()
            .get::<LingtianPlot>(barren_plot_entity)
            .unwrap()
            .harvest_count,
        0,
        "Renew 应修改准入时命中的贫瘠 plot"
    );
}

#[test]
fn renew_does_not_fallback_to_other_plot_when_bound_target_changes() {
    let mut app = build_app();
    let pos = BlockPos::new(8, 64, -4);
    let player = valid_test_player(
        &mut app,
        make_inventory_with_hoe(HoeKind::Xuantie, 1.0),
        pos,
    );
    let mut target = LingtianPlot::new(pos, Some(player));
    target.harvest_count = super::super::plot::N_RENEW;
    let target_entity = app.world_mut().spawn(target).id();

    app.world_mut().send_event(StartRenewRequest {
        player,
        pos,
        hoe_instance_id: 1,
    });
    app.update();
    app.world_mut()
        .get_mut::<LingtianPlot>(target_entity)
        .unwrap()
        .harvest_count = super::super::plot::N_RENEW - 1;
    let fallback_entity = app
        .world_mut()
        .spawn({
            let mut plot = LingtianPlot::new(pos, Some(player));
            plot.harvest_count = super::super::plot::N_RENEW;
            plot
        })
        .id();

    for _ in 0..RENEW_TICKS - 1 {
        app.update();
    }

    assert_eq!(
        app.world()
            .get::<LingtianPlot>(target_entity)
            .unwrap()
            .harvest_count,
        super::super::plot::N_RENEW - 1,
        "准入实体状态变化后 Renew 不应继续翻新该实体"
    );
    assert_eq!(
        app.world()
            .get::<LingtianPlot>(fallback_entity)
            .unwrap()
            .harvest_count,
        super::super::plot::N_RENEW,
        "Renew 不应回退到同位置的其他 plot"
    );
    let durability = app.world().get::<PlayerInventory>(player).unwrap().equipped[MAIN_HAND_SLOT]
        .held
        .as_ref()
        .unwrap()
        .durability;
    assert_eq!(durability, 1.0, "目标失效时不应扣锄耐久");
}

#[test]
fn planting_accepts_later_empty_duplicate_plot() {
    let mut app = build_planting_app();
    let pos = BlockPos::new(9, 64, -3);
    let player = valid_test_player(
        &mut app,
        make_inventory_with_seed("ci_she_hao_seed", 2),
        pos,
    );
    let mut blocked = LingtianPlot::new(pos, Some(player));
    blocked.crop = Some(CropInstance::new("ning_mai_cao".into()));
    let blocked_plot_entity = app.world_mut().spawn(blocked).id();
    let empty_plot_entity = app
        .world_mut()
        .spawn(LingtianPlot::new(pos, Some(player)))
        .id();

    app.world_mut().send_event(StartPlantingRequest {
        player,
        pos,
        plant_id: "ci_she_hao".into(),
    });
    for _ in 0..PLANTING_TICKS {
        app.update();
    }

    assert_eq!(
        app.world().resource::<ActiveLingtianSessions>().len(),
        0,
        "Planting 结算后 session 应清理"
    );
    assert_eq!(
        app.world()
            .get::<LingtianPlot>(blocked_plot_entity)
            .unwrap()
            .crop
            .as_ref()
            .map(|crop| crop.kind.as_str()),
        Some("ning_mai_cao"),
        "Planting 不应修改同位置首个已占用 plot"
    );
    assert_eq!(
        app.world()
            .get::<LingtianPlot>(empty_plot_entity)
            .unwrap()
            .crop
            .as_ref()
            .map(|crop| crop.kind.as_str()),
        Some("ci_she_hao"),
        "Planting 应修改准入时命中的空 plot"
    );
}

#[test]
fn drain_qi_accepts_later_nonzero_duplicate_plot() {
    let mut app = build_app();
    let pos = BlockPos::new(10, 64, -3);
    let player = valid_test_player(
        &mut app,
        (empty_inventory_8x8(), LifeRecord::new("duplicate-drain")),
        pos,
    );
    let empty_plot_entity = app.world_mut().spawn(LingtianPlot::new(pos, None)).id();
    let mut charged = LingtianPlot::new(pos, None);
    charged.plot_qi = 0.5;
    let charged_plot_entity = app.world_mut().spawn(charged).id();

    app.world_mut()
        .send_event(StartDrainQiRequest { player, pos });
    for _ in 0..DRAIN_QI_TICKS {
        app.update();
    }

    assert_eq!(
        app.world().resource::<ActiveLingtianSessions>().len(),
        0,
        "DrainQi 结算后 session 应清理"
    );
    assert_eq!(
        app.world()
            .get::<LingtianPlot>(empty_plot_entity)
            .unwrap()
            .plot_qi,
        0.0,
        "DrainQi 不应修改同位置首个空 plot"
    );
    assert_eq!(
        app.world()
            .get::<LingtianPlot>(charged_plot_entity)
            .unwrap()
            .plot_qi,
        0.0,
        "DrainQi 应修改准入时命中的有真元 plot"
    );
}

#[test]
fn till_e2e_spawns_plot_and_decrements_durability() {
    let mut app = build_app();
    let pos = BlockPos::new(10, 64, 10);
    let player = spawn_test_player(&mut app, make_inventory_with_hoe(HoeKind::Iron, 1.0));
    set_test_player_position(&mut app, player, pos);
    app.world_mut().send_event(StartTillRequest {
        player,
        pos,
        hoe_instance_id: 1,
        mode: SessionMode::Manual,
        terrain: TerrainKind::Grass,
        environment: PlotEnvironment::base(),
    });

    // 第 1 次 update：handle_start_till 起 session + tick_lingtian_sessions 推 1
    app.update();
    assert_eq!(app.world().resource::<ActiveLingtianSessions>().len(), 1);

    // 再 TILL_MANUAL_TICKS - 1 次 update（共 TILL_MANUAL_TICKS tick 满）
    for _ in 0..TILL_MANUAL_TICKS - 1 {
        app.update();
    }

    // session 应当 finished + plot spawn 完成 + 锄扣 1 次（durability -= 0.05）
    assert!(
        app.world().resource::<ActiveLingtianSessions>().is_empty(),
        "session 完成后应清出表"
    );
    let plots: Vec<_> = app
        .world_mut()
        .query::<&LingtianPlot>()
        .iter(app.world())
        .collect();
    assert_eq!(plots.len(), 1);
    assert_eq!(plots[0].pos, pos);
    let inv = app.world().get::<PlayerInventory>(player).unwrap();
    let dur = inv
        .equipped
        .get(MAIN_HAND_SLOT)
        .unwrap()
        .held
        .as_ref()
        .unwrap()
        .durability;
    assert!((dur - 0.95).abs() < 1e-9, "Iron 锄一次扣 0.05；实得 {dur}");
}

#[test]
fn till_emits_vfx() {
    let mut app = build_app();
    app.add_event::<VfxEventRequest>();
    let pos = BlockPos::new(10, 64, 10);
    let player = spawn_test_player(&mut app, make_inventory_with_hoe(HoeKind::Iron, 1.0));
    set_test_player_position(&mut app, player, pos);
    app.world_mut().send_event(StartTillRequest {
        player,
        pos,
        hoe_instance_id: 1,
        mode: SessionMode::Manual,
        terrain: TerrainKind::Grass,
        environment: PlotEnvironment::base(),
    });
    for _ in 0..TILL_MANUAL_TICKS {
        app.update();
    }

    let events = app.world().resource::<Events<VfxEventRequest>>();
    let emitted = events
        .iter_current_update_events()
        .next()
        .expect("finished till session should emit vfx");
    match &emitted.payload {
        crate::schema::vfx_event::VfxEventPayloadV1::SpawnParticle { event_id, .. } => {
            assert_eq!(event_id, gameplay_vfx::LINGTIAN_TILL);
        }
        other => panic!("expected SpawnParticle, got {other:?}"),
    }
}

#[test]
fn till_rejected_when_not_holding_hoe() {
    let mut app = build_app();
    // 玩家手里啥都没有
    let player = spawn_test_player(
        &mut app,
        PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(0),
            containers: vec![],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 45.0,
        },
    );
    app.world_mut().send_event(StartTillRequest {
        player,
        pos: BlockPos::new(0, 64, 0),
        hoe_instance_id: 1,
        mode: SessionMode::Manual,
        terrain: TerrainKind::Grass,
        environment: PlotEnvironment::base(),
    });
    app.update();
    assert!(app.world().resource::<ActiveLingtianSessions>().is_empty());
}

#[test]
fn till_rejected_on_blocked_terrain() {
    let mut app = build_app();
    let player = spawn_test_player(&mut app, make_inventory_with_hoe(HoeKind::Iron, 1.0));
    app.world_mut().send_event(StartTillRequest {
        player,
        pos: BlockPos::new(0, 64, 0),
        hoe_instance_id: 1,
        mode: SessionMode::Manual,
        terrain: TerrainKind::Stone,
        environment: PlotEnvironment::base(),
    });
    app.update();
    assert!(app.world().resource::<ActiveLingtianSessions>().is_empty());
}

#[test]
fn equipped_main_hand_hoe_returns_kind_and_instance_id() {
    let inv = make_inventory_with_hoe(HoeKind::Lingtie, 0.5);
    let (kind, id) = equipped_main_hand_hoe(&inv).expect("should resolve");
    assert_eq!(kind, HoeKind::Lingtie);
    assert_eq!(id, 1, "make_hoe_instance 默认 instance_id=1");
}

#[test]
fn equipped_main_hand_hoe_returns_none_for_non_hoe() {
    let mut equipped = HashMap::new();
    equipped.insert(
        MAIN_HAND_SLOT.to_string(),
        crate::inventory::SlotContents::held_single(ItemInstance {
            instance_id: 99,
            template_id: "rusted_blade".into(),
            display_name: "rusted_blade".into(),
            grid_w: 1,
            grid_h: 2,
            weight: 1.8,
            rarity: ItemRarity::Common,
            description: String::new(),
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
        }),
    );
    let inv = PlayerInventory {
        triggered_treasures: Vec::new(),
        revision: InventoryRevision(0),
        containers: vec![],
        equipped,
        hotbar: Default::default(),
        bone_coins: 0,
        max_weight: 45.0,
    };
    assert!(equipped_main_hand_hoe(&inv).is_none());
}

fn terminal_settlement(entity: Entity) -> NpcTerminalSettlementSucceeded {
    let life_record = LifeRecord::new(format!("npc:lingtian-test:{}", entity.to_bits()));
    NpcTerminalSettlementSucceeded {
        entity,
        at_tick: 1,
        cause: crate::npc::lifecycle::NpcDeathReason::Combat
            .as_str()
            .to_string(),
        reason: crate::npc::lifecycle::NpcDeathReason::Combat,
        attacker: None,
        attacker_player_id: None,
        authorize_loot: true,
        actor_qi_identity: crate::cultivation::components::ActorQiIdentity::from_life_record(
            &life_record,
            crate::cultivation::components::ActorQiKind::Npc,
        )
        .expect("lingtian terminal fixture must have canonical identity"),
    }
}

#[test]
fn release_lingtian_plot_owner_on_npc_death_clears_npc_owner() {
    let mut app = App::new();
    app.add_event::<NpcTerminalSettlementSucceeded>();
    app.add_systems(Update, release_lingtian_plot_owner_on_npc_death);

    let owner = app.world_mut().spawn(NpcMarker).id();
    let plot = app
        .world_mut()
        .spawn(LingtianPlot::new(BlockPos::new(1, 64, 1), Some(owner)))
        .id();

    app.world_mut().send_event(terminal_settlement(owner));
    app.update();

    assert_eq!(app.world().get::<LingtianPlot>(plot).unwrap().owner, None);
}

#[test]
fn release_lingtian_plot_owner_ignores_unrelated_settlement() {
    let mut app = App::new();
    app.add_event::<NpcTerminalSettlementSucceeded>();
    app.add_systems(Update, release_lingtian_plot_owner_on_npc_death);

    let owner = app.world_mut().spawn(NpcMarker).id();
    let other_npc = app.world_mut().spawn(NpcMarker).id();
    let plot = app
        .world_mut()
        .spawn(LingtianPlot::new(BlockPos::new(1, 64, 1), Some(owner)))
        .id();

    app.world_mut().send_event(terminal_settlement(other_npc));
    app.update();

    assert_eq!(
        app.world().get::<LingtianPlot>(plot).unwrap().owner,
        Some(owner)
    );
}

#[test]
fn till_rejected_when_request_instance_id_mismatches_main_hand() {
    let mut app = build_app();
    let player = spawn_test_player(&mut app, make_inventory_with_hoe(HoeKind::Iron, 1.0));
    // 主手 instance_id=1，但请求声 instance_id=2 → 应被拒
    app.world_mut().send_event(StartTillRequest {
        player,
        pos: BlockPos::new(0, 64, 0),
        hoe_instance_id: 2,
        mode: SessionMode::Manual,
        terrain: TerrainKind::Grass,
        environment: PlotEnvironment::base(),
    });
    app.update();
    assert!(app.world().resource::<ActiveLingtianSessions>().is_empty());
}

#[test]
fn second_till_during_active_session_is_rejected() {
    let mut app = build_app();
    let player = spawn_test_player(&mut app, make_inventory_with_hoe(HoeKind::Iron, 1.0));
    app.world_mut().send_event(StartTillRequest {
        player,
        pos: BlockPos::new(0, 64, 0),
        hoe_instance_id: 1,
        mode: SessionMode::Manual,
        terrain: TerrainKind::Grass,
        environment: PlotEnvironment::base(),
    });
    app.update();
    assert_eq!(app.world().resource::<ActiveLingtianSessions>().len(), 1);
    // 第二请求应被拒
    app.world_mut().send_event(StartTillRequest {
        player,
        pos: BlockPos::new(1, 64, 0),
        hoe_instance_id: 1,
        mode: SessionMode::Manual,
        terrain: TerrainKind::Dirt,
        environment: PlotEnvironment::base(),
    });
    app.update();
    assert_eq!(
        app.world().resource::<ActiveLingtianSessions>().len(),
        1,
        "重复请求不应叠 session"
    );
}

#[test]
fn renew_e2e_resets_barren_plot() {
    let mut app = build_app();
    let pos = BlockPos::new(5, 64, 5);
    let player = valid_test_player(
        &mut app,
        make_inventory_with_hoe(HoeKind::Xuantie, 1.0),
        pos,
    );
    // 直接 spawn 一个贫瘠 plot
    let mut plot = LingtianPlot::new(pos, Some(player));
    plot.harvest_count = super::super::plot::N_RENEW;
    app.world_mut().spawn(plot);

    app.world_mut().send_event(StartRenewRequest {
        player,
        pos,
        hoe_instance_id: 1,
    });
    for _ in 0..RENEW_TICKS {
        app.update();
    }
    assert!(app.world().resource::<ActiveLingtianSessions>().is_empty());
    let plot = app
        .world_mut()
        .query::<&LingtianPlot>()
        .iter(app.world())
        .next()
        .unwrap();
    assert_eq!(plot.harvest_count, 0, "翻新应重置 harvest_count");
    assert!(!plot.is_barren());
    // Xuantie 一次扣 0.01
    let inv = app.world().get::<PlayerInventory>(player).unwrap();
    let dur = inv
        .equipped
        .get(MAIN_HAND_SLOT)
        .unwrap()
        .held
        .as_ref()
        .unwrap()
        .durability;
    assert!((dur - 0.99).abs() < 1e-9, "Xuantie 一次扣 0.01；实得 {dur}");
}

#[test]
fn renew_rejected_when_plot_not_barren() {
    let mut app = build_app();
    let player = spawn_test_player(&mut app, make_inventory_with_hoe(HoeKind::Iron, 1.0));
    let pos = BlockPos::new(0, 64, 0);
    // 新 plot，未贫瘠
    app.world_mut().spawn(LingtianPlot::new(pos, None));
    app.world_mut().send_event(StartRenewRequest {
        player,
        pos,
        hoe_instance_id: 1,
    });
    app.update();
    assert!(app.world().resource::<ActiveLingtianSessions>().is_empty());
}

#[test]
fn hoe_breaks_at_zero_durability() {
    let mut app = build_app();
    // Iron 锄剩 0.05 → 一次操作就归零（uses_max=20，cost=0.05）
    let pos = BlockPos::new(0, 64, 0);
    let player = spawn_test_player(&mut app, make_inventory_with_hoe(HoeKind::Iron, 0.05));
    app.world_mut().send_event(StartTillRequest {
        player,
        pos,
        hoe_instance_id: 1,
        mode: SessionMode::Manual,
        terrain: TerrainKind::Grass,
        environment: PlotEnvironment::base(),
    });
    for _ in 0..TILL_MANUAL_TICKS {
        app.update();
    }
    let inv = app.world().get::<PlayerInventory>(player).unwrap();
    assert!(
        !inv.equipped.contains_key(MAIN_HAND_SLOT),
        "锄归零应从 equipped 移除"
    );
}

// ------------------------------------------------------------------------
// P2 生长 tick e2e
// ------------------------------------------------------------------------

use crate::botany::{GrowthCost, PlantKind, PlantKindRegistry, PlantRarity};
use crate::lingtian::environment::{PlotBiome, PlotEnvironment, PlotLingjuTier};
use crate::lingtian::plot::CropInstance;
use crate::lingtian::qi_account::BEVY_TICKS_PER_LINGTIAN_TICK;
use crate::world::season::Season;

fn ci_she_hao_kind() -> PlantKind {
    PlantKind {
        id: "ci_she_hao".into(),
        display_name: "刺舌蒿".into(),
        cultivable: true,
        growth_cost: GrowthCost::Low,
        growth_duration_ticks: 480,
        rarity: PlantRarity::Common,
        description: String::new(),
    }
}

fn registry_with(kind: PlantKind) -> PlantKindRegistry {
    let mut r = PlantKindRegistry::new();
    r.insert(kind).unwrap();
    r
}

fn build_growth_app(zone_qi: f32) -> App {
    let mut app = App::new();
    let mut acc = ZoneQiAccount::new();
    acc.set(DEFAULT_ZONE, zone_qi);
    app.insert_resource(LingtianTickAccumulator::new())
        .insert_resource(LingtianClock::default())
        .insert_resource(acc)
        .insert_resource(registry_with(ci_she_hao_kind()))
        .add_systems(Update, lingtian_growth_tick);
    app
}

fn build_collapsed_growth_app(zone_qi: f32) -> App {
    let mut app = build_growth_app(zone_qi);
    app.insert_resource(ZoneRegistry {
        zones: vec![crate::world::zone::Zone {
            name: "collapsed_test".to_string(),
            dimension: crate::world::dimension::DimensionKind::Overworld,
            bounds: (DVec3::new(-16.0, 0.0, -16.0), DVec3::new(16.0, 256.0, 16.0)),
            spirit_qi: 0.0,
            danger_level: 5,
            active_events: vec![EVENT_REALM_COLLAPSE.to_string()],
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
            qi_equilibrium: 0.0,
            qi_inflow_per_min: 0.0,
        }],
        spatial_revision: 0,
    });
    app
}

fn spawn_planted_plot(app: &mut App, plot_qi: f32) -> Entity {
    spawn_planted_plot_in_zone(app, plot_qi, "")
}

fn spawn_planted_plot_in_zone(app: &mut App, plot_qi: f32, zone: &str) -> Entity {
    let mut p = LingtianPlot::new(BlockPos::new(0, 64, 0), None);
    p.zone = zone.to_string();
    p.plot_qi = plot_qi;
    p.crop = Some(CropInstance::new("ci_she_hao".into()));
    app.world_mut().spawn(p).id()
}

// 注：1 lingtian-tick = 1200 Bevy tick；通过 `app.update()` 走完整路径
// 单测过慢（每 lingtian-tick ≥ 100ms）。其余生长测试改用
// `advance_n_lingtian_ticks_direct` 直推，accumulator 路径单独由
// `growth_tick_does_not_fire_before_1200_bevy_ticks` 守。

#[test]
fn growth_tick_does_not_fire_before_1200_bevy_ticks() {
    let mut app = build_growth_app(0.0);
    let plot = spawn_planted_plot(&mut app, 1000.0);
    // plot_qi_cap 默认 1.0；为做"持续 baseline mult"，本测只关心: < 1200 tick 不动
    for _ in 0..BEVY_TICKS_PER_LINGTIAN_TICK - 1 {
        app.update();
    }
    let p = app.world().get::<LingtianPlot>(plot).unwrap();
    assert_eq!(
        p.crop.as_ref().unwrap().growth,
        0.0,
        "1200 - 1 个 Bevy tick 不应触发 lingtian-tick"
    );
}

/// 直推 lingtian-tick，跳过 1200×N 个 Bevy update（accumulator 已有独立单测）。
fn advance_n_lingtian_ticks_direct(app: &mut App, n: u32) {
    for _ in 0..n {
        let world = app.world_mut();
        let mut zone_qi = world.remove_resource::<ZoneQiAccount>().unwrap();
        let registry = world.remove_resource::<PlantKindRegistry>().unwrap();
        let zone_registry = world.get_resource::<ZoneRegistry>().cloned();
        let mut state = world.query::<&mut LingtianPlot>();
        for mut plot in state.iter_mut(world) {
            advance_plot_one_lingtian_tick_in_zone(
                &mut plot,
                &registry,
                &mut zone_qi,
                zone_registry.as_ref(),
            );
        }
        world.insert_resource(zone_qi);
        world.insert_resource(registry);
    }
}

#[test]
fn ci_she_hao_ripens_in_480_lingtian_ticks_at_full_qi() {
    let mut app = build_growth_app(0.0);
    // plot_qi cap=1.0；每 lingtian-tick 扣 0.002（low）→ 480 tick 扣 0.96，不会枯。
    // ratio 起始=1.0 → mult=1.5 → 应早于 480 tick 熟。
    let plot = spawn_planted_plot(&mut app, 1.0);
    advance_n_lingtian_ticks_direct(&mut app, 480);
    let p = app.world().get::<LingtianPlot>(plot).unwrap();
    let crop = p.crop.as_ref().unwrap();
    assert!(crop.is_ripe(), "growth = {}", crop.growth);
}

#[test]
fn zone_leak_path_when_plot_qi_dry() {
    let mut app = build_growth_app(2.0); // zone qi 充足
    let plot = spawn_planted_plot(&mut app, 0.0);
    advance_n_lingtian_ticks_direct(&mut app, 10);
    let p = app.world().get::<LingtianPlot>(plot).unwrap();
    let g = p.crop.as_ref().unwrap().growth;
    // 漏吸 10 tick：每次 = 1/480 × 0.3 = 0.000625；累 10 = 0.00625
    let expected = 10.0 * (1.0_f32 / 480.0) * 0.3;
    assert!(
        (g - expected).abs() < 1e-5,
        "growth = {g}, expected ≈ {expected}"
    );
    let zone_left = app.world().resource::<ZoneQiAccount>().get(DEFAULT_ZONE);
    // 漏吸 10 次：每次 0.002 × 0.2 = 0.0004；累 10 = 0.004
    let zone_consumed = 10.0 * 0.002 * 0.2;
    assert!(
        (zone_left - (2.0 - zone_consumed)).abs() < 1e-5,
        "zone_left = {zone_left}"
    );
}

#[test]
fn zone_leak_path_uses_plot_zone_when_non_default() {
    let mut app = build_growth_app(0.0);
    app.world_mut()
        .resource_mut::<ZoneQiAccount>()
        .set("blood_valley", 2.0);
    let plot = spawn_planted_plot_in_zone(&mut app, 0.0, "blood_valley");

    advance_n_lingtian_ticks_direct(&mut app, 10);

    let p = app.world().get::<LingtianPlot>(plot).unwrap();
    let g = p.crop.as_ref().unwrap().growth;
    let expected = 10.0 * (1.0_f32 / 480.0) * 0.3;
    assert!(
        (g - expected).abs() < 1e-5,
        "非默认区 plot 应从 blood_valley 漏吸生长，growth={g}, expected≈{expected}"
    );
    let accounts = app.world().resource::<ZoneQiAccount>();
    let remote_left = accounts.get("blood_valley");
    let default_left = accounts.get(DEFAULT_ZONE);
    let zone_consumed = 10.0 * 0.002 * 0.2;
    assert!(
        (remote_left - (2.0 - zone_consumed)).abs() < 1e-5,
        "blood_valley 应被扣漏吸量，实际 {remote_left}"
    );
    assert_eq!(default_left, 0.0, "非默认区生长不应触碰 default zone");
}

#[test]
fn collapsed_zone_clears_plot_qi_and_stops_growth() {
    let mut app = build_collapsed_growth_app(2.0);
    let plot = spawn_planted_plot(&mut app, 1.0);

    advance_n_lingtian_ticks_direct(&mut app, 1);
    let p = app.world().get::<LingtianPlot>(plot).unwrap();

    assert_eq!(p.plot_qi, 0.0);
    assert_eq!(p.crop.as_ref().unwrap().growth, 0.0);
    assert_eq!(
        app.world().resource::<ZoneQiAccount>().get(DEFAULT_ZONE),
        2.0
    );
}

#[test]
fn stalls_when_plot_and_zone_both_dry() {
    let mut app = build_growth_app(0.0);
    let plot = spawn_planted_plot(&mut app, 0.0);
    advance_n_lingtian_ticks_direct(&mut app, 50);
    let p = app.world().get::<LingtianPlot>(plot).unwrap();
    assert_eq!(
        p.crop.as_ref().unwrap().growth,
        0.0,
        "双干 50 tick 不应有任何生长"
    );
}

// ------------------------------------------------------------------------
// P3 种植 e2e
// ------------------------------------------------------------------------

use crate::inventory::{ContainerState, PlacedItemState};

fn registry_with_three_test_plants() -> PlantKindRegistry {
    let mut r = PlantKindRegistry::new();
    for id in ["ci_she_hao", "ning_mai_cao", "ling_mu_miao"] {
        r.insert(PlantKind {
            id: id.into(),
            display_name: id.into(),
            cultivable: true,
            growth_cost: GrowthCost::Low,
            growth_duration_ticks: 480,
            rarity: PlantRarity::Common,
            description: String::new(),
        })
        .unwrap();
    }
    r
}

fn make_seed_instance(template_id: &str, stack: u32) -> ItemInstance {
    ItemInstance {
        instance_id: 100,
        template_id: template_id.into(),
        display_name: template_id.into(),
        grid_w: 1,
        grid_h: 1,
        weight: 0.05,
        rarity: ItemRarity::Common,
        description: String::new(),
        stack_count: stack,
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

fn make_inventory_with_seed(template_id: &str, stack: u32) -> PlayerInventory {
    let container = ContainerState {
        quick_access: false,
        id: "main_pack".into(),
        name: "main_pack".into(),
        rows: 4,
        cols: 4,
        items: vec![PlacedItemState {
            row: 0,
            col: 0,
            instance: make_seed_instance(template_id, stack),
        }],

        owner_instance_id: None,
    };
    PlayerInventory {
        triggered_treasures: Vec::new(),
        revision: InventoryRevision(0),
        containers: vec![container],
        equipped: HashMap::new(),
        hotbar: Default::default(),
        bone_coins: 0,
        max_weight: 45.0,
    }
}

fn build_planting_app() -> App {
    let mut app = App::new();
    let registry = registry_with_three_test_plants();
    let seeds = SeedRegistry::from_plant_registry(&registry);
    app.insert_resource(ActiveLingtianSessions::new())
        .insert_resource(registry)
        .insert_resource(seeds)
        .insert_resource(ItemRegistry::default())
        .insert_resource(InventoryInstanceIdAllocator::default())
        .insert_resource(LingtianHarvestRng::default())
        .insert_resource(ZoneQiAccount::new())
        .insert_resource(LingtianClock::default())
        .add_event::<StartPlantingRequest>()
        .add_event::<PlantingCompleted>()
        .add_event::<StartTillRequest>()
        .add_event::<TillCompleted>()
        .add_event::<StartRenewRequest>()
        .add_event::<RenewCompleted>()
        .add_event::<StartHarvestRequest>()
        .add_event::<HarvestCompleted>()
        .add_event::<StartReplenishRequest>()
        .add_event::<ReplenishCompleted>()
        .add_event::<DyeContaminationWarning>()
        .add_event::<StartDrainQiRequest>()
        .add_event::<DrainQiCompleted>()
        .add_event::<QiTransfer>()
        .add_event::<SkillXpGain>()
        // fix-spec-1901-v2 §4.1 — C2S 请求持久队列。
        .init_resource::<crate::lingtian::requests::PendingLingtianRequests>()
        .add_systems(
            Update,
            (
                validate_and_dispatch_lingtian_requests,
                handle_start_till,
                handle_start_renew,
                handle_start_planting,
                handle_start_harvest,
                handle_start_replenish,
                handle_start_drain_qi,
                tick_lingtian_sessions,
                apply_completed_sessions,
            )
                .chain()
                .after(crate::world::movement_commit::AuthoritativePositionCommitSet),
        );
    app
}

#[test]
fn planting_e2e_spawns_crop_and_consumes_seed() {
    let mut app = build_planting_app();
    let pos = BlockPos::new(0, 64, 0);
    let player = valid_test_player(
        &mut app,
        make_inventory_with_seed("ci_she_hao_seed", 5),
        pos,
    );
    // 已开垦的空 plot
    app.world_mut().spawn(LingtianPlot::new(pos, Some(player)));
    app.world_mut().send_event(StartPlantingRequest {
        player,
        pos,
        plant_id: "ci_she_hao".into(),
    });
    for _ in 0..PLANTING_TICKS {
        app.update();
    }
    // session 应已结算
    assert!(app.world().resource::<ActiveLingtianSessions>().is_empty());
    // plot 应有 crop = ci_she_hao
    let plot = app
        .world_mut()
        .query::<&LingtianPlot>()
        .iter(app.world())
        .next()
        .unwrap();
    assert_eq!(
        plot.crop.as_ref().map(|c| c.kind.as_str()),
        Some("ci_she_hao")
    );
    // 种子应 -1（5 → 4）
    let inv = app.world().get::<PlayerInventory>(player).unwrap();
    let stack = inv.containers[0].items[0].instance.stack_count;
    assert_eq!(stack, 4);
}

#[test]
fn planting_consumes_last_seed_then_removes_stack() {
    let mut app = build_planting_app();
    let pos = BlockPos::new(1, 64, 1);
    let player = valid_test_player(
        &mut app,
        make_inventory_with_seed("ning_mai_cao_seed", 1),
        pos,
    );
    app.world_mut().spawn(LingtianPlot::new(pos, Some(player)));
    app.world_mut().send_event(StartPlantingRequest {
        player,
        pos,
        plant_id: "ning_mai_cao".into(),
    });
    for _ in 0..PLANTING_TICKS {
        app.update();
    }
    let inv = app.world().get::<PlayerInventory>(player).unwrap();
    assert!(inv.containers[0].items.is_empty(), "最后 1 颗扣完应空格");
}

#[test]
fn planting_rejected_when_no_seed_in_inventory() {
    let mut app = build_planting_app();
    let pos = BlockPos::new(0, 64, 0);
    let player = valid_test_player(
        &mut app,
        make_inventory_with_seed("ci_she_hao_seed", 1),
        pos,
    );
    app.world_mut().spawn(LingtianPlot::new(pos, Some(player)));
    // 请求种 ling_mu_miao（没种子）
    app.world_mut().send_event(StartPlantingRequest {
        player,
        pos,
        plant_id: "ling_mu_miao".into(),
    });
    app.update();
    assert!(app.world().resource::<ActiveLingtianSessions>().is_empty());
}

#[test]
fn planting_rejected_when_plot_already_has_crop() {
    let mut app = build_planting_app();
    let pos = BlockPos::new(0, 64, 0);
    let player = valid_test_player(
        &mut app,
        make_inventory_with_seed("ci_she_hao_seed", 5),
        pos,
    );
    let mut plot = LingtianPlot::new(pos, Some(player));
    plot.crop = Some(CropInstance::new("ning_mai_cao".into()));
    app.world_mut().spawn(plot);
    app.world_mut().send_event(StartPlantingRequest {
        player,
        pos,
        plant_id: "ci_she_hao".into(),
    });
    app.update();
    assert!(app.world().resource::<ActiveLingtianSessions>().is_empty());
}

#[test]
fn planting_rejected_when_plot_barren() {
    let mut app = build_planting_app();
    let pos = BlockPos::new(0, 64, 0);
    let player = valid_test_player(
        &mut app,
        make_inventory_with_seed("ci_she_hao_seed", 5),
        pos,
    );
    let mut plot = LingtianPlot::new(pos, Some(player));
    plot.harvest_count = crate::lingtian::plot::N_RENEW; // 贫瘠
    app.world_mut().spawn(plot);
    app.world_mut().send_event(StartPlantingRequest {
        player,
        pos,
        plant_id: "ci_she_hao".into(),
    });
    app.update();
    assert!(app.world().resource::<ActiveLingtianSessions>().is_empty());
}

#[test]
fn planting_rejected_when_plant_id_unknown_to_seed_registry() {
    let mut app = build_planting_app();
    let pos = BlockPos::new(0, 64, 0);
    let player = valid_test_player(
        &mut app,
        make_inventory_with_seed("ci_she_hao_seed", 5),
        pos,
    );
    app.world_mut().spawn(LingtianPlot::new(pos, Some(player)));
    // shi_mai_gen 非 cultivable，SeedRegistry 不应有它
    app.world_mut().send_event(StartPlantingRequest {
        player,
        pos,
        plant_id: "shi_mai_gen".into(),
    });
    app.update();
    assert!(app.world().resource::<ActiveLingtianSessions>().is_empty());
}

// ------------------------------------------------------------------------
// P4 收获 e2e
// ------------------------------------------------------------------------

use crate::inventory::{ItemCategory, ItemEffect};

fn herb_template(id: &str, display: &str) -> ItemTemplate {
    ItemTemplate {
        id: id.into(),
        display_name: display.into(),
        category: ItemCategory::Herb,
        placeable: None,
        max_stack_count: 64,
        grid_w: 1,
        grid_h: 1,
        base_weight: 0.1,
        rarity: ItemRarity::Common,
        spirit_quality_initial: 0.85,
        description: String::new(),
        effect: None as Option<ItemEffect>,
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
        wearer_race: crate::body_plan::types::RaceGateOwned::default(),
    }
}

fn seed_template(id: &str) -> ItemTemplate {
    ItemTemplate {
        id: id.into(),
        display_name: id.into(),
        category: ItemCategory::Misc,
        placeable: None,
        max_stack_count: 1,
        grid_w: 1,
        grid_h: 1,
        base_weight: 0.05,
        rarity: ItemRarity::Common,
        spirit_quality_initial: 0.7,
        description: String::new(),
        effect: None as Option<ItemEffect>,
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
        wearer_race: crate::body_plan::types::RaceGateOwned::default(),
    }
}

fn registry_with_herb_and_seed_templates() -> ItemRegistry {
    let mut m = HashMap::new();
    for id in ["ci_she_hao", "ning_mai_cao", "ling_mu_miao"] {
        m.insert(id.to_string(), herb_template(id, id));
    }
    for id in ["ci_she_hao_seed", "ning_mai_cao_seed", "ling_mu_miao_seed"] {
        m.insert(id.to_string(), seed_template(id));
    }
    ItemRegistry::from_map(m)
}

fn build_harvest_app_with_item_registry(item_registry: ItemRegistry) -> App {
    let mut app = App::new();
    let plant_registry = registry_with_three_test_plants();
    let seeds = SeedRegistry::from_plant_registry(&plant_registry);
    app.insert_resource(ActiveLingtianSessions::new())
        .insert_resource(plant_registry)
        .insert_resource(seeds)
        .insert_resource(item_registry)
        .insert_resource(InventoryInstanceIdAllocator::default())
        .insert_resource(LingtianHarvestRng::new(0xDEAD_BEEF))
        .insert_resource(ZoneQiAccount::new())
        .insert_resource(LingtianClock::default())
        .add_event::<StartHarvestRequest>()
        .add_event::<HarvestCompleted>()
        .add_event::<StartTillRequest>()
        .add_event::<TillCompleted>()
        .add_event::<StartRenewRequest>()
        .add_event::<RenewCompleted>()
        .add_event::<StartPlantingRequest>()
        .add_event::<PlantingCompleted>()
        .add_event::<StartReplenishRequest>()
        .add_event::<ReplenishCompleted>()
        .add_event::<DyeContaminationWarning>()
        .add_event::<StartDrainQiRequest>()
        .add_event::<DrainQiCompleted>()
        .add_event::<QiTransfer>()
        .add_event::<SkillXpGain>()
        // fix-spec-1901-v2 §4.1 — C2S 请求持久队列。
        .init_resource::<crate::lingtian::requests::PendingLingtianRequests>()
        .add_systems(
            Update,
            (
                validate_and_dispatch_lingtian_requests,
                handle_start_till,
                handle_start_renew,
                handle_start_planting,
                handle_start_harvest,
                handle_start_replenish,
                handle_start_drain_qi,
                tick_lingtian_sessions,
                apply_completed_sessions,
            )
                .chain()
                .after(crate::world::movement_commit::AuthoritativePositionCommitSet),
        );
    app
}

fn build_harvest_app() -> App {
    build_harvest_app_with_item_registry(registry_with_herb_and_seed_templates())
}

fn empty_inventory_8x8() -> PlayerInventory {
    let main_pack = ContainerState {
        quick_access: false,
        id: MAIN_PACK_CONTAINER_ID.into(),
        name: "main".into(),
        rows: 8,
        cols: 8,
        items: vec![],
        owner_instance_id: None,
    };
    PlayerInventory {
        triggered_treasures: Vec::new(),
        revision: InventoryRevision(0),
        containers: vec![main_pack],
        equipped: HashMap::new(),
        hotbar: Default::default(),
        bone_coins: 0,
        max_weight: 999.0,
    }
}

fn spawn_ripe_plot(app: &mut App, plant_id: &str, pos: BlockPos) -> Entity {
    let mut p = LingtianPlot::new(pos, None);
    let mut crop = CropInstance::new(plant_id.into());
    crop.growth = 1.0;
    p.crop = Some(crop);
    app.world_mut().spawn(p).id()
}

fn count_in_main_pack(inv: &PlayerInventory, template_id: &str) -> u32 {
    inv.containers
        .iter()
        .find(|c| c.id == MAIN_PACK_CONTAINER_ID)
        .map(|c| {
            c.items
                .iter()
                .filter(|p| p.instance.template_id == template_id)
                .map(|p| p.instance.stack_count)
                .sum::<u32>()
        })
        .unwrap_or(0)
}

fn count_in_all_containers(inv: &PlayerInventory, template_id: &str) -> u32 {
    inv.containers
        .iter()
        .flat_map(|container| container.items.iter())
        .filter(|placed| placed.instance.template_id == template_id)
        .map(|placed| placed.instance.stack_count)
        .sum()
}

#[test]
fn harvest_e2e_drops_plant_and_clears_plot() {
    let mut app = build_harvest_app();
    let pos = BlockPos::new(2, 64, 2);
    let player = valid_test_player(&mut app, empty_inventory_8x8(), pos);
    let plot = spawn_ripe_plot(&mut app, "ci_she_hao", pos);
    app.world_mut().send_event(StartHarvestRequest {
        player,
        pos,
        mode: SessionMode::Manual,
    });
    for _ in 0..HARVEST_MANUAL_TICKS {
        app.update();
    }
    assert!(app.world().resource::<ActiveLingtianSessions>().is_empty());
    let p = app.world().get::<LingtianPlot>(plot).unwrap();
    assert!(p.crop.is_none(), "plot 应空");
    assert_eq!(p.harvest_count, 1, "harvest_count 应 +1");
    let inv = app.world().get::<PlayerInventory>(player).unwrap();
    assert_eq!(count_in_main_pack(inv, "ci_she_hao"), 1, "应得 1 株作物");
}

#[test]
fn harvest_with_default_runtime_pack_without_main_pack_is_not_lost() {
    let item_registry = crate::inventory::load_item_registry().expect("item registry should load");
    let loadout =
        crate::inventory::load_default_loadout(&item_registry).expect("default loadout loads");
    let mut loadout_allocator = InventoryInstanceIdAllocator::new(3000);
    let inventory = crate::inventory::instantiate_inventory_from_loadout(
        &loadout,
        &mut loadout_allocator,
        &item_registry,
    )
    .expect("default loadout should instantiate");
    let runtime_pack_id = inventory
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
        inventory
            .containers
            .iter()
            .all(|container| container.id != MAIN_PACK_CONTAINER_ID),
        "default loadout no longer creates `{MAIN_PACK_CONTAINER_ID}`; ids={:?}",
        inventory
            .containers
            .iter()
            .map(|container| container.id.as_str())
            .collect::<Vec<_>>()
    );

    let mut app = build_harvest_app_with_item_registry(item_registry);
    let pos = BlockPos::new(4, 64, 4);
    let player = valid_test_player(&mut app, inventory, pos);
    let plot = spawn_ripe_plot(&mut app, "ci_she_hao", pos);
    app.world_mut().send_event(StartHarvestRequest {
        player,
        pos,
        mode: SessionMode::Manual,
    });

    for _ in 0..HARVEST_MANUAL_TICKS {
        app.update();
    }

    let plot = app.world().get::<LingtianPlot>(plot).unwrap();
    assert!(plot.crop.is_none(), "收获完成后 plot 应清空");
    let inv = app.world().get::<PlayerInventory>(player).unwrap();
    assert_eq!(
        count_in_all_containers(inv, "ci_she_hao"),
        1,
        "灵田收获奖励必须进入随身容器，不能因缺 `{MAIN_PACK_CONTAINER_ID}` 静默丢失；\
             runtime_pack={runtime_pack_id}, containers={:?}",
        inv.containers
            .iter()
            .map(|container| {
                (
                    container.id.as_str(),
                    container
                        .items
                        .iter()
                        .map(|placed| placed.instance.template_id.as_str())
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>()
    );
}

#[test]
fn harvest_rejected_when_crop_not_ripe() {
    let mut app = build_harvest_app();
    let pos = BlockPos::new(0, 64, 0);
    let player = valid_test_player(&mut app, empty_inventory_8x8(), pos);
    let mut p = LingtianPlot::new(pos, None);
    let mut crop = CropInstance::new("ci_she_hao".into());
    crop.growth = 0.5;
    p.crop = Some(crop);
    app.world_mut().spawn(p);
    app.world_mut().send_event(StartHarvestRequest {
        player,
        pos,
        mode: SessionMode::Manual,
    });
    app.update();
    assert!(app.world().resource::<ActiveLingtianSessions>().is_empty());
}

#[test]
fn harvest_rejected_when_no_crop() {
    let mut app = build_harvest_app();
    let pos = BlockPos::new(0, 64, 0);
    let player = valid_test_player(&mut app, empty_inventory_8x8(), pos);
    app.world_mut().spawn(LingtianPlot::new(pos, None));
    app.world_mut().send_event(StartHarvestRequest {
        player,
        pos,
        mode: SessionMode::Manual,
    });
    app.update();
    assert!(app.world().resource::<ActiveLingtianSessions>().is_empty());
}

// ───────────────────────── F23 — Auto 采集服务端等级门禁 ─────────────────────────

fn skill_set_with_herbalism_lv(lv: u8) -> SkillSet {
    let mut skills = HashMap::new();
    skills.insert(
        SkillId::Herbalism,
        crate::skill::components::SkillEntry {
            lv,
            ..Default::default()
        },
    );
    SkillSet {
        skills,
        consumed_scrolls: Default::default(),
    }
}

#[test]
fn harvest_auto_rejected_when_herbalism_below_unlock_level() {
    let mut app = build_harvest_app();
    let pos = BlockPos::new(0, 64, 0);
    let player = valid_test_player(&mut app, empty_inventory_8x8(), pos);
    // auto_unlock_level 默认 3；lv=2 明确不足。
    app.world_mut()
        .entity_mut(player)
        .insert(skill_set_with_herbalism_lv(2));
    spawn_ripe_plot(&mut app, "ci_she_hao", pos);
    app.world_mut().send_event(StartHarvestRequest {
        player,
        pos,
        mode: SessionMode::Auto,
    });
    app.update();

    assert!(
        app.world().resource::<ActiveLingtianSessions>().is_empty(),
        "F23: herbalism lv=2 < auto_unlock_level=3 必须被服务端拒绝，不能靠 client UI \
             单层 gating（协议可绕过）"
    );
}

#[test]
fn harvest_auto_rejected_when_herbalism_missing_entirely() {
    // 完全没有 SkillSet/Cultivation 组件 —— 等价 herbalism lv=0，同样必须被拒。
    let mut app = build_harvest_app();
    let pos = BlockPos::new(0, 64, 0);
    let player = valid_test_player(&mut app, empty_inventory_8x8(), pos);
    spawn_ripe_plot(&mut app, "ci_she_hao", pos);
    app.world_mut().send_event(StartHarvestRequest {
        player,
        pos,
        mode: SessionMode::Auto,
    });
    app.update();

    assert!(
        app.world().resource::<ActiveLingtianSessions>().is_empty(),
        "F23: 缺 SkillSet 组件时 herbalism_effective_lv 应回落到 0，同样触发门禁拒绝"
    );
}

#[test]
fn harvest_auto_allowed_when_herbalism_meets_unlock_level() {
    let mut app = build_harvest_app();
    let pos = BlockPos::new(0, 64, 0);
    let player = valid_test_player(&mut app, empty_inventory_8x8(), pos);
    // 刚好等于 auto_unlock_level=3 —— 边界值必须放行（>= 不是 >）。
    app.world_mut()
        .entity_mut(player)
        .insert(skill_set_with_herbalism_lv(3));
    spawn_ripe_plot(&mut app, "ci_she_hao", pos);
    app.world_mut().send_event(StartHarvestRequest {
        player,
        pos,
        mode: SessionMode::Auto,
    });
    app.update();

    assert!(
        !app.world().resource::<ActiveLingtianSessions>().is_empty(),
        "F23: herbalism lv=3 == auto_unlock_level 边界应放行，不应被拒"
    );
}

#[test]
fn harvest_auto_allowed_when_herbalism_well_above_unlock_level() {
    let mut app = build_harvest_app();
    let pos = BlockPos::new(0, 64, 0);
    let player = valid_test_player(&mut app, empty_inventory_8x8(), pos);
    app.world_mut()
        .entity_mut(player)
        .insert(skill_set_with_herbalism_lv(10));
    spawn_ripe_plot(&mut app, "ci_she_hao", pos);
    app.world_mut().send_event(StartHarvestRequest {
        player,
        pos,
        mode: SessionMode::Auto,
    });
    app.update();

    assert!(
        !app.world().resource::<ActiveLingtianSessions>().is_empty(),
        "F23: herbalism lv 远高于门禁值时必须放行"
    );
}

#[test]
fn harvest_manual_mode_is_unaffected_by_herbalism_level() {
    // Manual 模式完全不该受门禁影响，即使玩家一点采集技艺都没有。
    let mut app = build_harvest_app();
    let pos = BlockPos::new(0, 64, 0);
    let player = valid_test_player(&mut app, empty_inventory_8x8(), pos);
    spawn_ripe_plot(&mut app, "ci_she_hao", pos);
    app.world_mut().send_event(StartHarvestRequest {
        player,
        pos,
        mode: SessionMode::Manual,
    });
    app.update();

    assert!(
        !app.world().resource::<ActiveLingtianSessions>().is_empty(),
        "F23: Manual 模式不应被 herbalism 等级门禁拦截"
    );
}

#[test]
fn five_harvests_make_plot_barren() {
    let mut app = build_harvest_app();
    let pos = BlockPos::new(3, 64, 3);
    let player = valid_test_player(&mut app, empty_inventory_8x8(), pos);
    // 收 N_RENEW 次：每次都重新种熟（手动设 growth=1）
    let plot = spawn_ripe_plot(&mut app, "ci_she_hao", pos);
    for i in 0..crate::lingtian::plot::N_RENEW {
        app.world_mut().send_event(StartHarvestRequest {
            player,
            pos,
            mode: SessionMode::Manual,
        });
        for _ in 0..HARVEST_MANUAL_TICKS {
            app.update();
        }
        // 复种（绕过 PlantingSession，直接重熟）
        let mut p = app.world_mut().get_mut::<LingtianPlot>(plot).unwrap();
        assert_eq!(p.harvest_count, i + 1);
        if i + 1 < crate::lingtian::plot::N_RENEW {
            let mut crop = CropInstance::new("ci_she_hao".into());
            crop.growth = 1.0;
            p.crop = Some(crop);
        }
    }
    let p = app.world().get::<LingtianPlot>(plot).unwrap();
    assert!(p.is_barren(), "5 次收获后应贫瘠");
}

#[test]
fn harvest_stack_increments_existing_stack() {
    let mut app = build_harvest_app();
    // 玩家先有一摞 ci_she_hao = 3
    let mut inv = empty_inventory_8x8();
    inv.containers[0]
        .items
        .push(crate::inventory::PlacedItemState {
            row: 0,
            col: 0,
            instance: ItemInstance {
                instance_id: 999,
                template_id: "ci_she_hao".into(),
                display_name: "ci_she_hao".into(),
                grid_w: 1,
                grid_h: 1,
                weight: 0.1,
                rarity: ItemRarity::Common,
                description: String::new(),
                stack_count: 3,
                spirit_quality: 0.85,
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
    let pos = BlockPos::new(0, 64, 0);
    let player = valid_test_player(&mut app, inv, pos);
    spawn_ripe_plot(&mut app, "ci_she_hao", pos);
    app.world_mut().send_event(StartHarvestRequest {
        player,
        pos,
        mode: SessionMode::Manual,
    });
    for _ in 0..HARVEST_MANUAL_TICKS {
        app.update();
    }
    let inv = app.world().get::<PlayerInventory>(player).unwrap();
    assert_eq!(count_in_main_pack(inv, "ci_she_hao"), 4, "原 3 → 4");
    // 校验"叠到原摞而非新建" — 数 ci_she_hao 的 PlacedItemState 数量
    // （种子可能另起一摞，所以不能数总 items.len）
    let ci_she_hao_stacks = inv.containers[0]
        .items
        .iter()
        .filter(|p| p.instance.template_id == "ci_she_hao")
        .count();
    assert_eq!(ci_she_hao_stacks, 1, "应叠到原 ci_she_hao 摞");
}

#[test]
fn harvest_completion_emits_herbalism_skill_xp() {
    let mut app = build_harvest_app();
    let pos = BlockPos::new(0, 64, 0);
    let player = valid_test_player(&mut app, empty_inventory_8x8(), pos);
    spawn_ripe_plot(&mut app, "ci_she_hao", pos);
    app.world_mut().send_event(StartHarvestRequest {
        player,
        pos,
        mode: SessionMode::Manual,
    });

    for _ in 0..HARVEST_MANUAL_TICKS {
        app.update();
    }

    let xp_events = app.world().resource::<Events<SkillXpGain>>();
    let xp = xp_events
        .iter_current_update_events()
        .next()
        .expect("harvest should emit herbalism xp");
    assert_eq!(xp.char_entity, player);
    assert_eq!(xp.skill, SkillId::Herbalism);
    assert_eq!(xp.amount, 2);
    assert!(matches!(
        &xp.source,
        XpGainSource::Action {
            plan_id: "lingtian",
            action: "harvest_manual",
        }
    ));
}

#[test]
fn harvest_drops_seed_when_rng_under_drop_rate() {
    // 先确认：seed=2 的第一 roll < 0.30（Common 掉率），otherwise 测试无意义
    let mut probe = LingtianHarvestRng::new(2);
    let roll = probe.next_f32();
    assert!(roll < 0.30, "seed 2 第一 roll = {roll} 应 < 0.30");

    let mut app = App::new();
    let plant_registry = registry_with_three_test_plants();
    let seeds = SeedRegistry::from_plant_registry(&plant_registry);
    app.insert_resource(ActiveLingtianSessions::new())
        .insert_resource(plant_registry)
        .insert_resource(seeds)
        .insert_resource(registry_with_herb_and_seed_templates())
        .insert_resource(InventoryInstanceIdAllocator::default())
        .insert_resource(LingtianHarvestRng::new(2))
        .insert_resource(ZoneQiAccount::new())
        .insert_resource(LingtianClock::default())
        .add_event::<StartHarvestRequest>()
        .add_event::<HarvestCompleted>()
        .add_event::<StartTillRequest>()
        .add_event::<TillCompleted>()
        .add_event::<StartRenewRequest>()
        .add_event::<RenewCompleted>()
        .add_event::<StartPlantingRequest>()
        .add_event::<PlantingCompleted>()
        .add_event::<StartReplenishRequest>()
        .add_event::<ReplenishCompleted>()
        .add_event::<DyeContaminationWarning>()
        .add_event::<StartDrainQiRequest>()
        .add_event::<DrainQiCompleted>()
        .add_event::<QiTransfer>()
        .add_event::<SkillXpGain>()
        .add_systems(
            Update,
            (
                handle_start_harvest,
                tick_lingtian_sessions,
                apply_completed_sessions,
            )
                .chain()
                .after(crate::world::movement_commit::AuthoritativePositionCommitSet),
        );

    let pos = BlockPos::new(0, 64, 0);
    let player = valid_test_player(&mut app, empty_inventory_8x8(), pos);
    spawn_ripe_plot(&mut app, "ci_she_hao", pos);
    app.world_mut().send_event(StartHarvestRequest {
        player,
        pos,
        mode: SessionMode::Manual,
    });
    for _ in 0..HARVEST_MANUAL_TICKS {
        app.update();
    }
    let inv = app.world().get::<PlayerInventory>(player).unwrap();
    assert_eq!(count_in_main_pack(inv, "ci_she_hao"), 1);
    assert_eq!(
        count_in_main_pack(inv, "ci_she_hao_seed"),
        1,
        "RNG roll < 0.3 应掉种子"
    );
}

// ------------------------------------------------------------------------
// P5 补灵 e2e
// ------------------------------------------------------------------------

fn spawn_empty_plot(app: &mut App, pos: BlockPos) -> Entity {
    spawn_empty_plot_in_zone(app, pos, "")
}

fn spawn_empty_plot_in_zone(app: &mut App, pos: BlockPos, zone: &str) -> Entity {
    let mut p = LingtianPlot::new(pos, None);
    p.zone = zone.to_string();
    p.plot_qi = 0.0;
    // plot_qi_cap 默认 1.0
    app.world_mut().spawn(p).id()
}

fn make_inventory_with_bone_coins(coins: u64) -> PlayerInventory {
    let mut inv = empty_inventory_8x8();
    inv.bone_coins = coins;
    inv
}

fn make_inventory_with_misc_stack(template_id: &str, stack: u32) -> PlayerInventory {
    let mut inv = empty_inventory_8x8();
    inv.containers[0]
        .items
        .push(crate::inventory::PlacedItemState {
            row: 0,
            col: 0,
            instance: ItemInstance {
                instance_id: 5000,
                template_id: template_id.into(),
                display_name: template_id.into(),
                grid_w: 1,
                grid_h: 1,
                weight: 0.3,
                rarity: ItemRarity::Common,
                description: String::new(),
                stack_count: stack,
                spirit_quality: 0.7,
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
    inv
}

fn make_inventory_with_residue(
    kind: crate::alchemy::residue::PillResidueKind,
    produced_at_tick: u64,
    stack: u32,
) -> PlayerInventory {
    let mut inv = make_inventory_with_misc_stack(kind.spec().template_id, stack);
    inv.containers[0].items[0].instance.alchemy = Some(
        crate::alchemy::residue::residue_alchemy_data(kind, produced_at_tick),
    );
    inv
}

#[test]
fn replenish_zone_drains_zone_qi_and_fills_plot() {
    let mut app = build_app();
    // zone qi 充足
    app.world_mut()
        .resource_mut::<ZoneQiAccount>()
        .set(DEFAULT_ZONE, 5.0);
    let pos = BlockPos::new(0, 64, 0);
    let player = valid_test_player(&mut app, empty_inventory_8x8(), pos);
    let plot = spawn_empty_plot(&mut app, pos);
    app.world_mut().send_event(StartReplenishRequest {
        player,
        pos,
        source: ReplenishSource::Zone,
    });
    for _ in 0..ReplenishSource::Zone.duration_ticks() {
        app.update();
    }
    assert!(app.world().resource::<ActiveLingtianSessions>().is_empty());
    let p = app.world().get::<LingtianPlot>(plot).unwrap();
    assert!((p.plot_qi - 0.5).abs() < 1e-6, "plot_qi 应 +0.5");
    let z = app.world().resource::<ZoneQiAccount>().get(DEFAULT_ZONE);
    assert!((z - 4.5).abs() < 1e-6, "zone qi 应 -0.5");
}

#[test]
fn replenish_zone_uses_non_default_plot_zone_for_precheck_and_debit() {
    let mut app = build_app();
    app.world_mut()
        .resource_mut::<ZoneQiAccount>()
        .set(DEFAULT_ZONE, 0.0);
    app.world_mut()
        .resource_mut::<ZoneQiAccount>()
        .set("blood_valley", 5.0);
    let pos = BlockPos::new(8, 64, 8);
    let player = valid_test_player(&mut app, empty_inventory_8x8(), pos);
    let plot = spawn_empty_plot_in_zone(&mut app, pos, "blood_valley");

    app.world_mut().send_event(StartReplenishRequest {
        player,
        pos,
        source: ReplenishSource::Zone,
    });
    for _ in 0..ReplenishSource::Zone.duration_ticks() {
        app.update();
    }

    assert!(
        app.world().resource::<ActiveLingtianSessions>().is_empty(),
        "blood_valley 余额充足时应允许非默认区补灵，不能被 default=0 拦截"
    );
    let p = app.world().get::<LingtianPlot>(plot).unwrap();
    assert!((p.plot_qi - 0.5).abs() < 1e-6, "plot_qi 应 +0.5");
    let accounts = app.world().resource::<ZoneQiAccount>();
    assert!(
        (accounts.get("blood_valley") - 4.5).abs() < 1e-6,
        "补灵应扣 blood_valley，而不是 default"
    );
    assert_eq!(accounts.get(DEFAULT_ZONE), 0.0, "default zone 不应被扣款");
}

/// plan-zone-qi-economy-v1 P2：地板红线——zone qi 不足以支付 `plot_qi_amount()`
/// 且留住 `QI_NPC_ABSORB_FLOOR` 底仓时，StartReplenishRequest 必须被材料检查直接
/// 拒绝（不开 session），不能像修 P0 之前那样把 zone 抽穿地板。
#[test]
fn replenish_zone_rejected_when_zone_qi_insufficient_to_cover_floor() {
    let mut app = build_app();
    // amount=0.5，floor=0.3 → 需要 >= 0.8 才允许；这里只给 0.79（差一点点）。
    app.world_mut()
        .resource_mut::<ZoneQiAccount>()
        .set(DEFAULT_ZONE, 0.79);
    let pos = BlockPos::new(0, 64, 0);
    let player = valid_test_player(&mut app, empty_inventory_8x8(), pos);
    spawn_empty_plot(&mut app, pos);
    app.world_mut().send_event(StartReplenishRequest {
        player,
        pos,
        source: ReplenishSource::Zone,
    });
    app.update();
    assert!(
        app.world().resource::<ActiveLingtianSessions>().is_empty(),
        "zone_qi=0.79 不足以支付 amount(0.5)+floor(0.3)=0.8，应被材料检查拒绝，不应开 session"
    );
    // zone qi 分毫未动（连 session 都没开）。
    let z = app.world().resource::<ZoneQiAccount>().get(DEFAULT_ZONE);
    assert!((z - 0.79).abs() < 1e-6, "被拒绝的请求不应触碰 zone qi");
}

/// 边界回归：zone qi 恰好等于 amount+floor（0.8）时应被允许，补灵后 zone 恰好
/// 停在地板（0.3），不多不少。
#[test]
fn replenish_zone_allowed_exactly_at_floor_boundary() {
    let mut app = build_app();
    app.world_mut()
        .resource_mut::<ZoneQiAccount>()
        .set(DEFAULT_ZONE, 0.8);
    let pos = BlockPos::new(0, 64, 0);
    let player = valid_test_player(&mut app, empty_inventory_8x8(), pos);
    let plot = spawn_empty_plot(&mut app, pos);
    app.world_mut().send_event(StartReplenishRequest {
        player,
        pos,
        source: ReplenishSource::Zone,
    });
    for _ in 0..ReplenishSource::Zone.duration_ticks() {
        app.update();
    }
    assert!(
        app.world().resource::<ActiveLingtianSessions>().is_empty(),
        "zone_qi=0.8 恰好等于 amount+floor，应被允许并正常完成"
    );
    let p = app.world().get::<LingtianPlot>(plot).unwrap();
    assert!((p.plot_qi - 0.5).abs() < 1e-6, "plot_qi 应 +0.5");
    let z = app.world().resource::<ZoneQiAccount>().get(DEFAULT_ZONE);
    assert!(
        (z - 0.3).abs() < 1e-6,
        "zone qi 补灵后应恰好停在地板 0.3，实际 {z}"
    );
}

#[test]
fn replenish_bone_coin_consumes_one_coin_and_adds_0_8() {
    let mut app = build_app();
    let pos = BlockPos::new(0, 64, 0);
    let player = valid_test_player(&mut app, make_inventory_with_bone_coins(3), pos);
    let plot = spawn_empty_plot(&mut app, pos);
    app.world_mut().send_event(StartReplenishRequest {
        player,
        pos,
        source: ReplenishSource::BoneCoin,
    });
    for _ in 0..ReplenishSource::BoneCoin.duration_ticks() {
        app.update();
    }
    let p = app.world().get::<LingtianPlot>(plot).unwrap();
    assert!((p.plot_qi - 0.8).abs() < 1e-6);
    let inv = app.world().get::<PlayerInventory>(player).unwrap();
    assert_eq!(inv.bone_coins, 2);
}

#[test]
fn replenish_beast_core_overflows_to_zone_when_full() {
    let mut app = build_app();
    let pos = BlockPos::new(0, 64, 0);
    let player = valid_test_player(
        &mut app,
        make_inventory_with_misc_stack("mutant_beast_core", 1),
        pos,
    );
    let plot = spawn_empty_plot(&mut app, pos);
    // plot_qi 已经在 0.5/1.0 → 注 2.0 → +0.5 满，溢出 1.5 回 zone
    app.world_mut()
        .get_mut::<LingtianPlot>(plot)
        .unwrap()
        .plot_qi = 0.5;
    let zone_before = app.world().resource::<ZoneQiAccount>().get(DEFAULT_ZONE);
    app.world_mut().send_event(StartReplenishRequest {
        player,
        pos,
        source: ReplenishSource::BeastCore,
    });
    for _ in 0..ReplenishSource::BeastCore.duration_ticks() {
        app.update();
    }
    let p = app.world().get::<LingtianPlot>(plot).unwrap();
    assert!((p.plot_qi - 1.0).abs() < 1e-6, "plot_qi 拉满 1.0");
    let zone_after = app.world().resource::<ZoneQiAccount>().get(DEFAULT_ZONE);
    assert!(
        (zone_after - zone_before - 1.5).abs() < 1e-6,
        "1.5 应回馈 zone"
    );
    // 兽核应被消耗（从背包移除）
    let inv = app.world().get::<PlayerInventory>(player).unwrap();
    assert_eq!(count_in_main_pack(inv, "mutant_beast_core"), 0);
}

#[test]
fn replenish_overflow_returns_to_non_default_plot_zone() {
    let mut app = build_app();
    app.world_mut()
        .resource_mut::<ZoneQiAccount>()
        .set(DEFAULT_ZONE, 0.0);
    app.world_mut()
        .resource_mut::<ZoneQiAccount>()
        .set("north_wastes", 1.0);
    let pos = BlockPos::new(9, 64, 9);
    let player = valid_test_player(
        &mut app,
        make_inventory_with_misc_stack("mutant_beast_core", 1),
        pos,
    );
    let plot = spawn_empty_plot_in_zone(&mut app, pos, "north_wastes");
    app.world_mut()
        .get_mut::<LingtianPlot>(plot)
        .unwrap()
        .plot_qi = 0.5;

    app.world_mut().send_event(StartReplenishRequest {
        player,
        pos,
        source: ReplenishSource::BeastCore,
    });
    for _ in 0..ReplenishSource::BeastCore.duration_ticks() {
        app.update();
    }

    let accounts = app.world().resource::<ZoneQiAccount>();
    assert!(
        (accounts.get("north_wastes") - 2.5).abs() < 1e-6,
        "1.5 overflow 应回流 north_wastes，实际 {}",
        accounts.get("north_wastes")
    );
    assert_eq!(
        accounts.get(DEFAULT_ZONE),
        0.0,
        "非默认区 overflow 不应串到 default"
    );
}

#[test]
fn replenish_ling_shui_consumes_one_bottle() {
    let mut app = build_app();
    let pos = BlockPos::new(0, 64, 0);
    let player = valid_test_player(
        &mut app,
        make_inventory_with_misc_stack("ling_shui", 2),
        pos,
    );
    let plot = spawn_empty_plot(&mut app, pos);
    app.world_mut().send_event(StartReplenishRequest {
        player,
        pos,
        source: ReplenishSource::LingShui,
    });
    for _ in 0..ReplenishSource::LingShui.duration_ticks() {
        app.update();
    }
    let p = app.world().get::<LingtianPlot>(plot).unwrap();
    assert!((p.plot_qi - 0.3).abs() < 1e-6);
    let inv = app.world().get::<PlayerInventory>(player).unwrap();
    assert_eq!(count_in_main_pack(inv, "ling_shui"), 1);
}

#[test]
fn all_pill_residue_kinds_consume_stack_and_apply_spec_effects() {
    for kind in [
        crate::alchemy::residue::PillResidueKind::FailedPill,
        crate::alchemy::residue::PillResidueKind::FlawedPill,
        crate::alchemy::residue::PillResidueKind::ProcessingDregs,
        crate::alchemy::residue::PillResidueKind::AgingScraps,
    ] {
        let mut app = build_app();
        app.world_mut()
            .insert_resource(LingtianHarvestRng::new(343));
        let pos = BlockPos::new(0, 64, 0);
        let player = valid_test_player(&mut app, make_inventory_with_residue(kind, 0, 1), pos);
        let plot = spawn_empty_plot(&mut app, pos);
        app.world_mut().send_event(StartReplenishRequest {
            player,
            pos,
            source: ReplenishSource::PillResidue { residue_kind: kind },
        });
        let duration = (ReplenishSource::PillResidue { residue_kind: kind }).duration_ticks();
        for _ in 0..duration {
            app.update();
        }

        let spec = kind.spec();
        let p = app.world().get::<LingtianPlot>(plot).unwrap();
        assert!(
            (p.plot_qi - spec.plot_qi_amount).abs() < 1e-6,
            "{kind:?} should add plot_qi per spec"
        );
        assert!(
            (p.dye_contamination - spec.contamination_delta).abs() < 1e-6,
            "{kind:?} should add contamination per spec when roll hits"
        );
        let inv = app.world().get::<PlayerInventory>(player).unwrap();
        assert_eq!(count_in_main_pack(inv, spec.template_id), 0);
    }
}

#[test]
fn residue_contamination_warning_records_world_state_event() {
    let mut app = build_app();
    app.world_mut().insert_resource(LingtianHarvestRng::new(2));
    app.world_mut().resource_mut::<CombatClock>().tick = 987;
    let kind = crate::alchemy::residue::PillResidueKind::FailedPill;
    let pos = BlockPos::new(0, 64, 0);
    let player = valid_test_player(
        &mut app,
        (
            Username("Azure".to_string()),
            make_inventory_with_residue(kind, 0, 1),
        ),
        pos,
    );
    let plot = spawn_empty_plot_in_zone(&mut app, pos, "lingquan_marsh");
    app.world_mut()
        .get_mut::<LingtianPlot>(plot)
        .unwrap()
        .dye_contamination = 0.25;

    app.world_mut().send_event(StartReplenishRequest {
        player,
        pos,
        source: ReplenishSource::PillResidue { residue_kind: kind },
    });
    let duration = (ReplenishSource::PillResidue { residue_kind: kind }).duration_ticks();
    for _ in 0..duration {
        app.update();
    }

    let events = app
        .world()
        .resource::<ActiveEventsResource>()
        .recent_events_snapshot();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_type, GameEventType::EventTriggered);
    assert_eq!(
        events[0].target.as_deref(),
        Some("lingtian_plot_dye_contamination_warning")
    );
    assert_eq!(events[0].zone.as_deref(), Some("lingquan_marsh"));
    assert_eq!(events[0].tick, 987);
    assert_eq!(events[0].player.as_deref(), Some("offline:Azure"));
    assert_eq!(
        events[0]
            .details
            .as_ref()
            .and_then(|details| details.get("source")),
        Some(&serde_json::json!("pill_residue_failed_pill"))
    );
}

#[test]
fn replenish_rejects_expired_residue() {
    let mut app = build_app();
    let kind = crate::alchemy::residue::PillResidueKind::FailedPill;
    let pos = BlockPos::new(0, 64, 0);
    let player = valid_test_player(&mut app, make_inventory_with_residue(kind, 10, 1), pos);
    app.world_mut().resource_mut::<CombatClock>().tick =
        10 + crate::alchemy::residue::PILL_RESIDUE_TTL_TICKS;
    spawn_empty_plot(&mut app, pos);
    app.world_mut().send_event(StartReplenishRequest {
        player,
        pos,
        source: ReplenishSource::PillResidue { residue_kind: kind },
    });
    app.update();

    assert!(app.world().resource::<ActiveLingtianSessions>().is_empty());
    let inv = app.world().get::<PlayerInventory>(player).unwrap();
    assert_eq!(count_in_main_pack(inv, kind.spec().template_id), 1);
}

#[test]
fn residue_now_tick_prefers_combat_clock_over_lingtian_clock() {
    let combat_clock = CombatClock { tick: 123 };
    let lingtian_clock = LingtianClock {
        lingtian_tick: 99_999,
    };

    assert_eq!(residue_now_tick(Some(&combat_clock), &lingtian_clock), 123);
}

#[test]
fn replenish_rejected_when_no_material() {
    let mut app = build_app();
    // bone_coins=0，请求 BoneCoin → 拒
    let pos = BlockPos::new(0, 64, 0);
    let player = valid_test_player(&mut app, empty_inventory_8x8(), pos);
    spawn_empty_plot(&mut app, pos);
    app.world_mut().send_event(StartReplenishRequest {
        player,
        pos,
        source: ReplenishSource::BoneCoin,
    });
    app.update();
    assert!(app.world().resource::<ActiveLingtianSessions>().is_empty());
}

#[test]
fn replenish_rejected_when_in_cooldown() {
    let mut app = build_app();
    let pos = BlockPos::new(0, 64, 0);
    let player = valid_test_player(&mut app, make_inventory_with_bone_coins(2), pos);
    let plot = spawn_empty_plot(&mut app, pos);
    // 模拟"刚补过" — last_replenish_at 设到当前 clock
    app.world_mut()
        .resource_mut::<LingtianClock>()
        .lingtian_tick = 1000;
    app.world_mut()
        .get_mut::<LingtianPlot>(plot)
        .unwrap()
        .last_replenish_at = 1000;

    app.world_mut().send_event(StartReplenishRequest {
        player,
        pos,
        source: ReplenishSource::BoneCoin,
    });
    app.update();
    assert!(app.world().resource::<ActiveLingtianSessions>().is_empty());
    // 骨币没扣
    let inv = app.world().get::<PlayerInventory>(player).unwrap();
    assert_eq!(inv.bone_coins, 2);
}

#[test]
fn replenish_allowed_after_cooldown_expires() {
    let mut app = build_app();
    let pos = BlockPos::new(0, 64, 0);
    let player = valid_test_player(&mut app, make_inventory_with_bone_coins(2), pos);
    let plot = spawn_empty_plot(&mut app, pos);
    app.world_mut()
        .resource_mut::<LingtianClock>()
        .lingtian_tick = REPLENISH_COOLDOWN_LINGTIAN_TICKS + 100;
    app.world_mut()
        .get_mut::<LingtianPlot>(plot)
        .unwrap()
        .last_replenish_at = 50; // 距今 4370 lingtian-tick > 4320 冷却
    app.world_mut().send_event(StartReplenishRequest {
        player,
        pos,
        source: ReplenishSource::BoneCoin,
    });
    for _ in 0..ReplenishSource::BoneCoin.duration_ticks() {
        app.update();
    }
    // 应已结算
    assert!(app.world().resource::<ActiveLingtianSessions>().is_empty());
    let p = app.world().get::<LingtianPlot>(plot).unwrap();
    assert!((p.plot_qi - 0.8).abs() < 1e-6);
}

// ------------------------------------------------------------------------
// P2 plot_qi_cap 修饰 e2e
// ------------------------------------------------------------------------

#[test]
fn till_with_combined_environment_yields_cap_2_8() {
    let mut app = build_app();
    let player = spawn_test_player(&mut app, make_inventory_with_hoe(HoeKind::Iron, 1.0));
    app.world_mut().send_event(StartTillRequest {
        player,
        pos: BlockPos::new(0, 64, 0),
        hoe_instance_id: 1,
        mode: SessionMode::Manual,
        terrain: TerrainKind::Grass,
        // 用 SummerToWinter（汐转期 modifier=0）保留 plan-lingtian-v1 的
        // "三大基础修饰 → cap 2.8" 锁定，避免与 plan-lingtian-weather-v1 §2
        // 夏散 -0.2 / 冬聚 +0.2 的物理修饰交织。
        environment: PlotEnvironment {
            water_adjacent: true,
            biome: PlotBiome::Wetland,
            zhenfa_lingju_tier: PlotLingjuTier::Full,
            season: Season::SummerToWinter,
            active_weather: None,
        },
    });
    for _ in 0..TILL_MANUAL_TICKS {
        app.update();
    }
    let plot = app
        .world_mut()
        .query::<&LingtianPlot>()
        .iter(app.world())
        .next()
        .unwrap();
    // 1.0 + 0.3 + 0.5 + 1.0 = 2.8
    assert!((plot.plot_qi_cap - 2.8).abs() < 1e-6);
}

#[test]
fn till_default_summer_environment_yields_cap_0_8() {
    // plan-lingtian-weather-v1 §2 — `PlotEnvironment::base()` 默认 Summer
    // (-0.2 modifier)，所以裸开垦的 plot_qi_cap = 1.0 - 0.2 = 0.8。
    let mut app = build_app();
    let player = spawn_test_player(&mut app, make_inventory_with_hoe(HoeKind::Iron, 1.0));
    app.world_mut().send_event(StartTillRequest {
        player,
        pos: BlockPos::new(0, 64, 0),
        hoe_instance_id: 1,
        mode: SessionMode::Manual,
        terrain: TerrainKind::Grass,
        environment: PlotEnvironment::base(),
    });
    for _ in 0..TILL_MANUAL_TICKS {
        app.update();
    }
    let plot = app
        .world_mut()
        .query::<&LingtianPlot>()
        .iter(app.world())
        .next()
        .unwrap();
    assert!(
        (plot.plot_qi_cap - 0.8).abs() < 1e-6,
        "Summer base 应当 0.8（=1.0 - 0.2 summer），实际 {}",
        plot.plot_qi_cap
    );
}

#[test]
fn till_xizhuan_environment_keeps_cap_at_1_0() {
    // 汐转期 modifier=0，plot_qi_cap 锁回 plan-lingtian-v1 的 1.0 基线。
    let mut app = build_app();
    let player = spawn_test_player(&mut app, make_inventory_with_hoe(HoeKind::Iron, 1.0));
    app.world_mut().send_event(StartTillRequest {
        player,
        pos: BlockPos::new(0, 64, 0),
        hoe_instance_id: 1,
        mode: SessionMode::Manual,
        terrain: TerrainKind::Grass,
        environment: PlotEnvironment {
            season: Season::SummerToWinter,
            ..PlotEnvironment::base()
        },
    });
    for _ in 0..TILL_MANUAL_TICKS {
        app.update();
    }
    let plot = app
        .world_mut()
        .query::<&LingtianPlot>()
        .iter(app.world())
        .next()
        .unwrap();
    assert!((plot.plot_qi_cap - 1.0).abs() < 1e-6);
}

// ------------------------------------------------------------------------
// §5.1 密度阈值 e2e
// ------------------------------------------------------------------------

use crate::lingtian::pressure::{PressureLevel as PL, PRESSURE_HIGH, PRESSURE_LOW, PRESSURE_MID};

fn build_pressure_app(natural_supply: f32) -> App {
    // 默认 pin 在 Summer（plan §2 的"夏散" 物理常态）：natural_supply -10%，
    // amplitude=0 → jitter 不影响（可重现）。原 plan-lingtian-v1 §5.1 测试
    // 大多用 natural_supply=0，0 × 任何系数仍是 0，不受影响；只有
    // `natural_supply_offsets_demand` 的断言因夏 -10% 调整。
    build_pressure_app_with_season(natural_supply, Season::Summer)
}

fn build_pressure_app_with_season(natural_supply: f32, season: Season) -> App {
    let mut app = App::new();
    let mut tracker = ZonePressureTracker::new();
    tracker.set_natural_supply(DEFAULT_ZONE, natural_supply);
    let mut plant_registry = PlantKindRegistry::new();
    plant_registry
        .insert(PlantKind {
            id: "ling_mu_miao".into(),
            display_name: "灵木苗".into(),
            cultivable: true,
            growth_cost: GrowthCost::High, // 0.012 / tick
            growth_duration_ticks: 28800,
            rarity: PlantRarity::Rare,
            description: String::new(),
        })
        .unwrap();
    // 显式 pin 季节状态：测试可重现，不受默认 query_season 影响。
    // 用 Default + 字段覆写避开 `tick_offset` 私有字段限制（cross-module）。
    let mut season_state = crate::world::season::WorldSeasonState::default();
    season_state.current = crate::world::season::SeasonState {
        season,
        tick_into_phase: 0,
        phase_total_ticks: season.phase_total_ticks(),
        year_index: 0,
    };
    season_state.last_phase_change_tick = 0;

    app.insert_resource(LingtianTickAccumulator::new())
        .insert_resource(LingtianClock::default())
        .insert_resource(ZoneQiAccount::new())
        .insert_resource(plant_registry)
        .insert_resource(tracker)
        .insert_resource(season_state)
        .add_event::<ReplenishCompleted>()
        .add_event::<StartDrainQiRequest>()
        .add_event::<DrainQiCompleted>()
        .add_event::<QiTransfer>()
        .add_event::<ZonePressureCrossed>()
        .add_systems(
            Update,
            (
                lingtian_growth_tick,
                record_replenish_to_pressure,
                compute_zone_pressure_system,
            )
                .chain(),
        );
    app
}

fn spawn_high_cost_planted(app: &mut App, n: u32) {
    spawn_high_cost_planted_with_owner(app, n, None);
}

fn spawn_high_cost_planted_with_owner(app: &mut App, n: u32, owner: Option<Entity>) {
    spawn_high_cost_planted_with_owner_in_zone(app, n, owner, "");
}

fn spawn_high_cost_planted_in_zone(app: &mut App, n: u32, zone: &str) {
    spawn_high_cost_planted_with_owner_in_zone(app, n, None, zone);
}

fn spawn_high_cost_planted_with_owner_in_zone(
    app: &mut App,
    n: u32,
    owner: Option<Entity>,
    zone: &str,
) {
    for i in 0..n {
        let mut p = LingtianPlot::new(BlockPos::new(i as i32, 64, 0), owner);
        p.zone = zone.to_string();
        p.plot_qi = 1.0;
        p.crop = Some(CropInstance::new("ling_mu_miao".into()));
        app.world_mut().spawn(p);
    }
}

fn step_one_lingtian_tick(app: &mut App) {
    for _ in 0..BEVY_TICKS_PER_LINGTIAN_TICK {
        app.update();
    }
}

fn collect_pressure_events(app: &mut App) -> Vec<(PL, f32)> {
    let world = app.world_mut();
    let events = world.resource::<bevy_ecs::event::Events<ZonePressureCrossed>>();
    let mut reader = events.get_reader();
    reader
        .read(events)
        .map(|e| (e.level, e.raw_pressure))
        .collect()
}

fn collect_pressure_event_zones(app: &mut App) -> Vec<(String, PL, f32)> {
    let world = app.world_mut();
    let events = world.resource::<bevy_ecs::event::Events<ZonePressureCrossed>>();
    let mut reader = events.get_reader();
    reader
        .read(events)
        .map(|e| (e.zone.clone(), e.level, e.raw_pressure))
        .collect()
}

#[test]
fn no_event_when_pressure_below_low() {
    let mut app = build_pressure_app(0.0);
    spawn_high_cost_planted(&mut app, 1);
    step_one_lingtian_tick(&mut app);
    assert!(collect_pressure_events(&mut app).is_empty());
    let tracker = app.world().resource::<ZonePressureTracker>();
    assert_eq!(
        tracker.state(DEFAULT_ZONE).map(|s| s.last_level),
        Some(PL::None)
    );
}

#[test]
fn rises_through_low_mid_high_with_increasing_plot_count() {
    let mut app = build_pressure_app(0.0);
    // demand 0.012 × N。f32 累加噪音 ~1e-7：用整除留 5% 余量
    // LOW: 26 × 0.012 ≈ 0.312
    spawn_high_cost_planted(&mut app, 26);
    step_one_lingtian_tick(&mut app);
    let evts = collect_pressure_events(&mut app);
    assert_eq!(evts.len(), 1);
    assert_eq!(evts[0].0, PL::Low);

    // 加到 51（demand ≈ 0.612 → MID）
    spawn_high_cost_planted(&mut app, 25);
    step_one_lingtian_tick(&mut app);
    let evts = collect_pressure_events(&mut app);
    assert_eq!(evts.last().map(|(l, _)| *l), Some(PL::Mid));

    // 加到 85（demand ≈ 1.020 → HIGH）
    spawn_high_cost_planted(&mut app, 34);
    step_one_lingtian_tick(&mut app);
    let evts = collect_pressure_events(&mut app);
    assert_eq!(evts.last().map(|(l, _)| *l), Some(PL::High));
}

#[test]
fn high_pressure_clears_zone_plot_qi() {
    let mut app = build_pressure_app(0.0);
    spawn_high_cost_planted(&mut app, 100); // demand ~1.2 → HIGH
    step_one_lingtian_tick(&mut app);
    let any_nonzero = app
        .world_mut()
        .query::<&LingtianPlot>()
        .iter(app.world())
        .any(|p| p.plot_qi > 0.0);
    assert!(!any_nonzero, "HIGH 应清掉所有 plot_qi");
}

#[test]
fn high_pressure_clears_only_matching_non_default_zone() {
    let mut app = build_pressure_app(0.0);
    spawn_high_cost_planted(&mut app, 1);
    spawn_high_cost_planted_in_zone(&mut app, 100, "blood_valley");

    step_one_lingtian_tick(&mut app);

    let tracker = app.world().resource::<ZonePressureTracker>();
    assert_eq!(
        tracker.state("blood_valley").unwrap().last_level,
        PL::High,
        "blood_valley 自身 demand 应触发 HIGH"
    );
    assert_eq!(
        tracker.state(DEFAULT_ZONE).unwrap().last_level,
        PL::None,
        "default 只有 1 个 plot，不应被 blood_valley 串账抬压"
    );
    let events = collect_pressure_event_zones(&mut app);
    assert!(
        events
            .iter()
            .any(|(zone, level, _)| zone == "blood_valley" && *level == PL::High),
        "应发 blood_valley 的 HIGH 事件，实际 {events:?}"
    );
    let (default_nonzero, remote_nonzero) = {
        let mut query = app.world_mut().query::<&LingtianPlot>();
        let mut default_nonzero = false;
        let mut remote_nonzero = false;
        for plot in query.iter(app.world()) {
            if plot_zone_key(plot) == DEFAULT_ZONE && plot.plot_qi > 0.0 {
                default_nonzero = true;
            }
            if plot_zone_key(plot) == "blood_valley" && plot.plot_qi > 0.0 {
                remote_nonzero = true;
            }
        }
        (default_nonzero, remote_nonzero)
    };
    assert!(default_nonzero, "default plot_qi 不应被非默认区 HIGH 清掉");
    assert!(!remote_nonzero, "blood_valley HIGH 应清掉本区 plot_qi");
}

#[test]
fn npc_owned_plots_count_toward_zone_pressure() {
    let mut app = build_pressure_app(0.0);
    let npc = app.world_mut().spawn(NpcMarker).id();
    spawn_high_cost_planted_with_owner(&mut app, 85, Some(npc)); // demand ~1.02 → HIGH
    step_one_lingtian_tick(&mut app);

    let tracker = app.world().resource::<ZonePressureTracker>();
    assert_eq!(
        tracker.state(DEFAULT_ZONE).unwrap().last_level,
        PL::High,
        "ZonePressureTracker 应统计 NPC owner 的灵田，而不是只统计玩家灵田"
    );
}

#[test]
fn natural_supply_offsets_demand_in_summer() {
    // plan-lingtian-weather-v1 §2 — Summer natural_supply -10%：
    // base 0.5 × 0.9 = 0.45 effective；demand 0.6（50 × 0.012/tick）；
    // pressure = 0.6 - 0.45 = 0.15（仍在 LOW 阈值 0.3 以下 → None）。
    let mut app = build_pressure_app(0.5);
    spawn_high_cost_planted(&mut app, 50);
    step_one_lingtian_tick(&mut app);
    let tracker = app.world().resource::<ZonePressureTracker>();
    let p = tracker.state(DEFAULT_ZONE).unwrap().last_pressure;
    assert!(
        (p - 0.15).abs() < 1e-3,
        "summer natural_supply offset 应当 0.15，实际 {p}"
    );
    assert_eq!(tracker.state(DEFAULT_ZONE).unwrap().last_level, PL::None);
}

#[test]
fn natural_supply_offsets_demand_in_winter_extra_supply() {
    // plan-lingtian-weather-v1 §2 — Winter natural_supply +10%：
    // base 0.5 × 1.1 = 0.55 effective；demand 0.6；pressure = 0.05。
    let mut app = build_pressure_app_with_season(0.5, Season::Winter);
    spawn_high_cost_planted(&mut app, 50);
    step_one_lingtian_tick(&mut app);
    let tracker = app.world().resource::<ZonePressureTracker>();
    let p = tracker.state(DEFAULT_ZONE).unwrap().last_pressure;
    assert!(
        (p - 0.05).abs() < 1e-3,
        "winter natural_supply offset 应当 0.05，实际 {p}"
    );
}

#[test]
fn haze_active_relaxes_pressure_classification_by_one_tier() {
    // plan-lingtian-weather-v1 §5 / worldview §七 — 阴霾期间天道注视减弱，
    // 阈值降 1 档。
    // setup：冬季（Blizzard / Haze 可触发 zone），HeavyHaze active；
    // 灌满 100 个 high_cost 作物（demand = 100 × 0.012 = 1.2 → 原 raw=1.2 → HIGH）
    // 但 haze 阈值降 1 档 → 应被分类为 Mid。
    let mut app = build_pressure_app_with_season(0.0, Season::Winter);
    // 注入 HeavyHaze active weather（覆盖 ActiveWeather 默认空状态）
    let mut active_weather = crate::lingtian::weather::ActiveWeather::new();
    active_weather.insert(
        DEFAULT_ZONE,
        crate::lingtian::weather::WeatherEvent::HeavyHaze,
        0,      // started_at
        10_000, // expires_at（远期，本测期间不清）
    );
    app.insert_resource(active_weather);

    spawn_high_cost_planted(&mut app, 100); // demand 1.2 → raw HIGH
    step_one_lingtian_tick(&mut app);
    let tracker = app.world().resource::<ZonePressureTracker>();
    let s = tracker.state(DEFAULT_ZONE).unwrap();
    // raw pressure 应该 ≈ 1.2（natural_supply=0、winter +10% 不影响 0）
    assert!(
        (s.last_pressure - 1.2).abs() < 1e-3,
        "raw pressure ≈ 1.2，实际 {}",
        s.last_pressure
    );
    // 但 classified 档位应当被降为 Mid（1 档）而非 High
    assert_eq!(
        s.last_level,
        PL::Mid,
        "阴霾期间 raw=1.2 应被降为 Mid，实际 {:?}",
        s.last_level
    );
}

#[test]
fn non_default_zone_pressure_uses_its_own_weather() {
    let mut app = build_pressure_app_with_season(0.0, Season::Winter);
    let mut active_weather = crate::lingtian::weather::ActiveWeather::new();
    active_weather.insert(
        "blood_valley",
        crate::lingtian::weather::WeatherEvent::HeavyHaze,
        0,
        10_000,
    );
    app.insert_resource(active_weather);

    spawn_high_cost_planted_in_zone(&mut app, 100, "blood_valley");
    step_one_lingtian_tick(&mut app);

    let tracker = app.world().resource::<ZonePressureTracker>();
    let s = tracker.state("blood_valley").unwrap();
    assert!(
        (s.last_pressure - 1.2).abs() < 1e-3,
        "blood_valley raw pressure ≈1.2，实际 {}",
        s.last_pressure
    );
    assert_eq!(
        s.last_level,
        PL::Mid,
        "blood_valley 的 HeavyHaze 应把 raw HIGH 降为 Mid，实际 {:?}",
        s.last_level
    );
    assert!(
        tracker.state(DEFAULT_ZONE).is_none()
            || tracker.state(DEFAULT_ZONE).unwrap().last_level == PL::None,
        "非默认区压力不应写入 default"
    );
}

#[test]
fn no_haze_no_relax_pressure_classified_normally() {
    // 对照：相同 raw pressure 但无 haze → classified 仍为 High。
    let mut app = build_pressure_app_with_season(0.0, Season::Winter);
    spawn_high_cost_planted(&mut app, 100);
    step_one_lingtian_tick(&mut app);
    let tracker = app.world().resource::<ZonePressureTracker>();
    let s = tracker.state(DEFAULT_ZONE).unwrap();
    assert!((s.last_pressure - 1.2).abs() < 1e-3);
    assert_eq!(
        s.last_level,
        PL::High,
        "无阴霾时 raw=1.2 应当 High，实际 {:?}",
        s.last_level
    );
}

#[test]
fn replenish_recent_7d_offsets_demand() {
    let mut app = build_pressure_app(0.0);
    spawn_high_cost_planted(&mut app, 50); // demand 0.6 → MID
    app.world_mut()
        .resource_mut::<ZonePressureTracker>()
        .state_mut(DEFAULT_ZONE)
        .record_replenish(0, 0.5);
    step_one_lingtian_tick(&mut app);
    let tracker = app.world().resource::<ZonePressureTracker>();
    assert_eq!(tracker.state(DEFAULT_ZONE).unwrap().last_level, PL::None);
}

#[test]
fn replenish_pressure_record_uses_plot_zone() {
    let mut app = build_pressure_app(0.0);
    let pos = BlockPos::new(12, 64, 12);
    spawn_empty_plot_in_zone(&mut app, pos, "lingquan_marsh");
    let player = app.world_mut().spawn_empty().id();

    app.world_mut().send_event(ReplenishCompleted {
        player,
        pos,
        source: ReplenishSource::BoneCoin,
        plot_qi_added: 0.8,
        overflow_to_zone: 0.2,
    });
    app.update();

    let tracker = app.world().resource::<ZonePressureTracker>();
    assert!(
        (tracker
            .state("lingquan_marsh")
            .unwrap()
            .replenish_total_7d()
            - 1.0)
            .abs()
            < 1e-6,
        "补灵压力应记录到 lingquan_marsh"
    );
    assert!(
        tracker.state(DEFAULT_ZONE).is_none()
            || tracker
                .state(DEFAULT_ZONE)
                .unwrap()
                .replenish_total_7d()
                .abs()
                < 1e-6,
        "非默认区补灵压力不应串到 default"
    );
}

#[test]
fn no_duplicate_event_when_pressure_stays_at_same_level() {
    let mut app = build_pressure_app(0.0);
    spawn_high_cost_planted(&mut app, 26); // LOW (>= 0.30 with f32 margin)
    step_one_lingtian_tick(&mut app);
    let evts1 = collect_pressure_events(&mut app);
    assert_eq!(evts1.len(), 1);
    step_one_lingtian_tick(&mut app);
    let evts2 = collect_pressure_events(&mut app);
    assert!(evts2.is_empty(), "档位未上升不该重复发");
}

#[test]
fn thresholds_match_plan_constants() {
    assert!((PRESSURE_LOW - 0.3).abs() < 1e-6);
    assert!((PRESSURE_MID - 0.6).abs() < 1e-6);
    assert!((PRESSURE_HIGH - 1.0).abs() < 1e-6);
}

// ------------------------------------------------------------------------
// §1.7 偷菜匿名记账 e2e
// ------------------------------------------------------------------------

use crate::cultivation::life_record::{BiographyEntry as BE, LifeRecord};

fn count_biography_matching<F: Fn(&BE) -> bool>(lr: &LifeRecord, f: F) -> usize {
    lr.biography.iter().filter(|e| f(e)).count()
}

/// build_harvest_app 已有；本 helper 在它基础上同时挂 LifeRecord 给 owner / operator
fn spawn_player_with_lifelog(app: &mut App, character_id: &str, target: BlockPos) -> Entity {
    let inv = empty_inventory_8x8();
    let lr = LifeRecord::new(character_id);
    valid_test_player(app, (inv, lr), target)
}

fn spawn_owned_ripe_plot(
    app: &mut App,
    plant_id: &str,
    pos: BlockPos,
    owner: Option<Entity>,
) -> Entity {
    let mut p = LingtianPlot::new(pos, owner);
    let mut crop = CropInstance::new(plant_id.into());
    crop.growth = 1.0;
    p.crop = Some(crop);
    app.world_mut().spawn(p).id()
}

#[test]
fn self_harvest_records_no_steal_entries() {
    let mut app = build_harvest_app();
    let pos = BlockPos::new(0, 64, 0);
    let player = spawn_player_with_lifelog(&mut app, "alice", pos);
    spawn_owned_ripe_plot(&mut app, "ci_she_hao", pos, Some(player));
    app.world_mut().send_event(StartHarvestRequest {
        player,
        pos,
        mode: SessionMode::Manual,
    });
    for _ in 0..HARVEST_MANUAL_TICKS {
        app.update();
    }
    let lr = app.world().get::<LifeRecord>(player).unwrap();
    assert_eq!(
        count_biography_matching(lr, |e| matches!(
            e,
            BE::PlotHarvestedByOther { .. } | BE::PlotHarvestedFromOther { .. }
        )),
        0,
        "自家收不应记偷菜条目"
    );
}

#[test]
fn stolen_harvest_records_both_sides() {
    let mut app = build_harvest_app();
    let pos = BlockPos::new(3, 64, 7);
    let owner = spawn_player_with_lifelog(&mut app, "alice", pos);
    let thief = spawn_player_with_lifelog(&mut app, "bob", pos);
    spawn_owned_ripe_plot(&mut app, "ning_mai_cao", pos, Some(owner));
    app.world_mut().send_event(StartHarvestRequest {
        player: thief,
        pos,
        mode: SessionMode::Manual,
    });
    for _ in 0..HARVEST_MANUAL_TICKS {
        app.update();
    }
    let owner_lr = app.world().get::<LifeRecord>(owner).unwrap();
    let thief_lr = app.world().get::<LifeRecord>(thief).unwrap();

    assert_eq!(
        count_biography_matching(owner_lr, |e| matches!(
            e,
            BE::PlotHarvestedByOther {
                plant_id, plot_pos, ..
            } if plant_id == "ning_mai_cao" && plot_pos == &[3, 64, 7]
        )),
        1,
        "owner 应记一条 PlotHarvestedByOther"
    );
    assert_eq!(
        count_biography_matching(thief_lr, |e| matches!(
            e,
            BE::PlotHarvestedFromOther {
                plant_id, plot_pos, ..
            } if plant_id == "ning_mai_cao" && plot_pos == &[3, 64, 7]
        )),
        1,
        "operator 应记一条 PlotHarvestedFromOther"
    );
}

#[test]
fn drain_qi_steals_into_player_and_zone_with_lifelog() {
    use crate::cultivation::components::Cultivation;
    use crate::lingtian::session::DRAIN_QI_TICKS;
    let mut app = build_app();
    let owner = app
        .world_mut()
        .spawn((empty_inventory_8x8(), LifeRecord::new("alice")))
        .id();
    let thief_cult = Cultivation {
        qi_current: 0.0,
        qi_max: 100.0,
        ..Default::default()
    };
    let pos = BlockPos::new(0, 64, 0);
    let thief = valid_test_player(
        &mut app,
        (empty_inventory_8x8(), LifeRecord::new("bob"), thief_cult),
        pos,
    );
    let mut p = LingtianPlot::new(pos, Some(owner));
    p.plot_qi = 0.5;
    let plot = app.world_mut().spawn(p).id();

    let zone_before = app.world().resource::<ZoneQiAccount>().get(DEFAULT_ZONE);
    app.world_mut()
        .send_event(StartDrainQiRequest { player: thief, pos });
    for _ in 0..DRAIN_QI_TICKS {
        app.update();
    }

    let p = app.world().get::<LingtianPlot>(plot).unwrap();
    assert!(p.plot_qi.abs() < 1e-6, "偷后 plot_qi 清零");

    let cult = app.world().get::<Cultivation>(thief).unwrap();
    assert!(
        (cult.qi_current - 0.4).abs() < 1e-5,
        "thief.qi_current={}",
        cult.qi_current
    );

    let zone_after = app.world().resource::<ZoneQiAccount>().get(DEFAULT_ZONE);
    assert!(
        (zone_after - zone_before - 0.1).abs() < 1e-5,
        "zone qi delta={}",
        zone_after - zone_before
    );
    let qi_transfers: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<QiTransfer>>()
        .drain()
        .collect();
    assert_eq!(
        qi_transfers.len(),
        2,
        "偷灵应写 player + zone 两笔 ledger event"
    );
    let plot_account =
        QiAccountId::container(format!("lingtian_plot:{},{},{}", pos.x, pos.y, pos.z));
    assert_eq!(qi_transfers[0].from, plot_account);
    assert_eq!(qi_transfers[0].to, QiAccountId::player("bob"));
    assert!((qi_transfers[0].amount - 0.4).abs() < 1e-6);
    assert_eq!(qi_transfers[0].reason, QiTransferReason::Channeling);
    assert_eq!(qi_transfers[1].from, plot_account);
    assert_eq!(qi_transfers[1].to, QiAccountId::zone(DEFAULT_ZONE));
    assert!((qi_transfers[1].amount - 0.1).abs() < 1e-6);
    assert_eq!(qi_transfers[1].reason, QiTransferReason::ReleaseToZone);

    let owner_lr = app.world().get::<LifeRecord>(owner).unwrap();
    let thief_lr = app.world().get::<LifeRecord>(thief).unwrap();
    assert_eq!(
        count_biography_matching(owner_lr, |e| matches!(e, BE::PlotQiDrainedByOther { .. })),
        1
    );
    assert_eq!(
        count_biography_matching(thief_lr, |e| matches!(e, BE::PlotQiDrainedFromOther { .. })),
        1
    );
}

#[test]
fn drain_qi_releases_to_non_default_plot_zone_and_ledger() {
    use crate::cultivation::components::Cultivation;
    use crate::lingtian::session::DRAIN_QI_TICKS;
    let mut app = build_app();
    app.world_mut()
        .resource_mut::<ZoneQiAccount>()
        .set(DEFAULT_ZONE, 0.0);
    app.world_mut()
        .resource_mut::<ZoneQiAccount>()
        .set("north_wastes", 3.0);
    let pos = BlockPos::new(4, 64, 4);
    let thief = valid_test_player(
        &mut app,
        (
            empty_inventory_8x8(),
            LifeRecord::new("bob"),
            Cultivation {
                qi_current: 0.0,
                qi_max: 100.0,
                ..Default::default()
            },
        ),
        pos,
    );
    let mut plot = LingtianPlot::new(pos, None).with_zone("north_wastes");
    plot.plot_qi = 0.5;
    app.world_mut().spawn(plot);

    app.world_mut()
        .send_event(StartDrainQiRequest { player: thief, pos });
    for _ in 0..DRAIN_QI_TICKS {
        app.update();
    }

    let accounts = app.world().resource::<ZoneQiAccount>();
    assert!(
        (accounts.get("north_wastes") - 3.1).abs() < 1e-5,
        "偷灵 20% 散逸应回流 north_wastes，实际 {}",
        accounts.get("north_wastes")
    );
    assert_eq!(accounts.get(DEFAULT_ZONE), 0.0, "default 不应收到散逸回流");

    let qi_transfers: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<QiTransfer>>()
        .drain()
        .collect();
    assert_eq!(
        qi_transfers.len(),
        2,
        "应写 player + north_wastes 两笔 ledger"
    );
    assert_eq!(qi_transfers[1].to, QiAccountId::zone("north_wastes"));
    assert_eq!(qi_transfers[1].reason, QiTransferReason::ReleaseToZone);
    assert!((qi_transfers[1].amount - 0.1).abs() < 1e-6);
}

#[test]
fn drain_qi_caps_at_qi_max() {
    use crate::cultivation::components::Cultivation;
    use crate::lingtian::session::DRAIN_QI_TICKS;
    let mut app = build_app();
    // plot_qi=5.0 → drained 5.0 → to_player 4.0；qi_current=99 / qi_max=100 余 1
    // → 注 1.0 → cap 100
    let pos = BlockPos::new(0, 64, 0);
    let mut p = LingtianPlot::new(pos, None);
    p.plot_qi_cap = 5.0;
    p.plot_qi = 5.0;
    app.world_mut().spawn(p);
    let zone_before = app.world().resource::<ZoneQiAccount>().get(DEFAULT_ZONE);
    let cult = Cultivation {
        qi_current: 99.0,
        qi_max: 100.0,
        ..Default::default()
    };
    let player = valid_test_player(
        &mut app,
        (empty_inventory_8x8(), LifeRecord::new("p"), cult),
        pos,
    );
    app.world_mut()
        .send_event(StartDrainQiRequest { player, pos });
    for _ in 0..DRAIN_QI_TICKS {
        app.update();
    }
    let cult = app.world().get::<Cultivation>(player).unwrap();
    assert!(
        (cult.qi_current - 100.0).abs() < 1e-5,
        "应封顶 qi_max=100, 实得 {}",
        cult.qi_current
    );
    let zone_after = app.world().resource::<ZoneQiAccount>().get(DEFAULT_ZONE);
    assert!(
        (zone_after - zone_before - 4.0).abs() < 1e-5,
        "玩家 cap 溢出应回流 zone, delta={}",
        zone_after - zone_before
    );
    let qi_transfers: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<QiTransfer>>()
        .drain()
        .collect();
    assert_eq!(qi_transfers.len(), 2);
    let plot_account =
        QiAccountId::container(format!("lingtian_plot:{},{},{}", pos.x, pos.y, pos.z));
    assert_eq!(qi_transfers[0].from, plot_account);
    assert_eq!(qi_transfers[0].to, QiAccountId::player("p"));
    assert!((qi_transfers[0].amount - 1.0).abs() < 1e-6);
    assert_eq!(qi_transfers[0].reason, QiTransferReason::Channeling);
    assert_eq!(qi_transfers[1].from, plot_account);
    assert_eq!(qi_transfers[1].to, QiAccountId::zone(DEFAULT_ZONE));
    assert!((qi_transfers[1].amount - 4.0).abs() < 1e-6);
    assert_eq!(qi_transfers[1].reason, QiTransferReason::ReleaseToZone);
}

#[test]
fn drain_qi_without_life_record_still_credits_cultivation_but_skips_player_ledger() {
    use crate::cultivation::components::Cultivation;
    use crate::lingtian::session::DRAIN_QI_TICKS;
    let mut app = build_app();
    let pos = BlockPos::new(0, 64, 0);
    let mut plot = LingtianPlot::new(pos, None);
    plot.plot_qi = 0.5;
    app.world_mut().spawn(plot);
    let player = valid_test_player(
        &mut app,
        (
            empty_inventory_8x8(),
            Cultivation {
                qi_current: 0.0,
                qi_max: 100.0,
                ..Default::default()
            },
        ),
        pos,
    );

    app.world_mut()
        .send_event(StartDrainQiRequest { player, pos });
    for _ in 0..DRAIN_QI_TICKS {
        app.update();
    }

    let cult = app.world().get::<Cultivation>(player).unwrap();
    assert!(
        (cult.qi_current - 0.4).abs() < 1e-5,
        "缺 LifeRecord 不应阻止 Cultivation 实际增长, got {}",
        cult.qi_current
    );
    let qi_transfers: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<QiTransfer>>()
        .drain()
        .collect();
    assert_eq!(qi_transfers.len(), 1, "缺稳定玩家账户时只写 zone 回流账");
    let plot_account =
        QiAccountId::container(format!("lingtian_plot:{},{},{}", pos.x, pos.y, pos.z));
    assert_eq!(qi_transfers[0].from, plot_account);
    assert_eq!(qi_transfers[0].to, QiAccountId::zone(DEFAULT_ZONE));
    assert!((qi_transfers[0].amount - 0.1).abs() < 1e-6);
    assert_eq!(qi_transfers[0].reason, QiTransferReason::ReleaseToZone);
}

#[test]
fn drain_qi_rejected_on_empty_plot() {
    let mut app = build_app();
    let pos = BlockPos::new(0, 64, 0);
    let player = valid_test_player(&mut app, (empty_inventory_8x8(), LifeRecord::new("p")), pos);
    let mut p = LingtianPlot::new(pos, None);
    p.plot_qi = 0.0;
    app.world_mut().spawn(p);
    app.world_mut()
        .send_event(StartDrainQiRequest { player, pos });
    app.update();
    assert!(app.world().resource::<ActiveLingtianSessions>().is_empty());
}

#[test]
fn ownerless_harvest_records_neither_side() {
    let mut app = build_harvest_app();
    let pos = BlockPos::new(0, 64, 0);
    let player = spawn_player_with_lifelog(&mut app, "wanderer", pos);
    spawn_owned_ripe_plot(&mut app, "ci_she_hao", pos, None); // 无主田
    app.world_mut().send_event(StartHarvestRequest {
        player,
        pos,
        mode: SessionMode::Manual,
    });
    for _ in 0..HARVEST_MANUAL_TICKS {
        app.update();
    }
    let lr = app.world().get::<LifeRecord>(player).unwrap();
    assert_eq!(
        count_biography_matching(lr, |e| matches!(
            e,
            BE::PlotHarvestedByOther { .. } | BE::PlotHarvestedFromOther { .. }
        )),
        0
    );
}

fn queue_test_request(app: &mut App, action: &str, player: Entity, pos: BlockPos) {
    let request = match action {
        "till" => PendingLingtianRequest::Till {
            actor: player,
            pos,
            hoe_instance_id: 1,
            mode: SessionMode::Manual,
        },
        "renew" => PendingLingtianRequest::Renew {
            actor: player,
            pos,
            hoe_instance_id: 1,
        },
        "planting" => PendingLingtianRequest::Planting {
            actor: player,
            pos,
            plant_id: "ci_she_hao".into(),
        },
        "harvest" => PendingLingtianRequest::Harvest {
            actor: player,
            pos,
            mode: SessionMode::Manual,
        },
        "replenish" => PendingLingtianRequest::Replenish {
            actor: player,
            pos,
            source: ReplenishSource::BoneCoin,
        },
        "drain_qi" => PendingLingtianRequest::DrainQi { actor: player, pos },
        _ => unreachable!(),
    };
    app.world_mut()
        .resource_mut::<PendingLingtianRequests>()
        .push(request);
}

#[test]
fn validator_preserves_cross_action_fifo_per_actor() {
    let mut app = App::new();
    app.init_resource::<PendingLingtianRequests>()
        .add_event::<StartTillRequest>()
        .add_event::<StartRenewRequest>()
        .add_event::<StartPlantingRequest>()
        .add_event::<StartHarvestRequest>()
        .add_event::<StartReplenishRequest>()
        .add_event::<StartDrainQiRequest>();
    app.add_systems(Update, validate_and_dispatch_lingtian_requests);
    // spawn bundle 与 Position 分开插入：bundle 自带 Position，同 tuple spawn 会
    // 撞重复组件 panic（insert 语义是替换所以分开插安全）
    let actor = spawn_test_player(&mut app, ());
    let pos = BlockPos::new(0, 64, 0);
    queue_test_request(&mut app, "till", actor, pos);
    queue_test_request(&mut app, "renew", actor, pos);
    queue_test_request(&mut app, "planting", actor, pos);
    queue_test_request(&mut app, "harvest", actor, pos);
    queue_test_request(&mut app, "replenish", actor, pos);
    queue_test_request(&mut app, "drain_qi", actor, pos);

    // central review 1984-31332727941 finding [5]：先前只验证前两条，后四条被
    // 重排（如 DrainQi 抢在 Planting 前）仍会通过。这里把六种 action 全部推进
    // 到底，每 tick 必须恰好 dispatch 入队顺序对应的那一种类型，且队列每 tick
    // 精确收缩 1 条（6→0）。任何乱序 dispatch 都会在它应出队的那一轮返回 0。
    // central review 1984-31447628937 finding [1]：每轮把**全部六种**事件资源
    // 排空计数——不只断言本轮应 dispatch 的那种恰好 1 条，还断言其余五种为 0。
    // 只检查应 dispatch 资源会把「正确弹出队列请求、但每 tick 额外多发一种
    // 事件」的坏实现放行（发错类型的多余事件从不落在被检查的资源上，或落在
    // 上一轮已被排空、此后不再检查的资源上）；排空全部资源使跨类型重复/多发
    // 在发生的当轮立刻暴露。
    let expected_order = [
        "till",
        "renew",
        "planting",
        "harvest",
        "replenish",
        "drain_qi",
    ];
    for (turn, action) in expected_order.into_iter().enumerate() {
        app.update();
        let dispatched_counts = [
            (
                "till",
                app.world_mut()
                    .resource_mut::<Events<StartTillRequest>>()
                    .drain()
                    .count(),
            ),
            (
                "renew",
                app.world_mut()
                    .resource_mut::<Events<StartRenewRequest>>()
                    .drain()
                    .count(),
            ),
            (
                "planting",
                app.world_mut()
                    .resource_mut::<Events<StartPlantingRequest>>()
                    .drain()
                    .count(),
            ),
            (
                "harvest",
                app.world_mut()
                    .resource_mut::<Events<StartHarvestRequest>>()
                    .drain()
                    .count(),
            ),
            (
                "replenish",
                app.world_mut()
                    .resource_mut::<Events<StartReplenishRequest>>()
                    .drain()
                    .count(),
            ),
            (
                "drain_qi",
                app.world_mut()
                    .resource_mut::<Events<StartDrainQiRequest>>()
                    .drain()
                    .count(),
            ),
        ];
        for (name, count) in dispatched_counts {
            if name == action {
                assert_eq!(
                    count,
                    1,
                    "update {}: exactly one `{action}` must dispatch (per-actor FIFO), got {count}",
                    turn + 1
                );
            } else {
                assert_eq!(
                    count,
                    0,
                    "update {}: `{name}` must NOT dispatch on the `{action}` tick \
                         (exactly one action per tick), got {count}",
                    turn + 1
                );
            }
        }
        assert_eq!(
            app.world().resource::<PendingLingtianRequests>().len(),
            6 - turn - 1,
            "update {}: queue must shrink by one per tick (FIFO)",
            turn + 1
        );
    }
}

#[test]
fn start_handlers_index_plots_only_for_matching_event_ticks() {
    let mut app = App::new();
    app.insert_resource(ActiveLingtianSessions::new())
        .insert_resource(SeedRegistry::new())
        .insert_resource(ZoneQiAccount::new())
        .insert_resource(LingtianClock::default())
        .insert_resource(StartHandlerPlotScanCount::default())
        .add_event::<StartTillRequest>()
        .add_event::<StartRenewRequest>()
        .add_event::<StartPlantingRequest>()
        .add_event::<StartHarvestRequest>()
        .add_event::<StartReplenishRequest>()
        .add_event::<StartDrainQiRequest>()
        .add_systems(
            Update,
            (
                handle_start_till,
                handle_start_renew,
                handle_start_planting,
                handle_start_harvest,
                handle_start_replenish,
                handle_start_drain_qi,
            )
                .chain(),
        );

    let plot_count = 7;
    for x in 0..plot_count {
        let mut plot = LingtianPlot::new(BlockPos::new(x, 64, 0), None);
        plot.plot_qi = 0.5;
        app.world_mut().spawn(plot);
    }

    app.update();
    app.update();
    let idle_count = app.world().resource::<StartHandlerPlotScanCount>();
    assert_eq!(
        (idle_count.index_builds, idle_count.scanned_plots),
        (0, 0),
        "six idle start handlers must not build an index or scan any plot"
    );

    app.world_mut().send_event(StartDrainQiRequest {
        player: Entity::from_raw(100),
        pos: BlockPos::new(0, 64, 0),
    });
    app.world_mut().send_event(StartDrainQiRequest {
        player: Entity::from_raw(101),
        pos: BlockPos::new(1, 64, 0),
    });
    app.update();
    let event_count = app.world().resource::<StartHandlerPlotScanCount>();
    assert_eq!(
        event_count.index_builds, 1,
        "only the handler with matching events may build a plot index on this tick"
    );
    assert_eq!(
        event_count.scanned_plots, plot_count as usize,
        "one event batch must scan each plot once, not once per request or idle handler"
    );

    app.update();
    let next_idle_count = app.world().resource::<StartHandlerPlotScanCount>();
    assert_eq!(
        (next_idle_count.index_builds, next_idle_count.scanned_plots),
        (1, plot_count as usize),
        "after the batch is consumed, the next idle tick must not scan or re-index plots"
    );
}

#[test]
fn every_start_handler_fails_closed_for_wrong_or_missing_authority_components() {
    for (action_index, action) in [
        "till",
        "renew",
        "planting",
        "harvest",
        "replenish",
        "drain_qi",
    ]
    .into_iter()
    .enumerate()
    {
        for (denial_index, denial) in ["wrong_dimension", "missing_position", "missing_dimension"]
            .into_iter()
            .enumerate()
        {
            let pos = BlockPos::new(
                10_000 + action_index as i32 * 10 + denial_index as i32,
                64,
                0,
            );
            let mut app = build_planting_app();
            let inventory = match action {
                "till" | "renew" => make_inventory_with_hoe(HoeKind::Iron, 1.0),
                "planting" => make_inventory_with_seed("ci_she_hao_seed", 2),
                "replenish" => {
                    let mut inventory = empty_inventory_8x8();
                    inventory.bone_coins = 2;
                    inventory
                }
                _ => empty_inventory_8x8(),
            };
            let player = valid_test_player(
                &mut app,
                (
                    inventory,
                    Cultivation::default(),
                    LifeRecord::new("authority"),
                ),
                pos,
            );
            match action {
                "renew" => {
                    let mut plot = LingtianPlot::new(pos, Some(player));
                    plot.harvest_count = crate::lingtian::plot::N_RENEW;
                    app.world_mut().spawn(plot);
                }
                "planting" => {
                    app.world_mut().spawn(LingtianPlot::new(pos, Some(player)));
                }
                "harvest" => {
                    spawn_ripe_plot(&mut app, "ci_she_hao", pos);
                }
                "replenish" => {
                    app.world_mut().spawn(LingtianPlot::new(pos, Some(player)));
                }
                "drain_qi" => {
                    let mut plot = LingtianPlot::new(pos, None);
                    plot.plot_qi = 0.5;
                    app.world_mut().spawn(plot);
                }
                _ => {}
            }
            let expected_reason = match denial {
                "wrong_dimension" => {
                    app.world_mut().entity_mut(player).insert(CurrentDimension(
                        crate::world::dimension::DimensionKind::Tsy,
                    ));
                    crate::lingtian::range_gate::LingtianInteractionDenial::WrongDimension
                }
                "missing_position" => {
                    app.world_mut().entity_mut(player).remove::<Position>();
                    crate::lingtian::range_gate::LingtianInteractionDenial::MissingPosition
                }
                "missing_dimension" => {
                    app.world_mut()
                        .entity_mut(player)
                        .remove::<CurrentDimension>();
                    crate::lingtian::range_gate::LingtianInteractionDenial::MissingDimension
                }
                _ => unreachable!(),
            };
            queue_test_request(&mut app, action, player, pos);
            app.update();
            assert!(
                crate::lingtian::range_gate::denial_was_logged(player, pos, expected_reason,),
                "{action} {denial} must execute the interaction gate denial path"
            );
            assert!(
                    app.world().resource::<ActiveLingtianSessions>().is_empty(),
                    "{action} must fail closed for {denial} even when all action prerequisites are valid"
                );
        }
    }
}

fn move_test_player_out_of_range(app: &mut App, player: Entity, target: BlockPos) {
    app.world_mut()
        .entity_mut(player)
        .insert(Position(DVec3::new(
            f64::from(target.x) + 20.5,
            f64::from(target.y) + 0.5,
            f64::from(target.z) + 0.5,
        )));
}

fn run_until_session_finishes(app: &mut App, ticks: u32) {
    for _ in 0..ticks {
        app.update();
    }
    assert!(
        app.world().resource::<ActiveLingtianSessions>().is_empty(),
        "finished session must leave the active-session table even when completion is denied"
    );
}

#[test]
fn all_start_handlers_reject_out_of_range_without_mutating_targets_or_inventory() {
    let far = Position(DVec3::new(20.5, 64.5, 0.5));
    let overworld = CurrentDimension(crate::world::dimension::DimensionKind::Overworld);

    let mut till = build_app();
    let till_player = spawn_test_player(&mut till, make_inventory_with_hoe(HoeKind::Iron, 1.0));
    till.world_mut().entity_mut(till_player).insert(far);
    queue_test_request(&mut till, "till", till_player, BlockPos::new(0, 64, 0));
    till.update();
    assert!(till.world().resource::<ActiveLingtianSessions>().is_empty());
    assert_eq!(
        till.world()
            .get::<PlayerInventory>(till_player)
            .unwrap()
            .equipped[MAIN_HAND_SLOT]
            .held
            .as_ref()
            .unwrap()
            .durability,
        1.0,
        "remote till must not wear the hoe"
    );
    assert_eq!(
        till.world_mut()
            .query::<&LingtianPlot>()
            .iter(till.world())
            .count(),
        0,
        "remote till must not create a plot"
    );

    let mut renew = build_app();
    let renew_pos = BlockPos::new(0, 64, 0);
    let renew_player = valid_test_player(
        &mut renew,
        make_inventory_with_hoe(HoeKind::Iron, 1.0),
        renew_pos,
    );
    renew.world_mut().entity_mut(renew_player).insert(far);
    let mut barren = LingtianPlot::new(renew_pos, Some(renew_player));
    barren.harvest_count = crate::lingtian::plot::N_RENEW;
    let renew_plot = renew.world_mut().spawn(barren).id();
    queue_test_request(&mut renew, "renew", renew_player, renew_pos);
    renew.update();
    assert!(renew
        .world()
        .resource::<ActiveLingtianSessions>()
        .is_empty());
    assert!(renew
        .world()
        .get::<LingtianPlot>(renew_plot)
        .unwrap()
        .is_barren());

    let mut planting = build_planting_app();
    let planting_pos = BlockPos::new(0, 64, 0);
    let planting_player = valid_test_player(
        &mut planting,
        make_inventory_with_seed("ci_she_hao_seed", 2),
        planting_pos,
    );
    planting.world_mut().entity_mut(planting_player).insert(far);
    let planting_plot = planting
        .world_mut()
        .spawn(LingtianPlot::new(planting_pos, Some(planting_player)))
        .id();
    queue_test_request(&mut planting, "planting", planting_player, planting_pos);
    planting.update();
    assert!(planting
        .world()
        .resource::<ActiveLingtianSessions>()
        .is_empty());
    assert!(planting
        .world()
        .get::<LingtianPlot>(planting_plot)
        .unwrap()
        .crop
        .is_none());
    assert_eq!(
        planting
            .world()
            .get::<PlayerInventory>(planting_player)
            .unwrap()
            .containers[0]
            .items[0]
            .instance
            .stack_count,
        2,
        "remote planting must not consume seed"
    );

    let mut harvest = build_harvest_app();
    let harvest_pos = BlockPos::new(0, 64, 0);
    let harvest_player = valid_test_player(&mut harvest, empty_inventory_8x8(), harvest_pos);
    harvest.world_mut().entity_mut(harvest_player).insert(far);
    let harvest_plot = spawn_ripe_plot(&mut harvest, "ci_she_hao", harvest_pos);
    queue_test_request(&mut harvest, "harvest", harvest_player, harvest_pos);
    harvest.update();
    assert!(harvest
        .world()
        .resource::<ActiveLingtianSessions>()
        .is_empty());
    assert!(harvest
        .world()
        .get::<LingtianPlot>(harvest_plot)
        .unwrap()
        .crop
        .as_ref()
        .is_some_and(|crop| crop.is_ripe()));

    let mut replenish = build_app();
    let replenish_pos = BlockPos::new(0, 64, 0);
    let mut replenish_inventory = empty_inventory_8x8();
    replenish_inventory.bone_coins = 2;
    let replenish_player = valid_test_player(&mut replenish, replenish_inventory, replenish_pos);
    replenish
        .world_mut()
        .entity_mut(replenish_player)
        .insert(far);
    let replenish_plot = replenish
        .world_mut()
        .spawn(LingtianPlot::new(replenish_pos, Some(replenish_player)))
        .id();
    queue_test_request(&mut replenish, "replenish", replenish_player, replenish_pos);
    replenish.update();
    assert!(replenish
        .world()
        .resource::<ActiveLingtianSessions>()
        .is_empty());
    assert_eq!(
        replenish
            .world()
            .get::<PlayerInventory>(replenish_player)
            .unwrap()
            .bone_coins,
        2,
        "remote replenish must not consume material"
    );
    assert_eq!(
        replenish
            .world()
            .get::<LingtianPlot>(replenish_plot)
            .unwrap()
            .plot_qi,
        0.0
    );

    let mut drain = build_app();
    let drain_pos = BlockPos::new(0, 64, 0);
    let drain_player = valid_test_player(
        &mut drain,
        (
            empty_inventory_8x8(),
            Cultivation::default(),
            LifeRecord::new("remote"),
        ),
        drain_pos,
    );
    drain.world_mut().entity_mut(drain_player).insert(far);
    drain.world_mut().entity_mut(drain_player).insert(overworld);
    let mut qi_plot = LingtianPlot::new(drain_pos, None);
    qi_plot.plot_qi = 0.5;
    let drain_plot = drain.world_mut().spawn(qi_plot).id();
    queue_test_request(&mut drain, "drain_qi", drain_player, drain_pos);
    drain.update();
    assert!(drain
        .world()
        .resource::<ActiveLingtianSessions>()
        .is_empty());
    assert_eq!(
        drain
            .world()
            .get::<LingtianPlot>(drain_plot)
            .unwrap()
            .plot_qi,
        0.5
    );
    assert_eq!(
        drain
            .world()
            .get::<Cultivation>(drain_player)
            .unwrap()
            .qi_current,
        0.0
    );
}

#[test]
fn all_player_completion_paths_revalidate_range_before_side_effects() {
    let pos = BlockPos::new(0, 64, 0);

    let mut till = build_app();
    let till_player =
        valid_test_player(&mut till, make_inventory_with_hoe(HoeKind::Iron, 1.0), pos);
    till.world_mut().send_event(StartTillRequest {
        player: till_player,
        pos,
        hoe_instance_id: 1,
        mode: SessionMode::Manual,
        terrain: TerrainKind::Grass,
        environment: PlotEnvironment::base(),
    });
    till.update();
    move_test_player_out_of_range(&mut till, till_player, pos);
    run_until_session_finishes(&mut till, TILL_MANUAL_TICKS - 1);
    assert_eq!(
        till.world_mut()
            .query::<&LingtianPlot>()
            .iter(till.world())
            .count(),
        0
    );
    assert_eq!(
        till.world()
            .get::<PlayerInventory>(till_player)
            .unwrap()
            .equipped[MAIN_HAND_SLOT]
            .held
            .as_ref()
            .unwrap()
            .durability,
        1.0
    );
    assert_eq!(
        till.world_mut()
            .resource_mut::<Events<TillCompleted>>()
            .drain()
            .count(),
        0
    );

    let mut renew = build_app();
    let renew_player =
        valid_test_player(&mut renew, make_inventory_with_hoe(HoeKind::Iron, 1.0), pos);
    let mut barren = LingtianPlot::new(pos, Some(renew_player));
    barren.harvest_count = crate::lingtian::plot::N_RENEW;
    let renew_plot = renew.world_mut().spawn(barren).id();
    renew.world_mut().send_event(StartRenewRequest {
        player: renew_player,
        pos,
        hoe_instance_id: 1,
    });
    renew.update();
    move_test_player_out_of_range(&mut renew, renew_player, pos);
    run_until_session_finishes(&mut renew, RENEW_TICKS - 1);
    assert!(renew
        .world()
        .get::<LingtianPlot>(renew_plot)
        .unwrap()
        .is_barren());
    assert_eq!(
        renew
            .world_mut()
            .resource_mut::<Events<RenewCompleted>>()
            .drain()
            .count(),
        0
    );

    let mut planting = build_planting_app();
    let planting_player = valid_test_player(
        &mut planting,
        make_inventory_with_seed("ci_she_hao_seed", 2),
        pos,
    );
    let planting_plot = planting
        .world_mut()
        .spawn(LingtianPlot::new(pos, Some(planting_player)))
        .id();
    planting.world_mut().send_event(StartPlantingRequest {
        player: planting_player,
        pos,
        plant_id: "ci_she_hao".into(),
    });
    planting.update();
    move_test_player_out_of_range(&mut planting, planting_player, pos);
    run_until_session_finishes(&mut planting, PLANTING_TICKS - 1);
    assert!(planting
        .world()
        .get::<LingtianPlot>(planting_plot)
        .unwrap()
        .crop
        .is_none());
    assert_eq!(
        planting
            .world()
            .get::<PlayerInventory>(planting_player)
            .unwrap()
            .containers[0]
            .items[0]
            .instance
            .stack_count,
        2
    );
    assert_eq!(
        planting
            .world_mut()
            .resource_mut::<Events<PlantingCompleted>>()
            .drain()
            .count(),
        0
    );

    let mut harvest = build_harvest_app();
    let harvest_player = valid_test_player(&mut harvest, empty_inventory_8x8(), pos);
    let harvest_plot = spawn_ripe_plot(&mut harvest, "ci_she_hao", pos);
    harvest.world_mut().send_event(StartHarvestRequest {
        player: harvest_player,
        pos,
        mode: SessionMode::Manual,
    });
    harvest.update();
    move_test_player_out_of_range(&mut harvest, harvest_player, pos);
    run_until_session_finishes(&mut harvest, HARVEST_MANUAL_TICKS - 1);
    let plot = harvest.world().get::<LingtianPlot>(harvest_plot).unwrap();
    assert!(plot.crop.as_ref().is_some_and(|crop| crop.is_ripe()));
    assert_eq!(plot.harvest_count, 0);
    assert_eq!(
        count_in_main_pack(
            harvest
                .world()
                .get::<PlayerInventory>(harvest_player)
                .unwrap(),
            "ci_she_hao"
        ),
        0
    );
    assert_eq!(
        harvest
            .world_mut()
            .resource_mut::<Events<HarvestCompleted>>()
            .drain()
            .count(),
        0
    );

    let mut replenish = build_app();
    let mut inventory = empty_inventory_8x8();
    inventory.bone_coins = 2;
    let replenish_player = valid_test_player(&mut replenish, inventory, pos);
    let replenish_plot = replenish
        .world_mut()
        .spawn(LingtianPlot::new(pos, Some(replenish_player)))
        .id();
    replenish.world_mut().send_event(StartReplenishRequest {
        player: replenish_player,
        pos,
        source: ReplenishSource::BoneCoin,
    });
    replenish.update();
    move_test_player_out_of_range(&mut replenish, replenish_player, pos);
    run_until_session_finishes(
        &mut replenish,
        ReplenishSource::BoneCoin.duration_ticks() - 1,
    );
    assert_eq!(
        replenish
            .world()
            .get::<PlayerInventory>(replenish_player)
            .unwrap()
            .bone_coins,
        2
    );
    assert_eq!(
        replenish
            .world()
            .get::<LingtianPlot>(replenish_plot)
            .unwrap()
            .plot_qi,
        0.0
    );
    assert_eq!(
        replenish
            .world_mut()
            .resource_mut::<Events<ReplenishCompleted>>()
            .drain()
            .count(),
        0
    );

    let mut drain = build_app();
    let drain_player = valid_test_player(
        &mut drain,
        (
            empty_inventory_8x8(),
            Cultivation::default(),
            LifeRecord::new("completion"),
        ),
        pos,
    );
    let mut qi_plot = LingtianPlot::new(pos, None);
    qi_plot.plot_qi = 0.5;
    let drain_plot = drain.world_mut().spawn(qi_plot).id();
    drain.world_mut().send_event(StartDrainQiRequest {
        player: drain_player,
        pos,
    });
    drain.update();
    move_test_player_out_of_range(&mut drain, drain_player, pos);
    run_until_session_finishes(&mut drain, DRAIN_QI_TICKS - 1);
    assert_eq!(
        drain
            .world()
            .get::<LingtianPlot>(drain_plot)
            .unwrap()
            .plot_qi,
        0.5
    );
    assert_eq!(
        drain
            .world()
            .get::<Cultivation>(drain_player)
            .unwrap()
            .qi_current,
        0.0
    );
    assert_eq!(
        drain
            .world_mut()
            .resource_mut::<Events<QiTransfer>>()
            .drain()
            .count(),
        0
    );
    assert_eq!(
        drain
            .world_mut()
            .resource_mut::<Events<DrainQiCompleted>>()
            .drain()
            .count(),
        0
    );
}

#[test]
fn completion_gate_rejects_wrong_or_missing_player_authority_components() {
    let pos = BlockPos::new(0, 64, 0);
    for denial in ["wrong_dimension", "missing_position", "missing_dimension"] {
        let mut app = build_app();
        let player = valid_test_player(&mut app, make_inventory_with_hoe(HoeKind::Iron, 1.0), pos);
        app.world_mut().send_event(StartTillRequest {
            player,
            pos,
            hoe_instance_id: 1,
            mode: SessionMode::Manual,
            terrain: TerrainKind::Grass,
            environment: PlotEnvironment::base(),
        });
        app.update();
        match denial {
            "wrong_dimension" => {
                app.world_mut().entity_mut(player).insert(CurrentDimension(
                    crate::world::dimension::DimensionKind::Tsy,
                ));
            }
            "missing_position" => {
                app.world_mut().entity_mut(player).remove::<Position>();
            }
            "missing_dimension" => {
                app.world_mut()
                    .entity_mut(player)
                    .remove::<CurrentDimension>();
            }
            _ => unreachable!(),
        }
        run_until_session_finishes(&mut app, TILL_MANUAL_TICKS - 1);
        assert_eq!(
            app.world_mut()
                .query::<&LingtianPlot>()
                .iter(app.world())
                .count(),
            0,
            "{denial} must reject before till creates a plot"
        );
        assert_eq!(
            app.world_mut()
                .resource_mut::<Events<TillCompleted>>()
                .drain()
                .count(),
            0,
            "{denial} must not emit TillCompleted"
        );
    }
}

#[test]
fn same_tick_dimension_transfer_precedes_start_validation() {
    let mut app = build_app();
    let overworld = app.world_mut().spawn(OverworldLayer).id();
    let tsy = app.world_mut().spawn(TsyLayer).id();
    app.insert_resource(DimensionLayers { overworld, tsy });
    app.add_event::<DimensionTransferRequest>();
    app.add_systems(
        Update,
        apply_dimension_transfers
            .in_set(DimensionTransferSet)
            .in_set(crate::world::movement_commit::AuthoritativePositionCommitSet),
    );

    let pos = BlockPos::new(0, 64, 0);
    let player = valid_test_player(&mut app, make_inventory_with_hoe(HoeKind::Iron, 1.0), pos);
    let mut visible_layers = VisibleEntityLayers::default();
    visible_layers.0.insert(tsy);
    app.world_mut().entity_mut(player).insert((
        CurrentDimension(DimensionKind::Tsy),
        Position(DVec3::new(100.5, 80.5, 100.5)),
        EntityLayerId(tsy),
        VisibleChunkLayer(tsy),
        visible_layers,
    ));
    app.world_mut().send_event(DimensionTransferRequest {
        entity: player,
        target: DimensionKind::Overworld,
        target_pos: DVec3::new(0.5, 64.5, 0.5),
    });
    app.world_mut()
        .resource_mut::<crate::lingtian::requests::PendingLingtianRequests>()
        .push(PendingLingtianRequest::Till {
            actor: player,
            pos,
            hoe_instance_id: 1,
            mode: SessionMode::Manual,
        });

    app.update();

    assert_eq!(
        app.world().get::<CurrentDimension>(player),
        Some(&CurrentDimension(DimensionKind::Overworld)),
        "same-tick transfer must be applied before start authority is read"
    );
    let start_events = app.world().resource::<Events<StartTillRequest>>();
    assert_eq!(
        start_events.get_reader().read(start_events).count(),
        1,
        "post-transfer gate must dispatch the real pending request on the first update"
    );
    assert!(
        app.world()
            .resource::<crate::lingtian::requests::PendingLingtianRequests>()
            .is_empty(),
        "accepted request must leave the persistent ingress queue"
    );
}

#[test]
fn same_tick_dimension_transfer_precedes_completion_revalidation() {
    let mut app = build_app();
    let overworld = app.world_mut().spawn(OverworldLayer).id();
    let tsy = app.world_mut().spawn(TsyLayer).id();
    app.insert_resource(DimensionLayers { overworld, tsy });
    app.add_event::<DimensionTransferRequest>();
    app.add_systems(
        Update,
        apply_dimension_transfers
            .in_set(DimensionTransferSet)
            .in_set(crate::world::movement_commit::AuthoritativePositionCommitSet),
    );

    let pos = BlockPos::new(0, 64, 0);
    let player = valid_test_player(&mut app, make_inventory_with_hoe(HoeKind::Iron, 1.0), pos);
    let mut visible_layers = VisibleEntityLayers::default();
    visible_layers.0.insert(overworld);
    app.world_mut().entity_mut(player).insert((
        EntityLayerId(overworld),
        VisibleChunkLayer(overworld),
        visible_layers,
    ));
    app.world_mut().send_event(StartTillRequest {
        player,
        pos,
        hoe_instance_id: 1,
        mode: SessionMode::Manual,
        terrain: TerrainKind::Grass,
        environment: PlotEnvironment::base(),
    });
    app.update();
    for _ in 0..TILL_MANUAL_TICKS - 2 {
        app.update();
    }

    app.world_mut().send_event(DimensionTransferRequest {
        entity: player,
        target: DimensionKind::Tsy,
        target_pos: DVec3::new(0.5, 80.5, 0.5),
    });
    app.update();

    assert_eq!(
        app.world().get::<CurrentDimension>(player),
        Some(&CurrentDimension(DimensionKind::Tsy)),
        "same-tick transfer must be applied before completion authority is read"
    );
    assert_eq!(
        app.world_mut()
            .query::<&LingtianPlot>()
            .iter(app.world())
            .count(),
        0,
        "transferred player must not create a plot on the finishing tick"
    );
    assert_eq!(
        app.world().get::<PlayerInventory>(player).unwrap().equipped[MAIN_HAND_SLOT]
            .held
            .as_ref()
            .unwrap()
            .durability,
        1.0,
        "transferred player must not wear the hoe on denied completion"
    );
    assert_eq!(
        app.world_mut()
            .resource_mut::<Events<TillCompleted>>()
            .drain()
            .count(),
        0,
        "transferred player must not emit TillCompleted"
    );
}

fn finished_session(mut session: ActiveSession) -> ActiveSession {
    while !session.is_finished() {
        session.tick();
    }
    session
}

#[test]
fn till_reservation_is_shared_exclusive_and_released_on_cancel_and_settlement() {
    let actor_a = Entity::from_raw(1);
    let actor_b = Entity::from_raw(2);
    let pos = BlockPos::new(5, 64, 5);
    let session = TillSession::new(
        pos,
        HoeKind::Iron,
        11,
        SessionMode::Manual,
        PlotEnvironment::base(),
    );
    let mut sessions = ActiveLingtianSessions::new();

    assert!(sessions.try_insert_till(actor_a, session.clone(), false));
    assert!(!sessions.try_insert_till(actor_b, session.clone(), false));
    assert!(!sessions.try_insert_till(actor_b, session.clone(), true));
    assert_eq!(sessions.pending_reservations(), 1);

    sessions.clear(actor_a);
    assert_eq!(sessions.pending_reservations(), 0);
    let finished = finished_session(ActiveSession::Till(session));
    let ActiveSession::Till(finished) = finished else {
        unreachable!();
    };
    assert!(sessions.try_insert_till(actor_b, finished, false));

    let drained = sessions.drain_finished();
    assert_eq!(drained.len(), 1);
    assert_eq!(sessions.pending_reservations(), 1);
    sessions.settle_reservations();
    assert_eq!(sessions.pending_reservations(), 0);
}

#[test]
fn npc_finished_sessions_settle_all_direct_farming_variants() {
    let pos = BlockPos::new(0, 64, 0);

    let mut till_app = build_app();
    let till_npc = till_app.world_mut().spawn(NpcMarker).id();
    let ActiveSession::Till(finished_till) =
        finished_session(ActiveSession::Till(TillSession::new(
            pos,
            HoeKind::Iron,
            1,
            SessionMode::Manual,
            PlotEnvironment::base(),
        )))
    else {
        unreachable!();
    };
    till_app
        .world_mut()
        .resource_mut::<ActiveLingtianSessions>()
        .try_insert_till(till_npc, finished_till, false);
    till_app.update();
    assert_eq!(
        till_app
            .world_mut()
            .query::<&LingtianPlot>()
            .iter(till_app.world())
            .filter(|plot| plot.pos == pos)
            .count(),
        1,
        "live NPC Till completion must create the plot"
    );

    let plant_id: PlantId = "ci_she_hao".into();
    let mut planting_app = build_app();
    let plant_registry = registry_with_three_test_plants();
    planting_app.insert_resource(SeedRegistry::from_plant_registry(&plant_registry));
    planting_app.insert_resource(plant_registry);
    let planting_npc = planting_app.world_mut().spawn(NpcMarker).id();
    let planting_plot = planting_app
        .world_mut()
        .spawn(LingtianPlot::new(pos, None))
        .id();
    planting_app
        .world_mut()
        .resource_mut::<ActiveLingtianSessions>()
        .try_insert(
            planting_npc,
            finished_session(ActiveSession::Planting(PlantingSession::new(
                pos,
                plant_id.clone(),
            ))),
        );
    planting_app.update();
    assert_eq!(
        planting_app
            .world()
            .get::<LingtianPlot>(planting_plot)
            .unwrap()
            .crop
            .as_ref()
            .map(|crop| &crop.kind),
        Some(&plant_id),
        "live NPC Planting completion must populate the crop"
    );

    let mut harvest_app = build_app();
    harvest_app.insert_resource(registry_with_three_test_plants());
    harvest_app.insert_resource(registry_with_herb_and_seed_templates());
    let harvest_npc = harvest_app.world_mut().spawn(NpcMarker).id();
    let mut harvest_plot_value = LingtianPlot::new(pos, None);
    let mut crop = CropInstance::new(plant_id.clone());
    crop.growth = 1.0;
    harvest_plot_value.crop = Some(crop);
    let harvest_plot = harvest_app.world_mut().spawn(harvest_plot_value).id();
    harvest_app
        .world_mut()
        .resource_mut::<ActiveLingtianSessions>()
        .try_insert(
            harvest_npc,
            finished_session(ActiveSession::Harvest(HarvestSession::new(
                pos,
                plant_id.clone(),
                SessionMode::Auto,
            ))),
        );
    harvest_app.update();
    assert!(
        harvest_app
            .world()
            .get::<LingtianPlot>(harvest_plot)
            .unwrap()
            .crop
            .is_none(),
        "live NPC Harvest completion must clear the ripe crop"
    );

    let mut replenish_app = build_app();
    replenish_app
        .world_mut()
        .resource_mut::<ZoneQiAccount>()
        .set(DEFAULT_ZONE, 1.0);
    let replenish_npc = replenish_app.world_mut().spawn(NpcMarker).id();
    let replenish_plot = replenish_app
        .world_mut()
        .spawn(LingtianPlot::new(pos, None))
        .id();
    replenish_app
        .world_mut()
        .resource_mut::<ActiveLingtianSessions>()
        .try_insert(
            replenish_npc,
            finished_session(ActiveSession::Replenish(ReplenishSession::new(
                pos,
                ReplenishSource::Zone,
            ))),
        );
    replenish_app.update();
    assert_eq!(
        replenish_app
            .world()
            .get::<LingtianPlot>(replenish_plot)
            .unwrap()
            .plot_qi,
        ReplenishSource::Zone.plot_qi_amount(),
        "live NPC Replenish completion must deposit plot qi"
    );

    // fix-spec-1901-v2 §6.3 / OPEN-1 — NPC DrainQi 生产链不存在（farming
    // brain 只注册 Till/Plant/Harvest/Replenish/Migrate），不保留"测试
    // 手塞可达、生产永远不可达"的假链路；NPC 抽灵由未来独立 NPC plan
    // 定义 scorer/action/producer/qi ownership 合同。这里不再有
    // DrainQi 的 NPC completion fixture。
}

#[test]
fn despawned_npc_finished_session_is_discarded() {
    let mut app = build_app();
    let pos = BlockPos::new(0, 64, 0);
    let npc = app.world_mut().spawn((NpcMarker, Despawned)).id();
    let ActiveSession::Till(finished_till) =
        finished_session(ActiveSession::Till(TillSession::new(
            pos,
            HoeKind::Iron,
            1,
            SessionMode::Manual,
            PlotEnvironment::base(),
        )))
    else {
        unreachable!();
    };
    app.world_mut()
        .resource_mut::<ActiveLingtianSessions>()
        .try_insert_till(npc, finished_till, false);

    app.update();

    assert_eq!(
        app.world_mut()
            .query::<&LingtianPlot>()
            .iter(app.world())
            .count(),
        0,
        "Despawned NPC must not settle a finished farming session"
    );
    assert_eq!(
        app.world_mut()
            .resource_mut::<Events<TillCompleted>>()
            .drain()
            .count(),
        0,
        "Despawned NPC must not emit completion events"
    );
}
//
// Regression: the previous implementation used `Added<LingtianPlot>` and
// returned early when ZoneRegistry was missing. If a plot spawned on a
// frame where the registry hadn't been inserted yet, `Added` fired once,
// the system bailed, and the plot's zone field stayed empty forever —
// breaking later zone-keyed queries (e.g. daoshen spawn).

fn zone_named(name: &str, min: DVec3, max: DVec3) -> crate::world::zone::Zone {
    crate::world::zone::Zone {
        name: name.to_string(),
        dimension: crate::world::dimension::DimensionKind::Overworld,
        bounds: (min, max),
        spirit_qi: 1.0,
        danger_level: 0,
        active_events: Vec::new(),
        patrol_anchors: Vec::new(),
        blocked_tiles: Vec::new(),
        qi_equilibrium: 0.0,
        qi_inflow_per_min: 0.0,
    }
}

#[test]
fn auto_set_plot_zone_retries_when_registry_inserted_late() {
    use crate::lingtian::plot::LingtianPlot;
    use crate::world::zone::ZoneRegistry;

    let mut app = App::new();
    app.init_resource::<PendingPlotZones>();
    app.add_systems(Update, auto_set_plot_zone);

    // Spawn a plot WITHOUT a ZoneRegistry resource (simulates registry
    // not yet ready when worldgen-driven plot spawning happens).
    let plot_entity = app
        .world_mut()
        .spawn(LingtianPlot::new(BlockPos::new(50, 64, 50), None))
        .id();

    app.update();

    let plot = app.world().get::<LingtianPlot>(plot_entity).unwrap();
    assert!(
        plot.zone.is_empty(),
        "tick 1: no registry → zone must stay empty (got {:?})",
        plot.zone
    );

    // Now insert the registry at its normal revision-zero baseline. The
    // pending-entity cache must retry because the registry was previously
    // unobserved (`last_seen_spatial_revision = None`), NOT because the
    // revision differs — a fresh insert must not be conflated with an
    // already-observed revision 0. Previously this fixture had to fake
    // `spatial_revision: 1` to force the retry, masking the production bug.
    app.insert_resource(ZoneRegistry {
        zones: vec![zone_named(
            "spawn_zone",
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(100.0, 100.0, 100.0),
        )],
        spatial_revision: 0,
    });

    app.update();

    let plot = app.world().get::<LingtianPlot>(plot_entity).unwrap();
    assert_eq!(
        plot.zone, "spawn_zone",
        "tick 2: registry present → zone must be back-filled (got {:?})",
        plot.zone
    );
}

#[test]
fn auto_set_plot_zone_retries_when_existing_registry_changes() {
    let mut app = App::new();
    app.insert_resource(ZoneRegistry {
        zones: Vec::new(),
        spatial_revision: 0,
    });
    app.init_resource::<PendingPlotZones>();
    app.add_systems(Update, auto_set_plot_zone);

    let plot_entity = app
        .world_mut()
        .spawn(LingtianPlot::new(BlockPos::new(50, 64, 50), None))
        .id();
    app.update();
    assert!(
        app.world()
            .get::<LingtianPlot>(plot_entity)
            .unwrap()
            .zone
            .is_empty(),
        "empty existing registry must leave the plot pending"
    );

    app.world_mut()
        .resource_mut::<ZoneRegistry>()
        .zones
        .push(zone_named(
            "added_zone",
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(100.0, 100.0, 100.0),
        ));
    app.world_mut()
        .resource_mut::<ZoneRegistry>()
        .spatial_revision = 1;
    app.update();

    assert_eq!(
        app.world().get::<LingtianPlot>(plot_entity).unwrap().zone,
        "added_zone",
        "in-place ZoneRegistry mutation must retry unresolved plots"
    );
}

#[test]
fn auto_set_plot_zone_does_not_retry_history_for_each_new_plot() {
    let mut app = App::new();
    app.insert_resource(ZoneRegistry {
        zones: vec![zone_named(
            "registry_zone",
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(100.0, 100.0, 100.0),
        )],
        spatial_revision: 0,
    });
    app.init_resource::<PendingPlotZones>();
    app.add_systems(Update, auto_set_plot_zone);

    let unresolved = app
        .world_mut()
        .spawn(LingtianPlot::new(BlockPos::new(500, 64, 500), None))
        .id();
    app.update();
    app.world_mut()
        .get_mut::<LingtianPlot>(unresolved)
        .unwrap()
        .pos = BlockPos::new(50, 64, 50);

    let new_plot = app
        .world_mut()
        .spawn(LingtianPlot::new(BlockPos::new(60, 64, 60), None))
        .id();
    app.update();

    assert!(
        app.world()
            .get::<LingtianPlot>(unresolved)
            .unwrap()
            .zone
            .is_empty(),
        "a new plot must not trigger a rescan of historical unresolved entries"
    );
    assert_eq!(
        app.world().get::<LingtianPlot>(new_plot).unwrap().zone,
        "registry_zone",
        "the newly added plot must still resolve immediately"
    );
}

#[test]
fn auto_set_plot_zone_does_not_overwrite_existing_zone() {
    use crate::lingtian::plot::LingtianPlot;
    use crate::world::zone::ZoneRegistry;

    let mut app = App::new();
    app.insert_resource(ZoneRegistry {
        zones: vec![zone_named(
            "registry_zone",
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(100.0, 100.0, 100.0),
        )],
        spatial_revision: 0,
    });
    app.init_resource::<PendingPlotZones>();
    app.add_systems(Update, auto_set_plot_zone);

    // Pre-set plot zone — system must NOT overwrite it (idempotent).
    let plot_entity = app
        .world_mut()
        .spawn(LingtianPlot::new(BlockPos::new(50, 64, 50), None).with_zone("explicit_zone"))
        .id();

    app.update();
    app.update();

    let plot = app.world().get::<LingtianPlot>(plot_entity).unwrap();
    assert_eq!(
        plot.zone, "explicit_zone",
        "system must not overwrite a non-empty zone field"
    );
}
