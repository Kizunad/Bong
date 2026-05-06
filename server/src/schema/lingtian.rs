//! 灵田 IPC 共享原子（plan-lingtian-v1 §4 数据契约）。
//!
//! 当前切片仅 `lingtian_session` 推送 active session 进度（用于客户端 HUD
//! 进度条）。后续切片再加 plot snapshot / inventory derived UI。

use serde::{Deserialize, Serialize};

/// 当前活跃 session 的种类（与 `crate::lingtian::ActiveSession` 判别式对齐 wire）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LingtianSessionKindV1 {
    Till,
    Renew,
    Planting,
    Harvest,
    Replenish,
    DrainQi,
}

/// plan §4 — 当前 player 的活跃 session 进度。`active=false` 表示当前无 session
/// （client 应隐藏 HUD 进度条）。其余字段在 `active=false` 时无意义。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LingtianSessionDataV1 {
    pub active: bool,
    pub kind: LingtianSessionKindV1,
    /// session 目标方块的整型 (x, y, z)。
    pub pos: [i32; 3],
    pub elapsed_ticks: u32,
    pub target_ticks: u32,
    /// 仅 Planting / Harvest 携带 plant_id；其余 None。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plant_id: Option<String>,
}

/// plan-botany-agent-v1 P3 — 灵田压力上升沿 Redis 观测事件。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LingtianZonePressureLevelV1 {
    Low,
    Mid,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LingtianZonePressureV1 {
    pub v: u8,
    pub zone: String,
    pub level: LingtianZonePressureLevelV1,
    pub raw_pressure: f32,
    pub tick: u64,
}

impl LingtianZonePressureV1 {
    pub fn new(
        zone: impl Into<String>,
        level: LingtianZonePressureLevelV1,
        raw_pressure: f32,
        tick: u64,
    ) -> Self {
        Self {
            v: 1,
            zone: zone.into(),
            level,
            raw_pressure,
            tick,
        }
    }
}

impl Default for LingtianSessionDataV1 {
    fn default() -> Self {
        Self {
            active: false,
            kind: LingtianSessionKindV1::Till,
            pos: [0, 0, 0],
            elapsed_ticks: 0,
            target_ticks: 0,
            plant_id: None,
        }
    }
}
