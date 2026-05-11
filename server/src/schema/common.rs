use serde::{Deserialize, Serialize};

// ─── 常量 ───────────────────────────────────────────────

pub const SPIRIT_QI_TOTAL: f64 = 100.0;
pub const INTENSITY_MIN: f64 = 0.0;
pub const INTENSITY_MAX: f64 = 1.0;
pub const COOLDOWN_SAME_TARGET_MS: u64 = 600_000;
pub const NEWBIE_POWER_THRESHOLD: f64 = 0.2;
pub const MAX_COMMANDS_PER_TICK: usize = 5;
pub const MAX_NARRATION_LENGTH: usize = 500;
pub const MAX_PAYLOAD_BYTES: usize = 8192;

// ─── 枚举 ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CommandType {
    SpawnEvent,
    SpawnNpc,
    DespawnNpc,
    FactionEvent,
    ModifyZone,
    NpcBehavior,
    HeartbeatOverride,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    ThunderTribulation,
    BeastTide,
    RealmCollapse,
    KarmaBacklash,
    PoisonMiasma,
    MeridianSeal,
    DaoxiangWave,
    HeavenlyFire,
    PressureInvert,
    AllWither,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NarrationScope {
    Broadcast,
    Zone,
    Player,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NarrationStyle {
    SystemWarning,
    Perception,
    Narration,
    EraDecree,
    PoliticalJianghu,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NarrationKind {
    DeathInsight,
    NicheIntrusion,
    NicheIntrusionByNpc,
    NpcFarmPressure,
    ScatteredCultivator,
    PoliticalJianghu,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ChatIntent {
    Complaint,
    Boast,
    Social,
    Help,
    Provoke,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PlayerTrend {
    Rising,
    Stable,
    Falling,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NpcStateKind {
    Idle,
    Fleeing,
    Attacking,
    Patrolling,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GameEventType {
    PlayerKillNpc,
    PlayerKillPlayer,
    PlayerDeath,
    NpcSpawn,
    ZoneQiChange,
    EventTriggered,
    PlayerJoin,
    PlayerLeave,
}
