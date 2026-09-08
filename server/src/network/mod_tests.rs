use super::*;
use crate::cultivation::components::{ColorKind, Realm};
use crate::cultivation::insight::{InsightAlignment, InsightEffect};
use crate::persistence::{load_agent_decisions, load_agent_eras};
use crate::schema::cultivation::{InsightChoiceV1, InsightOfferV1};
use crate::schema::server_data::{HeartDemonOfferChoiceV1, HeartDemonOfferV1};
use crossbeam_channel::{bounded, unbounded, Receiver};
use std::time::Duration;
use valence::testing::create_mock_client;

fn assert_approx_eq(left: f64, right: f64) {
    assert!(
        (left - right).abs() < 1e-9,
        "expected {left} to be approximately equal to {right}"
    );
}

#[test]
fn local_neg_pressure_from_sample_reports_hotspot_as_negative_pressure() {
    assert_eq!(local_neg_pressure_from_sample(0.8, 0.0), Some(-0.8));
    assert_eq!(local_neg_pressure_from_sample(0.8, 30.1), None);
    assert_eq!(local_neg_pressure_from_sample(0.0, 0.0), None);
}

#[test]
fn resolve_redis_url_prefers_non_empty_env_value() {
    let value = resolve_redis_url(Some("redis://10.0.0.8:6380".to_string()));
    assert_eq!(value, "redis://10.0.0.8:6380");
}

#[test]
fn resolve_redis_url_falls_back_to_default_when_missing_or_blank() {
    assert_eq!(resolve_redis_url(None), DEFAULT_REDIS_URL.to_string());
    assert_eq!(
        resolve_redis_url(Some("   \t\n ".to_string())),
        DEFAULT_REDIS_URL.to_string()
    );
}

#[test]
fn redact_redis_url_for_log_strips_credentials_and_paths() {
    assert_eq!(
        redact_redis_url_for_log("redis://:password@cache.internal:6380/4"),
        "cache.internal:6380"
    );
    assert_eq!(
        redact_redis_url_for_log("rediss://user:password@[::1]:6390/0?tls=true"),
        "[::1]:6390"
    );
}

#[test]
fn redact_redis_url_for_log_falls_back_for_invalid_values() {
    assert_eq!(
        redact_redis_url_for_log("not-a-redis-url"),
        "[redacted redis endpoint]"
    );
}

fn agent_insight_choices() -> Vec<InsightChoiceV1> {
    vec![
        InsightChoiceV1 {
            category: "qi".to_string(),
            effect_kind: "qi_regen_factor".to_string(),
            magnitude: 0.1,
            flavor_text: "agent converge".to_string(),
            narrator_voice: None,
            alignment: Some("converge".to_string()),
            cost_kind: Some("opposite_color_penalty".to_string()),
            cost_magnitude: Some(0.05),
            cost_flavor: Some("agent cost".to_string()),
        },
        InsightChoiceV1 {
            category: "composure".to_string(),
            effect_kind: "composure_recover".to_string(),
            magnitude: 0.1,
            flavor_text: "agent neutral".to_string(),
            narrator_voice: None,
            alignment: Some("neutral".to_string()),
            cost_kind: Some("shock_sensitivity".to_string()),
            cost_magnitude: Some(0.03),
            cost_flavor: Some("agent cost".to_string()),
        },
        InsightChoiceV1 {
            category: "color".to_string(),
            effect_kind: "color_cap_add".to_string(),
            magnitude: 0.04,
            flavor_text: "agent diverge".to_string(),
            narrator_voice: None,
            alignment: Some("diverge".to_string()),
            cost_kind: Some("main_color_penalty".to_string()),
            cost_magnitude: Some(0.1),
            cost_flavor: Some("agent cost".to_string()),
        },
    ]
}

#[test]
fn bridge_drain_is_non_blocking() {
    let (tx_to_agent, _rx_to_agent) = unbounded::<GameEvent>();
    let (_tx_from_agent, rx_from_agent) = unbounded::<AgentCommand>();
    let bridge = NetworkBridgeResource::new(tx_to_agent, rx_from_agent);

    let (done_tx, done_rx) = bounded::<usize>(1);

    std::thread::spawn(move || {
        let drained = drain_bridge_commands(&bridge, || {});
        let _ = done_tx.send(drained);
    });

    let drained = done_rx
        .recv_timeout(Duration::from_millis(100))
        .expect("drain should return immediately when channel is empty");

    assert_eq!(drained, 0);
}

#[test]
fn parse_heart_demon_trigger_id_requires_current_format() {
    assert_eq!(
        parse_heart_demon_trigger_id("heart_demon:42:1200"),
        Some((42, 1200))
    );
    assert_eq!(
        parse_heart_demon_trigger_id("heart_demon:demon:42:1200"),
        None
    );
    assert_eq!(parse_heart_demon_trigger_id("insight:42:1200"), None);
}

#[test]
fn process_redis_inbound_caches_heart_demon_offer_for_matching_client() {
    let (tx_outbound, _rx_outbound) = unbounded();
    let (tx_inbound, rx_inbound) = unbounded();
    let mut app = App::new();
    app.insert_resource(RedisBridgeResource {
        tx_outbound,
        rx_inbound,
    });
    app.insert_resource(CommandExecutorResource::default());
    app.insert_resource(NarrationDedupeResource::default());
    app.add_event::<crate::cultivation::insight::InsightOffer>();
    // plan-agent-ui-data-v1 P0 — process_redis_inbound 需要 AgentUiCmdEvent。
    app.add_event::<agent_ui::AgentUiCmdEvent>();
    app.add_systems(Update, process_redis_inbound);

    let (client_bundle, _helper) = create_mock_client("Azure");
    let entity = app.world_mut().spawn(client_bundle).id();
    let offer = HeartDemonOfferV1 {
        offer_id: format!("heart_demon:{}:1000", entity.index()),
        trigger_id: format!("heart_demon:{}:1000", entity.index()),
        trigger_label: "心魔照见".to_string(),
        realm_label: "渡虚劫 · 心魔".to_string(),
        composure: 0.6,
        quota_remaining: 1,
        quota_total: 1,
        expires_at_ms: 123,
        choices: vec![HeartDemonOfferChoiceV1 {
            choice_id: "heart_demon_choice_0".to_string(),
            category: "Composure".to_string(),
            title: "守本心".to_string(),
            effect_summary: "稳住心神，回复少量当前真元".to_string(),
            flavor: "旧事浮起，仍可不逐影。".to_string(),
            style_hint: "稳妥".to_string(),
        }],
    };

    tx_inbound
        .send(RedisInbound::HeartDemonOffer(offer.clone()))
        .expect("heart demon offer should enqueue");
    app.update();

    let cached = app
        .world()
        .get::<crate::cultivation::tribulation::PendingHeartDemonOffer>(entity)
        .expect("matching heart demon offer should be cached on client entity");
    assert_eq!(cached.trigger_id, offer.trigger_id);
    assert_eq!(cached.payload.choices[0].choice_id, "heart_demon_choice_0");
}

#[test]
fn process_redis_inbound_keeps_contextual_insight_offer_when_agent_overwrites_pending() {
    let (tx_outbound, _rx_outbound) = unbounded();
    let (tx_inbound, rx_inbound) = unbounded();
    let mut app = App::new();
    app.insert_resource(RedisBridgeResource {
        tx_outbound,
        rx_inbound,
    });
    app.insert_resource(CommandExecutorResource::default());
    app.insert_resource(NarrationDedupeResource::default());
    app.add_event::<crate::cultivation::insight::InsightOffer>();
    app.add_event::<agent_ui::AgentUiCmdEvent>();
    app.add_systems(Update, process_redis_inbound);

    let (client_bundle, _helper) = create_mock_client("Azure");
    let entity = app.world_mut().spawn(client_bundle).id();
    let trigger_id = "first_breakthrough_to_Induce";
    let qi_color = QiColor {
        main: ColorKind::Sharp,
        ..QiColor::default()
    };
    let mut practice_log = PracticeLog::default();
    practice_log.add(ColorKind::Sharp, 10.0);
    app.world_mut().entity_mut(entity).insert((
        Cultivation {
            realm: Realm::Solidify,
            ..Cultivation::default()
        },
        qi_color,
        practice_log,
        InsightQuota::default(),
        crate::cultivation::insight_flow::PendingInsightOffer {
            trigger_id: trigger_id.to_string(),
            choices: crate::cultivation::insight_fallback::fallback_for(trigger_id),
        },
    ));

    tx_inbound
        .send(RedisInbound::InsightOffer(InsightOfferV1 {
            offer_id: "offer-agent-1".to_string(),
            trigger_id: trigger_id.to_string(),
            character_id: "Azure".to_string(),
            choices: agent_insight_choices(),
        }))
        .expect("agent insight offer should enqueue");
    app.update();

    let pending = app
        .world()
        .get::<crate::cultivation::insight_flow::PendingInsightOffer>(entity)
        .expect("agent insight offer should overwrite PendingInsightOffer");
    assert_eq!(pending.trigger_id, trigger_id);
    assert!(
        pending
            .choices
            .iter()
            .any(|choice| choice.flavor.contains("锋锐")),
        "agent-fed PendingInsightOffer 不应退化成默认 Mellow 文案，实际 choices={:?}",
        pending.choices
    );
    assert!(
        pending.choices.iter().any(|choice| {
            choice.alignment == InsightAlignment::Diverge
                && matches!(
                    choice.effect,
                    InsightEffect::ColorCapAdd {
                        color: ColorKind::Heavy,
                        ..
                    }
                )
        }),
        "Diverge 槽应基于当前 Sharp 上下文选出最陌生色 Heavy，实际 choices={:?}",
        pending.choices
    );
}

mod world_state_tests {
    use super::*;
    use crate::npc::faction::{
        FactionId, FactionMembership, FactionRank, FactionStore, Lineage, MissionId,
        MissionQueue, Reputation,
    };
    use crate::npc::lifecycle::{npc_runtime_bundle, NpcArchetype, NpcLifespan};
    use crate::player::state::PlayerState;
    use crate::schema::social::{RelationshipKindV1, RenownTagV1};
    use crate::social::components::{Anonymity, Relationship, Relationships, Renown};

    fn setup_publish_app(with_zone_registry: bool) -> (App, Receiver<RedisOutbound>) {
        let (tx_outbound, rx_outbound) = unbounded();
        let (_tx_inbound, rx_inbound) = unbounded();
        let mut app = App::new();
        app.insert_resource(
            crate::cultivation::known_techniques::TechniqueRegistry::load_for_tests(),
        );

        crate::npc::faction::register(&mut app);

        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.insert_resource(WorldStateTimer {
            ticks: WORLD_STATE_PUBLISH_INTERVAL_TICKS - 1,
        });

        if with_zone_registry {
            app.insert_resource(ZoneRegistry::fallback());
        }

        app.add_systems(Update, publish_world_state_to_redis);

        (app, rx_outbound)
    }

    fn spawn_test_client(app: &mut App, username: &str, position: [f64; 3]) -> Entity {
        let (mut client_bundle, _helper) = create_mock_client(username);
        client_bundle.player.position = Position::new(position);

        app.world_mut().spawn(client_bundle).id()
    }

    fn publish_once(app: &mut App, rx_outbound: &Receiver<RedisOutbound>) -> WorldStateV1 {
        app.update();

        match rx_outbound
            .try_recv()
            .expect("world state publish should enqueue a Redis outbound message")
        {
            RedisOutbound::WorldState(state) => state,
            other => panic!("expected a world-state publish, got {other:?}"),
        }
    }

    fn dormant_snapshot(
        char_id: &str,
        pos: [f64; 3],
    ) -> crate::npc::dormant::NpcDormantSnapshot {
        let cultivation = Cultivation::default();
        crate::npc::dormant::NpcDormantSnapshot {
            char_id: char_id.to_string(),
            archetype: NpcArchetype::Rogue,
            dimension: DimensionKind::Overworld,
            zone_name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            position: pos,
            schedule_seed: None,
            cultivation: cultivation.clone(),
            meridian_system: MeridianSystem::default(),
            meridian_severed:
                crate::cultivation::meridian::severed::MeridianSeveredPermanent::default(),
            contamination: crate::cultivation::components::Contamination::default(),
            lifespan: NpcLifespan::new(10.0, 100.0),
            shared_lifespan: crate::cultivation::lifespan::LifespanComponent::for_realm(
                cultivation.realm,
            ),
            lifespan_extension_ledger:
                crate::cultivation::lifespan::LifespanExtensionLedger::default(),
            death_registry: crate::cultivation::lifespan::DeathRegistry::new(char_id),
            life_record: LifeRecord::new(char_id),
            memory: None,
            player_reputation: None,
            faction: None,
            emergent_group: None,
            patrol: None,
            loot_table: None,
            guardian_relic: None,
            mimic_spider: None,
            tsy_hostile: None,
            tsy_sentinel: None,
            intent: crate::npc::dormant::DormantBehaviorIntent::Wander { drift_radius: 12.0 },
            dormant_since_tick: 0,
            last_dormant_tick_processed: 0,
            initial_qi: cultivation.qi_current,
            qi_ledger_net: 0.0,
            combat_dead_pending_release: false,
            pending_combat_winner: None,
        }
    }

    #[test]
    fn uses_real_player_names_and_positions() {
        let (mut app, rx_outbound) = setup_publish_app(true);
        spawn_test_client(&mut app, "Alice", [8.0, 66.0, 8.0]);
        spawn_test_client(&mut app, "Bob", [12.5, 66.0, 9.25]);

        let state = publish_once(&mut app, &rx_outbound);
        let alice = state
            .players
            .iter()
            .find(|player| player.name == "Alice")
            .expect("Alice should be present in the world snapshot");
        let bob = state
            .players
            .iter()
            .find(|player| player.name == "Bob")
            .expect("Bob should be present in the world snapshot");

        assert_eq!(alice.pos, [8.0, 66.0, 8.0]);
        assert_eq!(bob.pos, [12.5, 66.0, 9.25]);
        assert!(
            state
                .players
                .iter()
                .all(|player| !player.name.starts_with("Player")),
            "placeholder Player{{i}} names should not be emitted once real usernames exist"
        );
    }

    #[test]
    fn emits_spawn_zone_without_players() {
        let (mut app, rx_outbound) = setup_publish_app(true);

        let state = publish_once(&mut app, &rx_outbound);
        let spawn_zone = state
            .zones
            .iter()
            .find(|zone| zone.name == DEFAULT_SPAWN_ZONE_NAME)
            .expect("spawn fallback zone should still be emitted with zero players");

        assert!(state.players.is_empty());
        assert_eq!(spawn_zone.player_count, 0);
        assert_eq!(spawn_zone.status, ZoneStatusV1::Normal);
        assert!(
            state.recent_events.is_empty(),
            "recent_events should be an explicit empty array when no event buffer exists"
        );
    }

