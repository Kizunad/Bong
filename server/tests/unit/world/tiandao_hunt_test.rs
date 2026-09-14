#![allow(dead_code, unused_imports)]

use bong_server::cultivation::components::{Cultivation, Realm};
use bong_server::cultivation::tick::CultivationClock;
use bong_server::world::dimension::{CurrentDimension, DimensionKind};
use bong_server::world::season::Season;
use bong_server::world::tiandao_hunt::*;
use bong_server::world::zone::{Zone, ZoneRegistry};
use bong_server::zhenfa::{DeceiveHeavenEvent, DeceiveHeavenExposedEvent};
use valence::prelude::{App, DVec3, Entity, Position, Update};
use valence::testing::create_mock_client;

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

#[test]
fn realm_base_rates_match_plan_scale() {
    assert_eq!(realm_base_rate(Realm::Awaken), 0.0);
    assert_eq!(realm_base_rate(Realm::Induce), 0.0);
    assert_eq!(realm_base_rate(Realm::Condense), 0.01);
    assert_eq!(realm_base_rate(Realm::Solidify), 0.05);
    assert_eq!(realm_base_rate(Realm::Spirit), 0.15);
    assert_eq!(realm_base_rate(Realm::Void), 0.40);
}

#[test]
fn zone_qi_factor_has_all_plan_thresholds() {
    assert_eq!(zone_qi_factor(-0.5), 0.3);
    assert_eq!(zone_qi_factor(0.1), 0.3);
    assert_eq!(zone_qi_factor(0.1001), 0.6);
    assert_eq!(zone_qi_factor(0.3), 0.6);
    assert_eq!(zone_qi_factor(0.3001), 1.0);
    assert_eq!(zone_qi_factor(0.6), 1.0);
    assert_eq!(zone_qi_factor(0.6001), 1.8);
}

#[test]
fn activity_factor_matches_plan_actions() {
    assert_eq!(activity_factor(TiandaoActivity::Meditating), 1.5);
    assert_eq!(activity_factor(TiandaoActivity::Combat), 1.2);
    assert_eq!(activity_factor(TiandaoActivity::Moving), 0.8);
    assert_eq!(activity_factor(TiandaoActivity::Standing), 1.0);
    assert_eq!(activity_factor(TiandaoActivity::InNiche), 0.5);
}

#[test]
fn xizhuan_season_increases_accumulation() {
    let normal = accumulation_rate(TiandaoAttentionInput {
        season: Season::Summer,
        ..input(Realm::Void)
    });
    let xizhuan = accumulation_rate(TiandaoAttentionInput {
        season: Season::SummerToWinter,
        ..input(Realm::Void)
    });
    assert!((xizhuan - normal * 1.5).abs() < f64::EPSILON);
}

#[test]
fn response_upgrade_thresholds_are_inclusive() {
    assert_eq!(
        response_for_level(TiandaoResponseLevel::None, 15.0),
        TiandaoResponseLevel::Watch
    );
    assert_eq!(
        response_for_level(TiandaoResponseLevel::Watch, 40.0),
        TiandaoResponseLevel::Pressure
    );
    assert_eq!(
        response_for_level(TiandaoResponseLevel::Pressure, 70.0),
        TiandaoResponseLevel::Tribulation
    );
    assert_eq!(
        response_for_level(TiandaoResponseLevel::Tribulation, 90.0),
        TiandaoResponseLevel::Annihilate
    );
}

