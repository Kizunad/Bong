use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use big_brain::prelude::{ActionState, Actor};
use rusqlite::{params, types::Type, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use uuid::Uuid;
use valence::prelude::bevy_ecs;
use valence::prelude::bevy_ecs::schedule::SystemSet;
use valence::prelude::{
    App, Client, Commands, Component, DVec3, Entity, EntityKind, IntoSystemConfigs, Position,
    Query, Res, ResMut, Resource, Startup, Update, Username, With,
};

use crate::combat::components::{Lifecycle, LifecycleState};
use crate::cultivation::components::{Cultivation, Realm};
use crate::cultivation::life_record::{BiographyEntry, DeathInsightRecord, LifeRecord};
use crate::cultivation::void::components::{VoidActionCooldowns, VoidActionKind};
use crate::npc::brain::{canonical_npc_id, ChaseAction, DashAction, FleeAction, MeleeAttackAction};
use crate::npc::movement::{MovementController, MovementCooldowns, MovementMode};
use crate::npc::patrol::NpcPatrol;
use crate::npc::spawn::{NpcBlackboard, NpcCombatLoadout, NpcMarker, NpcMeleeArchetype};
use crate::player::state::canonical_player_id;
use crate::schema::common::NpcStateKind;
use crate::schema::social::{
    ExposureKindV1, FactionMembershipSnapshotV1, RelationshipKindV1, RelationshipSnapshotV1,
    RenownTagV1,
};

#[allow(dead_code)]
pub mod identity;

pub const DEFAULT_DATABASE_PATH: &str = "data/bong.db";
pub const SQLITE_BUSY_TIMEOUT_MS: u64 = 15_000;
const DEFAULT_DECEASED_PUBLIC_DIR: &str = "../library-web/public/deceased";
const CURRENT_USER_VERSION: i32 = 22;
const AGENT_WORLD_MODEL_ROW_ID: i64 = 1;
const ASCENSION_QUOTA_ROW_ID: i64 = 1;
const TRIBULATION_KIND_DU_XU: &str = "du_xu";
const TRIBULATION_KIND_JUE_BI: &str = "jue_bi";
const JUEBI_SOURCE_VOID_QUOTA_EXCEEDED: &str = "void_quota_exceeded";
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
pub const ZONE_OVERLAY_PAYLOAD_VERSION: i32 = 2;
const NPC_ROW_SCHEMA_VERSION: i32 = 1;
const NPC_DIGEST_RETENTION_SECS: i64 = 180 * 24 * 60 * 60;
const NPC_DIGEST_SWEEP_INTERVAL_SECS: i64 = 7 * 24 * 60 * 60;
const AGENT_WORLD_MODEL_APPEND_ONLY_RETENTION_SECS: i64 = 180 * 24 * 60 * 60;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub(crate) struct PersistenceBootstrapSet;

#[derive(Debug, Default, Component)]
struct NpcArchivedPersistence;

#[derive(Debug, Default, Component)]
struct NpcLivePersistenceSnapshot;

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
    #[serde(default = "default_termination_category")]
    pub termination_category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeceasedSnapshot {
    pub char_id: String,
    pub died_at_tick: u64,
    #[serde(default = "default_termination_category")]
    pub termination_category: String,
    pub lifecycle: Lifecycle,
    pub life_record: LifeRecord,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub social: Option<DeceasedSocialSnapshot>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeceasedSocialSnapshot {
    #[serde(default)]
    pub renown: DeceasedRenownSnapshot,
    #[serde(default)]
    pub relationships: Vec<RelationshipSnapshotV1>,
    #[serde(default)]
    pub exposure_log: Vec<DeceasedExposureSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub faction_membership: Option<FactionMembershipSnapshotV1>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeceasedRenownSnapshot {
    pub fame: i32,
    pub notoriety: i32,
    #[serde(default)]
    pub tags: Vec<RenownTagV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeceasedExposureSnapshot {
    pub tick: u64,
    pub kind: ExposureKindV1,
    #[serde(default)]
    pub witnesses: Vec<String>,
}

type DeceasedFactionMembershipSqlRow = (Option<String>, i64, i64, i64, Option<i64>, i64);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeathInsightEventPayload {
    death_insight: DeathInsightRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LifeEventPayload {
    biography_entry: BiographyEntry,
}

#[derive(Debug)]
struct StagedDeceasedExport {
    snapshot_path: PathBuf,
    index_path: PathBuf,
    previous_snapshot: Option<Vec<u8>>,
    previous_index: Option<Vec<u8>>,
    relative_snapshot_path: String,
    _guard: MutexGuard<'static, ()>,
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
pub struct AgentEraRecord {
    pub event_id: String,
    pub envelope_id: String,
    pub source: String,
    pub era_name: String,
    pub since_tick: i64,
    pub global_effect: String,
    pub observed_at_tick: Option<i64>,
    pub observed_at_wall: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentDecisionRecord {
    pub event_id: String,
    pub envelope_id: String,
    pub source: String,
    pub agent_name: String,
    pub reasoning: String,
    pub command_count: u32,
    pub narration_count: u32,
    pub payload_json: String,
    pub observed_at_tick: Option<i64>,
    pub observed_at_wall: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActiveTribulationRecord {
    pub char_id: String,
    pub kind: String,
    pub source: String,
    pub origin_dimension: Option<String>,
    pub wave_current: u32,
    pub waves_total: u32,
    pub started_tick: u64,
    pub epicenter: [f64; 3],
    pub intensity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AscensionQuotaRecord {
    pub occupied_slots: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AscensionQuotaRelease {
    pub quota: AscensionQuotaRecord,
    pub opened_slot: bool,
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

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SocialAnonymityRecord {
    pub char_id: String,
    pub displayed_name: Option<String>,
    pub exposed_to_json: String,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SocialExposureRecord {
    pub event_id: String,
    pub char_id: String,
    pub kind: String,
    pub witnesses_json: String,
    pub at_tick: u64,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SocialRenownRecord {
    pub char_id: String,
    pub fame: i32,
    pub notoriety: i32,
    pub tags_json: String,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SocialSpiritNicheRecord {
    pub owner: String,
    pub pos: [i32; 3],
    pub placed_at_tick: u64,
    pub revealed: bool,
    pub revealed_by: Option<String>,
    pub guardians_json: String,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SocialPersistenceBundle {
    pub anonymity: Vec<SocialAnonymityRecord>,
    pub relationships: Vec<RelationshipRecord>,
    pub exposures: Vec<SocialExposureRecord>,
    pub renown: Vec<SocialRenownRecord>,
    pub spirit_niches: Vec<SocialSpiritNicheRecord>,
}

pub fn register(app: &mut App) {
    app.init_resource::<PersistenceSettings>()
        .init_resource::<NpcSnapshotTracker>()
        .init_resource::<NpcDigestSweepState>()
        .init_resource::<DailyBackupState>()
        .init_resource::<ZoneRuntimeSnapshotState>()
        .add_systems(
            Startup,
            bootstrap_persistence_system.in_set(PersistenceBootstrapSet),
        )
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
    mut void_action_cooldowns: Option<ResMut<VoidActionCooldowns>>,
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

    if let Some(cooldowns) = void_action_cooldowns.as_deref_mut() {
        match hydrate_void_action_cooldowns(&settings, cooldowns) {
            Ok(count) if count > 0 => tracing::info!(
                "[bong][persistence] hydrated {count} void-action cooldown(s) from sqlite"
            ),
            Ok(_) => {}
            Err(error) => panic!(
                "[bong][persistence] failed to hydrate void-action cooldowns at {}: {error}",
                settings.db_path().display()
            ),
        }
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
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            io::Error::other(format!(
                "sqlite journal_mode must be WAL, got `{journal_mode}`"
            )),
        )));
    }

    connection.execute_batch("PRAGMA foreign_keys = ON;")?;
    connection.busy_timeout(Duration::from_millis(SQLITE_BUSY_TIMEOUT_MS))?;
    Ok(())
}

fn run_integrity_check(connection: &Connection) -> rusqlite::Result<()> {
    let integrity: String =
        connection.query_row("PRAGMA integrity_check;", [], |row| row.get(0))?;
    if integrity != "ok" {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            io::Error::other(format!("sqlite integrity_check returned `{integrity}`")),
        )));
    }
    Ok(())
}

fn apply_migrations(connection: &mut Connection) -> rusqlite::Result<()> {
    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version > CURRENT_USER_VERSION {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            io::Error::other(format!(
                "sqlite user_version {current_version} is newer than supported {CURRENT_USER_VERSION}; refusing to open without modifying database"
            )),
        )));
    }

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

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 11 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS agent_eras (
                event_id TEXT PRIMARY KEY,
                envelope_id TEXT NOT NULL,
                source TEXT NOT NULL,
                era_name TEXT NOT NULL,
                since_tick INTEGER NOT NULL,
                global_effect TEXT NOT NULL,
                observed_at_tick INTEGER,
                observed_at_wall INTEGER NOT NULL CHECK (observed_at_wall >= 0),
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1)
            );
            CREATE INDEX IF NOT EXISTS idx_agent_eras_envelope_id
            ON agent_eras (envelope_id, observed_at_wall, event_id);
            CREATE TABLE IF NOT EXISTS agent_decisions (
                event_id TEXT PRIMARY KEY,
                envelope_id TEXT NOT NULL,
                source TEXT NOT NULL,
                agent_name TEXT NOT NULL,
                reasoning TEXT NOT NULL,
                command_count INTEGER NOT NULL CHECK (command_count >= 0),
                narration_count INTEGER NOT NULL CHECK (narration_count >= 0),
                payload_json TEXT NOT NULL,
                observed_at_tick INTEGER,
                observed_at_wall INTEGER NOT NULL CHECK (observed_at_wall >= 0),
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1)
            );
            CREATE INDEX IF NOT EXISTS idx_agent_decisions_envelope_agent
            ON agent_decisions (envelope_id, agent_name, observed_at_wall, event_id);
            PRAGMA user_version = 11;
            ",
        )?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 12 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS player_lifespan (
                username TEXT PRIMARY KEY,
                born_at_tick INTEGER NOT NULL CHECK (born_at_tick >= 0),
                years_lived REAL NOT NULL CHECK (years_lived >= 0),
                cap_by_realm INTEGER NOT NULL CHECK (cap_by_realm > 0),
                offline_pause_wall INTEGER NOT NULL CHECK (offline_pause_wall >= 0),
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            CREATE TABLE IF NOT EXISTS player_skills (
                username TEXT PRIMARY KEY,
                skill_set_json TEXT NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            PRAGMA user_version = 12;
            ",
        )?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 13 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS player_shrine (
                username TEXT PRIMARY KEY,
                anchor_x REAL NOT NULL,
                anchor_y REAL NOT NULL,
                anchor_z REAL NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            ",
        )?;
        let has_column: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('player_slow') WHERE name = 'last_dimension'",
            [],
            |row| row.get(0),
        )?;
        if has_column == 0 {
            transaction.execute_batch(
                "
                ALTER TABLE player_slow
                ADD COLUMN last_dimension TEXT NOT NULL DEFAULT 'overworld'
                CHECK (last_dimension IN ('overworld', 'tsy'));
                ",
            )?;
        }
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS player_cultivation (
                username TEXT PRIMARY KEY,
                cultivation_json TEXT NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            ",
        )?;

        // SQLite might already have a pruned player_core schema (e.g. older
        // dev databases). Drop columns only when they exist.
        let player_core_columns: Vec<String> = {
            let mut stmt = transaction.prepare("PRAGMA table_info(player_core)")?;
            let columns = stmt
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<Result<Vec<_>, _>>()?;
            columns
        };
        backfill_legacy_player_cultivation(&transaction, &player_core_columns)?;
        for legacy_col in ["realm", "spirit_qi", "spirit_qi_max", "experience"] {
            if player_core_columns.iter().any(|name| name == legacy_col) {
                transaction.execute(
                    &format!("ALTER TABLE player_core DROP COLUMN {legacy_col}"),
                    [],
                )?;
            }
        }

        transaction.execute_batch("PRAGMA user_version = 13;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 14 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS social_anonymity (
                char_id TEXT PRIMARY KEY,
                displayed_name TEXT,
                exposed_to_json TEXT NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            CREATE TABLE IF NOT EXISTS social_relationships (
                char_id TEXT NOT NULL,
                peer_char_id TEXT NOT NULL,
                relationship_type TEXT NOT NULL,
                since_tick INTEGER NOT NULL CHECK (since_tick >= 0),
                metadata_json TEXT NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0),
                PRIMARY KEY (char_id, peer_char_id, relationship_type)
            );
            CREATE TABLE IF NOT EXISTS social_exposures (
                event_id TEXT PRIMARY KEY,
                char_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                witnesses_json TEXT NOT NULL,
                at_tick INTEGER NOT NULL CHECK (at_tick >= 0),
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            CREATE INDEX IF NOT EXISTS idx_social_exposures_char_tick
            ON social_exposures (char_id, at_tick, event_id);
            CREATE TABLE IF NOT EXISTS social_renown (
                char_id TEXT PRIMARY KEY,
                fame INTEGER NOT NULL,
                notoriety INTEGER NOT NULL,
                tags_json TEXT NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            CREATE TABLE IF NOT EXISTS social_spirit_niches (
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
            PRAGMA user_version = 14;
            ",
        )?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 15 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS social_faction_memberships (
                char_id TEXT PRIMARY KEY,
                faction TEXT,
                rank INTEGER NOT NULL CHECK (rank >= 0),
                loyalty INTEGER NOT NULL,
                betrayal_count INTEGER NOT NULL CHECK (betrayal_count >= 0),
                invite_block_until_tick INTEGER,
                permanently_refused INTEGER NOT NULL CHECK (permanently_refused IN (0, 1)),
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            PRAGMA user_version = 15;
            ",
        )?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 16 {
        let transaction = connection.transaction()?;
        let columns = table_columns(&transaction, "social_spirit_niches")?;
        if !columns.iter().any(|column| column == "guardians_json") {
            transaction.execute_batch(
                "
                ALTER TABLE social_spirit_niches
                ADD COLUMN guardians_json TEXT NOT NULL DEFAULT '[]';
                ",
            )?;
        }
        transaction.execute_batch("PRAGMA user_version = 16;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 17 {
        let transaction = connection.transaction()?;
        identity::migrate_v17(&transaction)?;
        // 防 user_version 升级但表不存在的 silent regression：在 PRAGMA 前显式 assert
        let table_exists: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'player_identities'",
            [],
            |row| row.get(0),
        )?;
        if table_exists != 1 {
            return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                io::Error::other("v17 migration completed but player_identities table missing"),
            )));
        }
        transaction.execute_batch("PRAGMA user_version = 17;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 18 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS legacy_letterbox (
                owner_id TEXT PRIMARY KEY,
                inheritor_id TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                assigned_at_tick INTEGER NOT NULL CHECK (assigned_at_tick >= 0),
                reject_until_tick INTEGER NOT NULL CHECK (reject_until_tick >= 0),
                status TEXT NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            CREATE INDEX IF NOT EXISTS idx_legacy_letterbox_inheritor
            ON legacy_letterbox (inheritor_id, status);
            ",
        )?;
        assert_legacy_letterbox_schema_ready(&transaction)?;
        transaction.execute_batch("PRAGMA user_version = 18;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 19 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS void_action_cooldowns (
                character_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                ready_at_tick INTEGER NOT NULL CHECK (ready_at_tick >= 0),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0),
                PRIMARY KEY (character_id, kind)
            );
            ",
        )?;
        assert_void_action_cooldowns_schema_ready(&transaction)?;
        transaction.execute_batch("PRAGMA user_version = 19;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 20 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS high_renown_milestones (
                player_uuid TEXT NOT NULL,
                char_id TEXT NOT NULL,
                identity_id INTEGER NOT NULL CHECK (identity_id >= 0),
                milestone INTEGER NOT NULL CHECK (milestone >= 0),
                emitted_at_tick INTEGER NOT NULL CHECK (emitted_at_tick >= 0),
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0),
                PRIMARY KEY (player_uuid, identity_id, milestone)
            );
            CREATE INDEX IF NOT EXISTS idx_high_renown_milestones_char
            ON high_renown_milestones (char_id, identity_id, milestone);
            ",
        )?;
        assert_high_renown_milestones_schema_ready(&transaction)?;
        transaction.execute_batch("PRAGMA user_version = 20;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 21 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS tribulations_active (
                char_id TEXT PRIMARY KEY,
                kind TEXT NOT NULL DEFAULT 'du_xu',
                source TEXT NOT NULL DEFAULT '',
                wave_current INTEGER NOT NULL CHECK (wave_current >= 0),
                waves_total INTEGER NOT NULL CHECK (waves_total > 0),
                started_tick INTEGER NOT NULL CHECK (started_tick >= 0),
                epicenter_x REAL NOT NULL DEFAULT 0.0,
                epicenter_y REAL NOT NULL DEFAULT 64.0,
                epicenter_z REAL NOT NULL DEFAULT 0.0,
                intensity REAL NOT NULL DEFAULT 0.0,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            ",
        )?;
        let columns = table_columns(&transaction, "tribulations_active")?;
        if !columns.iter().any(|column| column == "kind") {
            transaction.execute_batch(
                "
                ALTER TABLE tribulations_active
                ADD COLUMN kind TEXT NOT NULL DEFAULT 'du_xu';
                ",
            )?;
        }
        if !columns.iter().any(|column| column == "source") {
            transaction.execute_batch(
                "
                ALTER TABLE tribulations_active
                ADD COLUMN source TEXT NOT NULL DEFAULT '';
                ",
            )?;
        }
        if !columns.iter().any(|column| column == "epicenter_x") {
            transaction.execute_batch(
                "
                ALTER TABLE tribulations_active
                ADD COLUMN epicenter_x REAL NOT NULL DEFAULT 0.0;
                ",
            )?;
        }
        if !columns.iter().any(|column| column == "epicenter_y") {
            transaction.execute_batch(
                "
                ALTER TABLE tribulations_active
                ADD COLUMN epicenter_y REAL NOT NULL DEFAULT 64.0;
                ",
            )?;
        }
        if !columns.iter().any(|column| column == "epicenter_z") {
            transaction.execute_batch(
                "
                ALTER TABLE tribulations_active
                ADD COLUMN epicenter_z REAL NOT NULL DEFAULT 0.0;
                ",
            )?;
        }
        if !columns.iter().any(|column| column == "intensity") {
            transaction.execute_batch(
                "
                ALTER TABLE tribulations_active
                ADD COLUMN intensity REAL NOT NULL DEFAULT 0.0;
                ",
            )?;
        }
        transaction.execute_batch("PRAGMA user_version = 21;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 22 {
        let transaction = connection.transaction()?;
        let columns = table_columns(&transaction, "tribulations_active")?;
        if !columns.iter().any(|column| column == "origin_dimension") {
            transaction.execute_batch(
                "
                ALTER TABLE tribulations_active
                ADD COLUMN origin_dimension TEXT;
                ",
            )?;
        }
        transaction.execute_batch("PRAGMA user_version = 22;")?;
        transaction.commit()?;
    }

    let final_version: i32 = connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if final_version != CURRENT_USER_VERSION {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            io::Error::other(format!(
                "sqlite user_version mismatch after migrations: expected {}, got {}",
                CURRENT_USER_VERSION, final_version
            )),
        )));
    }

    Ok(())
}

