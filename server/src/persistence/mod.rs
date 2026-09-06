use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use big_brain::prelude::{ActionState, Actor};
use rusqlite::{params, types::Type, Connection, OptionalExtension, TransactionBehavior};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use uuid::Uuid;
use valence::prelude::bevy_ecs;
use valence::prelude::bevy_ecs::event::{Events, ManualEventReader};
use valence::prelude::bevy_ecs::schedule::SystemSet;
use valence::prelude::{
    Added, App, AppExit, Changed, Client, Commands, Component, DVec3, Despawned, Entity,
    EntityKind, EventReader, IntoSystemConfigs, Last, Position, Query, Res, ResMut, Resource,
    Startup, Update, Username, With, Without, World,
};

use crate::combat::components::{Lifecycle, LifecycleState};
use crate::cultivation::components::{Contamination, Cultivation, MeridianSystem, Realm};
use crate::cultivation::known_techniques::{
    KnownTechniques, KnownTechniquesLoadFailed, KnownTechniquesReconnectBlocked,
    KnownTechniquesReconnectFailed, KnownTechniquesReconnectReady, TechniqueRegistry,
};
use crate::cultivation::life_record::{BiographyEntry, DeathInsightRecord, LifeRecord};
use crate::cultivation::tick::CultivationClock;
use crate::cultivation::void::components::{VoidActionCooldowns, VoidActionKind};
use crate::inventory::{DroppedLootEntry, JS_SAFE_INTEGER_MAX};
use crate::npc::brain::{canonical_npc_id, ChaseAction, DashAction, FleeAction, MeleeAttackAction};
use crate::npc::movement::{MovementController, MovementCooldowns, MovementMode};
use crate::npc::patrol::NpcPatrol;
use crate::npc::spawn::{NpcBlackboard, NpcCombatLoadout, NpcMarker, NpcMeleeArchetype};
use crate::player::state::{
    canonical_player_id, load_player_known_techniques_slice, open_player_connection,
    player_username_from_character_id, PlayerStatePersistence, PLAYER_ROW_SCHEMA_VERSION,
};
#[cfg(test)]
use crate::qi_physics::ledger::pending_inflow_account;
use crate::qi_physics::ledger::{
    persistent_runtime_qi_accounts, QiAccountId, WorldQiAccount, DYING_ELDER_DAN_EXCESS_ACCOUNT_ID,
    DYING_ELDER_RELEASE_OVERFLOW_ACCOUNT_ID, PENDING_INFLOW_ACCOUNT_ID,
    QI_FLOW_OVERFLOW_ACCOUNT_ID, RIFT_DRAIN_ACCOUNT_ID,
};
use crate::schema::common::NpcStateKind;
use crate::schema::pseudo_vein::PseudoVeinSeasonV1;
use crate::schema::social::{
    ExposureKindV1, FactionMembershipSnapshotV1, RelationshipKindV1, RelationshipSnapshotV1,
    RenownTagV1,
};
use crate::world::dimension::DimensionKind;
use crate::world::heartbeat::{
    is_heartbeat_pseudo_vein_zone_id, is_heartbeat_pseudo_vein_zone_namespace,
    validate_persisted_pseudo_vein_record, WorldHeartbeat, EVENT_PSEUDO_VEIN,
    HEARTBEAT_EVAL_INTERVAL_TICKS,
};

#[allow(dead_code)]
pub mod identity;
#[allow(dead_code)]
pub mod slice;

use slice::{
    dispatch_reconnect_handoff, reconnect_handoff_token, AutosavePolicy, DirtyAcknowledgement,
    DirtyRevision, DirtyTracker, GuardedSlice, LoadFailurePolicy, PersistedRevisionFence,
    PersistenceSlice, PersistenceSliceRegistry, ReconnectHandoffReport, ShutdownFlushRequest,
    SliceClock, SliceDescriptor, SliceId, SliceLoad, SliceRunContext, SliceRunError,
    SliceRunOutcome, SliceRunReason, SliceRunResult, SliceScope, TimeBasis, WriteAuthority,
    WriteBinding, WriteDomain, WriteOrdering, WriteOutlet,
};

pub const DEFAULT_DATABASE_PATH: &str = "data/bong.db";
// NPC 自动保存批次会让 WAL 数据库出现合法的并发写等待；phase-9 回归同时
// 驱动 20 个写入线程，15 秒不足以覆盖队列尾部，导致可恢复的 `SQLITE_BUSY`
// 被误报为保存失败。保持有界等待，同时覆盖既定批次压力范围。
pub const SQLITE_BUSY_TIMEOUT_MS: u64 = 30_000;
/// v33 新增伪灵脉 runtime；v34 持久化 pending inflow；v35 保存年龄/调度相位；
/// v36/v37 分别持久化锻造会话与掉落；v38 新增两项垂死大能稳定 overflow 池；
/// v39 新增 `player_lifecycle`（bughunt player-lifecycle-relog-death-consequence-wipe：
/// 断线重连此前从未持久化 `Lifecycle` 死亡/复活状态机，`fortune_remaining`/
/// `awaiting_decision`/`state` 全部被 `Lifecycle::default()` 抹回满状态新角色）；
/// v40 持久化 R5 真元事务固定 overflow 池；v41 持久化坍缩渊 drain 固定池；
/// v42 新增 dormant 终局 tombstone，跨 SQLite sink 与 Redis source deletion 防重放；
/// v43 移除已退役亡者公开站点的 `deceased_snapshots.public_path` 投影字段；
/// v44 破坏性清理已退役的 `legacy_letterbox` 表及其索引，不保留兼容数据。
const CURRENT_USER_VERSION: i32 = 44;
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
// fix/world-model-schema-drift：这三个字段名必须与 agent 侧
// WORLD_MODEL_STATE_FIELDS（redis-ipc.ts）逐字对齐，否则 mirror 回读又会静默丢字段。
pub const WORLD_MODEL_STATE_FIELD_NEG_DOMAIN_PENDING_TRIBULATIONS: &str =
    "neg_domain_pending_tribulations";
pub const WORLD_MODEL_STATE_FIELD_NEG_DOMAIN_ESCAPE_TELEMETRY: &str = "neg_domain_escape_telemetry";
pub const WORLD_MODEL_STATE_FIELD_NEG_DOMAIN_ESCAPE_SESSIONS: &str = "neg_domain_escape_sessions";
pub const WORLD_MODEL_STATE_FIELD_LAST_TICK: &str = "last_tick";
pub const WORLD_MODEL_STATE_FIELD_LAST_STATE_TS: &str = "last_state_ts";
const CURRENT_SCHEMA_VERSION: i32 = 1;
const EVENT_SCHEMA_VERSION: i32 = 1;
const EVENT_PAYLOAD_VERSION: i32 = 1;
pub const ZONE_OVERLAY_PAYLOAD_VERSION: i32 = 2;
const NPC_ROW_SCHEMA_VERSION: i32 = 1;
const NPC_DIGEST_RETENTION_SECS: i64 = 180 * 24 * 60 * 60;
const NPC_DIGEST_SWEEP_INTERVAL_SECS: i64 = 7 * 24 * 60 * 60;
/// plan-offscreen-war-v1 P3：未被玩家到访的离屏战场遗物的留存窗口（墙钟秒）。
///
/// 战死结算时把待物化遗物写进 `pending_dormant_relics`，玩家靠近 hydrate 才物化成 ground
/// loot。但末法残土处处战场——若**永无玩家**到访，这些 pending 行会无限堆积。TTL sweep
/// 在 `created_wall` 早于 `now - PENDING_RELIC_RETENTION_SECS`（默认 30 分钟）时清掉它们：
/// 那片战场玩家半小时没来，散落的骨片残卷也就随风化去了（叙事自洽 + 不留数据库孤儿）。
const PENDING_RELIC_RETENTION_SECS: i64 = 30 * 60;
/// 遗物 TTL sweep 的最小重跑间隔（墙钟秒，仿 [`NPC_DIGEST_SWEEP_INTERVAL_SECS`] 的手动限频）。
/// 比 digest 的 7 天短得多——遗物留存窗口本就只有 30 分钟，sweep 每 5 分钟扫一次足够及时。
const PENDING_RELIC_SWEEP_INTERVAL_SECS: i64 = 5 * 60;
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

/// plan-offscreen-war-v1 P3：战场遗物 TTL sweep 的手动限频状态（仿 [`NpcDigestSweepState`]）。
#[derive(Debug, Default)]
struct DormantRelicSweepState {
    last_sweep_wall: i64,
}

impl Resource for DormantRelicSweepState {}

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

/// plan-territory-v1 P0：zone_influence snapshot 的节流状态。
#[derive(Debug, Default)]
struct ZoneInfluenceSnapshotState {
    last_snapshot_wall: i64,
}

impl Resource for ZoneInfluenceSnapshotState {}

#[derive(Debug, Default)]
struct PersistenceShutdownReader(ManualEventReader<AppExit>);

impl Resource for PersistenceShutdownReader {}

#[derive(Debug, Clone, Copy)]
struct ProductionSliceClock {
    runtime_tick: u64,
    wall_unix_millis: u64,
}

impl SliceClock for ProductionSliceClock {
    fn runtime_tick(&self) -> u64 {
        self.runtime_tick
    }

    fn wall_unix_millis(&self) -> u64 {
        self.wall_unix_millis
    }
}

struct ZoneRuntimePersistenceSlice;

impl PersistenceSlice for ZoneRuntimePersistenceSlice {
    fn descriptor() -> &'static SliceDescriptor {
        &ZONE_RUNTIME_SLICE_DESCRIPTOR
    }
}

const ZONE_RUNTIME_SLICE_DESCRIPTOR: SliceDescriptor = SliceDescriptor {
    id: SliceId::new("world.zone_runtime"),
    scope: SliceScope::WorldResource,
    order: 100,
    load_failure: LoadFailurePolicy::RefuseStartup,
    time_basis: TimeBasis::None,
    write_binding: WriteBinding::new(
        WriteDomain::new("world.zone_runtime"),
        WriteAuthority::new("persistence.zone_runtime"),
    ),
    write_ordering: WriteOrdering::Serialized,
    autosave: AutosavePolicy::Disabled,
    hydrate: None,
    reconnect_preflight: None,
    reconnect_cleanup: None,
    rebase: None,
    disconnect_save: None,
    shutdown_flush: Some(flush_zone_runtime_slice),
};

struct KnownTechniquesPersistenceSlice;

impl PersistenceSlice for KnownTechniquesPersistenceSlice {
    fn descriptor() -> &'static SliceDescriptor {
        &KNOWN_TECHNIQUES_SLICE_DESCRIPTOR
    }
}

const KNOWN_TECHNIQUES_SLICE_ID: SliceId = SliceId::new("player.known_techniques");
const KNOWN_TECHNIQUES_SLICE_DESCRIPTOR: SliceDescriptor = SliceDescriptor {
    id: KNOWN_TECHNIQUES_SLICE_ID,
    scope: SliceScope::PlayerEntity,
    order: 10,
    load_failure: LoadFailurePolicy::BlockWrites,
    time_basis: TimeBasis::None,
    write_binding: WriteBinding::new(
        WriteDomain::new("player.known_techniques"),
        WriteAuthority::new("persistence.known_techniques"),
    ),
    write_ordering: WriteOrdering::Serialized,
    autosave: AutosavePolicy::Disabled,
    hydrate: Some(hydrate_known_techniques_slice),
    reconnect_preflight: Some(preflight_known_techniques_slice),
    reconnect_cleanup: Some(cleanup_known_techniques_slice),
    rebase: None,
    disconnect_save: Some(save_known_techniques_disconnect_slice),
    shutdown_flush: Some(flush_known_techniques_shutdown_slice),
};

#[derive(Debug)]
struct KnownTechniquesActivation {
    entity: Entity,
    guarded: GuardedSlice<KnownTechniques, String>,
    tracker: DirtyTracker,
    fence: PersistedRevisionFence,
}

#[derive(Debug, Default)]
struct KnownTechniquesActivations(HashMap<String, KnownTechniquesActivation>);

impl Resource for KnownTechniquesActivations {}

#[derive(Debug, Default)]
struct PendingKnownTechniquesHandoffs(HashMap<String, Entity>);

impl Resource for PendingKnownTechniquesHandoffs {}

#[derive(Debug, Default)]
struct PendingKnownTechniquesCandidates(HashMap<String, Vec<Entity>>);

impl Resource for PendingKnownTechniquesCandidates {}

#[derive(Debug, Default)]
struct KnownTechniquesRetryEntry {
    attempts: u8,
    next_attempt_frame: u64,
    next_log_frame: u64,
}

#[derive(Debug, Default)]
struct KnownTechniquesReconnectState {
    frame: u64,
    retries: HashMap<String, KnownTechniquesRetryEntry>,
    preflight_loads: Mutex<HashMap<String, Result<Option<KnownTechniques>, String>>>,
}

impl Resource for KnownTechniquesReconnectState {}

const KNOWN_TECHNIQUES_RETRY_MAX_ATTEMPTS: u8 = 8;
const KNOWN_TECHNIQUES_RETRY_MAX_BACKOFF_FRAMES: u64 = 64;
const KNOWN_TECHNIQUES_RETRY_LOG_INTERVAL_FRAMES: u64 = 64;

fn begin_known_techniques_retry(
    state: &mut KnownTechniquesReconnectState,
    subject: &str,
    frame: u64,
) -> bool {
    let entry = state.retries.entry(subject.to_string()).or_default();
    if frame < entry.next_attempt_frame {
        return false;
    }
    entry.attempts = entry
        .attempts
        .saturating_add(1)
        .min(KNOWN_TECHNIQUES_RETRY_MAX_ATTEMPTS);
    true
}

fn record_known_techniques_retry_failure(
    state: &mut KnownTechniquesReconnectState,
    subject: &str,
    frame: u64,
) -> bool {
    let entry = state.retries.entry(subject.to_string()).or_default();
    let capped = entry.attempts >= KNOWN_TECHNIQUES_RETRY_MAX_ATTEMPTS;
    if capped {
        entry.attempts = KNOWN_TECHNIQUES_RETRY_MAX_ATTEMPTS;
    }
    let backoff_shift = entry.attempts.saturating_sub(1).min(6);
    let backoff = 1_u64 << backoff_shift;
    entry.next_attempt_frame =
        frame.saturating_add(backoff.min(KNOWN_TECHNIQUES_RETRY_MAX_BACKOFF_FRAMES));
    capped
}

fn known_techniques_retry_log_allowed(
    state: &mut KnownTechniquesReconnectState,
    subject: &str,
    frame: u64,
) -> bool {
    let entry = state.retries.entry(subject.to_string()).or_default();
    if frame < entry.next_log_frame {
        return false;
    }
    entry.next_log_frame = frame.saturating_add(KNOWN_TECHNIQUES_RETRY_LOG_INTERVAL_FRAMES);
    true
}

fn clear_known_techniques_retry(state: &mut KnownTechniquesReconnectState, subject: &str) {
    state.retries.remove(subject);
}

fn known_techniques_live_activation(world: &World, subject: &str) -> Option<Entity> {
    world
        .resource::<KnownTechniquesActivations>()
        .0
        .get(subject)
        .filter(|activation| world.get::<Client>(activation.entity).is_some())
        .map(|activation| activation.entity)
}

fn reconnect_report_is_live_duplicate(report: &ReconnectHandoffReport) -> bool {
    report.failures.iter().any(|failure| {
        failure.reason == SliceRunReason::ReconnectPreflight
            && failure.error.message() == "known techniques subject already has a live activation"
    })
}

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
            server_run_id: Uuid::now_v7().to_string(),
        }
    }
}

impl PersistenceSettings {
    pub fn with_db_path(db_path: impl Into<PathBuf>, server_run_id: impl Into<String>) -> Self {
        Self {
            db_path: db_path.into(),
            server_run_id: server_run_id.into(),
        }
    }

    /// 旧测试夹具兼容构造器。亡者索引已并入 SQLite，历史调用方传入的目录不再参与
    /// 持久化；保留该入口可让跨模块回归测试继续复用同一套 fixture。
    #[cfg(test)]
    pub fn with_paths(
        db_path: impl Into<PathBuf>,
        _deceased_dir: impl Into<PathBuf>,
        server_run_id: impl Into<String>,
    ) -> Self {
        Self::with_db_path(db_path, server_run_id)
    }

