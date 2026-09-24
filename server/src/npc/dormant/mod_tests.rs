use super::*;
use crate::cultivation::components::{MeridianId, Realm};
use crate::qi_physics::constants::{QI_EPSILON, QI_ZONE_UNIT_CAPACITY};
use crate::qi_physics::QiAccountId;
use crate::world::dimension::DimensionKind;
use crate::world::zone::{Zone, DEFAULT_SPAWN_ZONE_NAME};
use std::time::{SystemTime, UNIX_EPOCH};
use valence::prelude::Events;

/// P0 bug② contract: the startup janitor purges leaked `{key}:tmp*` keys
/// but must never delete the live persisted hash. The deletion target is
/// decided by `tmp_keys_to_purge` over the SCAN results, so this pins that
/// decision directly (the SCAN/DEL I/O wrapper is best-effort glue around
/// it). Covers: empty input, mixed real-leak + legacy-nonce-leak, the
/// off-by-one where the live key shares the prefix but is NOT a temp key,
/// and an unrelated key.
#[test]
fn startup_janitor_purges_leaked_tmp_keys() {
    // Empty SCAN -> nothing to purge.
    assert!(
        tmp_keys_to_purge(&[]).is_empty(),
        "expected no purge targets from an empty SCAN because there is nothing leaked; got a non-empty list"
    );

    // The glob the janitor hands to SCAN must be anchored on the dormant key
    // and end in `:tmp*` so it catches both the deterministic temp key and
    // legacy nonce-suffixed survivors, and nothing outside the dormant key.
    assert_eq!(
        dormant_tmp_scan_pattern(),
        "bong:npc/dormant:tmp*",
        "expected the janitor SCAN glob to be `{{key}}:tmp*` so it matches every dormant temp key (deterministic + legacy nonce) and only those; got a different glob"
    );

    let scanned = vec![
        // Deterministic temp key from the current code path -> purge.
        "bong:npc/dormant:tmp".to_string(),
        // Legacy nonce-suffixed leak from the old code path -> purge.
        "bong:npc/dormant:tmp:1780000000000000000".to_string(),
        // The live persisted hash: shares the `bong:npc/dormant` prefix but
        // has NO `:tmp` segment -> must be KEPT (off-by-one boundary).
        NPC_DORMANT_REDIS_KEY.to_string(),
        // Unrelated key -> kept.
        "bong:world_state".to_string(),
    ];
    let purge = tmp_keys_to_purge(&scanned);

    assert_eq!(
        purge,
        vec![
            "bong:npc/dormant:tmp".to_string(),
            "bong:npc/dormant:tmp:1780000000000000000".to_string(),
        ],
        "expected exactly the two `{{key}}:tmp...` leaks to be purged because they are dead temp blobs, while the live `{NPC_DORMANT_REDIS_KEY}` hash and the unrelated key are preserved; got {purge:?}"
    );
    assert!(
        !purge.contains(&NPC_DORMANT_REDIS_KEY.to_string()),
        "expected the live persisted dormant hash to NEVER be a purge target (deleting it wipes all snapshots); it was selected for deletion"
    );
    assert!(
        !purge.contains(&"bong:world_state".to_string()),
        "expected unrelated keys to be left untouched by the dormant janitor; an unrelated key was selected for deletion"
    );

    let many = (0..(DORMANT_TEMP_DELETE_BATCH * 2 + 3))
        .map(|index| format!("{NPC_DORMANT_REDIS_KEY}:tmp:{index}"))
        .collect::<Vec<_>>();
    let batches = tmp_key_delete_batches(&many);
    assert_eq!(
        batches.iter().map(Vec::len).collect::<Vec<_>>(),
        vec![DORMANT_TEMP_DELETE_BATCH, DORMANT_TEMP_DELETE_BATCH, 3],
        "delete commands must remain bounded while preserving a short final page"
    );
    assert!(
        batches
            .iter()
            .all(|batch| batch.len() <= DORMANT_TEMP_DELETE_BATCH),
        "no janitor DEL command may exceed the fixed batch limit"
    );
}

fn zone() -> Zone {
    Zone {
        name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
        dimension: DimensionKind::Overworld,
        bounds: (DVec3::new(0.0, 0.0, 0.0), DVec3::new(100.0, 128.0, 100.0)),
        spirit_qi: 0.8,
        danger_level: 0,
        active_events: Vec::new(),
        patrol_anchors: vec![DVec3::new(10.0, 64.0, 10.0)],
        blocked_tiles: Vec::new(),
        qi_equilibrium: 0.0,
        qi_inflow_per_min: 0.0,
    }
}

fn snapshot(char_id: &str, pos: DVec3) -> NpcDormantSnapshot {
    let cultivation = Cultivation {
        qi_current: 0.1,
        qi_max: 1.0,
        ..Default::default()
    };
    NpcDormantSnapshot {
        char_id: char_id.to_string(),
        archetype: NpcArchetype::Rogue,
        dimension: DimensionKind::Overworld,
        zone_name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
        position: vec3_to_array(pos),
        schedule_seed: None,
        cultivation: cultivation.clone(),
        meridian_system: MeridianSystem::default(),
        meridian_severed: MeridianSeveredPermanent::default(),
        contamination: Contamination::default(),
        lifespan: NpcLifespan::new(0.0, 1_000.0),
        shared_lifespan: LifespanComponent::for_realm(cultivation.realm),
        lifespan_extension_ledger: LifespanExtensionLedger::default(),
        death_registry: DeathRegistry::new(char_id),
        life_record: LifeRecord::new(char_id),
        memory: None,
        player_reputation: None,
        faction: None,
        // 显式群体留空：走 effective_group 的 faction 派生回退路径（这里 faction=None ⇒
        // 群体 None ⇒ 不参战），顺带覆盖非破坏迁移分支。
        emergent_group: None,
        patrol: None,
        loot_table: None,
        guardian_relic: None,
        mimic_spider: None,
        tsy_hostile: None,
        tsy_sentinel: None,
        intent: DormantBehaviorIntent::Cultivate {
            zone: DEFAULT_SPAWN_ZONE_NAME.to_string(),
        },
        dormant_since_tick: 0,
        last_dormant_tick_processed: 0,
        initial_qi: 0.1,
        qi_ledger_net: 0.0,
        combat_dead_pending_release: false,
        pending_combat_winner: None,
    }
}

struct FixedRoll(f64);

impl RollSource for FixedRoll {
    fn roll_unit(&mut self) -> f64 {
        self.0
    }
}

fn open_regular_meridians(snapshot: &mut NpcDormantSnapshot, count: usize) {
    for id in MeridianId::REGULAR.into_iter().take(count) {
        let meridian = snapshot.meridian_system.get_mut(id);
        meridian.opened = true;
    }
}

#[test]
fn dormant_scatter_stays_in_zone_bounds() {
    // Hydration spawns the entity at exactly this position, so an
    // out-of-bounds seed would leak NPCs outside their home zone. Every
    // R2 sample (fx,fz ∈ [0,1)) plus clamp must land inside the AABB.
    let zone = zone();
    for idx in 0..256u32 {
        let pos = dormant_seed_scatter_position(&zone, idx);
        assert!(
            zone.contains(pos),
            "zone_local_index {idx} produced out-of-bound pos {pos:?} for bounds {:?}",
            zone.bounds
        );
    }
}

#[test]
fn dormant_scatter_spreads_across_zone_instead_of_clustering() {
    // Regression for the old anchor + ±2 block jitter: 64 snapshots seeded
    // into one zone must tile the whole footprint, not pile onto one anchor.
    let zone = zone();
    let (min, max) = zone.bounds;
    let width = max.x - min.x; // 100
    let depth = max.z - min.z; // 100
    let positions: Vec<DVec3> = (0..64u32)
        .map(|i| dormant_seed_scatter_position(&zone, i))
        .collect();

    // (a) Footprint span: the R2 sequence covers most of each axis.
    let span_x = positions
        .iter()
        .map(|p| p.x)
        .fold(f64::NEG_INFINITY, f64::max)
        - positions.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
    let span_z = positions
        .iter()
        .map(|p| p.z)
        .fold(f64::NEG_INFINITY, f64::max)
        - positions.iter().map(|p| p.z).fold(f64::INFINITY, f64::min);
    assert!(
        span_x > width * 0.8 && span_z > depth * 0.8,
        "64 snapshots should span >80% of the {width}x{depth} zone \
         (got span_x={span_x:.1}, span_z={span_z:.1}); the old ±2 jitter spanned <5",
    );

    // (b) No stacking: closest pair on the XZ plane stays well separated.
    let mut min_pair = f64::INFINITY;
    for (i, a) in positions.iter().enumerate() {
        for b in positions.iter().skip(i + 1) {
            let d = ((a.x - b.x).powi(2) + (a.z - b.z).powi(2)).sqrt();
            min_pair = min_pair.min(d);
        }
    }
    assert!(
        min_pair > 4.0,
        "closest pair of 64 scattered snapshots is {min_pair:.2} blocks apart; \
         expected > 4 (old jitter stacked many inside a 4-block box)",
    );
}

#[test]
fn dormant_scatter_is_deterministic_and_distinct() {
    // Same index → same position (Redis restore / re-seed stays stable);
    // distinct indices → distinct positions (no silent collisions / stacking).
    let zone = zone();
    assert_eq!(
        dormant_seed_scatter_position(&zone, 7),
        dormant_seed_scatter_position(&zone, 7),
        "scatter must be a pure function of (zone, index)"
    );
    let mut seen: Vec<DVec3> = Vec::new();
    for i in 0..128u32 {
        let pos = dormant_seed_scatter_position(&zone, i);
        assert!(
            !seen
                .iter()
                .any(|p| (p.x - pos.x).abs() < 1e-9 && (p.z - pos.z).abs() < 1e-9),
            "index {i} collided with an earlier snapshot at {pos:?}"
        );
        seen.push(pos);
    }
}

#[test]
fn store_indexes_by_archetype_and_zone() {
    let mut store = NpcDormantStore::default();
    store.insert(snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0)));
    store.insert(snapshot("npc_b", DVec3::new(11.0, 64.0, 10.0)));

    assert_eq!(
        store.ids_by_archetype(NpcArchetype::Rogue),
        &["npc_a".to_string(), "npc_b".to_string()]
    );
    assert_eq!(
        store.ids_by_zone(DEFAULT_SPAWN_ZONE_NAME),
        &["npc_a".to_string(), "npc_b".to_string()]
    );
}

/// P1 contract: a fresh store is clean, and the three real mutator
/// categories the publish gate cares about — seed, dormant aging tick, and
/// death/removal — each flip `is_dirty()` so the next publish cycle writes
/// the change to Redis. A change that never raises the flag would be
/// silently dropped by the dirty-gated publish path.
#[test]
fn dormant_store_dirty_set_on_seed_age_death() {
    // A default store has nothing to persist yet.
    assert!(
        !NpcDormantStore::default().is_dirty(),
        "expected a freshly constructed store to be clean because no snapshot has changed; it reported dirty"
    );

    // (1) seed: the real startup seed system populates the store and must
    // mark it dirty so the seeded population reaches Redis.
    let mut seed_app = App::new();
    seed_app.insert_resource(NpcVirtualizationConfig::default());
    seed_app.insert_resource(DormantRoguePopulationSeedConfig {
        target_count: 4,
        resource_fraction: 0.0,
        resource_spirit_qi_threshold: 0.4,
        max_initial_age_ratio: 0.0,
    });
    seed_app.insert_resource(ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone()],
    });
    seed_app.init_resource::<NpcDormantStore>();
    seed_app.add_systems(Update, seed_initial_dormant_population_on_startup);
    seed_app.update();
    let seeded = seed_app.world().resource::<NpcDormantStore>();
    assert!(
        !seeded.is_empty(),
        "seed system precondition failed: expected snapshots to be seeded before checking dirty"
    );
    assert!(
        seeded.is_dirty(),
        "expected the store to be dirty after the startup seed populated {} snapshots, so the seeded population is persisted; it stayed clean",
        seeded.len()
    );

    // (2) dormant aging tick: advancing an existing snapshot mutates its
    // age/position and must mark dirty.
    let mut age_app = App::new();
    age_app.add_event::<NpcDeathNotice>();
    age_app.add_event::<DormantCombatOutcome>();
    age_app.add_event::<PendingDormantRelicCreated>();
    age_app.insert_resource(NpcVirtualizationConfig {
        dormant_tick_interval_ticks: 1,
        ..Default::default()
    });
    age_app.insert_resource(GameTick(2400));
    age_app.insert_resource(ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone()],
    });
    age_app.insert_resource(WorldQiAccount::default());
    let mut store = NpcDormantStore::default();
    store.insert(snapshot("npc_age", DVec3::new(10.0, 64.0, 10.0)));
    // Clear the insert's dirty so we isolate the aging tick's effect.
    store.take_dirty();
    assert!(
        !store.is_dirty(),
        "test setup invariant: store must be clean before the aging tick so the tick is the only thing that can re-dirty it"
    );
    age_app.insert_resource(store);
    age_app.add_systems(Update, dormant_global_tick_system);
    age_app.update();
    assert!(
        age_app.world().resource::<NpcDormantStore>().is_dirty(),
        "expected the store to be dirty after a dormant aging tick advanced a snapshot (age/position changed), so the new state is persisted; it stayed clean"
    );

    // (3) death/removal: an expired snapshot whose qi is fully released is
    // removed by the tick; that removal must mark dirty.
    let mut death_app = App::new();
    death_app.add_event::<NpcDeathNotice>();
    death_app.add_event::<DormantCombatOutcome>();
    death_app.add_event::<PendingDormantRelicCreated>();
    death_app.insert_resource(NpcVirtualizationConfig {
        dormant_tick_interval_ticks: 1,
        ..Default::default()
    });
    death_app.insert_resource(GameTick(1));
    death_app.insert_resource(ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone()],
    });
    death_app.insert_resource(WorldQiAccount::default());
    let mut expired = snapshot("npc_dead", DVec3::new(10.0, 64.0, 10.0));
    expired.cultivation.qi_current = 0.0;
    expired.lifespan.age_ticks = expired.lifespan.max_age_ticks + 1.0;
    let mut death_store = NpcDormantStore::default();
    death_store.insert(expired);
    death_store.take_dirty();
    death_app.insert_resource(death_store);
    death_app.add_systems(Update, dormant_global_tick_system);
    death_app.update();
    let after_death = death_app.world().resource::<NpcDormantStore>();
    assert!(
        after_death.is_empty(),
        "death tick precondition failed: expired zero-qi snapshot should have been removed before checking dirty"
    );
    assert!(
        after_death.is_dirty(),
        "expected the store to be dirty after death removed an expired snapshot, so the deletion is persisted; it stayed clean"
    );
}

/// P1 contract: `take_dirty` reads-and-clears in one step. After taking, an
/// unchanged store reports clean and a second take returns false — the gate
/// will not re-write Redis on a cycle where nothing changed.
#[test]
fn dormant_receipt_cleanup_requires_current_success_and_keeps_tombstone_on_sqlite_failure() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("test clock should be after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "bong-dormant-receipt-{}-{unique}",
        std::process::id()
    ));
    let settings = crate::persistence::PersistenceSettings::with_db_path(
        root.join("data").join("bong.db"),
        "dormant-receipt-tombstone",
    );
    std::fs::create_dir_all(settings.db_path().parent().unwrap())
        .expect("receipt test database parent should exist");
    crate::persistence::bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("fixture sqlite should bootstrap");
    let record = crate::persistence::DormantTerminalCommitRecord {
        char_id: "npc:receipt:tombstone".to_string(),
        cause: "combat".to_string(),
        at_tick: 9,
        zone: "spawn".to_string(),
        winner: None,
        winner_group: None,
        loser_group: None,
        zone_accepted: 0.0,
        cleanup_revision: None,
    };
    crate::persistence::persist_dormant_terminal_commit(
        &settings,
        &record,
        &crate::world::zone::ZoneRegistry::fallback(),
        &crate::qi_physics::WorldQiAccount::default(),
        None,
    )
    .expect("terminal fixture should commit");

    let mut store = NpcDormantStore::default();
    store.install_terminal_tombstones(vec![record.clone()]);
    store.insert(snapshot(&record.char_id, DVec3::new(10.0, 64.0, 10.0)));
    assert!(
        store
            .to_redis_hash_payloads()
            .expect("tombstoned HASH serialization should succeed")
            .into_iter()
            .all(|(char_id, _)| char_id != record.char_id),
        "a tombstone must exclude its stale source even if a snapshot is accidentally retained"
    );
    let revision = store
        .begin_persistence()
        .expect("restored tombstone must publish a cleanup HASH");
    store
        .bind_unbound_terminal_tombstones(&settings, revision)
        .expect("cleanup revision should bind in sqlite");
    let tx = store.persistence_receipt_sender();
    tx.send(crate::network::redis_bridge::RedisDeliveryReceipt {
        delivery_id: (revision + 1).to_string(),
        outcome: Ok(()),
    })
    .unwrap();
    store.apply_persistence_receipts_with_settings(&settings);
    assert!(
        store.has_terminal_tombstone(&record.char_id),
        "non-current success must not authorize stale-source cleanup"
    );

    tx.send(crate::network::redis_bridge::RedisDeliveryReceipt {
        delivery_id: revision.to_string(),
        outcome: Err("delete publish failed".to_string()),
    })
    .unwrap();
    store.apply_persistence_receipts_with_settings(&settings);
    assert!(
        store.has_terminal_tombstone(&record.char_id),
        "failed deletion receipt must keep the tombstone for retry"
    );

    let retry = store
        .begin_persistence()
        .expect("failed deletion must re-arm the HASH revision");
    store
        .bind_unbound_terminal_tombstones(&settings, retry)
        .expect("retry cleanup revision should bind");
    let blocked_parent = root.join("not-a-directory");
    std::fs::write(&blocked_parent, b"block sqlite parent")
        .expect("cleanup failure fixture should create a regular file");
    let unavailable_settings = crate::persistence::PersistenceSettings::with_db_path(
        blocked_parent.join("bong.db"),
        "dormant-receipt-unavailable",
    );
    tx.send(crate::network::redis_bridge::RedisDeliveryReceipt {
        delivery_id: retry.to_string(),
        outcome: Ok(()),
    })
    .unwrap();
    store.apply_persistence_receipts_with_settings(&unavailable_settings);
    assert!(
        store.has_terminal_tombstone(&record.char_id),
        "a current successful HASH receipt must retain the tombstone when SQLite cleanup fails"
    );
    assert_eq!(
        crate::persistence::load_dormant_terminal_commits(&settings)
            .expect("failed cleanup must leave the original SQLite tombstone readable")
            .len(),
        1
    );

    let cleanup_revision = store
        .begin_persistence()
        .expect("SQLite cleanup failure must re-arm the HASH revision");
    tx.send(crate::network::redis_bridge::RedisDeliveryReceipt {
        delivery_id: cleanup_revision.to_string(),
        outcome: Ok(()),
    })
    .unwrap();
    store.apply_persistence_receipts_with_settings(&settings);
    assert!(
        !store.has_terminal_tombstone(&record.char_id),
        "only a current successful receipt plus successful SQLite cleanup may clear the tombstone"
    );
    assert!(crate::persistence::load_dormant_terminal_commits(&settings)
        .unwrap()
        .is_empty());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn dormant_persistence_receipts_are_revision_safe() {
    let mut store = NpcDormantStore::default();
    store.insert(snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0)));
    let first_revision = store
        .begin_persistence()
        .expect("dirty store must start its first HASH write");
    assert!(
        store.begin_persistence().is_none(),
        "an in-flight HASH write must block a second replacement"
    );

    store.mark_dirty();
    store
        .persistence_receipt_sender()
        .send(crate::network::redis_bridge::RedisDeliveryReceipt {
            delivery_id: first_revision.to_string(),
            outcome: Ok(()),
        })
        .unwrap();
    store.apply_persistence_receipts();
    assert!(
        !store.pending_combat_state_is_persisted(),
        "an older success must not confirm a mutation made while it was in flight"
    );

    let second_revision = store
        .begin_persistence()
        .expect("the newer dirty revision must start after the first receipt");
    store
        .persistence_receipt_sender()
        .send(crate::network::redis_bridge::RedisDeliveryReceipt {
            delivery_id: second_revision.to_string(),
            outcome: Err("redis unavailable".to_string()),
        })
        .unwrap();
    store.apply_persistence_receipts();
    assert!(
        store.is_dirty(),
        "a failed HASH receipt must re-arm persistence"
    );
    assert_eq!(
        store.begin_persistence(),
        Some(second_revision),
        "retry must preserve the failed mutation revision"
    );
}

/// P1 saturation: every store mutator raises dirty (insert AND remove), and
/// the Redis restore path does NOT (loaded snapshots are already
/// persisted). Also pins the de-dup behaviour: many mutations before one
/// take still need only a single take to clear.
#[test]
fn dormant_store_dirty_per_mutator_and_clean_on_restore() {
    // insert raises dirty.
    let mut store = NpcDormantStore::default();
    store.insert(snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0)));
    assert!(
        store.is_dirty(),
        "expected insert to mark the store dirty so the new snapshot is persisted; it did not"
    );

    // remove raises dirty (after clearing the insert's flag).
    store.take_dirty();
    let removed = store.remove("npc_a");
    assert!(
        removed.is_some(),
        "test setup invariant: the snapshot inserted above must exist so remove actually deletes it"
    );
    assert!(
        store.is_dirty(),
        "expected remove of an existing snapshot to mark the store dirty so the deletion is persisted; it did not"
    );

    // remove of a missing id must NOT raise dirty (nothing changed).
    store.take_dirty();
    let missing = store.remove("nope");
    assert!(
        missing.is_none(),
        "test setup invariant: removing an absent id must report None"
    );
    assert!(
        !store.is_dirty(),
        "expected removing an absent id to leave the store clean because nothing changed; it falsely marked dirty"
    );

    // Many mutations before a single take: one take clears them all.
    store.insert(snapshot("npc_b", DVec3::new(11.0, 64.0, 10.0)));
    store.insert(snapshot("npc_c", DVec3::new(12.0, 64.0, 10.0)));
    store.mark_dirty();
    assert!(
        store.take_dirty(),
        "expected take_dirty to return true after several mutations accumulated under one flag; it returned false"
    );
    assert!(
        !store.is_dirty(),
        "expected one take_dirty to clear the flag regardless of how many mutations preceded it; it stayed dirty"
    );

    // Redis restore path must NOT dirty the store: snapshots loaded from
    // Redis are already persisted, so writing them straight back would be a
    // wasteful no-op churn (the very thing P1 removes).
    let source = snapshot("npc_loaded", DVec3::new(10.0, 64.0, 10.0));
    let payload = serde_json::to_string(&source).expect("serialize dormant snapshot");
    let entries = HashMap::from([(source.char_id.clone(), payload)]);
    let mut restore_store = NpcDormantStore::default();
    let count =
        load_dormant_snapshots_from_hash_entries(&mut restore_store, entries).expect("load");
    assert_eq!(
        count, 1,
        "restore precondition: exactly one snapshot should have loaded"
    );
    assert!(
        restore_store.contains("npc_loaded"),
        "restore precondition: the loaded snapshot should be present"
    );
    assert!(
        !restore_store.is_dirty(),
        "expected a store restored from Redis to be CLEAN because the data is already persisted; restoring marked it dirty and would trigger a redundant write-back"
    );
}

#[test]
fn daozhan_terminal_release_settles_both_external_owners_atomically() {
    let mut snapshot = snapshot("npc_daozhan", DVec3::new(10.0, 64.0, 10.0));
    snapshot.cultivation.qi_current = 0.4;
    snapshot.tsy_hostile = Some(DormantTsyHostileSnapshot {
        family_id: "family-a".to_string(),
        zhinian_phase: None,
        zhinian_phase_entered_at_tick: None,
        fuya_aura: None,
        daoxiang_origin: None,
        daozhan: Some(DormantDaozhanSnapshot {
            state: DaoZhangState::default(),
            home_zone: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            home_pos: snapshot.position,
            daozhan_qi: 0.6,
            origin_realm: None,
            behavior_queue: Vec::new(),
            current_behavior_ticks: 0,
        }),
    });
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone()],
    };
    let mut ledger = WorldQiAccount::default();

    let outcome = release_dormant_qi_to_zone(&mut snapshot, &mut zones, &mut ledger)
        .expect("both dormant external owners should settle in one staged transaction");

    assert_eq!(outcome.source_debited, 1.0);
    assert_eq!(outcome.zone_accepted, 1.0);
    assert!(dormant_terminal_qi_is_settled(&snapshot));
    assert_eq!(outcome.transfers.len(), 2);
    assert!(outcome
        .transfers
        .iter()
        .all(|transfer| transfer.from == QiAccountId::npc("npc_daozhan")));
    assert_eq!(ledger.transfers().len(), 2);
}

#[test]
fn dormant_global_tick_settles_expired_daozhan_cultivation_and_drain_owners() {
    let mut app = App::new();
    app.add_event::<NpcDeathNotice>();
    app.add_event::<DormantCombatOutcome>();
    app.add_event::<PendingDormantRelicCreated>();
    app.insert_resource(NpcVirtualizationConfig {
        dormant_tick_interval_ticks: 1,
        ..Default::default()
    });
    app.insert_resource(GameTick(1));
    let mut full_zone = zone();
    full_zone.spirit_qi = 0.99;
    app.insert_resource(ZoneRegistry {
        spatial_revision: 0,
        zones: vec![full_zone],
    });
    app.insert_resource(WorldQiAccount::default());
    let mut expired = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
    expired.cultivation.qi_current = 1.0;
    expired.cultivation.qi_max = 2.0;
    expired.tsy_hostile = Some(DormantTsyHostileSnapshot {
        family_id: "family-a".to_string(),
        zhinian_phase: None,
        zhinian_phase_entered_at_tick: None,
        fuya_aura: None,
        daoxiang_origin: None,
        daozhan: Some(DormantDaozhanSnapshot {
            state: DaoZhangState::default(),
            home_zone: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            home_pos: expired.position,
            daozhan_qi: 1.0,
            origin_realm: None,
            behavior_queue: Vec::new(),
            current_behavior_ticks: 0,
        }),
    });
    expired.lifespan.age_ticks = expired.lifespan.max_age_ticks + 1.0;
    let mut store = NpcDormantStore::default();
    store.insert(expired);
    app.insert_resource(store);
    app.add_systems(Update, dormant_global_tick_system);

    app.update();

    let store = app.world().resource::<NpcDormantStore>();
    assert!(
        !store.contains("npc_a"),
        "successful typed settlement must remove the expired dormant owner even when the zone accepts only part"
    );
    let zones = app.world().resource::<ZoneRegistry>();
    assert!((zones.zones[0].spirit_qi - 1.0).abs() < 1e-9);
    let ledger = app.world().resource::<WorldQiAccount>();
    let release_total: f64 = ledger
        .transfers()
        .iter()
        .filter(|transfer| transfer.reason == QiTransferReason::ReleaseToZone)
        .map(|transfer| transfer.amount)
        .sum();
    assert!(
        (release_total - 2.0).abs() < 1e-9,
        "cultivation and Daozhan owners must release their full combined 2.0 qi through typed transfers"
    );
    assert!(
        (ledger.balance(&crate::qi_physics::qi_flow_overflow_account()) - 1.5).abs() < 1e-9,
        "the amount rejected by the near-full zone must persist in fixed overflow"
    );
    assert!(
        !ledger.has_account(&QiAccountId::npc("npc_a"))
            && !ledger.has_account(&QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME)),
        "successful settlement must not leave dormant or Zone mirrors"
    );
    let events = app.world().resource::<Events<NpcDeathNotice>>();
    assert_eq!(
        events.iter_current_update_events().count(),
        1,
        "natural death notice must emit after the full physical-owner settlement commits"
    );
}

#[test]
fn dormant_global_tick_retains_expired_snapshot_when_settlement_fails() {
    let mut app = App::new();
    app.add_event::<NpcDeathNotice>();
    app.add_event::<DormantCombatOutcome>();
    app.add_event::<PendingDormantRelicCreated>();
    app.insert_resource(NpcVirtualizationConfig {
        dormant_tick_interval_ticks: 1,
        ..Default::default()
    });
    app.insert_resource(GameTick(1));
    let mut invalid_zone = zone();
    invalid_zone.spirit_qi = f64::NAN;
    app.insert_resource(ZoneRegistry {
        spatial_revision: 0,
        zones: vec![invalid_zone],
    });
    app.insert_resource(WorldQiAccount::default());
    let mut expired = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
    expired.cultivation.qi_current = 5e-7;
    expired.cultivation.qi_max = 1.0;
    expired.lifespan.age_ticks = expired.lifespan.max_age_ticks + 1.0;
    let mut store = NpcDormantStore::default();
    store.insert(expired);
    store.take_dirty();
    app.insert_resource(store);
    app.add_systems(Update, dormant_global_tick_system);

    app.update();

    let mut store = app
        .world_mut()
        .remove_resource::<NpcDormantStore>()
        .expect("dormant store must survive a failed natural-death settlement");
    let retained = store
        .snapshots
        .get("npc_a")
        .expect("failed settlement must retain the positive sub-epsilon physical owner");
    assert_eq!(retained.cultivation.qi_current(), 5e-7);
    assert!(
        store.take_dirty(),
        "retained expiry state must be persisted"
    );
    let zones = app.world().resource::<ZoneRegistry>();
    assert!(zones.zones[0].spirit_qi.is_nan());
    let ledger = app.world().resource::<WorldQiAccount>();
    assert_eq!(ledger.total(), 0.0);
    assert!(ledger.transfers().is_empty());
    assert_eq!(
        app.world()
            .resource::<Events<NpcDeathNotice>>()
            .iter_current_update_events()
            .count(),
        0,
        "natural death must not publish before typed settlement commits"
    );
}

#[test]
fn dormant_global_tick_clears_indexes_when_all_snapshots_expire() {
    let mut app = App::new();
    app.add_event::<NpcDeathNotice>();
    app.add_event::<DormantCombatOutcome>();
    app.add_event::<PendingDormantRelicCreated>();
    app.insert_resource(NpcVirtualizationConfig {
        dormant_tick_interval_ticks: 1,
        ..Default::default()
    });
    app.insert_resource(GameTick(1));
    app.insert_resource(ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone()],
    });
    app.insert_resource(WorldQiAccount::default());
    let mut expired = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
    expired.lifespan.age_ticks = expired.lifespan.max_age_ticks + 1.0;
    let mut store = NpcDormantStore::default();
    store.insert(expired);
    app.insert_resource(store);
    app.add_systems(Update, dormant_global_tick_system);

    app.update();

    let store = app.world().resource::<NpcDormantStore>();
    assert!(store.is_empty());
    assert!(store.ids_by_archetype(NpcArchetype::Rogue).is_empty());
    assert!(store.ids_by_zone(DEFAULT_SPAWN_ZONE_NAME).is_empty());
}

#[test]
fn dormant_global_tick_refreshes_zone_index_after_movement() {
    let mut app = App::new();
    app.add_event::<NpcDeathNotice>();
    app.add_event::<DormantCombatOutcome>();
    app.add_event::<PendingDormantRelicCreated>();
    app.insert_resource(NpcVirtualizationConfig {
        dormant_tick_interval_ticks: 1,
        ..Default::default()
    });
    app.insert_resource(GameTick(2400));
    let second_zone = Zone {
        name: "east".to_string(),
        dimension: DimensionKind::Overworld,
        bounds: (DVec3::new(120.0, 0.0, 0.0), DVec3::new(200.0, 128.0, 80.0)),
        spirit_qi: 0.5,
        danger_level: 0,
        active_events: Vec::new(),
        patrol_anchors: Vec::new(),
        blocked_tiles: Vec::new(),
        qi_equilibrium: 0.0,
        qi_inflow_per_min: 0.0,
    };
    app.insert_resource(ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone(), second_zone],
    });
    app.insert_resource(WorldQiAccount::default());
    let mut mover = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
    mover.intent = DormantBehaviorIntent::PatrolToward {
        target: [130.0, 64.0, 10.0],
    };
    let mut store = NpcDormantStore::default();
    store.insert(mover);
    app.insert_resource(store);
    app.add_systems(Update, dormant_global_tick_system);

    app.update();

    let store = app.world().resource::<NpcDormantStore>();
    assert_eq!(store.snapshots["npc_a"].zone_name, "east");
    assert!(store.ids_by_zone(DEFAULT_SPAWN_ZONE_NAME).is_empty());
    assert_eq!(store.ids_by_zone("east"), &["npc_a"]);
}

#[test]
fn dormant_breakthrough_uses_cultivation_rules_below_duxu() {
    let mut snapshot = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
    snapshot.cultivation.realm = Realm::Awaken;
    snapshot.cultivation.qi_current = 20.0;
    snapshot.cultivation.qi_max = 100.0;
    snapshot.lifespan.age_ticks = 1_100.0;
    open_regular_meridians(&mut snapshot, 3);
    let mut roll = FixedRoll(0.0);
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone()],
    };
    let mut ledger = WorldQiAccount::default();

    let result = advance_dormant_breakthrough_with_roll(
        &mut snapshot,
        &mut zones,
        &mut ledger,
        1200,
        None,
        None,
        &mut roll,
    )
    .expect("eligible dormant NPC should attempt breakthrough")
    .expect("fixed low roll should pass");

    assert_eq!(result.to, Realm::Induce);
    assert_eq!(snapshot.cultivation.realm, Realm::Induce);
    assert_eq!(snapshot.cultivation.qi_current, 12.0);
    assert!(
        !ledger.has_account(&QiAccountId::npc("npc_a")),
        "dormant breakthrough must keep actor qi solely in the snapshot, without an NPC ledger shadow"
    );
    assert!(
        !ledger.has_account(&QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME)),
        "dormant breakthrough must keep environment qi solely in Zone.spirit_qi, without a Zone ledger shadow"
    );
    assert!(
        (zones.zones[0].spirit_qi - (0.8 + 8.0 / QI_ZONE_UNIT_CAPACITY)).abs() < 1e-9,
        "dormant breakthrough cost must be credited to the physical Zone owner"
    );
    let transfer = ledger
        .transfers()
        .last()
        .expect("dormant breakthrough should leave a QiTransfer");
    assert_eq!(transfer.reason, QiTransferReason::Breakthrough);
    assert_eq!(transfer.amount, 8.0);
    assert!(
        (snapshot.qi_ledger_net - (-8.0)).abs() < f64::EPSILON,
        "dormant breakthrough must debit qi_ledger_net by the spent qi; expected -8.0, got {}",
        snapshot.qi_ledger_net
    );
    assert_eq!(
        snapshot.shared_lifespan.cap_by_realm,
        LifespanCapTable::INDUCE
    );
    assert!((snapshot.lifespan.max_age_ticks - 1_666.666_666_666_666_7).abs() < 1e-9);
    assert!(!snapshot.lifespan.is_expired());
    assert!(snapshot.life_record.biography.iter().any(|entry| {
        matches!(
            entry,
            BiographyEntry::BreakthroughSucceeded {
                realm: Realm::Induce,
                tick: 1200
            }
        )
    }));
}

