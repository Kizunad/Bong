use super::*;
use std::collections::HashMap;

use serde_json::json;
use valence::prelude::{
    App, BlockPos, DVec3, EntityKind, Events, IntoSystemConfigs, Position, Update,
};
use valence::testing::ScenarioSingleClient;

use crate::npc::brain::{canonical_npc_id, NpcBehaviorConfig, DEFAULT_FLEE_THRESHOLD};
use crate::npc::faction::FactionStore;
use crate::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;
use crate::qi_physics::ledger::{pending_inflow_account, QiAccountId, QiTransferReason};
use crate::schema::agent_command::Command;
use crate::world::events::{ActiveEventsResource, EVENT_KARMA_BACKLASH, EVENT_THUNDER_TRIBULATION};
use crate::world::heartbeat::{HeartbeatEventKind, HeartbeatOverrideError};
use crate::world::karma::{TARGETED_CALAMITY_BASE_PROBABILITY, TARGETED_CALAMITY_MAX_PROBABILITY};
use crate::world::pseudo_vein_runtime::{PseudoVeinPhase, PseudoVeinRuntime, PSEUDO_VEIN_MAX_QI};

fn command(command_type: CommandType, target: &str, params: HashMap<String, Value>) -> Command {
    Command {
        command_type,
        target: target.to_string(),
        params,
    }
}

fn batch(id: &str, commands: Vec<Command>) -> AgentCommandV1 {
    AgentCommandV1 {
        v: 1,
        id: id.to_string(),
        source: Some("calamity".to_string()),
        commands,
    }
}

fn setup_executor_app() -> App {
    let scenario = ScenarioSingleClient::new();
    let mut app = scenario.app;
    app.insert_resource(crate::cultivation::known_techniques::TechniqueRegistry::load_for_tests());
    app.insert_resource(CommandExecutorResource::default());
    app.insert_resource(ZoneRegistry::fallback());
    app.insert_resource(ActiveEventsResource::default());
    app.insert_resource(NpcBehaviorConfig::default());
    app.insert_resource(NpcRegistry::default());
    app.insert_resource(FactionStore::default());
    app.insert_resource(KarmaWeightStore::default());
    app.insert_resource(QiDensityHeatmap::default());
    app.insert_resource(WorldQiAccount::default());
    app.add_event::<NpcSpawnNotice>();
    app.add_event::<FactionEventNotice>();
    app.add_event::<QiTransfer>();
    // plan-offscreen-war-v1 P6：headless 路径 B 出口
    app.add_event::<WarParticipateIntent>();
    // plan-era-state-v1 B1：生产路径 modify_zone{target="全局"} → EraDecreeIntent
    app.add_event::<EraDecreeIntent>();
    app.add_systems(Update, execute_agent_commands);
    app
}

#[test]
fn spawn_event_pseudo_vein_creates_runtime_component() {
    let mut app = setup_executor_app();
    let mut params = HashMap::new();
    params.insert("event".to_string(), json!("pseudo_vein"));
    params.insert("intensity".to_string(), json!(0.7));

    {
        let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
        let outcome = executor.enqueue_batch(batch(
            "cmd_pseudo_vein_runtime",
            vec![command(CommandType::SpawnEvent, "spawn", params)],
        ));
        assert!(outcome.accepted);
        assert!(!outcome.dedupe_drop);
    }

    app.update();

    let mut query = app.world_mut().query::<&PseudoVeinRuntime>();
    let runtimes = query.iter(app.world()).collect::<Vec<_>>();
    assert_eq!(runtimes.len(), 1);
    assert_eq!(runtimes[0].zone_id, "spawn");
    assert_eq!(runtimes[0].phase, PseudoVeinPhase::Rising);
}

#[test]
fn spawn_event_pseudo_vein_rejects_unknown_zone() {
    let mut app = setup_executor_app();
    let mut params = HashMap::new();
    params.insert("event".to_string(), json!("pseudo_vein"));
    params.insert("intensity".to_string(), json!(0.7));

    {
        let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
        let outcome = executor.enqueue_batch(batch(
            "cmd_pseudo_vein_unknown_zone",
            vec![command(CommandType::SpawnEvent, "missing_zone", params)],
        ));
        assert!(outcome.accepted);
        assert!(!outcome.dedupe_drop);
    }

    app.update();

    let mut query = app.world_mut().query::<&PseudoVeinRuntime>();
    let runtimes = query.iter(app.world()).collect::<Vec<_>>();
    assert!(
        runtimes.is_empty(),
        "pseudo_vein spawn_event should not create runtime for unknown zone"
    );
}

#[test]
fn spawn_event_pseudo_vein_injects_zone_qi_transfer() {
    // plan-zone-qi-economy-v1 P3 §8.1 决议 #3 — 灵潮注入从独立待分配池真实借出
    // （非凭空创生的 QiAccountId::tiandao()），需要先给池子充值才能借到全额。
    let mut app = setup_executor_app();
    app.world_mut()
        .resource_mut::<ZoneRegistry>()
        .find_zone_mut("spawn")
        .expect("fallback registry should contain spawn")
        .spirit_qi = 0.1;
    app.world_mut()
        .resource_mut::<WorldQiAccount>()
        .set_balance(pending_inflow_account(), 1000.0)
        .expect("seeding pending pool must succeed");
    let mut params = HashMap::new();
    params.insert("event".to_string(), json!("pseudo_vein"));

    {
        let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
        let outcome = executor.enqueue_batch(batch(
            "cmd_pseudo_vein_injects_qi",
            vec![command(CommandType::SpawnEvent, "spawn", params)],
        ));
        assert!(outcome.accepted);
        assert!(!outcome.dedupe_drop);
    }

    app.update();

    let zone_qi = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name("spawn")
        .expect("spawn zone should remain registered")
        .spirit_qi;
    assert_eq!(
        zone_qi, PSEUDO_VEIN_MAX_QI,
        "a well-funded pending pool must let the zone reach the full pseudo-vein target"
    );
    let expected_absolute = (PSEUDO_VEIN_MAX_QI - 0.1) * QI_ZONE_UNIT_CAPACITY;
    let transfers = app
        .world()
        .resource::<Events<QiTransfer>>()
        .iter_current_update_events()
        .collect::<Vec<_>>();
    assert_eq!(transfers.len(), 1);
    assert_eq!(
        transfers[0].from,
        pending_inflow_account(),
        "灵潮注入必须从独立待分配池真实借出，不是凭空创生的 tiandao 账户（§8.1 决议 #3）"
    );
    assert_eq!(transfers[0].to, QiAccountId::zone("spawn"));
    assert_eq!(transfers[0].amount, expected_absolute);
    assert_eq!(transfers[0].reason, QiTransferReason::ReleaseToZone);
    assert_eq!(
        app.world()
            .resource::<WorldQiAccount>()
            .balance(&pending_inflow_account()),
        1000.0 - expected_absolute,
        "the pending pool must be debited by exactly the amount credited to the zone \
             (conservation: no qi created out of thin air)"
    );
}

#[test]
fn spawn_event_pseudo_vein_is_idempotent_for_active_zone() {
    let mut app = setup_executor_app();
    app.world_mut()
        .resource_mut::<ZoneRegistry>()
        .find_zone_mut("spawn")
        .expect("fallback registry should contain spawn")
        .spirit_qi = 0.1;
    app.world_mut()
        .resource_mut::<WorldQiAccount>()
        .set_balance(pending_inflow_account(), 1000.0)
        .expect("seeding pending pool must succeed");
    let mut params = HashMap::new();
    params.insert("event".to_string(), json!("pseudo_vein"));

    {
        let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
        let outcome = executor.enqueue_batch(batch(
            "cmd_pseudo_vein_once",
            vec![command(CommandType::SpawnEvent, "spawn", params.clone())],
        ));
        assert!(outcome.accepted);
    }
    app.update();

    {
        let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
        let outcome = executor.enqueue_batch(batch(
            "cmd_pseudo_vein_retry",
            vec![command(CommandType::SpawnEvent, "spawn", params)],
        ));
        assert!(outcome.accepted);
    }
    app.update();

    let mut query = app.world_mut().query::<&PseudoVeinRuntime>();
    let runtimes = query.iter(app.world()).collect::<Vec<_>>();
    assert_eq!(runtimes.len(), 1);
    assert_eq!(runtimes[0].zone_id, "spawn");
    assert_eq!(
        runtimes[0].injected_qi,
        (PSEUDO_VEIN_MAX_QI - 0.1) * QI_ZONE_UNIT_CAPACITY
    );
}

