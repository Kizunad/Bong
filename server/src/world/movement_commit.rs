//! fix-spec-1901-v2 §4.2 — 权威移动写入统一 commit set。
//!
//! 所有能改变玩家权威 `Position` / `CurrentDimension` 的系统必须进入本 set：
//! 灵田的 post-transfer validator 与 completion 复验都排在它之后，保证
//! "本 tick 所有移动/维度写入已完成"是唯一可读状态。`DimensionTransferSet`
//! 继续作为本 set 的成员（或成员之一），不再是灵田排序的唯一依据。
//!
//! 约束：
//! ```text
//! AuthoritativePositionCommitSet
//!     → LingtianPostTransferValidationSet
//!     → LingtianStartSet
//!     → tick_lingtian_sessions
//!     → apply_completed_sessions
//! ```
//!
//! 若某 writer 位于 `PostUpdate` 或其它晚于 `Update` 的 schedule，必须把
//! validator/completion 同样移到其后的统一阶段；不允许"set 名称存在但仍有
//! writer 在 set 外晚写"的假排序。

use valence::prelude::bevy_ecs;

/// Update 内所有权威玩家位置/维度写入的统一点。
#[derive(bevy_ecs::schedule::SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AuthoritativePositionCommitSet;
