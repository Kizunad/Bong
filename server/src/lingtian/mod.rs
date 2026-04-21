//! plan-lingtian-v1 — 灵田专项（人工种植）。
//!
//! 与 plan-botany-v1（野生生态）职责分离，共用 `PlantKindRegistry`。
//!
//! 已落切片：
//!   * P0：§1.1 `LingtianPlot` Component + `CropInstance` + 翻新行为
//!   * P1：§1.2.1 锄头三档（HoeKind，items 在 assets/items/lingtian.toml）；
//!     §1.2.2 开垦 (TillSession) + 地形适合性检查；§1.6 翻新 (RenewSession)；
//!     §4 Till/Renew 事件 + ECS 通路（起 → tick → 完成 → spawn/reset Plot
//!     + 扣锄耐久；单 player 单 session；锄归零自动卸下）
//!   * P2：§1.3 生长模型（quality_multiplier + 区域漏吸 30% + 丰沛期品质
//!     加成）；`growth.rs` 纯函数 + `qi_account.rs` ZoneQiAccount + 1200
//!     Bevy-tick 累计器；`lingtian_growth_tick` system 每 lingtian-tick 推
//!     plot growth
//!   * P3：§1.2.3 种植 (PlantingSession 1s)；§1.2.4 种子 item（cultivable
//!     plant 派生 SeedRegistry，seeds.toml 三种子）；handle_start_planting
//!     验空 plot + 背包种子 → 完成 spawn crop + 扣种子 1
//!
//! 不在本切片：补灵 / 收获 / 偷菜 / 偷灵 / 密度阈值 / 客户端 UI。
//! valence BlockKind ↔ TerrainKind 适配 + 真正的 BlockEntity 持久化留给
//! 下游切片（与 plan-persistence-v1 联动）。

pub mod environment;
pub mod events;
pub mod growth;
pub mod hoe;
pub mod network_emit;
pub mod plot;
pub mod pressure;
pub mod qi_account;
pub mod seed;
pub mod session;
pub mod systems;
pub mod terrain;

#[allow(unused_imports)]
pub use environment::{compute_plot_qi_cap, PlotBiome, PlotEnvironment};
#[allow(unused_imports)]
pub use pressure::{
    compute_zone_pressure, PressureLevel, ZonePressureState, ZonePressureTracker, PRESSURE_HIGH,
    PRESSURE_LOW, PRESSURE_MID, REPLENISH_WINDOW_LINGTIAN_TICKS,
};

#[allow(unused_imports)]
pub use events::{
    DrainQiCompleted, HarvestCompleted, PlantingCompleted, RenewCompleted, ReplenishCompleted,
    StartDrainQiRequest, StartHarvestRequest, StartPlantingRequest, StartRenewRequest,
    StartReplenishRequest, StartTillRequest, TillCompleted, ZonePressureCrossed,
};
#[allow(unused_imports)]
pub use growth::{
    advance_one_lingtian_tick, quality_bonus, quality_multiplier, GrowthOutcome,
    FENGPEI_QUALITY_BONUS, FENGPEI_THRESHOLD, ZONE_LEAK_GROWTH_FACTOR, ZONE_LEAK_RATIO,
};
#[allow(unused_imports)]
pub use hoe::HoeKind;
#[allow(unused_imports)]
pub use plot::{CropInstance, LingtianPlot, N_RENEW, PLOT_QI_CAP_BASE, PLOT_QI_CAP_MAX};
#[allow(unused_imports)]
pub use qi_account::{
    LingtianTickAccumulator, ZoneQiAccount, BEVY_TICKS_PER_LINGTIAN_TICK, DEFAULT_ZONE,
};
#[allow(unused_imports)]
pub use seed::{seed_id_for, SeedRegistry};
#[allow(unused_imports)]
pub use session::{
    DrainQiSession, HarvestSession, PlantingSession, RenewSession, ReplenishSession,
    ReplenishSource, SessionMode, SessionState, TillSession, DRAIN_QI_TICKS,
    DRAIN_QI_TO_PLAYER_RATIO, DRAIN_QI_TO_ZONE_RATIO, HARVEST_AUTO_TICKS, HARVEST_MANUAL_TICKS,
    PLANTING_TICKS, RENEW_TICKS, REPLENISH_COOLDOWN_LINGTIAN_TICKS, TILL_AUTO_TICKS,
    TILL_MANUAL_TICKS,
};
#[allow(unused_imports)]
pub use systems::{ActiveLingtianSessions, ActiveSession, LingtianClock, LingtianHarvestRng};
#[allow(unused_imports)]
pub use terrain::{classify_for_till, TerrainKind, TillRejectReason};

use valence::prelude::{App, IntoSystemConfigs, Update};

use crate::botany::PlantKindRegistry;

pub fn register(app: &mut App) {
    tracing::info!("[bong][lingtian] registering lingtian subsystem (plan-lingtian-v1 P3)");
    app.insert_resource(ActiveLingtianSessions::new());
    app.insert_resource(LingtianTickAccumulator::new());
    let mut zone_qi = ZoneQiAccount::new();
    zone_qi.set(DEFAULT_ZONE, 5.0);
    app.insert_resource(zone_qi);

    // SeedRegistry 从 PlantKindRegistry 派生 — botany::register 必须先跑（main.rs 中已是这个顺序）。
    let plant_registry = app
        .world()
        .get_resource::<PlantKindRegistry>()
        .expect("[bong][lingtian] PlantKindRegistry missing — botany::register must run first");
    let seed_registry = SeedRegistry::from_plant_registry(plant_registry);
    tracing::info!(
        "[bong][lingtian] derived SeedRegistry with {} cultivable seed(s)",
        seed_registry.len()
    );
    app.insert_resource(seed_registry);

    app.insert_resource(LingtianHarvestRng::default());
    app.insert_resource(LingtianClock::default());
    app.insert_resource(ZonePressureTracker::new());

    app.add_event::<StartTillRequest>();
    app.add_event::<TillCompleted>();
    app.add_event::<StartRenewRequest>();
    app.add_event::<RenewCompleted>();
    app.add_event::<StartPlantingRequest>();
    app.add_event::<PlantingCompleted>();
    app.add_event::<StartHarvestRequest>();
    app.add_event::<HarvestCompleted>();
    app.add_event::<StartReplenishRequest>();
    app.add_event::<ReplenishCompleted>();
    app.add_event::<StartDrainQiRequest>();
    app.add_event::<DrainQiCompleted>();
    app.add_event::<ZonePressureCrossed>();
    // 11 systems — 用两段 .chain() 避开 Bevy IntoSystemConfigs 的 tuple 上限
    app.add_systems(
        Update,
        (
            systems::handle_start_till,
            systems::handle_start_renew,
            systems::handle_start_planting,
            systems::handle_start_harvest,
            systems::handle_start_replenish,
            systems::handle_start_drain_qi,
            systems::tick_lingtian_sessions,
            systems::apply_completed_sessions,
        )
            .chain(),
    );
    app.add_systems(
        Update,
        (
            systems::lingtian_growth_tick,
            // pressure 必须在 growth_tick 之后（共享 accumulator 节拍 + 用 clock 即时值）
            systems::record_replenish_to_pressure,
            systems::compute_zone_pressure_system,
            // session emit 在 apply 后跑，client 拿到的是结算后状态
            network_emit::emit_lingtian_session_to_clients,
        )
            .chain()
            .after(systems::apply_completed_sessions),
    );
}