    pub fn db_path(&self) -> &Path {
        self.db_path.as_path()
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

/// plan-offscreen-war-v1 P3：`pending_dormant_relics` 表的一行——一名克制判定通过的离屏
/// 战死者待物化的战场遗物。与列 1:1 映射（仿 [`NpcDigestRecord`]）。
///
/// **零真元不变量**：本记录**不含**任何真元字段——遗物 loot 物化时显式 `spirit_quality=0`，
/// 持久层完全不碰 `WorldQiAccount` / ledger（§10.1 #5 ④红线）。`loot_seed` 是 u64
/// deterministic 种子，但 sqlite 无 u64 → 存取走 `loot_seed as i64` / `i64 as u64` 位投影
/// （无损往返，见 [`upsert_pending_dormant_relic`] / [`load_pending_dormant_relics_for_zone`]）。
#[derive(Debug, Clone, PartialEq)]
pub struct PendingDormantRelicRecord {
    pub relic_id: String,
    pub char_id: String,
    pub zone: String,
    pub pos_x: f64,
    pub pos_y: f64,
    pub pos_z: f64,
    pub archetype: String,
    pub loot_seed: u64,
    pub created_tick: i64,
    pub created_wall: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersistDormantTerminalOutcome {
    Committed,
    AlreadyCommitted,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DormantTerminalCommitRecord {
    pub char_id: String,
    pub cause: String,
    pub at_tick: u64,
    pub zone: String,
    pub winner: Option<String>,
    pub winner_group: Option<u64>,
    pub loser_group: Option<u64>,
    pub zone_accepted: f64,
    pub cleanup_revision: Option<u64>,
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

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AgentWorldModelSnapshotRecord {
    #[serde(default)]
    pub current_era: Option<serde_json::Value>,
    #[serde(default)]
    pub zone_history: BTreeMap<String, Vec<serde_json::Value>>,
    #[serde(default)]
    pub last_decisions: BTreeMap<String, AgentWorldModelDecisionRecord>,
    #[serde(default)]
    pub player_first_seen_tick: BTreeMap<String, i64>,
    // fix/world-model-schema-drift：#[serde(default)] 用于容忍升级前（无这三个
    // 字段）的旧 SQLite snapshot_json blob；新写入的快照必须携带完整数据。
    #[serde(default)]
    pub neg_domain_pending_tribulations:
        BTreeMap<String, AgentWorldModelNegDomainPendingTribulationRecord>,
    #[serde(default)]
    pub neg_domain_escape_telemetry: AgentWorldModelNegDomainEscapeTelemetryRecord,
    #[serde(default)]
    pub neg_domain_escape_sessions: BTreeMap<String, AgentWorldModelNegDomainEscapeSessionRecord>,
    #[serde(default)]
    pub last_tick: Option<i64>,
    #[serde(default)]
    pub last_state_ts: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentWorldModelNegDomainPendingTribulationRecord {
    pub player_uuid: String,
    pub player_name: String,
    pub zone: String,
    pub entered_at_tick: i64,
    pub last_suppressed_tick: i64,
    pub reason: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AgentWorldModelNegDomainEscapeTelemetryRecord {
    pub escape_entry_count: i64,
    pub post_escape_realm_drop_count: i64,
    pub successful_tribulation_avoidance_count: i64,
    pub active_escape_session_count: i64,
    pub post_escape_realm_drop_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentWorldModelNegDomainEscapeSessionRecord {
    pub player_uuid: String,
    pub player_name: String,
    pub zone: String,
    pub entered_at_tick: i64,
    pub entry_realm_rank: f64,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HeartbeatPseudoVeinRecord {
    pub zone_id: String,
    pub dimension: DimensionKind,
    pub bounds_min: [f64; 3],
    pub bounds_max: [f64; 3],
    pub danger_level: u8,
    pub active_events: Vec<String>,
    pub patrol_anchors: Vec<[f64; 3]>,
    pub center_xz: [f64; 2],
    pub spawned_at_tick: u64,
    pub last_tick: u64,
    pub qi_current: f64,
    pub total_qi_consumed: f64,
    pub warning_sent: bool,
    pub dissipated: bool,
    pub season_at_spawn: PseudoVeinSeasonV1,
    pub observed_age_ticks: u64,
    pub pending_runtime_ticks: u64,
    pub pending_offline_ticks: u64,
    pub occupant_count: usize,
    pub eval_elapsed_ticks: u64,
    pub snapshot_wall: i64,
}

/// plan-territory-v1 P0：区域影响力持久化记录（zone_influence 表一行）。
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ZoneInfluenceRecord {
    pub zone_id: String,
    pub char_id: String,
    pub value: f64,
    pub meditation_ticks: u64,
    pub combat_wins: u32,
    pub player_kills: u32,
    pub gather_count: u32,
    pub continuous_sessions: u32,
    pub last_activity_tick: u64,
    /// dominant=true 表示该 char_id 是此 zone 的当前霸主。
    pub dominant: bool,
    pub established_tick: u64,
    pub public_known: bool,
    pub schema_version: i32,
    pub last_updated_wall: i64,
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

const KNOWN_TECHNIQUES_UPSERT: &str = "
    INSERT INTO player_known_techniques (
        username,
        known_techniques_json,
        schema_version,
        last_updated_wall
    ) VALUES (?1, ?2, ?3, ?4)
    ON CONFLICT(username) DO UPDATE SET
        known_techniques_json = excluded.known_techniques_json,
        schema_version = excluded.schema_version,
        last_updated_wall = excluded.last_updated_wall
";

fn persist_known_techniques_activation(
    activation: &mut KnownTechniquesActivation,
    persistence: &PlayerStatePersistence,
    outlet: WriteOutlet,
) -> Result<SliceRunOutcome, SliceRunError> {
    let permit = activation
        .guarded
        .write_permit(outlet)
        .map_err(|error| SliceRunError::new(error.to_string()))?;
    let Some(snapshot) = activation
        .tracker
        .begin_snapshot(permit, Clone::clone)
        .map_err(|error| SliceRunError::new(error.to_string()))?
    else {
        return Ok(SliceRunOutcome::Clean);
    };
    let username = player_username_from_character_id(snapshot.subject_key().as_str())
        .ok_or_else(|| SliceRunError::new("known techniques subject is not a player identity"))?
        .to_string();
    let known_techniques_json = serde_json::to_string(snapshot.payload())
        .map_err(|error| SliceRunError::new(error.to_string()))?;
    let mut connection = open_player_connection(persistence)
        .map_err(|error| SliceRunError::new(error.to_string()))?;
    let receipt = activation
        .fence
        .commit(&mut connection, snapshot, |request| {
            request.execute_serialized(
                KNOWN_TECHNIQUES_UPSERT,
                params![
                    username,
                    known_techniques_json,
                    PLAYER_ROW_SCHEMA_VERSION,
                    current_unix_seconds()
                ],
            )
        })
        .map_err(|error| SliceRunError::new(format!("{error:?}")))?;
    match activation.tracker.acknowledge(receipt) {
        DirtyAcknowledgement::Acknowledged => Ok(SliceRunOutcome::Flushed),
        acknowledgement => Err(SliceRunError::new(format!(
            "known techniques durable receipt was not acknowledged: {acknowledgement:?}"
        ))),
    }
}

fn sync_known_techniques_activation(
    world: &World,
    subject: &str,
    activation: &mut KnownTechniquesActivation,
) -> Result<(), SliceRunError> {
    let Some(current) = world.get::<KnownTechniques>(activation.entity) else {
        return Ok(());
    };
    if current != activation.guarded.value() {
        activation
            .guarded
            .mutate(&mut activation.tracker, |value| *value = current.clone())
            .map_err(|error| SliceRunError::new(format!("{subject}: {error}")))?;
    }
    Ok(())
}

fn hydrate_known_techniques_slice(world: &mut World, context: &SliceRunContext) -> SliceRunResult {
    let subject = context
        .handoff_key
        .as_deref()
        .ok_or_else(|| SliceRunError::new("known techniques hydrate has no subject"))?;
    let entity = world
        .resource::<PendingKnownTechniquesHandoffs>()
        .0
        .get(subject)
        .copied()
        .ok_or_else(|| SliceRunError::new("known techniques reconnect target is unavailable"))?;
    if world.get_entity(entity).is_none() {
        cleanup_stale_known_techniques_pending(world);
        return Err(SliceRunError::new(
            "known techniques reconnect target entity is gone",
        ));
    }
    validate_known_techniques_reconnect_target(world, subject, entity)?;
    let loaded = world
        .resource::<KnownTechniquesReconnectState>()
        .preflight_loads
        .lock()
        .map_err(|_| SliceRunError::new("known techniques preflight cache is poisoned"))?
        .remove(subject)
        .ok_or_else(|| SliceRunError::new("known techniques preflight load is unavailable"))?;
    let load = match loaded {
        Ok(Some(value)) => SliceLoad::loaded(value),
        Ok(None) => SliceLoad::missing(),
        Err(error) => SliceLoad::failed(error),
    };
    let activation = context.reconnect_activation()?;
    let missing_default = world
        .get_resource::<TechniqueRegistry>()
        .map_or_else(KnownTechniques::default, KnownTechniques::progression_reset);
    let missing_default_for_rebase = missing_default.clone();
    let mut guarded = world
        .resource_scope(
            |_, registry: valence::prelude::Mut<PersistenceSliceRegistry>| {
                registry.activate(
                    load,
                    KNOWN_TECHNIQUES_SLICE_ID,
                    activation,
                    DirtyRevision::default(),
                    || missing_default,
                    |_| missing_default_for_rebase,
                )
            },
        )
        .map_err(|error| SliceRunError::new(format!("activation failed: {error:?}")))?;
    let failed = guarded.load_status() == slice::SliceLoadStatus::Failed;
    let value = guarded.value().clone();
    let (tracker, fence) = guarded
        .restore_persistence_state()
        .map_err(|error| SliceRunError::new(error.to_string()))?;
    {
        let Some(mut target) = world.get_entity_mut(entity) else {
            cleanup_stale_known_techniques_pending(world);
            return Err(SliceRunError::new(
                "known techniques reconnect target entity disappeared during hydrate",
            ));
        };
        target.insert(value);
        if failed {
            target.insert(KnownTechniquesLoadFailed);
        } else {
            target.remove::<KnownTechniquesLoadFailed>();
        }
    }
    world.resource_mut::<KnownTechniquesActivations>().0.insert(
        subject.to_string(),
        KnownTechniquesActivation {
            entity,
            guarded,
            tracker,
            fence,
        },
    );
    world
        .resource_mut::<PendingKnownTechniquesHandoffs>()
        .0
        .remove(subject);
    Ok(SliceRunOutcome::Clean)
}

fn cleanup_stale_known_techniques_pending(world: &mut World) {
    let pending_subjects = world
        .resource::<PendingKnownTechniquesHandoffs>()
        .0
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    let stale_subjects = pending_subjects
        .iter()
        .filter(|subject| {
            let Some(entity) = world
                .resource::<PendingKnownTechniquesHandoffs>()
                .0
                .get(*subject)
                .copied()
            else {
                return true;
            };
            !known_techniques_reconnect_candidate_is_live(world, subject, entity)
        })
        .cloned()
        .collect::<Vec<_>>();
    for subject in stale_subjects {
        let entity = world
            .resource_mut::<PendingKnownTechniquesHandoffs>()
            .0
            .remove(&subject);
        if let Some(entity) = entity {
            if let Some(mut target) = world.get_entity_mut(entity) {
                target.remove::<KnownTechniquesReconnectBlocked>();
                target.remove::<KnownTechniquesReconnectFailed>();
                target.remove::<KnownTechniquesReconnectReady>();
            }
        }
        let mut state = world.resource_mut::<KnownTechniquesReconnectState>();
        state.retries.remove(&subject);
        if let Ok(mut loads) = state.preflight_loads.lock() {
            loads.remove(&subject);
        };
    }

    let subjects = world
        .resource::<PendingKnownTechniquesCandidates>()
        .0
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    let mut stale_candidates = Vec::new();
    for subject in subjects {
        let candidates = world
            .resource::<PendingKnownTechniquesCandidates>()
            .0
            .get(&subject)
            .cloned()
            .unwrap_or_default();
        let live = candidates
            .into_iter()
            .filter(|entity| known_techniques_reconnect_candidate_is_live(world, &subject, *entity))
            .collect::<Vec<_>>();
        if live.is_empty() {
            stale_candidates.push(subject);
        } else {
            world
                .resource_mut::<PendingKnownTechniquesCandidates>()
                .0
                .insert(subject, live);
        }
    }
    for subject in stale_candidates {
        world
            .resource_mut::<PendingKnownTechniquesCandidates>()
            .0
            .remove(&subject);
    }
}

fn known_techniques_reconnect_candidate_is_live(
    world: &World,
    subject: &str,
    entity: Entity,
) -> bool {
    let Some(target) = world.get_entity(entity) else {
        return false;
    };
    let Some(username) = target.get::<Username>() else {
        return false;
    };
    target.get::<Client>().is_some()
        && target.get::<Despawned>().is_none()
        && player_username_from_character_id(subject).is_some_and(|expected| username.0 == expected)
}

fn promote_known_techniques_candidate(world: &mut World, subject: &str) {
    if world
        .resource::<PendingKnownTechniquesHandoffs>()
        .0
        .contains_key(subject)
        || world
            .resource::<KnownTechniquesActivations>()
            .0
            .contains_key(subject)
    {
        return;
    }
    let candidate = world
        .resource::<PendingKnownTechniquesCandidates>()
        .0
        .get(subject)
        .and_then(|candidates| {
            candidates
                .iter()
                .copied()
                .min_by_key(|entity| entity.index())
        });
    let Some(entity) = candidate else {
        return;
    };
    world
        .resource_mut::<PendingKnownTechniquesCandidates>()
        .0
        .entry(subject.to_string())
        .and_modify(|candidates| candidates.retain(|candidate| *candidate != entity));
    if let Some(mut target) = world.get_entity_mut(entity) {
        target.remove::<KnownTechniquesReconnectBlocked>();
        target.remove::<KnownTechniquesReconnectFailed>();
    }
    world
        .resource_mut::<PendingKnownTechniquesHandoffs>()
        .0
        .insert(subject.to_string(), entity);
}

fn validate_known_techniques_reconnect_target(
    world: &World,
    subject: &str,
    entity: Entity,
) -> Result<(), SliceRunError> {
    let Some(target) = world.get_entity(entity) else {
        return Err(SliceRunError::new(
            "known techniques reconnect target entity is gone",
        ));
    };
    let Some(client) = target.get::<Client>() else {
        return Err(SliceRunError::new(
            "known techniques reconnect target is disconnected",
        ));
    };
    let username = target
        .get::<Username>()
        .ok_or_else(|| SliceRunError::new("known techniques reconnect target has no username"))?;
    let expected = player_username_from_character_id(subject)
        .ok_or_else(|| SliceRunError::new("known techniques subject is not a player identity"))?;
    if username.0 != expected {
        return Err(SliceRunError::new(
            "known techniques reconnect target identity mismatch",
        ));
    }
    let _ = client;
    if target.get::<Despawned>().is_some() {
        return Err(SliceRunError::new(
            "known techniques reconnect target is despawned",
        ));
    }
    Ok(())
}

fn preflight_known_techniques_slice(
    world: &mut World,
    context: &SliceRunContext,
) -> SliceRunResult {
    let subject = context
        .handoff_key
        .as_deref()
        .ok_or_else(|| SliceRunError::new("known techniques preflight has no subject"))?;
    let target = world
        .resource::<PendingKnownTechniquesHandoffs>()
        .0
        .get(subject)
        .copied()
        .ok_or_else(|| SliceRunError::new("known techniques reconnect target is unavailable"))?;
    validate_known_techniques_reconnect_target(world, subject, target)?;
    if known_techniques_live_activation(world, subject).is_some() {
        return Err(SliceRunError::new(
            "known techniques subject already has a live activation",
        ));
    }
    let has_activation = world
        .resource::<KnownTechniquesActivations>()
        .0
        .contains_key(subject);
    let persistence = world
        .get_resource::<PlayerStatePersistence>()
        .cloned()
        .ok_or_else(|| SliceRunError::new("PlayerStatePersistence is unavailable"))?;
    let username = player_username_from_character_id(subject)
        .ok_or_else(|| SliceRunError::new("known techniques subject is not a player identity"))?;
    let loaded = load_player_known_techniques_slice(&persistence, username);
    let cached = match loaded {
        Ok(value) => Ok(value),
        Err(error) if !has_activation => Err(error.to_string()),
        Err(error) => return Err(SliceRunError::new(error.to_string())),
    };
    world
        .resource::<KnownTechniquesReconnectState>()
        .preflight_loads
        .lock()
        .map_err(|_| SliceRunError::new("known techniques preflight cache is poisoned"))?
        .insert(subject.to_string(), cached);
    Ok(SliceRunOutcome::Clean)
}

fn cleanup_known_techniques_slice(world: &mut World, context: &SliceRunContext) {
    let Some(subject) = context.handoff_key.as_deref() else {
        return;
    };
    if let Some(activation) = world
        .resource_mut::<KnownTechniquesActivations>()
        .0
        .remove(subject)
    {
        if let Some(mut entity) = world.get_entity_mut(activation.entity) {
            entity.remove::<KnownTechniques>();
            entity.remove::<KnownTechniquesLoadFailed>();
        }
    }
}

fn save_known_techniques_disconnect_slice(
    world: &mut World,
    context: &SliceRunContext,
) -> SliceRunResult {
    let subject = context
        .handoff_key
        .as_deref()
        .ok_or_else(|| SliceRunError::new("known techniques disconnect save has no subject"))?;
    let persistence = world
        .get_resource::<PlayerStatePersistence>()
        .cloned()
        .ok_or_else(|| SliceRunError::new("PlayerStatePersistence is unavailable"))?;
    world.resource_scope(
        |world, mut activations: valence::prelude::Mut<KnownTechniquesActivations>| {
            let Some(activation) = activations.0.get_mut(subject) else {
                return Ok(SliceRunOutcome::Clean);
            };
            if activation.guarded.load_status() == slice::SliceLoadStatus::Failed {
                return Ok(SliceRunOutcome::Clean);
            }
            sync_known_techniques_activation(world, subject, activation)?;
            persist_known_techniques_activation(activation, &persistence, WriteOutlet::Disconnect)
        },
    )
}

fn flush_known_techniques_shutdown_slice(
    world: &mut World,
    _context: &SliceRunContext,
) -> SliceRunResult {
    let persistence = world
        .get_resource::<PlayerStatePersistence>()
        .cloned()
        .ok_or_else(|| SliceRunError::new("PlayerStatePersistence is unavailable"))?;
    world.resource_scope(
        |world, mut activations: valence::prelude::Mut<KnownTechniquesActivations>| {
            let mut subjects = activations.0.keys().cloned().collect::<Vec<_>>();
            subjects.sort();
            let mut flushed = false;
            let mut failures = Vec::new();
            for subject in subjects {
                let Some(activation) = activations.0.get_mut(&subject) else {
                    continue;
                };
                if activation.guarded.load_status() == slice::SliceLoadStatus::Failed {
                    continue;
                }
                let result = (|| -> Result<SliceRunOutcome, SliceRunError> {
                    sync_known_techniques_activation(world, &subject, activation)?;
                    persist_known_techniques_activation(
                        activation,
                        &persistence,
                        WriteOutlet::Shutdown,
                    )
                })();
                match result {
                    Ok(SliceRunOutcome::Flushed) => flushed = true,
                    Ok(SliceRunOutcome::Clean | SliceRunOutcome::SkippedBlocked) => {}
                    Err(error) => failures.push(format!("{subject}: {error}")),
                }
            }
            if failures.is_empty() {
                Ok(if flushed {
                    SliceRunOutcome::Flushed
                } else {
                    SliceRunOutcome::Clean
                })
            } else {
                Err(SliceRunError::new(format!(
                    "known techniques shutdown flush failed: {}",
                    failures.join("; ")
                )))
            }
        },
    )
}

fn production_slice_clock(world: &World) -> ProductionSliceClock {
    ProductionSliceClock {
        runtime_tick: world
            .get_resource::<CultivationClock>()
            .map_or(0, |clock| clock.tick),
        wall_unix_millis: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_millis() as u64),
    }
}

pub(crate) fn dispatch_known_techniques_reconnects(world: &mut World) {
    let frame = {
        let mut state = world.resource_mut::<KnownTechniquesReconnectState>();
        state.frame = state.frame.saturating_add(1);
        state.frame
    };

    cleanup_stale_known_techniques_pending(world);

    let mut added_query = world.query_filtered::<(Entity, &Username), Added<Client>>();
    let added = added_query
        .iter(world)
        .map(|(entity, username)| (canonical_player_id(username.0.as_str()), entity))
        .collect::<Vec<_>>();
    for (subject, entity) in added {
        let already_pending = world
            .resource::<PendingKnownTechniquesHandoffs>()
            .0
            .contains_key(&subject);
        if already_pending {
            world
                .entity_mut(entity)
                .insert(KnownTechniquesReconnectBlocked);
            world
                .resource_mut::<PendingKnownTechniquesCandidates>()
                .0
                .entry(subject.clone())
                .or_default()
                .push(entity);
            tracing::warn!(
                "[bong][persistence] rejecting duplicate known techniques reconnect target for `{subject}`"
            );
            continue;
        }
        world
            .resource_mut::<PendingKnownTechniquesHandoffs>()
            .0
            .insert(subject, entity);
    }

    let candidate_subjects = world
        .resource::<PendingKnownTechniquesCandidates>()
        .0
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    for subject in candidate_subjects {
        promote_known_techniques_candidate(world, &subject);
    }

    let disconnected_subjects = world
        .resource::<KnownTechniquesActivations>()
        .0
        .iter()
        .filter(|(_, activation)| world.get::<Client>(activation.entity).is_none())
        .map(|(subject, _)| subject.clone())
        .collect::<Vec<_>>();

    let persistence = world.get_resource::<PlayerStatePersistence>().cloned();
    let save_subjects = disconnected_subjects
        .into_iter()
        .filter(|subject| {
            !world
                .resource::<PendingKnownTechniquesHandoffs>()
                .0
                .contains_key(subject)
        })
        .collect::<Vec<_>>();
    for subject in save_subjects {
        let should_attempt = {
            let mut state = world.resource_mut::<KnownTechniquesReconnectState>();
            begin_known_techniques_retry(&mut state, &subject, frame)
        };
        if !should_attempt {
            continue;
        }
        let result = persistence.as_ref().map_or_else(
            || Err(SliceRunError::new("PlayerStatePersistence is unavailable")),
            |persistence| {
                world.resource_scope(
                    |world, mut activations: valence::prelude::Mut<KnownTechniquesActivations>| {
                        let Some(activation) = activations.0.get_mut(&subject) else {
                            return Ok(SliceRunOutcome::Clean);
                        };
                        if activation.guarded.load_status() == slice::SliceLoadStatus::Failed {
                            return Ok(SliceRunOutcome::Clean);
                        }
                        sync_known_techniques_activation(world, &subject, activation)?;
                        persist_known_techniques_activation(
                            activation,
                            persistence,
                            WriteOutlet::Disconnect,
                        )
                    },
                )
            },
        );
        match result {
            Ok(_) => {
                world
                    .resource_mut::<KnownTechniquesActivations>()
                    .0
                    .remove(&subject);
                clear_known_techniques_retry(
                    &mut world.resource_mut::<KnownTechniquesReconnectState>(),
                    &subject,
                );
            }
            Err(error) => {
                let (at_retry_cap, should_log) = {
                    let mut state = world.resource_mut::<KnownTechniquesReconnectState>();
                    let at_retry_cap =
                        record_known_techniques_retry_failure(&mut state, &subject, frame);
                    let should_log =
                        known_techniques_retry_log_allowed(&mut state, &subject, frame);
                    (at_retry_cap, should_log)
                };
                if should_log {
                    if at_retry_cap {
                        tracing::error!(
                            "[bong][persistence] known techniques disconnect flush remains unavailable at the retry cap for `{subject}`; retry scheduled: {error}"
                        );
                    } else {
                        tracing::warn!(
                            "[bong][persistence] known techniques disconnect flush failed for `{subject}`; retry scheduled: {error}"
                        );
                    }
                }
            }
        }
    }

    let candidate_subjects = world
        .resource::<PendingKnownTechniquesCandidates>()
        .0
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    for subject in candidate_subjects {
        promote_known_techniques_candidate(world, &subject);
    }
    let pending_subjects = world
        .resource::<PendingKnownTechniquesHandoffs>()
        .0
        .keys()
        .cloned()
        .collect::<Vec<_>>();

    // A subject that is still pending reconnect is saved by the handoff dispatcher below;
    // keeping its retry entry here would double-count attempts and alter the handoff gate.
    for subject in &pending_subjects {
        if !world
            .resource::<PendingKnownTechniquesHandoffs>()
            .0
            .contains_key(subject)
        {
            clear_known_techniques_retry(
                &mut world.resource_mut::<KnownTechniquesReconnectState>(),
                subject,
            );
        }
    }

    for subject in pending_subjects {
        if !world
            .resource::<PendingKnownTechniquesHandoffs>()
            .0
            .contains_key(&subject)
        {
            continue;
        }
        if known_techniques_live_activation(world, &subject).is_some() {
            if let Some(entity) = world
                .resource::<PendingKnownTechniquesHandoffs>()
                .0
                .get(&subject)
                .copied()
            {
                let was_blocked = world
                    .get::<KnownTechniquesReconnectBlocked>(entity)
                    .is_some();
                world
                    .entity_mut(entity)
                    .insert(KnownTechniquesReconnectBlocked);
                if !was_blocked {
                    tracing::warn!(
                        "[bong][persistence] rejecting live duplicate known techniques reconnect target for `{subject}`"
                    );
                }
            }
            clear_known_techniques_retry(
                &mut world.resource_mut::<KnownTechniquesReconnectState>(),
                &subject,
            );
            continue;
        }

        let should_attempt = {
            let mut state = world.resource_mut::<KnownTechniquesReconnectState>();
            begin_known_techniques_retry(&mut state, &subject, frame)
        };
        if !should_attempt {
            continue;
        }

        let clock = production_slice_clock(world);
        let (succeeded, stable_live_duplicate) = match dispatch_reconnect_handoff(
            world,
            reconnect_handoff_token(subject.clone()),
            &clock,
        ) {
            Ok(report)
                if report.failures.is_empty()
                    && report.blocked_saves.is_empty()
                    && report.blocked_loads.is_empty()
                    && report.blocked_preflights.is_empty()
                    && report.blocked_rebases.is_empty() =>
            {
                (true, false)
            }
            Ok(report) => {
                let stable_live_duplicate = reconnect_report_is_live_duplicate(&report);
                let pending_entity = world
                    .resource::<PendingKnownTechniquesHandoffs>()
                    .0
                    .get(&subject)
                    .copied();
                if let Some(entity) = pending_entity {
                    world
                        .entity_mut(entity)
                        .insert(KnownTechniquesReconnectFailed);
                }
                let should_log = known_techniques_retry_log_allowed(
                    &mut world.resource_mut::<KnownTechniquesReconnectState>(),
                    &subject,
                    frame,
                );
                if should_log {
                    tracing::error!(
                        "[bong][persistence] known techniques reconnect handoff failed closed for `{subject}`: {report:?}"
                    );
                }
                (false, stable_live_duplicate)
            }
            Err(error) => {
                if let Some(entity) = world
                    .resource::<PendingKnownTechniquesHandoffs>()
                    .0
                    .get(&subject)
                    .copied()
                {
                    world
                        .entity_mut(entity)
                        .insert(KnownTechniquesReconnectFailed);
                }
                let should_log = known_techniques_retry_log_allowed(
                    &mut world.resource_mut::<KnownTechniquesReconnectState>(),
                    &subject,
                    frame,
                );
                if should_log {
                    tracing::error!(
                        "[bong][persistence] known techniques reconnect dispatch failed for `{subject}`: {error}"
                    );
                }
                (false, false)
            }
        };
        if succeeded {
            if let Some(entity) = world
                .resource::<KnownTechniquesActivations>()
                .0
                .get(&subject)
                .map(|activation| activation.entity)
            {
                let load_failed = world
                    .resource::<KnownTechniquesActivations>()
                    .0
                    .get(&subject)
                    .is_some_and(|activation| {
                        activation.guarded.load_status() == slice::SliceLoadStatus::Failed
                    });
                if let Some(mut target) = world.get_entity_mut(entity) {
                    target.remove::<KnownTechniquesReconnectBlocked>();
                    if load_failed {
                        target.remove::<KnownTechniquesReconnectReady>();
                        target.insert(KnownTechniquesReconnectFailed);
                    } else {
                        target.remove::<KnownTechniquesReconnectFailed>();
                        target.insert(KnownTechniquesReconnectReady);
                    }
                }
            }
            world
                .resource_mut::<KnownTechniquesReconnectState>()
                .retries
                .remove(&subject);
        } else if stable_live_duplicate {
            clear_known_techniques_retry(
                &mut world.resource_mut::<KnownTechniquesReconnectState>(),
                &subject,
            );
        } else {
            let mut state = world.resource_mut::<KnownTechniquesReconnectState>();
            record_known_techniques_retry_failure(&mut state, &subject, frame);
        }
    }
}

fn flush_changed_known_techniques_slices(world: &mut World) {
    let mut query = world.query_filtered::<(Entity, &Username, &KnownTechniques), (
        With<Client>,
        Changed<KnownTechniques>,
        Without<KnownTechniquesLoadFailed>,
    )>();
    let changed = query
        .iter(world)
        .map(|(entity, username, value)| {
            (
                entity,
                canonical_player_id(username.0.as_str()),
                value.clone(),
            )
        })
        .collect::<Vec<_>>();
    let Some(persistence) = world.get_resource::<PlayerStatePersistence>().cloned() else {
        return;
    };
    for (entity, subject, value) in changed {
        let result = world.resource_scope(
            |_, mut activations: valence::prelude::Mut<KnownTechniquesActivations>| {
                let Some(activation) = activations.0.get_mut(&subject) else {
                    return Ok(SliceRunOutcome::Clean);
                };
                if activation.entity != entity || activation.guarded.value() == &value {
                    return Ok(SliceRunOutcome::Clean);
                }
                activation
                    .guarded
                    .mutate(&mut activation.tracker, |guarded| *guarded = value)
                    .map_err(|error| SliceRunError::new(error.to_string()))?;
                persist_known_techniques_activation(activation, &persistence, WriteOutlet::Changed)
            },
        );
        if let Err(error) = result {
            tracing::warn!(
                "[bong][persistence] immediate known techniques flush failed for `{subject}`: {error}"
            );
        }
    }
}

pub fn register(app: &mut App) {
    let mut slice_registry = PersistenceSliceRegistry::empty();
    slice_registry
        .register_slice::<ZoneRuntimePersistenceSlice>()
        .and_then(|()| slice_registry.register_slice::<KnownTechniquesPersistenceSlice>())
        .expect("production persistence slice descriptors must be valid");

    app.insert_resource(slice_registry)
        .init_resource::<PersistenceShutdownReader>()
        .init_resource::<KnownTechniquesActivations>()
        .init_resource::<PendingKnownTechniquesHandoffs>()
        .init_resource::<PendingKnownTechniquesCandidates>()
        .init_resource::<KnownTechniquesReconnectState>()
        .init_resource::<PersistenceSettings>()
        .init_resource::<NpcSnapshotTracker>()
        .init_resource::<NpcDigestSweepState>()
        .init_resource::<DormantRelicSweepState>()
        .init_resource::<DailyBackupState>()
        .init_resource::<ZoneRuntimeSnapshotState>()
        .init_resource::<ZoneInfluenceSnapshotState>()
        .add_systems(
            Startup,
            bootstrap_persistence_system
                .in_set(PersistenceBootstrapSet)
                .after(crate::world::zone::ZoneRegistryStartupSet),
        )
        .add_systems(
            Update,
            (
                dispatch_known_techniques_reconnects
                    .before(crate::player::init_clients)
                    .before(crate::player::attach_player_state_to_joined_clients),
                flush_changed_known_techniques_slices
                    .after(crate::player::attach_player_state_to_joined_clients),
                persist_npc_runtime_state_system,
                sweep_npc_digest_retention_system,
                persist_pending_dormant_relics_system,
                sweep_dormant_relic_retention_system,
                daily_midnight_backup_system,
                persist_zone_runtime_system,
                persist_zone_influence_system,
            ),
        )
        .add_systems(Last, dispatch_persistence_shutdown_flushes);
}

#[allow(clippy::too_many_arguments)]
fn bootstrap_persistence_system(
    settings: valence::prelude::Res<PersistenceSettings>,
    mut daily_backup_state: valence::prelude::ResMut<DailyBackupState>,
    mut zones: Option<ResMut<crate::world::zone::ZoneRegistry>>,
    mut heartbeat: Option<ResMut<WorldHeartbeat>>,
    clock: Res<CultivationClock>,
    mut qi_ledger: ResMut<WorldQiAccount>,
    mut void_action_cooldowns: Option<ResMut<VoidActionCooldowns>>,
    mut zone_influence_map: Option<ResMut<crate::world::territory::ZoneInfluenceMap>>,
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

    hydrate_runtime_qi_accounts(&settings, &mut qi_ledger).unwrap_or_else(|error| {
        panic!(
            "[bong][persistence] cannot safely hydrate runtime qi accounts at {}: {error}",
            settings.db_path().display()
        )
    });

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
        if let Some(heartbeat) = heartbeat.as_deref_mut() {
            match hydrate_heartbeat_pseudo_veins(
                &settings,
                heartbeat,
                zone_registry,
                clock.tick,
                wall_clock,
            ) {
                Ok(count) if count > 0 => {
                    tracing::info!(
                        "[bong][persistence] hydrated {count} heartbeat pseudo-vein runtime record(s) from sqlite"
                    );
                }
                Ok(_) => {}
                Err(error) => panic!(
                    "[bong][persistence] refusing startup after heartbeat pseudo-vein hydrate failure at {}: {error}",
                    settings.db_path().display()
                ),
            }
        }
        if let Err(error) = hydrate_zone_runtime(&settings, zone_registry) {
            panic!(
                "[bong][persistence] refusing startup after zone runtime hydrate failure at {}: {error}",
                settings.db_path().display()
            );
        }
        if let Some(heartbeat) = heartbeat.as_deref_mut() {
            heartbeat.sync_active_pseudo_vein_qi_from_zones(zone_registry);
        }
        // Zone balances are restored only into Zone.spirit_qi. Dynamic pseudo-veins use the same
        // external owner and settle through typed Zone↔stable-pool transactions; recreating a
        // `zone:*` ledger balance here would double-count every restored pseudo-vein.
        if let Err(error) = hydrate_zone_overlays(&settings, zone_registry) {
            tracing::warn!(
                "[bong][persistence] failed to hydrate zone overlays from sqlite at {}: {error}",
                settings.db_path().display()
            );
        }
    }

    // plan-territory-v1 P0：hydrate 区域影响力（照 zones_runtime hydrate 模式）
    if let Some(influence_map) = zone_influence_map.as_deref_mut() {
        match hydrate_zone_influence(&settings, influence_map) {
            Ok(count) if count > 0 => tracing::info!(
                "[bong][persistence] hydrated {count} zone-influence record(s) from sqlite"
            ),
            Ok(_) => {}
            Err(error) => tracing::warn!(
                "[bong][persistence] failed to hydrate zone influence from sqlite at {}: {error}",
                settings.db_path().display()
            ),
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
    heartbeat: Option<Res<WorldHeartbeat>>,
    qi_ledger: Res<WorldQiAccount>,
    clock: Res<CultivationClock>,
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

    match persist_zone_runtime_snapshot_with_heartbeat_at_tick(
        &settings,
        &zone_registry,
        heartbeat.as_deref(),
        &qi_ledger,
        clock.tick,
    ) {
        Ok(_) => {
            snapshot_state.last_snapshot_wall = wall_clock;
        }
        Err(error) => tracing::warn!(
            "[bong][persistence] failed to persist zone runtime snapshot at {}: {error}",
            settings.db_path().display()
        ),
    }
}

fn flush_zone_runtime_slice(world: &mut World, _context: &SliceRunContext) -> SliceRunResult {
    if !world.contains_resource::<crate::world::zone::ZoneRegistry>() {
        return Ok(SliceRunOutcome::Clean);
    }
    if !world.contains_resource::<PersistenceSettings>() {
        return Err(SliceRunError::new("PersistenceSettings is unavailable"));
    }
    world.resource_scope(
        |world, settings: valence::prelude::Mut<PersistenceSettings>| {
            world.resource_scope(
                |world, zones: valence::prelude::Mut<crate::world::zone::ZoneRegistry>| {
                    let heartbeat = world.get_resource::<WorldHeartbeat>();
                    let qi_ledger = world
                        .get_resource::<WorldQiAccount>()
                        .ok_or_else(|| SliceRunError::new("WorldQiAccount is unavailable"))?;
                    let clock_tick = world
                        .get_resource::<CultivationClock>()
                        .ok_or_else(|| SliceRunError::new("CultivationClock is unavailable"))?
                        .tick;

                    persist_zone_runtime_snapshot_with_heartbeat_at_tick(
                        &settings, &zones, heartbeat, qi_ledger, clock_tick,
                    )
                    .map(|_| SliceRunOutcome::Flushed)
                    .map_err(|error| SliceRunError::new(error.to_string()))
                },
            )
        },
    )
}

fn dispatch_persistence_shutdown_flushes(world: &mut World) {
    let requested = world.resource_scope(
        |world, mut reader: valence::prelude::Mut<PersistenceShutdownReader>| {
            world
                .get_resource::<Events<AppExit>>()
                .is_some_and(|events| reader.0.read(events).next().is_some())
        },
    );
    let request = if requested {
        ShutdownFlushRequest::Requested
    } else {
        ShutdownFlushRequest::NotRequested
    };
    let runtime_tick = world
        .get_resource::<CultivationClock>()
        .map_or(0, |clock| clock.tick);
    let wall_unix_millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64);
    let clock = ProductionSliceClock {
        runtime_tick,
        wall_unix_millis,
    };

    match slice::dispatch_shutdown_flushes(world, request, &clock) {
        Ok(report) => {
            for failure in report.failures {
                tracing::warn!(
                    "[bong][persistence] shutdown slice `{}` failed: {}",
                    failure.slice_id,
                    failure.error
                );
            }
        }
        Err(error) => {
            tracing::error!("[bong][persistence] shutdown slice dispatch failed closed: {error}")
        }
    }
}

/// plan-territory-v1 P0：zone_influence 快照 system（节流 ZONE_RUNTIME_SNAPSHOT_INTERVAL_SECS）。
fn persist_zone_influence_system(
    settings: Res<PersistenceSettings>,
    mut snapshot_state: ResMut<ZoneInfluenceSnapshotState>,
    influence_map: Option<Res<crate::world::territory::ZoneInfluenceMap>>,
) {
    let Some(influence_map) = influence_map else {
        return;
    };

    let wall_clock = current_unix_seconds();
    if snapshot_state.last_snapshot_wall > 0
        && wall_clock.saturating_sub(snapshot_state.last_snapshot_wall)
            < ZONE_RUNTIME_SNAPSHOT_INTERVAL_SECS
    {
        return;
    }

    match persist_zone_influence_snapshot(&settings, &influence_map) {
        Ok(_) => {
            snapshot_state.last_snapshot_wall = wall_clock;
        }
        Err(error) => tracing::warn!(
            "[bong][persistence] failed to persist zone influence snapshot at {}: {error}",
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
        WORLD_MODEL_STATE_FIELD_NEG_DOMAIN_PENDING_TRIBULATIONS.to_string(),
        serde_json::to_string(&snapshot.neg_domain_pending_tribulations)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
    );
    fields.insert(
        WORLD_MODEL_STATE_FIELD_NEG_DOMAIN_ESCAPE_TELEMETRY.to_string(),
        serde_json::to_string(&snapshot.neg_domain_escape_telemetry)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
    );
    fields.insert(
        WORLD_MODEL_STATE_FIELD_NEG_DOMAIN_ESCAPE_SESSIONS.to_string(),
        serde_json::to_string(&snapshot.neg_domain_escape_sessions)
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
    let initial_version = current_version;
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
                in_coffin INTEGER NOT NULL DEFAULT 0 CHECK (in_coffin IN (0, 1)),
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
                is_damaged INTEGER NOT NULL DEFAULT 0 CHECK (is_damaged IN (0, 1)),
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
                named_faction TEXT,
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

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 23 {
        let transaction = connection.transaction()?;
        let columns = table_columns(&transaction, "player_lifespan")?;
        if columns.is_empty() {
            transaction.execute_batch(
                "
                CREATE TABLE IF NOT EXISTS player_lifespan (
                    username TEXT PRIMARY KEY,
                    born_at_tick INTEGER NOT NULL CHECK (born_at_tick >= 0),
                    years_lived REAL NOT NULL CHECK (years_lived >= 0),
                    cap_by_realm INTEGER NOT NULL CHECK (cap_by_realm > 0),
                    offline_pause_wall INTEGER NOT NULL CHECK (offline_pause_wall >= 0),
                    in_coffin INTEGER NOT NULL DEFAULT 0 CHECK (in_coffin IN (0, 1)),
                    schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                    last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
                );
                ",
            )?;
        } else if !columns.iter().any(|column| column == "in_coffin") {
            transaction.execute_batch(
                "
                ALTER TABLE player_lifespan
                ADD COLUMN in_coffin INTEGER NOT NULL DEFAULT 0 CHECK (in_coffin IN (0, 1));
                ",
            )?;
        }
        transaction.execute_batch("PRAGMA user_version = 23;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 24 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS spirit_treasure_world (
                template_id TEXT PRIMARY KEY,
                instance_id INTEGER NOT NULL CHECK (instance_id >= 0),
                holder_kind TEXT NOT NULL CHECK (holder_kind IN ('player', 'ground', 'lost')),
                holder_id TEXT,
                holder_pos_x REAL,
                holder_pos_y REAL,
                holder_pos_z REAL,
                affinity REAL NOT NULL DEFAULT 0.5 CHECK (affinity >= 0.0 AND affinity <= 1.0),
                dialogue_count INTEGER NOT NULL DEFAULT 0 CHECK (dialogue_count >= 0),
                sleeping INTEGER NOT NULL DEFAULT 0 CHECK (sleeping IN (0, 1)),
                spawned_at_tick INTEGER NOT NULL CHECK (spawned_at_tick >= 0),
                schema_version INTEGER NOT NULL DEFAULT 1 CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL DEFAULT 0 CHECK (last_updated_wall >= 0)
            );

            CREATE TABLE IF NOT EXISTS spirit_treasure_dialogue_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                template_id TEXT NOT NULL,
                character_id TEXT NOT NULL,
                tick INTEGER NOT NULL CHECK (tick >= 0),
                speaker TEXT NOT NULL CHECK (speaker IN ('player', 'spirit')),
                content TEXT NOT NULL,
                affinity_delta REAL NOT NULL DEFAULT 0.0,
                schema_version INTEGER NOT NULL DEFAULT 1 CHECK (schema_version >= 1)
            );

            CREATE INDEX IF NOT EXISTS idx_spirit_treasure_dialogue_log_character
            ON spirit_treasure_dialogue_log (character_id, template_id, tick);
            ",
        )?;
        assert_spirit_treasure_schema_ready(&transaction)?;
        transaction.execute_batch("PRAGMA user_version = 24;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 25 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS player_known_techniques (
                username TEXT PRIMARY KEY,
                known_techniques_json TEXT NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            ",
        )?;
        assert_player_known_techniques_schema_ready(&transaction)?;
        transaction.execute_batch("PRAGMA user_version = 25;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 26 {
        // plan-offscreen-war-v1 P3：克制式战场遗物（deferred-on-hydrate）的待物化持久层。
        // 不走 worldgen 静态布局（战场 chunk 未加载），改用 sqlite + TTL sweep（§10.1 #6
        // 决议：选 sqlite 而非 Resource，避免遗物与已 remove 的死者生命周期耦合成孤儿）。
        // relic_id = UUID TEXT PK（同 char_id 款）；position 拆三列 REAL（同 npc_state pos_x/y/z）；
        // loot_seed 是 u64 deterministic 种子，sqlite 无 u64 → 以 i64 位投影存（读回再投影回来）；
        // created_tick 给 deferred-on-hydrate 时序校验；created_wall 给 TTL sweep（墙钟，不依赖逻辑 tick）。
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS pending_dormant_relics (
                relic_id     TEXT PRIMARY KEY,
                char_id      TEXT NOT NULL,
                zone         TEXT NOT NULL,
                pos_x        REAL NOT NULL,
                pos_y        REAL NOT NULL,
                pos_z        REAL NOT NULL,
                archetype    TEXT NOT NULL,
                loot_seed    INTEGER NOT NULL,
                created_tick INTEGER NOT NULL CHECK (created_tick >= 0),
                created_wall INTEGER NOT NULL CHECK (created_wall >= 0),
                schema_version INTEGER NOT NULL DEFAULT 1 CHECK (schema_version >= 1)
            );
            CREATE INDEX IF NOT EXISTS idx_pending_dormant_relics_zone
            ON pending_dormant_relics (zone, created_wall);
            CREATE INDEX IF NOT EXISTS idx_pending_dormant_relics_created_wall
            ON pending_dormant_relics (created_wall);
            ",
        )?;
        assert_pending_dormant_relics_schema_ready(&transaction)?;
        transaction.execute_batch("PRAGMA user_version = 26;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 27 {
        // plan-coffin-tiers-v1 P0：player_lifespan 加 coffin_grade 列（TEXT，default 'mundane'）
        let transaction = connection.transaction()?;
        let columns = table_columns(&transaction, "player_lifespan")?;
        if !columns.is_empty() && !columns.iter().any(|col| col == "coffin_grade") {
            transaction.execute_batch(
                "
                ALTER TABLE player_lifespan
                ADD COLUMN coffin_grade TEXT NOT NULL DEFAULT 'mundane';
                ",
            )?;
        }
        transaction.execute_batch("PRAGMA user_version = 27;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 28 {
        // plan-niche-craft-fix-v1 P1：灵龛本体增加单一受损态。
        let transaction = connection.transaction()?;
        let columns = table_columns(&transaction, "social_spirit_niches")?;
        if !columns.is_empty() && !columns.iter().any(|col| col == "is_damaged") {
            transaction.execute_batch(
                "
                ALTER TABLE social_spirit_niches
                ADD COLUMN is_damaged INTEGER NOT NULL DEFAULT 0 CHECK (is_damaged IN (0, 1));
                ",
            )?;
        }
        transaction.execute_batch("PRAGMA user_version = 28;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 29 {
        // plan-territory-v1 P0：区域影响力持久化表（zone_influence）。
        // key=(zone_id, char_id)；dominant=1 标记当前霸主行。
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS zone_influence (
                zone_id               TEXT    NOT NULL,
                char_id               TEXT    NOT NULL,
                value                 REAL    NOT NULL DEFAULT 0.0,
                meditation_ticks      INTEGER NOT NULL DEFAULT 0,
                combat_wins           INTEGER NOT NULL DEFAULT 0,
                player_kills          INTEGER NOT NULL DEFAULT 0,
                gather_count          INTEGER NOT NULL DEFAULT 0,
                continuous_sessions   INTEGER NOT NULL DEFAULT 0,
                last_activity_tick    INTEGER NOT NULL DEFAULT 0,
                dominant              INTEGER NOT NULL DEFAULT 0,
                established_tick      INTEGER NOT NULL DEFAULT 0,
                public_known          INTEGER NOT NULL DEFAULT 0,
                schema_version        INTEGER NOT NULL DEFAULT 1,
                last_updated_wall     INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (zone_id, char_id)
            );
            CREATE INDEX IF NOT EXISTS idx_zone_influence_zone_id
            ON zone_influence (zone_id);
            ",
        )?;
        transaction.execute_batch("PRAGMA user_version = 29;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 30 {
        // plan-faction-expansion-v1 P0：social_faction_memberships.named_faction 列迁移。
        //
        // 迁移目标：social_faction_memberships.faction 列存 "attack"/"defend"/"neutral"（v15 建，
        // FactionId 真持久化数据）→ 按 zone_anchor 归属回填到具名势力。
        // 映射依据：
        //   attack  → qingyun_hunters    （青云外门猎杀型主动出击）
        //   defend  → cangyuan_merchants  （血谷守矿型防守）
        //   neutral → north_waste_drifters（流窜无归属→北荒漂流者）
        //   此为 P0 既有数据合理 zone_anchor 归属，非凭空映射；正典依据见 NamedFactionId 注释。
        //
        // (a) 新增 named_faction 列（保留旧 faction 列不破坏 social-v2 读路径）。
        // (b) 三条 UPDATE 按 faction 值回填 named_faction（WHERE named_faction IS NULL 防幂等覆盖）。
        // (c) 表不存在时跳过列/UPDATE（早期 fixture 库，social v15 之前），直接升版本。
        let transaction = connection.transaction()?;
        let existing_columns = table_columns(&transaction, "social_faction_memberships")?;
        if !existing_columns.is_empty() {
            // 表存在：(a) 按需加列，(b) 三条 UPDATE 回填。
            if !existing_columns.iter().any(|c| c == "named_faction") {
                transaction.execute_batch(
                    "ALTER TABLE social_faction_memberships ADD COLUMN named_faction TEXT;",
                )?;
            }
            transaction.execute_batch(
                "
                UPDATE social_faction_memberships
                SET named_faction = 'qingyun_hunters'
                WHERE faction = 'attack' AND named_faction IS NULL;

                UPDATE social_faction_memberships
                SET named_faction = 'cangyuan_merchants'
                WHERE faction = 'defend' AND named_faction IS NULL;

                UPDATE social_faction_memberships
                SET named_faction = 'north_waste_drifters'
                WHERE faction = 'neutral' AND named_faction IS NULL;
                ",
            )?;
        }
        // 无论表是否存在，均升版本号（前置 migration 会在 v15 建表，本 migration 幂等）。
        transaction.execute_batch("PRAGMA user_version = 30;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 31 {
        // plan-life-record-epitaph-v1 P0：碑刻持久化表。
        //
        // epitaph_id TEXT PRIMARY KEY（UUID v7，时序可排序）；
        // entry_json TEXT NOT NULL（EpitaphEntry serde_json 全量序列化）；
        // death_tick / schema_version / last_updated_wall 便于运维查询与版本迁移。
        // 永久保留语义：内存 WorldEpitaphRegistry 超 cap 淘汰时不删除本表行。
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS epitaphs (
                epitaph_id          TEXT PRIMARY KEY,
                character_id        TEXT NOT NULL,
                entry_json          TEXT NOT NULL,
                death_tick          INTEGER NOT NULL CHECK (death_tick >= 0),
                schema_version      INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall   INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            CREATE INDEX IF NOT EXISTS idx_epitaphs_character_id
            ON epitaphs (character_id);
            PRAGMA user_version = 31;
            ",
        )?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 32 {
        // plan-faction-expansion-v1 P3：玩家 ↔ 具名势力声望持久化。
        //
        // key=(char_id, named_faction)，score 取 plan P3 的 -100..=100 区间；
        // 不挂到 social_faction_memberships，避免把“当前挂靠”与“历史信誉”混为一列。
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS social_faction_reputations (
                char_id             TEXT    NOT NULL,
                named_faction       TEXT    NOT NULL,
                score               INTEGER NOT NULL CHECK (score >= -100 AND score <= 100),
                schema_version      INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall   INTEGER NOT NULL CHECK (last_updated_wall >= 0),
                PRIMARY KEY (char_id, named_faction)
            );
            CREATE INDEX IF NOT EXISTS idx_social_faction_reputations_char_id
            ON social_faction_reputations (char_id);
            ",
        )?;
        assert_social_faction_reputations_schema_ready(&transaction)?;
        transaction.execute_batch("PRAGMA user_version = 32;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 33 {
        // plan-bughunt-ao-worldgen-state-pseudo-vein-restart-loss-v1：
        // heartbeat 生成的伪灵脉是动态 zone + lifecycle 双状态，不能只靠 zones_runtime 三列恢复。
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS heartbeat_pseudo_veins (
                zone_id             TEXT PRIMARY KEY,
                dimension           TEXT NOT NULL CHECK (dimension IN ('overworld', 'tsy')),
                min_x               REAL NOT NULL,
                min_y               REAL NOT NULL,
                min_z               REAL NOT NULL,
                max_x               REAL NOT NULL,
                max_y               REAL NOT NULL,
                max_z               REAL NOT NULL,
                danger_level        INTEGER NOT NULL CHECK (danger_level >= 0),
                active_events_json  TEXT NOT NULL,
                patrol_anchors_json TEXT NOT NULL,
                center_x            REAL NOT NULL,
                center_z            REAL NOT NULL,
                spawned_at_tick     INTEGER NOT NULL CHECK (spawned_at_tick >= 0),
                last_tick           INTEGER NOT NULL CHECK (last_tick >= 0),
                qi_current          REAL NOT NULL,
                total_qi_consumed   REAL NOT NULL,
                warning_sent        INTEGER NOT NULL CHECK (warning_sent IN (0, 1)),
                dissipated          INTEGER NOT NULL CHECK (dissipated IN (0, 1)),
                season_at_spawn     TEXT NOT NULL CHECK (
                    season_at_spawn IN (
                        'summer',
                        'summer_to_winter',
                        'winter',
                        'winter_to_summer'
                    )
                ),
                schema_version      INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall   INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            PRAGMA user_version = 33;
            ",
        )?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 34 {
        // pending inflow 没有对应 ECS component / zone 字段可在重启时重建，必须单独
        // 落盘；其余 WorldQiAccount 条目仍由各自物理权威字段恢复，不持久化审计轨迹。
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS qi_runtime_accounts (
                account_id         TEXT PRIMARY KEY,
                balance            REAL NOT NULL CHECK (balance >= 0),
                schema_version     INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall  INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            ",
        )?;
        // 只有全新 v0 数据库能证明历史 pending inflow 必为 0。v33 升级库没有
        // 旧账本可供重建，故意保留缺行，Startup 会 fail-closed，禁止把未知余额
        // 静默解释为 0 并在首帧覆盖。
        if initial_version == 0 {
            transaction.execute(
                "
                INSERT INTO qi_runtime_accounts (
                    account_id, balance, schema_version, last_updated_wall
                ) VALUES (?1, 0.0, ?2, 0)
                ON CONFLICT(account_id) DO NOTHING
                ",
                params![PENDING_INFLOW_ACCOUNT_ID, CURRENT_SCHEMA_VERSION],
            )?;
        }
        transaction.execute_batch("PRAGMA user_version = 34;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 35 {
        let transaction = connection.transaction()?;
        for (column, definition) in [
            (
                "observed_age_ticks",
                "INTEGER NOT NULL DEFAULT 0 CHECK (observed_age_ticks >= 0)",
            ),
            (
                "pending_runtime_ticks",
                "INTEGER NOT NULL DEFAULT 0 CHECK (pending_runtime_ticks >= 0)",
            ),
            (
                "pending_offline_ticks",
                "INTEGER NOT NULL DEFAULT 0 CHECK (pending_offline_ticks >= 0)",
            ),
            (
                "occupant_count",
                "INTEGER NOT NULL DEFAULT 0 CHECK (occupant_count >= 0)",
            ),
            (
                "eval_elapsed_ticks",
                "INTEGER NOT NULL DEFAULT 0 CHECK (eval_elapsed_ticks >= 0)",
            ),
        ] {
            let columns = table_columns(&transaction, "heartbeat_pseudo_veins")?;
            if !columns.iter().any(|existing| existing == column) {
                transaction.execute_batch(&format!(
                    "ALTER TABLE heartbeat_pseudo_veins ADD COLUMN {column} {definition};"
                ))?;
            }
        }
        // v33/v34 没有保存快照发生在 heartbeat 200-tick 周期中的精确位置。
        // 迁移时按最保守的 interval-1 回填，宁可最多提早 199 tick，也不延长生命周期。
        let conservative_elapsed = HEARTBEAT_EVAL_INTERVAL_TICKS.saturating_sub(1);
        transaction.execute(
            "
            UPDATE heartbeat_pseudo_veins
            SET observed_age_ticks =
                    CASE
                        WHEN last_tick >= spawned_at_tick
                        THEN last_tick - spawned_at_tick + ?1
                        ELSE ?1
                    END,
                pending_runtime_ticks = ?1,
                pending_offline_ticks = 0,
                occupant_count = 0,
                eval_elapsed_ticks = ?1
            ",
            params![i64::try_from(conservative_elapsed).unwrap_or(i64::MAX)],
        )?;
        transaction.execute_batch("PRAGMA user_version = 35;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 36 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS player_craft_sessions (
                username          TEXT PRIMARY KEY,
                session_json      TEXT NOT NULL,
                schema_version    INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            PRAGMA user_version = 36;
            ",
        )?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 37 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS dropped_loot (
                instance_id       INTEGER PRIMARY KEY CHECK (instance_id >= 0),
                entry_json        TEXT NOT NULL,
                schema_version    INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            PRAGMA user_version = 37;
            ",
        )?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 38 {
        let transaction = connection.transaction()?;
        // 两个垂死大能 overflow 池在 v38 首次成为稳定持久账户；旧版本从未有可恢复的
        // 聚合余额，因此升级时显式初始化为已知 0。pending inflow 仍保留 v34 的严格
        // unknown 语义，绝不在这里补零。
        for account_id in [
            DYING_ELDER_DAN_EXCESS_ACCOUNT_ID,
            DYING_ELDER_RELEASE_OVERFLOW_ACCOUNT_ID,
        ] {
            transaction.execute(
                "
                INSERT INTO qi_runtime_accounts (
                    account_id, balance, schema_version, last_updated_wall
                ) VALUES (?1, 0.0, ?2, 0)
                ON CONFLICT(account_id) DO NOTHING
                ",
                params![account_id, CURRENT_SCHEMA_VERSION],
            )?;
        }
        transaction.execute_batch("PRAGMA user_version = 38;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 39 {
        let transaction = connection.transaction()?;
        // bughunt player-lifecycle-relog-death-consequence-wipe：`combat::components::
        // Lifecycle`（死亡/复活状态机：state/fortune_remaining/awaiting_decision/各 deadline
        // tick）此前从未落盘，断线重连时 `attach_combat_bundle_to_joined_clients` 只能盲插
        // `Lifecycle::default()`，把 NearDeath/AwaitingRevival 玩家的运气次数与渡劫决策全部
        // 抹回满状态"新角色"。单 JSON 列镜像整个组件（同 `player_known_techniques` 的
        // `known_techniques_json` 模式），键仍是 `username`（与其余 per-character slice 表一致，
        // 代表"当前存活角色"）。
        //
        // `combat_clock_tick_at_save` 是 OPUS 返工要求的跨重启锚点：`CombatClock` 每次进程
        // 重启都从 0 重新计数（`combat::mod::register` 里 `insert_resource(CombatClock::
        // default())`），而 `near_death_deadline_tick`/`revival_decision_deadline_tick`/
        // `weakened_until_tick` 都是"绝对 tick"——落盘时刻的 CombatClock.tick 值。跨重启直接
        // 复用这些绝对值毫无意义（新进程 tick=0 时，旧 deadline 动辄百万级，等价于几十小时后
        // 才会被 near_death_tick/auto_confirm_revival_decisions 结算，期间玩家卡在
        // AwaitingRevival 无敌状态）。这里把落盘时刻的 CombatClock.tick 一并记录，配合既有的
        // `last_updated_wall`，在读档时按真实流逝墙钟秒数把 deadline 折算到读档当刻的 tick
        // 空间（见 `player::state::translate_lifecycle_deadline_tick_across_restart`），
        // 镜像 `player_lifespan.offline_pause_wall` 的按墙钟折算模式。
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS player_lifecycle (
                username TEXT PRIMARY KEY,
                lifecycle_json TEXT NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0),
                combat_clock_tick_at_save INTEGER NOT NULL DEFAULT 0
            );
            PRAGMA user_version = 39;
            ",
        )?;
        transaction.commit()?;
    }

    if current_version < 40 {
        let transaction = connection.transaction()?;
        // R5 的固定 overflow 池此前已经承载真实余额，但没有对应 ECS/zone 字段可从旧
        // 存档重建；从此 migration 起以已知 0 建立行，之后每次 snapshot/hydrate 都走
        // 与其它稳定 runtime pool 相同的完整 whitelist。
        transaction.execute(
            "
            INSERT INTO qi_runtime_accounts (
                account_id, balance, schema_version, last_updated_wall
            ) VALUES (?1, 0.0, ?2, 0)
            ON CONFLICT(account_id) DO NOTHING
            ",
            params![QI_FLOW_OVERFLOW_ACCOUNT_ID, CURRENT_SCHEMA_VERSION],
        )?;
        transaction.execute_batch("PRAGMA user_version = 40;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 41 {
        let transaction = connection.transaction()?;
        transaction.execute(
            "
            INSERT INTO qi_runtime_accounts (
                account_id, balance, schema_version, last_updated_wall
            ) VALUES (?1, 0.0, ?2, 0)
            ON CONFLICT(account_id) DO NOTHING
            ",
            params![RIFT_DRAIN_ACCOUNT_ID, CURRENT_SCHEMA_VERSION],
        )?;
        transaction.execute_batch("PRAGMA user_version = 41;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 42 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS dormant_terminal_commits (
                char_id                 TEXT PRIMARY KEY,
                cause                   TEXT NOT NULL,
                at_tick                 INTEGER NOT NULL CHECK (at_tick >= 0),
                zone                    TEXT NOT NULL,
                winner                  TEXT,
                winner_group            INTEGER,
                loser_group             INTEGER,
                zone_accepted           REAL NOT NULL CHECK (zone_accepted >= 0),
                cleanup_revision        INTEGER CHECK (cleanup_revision >= 0),
                created_wall            INTEGER NOT NULL CHECK (created_wall >= 0),
                schema_version          INTEGER NOT NULL CHECK (schema_version >= 1)
            );
            ",
        )?;
        assert_dormant_terminal_commits_schema_ready(&transaction)?;
        transaction.execute_batch("PRAGMA user_version = 42;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 43 {
        let transaction = connection.transaction()?;
        assert_dormant_terminal_commits_schema_ready(&transaction)?;
        if table_exists(&transaction, "deceased_snapshots")? {
            let columns = table_columns(&transaction, "deceased_snapshots")?;
            if columns.iter().any(|column| column == "public_path") {
                transaction
                    .execute_batch("ALTER TABLE deceased_snapshots DROP COLUMN public_path;")?;
            }
            assert_deceased_snapshots_schema_ready(&transaction)?;
        }
        transaction.execute_batch("PRAGMA user_version = 43;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 44 {
        let transaction = connection.transaction()?;
        // 传承死信箱已完全退役；旧数据库中的表和索引必须一并删除。
        transaction.execute_batch(
            "
            DROP INDEX IF EXISTS idx_legacy_letterbox_inheritor;
            DROP TABLE IF EXISTS legacy_letterbox;
            PRAGMA user_version = 44;
            ",
        )?;
        transaction.commit()?;
    }

    let deceased_schema_transaction = connection.transaction()?;
    if table_exists(&deceased_schema_transaction, "deceased_snapshots")? {
        assert_deceased_snapshots_schema_ready(&deceased_schema_transaction)?;
    }
    deceased_schema_transaction.commit()?;

    let terminal_schema_transaction = connection.transaction()?;
    assert_dormant_terminal_commits_schema_ready(&terminal_schema_transaction)?;
    terminal_schema_transaction.commit()?;

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

        let persisted_cultivation =
            crate::cultivation::components::encode_persisted_cultivation(&cultivation);
        let bundle = serde_json::json!({
            // plan-race-system-v1 P1a：写入的是当前形态 `MeridianSystem::default()`
            // （snake_case channel id），必须标当前 bundle 版本号，否则加载时会误走
            // legacy 迁移分支去解析本就不是 legacy 形态的数据。
            "v": crate::cultivation::legacy_meridian_bundle::CURRENT_BUNDLE_VERSION,
            "cultivation": persisted_cultivation,
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

fn table_exists(transaction: &rusqlite::Transaction<'_>, table: &str) -> rusqlite::Result<bool> {
    transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        params![table],
        |row| row.get(0),
    )
}

fn assert_spirit_treasure_schema_ready(
    transaction: &rusqlite::Transaction<'_>,
) -> rusqlite::Result<()> {
    for (table, required) in [
        (
            "spirit_treasure_world",
            &[
                "template_id",
                "instance_id",
                "holder_kind",
                "holder_id",
                "holder_pos_x",
                "holder_pos_y",
                "holder_pos_z",
                "affinity",
                "dialogue_count",
                "sleeping",
                "spawned_at_tick",
                "schema_version",
                "last_updated_wall",
            ][..],
        ),
        (
            "spirit_treasure_dialogue_log",
            &[
                "id",
                "template_id",
                "character_id",
                "tick",
                "speaker",
                "content",
                "affinity_delta",
                "schema_version",
            ][..],
        ),
    ] {
        let columns = table_columns(transaction, table)?;
        if let Some(missing) = required
            .iter()
            .find(|column| !columns.iter().any(|candidate| candidate == *column))
        {
            return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                io::Error::other(format!("{table} missing required column {missing}")),
            )));
        }
    }

    Ok(())
}

fn assert_player_known_techniques_schema_ready(
    transaction: &rusqlite::Transaction<'_>,
) -> rusqlite::Result<()> {
    let columns = table_columns(transaction, "player_known_techniques")?;
    let required = [
        "username",
        "known_techniques_json",
        "schema_version",
        "last_updated_wall",
    ];
    if let Some(missing) = required
        .iter()
        .find(|column| !columns.iter().any(|name| name == **column))
    {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            io::Error::other(format!(
                "v25 migration completed but player_known_techniques column {missing} missing"
            )),
        )));
    }

    let mut statement = transaction.prepare("PRAGMA table_info(player_known_techniques)")?;
    let primary_key = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, i32>(5)?))
        })?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|(_, pk_ordinal)| *pk_ordinal > 0)
        .collect::<Vec<_>>();
    let expected_primary_key = [("username".to_owned(), 1)];
    if primary_key.as_slice() != expected_primary_key.as_slice() {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            io::Error::other(format!(
                "v25 migration completed but player_known_techniques primary key mismatch: expected username got {primary_key:?}"
            )),
        )));
    }
    Ok(())
}

/// plan-offscreen-war-v1 P3：v26 迁移后验，确保 `pending_dormant_relics` 列与 PK 完整。
/// 仿 [`assert_player_known_techniques_schema_ready`]——迁移完成但列 / PK 漂移则直接报错，
/// 不让一个残缺表静默上线（遗物 upsert/load 会在运行时撞列名错误反而更难定位）。
fn assert_pending_dormant_relics_schema_ready(
    transaction: &rusqlite::Transaction<'_>,
) -> rusqlite::Result<()> {
    let columns = table_columns(transaction, "pending_dormant_relics")?;
    let required = [
        "relic_id",
        "char_id",
        "zone",
        "pos_x",
        "pos_y",
        "pos_z",
        "archetype",
        "loot_seed",
        "created_tick",
        "created_wall",
        "schema_version",
    ];
    if let Some(missing) = required
        .iter()
        .find(|column| !columns.iter().any(|name| name == **column))
    {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            io::Error::other(format!(
                "v26 migration completed but pending_dormant_relics column {missing} missing"
            )),
        )));
    }

    let mut statement = transaction.prepare("PRAGMA table_info(pending_dormant_relics)")?;
    let primary_key = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, i32>(5)?))
        })?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|(_, pk_ordinal)| *pk_ordinal > 0)
        .collect::<Vec<_>>();
    let expected_primary_key = [("relic_id".to_owned(), 1)];
    if primary_key.as_slice() != expected_primary_key.as_slice() {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            io::Error::other(format!(
                "v26 migration completed but pending_dormant_relics primary key mismatch: expected relic_id got {primary_key:?}"
            )),
        )));
    }
    Ok(())
}