    #[test]
    fn publishes_dormant_npcs_in_world_state_and_redis_hash() {
        let (mut app, rx_outbound) = setup_publish_app(true);
        let mut dormant_store = NpcDormantStore::default();
        dormant_store.insert(dormant_snapshot("npc_dormant_a", [32.0, 66.0, 32.0]));
        app.insert_resource(dormant_store);

        let state = publish_once(&mut app, &rx_outbound);
        let dormant = state
            .npcs
            .iter()
            .find(|npc| npc.id == "npc_dormant_a")
            .expect("dormant NPC should be included in world-state NPC list");
        assert_eq!(dormant.kind, "dormant:rogue");
        assert_eq!(dormant.pos, [32.0, 66.0, 32.0]);
        assert_eq!(
            dormant.blackboard.get("dormant"),
            Some(&serde_json::json!(true))
        );
        assert_eq!(
            dormant.digest.as_ref().and_then(|digest| digest.position),
            Some([32.0, 66.0, 32.0])
        );

        let outbound = rx_outbound
            .try_recv()
            .expect("dormant Redis HASH sync should follow world-state publish");
        let RedisOutbound::NpcDormantHash { entries, .. } = outbound else {
            panic!("expected dormant Redis HASH outbound, got {outbound:?}");
        };
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, "npc_dormant_a");
        let payload: serde_json::Value =
            serde_json::from_str(&entries[0].1).expect("dormant payload should be JSON");
        assert_eq!(payload["char_id"], "npc_dormant_a");
    }

    /// P1 contract: the dormant Redis hash sync is dirty-gated. The first
    /// publish after a change emits an `NpcDormantHash`; a later publish
    /// cycle with no intervening dormant change emits the `WorldState` but
    /// NO `NpcDormantHash`. This is the whole point of P1 — stop the every
    /// 200-tick full serde + hash replace when nothing changed.
    #[test]
    fn dormant_publish_skipped_when_clean() {
        let (mut app, rx_outbound) = setup_publish_app(true);
        let mut dormant_store = NpcDormantStore::default();
        dormant_store.insert(dormant_snapshot("npc_dormant_a", [32.0, 66.0, 32.0]));
        app.insert_resource(dormant_store);

        // Force every cycle onto a publishing tick so the gate, not the
        // 200-tick interval, decides whether dormant is written. Draining
        // the emitted messages is the caller's job via `rx_outbound`.
        fn force_publish_cycle(app: &mut App) {
            app.world_mut().resource_mut::<WorldStateTimer>().ticks =
                WORLD_STATE_PUBLISH_INTERVAL_TICKS - 1;
            // `app.update()` runs the `Main` schedule (where the publish
            // system lives) exactly like every other test in this crate;
            // `run_schedule(bevy_app::Main)` referenced a crate that is not
            // a dependency and never compiled under `--all-targets`.
            app.update();
        }

        // Cycle 1: store is dirty from the insert -> WorldState + dormant hash.
        force_publish_cycle(&mut app);
        let mut cycle1 = Vec::new();
        while let Ok(msg) = rx_outbound.try_recv() {
            cycle1.push(msg);
        }
        assert!(
            cycle1
                .iter()
                .any(|m| matches!(m, RedisOutbound::WorldState(_))),
            "cycle 1 must publish a WorldState; got {cycle1:?}"
        );
        assert!(
            cycle1
                .iter()
                .any(|m| matches!(m, RedisOutbound::NpcDormantHash { .. })),
            "cycle 1 must emit an NpcDormantHash because the store was dirtied by the insert; got {cycle1:?}"
        );

        let (revision, receipt_tx) = cycle1
            .iter()
            .find_map(|message| match message {
                RedisOutbound::NpcDormantHash {
                    revision,
                    receipt_tx,
                    ..
                } => Some((*revision, receipt_tx.clone())),
                _ => None,
            })
            .expect("cycle 1 must expose the dormant HASH receipt boundary");
        receipt_tx
            .send(crate::network::redis_bridge::RedisDeliveryReceipt {
                delivery_id: revision.to_string(),
                outcome: Ok(()),
            })
            .expect("dormant HASH receipt channel must remain connected");

        // Cycle 2: the success receipt is applied and nothing changed, so only WorldState is sent.
        force_publish_cycle(&mut app);
        let mut cycle2 = Vec::new();
        while let Ok(msg) = rx_outbound.try_recv() {
            cycle2.push(msg);
        }
        assert!(
            cycle2
                .iter()
                .any(|m| matches!(m, RedisOutbound::WorldState(_))),
            "cycle 2 must still publish a WorldState (world_state is not dirty-gated); got {cycle2:?}"
        );
        assert!(
            !cycle2
                .iter()
                .any(|m| matches!(m, RedisOutbound::NpcDormantHash { .. })),
            "expected cycle 2 to skip the dormant hash because nothing changed since cycle 1 cleared the dirty flag; an NpcDormantHash was emitted: {cycle2:?}"
        );

        // Cycle 3: dirty the store again (a real mutation) -> dormant hash
        // returns, proving the gate re-arms.
        app.world_mut()
            .resource_mut::<NpcDormantStore>()
            .insert(dormant_snapshot("npc_dormant_b", [33.0, 66.0, 33.0]));
        force_publish_cycle(&mut app);
        let mut cycle3 = Vec::new();
        while let Ok(msg) = rx_outbound.try_recv() {
            cycle3.push(msg);
        }
        assert!(
            cycle3
                .iter()
                .any(|m| matches!(m, RedisOutbound::NpcDormantHash { .. })),
            "expected cycle 3 to emit an NpcDormantHash again after a new insert re-dirtied the store; the gate did not re-arm: {cycle3:?}"
        );
    }

    #[test]
    fn world_state_marks_realm_collapse_zone_collapsed() {
        let (mut app, rx_outbound) = setup_publish_app(true);
        app.world_mut()
            .resource_mut::<ZoneRegistry>()
            .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
            .expect("spawn zone should exist")
            .active_events
            .push(EVENT_REALM_COLLAPSE.to_string());

        let state = publish_once(&mut app, &rx_outbound);
        let spawn_zone = state
            .zones
            .iter()
            .find(|zone| zone.name == DEFAULT_SPAWN_ZONE_NAME)
            .expect("spawn fallback zone should still be emitted");

        assert_eq!(spawn_zone.status, ZoneStatusV1::Collapsed);
        assert!(spawn_zone
            .active_events
            .iter()
            .any(|event| event == EVENT_REALM_COLLAPSE));
    }

    #[test]
    fn world_state_marks_tsy_race_out_zone() {
        let (mut app, rx_outbound) = setup_publish_app(true);
        app.world_mut()
            .resource_mut::<ZoneRegistry>()
            .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
            .expect("spawn zone should exist")
            .active_events
            .push(EVENT_TSY_RACE_OUT.to_string());

        let state = publish_once(&mut app, &rx_outbound);
        let spawn_zone = state
            .zones
            .iter()
            .find(|zone| zone.name == DEFAULT_SPAWN_ZONE_NAME)
            .expect("spawn fallback zone should still be emitted");

        assert_eq!(spawn_zone.status, ZoneStatusV1::RaceOut);
        assert!(spawn_zone
            .active_events
            .iter()
            .any(|event| event == EVENT_TSY_RACE_OUT));
    }

    #[test]
    fn world_state_prefers_realm_collapse_over_tsy_race_out() {
        let (mut app, rx_outbound) = setup_publish_app(true);
        {
            let mut zones = app.world_mut().resource_mut::<ZoneRegistry>();
            let zone = zones
                .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
                .expect("spawn zone should exist");
            zone.active_events.push(EVENT_TSY_RACE_OUT.to_string());
            zone.active_events.push(EVENT_REALM_COLLAPSE.to_string());
        }

        let state = publish_once(&mut app, &rx_outbound);
        let spawn_zone = state
            .zones
            .iter()
            .find(|zone| zone.name == DEFAULT_SPAWN_ZONE_NAME)
            .expect("spawn fallback zone should still be emitted");

        assert_eq!(spawn_zone.status, ZoneStatusV1::Collapsed);
    }

    /// plan-sword-path-v2 P2.3 review fix —— 化虚一击注册的盲区会把玩家从
    /// world_state.players 中过滤出去；盲区外玩家保留；player_counts 同步少 1。
    /// 守 worldview §八 天道感应：盲区内角色对 agent 是不可见的。
    #[test]
    fn world_state_excludes_player_inside_tiandao_blind_zone() {
        use crate::sword_path::heaven_gate::TiandaoBlindZoneRegistry;
        use crate::sword_path::tiandao_blind::TiandaoBlindZone;

        let (mut app, rx_outbound) = setup_publish_app(true);
        // 盲区中心点正好落在出生区中央，半径 100 格——一个玩家在中心，另一个
        // 在 1000 格外
        let mut registry = TiandaoBlindZoneRegistry::default();
        registry.add(TiandaoBlindZone {
            center: DVec3::new(0.0, 64.0, 0.0),
            radius: 100.0,
            ttl_ticks: 6000,
            created_tick: 0,
        });
        app.insert_resource(registry);

        spawn_test_client(&mut app, "inside_blind", [0.0, 64.0, 0.0]);
        spawn_test_client(&mut app, "outside_blind", [1000.0, 64.0, 0.0]);

        let state = publish_once(&mut app, &rx_outbound);

        let names: Vec<&str> = state.players.iter().map(|p| p.name.as_str()).collect();
        assert!(
            !names.contains(&"inside_blind"),
            "化虚一击注册的盲区内玩家必须从 world_state.players 中消失，\
             否则 agent 仍能看到 caster 坐标。实际 names={names:?}"
        );
        assert!(
            names.contains(&"outside_blind"),
            "盲区外玩家应保留，实际 names={names:?}"
        );
        // 盲区内玩家也不应计入 player_count（zones 列表中 spawn 的 count 应为 1
        // 不是 2）
        let spawn_zone = state
            .zones
            .iter()
            .find(|zone| zone.name == DEFAULT_SPAWN_ZONE_NAME);
        if let Some(spawn_zone) = spawn_zone {
            assert!(
                spawn_zone.player_count <= 1,
                "盲区内玩家不应计入 player_count，实际 spawn.player_count={}",
                spawn_zone.player_count
            );
        }
    }

    /// plan-sword-path-v2 P2.3 review fix —— 没有 TiandaoBlindZoneRegistry resource
    /// 时（如其他 plan 单测沿用同一 setup），行为不变，所有玩家都进 world_state。
    #[test]
    fn world_state_includes_all_players_when_no_blind_zone_registry() {
        let (mut app, rx_outbound) = setup_publish_app(true);
        spawn_test_client(&mut app, "player_a", [0.0, 64.0, 0.0]);
        spawn_test_client(&mut app, "player_b", [1000.0, 64.0, 0.0]);

        let state = publish_once(&mut app, &rx_outbound);

        let names: Vec<&str> = state.players.iter().map(|p| p.name.as_str()).collect();
        assert!(
            names.contains(&"player_a") && names.contains(&"player_b"),
            "无盲区 registry 时所有玩家都应出现在 world_state，实际 names={names:?}"
        );
    }

    /// plan-sword-path-v2 P2.3 review fix —— 盲区过期后玩家应重新出现。
    /// 验证 TiandaoBlindZone 是否到 TTL 后被 `tick_expire` 移除的端到端链路。
    #[test]
    fn world_state_re_includes_player_after_blind_zone_expires() {
        use crate::sword_path::heaven_gate::TiandaoBlindZoneRegistry;
        use crate::sword_path::tiandao_blind::TiandaoBlindZone;

        let (mut app, rx_outbound) = setup_publish_app(true);
        // 注册 ttl=1 的盲区，下一次 tick_expire 会清理它。
        let mut registry = TiandaoBlindZoneRegistry::default();
        registry.add(TiandaoBlindZone {
            center: DVec3::new(0.0, 64.0, 0.0),
            radius: 100.0,
            ttl_ticks: 1,
            created_tick: 0,
        });
        app.insert_resource(registry);

        spawn_test_client(&mut app, "tester", [0.0, 64.0, 0.0]);

        // 第 1 次 publish：盲区还活跃，玩家被隐藏。
        let state1 = publish_once(&mut app, &rx_outbound);
        assert!(
            !state1.players.iter().any(|p| p.name == "tester"),
            "首发 publish 时玩家应被盲区隐藏"
        );

        // 手动清空盲区 registry 模拟 tick_expire 完成。
        {
            let mut registry = app.world_mut().resource_mut::<TiandaoBlindZoneRegistry>();
            registry.tick_expire(u64::MAX);
        }
        // 让下一次 publish 触发：把 timer 回到能 publish 的状态
        app.world_mut().resource_mut::<WorldStateTimer>().ticks =
            WORLD_STATE_PUBLISH_INTERVAL_TICKS - 1;

        let state2 = publish_once(&mut app, &rx_outbound);
        assert!(
            state2.players.iter().any(|p| p.name == "tester"),
            "盲区过期后 publish 应重新包含玩家"
        );
    }

    #[test]
    fn uses_generation_aware_canonical_ids() {
        let (mut app, rx_outbound) = setup_publish_app(false);
        let player_entity = spawn_test_client(&mut app, "Azure", [8.0, 66.0, 8.0]);
        let npc_entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcBlackboard {
                    nearest_player: Some(player_entity),
                    ..Default::default()
                },
                Position::new([14.0, 66.0, 14.0]),
                EntityKind::ZOMBIE,
            ))
            .id();
        let expected_npc_id = format!("npc_{}v{}", npc_entity.index(), npc_entity.generation());

        let state = publish_once(&mut app, &rx_outbound);
        let player = state
            .players
            .iter()
            .find(|player| player.name == "Azure")
            .expect("Azure should be present in the world snapshot");
        let npc = state
            .npcs
            .iter()
            .find(|npc| npc.id == expected_npc_id)
            .expect("NPC snapshot should use the generation-aware canonical id");

        assert_eq!(player.uuid, "offline:Azure");
        assert_eq!(player.name, "Azure");
        assert_eq!(player.zone, DEFAULT_SPAWN_ZONE_NAME);
        assert_eq!(npc.id, canonical_npc_id(npc_entity));
        assert_eq!(npc.id, expected_npc_id);
        assert!(
            npc.id.contains('v'),
            "NPC canonical ids must include entity generation"
        );
        assert_eq!(
            npc.blackboard.get("nearest_player"),
            Some(&serde_json::Value::String("offline:Azure".to_string()))
        );
        assert!(
            state
                .players
                .iter()
                .all(|player| !player.uuid.contains("player_")),
            "canonical player ids must be offline:{{username}}, not offline:player_{{i}}"
        );
    }

    #[test]
    fn uses_attached_player_state_when_present() {
        let (mut app, rx_outbound) = setup_publish_app(true);
        let player_entity = spawn_test_client(&mut app, "Azure", [8.0, 66.0, 8.0]);

        app.world_mut().entity_mut(player_entity).insert((
            PlayerState {
                karma: 0.2,
                inventory_score: 0.4,
            },
            Cultivation {
                realm: Realm::Condense,
                qi_current: 78.0,
                qi_max: 100.0,
                ..Cultivation::default()
            },
        ));

        let state = publish_once(&mut app, &rx_outbound);
        let player = state
            .players
            .iter()
            .find(|player| player.name == "Azure")
            .expect("Azure should be present in the world snapshot");

        assert_eq!(player.realm, "Condense");
        assert_eq!(player.zone, DEFAULT_SPAWN_ZONE_NAME);
        assert!(
            player.composite_power > 0.0,
            "attached PlayerState should replace placeholder composite power"
        );
        assert!(
            player.breakdown.combat > 0.0,
            "attached PlayerState should replace placeholder power breakdown"
        );
    }

    #[test]
    fn publishes_attached_social_snapshot_when_present() {
        let (mut app, rx_outbound) = setup_publish_app(true);
        let player_entity = spawn_test_client(&mut app, "Azure", [8.0, 66.0, 8.0]);

        app.world_mut().entity_mut(player_entity).insert((
            Anonymity {
                displayed_name: None,
                exposed_to: ["char:bob".to_string(), "char:chen".to_string()]
                    .into_iter()
                    .collect(),
            },
            Renown {
                fame: 12,
                notoriety: 7,
                tags: (0..6)
                    .map(|index| RenownTagV1 {
                        tag: format!("tag:{index}"),
                        weight: index as f64,
                        last_seen_tick: WORLD_STATE_PUBLISH_INTERVAL_TICKS,
                        permanent: false,
                    })
                    .collect(),
            },
            Relationships {
                edges: vec![Relationship {
                    kind: RelationshipKindV1::Feud,
                    peer: "char:rival".to_string(),
                    since_tick: 17,
                    metadata: serde_json::json!({ "place": "spawn" }),
                }],
            },
        ));

        let state = publish_once(&mut app, &rx_outbound);
        let player = state
            .players
            .iter()
            .find(|player| player.name == "Azure")
            .expect("Azure should be present in the world snapshot");
        let social = player
            .social
            .as_ref()
            .expect("attached social components should publish a social snapshot");

        assert_eq!(social.exposed_to_count, 2);
        assert_eq!(social.renown.fame, 12);
        assert_eq!(social.renown.notoriety, 7);
        assert_eq!(social.renown.top_tags.len(), 5);
        assert_eq!(social.renown.top_tags[0].tag, "tag:5");
        assert_eq!(social.relationships.len(), 1);
        assert_eq!(social.relationships[0].kind, RelationshipKindV1::Feud);
        assert_eq!(social.relationships[0].peer, "char:rival");
        assert_eq!(social.relationships[0].since_tick, 17);
        assert_eq!(social.relationships[0].metadata["place"], "spawn");
    }

    #[test]
    fn publishes_optional_faction_and_disciple_summaries() {
        let (mut app, rx_outbound) = setup_publish_app(false);
        let faction_store = app.world().resource::<FactionStore>();
        assert_eq!(
            faction_store.factions.len(),
            3,
            "faction resource should bootstrap three factions"
        );

        let npc_entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcBlackboard::default(),
                Position::new([14.0, 66.0, 14.0]),
                EntityKind::ZOMBIE,
                npc_runtime_bundle(Entity::PLACEHOLDER, NpcArchetype::Disciple, Realm::Awaken),
                FactionMembership {
                    faction_id: FactionId::Defend,
                    rank: FactionRank::Disciple,
                    reputation: Reputation { loyalty: 0.81 },
                    lineage: Some(Lineage {
                        master_id: Some("npc_master_001".to_string()),
                        disciple_ids: vec!["npc_peer_001".to_string()],
                    }),
                    mission_queue: MissionQueue {
                        pending: vec![MissionId("mission:defend_gate".to_string())],
                    },
                },
            ))
            .id();

        let state = publish_once(&mut app, &rx_outbound);
        let npc = state
            .npcs
            .iter()
            .find(|npc| npc.id == canonical_npc_id(npc_entity))
            .expect("disciple npc should be present in snapshot");
        let digest = npc.digest.as_ref().expect("npc digest should be present");
        let disciple = digest
            .disciple
            .as_ref()
            .expect("disciple summary should be present for faction-bound npc");

        assert_eq!(digest.archetype, "disciple");
        assert_eq!(disciple.faction_id, FactionId::Defend);
        assert_eq!(disciple.rank, FactionRank::Disciple);
        assert!((disciple.loyalty - 0.81).abs() < 1e-6);
        assert_eq!(
            disciple
                .lineage
                .as_ref()
                .and_then(|lineage| lineage.master_id.as_deref()),
            Some("npc_master_001")
        );
        assert_eq!(
            disciple
                .mission_queue
                .as_ref()
                .and_then(|queue| queue.top_mission_id.as_deref()),
            Some("mission:defend_gate")
        );

        let factions = state
            .factions
            .as_ref()
            .expect("faction summaries should be published when store exists");
        assert_eq!(factions.len(), 3);
        assert_eq!(factions[0].id, FactionId::Attack);
        assert_eq!(factions[1].id, FactionId::Defend);
        assert_eq!(factions[2].id, FactionId::Neutral);
        assert!(factions
            .iter()
            .all(|summary| summary.mission_queue.is_none()));
    }
}