#[test]
fn dormant_breakthrough_settlement_failure_rolls_back_all_state() {
    let mut snapshot = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
    snapshot.cultivation.realm = Realm::Awaken;
    snapshot.cultivation.qi_current = 20.0;
    snapshot.cultivation.qi_max = 100.0;
    snapshot.lifespan.age_ticks = 1_100.0;
    open_regular_meridians(&mut snapshot, 3);
    let before_cultivation = snapshot.cultivation.clone();
    let before_meridians = snapshot.meridian_system.clone();
    let before_net = snapshot.qi_ledger_net;
    let mut roll = FixedRoll(0.0);
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone()],
    };
    zones.zones[0].spirit_qi = 1.0;
    let before_zone = zones.zones[0].spirit_qi;
    let overflow = crate::qi_physics::qi_flow_overflow_account();
    let mut ledger = WorldQiAccount::default();
    ledger
        .set_balance(overflow.clone(), f64::MAX)
        .expect("saturated overflow fixture must be valid");
    let before_audit = ledger.transfers().to_vec();

    let result = advance_dormant_breakthrough_with_roll(
        &mut snapshot,
        &mut zones,
        &mut ledger,
        1200,
        None,
        None,
        &mut roll,
    );

    assert!(
        result.is_none(),
        "failed qi settlement must abort breakthrough"
    );
    assert_eq!(snapshot.cultivation, before_cultivation);
    assert_eq!(snapshot.meridian_system, before_meridians);
    assert_eq!(snapshot.qi_ledger_net, before_net);
    assert_eq!(zones.zones[0].spirit_qi, before_zone);
    assert_eq!(ledger.balance(&overflow), f64::MAX);
    assert_eq!(ledger.transfers(), before_audit);
}

#[test]
fn loads_dormant_snapshots_from_redis_hash_entries() {
    let source = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
    let payload = serde_json::to_string(&source).expect("serialize dormant snapshot");
    let entries = HashMap::from([(source.char_id.clone(), payload)]);
    let mut store = NpcDormantStore::default();

    let count = load_dormant_snapshots_from_hash_entries(&mut store, entries).expect("load");

    assert_eq!(count, 1);
    assert!(store.contains("npc_a"));
    assert_eq!(store.ids_by_zone(DEFAULT_SPAWN_ZONE_NAME), &["npc_a"]);
}

#[test]
fn terminal_tombstone_suppresses_even_corrupt_stale_row_before_batch_validation() {
    let tombstoned_id = "npc_terminal_stale";
    let valid = snapshot("npc_valid", DVec3::new(2.0, 64.0, 2.0));
    let entries = HashMap::from([
        (tombstoned_id.to_string(), "{stale-corrupt-json".to_string()),
        (
            valid.char_id.clone(),
            serde_json::to_string(&valid).expect("serialize valid owner row"),
        ),
    ]);
    let mut store = NpcDormantStore::default();
    store.install_terminal_tombstones(vec![crate::persistence::DormantTerminalCommitRecord {
        char_id: tombstoned_id.to_string(),
        cause: "combat".to_string(),
        at_tick: 42,
        zone: DEFAULT_SPAWN_ZONE_NAME.to_string(),
        winner: Some("npc_winner".to_string()),
        winner_group: None,
        loser_group: None,
        zone_accepted: 1.0,
        cleanup_revision: None,
    }]);

    let count = load_dormant_snapshots_from_hash_entries(&mut store, entries)
        .expect("a committed stale source is suppressed by its HASH identity before decoding");

    assert_eq!(count, 1);
    assert!(store.contains("npc_valid"));
    assert!(!store.contains(tombstoned_id));
    assert!(
        store.is_dirty(),
        "installed tombstone must retain the cleanup HASH mutation"
    );
}

#[test]
fn redis_hash_restore_rejects_partial_corruption_without_mutating_live_store() {
    let existing = snapshot("npc_existing", DVec3::new(1.0, 64.0, 1.0));
    let incoming = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
    let payload = serde_json::to_string(&incoming).expect("serialize dormant snapshot");
    let entries = HashMap::from([
        (incoming.char_id.clone(), payload),
        ("npc_bad".to_string(), "{not-json".to_string()),
    ]);
    let mut store = NpcDormantStore::default();
    store.insert(existing);
    store.take_dirty();

    let error = load_dormant_snapshots_from_hash_entries(&mut store, entries)
        .expect_err("one corrupt dormant qi owner must reject the complete Redis restore");

    assert!(
        error.contains("refusing partial dormant Redis restore"),
        "partial corruption should report the fail-closed restore boundary, got: {error}"
    );
    assert_eq!(
        store.len(),
        1,
        "failed restore must leave the pre-existing live store unchanged"
    );
    assert!(store.contains("npc_existing"));
    assert!(
        !store.contains("npc_a") && !store.contains("npc_bad"),
        "failed restore must not commit even valid staged rows"
    );
    assert!(
        !store.is_dirty(),
        "failed restore must not arm a full-HASH write that could erase corrupt owners"
    );
}

#[test]
fn failed_restore_blocks_tombstone_cleanup_hash_replacement() {
    let mut store = NpcDormantStore::default();
    store.install_terminal_tombstones(vec![crate::persistence::DormantTerminalCommitRecord {
        char_id: "npc_terminal_stale".to_string(),
        cause: "combat".to_string(),
        at_tick: 42,
        zone: DEFAULT_SPAWN_ZONE_NAME.to_string(),
        winner: None,
        winner_group: None,
        loser_group: None,
        zone_accepted: 1.0,
        cleanup_revision: None,
    }]);
    store.mark_restore_failed();

    assert!(
        store.is_dirty(),
        "restored tombstone should still remember that its stale row needs deletion"
    );
    assert_eq!(
        store.begin_persistence(),
        None,
        "an untrusted partial restore must never replace the full HASH, even for tombstone cleanup"
    );
    assert!(store.has_terminal_tombstone("npc_terminal_stale"));
}

#[test]
fn redis_hash_restore_rejects_negative_daozhan_owner_atomically() {
    let existing = snapshot("npc_existing", DVec3::new(1.0, 64.0, 1.0));
    let valid = snapshot("npc_valid", DVec3::new(2.0, 64.0, 2.0));
    let mut invalid = snapshot("npc_daozhan", DVec3::new(10.0, 64.0, 10.0));
    invalid.tsy_hostile = Some(DormantTsyHostileSnapshot {
        family_id: "family-a".to_string(),
        zhinian_phase: None,
        zhinian_phase_entered_at_tick: None,
        fuya_aura: None,
        daoxiang_origin: None,
        daozhan: Some(DormantDaozhanSnapshot {
            state: DaoZhangState::default(),
            home_zone: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            home_pos: invalid.position,
            daozhan_qi: -0.25,
            origin_realm: None,
            behavior_queue: Vec::new(),
            current_behavior_ticks: 0,
        }),
    });
    let entries = HashMap::from([
        (
            valid.char_id.clone(),
            serde_json::to_string(&valid).expect("serialize valid row"),
        ),
        (
            invalid.char_id.clone(),
            serde_json::to_string(&invalid).expect("serialize invalid owner row"),
        ),
    ]);
    let mut store = NpcDormantStore::default();
    store.insert(existing);
    store.take_dirty();

    let error = load_dormant_snapshots_from_hash_entries(&mut store, entries)
        .expect_err("negative Daozhan qi must reject the complete Redis HASH");

    assert!(error.contains("invalid dormant Daozhan qi owner"));
    assert_eq!(store.len(), 1);
    assert!(store.contains("npc_existing"));
    assert!(!store.contains("npc_valid") && !store.contains("npc_daozhan"));
    assert!(!store.is_dirty());
}

#[test]
fn redis_hash_restore_rejects_hash_field_identity_mismatch_atomically() {
    let existing = snapshot("npc_existing", DVec3::new(1.0, 64.0, 1.0));
    let valid = snapshot("npc_valid", DVec3::new(2.0, 64.0, 2.0));
    let incoming = snapshot("payload_owner", DVec3::new(10.0, 64.0, 10.0));
    let entries = HashMap::from([
        (
            valid.char_id.clone(),
            serde_json::to_string(&valid).expect("serialize valid row"),
        ),
        (
            "different_hash_field".to_string(),
            serde_json::to_string(&incoming).expect("serialize mismatched row"),
        ),
    ]);
    let mut store = NpcDormantStore::default();
    store.insert(existing);
    store.take_dirty();

    let error = load_dormant_snapshots_from_hash_entries(&mut store, entries)
        .expect_err("HASH field and durable snapshot identity must agree");

    assert!(
        error.contains("does not match HASH field"),
        "identity mismatch should be explicit, got: {error}"
    );
    assert_eq!(store.len(), 1);
    assert!(store.contains("npc_existing"));
    assert!(
        !store.contains("npc_valid") && !store.contains("payload_owner"),
        "HASH/outer mismatch must reject valid staged rows without partially replacing the live store"
    );
    assert!(!store.is_dirty());
}

fn mutate_life_record(snapshot: &mut NpcDormantSnapshot) {
    snapshot.life_record.character_id = "spoofed".to_string();
}

fn mutate_death_registry(snapshot: &mut NpcDormantSnapshot) {
    snapshot.death_registry.char_id = "spoofed".to_string();
}

#[test]
fn redis_hash_restore_rejects_every_nested_identity_mismatch_atomically() {
    for (case, mutate) in [
        (
            "life_record",
            mutate_life_record as fn(&mut NpcDormantSnapshot),
        ),
        (
            "death_registry",
            mutate_death_registry as fn(&mut NpcDormantSnapshot),
        ),
    ] {
        let existing = snapshot("npc_existing", DVec3::new(1.0, 64.0, 1.0));
        let valid = snapshot("npc_valid", DVec3::new(2.0, 64.0, 2.0));
        let mut invalid = snapshot("npc_invalid", DVec3::new(3.0, 64.0, 3.0));
        mutate(&mut invalid);
        let entries = HashMap::from([
            (
                valid.char_id.clone(),
                serde_json::to_string(&valid).expect("serialize valid row"),
            ),
            (
                invalid.char_id.clone(),
                serde_json::to_string(&invalid).expect("serialize invalid row"),
            ),
        ]);
        let mut store = NpcDormantStore::default();
        store.insert(existing);
        store.take_dirty();

        let error = load_dormant_snapshots_from_hash_entries(&mut store, entries)
            .expect_err("nested identity spoofing must reject the complete restore");

        assert!(
            error.contains("durable identity tuple mismatch"),
            "case={case}: {error}"
        );
        assert_eq!(store.len(), 1, "case={case}");
        assert!(store.contains("npc_existing"), "case={case}");
        assert!(!store.contains("npc_valid"), "case={case}");
        assert!(!store.is_dirty(), "case={case}");
    }
}

#[test]
fn redis_hash_restore_rejects_semantically_invalid_cultivation_atomically() {
    for (case, qi_current) in [
        ("negative", "-1.0"),
        ("above_max", "101.0"),
        ("non_finite", "1e400"),
    ] {
        let existing = snapshot("npc_existing", DVec3::new(1.0, 64.0, 1.0));
        let valid = snapshot("npc_valid", DVec3::new(2.0, 64.0, 2.0));
        let invalid = snapshot("npc_invalid", DVec3::new(3.0, 64.0, 3.0));
        let template_json = serde_json::to_value(&invalid).expect("serialize invalid row template");
        let mut invalid_json = template_json.clone();
        invalid_json
            .get_mut("cultivation")
            .and_then(serde_json::Value::as_object_mut)
            .expect("snapshot fixture must contain a cultivation object")
            .insert(
                "qi_current".to_string(),
                qi_current
                    .parse::<f64>()
                    .map(serde_json::Value::from)
                    .unwrap_or_else(|_| serde_json::json!(qi_current)),
            );
        assert_ne!(
            invalid_json, template_json,
            "case={case}: cultivation fixture mutation must target nested qi_current"
        );
        let invalid_json = serde_json::to_string(&invalid_json)
            .expect("mutated invalid row must remain serializable");
        let entries = HashMap::from([
            (
                valid.char_id.clone(),
                serde_json::to_string(&valid).expect("serialize valid row"),
            ),
            (invalid.char_id.clone(), invalid_json),
        ]);
        let mut store = NpcDormantStore::default();
        store.insert(existing);
        store.take_dirty();

        load_dormant_snapshots_from_hash_entries(&mut store, entries)
            .expect_err("invalid cultivation must reject the complete restore");

        assert_eq!(store.len(), 1, "case={case}");
        assert!(store.contains("npc_existing"), "case={case}");
        assert!(!store.contains("npc_valid"), "case={case}");
        assert!(!store.is_dirty(), "case={case}");
    }
}

#[test]
fn redis_hash_restore_fails_when_every_entry_is_invalid() {
    let entries = HashMap::from([("npc_bad".to_string(), "{not-json".to_string())]);
    let mut store = NpcDormantStore::default();

    let error = load_dormant_snapshots_from_hash_entries(&mut store, entries)
        .expect_err("all invalid entries should fail restore");

    assert!(error.contains("refusing partial dormant Redis restore"));
    assert!(error.contains("1 of 1 snapshot entries were invalid"));
    assert!(store.is_empty());
}

// ── plan-offscreen-war-v1 P0：确定性 env 旋钮 ─────────────────────────
//
// 解析逻辑用纯函数 `parse_*` 测，避免 `std::env::set_var` 在并行测试间互相污染
// 全局进程状态（vitest/cargo test 默认多线程）。env 通路本身由真服 e2e 覆盖。

#[test]
fn bong_dormant_tick_interval_env_overrides_default() {
    // 合法正值覆盖默认 1200，让离屏 tick 快进到秒级（e2e 用此把 60s 压到 1 tick）。
    assert_eq!(
        parse_dormant_tick_interval(Some("5"), DORMANT_LIFECYCLE_TICK_INTERVAL),
        5,
        "正值 BONG_DORMANT_TICK_INTERVAL 必须覆盖默认 1200，否则 e2e 无法快进离屏 tick"
    );
    assert_eq!(
        parse_dormant_tick_interval(Some("  42  "), DORMANT_LIFECYCLE_TICK_INTERVAL),
        42,
        "首尾空白应被 trim 后解析，期望 42"
    );
}

#[test]
fn bong_dormant_tick_interval_unset_keeps_default() {
    assert_eq!(
        parse_dormant_tick_interval(None, DORMANT_LIFECYCLE_TICK_INTERVAL),
        DORMANT_LIFECYCLE_TICK_INTERVAL,
        "env 未设时必须保持默认 1200（= 现有运行时行为）"
    );
    // from_env 默认（未注入 env 旋钮的字段）应与 default 一致。
    let cfg = NpcVirtualizationConfig::default();
    assert_eq!(
        cfg.dormant_tick_interval_ticks,
        DORMANT_LIFECYCLE_TICK_INTERVAL
    );
    assert_eq!(
        cfg.sim_seed, 0,
        "默认 sim_seed 必须为 0 = 未注入 seed 时的确定性基线"
    );
}

#[test]
fn bong_dormant_tick_interval_zero_and_garbage_fall_back_gracefully() {
    // 0 非法（会造成 is_multiple_of(0) panic / 除零语义），必须回退默认而非采纳。
    assert_eq!(
        parse_dormant_tick_interval(Some("0"), DORMANT_LIFECYCLE_TICK_INTERVAL),
        DORMANT_LIFECYCLE_TICK_INTERVAL,
        "0 是非法 tick 间隔，期望 graceful 回退默认 1200"
    );
    // 垃圾 / 负数 / 溢出均无法 parse 为 u32，回退默认。
    for garbage in ["", "abc", "-3", "3.5", "99999999999999999999"] {
        assert_eq!(
            parse_dormant_tick_interval(Some(garbage), DORMANT_LIFECYCLE_TICK_INTERVAL),
            DORMANT_LIFECYCLE_TICK_INTERVAL,
            "非法值 {garbage:?} 期望回退默认 1200 而非 panic"
        );
    }
}

#[test]
fn bong_sim_seed_makes_combat_deterministic() {
    // P0 是 plumbing 层：相同 BONG_SIM_SEED 解析出相同 u64，注入 config.sim_seed
    // 后即可让 P1/P2 的 RNG 序列复现（同 seed → 同战死结果）。
    let seed_a = parse_sim_seed(Some("123456789"));
    let seed_b = parse_sim_seed(Some("123456789"));
    assert_eq!(
        seed_a, seed_b,
        "同一 BONG_SIM_SEED 必须解析出同值，否则离屏战争结果不可复现"
    );
    assert_eq!(seed_a, 123_456_789);

    // 不同 seed 必须区分（让 e2e 能跑出不同 RNG 序列）。
    assert_ne!(
        parse_sim_seed(Some("1")),
        parse_sim_seed(Some("2")),
        "不同 seed 必须解析为不同值"
    );

    // 注入路径：解析出的 seed 直接落进 NpcVirtualizationConfig.sim_seed，
    // dormant 战斗（P1/P2）即读这同一个种子，保证同 seed → 同战死结果。
    let parsed = parse_sim_seed(Some("777"));
    let cfg = NpcVirtualizationConfig {
        sim_seed: parsed,
        ..NpcVirtualizationConfig::default()
    };
    assert_eq!(
        cfg.sim_seed, parsed,
        "解析出的 seed 必须原样进入 config.sim_seed，否则 dormant 战斗 RNG 与配置不同步"
    );
}

#[test]
fn bong_sim_seed_unset_or_garbage_defaults_to_zero() {
    assert_eq!(parse_sim_seed(None), 0, "env 未设时默认种子 0 = 现有行为");
    for garbage in ["", "abc", "-1", "1.0"] {
        assert_eq!(
            parse_sim_seed(Some(garbage)),
            0,
            "非法 seed {garbage:?} 期望回退 0 而非 panic"
        );
    }
}

// ── plan-offscreen-war-v1 P0 #1：派系数据化 bootstrap ─────────────────

#[test]
fn seed_rogue_faction_is_deterministic_per_char_id() {
    // 同 char_id 跨调用必须分到同一派系（重启后 dormant 派系稳定）。
    let a = seed_rogue_faction("dormant:rogue:42");
    let b = seed_rogue_faction("dormant:rogue:42");
    assert_eq!(
        a.faction_id, b.faction_id,
        "同 char_id 必须确定性分派，否则重启后敌对关系漂移"
    );
    assert_eq!(a.rank, FactionRank::Disciple);
}

