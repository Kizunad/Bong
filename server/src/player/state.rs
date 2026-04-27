use std::fs;
use std::io;
use std::path::PathBuf;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use valence::prelude::{bevy_ecs, Component, Resource};

use crate::cultivation::components::{Cultivation, Realm};
use crate::cultivation::lifespan::{
    lifespan_delta_years_for_real_seconds, LifespanComponent, LIFESPAN_OFFLINE_MULTIPLIER,
};
use crate::inventory::PlayerInventory;
use crate::persistence::DEFAULT_DATABASE_PATH;
use crate::schema::cultivation::realm_to_string;
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
use crate::schema::world_state::PlayerPowerBreakdown;
use crate::skill::components::SkillSet;
use crate::world::dimension::DimensionKind;

pub const DEFAULT_PLAYER_DATA_DIR: &str = "data/players";

const PLAYER_ROW_SCHEMA_VERSION: i32 = 1;
const DEFAULT_INVENTORY_JSON: &str = "null";

#[derive(Clone, Debug, Component, Serialize, Deserialize, PartialEq)]
pub struct PlayerState {
    pub karma: f64,
    pub inventory_score: f64,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            karma: 0.0,
            inventory_score: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
struct PlayerUiPrefs {
    quick_slots: [Option<String>; 9],
}

#[derive(Debug, Clone)]
pub struct LoadedPlayerSlices {
    pub state: PlayerState,
    pub position: [f64; 3],
    pub last_dimension: DimensionKind,
    pub inventory: Option<PlayerInventory>,
    pub lifespan: Option<LifespanComponent>,
    pub skill_set: SkillSet,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerExportBundle {
    pub kind: String,
    pub username: String,
    pub current_char_id: String,
    pub state: PlayerState,
    pub position: [f64; 3],
    #[serde(default)]
    pub last_dimension: DimensionKind,
    pub inventory: Option<PlayerInventory>,
    pub skill_set: SkillSet,
    pub ui_prefs: serde_json::Value,
}

impl PlayerState {
    pub fn normalized(&self) -> Self {
        Self {
            karma: self.karma.clamp(-1.0, 1.0),
            inventory_score: clamp_unit(self.inventory_score),
        }
    }

    pub fn power_breakdown(&self, cultivation: &Cultivation) -> PlayerPowerBreakdown {
        let normalized = self.normalized();
        let realm_score = realm_progress_score(cultivation.realm);
        let qi_ratio = ratio_score(cultivation.qi_current, cultivation.qi_max);
        let wealth = clamp_unit(normalized.inventory_score);
        let karma_alignment = ((normalized.karma + 1.0) * 0.5).clamp(0.0, 1.0);
        let karma_influence = normalized.karma.abs().clamp(0.0, 1.0);

        PlayerPowerBreakdown {
            combat: clamp_unit(realm_score * 0.6 + qi_ratio * 0.4),
            wealth,
            social: clamp_unit(realm_score * 0.6 + karma_alignment * 0.4),
            karma: karma_influence,
            territory: clamp_unit(realm_score * 0.5 + wealth * 0.5),
        }
    }

    pub fn composite_power(&self, cultivation: &Cultivation) -> f64 {
        let breakdown = self.power_breakdown(cultivation);

        clamp_unit(
            breakdown.combat * 0.4
                + breakdown.wealth * 0.15
                + breakdown.social * 0.15
                + breakdown.karma * 0.15
                + breakdown.territory * 0.15,
        )
    }

