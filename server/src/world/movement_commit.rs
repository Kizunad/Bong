//! 权威移动写入的统一阶段。
//!
//! 修改玩家位置或维度的 Update 系统进入本集合；依赖最终位置的校验排在其后。
//TODO:lingtian_refactor 新田块交互校验需要排在权威移动提交后。

use valence::prelude::bevy_ecs;

/// Update 内所有权威玩家位置/维度写入的统一点。
#[derive(bevy_ecs::schedule::SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AuthoritativePositionCommitSet;
