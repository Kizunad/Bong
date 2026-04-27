pub mod gameplay;
pub mod state;

use self::state::{
    load_player_slices, save_player_core_slice, save_player_inventory_slice,
    save_player_lifespan_slice, save_player_skill_slice, save_player_slices,
    save_player_slow_slice, PlayerState, PlayerStateAutosaveTimer, PlayerStatePersistence,
};
use crate::combat::components::TICKS_PER_SECOND;
use crate::cultivation::color::PracticeLog;
use crate::cultivation::components::{Contamination, Cultivation, Karma, MeridianSystem, QiColor};
use crate::cultivation::insight::InsightQuota;
use crate::cultivation::insight_apply::{InsightModifiers, UnlockedPerceptions};
use crate::cultivation::life_record::LifeRecord;
use crate::cultivation::lifespan::LifespanComponent;
use crate::inventory::{attach_inventory_to_joined_clients, PlayerInventory};
use crate::persistence::persist_player_cultivation_bundle;
use crate::persistence::PersistenceSettings;
use crate::skill::components::SkillSet;
use crate::world::dimension::{CurrentDimension, DimensionKind, DimensionLayers};
use valence::message::SendMessage;
use valence::prelude::Despawned;
use valence::prelude::{
    Added, App, AppExit, Changed, Client, Commands, Entity, EntityLayerId, EventReader, GameMode,
    IntoSystemConfigs, Last, Position, Query, RemovedComponents, Res, ResMut, Update, Username,
    VisibleChunkLayer, VisibleEntityLayers, With, Without,
};

const SPAWN_POSITION: [f64; 3] = [8.0, 150.0, 8.0];
const WELCOME_MESSAGE: &str =
    "Welcome to Bong! Test commands: !zones, !tpzone <zone>, !top, !gm <c|a|s>, !spawn";
const CORE_SLICE_FLUSH_INTERVAL_TICKS: u64 = 5 * TICKS_PER_SECOND;
const SLOW_UI_SLICE_FLUSH_INTERVAL_TICKS: u64 = 60 * TICKS_PER_SECOND;
const LIFESPAN_SLICE_FLUSH_INTERVAL_TICKS: u64 = 60 * TICKS_PER_SECOND;
const CULTIVATION_FLUSH_INTERVAL_TICKS: u64 = 60 * TICKS_PER_SECOND;

type ClientInitQueryItem<'a> = (
    Entity,
    &'a mut Client,
    &'a mut EntityLayerId,
    &'a mut VisibleChunkLayer,
    &'a mut VisibleEntityLayers,
    &'a mut Position,
    &'a mut GameMode,
);

