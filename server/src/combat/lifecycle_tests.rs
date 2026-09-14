use super::*;

use crate::combat::anticheat::AntiCheatCounter;
use crate::combat::components::{BodyPart, DefenseWindow, Wound, WoundKind, REVIVE_WEAKENED_TICKS};
use crate::combat::events::{RevivalActionIntent, RevivalActionKind};
use crate::cultivation::components::Cultivation;
use crate::cultivation::life_record::LifeRecord;
use crate::cultivation::tick::CultivationClock;
use crate::network::agent_bridge::SERVER_DATA_CHANNEL;
use crate::persistence::{
    bootstrap_sqlite, complete_tribulation_ascension, load_ascension_quota,
    persist_active_tribulation, persist_player_cultivation_bundle, ActiveTribulationRecord,
    PersistenceSettings,
};
use crate::player::state::player_character_id;
use crate::schema::anticheat::ViolationKindV1;
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
use rusqlite::{params, Connection};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use valence::prelude::{App, Events, IntoSystemConfigs, Update};
use valence::protocol::packets::play::CustomPayloadS2c;
use valence::testing::{create_mock_client, MockClientHelper};

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
            LifeRecord::new(crate::player::state::canonical_player_id(username)),
            lifecycle,
        ))
        .id();
    (entity, helper)
}

fn seed_revival_cultivation_bundle(
    settings: &PersistenceSettings,
    username: &str,
    cultivation: &Cultivation,
    meridians: &MeridianSystem,
    contamination: &Contamination,
    life_record: &LifeRecord,
) {
    persist_player_cultivation_bundle(
        settings,
        username,
        cultivation,
        meridians,
        &crate::cultivation::components::QiColor::default(),
        &crate::cultivation::components::Karma::default(),
        contamination,
        life_record,
        &crate::cultivation::color::PracticeLog::default(),
        &crate::cultivation::insight::InsightQuota::default(),
        &crate::cultivation::insight_apply::UnlockedPerceptions::default(),
        &crate::cultivation::insight_apply::InsightModifiers::new(),
        None,
        &crate::cultivation::meridian::severed::MeridianSeveredPermanent::default(),
        None,
        None,
    )
    .expect("revival fixture cultivation bundle should persist");
}

