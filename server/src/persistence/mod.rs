use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use big_brain::prelude::{ActionState, Actor};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use valence::prelude::bevy_ecs;
use valence::prelude::{
    App, Client, Commands, Component, DVec3, Entity, EntityKind, Position, Query, Res, ResMut,
    Resource, Startup, Update, Username, With,
};

use crate::combat::components::{Lifecycle, LifecycleState};
use crate::cultivation::components::Cultivation;
use crate::cultivation::life_record::{BiographyEntry, LifeRecord};
use crate::npc::brain::{canonical_npc_id, ChaseAction, DashAction, FleeAction, MeleeAttackAction};
use crate::npc::movement::{MovementController, MovementCooldowns, MovementMode};
use crate::npc::patrol::NpcPatrol;
use crate::npc::spawn::{NpcBlackboard, NpcCombatLoadout, NpcMarker, NpcMeleeArchetype};
use crate::player::state::canonical_player_id;
use crate::schema::common::NpcStateKind;

pub const DEFAULT_DATABASE_PATH: &str = "data/bong.db";
const DEFAULT_DECEASED_PUBLIC_DIR: &str = "../library-web/public/deceased";
const CURRENT_USER_VERSION: i32 = 10;
const AGENT_WORLD_MODEL_ROW_ID: i64 = 1;
const ASCENSION_QUOTA_ROW_ID: i64 = 1;
pub const WORLD_MODEL_STATE_KEY: &str = "bong:tiandao:state";
pub const WORLD_MODEL_STATE_FIELD_CURRENT_ERA: &str = "current_era";
pub const WORLD_MODEL_STATE_FIELD_ZONE_HISTORY: &str = "zone_history";
pub const WORLD_MODEL_STATE_FIELD_LAST_DECISIONS: &str = "last_decisions";
pub const WORLD_MODEL_STATE_FIELD_PLAYER_FIRST_SEEN_TICK: &str = "player_first_seen_tick";
pub const WORLD_MODEL_STATE_FIELD_LAST_TICK: &str = "last_tick";
pub const WORLD_MODEL_STATE_FIELD_LAST_STATE_TS: &str = "last_state_ts";
const CURRENT_SCHEMA_VERSION: i32 = 1;
const EVENT_SCHEMA_VERSION: i32 = 1;
const EVENT_PAYLOAD_VERSION: i32 = 1;
const NPC_ROW_SCHEMA_VERSION: i32 = 1;
const NPC_DIGEST_RETENTION_SECS: i64 = 180 * 24 * 60 * 60;
const NPC_DIGEST_SWEEP_INTERVAL_SECS: i64 = 7 * 24 * 60 * 60;
const NPC_SNAPSHOT_INTERVAL_TICKS: u32 = 20 * 60;
const ZONE_RUNTIME_SNAPSHOT_INTERVAL_SECS: i64 = 5 * 60;
const STARTUP_BACKUP_DIR: &str = "data/backups";
const STARTUP_BACKUP_FILE_PREFIX: &str = "bong-";
const STARTUP_BACKUP_FILE_SUFFIX: &str = ".db";
const STARTUP_BACKUP_KEEP_COUNT: usize = 7;

#[derive(Debug, Clone)]
pub struct PersistenceSettings {
    db_path: PathBuf,
    deceased_public_dir: PathBuf,
    server_run_id: String,
}

impl Resource for PersistenceSettings {}

#[derive(Debug, Default)]
struct NpcSnapshotTracker {
    last_snapshot_tick: u32,
}

impl Resource for NpcSnapshotTracker {}

#[derive(Debug, Default)]
struct NpcDigestSweepState {
    last_sweep_wall: i64,
}

impl Resource for NpcDigestSweepState {}

#[derive(Debug, Default)]
struct DailyBackupState {
    last_backup_day: Option<i64>,
}

impl Resource for DailyBackupState {}

#[derive(Debug, Default)]
struct ZoneRuntimeSnapshotState {
    last_snapshot_wall: i64,
}

impl Resource for ZoneRuntimeSnapshotState {}

