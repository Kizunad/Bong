use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use valence::prelude::{App, Resource, Startup};

use crate::combat::components::Lifecycle;
use crate::cultivation::life_record::{BiographyEntry, LifeRecord};

pub const DEFAULT_DATABASE_PATH: &str = "data/bong.db";
const DEFAULT_DECEASED_PUBLIC_DIR: &str = "../library-web/public/deceased";
const CURRENT_USER_VERSION: i32 = 4;
const CURRENT_SCHEMA_VERSION: i32 = 1;
const EVENT_SCHEMA_VERSION: i32 = 1;
const EVENT_PAYLOAD_VERSION: i32 = 1;

#[derive(Debug, Clone)]
pub struct PersistenceSettings {
    db_path: PathBuf,
    deceased_public_dir: PathBuf,
    server_run_id: String,
}

impl Resource for PersistenceSettings {}

impl Default for PersistenceSettings {
    fn default() -> Self {
        Self {
            db_path: PathBuf::from(DEFAULT_DATABASE_PATH),
            deceased_public_dir: PathBuf::from(DEFAULT_DECEASED_PUBLIC_DIR),
            server_run_id: Uuid::now_v7().to_string(),
        }
    }
}

impl PersistenceSettings {
    #[cfg(test)]
    pub fn with_paths(
        db_path: impl Into<PathBuf>,
        deceased_public_dir: impl Into<PathBuf>,
        server_run_id: impl Into<String>,
    ) -> Self {
        Self {
            db_path: db_path.into(),
            deceased_public_dir: deceased_public_dir.into(),
            server_run_id: server_run_id.into(),
        }
    }

    pub fn db_path(&self) -> &Path {
        self.db_path.as_path()
    }

    pub fn deceased_public_dir(&self) -> &Path {
        self.deceased_public_dir.as_path()
    }

