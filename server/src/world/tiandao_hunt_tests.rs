#![allow(dead_code, unused_imports)]
use super::*;
use crate::combat::components::Wounds;
use crate::combat::events::{ApplyStatusEffectIntent, CombatEvent, DeathEvent};
use crate::combat::CombatClock;
use crate::cultivation::color::PracticeLog;
use crate::cultivation::components::{Contamination, Cultivation, MeridianSystem, QiColor, Realm};
use crate::cultivation::negative_zone::siphon_amount;
use crate::cultivation::tribulation::JueBiTriggerEvent;
use crate::inventory::{
    ContainerState, InventoryRevision, ItemInstance, ItemRarity, PlayerInventory,
    EQUIP_SLOT_MAIN_HAND, MAIN_PACK_CONTAINER_ID,
};
use crate::player::gameplay::PendingGameplayNarrations;
use crate::social::components::SpiritNiche;
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::season::Season;
use crate::world::zone::Zone;
use crate::zhenfa::{ZhenfaCarrierKind, ZhenfaKind, ZhenfaPlaceRequest, ZhenfaRegistry};
use valence::prelude::{ChunkLayer, DVec3, UnloadedChunk};
use valence::testing::{create_mock_client, ScenarioSingleClient};

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-9,
        "expected {expected}, got {actual}"
    );
}

fn input(realm: Realm) -> TiandaoAttentionInput {
    TiandaoAttentionInput {
        realm,
        zone_spirit_qi: 0.6,
        activity: TiandaoActivity::Standing,
        season: Season::Summer,
        is_dominant: false,
    }
}

fn sync_deceive_heaven_runtime(
    state: &mut DeceiveHeavenRuntimeState,
    deploy_event: Option<DeceiveHeavenEvent>,
    exposed_event: Option<DeceiveHeavenExposedEvent>,
) {
    let mut deploy_events = Events::default();
    let mut exposed_events = Events::default();
    let mut deploy_reader = bevy_ecs::event::ManualEventReader::default();
    let mut exposed_reader = bevy_ecs::event::ManualEventReader::default();

    if let Some(event) = deploy_event {
        deploy_events.send(event);
    }
    if let Some(event) = exposed_event {
        exposed_events.send(event);
    }

    state.record_events(
        Some(&deploy_events),
        Some(&exposed_events),
        &mut deploy_reader,
        &mut exposed_reader,
    );
}

fn deceive_heaven_deploy_event(
    array_id: u64,
    owner_player_id: &str,
    pos: [i32; 3],
    placed_at_tick: u64,
) -> DeceiveHeavenEvent {
    DeceiveHeavenEvent {
        owner: Entity::from_raw(1),
        owner_player_id: owner_player_id.to_string(),
        array_id,
        pos,
        self_weight_multiplier: 0.5,
        target_weight_multiplier: 1.5,
        reveal_chance: 0.10,
        placed_at_tick,
    }
}

fn deceive_heaven_exposed_event(
    array_id: u64,
    owner_player_id: &str,
    pos: [i32; 3],
    exposed_at_tick: u64,
) -> DeceiveHeavenExposedEvent {
    DeceiveHeavenExposedEvent {
        owner: Entity::from_raw(1),
        owner_player_id: owner_player_id.to_string(),
        array_id,
        pos,
        self_weight_multiplier: 0.5,
        target_weight_multiplier: 1.5,
        reveal_chance: 0.10,
        exposed_at_tick,
    }
}

fn negative_zone_escape_qi_cost_per_eval_for_tests(zone_spirit_qi: f64, qi_max: f64) -> f64 {
    siphon_amount(zone_spirit_qi, qi_max) * TIANDAO_HUNT_EVAL_INTERVAL_TICKS as f64
}

fn tiandao_runtime_app() -> (App, Entity) {
    let mut app = App::new();
    app.add_event::<DeceiveHeavenEvent>();
    app.add_event::<DeceiveHeavenExposedEvent>();
    app.insert_resource(CultivationClock { tick: 0 });
    app.init_resource::<DeceiveHeavenRuntimeState>();
    app.init_resource::<TiandaoActivityRuntimeState>();
    app.add_systems(Update, tiandao_hunt_tick);

    let (mut bundle, _helper) = create_mock_client("Alice");
    bundle.player.position = Position::new([0.0, 64.0, 0.0]);
    let player = app
        .world_mut()
        .spawn((
            bundle,
            Cultivation {
                realm: Realm::Awaken,
                qi_current: 100.0,
                qi_max: 100.0,
                ..Cultivation::default()
            },
            TiandaoAttention {
                level: 20.0,
                response: TiandaoResponseLevel::Watch,
                ..TiandaoAttention::default()
            },
        ))
        .id();
    (app, player)
}

fn tiandao_zhenfa_production_app() -> (App, Entity) {
    let scenario = ScenarioSingleClient::new();
    let mut app = scenario.app;
    crate::world::dimension::mark_test_layer_as_overworld(&mut app);
    app.world_mut()
        .get_mut::<ChunkLayer>(scenario.layer)
        .expect("test layer should carry ChunkLayer")
        .insert_chunk([37, 0], UnloadedChunk::new());
    app.insert_resource(CultivationClock { tick: 0 });
    app.insert_resource(CombatClock::default());
    app.insert_resource(PendingGameplayNarrations::default());
    app.add_event::<JueBiTriggerEvent>();
    app.add_event::<CombatEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<ApplyStatusEffectIntent>();
    crate::zhenfa::register(&mut app);
    super::register(&mut app);

    let (mut bundle, _helper) = create_mock_client("Alice");
    bundle.player.position = Position::new([0.0, 64.0, 0.0]);
    let player = app
        .world_mut()
        .spawn((
            bundle,
            Cultivation {
                realm: Realm::Solidify,
                qi_current: 100.0,
                qi_max: 100.0,
                ..Cultivation::default()
            },
            QiColor::default(),
            PracticeLog::default(),
            Wounds::default(),
            Contamination::default(),
            MeridianSystem::default(),
            TiandaoAttention {
                level: 20.0,
                response: TiandaoResponseLevel::Watch,
                ..TiandaoAttention::default()
            },
            deceive_heaven_test_inventory(),
        ))
        .id();
    (app, player)
}

fn deceive_heaven_test_inventory() -> PlayerInventory {
    const ZHENFA_FLAG_ITEM_ID_FOR_TEST: &str = "array_flag";
    let mut inventory = PlayerInventory {
        material_preparation: Default::default(),
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
        bone_coins: 10,
        max_weight: 45.0,
    };
    inventory.equipped.insert(
        EQUIP_SLOT_MAIN_HAND.to_string(),
        crate::inventory::SlotContents::held_single(test_item(
            9200,
            ZHENFA_FLAG_ITEM_ID_FOR_TEST,
            1,
        )),
    );
    inventory.containers[0]
        .items
        .push(crate::inventory::PlacedItemState {
            row: 0,
            col: 0,
            instance: test_item(9201, "ling_mu_ban", 2),
        });
    inventory.containers[0]
        .items
        .push(crate::inventory::PlacedItemState {
            row: 0,
            col: 1,
            instance: test_item(9202, "yi_shou_gu", 4),
        });
    inventory
}