#[test]
fn seed_rogue_faction_distribution_yields_both_attack_and_defend() {
    // 哈希分布必须同时产出 Attack 与 Defend，否则 is_hostile_pair 永远配不出对。
    let mut seen_attack = false;
    let mut seen_defend = false;
    for index in 0..256u32 {
        match seed_rogue_faction(&format!("dormant:rogue:{index}")).faction_id {
            FactionId::Attack => seen_attack = true,
            FactionId::Defend => seen_defend = true,
            FactionId::Neutral => {
                panic!("seed 派系绝不应分到 Neutral（Neutral 对谁都不敌对，会让战斗空转）")
            }
        }
        if seen_attack && seen_defend {
            break;
        }
    }
    assert!(
        seen_attack && seen_defend,
        "256 个 char_id 必须同时出现 Attack 与 Defend，保证 is_hostile_pair 有敌对对"
    );
}

#[test]
fn seed_rogue_faction_pairs_are_hostile_across_factions() {
    // 端到端契约：分到不同派系的两个 rogue，FactionStore::is_hostile_pair 必须为真。
    use crate::npc::faction::FactionStore;
    let store = FactionStore::default();
    let attacker = seed_rogue_faction("seed:attack-fixture");
    let defender = (0..64u32)
        .map(|i| seed_rogue_faction(&format!("seed:defend-fixture:{i}")))
        .find(|m| m.faction_id != attacker.faction_id)
        .expect("应能找到一个异派系成员");
    assert!(
        store.is_hostile_pair(attacker.faction_id, defender.faction_id),
        "Attack↔Defend 必须敌对，否则 P1/P2 战斗无候选对"
    );
    // 同派系不敌对（确认二分不会把同派系也当敌对）。
    assert!(
        !store.is_hostile_pair(attacker.faction_id, attacker.faction_id),
        "同派系不应敌对"
    );
}

#[test]
fn dormant_rogue_seed_snapshot_assigns_non_none_faction() {
    // 防回归：seed 出来的 dormant rogue 的 faction 字段必须非 None
    // （否则 e2e HGETALL 看到 faction=null，所有后续阶段空转）。
    let zone = zone();
    let snapshot = dormant_rogue_seed_snapshot(&zone, 0, 0, 0, 0.8, true);
    let membership = snapshot
        .faction
        .as_ref()
        .expect("seeded dormant rogue 必须带 FactionMembership，不能是 None");
    assert!(
        matches!(membership.faction_id, FactionId::Attack | FactionId::Defend),
        "seed 派系必须是 Attack 或 Defend（非 Neutral），实际 {:?}",
        membership.faction_id
    );
}

// ── plan-offscreen-war-v1 P5 reframe b：seed 涌现群体散布 ──────────────────

#[test]
fn seed_emergent_group_is_deterministic_per_char_id() {
    // 同 char_id 跨调用必须分到同一群体（否则重启后离屏敌对关系漂移）。
    let a = seed_emergent_group("dormant:rogue:42");
    let b = seed_emergent_group("dormant:rogue:42");
    assert_eq!(
        a, b,
        "同 char_id 必须确定性散布到同一涌现群体，否则重启后敌对关系漂移"
    );
}

#[test]
fn seed_emergent_group_distribution_covers_at_least_three_groups() {
    // reframe b 解锁 >2 群体互殴：256 个 char_id 的 emergent_group 必须覆盖 ≥3 个不同群体，
    // 否则散修永远塌成 ≤2 组、退回 P1 的 2-faction 上限。同时每个群体 id < EMERGENT_GROUP_COUNT。
    let mut groups = std::collections::BTreeSet::new();
    for index in 0..256u32 {
        let g = seed_emergent_group(&format!("dormant:rogue:{index}"));
        assert!(
            g.0 < EMERGENT_GROUP_COUNT,
            "seed group id {} must be < EMERGENT_GROUP_COUNT {EMERGENT_GROUP_COUNT}",
            g.0
        );
        groups.insert(g.0);
    }
    assert!(
        groups.len() >= 3,
        "256 char_ids must spread across at least 3 distinct emergent groups to unlock >2-group melee (reframe b §十); only saw {} group(s): {:?}",
        groups.len(),
        groups
    );
}

#[test]
fn dormant_rogue_seed_snapshot_assigns_explicit_emergent_group() {
    // 防回归：seed 出来的 dormant rogue 必须带显式 emergent_group（非 None），
    // 否则离屏战斗回退 faction 派生、退化成 2 群体上限。
    let zone = zone();
    let snapshot = dormant_rogue_seed_snapshot(&zone, 0, 0, 0, 0.8, true);
    let group = snapshot
        .emergent_group
        .expect("seeded dormant rogue 必须带显式 emergent_group，不能是 None");
    assert!(
        group.0 < EMERGENT_GROUP_COUNT,
        "seed emergent group id {} must be < EMERGENT_GROUP_COUNT {EMERGENT_GROUP_COUNT}",
        group.0
    );
}

// ── plan-npc-realm-distribution-v1 P1：种群 seeder 境界分布（饱和单测） ────────

fn realm_weight(table: &[(Realm, u32); 6], realm: Realm) -> u32 {
    table
        .iter()
        .find(|(r, _)| *r == realm)
        .map(|(_, w)| *w)
        .unwrap_or(0)
}

#[test]
fn realm_distribution_tables_sum_to_exactly_1000_per_mille() {
    // 防漂移：任何一次手改分布表数值（微调长尾）如果算错导致总和不再是 1000‰，
    // `sample_rogue_seed_realm` 的累积权重循环会在权重和 < 1000 时对部分 roll
    // 值静默兜底成 Realm::Awaken（人为压低非醒灵占比），必须显式撞红而非静默偏移。
    let background_sum: u32 = REALM_DISTRIBUTION_BACKGROUND.iter().map(|(_, w)| w).sum();
    let resource_sum: u32 = REALM_DISTRIBUTION_RESOURCE.iter().map(|(_, w)| w).sum();
    assert_eq!(
        background_sum, 1000,
        "background 分布表权重和必须恰为 1000‰，实际 {background_sum}"
    );
    assert_eq!(
        resource_sum, 1000,
        "resource 分布表权重和必须恰为 1000‰，实际 {resource_sum}"
    );
}

#[test]
fn realm_distribution_tables_never_seed_void_naturally() {
    // §8.1 #1 决议：化虚不自然刷，正典稀有仅垂死大能一类特殊实体。
    assert_eq!(
        realm_weight(&REALM_DISTRIBUTION_BACKGROUND, Realm::Void),
        0,
        "background 分布表化虚权重必须为 0"
    );
    assert_eq!(
        realm_weight(&REALM_DISTRIBUTION_RESOURCE, Realm::Void),
        0,
        "resource 分布表化虚权重必须为 0"
    );
}

#[test]
fn sample_rogue_seed_realm_is_deterministic_per_char_id_and_zone_kind() {
    // 同 char_id + 同 zone 档，跨调用必须抽到同一境界（否则重启后境界分布漂移）。
    for char_id in ["dormant:rogue:0", "dormant:rogue:1", "rogue-seed:zone:42"] {
        for is_resource in [true, false] {
            let a = sample_rogue_seed_realm(char_id, is_resource);
            let b = sample_rogue_seed_realm(char_id, is_resource);
            assert_eq!(
                a, b,
                "char_id={char_id} is_resource={is_resource}: 同输入必须抽到同一境界，\
                 实际两次调用分别得到 {a:?} 和 {b:?}"
            );
        }
    }
}

#[test]
fn sample_rogue_seed_realm_differs_by_zone_kind_salt_not_faction_or_group_salt() {
    // 境界抽样必须用专属 REALM_SEED_SALT，与 seed_rogue_faction（salt=0）/
    // seed_emergent_group（salt=GROUP_SALT）错开——否则境界会和派系/群体强相关
    // （比如同一 salt 下醒灵总是分到 Attack）。用同一 char_id 三个 salt 的哈希两两
    // 不相等来证明三者独立（char_id 选一个非退化样本，避免巧合碰撞误判）。
    let char_id = "dormant:rogue:7";
    let realm_hash = deterministic_hash(char_id, REALM_SEED_SALT);
    let faction_hash = deterministic_hash(char_id, 0);
    let group_hash = deterministic_hash(char_id, GROUP_SALT);
    assert_ne!(
        realm_hash, faction_hash,
        "REALM_SEED_SALT 必须与派系 salt=0 产生不同哈希"
    );
    assert_ne!(
        realm_hash, group_hash,
        "REALM_SEED_SALT 必须与 GROUP_SALT 产生不同哈希"
    );
}

#[test]
fn sample_rogue_seed_realm_background_distribution_matches_table_within_tolerance() {
    // 统计 pin（非精确计数）：2000 个不同 char_id 在 background 档下的境界直方图，
    // 逐境界占比须落在 §8.1 #1 background 表（57/30/12/1/0/0%）±8 个百分点内。
    let n = 2000;
    let mut counts: HashMap<&'static str, u32> = HashMap::new();
    for i in 0..n {
        let realm = sample_rogue_seed_realm(&format!("tolerance:background:{i}"), false);
        let key = match realm {
            Realm::Awaken => "awaken",
            Realm::Induce => "induce",
            Realm::Condense => "condense",
            Realm::Solidify => "solidify",
            Realm::Spirit => "spirit",
            Realm::Void => "void",
        };
        *counts.entry(key).or_insert(0) += 1;
    }
    let ratio = |key: &str| *counts.get(key).unwrap_or(&0) as f64 / n as f64;
    let assert_within = |label: &str, actual: f64, expected_pct: f64| {
        let tolerance = 0.08;
        assert!(
            (actual - expected_pct / 100.0).abs() <= tolerance,
            "background {label} 占比 {actual:.3} 偏离 §8.1 #1 预期 {expected_pct}% 超过容差 \
             ±{tolerance}（N={n} 样本 counts={counts:?}）"
        );
    };
    assert_within("醒灵", ratio("awaken"), 57.0);
    assert_within("引气", ratio("induce"), 30.0);
    assert_within("凝脉", ratio("condense"), 12.0);
    assert_within("固元", ratio("solidify"), 1.0);
    assert_eq!(
        counts.get("spirit").copied().unwrap_or(0),
        0,
        "background 档通灵权重为 0，绝不应抽到"
    );
    assert_eq!(counts.get("void").copied().unwrap_or(0), 0, "化虚不自然刷");
}

#[test]
fn sample_rogue_seed_realm_resource_distribution_matches_table_within_tolerance() {
    // 统计 pin：resource 档（42.5/35/20/2/0.5/0%）——通灵样本稀少（0.5%），
    // 用更大样本量 4000 降低小概率分支的统计噪声，容差同样 ±8 个百分点
    // （通灵/固元档额外用绝对宽松上界防止偶发 0 样本导致误判）。
    let n = 4000;
    let mut counts: HashMap<&'static str, u32> = HashMap::new();
    for i in 0..n {
        let realm = sample_rogue_seed_realm(&format!("tolerance:resource:{i}"), true);
        let key = match realm {
            Realm::Awaken => "awaken",
            Realm::Induce => "induce",
            Realm::Condense => "condense",
            Realm::Solidify => "solidify",
            Realm::Spirit => "spirit",
            Realm::Void => "void",
        };
        *counts.entry(key).or_insert(0) += 1;
    }
    let ratio = |key: &str| *counts.get(key).unwrap_or(&0) as f64 / n as f64;
    let assert_within = |label: &str, actual: f64, expected_pct: f64, tolerance: f64| {
        assert!(
            (actual - expected_pct / 100.0).abs() <= tolerance,
            "resource {label} 占比 {actual:.3} 偏离 §8.1 #1 预期 {expected_pct}% 超过容差 \
             ±{tolerance}（N={n} 样本 counts={counts:?}）"
        );
    };
    assert_within("醒灵", ratio("awaken"), 42.5, 0.08);
    assert_within("引气", ratio("induce"), 35.0, 0.08);
    assert_within("凝脉", ratio("condense"), 20.0, 0.08);
    assert_within("固元", ratio("solidify"), 2.0, 0.03);
    assert_within("通灵", ratio("spirit"), 0.5, 0.02);
    assert_eq!(counts.get("void").copied().unwrap_or(0), 0, "化虚不自然刷");
    // resource 档整体高境界（凝脉+固元+通灵）占比必须明显高于 background 档，
    // 证明两张表确实不同（不是同一张表被误接了两次）。
    let resource_high = ratio("condense") + ratio("solidify") + ratio("spirit");
    assert!(
        resource_high > 0.15,
        "resource 档凝脉+固元+通灵合计占比 {resource_high:.3} 偏低，\
         §8.1 #1 预期约 22.5%，可能误接了 background 表"
    );
}

#[test]
fn dormant_rogue_seed_snapshot_realm_distribution_not_always_awaken() {
    // 端到端契约：seed 出来的 dormant snapshot 的 Cultivation.realm 不能恒为醒灵
    // （否则 P0 choke-point 修复对 dormant seeder 完全没有生效）。
    let zone = zone();
    let realms: Vec<Realm> = (0..500u32)
        .map(|i| {
            dormant_rogue_seed_snapshot(&zone, i, i, 0, 0.8, true)
                .cultivation
                .realm
        })
        .collect();
    let non_awaken = realms.iter().filter(|r| **r != Realm::Awaken).count();
    assert!(
        non_awaken > 0,
        "500 个 dormant rogue snapshot 全部落在醒灵，期望按 §8.1 #1 分布表抽到非醒灵境界；\
         这意味着 dormant_rogue_seed_snapshot 回退成了 Cultivation::default()"
    );
    assert!(
        !realms.contains(&Realm::Void),
        "dormant seeder 绝不应抽到化虚（正典稀有，不自然刷）"
    );
}

#[test]
fn dormant_rogue_seed_snapshot_meridian_system_matches_sampled_realm_required_meridians() {
    // Verify blocker pin：dormant seeder 曾恒开 1 条肺经（MeridianSystem::default()
    // + 手动开 Lung），与抽样出的 realm 脱钩——凝脉/固元/通灵抽样命中却只有 1 条脉，
    // 撞 realm↔经脉双源矛盾。500 个样本里筛出每个非醒灵境界至少一例，核对
    // meridian_system.opened_count() 恰等于 realm.required_meridians()（用生产
    // 侧的 npc_meridian_system_for_realm 派生规则，不是重新定义一套开脉逻辑）。
    let zone = zone();
    let mut seen_realms: std::collections::HashSet<Realm> = std::collections::HashSet::new();
    for i in 0..500u32 {
        let snapshot = dormant_rogue_seed_snapshot(&zone, i, i, 0, 0.8, true);
        let realm = snapshot.cultivation.realm;
        let expected = realm.required_meridians();
        let actual = snapshot.meridian_system.opened_count();
        assert_eq!(
            actual, expected,
            "i={i} realm={realm:?}: dormant seeder 落地的 meridian_system 应开 \
             {expected} 条经脉（realm.required_meridians()），实得 {actual} 条 \
             ——若恒为 1 说明退回了 P0-era 恒开肺经的 bug"
        );
        let expected_system = crate::npc::technique::npc_meridian_system_for_realm(
            realm,
            crate::body_plan::humanoid_plan_static(),
        );
        let opened_mismatch = snapshot
            .meridian_system
            .iter()
            .zip(expected_system.iter())
            .enumerate()
            .find(|(_, (actual, expected))| actual.opened != expected.opened);
        assert!(
            opened_mismatch.is_none(),
            "i={i} realm={realm:?}: dormant seeder 的 meridian_system 必须与生产侧 \
             npc_meridian_system_for_realm(realm) 逐脉一致（同一份派生规则的单一来源），\
             首个不一致的经脉 index={opened_mismatch:?}"
        );
        seen_realms.insert(realm);
    }
    assert!(
        seen_realms.contains(&Realm::Condense) || seen_realms.contains(&Realm::Solidify),
        "500 个 resource 档样本应至少抽到一例凝脉或固元，否则本测试没有真正覆盖 \
         required_meridians()>1 的分支（fixture 完整性）；实抽到 {seen_realms:?}"
    );
}

#[test]
fn dormant_rogue_seed_snapshot_qi_current_stays_zero_regardless_of_sampled_realm() {
    // 守恒红线：无论抽到哪个境界，qi_current 必须保持 0.0（不满灵）——qi_max_for_realm
    // 只设容量上限，真元靠既有 apply_dormant_regen_with_multiplier 从 zone 逐步吸收；
    // spawn 时满灵会凭空产生真元，撞 qi_physics 守恒红线。qi_max 必须等于
    // qi_max_for_realm(抽到的 realm)，不能停留在 Cultivation::default() 的 10.0。
    let zone = zone();
    for i in 0..200u32 {
        let snapshot = dormant_rogue_seed_snapshot(&zone, i, i, 0, 0.8, true);
        assert_eq!(
            snapshot.cultivation.qi_current, 0.0,
            "index={i} realm={:?}: qi_current 必须恒 0.0（不满灵）",
            snapshot.cultivation.realm
        );
        assert_eq!(
            snapshot.cultivation.qi_max,
            qi_max_for_realm(snapshot.cultivation.realm),
            "index={i} realm={:?}: qi_max 必须等于 qi_max_for_realm(realm)，不能是 \
             Cultivation::default() 的醒灵默认值",
            snapshot.cultivation.realm
        );
    }
}

#[test]
fn dormant_rogue_seed_snapshot_same_seed_twice_produces_identical_realm() {
    // 确定性 pin：同 seed（同 zone/index）两次 genesis 必须逐 NPC realm 一致。
    let zone = zone();
    for i in 0..50u32 {
        let a = dormant_rogue_seed_snapshot(&zone, i, i, 0, 0.8, true);
        let b = dormant_rogue_seed_snapshot(&zone, i, i, 0, 0.8, true);
        assert_eq!(
            a.cultivation.realm, b.cultivation.realm,
            "index={i}: 同 seed 两次调用 dormant_rogue_seed_snapshot 必须得到相同 realm，\
             实际 {:?} vs {:?}",
            a.cultivation.realm, b.cultivation.realm
        );
    }
}

