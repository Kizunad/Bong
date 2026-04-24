pub mod gameplay;
mod progression;
pub mod state;

use self::state::{
    load_player_slices, save_player_core_slice, save_player_inventory_slice,
    save_player_progression_slice, save_player_skill_slice, save_player_slices,
    save_player_slow_slice, PlayerState, PlayerStateAutosaveTimer, PlayerStatePersistence,
};
use crate::combat::components::TICKS_PER_SECOND;
use crate::inventory::{attach_inventory_to_joined_clients, PlayerInventory};
use crate::skill::components::SkillSet;
use valence::message::SendMessage;
use valence::prelude::Despawned;
use valence::prelude::{
    Added, App, AppExit, Changed, ChunkLayer, Client, Commands, Entity, EntityLayer, EntityLayerId,
    EventReader, GameMode, IntoSystemConfigs, Last, Position, Query, RemovedComponents, Res,
    ResMut, Update, Username, VisibleChunkLayer, VisibleEntityLayers, With, Without,
};

const SPAWN_POSITION: [f64; 3] = [8.0, 150.0, 8.0];
const WELCOME_MESSAGE: &str =
    "Welcome to Bong! Test commands: !zones, !tpzone <zone>, !top, !gm <c|a|s>, !spawn";
const CORE_SLICE_FLUSH_INTERVAL_TICKS: u64 = 5 * TICKS_PER_SECOND;
const SLOW_UI_SLICE_FLUSH_INTERVAL_TICKS: u64 = 60 * TICKS_PER_SECOND;
const PROGRESSION_SLICE_FLUSH_INTERVAL_TICKS: u64 = 90 * TICKS_PER_SECOND;

type ClientInitQueryItem<'a> = (
    Entity,
    &'a mut Client,
    &'a mut EntityLayerId,
    &'a mut VisibleChunkLayer,
    &'a mut VisibleEntityLayers,
    &'a mut Position,
    &'a mut GameMode,
);

type JoinedClientsWithoutStateQueryItem<'a> = (Entity, &'a Username);
type JoinedClientsWithoutStateQueryFilter = (Added<Client>, Without<PlayerState>);
type ChangedInventoryClientsQueryItem<'a> = (&'a Username, &'a PlayerInventory);
type ChangedInventoryClientsQueryFilter = (With<Client>, Changed<PlayerInventory>);
type ChangedSkillClientsQueryItem<'a> = (&'a Username, &'a SkillSet);
type ChangedSkillClientsQueryFilter = (With<Client>, Changed<SkillSet>);

pub fn register(app: &mut App) {
    tracing::info!("[bong][player] registering player init/cleanup systems");
    app.insert_resource(PlayerStatePersistence::default());
    app.insert_resource(PlayerStateAutosaveTimer::default());
    gameplay::register(app);
    app.add_systems(
        Update,
        (
            init_clients,
            attach_player_state_to_joined_clients.after(init_clients),
            attach_inventory_to_joined_clients.after(attach_player_state_to_joined_clients),
            tick_player_persistence_timer,
            autosave_player_core_slices.after(tick_player_persistence_timer),
            autosave_player_slow_and_ui_slices.after(autosave_player_core_slices),
            autosave_player_progression_slices.after(autosave_player_slow_and_ui_slices),
            flush_changed_player_skills.after(autosave_player_progression_slices),
            flush_changed_player_inventories
                .after(attach_inventory_to_joined_clients)
                .after(flush_changed_player_skills),
            despawn_disconnected_clients.after(flush_changed_player_inventories),
        ),
    );
    app.add_systems(Last, flush_connected_players_on_shutdown);
}

pub fn spawn_position() -> [f64; 3] {
    SPAWN_POSITION
}

pub fn welcome_message() -> &'static str {
    WELCOME_MESSAGE
}

pub fn initial_game_mode() -> GameMode {
    GameMode::Creative
}

