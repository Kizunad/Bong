//! plan-botany-v1 — 植物物种 registry（野生 + 灵田共用）。
//!
//! 本切片（lingtian P0 依赖）只实装最小骨架：
//!   * `PlantKind` / `PlantKindRegistry` — 物种元定义 + TOML loader
//!   * `cultivable: bool` — 区分可种 vs 野生 only（plan-lingtian-v1 §1.2.4 / §2 表）
//!
//! 不在本切片：野生采集 system、harvest-popup UI、herbalism XP 联动 —— 留 botany 后续。

pub mod plant_kind;
pub mod registry;

#[allow(unused_imports)]
pub use plant_kind::{GrowthCost, PlantId, PlantKind, PlantRarity};
#[allow(unused_imports)]
pub use registry::{load_plant_kind_registry, PlantKindRegistry};

use valence::prelude::App;

pub fn register(app: &mut App) {
    let registry = load_plant_kind_registry().unwrap_or_else(|error| {
        panic!("[bong][botany] failed to load plant kind registry: {error}");
    });
    tracing::info!(
        "[bong][botany] loaded {} plant kind(s) from assets/botany/plants.toml ({} cultivable)",
        registry.len(),
        registry.cultivable_ids().count(),
    );
    app.insert_resource(registry);
}