/// plan-offscreen-war-v1 P0：`publish_qi_ledger_to_redis`（exclusive system）饱和测试。
///
/// 锁住可观察行为：① cadence 节流（非整周期不发）② 整周期 + bridge 在场 → 恰好一条
/// `RedisOutbound::QiLedgerHash`，聚合字段齐全 ③ 无 bridge → 静默 no-op（不 panic）
/// ④ 无 `WorldQiAccount`（FIX-1 graceful 路径）→ 仍发布，聚合字段在场（不 panic）。
mod qi_ledger_publish_tests {
    use super::*;
    use crate::qi_physics::{QiAccountId, WorldQiAccount, WorldQiBudget};

    /// 构建只挂 `publish_qi_ledger_to_redis` 的最小 App。
    /// `at_cadence=true` 时把 `QiLedgerTimer` 预置到 `INTERVAL-1`，一次 update 后命中整周期。
    /// `with_bridge=false` 时不插 `RedisBridgeResource`，验证缺 bridge 静默路径。
    /// `with_account=false` 时不插 `WorldQiAccount`，验证 FIX-1 graceful 路径。
    fn setup_qi_ledger_app(
        at_cadence: bool,
        with_bridge: bool,
        with_account: bool,
    ) -> (App, Option<Receiver<RedisOutbound>>) {
        let mut app = App::new();

        let initial_ticks = if at_cadence {
            WORLD_STATE_PUBLISH_INTERVAL_TICKS - 1
        } else {
            0
        };
        app.insert_resource(QiLedgerTimer {
            ticks: initial_ticks,
        });

        let rx = if with_bridge {
            let (tx_outbound, rx_outbound) = unbounded();
            let (_tx_inbound, rx_inbound) = unbounded();
            app.insert_resource(RedisBridgeResource {
                tx_outbound,
                rx_inbound,
            });
            Some(rx_outbound)
        } else {
            None
        };

        if with_account {
            app.insert_resource(WorldQiAccount::default());
        }
        // WorldQiBudget 缺省时 summarize_world_qi 会 unwrap_or_default()，
        // budget_initial_total 回退 DEFAULT_SPIRIT_QI_TOTAL——这里显式插入更贴近真服。
        app.insert_resource(WorldQiBudget::default());

        app.add_systems(Update, publish_qi_ledger_to_redis);

        (app, rx)
    }

    /// 在 fields 列表里取某 key 的值（缺则 panic 带修复线索）。
    fn field<'a>(fields: &'a [(String, String)], key: &str) -> &'a str {
        fields
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
            .unwrap_or_else(|| {
                panic!(
                    "qi/ledger HASH 应含聚合字段 `{key}`（外部 e2e 守恒断言依赖它），实际字段集 {:?}",
                    fields.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>()
                )
            })
    }

    fn drain_qi_ledger(rx: &Receiver<RedisOutbound>) -> Option<Vec<(String, String)>> {
        match rx.try_recv() {
            Ok(RedisOutbound::QiLedgerHash(fields)) => Some(fields),
            Ok(other) => panic!("期望 RedisOutbound::QiLedgerHash，实际得到 {other:?}"),
            Err(_) => None,
        }
    }

    #[test]
    fn below_cadence_emits_nothing() {
        // ticks 从 0 → 1（非 INTERVAL 整数倍）：节流，绝不发 QiLedgerHash。
        let (mut app, rx) = setup_qi_ledger_app(false, true, true);
        let rx = rx.expect("bridge 在场时 rx 必须存在");

        app.update();

        assert!(
            drain_qi_ledger(&rx).is_none(),
            "非整周期（tick=1）必须节流不发 QiLedgerHash，外部脚本不应收到本帧 telemetry"
        );
        assert_eq!(
            app.world().resource::<QiLedgerTimer>().ticks,
            1,
            "节流分支也必须推进 timer（0→1），否则永远到不了下一个整周期"
        );
    }

    #[test]
    fn at_cadence_with_bridge_emits_exactly_one_hash_with_aggregates() {
        let (mut app, rx) = setup_qi_ledger_app(true, true, true);
        let rx = rx.expect("bridge 在场时 rx 必须存在");

        app.update();

        let fields = drain_qi_ledger(&rx)
            .expect("整周期 + bridge 在场必须发恰好一条 QiLedgerHash，外部脚本据此对账");
        // 聚合字段必须齐全——外部 e2e 守恒断言锚点。
        assert!(
            !field(&fields, "total_observed").is_empty(),
            "total_observed 必须有值（已落位真元总量），供 e2e 精确守恒断言"
        );
        assert_eq!(
            field(&fields, "budget_initial_total"),
            crate::qi_physics::constants::DEFAULT_SPIRIT_QI_TOTAL.to_string(),
            "budget_initial_total 是守恒预算锚点，须等于 DEFAULT_SPIRIT_QI_TOTAL"
        );

        // 同一周期内只发一条，不重复。
        assert!(
            drain_qi_ledger(&rx).is_none(),
            "单个整周期 update 必须恰好发一条 QiLedgerHash，不能重复 enqueue"
        );
    }

    #[test]
    fn at_cadence_emits_per_account_rows_when_account_present() {
        // 有账户余额时，per-account 行须随聚合字段一起上线（稳定 lowercase wire key）。
        let (mut app, rx) = setup_qi_ledger_app(true, true, true);
        app.world_mut()
            .resource_mut::<WorldQiAccount>()
            .set_balance(QiAccountId::zone("spawn"), 12.0)
            .expect("set_balance 应接受有限非负值");
        let rx = rx.expect("bridge 在场时 rx 必须存在");

        app.update();

        let fields = drain_qi_ledger(&rx).expect("整周期 + bridge 在场必须发 QiLedgerHash");
        assert_eq!(
            field(&fields, "account:zone:spawn"),
            "12",
            "zone 账户余额须以稳定 lowercase wire key `account:zone:spawn` 上线"
        );
    }

    #[test]
    fn at_cadence_without_bridge_is_silent_noop() {
        // 无 RedisBridgeResource（部分测试 app）：必须静默跳过，绝不 panic。
        let (mut app, rx) = setup_qi_ledger_app(true, false, true);
        assert!(rx.is_none(), "本用例不挂 bridge");

        // 不 panic 即通过；timer 仍推进到整周期。
        app.update();

        assert!(
            app.world()
                .resource::<QiLedgerTimer>()
                .ticks
                .is_multiple_of(WORLD_STATE_PUBLISH_INTERVAL_TICKS),
            "缺 bridge 路径也必须推进 timer 到整周期（仅跳过发布），否则计数与真服不一致"
        );
    }

    #[test]
    fn at_cadence_without_world_qi_account_still_publishes() {
        // FIX-1 graceful 路径：缺 WorldQiAccount 时不得 panic（旧码 world.resource:: 会炸），
        // 仍须发布聚合字段（per-account 行为空，因为没有账本）。
        let (mut app, rx) = setup_qi_ledger_app(true, true, false);
        let rx = rx.expect("bridge 在场时 rx 必须存在");
        assert!(
            app.world().get_resource::<WorldQiAccount>().is_none(),
            "本用例刻意不插 WorldQiAccount，复现 FIX-1 缺资源场景"
        );

        // 不 panic 即跨过 FIX-1 关口。
        app.update();

        let fields = drain_qi_ledger(&rx)
            .expect("缺 WorldQiAccount 时仍须发布聚合 telemetry（FIX-1：graceful 而非 panic）");
        assert!(
            !field(&fields, "total_observed").is_empty(),
            "缺账本时 total_observed 仍来自 zone/player 快照，必须有值"
        );
        assert!(
            fields.iter().all(|(key, _)| !key.starts_with("account:")),
            "缺 WorldQiAccount 时不应有任何 account: 行（无账本可枚举），实际 {:?}",
            fields.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>()
        );
    }
}

mod narration_tests {
    use super::*;
    use crate::cultivation::life_record::{LifeRecord, SkillMilestone};
    use crate::schema::common::{NarrationKind, NarrationScope, NarrationStyle};
    use crate::schema::narration::{Narration, NarrationV1};
    use crate::skill::components::SkillId;
    use crate::world::zone::Zone;
    use crossbeam_channel::Sender;
    use valence::message::SendMessage;
    use valence::prelude::DVec3;
    use valence::protocol::packets::play::{CustomPayloadS2c, GameMessageS2c};
    use valence::testing::MockClientHelper;

    fn setup_narration_app_with_agent_ui(
        zone_registry: Option<ZoneRegistry>,
    ) -> (App, Sender<RedisInbound>, Receiver<RedisOutbound>) {
        let (tx_outbound, rx_outbound) = unbounded();
        let (tx_inbound, rx_inbound) = unbounded();
        let mut app = App::new();

        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.insert_resource(CommandExecutorResource::default());
        app.insert_resource(NarrationDedupeResource::default());
        app.init_resource::<agent_ui::AgentUiSessionStore>();

        if let Some(zone_registry) = zone_registry {
            app.insert_resource(zone_registry);
        }

        app.add_event::<crate::cultivation::insight::InsightOffer>();
        app.add_event::<agent_ui::AgentUiCmdEvent>();
        app.add_systems(
            Update,
            (process_redis_inbound, agent_ui::receive_agent_ui_cmd_system),
        );

        (app, tx_inbound, rx_outbound)
    }

    fn setup_narration_app(zone_registry: Option<ZoneRegistry>) -> (App, Sender<RedisInbound>) {
        let (app, tx_inbound, _rx_outbound) = setup_narration_app_with_agent_ui(zone_registry);
        (app, tx_inbound)
    }

    fn spawn_test_client_with_helper(
        app: &mut App,
        username: &str,
        position: [f64; 3],
    ) -> (Entity, MockClientHelper) {
        let (mut client_bundle, helper) = create_mock_client(username);
        client_bundle.player.position = Position::new(position);

        let entity = app.world_mut().spawn(client_bundle).id();
        (entity, helper)
    }

    fn enqueue_single_narration(tx_inbound: &Sender<RedisInbound>, narration: Narration) {
        tx_inbound
            .send(RedisInbound::AgentNarration(NarrationV1 {
                v: 1,
                narrations: vec![narration],
            }))
            .expect("narration message should enqueue into inbound channel");
    }

    fn flush_all_client_packets(app: &mut App) {
        let world = app.world_mut();
        let mut query = world.query::<&mut Client>();

        for mut client in query.iter_mut(world) {
            client
                .flush_packets()
                .expect("mock client packets should flush successfully");
        }
    }

    fn collect_narration_and_chat_packets(
        helper: &mut MockClientHelper,
    ) -> (Vec<ServerDataV1>, usize) {
        let mut payloads = Vec::new();
        let mut game_message_packets = 0;

        for frame in helper.collect_received().0 {
            if frame.decode::<GameMessageS2c>().is_ok() {
                game_message_packets += 1;
            }

            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };

            if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                continue;
            }

            let payload: ServerDataV1 = serde_json::from_slice(packet.data.0 .0)
                .expect("typed custom payload should decode as ServerDataV1 JSON");