#[test]
fn response_downgrade_uses_hysteresis() {
    assert_eq!(
        response_for_level(TiandaoResponseLevel::Watch, 10.0),
        TiandaoResponseLevel::Watch
    );
    assert_eq!(
        response_for_level(TiandaoResponseLevel::Watch, 9.99),
        TiandaoResponseLevel::None
    );
    assert_eq!(
        response_for_level(TiandaoResponseLevel::Pressure, 30.0),
        TiandaoResponseLevel::Pressure
    );
    assert_eq!(
        response_for_level(TiandaoResponseLevel::Pressure, 29.99),
        TiandaoResponseLevel::Watch
    );
    assert_eq!(
        response_for_level(TiandaoResponseLevel::Tribulation, 60.0),
        TiandaoResponseLevel::Tribulation
    );
    assert_eq!(
        response_for_level(TiandaoResponseLevel::Tribulation, 59.99),
        TiandaoResponseLevel::Pressure
    );
    assert_eq!(
        response_for_level(TiandaoResponseLevel::Annihilate, 80.0),
        TiandaoResponseLevel::Annihilate
    );
    assert_eq!(
        response_for_level(TiandaoResponseLevel::Annihilate, 79.99),
        TiandaoResponseLevel::Tribulation
    );
}

#[test]
fn dead_and_negative_zones_accelerate_decay_without_qi_transfer() {
    assert_eq!(zone_decay_multiplier(0.0), 3.0);
    assert_eq!(zone_decay_multiplier(-0.01), 5.0);
    assert_eq!(zone_decay_multiplier(0.01), 1.0);
    assert_close(decay_rate(TiandaoResponseLevel::Watch, 0.0), 0.15);
    assert_close(decay_rate(TiandaoResponseLevel::Watch, -0.1), 0.25);
}

#[test]
fn deceive_heaven_decoy_diverts_attention_when_far_active_and_not_revealed() {
    let start_level = 20.0;
    let mut attention = TiandaoAttention {
        level: start_level,
        response: TiandaoResponseLevel::Watch,
        ..TiandaoAttention::default()
    };
    let outcome = advance_attention_with_countermeasures(
        &mut attention,
        TiandaoAttentionInput {
            realm: Realm::Awaken,
            zone_spirit_qi: 0.6,
            activity: TiandaoActivity::Standing,
            season: Season::Summer,
            is_dominant: false,
        },
        TiandaoCountermeasureInput {
            deceive_heaven_decoy: Some(DeceiveHeavenDecoyInput {
                placed_tick: 200,
                distance_blocks: DECEIVE_HEAVEN_DECOY_MIN_DISTANCE_BLOCKS,
                exposed: false,
            }),
        },
        400,
    );

    assert_eq!(outcome.deceive_heaven, DeceiveHeavenOutcome::Diverted);
    assert_close(
        outcome.decay_multiplier,
        DECEIVE_HEAVEN_DECOY_DECAY_MULTIPLIER,
    );
    assert_close(
        attention.level,
        start_level - 0.05 * DECEIVE_HEAVEN_DECOY_DECAY_MULTIPLIER,
    );
    assert_eq!(attention.response, TiandaoResponseLevel::Watch);
}

#[test]
fn deceive_heaven_decoy_revealed_adds_twenty_attention_without_decay_bonus() {
    let start_level = 21.0;
    let mut attention = TiandaoAttention {
        level: start_level,
        response: TiandaoResponseLevel::Watch,
        ..TiandaoAttention::default()
    };
    let outcome = advance_attention_with_countermeasures(
        &mut attention,
        TiandaoAttentionInput {
            realm: Realm::Awaken,
            zone_spirit_qi: 0.6,
            activity: TiandaoActivity::Standing,
            season: Season::Summer,
            is_dominant: false,
        },
        TiandaoCountermeasureInput {
            deceive_heaven_decoy: Some(DeceiveHeavenDecoyInput {
                placed_tick: 0,
                distance_blocks: 800.0,
                exposed: true,
            }),
        },
        400,
    );

    assert_eq!(outcome.deceive_heaven, DeceiveHeavenOutcome::Revealed);
    assert_close(outcome.attention_penalty, DECEIVE_HEAVEN_REVEAL_PENALTY);
    assert_close(
        attention.level,
        start_level - 0.05 + DECEIVE_HEAVEN_REVEAL_PENALTY,
    );
    assert_eq!(attention.response, TiandaoResponseLevel::Pressure);
}