#[test]
fn spawn_event_pseudo_vein_rejects_same_batch_duplicate() {
    let mut app = setup_executor_app();
    app.world_mut()
        .resource_mut::<ZoneRegistry>()
        .find_zone_mut("spawn")
        .expect("fallback registry should contain spawn")
        .spirit_qi = 0.1;
    app.world_mut()
        .resource_mut::<WorldQiAccount>()
        .set_balance(pending_inflow_account(), 1000.0)
        .expect("seeding pending pool must succeed");
    let mut params = HashMap::new();
    params.insert("event".to_string(), json!("pseudo_vein"));

    {
        let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
        let outcome = executor.enqueue_batch(batch(
            "cmd_pseudo_vein_same_batch_duplicate",
            vec![
                command(CommandType::SpawnEvent, "spawn", params.clone()),
                command(CommandType::SpawnEvent, "spawn", params),
            ],
        ));
        assert!(outcome.accepted);
    }

    app.update();

    let mut query = app.world_mut().query::<&PseudoVeinRuntime>();
    let runtimes = query.iter(app.world()).collect::<Vec<_>>();
    assert_eq!(runtimes.len(), 1);
    assert_eq!(runtimes[0].zone_id, "spawn");
    assert_eq!(
        runtimes[0].injected_qi,
        (PSEUDO_VEIN_MAX_QI - 0.1) * QI_ZONE_UNIT_CAPACITY
    );
    let transfers = app
        .world()
        .resource::<Events<QiTransfer>>()
        .iter_current_update_events()
        .collect::<Vec<_>>();
    assert_eq!(
        transfers.len(),
        1,
        "same-batch duplicate should not emit a second pseudo vein injection"
    );
}

#[test]
fn spawn_event_applies_hidden_karma_probability_weighting() {
    let mut app = setup_executor_app();
    app.world_mut()
        .resource_mut::<KarmaWeightStore>()
        .mark_player(
            "Azure",
            Some("spawn".to_string()),
            BlockPos::new(8, 66, 8),
            1.0,
            99,
        );

    let mut params = HashMap::new();
    params.insert("event".to_string(), json!(EVENT_KARMA_BACKLASH));
    params.insert("intensity".to_string(), json!(0.2));
    params.insert("duration_ticks".to_string(), json!(3));

    {
        let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
        let outcome = executor.enqueue_batch(batch(
            "cmd_karma_backlash_weighted",
            vec![command(CommandType::SpawnEvent, "spawn", params)],
        ));
        assert!(outcome.accepted);
        assert!(!outcome.dedupe_drop);
    }

    app.update();

    let events = app.world().resource::<ActiveEventsResource>();
    let recent = events.recent_events_snapshot();
    let marker = recent
        .iter()
        .find(|event| event.target.as_deref() == Some(EVENT_KARMA_BACKLASH))
        .expect("karma backlash should record hidden marker");
    let details = marker.details.as_ref().expect("hidden marker details");
    assert_eq!(
        details.get("command_intensity").and_then(Value::as_f64),
        Some(0.2)
    );
    assert_eq!(
        details.get("karma_weight").and_then(Value::as_f64),
        Some(1.0)
    );
    assert_eq!(
        details.get("base_probability").and_then(Value::as_f64),
        Some(f64::from(TARGETED_CALAMITY_BASE_PROBABILITY))
    );
    assert_eq!(
        details.get("effective_probability").and_then(Value::as_f64),
        Some(f64::from(TARGETED_CALAMITY_MAX_PROBABILITY))
    );
}

#[test]
fn heartbeat_override_applies_via_executor_with_clock_fallback() {
    let mut app = setup_executor_app();
    let mut heartbeat = WorldHeartbeat::default();
    heartbeat.last_eval_tick = 10_000;
    heartbeat.eval_interval_ticks = 200;
    app.world_mut().insert_resource(heartbeat);

    let mut params = HashMap::new();
    params.insert("action".to_string(), json!("accelerate"));
    params.insert("event_type".to_string(), json!("beast_tide"));
    params.insert("target_zone".to_string(), json!("spawn"));
    params.insert("duration_ticks".to_string(), json!(600));
    params.insert("intensity_override".to_string(), json!(0.25));

    {
        let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
        let outcome = executor.enqueue_batch(batch(
            "cmd_heartbeat_override_ok",
            vec![command(CommandType::HeartbeatOverride, "spawn", params)],
        ));
        assert!(outcome.accepted);
        assert!(!outcome.dedupe_drop);
    }

    app.update();

    let heartbeat = app.world().resource::<WorldHeartbeat>();
    let override_ = heartbeat
        .override_for(HeartbeatEventKind::BeastTide, "spawn")
        .expect("heartbeat override should be stored");
    assert_eq!(
        override_.expires_at_tick, 10_800,
        "missing CultivationClock should fall back to last_eval_tick + eval_interval_ticks"
    );
    assert_eq!(
        override_.intensity_override,
        Some(0.25),
        "accelerate override should preserve configured intensity"
    );
}

#[test]
fn heartbeat_override_returns_missing_heartbeat_without_resource() {
    let mut heartbeat = None;
    let command = command(
        CommandType::HeartbeatOverride,
        "spawn",
        HashMap::from([
            ("action".to_string(), json!("accelerate")),
            ("event_type".to_string(), json!("beast_tide")),
        ]),
    );

    let result = execute_heartbeat_override(&command, &mut heartbeat, Some(1_000));
    assert_eq!(
        result,
        HeartbeatOverrideError::MissingHeartbeat.result_label(),
        "missing heartbeat resource should reject the command"
    );
}

#[test]
fn faction_event_updates_store_and_records_recent_event() {
    let mut app = setup_executor_app();

    let mut params = HashMap::new();
    params.insert("kind".to_string(), json!("enqueue_mission"));
    params.insert("faction_id".to_string(), json!("neutral"));
    params.insert("mission_id".to_string(), json!("mission:hold_spawn_gate"));

    {
        let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
        let outcome = executor.enqueue_batch(batch(
            "cmd_faction_event_ok",
            vec![command(CommandType::FactionEvent, "neutral", params)],
        ));
        assert!(outcome.accepted);
        assert!(!outcome.dedupe_drop);
    }

    app.update();

    let store = app.world().resource::<FactionStore>();
    let neutral = store
        .iter()
        .find(|faction| faction.id == FactionId::Neutral)
        .expect("neutral faction should exist");
    assert_eq!(neutral.mission_queue.pending_count(), 1);
    assert_eq!(
        neutral.mission_queue.top_mission_id(),
        Some("mission:hold_spawn_gate")
    );

    let events = app.world().resource::<ActiveEventsResource>();
    let recent = events.recent_events_snapshot();
    assert_eq!(recent.len(), 1);
    assert_eq!(recent[0].event_type, GameEventType::EventTriggered);
    assert_eq!(recent[0].target.as_deref(), Some("faction:neutral"));
    assert_eq!(
        recent[0]
            .details
            .as_ref()
            .and_then(|details| details.get("kind"))
            .and_then(Value::as_str),
        Some("enqueue_mission")
    );
    let notices = app
        .world()
        .resource::<valence::prelude::Events<FactionEventNotice>>();
    assert_eq!(notices.len(), 1);
}

#[test]
fn faction_event_rejects_invalid_or_unknown_faction_inputs() {
    let mut app = setup_executor_app();

    let mut commands = Vec::new();

    let mut invalid_kind = HashMap::new();
    invalid_kind.insert("kind".to_string(), json!("invent_new_faction_law"));
    invalid_kind.insert("faction_id".to_string(), json!("neutral"));
    commands.push(command(CommandType::FactionEvent, "neutral", invalid_kind));

    let mut unknown_faction = HashMap::new();
    unknown_faction.insert("kind".to_string(), json!("enqueue_mission"));
    unknown_faction.insert("faction_id".to_string(), json!("sky"));
    unknown_faction.insert("mission_id".to_string(), json!("mission:unknown"));
    commands.push(command(CommandType::FactionEvent, "sky", unknown_faction));

    let mut missing_payload = HashMap::new();
    missing_payload.insert("kind".to_string(), json!("adjust_loyalty_bias"));
    missing_payload.insert("faction_id".to_string(), json!("neutral"));
    commands.push(command(
        CommandType::FactionEvent,
        "neutral",
        missing_payload,
    ));

    {
        let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
        let outcome = executor.enqueue_batch(batch("cmd_faction_event_rejects", commands));
        assert!(outcome.accepted);
        assert!(!outcome.dedupe_drop);
    }

    app.update();

    let store = app.world().resource::<FactionStore>();
    let neutral = store
        .iter()
        .find(|faction| faction.id == FactionId::Neutral)
        .expect("neutral faction should exist");
    assert_eq!(neutral.mission_queue.pending_count(), 0);
    assert!((neutral.loyalty_bias - 0.5).abs() < 1e-9);

    let events = app.world().resource::<ActiveEventsResource>();
    assert!(events.recent_events_snapshot().is_empty());
}

