use crate::combat::{attach_combat_bundle_to_joined_clients, components::Lifecycle, is_damageable};
use crate::persistence::bootstrap_sqlite;
use crate::player::state::{
    player_character_id, save_player_shrine_anchor_slice, save_player_state, PlayerStatePersistence,
};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use valence::prelude::{App, Entity, GameMode, Query, Res, Update, Username};
use valence::testing::create_mock_client;

fn unique_temp_dir(test_name: &str) -> PathBuf {
    let unique_suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();

    std::env::temp_dir().join(format!(
        "bong-combat-mod-{test_name}-{}-{unique_suffix}",
        std::process::id()
    ))
}

#[derive(Clone, Copy)]
struct DamageabilityFixtures {
    no_mode: Entity,
    survival: Entity,
    creative: Entity,
    adventure: Entity,
    spectator: Entity,
}

impl valence::prelude::Resource for DamageabilityFixtures {}

fn assert_damageability(fixtures: Res<DamageabilityFixtures>, modes: Query<&GameMode>) {
    assert!(is_damageable(fixtures.no_mode, &modes));
    assert!(is_damageable(fixtures.survival, &modes));
    assert!(!is_damageable(fixtures.creative, &modes));
    assert!(!is_damageable(fixtures.adventure, &modes));
    assert!(!is_damageable(fixtures.spectator, &modes));
}

#[test]
fn damageable_gate_only_allows_survival_or_non_player_entities() {
    let mut app = App::new();
    let no_mode = app.world_mut().spawn_empty().id();
    let survival = app.world_mut().spawn(GameMode::Survival).id();
    let creative = app.world_mut().spawn(GameMode::Creative).id();
    let adventure = app.world_mut().spawn(GameMode::Adventure).id();
    let spectator = app.world_mut().spawn(GameMode::Spectator).id();
    app.insert_resource(DamageabilityFixtures {
        no_mode,
        survival,
        creative,
        adventure,
        spectator,
    });
    app.add_systems(Update, assert_damageability);

    app.update();
}

#[test]
fn joined_client_hydrates_shrine_anchor_from_sqlite_when_present() {
    let root = unique_temp_dir("hydrates-shrine-anchor");
    let data_dir = root.join("data");
    std::fs::create_dir_all(&data_dir).expect("data dir should create");
    let db_path = data_dir.join("bong.db");

    bootstrap_sqlite(&db_path, "combat-mod-hydrates").expect("sqlite bootstrap should succeed");
    let persistence = PlayerStatePersistence::with_db_path(&data_dir, &db_path);

    save_player_shrine_anchor_slice(&persistence, "Alice", Some([11.0, 22.0, 33.0]))
        .expect("save shrine anchor should succeed");

    let mut app = App::new();
    app.insert_resource(persistence);
    app.add_systems(Update, attach_combat_bundle_to_joined_clients);

    let (client_bundle, _helper) = create_mock_client("Alice");
    let entity = app.world_mut().spawn(client_bundle).id();
    app.update();

    let lifecycle = app
        .world()
        .get::<Lifecycle>(entity)
        .expect("joined client should receive Lifecycle");
    assert_eq!(lifecycle.spawn_anchor, Some([11.0, 22.0, 33.0]));

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn joined_client_hydrates_rotated_character_id_from_sqlite_when_present() {
    let root = unique_temp_dir("hydrates-character-id");
    let data_dir = root.join("data");
    std::fs::create_dir_all(&data_dir).expect("data dir should create");
    let db_path = data_dir.join("bong.db");

    bootstrap_sqlite(&db_path, "combat-mod-character-id").expect("sqlite bootstrap should succeed");
    let persistence = PlayerStatePersistence::with_db_path(&data_dir, &db_path);

    save_player_state(&persistence, "Alice", &Default::default())
        .expect("save player should initialize current_char_id");
    let current_char_id = crate::player::state::rotate_current_character_id(&persistence, "Alice")
        .expect("rotating current_char_id should succeed");

    let mut app = App::new();
    app.insert_resource(persistence);
    app.add_systems(Update, attach_combat_bundle_to_joined_clients);

    let (client_bundle, _helper) = create_mock_client("Alice");
    let entity = app.world_mut().spawn(client_bundle).id();
    app.update();

    let lifecycle = app
        .world()
        .get::<Lifecycle>(entity)
        .expect("joined client should receive Lifecycle");
    assert_eq!(
        lifecycle.character_id,
        player_character_id("Alice", &current_char_id)
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn joined_client_has_no_shrine_anchor_when_missing_in_sqlite() {
    let root = unique_temp_dir("missing-shrine-anchor");
    let data_dir = root.join("data");
    std::fs::create_dir_all(&data_dir).expect("data dir should create");
    let db_path = data_dir.join("bong.db");
    bootstrap_sqlite(&db_path, "combat-mod-missing").expect("sqlite bootstrap should succeed");
    let persistence = PlayerStatePersistence::with_db_path(&data_dir, &db_path);

    let mut app = App::new();
    app.insert_resource(persistence);
    app.add_systems(Update, attach_combat_bundle_to_joined_clients);

    let (client_bundle, _helper) = create_mock_client("Bob");
    let entity = app.world_mut().spawn(client_bundle).id();
    app.update();

    let lifecycle = app
        .world()
        .get::<Lifecycle>(entity)
        .expect("joined client should receive Lifecycle");
    assert_eq!(lifecycle.spawn_anchor, None);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn joined_client_hydrates_shrine_anchor_from_sqlite_with_optional_resource() {
    // Regression: attach_combat_bundle_to_joined_clients takes Option<Res<PlayerStatePersistence>>.
    // Ensure it still attaches the combat bundle even if persistence is missing.
    let mut app = App::new();
    app.add_systems(Update, attach_combat_bundle_to_joined_clients);

    let (client_bundle, _helper) = create_mock_client("NoDb");
    let entity = app.world_mut().spawn(client_bundle).id();
    app.update();

    let username = app
        .world()
        .get::<Username>(entity)
        .expect("mock client should have Username");
    assert_eq!(username.0.as_str(), "NoDb");
    let lifecycle = app
        .world()
        .get::<Lifecycle>(entity)
        .expect("joined client should receive Lifecycle");
    assert_eq!(lifecycle.spawn_anchor, None);
}
