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
//! - M4b+ — client tooltip / 消费侧接入 / 死物变体 / 跨 plan 参数定稿

pub mod compute;
pub mod container;
pub mod probe;
pub mod registry;
pub mod types;

pub use compute::{compute_current_qi, compute_track_state};
pub use container::{container_storage_multiplier, enter_container, exit_container};
pub use probe::{
    resolve_freshness_probe_intents, FreshnessProbeIntent, FreshnessProbeResponse,
    ProbeDenialReason, ProbeResult,
};
pub use registry::DecayProfileRegistry;
pub use types::{
    ContainerFreshnessBehavior, DecayFormula, DecayProfile, DecayProfileId, DecayTrack, Freshness,
    TrackState,
};

/// plan-shelflife-v1 M3a + M4a — 注册 shelflife 资源 + 事件 + 系统。
/// - DecayProfileRegistry 空骨架，M7 由各消费 plan 填充
/// - FreshnessProbeIntent/Response 事件 + resolver 系统
pub fn register(app: &mut valence::prelude::App) {
    use valence::prelude::Update;
    app.insert_resource(DecayProfileRegistry::default());
    app.add_event::<FreshnessProbeIntent>();
    app.add_event::<FreshnessProbeResponse>();
    app.add_systems(Update, resolve_freshness_probe_intents);
}
