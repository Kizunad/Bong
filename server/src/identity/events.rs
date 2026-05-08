//! plan-identity-v1 P1 + P3 内部 Bevy 事件。
//!
//! - [`IdentityCreatedEvent`]：`/identity new` 创建新 identity 后 emit
//! - [`IdentitySwitchedEvent`]：`/identity switch <id>` 成功切换后 emit（含 from / to）
//! - [`IdentityReactionChangedEvent`]：active identity 的反应分级跨 tier 边界后 emit
//!   （P3 reaction.rs 状态机消费）

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Entity, Event};

use super::reaction::ReactionTier;
use super::IdentityId;

/// `/identity new <display_name>` 成功创建 + 立即激活后 emit。
///
/// `previous_active` 为创建瞬间被冻结的旧 active id（用于审计 / agent 推送）。
#[derive(Debug, Clone, Event, Serialize, Deserialize, PartialEq)]
pub struct IdentityCreatedEvent {
    pub player: Entity,
    pub identity_id: IdentityId,
    pub display_name: String,
    pub previous_active: IdentityId,
    pub at_tick: u64,
}

/// `/identity switch <id>` 成功切换后 emit。`from` 现在 `frozen=true`、`to` 现在 `frozen=false`。
#[derive(Debug, Clone, Event, Serialize, Deserialize, PartialEq)]
pub struct IdentitySwitchedEvent {
    pub player: Entity,
    pub from: IdentityId,
    pub to: IdentityId,
    pub at_tick: u64,
}

/// 玩家 active identity 的 [`ReactionTier`] 跨边界后 emit。
///
/// 发射条件 = `from_tier != to_tier`（同 tier 内 reputation_score 漂移不发射）。
/// 由 [`crate::identity::reaction::update_identity_reaction_state`] 触发，下游
/// NPC big-brain blackboard / agent wanted 链路可订阅。
#[derive(Debug, Clone, Event, Serialize, Deserialize, PartialEq)]
pub struct IdentityReactionChangedEvent {
    pub player: Entity,
    pub identity_id: IdentityId,
    pub from_tier: ReactionTier,
    pub to_tier: ReactionTier,
    pub at_tick: u64,
}
