use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use valence::prelude::{App, Resource, Startup};

pub const DEFAULT_DATABASE_PATH: &str = "data/bong.db";
const CURRENT_USER_VERSION: i32 = 3;
const CURRENT_SCHEMA_VERSION: i32 = 1;

#[derive(Debug, Clone)]
pub struct PersistenceSettings {
    db_path: PathBuf,
    server_run_id: String,
}

impl Resource for PersistenceSettings {}

impl Default for PersistenceSettings {
    fn default() -> Self {
        Self {
            db_path: PathBuf::from(DEFAULT_DATABASE_PATH),
            server_run_id: Uuid::now_v7().to_string(),
        }
    }
}

impl PersistenceSettings {
    pub fn db_path(&self) -> &Path {
        self.db_path.as_path()
    }

    pub fn server_run_id(&self) -> &str {
        self.server_run_id.as_str()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct BootstrapPayload {
    id: String,
    schema_version: i32,
    note: String,
}

pub fn register(app: &mut App) {
    app.init_resource::<PersistenceSettings>()
        .add_systems(Startup, bootstrap_persistence_system);
}

fn bootstrap_persistence_system(settings: valence::prelude::Res<PersistenceSettings>) {
    if let Err(error) = bootstrap_sqlite(settings.db_path(), settings.server_run_id()) {
        panic!(
            "[bong][persistence] failed to bootstrap sqlite at {}: {error}",
            settings.db_path().display()
        );
    }
}

pub fn bootstrap_sqlite(db_path: &Path, server_run_id: &str) -> rusqlite::Result<()> {
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    }

    let mut connection = Connection::open(db_path)?;
    configure_connection(&connection)?;
    run_integrity_check(&connection)?;
    apply_migrations(&mut connection)?;
    record_bootstrap_event(&connection, server_run_id)?;
    Ok(())
}

fn configure_connection(connection: &Connection) -> rusqlite::Result<()> {
    let journal_mode: String =
        connection.query_row("PRAGMA journal_mode = WAL;", [], |row| row.get(0))?;
    if !journal_mode.eq_ignore_ascii_case("wal") {
        return Err(rusqlite::Error::ExecuteReturnedResults);
    }

    connection.execute_batch("PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;")?;
    Ok(())
}

fn run_integrity_check(connection: &Connection) -> rusqlite::Result<()> {
    let integrity: String =
        connection.query_row("PRAGMA integrity_check;", [], |row| row.get(0))?;
    if integrity != "ok" {
        return Err(rusqlite::Error::ExecuteReturnedResults);
    }
    Ok(())
}

fn apply_migrations(connection: &mut Connection) -> rusqlite::Result<()> {
    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;

    if current_version < 1 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS bootstrap_events (
                event_id TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                game_tick INTEGER NOT NULL CHECK (game_tick >= 0),
                wall_clock INTEGER NOT NULL CHECK (wall_clock >= 0),
                server_run_id TEXT NOT NULL,
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0),
                payload_json TEXT NOT NULL
            );
            PRAGMA user_version = 1;
            ",
        )?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 2 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE INDEX IF NOT EXISTS idx_bootstrap_events_wall_clock
            ON bootstrap_events (wall_clock, event_id);
            PRAGMA user_version = 2;
            ",
        )?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 3 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS player_core (
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
            CREATE TABLE IF NOT EXISTS player_slow (
                username TEXT PRIMARY KEY,
                pos_x REAL NOT NULL,
                pos_y REAL NOT NULL,
                pos_z REAL NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            CREATE TABLE IF NOT EXISTS inventories (
                username TEXT PRIMARY KEY,
                inventory_json TEXT NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            CREATE TABLE IF NOT EXISTS player_ui_prefs (
                username TEXT PRIMARY KEY,
                prefs_json TEXT NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            PRAGMA user_version = 3;
            ",
        )?;
        transaction.commit()?;
    }

    let final_version: i32 = connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if final_version != CURRENT_USER_VERSION {
        return Err(rusqlite::Error::ExecuteReturnedResults);
    }

    Ok(())
}

fn record_bootstrap_event(connection: &Connection, server_run_id: &str) -> rusqlite::Result<()> {
    let event_id = Uuid::now_v7().to_string();
    let wall_clock = current_unix_seconds();
    let payload = BootstrapPayload {
        id: event_id.clone(),
        schema_version: CURRENT_SCHEMA_VERSION,
        note: "sqlite bootstrap ready".to_string(),
    };
    let payload_json = serde_json::to_string(&payload)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;

    connection.execute(
        "
        INSERT OR IGNORE INTO bootstrap_events (
            event_id,
            kind,
            schema_version,
            game_tick,
            wall_clock,
            server_run_id,
            last_updated_wall,
            payload_json
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        ",
        params![
            event_id,
            "bootstrap_ready",
            CURRENT_SCHEMA_VERSION,
            0_i64,
            wall_clock,
            server_run_id,
            wall_clock,
            payload_json
        ],
    )?;

    Ok(())
}

fn current_unix_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_secs() as i64
}

#[cfg(test)]
mod persistence_tests {
    use super::*;
    use rusqlite::OptionalExtension;

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
        assert_eq!(user_version, CURRENT_USER_VERSION);

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
}