fn capture_production_deceive_heaven_exposure(
    app: &mut App,
    player: Entity,
) -> DeceiveHeavenExposedEvent {
    const MAX_EXPOSURE_CANDIDATES: i32 = 200;

    let mut requested_at_tick = 200;
    for offset in 0..MAX_EXPOSURE_CANDIDATES {
        let pos = [600 + offset, 64, 0];
        app.world_mut().resource_mut::<CultivationClock>().tick = requested_at_tick;
        app.world_mut().resource_mut::<CombatClock>().tick = requested_at_tick;
        app.world_mut()
            .entity_mut(player)
            .insert(deceive_heaven_test_inventory());
        app.world_mut()
            .get_mut::<Cultivation>(player)
            .unwrap()
            .qi_current = 100.0;
        app.world_mut().send_event(ZhenfaPlaceRequest {
            player,
            pos,
            kind: ZhenfaKind::DeceiveHeaven,
            carrier: ZhenfaCarrierKind::BeastCoreInlaid,
            qi_invest_ratio: 0.10,
            trigger: None,
            item_instance_id: None,
            target_face: None,
            requested_at_tick,
        });
        app.update();

        let instance = app
            .world()
            .resource::<ZhenfaRegistry>()
            .find_at(pos)
            .expect("production placement should create a deceive heaven instance")
            .clone();
        let probe_tick = instance.expires_at_tick.saturating_sub(1);
        {
            let mut attention = app.world_mut().get_mut::<TiandaoAttention>(player).unwrap();
            attention.level = 21.0;
            attention.response = TiandaoResponseLevel::Watch;
            attention.last_eval_tick = instance.placed_at_tick;
        }
        app.world_mut().resource_mut::<CultivationClock>().tick = probe_tick;
        app.world_mut().resource_mut::<CombatClock>().tick = probe_tick;
        app.update();

        let exposed = app
            .world()
            .resource::<Events<DeceiveHeavenExposedEvent>>()
            .iter_current_update_events()
            .find(|event| event.array_id == instance.id)
            .cloned();
        if let Some(event) = exposed {
            assert_eq!(event.owner_player_id, "offline:Alice");
            assert_eq!(event.pos, pos);
            assert_eq!(event.exposed_at_tick, probe_tick);
            return event;
        }

        requested_at_tick = instance.expires_at_tick.saturating_add(1);
    }

    panic!("production zhenfa path did not emit a deceive heaven exposure event");
}

fn test_item(instance_id: u64, template_id: &str, stack_count: u32) -> ItemInstance {
    ItemInstance {
        instance_id,
        template_id: template_id.to_string(),
        display_name: template_id.to_string(),
        grid_w: 1,
        grid_h: 1,
        weight: 0.1,
        rarity: ItemRarity::Common,
        description: template_id.to_string(),
        stack_count,
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

fn single_zone_registry(name: &str, spirit_qi: f64) -> ZoneRegistry {
    ZoneRegistry {
        spatial_revision: 0,
        zones: vec![Zone {
            name: name.to_string(),
            dimension: DimensionKind::Overworld,
            bounds: (DVec3::new(-16.0, 0.0, -16.0), DVec3::new(16.0, 128.0, 16.0)),
            spirit_qi,
            danger_level: 0,
            active_events: Vec::new(),
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
            qi_equilibrium: 0.0,
            qi_inflow_per_min: 0.0,
        }],
    }
}

fn evaluate_deceive_runtime(
    state: &mut DeceiveHeavenRuntimeState,
    player_pos: DVec3,
    now_tick: u64,
    start_level: f64,
) -> (TiandaoEvalSnapshot, TiandaoAttention) {
    let cultivation = Cultivation {
        realm: Realm::Awaken,
        ..Cultivation::default()
    };
    let position = Position(player_pos);
    let mut attention = TiandaoAttention {
        level: start_level,
        response: TiandaoResponseLevel::Watch,
        ..TiandaoAttention::default()
    };
    let countermeasures = state.countermeasure_input("offline:Alice", position.0, now_tick);
    let snapshot = apply_attention_eval(
        &cultivation,
        &position,
        &mut attention,
        TiandaoEvalContext {
            dimension: None,
            zones: None,
            season: Season::Summer,
            activity: TiandaoActivity::Standing,
            countermeasures,
            now_tick,
            is_dominant: false,
        },
    )
    .expect("ten-second tiandao_hunt eval should run");
    state.mark_countermeasure_applied(
        "offline:Alice",
        position.0,
        now_tick,
        snapshot.countermeasure,
    );

    (snapshot, attention)
}

fn narration_test_snapshot(response: TiandaoResponseLevel, level: f64) -> TiandaoEvalSnapshot {
    TiandaoEvalSnapshot {
        position: DVec3::new(1.2, 64.0, -3.4),
        zone_name: Some("spawn".to_string()),
        zone_spirit_qi: 0.6,
        realm: Realm::Spirit,
        activity: TiandaoActivity::Meditating,
        response,
        level,
        countermeasure: TiandaoCountermeasureOutcome::default(),
    }
}

fn narration_test_bridge() -> (
    RedisBridgeResource,
    crossbeam_channel::Receiver<RedisOutbound>,
) {
    let (tx_outbound, rx_outbound) = crossbeam_channel::unbounded();
    let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded();
    (
        RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        },
        rx_outbound,
    )
}

fn expect_tiandao_narration_request(
    rx: &crossbeam_channel::Receiver<RedisOutbound>,
) -> TiandaoHuntNarrationRequestV1 {
    match rx
        .try_recv()
        .expect("expected one tiandao narration outbound")
    {
        RedisOutbound::TiandaoHuntNarrationRequest(payload) => payload,
        other => panic!("expected TiandaoHuntNarrationRequest, got {other:?}"),
    }
}

fn narration_test_sinks(redis: &RedisBridgeResource) -> TiandaoResponseSinks<'_> {
    TiandaoResponseSinks {
        zones: None,
        active_events: None,
        vfx_events: None,
        audio_events: None,
        redis: Some(redis),
        qi_ledger: None,
    }
}

fn advance_for_minutes(
    attention: &mut TiandaoAttention,
    input: TiandaoAttentionInput,
    minutes: u64,
    eval_index: &mut u64,
) {
    for _ in 0..(minutes * 60 * 20 / TIANDAO_HUNT_EVAL_INTERVAL_TICKS) {
        *eval_index += 1;
        advance_attention(
            attention,
            input,
            *eval_index * TIANDAO_HUNT_EVAL_INTERVAL_TICKS,
        );
    }
}