            if matches!(payload.payload, ServerDataPayloadV1::Narration { .. }) {
                payloads.push(payload);
            }
        }

        (payloads, game_message_packets)
    }

    fn assert_single_narration_payload(payloads: &[ServerDataV1], expected_text: &str) {
        assert_eq!(
            payloads.len(),
            1,
            "expected exactly one typed narration payload"
        );

        match &payloads[0].payload {
            ServerDataPayloadV1::Narration { narrations } => {
                assert_eq!(narrations.len(), 1, "expected exactly one narration entry");
                assert_eq!(narrations[0].text, expected_text);
            }
            other => panic!("expected narration payload, got {other:?}"),
        }
    }

    #[test]
    fn broadcast_emits_only_typed_narration_payload() {
        let (mut app, tx_inbound) = setup_narration_app(None);
        let (_alice, mut alice_helper) =
            spawn_test_client_with_helper(&mut app, "Alice", [8.0, 66.0, 8.0]);

        enqueue_single_narration(
            &tx_inbound,
            Narration {
                scope: NarrationScope::Broadcast,
                target: None,
                text: "天地震荡，灵气翻涌。".to_string(),
                style: NarrationStyle::Narration,
                kind: None,
            },
        );

        app.update();
        flush_all_client_packets(&mut app);

        let (alice_payloads, alice_chat_packets) =
            collect_narration_and_chat_packets(&mut alice_helper);

        assert_single_narration_payload(alice_payloads.as_slice(), "天地震荡，灵气翻涌。");
        assert_eq!(
            alice_chat_packets, 0,
            "narration path should not emit mirrored GameMessageS2c chat packets"
        );
    }

    #[test]
    fn packet_collection_classifies_narration_and_chat_from_the_same_batch() {
        let (mut app, tx_inbound) = setup_narration_app(None);
        let (alice, mut alice_helper) =
            spawn_test_client_with_helper(&mut app, "Alice", [8.0, 66.0, 8.0]);

        enqueue_single_narration(
            &tx_inbound,
            Narration {
                scope: NarrationScope::Broadcast,
                target: None,
                text: "同批分类测试。".to_string(),
                style: NarrationStyle::Narration,
                kind: None,
            },
        );

        app.update();
        app.world_mut()
            .get_mut::<Client>(alice)
            .expect("test client should remain connected")
            .send_chat_message("同批聊天探针。");
        flush_all_client_packets(&mut app);

        let (payloads, chat_packets) = collect_narration_and_chat_packets(&mut alice_helper);
        assert_single_narration_payload(payloads.as_slice(), "同批分类测试。");
        assert_eq!(
            chat_packets, 1,
            "single receive batch should retain and classify the injected GameMessageS2c probe"
        );
    }

    #[test]
    fn zone_scope_filters_by_zone() {
        let spawn_zone = Zone {
            name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            dimension: crate::world::dimension::DimensionKind::Overworld,
            bounds: (DVec3::new(0.0, 64.0, 0.0), DVec3::new(128.0, 128.0, 128.0)),
            spirit_qi: 0.9,
            danger_level: 0,
            active_events: Vec::new(),
            patrol_anchors: vec![DVec3::new(14.0, 66.0, 14.0)],
            blocked_tiles: Vec::new(),
            qi_equilibrium: 0.0,
            qi_inflow_per_min: 0.0,
        };
        let blood_valley = Zone {
            name: "blood_valley".to_string(),
            dimension: crate::world::dimension::DimensionKind::Overworld,
            bounds: (
                DVec3::new(1000.0, 64.0, 1000.0),
                DVec3::new(1200.0, 128.0, 1200.0),
            ),
            spirit_qi: 0.4,
            danger_level: 4,
            active_events: Vec::new(),
            patrol_anchors: vec![DVec3::new(1004.0, 66.0, 1004.0)],
            blocked_tiles: Vec::new(),
            qi_equilibrium: 0.0,
            qi_inflow_per_min: 0.0,
        };

        let zone_registry = ZoneRegistry {
            spatial_revision: 0,
            zones: vec![spawn_zone, blood_valley],
        };

        let (mut app, tx_inbound) = setup_narration_app(Some(zone_registry));
        let (_alice, mut alice_helper) =
            spawn_test_client_with_helper(&mut app, "Alice", [8.0, 66.0, 8.0]);
        let (_bob, mut bob_helper) =
            spawn_test_client_with_helper(&mut app, "Bob", [1005.0, 66.0, 1005.0]);

        enqueue_single_narration(
            &tx_inbound,
            Narration {
                scope: NarrationScope::Zone,
                target: Some("blood_valley".to_string()),
                text: "血谷雷云聚集。".to_string(),
                style: NarrationStyle::SystemWarning,
                kind: None,
            },
        );

        app.update();
        flush_all_client_packets(&mut app);

        let (alice_payloads, alice_chat_packets) =
            collect_narration_and_chat_packets(&mut alice_helper);
        let (bob_payloads, bob_chat_packets) =
            collect_narration_and_chat_packets(&mut bob_helper);

        assert!(
            alice_payloads.is_empty(),
            "spawn zone player should not receive blood_valley scoped narration"
        );
        assert_eq!(
            alice_chat_packets, 0,
            "zone-scoped narration should not mirror chat packets"
        );
        assert_single_narration_payload(bob_payloads.as_slice(), "血谷雷云聚集。");
        assert_eq!(
            bob_chat_packets, 0,
            "zone-scoped narration should not mirror chat packets"
        );
    }

    #[test]
    fn player_scope_matches_username_and_offline_id() {
        let (mut app, tx_inbound) = setup_narration_app(None);
        let (_steve, mut steve_helper) =
            spawn_test_client_with_helper(&mut app, "Steve", [8.0, 66.0, 8.0]);
        let (_alex, mut alex_helper) =
            spawn_test_client_with_helper(&mut app, "Alex", [18.0, 66.0, 18.0]);

        enqueue_single_narration(
            &tx_inbound,
            Narration {
                scope: NarrationScope::Player,
                target: Some("Steve".to_string()),
                text: "第一段单人叙事。".to_string(),
                style: NarrationStyle::Perception,
                kind: None,
            },
        );

        app.update();
        flush_all_client_packets(&mut app);

        let (steve_plain, steve_chat_packets) =
            collect_narration_and_chat_packets(&mut steve_helper);
        let (alex_plain, alex_chat_packets) =
            collect_narration_and_chat_packets(&mut alex_helper);

        assert_single_narration_payload(steve_plain.as_slice(), "第一段单人叙事。");
        assert!(
            alex_plain.is_empty(),
            "non-targeted player must not receive payload"
        );
        assert_eq!(
            steve_chat_packets, 0,
            "player-scoped narration should not mirror chat packets"
        );
        assert_eq!(
            alex_chat_packets, 0,
            "non-targeted player must not receive chat packets"
        );

        enqueue_single_narration(
            &tx_inbound,
            Narration {
                scope: NarrationScope::Player,
                target: Some("offline:Steve".to_string()),
                text: "第二段单人叙事。".to_string(),
                style: NarrationStyle::Perception,
                kind: None,
            },
        );

        app.update();
        flush_all_client_packets(&mut app);

        let (steve_alias, steve_alias_chat_packets) =
            collect_narration_and_chat_packets(&mut steve_helper);
        let (alex_alias, alex_alias_chat_packets) =
            collect_narration_and_chat_packets(&mut alex_helper);

        assert_single_narration_payload(steve_alias.as_slice(), "第二段单人叙事。");
        assert!(
            alex_alias.is_empty(),
            "non-targeted player must not receive payload"
        );
        assert_eq!(
            steve_alias_chat_packets, 0,
            "player-scoped narration should not mirror chat packets"
        );
        assert_eq!(
            alex_alias_chat_packets, 0,
            "non-targeted player must not receive chat packets"
        );
    }

    fn consume_agent_ui_response_through_tiandao(
        response: &crate::schema::agent_ui::AgentUiResponsePayloadV1,
    ) -> Option<NarrationV1> {
        use std::io::Write as _;
        use std::process::{Command, Stdio};

        let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("server crate should live directly below the repository root");
        let tsx = repo_root.join("agent/node_modules/.bin/tsx");
        let runner =
            repo_root.join("agent/packages/tiandao/tests/ui-response-consumer-runner.ts");
        if !tsx.is_file() || !runner.is_file() {
            let message = format!(
                "cross-stack Tiandao consumer unavailable (tsx={}, runner={})",
                tsx.display(),
                runner.display()
            );
            if std::env::var_os("CI").is_some() {
                panic!("{message}; CI must install agent dependencies before cargo test");
            }
            eprintln!("[skip] {message}; run `cd agent && npm ci` for the full local chain");
            return None;
        }

        let (response_channel, producer_json) =
            redis_bridge::encode_agent_ui_response_wire_for_test(response)
                .expect("server producer response should pass the production Redis encoder");
        assert_eq!(
            response_channel,
            crate::schema::channels::CH_AGENT_UI_RESPONSE,
            "AgentUiResponse must use the production response channel"
        );
        let mut child = Command::new(&tsx)
            .arg(&runner)
            .current_dir(repo_root.join("agent/packages/tiandao"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|error| {
                panic!(
                    "failed to start production UiResponseConsumer through {}: {error}",
                    tsx.display()
                )
            });
        child
            .stdin
            .take()
            .expect("Tiandao runner stdin should be piped")
            .write_all(producer_json.as_bytes())
            .expect("production Redis response wire should reach the Tiandao runner");

        let output = child
            .wait_with_output()
            .expect("Tiandao runner should exit after one consumer dispatch");
        assert!(
            output.status.success(),
            "production UiResponseConsumer runner failed: status={} stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            output.stderr.is_empty(),
            "successful Tiandao runner must not emit stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let consumer_wire = std::str::from_utf8(&output.stdout).unwrap_or_else(|error| {
            panic!(
                "UiResponseConsumer stdout must be UTF-8 Redis wire JSON: {error}; stdout={:?}",
                output.stdout
            )
        });
        Some(
            redis_bridge::parse_agent_narration_wire_for_test(consumer_wire).unwrap_or_else(
                |error| {
                    panic!(
                        "UiResponseConsumer stdout must pass the production narration decoder: {error}; stdout={consumer_wire}"
                    )
                },
            ),
        )
    }

    #[test]
    fn realm_gate_producer_consumer_selector_routes_only_target_player() {
        const TARGET_USERNAME: &str = "E2EPlayer";
        const BYSTANDER_USERNAME: &str = "Bystander";
        const EXPECTED_TEXT: &str =
            "天道的注意力掠过，境界未至，缘分尚浅——此时感知到的，只是一片模糊的余韵。";

        let (mut app, tx_inbound, rx_outbound) = setup_narration_app_with_agent_ui(None);
        let (target, mut target_helper) =
            spawn_test_client_with_helper(&mut app, TARGET_USERNAME, [8.0, 66.0, 8.0]);
        let (bystander, mut bystander_helper) =
            spawn_test_client_with_helper(&mut app, BYSTANDER_USERNAME, [18.0, 66.0, 18.0]);
        app.world_mut().entity_mut(target).insert(Cultivation {
            realm: Realm::Induce,
            ..Cultivation::default()
        });
        app.world_mut().entity_mut(bystander).insert(Cultivation {
            realm: Realm::Void,
            ..Cultivation::default()
        });

        let target_player = canonical_player_id(TARGET_USERNAME);
        app.world_mut()
            .send_event(agent_ui::AgentUiCmdEvent(
                crate::schema::agent_ui::AgentUiRequestCommandV1 {
                    request_id: "req-realm-gate-real-chain".to_string(),
                    target_player: target_player.clone(),
                    xml: r#"<owo-ui><components><flow-layout><button id="btn_a">确认</button></flow-layout></components></owo-ui>"#.to_string(),
                    timeout_ticks: 600,
                    realm_gate: Realm::Condense.rank(),
                    allowed_button_ids: vec!["btn_a".to_string()],
                },
            ));

        // Stage 1: execute the production server gate and take its real Redis response.
        app.update();
        let response = match rx_outbound
            .try_recv()
            .expect("realm gate producer should emit one Redis response")
        {
            RedisOutbound::AgentUiResponse(response) => response,
            other => panic!("expected AgentUiResponse from producer, got {other:?}"),
        };
        assert_eq!(response.request_id, "req-realm-gate-real-chain");
        assert!(matches!(
            response.action,
            crate::schema::agent_ui::AgentUiActionType::Error
        ));
        assert_eq!(
            response.target_player.as_deref(),
            Some(target_player.as_str())
        );
        assert_eq!(
            response.params.get("reason").map(String::as_str),
            Some("realm_gate_rejected")
        );
        assert_eq!(
            response.params.get("player_realm").map(String::as_str),
            Some("2")
        );
        assert_eq!(
            response.params.get("required_realm").map(String::as_str),
            Some("3")
        );
        assert!(
            rx_outbound.try_recv().is_err(),
            "realm gate producer must emit exactly one response"
        );

        // Stage 2: execute the production TypeScript UiResponseConsumer in a real process.
        let Some(narration_envelope) = consume_agent_ui_response_through_tiandao(&response)
        else {
            // 环境缺 tsx devDependency：跨栈阶段跳过（Stage 1 的 producer gate 已验）。
            return;
        };
        assert_eq!(
            narration_envelope.narrations.len(),
            1,
            "UiResponseConsumer should publish exactly one private narration"
        );
        assert_eq!(
            narration_envelope.narrations[0].target.as_deref(),
            Some(target_player.as_str()),
            "consumer must preserve the producer's canonical target_player"
        );
        assert_eq!(narration_envelope.narrations[0].text, EXPECTED_TEXT);

        // Stage 3: feed that actual consumer output through the production recipient selector.
        tx_inbound
            .send(RedisInbound::AgentNarration(narration_envelope))
            .expect("consumer narration should enqueue into the server Redis inbound channel");
        app.update();
        flush_all_client_packets(&mut app);

        let (target_payloads, target_chat_packets) =
            collect_narration_and_chat_packets(&mut target_helper);
        let (bystander_payloads, bystander_chat_packets) =
            collect_narration_and_chat_packets(&mut bystander_helper);
        assert_single_narration_payload(target_payloads.as_slice(), EXPECTED_TEXT);
        match &target_payloads[0].payload {
            ServerDataPayloadV1::Narration { narrations } => assert_eq!(
                narrations[0].style,
                NarrationStyle::SystemWarning,
                "target player must receive a realm-gate system_warning"
            ),
            other => panic!("real chain should produce narration payload, got {other:?}"),
        }
        assert!(
            bystander_payloads.is_empty(),
            "bystander {BYSTANDER_USERNAME} must not receive {TARGET_USERNAME}'s rejection"
        );
        assert_eq!(
            target_chat_packets, 0,
            "target player should not receive a mirrored chat packet"
        );
        assert_eq!(
            bystander_chat_packets, 0,
            "bystander should not receive any mirrored chat packet"
        );
    }

    #[test]
    fn player_scope_matches_char_id_alias() {
        let (mut app, tx_inbound) = setup_narration_app(None);
        let (azure, mut azure_helper) =
            spawn_test_client_with_helper(&mut app, "Azure", [8.0, 66.0, 8.0]);
        let (_bob, mut bob_helper) =
            spawn_test_client_with_helper(&mut app, "Bob", [18.0, 66.0, 18.0]);

        enqueue_single_narration(
            &tx_inbound,
            Narration {
                scope: NarrationScope::Player,
                target: Some(format!("char:{}", azure.to_bits())),
                text: "第三段单人叙事。".to_string(),
                style: NarrationStyle::Narration,
                kind: None,
            },
        );

        app.update();
        flush_all_client_packets(&mut app);

        let (azure_payloads, azure_chat_packets) =
            collect_narration_and_chat_packets(&mut azure_helper);
        let (bob_payloads, bob_chat_packets) =
            collect_narration_and_chat_packets(&mut bob_helper);

        assert_single_narration_payload(azure_payloads.as_slice(), "第三段单人叙事。");
        assert!(
            bob_payloads.is_empty(),
            "char-id targeted narration must not leak to Bob"
        );
        assert_eq!(
            azure_chat_packets, 0,
            "player-scoped narration should not mirror chat packets"
        );
        assert_eq!(
            bob_chat_packets, 0,
            "non-targeted player must not receive chat packets"
        );
    }

    #[test]
    fn agent_skill_lv_up_narration_backfills_matching_life_record_milestone() {
        let (mut app, tx_inbound) = setup_narration_app(None);
        let (azure, _azure_helper) =
            spawn_test_client_with_helper(&mut app, "Azure", [8.0, 66.0, 8.0]);

        app.world_mut().entity_mut(azure).insert(LifeRecord {
            character_id: "offline:Azure".to_string(),
            created_at: 0,
            biography: Vec::new(),
            insights_taken: Vec::new(),
            death_insights: Vec::new(),
            skill_milestones: vec![SkillMilestone {
                skill: SkillId::Herbalism,
                new_lv: 4,
                achieved_at: 120,
                narration: "默认文案。".to_string(),
                total_xp_at: 900,
            }],
            spirit_root_first: None,
            ..LifeRecord::default()
        });

        enqueue_single_narration(
            &tx_inbound,
            Narration {
                scope: NarrationScope::Player,
                target: Some(format!("char:{}|skill:herbalism|lv:4", azure.to_bits())),
                text: "你摘辨草木渐熟，今又进一层，已至Lv.4。".to_string(),
                style: NarrationStyle::Narration,
                kind: None,
            },
        );

        app.update();

        let life_record = app
            .world()
            .get::<LifeRecord>(azure)
            .expect("life record should stay attached");
        assert_eq!(life_record.skill_milestones.len(), 1);
        assert_eq!(
            life_record.skill_milestones[0].narration,
            "你摘辨草木渐熟，今又进一层，已至Lv.4。"
        );
    }

    #[test]
    fn missing_player_target_is_ignored() {
        let (mut app, tx_inbound) = setup_narration_app(None);
        let (_alice, mut alice_helper) =
            spawn_test_client_with_helper(&mut app, "Alice", [8.0, 66.0, 8.0]);
        let (_bob, mut bob_helper) =
            spawn_test_client_with_helper(&mut app, "Bob", [20.0, 66.0, 20.0]);

        enqueue_single_narration(
            &tx_inbound,
            Narration {
                scope: NarrationScope::Player,
                target: Some("offline:Ghost".to_string()),
                text: "不存在目标，不应泄露。".to_string(),
                style: NarrationStyle::Narration,
                kind: None,
            },
        );

        app.update();
        flush_all_client_packets(&mut app);

        let (alice_payloads, alice_chat_packets) =
            collect_narration_and_chat_packets(&mut alice_helper);
        let (bob_payloads, bob_chat_packets) =
            collect_narration_and_chat_packets(&mut bob_helper);

        assert!(
            alice_payloads.is_empty(),
            "missing player target should not leak payload to Alice"
        );
        assert_eq!(
            alice_chat_packets, 0,
            "missing player target should not leak chat packets to Alice"
        );
        assert!(
            bob_payloads.is_empty(),
            "missing player target should not leak payload to Bob"
        );
        assert_eq!(
            bob_chat_packets, 0,
            "missing player target should not leak chat packets to Bob"
        );
    }

    #[test]
    fn duplicate_narration_payload_is_deduped_within_window() {
        let (mut app, tx_inbound) = setup_narration_app(None);
        let (_alice, mut alice_helper) =
            spawn_test_client_with_helper(&mut app, "Alice", [8.0, 66.0, 8.0]);

        let narration = Narration {
            scope: NarrationScope::Broadcast,
            target: None,
            text: "重复叙事只应投递一次。".to_string(),
            style: NarrationStyle::Narration,
            kind: None,
        };

        enqueue_single_narration(&tx_inbound, narration.clone());
        enqueue_single_narration(&tx_inbound, narration);

        app.update();
        flush_all_client_packets(&mut app);

        let (payloads, _) = collect_narration_and_chat_packets(&mut alice_helper);
        assert_eq!(
            payloads.len(),
            1,
            "duplicate narration payload should be dropped by short-window dedupe"
        );
    }

    #[test]
    fn duplicate_political_narration_payload_is_deduped_within_window() {
        let (mut app, tx_inbound) = setup_narration_app(None);
        let (_alice, mut alice_helper) =
            spawn_test_client_with_helper(&mut app, "Alice", [8.0, 66.0, 8.0]);

        let narration = Narration {
            scope: NarrationScope::Broadcast,
            target: None,
            text: "江湖有传，血谷旧怨又添一笔。".to_string(),
            style: NarrationStyle::PoliticalJianghu,
            kind: Some(NarrationKind::PoliticalJianghu),
        };

        enqueue_single_narration(&tx_inbound, narration.clone());
        enqueue_single_narration(&tx_inbound, narration);

        app.update();
        flush_all_client_packets(&mut app);

        let (payloads, _) = collect_narration_and_chat_packets(&mut alice_helper);
        assert_eq!(
            payloads.len(),
            1,
            "duplicate political narration payload should still use the server dedupe resource"
        );
    }
}

mod zone_payload_tests {
    use super::*;
    use crate::world::zone::Zone;
    use valence::prelude::DVec3;
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::MockClientHelper;

    fn setup_zone_transition_app(zone_registry: ZoneRegistry) -> App {
        let mut app = App::new();
        app.insert_resource(ZoneTransitionTracker::default());
        app.insert_resource(zone_registry);
        app.add_systems(Update, emit_zone_info_on_zone_transition);
        app
    }

    fn spawn_test_client_with_helper(
        app: &mut App,
        username: &str,
        position: [f64; 3],
    ) -> (Entity, MockClientHelper) {
        let (mut client_bundle, helper) = create_mock_client(username);
        client_bundle.player.position = Position::new(position);
        let entity = app.world_mut().spawn(client_bundle).id();
        (entity, helper)
    }

    fn flush_all_client_packets(app: &mut App) {
        let world = app.world_mut();
        let mut query = world.query::<&mut Client>();
        for mut client in query.iter_mut(world) {
            client
                .flush_packets()
                .expect("mock client packets should flush successfully");
        }
    }

