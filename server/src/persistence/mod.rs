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

mod agent;
mod bootstrap;
mod epitaph;
mod helpers;
mod known_techniques;
mod life;
mod migrations;
mod models;
mod npc;
mod player;
mod social;
mod tribulation;
mod void_actions;
mod world;
mod world_qi;

use self::migrations::*;
pub use agent::*;
pub use bootstrap::*;
pub use epitaph::*;
pub(crate) use helpers::*;
pub(crate) use known_techniques::*;
pub use life::*;
pub use models::*;
pub use npc::*;
pub(crate) use player::*;
pub use social::*;
pub use tribulation::*;
pub use void_actions::*;
pub use world::*;
pub(crate) use world_qi::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub(crate) struct PersistenceBootstrapSet;

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

#[cfg(test)]
mod tests;