type JoinedClientsWithoutStateQueryItem<'a> = (
    Entity,
    &'a Username,
    &'a mut EntityLayerId,
    &'a mut VisibleChunkLayer,
    &'a mut VisibleEntityLayers,
);
type JoinedClientsWithoutStateQueryFilter = (Added<Client>, Without<PlayerState>);
type ChangedInventoryClientsQueryItem<'a> = (&'a Username, &'a PlayerInventory);
type ChangedInventoryClientsQueryFilter = (With<Client>, Changed<PlayerInventory>);
type ChangedSkillClientsQueryItem<'a> = (&'a Username, &'a SkillSet);
type ChangedSkillClientsQueryFilter = (With<Client>, Changed<SkillSet>);
type CultivationBundleQueryItem<'a> = (
    &'a Username,
    &'a Cultivation,
    &'a MeridianSystem,
    &'a QiColor,
    &'a Karma,
    &'a PracticeLog,
    &'a Contamination,
    &'a LifeRecord,
    &'a InsightQuota,
    &'a UnlockedPerceptions,
    &'a InsightModifiers,
);

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
            autosave_player_cultivation_bundles.after(autosave_player_slow_and_ui_slices),
            autosave_player_lifespan_slices.after(autosave_player_cultivation_bundles),
            flush_changed_player_skills.after(autosave_player_lifespan_slices),
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
    mut commands: Commands,
    mut clients: Query<ClientInitQueryItem<'_>, Added<Client>>,
    dimension_layers: Option<Res<DimensionLayers>>,
) {
    // Spawn defaults route every client into the overworld layer. The follow-up
    // `attach_player_state_to_joined_clients` system reads persisted state and
    // reroutes the client to its `last_dimension` (and inserts a matching
    // `CurrentDimension`) before any client packets are flushed this tick.
    // `DimensionLayers` is missing only in tests that do not bootstrap the world
    // plugin — fall through silently in that case.
    let Some(dimension_layers) = dimension_layers else {
        return;
    };
    let layer = dimension_layers.overworld;

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
        commands.entity(entity).insert(CurrentDimension::default());

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
    dimension_layers: Option<Res<DimensionLayers>>,
    mut joined_clients: Query<
        JoinedClientsWithoutStateQueryItem<'_>,
        JoinedClientsWithoutStateQueryFilter,
    >,
) {
    for (entity, username, mut layer_id, mut visible_chunk_layer, mut visible_entity_layers) in
        &mut joined_clients
    {
        let persisted = load_player_slices(&persistence, username.0.as_str());
        let restored_inventory = persisted.inventory.is_some();
        let restored_lifespan = persisted.lifespan.is_some();
        let restored_skill = !persisted.skill_set.skills.is_empty()
            || !persisted.skill_set.consumed_scrolls.is_empty();
        let last_dimension = persisted.last_dimension;
        let composite_power = persisted.state.composite_power(&Cultivation::default());

        if let Some(layers) = dimension_layers.as_deref() {
            let target_layer = layers.entity_for(last_dimension);
            let previous_layer = layer_id.0;
            if previous_layer != target_layer {
                visible_entity_layers.0.remove(&previous_layer);
                layer_id.0 = target_layer;
                visible_chunk_layer.0 = target_layer;
                visible_entity_layers.0.insert(target_layer);
            }
        }

        let mut entity_commands = commands.entity(entity);
        entity_commands.insert((
            persisted.state,
            Position::new(persisted.position),
            CurrentDimension(last_dimension),
        ));
        if let Some(player_inventory) = persisted.inventory {
            entity_commands.insert(player_inventory);
        }
        if let Some(lifespan) = persisted.lifespan {
            entity_commands.insert(lifespan);
        }
        entity_commands.insert(persisted.skill_set);
        tracing::info!(
            "[bong][player] attached PlayerState to client entity {entity:?} for `{}` (composite_power={composite_power:.3}, restored_inventory={restored_inventory}, restored_lifespan={restored_lifespan}, restored_skill={restored_skill}, last_dimension={last_dimension:?})",
            username.0,
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
    settings: Res<PersistenceSettings>,
    core_players: Query<(
        &Username,
        &PlayerState,
        &Position,
        Option<&CurrentDimension>,
        Option<&PlayerInventory>,
        Option<&LifespanComponent>,
        Option<&SkillSet>,
    )>,
    cultivation_bundle: Query<(
        &Cultivation,
        &MeridianSystem,
        &QiColor,
        &Karma,
        &PracticeLog,
        &Contamination,
        &LifeRecord,
        &InsightQuota,
        &UnlockedPerceptions,
        &InsightModifiers,
    )>,
) {
    for entity in disconnected_clients.read() {
        if let Ok((
            username,
            player_state,
            position,
            current_dimension,
            player_inventory,
            lifespan,
            skill_set,
        )) = core_players.get(entity)
        {
            let last_dimension = current_dimension
                .map(|cd| cd.0)
                .unwrap_or(DimensionKind::default());

            if let Ok((
                cultivation,
                meridians,
                qi_color,
                karma,
                practice_log,
                contamination,
                life_record,
                insight_quota,
                unlocked_perceptions,
                insight_modifiers,
            )) = cultivation_bundle.get(entity)
            {
                if let Err(error) = persist_player_cultivation_bundle(
                    &settings,
                    username.0.as_str(),
                    cultivation,
                    meridians,
                    qi_color,
                    karma,
                    contamination,
                    life_record,
                    practice_log,
                    insight_quota,
                    unlocked_perceptions,
                    insight_modifiers,
                ) {
                    tracing::warn!(
                        "[bong][player] failed to persist cultivation bundle for disconnected client `{}`: {error}",
                        username.0,
                    );
                }
            }
            match save_player_slices(
                &persistence,
                username.0.as_str(),
                player_state,
                position_to_array(position),
                last_dimension,
                player_inventory,
                lifespan,
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
    settings: Res<PersistenceSettings>,
    players: Query<
        (
            Entity,
            &Username,
            &PlayerState,
            &Position,
            Option<&CurrentDimension>,
            Option<&PlayerInventory>,
            Option<&LifespanComponent>,
            Option<&SkillSet>,
        ),
        With<Client>,
    >,
    cultivation_bundle: Query<(
        &Cultivation,
        &MeridianSystem,
        &QiColor,
        &Karma,
        &PracticeLog,
        &Contamination,
        &LifeRecord,
        &InsightQuota,
        &UnlockedPerceptions,
        &InsightModifiers,
    )>,
) {
    if app_exit.read().next().is_none() {
        return;
    }

    for (
        entity,
        username,
        player_state,
        position,
        current_dimension,
        player_inventory,
        lifespan,
        skill_set,
    ) in &players
    {
        let last_dimension = current_dimension
            .map(|cd| cd.0)
            .unwrap_or(DimensionKind::default());

        if let Ok((
            cultivation,
            meridians,
            qi_color,
            karma,
            practice_log,
            contamination,
            life_record,
            insight_quota,
            unlocked_perceptions,
            insight_modifiers,
        )) = cultivation_bundle.get(entity)
        {
            if let Err(error) = persist_player_cultivation_bundle(
                &settings,
                username.0.as_str(),
                cultivation,
                meridians,
                qi_color,
                karma,
                contamination,
                life_record,
                practice_log,
                insight_quota,
                unlocked_perceptions,
                insight_modifiers,
            ) {
                tracing::warn!(
                    "[bong][player] failed to persist cultivation bundle during shutdown flush for `{}`: {error}",
                    username.0,
                );
            }
        }
        match save_player_slices(
            &persistence,
            username.0.as_str(),
            player_state,
            position_to_array(position),
            last_dimension,
            player_inventory,
            lifespan,
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
    players: Query<(&Username, &Position, Option<&CurrentDimension>), With<Client>>,
) {
    if !timer
        .ticks
        .is_multiple_of(SLOW_UI_SLICE_FLUSH_INTERVAL_TICKS)
    {
        return;
    }

    let mut saved_count = 0usize;

    for (username, position, current_dimension) in &players {
        let last_dimension = current_dimension
            .map(|cd| cd.0)
            .unwrap_or(DimensionKind::default());
        match save_player_slow_slice(
            &persistence,
            username.0.as_str(),
            position_to_array(position),
            last_dimension,
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

fn autosave_player_cultivation_bundles(
    settings: Res<PersistenceSettings>,
    timer: Res<PlayerStateAutosaveTimer>,
    players: Query<CultivationBundleQueryItem<'_>, With<Client>>,
) {
    if !timer.ticks.is_multiple_of(CULTIVATION_FLUSH_INTERVAL_TICKS) {
        return;
    }

    let mut saved_count = 0usize;

    for (
        username,
        cultivation,
        meridians,
        qi_color,
        karma,
        practice_log,
        contamination,
        life_record,
        insight_quota,
        unlocked_perceptions,
        insight_modifiers,
    ) in &players
    {
        match persist_player_cultivation_bundle(
            &settings,
            username.0.as_str(),
            cultivation,
            meridians,
            qi_color,
            karma,
            contamination,
            life_record,
            practice_log,
            insight_quota,
            unlocked_perceptions,
            insight_modifiers,
        ) {
            Ok(()) => saved_count += 1,
            Err(error) => tracing::warn!(
                "[bong][player] 60s cultivation flush failed for `{}`: {error}",
                username.0,
            ),
        }
    }

    tracing::info!(
        "[bong][player] flushed {saved_count} cultivation bundle(s) after {CULTIVATION_FLUSH_INTERVAL_TICKS} ticks"
    );
}

fn autosave_player_lifespan_slices(
    persistence: Res<PlayerStatePersistence>,
    timer: Res<PlayerStateAutosaveTimer>,
    players: Query<(&Username, &LifespanComponent), With<Client>>,
) {
    if !timer
        .ticks
        .is_multiple_of(LIFESPAN_SLICE_FLUSH_INTERVAL_TICKS)
    {
        return;
    }

    let mut saved_count = 0usize;

    for (username, lifespan) in &players {
        match save_player_lifespan_slice(&persistence, username.0.as_str(), lifespan) {
            Ok(_) => saved_count += 1,
            Err(error) => tracing::warn!(
                "[bong][player] 60s lifespan flush failed for `{}`: {error}",
                username.0,
            ),
        }
    }

    tracing::info!(
        "[bong][player] flushed {saved_count} lifespan slice(s) after {LIFESPAN_SLICE_FLUSH_INTERVAL_TICKS} ticks"
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
                        charges: None,
                    },
                }],
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 9,
            max_weight: 45.0,
        }
    }

    fn read_core_snapshot(db_path: &PathBuf) -> (f64, f64) {
        let connection = Connection::open(db_path).expect("sqlite db should open");
        connection
            .query_row(
                "
                SELECT karma, inventory_score
                FROM player_core
                WHERE username = ?1
                ",
                params!["Azure"],
                |row| Ok((row.get(0)?, row.get(1)?)),
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

    fn read_cultivation_json(db_path: &PathBuf) -> String {
        let connection = Connection::open(db_path).expect("sqlite db should open");
        connection
            .query_row(
                "SELECT cultivation_json FROM player_cultivation WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .expect("player_cultivation row should exist")
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
                flush_changed_player_inventories.after(autosave_player_slow_and_ui_slices),
            ),
        );

        let (mut client_bundle, _helper) = create_mock_client("Azure");
        client_bundle.player.position = Position::new([42.0, 77.0, -3.5]);
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(entity).insert(PlayerState {
            karma: 0.2,
            inventory_score: 0.4,
        });
        app.world_mut().entity_mut(entity).insert(make_inventory());

        app.update();

        let (karma, inventory_score) = read_core_snapshot(&db_path);
        let (pos_x, pos_y, pos_z) = read_position_snapshot(&db_path);
        let inventory_json = read_inventory_json(&db_path);
        let prefs_json = read_ui_prefs_json(&db_path);

        assert_eq!(karma, 0.2);
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

        let (karma_after_slow, inventory_score_after_slow) = read_core_snapshot(&db_path);
        let (pos_x_after_slow, pos_y_after_slow, pos_z_after_slow) =
            read_position_snapshot(&db_path);

        assert_eq!(karma_after_slow, 0.2);
        assert_eq!(inventory_score_after_slow, 0.4);
        assert_eq!(
            (pos_x_after_slow, pos_y_after_slow, pos_z_after_slow),
            (42.0, 77.0, -3.5)
        );

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn cultivation_bundle_flushes_periodically() {
        let (persistence, data_dir, db_path) = sqlite_persistence("cultivation-flush");
        let mut app = App::new();
        app.insert_resource(PersistenceSettings::with_paths(
            &db_path,
            data_dir.join("deceased"),
            "player-cultivation-flush",
        ));
        app.insert_resource(PlayerStateAutosaveTimer {
            ticks: CULTIVATION_FLUSH_INTERVAL_TICKS - 1,
        });
        app.add_systems(
            Update,
            (
                tick_player_persistence_timer,
                autosave_player_cultivation_bundles.after(tick_player_persistence_timer),
            ),
        );

        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(entity).insert((
            Cultivation {
                realm: crate::cultivation::components::Realm::Condense,
                qi_current: 42.0,
                qi_max: 88.0,
                ..Default::default()
            },
            MeridianSystem::default(),
            QiColor::default(),
            Karma::default(),
            PracticeLog::default(),
            Contamination::default(),
            LifeRecord::new(crate::player::state::canonical_player_id("Azure")),
            InsightQuota::default(),
            UnlockedPerceptions::default(),
            InsightModifiers::new(),
        ));

        app.update();

        let cultivation_json = read_cultivation_json(&db_path);
        let bundle: serde_json::Value =
            serde_json::from_str(&cultivation_json).expect("cultivation bundle should deserialize");
        assert_eq!(bundle["cultivation"]["realm"].as_str(), Some("Condense"));
        assert_eq!(bundle["cultivation"]["qi_current"].as_f64(), Some(42.0));
        assert_eq!(bundle["cultivation"]["qi_max"].as_f64(), Some(88.0));

        let _ = persistence;
        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn disconnect_flush_persists_latest_player_slices_before_cleanup() {
        let (persistence, data_dir, db_path) = sqlite_persistence("disconnect-flush");
        crate::player::state::save_player_state(&persistence, "Azure", &PlayerState::default())
            .expect("baseline player state should persist");

        let mut app = App::new();
        app.insert_resource(persistence);
        app.insert_resource(PersistenceSettings::with_paths(
            &db_path,
            data_dir.join("deceased"),
            "player-disconnect-flush",
        ));
        app.add_systems(Update, despawn_disconnected_clients);

        let (mut client_bundle, _helper) = create_mock_client("Azure");
        client_bundle.player.position = Position::new([42.0, 77.0, -3.5]);
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(entity).insert(PlayerState {
            karma: -0.15,
            inventory_score: 0.7,
        });
        app.world_mut().entity_mut(entity).insert(make_inventory());

        app.world_mut().entity_mut(entity).remove::<Client>();
        app.update();

        let (karma, inventory_score) = read_core_snapshot(&db_path);
        let (pos_x, pos_y, pos_z) = read_position_snapshot(&db_path);
        let inventory_json = read_inventory_json(&db_path);

        assert_eq!(karma, -0.15);
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
        app.insert_resource(PersistenceSettings::with_paths(
            &db_path,
            data_dir.join("deceased"),
            "player-shutdown-flush",
        ));
        app.add_systems(Last, flush_connected_players_on_shutdown);

        let (mut client_bundle, _helper) = create_mock_client("Azure");
        client_bundle.player.position = Position::new([64.0, 80.0, -12.0]);
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(entity).insert(PlayerState {
            karma: 0.33,
            inventory_score: 0.85,
        });
        app.world_mut().entity_mut(entity).insert(make_inventory());

        app.world_mut().send_event(AppExit::Success);
        app.update();

        let (karma, inventory_score) = read_core_snapshot(&db_path);
        let (pos_x, pos_y, pos_z) = read_position_snapshot(&db_path);
        let inventory_json = read_inventory_json(&db_path);

        assert_eq!(karma, 0.33);
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

    #[test]
    fn reconnecting_into_tsy_routes_layer_and_current_dimension() {
        use crate::world::dimension::{DimensionKind, DimensionLayers};

        let (persistence, data_dir, _db_path) = sqlite_persistence("reconnect-into-tsy");

        // Persist a player whose last dimension is Tsy so reconnect should
        // route them back into the TSY layer rather than the overworld default.
        crate::player::state::save_player_slices(
            &persistence,
            "Azure",
            &PlayerState::default(),
            [12.0, 80.0, -34.0],
            DimensionKind::Tsy,
            None,
            None,
            &SkillSet::default(),
        )
        .expect("seeding TSY-resident player should persist");

        let mut app = App::new();
        let overworld_layer = app.world_mut().spawn_empty().id();
        let tsy_layer = app.world_mut().spawn_empty().id();
        app.insert_resource(DimensionLayers {
            overworld: overworld_layer,
            tsy: tsy_layer,
        });
        app.insert_resource(persistence);
        app.add_systems(Update, attach_player_state_to_joined_clients);

        // Mock client bundle: Added<Client> fires this tick. Pre-set its layer
        // pointers to the overworld so we can verify attach reroutes them.
        let (mut client_bundle, _helper) = valence::testing::create_mock_client("Azure");
        client_bundle.player.layer.0 = overworld_layer;
        client_bundle.visible_chunk_layer.0 = overworld_layer;
        client_bundle
            .visible_entity_layers
            .0
            .insert(overworld_layer);
        let entity = app.world_mut().spawn(client_bundle).id();

        app.update();

        let world = app.world();
        let er = world.entity(entity);
        let current = er
            .get::<CurrentDimension>()
            .copied()
            .expect("attach should insert CurrentDimension");
        let layer_id = er
            .get::<EntityLayerId>()
            .expect("client bundle should carry EntityLayerId")
            .0;
        let visible_chunk = er
            .get::<VisibleChunkLayer>()
            .expect("client bundle should carry VisibleChunkLayer")
            .0;
        let visible_entities = &er
            .get::<VisibleEntityLayers>()
            .expect("client bundle should carry VisibleEntityLayers")
            .0;
        let position = er.get::<Position>().expect("position should be set").get();

        assert_eq!(current, CurrentDimension(DimensionKind::Tsy));
        assert_eq!(layer_id, tsy_layer);
        assert_eq!(visible_chunk, tsy_layer);
        assert!(visible_entities.contains(&tsy_layer));
        assert!(!visible_entities.contains(&overworld_layer));
        assert_eq!(position, DVec3::new(12.0, 80.0, -34.0));

        let _ = fs::remove_dir_all(&data_dir);
    }
}
