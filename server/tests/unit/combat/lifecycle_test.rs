#![allow(dead_code, unused_imports)]

use bong_server::alchemy::LearnedRecipes;
use bong_server::combat::components::*;
use bong_server::combat::events::*;
use bong_server::combat::lifecycle::*;
use bong_server::combat::CombatClock;
use bong_server::cultivation::color::*;
use bong_server::cultivation::components::*;
use bong_server::cultivation::death_hooks::*;
use bong_server::cultivation::known_techniques::{KnownTechniques, TechniqueRegistry};
use bong_server::cultivation::life_record::BiographyEntry;
use bong_server::cultivation::lifespan::*;
use bong_server::cultivation::tribulation::AscensionQuotaOpened;
use bong_server::inventory::*;
use bong_server::network::vfx_event_emit::VfxEventRequest;
use bong_server::npc::spawn::NpcMarker;
use bong_server::persistence::*;
use bong_server::player::state::{
    canonical_player_id, load_player_slices, save_player_slices, PlayerState,
    PlayerStatePersistence,
};
use bong_server::qi_physics::{QiTransfer, QiTransferReason, WorldQiAccount};
use bong_server::schema::cultivation::realm_to_string;
use bong_server::schema::death_cinematic::DeathCinematicS2cV1;
use bong_server::schema::death_insight::*;
use bong_server::schema::server_data::DeathScreenStageV1;
use bong_server::schema::vfx_event::VfxEventPayloadV1;
use bong_server::skill::components::SkillSet;
use bong_server::skin::NpcVisualProfile;
use bong_server::world::dimension::*;
use bong_server::world::spirit_eye::SpiritEyeRegistry;
use bong_server::world::zone::ZoneRegistry;

use bong_server::combat::anticheat::AntiCheatCounter;
use bong_server::combat::components::{
    ActiveStatusEffect, BodyPart, DefenseWindow, StatusEffects, Wound, WoundKind,
    IN_COMBAT_WINDOW_TICKS, REVIVE_WEAKENED_TICKS,
};
use bong_server::combat::events::{
    ApplyStatusEffectIntent, DefenseIntent, RevivalActionIntent, RevivalActionKind,
    StatusEffectKind,
};
use bong_server::cultivation::components::Cultivation;
use bong_server::cultivation::death_hooks::CultivationDeathCause;
use bong_server::cultivation::life_record::LifeRecord;
use bong_server::cultivation::tick::CultivationClock;
use bong_server::death_lifecycle::cinematic::DeathCinematicInit;
use bong_server::network::agent_bridge::SERVER_DATA_CHANNEL;
use bong_server::persistence::{
    bootstrap_sqlite, complete_tribulation_ascension, load_ascension_quota,
    persist_active_tribulation, ActiveTribulationRecord, DeceasedSnapshot, PersistenceSettings,
};
use bong_server::player::state::player_character_id;
use bong_server::qi_physics::constants::QI_ZHENMAI_PREP_WINDOW_MS;
use bong_server::schema::anticheat::ViolationKindV1;
use bong_server::schema::death_cinematic::{
    DeathCinematicRollV1, DeathCinematicZoneKindV1, DeathRollResultV1,
};
use bong_server::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
use rusqlite::{params, Connection};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use valence::prelude::*;
use valence::protocol::packets::play::CustomPayloadS2c;
use valence::testing::{create_mock_client, MockClientHelper};

fn spawn_actor(app: &mut App, wounds: Wounds, stamina: Stamina, lifecycle: Lifecycle) -> Entity {
    app.world_mut()
        .spawn((
            wounds,
            stamina,
            CombatState::default(),
            LifeRecord::default(),
            lifecycle,
        ))
        .id()
}

fn spawn_client_actor(
    app: &mut App,
    username: &str,
    wounds: Wounds,
    stamina: Stamina,
    lifecycle: Lifecycle,
) -> (Entity, MockClientHelper) {
    let (mut client_bundle, helper) = create_mock_client(username);
    client_bundle.player.position = Position::new([8.0, 66.0, 8.0]);
    let entity = app
        .world_mut()
        .spawn((
            client_bundle,
            wounds,
            stamina,
            CombatState::default(),
            LifeRecord::new(bong_server::player::state::canonical_player_id(username)),
            lifecycle,
        ))
        .id();
    (entity, helper)
}

fn flush_client_packets(app: &mut App) {
    let world = app.world_mut();
    let mut query = world.query::<&mut valence::prelude::Client>();
    for mut client in query.iter_mut(world) {
        client
            .flush_packets()
            .expect("mock client packets should flush successfully");
    }
}

fn collect_server_data_payloads(helper: &mut MockClientHelper) -> Vec<ServerDataV1> {
    let mut payloads = Vec::new();
    for frame in helper.collect_received().0 {
        let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
            continue;
        };
        if packet.channel.as_str() != SERVER_DATA_CHANNEL {
            continue;
        }
        payloads.push(
            serde_json::from_slice(packet.data.0 .0).expect("server_data payload should decode"),
        );
    }
    payloads
}

fn unique_temp_dir(test_name: &str) -> PathBuf {
    let unique_suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();

    std::env::temp_dir().join(format!(
        "bong-combat-lifecycle-{test_name}-{}-{unique_suffix}",
        std::process::id()
    ))
}

fn persistence_settings(test_name: &str) -> (PersistenceSettings, PathBuf) {
    let root = unique_temp_dir(test_name);
    let db_path = root.join("data").join("bong.db");
    bootstrap_sqlite(&db_path, &format!("combat-lifecycle-{test_name}"))
        .expect("sqlite bootstrap should succeed");
    (
        PersistenceSettings::with_db_path(&db_path, format!("combat-lifecycle-{test_name}")),
        root,
    )
}

#[test]
fn wound_bleed_tick_emits_single_death_event_on_alive_to_dead_transition() {
    let mut app = App::new();
    app.insert_resource(CombatClock {
        tick: BLEED_TICK_INTERVAL_TICKS,
    });
    app.add_event::<DeathEvent>();
    app.add_systems(Update, wound_bleed_tick);

    let entity = spawn_actor(
        &mut app,
        Wounds {
            health_current: 2.0,
            health_max: 30.0,
            entries: vec![Wound {
                location: bong_server::body_plan::legacy_body_part_to_id(BodyPart::Chest),
                kind: WoundKind::Cut,
                severity: 0.3,
                bleeding_per_sec: 3.0,
                created_at_tick: 0,
                inflicted_by: None,
            }],
        },
        Stamina::default(),
        Lifecycle::default(),
    );

    app.update();
    app.world_mut().resource_mut::<CombatClock>().tick += BLEED_TICK_INTERVAL_TICKS;
    app.update();

    let wounds = app.world().entity(entity).get::<Wounds>().unwrap();
    let death_events = app.world().resource::<Events<DeathEvent>>();
    assert_eq!(wounds.health_current, 0.0);
    assert_eq!(death_events.len(), 1);
}

#[test]
fn wound_bleed_tick_skips_creative_players() {
    let mut app = App::new();
    app.insert_resource(CombatClock {
        tick: BLEED_TICK_INTERVAL_TICKS,
    });
    app.add_event::<DeathEvent>();
    app.add_systems(Update, wound_bleed_tick);

    let entity = spawn_actor(
        &mut app,
        Wounds {
            health_current: 12.0,
            health_max: 30.0,
            entries: vec![Wound {
                location: bong_server::body_plan::legacy_body_part_to_id(BodyPart::Chest),
                kind: WoundKind::Cut,
                severity: 0.3,
                bleeding_per_sec: 3.0,
                created_at_tick: 0,
                inflicted_by: None,
            }],
        },
        Stamina::default(),
        Lifecycle::default(),
    );
    app.world_mut()
        .entity_mut(entity)
        .insert(GameMode::Creative);

    app.update();

    let wounds = app.world().entity(entity).get::<Wounds>().unwrap();
    assert_eq!(wounds.health_current, 12.0);
    assert_eq!(app.world().resource::<Events<DeathEvent>>().len(), 0);
}

#[test]
fn wound_bleed_tick_uses_latest_game_mode_component() {
    let mut app = App::new();
    app.insert_resource(CombatClock {
        tick: BLEED_TICK_INTERVAL_TICKS,
    });
    app.add_event::<DeathEvent>();
    app.add_systems(Update, wound_bleed_tick);

    let entity = spawn_actor(
        &mut app,
        Wounds {
            health_current: 12.0,
            health_max: 30.0,
            entries: vec![Wound {
                location: bong_server::body_plan::legacy_body_part_to_id(BodyPart::Chest),
                kind: WoundKind::Cut,
                severity: 0.3,
                bleeding_per_sec: 3.0,
                created_at_tick: 0,
                inflicted_by: None,
            }],
        },
        Stamina::default(),
        Lifecycle::default(),
    );

    app.world_mut()
        .entity_mut(entity)
        .insert(GameMode::Survival);
    app.update();
    let after_survival = app
        .world()
        .entity(entity)
        .get::<Wounds>()
        .unwrap()
        .health_current;
    assert_eq!(after_survival, 9.0);

    app.world_mut().resource_mut::<CombatClock>().tick += BLEED_TICK_INTERVAL_TICKS;
    app.world_mut()
        .entity_mut(entity)
        .insert(GameMode::Creative);
    app.update();

    let wounds = app.world().entity(entity).get::<Wounds>().unwrap();
    assert_eq!(
        wounds.health_current, after_survival,
        "switching to Creative must stop residual wound bleed damage"
    );
}

#[test]
fn health_regen_tick_recovers_base_rate() {
    let mut app = App::new();
    app.insert_resource(CombatClock {
        tick: HEALTH_REGEN_TICK_INTERVAL_TICKS,
    });
    app.add_systems(Update, health_regen_tick);

    let entity = spawn_actor(
        &mut app,
        Wounds {
            health_current: 10.0,
            health_max: 30.0,
            entries: Vec::new(),
        },
        Stamina::default(),
        Lifecycle::default(),
    );

    app.update();

    let wounds = app.world().entity(entity).get::<Wounds>().unwrap();
    assert!((wounds.health_current - 10.5).abs() < 1e-6);
}

#[test]
fn health_regen_tick_clamps_at_health_max() {
    let mut app = App::new();
    app.insert_resource(CombatClock {
        tick: HEALTH_REGEN_TICK_INTERVAL_TICKS,
    });
    app.add_systems(Update, health_regen_tick);

    let entity = spawn_actor(
        &mut app,
        Wounds {
            health_current: 29.8,
            health_max: 30.0,
            entries: Vec::new(),
        },
        Stamina::default(),
        Lifecycle::default(),
    );

    app.update();

    let wounds = app.world().entity(entity).get::<Wounds>().unwrap();
    assert_eq!(wounds.health_current, 30.0);
}

