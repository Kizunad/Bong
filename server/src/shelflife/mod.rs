//! 保质期 / 过期系统（plan-shelflife-v1）。
//!
//! 三路降级路径：
//! - **Decay** — 灵气逸散（灵石 / 骨币 / 残卷），减效不致伤
//! - **Spoil** — 腐败（兽血 / 兽肉 / 鲜草 / 过期丹），触发 contam
//! - **Age** — 陈化（陈酒 / 老坛丹），峰值超值 + 过峰 → Spoil 迁移
//!
//! 落地阶段：
//! - **M0** — 类型 + 纯函数（compute_current_qi / compute_track_state）+ 单测
//! - **M1** — Item NBT 接入 inventory（Freshness 字段写入 ItemInstance / wire schema）
//! - **M2** — 容器行为（ContainerFreshnessBehavior + enter/exit 冻结记账，纯函数层）
//! - **M3a** — DecayProfileRegistry resource + snapshot 衍生数据（current_qi / track_state）
//! - **M4a** — 神识感知 FreshnessProbeIntent + 凝脉+ 修为 gate 解析器
//! - **M5a** — 消费侧 helper + event 基础设施（decay_current_qi_factor / spoil_check / age_peak_check + SpoilConsumeWarning / AgeBonusRoll）
//! - M5b+ — alchemy / pill consume 接入 / 死物变体 / 跨 plan 参数定稿

pub mod compute;
pub mod consume;
pub mod container;
pub mod probe;
pub mod registry;
pub mod sweep;
pub mod types;
pub mod variant;

#[allow(unused_imports)]
pub use compute::{
    combine_storage_and_zone_multiplier, compute_current_qi, compute_track_state,
    zone_multiplier_lookup, DEAD_ZONE_SHELFLIFE_MULTIPLIER,
};
#[allow(unused_imports)]
pub use consume::{
    age_peak_check, decay_current_qi_factor, spoil_check, AgeBonusRoll, AgePeakCheck,
    SpoilCheckOutcome, SpoilConsumeWarning, SpoilSeverity, CRITICAL_BLOCK_RATIO,
};
#[allow(unused_imports)]
pub use container::{container_storage_multiplier, enter_container, exit_container};
#[allow(unused_imports)]
pub use probe::{
    resolve_freshness_probe_intents, FreshnessProbeIntent, FreshnessProbeResponse,
    ProbeDenialReason, ProbeResult,
};
pub use registry::{build_default_registry, DecayProfileRegistry};
#[allow(unused_imports)]
pub use types::{
    ContainerFreshnessBehavior, DecayFormula, DecayProfile, DecayProfileId, DecayTrack, Freshness,
    TrackState,
};
#[allow(unused_imports)]
pub use variant::apply_variant_switch;

/// plan-shelflife-v1 M3a + M4a + M5a + M6 — 注册 shelflife 资源 + 事件 + 系统。
/// - DecayProfileRegistry 默认注册 active plan 的生产 profile
/// - FreshnessProbeIntent/Response 事件 + resolver 系统（M4a）
/// - SpoilConsumeWarning / AgeBonusRoll 事件总线（M5a，consumer 侧 emit；M5b+ 接 alchemy/pill 调用）
/// - 变体 sweep 系统（M6 tick 200）
pub fn register(app: &mut valence::prelude::App) {
    use valence::prelude::Update;
    app.insert_resource(build_default_registry());
    app.add_event::<FreshnessProbeIntent>();
    app.add_event::<FreshnessProbeResponse>();
    app.add_event::<SpoilConsumeWarning>();
    app.add_event::<AgeBonusRoll>();
    app.add_systems(Update, resolve_freshness_probe_intents);
    sweep::register_sweep(app);
}
