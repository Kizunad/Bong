use crate::combat::{attach_combat_bundle_to_joined_clients, components::Lifecycle};
use crate::persistence::bootstrap_sqlite;
use crate::player::state::{save_player_shrine_anchor_slice, PlayerStatePersistence};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use valence::prelude::{App, Update, Username};
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