#[test]
fn deceive_heaven_decoy_has_distance_reveal_and_expiry_boundaries() {
    let active = DeceiveHeavenDecoyInput {
        placed_tick: 100,
        distance_blocks: 500.0,
        exposed: false,
    };
    assert_eq!(
        deceive_heaven_decoy_outcome(active, 100 + DECEIVE_HEAVEN_DECOY_DURATION_TICKS - 1),
        DeceiveHeavenOutcome::Diverted
    );
    assert_eq!(
        deceive_heaven_decoy_outcome(
            DeceiveHeavenDecoyInput {
                distance_blocks: 499.99,
                ..active
            },
            200
        ),
        DeceiveHeavenOutcome::TooClose
    );
    assert_eq!(
        deceive_heaven_decoy_outcome(
            DeceiveHeavenDecoyInput {
                exposed: true,
                distance_blocks: 499.99,
                ..active
            },
            200
        ),
        DeceiveHeavenOutcome::Revealed
    );
    assert_eq!(
        deceive_heaven_decoy_outcome(active, 100 + DECEIVE_HEAVEN_DECOY_DURATION_TICKS),
        DeceiveHeavenOutcome::Expired
    );
}

#[test]
fn deceive_heaven_exposure_penalty_wins_over_distance_boundary() {
    let outcome = countermeasure_outcome(
        TiandaoCountermeasureInput {
            deceive_heaven_decoy: Some(DeceiveHeavenDecoyInput {
                placed_tick: 100,
                distance_blocks: 1.0,
                exposed: true,
            }),
        },
        200,
    );

    assert_eq!(outcome.deceive_heaven, DeceiveHeavenOutcome::Revealed);
    assert_eq!(outcome.decay_multiplier, 1.0);
    assert_close(outcome.attention_penalty, DECEIVE_HEAVEN_REVEAL_PENALTY);
}

#[test]
fn tiandao_hunt_tick_consumes_deceive_heaven_deploy_event_and_decays_x4() {
    let (mut app, player) = tiandao_runtime_app();
    app.world_mut().send_event(deceive_heaven_deploy_event(
        11,
        "offline:Alice",
        [600, 64, 0],
        200,
    ));
    app.world_mut().resource_mut::<CultivationClock>().tick = 200;

    app.update();

    let attention = app.world().get::<TiandaoAttention>(player).unwrap();
    assert_close(
        attention.level,
        20.0 - decay_rate(TiandaoResponseLevel::Watch, 0.0) * DECEIVE_HEAVEN_DECOY_DECAY_MULTIPLIER,
    );
    assert_eq!(attention.last_eval_tick, 200);
}

#[test]
fn tiandao_hunt_tick_applies_deceive_heaven_exposed_penalty_once() {
    let (mut app, player) = tiandao_runtime_app();
    app.world_mut()
        .get_mut::<TiandaoAttention>(player)
        .unwrap()
        .level = 21.0;
    app.world_mut().send_event(deceive_heaven_deploy_event(
        12,
        "offline:Alice",
        [600, 64, 0],
        200,
    ));
    app.world_mut().send_event(deceive_heaven_exposed_event(
        12,
        "offline:Alice",
        [600, 64, 0],
        200,
    ));
    app.world_mut().resource_mut::<CultivationClock>().tick = 200;

    app.update();

    let first = app.world().get::<TiandaoAttention>(player).unwrap().clone();
    assert_close(
        first.level,
        21.0 - decay_rate(TiandaoResponseLevel::Watch, 0.0) + DECEIVE_HEAVEN_REVEAL_PENALTY,
    );
    assert_eq!(first.response, TiandaoResponseLevel::Pressure);

    app.world_mut().resource_mut::<CultivationClock>().tick = 400;
    app.update();

    let second = app.world().get::<TiandaoAttention>(player).unwrap();
    assert_close(
        second.level,
        first.level - decay_rate(TiandaoResponseLevel::Pressure, 0.0),
    );
    assert_eq!(second.last_eval_tick, 400);
}