#[test]
fn dormant_rogue_seed_snapshot_resource_vs_background_flag_changes_distribution() {
    // is_resource_zone 标志必须真正切换分布表：同一批 index 在 resource=true 下
    // 高境界（凝脉起）占比必须明显高于 resource=false（否则该参数被忽略/接反）。
    let zone = zone();
    let n = 1000u32;
    let count_high = |is_resource: bool| {
        (0..n)
            .filter(|&i| {
                let realm = dormant_rogue_seed_snapshot(&zone, i, i, 0, 0.8, is_resource)
                    .cultivation
                    .realm;
                matches!(realm, Realm::Condense | Realm::Solidify | Realm::Spirit)
            })
            .count()
    };
    let resource_high = count_high(true);
    let background_high = count_high(false);
    assert!(
        resource_high > background_high,
        "resource 档凝脉+固元+通灵计数 {resource_high} 必须明显高于 background 档 \
         {background_high}（N={n}），否则 is_resource_zone 参数未真正接线"
    );
}

// ── plan-offscreen-war-v1 P2：离屏派系互殴战死闭环（饱和单测） ─────────────
//
// 测契约不测实现：断言 store 人口回写 / 真元守恒回灌 / death notice 死因 / telemetry
// outcome / 防吞真元 retain，全部走 `run_dormant_combat_phase` 这个真实结算入口
// （而非私有中间步），接入面变了也不应红。

use crate::qi_physics::WorldQiAccount;

/// 一帧 `dormant_global_tick_system` 后收回的全部相关 event（P2 死亡/战果 + P3 待物化遗物）。
/// 用具名 struct 而非裸三元组，让断言读起来是 `events.relics` 而非 `.2`。
struct CombatTickEvents {
    deaths: Vec<NpcDeathNotice>,
    outcomes: Vec<DormantCombatOutcome>,
    relics: Vec<PendingDormantRelicCreated>,
}

/// 造一个**敌对、满真元、不会自然老死**的战斗候选快照：给定 char_id / 派系 / 真元。
/// realm 拉到 Condense 让 condition_factor 由满血+满真元主导，战力 realm-monotonic。
fn combat_snapshot(
    char_id: &str,
    faction: FactionId,
    qi_current: f64,
    pos: DVec3,
) -> NpcDormantSnapshot {
    let cultivation = Cultivation {
        realm: Realm::Condense,
        qi_current,
        qi_max: 60.0,
        ..Default::default()
    };
    NpcDormantSnapshot {
        char_id: char_id.to_string(),
        archetype: NpcArchetype::Rogue,
        dimension: DimensionKind::Overworld,
        zone_name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
        position: vec3_to_array(pos),
        schedule_seed: None,
        cultivation: cultivation.clone(),
        meridian_system: MeridianSystem::default(),
        meridian_severed: MeridianSeveredPermanent::default(),
        contamination: Contamination::default(),
        // 长寿命 + 0 起始年龄：本轮绝不自然老死，让"死人"唯一来源是战斗。
        lifespan: NpcLifespan::new(0.0, 1_000_000.0),
        shared_lifespan: LifespanComponent::for_realm(cultivation.realm),
        lifespan_extension_ledger: LifespanExtensionLedger::default(),
        death_registry: DeathRegistry::new(char_id),
        life_record: LifeRecord::new(char_id),
        memory: None,
        player_reputation: None,
        faction: Some(FactionMembership {
            faction_id: faction,
            rank: FactionRank::Disciple,
            reputation: Reputation::default(),
            lineage: None,
            mission_queue: MissionQueue::default(),
        }),
        // 显式群体留空：combat 候选默认走 faction 派生（Attack→0 / Defend→1），覆盖
        // 非破坏迁移路径；需要显式群体的测试单独 set 该字段。
        emergent_group: None,
        patrol: None,
        loot_table: None,
        guardian_relic: None,
        mimic_spider: None,
        tsy_hostile: None,
        tsy_sentinel: None,
        intent: DormantBehaviorIntent::Cultivate {
            zone: DEFAULT_SPAWN_ZONE_NAME.to_string(),
        },
        dormant_since_tick: 0,
        last_dormant_tick_processed: 0,
        initial_qi: qi_current,
        qi_ledger_net: 0.0,
        combat_dead_pending_release: false,
        pending_combat_winner: None,
    }
}

fn with_daozhan_owner(mut snapshot: NpcDormantSnapshot, daozhan_qi: f64) -> NpcDormantSnapshot {
    snapshot.tsy_hostile = Some(DormantTsyHostileSnapshot {
        family_id: "family-a".to_string(),
        zhinian_phase: None,
        zhinian_phase_entered_at_tick: None,
        fuya_aura: None,
        daoxiang_origin: None,
        daozhan: Some(DormantDaozhanSnapshot {
            state: DaoZhangState::default(),
            home_zone: snapshot.zone_name.clone(),
            home_pos: snapshot.position,
            daozhan_qi,
            origin_realm: None,
            behavior_queue: Vec::new(),
            current_behavior_ticks: 0,
        }),
    });
    snapshot
}

/// 跑一次完整 `dormant_global_tick_system`（含 combat phase）并收回本帧 death + outcome。
///
/// 用真实 `App` 驱动整条系统（最贴近运行时），把传入的 store/zones/ledger 装进资源，
/// `update()` 一帧，读回 `Events<NpcDeathNotice>` / `Events<DormantCombatOutcome>`，
/// 再把更新后的 store/zones/ledger 写回调用方引用。tick 通过 `GameTick` 注入；
/// `dormant_tick_interval_ticks=1` 保证整周期、必跑。返回 (deaths, outcomes)。
///
/// 关键：所有 combat snapshot 的 `last_dormant_tick_processed=0`，本帧 aging 会推进它们；
/// 但 combat snapshot 寿命 1_000_000 ticks，aging 不会触发自然老死，故死亡唯一来源是 combat。
fn run_combat_tick(
    store: &mut NpcDormantStore,
    zones: &mut ZoneRegistry,
    ledger: &mut WorldQiAccount,
    config: &NpcVirtualizationConfig,
    tick: u64,
) -> CombatTickEvents {
    let mut app = App::new();
    app.add_event::<NpcDeathNotice>();
    app.add_event::<DormantCombatOutcome>();
    app.add_event::<PendingDormantRelicCreated>();
    app.insert_resource(NpcVirtualizationConfig {
        dormant_tick_interval_ticks: 1,
        ..config.clone()
    });
    app.insert_resource(GameTick(tick as u32));
    app.insert_resource(FactionStore::default());
    app.insert_resource(std::mem::take(store));
    app.insert_resource(ZoneRegistry {
        spatial_revision: 0,
        zones: std::mem::take(&mut zones.zones),
    });
    app.insert_resource(std::mem::take(ledger));
    app.add_systems(Update, dormant_global_tick_system);

    app.update();

    let deaths = app
        .world_mut()
        .resource_mut::<Events<NpcDeathNotice>>()
        .drain()
        .collect::<Vec<_>>();
    let outcomes = app
        .world_mut()
        .resource_mut::<Events<DormantCombatOutcome>>()
        .drain()
        .collect::<Vec<_>>();
    let relics = app
        .world_mut()
        .resource_mut::<Events<PendingDormantRelicCreated>>()
        .drain()
        .collect::<Vec<_>>();

    // 写回更新后的资源到调用方引用。
    *store = app
        .world_mut()
        .remove_resource::<NpcDormantStore>()
        .unwrap();
    *zones = app.world_mut().remove_resource::<ZoneRegistry>().unwrap();
    *ledger = app.world_mut().remove_resource::<WorldQiAccount>().unwrap();
    CombatTickEvents {
        deaths,
        outcomes,
        relics,
    }
}

fn acknowledge_dormant_persistence(store: &mut NpcDormantStore) {
    let revision = store
        .begin_persistence()
        .expect("dirty dormant state must start one persistence revision");
    store
        .persistence_receipt_sender()
        .send(crate::network::redis_bridge::RedisDeliveryReceipt {
            delivery_id: revision.to_string(),
            outcome: Ok(()),
        })
        .expect("dormant persistence receipt channel must remain connected");
}

fn run_combat_to_completion(
    store: &mut NpcDormantStore,
    zones: &mut ZoneRegistry,
    ledger: &mut WorldQiAccount,
    config: &NpcVirtualizationConfig,
    tick: u64,
) -> CombatTickEvents {
    let pending = run_combat_tick(store, zones, ledger, config, tick);
    if !pending.deaths.is_empty()
        || !pending.outcomes.is_empty()
        || !pending.relics.is_empty()
        || !store
            .snapshots
            .values()
            .any(|snapshot| snapshot.combat_dead_pending_release)
    {
        return pending;
    }
    acknowledge_dormant_persistence(store);
    run_combat_tick(store, zones, ledger, config, tick)
}

#[test]
fn combat_terminal_events_wait_for_pending_hash_receipt() {
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone()],
    };
    let zone_before = zones.zones[0].spirit_qi;
    let mut store = NpcDormantStore::default();
    store.insert(combat_snapshot(
        "atk",
        FactionId::Attack,
        5.0,
        DVec3::new(10.0, 64.0, 10.0),
    ));
    store.insert(combat_snapshot(
        "def",
        FactionId::Defend,
        5.0,
        DVec3::new(11.0, 64.0, 11.0),
    ));
    store.take_dirty();
    let mut ledger = WorldQiAccount::default();
    let config = NpcVirtualizationConfig::default();

    let first = run_combat_tick(&mut store, &mut zones, &mut ledger, &config, 7);
    assert!(first.deaths.is_empty() && first.outcomes.is_empty());
    assert_eq!(store.len(), 2);
    assert!(store
        .snapshots
        .values()
        .any(|snapshot| snapshot.combat_dead_pending_release));
    let owner_total_before_retry = physical_owner_total(&store, &zones, &ledger);

    let unconfirmed = run_combat_tick(&mut store, &mut zones, &mut ledger, &config, 8);
    assert!(unconfirmed.deaths.is_empty() && unconfirmed.outcomes.is_empty());
    assert_eq!(store.len(), 2);
    assert_eq!(zones.zones[0].spirit_qi, zone_before);
    assert!(ledger.transfers().is_empty());
    assert_eq!(
        physical_owner_total(&store, &zones, &ledger),
        owner_total_before_retry
    );

    acknowledge_dormant_persistence(&mut store);
    let confirmed = run_combat_tick(&mut store, &mut zones, &mut ledger, &config, 9);
    assert_eq!(confirmed.deaths.len(), 1);
    assert_eq!(confirmed.outcomes.len(), 1);
    assert_eq!(store.len(), 1);
    assert_eq!(
        physical_owner_total(&store, &zones, &ledger),
        owner_total_before_retry
    );
}

#[test]
fn failed_pending_combat_hash_receipt_preserves_all_terminal_state_until_retry_succeeds() {
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone()],
    };
    let zone_before = zones.zones[0].spirit_qi;
    let mut store = NpcDormantStore::default();
    store.insert(with_daozhan_owner(
        combat_snapshot("atk", FactionId::Attack, 5.0, DVec3::new(10.0, 64.0, 10.0)),
        1.0,
    ));
    store.insert(with_daozhan_owner(
        combat_snapshot("def", FactionId::Defend, 5.0, DVec3::new(11.0, 64.0, 11.0)),
        1.0,
    ));
    store.take_dirty();
    let mut ledger = WorldQiAccount::default();
    let config = NpcVirtualizationConfig::default();

    let pending = run_combat_tick(&mut store, &mut zones, &mut ledger, &config, 7);
    assert!(pending.deaths.is_empty());
    assert!(pending.outcomes.is_empty());
    assert!(pending.relics.is_empty());
    let loser_id = store
        .snapshots
        .iter()
        .find_map(|(char_id, snapshot)| {
            snapshot
                .combat_dead_pending_release
                .then_some(char_id.clone())
        })
        .expect("combat tick must persist one pending loser");
    let owner_total_before_receipt = physical_owner_total(&store, &zones, &ledger);
    let revision = store
        .begin_persistence()
        .expect("pending combat mutation must start a HASH revision");
    store
        .persistence_receipt_sender()
        .send(crate::network::redis_bridge::RedisDeliveryReceipt {
            delivery_id: revision.to_string(),
            outcome: Err("redis unavailable".to_string()),
        })
        .unwrap();

    let failed = run_combat_tick(&mut store, &mut zones, &mut ledger, &config, 8);

    assert!(failed.deaths.is_empty());
    assert!(failed.outcomes.is_empty());
    assert!(failed.relics.is_empty());
    assert_eq!(store.persistence.persisted_revision, 0);
    assert!(store.is_dirty());
    let retained = &store.snapshots[&loser_id];
    assert!(retained.combat_dead_pending_release);
    assert!(retained.pending_combat_winner.is_some());
    assert!(retained.cultivation.qi_current > 0.0);
    assert!(retained
        .tsy_hostile
        .as_ref()
        .and_then(|hostile| hostile.daozhan.as_ref())
        .is_some_and(|daozhan| daozhan.daozhan_qi > 0.0));
    assert_eq!(zones.zones[0].spirit_qi, zone_before);
    assert!(ledger.transfers().is_empty());
    assert_eq!(
        physical_owner_total(&store, &zones, &ledger),
        owner_total_before_receipt
    );

    assert_eq!(
        store.begin_persistence(),
        Some(revision),
        "failed receipt must retry the same pending mutation revision"
    );
    store
        .persistence_receipt_sender()
        .send(crate::network::redis_bridge::RedisDeliveryReceipt {
            delivery_id: revision.to_string(),
            outcome: Ok(()),
        })
        .unwrap();
    let committed = run_combat_tick(&mut store, &mut zones, &mut ledger, &config, 9);

    assert_eq!(committed.deaths.len(), 1);
    assert_eq!(committed.outcomes.len(), 1);
    assert_eq!(committed.relics.len(), 1);
    assert!(!store.snapshots.contains_key(&loser_id));
    assert_eq!(store.persistence.persisted_revision, revision);
    assert_eq!(ledger.transfers().len(), 2);
    assert_eq!(
        physical_owner_total(&store, &zones, &ledger),
        owner_total_before_receipt
    );
}

#[test]
fn combat_terminal_settles_daozhan_cultivation_and_drain_owners_after_hash_receipt() {
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone()],
    };
    let owner_total_before = zones.zones[0].spirit_qi * QI_ZONE_UNIT_CAPACITY + 12.0;
    let mut store = NpcDormantStore::default();
    store.insert(with_daozhan_owner(
        combat_snapshot("atk", FactionId::Attack, 5.0, DVec3::new(10.0, 64.0, 10.0)),
        1.0,
    ));
    store.insert(with_daozhan_owner(
        combat_snapshot("def", FactionId::Defend, 5.0, DVec3::new(11.0, 64.0, 11.0)),
        1.0,
    ));
    store.take_dirty();
    let mut ledger = WorldQiAccount::default();
    let config = NpcVirtualizationConfig::default();

    let pending = run_combat_tick(&mut store, &mut zones, &mut ledger, &config, 7);
    assert!(pending.deaths.is_empty() && pending.outcomes.is_empty());
    assert_eq!(store.len(), 2);
    assert!(ledger.transfers().is_empty());

    acknowledge_dormant_persistence(&mut store);
    let committed = run_combat_tick(&mut store, &mut zones, &mut ledger, &config, 8);

    assert_eq!(committed.deaths.len(), 1);
    assert_eq!(committed.outcomes.len(), 1);
    assert_eq!(store.len(), 1);
    assert_eq!(
        ledger.transfers().len(),
        2,
        "combat loser cultivation and Daozhan owners must each emit one typed release"
    );
    assert!((physical_owner_total(&store, &zones, &ledger) - owner_total_before).abs() < 1e-9);
}

#[test]
fn legacy_pending_combat_without_winner_still_settles_and_terminates() {
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone()],
    };
    let mut legacy = with_daozhan_owner(
        combat_snapshot(
            "legacy-loser",
            FactionId::Attack,
            5.0,
            DVec3::new(10.0, 64.0, 10.0),
        ),
        1.0,
    );
    legacy.combat_dead_pending_release = true;
    legacy.pending_combat_winner = None;
    let mut store = NpcDormantStore::default();
    store.insert(legacy);
    acknowledge_dormant_persistence(&mut store);
    let mut ledger = WorldQiAccount::default();

    let committed = run_combat_tick(
        &mut store,
        &mut zones,
        &mut ledger,
        &NpcVirtualizationConfig::default(),
        8,
    );

    assert!(!store.contains("legacy-loser"));
    assert_eq!(committed.deaths.len(), 1);
    assert!(committed.outcomes.is_empty());
    assert_eq!(ledger.transfers().len(), 2);
}

#[test]
fn combat_death_releases_all_qi_to_zone() {
    // 一对敌对 dormant 在同 zone：战死一方的真元应**守恒回灌**给 zone（zone.spirit_qi 上升），
    // 且回灌量 == ledger transfer amount == outcome.qi_released。
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone()],
    }; // spirit_qi=0.8
    let zone_before = zones.zones[0].spirit_qi;
    let mut store = NpcDormantStore::default();
    store.insert(combat_snapshot(
        "atk",
        FactionId::Attack,
        5.0,
        DVec3::new(10.0, 64.0, 10.0),
    ));
    store.insert(combat_snapshot(
        "def",
        FactionId::Defend,
        5.0,
        DVec3::new(11.0, 64.0, 11.0),
    ));
    store.take_dirty();
    let mut ledger = WorldQiAccount::default();
    let config = NpcVirtualizationConfig::default();

    let CombatTickEvents {
        deaths, outcomes, ..
    } = run_combat_to_completion(&mut store, &mut zones, &mut ledger, &config, 7);

    assert_eq!(
        deaths.len(),
        1,
        "一对敌对 dormant 必须恰好死一个（战斗必致死），实际死 {} 个",
        deaths.len()
    );
    assert_eq!(
        outcomes.len(),
        1,
        "恰好一条战果 outcome 对应这场战死，实际 {} 条",
        outcomes.len()
    );
    let zone_after = zones.zones[0].spirit_qi;
    assert!(
        zone_after > zone_before,
        "战死方真元必须守恒回灌 zone（spirit_qi 上升）：before={zone_before} after={zone_after}"
    );
    // 回灌量精确对账：zone 上升的归一化量 × 容量 == outcome.qi_released。
    let zone_gain_abs = (zone_after - zone_before) * QI_ZONE_UNIT_CAPACITY;
    assert!(
        (zone_gain_abs - outcomes[0].qi_released).abs() < 1e-9,
        "zone 真元增量（{zone_gain_abs}）必须等于 outcome.qi_released（{}），否则 telemetry 与实际守恒不符",
        outcomes[0].qi_released
    );
    assert!(
        outcomes[0].qi_released > 0.0,
        "本场战死应有真元回灌（败者满真元 + zone 未满），qi_released={}",
        outcomes[0].qi_released
    );
}

