//! R1 交互会话 contract-first 框架。
//!
//! 本模块冻结 gameplay session 的 identity、生命周期 reducer 与 busy claim 语义；它
//! 不安装 Bevy resource，不替换现有 `craft::CraftSession`，也不把 alchemy、forge 或
//! 其他域接入生产调度。后续 domain adapter 只能通过这里的 typed contract 连接到
//! 持久化、wire 和 delivery owner。

mod lifecycle;
mod registry;

pub use lifecycle::{
    admit_session, reduce_session, reduce_session_with_context, AdmissionOutcome, AdmissionRequest,
    AuditEffect, BusyClaim, CheckpointEffect, ClaimEffect, CraftAdapter, CraftIntent,
    CraftOpenRejected, CraftSessionAdapter, CraftSessionProjection, CraftSessionTransition,
    HandoffEffect, IdentityError, InteractionSession, MaintenanceAuthority, PlayerKey,
    RuntimeBinding, SessionDecision, SessionDurability, SessionEvent, SessionIdentity, SessionKey,
    SessionLifecycleCtx, SessionRecord, SessionRejection, SessionState, SuspensionResult,
    TerminationCause, MAX_ACTIVE_SESSION_DELIVERY_BYTES, MAX_ACTIVE_SESSION_DELIVERY_ROWS,
    MAX_SESSION_DELIVERY_HISTORY_BYTES, MAX_SESSION_DELIVERY_HISTORY_ROWS,
    SESSION_DELIVERY_MAX_ATTEMPTS, SESSION_DELIVERY_MAX_PAYLOAD_BYTES,
    SESSION_DELIVERY_MAX_RETRY_AGE_TICKS, SESSION_DELIVERY_RESULT_REPLAY_TTL_TICKS,
    SESSION_DELIVERY_TOMBSTONE_TTL_TICKS, SESSION_SUSPENSION_SCAN_CADENCE_TICKS,
    SESSION_SUSPENSION_TTL_TICKS,
};
pub use registry::{BusyClaimConflict, BusyClaimRegistry, SessionRegistry, SessionRegistryError};

/// R3 owns persistence for this guard; R1 owns when the guard is emitted by a lifecycle
/// transition. Re-exporting the type keeps the reducer contract independent of SQLite details
/// while preserving one canonical wire/persistence shape.
pub use crate::persistence::{CraftRestoreGuard, ReconnectGuard};
