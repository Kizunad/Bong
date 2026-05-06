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
    /// 仅 Replenish 携带来源 wire id。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// 当前目标 plot 的杂染值；缺省表示 server 未携带 plot 状态。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dye_contamination: Option<f32>,
    /// `dye_contamination >= 0.3` 时客户端 HUD 显示"已染杂" tag。
    #[serde(default, skip_serializing_if = "is_false")]
    pub dye_contamination_warning: bool,
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
            source: None,
            dye_contamination: None,
            dye_contamination_warning: false,
        }
    }
}

fn is_false(value: &bool) -> bool {
    !*value
}