fn emit_response_chain_for_test(
    attention: &mut TiandaoAttention,
    response: TiandaoResponseLevel,
    level: f64,
    now_tick: u64,
) -> (
    ActiveEventsResource,
    ZoneRegistry,
    Events<VfxEventRequest>,
    Events<PlaySoundRecipeRequest>,
    crossbeam_channel::Receiver<RedisOutbound>,
) {
    let (redis, rx) = narration_test_bridge();
    let mut active_events = ActiveEventsResource::default();
    let mut zones = single_zone_registry("spawn", 0.6);
    let mut vfx_events = Events::<VfxEventRequest>::default();
    let mut audio_events = Events::<PlaySoundRecipeRequest>::default();

    apply_tiandao_response_chain(
        attention,
        narration_test_snapshot(response, level),
        Entity::from_raw(42),
        "Alice",
        TiandaoResponseSinks {
            zones: Some(&mut zones),
            active_events: Some(&mut active_events),
            vfx_events: Some(&mut vfx_events),
            audio_events: Some(&mut audio_events),
            redis: Some(&redis),
            qi_ledger: None,
        },
        now_tick,
    );

    (active_events, zones, vfx_events, audio_events, rx)
}

fn qi_snapshot(zone_qi: f64, ledger: &WorldQiAccount) -> crate::qi_physics::WorldQiSnapshot {
    crate::qi_physics::WorldQiSnapshot {
        player_qi: 0.0,
        zone_qi,
        container_qi: 0.0,
        ledger_qi: ledger.total(),
        era_decay_accum: 0.0,
        budget_initial_total: crate::qi_physics::constants::DEFAULT_SPIRIT_QI_TOTAL,
        budget_current_total: crate::qi_physics::constants::DEFAULT_SPIRIT_QI_TOTAL,
    }
}

#[test]
fn runtime_activity_tracks_movement_since_last_eval() {
    let entity = Entity::from_raw(7);
    let mut state = TiandaoActivityRuntimeState::default();

    let first = state.activity_for_eval(TiandaoActivityRuntimeInput {
        entity,
        position: DVec3::new(0.0, 64.0, 0.0),
        combat: None,
        lifecycle: None,
        practice_accumulator: None,
        spirit_niches: None,
        now_tick: 200,
    });
    let second = state.activity_for_eval(TiandaoActivityRuntimeInput {
        entity,
        position: DVec3::new(1.0, 64.0, 0.0),
        combat: None,
        lifecycle: None,
        practice_accumulator: None,
        spirit_niches: None,
        now_tick: 400,
    });

    assert_eq!(first, TiandaoActivity::Standing);
    assert_eq!(second, TiandaoActivity::Moving);
}

#[test]
fn runtime_activity_uses_combat_before_meditation_or_movement() {
    let entity = Entity::from_raw(8);
    let mut state = TiandaoActivityRuntimeState::default();
    let mut practice = CultivationSessionPracticeAccumulator::default();
    practice.note_practice_tick_for_tests(entity, 198);
    let combat = CombatState {
        in_combat_until_tick: Some(300),
        ..CombatState::default()
    };

    let activity = state.activity_for_eval(TiandaoActivityRuntimeInput {
        entity,
        position: DVec3::new(4.0, 64.0, 0.0),
        combat: Some(&combat),
        lifecycle: None,
        practice_accumulator: Some(&practice),
        spirit_niches: None,
        now_tick: 200,
    });

    assert_eq!(activity, TiandaoActivity::Combat);
}

#[test]
fn runtime_activity_uses_own_active_niche_before_meditation() {
    let entity = Entity::from_raw(9);
    let mut state = TiandaoActivityRuntimeState::default();
    let mut practice = CultivationSessionPracticeAccumulator::default();
    practice.note_practice_tick_for_tests(entity, 198);
    let lifecycle = Lifecycle {
        character_id: "offline:Alice".to_string(),
        ..Lifecycle::default()
    };
    let mut spirit_niches = SpiritNicheRegistry::default();
    spirit_niches.upsert(SpiritNiche {
        owner: "offline:Alice".to_string(),
        pos: [10, 64, 10],
        placed_at_tick: 1,
        revealed: false,
        revealed_by: None,
        is_damaged: false,
        guardians: Vec::new(),
    });

    let activity = state.activity_for_eval(TiandaoActivityRuntimeInput {
        entity,
        position: DVec3::new(10.5, 64.5, 10.5),
        combat: None,
        lifecycle: Some(&lifecycle),
        practice_accumulator: Some(&practice),
        spirit_niches: Some(&spirit_niches),
        now_tick: 200,
    });

    assert_eq!(activity, TiandaoActivity::InNiche);
}

#[test]
fn low_realms_never_accumulate_attention() {
    for realm in [Realm::Awaken, Realm::Induce] {
        let mut attention = TiandaoAttention {
            level: 10.0,
            ..TiandaoAttention::default()
        };
        let mut eval_index = 0;
        advance_for_minutes(
            &mut attention,
            TiandaoAttentionInput {
                zone_spirit_qi: 0.9,
                activity: TiandaoActivity::Meditating,
                ..input(realm)
            },
            24 * 60,
            &mut eval_index,
        );
        assert_eq!(attention.level, 0.0);
        assert_eq!(attention.accumulation_rate, 0.0);
        assert_eq!(attention.response, TiandaoResponseLevel::None);
    }
}

#[test]
fn advance_attention_clamps_and_tracks_peak() {
    let mut attention = TiandaoAttention {
        level: 99.9,
        response: TiandaoResponseLevel::Annihilate,
        last_eval_tick: u64::MAX,
        accumulation_rate: 0.0,
        peak_level: 50.0,
        ..TiandaoAttention::default()
    };
    advance_attention(
        &mut attention,
        TiandaoAttentionInput {
            realm: Realm::Void,
            zone_spirit_qi: 1.0,
            activity: TiandaoActivity::Meditating,
            season: Season::SummerToWinter,
            is_dominant: false,
        },
        u64::MAX,
    );
    assert_eq!(attention.level, 100.0);
    assert_eq!(attention.peak_level, 100.0);
    assert_eq!(attention.last_eval_tick, u64::MAX);
    assert_eq!(attention.response, TiandaoResponseLevel::Annihilate);
}

#[test]
fn decay_never_underflows_attention() {
    let mut attention = TiandaoAttention {
        level: 0.01,
        response: TiandaoResponseLevel::None,
        ..TiandaoAttention::default()
    };
    advance_attention(
        &mut attention,
        TiandaoAttentionInput {
            realm: Realm::Awaken,
            zone_spirit_qi: -0.5,
            activity: TiandaoActivity::Standing,
            season: Season::Summer,
            is_dominant: false,
        },
        200,
    );
    assert_eq!(attention.level, 0.0);
}

#[test]
fn none_stage_allows_attention_to_build_before_watch() {
    let input = TiandaoAttentionInput {
        realm: Realm::Condense,
        zone_spirit_qi: 0.9,
        activity: TiandaoActivity::Meditating,
        season: Season::Summer,
        is_dominant: false,
    };
    let mut attention = TiandaoAttention::default();
    advance_attention(&mut attention, input, 200);

    assert!(
        attention.level > 0.0,
        "凝脉以上在高灵气打坐时必须能从 None 建立注意力，否则 P4 曲线永远到不了 Watch"
    );
    assert_eq!(
        attention_decay_for_eval(TiandaoResponseLevel::None, input, accumulation_rate(input)),
        0.0
    );
}

