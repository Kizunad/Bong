//! plan-race-system-v1 P0 — 通用身体构型（BodyPlan）与种族（Race）底盘。
//!
//! 把「部位 / 命中几何 / 装备槽 / 经脉拓扑 = 人形硬编码」重构为按种族数据驱动的通用
//! 系统。本模块（P0a）交付：
//! - 数据模型（`types`）：`BodyPlan` / `BodyPartDef` / `PartConsequence` / `HitGeometry`
//!   （`HeightBands` 人形参数化 + `PartBoxes` 非人形局部盒）/ `IntrinsicRace` 组件
//! - `BodyPlanRegistry`（`registry`）：`assets/body_plans/plans/*.json` glob 加载
//! - `RaceRegistry`（`race_registry`）：`assets/body_plans/races.json` 单文件加载 +
//!   `BeastKind → RaceId` 派生查询 + `morph_pairs` 易形配对数据（P4 消费）
//! - 全图校验（`validate`）+ 纯几何函数（`geometry`）
//! - 唯一解析入口 `resolve_body_plan`（`resolve`）
//!
//! **接线约定**：本仓库未采用 Bevy `Plugin` trait，而是全仓统一的函数式
//! `mod::register(app: &mut App)` 惯例（见 `main.rs` 逐行 `xxx::register(&mut app)`）—
//! 下面的 `register` 函数就是本模块对该惯例的实现,不是字面 `impl Plugin`。
//!
//! **P0a 范围**：底盘（registry / resolve 入口 / 数据资产）。
//!
//! **P0b 战斗消费点接线**（本模块新增 `legacy` 子模块）：
//! - `combat::resolve::body_part_multipliers` 改查目标实体解析出的 `BodyPlan`（经
//!   `resolve_body_plan`），`Res<BodyPlanRegistry>`/`Res<RaceRegistry>` 缺失时（大量
//!   既有单测未插入这两个资源）优雅退化到 [`registry::humanoid_plan_static`]——生产
//!   环境 `body_plan::register()` 恒装载两资源，这条退化分支永不触发。
//! - `combat::raycast::classify_body_part` / `standing_humanoid_aabb` 改为内部调用
//!   `geometry::classify_height_bands` + 读取 [`registry::humanoid_plan_static`] 的
//!   `hit_geometry::HeightBands` 数据，替换原先散落的 `STANDING_HALF_WIDTH` /
//!   `ARM_LATERAL_THRESHOLD` / `LEG_ABDOMEN_BOUNDARY` 等硬编码常量——函数签名保持不变
//!   （二者是无 ECS 访问权限的纯函数，`combat/resolve.rs`、`combat/carrier.rs` 及自身
//!   测试模块的全部既有调用点不受影响，`combat::carrier` 963 行附近的投射物命中分支
//!   因此"自动"改走新入口，无需改代码）。
//! - `combat::arm_wound` / `movement::leg_wound` 的主/副手臂、双腿身份判定改为查询
//!   [`legacy::legacy_body_parts_matching`]（按 `PartConsequence::Manipulator{main_hand}`
//!   / `Locomotion` 标签分发），不再硬编码假设"这两个 enum 变体就是双臂/双腿"——
//!   `MAIN_ARM`/`OFF_ARM` 两个 `pub const BodyPart` 因外部既有调用点（`combat/needle.rs`
//!   `combat/anqi_v2.rs`）而保留为编译期字面量，一条 pin 测试锁死它们与 humanoid.json
//!   `PartConsequence` 标签的一致性。
//!
//! 「humanoid 路径必须与现状 bit-for-bit 一致」这条红线通过 `geometry.rs` 的批量行为
//! 对拍测试 + `humanoid.json` 数值与旧硬编码表的逐项 pin 测试来保证。

pub mod geometry;
pub mod legacy;
pub mod race_registry;
pub mod registry;
pub mod resolve;
pub mod types;
pub mod validate;

pub use legacy::{id_to_legacy_body_part, legacy_body_part_to_id, legacy_body_parts_matching};
pub use race_registry::{RaceLoadError, RaceRegistry, HUMAN_RACE_ID};
pub use registry::{
    humanoid_plan_static, BodyPlanLoadError, BodyPlanRegistry, HUMANOID_BODY_PLAN_ID,
};
pub use resolve::{
    body_part_for_mutation_slot, resolve_body_plan, BodyPlanPurpose, BodyPlanResolveInputs,
    ResolveBodyPlanError,
};
pub use types::{
    BodyPartDef, BodyPartId, BodyPlan, BodyPlanId, HeightBand, HeightBandAssignment, HitGeometry,
    IntrinsicRace, MeridianProfile, PartBox, PartConsequence, RaceId, StandingAabbSpec,
};

use std::path::Path;

use valence::prelude::App;

pub fn register(app: &mut App) {
    tracing::info!("[bong][body_plan] registering body plan / race registries");

    let plans_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(registry::DEFAULT_BODY_PLANS_DIR);
    let body_plans = registry::BodyPlanRegistry::load_dir(&plans_dir).unwrap_or_else(|error| {
        panic!(
            "[bong][body_plan] failed to load body plans from {}: {error}",
            plans_dir.display()
        )
    });
    if !body_plans.contains(&types::BodyPlanId::new(registry::HUMANOID_BODY_PLAN_ID)) {
        panic!(
            "[bong][body_plan] mandatory \"{}\" body plan missing from {} — every entity \
             resolution path falls back to it, this is a fatal deployment misconfiguration",
            registry::HUMANOID_BODY_PLAN_ID,
            plans_dir.display()
        );
    }
    tracing::info!("[bong][body_plan] loaded {} body plan(s)", body_plans.len());

    let races_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(race_registry::DEFAULT_RACES_PATH);
    let races =
        race_registry::RaceRegistry::load_file(&races_path, &body_plans).unwrap_or_else(|error| {
            panic!(
                "[bong][body_plan] failed to load races from {}: {error}",
                races_path.display()
            )
        });
    tracing::info!("[bong][body_plan] loaded {} race(s)", races.len());

    app.insert_resource(body_plans);
    app.insert_resource(races);
}