#[test]
fn health_regen_tick_skips_zero_full_and_active_bleeding() {
    let mut app = App::new();
    app.insert_resource(CombatClock {
        tick: HEALTH_REGEN_TICK_INTERVAL_TICKS,
    });
    app.add_systems(Update, health_regen_tick);

    let zero_health = spawn_actor(
        &mut app,
        Wounds {
            health_current: 0.0,
            health_max: 30.0,
            entries: Vec::new(),
        },
        Stamina::default(),
        Lifecycle::default(),
    );
    let full_health = spawn_actor(
        &mut app,
        Wounds {
            health_current: 30.0,
            health_max: 30.0,
            entries: Vec::new(),
        },
        Stamina::default(),
        Lifecycle::default(),
    );
    let bleeding = spawn_actor(
        &mut app,
        Wounds {
            health_current: 12.0,
            health_max: 30.0,
            entries: vec![Wound {
                location: bong_server::body_plan::legacy_body_part_to_id(BodyPart::Chest),
                kind: WoundKind::Cut,
                severity: 0.3,
                bleeding_per_sec: 0.1,
                created_at_tick: 0,
                inflicted_by: None,
            }],
        },
        Stamina::default(),
        Lifecycle::default(),
    );

    app.update();

    assert_eq!(
        app.world()
            .entity(zero_health)
            .get::<Wounds>()
            .unwrap()
            .health_current,
        0.0
    );
    assert_eq!(
        app.world()
            .entity(full_health)
            .get::<Wounds>()
            .unwrap()
            .health_current,
        30.0
    );
    assert_eq!(
        app.world()
            .entity(bleeding)
            .get::<Wounds>()
            .unwrap()
            .health_current,
        12.0
    );
}

#[test]
fn health_regen_tick_skips_pending_revival_lifecycles() {
    let mut app = App::new();
    app.insert_resource(CombatClock {
        tick: HEALTH_REGEN_TICK_INTERVAL_TICKS,
    });
    app.add_systems(Update, health_regen_tick);

    let awaiting_revival = spawn_actor(
        &mut app,
        Wounds {
            health_current: 1.0,
            health_max: 30.0,
            entries: Vec::new(),
        },
        Stamina::default(),
        Lifecycle {
            state: LifecycleState::AwaitingRevival,
            ..Lifecycle::default()
        },
    );
    let terminated = spawn_actor(
        &mut app,
        Wounds {
            health_current: 1.0,
            health_max: 30.0,
            entries: Vec::new(),
        },
        Stamina::default(),
        Lifecycle {
            state: LifecycleState::Terminated,
            ..Lifecycle::default()
        },
    );

    app.update();

    assert_eq!(
        app.world()
            .entity(awaiting_revival)
            .get::<Wounds>()
            .unwrap()
            .health_current,
        1.0
    );
    assert_eq!(
        app.world()
            .entity(terminated)
            .get::<Wounds>()
            .unwrap()
            .health_current,
        1.0
    );
}

#[test]
fn health_regen_tick_multiplies_derived_attrs_and_status_boost() {
    let mut app = App::new();
    app.insert_resource(CombatClock {
        tick: HEALTH_REGEN_TICK_INTERVAL_TICKS,
    });
    app.add_systems(Update, health_regen_tick);

    let entity = spawn_actor(
        &mut app,
        Wounds {
            health_current: 10.0,
            health_max: 30.0,
            entries: Vec::new(),
        },
        Stamina::default(),
        Lifecycle::default(),
    );
    app.world_mut().entity_mut(entity).insert((
        DerivedAttrs {
            healing_rate_multiplier: 1.5,
            ..DerivedAttrs::default()
        },
        StatusEffects {
            active: vec![ActiveStatusEffect {
                kind: StatusEffectKind::HealthRegenBoost,
                magnitude: 0.5,
                remaining_ticks: 100,
                source_pill: None,
            }],
        },
    ));

    app.update();

    let wounds = app.world().entity(entity).get::<Wounds>().unwrap();
    assert!((wounds.health_current - 11.125).abs() < 1e-6);
}

#[test]
fn stamina_tick_recovers_exhausted_back_to_idle_after_threshold() {
    let mut app = App::new();
    app.insert_resource(CombatClock {
        tick: STAMINA_TICK_INTERVAL_TICKS,
    });
    app.add_systems(Update, stamina_tick);

    let entity = spawn_actor(
        &mut app,
        Wounds::default(),
        Stamina {
            current: 30.0,
            max: 100.0,
            recover_per_sec: 5.0,
            last_drain_tick: None,
            state: StaminaState::Exhausted,
        },
        Lifecycle::default(),
    );

    app.update();

    let stamina = app.world().entity(entity).get::<Stamina>().unwrap();
    assert!(stamina.current > 30.0);
    assert_eq!(stamina.state, StaminaState::Idle);
}

#[test]
fn sync_combat_state_marks_both_sides_and_charges_attacker_stamina() {
    let mut app = App::new();
    app.add_event::<CombatEvent>();
    app.add_systems(Update, sync_combat_state_from_events);

    let attacker = spawn_actor(
        &mut app,
        Wounds::default(),
        Stamina::default(),
        Lifecycle::default(),
    );
    let target = spawn_actor(
        &mut app,
        Wounds::default(),
        Stamina::default(),
        Lifecycle::default(),
    );

    app.world_mut().send_event(CombatEvent {
        attacker,
        target,
        resolved_at_tick: 15,
        body_part: BodyPart::Chest,
        wound_kind: WoundKind::Blunt,
        source: bong_server::combat::events::AttackSource::Melee,
        debug_command: false,
        physical_damage: 0.0,
        damage: 3.0,
        contam_delta: 0.75,
        description: "hit".to_string(),
        defense_kind: None,
        defense_effectiveness: None,
        defense_contam_reduced: None,
        defense_wound_severity: None,
    });
    app.update();

    let attacker_ref = app.world().entity(attacker);
    let target_ref = app.world().entity(target);
    let attacker_state = attacker_ref.get::<CombatState>().unwrap();
    let target_state = target_ref.get::<CombatState>().unwrap();
    let attacker_stamina = attacker_ref.get::<Stamina>().unwrap();
    let target_stamina = target_ref.get::<Stamina>().unwrap();

    assert_eq!(attacker_state.last_attack_at_tick, Some(15));
    assert_eq!(
        attacker_state.in_combat_until_tick,
        Some(15 + IN_COMBAT_WINDOW_TICKS)
    );
    assert_eq!(
        target_state.in_combat_until_tick,
        Some(15 + IN_COMBAT_WINDOW_TICKS)
    );
    assert!(attacker_stamina.current <= 97.0);
    assert!(attacker_stamina.current >= 94.0);
    assert_eq!(attacker_stamina.state, StaminaState::Combat);
    assert_eq!(target_stamina.state, StaminaState::Combat);
    assert!(
        app.world()
            .entity(attacker)
            .contains::<ActiveCombatWindow>(),
        "攻击方进入战斗窗口时必须挂活跃标记，供精确到期系统消费"
    );
    assert!(
        app.world().entity(target).contains::<ActiveCombatWindow>(),
        "受击方进入战斗窗口时必须挂活跃标记，供精确到期系统消费"
    );
}

#[test]
fn combat_state_tick_clears_expired_windows_and_combat_stamina_state() {
    let mut app = App::new();
    app.insert_resource(CombatClock {
        tick: COMBAT_STATE_TICK_INTERVAL_TICKS,
    });
    app.add_systems(
        Update,
        (combat_state_tick, combat_window_expiry_tick).chain(),
    );

    let entity = app
        .world_mut()
        .spawn((
            Wounds::default(),
            Stamina {
                current: 40.0,
                max: 100.0,
                recover_per_sec: 5.0,
                last_drain_tick: None,
                state: StaminaState::Combat,
            },
            CombatState {
                in_combat_until_tick: Some(10),
                last_attack_at_tick: Some(1),
                incoming_window: Some(DefenseWindow {
                    opened_at_tick: 0,
                    duration_ms: 100,
                }),
            },
            ActiveCombatWindow,
            Lifecycle::default(),
        ))
        .id();

    app.update();

    let state = app.world().entity(entity).get::<CombatState>().unwrap();
    let stamina = app.world().entity(entity).get::<Stamina>().unwrap();
    assert!(state.in_combat_until_tick.is_none());
    assert!(state.incoming_window.is_none());
    assert_eq!(stamina.state, StaminaState::Idle);
    assert!(
        !app.world().entity(entity).contains::<ActiveCombatWindow>(),
        "战斗窗口到期后必须移除活跃标记，避免继续进入逐 tick 查询"
    );
}

#[test]
fn combat_window_expiry_tick_clears_combat_window_on_non_interval_expiry_tick() {
    let mut app = App::new();
    app.insert_resource(CombatClock { tick: 7 });
    app.add_systems(Update, combat_window_expiry_tick);

    let entity = app
        .world_mut()
        .spawn((
            Stamina {
                current: 40.0,
                max: 100.0,
                recover_per_sec: 5.0,
                last_drain_tick: None,
                state: StaminaState::Combat,
            },
            CombatState {
                in_combat_until_tick: Some(7),
                ..CombatState::default()
            },
            ActiveCombatWindow,
        ))
        .id();

    app.update();

    let state = app.world().entity(entity).get::<CombatState>().unwrap();
    let stamina = app.world().entity(entity).get::<Stamina>().unwrap();
    assert!(
        state.in_combat_until_tick.is_none(),
        "非整秒边界到期时必须清除 CombatState，才能触发 HUD 脱战快照"
    );
    assert_eq!(stamina.state, StaminaState::Idle);
    assert!(
        !app.world().entity(entity).contains::<ActiveCombatWindow>(),
        "战斗窗口到期后必须移除活跃标记，避免继续进入逐 tick 查询"
    );
}

#[test]
fn defense_intent_opens_incoming_window() {
    let mut app = App::new();
    app.add_event::<DefenseIntent>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_systems(Update, bong_server::combat::resolve::apply_defense_intents);

    let entity = spawn_actor(
        &mut app,
        Wounds::default(),
        Stamina::default(),
        Lifecycle::default(),
    );
    app.world_mut().entity_mut(entity).insert((
        Cultivation {
            realm: bong_server::cultivation::components::Realm::Induce,
            qi_current: 10.0,
            qi_max: 10.0,
            ..Cultivation::default()
        },
        StatusEffects::default(),
    ));

    app.world_mut().send_event(DefenseIntent {
        defender: entity,
        issued_at_tick: 42,
    });
    app.update();

    let state = app.world().entity(entity).get::<CombatState>().unwrap();
    let window = state.incoming_window.as_ref().expect("window should open");
    assert_eq!(window.opened_at_tick, 42);
    assert_eq!(window.duration_ms, QI_ZHENMAI_PREP_WINDOW_MS);
}