#[test]
fn condense_high_qi_meditation_reaches_watch_then_moving_escapes_to_none() {
    let mut attention = TiandaoAttention::default();
    let mut eval_index = 0;
    let high_qi_meditation = TiandaoAttentionInput {
        realm: Realm::Condense,
        zone_spirit_qi: 0.9,
        activity: TiandaoActivity::Meditating,
        season: Season::Summer,
        is_dominant: false,
    };
    advance_for_minutes(&mut attention, high_qi_meditation, 250, &mut eval_index);
    assert_eq!(attention.response, TiandaoResponseLevel::Watch);
    assert!(
        attention.level >= 10.0 && attention.level < 15.0,
        "凝脉高灵气打坐应只擦到 Watch 边缘并受滞后保护，actual={}",
        attention.level
    );

    advance_for_minutes(
        &mut attention,
        TiandaoAttentionInput {
            activity: TiandaoActivity::Moving,
            ..high_qi_meditation
        },
        30,
        &mut eval_index,
    );
    assert_eq!(attention.response, TiandaoResponseLevel::None);
    assert!(
        attention.level < 10.0,
        "凝脉 Watch 边缘跑路 30 分钟应脱离天道注视，actual={}",
        attention.level
    );
}

#[test]
fn solidify_attention_reaches_watch_pressure_then_moving_downgrades() {
    let mut attention = TiandaoAttention::default();
    let mut eval_index = 0;
    let standing = input(Realm::Solidify);
    advance_for_minutes(&mut attention, standing, 50, &mut eval_index);
    assert_eq!(attention.response, TiandaoResponseLevel::Watch);
    assert!(
        attention.level >= 15.0 && attention.level < 40.0,
        "固元 50 分钟应进入 Watch 但未到 Pressure，actual={}",
        attention.level
    );

    let high_qi_meditation = TiandaoAttentionInput {
        realm: Realm::Solidify,
        zone_spirit_qi: 0.9,
        activity: TiandaoActivity::Meditating,
        season: Season::Summer,
        is_dominant: false,
    };
    advance_for_minutes(&mut attention, high_qi_meditation, 70, &mut eval_index);
    assert_eq!(attention.response, TiandaoResponseLevel::Pressure);

    let before_move = attention.level;
    advance_for_minutes(
        &mut attention,
        TiandaoAttentionInput {
            activity: TiandaoActivity::Moving,
            ..standing
        },
        15,
        &mut eval_index,
    );
    assert!(
        attention.level < before_move,
        "固元移动应降低 Pressure 注意力，before={before_move} after={}",
        attention.level
    );
    assert!(
        matches!(
            attention.response,
            TiandaoResponseLevel::Pressure | TiandaoResponseLevel::Watch
        ),
        "固元移动后只能保持 Pressure 或降回 Watch，actual={:?}",
        attention.response
    );
}

#[test]
fn spirit_attention_reaches_watch_pressure_tribulation_in_order() {
    let mut attention = TiandaoAttention::default();
    let mut eval_index = 0;
    let standing = input(Realm::Spirit);

    advance_for_minutes(&mut attention, standing, 17, &mut eval_index);
    assert_eq!(attention.response, TiandaoResponseLevel::Watch);

    advance_for_minutes(&mut attention, standing, 73, &mut eval_index);
    assert_eq!(attention.response, TiandaoResponseLevel::Pressure);

    advance_for_minutes(&mut attention, standing, 30, &mut eval_index);
    assert_eq!(attention.response, TiandaoResponseLevel::Tribulation);
    assert!(
        attention.peak_level >= 70.0,
        "通灵曲线必须按 Watch→Pressure→Tribulation 升级，peak={}",
        attention.peak_level
    );
}

#[test]
fn void_attention_reaches_watch_tribulation_annihilate_in_order() {
    let mut attention = TiandaoAttention::default();
    let mut eval_index = 0;
    let standing = input(Realm::Void);

    advance_for_minutes(&mut attention, standing, 7, &mut eval_index);
    assert_eq!(attention.response, TiandaoResponseLevel::Watch);

    advance_for_minutes(&mut attention, standing, 31, &mut eval_index);
    assert_eq!(attention.response, TiandaoResponseLevel::Tribulation);

    advance_for_minutes(&mut attention, standing, 12, &mut eval_index);
    assert_eq!(attention.response, TiandaoResponseLevel::Annihilate);
    assert_eq!(
        decay_rate(TiandaoResponseLevel::Annihilate, standing.zone_spirit_qi),
        0.0,
        "Annihilate 级注意力不自然衰减"
    );
}

#[test]
fn deceive_heaven_deploy_event_enters_tiandao_hunt_runtime_and_decays_x4() {
    let mut state = DeceiveHeavenRuntimeState::default();
    sync_deceive_heaven_runtime(
        &mut state,
        Some(deceive_heaven_deploy_event(
            7,
            "offline:Alice",
            [600, 64, 0],
            100,
        )),
        None,
    );

    let (snapshot, attention) =
        evaluate_deceive_runtime(&mut state, DVec3::new(0.0, 64.0, 0.0), 300, 20.0);

    assert_eq!(
        snapshot.countermeasure.deceive_heaven,
        DeceiveHeavenOutcome::Diverted
    );
    assert_close(
        snapshot.countermeasure.decay_multiplier,
        DECEIVE_HEAVEN_DECOY_DECAY_MULTIPLIER,
    );
    assert_close(
        attention.level,
        20.0 - decay_rate(TiandaoResponseLevel::Watch, 0.0) * DECEIVE_HEAVEN_DECOY_DECAY_MULTIPLIER,
    );
}

#[test]
fn deceive_heaven_runtime_rejects_too_close_and_expired_deployments() {
    let mut too_close_state = DeceiveHeavenRuntimeState::default();
    sync_deceive_heaven_runtime(
        &mut too_close_state,
        Some(deceive_heaven_deploy_event(
            8,
            "offline:Alice",
            [499, 64, 0],
            100,
        )),
        None,
    );

    let (too_close, too_close_attention) =
        evaluate_deceive_runtime(&mut too_close_state, DVec3::new(0.0, 64.0, 0.0), 300, 20.0);

    assert_eq!(
        too_close.countermeasure.deceive_heaven,
        DeceiveHeavenOutcome::TooClose
    );
    assert_close(
        too_close_attention.level,
        20.0 - decay_rate(TiandaoResponseLevel::Watch, 0.0),
    );

    let mut expired_state = DeceiveHeavenRuntimeState::default();
    sync_deceive_heaven_runtime(
        &mut expired_state,
        Some(deceive_heaven_deploy_event(
            9,
            "offline:Alice",
            [600, 64, 0],
            100,
        )),
        None,
    );

    let (expired, expired_attention) = evaluate_deceive_runtime(
        &mut expired_state,
        DVec3::new(0.0, 64.0, 0.0),
        100 + DECEIVE_HEAVEN_DECOY_DURATION_TICKS,
        20.0,
    );

    assert_eq!(
        expired.countermeasure.deceive_heaven,
        DeceiveHeavenOutcome::Expired
    );
    assert_close(
        expired_attention.level,
        20.0 - decay_rate(TiandaoResponseLevel::Watch, 0.0),
    );
}