fn backfill_legacy_player_cultivation(
    transaction: &rusqlite::Transaction<'_>,
    player_core_columns: &[String],
) -> rusqlite::Result<()> {
    let has_column = |column: &str| player_core_columns.iter().any(|name| name == column);
    if !(has_column("username")
        && has_column("realm")
        && has_column("spirit_qi")
        && has_column("spirit_qi_max"))
    {
        return Ok(());
    }

    let legacy_rows = {
        let mut stmt = transaction.prepare(
            "
            SELECT username, realm, spirit_qi, spirit_qi_max
            FROM player_core
            ",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, f64>(2)?,
                row.get::<_, f64>(3)?,
            ))
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    let wall_clock = current_unix_seconds();
    for (username, realm, spirit_qi, spirit_qi_max) in legacy_rows {
        let mut cultivation = Cultivation::default();
        if let Some(restored_realm) = legacy_player_realm_to_cultivation(realm.as_str()) {
            cultivation.realm = restored_realm;
        }
        if spirit_qi.is_finite() {
            cultivation.qi_current = spirit_qi.max(0.0);
        }
        if spirit_qi_max.is_finite() && spirit_qi_max > 0.0 {
            cultivation.qi_max = spirit_qi_max;
        }

        let bundle = serde_json::json!({
            "v": 1,
            "cultivation": cultivation,
            "meridians": crate::cultivation::components::MeridianSystem::default(),
            "qi_color": crate::cultivation::components::QiColor::default(),
            "karma": crate::cultivation::components::Karma::default(),
            "contamination": crate::cultivation::components::Contamination::default(),
            "life_record": crate::cultivation::life_record::LifeRecord::new(
                canonical_player_id(username.as_str()),
            ),
            "practice_log": crate::cultivation::color::PracticeLog::default(),
            "insight_quota": crate::cultivation::insight::InsightQuota::default(),
            "unlocked_perceptions": crate::cultivation::insight_apply::UnlockedPerceptions::default(),
            "insight_modifiers": crate::cultivation::insight_apply::InsightModifiers::new(),
        });
        let cultivation_json = serde_json::to_string(&bundle)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;

        transaction.execute(
            "
            INSERT INTO player_cultivation (
                username,
                cultivation_json,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(username) DO NOTHING
            ",
            params![
                username,
                cultivation_json,
                CURRENT_SCHEMA_VERSION,
                wall_clock,
            ],
        )?;
    }

    Ok(())
}

fn table_columns(
    transaction: &rusqlite::Transaction<'_>,
    table: &str,
) -> rusqlite::Result<Vec<String>> {
    let mut statement = transaction.prepare(&format!("PRAGMA table_info({table})"))?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn assert_legacy_letterbox_schema_ready(
    transaction: &rusqlite::Transaction<'_>,
) -> rusqlite::Result<()> {
    let columns = table_columns(transaction, "legacy_letterbox")?;
    let required = [
        "owner_id",
        "inheritor_id",
        "payload_json",
        "assigned_at_tick",
        "reject_until_tick",
        "status",
        "schema_version",
        "last_updated_wall",
    ];
    if let Some(missing) = required
        .iter()
        .find(|column| !columns.iter().any(|name| name == **column))
    {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            io::Error::other(format!(
                "v18 migration completed but legacy_letterbox column {missing} missing"
            )),
        )));
    }
    let index_exists: i64 = transaction.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_legacy_letterbox_inheritor'",
        [],
        |row| row.get(0),
    )?;
    if index_exists != 1 {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            io::Error::other("v18 migration completed but legacy_letterbox index missing"),
        )));
    }
    Ok(())
}