fn assert_dormant_terminal_commits_schema_ready(
    transaction: &rusqlite::Transaction<'_>,
) -> rusqlite::Result<()> {
    let columns = table_columns(transaction, "dormant_terminal_commits")?;
    let required = [
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
    ];
    if let Some(missing) = required
        .iter()
        .find(|column| !columns.iter().any(|name| name == **column))
    {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            io::Error::other(format!(
                "v42 migration completed but dormant_terminal_commits column {missing} missing"
            )),
        )));
    }

    let mut statement = transaction.prepare("PRAGMA table_info(dormant_terminal_commits)")?;
    let primary_key = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, i32>(5)?))
        })?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|(_, pk_ordinal)| *pk_ordinal > 0)
        .collect::<Vec<_>>();
    let expected_primary_key = [("char_id".to_owned(), 1)];
    if primary_key.as_slice() != expected_primary_key.as_slice() {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            io::Error::other(format!(
                "v42 migration completed but dormant_terminal_commits primary key mismatch: expected char_id got {primary_key:?}"
            )),
        )));
    }
    Ok(())
}

fn assert_deceased_snapshots_schema_ready(
    transaction: &rusqlite::Transaction<'_>,
) -> rusqlite::Result<()> {
    let columns = table_columns(transaction, "deceased_snapshots")?;
    let required = [
        "char_id",
        "snapshot_json",
        "died_at_tick",
        "schema_version",
        "last_updated_wall",
    ];
    if let Some(missing) = required
        .iter()
        .find(|column| !columns.iter().any(|candidate| candidate == *column))
    {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            io::Error::other(format!(
                "v43 migration completed but deceased_snapshots column {missing} missing"
            )),
        )));
    }
    if columns.iter().any(|column| column == "public_path") {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            io::Error::other(
                "v43 migration completed but retired deceased_snapshots.public_path remains",
            ),
        )));
    }
    Ok(())
}