#[test]
fn deceive_heaven_exposed_event_adds_tiandao_hunt_attention_penalty() {
    let mut state = DeceiveHeavenRuntimeState::default();
    sync_deceive_heaven_runtime(
        &mut state,
        Some(deceive_heaven_deploy_event(
            10,
            "offline:Alice",
            [600, 64, 0],
            100,
        )),
        None,
    );
    sync_deceive_heaven_runtime(
        &mut state,
        None,
        Some(deceive_heaven_exposed_event(
            10,
            "offline:Alice",
            [600, 64, 0],
            250,
        )),
    );

    let (snapshot, attention) =
        evaluate_deceive_runtime(&mut state, DVec3::new(0.0, 64.0, 0.0), 300, 21.0);

    assert_eq!(
        snapshot.countermeasure.deceive_heaven,
        DeceiveHeavenOutcome::Revealed
    );
    assert_close(
        snapshot.countermeasure.attention_penalty,
        DECEIVE_HEAVEN_REVEAL_PENALTY,
    );
    assert_close(
        attention.level,
        21.0 - decay_rate(TiandaoResponseLevel::Watch, 0.0) + DECEIVE_HEAVEN_REVEAL_PENALTY,
    );
    assert_eq!(attention.response, TiandaoResponseLevel::Pressure);
}

#[test]
fn production_zhenfa_deceive_heaven_place_feeds_tiandao_hunt_same_update() {
    let (mut app, player) = tiandao_zhenfa_production_app();
    app.world_mut().send_event(ZhenfaPlaceRequest {
        player,
        pos: [600, 64, 0],
        kind: ZhenfaKind::DeceiveHeaven,
        carrier: ZhenfaCarrierKind::BeastCoreInlaid,
        qi_invest_ratio: 0.80,
        trigger: None,
        item_instance_id: None,
        target_face: None,
        requested_at_tick: 200,
    });
    app.world_mut().resource_mut::<CultivationClock>().tick = 200;

    app.update();

    let attention = app.world().get::<TiandaoAttention>(player).unwrap();
    assert_close(
        attention.level,
        20.0 + accumulation_rate(TiandaoAttentionInput {
            realm: Realm::Solidify,
            zone_spirit_qi: 0.0,
            activity: TiandaoActivity::Standing,
            season: Season::default(),
            is_dominant: false,
        }) - decay_rate(TiandaoResponseLevel::Watch, 0.0) * DECEIVE_HEAVEN_DECOY_DECAY_MULTIPLIER,
    );
    assert_eq!(attention.last_eval_tick, 200);
}

#[test]
fn production_zhenfa_exposure_feeds_tiandao_hunt_penalty_once() {
    let (mut app, player) = tiandao_zhenfa_production_app();
    let exposed = capture_production_deceive_heaven_exposure(&mut app, player);

    let first = app.world().get::<TiandaoAttention>(player).unwrap().clone();
    assert_eq!(first.last_eval_tick, exposed.exposed_at_tick);
    assert_close(
        first.level,
        21.0 + first.accumulation_rate - decay_rate(TiandaoResponseLevel::Watch, 0.0)
            + DECEIVE_HEAVEN_REVEAL_PENALTY,
    );
    assert_eq!(first.response, TiandaoResponseLevel::Pressure);

    let next_eval_tick = first
        .last_eval_tick
        .saturating_add(TIANDAO_HUNT_EVAL_INTERVAL_TICKS);
    app.world_mut().resource_mut::<CultivationClock>().tick = next_eval_tick;
    app.world_mut().resource_mut::<CombatClock>().tick = next_eval_tick;
    app.update();

    let second = app.world().get::<TiandaoAttention>(player).unwrap();
    assert_close(
        second.level,
        first.level + second.accumulation_rate - decay_rate(TiandaoResponseLevel::Pressure, 0.0),
    );
    assert_eq!(second.last_eval_tick, next_eval_tick);
}

#[test]
fn negative_zone_escape_applies_decay_x5_and_reports_existing_siphon_cost() {
    let mut attention = TiandaoAttention {
        level: 50.0,
        response: TiandaoResponseLevel::Pressure,
        ..TiandaoAttention::default()
    };
    advance_attention(
        &mut attention,
        TiandaoAttentionInput {
            realm: Realm::Awaken,
            zone_spirit_qi: -0.5,
            activity: TiandaoActivity::Standing,
            season: Season::Summer,
            is_dominant: false,
        },
        200,
    );
    assert_close(attention.level, 50.0 - 0.03 * 5.0);
    assert_close(
        negative_zone_escape_qi_cost_per_eval_for_tests(-0.5, 1000.0),
        siphon_amount(-0.5, 1000.0) * TIANDAO_HUNT_EVAL_INTERVAL_TICKS as f64,
    );
    assert!(
        negative_zone_escape_qi_cost_per_eval_for_tests(-0.5, 1000.0)
            > negative_zone_escape_qi_cost_per_eval_for_tests(-0.5, 100.0),
        "负灵域反制代价必须随 qi_max 放大，让高境承担更高真元消耗"
    );
}

#[test]
fn nomadic_meditation_cycle_does_not_enter_pressure() {
    let mut attention = TiandaoAttention {
        level: 30.0,
        response: TiandaoResponseLevel::Watch,
        peak_level: 30.0,
        ..TiandaoAttention::default()
    };
    let mut eval_index = 1;
    for _cycle in 0..4 {
        for _ in 0..60 {
            advance_attention(
                &mut attention,
                TiandaoAttentionInput {
                    activity: TiandaoActivity::Meditating,
                    ..input(Realm::Solidify)
                },
                eval_index * TIANDAO_HUNT_EVAL_INTERVAL_TICKS,
            );
            eval_index += 1;
        }
        for _ in 0..30 {
            advance_attention(
                &mut attention,
                TiandaoAttentionInput {
                    activity: TiandaoActivity::Moving,
                    ..input(Realm::Solidify)
                },
                eval_index * TIANDAO_HUNT_EVAL_INTERVAL_TICKS,
            );
            eval_index += 1;
        }
    }

    assert!(
        attention.level < 40.0,
        "固元 4 轮游牧打坐/转移应停在 Pressure 阈值下，actual={}",
        attention.level
    );
    assert_eq!(attention.response, TiandaoResponseLevel::Watch);
}

#[test]
fn void_high_qi_meditation_reaches_watch_in_plan_timeframe() {
    let mut attention = TiandaoAttention::default();
    let input = TiandaoAttentionInput {
        realm: Realm::Void,
        zone_spirit_qi: 0.9,
        activity: TiandaoActivity::Meditating,
        season: Season::Summer,
        is_dominant: false,
    };
    for _ in 0..28 {
        advance_attention(&mut attention, input, 200);
    }
    assert_eq!(attention.response, TiandaoResponseLevel::Watch);
    assert!(attention.level >= 15.0);
}