fn assert_void_action_cooldowns_schema_ready(
    transaction: &rusqlite::Transaction<'_>,
) -> rusqlite::Result<()> {
    let columns = table_columns(transaction, "void_action_cooldowns")?;
    let required = ["character_id", "kind", "ready_at_tick", "last_updated_wall"];
    if let Some(missing) = required
        .iter()
        .find(|column| !columns.iter().any(|name| name == **column))
    {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            io::Error::other(format!(
                "v19 migration completed but void_action_cooldowns column {missing} missing"
            )),
        )));
    }
    let mut statement = transaction.prepare("PRAGMA table_info(void_action_cooldowns)")?;
    let primary_key = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, i32>(5)?))
        })?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|(_, pk_ordinal)| *pk_ordinal > 0)
        .collect::<Vec<_>>();
    let expected_primary_key = [("character_id".to_owned(), 1), ("kind".to_owned(), 2)];
    if primary_key.as_slice() != expected_primary_key.as_slice() {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            io::Error::other(format!(
                "v19 migration completed but void_action_cooldowns primary key mismatch: expected character_id,kind got {primary_key:?}"
            )),
        )));
    }
    Ok(())
}

fn assert_high_renown_milestones_schema_ready(
    transaction: &rusqlite::Transaction<'_>,
) -> rusqlite::Result<()> {
    let columns = table_columns(transaction, "high_renown_milestones")?;
    let required = [
        "player_uuid",
        "char_id",
        "identity_id",
        "milestone",
        "emitted_at_tick",
        "schema_version",
        "last_updated_wall",
    ];
    if let Some(missing) = required
        .iter()
        .find(|column| !columns.iter().any(|name| name == **column))
    {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            io::Error::other(format!(
                "v20 migration completed but high_renown_milestones column {missing} missing"
            )),
        )));
    }
    let index_exists: i64 = transaction.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_high_renown_milestones_char'",
        [],
        |row| row.get(0),
    )?;
    if index_exists != 1 {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            io::Error::other("v20 migration completed but high_renown_milestones index missing"),
        )));
    }
    let mut statement = transaction.prepare("PRAGMA table_info(high_renown_milestones)")?;
    let primary_key = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, i32>(5)?))
        })?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|(_, pk_ordinal)| *pk_ordinal > 0)
        .collect::<Vec<_>>();
    let expected_primary_key = [
        ("player_uuid".to_owned(), 1),
        ("identity_id".to_owned(), 2),
        ("milestone".to_owned(), 3),
    ];
    if primary_key.as_slice() != expected_primary_key.as_slice() {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            io::Error::other(format!(
                "v20 migration completed but high_renown_milestones primary key mismatch: expected player_uuid,identity_id,milestone got {primary_key:?}"
            )),
        )));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct VoidActionCooldownRecord {
    character_id: String,
    kind: VoidActionKind,
    ready_at_tick: u64,
}

pub fn persist_void_action_cooldown(
    settings: &PersistenceSettings,
    character_id: &str,
    kind: VoidActionKind,
    ready_at_tick: u64,
) -> io::Result<()> {
    if kind.cooldown_ticks() == 0 {
        return Ok(());
    }
    let ready_at_tick = i64::try_from(ready_at_tick).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("void-action cooldown tick overflows sqlite INTEGER: {error}"),
        )
    })?;
    let connection = open_persistence_connection(settings)?;
    connection
        .execute(
            "
            INSERT INTO void_action_cooldowns (
                character_id,
                kind,
                ready_at_tick,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(character_id, kind) DO UPDATE SET
                ready_at_tick = excluded.ready_at_tick,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                character_id,
                kind.wire_name(),
                ready_at_tick,
                current_unix_seconds(),
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

fn load_void_action_cooldown_records(
    settings: &PersistenceSettings,
) -> io::Result<Vec<VoidActionCooldownRecord>> {
    let connection = open_persistence_connection(settings)?;
    let mut statement = connection
        .prepare(
            "
            SELECT character_id, kind, ready_at_tick
            FROM void_action_cooldowns
            ORDER BY character_id, kind
            ",
        )
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map([], |row| {
            let character_id: String = row.get(0)?;
            let kind_name: String = row.get(1)?;
            let kind = VoidActionKind::from_wire_name(kind_name.as_str()).ok_or_else(|| {
                rusqlite::Error::FromSqlConversionFailure(
                    1,
                    Type::Text,
                    Box::new(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("unknown void-action kind `{kind_name}`"),
                    )),
                )
            })?;
            let ready_at_tick: i64 = row.get(2)?;
            Ok(VoidActionCooldownRecord {
                character_id,
                kind,
                ready_at_tick: ready_at_tick as u64,
            })
        })
        .map_err(io::Error::other)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(io::Error::other)
}

fn hydrate_void_action_cooldowns(
    settings: &PersistenceSettings,
    cooldowns: &mut VoidActionCooldowns,
) -> io::Result<usize> {
    let records = load_void_action_cooldown_records(settings)?;
    let count = records.len();
    for record in records {
        cooldowns.force_ready_at(
            record.character_id.as_str(),
            record.kind,
            record.ready_at_tick,
        );
    }
    Ok(count)
}

fn legacy_player_realm_to_cultivation(realm: &str) -> Option<Realm> {
    match realm {
        "mortal" => Some(Realm::Awaken),
        "qi_refining_1" => Some(Realm::Induce),
        "qi_refining_2" => Some(Realm::Condense),
        "qi_refining_3" | "foundation_establishment_1" => Some(Realm::Spirit),
        "Awaken" => Some(Realm::Awaken),
        "Induce" => Some(Realm::Induce),
        "Condense" => Some(Realm::Condense),
        "Solidify" => Some(Realm::Solidify),
        "Spirit" => Some(Realm::Spirit),
        "Void" => Some(Realm::Void),
        _ => None,
    }
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

#[cfg_attr(not(test), allow(dead_code))]
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

pub fn persist_agent_world_model_authority_state(
    settings: &PersistenceSettings,
    envelope_id: &str,
    source: &str,
    snapshot: &AgentWorldModelSnapshotRecord,
) -> io::Result<()> {
    let wall_clock = current_unix_seconds();
    let snapshot_json = serde_json::to_string(snapshot)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    upsert_agent_world_model_snapshot(&transaction, &snapshot_json, wall_clock)?;
    if let Some(era) = snapshot.current_era.as_ref() {
        append_agent_era(
            &transaction,
            envelope_id,
            source,
            era,
            snapshot.last_tick,
            wall_clock,
        )?;
    }
    for (agent_name, decision) in &snapshot.last_decisions {
        append_agent_decision(
            &transaction,
            envelope_id,
            source,
            agent_name,
            decision,
            snapshot.last_tick,
            wall_clock,
        )?;
    }
    prune_agent_world_model_append_only(&transaction, wall_clock)?;
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

#[cfg_attr(not(test), allow(dead_code))]
pub fn load_agent_eras(settings: &PersistenceSettings) -> io::Result<Vec<AgentEraRecord>> {
    let connection = open_persistence_connection(settings)?;
    load_agent_eras_from_connection(&connection)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn load_agent_decisions(
    settings: &PersistenceSettings,
) -> io::Result<Vec<AgentDecisionRecord>> {
    let connection = open_persistence_connection(settings)?;
    load_agent_decisions_from_connection(&connection)
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

pub fn load_active_tribulation_count(settings: &PersistenceSettings) -> io::Result<u32> {
    let connection = open_persistence_connection(settings)?;
    let count: i64 = connection
        .query_row(
            "
            SELECT COUNT(*) FROM tribulations_active
            WHERE kind = ?1
               OR (kind = ?2 AND source = ?3)
            ",
            params![
                TRIBULATION_KIND_DU_XU,
                TRIBULATION_KIND_JUE_BI,
                JUEBI_SOURCE_VOID_QUOTA_EXCEEDED
            ],
            |row| row.get(0),
        )
        .map_err(io::Error::other)?;
    sql_to_u32(count)
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
    let active_kind_source: Option<(String, String)> = transaction
        .query_row(
            "SELECT kind, source FROM tribulations_active WHERE char_id = ?1",
            params![char_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(io::Error::other)?;
    let occupies_quota = matches!(
        active_kind_source
            .as_ref()
            .map(|(kind, source)| (kind.as_str(), source.as_str())),
        Some((TRIBULATION_KIND_DU_XU, _))
            | Some((TRIBULATION_KIND_JUE_BI, JUEBI_SOURCE_VOID_QUOTA_EXCEEDED))
    );
    if occupies_quota {
        quota.occupied_slots = quota.occupied_slots.saturating_add(1);
    }

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

pub fn release_ascension_quota_slot(
    settings: &PersistenceSettings,
) -> io::Result<AscensionQuotaRelease> {
    let wall_clock = current_unix_seconds();
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    let mut quota = load_ascension_quota_from_transaction(&transaction)?;
    let opened_slot = quota.occupied_slots > 0;
    quota.occupied_slots = quota.occupied_slots.saturating_sub(1);
    upsert_ascension_quota(&transaction, &quota, wall_clock)?;
    transaction.commit().map_err(io::Error::other)?;
    Ok(AscensionQuotaRelease { quota, opened_slot })
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

fn normalize_zone_overlay_payload(
    record: ZoneOverlayRecord,
    supported_payload_version: i32,
) -> io::Result<Option<ZoneOverlayRecord>> {
    if record.payload_version < 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "zone overlay payload_version {} must be >= 1",
                record.payload_version
            ),
        ));
    }
    if record.payload_version > supported_payload_version {
        tracing::warn!(
            "[bong][persistence] preserve future zone overlay `{}`/`{}` at {}: payload_version {} is newer than supported {}",
            record.zone_id,
            record.overlay_kind,
            record.since_wall,
            record.payload_version,
            supported_payload_version
        );
        return Ok(Some(record));
    }

    let mut migrated = record;
    while migrated.payload_version < supported_payload_version {
        migrated = match migrated.payload_version {
            1 => migrate_zone_overlay_payload_v1_to_v2(migrated)?,
            unsupported => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("no zone overlay payload migration from version {unsupported}"),
                ));
            }
        };
    }

    Ok(Some(migrated))
}

fn migrate_zone_overlay_payload_v1_to_v2(
    mut record: ZoneOverlayRecord,
) -> io::Result<ZoneOverlayRecord> {
    let mut payload: serde_json::Value = serde_json::from_str(record.payload_json.as_str())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let Some(payload_object) = payload.as_object_mut() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "zone overlay v1 payload must be a JSON object",
        ));
    };
    payload_object
        .entry("payload_schema".to_string())
        .or_insert_with(|| serde_json::Value::String("zone_overlay_v2".to_string()));
    record.payload_json = serde_json::to_string(&payload)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    record.payload_version = 2;
    Ok(record)
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