#[test]
fn cultivation_death_immediately_enters_tribulation_decision() {
    let mut app = App::new();
    let (settings, root) = persistence_settings("terminate-existing");
    app.insert_resource(settings);
    app.insert_resource(CombatClock { tick: 40 });
    app.add_event::<DeathEvent>();
    app.add_event::<CultivationDeathTrigger>();
    app.add_event::<DeathInsightRequested>();
    app.add_event::<DeathCinematicPublished>();
    app.add_event::<PlayerRevived>();
    app.add_event::<PlayerTerminated>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<QiTransfer>();
    app.add_systems(Update, (death_arbiter_tick,));

    let entity = spawn_actor(
        &mut app,
        Wounds::default(),
        Stamina::default(),
        Lifecycle {
            fortune_remaining: 0,
            ..Default::default()
        },
    );

    app.world_mut().send_event(CultivationDeathTrigger {
        entity,
        cause: CultivationDeathCause::NegativeZoneDrain,
        context: serde_json::json!({"zone": "rift_valley"}),
    });
    app.update();

    let lifecycle = app.world().entity(entity).get::<Lifecycle>().unwrap();
    let terminated_events = app.world().resource::<Events<PlayerTerminated>>();
    assert_eq!(lifecycle.state, LifecycleState::AwaitingRevival);
    assert!(matches!(
        lifecycle.awaiting_decision,
        Some(RevivalDecision::Tribulation { chance }) if (chance - 0.80).abs() < 1e-9
    ));
    assert_eq!(terminated_events.len(), 0);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn death_arbiter_skips_death_event_reentry_while_awaiting_revival() {
    // bughunt 实证：污染溢出持续触发 DeathEvent，AwaitingRevival（死亡屏，60s 确认窗口）
    // pin 住：死亡屏等待决策期间的死亡事件必须被 continue 跳过，不触碰任何状态。
    let mut app = App::new();
    let (settings, root) = persistence_settings("awaiting-revival-skip-death-event");
    app.insert_resource(settings);
    app.insert_resource(CombatClock { tick: 900 });
    app.add_event::<DeathEvent>();
    app.add_event::<CultivationDeathTrigger>();
    app.add_event::<DeathInsightRequested>();
    app.add_event::<PlayerTerminated>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<QiTransfer>();
    app.add_systems(Update, death_arbiter_tick);

    let mut life_record = LifeRecord::default();
    life_record.push(BiographyEntry::Death {
        cause: "prior".to_string(),
        tick: 100,
    });
    let biography_len_before = life_record.biography.len();

    let entity = app
        .world_mut()
        .spawn((
            Wounds {
                health_current: 0.0,
                health_max: 30.0,
                entries: Vec::new(),
            },
            Stamina::default(),
            CombatState::default(),
            life_record,
            Lifecycle {
                state: LifecycleState::AwaitingRevival,
                awaiting_decision: Some(RevivalDecision::Fortune { chance: 1.0 }),
                revival_decision_deadline_tick: Some(999),
                death_count: 1,
                ..Default::default()
            },
        ))
        .id();

    app.world_mut().send_event(DeathEvent {
        target: entity,
        cause: "contamination_overflow".to_string(),
        attacker: None,
        attacker_player_id: None,
        at_tick: 900,
    });
    app.update();

    let lifecycle = app.world().entity(entity).get::<Lifecycle>().unwrap();
    assert_eq!(
            lifecycle.state,
            LifecycleState::AwaitingRevival,
            "期望仍是 AwaitingRevival 因为死亡屏等待决策期间不应接受新死亡事件重入把状态拍回 AwaitingRevival；实际 {:?}",
            lifecycle.state
        );
    assert_eq!(
        lifecycle.revival_decision_deadline_tick,
        Some(999),
        "期望死亡屏 60s 确认窗口 deadline 保持不变（不被新死亡事件打断/重置）；实际 {:?}",
        lifecycle.revival_decision_deadline_tick
    );
    assert_eq!(
        lifecycle.death_count, 1,
        "期望 death_count 不因重入死亡事件而递增；实际 {}",
        lifecycle.death_count
    );

    let life_record = app.world().entity(entity).get::<LifeRecord>().unwrap();
    assert_eq!(
        life_record.biography.len(),
        biography_len_before,
        "期望 biography 不新增 Death 条目因为守卫应在 push 之前 continue；实际长度 {}",
        life_record.biography.len()
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn death_arbiter_tick_auto_releases_morph_state_on_death() {
    // plan-race-system-v1 P4 opus verifier MAJOR — 死亡三条易形自动解除触发路径
    // 之一（见 death_arbiter_tick 内 release_morph_state deferred command）此前
    // 零测试断言真被 remove。走真实事件 → 真实 system → 真实 Commands flush
    // （单次 app.update() 后 Bevy 自动 apply_deferred，见 `combat::lifecycle`
    // 模块内其余测试同款依赖 —— DeathDropAnchor 断言同一 tick 内可见的既有惯例）。
    let mut app = App::new();
    let (settings, root) = persistence_settings("morph-auto-release-death");
    app.insert_resource(settings);
    app.insert_resource(CombatClock { tick: 950 });
    app.add_event::<DeathEvent>();
    app.add_event::<CultivationDeathTrigger>();
    app.add_event::<DeathInsightRequested>();
    app.add_event::<PlayerTerminated>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<QiTransfer>();
    app.add_systems(Update, death_arbiter_tick);

    let entity = app
        .world_mut()
        .spawn((
            bong_server::body_plan::MorphState::new(
                bong_server::body_plan::RaceId::new("whale"),
                0,
                900,
            ),
            Wounds {
                health_current: 0.0,
                health_max: 30.0,
                entries: Vec::new(),
            },
            Stamina::default(),
            CombatState::default(),
            Lifecycle::default(),
        ))
        .id();

    assert!(
        app.world()
            .entity(entity)
            .get::<bong_server::body_plan::MorphState>()
            .is_some(),
        "前置条件：死亡前应处于易形态"
    );

    app.world_mut().send_event(DeathEvent {
        target: entity,
        cause: "test_death".to_string(),
        attacker: None,
        attacker_player_id: None,
        at_tick: 950,
    });
    app.update();

    assert!(
        app.world()
            .entity(entity)
            .get::<bong_server::body_plan::MorphState>()
            .is_none(),
        "死亡应通过 release_morph_state 的 deferred command 移除 MorphState \
             （单次 app.update() 后 Commands 已 flush），实测组件仍在场"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn death_arbiter_skips_cultivation_death_trigger_reentry_while_awaiting_revival() {
    // 同上，覆盖 cultivation_deaths 事件循环的守卫（第二处跳过点，独立于 DeathEvent 路径）。
    let mut app = App::new();
    let (settings, root) = persistence_settings("awaiting-revival-skip-cultivation-trigger");
    app.insert_resource(settings);
    app.insert_resource(CombatClock { tick: 900 });
    app.add_event::<DeathEvent>();
    app.add_event::<CultivationDeathTrigger>();
    app.add_event::<DeathInsightRequested>();
    app.add_event::<PlayerTerminated>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<QiTransfer>();
    app.add_systems(Update, death_arbiter_tick);

    let mut life_record = LifeRecord::default();
    life_record.push(BiographyEntry::Death {
        cause: "prior".to_string(),
        tick: 100,
    });
    let biography_len_before = life_record.biography.len();

    let entity = app
        .world_mut()
        .spawn((
            Wounds {
                health_current: 0.0,
                health_max: 30.0,
                entries: Vec::new(),
            },
            Stamina::default(),
            CombatState::default(),
            life_record,
            Lifecycle {
                state: LifecycleState::AwaitingRevival,
                awaiting_decision: Some(RevivalDecision::Tribulation { chance: 0.5 }),
                revival_decision_deadline_tick: Some(1500),
                death_count: 2,
                ..Default::default()
            },
        ))
        .id();

    app.world_mut().send_event(CultivationDeathTrigger {
        entity,
        cause: CultivationDeathCause::NegativeZoneDrain,
        context: serde_json::json!({"zone": "rift_valley"}),
    });
    app.update();

    let lifecycle = app.world().entity(entity).get::<Lifecycle>().unwrap();
    assert_eq!(
            lifecycle.state,
            LifecycleState::AwaitingRevival,
            "期望仍是 AwaitingRevival 因为死亡屏等待决策期间不应接受新 cultivation 死亡事件重入；实际 {:?}",
            lifecycle.state
        );
    assert_eq!(
        lifecycle.revival_decision_deadline_tick,
        Some(1500),
        "期望死亡屏确认窗口 deadline 不被新 cultivation 死亡事件重置；实际 {:?}",
        lifecycle.revival_decision_deadline_tick
    );
    assert_eq!(
        lifecycle.death_count, 2,
        "期望 death_count 不因重入 cultivation 死亡事件而递增；实际 {}",
        lifecycle.death_count
    );

    let life_record = app.world().entity(entity).get::<LifeRecord>().unwrap();
    assert_eq!(
        life_record.biography.len(),
        biography_len_before,
        "期望 biography 不新增 Death 条目因为守卫应在 push 之前 continue；实际长度 {}",
        life_record.biography.len()
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn repeated_death_events_do_not_extend_revival_deadline() {
    let mut app = App::new();
    app.insert_resource(CombatClock { tick: 10 });
    let (settings, root) = persistence_settings("repeated-death");
    app.insert_resource(settings.clone());
    app.add_event::<DeathEvent>();
    app.add_event::<CultivationDeathTrigger>();
    app.add_event::<DeathInsightRequested>();
    app.add_event::<DeathCinematicPublished>();
    app.add_event::<PlayerTerminated>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<QiTransfer>();
    app.add_systems(Update, death_arbiter_tick);

    let entity = spawn_actor(
        &mut app,
        Wounds::default(),
        Stamina::default(),
        Lifecycle::default(),
    );

    app.world_mut().send_event(DeathEvent {
        target: entity,
        cause: "first".to_string(),
        attacker: None,
        attacker_player_id: None,
        at_tick: 10,
    });
    app.update();

    let first_deadline = app
        .world()
        .entity(entity)
        .get::<Lifecycle>()
        .unwrap()
        .revival_decision_deadline_tick;

    app.world_mut().resource_mut::<CombatClock>().tick = 200;
    app.world_mut().send_event(DeathEvent {
        target: entity,
        cause: "second".to_string(),
        attacker: None,
        attacker_player_id: None,
        at_tick: 200,
    });
    app.update();

    let lifecycle = app.world().entity(entity).get::<Lifecycle>().unwrap();
    assert_eq!(lifecycle.state, LifecycleState::AwaitingRevival);
    assert_eq!(lifecycle.revival_decision_deadline_tick, first_deadline);
    assert_eq!(lifecycle.death_count, 1);
    let insight_events = app.world().resource::<Events<DeathInsightRequested>>();
    let mut insight_reader = insight_events.get_reader();
    let insights: Vec<_> = insight_reader.read(insight_events).cloned().collect();
    assert_eq!(insights.len(), 1);
    assert_eq!(insights[0].payload.character_id, "unassigned:life_record");
    assert_eq!(insights[0].payload.cause, "first");
    assert_eq!(insights[0].payload.category, DeathInsightCategoryV1::Combat);

    let connection = Connection::open(settings.db_path()).expect("db should open");
    let life_event_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM life_events WHERE char_id = ?1",
            params!["unassigned:life_record"],
            |row| row.get(0),
        )
        .expect("life_events query should succeed");
    assert_eq!(life_event_count, 1);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn death_arbiter_clears_status_effects_on_death() {
    let mut app = App::new();
    app.insert_resource(CombatClock { tick: 10 });
    let (settings, root) = persistence_settings("death-clears-status-effects");
    app.insert_resource(settings);
    app.add_event::<DeathEvent>();
    app.add_event::<CultivationDeathTrigger>();
    app.add_event::<DeathInsightRequested>();
    app.add_event::<PlayerTerminated>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<QiTransfer>();
    app.add_systems(Update, death_arbiter_tick);

    let entity = spawn_actor(
        &mut app,
        Wounds::default(),
        Stamina::default(),
        Lifecycle::default(),
    );
    app.world_mut().entity_mut(entity).insert(StatusEffects {
        active: vec![ActiveStatusEffect {
            kind: StatusEffectKind::Bleeding,
            magnitude: 1.0,
            remaining_ticks: 120,
            source_pill: None,
        }],
    });

    app.world_mut().send_event(DeathEvent {
        target: entity,
        cause: "bleed_out".to_string(),
        attacker: None,
        attacker_player_id: None,
        at_tick: 10,
    });
    app.update();

    let lifecycle = app.world().entity(entity).get::<Lifecycle>().unwrap();
    assert_eq!(lifecycle.state, LifecycleState::AwaitingRevival);
    let statuses = app.world().entity(entity).get::<StatusEffects>().unwrap();
    assert!(statuses.active.is_empty());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn missing_death_registry_uses_lifecycle_death_count_for_tribulation_stage() {
    let mut app = App::new();
    let (settings, root) = persistence_settings("lifecycle-count-without-registry");
    app.insert_resource(settings);
    app.insert_resource(CombatClock { tick: 200 });
    app.add_event::<DeathEvent>();
    app.add_event::<CultivationDeathTrigger>();
    app.add_event::<DeathInsightRequested>();
    app.add_event::<DeathCinematicPublished>();
    app.add_event::<PlayerTerminated>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<QiTransfer>();
    app.add_systems(Update, death_arbiter_tick);

    let entity = app
        .world_mut()
        .spawn((
            Wounds::default(),
            Stamina::default(),
            CombatState::default(),
            Lifecycle {
                character_id: "offline:FourthDeath".to_string(),
                death_count: 3,
                fortune_remaining: 3,
                last_death_tick: Some(1),
                ..Default::default()
            },
            LifeRecord::new("offline:FourthDeath"),
        ))
        .id();

    app.world_mut().send_event(DeathEvent {
        target: entity,
        cause: "bleed_out".to_string(),
        attacker: None,
        attacker_player_id: None,
        at_tick: 200,
    });
    app.update();

    let lifecycle = app.world().entity(entity).get::<Lifecycle>().unwrap();
    assert_eq!(lifecycle.state, LifecycleState::AwaitingRevival);
    assert_eq!(lifecycle.death_count, 4);

    let insight_events = app.world().resource::<Events<DeathInsightRequested>>();
    let mut insight_reader = insight_events.get_reader();
    let insights: Vec<_> = insight_reader.read(insight_events).cloned().collect();
    assert_eq!(insights.len(), 1);
    let payload = &insights[0].payload;
    assert_eq!(payload.character_id, "offline:FourthDeath");
    assert_eq!(payload.death_count, 4);
    assert_eq!(payload.category, DeathInsightCategoryV1::Tribulation);
    assert_eq!(payload.rebirth_chance, Some(0.65));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn natural_aging_death_emits_natural_death_insight_request() {
    let mut app = App::new();
    let (settings, root) = persistence_settings("natural-aging-insight");
    app.insert_resource(settings.clone());
    app.insert_resource(CombatClock { tick: 440 });
    app.add_event::<DeathEvent>();
    app.add_event::<CultivationDeathTrigger>();
    app.add_event::<DeathInsightRequested>();
    app.add_event::<DeathCinematicPublished>();
    app.add_event::<PlayerTerminated>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<QiTransfer>();
    app.add_systems(Update, death_arbiter_tick);

    let entity = app
        .world_mut()
        .spawn((
            Wounds::default(),
            Stamina::default(),
            CombatState::default(),
            Lifecycle {
                character_id: "offline:Ancestor".to_string(),
                death_count: 4,
                fortune_remaining: 0,
                last_death_tick: Some(300),
                ..Default::default()
            },
            Cultivation {
                realm: Realm::Condense,
                ..Default::default()
            },
            LifeRecord::new("offline:Ancestor"),
            DeathRegistry {
                char_id: "offline:Ancestor".to_string(),
                death_count: 4,
                last_death_tick: Some(300),
                prev_death_tick: None,
                last_death_zone: Some(ZoneDeathKind::Ordinary),
            },
            LifespanComponent {
                born_at_tick: 0,
                years_lived: 349.0,
                cap_by_realm: LifespanCapTable::CONDENSE,
                offline_pause_tick: None,
            },
            Position::new([9.0, 80.0, -3.0]),
        ))
        .id();

    app.world_mut().send_event(CultivationDeathTrigger {
        entity,
        cause: CultivationDeathCause::NaturalAging,
        context: serde_json::json!({"source": "lifespan_tick"}),
    });
    app.update();

    let lifecycle = app.world().entity(entity).get::<Lifecycle>().unwrap();
    assert_eq!(lifecycle.state, LifecycleState::Terminated);
    assert_eq!(lifecycle.death_count, 5);
    assert_eq!(lifecycle.last_death_tick, Some(440));
    let lifespan = app
        .world()
        .entity(entity)
        .get::<LifespanComponent>()
        .expect("lifespan should remain attached");
    assert_eq!(lifespan.years_lived, LifespanCapTable::CONDENSE as f64);
    assert_eq!(lifespan.remaining_years(), 0.0);
    let insight_events = app.world().resource::<Events<DeathInsightRequested>>();
    let mut insight_reader = insight_events.get_reader();
    let insights: Vec<_> = insight_reader.read(insight_events).cloned().collect();
    assert_eq!(insights.len(), 1);
    let payload = &insights[0].payload;
    assert_eq!(payload.v, 1);
    assert_eq!(payload.character_id, "offline:Ancestor");
    assert_eq!(payload.cause, "cultivation:NaturalAging");
    assert_eq!(payload.category, DeathInsightCategoryV1::Natural);
    assert_eq!(payload.realm.as_deref(), Some("Condense"));
    assert_eq!(payload.death_count, 5);
    assert_eq!(payload.lifespan_remaining_years, Some(0.0));
    assert_eq!(payload.zone_kind, DeathInsightZoneKindV1::Ordinary);
    assert_eq!(payload.context["will_terminate"], true);

    let connection = Connection::open(settings.db_path()).expect("db should open");
    let death_registry: (i64, i64, String) = connection
            .query_row(
                "SELECT death_count, last_death_tick, last_death_cause FROM death_registry WHERE char_id = ?1",
                params!["offline:Ancestor"],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("natural end should persist death registry");
    let lifespan_events: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM lifespan_events WHERE char_id = ?1 AND event_type = 'death_penalty'",
                params!["offline:Ancestor"],
                |row| row.get(0),
            )
            .expect("lifespan event count should be readable");
    assert_eq!(
        death_registry,
        (5, 440, "cultivation:NaturalAging".to_string())
    );
    assert_eq!(lifespan_events, 0);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn void_quota_exceeded_cultivation_death_terminates_without_lifespan_penalty() {
    let mut app = App::new();
    let (settings, root) = persistence_settings("void-quota-exceeded");
    app.insert_resource(settings.clone());
    app.insert_resource(CombatClock { tick: 300 });
    app.add_event::<DeathEvent>();
    app.add_event::<CultivationDeathTrigger>();
    app.add_event::<DeathInsightRequested>();
    app.add_event::<DeathCinematicPublished>();
    app.add_event::<PlayerTerminated>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<QiTransfer>();
    app.add_systems(Update, death_arbiter_tick);

    let entity = app
        .world_mut()
        .spawn((
            Wounds::default(),
            Stamina::default(),
            CombatState::default(),
            Lifecycle {
                character_id: "offline:Azure".to_string(),
                ..Default::default()
            },
            Cultivation {
                realm: Realm::Spirit,
                ..Default::default()
            },
            LifeRecord::new("offline:Azure"),
            DeathRegistry::new("offline:Azure"),
            LifespanComponent {
                born_at_tick: 0,
                years_lived: 80.0,
                cap_by_realm: LifespanCapTable::SPIRIT,
                offline_pause_tick: None,
            },
            Position::new([0.0, 66.0, 0.0]),
        ))
        .id();

    app.world_mut().send_event(CultivationDeathTrigger {
        entity,
        cause: CultivationDeathCause::VoidQuotaExceeded,
        context: serde_json::json!({"reason": "void_quota_exceeded"}),
    });
    app.update();

    let lifecycle = app.world().entity(entity).get::<Lifecycle>().unwrap();
    assert_eq!(lifecycle.state, LifecycleState::Terminated);
    assert_eq!(lifecycle.death_count, 1);
    assert_eq!(lifecycle.last_death_tick, Some(300));
    let life_record = app.world().entity(entity).get::<LifeRecord>().unwrap();
    assert!(matches!(
        life_record.biography.last(),
        Some(BiographyEntry::Terminated { cause, tick })
            if cause == bong_server::cultivation::tribulation::VOID_QUOTA_EXCEEDED_REASON
                && *tick == 300
    ));
    let lifespan = app
        .world()
        .entity(entity)
        .get::<LifespanComponent>()
        .expect("lifespan should remain attached");
    assert_eq!(lifespan.years_lived, 80.0);
    assert_eq!(app.world().resource::<Events<PlayerTerminated>>().len(), 1);

    let insight_events = app.world().resource::<Events<DeathInsightRequested>>();
    let mut insight_reader = insight_events.get_reader();
    let insights: Vec<_> = insight_reader.read(insight_events).cloned().collect();
    assert_eq!(insights.len(), 1);
    let payload = &insights[0].payload;
    assert_eq!(payload.character_id, "offline:Azure");
    assert_eq!(payload.cause, "cultivation:VoidQuotaExceeded");
    assert_eq!(payload.category, DeathInsightCategoryV1::Cultivation);
    assert_eq!(payload.context["will_terminate"], true);

    let connection = Connection::open(settings.db_path()).expect("db should open");
    let death_registry: (i64, i64, String) = connection
            .query_row(
                "SELECT death_count, last_death_tick, last_death_cause FROM death_registry WHERE char_id = ?1",
                params!["offline:Azure"],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("void-quota death should persist death registry");
    let lifespan_events: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM lifespan_events WHERE char_id = ?1 AND event_type = 'death_penalty'",
                params!["offline:Azure"],
                |row| row.get(0),
            )
            .expect("lifespan event count should be readable");
    assert_eq!(
        death_registry,
        (1, 300, "cultivation:VoidQuotaExceeded".to_string())
    );
    assert_eq!(lifespan_events, 0);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn void_action_backlash_records_dedicated_termination_cause() {
    let mut app = App::new();
    let (settings, root) = persistence_settings("void-action-backlash");
    app.insert_resource(settings);
    app.insert_resource(CombatClock { tick: 320 });
    app.add_event::<DeathEvent>();
    app.add_event::<CultivationDeathTrigger>();
    app.add_event::<DeathInsightRequested>();
    app.add_event::<DeathCinematicPublished>();
    app.add_event::<PlayerTerminated>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<QiTransfer>();
    app.add_systems(Update, death_arbiter_tick);

    let entity = app
        .world_mut()
        .spawn((
            Wounds::default(),
            Stamina::default(),
            CombatState::default(),
            Lifecycle {
                character_id: "offline:Void".to_string(),
                ..Default::default()
            },
            Cultivation {
                realm: Realm::Void,
                ..Default::default()
            },
            LifeRecord::new("offline:Void"),
            DeathRegistry::new("offline:Void"),
            LifespanComponent {
                born_at_tick: 0,
                years_lived: LifespanCapTable::VOID as f64,
                cap_by_realm: LifespanCapTable::VOID,
                offline_pause_tick: None,
            },
            Position::new([0.0, 66.0, 0.0]),
        ))
        .id();

    app.world_mut().send_event(CultivationDeathTrigger {
        entity,
        cause: CultivationDeathCause::VoidActionBacklash,
        context: serde_json::json!({"kind": "barrier"}),
    });
    app.update();

    let life_record = app.world().entity(entity).get::<LifeRecord>().unwrap();
    assert!(matches!(
        life_record.biography.last(),
        Some(BiographyEntry::Terminated { cause, tick })
            if cause == "void_action_backlash" && *tick == 320
    ));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn negative_zone_death_insight_is_classified_as_tribulation_before_fourth_death() {
    let mut app = App::new();
    let (settings, root) = persistence_settings("negative-zone-tribulation-insight");
    app.insert_resource(settings);
    app.insert_resource(CombatClock { tick: 120 });
    app.add_event::<DeathEvent>();
    app.add_event::<CultivationDeathTrigger>();
    app.add_event::<DeathInsightRequested>();
    app.add_event::<DeathCinematicPublished>();
    app.add_event::<PlayerTerminated>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<QiTransfer>();
    app.add_systems(Update, death_arbiter_tick);

    let entity = app
        .world_mut()
        .spawn((
            Wounds::default(),
            Stamina::default(),
            CombatState::default(),
            Lifecycle {
                character_id: "offline:DepthWalker".to_string(),
                fortune_remaining: 3,
                ..Default::default()
            },
            LifeRecord::new("offline:DepthWalker"),
            DeathRegistry::new("offline:DepthWalker"),
            Position::new([3.0, 55.0, -7.0]),
        ))
        .id();

    app.world_mut().send_event(DeathEvent {
        target: entity,
        cause: "negative_zone_drain".to_string(),
        attacker: None,
        attacker_player_id: None,
        at_tick: 120,
    });
    app.update();

    let insight_events = app.world().resource::<Events<DeathInsightRequested>>();
    let mut insight_reader = insight_events.get_reader();
    let insights: Vec<_> = insight_reader.read(insight_events).cloned().collect();
    assert_eq!(insights.len(), 1);
    let payload = &insights[0].payload;
    assert_eq!(payload.character_id, "offline:DepthWalker");
    assert_eq!(payload.death_count, 1);
    assert_eq!(payload.category, DeathInsightCategoryV1::Tribulation);
    assert_eq!(payload.zone_kind, DeathInsightZoneKindV1::Negative);
    assert_eq!(payload.rebirth_chance, Some(0.80));

    let lifecycle = app.world().entity(entity).get::<Lifecycle>().unwrap();
    assert_eq!(lifecycle.state, LifecycleState::AwaitingRevival);
    assert_eq!(lifecycle.death_count, 1);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn death_penalty_exhaustion_persists_registry_and_lifespan_event_before_termination() {
    let mut app = App::new();
    let (settings, root) = persistence_settings("death-penalty-exhaustion");
    app.insert_resource(settings.clone());
    app.insert_resource(CombatClock { tick: 240 });
    app.add_event::<DeathEvent>();
    app.add_event::<CultivationDeathTrigger>();
    app.add_event::<DeathInsightRequested>();
    app.add_event::<DeathCinematicPublished>();
    app.add_event::<PlayerTerminated>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<QiTransfer>();
    app.add_systems(Update, death_arbiter_tick);

    let entity = app
        .world_mut()
        .spawn((
            Wounds::default(),
            Stamina::default(),
            CombatState::default(),
            Lifecycle {
                character_id: "offline:ShortLived".to_string(),
                ..Default::default()
            },
            Cultivation {
                realm: Realm::Awaken,
                ..Default::default()
            },
            LifeRecord::new("offline:ShortLived"),
            DeathRegistry::new("offline:ShortLived"),
            LifespanComponent {
                born_at_tick: 0,
                years_lived: LifespanCapTable::AWAKEN as f64 - 1.0,
                cap_by_realm: LifespanCapTable::AWAKEN,
                offline_pause_tick: None,
            },
            Position::new([2.0, 70.0, 2.0]),
        ))
        .id();

    app.world_mut().send_event(DeathEvent {
        target: entity,
        cause: "bleed_out".to_string(),
        attacker: None,
        attacker_player_id: None,
        at_tick: 240,
    });
    app.update();

    let lifecycle = app.world().entity(entity).get::<Lifecycle>().unwrap();
    assert_eq!(lifecycle.state, LifecycleState::Terminated);
    assert_eq!(lifecycle.death_count, 1);
    let lifespan = app
        .world()
        .entity(entity)
        .get::<LifespanComponent>()
        .expect("lifespan should remain attached");
    assert_eq!(lifespan.remaining_years(), 0.0);

    let connection = Connection::open(settings.db_path()).expect("db should open");
    let death_registry: (i64, i64, String) = connection
            .query_row(
                "SELECT death_count, last_death_tick, last_death_cause FROM death_registry WHERE char_id = ?1",
                params!["offline:ShortLived"],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("death penalty exhaustion should persist death registry");
    let lifespan_payload_json: String = connection
            .query_row(
                "SELECT payload_json FROM lifespan_events WHERE char_id = ?1 AND event_type = 'death_penalty'",
                params!["offline:ShortLived"],
                |row| row.get(0),
            )
            .expect("death penalty lifespan event should persist");
    let lifespan_payload: LifespanEventRecord =
        serde_json::from_str(&lifespan_payload_json).expect("lifespan payload should decode");
    let snapshot_json: String = connection
        .query_row(
            "SELECT snapshot_json FROM deceased_snapshots WHERE char_id = ?1",
            params!["offline:ShortLived"],
            |row| row.get(0),
        )
        .expect("deceased snapshot should persist in sqlite");
    let snapshot: DeceasedSnapshot =
        serde_json::from_str(&snapshot_json).expect("deceased snapshot should decode");

    assert_eq!(death_registry, (1, 240, "bleed_out".to_string()));
    assert_eq!(lifespan_payload.kind, "death_penalty");
    assert_eq!(lifespan_payload.delta_years, -6);
    assert_eq!(lifespan_payload.source, "bleed_out");
    assert_eq!(snapshot.lifecycle.death_count, 1);
    assert_eq!(snapshot.termination_category, "善终");
    assert!(matches!(
        snapshot.life_record.biography.last(),
        Some(BiographyEntry::Terminated { cause, tick })
            if cause == "natural_end" && *tick == 240
    ));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn terminate_action_is_ignored_for_alive_and_fortune_stage_characters() {
    let mut app = App::new();
    let (settings, root) = persistence_settings("terminate-gated");
    app.insert_resource(settings);
    app.insert_resource(CombatClock { tick: 120 });
    app.add_event::<RevivalActionIntent>();
    app.add_event::<PlayerRevived>();
    app.add_event::<PlayerTerminated>();
    app.add_event::<AscensionQuotaOpened>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<QiTransfer>();
    app.add_event::<bong_server::coffin::CoffinStateChanged>();
    app.insert_resource(WorldQiAccount::default());
    app.add_systems(Update, handle_revival_action_intents);

    let alive = app
        .world_mut()
        .spawn((
            Lifecycle {
                character_id: "offline:Alive".to_string(),
                state: LifecycleState::Alive,
                ..Default::default()
            },
            LifeRecord::new("offline:Alive"),
        ))
        .id();
    let fortune_stage = app
        .world_mut()
        .spawn((
            Lifecycle {
                character_id: "offline:Fortune".to_string(),
                state: LifecycleState::AwaitingRevival,
                awaiting_decision: Some(RevivalDecision::Fortune { chance: 1.0 }),
                revival_decision_deadline_tick: Some(200),
                ..Default::default()
            },
            LifeRecord::new("offline:Fortune"),
        ))
        .id();

    app.world_mut().send_event(RevivalActionIntent {
        entity: alive,
        action: RevivalActionKind::Terminate,
        issued_at_tick: 120,
    });
    app.world_mut().send_event(RevivalActionIntent {
        entity: fortune_stage,
        action: RevivalActionKind::Terminate,
        issued_at_tick: 120,
    });
    app.update();

    assert_eq!(
        app.world().entity(alive).get::<Lifecycle>().unwrap().state,
        LifecycleState::Alive
    );
    assert_eq!(
        app.world()
            .entity(fortune_stage)
            .get::<Lifecycle>()
            .unwrap()
            .state,
        LifecycleState::AwaitingRevival
    );
    assert_eq!(app.world().resource::<Events<PlayerTerminated>>().len(), 0);

    let _ = fs::remove_dir_all(root);
}

// ── bughunt player-lifecycle-relog-death-consequence-wipe（OPUS 返工要求 2）──
//
// 断线时正处于 AwaitingRevival 的角色重连后必须重新收到死亡屏 + DeathCinematic，不能
// 让玩家满血、无 UI 地"裸奔"在这个阻断攻防、又会被 auto_confirm_revival_decisions
// 强制结算（可能永久终结角色）的状态里。下面的用例覆盖：两个 RevivalDecision 变体各一条
// 专属 case（happy path）、Alive 不该触发的状态（负分支）、
// awaiting_decision=None 的内部不一致状态（错误分支，不panic）、以及
// Without<DeathCinematic> 过滤器的防重复触发保护。

fn spawn_reconnected_client_actor(
    app: &mut App,
    username: &str,
    lifecycle: Lifecycle,
) -> (Entity, MockClientHelper) {
    spawn_client_actor(
        app,
        username,
        Wounds {
            health_current: 30.0,
            health_max: 30.0,
            entries: Vec::new(),
        },
        Stamina::default(),
        lifecycle,
    )
}

#[test]
fn reconnect_while_alive_does_not_reemit_death_screen() {
    let mut app = App::new();
    app.insert_resource(CombatClock { tick: 500 });
    app.add_event::<DeathCinematicPublished>();
    app.add_systems(
        Update,
        reemit_death_screen_for_reconnected_awaiting_revival_clients,
    );

    let (_entity, mut helper) =
        spawn_reconnected_client_actor(&mut app, "ReconnectAlive", Lifecycle::default());

    app.update();
    flush_client_packets(&mut app);

    let payloads = collect_server_data_payloads(&mut helper);
    assert!(
        payloads.is_empty(),
        "Alive 状态（最常见的健康在线玩家重连路径）绝不应该触发死亡屏；\
             实际收到 {} 个 payload：{payloads:?}",
        payloads.len()
    );
}

#[test]
fn reconnect_while_awaiting_revival_without_pending_decision_skips_without_panicking() {
    // 状态机内部不一致：state=AwaitingRevival 却没有 awaiting_decision（正常流程不会
    // 产生这种组合，但组件是外部可写的，防御性地要求不 panic、不发送残缺 payload）。
    let mut app = App::new();
    app.insert_resource(CombatClock { tick: 500 });
    app.add_event::<DeathCinematicPublished>();
    app.add_systems(
        Update,
        reemit_death_screen_for_reconnected_awaiting_revival_clients,
    );

    let (_entity, mut helper) = spawn_reconnected_client_actor(
        &mut app,
        "ReconnectInconsistent",
        Lifecycle {
            state: LifecycleState::AwaitingRevival,
            awaiting_decision: None,
            ..Default::default()
        },
    );

    app.update();
    flush_client_packets(&mut app);

    let payloads = collect_server_data_payloads(&mut helper);
    assert!(
        payloads.is_empty(),
        "awaiting_decision=None 时没有决策可展示，不应该发送残缺的死亡屏 payload；\
             实际收到 {} 个 payload：{payloads:?}",
        payloads.len()
    );
}

#[test]
fn reconnect_skips_entities_that_already_carry_a_death_cinematic() {
    // Without<DeathCinematic> 过滤器防重复触发：如果实体在 Added<Client> 这一 tick
    // 就已经带着 DeathCinematic（例如某种未来的预取/迁移路径），本系统不应该覆盖或
    // 重复发送。
    let mut app = App::new();
    app.insert_resource(CombatClock { tick: 500 });
    app.add_event::<DeathCinematicPublished>();
    app.add_systems(
        Update,
        reemit_death_screen_for_reconnected_awaiting_revival_clients,
    );

    let (entity, mut helper) = spawn_reconnected_client_actor(
        &mut app,
        "ReconnectAlreadyCinematic",
        Lifecycle {
            state: LifecycleState::AwaitingRevival,
            awaiting_decision: Some(RevivalDecision::Fortune { chance: 1.0 }),
            revival_decision_deadline_tick: Some(560),
            ..Default::default()
        },
    );
    let pre_existing_cinematic =
        bong_server::death_lifecycle::cinematic::DeathCinematic::new(DeathCinematicInit {
            character_id: "offline:ReconnectAlreadyCinematic".to_string(),
            started_at_tick: 400,
            roll: DeathCinematicRollV1 {
                probability: 1.0,
                threshold: 1.0,
                luck_value: 1.0,
                result: DeathRollResultV1::Survive,
            },
            insight_text: vec!["既有插曲".to_string()],
            is_final: false,
            death_number: 1,
            zone_kind: DeathCinematicZoneKindV1::Ordinary,
            tsy_death: false,
        });
    app.world_mut()
        .entity_mut(entity)
        .insert(pre_existing_cinematic.clone());

    app.update();
    flush_client_packets(&mut app);

    let payloads = collect_server_data_payloads(&mut helper);
    assert!(
        payloads.is_empty(),
        "已经携带 DeathCinematic 的实体必须被 Without<DeathCinematic> 过滤掉，不应该\
             再收到一份重复的死亡屏；实际收到 {} 个 payload：{payloads:?}",
        payloads.len()
    );
    assert_eq!(
        app.world()
            .entity(entity)
            .get::<bong_server::death_lifecycle::cinematic::DeathCinematic>(),
        Some(&pre_existing_cinematic),
        "既有 DeathCinematic 不应该被覆盖"
    );
}

#[test]
fn create_new_character_uses_distinct_character_ids_for_deceased_snapshots() {
    let mut app = App::new();
    let (settings, root) = persistence_settings("new-character-deceased-unique");
    let data_dir = root.join("data");
    app.insert_resource(settings.clone());
    app.insert_resource(PlayerStatePersistence::with_db_path(
        &data_dir,
        settings.db_path(),
    ));
    app.insert_resource(CombatClock { tick: 800 });

    let item_registry =
        bong_server::inventory::load_item_registry().expect("item registry should load");
    let default_loadout = bong_server::inventory::load_default_loadout(&item_registry)
        .expect("default loadout should load");
    app.insert_resource(DefaultLoadout(default_loadout));
    // plan-layered-equip-v1 P0.6 — reset_for_new_character 现需 ItemRegistry 重建 inventory。
    app.insert_resource(item_registry);
    app.insert_resource(InventoryInstanceIdAllocator::default());

    app.add_event::<RevivalActionIntent>();
    app.add_event::<PlayerRevived>();
    app.add_event::<PlayerTerminated>();
    app.add_event::<AscensionQuotaOpened>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<QiTransfer>();
    app.add_event::<bong_server::coffin::CoffinStateChanged>();
    app.insert_resource(WorldQiAccount::default());
    app.add_systems(Update, handle_revival_action_intents);

    let username = Username("Azure".to_string());
    save_player_slices(
        &PlayerStatePersistence::with_db_path(&data_dir, settings.db_path()),
        username.0.as_str(),
        &PlayerState::default(),
        bong_server::player::spawn_position(),
        DimensionKind::default(),
        None,
        None,
        &SkillSet::default(),
    )
    .expect("initial player slices should persist");

    let entity = app
        .world_mut()
        .spawn((
            Wounds::default(),
            Stamina::default(),
            CombatState::default(),
            Lifecycle {
                character_id: "offline:Ancestor".to_string(),
                state: LifecycleState::Terminated,
                ..Default::default()
            },
            LifeRecord::new("offline:Ancestor"),
            DeathRegistry::new("offline:Ancestor"),
            LifespanComponent::new(LifespanCapTable::MORTAL),
            PlayerState::default(),
            Position::new(bong_server::player::spawn_position()),
            username.clone(),
            SkillSet::default(),
        ))
        .id();

    app.world_mut().send_event(RevivalActionIntent {
        entity,
        action: RevivalActionKind::CreateNewCharacter,
        issued_at_tick: 800,
    });
    app.update();
    let first_character_id = app
        .world()
        .entity(entity)
        .get::<Lifecycle>()
        .unwrap()
        .character_id
        .clone();

    {
        let mut lifecycle = app.world_mut().entity_mut(entity);
        *lifecycle.get_mut::<Lifecycle>().unwrap() = Lifecycle {
            character_id: first_character_id.clone(),
            state: LifecycleState::Terminated,
            ..Default::default()
        };
        *lifecycle.get_mut::<LifeRecord>().unwrap() = LifeRecord::new(first_character_id.clone());
    }
    app.world_mut().send_event(RevivalActionIntent {
        entity,
        action: RevivalActionKind::CreateNewCharacter,
        issued_at_tick: 801,
    });
    app.update();
    let second_character_id = app
        .world()
        .entity(entity)
        .get::<Lifecycle>()
        .unwrap()
        .character_id
        .clone();

    assert_ne!(first_character_id, second_character_id);

    let mut first_lifecycle = Lifecycle {
        character_id: first_character_id.clone(),
        state: LifecycleState::Terminated,
        ..Default::default()
    };
    let mut first_life_record = LifeRecord::new(first_character_id.clone());
    first_life_record.push(BiographyEntry::Terminated {
        cause: "voluntary_retire".to_string(),
        tick: 900,
    });
    first_lifecycle.terminate(900);
    persist_termination_transition(&settings, &first_lifecycle, &first_life_record)
        .expect("first terminated snapshot should persist");

    let mut second_lifecycle = Lifecycle {
        character_id: second_character_id.clone(),
        state: LifecycleState::Terminated,
        ..Default::default()
    };
    let mut second_life_record = LifeRecord::new(second_character_id.clone());
    second_life_record.push(BiographyEntry::Terminated {
        cause: "voluntary_retire".to_string(),
        tick: 901,
    });
    second_lifecycle.terminate(901);
    persist_termination_transition(&settings, &second_lifecycle, &second_life_record)
        .expect("second terminated snapshot should persist");

    let connection = Connection::open(settings.db_path()).expect("db should open");
    let snapshot_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM deceased_snapshots WHERE char_id IN (?1, ?2)",
            params![first_character_id, second_character_id],
            |row| row.get(0),
        )
        .expect("deceased snapshot rows should be queryable");
    assert_eq!(snapshot_count, 2);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn shrine_anchor_allows_fortune_stage_under_recent_death_and_high_karma() {
    let mut app = App::new();
    let (settings, root) = persistence_settings("shrine-fortune-stage");
    app.insert_resource(settings);
    app.insert_resource(CombatClock { tick: 100 });
    app.add_event::<DeathEvent>();
    app.add_event::<CultivationDeathTrigger>();
    app.add_event::<DeathInsightRequested>();
    app.add_event::<DeathCinematicPublished>();
    app.add_event::<PlayerRevived>();
    app.add_event::<PlayerTerminated>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<QiTransfer>();
    app.add_systems(Update, (death_arbiter_tick,));

    let player_state = PlayerState {
        karma: 0.9,
        inventory_score: 0.0,
    };

    let wounds = Wounds {
        health_current: 0.0,
        health_max: 30.0,
        entries: Vec::new(),
    };

    let without_shrine = app
        .world_mut()
        .spawn((
            wounds.clone(),
            Stamina::default(),
            CombatState::default(),
            Position::new([8.0, 66.0, 8.0]),
            Lifecycle {
                fortune_remaining: 1,
                spawn_anchor: None,
                ..Default::default()
            },
            DeathRegistry {
                char_id: "offline:NoShrine".to_string(),
                death_count: 1,
                // 当前死亡会在 death_arbiter_tick 内 record_death；这里模拟“上一次死亡”发生在 24h 内，
                // 使 without_shrine 不满足运数期保底条件。
                last_death_tick: Some(1),
                prev_death_tick: None,
                last_death_zone: Some(ZoneDeathKind::Ordinary),
            },
            player_state.clone(),
        ))
        .id();

    let with_shrine = app
        .world_mut()
        .spawn((
            wounds,
            Stamina::default(),
            CombatState::default(),
            Position::new([8.0, 66.0, 8.0]),
            Lifecycle {
                fortune_remaining: 1,
                spawn_anchor: Some([11.0, 22.0, 33.0]),
                ..Default::default()
            },
            DeathRegistry {
                char_id: "offline:WithShrine".to_string(),
                death_count: 1,
                last_death_tick: Some(1),
                prev_death_tick: None,
                last_death_zone: Some(ZoneDeathKind::Ordinary),
            },
            player_state,
        ))
        .id();

    app.world_mut().send_event(DeathEvent {
        target: without_shrine,
        cause: "test".to_string(),
        attacker: None,
        attacker_player_id: None,
        at_tick: 100,
    });
    app.world_mut().send_event(DeathEvent {
        target: with_shrine,
        cause: "test".to_string(),
        attacker: None,
        attacker_player_id: None,
        at_tick: 100,
    });
    app.update();

    app.world_mut().resource_mut::<CombatClock>().tick = 701;
    app.update();

    let lifecycle_without_shrine = app
        .world()
        .entity(without_shrine)
        .get::<Lifecycle>()
        .expect("lifecycle should exist");
    assert!(matches!(
        lifecycle_without_shrine.awaiting_decision,
        Some(RevivalDecision::Tribulation { chance }) if (chance - 0.80).abs() < 1e-9
    ));

    let lifecycle_with_shrine = app
        .world()
        .entity(with_shrine)
        .get::<Lifecycle>()
        .expect("lifecycle should exist");
    assert!(matches!(
        lifecycle_with_shrine.awaiting_decision,
        Some(RevivalDecision::Fortune { chance }) if (chance - 1.0).abs() < 1e-9
    ));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn npc_reincarnate_intent_fails_closed_without_owner_or_event_mutation() {
    let mut app = App::new();
    let (settings, root) = persistence_settings("npc-reincarnate-rejected");
    app.insert_resource(settings);
    app.insert_resource(CombatClock { tick: 42 });
    app.add_event::<RevivalActionIntent>();
    app.add_event::<PlayerRevived>();
    app.add_event::<PlayerTerminated>();
    app.add_event::<AscensionQuotaOpened>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<QiTransfer>();
    app.add_event::<bong_server::coffin::CoffinStateChanged>();
    app.insert_resource(WorldQiAccount::default());
    app.add_systems(Update, handle_revival_action_intents);

    let entity = app
        .world_mut()
        .spawn((
            NpcMarker,
            Lifecycle {
                character_id: "npc:revival-rejected".to_string(),
                state: LifecycleState::AwaitingRevival,
                awaiting_decision: Some(RevivalDecision::Fortune { chance: 1.0 }),
                revival_decision_deadline_tick: Some(100),
                fortune_remaining: 1,
                ..Default::default()
            },
            Cultivation {
                realm: Realm::Induce,
                qi_current: 8.0,
                qi_max: 24.0,
                ..Default::default()
            },
            MeridianSystem::default(),
            Contamination::default(),
            LifeRecord::new("npc:revival-rejected"),
        ))
        .id();
    let baseline_cultivation = app.world().get::<Cultivation>(entity).unwrap().clone();

    app.world_mut().send_event(RevivalActionIntent {
        entity,
        action: RevivalActionKind::Reincarnate,
        issued_at_tick: 42,
    });
    app.update();

    let lifecycle = app.world().get::<Lifecycle>(entity).unwrap();
    assert_eq!(lifecycle.state, LifecycleState::AwaitingRevival);
    assert_eq!(lifecycle.last_revive_tick, None);
    assert_eq!(
        bong_server::cultivation::components::encode_persisted_cultivation(
            app.world().get::<Cultivation>(entity).unwrap()
        ),
        bong_server::cultivation::components::encode_persisted_cultivation(&baseline_cultivation)
    );
    assert_eq!(app.world().resource::<WorldQiAccount>().total(), 0.0);
    assert!(app
        .world()
        .resource::<WorldQiAccount>()
        .transfers()
        .is_empty());
    assert_eq!(app.world().resource::<Events<PlayerRevived>>().len(), 0);
    assert_eq!(app.world().resource::<Events<QiTransfer>>().len(), 0);

    let _ = fs::remove_dir_all(root);
}

// ── plan-shield-block-v1 P2 §Issue5.2 — sync_combat_state ShieldBlocking 保留 ──
// 被命中时若受击方处于 ShieldBlocking 状态，sync_combat_state_from_events 不应将其
// stamina.state 翻成 Combat（应保留 ShieldBlocking，由 stamina_tick 维护 drain 逻辑）。
#[test]
fn sync_combat_state_preserves_shield_blocking_state_on_target() {
    let mut app = App::new();
    app.add_event::<CombatEvent>();
    app.add_systems(Update, sync_combat_state_from_events);

    let attacker = app
        .world_mut()
        .spawn((
            Wounds::default(),
            Stamina {
                current: 100.0,
                max: 100.0,
                recover_per_sec: 5.0,
                state: StaminaState::Combat,
                last_drain_tick: None,
            },
            CombatState::default(),
            Lifecycle::default(),
        ))
        .id();
    let target = app
        .world_mut()
        .spawn((
            Wounds::default(),
            Stamina {
                current: 60.0,
                max: 100.0,
                recover_per_sec: 5.0,
                state: StaminaState::ShieldBlocking,
                last_drain_tick: None,
            },
            CombatState::default(),
            Lifecycle::default(),
        ))
        .id();

    app.world_mut().send_event(CombatEvent {
        attacker,
        target,
        resolved_at_tick: 100,
        body_part: BodyPart::Chest,
        wound_kind: WoundKind::Blunt,
        source: bong_server::combat::events::AttackSource::Melee,
        debug_command: false,
        physical_damage: 0.5,
        damage: 0.0,
        contam_delta: 0.0,
        description: "test_hit".to_string(),
        defense_kind: Some(bong_server::combat::events::DefenseKind::ShieldBlock),
        defense_effectiveness: Some(0.6),
        defense_contam_reduced: None,
        defense_wound_severity: None,
    });
    app.update();

    let target_stamina = app.world().entity(target).get::<Stamina>().unwrap();
    assert_eq!(
        target_stamina.state,
        StaminaState::ShieldBlocking,
        "sync_combat_state_from_events 被命中时不应将 ShieldBlocking 状态覆写为 Combat；\
             举盾状态由 stamina_tick 维护（drain/exhausted 逻辑）；\
             actual: {:?}",
        target_stamina.state
    );
}

// ─────────────── P0 fix: revive/new_char 清 coffin 状态 ───────────────
//
// 覆盖 r5-P0 修复：入棺玩家复活/新建角色后 coffin 状态必须彻底清除。
// 三件套：CoffinComponent（ECS）+ CoffinRegistry + CoffinStateChanged 事件。
// 持久化层（SQLite persist_in_coffin）在无 PlayerStatePersistence 时静默跳过，
// 单测靠 CoffinRegistry + ECS 断言可观察行为。

fn make_coffin_registry_with_player(player: Entity) -> bong_server::coffin::CoffinRegistry {
    let lower = valence::prelude::BlockPos::new(10, 64, 10);
    let mut registry = bong_server::coffin::CoffinRegistry::default();
    registry.insert(lower, 0, bong_server::coffin::CoffinGrade::Mundane);
    registry.set_occupied(lower, player);
    registry
}

fn coffin_setup_base(app: &mut App, tick: u64, test_name: &str) -> (PersistenceSettings, PathBuf) {
    let (settings, root) = persistence_settings(test_name);
    app.insert_resource(settings.clone());
    app.insert_resource(CombatClock { tick });
    app.add_event::<RevivalActionIntent>();
    app.add_event::<PlayerRevived>();
    app.add_event::<PlayerTerminated>();
    app.add_event::<AscensionQuotaOpened>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<QiTransfer>();
    app.add_event::<bong_server::coffin::CoffinStateChanged>();
    app.insert_resource(WorldQiAccount::default());
    app.add_systems(Update, handle_revival_action_intents);
    (settings, root)
}

/// 入棺玩家复活后：
///   - CoffinComponent 从 entity 移除（期望：None，因为复活后不应继续锁棺）
///   - CoffinRegistry.player_in_coffin 不含该 entity（期望：None，因为 clear_player 清双索引）
///   - CoffinStateChanged 事件被发出 grade=None（期望：收到 1 条，因为玩家确实在棺内）
#[test]
fn create_new_character_clears_coffin_state_and_emits_state_changed() {
    let mut app = App::new();
    let (settings, root) = persistence_settings("coffin-clear-new-char");
    let data_dir = root.join("data");
    app.insert_resource(settings.clone());
    app.insert_resource(PlayerStatePersistence::with_db_path(
        &data_dir,
        settings.db_path(),
    ));
    app.insert_resource(CombatClock { tick: 800 });
    let item_registry =
        bong_server::inventory::load_item_registry().expect("item registry should load");
    let default_loadout = bong_server::inventory::load_default_loadout(&item_registry)
        .expect("default loadout should load");
    app.insert_resource(DefaultLoadout(default_loadout));
    // plan-layered-equip-v1 P0.6 — reset_for_new_character 现需 ItemRegistry 重建 inventory。
    app.insert_resource(item_registry);
    app.insert_resource(InventoryInstanceIdAllocator::default());
    app.add_event::<RevivalActionIntent>();
    app.add_event::<PlayerRevived>();
    app.add_event::<PlayerTerminated>();
    app.add_event::<AscensionQuotaOpened>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<QiTransfer>();
    app.add_event::<bong_server::coffin::CoffinStateChanged>();
    app.insert_resource(WorldQiAccount::default());
    app.add_systems(Update, handle_revival_action_intents);

    let entity = app
        .world_mut()
        .spawn((
            Wounds::default(),
            Stamina::default(),
            CombatState::default(),
            Lifecycle {
                character_id: "offline:CoffinNewChar".to_string(),
                state: LifecycleState::Terminated,
                ..Default::default()
            },
            LifeRecord::new("offline:CoffinNewChar"),
            DeathRegistry::new("offline:CoffinNewChar"),
            LifespanComponent {
                born_at_tick: 0,
                years_lived: 50.0,
                cap_by_realm: bong_server::cultivation::lifespan::LifespanCapTable::AWAKEN,
                offline_pause_tick: None,
            },
            Cultivation::default(),
            MeridianSystem::default(),
            bong_server::coffin::CoffinComponent {
                entered_at_tick: 700,
                coffin_lower: valence::prelude::BlockPos::new(20, 64, 20),
                grade: bong_server::coffin::CoffinGrade::Jade,
            },
        ))
        .id();

    let registry = make_coffin_registry_with_player(entity);
    app.insert_resource(registry);

    app.world_mut().send_event(RevivalActionIntent {
        entity,
        action: RevivalActionKind::CreateNewCharacter,
        issued_at_tick: 800,
    });
    app.update();

    // CoffinComponent 应已从 entity 移除（新建角色不继承旧棺状态）
    assert!(
        app.world()
            .entity(entity)
            .get::<bong_server::coffin::CoffinComponent>()
            .is_none(),
        "期望 CoffinComponent=None（新建角色后不锁棺），实际仍有 CoffinComponent"
    );

    // CoffinRegistry.player_in_coffin 应清空
    let reg = app
        .world()
        .resource::<bong_server::coffin::CoffinRegistry>();
    assert!(
        !reg.player_in_coffin.contains_key(&entity),
        "期望新建角色后 player_in_coffin 不含 entity，实际仍含"
    );

    // CoffinStateChanged(grade=None) 应被发送（玩家确实在棺内 → clear_player 返回 Some）
    let state_events = app
        .world_mut()
        .resource_mut::<Events<bong_server::coffin::CoffinStateChanged>>()
        .drain()
        .collect::<Vec<_>>();
    assert_eq!(
        state_events.len(),
        1,
        "期望新建角色发出 1 条 CoffinStateChanged（玩家在棺内），实际 {} 条",
        state_events.len()
    );
    assert!(
        state_events[0].grade.is_none(),
        "期望 CoffinStateChanged.grade=None（离棺），实际 {:?}",
        state_events[0].grade
    );

    let _ = fs::remove_dir_all(root);
}

// ─────────── must_fix #1&#2: terminate 路径清 coffin（ECS + Registry + CoffinStateChanged）───────────

/// 劫数不过（tribulation_failed）→ terminate_lifecycle 后 coffin 状态必须全部清除：
///   - CoffinComponent 从 entity 移除（期望：None）
///   - CoffinRegistry.player_in_coffin 不含该 entity（期望：None）
///   - CoffinStateChanged(grade=None) 被发出（期望：1 条）
#[test]
fn tribulation_failed_terminate_clears_coffin_state() {
    let mut app = App::new();
    let (_settings, _root) = coffin_setup_base(&mut app, 600, "tribulation-failed-coffin-clear");
    // 注：coffin_setup_base 不预插 CoffinRegistry，需手动 insert 后再 set_occupied
    app.insert_resource(bong_server::coffin::CoffinRegistry::default());

    let lower = valence::prelude::BlockPos::new(15, 64, 15);
    let entity = app
        .world_mut()
        .spawn((
            Wounds {
                health_current: 1.0,
                health_max: 30.0,
                entries: Vec::new(),
            },
            Stamina::default(),
            CombatState::default(),
            Lifecycle {
                character_id: "offline:TribFail".to_string(),
                state: LifecycleState::AwaitingRevival,
                // 劫数决策，chance=0 → roll_rebirth 必然返回 false → 走 terminate 分支
                awaiting_decision: Some(RevivalDecision::Tribulation { chance: 0.0 }),
                revival_decision_deadline_tick: Some(700),
                fortune_remaining: 0,
                ..Default::default()
            },
            Cultivation {
                realm: Realm::Awaken,
                qi_current: 10.0,
                qi_max: 100.0,
                ..Default::default()
            },
            MeridianSystem::default(),
            Contamination::default(),
            LifeRecord::new("offline:TribFail"),
            bong_server::coffin::CoffinComponent {
                entered_at_tick: 550,
                coffin_lower: lower,
                grade: bong_server::coffin::CoffinGrade::Mundane,
            },
        ))
        .id();

    {
        let mut reg = app
            .world_mut()
            .resource_mut::<bong_server::coffin::CoffinRegistry>();
        reg.insert(lower, 0, bong_server::coffin::CoffinGrade::Mundane);
        reg.set_occupied(lower, entity);
    }

    // 发 Reincarnate；因 chance=0 roll 必失 → 走 terminate 分支
    app.world_mut().send_event(RevivalActionIntent {
        entity,
        action: RevivalActionKind::Reincarnate,
        issued_at_tick: 600,
    });
    app.update();

    // 验证：CoffinComponent 已移除
    assert!(
        app.world()
            .entity(entity)
            .get::<bong_server::coffin::CoffinComponent>()
            .is_none(),
        "期望 tribulation_failed 后 CoffinComponent=None，实际仍存在"
    );

    // 验证：Registry 已清空
    let reg = app
        .world()
        .resource::<bong_server::coffin::CoffinRegistry>();
    assert!(
        !reg.player_in_coffin.contains_key(&entity),
        "期望 tribulation_failed 后 player_in_coffin 不含 entity，实际仍含"
    );

    // 验证：CoffinStateChanged(grade=None) 被发出
    let state_events = app
        .world_mut()
        .resource_mut::<Events<bong_server::coffin::CoffinStateChanged>>()
        .drain()
        .collect::<Vec<_>>();
    assert_eq!(
        state_events.len(),
        1,
        "期望 tribulation_failed 发出 1 条 CoffinStateChanged，实际 {} 条",
        state_events.len()
    );
    assert!(
        state_events[0].grade.is_none(),
        "期望 CoffinStateChanged.grade=None（离棺），实际 {:?}",
        state_events[0].grade
    );
}

/// 主动归隐（voluntary_retire / Terminate 决策）后 coffin 状态必须全部清除：
///   - CoffinComponent 从 entity 移除（期望：None）
///   - CoffinRegistry.player_in_coffin 不含该 entity（期望：None）
///   - CoffinStateChanged(grade=None) 被发出（期望：1 条）
#[test]
fn voluntary_retire_terminate_clears_coffin_state() {
    let mut app = App::new();
    let (_settings, _root) = coffin_setup_base(&mut app, 700, "voluntary-retire-coffin-clear");
    app.insert_resource(bong_server::coffin::CoffinRegistry::default());

    let lower = valence::prelude::BlockPos::new(25, 64, 25);
    let entity = app
        .world_mut()
        .spawn((
            Wounds {
                health_current: 1.0,
                health_max: 30.0,
                entries: Vec::new(),
            },
            Stamina::default(),
            CombatState::default(),
            Lifecycle {
                character_id: "offline:VolRetire".to_string(),
                state: LifecycleState::AwaitingRevival,
                // Tribulation 决策 + fortune_remaining=0 → can_terminate()=true
                awaiting_decision: Some(RevivalDecision::Tribulation { chance: 0.5 }),
                revival_decision_deadline_tick: Some(800),
                fortune_remaining: 0,
                ..Default::default()
            },
            Cultivation {
                realm: Realm::Awaken,
                qi_current: 10.0,
                qi_max: 100.0,
                ..Default::default()
            },
            MeridianSystem::default(),
            Contamination::default(),
            LifeRecord::new("offline:VolRetire"),
            bong_server::coffin::CoffinComponent {
                entered_at_tick: 650,
                coffin_lower: lower,
                grade: bong_server::coffin::CoffinGrade::Mundane,
            },
        ))
        .id();

    {
        let mut reg = app
            .world_mut()
            .resource_mut::<bong_server::coffin::CoffinRegistry>();
        reg.insert(lower, 0, bong_server::coffin::CoffinGrade::Mundane);
        reg.set_occupied(lower, entity);
    }

    // 发 Terminate（主动归隐）
    app.world_mut().send_event(RevivalActionIntent {
        entity,
        action: RevivalActionKind::Terminate,
        issued_at_tick: 700,
    });
    app.update();

    // 验证：CoffinComponent 已移除
    assert!(
        app.world()
            .entity(entity)
            .get::<bong_server::coffin::CoffinComponent>()
            .is_none(),
        "期望 voluntary_retire 后 CoffinComponent=None，实际仍存在"
    );

    // 验证：Registry 已清空
    let reg = app
        .world()
        .resource::<bong_server::coffin::CoffinRegistry>();
    assert!(
        !reg.player_in_coffin.contains_key(&entity),
        "期望 voluntary_retire 后 player_in_coffin 不含 entity，实际仍含"
    );

    // 验证：CoffinStateChanged(grade=None) 被发出
    let state_events = app
        .world_mut()
        .resource_mut::<Events<bong_server::coffin::CoffinStateChanged>>()
        .drain()
        .collect::<Vec<_>>();
    assert_eq!(
        state_events.len(),
        1,
        "期望 voluntary_retire 发出 1 条 CoffinStateChanged，实际 {} 条",
        state_events.len()
    );
    assert!(
        state_events[0].grade.is_none(),
        "期望 CoffinStateChanged.grade=None（离棺），实际 {:?}",
        state_events[0].grade
    );
}

// ─────────── must_fix #3a: SQLite in_coffin 持久化契约锁住（带 Username + PlayerStatePersistence）─────────

/// 劫数不过 terminate 后，SQLite in_coffin 列必须被写为 false（0）。
/// 同 revive_with_username_clears_sqlite_in_coffin，但走 terminate 路径。
#[test]
fn terminate_tribulation_failed_with_username_clears_sqlite_in_coffin() {
    let mut app = App::new();
    let (settings, root) = persistence_settings("sqlite-coffin-term");
    let data_dir = root.join("data");

    app.insert_resource(settings.clone());
    app.insert_resource(PlayerStatePersistence::with_db_path(
        &data_dir,
        settings.db_path(),
    ));
    app.insert_resource(CombatClock { tick: 600 });
    app.add_event::<RevivalActionIntent>();
    app.add_event::<PlayerRevived>();
    app.add_event::<PlayerTerminated>();
    app.add_event::<AscensionQuotaOpened>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<QiTransfer>();
    app.add_event::<bong_server::coffin::CoffinStateChanged>();
    app.insert_resource(WorldQiAccount::default());
    app.add_systems(Update, handle_revival_action_intents);

    let username = Username("SQLiteCoffinTerm".to_string());
    let lifespan = bong_server::cultivation::lifespan::LifespanComponent {
        born_at_tick: 0,
        years_lived: 80.0,
        cap_by_realm: bong_server::cultivation::lifespan::LifespanCapTable::AWAKEN,
        offline_pause_tick: None,
    };
    let lower = valence::prelude::BlockPos::new(50, 64, 50);

    // 先写入 in_coffin=true 到 SQLite
    bong_server::player::state::save_player_lifespan_slice_with_coffin(
        &PlayerStatePersistence::with_db_path(&data_dir, settings.db_path()),
        username.0.as_str(),
        &lifespan,
        Some(bong_server::coffin::CoffinGrade::Jade),
    )
    .expect("pre-populate in_coffin=true 应成功");

    // 前置验证
    let before = bong_server::player::state::load_player_slices(
        &PlayerStatePersistence::with_db_path(&data_dir, settings.db_path()),
        username.0.as_str(),
    );
    assert!(
        before.in_coffin,
        "前置条件：SQLite in_coffin 应为 true，实际 false"
    );

    let entity = app
        .world_mut()
        .spawn((
            Wounds {
                health_current: 1.0,
                health_max: 30.0,
                entries: Vec::new(),
            },
            Stamina::default(),
            CombatState::default(),
            Lifecycle {
                character_id: "offline:SQLiteCoffinTerm".to_string(),
                state: LifecycleState::AwaitingRevival,
                // chance=0 → roll 必失 → terminate 分支
                awaiting_decision: Some(RevivalDecision::Tribulation { chance: 0.0 }),
                revival_decision_deadline_tick: Some(700),
                fortune_remaining: 0,
                ..Default::default()
            },
            Cultivation {
                realm: Realm::Awaken,
                qi_current: 10.0,
                qi_max: 100.0,
                ..Default::default()
            },
            MeridianSystem::default(),
            Contamination::default(),
            LifeRecord::new("offline:SQLiteCoffinTerm"),
            lifespan.clone(),
            username.clone(),
            bong_server::coffin::CoffinComponent {
                entered_at_tick: 550,
                coffin_lower: lower,
                grade: bong_server::coffin::CoffinGrade::Jade,
            },
        ))
        .id();

    {
        let mut reg = bong_server::coffin::CoffinRegistry::default();
        reg.insert(lower, 0, bong_server::coffin::CoffinGrade::Jade);
        reg.set_occupied(lower, entity);
        app.insert_resource(reg);
    }

    // 触发 Reincarnate（chance=0 → 必走 terminate 分支）
    app.world_mut().send_event(RevivalActionIntent {
        entity,
        action: RevivalActionKind::Reincarnate,
        issued_at_tick: 600,
    });
    app.update();

    // 核心断言：SQLite in_coffin 必须为 false
    let after = bong_server::player::state::load_player_slices(
        &PlayerStatePersistence::with_db_path(&data_dir, settings.db_path()),
        username.0.as_str(),
    );
    assert!(
        !after.in_coffin,
        "期望 tribulation_failed terminate 后 SQLite in_coffin=false（重启不应复钉），实际 true"
    );
    assert!(
        after.coffin_grade.is_none(),
        "期望 terminate 后 SQLite coffin_grade=None，实际 {:?}",
        after.coffin_grade
    );

    let _ = fs::remove_dir_all(root);
}
