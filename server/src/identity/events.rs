//! plan-identity-v1 P1 内部 Bevy 事件。P3 时新增 `IdentityReactionChangedEvent`
//! （reaction.rs 落地后扩这里）。
//!
//! - [`IdentityCreatedEvent`]：`/identity new` 创建新 identity 后 emit
//! - [`IdentitySwitchedEvent`]：`/identity switch <id>` 成功切换后 emit（含 from / to）

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Entity, Event};

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