pub fn persist_lifespan_event(
    settings: &PersistenceSettings,
    char_id: &str,
    event: &LifespanEventRecord,
) -> io::Result<()> {
    let wall_clock = current_unix_seconds();
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;

    append_lifespan_event(&transaction, char_id, event, wall_clock)?;

    transaction.commit().map_err(io::Error::other)
}

pub fn persist_life_record_death_insight(
    settings: &PersistenceSettings,
    life_record: &LifeRecord,
) -> io::Result<()> {
    let Some(death_insight) = life_record.death_insights.last() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "life_record must contain at least one death insight before persistence",
        ));
    };

    let wall_clock = current_unix_seconds();
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;

    upsert_life_record(&transaction, life_record, wall_clock)?;
    append_death_insight_event(
        &transaction,
        life_record.character_id.as_str(),
        death_insight,
        wall_clock,
    )?;
    update_deceased_snapshot_life_record(
        &transaction,
        life_record.character_id.as_str(),
        life_record,
        wall_clock,
    )?;

    transaction.commit().map_err(io::Error::other)
}

pub fn persist_termination_transition(
    settings: &PersistenceSettings,
    lifecycle: &Lifecycle,
    life_record: &LifeRecord,
) -> io::Result<()> {
    persist_termination_transition_inner(settings, lifecycle, life_record, None, None)
}

pub fn persist_termination_transition_with_death_context(
    settings: &PersistenceSettings,
    lifecycle: &Lifecycle,
    life_record: &LifeRecord,
    death_registry_cause: Option<&str>,
    lifespan_event: Option<&LifespanEventRecord>,
) -> io::Result<()> {
    persist_termination_transition_inner(
        settings,
        lifecycle,
        life_record,
        death_registry_cause,
        lifespan_event,
    )
}

