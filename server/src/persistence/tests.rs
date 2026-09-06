use super::*;
use crate::combat::components::LifecycleState;
use crate::cultivation::components::{
    ColorKind, ContamSource, Contamination, Cultivation, Karma, MeridianSystem, QiColor, Realm,
};
use crate::cultivation::known_techniques::KnownTechnique;
use crate::npc::movement::{MovementController, MovementCooldowns, MovementMode, SprintState};
use crate::npc::patrol::NpcPatrol;
use crate::npc::spawn::{NpcBlackboard, NpcCombatLoadout, NpcMarker, NpcMeleeArchetype};
use crate::persistence::slice::{
    dispatch_shutdown_flushes, DirtyRevision, PersistenceSubjectKey, ShutdownFlushReport, SliceLoad,
};
use crate::player::state::{
    save_player_core_slice, save_player_state, PlayerState, PlayerStatePersistence,
};
use crate::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;
use crate::qi_physics::ledger::{assert_conservation, qi_flow_overflow_account, WorldQiSnapshot};
use crate::schema::common::{NpcStateKind, SPIRIT_QI_TOTAL};
use crate::world::zone::DEFAULT_SPAWN_ZONE_NAME;
use rusqlite::{params, OptionalExtension};
use serde_json::Value;
use std::sync::{Arc, Barrier};
use std::time::Instant;
use valence::prelude::{App, AppExit, DVec3, EntityKind, Events, Position, PostUpdate, Update};
use valence::protocol::packets::play::GameMessageS2c;
use valence::testing::create_mock_client;

fn known_techniques_fixture(id: &str, proficiency: f32) -> KnownTechniques {
    KnownTechniques {
        entries: vec![KnownTechnique {
            id: id.to_string(),
            proficiency,
            active: true,
        }],
    }
}

fn flush_mock_client_packets(mut clients: Query<&mut Client>) {
    for mut client in &mut clients {
        let _ = client.flush_packets();
    }
}

fn known_techniques_app(
    test_name: &str,
) -> (App, PlayerStatePersistence, PersistenceSettings, PathBuf) {
    let (settings, root) = persistence_settings(test_name);
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("known techniques fixture database should bootstrap");
    let player_persistence =
        PlayerStatePersistence::with_db_path(root.join("data").join("players"), settings.db_path());
    let mut app = App::new();
    app.add_event::<AppExit>();
    app.add_event::<crate::npc::dormant::PendingDormantRelicCreated>();
    app.insert_resource(CultivationClock::default());
    app.insert_resource(WorldQiAccount::default());
    let overworld_layer = app.world_mut().spawn_empty().id();
    let tsy_layer = app.world_mut().spawn_empty().id();
    app.insert_resource(crate::world::dimension::DimensionLayers {
        overworld: overworld_layer,
        tsy: tsy_layer,
    });
    register(&mut app);
    app.insert_resource(settings.clone());
    app.insert_resource(player_persistence.clone());
    app.add_systems(
        Update,
        (
            crate::player::init_clients,
            crate::player::attach_player_state_to_joined_clients.after(crate::player::init_clients),
        ),
    );
    app.add_systems(PostUpdate, flush_mock_client_packets);
    (app, player_persistence, settings, root)
}

fn persisted_known_techniques(
    persistence: &PlayerStatePersistence,
    username: &str,
) -> Option<KnownTechniques> {
    load_player_known_techniques_slice(persistence, username)
        .expect("known techniques row should decode")
}

#[test]
fn known_techniques_retry_uses_bounded_backoff_capped_attempts_and_log_coalescing() {
    let subject = "offline:retry-boundaries";
    let mut state = KnownTechniquesReconnectState::default();

    let retry_frames = [0_u64, 1, 3, 7, 15, 31, 63, 127];
    for (index, frame) in retry_frames.into_iter().enumerate() {
        assert!(
            begin_known_techniques_retry(&mut state, subject, frame),
            "attempt {} should run at its scheduled frame",
            index + 1
        );
        assert_eq!(
            state.retries.get(subject).map(|entry| entry.attempts),
            Some(((index + 1) as u8).min(KNOWN_TECHNIQUES_RETRY_MAX_ATTEMPTS)),
            "retry attempts must count actual attempts, not skipped frames"
        );
        let at_cap = index >= retry_frames.len() - 1;
        assert_eq!(
            record_known_techniques_retry_failure(&mut state, subject, frame),
            at_cap,
            "only the retry cap reports back to the caller"
        );
        assert!(
            state
                .retries
                .get(subject)
                .map(|entry| entry.next_attempt_frame > frame)
                .unwrap_or(false),
            "every failure must schedule a later retry frame, never a terminal one"
        );
        if !at_cap {
            assert_eq!(
                state
                    .retries
                    .get(subject)
                    .map(|entry| entry.next_attempt_frame),
                Some(retry_frames[index + 1]),
                "failure must schedule the documented exponential backoff"
            );
        }
    }

    // 8 次瞬态失败后不得永久终止：attempts 停在 cap，backoff 继续按 64 帧上限调度。
    let entry = state.retries.get(subject).expect("retry entry");
    assert_eq!(entry.attempts, KNOWN_TECHNIQUES_RETRY_MAX_ATTEMPTS);
    let next_frame = entry.next_attempt_frame;
    assert!(!begin_known_techniques_retry(
        &mut state,
        subject,
        next_frame - 1
    ));
    assert!(begin_known_techniques_retry(
        &mut state, subject, next_frame
    ));
    assert_eq!(
        state.retries.get(subject).map(|entry| entry.attempts),
        Some(KNOWN_TECHNIQUES_RETRY_MAX_ATTEMPTS),
        "attempts must stay capped at the retry limit, not grow"
    );
    assert!(record_known_techniques_retry_failure(
        &mut state, subject, next_frame
    ));
    assert!(
        state
            .retries
            .get(subject)
            .map(|entry| entry.next_attempt_frame > next_frame)
            .unwrap_or(false),
        "post-cap failures must keep scheduling retries"
    );

    assert!(known_techniques_retry_log_allowed(&mut state, subject, 0));
    assert!(!known_techniques_retry_log_allowed(&mut state, subject, 1));
    assert!(!known_techniques_retry_log_allowed(&mut state, subject, 63));
    assert!(known_techniques_retry_log_allowed(&mut state, subject, 64));

    clear_known_techniques_retry(&mut state, subject);
    assert!(!state.retries.contains_key(subject));
}

#[test]
fn known_techniques_retry_cleanup_removes_only_subjects_no_longer_pending() {
    let mut state = KnownTechniquesReconnectState::default();
    assert!(begin_known_techniques_retry(
        &mut state,
        "offline:stale-retry",
        0
    ));
    assert!(begin_known_techniques_retry(
        &mut state,
        "offline:pending-retry",
        0
    ));
    assert!(begin_known_techniques_retry(
        &mut state,
        "offline:failed-disconnect-save",
        0
    ));

    let active_retry_subjects = std::collections::HashSet::from([
        "offline:pending-retry".to_string(),
        "offline:failed-disconnect-save".to_string(),
    ]);
    clear_stale_known_techniques_retries(&mut state, &active_retry_subjects);

    assert!(
        !state.retries.contains_key("offline:stale-retry"),
        "retry state for a subject no longer pending must be cleared"
    );
    assert!(
        state.retries.contains_key("offline:pending-retry"),
        "retry state for a still-pending subject must remain available for the handoff"
    );
    assert!(
        state.retries.contains_key("offline:failed-disconnect-save"),
        "retry state for a disconnected save failure must remain available without a handoff"
    );
}

#[test]
fn known_techniques_descriptor_pins_canonical_slice_contract() {
    let descriptor = &KNOWN_TECHNIQUES_SLICE_DESCRIPTOR;
    assert_eq!(descriptor.id, KNOWN_TECHNIQUES_SLICE_ID);
    assert_eq!(descriptor.scope, SliceScope::PlayerEntity);
    assert_eq!(descriptor.order, 10);
    assert_eq!(descriptor.load_failure, LoadFailurePolicy::BlockWrites);
    assert_eq!(descriptor.time_basis, TimeBasis::None);
    assert_eq!(
        descriptor.write_binding,
        WriteBinding::new(
            WriteDomain::new("player.known_techniques"),
            WriteAuthority::new("persistence.known_techniques"),
        )
    );
    assert_eq!(descriptor.write_ordering, WriteOrdering::Serialized);
    assert_eq!(descriptor.autosave, AutosavePolicy::Disabled);
    assert!(descriptor.hydrate.is_some());
    assert!(descriptor.reconnect_preflight.is_some());
    assert!(descriptor.reconnect_cleanup.is_some());
    assert!(descriptor.rebase.is_none());
    assert!(descriptor.disconnect_save.is_some());
    assert!(descriptor.shutdown_flush.is_some());
}

#[test]
fn hydrate_known_techniques_cleans_pending_entry_when_target_is_gone() {
    let mut world = World::new();
    world.insert_resource(PendingKnownTechniquesHandoffs::default());
    world.insert_resource(PendingKnownTechniquesCandidates::default());
    world.insert_resource(KnownTechniquesReconnectState::default());
    let entity = world.spawn_empty().id();
    world
        .resource_mut::<PendingKnownTechniquesHandoffs>()
        .0
        .insert("player:Azure".to_string(), entity);
    world.despawn(entity);

    let context = SliceRunContext {
        reason: SliceRunReason::ReconnectLoad,
        runtime_tick: 0,
        wall_unix_millis: 0,
        handoff_key: Some("player:Azure".to_string()),
        reconnect_activation: None,
    };
    let error = hydrate_known_techniques_slice(&mut world, &context)
        .expect_err("a vanished target must fail without panicking");
    assert!(error.message().contains("entity is gone"));
    assert!(
        !world
            .resource::<PendingKnownTechniquesHandoffs>()
            .0
            .contains_key("player:Azure"),
        "stale pending handoffs must be removed after the target disappears"
    );
}

#[test]
fn production_known_techniques_join_and_changed_write_use_canonical_activation() {
    let (mut app, persistence, _settings, root) =
        known_techniques_app("known-techniques-production-changed");
    let initial = known_techniques_fixture("movement.dash", 0.25);
    crate::player::state::save_player_known_techniques_slice(&persistence, "Azure", &initial)
        .expect("initial known techniques should persist");
    let (client_bundle, _helper) = create_mock_client("Azure");
    let player = app.world_mut().spawn(client_bundle).id();

    app.update();

    assert_eq!(
        app.world().get::<KnownTechniques>(player),
        Some(&initial),
        "Added<Client> must hydrate KnownTechniques through the production dispatcher"
    );
    let subject = canonical_player_id("Azure");
    let activation = app
        .world()
        .resource::<KnownTechniquesActivations>()
        .0
        .get(&subject)
        .expect("join must create the canonical activation");
    assert_eq!(activation.entity, player);
    assert_eq!(
        activation.guarded.load_status(),
        slice::SliceLoadStatus::Loaded
    );
    assert!(!activation.tracker.is_dirty());

    let changed = known_techniques_fixture("movement.dash", 0.75);
    app.world_mut().entity_mut(player).insert(changed.clone());
    app.update();

    assert_eq!(
        persisted_known_techniques(&persistence, "Azure"),
        Some(changed),
        "Changed<KnownTechniques> must commit through the production durable fence"
    );
    let activation = app
        .world()
        .resource::<KnownTechniquesActivations>()
        .0
        .get(&subject)
        .expect("changed write must retain the activation");
    assert!(!activation.tracker.is_dirty());
    assert_eq!(
        activation.fence.persisted_revision(),
        activation.tracker.current_revision(),
        "durable receipt acknowledgement must align fence and tracker revisions"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn production_connection_outage_stays_read_only_until_reconnect_rehydrates() {
    let (mut app, persistence, settings, root) =
        known_techniques_app("known-techniques-production-connection-outage");
    let initial = known_techniques_fixture("movement.dash", 0.2);
    crate::player::state::save_player_known_techniques_slice(&persistence, "Azure", &initial)
        .expect("initial known techniques should persist");
    app.update();

    let backup_path = root.join("known-techniques-outage-backup.db");
    fs::rename(settings.db_path(), &backup_path)
        .expect("fixture database should move out of the production path");
    fs::create_dir(settings.db_path())
        .expect("directory placeholder should make player connection unavailable");

    let (old_bundle, _old_helper) = create_mock_client("Azure");
    let old_player = app.world_mut().spawn(old_bundle).id();
    app.update();

    assert!(app
        .world()
        .get::<KnownTechniquesLoadFailed>(old_player)
        .is_some());
    assert!(
        app.world().get::<PlayerState>(old_player).is_some(),
        "known-techniques DB outage must not prevent the ordinary PlayerState bundle from attaching"
    );
    assert!(
        app.world()
            .get::<KnownTechniquesReconnectFailed>(old_player)
            .is_some(),
        "a transient known-techniques outage must mark only that slice as degraded"
    );
    assert!(
        app.world()
            .get::<KnownTechniquesReconnectBlocked>(old_player)
            .is_none(),
        "a load failure must not use the duplicate-login block marker"
    );
    assert_eq!(
        app.world()
            .resource::<KnownTechniquesActivations>()
            .0
            .get(&canonical_player_id("Azure"))
            .map(|activation| activation.guarded.load_status()),
        Some(slice::SliceLoadStatus::Failed),
        "a production connection-open failure must retain Failed provenance"
    );

    let during_outage = known_techniques_fixture("movement.dash", 0.8);
    app.world_mut().entity_mut(old_player).insert(during_outage);
    app.update();

    fs::remove_dir(settings.db_path()).expect("database outage placeholder should be removable");
    fs::rename(&backup_path, settings.db_path())
        .expect("the durable database should be restored after the outage");
    assert_eq!(
        persisted_known_techniques(&persistence, "Azure"),
        Some(initial.clone()),
        "an outage Changed event must not overwrite the existing durable row"
    );

    let before_reconnect = known_techniques_fixture("movement.dash", 0.9);
    app.world_mut()
        .entity_mut(old_player)
        .insert(before_reconnect);
    app.update();
    assert_eq!(
        persisted_known_techniques(&persistence, "Azure"),
        Some(initial.clone()),
        "database recovery alone must not make the failed activation writable"
    );

    app.world_mut().entity_mut(old_player).remove::<Client>();
    let (new_bundle, _new_helper) = create_mock_client("Azure");
    let new_player = app.world_mut().spawn(new_bundle).id();
    app.update();

    assert_eq!(
        app.world().get::<KnownTechniques>(new_player),
        Some(&initial),
        "reconnect must rehydrate the durable row before reopening writes"
    );
    assert!(app
        .world()
        .get::<KnownTechniquesLoadFailed>(new_player)
        .is_none());

    let after_reconnect = known_techniques_fixture("movement.dash", 1.0);
    app.world_mut()
        .entity_mut(new_player)
        .insert(after_reconnect.clone());
    app.update();
    assert_eq!(
        persisted_known_techniques(&persistence, "Azure"),
        Some(after_reconnect),
        "only the rehydrated activation may resume Changed writes"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn production_missing_known_techniques_row_uses_injected_registry_and_persists_first_change() {
    let (mut app, persistence, _settings, root) =
        known_techniques_app("known-techniques-production-missing");
    let registry = TechniqueRegistry::load_for_tests();
    let expected_reset = KnownTechniques::progression_reset(&registry);
    app.insert_resource(registry);
    let (client_bundle, _helper) = create_mock_client("Azure");
    let player = app.world_mut().spawn(client_bundle).id();

    app.update();

    let subject = canonical_player_id("Azure");
    assert_eq!(
        app.world()
            .resource::<KnownTechniquesActivations>()
            .0
            .get(&subject)
            .map(|activation| activation.guarded.load_status()),
        Some(slice::SliceLoadStatus::Missing),
        "an absent durable row must preserve Missing provenance"
    );
    assert_eq!(
        app.world().get::<KnownTechniques>(player),
        Some(&expected_reset),
        "a missing row must derive its progression reset from the injected runtime registry"
    );

    let first = known_techniques_fixture("movement.dash", 0.1);
    app.world_mut().entity_mut(player).insert(first.clone());
    app.update();

    assert_eq!(
        persisted_known_techniques(&persistence, "Azure"),
        Some(first),
        "Missing provenance must allow the first Changed write"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn production_known_techniques_disconnect_releases_activation() {
    let (mut app, persistence, _settings, root) =
        known_techniques_app("known-techniques-production-disconnect");
    let (client_bundle, _helper) = create_mock_client("Azure");
    let player = app.world_mut().spawn(client_bundle).id();
    app.update();
    let changed = known_techniques_fixture("movement.dash", 0.4);
    app.world_mut().entity_mut(player).insert(changed.clone());
    app.world_mut().entity_mut(player).remove::<Client>();

    app.update();

    assert_eq!(
        persisted_known_techniques(&persistence, "Azure"),
        Some(changed)
    );
    assert!(
        !app.world()
            .resource::<KnownTechniquesActivations>()
            .0
            .contains_key(&canonical_player_id("Azure")),
        "ordinary disconnect must release the durable-subject activation"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn production_known_techniques_same_tick_reconnect_saves_before_hydrate() {
    let (mut app, persistence, _settings, root) =
        known_techniques_app("known-techniques-production-reconnect");
    let (old_bundle, _old_helper) = create_mock_client("Azure");
    let old_player = app.world_mut().spawn(old_bundle).id();
    app.update();
    let unsaved = known_techniques_fixture("movement.dash", 0.9);
    app.world_mut()
        .entity_mut(old_player)
        .insert(unsaved.clone());
    app.world_mut().entity_mut(old_player).remove::<Client>();
    let (new_bundle, _new_helper) = create_mock_client("Azure");
    let new_player = app.world_mut().spawn(new_bundle).id();

    app.update();

    assert_eq!(
        app.world().get::<KnownTechniques>(new_player),
        Some(&unsaved),
        "same-tick reconnect must hydrate the row written from the old activation"
    );
    assert_eq!(
        persisted_known_techniques(&persistence, "Azure"),
        Some(unsaved)
    );
    let activation = app
        .world()
        .resource::<KnownTechniquesActivations>()
        .0
        .get(&canonical_player_id("Azure"))
        .expect("new activation should replace the old one");
    assert_eq!(activation.entity, new_player);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn production_known_techniques_reconnect_retries_after_save_failure() {
    let (mut app, persistence, settings, root) =
        known_techniques_app("known-techniques-production-reconnect-retry");
    let (old_bundle, _old_helper) = create_mock_client("Azure");
    let old_player = app.world_mut().spawn(old_bundle).id();
    app.update();

    let unsaved = known_techniques_fixture("movement.dash", 0.85);
    app.world_mut()
        .entity_mut(old_player)
        .insert(unsaved.clone());
    app.world_mut().entity_mut(old_player).remove::<Client>();
    let (new_bundle, _new_helper) = create_mock_client("Azure");
    let new_player = app.world_mut().spawn(new_bundle).id();
    let backup_path = root.join("retry-backup.db");
    fs::rename(settings.db_path(), &backup_path)
        .expect("fixture database should move out of the production path");
    fs::create_dir(settings.db_path())
        .expect("directory placeholder should force the disconnect save to fail");

    app.update();

    let subject = canonical_player_id("Azure");
    assert_eq!(
        app.world()
            .resource::<KnownTechniquesActivations>()
            .0
            .get(&subject)
            .map(|activation| activation.entity),
        Some(old_player),
        "failed save must retain the old activation for a later retry"
    );
    assert_eq!(
        app.world()
            .resource::<PendingKnownTechniquesHandoffs>()
            .0
            .get(&subject),
        Some(&new_player),
        "failed handoff must retain the reconnect target"
    );
    assert!(
        app.world().get::<KnownTechniques>(new_player).is_none(),
        "hydrate must not run after the old activation failed to save"
    );

    fs::remove_dir(settings.db_path()).expect("directory placeholder should be removable");
    fs::rename(&backup_path, settings.db_path())
        .expect("fixture database should return to the production path");
    app.update();

    assert_eq!(
        persisted_known_techniques(&persistence, "Azure"),
        Some(unsaved.clone()),
        "retry must durably save the old activation before hydrating"
    );
    assert_eq!(
        app.world().get::<KnownTechniques>(new_player),
        Some(&unsaved)
    );
    assert!(
        !app.world()
            .resource::<PendingKnownTechniquesHandoffs>()
            .0
            .contains_key(&subject),
        "successful retry must consume the pending reconnect target"
    );
    assert_eq!(
        app.world()
            .resource::<KnownTechniquesActivations>()
            .0
            .get(&subject)
            .map(|activation| activation.entity),
        Some(new_player),
        "successful retry must replace the old activation exactly once"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn production_known_techniques_disconnect_save_failure_preserves_retry_without_handoff() {
    let (mut app, _persistence, settings, root) =
        known_techniques_app("known-techniques-production-disconnect-retry");
    let (client_bundle, _helper) = create_mock_client("Azure");
    let player = app.world_mut().spawn(client_bundle).id();
    app.update();

    app.world_mut()
        .entity_mut(player)
        .insert(known_techniques_fixture("movement.dash", 0.85));
    app.world_mut().entity_mut(player).remove::<Client>();

    let backup_path = root.join("disconnect-retry-backup.db");
    fs::rename(settings.db_path(), &backup_path)
        .expect("fixture database should move out of the production path");
    fs::create_dir(settings.db_path())
        .expect("directory placeholder should force the disconnect save to fail");

    app.update();

    let subject = canonical_player_id("Azure");
    let retry = app
        .world()
        .resource::<KnownTechniquesReconnectState>()
        .retries
        .get(&subject)
        .expect("a failed disconnected save must retain a retry entry without a reconnect handoff");
    assert_eq!(
        retry.attempts, 1,
        "the first disconnected save failure must be counted for later retry"
    );
    assert_eq!(
        retry.next_attempt_frame, 3,
        "the first disconnected save failure must retain its bounded backoff schedule"
    );
    assert!(
        app.world()
            .resource::<PendingKnownTechniquesHandoffs>()
            .0
            .is_empty(),
        "this regression must exercise the no-handoff cleanup path"
    );

    fs::remove_dir(settings.db_path()).expect("database outage placeholder should be removable");
    fs::rename(&backup_path, settings.db_path())
        .expect("the durable database should return to the production path");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn reconnect_ready_client_reruns_init_clients_welcome() {
    let (mut app, persistence, settings, root) =
        known_techniques_app("known-techniques-reconnect-ready-init");
    let initial = known_techniques_fixture("movement.dash", 0.85);
    crate::player::state::save_player_known_techniques_slice(&persistence, "Azure", &initial)
        .expect("initial known techniques should persist");

    let (old_bundle, _old_helper) = create_mock_client("Azure");
    let old_player = app.world_mut().spawn(old_bundle).id();
    app.update();

    let unsaved = known_techniques_fixture("movement.dash", 0.9);
    app.world_mut()
        .entity_mut(old_player)
        .insert(unsaved.clone());
    app.world_mut().entity_mut(old_player).remove::<Client>();
    let (new_bundle, mut new_helper) = create_mock_client("Azure");
    let new_player = app.world_mut().spawn(new_bundle).id();
    let backup_path = root.join("reconnect-ready-backup.db");
    fs::rename(settings.db_path(), &backup_path)
        .expect("fixture database should move out of the production path");
    fs::create_dir(settings.db_path())
        .expect("directory placeholder should force the disconnect save to fail");

    app.update();
    new_helper.clear_received();

    fs::remove_dir(settings.db_path()).expect("directory placeholder should be removable");
    fs::rename(&backup_path, settings.db_path())
        .expect("fixture database should return to the production path");
    app.update();

    let messages: Vec<String> = new_helper
        .collect_received()
        .0
        .into_iter()
        .filter_map(|frame| {
            frame
                .decode::<GameMessageS2c>()
                .ok()
                .map(|message| message.chat.to_legacy_lossy())
        })
        .collect();
    assert!(
        messages
            .iter()
            .any(|message| message.contains(crate::player::welcome_message())),
        "the Ready edge must rerun init_clients and deliver the welcome message; \
         actual messages={messages:?}"
    );
    assert_eq!(
        app.world().get::<KnownTechniques>(new_player),
        Some(&unsaved),
        "the delayed reconnect must still hydrate the known techniques slice"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn production_live_duplicate_login_fails_closed_without_activation_theft() {
    let (mut app, persistence, _settings, root) =
        known_techniques_app("known-techniques-production-live-duplicate");
    let initial = known_techniques_fixture("movement.dash", 0.55);
    crate::player::state::save_player_known_techniques_slice(&persistence, "Azure", &initial)
        .expect("initial known techniques should persist");

    let (old_bundle, _old_helper) = create_mock_client("Azure");
    let old_player = app.world_mut().spawn(old_bundle).id();
    app.update();

    let subject = canonical_player_id("Azure");
    let before = {
        let activation = app
            .world()
            .resource::<KnownTechniquesActivations>()
            .0
            .get(&subject)
            .expect("the first client must own the canonical activation");
        (
            activation.entity,
            activation.tracker.current_revision(),
            activation.fence.persisted_revision(),
        )
    };

    let (replacement_bundle, _replacement_helper) = create_mock_client("Azure");
    let replacement = app.world_mut().spawn(replacement_bundle).id();
    app.update();

    assert_eq!(
        app.world().get::<KnownTechniques>(old_player),
        Some(&initial),
        "a live duplicate must not remove KnownTechniques from the original entity"
    );
    let activation = app
        .world()
        .resource::<KnownTechniquesActivations>()
        .0
        .get(&subject)
        .expect("a live duplicate must retain the original activation");
    assert_eq!(activation.entity, before.0);
    assert_eq!(activation.entity, old_player);
    assert_eq!(
        activation.tracker.current_revision(),
        before.1,
        "duplicate-login preflight must not discard the original DirtyTracker"
    );
    assert_eq!(
        activation.fence.persisted_revision(),
        before.2,
        "duplicate-login preflight must not discard the original revision fence"
    );
    assert!(
        app.world()
            .get::<KnownTechniquesReconnectBlocked>(replacement)
            .is_some(),
        "the replacement target must fail closed when the canonical subject is still live"
    );
    assert!(
        app.world()
            .get::<KnownTechniquesReconnectReady>(replacement)
            .is_none(),
        "a rejected duplicate must never enter Ready hydration"
    );
    assert!(
        app.world().get::<KnownTechniques>(replacement).is_none(),
        "a rejected duplicate must not receive a second KnownTechniques activation"
    );
    assert_eq!(
        app.world()
            .resource::<PendingKnownTechniquesHandoffs>()
            .0
            .get(&subject),
        Some(&replacement),
        "the blocked replacement remains the only pending target and cannot steal the old activation"
    );

    let changed = known_techniques_fixture("movement.dash", 0.85);
    app.world_mut()
        .entity_mut(old_player)
        .insert(changed.clone());
    app.update();
    assert_eq!(
        persisted_known_techniques(&persistence, "Azure"),
        Some(changed),
        "the original live entity must still be writable after duplicate-login rejection"
    );
    assert_eq!(
        app.world()
            .resource::<KnownTechniquesActivations>()
            .0
            .get(&subject)
            .map(|activation| activation.entity),
        Some(old_player),
        "a duplicate login must never transfer the canonical activation"
    );
    assert!(
        !app.world()
            .resource::<KnownTechniquesReconnectState>()
            .retries
            .contains_key(&subject),
        "a still-live duplicate must be a stable block, not a backoff retry"
    );

    app.update();
    assert!(
        !app.world()
            .resource::<KnownTechniquesReconnectState>()
            .retries
            .contains_key(&subject),
        "repeated frames must not reintroduce retry accounting while the old client is live"
    );
    assert!(app
        .world()
        .get::<KnownTechniquesReconnectBlocked>(replacement)
        .is_some());
    assert!(app
        .world()
        .get::<KnownTechniquesReconnectReady>(replacement)
        .is_none());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn production_duplicate_pending_target_keeps_first_target_and_blocks_later_client() {
    let (mut app, persistence, _settings, root) =
        known_techniques_app("known-techniques-production-duplicate-pending");
    let initial = known_techniques_fixture("movement.dash", 0.2);
    crate::player::state::save_player_known_techniques_slice(&persistence, "Azure", &initial)
        .expect("initial known techniques should persist");

    let (first_bundle, _first_helper) = create_mock_client("Azure");
    let first_target = app.world_mut().spawn(first_bundle).id();
    let (second_bundle, _second_helper) = create_mock_client("Azure");
    let second_target = app.world_mut().spawn(second_bundle).id();
    app.update();

    let subject = canonical_player_id("Azure");
    assert_eq!(
        app.world()
            .resource::<KnownTechniquesActivations>()
            .0
            .get(&subject)
            .map(|activation| activation.entity),
        Some(first_target),
        "the first same-tick target must own the canonical activation"
    );
    assert_eq!(
        app.world().get::<KnownTechniques>(first_target),
        Some(&initial),
        "the first target must complete durable hydration"
    );
    assert!(
        app.world()
            .get::<KnownTechniquesReconnectBlocked>(second_target)
            .is_some(),
        "a later same-subject target must be blocked rather than replacing pending state"
    );
    assert!(
        app.world()
            .get::<KnownTechniquesReconnectReady>(second_target)
            .is_none(),
        "the later duplicate must not enter Ready hydration"
    );
    assert!(
        app.world().get::<KnownTechniques>(second_target).is_none(),
        "the later duplicate must not receive the first target's durable slice"
    );
    assert!(
        !app.world()
            .resource::<PendingKnownTechniquesHandoffs>()
            .0
            .contains_key(&subject),
        "successful hydration must consume the first target's pending handoff"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn production_blocked_duplicate_promoted_and_hydrated_after_first_target_despawns() {
    let (mut app, persistence, _settings, root) =
        known_techniques_app("known-techniques-production-duplicate-promotion");
    let initial = known_techniques_fixture("movement.dash", 0.2);
    crate::player::state::save_player_known_techniques_slice(&persistence, "Azure", &initial)
        .expect("initial known techniques should persist");

    let (first_bundle, _first_helper) = create_mock_client("Azure");
    let first_target = app.world_mut().spawn(first_bundle).id();
    let (second_bundle, _second_helper) = create_mock_client("Azure");
    let second_target = app.world_mut().spawn(second_bundle).id();
    app.update();

    let subject = canonical_player_id("Azure");
    assert_eq!(
        app.world().get::<KnownTechniques>(first_target),
        Some(&initial),
        "the first target must hydrate before the duplicate arrives"
    );
    assert!(
        app.world()
            .get::<KnownTechniquesReconnectBlocked>(second_target)
            .is_some(),
        "the duplicate must be blocked while the first target holds the activation"
    );
    assert!(
        app.world().get::<KnownTechniques>(second_target).is_none(),
        "the blocked duplicate must not hydrate while the first target is live"
    );

    app.world_mut().entity_mut(first_target).remove::<Client>();
    app.update();

    assert_eq!(
        app.world().get::<KnownTechniques>(second_target),
        Some(&initial),
        "after the first target disconnects, the blocked duplicate must be promoted and hydrated"
    );
    assert!(
        app.world()
            .get::<KnownTechniquesReconnectReady>(second_target)
            .is_some(),
        "promotion must emit the Ready marker that drives hydration"
    );
    assert!(
        app.world()
            .get::<KnownTechniquesReconnectBlocked>(second_target)
            .is_none(),
        "promotion must clear the block on the promoted duplicate"
    );
    assert_eq!(
        app.world()
            .resource::<KnownTechniquesActivations>()
            .0
            .get(&subject)
            .map(|activation| activation.entity),
        Some(second_target),
        "the canonical activation must transfer to the promoted duplicate"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn production_known_techniques_shutdown_flushes_dirty_activation() {
    let (mut app, persistence, _settings, root) =
        known_techniques_app("known-techniques-production-shutdown");
    let (client_bundle, _helper) = create_mock_client("Azure");
    let player = app.world_mut().spawn(client_bundle).id();
    app.update();
    let changed = known_techniques_fixture("movement.dash", 0.6);
    app.world_mut().entity_mut(player).insert(changed.clone());
    app.world_mut()
        .resource_mut::<Events<AppExit>>()
        .send(AppExit::Success);

    app.world_mut().run_schedule(Last);

    assert_eq!(
        persisted_known_techniques(&persistence, "Azure"),
        Some(changed),
        "AppExit -> Last must flush the canonical KnownTechniques activation"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn production_failed_load_stays_read_only_and_recovers_on_reconnect() {
    let (mut app, persistence, settings, root) =
        known_techniques_app("known-techniques-production-load-recovery");
    let connection = Connection::open(settings.db_path()).expect("fixture db should open");
    connection
        .execute(
            "INSERT INTO player_known_techniques (username, known_techniques_json, schema_version, last_updated_wall) VALUES (?1, ?2, ?3, ?4)",
            params!["Azure", "{broken-json", PLAYER_ROW_SCHEMA_VERSION, 1],
        )
        .expect("corrupt known techniques row should seed");
    drop(connection);
    let (old_bundle, _old_helper) = create_mock_client("Azure");
    let old_player = app.world_mut().spawn(old_bundle).id();
    app.update();

    assert!(app
        .world()
        .get::<KnownTechniquesLoadFailed>(old_player)
        .is_some());
    assert!(
        app.world().get::<PlayerState>(old_player).is_some(),
        "a corrupt known-techniques row must not prevent PlayerState attachment"
    );
    assert!(
        app.world()
            .get::<KnownTechniquesReconnectFailed>(old_player)
            .is_some(),
        "a corrupt known-techniques row must mark only that slice as degraded"
    );
    assert!(
        app.world()
            .get::<KnownTechniquesReconnectBlocked>(old_player)
            .is_none(),
        "a corrupt row must not be treated as a duplicate-login block"
    );
    app.world_mut()
        .entity_mut(old_player)
        .insert(known_techniques_fixture("movement.dash", 0.99));
    app.update();
    let connection = Connection::open(settings.db_path()).expect("fixture db should reopen");
    let stored: String = connection
        .query_row(
            "SELECT known_techniques_json FROM player_known_techniques WHERE username = ?1",
            params!["Azure"],
            |row| row.get(0),
        )
        .expect("corrupt row should remain present");
    assert_eq!(
        stored, "{broken-json",
        "failed provenance must block Changed writes"
    );
    drop(connection);

    let repaired = known_techniques_fixture("movement.dash", 0.35);
    crate::player::state::save_player_known_techniques_slice(&persistence, "Azure", &repaired)
        .expect("operator repair should replace the corrupt row");
    app.world_mut().entity_mut(old_player).remove::<Client>();
    let (new_bundle, _new_helper) = create_mock_client("Azure");
    let new_player = app.world_mut().spawn(new_bundle).id();
    app.update();

    assert_eq!(
        app.world().get::<KnownTechniques>(new_player),
        Some(&repaired)
    );
    assert!(app
        .world()
        .get::<KnownTechniquesLoadFailed>(new_player)
        .is_none());
    assert_eq!(
        app.world()
            .resource::<KnownTechniquesActivations>()
            .0
            .get(&canonical_player_id("Azure"))
            .map(|activation| activation.entity),
        Some(new_player),
        "failed activation lease must be releasable so a repaired row can hydrate"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn pvp_biography_variants_pin_event_type_tick_and_payload_tag() {
    let encounter = BiographyEntry::PvpEncounter {
        counterparty_id: "char:bob".to_string(),
        outcome: "betrayal".to_string(),
        zone: "blood_valley".to_string(),
        context: "tsy_extract".to_string(),
        observed_style: Some("woliu".to_string()),
        appearance_hint: Some("右手持骨刺".to_string()),
        qi_color_hint: Some("青白".to_string()),
        tick: 91,
    };
    let betrayal = BiographyEntry::PvpBetrayal {
        betrayer_id: "char:bob".to_string(),
        victim_id: "char:alice".to_string(),
        scene: "tsy_extract".to_string(),
        npc_witnessed: true,
        tick: 92,
    };

    for (entry, expected_event_type, wrong_event_type, expected_tick, expected_tag) in [
        (
            encounter,
            "pvp_encounter",
            "pvp_betrayal",
            91_u64,
            "PvpEncounter",
        ),
        (
            betrayal,
            "pvp_betrayal",
            "pvp_encounter",
            92_u64,
            "PvpBetrayal",
        ),
    ] {
        assert_eq!(biography_event_type(&entry), expected_event_type);
        assert_ne!(biography_event_type(&entry), wrong_event_type);
        assert_eq!(biography_tick(&entry), expected_tick);

        let payload_json = serde_json::to_string(&LifeEventPayload {
            biography_entry: entry.clone(),
        })
        .expect("pvp life event payload should serialize");
        let payload_value: Value =
            serde_json::from_str(&payload_json).expect("pvp life event payload should be json");
        assert!(
            payload_value["biography_entry"].get(expected_tag).is_some(),
            "expected pvp biography payload tag {expected_tag}, actual {payload_value}"
        );

        let decoded: LifeEventPayload =
            serde_json::from_str(&payload_json).expect("pvp life event payload should deserialize");
        assert_eq!(
            biography_event_type(&decoded.biography_entry),
            expected_event_type
        );
        assert_eq!(biography_tick(&decoded.biography_entry), expected_tick);
    }
}

#[test]
fn mutation_advanced_biography_pin_event_type_tick_and_payload_tag() {
    let entry = BiographyEntry::MutationAdvanced {
        from_stage: 1,
        to_stage: 2,
        cumulative_toxin: 105.5,
        tick: 8888,
    };

    // Pin event type.
    assert_eq!(
        biography_event_type(&entry),
        "mutation_advanced",
        "MutationAdvanced event type should be 'mutation_advanced'"
    );
    assert_ne!(
        biography_event_type(&entry),
        "mutation",
        "MutationAdvanced event type should not be 'mutation'"
    );

    // Pin tick extraction.
    assert_eq!(
        biography_tick(&entry),
        8888,
        "MutationAdvanced tick should be 8888"
    );

    // Pin LifeEventPayload serialization tag.
    let payload_json = serde_json::to_string(&LifeEventPayload {
        biography_entry: entry.clone(),
    })
    .expect("MutationAdvanced life event payload should serialize");
    let payload_value: Value =
        serde_json::from_str(&payload_json).expect("payload should be valid json");
    assert!(
        payload_value["biography_entry"]
            .get("MutationAdvanced")
            .is_some(),
        "expected MutationAdvanced tag in serialized payload, actual: {payload_value}"
    );

    // Pin round-trip.
    let decoded: LifeEventPayload = serde_json::from_str(&payload_json)
        .expect("MutationAdvanced life event payload should deserialize");
    assert_eq!(
        biography_event_type(&decoded.biography_entry),
        "mutation_advanced"
    );
    assert_eq!(biography_tick(&decoded.biography_entry), 8888);

    // Verify fields survive round-trip.
    match &decoded.biography_entry {
        BiographyEntry::MutationAdvanced {
            from_stage,
            to_stage,
            cumulative_toxin,
            tick,
        } => {
            assert_eq!(*from_stage, 1);
            assert_eq!(*to_stage, 2);
            assert!((cumulative_toxin - 105.5).abs() < f64::EPSILON);
            assert_eq!(*tick, 8888);
        }
        other => panic!("expected MutationAdvanced, got {other:?}"),
    }
}

fn unique_temp_dir(test_name: &str) -> PathBuf {
    let unique_suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();

    std::env::temp_dir().join(format!(
        "bong-persistence-{test_name}-{}-{unique_suffix}",
        std::process::id()
    ))
}

fn database_path(test_name: &str) -> PathBuf {
    unique_temp_dir(test_name).join("bong.db")
}

fn reject_if_user_version_exceeds_supported(
    connection: &Connection,
    max_supported_user_version: i32,
) -> rusqlite::Result<()> {
    let user_version: i32 = connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if user_version > max_supported_user_version {
        return Err(rusqlite::Error::ExecuteReturnedResults);
    }
    Ok(())
}

fn persistence_settings(test_name: &str) -> (PersistenceSettings, PathBuf) {
    let root = unique_temp_dir(test_name);
    let db_path = root.join("data").join("bong.db");
    (
        PersistenceSettings::with_db_path(&db_path, format!("task3-{test_name}")),
        root,
    )
}

fn heartbeat_pseudo_vein_record(zone_id: &str) -> HeartbeatPseudoVeinRecord {
    HeartbeatPseudoVeinRecord {
        zone_id: zone_id.to_string(),
        dimension: DimensionKind::Overworld,
        bounds_min: [-140.0, 60.0, -240.0],
        bounds_max: [160.0, 90.0, 60.0],
        danger_level: 4,
        active_events: vec![crate::world::heartbeat::EVENT_PSEUDO_VEIN.to_string()],
        patrol_anchors: vec![[10.0, 65.0, -90.0]],
        center_xz: [10.0, -90.0],
        spawned_at_tick: 1_000,
        last_tick: 1_800,
        qi_current: 0.37,
        total_qi_consumed: 0.23,
        warning_sent: true,
        dissipated: false,
        season_at_spawn: PseudoVeinSeasonV1::SummerToWinter,
        observed_age_ticks: 800,
        pending_runtime_ticks: 0,
        pending_offline_ticks: 0,
        occupant_count: 0,
        eval_elapsed_ticks: 0,
        snapshot_wall: 0,
    }
}

fn heartbeat_zone_runtime_record(
    record: &HeartbeatPseudoVeinRecord,
    spirit_qi: f64,
) -> ZoneRuntimeRecord {
    ZoneRuntimeRecord {
        zone_id: record.zone_id.clone(),
        spirit_qi,
        danger_level: record.danger_level,
    }
}

fn assert_pseudo_vein_startup_fails_closed(
    settings: &PersistenceSettings,
    zone_id: &str,
    expected_lifecycle_rows: i64,
    expected_runtime_rows: i64,
) {
    let mut app = App::new();
    app.insert_resource(settings.clone());
    app.insert_resource(DailyBackupState::default());
    app.insert_resource(crate::world::zone::ZoneRegistry::fallback());
    app.insert_resource(WorldHeartbeat::default());
    app.insert_resource(CultivationClock::default());
    app.insert_resource(WorldQiAccount::default());
    app.add_systems(Startup, bootstrap_persistence_system);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| app.update()));
    assert!(
        result.is_err(),
        "expected inconsistent pseudo-vein persistence to stop startup"
    );
    assert_eq!(
        app.world()
            .resource::<WorldHeartbeat>()
            .active_pseudo_vein_count(),
        0,
        "failed startup must not install a partial pseudo-vein lifecycle"
    );
    assert!(
        app.world()
            .resource::<crate::world::zone::ZoneRegistry>()
            .find_zone_by_name(zone_id)
            .is_none(),
        "failed startup must not install a partial pseudo-vein zone"
    );
    assert_eq!(
        app.world()
            .resource::<WorldQiAccount>()
            .balance(&QiAccountId::zone(zone_id)),
        0.0,
        "failed startup must not mint a pseudo-vein ledger balance"
    );

    let connection = Connection::open(settings.db_path()).expect("fixture db should reopen");
    let lifecycle_rows: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM heartbeat_pseudo_veins WHERE zone_id = ?1",
            params![zone_id],
            |row| row.get(0),
        )
        .expect("heartbeat row count should query");
    let runtime_rows: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM zones_runtime WHERE zone_id = ?1",
            params![zone_id],
            |row| row.get(0),
        )
        .expect("zone runtime row count should query");
    assert_eq!(
        lifecycle_rows, expected_lifecycle_rows,
        "failed startup must preserve the authoritative lifecycle row count"
    );
    assert_eq!(
        runtime_rows, expected_runtime_rows,
        "failed startup must preserve the authoritative runtime row count"
    );
}

#[test]
fn runtime_system_throttles_live_npc_snapshots_between_intervals() {
    let (settings, root) = persistence_settings("live-npc-runtime-throttle");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let mut app = App::new();
    app.insert_resource(settings.clone());
    app.insert_resource(NpcSnapshotTracker::default());
    app.insert_resource(crate::npc::movement::GameTick(0));
    app.add_systems(Update, persist_npc_runtime_state_system);

    let npc = app
        .world_mut()
        .spawn((
            NpcMarker,
            Position::new([1.0, 66.0, 1.0]),
            EntityKind::ZOMBIE,
            NpcBlackboard::default(),
            NpcCombatLoadout::civilian(),
            NpcPatrol::new(DEFAULT_SPAWN_ZONE_NAME, DVec3::new(4.0, 66.0, 4.0)),
            MovementController::new(),
            MovementCooldowns::default(),
            Lifecycle {
                character_id: "npc:runtime-throttle".to_string(),
                state: LifecycleState::Alive,
                fortune_remaining: 0,
                ..Default::default()
            },
        ))
        .id();

    app.update();
    assert!(
        app.world().get::<NpcLivePersistenceSnapshot>(npc).is_some(),
        "first live snapshot should mark the NPC so subsequent ticks skip sqlite writes"
    );
    let first = load_npc_state(&settings, "npc:runtime-throttle")
        .expect("npc state lookup should succeed")
        .expect("first snapshot should persist live npc");
    assert_eq!(first.pos, [1.0, 66.0, 1.0]);

    *app.world_mut().get_mut::<Position>(npc).unwrap() = Position::new([9.0, 66.0, 9.0]);
    app.world_mut()
        .resource_mut::<crate::npc::movement::GameTick>()
        .0 = 1;
    app.update();
    let before_interval = load_npc_state(&settings, "npc:runtime-throttle")
        .expect("npc state lookup should succeed")
        .expect("live npc row should still exist");
    assert_eq!(
        before_interval.pos,
        [1.0, 66.0, 1.0],
        "live NPC runtime persistence must not write every tick"
    );

    app.world_mut()
        .resource_mut::<crate::npc::movement::GameTick>()
        .0 = NPC_SNAPSHOT_INTERVAL_TICKS;
    app.update();
    let after_interval = load_npc_state(&settings, "npc:runtime-throttle")
        .expect("npc state lookup should succeed")
        .expect("interval snapshot should keep live npc row");
    assert_eq!(after_interval.pos, [9.0, 66.0, 9.0]);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn persistence_bootstrap_enables_wal_and_integrity_check() {
    let db_path = database_path("wal-integrity");
    bootstrap_sqlite(&db_path, "server-run-test").expect("bootstrap should succeed");

    let connection = Connection::open(&db_path).expect("db should open");
    let journal_mode: String = connection
        .query_row("PRAGMA journal_mode;", [], |row| row.get(0))
        .expect("journal mode should be readable");
    assert_eq!(journal_mode.to_ascii_lowercase(), "wal");

    let integrity: String = connection
        .query_row("PRAGMA integrity_check;", [], |row| row.get(0))
        .expect("integrity check should run");
    assert_eq!(integrity, "ok");

    let stored_server_run_id: String = connection
        .query_row(
            "SELECT server_run_id FROM bootstrap_events LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("bootstrap event should exist");
    assert_eq!(stored_server_run_id, "server-run-test");
}

#[test]
fn persistence_migrations_are_ordered_and_idempotent() {
    let db_path = database_path("migrations");
    bootstrap_sqlite(&db_path, "first-run").expect("first bootstrap should succeed");
    bootstrap_sqlite(&db_path, "second-run").expect("second bootstrap should succeed");

    let connection = Connection::open(&db_path).expect("db should open");
    let user_version: i32 = connection
        .query_row("PRAGMA user_version;", [], |row| row.get(0))
        .expect("user_version should exist");
    assert_eq!(
        user_version, CURRENT_USER_VERSION,
        "expected user_version to advance to CURRENT_USER_VERSION ({CURRENT_USER_VERSION}) because all migrations should finish after bootstrap, actual {user_version}"
    );

    let has_index: Option<String> = connection
        .query_row(
            "SELECT name FROM sqlite_master WHERE type = 'index' AND name = 'idx_bootstrap_events_wall_clock'",
            [],
            |row| row.get(0),
        )
        .optional()
        .expect("sqlite_master query should succeed");
    assert_eq!(
        has_index.as_deref(),
        Some("idx_bootstrap_events_wall_clock")
    );

    let player_core_exists: Option<String> = connection
        .query_row(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'player_core'",
            [],
            |row| row.get(0),
        )
        .optional()
        .expect("sqlite_master player_core query should succeed");
    assert_eq!(player_core_exists.as_deref(), Some("player_core"));

    for table in [
        "social_anonymity",
        "social_relationships",
        "social_exposures",
        "social_renown",
        "social_spirit_niches",
        "social_faction_memberships",
        "void_action_cooldowns",
        "high_renown_milestones",
        "spirit_treasure_world",
        "spirit_treasure_dialogue_log",
    ] {
        let exists: Option<String> = connection
            .query_row(
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name = ?1",
                params![table],
                |row| row.get(0),
            )
            .optional()
            .expect("sqlite_master social table query should succeed");
        assert_eq!(exists.as_deref(), Some(table), "{table} should exist");
    }

    for column in [
        "player_uuid",
        "char_id",
        "identity_id",
        "milestone",
        "emitted_at_tick",
        "schema_version",
        "last_updated_wall",
    ] {
        let column_exists: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('high_renown_milestones') WHERE name = ?1",
                params![column],
                |row| row.get(0),
            )
            .expect("high_renown_milestones column query should succeed");
        assert_eq!(column_exists, 1, "{column} should exist");
    }

    let high_renown_index: Option<String> = connection
        .query_row(
            "SELECT name FROM sqlite_master WHERE type = 'index' AND name = 'idx_high_renown_milestones_char'",
            [],
            |row| row.get(0),
        )
        .optional()
        .expect("high renown index query should succeed");
    assert_eq!(
        high_renown_index.as_deref(),
        Some("idx_high_renown_milestones_char")
    );

    let mut high_renown_pk_statement = connection
        .prepare("PRAGMA table_info(high_renown_milestones)")
        .expect("high renown table_info should prepare");
    let high_renown_pk = high_renown_pk_statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, i32>(5)?))
        })
        .expect("high renown table_info query should succeed")
        .collect::<Result<Vec<_>, _>>()
        .expect("high renown table_info rows should collect")
        .into_iter()
        .filter(|(_, pk_ordinal)| *pk_ordinal > 0)
        .collect::<Vec<_>>();
    assert_eq!(
        high_renown_pk,
        [
            ("player_uuid".to_string(), 1),
            ("identity_id".to_string(), 2),
            ("milestone".to_string(), 3),
        ]
    );
}

#[test]
fn v20_migration_rejects_malformed_high_renown_table() {
    let db_path = database_path("v20-malformed-high-renown");
    fs::create_dir_all(db_path.parent().expect("db path should have parent"))
        .expect("temp db parent should be created");
    let mut connection = Connection::open(&db_path).expect("db should open");
    connection
        .execute_batch(
            "
            CREATE TABLE high_renown_milestones (
                player_uuid TEXT NOT NULL,
                identity_id INTEGER NOT NULL,
                milestone INTEGER NOT NULL,
                PRIMARY KEY (player_uuid, identity_id, milestone)
            );
            PRAGMA user_version = 19;
            ",
        )
        .expect("legacy malformed fixture should be created");

    let error =
        apply_migrations(&mut connection).expect_err("v20 migration should reject malformed table");
    let message = error.to_string();
    assert!(
        message.contains("high_renown_milestones column char_id missing")
            || message.contains("no such column: char_id"),
        "unexpected error: {message}"
    );
    let user_version: i32 = connection
        .query_row("PRAGMA user_version;", [], |row| row.get(0))
        .expect("user_version should still be readable");
    assert_eq!(user_version, 19);
}

#[test]
fn v20_migration_rejects_high_renown_table_with_wrong_primary_key() {
    let db_path = database_path("v20-wrong-high-renown-pk");
    fs::create_dir_all(db_path.parent().expect("db path should have parent"))
        .expect("temp db parent should be created");
    let mut connection = Connection::open(&db_path).expect("db should open");
    connection
        .execute_batch(
            "
            CREATE TABLE high_renown_milestones (
                player_uuid TEXT NOT NULL,
                char_id TEXT NOT NULL,
                identity_id INTEGER NOT NULL,
                milestone INTEGER NOT NULL,
                emitted_at_tick INTEGER NOT NULL,
                schema_version INTEGER NOT NULL,
                last_updated_wall INTEGER NOT NULL,
                PRIMARY KEY (char_id, identity_id, milestone)
            );
            CREATE INDEX idx_high_renown_milestones_char
            ON high_renown_milestones (char_id, identity_id, milestone);
            PRAGMA user_version = 19;
            ",
        )
        .expect("legacy wrong primary key fixture should be created");

    let error = apply_migrations(&mut connection)
        .expect_err("v20 migration should reject wrong high renown primary key");
    let message = error.to_string();
    assert!(
        message.contains("high_renown_milestones primary key mismatch"),
        "unexpected error: {message}"
    );
    let user_version: i32 = connection
        .query_row("PRAGMA user_version;", [], |row| row.get(0))
        .expect("user_version should still be readable");
    assert_eq!(user_version, 19);
}

#[test]
fn v19_migration_rejects_partial_void_action_cooldowns_schema() {
    let db_path = database_path("v19-partial-void-action-cooldowns");
    fs::create_dir_all(db_path.parent().expect("db path should have parent"))
        .expect("temp db parent should be created");
    let connection = Connection::open(&db_path).expect("db should open");
    connection
        .execute_batch(
            "
            CREATE TABLE void_action_cooldowns (
                character_id TEXT NOT NULL
            );
            PRAGMA user_version = 18;
            ",
        )
        .expect("partial cooldown table should be created");
    drop(connection);

    bootstrap_sqlite(&db_path, "server-run-test")
        .expect_err("partial void_action_cooldowns schema must block v19 migration");

    let connection = Connection::open(&db_path).expect("db should reopen");
    let user_version: i32 = connection
        .query_row("PRAGMA user_version;", [], |row| row.get(0))
        .expect("user_version should be readable");
    assert_eq!(user_version, 18);
}

#[test]
fn v19_migration_rejects_void_action_cooldowns_without_composite_primary_key() {
    let db_path = database_path("v19-void-action-cooldowns-bad-primary-key");
    fs::create_dir_all(db_path.parent().expect("db path should have parent"))
        .expect("temp db parent should be created");
    let connection = Connection::open(&db_path).expect("db should open");
    connection
        .execute_batch(
            "
            CREATE TABLE void_action_cooldowns (
                character_id TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                ready_at_tick INTEGER NOT NULL CHECK (ready_at_tick >= 0),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            PRAGMA user_version = 18;
            ",
        )
        .expect("bad cooldown table should be created");
    drop(connection);

    bootstrap_sqlite(&db_path, "server-run-test")
        .expect_err("void_action_cooldowns must keep composite primary key");

    let connection = Connection::open(&db_path).expect("db should reopen");
    let user_version: i32 = connection
        .query_row("PRAGMA user_version;", [], |row| row.get(0))
        .expect("user_version should be readable");
    assert_eq!(user_version, 18);
}

#[test]
fn void_action_cooldowns_roundtrip_hydrates_resource() {
    let (settings, _root) = persistence_settings("void-action-cooldowns-roundtrip");
    bootstrap_sqlite(settings.db_path(), "server-run-test").expect("bootstrap should succeed");
    persist_void_action_cooldown(&settings, "offline:Void", VoidActionKind::Barrier, 12_345)
        .expect("cooldown should persist");

    let mut cooldowns = VoidActionCooldowns::default();
    let count =
        hydrate_void_action_cooldowns(&settings, &mut cooldowns).expect("cooldowns should hydrate");

    assert_eq!(count, 1);
    assert_eq!(
        cooldowns.ready_at("offline:Void", VoidActionKind::Barrier),
        12_345
    );
}

#[test]
fn void_action_cooldown_negative_tick_fails_closed_during_hydrate() {
    let (settings, root) = persistence_settings("void-action-negative-tick");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");
    persist_void_action_cooldown(&settings, "offline:Void", VoidActionKind::Barrier, 12_345)
        .expect("valid cooldown should persist before corruption");

    let connection = Connection::open(settings.db_path()).expect("sqlite should open");
    connection
        .execute_batch("PRAGMA ignore_check_constraints = ON;")
        .expect("fixture should allow deliberate negative-value corruption");
    connection
        .execute(
            "UPDATE void_action_cooldowns SET ready_at_tick = -1 WHERE character_id = ?1",
            params!["offline:Void"],
        )
        .expect("negative cooldown fixture should be writable");
    drop(connection);

    let mut cooldowns = VoidActionCooldowns::default();
    let error = hydrate_void_action_cooldowns(&settings, &mut cooldowns)
        .expect_err("negative persisted cooldown must fail closed");
    assert!(
        error
            .to_string()
            .contains("negative void-action cooldown tick"),
        "hydrate error should explain the rejected signed tick, actual={error}"
    );
    assert_eq!(
        cooldowns.ready_at("offline:Void", VoidActionKind::Barrier),
        0,
        "failed cooldown hydrate must not install a reinterpreted u64::MAX deadline"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn task13_migration_backfills_legacy_player_cultivation() {
    let db_path = database_path("task13-legacy-cultivation-backfill");
    fs::create_dir_all(db_path.parent().expect("db path should have parent"))
        .expect("temp db parent should be created");

    let mut connection = Connection::open(&db_path).expect("db should open");
    connection
        .execute_batch(
            "
            CREATE TABLE player_core (
                username TEXT PRIMARY KEY,
                current_char_id TEXT NOT NULL,
                realm TEXT NOT NULL,
                spirit_qi REAL NOT NULL,
                spirit_qi_max REAL NOT NULL,
                karma REAL NOT NULL,
                experience INTEGER NOT NULL,
                inventory_score REAL NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            CREATE TABLE player_slow (
                username TEXT PRIMARY KEY,
                pos_x REAL NOT NULL,
                pos_y REAL NOT NULL,
                pos_z REAL NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            PRAGMA user_version = 12;
            ",
        )
        .expect("legacy schema should be created");
    connection
        .execute(
            "
            INSERT INTO player_core (
                username,
                current_char_id,
                realm,
                spirit_qi,
                spirit_qi_max,
                karma,
                experience,
                inventory_score,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ",
            params![
                "Azure",
                canonical_player_id("Azure"),
                "qi_refining_3",
                77.5_f64,
                123.0_f64,
                0.25_f64,
                900_i64,
                0.5_f64,
                CURRENT_SCHEMA_VERSION,
                1_i64,
            ],
        )
        .expect("legacy player should be inserted");

    apply_migrations(&mut connection).expect("v13 migration should succeed");

    let user_version: i32 = connection
        .query_row("PRAGMA user_version;", [], |row| row.get(0))
        .expect("user_version should be readable");
    assert_eq!(
        user_version, CURRENT_USER_VERSION,
        "expected user_version to advance to CURRENT_USER_VERSION ({CURRENT_USER_VERSION}) because v13 migration should succeed, actual {user_version}"
    );

    for dropped_column in ["realm", "spirit_qi", "spirit_qi_max", "experience"] {
        let count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('player_core') WHERE name = ?1",
                params![dropped_column],
                |row| row.get(0),
            )
            .expect("player_core table_info should be readable");
        assert_eq!(count, 0, "{dropped_column} should be dropped");
    }

    let cultivation_json: String = connection
        .query_row(
            "SELECT cultivation_json FROM player_cultivation WHERE username = ?1",
            params!["Azure"],
            |row| row.get(0),
        )
        .expect("backfilled cultivation row should exist");
    let bundle: Value =
        serde_json::from_str(&cultivation_json).expect("cultivation bundle should deserialize");

    assert_eq!(bundle["cultivation"]["realm"].as_str(), Some("Spirit"));
    assert_eq!(bundle["cultivation"]["qi_current"].as_f64(), Some(77.5));
    assert_eq!(bundle["cultivation"]["qi_max"].as_f64(), Some(123.0));
    assert_eq!(
        bundle["life_record"]["character_id"].as_str(),
        Some(canonical_player_id("Azure").as_str())
    );

    let _ = fs::remove_dir_all(db_path.parent().expect("db path should have parent"));
}

#[test]
fn legacy_player_negative_qi_migration_fails_closed_without_partial_backfill() {
    let db_path = database_path("legacy-negative-qi-backfill");
    fs::create_dir_all(db_path.parent().expect("db path should have parent"))
        .expect("temp db parent should be created");

    let mut connection = Connection::open(&db_path).expect("db should open");
    connection
        .execute_batch(
            "
            CREATE TABLE player_core (
                username TEXT PRIMARY KEY,
                current_char_id TEXT NOT NULL,
                realm TEXT NOT NULL,
                spirit_qi REAL NOT NULL,
                spirit_qi_max REAL NOT NULL,
                karma REAL NOT NULL,
                experience INTEGER NOT NULL,
                inventory_score REAL NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            CREATE TABLE player_slow (
                username TEXT PRIMARY KEY,
                pos_x REAL NOT NULL,
                pos_y REAL NOT NULL,
                pos_z REAL NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            PRAGMA user_version = 12;
            ",
        )
        .expect("legacy schema should be created");
    connection
        .execute(
            "
            INSERT INTO player_core (
                username, current_char_id, realm, spirit_qi, spirit_qi_max,
                karma, experience, inventory_score, schema_version, last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ",
            params![
                "Azure",
                canonical_player_id("Azure"),
                "qi_refining_3",
                -0.5_f64,
                123.0_f64,
                0.25_f64,
                900_i64,
                0.5_f64,
                CURRENT_SCHEMA_VERSION,
                1_i64,
            ],
        )
        .expect("negative legacy player should be insertable into the old schema");

    let error = apply_migrations(&mut connection)
        .expect_err("negative legacy player qi must not be clamped into a new snapshot");
    assert!(
        error.to_string().contains("legacy_player.spirit_qi"),
        "migration error should identify the rejected qi field, actual={error}"
    );
    let user_version: i32 = connection
        .query_row("PRAGMA user_version;", [], |row| row.get(0))
        .expect("user_version should remain queryable after rollback");
    assert_eq!(
        user_version, 12,
        "failed legacy qi validation must roll back the v13 migration"
    );
    let cultivation_table_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'player_cultivation'",
            [],
            |row| row.get(0),
        )
        .expect("sqlite_master should remain queryable after rollback");
    assert_eq!(
        cultivation_table_count, 0,
        "failed legacy qi migration must not leave a partial cultivation table"
    );
    let _ = fs::remove_dir_all(db_path.parent().expect("db path should have parent"));
}

#[test]
fn phase7_migration_drill_upgrades_legacy_v12_fixture_to_current_schema() {
    let db_path = database_path("phase7-v12-fixture");
    fs::create_dir_all(db_path.parent().expect("db path should have parent"))
        .expect("temp db parent should be created");

    let mut connection = Connection::open(&db_path).expect("legacy db should open");
    connection
        .execute_batch(
            "
            CREATE TABLE player_core (
                username TEXT PRIMARY KEY,
                current_char_id TEXT NOT NULL,
                realm TEXT NOT NULL,
                spirit_qi REAL NOT NULL,
                spirit_qi_max REAL NOT NULL,
                karma REAL NOT NULL,
                experience INTEGER NOT NULL,
                inventory_score REAL NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            CREATE TABLE player_slow (
                username TEXT PRIMARY KEY,
                pos_x REAL NOT NULL,
                pos_y REAL NOT NULL,
                pos_z REAL NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            PRAGMA user_version = 12;
            ",
        )
        .expect("legacy v12 fixture schema should be created");
    connection
        .execute(
            "
            INSERT INTO player_core (
                username, current_char_id, realm, spirit_qi, spirit_qi_max,
                karma, experience, inventory_score, schema_version, last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ",
            params![
                "Azure",
                canonical_player_id("Azure"),
                "foundation_2",
                51.0_f64,
                90.0_f64,
                -0.25_f64,
                700_i64,
                0.42_f64,
                CURRENT_SCHEMA_VERSION,
                12_i64,
            ],
        )
        .expect("legacy player_core row should insert");
    connection
        .execute(
            "
            INSERT INTO player_slow (
                username, pos_x, pos_y, pos_z, schema_version, last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ",
            params![
                "Azure",
                7.0_f64,
                70.0_f64,
                -9.0_f64,
                CURRENT_SCHEMA_VERSION,
                12_i64,
            ],
        )
        .expect("legacy player_slow row should insert");

    apply_migrations(&mut connection).expect("legacy v12 fixture should migrate to current");

    let user_version: i32 = connection
        .query_row("PRAGMA user_version;", [], |row| row.get(0))
        .expect("user_version should be readable");
    assert_eq!(
        user_version, CURRENT_USER_VERSION,
        "expected user_version to advance to CURRENT_USER_VERSION ({CURRENT_USER_VERSION}) because legacy v12 fixture should migrate to current, actual {user_version}"
    );

    for table in [
        "player_known_techniques",
        "player_shrine",
        "player_cultivation",
        "social_anonymity",
        "social_relationships",
        "social_exposures",
        "social_renown",
        "social_spirit_niches",
        "social_faction_memberships",
    ] {
        let exists: Option<String> = connection
            .query_row(
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name = ?1",
                params![table],
                |row| row.get(0),
            )
            .optional()
            .expect("sqlite_master table query should succeed");
        assert_eq!(exists.as_deref(), Some(table), "{table} should exist");
    }

    let social_index: Option<String> = connection
        .query_row(
            "SELECT name FROM sqlite_master WHERE type = 'index' AND name = 'idx_social_exposures_char_tick'",
            [],
            |row| row.get(0),
        )
        .optional()
        .expect("social exposure index query should succeed");
    assert_eq!(
        social_index.as_deref(),
        Some("idx_social_exposures_char_tick")
    );

    let last_dimension: String = connection
        .query_row(
            "SELECT last_dimension FROM player_slow WHERE username = ?1",
            params!["Azure"],
            |row| row.get(0),
        )
        .expect("player_slow migrated row should exist");
    assert_eq!(last_dimension, "overworld");

    let player_core: (String, f64, f64) = connection
        .query_row(
            "SELECT current_char_id, karma, inventory_score FROM player_core WHERE username = ?1",
            params!["Azure"],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("player_core migrated row should exist");
    assert_eq!(player_core.0, canonical_player_id("Azure"));
    assert_eq!(player_core.1, -0.25);
    assert_eq!(player_core.2, 0.42);

    let cultivation_json: String = connection
        .query_row(
            "SELECT cultivation_json FROM player_cultivation WHERE username = ?1",
            params!["Azure"],
            |row| row.get(0),
        )
        .expect("player_cultivation backfill should exist");
    let cultivation_bundle: Value = serde_json::from_str(cultivation_json.as_str())
        .expect("player_cultivation backfill should be JSON");
    assert_eq!(
        cultivation_bundle["life_record"]["character_id"].as_str(),
        Some(canonical_player_id("Azure").as_str())
    );

    let _ = fs::remove_dir_all(db_path.parent().expect("db path should have parent"));
}

#[test]
fn startup_backup_creates_pre_bootstrap_snapshot_for_existing_db() {
    let (settings, root) = persistence_settings("startup-backup-pre-bootstrap");
    let wall_clock = 1_735_689_600;
    bootstrap_sqlite(settings.db_path(), "first-run").expect("first bootstrap should succeed");

    let backup_path = run_startup_backup(&settings, wall_clock)
        .expect("startup backup should succeed")
        .expect("existing db should produce a startup backup");
    bootstrap_sqlite(settings.db_path(), "second-run").expect("second bootstrap should succeed");

    assert_eq!(backup_path, startup_backup_path(&settings, wall_clock));
    assert!(backup_path.exists(), "startup backup file should exist");

    let live_connection = Connection::open(settings.db_path()).expect("live db should open");
    let live_bootstrap_events: i64 = live_connection
        .query_row("SELECT COUNT(*) FROM bootstrap_events", [], |row| {
            row.get(0)
        })
        .expect("live bootstrap event count should be readable");
    assert_eq!(live_bootstrap_events, 2);

    let backup_connection = Connection::open(&backup_path).expect("backup db should open");
    let backup_bootstrap_events: i64 = backup_connection
        .query_row("SELECT COUNT(*) FROM bootstrap_events", [], |row| {
            row.get(0)
        })
        .expect("backup bootstrap event count should be readable");
    let integrity: String = backup_connection
        .query_row("PRAGMA integrity_check;", [], |row| row.get(0))
        .expect("backup integrity check should run");
    assert_eq!(backup_bootstrap_events, 1);
    assert_eq!(integrity, "ok");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn startup_backup_retention_keeps_latest_seven_matching_backups() {
    let (settings, root) = persistence_settings("startup-backup-retention");
    let backup_root = resolve_persistence_relative_path(&settings, STARTUP_BACKUP_DIR);
    fs::create_dir_all(&backup_root).expect("backup root should be creatable");

    for stamp in [
        "20240101-000000",
        "20240102-000000",
        "20240103-000000",
        "20240104-000000",
        "20240105-000000",
        "20240106-000000",
        "20240107-000000",
        "20240108-000000",
        "20240109-000000",
    ] {
        fs::write(
            backup_root.join(format!(
                "{STARTUP_BACKUP_FILE_PREFIX}{stamp}{STARTUP_BACKUP_FILE_SUFFIX}",
            )),
            b"snapshot",
        )
        .expect("backup fixture should be writable");
    }
    let unrelated = backup_root.join("note.txt");
    fs::write(&unrelated, b"keep-me").expect("unrelated fixture should be writable");

    let pruned = prune_startup_backups(&settings, STARTUP_BACKUP_KEEP_COUNT)
        .expect("startup backup pruning should succeed");
    let pruned_names = pruned
        .iter()
        .map(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .expect("pruned backup should have a valid file name")
                .to_string()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        pruned_names,
        vec![
            "bong-20240101-000000.db".to_string(),
            "bong-20240102-000000.db".to_string(),
        ]
    );

    let mut remaining = collect_files_with_suffix(&backup_root, STARTUP_BACKUP_FILE_SUFFIX)
        .expect("remaining backups should be enumerable")
        .into_iter()
        .filter_map(|path| {
            let name = path.file_name()?.to_str()?;
            if name.starts_with(STARTUP_BACKUP_FILE_PREFIX)
                && name.ends_with(STARTUP_BACKUP_FILE_SUFFIX)
            {
                Some(name.to_string())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    remaining.sort();
    assert_eq!(
        remaining,
        vec![
            "bong-20240103-000000.db".to_string(),
            "bong-20240104-000000.db".to_string(),
            "bong-20240105-000000.db".to_string(),
            "bong-20240106-000000.db".to_string(),
            "bong-20240107-000000.db".to_string(),
            "bong-20240108-000000.db".to_string(),
            "bong-20240109-000000.db".to_string(),
        ]
    );
    assert!(
        unrelated.exists(),
        "unrelated backup-root files should remain untouched"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn startup_backup_skips_when_db_does_not_exist() {
    let (settings, root) = persistence_settings("startup-backup-missing-db");
    let backup = run_startup_backup(&settings, 1_735_689_600)
        .expect("missing db should skip backup without error");

    assert!(backup.is_none());
    assert!(
        !resolve_persistence_relative_path(&settings, STARTUP_BACKUP_DIR).exists(),
        "backup directory should not be created when the live db is absent"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn daily_backup_cycle_waits_for_utc_day_rollover_before_snapshot() {
    let (settings, root) = persistence_settings("daily-backup-rollover");
    let day_zero = 1_735_689_600;
    let day_one = day_zero + 86_400;
    bootstrap_sqlite(settings.db_path(), "first-run").expect("first bootstrap should succeed");

    let mut state = DailyBackupState {
        last_backup_day: Some(utc_day_from_unix_seconds(day_zero)),
    };
    let same_day = run_daily_backup_cycle(&settings, &mut state, day_zero + 3_600)
        .expect("same-day daily backup cycle should succeed");
    assert!(!same_day.triggered);
    assert!(same_day.backup_path.is_none());

    bootstrap_sqlite(settings.db_path(), "second-run").expect("second bootstrap should succeed");

    let next_day = run_daily_backup_cycle(&settings, &mut state, day_one)
        .expect("next-day daily backup cycle should succeed");
    assert!(next_day.triggered);
    let backup_path = next_day
        .backup_path
        .clone()
        .expect("next-day daily backup should create a backup path");
    assert!(backup_path.exists());

    let backup_connection = Connection::open(&backup_path).expect("backup db should open");
    let backup_bootstrap_events: i64 = backup_connection
        .query_row("SELECT COUNT(*) FROM bootstrap_events", [], |row| {
            row.get(0)
        })
        .expect("backup bootstrap count should be readable");
    assert_eq!(backup_bootstrap_events, 2);
    assert_eq!(
        state.last_backup_day,
        Some(utc_day_from_unix_seconds(day_one)),
        "daily backup state should advance to the new utc day"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn daily_backup_cycle_prunes_old_backups_when_triggered() {
    let (settings, root) = persistence_settings("daily-backup-prune");
    let day_zero = 1_735_689_600;
    let day_one = day_zero + 86_400;
    let backup_root = resolve_persistence_relative_path(&settings, STARTUP_BACKUP_DIR);
    fs::create_dir_all(&backup_root).expect("backup root should be creatable");
    bootstrap_sqlite(settings.db_path(), "first-run").expect("bootstrap should succeed");

    for stamp in [
        "20241224-000000",
        "20241225-000000",
        "20241226-000000",
        "20241227-000000",
        "20241228-000000",
        "20241229-000000",
        "20241230-000000",
        "20241231-000000",
    ] {
        fs::write(
            backup_root.join(format!(
                "{STARTUP_BACKUP_FILE_PREFIX}{stamp}{STARTUP_BACKUP_FILE_SUFFIX}",
            )),
            b"snapshot",
        )
        .expect("backup fixture should be writable");
    }

    let mut state = DailyBackupState {
        last_backup_day: Some(utc_day_from_unix_seconds(day_zero)),
    };
    let run = run_daily_backup_cycle(&settings, &mut state, day_one)
        .expect("daily backup cycle should succeed on new day");

    assert!(run.triggered);
    assert_eq!(run.pruned_paths.len(), 2);
    let mut remaining = collect_files_with_suffix(&backup_root, STARTUP_BACKUP_FILE_SUFFIX)
        .expect("remaining backups should be enumerable")
        .into_iter()
        .filter_map(|path| {
            let name = path.file_name()?.to_str()?;
            if name.starts_with(STARTUP_BACKUP_FILE_PREFIX)
                && name.ends_with(STARTUP_BACKUP_FILE_SUFFIX)
            {
                Some(name.to_string())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    remaining.sort();
    assert_eq!(remaining.len(), STARTUP_BACKUP_KEEP_COUNT);
    assert!(
        run.backup_path.as_ref().is_some_and(|path| path.exists()),
        "daily backup cycle should write the new backup before pruning"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn persistence_uuidv7_time_fields_and_payload_version_roundtrip() {
    let db_path = database_path("payload-roundtrip");
    bootstrap_sqlite(&db_path, "uuidv7-run").expect("bootstrap should succeed");

    let connection = Connection::open(&db_path).expect("db should open");
    let (event_id, schema_version, game_tick, wall_clock, last_updated_wall, payload_json): (
        String,
        i32,
        i64,
        i64,
        i64,
        String,
    ) = connection
        .query_row(
            "
            SELECT event_id, schema_version, game_tick, wall_clock, last_updated_wall, payload_json
            FROM bootstrap_events
            LIMIT 1
            ",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .expect("bootstrap row should exist");

    let uuid = Uuid::parse_str(&event_id).expect("event_id should be a valid UUID");
    assert_eq!(uuid.get_version_num(), 7);
    assert_eq!(schema_version, CURRENT_SCHEMA_VERSION);
    assert_eq!(game_tick, 0);
    assert!(wall_clock > 0);
    assert_eq!(last_updated_wall, wall_clock);

    let payload: BootstrapPayload =
        serde_json::from_str(&payload_json).expect("payload should deserialize");
    assert_eq!(payload.schema_version, CURRENT_SCHEMA_VERSION);
    assert_eq!(payload.id, event_id);
    assert_eq!(payload.note, "sqlite bootstrap ready");
}

#[test]
fn task3_migrations_create_life_and_deceased_tables() {
    let db_path = database_path("task3-migrations");
    bootstrap_sqlite(&db_path, "task3-migrations").expect("bootstrap should succeed");

    let connection = Connection::open(&db_path).expect("db should open");
    for table_name in [
        "life_records",
        "life_events",
        "death_registry",
        "lifespan_events",
        "deceased_snapshots",
    ] {
        let exists: Option<String> = connection
            .query_row(
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name = ?1",
                params![table_name],
                |row| row.get(0),
            )
            .optional()
            .expect("sqlite_master query should succeed");
        assert_eq!(exists.as_deref(), Some(table_name));
    }
}

#[test]
fn task6_migrations_create_tribulations_active_table() {
    let db_path = database_path("task6-tribulations-active");
    bootstrap_sqlite(&db_path, "task6-tribulations-active").expect("bootstrap should succeed");

    let connection = Connection::open(&db_path).expect("db should open");
    let exists: Option<String> = connection
        .query_row(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'tribulations_active'",
            [],
            |row| row.get(0),
        )
        .optional()
        .expect("sqlite_master tribulations_active query should succeed");
    assert_eq!(exists.as_deref(), Some("tribulations_active"));
}

#[test]
fn v21_migration_backfills_partial_juebi_epicenter_columns() {
    let db_path = database_path("v21-partial-juebi-epicenter");
    fs::create_dir_all(db_path.parent().expect("db path should have parent"))
        .expect("temp db parent should be created");
    let mut connection = Connection::open(&db_path).expect("db should open");
    connection
        .execute_batch(
            "
            CREATE TABLE tribulations_active (
                char_id TEXT PRIMARY KEY,
                kind TEXT NOT NULL DEFAULT 'du_xu',
                source TEXT NOT NULL DEFAULT '',
                wave_current INTEGER NOT NULL CHECK (wave_current >= 0),
                waves_total INTEGER NOT NULL CHECK (waves_total > 0),
                started_tick INTEGER NOT NULL CHECK (started_tick >= 0),
                epicenter_x REAL NOT NULL DEFAULT 0.0,
                intensity REAL NOT NULL DEFAULT 0.0,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            INSERT INTO tribulations_active (
                char_id,
                kind,
                source,
                wave_current,
                waves_total,
                started_tick,
                epicenter_x,
                intensity,
                schema_version,
                last_updated_wall
            ) VALUES (
                'offline:Azure',
                'jue_bi',
                'void_action_explode_zone',
                2,
                3,
                120,
                12.0,
                1.6,
                1,
                1
            );
            PRAGMA user_version = 20;
            ",
        )
        .expect("partial v20 tribulation table should be created");

    apply_migrations(&mut connection).expect("partial v20 table should migrate to v21");

    for column in ["epicenter_x", "epicenter_y", "epicenter_z"] {
        let count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('tribulations_active') WHERE name = ?1",
                params![column],
                |row| row.get(0),
            )
            .expect("tribulations_active column query should succeed");
        assert_eq!(count, 1, "{column} should exist after v21 migration");
    }
    let active = load_active_tribulation_from_connection(&connection, "offline:Azure")
        .expect("active tribulation query should succeed")
        .expect("legacy active row should survive migration");
    assert_eq!(active.kind, "jue_bi");
    assert_eq!(active.origin_dimension, None);
    assert_eq!(active.epicenter, [12.0, 64.0, 0.0]);
    assert_eq!(active.intensity, 1.6);

    let _ = fs::remove_dir_all(db_path.parent().expect("db path should have parent"));
}

#[test]
fn task7_migrations_create_ascension_quota_table() {
    let db_path = database_path("task7-ascension-quota");
    bootstrap_sqlite(&db_path, "task7-ascension-quota").expect("bootstrap should succeed");

    let connection = Connection::open(&db_path).expect("db should open");
    let exists: Option<String> = connection
        .query_row(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'ascension_quota'",
            [],
            |row| row.get(0),
        )
        .optional()
        .expect("sqlite_master ascension_quota query should succeed");
    assert_eq!(exists.as_deref(), Some("ascension_quota"));
}

#[test]
fn task8_migrations_create_zones_runtime_table() {
    let db_path = database_path("task8-zones-runtime");
    bootstrap_sqlite(&db_path, "task8-zones-runtime").expect("bootstrap should succeed");

    let connection = Connection::open(&db_path).expect("db should open");
    let exists: Option<String> = connection
        .query_row(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'zones_runtime'",
            [],
            |row| row.get(0),
        )
        .optional()
        .expect("sqlite_master zones_runtime query should succeed");
    assert_eq!(exists.as_deref(), Some("zones_runtime"));
}

#[test]
fn task9_migrations_create_zone_overlays_table() {
    let db_path = database_path("task9-zone-overlays");
    bootstrap_sqlite(&db_path, "task9-zone-overlays").expect("bootstrap should succeed");

    let connection = Connection::open(&db_path).expect("db should open");
    let exists: Option<String> = connection
        .query_row(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'zone_overlays'",
            [],
            |row| row.get(0),
        )
        .optional()
        .expect("sqlite_master zone_overlays query should succeed");
    assert_eq!(exists.as_deref(), Some("zone_overlays"));
}

#[test]
fn task10_migration_adds_zone_overlays_payload_version_column() {
    let db_path = database_path("task10-zone-overlays-payload-version");
    bootstrap_sqlite(&db_path, "task10-zone-overlays-payload-version")
        .expect("bootstrap should succeed");

    let connection = Connection::open(&db_path).expect("db should open");
    let mut statement = connection
        .prepare("PRAGMA table_info(zone_overlays)")
        .expect("table_info should prepare");
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .expect("table_info should query")
        .collect::<Result<Vec<_>, _>>()
        .expect("table_info rows should collect");
    assert!(
        columns.iter().any(|column| column == "payload_version"),
        "zone_overlays should include payload_version after migration"
    );
}

#[test]
fn task11_migration_creates_agent_append_only_tables() {
    let db_path = database_path("task11-agent-append-only");
    bootstrap_sqlite(&db_path, "task11-agent-append-only").expect("bootstrap should succeed");

    let connection = Connection::open(&db_path).expect("db should open");
    for table_name in ["agent_eras", "agent_decisions"] {
        let exists: Option<String> = connection
            .query_row(
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name = ?1",
                params![table_name],
                |row| row.get(0),
            )
            .optional()
            .expect("sqlite_master query should succeed");
        assert_eq!(exists.as_deref(), Some(table_name));
    }
}

#[test]
fn task12_migration_creates_player_lifespan_table() {
    let db_path = database_path("task12-player-lifespan");
    bootstrap_sqlite(&db_path, "task12-player-lifespan").expect("bootstrap should succeed");

    let connection = Connection::open(&db_path).expect("db should open");
    let exists: Option<String> = connection
        .query_row(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'player_lifespan'",
            [],
            |row| row.get(0),
        )
        .optional()
        .expect("sqlite_master player_lifespan query should succeed");
    assert_eq!(exists.as_deref(), Some("player_lifespan"));

    let mut statement = connection
        .prepare("PRAGMA table_info(player_lifespan)")
        .expect("player_lifespan table_info should prepare");
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .expect("player_lifespan table_info should query")
        .collect::<Result<Vec<_>, _>>()
        .expect("player_lifespan columns should collect");
    for column in [
        "username",
        "born_at_tick",
        "years_lived",
        "cap_by_realm",
        "offline_pause_wall",
        "in_coffin",
        "schema_version",
        "last_updated_wall",
    ] {
        assert!(
            columns.iter().any(|candidate| candidate == column),
            "player_lifespan should include {column}"
        );
    }
}

#[test]
fn v23_migration_adds_in_coffin_to_legacy_player_lifespan_table() {
    let db_path = database_path("v23-player-lifespan-in-coffin");
    fs::create_dir_all(db_path.parent().expect("db path should have parent"))
        .expect("temp db parent should be created");
    let mut connection = Connection::open(&db_path).expect("db should open");
    connection
        .execute_batch(
            "
            CREATE TABLE player_lifespan (
                username TEXT PRIMARY KEY,
                born_at_tick INTEGER NOT NULL CHECK (born_at_tick >= 0),
                years_lived REAL NOT NULL CHECK (years_lived >= 0),
                cap_by_realm INTEGER NOT NULL CHECK (cap_by_realm > 0),
                offline_pause_wall INTEGER NOT NULL CHECK (offline_pause_wall >= 0),
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            INSERT INTO player_lifespan (
                username,
                born_at_tick,
                years_lived,
                cap_by_realm,
                offline_pause_wall,
                schema_version,
                last_updated_wall
            ) VALUES ('Azure', 0, 3.5, 80, 0, 1, 0);
            PRAGMA user_version = 22;
            ",
        )
        .expect("legacy player_lifespan fixture should create");

    apply_migrations(&mut connection).expect("v23 migration should succeed");

    let mut statement = connection
        .prepare("PRAGMA table_info(player_lifespan)")
        .expect("player_lifespan table_info should prepare");
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .expect("player_lifespan table_info should query")
        .collect::<Result<Vec<_>, _>>()
        .expect("player_lifespan columns should collect");
    assert!(
        columns.iter().any(|column| column == "in_coffin"),
        "player_lifespan should include in_coffin after v23 migration"
    );
    let in_coffin: i64 = connection
        .query_row(
            "SELECT in_coffin FROM player_lifespan WHERE username = 'Azure'",
            [],
            |row| row.get(0),
        )
        .expect("legacy row should get default in_coffin");
    assert_eq!(in_coffin, 0);
}

#[test]
fn v24_migration_adds_spirit_treasure_tables() {
    let db_path = database_path("v24-spirit-treasure-tables");
    fs::create_dir_all(db_path.parent().expect("db path should have parent"))
        .expect("temp db parent should be created");
    let mut connection = Connection::open(&db_path).expect("db should open");
    connection
        .execute_batch("PRAGMA user_version = 23;")
        .expect("legacy v23 fixture should create");

    apply_migrations(&mut connection).expect("v24 migration should succeed");

    let user_version: i32 = connection
        .query_row("PRAGMA user_version;", [], |row| row.get(0))
        .expect("user_version should be readable");
    assert_eq!(user_version, CURRENT_USER_VERSION);

    for table in ["spirit_treasure_world", "spirit_treasure_dialogue_log"] {
        let exists: Option<String> = connection
            .query_row(
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name = ?1",
                params![table],
                |row| row.get(0),
            )
            .optional()
            .expect("sqlite_master query should succeed");
        assert_eq!(exists.as_deref(), Some(table));
    }

    let mut statement = connection
        .prepare("PRAGMA table_info(spirit_treasure_world)")
        .expect("spirit_treasure_world table_info should prepare");
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .expect("spirit_treasure_world table_info should query")
        .collect::<Result<Vec<_>, _>>()
        .expect("spirit_treasure_world columns should collect");
    for column in [
        "template_id",
        "instance_id",
        "holder_kind",
        "affinity",
        "dialogue_count",
        "sleeping",
        "spawned_at_tick",
    ] {
        assert!(
            columns.iter().any(|candidate| candidate == column),
            "spirit_treasure_world should include {column}"
        );
    }
}

#[test]
fn v25_migration_adds_player_known_techniques_table() {
    let db_path = database_path("v25-player-known-techniques");
    fs::create_dir_all(db_path.parent().expect("db path should have parent"))
        .expect("temp db parent should be created");
    let mut connection = Connection::open(&db_path).expect("db should open");
    connection
        .execute_batch("PRAGMA user_version = 24;")
        .expect("legacy v24 fixture should create");

    apply_migrations(&mut connection).expect("v25 migration should succeed");

    let user_version: i32 = connection
        .query_row("PRAGMA user_version;", [], |row| row.get(0))
        .expect("user_version should be readable");
    assert_eq!(
        user_version, CURRENT_USER_VERSION,
        "expected user_version to advance to CURRENT_USER_VERSION ({CURRENT_USER_VERSION}) because v25 migration succeeded, actual {user_version}"
    );

    let exists: Option<String> = connection
        .query_row(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'player_known_techniques'",
            [],
            |row| row.get(0),
        )
        .optional()
        .expect("sqlite_master query should succeed");
    assert_eq!(
        exists.as_deref(),
        Some("player_known_techniques"),
        "expected sqlite_master to include player_known_techniques because v25 migration should create it, actual {exists:?}"
    );

    let mut statement = connection
        .prepare("PRAGMA table_info(player_known_techniques)")
        .expect("player_known_techniques table_info should prepare");
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .expect("player_known_techniques table_info should query")
        .collect::<Result<Vec<_>, _>>()
        .expect("player_known_techniques columns should collect");
    for column in [
        "username",
        "known_techniques_json",
        "schema_version",
        "last_updated_wall",
    ] {
        assert!(
            columns.iter().any(|candidate| candidate == column),
            "player_known_techniques should include {column}"
        );
    }

    let primary_keys = connection
        .prepare("PRAGMA table_info(player_known_techniques)")
        .expect("player_known_techniques primary key table_info should prepare")
        .query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, i32>(5)?))
        })
        .expect("player_known_techniques primary key table_info should query")
        .filter_map(|row| {
            let (name, pk) = row.expect("player_known_techniques primary key row should decode");
            (pk > 0).then_some(name)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        primary_keys,
        vec!["username".to_string()],
        "expected username to be the only primary key because player_known_techniques is keyed by player, actual {primary_keys:?}"
    );
}

#[test]
fn v25_migration_rejects_partial_player_known_techniques_schema() {
    let db_path = database_path("v25-partial-player-known-techniques");
    fs::create_dir_all(db_path.parent().expect("db path should have parent"))
        .expect("temp db parent should be created");
    let mut connection = Connection::open(&db_path).expect("db should open");
    connection
        .execute_batch(
            "
            CREATE TABLE player_known_techniques (
                username TEXT PRIMARY KEY
            );
            PRAGMA user_version = 24;
            ",
        )
        .expect("partial v24 fixture should be created");

    let error = apply_migrations(&mut connection)
        .expect_err("v25 migration should reject partial player_known_techniques schema");
    assert!(
        error
            .to_string()
            .contains("player_known_techniques column"),
        "expected partial schema error to name player_known_techniques column, actual error={error}"
    );

    let user_version: i32 = connection
        .query_row("PRAGMA user_version;", [], |row| row.get(0))
        .expect("user_version should be readable");
    assert_eq!(
        user_version, 24,
        "expected failed v25 migration to leave user_version at 24, actual {user_version}"
    );
}

#[test]
fn v25_migration_rejects_player_known_techniques_without_username_primary_key() {
    let db_path = database_path("v25-wrong-player-known-techniques-pk");
    fs::create_dir_all(db_path.parent().expect("db path should have parent"))
        .expect("temp db parent should be created");
    let mut connection = Connection::open(&db_path).expect("db should open");
    connection
        .execute_batch(
            "
            CREATE TABLE player_known_techniques (
                username TEXT NOT NULL,
                known_techniques_json TEXT NOT NULL,
                schema_version INTEGER NOT NULL,
                last_updated_wall INTEGER NOT NULL
            );
            PRAGMA user_version = 24;
            ",
        )
        .expect("wrong primary key v24 fixture should be created");

    let error = apply_migrations(&mut connection).expect_err(
        "v25 migration should reject player_known_techniques without username primary key",
    );
    assert!(
        error.to_string().contains("primary key mismatch"),
        "expected primary key mismatch error, actual error={error}"
    );

    let user_version: i32 = connection
        .query_row("PRAGMA user_version;", [], |row| row.get(0))
        .expect("user_version should be readable");
    assert_eq!(
        user_version, 24,
        "expected failed v25 migration to leave user_version at 24, actual {user_version}"
    );
}

#[test]
fn task13_migration_creates_player_shrine_table() {
    let db_path = database_path("task13-player-shrine");
    bootstrap_sqlite(&db_path, "task13-player-shrine").expect("bootstrap should succeed");

    let connection = Connection::open(&db_path).expect("db should open");
    let exists: Option<String> = connection
        .query_row(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'player_shrine'",
            [],
            |row| row.get(0),
        )
        .optional()
        .expect("sqlite_master player_shrine query should succeed");
    assert_eq!(exists.as_deref(), Some("player_shrine"));

    let mut statement = connection
        .prepare("PRAGMA table_info(player_shrine)")
        .expect("player_shrine table_info should prepare");
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .expect("table_info should query")
        .collect::<Result<Vec<_>, _>>()
        .expect("player_shrine columns should collect");
    for column in [
        "username",
        "anchor_x",
        "anchor_y",
        "anchor_z",
        "schema_version",
        "last_updated_wall",
    ] {
        assert!(
            columns.iter().any(|candidate| candidate == column),
            "player_shrine should include {column}"
        );
    }
}

#[test]
fn bootstrap_migrates_v9_zone_overlays_and_preserves_existing_rows() {
    let db_path = database_path("zone-overlays-v9-migration-drill");
    bootstrap_sqlite(&db_path, "zone-overlays-v9-baseline")
        .expect("baseline bootstrap should succeed");

    {
        let connection = Connection::open(&db_path).expect("legacy db should open");
        connection
            .execute_batch(
                "
                DROP TABLE zone_overlays;
                CREATE TABLE zone_overlays (
                    zone_id TEXT NOT NULL,
                    overlay_kind TEXT NOT NULL,
                    payload_json TEXT NOT NULL,
                    since_wall INTEGER NOT NULL,
                    schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                    last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0),
                    PRIMARY KEY (zone_id, overlay_kind, since_wall),
                    CHECK (since_wall >= 0)
                );
                PRAGMA user_version = 9;
                ",
            )
            .expect("legacy zone_overlays schema should be creatable");
        connection
            .execute(
                "
                INSERT INTO zone_overlays (
                    zone_id,
                    overlay_kind,
                    payload_json,
                    since_wall,
                    schema_version,
                    last_updated_wall
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                ",
                params![
                    DEFAULT_SPAWN_ZONE_NAME,
                    "collapsed",
                    serde_json::json!({"danger_level": 4}).to_string(),
                    77_i64,
                    CURRENT_SCHEMA_VERSION,
                    88_i64,
                ],
            )
            .expect("legacy zone_overlays row should insert");
    }

    bootstrap_sqlite(&db_path, "zone-overlays-v9-migration-drill")
        .expect("bootstrap migration should succeed");

    let connection = Connection::open(&db_path).expect("migrated db should open");
    let user_version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("user_version should be readable");
    assert_eq!(user_version as i32, CURRENT_USER_VERSION);

    let mut statement = connection
        .prepare("PRAGMA table_info(zone_overlays)")
        .expect("table_info should prepare");
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .expect("table_info should query")
        .collect::<Result<Vec<_>, _>>()
        .expect("table_info rows should collect");
    assert!(
        columns.iter().any(|column| column == "payload_version"),
        "migrated zone_overlays should include payload_version"
    );

    let migrated_row: ZoneOverlayRecord = connection
        .query_row(
            "
            SELECT zone_id, overlay_kind, payload_json, payload_version, since_wall
            FROM zone_overlays
            WHERE zone_id = ?1 AND overlay_kind = ?2
            ",
            params![DEFAULT_SPAWN_ZONE_NAME, "collapsed"],
            |row| {
                Ok(ZoneOverlayRecord {
                    zone_id: row.get(0)?,
                    overlay_kind: row.get(1)?,
                    payload_json: row.get(2)?,
                    payload_version: row.get(3)?,
                    since_wall: row.get(4)?,
                })
            },
        )
        .expect("migrated zone_overlays row should exist");
    assert_eq!(
        migrated_row,
        ZoneOverlayRecord {
            zone_id: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            overlay_kind: "collapsed".to_string(),
            payload_json: serde_json::json!({"danger_level": 4}).to_string(),
            payload_version: 1,
            since_wall: 77,
        }
    );

    let _ = fs::remove_dir_all(
        db_path
            .parent()
            .expect("migration drill db path should still have parent directory"),
    );
}

#[test]
fn agent_world_model_snapshot_roundtrips_through_sqlite() {
    let (settings, root) = persistence_settings("agent-world-model-roundtrip");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    // fix/world-model-schema-drift：neg_domain 三字段是本次修复新增的
    // persistence 字段，roundtrip 必须带上非空数据，否则回归会被
    // #[serde(default)] 的"缺字段容忍"悄悄吞掉而测不出来。
    let snapshot = AgentWorldModelSnapshotRecord {
        current_era: Some(serde_json::json!({
            "name": "blood_moon",
            "since_tick": 4096,
            "global_effect": "qi tides run violent"
        })),
        zone_history: BTreeMap::from([(
            "spawn".to_string(),
            vec![serde_json::json!({
                "name": "spawn",
                "spirit_qi": 0.35,
                "danger_level": 2,
                "active_events": ["blood_moon"],
                "player_count": 1
            })],
        )]),
        last_decisions: BTreeMap::from([(
            "era".to_string(),
            AgentWorldModelDecisionRecord {
                commands: Vec::new(),
                narrations: Vec::new(),
                reasoning: "era shift persisted for recovery".to_string(),
            },
        )]),
        player_first_seen_tick: BTreeMap::from([("Azure".to_string(), 128_i64)]),
        neg_domain_pending_tribulations: BTreeMap::from([(
            "Azure".to_string(),
            AgentWorldModelNegDomainPendingTribulationRecord {
                player_uuid: "Azure".to_string(),
                player_name: "Azure".to_string(),
                zone: "rift_valley".to_string(),
                entered_at_tick: 4_000,
                last_suppressed_tick: 4_100,
                reason: "negative_domain_tribulation_exempt".to_string(),
            },
        )]),
        neg_domain_escape_telemetry: AgentWorldModelNegDomainEscapeTelemetryRecord {
            escape_entry_count: 6,
            post_escape_realm_drop_count: 2,
            successful_tribulation_avoidance_count: 4,
            active_escape_session_count: 1,
            post_escape_realm_drop_rate: 1.0 / 3.0,
        },
        neg_domain_escape_sessions: BTreeMap::from([(
            "Azure".to_string(),
            AgentWorldModelNegDomainEscapeSessionRecord {
                player_uuid: "Azure".to_string(),
                player_name: "Azure".to_string(),
                zone: "rift_valley".to_string(),
                entered_at_tick: 4_000,
                entry_realm_rank: 2.5,
            },
        )]),
        last_tick: Some(4_200),
        last_state_ts: Some(1_704_067_200),
    };

    persist_agent_world_model_snapshot(&settings, &snapshot)
        .expect("agent world model snapshot should persist");
    let loaded = load_agent_world_model_snapshot(&settings)
        .expect("agent world model snapshot should load")
        .expect("agent world model snapshot should exist");

    assert_eq!(
        loaded, snapshot,
        "full snapshot (incl. neg_domain fields) should roundtrip byte-for-byte through sqlite"
    );

    let pending = loaded
        .neg_domain_pending_tribulations
        .get("Azure")
        .expect("neg_domain_pending_tribulations should roundtrip through sqlite");
    assert_eq!(pending.zone, "rift_valley");
    assert_eq!(pending.reason, "negative_domain_tribulation_exempt");
    assert_eq!(loaded.neg_domain_escape_telemetry.escape_entry_count, 6);
    assert_eq!(
        loaded
            .neg_domain_escape_sessions
            .get("Azure")
            .map(|s| s.entry_realm_rank),
        Some(2.5)
    );

    let connection = Connection::open(settings.db_path()).expect("sqlite db should open");
    let schema_version: i32 = connection
        .query_row(
            "SELECT schema_version FROM agent_world_model WHERE row_id = ?1",
            params![AGENT_WORLD_MODEL_ROW_ID],
            |row| row.get(0),
        )
        .expect("agent_world_model schema_version should exist");
    assert_eq!(schema_version, CURRENT_SCHEMA_VERSION);

    let _ = fs::remove_dir_all(root);
}

// fix/world-model-schema-drift：老 SQLite blob（升级前写入，没有 neg_domain
// 三字段的 JSON）必须仍能 load 成功，#[serde(default)] 负责补零值/空表。
#[test]
fn agent_world_model_snapshot_load_tolerates_legacy_json_missing_neg_domain_fields() {
    let (settings, root) = persistence_settings("agent-world-model-legacy-missing-fields");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let legacy_snapshot_json = serde_json::json!({
        "current_era": null,
        "zone_history": {},
        "last_decisions": {},
        "player_first_seen_tick": {},
        "last_tick": 100,
        "last_state_ts": 1_700_000_000
    })
    .to_string();

    let mut connection = open_persistence_connection(&settings).expect("db should open");
    let transaction = connection.transaction().expect("transaction should open");
    transaction
        .execute(
            "
            INSERT INTO agent_world_model (row_id, snapshot_json, schema_version, last_updated_wall)
            VALUES (?1, ?2, ?3, ?4)
            ",
            params![
                AGENT_WORLD_MODEL_ROW_ID,
                legacy_snapshot_json,
                CURRENT_SCHEMA_VERSION,
                1_700_000_000_i64
            ],
        )
        .expect("legacy snapshot_json row should insert");
    transaction.commit().expect("transaction should commit");

    let loaded = load_agent_world_model_snapshot(&settings)
        .expect("legacy snapshot missing neg_domain fields should still load")
        .expect("legacy snapshot row should exist");

    assert!(loaded.neg_domain_pending_tribulations.is_empty());
    assert_eq!(
        loaded.neg_domain_escape_telemetry,
        AgentWorldModelNegDomainEscapeTelemetryRecord::default()
    );
    assert!(loaded.neg_domain_escape_sessions.is_empty());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn agent_authority_write_persists_snapshot_and_append_only_rows() {
    let (settings, root) = persistence_settings("agent-authority-append-only");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let first_snapshot = AgentWorldModelSnapshotRecord {
        current_era: Some(serde_json::json!({
            "name": "blood_moon",
            "since_tick": 4096,
            "global_effect": "qi tides run violent"
        })),
        zone_history: BTreeMap::from([(
            "spawn".to_string(),
            vec![serde_json::json!({
                "name": "spawn",
                "spirit_qi": 0.35,
                "danger_level": 2,
                "active_events": ["blood_moon"],
                "player_count": 1
            })],
        )]),
        last_decisions: BTreeMap::from([(
            "era".to_string(),
            AgentWorldModelDecisionRecord {
                commands: Vec::new(),
                narrations: Vec::new(),
                reasoning: "era shift persisted for recovery".to_string(),
            },
        )]),
        player_first_seen_tick: BTreeMap::from([("Azure".to_string(), 128_i64)]),
        last_tick: Some(4_200),
        last_state_ts: Some(1_704_067_200),
        ..Default::default()
    };

    persist_agent_world_model_authority_state(&settings, "wm-append-1", "arbiter", &first_snapshot)
        .expect("first authority write should succeed");

    let second_snapshot = AgentWorldModelSnapshotRecord {
        current_era: Some(serde_json::json!({
            "name": "ashen_sky",
            "since_tick": 5000,
            "global_effect": "embers drift across the realm"
        })),
        zone_history: first_snapshot.zone_history.clone(),
        last_decisions: BTreeMap::from([
            (
                "era".to_string(),
                AgentWorldModelDecisionRecord {
                    commands: Vec::new(),
                    narrations: Vec::new(),
                    reasoning: "era advanced under persistent authority".to_string(),
                },
            ),
            (
                "calamity".to_string(),
                AgentWorldModelDecisionRecord {
                    commands: vec![AgentWorldModelCommandRecord {
                        command_type: "spawn_event".to_string(),
                        target: "spawn".to_string(),
                        params: serde_json::Map::new(),
                    }],
                    narrations: vec![AgentWorldModelNarrationRecord {
                        scope: "broadcast".to_string(),
                        target: None,
                        text: "灾潮将起".to_string(),
                        style: "era_decree".to_string(),
                    }],
                    reasoning: "calamity prepared one command and one narration".to_string(),
                },
            ),
        ]),
        player_first_seen_tick: BTreeMap::from([("Azure".to_string(), 128_i64)]),
        last_tick: Some(5_100),
        last_state_ts: Some(1_704_067_500),
        ..Default::default()
    };

    persist_agent_world_model_authority_state(
        &settings,
        "wm-append-2",
        "calamity",
        &second_snapshot,
    )
    .expect("second authority write should succeed");

    let loaded = load_agent_world_model_snapshot(&settings)
        .expect("authority snapshot should load")
        .expect("authority snapshot should exist");
    assert_eq!(loaded, second_snapshot);

    let eras = load_agent_eras(&settings).expect("agent eras should load");
    assert_eq!(eras.len(), 2);
    assert_eq!(eras[0].envelope_id, "wm-append-1");
    assert_eq!(eras[0].source, "arbiter");
    assert_eq!(eras[0].era_name, "blood_moon");
    assert_eq!(eras[1].envelope_id, "wm-append-2");
    assert_eq!(eras[1].source, "calamity");
    assert_eq!(eras[1].era_name, "ashen_sky");

    let decisions = load_agent_decisions(&settings).expect("agent decisions should load");
    assert_eq!(decisions.len(), 3);
    assert_eq!(decisions[0].envelope_id, "wm-append-1");
    assert_eq!(decisions[0].agent_name, "era");
    assert_eq!(decisions[1].envelope_id, "wm-append-2");
    assert_eq!(decisions[1].agent_name, "calamity");
    assert_eq!(decisions[1].command_count, 1);
    assert_eq!(decisions[1].narration_count, 1);
    assert_eq!(decisions[2].envelope_id, "wm-append-2");
    assert_eq!(decisions[2].agent_name, "era");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn agent_authority_write_prunes_append_only_rows_older_than_180_days() {
    let (settings, root) = persistence_settings("agent-authority-retention");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let stale_wall = 1_700_000_000;
    let prune_now = stale_wall + AGENT_WORLD_MODEL_APPEND_ONLY_RETENTION_SECS + 60;
    let snapshot = AgentWorldModelSnapshotRecord {
        current_era: Some(serde_json::json!({
            "name": "blood_moon",
            "since_tick": 4096,
            "global_effect": "qi tides run violent"
        })),
        zone_history: BTreeMap::new(),
        last_decisions: BTreeMap::from([(
            "era".to_string(),
            AgentWorldModelDecisionRecord {
                commands: Vec::new(),
                narrations: Vec::new(),
                reasoning: "retention drill".to_string(),
            },
        )]),
        player_first_seen_tick: BTreeMap::new(),
        last_tick: Some(4_200),
        last_state_ts: Some(1_704_067_200),
        ..Default::default()
    };

    persist_agent_world_model_authority_state(&settings, "wm-old", "arbiter", &snapshot)
        .expect("first authority write should succeed");

    let mut connection = open_persistence_connection(&settings).expect("db should open");
    let transaction = connection.transaction().expect("transaction should open");
    transaction
        .execute(
            "UPDATE agent_eras SET observed_at_wall = ?1 WHERE envelope_id = ?2",
            params![stale_wall, "wm-old"],
        )
        .expect("test should age era row");
    transaction
        .execute(
            "UPDATE agent_decisions SET observed_at_wall = ?1 WHERE envelope_id = ?2",
            params![stale_wall, "wm-old"],
        )
        .expect("test should age decision row");
    prune_agent_world_model_append_only(&transaction, prune_now)
        .expect("retention prune should succeed");
    transaction
        .commit()
        .expect("retention transaction should commit");

    let eras = load_agent_eras(&settings).expect("agent eras should load");
    let decisions = load_agent_decisions(&settings).expect("agent decisions should load");
    assert!(eras.is_empty(), "stale agent eras should be pruned");
    assert!(
        decisions.is_empty(),
        "stale agent decisions should be pruned"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn active_tribulation_roundtrip_and_delete() {
    let (settings, root) = persistence_settings("tribulation-roundtrip");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let record = ActiveTribulationRecord {
        char_id: "offline:Azure".to_string(),
        kind: "jue_bi".to_string(),
        source: "void_action_explode_zone".to_string(),
        origin_dimension: Some("minecraft:overworld".to_string()),
        wave_current: 2,
        waves_total: 5,
        started_tick: 1440,
        epicenter: [12.0, 66.0, -3.0],
        intensity: 1.6,
    };
    persist_active_tribulation(&settings, &record).expect("active tribulation should persist");

    let loaded = load_active_tribulation(&settings, record.char_id.as_str())
        .expect("active tribulation query should succeed")
        .expect("active tribulation row should exist");
    assert_eq!(loaded, record);

    delete_active_tribulation(&settings, record.char_id.as_str())
        .expect("active tribulation delete should succeed");
    let deleted = load_active_tribulation(&settings, record.char_id.as_str())
        .expect("post-delete active tribulation query should succeed");
    assert!(deleted.is_none());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn ascension_quota_defaults_to_zero_and_roundtrips_updates() {
    let (settings, root) = persistence_settings("ascension-quota-roundtrip");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let initial = load_ascension_quota(&settings).expect("quota load should succeed");
    assert_eq!(initial.occupied_slots, 0);

    let wall_clock = current_unix_seconds();
    let mut connection = open_persistence_connection(&settings).expect("db should open");
    let transaction = connection.transaction().expect("transaction should open");
    upsert_ascension_quota(
        &transaction,
        &AscensionQuotaRecord { occupied_slots: 3 },
        wall_clock,
    )
    .expect("quota upsert should succeed");
    transaction.commit().expect("transaction should commit");

    let updated = load_ascension_quota(&settings).expect("quota reload should succeed");
    assert_eq!(updated.occupied_slots, 3);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn complete_tribulation_ascension_clears_active_row_and_increments_quota() {
    let (settings, root) = persistence_settings("ascension-quota-complete-tribulation");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let record = ActiveTribulationRecord {
        char_id: "offline:Azure".to_string(),
        kind: "du_xu".to_string(),
        source: String::new(),
        origin_dimension: Some("minecraft:overworld".to_string()),
        wave_current: 4,
        waves_total: 5,
        started_tick: 2880,
        epicenter: [0.0, 64.0, 0.0],
        intensity: 0.0,
    };
    persist_active_tribulation(&settings, &record).expect("active tribulation should persist");

    let quota = complete_tribulation_ascension(&settings, record.char_id.as_str())
        .expect("tribulation completion should succeed");
    assert_eq!(quota.occupied_slots, 1);

    let loaded_quota = load_ascension_quota(&settings).expect("quota load should succeed");
    assert_eq!(loaded_quota.occupied_slots, 1);

    let active = load_active_tribulation(&settings, record.char_id.as_str())
        .expect("active tribulation query should succeed");
    assert!(active.is_none());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn complete_tribulation_ascension_without_active_row_is_idempotent_for_quota() {
    let (settings, root) = persistence_settings("ascension-quota-complete-no-active");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let quota = complete_tribulation_ascension(&settings, "offline:Azure")
        .expect("missing active row completion should stay idempotent");
    assert_eq!(quota.occupied_slots, 0);

    let loaded_quota = load_ascension_quota(&settings).expect("quota load should succeed");
    assert_eq!(loaded_quota.occupied_slots, 0);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn complete_independent_juebi_clears_active_without_incrementing_quota() {
    let (settings, root) = persistence_settings("juebi-complete-no-quota");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let record = ActiveTribulationRecord {
        char_id: "offline:Azure".to_string(),
        kind: TRIBULATION_KIND_JUE_BI.to_string(),
        source: "void_action_explode_zone".to_string(),
        origin_dimension: Some("minecraft:overworld".to_string()),
        wave_current: 3,
        waves_total: 3,
        started_tick: 2880,
        epicenter: [0.0, 64.0, 0.0],
        intensity: 1.6,
    };
    persist_active_tribulation(&settings, &record).expect("active JueBi should persist");

    let quota = complete_tribulation_ascension(&settings, record.char_id.as_str())
        .expect("independent JueBi completion should clear active row");
    assert_eq!(quota.occupied_slots, 0);

    let loaded_quota = load_ascension_quota(&settings).expect("quota load should succeed");
    assert_eq!(loaded_quota.occupied_slots, 0);
    let active = load_active_tribulation(&settings, record.char_id.as_str())
        .expect("active tribulation query should succeed");
    assert!(active.is_none());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn complete_void_quota_juebi_clears_active_and_increments_quota() {
    let (settings, root) = persistence_settings("juebi-complete-void-quota");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let record = ActiveTribulationRecord {
        char_id: "offline:Azure".to_string(),
        kind: TRIBULATION_KIND_JUE_BI.to_string(),
        source: JUEBI_SOURCE_VOID_QUOTA_EXCEEDED.to_string(),
        origin_dimension: Some("minecraft:overworld".to_string()),
        wave_current: 3,
        waves_total: 3,
        started_tick: 2880,
        epicenter: [0.0, 64.0, 0.0],
        intensity: 1.6,
    };
    persist_active_tribulation(&settings, &record).expect("void-quota JueBi should persist");

    let quota = complete_tribulation_ascension(&settings, record.char_id.as_str())
        .expect("void-quota JueBi completion should occupy quota");
    assert_eq!(quota.occupied_slots, 1);

    let loaded_quota = load_ascension_quota(&settings).expect("quota load should succeed");
    assert_eq!(loaded_quota.occupied_slots, 1);
    let active = load_active_tribulation(&settings, record.char_id.as_str())
        .expect("active tribulation query should succeed");
    assert!(active.is_none());

    let _ = fs::remove_dir_all(root);
}

// r1-P5 并发增量单调性回归钉：两次串行调用不得互相覆盖，最终 occupied_slots 必须 == 2。
// 若将来有人把 IMMEDIATE 改回 DEFERRED，在非 WAL 连接池场景下两次各读 0→写 1，
// 第二次覆盖第一次，occupied_slots 会是 1 而非 2，本测试立刻撞红。
#[test]
fn complete_tribulation_ascension_concurrent_increments_are_not_lost() {
    let (settings, root) = persistence_settings("ascension-quota-concurrent-increments-not-lost");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    // 两位不同修士各自有活跃渡劫行
    for char_id in ["offline:QingLong", "offline:BaiHu"] {
        let record = ActiveTribulationRecord {
            char_id: char_id.to_string(),
            kind: "du_xu".to_string(),
            source: String::new(),
            origin_dimension: Some("minecraft:overworld".to_string()),
            wave_current: 9,
            waves_total: 9,
            started_tick: 10_000,
            epicenter: [0.0, 64.0, 0.0],
            intensity: 0.0,
        };
        persist_active_tribulation(&settings, &record).expect("active du_xu row should persist");
    }

    // 串行化调用，模拟两次 DuXu 成功：第一次
    let q1 = complete_tribulation_ascension(&settings, "offline:QingLong")
        .expect("first completion should succeed");
    assert_eq!(
        q1.occupied_slots, 1,
        "after first completion occupied_slots 应为 1，实际为 {} — IMMEDIATE 事务保证增量不被覆盖",
        q1.occupied_slots
    );

    // 第二次——在不使用 IMMEDIATE 的 lost-update 场景下，第二次会读到「已提交的 1」
    // 并正确写 2；若 DEFERRED + 连接池序列化失效，则会写错的 1（读到过期值 0）。
    // IMMEDIATE 保证第二次必然读到最新 committed 值。
    let q2 = complete_tribulation_ascension(&settings, "offline:BaiHu")
        .expect("second completion should succeed");
    assert_eq!(
        q2.occupied_slots, 2,
        "after second completion occupied_slots 应为 2，实际为 {} — 第二次 IMMEDIATE 事务应读到第一次已提交的 1",
        q2.occupied_slots
    );

    // 从 DB 重新加载确认持久化
    let loaded = load_ascension_quota(&settings).expect("quota reload should succeed");
    assert_eq!(
        loaded.occupied_slots, 2,
        "reload 后 occupied_slots 应为 2，实际为 {} — 两次增量必须都落盘",
        loaded.occupied_slots
    );

    // 两个 active 行均已清除
    for char_id in ["offline:QingLong", "offline:BaiHu"] {
        let active =
            load_active_tribulation(&settings, char_id).expect("active query should not error");
        assert!(active.is_none(), "{char_id} 的 active 行应已删除，但仍存在");
    }

    let _ = fs::remove_dir_all(root);
}

// r1-P5 事务行为钉：complete_tribulation_ascension 在已有竞争 IMMEDIATE 写锁时
// 不应返回 SQLITE_BUSY 错误 —— 它必须等锁而非立刻失败。
// 策略：用第二个 Connection 手动开启 BEGIN IMMEDIATE，持锁期间调用函数，
// 然后在另一线程释放锁，验证函数最终成功而非返回 Err。
#[test]
fn complete_tribulation_ascension_uses_immediate_transaction_behavior() {
    use rusqlite::Connection;
    use std::sync::{Arc, Barrier};
    use std::thread;

    let (settings, root) = persistence_settings("ascension-quota-immediate-txn-behavior");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    // 预先插入 active 行供函数消费
    let record = ActiveTribulationRecord {
        char_id: "offline:XuanWu".to_string(),
        kind: "du_xu".to_string(),
        source: String::new(),
        origin_dimension: Some("minecraft:overworld".to_string()),
        wave_current: 7,
        waves_total: 7,
        started_tick: 5_000,
        epicenter: [0.0, 64.0, 0.0],
        intensity: 0.0,
    };
    persist_active_tribulation(&settings, &record)
        .expect("active row should persist before lock test");

    // 开竞争写连接：BEGIN IMMEDIATE 拿写锁，然后用 Barrier 协调释放时机
    let db_path = settings.db_path().to_path_buf();
    let barrier_before = Arc::new(Barrier::new(2));
    let barrier_after = Arc::new(Barrier::new(2));
    let b_before_clone = Arc::clone(&barrier_before);
    let b_after_clone = Arc::clone(&barrier_after);

    let lock_thread = thread::spawn(move || {
        let mut conn = Connection::open(&db_path).expect("competitor conn should open");
        // 必须设置 busy_timeout，否则 IMMEDIATE 立即失败
        conn.busy_timeout(std::time::Duration::from_secs(5))
            .expect("busy timeout should set");
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("competitor IMMEDIATE txn should start");
        // 持锁期间写入 occupied_slots=10——commit 后被测函数（IMMEDIATE 等锁）
        // 必须读到这个新值。若函数退化为 DEFERRED，会在 commit 前读到旧值 0、
        // 写 1，丢掉本次更新，下方断言 11 立即撞红（pin read-after-write）。
        upsert_ascension_quota(
            &tx,
            &AscensionQuotaRecord { occupied_slots: 10 },
            current_unix_seconds(),
        )
        .expect("competitor should stage occupied_slots=10 before releasing the write lock");
        // 写锁已持有 + 已暂存写入，通知主线程可以发起 complete_tribulation_ascension
        b_before_clone.wait();
        // 等主线程通知可以释放
        b_after_clone.wait();
        // commit（或 drop 回滚）释放写锁
        tx.commit().expect("competitor commit should succeed");
    });

    // 等竞争线程拿到写锁
    barrier_before.wait();

    // complete_tribulation_ascension 内部应 BEGIN IMMEDIATE 并等待锁；
    // 在竞争 tx commit 前它会 busy-wait（因为 open_persistence_connection 设了
    // busy_timeout），不应提前失败。
    // 我们在另一个线程发起，以免主线程阻塞影响 barrier 释放
    let settings_clone = settings.clone();
    let call_thread = thread::spawn(move || {
        // 给竞争线程 25 ms 确保确实已经拿锁，避免 race
        thread::sleep(std::time::Duration::from_millis(25));
        complete_tribulation_ascension(&settings_clone, "offline:XuanWu")
    });

    // 短暂后释放竞争锁
    thread::sleep(std::time::Duration::from_millis(50));
    barrier_after.wait();

    let result = call_thread.join().expect("call thread should not panic");
    assert!(
        result.is_ok(),
        "complete_tribulation_ascension 在竞争 IMMEDIATE 锁释放后应成功，实际错误：{:?} \
         — 若函数使用 DEFERRED 且 busy_timeout 未设，可能提前 SQLITE_BUSY",
        result.err()
    );
    let quota = result.unwrap();
    assert_eq!(
        quota.occupied_slots, 11,
        "竞争事务先提交 occupied_slots=10，本调用须在 BEGIN IMMEDIATE 等锁后读到 10 并写 11；\
         实际为 {} — 若读到 0（旧值）说明退化为 DEFERRED 读后写、丢更新",
        quota.occupied_slots
    );

    lock_thread.join().expect("lock thread should not panic");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn release_ascension_quota_slot_decrements_safely() {
    let (settings, root) = persistence_settings("ascension-quota-release");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let wall_clock = current_unix_seconds();
    let mut connection = open_persistence_connection(&settings).expect("db should open");
    let transaction = connection.transaction().expect("transaction should open");
    upsert_ascension_quota(
        &transaction,
        &AscensionQuotaRecord { occupied_slots: 2 },
        wall_clock,
    )
    .expect("quota upsert should succeed");
    transaction.commit().expect("transaction should commit");

    let release = release_ascension_quota_slot(&settings).expect("release should succeed");
    assert_eq!(release.quota.occupied_slots, 1);
    assert!(release.opened_slot);
    let release = release_ascension_quota_slot(&settings).expect("release should succeed");
    assert_eq!(release.quota.occupied_slots, 0);
    assert!(release.opened_slot);
    let release = release_ascension_quota_slot(&settings).expect("empty release should succeed");
    assert_eq!(release.quota.occupied_slots, 0);
    assert!(!release.opened_slot);

    let _ = fs::remove_dir_all(root);
}

// r3-P2 事务行为钉：release_ascension_quota_slot 在已有竞争 IMMEDIATE 写锁时
// 不应返回 SQLITE_BUSY 错误 —— 它必须等锁而非立刻失败。
// 策略：用第二个 Connection 手动开启 BEGIN IMMEDIATE，持锁期间写 occupied_slots=10
// 并 commit 后，被测函数（IMMEDIATE 等锁）必须读到 10 并写 9（减 1），
// 而非读到旧值（如 0）再写 0——后者说明退化为 DEFERRED 读、丢失竞争写。
#[test]
fn release_ascension_quota_slot_uses_immediate_transaction_behavior() {
    use rusqlite::Connection;
    use std::sync::{Arc, Barrier};
    use std::thread;

    let (settings, root) = persistence_settings("release-quota-immediate-txn-behavior");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    // 初始 occupied_slots 为 0（bootstrap 默认）
    let db_path = settings.db_path().to_path_buf();
    let barrier_before = Arc::new(Barrier::new(2));
    let barrier_after = Arc::new(Barrier::new(2));
    let b_before_clone = Arc::clone(&barrier_before);
    let b_after_clone = Arc::clone(&barrier_after);

    let lock_thread = thread::spawn(move || {
        let mut conn = Connection::open(&db_path).expect("competitor conn should open");
        // 设置 busy_timeout 防止竞争连接本身立即 SQLITE_BUSY
        conn.busy_timeout(std::time::Duration::from_secs(5))
            .expect("busy timeout should set");
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("competitor IMMEDIATE txn should start");
        // 持锁期间将 occupied_slots 写为 10 —— commit 后被测函数（IMMEDIATE 等锁）
        // 必须读到 10 并写 9。若函数退化为 DEFERRED，会在 commit 前读到旧值 0、
        // 写 0（saturating_sub 0 → 0），下方断言 9 立即撞红（pin read-after-write）。
        upsert_ascension_quota(
            &tx,
            &AscensionQuotaRecord { occupied_slots: 10 },
            current_unix_seconds(),
        )
        .expect("competitor should stage occupied_slots=10 before releasing write lock");
        // 写锁已持有 + 已暂存写入，通知主线程可以发起 release_ascension_quota_slot
        b_before_clone.wait();
        // 等主线程通知可以释放
        b_after_clone.wait();
        tx.commit().expect("competitor commit should succeed");
    });

    // 等竞争线程拿到写锁
    barrier_before.wait();

    // release_ascension_quota_slot 内部应 BEGIN IMMEDIATE 并等待锁；
    // 在竞争 tx commit 前它会 busy-wait（open_persistence_connection 设了 busy_timeout），
    // 不应提前失败。在另一线程发起，避免主线程阻塞影响 barrier 释放。
    let settings_clone = settings.clone();
    let call_thread = thread::spawn(move || {
        // 给竞争线程 25 ms 确保已确实拿锁，避免 race
        thread::sleep(std::time::Duration::from_millis(25));
        release_ascension_quota_slot(&settings_clone)
    });

    // 短暂后释放竞争锁
    thread::sleep(std::time::Duration::from_millis(50));
    barrier_after.wait();

    let result = call_thread.join().expect("call thread should not panic");
    assert!(
        result.is_ok(),
        "release_ascension_quota_slot 在竞争 IMMEDIATE 锁释放后应成功，实际错误：{:?} \
         — 若函数使用 DEFERRED 且 busy_timeout 未设，可能提前 SQLITE_BUSY",
        result.err()
    );
    let release = result.unwrap();
    assert_eq!(
        release.quota.occupied_slots, 9,
        "竞争事务先提交 occupied_slots=10，本调用须在 BEGIN IMMEDIATE 等锁后读到 10 并写 9；\
         实际为 {} — 若读到 0（旧值）说明退化为 DEFERRED 读后写、丢更新（saturating_sub 0→0）",
        release.quota.occupied_slots
    );
    assert!(
        release.opened_slot,
        "occupied_slots 从 10 减到 9，opened_slot 应为 true；实际为 false — \
         说明函数读到了错误的旧值 0（没有打开名额）"
    );

    lock_thread.join().expect("lock thread should not panic");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn zones_runtime_roundtrip_persists_spirit_qi_and_danger_level() {
    let (settings, root) = persistence_settings("zones-runtime-roundtrip");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let zones = crate::world::zone::ZoneRegistry {
        spatial_revision: 0,
        zones: vec![crate::world::zone::Zone {
            name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            dimension: crate::world::dimension::DimensionKind::Overworld,
            bounds: crate::world::zone::default_spawn_bounds(),
            spirit_qi: 0.42,
            danger_level: 3,
            active_events: vec!["beast_tide".to_string()],
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
            qi_equilibrium: 0.0,
            qi_inflow_per_min: 0.0,
        }],
    };

    persist_zone_runtime_snapshot(&settings, &zones).expect("zone runtime snapshot should persist");
    let records = load_zone_runtime_snapshot(&settings).expect("zone runtime snapshot should load");
    assert_eq!(
        records,
        vec![ZoneRuntimeRecord {
            zone_id: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            spirit_qi: 0.42,
            danger_level: 3,
        }]
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn heartbeat_pseudo_veins_roundtrip_preserves_dynamic_zone_lifecycle() {
    let (settings, root) = persistence_settings("heartbeat-pseudo-vein-roundtrip");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let record = heartbeat_pseudo_vein_record("pseudo_vein_heartbeat_7");
    let mut heartbeat = WorldHeartbeat::default();
    let mut zones = crate::world::zone::ZoneRegistry::fallback();
    let restored = heartbeat.restore_pseudo_vein_records(
        &mut zones,
        std::slice::from_ref(&record),
        record.last_tick,
    );
    assert_eq!(
        restored, 1,
        "fixture record must restore into heartbeat before persistence roundtrip"
    );
    zones
        .find_zone_mut(record.zone_id.as_str())
        .expect("restored heartbeat zone must exist")
        .spirit_qi = record.qi_current;

    persist_heartbeat_pseudo_veins_snapshot(&settings, &heartbeat, &zones)
        .expect("heartbeat pseudo-vein snapshot should persist");
    let records = load_heartbeat_pseudo_veins_snapshot(&settings)
        .expect("heartbeat pseudo-vein snapshot should load");

    let mut expected = record;
    expected.snapshot_wall = records[0].snapshot_wall;
    assert_eq!(
        records,
        vec![expected],
        "伪灵脉 heartbeat runtime 必须完整保留 bounds/dimension/lifecycle/warning/season"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn atomic_pseudo_vein_snapshot_uses_physical_zone_qi_between_heartbeat_ticks() {
    let (settings, root) = persistence_settings("heartbeat-zone-qi-atomic-snapshot");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let record = heartbeat_pseudo_vein_record("pseudo_vein_heartbeat_7");
    let mut heartbeat = WorldHeartbeat::default();
    let mut zones = crate::world::zone::ZoneRegistry::fallback();
    assert_eq!(
        heartbeat.restore_pseudo_vein_records(
            &mut zones,
            std::slice::from_ref(&record),
            record.last_tick,
        ),
        1,
        "fixture record must restore before atomic snapshot"
    );
    zones
        .find_zone_mut(record.zone_id.as_str())
        .expect("restored dynamic zone must exist")
        .spirit_qi = 0.29;

    persist_zone_runtime_snapshot_with_heartbeat(
        &settings,
        &zones,
        Some(&heartbeat),
        &WorldQiAccount::default(),
    )
    .expect("atomic zone/lifecycle snapshot should persist");

    let heartbeat_rows =
        load_heartbeat_pseudo_veins_snapshot(&settings).expect("heartbeat rows should load");
    let zone_rows = load_zone_runtime_snapshot(&settings).expect("zone runtime rows should load");
    assert_eq!(
        heartbeat_rows
            .iter()
            .find(|row| row.zone_id == record.zone_id)
            .expect("dynamic heartbeat row must persist")
            .qi_current,
        0.29,
        "expected lifecycle row to use the physical zone balance, not stale heartbeat state"
    );
    assert_eq!(
        zone_rows
            .iter()
            .find(|row| row.zone_id == record.zone_id)
            .expect("dynamic zone runtime row must persist")
            .spirit_qi,
        0.29,
        "expected atomic zones_runtime row to match heartbeat lifecycle qi"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn zone_runtime_snapshot_removes_dissipated_heartbeat_zone_orphan() {
    let (settings, root) = persistence_settings("heartbeat-zone-runtime-orphan-cleanup");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let record = heartbeat_pseudo_vein_record("pseudo_vein_heartbeat_7");
    let mut heartbeat = WorldHeartbeat::default();
    let mut zones = crate::world::zone::ZoneRegistry::fallback();
    assert_eq!(
        heartbeat.restore_pseudo_vein_records(
            &mut zones,
            std::slice::from_ref(&record),
            record.last_tick,
        ),
        1,
        "fixture record must restore before first snapshot"
    );
    zones
        .find_zone_mut(record.zone_id.as_str())
        .expect("restored dynamic zone must exist")
        .spirit_qi = record.qi_current;
    let qi_ledger = WorldQiAccount::default();
    persist_zone_runtime_snapshot_with_heartbeat(&settings, &zones, Some(&heartbeat), &qi_ledger)
        .expect("active pseudo-vein snapshot should persist");
    assert!(
        load_zone_runtime_snapshot(&settings)
            .expect("first zone runtime snapshot should load")
            .iter()
            .any(|row| row.zone_id == record.zone_id),
        "expected active dynamic zone row before dissipation"
    );

    zones.zones.retain(|zone| zone.name != record.zone_id);
    let no_active_heartbeat = WorldHeartbeat::default();
    persist_zone_runtime_snapshot_with_heartbeat(
        &settings,
        &zones,
        Some(&no_active_heartbeat),
        &qi_ledger,
    )
    .expect("post-dissipation snapshot should commit");

    assert!(
        load_heartbeat_pseudo_veins_snapshot(&settings)
            .expect("post-dissipation heartbeat snapshot should load")
            .is_empty(),
        "expected lifecycle row to disappear after dissipation"
    );
    assert!(
        load_zone_runtime_snapshot(&settings)
            .expect("post-dissipation zone runtime snapshot should load")
            .iter()
            .all(|row| row.zone_id != record.zone_id),
        "expected stale dynamic zones_runtime row to be deleted in the same snapshot"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn zone_overlays_roundtrip_preserves_ordered_records() {
    let (settings, root) = persistence_settings("zone-overlays-roundtrip");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let overlays = vec![
        ZoneOverlayRecord {
            zone_id: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            overlay_kind: "collapsed".to_string(),
            payload_json: serde_json::json!({"danger_level": 3}).to_string(),
            payload_version: ZONE_OVERLAY_PAYLOAD_VERSION,
            since_wall: 10,
        },
        ZoneOverlayRecord {
            zone_id: "blood_valley".to_string(),
            overlay_kind: "ruins_discovered".to_string(),
            payload_json: serde_json::json!({"active_events": ["ruins_discovered"]}).to_string(),
            payload_version: ZONE_OVERLAY_PAYLOAD_VERSION,
            since_wall: 20,
        },
    ];

    persist_zone_overlays(&settings, &overlays).expect("zone overlays should persist");
    let loaded = load_zone_overlays(&settings).expect("zone overlays should load");
    assert_eq!(
        loaded,
        vec![
            ZoneOverlayRecord {
                zone_id: "blood_valley".to_string(),
                overlay_kind: "ruins_discovered".to_string(),
                payload_json: serde_json::json!({"active_events": ["ruins_discovered"]})
                    .to_string(),
                payload_version: ZONE_OVERLAY_PAYLOAD_VERSION,
                since_wall: 20,
            },
            ZoneOverlayRecord {
                zone_id: DEFAULT_SPAWN_ZONE_NAME.to_string(),
                overlay_kind: "collapsed".to_string(),
                payload_json: serde_json::json!({"danger_level": 3}).to_string(),
                payload_version: ZONE_OVERLAY_PAYLOAD_VERSION,
                since_wall: 10,
            },
        ]
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn zone_overlay_payload_migration_upgrades_v1_and_preserves_future_versions() {
    let (settings, root) = persistence_settings("zone-overlay-payload-migration");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let connection = Connection::open(settings.db_path()).expect("db should open");
    connection
        .execute(
            "
            INSERT INTO zone_overlays (
                zone_id, overlay_kind, payload_json, payload_version,
                since_wall, schema_version, last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ",
            params![
                DEFAULT_SPAWN_ZONE_NAME,
                "collapsed",
                serde_json::json!({"danger_level": 4}).to_string(),
                1_i64,
                10_i64,
                CURRENT_SCHEMA_VERSION,
                10_i64,
            ],
        )
        .expect("legacy v1 zone overlay should insert");
    connection
        .execute(
            "
            INSERT INTO zone_overlays (
                zone_id, overlay_kind, payload_json, payload_version,
                since_wall, schema_version, last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ",
            params![
                DEFAULT_SPAWN_ZONE_NAME,
                "qi_eye_formed",
                serde_json::json!({"active_events": ["future_qi_eye"]}).to_string(),
                i64::from(ZONE_OVERLAY_PAYLOAD_VERSION + 1),
                11_i64,
                CURRENT_SCHEMA_VERSION,
                11_i64,
            ],
        )
        .expect("future zone overlay should insert for preservation drill");
    drop(connection);

    let loaded = load_zone_overlays(&settings).expect("zone overlays should load");
    assert_eq!(
        loaded.len(),
        2,
        "future payload_version rows should be preserved so delete+reinsert writers cannot drop them"
    );
    let overlay = &loaded[0];
    assert_eq!(overlay.overlay_kind, "collapsed");
    assert_eq!(overlay.payload_version, ZONE_OVERLAY_PAYLOAD_VERSION);
    let payload: Value = serde_json::from_str(overlay.payload_json.as_str())
        .expect("migrated overlay payload should remain JSON");
    assert_eq!(payload["danger_level"].as_u64(), Some(4));
    assert_eq!(
        payload["payload_schema"].as_str(),
        Some("zone_overlay_v2"),
        "v1 payload migration should stamp the v2 marker field"
    );
    assert_eq!(loaded[1].overlay_kind, "qi_eye_formed");
    assert_eq!(loaded[1].payload_version, ZONE_OVERLAY_PAYLOAD_VERSION + 1);

    persist_zone_overlays(&settings, &loaded)
        .expect("delete+reinsert writer should preserve future overlay rows atomically");
    let connection = Connection::open(settings.db_path()).expect("db should reopen");
    let future_count: i64 = connection
        .query_row(
            "
            SELECT COUNT(*) FROM zone_overlays
            WHERE overlay_kind = 'qi_eye_formed' AND payload_version = ?1
            ",
            params![i64::from(ZONE_OVERLAY_PAYLOAD_VERSION + 1)],
            |row| row.get(0),
        )
        .expect("future overlay count should be readable");
    assert_eq!(future_count, 1);

    let mut registry = crate::world::zone::ZoneRegistry::fallback();
    hydrate_zone_overlays(&settings, &mut registry)
        .expect("future overlay should be skipped at runtime apply only");
    assert!(
        !registry.zones[0]
            .active_events
            .iter()
            .any(|event| event == "future_qi_eye"),
        "future payload_version should not be applied to runtime zones"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn bootstrap_hydrates_zone_overlays_into_registry() {
    let (settings, root) = persistence_settings("zone-overlays-hydrate");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    persist_zone_overlays(
        &settings,
        &[ZoneOverlayRecord {
            zone_id: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            overlay_kind: "collapsed".to_string(),
            payload_json: serde_json::json!({
                "danger_level": 4,
                "active_events": ["realm_collapse"],
                "blocked_tiles": [[7, 8]],
            })
            .to_string(),
            payload_version: ZONE_OVERLAY_PAYLOAD_VERSION,
            since_wall: 100,
        }],
    )
    .expect("zone overlays should persist");

    let mut registry = crate::world::zone::ZoneRegistry::fallback();
    hydrate_zone_overlays(&settings, &mut registry).expect("zone overlay hydration should succeed");
    assert_eq!(registry.zones[0].danger_level, 4);
    assert_eq!(
        registry.zones[0].active_events,
        vec!["realm_collapse".to_string()]
    );
    assert_eq!(registry.zones[0].blocked_tiles, vec![(7, 8)]);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn bootstrap_persistence_keeps_fallback_zone_registry_when_overlay_payload_is_invalid() {
    let (settings, root) = persistence_settings("zone-overlays-invalid-payload-bootstrap");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    persist_zone_overlays(
        &settings,
        &[ZoneOverlayRecord {
            zone_id: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            overlay_kind: "collapsed".to_string(),
            payload_json: serde_json::json!({
                "danger_level": "not-a-number",
                "active_events": ["realm_collapse"],
                "blocked_tiles": [[7, 8]],
            })
            .to_string(),
            payload_version: ZONE_OVERLAY_PAYLOAD_VERSION,
            since_wall: 100,
        }],
    )
    .expect("invalid zone overlay payload row should still persist");

    let mut app = App::new();
    app.insert_resource(settings.clone());
    app.insert_resource(DailyBackupState::default());
    app.insert_resource(CultivationClock::default());
    app.insert_resource(WorldQiAccount::default());
    app.insert_resource(crate::world::zone::ZoneRegistry::fallback());
    app.add_systems(Startup, bootstrap_persistence_system);

    app.update();

    let registry = app.world().resource::<crate::world::zone::ZoneRegistry>();
    assert_eq!(
        registry.zones.len(),
        1,
        "fallback registry should remain intact"
    );

    let spawn = &registry.zones[0];
    assert_eq!(spawn.name, DEFAULT_SPAWN_ZONE_NAME);
    assert_eq!(spawn.danger_level, 0);
    assert!(spawn.active_events.is_empty());
    assert!(spawn.blocked_tiles.is_empty());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn export_zone_persistence_aggregates_runtime_and_overlays() {
    let (settings, root) = persistence_settings("zone-export-bundle");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let zones = crate::world::zone::ZoneRegistry {
        spatial_revision: 0,
        zones: vec![crate::world::zone::Zone {
            name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            dimension: crate::world::dimension::DimensionKind::Overworld,
            bounds: crate::world::zone::default_spawn_bounds(),
            spirit_qi: 0.31,
            danger_level: 2,
            active_events: Vec::new(),
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
            qi_equilibrium: 0.0,
            qi_inflow_per_min: 0.0,
        }],
    };
    persist_zone_runtime_snapshot(&settings, &zones).expect("zone runtime snapshot should persist");
    persist_zone_overlays(
        &settings,
        &[ZoneOverlayRecord {
            zone_id: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            overlay_kind: "collapsed".to_string(),
            payload_json: serde_json::json!({"danger_level": 4}).to_string(),
            payload_version: ZONE_OVERLAY_PAYLOAD_VERSION,
            since_wall: 42,
        }],
    )
    .expect("zone overlays should persist");

    let bundle = export_zone_persistence(&settings).expect("zone export should succeed");
    assert_eq!(bundle.schema_version, CURRENT_SCHEMA_VERSION);
    assert_eq!(bundle.kind, "zones_export_v1");
    assert_eq!(bundle.zones_runtime.len(), 1);
    assert_eq!(bundle.zone_overlays.len(), 1);
    assert_eq!(bundle.zones_runtime[0].zone_id, DEFAULT_SPAWN_ZONE_NAME);
    assert_eq!(bundle.zone_overlays[0].overlay_kind, "collapsed");
    assert_eq!(
        bundle.zone_overlays[0].payload_version,
        ZONE_OVERLAY_PAYLOAD_VERSION
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn import_zone_persistence_replaces_existing_zone_rows_atomically() {
    let (settings, root) = persistence_settings("zone-import-bundle");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let existing_zones = crate::world::zone::ZoneRegistry {
        spatial_revision: 0,
        zones: vec![crate::world::zone::Zone {
            name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            dimension: crate::world::dimension::DimensionKind::Overworld,
            bounds: crate::world::zone::default_spawn_bounds(),
            spirit_qi: -0.55,
            danger_level: 5,
            active_events: Vec::new(),
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
            qi_equilibrium: 0.0,
            qi_inflow_per_min: 0.0,
        }],
    };
    persist_zone_runtime_snapshot(&settings, &existing_zones)
        .expect("existing zone runtime should persist");
    persist_zone_overlays(
        &settings,
        &[ZoneOverlayRecord {
            zone_id: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            overlay_kind: "collapsed".to_string(),
            payload_json: serde_json::json!({"danger_level": 5}).to_string(),
            payload_version: ZONE_OVERLAY_PAYLOAD_VERSION,
            since_wall: 1,
        }],
    )
    .expect("existing zone overlays should persist");

    let bundle = ZoneExportBundle {
        schema_version: CURRENT_SCHEMA_VERSION,
        kind: "zones_export_v1".to_string(),
        zones_runtime: vec![ZoneRuntimeRecord {
            zone_id: "blood_valley".to_string(),
            spirit_qi: 0.44,
            danger_level: 2,
        }],
        zone_overlays: vec![ZoneOverlayRecord {
            zone_id: "blood_valley".to_string(),
            overlay_kind: "ruins_discovered".to_string(),
            payload_json: serde_json::json!({"active_events": ["ruins_discovered"]}).to_string(),
            payload_version: ZONE_OVERLAY_PAYLOAD_VERSION,
            since_wall: 99,
        }],
    };

    import_zone_persistence(&settings, &bundle).expect("zone import should succeed");

    assert_eq!(
        load_zone_runtime_snapshot(&settings).expect("zone runtime should load"),
        bundle.zones_runtime
    );
    assert_eq!(
        load_zone_overlays(&settings).expect("zone overlays should load"),
        bundle.zone_overlays
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn import_zone_persistence_rejects_wrong_kind() {
    let (settings, root) = persistence_settings("zone-import-wrong-kind");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let error = import_zone_persistence(
        &settings,
        &ZoneExportBundle {
            schema_version: CURRENT_SCHEMA_VERSION,
            kind: "players_export_v1".to_string(),
            zones_runtime: Vec::new(),
            zone_overlays: Vec::new(),
        },
    )
    .expect_err("wrong kind should be rejected");
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn import_zone_persistence_rejects_future_schema_version() {
    let (settings, root) = persistence_settings("zone-import-future-schema");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let error = import_zone_persistence(
        &settings,
        &ZoneExportBundle {
            schema_version: CURRENT_SCHEMA_VERSION + 1,
            kind: "zones_export_v1".to_string(),
            zones_runtime: Vec::new(),
            zone_overlays: Vec::new(),
        },
    )
    .expect_err("future schema_version should be rejected");
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn bootstrap_rejects_future_user_version() {
    let db_path = database_path("future-user-version-rejected");
    bootstrap_sqlite(&db_path, "future-user-version-rejected").expect("bootstrap should succeed");

    let connection = Connection::open(&db_path).expect("db should open");
    let bootstrap_events_before: i64 = connection
        .query_row("SELECT COUNT(*) FROM bootstrap_events", [], |row| {
            row.get(0)
        })
        .expect("bootstrap event count should be readable before rejection");
    connection
        .execute_batch("PRAGMA user_version = 999;")
        .expect("user_version override should succeed");
    drop(connection);

    let error = bootstrap_sqlite(&db_path, "future-user-version-rejected")
        .expect_err("future user_version should be rejected");
    assert!(
        matches!(error, rusqlite::Error::ToSqlConversionFailure(_)),
        "unexpected error when rejecting future user_version: {error:?}"
    );
    assert!(
        error.to_string().contains("is newer than supported"),
        "future user_version rejection should include a specific mismatch message: {error:?}"
    );

    let connection = Connection::open(&db_path).expect("db should reopen");
    let user_version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("user_version should remain readable after rejection");
    let bootstrap_events_after: i64 = connection
        .query_row("SELECT COUNT(*) FROM bootstrap_events", [], |row| {
            row.get(0)
        })
        .expect("bootstrap event count should be readable after rejection");
    assert_eq!(user_version, 999);
    assert_eq!(
        bootstrap_events_after, bootstrap_events_before,
        "future user_version rejection must not record a new bootstrap event"
    );
}

#[test]
fn legacy_v9_reader_rejects_current_v10_database() {
    let db_path = database_path("legacy-v9-reader-rejects-v10-db");
    bootstrap_sqlite(&db_path, "legacy-v9-reader-rejects-v10-db")
        .expect("bootstrap should succeed");

    let connection = Connection::open(&db_path).expect("db should open");
    connection
        .execute(
            "
            INSERT INTO zone_overlays (
                zone_id,
                overlay_kind,
                payload_json,
                since_wall,
                schema_version,
                last_updated_wall,
                payload_version
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ",
            params![
                DEFAULT_SPAWN_ZONE_NAME,
                "collapsed",
                serde_json::json!({"danger_level": 5}).to_string(),
                123_i64,
                CURRENT_SCHEMA_VERSION,
                456_i64,
                1_i64,
            ],
        )
        .expect("current-schema zone_overlays row should insert");

    let user_version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("user_version should be readable");
    assert_eq!(user_version as i32, CURRENT_USER_VERSION);

    let error = reject_if_user_version_exceeds_supported(&connection, CURRENT_USER_VERSION - 1)
        .expect_err("simulated v9 reader should reject current v10 database");
    assert!(
        matches!(error, rusqlite::Error::ExecuteReturnedResults),
        "unexpected error when simulating legacy v9 rejection: {error:?}"
    );

    let row_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM zone_overlays", [], |row| row.get(0))
        .expect("zone_overlays count should be readable after rejection");
    assert_eq!(row_count, 1);

    let _ = fs::remove_dir_all(
        db_path
            .parent()
            .expect("legacy reader test db path should still have parent directory"),
    );
}

#[test]
fn bootstrap_hydrates_zone_runtime_into_registry() {
    let (settings, root) = persistence_settings("zones-runtime-hydrate");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let persisted = crate::world::zone::ZoneRegistry {
        spatial_revision: 0,
        zones: vec![crate::world::zone::Zone {
            name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            dimension: crate::world::dimension::DimensionKind::Overworld,
            bounds: crate::world::zone::default_spawn_bounds(),
            spirit_qi: -0.15,
            danger_level: 4,
            active_events: Vec::new(),
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
            qi_equilibrium: 0.0,
            qi_inflow_per_min: 0.0,
        }],
    };
    persist_zone_runtime_snapshot(&settings, &persisted)
        .expect("zone runtime snapshot should persist");

    let mut registry = crate::world::zone::ZoneRegistry::fallback();
    hydrate_zone_runtime(&settings, &mut registry).expect("zone runtime hydration should succeed");
    assert_eq!(registry.zones[0].spirit_qi, -0.15);
    assert_eq!(registry.zones[0].danger_level, 4);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn bootstrap_hydrates_heartbeat_pseudo_vein_before_zone_runtime_overlay() {
    let (settings, root) = persistence_settings("heartbeat-pseudo-vein-hydrate");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let mut record = heartbeat_pseudo_vein_record("pseudo_vein_heartbeat_7");
    record.qi_current = 0.41;
    let mut seed_heartbeat = WorldHeartbeat::default();
    let mut seed_zones = crate::world::zone::ZoneRegistry::fallback();
    assert_eq!(
        seed_heartbeat.restore_pseudo_vein_records(
            &mut seed_zones,
            std::slice::from_ref(&record),
            record.last_tick,
        ),
        1
    );
    persist_heartbeat_pseudo_veins_snapshot(&settings, &seed_heartbeat, &seed_zones)
        .expect("heartbeat pseudo-vein snapshot should persist");
    let persisted_record = load_heartbeat_pseudo_veins_snapshot(&settings)
        .expect("heartbeat pseudo-vein snapshot should load")
        .into_iter()
        .next()
        .expect("persisted heartbeat pseudo-vein record should exist");
    assert_eq!(persisted_record.observed_age_ticks, 800);

    let mut runtime_zones = seed_zones.clone();
    runtime_zones
        .find_zone_mut("pseudo_vein_heartbeat_7")
        .expect("seed pseudo-vein zone must exist")
        .spirit_qi = 0.33;
    persist_zone_runtime_snapshot(&settings, &runtime_zones)
        .expect("zone runtime snapshot should persist pseudo-vein row");

    let restart_tick = 25;
    let mut app = App::new();
    app.insert_resource(settings.clone());
    app.insert_resource(DailyBackupState::default());
    app.insert_resource(crate::world::zone::ZoneRegistry::fallback());
    app.insert_resource(WorldHeartbeat::default());
    app.insert_resource(CultivationClock { tick: restart_tick });
    app.insert_resource(WorldQiAccount::default());
    app.add_systems(Startup, bootstrap_persistence_system);
    let bootstrap_started_wall = current_unix_seconds();
    app.update();
    let bootstrap_finished_wall = current_unix_seconds();

    let restored_heartbeat = app.world().resource::<WorldHeartbeat>();
    let restored_zones = app.world().resource::<crate::world::zone::ZoneRegistry>();
    assert_eq!(
        restored_heartbeat.active_pseudo_vein_count(),
        1,
        "expected Startup bootstrap to restore one pseudo-vein, actual {}",
        restored_heartbeat.active_pseudo_vein_count()
    );
    let restored_records = restored_heartbeat.active_pseudo_vein_records(restored_zones);
    assert_eq!(
        restored_records.len(),
        1,
        "expected one persistable record after Startup hydration, actual {}",
        restored_records.len()
    );
    let restored_age = restored_records[0]
        .last_tick
        .saturating_sub(restored_records[0].spawned_at_tick);
    let expected_age_at = |current_wall: i64| {
        let elapsed_seconds = current_wall
            .saturating_sub(persisted_record.snapshot_wall)
            .max(0);
        persisted_record.observed_age_ticks.saturating_add(
            u64::try_from(elapsed_seconds)
                .unwrap_or(u64::MAX)
                .saturating_mul(crate::worldgen::pseudo_vein::TICKS_PER_SECOND),
        )
    };
    let minimum_expected_age = expected_age_at(bootstrap_started_wall);
    let maximum_expected_age = expected_age_at(bootstrap_finished_wall);
    let restored_offline_ticks = restored_age
        .checked_sub(persisted_record.observed_age_ticks)
        .expect("Startup hydration must not reduce the persisted pseudo-vein age");
    assert!(
        (minimum_expected_age..=maximum_expected_age).contains(&restored_age),
        "expected Startup hydration at raw tick {restart_tick} to retain persisted age 800 plus wall-clock offline ticks in {minimum_expected_age}..={maximum_expected_age}, actual {restored_age}"
    );
    assert_eq!(
        restored_offline_ticks % crate::worldgen::pseudo_vein::TICKS_PER_SECOND,
        0,
        "expected Startup offline age increment to be quantized in whole-second tick steps, actual increment {restored_offline_ticks}"
    );
    let restored_zone = restored_zones
        .find_zone_by_name("pseudo_vein_heartbeat_7")
        .expect("hydrate must recreate missing dynamic pseudo-vein zone before runtime rows");
    assert_eq!(restored_zone.dimension, DimensionKind::Overworld);
    assert_eq!(restored_zone.bounds.0, DVec3::new(-140.0, 60.0, -240.0));
    assert_eq!(
        restored_zone.spirit_qi, 0.33,
        "zones_runtime 三列表应在动态 zone 重建后照常覆盖最新 spirit_qi"
    );
    assert_eq!(
        restored_records[0].qi_current, 0.33,
        "expected zones_runtime physical balance to realign the restored lifecycle qi"
    );
    assert!(
        !app.world()
            .resource::<WorldQiAccount>()
            .has_account(&QiAccountId::zone("pseudo_vein_heartbeat_7")),
        "Startup hydration must restore dynamic pseudo-vein qi only into Zone.spirit_qi"
    );
    assert!(
        restored_zone
            .active_events
            .iter()
            .any(|event| event == crate::world::heartbeat::EVENT_PSEUDO_VEIN),
        "恢复出的动态 zone 必须继续带 pseudo_vein active_event"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn production_startup_order_restores_pseudo_vein_before_first_snapshot() {
    let (settings, root) = persistence_settings("heartbeat-production-startup-order");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let mut record = heartbeat_pseudo_vein_record("pseudo_vein_heartbeat_7");
    record.qi_current = 0.41;
    let mut seed_heartbeat = WorldHeartbeat::default();
    let mut seed_zones = crate::world::zone::ZoneRegistry::fallback();
    let seeded = seed_heartbeat.restore_pseudo_vein_records(
        &mut seed_zones,
        std::slice::from_ref(&record),
        record.last_tick,
    );
    assert_eq!(
        seeded, 1,
        "expected one seed pseudo-vein before production startup test, actual {seeded}"
    );
    seed_zones
        .find_zone_mut(record.zone_id.as_str())
        .expect("seed pseudo-vein zone must exist")
        .spirit_qi = record.qi_current;
    let zone_absolute = record.qi_current * QI_ZONE_UNIT_CAPACITY;
    let mut seed_ledger = WorldQiAccount::default();
    seed_ledger
        .set_balance(pending_inflow_account(), SPIRIT_QI_TOTAL - zone_absolute)
        .expect("seed pending inflow balance should be finite");
    let total_before_restart = zone_absolute + seed_ledger.total();
    persist_zone_runtime_snapshot_with_heartbeat(
        &settings,
        &seed_zones,
        Some(&seed_heartbeat),
        &seed_ledger,
    )
    .expect("seed pseudo-vein, zone runtime and pending pool should persist atomically");

    let mut app = App::new();
    app.insert_resource(settings.clone());
    app.insert_resource(WorldHeartbeat::default());
    app.insert_resource(CultivationClock { tick: 25 });
    app.insert_resource(WorldQiAccount::default());
    app.add_event::<crate::npc::dormant::PendingDormantRelicCreated>();
    crate::world::zone::register(&mut app);
    register(&mut app);
    let due_snapshot_wall =
        current_unix_seconds().saturating_sub(ZONE_RUNTIME_SNAPSHOT_INTERVAL_SECS);
    app.world_mut()
        .resource_mut::<ZoneRuntimeSnapshotState>()
        .last_snapshot_wall = due_snapshot_wall;

    app.update();

    let heartbeat = app.world().resource::<WorldHeartbeat>();
    assert_eq!(
        heartbeat.active_pseudo_vein_count(),
        1,
        "expected production Startup ordering to hydrate one pseudo-vein, actual {}",
        heartbeat.active_pseudo_vein_count()
    );
    let snapshot_wall = app
        .world()
        .resource::<ZoneRuntimeSnapshotState>()
        .last_snapshot_wall;
    assert!(
        snapshot_wall > due_snapshot_wall,
        "expected first Update to execute the due zone-runtime snapshot because Startup hydration completed first; previous wall {due_snapshot_wall}, actual {snapshot_wall}"
    );
    let persisted_after_update = load_heartbeat_pseudo_veins_snapshot(&settings)
        .expect("first Update should leave a readable pseudo-vein snapshot");
    assert_eq!(
        persisted_after_update.len(),
        1,
        "expected first Update snapshot to preserve hydrated SQLite row, actual {}",
        persisted_after_update.len()
    );
    assert_eq!(
        persisted_after_update[0].zone_id, record.zone_id,
        "expected first Update to retain the restored pseudo-vein id"
    );
    let persisted_zone_runtime = load_zone_runtime_snapshot(&settings)
        .expect("first Update should write a readable zone-runtime snapshot");
    let persisted_pseudo_vein = persisted_zone_runtime
        .iter()
        .find(|runtime| runtime.zone_id == record.zone_id)
        .expect("first Update must snapshot the pseudo-vein created during Startup hydration");
    assert_eq!(
        persisted_pseudo_vein.spirit_qi, record.qi_current,
        "expected first Update to persist hydrated pseudo-vein spirit_qi {}, actual {}",
        record.qi_current, persisted_pseudo_vein.spirit_qi
    );
    let restored_ledger = app.world().resource::<WorldQiAccount>();
    assert_eq!(
        restored_ledger.balance(&pending_inflow_account()),
        SPIRIT_QI_TOTAL - zone_absolute,
        "expected restart to restore the pending pool that backs the active pseudo-vein loan"
    );
    assert!(
        (record.qi_current * QI_ZONE_UNIT_CAPACITY + restored_ledger.total()
            - total_before_restart)
            .abs()
            < 1e-9,
        "expected pending pool plus external dynamic Zone owner to conserve across restart"
    );
    let persisted_heartbeat_record = persisted_after_update
        .iter()
        .find(|persisted| persisted.zone_id == record.zone_id)
        .expect("first Update must retain the restored pseudo-vein lifecycle record");
    assert_eq!(
        persisted_heartbeat_record.qi_current, persisted_pseudo_vein.spirit_qi,
        "expected first Update to persist identical lifecycle and zone qi values"
    );
    assert!(
        !app.world()
            .resource::<WorldQiAccount>()
            .has_account(&QiAccountId::zone(record.zone_id.as_str())),
        "expected restored pseudo-vein qi to remain solely in the external Zone owner"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn zone_runtime_snapshot_system_respects_five_minute_interval() {
    let (settings, root) = persistence_settings("zones-runtime-interval");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let mut app = App::new();
    app.insert_resource(settings.clone());
    app.insert_resource(ZoneRuntimeSnapshotState::default());
    app.insert_resource(CultivationClock::default());
    app.insert_resource(WorldQiAccount::default());
    app.insert_resource(crate::world::zone::ZoneRegistry {
        spatial_revision: 0,
        zones: vec![crate::world::zone::Zone {
            name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            dimension: crate::world::dimension::DimensionKind::Overworld,
            bounds: crate::world::zone::default_spawn_bounds(),
            spirit_qi: 0.25,
            danger_level: 1,
            active_events: Vec::new(),
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
            qi_equilibrium: 0.0,
            qi_inflow_per_min: 0.0,
        }],
    });
    app.add_systems(Update, persist_zone_runtime_system);

    app.update();
    let first_records =
        load_zone_runtime_snapshot(&settings).expect("first zone runtime snapshot should load");
    assert_eq!(first_records.len(), 1);

    {
        let mut snapshot_state = app.world_mut().resource_mut::<ZoneRuntimeSnapshotState>();
        snapshot_state.last_snapshot_wall = current_unix_seconds();
    }
    {
        let mut zones = app
            .world_mut()
            .resource_mut::<crate::world::zone::ZoneRegistry>();
        zones.zones[0].spirit_qi = -0.5;
        zones.zones[0].danger_level = 5;
    }

    app.update();
    let second_records =
        load_zone_runtime_snapshot(&settings).expect("second zone runtime snapshot should load");
    assert_eq!(second_records[0].spirit_qi, 0.25);
    assert_eq!(second_records[0].danger_level, 1);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn production_registry_dispatches_zone_runtime_slice_on_app_exit() {
    let (settings, root) = persistence_settings("production-zone-runtime-shutdown-slice");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let mut app = App::new();
    app.add_event::<AppExit>();
    app.add_event::<crate::npc::dormant::PendingDormantRelicCreated>();
    app.insert_resource(settings.clone());
    app.insert_resource(CultivationClock::default());
    app.insert_resource(WorldQiAccount::default());
    app.insert_resource(crate::world::zone::ZoneRegistry {
        zones: vec![crate::world::zone::Zone {
            name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            dimension: crate::world::dimension::DimensionKind::Overworld,
            bounds: crate::world::zone::default_spawn_bounds(),
            spirit_qi: 0.37,
            danger_level: 3,
            active_events: Vec::new(),
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
            qi_equilibrium: 0.0,
            qi_inflow_per_min: 0.0,
        }],
        spatial_revision: 0,
    });
    register(&mut app);
    app.world_mut()
        .resource_mut::<ZoneRuntimeSnapshotState>()
        .last_snapshot_wall = current_unix_seconds();

    app.world_mut().send_event(AppExit::Success);
    app.update();

    let records = load_zone_runtime_snapshot(&settings)
        .expect("production shutdown dispatcher should persist zone runtime");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].zone_id, DEFAULT_SPAWN_ZONE_NAME);
    assert_eq!(records[0].spirit_qi, 0.37);
    assert_eq!(records[0].danger_level, 3);
    assert_eq!(
        app.world()
            .resource::<PersistenceSliceRegistry>()
            .descriptors()
            .map(|descriptor| descriptor.id.as_str())
            .collect::<Vec<_>>(),
        vec!["player.known_techniques", "world.zone_runtime"],
        "production must install every wired production descriptor"
    );

    let _ = fs::remove_dir_all(root);
}

fn production_zone_runtime_registry() -> PersistenceSliceRegistry {
    let mut registry = PersistenceSliceRegistry::empty();
    registry
        .register_slice::<ZoneRuntimePersistenceSlice>()
        .and_then(|()| registry.register_slice::<KnownTechniquesPersistenceSlice>())
        .expect("production slice descriptors must remain valid");
    registry
}

fn dispatch_production_shutdown_flushes(world: &mut World) -> ShutdownFlushReport {
    dispatch_shutdown_flushes(
        world,
        ShutdownFlushRequest::Requested,
        &ProductionSliceClock {
            runtime_tick: 0,
            wall_unix_millis: 0,
        },
    )
    .expect("production shutdown dispatch must not fail closed on a missing registry")
}

fn zone_runtime_failure_message(report: &ShutdownFlushReport) -> Option<&str> {
    report
        .failures
        .iter()
        .find(|failure| failure.slice_id.as_str() == "world.zone_runtime")
        .map(|failure| failure.error.message())
}

#[test]
fn production_zone_runtime_flush_without_registry_is_clean_noop() {
    let (settings, root) = persistence_settings("zone-flush-no-registry");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("fixture database should bootstrap");
    let mut world = World::new();
    world.insert_resource(production_zone_runtime_registry());
    world.insert_resource(settings.clone());
    world.insert_resource(CultivationClock { tick: 3 });
    world.insert_resource(WorldQiAccount::default());
    world.insert_resource(KnownTechniquesActivations::default());
    world.insert_resource(PlayerStatePersistence::with_db_path(
        root.join("data").join("players"),
        settings.db_path(),
    ));

    let report = dispatch_production_shutdown_flushes(&mut world);

    assert_eq!(
        report.attempted, 2,
        "both production descriptors must be attempted, actual {report:?}"
    );
    assert!(
        report.failures.is_empty(),
        "an absent ZoneRegistry must be a clean no-op, actual {report:?}"
    );
    assert_eq!(report.clean, 2, "both slices must report Clean");
    assert!(
        load_zone_runtime_snapshot(&settings)
            .expect("zone snapshot load must work on a fixture database")
            .is_empty(),
        "an absent ZoneRegistry must not write any zone runtime rows"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn production_zone_runtime_flush_without_settings_errors() {
    let (settings, root) = persistence_settings("zone-flush-no-settings");
    let mut world = World::new();
    world.insert_resource(production_zone_runtime_registry());
    world.insert_resource(crate::world::zone::ZoneRegistry::fallback());
    world.insert_resource(CultivationClock { tick: 3 });
    world.insert_resource(WorldQiAccount::default());
    world.insert_resource(KnownTechniquesActivations::default());
    world.insert_resource(PlayerStatePersistence::with_db_path(
        root.join("data").join("players"),
        settings.db_path(),
    ));

    let report = dispatch_production_shutdown_flushes(&mut world);

    assert_eq!(
        zone_runtime_failure_message(&report),
        Some("PersistenceSettings is unavailable"),
        "the zone slice must fail closed without PersistenceSettings, actual {report:?}"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn production_zone_runtime_flush_without_qi_account_errors() {
    let (settings, root) = persistence_settings("zone-flush-no-qi-account");
    let mut world = World::new();
    world.insert_resource(production_zone_runtime_registry());
    world.insert_resource(settings.clone());
    world.insert_resource(crate::world::zone::ZoneRegistry::fallback());
    world.insert_resource(CultivationClock { tick: 3 });
    world.insert_resource(KnownTechniquesActivations::default());
    world.insert_resource(PlayerStatePersistence::with_db_path(
        root.join("data").join("players"),
        settings.db_path(),
    ));

    let report = dispatch_production_shutdown_flushes(&mut world);

    assert_eq!(
        report.failures.len(),
        1,
        "only the zone slice may fail without WorldQiAccount, actual {report:?}"
    );
    assert_eq!(
        zone_runtime_failure_message(&report),
        Some("WorldQiAccount is unavailable"),
        "the zone slice must fail closed without WorldQiAccount"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn production_zone_runtime_flush_without_clock_errors() {
    let (settings, root) = persistence_settings("zone-flush-no-clock");
    let mut world = World::new();
    world.insert_resource(production_zone_runtime_registry());
    world.insert_resource(settings.clone());
    world.insert_resource(crate::world::zone::ZoneRegistry::fallback());
    world.insert_resource(WorldQiAccount::default());
    world.insert_resource(KnownTechniquesActivations::default());
    world.insert_resource(PlayerStatePersistence::with_db_path(
        root.join("data").join("players"),
        settings.db_path(),
    ));

    let report = dispatch_production_shutdown_flushes(&mut world);

    assert_eq!(
        report.failures.len(),
        1,
        "only the zone slice may fail without CultivationClock, actual {report:?}"
    );
    assert_eq!(
        zone_runtime_failure_message(&report),
        Some("CultivationClock is unavailable"),
        "the zone slice must fail closed without CultivationClock"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn production_known_techniques_resource_failure_keeps_later_zone_flush_running() {
    let (settings, root) = persistence_settings("zone-flush-later-descriptor");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("fixture database should bootstrap");
    let mut world = World::new();
    world.insert_resource(production_zone_runtime_registry());
    world.insert_resource(settings.clone());
    world.insert_resource(crate::world::zone::ZoneRegistry::fallback());
    world.insert_resource(CultivationClock { tick: 3 });
    world.insert_resource(WorldQiAccount::default());
    world.insert_resource(KnownTechniquesActivations::default());

    let report = dispatch_production_shutdown_flushes(&mut world);

    assert_eq!(
        report.failures.len(),
        1,
        "only the known-techniques slice may fail without PlayerStatePersistence, actual {report:?}"
    );
    assert_eq!(
        report.failures[0].slice_id.as_str(),
        "player.known_techniques"
    );
    assert!(
        report.flushed >= 1,
        "the zone slice must still flush, actual {report:?}"
    );
    let records = load_zone_runtime_snapshot(&settings)
        .expect("the later zone descriptor must have persisted its snapshot");
    assert!(
        !records.is_empty(),
        "the later zone descriptor must write rows after the earlier known-techniques failure"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn production_shutdown_flush_failing_subject_does_not_abort_later_subject() {
    let (settings, root) = persistence_settings("zone-flush-per-subject-isolation");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("fixture database should bootstrap");
    let player_persistence =
        PlayerStatePersistence::with_db_path(root.join("data").join("players"), settings.db_path());
    let registry = production_zone_runtime_registry();
    let mut world = World::new();

    let failing = known_techniques_fixture("movement.dash", 0.2);
    let mut failing_guard = registry
        .activate_test_subject(
            SliceLoad::<KnownTechniques, String>::loaded(failing),
            KNOWN_TECHNIQUES_SLICE_ID,
            PersistenceSubjectKey::new("npc:hermit"),
            DirtyRevision::default(),
            KnownTechniques::default,
            |_| KnownTechniques::default(),
        )
        .expect("failing subject activation should succeed");
    let (failing_tracker, failing_fence) = failing_guard
        .restore_persistence_state()
        .expect("failing subject tracker should issue once");
    let failing_entity = world
        .spawn(known_techniques_fixture("movement.dash", 0.9))
        .id();

    let later = known_techniques_fixture("movement.dash", 0.3);
    let mut later_guard = registry
        .activate_test_subject(
            SliceLoad::<KnownTechniques, String>::loaded(later),
            KNOWN_TECHNIQUES_SLICE_ID,
            PersistenceSubjectKey::new("offline:Beta"),
            DirtyRevision::default(),
            KnownTechniques::default,
            |_| KnownTechniques::default(),
        )
        .expect("later subject activation should succeed");
    let (later_tracker, later_fence) = later_guard
        .restore_persistence_state()
        .expect("later subject tracker should issue once");
    let later_entity = world
        .spawn(known_techniques_fixture("movement.dash", 0.8))
        .id();

    world.insert_resource(registry);
    world.insert_resource(settings.clone());
    world.insert_resource(player_persistence.clone());
    world.insert_resource(crate::world::zone::ZoneRegistry::fallback());
    world.insert_resource(CultivationClock { tick: 3 });
    world.insert_resource(WorldQiAccount::default());
    world.insert_resource(KnownTechniquesActivations(HashMap::from([
        (
            "npc:hermit".to_string(),
            KnownTechniquesActivation {
                entity: failing_entity,
                guarded: failing_guard,
                tracker: failing_tracker,
                fence: failing_fence,
            },
        ),
        (
            canonical_player_id("Beta"),
            KnownTechniquesActivation {
                entity: later_entity,
                guarded: later_guard,
                tracker: later_tracker,
                fence: later_fence,
            },
        ),
    ])));

    let report = dispatch_production_shutdown_flushes(&mut world);

    assert_eq!(
        report.failures.len(),
        1,
        "only the failing subject may report, actual {report:?}"
    );
    assert_eq!(
        report.failures[0].slice_id.as_str(),
        "player.known_techniques"
    );
    assert!(
        report.failures[0].error.message().contains("npc:hermit"),
        "the failure must identify the failing subject, actual {:?}",
        report.failures[0].error.message()
    );
    assert_eq!(
        load_player_known_techniques_slice(&player_persistence, "Beta")
            .expect("later subject row should decode"),
        Some(known_techniques_fixture("movement.dash", 0.8)),
        "the later subject must still flush after the earlier subject failed"
    );
    assert!(
        load_player_known_techniques_slice(&player_persistence, "hermit")
            .expect("failing subject lookup should decode")
            .is_none(),
        "the failing subject must not write any row"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn persist_termination_transition_stores_complete_deceased_snapshot_in_sqlite() {
    let (settings, root) = persistence_settings("deceased-snapshot-sqlite");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");
    let tags_json = serde_json::to_string(&vec![RenownTagV1 {
        tag: "三叛之人".to_string(),
        weight: 20.0,
        last_seen_tick: 70,
        permanent: true,
    }])
    .expect("renown tags should serialize");
    Connection::open(settings.db_path())
        .expect("db should open")
        .execute(
            "
            INSERT INTO social_renown (
                char_id, fame, notoriety, tags_json, schema_version, last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, 1, 1)
            ",
            params!["offline:Ancestor", 12, 80, tags_json],
        )
        .expect("renown row should insert");

    let life_record = LifeRecord {
        character_id: "offline:Ancestor".to_string(),
        created_at: 11,
        biography: vec![BiographyEntry::Terminated {
            cause: "natural_end".to_string(),
            tick: 77,
        }],
        skill_milestones: vec![crate::cultivation::life_record::SkillMilestone {
            skill: crate::skill::components::SkillId::Alchemy,
            new_lv: 4,
            achieved_at: 75,
            narration: "丹火三转，炉意已成。".to_string(),
            total_xp_at: 1_280,
        }],
        ..LifeRecord::default()
    };
    let lifecycle = Lifecycle {
        character_id: life_record.character_id.clone(),
        death_count: 3,
        fortune_remaining: 0,
        last_death_tick: Some(77),
        state: LifecycleState::Terminated,
        ..Lifecycle::default()
    };

    persist_termination_transition(&settings, &lifecycle, &life_record)
        .expect("terminated snapshot should persist");

    let (snapshot_json, died_at_tick): (String, i64) = Connection::open(settings.db_path())
        .expect("db should reopen")
        .query_row(
            "SELECT snapshot_json, died_at_tick FROM deceased_snapshots WHERE char_id = ?1",
            params!["offline:Ancestor"],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("deceased snapshot row should exist");
    let snapshot: DeceasedSnapshot =
        serde_json::from_str(&snapshot_json).expect("snapshot should decode");

    assert_eq!(died_at_tick, 77);
    assert_eq!(snapshot.char_id, "offline:Ancestor");
    assert_eq!(snapshot.termination_category, "善终");
    assert_eq!(snapshot.lifecycle.character_id, lifecycle.character_id);
    assert_eq!(snapshot.lifecycle.death_count, 3);
    assert_eq!(snapshot.lifecycle.fortune_remaining, 0);
    assert_eq!(snapshot.lifecycle.last_death_tick, Some(77));
    assert_eq!(snapshot.lifecycle.state, LifecycleState::Terminated);
    assert_eq!(snapshot.life_record.character_id, life_record.character_id);
    assert_eq!(snapshot.life_record.created_at, life_record.created_at);
    assert!(matches!(
        snapshot.life_record.biography.last(),
        Some(BiographyEntry::Terminated { tick: 77, .. })
    ));
    assert_eq!(snapshot.life_record.skill_milestones.len(), 1);
    assert_eq!(
        snapshot.life_record.skill_milestones[0].skill,
        crate::skill::components::SkillId::Alchemy
    );
    assert_eq!(snapshot.life_record.skill_milestones[0].new_lv, 4);
    assert_eq!(snapshot.life_record.skill_milestones[0].achieved_at, 75);
    assert_eq!(
        snapshot.life_record.skill_milestones[0].narration,
        "丹火三转，炉意已成。"
    );
    assert_eq!(snapshot.life_record.skill_milestones[0].total_xp_at, 1_280);
    let social = snapshot.social.expect("social snapshot should persist");
    assert_eq!(social.renown.fame, 12);
    assert_eq!(social.renown.notoriety, 80);
    assert_eq!(social.renown.tags[0].tag, "三叛之人");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn persist_termination_transition_persists_complete_social_snapshot_in_sqlite() {
    let (settings, root) = persistence_settings("deceased-snapshot-social-sqlite");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let connection = Connection::open(settings.db_path()).expect("db should open");
    let tags_json = serde_json::to_string(&vec![RenownTagV1 {
        tag: "三叛之人".to_string(),
        weight: 20.0,
        last_seen_tick: 70,
        permanent: true,
    }])
    .expect("renown tags should serialize");
    connection
        .execute(
            "
            INSERT INTO social_renown (
                char_id, fame, notoriety, tags_json, schema_version, last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, 1, 1)
            ",
            params!["offline:Social", 12, 80, tags_json],
        )
        .expect("renown row should insert");
    connection
        .execute(
            "
            INSERT INTO social_relationships (
                char_id, peer_char_id, relationship_type, since_tick, metadata_json,
                schema_version, last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, 1, 1)
            ",
            params![
                "offline:Social",
                "char:rival",
                "feud",
                33,
                r#"{"cause":"ambush"}"#
            ],
        )
        .expect("relationship row should insert");
    let witnesses_json = serde_json::to_string(&vec!["char:killer", "char:witness"])
        .expect("witnesses should serialize");
    connection
        .execute(
            "
            INSERT INTO social_exposures (
                event_id, char_id, kind, witnesses_json, at_tick, schema_version, last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, 1, 1)
            ",
            params![
                "exposure-death-social",
                "offline:Social",
                "death",
                witnesses_json,
                77
            ],
        )
        .expect("exposure row should insert");
    connection
        .execute(
            "
            INSERT INTO social_faction_memberships (
                char_id, faction, rank, loyalty, betrayal_count, invite_block_until_tick,
                permanently_refused, schema_version, last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, 1)
            ",
            params!["offline:Social", "attack", 2, -10, 3, 88, 1],
        )
        .expect("faction membership row should insert");
    drop(connection);

    let life_record = LifeRecord {
        character_id: "offline:Social".to_string(),
        created_at: 11,
        biography: vec![BiographyEntry::Terminated {
            cause: "fortune_exhausted".to_string(),
            tick: 77,
        }],
        ..LifeRecord::default()
    };
    let lifecycle = Lifecycle {
        character_id: life_record.character_id.clone(),
        death_count: 3,
        fortune_remaining: 0,
        last_death_tick: Some(77),
        state: LifecycleState::Terminated,
        ..Lifecycle::default()
    };

    persist_termination_transition(&settings, &lifecycle, &life_record)
        .expect("terminated snapshot should persist");

    let snapshot_json: String = Connection::open(settings.db_path())
        .expect("db should reopen")
        .query_row(
            "SELECT snapshot_json FROM deceased_snapshots WHERE char_id = ?1",
            params!["offline:Social"],
            |row| row.get(0),
        )
        .expect("deceased snapshot row should exist");
    let snapshot: DeceasedSnapshot =
        serde_json::from_str(&snapshot_json).expect("snapshot should decode");
    let social = snapshot.social.expect("social snapshot should persist");
    assert_eq!(social.renown.fame, 12);
    assert_eq!(social.renown.notoriety, 80);
    assert_eq!(social.renown.tags[0].tag, "三叛之人");
    assert_eq!(social.relationships.len(), 1);
    assert_eq!(social.relationships[0].kind, RelationshipKindV1::Feud);
    assert_eq!(social.relationships[0].peer, "char:rival");
    assert_eq!(social.relationships[0].since_tick, 33);
    assert_eq!(social.relationships[0].metadata["cause"], "ambush");
    assert_eq!(social.exposure_log.len(), 1);
    assert_eq!(social.exposure_log[0].kind, ExposureKindV1::Death);
    assert_eq!(social.exposure_log[0].tick, 77);
    assert_eq!(social.exposure_log[0].witnesses.len(), 2);
    let membership = social
        .faction_membership
        .expect("faction membership should persist");
    assert_eq!(membership.faction, "attack");
    assert_eq!(membership.rank, 2);
    assert_eq!(membership.loyalty, -10);
    assert_eq!(membership.betrayal_count, 3);
    assert_eq!(membership.invite_block_until_tick, Some(88));
    assert!(membership.permanently_refused);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn persist_termination_transition_updates_sqlite_snapshot_and_categories() {
    let (settings, root) = persistence_settings("deceased-snapshot-categories");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    for (char_id, cause, expected_category, tick) in [
        ("offline:OldOne", "natural_end", "善终", 88_u64),
        ("offline:Hermit", "voluntary_retire", "自主归隐", 89_u64),
    ] {
        let life_record = LifeRecord {
            character_id: char_id.to_string(),
            created_at: 11,
            biography: vec![BiographyEntry::Terminated {
                cause: cause.to_string(),
                tick,
            }],
            ..LifeRecord::default()
        };
        let lifecycle = Lifecycle {
            character_id: char_id.to_string(),
            death_count: 1,
            fortune_remaining: 0,
            last_death_tick: Some(tick),
            state: LifecycleState::Terminated,
            ..Lifecycle::default()
        };

        persist_termination_transition(&settings, &lifecycle, &life_record)
            .expect("terminated snapshot should persist");
        let snapshot_json: String = Connection::open(settings.db_path())
            .expect("db should reopen")
            .query_row(
                "SELECT snapshot_json FROM deceased_snapshots WHERE char_id = ?1",
                params![char_id],
                |row| row.get(0),
            )
            .expect("deceased snapshot row should exist");
        let snapshot: DeceasedSnapshot =
            serde_json::from_str(&snapshot_json).expect("snapshot should decode");
        assert_eq!(snapshot.termination_category, expected_category);
        assert_eq!(snapshot.died_at_tick, tick);
    }

    let (died_at_tick, snapshot_json): (i64, String) = Connection::open(settings.db_path())
        .expect("db should reopen for overwrite")
        .query_row(
            "SELECT died_at_tick, snapshot_json FROM deceased_snapshots WHERE char_id = ?1",
            params!["offline:Hermit"],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("hermit snapshot row should exist");
    let snapshot: DeceasedSnapshot =
        serde_json::from_str(&snapshot_json).expect("overwritten snapshot should decode");
    assert_eq!(died_at_tick, 89);
    assert_eq!(snapshot.char_id, "offline:Hermit");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn semantic_death_peak_keeps_sqlite_life_registry_and_snapshots_consistent() {
    let (settings, root) = persistence_settings("semantic-death-peak-sqlite");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let writer_count = 10usize;
    let settings = Arc::new(settings);
    let barrier = Arc::new(Barrier::new(writer_count + 1));
    let handles = (0..writer_count)
        .map(|index| {
            let settings = Arc::clone(&settings);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                let char_id = format!("offline:PeakDeath{index}");
                let tick = 2_000 + index as u64;
                let life_record = LifeRecord {
                    character_id: char_id.clone(),
                    created_at: tick.saturating_sub(100),
                    biography: vec![BiographyEntry::Terminated {
                        cause: "peak_death".to_string(),
                        tick,
                    }],
                    ..LifeRecord::default()
                };
                let lifecycle = Lifecycle {
                    character_id: char_id,
                    death_count: 1,
                    fortune_remaining: 0,
                    last_death_tick: Some(tick),
                    state: LifecycleState::Terminated,
                    ..Lifecycle::default()
                };
                let lifespan_event = LifespanEventRecord {
                    at_tick: tick,
                    kind: "termination".to_string(),
                    delta_years: -999,
                    source: "peak_death".to_string(),
                };

                barrier.wait();
                persist_termination_transition_with_death_context(
                    settings.as_ref(),
                    &lifecycle,
                    &life_record,
                    Some("peak_death"),
                    Some(&lifespan_event),
                )
            })
        })
        .collect::<Vec<_>>();

    barrier.wait();
    let errors = handles
        .into_iter()
        .map(|handle| handle.join().expect("death writer should not panic"))
        .filter_map(Result::err)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(
        errors.is_empty(),
        "concurrent termination events should persist atomically: {errors:?}"
    );

    let connection = Connection::open(settings.db_path()).expect("db should open");
    for (table, expected) in [
        ("life_records", writer_count as i64),
        ("life_events", writer_count as i64),
        ("death_registry", writer_count as i64),
        ("lifespan_events", writer_count as i64),
        ("deceased_snapshots", writer_count as i64),
    ] {
        let sql = format!("SELECT COUNT(*) FROM {table}");
        let count: i64 = connection
            .query_row(sql.as_str(), [], |row| row.get(0))
            .expect("table count should be readable");
        assert_eq!(count, expected, "{table} should contain all peak rows");
    }
    for index in 0..writer_count {
        let char_id = format!("offline:PeakDeath{index}");
        let snapshot_json: String = connection
            .query_row(
                "SELECT snapshot_json FROM deceased_snapshots WHERE char_id = ?1",
                params![char_id],
                |row| row.get(0),
            )
            .expect("peak snapshot should exist");
        let snapshot: DeceasedSnapshot =
            serde_json::from_str(&snapshot_json).expect("peak snapshot should decode");
        assert_eq!(snapshot.died_at_tick, 2_000 + index as u64);
        assert_eq!(snapshot.termination_category, "横死");
    }

    let _ = fs::remove_dir_all(root);
}

fn sample_npc_life_record(char_id: &str) -> LifeRecord {
    LifeRecord {
        character_id: char_id.to_string(),
        created_at: 12,
        biography: vec![
            BiographyEntry::CombatHit {
                attacker_id: "offline:Azure".to_string(),
                body_part: "Chest".to_string(),
                wound_kind: "Cut".to_string(),
                damage: 12.5,
                tick: 41,
            },
            BiographyEntry::NearDeath {
                cause: "duel".to_string(),
                tick: 77,
            },
        ],
        insights_taken: Vec::new(),
        death_insights: Vec::new(),
        skill_milestones: Vec::new(),
        spirit_root_first: None,
        ..LifeRecord::default()
    }
}

fn sample_npc_capture(char_id: &str) -> NpcPersistenceCapture {
    let mut app = App::new();
    let entity = app.world_mut().spawn_empty().id();
    let mut movement = MovementController::new();
    movement.mode = MovementMode::Sprinting(SprintState {
        multiplier: 2.2,
        remaining_ticks: 18,
    });
    let life_record = sample_npc_life_record(char_id);
    let capture = capture_npc_persistence(
        entity,
        &Position::new([14.0, 66.0, 9.0]),
        EntityKind::ZOMBIE,
        NpcStateKind::Attacking,
        &NpcBlackboard {
            nearest_player: None,
            player_distance: 6.5,
            target_position: Some(DVec3::new(8.0, 66.0, 8.0)),
            last_melee_tick: 77,
            threat_assessment: None,
            self_interest_decision: None,
            retaliation_target: None,
            decoy_target: None,
        },
        Some("offline:Azure"),
        &NpcCombatLoadout::fighter(NpcMeleeArchetype::Sword),
        &NpcPatrol::new(DEFAULT_SPAWN_ZONE_NAME, DVec3::new(12.0, 66.0, 12.0)),
        &movement,
        &MovementCooldowns {
            sprint_ready_at: 5,
            dash_ready_at: 33,
        },
        &Lifecycle {
            character_id: char_id.to_string(),
            death_count: 1,
            fortune_remaining: 2,
            last_death_tick: Some(55),
            last_revive_tick: Some(66),
            spawn_anchor: None,
            spawn_anchor_damaged: false,
            near_death_deadline_tick: None,
            awaiting_decision: None,
            revival_decision_deadline_tick: None,
            weakened_until_tick: None,
            state: LifecycleState::Alive,
        },
        Some(&Cultivation {
            realm: Realm::Spirit,
            ..Default::default()
        }),
        Some(&life_record),
    );

    NpcPersistenceCapture {
        captured_at_wall: 1_704_067_200,
        digest: NpcDigestRecord {
            last_referenced_wall: 1_704_067_200,
            ..capture.digest
        },
        ..capture
    }
}

#[derive(Debug, Default)]
struct WriteBatchMetrics {
    writes: usize,
    total_write_ms: u128,
    max_write_ms: u128,
    errors: Vec<String>,
}

impl WriteBatchMetrics {
    fn record(&mut self, started_at: Instant, result: io::Result<()>) {
        let write_ms = started_at.elapsed().as_millis();
        self.writes += 1;
        self.total_write_ms += write_ms;
        self.max_write_ms = self.max_write_ms.max(write_ms);
        if let Err(error) = result {
            self.errors.push(error.to_string());
        }
    }

    fn merge(mut self, other: Self) -> Self {
        self.writes += other.writes;
        self.total_write_ms += other.total_write_ms;
        self.max_write_ms = self.max_write_ms.max(other.max_write_ms);
        self.errors.extend(other.errors);
        self
    }
}

#[test]
fn phase9_throttled_write_regression_handles_1000_npc_and_50_players() {
    let (settings, root) = persistence_settings("phase9-throttled-write-regression");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let player_persistence =
        PlayerStatePersistence::with_db_path(root.join("data").join("players"), settings.db_path());
    let player_count = 50usize;
    let npc_count = 1_000usize;
    for index in 0..player_count {
        save_player_state(
            &player_persistence,
            format!("PerfPlayer{index}").as_str(),
            &PlayerState {
                karma: 0.0,
                inventory_score: 0.0,
            },
        )
        .expect("seed player state should persist");
    }

    let npc_captures = (0..npc_count)
        .map(|index| sample_npc_capture(format!("npc_perf_{index}").as_str()))
        .collect::<Vec<_>>();
    let settings = Arc::new(settings);
    let player_persistence = Arc::new(player_persistence);
    let npc_captures = Arc::new(npc_captures);
    let npc_worker_count = 16usize;
    let player_worker_count = 4usize;
    let barrier = Arc::new(Barrier::new(npc_worker_count + player_worker_count + 1));

    let npc_handles = (0..npc_worker_count)
        .map(|worker| {
            let settings = Arc::clone(&settings);
            let captures = Arc::clone(&npc_captures);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                let start = worker * npc_count / npc_worker_count;
                let end = (worker + 1) * npc_count / npc_worker_count;
                let mut metrics = WriteBatchMetrics::default();
                barrier.wait();
                for index in start..end {
                    let started_at = Instant::now();
                    metrics.record(
                        started_at,
                        persist_npc_capture(settings.as_ref(), &captures[index]),
                    );
                }
                metrics
            })
        })
        .collect::<Vec<_>>();

    let player_handles = (0..player_worker_count)
        .map(|worker| {
            let persistence = Arc::clone(&player_persistence);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                let start = worker * player_count / player_worker_count;
                let end = (worker + 1) * player_count / player_worker_count;
                let mut metrics = WriteBatchMetrics::default();
                barrier.wait();
                for index in start..end {
                    let username = format!("PerfPlayer{index}");
                    let state = PlayerState {
                        karma: ((index as f64 / player_count as f64) * 2.0 - 1.0).clamp(-1.0, 1.0),
                        inventory_score: (index as f64 / player_count as f64).clamp(0.0, 1.0),
                    };
                    let started_at = Instant::now();
                    metrics.record(
                        started_at,
                        save_player_core_slice(persistence.as_ref(), username.as_str(), &state)
                            .map(|_| ()),
                    );
                }
                metrics
            })
        })
        .collect::<Vec<_>>();

    let batch_started = Instant::now();
    barrier.wait();
    let metrics = npc_handles
        .into_iter()
        .map(|handle| handle.join().expect("npc worker should not panic"))
        .chain(
            player_handles
                .into_iter()
                .map(|handle| handle.join().expect("player worker should not panic")),
        )
        .fold(WriteBatchMetrics::default(), WriteBatchMetrics::merge);
    let elapsed = batch_started.elapsed();
    let lock_failures = metrics
        .errors
        .iter()
        .filter(|error| error.contains("locked") || error.contains("busy"))
        .count();
    let failure_rate = metrics.errors.len() as f64 / metrics.writes as f64;
    eprintln!(
        "[phase9] sqlite throttled write regression: writes={} elapsed_ms={} total_write_ms={} max_write_ms={} lock_failures={} failure_rate={:.4}",
        metrics.writes,
        elapsed.as_millis(),
        metrics.total_write_ms,
        metrics.max_write_ms,
        lock_failures,
        failure_rate
    );

    assert_eq!(metrics.writes, npc_count + player_count);
    assert!(
        metrics.errors.is_empty(),
        "1000 NPC + 50 player throttled writes should not fail; lock_failures={lock_failures}, errors={:?}",
        metrics.errors
    );
    assert!(
        elapsed.as_secs() < 60,
        "1000 NPC + 50 player throttled writes should remain inside the 60s regression envelope; elapsed={elapsed:?}"
    );

    let connection = Connection::open(settings.db_path()).expect("db should open");
    let npc_state_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM npc_state", [], |row| row.get(0))
        .expect("npc_state count should be readable");
    let player_count_actual: i64 = connection
        .query_row("SELECT COUNT(*) FROM player_core", [], |row| row.get(0))
        .expect("player_core count should be readable");
    assert_eq!(npc_state_count, npc_count as i64);
    assert_eq!(player_count_actual, player_count as i64);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn semantic_event_writers_serialize_under_wal_busy_timeout() {
    let (settings, root) = persistence_settings("near-death-concurrency");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let settings = Arc::new(settings);
    let writer_count = 10usize;
    let barrier = Arc::new(Barrier::new(writer_count + 1));
    let handles = (0..writer_count)
        .map(|index| {
            let settings = Arc::clone(&settings);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                let char_id = format!("offline:Conflict{index}");
                let tick = 100 + index as u64;
                let life_record = LifeRecord {
                    character_id: char_id.clone(),
                    created_at: tick.saturating_sub(10),
                    biography: vec![BiographyEntry::NearDeath {
                        cause: format!("duel-{index}"),
                        tick,
                    }],
                    insights_taken: Vec::new(),
                    death_insights: Vec::new(),
                    skill_milestones: Vec::new(),
                    spirit_root_first: None,
                    ..LifeRecord::default()
                };
                let lifecycle = Lifecycle {
                    character_id: char_id.clone(),
                    death_count: 1,
                    fortune_remaining: 1,
                    last_death_tick: Some(tick),
                    last_revive_tick: Some(tick.saturating_sub(1)),
                    spawn_anchor: None,
                    spawn_anchor_damaged: false,
                    near_death_deadline_tick: Some(tick + 30),
                    awaiting_decision: None,
                    revival_decision_deadline_tick: None,
                    weakened_until_tick: Some(tick + 5),
                    state: LifecycleState::NearDeath,
                };
                let lifespan_event = LifespanEventRecord {
                    at_tick: tick,
                    kind: "near_death".to_string(),
                    delta_years: -1,
                    source: format!("duel-{index}"),
                };

                barrier.wait();
                persist_near_death_transition(
                    settings.as_ref(),
                    &lifecycle,
                    &life_record,
                    "duel",
                    Some(&lifespan_event),
                )
            })
        })
        .collect::<Vec<_>>();

    barrier.wait();
    let results = handles
        .into_iter()
        .map(|handle| handle.join().expect("writer thread should not panic"))
        .collect::<Vec<_>>();
    let errors = results
        .into_iter()
        .filter_map(Result::err)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(
        errors.is_empty(),
        "all concurrent semantic-event writers should succeed: {errors:?}"
    );

    let connection = Connection::open(settings.db_path()).expect("db should open");
    let life_records: i64 = connection
        .query_row("SELECT COUNT(*) FROM life_records", [], |row| row.get(0))
        .expect("life_records count should be readable");
    let life_events: i64 = connection
        .query_row("SELECT COUNT(*) FROM life_events", [], |row| row.get(0))
        .expect("life_events count should be readable");
    let death_registry: i64 = connection
        .query_row("SELECT COUNT(*) FROM death_registry", [], |row| row.get(0))
        .expect("death_registry count should be readable");
    let lifespan_events: i64 = connection
        .query_row("SELECT COUNT(*) FROM lifespan_events", [], |row| row.get(0))
        .expect("lifespan_events count should be readable");

    assert_eq!(life_records, writer_count as i64);
    assert_eq!(life_events, writer_count as i64);
    assert_eq!(death_registry, writer_count as i64);
    assert_eq!(lifespan_events, writer_count as i64);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn mixed_player_core_and_semantic_event_writers_share_sqlite_without_lock_failures() {
    let (settings, root) = persistence_settings("mixed-core-near-death");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let player_persistence =
        PlayerStatePersistence::with_db_path(root.join("data").join("players"), settings.db_path());
    let player_seed = PlayerState {
        karma: 0.1,
        inventory_score: 0.2,
    };
    let player_writer_count = 10usize;
    let semantic_writer_count = 10usize;

    for index in 0..player_writer_count {
        save_player_state(
            &player_persistence,
            format!("MixedPlayer{index}").as_str(),
            &player_seed,
        )
        .expect("seed player state should persist");
    }

    let settings = Arc::new(settings);
    let player_persistence = Arc::new(player_persistence);
    let barrier = Arc::new(Barrier::new(
        player_writer_count + semantic_writer_count + 1,
    ));

    let player_handles = (0..player_writer_count)
        .map(|index| {
            let persistence = Arc::clone(&player_persistence);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                let username = format!("MixedPlayer{index}");
                let updated_state = PlayerState {
                    karma: ((index as f64 / 5.0) - 1.0).clamp(-1.0, 1.0),
                    inventory_score: (index as f64 / player_writer_count as f64).clamp(0.0, 1.0),
                };

                barrier.wait();
                save_player_core_slice(persistence.as_ref(), username.as_str(), &updated_state)
            })
        })
        .collect::<Vec<_>>();

    let semantic_handles = (0..semantic_writer_count)
        .map(|index| {
            let settings = Arc::clone(&settings);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                let char_id = format!("offline:MixedConflict{index}");
                let tick = 500 + index as u64;
                let life_record = LifeRecord {
                    character_id: char_id.clone(),
                    created_at: tick.saturating_sub(20),
                    biography: vec![BiographyEntry::NearDeath {
                        cause: format!("mixed-duel-{index}"),
                        tick,
                    }],
                    insights_taken: Vec::new(),
                    death_insights: Vec::new(),
                    skill_milestones: Vec::new(),
                    spirit_root_first: None,
                    ..LifeRecord::default()
                };
                let lifecycle = Lifecycle {
                    character_id: char_id.clone(),
                    death_count: 1,
                    fortune_remaining: 1,
                    last_death_tick: Some(tick),
                    last_revive_tick: Some(tick.saturating_sub(1)),
                    spawn_anchor: None,
                    spawn_anchor_damaged: false,
                    near_death_deadline_tick: Some(tick + 30),
                    awaiting_decision: None,
                    revival_decision_deadline_tick: None,
                    weakened_until_tick: Some(tick + 5),
                    state: LifecycleState::NearDeath,
                };
                let lifespan_event = LifespanEventRecord {
                    at_tick: tick,
                    kind: "near_death".to_string(),
                    delta_years: -1,
                    source: format!("mixed-duel-{index}"),
                };

                barrier.wait();
                persist_near_death_transition(
                    settings.as_ref(),
                    &lifecycle,
                    &life_record,
                    "mixed-duel",
                    Some(&lifespan_event),
                )
            })
        })
        .collect::<Vec<_>>();

    barrier.wait();
    let errors = player_handles
        .into_iter()
        .map(|handle| {
            handle
                .join()
                .expect("player writer should not panic")
                .map(|_| ())
        })
        .chain(
            semantic_handles
                .into_iter()
                .map(|handle| handle.join().expect("semantic writer should not panic")),
        )
        .filter_map(Result::err)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(
        errors.is_empty(),
        "mixed player core and semantic writers should all succeed: {errors:?}"
    );

    let connection = Connection::open(settings.db_path()).expect("db should open");
    let life_records: i64 = connection
        .query_row("SELECT COUNT(*) FROM life_records", [], |row| row.get(0))
        .expect("life_records count should be readable");
    let life_events: i64 = connection
        .query_row("SELECT COUNT(*) FROM life_events", [], |row| row.get(0))
        .expect("life_events count should be readable");
    let death_registry: i64 = connection
        .query_row("SELECT COUNT(*) FROM death_registry", [], |row| row.get(0))
        .expect("death_registry count should be readable");
    let lifespan_events: i64 = connection
        .query_row("SELECT COUNT(*) FROM lifespan_events", [], |row| row.get(0))
        .expect("lifespan_events count should be readable");

    assert_eq!(life_records, semantic_writer_count as i64);
    assert_eq!(life_events, semantic_writer_count as i64);
    assert_eq!(death_registry, semantic_writer_count as i64);
    assert_eq!(lifespan_events, semantic_writer_count as i64);

    for index in 0..player_writer_count {
        let username = format!("MixedPlayer{index}");
        let (karma, inventory_score): (f64, f64) = connection
            .query_row(
                "SELECT karma, inventory_score FROM player_core WHERE username = ?1",
                params![username.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("player core row should exist after mixed load");
        assert_eq!(karma, ((index as f64 / 5.0) - 1.0).clamp(-1.0, 1.0));
        assert_eq!(
            inventory_score,
            (index as f64 / player_writer_count as f64).clamp(0.0, 1.0)
        );
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn mixed_player_semantic_and_npc_writers_share_sqlite_without_lock_failures() {
    let (settings, root) = persistence_settings("mixed-player-semantic-npc");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let player_persistence =
        PlayerStatePersistence::with_db_path(root.join("data").join("players"), settings.db_path());
    let player_seed = PlayerState {
        karma: 0.1,
        inventory_score: 0.2,
    };
    let player_writer_count = 10usize;
    let semantic_writer_count = 10usize;
    let npc_writer_count = 10usize;

    for index in 0..player_writer_count {
        save_player_state(
            &player_persistence,
            format!("MixedNpcPlayer{index}").as_str(),
            &player_seed,
        )
        .expect("seed player state should persist");
    }

    let settings = Arc::new(settings);
    let player_persistence = Arc::new(player_persistence);
    let barrier = Arc::new(Barrier::new(
        player_writer_count + semantic_writer_count + npc_writer_count + 1,
    ));

    let player_handles = (0..player_writer_count)
        .map(|index| {
            let persistence = Arc::clone(&player_persistence);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                let username = format!("MixedNpcPlayer{index}");
                let updated_state = PlayerState {
                    karma: ((index as f64 / 5.0) - 1.0).clamp(-1.0, 1.0),
                    inventory_score: (index as f64 / player_writer_count as f64).clamp(0.0, 1.0),
                };

                barrier.wait();
                save_player_core_slice(persistence.as_ref(), username.as_str(), &updated_state)
                    .map(|_| ())
            })
        })
        .collect::<Vec<_>>();

    let semantic_handles = (0..semantic_writer_count)
        .map(|index| {
            let settings = Arc::clone(&settings);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                let char_id = format!("offline:MixedNpcConflict{index}");
                let tick = 700 + index as u64;
                let life_record = LifeRecord {
                    character_id: char_id.clone(),
                    created_at: tick.saturating_sub(20),
                    biography: vec![BiographyEntry::NearDeath {
                        cause: format!("mixed-npc-duel-{index}"),
                        tick,
                    }],
                    insights_taken: Vec::new(),
                    death_insights: Vec::new(),
                    skill_milestones: Vec::new(),
                    spirit_root_first: None,
                    ..LifeRecord::default()
                };
                let lifecycle = Lifecycle {
                    character_id: char_id.clone(),
                    death_count: 1,
                    fortune_remaining: 1,
                    last_death_tick: Some(tick),
                    last_revive_tick: Some(tick.saturating_sub(1)),
                    spawn_anchor: None,
                    spawn_anchor_damaged: false,
                    near_death_deadline_tick: Some(tick + 30),
                    awaiting_decision: None,
                    revival_decision_deadline_tick: None,
                    weakened_until_tick: Some(tick + 5),
                    state: LifecycleState::NearDeath,
                };
                let lifespan_event = LifespanEventRecord {
                    at_tick: tick,
                    kind: "near_death".to_string(),
                    delta_years: -1,
                    source: format!("mixed-npc-duel-{index}"),
                };

                barrier.wait();
                persist_near_death_transition(
                    settings.as_ref(),
                    &lifecycle,
                    &life_record,
                    "mixed-npc-duel",
                    Some(&lifespan_event),
                )
            })
        })
        .collect::<Vec<_>>();

    let npc_handles = (0..npc_writer_count)
        .map(|index| {
            let settings = Arc::clone(&settings);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                let capture = sample_npc_capture(format!("npc_mixed_{index}").as_str());
                barrier.wait();
                persist_npc_capture(settings.as_ref(), &capture)
            })
        })
        .collect::<Vec<_>>();

    barrier.wait();
    let errors = player_handles
        .into_iter()
        .map(|handle| handle.join().expect("player writer should not panic"))
        .chain(
            semantic_handles
                .into_iter()
                .map(|handle| handle.join().expect("semantic writer should not panic")),
        )
        .chain(
            npc_handles
                .into_iter()
                .map(|handle| handle.join().expect("npc writer should not panic")),
        )
        .filter_map(Result::err)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(
        errors.is_empty(),
        "mixed player, semantic, and npc writers should all succeed: {errors:?}"
    );

    let connection = Connection::open(settings.db_path()).expect("db should open");
    let life_records: i64 = connection
        .query_row("SELECT COUNT(*) FROM life_records", [], |row| row.get(0))
        .expect("life_records count should be readable");
    let life_events: i64 = connection
        .query_row("SELECT COUNT(*) FROM life_events", [], |row| row.get(0))
        .expect("life_events count should be readable");
    let death_registry: i64 = connection
        .query_row("SELECT COUNT(*) FROM death_registry", [], |row| row.get(0))
        .expect("death_registry count should be readable");
    let lifespan_events: i64 = connection
        .query_row("SELECT COUNT(*) FROM lifespan_events", [], |row| row.get(0))
        .expect("lifespan_events count should be readable");
    let npc_state_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM npc_state", [], |row| row.get(0))
        .expect("npc_state count should be readable");
    let npc_digest_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM npc_digests", [], |row| row.get(0))
        .expect("npc_digests count should be readable");
    let archetype_registry_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM archetype_registry", [], |row| {
            row.get(0)
        })
        .expect("archetype_registry count should be readable");

    assert_eq!(life_records, semantic_writer_count as i64);
    assert_eq!(life_events, semantic_writer_count as i64);
    assert_eq!(death_registry, semantic_writer_count as i64);
    assert_eq!(lifespan_events, semantic_writer_count as i64);
    assert_eq!(npc_state_count, npc_writer_count as i64);
    assert_eq!(npc_digest_count, npc_writer_count as i64);
    assert_eq!(archetype_registry_count, npc_writer_count as i64);

    for index in 0..player_writer_count {
        let username = format!("MixedNpcPlayer{index}");
        let (karma, inventory_score): (f64, f64) = connection
            .query_row(
                "SELECT karma, inventory_score FROM player_core WHERE username = ?1",
                params![username.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("player core row should exist after mixed npc load");
        assert_eq!(karma, ((index as f64 / 5.0) - 1.0).clamp(-1.0, 1.0));
        assert_eq!(
            inventory_score,
            (index as f64 / player_writer_count as f64).clamp(0.0, 1.0)
        );
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn mixed_player_semantic_npc_and_zone_runtime_writers_share_sqlite_without_lock_failures() {
    let (settings, root) = persistence_settings("mixed-player-semantic-npc-zone");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let player_persistence =
        PlayerStatePersistence::with_db_path(root.join("data").join("players"), settings.db_path());
    let player_seed = PlayerState {
        karma: 0.1,
        inventory_score: 0.2,
    };
    let player_writer_count = 10usize;
    let semantic_writer_count = 10usize;
    let npc_writer_count = 10usize;
    let zone_writer_count = 5usize;

    for index in 0..player_writer_count {
        save_player_state(
            &player_persistence,
            format!("MixedZonePlayer{index}").as_str(),
            &player_seed,
        )
        .expect("seed player state should persist");
    }

    let settings = Arc::new(settings);
    let player_persistence = Arc::new(player_persistence);
    let barrier = Arc::new(Barrier::new(
        player_writer_count + semantic_writer_count + npc_writer_count + zone_writer_count + 1,
    ));

    let player_handles = (0..player_writer_count)
        .map(|index| {
            let persistence = Arc::clone(&player_persistence);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                let username = format!("MixedZonePlayer{index}");
                let updated_state = PlayerState {
                    karma: ((index as f64 / 5.0) - 1.0).clamp(-1.0, 1.0),
                    inventory_score: (index as f64 / player_writer_count as f64).clamp(0.0, 1.0),
                };

                barrier.wait();
                save_player_core_slice(persistence.as_ref(), username.as_str(), &updated_state)
                    .map(|_| ())
            })
        })
        .collect::<Vec<_>>();

    let semantic_handles = (0..semantic_writer_count)
        .map(|index| {
            let settings = Arc::clone(&settings);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                let char_id = format!("offline:MixedZoneConflict{index}");
                let tick = 900 + index as u64;
                let life_record = LifeRecord {
                    character_id: char_id.clone(),
                    created_at: tick.saturating_sub(20),
                    biography: vec![BiographyEntry::NearDeath {
                        cause: format!("mixed-zone-duel-{index}"),
                        tick,
                    }],
                    insights_taken: Vec::new(),
                    death_insights: Vec::new(),
                    skill_milestones: Vec::new(),
                    spirit_root_first: None,
                    ..LifeRecord::default()
                };
                let lifecycle = Lifecycle {
                    character_id: char_id.clone(),
                    death_count: 1,
                    fortune_remaining: 1,
                    last_death_tick: Some(tick),
                    last_revive_tick: Some(tick.saturating_sub(1)),
                    spawn_anchor: None,
                    spawn_anchor_damaged: false,
                    near_death_deadline_tick: Some(tick + 30),
                    awaiting_decision: None,
                    revival_decision_deadline_tick: None,
                    weakened_until_tick: Some(tick + 5),
                    state: LifecycleState::NearDeath,
                };
                let lifespan_event = LifespanEventRecord {
                    at_tick: tick,
                    kind: "near_death".to_string(),
                    delta_years: -1,
                    source: format!("mixed-zone-duel-{index}"),
                };

                barrier.wait();
                persist_near_death_transition(
                    settings.as_ref(),
                    &lifecycle,
                    &life_record,
                    "mixed-zone-duel",
                    Some(&lifespan_event),
                )
            })
        })
        .collect::<Vec<_>>();

    let npc_handles = (0..npc_writer_count)
        .map(|index| {
            let settings = Arc::clone(&settings);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                let capture = sample_npc_capture(format!("npc_zone_mixed_{index}").as_str());
                barrier.wait();
                persist_npc_capture(settings.as_ref(), &capture)
            })
        })
        .collect::<Vec<_>>();

    let zone_handles = (0..zone_writer_count)
        .map(|index| {
            let settings = Arc::clone(&settings);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                let registry = crate::world::zone::ZoneRegistry {
                    spatial_revision: 0,
                    zones: vec![crate::world::zone::Zone {
                        name: format!("mixed_zone_{index}"),
                        dimension: crate::world::dimension::DimensionKind::Overworld,
                        bounds: crate::world::zone::default_spawn_bounds(),
                        spirit_qi: 0.1 + index as f64,
                        danger_level: 1 + index as u8,
                        active_events: Vec::new(),
                        patrol_anchors: Vec::new(),
                        blocked_tiles: Vec::new(),
                        qi_equilibrium: 0.0,
                        qi_inflow_per_min: 0.0,
                    }],
                };

                barrier.wait();
                persist_zone_runtime_snapshot(settings.as_ref(), &registry)
            })
        })
        .collect::<Vec<_>>();

    barrier.wait();
    let errors = player_handles
        .into_iter()
        .map(|handle| handle.join().expect("player writer should not panic"))
        .chain(
            semantic_handles
                .into_iter()
                .map(|handle| handle.join().expect("semantic writer should not panic")),
        )
        .chain(
            npc_handles
                .into_iter()
                .map(|handle| handle.join().expect("npc writer should not panic")),
        )
        .chain(
            zone_handles
                .into_iter()
                .map(|handle| handle.join().expect("zone writer should not panic")),
        )
        .filter_map(Result::err)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(
        errors.is_empty(),
        "mixed player, semantic, npc, and zone writers should all succeed: {errors:?}"
    );

    let connection = Connection::open(settings.db_path()).expect("db should open");
    let life_records: i64 = connection
        .query_row("SELECT COUNT(*) FROM life_records", [], |row| row.get(0))
        .expect("life_records count should be readable");
    let life_events: i64 = connection
        .query_row("SELECT COUNT(*) FROM life_events", [], |row| row.get(0))
        .expect("life_events count should be readable");
    let death_registry: i64 = connection
        .query_row("SELECT COUNT(*) FROM death_registry", [], |row| row.get(0))
        .expect("death_registry count should be readable");
    let lifespan_events: i64 = connection
        .query_row("SELECT COUNT(*) FROM lifespan_events", [], |row| row.get(0))
        .expect("lifespan_events count should be readable");
    let npc_state_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM npc_state", [], |row| row.get(0))
        .expect("npc_state count should be readable");
    let npc_digest_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM npc_digests", [], |row| row.get(0))
        .expect("npc_digests count should be readable");
    let archetype_registry_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM archetype_registry", [], |row| {
            row.get(0)
        })
        .expect("archetype_registry count should be readable");

    assert_eq!(life_records, semantic_writer_count as i64);
    assert_eq!(life_events, semantic_writer_count as i64);
    assert_eq!(death_registry, semantic_writer_count as i64);
    assert_eq!(lifespan_events, semantic_writer_count as i64);
    assert_eq!(npc_state_count, npc_writer_count as i64);
    assert_eq!(npc_digest_count, npc_writer_count as i64);
    assert_eq!(archetype_registry_count, npc_writer_count as i64);

    for index in 0..player_writer_count {
        let username = format!("MixedZonePlayer{index}");
        let (karma, inventory_score): (f64, f64) = connection
            .query_row(
                "SELECT karma, inventory_score FROM player_core WHERE username = ?1",
                params![username.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("player core row should exist after mixed zone load");
        assert_eq!(karma, ((index as f64 / 5.0) - 1.0).clamp(-1.0, 1.0));
        assert_eq!(
            inventory_score,
            (index as f64 / player_writer_count as f64).clamp(0.0, 1.0)
        );
    }

    let runtime_rows = load_zone_runtime_snapshot(settings.as_ref())
        .expect("zone runtime snapshot should load after mixed zone load");
    assert_eq!(runtime_rows.len(), zone_writer_count);
    for index in 0..zone_writer_count {
        let zone_id = format!("mixed_zone_{index}");
        let record = runtime_rows
            .iter()
            .find(|row| row.zone_id == zone_id)
            .unwrap_or_else(|| panic!("missing runtime row for {zone_id}"));
        assert_eq!(record.spirit_qi, 0.1 + index as f64);
        assert_eq!(record.danger_level, 1 + index as u8);
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn mixed_sqlite_writers_remain_correct_across_multiple_contention_batches() {
    let (settings, root) = persistence_settings("mixed-sqlite-multi-batch");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let player_persistence =
        PlayerStatePersistence::with_db_path(root.join("data").join("players"), settings.db_path());
    let player_seed = PlayerState {
        karma: 0.1,
        inventory_score: 0.2,
    };
    let batch_count = 3usize;
    let player_writer_count = 10usize;
    let semantic_writer_count = 10usize;
    let npc_writer_count = 10usize;
    let zone_writer_count = 5usize;

    for index in 0..player_writer_count {
        save_player_state(
            &player_persistence,
            format!("BatchPlayer{index}").as_str(),
            &player_seed,
        )
        .expect("seed player state should persist");
    }

    let settings = Arc::new(settings);
    let player_persistence = Arc::new(player_persistence);
    let mut all_errors = Vec::new();

    for batch in 0..batch_count {
        let barrier = Arc::new(Barrier::new(
            player_writer_count + semantic_writer_count + npc_writer_count + zone_writer_count + 1,
        ));

        let player_handles = (0..player_writer_count)
            .map(|index| {
                let persistence = Arc::clone(&player_persistence);
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    let username = format!("BatchPlayer{index}");
                    let updated_state = PlayerState {
                        karma: (0.1 * batch as f64).clamp(-1.0, 1.0),
                        inventory_score: (0.01 * ((batch * 10 + index) as f64)).clamp(0.0, 1.0),
                    };

                    barrier.wait();
                    save_player_core_slice(persistence.as_ref(), username.as_str(), &updated_state)
                        .map(|_| ())
                })
            })
            .collect::<Vec<_>>();

        let semantic_handles = (0..semantic_writer_count)
            .map(|index| {
                let settings = Arc::clone(&settings);
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    let char_id = format!("offline:Batch{batch}_Conflict{index}");
                    let tick = 1_100 + (batch as u64 * 100) + index as u64;
                    let life_record = LifeRecord {
                        character_id: char_id.clone(),
                        created_at: tick.saturating_sub(20),
                        biography: vec![BiographyEntry::NearDeath {
                            cause: format!("batch-duel-{batch}-{index}"),
                            tick,
                        }],
                        insights_taken: Vec::new(),
                        death_insights: Vec::new(),
                        skill_milestones: Vec::new(),
                        spirit_root_first: None,
                        ..LifeRecord::default()
                    };
                    let lifecycle = Lifecycle {
                        character_id: char_id.clone(),
                        death_count: 1,
                        fortune_remaining: 1,
                        last_death_tick: Some(tick),
                        last_revive_tick: Some(tick.saturating_sub(1)),
                        spawn_anchor: None,
                        spawn_anchor_damaged: false,
                        near_death_deadline_tick: Some(tick + 30),
                        awaiting_decision: None,
                        revival_decision_deadline_tick: None,
                        weakened_until_tick: Some(tick + 5),
                        state: LifecycleState::NearDeath,
                    };
                    let lifespan_event = LifespanEventRecord {
                        at_tick: tick,
                        kind: "near_death".to_string(),
                        delta_years: -1,
                        source: format!("batch-duel-{batch}-{index}"),
                    };

                    barrier.wait();
                    persist_near_death_transition(
                        settings.as_ref(),
                        &lifecycle,
                        &life_record,
                        "batch-duel",
                        Some(&lifespan_event),
                    )
                })
            })
            .collect::<Vec<_>>();

        let npc_handles = (0..npc_writer_count)
            .map(|index| {
                let settings = Arc::clone(&settings);
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    let capture = sample_npc_capture(format!("npc_batch_{batch}_{index}").as_str());
                    barrier.wait();
                    persist_npc_capture(settings.as_ref(), &capture)
                })
            })
            .collect::<Vec<_>>();

        let zone_handles = (0..zone_writer_count)
            .map(|index| {
                let settings = Arc::clone(&settings);
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    let registry = crate::world::zone::ZoneRegistry {
                        spatial_revision: 0,
                        zones: vec![crate::world::zone::Zone {
                            name: format!("mixed_zone_{batch}_{index}"),
                            dimension: crate::world::dimension::DimensionKind::Overworld,
                            bounds: crate::world::zone::default_spawn_bounds(),
                            spirit_qi: 0.1 + batch as f64 + index as f64,
                            danger_level: 1 + batch as u8 + index as u8,
                            active_events: Vec::new(),
                            patrol_anchors: Vec::new(),
                            blocked_tiles: Vec::new(),
                            qi_equilibrium: 0.0,
                            qi_inflow_per_min: 0.0,
                        }],
                    };

                    barrier.wait();
                    persist_zone_runtime_snapshot(settings.as_ref(), &registry)
                })
            })
            .collect::<Vec<_>>();

        barrier.wait();
        let batch_errors = player_handles
            .into_iter()
            .map(|handle| handle.join().expect("player writer should not panic"))
            .chain(
                semantic_handles
                    .into_iter()
                    .map(|handle| handle.join().expect("semantic writer should not panic")),
            )
            .chain(
                npc_handles
                    .into_iter()
                    .map(|handle| handle.join().expect("npc writer should not panic")),
            )
            .chain(
                zone_handles
                    .into_iter()
                    .map(|handle| handle.join().expect("zone writer should not panic")),
            )
            .filter_map(Result::err)
            .map(|error| error.to_string())
            .collect::<Vec<_>>();
        all_errors.extend(batch_errors);
    }

    assert!(
        all_errors.is_empty(),
        "multi-batch mixed sqlite writers should all succeed: {all_errors:?}"
    );

    let connection = Connection::open(settings.db_path()).expect("db should open");
    let expected_semantic_rows = (batch_count * semantic_writer_count) as i64;
    let expected_npc_rows = (batch_count * npc_writer_count) as i64;
    let expected_zone_rows = batch_count * zone_writer_count;
    let life_records: i64 = connection
        .query_row("SELECT COUNT(*) FROM life_records", [], |row| row.get(0))
        .expect("life_records count should be readable");
    let life_events: i64 = connection
        .query_row("SELECT COUNT(*) FROM life_events", [], |row| row.get(0))
        .expect("life_events count should be readable");
    let death_registry: i64 = connection
        .query_row("SELECT COUNT(*) FROM death_registry", [], |row| row.get(0))
        .expect("death_registry count should be readable");
    let lifespan_events: i64 = connection
        .query_row("SELECT COUNT(*) FROM lifespan_events", [], |row| row.get(0))
        .expect("lifespan_events count should be readable");
    let npc_state_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM npc_state", [], |row| row.get(0))
        .expect("npc_state count should be readable");
    let npc_digest_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM npc_digests", [], |row| row.get(0))
        .expect("npc_digests count should be readable");
    let archetype_registry_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM archetype_registry", [], |row| {
            row.get(0)
        })
        .expect("archetype_registry count should be readable");

    assert_eq!(life_records, expected_semantic_rows);
    assert_eq!(life_events, expected_semantic_rows);
    assert_eq!(death_registry, expected_semantic_rows);
    assert_eq!(lifespan_events, expected_semantic_rows);
    assert_eq!(npc_state_count, expected_npc_rows);
    assert_eq!(npc_digest_count, expected_npc_rows);
    assert_eq!(archetype_registry_count, expected_npc_rows);

    let final_batch = batch_count - 1;
    for index in 0..player_writer_count {
        let username = format!("BatchPlayer{index}");
        let (karma, inventory_score): (f64, f64) = connection
            .query_row(
                "SELECT karma, inventory_score FROM player_core WHERE username = ?1",
                params![username.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("player core row should exist after multi-batch load");
        assert_eq!(karma, (0.1 * final_batch as f64).clamp(-1.0, 1.0));
        assert_eq!(
            inventory_score,
            (0.01 * ((final_batch * 10 + index) as f64)).clamp(0.0, 1.0)
        );
    }

    let runtime_rows = load_zone_runtime_snapshot(settings.as_ref())
        .expect("zone runtime snapshot should load after multi-batch load");
    assert_eq!(runtime_rows.len(), expected_zone_rows);
    for batch in 0..batch_count {
        for index in 0..zone_writer_count {
            let zone_id = format!("mixed_zone_{batch}_{index}");
            let record = runtime_rows
                .iter()
                .find(|row| row.zone_id == zone_id)
                .unwrap_or_else(|| panic!("missing runtime row for {zone_id}"));
            assert_eq!(record.spirit_qi, 0.1 + batch as f64 + index as f64);
            assert_eq!(record.danger_level, 1 + batch as u8 + index as u8);
        }
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn npc_state_roundtrip_preserves_runtime_capture_fields() {
    let (settings, root) = persistence_settings("npc-state-roundtrip");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let capture = sample_npc_capture("npc_state_roundtrip");
    persist_npc_capture(&settings, &capture).expect("npc capture should persist");

    let state = load_npc_state(&settings, capture.state.char_id.as_str())
        .expect("npc state query should succeed")
        .expect("npc state should exist");
    let digest = load_npc_digest(&settings, capture.state.char_id.as_str())
        .expect("npc digest query should succeed")
        .expect("npc digest should exist");
    let registry = load_archetype_registry(&settings, capture.state.char_id.as_str())
        .expect("archetype registry query should succeed");

    assert_eq!(state.char_id, capture.state.char_id);
    assert_eq!(state.kind, "ZOMBIE");
    assert_eq!(state.archetype, "sword");
    assert_eq!(state.state, "attacking");
    assert_eq!(state.pos, [14.0, 66.0, 9.0]);
    assert_eq!(state.home_zone, DEFAULT_SPAWN_ZONE_NAME);
    assert_eq!(state.patrol_target, [12.0, 66.0, 12.0]);
    assert_eq!(state.movement_mode, "sprinting");
    assert!(state.can_sprint);
    assert!(state.can_dash);
    assert_eq!(state.sprint_ready_at, 5);
    assert_eq!(state.dash_ready_at, 33);
    assert_eq!(
        state.blackboard.get("nearest_player"),
        Some(&Value::String("offline:Azure".to_string()))
    );
    assert_eq!(
        state.blackboard.get("last_melee_tick"),
        Some(&Value::from(77))
    );
    assert_eq!(state.lifecycle_state, "alive");
    assert_eq!(state.death_count, 1);
    assert_eq!(state.last_death_tick, Some(55));
    assert_eq!(state.last_revive_tick, Some(66));
    assert_eq!(digest.char_id, capture.state.char_id);
    assert_eq!(digest.archetype, "sword");
    assert_eq!(digest.realm, "spirit");
    assert_eq!(digest.faction_id, None);
    assert!(digest.recent_summary.contains("near_death:duel"));
    assert_eq!(registry.len(), 1);
    assert_eq!(registry[0].char_id, capture.state.char_id);
    assert_eq!(registry[0].archetype, "sword");
    assert_eq!(registry[0].since_tick, 12);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn archetype_registry_preserves_multiple_transitions() {
    let (settings, root) = persistence_settings("archetype-registry");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    record_archetype_transition(
        &settings,
        &ArchetypeRegistryEntry {
            char_id: "npc_registry".to_string(),
            archetype: "brawler".to_string(),
            since_tick: 12,
        },
    )
    .expect("initial archetype should persist");
    record_archetype_transition(
        &settings,
        &ArchetypeRegistryEntry {
            char_id: "npc_registry".to_string(),
            archetype: "sword".to_string(),
            since_tick: 88,
        },
    )
    .expect("follow-up archetype should persist");

    let registry = load_archetype_registry(&settings, "npc_registry")
        .expect("archetype registry query should succeed");
    assert_eq!(registry.len(), 2);
    assert_eq!(registry[0].archetype, "brawler");
    assert_eq!(registry[0].since_tick, 12);
    assert_eq!(registry[1].archetype, "sword");
    assert_eq!(registry[1].since_tick, 88);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn npc_archive_pipeline_writes_index_and_zstd_bundle() {
    let (settings, root) = persistence_settings("npc-archive-pipeline");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let capture = sample_npc_capture("npc_archive_pipeline");
    persist_npc_capture(&settings, &capture).expect("npc capture should persist before archive");

    let archive = NpcDeceasedArchiveRecord {
        char_id: capture.state.char_id.clone(),
        archetype: capture.state.archetype.clone(),
        died_at_tick: 777,
        archived_at_wall: 1_704_067_200,
        lifecycle_state: "terminated".to_string(),
        death_count: 2,
        state: Some(capture.state.clone()),
        digest: Some(capture.digest.clone()),
        life_record: Some(LifeRecord {
            biography: vec![BiographyEntry::Terminated {
                cause: "fortune_exhausted".to_string(),
                tick: 777,
            }],
            ..sample_npc_life_record(capture.state.char_id.as_str())
        }),
    };

    persist_npc_deceased_archive(&settings, &archive).expect("npc archive should persist");

    let connection = Connection::open(settings.db_path()).expect("db should open");
    let (archetype, died_at_tick, path): (String, i64, String) = connection
        .query_row(
            "SELECT archetype, died_at_tick, path FROM npc_deceased_index WHERE char_id = ?1",
            params![archive.char_id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("npc_deceased_index row should exist");
    let loaded_archive = load_npc_deceased_archive(&settings, archive.char_id.as_str())
        .expect("archive read should succeed")
        .expect("archive should exist");

    assert_eq!(archetype, "sword");
    assert_eq!(died_at_tick, 777);
    assert_eq!(
        path,
        format!(
            "data/archive/npc_deceased/{}/{}.json.zst",
            utc_year_from_unix_seconds(archive.archived_at_wall),
            archive.char_id
        )
    );
    assert_eq!(loaded_archive.char_id, archive.char_id);
    assert_eq!(loaded_archive.archetype, archive.archetype);
    assert_eq!(loaded_archive.died_at_tick, 777);
    assert_eq!(loaded_archive.lifecycle_state, "terminated");
    assert!(matches!(
        loaded_archive
            .life_record
            .as_ref()
            .and_then(|record| record.biography.last()),
        Some(BiographyEntry::Terminated { tick: 777, .. })
    ));
    assert!(
        load_npc_state(&settings, archive.char_id.as_str())
            .expect("npc state query should succeed")
            .is_none(),
        "dead NPC should be removed from hot npc_state table"
    );
    assert!(
        load_npc_digest(&settings, archive.char_id.as_str())
            .expect("npc digest query should succeed")
            .is_none(),
        "dead NPC should be removed from hot npc_digests table"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn npc_archive_db_open_failure_restores_previous_bundle() {
    let (settings, root) = persistence_settings("npc-archive-db-open-rollback");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let capture = sample_npc_capture("npc_archive_db_open_rollback");
    let mut archive = NpcDeceasedArchiveRecord {
        char_id: capture.state.char_id.clone(),
        archetype: capture.state.archetype.clone(),
        died_at_tick: 700,
        archived_at_wall: 1_704_067_250,
        lifecycle_state: "terminated".to_string(),
        death_count: 1,
        state: Some(capture.state.clone()),
        digest: Some(capture.digest.clone()),
        life_record: Some(sample_npc_life_record(capture.state.char_id.as_str())),
    };
    persist_npc_deceased_archive(&settings, &archive).expect("initial npc archive should persist");
    let archive_path = npc_deceased_archive_absolute_path(
        &settings,
        archive.char_id.as_str(),
        archive.archived_at_wall,
    )
    .expect("archive path should be valid");
    let previous_bundle = fs::read(&archive_path).expect("initial archive bundle should exist");

    fs::remove_file(settings.db_path()).expect("fixture database should be removable");
    fs::create_dir(settings.db_path()).expect("database path should become an invalid directory");
    archive.death_count = 2;
    archive.died_at_tick = 701;
    let error = persist_npc_deceased_archive(&settings, &archive)
        .expect_err("database open failure must abort archive persistence");
    assert!(
        !error.to_string().is_empty(),
        "database open failure should retain its diagnostic"
    );
    assert_eq!(
        fs::read(&archive_path).expect("previous archive should be restored"),
        previous_bundle,
        "DB-open failure after bundle replacement must restore the previous archive bytes"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn npc_archive_transaction_begin_failure_restores_previous_bundle() {
    let (settings, root) = persistence_settings("npc-archive-transaction-begin-rollback");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let capture = sample_npc_capture("npc_archive_transaction_begin_rollback");
    let mut archive = NpcDeceasedArchiveRecord {
        char_id: capture.state.char_id.clone(),
        archetype: capture.state.archetype.clone(),
        died_at_tick: 710,
        archived_at_wall: 1_704_067_260,
        lifecycle_state: "terminated".to_string(),
        death_count: 1,
        state: Some(capture.state.clone()),
        digest: Some(capture.digest.clone()),
        life_record: Some(sample_npc_life_record(capture.state.char_id.as_str())),
    };
    persist_npc_deceased_archive(&settings, &archive).expect("initial npc archive should persist");
    let archive_path = npc_deceased_archive_absolute_path(
        &settings,
        archive.char_id.as_str(),
        archive.archived_at_wall,
    )
    .expect("archive path should be valid");
    let previous_bundle = fs::read(&archive_path).expect("initial archive bundle should exist");

    archive.death_count = 2;
    archive.died_at_tick = 711;
    let error = persist_npc_deceased_archive_with_connection(&settings, &archive, |settings| {
        let connection = open_persistence_connection(settings)?;
        connection
            .execute_batch("BEGIN DEFERRED TRANSACTION")
            .map_err(io::Error::other)?;
        Ok(connection)
    })
    .expect_err("transaction begin failure must abort archive persistence");
    assert!(
        !error.to_string().is_empty(),
        "transaction begin failure should retain its diagnostic"
    );
    assert_eq!(
        fs::read(&archive_path).expect("previous archive should be restored"),
        previous_bundle,
        "transaction-begin failure after bundle replacement must restore the previous archive bytes"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn npc_archive_replacement_write_failure_preserves_bundle_and_index() {
    let (settings, root) = persistence_settings("npc-archive-replacement-write-rollback");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let capture = sample_npc_capture("npc_archive_replacement_write_rollback");
    let mut archive = NpcDeceasedArchiveRecord {
        char_id: capture.state.char_id.clone(),
        archetype: capture.state.archetype.clone(),
        died_at_tick: 730,
        archived_at_wall: 1_704_067_280,
        lifecycle_state: "terminated".to_string(),
        death_count: 1,
        state: Some(capture.state.clone()),
        digest: Some(capture.digest.clone()),
        life_record: Some(sample_npc_life_record(capture.state.char_id.as_str())),
    };
    persist_npc_deceased_archive(&settings, &archive)
        .expect("initial archive should establish the durable baseline");

    let archive_path = npc_deceased_archive_absolute_path(
        &settings,
        archive.char_id.as_str(),
        archive.archived_at_wall,
    )
    .expect("archive path should be valid");
    let previous_bundle = fs::read(&archive_path).expect("baseline bundle should exist");
    let previous_index: (String, i64, String) = {
        let connection = Connection::open(settings.db_path()).expect("db should open");
        connection
            .query_row(
                "SELECT archetype, died_at_tick, path FROM npc_deceased_index WHERE char_id = ?1",
                params![archive.char_id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("baseline index row should exist")
    };

    archive.death_count = 2;
    archive.died_at_tick = 731;
    let error = persist_npc_deceased_archive_with_hooks(
        &settings,
        &archive,
        open_persistence_connection,
        |path, payload| {
            write_zstd_bundle_with_writer(path, payload, |file, compressed| {
                let partial_len = (compressed.len() / 2).max(1).min(compressed.len());
                file.write_all(&compressed[..partial_len])?;
                Err(io::Error::other("injected replacement short write"))
            })
        },
    )
    .expect_err("a replacement short write must abort before touching the final bundle");
    assert!(
        error
            .to_string()
            .contains("injected replacement short write"),
        "the injected write failure should remain observable"
    );
    assert_eq!(
        fs::read(&archive_path).expect("the previous final bundle must remain present"),
        previous_bundle,
        "a failed replacement write must preserve the complete previous bundle"
    );

    let connection = Connection::open(settings.db_path()).expect("db should reopen");
    let current_index: (String, i64, String) = connection
        .query_row(
            "SELECT archetype, died_at_tick, path FROM npc_deceased_index WHERE char_id = ?1",
            params![archive.char_id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("the previous index row must remain present");
    assert_eq!(
        current_index, previous_index,
        "a failed replacement write must not advance npc_deceased_index"
    );
    drop(connection);

    let loaded = load_npc_deceased_archive(&settings, archive.char_id.as_str())
        .expect("the unchanged baseline bundle must remain readable")
        .expect("the baseline archive should remain indexed");
    assert_eq!(loaded.died_at_tick, 730);
    assert_eq!(loaded.death_count, 1);
    let temporary_files = fs::read_dir(
        archive_path
            .parent()
            .expect("archive bundle should have a parent directory"),
    )
    .expect("archive directory should remain readable")
    .filter_map(Result::ok)
    .filter(|entry| {
        entry
            .file_name()
            .to_string_lossy()
            .starts_with(".npc_archive_replacement_write_rollback.json.zst.tmp-")
    })
    .count();
    assert_eq!(
        temporary_files, 0,
        "a failed temporary replacement write must clean up its partial file"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn npc_first_archive_failure_removes_new_bundle() {
    let (settings, root) = persistence_settings("npc-first-archive-rollback");
    let capture = sample_npc_capture("npc_first_archive_rollback");
    let archive = NpcDeceasedArchiveRecord {
        char_id: capture.state.char_id.clone(),
        archetype: capture.state.archetype.clone(),
        died_at_tick: 720,
        archived_at_wall: 1_704_067_270,
        lifecycle_state: "terminated".to_string(),
        death_count: 1,
        state: Some(capture.state.clone()),
        digest: Some(capture.digest.clone()),
        life_record: Some(sample_npc_life_record(capture.state.char_id.as_str())),
    };
    let archive_path = npc_deceased_archive_absolute_path(
        &settings,
        archive.char_id.as_str(),
        archive.archived_at_wall,
    )
    .expect("archive path should be valid");
    assert!(
        !archive_path.exists(),
        "first archive fixture must begin without previous bytes"
    );

    let error = persist_npc_deceased_archive_with_connection(&settings, &archive, |_| {
        Err(io::Error::other("injected database open failure"))
    })
    .expect_err("database failure must abort first archive persistence");
    assert!(
        error.to_string().contains("injected database open failure"),
        "injected open failure should remain observable"
    );
    assert!(
        !archive_path.exists(),
        "failure after writing a first archive must remove the unindexed bundle"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn npc_archive_non_not_found_prior_read_aborts_before_write_or_db() {
    let (settings, root) = persistence_settings("npc-archive-prior-read-error");
    let capture = sample_npc_capture("npc_archive_prior_read_error");
    let archive = NpcDeceasedArchiveRecord {
        char_id: capture.state.char_id.clone(),
        archetype: capture.state.archetype.clone(),
        died_at_tick: 721,
        archived_at_wall: 1_704_067_271,
        lifecycle_state: "terminated".to_string(),
        death_count: 1,
        state: Some(capture.state.clone()),
        digest: Some(capture.digest.clone()),
        life_record: Some(sample_npc_life_record(capture.state.char_id.as_str())),
    };
    let archive_path = npc_deceased_archive_absolute_path(
        &settings,
        archive.char_id.as_str(),
        archive.archived_at_wall,
    )
    .expect("archive path should be valid");
    fs::create_dir_all(&archive_path).expect("directory fixture should be creatable");
    let open_called = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let open_called_for_hook = open_called.clone();
    let error = persist_npc_deceased_archive_with_hooks(
        &settings,
        &archive,
        move |_| {
            open_called_for_hook.store(true, std::sync::atomic::Ordering::SeqCst);
            Err(io::Error::other("database hook must not run"))
        },
        |_, _| panic!("write hook must not run after a prior read error"),
    )
    .expect_err("a non-NotFound prior read error must abort before mutation");
    assert_eq!(error.kind(), io::ErrorKind::IsADirectory);
    assert!(
        !open_called.load(std::sync::atomic::Ordering::SeqCst),
        "prior-file read errors must not open the database or run hooks"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn archive_path_helpers_reject_unsafe_components() {
    let (_, root) = persistence_settings("archive-path-component-validation");
    for unsafe_id in ["", ".", "..", "../escape", r"..\escape", "/tmp/escape"] {
        assert!(
            validate_archive_component(unsafe_id).is_err(),
            "archive component `{unsafe_id}` must fail closed"
        );
        assert!(
            npc_deceased_archive_relative_path(unsafe_id, 0).is_err(),
            "deceased archive path must reject `{unsafe_id}`"
        );
        assert!(
            npc_digest_archive_relative_path(unsafe_id).is_err(),
            "digest archive path must reject `{unsafe_id}`"
        );
    }

    assert_eq!(
        npc_deceased_archive_relative_path("npc:valid", 0)
            .expect("a single safe component should be accepted"),
        "data/archive/npc_deceased/1970/npc:valid.json.zst"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn zstd_bundle_publication_does_not_overwrite_existing_target() {
    let (_, root) = persistence_settings("zstd-bundle-no-replace");
    let path = root.join("archive.json.zst");
    write_zstd_bundle(&path, br#"{"version":1}"#).expect("initial bundle should publish");
    let previous = fs::read(&path).expect("initial bundle should be readable");

    let error = write_zstd_bundle(&path, br#"{"version":2}"#)
        .expect_err("publishing over an existing bundle must fail atomically");
    assert_eq!(
        error.kind(),
        io::ErrorKind::AlreadyExists,
        "existing target must reject replacement rather than overwrite it"
    );
    assert_eq!(
        fs::read(&path).expect("existing bundle should remain readable"),
        previous,
        "failed no-replace publication must preserve the previous bytes"
    );
    let temp_files = fs::read_dir(&root)
        .expect("bundle parent should remain readable")
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp-"))
        .count();
    assert_eq!(
        temp_files, 0,
        "failed publication must clean its temporary file"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn rollback_file_treats_missing_cleanup_as_idempotent() {
    let (_, root) = persistence_settings("rollback-file-missing-cleanup");
    let path = root.join("missing.json");
    rollback_file(&path, None).expect("removing an already-missing file is idempotent");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn rollback_file_surfaces_write_and_remove_errors() {
    let (_, root) = persistence_settings("rollback-file-errors");
    let write_path = root.join("write-error");
    fs::create_dir_all(&write_path).expect("write-error directory should be creatable");
    let write_error = rollback_file(&write_path, Some(b"previous"))
        .expect_err("rollback writes must surface destination errors");
    assert_eq!(write_error.kind(), io::ErrorKind::IsADirectory);

    let remove_path = root.join("remove-error");
    fs::create_dir_all(&remove_path).expect("remove-error directory should be creatable");
    let remove_error = rollback_file(&remove_path, None)
        .expect_err("rollback removes must surface non-NotFound errors");
    assert_eq!(remove_error.kind(), io::ErrorKind::IsADirectory);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn npc_archive_reports_primary_and_rollback_failures_together() {
    let (settings, root) = persistence_settings("npc-archive-composite-diagnostic");
    let capture = sample_npc_capture("npc_archive_composite_diagnostic");
    let archive = NpcDeceasedArchiveRecord {
        char_id: capture.state.char_id.clone(),
        archetype: capture.state.archetype.clone(),
        died_at_tick: 722,
        archived_at_wall: 1_704_067_272,
        lifecycle_state: "terminated".to_string(),
        death_count: 1,
        state: Some(capture.state.clone()),
        digest: Some(capture.digest.clone()),
        life_record: Some(sample_npc_life_record(capture.state.char_id.as_str())),
    };
    let error = persist_npc_deceased_archive_with_hooks(
        &settings,
        &archive,
        |_| Err(io::Error::other("primary database failure")),
        |path, payload| {
            write_zstd_bundle(path, payload)?;
            fs::remove_file(path)?;
            fs::create_dir(path)?;
            Ok(())
        },
    )
    .expect_err("database failure with failed rollback must retain both diagnostics");
    let message = error.to_string();
    assert!(message.contains("primary database failure"));
    assert!(message.contains("rollback failed"));
    assert!(message.contains("Is a directory") || message.contains("directory"));
    let composite = error
        .get_ref()
        .and_then(|source| source.downcast_ref::<PersistenceRollbackFailure>())
        .expect("combined failure must retain its structured primary/rollback errors");
    assert_eq!(
        composite.primary.kind(),
        io::ErrorKind::Other,
        "the primary failure must remain the composite source"
    );
    assert_eq!(
        composite.rollback.kind(),
        io::ErrorKind::IsADirectory,
        "the rollback failure must remain available in the composite diagnostic"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn npc_archive_failed_no_replace_publish_preserves_competing_target() {
    let (settings, root) = persistence_settings("npc-archive-competing-publish");
    let capture = sample_npc_capture("npc_archive_competing_publish");
    let archive = NpcDeceasedArchiveRecord {
        char_id: capture.state.char_id.clone(),
        archetype: capture.state.archetype.clone(),
        died_at_tick: 723,
        archived_at_wall: 1_704_067_273,
        lifecycle_state: "terminated".to_string(),
        death_count: 1,
        state: Some(capture.state.clone()),
        digest: Some(capture.digest.clone()),
        life_record: Some(sample_npc_life_record(capture.state.char_id.as_str())),
    };
    let archive_path = npc_deceased_archive_absolute_path(
        &settings,
        archive.char_id.as_str(),
        archive.archived_at_wall,
    )
    .expect("archive path should be valid");
    let archive_relative_path =
        npc_deceased_archive_relative_path(archive.char_id.as_str(), archive.archived_at_wall)
            .expect("archive relative path should be valid");
    let competing_payload = br#"{"owner":"competing-publisher"}"#;

    let error = persist_npc_deceased_archive_with_hooks(
        &settings,
        &archive,
        |_| {
            Err(io::Error::other(
                "database hook must not run after competing publish",
            ))
        },
        |path, payload| {
            // Publish a competing target while our temporary file is still open. The outer
            // no-replace hard_link must fail without granting this caller ownership of target.
            write_zstd_bundle_with_writer(path, payload, |_temp_file, _compressed| {
                write_zstd_bundle(path, competing_payload)
            })
        },
    )
    .expect_err("a competing no-replace publisher must abort without deleting its target");
    assert_eq!(
        error.kind(),
        io::ErrorKind::AlreadyExists,
        "competing publication should fail at the no-replace boundary, actual={error}"
    );
    assert_eq!(
        read_zstd_bundle(settings.db_path(), archive_relative_path.as_str())
            .expect("competing archive should remain readable"),
        competing_payload,
        "failed publication must not remove the competing owner's archive"
    );
    assert!(
        archive_path.exists(),
        "the competing archive target must still exist after the losing publish fails"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_npc_deceased_archive_rejects_corrupted_zstd_bundle() {
    let (settings, root) = persistence_settings("npc-archive-corrupt-read");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let capture = sample_npc_capture("npc_archive_corrupt");
    persist_npc_capture(&settings, &capture).expect("npc capture should persist before archive");

    let archive = NpcDeceasedArchiveRecord {
        char_id: capture.state.char_id.clone(),
        archetype: capture.state.archetype.clone(),
        died_at_tick: 888,
        archived_at_wall: 1_704_067_300,
        lifecycle_state: "terminated".to_string(),
        death_count: 3,
        state: Some(capture.state.clone()),
        digest: Some(capture.digest.clone()),
        life_record: Some(LifeRecord {
            biography: vec![BiographyEntry::Terminated {
                cause: "fortune_exhausted".to_string(),
                tick: 888,
            }],
            ..sample_npc_life_record(capture.state.char_id.as_str())
        }),
    };

    persist_npc_deceased_archive(&settings, &archive)
        .expect("npc archive should persist before corruption");

    let archive_path = npc_deceased_archive_absolute_path(
        &settings,
        archive.char_id.as_str(),
        archive.archived_at_wall,
    )
    .expect("archive path should be valid");
    fs::write(&archive_path, b"not a zstd bundle")
        .expect("corrupted archive fixture should overwrite bundle");

    let error = load_npc_deceased_archive(&settings, archive.char_id.as_str())
        .expect_err("corrupted archive bundle should fail to load");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);

    let connection = Connection::open(settings.db_path()).expect("db should open");
    let path: String = connection
        .query_row(
            "SELECT path FROM npc_deceased_index WHERE char_id = ?1",
            params![archive.char_id.as_str()],
            |row| row.get(0),
        )
        .expect("npc_deceased_index row should still exist");
    assert_eq!(
        path,
        format!(
            "data/archive/npc_deceased/{}/{}.json.zst",
            utc_year_from_unix_seconds(archive.archived_at_wall),
            archive.char_id
        )
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn find_orphaned_npc_archive_paths_reports_unindexed_archives() {
    let (settings, root) = persistence_settings("npc-archive-orphan-scan");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let indexed_capture = sample_npc_capture("npc_archive_indexed");
    persist_npc_capture(&settings, &indexed_capture)
        .expect("indexed capture should persist before archive");
    let indexed_archive = NpcDeceasedArchiveRecord {
        char_id: indexed_capture.state.char_id.clone(),
        archetype: indexed_capture.state.archetype.clone(),
        died_at_tick: 901,
        archived_at_wall: 1_704_067_400,
        lifecycle_state: "terminated".to_string(),
        death_count: 1,
        state: Some(indexed_capture.state.clone()),
        digest: Some(indexed_capture.digest.clone()),
        life_record: Some(sample_npc_life_record(
            indexed_capture.state.char_id.as_str(),
        )),
    };
    persist_npc_deceased_archive(&settings, &indexed_archive)
        .expect("indexed archive should persist");

    let orphan_capture = sample_npc_capture("npc_archive_orphan");
    persist_npc_capture(&settings, &orphan_capture)
        .expect("orphan capture should persist before archive");
    let orphan_archive = NpcDeceasedArchiveRecord {
        char_id: orphan_capture.state.char_id.clone(),
        archetype: orphan_capture.state.archetype.clone(),
        died_at_tick: 902,
        archived_at_wall: 1_704_067_500,
        lifecycle_state: "terminated".to_string(),
        death_count: 2,
        state: Some(orphan_capture.state.clone()),
        digest: Some(orphan_capture.digest.clone()),
        life_record: Some(sample_npc_life_record(
            orphan_capture.state.char_id.as_str(),
        )),
    };
    persist_npc_deceased_archive(&settings, &orphan_archive)
        .expect("orphan archive should persist before index deletion");

    let orphan_path = npc_deceased_archive_absolute_path(
        &settings,
        orphan_archive.char_id.as_str(),
        orphan_archive.archived_at_wall,
    )
    .expect("archive path should be valid");
    let indexed_path = npc_deceased_archive_absolute_path(
        &settings,
        indexed_archive.char_id.as_str(),
        indexed_archive.archived_at_wall,
    )
    .expect("archive path should be valid");
    let connection = Connection::open(settings.db_path()).expect("db should open");
    connection
        .execute(
            "DELETE FROM npc_deceased_index WHERE char_id = ?1",
            params![orphan_archive.char_id.as_str()],
        )
        .expect("test should delete orphan index row");

    let orphaned =
        find_orphaned_npc_archive_paths(&settings).expect("orphan scan helper should succeed");
    scan_orphaned_npc_archives(&settings).expect("orphan scan entrypoint should succeed");

    assert_eq!(orphaned, vec![orphan_path]);
    assert!(
        indexed_path.exists(),
        "indexed archive fixture should remain on disk"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn npc_digest_retention_sweeps_180_day_stale_rows() {
    let (settings, root) = persistence_settings("npc-digest-retention");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let now_wall = 1_725_000_000;
    let stale_wall = now_wall - NPC_DIGEST_RETENTION_SECS - 1;
    let fresh_wall = now_wall - NPC_DIGEST_RETENTION_SECS + 60;
    let stale = NpcPersistenceCapture {
        captured_at_wall: stale_wall,
        digest: NpcDigestRecord {
            last_referenced_wall: stale_wall,
            ..sample_npc_capture("npc_digest_stale").digest
        },
        ..sample_npc_capture("npc_digest_stale")
    };
    let fresh = NpcPersistenceCapture {
        captured_at_wall: fresh_wall,
        digest: NpcDigestRecord {
            last_referenced_wall: fresh_wall,
            ..sample_npc_capture("npc_digest_fresh").digest
        },
        ..sample_npc_capture("npc_digest_fresh")
    };
    persist_npc_capture(&settings, &stale).expect("stale capture should persist");
    persist_npc_capture(&settings, &fresh).expect("fresh capture should persist");

    let archived =
        sweep_stale_npc_digests(&settings, now_wall).expect("digest sweep should succeed");

    assert_eq!(archived.len(), 1);
    assert_eq!(archived[0].char_id, stale.state.char_id);
    assert!(
        load_npc_digest(&settings, stale.state.char_id.as_str())
            .expect("stale digest query should succeed")
            .is_none(),
        "stale digest should be removed from hot table"
    );
    assert!(
        load_npc_digest(&settings, fresh.state.char_id.as_str())
            .expect("fresh digest query should succeed")
            .is_some(),
        "fresh digest should remain in hot table"
    );
    assert!(
        npc_digest_archive_absolute_path(&settings, stale.state.char_id.as_str(), now_wall,)
            .expect("digest archive path should be valid")
            .exists(),
        "stale digest should be written to cold archive"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn npc_digest_failed_no_replace_publish_preserves_competing_target() {
    let (settings, root) = persistence_settings("npc-digest-competing-publish");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let now_wall = 1_725_000_000;
    let stale_wall = now_wall - NPC_DIGEST_RETENTION_SECS - 1;
    let stale = NpcPersistenceCapture {
        captured_at_wall: stale_wall,
        digest: NpcDigestRecord {
            last_referenced_wall: stale_wall,
            ..sample_npc_capture("npc_digest_competing_publish").digest
        },
        ..sample_npc_capture("npc_digest_competing_publish")
    };
    persist_npc_capture(&settings, &stale).expect("stale digest should persist");

    let archive_path = npc_digest_archive_absolute_path(
        &settings,
        stale.state.char_id.as_str(),
        now_wall,
    )
    .expect("digest archive path should be valid");
    let archive_relative_path =
        npc_digest_archive_relative_path(stale.state.char_id.as_str())
            .expect("digest archive relative path should be valid");
    let competing_payload = br#"{"owner":"competing-digest-publisher"}"#;

    let error = sweep_stale_npc_digests_with_writer(&settings, now_wall, |path, payload| {
        // Publish a competing target while the losing temporary file is still open. The
        // outer no-replace hard_link must fail without granting this caller target ownership.
        write_zstd_bundle_with_writer(path, payload, |_temp_file, _compressed| {
            write_zstd_bundle(path, competing_payload)
        })
    })
    .expect_err("a competing digest publisher must abort without deleting its target");
    assert_eq!(
        error.kind(),
        io::ErrorKind::AlreadyExists,
        "competing digest publication should fail at the no-replace boundary, actual={error}"
    );
    assert_eq!(
        read_zstd_bundle(settings.db_path(), archive_relative_path.as_str())
            .expect("competing digest archive should remain readable"),
        competing_payload,
        "failed digest publication must not remove the competing owner's archive"
    );
    assert!(
        archive_path.exists(),
        "the competing digest archive target must still exist after the losing publish fails"
    );
    assert!(
        load_npc_digest(&settings, stale.state.char_id.as_str())
            .expect("failed sweep should leave the hot digest readable")
            .is_some(),
        "failed digest publication must not delete the hot row"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn faction_social_state_defaults_to_empty_roundtrip() {
    let (settings, root) = persistence_settings("faction-social-empty");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let bundle =
        load_faction_social_state(&settings).expect("empty social bundle query should succeed");
    assert_eq!(bundle, FactionSocialBundle::default());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn faction_social_state_roundtrips_without_runtime_systems() {
    let (settings, root) = persistence_settings("faction-social-roundtrip");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let bundle = FactionSocialBundle {
        factions: vec![FactionRecord {
            faction_id: "sect.azure".to_string(),
            display_name: "Azure Sect".to_string(),
            doctrine: "orthodox".to_string(),
            metadata_json: "{}".to_string(),
        }],
        reputations: vec![FactionReputationRecord {
            faction_id: "sect.azure".to_string(),
            target_faction_id: "sect.crimson".to_string(),
            score: -35,
        }],
        memberships: vec![FactionMembershipRecord {
            faction_id: "sect.azure".to_string(),
            char_id: "npc_social_1".to_string(),
            role: "outer_disciple".to_string(),
            joined_at_tick: 120,
            metadata_json: "{}".to_string(),
        }],
        relationships: vec![RelationshipRecord {
            char_id: "npc_social_1".to_string(),
            peer_char_id: "npc_social_2".to_string(),
            relationship_type: "rivalry".to_string(),
            since_tick: 121,
            metadata_json: "{\"intensity\":2}".to_string(),
        }],
    };

    replace_faction_social_state(&settings, &bundle).expect("social bundle should persist");

    let loaded = load_faction_social_state(&settings).expect("social bundle should load");
    assert_eq!(loaded, bundle);

    let _ = fs::remove_dir_all(root);
}

// ─────────────────────────────────────────────────────────────
// plan-halfstep-buff-v1 P2：try_complete_tribulation_ascension 原子校验
// ─────────────────────────────────────────────────────────────

fn persist_du_xu_active(settings: &PersistenceSettings, char_id: &str) {
    persist_active_tribulation(
        settings,
        &ActiveTribulationRecord {
            char_id: char_id.to_string(),
            kind: "du_xu".to_string(),
            source: String::new(),
            origin_dimension: Some("minecraft:overworld".to_string()),
            wave_current: 4,
            waves_total: 5,
            started_tick: 2880,
            epicenter: [0.0, 64.0, 0.0],
            intensity: 0.0,
        },
    )
    .expect("active tribulation should persist");
}

fn persist_jue_bi_active(settings: &PersistenceSettings, char_id: &str, source: &str) {
    persist_active_tribulation(
        settings,
        &ActiveTribulationRecord {
            char_id: char_id.to_string(),
            kind: "jue_bi".to_string(),
            source: source.to_string(),
            origin_dimension: Some("minecraft:overworld".to_string()),
            wave_current: 3,
            waves_total: 3,
            started_tick: 1440,
            epicenter: [0.0, 64.0, 0.0],
            intensity: 1.6,
        },
    )
    .expect("active jue_bi should persist");
}

#[test]
fn try_ascension_grants_when_within_limit() {
    let (settings, root) = persistence_settings("ascension-atomic-grant");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");
    persist_du_xu_active(&settings, "offline:A");
    let outcome = try_complete_tribulation_ascension(&settings, "offline:A", 3)
        .expect("atomic ascension should succeed");
    assert_eq!(
        outcome.grant,
        AscensionGrant::Granted,
        "limit=3 occupied_before=0 must Granted; got {:?}",
        outcome.grant
    );
    assert_eq!(
        outcome.quota.occupied_slots, 1,
        "Granted 必须把 quota.occupied_slots 由 0 增到 1；got outcome={outcome:?}"
    );
    assert_eq!(
        outcome.occupied_before, 0,
        "事务读取的 occupied_before 应为入前 quota 状态 (0)；got outcome={outcome:?}"
    );
    assert_eq!(
        outcome.limit_used, 3,
        "limit_used 必须回传调用方传入的 quota_limit=3；got outcome={outcome:?}"
    );
    // active row 已删
    assert!(load_active_tribulation(&settings, "offline:A")
        .expect("active query should succeed")
        .is_none());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn try_ascension_denies_when_at_limit_and_does_not_increment_quota() {
    let (settings, root) = persistence_settings("ascension-atomic-deny-full");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");
    // 先把 quota.occupied 设到 limit 边界
    let wall_clock = current_unix_seconds();
    {
        let mut connection = open_persistence_connection(&settings).unwrap();
        let transaction = connection.transaction().unwrap();
        upsert_ascension_quota(
            &transaction,
            &AscensionQuotaRecord { occupied_slots: 2 },
            wall_clock,
        )
        .unwrap();
        transaction.commit().unwrap();
    }
    persist_du_xu_active(&settings, "offline:B");
    let outcome = try_complete_tribulation_ascension(&settings, "offline:B", 2)
        .expect("atomic ascension should succeed");
    assert_eq!(
        outcome.grant,
        AscensionGrant::Denied,
        "occupied=2 limit=2 必须 Denied；增长会突破 §三 化虚稀缺底线"
    );
    assert_eq!(
        outcome.quota.occupied_slots, 2,
        "denied 不增量；occupied_slots 保持 2"
    );
    assert_eq!(
        outcome.occupied_before, 2,
        "occupied_before 必须报告事务起始时的 quota.occupied (=2)，与 limit 相等才触发 Denied；got outcome={outcome:?}"
    );
    // active row 仍删除（entity 渡劫流程已完毕）
    assert!(load_active_tribulation(&settings, "offline:B")
        .expect("active query should succeed")
        .is_none());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn try_ascension_denies_when_quota_limit_zero() {
    let (settings, root) = persistence_settings("ascension-atomic-deny-zero-limit");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");
    persist_du_xu_active(&settings, "offline:C");
    // quota_limit=0 代表灵气枯竭（compute_void_quota_limit(total_qi=0, ...) = 0）—— 必须拒绝
    let outcome = try_complete_tribulation_ascension(&settings, "offline:C", 0)
        .expect("atomic ascension should succeed");
    assert_eq!(
        outcome.grant,
        AscensionGrant::Denied,
        "limit=0 永远不授予；灵气枯竭名额清零"
    );
    assert_eq!(
        outcome.quota.occupied_slots, 0,
        "limit=0 → Denied，quota.occupied_slots 必须保持 0 不增量；got outcome={outcome:?}"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn try_ascension_reports_missing_active_when_no_active_row() {
    // CodeRabbit P3 review #4：缺 active row 不再当 granted=true；
    // 改返回 AscensionGrant::MissingActive，让 caller 显式 warn 不升 Realm。
    let (settings, root) = persistence_settings("ascension-atomic-missing-active");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");
    // 无 active row（重复结算 / 状态错乱）
    let outcome = try_complete_tribulation_ascension(&settings, "offline:phantom", 3)
        .expect("atomic ascension should succeed");
    assert_eq!(
        outcome.grant,
        AscensionGrant::MissingActive,
        "missing active row 必须返回 MissingActive；混入 Granted 会让 caller 错升 Realm"
    );
    assert_eq!(outcome.quota.occupied_slots, 0, "MissingActive 路径不增量");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn try_ascension_grants_for_jue_bi_void_quota_exceeded_kind() {
    // CodeRabbit P3 review #5：jue_bi + source=void_quota_exceeded 属于 occupies_quota 分支
    // —— DuXu 起劫时 quota 已满，转 JueBi 绝壁劫；JueBi 成功 → 占额升 Realm
    let (settings, root) = persistence_settings("ascension-atomic-juebi-quota-exceeded");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");
    persist_jue_bi_active(&settings, "offline:JB1", JUEBI_SOURCE_VOID_QUOTA_EXCEEDED);
    let outcome = try_complete_tribulation_ascension(&settings, "offline:JB1", 3)
        .expect("atomic ascension should succeed");
    assert_eq!(
        outcome.grant,
        AscensionGrant::Granted,
        "jue_bi + void_quota_exceeded 是占额路径；limit=3 空位下应 Granted"
    );
    assert_eq!(
        outcome.quota.occupied_slots, 1,
        "占额路径成功后 occupied_slots 必须 +1"
    );
    assert!(load_active_tribulation(&settings, "offline:JB1")
        .expect("active query")
        .is_none());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn try_ascension_settled_only_without_increment_for_jue_bi_other_source() {
    // CodeRabbit P3 review #5 + P4 review #2：jue_bi + source != void_quota_exceeded（例
    // void_action_explode_zone）是非占额独立 JueBi；幸存不算升格，不增 quota，
    // grant=SettledOnly（而非 Granted，否则 caller 会误升 Realm）
    let (settings, root) = persistence_settings("ascension-atomic-juebi-settled-only");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");
    persist_jue_bi_active(&settings, "offline:JB2", "void_action_explode_zone");
    let outcome = try_complete_tribulation_ascension(&settings, "offline:JB2", 3)
        .expect("atomic ascension should succeed");
    assert_eq!(
        outcome.grant,
        AscensionGrant::SettledOnly,
        "独立 JueBi 非占额路径必须 SettledOnly；Granted 会让 caller 误升 Realm"
    );
    assert_eq!(
        outcome.quota.occupied_slots, 0,
        "独立 JueBi 路径绝不增 quota；增量会让化虚老怪扛过 zone collapse 把 quota 用掉"
    );
    assert!(load_active_tribulation(&settings, "offline:JB2")
        .expect("active query")
        .is_none());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn try_ascension_serializes_truly_concurrent_settlements_atomically() {
    // plan-halfstep-buff-v1 P2 真并发测试（不是串行）：5 个线程同时 settle，limit=2。
    //
    // 用 `std::sync::Barrier` 让线程齐头并进，每线程独立 open connection 模拟真实并发。
    // IMMEDIATE 事务保证 select-check-update **原子串行化**（atomic serialization），
    // 即没有 writer 在中间插队读到陈旧 occupied_slots 然后都增量。
    //
    // 注意：SQLite IMMEDIATE 不承诺公平/FIFO 顺序——多个 BEGIN IMMEDIATE 谁先拿到写锁
    // 由 SQLite 内部锁队列决定，与调用次序无关。本测试只断言"原子性"和"不破名额上限"，
    // 不断言哪几个 thread 具体 granted（防止依赖未定义的 fairness 假设）。
    //
    // 期望：
    //   - 恰好 limit (=2) 个线程 granted（哪几个不定）
    //   - 其余 (=3) 个线程 denied
    //   - 没有线程返回 SQLITE_BUSY / SQLITE_LOCKED（IMMEDIATE 会让冲突 writer 等而非立刻 fail）
    //   - 最终 quota.occupied_slots == limit（强守恒）
    use std::sync::{Arc, Barrier};
    use std::thread;

    let (settings, root) = persistence_settings("ascension-atomic-true-concurrent");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");
    const N: usize = 5;
    const LIMIT: u32 = 2;
    for i in 0..N {
        persist_du_xu_active(&settings, &format!("offline:R{i}"));
    }

    let barrier = Arc::new(Barrier::new(N));
    let settings = Arc::new(settings);
    let handles: Vec<_> = (0..N)
        .map(|i| {
            let barrier = Arc::clone(&barrier);
            let settings = Arc::clone(&settings);
            thread::spawn(move || -> io::Result<AtomicAscensionOutcome> {
                let char_id = format!("offline:R{i}");
                // 在 barrier 处齐头：5 个 thread 同一瞬间发起 BEGIN IMMEDIATE
                barrier.wait();
                try_complete_tribulation_ascension(&settings, &char_id, LIMIT)
            })
        })
        .collect();

    let mut granted_count = 0;
    let mut denied_count = 0;
    let mut other_count = 0;
    let mut errors: Vec<String> = Vec::new();
    for handle in handles {
        match handle.join().expect("thread join should succeed") {
            Ok(outcome) => match outcome.grant {
                AscensionGrant::Granted => granted_count += 1,
                AscensionGrant::Denied => denied_count += 1,
                AscensionGrant::SettledOnly | AscensionGrant::MissingActive => other_count += 1,
            },
            Err(error) => errors.push(error.to_string()),
        }
    }
    assert!(
        errors.is_empty(),
        "没有线程应返回 SQLITE_BUSY/LOCKED 错误；IMMEDIATE 事务应序列化等待。got errors: {errors:?}"
    );
    assert_eq!(
        other_count, 0,
        "所有 thread 都 persist 了 du_xu active row（占额路径），不该出现 SettledOnly/MissingActive；got {other_count}"
    );
    assert_eq!(
        granted_count, LIMIT as usize,
        "limit=2 + N=5 真并发结算应恰好 2 granted；得到 {granted_count}"
    );
    assert_eq!(
        denied_count,
        N - LIMIT as usize,
        "剩余 {} 个应全部 denied；得到 {denied_count}",
        N - LIMIT as usize
    );

    let settings = Arc::try_unwrap(settings).expect("settings should have no other refs");
    let final_quota = load_ascension_quota(&settings).expect("final quota load");
    assert_eq!(
        final_quota.occupied_slots, LIMIT,
        "最终 occupied 必须严格 == limit；任何 > limit 都是 §三 稀缺性突破"
    );
    // 所有 active 行都被清理（无论 granted/denied）
    for i in 0..N {
        assert!(
            load_active_tribulation(&settings, &format!("offline:R{i}"))
                .unwrap()
                .is_none(),
            "active row {i} 应被事务删除（granted/denied 都删）"
        );
    }
    let _ = fs::remove_dir_all(root);
}

// ── plan-offscreen-war-v1 P3：战场遗物 pending_dormant_relics 持久层（交付物 2） ──

fn pending_relic_record(
    relic_id: &str,
    char_id: &str,
    zone: &str,
    loot_seed: u64,
    created_wall: i64,
) -> PendingDormantRelicRecord {
    PendingDormantRelicRecord {
        relic_id: relic_id.to_string(),
        char_id: char_id.to_string(),
        zone: zone.to_string(),
        pos_x: 12.5,
        pos_y: 64.0,
        pos_z: -8.25,
        archetype: crate::npc::lifecycle::NpcArchetype::Disciple
            .as_str()
            .to_string(),
        loot_seed,
        created_tick: 7,
        created_wall,
    }
}

#[test]
fn pending_relic_persisted_to_sqlite_round_trips_all_fields() {
    // 交付物 2 happy path：upsert 一行 → load 回来字段逐一相等。重点锁 loot_seed 的
    // u64→i64→u64 位投影**无损**（用一个 high-bit-set 的 u64，i64 会变负，必须能投影回来）。
    let (settings, root) = persistence_settings("pending-relic-roundtrip");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let high_bit_seed = 0xFFFF_FFFF_0000_0001u64; // as i64 为负，验证投影往返
    let record = pending_relic_record(
        "relic-uuid-1",
        "dormant:fallen:disciple",
        "rift_valley",
        high_bit_seed,
        1_000,
    );
    persist_pending_dormant_relic(&settings, &record).expect("persist should succeed");

    let loaded = load_pending_dormant_relics_for_zone(&settings, "rift_valley")
        .expect("load should succeed");
    assert_eq!(
        loaded.len(),
        1,
        "exactly one pending relic should be persisted for the zone, got {}",
        loaded.len()
    );
    assert_eq!(
        loaded[0], record,
        "the loaded pending relic must field-for-field equal what was persisted \
         (including the high-bit u64 loot_seed surviving the i64 round-trip); got {:?}",
        loaded[0]
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn pending_relic_load_filters_by_zone() {
    // load_pending_dormant_relics_for_zone 只返回**该 zone** 的遗物——玩家在 A zone 不该
    // 物化 B zone 的战场遗物。
    let (settings, root) = persistence_settings("pending-relic-zone-filter");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    persist_pending_dormant_relic(
        &settings,
        &pending_relic_record("r-a1", "c1", "rift_valley", 11, 100),
    )
    .unwrap();
    persist_pending_dormant_relic(
        &settings,
        &pending_relic_record("r-a2", "c2", "rift_valley", 22, 200),
    )
    .unwrap();
    persist_pending_dormant_relic(
        &settings,
        &pending_relic_record("r-b1", "c3", "north_wastes", 33, 150),
    )
    .unwrap();

    let rift = load_pending_dormant_relics_for_zone(&settings, "rift_valley").unwrap();
    assert_eq!(
        rift.len(),
        2,
        "rift_valley must return exactly its 2 relics, not the north_wastes one, got {}",
        rift.len()
    );
    // 排序确定性：按 created_wall ASC（r-a1 created_wall=100 在 r-a2 created_wall=200 前）。
    assert_eq!(
        rift[0].relic_id, "r-a1",
        "relics must come back ordered by created_wall ASC for deterministic hydrate, got first={}",
        rift[0].relic_id
    );
    let north = load_pending_dormant_relics_for_zone(&settings, "north_wastes").unwrap();
    assert_eq!(north.len(), 1, "north_wastes must return only its 1 relic");
    let empty = load_pending_dormant_relics_for_zone(&settings, "spawn").unwrap();
    assert!(
        empty.is_empty(),
        "a zone with no relics must return an empty vec, got {} rows",
        empty.len()
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn pending_relic_delete_removes_only_target_row() {
    // 消费后删除：玩家拾走遗物 → delete 该 relic_id → 它不再被 load（不二次物化），
    // 但同 zone 其它遗物保留。
    let (settings, root) = persistence_settings("pending-relic-delete");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    persist_pending_dormant_relic(
        &settings,
        &pending_relic_record("keep", "c1", "rift_valley", 1, 100),
    )
    .unwrap();
    persist_pending_dormant_relic(
        &settings,
        &pending_relic_record("drop", "c2", "rift_valley", 2, 100),
    )
    .unwrap();

    delete_pending_dormant_relic(&settings, "drop").expect("delete should succeed");

    let remaining = load_pending_dormant_relics_for_zone(&settings, "rift_valley").unwrap();
    assert_eq!(
        remaining.len(),
        1,
        "after deleting one relic, exactly one must remain, got {}",
        remaining.len()
    );
    assert_eq!(
        remaining[0].relic_id, "keep",
        "delete must remove only the targeted relic_id, leaving 'keep'; got {}",
        remaining[0].relic_id
    );
    // 删一个不存在的 relic_id 必须无害（幂等：玩家重复拾取请求不应炸）。
    delete_pending_dormant_relic(&settings, "nonexistent")
        .expect("deleting a missing relic must be a no-op, not an error");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn relic_ttl_swept_after_expiry_off_by_one() {
    // TTL sweep 边界（off-by-one）：阈值 = now - PENDING_RELIC_RETENTION_SECS。
    //   - created_wall == threshold-1（早于阈值 1 秒，已过期）→ 必被清；
    //   - created_wall == threshold（恰好等于阈值，DELETE 条件是 `< threshold`）→ 必保留；
    //   - created_wall == threshold+1（晚于阈值，更新鲜）→ 必保留。
    let (settings, root) = persistence_settings("pending-relic-ttl-sweep");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let now_wall = 1_000_000i64;
    let threshold = now_wall - PENDING_RELIC_RETENTION_SECS;
    persist_pending_dormant_relic(
        &settings,
        &pending_relic_record("expired", "c1", "rift_valley", 1, threshold - 1),
    )
    .unwrap();
    persist_pending_dormant_relic(
        &settings,
        &pending_relic_record("boundary", "c2", "rift_valley", 2, threshold),
    )
    .unwrap();
    persist_pending_dormant_relic(
        &settings,
        &pending_relic_record("fresh", "c3", "rift_valley", 3, threshold + 1),
    )
    .unwrap();

    let removed = sweep_stale_dormant_relics(&settings, now_wall).expect("sweep should succeed");
    assert_eq!(
        removed, 1,
        "exactly the one relic created strictly before the threshold must be swept, got {removed}"
    );

    let remaining = load_pending_dormant_relics_for_zone(&settings, "rift_valley").unwrap();
    let ids: HashSet<&str> = remaining.iter().map(|r| r.relic_id.as_str()).collect();
    assert!(
        !ids.contains("expired"),
        "the expired relic (created_wall < threshold) must be swept, but it survived"
    );
    assert!(
        ids.contains("boundary"),
        "the boundary relic (created_wall == threshold) must NOT be swept — DELETE is strict `< threshold`, off-by-one would wrongly drop it"
    );
    assert!(
        ids.contains("fresh"),
        "the fresh relic (created_wall > threshold) must survive the sweep"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn persist_pending_relic_system_consumes_event_into_sqlite() {
    // 端到端 system 级：emit 一个 PendingDormantRelicCreated event → 跑 persist system →
    // sqlite 出现对应行（zone/pos/archetype/loot_seed 正确）。这把"combat phase emit"与
    // "持久层落盘"之间的消费契约锁住，真实 impl 只换 emit 源不动这条断言。
    let (settings, root) = persistence_settings("pending-relic-system");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let mut app = App::new();
    app.insert_resource(settings.clone());
    app.add_event::<crate::npc::dormant::PendingDormantRelicCreated>();
    app.add_systems(Update, persist_pending_dormant_relics_system);

    app.world_mut()
        .send_event(crate::npc::dormant::PendingDormantRelicCreated {
            char_id: "dormant:fallen:elder".to_string(),
            zone: "qingyun_peaks".to_string(),
            position: [42.0, 70.0, -13.5],
            archetype: crate::npc::lifecycle::NpcArchetype::GuardianRelic,
            loot_seed: 0xABCD_1234_5678_9F00,
            created_tick: 99,
        });
    app.update();

    let loaded = load_pending_dormant_relics_for_zone(&settings, "qingyun_peaks")
        .expect("load should succeed");
    assert_eq!(
        loaded.len(),
        1,
        "the persist system must write exactly one sqlite row from one event, got {}",
        loaded.len()
    );
    let row = &loaded[0];
    assert_eq!(row.char_id, "dormant:fallen:elder");
    assert_eq!(
        row.archetype,
        crate::npc::lifecycle::NpcArchetype::GuardianRelic.as_str(),
        "system must store archetype via as_str() so hydrate can from_str() it back"
    );
    assert_eq!(
        row.loot_seed, 0xABCD_1234_5678_9F00,
        "loot_seed must survive the event→sqlite path losslessly for deterministic re-roll"
    );
    assert_eq!(
        (row.pos_x, row.pos_y, row.pos_z),
        (42.0, 70.0, -13.5),
        "relic position must be stored exactly (split into pos_x/y/z)"
    );
    assert_eq!(
        row.created_tick, 99,
        "created_tick must carry the settlement tick for deferred-on-hydrate ordering"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn persist_pending_relic_system_is_idempotent_for_same_logical_death() {
    // plan-offscreen-war-v1 P3 review-fix（CodeRabbit）：relic_id 现为确定性复合键
    // （char_id + created_tick + loot_seed）。同一逻辑战死的事件即便 emit / persist 两次
    // （重发、重试），也只产出**一行**（ON CONFLICT(relic_id) DO UPDATE 覆盖而非插重复），
    // 不再像随机 UUID 那样每次造一行孤儿。
    let (settings, root) = persistence_settings("pending-relic-idempotent");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let make_event = || crate::npc::dormant::PendingDormantRelicCreated {
        char_id: "dormant:fallen:disciple".to_string(),
        zone: "rift_valley".to_string(),
        position: [1.0, 64.0, 2.0],
        archetype: crate::npc::lifecycle::NpcArchetype::Disciple,
        loot_seed: 0xDEAD_BEEF_0000_0001,
        created_tick: 123,
    };
    // 先确认确定性 relic_id 对同一逻辑死亡稳定（与 created_wall 墙钟无关）。
    assert_eq!(
        deterministic_relic_id(&make_event()),
        deterministic_relic_id(&make_event()),
        "deterministic_relic_id must be stable for the same logical death (char_id+created_tick+loot_seed) so re-emits dedupe via the PK; two calls differed"
    );

    // 跑两次 persist system（两帧），各 emit 同一逻辑死亡事件。
    for _ in 0..2 {
        let mut app = App::new();
        app.insert_resource(settings.clone());
        app.add_event::<crate::npc::dormant::PendingDormantRelicCreated>();
        app.add_systems(Update, persist_pending_dormant_relics_system);
        app.world_mut().send_event(make_event());
        app.update();
    }

    let loaded = load_pending_dormant_relics_for_zone(&settings, "rift_valley")
        .expect("load should succeed");
    assert_eq!(
        loaded.len(),
        1,
        "persisting the SAME logical death twice must leave exactly ONE row (deterministic relic_id + ON CONFLICT upsert dedupe), not duplicate orphans; got {} rows",
        loaded.len()
    );
    // plan-offscreen-war-v1 P3 二修（CodeRabbit Major）：只断言**可观察行为**，不绑死 relic_id
    // 的精确编码格式。换一种等价 id 编码不该让本测试无谓变红——去重契约由上面的 len==1 锁住，
    // id 的稳定性已由开头的 `deterministic_relic_id(a)==deterministic_relic_id(a)` 锁住。这里只
    // 要求落库行带一个**非空** relic_id（PK 不能空）且**业务字段**与发出的事件一致。
    let row = &loaded[0];
    assert!(
        !row.relic_id.is_empty(),
        "the single persisted row must carry a non-empty relic_id (it is the primary key); got an empty string"
    );
    let event = make_event();
    assert_eq!(
        row.char_id, event.char_id,
        "the persisted row's char_id must match the emitted logical death's char_id; got {} expected {}",
        row.char_id, event.char_id
    );
    assert_eq!(
        row.created_tick as u64, event.created_tick,
        "the persisted row's created_tick must match the emitted event's settlement tick; got {} expected {}",
        row.created_tick, event.created_tick
    );
    assert_eq!(
        row.loot_seed, event.loot_seed,
        "the persisted row's loot_seed must match the emitted event's loot_seed (drives deterministic re-roll); got {:#x} expected {:#x}",
        row.loot_seed, event.loot_seed
    );
    assert_eq!(
        row.zone, event.zone,
        "the persisted row's zone must match the emitted event's zone; got {} expected {}",
        row.zone, event.zone
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn pending_relic_reupsert_keeps_earliest_created_wall() {
    // plan-offscreen-war-v1 P3 二修（CodeRabbit Major）：同一 relic_id 二次 persist（第二次
    // created_wall 更晚，模拟重发 / 重试），落库的 created_wall 必须仍是**更早**那个。
    //
    // 回归面：`upsert_pending_dormant_relic` 的 ON CONFLICT 旧 `created_wall = excluded.created_wall`
    // 会把墙钟覆盖成新事件的（更晚）值。created_wall 是 TTL retention sweep 的判定锚点
    // （`sweep_stale_dormant_relics` 删 `created_wall < now - RETENTION`）：覆盖成更晚墙钟 =
    // 给陈旧遗物无限续命，「幂等」只对去重成立、不对可观察 TTL 成立。本测试固定 created_wall
    // 走 `persist_pending_dormant_relic`（精确控制，不依赖 current_unix_seconds），断言取 MIN。
    let (settings, root) = persistence_settings("pending-relic-reupsert-earliest-wall");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    const EARLY_WALL: i64 = 1_000;
    const LATE_WALL: i64 = 5_000;
    // 首次 persist：早墙钟。
    persist_pending_dormant_relic(
        &settings,
        &pending_relic_record("relic-ttl", "c1", "rift_valley", 7, EARLY_WALL),
    )
    .expect("first persist should succeed");
    // 同一 relic_id 二次 persist：晚墙钟（重发 / 重试场景）。
    persist_pending_dormant_relic(
        &settings,
        &pending_relic_record("relic-ttl", "c1", "rift_valley", 7, LATE_WALL),
    )
    .expect("re-persist should succeed");

    let loaded = load_pending_dormant_relics_for_zone(&settings, "rift_valley")
        .expect("load should succeed");
    assert_eq!(
        loaded.len(),
        1,
        "二次 upsert 同一 relic_id 必须仍只有一行（ON CONFLICT 覆盖、不插重复），got {}",
        loaded.len()
    );
    assert_eq!(
        loaded[0].created_wall, EARLY_WALL,
        "二次 persist（晚墙钟 {LATE_WALL}）后 created_wall 必须仍是更早的 {EARLY_WALL}——\
         ON CONFLICT 取 MIN(existing, excluded)，绝不刷新 TTL（否则陈旧遗物被无限续命，sweep \
         永远删不掉）；实际落库 {}",
        loaded[0].created_wall
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn v26_migration_rejects_pending_dormant_relics_with_missing_column() {
    // plan-offscreen-war-v1 P3 review-fix（CodeRabbit）：v26 迁移护栏必须拒绝**列残缺**的
    // 已有 pending_dormant_relics 表（仿 v20 high_renown 护栏失败用例）。预置一个缺
    // schema_version/loot_seed/created_tick 的部分表 + user_version=25，跑迁移必拒绝放行残表
    // （残表会让运行时 upsert/load 撞列名错误）。保留 zone/created_wall 让两个 CREATE INDEX
    // 先建成功，从而精确命中 assert_pending_dormant_relics_schema_ready 的「column ... missing」
    // 护栏（而非更早的 CREATE INDEX 列缺失错误）。
    let db_path = database_path("v26-relic-missing-column");
    fs::create_dir_all(db_path.parent().expect("db path should have parent"))
        .expect("temp db parent should be created");
    let mut connection = Connection::open(&db_path).expect("db should open");
    connection
        .execute_batch(
            "
            CREATE TABLE pending_dormant_relics (
                relic_id TEXT NOT NULL,
                char_id TEXT NOT NULL,
                zone TEXT NOT NULL,
                pos_x REAL NOT NULL,
                pos_y REAL NOT NULL,
                pos_z REAL NOT NULL,
                archetype TEXT NOT NULL,
                created_wall INTEGER NOT NULL,
                PRIMARY KEY (relic_id)
            );
            PRAGMA user_version = 25;
            ",
        )
        .expect("partial-schema relic fixture should be created");

    let error = apply_migrations(&mut connection).expect_err(
        "v26 migration must reject a pending_dormant_relics table missing required columns",
    );
    let message = error.to_string();
    assert!(
        message.contains("pending_dormant_relics column") && message.contains("missing"),
        "expected a 'pending_dormant_relics column ... missing' guard error because the pre-existing table lacks required columns (loot_seed/created_tick/schema_version); got: {message}"
    );
    let _ = fs::remove_dir_all(db_path.parent().expect("db path should have parent"));
}

#[test]
fn v26_migration_rejects_pending_dormant_relics_with_wrong_primary_key() {
    // plan-offscreen-war-v1 P3 review-fix（CodeRabbit）：v26 迁移护栏必须拒绝**主键错误**的
    // 已有 pending_dormant_relics 表。预置全列齐但 PK 设成 (char_id) 而非 (relic_id) +
    // user_version=25，跑迁移必报「primary key mismatch」——错 PK 会让确定性 relic_id 的
    // ON CONFLICT 去重失效。
    let db_path = database_path("v26-relic-wrong-pk");
    fs::create_dir_all(db_path.parent().expect("db path should have parent"))
        .expect("temp db parent should be created");
    let mut connection = Connection::open(&db_path).expect("db should open");
    connection
        .execute_batch(
            "
            CREATE TABLE pending_dormant_relics (
                relic_id TEXT NOT NULL,
                char_id TEXT NOT NULL,
                zone TEXT NOT NULL,
                pos_x REAL NOT NULL,
                pos_y REAL NOT NULL,
                pos_z REAL NOT NULL,
                archetype TEXT NOT NULL,
                loot_seed INTEGER NOT NULL,
                created_tick INTEGER NOT NULL,
                created_wall INTEGER NOT NULL,
                schema_version INTEGER NOT NULL,
                PRIMARY KEY (char_id)
            );
            PRAGMA user_version = 25;
            ",
        )
        .expect("wrong-PK relic fixture should be created");

    let error = apply_migrations(&mut connection)
        .expect_err("v26 migration must reject a pending_dormant_relics table whose primary key is not relic_id");
    let message = error.to_string();
    assert!(
        message.contains("pending_dormant_relics primary key mismatch"),
        "expected a 'pending_dormant_relics primary key mismatch' guard error because the pre-existing table keys on char_id not relic_id (which would break ON CONFLICT dedupe); got: {message}"
    );
    let _ = fs::remove_dir_all(db_path.parent().expect("db path should have parent"));
}

#[test]
fn sweep_relic_system_throttles_between_intervals_then_sweeps() {
    // sweep system 限频契约：首次跑（last_sweep_wall==0）必执行；紧接着再跑（间隔 < 阈值）
    // 必跳过（不重复扫）。这把"每帧都开连接 DELETE"的浪费挡住，同时保证首扫一定发生。
    let (settings, root) = persistence_settings("pending-relic-sweep-throttle");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let mut app = App::new();
    app.insert_resource(settings.clone());
    app.insert_resource(DormantRelicSweepState::default());
    app.add_systems(Update, sweep_dormant_relic_retention_system);

    // 首帧：last_sweep_wall 从 0 起，sweep 必执行 → state 被更新成非 0。
    app.update();
    let after_first = app
        .world()
        .resource::<DormantRelicSweepState>()
        .last_sweep_wall;
    assert!(
        after_first > 0,
        "first sweep must run and stamp last_sweep_wall (>0), got {after_first}"
    );

    // 紧接着第二帧（墙钟几乎没动，间隔 < PENDING_RELIC_SWEEP_INTERVAL_SECS）：必跳过 →
    // last_sweep_wall 不变（throttle 生效）。
    app.update();
    let after_second = app
        .world()
        .resource::<DormantRelicSweepState>()
        .last_sweep_wall;
    assert_eq!(
        after_first, after_second,
        "second consecutive sweep within the throttle window must be skipped (last_sweep_wall unchanged); \
         got {after_first} then {after_second}"
    );
    let _ = fs::remove_dir_all(root);
}

// ─── plan-coffin-tiers-v1 P0 charge #5 — v27 migration test ──────────────

#[test]
fn v27_migration_adds_coffin_grade_to_legacy_player_lifespan_table() {
    let db_path = database_path("v27-player-lifespan-coffin-grade");
    fs::create_dir_all(db_path.parent().expect("db path should have parent"))
        .expect("temp db parent should be created");
    let mut connection = Connection::open(&db_path).expect("db should open");

    // 建立 v26 前状态：player_lifespan 有 in_coffin 但无 coffin_grade，模拟真实升级场景
    connection
        .execute_batch(
            "
            CREATE TABLE player_lifespan (
                username TEXT PRIMARY KEY,
                born_at_tick INTEGER NOT NULL CHECK (born_at_tick >= 0),
                years_lived REAL NOT NULL CHECK (years_lived >= 0),
                cap_by_realm INTEGER NOT NULL CHECK (cap_by_realm > 0),
                offline_pause_wall INTEGER NOT NULL CHECK (offline_pause_wall >= 0),
                in_coffin INTEGER NOT NULL DEFAULT 0,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            INSERT INTO player_lifespan (
                username, born_at_tick, years_lived, cap_by_realm,
                offline_pause_wall, in_coffin, schema_version, last_updated_wall
            ) VALUES ('Azure', 0, 5.0, 80, 0, 1, 1, 0);
            PRAGMA user_version = 26;
            ",
        )
        .expect("legacy v26 player_lifespan fixture should create");

    apply_migrations(&mut connection).expect("v27 migration should succeed");

    // 1. coffin_grade 列已加
    let mut statement = connection
        .prepare("PRAGMA table_info(player_lifespan)")
        .expect("player_lifespan table_info should prepare");
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .expect("player_lifespan table_info should query")
        .collect::<Result<Vec<_>, _>>()
        .expect("player_lifespan columns should collect");
    assert!(
        columns.iter().any(|col| col == "coffin_grade"),
        "player_lifespan should have coffin_grade column after v27 migration, \
         columns: {columns:?}"
    );

    // 2. 旧行默认 'mundane'
    let grade: String = connection
        .query_row(
            "SELECT coffin_grade FROM player_lifespan WHERE username = 'Azure'",
            [],
            |row| row.get(0),
        )
        .expect("legacy row should have default coffin_grade after migration");
    assert_eq!(
        grade, "mundane",
        "legacy row (no coffin_grade) should default to 'mundane' after v27 migration, \
         got '{grade}'"
    );

    // 3. columns.is_empty() 空表分支：再运行一次 v27 对空表不应报错
    let db_path2 = database_path("v27-empty-table-branch");
    fs::create_dir_all(db_path2.parent().expect("db path should have parent"))
        .expect("temp db parent should be created");
    let mut conn2 = Connection::open(&db_path2).expect("db2 should open");
    conn2
        .execute_batch("PRAGMA user_version = 26;")
        .expect("set v26");
    apply_migrations(&mut conn2).expect("v27 migration on fresh db should succeed");
    let version2: i32 = conn2
        .query_row("PRAGMA user_version;", [], |row| row.get(0))
        .expect("user_version should be readable");
    assert_eq!(
        version2, CURRENT_USER_VERSION,
        "user_version after v27 migration on fresh db should be CURRENT_USER_VERSION"
    );
}

// ── plan-territory-v1 P0：zone_influence 持久化 round-trip ───────────────

#[test]
fn zone_influence_persistence_round_trip() {
    use crate::world::territory::{
        InfluenceSources, PlayerInfluence, ZoneDominance, ZoneInfluenceEntry, ZoneInfluenceMap,
    };

    let (settings, root) = persistence_settings("zone-influence-roundtrip");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    // 构造 ZoneInfluenceMap：2 个 zone，其中 spawn 有霸主
    let mut influence_map = ZoneInfluenceMap::default();

    // zone 1: "spawn" — 有两个玩家，p1 是霸主
    let mut spawn_entry = ZoneInfluenceEntry::default();
    spawn_entry.players.insert(
        "offline:HeroA".to_string(),
        PlayerInfluence {
            value: 45.5,
            last_activity_tick: 12000,
            source_breakdown: InfluenceSources {
                meditation_ticks: 200,
                combat_wins: 5,
                player_kills: 1,
                gather_count: 10,
                continuous_sessions: 3,
            },
        },
    );
    spawn_entry.players.insert(
        "offline:RivalB".to_string(),
        PlayerInfluence {
            value: 30.0,
            last_activity_tick: 11000,
            source_breakdown: InfluenceSources {
                meditation_ticks: 100,
                ..Default::default()
            },
        },
    );
    spawn_entry.dominant = Some(ZoneDominance {
        char_id: "offline:HeroA".to_string(),
        influence: 45.5,
        established_tick: 8000,
        public_known: true,
        realm_band: None,
    });
    influence_map.zones.insert("spawn".to_string(), spawn_entry);

    // zone 2: "wilderness" — 单玩家无霸主
    let mut wild_entry = ZoneInfluenceEntry::default();
    wild_entry.players.insert(
        "offline:Wanderer".to_string(),
        PlayerInfluence {
            value: 5.0,
            last_activity_tick: 500,
            source_breakdown: Default::default(),
        },
    );
    influence_map
        .zones
        .insert("wilderness".to_string(), wild_entry);

    // 持久化
    persist_zone_influence_snapshot(&settings, &influence_map)
        .expect("zone influence snapshot should persist");

    // 读回
    let records =
        load_zone_influence_snapshot(&settings).expect("zone influence snapshot should load");
    assert_eq!(
        records.len(),
        3,
        "应有 3 条记录（spawn×2 + wilderness×1），实际 {}",
        records.len()
    );

    // 找霸主行
    let hero_record = records
        .iter()
        .find(|r| r.zone_id == "spawn" && r.char_id == "offline:HeroA")
        .expect("应有 HeroA 记录");
    assert!(
        (hero_record.value - 45.5).abs() < 1e-9,
        "value 应为 45.5，实际 {}",
        hero_record.value
    );
    assert_eq!(
        hero_record.meditation_ticks, 200,
        "meditation_ticks 应为 200"
    );
    assert_eq!(hero_record.combat_wins, 5, "combat_wins 应为 5");
    assert_eq!(hero_record.player_kills, 1, "player_kills 应为 1");
    assert_eq!(hero_record.gather_count, 10, "gather_count 应为 10");
    assert_eq!(
        hero_record.continuous_sessions, 3,
        "continuous_sessions 应为 3"
    );
    assert_eq!(
        hero_record.last_activity_tick, 12000,
        "last_activity_tick 应为 12000"
    );
    assert!(hero_record.dominant, "HeroA 应标记为 dominant=true");
    assert_eq!(
        hero_record.established_tick, 8000,
        "established_tick 应为 8000"
    );
    assert!(hero_record.public_known, "HeroA public_known 应为 true");

    // 非霸主行
    let rival_record = records
        .iter()
        .find(|r| r.zone_id == "spawn" && r.char_id == "offline:RivalB")
        .expect("应有 RivalB 记录");
    assert!(!rival_record.dominant, "RivalB 不应是霸主");

    // 空表分支：wilderness 无霸主
    let wanderer_record = records
        .iter()
        .find(|r| r.zone_id == "wilderness" && r.char_id == "offline:Wanderer")
        .expect("应有 Wanderer 记录");
    assert!(!wanderer_record.dominant, "Wanderer 不是霸主");

    // 全量快照必须替换旧集合，而不是仅 upsert 当前键；删除 wilderness 与 RivalB
    // 后，下一次 hydrate 不得复活这两条陈旧行。
    influence_map
        .zones
        .get_mut("spawn")
        .expect("spawn fixture should remain present")
        .players
        .remove("offline:RivalB");
    influence_map.zones.remove("wilderness");
    persist_zone_influence_snapshot(&settings, &influence_map)
        .expect("replacement zone influence snapshot should persist");
    let replacement_records =
        load_zone_influence_snapshot(&settings).expect("replacement snapshot should load");
    assert_eq!(
        replacement_records.len(),
        1,
        "removed influence rows must not survive a full snapshot replacement"
    );
    assert_eq!(
        replacement_records[0].char_id, "offline:HeroA",
        "the surviving snapshot row must be the current HeroA entry"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn zone_influence_load_empty_returns_empty_vec() {
    let (settings, root) = persistence_settings("zone-influence-empty");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    let records = load_zone_influence_snapshot(&settings).expect("空表 load 应返回空 Vec");
    assert!(
        records.is_empty(),
        "空表 load 应返回 [], 实际 {:?}",
        records
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn zone_influence_hydrate_restores_dominant() {
    use crate::world::territory::{
        InfluenceSources, PlayerInfluence, ZoneDominance, ZoneInfluenceEntry, ZoneInfluenceMap,
    };

    let (settings, root) = persistence_settings("zone-influence-hydrate");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("bootstrap should succeed");

    // 持久化一个有霸主的 zone
    let mut map = ZoneInfluenceMap::default();
    let mut entry = ZoneInfluenceEntry::default();
    entry.players.insert(
        "offline:King".to_string(),
        PlayerInfluence {
            value: 60.0,
            last_activity_tick: 9999,
            source_breakdown: InfluenceSources {
                meditation_ticks: 500,
                ..Default::default()
            },
        },
    );
    entry.dominant = Some(ZoneDominance {
        char_id: "offline:King".to_string(),
        influence: 60.0,
        established_tick: 5000,
        public_known: false,
        realm_band: None,
    });
    map.zones.insert("throne_hall".to_string(), entry);

    persist_zone_influence_snapshot(&settings, &map)
        .expect("zone influence snapshot should persist");

    // hydrate 到新的 map
    let mut restored = ZoneInfluenceMap::default();
    let count = hydrate_zone_influence(&settings, &mut restored).expect("hydrate 应成功");
    assert_eq!(count, 1, "应 hydrate 1 条记录");

    let zone_entry = restored
        .zones
        .get("throne_hall")
        .expect("throne_hall 应存在");
    let king = zone_entry.players.get("offline:King").expect("King 应存在");
    assert!(
        (king.value - 60.0).abs() < 1e-9,
        "value 应为 60.0，实际 {}",
        king.value
    );
    assert_eq!(king.last_activity_tick, 9999);

    let dom = zone_entry.dominant.as_ref().expect("应有霸主");
    assert_eq!(dom.char_id, "offline:King", "霸主应为 King");
    assert_eq!(dom.established_tick, 5000);
    assert!(!dom.public_known, "public_known 应为 false");

    let _ = fs::remove_dir_all(root);
}
#[test]
fn v28_migration_adds_spirit_niche_damage_flag_with_default_false() {
    let db_path = database_path("v28-spirit-niche-damage-flag");
    fs::create_dir_all(db_path.parent().expect("db path should have parent"))
        .expect("temp db parent should be created");
    let mut connection = Connection::open(&db_path).expect("db should open");

    connection
        .execute_batch(
            "
            CREATE TABLE social_spirit_niches (
                owner TEXT PRIMARY KEY,
                pos_x INTEGER NOT NULL,
                pos_y INTEGER NOT NULL,
                pos_z INTEGER NOT NULL,
                placed_at_tick INTEGER NOT NULL CHECK (placed_at_tick >= 0),
                revealed INTEGER NOT NULL CHECK (revealed IN (0, 1)),
                revealed_by TEXT,
                defense_mode TEXT,
                guardians_json TEXT NOT NULL DEFAULT '[]',
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            INSERT INTO social_spirit_niches (
                owner, pos_x, pos_y, pos_z, placed_at_tick, revealed, revealed_by,
                guardians_json, schema_version, last_updated_wall
            ) VALUES ('char:owner', 10, 64, 10, 1, 0, NULL, '[]', 1, 0);
            PRAGMA user_version = 27;
            ",
        )
        .expect("legacy v27 social_spirit_niches fixture should create");

    apply_migrations(&mut connection).expect("v28 migration should succeed");

    let mut statement = connection
        .prepare("PRAGMA table_info(social_spirit_niches)")
        .expect("social_spirit_niches table_info should prepare");
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .expect("social_spirit_niches table_info should query")
        .collect::<Result<Vec<_>, _>>()
        .expect("social_spirit_niches columns should collect");
    assert!(
        columns.iter().any(|col| col == "is_damaged"),
        "social_spirit_niches should have is_damaged after v28 migration; columns: {columns:?}"
    );
    let is_damaged: i64 = connection
        .query_row(
            "SELECT is_damaged FROM social_spirit_niches WHERE owner = 'char:owner'",
            [],
            |row| row.get(0),
        )
        .expect("legacy row should keep default is_damaged");
    assert_eq!(
        is_damaged, 0,
        "legacy spirit niche row should default to undamaged after v28 migration"
    );
    let user_version: i32 = connection
        .query_row("PRAGMA user_version;", [], |row| row.get(0))
        .expect("user_version should be readable");
    assert_eq!(user_version, CURRENT_USER_VERSION);
}

// ── plan-faction-expansion-v1 P0：v30 migration 单测 ────────────────────────

#[test]
fn test_migration_v29_to_v30_real_migrate() {
    // 防孤岛 #3：建 user_version=29 库 + social_faction_memberships 塞 attack/defend/neutral
    // 三行→open_database→断言 named_faction 列各=对应具名势力且 user_version==30（真迁移验证）。
    let db_path = database_path("v30-named-faction-migration");
    fs::create_dir_all(db_path.parent().expect("db path parent"))
        .expect("temp db dir should create");
    let mut connection = Connection::open(&db_path).expect("db should open");

    connection
        .execute_batch(
            "
            CREATE TABLE IF NOT EXISTS social_faction_memberships (
                char_id TEXT PRIMARY KEY,
                faction TEXT,
                rank INTEGER NOT NULL DEFAULT 0,
                loyalty INTEGER NOT NULL DEFAULT 0,
                betrayal_count INTEGER NOT NULL DEFAULT 0,
                invite_block_until_tick INTEGER,
                permanently_refused INTEGER NOT NULL DEFAULT 0,
                schema_version INTEGER NOT NULL DEFAULT 1,
                last_updated_wall INTEGER NOT NULL DEFAULT 0
            );
            INSERT INTO social_faction_memberships (char_id, faction, rank, loyalty, betrayal_count, permanently_refused, schema_version, last_updated_wall)
                VALUES ('char:hunter', 'attack', 1, 50, 0, 0, 1, 0);
            INSERT INTO social_faction_memberships (char_id, faction, rank, loyalty, betrayal_count, permanently_refused, schema_version, last_updated_wall)
                VALUES ('char:merchant', 'defend', 1, 60, 0, 0, 1, 0);
            INSERT INTO social_faction_memberships (char_id, faction, rank, loyalty, betrayal_count, permanently_refused, schema_version, last_updated_wall)
                VALUES ('char:drifter', 'neutral', 0, 40, 0, 0, 1, 0);
            PRAGMA user_version = 29;
            ",
        )
        .expect("v29 social_faction_memberships fixture should create");

    apply_migrations(&mut connection).expect("v30 migration should succeed");

    let hunter_named: Option<String> = connection
        .query_row(
            "SELECT named_faction FROM social_faction_memberships WHERE char_id = 'char:hunter'",
            [],
            |row| row.get(0),
        )
        .expect("char:hunter row should exist");
    assert_eq!(
        hunter_named.as_deref(),
        Some("qingyun_hunters"),
        "attack faction 必须迁移到 qingyun_hunters，实际 {:?}（防孤岛 #3：真迁移现有 FactionId 持久化数据）",
        hunter_named
    );

    let merchant_named: Option<String> = connection
        .query_row(
            "SELECT named_faction FROM social_faction_memberships WHERE char_id = 'char:merchant'",
            [],
            |row| row.get(0),
        )
        .expect("char:merchant row should exist");
    assert_eq!(
        merchant_named.as_deref(),
        Some("cangyuan_merchants"),
        "defend faction 必须迁移到 cangyuan_merchants，实际 {:?}",
        merchant_named
    );

    let drifter_named: Option<String> = connection
        .query_row(
            "SELECT named_faction FROM social_faction_memberships WHERE char_id = 'char:drifter'",
            [],
            |row| row.get(0),
        )
        .expect("char:drifter row should exist");
    assert_eq!(
        drifter_named.as_deref(),
        Some("north_waste_drifters"),
        "neutral faction 必须迁移到 north_waste_drifters，实际 {:?}",
        drifter_named
    );

    let user_version: i32 = connection
        .query_row("PRAGMA user_version;", [], |row| row.get(0))
        .expect("user_version should be readable");
    assert_eq!(
        user_version, CURRENT_USER_VERSION,
        "v30 迁移完成后 user_version 必须是 CURRENT_USER_VERSION={CURRENT_USER_VERSION}，实际 {user_version}"
    );
}

#[test]
fn test_migration_v30_idempotent() {
    // 幂等性：已 v30 库再 open 不重复 ALTER/不报错（named_faction IS NULL 守卫）。
    let db_path = database_path("v30-idempotent");
    fs::create_dir_all(db_path.parent().expect("db path parent"))
        .expect("temp db dir should create");
    let mut connection = Connection::open(&db_path).expect("db should open");

    // 先跑一次到 v30。
    connection
        .execute_batch(
            "
            CREATE TABLE IF NOT EXISTS social_faction_memberships (
                char_id TEXT PRIMARY KEY,
                faction TEXT,
                rank INTEGER NOT NULL DEFAULT 0,
                loyalty INTEGER NOT NULL DEFAULT 0,
                betrayal_count INTEGER NOT NULL DEFAULT 0,
                invite_block_until_tick INTEGER,
                permanently_refused INTEGER NOT NULL DEFAULT 0,
                schema_version INTEGER NOT NULL DEFAULT 1,
                last_updated_wall INTEGER NOT NULL DEFAULT 0
            );
            INSERT INTO social_faction_memberships (char_id, faction, rank, loyalty, betrayal_count, permanently_refused, schema_version, last_updated_wall)
                VALUES ('char:a', 'attack', 0, 0, 0, 0, 1, 0);
            PRAGMA user_version = 29;
            ",
        )
        .expect("v29 fixture should create");
    apply_migrations(&mut connection).expect("first v30 migration should succeed");

    // 再 apply 一次——必须不报错（named_faction 列已存在，UPDATE 守卫 IS NULL）。
    apply_migrations(&mut connection)
        .expect("second apply_migrations must be idempotent (v30 already applied)");

    let user_version: i32 = connection
        .query_row("PRAGMA user_version;", [], |row| row.get(0))
        .expect("user_version should be readable");
    assert_eq!(
        user_version, CURRENT_USER_VERSION,
        "幂等后 user_version 必须仍是 CURRENT_USER_VERSION={CURRENT_USER_VERSION}，实际 {user_version}"
    );
}

#[test]
fn v32_migration_creates_social_faction_reputations_table_with_constraints() {
    let db_path = database_path("v32-social-faction-reputations");
    fs::create_dir_all(db_path.parent().expect("db path parent"))
        .expect("temp db dir should create");
    let mut connection = Connection::open(&db_path).expect("db should open");
    connection
        .execute_batch("PRAGMA user_version = 31;")
        .expect("v31 fixture should set user_version");

    apply_migrations(&mut connection).expect("v32 migration should succeed");

    let mut statement = connection
        .prepare("PRAGMA table_info(social_faction_reputations)")
        .expect("social_faction_reputations table_info should prepare");
    let columns = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(1)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(5)?,
            ))
        })
        .expect("social_faction_reputations table_info should query")
        .collect::<Result<Vec<_>, _>>()
        .expect("social_faction_reputations columns should collect");
    drop(statement);
    let column_names: Vec<_> = columns.iter().map(|(name, _, _)| name.as_str()).collect();
    assert_eq!(
        column_names,
        vec![
            "char_id",
            "named_faction",
            "score",
            "schema_version",
            "last_updated_wall",
        ],
        "v32 migration 应创建固定列序，实际 columns={columns:?}"
    );
    let primary_key: Vec<_> = columns
        .iter()
        .filter_map(|(name, _, pk)| (*pk > 0).then_some((name.as_str(), *pk)))
        .collect();
    assert_eq!(
        primary_key,
        vec![("char_id", 1), ("named_faction", 2)],
        "social_faction_reputations 主键必须是 (char_id, named_faction)，实际 {primary_key:?}"
    );
    let not_null_columns: Vec<_> = columns
        .iter()
        .filter_map(|(name, not_null, _)| (*not_null != 0).then_some(name.as_str()))
        .collect();
    assert!(
        not_null_columns.contains(&"char_id") && not_null_columns.contains(&"named_faction"),
        "social_faction_reputations 身份列必须显式 NOT NULL，实际 not_null={not_null_columns:?}"
    );

    connection
        .execute(
            "
            INSERT INTO social_faction_reputations
                (char_id, named_faction, score, schema_version, last_updated_wall)
            VALUES (?1, ?2, ?3, 1, 0)
            ",
            params!["char:azure", "qingyun_hunters", 100],
        )
        .expect("valid faction reputation row should insert");
    connection
        .execute(
            "
            INSERT INTO social_faction_reputations
                (char_id, named_faction, score, schema_version, last_updated_wall)
            VALUES (?1, ?2, ?3, 1, 0)
            ",
            params!["char:azure-low", "qingyun_hunters", -100],
        )
        .expect("score=-100 是合法下界，必须可写入");
    let duplicate = connection.execute(
        "
        INSERT INTO social_faction_reputations
            (char_id, named_faction, score, schema_version, last_updated_wall)
        VALUES (?1, ?2, ?3, 1, 0)
        ",
        params!["char:azure", "qingyun_hunters", 10],
    );
    assert!(
        duplicate.is_err(),
        "重复 (char_id, named_faction) 必须被主键拒绝"
    );
    let out_of_range = connection.execute(
        "
        INSERT INTO social_faction_reputations
            (char_id, named_faction, score, schema_version, last_updated_wall)
        VALUES (?1, ?2, ?3, 1, 0)
        ",
        params!["char:azure", "cangyuan_merchants", 101],
    );
    assert!(
        out_of_range.is_err(),
        "score > 100 必须被 v32 CHECK 约束拒绝"
    );
    for (char_id, score, schema_version, last_updated_wall, hint) in [
        ("char:below-min", -101, 1, 0, "score < -100"),
        ("char:bad-schema", 0, 0, 0, "schema_version < 1"),
        ("char:bad-wall", 0, 1, -1, "last_updated_wall < 0"),
    ] {
        let rejected = connection.execute(
            "
            INSERT INTO social_faction_reputations
                (char_id, named_faction, score, schema_version, last_updated_wall)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ",
            params![
                char_id,
                "cangyuan_merchants",
                score,
                schema_version,
                last_updated_wall
            ],
        );
        assert!(rejected.is_err(), "{hint} 必须被 v32 CHECK 约束拒绝");
    }

    apply_migrations(&mut connection).expect("second v32 apply_migrations must be idempotent");
    let user_version: i32 = connection
        .query_row("PRAGMA user_version;", [], |row| row.get(0))
        .expect("user_version should be readable");
    assert_eq!(
        user_version, CURRENT_USER_VERSION,
        "v32 迁移完成后 user_version 必须是 CURRENT_USER_VERSION={CURRENT_USER_VERSION}，实际 {user_version}"
    );
}

#[test]
fn v32_migration_rejects_existing_social_faction_reputations_bad_schema() {
    let db_path = database_path("v32-social-faction-reputations-bad-schema");
    fs::create_dir_all(db_path.parent().expect("db path parent"))
        .expect("temp db dir should create");
    let mut connection = Connection::open(&db_path).expect("db should open");
    connection
        .execute_batch(
            "
            PRAGMA user_version = 31;
            CREATE TABLE social_faction_reputations (
                char_id             TEXT    NOT NULL PRIMARY KEY,
                named_faction       TEXT    NOT NULL,
                score               INTEGER NOT NULL,
                schema_version      INTEGER NOT NULL,
                last_updated_wall   INTEGER NOT NULL
            );
            ",
        )
        .expect("bad preexisting v31 table fixture should create");

    let error = apply_migrations(&mut connection)
        .expect_err("v32 migration must reject bad preexisting reputation schema");
    let error_text = format!("{error:?}");
    assert!(
        error_text.contains("social_faction_reputations primary key mismatch")
            || error_text.contains("social_faction_reputations CHECK"),
        "v32 guard must explain bad social_faction_reputations schema, actual {error_text}"
    );
    let user_version: i32 = connection
        .query_row("PRAGMA user_version;", [], |row| row.get(0))
        .expect("user_version should remain readable");
    assert_eq!(
        user_version, 31,
        "bad v32 schema must not advance user_version, actual {user_version}"
    );
}

#[test]
fn v32_migration_rejects_nullable_social_faction_reputation_identity_columns() {
    let db_path = database_path("v32-social-faction-reputations-nullable-identity");
    fs::create_dir_all(db_path.parent().expect("db path parent"))
        .expect("temp db dir should create");
    let mut connection = Connection::open(&db_path).expect("db should open");
    connection
        .execute_batch(
            "
            PRAGMA user_version = 31;
            CREATE TABLE social_faction_reputations (
                char_id             TEXT,
                named_faction       TEXT,
                score               INTEGER NOT NULL CHECK (score >= -100 AND score <= 100),
                schema_version      INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall   INTEGER NOT NULL CHECK (last_updated_wall >= 0),
                PRIMARY KEY (char_id, named_faction)
            );
            ",
        )
        .expect("nullable identity column fixture should create");

    let error = apply_migrations(&mut connection)
        .expect_err("v32 migration must reject nullable identity columns");
    let error_text = format!("{error:?}");
    assert!(
        error_text.contains("column char_id must be NOT NULL")
            || error_text.contains("column named_faction must be NOT NULL"),
        "v32 guard must reject nullable identity columns, actual {error_text}"
    );
    let user_version: i32 = connection
        .query_row("PRAGMA user_version;", [], |row| row.get(0))
        .expect("user_version should remain readable");
    assert_eq!(
        user_version, 31,
        "nullable v32 schema must not advance user_version, actual {user_version}"
    );
}

#[test]
fn v33_migration_creates_heartbeat_pseudo_veins_table_with_runtime_columns() {
    let db_path = database_path("v33-heartbeat-pseudo-veins");
    fs::create_dir_all(db_path.parent().expect("db path parent"))
        .expect("temp db dir should create");
    let mut connection = Connection::open(&db_path).expect("db should open");
    connection
        .execute_batch("PRAGMA user_version = 32;")
        .expect("v32 fixture should set user_version");

    apply_migrations(&mut connection).expect("v33 migration should succeed");

    let mut statement = connection
        .prepare("PRAGMA table_info(heartbeat_pseudo_veins)")
        .expect("heartbeat_pseudo_veins table_info should prepare");
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .expect("heartbeat_pseudo_veins table_info should query")
        .collect::<Result<Vec<_>, _>>()
        .expect("heartbeat_pseudo_veins columns should collect");
    assert_eq!(
        columns,
        vec![
            "zone_id",
            "dimension",
            "min_x",
            "min_y",
            "min_z",
            "max_x",
            "max_y",
            "max_z",
            "danger_level",
            "active_events_json",
            "patrol_anchors_json",
            "center_x",
            "center_z",
            "spawned_at_tick",
            "last_tick",
            "qi_current",
            "total_qi_consumed",
            "warning_sent",
            "dissipated",
            "season_at_spawn",
            "schema_version",
            "last_updated_wall",
            "observed_age_ticks",
            "pending_runtime_ticks",
            "pending_offline_ticks",
            "occupant_count",
            "eval_elapsed_ticks",
        ],
        "v33 migration 必须保存动态 zone 本体和 heartbeat lifecycle，实际 columns={columns:?}"
    );

    connection
        .execute(
            "
            INSERT INTO heartbeat_pseudo_veins (
                zone_id, dimension,
                min_x, min_y, min_z,
                max_x, max_y, max_z,
                danger_level,
                active_events_json,
                patrol_anchors_json,
                center_x, center_z,
                spawned_at_tick,
                last_tick,
                qi_current,
                total_qi_consumed,
                warning_sent,
                dissipated,
                season_at_spawn,
                schema_version,
                last_updated_wall
            ) VALUES (
                ?1, 'overworld',
                -1.0, 60.0, -1.0,
                1.0, 90.0, 1.0,
                4,
                '[\"pseudo_vein\"]',
                '[[0.0,65.0,0.0]]',
                0.0, 0.0,
                10,
                20,
                0.4,
                0.2,
                1,
                0,
                'summer_to_winter',
                1,
                0
            )
            ",
            params!["pseudo_vein_heartbeat_0"],
        )
        .expect("合法 heartbeat_pseudo_veins runtime 行应可写入");
    let bad_dimension = connection.execute(
        "
        INSERT INTO heartbeat_pseudo_veins (
            zone_id, dimension,
            min_x, min_y, min_z,
            max_x, max_y, max_z,
            danger_level,
            active_events_json,
            patrol_anchors_json,
            center_x, center_z,
            spawned_at_tick,
            last_tick,
            qi_current,
            total_qi_consumed,
            warning_sent,
            dissipated,
            season_at_spawn,
            schema_version,
            last_updated_wall
        ) VALUES (
            ?1, 'bad_dim',
            -1.0, 60.0, -1.0,
            1.0, 90.0, 1.0,
            4,
            '[\"pseudo_vein\"]',
            '[[0.0,65.0,0.0]]',
            0.0, 0.0,
            10,
            20,
            0.4,
            0.2,
            0,
            0,
            'summer',
            1,
            0
        )
        ",
        params!["pseudo_vein_bad_dim"],
    );
    assert!(
        bad_dimension.is_err(),
        "v33 heartbeat_pseudo_veins.dimension CHECK 必须拒绝未知维度"
    );

    let user_version: i32 = connection
        .query_row("PRAGMA user_version;", [], |row| row.get(0))
        .expect("user_version should be readable");
    assert_eq!(
        user_version, CURRENT_USER_VERSION,
        "v33 迁移完成后 user_version 必须是 CURRENT_USER_VERSION={CURRENT_USER_VERSION}，实际 {user_version}"
    );
}

#[test]
fn v34_migration_creates_pending_inflow_runtime_account_table() {
    let db_path = database_path("v34-pending-inflow-runtime-account");
    fs::create_dir_all(db_path.parent().expect("db path parent"))
        .expect("temp db dir should create");
    let mut connection = Connection::open(&db_path).expect("db should open");
    connection
        .execute_batch("PRAGMA user_version = 32;")
        .expect("pre-v33 fixture should set user_version");

    apply_migrations(&mut connection).expect("v34 migration should succeed");

    let mut statement = connection
        .prepare("PRAGMA table_info(qi_runtime_accounts)")
        .expect("qi_runtime_accounts table_info should prepare");
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .expect("qi_runtime_accounts table_info should query")
        .collect::<Result<Vec<_>, _>>()
        .expect("qi_runtime_accounts columns should collect");
    assert_eq!(
        columns,
        vec![
            "account_id",
            "balance",
            "schema_version",
            "last_updated_wall",
        ],
        "expected v34 runtime account table to preserve the pending pool, actual {columns:?}"
    );
    let migrated_rows: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM qi_runtime_accounts WHERE account_id = ?1",
            params![PENDING_INFLOW_ACCOUNT_ID],
            |row| row.get(0),
        )
        .expect("migration row count should query");
    assert_eq!(
        migrated_rows, 0,
        "expected a pre-v34 database to retain an explicit unknown balance rather than inventing zero"
    );
    connection
        .execute(
            "
            INSERT INTO qi_runtime_accounts
                (account_id, balance, schema_version, last_updated_wall)
            VALUES (?1, 12.5, 1, 0)
            ",
            params![PENDING_INFLOW_ACCOUNT_ID],
        )
        .expect("valid pending inflow balance should persist");
    let negative = connection.execute(
        "
        INSERT INTO qi_runtime_accounts
            (account_id, balance, schema_version, last_updated_wall)
        VALUES ('invalid-negative', -0.1, 1, 0)
        ",
        [],
    );
    assert!(
        negative.is_err(),
        "expected v34 balance CHECK to reject negative pending qi, actual {negative:?}"
    );
    let user_version: i32 = connection
        .query_row("PRAGMA user_version;", [], |row| row.get(0))
        .expect("user_version should be readable");
    assert_eq!(
        user_version, CURRENT_USER_VERSION,
        "expected v34 migration to advance to {CURRENT_USER_VERSION}, actual {user_version}"
    );
}

#[test]
fn fresh_database_initializes_known_zero_pending_inflow() {
    let (settings, root) = persistence_settings("fresh-known-zero-pending");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("fresh sqlite should bootstrap");

    assert_eq!(
        load_pending_inflow_balance(&settings).expect("fresh pending row should be known"),
        0.0,
        "expected only a provably fresh database to initialize pending inflow to zero"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn v38_and_v39_migrations_initialize_new_stable_overflow_accounts() {
    let db_path = database_path("v38-dying-elder-overflow-accounts");
    let root = db_path
        .parent()
        .expect("db path should have parent")
        .to_path_buf();
    bootstrap_sqlite(&db_path, "v38-fixture").expect("fresh fixture should bootstrap");
    let mut connection = Connection::open(&db_path).expect("db should open");
    connection
        .execute_batch(
            "
            DELETE FROM qi_runtime_accounts;
            PRAGMA user_version = 37;
            ",
        )
        .expect("fixture should emulate v37 with unknown pending inflow");

    apply_migrations(&mut connection).expect("v37 to v39 migration should succeed");

    for account_id in [
        DYING_ELDER_DAN_EXCESS_ACCOUNT_ID,
        DYING_ELDER_RELEASE_OVERFLOW_ACCOUNT_ID,
        QI_FLOW_OVERFLOW_ACCOUNT_ID,
    ] {
        let balance: f64 = connection
            .query_row(
                "SELECT balance FROM qi_runtime_accounts WHERE account_id = ?1",
                params![account_id],
                |row| row.get(0),
            )
            .unwrap_or_else(|error| panic!("migration should add {account_id}: {error}"));
        assert_eq!(
            balance, 0.0,
            "new stable account {account_id} must start at known zero"
        );
    }
    let pending_rows: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM qi_runtime_accounts WHERE account_id = ?1",
            params![PENDING_INFLOW_ACCOUNT_ID],
            |row| row.get(0),
        )
        .expect("pending row count should query");
    assert_eq!(
        pending_rows, 0,
        "v39 must not invent zero for a missing pre-v34 pending inflow balance"
    );
    let user_version: i32 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("user_version should query");
    assert_eq!(user_version, CURRENT_USER_VERSION);
    drop(connection);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn v40_migration_initializes_qi_flow_overflow_without_mutating_existing_runtime_balances() {
    let db_path = database_path("v40-qi-flow-overflow-account");
    let root = db_path
        .parent()
        .expect("db path should have parent")
        .to_path_buf();
    bootstrap_sqlite(&db_path, "v40-fixture").expect("fresh fixture should bootstrap");
    let mut connection = Connection::open(&db_path).expect("db should open");
    connection
        .execute_batch(
            "
            DELETE FROM qi_runtime_accounts WHERE account_id = 'qi_flow_overflow';
            UPDATE qi_runtime_accounts
            SET balance = 12.5
            WHERE account_id = 'pending_inflow';
            PRAGMA user_version = 39;
            ",
        )
        .expect("fixture should emulate a v39 database with a real pending balance");

    apply_migrations(&mut connection).expect("v40 migration should succeed");

    let qi_flow_balance: f64 = connection
        .query_row(
            "SELECT balance FROM qi_runtime_accounts WHERE account_id = ?1",
            params![QI_FLOW_OVERFLOW_ACCOUNT_ID],
            |row| row.get(0),
        )
        .expect("v40 must initialize the new qi_flow_overflow row");
    assert_eq!(
        qi_flow_balance, 0.0,
        "a pre-v40 database has no recoverable R5 overflow history, so the new account starts at known zero"
    );
    let pending_balance: f64 = connection
        .query_row(
            "SELECT balance FROM qi_runtime_accounts WHERE account_id = ?1",
            params![PENDING_INFLOW_ACCOUNT_ID],
            |row| row.get(0),
        )
        .expect("existing pending balance should remain readable");
    assert_eq!(
        pending_balance, 12.5,
        "v40 must not overwrite existing stable runtime balances while adding qi_flow_overflow"
    );
    let user_version: i32 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("user_version should query");
    assert_eq!(user_version, CURRENT_USER_VERSION);

    drop(connection);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn v41_migration_initializes_absent_rift_drain_at_known_zero() {
    let db_path = database_path("v41-rift-drain-account-absent");
    let root = db_path
        .parent()
        .expect("db path should have parent")
        .to_path_buf();
    bootstrap_sqlite(&db_path, "v41-absent-fixture").expect("fresh fixture should bootstrap");
    let mut connection = Connection::open(&db_path).expect("db should open");
    connection
        .execute_batch(
            "
            DELETE FROM qi_runtime_accounts WHERE account_id = 'rift_drain';
            UPDATE qi_runtime_accounts
            SET balance = 12.5
            WHERE account_id = 'pending_inflow';
            DROP TABLE dormant_terminal_commits;
            PRAGMA user_version = 40;
            ",
        )
        .expect("fixture should emulate a v40 database without the v41-only row");

    let absent_rows: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM qi_runtime_accounts WHERE account_id = ?1",
            params![RIFT_DRAIN_ACCOUNT_ID],
            |row| row.get(0),
        )
        .expect("pre-v41 rift row count should query");
    assert_eq!(
        absent_rows, 0,
        "the pre-v41 fixture must genuinely omit rift_drain before migration"
    );

    apply_migrations(&mut connection)
        .expect("v41 rift migration and current v43 migration should succeed");

    let rift_balance: f64 = connection
        .query_row(
            "SELECT balance FROM qi_runtime_accounts WHERE account_id = ?1",
            params![RIFT_DRAIN_ACCOUNT_ID],
            |row| row.get(0),
        )
        .expect("v41 must create the missing rift_drain row");
    assert_eq!(
        rift_balance, 0.0,
        "a v40 database without rift history must initialize the new account at known zero"
    );
    let pending_balance: f64 = connection
        .query_row(
            "SELECT balance FROM qi_runtime_accounts WHERE account_id = ?1",
            params![PENDING_INFLOW_ACCOUNT_ID],
            |row| row.get(0),
        )
        .expect("existing pending balance should remain readable");
    assert_eq!(
        pending_balance, 12.5,
        "v41 must preserve existing stable runtime balances while adding rift_drain"
    );
    let user_version: i32 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("user_version should query");
    assert_eq!(
        user_version, CURRENT_USER_VERSION,
        "the v41 fixture must continue through the current v43 schema"
    );
    let terminal_table_rows: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'dormant_terminal_commits'",
            [],
            |row| row.get(0),
        )
        .expect("current terminal table existence should query");
    assert_eq!(
        terminal_table_rows, 1,
        "the migration chain must retain the v42 dormant terminal schema"
    );

    drop(connection);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn v41_migration_initializes_rift_drain_without_mutating_existing_runtime_balances() {
    let db_path = database_path("v41-rift-drain-account");
    let root = db_path
        .parent()
        .expect("db path should have parent")
        .to_path_buf();
    bootstrap_sqlite(&db_path, "v41-fixture").expect("fresh fixture should bootstrap");
    let mut connection = Connection::open(&db_path).expect("db should open");
    connection
        .execute_batch(
            "
            UPDATE qi_runtime_accounts
            SET balance = 55.5
            WHERE account_id = 'rift_drain';
            UPDATE qi_runtime_accounts
            SET balance = 12.5
            WHERE account_id = 'pending_inflow';
            DROP TABLE dormant_terminal_commits;
            PRAGMA user_version = 40;
            ",
        )
        .expect("fixture should emulate a v40 database with existing runtime balances");

    apply_migrations(&mut connection)
        .expect("v41 rift migration and current v43 migration should succeed");

    let rift_balance: f64 = connection
        .query_row(
            "SELECT balance FROM qi_runtime_accounts WHERE account_id = ?1",
            params![RIFT_DRAIN_ACCOUNT_ID],
            |row| row.get(0),
        )
        .expect("v41 must preserve the existing rift_drain row");
    assert_eq!(
        rift_balance, 55.5,
        "v41 must preserve an existing rift-drain balance instead of resetting it"
    );
    let pending_balance: f64 = connection
        .query_row(
            "SELECT balance FROM qi_runtime_accounts WHERE account_id = ?1",
            params![PENDING_INFLOW_ACCOUNT_ID],
            |row| row.get(0),
        )
        .expect("existing pending balance should remain readable");
    assert_eq!(
        pending_balance, 12.5,
        "v41 must not overwrite existing stable runtime balances while adding rift_drain"
    );
    let user_version: i32 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("user_version should query");
    assert_eq!(
        user_version, CURRENT_USER_VERSION,
        "the v41 fixture must continue through the current v43 schema"
    );
    let terminal_table_rows: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'dormant_terminal_commits'",
            [],
            |row| row.get(0),
        )
        .expect("current terminal table existence should query");
    assert_eq!(
        terminal_table_rows, 1,
        "the migration chain must retain the v42 dormant terminal schema"
    );

    drop(connection);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn v42_migration_creates_guarded_dormant_terminal_commits_schema() {
    let db_path = database_path("v42-dormant-terminal-schema");
    let root = db_path
        .parent()
        .expect("db path should have parent")
        .to_path_buf();
    bootstrap_sqlite(&db_path, "v42-fixture").expect("fresh fixture should bootstrap");
    let mut connection = Connection::open(&db_path).expect("db should open");
    connection
        .execute_batch(
            "
            DROP TABLE dormant_terminal_commits;
            PRAGMA user_version = 41;
            ",
        )
        .expect("fixture should emulate a v41 database");

    apply_migrations(&mut connection).expect("v42 migration should create the terminal table");
    let columns = table_columns(
        &connection.transaction().unwrap(),
        "dormant_terminal_commits",
    )
    .expect("terminal table columns should query");
    for required in [
        "char_id",
        "cause",
        "at_tick",
        "zone",
        "winner",
        "winner_group",
        "loser_group",
        "zone_accepted",
        "cleanup_revision",
        "created_wall",
        "schema_version",
    ] {
        assert!(
            columns.iter().any(|column| column == required),
            "v42 terminal table must contain `{required}`"
        );
    }
    let user_version: i32 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("user_version should query");
    assert_eq!(user_version, CURRENT_USER_VERSION);
    drop(connection);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn v43_migration_removes_retired_deceased_public_path_without_losing_snapshot() {
    let db_path = database_path("v43-deceased-public-path");
    let root = db_path
        .parent()
        .expect("db path should have parent")
        .to_path_buf();
    bootstrap_sqlite(&db_path, "v43-fixture").expect("fresh fixture should bootstrap");
    let mut connection = Connection::open(&db_path).expect("db should open");
    connection
        .execute_batch(
            "
            ALTER TABLE deceased_snapshots ADD COLUMN public_path TEXT;
            INSERT INTO deceased_snapshots (
                char_id, snapshot_json, public_path, died_at_tick, schema_version, last_updated_wall
            ) VALUES ('offline:Legacy', '{}', 'deceased/offline_Legacy.json', 7, 1, 1);
            PRAGMA user_version = 42;
            ",
        )
        .expect("fixture should emulate a v42 database with the retired column");

    apply_migrations(&mut connection).expect("v43 migration should remove retired column");

    let columns = table_columns(&connection.transaction().unwrap(), "deceased_snapshots")
        .expect("deceased snapshot columns should query");
    assert!(!columns.iter().any(|column| column == "public_path"));
    let snapshot_json: String = connection
        .query_row(
            "SELECT snapshot_json FROM deceased_snapshots WHERE char_id = ?1",
            params!["offline:Legacy"],
            |row| row.get(0),
        )
        .expect("legacy deceased snapshot should survive migration");
    assert_eq!(snapshot_json, "{}");
    let user_version: i32 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("user_version should query");
    assert_eq!(user_version, CURRENT_USER_VERSION);

    drop(connection);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn v44_migration_removes_retired_legacy_letterbox_schema_and_is_idempotent() {
    let db_path = database_path("v44-legacy-letterbox-cleanup");
    let root = db_path
        .parent()
        .expect("db path should have parent")
        .to_path_buf();
    bootstrap_sqlite(&db_path, "v44-fixture").expect("fresh fixture should bootstrap");
    let mut connection = Connection::open(&db_path).expect("db should open");
    connection
        .execute_batch(
            "
            CREATE TABLE legacy_letterbox (
                owner_id TEXT PRIMARY KEY,
                inheritor_id TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                assigned_at_tick INTEGER NOT NULL,
                reject_until_tick INTEGER NOT NULL,
                status TEXT NOT NULL,
                schema_version INTEGER NOT NULL,
                last_updated_wall INTEGER NOT NULL
            );
            CREATE INDEX idx_legacy_letterbox_inheritor
            ON legacy_letterbox (inheritor_id, status);
            INSERT INTO legacy_letterbox (
                owner_id, inheritor_id, payload_json, assigned_at_tick,
                reject_until_tick, status, schema_version, last_updated_wall
            ) VALUES ('offline:retired', 'offline:recipient', '{}', 1, 2, 'pending', 1, 3);
            PRAGMA user_version = 43;
            ",
        )
        .expect("fixture should emulate a v43 database with the retired table");

    apply_migrations(&mut connection).expect("v44 migration should remove retired schema");

    let retired_table: Option<String> = connection
        .query_row(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'legacy_letterbox'",
            [],
            |row| row.get(0),
        )
        .optional()
        .expect("retired table query should succeed");
    assert_eq!(
        retired_table, None,
        "v44 must delete the retired table and its data"
    );
    let retired_index: Option<String> = connection
        .query_row(
            "SELECT name FROM sqlite_master WHERE type = 'index' AND name = 'idx_legacy_letterbox_inheritor'",
            [],
            |row| row.get(0),
        )
        .optional()
        .expect("retired index query should succeed");
    assert_eq!(
        retired_index, None,
        "v44 must delete the retired inheritor index"
    );
    let user_version: i32 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("user_version should query");
    assert_eq!(user_version, CURRENT_USER_VERSION);

    apply_migrations(&mut connection)
        .expect("reapplying v44 migration must remain idempotent after cleanup");
    let user_version_after_retry: i32 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("user_version should remain queryable after retry");
    assert_eq!(user_version_after_retry, CURRENT_USER_VERSION);

    drop(connection);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn v43_migration_tolerates_partial_fixture_without_deceased_snapshots_table() {
    let db_path = database_path("v43-partial-fixture-without-deceased-table");
    let root = db_path
        .parent()
        .expect("db path should have parent")
        .to_path_buf();
    bootstrap_sqlite(&db_path, "v43-partial-fixture").expect("fresh fixture should bootstrap");
    let mut connection = Connection::open(&db_path).expect("db should open");
    connection
        .execute_batch(
            "
            DROP TABLE deceased_snapshots;
            PRAGMA user_version = 42;
            ",
        )
        .expect("fixture should emulate a focused migration test without the unrelated table");

    apply_migrations(&mut connection)
        .expect("v43 should not break focused migration fixtures that omit the table");
    assert!(
        !table_exists(&connection.transaction().unwrap(), "deceased_snapshots")
            .expect("table existence should query")
    );
    let user_version: i32 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("user_version should query");
    assert_eq!(user_version, CURRENT_USER_VERSION);

    drop(connection);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn v43_migration_rejects_malformed_deceased_schema_without_advancing_version() {
    let db_path = database_path("v43-deceased-bad-schema");
    let root = db_path
        .parent()
        .expect("db path should have parent")
        .to_path_buf();
    bootstrap_sqlite(&db_path, "v43-bad-schema-fixture").expect("fresh fixture should bootstrap");
    let mut connection = Connection::open(&db_path).expect("db should open");
    connection
        .execute_batch(
            "
            DROP TABLE deceased_snapshots;
            CREATE TABLE deceased_snapshots (
                char_id TEXT PRIMARY KEY,
                public_path TEXT
            );
            PRAGMA user_version = 42;
            ",
        )
        .expect("fixture should install a malformed v42 deceased table");

    let error = apply_migrations(&mut connection)
        .expect_err("v43 must reject a malformed preexisting deceased table");
    assert!(
        error
            .to_string()
            .contains("deceased_snapshots column snapshot_json missing"),
        "schema guard should identify the missing deceased column: {error}"
    );
    let user_version: i32 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("user_version should remain readable");
    assert_eq!(
        user_version, 42,
        "failed v43 schema validation must not advance user_version"
    );
    let columns = table_columns(&connection.transaction().unwrap(), "deceased_snapshots")
        .expect("rolled-back deceased columns should query");
    assert!(
        columns.iter().any(|column| column == "public_path"),
        "the failed migration must roll back its DROP COLUMN"
    );

    drop(connection);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn current_v43_database_rejects_retired_deceased_public_path() {
    let db_path = database_path("v43-current-deceased-public-path");
    let root = db_path
        .parent()
        .expect("db path should have parent")
        .to_path_buf();
    bootstrap_sqlite(&db_path, "v43-current-public-path-fixture")
        .expect("fresh fixture should bootstrap");
    let mut connection = Connection::open(&db_path).expect("db should open");
    connection
        .execute_batch("ALTER TABLE deceased_snapshots ADD COLUMN public_path TEXT;")
        .expect("fixture should add the retired column without changing user_version");

    let error = apply_migrations(&mut connection)
        .expect_err("current v43 schema must reject the retired public projection");
    assert!(
        error
            .to_string()
            .contains("retired deceased_snapshots.public_path remains"),
        "schema guard should identify the retired column: {error}"
    );
    let user_version: i32 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("user_version should remain readable");
    assert_eq!(user_version, CURRENT_USER_VERSION);

    drop(connection);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn v42_migration_rejects_preexisting_terminal_table_with_wrong_schema() {
    let db_path = database_path("v42-dormant-terminal-bad-schema");
    let root = db_path
        .parent()
        .expect("db path should have parent")
        .to_path_buf();
    bootstrap_sqlite(&db_path, "v42-bad-fixture").expect("fresh fixture should bootstrap");
    let mut connection = Connection::open(&db_path).expect("db should open");
    connection
        .execute_batch(
            "
            DROP TABLE dormant_terminal_commits;
            CREATE TABLE dormant_terminal_commits (
                char_id TEXT NOT NULL,
                cause TEXT PRIMARY KEY
            );
            PRAGMA user_version = 41;
            ",
        )
        .expect("fixture should install a malformed preexisting table");

    let error = apply_migrations(&mut connection)
        .expect_err("v42 migration must reject a malformed preexisting terminal table");
    assert!(
        error
            .to_string()
            .contains("dormant_terminal_commits column"),
        "schema guard should identify the missing terminal column: {error}"
    );
    let user_version: i32 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("user_version should remain readable");
    assert_eq!(
        user_version, 41,
        "failed v42 schema validation must not advance user_version"
    );
    drop(connection);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn current_v42_database_rejects_malformed_terminal_schema() {
    let db_path = database_path("v42-current-dormant-terminal-bad-schema");
    let root = db_path
        .parent()
        .expect("db path should have parent")
        .to_path_buf();
    bootstrap_sqlite(&db_path, "v42-current-bad-fixture").expect("fresh fixture should bootstrap");
    let mut connection = Connection::open(&db_path).expect("db should open");
    connection
        .execute_batch(
            "
            DROP TABLE dormant_terminal_commits;
            CREATE TABLE dormant_terminal_commits (
                char_id TEXT NOT NULL,
                cause TEXT PRIMARY KEY
            );
            PRAGMA user_version = 42;
            ",
        )
        .expect("fixture should install a malformed current-version table");

    let error = apply_migrations(&mut connection)
        .expect_err("current v42 schema must be validated on every bootstrap");
    assert!(
        error
            .to_string()
            .contains("dormant_terminal_commits column"),
        "current-version schema guard should identify the missing terminal column: {error}"
    );
    let user_version: i32 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("user_version should remain readable");
    assert_eq!(
        user_version, 42,
        "schema validation must fail without mutating current user_version"
    );
    drop(connection);
    let _ = fs::remove_dir_all(root);
}

fn terminal_commit_record(char_id: &str, zone_accepted: f64) -> DormantTerminalCommitRecord {
    DormantTerminalCommitRecord {
        char_id: char_id.to_string(),
        cause: "combat".to_string(),
        at_tick: 77,
        zone: DEFAULT_SPAWN_ZONE_NAME.to_string(),
        winner: Some("npc:winner".to_string()),
        winner_group: Some(11),
        loser_group: Some(22),
        zone_accepted,
        cleanup_revision: None,
    }
}

fn terminal_relic_event(char_id: &str) -> crate::npc::dormant::PendingDormantRelicCreated {
    crate::npc::dormant::PendingDormantRelicCreated {
        char_id: char_id.to_string(),
        zone: DEFAULT_SPAWN_ZONE_NAME.to_string(),
        position: [12.5, 64.0, -8.25],
        archetype: crate::npc::lifecycle::NpcArchetype::Disciple,
        loot_seed: 0xA55A,
        created_tick: 77,
    }
}

#[test]
fn dormant_terminal_commit_is_atomic_and_duplicate_does_not_rewrite_sink() {
    let (settings, root) = persistence_settings("dormant-terminal-atomic");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("fixture sqlite should bootstrap");
    let char_id = "npc:terminal:atomic";
    let record = terminal_commit_record(char_id, 2.5);
    let relic = terminal_relic_event(char_id);
    let mut zones = crate::world::zone::ZoneRegistry::fallback();
    zones.zones[0].spirit_qi = 0.42;
    zones.zones[0].danger_level = 3;
    let mut ledger = WorldQiAccount::default();
    ledger
        .set_balance(crate::qi_physics::ledger::rift_drain_account(), 7.25)
        .expect("fixture runtime balance should be valid");

    assert_eq!(
        persist_dormant_terminal_commit(&settings, &record, &zones, &ledger, Some(&relic))
            .expect("first terminal transaction should commit"),
        PersistDormantTerminalOutcome::Committed
    );
    assert_eq!(
        load_dormant_terminal_commits(&settings).unwrap(),
        vec![record.clone()]
    );
    assert_eq!(
        load_pending_dormant_relics_for_zone(&settings, DEFAULT_SPAWN_ZONE_NAME)
            .expect("terminal relic should load")
            .len(),
        1
    );

    let mut replacement_zones = zones.clone();
    replacement_zones.zones[0].spirit_qi = 0.11;
    let mut replacement_ledger = WorldQiAccount::default();
    replacement_ledger
        .set_balance(crate::qi_physics::ledger::rift_drain_account(), 99.0)
        .expect("replacement runtime balance should be valid");
    let mut duplicate = record.clone();
    duplicate.cause = "natural_aging".to_string();
    duplicate.zone_accepted = 9.0;
    assert_eq!(
        persist_dormant_terminal_commit(
            &settings,
            &duplicate,
            &replacement_zones,
            &replacement_ledger,
            None,
        )
        .expect("duplicate terminal transaction should be recognized"),
        PersistDormantTerminalOutcome::AlreadyCommitted
    );

    let connection = open_persistence_connection(&settings).expect("db should reopen");
    let persisted_zone: f64 = connection
        .query_row(
            "SELECT spirit_qi FROM zones_runtime WHERE zone_id = ?1",
            params![DEFAULT_SPAWN_ZONE_NAME],
            |row| row.get(0),
        )
        .expect("terminal zone sink should exist");
    let persisted_rift: f64 = connection
        .query_row(
            "SELECT balance FROM qi_runtime_accounts WHERE account_id = ?1",
            params![RIFT_DRAIN_ACCOUNT_ID],
            |row| row.get(0),
        )
        .expect("terminal runtime sink should exist");
    assert_eq!(
        persisted_zone, 0.42,
        "duplicate must not rewrite the zone sink"
    );
    assert_eq!(
        persisted_rift, 7.25,
        "duplicate must not rewrite runtime qi"
    );
    assert_eq!(
        load_dormant_terminal_commits(&settings).unwrap(),
        vec![record]
    );
    drop(connection);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn dormant_terminal_commit_rolls_back_all_rows_when_tombstone_insert_fails() {
    let (settings, root) = persistence_settings("dormant-terminal-rollback");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("fixture sqlite should bootstrap");
    let mut zones = crate::world::zone::ZoneRegistry::fallback();
    zones.zones[0].spirit_qi = 0.37;
    let mut ledger = WorldQiAccount::default();
    ledger
        .set_balance(crate::qi_physics::ledger::rift_drain_account(), 8.5)
        .expect("fixture runtime balance should be valid");
    let char_id = "npc:terminal:rollback";
    let invalid = terminal_commit_record(char_id, -1.0);
    let relic = terminal_relic_event(char_id);

    persist_dormant_terminal_commit(&settings, &invalid, &zones, &ledger, Some(&relic))
        .expect_err("negative zone_accepted must fail at the final tombstone insert");

    assert!(load_dormant_terminal_commits(&settings).unwrap().is_empty());
    assert!(
        load_pending_dormant_relics_for_zone(&settings, DEFAULT_SPAWN_ZONE_NAME)
            .unwrap()
            .is_empty(),
        "relic written before the failing tombstone insert must roll back"
    );
    let connection = open_persistence_connection(&settings).expect("db should reopen");
    let zone_rows: i64 = connection
        .query_row("SELECT COUNT(*) FROM zones_runtime", [], |row| row.get(0))
        .expect("zone row count should query");
    let rift_balance: f64 = connection
        .query_row(
            "SELECT balance FROM qi_runtime_accounts WHERE account_id = ?1",
            params![RIFT_DRAIN_ACCOUNT_ID],
            |row| row.get(0),
        )
        .expect("bootstrap runtime row should exist");
    assert_eq!(zone_rows, 0, "staged zone sink must roll back");
    assert_eq!(rift_balance, 0.0, "staged runtime qi must roll back");
    drop(connection);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn dormant_terminal_cleanup_rearms_on_restart_and_clears_only_confirmed_revision() {
    let (settings, root) = persistence_settings("dormant-terminal-rearm");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("fixture sqlite should bootstrap");
    let record = terminal_commit_record("npc:terminal:rearm", 0.0);
    persist_dormant_terminal_commit(
        &settings,
        &record,
        &crate::world::zone::ZoneRegistry::fallback(),
        &WorldQiAccount::default(),
        None,
    )
    .expect("terminal fixture should commit");

    bind_dormant_terminal_cleanup_revision(&settings, std::slice::from_ref(&record.char_id), 7)
        .expect("first cleanup revision should bind");
    assert_eq!(
        load_dormant_terminal_commits(&settings).unwrap()[0].cleanup_revision,
        Some(7)
    );
    let rearmed = rearm_dormant_terminal_commits(&settings)
        .expect("restart must reset uncertain pre-crash bindings");
    assert_eq!(rearmed[0].cleanup_revision, None);

    bind_dormant_terminal_cleanup_revision(&settings, std::slice::from_ref(&record.char_id), 9)
        .expect("post-restart deletion revision should bind");
    assert_eq!(
        clear_dormant_terminal_commits_through_revision(&settings, 8).unwrap(),
        0
    );
    assert_eq!(load_dormant_terminal_commits(&settings).unwrap().len(), 1);
    assert_eq!(
        clear_dormant_terminal_commits_through_revision(&settings, 9).unwrap(),
        1
    );
    assert!(load_dormant_terminal_commits(&settings).unwrap().is_empty());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn runtime_qi_accounts_persist_and_fresh_ledger_hydrate_roundtrip() {
    let (settings, root) = persistence_settings("runtime-qi-five-account-roundtrip");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("fixture sqlite should bootstrap");

    let expected = [
        (pending_inflow_account(), 11.25),
        (crate::qi_physics::ledger::qi_flow_overflow_account(), 17.0),
        (
            crate::qi_physics::ledger::dying_elder_dan_excess_account(),
            22.5,
        ),
        (
            crate::qi_physics::ledger::dying_elder_release_overflow_account(),
            33.75,
        ),
        (crate::qi_physics::ledger::rift_drain_account(), 55.5),
    ];
    let expected_total = expected.iter().map(|(_, balance)| balance).sum::<f64>();
    let mut source = WorldQiAccount::default();
    for (account, balance) in &expected {
        source
            .set_balance(account.clone(), *balance)
            .expect("fixture runtime balance should be valid");
    }
    // 白名单之外的 ledger 账户仍由自己的物理权威恢复，不能被本表顺手持久化。
    source
        .set_balance(QiAccountId::zone("spawn"), 99.0)
        .expect("unrelated fixture balance should be valid");

    persist_zone_runtime_snapshot_with_heartbeat(
        &settings,
        &crate::world::zone::ZoneRegistry::fallback(),
        None,
        &source,
    )
    .expect("production snapshot path should persist five runtime balances");

    let mut hydrated = WorldQiAccount::default();
    assert_eq!(
        hydrate_runtime_qi_accounts(&settings, &mut hydrated)
            .expect("fresh ledger should hydrate all stable runtime accounts"),
        5
    );
    for (account, balance) in expected {
        assert_eq!(hydrated.balance(&account), balance, "account={account}");
    }
    assert_eq!(hydrated.balance(&QiAccountId::zone("spawn")), 0.0);
    let before = WorldQiSnapshot {
        player_qi: 0.0,
        zone_qi: 0.0,
        container_qi: 0.0,
        ledger_qi: expected_total,
        era_decay_accum: 0.0,
        budget_initial_total: 0.0,
        budget_current_total: 0.0,
    };
    let after = WorldQiSnapshot {
        ledger_qi: hydrated.total(),
        ..before
    };
    assert_conservation(&before, &after, 0.0)
        .expect("runtime account restart must preserve the stable-pool qi total");
    assert!(
        hydrated.has_account(&qi_flow_overflow_account()),
        "qi_flow_overflow must hydrate as a real stable ledger owner"
    );
    assert!(
        !hydrated.has_account(&QiAccountId::zone("spawn")),
        "non-whitelist ledger accounts must not hydrate from runtime pools"
    );
    assert!(
        hydrated.transfers().is_empty(),
        "restart must not restore audit history"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn revival_qi_transaction_rolls_back_every_durable_owner_on_late_quota_failure() {
    use crate::qi_physics::ledger::{
        dying_elder_dan_excess_account, dying_elder_release_overflow_account,
        qi_flow_overflow_account,
    };
    use crate::world::zone::ZoneRegistry;

    let (settings, root) = persistence_settings("revival-qi-late-quota-rollback");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("fixture sqlite should bootstrap");
    let char_id = "offline:RevivalRollback";
    let username = "RevivalRollback";
    let baseline_life = LifeRecord {
        character_id: char_id.to_string(),
        created_at: 1,
        biography: vec![BiographyEntry::NearDeath {
            cause: "fixture".to_string(),
            tick: 40,
        }],
        ..LifeRecord::default()
    };
    let staged_life = LifeRecord {
        biography: vec![
            BiographyEntry::NearDeath {
                cause: "fixture".to_string(),
                tick: 40,
            },
            BiographyEntry::Rebirth {
                prior_realm: Realm::Void,
                new_realm: Realm::Spirit,
                tick: 41,
            },
        ],
        ..baseline_life.clone()
    };
    let baseline_cultivation = Cultivation {
        realm: Realm::Void,
        qi_current: 7.0,
        qi_max: 12.0,
        ..Cultivation::default()
    };
    let staged_cultivation = Cultivation {
        realm: Realm::Spirit,
        qi_current: 3.0,
        qi_max: 9.0,
        ..Cultivation::default()
    };
    let baseline_meridians = MeridianSystem::default();
    let mut staged_meridians = MeridianSystem::default();
    staged_meridians.regular[0].opened = true;
    staged_meridians.regular[0].opened_at = 41;
    let baseline_contamination = Contamination::default();
    let staged_contamination = Contamination {
        entries: vec![ContamSource {
            amount: 1.25,
            color: ColorKind::Sharp,
            meridian_id: None,
            attacker_id: Some("fixture-attacker".to_string()),
            introduced_at: 41,
        }],
    };
    let baseline_qi_color = QiColor {
        secondary: Some(ColorKind::Heavy),
        ..QiColor::default()
    };
    let baseline_karma = Karma { weight: 42.5 };
    let baseline_bundle = serde_json::json!({
        "v": crate::cultivation::legacy_meridian_bundle::CURRENT_BUNDLE_VERSION,
        "cultivation": crate::cultivation::components::encode_persisted_cultivation(&baseline_cultivation),
        "meridians": baseline_meridians,
        "qi_color": baseline_qi_color,
        "karma": baseline_karma,
        "qi_accumulator": { "pending": 7.25, "ticks": 40 },
        "future_sibling": { "must": ["survive", 1] },
        "contamination": baseline_contamination,
        "life_record": baseline_life,
    });
    let baseline_bundle_json = serde_json::to_string(&baseline_bundle)
        .expect("baseline player cultivation bundle should serialize");
    let baseline_life =
        serde_json::from_value::<LifeRecord>(baseline_bundle["life_record"].clone())
            .expect("baseline bundle life record should decode");
    let baseline_zones = ZoneRegistry::fallback();
    let mut staged_zones = baseline_zones.clone();
    staged_zones.zones[0].spirit_qi = -0.35;
    staged_zones.zones[0].danger_level = 6;
    let stale_dynamic_zone_id = "pseudo_vein_heartbeat_42";

    let old_values = [
        (pending_inflow_account(), 11.0),
        (qi_flow_overflow_account(), 17.0),
        (dying_elder_dan_excess_account(), 22.0),
        (dying_elder_release_overflow_account(), 33.0),
    ];
    let new_values = [
        (pending_inflow_account(), 111.0),
        (qi_flow_overflow_account(), 117.0),
        (dying_elder_dan_excess_account(), 222.0),
        (dying_elder_release_overflow_account(), 333.0),
    ];
    let mut baseline_ledger = WorldQiAccount::default();
    let mut staged_ledger = WorldQiAccount::default();
    for (account, balance) in old_values.iter().cloned() {
        baseline_ledger
            .set_balance(account, balance)
            .expect("baseline runtime qi balance should be valid");
    }
    for (account, balance) in new_values.iter().cloned() {
        staged_ledger
            .set_balance(account, balance)
            .expect("staged runtime qi balance should be valid");
    }

    let mut connection = open_persistence_connection(&settings).expect("db should open");
    {
        let transaction = connection
            .transaction()
            .expect("baseline transaction should start");
        transaction
            .execute(
                "
                INSERT INTO player_cultivation (
                    username, cultivation_json, schema_version, last_updated_wall
                ) VALUES (?1, ?2, ?3, ?4)
                ",
                params![
                    username,
                    baseline_bundle_json,
                    CURRENT_SCHEMA_VERSION,
                    100_i64
                ],
            )
            .expect("baseline player cultivation bundle should persist");
        upsert_life_record(&transaction, &baseline_life, 100)
            .expect("baseline life record should persist");
        persist_zone_runtime_records(&transaction, &baseline_zones, 100)
            .expect("baseline Zone owner should persist");
        upsert_zone_runtime(
            &transaction,
            &ZoneRuntimeRecord {
                zone_id: stale_dynamic_zone_id.to_string(),
                spirit_qi: 0.45,
                danger_level: 2,
            },
            100,
        )
        .expect("stale dynamic Zone row should persist");
        upsert_runtime_qi_account_balances(&transaction, &baseline_ledger, 100)
            .expect("baseline stable qi owners should persist");
        upsert_ascension_quota(
            &transaction,
            &AscensionQuotaRecord { occupied_slots: 1 },
            100,
        )
        .expect("baseline quota should persist");
        transaction.commit().expect("baseline rows should commit");
    }
    let baseline_bundle_json: String = connection
        .query_row(
            "SELECT cultivation_json FROM player_cultivation WHERE username = ?1",
            params![username],
            |row| row.get(0),
        )
        .expect("baseline player cultivation bundle should query");
    let baseline_life_json: String = connection
        .query_row(
            "SELECT life_record_json FROM life_records WHERE char_id = ?1",
            params![char_id],
            |row| row.get(0),
        )
        .expect("baseline life record should query");
    let baseline_event_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM life_events WHERE char_id = ?1",
            params![char_id],
            |row| row.get(0),
        )
        .expect("baseline life event count should query");
    connection
        .execute_batch(
            "
            CREATE TRIGGER reject_revival_quota_update
            BEFORE UPDATE ON ascension_quota
            WHEN NEW.row_id = 1
            BEGIN
                SELECT RAISE(ABORT, 'fixture rejects late revival quota update');
            END;
            ",
        )
        .expect("late-failure trigger should install");
    drop(connection);

    let error = persist_revival_qi_transaction(
        &settings,
        username,
        &staged_cultivation,
        &staged_meridians,
        &staged_contamination,
        &staged_life,
        Some(&staged_zones),
        &staged_ledger,
        true,
    )
    .expect_err("late quota failure must abort the entire revival transaction");
    assert!(
        error
            .to_string()
            .contains("fixture rejects late revival quota update"),
        "error should expose the forced final-write failure, actual={error}"
    );

    let connection = open_persistence_connection(&settings).expect("db should reopen");
    let actual_bundle_json: String = connection
        .query_row(
            "SELECT cultivation_json FROM player_cultivation WHERE username = ?1",
            params![username],
            |row| row.get(0),
        )
        .expect("rolled-back player cultivation bundle should query");
    assert_eq!(
        actual_bundle_json, baseline_bundle_json,
        "late quota failure must roll back the revival player bundle before any restart can observe staged actor qi"
    );
    let actual_life_json: String = connection
        .query_row(
            "SELECT life_record_json FROM life_records WHERE char_id = ?1",
            params![char_id],
            |row| row.get(0),
        )
        .expect("rolled-back life record should query");
    assert_eq!(actual_life_json, baseline_life_json);
    let actual_event_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM life_events WHERE char_id = ?1",
            params![char_id],
            |row| row.get(0),
        )
        .expect("rolled-back life event count should query");
    assert_eq!(actual_event_count, baseline_event_count);
    let rebirth_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM life_events WHERE char_id = ?1 AND event_type = 'rebirth'",
            params![char_id],
            |row| row.get(0),
        )
        .expect("rebirth event count should query");
    assert_eq!(rebirth_count, 0);

    let persisted_zones = load_zone_runtime_snapshot_from_connection(&connection)
        .expect("rolled-back Zone rows should load");
    let spawn = persisted_zones
        .iter()
        .find(|record| record.zone_id == baseline_zones.zones[0].name)
        .expect("baseline spawn Zone row must remain");
    assert_eq!(spawn.spirit_qi, baseline_zones.zones[0].spirit_qi);
    assert_eq!(spawn.danger_level, baseline_zones.zones[0].danger_level);
    let stale_dynamic = persisted_zones
        .iter()
        .find(|record| record.zone_id == stale_dynamic_zone_id)
        .expect("rolled-back prefix deletion must restore the stale dynamic Zone row");
    assert_eq!(stale_dynamic.spirit_qi, 0.45);
    assert_eq!(stale_dynamic.danger_level, 2);

    for (account, expected_balance) in old_values {
        let actual_balance: f64 = connection
            .query_row(
                "SELECT balance FROM qi_runtime_accounts WHERE account_id = ?1",
                params![account.id],
                |row| row.get(0),
            )
            .unwrap_or_else(|error| {
                panic!(
                    "rolled-back qi balance should query {}: {error}",
                    account.id
                )
            });
        assert_eq!(actual_balance, expected_balance, "account={account}");
    }
    assert_eq!(
        load_ascension_quota_from_connection(&connection)
            .expect("rolled-back quota should load")
            .occupied_slots,
        1
    );

    drop(connection);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn revival_qi_transaction_replaces_owner_slices_and_preserves_bundle_siblings_for_restart() {
    let (settings, root) = persistence_settings("revival-qi-bundle-restart-roundtrip");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("fixture sqlite should bootstrap");

    let username = "RevivalRoundtrip";
    let baseline_life = LifeRecord {
        character_id: "offline:RevivalRoundtrip".to_string(),
        created_at: 7,
        biography: vec![BiographyEntry::NearDeath {
            cause: "fixture".to_string(),
            tick: 70,
        }],
        ..LifeRecord::default()
    };
    let staged_life = LifeRecord {
        biography: vec![
            BiographyEntry::NearDeath {
                cause: "fixture".to_string(),
                tick: 70,
            },
            BiographyEntry::Rebirth {
                prior_realm: Realm::Void,
                new_realm: Realm::Spirit,
                tick: 71,
            },
        ],
        ..baseline_life.clone()
    };
    let baseline_cultivation = Cultivation {
        realm: Realm::Void,
        qi_current: 8.0,
        qi_max: 13.0,
        ..Cultivation::default()
    };
    let staged_cultivation = Cultivation {
        realm: Realm::Spirit,
        qi_current: 2.0,
        qi_max: 8.0,
        ..Cultivation::default()
    };
    let baseline_meridians = MeridianSystem::default();
    let mut staged_meridians = MeridianSystem::default();
    staged_meridians.regular[0].opened = true;
    staged_meridians.regular[0].opened_at = 71;
    let baseline_contamination = Contamination::default();
    let staged_contamination = Contamination {
        entries: vec![ContamSource {
            amount: 2.5,
            color: ColorKind::Sharp,
            meridian_id: None,
            attacker_id: Some("roundtrip-attacker".to_string()),
            introduced_at: 71,
        }],
    };
    let qi_color = QiColor {
        secondary: Some(ColorKind::Heavy),
        ..QiColor::default()
    };
    let karma = Karma { weight: 19.0 };
    let qi_accumulator = serde_json::json!({ "pending": 3.5, "ticks": 70 });
    let future_sibling = serde_json::json!({ "schema": "future", "values": [1, 2] });
    let baseline_bundle = serde_json::json!({
        "v": crate::cultivation::legacy_meridian_bundle::CURRENT_BUNDLE_VERSION,
        "cultivation": crate::cultivation::components::encode_persisted_cultivation(&baseline_cultivation),
        "meridians": baseline_meridians,
        "qi_color": qi_color,
        "karma": karma,
        "qi_accumulator": qi_accumulator,
        "future_sibling": future_sibling,
        "contamination": baseline_contamination,
        "life_record": baseline_life,
    });
    let baseline_bundle_json = serde_json::to_string(&baseline_bundle)
        .expect("baseline player cultivation bundle should serialize");
    let connection = open_persistence_connection(&settings).expect("fixture db should open");
    connection
        .execute(
            "
            INSERT INTO player_cultivation (
                username, cultivation_json, schema_version, last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4)
            ",
            params![
                username,
                baseline_bundle_json,
                CURRENT_SCHEMA_VERSION,
                70_i64
            ],
        )
        .expect("baseline player cultivation bundle should persist");
    drop(connection);

    let staged_ledger = WorldQiAccount::default();
    let quota_release = persist_revival_qi_transaction(
        &settings,
        username,
        &staged_cultivation,
        &staged_meridians,
        &staged_contamination,
        &staged_life,
        None,
        &staged_ledger,
        false,
    )
    .expect("revival transaction should persist every staged owner slice");
    assert!(
        quota_release.is_none(),
        "no quota release was requested for this roundtrip"
    );

    let bundle = load_player_cultivation_bundle(&settings, username)
        .expect("restart loader should read the persisted bundle")
        .expect("revival must retain the existing player bundle row");
    let bundle_version = bundle["v"]
        .as_i64()
        .expect("revival bundle must retain an integer wire version");
    let restored_cultivation =
        crate::cultivation::components::decode_persisted_cultivation(bundle["cultivation"].clone())
            .expect("restart cultivation decoder should accept the staged owner slice");
    assert_eq!(
        restored_cultivation, staged_cultivation,
        "restart must observe the staged cultivation rather than pre-revival qi"
    );
    let restored_meridians = crate::cultivation::legacy_meridian_bundle::decode_meridian_system(
        bundle["meridians"].clone(),
        bundle_version,
    )
    .expect("restart meridian decoder should accept the staged owner slice");
    assert_eq!(
        restored_meridians, staged_meridians,
        "restart must observe staged meridian state"
    );
    assert_eq!(
        bundle["contamination"],
        serde_json::to_value(&staged_contamination).expect("staged contamination should serialize"),
        "restart bundle must contain staged contamination"
    );
    assert_eq!(
        bundle["life_record"],
        serde_json::to_value(&staged_life).expect("staged life record should serialize"),
        "restart bundle must contain the rebirth life record"
    );
    assert_eq!(
        bundle["qi_color"], baseline_bundle["qi_color"],
        "revival must preserve the qi_color sibling"
    );
    assert_eq!(
        bundle["karma"], baseline_bundle["karma"],
        "revival must preserve the karma sibling"
    );
    assert_eq!(
        bundle["qi_accumulator"], baseline_bundle["qi_accumulator"],
        "revival must preserve the qi_accumulator sibling"
    );
    assert_eq!(
        bundle["future_sibling"], baseline_bundle["future_sibling"],
        "revival must preserve unknown future sibling slices"
    );

    let connection = open_persistence_connection(&settings).expect("db should reopen");
    let persisted_life_json: String = connection
        .query_row(
            "SELECT life_record_json FROM life_records WHERE char_id = ?1",
            params![staged_life.character_id],
            |row| row.get(0),
        )
        .expect("life record should commit with the bundle");
    assert_eq!(
        serde_json::from_str::<Value>(&persisted_life_json)
            .expect("persisted life record should be valid JSON"),
        serde_json::to_value(&staged_life).expect("staged life record should serialize"),
        "life_records row must commit the same rebirth state as the player bundle"
    );
    let rebirth_events: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM life_events WHERE char_id = ?1 AND event_type = 'rebirth'",
            params![staged_life.character_id],
            |row| row.get(0),
        )
        .expect("rebirth event count should query");
    assert_eq!(
        rebirth_events, 1,
        "successful revival must append one rebirth event"
    );

    drop(connection);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn revival_qi_transaction_rejects_missing_or_corrupt_bundle_without_durable_prefix() {
    use crate::qi_physics::ledger::{
        dying_elder_dan_excess_account, dying_elder_release_overflow_account,
        qi_flow_overflow_account,
    };
    use crate::world::zone::ZoneRegistry;

    for (case, existing_bundle, expected_kind) in [
        ("missing", None, std::io::ErrorKind::NotFound),
        (
            "malformed-json",
            Some("{not-json"),
            std::io::ErrorKind::InvalidData,
        ),
        ("non-object", Some("[]"), std::io::ErrorKind::InvalidData),
    ] {
        let (settings, root) = persistence_settings(&format!("revival-qi-invalid-bundle-{case}"));
        bootstrap_sqlite(settings.db_path(), settings.server_run_id())
            .expect("fixture sqlite should bootstrap");
        let username = format!("RevivalInvalid{case}");
        let char_id = format!("offline:RevivalInvalid{case}");
        let baseline_life = LifeRecord {
            character_id: char_id.clone(),
            created_at: 9,
            biography: vec![BiographyEntry::NearDeath {
                cause: "fixture".to_string(),
                tick: 90,
            }],
            ..LifeRecord::default()
        };
        let staged_life = LifeRecord {
            biography: vec![
                BiographyEntry::NearDeath {
                    cause: "fixture".to_string(),
                    tick: 90,
                },
                BiographyEntry::Rebirth {
                    prior_realm: Realm::Void,
                    new_realm: Realm::Spirit,
                    tick: 91,
                },
            ],
            ..baseline_life.clone()
        };
        let staged_cultivation = Cultivation {
            realm: Realm::Spirit,
            qi_current: 2.0,
            qi_max: 8.0,
            ..Cultivation::default()
        };
        let staged_meridians = MeridianSystem::default();
        let staged_contamination = Contamination::default();
        let baseline_zones = ZoneRegistry::fallback();
        let mut staged_zones = baseline_zones.clone();
        staged_zones.zones[0].spirit_qi = -0.7;
        staged_zones.zones[0].danger_level = 7;
        let old_values = [
            (pending_inflow_account(), 11.0),
            (qi_flow_overflow_account(), 17.0),
            (dying_elder_dan_excess_account(), 22.0),
            (dying_elder_release_overflow_account(), 33.0),
        ];
        let mut baseline_ledger = WorldQiAccount::default();
        let mut staged_ledger = WorldQiAccount::default();
        for (account, balance) in old_values.iter().cloned() {
            baseline_ledger
                .set_balance(account.clone(), balance)
                .expect("baseline runtime qi balance should be valid");
            staged_ledger
                .set_balance(account, balance + 100.0)
                .expect("staged runtime qi balance should be valid");
        }

        let mut connection =
            open_persistence_connection(&settings).expect("fixture db should open");
        {
            let transaction = connection
                .transaction()
                .expect("baseline transaction should start");
            if let Some(existing_bundle) = existing_bundle {
                transaction
                    .execute(
                        "
                        INSERT INTO player_cultivation (
                            username, cultivation_json, schema_version, last_updated_wall
                        ) VALUES (?1, ?2, ?3, ?4)
                        ",
                        params![username, existing_bundle, CURRENT_SCHEMA_VERSION, 90_i64],
                    )
                    .expect("corrupt fixture bundle should persist as raw text");
            }
            upsert_life_record(&transaction, &baseline_life, 90)
                .expect("baseline life record should persist");
            persist_zone_runtime_records(&transaction, &baseline_zones, 90)
                .expect("baseline zones should persist");
            upsert_runtime_qi_account_balances(&transaction, &baseline_ledger, 90)
                .expect("baseline stable accounts should persist");
            upsert_ascension_quota(
                &transaction,
                &AscensionQuotaRecord { occupied_slots: 1 },
                90,
            )
            .expect("baseline quota should persist");
            transaction.commit().expect("baseline rows should commit");
        }
        let baseline_bundle: Option<String> = connection
            .query_row(
                "SELECT cultivation_json FROM player_cultivation WHERE username = ?1",
                params![username],
                |row| row.get(0),
            )
            .optional()
            .expect("baseline bundle row should query");
        let baseline_life_json: String = connection
            .query_row(
                "SELECT life_record_json FROM life_records WHERE char_id = ?1",
                params![char_id],
                |row| row.get(0),
            )
            .expect("baseline life record should query");
        let baseline_event_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM life_events WHERE char_id = ?1",
                params![char_id],
                |row| row.get(0),
            )
            .expect("baseline life event count should query");
        drop(connection);

        let error = persist_revival_qi_transaction(
            &settings,
            username.as_str(),
            &staged_cultivation,
            &staged_meridians,
            &staged_contamination,
            &staged_life,
            Some(&staged_zones),
            &staged_ledger,
            true,
        )
        .expect_err("missing or corrupt player bundle must fail closed");
        assert_eq!(
            error.kind(),
            expected_kind,
            "case={case} must reject before any durable owner write, error={error}"
        );

        let connection = open_persistence_connection(&settings).expect("db should reopen");
        let actual_bundle: Option<String> = connection
            .query_row(
                "SELECT cultivation_json FROM player_cultivation WHERE username = ?1",
                params![username],
                |row| row.get(0),
            )
            .optional()
            .expect("rolled-back bundle row should query");
        assert_eq!(
            actual_bundle, baseline_bundle,
            "case={case} must not manufacture or overwrite a player bundle"
        );
        let actual_life_json: String = connection
            .query_row(
                "SELECT life_record_json FROM life_records WHERE char_id = ?1",
                params![char_id],
                |row| row.get(0),
            )
            .expect("rolled-back life record should query");
        assert_eq!(
            actual_life_json, baseline_life_json,
            "case={case} must not update life_records before bundle validation"
        );
        let actual_event_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM life_events WHERE char_id = ?1",
                params![char_id],
                |row| row.get(0),
            )
            .expect("rolled-back life event count should query");
        assert_eq!(
            actual_event_count, baseline_event_count,
            "case={case} must not append a rebirth event before bundle validation"
        );
        let persisted_zones = load_zone_runtime_snapshot_from_connection(&connection)
            .expect("rolled-back zone rows should load");
        let spawn = persisted_zones
            .iter()
            .find(|record| record.zone_id == baseline_zones.zones[0].name)
            .expect("baseline spawn zone must remain");
        assert_eq!(
            spawn.spirit_qi, baseline_zones.zones[0].spirit_qi,
            "case={case} must not change signed zone qi"
        );
        assert_eq!(
            spawn.danger_level, baseline_zones.zones[0].danger_level,
            "case={case} must not change zone danger"
        );
        for (account, expected_balance) in old_values {
            let actual_balance: f64 = connection
                .query_row(
                    "SELECT balance FROM qi_runtime_accounts WHERE account_id = ?1",
                    params![account.id],
                    |row| row.get(0),
                )
                .unwrap_or_else(|error| {
                    panic!(
                        "case={case} should retain stable account {}: {error}",
                        account.id
                    )
                });
            assert_eq!(
                actual_balance, expected_balance,
                "case={case} must not partially update stable account={account}"
            );
        }
        assert_eq!(
            load_ascension_quota_from_connection(&connection)
                .expect("rolled-back quota should load")
                .occupied_slots,
            1,
            "case={case} must not release the quota"
        );

        drop(connection);
        let _ = fs::remove_dir_all(root);
    }
}

#[test]
fn runtime_qi_account_persist_failure_rolls_back_staged_prefix() {
    use crate::qi_physics::ledger::{
        dying_elder_dan_excess_account, dying_elder_release_overflow_account,
        qi_flow_overflow_account,
    };

    let (settings, root) = persistence_settings("runtime-qi-persist-atomic-rollback");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("fixture sqlite should bootstrap");

    let old_values = [
        (PENDING_INFLOW_ACCOUNT_ID, 11.0),
        (DYING_ELDER_DAN_EXCESS_ACCOUNT_ID, 22.0),
        (DYING_ELDER_RELEASE_OVERFLOW_ACCOUNT_ID, 33.0),
        // 唯一的旧值：seed 循环后 SQLite 中 qi_flow_overflow 以最后一次写入为准（44.0），
        // 若这里重复列出 17.0 会让回滚断言拿旧账对拍新账而误红。
        (QI_FLOW_OVERFLOW_ACCOUNT_ID, 44.0),
    ];
    let mut connection = open_persistence_connection(&settings).expect("db should open");
    {
        let transaction = connection
            .transaction()
            .expect("fixture transaction should start");
        for (account_id, balance) in old_values {
            transaction
                .execute(
                    "UPDATE qi_runtime_accounts SET balance = ?2 WHERE account_id = ?1",
                    params![account_id, balance],
                )
                .unwrap_or_else(|error| panic!("fixture should seed {account_id}: {error}"));
        }
        transaction
            .commit()
            .expect("fixture old balances should commit");
    }

    let mut source = WorldQiAccount::default();
    source
        .set_balance(pending_inflow_account(), 111.0)
        .expect("pending staged balance should be valid");
    source
        .set_balance(qi_flow_overflow_account(), 117.0)
        .expect("qi flow overflow staged balance should be valid");
    source
        .set_balance(dying_elder_dan_excess_account(), 222.0)
        .expect("dan excess staged balance should be valid");
    let release_account = dying_elder_release_overflow_account();
    source
        .set_balance(release_account.clone(), 333.0)
        .expect("release overflow staged balance should be valid");
    source
        .set_balance(qi_flow_overflow_account(), 444.0)
        .expect("qi flow overflow staged balance should be valid");

    // 账本现在 fail-closed 拒绝不可表示的余额；用 SQLite 的测试专用触发器在第四个
    // 白名单账户写入时注入持久化失败，继续覆盖前缀写入必须随事务整体回滚的契约。
    connection
        .execute_batch(
            "
            CREATE TRIGGER runtime_qi_atomic_rollback_fail_fourth
            BEFORE UPDATE OF balance ON qi_runtime_accounts
            WHEN OLD.account_id = 'qi_flow_overflow'
            BEGIN
                SELECT RAISE(ABORT, 'fixture rejects qi_flow_overflow');
            END;
            ",
        )
        .expect("fixture persistence failure trigger should create");

    {
        let transaction = connection
            .transaction()
            .expect("failing persist transaction should start");
        let error = upsert_runtime_qi_account_balances(&transaction, &source, 456)
            .expect_err("the fourth whitelist update must fail inside the transaction");
        assert!(
            error.to_string().contains(QI_FLOW_OVERFLOW_ACCOUNT_ID),
            "error should identify the rejected fourth account, actual={error}"
        );
        drop(transaction);
    }

    for (account_id, expected_balance) in old_values {
        let actual_balance: f64 = connection
            .query_row(
                "SELECT balance FROM qi_runtime_accounts WHERE account_id = ?1",
                params![account_id],
                |row| row.get(0),
            )
            .unwrap_or_else(|error| panic!("persisted balance should query {account_id}: {error}"));
        assert_eq!(
            actual_balance, expected_balance,
            "failed transaction must roll back staged prefix account={account_id}"
        );
    }

    drop(connection);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn runtime_qi_accounts_missing_or_invalid_row_fail_closed_without_partial_hydrate() {
    for (case, account_id) in [
        ("pending", PENDING_INFLOW_ACCOUNT_ID),
        ("qi-flow-overflow", QI_FLOW_OVERFLOW_ACCOUNT_ID),
        ("dan-excess", DYING_ELDER_DAN_EXCESS_ACCOUNT_ID),
        ("death-overflow", DYING_ELDER_RELEASE_OVERFLOW_ACCOUNT_ID),
        ("qi-flow-overflow", QI_FLOW_OVERFLOW_ACCOUNT_ID),
    ] {
        for corruption in ["missing", "negative"] {
            let test_name = format!("runtime-qi-{case}-{corruption}");
            let (settings, root) = persistence_settings(&test_name);
            bootstrap_sqlite(settings.db_path(), settings.server_run_id())
                .expect("fixture sqlite should bootstrap");
            let connection = Connection::open(settings.db_path()).expect("db should open");
            match corruption {
                "missing" => {
                    connection
                        .execute(
                            "DELETE FROM qi_runtime_accounts WHERE account_id = ?1",
                            params![account_id],
                        )
                        .expect("fixture row should delete");
                }
                "negative" => {
                    connection
                        .execute_batch("PRAGMA ignore_check_constraints = ON;")
                        .expect("fixture should allow deliberate corruption");
                    connection
                        .execute(
                            "UPDATE qi_runtime_accounts SET balance = -1.0 WHERE account_id = ?1",
                            params![account_id],
                        )
                        .expect("fixture row should corrupt");
                }
                _ => unreachable!(),
            }
            drop(connection);

            let mut hydrated = WorldQiAccount::default();
            let error = hydrate_runtime_qi_accounts(&settings, &mut hydrated)
                .expect_err("missing/invalid stable pool must fail closed");
            assert!(
                error.to_string().contains(account_id),
                "error should identify {account_id}, actual={error}"
            );
            assert!(
                hydrated.iter_balances().next().is_none(),
                "failed load must not partially hydrate earlier whitelist entries"
            );
            let _ = fs::remove_dir_all(root);
        }
    }
}

#[test]
fn runtime_qi_accounts_unknown_row_fails_closed_instead_of_being_ignored() {
    let (settings, root) = persistence_settings("runtime-qi-unknown-account");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("fixture sqlite should bootstrap");
    let connection = Connection::open(settings.db_path()).expect("sqlite should open");
    connection
        .execute(
            "INSERT INTO qi_runtime_accounts (account_id, balance, schema_version, last_updated_wall) VALUES (?1, ?2, ?3, ?4)",
            params!["unexpected_runtime_owner", 17.0_f64, CURRENT_SCHEMA_VERSION, 0_i64],
        )
        .expect("fixture should be able to add an unknown durable owner");
    drop(connection);

    let mut hydrated = WorldQiAccount::default();
    let error = hydrate_runtime_qi_accounts(&settings, &mut hydrated)
        .expect_err("unknown durable qi owner must fail closed rather than disappear");
    assert!(
        error.to_string().contains("unexpected_runtime_owner"),
        "unknown-owner error should identify the ignored value, actual={error}"
    );
    assert!(
        hydrated.iter_balances().next().is_none(),
        "unknown-owner rejection must not partially hydrate the fixed ledger owners"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn v35_migration_backfills_conservative_age_and_eval_phase() {
    let (settings, root) = persistence_settings("v35-heartbeat-timing-backfill");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("fixture sqlite should bootstrap");
    let mut connection = open_persistence_connection(&settings).expect("db should open");
    let transaction = connection.transaction().expect("transaction should start");
    let record = heartbeat_pseudo_vein_record("pseudo_vein_heartbeat_7");
    upsert_heartbeat_pseudo_vein(&transaction, &record, 100)
        .expect("fixture heartbeat row should persist");
    transaction
        .commit()
        .expect("fixture transaction should commit");
    connection
        .execute_batch(
            "
            UPDATE heartbeat_pseudo_veins
            SET observed_age_ticks = 0,
                pending_runtime_ticks = 0,
                pending_offline_ticks = 0,
                occupant_count = 0,
                eval_elapsed_ticks = 0;
            PRAGMA user_version = 34;
            ",
        )
        .expect("fixture should emulate a pre-v35 row");

    apply_migrations(&mut connection).expect("v35 timing migration should succeed");

    let timing: (i64, i64, i64) = connection
        .query_row(
            "
            SELECT observed_age_ticks, pending_runtime_ticks, eval_elapsed_ticks
            FROM heartbeat_pseudo_veins
            WHERE zone_id = ?1
            ",
            params![record.zone_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("backfilled timing should query");
    let conservative = i64::try_from(HEARTBEAT_EVAL_INTERVAL_TICKS - 1).unwrap();
    assert_eq!(
        timing,
        (800 + conservative, conservative, conservative),
        "expected migration to preserve known age plus a conservative full phase, actual {timing:?}"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn pre_v34_unknown_pending_balance_fails_closed_without_writing_zero() {
    let (settings, root) = persistence_settings("unknown-pending-fail-closed");
    fs::create_dir_all(settings.db_path().parent().expect("db parent"))
        .expect("db parent should create");
    let connection = Connection::open(settings.db_path()).expect("fixture db should open");
    connection
        .execute_batch("PRAGMA user_version = 32;")
        .expect("pre-v34 fixture should set version");
    drop(connection);

    let mut app = App::new();
    app.insert_resource(settings.clone());
    app.insert_resource(DailyBackupState::default());
    app.insert_resource(crate::world::zone::ZoneRegistry::fallback());
    app.insert_resource(WorldHeartbeat::default());
    app.insert_resource(CultivationClock::default());
    app.insert_resource(WorldQiAccount::default());
    app.add_systems(Startup, bootstrap_persistence_system);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| app.update()));
    assert!(
        result.is_err(),
        "expected startup to fail closed because a pre-v34 pending balance is unknowable"
    );

    let connection = Connection::open(settings.db_path()).expect("fixture db should reopen");
    let rows: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM qi_runtime_accounts WHERE account_id = ?1",
            params![PENDING_INFLOW_ACCOUNT_ID],
            |row| row.get(0),
        )
        .expect("pending row count should query");
    assert_eq!(
        rows, 0,
        "expected failed startup not to overwrite the unknown balance with zero"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn heartbeat_lifecycle_without_zone_runtime_fails_closed_without_deletion() {
    let (settings, root) = persistence_settings("heartbeat-missing-runtime-fail-closed");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("fixture sqlite should bootstrap");
    let record = heartbeat_pseudo_vein_record("pseudo_vein_heartbeat_7");
    let mut connection = open_persistence_connection(&settings).expect("db should open");
    let transaction = connection.transaction().expect("transaction should start");
    upsert_heartbeat_pseudo_vein(&transaction, &record, 100)
        .expect("valid lifecycle row should persist");
    transaction
        .commit()
        .expect("fixture transaction should commit");

    assert_pseudo_vein_startup_fails_closed(&settings, record.zone_id.as_str(), 1, 0);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn zone_runtime_without_heartbeat_lifecycle_fails_closed_without_deletion() {
    let (settings, root) = persistence_settings("heartbeat-missing-lifecycle-fail-closed");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("fixture sqlite should bootstrap");
    let record = heartbeat_pseudo_vein_record("pseudo_vein_heartbeat_7");
    let mut connection = open_persistence_connection(&settings).expect("db should open");
    let transaction = connection.transaction().expect("transaction should start");
    upsert_zone_runtime(
        &transaction,
        &heartbeat_zone_runtime_record(&record, record.qi_current),
        100,
    )
    .expect("valid runtime row should persist");
    transaction
        .commit()
        .expect("fixture transaction should commit");

    assert_pseudo_vein_startup_fails_closed(&settings, record.zone_id.as_str(), 0, 1);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn invalid_heartbeat_bounds_fails_closed_without_partial_restore() {
    let (settings, root) = persistence_settings("heartbeat-invalid-bounds-fail-closed");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("fixture sqlite should bootstrap");
    let record = heartbeat_pseudo_vein_record("pseudo_vein_heartbeat_7");
    let mut connection = open_persistence_connection(&settings).expect("db should open");
    let transaction = connection.transaction().expect("transaction should start");
    upsert_heartbeat_pseudo_vein(&transaction, &record, 100)
        .expect("valid lifecycle row should persist before corruption");
    upsert_zone_runtime(
        &transaction,
        &heartbeat_zone_runtime_record(&record, record.qi_current),
        100,
    )
    .expect("matching runtime row should persist");
    transaction
        .execute(
            "UPDATE heartbeat_pseudo_veins SET min_x = 200.0, max_x = 100.0 WHERE zone_id = ?1",
            params![record.zone_id.as_str()],
        )
        .expect("fixture should corrupt persisted bounds");
    transaction
        .commit()
        .expect("fixture transaction should commit");

    assert_pseudo_vein_startup_fails_closed(&settings, record.zone_id.as_str(), 1, 1);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn out_of_range_pseudo_vein_qi_fails_closed_without_ledger_mint() {
    let (settings, root) = persistence_settings("heartbeat-invalid-zone-qi-fail-closed");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("fixture sqlite should bootstrap");
    let record = heartbeat_pseudo_vein_record("pseudo_vein_heartbeat_7");
    let mut connection = open_persistence_connection(&settings).expect("db should open");
    let transaction = connection.transaction().expect("transaction should start");
    upsert_heartbeat_pseudo_vein(&transaction, &record, 100)
        .expect("valid lifecycle row should persist");
    upsert_zone_runtime(
        &transaction,
        &heartbeat_zone_runtime_record(&record, record.qi_current),
        100,
    )
    .expect("valid runtime row should persist before corruption");
    transaction
        .execute(
            "UPDATE zones_runtime SET spirit_qi = 9.0 WHERE zone_id = ?1",
            params![record.zone_id.as_str()],
        )
        .expect("fixture should corrupt persisted zone qi");
    transaction
        .commit()
        .expect("fixture transaction should commit");

    assert_pseudo_vein_startup_fails_closed(&settings, record.zone_id.as_str(), 1, 1);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn corrupt_heartbeat_hydration_panics_without_deleting_valid_rows() {
    let (settings, root) = persistence_settings("heartbeat-corrupt-fail-closed");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("fixture sqlite should bootstrap");
    let mut connection = open_persistence_connection(&settings).expect("db should open");
    let transaction = connection.transaction().expect("transaction should start");
    let first = heartbeat_pseudo_vein_record("pseudo_vein_heartbeat_7");
    let mut second = heartbeat_pseudo_vein_record("pseudo_vein_heartbeat_8");
    second.center_xz = [20.0, -80.0];
    upsert_heartbeat_pseudo_vein(&transaction, &first, 100).expect("valid row should persist");
    upsert_heartbeat_pseudo_vein(&transaction, &second, 100)
        .expect("second valid row should persist");
    upsert_zone_runtime(
        &transaction,
        &heartbeat_zone_runtime_record(&first, first.qi_current),
        100,
    )
    .expect("first matching runtime row should persist");
    upsert_zone_runtime(
        &transaction,
        &heartbeat_zone_runtime_record(&second, second.qi_current),
        100,
    )
    .expect("second matching runtime row should persist");
    transaction
        .execute(
            "UPDATE heartbeat_pseudo_veins SET active_events_json = '{' WHERE zone_id = ?1",
            params![second.zone_id],
        )
        .expect("fixture should corrupt one JSON row");
    transaction
        .commit()
        .expect("fixture transaction should commit");

    let mut app = App::new();
    app.insert_resource(settings.clone());
    app.insert_resource(DailyBackupState::default());
    app.insert_resource(crate::world::zone::ZoneRegistry::fallback());
    app.insert_resource(WorldHeartbeat::default());
    app.insert_resource(CultivationClock::default());
    app.insert_resource(WorldQiAccount::default());
    app.add_systems(Startup, bootstrap_persistence_system);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| app.update()));
    assert!(
        result.is_err(),
        "expected malformed authoritative heartbeat data to stop startup"
    );

    let connection = Connection::open(settings.db_path()).expect("fixture db should reopen");
    let rows: i64 = connection
        .query_row("SELECT COUNT(*) FROM heartbeat_pseudo_veins", [], |row| {
            row.get(0)
        })
        .expect("heartbeat row count should query");
    assert_eq!(
        rows, 2,
        "expected failed hydration not to run destructive empty-state replacement"
    );
    let runtime_rows: i64 = connection
        .query_row("SELECT COUNT(*) FROM zones_runtime", [], |row| row.get(0))
        .expect("zone runtime row count should query");
    assert_eq!(
        runtime_rows, 2,
        "expected failed hydration not to delete matching zone runtime rows"
    );
    let _ = fs::remove_dir_all(root);
}
