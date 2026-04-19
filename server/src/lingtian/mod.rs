//! plan-lingtian-v1 — 灵田专项（人工种植）。
//!
//! 与 plan-botany-v1（野生生态）职责分离，共用 `PlantKindRegistry`。
//!
//! 已落切片：
//!   * P0：§1.1 `LingtianPlot` Component + `CropInstance` + 翻新行为
//!   * P1：§1.2.1 锄头三档（HoeKind，items 在 assets/items/lingtian.toml）；
//!     §1.2.2 开垦 (TillSession) + 地形适合性检查；
//!     §1.6 翻新 (RenewSession)；
//!     §4 StartTill / TillCompleted / StartRenew / RenewCompleted 事件；
//!     ECS 驱动 system（起 session → tick → 完成 → spawn/reset Plot + 扣锄耐久）
//!
//! 不在本切片：种植 / 补灵 / 收获 / 偷菜 / 偷灵 / 密度阈值 / 客户端 UI。
//! valence BlockKind ↔ TerrainKind 适配 + 真正的 BlockEntity 持久化留给
//! 下游切片（与 plan-persistence-v1 联动）。

pub mod events;
pub mod hoe;
pub mod plot;
pub mod session;
pub mod systems;
pub mod terrain;

#[allow(unused_imports)]
pub use events::{RenewCompleted, StartRenewRequest, StartTillRequest, TillCompleted};
#[allow(unused_imports)]
pub use hoe::HoeKind;
#[allow(unused_imports)]
pub use plot::{CropInstance, LingtianPlot, N_RENEW, PLOT_QI_CAP_BASE, PLOT_QI_CAP_MAX};
#[allow(unused_imports)]
pub use session::{
    RenewSession, SessionMode, SessionState, TillSession, RENEW_TICKS, TILL_AUTO_TICKS,
    TILL_MANUAL_TICKS,
};
#[allow(unused_imports)]
pub use systems::{ActiveLingtianSessions, ActiveSession};
#[allow(unused_imports)]
pub use terrain::{classify_for_till, TerrainKind, TillRejectReason};

use valence::prelude::{App, IntoSystemConfigs, Update};

pub fn register(app: &mut App) {
    tracing::info!("[bong][lingtian] registering lingtian subsystem (plan-lingtian-v1 P1)");
    app.insert_resource(ActiveLingtianSessions::new());
    app.add_event::<StartTillRequest>();
    app.add_event::<TillCompleted>();
    app.add_event::<StartRenewRequest>();
    app.add_event::<RenewCompleted>();
    app.add_systems(
        Update,
        (
            systems::handle_start_till,
            systems::handle_start_renew,
            systems::tick_lingtian_sessions,
            systems::apply_completed_sessions,
        )
            .chain(),
    );
}