#[test]
fn tiandao_spawn_command_uses_target_and_attention_wire_value() {
    let command = tiandao_spawn_event_command(
        "spawn",
        EVENT_THUNDER_TRIBULATION,
        600,
        1.2,
        "player-1",
        TiandaoResponseLevel::Tribulation,
    );
    assert_eq!(command.command_type, CommandType::SpawnEvent);
    assert_eq!(command.target, "spawn");
    assert_eq!(
        command.params.get("event").and_then(|value| value.as_str()),
        Some(EVENT_THUNDER_TRIBULATION)
    );
    assert_eq!(
        command
            .params
            .get("target_player")
            .and_then(|value| value.as_str()),
        Some("player-1")
    );
    assert_eq!(
        command
            .params
            .get("attention_level")
            .and_then(|value| value.as_str()),
        Some("tribulation")
    );
    assert_eq!(
        command
            .params
            .get("reason")
            .and_then(|value| value.as_str()),
        Some("tiandao_hunt_p1")
    );
}

#[test]
fn pressure_response_enqueues_beast_tide_and_emits_multimodal_feedback() {
    let mut attention = TiandaoAttention::default();
    let (active_events, _zones, vfx_events, audio_events, rx) =
        emit_response_chain_for_test(&mut attention, TiandaoResponseLevel::Pressure, 45.0, 200);

    assert!(active_events.contains("spawn", EVENT_BEAST_TIDE));
    assert_eq!(
        active_events.count_by_zone_and_event("spawn", EVENT_BEAST_TIDE),
        1,
        "Pressure 响应必须只入队一次定向兽潮"
    );
    assert_eq!(vfx_events.iter_current_update_events().count(), 1);
    assert_eq!(audio_events.iter_current_update_events().count(), 1);
    let narration = expect_tiandao_narration_request(&rx);
    assert_eq!(
        narration.response_level,
        TiandaoHuntResponseLevelV1::Pressure
    );
}

#[test]
fn watch_response_drains_zone_qi_through_conserving_ledger_transfer() {
    let mut zones = single_zone_registry("spawn", 0.6);
    let mut ledger = WorldQiAccount::default();
    let before = qi_snapshot(0.6, &ledger);

    let transfer = apply_watch_zone_qi_drain(
        "spawn",
        tiandao_response_profile(TiandaoResponseLevel::Watch)
            .unwrap()
            .interval_ticks,
        &mut zones,
        &mut ledger,
    )
    .expect("Watch 级必须发出守恒 QiTransfer");

    assert_eq!(transfer.from, QiAccountId::zone("spawn"));
    assert_eq!(transfer.to, QiAccountId::tiandao());
    assert_eq!(transfer.reason, QiTransferReason::TiandaoWatchDrain);
    assert_close(transfer.amount, 0.05);
    assert_close(zones.find_zone_by_name("spawn").unwrap().spirit_qi, 0.55);
    assert_close(ledger.balance(&QiAccountId::zone("spawn")), 0.0);
    assert_close(ledger.balance(&QiAccountId::tiandao()), 0.05);
    crate::qi_physics::assert_conservation(
        &before,
        &qi_snapshot(zones.find_zone_by_name("spawn").unwrap().spirit_qi, &ledger),
        0.0,
    )
    .expect("Watch 级 zone qi 微调必须在 zone_qi + ledger_qi 口径守恒");
}

#[test]
fn watch_zone_qi_drain_is_noop_when_zone_is_missing() {
    let mut zones = single_zone_registry("spawn", 0.6);
    let mut ledger = WorldQiAccount::default();

    let transfer = apply_watch_zone_qi_drain(
        "missing",
        tiandao_response_profile(TiandaoResponseLevel::Watch)
            .unwrap()
            .interval_ticks,
        &mut zones,
        &mut ledger,
    );

    assert!(
        transfer.is_none(),
        "未知 zone 不应生成 Watch 级 QiTransfer，got={transfer:?}"
    );
    assert_close(zones.find_zone_by_name("spawn").unwrap().spirit_qi, 0.6);
    assert_close(ledger.total(), 0.0);
}

#[test]
fn watch_zone_qi_drain_is_noop_when_zone_qi_is_empty_or_negative() {
    for zone_qi in [0.0, -0.1] {
        let mut zones = single_zone_registry("spawn", zone_qi);
        let mut ledger = WorldQiAccount::default();

        let transfer = apply_watch_zone_qi_drain(
            "spawn",
            tiandao_response_profile(TiandaoResponseLevel::Watch)
                .unwrap()
                .interval_ticks,
            &mut zones,
            &mut ledger,
        );

        assert!(
            transfer.is_none(),
            "zone_qi={zone_qi} 时 Watch 级不应从空/负真元区抽取，got={transfer:?}"
        );
        assert_close(zones.find_zone_by_name("spawn").unwrap().spirit_qi, zone_qi);
        assert_close(ledger.total(), 0.0);
    }
}

#[test]
fn watch_zone_qi_drain_partial_transfer_preserves_conservation() {
    // plan-zone-qi-economy-v1 P2：zone_qi=0.32，地板 QI_NPC_ABSORB_FLOOR=0.3，
    // 地板以上可吸取余量仅 0.02 < 单次请求量 0.05 —— 必须部分转移且钳在地板，
    // 而非（改地板前旧行为）一路抽到 0。
    let mut zones = single_zone_registry("spawn", 0.32);
    let mut ledger = WorldQiAccount::default();
    let before = qi_snapshot(0.32, &ledger);

    let transfer = apply_watch_zone_qi_drain(
        "spawn",
        tiandao_response_profile(TiandaoResponseLevel::Watch)
            .unwrap()
            .interval_ticks,
        &mut zones,
        &mut ledger,
    )
    .expect("地板以上有余量（即便小于单次请求量）必须被部分转移");

    assert_close(transfer.amount, 0.02);
    assert_close(zones.find_zone_by_name("spawn").unwrap().spirit_qi, 0.3);
    assert_close(ledger.balance(&QiAccountId::tiandao()), 0.02);
    crate::qi_physics::assert_conservation(
        &before,
        &qi_snapshot(zones.find_zone_by_name("spawn").unwrap().spirit_qi, &ledger),
        0.0,
    )
    .expect("Watch 级部分抽取必须保持 zone_qi + ledger_qi 守恒");
}