fn init_clients(
    mut clients: Query<ClientInitQueryItem<'_>, Added<Client>>,
    layers: Query<Entity, (With<ChunkLayer>, With<EntityLayer>)>,
) {
    let layer = layers.single();

    for (
        entity,
        mut client,
        mut layer_id,
        mut visible_chunk_layer,
        mut visible_entity_layers,
        mut position,
        mut game_mode,
    ) in &mut clients
    {
        apply_spawn_defaults(
            layer,
            &mut layer_id,
            &mut visible_chunk_layer,
            &mut visible_entity_layers,
            &mut position,
            &mut game_mode,
        );

        client.send_chat_message(welcome_message());

        let spawn_position = spawn_position();
        tracing::info!(
            "[bong][player] initialized client entity {entity:?} at [{}, {}, {}] in Adventure",
            spawn_position[0],
            spawn_position[1],
            spawn_position[2]
        );
    }
}

pub(crate) fn attach_player_state_to_joined_clients(
    mut commands: Commands,
    persistence: Res<PlayerStatePersistence>,
    joined_clients: Query<
        JoinedClientsWithoutStateQueryItem<'_>,
        JoinedClientsWithoutStateQueryFilter,
    >,
) {
    for (entity, username) in &joined_clients {
        let persisted = load_player_slices(&persistence, username.0.as_str());
        let realm = persisted.state.realm.clone();
        let composite_power = persisted.state.composite_power();
        let restored_inventory = persisted.inventory.is_some();
        let restored_skill = !persisted.skill_set.skills.is_empty()
            || !persisted.skill_set.consumed_scrolls.is_empty();
        let mut entity_commands = commands.entity(entity);

        entity_commands.insert((persisted.state, Position::new(persisted.position)));
        if let Some(player_inventory) = persisted.inventory {
            entity_commands.insert(player_inventory);
        }
        entity_commands.insert(persisted.skill_set);
        tracing::info!(
            "[bong][player] attached PlayerState to client entity {entity:?} for `{}` (realm={}, composite_power={composite_power:.3}, restored_inventory={restored_inventory}, restored_skill={restored_skill})",
            username.0,
            realm,
        );
    }
}

fn apply_spawn_defaults(
    layer: Entity,
    layer_id: &mut EntityLayerId,
    visible_chunk_layer: &mut VisibleChunkLayer,
    visible_entity_layers: &mut VisibleEntityLayers,
    position: &mut Position,
    game_mode: &mut GameMode,
) {
    layer_id.0 = layer;
    visible_chunk_layer.0 = layer;
    visible_entity_layers.0.insert(layer);
    position.set(spawn_position());
    *game_mode = initial_game_mode();
}

fn position_to_array(position: &Position) -> [f64; 3] {
    let current = position.get();
    [current.x, current.y, current.z]
}

fn tick_player_persistence_timer(mut timer: ResMut<PlayerStateAutosaveTimer>) {
    timer.ticks += 1;
}

#[allow(clippy::type_complexity)]
fn despawn_disconnected_clients(
    mut commands: Commands,
    persistence: Res<PlayerStatePersistence>,
    mut disconnected_clients: RemovedComponents<Client>,
    persisted_players: Query<(
        &Username,
        &PlayerState,
        &Position,
        Option<&PlayerInventory>,
        Option<&SkillSet>,
    )>,
) {
    for entity in disconnected_clients.read() {
        if let Ok((username, player_state, position, player_inventory, skill_set)) =
            persisted_players.get(entity)
        {
            match save_player_slices(
                &persistence,
                username.0.as_str(),
                player_state,
                position_to_array(position),
                player_inventory,
                skill_set.unwrap_or(&SkillSet::default()),
            ) {
                Ok(path) => tracing::info!(
                    "[bong][player] saved player slices for disconnected client `{}` to {} before cleanup",
                    username.0,
                    path.display()
                ),
                Err(error) => tracing::warn!(
                    "[bong][player] failed to save player slices for disconnected client `{}`: {error}",
                    username.0,
                ),
            }
        } else {
            tracing::warn!(
                "[bong][player] disconnected client entity {entity:?} had no username/PlayerState/Position to persist before cleanup"
            );
        }

        tracing::info!("[bong][player] cleaning up disconnected client entity {entity:?}");
        if let Some(mut entity_commands) = commands.get_entity(entity) {
            entity_commands.insert(Despawned);
        }
    }
}

