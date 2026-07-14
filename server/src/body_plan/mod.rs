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
pub mod layout;
pub mod legacy;
pub mod morph;
pub mod race_registry;
pub mod registry;
pub mod resolve;
pub mod types;
pub mod validate;

pub use layout::{humanoid_layout_static, BodyPlanLayoutLoadError, BodyPlanLayoutRegistry};
pub use legacy::{id_to_legacy_body_part, legacy_body_part_to_id, legacy_body_parts_matching};
pub use morph::{form_anchors_open, technique_requires_form_anchor, MorphState};
pub use race_registry::{MeridianMappingDef, RaceLoadError, RaceRegistry, HUMAN_RACE_ID};
pub use registry::{
    humanoid_plan_static, humanoid_topology_static, BodyPlanLoadError, BodyPlanRegistry,
    HUMANOID_BODY_PLAN_ID,
};
pub use resolve::{
    body_part_for_mutation_slot, channel_body_part, dugu_injection_channel,
    form_identity_from_world, intrinsic_is_humanoid_from_world, resolve_body_plan,
    resolve_body_plan_for_target, resolve_meridian_topology_for_target, resolve_race_to_plan,
    BodyPlanPurpose, BodyPlanResolveInputs, ResolveBodyPlanError,
};
pub use types::{
    BodyPartDef, BodyPartId, BodyPlan, BodyPlanId, ChannelDef, ChannelRole, DuguInjectionEntry,
    HeightBand, HeightBandAssignment, HitGeometry, IntrinsicRace, MeridianFamily, MeridianProfile,
    PartBox, PartConsequence, RaceGate, RaceGateOwned, RaceId, RealmMeridianReq, StandingAabbSpec,
    TopologyEdge,
};

use std::path::PathBuf;

use valence::prelude::App;

/// bughunt major-3 修复：显式覆盖 `assets/` 资产根目录的环境变量——按仓内既有先例
/// `world::mod::BONG_TERRAIN_RASTER_PATH` 的运行时 env var 覆盖模式。部署时把 binary
/// 拷到别处运行、又不满足下面 [`resolve_assets_root`] 的 cwd/`current_exe` 兜底探测时，
/// 用这个变量直接指向包含 `assets/` 子目录的根路径。
pub const BONG_ASSETS_DIR_ENV_VAR: &str = "BONG_ASSETS_DIR";

