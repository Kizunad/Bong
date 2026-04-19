//! plan-lingtian-v1 — 灵田专项（人工种植）。
//!
//! 与 plan-botany-v1（野生生态）职责分离，共用 `PlantKindRegistry`。
//!
//! 本切片（P0）只实装：
//!   * §1.1 `LingtianPlot` Component + `CropInstance`
//!   * §1.6 翻新（贫瘠 / renew）行为
//!   * `register(app)` 入口（仅占位 — tick / session / IPC 后续切片）
//!
//! 不在本切片：开垦/种植/补灵/收获 session、灵气 tick、SeedRegistry、
//! 偷菜偷灵、密度阈值、UI handler。这些按 plan §5 阶段表逐 Phase 接入。

pub mod plot;

#[allow(unused_imports)]
pub use plot::{CropInstance, LingtianPlot, N_RENEW, PLOT_QI_CAP_BASE, PLOT_QI_CAP_MAX};

use valence::prelude::App;

pub fn register(_app: &mut App) {
    tracing::info!("[bong][lingtian] registering lingtian subsystem (plan-lingtian-v1 P0)");
    // P0 仅暴露组件类型，无 system / event / resource 注册。
    // P1+ 在此追加：SeedRegistry resource、Session events、tick system。
}
