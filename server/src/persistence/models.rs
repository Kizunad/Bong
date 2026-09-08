//! Shared persistence settings, runtime contracts, and durable record models.

use super::*;

#[derive(Debug, Clone)]
pub struct PersistenceSettings {
    pub(super) db_path: PathBuf,
    pub(super) server_run_id: String,
}

impl Resource for PersistenceSettings {}

#[derive(Debug, Clone, Copy)]
pub(super) struct ProductionSliceClock {
    pub(super) runtime_tick: u64,
    pub(super) wall_unix_millis: u64,
}

impl SliceClock for ProductionSliceClock {
    fn runtime_tick(&self) -> u64 {
        self.runtime_tick
    }

    fn wall_unix_millis(&self) -> u64 {
        self.wall_unix_millis
    }
}

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

pub(super) type DeceasedFactionMembershipSqlRow = (Option<String>, i64, i64, i64, Option<i64>, i64);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct DeathInsightEventPayload {
    pub(super) death_insight: DeathInsightRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct LifeEventPayload {
    pub(super) biography_entry: BiographyEntry,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct BootstrapPayload {
    pub(super) id: String,
    pub(super) schema_version: i32,
    pub(super) note: String,
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