#[test]
fn combat_death_emits_notice_with_combat_reason_and_pos() {
    // 战死 notice 必须 reason=Combat + from_dormant_combat=true + pos=Some(战场坐标)，
    // 让 agent / e2e 能把战死与自然老死区分开。
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone()],
    };
    let loser_pos = DVec3::new(11.0, 64.0, 11.0);
    let mut store = NpcDormantStore::default();
    store.insert(combat_snapshot(
        "atk",
        FactionId::Attack,
        5.0,
        DVec3::new(10.0, 64.0, 10.0),
    ));
    store.insert(combat_snapshot("def", FactionId::Defend, 5.0, loser_pos));
    let mut ledger = WorldQiAccount::default();
    let config = NpcVirtualizationConfig::default();

    let CombatTickEvents {
        deaths, outcomes, ..
    } = run_combat_to_completion(&mut store, &mut zones, &mut ledger, &config, 7);

    let notice = &deaths[0];
    assert_eq!(
        notice.reason,
        NpcDeathReason::Combat,
        "战死 notice 的 reason 必须是 Combat（非 NaturalAging），否则 agent 当成老死，实际 {:?}",
        notice.reason
    );
    assert!(
        notice.from_dormant_combat,
        "战死 notice 的 from_dormant_combat 必须为 true（区别于在场战斗 / 老死）"
    );
    assert!(
        notice.pos.is_some(),
        "战死 notice 必须带 pos（战场坐标），供 agent 派系战报定位 / e2e 断言"
    );
    // notice 的死者就是 outcome 的 loser，且 pos 来自该败者快照。
    assert_eq!(
        notice.npc_id, outcomes[0].loser,
        "death notice 的 npc_id 必须 == outcome.loser（同一场战死的两面）"
    );
}

#[test]
fn combat_death_removes_loser_from_store() {
    // 战死方真元全释放后必须从 store 移除（人口回写），胜者保留。
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone()],
    };
    let mut store = NpcDormantStore::default();
    store.insert(combat_snapshot(
        "atk",
        FactionId::Attack,
        5.0,
        DVec3::new(10.0, 64.0, 10.0),
    ));
    store.insert(combat_snapshot(
        "def",
        FactionId::Defend,
        5.0,
        DVec3::new(11.0, 64.0, 11.0),
    ));
    let mut ledger = WorldQiAccount::default();
    let config = NpcVirtualizationConfig::default();

    let CombatTickEvents { deaths, .. } =
        run_combat_to_completion(&mut store, &mut zones, &mut ledger, &config, 7);

    assert_eq!(
        store.len(),
        1,
        "两个 dormant 战斗后必须剩 1 个（败者真元全释放 → 移除、胜者留），实际剩 {}",
        store.len()
    );
    let loser = &deaths[0].npc_id;
    assert!(
        !store.contains(loser),
        "战死方 `{loser}` 必须已从 store 移除"
    );
    // 剩下的就是胜者。
    let winner_id = if loser == "atk" { "def" } else { "atk" };
    assert!(
        store.contains(winner_id),
        "胜者 `{winner_id}` 必须仍在 store（未参与死亡）"
    );
}

#[test]
fn winner_qi_unchanged() {
    // dormant 简化：胜者真元不变（未流动即未失衡，§10.1 #5 ③）。
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone()],
    };
    let mut store = NpcDormantStore::default();
    store.insert(combat_snapshot(
        "atk",
        FactionId::Attack,
        5.0,
        DVec3::new(10.0, 64.0, 10.0),
    ));
    store.insert(combat_snapshot(
        "def",
        FactionId::Defend,
        5.0,
        DVec3::new(11.0, 64.0, 11.0),
    ));
    let mut ledger = WorldQiAccount::default();
    let config = NpcVirtualizationConfig::default();

    let CombatTickEvents { deaths, .. } =
        run_combat_to_completion(&mut store, &mut zones, &mut ledger, &config, 7);

    let loser = &deaths[0].npc_id;
    let winner_id = if loser == "atk" { "def" } else { "atk" };
    let winner = store.snapshots.get(winner_id).expect("胜者必须仍在 store");
    assert!(
        (winner.cultivation.qi_current - 5.0).abs() < 1e-9,
        "胜者真元必须保持 5.0 不变（dormant 简化不扣胜者），实际 {}",
        winner.cultivation.qi_current
    );
}

#[test]
fn zone_full_settles_loser_into_fixed_overflow_without_shadow() {
    let mut full_zone = zone();
    full_zone.spirit_qi = 1.0;
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![full_zone],
    };
    let mut store = NpcDormantStore::default();
    store.insert(combat_snapshot(
        "atk",
        FactionId::Attack,
        5.0,
        DVec3::new(10.0, 64.0, 10.0),
    ));
    store.insert(combat_snapshot(
        "def",
        FactionId::Defend,
        5.0,
        DVec3::new(11.0, 64.0, 11.0),
    ));
    let mut ledger = WorldQiAccount::default();
    let config = NpcVirtualizationConfig::default();

    let CombatTickEvents {
        deaths, outcomes, ..
    } = run_combat_to_completion(&mut store, &mut zones, &mut ledger, &config, 7);

    assert_eq!(deaths.len(), 1);
    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].qi_released, 0.0);
    assert_eq!(store.len(), 1, "a durably settled loser must be removed");
    assert!(
        (ledger.balance(&crate::qi_physics::qi_flow_overflow_account()) - 5.0).abs() < 1e-9,
        "full-zone death must move every rejected qi unit into fixed durable overflow"
    );
    assert!(
        !ledger.iter_balances().any(|(account, _)| matches!(
            account.kind,
            crate::qi_physics::QiAccountKind::Npc | crate::qi_physics::QiAccountKind::Zone
        )),
        "full-zone settlement must not leave NPC or Zone ledger shadows"
    );
}

#[test]
fn settlement_failure_retains_loser_and_marks_store_dirty() {
    const RUN_TICK: u64 = 9;

    let mut invalid_zone = zone();
    invalid_zone.spirit_qi = f64::NAN;
    let mut atk = combat_snapshot("atk", FactionId::Attack, 5.0, DVec3::new(10.0, 64.0, 10.0));
    let mut def = combat_snapshot("def", FactionId::Defend, 5.0, DVec3::new(11.0, 64.0, 11.0));
    atk.last_dormant_tick_processed = RUN_TICK;
    def.last_dormant_tick_processed = RUN_TICK;

    let mut store = NpcDormantStore::default();
    store.insert(atk);
    store.insert(def);

    let mut app = App::new();
    app.add_event::<NpcDeathNotice>();
    app.add_event::<DormantCombatOutcome>();
    app.add_event::<PendingDormantRelicCreated>();
    app.insert_resource(NpcVirtualizationConfig {
        dormant_tick_interval_ticks: 1,
        ..NpcVirtualizationConfig::default()
    });
    app.insert_resource(GameTick(RUN_TICK as u32));
    app.insert_resource(FactionStore::default());
    app.insert_resource(ZoneRegistry {
        spatial_revision: 0,
        zones: vec![invalid_zone],
    });
    app.insert_resource(WorldQiAccount::default());
    store.take_dirty();
    app.insert_resource(store);
    app.add_systems(Update, dormant_global_tick_system);

    app.update();

    let mut store = app
        .world_mut()
        .remove_resource::<NpcDormantStore>()
        .expect("store resource must survive the tick");
    assert_eq!(
        store.len(),
        2,
        "an invalid signed-zone owner must fail closed and retain the dead snapshot"
    );
    let retained_loser = store
        .snapshots
        .values()
        .find(|snapshot| snapshot.combat_dead_pending_release)
        .expect(
            "the logically dead loser must be excluded from future combat while settlement retries",
        );
    assert!(
        retained_loser.cultivation.qi_current() > QI_EPSILON,
        "failed settlement must leave the dormant physical owner untouched"
    );
    assert!(
        store.take_dirty(),
        "retaining a logically dead snapshot must persist the retry marker"
    );
}

#[test]
fn sequential_release_no_overflow() {
    // 同 zone 多败者：顺序 release（先 release 抬高 zone_qi，后者读更高基线），
    // 总回灌量受 zone 容量 clamp、不溢出。两对敌对 dormant → 两个败者同 zone 回灌。
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone()],
    }; // spirit_qi=0.8
    let zone_before = zones.zones[0].spirit_qi;
    let mut store = NpcDormantStore::default();
    // 两对：a-b、c-d，全在同 zone（升序两两配对 → (a,b)+(c,d)）。
    store.insert(combat_snapshot(
        "a",
        FactionId::Attack,
        3.0,
        DVec3::new(10.0, 64.0, 10.0),
    ));
    store.insert(combat_snapshot(
        "b",
        FactionId::Defend,
        3.0,
        DVec3::new(11.0, 64.0, 11.0),
    ));
    store.insert(combat_snapshot(
        "c",
        FactionId::Attack,
        3.0,
        DVec3::new(12.0, 64.0, 12.0),
    ));
    store.insert(combat_snapshot(
        "d",
        FactionId::Defend,
        3.0,
        DVec3::new(13.0, 64.0, 13.0),
    ));
    let mut ledger = WorldQiAccount::default();
    // max_combats_per_zone 默认 3 ≥ 2，足够配出两对。
    let config = NpcVirtualizationConfig::default();

    let CombatTickEvents {
        deaths, outcomes, ..
    } = run_combat_to_completion(&mut store, &mut zones, &mut ledger, &config, 7);

    assert_eq!(
        deaths.len(),
        2,
        "两对敌对 dormant 必须死两个（每对死一个），实际 {}",
        deaths.len()
    );
    let zone_after = zones.zones[0].spirit_qi;
    assert!(
        zone_after <= 1.0 + 1e-9,
        "顺序回灌后 zone.spirit_qi 必须 ≤ 1.0（容量 clamp，不溢出），实际 {zone_after}"
    );
    // 两条 outcome 的 qi_released 之和 == zone 实际上升的绝对量（顺序无关、不丢不溢）。
    let total_released: f64 = outcomes.iter().map(|o| o.qi_released).sum();
    let zone_gain_abs = (zone_after - zone_before) * QI_ZONE_UNIT_CAPACITY;
    assert!(
        (total_released - zone_gain_abs).abs() < 1e-9,
        "两个败者顺序回灌总量（{total_released}）必须精确等于 zone 上升绝对量（{zone_gain_abs}），\
         否则顺序 release 有丢失 / 重复计数",
    );
}

#[test]
fn offscreen_war_conserves_physical_owner_total_without_actor_or_zone_shadows() {
    // R5 owner model: dormant Cultivation + signed Zone.spirit_qi + durable stable pools
    // are the physical owners. Player/NPC/Zone ledger mirrors must not be seeded or left behind.
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone()],
    };
    let mut store = NpcDormantStore::default();
    const N: u32 = 500;
    for i in 0..N {
        let faction = if i % 2 == 0 {
            FactionId::Attack
        } else {
            FactionId::Defend
        };
        store.insert(combat_snapshot(
            &format!("w{i:04}"),
            faction,
            4.0,
            DVec3::new(10.0 + (i % 50) as f64, 64.0, 10.0 + (i / 50) as f64),
        ));
    }

    let mut ledger = WorldQiAccount::default();
    let before = physical_owner_total(&store, &zones, &ledger);
    let initial_pop = store.len();
    let config = NpcVirtualizationConfig {
        max_combats_per_zone: 64,
        sim_seed: 20_260_531,
        ..Default::default()
    };
    let mut total_deaths = 0usize;
    for round in 0..10u64 {
        let CombatTickEvents { deaths, .. } =
            run_combat_to_completion(&mut store, &mut zones, &mut ledger, &config, round + 1);
        total_deaths += deaths.len();
        let mid = physical_owner_total(&store, &zones, &ledger);
        assert!(
            (before - mid).abs() < 1e-9,
            "round {round} changed dormant+zone+stable-owner qi: before={before}, after={mid}, deaths={total_deaths}"
        );
        assert_no_dormant_or_zone_shadows(&store, &zones, &ledger);
    }

    assert!(
        total_deaths > 0,
        "multi-round dormant combat must settle real deaths; got none"
    );
    assert!(
        initial_pop - store.len() <= total_deaths,
        "removed dormant population cannot exceed declared combat deaths"
    );
    assert!(
        (before - physical_owner_total(&store, &zones, &ledger)).abs() < 1e-9,
        "final dormant+zone+stable-owner qi total drifted"
    );
}

// ── plan-offscreen-war-v1 P3：克制式战场遗物结算（守恒 + 克制 + 时序窗口） ────

/// 造一个**指定 archetype** 的战斗候选快照（其余同 `combat_snapshot`）。
/// 用于 P3 区分"知名战死者（Disciple/GuardianRelic/有派系）留遗物 vs 普通 rogue 不留"。
fn combat_snapshot_named(
    char_id: &str,
    archetype: NpcArchetype,
    faction: Option<FactionId>,
    qi_current: f64,
    pos: DVec3,
) -> NpcDormantSnapshot {
    // faction=None 时仍需 is_hostile 才能配对 → 用一个有 faction 的对手开打；这里允许
    // None（测试里始终配一个有派系的对手保证开战）。
    let mut snap = combat_snapshot(
        char_id,
        faction.unwrap_or(FactionId::Attack),
        qi_current,
        pos,
    );
    snap.archetype = archetype;
    snap.faction = faction.map(|faction_id| FactionMembership {
        faction_id,
        rank: FactionRank::Disciple,
        reputation: Reputation::default(),
        lineage: None,
        mission_queue: MissionQueue::default(),
    });
    snap
}

#[test]
fn combat_death_emits_pending_relic_for_named_disciple() {
    // 一对敌对 dormant，败者是 Disciple（必留遗物）。真元充足 zone（spirit_qi=0.8）→ 全额
    // 释放 → 败者本轮移除 → 遗物 event 必 emit。遗物字段（zone/pos/archetype/seed）正确。
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone()],
    };
    let mut store = NpcDormantStore::default();
    store.insert(combat_snapshot_named(
        "fallen_disciple",
        NpcArchetype::Disciple,
        Some(FactionId::Attack),
        5.0,
        DVec3::new(10.0, 64.0, 10.0),
    ));
    store.insert(combat_snapshot_named(
        "rival",
        NpcArchetype::Disciple,
        Some(FactionId::Defend),
        5.0,
        DVec3::new(11.0, 64.0, 11.0),
    ));
    store.take_dirty();
    let mut ledger = WorldQiAccount::default();
    let config = NpcVirtualizationConfig::default();

    let events = run_combat_to_completion(&mut store, &mut zones, &mut ledger, &config, 7);

    assert_eq!(
        events.deaths.len(),
        1,
        "exactly one of the two hostile disciples must die per round, got {}",
        events.deaths.len()
    );
    // 两名都是 Disciple ⇒ 无论谁死都该留遗物。
    assert_eq!(
        events.relics.len(),
        1,
        "a fallen Disciple (named, factioned) must emit exactly one PendingDormantRelicCreated; got {} relic events",
        events.relics.len()
    );
    let relic = &events.relics[0];
    let loser_id = &events.deaths[0].npc_id;
    assert_eq!(
        &relic.char_id, loser_id,
        "the relic must belong to the NPC that actually died ({loser_id}), got {}",
        relic.char_id
    );
    assert_eq!(
        relic.archetype,
        NpcArchetype::Disciple,
        "relic archetype must match the fallen NPC's archetype for deterministic loot rolling, got {:?}",
        relic.archetype
    );
    assert_eq!(
        relic.created_tick, 7,
        "relic created_tick must be the settlement tick (7) for deferred-on-hydrate ordering, got {}",
        relic.created_tick
    );
    // loot_seed 必须 == relic_loot_seed(loser, tick, sim_seed)（确定性可复现）。
    let expected_seed = combat::relic_loot_seed(loser_id, 7, config.sim_seed);
    assert_eq!(
        relic.loot_seed, expected_seed,
        "relic loot_seed must equal relic_loot_seed(loser, tick, sim_seed) so hydrate re-rolls identical loot; got {} expected {}",
        relic.loot_seed, expected_seed
    );
}

#[test]
fn no_relic_emitted_for_factionless_rogue_through_full_combat_tick() {
    // 端到端：两名 **Awaken 无派系** Rogue 无法用 faction 配对开打。为让它们真的开战且
    // 验证"打死了但不留遗物"，借 collect 的现实：配对需 faction。故构造"一名有派系的
    // 高手 vs 一名普通无名 rogue"会让无名方有概率活/死，难确定性断言无名方一定死。
    // 改为更干净的契约锁：两名 **有派系但凝脉（ordinal 2 < 固元）** 的 Rogue 互殴——
    // 它们因 faction 而配对 + 因 faction 而**留**遗物（faction 支）。这验证 faction 支生效。
    // 真正"普通 rogue 不留"由上一条 should_leave_relic 单测 + combat.rs 饱和单测锁死。
    //
    // 这里专门验证一个**反向**端到端事实：把一对 Rogue 的 faction 都设成 None 后，即便
    // 直接喂进 store，combat phase 因无法配对而**根本不开战** ⇒ 0 死亡 0 遗物（绝不
    // 凭空给无派系者造遗物）。
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone()],
    };
    let mut store = NpcDormantStore::default();
    let mut a = combat_snapshot_named(
        "rogue_a",
        NpcArchetype::Rogue,
        None,
        5.0,
        DVec3::new(10.0, 64.0, 10.0),
    );
    a.cultivation.realm = Realm::Awaken;
    let mut b = combat_snapshot_named(
        "rogue_b",
        NpcArchetype::Rogue,
        None,
        5.0,
        DVec3::new(11.0, 64.0, 11.0),
    );
    b.cultivation.realm = Realm::Awaken;
    store.insert(a);
    store.insert(b);
    store.take_dirty();
    let mut ledger = WorldQiAccount::default();
    let config = NpcVirtualizationConfig::default();

    let events = run_combat_to_completion(&mut store, &mut zones, &mut ledger, &config, 7);

    assert!(
        events.deaths.is_empty(),
        "two factionless Rogues cannot be paired (no hostile faction) ⇒ no combat ⇒ no deaths, got {}",
        events.deaths.len()
    );
    assert!(
        events.relics.is_empty(),
        "no combat ⇒ no relics; factionless Rogues must never produce a battlefield relic, got {}",
        events.relics.len()
    );
}

