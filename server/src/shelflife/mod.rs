//! 保质期 / 过期系统（plan-shelflife-v1 M0 — 纯函数层）。
//!
//! 三路降级路径：
//! - **Decay** — 灵气逸散（灵石 / 骨币 / 残卷），减效不致伤
//! - **Spoil** — 腐败（兽血 / 兽肉 / 鲜草 / 过期丹），触发 contam
//! - **Age** — 陈化（陈酒 / 老坛丹），峰值超值 + 过峰 → Spoil 迁移
//!
//! M0 仅包含类型 + 纯函数 + 单测，**不**挂任何 System / Event / Resource。

pub mod compute;
pub mod types;

pub use compute::{compute_current_qi, compute_track_state};
pub use types::{DecayFormula, DecayProfile, DecayProfileId, DecayTrack, Freshness, TrackState};