#[test]
fn watch_zone_qi_drain_is_noop_when_zone_qi_is_positive_but_at_or_below_absorb_floor() {
    // plan-zone-qi-economy-v1 P2：地板红线——zone_qi 在 (0, QI_NPC_ABSORB_FLOOR] 区间
    // 时（不同于既有的 0.0/负值 用例），天道监视也不得再抽，否则地板形同虚设。
    for zone_qi in [0.3, 0.2, 0.05] {
        let mut zones = single_zone_registry("spawn", zone_qi);
        let mut ledger = WorldQiAccount::default();

        let transfer = apply_watch_zone_qi_drain(
            "spawn",
            tiandao_response_profile(TiandaoResponseLevel::Watch)
                .unwrap()
                .interval_ticks,
            &mut zones,
            &mut ledger,
        );

        assert!(
            transfer.is_none(),
            "zone_qi={zone_qi} 已在/低于 QI_NPC_ABSORB_FLOOR(0.3)，Watch 级不应再抽取，\
             got={transfer:?}"
        );
        assert_close(zones.find_zone_by_name("spawn").unwrap().spirit_qi, zone_qi);
        assert_close(ledger.total(), 0.0);
    }
}

#[test]
fn watch_zone_qi_drain_stops_exactly_at_absorb_floor_boundary() {
    // 边界回归：zone_qi 略高于地板（0.31）时只能抽出地板以上的 0.01，
    // 即使单次请求量 0.05 远大于此，也绝不能把 zone 拉穿地板。
    let mut zones = single_zone_registry("spawn", 0.31);
    let mut ledger = WorldQiAccount::default();

    let transfer = apply_watch_zone_qi_drain(
        "spawn",
        tiandao_response_profile(TiandaoResponseLevel::Watch)
            .unwrap()
            .interval_ticks,
        &mut zones,
        &mut ledger,
    )
    .expect("地板以上还有 0.01 余量，必须发生一次部分转移");

    assert_close(transfer.amount, 0.01);
    assert_close(
        zones.find_zone_by_name("spawn").unwrap().spirit_qi,
        crate::qi_physics::constants::QI_NPC_ABSORB_FLOOR,
    );
}

#[test]
fn watch_zone_qi_drain_zero_interval_is_noop() {
    let mut zones = single_zone_registry("spawn", 0.6);
    let mut ledger = WorldQiAccount::default();

    let transfer = apply_watch_zone_qi_drain("spawn", 0, &mut zones, &mut ledger);

    assert!(
        transfer.is_none(),
        "interval_ticks=0 时 Watch 级不应生成 QiTransfer，got={transfer:?}"
    );
    assert_close(zones.find_zone_by_name("spawn").unwrap().spirit_qi, 0.6);
    assert_close(ledger.total(), 0.0);
}

#[test]
fn tribulation_response_enqueues_thunder_with_targeted_feedback() {
    let mut attention = TiandaoAttention::default();
    let (active_events, _zones, vfx_events, audio_events, rx) =
        emit_response_chain_for_test(&mut attention, TiandaoResponseLevel::Tribulation, 75.0, 200);

    assert!(active_events.contains("spawn", EVENT_THUNDER_TRIBULATION));
    assert_eq!(
        active_events.thunder_target_for_zone("spawn").as_deref(),
        Some("offline:Alice"),
        "Tribulation 雷劫必须带 target_player，避免退化成普通区域天灾"
    );
    assert_eq!(vfx_events.iter_current_update_events().count(), 1);
    assert_eq!(audio_events.iter_current_update_events().count(), 1);
    let narration = expect_tiandao_narration_request(&rx);
    assert_eq!(
        narration.response_level,
        TiandaoHuntResponseLevelV1::Tribulation
    );
}

#[test]
fn annihilate_response_enqueues_realm_collapse_and_keeps_attention_sticky() {
    let mut attention = TiandaoAttention {
        level: 95.0,
        response: TiandaoResponseLevel::Annihilate,
        ..TiandaoAttention::default()
    };
    let (active_events, _zones, vfx_events, audio_events, rx) =
        emit_response_chain_for_test(&mut attention, TiandaoResponseLevel::Annihilate, 95.0, 200);

    assert!(active_events.contains("spawn", EVENT_REALM_COLLAPSE));
    assert_eq!(vfx_events.iter_current_update_events().count(), 1);
    assert_eq!(audio_events.iter_current_update_events().count(), 1);
    let narration = expect_tiandao_narration_request(&rx);
    assert_eq!(
        narration.response_level,
        TiandaoHuntResponseLevelV1::Annihilate
    );

    advance_attention(&mut attention, input(Realm::Void), 400);
    assert_eq!(
        attention.response,
        TiandaoResponseLevel::Annihilate,
        "Annihilate 级不会靠自然衰减解除，必须死亡或负灵域反制"
    );
}

#[test]
fn watch_profile_has_audio_but_no_particle_event() {
    let eval = TiandaoEvalSnapshot {
        position: DVec3::new(1.2, 64.0, -3.4),
        zone_name: Some("spawn".to_string()),
        zone_spirit_qi: 0.6,
        realm: Realm::Spirit,
        activity: TiandaoActivity::Standing,
        response: TiandaoResponseLevel::Watch,
        level: 20.0,
        countermeasure: TiandaoCountermeasureOutcome::default(),
    };
    let profile = tiandao_response_profile(TiandaoResponseLevel::Watch).unwrap();

    let player = Entity::from_raw(42);
    let audio = tiandao_audio_request(&eval, profile, player);

    assert_eq!(audio.recipe_id, "tiandao_watch_ambient");
    assert_eq!(audio.instance_id, TIANDAO_AUDIO_INSTANCE_BASE + 1);
    assert_eq!(audio.flag.as_deref(), Some("tiandao:watch"));
    assert!(matches!(audio.recipient, AudioRecipient::Single(entity) if entity == player));
    assert!(
        tiandao_vfx_request(&eval, profile).is_none(),
        "Watch 级按 plan 只给 HUD/音效氛围，不产生可见粒子"
    );
}

#[test]
fn watch_response_publishes_narration_request_before_event_name_gate() {
    let (redis, rx) = narration_test_bridge();
    let mut attention = TiandaoAttention::default();

    apply_tiandao_response_chain(
        &mut attention,
        narration_test_snapshot(TiandaoResponseLevel::Watch, 20.0),
        Entity::from_raw(42),
        "Alice",
        TiandaoResponseSinks {
            zones: None,
            active_events: None,
            vfx_events: None,
            audio_events: None,
            redis: Some(&redis),
            qi_ledger: None,
        },
        200,
    );

    let payload = expect_tiandao_narration_request(&rx);
    assert_eq!(payload.character_id, "offline:Alice");
    assert_eq!(payload.realm, "Spirit");
    assert_eq!(payload.attention_level, 20.0);
    assert_eq!(payload.response_level, TiandaoHuntResponseLevelV1::Watch);
    assert_eq!(payload.zone, "spawn");
    assert_eq!(payload.narration_count, 0);
    assert!(
        payload
            .recent_actions
            .iter()
            .any(|entry| entry == "activity:meditating"),
        "recent_actions must carry the activity context used by the agent prompt"
    );
}