#[allow(clippy::type_complexity)]
fn flush_connected_players_on_shutdown(
    persistence: Res<PlayerStatePersistence>,
    mut app_exit: EventReader<AppExit>,
    players: Query<
        (
            &Username,
            &PlayerState,
            &Position,
            Option<&PlayerInventory>,
            Option<&SkillSet>,
        ),
        With<Client>,
    >,
) {
    if app_exit.read().next().is_none() {
        return;
    }

    for (username, player_state, position, player_inventory, skill_set) in &players {
        match save_player_slices(
            &persistence,
            username.0.as_str(),
            player_state,
            position_to_array(position),
            player_inventory,
            skill_set.unwrap_or(&SkillSet::default()),
        ) {
            Ok(path) => tracing::info!(
                "[bong][player] saved player slices for shutdown flush `{}` to {}",
                username.0,
                path.display()
            ),
            Err(error) => tracing::warn!(
                "[bong][player] failed to save player slices during shutdown flush for `{}`: {error}",
                username.0,
            ),
        }
    }
}

fn autosave_player_core_slices(
    persistence: Res<PlayerStatePersistence>,
    timer: Res<PlayerStateAutosaveTimer>,
    players: Query<(&Username, &PlayerState), With<Client>>,
) {
    if !timer.ticks.is_multiple_of(CORE_SLICE_FLUSH_INTERVAL_TICKS) {
        return;
    }

    let mut saved_count = 0usize;

    for (username, player_state) in &players {
        match save_player_core_slice(&persistence, username.0.as_str(), player_state) {
            Ok(_) => saved_count += 1,
            Err(error) => tracing::warn!(
                "[bong][player] 5s core flush failed for `{}`: {error}",
                username.0,
            ),
        }
    }

    tracing::info!(
        "[bong][player] flushed {saved_count} core player slice(s) after {CORE_SLICE_FLUSH_INTERVAL_TICKS} ticks"
    );
}

fn autosave_player_slow_and_ui_slices(
    persistence: Res<PlayerStatePersistence>,
    timer: Res<PlayerStateAutosaveTimer>,
    players: Query<(&Username, &Position), With<Client>>,
) {
    if !timer
        .ticks
        .is_multiple_of(SLOW_UI_SLICE_FLUSH_INTERVAL_TICKS)
    {
        return;
    }

    let mut saved_count = 0usize;

    for (username, position) in &players {
        match save_player_slow_slice(
            &persistence,
            username.0.as_str(),
            position_to_array(position),
        ) {
            Ok(_) => saved_count += 1,
            Err(error) => tracing::warn!(
                "[bong][player] 60s slow/ui flush failed for `{}`: {error}",
                username.0,
            ),
        }
    }

    tracing::info!(
        "[bong][player] flushed {saved_count} slow/ui player slice(s) after {SLOW_UI_SLICE_FLUSH_INTERVAL_TICKS} ticks"
    );
}

fn autosave_player_progression_slices(
    persistence: Res<PlayerStatePersistence>,
    timer: Res<PlayerStateAutosaveTimer>,
    players: Query<(&Username, &PlayerState), With<Client>>,
) {
    if !timer
        .ticks
        .is_multiple_of(PROGRESSION_SLICE_FLUSH_INTERVAL_TICKS)
    {
        return;
    }

    let mut saved_count = 0usize;

    for (username, player_state) in &players {
        match save_player_progression_slice(&persistence, username.0.as_str(), player_state) {
            Ok(_) => saved_count += 1,
            Err(error) => tracing::warn!(
                "[bong][player] 90s progression flush failed for `{}`: {error}",
                username.0,
            ),
        }
    }

    tracing::info!(
        "[bong][player] flushed {saved_count} progression player slice(s) after {PROGRESSION_SLICE_FLUSH_INTERVAL_TICKS} ticks"
    );
}

fn flush_changed_player_inventories(
    persistence: Res<PlayerStatePersistence>,
    players: Query<ChangedInventoryClientsQueryItem<'_>, ChangedInventoryClientsQueryFilter>,
) {
    for (username, player_inventory) in &players {
        if let Err(error) =
            save_player_inventory_slice(&persistence, username.0.as_str(), Some(player_inventory))
        {
            tracing::warn!(
                "[bong][player] immediate inventory flush failed for `{}`: {error}",
                username.0,
            );
        }
    }
}