/// 运行时解析 `assets/` 所在根目录，取代编译期烙死的 `env!("CARGO_MANIFEST_DIR")`。
///
/// bughunt major-3：`env!("CARGO_MANIFEST_DIR")` 在编译期把**构建机器**上的源码树
/// 绝对路径烙进二进制——只要部署方式是"拷贝 binary 到别处运行"（不带完整源码树），
/// 这个路径在目标机器上多半不存在，`registry::load_dir`/`race_registry::load_file`
/// 读不到文件直接 `panic!`，服务器启动即崩，且崩溝原因（一个构建机器上的路径）
/// 对运维几乎不可读。
///
/// 解析顺序（每一步都要求候选目录下确实存在 `assets/` 子目录才采信，不满足就试
/// 下一步，避免误报"解析成功"却在稍后加载具体文件时才 panic）：
/// 1. [`BONG_ASSETS_DIR_ENV_VAR`] 环境变量显式覆盖——部署时最推荐，直接跳过全部探测；
/// 2. 当前工作目录——`cargo run`/"先 cd 到项目根再跑 binary"是本仓库最常见的部署
///    习惯（`scripts/dev-reload.sh`、`cargo run` 文档约定均如此）；
/// 3. `current_exe()` 所在目录——"binary 和 assets/ 放在同一目录分发"的部署方式；
/// 4. `CARGO_MANIFEST_DIR`（编译期常量）——dev/`cargo test` 下游兜底，保证脱离本函数
///    改造前既有的全部测试行为不变（测试环境的 cwd 未必是 crate 根，见下方
///    `resolve_assets_root_falls_back_to_env_macro_constant` pin）。
pub fn resolve_assets_root() -> PathBuf {
    if let Some(value) = std::env::var_os(BONG_ASSETS_DIR_ENV_VAR) {
        return PathBuf::from(value);
    }
    if let Ok(cwd) = std::env::current_dir() {
        if cwd.join("assets").is_dir() {
            return cwd;
        }
    }
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            if exe_dir.join("assets").is_dir() {
                return exe_dir.to_path_buf();
            }
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub fn register(app: &mut App) {
    tracing::info!("[bong][body_plan] registering body plan / race registries");

    let assets_root = resolve_assets_root();
    let plans_dir = assets_root.join(registry::DEFAULT_BODY_PLANS_DIR);
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

    let races_path = assets_root.join(race_registry::DEFAULT_RACES_PATH);
    let races =
        race_registry::RaceRegistry::load_file(&races_path, &body_plans).unwrap_or_else(|error| {
            panic!(
                "[bong][body_plan] failed to load races from {}: {error}",
                races_path.display()
            )
        });
    tracing::info!("[bong][body_plan] loaded {} race(s)", races.len());

    let layouts_dir = assets_root.join(layout::DEFAULT_BODY_PLAN_LAYOUTS_DIR);
    let layouts = layout::BodyPlanLayoutRegistry::load_dir(&layouts_dir, &body_plans)
        .unwrap_or_else(|error| {
            panic!(
                "[bong][body_plan] failed to load body plan layouts from {}: {error}",
                layouts_dir.display()
            )
        });
    tracing::info!(
        "[bong][body_plan] loaded {} body plan layout(s)",
        layouts.len()
    );

    app.insert_resource(body_plans);
    app.insert_resource(races);
    app.insert_resource(layouts);
}

#[cfg(test)]
mod resolve_assets_root_tests {
    use super::*;

    // 与 `world::mod` 既有的 `ScopedEnvVar`/`env_lock` 先例同构：修改进程级环境变量 /
    // 当前工作目录必须串行化，否则并发跑的其他测试可能读到中间态。
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    struct ScopedEnvVar {
        key: &'static str,
        previous: Option<std::ffi::OsString>,
    }

    impl ScopedEnvVar {
        fn set(key: &'static str, value: Option<&std::ffi::OsStr>) -> Self {
            let previous = std::env::var_os(key);
            if let Some(value) = value {
                std::env::set_var(key, value);
            } else {
                std::env::remove_var(key);
            }
            Self { key, previous }
        }
    }

    impl Drop for ScopedEnvVar {
        fn drop(&mut self) {
            if let Some(previous) = self.previous.take() {
                std::env::set_var(self.key, previous);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("current time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{nanos}-{}", std::process::id()))
    }

    #[test]
    fn resolve_assets_root_prefers_env_var_override_even_without_assets_subdir() {
        let _lock = env_lock();
        // 故意用一个**不存在** `assets/` 子目录的路径——env var 覆盖分支必须无条件
        // 采信调用方显式给出的路径，不像 cwd/`current_exe` 兜底那样先探测
        // `assets/` 是否存在才采信（那两步探测的前提是"猜"，env var 覆盖是"确定"）。
        let override_dir = unique_temp_dir("bong-assets-root-env-override");
        let _guard = ScopedEnvVar::set(BONG_ASSETS_DIR_ENV_VAR, Some(override_dir.as_os_str()));

        assert_eq!(
            resolve_assets_root(),
            override_dir,
            "BONG_ASSETS_DIR must win unconditionally over cwd/current_exe/CARGO_MANIFEST_DIR \
             fallbacks, even when the overridden path has no assets/ subdir yet"
        );
    }

    #[test]
    fn resolve_assets_root_falls_back_to_cwd_when_env_var_absent_and_cwd_has_assets_dir() {
        let _lock = env_lock();
        let _guard = ScopedEnvVar::set(BONG_ASSETS_DIR_ENV_VAR, None);

        let fake_root = unique_temp_dir("bong-assets-root-cwd-fallback");
        std::fs::create_dir_all(fake_root.join("assets"))
            .expect("temp assets/ subdir should be creatable");
        let original_cwd = std::env::current_dir().expect("cwd should be readable");
        std::env::set_current_dir(&fake_root).expect("should be able to chdir into fixture root");

        let resolved = resolve_assets_root();

        std::env::set_current_dir(&original_cwd).expect("must restore original cwd");

        assert_eq!(
            resolved, fake_root,
            "with no env override, a cwd that contains an assets/ subdir must be preferred over \
             the compile-time CARGO_MANIFEST_DIR fallback"
        );

        let _ = std::fs::remove_dir_all(&fake_root);
    }

    #[test]
    fn resolve_assets_root_falls_back_to_cargo_manifest_dir_when_nothing_else_matches() {
        let _lock = env_lock();
        let _guard = ScopedEnvVar::set(BONG_ASSETS_DIR_ENV_VAR, None);

        // cwd 挪到一个既没有 `assets/` 子目录、也不是 `current_exe()` 所在目录的空临时
        // 目录——两层探测都落空，必须兜到编译期 `CARGO_MANIFEST_DIR` 常量（dev/test
        // 环境下这个常量恒定有效，正是本函数改造前的原始行为，保证测试环境不受影响）。
        let empty_dir = unique_temp_dir("bong-assets-root-no-assets-anywhere");
        std::fs::create_dir_all(&empty_dir).expect("empty temp dir should be creatable");
        let original_cwd = std::env::current_dir().expect("cwd should be readable");
        std::env::set_current_dir(&empty_dir).expect("should be able to chdir into empty temp dir");

        let resolved = resolve_assets_root();

        std::env::set_current_dir(&original_cwd).expect("must restore original cwd");

        assert_eq!(
            resolved,
            PathBuf::from(env!("CARGO_MANIFEST_DIR")),
            "with no env override and no assets/ dir reachable via cwd (current_exe()'s directory \
             is target/debug/deps or similar during `cargo test`, which also has no assets/ \
             subdir in this repo), must fall back to the compile-time CARGO_MANIFEST_DIR constant"
        );

        let _ = std::fs::remove_dir_all(&empty_dir);
    }
}