#[test]
fn despawn_npc_marks_live_target_as_despawned() {
    let mut app = setup_executor_app();

    let npc = app.world_mut().spawn(NpcMarker).id();
    let npc_id = canonical_npc_id(npc);

    {
        let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
        let outcome = executor.enqueue_batch(batch(
            "cmd_despawn_npc_ok",
            vec![command(
                CommandType::DespawnNpc,
                npc_id.as_str(),
                HashMap::new(),
            )],
        ));
        assert!(outcome.accepted);
        assert!(!outcome.dedupe_drop);
    }

    app.update();

    let live_npcs = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, (With<NpcMarker>, Without<Despawned>)>();
        query.iter(world).collect::<Vec<_>>()
    };
    assert!(live_npcs.is_empty());
}

#[test]
fn despawn_npc_rejects_invalid_or_unknown_targets() {
    let mut app = setup_executor_app();
    let live_npc = app.world_mut().spawn(NpcMarker).id();
    let live_npc_id = canonical_npc_id(live_npc);
    let missing_npc_id = format!("npc_{}v1", live_npc.index() + 99_999);

    {
        let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
        let outcome = executor.enqueue_batch(batch(
            "cmd_despawn_npc_rejects",
            vec![
                command(CommandType::DespawnNpc, "npc_123", HashMap::new()),
                command(
                    CommandType::DespawnNpc,
                    missing_npc_id.as_str(),
                    HashMap::new(),
                ),
            ],
        ));
        assert!(outcome.accepted);
        assert!(!outcome.dedupe_drop);
    }

    app.update();

    let live_npcs_after_rejects = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, (With<NpcMarker>, Without<Despawned>)>();
        query.iter(world).collect::<Vec<_>>()
    };
    assert_eq!(live_npcs_after_rejects, vec![live_npc]);

    let mut behavior_params = HashMap::new();
    behavior_params.insert("flee_threshold".to_string(), json!(0.2));

    {
        let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
        let outcome = executor.enqueue_batch(batch(
            "cmd_despawn_npc_then_behavior",
            vec![
                command(
                    CommandType::DespawnNpc,
                    live_npc_id.as_str(),
                    HashMap::new(),
                ),
                command(
                    CommandType::NpcBehavior,
                    live_npc_id.as_str(),
                    behavior_params,
                ),
            ],
        ));
        assert!(outcome.accepted);
        assert!(!outcome.dedupe_drop);
    }

    app.update();

    let live_npcs_after_despawn = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, (With<NpcMarker>, Without<Despawned>)>();
        query.iter(world).collect::<Vec<_>>()
    };
    assert!(live_npcs_after_despawn.is_empty());

    let behavior = app.world().resource::<NpcBehaviorConfig>();
    assert_eq!(
        behavior.threshold_for_npc_id(live_npc_id.as_str()),
        DEFAULT_FLEE_THRESHOLD
    );
}

#[test]
fn spawn_npc_creates_zombie_in_requested_zone() {
    let mut app = setup_executor_app();

    let mut params = HashMap::new();
    params.insert("archetype".to_string(), json!("zombie"));

    {
        let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
        let outcome = executor.enqueue_batch(batch(
            "cmd_spawn_npc_ok",
            vec![command(CommandType::SpawnNpc, "spawn", params)],
        ));
        assert!(outcome.accepted);
        assert!(!outcome.dedupe_drop);
    }

    app.update();

    let npcs = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
        query.iter(world).collect::<Vec<_>>()
    };
    assert_eq!(npcs.len(), 1);

    let npc = npcs[0];
    let npc_archetype = app
        .world()
        .get::<NpcArchetype>(npc)
        .expect("spawned npc should have archetype");
    assert_eq!(*npc_archetype, NpcArchetype::Zombie);

    let spawn_zone = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name("spawn")
        .expect("spawn zone should exist")
        .clone();

    let patrol = app
        .world()
        .get::<crate::npc::patrol::NpcPatrol>(npc)
        .expect("spawned npc should have patrol state");
    assert_eq!(patrol.home_zone, "spawn");
    assert!(patrol.current_target.distance_squared(spawn_zone.center()) < 1e-9);

    let position = app
        .world()
        .get::<Position>(npc)
        .expect("spawned npc should have position");
    assert!(spawn_zone.contains(position.get()));
    let expected_spawn = spawn_zone
        .patrol_anchors
        .first()
        .copied()
        .unwrap_or_else(|| spawn_zone.center());
    assert!(position.get().distance_squared(expected_spawn) < 1e-9);

    let kind = app
        .world()
        .get::<EntityKind>(npc)
        .expect("spawned npc should have entity kind");
    assert_eq!(*kind, EntityKind::ZOMBIE);

    let registry = app.world().resource::<NpcRegistry>();
    assert_eq!(registry.live_npc_count, 1);
}

#[test]
fn spawn_npc_creates_commoner_when_archetype_param_is_commoner() {
    let mut app = setup_executor_app();

    let mut params = HashMap::new();
    params.insert("archetype".to_string(), json!("commoner"));
    params.insert("initial_age_ticks".to_string(), json!(42.0));

    {
        let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
        let outcome = executor.enqueue_batch(batch(
            "cmd_spawn_npc_commoner",
            vec![command(CommandType::SpawnNpc, "spawn", params)],
        ));
        assert!(outcome.accepted);
    }

    app.update();

    let npcs = {
        let world = app.world_mut();
        let mut query =
            world.query_filtered::<(Entity, &NpcArchetype, &EntityKind), With<NpcMarker>>();
        query
            .iter(world)
            .map(|(e, a, k)| (e, *a, *k))
            .collect::<Vec<_>>()
    };
    assert_eq!(npcs.len(), 1);
    let (entity, archetype, kind) = npcs[0];
    assert_eq!(archetype, NpcArchetype::Commoner);
    assert_eq!(kind, EntityKind::VILLAGER);

    let lifespan = app
        .world()
        .get::<crate::npc::lifecycle::NpcLifespan>(entity)
        .expect("commoner should include lifespan");
    assert_eq!(lifespan.age_ticks, 42.0);

    let hunger = app
        .world()
        .get::<crate::npc::hunger::Hunger>(entity)
        .expect("commoner should include Hunger component");
    assert_eq!(hunger.value, 1.0);
}

#[test]
fn spawn_npc_creates_rogue_when_archetype_param_is_rogue() {
    let mut app = setup_executor_app();

    let mut params = HashMap::new();
    params.insert("archetype".to_string(), json!("rogue"));
    params.insert("initial_age_ticks".to_string(), json!(5000.0));

    {
        let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
        let outcome = executor.enqueue_batch(batch(
            "cmd_spawn_npc_rogue",
            vec![command(CommandType::SpawnNpc, "spawn", params)],
        ));
        assert!(outcome.accepted);
    }

    app.update();

    let npcs = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<(Entity, &NpcArchetype), With<NpcMarker>>();
        query.iter(world).map(|(e, a)| (e, *a)).collect::<Vec<_>>()
    };
    assert_eq!(npcs.len(), 1);
    assert_eq!(npcs[0].1, NpcArchetype::Rogue);

    let lifespan = app
        .world()
        .get::<crate::npc::lifecycle::NpcLifespan>(npcs[0].0)
        .unwrap();
    assert_eq!(lifespan.age_ticks, 5000.0);
}