    pub fn server_run_id(&self) -> &str {
        self.server_run_id.as_str()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LifespanEventRecord {
    pub at_tick: u64,
    pub kind: String,
    pub delta_years: i64,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeceasedIndexEntry {
    pub char_id: String,
    pub died_at_tick: u64,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeceasedSnapshot {
    pub char_id: String,
    pub died_at_tick: u64,
    pub lifecycle: Lifecycle,
    pub life_record: LifeRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LifeEventPayload {
    biography_entry: BiographyEntry,
}

#[derive(Debug, Clone)]
struct StagedDeceasedExport {
    snapshot_path: PathBuf,
    index_path: PathBuf,
    previous_snapshot: Option<Vec<u8>>,
    previous_index: Option<Vec<u8>>,
    relative_snapshot_path: String,
}

impl StagedDeceasedExport {
    fn relative_snapshot_path(&self) -> &str {
        self.relative_snapshot_path.as_str()
    }

    fn rollback(&self) {
        rollback_file(&self.snapshot_path, self.previous_snapshot.as_deref());
        rollback_file(&self.index_path, self.previous_index.as_deref());
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

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 4 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS life_records (
                char_id TEXT PRIMARY KEY,
                life_record_json TEXT NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            CREATE TABLE IF NOT EXISTS life_events (
                event_id TEXT PRIMARY KEY,
                char_id TEXT NOT NULL,
                event_type TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                payload_version INTEGER NOT NULL CHECK (payload_version >= 1),
                game_tick INTEGER NOT NULL CHECK (game_tick >= 0),
                wall_clock INTEGER NOT NULL CHECK (wall_clock >= 0),
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1)
            );
            CREATE INDEX IF NOT EXISTS idx_life_events_char_tick
            ON life_events (char_id, game_tick, event_id);
            CREATE TABLE IF NOT EXISTS death_registry (
                char_id TEXT PRIMARY KEY,
                death_count INTEGER NOT NULL CHECK (death_count >= 0),
                last_death_tick INTEGER NOT NULL CHECK (last_death_tick >= 0),
                last_death_cause TEXT NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            CREATE TABLE IF NOT EXISTS lifespan_events (
                event_id TEXT PRIMARY KEY,
                char_id TEXT NOT NULL,
                event_type TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                payload_version INTEGER NOT NULL CHECK (payload_version >= 1),
                game_tick INTEGER NOT NULL CHECK (game_tick >= 0),
                wall_clock INTEGER NOT NULL CHECK (wall_clock >= 0),
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1)
            );
            CREATE INDEX IF NOT EXISTS idx_lifespan_events_char_tick
            ON lifespan_events (char_id, game_tick, event_id);
            CREATE TABLE IF NOT EXISTS deceased_snapshots (
                char_id TEXT PRIMARY KEY,
                snapshot_json TEXT NOT NULL,
                public_path TEXT,
                died_at_tick INTEGER NOT NULL CHECK (died_at_tick >= 0),
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            PRAGMA user_version = 4;
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

pub fn persist_near_death_transition(
    settings: &PersistenceSettings,
    lifecycle: &Lifecycle,
    life_record: &LifeRecord,
    cause: &str,
    lifespan_event: Option<&LifespanEventRecord>,
) -> io::Result<()> {
    let entry = latest_biography_entry(life_record)?;
    let wall_clock = current_unix_seconds();
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;

    upsert_life_record(&transaction, life_record, wall_clock)?;
    append_life_event(
        &transaction,
        life_record.character_id.as_str(),
        entry,
        wall_clock,
    )?;
    upsert_death_registry(
        &transaction,
        life_record.character_id.as_str(),
        lifecycle,
        cause,
        wall_clock,
    )?;
    if let Some(lifespan_event) = lifespan_event {
        append_lifespan_event(
            &transaction,
            life_record.character_id.as_str(),
            lifespan_event,
            wall_clock,
        )?;
    }

    transaction.commit().map_err(io::Error::other)
}

pub fn persist_revival_transition(
    settings: &PersistenceSettings,
    life_record: &LifeRecord,
) -> io::Result<()> {
    let entry = latest_biography_entry(life_record)?;
    let wall_clock = current_unix_seconds();
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;

    upsert_life_record(&transaction, life_record, wall_clock)?;
    append_life_event(
        &transaction,
        life_record.character_id.as_str(),
        entry,
        wall_clock,
    )?;

    transaction.commit().map_err(io::Error::other)
}

pub fn persist_termination_transition(
    settings: &PersistenceSettings,
    lifecycle: &Lifecycle,
    life_record: &LifeRecord,
) -> io::Result<()> {
    let entry = latest_biography_entry(life_record)?;
    let wall_clock = current_unix_seconds();
    let died_at_tick = biography_tick(entry);
    let snapshot = DeceasedSnapshot {
        char_id: life_record.character_id.clone(),
        died_at_tick,
        lifecycle: lifecycle.clone(),
        life_record: life_record.clone(),
    };
    let snapshot_json = serde_json::to_string_pretty(&snapshot)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let staged_export = if should_export_public_snapshot(life_record.character_id.as_str()) {
        Some(stage_public_deceased_export(
            settings,
            life_record.character_id.as_str(),
            snapshot_json.as_str(),
            died_at_tick,
        )?)
    } else {
        None
    };

    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    let persisted = (|| -> io::Result<()> {
        upsert_life_record(&transaction, life_record, wall_clock)?;
        append_life_event(
            &transaction,
            life_record.character_id.as_str(),
            entry,
            wall_clock,
        )?;
        upsert_deceased_snapshot(
            &transaction,
            life_record.character_id.as_str(),
            snapshot_json.as_str(),
            staged_export
                .as_ref()
                .map(|export| export.relative_snapshot_path().to_string()),
            died_at_tick,
            wall_clock,
        )?;

        transaction.commit().map_err(io::Error::other)
    })();

    if persisted.is_err() {
        if let Some(export) = staged_export.as_ref() {
            export.rollback();
        }
    }

    persisted
}

fn open_persistence_connection(settings: &PersistenceSettings) -> io::Result<Connection> {
    if let Some(parent) = settings.db_path().parent() {
        fs::create_dir_all(parent)?;
    }

    let connection = Connection::open(settings.db_path()).map_err(io::Error::other)?;
    configure_connection(&connection).map_err(io::Error::other)?;
    Ok(connection)
}

fn latest_biography_entry(life_record: &LifeRecord) -> io::Result<&BiographyEntry> {
    life_record.biography.last().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "life_record must contain at least one biography entry before persistence",
        )
    })
}

fn biography_event_type(entry: &BiographyEntry) -> &'static str {
    match entry {
        BiographyEntry::BreakthroughStarted { .. } => "breakthrough_started",
        BiographyEntry::BreakthroughSucceeded { .. } => "breakthrough_succeeded",
        BiographyEntry::BreakthroughFailed { .. } => "breakthrough_failed",
        BiographyEntry::MeridianOpened { .. } => "meridian_opened",
        BiographyEntry::MeridianClosed { .. } => "meridian_closed",
        BiographyEntry::ForgedRate { .. } => "forged_rate",
        BiographyEntry::ForgedCapacity { .. } => "forged_capacity",
        BiographyEntry::ColorShift { .. } => "color_shift",
        BiographyEntry::InsightTaken { .. } => "insight_taken",
        BiographyEntry::Rebirth { .. } => "rebirth",
        BiographyEntry::CombatHit { .. } => "combat_hit",
        BiographyEntry::NearDeath { .. } => "near_death",
        BiographyEntry::Terminated { .. } => "terminated",
    }
}

fn biography_tick(entry: &BiographyEntry) -> u64 {
    match entry {
        BiographyEntry::BreakthroughStarted { tick, .. }
        | BiographyEntry::BreakthroughSucceeded { tick, .. }
        | BiographyEntry::BreakthroughFailed { tick, .. }
        | BiographyEntry::MeridianOpened { tick, .. }
        | BiographyEntry::MeridianClosed { tick, .. }
        | BiographyEntry::ForgedRate { tick, .. }
        | BiographyEntry::ForgedCapacity { tick, .. }
        | BiographyEntry::ColorShift { tick, .. }
        | BiographyEntry::InsightTaken { tick, .. }
        | BiographyEntry::Rebirth { tick, .. }
        | BiographyEntry::CombatHit { tick, .. }
        | BiographyEntry::NearDeath { tick, .. }
        | BiographyEntry::Terminated { tick, .. } => *tick,
    }
}

fn upsert_life_record(
    transaction: &rusqlite::Transaction<'_>,
    life_record: &LifeRecord,
    wall_clock: i64,
) -> io::Result<()> {
    let life_record_json = serde_json::to_string(life_record)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    transaction
        .execute(
            "
            INSERT INTO life_records (
                char_id,
                life_record_json,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(char_id) DO UPDATE SET
                life_record_json = excluded.life_record_json,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                life_record.character_id,
                life_record_json,
                EVENT_SCHEMA_VERSION,
                wall_clock
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

fn append_life_event(
    transaction: &rusqlite::Transaction<'_>,
    char_id: &str,
    entry: &BiographyEntry,
    wall_clock: i64,
) -> io::Result<()> {
    let payload_json = serde_json::to_string(&LifeEventPayload {
        biography_entry: entry.clone(),
    })
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    transaction
        .execute(
            "
            INSERT INTO life_events (
                event_id,
                char_id,
                event_type,
                payload_json,
                payload_version,
                game_tick,
                wall_clock,
                schema_version
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ",
            params![
                Uuid::now_v7().to_string(),
                char_id,
                biography_event_type(entry),
                payload_json,
                EVENT_PAYLOAD_VERSION,
                tick_to_sql(biography_tick(entry))?,
                wall_clock,
                EVENT_SCHEMA_VERSION
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

fn upsert_death_registry(
    transaction: &rusqlite::Transaction<'_>,
    char_id: &str,
    lifecycle: &Lifecycle,
    cause: &str,
    wall_clock: i64,
) -> io::Result<()> {
    transaction
        .execute(
            "
            INSERT INTO death_registry (
                char_id,
                death_count,
                last_death_tick,
                last_death_cause,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(char_id) DO UPDATE SET
                death_count = excluded.death_count,
                last_death_tick = excluded.last_death_tick,
                last_death_cause = excluded.last_death_cause,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                char_id,
                i64::from(lifecycle.death_count),
                tick_to_sql(lifecycle.last_death_tick.unwrap_or_default())?,
                cause,
                EVENT_SCHEMA_VERSION,
                wall_clock
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

fn append_lifespan_event(
    transaction: &rusqlite::Transaction<'_>,
    char_id: &str,
    event: &LifespanEventRecord,
    wall_clock: i64,
) -> io::Result<()> {
    let payload_json = serde_json::to_string(event)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    transaction
        .execute(
            "
            INSERT INTO lifespan_events (
                event_id,
                char_id,
                event_type,
                payload_json,
                payload_version,
                game_tick,
                wall_clock,
                schema_version
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ",
            params![
                Uuid::now_v7().to_string(),
                char_id,
                event.kind,
                payload_json,
                EVENT_PAYLOAD_VERSION,
                tick_to_sql(event.at_tick)?,
                wall_clock,
                EVENT_SCHEMA_VERSION
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

fn upsert_deceased_snapshot(
    transaction: &rusqlite::Transaction<'_>,
    char_id: &str,
    snapshot_json: &str,
    public_path: Option<String>,
    died_at_tick: u64,
    wall_clock: i64,
) -> io::Result<()> {
    transaction
        .execute(
            "
            INSERT INTO deceased_snapshots (
                char_id,
                snapshot_json,
                public_path,
                died_at_tick,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(char_id) DO UPDATE SET
                snapshot_json = excluded.snapshot_json,
                public_path = excluded.public_path,
                died_at_tick = excluded.died_at_tick,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                char_id,
                snapshot_json,
                public_path,
                tick_to_sql(died_at_tick)?,
                EVENT_SCHEMA_VERSION,
                wall_clock
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

fn tick_to_sql(tick: u64) -> io::Result<i64> {
    i64::try_from(tick).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn should_export_public_snapshot(char_id: &str) -> bool {
    char_id.starts_with("offline:")
}

fn stage_public_deceased_export(
    settings: &PersistenceSettings,
    char_id: &str,
    snapshot_json: &str,
    died_at_tick: u64,
) -> io::Result<StagedDeceasedExport> {
    fs::create_dir_all(settings.deceased_public_dir())?;

    let snapshot_path = settings
        .deceased_public_dir()
        .join(format!("{char_id}.json"));
    let index_path = settings.deceased_public_dir().join("_index.json");
    let previous_snapshot = fs::read(&snapshot_path).ok();
    let previous_index = fs::read(&index_path).ok();
    fs::write(&snapshot_path, snapshot_json.as_bytes())?;

    let relative_snapshot_path = format!("deceased/{char_id}.json");
    let mut entries = read_deceased_index(&index_path)?;
    entries.retain(|entry| entry.char_id != char_id);
    entries.push(DeceasedIndexEntry {
        char_id: char_id.to_string(),
        died_at_tick,
        path: relative_snapshot_path.clone(),
    });
    entries.sort_by(|left, right| {
        left.died_at_tick
            .cmp(&right.died_at_tick)
            .then_with(|| left.char_id.cmp(&right.char_id))
    });
    let index_json = serde_json::to_string_pretty(&entries)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    fs::write(&index_path, index_json.as_bytes())?;

    Ok(StagedDeceasedExport {
        snapshot_path,
        index_path,
        previous_snapshot,
        previous_index,
        relative_snapshot_path,
    })
}

fn read_deceased_index(index_path: &Path) -> io::Result<Vec<DeceasedIndexEntry>> {
    match fs::read_to_string(index_path) {
        Ok(contents) => serde_json::from_str(&contents)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}

fn rollback_file(path: &Path, previous: Option<&[u8]>) {
    match previous {
        Some(contents) => {
            let _ = fs::write(path, contents);
        }
        None => {
            let _ = fs::remove_file(path);
        }
    }
}

#[cfg(test)]
mod persistence_tests {
    use super::*;
    use rusqlite::{params, OptionalExtension};

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

    fn persistence_settings(test_name: &str) -> (PersistenceSettings, PathBuf) {
        let root = unique_temp_dir(test_name);
        let db_path = root.join("data").join("bong.db");
        let deceased_dir = root.join("library-web").join("public").join("deceased");
        (
            PersistenceSettings::with_paths(&db_path, &deceased_dir, format!("task3-{test_name}")),
            root,
        )
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
    fn persistence_can_write_deceased_snapshot_and_public_index() {
        let (settings, root) = persistence_settings("deceased-export");
        bootstrap_sqlite(settings.db_path(), settings.server_run_id())
            .expect("bootstrap should succeed");

        let life_record = LifeRecord {
            character_id: "offline:Ancestor".to_string(),
            created_at: 11,
            biography: vec![BiographyEntry::Terminated {
                cause: "fortune_exhausted".to_string(),
                tick: 77,
            }],
            insights_taken: Vec::new(),
            spirit_root_first: None,
        };
        let lifecycle = Lifecycle {
            character_id: life_record.character_id.clone(),
            death_count: 3,
            fortune_remaining: 0,
            last_death_tick: Some(77),
            last_revive_tick: Some(55),
            near_death_deadline_tick: None,
            weakened_until_tick: None,
            state: crate::combat::components::LifecycleState::Terminated,
        };

        persist_termination_transition(&settings, &lifecycle, &life_record)
            .expect("terminated snapshot should persist");

        let snapshot_path = settings.deceased_public_dir().join("offline:Ancestor.json");
        let index_path = settings.deceased_public_dir().join("_index.json");
        let snapshot: DeceasedSnapshot = serde_json::from_str(
            &fs::read_to_string(&snapshot_path).expect("snapshot json should exist"),
        )
        .expect("snapshot json should deserialize");
        let index: Vec<DeceasedIndexEntry> = serde_json::from_str(
            &fs::read_to_string(&index_path).expect("index json should exist"),
        )
        .expect("index json should deserialize");
        let connection = Connection::open(settings.db_path()).expect("db should open");
        let public_path: String = connection
            .query_row(
                "SELECT public_path FROM deceased_snapshots WHERE char_id = ?1",
                params!["offline:Ancestor"],
                |row| row.get(0),
            )
            .expect("deceased snapshot row should exist");

        assert_eq!(snapshot.char_id, "offline:Ancestor");
        assert_eq!(snapshot.died_at_tick, 77);
        assert_eq!(snapshot.lifecycle.character_id, lifecycle.character_id);
        assert_eq!(snapshot.lifecycle.death_count, lifecycle.death_count);
        assert_eq!(
            snapshot.lifecycle.fortune_remaining,
            lifecycle.fortune_remaining
        );
        assert_eq!(
            snapshot.lifecycle.last_death_tick,
            lifecycle.last_death_tick
        );
        assert_eq!(
            snapshot.lifecycle.last_revive_tick,
            lifecycle.last_revive_tick
        );
        assert_eq!(snapshot.lifecycle.state, lifecycle.state);
        assert_eq!(snapshot.life_record.character_id, life_record.character_id);
        assert_eq!(snapshot.life_record.created_at, life_record.created_at);
        assert_eq!(
            snapshot.life_record.biography.len(),
            life_record.biography.len()
        );
        assert!(matches!(
            snapshot.life_record.biography.last(),
            Some(BiographyEntry::Terminated { tick: 77, .. })
        ));
        assert_eq!(index.len(), 1);
        assert_eq!(index[0].char_id, "offline:Ancestor");
        assert_eq!(index[0].died_at_tick, 77);
        assert_eq!(index[0].path, "deceased/offline:Ancestor.json");
        assert_eq!(public_path, "deceased/offline:Ancestor.json");

        let _ = fs::remove_dir_all(root);
    }
}