fn persist_termination_transition_inner(
    settings: &PersistenceSettings,
    lifecycle: &Lifecycle,
    life_record: &LifeRecord,
    death_registry_cause: Option<&str>,
    lifespan_event: Option<&LifespanEventRecord>,
) -> io::Result<()> {
    let entry = latest_biography_entry(life_record)?;
    let wall_clock = current_unix_seconds();
    let died_at_tick = biography_tick(entry);
    let termination_category = termination_category_from_entry(entry);
    let social = load_deceased_social_snapshot(settings, life_record.character_id.as_str())?;
    let snapshot = DeceasedSnapshot {
        char_id: life_record.character_id.clone(),
        died_at_tick,
        termination_category: termination_category.clone(),
        lifecycle: lifecycle.clone(),
        life_record: life_record.clone(),
        social,
    };
    let snapshot_json = serde_json::to_string_pretty(&snapshot)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let staged_export = if should_export_public_snapshot(life_record.character_id.as_str()) {
        Some(stage_public_deceased_export(
            settings,
            life_record.character_id.as_str(),
            snapshot_json.as_str(),
            died_at_tick,
            termination_category.as_str(),
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
        if let Some(death_registry_cause) = death_registry_cause {
            upsert_death_registry(
                &transaction,
                life_record.character_id.as_str(),
                lifecycle,
                death_registry_cause,
                wall_clock,
            )?;
        }
        if let Some(lifespan_event) = lifespan_event {
            append_lifespan_event(
                &transaction,
                life_record.character_id.as_str(),
                lifespan_event,
                wall_clock,
            )?;
        }
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

fn prune_agent_world_model_append_only(
    transaction: &rusqlite::Transaction<'_>,
    now_wall: i64,
) -> io::Result<()> {
    let threshold = now_wall.saturating_sub(AGENT_WORLD_MODEL_APPEND_ONLY_RETENTION_SECS);
    transaction
        .execute(
            "DELETE FROM agent_eras WHERE observed_at_wall < ?1",
            params![threshold],
        )
        .map_err(io::Error::other)?;
    transaction
        .execute(
            "DELETE FROM agent_decisions WHERE observed_at_wall < ?1",
            params![threshold],
        )
        .map_err(io::Error::other)?;
    Ok(())
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
    Option<&'a NpcLivePersistenceSnapshot>,
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
        live_snapshot,
        archived,
    ) in &npcs
    {
        let nearest_player_id = resolve_nearest_player_id(blackboard, &players);
        let effective_state = effective_npc_state(entity, lifecycle, &action_states);
        let is_terminated = lifecycle.state == LifecycleState::Terminated;
        let should_snapshot = if is_terminated {
            archived.is_none()
        } else {
            snapshot_due || live_snapshot.is_none()
        };
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

        if is_terminated && archived.is_none() {
            commands.entity(entity).insert(NpcArchivedPersistence);
        } else if !is_terminated && live_snapshot.is_none() {
            commands.entity(entity).insert(NpcLivePersistenceSnapshot);
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

pub(crate) fn open_persistence_connection(
    settings: &PersistenceSettings,
) -> io::Result<Connection> {
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

fn upsert_agent_world_model_snapshot(
    transaction: &rusqlite::Transaction<'_>,
    snapshot_json: &str,
    wall_clock: i64,
) -> io::Result<()> {
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
    Ok(())
}

fn append_agent_era(
    transaction: &rusqlite::Transaction<'_>,
    envelope_id: &str,
    source: &str,
    era: &serde_json::Value,
    observed_at_tick: Option<i64>,
    wall_clock: i64,
) -> io::Result<()> {
    let era_name = era
        .get("name")
        .and_then(|value| value.as_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "agent era missing name"))?;
    let since_tick = era
        .get("since_tick")
        .and_then(|value| value.as_i64())
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "agent era missing since_tick")
        })?;
    let global_effect = era
        .get("global_effect")
        .and_then(|value| value.as_str())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "agent era missing global_effect",
            )
        })?;
    transaction
        .execute(
            "
            INSERT INTO agent_eras (
                event_id,
                envelope_id,
                source,
                era_name,
                since_tick,
                global_effect,
                observed_at_tick,
                observed_at_wall,
                schema_version
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ",
            params![
                Uuid::now_v7().to_string(),
                envelope_id,
                source,
                era_name,
                since_tick,
                global_effect,
                observed_at_tick,
                wall_clock,
                EVENT_SCHEMA_VERSION,
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

fn append_agent_decision(
    transaction: &rusqlite::Transaction<'_>,
    envelope_id: &str,
    source: &str,
    agent_name: &str,
    decision: &AgentWorldModelDecisionRecord,
    observed_at_tick: Option<i64>,
    wall_clock: i64,
) -> io::Result<()> {
    let payload_json = serde_json::to_string(decision)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    transaction
        .execute(
            "
            INSERT INTO agent_decisions (
                event_id,
                envelope_id,
                source,
                agent_name,
                reasoning,
                command_count,
                narration_count,
                payload_json,
                observed_at_tick,
                observed_at_wall,
                schema_version
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ",
            params![
                Uuid::now_v7().to_string(),
                envelope_id,
                source,
                agent_name,
                decision.reasoning,
                i64::try_from(decision.commands.len())
                    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
                i64::try_from(decision.narrations.len())
                    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
                payload_json,
                observed_at_tick,
                wall_clock,
                EVENT_SCHEMA_VERSION,
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
                kind,
                source,
                origin_dimension,
                wave_current,
                waves_total,
                started_tick,
                epicenter_x,
                epicenter_y,
                epicenter_z,
                intensity,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            ON CONFLICT(char_id) DO UPDATE SET
                kind = excluded.kind,
                source = excluded.source,
                origin_dimension = excluded.origin_dimension,
                wave_current = excluded.wave_current,
                waves_total = excluded.waves_total,
                started_tick = excluded.started_tick,
                epicenter_x = excluded.epicenter_x,
                epicenter_y = excluded.epicenter_y,
                epicenter_z = excluded.epicenter_z,
                intensity = excluded.intensity,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                record.char_id.as_str(),
                record.kind.as_str(),
                record.source.as_str(),
                record.origin_dimension.as_deref(),
                i64::from(record.wave_current),
                i64::from(record.waves_total),
                tick_to_sql(record.started_tick)?,
                record.epicenter[0],
                record.epicenter[1],
                record.epicenter[2],
                f64::from(record.intensity),
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
    let record = normalize_zone_overlay_payload(record.clone(), ZONE_OVERLAY_PAYLOAD_VERSION)?
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "zone overlay payload_version {} is newer than supported {}",
                    record.payload_version, ZONE_OVERLAY_PAYLOAD_VERSION
                ),
            )
        })?;
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
    type ActiveTribulationRow = (
        String,
        String,
        Option<String>,
        i64,
        i64,
        i64,
        f64,
        f64,
        f64,
        f64,
    );
    let row: Option<ActiveTribulationRow> = connection
        .query_row(
            "
            SELECT kind, source, origin_dimension, wave_current, waves_total, started_tick, epicenter_x, epicenter_y, epicenter_z, intensity
            FROM tribulations_active
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
                ))
            },
        )
        .optional()
        .map_err(io::Error::other)?;
    let Some((
        kind,
        source,
        origin_dimension,
        wave_current,
        waves_total,
        started_tick,
        x,
        y,
        z,
        intensity,
    )) = row
    else {
        return Ok(None);
    };

    Ok(Some(ActiveTribulationRecord {
        char_id: char_id.to_string(),
        kind,
        source,
        origin_dimension,
        wave_current: sql_to_u32(wave_current)?,
        waves_total: sql_to_u32(waves_total)?,
        started_tick: sql_to_tick(started_tick)?,
        epicenter: [x, y, z],
        intensity: intensity as f32,
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
        let record = row.map_err(io::Error::other)?;
        if let Some(record) = normalize_zone_overlay_payload(record, ZONE_OVERLAY_PAYLOAD_VERSION)?
        {
            overlays.push(record);
        }
    }
    Ok(overlays)
}

fn load_agent_eras_from_connection(connection: &Connection) -> io::Result<Vec<AgentEraRecord>> {
    let mut statement = connection
        .prepare(
            "
            SELECT event_id, envelope_id, source, era_name, since_tick, global_effect,
                   observed_at_tick, observed_at_wall
            FROM agent_eras
            ORDER BY observed_at_wall ASC, event_id ASC
            ",
        )
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map([], |row| {
            Ok(AgentEraRecord {
                event_id: row.get(0)?,
                envelope_id: row.get(1)?,
                source: row.get(2)?,
                era_name: row.get(3)?,
                since_tick: row.get(4)?,
                global_effect: row.get(5)?,
                observed_at_tick: row.get(6)?,
                observed_at_wall: row.get(7)?,
            })
        })
        .map_err(io::Error::other)?;

    let mut records = Vec::new();
    for row in rows {
        records.push(row.map_err(io::Error::other)?);
    }
    Ok(records)
}

#[cfg_attr(not(test), allow(dead_code))]
#[allow(clippy::too_many_arguments)]
pub fn persist_player_cultivation_bundle(
    settings: &PersistenceSettings,
    username: &str,
    cultivation: &crate::cultivation::components::Cultivation,
    meridians: &crate::cultivation::components::MeridianSystem,
    qi_color: &crate::cultivation::components::QiColor,
    karma: &crate::cultivation::components::Karma,
    contamination: &crate::cultivation::components::Contamination,
    life_record: &crate::cultivation::life_record::LifeRecord,
    practice_log: &crate::cultivation::color::PracticeLog,
    insight_quota: &crate::cultivation::insight::InsightQuota,
    unlocked_perceptions: &crate::cultivation::insight_apply::UnlockedPerceptions,
    insight_modifiers: &crate::cultivation::insight_apply::InsightModifiers,
    tutorial_state: Option<&crate::world::spawn_tutorial::TutorialState>,
    meridian_severed: &crate::cultivation::meridian::severed::MeridianSeveredPermanent,
    poison_toxicity: Option<&crate::cultivation::poison_trait::PoisonToxicity>,
    digestion_load: Option<&crate::cultivation::poison_trait::DigestionLoad>,
) -> io::Result<()> {
    let wall_clock = current_unix_seconds();
    let bundle = serde_json::json!({
        "v": 1,
        "cultivation": cultivation,
        "meridians": meridians,
        "qi_color": qi_color,
        "karma": karma,
        "contamination": contamination,
        "life_record": life_record,
        "practice_log": practice_log,
        "insight_quota": insight_quota,
        "unlocked_perceptions": unlocked_perceptions,
        "insight_modifiers": insight_modifiers,
        "tutorial_state": tutorial_state,
        "meridian_severed": meridian_severed,
        "poison_toxicity": poison_toxicity,
        "digestion_load": digestion_load,
    });
    let cultivation_json = serde_json::to_string(&bundle)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    let connection = open_persistence_connection(settings)?;
    connection
        .execute(
            "
            INSERT INTO player_cultivation (
                username,
                cultivation_json,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(username) DO UPDATE SET
                cultivation_json = excluded.cultivation_json,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                username,
                cultivation_json,
                CURRENT_SCHEMA_VERSION,
                wall_clock
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn load_player_cultivation_bundle(
    settings: &PersistenceSettings,
    username: &str,
) -> io::Result<Option<serde_json::Value>> {
    let connection = open_persistence_connection(settings)?;
    let row: Option<String> = connection
        .query_row(
            "
            SELECT cultivation_json
            FROM player_cultivation
            WHERE username = ?1
            ",
            params![username],
            |row| row.get(0),
        )
        .optional()
        .map_err(io::Error::other)?;
    let Some(json) = row else {
        return Ok(None);
    };
    let decoded = serde_json::from_str(&json)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    Ok(Some(decoded))
}

fn load_agent_decisions_from_connection(
    connection: &Connection,
) -> io::Result<Vec<AgentDecisionRecord>> {
    let mut statement = connection
        .prepare(
            "
            SELECT event_id, envelope_id, source, agent_name, reasoning, command_count,
                   narration_count, payload_json, observed_at_tick, observed_at_wall
            FROM agent_decisions
            ORDER BY observed_at_wall ASC, event_id ASC
            ",
        )
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, Option<i64>>(8)?,
                row.get::<_, i64>(9)?,
            ))
        })
        .map_err(io::Error::other)?;

    let mut records = Vec::new();
    for row in rows {
        let (
            event_id,
            envelope_id,
            source,
            agent_name,
            reasoning,
            command_count,
            narration_count,
            payload_json,
            observed_at_tick,
            observed_at_wall,
        ) = row.map_err(io::Error::other)?;
        records.push(AgentDecisionRecord {
            event_id,
            envelope_id,
            source,
            agent_name,
            reasoning,
            command_count: sql_to_u32(command_count)?,
            narration_count: sql_to_u32(narration_count)?,
            payload_json,
            observed_at_tick,
            observed_at_wall,
        });
    }
    Ok(records)
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
        BiographyEntry::SpiritEyeBreakthrough { .. } => "spirit_eye_breakthrough",
        BiographyEntry::BreakthroughFailed { .. } => "breakthrough_failed",
        BiographyEntry::MeridianOpened { .. } => "meridian_opened",
        BiographyEntry::MeridianClosed { .. } => "meridian_closed",
        BiographyEntry::ForgedRate { .. } => "forged_rate",
        BiographyEntry::ForgedCapacity { .. } => "forged_capacity",
        BiographyEntry::ColorShift { .. } => "color_shift",
        BiographyEntry::InsightTaken { .. } => "insight_taken",
        BiographyEntry::InsightDiverge { .. } => "insight_diverge",
        BiographyEntry::Rebirth { .. } => "rebirth",
        BiographyEntry::CombatHit { .. } => "combat_hit",
        BiographyEntry::DuguPoisonInflicted { .. } => "dugu_poison_inflicted",
        BiographyEntry::JiemaiParry { .. } => "jiemai_parry",
        BiographyEntry::NearDeath { .. } => "near_death",
        BiographyEntry::Terminated { .. } => "terminated",
        BiographyEntry::LifespanExtended { .. } => "lifespan_extended",
        BiographyEntry::DuoShePerformed { .. } => "duoshe_performed",
        BiographyEntry::PossessedBy { .. } => "possessed_by",
        BiographyEntry::AlchemyAttempt { .. } => "alchemy_attempt",
        BiographyEntry::PlotHarvestedByOther { .. } => "plot_harvested_by_other",
        BiographyEntry::PlotHarvestedFromOther { .. } => "plot_harvested_from_other",
        BiographyEntry::PlotQiDrainedByOther { .. } => "plot_qi_drained_by_other",
        BiographyEntry::PlotQiDrainedFromOther { .. } => "plot_qi_drained_from_other",
        BiographyEntry::PlotDestroyedByOther { .. } => "plot_destroyed_by_other",
        BiographyEntry::TribulationIntercepted { .. } => "tribulation_intercepted",
        BiographyEntry::TribulationFled { .. } => "tribulation_fled",
        BiographyEntry::HeartDemonRecord { .. } => "heart_demon_record",
        BiographyEntry::TradeCompleted { .. } => "trade_completed",
        BiographyEntry::NicheIntrusion { .. } => "niche_intrusion",
        BiographyEntry::VortexProjectileDrained { .. } => "vortex_projectile_drained",
        BiographyEntry::VortexBackfired { .. } => "vortex_backfired",
        BiographyEntry::AnqiSniped { .. } => "anqi_sniped",
        BiographyEntry::FalseSkinShed { .. } => "false_skin_shed",
        BiographyEntry::SpawnTutorialCompleted { .. } => "spawn_tutorial_completed",
        BiographyEntry::VoidAction { .. } => "void_action",
        BiographyEntry::JueBiSurvived { .. } => "jue_bi_survived",
        BiographyEntry::JueBiKilled { .. } => "jue_bi_killed",
    }
}