#[test]
fn settlement_failure_emits_no_relic_and_pending_loser_stays_frozen() {
    let mut invalid_zone = zone();
    invalid_zone.spirit_qi = f64::NAN;
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![invalid_zone],
    };
    let mut store = NpcDormantStore::default();
    store.insert(combat_snapshot_named(
        "fallen_disciple",
        NpcArchetype::Disciple,
        Some(FactionId::Attack),
        5e-7,
        DVec3::new(10.0, 64.0, 10.0),
    ));
    store.insert(combat_snapshot_named(
        "rival",
        NpcArchetype::Disciple,
        Some(FactionId::Defend),
        5e-7,
        DVec3::new(11.0, 64.0, 11.0),
    ));
    let mut ledger = WorldQiAccount::default();
    let config = NpcVirtualizationConfig::default();

    let tick_one = run_combat_to_completion(&mut store, &mut zones, &mut ledger, &config, 7);
    assert!(
        tick_one.deaths.is_empty() && tick_one.outcomes.is_empty(),
        "failed settlement must persist pending state before publishing terminal events"
    );
    assert!(tick_one.relics.is_empty());
    let (loser_id, retained) = store
        .snapshots
        .iter()
        .find(|(_, snapshot)| snapshot.combat_dead_pending_release)
        .expect("failed settlement must retain the dormant physical owner");
    let loser_id = loser_id.clone();
    assert!(retained.pending_combat_winner.is_some());
    assert_eq!(
        retained.cultivation.qi_current(),
        5e-7,
        "positive sub-epsilon qi must remain owned when combat settlement fails"
    );
    let position = retained.position;
    let qi = retained.cultivation.qi_current();
    let realm = retained.cultivation.realm;

    let tick_two = run_combat_tick(&mut store, &mut zones, &mut ledger, &config, 8);
    assert!(tick_two.deaths.is_empty());
    assert!(tick_two.outcomes.is_empty());
    assert!(tick_two.relics.is_empty());
    let still = store
        .snapshots
        .get(&loser_id)
        .expect("repeated failed settlement must keep the same dormant owner");
    assert!(still.combat_dead_pending_release);
    assert_eq!(still.position, position);
    assert_eq!(still.cultivation.qi_current(), qi);
    assert_eq!(still.cultivation.realm, realm);
}

#[test]
fn pending_release_retries_then_emits_one_relic_after_zone_recovers() {
    let mut invalid_zone = zone();
    invalid_zone.spirit_qi = f64::NAN;
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![invalid_zone],
    };
    let mut store = NpcDormantStore::default();
    store.insert(combat_snapshot_named(
        "fallen_disciple",
        NpcArchetype::Disciple,
        Some(FactionId::Attack),
        5.0,
        DVec3::new(10.0, 64.0, 10.0),
    ));
    store.insert(combat_snapshot_named(
        "rival",
        NpcArchetype::Disciple,
        Some(FactionId::Defend),
        5.0,
        DVec3::new(11.0, 64.0, 11.0),
    ));
    let mut ledger = WorldQiAccount::default();
    let config = NpcVirtualizationConfig::default();

    let tick_one = run_combat_to_completion(&mut store, &mut zones, &mut ledger, &config, 7);
    assert!(tick_one.deaths.is_empty());
    let loser_id = store
        .snapshots
        .iter()
        .find(|(_, snapshot)| snapshot.combat_dead_pending_release)
        .map(|(id, _)| id.clone())
        .expect("failed settlement must retain one pending loser");
    assert!(store.snapshots[&loser_id].pending_combat_winner.is_some());
    zones.zones[0].spirit_qi = 0.8;

    let tick_two = run_combat_tick(&mut store, &mut zones, &mut ledger, &config, 8);
    assert_eq!(tick_two.deaths.len(), 1);
    assert_eq!(tick_two.deaths[0].npc_id, loser_id);
    assert_eq!(tick_two.outcomes.len(), 1);
    assert_eq!(tick_two.relics.len(), 1);
    assert_eq!(tick_two.relics[0].char_id, loser_id);
    assert!(!store.contains(&loser_id));
    assert!(
        !ledger.has_account(&QiAccountId::npc(loser_id.clone()))
            && !ledger.has_account(&QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME)),
        "retry settlement must still avoid dormant and Zone ledger shadows"
    );
}
#[test]
fn pending_release_retry_freezes_per_char_state_and_preserves_owners_and_audit_each_tick() {
    let mut invalid_zone = zone();
    invalid_zone.spirit_qi = f64::NAN;
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![invalid_zone],
    };
    let mut store = NpcDormantStore::default();
    let mut pending = combat_snapshot_named(
        "fallen_disciple",
        NpcArchetype::Disciple,
        Some(FactionId::Attack),
        5.0,
        DVec3::new(10.0, 64.0, 10.0),
    );
    pending.combat_dead_pending_release = true;
    pending.pending_combat_winner = Some("rival".to_string());
    pending.intent = DormantBehaviorIntent::PatrolToward {
        target: vec3_to_array(DVec3::new(90.0, 64.0, 90.0)),
    };
    pending.last_dormant_tick_processed = 0;
    pending.lifespan.age_ticks = 10.0;
    let pending_id = pending.char_id.clone();
    store.insert(pending);
    acknowledge_dormant_persistence(&mut store);
    store.apply_persistence_receipts();
    let baseline = store.snapshots[&pending_id].clone();
    let mut ledger = WorldQiAccount::default();
    let config = NpcVirtualizationConfig::default();

    for tick in [1200, 2400, 3600, 4800] {
        let zone_before = zones.zones[0].spirit_qi;
        let ledger_before = ledger.clone();
        let events = run_combat_tick(&mut store, &mut zones, &mut ledger, &config, tick);

        assert!(events.deaths.is_empty(), "tick={tick}");
        assert!(events.outcomes.is_empty(), "tick={tick}");
        assert!(events.relics.is_empty(), "tick={tick}");
        let retained = store
            .snapshots
            .get(&pending_id)
            .expect("failed retry must retain the same physical owner");
        assert!(retained.combat_dead_pending_release, "tick={tick}");
        assert_eq!(retained.position, baseline.position, "tick={tick}");
        assert_eq!(retained.intent, baseline.intent, "tick={tick}");
        assert_eq!(
            retained.last_dormant_tick_processed, baseline.last_dormant_tick_processed,
            "logical death must freeze the dormant clock at tick={tick}"
        );
        assert_eq!(
            retained.lifespan.age_ticks, baseline.lifespan.age_ticks,
            "tick={tick}"
        );
        assert_eq!(
            retained.lifespan.max_age_ticks, baseline.lifespan.max_age_ticks,
            "tick={tick}"
        );
        assert_eq!(retained.cultivation, baseline.cultivation, "tick={tick}");
        assert_eq!(
            retained.life_record.character_id, baseline.life_record.character_id,
            "tick={tick}"
        );
        assert_eq!(
            retained.life_record.created_at, baseline.life_record.created_at,
            "tick={tick}"
        );
        assert_eq!(
            retained.life_record.biography.len(),
            baseline.life_record.biography.len(),
            "tick={tick}"
        );
        assert_eq!(
            retained.life_record.death_insights, baseline.life_record.death_insights,
            "tick={tick}"
        );
        assert_eq!(
            retained.life_record.skill_milestones, baseline.life_record.skill_milestones,
            "tick={tick}"
        );
        assert!(zones.zones[0].spirit_qi.is_nan() && zone_before.is_nan());
        assert_eq!(
            ledger.balance(&crate::qi_physics::qi_flow_overflow_account()),
            0.0,
            "tick={tick}"
        );
        assert_eq!(ledger.transfers(), ledger_before.transfers(), "tick={tick}");
        assert_eq!(ledger.total(), ledger_before.total(), "tick={tick}");
        assert_no_dormant_or_zone_shadows(&store, &zones, &ledger);
    }
}

#[test]
fn failed_pending_release_roundtrips_without_duplicate_events_then_finalizes_once() {
    let mut invalid_zone = zone();
    invalid_zone.spirit_qi = f64::NAN;
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![invalid_zone],
    };
    let mut store = NpcDormantStore::default();
    store.insert(combat_snapshot_named(
        "fallen_disciple",
        NpcArchetype::Disciple,
        Some(FactionId::Attack),
        5.0,
        DVec3::new(10.0, 64.0, 10.0),
    ));
    store.insert(combat_snapshot_named(
        "rival",
        NpcArchetype::Disciple,
        Some(FactionId::Defend),
        5.0,
        DVec3::new(11.0, 64.0, 11.0),
    ));
    store.take_dirty();
    let mut ledger = WorldQiAccount::default();
    let config = NpcVirtualizationConfig::default();

    let initial = run_combat_to_completion(&mut store, &mut zones, &mut ledger, &config, 7);
    assert!(initial.deaths.is_empty());
    assert!(initial.outcomes.is_empty());
    assert!(initial.relics.is_empty());
    let loser_id = store
        .snapshots
        .iter()
        .find(|(_, snapshot)| snapshot.combat_dead_pending_release)
        .map(|(id, _)| id.clone())
        .expect("failed settlement must persist one pending loser");
    let retained_qi = store.snapshots[&loser_id].cultivation.qi_current();
    let payload = store
        .to_redis_hash_payloads()
        .expect("pending loser must serialize")
        .into_iter()
        .find(|(id, _)| id == &loser_id)
        .expect("pending loser must be present in the Redis hash payload")
        .1;
    let restored_loser: NpcDormantSnapshot =
        serde_json::from_str(&payload).expect("pending loser must survive Redis JSON roundtrip");
    assert!(restored_loser.combat_dead_pending_release);
    assert!(
        restored_loser.pending_combat_winner.is_some(),
        "pending winner context must survive Redis before any terminal event is published"
    );
    assert_eq!(restored_loser.cultivation.qi_current(), retained_qi);

    let mut restored_store = NpcDormantStore::default();
    load_dormant_snapshots_from_hash_entries(
        &mut restored_store,
        std::collections::HashMap::from([(loser_id.clone(), payload)]),
    )
    .expect("pending loser must restore through the production HASH boundary");
    for tick in [8, 9, 10] {
        let events = run_combat_tick(&mut restored_store, &mut zones, &mut ledger, &config, tick);
        assert!(events.deaths.is_empty(), "tick={tick}");
        assert!(events.outcomes.is_empty(), "tick={tick}");
        assert!(events.relics.is_empty(), "tick={tick}");
        assert_eq!(
            restored_store.snapshots[&loser_id].cultivation.qi_current(),
            retained_qi,
            "failed retry must preserve the actor owner at tick={tick}"
        );
        assert!(ledger.transfers().is_empty(), "tick={tick}");
    }

    zones.zones[0].spirit_qi = 0.8;
    let recovered = run_combat_tick(&mut restored_store, &mut zones, &mut ledger, &config, 11);
    assert_eq!(recovered.deaths.len(), 1);
    assert_eq!(recovered.deaths[0].npc_id, loser_id);
    assert_eq!(recovered.outcomes.len(), 1);
    assert_eq!(recovered.relics.len(), 1);
    assert_eq!(recovered.relics[0].char_id, loser_id);
    assert!(!restored_store.contains(&loser_id));
    assert_eq!(ledger.transfers().len(), 1);
    assert_eq!(
        ledger.transfers()[0].reason,
        QiTransferReason::ReleaseToZone
    );

    let after_finalize = run_combat_tick(&mut restored_store, &mut zones, &mut ledger, &config, 12);
    assert!(after_finalize.deaths.is_empty());
    assert!(after_finalize.outcomes.is_empty());
    assert!(after_finalize.relics.is_empty());
    assert_eq!(
        ledger.transfers().len(),
        1,
        "a finalized pending loser must not settle or emit again"
    );
}

fn physical_owner_total(
    store: &NpcDormantStore,
    zones: &ZoneRegistry,
    ledger: &WorldQiAccount,
) -> f64 {
    let dormant_qi: f64 = store
        .snapshots
        .values()
        .map(|snapshot| {
            snapshot.cultivation.qi_current()
                + snapshot
                    .tsy_hostile
                    .as_ref()
                    .and_then(|hostile| hostile.daozhan.as_ref())
                    .map_or(0.0, |daozhan| daozhan.daozhan_qi)
        })
        .sum();
    let zone_qi: f64 = zones
        .zones
        .iter()
        .map(|zone| zone.spirit_qi * QI_ZONE_UNIT_CAPACITY)
        .sum();
    dormant_qi + zone_qi + ledger.total()
}