#[test]
fn agent_spawn_npc_propagates_authoritative_registry_to_loadout() {
    // M06：agent-command NPC spawn 必须把注入的权威 registry 传给 constructor，
    // 产出的 loadout 跟随 runtime override。若 execute_spawn_npc 忽略参数并改读
    // 默认/静态 catalog，runtime-only 招式会从 spawn 出的 NPC KnownTechniques
    // 里消失并撞红。
    let mut app = setup_executor_app();
    app.insert_resource(
        crate::cultivation::known_techniques::TechniqueRegistry::load_from_contents_for_tests(
            r#"
[[techniques]]
 id = "runtime.only"
 display_name = "运行时专属"
 grade = "common"
 description = "只存在于注入 registry 的 runtime-only 招式（M06 契约）。"
 required_realm = "Awaken"
 required_meridians = []
 required_race = { kind = "any" }
 qi_cost = 1.0
 stamina_cost = 1.0
 cast_ticks = 10
 cooldown_ticks = 20
 range = 3.0
 icon_texture = "bong-client:textures/gui/items/skill_scroll_runtime_only.png"
 category = "attack"
 dispatch = "metadata_backed"
"#,
        )
        .expect("runtime-only test catalog must load"),
    );

    let mut params = HashMap::new();
    params.insert("archetype".to_string(), json!("rogue"));

    {
        let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
        let outcome = executor.enqueue_batch(batch(
            "cmd_spawn_npc_rogue_runtime",
            vec![command(CommandType::SpawnNpc, "spawn", params)],
        ));
        assert!(outcome.accepted);
    }

    app.update();

    let npc = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<(Entity, &NpcArchetype), With<NpcMarker>>();
        query.iter(world).next().map(|(e, a)| (e, *a)).unwrap()
    };
    assert_eq!(npc.1, NpcArchetype::Rogue);
    let known = app
        .world()
        .get::<crate::cultivation::known_techniques::KnownTechniques>(npc.0)
        .expect("spawned NPC must carry KnownTechniques loadout");
    assert!(
        known.entries.iter().any(|entry| entry.id == "runtime.only"),
        "spawned NPC loadout must follow the injected authoritative registry (M06)"
    );
}

#[test]
fn spawn_npc_count_spawns_batch_and_clamps_to_remaining_budget() {
    let mut app = setup_executor_app();

    let mut params = HashMap::new();
    params.insert("archetype".to_string(), json!("rogue"));
    params.insert("count".to_string(), json!(3));

    {
        let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
        let outcome = executor.enqueue_batch(batch(
            "cmd_spawn_npc_count",
            vec![command(CommandType::SpawnNpc, "spawn", params)],
        ));
        assert!(outcome.accepted);
    }

    app.update();

    let rogue_count = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<&NpcArchetype, With<NpcMarker>>();
        query
            .iter(world)
            .filter(|archetype| **archetype == NpcArchetype::Rogue)
            .count()
    };
    assert_eq!(rogue_count, 3);
    assert_eq!(app.world().resource::<NpcRegistry>().live_npc_count, 3);

    {
        let mut registry = app.world_mut().resource_mut::<NpcRegistry>();
        registry.live_npc_count = registry.max_npc_count - 1;
        registry.spawn_paused = false;
    }
    let mut clamped_params = HashMap::new();
    clamped_params.insert("archetype".to_string(), json!("rogue"));
    clamped_params.insert("count".to_string(), json!(5));

    {
        let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
        let outcome = executor.enqueue_batch(batch(
            "cmd_spawn_npc_count_clamped",
            vec![command(CommandType::SpawnNpc, "spawn", clamped_params)],
        ));
        assert!(outcome.accepted);
    }

    app.update();

    let all_rogues = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<&NpcArchetype, With<NpcMarker>>();
        query
            .iter(world)
            .filter(|archetype| **archetype == NpcArchetype::Rogue)
            .count()
    };
    assert_eq!(
        all_rogues, 4,
        "second batch should reserve only one remaining slot"
    );
    assert_eq!(
        app.world().resource::<NpcRegistry>().live_npc_count,
        app.world().resource::<NpcRegistry>().max_npc_count
    );
}

#[test]
fn spawn_npc_rejects_unknown_zone_unsupported_archetype_and_exhausted_budget() {
    let mut app = setup_executor_app();

    let mut commands = Vec::new();

    let mut bad_zone = HashMap::new();
    bad_zone.insert("archetype".to_string(), json!("zombie"));
    commands.push(command(CommandType::SpawnNpc, "missing_zone", bad_zone));

    let mut bad_archetype = HashMap::new();
    bad_archetype.insert("archetype".to_string(), json!("immortal"));
    commands.push(command(CommandType::SpawnNpc, "spawn", bad_archetype));

    {
        let mut registry = app.world_mut().resource_mut::<NpcRegistry>();
        registry.live_npc_count = registry.max_npc_count;
        registry.spawn_paused = true;
    }

    let mut exhausted_budget = HashMap::new();
    exhausted_budget.insert("archetype".to_string(), json!("zombie"));
    commands.push(command(CommandType::SpawnNpc, "spawn", exhausted_budget));

    {
        let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
        let outcome = executor.enqueue_batch(batch("cmd_spawn_npc_rejects", commands));
        assert!(outcome.accepted);
        assert!(!outcome.dedupe_drop);
    }

    app.update();

    let npcs = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
        query.iter(world).collect::<Vec<_>>()
    };
    assert!(npcs.is_empty());
}

#[test]
fn spawn_npc_rejects_missing_archetype_param() {
    let mut app = setup_executor_app();

    {
        let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
        let outcome = executor.enqueue_batch(batch(
            "cmd_spawn_npc_missing_archetype",
            vec![command(CommandType::SpawnNpc, "spawn", HashMap::new())],
        ));
        assert!(outcome.accepted);
        assert!(!outcome.dedupe_drop);
    }

    app.update();

    let npcs = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
        query.iter(world).collect::<Vec<_>>()
    };
    assert!(npcs.is_empty());
}

// ——— TSY archetype spawn tests (r4-P3#2 fix) ———

/// Verifies that `spawn_npc` with archetype=daoxiang creates a Daoxiang NPC
/// (previously rejected as unsupported, silently discarding the agent command).
#[test]
fn spawn_npc_creates_daoxiang_when_archetype_param_is_daoxiang() {
    let mut app = setup_executor_app();

    let mut params = HashMap::new();
    params.insert("archetype".to_string(), json!("daoxiang"));

    {
        let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
        let outcome = executor.enqueue_batch(batch(
            "cmd_spawn_npc_daoxiang",
            vec![command(CommandType::SpawnNpc, "spawn", params)],
        ));
        assert!(
            outcome.accepted,
            "spawn_npc daoxiang command should be accepted (was enqueued)"
        );
    }

    app.update();

    let npcs = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<(Entity, &NpcArchetype), With<NpcMarker>>();
        query.iter(world).map(|(e, a)| (e, *a)).collect::<Vec<_>>()
    };
    assert_eq!(
        npcs.len(),
        1,
        "expected 1 NPC spawned for daoxiang archetype; got {} — \
             spawn_npc was silently dropping TSY archetypes before this fix",
        npcs.len()
    );
    assert_eq!(
        npcs[0].1,
        NpcArchetype::Daoxiang,
        "expected NpcArchetype::Daoxiang, got {:?} — archetype tag mismatch",
        npcs[0].1
    );

    let registry = app.world().resource::<NpcRegistry>();
    assert_eq!(
        registry.live_npc_count, 1,
        "NpcRegistry should reflect 1 live NPC after daoxiang spawn"
    );
}

/// Verifies that `spawn_npc` with archetype=zhinian creates a Zhinian NPC.
#[test]
fn spawn_npc_creates_zhinian_when_archetype_param_is_zhinian() {
    let mut app = setup_executor_app();

    let mut params = HashMap::new();
    params.insert("archetype".to_string(), json!("zhinian"));

    {
        let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
        let outcome = executor.enqueue_batch(batch(
            "cmd_spawn_npc_zhinian",
            vec![command(CommandType::SpawnNpc, "spawn", params)],
        ));
        assert!(
            outcome.accepted,
            "spawn_npc zhinian command should be accepted"
        );
    }

    app.update();

    let npcs = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<(Entity, &NpcArchetype), With<NpcMarker>>();
        query.iter(world).map(|(e, a)| (e, *a)).collect::<Vec<_>>()
    };
    assert_eq!(
        npcs.len(),
        1,
        "expected 1 NPC spawned for zhinian archetype; got {}",
        npcs.len()
    );
    assert_eq!(
        npcs[0].1,
        NpcArchetype::Zhinian,
        "expected NpcArchetype::Zhinian, got {:?}",
        npcs[0].1
    );

    let registry = app.world().resource::<NpcRegistry>();
    assert_eq!(
        registry.live_npc_count, 1,
        "NpcRegistry should reflect 1 live NPC after zhinian spawn"
    );
}