#[test]
fn tiandao_hunt_tick_uses_negative_zone_registry_after_ten_second_interval() {
    let (mut app, player) = tiandao_runtime_app();
    app.insert_resource(single_zone_registry("test_negative_field", -0.3));

    app.world_mut().resource_mut::<CultivationClock>().tick = 199;
    app.update();
    let before = app.world().get::<TiandaoAttention>(player).unwrap();
    assert_eq!(before.last_eval_tick, 0);
    assert_close(before.level, 20.0);

    app.world_mut().resource_mut::<CultivationClock>().tick = 200;
    app.update();

    let after = app.world().get::<TiandaoAttention>(player).unwrap();
    assert_eq!(after.last_eval_tick, 200);
    assert_close(
        after.level,
        20.0 - decay_rate(TiandaoResponseLevel::Watch, -0.3),
    );
}

#[test]
fn realm_regression_recomputes_accumulation_rate_from_new_realm() {
    let (mut app, player) = tiandao_runtime_app();
    app.insert_resource(single_zone_registry("test_plain_field", 0.6));
    app.world_mut()
        .get_mut::<TiandaoAttention>(player)
        .unwrap()
        .level = 0.0;
    app.world_mut()
        .get_mut::<Cultivation>(player)
        .unwrap()
        .realm = Realm::Spirit;

    app.world_mut().resource_mut::<CultivationClock>().tick = 200;
    app.update();
    let first = app.world().get::<TiandaoAttention>(player).unwrap();
    assert_close(first.accumulation_rate, 0.15);

    app.world_mut()
        .get_mut::<Cultivation>(player)
        .unwrap()
        .realm = Realm::Solidify;
    app.world_mut().resource_mut::<CultivationClock>().tick = 400;
    app.update();
    let second = app.world().get::<TiandaoAttention>(player).unwrap();
    assert_close(second.accumulation_rate, 0.05);
}

#[test]
fn evaluation_respects_ten_second_interval() {
    let cultivation = Cultivation {
        realm: Realm::Void,
        ..Cultivation::default()
    };
    let position = Position(DVec3::new(0.0, 65.0, 0.0));
    let dimension = CurrentDimension(DimensionKind::Overworld);
    let zones = ZoneRegistry::fallback();
    let mut attention = TiandaoAttention::default();

    apply_attention_eval(
        &cultivation,
        &position,
        &mut attention,
        TiandaoEvalContext {
            dimension: Some(&dimension),
            zones: Some(&zones),
            season: Season::Summer,
            activity: TiandaoActivity::Standing,
            countermeasures: TiandaoCountermeasureInput::default(),
            now_tick: 199,
            is_dominant: false,
        },
    );
    assert_eq!(attention.last_eval_tick, 0);
    assert_eq!(attention.level, 0.0);

    apply_attention_eval(
        &cultivation,
        &position,
        &mut attention,
        TiandaoEvalContext {
            dimension: Some(&dimension),
            zones: Some(&zones),
            season: Season::Summer,
            activity: TiandaoActivity::Standing,
            countermeasures: TiandaoCountermeasureInput::default(),
            now_tick: 200,
            is_dominant: false,
        },
    );
    assert_eq!(attention.last_eval_tick, 200);
    assert!(attention.level > 0.0);
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

#[test]
fn realm_rank_gates_presence_payload_to_spirit_and_above() {
    assert_eq!(realm_rank(Realm::Awaken), 0);
    assert_eq!(realm_rank(Realm::Induce), 1);
    assert_eq!(realm_rank(Realm::Condense), 2);
    assert_eq!(realm_rank(Realm::Solidify), 3);
    assert_eq!(realm_rank(Realm::Spirit), 4);
    assert_eq!(realm_rank(Realm::Void), 5);
}
