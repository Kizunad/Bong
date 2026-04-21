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
//! - M3b+ — client tooltip / 神识感知 / 消费侧接入 / 死物变体 / 跨 plan 参数定稿

pub mod compute;
pub mod container;
pub mod registry;
pub mod types;

pub use compute::{compute_current_qi, compute_track_state};
pub use container::{container_storage_multiplier, enter_container, exit_container};
pub use registry::DecayProfileRegistry;
pub use types::{
    ContainerFreshnessBehavior, DecayFormula, DecayProfile, DecayProfileId, DecayTrack, Freshness,
    TrackState,
};

/// plan-shelflife-v1 M3a — 将 DecayProfileRegistry 作为默认空 resource 插入 App。
/// M7 正式定稿时由各 plan（mineral / fauna / botany / alchemy / food / forge）
/// 调用 `app.world_mut().resource_mut::<DecayProfileRegistry>().insert(...)` 填充。
pub fn register(app: &mut valence::prelude::App) {
    app.insert_resource(DecayProfileRegistry::default());
}