/// Verifies that `spawn_npc` with archetype=fuya creates a Fuya NPC.
#[test]
fn spawn_npc_creates_fuya_when_archetype_param_is_fuya() {
    let mut app = setup_executor_app();

    let mut params = HashMap::new();
    params.insert("archetype".to_string(), json!("fuya"));

    {
        let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
        let outcome = executor.enqueue_batch(batch(
            "cmd_spawn_npc_fuya",
            vec![command(CommandType::SpawnNpc, "spawn", params)],
        ));
        assert!(
            outcome.accepted,
            "spawn_npc fuya command should be accepted"
        );
    }

    app.update();

    let npcs = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<(Entity, &NpcArchetype), With<NpcMarker>>();
        query.iter(world).map(|(e, a)| (e, *a)).collect::<Vec<_>>()
    };
    assert_eq!(
        npcs.len(),
        1,
        "expected 1 NPC spawned for fuya archetype; got {}",
        npcs.len()
    );
    assert_eq!(
        npcs[0].1,
        NpcArchetype::Fuya,
        "expected NpcArchetype::Fuya, got {:?}",
        npcs[0].1
    );

    let registry = app.world().resource::<NpcRegistry>();
    assert_eq!(
        registry.live_npc_count, 1,
        "NpcRegistry should reflect 1 live NPC after fuya spawn"
    );
}

/// Verifies that `spawn_npc` with archetype=skull_fiend creates a SkullFiend NPC.
#[test]
fn spawn_npc_creates_skull_fiend_when_archetype_param_is_skull_fiend() {
    let mut app = setup_executor_app();

    let mut params = HashMap::new();
    params.insert("archetype".to_string(), json!("skull_fiend"));

    {
        let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
        let outcome = executor.enqueue_batch(batch(
            "cmd_spawn_npc_skull_fiend",
            vec![command(CommandType::SpawnNpc, "spawn", params)],
        ));
        assert!(
            outcome.accepted,
            "spawn_npc skull_fiend command should be accepted"
        );
    }

    app.update();

    let npcs = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<(Entity, &NpcArchetype), With<NpcMarker>>();
        query.iter(world).map(|(e, a)| (e, *a)).collect::<Vec<_>>()
    };
    assert_eq!(
        npcs.len(),
        1,
        "expected 1 NPC spawned for skull_fiend archetype; got {}",
        npcs.len()
    );
    assert_eq!(
        npcs[0].1,
        NpcArchetype::SkullFiend,
        "expected NpcArchetype::SkullFiend, got {:?}",
        npcs[0].1
    );

    let registry = app.world().resource::<NpcRegistry>();
    assert_eq!(
        registry.live_npc_count, 1,
        "NpcRegistry should reflect 1 live NPC after skull_fiend spawn"
    );
}

/// Verifies that a truly unknown archetype string is still rejected,
/// and that TSY archetypes (daoxiang/zhinian/fuya/skull_fiend) are all accepted.
#[test]
fn spawn_npc_tsy_all_four_accepted_unknown_still_rejected() {
    let mut app = setup_executor_app();

    let tsy_archetypes = ["daoxiang", "zhinian", "fuya", "skull_fiend"];
    for (i, archetype_str) in tsy_archetypes.iter().enumerate() {
        let mut params = HashMap::new();
        params.insert("archetype".to_string(), json!(*archetype_str));
        {
            let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
            executor.enqueue_batch(batch(
                format!("cmd_spawn_tsy_{i}").as_str(),
                vec![command(CommandType::SpawnNpc, "spawn", params)],
            ));
        }
        app.update();
    }

    let spawned_count = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
        query.iter(world).count()
    };
    assert_eq!(
        spawned_count, 4,
        "expected 4 TSY NPCs spawned (one per archetype); got {} — \
             one or more TSY archetypes are still being silently dropped",
        spawned_count
    );

    // Now verify a truly unsupported archetype is still rejected.
    // Reset the world for a clean check.
    let mut unknown_params = HashMap::new();
    unknown_params.insert("archetype".to_string(), json!("immortal"));
    {
        let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
        executor.enqueue_batch(batch(
            "cmd_spawn_unknown",
            vec![command(CommandType::SpawnNpc, "spawn", unknown_params)],
        ));
    }
    app.update();

    let spawned_after_unknown = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
        query.iter(world).count()
    };
    assert_eq!(
        spawned_after_unknown, 4,
        "unknown archetype 'immortal' should be rejected and spawn no NPC; \
             total NPC count should remain 4, got {}",
        spawned_after_unknown
    );
}

#[test]
fn clamps_modify_zone_to_negative_and_positive_bounds() {
    let mut app = setup_executor_app();

    let mut params = HashMap::new();
    params.insert("spirit_qi_delta".to_string(), json!(-2.0));
    params.insert("danger_level_delta".to_string(), json!(99));

    {
        let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
        let outcome = executor.enqueue_batch(batch(
            "cmd_modify_zone",
            vec![command(CommandType::ModifyZone, "spawn", params)],
        ));
        assert!(outcome.accepted);
        assert!(!outcome.dedupe_drop);
    }

    app.update();

    let zone_registry = app.world().resource::<ZoneRegistry>();
    let spawn_zone = zone_registry
        .find_zone(
            crate::world::dimension::DimensionKind::Overworld,
            DVec3::new(8.0, 66.0, 8.0),
        )
        .expect("spawn zone should still exist");

    assert_eq!(spawn_zone.spirit_qi, -1.0);
    assert_eq!(spawn_zone.danger_level, 5);

    let mut params = HashMap::new();
    params.insert("spirit_qi_delta".to_string(), json!(3.0));

    {
        let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
        let outcome = executor.enqueue_batch(batch(
            "cmd_modify_zone_cap_upper",
            vec![command(CommandType::ModifyZone, "spawn", params)],
        ));
        assert!(outcome.accepted);
        assert!(!outcome.dedupe_drop);
    }

    app.update();

    let zone_registry = app.world().resource::<ZoneRegistry>();
    let spawn_zone = zone_registry
        .find_zone(
            crate::world::dimension::DimensionKind::Overworld,
            DVec3::new(8.0, 66.0, 8.0),
        )
        .expect("spawn zone should still exist");

    assert_eq!(spawn_zone.spirit_qi, 1.0);
}

#[test]
fn modify_zone_preserves_negative_one_without_clamping_back_to_zero() {
    let mut app = setup_executor_app();

    let mut lower_to_negative_bound_params = HashMap::new();
    lower_to_negative_bound_params.insert("spirit_qi_delta".to_string(), json!(-10.0));

    {
        let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
        let outcome = executor.enqueue_batch(batch(
            "cmd_modify_zone_reach_negative_one",
            vec![command(
                CommandType::ModifyZone,
                "spawn",
                lower_to_negative_bound_params,
            )],
        ));
        assert!(outcome.accepted);
        assert!(!outcome.dedupe_drop);
    }

    app.update();

    let zone_registry = app.world().resource::<ZoneRegistry>();
    let spawn_zone = zone_registry
        .find_zone(
            crate::world::dimension::DimensionKind::Overworld,
            DVec3::new(8.0, 66.0, 8.0),
        )
        .expect("spawn zone should still exist");

    assert_eq!(spawn_zone.spirit_qi, -1.0);

    let mut still_negative_params = HashMap::new();
    still_negative_params.insert("spirit_qi_delta".to_string(), json!(-0.25));

    {
        let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
        let outcome = executor.enqueue_batch(batch(
            "cmd_modify_zone_stay_negative",
            vec![command(
                CommandType::ModifyZone,
                "spawn",
                still_negative_params,
            )],
        ));
        assert!(outcome.accepted);
        assert!(!outcome.dedupe_drop);
    }

    app.update();

    let zone_registry = app.world().resource::<ZoneRegistry>();
    let spawn_zone = zone_registry
        .find_zone(
            crate::world::dimension::DimensionKind::Overworld,
            DVec3::new(8.0, 66.0, 8.0),
        )
        .expect("spawn zone should still exist");

    assert_eq!(spawn_zone.spirit_qi, -1.0);
}