    fn collect_zone_info_payloads(helper: &mut MockClientHelper) -> Vec<ServerDataV1> {
        let mut payloads = Vec::new();

        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };
            if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                continue;
            }

            let payload: ServerDataV1 = serde_json::from_slice(packet.data.0 .0)
                .expect("typed payload should decode as ServerDataV1");

            if matches!(payload.payload, ServerDataPayloadV1::ZoneInfo { .. }) {
                payloads.push(payload);
            }
        }

        payloads
    }

    #[test]
    fn emits_zone_info_on_transition() {
        let zone_registry = ZoneRegistry {
            spatial_revision: 0,
            zones: vec![
                Zone {
                    name: "spawn".to_string(),
                    dimension: crate::world::dimension::DimensionKind::Overworld,
                    bounds: (DVec3::new(0.0, 64.0, 0.0), DVec3::new(128.0, 128.0, 128.0)),
                    spirit_qi: 0.9,
                    danger_level: 0,
                    active_events: vec![],
                    patrol_anchors: vec![DVec3::new(14.0, 66.0, 14.0)],
                    blocked_tiles: vec![],
                    qi_equilibrium: 0.0,
                    qi_inflow_per_min: 0.0,
                },
                Zone {
                    name: "blood_valley".to_string(),
                    dimension: crate::world::dimension::DimensionKind::Overworld,
                    bounds: (
                        DVec3::new(1000.0, 64.0, 1000.0),
                        DVec3::new(1200.0, 128.0, 1200.0),
                    ),
                    spirit_qi: 0.42,
                    danger_level: 4,
                    active_events: vec!["beast_tide".to_string()],
                    patrol_anchors: vec![DVec3::new(1004.0, 66.0, 1004.0)],
                    blocked_tiles: vec![],
                    qi_equilibrium: 0.0,
                    qi_inflow_per_min: 0.0,
                },
            ],
        };

        let mut app = setup_zone_transition_app(zone_registry);
        let (entity, mut helper) =
            spawn_test_client_with_helper(&mut app, "Alice", [8.0, 66.0, 8.0]);

        app.update();
        flush_all_client_packets(&mut app);

        let first_payloads = collect_zone_info_payloads(&mut helper);
        assert_eq!(
            first_payloads.len(),
            1,
            "first zone snapshot should be sent on initial track"
        );

        match &first_payloads[0].payload {
            ServerDataPayloadV1::ZoneInfo {
                zone,
                spirit_qi,
                danger_level,
                status,
                active_events,
                perception_text,
            } => {
                assert_eq!(zone, "spawn");
                assert_eq!(*spirit_qi, 0.9);
                assert_eq!(*danger_level, 0);
                assert_eq!(*status, ZoneStatusV1::Normal);
                assert_eq!(active_events, &None);
                assert_eq!(perception_text, &None);
            }
            other => panic!("expected zone_info payload, got {other:?}"),
        }

        {
            let mut query = app.world_mut().query::<&mut Position>();
            let mut position = query
                .get_mut(app.world_mut(), entity)
                .expect("test client position should be mutable");
            position.set([1005.0, 66.0, 1005.0]);
        }

        app.update();
        flush_all_client_packets(&mut app);

        let second_payloads = collect_zone_info_payloads(&mut helper);
        assert_eq!(
            second_payloads.len(),
            1,
            "transition should emit exactly one zone_info payload"
        );

        match &second_payloads[0].payload {
            ServerDataPayloadV1::ZoneInfo {
                zone,
                spirit_qi,
                danger_level,
                status,
                active_events,
                perception_text,
            } => {
                assert_eq!(zone, "blood_valley");
                assert_eq!(*spirit_qi, 0.42);
                assert_eq!(*danger_level, 4);
                assert_eq!(*status, ZoneStatusV1::Normal);
                assert_eq!(active_events, &Some(vec!["beast_tide".to_string()]));
                assert_eq!(
                    perception_text.as_deref(),
                    Some("灵气几近断绝，此地有不祥预感")
                );
            }
            other => panic!("expected zone_info payload, got {other:?}"),
        }

        app.update();
        flush_all_client_packets(&mut app);
        let third_payloads = collect_zone_info_payloads(&mut helper);
        assert!(
            third_payloads.is_empty(),
            "no additional payload should be emitted without a new transition"
        );
    }

    #[test]
    fn emits_zone_info_when_runtime_state_changes_without_transition() {
        let zone_registry = ZoneRegistry {
            spatial_revision: 0,
            zones: vec![Zone {
                name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
                dimension: crate::world::dimension::DimensionKind::Overworld,
                bounds: (DVec3::new(0.0, 64.0, 0.0), DVec3::new(128.0, 128.0, 128.0)),
                spirit_qi: 0.9,
                danger_level: 0,
                active_events: vec![],
                patrol_anchors: vec![DVec3::new(14.0, 66.0, 14.0)],
                blocked_tiles: vec![],
                qi_equilibrium: 0.0,
                qi_inflow_per_min: 0.0,
            }],
        };

        let mut app = setup_zone_transition_app(zone_registry);
        let (_entity, mut helper) =
            spawn_test_client_with_helper(&mut app, "Alice", [8.0, 66.0, 8.0]);

        app.update();
        flush_all_client_packets(&mut app);
        let first_payloads = collect_zone_info_payloads(&mut helper);
        assert_eq!(
            first_payloads.len(),
            1,
            "initial zone snapshot should be sent"
        );

        {
            let mut zone_registry = app.world_mut().resource_mut::<ZoneRegistry>();
            let zone = zone_registry
                .zones
                .iter_mut()
                .find(|zone| zone.name == DEFAULT_SPAWN_ZONE_NAME)
                .expect("spawn zone should exist in test registry");
            zone.spirit_qi = 0.0;
            zone.danger_level = 5;
            zone.active_events = vec![EVENT_REALM_COLLAPSE.to_string()];
        }

        app.update();
        flush_all_client_packets(&mut app);
        let refreshed_payloads = collect_zone_info_payloads(&mut helper);
        assert_eq!(
            refreshed_payloads.len(),
            1,
            "runtime zone state change should emit one zone_info payload without movement"
        );

        match &refreshed_payloads[0].payload {
            ServerDataPayloadV1::ZoneInfo {
                zone,
                spirit_qi,
                danger_level,
                status,
                active_events,
                perception_text,
            } => {
                assert_eq!(zone, DEFAULT_SPAWN_ZONE_NAME);
                assert_eq!(*spirit_qi, 0.0);
                assert_eq!(*danger_level, 5);
                assert_eq!(*status, ZoneStatusV1::Collapsed);
                assert_eq!(active_events, &Some(vec![EVENT_REALM_COLLAPSE.to_string()]));
                assert_eq!(
                    perception_text.as_deref(),
                    Some("灵气几近断绝，此地有不祥预感")
                );
            }
            other => panic!("expected zone_info payload, got {other:?}"),
        }

        app.update();
        flush_all_client_packets(&mut app);
        let stable_payloads = collect_zone_info_payloads(&mut helper);
        assert!(
            stable_payloads.is_empty(),
            "unchanged runtime state should not spam zone_info payloads"
        );
    }

    #[test]
    fn zone_info_marks_realm_collapse_zone_collapsed() {
        let collapsed_zone = Zone {
            name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            dimension: crate::world::dimension::DimensionKind::Overworld,
            bounds: (DVec3::new(0.0, 64.0, 0.0), DVec3::new(128.0, 128.0, 128.0)),
            spirit_qi: 0.0,
            danger_level: 5,
            active_events: vec![EVENT_REALM_COLLAPSE.to_string()],
            patrol_anchors: vec![DVec3::new(14.0, 66.0, 14.0)],
            blocked_tiles: vec![],
            qi_equilibrium: 0.0,
            qi_inflow_per_min: 0.0,
        };
        let mut app = setup_zone_transition_app(ZoneRegistry {
            spatial_revision: 0,
            zones: vec![collapsed_zone],
        });
        let (_entity, mut helper) =
            spawn_test_client_with_helper(&mut app, "Alice", [8.0, 66.0, 8.0]);

        app.update();
        flush_all_client_packets(&mut app);

        let payloads = collect_zone_info_payloads(&mut helper);
        assert_eq!(payloads.len(), 1);
        match &payloads[0].payload {
            ServerDataPayloadV1::ZoneInfo {
                zone,
                status,
                active_events,
                ..
            } => {
                assert_eq!(zone, DEFAULT_SPAWN_ZONE_NAME);
                assert_eq!(*status, ZoneStatusV1::Collapsed);
                assert_eq!(active_events, &Some(vec![EVENT_REALM_COLLAPSE.to_string()]));
            }
            other => panic!("expected zone_info payload, got {other:?}"),
        }
    }

    #[test]
    fn zone_info_marks_tsy_race_out_zone() {
        let race_out_zone = Zone {
            name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            dimension: crate::world::dimension::DimensionKind::Overworld,
            bounds: (DVec3::new(0.0, 64.0, 0.0), DVec3::new(128.0, 128.0, 128.0)),
            spirit_qi: -0.7,
            danger_level: 5,
            active_events: vec![EVENT_TSY_RACE_OUT.to_string()],
            patrol_anchors: vec![DVec3::new(14.0, 66.0, 14.0)],
            blocked_tiles: vec![],
            qi_equilibrium: 0.0,
            qi_inflow_per_min: 0.0,
        };
        let mut app = setup_zone_transition_app(ZoneRegistry {
            spatial_revision: 0,
            zones: vec![race_out_zone],
        });
        let (_entity, mut helper) =
            spawn_test_client_with_helper(&mut app, "Alice", [8.0, 66.0, 8.0]);

        app.update();
        flush_all_client_packets(&mut app);

        let payloads = collect_zone_info_payloads(&mut helper);
        assert_eq!(payloads.len(), 1);
        match &payloads[0].payload {
            ServerDataPayloadV1::ZoneInfo {
                zone,
                status,
                active_events,
                ..
            } => {
                assert_eq!(zone, DEFAULT_SPAWN_ZONE_NAME);
                assert_eq!(*status, ZoneStatusV1::RaceOut);
                assert_eq!(active_events, &Some(vec![EVENT_TSY_RACE_OUT.to_string()]));
            }
            other => panic!("expected zone_info payload, got {other:?}"),
        }
    }
}

mod player_state_payload_tests {
    use super::*;
    use crate::player::state::PlayerState;
    use crate::schema::social::RenownTagV1;
    use crate::schema::world_state::SeasonV1;
    use crate::social::components::{FactionMembership as PlayerFactionMembership, Renown};
    use crate::world::season::{Season, SUMMER_TICKS};
    use crate::world::zone::ZoneRegistry;
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::MockClientHelper;

    fn emit_player_state_payloads_periodically_without_change(
        zone_registry: Option<Res<ZoneRegistry>>,
        clock: Option<Res<CombatClock>>,
        mut tick_counter: valence::prelude::Local<u64>,
        mut clients: Query<PlayerStateEmitQueryItem<'_>, With<Client>>,
    ) {
        *tick_counter += 1;
        if !tick_counter.is_multiple_of(WORLD_STATE_PUBLISH_INTERVAL_TICKS) {
            return;
        }

        let zone_registry = effective_zone_registry(zone_registry.as_deref());
        let tick = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();

        for (
            entity,
            mut client,
            username,
            position,
            _current_dimension,
            player_state,
            cultivation,
            anonymity,
            renown,
            relationships,
            faction_membership,
        ) in &mut clients
        {
            let zone_name = zone_name_for_position(&zone_registry, position.get());
            let social = build_player_social_snapshot(
                tick,
                anonymity,
                renown,
                relationships,
                faction_membership,
            );
            let payload = player_state.server_payload_with_social_and_local_pressure(
                cultivation,
                Some(canonical_player_id(username.0.as_str())),
                zone_name,
                social,
                None,
                None,
            );
            let payload_type = payload_type_label(payload.payload_type());
            let payload_bytes = match serialize_server_data_payload(&payload) {
                Ok(payload) => payload,
                Err(error) => {
                    log_payload_build_error(payload_type, &error);
                    continue;
                }
            };

            send_server_data_payload(&mut client, payload_bytes.as_slice());
            tracing::info!(
                "[bong][network] sent {} {} payload to client entity {entity:?} for `{}` (periodic test seam)",
                SERVER_DATA_CHANNEL,
                payload_type,
                username.0,
            );
        }
    }

    fn setup_player_state_payload_app() -> App {
        let mut app = App::new();
        app.insert_resource(ZoneRegistry::fallback());
        app.add_systems(Update, emit_player_state_payloads);
        app
    }

    fn setup_season_changed_player_state_app() -> App {
        let mut app = App::new();
        let (tx_outbound, _rx_outbound) = crossbeam_channel::unbounded();
        let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded();

        app.insert_resource(ZoneRegistry::fallback());
        app.insert_resource(WorldSeasonState::default());
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.add_event::<SeasonChangedEvent>();
        app.add_systems(Update, publish_season_changed_events);
        app
    }

    fn spawn_test_client_with_helper(
        app: &mut App,
        username: &str,
        position: [f64; 3],
    ) -> (Entity, MockClientHelper) {
        let (mut client_bundle, helper) = create_mock_client(username);
        client_bundle.player.position = Position::new(position);
        let entity = app.world_mut().spawn(client_bundle).id();
        (entity, helper)
    }

    fn flush_all_client_packets(app: &mut App) {
        let world = app.world_mut();
        let mut query = world.query::<&mut Client>();
        for mut client in query.iter_mut(world) {
            client
                .flush_packets()
                .expect("mock client packets should flush successfully");
        }
    }

    fn collect_player_state_payloads(helper: &mut MockClientHelper) -> Vec<ServerDataV1> {
        let mut payloads = Vec::new();

        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };

            if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                continue;
            }

            let payload: ServerDataV1 = serde_json::from_slice(packet.data.0 .0)
                .expect("typed payload should decode as ServerDataV1");

            if matches!(payload.payload, ServerDataPayloadV1::PlayerState { .. }) {
                payloads.push(payload);
            }
        }

        payloads
    }

    #[test]
    fn emits_player_state_on_join_and_change() {
        let mut app = setup_player_state_payload_app();
        let (entity, mut helper) =
            spawn_test_client_with_helper(&mut app, "Azure", [8.0, 66.0, 8.0]);

        app.world_mut().entity_mut(entity).insert((
            PlayerState {
                karma: 0.2,
                inventory_score: 0.4,
            },
            Cultivation {
                realm: Realm::Condense,
                qi_current: 78.0,
                qi_max: 100.0,
                ..Cultivation::default()
            },
        ));

        app.update();
        flush_all_client_packets(&mut app);

        let first_payloads = collect_player_state_payloads(&mut helper);
        assert_eq!(
            first_payloads.len(),
            1,
            "join/attach should emit one player_state payload"
        );

        match &first_payloads[0].payload {
            ServerDataPayloadV1::PlayerState {
                player,
                realm,
                spirit_qi,
                zone,
                season_state,
                ..
            } => {
                assert_eq!(player.as_deref(), Some("offline:Azure"));
                assert_eq!(realm, "Condense");
                assert_eq!(*spirit_qi, 78.0);
                assert_eq!(zone, DEFAULT_SPAWN_ZONE_NAME);
                assert!(
                    season_state.is_some(),
                    "player_state payload should carry SeasonState for indirect client visuals"
                );
            }
            other => panic!("expected player_state payload, got {other:?}"),
        }

        {
            let mut query = app.world_mut().query::<&mut Cultivation>();
            let mut cultivation = query
                .get_mut(app.world_mut(), entity)
                .expect("test client Cultivation should be mutable");
            cultivation.qi_current = 81.0;
        }

        app.update();
        flush_all_client_packets(&mut app);

        let second_payloads = collect_player_state_payloads(&mut helper);
        assert_eq!(
            second_payloads.len(),
            1,
            "PlayerState change should emit exactly one payload"
        );

        match &second_payloads[0].payload {
            ServerDataPayloadV1::PlayerState { spirit_qi, .. } => {
                assert_eq!(*spirit_qi, 81.0);
            }
            other => panic!("expected player_state payload, got {other:?}"),
        }
    }

    #[test]
    fn missing_target_route_player_state_does_not_broadcast_to_all_clients() {
        let mut app = setup_player_state_payload_app();
        let (azure_entity, mut azure_helper) =
            spawn_test_client_with_helper(&mut app, "Azure", [8.0, 66.0, 8.0]);
        let (_bob_entity, mut bob_helper) =
            spawn_test_client_with_helper(&mut app, "Bob", [20.0, 66.0, 20.0]);

        app.world_mut().entity_mut(azure_entity).insert((
            PlayerState {
                karma: 0.2,
                inventory_score: 0.4,
            },
            Cultivation {
                realm: Realm::Condense,
                qi_current: 78.0,
                qi_max: 100.0,
                ..Cultivation::default()
            },
        ));
        app.world_mut().entity_mut(_bob_entity).insert((
            PlayerState {
                karma: 0.0,
                inventory_score: 0.0,
            },
            Cultivation {
                realm: Realm::Awaken,
                qi_current: 0.0,
                qi_max: 100.0,
                ..Cultivation::default()
            },
        ));

        app.update();
        flush_all_client_packets(&mut app);
        let _ = collect_player_state_payloads(&mut azure_helper);
        let _ = collect_player_state_payloads(&mut bob_helper);

        {
            let mut query = app.world_mut().query::<&mut Cultivation>();
            let mut azure_cultivation = query
                .get_mut(app.world_mut(), azure_entity)
                .expect("azure cultivation should be mutable");
            azure_cultivation.qi_current = 81.0;
        }

        app.update();
        flush_all_client_packets(&mut app);

        let azure_payloads = collect_player_state_payloads(&mut azure_helper);
        let bob_payloads = collect_player_state_payloads(&mut bob_helper);

        assert_eq!(
            azure_payloads.len(),
            1,
            "changed target should receive one payload"
        );
        assert!(
            bob_payloads.is_empty(),
            "missing-route/fallthrough must not broadcast player_state to other clients"
        );
    }

    #[test]
    fn emits_self_social_snapshot_in_player_state() {
        let mut app = setup_player_state_payload_app();
        app.insert_resource(CombatClock { tick: 123 });
        let (entity, mut helper) =
            spawn_test_client_with_helper(&mut app, "Azure", [8.0, 66.0, 8.0]);

        app.world_mut().entity_mut(entity).insert((
            PlayerState {
                karma: 0.2,
                inventory_score: 0.4,
            },
            Cultivation {
                realm: Realm::Condense,
                qi_current: 78.0,
                qi_max: 100.0,
                ..Cultivation::default()
            },
            Renown {
                fame: 7,
                notoriety: 12,
                tags: vec![RenownTagV1 {
                    tag: "背盟者".to_string(),
                    weight: 50.0,
                    last_seen_tick: 123,
                    permanent: true,
                }],
            },
            PlayerFactionMembership {
                faction: crate::npc::faction::FactionId::Defend,
                named_faction: None,
                rank: 0,
                loyalty: 10,
                betrayal_count: 1,
                invite_block_until_tick: Some(200),
                permanently_refused: false,
            },
        ));

        app.update();
        flush_all_client_packets(&mut app);

        let payloads = collect_player_state_payloads(&mut helper);
        match &payloads[0].payload {
            ServerDataPayloadV1::PlayerState { social, .. } => {
                let social = social
                    .as_ref()
                    .expect("self social snapshot should be present");
                assert_eq!(social.renown.fame, 7);
                assert_eq!(social.renown.notoriety, 12);
                assert_eq!(social.renown.top_tags[0].tag, "背盟者");
                let membership = social
                    .faction_membership
                    .as_ref()
                    .expect("self faction membership should be present");
                assert_eq!(membership.faction, "defend");
                assert_eq!(membership.rank, 0);
                assert_eq!(membership.loyalty, 10);
                assert_eq!(membership.invite_block_until_tick, Some(200));
            }
            other => panic!("expected player_state payload, got {other:?}"),
        }
    }

    #[test]
    fn player_state_periodic_emission_happens_without_component_change() {
        let mut app = App::new();
        app.insert_resource(ZoneRegistry::fallback());
        app.add_systems(
            Update,
            emit_player_state_payloads_periodically_without_change,
        );

        let (entity, mut helper) =
            spawn_test_client_with_helper(&mut app, "Azure", [8.0, 66.0, 8.0]);
        app.world_mut().entity_mut(entity).insert((
            PlayerState {
                karma: 0.0,
                inventory_score: 0.0,
            },
            Cultivation {
                realm: Realm::Condense,
                qi_current: 78.0,
                qi_max: 100.0,
                ..Cultivation::default()
            },
        ));

        app.update();
        flush_all_client_packets(&mut app);
        let _ = collect_player_state_payloads(&mut helper);

        for _ in 0..(WORLD_STATE_PUBLISH_INTERVAL_TICKS - 1) {
            app.update();
        }
        flush_all_client_packets(&mut app);

        let periodic_payloads = collect_player_state_payloads(&mut helper);
        assert_eq!(
            periodic_payloads.len(),
            1,
            "periodic cadence should emit one player_state payload without Changed<PlayerState>"
        );
    }

    #[test]
    fn season_changed_event_emits_player_state_without_component_change() {
        let mut app = setup_season_changed_player_state_app();
        let (entity, mut helper) =
            spawn_test_client_with_helper(&mut app, "Azure", [8.0, 66.0, 8.0]);
        app.world_mut().entity_mut(entity).insert((
            PlayerState {
                karma: 0.0,
                inventory_score: 0.0,
            },
            Cultivation {
                realm: Realm::Condense,
                qi_current: 100.0,
                qi_max: 100.0,
                composure: 1.0,
                ..Cultivation::default()
            },
        ));

        app.update();
        flush_all_client_packets(&mut app);
        assert!(
            collect_player_state_payloads(&mut helper).is_empty(),
            "idle clients should not receive player_state packets without a season event"
        );

        {
            let mut season_state = app.world_mut().resource_mut::<WorldSeasonState>();
            season_state.current = query_season("", SUMMER_TICKS);
            season_state.last_phase_change_tick = 42;
        }
        app.world_mut().send_event(SeasonChangedEvent {
            from: Season::Summer,
            to: Season::SummerToWinter,
            tick: 42,
        });

        app.update();
        flush_all_client_packets(&mut app);

        let payloads = collect_player_state_payloads(&mut helper);
        assert_eq!(
            payloads.len(),
            1,
            "season boundary should push one player_state payload even without component changes"
        );
        match &payloads[0].payload {
            ServerDataPayloadV1::PlayerState {
                season_state,
                spirit_qi,
                spirit_qi_max,
                ..
            } => {
                let season_state = season_state
                    .as_ref()
                    .expect("season change payload should carry SeasonState");
                assert_eq!(season_state.season, SeasonV1::SummerToWinter);
                assert_eq!(season_state.tick_into_phase, 0);
                assert_eq!(
                    season_state.phase_total_ticks,
                    Season::SummerToWinter.phase_total_ticks()
                );
                assert_eq!(*spirit_qi, 100.0);
                assert_eq!(*spirit_qi_max, 100.0);
            }
            other => panic!("expected player_state payload, got {other:?}"),
        }
    }
}

mod event_payload_tests {
    use super::*;
    use crate::world::events::{
        ActiveEventsResource, EVENT_BEAST_TIDE, EVENT_THUNDER_TRIBULATION,
    };
    use crate::world::zone::ZoneRegistry;
    use std::collections::HashMap;
    use valence::prelude::Events;
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::MockClientHelper;

    fn spawn_event_command(
        target: &str,
        event: &str,
        duration_ticks: u64,
    ) -> crate::schema::agent_command::Command {
        let mut params = HashMap::new();
        params.insert("event".to_string(), serde_json::json!(event));
        params.insert(
            "duration_ticks".to_string(),
            serde_json::json!(duration_ticks),
        );

        crate::schema::agent_command::Command {
            command_type: crate::schema::common::CommandType::SpawnEvent,
            target: target.to_string(),
            params,
        }
    }

    fn setup_event_alert_app() -> App {
        let mut app = App::new();
        app.insert_resource(ActiveEventsResource::default());
        app.insert_resource(ZoneRegistry::fallback());
        app.add_event::<audio_event_emit::PlaySoundRecipeRequest>();
        app.add_systems(Update, emit_event_alerts_on_major_event_creation);
        app
    }

    fn spawn_test_client_with_helper(
        app: &mut App,
        username: &str,
        position: [f64; 3],
    ) -> (Entity, MockClientHelper) {
        let (mut client_bundle, helper) = create_mock_client(username);
        client_bundle.player.position = Position::new(position);
        let entity = app.world_mut().spawn(client_bundle).id();
        (entity, helper)
    }

    fn flush_all_client_packets(app: &mut App) {
        let world = app.world_mut();
        let mut query = world.query::<&mut Client>();
        for mut client in query.iter_mut(world) {
            client
                .flush_packets()
                .expect("mock client packets should flush successfully");
        }
    }

    fn collect_event_alert_payloads(helper: &mut MockClientHelper) -> Vec<ServerDataV1> {
        let mut payloads = Vec::new();

        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };
            if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                continue;
            }

            let payload: ServerDataV1 = serde_json::from_slice(packet.data.0 .0)
                .expect("typed payload should decode as ServerDataV1");

            if matches!(payload.payload, ServerDataPayloadV1::EventAlert { .. }) {
                payloads.push(payload);
            }
        }

        payloads
    }

    #[test]
    fn emits_event_alert_on_major_event() {
        let mut app = setup_event_alert_app();
        let (_entity, mut helper) =
            spawn_test_client_with_helper(&mut app, "Alice", [8.0, 66.0, 8.0]);

        {
            let world = app.world_mut();
            let command = spawn_event_command("spawn", EVENT_THUNDER_TRIBULATION, 180);
            world.resource_scope(|world, mut zones: valence::prelude::Mut<ZoneRegistry>| {
                let mut events = world.resource_mut::<ActiveEventsResource>();
                let accepted = events.enqueue_from_spawn_command(&command, Some(&mut zones));
                assert!(
                    accepted,
                    "thunder major event should be accepted into scheduler"
                );
            });
        }

        app.update();
        flush_all_client_packets(&mut app);

        let payloads = collect_event_alert_payloads(&mut helper);
        assert_eq!(
            payloads.len(),
            1,
            "major event enqueue should emit one event_alert payload"
        );

        match &payloads[0].payload {
            ServerDataPayloadV1::EventAlert {
                event,
                message,
                zone,
                duration_ticks,
            } => {
                assert_eq!(*event, EventKind::ThunderTribulation);
                assert!(message.contains("天劫"));
                assert_eq!(zone.as_deref(), Some("spawn"));
                assert_eq!(*duration_ticks, Some(180));
            }
            other => panic!("expected event_alert payload, got {other:?}"),
        }

        app.update();
        flush_all_client_packets(&mut app);
        let second = collect_event_alert_payloads(&mut helper);
        assert!(
            second.is_empty(),
            "drained major-event alerts must not be resent on subsequent ticks"
        );
    }

    #[test]
    fn locust_swarm_alert_emits_custom_warning_and_audio_request() {
        let mut app = setup_event_alert_app();
        let (entity, mut helper) =
            spawn_test_client_with_helper(&mut app, "Alice", [8.0, 66.0, 8.0]);

        {
            let world = app.world_mut();
            let mut command = spawn_event_command("spawn", EVENT_BEAST_TIDE, 24000);
            command
                .params
                .insert("tide_kind".to_string(), serde_json::json!("locust_swarm"));
            world.resource_scope(|world, mut zones: valence::prelude::Mut<ZoneRegistry>| {
                let mut events = world.resource_mut::<ActiveEventsResource>();
                let accepted = events.enqueue_from_spawn_command(&command, Some(&mut zones));
                assert!(accepted, "locust swarm should be accepted into scheduler");
            });
        }

        app.update();
        flush_all_client_packets(&mut app);

        let has_locust_payload = helper.collect_received().0.into_iter().any(|frame| {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                return false;
            };
            packet.channel.as_str() == "bong:locust_swarm_warning"
        });
        assert!(
            has_locust_payload,
            "locust swarm should emit dedicated warning payload"
        );

        let audio_events = app
            .world()
            .resource::<Events<audio_event_emit::PlaySoundRecipeRequest>>();
        let request = audio_events
            .iter_current_update_events()
            .next()
            .expect("locust swarm warning should queue an audio cue");
        assert_eq!(request.recipe_id, "locust_swarm_warning");
        assert_eq!(
            request.recipient,
            audio_event_emit::AudioRecipient::Single(entity)
        );
    }
}