fn assert_social_faction_reputations_schema_ready(
    transaction: &rusqlite::Transaction<'_>,
) -> rusqlite::Result<()> {
    let columns = table_columns(transaction, "social_faction_reputations")?;
    let required = [
        "char_id",
        "named_faction",
        "score",
        "schema_version",
        "last_updated_wall",
    ];
    if let Some(missing) = required
        .iter()
        .find(|column| !columns.iter().any(|name| name == **column))
    {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            io::Error::other(format!(
                "v32 migration completed but social_faction_reputations column {missing} missing"
            )),
        )));
    }

    let mut statement = transaction.prepare("PRAGMA table_info(social_faction_reputations)")?;
    let table_info = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(1)?,
                row.get::<_, i32>(3)?,
                row.get::<_, i32>(5)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let primary_key = table_info
        .iter()
        .map(|(name, _, pk_ordinal)| (name.clone(), *pk_ordinal))
        .filter(|(_, pk_ordinal)| *pk_ordinal > 0)
        .collect::<Vec<_>>();
    let expected_primary_key = [("char_id".to_owned(), 1), ("named_faction".to_owned(), 2)];
    if primary_key.as_slice() != expected_primary_key.as_slice() {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            io::Error::other(format!(
                "v32 migration completed but social_faction_reputations primary key mismatch: expected (char_id, named_faction) got {primary_key:?}"
            )),
        )));
    }
    for required_not_null in ["char_id", "named_faction"] {
        let is_not_null = table_info
            .iter()
            .find(|(name, _, _)| name == required_not_null)
            .map(|(_, not_null, _)| *not_null != 0)
            .unwrap_or(false);
        if !is_not_null {
            return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                io::Error::other(format!(
                    "v32 migration completed but social_faction_reputations column {required_not_null} must be NOT NULL"
                )),
            )));
        }
    }

    let create_sql: Option<String> = transaction
        .query_row(
            "
            SELECT sql
            FROM sqlite_master
            WHERE type = 'table' AND name = 'social_faction_reputations'
            ",
            [],
            |row| row.get(0),
        )
        .optional()?;
    let Some(create_sql) = create_sql else {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            io::Error::other(
                "v32 migration completed but social_faction_reputations table missing",
            ),
        )));
    };
    for required_check in [
        "score >= -100",
        "score <= 100",
        "schema_version >= 1",
        "last_updated_wall >= 0",
    ] {
        if !create_sql.contains(required_check) {
            return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                io::Error::other(format!(
                    "v32 migration completed but social_faction_reputations CHECK `{required_check}` missing"
                )),
            )));
        }
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
    // r1-P5 fix：改用 IMMEDIATE 事务，起手即取写锁。
    //
    // 原来的 DEFERRED 事务在 WAL 模式下先读后写：两个并发 DuXu 完成各自在
    // SHARED 锁下读到相同的 occupied_slots（如 1），然后都写 2，丢失一次增量
    // （lost update）。IMMEDIATE 在 BEGIN 时就拿 RESERVED 写锁，保证
    // read-check-write 相对于其他 IMMEDIATE/EXCLUSIVE writer 是原子串行的。
    // 这是 worldview §三:78 化虚稀缺不变量在 SQLite 层面的硬保证。
    // 与 try_complete_tribulation_ascension（:2831）保持一致。
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(io::Error::other)?;
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

