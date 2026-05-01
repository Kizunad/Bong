//! plan-mineral-v1 — 矿物 server runtime。
//!
//! 模块构成：
//!  * [`types`] — `MineralId` enum（18 个 mineral，含灵石四档）+ 品阶 / 范畴。
//!  * [`registry`] — `MineralRegistry` resource（18 条静态元数据）。
//!  * [`anchors`] — worldgen 固定锚点 manifest → `MineralOreNode` 启动期物化。
//!  * [`components`] — `MineralOreNode` component + `MineralOreIndex` 反查表。
//!  * [`events`] — Probe / Drop / Exhausted / KarmaFlag 4 个 Bevy events。
//!  * [`break_handler`] — `DiggingEvent` listener，重写 vanilla loot drop 走 mineral_id。
//!
//! 非矿脉 block 静默 no-op；M2 启动期从 `worldgen/blueprint/mineral_anchors.json`
//! 物化固定锚点，后续 raster `mineral_density/mineral_kind` 可继续扩展同一 index。

pub mod anchors;
pub mod break_handler;
pub mod bridge;
pub mod components;
pub mod events;
pub mod inventory_grant;
pub mod persistence;
pub mod probe;
pub mod registry;
pub mod session;
pub mod types;

// 公共 re-exports — M3 阶段只 register / break_handler 真正使用；其他类型作为
// 模块公共 API surface（M2/M4/M5 后续接入用），故 #[allow(unused_imports)]。
#[allow(unused_imports)]
pub use components::{MineralOreIndex, MineralOreNode};
#[allow(unused_imports)]
pub use events::{
    KarmaFlagIntent, MineralDropEvent, MineralExhaustedEvent, MineralFeedbackEvent,
    MineralProbeDenialReason, MineralProbeIntent, MineralProbeResponse, MineralProbeResult,
};
#[allow(unused_imports)]
pub use persistence::{
    load_exhausted_log, ExhaustedEntry, ExhaustedLogFile, ExhaustedMineralsLog, MineralTickClock,
};
#[allow(unused_imports)]
pub use registry::{build_default_registry, LingShiQiRange, MineralEntry, MineralRegistry};
#[allow(unused_imports)]
pub use session::{ticks_total_for_rarity, MiningSession, MiningSessionState};
#[allow(unused_imports)]
pub use types::{MineralCategory, MineralId, MineralRarity};

use valence::prelude::{App, IntoSystemConfigs, Startup, Update};

use anchors::{spawn_mineral_anchor_nodes, MineralAnchorConfig};
use break_handler::handle_block_break_for_mineral;
use bridge::{forward_karma_flag_to_agent, record_karma_flag_weights};
use events::emit_mineral_feedback_chat;
use inventory_grant::consume_mineral_drops_into_inventory;
use persistence::{record_exhausted_minerals, tick_mineral_clock};
use probe::resolve_mineral_probe_intents;

pub fn register(app: &mut App) {
    let registry = build_default_registry();
    tracing::info!(
        target: "bong::mineral",
        "[bong][mineral] loaded {} mineral entries from default registry",
        registry.len()
    );

    app.insert_resource(registry);
    app.insert_resource(MineralOreIndex::default());
    app.insert_resource(MineralAnchorConfig::default());
    // plan-mineral-v1 §M6 — 启动时从 data/minerals/exhausted.json hydrate
    // 已耗尽矿脉记录，避免 worldgen 重新生成已挖穿的 ore 块。
    let exhausted_log = ExhaustedMineralsLog::hydrated();
    tracing::info!(
        target: "bong::mineral",
        "[bong][mineral] hydrated {} exhausted entries from disk",
        exhausted_log.entries().len()
    );
    app.insert_resource(exhausted_log);
    app.insert_resource(MineralTickClock::default());

    app.add_event::<MineralProbeIntent>();
    app.add_event::<MineralProbeResponse>();
    app.add_event::<MineralDropEvent>();
    app.add_event::<MineralExhaustedEvent>();
    app.add_event::<KarmaFlagIntent>();
    app.add_event::<MineralFeedbackEvent>();

    app.add_systems(
        Startup,
        spawn_mineral_anchor_nodes.after(crate::world::setup_world),
    );

    app.add_systems(
        Update,
        (
            tick_mineral_clock,
            resolve_mineral_probe_intents,
            handle_block_break_for_mineral,
            emit_mineral_feedback_chat,
            // plan-mineral-v1 §2.2 — drop 事件由 inventory_grant 在同一 Update 内消费；
            // Bevy 的 Events 支持单 tick 内 writer → reader 管道（EventReader 扫整帧的 events）。
            consume_mineral_drops_into_inventory,
            record_exhausted_minerals,
            record_karma_flag_weights,
            forward_karma_flag_to_agent,
        ),
    );
}
