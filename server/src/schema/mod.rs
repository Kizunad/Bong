//! IPC schema definitions — Rust 侧，与 @bong/schema (TypeScript) 1:1 对应。
//!
//! 两端通过 `agent/packages/schema/samples/*.json` 做对齐校验。

pub mod agent_command;
pub mod channels;
pub mod chat_message;
pub mod client_payload;
pub mod common;
pub mod narration;
pub mod world_state;