fn seed_revival_entity_bundle(
    app: &mut App,
    settings: &PersistenceSettings,
    entity: Entity,
    username: &str,
) {
    let (cultivation, meridians, contamination, life_record) = {
        let entity_ref = app.world().entity(entity);
        (
            entity_ref
                .get::<Cultivation>()
                .expect("revival fixture requires Cultivation")
                .clone(),
            entity_ref
                .get::<MeridianSystem>()
                .expect("revival fixture requires MeridianSystem")
                .clone(),
            entity_ref
                .get::<Contamination>()
                .expect("revival fixture requires Contamination")
                .clone(),
            entity_ref
                .get::<LifeRecord>()
                .expect("revival fixture requires LifeRecord")
                .clone(),
        )
    };
    app.world_mut()
        .entity_mut(entity)
        .insert(Username(username.to_string()));
    seed_revival_cultivation_bundle(
        settings,
        username,
        &cultivation,
        &meridians,
        &contamination,
        &life_record,
    );
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

fn npc_terminal_test_app(test_name: &str) -> (App, PathBuf) {
    let (settings, root) = persistence_settings(test_name);
    let mut app = App::new();
    app.insert_resource(settings);
    app.insert_resource(CombatClock { tick: 200 });
    app.insert_resource(ZoneRegistry::fallback());
    app.insert_resource(WorldQiAccount::default());
    crate::npc::lifecycle::register(&mut app);
    app.add_event::<PlayerRevived>();
    app.add_event::<DeathCinematicPublished>();
    app.add_event::<DeathEvent>();
    app.add_event::<CultivationDeathTrigger>();
    app.add_systems(
        Update,
        death_arbiter_tick.in_set(crate::npc::lifecycle::NpcTerminalSystemSet::Stage),
    );
    (app, root)
}

fn spawn_dying_npc(
    app: &mut App,
    archetype: crate::npc::lifecycle::NpcArchetype,
    realm: Realm,
) -> (Entity, f64) {
    let entity = app
        .world_mut()
        .spawn((
            NpcMarker,
            Position::new([0.0, 66.0, 0.0]),
            CurrentDimension(crate::world::dimension::DimensionKind::Overworld),
        ))
        .id();
    let mut bundle = crate::npc::lifecycle::npc_runtime_bundle(entity, archetype, realm);
    bundle.wounds.health_current = 0.0;
    bundle.wounds.health_max = 100.0;
    bundle.cultivation.qi_current = bundle.cultivation.qi_max * 0.5;
    let initial_qi = bundle.cultivation.qi_current;
    app.world_mut().entity_mut(entity).insert(bundle);
    app.world_mut().send_event(DeathEvent {
        target: entity,
        cause: "combat".to_string(),
        at_tick: 200,
        attacker: None,
        attacker_player_id: None,
    });
    (entity, initial_qi)
}

fn assert_npc_terminal_commit(app: &App, entity: Entity, initial_qi: f64) {
    let lifecycle = app.world().entity(entity).get::<Lifecycle>().unwrap();
    assert_eq!(lifecycle.state, LifecycleState::Terminated);
    assert!(
        app.world()
            .get::<valence::prelude::Despawned>(entity)
            .is_some(),
        "NPC terminal commit must mark the entity Despawned"
    );
    assert!(
        app.world()
            .get::<crate::npc::lifecycle::NpcTerminalCommitted>(entity)
            .is_some(),
        "NPC terminal commit must leave its committed marker"
    );
    assert_eq!(
        app.world().resource::<Events<PlayerTerminated>>().len(),
        0,
        "NPC terminal staging must not emit the player-only termination event"
    );
    assert_eq!(
        app.world()
            .resource::<Events<crate::npc::lifecycle::NpcDeathNotice>>()
            .iter_current_update_events()
            .count(),
        1,
        "NPC terminal commit must emit one death notice"
    );
    assert_eq!(
        app.world()
            .resource::<Events<crate::npc::lifecycle::NpcTerminalSettlementSucceeded>>()
            .iter_current_update_events()
            .count(),
        1,
        "NPC terminal commit must emit one settlement success"
    );
    let cultivation = app.world().entity(entity).get::<Cultivation>().unwrap();
    assert_eq!(cultivation.qi_current, 0.0);
    let settled_qi: f64 = app
        .world()
        .resource::<WorldQiAccount>()
        .transfers()
        .iter()
        .filter(|transfer| transfer.reason == QiTransferReason::ReleaseToZone)
        .map(|transfer| transfer.amount)
        .sum();
    assert!((settled_qi - initial_qi).abs() < 1e-9);
}

fn assert_contamination_matches(actual: &Contamination, expected: &Contamination) {
    assert_eq!(
        actual.entries.len(),
        expected.entries.len(),
        "contamination entry count changed across failed revival"
    );
    for (actual, expected) in actual.entries.iter().zip(&expected.entries) {
        assert_eq!(actual.color, expected.color);
        assert_eq!(actual.meridian_id, expected.meridian_id);
        assert_eq!(actual.attacker_id, expected.attacker_id);
        assert_eq!(actual.introduced_at, expected.introduced_at);
        assert_eq!(actual.amount, expected.amount);
    }
}

fn make_coffin_registry_with_player(player: Entity) -> crate::coffin::CoffinRegistry {
    let lower = valence::prelude::BlockPos::new(10, 64, 10);
    let mut registry = crate::coffin::CoffinRegistry::default();
    registry.insert(lower, 0, crate::coffin::CoffinGrade::Mundane);
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
    app.add_event::<crate::coffin::CoffinStateChanged>();
    app.insert_resource(WorldQiAccount::default());
    app.add_systems(Update, handle_revival_action_intents);
    (settings, root)
}
#[test]
fn realm_collapse_death_causes_count_as_death_zone() {
    assert_eq!(
        death_zone_from_context("realm_collapse", None, None),
        ZoneDeathKind::Death
    );
    assert_eq!(
        death_zone_from_context("realm_collapse_entry_lock", None, None),
        ZoneDeathKind::Death
    );
}
#[test]
fn tsy_collapsed_death_keeps_standard_fortune_revival_decision() {
    let mut lifecycle = Lifecycle {
        fortune_remaining: 1,
        ..Default::default()
    };
    lifecycle.await_revival_decision(
        crate::combat::components::RevivalDecision::Fortune { chance: 1.0 },
        100,
    );

    let decision = determine_revival_decision(
        &lifecycle,
        None,
        "tsy_collapsed",
        None,
        None,
        None,
        None,
        701,
    );

    assert!(matches!(
        decision,
        Some(RevivalDecision::Fortune { chance }) if (chance - 1.0).abs() < f64::EPSILON
    ));
}

#[test]
fn death_loop_full_cycle_reentrant_death_event_does_not_block_reincarnate() {
    // 死亡后下一 tick 重复触发，仍须保留原裁决并允许复活。
    let mut app = App::new();
    let (settings, root) = persistence_settings("death-loop-full-cycle");
    app.insert_resource(settings.clone());
    app.insert_resource(CombatClock { tick: 100 });
    app.add_event::<DeathEvent>();
    app.add_event::<CultivationDeathTrigger>();
    app.add_event::<DeathInsightRequested>();
    app.add_event::<DeathCinematicPublished>();
    app.add_event::<PlayerRevived>();
    app.add_event::<PlayerTerminated>();
    app.add_event::<RevivalActionIntent>();
    app.add_event::<AscensionQuotaOpened>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<QiTransfer>();
    app.add_event::<crate::coffin::CoffinStateChanged>();
    app.insert_resource(WorldQiAccount::default());
    app.add_systems(
        Update,
        (
            death_arbiter_tick,
            handle_revival_action_intents.after(death_arbiter_tick),
        ),
    );

    let (entity, _helper) = spawn_client_actor(
        &mut app,
        "Loopy",
        Wounds {
            health_current: 0.0,
            health_max: 30.0,
            entries: Vec::new(),
        },
        Stamina::default(),
        Lifecycle {
            fortune_remaining: 1,
            ..Default::default()
        },
    );

    app.world_mut().entity_mut(entity).insert((
        Cultivation::default(),
        MeridianSystem::default(),
        Contamination::default(),
    ));
    seed_revival_entity_bundle(&mut app, &settings, entity, "Loopy");

    // 首次死亡事件：Alive → AwaitingRevival。
    app.world_mut().send_event(DeathEvent {
        target: entity,
        cause: "contamination_overflow".to_string(),
        attacker: None,
        attacker_player_id: None,
        at_tick: 100,
    });
    app.update();
    {
        let lifecycle = app.world().entity(entity).get::<Lifecycle>().unwrap();
        assert_eq!(
            lifecycle.state,
            LifecycleState::AwaitingRevival,
            "期望首次死亡事件后进入 AwaitingRevival；实际 {:?}",
            lifecycle.state
        );
    }

    // 实证场景：污染溢出在死亡屏挂起期间又触发一条同 cause 死亡事件（下一 tick，601 之后每 601 tick 重入）。
    app.world_mut().send_event(DeathEvent {
        target: entity,
        cause: "contamination_overflow".to_string(),
        attacker: None,
        attacker_player_id: None,
        at_tick: 101,
    });
    app.update();
    {
        let lifecycle = app.world().entity(entity).get::<Lifecycle>().unwrap();
        assert_eq!(
            lifecycle.state,
            LifecycleState::AwaitingRevival,
            "期望重入死亡事件后状态仍是 AwaitingRevival——这是 Bug 1 的核心断言；实际 {:?}",
            lifecycle.state
        );
    }

    // 玩家送出 Reincarnate 决策：必须成功复活，而不是因状态已被重入死亡事件破坏而被
    // `lifecycle.state != AwaitingRevival` 静默丢弃。
    app.world_mut().send_event(RevivalActionIntent {
        entity,
        action: RevivalActionKind::Reincarnate,
        issued_at_tick: 102,
    });
    app.update();

    let lifecycle = app.world().entity(entity).get::<Lifecycle>().unwrap();
    assert_eq!(
        lifecycle.state,
        LifecycleState::Alive,
        "期望 Reincarnate 决策后成功复活为 Alive 因为死亡屏窗口本应完整存活直到玩家决策；实际 {:?}",
        lifecycle.state
    );
    let revived_events = app.world().resource::<Events<PlayerRevived>>();
    assert_eq!(
        revived_events.len(),
        1,
        "期望恰好一次 PlayerRevived 事件；实际 {}",
        revived_events.len()
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn npc_death_termination_keeps_high_realm_qi_burst_profile() {
    let (mut app, root) = npc_terminal_test_app("npc-near-death-vfx");
    let entity = {
        let (entity, initial_qi) = spawn_dying_npc(
            &mut app,
            crate::npc::lifecycle::NpcArchetype::Rogue,
            Realm::Spirit,
        );
        let profile = crate::skin::select_npc_visual_profile(
            crate::npc::lifecycle::NpcArchetype::Rogue,
            Realm::Spirit,
            None,
            None,
            0.5,
        );
        app.world_mut().entity_mut(entity).insert(profile);
        app.update();
        assert_npc_terminal_commit(&app, entity, initial_qi);
        entity
    };

    let vfx_events = app.world().resource::<Events<VfxEventRequest>>();
    let mut reader = vfx_events.get_reader();
    let event_ids = reader
        .read(vfx_events)
        .filter_map(|request| match &request.payload {
            VfxEventPayloadV1::SpawnParticle { event_id, .. } => Some(event_id.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(event_ids.contains(&"bong:death_soul_dissipate"));
    assert!(event_ids.contains(&"bong:npc_death_smoke"));
    assert!(event_ids.contains(&"bong:npc_death_qi_burst"));
    let _ = (entity, fs::remove_dir_all(root));
}

#[test]
fn dying_rat_terminates_without_waiting_for_player_revival_window() {
    let (mut app, root) = npc_terminal_test_app("rat-near-death-immediate");
    let (entity, initial_qi) = spawn_dying_npc(
        &mut app,
        crate::npc::lifecycle::NpcArchetype::Beast,
        Realm::Awaken,
    );
    app.world_mut()
        .entity_mut(entity)
        .insert(crate::fauna::components::FaunaTag::new(
            crate::fauna::components::BeastKind::Rat,
        ));
    app.update();
    assert_npc_terminal_commit(&app, entity, initial_qi);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn dying_non_rat_npc_terminates_immediately() {
    let (mut app, root) = npc_terminal_test_app("spider-near-death-immediate");
    let (entity, initial_qi) = spawn_dying_npc(
        &mut app,
        crate::npc::lifecycle::NpcArchetype::Beast,
        Realm::Awaken,
    );
    app.world_mut()
        .entity_mut(entity)
        .insert(crate::fauna::components::FaunaTag::new(
            crate::fauna::components::BeastKind::Spider,
        ));
    app.update();
    assert_npc_terminal_commit(&app, entity, initial_qi);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn life_events_are_append_only_and_atomic_with_state_updates() {
    let mut app = App::new();
    let (settings, root) = persistence_settings("append-only-atomic");
    app.insert_resource(settings.clone());
    app.insert_resource(CombatClock { tick: 90 });
    app.insert_resource(CultivationClock { tick: 691 });
    app.add_event::<DeathEvent>();
    app.add_event::<CultivationDeathTrigger>();
    app.add_event::<DeathInsightRequested>();
    app.add_event::<DeathCinematicPublished>();
    app.add_event::<PlayerRevived>();
    app.add_event::<PlayerTerminated>();
    app.add_event::<RevivalActionIntent>();
    app.add_event::<AscensionQuotaOpened>();
    app.add_event::<crate::skill::events::SkillCapChanged>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<QiTransfer>();
    app.add_event::<crate::coffin::CoffinStateChanged>();
    app.insert_resource(WorldQiAccount::default());
    app.add_systems(
        Update,
        (
            death_arbiter_tick,
            handle_revival_action_intents.after(death_arbiter_tick),
            crate::cultivation::death_hooks::on_player_terminated.after(death_arbiter_tick),
        ),
    );

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
            Lifecycle {
                character_id: "offline:Ancestor".to_string(),
                fortune_remaining: 1,
                ..Default::default()
            },
            crate::cultivation::components::Cultivation {
                realm: Realm::Induce,
                qi_current: 12.0,
                qi_max: 24.0,
                ..Default::default()
            },
            crate::cultivation::components::MeridianSystem::default(),
            crate::cultivation::components::Contamination::default(),
            LifeRecord::new("offline:Ancestor"),
        ))
        .id();
    seed_revival_entity_bundle(&mut app, &settings, entity, "Ancestor");

    app.world_mut().send_event(DeathEvent {
        target: entity,
        cause: "bleed_out".to_string(),
        attacker: None,
        attacker_player_id: None,
        at_tick: 90,
    });
    app.update();

    let connection = Connection::open(settings.db_path()).expect("db should open");
    let death_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM life_events WHERE char_id = ?1 AND event_type = 'death'",
            params!["offline:Ancestor"],
            |row| row.get(0),
        )
        .expect("death count query should succeed");
    let lifespan_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM lifespan_events WHERE char_id = ?1 AND event_type = 'death_penalty'",
            params!["offline:Ancestor"],
            |row| row.get(0),
        )
        .expect("lifespan count query should succeed");
    let death_registry: (i64, i64, String) = connection
        .query_row(
            "SELECT death_count, last_death_tick, last_death_cause FROM death_registry WHERE char_id = ?1",
            params!["offline:Ancestor"],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("death registry should exist");

    assert_eq!(death_count, 1);
    assert_eq!(lifespan_count, 1);
    assert_eq!(death_registry, (1, 90, "bleed_out".to_string()));
    assert_eq!(
        app.world().entity(entity).get::<Lifecycle>().unwrap().state,
        LifecycleState::AwaitingRevival
    );

    app.world_mut().resource_mut::<CombatClock>().tick = 691;
    app.update();
    app.world_mut().send_event(RevivalActionIntent {
        entity,
        action: RevivalActionKind::Reincarnate,
        issued_at_tick: 691,
    });
    app.update();

    let life_event_types: Vec<String> = connection
        .prepare(
            "SELECT event_type FROM life_events WHERE char_id = ?1 ORDER BY game_tick, event_id",
        )
        .expect("statement should prepare")
        .query_map(params!["offline:Ancestor"], |row| row.get(0))
        .expect("life_events query should succeed")
        .map(|row| row.expect("row should decode"))
        .collect();
    let lifespan_payload_json: String = connection
        .query_row(
            "SELECT payload_json FROM lifespan_events WHERE char_id = ?1 LIMIT 1",
            params!["offline:Ancestor"],
            |row| row.get(0),
        )
        .expect("lifespan payload should exist");
    let lifespan_payload: crate::persistence::LifespanEventRecord =
        serde_json::from_str(&lifespan_payload_json).expect("lifespan payload should decode");

    assert_eq!(
        life_event_types,
        vec!["death".to_string(), "rebirth".to_string()]
    );
    assert_eq!(lifespan_payload.delta_years, -10);
    assert_eq!(lifespan_payload.kind, "death_penalty");
    assert_eq!(
        app.world().entity(entity).get::<Lifecycle>().unwrap().state,
        LifecycleState::Alive
    );
    assert!(matches!(
        app.world()
            .entity(entity)
            .get::<LifeRecord>()
            .unwrap()
            .biography
            .last(),
        Some(BiographyEntry::Rebirth { tick: 691, .. })
    ));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn create_new_character_rehydrates_default_character_state_and_persists_slices() {
    let mut app = App::new();
    let (settings, root) = persistence_settings("create-new-character");
    let data_dir = root.join("data");
    app.insert_resource(settings.clone());
    app.insert_resource(PlayerStatePersistence::with_db_path(
        &data_dir,
        settings.db_path(),
    ));
    app.insert_resource(CombatClock { tick: 800 });

    let item_registry = crate::inventory::load_item_registry().expect("item registry should load");
    let default_loadout = crate::inventory::load_default_loadout(&item_registry)
        .expect("default loadout should load");
    app.insert_resource(DefaultLoadout(default_loadout));
    // plan-layered-equip-v1 P0.6 — reset_for_new_character 现需 ItemRegistry 重建 inventory。
    app.insert_resource(item_registry);
    app.insert_resource(InventoryInstanceIdAllocator::default());
    app.insert_resource(TechniqueRegistry::load_for_tests());

    app.add_event::<RevivalActionIntent>();
    app.add_event::<PlayerRevived>();
    app.add_event::<PlayerTerminated>();
    app.add_event::<AscensionQuotaOpened>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<QiTransfer>();
    app.add_event::<crate::coffin::CoffinStateChanged>();
    app.insert_resource(WorldQiAccount::default());
    app.add_systems(Update, handle_revival_action_intents);

    let username = Username("Azure".to_string());
    let mut anticheat_counter = AntiCheatCounter::default();
    anticheat_counter.record_violation(ViolationKindV1::ReachExceeded, "reach: previous character");
    let _ = save_player_slices(
        &PlayerStatePersistence::with_db_path(&data_dir, settings.db_path()),
        username.0.as_str(),
        &PlayerState {
            karma: 0.4,
            inventory_score: 0.8,
        },
        [99.0, 64.0, 99.0],
        DimensionKind::default(),
        None,
        None,
        &SkillSet::default(),
    );

    let entity = app
        .world_mut()
        .spawn((
            Wounds {
                health_current: 0.0,
                health_max: 30.0,
                entries: vec![Wound {
                    location: crate::body_plan::legacy_body_part_to_id(BodyPart::Chest),
                    kind: WoundKind::Cut,
                    severity: 0.9,
                    bleeding_per_sec: 2.0,
                    created_at_tick: 1,
                    inflicted_by: Some("offline:Enemy".to_string()),
                }],
            },
            Stamina {
                current: 1.0,
                max: 100.0,
                recover_per_sec: 5.0,
                last_drain_tick: Some(12),
                state: StaminaState::Exhausted,
            },
            CombatState {
                in_combat_until_tick: Some(900),
                last_attack_at_tick: Some(700),
                incoming_window: Some(DefenseWindow {
                    opened_at_tick: 700,
                    duration_ms: 100,
                }),
            },
            Lifecycle {
                character_id: "offline:Ancestor".to_string(),
                state: LifecycleState::Terminated,
                death_count: 9,
                fortune_remaining: 0,
                last_death_tick: Some(799),
                ..Default::default()
            },
            LifeRecord::new("offline:Ancestor"),
            DeathRegistry {
                char_id: "offline:Ancestor".to_string(),
                death_count: 9,
                last_death_tick: Some(799),
                prev_death_tick: None,
                last_death_zone: Some(ZoneDeathKind::Death),
            },
            LifespanComponent {
                born_at_tick: 10,
                years_lived: 79.0,
                cap_by_realm: 80,
                offline_pause_tick: Some(700),
            },
            PlayerState {
                karma: 0.4,
                inventory_score: 0.8,
            },
            anticheat_counter,
            Position::new([99.0, 64.0, 99.0]),
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

    let entity_ref = app.world().entity(entity);
    let lifecycle = entity_ref
        .get::<Lifecycle>()
        .expect("lifecycle should remain attached");
    let death_registry = entity_ref
        .get::<DeathRegistry>()
        .expect("death registry should be reset for new character");
    let lifespan = entity_ref
        .get::<LifespanComponent>()
        .expect("lifespan should be reset for new character");
    let player_state = entity_ref
        .get::<PlayerState>()
        .expect("player state should remain attached");
    let position = entity_ref
        .get::<Position>()
        .expect("position should remain attached");
    let cultivation = entity_ref
        .get::<Cultivation>()
        .expect("cultivation should be reattached for new character");
    let meridians = entity_ref
        .get::<MeridianSystem>()
        .expect("meridians should be reattached for new character");
    let known_techniques = entity_ref
        .get::<KnownTechniques>()
        .expect("new character should receive a KnownTechniques component");
    let learned = entity_ref
        .get::<LearnedRecipes>()
        .expect("learned recipes should be reattached for new character");
    let inventory = entity_ref
        .get::<PlayerInventory>()
        .expect("inventory should be reinitialized for new character");
    let anticheat_counter = entity_ref
        .get::<AntiCheatCounter>()
        .expect("anticheat counter should remain attached");

    assert_eq!(lifecycle.state, LifecycleState::Alive);
    let connection = Connection::open(settings.db_path()).expect("db should open");
    let current_char_id: String = connection
        .query_row(
            "SELECT current_char_id FROM player_core WHERE username = ?1",
            params![username.0.as_str()],
            |row| row.get(0),
        )
        .expect("current_char_id should persist");
    assert_eq!(
        lifecycle.character_id,
        player_character_id(username.0.as_str(), &current_char_id)
    );
    assert_eq!(lifecycle.death_count, 0);
    assert_eq!(lifecycle.fortune_remaining, 3);
    assert_eq!(death_registry.death_count, 0);
    assert_eq!(death_registry.char_id, lifecycle.character_id);
    // plan-multi-life-v1 §2：新角色 = Awaken 境界，寿元 = 醒灵 cap (AWAKEN=120)
    // 与 attach_cultivation_to_joined_clients 路径保持一致；旧值 MORTAL=80 是 bug。
    assert_eq!(lifespan.cap_by_realm, LifespanCapTable::AWAKEN);
    assert_eq!(lifespan.years_lived, 0.0);
    assert_eq!(player_state, &PlayerState::default());
    let expected_spawn =
        crate::cultivation::character_select::next_character_spec_for_seed(&lifecycle.character_id)
            .spawn_pos;
    assert_eq!(position.get(), Position::new(expected_spawn).get());
    assert_eq!(cultivation.realm, Realm::Awaken);
    assert_eq!(cultivation.qi_current, 0.0);
    assert_eq!(cultivation.qi_max, 10.0);
    assert_eq!(meridians.opened_count(), 0);
    #[cfg(feature = "dev-techniques")]
    assert_eq!(
        known_techniques.entries.len(),
        KnownTechniques::progression_reset(app.world().resource::<TechniqueRegistry>())
            .entries
            .len(),
        "新角色应按出生配置获得功法，身法由卷轴学习"
    );
    #[cfg(not(feature = "dev-techniques"))]
    assert!(
        known_techniques.entries.is_empty(),
        "production new-character reset must keep technique progression empty"
    );
    assert_eq!(learned.ids, vec!["kai_mai_pill_v0".to_string()]);
    assert!(inventory.revision.0 >= 1);
    assert_eq!(anticheat_counter.reach_violations, 0);
    assert_eq!(anticheat_counter.cooldown_violations, 0);
    assert_eq!(anticheat_counter.qi_invest_violations, 0);
    assert!(anticheat_counter.last_reach_details.is_empty());

    let persisted = crate::player::state::load_player_slices(
        &PlayerStatePersistence::with_db_path(&data_dir, settings.db_path()),
        username.0.as_str(),
    );
    assert_eq!(persisted.state, PlayerState::default());
    assert_eq!(persisted.position, expected_spawn);
    assert!(persisted.inventory.is_some());
    let persisted_lifespan = persisted.lifespan.expect("fresh lifespan should persist");
    assert_eq!(persisted_lifespan.born_at_tick, 0);
    // plan-multi-life-v1 §2：持久化的 lifespan 同样为 AWAKEN cap
    assert_eq!(persisted_lifespan.cap_by_realm, LifespanCapTable::AWAKEN);
    assert!(persisted_lifespan.years_lived >= 0.0);
    assert!(persisted_lifespan.years_lived < 0.01);
    assert_eq!(persisted_lifespan.offline_pause_tick, None);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn void_revival_releases_ascension_quota() {
    let mut app = App::new();
    let (settings, root) = persistence_settings("void-revival-release-quota");
    persist_active_tribulation(
        &settings,
        &ActiveTribulationRecord {
            char_id: "offline:VoidWalker".to_string(),
            kind: "du_xu".to_string(),
            source: String::new(),
            origin_dimension: Some("minecraft:overworld".to_string()),
            wave_current: 3,
            waves_total: 3,
            started_tick: 10,
            epicenter: [0.0, 64.0, 0.0],
            intensity: 0.0,
        },
    )
    .expect("active DuXu should persist before quota setup");
    complete_tribulation_ascension(&settings, "offline:VoidWalker")
        .expect("quota setup should succeed");
    app.insert_resource(settings.clone());
    app.insert_resource(CombatClock { tick: 700 });
    app.add_event::<RevivalActionIntent>();
    app.add_event::<PlayerRevived>();
    app.add_event::<PlayerTerminated>();
    app.add_event::<AscensionQuotaOpened>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<QiTransfer>();
    app.add_event::<crate::coffin::CoffinStateChanged>();
    app.insert_resource(WorldQiAccount::default());
    app.add_systems(Update, handle_revival_action_intents);

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
                character_id: "offline:VoidWalker".to_string(),
                state: LifecycleState::AwaitingRevival,
                awaiting_decision: Some(RevivalDecision::Fortune { chance: 1.0 }),
                revival_decision_deadline_tick: Some(800),
                fortune_remaining: 1,
                ..Default::default()
            },
            Cultivation {
                realm: Realm::Void,
                qi_current: 12.0,
                qi_max: 240.0,
                ..Default::default()
            },
            MeridianSystem::default(),
            Contamination::default(),
            LifeRecord::new("offline:VoidWalker"),
        ))
        .id();
    seed_revival_entity_bundle(&mut app, &settings, entity, "VoidWalker");

    app.world_mut().send_event(RevivalActionIntent {
        entity,
        action: RevivalActionKind::Reincarnate,
        issued_at_tick: 700,
    });
    app.update();

    let cultivation = app
        .world()
        .get::<Cultivation>(entity)
        .expect("cultivation should remain attached");
    assert_eq!(cultivation.realm, Realm::Spirit);
    let quota = load_ascension_quota(&settings).expect("quota load should succeed");
    assert_eq!(quota.occupied_slots, 0);
    let quota_events: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<AscensionQuotaOpened>>()
        .drain()
        .collect();
    assert_eq!(quota_events.len(), 1);
    assert_eq!(quota_events[0].occupied_slots, 0);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn revival_roll_is_locked_persisted_and_settled_only_after_animation() {
    for (chance, survived) in [(1.0, true), (0.0, false)] {
        let mut app = App::new();
        let (settings, _root) = persistence_settings("locked-revival-roll");
        let storage = PlayerStatePersistence::with_db_path(_root.join("data"), settings.db_path());
        app.insert_resource(settings.clone());
        app.insert_resource(storage.clone());
        app.insert_resource(CombatClock { tick: 100 });
        app.insert_resource(WorldQiAccount::default());
        app.add_event::<RevivalActionIntent>();
        app.add_event::<PlayerRevived>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<AscensionQuotaOpened>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<QiTransfer>();
        app.add_event::<crate::coffin::CoffinStateChanged>();
        app.add_systems(
            Update,
            (
                auto_confirm_revival_decisions,
                handle_revival_action_intents,
            )
                .chain(),
        );
        let (entity, mut helper) = spawn_client_actor(
            &mut app,
            "Dice",
            Wounds {
                health_current: 0.0,
                ..Default::default()
            },
            Stamina::default(),
            Lifecycle {
                character_id: "offline:Dice".into(),
                state: LifecycleState::AwaitingRevival,
                awaiting_decision: Some(RevivalDecision::Tribulation { chance }),
                revival_decision_deadline_tick: Some(900),
                ..Default::default()
            },
        );
        app.world_mut().entity_mut(entity).insert((
            Cultivation::default(),
            MeridianSystem::default(),
            Contamination::default(),
        ));
        seed_revival_entity_bundle(&mut app, &settings, entity, "Dice");
        app.world_mut().send_event(RevivalActionIntent {
            entity,
            action: RevivalActionKind::RollRebirth,
            issued_at_tick: 100,
        });
        app.update();
        let locked = app.world().get::<Lifecycle>(entity).unwrap().clone();
        assert_eq!(
            locked.state,
            LifecycleState::AwaitingRevival,
            "掷骰时不能提前复活或终结"
        );
        let persisted = crate::player::state::load_player_lifecycle_slice(&storage, "Dice", 100)
            .unwrap()
            .unwrap();
        assert_eq!(
            persisted.revival_roll_survived,
            Some(survived),
            "重连必须恢复同一次裁决"
        );
        flush_client_packets(&mut app);
        assert!(collect_server_data_payloads(&mut helper).iter().any(|payload| matches!(
                &payload.payload, ServerDataPayloadV1::DeathScreen {
                    can_reincarnate: false, can_terminate: false, cinematic: Some(cinematic), ..
                } if cinematic.roll.result == if survived { crate::schema::death_cinematic::DeathRollResultV1::Survive } else { crate::schema::death_cinematic::DeathRollResultV1::Fall }
            )), "客户端必须收到锁定结果且两项操作禁用");
        app.world_mut().resource_mut::<CombatClock>().tick = 101;
        for action in [
            RevivalActionKind::RollRebirth,
            RevivalActionKind::Terminate,
            RevivalActionKind::Reincarnate,
        ] {
            app.world_mut().send_event(RevivalActionIntent {
                entity,
                action,
                issued_at_tick: 101,
            });
        }
        app.update();
        let life = app.world().get::<Lifecycle>(entity).unwrap();
        assert_eq!(life.state, LifecycleState::AwaitingRevival);
        assert_eq!(life.revival_roll_survived, locked.revival_roll_survived);
        assert_eq!(
            life.revival_decision_deadline_tick, locked.revival_decision_deadline_tick,
            "重复请求不能重掷、改选或延长演出"
        );
        app.world_mut().resource_mut::<CombatClock>().tick = 100 + REVIVAL_ROLL_TICKS;
        app.update();
        let life = app.world().get::<Lifecycle>(entity).unwrap();
        assert_eq!(
            life.state,
            if survived {
                LifecycleState::Alive
            } else {
                LifecycleState::Terminated
            },
            "大重生，小终结，结算沿用锁定结果"
        );
    }
}

#[test]
fn reincarnate_places_player_at_shrine_anchor_or_world_spawn() {
    let mut app = App::new();
    let (settings, root) = persistence_settings("revive-spawn-anchor");
    app.insert_resource(settings.clone());
    app.insert_resource(CombatClock { tick: 42 });
    app.add_event::<RevivalActionIntent>();
    app.add_event::<PlayerRevived>();
    app.add_event::<PlayerTerminated>();
    app.add_event::<AscensionQuotaOpened>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<QiTransfer>();
    app.add_event::<crate::coffin::CoffinStateChanged>();
    app.insert_resource(WorldQiAccount::default());
    app.add_systems(Update, handle_revival_action_intents);

    let shrine_anchor = [123.0, 45.0, -67.0];

    let with_shrine = app
        .world_mut()
        .spawn((
            Position::new([99.0, 64.0, 99.0]),
            Lifecycle {
                character_id: "offline:WithShrineRevive".to_string(),
                state: LifecycleState::AwaitingRevival,
                awaiting_decision: Some(RevivalDecision::Fortune { chance: 1.0 }),
                spawn_anchor: Some(shrine_anchor),
                fortune_remaining: 1,
                ..Default::default()
            },
            Cultivation::default(),
            MeridianSystem::default(),
            Contamination::default(),
            LifeRecord::new("offline:WithShrineRevive"),
        ))
        .id();

    let without_shrine = app
        .world_mut()
        .spawn((
            Position::new([99.0, 64.0, 99.0]),
            Lifecycle {
                character_id: "offline:WithoutShrineRevive".to_string(),
                state: LifecycleState::AwaitingRevival,
                awaiting_decision: Some(RevivalDecision::Fortune { chance: 1.0 }),
                spawn_anchor: None,
                fortune_remaining: 1,
                ..Default::default()
            },
            Cultivation::default(),
            MeridianSystem::default(),
            Contamination::default(),
            LifeRecord::new("offline:WithoutShrineRevive"),
        ))
        .id();
    seed_revival_entity_bundle(&mut app, &settings, with_shrine, "WithShrineRevive");
    seed_revival_entity_bundle(&mut app, &settings, without_shrine, "WithoutShrineRevive");

    app.world_mut().send_event(RevivalActionIntent {
        entity: with_shrine,
        action: RevivalActionKind::Reincarnate,
        issued_at_tick: 42,
    });
    app.world_mut().send_event(RevivalActionIntent {
        entity: without_shrine,
        action: RevivalActionKind::Reincarnate,
        issued_at_tick: 42,
    });
    app.update();

    let with_shrine_pos = app
        .world()
        .entity(with_shrine)
        .get::<Position>()
        .expect("position should exist")
        .get();
    assert_eq!(with_shrine_pos, Position::new(shrine_anchor).get());

    let without_shrine_pos = app
        .world()
        .entity(without_shrine)
        .get::<Position>()
        .expect("position should exist")
        .get();
    assert_eq!(
        without_shrine_pos,
        Position::new(crate::player::spawn_position()).get()
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn damaged_spawn_anchor_doubles_revive_weakened_duration() {
    let mut app = App::new();
    let (settings, root) = persistence_settings("revive-damaged-spawn-anchor");
    app.insert_resource(settings.clone());
    app.insert_resource(CombatClock { tick: 42 });
    app.add_event::<RevivalActionIntent>();
    app.add_event::<PlayerRevived>();
    app.add_event::<PlayerTerminated>();
    app.add_event::<AscensionQuotaOpened>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<QiTransfer>();
    app.add_event::<crate::coffin::CoffinStateChanged>();
    app.insert_resource(WorldQiAccount::default());
    app.add_systems(Update, handle_revival_action_intents);

    let damaged = app
        .world_mut()
        .spawn((
            Position::new([99.0, 64.0, 99.0]),
            Lifecycle {
                character_id: "offline:DamagedAnchorRevive".to_string(),
                state: LifecycleState::AwaitingRevival,
                awaiting_decision: Some(RevivalDecision::Fortune { chance: 1.0 }),
                spawn_anchor: Some([11.0, 65.0, 10.0]),
                spawn_anchor_damaged: true,
                fortune_remaining: 1,
                ..Default::default()
            },
            Cultivation::default(),
            MeridianSystem::default(),
            Contamination::default(),
            LifeRecord::new("offline:DamagedAnchorRevive"),
        ))
        .id();
    let intact = app
        .world_mut()
        .spawn((
            Position::new([99.0, 64.0, 99.0]),
            Lifecycle {
                character_id: "offline:IntactAnchorRevive".to_string(),
                state: LifecycleState::AwaitingRevival,
                awaiting_decision: Some(RevivalDecision::Fortune { chance: 1.0 }),
                spawn_anchor: Some([12.0, 65.0, 10.0]),
                spawn_anchor_damaged: false,
                fortune_remaining: 1,
                ..Default::default()
            },
            Cultivation::default(),
            MeridianSystem::default(),
            Contamination::default(),
            LifeRecord::new("offline:IntactAnchorRevive"),
        ))
        .id();
    seed_revival_entity_bundle(&mut app, &settings, damaged, "DamagedAnchorRevive");
    seed_revival_entity_bundle(&mut app, &settings, intact, "IntactAnchorRevive");

    for entity in [damaged, intact] {
        app.world_mut().send_event(RevivalActionIntent {
            entity,
            action: RevivalActionKind::Reincarnate,
            issued_at_tick: 42,
        });
    }
    app.update();

    let damaged_lifecycle = app.world().entity(damaged).get::<Lifecycle>().unwrap();
    let intact_lifecycle = app.world().entity(intact).get::<Lifecycle>().unwrap();
    assert_eq!(
        damaged_lifecycle.weakened_until_tick.unwrap() - 42,
        REVIVE_WEAKENED_TICKS * 2,
        "damaged spirit niche spawn anchor should double revive weakened duration"
    );
    assert_eq!(
        intact_lifecycle.weakened_until_tick.unwrap() - 42,
        REVIVE_WEAKENED_TICKS,
        "intact spirit niche spawn anchor should keep baseline revive weakened duration"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn revival_late_sqlite_failure_keeps_every_memory_owner_and_event_unchanged() {
    let mut app = App::new();
    let (settings, root) = persistence_settings("revival-system-late-failure");
    persist_active_tribulation(
        &settings,
        &ActiveTribulationRecord {
            char_id: "offline:SystemRollback".to_string(),
            kind: "du_xu".to_string(),
            source: String::new(),
            origin_dimension: Some("minecraft:overworld".to_string()),
            wave_current: 3,
            waves_total: 3,
            started_tick: 10,
            epicenter: [0.0, 64.0, 0.0],
            intensity: 0.0,
        },
    )
    .expect("active DuXu should persist before quota setup");
    complete_tribulation_ascension(&settings, "offline:SystemRollback")
        .expect("quota setup should occupy one slot");

    app.insert_resource(settings.clone());
    app.insert_resource(CombatClock { tick: 700 });
    app.add_event::<RevivalActionIntent>();
    app.add_event::<PlayerRevived>();
    app.add_event::<PlayerTerminated>();
    app.add_event::<AscensionQuotaOpened>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<QiTransfer>();
    app.add_event::<crate::coffin::CoffinStateChanged>();
    let mut zones = ZoneRegistry::fallback();
    zones.zones[0].spirit_qi = 0.2;
    app.insert_resource(zones);
    let mut ledger = WorldQiAccount::default();
    ledger
        .set_balance(crate::qi_physics::ledger::qi_flow_overflow_account(), 17.0)
        .expect("fixture overflow balance should be valid");
    app.insert_resource(ledger);
    app.add_systems(Update, handle_revival_action_intents);

    let entity = app
        .world_mut()
        .spawn((
            Wounds {
                health_current: 1.0,
                health_max: 30.0,
                entries: Vec::new(),
            },
            Stamina {
                current: 12.0,
                max: 90.0,
                ..Default::default()
            },
            CombatState::default(),
            Position::new([8.0, 66.0, 8.0]),
            CurrentDimension(crate::world::dimension::DimensionKind::Overworld),
            Lifecycle {
                character_id: "offline:SystemRollback".to_string(),
                state: LifecycleState::AwaitingRevival,
                awaiting_decision: Some(RevivalDecision::Fortune { chance: 1.0 }),
                revival_decision_deadline_tick: Some(800),
                fortune_remaining: 1,
                ..Default::default()
            },
            Cultivation {
                realm: Realm::Void,
                qi_current: 12.0,
                qi_max: 240.0,
                ..Default::default()
            },
            MeridianSystem::default(),
            Contamination {
                entries: vec![crate::cultivation::components::ContamSource {
                    amount: 1.25,
                    color: crate::cultivation::components::ColorKind::Sharp,
                    meridian_id: None,
                    attacker_id: Some("fixture-attacker".to_string()),
                    introduced_at: 699,
                }],
            },
            LifeRecord::new("offline:SystemRollback"),
            crate::coffin::CoffinComponent {
                entered_at_tick: 600,
                coffin_lower: valence::prelude::BlockPos::new(10, 64, 10),
                grade: crate::coffin::CoffinGrade::Mundane,
            },
        ))
        .id();
    seed_revival_entity_bundle(&mut app, &settings, entity, "SystemRollback");
    app.insert_resource(make_coffin_registry_with_player(entity));

    let baseline_lifecycle = app.world().get::<Lifecycle>(entity).unwrap().clone();
    let baseline_cultivation = app.world().get::<Cultivation>(entity).unwrap().clone();
    let baseline_meridians = app.world().get::<MeridianSystem>(entity).unwrap().clone();
    let baseline_contamination = app.world().get::<Contamination>(entity).unwrap().clone();
    let baseline_life = app.world().get::<LifeRecord>(entity).unwrap().clone();
    let baseline_wounds = app.world().get::<Wounds>(entity).unwrap().clone();
    let baseline_stamina = app.world().get::<Stamina>(entity).unwrap().clone();
    let baseline_position = app.world().get::<Position>(entity).unwrap().get();
    let baseline_zones = app.world().resource::<ZoneRegistry>().clone();
    let baseline_ledger = app.world().resource::<WorldQiAccount>().clone();
    let baseline_bundle =
        crate::persistence::load_player_cultivation_bundle(&settings, "SystemRollback")
            .expect("baseline bundle should load")
            .expect("baseline bundle should exist");
    let baseline_zone_rows = crate::persistence::load_zone_runtime_snapshot(&settings)
        .expect("baseline zone runtime snapshot query should succeed");
    let mut baseline_durable_ledger = WorldQiAccount::default();
    crate::persistence::hydrate_runtime_qi_accounts(&settings, &mut baseline_durable_ledger)
        .expect("baseline stable runtime qi owners should hydrate");
    let baseline_quota = load_ascension_quota(&settings).expect("quota should load");
    let baseline_rebirth_count: i64 = Connection::open(settings.db_path())
        .expect("db should open")
        .query_row(
            "SELECT COUNT(*) FROM life_events WHERE char_id = ?1 AND event_type = 'rebirth'",
            params!["offline:SystemRollback"],
            |row| row.get(0),
        )
        .expect("baseline rebirth count should query");

    Connection::open(settings.db_path())
        .expect("db should reopen for trigger")
        .execute_batch(
            "
                CREATE TRIGGER reject_system_revival_quota_update
                BEFORE UPDATE ON ascension_quota
                WHEN NEW.row_id = 1
                BEGIN
                    SELECT RAISE(ABORT, 'fixture rejects system revival quota update');
                END;
                ",
        )
        .expect("late-failure trigger should install");

    app.world_mut().send_event(RevivalActionIntent {
        entity,
        action: RevivalActionKind::Reincarnate,
        issued_at_tick: 700,
    });
    app.update();

    let lifecycle = app.world().get::<Lifecycle>(entity).unwrap();
    assert_eq!(lifecycle.state, baseline_lifecycle.state);
    assert_eq!(
        lifecycle.fortune_remaining,
        baseline_lifecycle.fortune_remaining
    );
    assert_eq!(
        lifecycle.last_revive_tick,
        baseline_lifecycle.last_revive_tick
    );
    assert_eq!(
        lifecycle.weakened_until_tick,
        baseline_lifecycle.weakened_until_tick
    );
    assert_eq!(
        crate::cultivation::components::encode_persisted_cultivation(
            app.world().get::<Cultivation>(entity).unwrap()
        ),
        crate::cultivation::components::encode_persisted_cultivation(&baseline_cultivation)
    );
    assert_eq!(
        app.world().get::<MeridianSystem>(entity).unwrap(),
        &baseline_meridians
    );
    assert_contamination_matches(
        app.world().get::<Contamination>(entity).unwrap(),
        &baseline_contamination,
    );
    let life = app.world().get::<LifeRecord>(entity).unwrap();
    assert_eq!(life.character_id, baseline_life.character_id);
    assert_eq!(life.biography.len(), baseline_life.biography.len());
    assert_eq!(
        app.world().get::<Wounds>(entity).unwrap().health_current,
        baseline_wounds.health_current
    );
    assert_eq!(
        app.world().get::<Stamina>(entity).unwrap().current,
        baseline_stamina.current
    );
    assert_eq!(
        app.world().get::<Position>(entity).unwrap().get(),
        baseline_position
    );
    assert_eq!(app.world().resource::<ZoneRegistry>(), &baseline_zones);
    let ledger = app.world().resource::<WorldQiAccount>();
    assert_eq!(ledger.total(), baseline_ledger.total());
    assert_eq!(ledger.transfers(), baseline_ledger.transfers());
    assert!(app
        .world()
        .get::<crate::coffin::CoffinComponent>(entity)
        .is_some());
    assert!(app
        .world()
        .resource::<crate::coffin::CoffinRegistry>()
        .player_in_coffin
        .contains_key(&entity));
    assert_eq!(app.world().resource::<Events<PlayerRevived>>().len(), 0);
    assert_eq!(app.world().resource::<Events<QiTransfer>>().len(), 0);
    assert_eq!(
        app.world().resource::<Events<AscensionQuotaOpened>>().len(),
        0
    );
    assert_eq!(
        app.world()
            .resource::<Events<crate::coffin::CoffinStateChanged>>()
            .len(),
        0
    );

    assert_eq!(
        crate::persistence::load_player_cultivation_bundle(&settings, "SystemRollback")
            .expect("rolled-back bundle should load")
            .expect("rolled-back bundle should exist"),
        baseline_bundle
    );
    assert_eq!(
        crate::persistence::load_zone_runtime_snapshot(&settings)
            .expect("rolled-back zone snapshot should query"),
        baseline_zone_rows
    );
    let mut actual_durable_ledger = WorldQiAccount::default();
    crate::persistence::hydrate_runtime_qi_accounts(&settings, &mut actual_durable_ledger)
        .expect("rolled-back stable runtime qi owners should hydrate");
    for account in crate::qi_physics::ledger::persistent_runtime_qi_accounts() {
        assert_eq!(
            actual_durable_ledger.balance(&account),
            baseline_durable_ledger.balance(&account),
            "stable runtime qi owner changed despite late SQLite rollback: {account}"
        );
    }
    assert_eq!(
        load_ascension_quota(&settings).expect("rolled-back quota should load"),
        baseline_quota
    );
    let actual_rebirth_count: i64 = Connection::open(settings.db_path())
        .expect("db should reopen")
        .query_row(
            "SELECT COUNT(*) FROM life_events WHERE char_id = ?1 AND event_type = 'rebirth'",
            params!["offline:SystemRollback"],
            |row| row.get(0),
        )
        .expect("rolled-back rebirth count should query");
    assert_eq!(actual_rebirth_count, baseline_rebirth_count);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn revive_clears_coffin_component_and_registry_and_emits_state_changed() {
    let mut app = App::new();
    let (settings, root) = coffin_setup_base(&mut app, 500, "coffin-clear-revive");

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
                character_id: "offline:CoffinRevive".to_string(),
                state: LifecycleState::AwaitingRevival,
                awaiting_decision: Some(RevivalDecision::Fortune { chance: 1.0 }),
                revival_decision_deadline_tick: Some(600),
                fortune_remaining: 1,
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
            LifeRecord::new("offline:CoffinRevive"),
            crate::coffin::CoffinComponent {
                entered_at_tick: 400,
                coffin_lower: valence::prelude::BlockPos::new(10, 64, 10),
                grade: crate::coffin::CoffinGrade::Mundane,
            },
        ))
        .id();
    seed_revival_entity_bundle(&mut app, &settings, entity, "CoffinRevive");

    let registry = make_coffin_registry_with_player(entity);
    app.insert_resource(registry);

    app.world_mut().send_event(RevivalActionIntent {
        entity,
        action: RevivalActionKind::Reincarnate,
        issued_at_tick: 500,
    });
    app.update();

    // CoffinComponent 应已从 entity 移除（复活后不继续锁棺）
    assert!(
        app.world()
            .entity(entity)
            .get::<crate::coffin::CoffinComponent>()
            .is_none(),
        "期望 CoffinComponent=None（复活后不锁棺），实际仍有 CoffinComponent"
    );

    // CoffinRegistry.player_in_coffin 应清空
    let reg = app.world().resource::<crate::coffin::CoffinRegistry>();
    assert!(
        !reg.player_in_coffin.contains_key(&entity),
        "期望 player_in_coffin 不含 entity（clear_player 应清双索引），实际仍含该 entity"
    );

    // CoffinStateChanged(grade=None) 应被发送
    let state_events = app
        .world_mut()
        .resource_mut::<Events<crate::coffin::CoffinStateChanged>>()
        .drain()
        .collect::<Vec<_>>();
    assert_eq!(
        state_events.len(),
        1,
        "期望发出 1 条 CoffinStateChanged（玩家在棺内复活），实际发出 {} 条",
        state_events.len()
    );
    assert!(
        state_events[0].grade.is_none(),
        "期望 CoffinStateChanged.grade=None（离棺），实际 {:?}",
        state_events[0].grade
    );

    let _ = fs::remove_dir_all(root);
}

/// 非入棺玩家复活：不误清、不误发 CoffinStateChanged。
///   - CoffinComponent 不存在（期望：无副作用，remove 幂等）
///   - CoffinStateChanged 事件不发（期望：0 条，因为玩家本来不在棺内）

#[test]
fn revive_without_coffin_does_not_emit_coffin_state_changed() {
    let mut app = App::new();
    let (settings, root) = coffin_setup_base(&mut app, 500, "revive-without-coffin");
    // 空 registry：无任何棺
    app.insert_resource(crate::coffin::CoffinRegistry::default());

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
                character_id: "offline:NoCoffin".to_string(),
                state: LifecycleState::AwaitingRevival,
                awaiting_decision: Some(RevivalDecision::Fortune { chance: 1.0 }),
                revival_decision_deadline_tick: Some(600),
                fortune_remaining: 1,
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
            LifeRecord::new("offline:NoCoffin"),
            // 无 CoffinComponent
        ))
        .id();
    seed_revival_entity_bundle(&mut app, &settings, entity, "NoCoffin");

    app.world_mut().send_event(RevivalActionIntent {
        entity,
        action: RevivalActionKind::Reincarnate,
        issued_at_tick: 500,
    });
    app.update();

    // 先锁住复活确实成功，避免 durable fail-closed 时仅凭“不发 coffin event”假绿。
    assert_eq!(
        app.world().entity(entity).get::<Lifecycle>().unwrap().state,
        LifecycleState::Alive,
        "非棺内玩家的完整持久化 owner bundle 应允许复活成功"
    );

    // CoffinComponent 本来就没有，remove 幂等，entity 无异常
    assert!(
        app.world()
            .entity(entity)
            .get::<crate::coffin::CoffinComponent>()
            .is_none(),
        "非棺内玩家复活后 CoffinComponent 应为 None（remove 幂等）"
    );

    // 不应发出 CoffinStateChanged（clear_player 返回 None → 条件不满足 → 不发事件）
    let state_events = app
        .world_mut()
        .resource_mut::<Events<crate::coffin::CoffinStateChanged>>()
        .drain()
        .collect::<Vec<_>>();
    assert_eq!(
        state_events.len(),
        0,
        "期望非棺内玩家复活不发 CoffinStateChanged（避免噪音推送），实际发出 {} 条",
        state_events.len()
    );

    let _ = fs::remove_dir_all(root);
}

/// 新建角色：coffin 状态同样清除（即便理论上新角色无 coffin，防止旧 entity 残留）。
///   - CoffinComponent 从 entity 移除（期望：None，因为新角色不继承死亡前棺状态）
///   - CoffinRegistry.player_in_coffin 不含该 entity（期望：None）
///   - CoffinStateChanged 事件被发出 grade=None（期望：1 条，因为玩家在棺内）

#[test]
fn revive_enter_coffin_revive_cycle_clears_correctly() {
    let mut app = App::new();
    let (settings, root) = coffin_setup_base(&mut app, 100, "revive-coffin-cycle");
    app.insert_resource(crate::coffin::CoffinRegistry::default());

    // 第一轮：带 CoffinComponent 的玩家复活
    let lower = valence::prelude::BlockPos::new(5, 64, 5);
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
                character_id: "offline:CycleTest".to_string(),
                state: LifecycleState::AwaitingRevival,
                awaiting_decision: Some(RevivalDecision::Fortune { chance: 1.0 }),
                revival_decision_deadline_tick: Some(200),
                fortune_remaining: 3,
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
            LifeRecord::new("offline:CycleTest"),
            crate::coffin::CoffinComponent {
                entered_at_tick: 50,
                coffin_lower: lower,
                grade: crate::coffin::CoffinGrade::Mundane,
            },
        ))
        .id();
    seed_revival_entity_bundle(&mut app, &settings, entity, "CycleTest");

    {
        let mut reg = app
            .world_mut()
            .resource_mut::<crate::coffin::CoffinRegistry>();
        reg.insert(lower, 0, crate::coffin::CoffinGrade::Mundane);
        reg.set_occupied(lower, entity);
    }

    // 第一次复活
    app.world_mut().send_event(RevivalActionIntent {
        entity,
        action: RevivalActionKind::Reincarnate,
        issued_at_tick: 100,
    });
    app.update();

    // 第一次复活后：棺状态已清除
    assert!(
        app.world()
            .entity(entity)
            .get::<crate::coffin::CoffinComponent>()
            .is_none(),
        "第一次复活后 CoffinComponent 应为 None"
    );
    {
        let reg = app.world().resource::<crate::coffin::CoffinRegistry>();
        assert!(
            !reg.player_in_coffin.contains_key(&entity),
            "第一次复活后 player_in_coffin 应为空"
        );
    }

    // 模拟第二次入棺（ECS 加回 CoffinComponent，registry 重新 set_occupied）
    let lower2 = valence::prelude::BlockPos::new(30, 64, 30);
    app.world_mut()
        .entity_mut(entity)
        .insert(crate::coffin::CoffinComponent {
            entered_at_tick: 150,
            coffin_lower: lower2,
            grade: crate::coffin::CoffinGrade::Mundane,
        });
    {
        let world = app.world_mut();
        let mut entity_ref = world.entity_mut(entity);
        let mut lifecycle = entity_ref.get_mut::<Lifecycle>().unwrap();
        lifecycle.state = LifecycleState::AwaitingRevival;
        lifecycle.awaiting_decision = Some(RevivalDecision::Fortune { chance: 1.0 });
        lifecycle.revival_decision_deadline_tick = Some(300);
    }
    {
        let mut reg = app
            .world_mut()
            .resource_mut::<crate::coffin::CoffinRegistry>();
        reg.insert(lower2, 100, crate::coffin::CoffinGrade::Mundane);
        reg.set_occupied(lower2, entity);
    }

    // 第二次复活
    app.world_mut().send_event(RevivalActionIntent {
        entity,
        action: RevivalActionKind::Reincarnate,
        issued_at_tick: 200,
    });
    app.update();

    // 第二次复活后：棺状态同样清除
    assert!(
        app.world()
            .entity(entity)
            .get::<crate::coffin::CoffinComponent>()
            .is_none(),
        "第二次复活后 CoffinComponent 应为 None（循环应正常清除）"
    );
    {
        let reg = app.world().resource::<crate::coffin::CoffinRegistry>();
        assert!(
            !reg.player_in_coffin.contains_key(&entity),
            "第二次复活后 player_in_coffin 应为空（循环应正常清除）"
        );
    }

    let _ = fs::remove_dir_all(root);
}

/// 棺内玩家复活后，SQLite in_coffin 列必须被写为 false（0）。
/// 这是重启后唯一的权威来源（player/mod.rs:258 读 in_coffin=true 即重新复钉）。
///
/// 断言：load_player_slices(...).in_coffin == false（回读 SQLite，不依赖内存状态）
#[test]
fn revive_with_username_clears_sqlite_in_coffin() {
    let mut app = App::new();
    let (settings, root) = persistence_settings("sqlite-coffin-revive");
    let data_dir = root.join("data");

    app.insert_resource(settings.clone());
    app.insert_resource(PlayerStatePersistence::with_db_path(
        &data_dir,
        settings.db_path(),
    ));
    app.insert_resource(CombatClock { tick: 500 });
    app.add_event::<RevivalActionIntent>();
    app.add_event::<PlayerRevived>();
    app.add_event::<PlayerTerminated>();
    app.add_event::<AscensionQuotaOpened>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<QiTransfer>();
    app.add_event::<crate::coffin::CoffinStateChanged>();
    app.insert_resource(WorldQiAccount::default());
    app.add_systems(Update, handle_revival_action_intents);

    let username = Username("SQLiteCoffinRevive".to_string());
    let lifespan = crate::cultivation::lifespan::LifespanComponent {
        born_at_tick: 0,
        years_lived: 20.0,
        cap_by_realm: crate::cultivation::lifespan::LifespanCapTable::AWAKEN,
        offline_pause_tick: None,
    };
    let lower = valence::prelude::BlockPos::new(40, 64, 40);

    // 先写入 in_coffin=true 到 SQLite（模拟玩家断线前已入棺状态）
    crate::player::state::save_player_lifespan_slice_with_coffin(
        &PlayerStatePersistence::with_db_path(&data_dir, settings.db_path()),
        username.0.as_str(),
        &lifespan,
        Some(crate::coffin::CoffinGrade::Mundane),
    )
    .expect("pre-populate in_coffin=true 应成功");

    // 验证前置条件：SQLite 已有 in_coffin=true
    let before = crate::player::state::load_player_slices(
        &PlayerStatePersistence::with_db_path(&data_dir, settings.db_path()),
        username.0.as_str(),
    );
    assert!(
        before.in_coffin,
        "前置条件：SQLite in_coffin 应为 true（已写入），实际 false"
    );

    // 构造带 Username + LifespanComponent 的玩家 entity
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
                character_id: "offline:SQLiteCoffinRevive".to_string(),
                state: LifecycleState::AwaitingRevival,
                awaiting_decision: Some(RevivalDecision::Fortune { chance: 1.0 }),
                revival_decision_deadline_tick: Some(600),
                fortune_remaining: 1,
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
            LifeRecord::new("offline:SQLiteCoffinRevive"),
            lifespan.clone(),
            username.clone(),
            crate::coffin::CoffinComponent {
                entered_at_tick: 400,
                coffin_lower: lower,
                grade: crate::coffin::CoffinGrade::Mundane,
            },
        ))
        .id();

    seed_revival_entity_bundle(&mut app, &settings, entity, username.0.as_str());

    {
        let mut reg = crate::coffin::CoffinRegistry::default();
        reg.insert(lower, 0, crate::coffin::CoffinGrade::Mundane);
        reg.set_occupied(lower, entity);
        app.insert_resource(reg);
    }

    // 触发复活
    app.world_mut().send_event(RevivalActionIntent {
        entity,
        action: RevivalActionKind::Reincarnate,
        issued_at_tick: 500,
    });
    app.update();

    // 核心断言：SQLite in_coffin 必须为 false（回读验证，不依赖 ECS 内存）
    let after = crate::player::state::load_player_slices(
        &PlayerStatePersistence::with_db_path(&data_dir, settings.db_path()),
        username.0.as_str(),
    );
    assert!(
        !after.in_coffin,
        "期望复活后 SQLite in_coffin=false（重启不应再复钉），实际 in_coffin=true"
    );
    assert!(
        after.coffin_grade.is_none(),
        "期望复活后 SQLite coffin_grade=None（清棺），实际 {:?}",
        after.coffin_grade
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn death_arbiter_immediately_publishes_fortune_decision() {
    let mut app = App::new();
    let (settings, root) = persistence_settings("revive-existing");
    app.insert_resource(settings);
    app.insert_resource(CombatClock { tick: 100 });
    app.add_event::<DeathEvent>();
    app.add_event::<CultivationDeathTrigger>();
    app.add_event::<DeathInsightRequested>();
    app.add_event::<DeathCinematicPublished>();
    app.add_event::<PlayerRevived>();
    app.add_event::<PlayerTerminated>();
    app.add_event::<RevivalActionIntent>();
    app.add_event::<AscensionQuotaOpened>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<QiTransfer>();
    app.add_event::<crate::coffin::CoffinStateChanged>();
    app.insert_resource(WorldQiAccount::default());
    app.add_systems(
        Update,
        (
            death_arbiter_tick,
            handle_revival_action_intents.after(death_arbiter_tick),
        ),
    );

    let (entity, mut helper) = spawn_client_actor(
        &mut app,
        "Azure",
        Wounds {
            health_current: 0.0,
            health_max: 30.0,
            entries: Vec::new(),
        },
        Stamina::default(),
        Lifecycle {
            fortune_remaining: 1,
            ..Default::default()
        },
    );

    app.world_mut().send_event(DeathEvent {
        target: entity,
        cause: "test".to_string(),
        attacker: None,
        attacker_player_id: None,
        at_tick: 100,
    });
    app.update();

    {
        let lifecycle = app.world().entity(entity).get::<Lifecycle>().unwrap();
        assert_eq!(lifecycle.state, LifecycleState::AwaitingRevival);
        assert_eq!(lifecycle.death_count, 1);
        let insight_events = app.world().resource::<Events<DeathInsightRequested>>();
        let mut insight_reader = insight_events.get_reader();
        let insights: Vec<_> = insight_reader.read(insight_events).cloned().collect();
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].payload.character_id, "offline:Azure");
        assert_eq!(insights[0].payload.cause, "test");
        assert_eq!(insights[0].payload.category, DeathInsightCategoryV1::Combat);
    }

    flush_client_packets(&mut app);

    let lifecycle = app.world().entity(entity).get::<Lifecycle>().unwrap();
    let revived_events = app.world().resource::<Events<PlayerRevived>>();
    assert_eq!(lifecycle.state, LifecycleState::AwaitingRevival);
    assert!(matches!(
        lifecycle.awaiting_decision,
        Some(RevivalDecision::Fortune { chance }) if (chance - 1.0).abs() < 1e-9
    ));
    assert_eq!(revived_events.len(), 0);

    let payloads = collect_server_data_payloads(&mut helper);
    assert!(payloads.iter().any(|payload| matches!(
        payload.payload,
        ServerDataPayloadV1::DeathScreen {
            visible: true,
            can_reincarnate: true,
            can_terminate: false,
            stage: Some(DeathScreenStageV1::Fortune),
            cinematic: Some(DeathCinematicS2cV1 {
                phase: crate::schema::death_cinematic::DeathCinematicPhaseV1::Roll,
                ..
            }),
            ..
        }
    )));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn fortune_stage_death_screen_disables_voluntary_termination() {
    let mut app = App::new();
    let (settings, root) = persistence_settings("fortune-no-terminate-button");
    app.insert_resource(settings);
    app.insert_resource(CombatClock { tick: 100 });
    app.add_event::<DeathEvent>();
    app.add_event::<CultivationDeathTrigger>();
    app.add_event::<DeathInsightRequested>();
    app.add_event::<DeathCinematicPublished>();
    app.add_event::<PlayerRevived>();
    app.add_event::<PlayerTerminated>();
    app.add_event::<RevivalActionIntent>();
    app.add_event::<AscensionQuotaOpened>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<QiTransfer>();
    app.add_event::<crate::coffin::CoffinStateChanged>();
    app.insert_resource(WorldQiAccount::default());
    app.add_systems(
        Update,
        (
            death_arbiter_tick,
            handle_revival_action_intents.after(death_arbiter_tick),
        ),
    );

    let (entity, mut helper) = spawn_client_actor(
        &mut app,
        "FortuneOnly",
        Wounds {
            health_current: 0.0,
            health_max: 30.0,
            entries: Vec::new(),
        },
        Stamina::default(),
        Lifecycle {
            fortune_remaining: 1,
            ..Default::default()
        },
    );

    app.world_mut().send_event(DeathEvent {
        target: entity,
        cause: "test".to_string(),
        attacker: None,
        attacker_player_id: None,
        at_tick: 100,
    });
    app.update();
    app.world_mut().resource_mut::<CombatClock>().tick = 701;
    app.update();
    flush_client_packets(&mut app);

    let payloads = collect_server_data_payloads(&mut helper);
    assert!(payloads.iter().any(|payload| matches!(
        payload.payload,
        ServerDataPayloadV1::DeathScreen {
            visible: true,
            can_reincarnate: true,
            can_terminate: false,
            stage: Some(DeathScreenStageV1::Fortune),
            ..
        }
    )));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn reconnect_while_awaiting_revival_tribulation_reemits_death_screen_and_cinematic() {
    let mut app = App::new();
    app.insert_resource(CombatClock { tick: 500 });
    app.add_event::<DeathCinematicPublished>();
    app.add_systems(
        Update,
        reemit_death_screen_for_reconnected_awaiting_revival_clients,
    );

    let (entity, mut helper) = spawn_reconnected_client_actor(
        &mut app,
        "ReconnectTribulation",
        Lifecycle {
            character_id: "offline:ReconnectTribulation".to_string(),
            death_count: 2,
            fortune_remaining: 0,
            state: LifecycleState::AwaitingRevival,
            awaiting_decision: Some(RevivalDecision::Tribulation { chance: 0.2 }),
            revival_decision_deadline_tick: Some(560),
            ..Default::default()
        },
    );

    app.update();
    flush_client_packets(&mut app);

    let payloads = collect_server_data_payloads(&mut helper);
    assert!(
        payloads.iter().any(|payload| matches!(
            payload.payload,
            ServerDataPayloadV1::DeathScreen {
                visible: true,
                can_reincarnate: true,
                can_terminate: true,
                stage: Some(DeathScreenStageV1::Tribulation),
                ..
            }
        )),
        "重连时处于 AwaitingRevival + Tribulation 待决策的角色必须重新收到死亡屏\
             （can_terminate=true 因为 Tribulation 携带永久终结风险）；实际 payloads={payloads:?}"
    );

    let cinematic = app
        .world()
        .entity(entity)
        .get::<crate::death_lifecycle::cinematic::DeathCinematic>();
    assert!(
        cinematic.is_some(),
        "重连必须重新插入 DeathCinematic 组件，不能让玩家停在无 UI 的裸奔状态"
    );
}

#[test]
fn reconnect_while_awaiting_revival_fortune_reemits_death_screen_without_terminate_button() {
    let mut app = App::new();
    app.insert_resource(CombatClock { tick: 500 });
    app.add_event::<DeathCinematicPublished>();
    app.add_systems(
        Update,
        reemit_death_screen_for_reconnected_awaiting_revival_clients,
    );

    let (entity, mut helper) = spawn_reconnected_client_actor(
        &mut app,
        "ReconnectFortune",
        Lifecycle {
            character_id: "offline:ReconnectFortune".to_string(),
            fortune_remaining: 1,
            state: LifecycleState::AwaitingRevival,
            awaiting_decision: Some(RevivalDecision::Fortune { chance: 1.0 }),
            revival_decision_deadline_tick: Some(560),
            ..Default::default()
        },
    );

    app.update();
    flush_client_packets(&mut app);

    let payloads = collect_server_data_payloads(&mut helper);
    assert!(
        payloads.iter().any(|payload| matches!(
            payload.payload,
            ServerDataPayloadV1::DeathScreen {
                visible: true,
                can_reincarnate: true,
                can_terminate: false,
                stage: Some(DeathScreenStageV1::Fortune),
                ..
            }
        )),
        "Fortune 分支重连必须重新收到死亡屏，且 can_terminate=false（Fortune 不携带\
             永久终结风险，voluntary termination 按钮不应出现）；实际 payloads={payloads:?}"
    );

    assert!(
        app.world()
            .entity(entity)
            .get::<crate::death_lifecycle::cinematic::DeathCinematic>()
            .is_some(),
        "Fortune 分支重连同样必须重新插入 DeathCinematic"
    );
}

#[test]
fn termination_publishes_final_attributes_before_cleanup_and_does_not_resample() {
    let mut app = App::new();
    let (settings, _root) = persistence_settings("termination-summary");
    app.insert_resource(settings);
    app.insert_resource(WorldQiAccount::default());
    app.add_event::<PlayerTerminated>();
    app.add_event::<AscensionQuotaOpened>();
    app.add_event::<QiTransfer>();
    app.add_systems(
        Update,
        (
            crate::combat::termination::publish_termination,
            crate::cultivation::death_hooks::on_player_terminated,
        )
            .chain(),
    );
    let (entity, mut helper) = spawn_client_actor(
        &mut app,
        "LastName",
        Wounds {
            health_max: 72.0,
            ..Default::default()
        },
        Stamina::default(),
        Lifecycle {
            character_id: "offline:LastName".into(),
            state: LifecycleState::Terminated,
            death_count: 4,
            ..Default::default()
        },
    );
    app.world_mut().entity_mut(entity).insert((
        Cultivation {
            realm: Realm::Condense,
            qi_max: 88.0,
            ..Default::default()
        },
        MeridianSystem::default(),
    ));
    app.world_mut().send_event(PlayerTerminated { entity });
    app.update();
    assert!(
        app.world().get::<Cultivation>(entity).is_none(),
        "测试必须走真实终结清理"
    );
    flush_client_packets(&mut app);
    let payloads = collect_server_data_payloads(&mut helper);
    assert!(
        payloads.iter().any(|payload| matches!(&payload.payload,
            ServerDataPayloadV1::TerminateScreen { visible: true, summary: Some(summary), .. }
            if summary.realm == "Condense" && summary.qi_max == Some(88.0)
                && summary.health_max == Some(72.0) && summary.death_count == 4
        )),
        "终局协议必须携带清理之前的真实属性"
    );
    app.world_mut()
        .entity_mut(entity)
        .insert(Cultivation::default());
    app.update();
    flush_client_packets(&mut app);
    assert!(
        collect_server_data_payloads(&mut helper).is_empty(),
        "同一终局不能被后续组件快照覆盖"
    );
}

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