fn flush_changed_player_skills(
    persistence: Res<PlayerStatePersistence>,
    players: Query<ChangedSkillClientsQueryItem<'_>, ChangedSkillClientsQueryFilter>,
) {
    for (username, skill_set) in &players {
        if let Err(error) = save_player_skill_slice(&persistence, username.0.as_str(), skill_set) {
            tracing::warn!(
                "[bong][player] immediate skill flush failed for `{}`: {error}",
                username.0,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use rusqlite::{params, Connection};
    use valence::prelude::{App, DVec3, Position, Update};
    use valence::testing::create_mock_client;

    use crate::inventory::{
        ContainerState, InventoryRevision, ItemInstance, ItemRarity, PlacedItemState,
        PlayerInventory,
    };
    use crate::persistence::bootstrap_sqlite;

    #[test]
    fn spawn_defaults_are_preserved() {
        let mut app = App::new();
        let initial_layer = app.world_mut().spawn_empty().id();
        let spawn_layer = app.world_mut().spawn_empty().id();
        let mut layer_id = EntityLayerId(initial_layer);
        let mut visible_chunk_layer = VisibleChunkLayer(initial_layer);
        let mut visible_entity_layers = VisibleEntityLayers::default();
        let mut position = Position::new([0.0, 0.0, 0.0]);
        let mut game_mode = GameMode::Survival;

        visible_entity_layers.0.insert(initial_layer);

        apply_spawn_defaults(
            spawn_layer,
            &mut layer_id,
            &mut visible_chunk_layer,
            &mut visible_entity_layers,
            &mut position,
            &mut game_mode,
        );

        assert_eq!(spawn_position(), [8.0, 150.0, 8.0]);
        assert_eq!(position.get(), DVec3::new(8.0, 150.0, 8.0));
        assert_eq!(initial_game_mode(), GameMode::Creative);
        assert_eq!(game_mode, GameMode::Creative);
        assert_eq!(welcome_message(), WELCOME_MESSAGE);
        assert_eq!(layer_id.0, spawn_layer);
        assert_eq!(visible_chunk_layer.0, spawn_layer);
        assert!(visible_entity_layers.0.contains(&spawn_layer));
    }

    fn unique_temp_dir(test_name: &str) -> PathBuf {
        let unique_suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();

        std::env::temp_dir().join(format!(
            "bong-player-mod-{test_name}-{}-{unique_suffix}",
            std::process::id()
        ))
    }

    fn sqlite_persistence(test_name: &str) -> (PlayerStatePersistence, PathBuf, PathBuf) {
        let data_dir = unique_temp_dir(test_name);
        let db_path = data_dir.join("bong.db");
        bootstrap_sqlite(&db_path, &format!("player-mod-{test_name}"))
            .expect("sqlite bootstrap should succeed");
        (
            PlayerStatePersistence::with_db_path(&data_dir, &db_path),
            data_dir,
            db_path,
        )
    }

    fn make_inventory() -> PlayerInventory {
        PlayerInventory {
            revision: InventoryRevision(7),
            containers: vec![ContainerState {
                id: "main_pack".to_string(),
                name: "主背包".to_string(),
                rows: 5,
                cols: 7,
                items: vec![PlacedItemState {
                    row: 0,
                    col: 0,
                    instance: ItemInstance {
                        instance_id: 77,
                        template_id: "starter_talisman".to_string(),
                        display_name: "启程护符".to_string(),
                        grid_w: 1,
                        grid_h: 1,
                        weight: 0.1,
                        rarity: ItemRarity::Common,
                        description: "fixture".to_string(),
                        stack_count: 1,
                        spirit_quality: 1.0,
                        durability: 1.0,
                        freshness: None,
                        mineral_id: None,
                    },
                }],
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 9,
            max_weight: 45.0,
        }
    }

    fn read_core_snapshot(db_path: &PathBuf) -> (String, f64, f64, f64, i64, f64) {
        let connection = Connection::open(db_path).expect("sqlite db should open");
        connection
            .query_row(
                "
                SELECT realm, spirit_qi, spirit_qi_max, karma, experience, inventory_score
                FROM player_core
                WHERE username = ?1
                ",
                params!["Azure"],
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
            .expect("player_core row should exist")
    }

    fn read_position_snapshot(db_path: &PathBuf) -> (f64, f64, f64) {
        let connection = Connection::open(db_path).expect("sqlite db should open");
        connection
            .query_row(
                "SELECT pos_x, pos_y, pos_z FROM player_slow WHERE username = ?1",
                params!["Azure"],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("player_slow row should exist")
    }

    fn read_inventory_json(db_path: &PathBuf) -> String {
        let connection = Connection::open(db_path).expect("sqlite db should open");
        connection
            .query_row(
                "SELECT inventory_json FROM inventories WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .expect("inventories row should exist")
    }

    fn read_ui_prefs_json(db_path: &PathBuf) -> String {
        let connection = Connection::open(db_path).expect("sqlite db should open");
        connection
            .query_row(
                "SELECT prefs_json FROM player_ui_prefs WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .expect("player_ui_prefs row should exist")
    }

    #[test]
    fn player_flushes_core_slow_inventory_and_ui_slices() {
        let (persistence, data_dir, db_path) = sqlite_persistence("flush-slices");
        crate::player::state::save_player_state(&persistence, "Azure", &PlayerState::default())
            .expect("baseline player state should persist");
        let mut app = App::new();
        app.insert_resource(persistence);
        app.insert_resource(PlayerStateAutosaveTimer {
            ticks: CORE_SLICE_FLUSH_INTERVAL_TICKS - 1,
        });
        app.add_systems(
            Update,
            (
                tick_player_persistence_timer,
                autosave_player_core_slices.after(tick_player_persistence_timer),
                autosave_player_slow_and_ui_slices.after(autosave_player_core_slices),
                autosave_player_progression_slices.after(autosave_player_slow_and_ui_slices),
                flush_changed_player_inventories.after(autosave_player_progression_slices),
            ),
        );

        let (mut client_bundle, _helper) = create_mock_client("Azure");
        client_bundle.player.position = Position::new([42.0, 77.0, -3.5]);
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(entity).insert(PlayerState {
            realm: "qi_refining_3".to_string(),
            spirit_qi: 78.0,
            spirit_qi_max: 100.0,
            karma: 0.2,
            experience: 1_200,
            inventory_score: 0.4,
        });
        app.world_mut().entity_mut(entity).insert(make_inventory());

        app.update();

        let (realm, spirit_qi, spirit_qi_max, karma, experience, inventory_score) =
            read_core_snapshot(&db_path);
        let (pos_x, pos_y, pos_z) = read_position_snapshot(&db_path);
        let inventory_json = read_inventory_json(&db_path);
        let prefs_json = read_ui_prefs_json(&db_path);

        assert_eq!(realm, "mortal");
        assert_eq!(spirit_qi, 78.0);
        assert_eq!(spirit_qi_max, 100.0);
        assert_eq!(karma, 0.2);
        assert_eq!(experience, 0);
        assert_eq!(inventory_score, 0.4);
        assert_eq!((pos_x, pos_y, pos_z), (8.0, 150.0, 8.0));
        assert_ne!(
            serde_json::from_str::<serde_json::Value>(&inventory_json)
                .expect("inventory_json should decode"),
            serde_json::Value::Null
        );
        assert!(serde_json::from_str::<serde_json::Value>(&prefs_json)
            .expect("prefs_json should decode")
            .get("quick_slots")
            .is_some());

        app.world_mut()
            .resource_mut::<PlayerStateAutosaveTimer>()
            .ticks = SLOW_UI_SLICE_FLUSH_INTERVAL_TICKS - 1;
        app.update();

        let (realm_after_slow, _, _, _, experience_after_slow, _) = read_core_snapshot(&db_path);
        let (pos_x_after_slow, pos_y_after_slow, pos_z_after_slow) =
            read_position_snapshot(&db_path);

        assert_eq!(realm_after_slow, "mortal");
        assert_eq!(experience_after_slow, 0);
        assert_eq!(
            (pos_x_after_slow, pos_y_after_slow, pos_z_after_slow),
            (42.0, 77.0, -3.5)
        );

        app.world_mut()
            .resource_mut::<PlayerStateAutosaveTimer>()
            .ticks = PROGRESSION_SLICE_FLUSH_INTERVAL_TICKS - 1;
        app.update();

        let (
            realm_after_progression,
            _,
            spirit_qi_max_after_progression,
            _,
            experience_after_progression,
            _,
        ) = read_core_snapshot(&db_path);

        assert_eq!(realm_after_progression, "qi_refining_3");
        assert_eq!(spirit_qi_max_after_progression, 100.0);
        assert_eq!(experience_after_progression, 1_200);

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn disconnect_flush_persists_latest_player_slices_before_cleanup() {
        let (persistence, data_dir, db_path) = sqlite_persistence("disconnect-flush");
        crate::player::state::save_player_state(&persistence, "Azure", &PlayerState::default())
            .expect("baseline player state should persist");

        let mut app = App::new();
        app.insert_resource(persistence);
        app.add_systems(Update, despawn_disconnected_clients);

        let (mut client_bundle, _helper) = create_mock_client("Azure");
        client_bundle.player.position = Position::new([42.0, 77.0, -3.5]);
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(entity).insert(PlayerState {
            realm: "qi_refining_4".to_string(),
            spirit_qi: 88.0,
            spirit_qi_max: 120.0,
            karma: -0.15,
            experience: 2_400,
            inventory_score: 0.7,
        });
        app.world_mut().entity_mut(entity).insert(make_inventory());

        app.world_mut().entity_mut(entity).remove::<Client>();
        app.update();

        let (realm, spirit_qi, spirit_qi_max, karma, experience, inventory_score) =
            read_core_snapshot(&db_path);
        let (pos_x, pos_y, pos_z) = read_position_snapshot(&db_path);
        let inventory_json = read_inventory_json(&db_path);

        assert_eq!(realm, "qi_refining_4");
        assert_eq!(spirit_qi, 88.0);
        assert_eq!(spirit_qi_max, 120.0);
        assert_eq!(karma, -0.15);
        assert_eq!(experience, 2_400);
        assert_eq!(inventory_score, 0.7);
        assert_eq!((pos_x, pos_y, pos_z), (42.0, 77.0, -3.5));
        assert_ne!(
            serde_json::from_str::<serde_json::Value>(&inventory_json)
                .expect("inventory_json should decode"),
            serde_json::Value::Null
        );
        assert!(
            app.world().get::<Despawned>(entity).is_some(),
            "disconnect cleanup should mark entity as despawned"
        );

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn shutdown_flush_persists_connected_player_slices_without_disconnect() {
        let (persistence, data_dir, db_path) = sqlite_persistence("shutdown-flush");
        crate::player::state::save_player_state(&persistence, "Azure", &PlayerState::default())
            .expect("baseline player state should persist");

        let mut app = App::default();
        app.insert_resource(persistence);
        app.add_systems(Last, flush_connected_players_on_shutdown);

        let (mut client_bundle, _helper) = create_mock_client("Azure");
        client_bundle.player.position = Position::new([64.0, 80.0, -12.0]);
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(entity).insert(PlayerState {
            realm: "qi_refining_5".to_string(),
            spirit_qi: 91.0,
            spirit_qi_max: 140.0,
            karma: 0.33,
            experience: 3_200,
            inventory_score: 0.85,
        });
        app.world_mut().entity_mut(entity).insert(make_inventory());

        app.world_mut().send_event(AppExit::Success);
        app.update();

        let (realm, spirit_qi, spirit_qi_max, karma, experience, inventory_score) =
            read_core_snapshot(&db_path);
        let (pos_x, pos_y, pos_z) = read_position_snapshot(&db_path);
        let inventory_json = read_inventory_json(&db_path);

        assert_eq!(realm, "qi_refining_5");
        assert_eq!(spirit_qi, 91.0);
        assert_eq!(spirit_qi_max, 140.0);
        assert_eq!(karma, 0.33);
        assert_eq!(experience, 3_200);
        assert_eq!(inventory_score, 0.85);
        assert_eq!((pos_x, pos_y, pos_z), (64.0, 80.0, -12.0));
        assert_ne!(
            serde_json::from_str::<serde_json::Value>(&inventory_json)
                .expect("inventory_json should decode"),
            serde_json::Value::Null
        );
        assert!(
            app.world().get::<Client>(entity).is_some(),
            "shutdown flush should persist while the player is still connected"
        );

        let _ = fs::remove_dir_all(&data_dir);
    }
}
