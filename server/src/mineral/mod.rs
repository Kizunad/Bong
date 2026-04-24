//! plan-mineral-v1 — 矿物 server runtime。
//!
//! 模块构成：
//!  * [`types`] — `MineralId` enum（18 个 mineral，含灵石四档）+ 品阶 / 范畴。
//!  * [`registry`] — `MineralRegistry` resource（18 条静态元数据）。
//!  * [`components`] — `MineralOreNode` component + `MineralOreIndex` 反查表。
//!  * [`events`] — Probe / Drop / Exhausted / KarmaFlag 4 个 Bevy events。
//!  * [`break_handler`] — `DiggingEvent` listener，重写 vanilla loot drop 走 mineral_id。
//!
//! M3 阶段未接入 worldgen — `MineralOreIndex` 启动时为空，`break_handler` 对所有
//! 非矿脉 block 静默 no-op；M2 worldgen 写矿脉时同步插入 OreNode 到 index。

pub mod break_handler;
pub mod components;
pub mod events;
pub mod registry;
pub mod types;

pub use components::{MineralOreIndex, MineralOreNode};
pub use events::{KarmaFlagIntent, MineralDropEvent, MineralExhaustedEvent, MineralProbeIntent};
pub use registry::{build_default_registry, LingShiQiRange, MineralEntry, MineralRegistry};
pub use types::{MineralCategory, MineralId, MineralRarity};

use valence::prelude::{App, Update};

use break_handler::handle_block_break_for_mineral;

pub fn register(app: &mut App) {
    let registry = build_default_registry();
    tracing::info!(
        target: "bong::mineral",
        "[bong][mineral] loaded {} mineral entries from default registry",
        registry.len()
    );

    app.insert_resource(registry);
    app.insert_resource(MineralOreIndex::default());

    app.add_event::<MineralProbeIntent>();
    app.add_event::<MineralDropEvent>();
    app.add_event::<MineralExhaustedEvent>();
    app.add_event::<KarmaFlagIntent>();

    app.add_systems(Update, handle_block_break_for_mineral);
}
