//! IPC schema definitions — Rust 侧，与 @bong/schema (TypeScript) 1:1 对应。
//!
//! 两端通过 `agent/packages/schema/samples/*.json` 做对齐校验。

pub mod channels;
pub mod common;
pub mod world_state;
pub mod agent_command;
pub mod narration;
pub mod chat_message;

// Re-exports
pub use channels::*;
pub use common::*;
pub use world_state::WorldStateV1;
pub use agent_command::AgentCommandV1;
pub use narration::NarrationV1;
pub use chat_message::ChatMessageV1;