#[test]
fn narration_request_respects_interval_and_increments_same_response_count() {
    let (redis, rx) = narration_test_bridge();
    let mut attention = TiandaoAttention::default();

    apply_tiandao_response_chain(
        &mut attention,
        narration_test_snapshot(TiandaoResponseLevel::Watch, 20.0),
        Entity::from_raw(42),
        "Alice",
        narration_test_sinks(&redis),
        200,
    );
    assert_eq!(expect_tiandao_narration_request(&rx).narration_count, 0);

    apply_tiandao_response_chain(
        &mut attention,
        narration_test_snapshot(TiandaoResponseLevel::Watch, 21.0),
        Entity::from_raw(42),
        "Alice",
        narration_test_sinks(&redis),
        200 + tiandao_response_profile(TiandaoResponseLevel::Watch)
            .unwrap()
            .interval_ticks
            - 1,
    );
    assert!(
        rx.try_recv().is_err(),
        "same response inside interval must not publish duplicate narration"
    );

    apply_tiandao_response_chain(
        &mut attention,
        narration_test_snapshot(TiandaoResponseLevel::Watch, 22.0),
        Entity::from_raw(42),
        "Alice",
        narration_test_sinks(&redis),
        200 + tiandao_response_profile(TiandaoResponseLevel::Watch)
            .unwrap()
            .interval_ticks,
    );
    assert_eq!(expect_tiandao_narration_request(&rx).narration_count, 1);
}

#[test]
fn narration_count_resets_when_response_level_upgrades() {
    let (redis, rx) = narration_test_bridge();
    let mut attention = TiandaoAttention::default();

    apply_tiandao_response_chain(
        &mut attention,
        narration_test_snapshot(TiandaoResponseLevel::Watch, 20.0),
        Entity::from_raw(42),
        "Alice",
        narration_test_sinks(&redis),
        200,
    );
    let _ = expect_tiandao_narration_request(&rx);

    apply_tiandao_response_chain(
        &mut attention,
        narration_test_snapshot(TiandaoResponseLevel::Watch, 22.0),
        Entity::from_raw(42),
        "Alice",
        narration_test_sinks(&redis),
        200 + tiandao_response_profile(TiandaoResponseLevel::Watch)
            .unwrap()
            .interval_ticks,
    );
    assert_eq!(expect_tiandao_narration_request(&rx).narration_count, 1);

    apply_tiandao_response_chain(
        &mut attention,
        narration_test_snapshot(TiandaoResponseLevel::Pressure, 41.0),
        Entity::from_raw(42),
        "Alice",
        narration_test_sinks(&redis),
        200 + tiandao_response_profile(TiandaoResponseLevel::Watch)
            .unwrap()
            .interval_ticks
            + 1,
    );
    let payload = expect_tiandao_narration_request(&rx);
    assert_eq!(payload.response_level, TiandaoHuntResponseLevelV1::Pressure);
    assert_eq!(payload.narration_count, 0);
}

#[test]
fn narration_count_resets_after_response_drops_to_none() {
    let (redis, rx) = narration_test_bridge();
    let mut attention = TiandaoAttention::default();
    let interval = tiandao_response_profile(TiandaoResponseLevel::Watch)
        .unwrap()
        .interval_ticks;

    apply_tiandao_response_chain(
        &mut attention,
        narration_test_snapshot(TiandaoResponseLevel::Watch, 20.0),
        Entity::from_raw(42),
        "Alice",
        narration_test_sinks(&redis),
        200,
    );
    assert_eq!(expect_tiandao_narration_request(&rx).narration_count, 0);

    apply_tiandao_response_chain(
        &mut attention,
        narration_test_snapshot(TiandaoResponseLevel::Watch, 22.0),
        Entity::from_raw(42),
        "Alice",
        narration_test_sinks(&redis),
        200 + interval,
    );
    assert_eq!(expect_tiandao_narration_request(&rx).narration_count, 1);

    apply_tiandao_response_chain(
        &mut attention,
        narration_test_snapshot(TiandaoResponseLevel::None, 8.0),
        Entity::from_raw(42),
        "Alice",
        narration_test_sinks(&redis),
        200 + interval + 1,
    );
    assert_eq!(attention.last_emitted_response, TiandaoResponseLevel::None);
    assert_eq!(attention.last_response_tick, 0);
    assert_eq!(attention.narration_count, 0);
    assert!(
        rx.try_recv().is_err(),
        "None response must not publish narration while resetting response state"
    );

    apply_tiandao_response_chain(
        &mut attention,
        narration_test_snapshot(TiandaoResponseLevel::Watch, 19.0),
        Entity::from_raw(42),
        "Alice",
        narration_test_sinks(&redis),
        200 + interval + 2,
    );
    assert_eq!(expect_tiandao_narration_request(&rx).narration_count, 0);
}

#[test]
fn response_profile_matches_four_levels() {
    assert!(tiandao_response_profile(TiandaoResponseLevel::None).is_none());

    let watch = tiandao_response_profile(TiandaoResponseLevel::Watch).unwrap();
    assert_eq!(watch.audio_recipe_id, "tiandao_watch_ambient");
    assert_eq!(watch.vfx_event_id, None);
    assert_eq!(watch.event_name, None);

    let pressure = tiandao_response_profile(TiandaoResponseLevel::Pressure).unwrap();
    assert_eq!(pressure.audio_recipe_id, "tiandao_pressure_ambient");
    assert_eq!(pressure.vfx_event_id, Some("bong:tiandao_beast_spawn"));
    assert_eq!(pressure.event_name, Some(EVENT_BEAST_TIDE));

    let tribulation = tiandao_response_profile(TiandaoResponseLevel::Tribulation).unwrap();
    assert_eq!(tribulation.audio_recipe_id, "tiandao_tribulation_ambient");
    assert_eq!(
        tribulation.vfx_event_id,
        Some("bong:tiandao_directed_thunder")
    );
    assert_eq!(tribulation.event_name, Some(EVENT_THUNDER_TRIBULATION));

    let annihilate = tiandao_response_profile(TiandaoResponseLevel::Annihilate).unwrap();
    assert_eq!(annihilate.audio_recipe_id, "tiandao_annihilate_ambient");
    assert_eq!(
        annihilate.vfx_event_id,
        Some("bong:realm_collapse_boundary")
    );
    assert_eq!(annihilate.event_name, Some(EVENT_REALM_COLLAPSE));
}

#[test]
fn response_profiles_pin_plan_intervals() {
    assert_eq!(
        tiandao_response_profile(TiandaoResponseLevel::Watch)
            .unwrap()
            .interval_ticks,
        5 * 60 * 20
    );
    assert_eq!(
        tiandao_response_profile(TiandaoResponseLevel::Pressure)
            .unwrap()
            .interval_ticks,
        TIANDAO_PRESSURE_EVENT_INTERVAL_TICKS
    );
    assert_eq!(
        tiandao_response_profile(TiandaoResponseLevel::Tribulation)
            .unwrap()
            .interval_ticks,
        TIANDAO_TRIBULATION_EVENT_INTERVAL_TICKS
    );
    assert_eq!(
        tiandao_response_profile(TiandaoResponseLevel::Annihilate)
            .unwrap()
            .interval_ticks,
        TIANDAO_ANNIHILATE_EVENT_INTERVAL_TICKS
    );
}