fn append_death_insight_event(
    transaction: &rusqlite::Transaction<'_>,
    char_id: &str,
    death_insight: &DeathInsightRecord,
    wall_clock: i64,
) -> io::Result<()> {
    let payload_json = serde_json::to_string(&DeathInsightEventPayload {
        death_insight: death_insight.clone(),
    })
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let event_id = format!(
        "{}:death_insight:{}:{}",
        char_id, death_insight.tick, wall_clock
    );

    transaction
        .execute(
            "
            INSERT OR IGNORE INTO life_events (
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
                event_id,
                char_id,
                "death_insight",
                payload_json,
                EVENT_PAYLOAD_VERSION,
                tick_to_sql(death_insight.tick)?,
                wall_clock,
                EVENT_SCHEMA_VERSION
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

fn biography_tick(entry: &BiographyEntry) -> u64 {
    match entry {
        BiographyEntry::BreakthroughStarted { tick, .. }
        | BiographyEntry::BreakthroughSucceeded { tick, .. }
        | BiographyEntry::SpiritEyeBreakthrough { tick, .. }
        | BiographyEntry::BreakthroughFailed { tick, .. }
        | BiographyEntry::MeridianOpened { tick, .. }
        | BiographyEntry::MeridianClosed { tick, .. }
        | BiographyEntry::ForgedRate { tick, .. }
        | BiographyEntry::ForgedCapacity { tick, .. }
        | BiographyEntry::ColorShift { tick, .. }
        | BiographyEntry::InsightTaken { tick, .. }
        | BiographyEntry::InsightDiverge { tick, .. }
        | BiographyEntry::Rebirth { tick, .. }
        | BiographyEntry::CombatHit { tick, .. }
        | BiographyEntry::DuguPoisonInflicted { tick, .. }
        | BiographyEntry::JiemaiParry { tick, .. }
        | BiographyEntry::NearDeath { tick, .. }
        | BiographyEntry::Terminated { tick, .. }
        | BiographyEntry::LifespanExtended { tick, .. }
        | BiographyEntry::DuoShePerformed { tick, .. }
        | BiographyEntry::PossessedBy { tick, .. }
        | BiographyEntry::AlchemyAttempt { tick, .. }
        | BiographyEntry::PlotHarvestedByOther { tick, .. }
        | BiographyEntry::PlotHarvestedFromOther { tick, .. }
        | BiographyEntry::PlotQiDrainedByOther { tick, .. }
        | BiographyEntry::PlotQiDrainedFromOther { tick, .. }
        | BiographyEntry::PlotDestroyedByOther { tick, .. }
        | BiographyEntry::TribulationIntercepted { tick, .. }
        | BiographyEntry::TribulationFled { tick, .. }
        | BiographyEntry::HeartDemonRecord { tick, .. }
        | BiographyEntry::TradeCompleted { tick, .. }
        | BiographyEntry::NicheIntrusion { tick, .. }
        | BiographyEntry::VortexProjectileDrained { tick, .. }
        | BiographyEntry::VortexBackfired { tick, .. }
        | BiographyEntry::AnqiSniped { tick, .. }
        | BiographyEntry::FalseSkinShed { tick, .. }
        | BiographyEntry::SpawnTutorialCompleted { tick, .. }
        | BiographyEntry::VoidAction { tick, .. }
        | BiographyEntry::JueBiSurvived { tick, .. }
        | BiographyEntry::JueBiKilled { tick, .. } => *tick,
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

fn load_deceased_social_snapshot(
    settings: &PersistenceSettings,
    char_id: &str,
) -> io::Result<Option<DeceasedSocialSnapshot>> {
    let connection = open_persistence_connection(settings)?;
    let renown = load_deceased_renown(&connection, char_id)?;
    let relationships = load_deceased_relationships(&connection, char_id)?;
    let exposure_log = load_deceased_exposure_log(&connection, char_id)?;
    let faction_membership = load_deceased_faction_membership(&connection, char_id)?;

    if renown == DeceasedRenownSnapshot::default()
        && relationships.is_empty()
        && exposure_log.is_empty()
        && faction_membership.is_none()
    {
        return Ok(None);
    }

    Ok(Some(DeceasedSocialSnapshot {
        renown,
        relationships,
        exposure_log,
        faction_membership,
    }))
}

fn load_deceased_renown(
    connection: &Connection,
    char_id: &str,
) -> io::Result<DeceasedRenownSnapshot> {
    let row: Option<(i32, i32, String)> = connection
        .query_row(
            "SELECT fame, notoriety, tags_json FROM social_renown WHERE char_id = ?1",
            params![char_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(io::Error::other)?;
    let Some((fame, notoriety, tags_json)) = row else {
        return Ok(DeceasedRenownSnapshot::default());
    };
    let tags = serde_json::from_str::<Vec<RenownTagV1>>(tags_json.as_str())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    Ok(DeceasedRenownSnapshot {
        fame,
        notoriety,
        tags,
    })
}

fn load_deceased_relationships(
    connection: &Connection,
    char_id: &str,
) -> io::Result<Vec<RelationshipSnapshotV1>> {
    let mut statement = connection
        .prepare(
            "
            SELECT peer_char_id, relationship_type, since_tick, metadata_json
            FROM social_relationships
            WHERE char_id = ?1
            ORDER BY peer_char_id ASC, relationship_type ASC
            ",
        )
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map(params![char_id], |row| {
            let kind_label: String = row.get(1)?;
            let metadata_json: String = row.get(3)?;
            let kind =
                parse_enum_label::<RelationshipKindV1>(kind_label.as_str()).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        1,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
            let metadata = serde_json::from_str(metadata_json.as_str()).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    3,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?;
            Ok(RelationshipSnapshotV1 {
                peer: row.get(0)?,
                kind,
                since_tick: sql_to_tick(row.get::<_, i64>(2)?).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        2,
                        rusqlite::types::Type::Integer,
                        Box::new(error),
                    )
                })?,
                metadata,
            })
        })
        .map_err(io::Error::other)?;

    let mut relationships = Vec::new();
    for row in rows {
        relationships.push(row.map_err(io::Error::other)?);
    }
    Ok(relationships)
}

fn load_deceased_exposure_log(
    connection: &Connection,
    char_id: &str,
) -> io::Result<Vec<DeceasedExposureSnapshot>> {
    let mut statement = connection
        .prepare(
            "
            SELECT kind, witnesses_json, at_tick
            FROM social_exposures
            WHERE char_id = ?1
            ORDER BY at_tick ASC, event_id ASC
            ",
        )
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map(params![char_id], |row| {
            let kind_label: String = row.get(0)?;
            let witnesses_json: String = row.get(1)?;
            let kind =
                parse_enum_label::<ExposureKindV1>(kind_label.as_str()).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
            let witnesses = serde_json::from_str(witnesses_json.as_str()).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    1,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?;
            Ok(DeceasedExposureSnapshot {
                kind,
                witnesses,
                tick: sql_to_tick(row.get::<_, i64>(2)?).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        2,
                        rusqlite::types::Type::Integer,
                        Box::new(error),
                    )
                })?,
            })
        })
        .map_err(io::Error::other)?;

    let mut exposure_log = Vec::new();
    for row in rows {
        exposure_log.push(row.map_err(io::Error::other)?);
    }
    Ok(exposure_log)
}

fn load_deceased_faction_membership(
    connection: &Connection,
    char_id: &str,
) -> io::Result<Option<FactionMembershipSnapshotV1>> {
    let row: Option<DeceasedFactionMembershipSqlRow> = connection
        .query_row(
            "
            SELECT faction, rank, loyalty, betrayal_count, invite_block_until_tick, permanently_refused
            FROM social_faction_memberships
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
                ))
            },
        )
        .optional()
        .map_err(io::Error::other)?;
    let Some((
        faction,
        rank,
        loyalty,
        betrayal_count,
        invite_block_until_tick,
        permanently_refused,
    )) = row
    else {
        return Ok(None);
    };

    Ok(Some(FactionMembershipSnapshotV1 {
        faction: faction.unwrap_or_else(|| "neutral".to_string()),
        rank: u8::try_from(rank)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
        loyalty: i32::try_from(loyalty)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
        betrayal_count: u8::try_from(betrayal_count)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
        invite_block_until_tick: invite_block_until_tick.map(sql_to_tick).transpose()?,
        permanently_refused: permanently_refused != 0,
    }))
}