#[test]
fn caps_commands_per_tick() {
    let mut app = setup_executor_app();

    let commands = (0..(MAX_COMMANDS_PER_TICK + 1))
        .map(|_| {
            let mut params = HashMap::new();
            params.insert("spirit_qi_delta".to_string(), json!(-0.01));
            command(CommandType::ModifyZone, "spawn", params)
        })
        .collect::<Vec<_>>();

    {
        let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
        let outcome = executor.enqueue_batch(batch("cmd_budget", commands));
        assert!(outcome.accepted);
        assert!(!outcome.dedupe_drop);
    }

    app.update();

    {
        let zone_registry = app.world().resource::<ZoneRegistry>();
        let spawn_zone = zone_registry
            .find_zone(
                crate::world::dimension::DimensionKind::Overworld,
                DVec3::new(8.0, 66.0, 8.0),
            )
            .expect("spawn zone should still exist");
        let expected = 0.9 - (MAX_COMMANDS_PER_TICK as f64 * 0.01);
        assert!((spawn_zone.spirit_qi - expected).abs() < 1e-9);
    }

    {
        let executor = app.world().resource::<CommandExecutorResource>();
        assert_eq!(executor.pending_command_count(), 1);
    }

    app.update();

    {
        let zone_registry = app.world().resource::<ZoneRegistry>();
        let spawn_zone = zone_registry
            .find_zone(
                crate::world::dimension::DimensionKind::Overworld,
                DVec3::new(8.0, 66.0, 8.0),
            )
            .expect("spawn zone should still exist");
        let expected = 0.9 - ((MAX_COMMANDS_PER_TICK + 1) as f64 * 0.01);
        assert!((spawn_zone.spirit_qi - expected).abs() < 1e-9);
    }

    {
        let executor = app.world().resource::<CommandExecutorResource>();
        assert_eq!(executor.pending_command_count(), 0);
    }
}

#[test]
fn updates_flee_threshold_only_for_generation_aware_canonical_target() {
    let mut app = setup_executor_app();
    let npc_a = app.world_mut().spawn(NpcMarker).id();
    let npc_b = app.world_mut().spawn(NpcMarker).id();
    let npc_a_id = canonical_npc_id(npc_a);
    let npc_b_id = canonical_npc_id(npc_b);

    let mut bare_index_params = HashMap::new();
    bare_index_params.insert("flee_threshold".to_string(), json!(0.2));

    {
        let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
        let outcome = executor.enqueue_batch(batch(
            "cmd_npc_behavior",
            vec![command(
                CommandType::NpcBehavior,
                format!("npc_{}", npc_a.index()).as_str(),
                bare_index_params,
            )],
        ));
        assert!(outcome.accepted);
        assert!(!outcome.dedupe_drop);
    }

    app.update();

    {
        let behavior = app.world().resource::<NpcBehaviorConfig>();
        assert_eq!(behavior.threshold_for_npc(npc_a), DEFAULT_FLEE_THRESHOLD);
        assert_eq!(
            behavior.threshold_for_npc_id(npc_a_id.as_str()),
            DEFAULT_FLEE_THRESHOLD
        );
        assert_eq!(
            behavior.threshold_for_npc_id(npc_b_id.as_str()),
            DEFAULT_FLEE_THRESHOLD
        );
    }

    let mut canonical_params = HashMap::new();
    canonical_params.insert("flee_threshold".to_string(), json!(0.2));

    {
        let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
        let outcome = executor.enqueue_batch(batch(
            "cmd_npc_behavior_canonical",
            vec![command(
                CommandType::NpcBehavior,
                npc_a_id.as_str(),
                canonical_params,
            )],
        ));
        assert!(outcome.accepted);
        assert!(!outcome.dedupe_drop);
    }

    app.update();

    let behavior = app.world().resource::<NpcBehaviorConfig>();
    assert!((behavior.threshold_for_npc(npc_a) - 0.2).abs() < 1e-6);
    assert_eq!(behavior.threshold_for_npc_id(npc_a_id.as_str()), 0.2);
    assert_eq!(
        behavior.threshold_for_npc_id(npc_b_id.as_str()),
        DEFAULT_FLEE_THRESHOLD
    );
    assert_eq!(behavior.default_flee_threshold, DEFAULT_FLEE_THRESHOLD);
}

#[test]
fn rejects_unknown_targets() {
    let mut app = setup_executor_app();

    let mut commands = Vec::new();

    let mut bad_spawn_npc_params = HashMap::new();
    bad_spawn_npc_params.insert("archetype".to_string(), json!("zombie"));
    commands.push(command(
        CommandType::SpawnNpc,
        "unknown_zone",
        bad_spawn_npc_params,
    ));

    let mut bad_zone_params = HashMap::new();
    bad_zone_params.insert("spirit_qi_delta".to_string(), json!(0.1));
    commands.push(command(
        CommandType::ModifyZone,
        "unknown_zone",
        bad_zone_params,
    ));

    let mut bad_npc_params = HashMap::new();
    bad_npc_params.insert("flee_threshold".to_string(), json!(0.1));
    commands.push(command(
        CommandType::NpcBehavior,
        "npc_999999v1",
        bad_npc_params,
    ));

    commands.push(command(
        CommandType::DespawnNpc,
        "npc_999999v1",
        HashMap::new(),
    ));

    let mut bad_event_params = HashMap::new();
    bad_event_params.insert("event".to_string(), json!(EVENT_THUNDER_TRIBULATION));
    bad_event_params.insert("intensity".to_string(), json!(0.8));
    bad_event_params.insert("duration_ticks".to_string(), json!(120));
    commands.push(command(
        CommandType::SpawnEvent,
        "missing_zone",
        bad_event_params,
    ));

    let mut unsupported_event_params = HashMap::new();
    unsupported_event_params.insert("event".to_string(), json!("unknown_calamity"));
    unsupported_event_params.insert("intensity".to_string(), json!(0.3));
    commands.push(command(
        CommandType::SpawnEvent,
        "spawn",
        unsupported_event_params,
    ));

    let mut bad_params = HashMap::new();
    bad_params.insert("spirit_qi_delta".to_string(), json!("bad-number"));
    commands.push(command(CommandType::ModifyZone, "spawn", bad_params));

    {
        let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
        let outcome = executor.enqueue_batch(batch("cmd_reject_unknown", commands));
        assert!(outcome.accepted);
        assert!(!outcome.dedupe_drop);
    }

    app.update();

    let zone_registry = app.world().resource::<ZoneRegistry>();
    let spawn_zone = zone_registry
        .find_zone(
            crate::world::dimension::DimensionKind::Overworld,
            DVec3::new(8.0, 66.0, 8.0),
        )
        .expect("spawn zone should still exist");

    assert_eq!(spawn_zone.spirit_qi, 0.9);
    assert_eq!(spawn_zone.danger_level, 0);
    assert!(spawn_zone.active_events.is_empty());

    let behavior = app.world().resource::<NpcBehaviorConfig>();
    assert_eq!(behavior.default_flee_threshold, DEFAULT_FLEE_THRESHOLD);
    assert_eq!(
        behavior.threshold_for_npc_id("npc_999999v1"),
        DEFAULT_FLEE_THRESHOLD
    );

    let npc_count = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
        query.iter(world).count()
    };
    assert_eq!(npc_count, 0);
}

// ─────────── plan-offscreen-war-v1 P6：路径 B（headless 玩家参与）─────────────
//
// 与 brigadier 路径 A（cmd/gameplay/war.rs 的 4 个 test）对拍：
// 同一 zone/role/group 输入下，路径 B 必须发出与路径 A 同形的 WarParticipateIntent，
// 且错误分支（缺 player_id / 非法 role）必须返回与生产一致的 label 且不发 intent。

/// 捕获 `execute_war_participate_headless` 返回的 label，供错误分支断言。
#[derive(Resource, Default)]
struct WarLabelCapture(&'static str);

#[derive(Resource)]
struct HeadlessParticipateCommand(Command);

/// 一次性 wrapper system：用 Bevy 提供的真实 EventWriter 跑生产 fn，
/// 把返回 label 落进 WarLabelCapture，让错误分支 label 可观测（不绑实现细节）。
fn run_headless_participate(app: &mut App, command: Command) -> &'static str {
    app.insert_resource(WarLabelCapture::default());
    app.insert_resource(HeadlessParticipateCommand(command));

    fn capture_system(
        cmd: Res<HeadlessParticipateCommand>,
        mut label: ResMut<WarLabelCapture>,
        mut war_intents: EventWriter<WarParticipateIntent>,
    ) {
        label.0 = super::execute_war_participate_headless(&cmd.0, &mut war_intents);
    }

    let mut schedule = bevy_ecs::schedule::Schedule::default();
    schedule.add_systems(capture_system);
    schedule.run(app.world_mut());

    app.world().resource::<WarLabelCapture>().0
}