fn assert_no_dormant_or_zone_shadows(
    store: &NpcDormantStore,
    zones: &ZoneRegistry,
    ledger: &WorldQiAccount,
) {
    for snapshot in store.snapshots.values() {
        assert!(
            !ledger.has_account(&QiAccountId::npc(snapshot.char_id.clone())),
            "dormant actor `{}` must remain owned by its snapshot, not a ledger shadow",
            snapshot.char_id
        );
    }
    for zone in &zones.zones {
        assert!(
            !ledger.has_account(&QiAccountId::zone(zone.name.clone())),
            "zone `{}` must remain owned by Zone.spirit_qi, not a ledger shadow",
            zone.name
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────
// plan-npc-realm-distribution-v1 P3 §8.1 #3 — 存量 dormant 快照迁移 + marker 幂等
// ─────────────────────────────────────────────────────────────────────────
//
// 每个测试都用 `BONG_NPC_REALM_MIGRATION_MARKER_PATH` 把 marker 路径钉到临时目录，
// 绝不能碰真实 checkout 里的 `server/data/npc/realm_migration_v1.marker`
// （那是运行时生成的非提交产物，见 .gitignore）。`ENV_LOCK` 序列化对该 env var 的
// 读写，防止并行跑的测试互相踩脚（`cargo test` 默认多线程跑同进程内的测试）。

// `cargo test` 默认多线程并发跑同进程内的测试，而 `std::env::set_var` 是进程级全局
// 状态——若锁只在 `set`/`drop` 内瞬时持有，两个测试仍可能交错（A 设置 env var 后、
// 在 A 的 `app.update()` 读取它之前，B 的 `set()` 把它改成另一个路径），A 就会读到
// 错误的 marker 路径而误判"marker 不存在"。必须让 guard 存活到整个测试结束（挂在
// `ScopedMarkerEnvVar` 实例上，随其 Drop 才释放），而不是只在设置那一刻短暂加锁。
struct ScopedMarkerEnvVar {
    _guard: std::sync::MutexGuard<'static, ()>,
    previous: Option<std::ffi::OsString>,
}

static MARKER_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

impl ScopedMarkerEnvVar {
    fn set(path: &std::path::Path) -> Self {
        let guard = MARKER_ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let previous = std::env::var_os(NPC_REALM_MIGRATION_MARKER_ENV_VAR);
        std::env::set_var(NPC_REALM_MIGRATION_MARKER_ENV_VAR, path);
        Self {
            _guard: guard,
            previous,
        }
    }
}

impl Drop for ScopedMarkerEnvVar {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.take() {
            std::env::set_var(NPC_REALM_MIGRATION_MARKER_ENV_VAR, previous);
        } else {
            std::env::remove_var(NPC_REALM_MIGRATION_MARKER_ENV_VAR);
        }
    }
}

fn unique_marker_path(label: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("current time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("bong-realm-migration-{label}-{nanos}.marker"))
}

fn legacy_rogue_snapshot(char_id: &str) -> NpcDormantSnapshot {
    let mut snap = snapshot(char_id, DVec3::new(5.0, 64.0, 5.0));
    // P0-era bug state: realm 恒醒灵，qi_max 恒 Cultivation::default() 的 10.0。
    snap.cultivation.realm = Realm::Awaken;
    snap.cultivation.qi_max = 10.0;
    snap.shared_lifespan = LifespanComponent::for_realm(Realm::Awaken);
    snap
}

fn migration_test_app(zone_registry: ZoneRegistry, store: NpcDormantStore) -> App {
    let mut app = App::new();
    app.insert_resource(zone_registry);
    app.insert_resource(store);
    app.insert_resource(DormantRoguePopulationSeedConfig::default());
    app.insert_resource(PendingGameplayNarrations::default());
    app.add_systems(Update, migrate_dormant_realm_distribution_v1);
    app
}

#[test]
fn migration_marker_path_pinned_to_spec() {
    // §8.1 #3 落点原文：`data/npc/realm_migration_v1.marker`。防止路径漂移。
    assert_eq!(
        NPC_REALM_MIGRATION_MARKER_DEFAULT_PATH, "data/npc/realm_migration_v1.marker",
        "marker 默认路径必须逐字对拍 plan §8.1 #3 落点原文，不允许漂移"
    );
}

#[test]
fn no_marker_triggers_reroll_and_writes_marker_file() {
    let marker_path = unique_marker_path("reroll");
    let _env = ScopedMarkerEnvVar::set(&marker_path);
    assert!(
        !marker_path.exists(),
        "precondition: 临时 marker 路径不应预先存在"
    );

    let mut store = NpcDormantStore::default();
    for i in 0..40u32 {
        let snap = legacy_rogue_snapshot(&format!("legacy:rogue:{i}"));
        store.snapshots.insert(snap.char_id.clone(), snap);
    }
    store.rebuild_indexes();

    let mut z = zone();
    z.spirit_qi = 0.1; // 低于 default threshold 0.4 -> background zone
    let registry = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![z],
    };

    let mut app = migration_test_app(registry, store);
    app.update();

    assert!(
        marker_path.exists(),
        "迁移完成后应写出 marker 文件到 {}",
        marker_path.display()
    );

    let migrated_store = app.world().resource::<NpcDormantStore>();
    assert!(
        migrated_store.is_dirty(),
        "至少一条快照 realm 变化应把 store 标脏，否则重 roll 结果不会持久化"
    );

    // 逐条对拍：迁移结果必须与「直接调用同一份 §8.1 #1 抽样函数」完全一致——
    // 测契约（"迁移是否正确委托给规范抽样函数"），不是重新验证分布算法本身
    // （分布算法已由 sample_rogue_seed_realm 的专属测试饱和覆盖）。
    let mut saw_non_awaken = false;
    for i in 0..40u32 {
        let char_id = format!("legacy:rogue:{i}");
        let expected = sample_rogue_seed_realm(char_id.as_str(), false);
        let actual = migrated_store
            .snapshots
            .get(&char_id)
            .unwrap_or_else(|| panic!("snapshot {char_id} should still exist after migration"))
            .cultivation
            .realm;
        assert_eq!(
            actual, expected,
            "char_id={char_id}: 迁移后的 realm 应等于 sample_rogue_seed_realm 对同一 \
             char_id/is_resource_zone 的确定性抽样结果，实得 {actual:?} 期望 {expected:?}"
        );
        if actual != Realm::Awaken {
            saw_non_awaken = true;
        }
        // qi_max 必须随新 realm 同步重算，不能停在 bug 时代的 10.0（除非新 realm 恰好
        // 仍是 Awaken，此时 10.0 本就是对的）。
        let expected_qi_max = qi_max_for_realm(expected);
        let actual_qi_max = migrated_store
            .snapshots
            .get(&char_id)
            .unwrap()
            .cultivation
            .qi_max;
        assert!(
            (actual_qi_max - expected_qi_max).abs() < 1e-9,
            "char_id={char_id}: qi_max 应随迁移后的 realm={expected:?} 重算为 \
             {expected_qi_max}，实得 {actual_qi_max}"
        );
    }
    assert!(
        saw_non_awaken,
        "40 条 legacy 快照跑一遍 §8.1 #1 分布表重抽样，至少应有一条不再是醒灵 \
         （分布表醒灵权重远小于 100%），否则说明重 roll 根本没生效"
    );
}

#[test]
fn migration_reroll_resyncs_meridian_system_to_new_realm_required_meridians() {
    // Verify blocker pin：迁移器此前只重算 realm/qi_max/shared_lifespan，从不重派
    // meridian_system——重 roll 到凝脉/固元/通灵后仍停在迁移前的开脉数，与新 realm
    // 脱钩。legacy_rogue_snapshot 起点固定 Realm::Awaken + snapshot() 默认全闭经脉
    // （P0-era 状态），迁移后必须让 meridian_system.opened_count() 追上新 realm。
    let marker_path = unique_marker_path("meridian-resync");
    let _env = ScopedMarkerEnvVar::set(&marker_path);

    let mut store = NpcDormantStore::default();
    for i in 0..40u32 {
        let mut snap = legacy_rogue_snapshot(&format!("legacy:meridian:{i}"));
        // 复刻真实 P0-era 生产快照的形状：修 dormant_rogue_seed_snapshot 之前恒开
        // 1 条肺经（而非 legacy_rogue_snapshot 继承的通用 test helper 全闭默认值）
        // ——否则本测试会在「重抽样恰好落回 Awaken（未改变）」的分支上，把「legacy
        // fixture 本身形状失真」误判成「迁移器没有同步重派」的假阳性。
        snap.meridian_system = MeridianSystem::default();
        snap.meridian_system
            .get_mut(crate::cultivation::components::MeridianId::Lung)
            .opened = true;
        store.snapshots.insert(snap.char_id.clone(), snap);
    }
    store.rebuild_indexes();

    let mut z = zone();
    z.spirit_qi = 0.9; // 高于阈值 -> resource zone，拉高凝脉/固元/通灵命中率
    let registry = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![z],
    };

    let mut app = migration_test_app(registry, store);
    app.update();

    let migrated_store = app.world().resource::<NpcDormantStore>();
    let mut saw_multi_meridian_realm = false;
    for i in 0..40u32 {
        let char_id = format!("legacy:meridian:{i}");
        let snap = migrated_store
            .snapshots
            .get(&char_id)
            .unwrap_or_else(|| panic!("snapshot {char_id} should still exist after migration"));
        let expected_count = snap.cultivation.realm.required_meridians();
        let actual_count = snap.meridian_system.opened_count();
        assert_eq!(
            actual_count, expected_count,
            "char_id={char_id}: 迁移重 roll 后 realm={:?} 要求开 {expected_count} 条经脉，\
             但 meridian_system 实开 {actual_count} 条——meridian_system 没有随 realm 重 roll \
             同步重派（迁移器只改了 realm/qi_max/shared_lifespan）",
            snap.cultivation.realm
        );
        if expected_count > 1 {
            saw_multi_meridian_realm = true;
        }
    }
    assert!(
        saw_multi_meridian_realm,
        "40 条 legacy 快照在 resource zone 下重抽样，至少应有一条落在 required_meridians()>1 \
         的境界（凝脉=6/固元=12/通灵=16），否则本测试没有真正覆盖迁移器重派 \
         meridian_system 的分支（fixture 完整性）"
    );
}

#[test]
fn migration_reroll_respects_permanently_severed_meridians() {
    // minor fix pin：迁移器重派 meridian_system 时用 npc_meridian_system_for_realm
    // 整段覆盖，会把「已被 MeridianSeveredPermanent 永久记录断绝」的经脉也一并
    // 按新 realm 重新打开——这与"永久断脉"语义矛盾（断脉只应在跨周目重置，
    // realm 迁移这种同一角色的境界重 roll 不该复活它）。用 Lung（MeridianId::ALL[0]，
    // 任何 realm 的 required_meridians() >= 1 都会覆盖到它）作为永久断脉标的，
    // 断言迁移后依旧 opened=false。
    let marker_path = unique_marker_path("meridian-severed-respect");
    let _env = ScopedMarkerEnvVar::set(&marker_path);

    let mut store = NpcDormantStore::default();
    for i in 0..40u32 {
        let mut snap = legacy_rogue_snapshot(&format!("legacy:severed:{i}"));
        snap.meridian_system = MeridianSystem::default();
        snap.meridian_severed
            .severed_meridians
            .insert(crate::cultivation::components::MeridianId::Lung.channel_id());
        snap.meridian_severed.severed_at.insert(
            crate::cultivation::components::MeridianId::Lung.channel_id(),
            crate::cultivation::meridian::severed::SeveredRecord {
                at_tick: 0,
                source: crate::cultivation::meridian::severed::SeveredSource::CombatWound,
            },
        );
        store.snapshots.insert(snap.char_id.clone(), snap);
    }
    store.rebuild_indexes();

    let mut z = zone();
    z.spirit_qi = 0.9; // 高于阈值 -> resource zone，拉高高境界命中率，确保有 realm 变化分支被覆盖
    let registry = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![z],
    };

    let mut app = migration_test_app(registry, store);
    app.update();

    let migrated_store = app.world().resource::<NpcDormantStore>();
    let mut saw_changed_realm = false;
    for i in 0..40u32 {
        let char_id = format!("legacy:severed:{i}");
        let snap = migrated_store
            .snapshots
            .get(&char_id)
            .unwrap_or_else(|| panic!("snapshot {char_id} should still exist after migration"));
        if snap.cultivation.realm != Realm::Awaken {
            saw_changed_realm = true;
        }
        let lung = snap
            .meridian_system
            .get(crate::cultivation::components::MeridianId::Lung);
        assert!(
            !lung.opened,
            "char_id={char_id}: Lung 经脉在 meridian_severed 中被永久记录断绝，\
             迁移重派 meridian_system 后仍必须保持 opened=false（实际 opened=true），\
             否则永久断脉被 realm 迁移悄悄复活，与 MeridianSeveredPermanent 记录矛盾"
        );
        assert!(
            snap.meridian_severed
                .severed_meridians
                .contains(&crate::cultivation::components::MeridianId::Lung.channel_id()),
            "char_id={char_id}: 迁移不应改动 meridian_severed 记录本身"
        );
    }
    assert!(
        saw_changed_realm,
        "40 条 legacy 快照在 resource zone 下重抽样，至少应有一条 realm 发生变化，\
         否则本测试没有真正覆盖迁移器重派 meridian_system 的分支（fixture 完整性）"
    );
}

#[test]
fn marker_already_exists_skips_reroll_idempotently() {
    let marker_path = unique_marker_path("skip");
    let _env = ScopedMarkerEnvVar::set(&marker_path);
    // 预先写好 marker——模拟"上次已经迁移过"。
    if let Some(parent) = marker_path.parent() {
        std::fs::create_dir_all(parent).expect("temp dir must be creatable");
    }
    std::fs::write(&marker_path, b"v1\n").expect("precondition marker write must succeed");

    let mut store = NpcDormantStore::default();
    for i in 0..10u32 {
        let snap = legacy_rogue_snapshot(&format!("legacy:skip:{i}"));
        store.snapshots.insert(snap.char_id.clone(), snap);
    }
    store.rebuild_indexes();
    // 显式确认起点是 clean（构造过程没有调用任何 mark_dirty 路径）。
    assert!(!store.is_dirty());

    let registry = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone()],
    };
    let mut app = migration_test_app(registry, store);
    app.update();

    let after = app.world().resource::<NpcDormantStore>();
    assert!(
        !after.is_dirty(),
        "marker 已存在时不应做任何重 roll，store 不该被标脏"
    );
    for i in 0..10u32 {
        let char_id = format!("legacy:skip:{i}");
        let realm = after.snapshots.get(&char_id).unwrap().cultivation.realm;
        assert_eq!(
            realm,
            Realm::Awaken,
            "char_id={char_id}: marker 已存在应跳过重 roll，realm 应保持迁移前的 \
             Realm::Awaken（bug 时代遗留值），实得 {realm:?}"
        );
    }

    // 内容也保持不变（同一份写入的 marker 内容原样保留，未被二次改写）。
    let content = std::fs::read_to_string(&marker_path).expect("marker should still exist");
    assert_eq!(content, "v1\n");
}

#[test]
fn identity_archetypes_write_identity_realm_not_sampled() {
    let marker_path = unique_marker_path("identity");
    let _env = ScopedMarkerEnvVar::set(&marker_path);

    let mut store = NpcDormantStore::default();

    let mut guardian = legacy_rogue_snapshot("legacy:guardian");
    guardian.archetype = NpcArchetype::GuardianRelic;
    store.snapshots.insert(guardian.char_id.clone(), guardian);

    let mut zhinian = legacy_rogue_snapshot("legacy:zhinian");
    zhinian.archetype = NpcArchetype::Zhinian;
    store.snapshots.insert(zhinian.char_id.clone(), zhinian);

    let mut daoxiang = legacy_rogue_snapshot("legacy:daoxiang");
    daoxiang.archetype = NpcArchetype::Daoxiang;
    store.snapshots.insert(daoxiang.char_id.clone(), daoxiang);

    let mut beast = legacy_rogue_snapshot("legacy:beast");
    beast.archetype = NpcArchetype::Beast;
    store.snapshots.insert(beast.char_id.clone(), beast);

    let mut leader = legacy_rogue_snapshot("legacy:leader");
    leader.archetype = NpcArchetype::Disciple;
    leader.faction = Some(FactionMembership {
        faction_id: FactionId::Defend, // CangyuanMerchants -> Spirit
        rank: FactionRank::Leader,
        reputation: Reputation::default(),
        lineage: None,
        mission_queue: MissionQueue::default(),
    });
    store
        .snapshots
        .insert(leader.char_id.clone(), leader.clone());

    store.rebuild_indexes();

    let registry = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone()],
    };
    let mut app = migration_test_app(registry, store);
    app.update();

    let after = app.world().resource::<NpcDormantStore>();
    let realm_of = |id: &str| after.snapshots.get(id).unwrap().cultivation.realm;

    assert_eq!(
        realm_of("legacy:guardian"),
        Realm::Spirit,
        "GuardianRelic 身份 realm 应直写 Spirit，不参与分布抽样"
    );
    assert_eq!(
        realm_of("legacy:zhinian"),
        Realm::Condense,
        "Zhinian 身份 realm 应直写 Condense"
    );
    assert_eq!(
        realm_of("legacy:daoxiang"),
        Realm::Induce,
        "Daoxiang 身份 realm 应直写 TSY 默认值 Induce"
    );
    assert_eq!(
        realm_of("legacy:beast"),
        Realm::Awaken,
        "Beast 恒字面量 Awaken，迁移不应把它拉进分布抽样（保持设计上的恒低威胁）"
    );
    assert_eq!(
        realm_of("legacy:leader"),
        Realm::Spirit,
        "faction Leader（Defend/CangyuanMerchants）应直写 leader_realm_for 对应的 Spirit，\
         不受分布表影响"
    );
}

#[test]
fn marker_write_failure_does_not_silently_swallow_error() {
    // 制造一个必然写失败的路径：父目录的父目录其实是个*文件*，create_dir_all 会报错。
    let blocked_parent = unique_marker_path("blocked-parent-file");
    std::fs::write(&blocked_parent, b"i am a file, not a directory")
        .expect("setup: create the blocking regular file");
    let marker_path = blocked_parent.join("sub").join("realm_migration_v1.marker");
    let _env = ScopedMarkerEnvVar::set(&marker_path);

    let mut store = NpcDormantStore::default();
    let snap = legacy_rogue_snapshot("legacy:writefail");
    store.snapshots.insert(snap.char_id.clone(), snap);
    store.rebuild_indexes();

    // 显式钉 zone 灵气档为 background（低于 default threshold 0.4），让下面的
    // `sample_rogue_seed_realm(..., false)` 探测与迁移器内部真实算出的 is_resource
    // 保持一致，不依赖 `zone()` helper 默认值今后是否变动。
    let mut z = zone();
    z.spirit_qi = 0.1;
    let registry = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![z],
    };
    let mut app = migration_test_app(registry, store);

    // 直接调用底层写函数验证返回值语义（true=成功/false=失败），不靠系统副作用间接推断。
    assert!(
        !write_realm_migration_marker(&marker_path),
        "父目录路径被文件挡住时，写 marker 必须返回失败而不是假装成功"
    );
    assert!(
        !marker_path.exists(),
        "写失败后 marker 文件不应该神奇地出现"
    );

    // 即使 marker 落盘失败，system 跑一遍仍必须完成本次的 realm 重 roll（降级只影响
    // "下次重启是否会重复重 roll"这个幂等信号，不能连本次的迁移本体都吞掉）。
    app.update();
    let after = app.world().resource::<NpcDormantStore>();
    let realm = after
        .snapshots
        .get("legacy:writefail")
        .unwrap()
        .cultivation
        .realm;
    let expected = sample_rogue_seed_realm("legacy:writefail", false);
    assert_eq!(
        realm, expected,
        "marker 写失败不该连带吞掉本次 reroll 本身——快照 realm 仍应等于确定性抽样结果"
    );

    let _ = std::fs::remove_file(&blocked_parent);
}

#[test]
fn empty_store_writes_marker_without_touching_anything() {
    let marker_path = unique_marker_path("empty-store");
    let _env = ScopedMarkerEnvVar::set(&marker_path);

    let store = NpcDormantStore::default();
    assert!(store.is_empty());
    let registry = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![zone()],
    };
    let mut app = migration_test_app(registry, store);
    app.update();

    assert!(
        marker_path.exists(),
        "空 store（新世界，无存量）也应该写 marker，避免每次 Startup 重复判定空 store"
    );
    assert!(
        !app.world().resource::<NpcDormantStore>().is_dirty(),
        "空 store 没有任何快照可改，不应被标脏"
    );
}

#[test]
fn migration_pushes_zone_perception_narration_for_upgraded_realms() {
    let marker_path = unique_marker_path("narration");
    let _env = ScopedMarkerEnvVar::set(&marker_path);

    // 显式钉 zone 灵气档为 background（低于 default threshold 0.4），让下面的
    // `sample_rogue_seed_realm(..., false)` 探测与迁移器内部真实算出的 is_resource
    // 保持一致，不依赖 `zone()` helper 默认值今后是否变动。
    //
    // 固定挑一个 char_id，其分布抽样结果已知会命中 Condense 或更高（用同一份抽样函数
    // 先探测，避免测试跟迁移函数各自实现一套判定逻辑）。
    let mut store = NpcDormantStore::default();
    let mut hit_condense_or_above = None;
    for i in 0..200u32 {
        let char_id = format!("legacy:narration:{i}");
        let realm = sample_rogue_seed_realm(char_id.as_str(), false);
        if matches!(realm, Realm::Condense | Realm::Solidify) {
            hit_condense_or_above = Some((char_id.clone(), realm));
        }
        let snap = legacy_rogue_snapshot(&char_id);
        store.snapshots.insert(snap.char_id.clone(), snap);
    }
    store.rebuild_indexes();
    let (hit_char_id, hit_realm) = hit_condense_or_above.expect(
        "200 条随机 char_id 里按分布表理应至少命中一条 Condense/Solidify，\
         否则下面的 narration 断言无法验证任何真实行为",
    );

    let mut z = zone();
    z.spirit_qi = 0.1;
    let registry = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![z],
    };
    let mut app = migration_test_app(registry, store);
    app.update();

    let mut narrations = app.world_mut().resource_mut::<PendingGameplayNarrations>();
    let drained = narrations.drain();
    assert!(
        !drained.is_empty(),
        "命中 char_id={hit_char_id} 应重 roll 出 {hit_realm:?}，理应推送至少一条 \
         zone-scope 境界识破 narration，实得 0 条"
    );
    assert!(
        drained
            .iter()
            .all(|n| n.scope == crate::schema::common::NarrationScope::Zone
                && n.style == crate::schema::common::NarrationStyle::Perception),
        "迁移触发的境界识破 narration 必须是 Zone scope + Perception style，实得 {drained:?}"
    );
}