/// plan-halfstep-buff-v1 P2 atomic ascension grant 四态决策。
///
/// 演进历史：
/// - P3 review #4：把"缺 active row"从 `granted=true` 中拆出（避免误升 Realm）→ `MissingActive`
/// - P4 review #2：把"占额成功"和"非占额仅结算"也拆开（独立 JueBi 不应升 Realm）→ `SettledOnly`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AscensionGrant {
    /// 占额路径（`du_xu` / `jue_bi + void_quota_exceeded`）+ 事务内 quota 校验通过：
    /// `occupied_slots` 已 +1，caller **应升 Realm 到 Void**
    Granted,
    /// 非占额路径（独立 JueBi，如 `void_action_explode_zone`）幸存：
    /// active row 已删但 `occupied_slots` **未增**；caller **不升 Realm**（化虚老怪扛过
    /// 额外天劫不算升格冲刺），仅作 settlement-success 标志
    SettledOnly,
    /// quota 已满（`occupied == limit`）或 `limit=0`（灵气枯竭），caller 回退 HalfStep
    Denied,
    /// `tribulations_active` 找不到 char_id（重复结算 / 状态错乱 / 已被另一进程 settle）。
    /// 本分支**不增量** `quota.occupied_slots`，但仍 commit transaction（保 idempotency +
    /// 清理 active 行）。caller 应 warn + 回退 HalfStep，绝不升 Realm
    MissingActive,
}

/// plan-halfstep-buff-v1 P2 atomic ascension grant outcome。
///
/// `quota` 是事务 commit 后最终的 [`AscensionQuotaRecord`]；`grant` 是 4 态决策
/// [`AscensionGrant::Granted`] / [`AscensionGrant::SettledOnly`] /
/// [`AscensionGrant::Denied`] / [`AscensionGrant::MissingActive`]，caller 必须 match
/// 全部 4 分支（典型用法：仅 `Granted` 升 Realm；其余 3 态均回退 HalfStep / 不升 Realm）；
/// `limit_used` / `occupied_before` 便于追踪并发情况和测试断言。
///
/// **事务行为**：两者均使用 IMMEDIATE 事务（起手即取写锁），无论 `grant` 何种状态，
/// 事务都会删除 `tribulations_active` 行 + commit quota 行（保 idempotency）；
/// 区别在只有 `Granted` 路径会 `occupied_slots += 1`，其他 3 态保持 quota 不变。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtomicAscensionOutcome {
    pub quota: AscensionQuotaRecord,
    pub grant: AscensionGrant,
    pub limit_used: u32,
    pub occupied_before: u32,
}

/// plan-halfstep-buff-v1 P2：事务内原子校验 quota 限额后再决定是否授予 ascension。
///
/// 与 `complete_tribulation_ascension` 的区别：本函数在 transaction 内额外检查
/// `quota.occupied_slots < quota_limit`；如果已满，**不增量、不破坏 DB 状态**，仅返回
/// `AscensionGrant::Denied`。即使如此，仍然删除 `tribulations_active` 行（entity 渡劫
/// 流程已完成，不该留下孤儿 active 记录）+ commit quota 行（保持 idempotent）。返回值
/// 见 [`AtomicAscensionOutcome`] 的 4 态枚举说明。
///
/// 这是 worldview §三:78 化虚稀缺性的硬保证 —— 即使多人同 tick 渡虚劫成功也不会突破名额上限。
///
/// **并发语义**（P5 review #5 澄清）：IMMEDIATE 事务保证 select-check-update 的
/// **原子串行化**（atomic serialization），即任何两个并发调用不会同时读到相同 quota
/// 然后都增量；SQLite **不承诺公平/FIFO 顺序**——多个 BEGIN IMMEDIATE 的获取顺序由
/// SQLite 内部锁队列决定，不一定按调用次序。worldview §三:78 关心的是"不突破名额上限"
/// （原子性保证），不是"谁先谁后"（公平性），所以这里的语义足够。
pub fn try_complete_tribulation_ascension(
    settings: &PersistenceSettings,
    char_id: &str,
    quota_limit: u32,
) -> io::Result<AtomicAscensionOutcome> {
    let wall_clock = current_unix_seconds();
    let mut connection = open_persistence_connection(settings)?;
    // plan-halfstep-buff-v1 P2 fix：用 IMMEDIATE 事务而非默认 DEFERRED。
    //
    // DEFERRED 在 WAL 模式下先读后写：另一个 writer 在我们 BEGIN 之后、UPDATE 之前提交了
    // 自己的写入，会让我们的 commit 失败为 `SQLITE_BUSY_SNAPSHOT` 或 `SQLITE_BUSY`，而不是
    // 把 read-check-write 序列化。IMMEDIATE 立即拿写锁，保证 `quota.occupied_slots <
    // quota_limit` 检查与 UPDATE 之间没有并发 writer 插队。这是 §三:78 化虚稀缺底线在
    // SQLite 层面的硬保证。
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(io::Error::other)?;
    let mut quota = load_ascension_quota_from_transaction(&transaction)?;
    let occupied_before = quota.occupied_slots;

    let active_kind_source: Option<(String, String)> = transaction
        .query_row(
            "SELECT kind, source FROM tribulations_active WHERE char_id = ?1",
            params![char_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(io::Error::other)?;

    let grant = match active_kind_source.as_ref().map(|(kind, source)| {
        (
            kind.as_str(),
            source.as_str(),
            matches!(
                (kind.as_str(), source.as_str()),
                (TRIBULATION_KIND_DU_XU, _)
                    | (TRIBULATION_KIND_JUE_BI, JUEBI_SOURCE_VOID_QUOTA_EXCEEDED)
            ),
        )
    }) {
        None => {
            // active row 缺失 → 状态错乱或重复结算；不增量，让 caller 走 warn + HalfStep
            AscensionGrant::MissingActive
        }
        Some((_, _, true)) => {
            // 占额路径（du_xu / jue_bi+void_quota_exceeded）→ 名额校验
            if quota_limit > 0 && quota.occupied_slots < quota_limit {
                quota.occupied_slots = quota.occupied_slots.saturating_add(1);
                AscensionGrant::Granted
            } else {
                AscensionGrant::Denied
            }
        }
        Some((_, _, false)) => {
            // 非占额路径（独立 JueBi 如 VoidActionExplodeZone）→ 不增不减
            // 用 `SettledOnly` 而非 `Granted` 让 caller 显式不升 Realm
            AscensionGrant::SettledOnly
        }
    };

    transaction
        .execute(
            "DELETE FROM tribulations_active WHERE char_id = ?1",
            params![char_id],
        )
        .map_err(io::Error::other)?;
    upsert_ascension_quota(&transaction, &quota, wall_clock)?;
    transaction.commit().map_err(io::Error::other)?;
    Ok(AtomicAscensionOutcome {
        quota,
        grant,
        limit_used: quota_limit,
        occupied_before,
    })
}

pub fn release_ascension_quota_slot(
    settings: &PersistenceSettings,
) -> io::Result<AscensionQuotaRelease> {
    let wall_clock = current_unix_seconds();
    let mut connection = open_persistence_connection(settings)?;
    // r3-P2 fix：改用 IMMEDIATE 事务，起手即取写锁。
    //
    // 原来的 DEFERRED 事务在 WAL 模式下先读后写：两个并发 release 各自在
    // SHARED 锁下读到相同的 occupied_slots（如 2），然后都写 1，丢失一次减量
    // （lost update）。IMMEDIATE 在 BEGIN 时就拿 RESERVED 写锁，保证
    // read-check-write 相对于其他 IMMEDIATE/EXCLUSIVE writer 是原子串行的。
    // 与 complete_tribulation_ascension（:2736）保持一致。
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(io::Error::other)?;
    let mut quota = load_ascension_quota_from_transaction(&transaction)?;
    let opened_slot = quota.occupied_slots > 0;
    quota.occupied_slots = quota.occupied_slots.saturating_sub(1);
    upsert_ascension_quota(&transaction, &quota, wall_clock)?;
    transaction.commit().map_err(io::Error::other)?;
    Ok(AscensionQuotaRelease { quota, opened_slot })
}

pub fn persist_zone_and_runtime_qi_snapshot(
    settings: &PersistenceSettings,
    zones: Option<&crate::world::zone::ZoneRegistry>,
    qi_ledger: &WorldQiAccount,
) -> io::Result<()> {
    let wall_clock = current_unix_seconds();
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    if let Some(zones) = zones {
        persist_zone_runtime_records(&transaction, zones, wall_clock)?;
    }
    upsert_runtime_qi_account_balances(&transaction, qi_ledger, wall_clock)?;
    transaction.commit().map_err(io::Error::other)
}

pub fn persist_zone_runtime_snapshot(
    settings: &PersistenceSettings,
    zones: &crate::world::zone::ZoneRegistry,
) -> io::Result<()> {
    let wall_clock = current_unix_seconds();
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    persist_zone_runtime_records(&transaction, zones, wall_clock)?;
    transaction.commit().map_err(io::Error::other)
}

#[cfg_attr(not(test), allow(dead_code))]
fn persist_zone_runtime_snapshot_with_heartbeat(
    settings: &PersistenceSettings,
    zones: &crate::world::zone::ZoneRegistry,
    heartbeat: Option<&WorldHeartbeat>,
    qi_ledger: &WorldQiAccount,
) -> io::Result<()> {
    let current_tick = heartbeat
        .map(|heartbeat| heartbeat.last_eval_tick)
        .unwrap_or_default();
    persist_zone_runtime_snapshot_with_heartbeat_at_tick(
        settings,
        zones,
        heartbeat,
        qi_ledger,
        current_tick,
    )
}

fn persist_zone_runtime_snapshot_with_heartbeat_at_tick(
    settings: &PersistenceSettings,
    zones: &crate::world::zone::ZoneRegistry,
    heartbeat: Option<&WorldHeartbeat>,
    qi_ledger: &WorldQiAccount,
    current_tick: u64,
) -> io::Result<()> {
    let wall_clock = current_unix_seconds();
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    persist_zone_runtime_records(&transaction, zones, wall_clock)?;
    if let Some(heartbeat) = heartbeat {
        let pseudo_veins = heartbeat.active_pseudo_vein_records_at_tick(zones, current_tick);
        replace_heartbeat_pseudo_vein_records(&transaction, &pseudo_veins, wall_clock)?;
    }
    upsert_runtime_qi_account_balances(&transaction, qi_ledger, wall_clock)?;
    transaction.commit().map_err(io::Error::other)
}

fn persist_zone_runtime_records(
    transaction: &rusqlite::Transaction<'_>,
    zones: &crate::world::zone::ZoneRegistry,
    wall_clock: i64,
) -> io::Result<()> {
    // heartbeat 动态 zone 会在消散后从 ZoneRegistry 删除。先在同一事务中清掉该命名域的
    // 旧行，再由下方当前 registry 全量重插仍活跃者，避免已结算余额的孤儿行永久残留。
    transaction
        .execute(
            "DELETE FROM zones_runtime WHERE zone_id GLOB 'pseudo_vein_heartbeat_*'",
            [],
        )
        .map_err(io::Error::other)?;
    for zone in &zones.zones {
        upsert_zone_runtime(
            transaction,
            &ZoneRuntimeRecord {
                zone_id: zone.name.clone(),
                spirit_qi: zone.spirit_qi,
                danger_level: zone.danger_level,
            },
            wall_clock,
        )?;
    }
    Ok(())
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn load_zone_runtime_snapshot(
    settings: &PersistenceSettings,
) -> io::Result<Vec<ZoneRuntimeRecord>> {
    let connection = open_persistence_connection(settings)?;
    load_zone_runtime_snapshot_from_connection(&connection)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn persist_heartbeat_pseudo_veins_snapshot(
    settings: &PersistenceSettings,
    heartbeat: &WorldHeartbeat,
    zones: &crate::world::zone::ZoneRegistry,
) -> io::Result<()> {
    let wall_clock = current_unix_seconds();
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    let records = heartbeat.active_pseudo_vein_records(zones);
    replace_heartbeat_pseudo_vein_records(&transaction, &records, wall_clock)?;
    transaction.commit().map_err(io::Error::other)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn load_heartbeat_pseudo_veins_snapshot(
    settings: &PersistenceSettings,
) -> io::Result<Vec<HeartbeatPseudoVeinRecord>> {
    let connection = open_persistence_connection(settings)?;
    load_heartbeat_pseudo_veins_from_connection(&connection)
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
    for record in &runtime_rows {
        if !is_heartbeat_pseudo_vein_zone_namespace(record.zone_id.as_str()) {
            continue;
        }
        let zone = zones
            .find_zone_by_name(record.zone_id.as_str())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "orphan pseudo-vein zone runtime `{}` has no restored lifecycle",
                        record.zone_id
                    ),
                )
            })?;
        if !zone
            .active_events
            .iter()
            .any(|event| event == EVENT_PSEUDO_VEIN)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "pseudo-vein zone runtime `{}` is not backed by an active lifecycle",
                    record.zone_id
                ),
            ));
        }
    }
    zones.apply_runtime_records(&runtime_rows);
    Ok(())
}