/// 通过完整 `execute_agent_commands` 管线跑路径 B（war_participate=true），
/// drain 出 WarParticipateIntent 断言形状——这是与路径 A "同效" 的对拍口。
fn drain_war_intents_after_pipeline(app: &App) -> Vec<WarParticipateIntent> {
    app.world()
        .resource::<Events<WarParticipateIntent>>()
        .iter_current_update_events()
        .cloned()
        .collect()
}

fn war_participate_params(role: &str, group: Option<u64>) -> HashMap<String, Value> {
    let mut params = HashMap::new();
    params.insert("war_participate".to_string(), json!(true));
    params.insert("player_id".to_string(), json!("offline:e2e_merc"));
    params.insert("role".to_string(), json!(role));
    if let Some(g) = group {
        params.insert("group".to_string(), json!(g));
    }
    params
}

#[test]
fn war_participate_headless_emits_enlist_intent_matching_brigadier_shape() {
    // happy path：war_participate=true + role=enlist + group=2 →
    // 与 brigadier `/faction join 2`（路径 A）同形的 WarParticipateIntent。
    let mut app = setup_executor_app();

    {
        let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
        let outcome = executor.enqueue_batch(batch(
            "cmd_war_participate_enlist",
            vec![command(
                CommandType::FactionEvent,
                "remnant_ash_valley",
                war_participate_params("enlist", Some(2)),
            )],
        ));
        assert!(outcome.accepted);
        assert!(!outcome.dedupe_drop);
    }

    app.update();

    let intents = drain_war_intents_after_pipeline(&app);
    assert_eq!(
            intents.len(),
            1,
            "期望路径 B war_participate 发出 1 条 WarParticipateIntent 因 happy path 应汇聚到同一队列，实际 {}",
            intents.len()
        );
    let intent = &intents[0];
    assert_eq!(
        intent.zone, "remnant_ash_valley",
        "期望 zone=target(remnant_ash_valley) 因路径 B 约定 target==zone，实际 {}",
        intent.zone
    );
    assert_eq!(
        intent.player_id, "offline:e2e_merc",
        "期望 player_id 透传自 params 因路径 B 必须保留参与者身份，实际 {}",
        intent.player_id
    );
    assert_eq!(
        intent.role,
        WarRole::Enlist,
        "期望 role=Enlist 因 \"enlist\" 字符串映射，实际 {:?}",
        intent.role
    );
    assert_eq!(
        intent.allied_group,
        Some(EmergentGroupId(2)),
        "期望 allied_group=Some(EmergentGroupId(2)) 因 group=2 应解析为裸匿名群体 id，实际 {:?}",
        intent.allied_group
    );
}

#[test]
fn war_participate_headless_role_string_maps_all_four_variants() {
    // role 字符串四态全覆盖：enlist/mercenary/intercept/spectate 各自映射正确，
    // 且默认（缺 role）退化为 Spectate。每条都通过管线 drain 断言。
    let cases = [
        (
            "enlist",
            WarRole::Enlist,
            Some(3u64),
            Some(EmergentGroupId(3)),
        ),
        (
            "mercenary",
            WarRole::Mercenary,
            Some(7),
            Some(EmergentGroupId(7)),
        ),
        ("intercept", WarRole::Intercept, None, None),
        ("spectate", WarRole::Spectate, None, None),
    ];

    for (role_str, expected_role, group, expected_group) in cases {
        let mut app = setup_executor_app();
        {
            let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
            let outcome = executor.enqueue_batch(batch(
                &format!("cmd_war_role_{role_str}"),
                vec![command(
                    CommandType::FactionEvent,
                    "remnant_ash_valley",
                    war_participate_params(role_str, group),
                )],
            ));
            assert!(outcome.accepted);
        }
        app.update();

        let intents = drain_war_intents_after_pipeline(&app);
        assert_eq!(
            intents.len(),
            1,
            "期望 role={role_str} 发出 1 条 intent 因合法 role 应入队，实际 {}",
            intents.len()
        );
        assert_eq!(
            intents[0].role, expected_role,
            "期望 role 字符串 \"{role_str}\" 映射为 {expected_role:?} 因四态映射表，实际 {:?}",
            intents[0].role
        );
        assert_eq!(
                intents[0].allied_group, expected_group,
                "期望 role={role_str} 的 allied_group={expected_group:?} 因仅 Enlist/Mercenary 带 group，实际 {:?}",
                intents[0].allied_group
            );
    }
}

#[test]
fn war_participate_headless_missing_role_defaults_to_spectate() {
    // 缺 role 字段 → 默认 Spectate（不报错、发 intent）。
    let mut app = setup_executor_app();
    let mut params = HashMap::new();
    params.insert("war_participate".to_string(), json!(true));
    params.insert("player_id".to_string(), json!("offline:lurker"));

    let label = run_headless_participate(
        &mut app,
        command(CommandType::FactionEvent, "remnant_ash_valley", params),
    );
    assert_eq!(
        label, "ok",
        "期望缺 role 返回 ok 因 role 默认为 spectate，实际 {label}"
    );

    let intents = drain_war_intents_after_pipeline(&app);
    assert_eq!(
        intents.len(),
        1,
        "期望缺 role 仍发 1 条 intent，实际 {}",
        intents.len()
    );
    assert_eq!(
        intents[0].role,
        WarRole::Spectate,
        "期望缺 role 默认 Spectate 因 unwrap_or(\"spectate\")，实际 {:?}",
        intents[0].role
    );
}

#[test]
fn war_participate_headless_missing_player_id_rejects_and_emits_nothing() {
    // 错误分支①：缺 player_id → label=rejected_missing_player_id，不发 intent。
    let mut app = setup_executor_app();
    let mut params = HashMap::new();
    params.insert("war_participate".to_string(), json!(true));
    params.insert("role".to_string(), json!("enlist"));
    params.insert("group".to_string(), json!(1));

    let label = run_headless_participate(
        &mut app,
        command(CommandType::FactionEvent, "remnant_ash_valley", params),
    );
    assert_eq!(
        label, "rejected_missing_player_id",
        "期望缺 player_id 返回 rejected_missing_player_id 因路径 B 必须有参与者身份，实际 {label}"
    );

    let intents = drain_war_intents_after_pipeline(&app);
    assert!(
        intents.is_empty(),
        "期望缺 player_id 不发任何 intent 因前置校验失败应 early return，实际发出 {}",
        intents.len()
    );
}

#[test]
fn war_participate_headless_invalid_role_rejects_and_emits_nothing() {
    // 错误分支②：非法 role 字符串 → label=rejected_invalid_war_role，不发 intent。
    let mut app = setup_executor_app();
    let mut params = HashMap::new();
    params.insert("war_participate".to_string(), json!(true));
    params.insert("player_id".to_string(), json!("offline:e2e_merc"));
    params.insert("role".to_string(), json!("declare_war_on_sect"));

    let label = run_headless_participate(
        &mut app,
        command(CommandType::FactionEvent, "remnant_ash_valley", params),
    );
    assert_eq!(
        label, "rejected_invalid_war_role",
        "期望非法 role 返回 rejected_invalid_war_role 因只接受四态枚举，实际 {label}"
    );

    let intents = drain_war_intents_after_pipeline(&app);
    assert!(
        intents.is_empty(),
        "期望非法 role 不发任何 intent 因 role 解析失败应 early return，实际发出 {}",
        intents.len()
    );
}

#[test]
fn war_participate_headless_group_overflow_clamps_to_none() {
    // 边界：group 超出 u16 范围 → u16::try_from 失败 → allied_group=None（不 panic）。
    let mut app = setup_executor_app();

    {
        let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
        let outcome = executor.enqueue_batch(batch(
            "cmd_war_group_overflow",
            vec![command(
                CommandType::FactionEvent,
                "remnant_ash_valley",
                war_participate_params("mercenary", Some(70_000)),
            )],
        ));
        assert!(outcome.accepted);
    }
    app.update();

    let intents = drain_war_intents_after_pipeline(&app);
    assert_eq!(
        intents.len(),
        1,
        "期望溢出 group 仍发 intent，实际 {}",
        intents.len()
    );
    assert_eq!(
        intents[0].allied_group, None,
        "期望 group=70000(超 u16) 解析为 None 因 u16::try_from 失败应 drop 而非 panic，实际 {:?}",
        intents[0].allied_group
    );
}