    pub fn server_payload(
        &self,
        cultivation: &Cultivation,
        player: Option<String>,
        zone: impl Into<String>,
    ) -> ServerDataV1 {
        let normalized = self.normalized();
        let breakdown = normalized.power_breakdown(cultivation);
        let composite_power = clamp_unit(
            breakdown.combat * 0.4
                + breakdown.wealth * 0.15
                + breakdown.social * 0.15
                + breakdown.karma * 0.15
                + breakdown.territory * 0.15,
        );

        ServerDataV1::new(ServerDataPayloadV1::PlayerState {
            player,
            realm: realm_to_string(cultivation.realm).to_string(),
            spirit_qi: cultivation.qi_current,
            karma: normalized.karma,
            composite_power,
            breakdown,
            zone: zone.into(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct PlayerStatePersistence {
    data_dir: PathBuf,
    db_path: PathBuf,
}

impl Default for PlayerStatePersistence {
    fn default() -> Self {
        Self::new(DEFAULT_PLAYER_DATA_DIR)
    }
}

impl Resource for PlayerStatePersistence {}

impl PlayerStatePersistence {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self::with_db_path(data_dir, DEFAULT_DATABASE_PATH)
    }

    pub fn with_db_path(data_dir: impl Into<PathBuf>, db_path: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
            db_path: db_path.into(),
        }
    }

    pub fn db_path(&self) -> &std::path::Path {
        self.db_path.as_path()
    }

    #[cfg(test)]
    pub fn data_dir(&self) -> &std::path::Path {
        self.data_dir.as_path()
    }

    pub fn path_for_username(&self, username: &str) -> PathBuf {
        let player_key = canonical_player_id(username);
        self.data_dir.join(format!("{player_key}.json"))
    }

    fn migrated_path_for_username(&self, username: &str) -> PathBuf {
        let player_key = canonical_player_id(username);
        self.data_dir.join(format!("{player_key}.json.migrated"))
    }
}

#[derive(Debug, Default)]
pub struct PlayerStateAutosaveTimer {
    pub ticks: u64,
}

impl Resource for PlayerStateAutosaveTimer {}

pub fn canonical_player_id(username: &str) -> String {
    format!("offline:{username}")
}

pub fn player_character_id(username: &str, current_char_id: &str) -> String {
    if current_char_id.trim().is_empty() {
        canonical_player_id(username)
    } else {
        format!("{}:{current_char_id}", canonical_player_id(username))
    }
}

pub fn player_username_from_character_id(character_id: &str) -> Option<&str> {
    let rest = character_id.strip_prefix("offline:")?;
    let username = rest.split_once(':').map_or(rest, |(username, _)| username);
    if username.is_empty() {
        None
    } else {
        Some(username)
    }
}

pub fn load_current_character_id(
    persistence: &PlayerStatePersistence,
    username: &str,
) -> io::Result<Option<String>> {
    let connection = open_player_connection(persistence)?;
    ensure_player_schema(&connection)?;
    connection
        .query_row(
            "SELECT current_char_id FROM player_core WHERE username = ?1",
            params![username],
            |row| row.get(0),
        )
        .optional()
        .map_err(io::Error::other)
}

pub fn load_player_state(persistence: &PlayerStatePersistence, username: &str) -> PlayerState {
    let mut connection = match open_player_connection(persistence) {
        Ok(connection) => connection,
        Err(error) => {
            tracing::warn!(
                "[bong][player] failed to open sqlite PlayerState store for `{}` at {}: {error}; using default state",
                username,
                persistence.db_path().display()
            );
            return PlayerState::default();
        }
    };

    match load_player_state_from_sqlite(&connection, username) {
        Ok(Some(state)) => {
            if let Err(error) = ensure_player_auxiliary_rows(&mut connection, username) {
                tracing::warn!(
                    "[bong][player] failed to ensure auxiliary sqlite rows for `{}`: {error}",
                    username
                );
            }
            return state;
        }
        Ok(None) => {}
        Err(error) => {
            tracing::warn!(
                "[bong][player] failed to load PlayerState for `{}` from sqlite {}: {error}; using default state",
                username,
                persistence.db_path().display()
            );
            return PlayerState::default();
        }
    }

    match migrate_legacy_player_json_to_sqlite(persistence, &mut connection, username) {
        Ok(Some(state)) => return state,
        Ok(None) => {}
        Err(error) => tracing::warn!(
            "[bong][player] failed to migrate legacy PlayerState for `{}` from {}: {error}; using default state",
            username,
            persistence.path_for_username(username).display()
        ),
    }

    let default_state = PlayerState::default();
    if let Err(error) = save_player_state(persistence, username, &default_state) {
        tracing::warn!(
            "[bong][player] failed to initialize default sqlite PlayerState for `{}`: {error}",
            username
        );
    } else {
        tracing::warn!(
            "[bong][player] no sqlite PlayerState for `{}`; initialized default state in {}",
            username,
            persistence.db_path().display()
        );
    }

    default_state
}

pub fn load_player_slices(
    persistence: &PlayerStatePersistence,
    username: &str,
) -> LoadedPlayerSlices {
    let state = load_player_state(persistence, username);
    let connection = match open_player_connection(persistence) {
        Ok(connection) => connection,
        Err(error) => {
            tracing::warn!(
                "[bong][player] failed to reopen sqlite player slice store for `{}` at {}: {error}; using default slow/inventory slices",
                username,
                persistence.db_path().display()
            );
            return LoadedPlayerSlices {
                state,
                position: crate::player::spawn_position(),
                last_dimension: DimensionKind::default(),
                inventory: None,
                lifespan: None,
                skill_set: SkillSet::default(),
            };
        }
    };

    let (position, last_dimension) = match load_player_slow_from_sqlite(&connection, username) {
        Ok(Some((pos, dim))) => (pos, dim),
        Ok(None) => (crate::player::spawn_position(), DimensionKind::default()),
        Err(error) => {
            tracing::warn!(
                "[bong][player] failed to load persisted position/dimension for `{}` from sqlite {}: {error}; using spawn defaults",
                username,
                persistence.db_path().display()
            );
            (crate::player::spawn_position(), DimensionKind::default())
        }
    };
    let inventory = match load_player_inventory_from_sqlite(&connection, username) {
        Ok(inventory) => inventory,
        Err(error) => {
            tracing::warn!(
                "[bong][player] failed to load persisted inventory for `{}` from sqlite {}: {error}; using default inventory fallback",
                username,
                persistence.db_path().display()
            );
            None
        }
    };
    let lifespan = match load_player_lifespan_from_sqlite(&connection, username) {
        Ok(lifespan) => lifespan,
        Err(error) => {
            tracing::warn!(
                "[bong][player] failed to load persisted lifespan for `{}` from sqlite {}: {error}; using runtime default",
                username,
                persistence.db_path().display()
            );
            None
        }
    };
    let skill_set = match load_player_skill_set_from_sqlite(&connection, username) {
        Ok(skill_set) => skill_set,
        Err(error) => {
            tracing::warn!(
                "[bong][player] failed to load persisted skill set for `{}` from sqlite {}: {error}; using default skill set",
                username,
                persistence.db_path().display()
            );
            SkillSet::default()
        }
    };

    LoadedPlayerSlices {
        state,
        position,
        last_dimension,
        inventory,
        lifespan,
        skill_set,
    }
}

pub fn load_player_shrine_anchor_slice(
    persistence: &PlayerStatePersistence,
    username: &str,
) -> io::Result<Option<[f64; 3]>> {
    let connection = open_player_connection(persistence)?;
    load_player_shrine_anchor_from_sqlite(&connection, username)
}

pub fn save_player_shrine_anchor_slice(
    persistence: &PlayerStatePersistence,
    username: &str,
    anchor: Option<[f64; 3]>,
) -> io::Result<PathBuf> {
    let mut connection = open_player_connection(persistence)?;
    persist_player_shrine_anchor_slice_in_sqlite(&mut connection, username, anchor)?;
    Ok(persistence.db_path().to_path_buf())
}

pub fn save_player_state(
    persistence: &PlayerStatePersistence,
    username: &str,
    state: &PlayerState,
) -> io::Result<PathBuf> {
    save_player_slices(
        persistence,
        username,
        state,
        crate::player::spawn_position(),
        DimensionKind::default(),
        None,
        None,
        &SkillSet::default(),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn save_player_slices(
    persistence: &PlayerStatePersistence,
    username: &str,
    state: &PlayerState,
    position: [f64; 3],
    last_dimension: DimensionKind,
    inventory: Option<&PlayerInventory>,
    lifespan: Option<&LifespanComponent>,
    skill_set: &SkillSet,
) -> io::Result<PathBuf> {
    let mut connection = open_player_connection(persistence)?;
    persist_player_slices_in_sqlite(
        &mut connection,
        username,
        state,
        position,
        last_dimension,
        inventory,
        lifespan,
        skill_set,
    )?;
    Ok(persistence.db_path().to_path_buf())
}

pub fn save_player_lifespan_slice(
    persistence: &PlayerStatePersistence,
    username: &str,
    lifespan: &LifespanComponent,
) -> io::Result<PathBuf> {
    let mut connection = open_player_connection(persistence)?;
    persist_player_lifespan_slice_in_sqlite(&mut connection, username, lifespan, None)?;
    Ok(persistence.db_path().to_path_buf())
}

pub fn save_player_core_slice(
    persistence: &PlayerStatePersistence,
    username: &str,
    state: &PlayerState,
) -> io::Result<PathBuf> {
    let mut connection = open_player_connection(persistence)?;
    persist_player_core_slice_in_sqlite(&mut connection, username, state)?;
    Ok(persistence.db_path().to_path_buf())
}

pub fn save_player_slow_slice(
    persistence: &PlayerStatePersistence,
    username: &str,
    position: [f64; 3],
    last_dimension: DimensionKind,
) -> io::Result<PathBuf> {
    let mut connection = open_player_connection(persistence)?;
    persist_player_slow_slice_in_sqlite(&mut connection, username, position, last_dimension)?;
    Ok(persistence.db_path().to_path_buf())
}

pub fn save_player_inventory_slice(
    persistence: &PlayerStatePersistence,
    username: &str,
    inventory: Option<&PlayerInventory>,
) -> io::Result<PathBuf> {
    let mut connection = open_player_connection(persistence)?;
    persist_player_inventory_slice_in_sqlite(&mut connection, username, inventory)?;
    Ok(persistence.db_path().to_path_buf())
}

pub fn rotate_current_character_id(
    persistence: &PlayerStatePersistence,
    username: &str,
) -> io::Result<String> {
    let connection = open_player_connection(persistence)?;
    ensure_player_schema(&connection)?;
    let next_char_id = Uuid::now_v7().to_string();
    let last_updated_wall = current_unix_seconds();

    connection
        .execute(
            "
            INSERT INTO player_core (
                username,
                current_char_id,
                karma,
                inventory_score,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, 0.0, 0.0, ?3, ?4)
            ON CONFLICT(username) DO UPDATE SET
                current_char_id = excluded.current_char_id,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                username,
                next_char_id,
                PLAYER_ROW_SCHEMA_VERSION,
                last_updated_wall
            ],
        )
        .map_err(io::Error::other)?;

    Ok(next_char_id)
}

fn ensure_player_schema(connection: &Connection) -> io::Result<()> {
    let has_player_core: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'player_core'",
            [],
            |row| row.get(0),
        )
        .map_err(io::Error::other)?;
    if has_player_core == 0 {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "player_core table is missing; bootstrap sqlite before loading character ids",
        ));
    }
    Ok(())
}

pub fn save_player_skill_slice(
    persistence: &PlayerStatePersistence,
    username: &str,
    skill_set: &SkillSet,
) -> io::Result<PathBuf> {
    let mut connection = open_player_connection(persistence)?;
    persist_player_skill_slice_in_sqlite(&mut connection, username, skill_set)?;
    Ok(persistence.db_path().to_path_buf())
}

pub fn export_player_bundle(
    persistence: &PlayerStatePersistence,
    username: &str,
) -> io::Result<PlayerExportBundle> {
    let loaded = load_player_slices(persistence, username);
    let connection = open_player_connection(persistence)?;
    let current_char_id: String = connection
        .query_row(
            "SELECT current_char_id FROM player_core WHERE username = ?1",
            params![username],
            |row| row.get(0),
        )
        .map_err(io::Error::other)?;
    let ui_prefs_json: String = connection
        .query_row(
            "SELECT prefs_json FROM player_ui_prefs WHERE username = ?1",
            params![username],
            |row| row.get(0),
        )
        .map_err(io::Error::other)?;
    let ui_prefs = serde_json::from_str(&ui_prefs_json)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    Ok(PlayerExportBundle {
        kind: "player_export_v1".to_string(),
        username: username.to_string(),
        current_char_id,
        state: loaded.state,
        position: loaded.position,
        last_dimension: loaded.last_dimension,
        inventory: loaded.inventory,
        skill_set: loaded.skill_set,
        ui_prefs,
    })
}

pub fn import_player_bundle(
    persistence: &PlayerStatePersistence,
    bundle: &PlayerExportBundle,
) -> io::Result<()> {
    if bundle.kind != "player_export_v1" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unexpected player export kind: {}", bundle.kind),
        ));
    }