#[derive(Debug, Default, Component)]
struct NpcArchivedPersistence;

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NpcStateRecord {
    pub char_id: String,
    pub kind: String,
    pub pos: [f64; 3],
    pub state: String,
    pub blackboard: HashMap<String, serde_json::Value>,
    pub archetype: String,
    pub home_zone: String,
    pub patrol_anchor_index: usize,
    pub patrol_target: [f64; 3],
    pub movement_mode: String,
    pub can_sprint: bool,
    pub can_dash: bool,
    pub sprint_ready_at: u32,
    pub dash_ready_at: u32,
    pub lifecycle_state: String,
    pub death_count: u32,
    pub last_death_tick: Option<u64>,
    pub last_revive_tick: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NpcDigestRecord {
    pub char_id: String,
    pub archetype: String,
    pub realm: String,
    pub faction_id: Option<String>,
    pub recent_summary: String,
    pub last_referenced_wall: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArchetypeRegistryEntry {
    pub char_id: String,
    pub archetype: String,
    pub since_tick: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentWorldModelCommandRecord {
    #[serde(rename = "type")]
    pub command_type: String,
    pub target: String,
    #[serde(default)]
    pub params: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentWorldModelNarrationRecord {
    pub scope: String,
    #[serde(default)]
    pub target: Option<String>,
    pub text: String,
    pub style: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentWorldModelDecisionRecord {
    #[serde(default)]
    pub commands: Vec<AgentWorldModelCommandRecord>,
    #[serde(default)]
    pub narrations: Vec<AgentWorldModelNarrationRecord>,
    pub reasoning: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentWorldModelSnapshotRecord {
    #[serde(default)]
    pub current_era: Option<serde_json::Value>,
    #[serde(default)]
    pub zone_history: BTreeMap<String, Vec<serde_json::Value>>,
    #[serde(default)]
    pub last_decisions: BTreeMap<String, AgentWorldModelDecisionRecord>,
    #[serde(default)]
    pub player_first_seen_tick: BTreeMap<String, i64>,
    #[serde(default)]
    pub last_tick: Option<i64>,
    #[serde(default)]
    pub last_state_ts: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActiveTribulationRecord {
    pub char_id: String,
    pub wave_current: u32,
    pub waves_total: u32,
    pub started_tick: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AscensionQuotaRecord {
    pub occupied_slots: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ZoneRuntimeRecord {
    pub zone_id: String,
    pub spirit_qi: f64,
    pub danger_level: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ZoneOverlayRecord {
    pub zone_id: String,
    pub overlay_kind: String,
    pub payload_json: String,
    pub payload_version: i32,
    pub since_wall: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ZoneExportBundle {
    pub schema_version: i32,
    pub kind: String,
    pub zones_runtime: Vec<ZoneRuntimeRecord>,
    pub zone_overlays: Vec<ZoneOverlayRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NpcPersistenceCapture {
    pub state: NpcStateRecord,
    pub digest: NpcDigestRecord,
    pub archetype_entry: ArchetypeRegistryEntry,
    pub captured_at_wall: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NpcDeceasedIndexRecord {
    pub char_id: String,
    pub archetype: String,
    pub died_at_tick: u64,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpcDeceasedArchiveRecord {
    pub char_id: String,
    pub archetype: String,
    pub died_at_tick: u64,
    pub archived_at_wall: i64,
    pub lifecycle_state: String,
    pub death_count: u32,
    pub state: Option<NpcStateRecord>,
    pub digest: Option<NpcDigestRecord>,
    pub life_record: Option<LifeRecord>,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FactionRecord {
    pub faction_id: String,
    pub display_name: String,
    pub doctrine: String,
    pub metadata_json: String,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FactionReputationRecord {
    pub faction_id: String,
    pub target_faction_id: String,
    pub score: i32,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FactionMembershipRecord {
    pub faction_id: String,
    pub char_id: String,
    pub role: String,
    pub joined_at_tick: u64,
    pub metadata_json: String,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RelationshipRecord {
    pub char_id: String,
    pub peer_char_id: String,
    pub relationship_type: String,
    pub since_tick: u64,
    pub metadata_json: String,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct FactionSocialBundle {
    pub factions: Vec<FactionRecord>,
    pub reputations: Vec<FactionReputationRecord>,
    pub memberships: Vec<FactionMembershipRecord>,
    pub relationships: Vec<RelationshipRecord>,
}

pub fn register(app: &mut App) {
    app.init_resource::<PersistenceSettings>()
        .init_resource::<NpcSnapshotTracker>()
        .init_resource::<NpcDigestSweepState>()
        .init_resource::<DailyBackupState>()
        .init_resource::<ZoneRuntimeSnapshotState>()
        .add_systems(Startup, bootstrap_persistence_system)
        .add_systems(
            Update,
            (
                persist_npc_runtime_state_system,
                sweep_npc_digest_retention_system,
                daily_midnight_backup_system,
                persist_zone_runtime_system,
            ),
        );
}

fn bootstrap_persistence_system(
    settings: valence::prelude::Res<PersistenceSettings>,
    mut daily_backup_state: valence::prelude::ResMut<DailyBackupState>,
    mut zones: Option<ResMut<crate::world::zone::ZoneRegistry>>,
) {
    let wall_clock = current_unix_seconds();
    daily_backup_state.last_backup_day = Some(utc_day_from_unix_seconds(wall_clock));
    match run_startup_backup(&settings, wall_clock) {
        Ok(Some(path)) => tracing::info!(
            "[bong][persistence] created startup sqlite backup at {}",
            path.display()
        ),
        Ok(None) => {}
        Err(error) => tracing::warn!(
            "[bong][persistence] failed to create startup sqlite backup at {}: {error}",
            settings.db_path().display()
        ),
    }

    match prune_startup_backups(&settings, STARTUP_BACKUP_KEEP_COUNT) {
        Ok(pruned) if !pruned.is_empty() => tracing::info!(
            "[bong][persistence] pruned {} stale startup backup(s) under {}",
            pruned.len(),
            resolve_persistence_relative_path(&settings, STARTUP_BACKUP_DIR).display()
        ),
        Ok(_) => {}
        Err(error) => tracing::warn!(
            "[bong][persistence] failed to prune startup backups under {}: {error}",
            resolve_persistence_relative_path(&settings, STARTUP_BACKUP_DIR).display()
        ),
    }

    if let Err(error) = bootstrap_sqlite(settings.db_path(), settings.server_run_id()) {
        panic!(
            "[bong][persistence] failed to bootstrap sqlite at {}: {error}",
            settings.db_path().display()
        );
    }

    if let Err(error) = scan_orphaned_npc_archives(&settings) {
        tracing::warn!(
            "[bong][persistence] failed to scan orphaned npc archives at {}: {error}",
            settings.db_path().display()
        );
    }

    if let Some(zone_registry) = zones.as_deref_mut() {
        if let Err(error) = hydrate_zone_runtime(&settings, zone_registry) {
            tracing::warn!(
                "[bong][persistence] failed to hydrate zone runtime from sqlite at {}: {error}",
                settings.db_path().display()
            );
        }
        if let Err(error) = hydrate_zone_overlays(&settings, zone_registry) {
            tracing::warn!(
                "[bong][persistence] failed to hydrate zone overlays from sqlite at {}: {error}",
                settings.db_path().display()
            );
        }
    }
}

fn daily_midnight_backup_system(
    settings: Res<PersistenceSettings>,
    mut daily_backup_state: ResMut<DailyBackupState>,
) {
    let wall_clock = current_unix_seconds();
    match run_daily_backup_cycle(&settings, &mut daily_backup_state, wall_clock) {
        Ok(run) if !run.triggered => {}
        Ok(run) => {
            if let Some(path) = run.backup_path {
                tracing::info!(
                    "[bong][persistence] created daily sqlite backup at {}",
                    path.display()
                );
            }
            if !run.pruned_paths.is_empty() {
                tracing::info!(
                    "[bong][persistence] pruned {} stale daily backup(s) under {}",
                    run.pruned_paths.len(),
                    resolve_persistence_relative_path(&settings, STARTUP_BACKUP_DIR).display()
                );
            }
        }
        Err(error) => tracing::warn!(
            "[bong][persistence] daily backup cycle failed at {}: {error}",
            settings.db_path().display()
        ),
    }
}

fn persist_zone_runtime_system(
    settings: Res<PersistenceSettings>,
    mut snapshot_state: ResMut<ZoneRuntimeSnapshotState>,
    zones: Option<Res<crate::world::zone::ZoneRegistry>>,
) {
    let Some(zone_registry) = zones else {
        return;
    };

    let wall_clock = current_unix_seconds();
    if snapshot_state.last_snapshot_wall > 0
        && wall_clock.saturating_sub(snapshot_state.last_snapshot_wall)
            < ZONE_RUNTIME_SNAPSHOT_INTERVAL_SECS
    {
        return;
    }

    match persist_zone_runtime_snapshot(&settings, &zone_registry) {
        Ok(_) => {
            snapshot_state.last_snapshot_wall = wall_clock;
        }
        Err(error) => tracing::warn!(
            "[bong][persistence] failed to persist zone runtime snapshot at {}: {error}",
            settings.db_path().display()
        ),
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

pub fn bootstrap_agent_world_model_mirror(
    settings: &PersistenceSettings,
) -> io::Result<Option<AgentWorldModelSnapshotRecord>> {
    let snapshot = load_agent_world_model_snapshot(settings)?;
    Ok(snapshot)
}

pub fn world_model_snapshot_to_mirror_fields(
    snapshot: &AgentWorldModelSnapshotRecord,
) -> io::Result<BTreeMap<String, String>> {
    let mut fields = BTreeMap::new();
    fields.insert(
        WORLD_MODEL_STATE_FIELD_CURRENT_ERA.to_string(),
        serde_json::to_string(&snapshot.current_era)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
    );
    fields.insert(
        WORLD_MODEL_STATE_FIELD_ZONE_HISTORY.to_string(),
        serde_json::to_string(&snapshot.zone_history)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
    );
    fields.insert(
        WORLD_MODEL_STATE_FIELD_LAST_DECISIONS.to_string(),
        serde_json::to_string(&snapshot.last_decisions)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
    );
    fields.insert(
        WORLD_MODEL_STATE_FIELD_PLAYER_FIRST_SEEN_TICK.to_string(),
        serde_json::to_string(&snapshot.player_first_seen_tick)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
    );
    fields.insert(
        WORLD_MODEL_STATE_FIELD_LAST_TICK.to_string(),
        snapshot
            .last_tick
            .map(|value| value.to_string())
            .unwrap_or_default(),
    );
    fields.insert(
        WORLD_MODEL_STATE_FIELD_LAST_STATE_TS.to_string(),
        snapshot
            .last_state_ts
            .map(|value| value.to_string())
            .unwrap_or_default(),
    );
    Ok(fields)
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

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 5 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS npc_state (
                char_id TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                archetype TEXT NOT NULL,
                pos_x REAL NOT NULL,
                pos_y REAL NOT NULL,
                pos_z REAL NOT NULL,
                state TEXT NOT NULL,
                blackboard_json TEXT NOT NULL,
                home_zone TEXT NOT NULL,
                patrol_anchor_index INTEGER NOT NULL CHECK (patrol_anchor_index >= 0),
                patrol_target_x REAL NOT NULL,
                patrol_target_y REAL NOT NULL,
                patrol_target_z REAL NOT NULL,
                movement_mode TEXT NOT NULL,
                can_sprint INTEGER NOT NULL,
                can_dash INTEGER NOT NULL,
                sprint_ready_at INTEGER NOT NULL CHECK (sprint_ready_at >= 0),
                dash_ready_at INTEGER NOT NULL CHECK (dash_ready_at >= 0),
                lifecycle_state TEXT NOT NULL,
                death_count INTEGER NOT NULL CHECK (death_count >= 0),
                last_death_tick INTEGER,
                last_revive_tick INTEGER,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            CREATE TABLE IF NOT EXISTS npc_digests (
                char_id TEXT PRIMARY KEY,
                archetype TEXT NOT NULL,
                realm TEXT NOT NULL,
                faction_id TEXT,
                recent_summary TEXT NOT NULL,
                last_referenced_wall INTEGER NOT NULL CHECK (last_referenced_wall >= 0),
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            CREATE INDEX IF NOT EXISTS idx_npc_digests_last_referenced_wall
            ON npc_digests (last_referenced_wall, char_id);
            CREATE TABLE IF NOT EXISTS factions (
                faction_id TEXT PRIMARY KEY,
                display_name TEXT NOT NULL,
                doctrine TEXT NOT NULL,
                metadata_json TEXT NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            CREATE TABLE IF NOT EXISTS reputation (
                faction_id TEXT NOT NULL,
                target_faction_id TEXT NOT NULL,
                score INTEGER NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0),
                PRIMARY KEY (faction_id, target_faction_id)
            );
            CREATE TABLE IF NOT EXISTS membership (
                faction_id TEXT NOT NULL,
                char_id TEXT NOT NULL,
                role TEXT NOT NULL,
                joined_at_tick INTEGER NOT NULL CHECK (joined_at_tick >= 0),
                metadata_json TEXT NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0),
                PRIMARY KEY (faction_id, char_id)
            );
            CREATE TABLE IF NOT EXISTS relationships (
                char_id TEXT NOT NULL,
                peer_char_id TEXT NOT NULL,
                relationship_type TEXT NOT NULL,
                since_tick INTEGER NOT NULL CHECK (since_tick >= 0),
                metadata_json TEXT NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0),
                PRIMARY KEY (char_id, peer_char_id, relationship_type)
            );
            CREATE TABLE IF NOT EXISTS archetype_registry (
                char_id TEXT NOT NULL,
                archetype TEXT NOT NULL,
                since_tick INTEGER NOT NULL CHECK (since_tick >= 0),
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0),
                PRIMARY KEY (char_id, since_tick, archetype)
            );
            CREATE INDEX IF NOT EXISTS idx_archetype_registry_char_tick
            ON archetype_registry (char_id, since_tick, archetype);
            CREATE TABLE IF NOT EXISTS npc_deceased_index (
                char_id TEXT PRIMARY KEY,
                archetype TEXT NOT NULL,
                died_at_tick INTEGER NOT NULL CHECK (died_at_tick >= 0),
                path TEXT NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            PRAGMA user_version = 5;
            ",
        )?;
        transaction.commit()?;
    }

    ensure_agent_world_model_table(connection)?;

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 6 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS tribulations_active (
                char_id TEXT PRIMARY KEY,
                wave_current INTEGER NOT NULL CHECK (wave_current >= 0),
                waves_total INTEGER NOT NULL CHECK (waves_total > 0),
                started_tick INTEGER NOT NULL CHECK (started_tick >= 0),
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            PRAGMA user_version = 6;
            ",
        )?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 7 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS ascension_quota (
                row_id INTEGER PRIMARY KEY CHECK (row_id = 1),
                occupied_slots INTEGER NOT NULL CHECK (occupied_slots >= 0),
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            PRAGMA user_version = 7;
            ",
        )?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 8 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS zones_runtime (
                zone_id TEXT PRIMARY KEY,
                spirit_qi REAL NOT NULL,
                danger_level INTEGER NOT NULL CHECK (danger_level >= 0),
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            PRAGMA user_version = 8;
            ",
        )?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 9 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS zone_overlays (
                zone_id TEXT NOT NULL,
                overlay_kind TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                since_wall INTEGER NOT NULL CHECK (since_wall >= 0),
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0),
                PRIMARY KEY (zone_id, overlay_kind, since_wall)
            );
            PRAGMA user_version = 9;
            ",
        )?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 10 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            ALTER TABLE zone_overlays
            ADD COLUMN payload_version INTEGER NOT NULL DEFAULT 1 CHECK (payload_version >= 1);
            PRAGMA user_version = 10;
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

fn ensure_agent_world_model_table(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS agent_world_model (
            row_id INTEGER PRIMARY KEY CHECK (row_id = 1),
            snapshot_json TEXT NOT NULL,
            schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
            last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
        );
        ",
    )?;
    Ok(())
}

pub fn persist_agent_world_model_snapshot(
    settings: &PersistenceSettings,
    snapshot: &AgentWorldModelSnapshotRecord,
) -> io::Result<()> {
    let wall_clock = current_unix_seconds();
    let snapshot_json = serde_json::to_string(snapshot)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    transaction
        .execute(
            "
            INSERT INTO agent_world_model (
                row_id,
                snapshot_json,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(row_id) DO UPDATE SET
                snapshot_json = excluded.snapshot_json,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                AGENT_WORLD_MODEL_ROW_ID,
                snapshot_json,
                CURRENT_SCHEMA_VERSION,
                wall_clock
            ],
        )
        .map_err(io::Error::other)?;
    transaction.commit().map_err(io::Error::other)
}

pub fn load_agent_world_model_snapshot(
    settings: &PersistenceSettings,
) -> io::Result<Option<AgentWorldModelSnapshotRecord>> {
    let connection = open_persistence_connection(settings)?;
    let snapshot_json: Option<String> = connection
        .query_row(
            "SELECT snapshot_json FROM agent_world_model WHERE row_id = ?1",
            params![AGENT_WORLD_MODEL_ROW_ID],
            |row| row.get(0),
        )
        .optional()
        .map_err(io::Error::other)?;

    let Some(snapshot_json) = snapshot_json else {
        return Ok(None);
    };

    let snapshot = serde_json::from_str::<AgentWorldModelSnapshotRecord>(&snapshot_json)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    Ok(Some(snapshot))
}

pub fn persist_active_tribulation(
    settings: &PersistenceSettings,
    record: &ActiveTribulationRecord,
) -> io::Result<()> {
    let wall_clock = current_unix_seconds();
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    upsert_active_tribulation(&transaction, record, wall_clock)?;
    transaction.commit().map_err(io::Error::other)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn load_active_tribulation(
    settings: &PersistenceSettings,
    char_id: &str,
) -> io::Result<Option<ActiveTribulationRecord>> {
    let connection = open_persistence_connection(settings)?;
    load_active_tribulation_from_connection(&connection, char_id)
}

pub fn delete_active_tribulation(settings: &PersistenceSettings, char_id: &str) -> io::Result<()> {
    let connection = open_persistence_connection(settings)?;
    connection
        .execute(
            "DELETE FROM tribulations_active WHERE char_id = ?1",
            params![char_id],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn load_ascension_quota(settings: &PersistenceSettings) -> io::Result<AscensionQuotaRecord> {
    let connection = open_persistence_connection(settings)?;
    load_ascension_quota_from_connection(&connection)
}

pub fn complete_tribulation_ascension(
    settings: &PersistenceSettings,
    char_id: &str,
) -> io::Result<AscensionQuotaRecord> {
    let wall_clock = current_unix_seconds();
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    let mut quota = load_ascension_quota_from_transaction(&transaction)?;
    quota.occupied_slots = quota.occupied_slots.saturating_add(1);

    transaction
        .execute(
            "DELETE FROM tribulations_active WHERE char_id = ?1",
            params![char_id],
        )
        .map_err(io::Error::other)?;
    upsert_ascension_quota(&transaction, &quota, wall_clock)?;
    transaction.commit().map_err(io::Error::other)?;
    Ok(quota)
}

pub fn persist_zone_runtime_snapshot(
    settings: &PersistenceSettings,
    zones: &crate::world::zone::ZoneRegistry,
) -> io::Result<()> {
    let wall_clock = current_unix_seconds();
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    for zone in &zones.zones {
        upsert_zone_runtime(
            &transaction,
            &ZoneRuntimeRecord {
                zone_id: zone.name.clone(),
                spirit_qi: zone.spirit_qi,
                danger_level: zone.danger_level,
            },
            wall_clock,
        )?;
    }
    transaction.commit().map_err(io::Error::other)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn load_zone_runtime_snapshot(
    settings: &PersistenceSettings,
) -> io::Result<Vec<ZoneRuntimeRecord>> {
    let connection = open_persistence_connection(settings)?;
    load_zone_runtime_snapshot_from_connection(&connection)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn persist_zone_overlays(
    settings: &PersistenceSettings,
    overlays: &[ZoneOverlayRecord],
) -> io::Result<()> {
    let wall_clock = current_unix_seconds();
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    transaction
        .execute("DELETE FROM zone_overlays", [])
        .map_err(io::Error::other)?;
    for overlay in overlays {
        upsert_zone_overlay(&transaction, overlay, wall_clock)?;
    }
    transaction.commit().map_err(io::Error::other)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn load_zone_overlays(settings: &PersistenceSettings) -> io::Result<Vec<ZoneOverlayRecord>> {
    let connection = open_persistence_connection(settings)?;
    load_zone_overlays_from_connection(&connection)
}

pub fn export_zone_persistence(settings: &PersistenceSettings) -> io::Result<ZoneExportBundle> {
    Ok(ZoneExportBundle {
        schema_version: CURRENT_SCHEMA_VERSION,
        kind: "zones_export_v1".to_string(),
        zones_runtime: load_zone_runtime_snapshot(settings)?,
        zone_overlays: load_zone_overlays(settings)?,
    })
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn import_zone_persistence(
    settings: &PersistenceSettings,
    bundle: &ZoneExportBundle,
) -> io::Result<()> {
    if bundle.kind != "zones_export_v1" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unexpected zone export kind: {}", bundle.kind),
        ));
    }
    if bundle.schema_version > CURRENT_SCHEMA_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "zone export schema_version {} is newer than supported {}",
                bundle.schema_version, CURRENT_SCHEMA_VERSION
            ),
        ));
    }

    let wall_clock = current_unix_seconds();
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;

    transaction
        .execute("DELETE FROM zones_runtime", [])
        .map_err(io::Error::other)?;
    for runtime in &bundle.zones_runtime {
        upsert_zone_runtime(&transaction, runtime, wall_clock)?;
    }

    transaction
        .execute("DELETE FROM zone_overlays", [])
        .map_err(io::Error::other)?;
    for overlay in &bundle.zone_overlays {
        upsert_zone_overlay(&transaction, overlay, wall_clock)?;
    }

    transaction.commit().map_err(io::Error::other)
}

fn hydrate_zone_runtime(
    settings: &PersistenceSettings,
    zones: &mut crate::world::zone::ZoneRegistry,
) -> io::Result<()> {
    let runtime_rows = load_zone_runtime_snapshot(settings)?;
    zones.apply_runtime_records(&runtime_rows);
    Ok(())
}

fn hydrate_zone_overlays(
    settings: &PersistenceSettings,
    zones: &mut crate::world::zone::ZoneRegistry,
) -> io::Result<()> {
    let overlay_rows = load_zone_overlays(settings)?;
    zones
        .apply_overlay_records(&overlay_rows)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
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

fn utc_day_from_unix_seconds(unix_seconds: i64) -> i64 {
    unix_seconds.div_euclid(86_400)
}

#[derive(Debug, Default)]
struct DailyBackupRun {
    triggered: bool,
    backup_path: Option<PathBuf>,
    pruned_paths: Vec<PathBuf>,
}

fn run_daily_backup_cycle(
    settings: &PersistenceSettings,
    state: &mut DailyBackupState,
    wall_clock: i64,
) -> io::Result<DailyBackupRun> {
    let current_day = utc_day_from_unix_seconds(wall_clock);
    if state
        .last_backup_day
        .is_some_and(|last_backup_day| current_day <= last_backup_day)
    {
        return Ok(DailyBackupRun::default());
    }

    state.last_backup_day = Some(current_day);
    let backup_path = run_startup_backup(settings, wall_clock)?;
    let pruned_paths = prune_startup_backups(settings, STARTUP_BACKUP_KEEP_COUNT)?;
    Ok(DailyBackupRun {
        triggered: true,
        backup_path,
        pruned_paths,
    })
}

fn run_startup_backup(
    settings: &PersistenceSettings,
    wall_clock: i64,
) -> io::Result<Option<PathBuf>> {
    if !settings.db_path().exists() {
        return Ok(None);
    }

    let backup_path = startup_backup_path(settings, wall_clock);
    snapshot_existing_sqlite(settings.db_path(), &backup_path)?;
    Ok(Some(backup_path))
}

fn startup_backup_path(settings: &PersistenceSettings, wall_clock: i64) -> PathBuf {
    resolve_persistence_relative_path(settings, STARTUP_BACKUP_DIR).join(format!(
        "{STARTUP_BACKUP_FILE_PREFIX}{}{STARTUP_BACKUP_FILE_SUFFIX}",
        format_startup_backup_stamp(wall_clock),
    ))
}

fn format_startup_backup_stamp(unix_seconds: i64) -> String {
    let days = unix_seconds.div_euclid(86_400);
    let seconds_of_day = unix_seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;

    format!("{year:04}{month:02}{day:02}-{hour:02}{minute:02}{second:02}",)
}

fn snapshot_existing_sqlite(db_path: &Path, backup_path: &Path) -> io::Result<()> {
    if let Some(parent) = backup_path.parent() {
        fs::create_dir_all(parent)?;
    }
    if backup_path.exists() {
        fs::remove_file(backup_path)?;
    }

    let connection = Connection::open(db_path).map_err(io::Error::other)?;
    configure_connection(&connection).map_err(io::Error::other)?;
    let escaped_path = backup_path.to_string_lossy().replace('\'', "''");
    let sql = format!("VACUUM main INTO '{escaped_path}';");
    connection.execute_batch(&sql).map_err(io::Error::other)
}

fn prune_startup_backups(settings: &PersistenceSettings, keep: usize) -> io::Result<Vec<PathBuf>> {
    let backup_root = resolve_persistence_relative_path(settings, STARTUP_BACKUP_DIR);
    let mut backup_files = collect_files_with_suffix(&backup_root, STARTUP_BACKUP_FILE_SUFFIX)?;
    backup_files.retain(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| {
                name.starts_with(STARTUP_BACKUP_FILE_PREFIX)
                    && name.ends_with(STARTUP_BACKUP_FILE_SUFFIX)
            })
    });
    backup_files.sort_by(|left, right| {
        left.file_name()
            .cmp(&right.file_name())
            .then_with(|| left.cmp(right))
    });

    if backup_files.len() <= keep {
        return Ok(Vec::new());
    }

    let stale_count = backup_files.len() - keep;
    let stale_files = backup_files
        .into_iter()
        .take(stale_count)
        .collect::<Vec<_>>();
    for path in &stale_files {
        fs::remove_file(path)?;
    }

    Ok(stale_files)
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

#[allow(clippy::too_many_arguments)]
pub fn capture_npc_persistence(
    entity: Entity,
    position: &Position,
    kind: EntityKind,
    state: NpcStateKind,
    blackboard: &NpcBlackboard,
    nearest_player_id: Option<&str>,
    loadout: &NpcCombatLoadout,
    patrol: &NpcPatrol,
    movement: &MovementController,
    cooldowns: &MovementCooldowns,
    lifecycle: &Lifecycle,
    cultivation: Option<&Cultivation>,
    life_record: Option<&LifeRecord>,
) -> NpcPersistenceCapture {
    let char_id = if lifecycle.character_id != "unbound:character" {
        lifecycle.character_id.clone()
    } else {
        canonical_npc_id(entity)
    };
    let archetype = npc_archetype_label(loadout.melee_archetype).to_string();
    let blackboard_snapshot = build_npc_blackboard_snapshot(blackboard, nearest_player_id);
    let since_tick = life_record
        .map(|record| record.created_at)
        .unwrap_or_else(|| lifecycle.last_revive_tick.unwrap_or_default());
    let digest = NpcDigestRecord {
        char_id: char_id.clone(),
        archetype: archetype.clone(),
        realm: cultivation
            .map(|cultivation| format!("{:?}", cultivation.realm).to_ascii_lowercase())
            .unwrap_or_else(|| "unknown".to_string()),
        faction_id: None,
        recent_summary: life_record
            .map(|record| record.recent_summary_text(3))
            .filter(|summary| !summary.is_empty())
            .unwrap_or_else(|| format!("{}:{}", char_id, state_label(&state))),
        last_referenced_wall: current_unix_seconds(),
    };

    NpcPersistenceCapture {
        state: NpcStateRecord {
            char_id: char_id.clone(),
            kind: entity_kind_label(kind).to_string(),
            pos: vec3_to_array(position.get()),
            state: state_label(&state).to_string(),
            blackboard: blackboard_snapshot,
            archetype: archetype.clone(),
            home_zone: patrol.home_zone.clone(),
            patrol_anchor_index: patrol.anchor_index,
            patrol_target: vec3_to_array(patrol.current_target),
            movement_mode: movement_mode_label(&movement.mode).to_string(),
            can_sprint: loadout.movement_capabilities.can_sprint,
            can_dash: loadout.movement_capabilities.can_dash,
            sprint_ready_at: cooldowns.sprint_ready_at,
            dash_ready_at: cooldowns.dash_ready_at,
            lifecycle_state: lifecycle_state_label(&lifecycle.state).to_string(),
            death_count: lifecycle.death_count,
            last_death_tick: lifecycle.last_death_tick,
            last_revive_tick: lifecycle.last_revive_tick,
        },
        digest,
        archetype_entry: ArchetypeRegistryEntry {
            char_id,
            archetype,
            since_tick,
        },
        captured_at_wall: current_unix_seconds(),
    }
}

pub fn persist_npc_capture(
    settings: &PersistenceSettings,
    capture: &NpcPersistenceCapture,
) -> io::Result<()> {
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    (|| -> io::Result<()> {
        upsert_npc_state(&transaction, &capture.state, capture.captured_at_wall)?;
        upsert_npc_digest(&transaction, &capture.digest, capture.captured_at_wall)?;
        upsert_archetype_registry_entry(
            &transaction,
            &capture.archetype_entry,
            capture.captured_at_wall,
        )?;
        transaction.commit().map_err(io::Error::other)
    })()
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn load_npc_state(
    settings: &PersistenceSettings,
    char_id: &str,
) -> io::Result<Option<NpcStateRecord>> {
    let connection = open_persistence_connection(settings)?;
    load_npc_state_from_connection(&connection, char_id)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn load_npc_digest(
    settings: &PersistenceSettings,
    char_id: &str,
) -> io::Result<Option<NpcDigestRecord>> {
    let connection = open_persistence_connection(settings)?;
    load_npc_digest_from_connection(&connection, char_id)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn record_archetype_transition(
    settings: &PersistenceSettings,
    entry: &ArchetypeRegistryEntry,
) -> io::Result<()> {
    let wall_clock = current_unix_seconds();
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    upsert_archetype_registry_entry(&transaction, entry, wall_clock)?;
    transaction.commit().map_err(io::Error::other)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn load_archetype_registry(
    settings: &PersistenceSettings,
    char_id: &str,
) -> io::Result<Vec<ArchetypeRegistryEntry>> {
    let connection = open_persistence_connection(settings)?;
    load_archetype_registry_from_connection(&connection, char_id)
}

pub fn persist_npc_deceased_archive(
    settings: &PersistenceSettings,
    archive: &NpcDeceasedArchiveRecord,
) -> io::Result<()> {
    let archive_path = npc_deceased_archive_absolute_path(
        settings,
        archive.char_id.as_str(),
        archive.archived_at_wall,
    );
    let relative_path =
        npc_deceased_archive_relative_path(archive.char_id.as_str(), archive.archived_at_wall);
    let previous_archive = fs::read(&archive_path).ok();
    let archive_json = serde_json::to_vec_pretty(archive)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    write_zstd_bundle(&archive_path, &archive_json)?;

    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    let persisted = (|| -> io::Result<()> {
        upsert_npc_deceased_index(
            &transaction,
            &NpcDeceasedIndexRecord {
                char_id: archive.char_id.clone(),
                archetype: archive.archetype.clone(),
                died_at_tick: archive.died_at_tick,
                path: relative_path.clone(),
            },
            archive.archived_at_wall,
        )?;
        delete_npc_hot_rows(&transaction, archive.char_id.as_str())?;
        transaction.commit().map_err(io::Error::other)
    })();

    if persisted.is_err() {
        rollback_file(&archive_path, previous_archive.as_deref());
    }

    persisted
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn load_npc_deceased_archive(
    settings: &PersistenceSettings,
    char_id: &str,
) -> io::Result<Option<NpcDeceasedArchiveRecord>> {
    let connection = open_persistence_connection(settings)?;
    let path: Option<String> = connection
        .query_row(
            "SELECT path FROM npc_deceased_index WHERE char_id = ?1",
            params![char_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(io::Error::other)?;
    let Some(path) = path else {
        return Ok(None);
    };
    let bytes = read_zstd_bundle(settings.db_path(), path.as_str())?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

pub fn sweep_stale_npc_digests(
    settings: &PersistenceSettings,
    now_wall: i64,
) -> io::Result<Vec<NpcDigestRecord>> {
    let threshold = now_wall - NPC_DIGEST_RETENTION_SECS;
    let mut connection = open_persistence_connection(settings)?;
    let stale_digests = load_stale_npc_digests(&connection, threshold)?;
    if stale_digests.is_empty() {
        return Ok(Vec::new());
    }

    for digest in &stale_digests {
        let archive_path =
            npc_digest_archive_absolute_path(settings, digest.char_id.as_str(), now_wall);
        let previous_archive = fs::read(&archive_path).ok();
        let archive_json = serde_json::to_vec_pretty(digest)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        if let Err(error) = write_zstd_bundle(&archive_path, &archive_json) {
            rollback_file(&archive_path, previous_archive.as_deref());
            return Err(error);
        }
    }

    let transaction = connection.transaction().map_err(io::Error::other)?;
    for digest in &stale_digests {
        transaction
            .execute(
                "DELETE FROM npc_digests WHERE char_id = ?1",
                params![digest.char_id.as_str()],
            )
            .map_err(io::Error::other)?;
    }
    transaction.commit().map_err(io::Error::other)?;

    Ok(stale_digests)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn replace_faction_social_state(
    settings: &PersistenceSettings,
    bundle: &FactionSocialBundle,
) -> io::Result<()> {
    let wall_clock = current_unix_seconds();
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    (|| -> io::Result<()> {
        transaction
            .execute("DELETE FROM factions", [])
            .map_err(io::Error::other)?;
        transaction
            .execute("DELETE FROM reputation", [])
            .map_err(io::Error::other)?;
        transaction
            .execute("DELETE FROM membership", [])
            .map_err(io::Error::other)?;
        transaction
            .execute("DELETE FROM relationships", [])
            .map_err(io::Error::other)?;

        for faction in &bundle.factions {
            upsert_faction(&transaction, faction, wall_clock)?;
        }
        for reputation in &bundle.reputations {
            upsert_faction_reputation(&transaction, reputation, wall_clock)?;
        }
        for membership in &bundle.memberships {
            upsert_faction_membership(&transaction, membership, wall_clock)?;
        }
        for relationship in &bundle.relationships {
            upsert_relationship(&transaction, relationship, wall_clock)?;
        }

        transaction.commit().map_err(io::Error::other)
    })()
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn load_faction_social_state(
    settings: &PersistenceSettings,
) -> io::Result<FactionSocialBundle> {
    let connection = open_persistence_connection(settings)?;
    Ok(FactionSocialBundle {
        factions: load_factions_from_connection(&connection)?,
        reputations: load_reputations_from_connection(&connection)?,
        memberships: load_memberships_from_connection(&connection)?,
        relationships: load_relationships_from_connection(&connection)?,
    })
}

type NpcPersistenceQueryItem<'a> = (
    Entity,
    &'a Position,
    &'a EntityKind,
    &'a NpcBlackboard,
    &'a NpcCombatLoadout,
    &'a NpcPatrol,
    &'a MovementController,
    &'a MovementCooldowns,
    &'a Lifecycle,
    Option<&'a Cultivation>,
    Option<&'a LifeRecord>,
    Option<&'a NpcArchivedPersistence>,
);

#[allow(clippy::too_many_arguments)]
fn persist_npc_runtime_state_system(
    settings: Res<PersistenceSettings>,
    mut commands: Commands,
    mut snapshot_tracker: ResMut<NpcSnapshotTracker>,
    players: Query<(Entity, &Username), With<Client>>,
    npcs: Query<NpcPersistenceQueryItem<'_>, With<NpcMarker>>,
    flee_actions: Query<(&Actor, &ActionState), With<FleeAction>>,
    chase_actions: Query<(&Actor, &ActionState), With<ChaseAction>>,
    melee_actions: Query<(&Actor, &ActionState), With<MeleeAttackAction>>,
    dash_actions: Query<(&Actor, &ActionState), With<DashAction>>,
    game_tick: Option<Res<crate::npc::movement::GameTick>>,
) {
    let snapshot_due = game_tick.as_ref().is_none_or(|tick| {
        tick.0.wrapping_sub(snapshot_tracker.last_snapshot_tick) >= NPC_SNAPSHOT_INTERVAL_TICKS
    });
    let action_states =
        collect_npc_action_states(&flee_actions, &chase_actions, &melee_actions, &dash_actions);

    for (
        entity,
        position,
        kind,
        blackboard,
        loadout,
        patrol,
        movement,
        cooldowns,
        lifecycle,
        cultivation,
        life_record,
        archived,
    ) in &npcs
    {
        let nearest_player_id = resolve_nearest_player_id(blackboard, &players);
        let effective_state = effective_npc_state(entity, lifecycle, &action_states);
        let should_snapshot =
            snapshot_due || archived.is_none() || lifecycle.state == LifecycleState::Terminated;
        if !should_snapshot {
            continue;
        }

        let capture = capture_npc_persistence(
            entity,
            position,
            *kind,
            effective_state,
            blackboard,
            nearest_player_id.as_deref(),
            loadout,
            patrol,
            movement,
            cooldowns,
            lifecycle,
            cultivation,
            life_record,
        );

        let result = if lifecycle.state == LifecycleState::Terminated {
            persist_npc_deceased_archive(
                &settings,
                &NpcDeceasedArchiveRecord {
                    char_id: capture.state.char_id.clone(),
                    archetype: capture.state.archetype.clone(),
                    died_at_tick: lifecycle.last_death_tick.unwrap_or_default(),
                    archived_at_wall: capture.captured_at_wall,
                    lifecycle_state: capture.state.lifecycle_state.clone(),
                    death_count: capture.state.death_count,
                    state: Some(capture.state.clone()),
                    digest: Some(capture.digest.clone()),
                    life_record: life_record.cloned(),
                },
            )
        } else {
            persist_npc_capture(&settings, &capture)
        };

        if let Err(error) = result {
            tracing::warn!(
                "[bong][persistence] failed to persist npc {}: {error}",
                capture.state.char_id
            );
            continue;
        }

        if lifecycle.state == LifecycleState::Terminated && archived.is_none() {
            commands.entity(entity).insert(NpcArchivedPersistence);
        }
    }

    if snapshot_due {
        if let Some(tick) = game_tick.as_ref() {
            snapshot_tracker.last_snapshot_tick = tick.0;
        }
    }
}

fn sweep_npc_digest_retention_system(
    settings: Res<PersistenceSettings>,
    mut sweep_state: ResMut<NpcDigestSweepState>,
) {
    let now_wall = current_unix_seconds();
    if sweep_state.last_sweep_wall > 0
        && now_wall.saturating_sub(sweep_state.last_sweep_wall) < NPC_DIGEST_SWEEP_INTERVAL_SECS
    {
        return;
    }

    match sweep_stale_npc_digests(&settings, now_wall) {
        Ok(_) => {
            sweep_state.last_sweep_wall = now_wall;
        }
        Err(error) => {
            tracing::warn!("[bong][persistence] failed npc digest retention sweep: {error}");
        }
    }
}

fn effective_npc_state(
    entity: Entity,
    lifecycle: &Lifecycle,
    action_states: &HashMap<Entity, NpcStateKind>,
) -> NpcStateKind {
    if lifecycle.state == LifecycleState::Terminated {
        return NpcStateKind::Idle;
    }
    action_states
        .get(&entity)
        .cloned()
        .unwrap_or(NpcStateKind::Idle)
}

fn collect_npc_action_states(
    flee_actions: &Query<(&Actor, &ActionState), With<FleeAction>>,
    chase_actions: &Query<(&Actor, &ActionState), With<ChaseAction>>,
    melee_actions: &Query<(&Actor, &ActionState), With<MeleeAttackAction>>,
    dash_actions: &Query<(&Actor, &ActionState), With<DashAction>>,
) -> HashMap<Entity, NpcStateKind> {
    let mut states = HashMap::new();
    for (Actor(entity), action_state) in chase_actions.iter() {
        if matches!(action_state, ActionState::Executing) {
            states.insert(*entity, NpcStateKind::Patrolling);
        }
    }
    for (Actor(entity), action_state) in flee_actions.iter() {
        if matches!(action_state, ActionState::Executing) {
            states.insert(*entity, NpcStateKind::Fleeing);
        }
    }
    for (Actor(entity), action_state) in dash_actions.iter() {
        if matches!(action_state, ActionState::Executing) {
            states.insert(*entity, NpcStateKind::Attacking);
        }
    }
    for (Actor(entity), action_state) in melee_actions.iter() {
        if matches!(action_state, ActionState::Executing) {
            states.insert(*entity, NpcStateKind::Attacking);
        }
    }
    states
}

fn resolve_nearest_player_id(
    blackboard: &NpcBlackboard,
    players: &Query<(Entity, &Username), With<Client>>,
) -> Option<String> {
    let player_entity = blackboard.nearest_player?;
    let Ok((_, username)) = players.get(player_entity) else {
        return None;
    };
    Some(canonical_player_id(username.0.as_str()))
}

fn open_persistence_connection(settings: &PersistenceSettings) -> io::Result<Connection> {
    if let Some(parent) = settings.db_path().parent() {
        fs::create_dir_all(parent)?;
    }

    let connection = Connection::open(settings.db_path()).map_err(io::Error::other)?;
    configure_connection(&connection).map_err(io::Error::other)?;
    Ok(connection)
}

fn upsert_npc_state(
    transaction: &rusqlite::Transaction<'_>,
    state: &NpcStateRecord,
    wall_clock: i64,
) -> io::Result<()> {
    let blackboard_json = serde_json::to_string(&state.blackboard)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    transaction
        .execute(
            "
            INSERT INTO npc_state (
                char_id,
                kind,
                archetype,
                pos_x,
                pos_y,
                pos_z,
                state,
                blackboard_json,
                home_zone,
                patrol_anchor_index,
                patrol_target_x,
                patrol_target_y,
                patrol_target_z,
                movement_mode,
                can_sprint,
                can_dash,
                sprint_ready_at,
                dash_ready_at,
                lifecycle_state,
                death_count,
                last_death_tick,
                last_revive_tick,
                schema_version,
                last_updated_wall
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16,
                ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24
            )
            ON CONFLICT(char_id) DO UPDATE SET
                kind = excluded.kind,
                archetype = excluded.archetype,
                pos_x = excluded.pos_x,
                pos_y = excluded.pos_y,
                pos_z = excluded.pos_z,
                state = excluded.state,
                blackboard_json = excluded.blackboard_json,
                home_zone = excluded.home_zone,
                patrol_anchor_index = excluded.patrol_anchor_index,
                patrol_target_x = excluded.patrol_target_x,
                patrol_target_y = excluded.patrol_target_y,
                patrol_target_z = excluded.patrol_target_z,
                movement_mode = excluded.movement_mode,
                can_sprint = excluded.can_sprint,
                can_dash = excluded.can_dash,
                sprint_ready_at = excluded.sprint_ready_at,
                dash_ready_at = excluded.dash_ready_at,
                lifecycle_state = excluded.lifecycle_state,
                death_count = excluded.death_count,
                last_death_tick = excluded.last_death_tick,
                last_revive_tick = excluded.last_revive_tick,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                state.char_id,
                state.kind,
                state.archetype,
                state.pos[0],
                state.pos[1],
                state.pos[2],
                state.state,
                blackboard_json,
                state.home_zone,
                sql_usize(state.patrol_anchor_index)?,
                state.patrol_target[0],
                state.patrol_target[1],
                state.patrol_target[2],
                state.movement_mode,
                bool_to_sql(state.can_sprint),
                bool_to_sql(state.can_dash),
                i64::from(state.sprint_ready_at),
                i64::from(state.dash_ready_at),
                state.lifecycle_state,
                i64::from(state.death_count),
                optional_tick_to_sql(state.last_death_tick)?,
                optional_tick_to_sql(state.last_revive_tick)?,
                NPC_ROW_SCHEMA_VERSION,
                wall_clock,
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

fn upsert_npc_digest(
    transaction: &rusqlite::Transaction<'_>,
    digest: &NpcDigestRecord,
    wall_clock: i64,
) -> io::Result<()> {
    transaction
        .execute(
            "
            INSERT INTO npc_digests (
                char_id,
                archetype,
                realm,
                faction_id,
                recent_summary,
                last_referenced_wall,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(char_id) DO UPDATE SET
                archetype = excluded.archetype,
                realm = excluded.realm,
                faction_id = excluded.faction_id,
                recent_summary = excluded.recent_summary,
                last_referenced_wall = excluded.last_referenced_wall,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                digest.char_id,
                digest.archetype,
                digest.realm,
                digest.faction_id,
                digest.recent_summary,
                digest.last_referenced_wall,
                NPC_ROW_SCHEMA_VERSION,
                wall_clock,
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

fn upsert_archetype_registry_entry(
    transaction: &rusqlite::Transaction<'_>,
    entry: &ArchetypeRegistryEntry,
    wall_clock: i64,
) -> io::Result<()> {
    transaction
        .execute(
            "
            INSERT INTO archetype_registry (
                char_id,
                archetype,
                since_tick,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(char_id, since_tick, archetype) DO UPDATE SET
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                entry.char_id,
                entry.archetype,
                tick_to_sql(entry.since_tick)?,
                NPC_ROW_SCHEMA_VERSION,
                wall_clock,
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

fn upsert_npc_deceased_index(
    transaction: &rusqlite::Transaction<'_>,
    entry: &NpcDeceasedIndexRecord,
    wall_clock: i64,
) -> io::Result<()> {
    transaction
        .execute(
            "
            INSERT INTO npc_deceased_index (
                char_id,
                archetype,
                died_at_tick,
                path,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(char_id) DO UPDATE SET
                archetype = excluded.archetype,
                died_at_tick = excluded.died_at_tick,
                path = excluded.path,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                entry.char_id,
                entry.archetype,
                tick_to_sql(entry.died_at_tick)?,
                entry.path,
                NPC_ROW_SCHEMA_VERSION,
                wall_clock,
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

fn upsert_active_tribulation(
    transaction: &rusqlite::Transaction<'_>,
    record: &ActiveTribulationRecord,
    wall_clock: i64,
) -> io::Result<()> {
    transaction
        .execute(
            "
            INSERT INTO tribulations_active (
                char_id,
                wave_current,
                waves_total,
                started_tick,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(char_id) DO UPDATE SET
                wave_current = excluded.wave_current,
                waves_total = excluded.waves_total,
                started_tick = excluded.started_tick,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                record.char_id,
                i64::from(record.wave_current),
                i64::from(record.waves_total),
                tick_to_sql(record.started_tick)?,
                CURRENT_SCHEMA_VERSION,
                wall_clock,
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

fn upsert_ascension_quota(
    transaction: &rusqlite::Transaction<'_>,
    record: &AscensionQuotaRecord,
    wall_clock: i64,
) -> io::Result<()> {
    transaction
        .execute(
            "
            INSERT INTO ascension_quota (
                row_id,
                occupied_slots,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(row_id) DO UPDATE SET
                occupied_slots = excluded.occupied_slots,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                ASCENSION_QUOTA_ROW_ID,
                i64::from(record.occupied_slots),
                CURRENT_SCHEMA_VERSION,
                wall_clock,
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

fn upsert_zone_runtime(
    transaction: &rusqlite::Transaction<'_>,
    record: &ZoneRuntimeRecord,
    wall_clock: i64,
) -> io::Result<()> {
    transaction
        .execute(
            "
            INSERT INTO zones_runtime (
                zone_id,
                spirit_qi,
                danger_level,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(zone_id) DO UPDATE SET
                spirit_qi = excluded.spirit_qi,
                danger_level = excluded.danger_level,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                record.zone_id,
                record.spirit_qi,
                i64::from(record.danger_level),
                CURRENT_SCHEMA_VERSION,
                wall_clock,
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

#[cfg_attr(not(test), allow(dead_code))]
fn upsert_zone_overlay(
    transaction: &rusqlite::Transaction<'_>,
    record: &ZoneOverlayRecord,
    wall_clock: i64,
) -> io::Result<()> {
    transaction
        .execute(
            "
            INSERT INTO zone_overlays (
                zone_id,
                overlay_kind,
                payload_json,
                payload_version,
                since_wall,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(zone_id, overlay_kind, since_wall) DO UPDATE SET
                payload_json = excluded.payload_json,
                payload_version = excluded.payload_version,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                record.zone_id,
                record.overlay_kind,
                record.payload_json,
                record.payload_version,
                record.since_wall,
                CURRENT_SCHEMA_VERSION,
                wall_clock,
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

fn delete_npc_hot_rows(transaction: &rusqlite::Transaction<'_>, char_id: &str) -> io::Result<()> {
    transaction
        .execute("DELETE FROM npc_state WHERE char_id = ?1", params![char_id])
        .map_err(io::Error::other)?;
    transaction
        .execute(
            "DELETE FROM npc_digests WHERE char_id = ?1",
            params![char_id],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

#[cfg_attr(not(test), allow(dead_code))]
fn load_active_tribulation_from_connection(
    connection: &Connection,
    char_id: &str,
) -> io::Result<Option<ActiveTribulationRecord>> {
    let row: Option<(i64, i64, i64)> = connection
        .query_row(
            "
            SELECT wave_current, waves_total, started_tick
            FROM tribulations_active
            WHERE char_id = ?1
            ",
            params![char_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(io::Error::other)?;
    let Some((wave_current, waves_total, started_tick)) = row else {
        return Ok(None);
    };

    Ok(Some(ActiveTribulationRecord {
        char_id: char_id.to_string(),
        wave_current: sql_to_u32(wave_current)?,
        waves_total: sql_to_u32(waves_total)?,
        started_tick: sql_to_tick(started_tick)?,
    }))
}

#[cfg_attr(not(test), allow(dead_code))]
fn load_ascension_quota_from_connection(
    connection: &Connection,
) -> io::Result<AscensionQuotaRecord> {
    let row: Option<i64> = connection
        .query_row(
            "SELECT occupied_slots FROM ascension_quota WHERE row_id = ?1",
            params![ASCENSION_QUOTA_ROW_ID],
            |row| row.get(0),
        )
        .optional()
        .map_err(io::Error::other)?;

    Ok(AscensionQuotaRecord {
        occupied_slots: match row {
            Some(occupied_slots) => sql_to_u32(occupied_slots)?,
            None => 0,
        },
    })
}

fn load_zone_overlays_from_connection(
    connection: &Connection,
) -> io::Result<Vec<ZoneOverlayRecord>> {
    let mut statement = connection
        .prepare(
            "
            SELECT zone_id, overlay_kind, payload_json, payload_version, since_wall
            FROM zone_overlays
            ORDER BY zone_id ASC, overlay_kind ASC, since_wall ASC
            ",
        )
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map([], |row| {
            Ok(ZoneOverlayRecord {
                zone_id: row.get(0)?,
                overlay_kind: row.get(1)?,
                payload_json: row.get(2)?,
                payload_version: row.get(3)?,
                since_wall: row.get(4)?,
            })
        })
        .map_err(io::Error::other)?;

    let mut overlays = Vec::new();
    for row in rows {
        overlays.push(row.map_err(io::Error::other)?);
    }
    Ok(overlays)
}

fn load_zone_runtime_snapshot_from_connection(
    connection: &Connection,
) -> io::Result<Vec<ZoneRuntimeRecord>> {
    let mut statement = connection
        .prepare(
            "
            SELECT zone_id, spirit_qi, danger_level
            FROM zones_runtime
            ORDER BY zone_id ASC
            ",
        )
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, f64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .map_err(io::Error::other)?;

    let mut records = Vec::new();
    for row in rows {
        let (zone_id, spirit_qi, danger_level) = row.map_err(io::Error::other)?;
        records.push(ZoneRuntimeRecord {
            zone_id,
            spirit_qi,
            danger_level: sql_to_u8(danger_level)?,
        });
    }
    Ok(records)
}

fn load_ascension_quota_from_transaction(
    transaction: &rusqlite::Transaction<'_>,
) -> io::Result<AscensionQuotaRecord> {
    let row: Option<i64> = transaction
        .query_row(
            "SELECT occupied_slots FROM ascension_quota WHERE row_id = ?1",
            params![ASCENSION_QUOTA_ROW_ID],
            |row| row.get(0),
        )
        .optional()
        .map_err(io::Error::other)?;

    Ok(AscensionQuotaRecord {
        occupied_slots: match row {
            Some(occupied_slots) => sql_to_u32(occupied_slots)?,
            None => 0,
        },
    })
}

#[cfg_attr(not(test), allow(dead_code))]
fn load_npc_state_from_connection(
    connection: &Connection,
    char_id: &str,
) -> io::Result<Option<NpcStateRecord>> {
    let row: Option<NpcStateSqlRow> = connection
        .query_row(
            "
            SELECT kind, archetype, pos_x, pos_y, pos_z, state, blackboard_json, home_zone,
                   patrol_anchor_index, patrol_target_x, patrol_target_y, patrol_target_z,
                   movement_mode, can_sprint, can_dash, sprint_ready_at, dash_ready_at,
                   lifecycle_state, death_count, last_death_tick, last_revive_tick
            FROM npc_state
            WHERE char_id = ?1
            ",
            params![char_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get(9)?,
                    row.get(10)?,
                    row.get(11)?,
                    row.get(12)?,
                    row.get(13)?,
                    row.get(14)?,
                    row.get(15)?,
                    row.get(16)?,
                    row.get(17)?,
                    row.get(18)?,
                    row.get(19)?,
                    row.get(20)?,
                ))
            },
        )
        .optional()
        .map_err(io::Error::other)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let blackboard = serde_json::from_str(&row.6)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    Ok(Some(NpcStateRecord {
        char_id: char_id.to_string(),
        kind: row.0,
        archetype: row.1,
        pos: [row.2, row.3, row.4],
        state: row.5,
        blackboard,
        home_zone: row.7,
        patrol_anchor_index: sql_to_usize(row.8)?,
        patrol_target: [row.9, row.10, row.11],
        movement_mode: row.12,
        can_sprint: sql_to_bool(row.13),
        can_dash: sql_to_bool(row.14),
        sprint_ready_at: sql_to_u32(row.15)?,
        dash_ready_at: sql_to_u32(row.16)?,
        lifecycle_state: row.17,
        death_count: sql_to_u32(row.18)?,
        last_death_tick: optional_sql_to_tick(row.19)?,
        last_revive_tick: optional_sql_to_tick(row.20)?,
    }))
}

#[cfg_attr(not(test), allow(dead_code))]
fn load_npc_digest_from_connection(
    connection: &Connection,
    char_id: &str,
) -> io::Result<Option<NpcDigestRecord>> {
    let row: Option<(String, String, Option<String>, String, i64)> = connection
        .query_row(
            "
            SELECT archetype, realm, faction_id, recent_summary, last_referenced_wall
            FROM npc_digests
            WHERE char_id = ?1
            ",
            params![char_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .optional()
        .map_err(io::Error::other)?;
    Ok(row.map(
        |(archetype, realm, faction_id, recent_summary, last_referenced_wall)| NpcDigestRecord {
            char_id: char_id.to_string(),
            archetype,
            realm,
            faction_id,
            recent_summary,
            last_referenced_wall,
        },
    ))
}

#[cfg_attr(not(test), allow(dead_code))]
fn load_archetype_registry_from_connection(
    connection: &Connection,
    char_id: &str,
) -> io::Result<Vec<ArchetypeRegistryEntry>> {
    let mut statement = connection
        .prepare(
            "
            SELECT archetype, since_tick
            FROM archetype_registry
            WHERE char_id = ?1
            ORDER BY since_tick ASC, archetype ASC
            ",
        )
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map(params![char_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(io::Error::other)?;

    let mut registry = Vec::new();
    for row in rows {
        let (archetype, since_tick) = row.map_err(io::Error::other)?;
        registry.push(ArchetypeRegistryEntry {
            char_id: char_id.to_string(),
            archetype,
            since_tick: sql_to_tick(since_tick)?,
        });
    }
    Ok(registry)
}

fn load_stale_npc_digests(
    connection: &Connection,
    threshold: i64,
) -> io::Result<Vec<NpcDigestRecord>> {
    let mut statement = connection
        .prepare(
            "
            SELECT char_id, archetype, realm, faction_id, recent_summary, last_referenced_wall
            FROM npc_digests
            WHERE last_referenced_wall < ?1
            ORDER BY last_referenced_wall ASC, char_id ASC
            ",
        )
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map(params![threshold], |row| {
            Ok(NpcDigestRecord {
                char_id: row.get(0)?,
                archetype: row.get(1)?,
                realm: row.get(2)?,
                faction_id: row.get(3)?,
                recent_summary: row.get(4)?,
                last_referenced_wall: row.get(5)?,
            })
        })
        .map_err(io::Error::other)?;

    let mut digests = Vec::new();
    for row in rows {
        digests.push(row.map_err(io::Error::other)?);
    }
    Ok(digests)
}

#[cfg_attr(not(test), allow(dead_code))]
fn upsert_faction(
    transaction: &rusqlite::Transaction<'_>,
    faction: &FactionRecord,
    wall_clock: i64,
) -> io::Result<()> {
    transaction
        .execute(
            "
            INSERT INTO factions (
                faction_id, display_name, doctrine, metadata_json, schema_version, last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(faction_id) DO UPDATE SET
                display_name = excluded.display_name,
                doctrine = excluded.doctrine,
                metadata_json = excluded.metadata_json,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                faction.faction_id,
                faction.display_name,
                faction.doctrine,
                faction.metadata_json,
                NPC_ROW_SCHEMA_VERSION,
                wall_clock,
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

#[cfg_attr(not(test), allow(dead_code))]
fn upsert_faction_reputation(
    transaction: &rusqlite::Transaction<'_>,
    reputation: &FactionReputationRecord,
    wall_clock: i64,
) -> io::Result<()> {
    transaction
        .execute(
            "
            INSERT INTO reputation (
                faction_id, target_faction_id, score, schema_version, last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(faction_id, target_faction_id) DO UPDATE SET
                score = excluded.score,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                reputation.faction_id,
                reputation.target_faction_id,
                reputation.score,
                NPC_ROW_SCHEMA_VERSION,
                wall_clock,
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

#[cfg_attr(not(test), allow(dead_code))]
fn upsert_faction_membership(
    transaction: &rusqlite::Transaction<'_>,
    membership: &FactionMembershipRecord,
    wall_clock: i64,
) -> io::Result<()> {
    transaction
        .execute(
            "
            INSERT INTO membership (
                faction_id, char_id, role, joined_at_tick, metadata_json, schema_version, last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(faction_id, char_id) DO UPDATE SET
                role = excluded.role,
                joined_at_tick = excluded.joined_at_tick,
                metadata_json = excluded.metadata_json,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                membership.faction_id,
                membership.char_id,
                membership.role,
                tick_to_sql(membership.joined_at_tick)?,
                membership.metadata_json,
                NPC_ROW_SCHEMA_VERSION,
                wall_clock,
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

#[cfg_attr(not(test), allow(dead_code))]
fn upsert_relationship(
    transaction: &rusqlite::Transaction<'_>,
    relationship: &RelationshipRecord,
    wall_clock: i64,
) -> io::Result<()> {
    transaction
        .execute(
            "
            INSERT INTO relationships (
                char_id, peer_char_id, relationship_type, since_tick, metadata_json, schema_version, last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(char_id, peer_char_id, relationship_type) DO UPDATE SET
                since_tick = excluded.since_tick,
                metadata_json = excluded.metadata_json,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                relationship.char_id,
                relationship.peer_char_id,
                relationship.relationship_type,
                tick_to_sql(relationship.since_tick)?,
                relationship.metadata_json,
                NPC_ROW_SCHEMA_VERSION,
                wall_clock,
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

#[cfg_attr(not(test), allow(dead_code))]
fn load_factions_from_connection(connection: &Connection) -> io::Result<Vec<FactionRecord>> {
    let mut statement = connection
        .prepare(
            "SELECT faction_id, display_name, doctrine, metadata_json FROM factions ORDER BY faction_id ASC",
        )
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map([], |row| {
            Ok(FactionRecord {
                faction_id: row.get(0)?,
                display_name: row.get(1)?,
                doctrine: row.get(2)?,
                metadata_json: row.get(3)?,
            })
        })
        .map_err(io::Error::other)?;
    collect_rows(rows)
}

#[cfg_attr(not(test), allow(dead_code))]
fn load_reputations_from_connection(
    connection: &Connection,
) -> io::Result<Vec<FactionReputationRecord>> {
    let mut statement = connection
        .prepare(
            "SELECT faction_id, target_faction_id, score FROM reputation ORDER BY faction_id ASC, target_faction_id ASC",
        )
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map([], |row| {
            Ok(FactionReputationRecord {
                faction_id: row.get(0)?,
                target_faction_id: row.get(1)?,
                score: row.get(2)?,
            })
        })
        .map_err(io::Error::other)?;
    collect_rows(rows)
}

#[cfg_attr(not(test), allow(dead_code))]
fn load_memberships_from_connection(
    connection: &Connection,
) -> io::Result<Vec<FactionMembershipRecord>> {
    let mut statement = connection
        .prepare(
            "
            SELECT faction_id, char_id, role, joined_at_tick, metadata_json
            FROM membership
            ORDER BY faction_id ASC, char_id ASC
            ",
        )
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map([], |row| {
            Ok(FactionMembershipRecord {
                faction_id: row.get(0)?,
                char_id: row.get(1)?,
                role: row.get(2)?,
                joined_at_tick: sql_to_tick(row.get::<_, i64>(3)?).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        3,
                        rusqlite::types::Type::Integer,
                        Box::new(error),
                    )
                })?,
                metadata_json: row.get(4)?,
            })
        })
        .map_err(io::Error::other)?;
    collect_rows(rows)
}

#[cfg_attr(not(test), allow(dead_code))]
fn load_relationships_from_connection(
    connection: &Connection,
) -> io::Result<Vec<RelationshipRecord>> {
    let mut statement = connection
        .prepare(
            "
            SELECT char_id, peer_char_id, relationship_type, since_tick, metadata_json
            FROM relationships
            ORDER BY char_id ASC, peer_char_id ASC, relationship_type ASC
            ",
        )
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map([], |row| {
            Ok(RelationshipRecord {
                char_id: row.get(0)?,
                peer_char_id: row.get(1)?,
                relationship_type: row.get(2)?,
                since_tick: sql_to_tick(row.get::<_, i64>(3)?).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        3,
                        rusqlite::types::Type::Integer,
                        Box::new(error),
                    )
                })?,
                metadata_json: row.get(4)?,
            })
        })
        .map_err(io::Error::other)?;
    collect_rows(rows)
}

#[cfg_attr(not(test), allow(dead_code))]
fn collect_rows<T>(
    rows: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>>,
) -> io::Result<Vec<T>> {
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(io::Error::other)?);
    }
    Ok(out)
}

#[cfg_attr(not(test), allow(dead_code))]
type NpcStateSqlRow = (
    String,
    String,
    f64,
    f64,
    f64,
    String,
    String,
    String,
    i64,
    f64,
    f64,
    f64,
    String,
    i64,
    i64,
    i64,
    i64,
    String,
    i64,
    Option<i64>,
    Option<i64>,
);

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

fn build_npc_blackboard_snapshot(
    blackboard: &NpcBlackboard,
    nearest_player_id: Option<&str>,
) -> HashMap<String, serde_json::Value> {
    let mut snapshot = HashMap::new();
    if let Some(player_id) = nearest_player_id {
        snapshot.insert(
            "nearest_player".to_string(),
            serde_json::Value::String(player_id.to_string()),
        );
    }
    if blackboard.player_distance.is_finite() {
        snapshot.insert(
            "player_distance".to_string(),
            serde_json::Value::from(f64::from(blackboard.player_distance)),
        );
    }
    if let Some(target_position) = blackboard.target_position {
        snapshot.insert(
            "target_position".to_string(),
            serde_json::json!(vec3_to_array(target_position)),
        );
    }
    snapshot.insert(
        "last_melee_tick".to_string(),
        serde_json::Value::from(blackboard.last_melee_tick),
    );
    snapshot
}

fn vec3_to_array(position: DVec3) -> [f64; 3] {
    [position.x, position.y, position.z]
}

fn state_label(state: &NpcStateKind) -> &'static str {
    match state {
        NpcStateKind::Idle => "idle",
        NpcStateKind::Fleeing => "fleeing",
        NpcStateKind::Attacking => "attacking",
        NpcStateKind::Patrolling => "patrolling",
    }
}

fn lifecycle_state_label(state: &LifecycleState) -> &'static str {
    match state {
        LifecycleState::Alive => "alive",
        LifecycleState::NearDeath => "near_death",
        LifecycleState::AwaitingRevival => "awaiting_revival",
        LifecycleState::Terminated => "terminated",
    }
}

fn movement_mode_label(mode: &MovementMode) -> &'static str {
    match mode {
        MovementMode::GroundNav => "ground_nav",
        MovementMode::Sprinting(_) => "sprinting",
        MovementMode::Override(crate::npc::movement::ActiveOverride::Dash(_)) => "override_dash",
        MovementMode::Override(crate::npc::movement::ActiveOverride::Knockback(_)) => {
            "override_knockback"
        }
    }
}

fn npc_archetype_label(archetype: NpcMeleeArchetype) -> &'static str {
    match archetype {
        NpcMeleeArchetype::Brawler => "brawler",
        NpcMeleeArchetype::Sword => "sword",
        NpcMeleeArchetype::Spear => "spear",
    }
}

fn entity_kind_label(kind: EntityKind) -> String {
    let debug = format!("{kind:?}");
    if let Some((_, label)) = debug.split_once(' ') {
        label
            .trim_start_matches('(')
            .trim_end_matches(')')
            .to_string()
    } else {
        debug
    }
}

fn bool_to_sql(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}

#[cfg_attr(not(test), allow(dead_code))]
fn sql_to_bool(value: i64) -> bool {
    value != 0
}

#[cfg_attr(not(test), allow(dead_code))]
fn sql_to_tick(value: i64) -> io::Result<u64> {
    u64::try_from(value).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn optional_tick_to_sql(tick: Option<u64>) -> io::Result<Option<i64>> {
    tick.map(tick_to_sql).transpose()
}

#[cfg_attr(not(test), allow(dead_code))]
fn optional_sql_to_tick(value: Option<i64>) -> io::Result<Option<u64>> {
    value.map(sql_to_tick).transpose()
}

#[cfg_attr(not(test), allow(dead_code))]
fn sql_to_u32(value: i64) -> io::Result<u32> {
    u32::try_from(value).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn sql_to_u8(value: i64) -> io::Result<u8> {
    u8::try_from(value).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

#[cfg_attr(not(test), allow(dead_code))]
fn sql_to_usize(value: i64) -> io::Result<usize> {
    usize::try_from(value).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn sql_usize(value: usize) -> io::Result<i64> {
    i64::try_from(value).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn npc_deceased_archive_relative_path(char_id: &str, archived_at_wall: i64) -> String {
    format!(
        "data/archive/npc_deceased/{}/{}.json.zst",
        utc_year_from_unix_seconds(archived_at_wall),
        char_id
    )
}

fn npc_deceased_archive_absolute_path(
    settings: &PersistenceSettings,
    char_id: &str,
    archived_at_wall: i64,
) -> PathBuf {
    resolve_persistence_relative_path(
        settings,
        npc_deceased_archive_relative_path(char_id, archived_at_wall).as_str(),
    )
}

fn npc_digest_archive_relative_path(char_id: &str) -> String {
    format!("data/archive/npc_digests/{char_id}.json.zst")
}

fn npc_digest_archive_absolute_path(
    settings: &PersistenceSettings,
    char_id: &str,
    _archived_at_wall: i64,
) -> PathBuf {
    resolve_persistence_relative_path(settings, npc_digest_archive_relative_path(char_id).as_str())
}

fn resolve_persistence_relative_path(
    settings: &PersistenceSettings,
    relative_path: &str,
) -> PathBuf {
    let path = PathBuf::from(relative_path);
    if path.is_absolute() {
        return path;
    }

    let Some(data_dir) = settings.db_path().parent() else {
        return path;
    };
    if data_dir.file_name().is_some_and(|name| name == "data") {
        if let Some(root) = data_dir.parent() {
            return root.join(relative_path);
        }
        return path;
    }

    data_dir.join(relative_path)
}

fn write_zstd_bundle(path: &Path, payload: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let compressed = zstd::stream::encode_all(payload, 3).map_err(io::Error::other)?;
    fs::write(path, compressed)
}

#[cfg_attr(not(test), allow(dead_code))]
fn read_zstd_bundle(reference: &Path, relative_path: &str) -> io::Result<Vec<u8>> {
    let absolute_path = if Path::new(relative_path).is_absolute() {
        PathBuf::from(relative_path)
    } else {
        let settings = PersistenceSettings {
            db_path: reference.to_path_buf(),
            deceased_public_dir: PathBuf::new(),
            server_run_id: String::new(),
        };
        resolve_persistence_relative_path(&settings, relative_path)
    };
    let compressed = fs::read(absolute_path)?;
    zstd::stream::decode_all(compressed.as_slice()).map_err(io::Error::other)
}

fn utc_year_from_unix_seconds(unix_seconds: i64) -> i32 {
    let days = unix_seconds.div_euclid(86_400);
    civil_from_days(days).0
}

fn civil_from_days(days_since_unix_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_unix_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if m <= 2 { 1 } else { 0 };

    (year as i32, m as u32, d as u32)
}

fn scan_orphaned_npc_archives(settings: &PersistenceSettings) -> io::Result<()> {
    let connection = open_persistence_connection(settings)?;
    let mut statement = connection
        .prepare("SELECT path FROM npc_deceased_index")
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(io::Error::other)?;
    let mut indexed_paths = HashSet::new();
    for row in rows {
        indexed_paths.insert(row.map_err(io::Error::other)?);
    }

    let archive_root = resolve_persistence_relative_path(settings, "data/archive/npc_deceased");
    let archive_files = collect_files_with_suffix(&archive_root, ".json.zst")?;
    for archive_file in archive_files {
        let Ok(relative_path) = archive_file.strip_prefix(
            archive_root
                .parent()
                .and_then(|parent| parent.parent())
                .unwrap_or(archive_root.as_path()),
        ) else {
            continue;
        };
        let normalized = relative_path.to_string_lossy().replace('\\', "/");
        let normalized = if normalized.starts_with("data/") {
            normalized
        } else {
            format!("data/{normalized}")
        };
        if !indexed_paths.contains(&normalized) {
            tracing::warn!(
                "[bong][persistence] orphaned npc archive without sqlite index: {}",
                archive_file.display()
            );
        }
    }

    Ok(())
}

fn collect_files_with_suffix(root: &Path, suffix: &str) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    if !root.exists() {
        return Ok(files);
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            files.extend(collect_files_with_suffix(&path, suffix)?);
        } else if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(suffix))
        {
            files.push(path);
        }
    }
    Ok(files)
}

#[cfg(test)]
mod persistence_tests {
    use super::*;
    use crate::combat::components::LifecycleState;
    use crate::cultivation::components::{Cultivation, Realm};
    use crate::npc::movement::{MovementController, MovementCooldowns, MovementMode, SprintState};
    use crate::npc::patrol::NpcPatrol;
    use crate::npc::spawn::{NpcBlackboard, NpcCombatLoadout, NpcMeleeArchetype};
    use crate::player::state::{
        save_player_core_slice, save_player_state, PlayerState, PlayerStatePersistence,
    };
    use crate::schema::common::NpcStateKind;
    use crate::world::zone::DEFAULT_SPAWN_ZONE_NAME;
    use rusqlite::{params, OptionalExtension};
    use serde_json::Value;
    use std::sync::{Arc, Barrier};
    use valence::prelude::{App, DVec3, EntityKind, Position};

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
        let user_version: i32 =
            connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
        if user_version > max_supported_user_version {
            return Err(rusqlite::Error::ExecuteReturnedResults);
        }
        Ok(())
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
    fn startup_backup_creates_pre_bootstrap_snapshot_for_existing_db() {
        let (settings, root) = persistence_settings("startup-backup-pre-bootstrap");
        let wall_clock = 1_735_689_600;
        bootstrap_sqlite(settings.db_path(), "first-run").expect("first bootstrap should succeed");

        let backup_path = run_startup_backup(&settings, wall_clock)
            .expect("startup backup should succeed")
            .expect("existing db should produce a startup backup");
        bootstrap_sqlite(settings.db_path(), "second-run")
            .expect("second bootstrap should succeed");

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

        bootstrap_sqlite(settings.db_path(), "second-run")
            .expect("second bootstrap should succeed");

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
            last_tick: Some(4_200),
            last_state_ts: Some(1_704_067_200),
        };

        persist_agent_world_model_snapshot(&settings, &snapshot)
            .expect("agent world model snapshot should persist");
        let loaded = load_agent_world_model_snapshot(&settings)
            .expect("agent world model snapshot should load")
            .expect("agent world model snapshot should exist");

        assert_eq!(loaded, snapshot);

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

    #[test]
    fn active_tribulation_roundtrip_and_delete() {
        let (settings, root) = persistence_settings("tribulation-roundtrip");
        bootstrap_sqlite(settings.db_path(), settings.server_run_id())
            .expect("bootstrap should succeed");

        let record = ActiveTribulationRecord {
            char_id: "offline:Azure".to_string(),
            wave_current: 2,
            waves_total: 5,
            started_tick: 1440,
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
            wave_current: 4,
            waves_total: 5,
            started_tick: 2880,
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
    fn zones_runtime_roundtrip_persists_spirit_qi_and_danger_level() {
        let (settings, root) = persistence_settings("zones-runtime-roundtrip");
        bootstrap_sqlite(settings.db_path(), settings.server_run_id())
            .expect("bootstrap should succeed");

        let zones = crate::world::zone::ZoneRegistry {
            zones: vec![crate::world::zone::Zone {
                name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
                bounds: crate::world::zone::default_spawn_bounds(),
                spirit_qi: 0.42,
                danger_level: 3,
                active_events: vec!["beast_tide".to_string()],
                patrol_anchors: Vec::new(),
                blocked_tiles: Vec::new(),
            }],
        };

        persist_zone_runtime_snapshot(&settings, &zones)
            .expect("zone runtime snapshot should persist");
        let records =
            load_zone_runtime_snapshot(&settings).expect("zone runtime snapshot should load");
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
    fn zone_overlays_roundtrip_preserves_ordered_records() {
        let (settings, root) = persistence_settings("zone-overlays-roundtrip");
        bootstrap_sqlite(settings.db_path(), settings.server_run_id())
            .expect("bootstrap should succeed");

        let overlays = vec![
            ZoneOverlayRecord {
                zone_id: DEFAULT_SPAWN_ZONE_NAME.to_string(),
                overlay_kind: "collapsed".to_string(),
                payload_json: serde_json::json!({"danger_level": 3}).to_string(),
                payload_version: 1,
                since_wall: 10,
            },
            ZoneOverlayRecord {
                zone_id: "blood_valley".to_string(),
                overlay_kind: "ruins_discovered".to_string(),
                payload_json: serde_json::json!({"active_events": ["ruins_discovered"]})
                    .to_string(),
                payload_version: 1,
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
                    payload_version: 1,
                    since_wall: 20,
                },
                ZoneOverlayRecord {
                    zone_id: DEFAULT_SPAWN_ZONE_NAME.to_string(),
                    overlay_kind: "collapsed".to_string(),
                    payload_json: serde_json::json!({"danger_level": 3}).to_string(),
                    payload_version: 1,
                    since_wall: 10,
                },
            ]
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
                payload_version: 1,
                since_wall: 100,
            }],
        )
        .expect("zone overlays should persist");

        let mut registry = crate::world::zone::ZoneRegistry::fallback();
        hydrate_zone_overlays(&settings, &mut registry)
            .expect("zone overlay hydration should succeed");
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
                payload_version: 1,
                since_wall: 100,
            }],
        )
        .expect("invalid zone overlay payload row should still persist");

        let mut app = App::new();
        app.insert_resource(settings.clone());
        app.insert_resource(DailyBackupState::default());
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
            zones: vec![crate::world::zone::Zone {
                name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
                bounds: crate::world::zone::default_spawn_bounds(),
                spirit_qi: 0.31,
                danger_level: 2,
                active_events: Vec::new(),
                patrol_anchors: Vec::new(),
                blocked_tiles: Vec::new(),
            }],
        };
        persist_zone_runtime_snapshot(&settings, &zones)
            .expect("zone runtime snapshot should persist");
        persist_zone_overlays(
            &settings,
            &[ZoneOverlayRecord {
                zone_id: DEFAULT_SPAWN_ZONE_NAME.to_string(),
                overlay_kind: "collapsed".to_string(),
                payload_json: serde_json::json!({"danger_level": 4}).to_string(),
                payload_version: 1,
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
        assert_eq!(bundle.zone_overlays[0].payload_version, 1);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn import_zone_persistence_replaces_existing_zone_rows_atomically() {
        let (settings, root) = persistence_settings("zone-import-bundle");
        bootstrap_sqlite(settings.db_path(), settings.server_run_id())
            .expect("bootstrap should succeed");

        let existing_zones = crate::world::zone::ZoneRegistry {
            zones: vec![crate::world::zone::Zone {
                name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
                bounds: crate::world::zone::default_spawn_bounds(),
                spirit_qi: -0.55,
                danger_level: 5,
                active_events: Vec::new(),
                patrol_anchors: Vec::new(),
                blocked_tiles: Vec::new(),
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
                payload_version: 1,
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
                payload_json: serde_json::json!({"active_events": ["ruins_discovered"]})
                    .to_string(),
                payload_version: 1,
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
        bootstrap_sqlite(&db_path, "future-user-version-rejected")
            .expect("bootstrap should succeed");

        let connection = Connection::open(&db_path).expect("db should open");
        connection
            .execute_batch("PRAGMA user_version = 999;")
            .expect("user_version override should succeed");

        let error = bootstrap_sqlite(&db_path, "future-user-version-rejected")
            .expect_err("future user_version should be rejected");
        assert!(
            matches!(error, rusqlite::Error::ExecuteReturnedResults),
            "unexpected error when rejecting future user_version: {error:?}"
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
            zones: vec![crate::world::zone::Zone {
                name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
                bounds: crate::world::zone::default_spawn_bounds(),
                spirit_qi: -0.15,
                danger_level: 4,
                active_events: Vec::new(),
                patrol_anchors: Vec::new(),
                blocked_tiles: Vec::new(),
            }],
        };
        persist_zone_runtime_snapshot(&settings, &persisted)
            .expect("zone runtime snapshot should persist");

        let mut registry = crate::world::zone::ZoneRegistry::fallback();
        hydrate_zone_runtime(&settings, &mut registry)
            .expect("zone runtime hydration should succeed");
        assert_eq!(registry.zones[0].spirit_qi, -0.15);
        assert_eq!(registry.zones[0].danger_level, 4);

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
        app.insert_resource(crate::world::zone::ZoneRegistry {
            zones: vec![crate::world::zone::Zone {
                name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
                bounds: crate::world::zone::default_spawn_bounds(),
                spirit_qi: 0.25,
                danger_level: 1,
                active_events: Vec::new(),
                patrol_anchors: Vec::new(),
                blocked_tiles: Vec::new(),
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
        let second_records = load_zone_runtime_snapshot(&settings)
            .expect("second zone runtime snapshot should load");
        assert_eq!(second_records[0].spirit_qi, 0.25);
        assert_eq!(second_records[0].danger_level, 1);

        let _ = fs::remove_dir_all(root);
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

    #[test]
    fn persist_termination_transition_rewrites_existing_public_index_entry_for_same_char() {
        let (settings, root) = persistence_settings("deceased-export-rewrite");
        bootstrap_sqlite(settings.db_path(), settings.server_run_id())
            .expect("bootstrap should succeed");

        let first_life_record = LifeRecord {
            character_id: "offline:Ancestor".to_string(),
            created_at: 11,
            biography: vec![BiographyEntry::Terminated {
                cause: "fortune_exhausted".to_string(),
                tick: 77,
            }],
            insights_taken: Vec::new(),
            spirit_root_first: None,
        };
        let first_lifecycle = Lifecycle {
            character_id: first_life_record.character_id.clone(),
            death_count: 3,
            fortune_remaining: 0,
            last_death_tick: Some(77),
            last_revive_tick: Some(55),
            near_death_deadline_tick: None,
            weakened_until_tick: None,
            state: crate::combat::components::LifecycleState::Terminated,
        };
        persist_termination_transition(&settings, &first_lifecycle, &first_life_record)
            .expect("first terminated snapshot should persist");

        let second_life_record = LifeRecord {
            character_id: "offline:Ancestor".to_string(),
            created_at: 11,
            biography: vec![BiographyEntry::Terminated {
                cause: "tribulation_aftershock".to_string(),
                tick: 99,
            }],
            insights_taken: Vec::new(),
            spirit_root_first: None,
        };
        let second_lifecycle = Lifecycle {
            character_id: second_life_record.character_id.clone(),
            death_count: 4,
            fortune_remaining: 0,
            last_death_tick: Some(99),
            last_revive_tick: Some(55),
            near_death_deadline_tick: None,
            weakened_until_tick: None,
            state: crate::combat::components::LifecycleState::Terminated,
        };
        persist_termination_transition(&settings, &second_lifecycle, &second_life_record)
            .expect("second terminated snapshot should overwrite export");

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
        let (died_at_tick, public_path): (i64, String) = connection
            .query_row(
                "SELECT died_at_tick, public_path FROM deceased_snapshots WHERE char_id = ?1",
                params!["offline:Ancestor"],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("deceased snapshot row should exist");

        assert_eq!(snapshot.char_id, "offline:Ancestor");
        assert_eq!(snapshot.died_at_tick, 99);
        assert!(matches!(
            snapshot.life_record.biography.last(),
            Some(BiographyEntry::Terminated { tick: 99, .. })
        ));
        assert_eq!(index.len(), 1);
        assert_eq!(index[0].char_id, "offline:Ancestor");
        assert_eq!(index[0].died_at_tick, 99);
        assert_eq!(index[0].path, "deceased/offline:Ancestor.json");
        assert_eq!(died_at_tick, 99);
        assert_eq!(public_path, "deceased/offline:Ancestor.json");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn persist_termination_transition_sorts_public_index_by_died_at_tick_then_char_id() {
        let (settings, root) = persistence_settings("deceased-export-index-ordering");
        bootstrap_sqlite(settings.db_path(), settings.server_run_id())
            .expect("bootstrap should succeed");

        let exports = [
            ("offline:Crimson", 90_i64),
            ("offline:Azure", 90_i64),
            ("offline:Bronze", 77_i64),
        ];

        for (char_id, died_at_tick) in exports {
            let life_record = LifeRecord {
                character_id: char_id.to_string(),
                created_at: 11,
                biography: vec![BiographyEntry::Terminated {
                    cause: "fortune_exhausted".to_string(),
                    tick: died_at_tick as u64,
                }],
                insights_taken: Vec::new(),
                spirit_root_first: None,
            };
            let lifecycle = Lifecycle {
                character_id: life_record.character_id.clone(),
                death_count: 1,
                fortune_remaining: 0,
                last_death_tick: Some(died_at_tick as u64),
                last_revive_tick: None,
                near_death_deadline_tick: None,
                weakened_until_tick: None,
                state: crate::combat::components::LifecycleState::Terminated,
            };

            persist_termination_transition(&settings, &lifecycle, &life_record)
                .expect("terminated snapshot should persist");
        }

        let index_path = settings.deceased_public_dir().join("_index.json");
        let index: Vec<DeceasedIndexEntry> = serde_json::from_str(
            &fs::read_to_string(&index_path).expect("index json should exist"),
        )
        .expect("index json should deserialize");

        assert_eq!(index.len(), 3);
        assert_eq!(index[0].char_id, "offline:Bronze");
        assert_eq!(index[0].died_at_tick, 77);
        assert_eq!(index[0].path, "deceased/offline:Bronze.json");

        assert_eq!(index[1].char_id, "offline:Azure");
        assert_eq!(index[1].died_at_tick, 90);
        assert_eq!(index[1].path, "deceased/offline:Azure.json");

        assert_eq!(index[2].char_id, "offline:Crimson");
        assert_eq!(index[2].died_at_tick, 90);
        assert_eq!(index[2].path, "deceased/offline:Crimson.json");

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
            spirit_root_first: None,
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
                near_death_deadline_tick: None,
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
                        spirit_root_first: None,
                    };
                    let lifecycle = Lifecycle {
                        character_id: char_id.clone(),
                        death_count: 1,
                        fortune_remaining: 1,
                        last_death_tick: Some(tick),
                        last_revive_tick: Some(tick.saturating_sub(1)),
                        near_death_deadline_tick: Some(tick + 30),
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

        let player_persistence = PlayerStatePersistence::with_db_path(
            root.join("data").join("players"),
            settings.db_path(),
        );
        let player_seed = PlayerState {
            realm: "qi_refining_1".to_string(),
            spirit_qi: 12.0,
            spirit_qi_max: 100.0,
            karma: 0.1,
            experience: 640,
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
                        realm: "qi_refining_3".to_string(),
                        spirit_qi: 25.0 + index as f64,
                        spirit_qi_max: 160.0,
                        karma: ((index as f64 / 5.0) - 1.0).clamp(-1.0, 1.0),
                        experience: 3_000 + index as u64,
                        inventory_score: (index as f64 / player_writer_count as f64)
                            .clamp(0.0, 1.0),
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
                        spirit_root_first: None,
                    };
                    let lifecycle = Lifecycle {
                        character_id: char_id.clone(),
                        death_count: 1,
                        fortune_remaining: 1,
                        last_death_tick: Some(tick),
                        last_revive_tick: Some(tick.saturating_sub(1)),
                        near_death_deadline_tick: Some(tick + 30),
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
            let (spirit_qi, karma, inventory_score): (f64, f64, f64) = connection
                .query_row(
                    "SELECT spirit_qi, karma, inventory_score FROM player_core WHERE username = ?1",
                    params![username.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .expect("player core row should exist after mixed load");
            assert_eq!(spirit_qi, 25.0 + index as f64);
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

        let player_persistence = PlayerStatePersistence::with_db_path(
            root.join("data").join("players"),
            settings.db_path(),
        );
        let player_seed = PlayerState {
            realm: "qi_refining_1".to_string(),
            spirit_qi: 12.0,
            spirit_qi_max: 100.0,
            karma: 0.1,
            experience: 640,
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
                        realm: "qi_refining_3".to_string(),
                        spirit_qi: 35.0 + index as f64,
                        spirit_qi_max: 180.0,
                        karma: ((index as f64 / 5.0) - 1.0).clamp(-1.0, 1.0),
                        experience: 5_000 + index as u64,
                        inventory_score: (index as f64 / player_writer_count as f64)
                            .clamp(0.0, 1.0),
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
                        spirit_root_first: None,
                    };
                    let lifecycle = Lifecycle {
                        character_id: char_id.clone(),
                        death_count: 1,
                        fortune_remaining: 1,
                        last_death_tick: Some(tick),
                        last_revive_tick: Some(tick.saturating_sub(1)),
                        near_death_deadline_tick: Some(tick + 30),
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
            let (spirit_qi, karma, inventory_score): (f64, f64, f64) = connection
                .query_row(
                    "SELECT spirit_qi, karma, inventory_score FROM player_core WHERE username = ?1",
                    params![username.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .expect("player core row should exist after mixed npc load");
            assert_eq!(spirit_qi, 35.0 + index as f64);
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

        let player_persistence = PlayerStatePersistence::with_db_path(
            root.join("data").join("players"),
            settings.db_path(),
        );
        let player_seed = PlayerState {
            realm: "qi_refining_1".to_string(),
            spirit_qi: 12.0,
            spirit_qi_max: 100.0,
            karma: 0.1,
            experience: 640,
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
                        realm: "qi_refining_3".to_string(),
                        spirit_qi: 45.0 + index as f64,
                        spirit_qi_max: 200.0,
                        karma: ((index as f64 / 5.0) - 1.0).clamp(-1.0, 1.0),
                        experience: 7_000 + index as u64,
                        inventory_score: (index as f64 / player_writer_count as f64)
                            .clamp(0.0, 1.0),
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
                        spirit_root_first: None,
                    };
                    let lifecycle = Lifecycle {
                        character_id: char_id.clone(),
                        death_count: 1,
                        fortune_remaining: 1,
                        last_death_tick: Some(tick),
                        last_revive_tick: Some(tick.saturating_sub(1)),
                        near_death_deadline_tick: Some(tick + 30),
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
                        zones: vec![crate::world::zone::Zone {
                            name: format!("mixed_zone_{index}"),
                            bounds: crate::world::zone::default_spawn_bounds(),
                            spirit_qi: 0.1 + index as f64,
                            danger_level: 1 + index as u8,
                            active_events: Vec::new(),
                            patrol_anchors: Vec::new(),
                            blocked_tiles: Vec::new(),
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
            let (spirit_qi, karma, inventory_score): (f64, f64, f64) = connection
                .query_row(
                    "SELECT spirit_qi, karma, inventory_score FROM player_core WHERE username = ?1",
                    params![username.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .expect("player core row should exist after mixed zone load");
            assert_eq!(spirit_qi, 45.0 + index as f64);
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

        let player_persistence = PlayerStatePersistence::with_db_path(
            root.join("data").join("players"),
            settings.db_path(),
        );
        let player_seed = PlayerState {
            realm: "qi_refining_1".to_string(),
            spirit_qi: 12.0,
            spirit_qi_max: 100.0,
            karma: 0.1,
            experience: 640,
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
                player_writer_count
                    + semantic_writer_count
                    + npc_writer_count
                    + zone_writer_count
                    + 1,
            ));

            let player_handles = (0..player_writer_count)
                .map(|index| {
                    let persistence = Arc::clone(&player_persistence);
                    let barrier = Arc::clone(&barrier);
                    std::thread::spawn(move || {
                        let username = format!("BatchPlayer{index}");
                        let updated_state = PlayerState {
                            realm: "qi_refining_3".to_string(),
                            spirit_qi: 10.0 * batch as f64 + index as f64,
                            spirit_qi_max: 220.0,
                            karma: (0.1 * batch as f64).clamp(-1.0, 1.0),
                            experience: 10_000 + (batch as u64 * 100) + index as u64,
                            inventory_score: (0.01 * ((batch * 10 + index) as f64)).clamp(0.0, 1.0),
                        };

                        barrier.wait();
                        save_player_core_slice(
                            persistence.as_ref(),
                            username.as_str(),
                            &updated_state,
                        )
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
                            spirit_root_first: None,
                        };
                        let lifecycle = Lifecycle {
                            character_id: char_id.clone(),
                            death_count: 1,
                            fortune_remaining: 1,
                            last_death_tick: Some(tick),
                            last_revive_tick: Some(tick.saturating_sub(1)),
                            near_death_deadline_tick: Some(tick + 30),
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
                        let capture =
                            sample_npc_capture(format!("npc_batch_{batch}_{index}").as_str());
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
                            zones: vec![crate::world::zone::Zone {
                                name: format!("mixed_zone_{batch}_{index}"),
                                bounds: crate::world::zone::default_spawn_bounds(),
                                spirit_qi: 0.1 + batch as f64 + index as f64,
                                danger_level: 1 + batch as u8 + index as u8,
                                active_events: Vec::new(),
                                patrol_anchors: Vec::new(),
                                blocked_tiles: Vec::new(),
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
            let (spirit_qi, karma, inventory_score): (f64, f64, f64) = connection
                .query_row(
                    "SELECT spirit_qi, karma, inventory_score FROM player_core WHERE username = ?1",
                    params![username.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .expect("player core row should exist after multi-batch load");
            assert_eq!(spirit_qi, 10.0 * final_batch as f64 + index as f64);
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
        persist_npc_capture(&settings, &capture)
            .expect("npc capture should persist before archive");

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
                .exists(),
            "stale digest should be written to cold archive"
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
}