fn hydrate_heartbeat_pseudo_veins(
    settings: &PersistenceSettings,
    heartbeat: &mut WorldHeartbeat,
    zones: &mut crate::world::zone::ZoneRegistry,
    current_tick: u64,
    current_wall: i64,
) -> io::Result<usize> {
    let pseudo_veins = load_heartbeat_pseudo_veins_snapshot(settings)?;
    let runtime_rows = load_zone_runtime_snapshot(settings)?;
    validate_pseudo_vein_snapshot_pair(&pseudo_veins, &runtime_rows)?;
    let restored = heartbeat.restore_pseudo_vein_records_at_wall(
        zones,
        &pseudo_veins,
        current_tick,
        current_wall,
    );
    if restored != pseudo_veins.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "restored {restored} of {} validated pseudo-vein lifecycle rows",
                pseudo_veins.len()
            ),
        ));
    }
    Ok(restored)
}

fn validate_pseudo_vein_snapshot_pair(
    pseudo_veins: &[HeartbeatPseudoVeinRecord],
    runtime_rows: &[ZoneRuntimeRecord],
) -> io::Result<()> {
    let heartbeat_by_id = pseudo_veins
        .iter()
        .map(|record| (record.zone_id.as_str(), record))
        .collect::<HashMap<_, _>>();
    let runtime_by_id = runtime_rows
        .iter()
        .filter(|record| is_heartbeat_pseudo_vein_zone_namespace(record.zone_id.as_str()))
        .map(|record| (record.zone_id.as_str(), record))
        .collect::<HashMap<_, _>>();

    for record in pseudo_veins {
        validate_persisted_pseudo_vein_record(record).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "invalid pseudo-vein lifecycle `{}`: {error}",
                    record.zone_id
                ),
            )
        })?;
        let runtime = runtime_by_id.get(record.zone_id.as_str()).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "pseudo-vein lifecycle `{}` has no matching zones_runtime row",
                    record.zone_id
                ),
            )
        })?;
        if !(0.0..=1.0).contains(&runtime.spirit_qi) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "pseudo-vein zone `{}` spirit_qi must be within [0, 1], actual {}",
                    runtime.zone_id, runtime.spirit_qi
                ),
            ));
        }
    }
    for runtime in runtime_by_id.values() {
        if !heartbeat_by_id.contains_key(runtime.zone_id.as_str()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "pseudo-vein zones_runtime row `{}` has no matching lifecycle row",
                    runtime.zone_id
                ),
            ));
        }
    }
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

/// plan-territory-v1 P0：持久化 ZoneInfluenceMap 快照到 SQLite。
/// 照 `persist_zone_runtime_snapshot` 范本。
#[cfg_attr(not(test), allow(dead_code))]
pub fn persist_zone_influence_snapshot(
    settings: &PersistenceSettings,
    influence_map: &crate::world::territory::ZoneInfluenceMap,
) -> io::Result<()> {
    let wall_clock = current_unix_seconds();
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    for (zone_id, entry) in &influence_map.zones {
        for (char_id, player_inf) in &entry.players {
            let is_dominant = entry
                .dominant
                .as_ref()
                .is_some_and(|d| d.char_id == *char_id);
            let (established_tick, public_known) = if let Some(dom) = &entry.dominant {
                if dom.char_id == *char_id {
                    (dom.established_tick, dom.public_known)
                } else {
                    (0u64, false)
                }
            } else {
                (0u64, false)
            };
            upsert_zone_influence(
                &transaction,
                &ZoneInfluenceRecord {
                    zone_id: zone_id.clone(),
                    char_id: char_id.clone(),
                    value: player_inf.value,
                    meditation_ticks: player_inf.source_breakdown.meditation_ticks,
                    combat_wins: player_inf.source_breakdown.combat_wins,
                    player_kills: player_inf.source_breakdown.player_kills,
                    gather_count: player_inf.source_breakdown.gather_count,
                    continuous_sessions: player_inf.source_breakdown.continuous_sessions,
                    last_activity_tick: player_inf.last_activity_tick,
                    dominant: is_dominant,
                    established_tick,
                    public_known,
                    schema_version: CURRENT_SCHEMA_VERSION,
                    last_updated_wall: wall_clock,
                },
            )?;
        }
    }
    transaction.commit().map_err(io::Error::other)
}

/// plan-territory-v1 P0：从 SQLite 读取所有 zone_influence 记录。
#[cfg_attr(not(test), allow(dead_code))]
pub fn load_zone_influence_snapshot(
    settings: &PersistenceSettings,
) -> io::Result<Vec<ZoneInfluenceRecord>> {
    let connection = open_persistence_connection(settings)?;
    load_zone_influence_snapshot_from_connection(&connection)
}

/// plan-territory-v1 P0：从已有 Connection 读取 zone_influence 记录。
#[cfg_attr(not(test), allow(dead_code))]
pub fn load_zone_influence_snapshot_from_connection(
    connection: &Connection,
) -> io::Result<Vec<ZoneInfluenceRecord>> {
    let mut statement = connection
        .prepare(
            "
            SELECT zone_id, char_id, value,
                   meditation_ticks, combat_wins, player_kills,
                   gather_count, continuous_sessions, last_activity_tick,
                   dominant, established_tick, public_known,
                   schema_version, last_updated_wall
            FROM zone_influence
            ORDER BY zone_id ASC, char_id ASC
            ",
        )
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, f64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, i64>(8)?,
                row.get::<_, i64>(9)?,
                row.get::<_, i64>(10)?,
                row.get::<_, i64>(11)?,
                row.get::<_, i64>(12)?,
                row.get::<_, i64>(13)?,
            ))
        })
        .map_err(io::Error::other)?;

    let mut records = Vec::new();
    for row in rows {
        let (
            zone_id,
            char_id,
            value,
            meditation_ticks,
            combat_wins,
            player_kills,
            gather_count,
            continuous_sessions,
            last_activity_tick,
            dominant,
            established_tick,
            public_known,
            schema_version,
            last_updated_wall,
        ) = row.map_err(io::Error::other)?;
        records.push(ZoneInfluenceRecord {
            zone_id,
            char_id,
            value,
            meditation_ticks: u64::try_from(meditation_ticks.max(0)).unwrap_or(u64::MAX),
            combat_wins: sql_to_u32(combat_wins)?,
            player_kills: sql_to_u32(player_kills)?,
            gather_count: sql_to_u32(gather_count)?,
            continuous_sessions: sql_to_u32(continuous_sessions)?,
            last_activity_tick: u64::try_from(last_activity_tick.max(0)).unwrap_or(u64::MAX),
            dominant: dominant != 0,
            established_tick: u64::try_from(established_tick.max(0)).unwrap_or(u64::MAX),
            public_known: public_known != 0,
            schema_version: i32::try_from(schema_version).unwrap_or(CURRENT_SCHEMA_VERSION),
            last_updated_wall,
        });
    }
    Ok(records)
}

/// plan-territory-v1 P0：从 SQLite 记录 hydrate 到 ZoneInfluenceMap Resource。
/// 返回 hydrate 成功的记录数。
#[cfg_attr(not(test), allow(dead_code))]
fn hydrate_zone_influence(
    settings: &PersistenceSettings,
    influence_map: &mut crate::world::territory::ZoneInfluenceMap,
) -> io::Result<usize> {
    use crate::world::territory::{InfluenceSources, PlayerInfluence, ZoneDominance};
    let records = load_zone_influence_snapshot(settings)?;
    let count = records.len();
    for record in records {
        let entry = influence_map
            .zones
            .entry(record.zone_id.clone())
            .or_default();
        entry.players.insert(
            record.char_id.clone(),
            PlayerInfluence {
                value: record.value,
                last_activity_tick: record.last_activity_tick,
                source_breakdown: InfluenceSources {
                    meditation_ticks: record.meditation_ticks,
                    combat_wins: record.combat_wins,
                    player_kills: record.player_kills,
                    gather_count: record.gather_count,
                    continuous_sessions: record.continuous_sessions,
                },
            },
        );
        // 恢复霸主状态（dominant=true 的那行）
        if record.dominant {
            entry.dominant = Some(ZoneDominance {
                char_id: record.char_id.clone(),
                influence: record.value,
                established_tick: record.established_tick,
                public_known: record.public_known,
                realm_band: None, // persistence 存量无境界段，P3 新增字段
            });
        }
    }
    Ok(count)
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

#[allow(clippy::too_many_arguments)]
pub fn persist_revival_qi_transaction(
    settings: &PersistenceSettings,
    username: &str,
    cultivation: &Cultivation,
    meridians: &MeridianSystem,
    contamination: &Contamination,
    life_record: &LifeRecord,
    zones: Option<&crate::world::zone::ZoneRegistry>,
    qi_ledger: &WorldQiAccount,
    release_void_quota: bool,
) -> io::Result<Option<AscensionQuotaRelease>> {
    let entry = latest_biography_entry(life_record)?;
    if !matches!(entry, BiographyEntry::Rebirth { .. }) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "revival qi transaction requires the latest biography entry to be Rebirth",
        ));
    }

    let wall_clock = current_unix_seconds();
    let mut connection = open_persistence_connection(settings)?;
    // Revival owns the actor bundle, biography, signed zone pressure, stable ownerless qi pools
    // and the optional 化虚 quota release as one durable transition. IMMEDIATE serializes the
    // quota read-modify-write with all other quota writers; every write below rolls back if a
    // later owner fails validation or persistence.
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(io::Error::other)?;
    let updated_bundle = prepare_revival_player_cultivation_bundle(
        &transaction,
        username,
        cultivation,
        meridians,
        contamination,
        life_record,
    )?;

    update_revival_player_cultivation_bundle(&transaction, username, &updated_bundle, wall_clock)?;
    upsert_life_record(&transaction, life_record, wall_clock)?;
    append_life_event(
        &transaction,
        life_record.character_id.as_str(),
        entry,
        wall_clock,
    )?;
    if let Some(zones) = zones {
        persist_zone_runtime_records(&transaction, zones, wall_clock)?;
    }
    upsert_runtime_qi_account_balances(&transaction, qi_ledger, wall_clock)?;

    let quota_release = if release_void_quota {
        let mut quota = load_ascension_quota_from_transaction(&transaction)?;
        let opened_slot = quota.occupied_slots > 0;
        quota.occupied_slots = quota.occupied_slots.saturating_sub(1);
        upsert_ascension_quota(&transaction, &quota, wall_clock)?;
        Some(AscensionQuotaRelease { quota, opened_slot })
    } else {
        None
    };

    transaction.commit().map_err(io::Error::other)?;
    Ok(quota_release)
}

/// Build the revival replacement blob only from an existing, fully decodable player bundle.
///
/// This is intentionally stricter than `upsert_player_cultivation_slice`: revival is a durable
/// owner transfer, so it may not manufacture a partial bundle or overwrite corrupt sibling state.
/// Only the four staged owner slices (and the bundle version needed for the current meridian
/// wire shape) change; every other sibling value is retained bit-for-bit in the JSON object.
fn prepare_revival_player_cultivation_bundle(
    transaction: &rusqlite::Transaction<'_>,
    username: &str,
    cultivation: &Cultivation,
    meridians: &MeridianSystem,
    contamination: &Contamination,
    life_record: &LifeRecord,
) -> io::Result<String> {
    let existing: Option<String> = transaction
        .query_row(
            "SELECT cultivation_json FROM player_cultivation WHERE username = ?1",
            params![username],
            |row| row.get(0),
        )
        .optional()
        .map_err(io::Error::other)?;
    let existing = existing.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("revival requires an existing player_cultivation bundle for `{username}`"),
        )
    })?;
    let mut bundle: serde_json::Value = serde_json::from_str(&existing)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let object = bundle.as_object_mut().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("player_cultivation for `{username}` must be a JSON object"),
        )
    })?;
    let bundle_version = match object.get("v") {
        Some(version) => version.as_i64().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("player_cultivation for `{username}` has a non-integer bundle version"),
            )
        })?,
        None => 1,
    };
    if bundle_version < 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "player_cultivation for `{username}` has invalid bundle version {bundle_version}"
            ),
        ));
    }

    {
        let required_slice = |name: &str| {
            object.get(name).cloned().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "player_cultivation for `{username}` is missing required `{name}` slice"
                    ),
                )
            })
        };
        crate::cultivation::components::decode_persisted_cultivation(required_slice(
            "cultivation",
        )?)
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "player_cultivation for `{username}` has invalid cultivation slice: {error}"
                ),
            )
        })?;
        crate::cultivation::legacy_meridian_bundle::decode_meridian_system(
            required_slice("meridians")?,
            bundle_version,
        )
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("player_cultivation for `{username}` has invalid meridians slice: {error}"),
            )
        })?;
        serde_json::from_value::<Contamination>(required_slice("contamination")?).map_err(
            |error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "player_cultivation for `{username}` has invalid contamination slice: {error}"
                    ),
                )
            },
        )?;
        let persisted_life_record = serde_json::from_value::<LifeRecord>(required_slice(
            "life_record",
        )?)
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "player_cultivation for `{username}` has invalid life_record slice: {error}"
                ),
            )
        })?;
        if persisted_life_record.character_id != life_record.character_id {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "player_cultivation identity mismatch for `{username}`: persisted={} staged={}",
                    persisted_life_record.character_id, life_record.character_id
                ),
            ));
        }
    }

    let staged_cultivation = serde_json::to_value(
        crate::cultivation::components::encode_persisted_cultivation(cultivation),
    )
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    crate::cultivation::components::decode_persisted_cultivation(staged_cultivation.clone())
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("staged revival cultivation is invalid: {error}"),
            )
        })?;
    let staged_meridians = serde_json::to_value(meridians)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    crate::cultivation::legacy_meridian_bundle::decode_meridian_system(
        staged_meridians.clone(),
        crate::cultivation::legacy_meridian_bundle::CURRENT_BUNDLE_VERSION,
    )
    .map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("staged revival meridians are invalid: {error}"),
        )
    })?;
    let staged_contamination = serde_json::to_value(contamination)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    serde_json::from_value::<Contamination>(staged_contamination.clone()).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("staged revival contamination is invalid: {error}"),
        )
    })?;
    let staged_life_record = serde_json::to_value(life_record)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    serde_json::from_value::<LifeRecord>(staged_life_record.clone()).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("staged revival life_record is invalid: {error}"),
        )
    })?;

    object.insert(
        "v".to_string(),
        serde_json::json!(crate::cultivation::legacy_meridian_bundle::CURRENT_BUNDLE_VERSION),
    );
    object.insert("cultivation".to_string(), staged_cultivation);
    object.insert("meridians".to_string(), staged_meridians);
    object.insert("contamination".to_string(), staged_contamination);
    object.insert("life_record".to_string(), staged_life_record);
    serde_json::to_string(&bundle)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn update_revival_player_cultivation_bundle(
    transaction: &rusqlite::Transaction<'_>,
    username: &str,
    cultivation_json: &str,
    wall_clock: i64,
) -> io::Result<()> {
    let updated = transaction
        .execute(
            "
            UPDATE player_cultivation
            SET cultivation_json = ?2,
                schema_version = ?3,
                last_updated_wall = ?4
            WHERE username = ?1
            ",
            params![
                username,
                cultivation_json,
                CURRENT_SCHEMA_VERSION,
                wall_clock
            ],
        )
        .map_err(io::Error::other)?;
    if updated != 1 {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("revival player_cultivation row disappeared for `{username}`"),
        ));
    }
    Ok(())
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
    persist_termination_transition_inner(settings, lifecycle, life_record, None, None, None)
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
        None,
    )
}

pub fn persist_npc_termination_with_qi_snapshot(
    settings: &PersistenceSettings,
    lifecycle: &Lifecycle,
    life_record: &LifeRecord,
    death_registry_cause: &str,
    lifespan_event: Option<&LifespanEventRecord>,
    zones: &crate::world::zone::ZoneRegistry,
    qi_ledger: &WorldQiAccount,
) -> io::Result<()> {
    persist_termination_transition_inner(
        settings,
        lifecycle,
        life_record,
        Some(death_registry_cause),
        lifespan_event,
        Some((zones, qi_ledger)),
    )
}

fn persist_termination_transition_inner(
    settings: &PersistenceSettings,
    lifecycle: &Lifecycle,
    life_record: &LifeRecord,
    death_registry_cause: Option<&str>,
    lifespan_event: Option<&LifespanEventRecord>,
    qi_snapshot: Option<(&crate::world::zone::ZoneRegistry, &WorldQiAccount)>,
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

    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
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
    if let Some((zones, qi_ledger)) = qi_snapshot {
        persist_zone_runtime_records(&transaction, zones, wall_clock)?;
        upsert_runtime_qi_account_balances(&transaction, qi_ledger, wall_clock)?;
    }
    upsert_deceased_snapshot(
        &transaction,
        life_record.character_id.as_str(),
        snapshot_json.as_str(),
        died_at_tick,
        wall_clock,
    )?;

    transaction.commit().map_err(io::Error::other)
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
    persist_npc_deceased_archive_with_connection(settings, archive, open_persistence_connection)
}

fn persist_npc_deceased_archive_with_connection(
    settings: &PersistenceSettings,
    archive: &NpcDeceasedArchiveRecord,
    open_connection: impl FnOnce(&PersistenceSettings) -> io::Result<Connection>,
) -> io::Result<()> {
    persist_npc_deceased_archive_with_hooks(settings, archive, open_connection, write_zstd_bundle)
}