    let _ = Uuid::parse_str(&bundle.current_char_id)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let ui_prefs = serde_json::from_value::<PlayerUiPrefs>(bundle.ui_prefs.clone())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let ui_prefs_json = serde_json::to_string(&ui_prefs)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let inventory_json = serialize_inventory_json(bundle.inventory.as_ref())?;
    let skill_set_json = serialize_skill_set_json(&bundle.skill_set)?;
    let normalized = bundle.state.normalized();
    let [pos_x, pos_y, pos_z] = bundle.position;
    let last_updated_wall = current_unix_seconds();
    let mut connection = open_player_connection(persistence)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;

    transaction
        .execute(
            "
            INSERT INTO player_core (
                username,
                current_char_id,
                karma,
                inventory_score,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(username) DO UPDATE SET
                current_char_id = excluded.current_char_id,
                karma = excluded.karma,
                inventory_score = excluded.inventory_score,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                bundle.username,
                bundle.current_char_id,
                normalized.karma,
                normalized.inventory_score,
                PLAYER_ROW_SCHEMA_VERSION,
                last_updated_wall
            ],
        )
        .map_err(io::Error::other)?;
    transaction
        .execute(
            "
            INSERT INTO player_slow (
                username,
                pos_x,
                pos_y,
                pos_z,
                last_dimension,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(username) DO UPDATE SET
                pos_x = excluded.pos_x,
                pos_y = excluded.pos_y,
                pos_z = excluded.pos_z,
                last_dimension = excluded.last_dimension,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                bundle.username,
                pos_x,
                pos_y,
                pos_z,
                dimension_kind_to_sql(bundle.last_dimension),
                PLAYER_ROW_SCHEMA_VERSION,
                last_updated_wall
            ],
        )
        .map_err(io::Error::other)?;
    transaction
        .execute(
            "
            INSERT INTO inventories (
                username,
                inventory_json,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(username) DO UPDATE SET
                inventory_json = excluded.inventory_json,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                bundle.username,
                inventory_json,
                PLAYER_ROW_SCHEMA_VERSION,
                last_updated_wall
            ],
        )
        .map_err(io::Error::other)?;
    transaction
        .execute(
            "
            INSERT INTO player_skills (
                username,
                skill_set_json,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(username) DO UPDATE SET
                skill_set_json = excluded.skill_set_json,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                bundle.username,
                skill_set_json,
                PLAYER_ROW_SCHEMA_VERSION,
                last_updated_wall
            ],
        )
        .map_err(io::Error::other)?;
    transaction
        .execute(
            "
            INSERT INTO player_ui_prefs (
                username,
                prefs_json,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(username) DO UPDATE SET
                prefs_json = excluded.prefs_json,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                bundle.username,
                ui_prefs_json,
                PLAYER_ROW_SCHEMA_VERSION,
                last_updated_wall
            ],
        )
        .map_err(io::Error::other)?;

    transaction.commit().map_err(io::Error::other)
}

fn open_player_connection(persistence: &PlayerStatePersistence) -> io::Result<Connection> {
    if let Some(parent) = persistence.db_path().parent() {
        fs::create_dir_all(parent)?;
    }

    Connection::open(persistence.db_path()).map_err(io::Error::other)
}

