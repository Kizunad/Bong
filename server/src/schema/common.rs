use serde::{Deserialize, Serialize};

// ─── 常量 ───────────────────────────────────────────────

pub const SPIRIT_QI_TOTAL: f64 = 100.0;
pub const INTENSITY_MIN: f64 = 0.0;
pub const INTENSITY_MAX: f64 = 1.0;
pub const COOLDOWN_SAME_TARGET_MS: u64 = 600_000;
pub const NEWBIE_POWER_THRESHOLD: f64 = 0.2;
pub const MAX_COMMANDS_PER_TICK: usize = 5;
pub const MAX_NARRATION_LENGTH: usize = 500;
pub const MAX_PAYLOAD_BYTES: usize = 32_768;

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
    /// plan-exploration-probe-return-v1 P1 — 通用提示（探针 Denied 等非灾事件通知）。
    Generic,
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
    // bug-hunt-1: agent 端 TypeBox（agent/packages/schema/src/common.ts:81-87）有这 6 个
    // 额外 kind，server 侧 enum + validate_narration_entry 白名单此前漏列，导致天道
    // 发出的 war_outcome / dying_elder_* narration 在 redis_bridge 校验处被整 batch 拒
    // 收并仅 tracing::warn! 丢弃，玩家永远收不到大战结局与濒死老者剧情播报。
    WarOutcome,
    DyingElderAppeared,
    DyingElderDanReceived,
    DyingElderBetrayal,
    DyingElderDeadNatural,
    DyingElderDeadPlayerKill,
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

#[cfg(test)]
mod narration_kind_tests {
    use super::NarrationKind;

    /// bug-hunt-1: 这 6 个 NarrationKind 变体必须与 agent 端 TypeBox
    /// （agent/packages/schema/src/common.ts:81-87）字面量逐字一致，且走 snake_case
    /// 序列化。否则天道发的 war_outcome / dying_elder_* narration 在 redis_bridge 的
    /// validate_narration_entry 白名单或 serde 反序列化处被拒，整 batch 静默丢弃。
    #[test]
    fn new_agent_narration_kinds_serialize_to_expected_wire_names() {
        let cases = [
            (NarrationKind::WarOutcome, "war_outcome"),
            (NarrationKind::DyingElderAppeared, "dying_elder_appeared"),
            (
                NarrationKind::DyingElderDanReceived,
                "dying_elder_dan_received",
            ),
            (NarrationKind::DyingElderBetrayal, "dying_elder_betrayal"),
            (
                NarrationKind::DyingElderDeadNatural,
                "dying_elder_dead_natural",
            ),
            (
                NarrationKind::DyingElderDeadPlayerKill,
                "dying_elder_dead_player_kill",
            ),
        ];
        for (variant, wire) in cases {
            let json = serde_json::to_string(&variant)
                .unwrap_or_else(|e| panic!("序列化 {variant:?} 失败: {e}"));
            assert_eq!(
                json,
                format!("\"{wire}\""),
                "{variant:?} 应序列化为 \"{wire}\"（须与 agent TypeBox 字面量一致）；实际 {json}"
            );
            let back: NarrationKind =
                serde_json::from_str(&json).unwrap_or_else(|e| panic!("反序列化 {wire} 失败: {e}"));
            assert_eq!(back, variant, "{wire} 应能反序列化回 {variant:?}");
        }
    }
}