fn persist_npc_deceased_archive_with_hooks(
    settings: &PersistenceSettings,
    archive: &NpcDeceasedArchiveRecord,
    open_connection: impl FnOnce(&PersistenceSettings) -> io::Result<Connection>,
    write_bundle: impl FnOnce(&Path, &[u8]) -> io::Result<()>,
) -> io::Result<()> {
    let archive_path = npc_deceased_archive_absolute_path(
        settings,
        archive.char_id.as_str(),
        archive.archived_at_wall,
    );
    let relative_path =
        npc_deceased_archive_relative_path(archive.char_id.as_str(), archive.archived_at_wall);
    let previous_archive = read_optional_file(&archive_path)?;
    let archive_json = serde_json::to_vec_pretty(archive)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if let Err(error) = write_bundle(&archive_path, &archive_json) {
        return match rollback_file(&archive_path, previous_archive.as_deref()) {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(io::Error::other(format!(
                "npc archive replacement failed: {error}; rollback failed: {rollback_error}"
            ))),
        };
    }

    let persisted = (|| -> io::Result<()> {
        let mut connection = open_connection(settings)?;
        let transaction = connection.transaction().map_err(io::Error::other)?;
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

    match persisted {
        Ok(()) => Ok(()),
        Err(error) => match rollback_file(&archive_path, previous_archive.as_deref()) {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(io::Error::other(format!(
                "npc archive persistence failed: {error}; rollback failed: {rollback_error}"
            ))),
        },
    }
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
        let previous_archive = read_optional_file(&archive_path)?;
        let archive_json = serde_json::to_vec_pretty(digest)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        if let Err(error) = write_zstd_bundle(&archive_path, &archive_json) {
            return match rollback_file(&archive_path, previous_archive.as_deref()) {
                Ok(()) => Err(error),
                Err(rollback_error) => Err(io::Error::other(format!(
                    "npc digest archive replacement failed: {error}; rollback failed: {rollback_error}"
                ))),
            };
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

/// plan-offscreen-war-v1 P3：消费 [`PendingDormantRelicCreated`](crate::npc::dormant::PendingDormantRelicCreated)
/// → 把待物化战场遗物落盘进 `pending_dormant_relics`。
///
/// 事件由 `run_dormant_combat_phase` 在败者真元**已守恒释放完毕**且克制判定通过时 emit
/// （严格在 `release_dormant_qi_to_zone` 之后、`store.remove` 之前——无吞真元窗口）。本 system
/// 只把 event 持久化，**不碰任何真元 / ledger**（遗物零真元，§10.1 #5 ④红线）。现开连接同步写
/// （仿 `persist_npc_runtime_state_system` 范式，无 deferred channel）。
///
/// `relic_id` 用**确定性**复合键（char_id + created_tick + loot_seed）而非随机 UUID（CodeRabbit）：
/// 一个逻辑战死对应唯一 (char_id, created_tick)，loot_seed 由 (char_id, tick, sim_seed) 确定，
/// 故同一逻辑死亡始终映射到同一 relic_id。配合 `upsert_pending_dormant_relic` 的
/// `ON CONFLICT(relic_id) DO UPDATE`，**重复 emit 同一遗物天然幂等**（覆盖而非插重复行），
/// 也让未来若加重试路径能靠 relic_id 去重。
/// 注：遗物是**零真元** telemetry/cosmetic loot 占位，持久化失败仅丢一处遗物 ground loot、
/// **不违反守恒**（不像 dormant qi 快照丢失=吞真元）；故此处失败 warn+drop 而非引入重型重试
/// 队列子系统——确定性 relic_id 已消除「随机 id 无法去重」这一真正的回归隐患。
/// 由战场遗物 event 的**逻辑标识字段**（char_id + created_tick + loot_seed）构造确定性
/// `relic_id`。同一逻辑战死无论 emit / persist 多少次都得到同一 id，配合 PK `ON CONFLICT`
/// upsert 实现幂等。created_wall（墙钟）**不**进 id（它随重试漂移、会破坏幂等）。
fn deterministic_relic_id(event: &crate::npc::dormant::PendingDormantRelicCreated) -> String {
    format!(
        "relic:{}:{}:{:016x}",
        event.char_id, event.created_tick, event.loot_seed
    )
}

fn persist_pending_dormant_relics_system(
    settings: Res<PersistenceSettings>,
    mut events: EventReader<crate::npc::dormant::PendingDormantRelicCreated>,
) {
    let pending: Vec<&crate::npc::dormant::PendingDormantRelicCreated> = events.read().collect();
    if pending.is_empty() {
        return;
    }
    let created_wall = current_unix_seconds();
    for event in pending {
        let record = PendingDormantRelicRecord {
            relic_id: deterministic_relic_id(event),
            char_id: event.char_id.clone(),
            zone: event.zone.clone(),
            pos_x: event.position[0],
            pos_y: event.position[1],
            pos_z: event.position[2],
            archetype: event.archetype.as_str().to_string(),
            loot_seed: event.loot_seed,
            created_tick: event.created_tick as i64,
            created_wall,
        };
        if let Err(error) = persist_pending_dormant_relic(&settings, &record) {
            tracing::warn!(
                "[bong][persistence] failed to persist pending dormant relic for {}: {error}",
                event.char_id
            );
            continue;
        }
        tracing::debug!(
            "[bong][persistence] persisted pending battlefield relic {} (char={} zone={} archetype={})",
            record.relic_id,
            record.char_id,
            record.zone,
            record.archetype,
        );
    }
}

/// plan-offscreen-war-v1 P3：战场遗物 TTL retention sweep（仿 [`sweep_npc_digest_retention_system`]）。
/// 墙钟手动限频（[`PENDING_RELIC_SWEEP_INTERVAL_SECS`]）；每次清掉 `created_wall` 早于
/// `now - PENDING_RELIC_RETENTION_SECS` 的陈旧遗物，避免无人到访的战场遗物永久堆积。
fn sweep_dormant_relic_retention_system(
    settings: Res<PersistenceSettings>,
    mut sweep_state: ResMut<DormantRelicSweepState>,
) {
    let now_wall = current_unix_seconds();
    if sweep_state.last_sweep_wall > 0
        && now_wall.saturating_sub(sweep_state.last_sweep_wall) < PENDING_RELIC_SWEEP_INTERVAL_SECS
    {
        return;
    }

    match sweep_stale_dormant_relics(&settings, now_wall) {
        Ok(removed) => {
            sweep_state.last_sweep_wall = now_wall;
            if removed > 0 {
                tracing::debug!(
                    "[bong][persistence] swept {removed} stale battlefield relic(s) (older than {PENDING_RELIC_RETENTION_SECS}s)"
                );
            }
        }
        Err(error) => {
            tracing::warn!("[bong][persistence] failed dormant relic retention sweep: {error}");
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

/// plan-offscreen-war-v1 P3：把一行待物化战场遗物 upsert 进 `pending_dormant_relics`
/// （仿 [`upsert_npc_digest`] 签名）。`loot_seed: u64` 经 `as i64` 位投影存（sqlite 无 u64）。
/// `relic_id` 是 UUID PK，正常情况下每个 event 唯一；用 upsert 是为幂等（同一 event 万一被
/// 重复消费也不双写）。**不碰 ledger / WorldQiAccount**——遗物零真元（§10.1 #5 ④）。
fn upsert_pending_dormant_relic(
    transaction: &rusqlite::Transaction<'_>,
    record: &PendingDormantRelicRecord,
) -> io::Result<()> {
    transaction
        .execute(
            "
            INSERT INTO pending_dormant_relics (
                relic_id,
                char_id,
                zone,
                pos_x,
                pos_y,
                pos_z,
                archetype,
                loot_seed,
                created_tick,
                created_wall,
                schema_version
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ON CONFLICT(relic_id) DO UPDATE SET
                char_id = excluded.char_id,
                zone = excluded.zone,
                pos_x = excluded.pos_x,
                pos_y = excluded.pos_y,
                pos_z = excluded.pos_z,
                archetype = excluded.archetype,
                loot_seed = excluded.loot_seed,
                created_tick = excluded.created_tick,
                -- plan-offscreen-war-v1 P3 review-fix（CodeRabbit Major）：冲突时**保留更早的**
                -- created_wall。它是 TTL retention sweep 与 hydrate 排序的墙钟锚点；若覆盖成新事件
                -- 的墙钟，同一逻辑死亡重发 / 重试会刷新 TTL（陈旧遗物被无限续命）、并打乱 hydrate
                -- 排序。幂等必须对**可观察 TTL** 也成立，而不只对去重成立——故取两者的最小值。
                created_wall = MIN(pending_dormant_relics.created_wall, excluded.created_wall),
                schema_version = excluded.schema_version
            ",
            params![
                record.relic_id,
                record.char_id,
                record.zone,
                record.pos_x,
                record.pos_y,
                record.pos_z,
                record.archetype,
                record.loot_seed as i64,
                record.created_tick,
                record.created_wall,
                NPC_ROW_SCHEMA_VERSION,
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

/// 原子持久化 dormant 终局：staged zone sink、固定 runtime qi accounts、幂等 tombstone
/// 与可选零真元遗物在同一个 SQLite transaction 中提交。首次提交返回 `Committed`；同一
/// `char_id` 已有 tombstone 时返回 `AlreadyCommitted`，且绝不重写 sink 或终局上下文。
pub fn persist_dormant_terminal_commit(
    settings: &PersistenceSettings,
    record: &DormantTerminalCommitRecord,
    zones: &crate::world::zone::ZoneRegistry,
    qi_ledger: &WorldQiAccount,
    relic: Option<&crate::npc::dormant::PendingDormantRelicCreated>,
) -> io::Result<PersistDormantTerminalOutcome> {
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    let exists = transaction
        .query_row(
            "SELECT 1 FROM dormant_terminal_commits WHERE char_id = ?1",
            params![record.char_id],
            |_| Ok(()),
        )
        .optional()
        .map_err(io::Error::other)?
        .is_some();
    if exists {
        transaction.rollback().map_err(io::Error::other)?;
        return Ok(PersistDormantTerminalOutcome::AlreadyCommitted);
    }

    let wall_clock = current_unix_seconds();
    persist_zone_runtime_records(&transaction, zones, wall_clock)?;
    upsert_runtime_qi_account_balances(&transaction, qi_ledger, wall_clock)?;
    if let Some(event) = relic {
        let relic = PendingDormantRelicRecord {
            relic_id: deterministic_relic_id(event),
            char_id: event.char_id.clone(),
            zone: event.zone.clone(),
            pos_x: event.position[0],
            pos_y: event.position[1],
            pos_z: event.position[2],
            archetype: event.archetype.as_str().to_string(),
            loot_seed: event.loot_seed,
            created_tick: i64::try_from(event.created_tick).unwrap_or(i64::MAX),
            created_wall: wall_clock,
        };
        upsert_pending_dormant_relic(&transaction, &relic)?;
    }
    transaction
        .execute(
            "
            INSERT INTO dormant_terminal_commits (
                char_id, cause, at_tick, zone, winner, winner_group, loser_group,
                zone_accepted, cleanup_revision, created_wall, schema_version
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, ?9, ?10)
            ",
            params![
                record.char_id,
                record.cause,
                i64::try_from(record.at_tick).unwrap_or(i64::MAX),
                record.zone,
                record.winner,
                record.winner_group.map(|value| value as i64),
                record.loser_group.map(|value| value as i64),
                record.zone_accepted,
                wall_clock,
                NPC_ROW_SCHEMA_VERSION,
            ],
        )
        .map_err(io::Error::other)?;
    transaction.commit().map_err(io::Error::other)?;
    Ok(PersistDormantTerminalOutcome::Committed)
}

pub fn load_dormant_terminal_commits(
    settings: &PersistenceSettings,
) -> io::Result<Vec<DormantTerminalCommitRecord>> {
    let connection = open_persistence_connection(settings)?;
    let mut statement = connection
        .prepare(
            "
            SELECT char_id, cause, at_tick, zone, winner, winner_group, loser_group,
                   zone_accepted, cleanup_revision
            FROM dormant_terminal_commits
            ORDER BY char_id
            ",
        )
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map([], |row| {
            Ok(DormantTerminalCommitRecord {
                char_id: row.get(0)?,
                cause: row.get(1)?,
                at_tick: row.get::<_, i64>(2)? as u64,
                zone: row.get(3)?,
                winner: row.get(4)?,
                winner_group: row.get::<_, Option<i64>>(5)?.map(|value| value as u64),
                loser_group: row.get::<_, Option<i64>>(6)?.map(|value| value as u64),
                zone_accepted: row.get(7)?,
                cleanup_revision: row.get::<_, Option<i64>>(8)?.map(|value| value as u64),
            })
        })
        .map_err(io::Error::other)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(io::Error::other)
}

pub fn rearm_dormant_terminal_commits(
    settings: &PersistenceSettings,
) -> io::Result<Vec<DormantTerminalCommitRecord>> {
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    transaction
        .execute(
            "UPDATE dormant_terminal_commits SET cleanup_revision = NULL",
            [],
        )
        .map_err(io::Error::other)?;
    transaction.commit().map_err(io::Error::other)?;
    load_dormant_terminal_commits(settings)
}

pub fn bind_dormant_terminal_cleanup_revision(
    settings: &PersistenceSettings,
    char_ids: &[String],
    revision: u64,
) -> io::Result<()> {
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    for char_id in char_ids {
        transaction
            .execute(
                "
                UPDATE dormant_terminal_commits
                SET cleanup_revision = ?2
                WHERE char_id = ?1 AND cleanup_revision IS NULL
                ",
                params![char_id, i64::try_from(revision).unwrap_or(i64::MAX)],
            )
            .map_err(io::Error::other)?;
    }
    transaction.commit().map_err(io::Error::other)
}

pub fn clear_dormant_terminal_commits_through_revision(
    settings: &PersistenceSettings,
    revision: u64,
) -> io::Result<usize> {
    let connection = open_persistence_connection(settings)?;
    connection
        .execute(
            "
            DELETE FROM dormant_terminal_commits
            WHERE cleanup_revision IS NOT NULL AND cleanup_revision <= ?1
            ",
            params![i64::try_from(revision).unwrap_or(i64::MAX)],
        )
        .map_err(io::Error::other)
}

pub fn persist_pending_dormant_relic(
    settings: &PersistenceSettings,
    record: &PendingDormantRelicRecord,
) -> io::Result<()> {
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    upsert_pending_dormant_relic(&transaction, record)?;
    transaction.commit().map_err(io::Error::other)?;
    Ok(())
}

/// plan-offscreen-war-v1 P3：读出某个 zone 全部待物化战场遗物（按 created_wall 稳定排序，
/// 让 deferred-on-hydrate 物化顺序确定性）。`loot_seed` 从 i64 投影回 u64（无损往返）。
/// 消费方：`npc::dormant::relic_hydrate::hydrate_pending_dormant_relics_system`（交付物 3）。
pub fn load_pending_dormant_relics_for_zone(
    settings: &PersistenceSettings,
    zone: &str,
) -> io::Result<Vec<PendingDormantRelicRecord>> {
    let connection = open_persistence_connection(settings)?;
    let mut statement = connection
        .prepare(
            "
            SELECT relic_id, char_id, zone, pos_x, pos_y, pos_z, archetype,
                   loot_seed, created_tick, created_wall
            FROM pending_dormant_relics
            WHERE zone = ?1
            ORDER BY created_wall ASC, relic_id ASC
            ",
        )
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map(params![zone], |row| {
            Ok(PendingDormantRelicRecord {
                relic_id: row.get(0)?,
                char_id: row.get(1)?,
                zone: row.get(2)?,
                pos_x: row.get(3)?,
                pos_y: row.get(4)?,
                pos_z: row.get(5)?,
                archetype: row.get(6)?,
                loot_seed: row.get::<_, i64>(7)? as u64,
                created_tick: row.get(8)?,
                created_wall: row.get(9)?,
            })
        })
        .map_err(io::Error::other)?;

    let mut relics = Vec::new();
    for row in rows {
        relics.push(row.map_err(io::Error::other)?);
    }
    Ok(relics)
}

/// plan-offscreen-war-v1 P3：删一行已物化（hydrate 消费完）的战场遗物。消费后立刻删，
/// 保证同一遗物不被二次物化（玩家拾走后再次靠近不再凭空再生一份 loot）。
/// 消费方：`npc::dormant::relic_hydrate::hydrate_pending_dormant_relics_system`（交付物 3）。
pub fn delete_pending_dormant_relic(
    settings: &PersistenceSettings,
    relic_id: &str,
) -> io::Result<()> {
    let connection = open_persistence_connection(settings)?;
    connection
        .execute(
            "DELETE FROM pending_dormant_relics WHERE relic_id = ?1",
            params![relic_id],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

/// plan-offscreen-war-v1 P3：清掉 `created_wall` 早于 `now - PENDING_RELIC_RETENTION_SECS`
/// 的陈旧战场遗物（仿 [`sweep_stale_npc_digests`]，但无 zstd 归档——遗物只是 ground loot
/// 占位，过期即风化，不值得归档）。返回被清掉的行数（telemetry / 测试断言用）。
pub fn sweep_stale_dormant_relics(
    settings: &PersistenceSettings,
    now_wall: i64,
) -> io::Result<usize> {
    let threshold = now_wall.saturating_sub(PENDING_RELIC_RETENTION_SECS);
    let connection = open_persistence_connection(settings)?;
    let removed = connection
        .execute(
            "DELETE FROM pending_dormant_relics WHERE created_wall < ?1",
            params![threshold],
        )
        .map_err(io::Error::other)?;
    Ok(removed)
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

pub(crate) fn upsert_zone_runtime(
    transaction: &rusqlite::Transaction<'_>,
    record: &ZoneRuntimeRecord,
    wall_clock: i64,
) -> io::Result<()> {
    validate_zone_runtime_record(record)?;
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

fn validate_zone_runtime_record(record: &ZoneRuntimeRecord) -> io::Result<()> {
    if record.zone_id.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "zone runtime id must not be empty",
        ));
    }
    if !record.spirit_qi.is_finite() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "zone runtime `{}` spirit_qi must be finite, actual {}",
                record.zone_id, record.spirit_qi
            ),
        ));
    }
    if is_heartbeat_pseudo_vein_zone_namespace(record.zone_id.as_str())
        && !is_heartbeat_pseudo_vein_zone_id(record.zone_id.as_str())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "pseudo-vein zone runtime id `{}` must end in a decimal u64 index",
                record.zone_id
            ),
        ));
    }
    if is_heartbeat_pseudo_vein_zone_id(record.zone_id.as_str())
        && !(0.0..=1.0).contains(&record.spirit_qi)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "pseudo-vein zone runtime `{}` spirit_qi must be within [0, 1], actual {}",
                record.zone_id, record.spirit_qi
            ),
        ));
    }
    Ok(())
}

fn upsert_runtime_qi_account_balance(
    transaction: &rusqlite::Transaction<'_>,
    qi_ledger: &WorldQiAccount,
    account: &QiAccountId,
    wall_clock: i64,
) -> io::Result<()> {
    let balance = qi_ledger.balance(account);
    if !balance.is_finite() || balance < 0.0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid runtime qi balance account={account} balance={balance}"),
        ));
    }
    transaction
        .execute(
            "
        INSERT INTO qi_runtime_accounts (
            account_id,
            balance,
            schema_version,
            last_updated_wall
        ) VALUES (?1, ?2, ?3, ?4)
        ON CONFLICT(account_id) DO UPDATE SET
            balance = excluded.balance,
            schema_version = excluded.schema_version,
            last_updated_wall = excluded.last_updated_wall
        ",
            params![
                account.id.as_str(),
                balance,
                CURRENT_SCHEMA_VERSION,
                wall_clock,
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

pub(crate) fn upsert_runtime_qi_account_balances(
    transaction: &rusqlite::Transaction<'_>,
    qi_ledger: &WorldQiAccount,
    wall_clock: i64,
) -> io::Result<()> {
    // Main credits TSY drain into the fixed `rift_drain_account()`, which is already in this
    // whitelist. Sync every durable account through one path; do not recreate the PR's obsolete
    // zone-specific `rift:*` row scan.
    for account in persistent_runtime_qi_accounts() {
        upsert_runtime_qi_account_balance(transaction, qi_ledger, &account, wall_clock)?;
    }
    Ok(())
}

pub(crate) fn upsert_player_cultivation_slice(
    transaction: &rusqlite::Transaction<'_>,
    username: &str,
    cultivation: &Cultivation,
    wall_clock: i64,
) -> io::Result<()> {
    let existing: Option<String> = transaction
        .query_row(
            "SELECT cultivation_json FROM player_cultivation WHERE username = ?1",
            params![username],
            |row| row.get(0),
        )
        .optional()
        .map_err(io::Error::other)?;
    let mut bundle = existing
        .map(|json| {
            serde_json::from_str::<serde_json::Value>(&json)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
        })
        .transpose()?
        .unwrap_or_else(|| serde_json::json!({ "v": 1 }));
    let object = bundle.as_object_mut().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("player_cultivation for `{username}` must be a JSON object"),
        )
    })?;
    object.insert(
        "cultivation".to_string(),
        serde_json::to_value(
            crate::cultivation::components::encode_persisted_cultivation(cultivation),
        )
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
    );
    let cultivation_json = serde_json::to_string(&bundle)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    transaction
        .execute(
            "
            INSERT INTO player_cultivation (
                username, cultivation_json, schema_version, last_updated_wall
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

pub(crate) fn upsert_dropped_loot_entries(
    transaction: &rusqlite::Transaction<'_>,
    entries: &[DroppedLootEntry],
    wall_clock: i64,
) -> io::Result<()> {
    for entry in entries {
        if entry.instance_id != entry.item.instance_id || entry.instance_id > JS_SAFE_INTEGER_MAX {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "invalid durable dropped loot id={} item_id={} max={JS_SAFE_INTEGER_MAX}",
                    entry.instance_id, entry.item.instance_id
                ),
            ));
        }
        let entry_json = serde_json::to_string(entry)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        transaction
            .execute(
                "
                INSERT INTO dropped_loot (
                    instance_id, entry_json, schema_version, last_updated_wall
                ) VALUES (?1, ?2, ?3, ?4)
                ON CONFLICT(instance_id) DO UPDATE SET
                    entry_json = excluded.entry_json,
                    schema_version = excluded.schema_version,
                    last_updated_wall = excluded.last_updated_wall
                ",
                params![
                    i64::try_from(entry.instance_id).map_err(io::Error::other)?,
                    entry_json,
                    CURRENT_SCHEMA_VERSION,
                    wall_clock
                ],
            )
            .map_err(io::Error::other)?;
    }
    Ok(())
}

pub(crate) fn delete_dropped_loot_entry(
    transaction: &rusqlite::Transaction<'_>,
    instance_id: u64,
) -> io::Result<()> {
    transaction
        .execute(
            "DELETE FROM dropped_loot WHERE instance_id = ?1",
            params![i64::try_from(instance_id).map_err(io::Error::other)?],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

pub fn load_durable_dropped_loot(
    settings: &PersistenceSettings,
) -> io::Result<HashMap<u64, DroppedLootEntry>> {
    let connection = open_persistence_connection(settings)?;
    let mut statement = connection
        .prepare("SELECT instance_id, entry_json FROM dropped_loot ORDER BY instance_id")
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(io::Error::other)?;
    let mut entries = HashMap::new();
    for row in rows {
        let (stored_id, entry_json) = row.map_err(io::Error::other)?;
        let stored_id = u64::try_from(stored_id).map_err(io::Error::other)?;
        let entry: DroppedLootEntry = serde_json::from_str(&entry_json)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        if stored_id != entry.instance_id
            || entry.instance_id != entry.item.instance_id
            || stored_id > JS_SAFE_INTEGER_MAX
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "durable dropped loot id mismatch row={stored_id} entry={} item={}",
                    entry.instance_id, entry.item.instance_id
                ),
            ));
        }
        entries.insert(stored_id, entry);
    }
    Ok(entries)
}