fn load_player_state_from_sqlite(
    connection: &Connection,
    username: &str,
) -> io::Result<Option<PlayerState>> {
    let row: Option<(f64, f64)> = connection
        .query_row(
            "
            SELECT karma, inventory_score
            FROM player_core
            WHERE username = ?1
            ",
            params![username],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(io::Error::other)?;

    let Some((karma, inventory_score)) = row else {
        return Ok(None);
    };

    Ok(Some(
        PlayerState {
            karma,
            inventory_score,
        }
        .normalized(),
    ))
}

fn load_player_slow_from_sqlite(
    connection: &Connection,
    username: &str,
) -> io::Result<Option<([f64; 3], DimensionKind)>> {
    let row: Option<(f64, f64, f64, String)> = connection
        .query_row(
            "
            SELECT pos_x, pos_y, pos_z, last_dimension
            FROM player_slow
            WHERE username = ?1
            ",
            params![username],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(io::Error::other)?;

    let Some((pos_x, pos_y, pos_z, dimension_text)) = row else {
        return Ok(None);
    };

    let last_dimension = dimension_kind_from_sql(&dimension_text).unwrap_or_else(|error| {
        tracing::warn!(
            "[bong][player] unknown last_dimension `{dimension_text}` for `{username}`: {error}; defaulting to overworld"
        );
        DimensionKind::default()
    });

    Ok(Some(([pos_x, pos_y, pos_z], last_dimension)))
}

fn load_player_inventory_from_sqlite(
    connection: &Connection,
    username: &str,
) -> io::Result<Option<PlayerInventory>> {
    let inventory_json: Option<String> = connection
        .query_row(
            "
            SELECT inventory_json
            FROM inventories
            WHERE username = ?1
            ",
            params![username],
            |row| row.get(0),
        )
        .optional()
        .map_err(io::Error::other)?;

    let Some(inventory_json) = inventory_json else {
        return Ok(None);
    };

    if inventory_json.trim() == DEFAULT_INVENTORY_JSON {
        return Ok(None);
    }

    serde_json::from_str::<PlayerInventory>(&inventory_json)
        .map(Some)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn load_player_lifespan_from_sqlite(
    connection: &Connection,
    username: &str,
) -> io::Result<Option<LifespanComponent>> {
    let row: Option<(u64, f64, u32, i64)> = connection
        .query_row(
            "
            SELECT born_at_tick, years_lived, cap_by_realm, offline_pause_wall
            FROM player_lifespan
            WHERE username = ?1
            ",
            params![username],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(io::Error::other)?;

    let Some((born_at_tick, years_lived, cap_by_realm, offline_pause_wall)) = row else {
        return Ok(None);
    };
    let now_wall = current_unix_seconds();
    let offline_seconds = if offline_pause_wall > 0 {
        u64::try_from(now_wall.saturating_sub(offline_pause_wall)).unwrap_or(0)
    } else {
        0
    };
    let years_lived = years_lived
        + lifespan_delta_years_for_real_seconds(offline_seconds, LIFESPAN_OFFLINE_MULTIPLIER);
    let mut lifespan = LifespanComponent {
        born_at_tick,
        years_lived: years_lived.min(cap_by_realm as f64),
        cap_by_realm,
        offline_pause_tick: None,
    };
    lifespan.apply_cap(cap_by_realm.max(1));
    Ok(Some(lifespan))
}

fn load_player_shrine_anchor_from_sqlite(
    connection: &Connection,
    username: &str,
) -> io::Result<Option<[f64; 3]>> {
    let row: Option<(f64, f64, f64)> = connection
        .query_row(
            "
            SELECT anchor_x, anchor_y, anchor_z
            FROM player_shrine
            WHERE username = ?1
            ",
            params![username],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(io::Error::other)?;
    Ok(row.map(|(x, y, z)| [x, y, z]))
}

fn persist_player_shrine_anchor_slice_in_sqlite(
    connection: &mut Connection,
    username: &str,
    anchor: Option<[f64; 3]>,
) -> io::Result<()> {
    let last_updated_wall = current_unix_seconds();

    match anchor {
        Some([x, y, z]) => {
            connection
                .execute(
                    "
                    INSERT INTO player_shrine (
                        username,
                        anchor_x,
                        anchor_y,
                        anchor_z,
                        schema_version,
                        last_updated_wall
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                    ON CONFLICT(username) DO UPDATE SET
                        anchor_x = excluded.anchor_x,
                        anchor_y = excluded.anchor_y,
                        anchor_z = excluded.anchor_z,
                        schema_version = excluded.schema_version,
                        last_updated_wall = excluded.last_updated_wall
                    ",
                    params![
                        username,
                        x,
                        y,
                        z,
                        PLAYER_ROW_SCHEMA_VERSION,
                        last_updated_wall
                    ],
                )
                .map_err(io::Error::other)?;
        }
        None => {
            connection
                .execute(
                    "DELETE FROM player_shrine WHERE username = ?1",
                    params![username],
                )
                .map_err(io::Error::other)?;
        }
    }

    Ok(())
}

fn persist_player_lifespan_slice_in_sqlite(
    connection: &mut Connection,
    username: &str,
    lifespan: &LifespanComponent,
    offline_pause_wall: Option<i64>,
) -> io::Result<()> {
    let last_updated_wall = current_unix_seconds();
    let offline_pause_wall = offline_pause_wall.unwrap_or(last_updated_wall).max(0);
    connection
        .execute(
            "
            INSERT INTO player_lifespan (
                username,
                born_at_tick,
                years_lived,
                cap_by_realm,
                offline_pause_wall,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(username) DO UPDATE SET
                born_at_tick = excluded.born_at_tick,
                years_lived = excluded.years_lived,
                cap_by_realm = excluded.cap_by_realm,
                offline_pause_wall = excluded.offline_pause_wall,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                username,
                lifespan.born_at_tick,
                lifespan.years_lived.min(lifespan.cap_by_realm as f64),
                lifespan.cap_by_realm,
                offline_pause_wall,
                PLAYER_ROW_SCHEMA_VERSION,
                last_updated_wall
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

fn load_player_skill_set_from_sqlite(
    connection: &Connection,
    username: &str,
) -> io::Result<SkillSet> {
    let skill_set_json: Option<String> = connection
        .query_row(
            "
            SELECT skill_set_json
            FROM player_skills
            WHERE username = ?1
            ",
            params![username],
            |row| row.get(0),
        )
        .optional()
        .map_err(io::Error::other)?;

    let Some(skill_set_json) = skill_set_json else {
        return Ok(SkillSet::default());
    };

    serde_json::from_str::<SkillSet>(&skill_set_json)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn persist_player_core_slice_in_sqlite(
    connection: &mut Connection,
    username: &str,
    state: &PlayerState,
) -> io::Result<()> {
    let normalized = state.normalized();
    let last_updated_wall = current_unix_seconds();
    let updated = connection
        .execute(
            "
            UPDATE player_core
            SET karma = ?2,
                inventory_score = ?3,
                schema_version = ?4,
                last_updated_wall = ?5
            WHERE username = ?1
            ",
            params![
                username,
                normalized.karma,
                normalized.inventory_score,
                PLAYER_ROW_SCHEMA_VERSION,
                last_updated_wall
            ],
        )
        .map_err(io::Error::other)?;

    if updated == 0 {
        persist_player_slices_in_sqlite(
            connection,
            username,
            state,
            crate::player::spawn_position(),
            DimensionKind::default(),
            None,
            None,
            &SkillSet::default(),
        )?;
    }

    Ok(())
}

fn persist_player_slow_slice_in_sqlite(
    connection: &mut Connection,
    username: &str,
    position: [f64; 3],
    last_dimension: DimensionKind,
) -> io::Result<()> {
    let [pos_x, pos_y, pos_z] = position;
    let last_updated_wall = current_unix_seconds();
    let prefs_json = default_ui_prefs_json()?;

    connection
        .execute(
            "
            INSERT INTO player_slow (
                username,
                pos_x,
                pos_y,
                pos_z,
                last_dimension,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(username) DO UPDATE SET
                pos_x = excluded.pos_x,
                pos_y = excluded.pos_y,
                pos_z = excluded.pos_z,
                last_dimension = excluded.last_dimension,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                username,
                pos_x,
                pos_y,
                pos_z,
                dimension_kind_to_sql(last_dimension),
                PLAYER_ROW_SCHEMA_VERSION,
                last_updated_wall
            ],
        )
        .map_err(io::Error::other)?;
    connection
        .execute(
            "
            INSERT OR IGNORE INTO player_ui_prefs (
                username,
                prefs_json,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4)
            ",
            params![
                username,
                prefs_json,
                PLAYER_ROW_SCHEMA_VERSION,
                last_updated_wall
            ],
        )
        .map_err(io::Error::other)?;
    connection
        .execute(
            "
            UPDATE player_ui_prefs
            SET schema_version = ?2,
                last_updated_wall = ?3
            WHERE username = ?1
            ",
            params![username, PLAYER_ROW_SCHEMA_VERSION, last_updated_wall],
        )
        .map_err(io::Error::other)?;

    Ok(())
}

fn persist_player_inventory_slice_in_sqlite(
    connection: &mut Connection,
    username: &str,
    inventory: Option<&PlayerInventory>,
) -> io::Result<()> {
    let inventory_json = serialize_inventory_json(inventory)?;
    let last_updated_wall = current_unix_seconds();

    connection
        .execute(
            "
            INSERT INTO inventories (
                username,
                inventory_json,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(username) DO UPDATE SET
                inventory_json = excluded.inventory_json,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                username,
                inventory_json,
                PLAYER_ROW_SCHEMA_VERSION,
                last_updated_wall
            ],
        )
        .map_err(io::Error::other)?;

    Ok(())
}

fn persist_player_skill_slice_in_sqlite(
    connection: &mut Connection,
    username: &str,
    skill_set: &SkillSet,
) -> io::Result<()> {
    let skill_set_json = serialize_skill_set_json(skill_set)?;
    let last_updated_wall = current_unix_seconds();

    connection
        .execute(
            "
            INSERT INTO player_skills (
                username,
                skill_set_json,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(username) DO UPDATE SET
                skill_set_json = excluded.skill_set_json,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                username,
                skill_set_json,
                PLAYER_ROW_SCHEMA_VERSION,
                last_updated_wall
            ],
        )
        .map_err(io::Error::other)?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn persist_player_slices_in_sqlite(
    connection: &mut Connection,
    username: &str,
    state: &PlayerState,
    position: [f64; 3],
    last_dimension: DimensionKind,
    inventory: Option<&PlayerInventory>,
    lifespan: Option<&LifespanComponent>,
    skill_set: &SkillSet,
) -> io::Result<()> {
    let normalized = state.normalized();
    let karma = normalized.karma;
    let inventory_score = normalized.inventory_score;
    let [pos_x, pos_y, pos_z] = position;
    let inventory_json = serialize_inventory_json(inventory)?;
    let skill_set_json = serialize_skill_set_json(skill_set)?;
    let last_updated_wall = current_unix_seconds();
    let prefs_json = default_ui_prefs_json()?;

    let transaction = connection.transaction().map_err(io::Error::other)?;
    let current_char_id: Option<String> = transaction
        .query_row(
            "SELECT current_char_id FROM player_core WHERE username = ?1",
            params![username],
            |row| row.get(0),
        )
        .optional()
        .map_err(io::Error::other)?;
    let current_char_id = current_char_id.unwrap_or_else(|| Uuid::now_v7().to_string());

    transaction
        .execute(
            "
            INSERT INTO player_core (
                username,
                current_char_id,
                karma,
                inventory_score,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(username) DO UPDATE SET
                current_char_id = excluded.current_char_id,
                karma = excluded.karma,
                inventory_score = excluded.inventory_score,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                username,
                current_char_id,
                karma,
                inventory_score,
                PLAYER_ROW_SCHEMA_VERSION,
                last_updated_wall
            ],
        )
        .map_err(io::Error::other)?;

    transaction
        .execute(
            "
            INSERT INTO player_slow (
                username,
                pos_x,
                pos_y,
                pos_z,
                last_dimension,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(username) DO UPDATE SET
                pos_x = excluded.pos_x,
                pos_y = excluded.pos_y,
                pos_z = excluded.pos_z,
                last_dimension = excluded.last_dimension,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                username,
                pos_x,
                pos_y,
                pos_z,
                dimension_kind_to_sql(last_dimension),
                PLAYER_ROW_SCHEMA_VERSION,
                last_updated_wall
            ],
        )
        .map_err(io::Error::other)?;
    transaction
        .execute(
            "
            INSERT INTO inventories (
                username,
                inventory_json,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(username) DO UPDATE SET
                inventory_json = excluded.inventory_json,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                username,
                inventory_json,
                PLAYER_ROW_SCHEMA_VERSION,
                last_updated_wall
            ],
        )
        .map_err(io::Error::other)?;
    transaction
        .execute(
            "
            INSERT INTO player_skills (
                username,
                skill_set_json,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(username) DO UPDATE SET
                skill_set_json = excluded.skill_set_json,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                username,
                skill_set_json,
                PLAYER_ROW_SCHEMA_VERSION,
                last_updated_wall
            ],
        )
        .map_err(io::Error::other)?;
    transaction
        .execute(
            "
            INSERT OR IGNORE INTO player_ui_prefs (
                username,
                prefs_json,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4)
            ",
            params![
                username,
                prefs_json,
                PLAYER_ROW_SCHEMA_VERSION,
                last_updated_wall
            ],
        )
        .map_err(io::Error::other)?;
    if let Some(lifespan) = lifespan {
        let offline_pause_wall = last_updated_wall;
        transaction
            .execute(
                "
                INSERT INTO player_lifespan (
                    username,
                    born_at_tick,
                    years_lived,
                    cap_by_realm,
                    offline_pause_wall,
                    schema_version,
                    last_updated_wall
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                ON CONFLICT(username) DO UPDATE SET
                    born_at_tick = excluded.born_at_tick,
                    years_lived = excluded.years_lived,
                    cap_by_realm = excluded.cap_by_realm,
                    offline_pause_wall = excluded.offline_pause_wall,
                    schema_version = excluded.schema_version,
                    last_updated_wall = excluded.last_updated_wall
                ",
                params![
                    username,
                    lifespan.born_at_tick,
                    lifespan.years_lived.min(lifespan.cap_by_realm as f64),
                    lifespan.cap_by_realm,
                    offline_pause_wall,
                    PLAYER_ROW_SCHEMA_VERSION,
                    last_updated_wall
                ],
            )
            .map_err(io::Error::other)?;
    }
    transaction.commit().map_err(io::Error::other)
}

fn ensure_player_auxiliary_rows(connection: &mut Connection, username: &str) -> io::Result<()> {
    let last_updated_wall = current_unix_seconds();
    let prefs_json = default_ui_prefs_json()?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    insert_default_player_slice_rows(&transaction, username, last_updated_wall, &prefs_json)
        .map_err(io::Error::other)?;
    transaction.commit().map_err(io::Error::other)
}

fn insert_default_player_slice_rows(
    transaction: &rusqlite::Transaction<'_>,
    username: &str,
    last_updated_wall: i64,
    prefs_json: &str,
) -> rusqlite::Result<()> {
    let [pos_x, pos_y, pos_z] = crate::player::spawn_position();
    let skill_set_json = serialize_skill_set_json(&SkillSet::default())
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

    transaction.execute(
        "
        INSERT OR IGNORE INTO player_slow (
            username,
            pos_x,
            pos_y,
            pos_z,
            last_dimension,
            schema_version,
            last_updated_wall
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ",
        params![
            username,
            pos_x,
            pos_y,
            pos_z,
            dimension_kind_to_sql(DimensionKind::default()),
            PLAYER_ROW_SCHEMA_VERSION,
            last_updated_wall
        ],
    )?;
    transaction.execute(
        "
        INSERT OR IGNORE INTO inventories (
            username,
            inventory_json,
            schema_version,
            last_updated_wall
        ) VALUES (?1, ?2, ?3, ?4)
        ",
        params![
            username,
            DEFAULT_INVENTORY_JSON,
            PLAYER_ROW_SCHEMA_VERSION,
            last_updated_wall
        ],
    )?;
    transaction.execute(
        "
        INSERT OR IGNORE INTO player_skills (
            username,
            skill_set_json,
            schema_version,
            last_updated_wall
        ) VALUES (?1, ?2, ?3, ?4)
        ",
        params![
            username,
            skill_set_json,
            PLAYER_ROW_SCHEMA_VERSION,
            last_updated_wall
        ],
    )?;
    transaction.execute(
        "
        INSERT OR IGNORE INTO player_ui_prefs (
            username,
            prefs_json,
            schema_version,
            last_updated_wall
        ) VALUES (?1, ?2, ?3, ?4)
        ",
        params![
            username,
            prefs_json,
            PLAYER_ROW_SCHEMA_VERSION,
            last_updated_wall
        ],
    )?;

    Ok(())
}

fn migrate_legacy_player_json_to_sqlite(
    persistence: &PlayerStatePersistence,
    connection: &mut Connection,
    username: &str,
) -> io::Result<Option<PlayerState>> {
    let path = persistence.path_for_username(username);
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };

    let state = serde_json::from_str::<PlayerState>(&contents)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?
        .normalized();
    persist_player_slices_in_sqlite(
        connection,
        username,
        &state,
        crate::player::spawn_position(),
        DimensionKind::default(),
        None,
        None,
        &SkillSet::default(),
    )?;
    fs::rename(&path, persistence.migrated_path_for_username(username))?;
    Ok(Some(state))
}

fn default_ui_prefs_json() -> io::Result<String> {
    serde_json::to_string(&PlayerUiPrefs::default())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn serialize_inventory_json(inventory: Option<&PlayerInventory>) -> io::Result<String> {
    match inventory {
        Some(inventory) => serde_json::to_string(inventory)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error)),
        None => Ok(DEFAULT_INVENTORY_JSON.to_string()),
    }
}

fn serialize_skill_set_json(skill_set: &SkillSet) -> io::Result<String> {
    serde_json::to_string(skill_set)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn dimension_kind_to_sql(kind: DimensionKind) -> &'static str {
    match kind {
        DimensionKind::Overworld => "overworld",
        DimensionKind::Tsy => "tsy",
    }
}

fn dimension_kind_from_sql(value: &str) -> io::Result<DimensionKind> {
    match value {
        "overworld" => Ok(DimensionKind::Overworld),
        "tsy" => Ok(DimensionKind::Tsy),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unknown dimension kind `{other}`"),
        )),
    }
}
fn current_unix_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_secs() as i64
}

fn ratio_score(value: f64, max: f64) -> f64 {
    if max <= 0.0 {
        0.0
    } else {
        (value / max).clamp(0.0, 1.0)
    }
}

fn clamp_unit(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

fn realm_progress_score(realm: Realm) -> f64 {
    // Realm score is only used for coarse power estimation in player/world snapshots.
    // Keep the mapping stable and monotonic across the six realms.
    match realm {
        Realm::Awaken => 0.05,
        Realm::Induce => 0.25,
        Realm::Condense => 0.4,
        Realm::Solidify => 0.55,
        Realm::Spirit => 0.75,
        Realm::Void => 1.0,
    }
}

#[cfg(test)]
mod player_state_tests {
    use super::*;
    use crate::combat::components::TICKS_PER_SECOND;
    use crate::cultivation::lifespan::LifespanCapTable;
    use crate::network::agent_bridge::serialize_server_data_payload;
    use crate::persistence::bootstrap_sqlite;
    use crate::schema::server_data::{ServerDataPayloadV1, SERVER_DATA_VERSION};
    use rusqlite::{params, Connection};
    use std::path::PathBuf;
    use std::sync::{Arc, Barrier};
    use std::time::{SystemTime, UNIX_EPOCH};
    use uuid::Uuid;

    fn unique_temp_dir(test_name: &str) -> PathBuf {
        let unique_suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();

        std::env::temp_dir().join(format!(
            "bong-player-state-{test_name}-{}-{unique_suffix}",
            std::process::id()
        ))
    }

    fn approx_eq(left: f64, right: f64) {
        assert!(
            (left - right).abs() < 1e-9,
            "expected {left} to be approximately equal to {right}"
        );
    }

    fn sqlite_persistence(test_name: &str) -> (PlayerStatePersistence, PathBuf) {
        let data_dir = unique_temp_dir(test_name);
        let db_path = data_dir.join("bong.db");
        bootstrap_sqlite(&db_path, &format!("player-state-{test_name}"))
            .expect("sqlite bootstrap should succeed");
        (
            PlayerStatePersistence::with_db_path(&data_dir, &db_path),
            data_dir,
        )
    }

    #[test]
    fn loads_and_saves_player_state_in_sqlite() {
        let (persistence, data_dir) = sqlite_persistence("sqlite-load-save");
        let autosave_interval_ticks = 60 * TICKS_PER_SECOND;

        let persisted = PlayerState {
            karma: 0.2,
            inventory_score: 0.4,
        };

        let save_path = save_player_state(&persistence, "Azure", &persisted)
            .expect("saving PlayerState should succeed");
        let reloaded = load_player_state(&persistence, "Azure");
        let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
        let current_char_id: String = connection
            .query_row(
                "SELECT current_char_id FROM player_core WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .expect("player_core row should exist");
        let (pos_x, pos_y, pos_z): (f64, f64, f64) = connection
            .query_row(
                "SELECT pos_x, pos_y, pos_z FROM player_slow WHERE username = ?1",
                params!["Azure"],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("player_slow row should exist");
        let inventory_json: String = connection
            .query_row(
                "SELECT inventory_json FROM inventories WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .expect("inventories row should exist");
        let prefs_json: String = connection
            .query_row(
                "SELECT prefs_json FROM player_ui_prefs WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .expect("player_ui_prefs row should exist");
        let prefs: PlayerUiPrefs =
            serde_json::from_str(&prefs_json).expect("prefs_json should decode");
        let current_char_uuid =
            Uuid::parse_str(&current_char_id).expect("current_char_id should be a UUID");
        let [spawn_x, spawn_y, spawn_z] = crate::player::spawn_position();

        assert_eq!(save_path, persistence.db_path().to_path_buf());
        assert_eq!(reloaded, persisted.normalized());
        assert_eq!(autosave_interval_ticks, 1_200);
        assert_eq!(current_char_uuid.get_version_num(), 7);
        assert_eq!((pos_x, pos_y, pos_z), (spawn_x, spawn_y, spawn_z));
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&inventory_json)
                .expect("inventory_json should decode"),
            serde_json::Value::Null
        );
        assert_eq!(prefs, PlayerUiPrefs::default());

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn player_export_bundle_roundtrips_back_into_sqlite() {
        let (source_persistence, source_data_dir) = sqlite_persistence("export-bundle-source");
        let exported_state = PlayerState {
            karma: 0.25,
            inventory_score: 0.7,
        };
        save_player_slices(
            &source_persistence,
            "Azure",
            &exported_state,
            [64.0, 80.0, -12.0],
            DimensionKind::Tsy,
            None,
            None,
            &SkillSet::default(),
        )
        .expect("source player slices should persist");

        let bundle = export_player_bundle(&source_persistence, "Azure")
            .expect("player export bundle should load");

        let (target_persistence, target_data_dir) = sqlite_persistence("export-bundle-target");
        import_player_bundle(&target_persistence, &bundle)
            .expect("player export bundle should import");

        let connection =
            Connection::open(target_persistence.db_path()).expect("sqlite db should open");
        let current_char_id: String = connection
            .query_row(
                "SELECT current_char_id FROM player_core WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .expect("player_core row should exist after import");
        let (karma, inventory_score): (f64, f64) = connection
            .query_row(
                "
                SELECT karma, inventory_score
                FROM player_core
                WHERE username = ?1
                ",
                params!["Azure"],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("player_core payload should exist after import");
        let (pos_x, pos_y, pos_z, last_dimension_text): (f64, f64, f64, String) = connection
            .query_row(
                "SELECT pos_x, pos_y, pos_z, last_dimension FROM player_slow WHERE username = ?1",
                params!["Azure"],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("player_slow row should exist after import");
        let inventory_json: String = connection
            .query_row(
                "SELECT inventory_json FROM inventories WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .expect("inventories row should exist after import");
        let prefs_json: String = connection
            .query_row(
                "SELECT prefs_json FROM player_ui_prefs WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .expect("player_ui_prefs row should exist after import");

        assert_eq!(bundle.kind, "player_export_v1");
        assert_eq!(current_char_id, bundle.current_char_id);
        assert_eq!(karma, 0.25);
        assert_eq!(inventory_score, 0.7);
        assert_eq!((pos_x, pos_y, pos_z), (64.0, 80.0, -12.0));
        assert_eq!(last_dimension_text, "tsy");
        assert_eq!(bundle.last_dimension, DimensionKind::Tsy);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&inventory_json)
                .expect("inventory_json should decode"),
            serde_json::Value::Null
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&prefs_json)
                .expect("prefs_json should decode"),
            bundle.ui_prefs
        );

        let _ = fs::remove_dir_all(&source_data_dir);
        let _ = fs::remove_dir_all(&target_data_dir);
    }

    #[test]
    fn player_lifespan_slice_roundtrips_with_offline_pause_wall() {
        let (persistence, data_dir) = sqlite_persistence("lifespan-roundtrip");
        let player_state = PlayerState::default();
        let lifespan = LifespanComponent {
            born_at_tick: 144,
            years_lived: 12.5,
            cap_by_realm: LifespanCapTable::CONDENSE,
            offline_pause_tick: Some(120),
        };

        save_player_slices(
            &persistence,
            "Azure",
            &player_state,
            [11.0, 70.0, -2.0],
            DimensionKind::default(),
            None,
            Some(&lifespan),
            &SkillSet::default(),
        )
        .expect("lifespan slice should persist with player slices");

        let loaded = load_player_slices(&persistence, "Azure");
        let loaded_lifespan = loaded.lifespan.expect("lifespan should reload");
        let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
        let offline_pause_wall: i64 = connection
            .query_row(
                "SELECT offline_pause_wall FROM player_lifespan WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .expect("player_lifespan row should exist");

        assert_eq!(loaded_lifespan.born_at_tick, lifespan.born_at_tick);
        assert_eq!(loaded_lifespan.cap_by_realm, lifespan.cap_by_realm);
        assert!(loaded_lifespan.years_lived >= lifespan.years_lived);
        assert!(loaded_lifespan.years_lived < lifespan.years_lived + 0.01);
        assert!(offline_pause_wall > 0);

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn player_lifespan_load_applies_offline_delta_from_pause_wall() {
        let (persistence, data_dir) = sqlite_persistence("lifespan-offline-delta");
        save_player_state(&persistence, "Azure", &PlayerState::default())
            .expect("baseline player state should persist");

        let offline_pause_wall = current_unix_seconds()
            - (crate::cultivation::lifespan::LIFESPAN_SECONDS_PER_YEAR as i64 * 10);
        let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
        connection
            .execute(
                "
                INSERT INTO player_lifespan (
                    username,
                    born_at_tick,
                    years_lived,
                    cap_by_realm,
                    offline_pause_wall,
                    schema_version,
                    last_updated_wall
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                ",
                params![
                    "Azure",
                    0_u64,
                    6.0_f64,
                    LifespanCapTable::AWAKEN,
                    offline_pause_wall,
                    PLAYER_ROW_SCHEMA_VERSION,
                    offline_pause_wall,
                ],
            )
            .expect("lifespan fixture should insert");

        let loaded = load_player_slices(&persistence, "Azure");
        let loaded_lifespan = loaded.lifespan.expect("lifespan should reload");

        assert!(
            (6.99..=7.01).contains(&loaded_lifespan.years_lived),
            "expected ten offline real hours at x0.1 to add about one year, got {}",
            loaded_lifespan.years_lived
        );

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn player_lifespan_load_treats_zero_pause_wall_as_no_offline_delta() {
        let (persistence, data_dir) = sqlite_persistence("lifespan-zero-pause-wall");
        save_player_state(&persistence, "Azure", &PlayerState::default())
            .expect("baseline player state should persist");

        let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
        connection
            .execute(
                "
                INSERT INTO player_lifespan (
                    username,
                    born_at_tick,
                    years_lived,
                    cap_by_realm,
                    offline_pause_wall,
                    schema_version,
                    last_updated_wall
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                ",
                params![
                    "Azure",
                    0_u64,
                    12.0_f64,
                    LifespanCapTable::AWAKEN,
                    0_i64,
                    PLAYER_ROW_SCHEMA_VERSION,
                    0_i64,
                ],
            )
            .expect("legacy zero-pause lifespan fixture should insert");

        let loaded = load_player_slices(&persistence, "Azure");
        let loaded_lifespan = loaded.lifespan.expect("lifespan should reload");

        assert_eq!(loaded_lifespan.years_lived, 12.0);

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn import_player_bundle_rejects_invalid_current_char_id() {
        let (persistence, data_dir) = sqlite_persistence("import-invalid-char-id");
        let bundle = PlayerExportBundle {
            kind: "player_export_v1".to_string(),
            username: "Azure".to_string(),
            current_char_id: "not-a-uuid".to_string(),
            state: PlayerState {
                karma: 0.25,
                inventory_score: 0.7,
            },
            position: [64.0, 80.0, -12.0],
            last_dimension: DimensionKind::default(),
            inventory: None,
            skill_set: SkillSet::default(),
            ui_prefs: serde_json::json!({
                "quick_slots": [null, null, null, null, null, null, null, null, null]
            }),
        };

        let error = import_player_bundle(&persistence, &bundle)
            .expect_err("invalid current_char_id should be rejected");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);

        let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
        let player_core_exists: Option<String> = connection
            .query_row(
                "SELECT username FROM player_core WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .optional()
            .expect("player_core query should succeed");
        let player_slow_exists: Option<String> = connection
            .query_row(
                "SELECT username FROM player_slow WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .optional()
            .expect("player_slow query should succeed");
        let inventories_exists: Option<String> = connection
            .query_row(
                "SELECT username FROM inventories WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .optional()
            .expect("inventories query should succeed");
        let prefs_exists: Option<String> = connection
            .query_row(
                "SELECT username FROM player_ui_prefs WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .optional()
            .expect("player_ui_prefs query should succeed");

        assert!(player_core_exists.is_none());
        assert!(player_slow_exists.is_none());
        assert!(inventories_exists.is_none());
        assert!(prefs_exists.is_none());

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn import_player_bundle_rejects_invalid_ui_prefs() {
        let (persistence, data_dir) = sqlite_persistence("import-invalid-ui-prefs");
        let bundle = PlayerExportBundle {
            kind: "player_export_v1".to_string(),
            username: "Azure".to_string(),
            current_char_id: Uuid::now_v7().to_string(),
            state: PlayerState {
                karma: 0.25,
                inventory_score: 0.7,
            },
            position: [64.0, 80.0, -12.0],
            last_dimension: DimensionKind::default(),
            inventory: None,
            skill_set: SkillSet::default(),
            ui_prefs: serde_json::json!({
                "quick_slots": [0, 1, 2]
            }),
        };

        let error = import_player_bundle(&persistence, &bundle)
            .expect_err("invalid ui_prefs should be rejected");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);

        let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
        let player_core_exists: Option<String> = connection
            .query_row(
                "SELECT username FROM player_core WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .optional()
            .expect("player_core query should succeed");
        let player_slow_exists: Option<String> = connection
            .query_row(
                "SELECT username FROM player_slow WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .optional()
            .expect("player_slow query should succeed");
        let inventories_exists: Option<String> = connection
            .query_row(
                "SELECT username FROM inventories WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .optional()
            .expect("inventories query should succeed");
        let prefs_exists: Option<String> = connection
            .query_row(
                "SELECT username FROM player_ui_prefs WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .optional()
            .expect("player_ui_prefs query should succeed");

        assert!(player_core_exists.is_none());
        assert!(player_slow_exists.is_none());
        assert!(inventories_exists.is_none());
        assert!(prefs_exists.is_none());

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn computes_composite_power() {
        let state = PlayerState {
            karma: 0.25,
            inventory_score: 0.4,
        };

        let cultivation = Cultivation {
            realm: Realm::Induce,
            qi_current: 60.0,
            qi_max: 100.0,
            ..Cultivation::default()
        };

        let breakdown = state.power_breakdown(&cultivation);
        approx_eq(breakdown.combat, 0.39);
        approx_eq(breakdown.wealth, 0.4);
        approx_eq(breakdown.social, 0.4);
        approx_eq(breakdown.karma, 0.25);
        approx_eq(breakdown.territory, 0.325);
        approx_eq(state.composite_power(&cultivation), 0.36225);
    }

    #[test]
    fn serializes_player_state_payload() {
        let state = PlayerState {
            karma: 0.2,
            inventory_score: 0.4,
        };

        let cultivation = Cultivation {
            realm: Realm::Induce,
            qi_current: 78.0,
            qi_max: 100.0,
            ..Cultivation::default()
        };

        let payload = state.server_payload(
            &cultivation,
            Some(canonical_player_id("Steve")),
            "blood_valley",
        );
        let bytes =
            serialize_server_data_payload(&payload).expect("PlayerState payload should serialize");
        let json: serde_json::Value =
            serde_json::from_slice(&bytes).expect("serialized payload should decode as JSON value");

        assert_eq!(json.get("v"), Some(&serde_json::json!(SERVER_DATA_VERSION)));
        assert_eq!(json.get("type"), Some(&serde_json::json!("player_state")));
        assert_eq!(
            json.get("player"),
            Some(&serde_json::json!("offline:Steve"))
        );
        assert_eq!(json.get("realm"), Some(&serde_json::json!("Induce")));
        assert_eq!(json.get("spirit_qi"), Some(&serde_json::json!(78.0)));
        assert_eq!(json.get("karma"), Some(&serde_json::json!(0.2)));
        assert_eq!(json.get("zone"), Some(&serde_json::json!("blood_valley")));

        match payload.payload {
            ServerDataPayloadV1::PlayerState {
                composite_power,
                breakdown,
                ..
            } => {
                approx_eq(composite_power, state.composite_power(&cultivation));
                approx_eq(breakdown.combat, state.power_breakdown(&cultivation).combat);
                approx_eq(breakdown.wealth, state.power_breakdown(&cultivation).wealth);
                approx_eq(breakdown.social, state.power_breakdown(&cultivation).social);
                approx_eq(breakdown.karma, state.power_breakdown(&cultivation).karma);
                approx_eq(
                    breakdown.territory,
                    state.power_breakdown(&cultivation).territory,
                );
            }
            other => panic!("expected PlayerState payload, got {other:?}"),
        }
    }

    #[test]
    fn migrate_legacy_player_json_to_sqlite_once() {
        let (persistence, data_dir) = sqlite_persistence("legacy-migrate");

        #[derive(serde::Serialize)]
        struct LegacyPlayerStateV0 {
            realm: String,
            spirit_qi: f64,
            spirit_qi_max: f64,
            karma: f64,
            experience: u64,
            inventory_score: f64,
        }

        let legacy_state = LegacyPlayerStateV0 {
            realm: "Induce".to_string(),
            spirit_qi: 78.0,
            spirit_qi_max: 100.0,
            karma: 0.2,
            experience: 1_200,
            inventory_score: 0.4,
        };
        let expected_state = PlayerState {
            karma: 0.2,
            inventory_score: 0.4,
        };
        let save_path = persistence.path_for_username("CorruptCultivator");
        let migrated_path = persistence.migrated_path_for_username("CorruptCultivator");

        fs::create_dir_all(persistence.data_dir()).expect("test data dir should be creatable");
        fs::write(
            &save_path,
            serde_json::to_vec_pretty(&legacy_state).expect("legacy state should serialize"),
        )
        .expect("legacy PlayerState fixture should be writable");

        let migrated = load_player_state(&persistence, "CorruptCultivator");
        let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
        let first_char_id: String = connection
            .query_row(
                "SELECT current_char_id FROM player_core WHERE username = ?1",
                params!["CorruptCultivator"],
                |row| row.get(0),
            )
            .expect("migrated player_core row should exist");
        let reloaded = load_player_state(&persistence, "CorruptCultivator");
        let second_char_id: String = connection
            .query_row(
                "SELECT current_char_id FROM player_core WHERE username = ?1",
                params!["CorruptCultivator"],
                |row| row.get(0),
            )
            .expect("reloaded player_core row should exist");

        assert_eq!(migrated, expected_state.normalized());
        assert_eq!(reloaded, expected_state.normalized());
        assert!(
            !save_path.exists(),
            "legacy json should be renamed after migration"
        );
        assert!(
            migrated_path.exists(),
            "migrated legacy json should be preserved"
        );
        assert_eq!(first_char_id, second_char_id);
        assert_eq!(
            Uuid::parse_str(&first_char_id)
                .expect("current_char_id should be a UUID")
                .get_version_num(),
            7
        );

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn corrupt_legacy_player_json_falls_back_without_affecting_other_players() {
        let (persistence, data_dir) = sqlite_persistence("corrupt-json-isolation");
        let corrupted_username = "CorruptCultivator";
        let healthy_username = "StableCultivator";
        let corrupted_path = persistence.path_for_username(corrupted_username);
        let corrupted_migrated_path = persistence.migrated_path_for_username(corrupted_username);
        let healthy_state = PlayerState {
            karma: -0.3,
            inventory_score: 0.55,
        };

        save_player_state(&persistence, healthy_username, &healthy_state)
            .expect("healthy player state should persist");

        fs::create_dir_all(persistence.data_dir()).expect("test data dir should be creatable");
        fs::write(&corrupted_path, br#"{"realm":"broken""#)
            .expect("corrupted legacy fixture should be writable");

        let corrupted_loaded = load_player_state(&persistence, corrupted_username);
        let healthy_loaded = load_player_state(&persistence, healthy_username);

        assert_eq!(corrupted_loaded, PlayerState::default());
        assert_eq!(healthy_loaded, healthy_state.normalized());
        assert!(
            corrupted_path.exists(),
            "corrupted legacy json should remain in place after failed migration"
        );
        assert!(
            !corrupted_migrated_path.exists(),
            "corrupted legacy json should not be marked as migrated"
        );

        let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
        let corrupted_row: Option<(f64, f64)> = connection
            .query_row(
                "
                SELECT karma, inventory_score
                FROM player_core
                WHERE username = ?1
                ",
                params![corrupted_username],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .expect("corrupted player_core row query should succeed");
        let healthy_row: (f64, f64) = connection
            .query_row(
                "
                SELECT karma, inventory_score
                FROM player_core
                WHERE username = ?1
                ",
                params![healthy_username],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("healthy player_core row should exist");

        assert_eq!(
            corrupted_row,
            Some((
                PlayerState::default().karma,
                PlayerState::default().inventory_score,
            ))
        );
        assert_eq!(
            healthy_row,
            (
                healthy_state.normalized().karma,
                healthy_state.normalized().inventory_score,
            )
        );

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn concurrent_player_core_slice_writers_serialize_under_sqlite_busy_timeout() {
        let (persistence, data_dir) = sqlite_persistence("core-slice-concurrency");
        let writer_count = 50usize;
        let baseline_state = PlayerState {
            karma: 0.1,
            inventory_score: 0.2,
        };

        for index in 0..writer_count {
            save_player_state(
                &persistence,
                format!("Player{index}").as_str(),
                &baseline_state,
            )
            .expect("baseline player state should persist");
        }

        let persistence = Arc::new(persistence);
        let barrier = Arc::new(Barrier::new(writer_count + 1));
        let handles = (0..writer_count)
            .map(|index| {
                let persistence = Arc::clone(&persistence);
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    let username = format!("Player{index}");
                    let updated_state = PlayerState {
                        karma: ((index as f64 / 25.0) - 1.0).clamp(-1.0, 1.0),
                        inventory_score: (index as f64 / writer_count as f64).clamp(0.0, 1.0),
                    };

                    barrier.wait();
                    save_player_core_slice(persistence.as_ref(), username.as_str(), &updated_state)
                })
            })
            .collect::<Vec<_>>();

        barrier.wait();
        let errors = handles
            .into_iter()
            .map(|handle| handle.join().expect("writer thread should not panic"))
            .filter_map(Result::err)
            .map(|error| error.to_string())
            .collect::<Vec<_>>();
        assert!(
            errors.is_empty(),
            "all concurrent player core slice writers should succeed: {errors:?}"
        );

        let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
        let row_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM player_core", [], |row| row.get(0))
            .expect("player_core row count should be readable");
        assert_eq!(row_count, writer_count as i64);

        for index in 0..writer_count {
            let username = format!("Player{index}");
            let (karma, inventory_score): (f64, f64) = connection
                .query_row(
                    "
                    SELECT karma, inventory_score
                    FROM player_core
                    WHERE username = ?1
                    ",
                    params![username.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("updated player_core row should exist");

            assert_eq!(karma, ((index as f64 / 25.0) - 1.0).clamp(-1.0, 1.0));
            assert_eq!(
                inventory_score,
                (index as f64 / writer_count as f64).clamp(0.0, 1.0)
            );
        }

        let _ = fs::remove_dir_all(&data_dir);
    }
}
