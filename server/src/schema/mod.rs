//! IPC schema definitions — Rust 侧，与 @bong/schema (TypeScript) 1:1 对应。
//!
//! 两端通过 `agent/packages/schema/samples/*.json` 做对齐校验。

pub mod agent_command;
pub mod agent_world_model;
pub mod alchemy;
pub mod armor_event;
pub mod botany;
pub mod channels;
pub mod chat_message;
pub mod client_payload;
pub mod client_request;
pub mod combat_event;
pub mod combat_hud;
pub mod common;
pub mod cultivation;
pub mod inventory;
pub mod lingtian;
pub mod narration;
pub mod server_data;
pub mod skill;
pub mod vfx_event;
pub mod world_state;