mod gameplay_tests {
    use super::*;
    use crate::combat::{
        components::{CombatState, DerivedAttrs, Lifecycle, Stamina, StatusEffects, Wounds},
        events::{ApplyStatusEffectIntent, AttackIntent, CombatEvent, DeathEvent},
        CombatClock,
    };
    use crate::cultivation::breakthrough::{breakthrough_system, BreakthroughRequest};
    use crate::cultivation::components::{
        Contamination, Cultivation, MeridianId, MeridianSystem, QiColor, Realm,
    };
    use crate::cultivation::life_record::{LifeRecord, SkillMilestone};
    use crate::cultivation::tick::CultivationClock;
    use crate::persistence::{
        load_agent_world_model_snapshot, persist_agent_world_model_snapshot,
        PersistenceSettings, WORLD_MODEL_STATE_FIELD_CURRENT_ERA,
        WORLD_MODEL_STATE_FIELD_LAST_DECISIONS, WORLD_MODEL_STATE_FIELD_LAST_STATE_TS,
        WORLD_MODEL_STATE_FIELD_LAST_TICK, WORLD_MODEL_STATE_FIELD_NEG_DOMAIN_ESCAPE_SESSIONS,
        WORLD_MODEL_STATE_FIELD_NEG_DOMAIN_ESCAPE_TELEMETRY,
        WORLD_MODEL_STATE_FIELD_NEG_DOMAIN_PENDING_TRIBULATIONS,
        WORLD_MODEL_STATE_FIELD_PLAYER_FIRST_SEEN_TICK, WORLD_MODEL_STATE_FIELD_ZONE_HISTORY,
        WORLD_MODEL_STATE_KEY,
    };
    use crate::player::gameplay::{
        CombatAction, GameplayAction, GameplayActionQueue, GameplayTick, GatherAction,
        PendingGameplayNarrations,
    };
    use crate::qi_physics::{
        constants::QI_GATHER_REWARD, QiAccountId, QiTransferReason, WorldQiAccount,
    };
    use crate::schema::agent_world_model::{
        AgentWorldModelEnvelopeV1, AgentWorldModelSnapshotV1, CurrentEraV1, ZoneHistoryEntryV1,
    };
    use crate::skill::components::SkillId;
    use crate::world::events::ActiveEventsResource;
    use crossbeam_channel::{unbounded, Receiver};
    use std::collections::BTreeMap;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};
    use valence::prelude::Events;
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::MockClientHelper;

    fn unique_temp_dir(test_name: &str) -> std::path::PathBuf {
        let unique_suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();

        std::env::temp_dir().join(format!(
            "bong-network-world-model-{test_name}-{}-{unique_suffix}",
            std::process::id()
        ))
    }

    fn setup_gameplay_app() -> (App, Receiver<RedisOutbound>) {
        let (tx_outbound, rx_outbound) = unbounded();
        let (_tx_inbound, rx_inbound) = unbounded();
        let mut app = App::new();

        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.insert_resource(WorldStateTimer {
            ticks: WORLD_STATE_PUBLISH_INTERVAL_TICKS - 1,
        });
        app.insert_resource(ZoneRegistry::fallback());
        app.insert_resource(ActiveEventsResource::default());
        app.insert_resource(GameplayActionQueue::default());
        app.insert_resource(PendingGameplayNarrations::default());
        app.insert_resource(GameplayTick::default());
        app.insert_resource(WorldQiAccount::default());
        app.insert_resource(CombatClock::default());
        app.insert_resource(CultivationClock::default());
        app.add_event::<AttackIntent>();
        app.add_event::<BreakthroughRequest>();
        app.add_event::<crate::cultivation::breakthrough::BreakthroughOutcome>();
        app.add_event::<crate::cultivation::death_hooks::CultivationDeathTrigger>();
        app.add_event::<crate::network::vfx_event_emit::VfxEventRequest>();
        app.add_event::<crate::skill::events::SkillCapChanged>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<crate::combat::weapon::ShieldBlockHit>();
        app.add_event::<crate::inventory::InventoryDurabilityChangedEvent>();
        app.add_systems(
            Update,
            (
                crate::combat::debug::tick_combat_clock,
                crate::player::gameplay::apply_queued_gameplay_actions
                    .after(crate::combat::debug::tick_combat_clock),
                breakthrough_system
                    .after(crate::player::gameplay::apply_queued_gameplay_actions),
                crate::combat::status::status_effect_apply_tick
                    .after(crate::player::gameplay::apply_queued_gameplay_actions),
                crate::combat::status::attribute_aggregate_tick
                    .after(crate::combat::status::status_effect_apply_tick),
                crate::combat::resolve::resolve_attack_intents
                    .after(crate::player::gameplay::apply_queued_gameplay_actions),
                emit_gameplay_narrations.after(crate::combat::resolve::resolve_attack_intents),
                emit_player_state_payloads
                    .after(crate::player::gameplay::apply_queued_gameplay_actions),
                publish_world_state_to_redis
                    .after(crate::combat::resolve::resolve_attack_intents),
            ),
        );

        (app, rx_outbound)
    }

    fn spawn_test_client_with_state(
        app: &mut App,
        username: &str,
        position: [f64; 3],
        player_state: PlayerState,
        cultivation: Cultivation,
    ) -> (Entity, MockClientHelper) {
        let (mut client_bundle, helper) = create_mock_client(username);
        client_bundle.player.position = Position::new(position);
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                cultivation,
                player_state,
                Wounds::default(),
                Stamina::default(),
                CombatState::default(),
                StatusEffects::default(),
                DerivedAttrs::default(),
                Lifecycle {
                    character_id: canonical_player_id(username),
                    ..Default::default()
                },
                Contamination::default(),
                MeridianSystem::default(),
                QiColor::default(),
                LifeRecord::new(canonical_player_id(username)),
            ))
            .id();
        (entity, helper)
    }

    fn flush_all_client_packets(app: &mut App) {
        let world = app.world_mut();
        let mut query = world.query::<&mut Client>();

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
                serde_json::from_slice(packet.data.0 .0)
                    .expect("typed payload should decode as ServerDataV1"),
            );
        }

        payloads
    }

    fn extract_player_state_payloads(payloads: &[ServerDataV1]) -> Vec<&ServerDataV1> {
        payloads
            .iter()
            .filter(|payload| {
                matches!(payload.payload, ServerDataPayloadV1::PlayerState { .. })
            })
            .collect()
    }

    fn extract_narration_payloads(payloads: &[ServerDataV1]) -> Vec<&ServerDataV1> {
        payloads
            .iter()
            .filter(|payload| matches!(payload.payload, ServerDataPayloadV1::Narration { .. }))
            .collect()
    }

    fn dequeue_world_state(rx_outbound: &Receiver<RedisOutbound>) -> WorldStateV1 {
        match rx_outbound
            .try_recv()
            .expect("world state publish should enqueue a Redis outbound message")
        {
            RedisOutbound::WorldState(state) => state,
            other => panic!("expected world-state publish, got {other:?}"),
        }
    }

    #[test]
    fn world_state_includes_structured_skill_milestones_in_life_record_snapshot() {
        let (mut app, rx_outbound) = setup_gameplay_app();
        let (entity, _helper) = spawn_test_client_with_state(
            &mut app,
            "Azure",
            [8.0, 66.0, 8.0],
            PlayerState {
                karma: 0.1,
                inventory_score: 0.2,
            },
            Cultivation {
                realm: Realm::Induce,
                qi_current: 50.0,
                qi_max: 100.0,
                ..Cultivation::default()
            },
        );
        app.world_mut().entity_mut(entity).insert(LifeRecord {
            character_id: canonical_player_id("Azure"),
            created_at: 0,
            biography: Vec::new(),
            insights_taken: Vec::new(),
            death_insights: Vec::new(),
            skill_milestones: vec![SkillMilestone {
                skill: SkillId::Herbalism,
                new_lv: 3,
                achieved_at: 82000,
                narration: "你摘辨草木渐熟，今已至Lv.3。".to_string(),
                total_xp_at: 550,
            }],
            spirit_root_first: None,
            ..LifeRecord::default()
        });

        app.update();

        let world_state = dequeue_world_state(&rx_outbound);
        let player = world_state
            .players
            .iter()
            .find(|player| player.uuid == canonical_player_id("Azure"))
            .expect("Azure should be included in world state");
        let life_record = player
            .life_record
            .as_ref()
            .expect("life_record snapshot should be present");

        assert_eq!(life_record.skill_milestones.len(), 1);
        assert_eq!(life_record.skill_milestones[0].skill, "herbalism");
        assert_eq!(life_record.skill_milestones[0].new_lv, 3);
        assert_eq!(life_record.skill_milestones[0].achieved_at, 82000);
        assert_eq!(life_record.skill_milestones[0].total_xp_at, 550);
        assert_eq!(
            life_record.skill_milestones[0].narration,
            "你摘辨草木渐熟，今已至Lv.3。"
        );
    }

    #[test]
    fn agent_state_sqlite_authority_survives_redis_restart() {
        let root = unique_temp_dir("sqlite-authority-survives-redis-restart");
        let db_path = root.join("data").join("bong.db");
        let settings = PersistenceSettings::with_db_path(
            &db_path,
            "agent_state_sqlite_authority_survives_redis_restart",
        );

        crate::persistence::bootstrap_sqlite(settings.db_path(), settings.server_run_id())
            .expect("bootstrap should succeed");

        let snapshot = crate::persistence::AgentWorldModelSnapshotRecord {
            current_era: Some(serde_json::json!({
                "name": "末法纪",
                "sinceTick": 188,
                "globalEffect": "灵机渐枯"
            })),
            zone_history: BTreeMap::from([(
                "blood_valley".to_string(),
                vec![serde_json::json!({
                    "name": "blood_valley",
                    "spirit_qi": 0.45,
                    "danger_level": 2,
                    "active_events": ["tribulation"],
                    "player_count": 3
                })],
            )]),
            last_decisions: BTreeMap::new(),
            player_first_seen_tick: BTreeMap::from([("offline:test-player".to_string(), 188)]),
            last_tick: Some(188),
            last_state_ts: Some(1_711_111_188),
            ..Default::default()
        };

        persist_agent_world_model_snapshot(&settings, &snapshot)
            .expect("sqlite authority persist should succeed");

        let loaded = load_agent_world_model_snapshot(&settings)
            .expect("sqlite authority load should succeed")
            .expect("world model snapshot should exist");
        assert_eq!(loaded, snapshot);

        let mirror_fields = crate::persistence::world_model_snapshot_to_mirror_fields(&loaded)
            .expect("mirror field projection should succeed");
        assert_eq!(
            mirror_fields.get(WORLD_MODEL_STATE_FIELD_LAST_TICK),
            Some(&"188".to_string())
        );
        assert_eq!(
            mirror_fields.get(WORLD_MODEL_STATE_FIELD_LAST_STATE_TS),
            Some(&"1711111188".to_string())
        );

        let required_fields = [
            WORLD_MODEL_STATE_FIELD_CURRENT_ERA,
            WORLD_MODEL_STATE_FIELD_ZONE_HISTORY,
            WORLD_MODEL_STATE_FIELD_LAST_DECISIONS,
            WORLD_MODEL_STATE_FIELD_PLAYER_FIRST_SEEN_TICK,
            WORLD_MODEL_STATE_FIELD_NEG_DOMAIN_PENDING_TRIBULATIONS,
            WORLD_MODEL_STATE_FIELD_NEG_DOMAIN_ESCAPE_TELEMETRY,
            WORLD_MODEL_STATE_FIELD_NEG_DOMAIN_ESCAPE_SESSIONS,
            WORLD_MODEL_STATE_FIELD_LAST_TICK,
            WORLD_MODEL_STATE_FIELD_LAST_STATE_TS,
        ];
        for field in required_fields {
            assert!(
                mirror_fields.contains_key(field),
                "runtime mirror should include required field {field}"
            );
        }
        assert_eq!(WORLD_MODEL_STATE_KEY, "bong:tiandao:state");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn bootstrap_world_model_runtime_mirror_loads_sqlite_snapshot_before_mirror_write() {
        let root = unique_temp_dir("runtime-mirror-bootstrap-loads-sqlite-snapshot");
        let db_path = root.join("data").join("bong.db");
        let settings = PersistenceSettings::with_db_path(
            &db_path,
            "runtime_mirror_bootstrap_loads_sqlite_snapshot",
        );

        crate::persistence::bootstrap_sqlite(settings.db_path(), settings.server_run_id())
            .expect("bootstrap should succeed");

        let snapshot = crate::persistence::AgentWorldModelSnapshotRecord {
            current_era: Some(serde_json::json!({
                "name": "赤月纪",
                "sinceTick": 256,
                "globalEffect": "灵潮倒卷"
            })),
            zone_history: BTreeMap::from([(
                "red_marsh".to_string(),
                vec![serde_json::json!({
                    "name": "red_marsh",
                    "spirit_qi": 0.62,
                    "danger_level": 3,
                    "active_events": ["blood_moon"],
                    "player_count": 2
                })],
            )]),
            last_decisions: BTreeMap::new(),
            player_first_seen_tick: BTreeMap::from([("offline:azure".to_string(), 256)]),
            last_tick: Some(256),
            last_state_ts: Some(1_711_333_256),
            ..Default::default()
        };

        persist_agent_world_model_snapshot(&settings, &snapshot)
            .expect("sqlite authority persist should succeed before bootstrap");

        let bootstrapped = bootstrap_agent_world_model_mirror(&settings)
            .expect("runtime mirror bootstrap load should succeed")
            .expect("runtime mirror bootstrap should return sqlite snapshot");
        assert_eq!(bootstrapped, snapshot);

        let mirror_fields =
            crate::persistence::world_model_snapshot_to_mirror_fields(&bootstrapped)
                .expect("bootstrapped snapshot should project to mirror fields");
        assert_eq!(
            mirror_fields.get(WORLD_MODEL_STATE_FIELD_LAST_TICK),
            Some(&"256".to_string())
        );
        assert_eq!(
            mirror_fields.get(WORLD_MODEL_STATE_FIELD_LAST_STATE_TS),
            Some(&"1711333256".to_string())
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn agent_world_model_ingress_persists_sqlite_without_runtime_mirror_config() {
        let root = unique_temp_dir("agent-world-model-ingress-without-runtime-mirror");
        let db_path = root.join("data").join("bong.db");
        let settings = PersistenceSettings::with_db_path(
            &db_path,
            "agent_world_model_ingress_without_runtime_mirror",
        );

        crate::persistence::bootstrap_sqlite(settings.db_path(), settings.server_run_id())
            .expect("bootstrap should succeed");

        let envelope = AgentWorldModelEnvelopeV1 {
            v: 1,
            id: "wm-ingress-1".to_string(),
            source: Some("arbiter".to_string()),
            snapshot: AgentWorldModelSnapshotV1 {
                current_era: Some(CurrentEraV1 {
                    name: "赤月纪".to_string(),
                    since_tick: 512,
                    global_effect: "灵潮倒卷".to_string(),
                }),
                zone_history: HashMap::from([(
                    "red_marsh".to_string(),
                    vec![ZoneHistoryEntryV1 {
                        name: "red_marsh".to_string(),
                        spirit_qi: 0.62,
                        danger_level: 3,
                        active_events: vec!["blood_moon".to_string()],
                        player_count: 2,
                    }],
                )]),
                last_decisions: BTreeMap::new(),
                player_first_seen_tick: BTreeMap::from([("offline:azure".to_string(), 512)]),
                // fix/world-model-schema-drift：neg_domain 三字段自诞生 commit
                // 起就没有 Rust 侧持久化链路，这里用非空数据驱动
                // ingress -> sqlite -> load 全链路验证不再被静默丢弃。
                neg_domain_pending_tribulations: BTreeMap::from([(
                    "offline:azure".to_string(),
                    crate::schema::agent_world_model::NegDomainPendingTribulationV1 {
                        player_uuid: "offline:azure".to_string(),
                        player_name: "Azure".to_string(),
                        zone: "red_marsh".to_string(),
                        entered_at_tick: 500,
                        last_suppressed_tick: 510,
                        reason: "negative_domain_tribulation_exempt".to_string(),
                    },
                )]),
                neg_domain_escape_telemetry:
                    crate::schema::agent_world_model::NegDomainEscapeTelemetryV1 {
                        escape_entry_count: 3,
                        post_escape_realm_drop_count: 1,
                        successful_tribulation_avoidance_count: 2,
                        active_escape_session_count: 1,
                        post_escape_realm_drop_rate: 1.0 / 3.0,
                    },
                neg_domain_escape_sessions: BTreeMap::from([(
                    "offline:azure".to_string(),
                    crate::schema::agent_world_model::NegDomainEscapeSessionV1 {
                        player_uuid: "offline:azure".to_string(),
                        player_name: "Azure".to_string(),
                        zone: "red_marsh".to_string(),
                        entered_at_tick: 500,
                        entry_realm_rank: 3.0,
                    },
                )]),
                last_tick: Some(512),
                last_state_ts: Some(1_711_444_512),
            },
        };

        process_agent_world_model_envelope(Some(&settings), None, &envelope);

        let loaded = load_agent_world_model_snapshot(&settings)
            .expect("sqlite authority load should succeed")
            .expect("ingress should persist snapshot even without runtime mirror config");
        let expected = agent_world_model_snapshot_from_wire(&envelope.snapshot);

        assert_eq!(loaded, expected);
        assert_eq!(loaded.last_tick, Some(512));
        assert_eq!(loaded.last_state_ts, Some(1_711_444_512));
        assert_eq!(
            loaded.current_era,
            Some(serde_json::json!({
                "name": "赤月纪",
                "since_tick": 512,
                "global_effect": "灵潮倒卷"
            }))
        );
        assert_eq!(
            loaded.zone_history.get("red_marsh").map(Vec::len),
            Some(1),
            "zone history should persist through ingress without runtime mirror config"
        );

        let pending = loaded
            .neg_domain_pending_tribulations
            .get("offline:azure")
            .expect("neg_domain_pending_tribulations should survive sqlite ingress roundtrip");
        assert_eq!(pending.zone, "red_marsh");
        assert_eq!(pending.entered_at_tick, 500);
        assert_eq!(pending.reason, "negative_domain_tribulation_exempt");

        assert_eq!(loaded.neg_domain_escape_telemetry.escape_entry_count, 3);
        assert_eq!(
            loaded
                .neg_domain_escape_telemetry
                .post_escape_realm_drop_count,
            1
        );

        let session = loaded
            .neg_domain_escape_sessions
            .get("offline:azure")
            .expect("neg_domain_escape_sessions should survive sqlite ingress roundtrip");
        assert_eq!(session.entry_realm_rank, 3.0);

        let mirror_fields = crate::persistence::world_model_snapshot_to_mirror_fields(&loaded)
            .expect("mirror field projection should succeed for neg_domain fields");
        for field in [
            crate::persistence::WORLD_MODEL_STATE_FIELD_NEG_DOMAIN_PENDING_TRIBULATIONS,
            crate::persistence::WORLD_MODEL_STATE_FIELD_NEG_DOMAIN_ESCAPE_TELEMETRY,
            crate::persistence::WORLD_MODEL_STATE_FIELD_NEG_DOMAIN_ESCAPE_SESSIONS,
        ] {
            assert!(
                mirror_fields.contains_key(field),
                "runtime mirror should include neg_domain field {field}"
            );
        }
        assert!(
            mirror_fields
                [crate::persistence::WORLD_MODEL_STATE_FIELD_NEG_DOMAIN_PENDING_TRIBULATIONS]
                .contains("offline:azure"),
            "mirror neg_domain_pending_tribulations JSON should contain the persisted player"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn agent_world_model_ingress_persists_append_only_rows_without_runtime_mirror_config() {
        let root = unique_temp_dir("agent-world-model-ingress-append-only-no-mirror");
        let db_path = root.join("data").join("bong.db");
        let settings = PersistenceSettings::with_db_path(
            &db_path,
            "agent_world_model_ingress_append_only_without_runtime_mirror",
        );

        crate::persistence::bootstrap_sqlite(settings.db_path(), settings.server_run_id())
            .expect("bootstrap should succeed");

        let envelope = AgentWorldModelEnvelopeV1 {
            v: 1,
            id: "wm-ingress-append-only-1".to_string(),
            source: Some("arbiter".to_string()),
            snapshot: AgentWorldModelSnapshotV1 {
                current_era: Some(CurrentEraV1 {
                    name: "霜烬纪".to_string(),
                    since_tick: 640,
                    global_effect: "镜海回响".to_string(),
                }),
                zone_history: HashMap::from([(
                    "frost_marsh".to_string(),
                    vec![ZoneHistoryEntryV1 {
                        name: "frost_marsh".to_string(),
                        spirit_qi: 0.71,
                        danger_level: 4,
                        active_events: vec!["frost_tide".to_string()],
                        player_count: 1,
                    }],
                )]),
                last_decisions: BTreeMap::from([(
                    "era".to_string(),
                    crate::schema::agent_world_model::AgentWorldModelDecisionV1 {
                        commands: Vec::new(),
                        narrations: Vec::new(),
                        reasoning: "append-only authority ingress should persist era rows"
                            .to_string(),
                    },
                )]),
                player_first_seen_tick: BTreeMap::from([("offline:azure".to_string(), 640)]),
                neg_domain_pending_tribulations: BTreeMap::new(),
                neg_domain_escape_telemetry:
                    crate::schema::agent_world_model::NegDomainEscapeTelemetryV1::default(),
                neg_domain_escape_sessions: BTreeMap::new(),
                last_tick: Some(640),
                last_state_ts: Some(1_711_555_640),
            },
        };

        process_agent_world_model_envelope(Some(&settings), None, &envelope);

        let eras = load_agent_eras(&settings).expect("agent eras should load after ingress");
        assert_eq!(eras.len(), 1);
        assert_eq!(eras[0].envelope_id, envelope.id);
        assert_eq!(eras[0].source, "arbiter");
        assert_eq!(eras[0].era_name, "霜烬纪");

        let decisions =
            load_agent_decisions(&settings).expect("agent decisions should load after ingress");
        assert_eq!(decisions.len(), 1);
        assert_eq!(decisions[0].envelope_id, envelope.id);
        assert_eq!(decisions[0].agent_name, "era");
        assert_eq!(decisions[0].command_count, 0);
        assert_eq!(decisions[0].narration_count, 0);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn agent_publish_failure_does_not_roll_back_sqlite() {
        let root = unique_temp_dir("publish-failure-does-not-rollback-sqlite");
        let db_path = root.join("data").join("bong.db");
        let settings = PersistenceSettings::with_db_path(
            &db_path,
            "agent_publish_failure_does_not_roll_back_sqlite",
        );

        crate::persistence::bootstrap_sqlite(settings.db_path(), settings.server_run_id())
            .expect("bootstrap should succeed");

        let snapshot = crate::persistence::AgentWorldModelSnapshotRecord {
            current_era: Some(serde_json::json!({
                "name": "末法纪",
                "sinceTick": 200,
                "globalEffect": "灵机渐枯"
            })),
            zone_history: BTreeMap::from([(
                "starter_zone".to_string(),
                vec![serde_json::json!({
                    "name": "starter_zone",
                    "spirit_qi": 0.52,
                    "danger_level": 1,
                    "active_events": [],
                    "player_count": 1
                })],
            )]),
            last_decisions: BTreeMap::new(),
            player_first_seen_tick: BTreeMap::from([("offline:alpha".to_string(), 200)]),
            last_tick: Some(200),
            last_state_ts: Some(1_711_222_200),
            ..Default::default()
        };

        persist_agent_world_model_snapshot(&settings, &snapshot)
            .expect("sqlite authority persist should succeed before redis mirror attempt");

        let redis_config = RuntimeMirrorRedisConfig::new("redis://127.0.0.1:1".to_string())
            .expect("test redis config should construct");
        let mirror_result = write_world_model_runtime_mirror(&redis_config, Some(&snapshot));
        assert!(
            mirror_result.is_err(),
            "redis mirror write should fail on unreachable endpoint"
        );

        let loaded = load_agent_world_model_snapshot(&settings)
            .expect("sqlite authority load should succeed even after mirror failure")
            .expect("world model snapshot should still exist");
        assert_eq!(loaded, snapshot);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn world_model_runtime_mirror_reconcile_waits_for_five_minute_interval() {
        let mut state = WorldModelMirrorReconcileState::default();

        for _ in 0..(WORLD_MODEL_RUNTIME_MIRROR_RECONCILE_INTERVAL_TICKS - 1) {
            assert!(
                !should_run_world_model_runtime_mirror_reconcile(&mut state),
                "reconcile should not run before the five-minute cadence elapses"
            );
        }

        assert!(
            should_run_world_model_runtime_mirror_reconcile(&mut state),
            "reconcile should run exactly when the five-minute cadence elapses"
        );
        assert_eq!(state.ticks_since_last_reconcile, 0);
        assert!(
            !should_run_world_model_runtime_mirror_reconcile(&mut state),
            "reconcile cadence should reset after a successful run"
        );
    }

    #[test]
    fn reconcile_world_model_runtime_mirror_loads_sqlite_snapshot_before_writer() {
        let root = unique_temp_dir("runtime-mirror-reconcile-loads-sqlite-snapshot");
        let db_path = root.join("data").join("bong.db");
        let settings = PersistenceSettings::with_db_path(
            &db_path,
            "runtime_mirror_reconcile_loads_sqlite_snapshot",
        );

        crate::persistence::bootstrap_sqlite(settings.db_path(), settings.server_run_id())
            .expect("bootstrap should succeed");

        let snapshot = crate::persistence::AgentWorldModelSnapshotRecord {
            current_era: Some(serde_json::json!({
                "name": "霜烬纪",
                "sinceTick": 640,
                "globalEffect": "镜海回响"
            })),
            zone_history: BTreeMap::from([(
                "frost_marsh".to_string(),
                vec![serde_json::json!({
                    "name": "frost_marsh",
                    "spirit_qi": 0.71,
                    "danger_level": 4,
                    "active_events": ["frost_tide"],
                    "player_count": 1
                })],
            )]),
            last_decisions: BTreeMap::new(),
            player_first_seen_tick: BTreeMap::from([("offline:azure".to_string(), 640)]),
            last_tick: Some(640),
            last_state_ts: Some(1_711_555_640),
            ..Default::default()
        };
        persist_agent_world_model_snapshot(&settings, &snapshot)
            .expect("sqlite authority persist should succeed before reconcile");

        let mut captured = None;
        reconcile_world_model_runtime_mirror_with_writer(&settings, |loaded| {
            captured = loaded.cloned();
            Ok(())
        })
        .expect("reconcile helper should load sqlite authority before writer invocation");

        assert_eq!(captured, Some(snapshot));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn reconcile_world_model_runtime_mirror_passes_none_when_sqlite_is_empty() {
        let root = unique_temp_dir("runtime-mirror-reconcile-empty-sqlite");
        let db_path = root.join("data").join("bong.db");
        let settings = PersistenceSettings::with_db_path(
            &db_path,
            "runtime_mirror_reconcile_empty_sqlite",
        );

        crate::persistence::bootstrap_sqlite(settings.db_path(), settings.server_run_id())
            .expect("bootstrap should succeed");

        let mut saw_none = false;
        reconcile_world_model_runtime_mirror_with_writer(&settings, |loaded| {
            saw_none = loaded.is_none();
            Ok(())
        })
        .expect("reconcile helper should succeed when sqlite authority is empty");

        assert!(
            saw_none,
            "reconcile should ask the mirror writer to clear stale state when sqlite has no snapshot"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn combat_routes_debug_attack_through_resolver() {
        let (mut app, rx_outbound) = setup_gameplay_app();
        let (attacker, _attacker_helper) = spawn_test_client_with_state(
            &mut app,
            "Azure",
            [8.0, 66.0, 8.0],
            PlayerState {
                karma: 0.05,
                inventory_score: 0.10,
            },
            Cultivation {
                realm: Realm::Induce,
                qi_current: 70.0,
                qi_max: 100.0,
                ..Cultivation::default()
            },
        );
        let (target, _target_helper) = spawn_test_client_with_state(
            &mut app,
            "Crimson",
            [9.0, 66.0, 8.0],
            PlayerState {
                karma: 0.0,
                inventory_score: 0.05,
            },
            Cultivation {
                realm: Realm::Induce,
                qi_current: 65.0,
                qi_max: 100.0,
                ..Cultivation::default()
            },
        );

        let mut target_meridians = MeridianSystem::default();
        target_meridians.get_mut(MeridianId::Lung).opened = true;
        app.world_mut().entity_mut(target).insert((
            Wounds {
                entries: Vec::new(),
                health_current: 8.0,
                health_max: 100.0,
            },
            target_meridians,
        ));

        app.world_mut()
            .resource_mut::<GameplayActionQueue>()
            .enqueue(
                "Azure",
                GameplayAction::Combat(CombatAction {
                    target: "Crimson".to_string(),
                    qi_invest: 40.0,
                }),
            );

        app.update();
        flush_all_client_packets(&mut app);

        let world_state = dequeue_world_state(&rx_outbound);
        assert_eq!(world_state.recent_events.len(), 1);
        assert_eq!(
            world_state.recent_events[0].event_type,
            crate::schema::common::GameEventType::EventTriggered
        );

        let expected_target_id = canonical_player_id("Crimson");
        assert_eq!(
            world_state.recent_events[0].target.as_deref(),
            Some(expected_target_id.as_str())
        );

        {
            let world = app.world_mut();
            let wounds = world
                .entity(target)
                .get::<Wounds>()
                .expect("target should keep combat wounds after resolver");
            let stamina = world
                .entity(target)
                .get::<Stamina>()
                .expect("target should keep stamina after resolver");
            let contamination = world
                .entity(target)
                .get::<Contamination>()
                .expect("target should keep contamination after resolver");
            let meridians = world
                .entity(target)
                .get::<MeridianSystem>()
                .expect("target should keep meridians after resolver");

            assert!(wounds.health_current <= 0.0);
            assert_eq!(wounds.entries.len(), 1);
            assert!(stamina.current < stamina.max);
            let expected_attacker_id = canonical_player_id("Azure");
            assert_eq!(
                contamination.entries[0].attacker_id.as_deref(),
                Some(expected_attacker_id.as_str())
            );
            assert!(meridians.get(MeridianId::Lung).throughput_current > 0.0);
        }

        let combat_events = app.world().resource::<Events<CombatEvent>>();
        let death_events = app.world().resource::<Events<DeathEvent>>();
        assert!(
            !combat_events.is_empty(),
            "combat should emit CombatEvent via resolver"
        );
        assert!(
            !death_events.is_empty(),
            "lethal debug combat should emit DeathEvent"
        );

        let attacker_state = app
            .world()
            .entity(attacker)
            .get::<PlayerState>()
            .expect("attacker player state should remain attached");
        assert_eq!(
            attacker_state,
            &PlayerState {
                karma: 0.05,
                inventory_score: 0.10,
            },
            "attacker PlayerState should not be mutated by combat"
        );
        let attacker_cultivation = app
            .world()
            .entity(attacker)
            .get::<crate::cultivation::components::Cultivation>()
            .expect("attacker cultivation should be present for qi-backed combat");
        assert_eq!(attacker_cultivation.qi_current, 30.0);
    }

    #[test]
    fn gathering_grants_experience() {
        let (mut app, rx_outbound) = setup_gameplay_app();
        let (entity, mut helper) = spawn_test_client_with_state(
            &mut app,
            "Gatherer",
            [8.0, 66.0, 8.0],
            PlayerState {
                karma: 0.0,
                inventory_score: 0.0,
            },
            Cultivation {
                realm: Realm::Awaken,
                qi_current: 20.0,
                qi_max: 100.0,
                ..Cultivation::default()
            },
        );

        app.world_mut()
            .resource_mut::<GameplayActionQueue>()
            .enqueue(
                "Gatherer",
                GameplayAction::Gather(GatherAction {
                    resource: "spirit_herb".to_string(),
                    target_entity: None,
                    mode: None,
                }),
            );

        app.update();
        flush_all_client_packets(&mut app);

        let payloads = collect_server_data_payloads(&mut helper);
        let player_state_payloads = extract_player_state_payloads(payloads.as_slice());
        let narration_payloads = extract_narration_payloads(payloads.as_slice());
        assert_eq!(
            player_state_payloads.len(),
            1,
            "gathering should emit one player_state payload"
        );
        assert_eq!(
            narration_payloads.len(),
            1,
            "gathering should emit one narration payload"
        );

        match &player_state_payloads[0].payload {
            ServerDataPayloadV1::PlayerState {
                spirit_qi,
                karma,
                zone,
                ..
            } => {
                assert_eq!(*spirit_qi, 34.0);
                assert_eq!(*karma, 0.06);
                assert_eq!(zone, DEFAULT_SPAWN_ZONE_NAME);
            }
            other => panic!("expected player_state payload, got {other:?}"),
        }

        let world_state = dequeue_world_state(&rx_outbound);
        assert_eq!(world_state.recent_events.len(), 1);
        assert_eq!(
            world_state.recent_events[0].event_type,
            crate::schema::common::GameEventType::ZoneQiChange
        );
        assert_eq!(
            world_state.recent_events[0].target.as_deref(),
            Some("spirit_herb")
        );

        let qi_ledger = app.world().resource::<WorldQiAccount>();
        let transfer = qi_ledger
            .transfers()
            .last()
            .expect("gathering must record zone-to-player QiTransfer audit");
        assert_eq!(
            transfer.from,
            QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME.to_string())
        );
        assert_eq!(transfer.to, QiAccountId::player("offline:Gatherer"));
        assert_eq!(transfer.amount, QI_GATHER_REWARD);
        assert_eq!(transfer.reason, QiTransferReason::CultivationRegen);
        assert_eq!(
            qi_ledger.total(),
            0.0,
            "gathering audit must not mirror live state balances into WorldQiAccount"
        );

        {
            let world = app.world_mut();
            let player_state = world
                .entity(entity)
                .get::<PlayerState>()
                .expect("player state should remain attached after gathering");
            assert_approx_eq(player_state.inventory_score, 0.12);
            assert_approx_eq(player_state.karma, 0.06);

            let cultivation = world
                .entity(entity)
                .get::<Cultivation>()
                .expect("cultivation should remain attached after gathering");
            assert_approx_eq(cultivation.qi_current, 34.0);
        }
    }

    #[test]
    fn realm_breakthrough_updates_payloads() {
        let (mut app, _rx_outbound) = setup_gameplay_app();
        let initial_state = PlayerState {
            karma: -0.9,
            inventory_score: 0.05,
        };
        let (entity, mut helper) = spawn_test_client_with_state(
            &mut app,
            "Seeker",
            [8.0, 66.0, 8.0],
            initial_state.clone(),
            Cultivation {
                realm: Realm::Awaken,
                qi_current: 80.0,
                qi_max: 100.0,
                ..Cultivation::default()
            },
        );

        // Drain the baseline payload emitted on initial attach.
        app.update();
        flush_all_client_packets(&mut app);
        let _ = collect_server_data_payloads(&mut helper);

        // 引气正典门槛：3 条正经。
        let mut meridians = MeridianSystem::default();
        for id in MeridianId::REGULAR.iter().take(3) {
            meridians.get_mut(*id).opened = true;
        }
        app.world_mut().entity_mut(entity).insert(meridians);

        app.world_mut()
            .resource_mut::<GameplayActionQueue>()
            .enqueue("Seeker", GameplayAction::AttemptBreakthrough);

        app.update();
        flush_all_client_packets(&mut app);

        // Flush one more tick so the post-breakthrough Cultivation change is observed
        // by `emit_player_state_payloads` (which runs earlier in the Update chain).
        app.update();
        flush_all_client_packets(&mut app);

        let cultivation = app
            .world()
            .entity(entity)
            .get::<Cultivation>()
            .expect("cultivation should remain attached after breakthrough");
        assert_eq!(cultivation.realm, Realm::Induce);
        assert_approx_eq(cultivation.qi_current, 72.0);
        assert_approx_eq(cultivation.qi_max, 200.0);

        let player_state = app
            .world()
            .entity(entity)
            .get::<PlayerState>()
            .expect("player state should remain attached after bridge");
        assert_eq!(player_state, &initial_state);

        let payloads = collect_server_data_payloads(&mut helper);
        let player_state_payloads = extract_player_state_payloads(payloads.as_slice());
        assert_eq!(
            player_state_payloads.len(),
            1,
            "breakthrough should emit one updated player_state payload"
        );
        match &player_state_payloads[0].payload {
            ServerDataPayloadV1::PlayerState {
                realm, spirit_qi, ..
            } => {
                assert_eq!(realm, "Induce");
                assert_approx_eq(*spirit_qi, 72.0);
            }
            other => panic!("expected player_state payload, got {other:?}"),
        }
    }

    #[test]
    fn realm_breakthrough_rejects_invalid_karma_without_side_effects() {
        let (mut app, _rx_outbound) = setup_gameplay_app();
        let initial_state = PlayerState {
            karma: -0.9,
            inventory_score: 0.05,
        };
        let (entity, mut helper) = spawn_test_client_with_state(
            &mut app,
            "Ascetic",
            [8.0, 66.0, 8.0],
            initial_state.clone(),
            Cultivation {
                realm: Realm::Awaken,
                qi_current: 80.0,
                qi_max: 100.0,
                ..Cultivation::default()
            },
        );

        // Drain the baseline payload emitted on initial attach.
        app.update();
        flush_all_client_packets(&mut app);
        let _ = collect_server_data_payloads(&mut helper);

        // Keep meridians closed so the cultivation breakthrough rejects the attempt.
        app.world_mut()
            .resource_mut::<GameplayActionQueue>()
            .enqueue("offline:Ascetic", GameplayAction::AttemptBreakthrough);

        app.update();
        flush_all_client_packets(&mut app);

        let outcomes = app
            .world()
            .resource::<Events<crate::cultivation::breakthrough::BreakthroughOutcome>>();
        assert!(
            !outcomes.is_empty(),
            "breakthrough forwarding should emit an outcome even when requirements are unmet"
        );

        let cultivation = app
            .world()
            .entity(entity)
            .get::<Cultivation>()
            .expect("cultivation should remain attached after breakthrough attempt");
        assert_eq!(cultivation.realm, Realm::Awaken);
        assert_approx_eq(cultivation.qi_current, 80.0);
        assert_approx_eq(cultivation.qi_max, 100.0);

        let player_state = app
            .world()
            .entity(entity)
            .get::<PlayerState>()
            .expect("player state should remain attached after rejected breakthrough");
        assert_eq!(player_state, &initial_state);

        let payloads = collect_server_data_payloads(&mut helper);
        assert!(
            extract_player_state_payloads(payloads.as_slice()).is_empty(),
            "failed cultivation breakthrough should not emit player_state payload"
        );
    }
}