pub fn persisted_inventory_instance_id_high_water(
    settings: &PersistenceSettings,
) -> io::Result<Option<u64>> {
    fn visit(value: &serde_json::Value, high_water: &mut Option<u64>) -> io::Result<()> {
        match value {
            serde_json::Value::Object(fields) => {
                for (key, value) in fields {
                    if key == "instance_id" {
                        let id = value.as_u64().ok_or_else(|| {
                            io::Error::new(
                                io::ErrorKind::InvalidData,
                                format!("persisted `{key}` must be an unsigned integer"),
                            )
                        })?;
                        if id > JS_SAFE_INTEGER_MAX {
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                format!(
                                    "persisted inventory instance id {id} exceeds JS safe integer max {JS_SAFE_INTEGER_MAX}"
                                ),
                            ));
                        }
                        *high_water = Some(high_water.map_or(id, |current| current.max(id)));
                    }
                    visit(value, high_water)?;
                }
            }
            serde_json::Value::Array(values) => {
                for value in values {
                    visit(value, high_water)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    let connection = open_persistence_connection(settings)?;
    let mut high_water = None;
    let mut statement = connection
        .prepare("SELECT inventory_json FROM inventories")
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(io::Error::other)?;
    for row in rows {
        let json = row.map_err(io::Error::other)?;
        let value: serde_json::Value = serde_json::from_str(&json)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        visit(&value, &mut high_water)?;
    }
    let durable = load_durable_dropped_loot(settings)?;
    for id in durable.keys().copied() {
        high_water = Some(high_water.map_or(id, |current| current.max(id)));
    }
    Ok(high_water)
}

fn replace_heartbeat_pseudo_vein_records(
    transaction: &rusqlite::Transaction<'_>,
    records: &[HeartbeatPseudoVeinRecord],
    wall_clock: i64,
) -> io::Result<()> {
    transaction
        .execute("DELETE FROM heartbeat_pseudo_veins", [])
        .map_err(io::Error::other)?;
    for record in records {
        upsert_heartbeat_pseudo_vein(transaction, record, wall_clock)?;
    }
    Ok(())
}

fn upsert_heartbeat_pseudo_vein(
    transaction: &rusqlite::Transaction<'_>,
    record: &HeartbeatPseudoVeinRecord,
    wall_clock: i64,
) -> io::Result<()> {
    validate_persisted_pseudo_vein_record(record).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "invalid pseudo-vein lifecycle `{}`: {error}",
                record.zone_id
            ),
        )
    })?;
    let active_events_json = serde_json::to_string(&record.active_events)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let patrol_anchors_json = serde_json::to_string(&record.patrol_anchors)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    transaction
        .execute(
            "
            INSERT INTO heartbeat_pseudo_veins (
                zone_id,
                dimension,
                min_x,
                min_y,
                min_z,
                max_x,
                max_y,
                max_z,
                danger_level,
                active_events_json,
                patrol_anchors_json,
                center_x,
                center_z,
                spawned_at_tick,
                last_tick,
                qi_current,
                total_qi_consumed,
                warning_sent,
                dissipated,
                season_at_spawn,
                observed_age_ticks,
                pending_runtime_ticks,
                pending_offline_ticks,
                occupant_count,
                eval_elapsed_ticks,
                schema_version,
                last_updated_wall
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11,
                ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22,
                ?23, ?24, ?25, ?26, ?27
            )
            ON CONFLICT(zone_id) DO UPDATE SET
                dimension = excluded.dimension,
                min_x = excluded.min_x,
                min_y = excluded.min_y,
                min_z = excluded.min_z,
                max_x = excluded.max_x,
                max_y = excluded.max_y,
                max_z = excluded.max_z,
                danger_level = excluded.danger_level,
                active_events_json = excluded.active_events_json,
                patrol_anchors_json = excluded.patrol_anchors_json,
                center_x = excluded.center_x,
                center_z = excluded.center_z,
                spawned_at_tick = excluded.spawned_at_tick,
                last_tick = excluded.last_tick,
                qi_current = excluded.qi_current,
                total_qi_consumed = excluded.total_qi_consumed,
                warning_sent = excluded.warning_sent,
                dissipated = excluded.dissipated,
                season_at_spawn = excluded.season_at_spawn,
                observed_age_ticks = excluded.observed_age_ticks,
                pending_runtime_ticks = excluded.pending_runtime_ticks,
                pending_offline_ticks = excluded.pending_offline_ticks,
                occupant_count = excluded.occupant_count,
                eval_elapsed_ticks = excluded.eval_elapsed_ticks,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                record.zone_id.as_str(),
                dimension_kind_to_sql(record.dimension),
                record.bounds_min[0],
                record.bounds_min[1],
                record.bounds_min[2],
                record.bounds_max[0],
                record.bounds_max[1],
                record.bounds_max[2],
                i64::from(record.danger_level),
                active_events_json,
                patrol_anchors_json,
                record.center_xz[0],
                record.center_xz[1],
                tick_to_sql(record.spawned_at_tick)?,
                tick_to_sql(record.last_tick)?,
                record.qi_current,
                record.total_qi_consumed,
                bool_to_sql(record.warning_sent),
                bool_to_sql(record.dissipated),
                pseudo_vein_season_to_sql(record.season_at_spawn),
                tick_to_sql(record.observed_age_ticks)?,
                tick_to_sql(record.pending_runtime_ticks)?,
                tick_to_sql(record.pending_offline_ticks)?,
                i64::try_from(record.occupant_count).unwrap_or(i64::MAX),
                tick_to_sql(record.eval_elapsed_ticks)?,
                CURRENT_SCHEMA_VERSION,
                wall_clock,
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

/// plan-territory-v1 P0：upsert zone_influence 行（照 upsert_zone_runtime 范本）。
#[cfg_attr(not(test), allow(dead_code))]
fn upsert_zone_influence(
    transaction: &rusqlite::Transaction<'_>,
    record: &ZoneInfluenceRecord,
) -> io::Result<()> {
    transaction
        .execute(
            "
            INSERT INTO zone_influence (
                zone_id,
                char_id,
                value,
                meditation_ticks,
                combat_wins,
                player_kills,
                gather_count,
                continuous_sessions,
                last_activity_tick,
                dominant,
                established_tick,
                public_known,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
            ON CONFLICT(zone_id, char_id) DO UPDATE SET
                value                = excluded.value,
                meditation_ticks     = excluded.meditation_ticks,
                combat_wins          = excluded.combat_wins,
                player_kills         = excluded.player_kills,
                gather_count         = excluded.gather_count,
                continuous_sessions  = excluded.continuous_sessions,
                last_activity_tick   = excluded.last_activity_tick,
                dominant             = excluded.dominant,
                established_tick     = excluded.established_tick,
                public_known         = excluded.public_known,
                schema_version       = excluded.schema_version,
                last_updated_wall    = excluded.last_updated_wall
            ",
            params![
                record.zone_id,
                record.char_id,
                record.value,
                i64::try_from(record.meditation_ticks).unwrap_or(i64::MAX),
                i64::from(record.combat_wins),
                i64::from(record.player_kills),
                i64::from(record.gather_count),
                i64::from(record.continuous_sessions),
                i64::try_from(record.last_activity_tick).unwrap_or(i64::MAX),
                i64::from(record.dominant),
                i64::try_from(record.established_tick).unwrap_or(i64::MAX),
                i64::from(record.public_known),
                record.schema_version,
                record.last_updated_wall,
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
    let persisted_cultivation =
        crate::cultivation::components::encode_persisted_cultivation(cultivation);
    let bundle = serde_json::json!({
        // plan-race-system-v1 P1a —— bump 1→2：`meridians`/`meridian_severed` 子字段
        // channel id 从 `MeridianId` PascalCase 枚举名换轨为 humanoid.json 声明的
        // snake_case `MeridianChannelId`（见
        // `crate::cultivation::legacy_meridian_bundle`）。旧存档（v1 或缺失 `"v"`）
        // 载入时在该模块显式迁移，此处只负责新写入必须标最新版本号。
        "v": crate::cultivation::legacy_meridian_bundle::CURRENT_BUNDLE_VERSION,
        "cultivation": persisted_cultivation,
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
        let record = ZoneRuntimeRecord {
            zone_id,
            spirit_qi,
            danger_level: sql_to_u8(danger_level)?,
        };
        validate_zone_runtime_record(&record)?;
        records.push(record);
    }
    Ok(records)
}

pub(crate) fn load_runtime_qi_account_balances(
    settings: &PersistenceSettings,
) -> io::Result<Vec<(QiAccountId, f64)>> {
    let connection = open_persistence_connection(settings)?;
    let mut balances = Vec::new();
    for account in persistent_runtime_qi_accounts() {
        let balance = connection
            .query_row(
                "
            SELECT balance
            FROM qi_runtime_accounts
            WHERE account_id = ?1
            ",
                params![account.id.as_str()],
                |row| row.get::<_, f64>(0),
            )
            .optional()
            .map_err(io::Error::other)?;
        match balance {
            Some(value) if value.is_finite() && value >= 0.0 => balances.push((account, value)),
            Some(value) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "invalid persisted runtime qi balance account={} balance={value}",
                        account.id
                    ),
                ));
            }
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "runtime qi balance account={} is unknown; refusing to invent zero",
                        account.id
                    ),
                ));
            }
        }
    }
    Ok(balances)
}

pub(crate) fn hydrate_runtime_qi_accounts(
    settings: &PersistenceSettings,
    qi_ledger: &mut WorldQiAccount,
) -> io::Result<usize> {
    let balances = load_runtime_qi_account_balances(settings)?;
    for (account, balance) in &balances {
        qi_ledger
            .set_balance(account.clone(), *balance)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    }
    Ok(balances.len())
}

#[cfg(test)]
pub(crate) fn load_pending_inflow_balance(settings: &PersistenceSettings) -> io::Result<f64> {
    load_runtime_qi_account_balances(settings)?
        .into_iter()
        .find(|(account, _)| *account == pending_inflow_account())
        .map(|(_, balance)| balance)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "pending inflow account missing from persistent runtime whitelist",
            )
        })
}

fn load_heartbeat_pseudo_veins_from_connection(
    connection: &Connection,
) -> io::Result<Vec<HeartbeatPseudoVeinRecord>> {
    let mut statement = connection
        .prepare(
            "
            SELECT zone_id, dimension,
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
                   observed_age_ticks,
                   pending_runtime_ticks,
                   pending_offline_ticks,
                   occupant_count,
                   eval_elapsed_ticks,
                   last_updated_wall
            FROM heartbeat_pseudo_veins
            ORDER BY zone_id ASC
            ",
        )
        .map_err(io::Error::other)?;
    let mut rows = statement.query([]).map_err(io::Error::other)?;

    let mut records = Vec::new();
    while let Some(row) = rows.next().map_err(io::Error::other)? {
        let dimension: String = row.get(1).map_err(io::Error::other)?;
        let active_events_json: String = row.get(9).map_err(io::Error::other)?;
        let patrol_anchors_json: String = row.get(10).map_err(io::Error::other)?;
        let season_at_spawn: String = row.get(19).map_err(io::Error::other)?;
        records.push(HeartbeatPseudoVeinRecord {
            zone_id: row.get(0).map_err(io::Error::other)?,
            dimension: sql_to_dimension_kind(dimension.as_str())?,
            bounds_min: [
                row.get(2).map_err(io::Error::other)?,
                row.get(3).map_err(io::Error::other)?,
                row.get(4).map_err(io::Error::other)?,
            ],
            bounds_max: [
                row.get(5).map_err(io::Error::other)?,
                row.get(6).map_err(io::Error::other)?,
                row.get(7).map_err(io::Error::other)?,
            ],
            danger_level: sql_to_u8(row.get(8).map_err(io::Error::other)?)?,
            active_events: serde_json::from_str(&active_events_json)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
            patrol_anchors: serde_json::from_str(&patrol_anchors_json)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
            center_xz: [
                row.get(11).map_err(io::Error::other)?,
                row.get(12).map_err(io::Error::other)?,
            ],
            spawned_at_tick: sql_to_tick(row.get(13).map_err(io::Error::other)?)?,
            last_tick: sql_to_tick(row.get(14).map_err(io::Error::other)?)?,
            qi_current: row.get(15).map_err(io::Error::other)?,
            total_qi_consumed: row.get(16).map_err(io::Error::other)?,
            warning_sent: sql_to_bool(row.get(17).map_err(io::Error::other)?),
            dissipated: sql_to_bool(row.get(18).map_err(io::Error::other)?),
            season_at_spawn: sql_to_pseudo_vein_season(season_at_spawn.as_str())?,
            observed_age_ticks: sql_to_tick(row.get(20).map_err(io::Error::other)?)?,
            pending_runtime_ticks: sql_to_tick(row.get(21).map_err(io::Error::other)?)?,
            pending_offline_ticks: sql_to_tick(row.get(22).map_err(io::Error::other)?)?,
            occupant_count: usize::try_from(sql_to_tick(row.get(23).map_err(io::Error::other)?)?)
                .map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "occupant_count overflow")
            })?,
            eval_elapsed_ticks: sql_to_tick(row.get(24).map_err(io::Error::other)?)?,
            snapshot_wall: row.get(25).map_err(io::Error::other)?,
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
        BiographyEntry::PvpEncounter { .. } => "pvp_encounter",
        BiographyEntry::PvpBetrayal { .. } => "pvp_betrayal",
        BiographyEntry::NicheIntrusion { .. } => "niche_intrusion",
        BiographyEntry::VortexProjectileDrained { .. } => "vortex_projectile_drained",
        BiographyEntry::VortexBackfired { .. } => "vortex_backfired",
        BiographyEntry::AnqiSniped { .. } => "anqi_sniped",
        BiographyEntry::FalseSkinShed { .. } => "false_skin_shed",
        BiographyEntry::SpawnTutorialCompleted { .. } => "spawn_tutorial_completed",
        BiographyEntry::VoidAction { .. } => "void_action",
        BiographyEntry::JueBiSurvived { .. } => "jue_bi_survived",
        BiographyEntry::JueBiKilled { .. } => "jue_bi_killed",
        BiographyEntry::MutationAdvanced { .. } => "mutation_advanced",
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
        | BiographyEntry::PvpEncounter { tick, .. }
        | BiographyEntry::PvpBetrayal { tick, .. }
        | BiographyEntry::NicheIntrusion { tick, .. }
        | BiographyEntry::VortexProjectileDrained { tick, .. }
        | BiographyEntry::VortexBackfired { tick, .. }
        | BiographyEntry::AnqiSniped { tick, .. }
        | BiographyEntry::FalseSkinShed { tick, .. }
        | BiographyEntry::SpawnTutorialCompleted { tick, .. }
        | BiographyEntry::VoidAction { tick, .. }
        | BiographyEntry::JueBiSurvived { tick, .. }
        | BiographyEntry::JueBiKilled { tick, .. }
        | BiographyEntry::MutationAdvanced { tick, .. } => *tick,
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
    died_at_tick: u64,
    wall_clock: i64,
) -> io::Result<()> {
    transaction
        .execute(
            "
            INSERT INTO deceased_snapshots (
                char_id,
                snapshot_json,
                died_at_tick,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(char_id) DO UPDATE SET
                snapshot_json = excluded.snapshot_json,
                died_at_tick = excluded.died_at_tick,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                char_id,
                snapshot_json,
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

fn read_optional_file(path: &Path) -> io::Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(contents) => Ok(Some(contents)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn rollback_file(path: &Path, previous: Option<&[u8]>) -> io::Result<()> {
    match previous {
        Some(contents) => fs::write(path, contents),
        None => match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        },
    }
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

fn dimension_kind_to_sql(dimension: DimensionKind) -> &'static str {
    match dimension {
        DimensionKind::Overworld => "overworld",
        DimensionKind::Tsy => "tsy",
    }
}

fn sql_to_dimension_kind(value: &str) -> io::Result<DimensionKind> {
    match value {
        "overworld" => Ok(DimensionKind::Overworld),
        "tsy" => Ok(DimensionKind::Tsy),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unknown dimension kind `{other}`"),
        )),
    }
}

fn pseudo_vein_season_to_sql(season: PseudoVeinSeasonV1) -> &'static str {
    match season {
        PseudoVeinSeasonV1::Summer => "summer",
        PseudoVeinSeasonV1::SummerToWinter => "summer_to_winter",
        PseudoVeinSeasonV1::Winter => "winter",
        PseudoVeinSeasonV1::WinterToSummer => "winter_to_summer",
    }
}

fn sql_to_pseudo_vein_season(value: &str) -> io::Result<PseudoVeinSeasonV1> {
    match value {
        "summer" => Ok(PseudoVeinSeasonV1::Summer),
        "summer_to_winter" => Ok(PseudoVeinSeasonV1::SummerToWinter),
        "winter" => Ok(PseudoVeinSeasonV1::Winter),
        "winter_to_summer" => Ok(PseudoVeinSeasonV1::WinterToSummer),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unknown pseudo-vein season `{other}`"),
        )),
    }
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
    write_zstd_bundle_with_writer(path, payload, |file, compressed| file.write_all(compressed))
}

fn write_zstd_bundle_with_writer(
    path: &Path,
    payload: &[u8],
    write_temp: impl FnOnce(&mut fs::File, &[u8]) -> io::Result<()>,
) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let compressed = zstd::stream::encode_all(payload, 3).map_err(io::Error::other)?;
    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
    let filename = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "bundle".to_string());
    let temp_path = path.with_file_name(format!(
        ".{filename}.tmp-{}-{}",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let mut temp_file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp_path)?;
    let result = (|| {
        write_temp(&mut temp_file, &compressed)?;
        temp_file.sync_all()?;
        drop(temp_file);
        fs::rename(&temp_path, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

#[cfg_attr(not(test), allow(dead_code))]
fn read_zstd_bundle(reference: &Path, relative_path: &str) -> io::Result<Vec<u8>> {
    let absolute_path = if Path::new(relative_path).is_absolute() {
        PathBuf::from(relative_path)
    } else {
        let settings = PersistenceSettings {
            db_path: reference.to_path_buf(),
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

// ─── plan-life-record-epitaph-v1 P0：碑刻持久化 ───────────────────────────────

/// 持久化一条碑刻到 SQLite epitaphs 表（幂等 ON CONFLICT DO UPDATE）。
///
/// 仿 `upsert_deceased_snapshot` 的 INSERT...ON CONFLICT 模式（:5926-:5963）。
/// SQLite epitaphs 表永久保留——即使 WorldEpitaphRegistry 内存 cap 淘汰也不删表行。
pub fn persist_epitaph(
    settings: &PersistenceSettings,
    entry: &crate::cultivation::epitaph::EpitaphEntry,
) -> io::Result<()> {
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(io::Error::other)?;
    let entry_json = serde_json::to_string(entry)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let wall_clock = current_unix_seconds();
    transaction
        .execute(
            "
            INSERT INTO epitaphs (
                epitaph_id,
                character_id,
                entry_json,
                death_tick,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(epitaph_id) DO UPDATE SET
                character_id      = excluded.character_id,
                entry_json        = excluded.entry_json,
                death_tick        = excluded.death_tick,
                schema_version    = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                entry.id.0,
                entry.character_id,
                entry_json,
                tick_to_sql(entry.death_tick)?,
                EVENT_SCHEMA_VERSION,
                wall_clock
            ],
        )
        .map_err(io::Error::other)?;
    transaction.commit().map_err(io::Error::other)?;
    Ok(())
}

/// 按 epitaph_id 读回单条碑刻（用于测试 round-trip 与运维查询）。
#[allow(dead_code)]
pub fn load_epitaph(
    settings: &PersistenceSettings,
    epitaph_id: &str,
) -> io::Result<Option<crate::cultivation::epitaph::EpitaphEntry>> {
    let connection = open_persistence_connection(settings)?;
    let result = connection
        .query_row(
            "SELECT entry_json FROM epitaphs WHERE epitaph_id = ?1",
            params![epitaph_id],
            |row| {
                let json: String = row.get(0)?;
                Ok(json)
            },
        )
        .optional()
        .map_err(io::Error::other)?;
    match result {
        None => Ok(None),
        Some(json) => {
            let entry: crate::cultivation::epitaph::EpitaphEntry = serde_json::from_str(&json)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            Ok(Some(entry))
        }
    }
}

#[cfg(test)]
mod tests;