fn update_deceased_snapshot_life_record(
    transaction: &rusqlite::Transaction<'_>,
    char_id: &str,
    life_record: &LifeRecord,
    wall_clock: i64,
) -> io::Result<()> {
    let Some(existing_snapshot_json) = transaction
        .query_row(
            "SELECT snapshot_json FROM deceased_snapshots WHERE char_id = ?1",
            params![char_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(io::Error::other)?
    else {
        return Ok(());
    };

    let mut snapshot: DeceasedSnapshot = serde_json::from_str(existing_snapshot_json.as_str())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    snapshot.life_record = life_record.clone();
    let snapshot_json = serde_json::to_string_pretty(&snapshot)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    transaction
        .execute(
            "
            UPDATE deceased_snapshots
            SET snapshot_json = ?2,
                last_updated_wall = ?3
            WHERE char_id = ?1
            ",
            params![char_id, snapshot_json, wall_clock],
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

fn sanitize_deceased_snapshot_stem(char_id: &str) -> String {
    char_id
        .chars()
        .map(|character| match character {
            ':' | '/' | '\\' => '_',
            _ => character,
        })
        .collect()
}

fn default_termination_category() -> String {
    "横死".to_string()
}

fn parse_enum_label<T>(label: &str) -> io::Result<T>
where
    T: DeserializeOwned,
{
    serde_json::from_value(serde_json::Value::String(label.to_string()))
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn termination_category_from_entry(entry: &BiographyEntry) -> String {
    let BiographyEntry::Terminated { cause, .. } = entry else {
        return default_termination_category();
    };
    match cause.as_str() {
        "natural_end" => "善终",
        "voluntary_retire" => "自主归隐",
        "duo_she" => "夺舍者",
        _ => "横死",
    }
    .to_string()
}

fn deceased_export_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn stage_public_deceased_export(
    settings: &PersistenceSettings,
    char_id: &str,
    snapshot_json: &str,
    died_at_tick: u64,
    termination_category: &str,
) -> io::Result<StagedDeceasedExport> {
    let guard = deceased_export_lock()
        .lock()
        .map_err(|_| io::Error::other("deceased public export lock poisoned"))?;
    fs::create_dir_all(settings.deceased_public_dir())?;

    let snapshot_stem = sanitize_deceased_snapshot_stem(char_id);
    let snapshot_path = settings
        .deceased_public_dir()
        .join(format!("{snapshot_stem}.json"));
    let index_path = settings.deceased_public_dir().join("_index.json");
    let previous_snapshot = fs::read(&snapshot_path).ok();
    let previous_index = fs::read(&index_path).ok();
    fs::write(&snapshot_path, snapshot_json.as_bytes())?;

    let relative_snapshot_path = format!("deceased/{snapshot_stem}.json");
    let mut entries = read_deceased_index(&index_path)?;
    entries.retain(|entry| entry.char_id != char_id);
    entries.push(DeceasedIndexEntry {
        char_id: char_id.to_string(),
        died_at_tick,
        path: relative_snapshot_path.clone(),
        termination_category: termination_category.to_string(),
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
        _guard: guard,
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
    zstd::stream::decode_all(compressed.as_slice())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
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

fn find_orphaned_npc_archive_paths(settings: &PersistenceSettings) -> io::Result<Vec<PathBuf>> {
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
    let mut archive_files = collect_files_with_suffix(&archive_root, ".json.zst")?;
    archive_files.sort();
    let mut orphaned = Vec::new();
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
            orphaned.push(archive_file);
        }
    }

    Ok(orphaned)
}

fn scan_orphaned_npc_archives(settings: &PersistenceSettings) -> io::Result<()> {
    for archive_file in find_orphaned_npc_archive_paths(settings)? {
        tracing::warn!(
            "[bong][persistence] orphaned npc archive without sqlite index: {}",
            archive_file.display()
        );
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
    use crate::npc::spawn::{NpcBlackboard, NpcCombatLoadout, NpcMarker, NpcMeleeArchetype};
    use crate::player::state::{
        save_player_core_slice, save_player_state, PlayerState, PlayerStatePersistence,
    };
    use crate::schema::common::NpcStateKind;
    use crate::world::zone::DEFAULT_SPAWN_ZONE_NAME;
    use rusqlite::{params, OptionalExtension};
    use serde_json::Value;
    use std::sync::{Arc, Barrier};
    use std::time::Instant;
    use valence::prelude::{App, DVec3, EntityKind, Position, Update};

    #[test]
    fn sanitize_deceased_snapshot_stem_replaces_windows_invalid_separators() {
        assert_eq!(
            sanitize_deceased_snapshot_stem("offline:Ancestor/Shard\\Echo"),
            "offline_Ancestor_Shard_Echo"
        );
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

        for table in [
            "social_anonymity",
            "social_relationships",
            "social_exposures",
            "social_renown",
            "social_spirit_niches",
            "social_faction_memberships",
            "legacy_letterbox",
            "void_action_cooldowns",
            "high_renown_milestones",
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

        let error = apply_migrations(&mut connection)
            .expect_err("v20 migration should reject malformed table");
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
    fn v18_migration_rejects_partial_legacy_letterbox_schema() {
        let db_path = database_path("v18-partial-legacy-letterbox");
        fs::create_dir_all(db_path.parent().expect("db path should have parent"))
            .expect("temp db parent should be created");
        let connection = Connection::open(&db_path).expect("db should open");
        connection
            .execute_batch(
                "
                CREATE TABLE legacy_letterbox (
                    owner_id TEXT PRIMARY KEY
                );
                PRAGMA user_version = 17;
                ",
            )
            .expect("partial legacy table should be created");
        drop(connection);

        bootstrap_sqlite(&db_path, "server-run-test")
            .expect_err("partial legacy_letterbox schema must block v18 migration");

        let connection = Connection::open(&db_path).expect("db should reopen");
        let user_version: i32 = connection
            .query_row("PRAGMA user_version;", [], |row| row.get(0))
            .expect("user_version should be readable");
        assert_eq!(user_version, 17);
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
        let count = hydrate_void_action_cooldowns(&settings, &mut cooldowns)
            .expect("cooldowns should hydrate");

        assert_eq!(count, 1);
        assert_eq!(
            cooldowns.ready_at("offline:Void", VoidActionKind::Barrier),
            12_345
        );
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
        assert_eq!(user_version, CURRENT_USER_VERSION);

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
        assert_eq!(user_version, CURRENT_USER_VERSION);

        for table in [
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
        };

        persist_agent_world_model_authority_state(
            &settings,
            "wm-append-1",
            "arbiter",
            &first_snapshot,
        )
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
        let release =
            release_ascension_quota_slot(&settings).expect("empty release should succeed");
        assert_eq!(release.quota.occupied_slots, 0);
        assert!(!release.opened_slot);

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
                dimension: crate::world::dimension::DimensionKind::Overworld,
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
                payload_version: ZONE_OVERLAY_PAYLOAD_VERSION,
                since_wall: 10,
            },
            ZoneOverlayRecord {
                zone_id: "blood_valley".to_string(),
                overlay_kind: "ruins_discovered".to_string(),
                payload_json: serde_json::json!({"active_events": ["ruins_discovered"]})
                    .to_string(),
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
                payload_version: ZONE_OVERLAY_PAYLOAD_VERSION,
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
                dimension: crate::world::dimension::DimensionKind::Overworld,
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
            zones: vec![crate::world::zone::Zone {
                name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
                dimension: crate::world::dimension::DimensionKind::Overworld,
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
                payload_json: serde_json::json!({"active_events": ["ruins_discovered"]})
                    .to_string(),
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
        bootstrap_sqlite(&db_path, "future-user-version-rejected")
            .expect("bootstrap should succeed");

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
            zones: vec![crate::world::zone::Zone {
                name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
                dimension: crate::world::dimension::DimensionKind::Overworld,
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
                dimension: crate::world::dimension::DimensionKind::Overworld,
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
            death_insights: Vec::new(),
            skill_milestones: Vec::new(),
            spirit_root_first: None,
            ..LifeRecord::default()
        };
        let lifecycle = Lifecycle {
            character_id: life_record.character_id.clone(),
            death_count: 3,
            fortune_remaining: 0,
            last_death_tick: Some(77),
            last_revive_tick: Some(55),
            spawn_anchor: None,
            near_death_deadline_tick: None,
            awaiting_decision: None,
            revival_decision_deadline_tick: None,
            weakened_until_tick: None,
            state: crate::combat::components::LifecycleState::Terminated,
        };

        persist_termination_transition(&settings, &lifecycle, &life_record)
            .expect("terminated snapshot should persist");

        let snapshot_path = settings.deceased_public_dir().join("offline_Ancestor.json");
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
        assert_eq!(snapshot.termination_category, "横死");
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
        assert_eq!(index[0].path, "deceased/offline_Ancestor.json");
        assert_eq!(index[0].termination_category, "横死");
        assert_eq!(public_path, "deceased/offline_Ancestor.json");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn persist_termination_transition_preserves_skill_milestones_and_narration_in_deceased_exports()
    {
        let (settings, root) = persistence_settings("deceased-export-skill-milestones");
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
            death_insights: Vec::new(),
            skill_milestones: vec![crate::cultivation::life_record::SkillMilestone {
                skill: crate::skill::components::SkillId::Alchemy,
                new_lv: 4,
                achieved_at: 75,
                narration: "丹火三转，炉意已成。".to_string(),
                total_xp_at: 1_280,
            }],
            spirit_root_first: None,
            ..LifeRecord::default()
        };
        let lifecycle = Lifecycle {
            character_id: life_record.character_id.clone(),
            death_count: 3,
            fortune_remaining: 0,
            last_death_tick: Some(77),
            last_revive_tick: Some(55),
            spawn_anchor: None,
            near_death_deadline_tick: None,
            awaiting_decision: None,
            revival_decision_deadline_tick: None,
            weakened_until_tick: None,
            state: crate::combat::components::LifecycleState::Terminated,
        };

        persist_termination_transition(&settings, &lifecycle, &life_record)
            .expect("terminated snapshot should persist");

        let snapshot_path = settings.deceased_public_dir().join("offline_Ancestor.json");
        let public_snapshot: DeceasedSnapshot = serde_json::from_str(
            &fs::read_to_string(&snapshot_path).expect("snapshot json should exist"),
        )
        .expect("public snapshot json should deserialize");
        let connection = Connection::open(settings.db_path()).expect("db should open");
        let sqlite_snapshot_json: String = connection
            .query_row(
                "SELECT snapshot_json FROM deceased_snapshots WHERE char_id = ?1",
                params!["offline:Ancestor"],
                |row| row.get(0),
            )
            .expect("deceased snapshot row should exist");
        let sqlite_snapshot: DeceasedSnapshot = serde_json::from_str(&sqlite_snapshot_json)
            .expect("sqlite snapshot json should deserialize");

        for snapshot in [&public_snapshot, &sqlite_snapshot] {
            assert_eq!(snapshot.life_record.skill_milestones.len(), 1);
            assert_eq!(
                snapshot.life_record.skill_milestones[0].skill,
                crate::skill::components::SkillId::Alchemy
            );
            assert_eq!(snapshot.life_record.skill_milestones[0].new_lv, 4);
            assert_eq!(snapshot.life_record.skill_milestones[0].achieved_at, 75);
            assert_eq!(snapshot.life_record.skill_milestones[0].total_xp_at, 1_280);
            assert_eq!(
                snapshot.life_record.skill_milestones[0].narration,
                "丹火三转，炉意已成。"
            );
        }

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn persist_termination_transition_exports_public_social_snapshot() {
        let (settings, root) = persistence_settings("deceased-export-social");
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
                params!["offline:Ancestor", 12, 80, tags_json],
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
                    "offline:Ancestor",
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
                    "exposure-death-1",
                    "offline:Ancestor",
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
                params!["offline:Ancestor", "attack", 2, -10, 3, 88, 1],
            )
            .expect("faction membership row should insert");
        drop(connection);

        let life_record = LifeRecord {
            character_id: "offline:Ancestor".to_string(),
            created_at: 11,
            biography: vec![BiographyEntry::Terminated {
                cause: "fortune_exhausted".to_string(),
                tick: 77,
            }],
            insights_taken: Vec::new(),
            death_insights: Vec::new(),
            skill_milestones: Vec::new(),
            spirit_root_first: None,
            ..LifeRecord::default()
        };
        let lifecycle = Lifecycle {
            character_id: life_record.character_id.clone(),
            death_count: 3,
            fortune_remaining: 0,
            last_death_tick: Some(77),
            last_revive_tick: Some(55),
            spawn_anchor: None,
            near_death_deadline_tick: None,
            awaiting_decision: None,
            revival_decision_deadline_tick: None,
            weakened_until_tick: None,
            state: crate::combat::components::LifecycleState::Terminated,
        };

        persist_termination_transition(&settings, &lifecycle, &life_record)
            .expect("terminated snapshot should persist");

        let public_snapshot: DeceasedSnapshot = serde_json::from_str(
            &fs::read_to_string(settings.deceased_public_dir().join("offline_Ancestor.json"))
                .expect("snapshot json should exist"),
        )
        .expect("public snapshot json should deserialize");
        let sqlite_snapshot_json: String = Connection::open(settings.db_path())
            .expect("db should reopen")
            .query_row(
                "SELECT snapshot_json FROM deceased_snapshots WHERE char_id = ?1",
                params!["offline:Ancestor"],
                |row| row.get(0),
            )
            .expect("deceased snapshot row should exist");
        let sqlite_snapshot: DeceasedSnapshot = serde_json::from_str(&sqlite_snapshot_json)
            .expect("sqlite snapshot json should deserialize");

        for snapshot in [&public_snapshot, &sqlite_snapshot] {
            let social = snapshot.social.as_ref().expect("social should be public");
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
                .as_ref()
                .expect("faction membership should be public");
            assert_eq!(membership.faction, "attack");
            assert_eq!(membership.rank, 2);
            assert_eq!(membership.loyalty, -10);
            assert_eq!(membership.betrayal_count, 3);
            assert_eq!(membership.invite_block_until_tick, Some(88));
            assert!(membership.permanently_refused);
        }

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
            death_insights: Vec::new(),
            skill_milestones: Vec::new(),
            spirit_root_first: None,
            ..LifeRecord::default()
        };
        let first_lifecycle = Lifecycle {
            character_id: first_life_record.character_id.clone(),
            death_count: 3,
            fortune_remaining: 0,
            last_death_tick: Some(77),
            last_revive_tick: Some(55),
            spawn_anchor: None,
            near_death_deadline_tick: None,
            awaiting_decision: None,
            revival_decision_deadline_tick: None,
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
            death_insights: Vec::new(),
            skill_milestones: Vec::new(),
            spirit_root_first: None,
            ..LifeRecord::default()
        };
        let second_lifecycle = Lifecycle {
            character_id: second_life_record.character_id.clone(),
            death_count: 4,
            fortune_remaining: 0,
            last_death_tick: Some(99),
            last_revive_tick: Some(55),
            spawn_anchor: None,
            near_death_deadline_tick: None,
            awaiting_decision: None,
            revival_decision_deadline_tick: None,
            weakened_until_tick: None,
            state: crate::combat::components::LifecycleState::Terminated,
        };
        persist_termination_transition(&settings, &second_lifecycle, &second_life_record)
            .expect("second terminated snapshot should overwrite export");

        let snapshot_path = settings.deceased_public_dir().join("offline_Ancestor.json");
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
        assert_eq!(snapshot.termination_category, "横死");
        assert!(matches!(
            snapshot.life_record.biography.last(),
            Some(BiographyEntry::Terminated { tick: 99, .. })
        ));
        assert_eq!(index.len(), 1);
        assert_eq!(index[0].char_id, "offline:Ancestor");
        assert_eq!(index[0].died_at_tick, 99);
        assert_eq!(index[0].path, "deceased/offline_Ancestor.json");
        assert_eq!(index[0].termination_category, "横死");
        assert_eq!(died_at_tick, 99);
        assert_eq!(public_path, "deceased/offline_Ancestor.json");

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
                death_insights: Vec::new(),
                skill_milestones: Vec::new(),
                spirit_root_first: None,
                ..LifeRecord::default()
            };
            let lifecycle = Lifecycle {
                character_id: life_record.character_id.clone(),
                death_count: 1,
                fortune_remaining: 0,
                last_death_tick: Some(died_at_tick as u64),
                last_revive_tick: None,
                spawn_anchor: None,
                near_death_deadline_tick: None,
                awaiting_decision: None,
                revival_decision_deadline_tick: None,
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
        assert_eq!(index[0].path, "deceased/offline_Bronze.json");
        assert_eq!(index[0].termination_category, "横死");

        assert_eq!(index[1].char_id, "offline:Azure");
        assert_eq!(index[1].died_at_tick, 90);
        assert_eq!(index[1].path, "deceased/offline_Azure.json");
        assert_eq!(index[1].termination_category, "横死");

        assert_eq!(index[2].char_id, "offline:Crimson");
        assert_eq!(index[2].died_at_tick, 90);
        assert_eq!(index[2].path, "deceased/offline_Crimson.json");
        assert_eq!(index[2].termination_category, "横死");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn persist_termination_transition_classifies_good_end_and_voluntary_retire() {
        let (settings, root) = persistence_settings("deceased-export-category");
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
                insights_taken: Vec::new(),
                death_insights: Vec::new(),
                skill_milestones: Vec::new(),
                spirit_root_first: None,
                ..LifeRecord::default()
            };
            let lifecycle = Lifecycle {
                character_id: life_record.character_id.clone(),
                death_count: 1,
                fortune_remaining: 0,
                last_death_tick: Some(tick),
                last_revive_tick: None,
                spawn_anchor: None,
                near_death_deadline_tick: None,
                awaiting_decision: None,
                revival_decision_deadline_tick: None,
                weakened_until_tick: None,
                state: crate::combat::components::LifecycleState::Terminated,
            };

            persist_termination_transition(&settings, &lifecycle, &life_record)
                .expect("terminated snapshot should persist");

            let snapshot: DeceasedSnapshot = serde_json::from_str(
                &fs::read_to_string(
                    settings
                        .deceased_public_dir()
                        .join(format!("{}.json", sanitize_deceased_snapshot_stem(char_id))),
                )
                .expect("snapshot json should exist"),
            )
            .expect("snapshot json should deserialize");
            assert_eq!(snapshot.termination_category, expected_category);
        }

        let index_path = settings.deceased_public_dir().join("_index.json");
        let index: Vec<DeceasedIndexEntry> = serde_json::from_str(
            &fs::read_to_string(&index_path).expect("index json should exist"),
        )
        .expect("index json should deserialize");
        assert!(
            index
                .iter()
                .any(|entry| entry.char_id == "offline:OldOne"
                    && entry.termination_category == "善终")
        );
        assert!(index
            .iter()
            .any(|entry| entry.char_id == "offline:Hermit"
                && entry.termination_category == "自主归隐"));

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

        let player_persistence = PlayerStatePersistence::with_db_path(
            root.join("data").join("players"),
            settings.db_path(),
        );
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
                            karma: ((index as f64 / player_count as f64) * 2.0 - 1.0)
                                .clamp(-1.0, 1.0),
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
    fn semantic_death_peak_keeps_life_registry_and_public_exports_consistent() {
        let (settings, root) = persistence_settings("semantic-death-peak");
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
                        insights_taken: Vec::new(),
                        death_insights: Vec::new(),
                        skill_milestones: Vec::new(),
                        spirit_root_first: None,
                        ..LifeRecord::default()
                    };
                    let lifecycle = Lifecycle {
                        character_id: char_id,
                        death_count: 1,
                        fortune_remaining: 0,
                        last_death_tick: Some(tick),
                        last_revive_tick: None,
                        spawn_anchor: None,
                        near_death_deadline_tick: None,
                        awaiting_decision: None,
                        revival_decision_deadline_tick: None,
                        weakened_until_tick: None,
                        state: LifecycleState::Terminated,
                    };
                    let lifespan_event = LifespanEventRecord {
                        at_tick: tick,
                        kind: "termination".to_string(),
                        delta_years: -999,
                        source: "peak_death".to_string(),
                    };

                    barrier.wait();
                    let started_at = Instant::now();
                    let result = persist_termination_transition_with_death_context(
                        settings.as_ref(),
                        &lifecycle,
                        &life_record,
                        Some("peak_death"),
                        Some(&lifespan_event),
                    );
                    let mut metrics = WriteBatchMetrics::default();
                    metrics.record(started_at, result);
                    metrics
                })
            })
            .collect::<Vec<_>>();

        let batch_started = Instant::now();
        barrier.wait();
        let metrics = handles
            .into_iter()
            .map(|handle| handle.join().expect("death writer should not panic"))
            .fold(WriteBatchMetrics::default(), WriteBatchMetrics::merge);
        let elapsed = batch_started.elapsed();
        let lock_failures = metrics
            .errors
            .iter()
            .filter(|error| error.contains("locked") || error.contains("busy"))
            .count();
        eprintln!(
            "[phase9] semantic death peak: writes={} elapsed_ms={} max_write_ms={} lock_failures={} failure_rate={:.4}",
            metrics.writes,
            elapsed.as_millis(),
            metrics.max_write_ms,
            lock_failures,
            metrics.errors.len() as f64 / metrics.writes as f64
        );
        assert!(
            metrics.errors.is_empty(),
            "10 concurrent termination events should persist atomically: {:?}",
            metrics.errors
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

        let index_path = settings.deceased_public_dir().join("_index.json");
        let index_entries: Vec<DeceasedIndexEntry> = serde_json::from_str(
            fs::read_to_string(&index_path)
                .expect("deceased index should exist")
                .as_str(),
        )
        .expect("deceased index should decode");
        assert_eq!(index_entries.len(), writer_count);

        for index in 0..writer_count {
            let char_id = format!("offline:PeakDeath{index}");
            assert!(
                index_path
                    .parent()
                    .expect("deceased dir should exist")
                    .join(format!("offline_PeakDeath{index}.json"))
                    .exists(),
                "public deceased snapshot for {char_id} should exist"
            );
            assert!(
                index_entries.iter().any(|entry| entry.char_id == char_id
                    && entry.path == format!("deceased/offline_PeakDeath{index}.json")),
                "deceased index should contain {char_id}"
            );
        }

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

        let player_persistence = PlayerStatePersistence::with_db_path(
            root.join("data").join("players"),
            settings.db_path(),
        );
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

        let player_persistence = PlayerStatePersistence::with_db_path(
            root.join("data").join("players"),
            settings.db_path(),
        );
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
                        zones: vec![crate::world::zone::Zone {
                            name: format!("mixed_zone_{index}"),
                            dimension: crate::world::dimension::DimensionKind::Overworld,
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

        let player_persistence = PlayerStatePersistence::with_db_path(
            root.join("data").join("players"),
            settings.db_path(),
        );
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
                            karma: (0.1 * batch as f64).clamp(-1.0, 1.0),
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
                                dimension: crate::world::dimension::DimensionKind::Overworld,
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
    fn load_npc_deceased_archive_rejects_corrupted_zstd_bundle() {
        let (settings, root) = persistence_settings("npc-archive-corrupt-read");
        bootstrap_sqlite(settings.db_path(), settings.server_run_id())
            .expect("bootstrap should succeed");

        let capture = sample_npc_capture("npc_archive_corrupt");
        persist_npc_capture(&settings, &capture)
            .expect("npc capture should persist before archive");

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
        );
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
        );
        let indexed_path = npc_deceased_archive_absolute_path(
            &settings,
            indexed_archive.char_id.as_str(),
            indexed_archive.archived_at_wall,
        );
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