#[test]
fn faction_event_without_war_participate_flag_emits_no_war_intent() {
    // 状态转换守卫：war_participate 缺失/非 true 时不得误入路径 B，
    // 仍走原 faction 逻辑且不发 WarParticipateIntent。
    let mut app = setup_executor_app();
    let mut params = HashMap::new();
    params.insert("kind".to_string(), json!("enqueue_mission"));
    params.insert("faction_id".to_string(), json!("neutral"));
    params.insert("mission_id".to_string(), json!("mission:hold_spawn_gate"));

    {
        let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
        let outcome = executor.enqueue_batch(batch(
            "cmd_faction_event_no_war_flag",
            vec![command(CommandType::FactionEvent, "neutral", params)],
        ));
        assert!(outcome.accepted);
    }
    app.update();

    let intents = drain_war_intents_after_pipeline(&app);
    assert!(
            intents.is_empty(),
            "期望无 war_participate flag 的 faction_event 不发 WarParticipateIntent 因不应误入路径 B，实际发出 {}",
            intents.len()
        );
}

#[test]
fn duplicate_batch_id_is_dropped_before_queueing() {
    let mut app = setup_executor_app();

    let mut params = HashMap::new();
    params.insert("spirit_qi_delta".to_string(), json!(-0.1));

    let duplicate_id = "cmd_dedupe_me";

    {
        let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
        let first = executor.enqueue_batch(batch(
            duplicate_id,
            vec![command(CommandType::ModifyZone, "spawn", params.clone())],
        ));
        let second = executor.enqueue_batch(batch(
            duplicate_id,
            vec![command(CommandType::ModifyZone, "spawn", params)],
        ));

        assert!(first.accepted);
        assert!(!first.dedupe_drop);
        assert!(!second.accepted);
        assert!(second.dedupe_drop);
        assert_eq!(executor.pending_command_count(), 1);
        assert_eq!(executor.dedupe_cache_len(), 1);
    }

    app.update();

    let zone_registry = app.world().resource::<ZoneRegistry>();
    let spawn_zone = zone_registry
        .find_zone(
            crate::world::dimension::DimensionKind::Overworld,
            DVec3::new(8.0, 66.0, 8.0),
        )
        .expect("spawn zone should still exist");
    assert!((spawn_zone.spirit_qi - 0.8).abs() < 1e-9);
}

// ── plan-era-state-v1 B1 — 生产路径集成测试 ─────────────────────────────

fn drain_era_decree_intents(app: &App) -> Vec<EraDecreeIntent> {
    app.world()
        .resource::<Events<EraDecreeIntent>>()
        .iter_current_update_events()
        .cloned()
        .collect()
}

/// 生产路径：modify_zone{target="全局", era_name="calamity"} 应 emit EraDecreeIntent。
///
/// 这是 B1 断路修复的核心集成测试——任何回归都会让此测试失败：
/// 若 execute_modify_zone 不处理 "全局" target 并 emit EraDecreeIntent，
/// drain 结果为空，断言失败。
#[test]
fn modify_zone_global_era_name_emits_era_decree_intent_via_production_path() {
    let mut app = setup_executor_app();
    // 注入 WorldEraState（era_decree_system 消费者依赖此 Resource）
    app.insert_resource(crate::world::era::WorldEraState::default());
    app.add_event::<crate::world::era::EraChangedEvent>();
    app.add_systems(
        Update,
        crate::world::era::era_decree_system.after(execute_agent_commands),
    );

    let mut params = HashMap::new();
    params.insert("era_name".to_string(), json!("calamity"));
    params.insert("global_effect".to_string(), json!("天地肃杀，渡劫更难"));
    params.insert("spirit_qi_delta".to_string(), json!(-0.05));
    params.insert("danger_level_delta".to_string(), json!(2));

    {
        let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
        let outcome = executor.enqueue_batch(batch(
            "cmd_era_decree_calamity_e2e",
            vec![command(CommandType::ModifyZone, "全局", params)],
        ));
        assert!(
            outcome.accepted,
            "era_decree modify_zone batch 应被接受入队列，实际 accepted={}",
            outcome.accepted
        );
    }

    app.update();

    // 验证 EraDecreeIntent 通过生产路径发出
    let era_intents = drain_era_decree_intents(&app);
    assert_eq!(
        era_intents.len(),
        1,
        "期望恰好一个 EraDecreeIntent 通过生产路径（execute_modify_zone 全局分支）发出；\
             实际 {}（=0 说明 modify_zone 未处理 全局 target，B1 断路未修）",
        era_intents.len()
    );

    let intent = &era_intents[0];
    assert_eq!(
        intent.era_name, "calamity",
        "EraDecreeIntent.era_name 应等于 agent 发出的值；期望 calamity 实际 {}",
        intent.era_name
    );
    assert!(
        (intent.spirit_qi_delta - (-0.05)).abs() < 1e-9,
        "EraDecreeIntent.spirit_qi_delta 应等于 params 里的值；期望 -0.05 实际 {}",
        intent.spirit_qi_delta
    );
    assert_eq!(
        intent.danger_level_delta, 2,
        "EraDecreeIntent.danger_level_delta 应等于 params 里的值；期望 2 实际 {}",
        intent.danger_level_delta
    );

    // 验证 WorldEraState 被 era_decree_system 真实更新（生产链路全通）
    let world_era = app.world().resource::<crate::world::era::WorldEraState>();
    assert_eq!(
        world_era.era,
        crate::world::era::EraType::Calamity,
        "WorldEraState.era 应在 EraDecreeIntent → era_decree_system 消费后更新为 Calamity；\
             实际 {:?}（=Unknown 说明 era_decree_system 未被驱动或 EraDecreeIntent 未发出）",
        world_era.era
    );
}

/// 生产路径：modify_zone{target="全局"} 无 era_name 应被拒绝，不 panic，不 emit intent。
#[test]
fn modify_zone_global_without_era_name_is_rejected_cleanly() {
    let mut app = setup_executor_app();

    let mut params = HashMap::new();
    params.insert("spirit_qi_delta".to_string(), json!(-0.02));
    // 故意不加 era_name

    {
        let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
        executor.enqueue_batch(batch(
            "cmd_global_no_era_name",
            vec![command(CommandType::ModifyZone, "全局", params)],
        ));
    }

    app.update(); // 不应 panic

    let era_intents = drain_era_decree_intents(&app);
    assert!(
        era_intents.is_empty(),
        "无 era_name 的全局 modify_zone 不应发出 EraDecreeIntent；实际发出 {}",
        era_intents.len()
    );
}

/// 生产路径：modify_zone{target="mutation"} (Change 时代别名) 应 emit EraDecreeIntent，
/// era_decree_system 应将 mutation 解析为 EraType::Change。
#[test]
fn modify_zone_global_era_name_mutation_maps_to_change_via_production_path() {
    let mut app = setup_executor_app();
    app.insert_resource(crate::world::era::WorldEraState::default());
    app.add_event::<crate::world::era::EraChangedEvent>();
    app.add_systems(
        Update,
        crate::world::era::era_decree_system.after(execute_agent_commands),
    );

    let mut params = HashMap::new();
    params.insert("era_name".to_string(), json!("mutation"));
    params.insert("spirit_qi_delta".to_string(), json!(0.0));

    {
        let mut executor = app.world_mut().resource_mut::<CommandExecutorResource>();
        executor.enqueue_batch(batch(
            "cmd_era_decree_mutation_e2e",
            vec![command(CommandType::ModifyZone, "全局", params)],
        ));
    }

    app.update();

    let era_intents = drain_era_decree_intents(&app);
    assert_eq!(
        era_intents.len(),
        1,
        "期望 mutation 时代 modify_zone 全局 发出一个 EraDecreeIntent；实际 {}",
        era_intents.len()
    );
    assert_eq!(
        era_intents[0].era_name, "mutation",
        "EraDecreeIntent.era_name 应保留 agent 发出的原始字符串 mutation；实际 {}",
        era_intents[0].era_name
    );

    // era_decree_system 消费后 WorldEraState 应为 Change（mutation → Change 映射）
    let world_era = app.world().resource::<crate::world::era::WorldEraState>();
    assert_eq!(
        world_era.era,
        crate::world::era::EraType::Change,
        "WorldEraState.era 应为 Change（mutation 别名映射）；实际 {:?}",
        world_era.era
    );
}
