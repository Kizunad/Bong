//! Session identity, lifecycle reducer, and the declared craft adapter seam.
//!
//! The reducer is deliberately side-effect free: it returns claim/checkpoint/handoff effects
//! for the owning systems to commit. This keeps a rejected or stale event from mutating a
//! player, inventory, qi ledger, or durable obligation before the later atomic activation wave.

use std::fmt;

use serde::{Deserialize, Serialize};

use super::ReconnectGuard;

/// Suspended sessions remain reclaimable for exactly one day of server ticks.
pub const SESSION_SUSPENSION_TTL_TICKS: u64 = 1_728_000;
/// The scanner need not inspect suspended sessions every tick.
pub const SESSION_SUSPENSION_SCAN_CADENCE_TICKS: u64 = 1_200;
/// Canonical terminal payload upper bound (1 MiB).
pub const SESSION_DELIVERY_MAX_PAYLOAD_BYTES: u64 = 1_048_576;
/// Aggregate active obligation row quota.
pub const MAX_ACTIVE_SESSION_DELIVERY_ROWS: u64 = 4_096;
/// Aggregate active obligation byte quota (4 GiB).
pub const MAX_ACTIVE_SESSION_DELIVERY_BYTES: u64 = 4_294_967_296;
/// An obligation becomes dead-lettered on the eighth attempt.
pub const SESSION_DELIVERY_MAX_ATTEMPTS: u32 = 8;
/// An obligation cannot be retried beyond one day of runtime ticks.
pub const SESSION_DELIVERY_MAX_RETRY_AGE_TICKS: u64 = 1_728_000;
/// Complete receipt/disposition replay horizon (seven days).
pub const SESSION_DELIVERY_RESULT_REPLAY_TTL_TICKS: u64 = 12_096_000;
/// Bounded tombstone horizon (thirty days).
pub const SESSION_DELIVERY_TOMBSTONE_TTL_TICKS: u64 = 51_840_000;
/// Maximum retained receipt/disposition rows.
pub const MAX_SESSION_DELIVERY_HISTORY_ROWS: u64 = 65_536;
/// Maximum retained receipt/disposition bytes.
pub const MAX_SESSION_DELIVERY_HISTORY_BYTES: u64 = 134_217_728;

/// A validated, stable session identity. Runtime ECS entities are never used as this key.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SessionKey(String);

impl SessionKey {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn try_new(value: impl Into<String>) -> Result<Self, IdentityError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(IdentityError::Empty("session_key"));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for SessionKey {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for SessionKey {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for SessionKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// A canonical player identity. Display names and runtime entities are not authorization keys.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PlayerKey(String);

impl PlayerKey {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn try_new(value: impl Into<String>) -> Result<Self, IdentityError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(IdentityError::Empty("owner_key"));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for PlayerKey {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for PlayerKey {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for PlayerKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Identity construction failure. Empty keys are rejected before any claim or ledger effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityError {
    Empty(&'static str),
}

impl fmt::Display for IdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty(field) => write!(formatter, "{field} must not be empty"),
        }
    }
}

impl std::error::Error for IdentityError {}

/// The two gameplay ownership policies supported by R1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SessionDurability {
    Checkpointed,
    Volatile,
}

/// Canonical gameplay state. `Absent` is used only by the admission reducer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SessionState {
    Absent,
    Running,
    Paused,
    Suspended,
    HandoffPreparing,
    Ended,
}

/// A terminal reason is part of the canonical handoff payload, not a log-only label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TerminationCause {
    Completed,
    VoluntaryCancel,
    Disconnect,
    DimensionChange,
    Shutdown,
    InvalidRestore,
    SuspensionExpired,
    AuthorizedAdministratorClosure,
}

/// The three centralized busy namespaces. Claims conflict only within the same namespace.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BusyClaim {
    PlayerExclusive(PlayerKey),
    TargetExclusive(String),
    FacilityExclusive(String),
}

impl BusyClaim {
    /// Builds a player-exclusive claim from the canonical player identity.
    pub fn player(player: impl Into<PlayerKey>) -> Self {
        Self::PlayerExclusive(player.into())
    }

    /// Builds a target-exclusive claim from a stable target identity.
    pub fn target(target: impl Into<String>) -> Self {
        Self::TargetExclusive(target.into())
    }

    /// Builds a facility-exclusive claim from a stable facility identity.
    pub fn facility(facility: impl Into<String>) -> Self {
        Self::FacilityExclusive(facility.into())
    }

    /// Reports whether the claim carries a non-empty canonical identity.
    pub fn is_valid(&self) -> bool {
        match self {
            Self::PlayerExclusive(player) => !player.as_str().trim().is_empty(),
            Self::TargetExclusive(target) | Self::FacilityExclusive(target) => {
                !target.trim().is_empty()
            }
        }
    }

    /// Reports whether two claims address the same exclusive namespace and identity.
    pub fn conflicts(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::PlayerExclusive(left), Self::PlayerExclusive(right)) => left == right,
            (Self::TargetExclusive(left), Self::TargetExclusive(right)) => left == right,
            (Self::FacilityExclusive(left), Self::FacilityExclusive(right)) => left == right,
            _ => false,
        }
    }
}

/// Runtime locator kept separate from durable `SessionKey`/`placed_id` identities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeBinding {
    pub entity_id: u64,
    pub placed_id: Option<String>,
    pub dimension: String,
}

/// Identity envelope carried by every non-admission gameplay event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionIdentity {
    pub session_key: SessionKey,
    pub owner_key: PlayerKey,
    pub generation: u64,
}

/// A domain-neutral session record used by the contract tests and adapters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRecord {
    pub session_key: SessionKey,
    pub owner_key: PlayerKey,
    pub durability: SessionDurability,
    pub state: SessionState,
    pub generation: u64,
    pub phase_revision: u64,
    pub busy_claim: BusyClaim,
    pub runtime_binding: Option<RuntimeBinding>,
    pub handoff_origin: Option<SessionState>,
    /// The one-time reconnect capability retained while the session is suspended.
    pub restore_token: Option<String>,
}

impl SessionRecord {
    /// Creates a running record for a trusted admission owner.
    pub fn new(
        session_key: impl Into<SessionKey>,
        owner_key: impl Into<PlayerKey>,
        durability: SessionDurability,
        busy_claim: BusyClaim,
    ) -> Self {
        Self {
            session_key: session_key.into(),
            owner_key: owner_key.into(),
            durability,
            state: SessionState::Running,
            generation: 0,
            phase_revision: 0,
            busy_claim,
            runtime_binding: None,
            handoff_origin: None,
            restore_token: None,
        }
    }

    /// Returns the identity envelope used by subsequent lifecycle events.
    pub fn identity(&self) -> SessionIdentity {
        SessionIdentity {
            session_key: self.session_key.clone(),
            owner_key: self.owner_key.clone(),
            generation: self.generation,
        }
    }

    fn bump_revision(&mut self) {
        self.phase_revision = self.phase_revision.saturating_add(1);
    }
}

/// A lifecycle context is intentionally data-only; an owner commits its effects after reduce.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionLifecycleCtx {
    pub current_tick: u64,
    pub executor_id: Option<String>,
}

/// Server console or a capability bound to the current executor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaintenanceAuthority {
    ServerConsole,
    BoundCapability { principal: String, executor: String },
    PlayerRequest { principal: String },
}

impl MaintenanceAuthority {
    fn is_authorized(&self, executor_id: Option<&str>) -> bool {
        match self {
            Self::ServerConsole => true,
            Self::BoundCapability { executor, .. } => {
                executor_id.is_some_and(|current| current == executor)
            }
            Self::PlayerRequest { .. } => false,
        }
    }
}

/// The result of a durable S-07 checkpoint fence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SuspensionResult {
    Committed(ReconnectGuard),
    Failed,
}

/// Events accepted by the canonical session reducer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionEvent {
    Pause {
        identity: SessionIdentity,
    },
    Resume {
        identity: SessionIdentity,
    },
    VoluntaryCancel {
        identity: SessionIdentity,
    },
    Completed {
        identity: SessionIdentity,
    },
    Disconnect {
        identity: SessionIdentity,
        suspension: SuspensionResult,
    },
    Shutdown {
        identity: SessionIdentity,
        suspension: SuspensionResult,
    },
    DimensionChange {
        identity: SessionIdentity,
    },
    Restore {
        identity: SessionIdentity,
        guard: ReconnectGuard,
        phase_revision: u64,
    },
    SuspensionExpired {
        identity: SessionIdentity,
    },
    AuthorizedAdministratorClosure {
        identity: SessionIdentity,
        authority: MaintenanceAuthority,
        reason: String,
    },
    HandoffCommitted {
        identity: SessionIdentity,
    },
    HandoffRolledBack {
        identity: SessionIdentity,
    },
    StartupEpochDetected {
        identity: SessionIdentity,
        guard: ReconnectGuard,
    },
    CraftStart {
        identity: SessionIdentity,
        recipe_id: String,
        quantity: u32,
    },
}

/// Admission input captured before a reservation/claim transaction is attempted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmissionRequest {
    pub request_id: Option<String>,
    pub session_key: SessionKey,
    pub owner_key: PlayerKey,
    pub durability: SessionDurability,
    pub busy_claim: BusyClaim,
}

/// Reservation result from the durable obligation owner plus the runtime claim race.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionOutcome {
    Rejected,
    ReservedClaimWon,
    ReservedClaimLost,
}

/// A typed rejection that remains correlated to a valid CraftOpen request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CraftOpenRejected {
    pub request_id: String,
    pub reason: String,
}

/// Reasons a lifecycle event is rejected without changing state or claims.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionRejection {
    InvalidIdentity,
    StaleGeneration,
    FutureGeneration,
    InvalidState,
    InvalidRestore,
    DurableCheckpointFailed,
    MaintenanceDenied,
    MissingRequestId,
    ReservationRejected,
    InvalidCraftStart,
}

/// Claim mutation requested by the reducer; the registry/owner performs the actual mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaimEffect {
    None,
    Acquire(BusyClaim),
    Retain,
    Release(BusyClaim),
}

/// Durable checkpoint mutation requested by the reducer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckpointEffect {
    None,
    PersistReconnectGuard(ReconnectGuard),
    ConsumeReconnectGuard,
    NormalizeSuspended(ReconnectGuard),
}

/// Delivery owner handoff intent. No inventory, qi, or SQLite mutation occurs here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandoffEffect {
    None,
    Prepare(TerminationCause),
    Commit,
    Retry,
}

/// Reducer output. `rejection` being `Some` means all other effects are no-op.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionDecision {
    pub accepted: bool,
    pub next_state: SessionState,
    /// The newly admitted record, present only for a successful reservation/claim race.
    pub admitted_session: Option<SessionRecord>,
    pub handoff: HandoffEffect,
    pub claim_effect: ClaimEffect,
    pub checkpoint_effect: CheckpointEffect,
    pub audit_effect: AuditEffect,
    pub rejection: Option<SessionRejection>,
    pub correlated_rejection: Option<CraftOpenRejected>,
}

impl SessionDecision {
    fn accepted(next_state: SessionState) -> Self {
        Self {
            accepted: true,
            next_state,
            admitted_session: None,
            handoff: HandoffEffect::None,
            claim_effect: ClaimEffect::None,
            checkpoint_effect: CheckpointEffect::None,
            audit_effect: AuditEffect::None,
            rejection: None,
            correlated_rejection: None,
        }
    }

    fn rejected(state: SessionState, reason: SessionRejection) -> Self {
        Self {
            accepted: false,
            next_state: state,
            admitted_session: None,
            handoff: HandoffEffect::None,
            claim_effect: ClaimEffect::None,
            checkpoint_effect: CheckpointEffect::None,
            audit_effect: AuditEffect::None,
            rejection: Some(reason),
            correlated_rejection: None,
        }
    }

    pub fn is_rejected(&self) -> bool {
        !self.accepted
    }
}

/// Audit is deliberately a small typed marker in P1; persistence owns its durable encoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditEffect {
    None,
    Record(&'static str),
}

/// The only trait a domain adapter must implement. Production registration is deferred to M-10.
pub trait InteractionSession: Send + Sync {
    fn session_key(&self) -> SessionKey;
    fn owner_key(&self) -> &PlayerKey;
    fn durability(&self) -> SessionDurability;
    fn busy_claim(&self) -> BusyClaim;
    fn reduce(&mut self, event: SessionEvent, ctx: &mut SessionLifecycleCtx) -> SessionDecision;
}

impl InteractionSession for SessionRecord {
    fn session_key(&self) -> SessionKey {
        self.session_key.clone()
    }

    fn owner_key(&self) -> &PlayerKey {
        &self.owner_key
    }

    fn durability(&self) -> SessionDurability {
        self.durability
    }

    fn busy_claim(&self) -> BusyClaim {
        self.busy_claim.clone()
    }

    fn reduce(&mut self, event: SessionEvent, ctx: &mut SessionLifecycleCtx) -> SessionDecision {
        reduce_session_with_context(self, event, ctx)
    }
}

fn identity_rejection(
    state: &SessionRecord,
    identity: &SessionIdentity,
) -> Option<SessionRejection> {
    if state.session_key != identity.session_key || state.owner_key != identity.owner_key {
        Some(SessionRejection::InvalidIdentity)
    } else if identity.generation < state.generation {
        Some(SessionRejection::StaleGeneration)
    } else if identity.generation > state.generation {
        Some(SessionRejection::FutureGeneration)
    } else {
        None
    }
}

fn check_identity(state: &SessionRecord, identity: &SessionIdentity) -> Option<SessionDecision> {
    identity_rejection(state, identity).map(|reason| SessionDecision::rejected(state.state, reason))
}

fn prepare_handoff(state: &mut SessionRecord, cause: TerminationCause) -> SessionDecision {
    state.handoff_origin = Some(state.state);
    state.state = SessionState::HandoffPreparing;
    state.bump_revision();
    let mut decision = SessionDecision::accepted(state.state);
    decision.handoff = HandoffEffect::Prepare(cause);
    decision.claim_effect = ClaimEffect::Retain;
    decision
}

/// Reduce a non-admission event with a fresh, no-authority context.
pub fn reduce_session(state: &mut SessionRecord, event: SessionEvent) -> SessionDecision {
    reduce_session_with_context(state, event, &mut SessionLifecycleCtx::default())
}

/// Canonical gameplay reducer. Effects are declarations; their owners commit them atomically.
pub fn reduce_session_with_context(
    state: &mut SessionRecord,
    event: SessionEvent,
    context: &mut SessionLifecycleCtx,
) -> SessionDecision {
    let identity = match &event {
        SessionEvent::Pause { identity }
        | SessionEvent::Resume { identity }
        | SessionEvent::VoluntaryCancel { identity }
        | SessionEvent::Completed { identity }
        | SessionEvent::Disconnect { identity, .. }
        | SessionEvent::Shutdown { identity, .. }
        | SessionEvent::DimensionChange { identity }
        | SessionEvent::Restore { identity, .. }
        | SessionEvent::SuspensionExpired { identity }
        | SessionEvent::AuthorizedAdministratorClosure { identity, .. }
        | SessionEvent::HandoffCommitted { identity }
        | SessionEvent::HandoffRolledBack { identity }
        | SessionEvent::StartupEpochDetected { identity, .. }
        | SessionEvent::CraftStart { identity, .. } => identity,
    };
    if let Some(decision) = check_identity(state, identity) {
        return decision;
    }

    match event {
        SessionEvent::Pause { .. } => {
            if state.state != SessionState::Running {
                return SessionDecision::rejected(state.state, SessionRejection::InvalidState);
            }
            state.state = SessionState::Paused;
            state.bump_revision();
            let mut decision = SessionDecision::accepted(state.state);
            decision.claim_effect = ClaimEffect::Retain;
            decision
        }
        SessionEvent::Resume { .. } => {
            if state.state != SessionState::Paused {
                return SessionDecision::rejected(state.state, SessionRejection::InvalidState);
            }
            state.state = SessionState::Running;
            state.bump_revision();
            let mut decision = SessionDecision::accepted(state.state);
            decision.claim_effect = ClaimEffect::Retain;
            decision
        }
        SessionEvent::VoluntaryCancel { .. } => match state.state {
            SessionState::Running | SessionState::Paused | SessionState::Suspended => {
                prepare_handoff(state, TerminationCause::VoluntaryCancel)
            }
            _ => SessionDecision::rejected(state.state, SessionRejection::InvalidState),
        },
        SessionEvent::Completed { .. } => match state.state {
            SessionState::Running | SessionState::Paused => {
                prepare_handoff(state, TerminationCause::Completed)
            }
            _ => SessionDecision::rejected(state.state, SessionRejection::InvalidState),
        },
        SessionEvent::Disconnect {
            suspension,
            identity: _,
        } => reduce_disconnect(state, TerminationCause::Disconnect, suspension),
        SessionEvent::Shutdown {
            suspension,
            identity: _,
        } => reduce_disconnect(state, TerminationCause::Shutdown, suspension),
        SessionEvent::DimensionChange { .. } => match state.state {
            SessionState::Running | SessionState::Paused => {
                prepare_handoff(state, TerminationCause::DimensionChange)
            }
            _ => SessionDecision::rejected(state.state, SessionRejection::InvalidState),
        },
        SessionEvent::Restore {
            guard,
            phase_revision,
            ..
        } => reduce_restore(state, guard, phase_revision),
        SessionEvent::SuspensionExpired { .. } => {
            if state.state != SessionState::Suspended {
                return SessionDecision::rejected(state.state, SessionRejection::InvalidState);
            }
            prepare_handoff(state, TerminationCause::SuspensionExpired)
        }
        SessionEvent::AuthorizedAdministratorClosure {
            authority, reason, ..
        } => {
            if state.state != SessionState::Suspended
                || !authority.is_authorized(context.executor_id.as_deref())
            {
                return SessionDecision::rejected(
                    state.state,
                    if authority.is_authorized(context.executor_id.as_deref()) {
                        SessionRejection::InvalidState
                    } else {
                        SessionRejection::MaintenanceDenied
                    },
                );
            }
            let mut decision =
                prepare_handoff(state, TerminationCause::AuthorizedAdministratorClosure);
            decision.audit_effect = if reason.trim().is_empty() {
                AuditEffect::Record("admin-close-without-reason")
            } else {
                AuditEffect::Record("authorized-admin-close")
            };
            decision
        }
        SessionEvent::HandoffCommitted { .. } => {
            if state.state != SessionState::HandoffPreparing {
                return SessionDecision::rejected(state.state, SessionRejection::InvalidState);
            }
            state.state = SessionState::Ended;
            state.handoff_origin = None;
            state.restore_token = None;
            state.bump_revision();
            let mut decision = SessionDecision::accepted(state.state);
            decision.handoff = HandoffEffect::Commit;
            decision.claim_effect = ClaimEffect::Release(state.busy_claim.clone());
            decision.checkpoint_effect = CheckpointEffect::ConsumeReconnectGuard;
            decision
        }
        SessionEvent::HandoffRolledBack { .. } => {
            if state.state != SessionState::HandoffPreparing {
                return SessionDecision::rejected(state.state, SessionRejection::InvalidState);
            }
            state.state = state.handoff_origin.take().unwrap_or(SessionState::Paused);
            state.bump_revision();
            let mut decision = SessionDecision::accepted(state.state);
            decision.handoff = HandoffEffect::Retry;
            decision.claim_effect = ClaimEffect::Retain;
            decision
        }
        SessionEvent::StartupEpochDetected { guard, .. } => {
            if !matches!(state.state, SessionState::Running | SessionState::Paused) {
                return SessionDecision::rejected(state.state, SessionRejection::InvalidState);
            }
            if guard.session_key != state.session_key.as_str()
                || guard.owner_key != state.owner_key.as_str()
                || guard.generation != state.generation
                || guard.phase_revision > state.phase_revision
                || guard.restore_token.trim().is_empty()
            {
                return SessionDecision::rejected(state.state, SessionRejection::InvalidRestore);
            }
            let Some(next_generation) = state.generation.checked_add(1) else {
                return SessionDecision::rejected(state.state, SessionRejection::InvalidRestore);
            };
            state.generation = next_generation;
            state.state = SessionState::Suspended;
            state.bump_revision();
            let normalized_guard = ReconnectGuard {
                generation: state.generation,
                phase_revision: state.phase_revision,
                ..guard
            };
            state.restore_token = Some(normalized_guard.restore_token.clone());
            let mut decision = SessionDecision::accepted(state.state);
            decision.checkpoint_effect = CheckpointEffect::NormalizeSuspended(normalized_guard);
            decision.claim_effect = ClaimEffect::Retain;
            decision
        }
        SessionEvent::CraftStart {
            recipe_id,
            quantity,
            ..
        } => {
            if state.state != SessionState::Running || recipe_id.trim().is_empty() || quantity == 0
            {
                return SessionDecision::rejected(state.state, SessionRejection::InvalidCraftStart);
            }
            state.bump_revision();
            let mut decision = SessionDecision::accepted(state.state);
            decision.audit_effect = AuditEffect::Record("craft-start-admitted");
            decision
        }
    }
}

fn reduce_disconnect(
    state: &mut SessionRecord,
    cause: TerminationCause,
    suspension: SuspensionResult,
) -> SessionDecision {
    if state.state == SessionState::Suspended {
        return SessionDecision::accepted(SessionState::Suspended);
    }
    if !matches!(state.state, SessionState::Running | SessionState::Paused) {
        return SessionDecision::rejected(state.state, SessionRejection::InvalidState);
    }
    if state.durability == SessionDurability::Checkpointed {
        let SuspensionResult::Committed(guard) = suspension else {
            return SessionDecision::rejected(
                state.state,
                SessionRejection::DurableCheckpointFailed,
            );
        };
        state.state = SessionState::Suspended;
        state.restore_token = Some(guard.restore_token.clone());
        state.bump_revision();
        let mut decision = SessionDecision::accepted(state.state);
        decision.checkpoint_effect = CheckpointEffect::PersistReconnectGuard(guard);
        decision.claim_effect = ClaimEffect::Retain;
        return decision;
    }
    prepare_handoff(state, cause)
}

fn reduce_restore(
    state: &mut SessionRecord,
    guard: ReconnectGuard,
    phase_revision: u64,
) -> SessionDecision {
    if state.state != SessionState::Suspended
        || guard.session_key != state.session_key.as_str()
        || guard.owner_key != state.owner_key.as_str()
        || guard.generation != state.generation
        || state.restore_token.as_deref() != Some(guard.restore_token.as_str())
        || phase_revision <= state.phase_revision
    {
        return SessionDecision::rejected(state.state, SessionRejection::InvalidRestore);
    }
    state.state = SessionState::Paused;
    state.phase_revision = phase_revision;
    state.restore_token = None;
    let mut decision = SessionDecision::accepted(state.state);
    decision.checkpoint_effect = CheckpointEffect::ConsumeReconnectGuard;
    decision.claim_effect = ClaimEffect::Retain;
    decision.audit_effect = AuditEffect::Record("guarded-restore");
    decision
}

/// Apply an admission reservation result and describe the record to register on a claim win.
pub fn admit_session(request: AdmissionRequest, outcome: AdmissionOutcome) -> SessionDecision {
    let valid_request = request
        .request_id
        .as_deref()
        .is_some_and(|request_id| !request_id.trim().is_empty());
    if !valid_request {
        let mut decision =
            SessionDecision::rejected(SessionState::Absent, SessionRejection::MissingRequestId);
        decision.audit_effect = AuditEffect::Record("craft-open-parse-rejection");
        return decision;
    }

    if request.session_key.as_str().trim().is_empty()
        || request.owner_key.as_str().trim().is_empty()
        || !request.busy_claim.is_valid()
    {
        let mut decision =
            SessionDecision::rejected(SessionState::Absent, SessionRejection::InvalidIdentity);
        decision.correlated_rejection = Some(CraftOpenRejected {
            request_id: request.request_id.expect("validated request id"),
            reason: "invalid session identity".to_string(),
        });
        return decision;
    }

    match outcome {
        AdmissionOutcome::Rejected => {
            let mut decision = SessionDecision::rejected(
                SessionState::Absent,
                SessionRejection::ReservationRejected,
            );
            decision.correlated_rejection = Some(CraftOpenRejected {
                request_id: request.request_id.expect("validated request id"),
                reason: "admission rejected".to_string(),
            });
            decision
        }
        AdmissionOutcome::ReservedClaimWon => {
            let mut decision = SessionDecision::accepted(SessionState::Running);
            decision.admitted_session = Some(SessionRecord::new(
                request.session_key,
                request.owner_key,
                request.durability,
                request.busy_claim.clone(),
            ));
            decision.claim_effect = ClaimEffect::Acquire(request.busy_claim);
            decision.audit_effect = AuditEffect::Record("session-admitted");
            decision
        }
        AdmissionOutcome::ReservedClaimLost => {
            let mut decision = SessionDecision::accepted(SessionState::Absent);
            decision.handoff = HandoffEffect::Retry;
            decision.claim_effect = ClaimEffect::Release(request.busy_claim);
            decision.audit_effect = AuditEffect::Record("busy-race-lost");
            decision
        }
    }
}

/// Contract-first craft intent. The existing `craft` producer remains unwired until M-10.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CraftIntent {
    Open(AdmissionRequest),
    Start {
        identity: SessionIdentity,
        recipe_id: String,
        quantity: u32,
    },
    Pause {
        identity: SessionIdentity,
    },
    Resume {
        identity: SessionIdentity,
    },
    Cancel {
        identity: SessionIdentity,
    },
}

/// The authoritative phase transition discriminator consumed by the future A-06 adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CraftSessionTransition {
    Initial,
    Rollover { previous_session_key: SessionKey },
    Restore { restore_token: String },
}

/// Test-only/declared authoritative projection for the craft wire adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CraftSessionProjection {
    pub active: bool,
    pub session_key: SessionKey,
    pub owner_key: PlayerKey,
    pub generation: u64,
    pub phase_revision: u64,
    pub state: SessionState,
    pub open_request_id: Option<String>,
    pub session_transition: CraftSessionTransition,
}

/// Adapter seam translating craft intents to the generic reducer without production wiring.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CraftSessionAdapter {
    pub record: SessionRecord,
    pub open_request_id: Option<String>,
    pub session_transition: CraftSessionTransition,
}

impl CraftSessionAdapter {
    pub fn new(record: SessionRecord, open_request_id: String) -> Self {
        Self {
            record,
            open_request_id: Some(open_request_id),
            session_transition: CraftSessionTransition::Initial,
        }
    }

    pub fn apply_intent(
        &mut self,
        intent: CraftIntent,
        context: &mut SessionLifecycleCtx,
    ) -> SessionDecision {
        let event = match intent {
            CraftIntent::Open(_) => {
                return SessionDecision::rejected(self.record.state, SessionRejection::InvalidState)
            }
            CraftIntent::Start {
                identity,
                recipe_id,
                quantity,
            } => SessionEvent::CraftStart {
                identity,
                recipe_id,
                quantity,
            },
            CraftIntent::Pause { identity } => SessionEvent::Pause { identity },
            CraftIntent::Resume { identity } => SessionEvent::Resume { identity },
            CraftIntent::Cancel { identity } => SessionEvent::VoluntaryCancel { identity },
        };
        self.record.reduce(event, context)
    }

    pub fn project(&self) -> CraftSessionProjection {
        CraftSessionProjection {
            active: !matches!(
                self.record.state,
                SessionState::Absent | SessionState::Ended
            ),
            session_key: self.record.session_key.clone(),
            owner_key: self.record.owner_key.clone(),
            generation: self.record.generation,
            phase_revision: self.record.phase_revision,
            state: self.record.state,
            open_request_id: self.open_request_id.clone(),
            session_transition: self.session_transition.clone(),
        }
    }
}

/// Short aliases make the contract name visible to adapters without introducing a second type.
pub type CraftAdapter = CraftSessionAdapter;

impl InteractionSession for CraftSessionAdapter {
    fn session_key(&self) -> SessionKey {
        self.record.session_key.clone()
    }

    fn owner_key(&self) -> &PlayerKey {
        &self.record.owner_key
    }

    fn durability(&self) -> SessionDurability {
        self.record.durability
    }

    fn busy_claim(&self) -> BusyClaim {
        self.record.busy_claim.clone()
    }

    fn reduce(
        &mut self,
        event: SessionEvent,
        context: &mut SessionLifecycleCtx,
    ) -> SessionDecision {
        self.record.reduce(event, context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(record: &SessionRecord) -> SessionIdentity {
        record.identity()
    }

    fn guard(record: &SessionRecord) -> ReconnectGuard {
        ReconnectGuard {
            owner_key: record.owner_key.as_str().to_string(),
            session_key: record.session_key.as_str().to_string(),
            generation: record.generation,
            phase_revision: record.phase_revision,
            restore_token: "token-1".to_string(),
        }
    }

    #[test]
    fn constants_pin_the_r1_delivery_and_suspension_contract() {
        assert_eq!(SESSION_SUSPENSION_TTL_TICKS, 1_728_000);
        assert_eq!(SESSION_SUSPENSION_SCAN_CADENCE_TICKS, 1_200);
        assert_eq!(SESSION_DELIVERY_MAX_PAYLOAD_BYTES, 1_048_576);
        assert_eq!(MAX_ACTIVE_SESSION_DELIVERY_ROWS, 4_096);
        assert_eq!(MAX_ACTIVE_SESSION_DELIVERY_BYTES, 4_294_967_296);
        assert_eq!(SESSION_DELIVERY_MAX_ATTEMPTS, 8);
        assert_eq!(SESSION_DELIVERY_MAX_RETRY_AGE_TICKS, 1_728_000);
    }

    #[test]
    fn busy_claims_conflict_only_within_their_namespace() {
        let player = BusyClaim::player("offline:alice");
        assert!(player.conflicts(&BusyClaim::player("offline:alice")));
        assert!(!player.conflicts(&BusyClaim::player("offline:bob")));
        assert!(!player.conflicts(&BusyClaim::facility("bench-1")));
    }

    #[test]
    fn running_pause_resume_and_matching_craft_start_are_admitted() {
        let mut record = SessionRecord::new(
            "craft-1",
            "offline:alice",
            SessionDurability::Checkpointed,
            BusyClaim::player("offline:alice"),
        );
        let initial_identity = identity(&record);
        let paused = reduce_session(
            &mut record,
            SessionEvent::Pause {
                identity: initial_identity,
            },
        );
        assert!(paused.accepted);
        assert_eq!(record.state, SessionState::Paused);
        let paused_identity = identity(&record);
        let resumed = reduce_session(
            &mut record,
            SessionEvent::Resume {
                identity: paused_identity,
            },
        );
        assert!(resumed.accepted);
        let running_identity = identity(&record);
        let started = reduce_session(
            &mut record,
            SessionEvent::CraftStart {
                identity: running_identity,
                recipe_id: "craft.example".to_string(),
                quantity: 1,
            },
        );
        assert!(started.accepted);
        assert_eq!(record.state, SessionState::Running);
    }

    #[test]
    fn stale_and_future_generation_are_rejected_without_mutation() {
        let mut record = SessionRecord::new(
            "craft-1",
            "offline:alice",
            SessionDurability::Volatile,
            BusyClaim::player("offline:alice"),
        );
        let initial_identity = identity(&record);
        let paused = reduce_session(
            &mut record,
            SessionEvent::Pause {
                identity: SessionIdentity {
                    generation: 0,
                    ..initial_identity
                },
            },
        );
        assert!(paused.accepted);
        record.generation = 1;
        let before_rejected_events = record.clone();
        let current_identity = identity(&record);
        let stale = reduce_session(
            &mut record,
            SessionEvent::Pause {
                identity: SessionIdentity {
                    generation: 0,
                    ..current_identity.clone()
                },
            },
        );
        assert_eq!(stale.rejection, Some(SessionRejection::StaleGeneration));
        let future = reduce_session(
            &mut record,
            SessionEvent::Pause {
                identity: SessionIdentity {
                    generation: 2,
                    ..current_identity
                },
            },
        );
        assert_eq!(future.rejection, Some(SessionRejection::FutureGeneration));
        assert_eq!(record, before_rejected_events);
    }

    #[test]
    fn mismatched_identity_is_rejected_without_claim_or_state_effect() {
        let mut record = SessionRecord::new(
            "craft-1",
            "offline:alice",
            SessionDurability::Checkpointed,
            BusyClaim::player("offline:alice"),
        );
        let before = record.clone();
        let decision = reduce_session(
            &mut record,
            SessionEvent::Pause {
                identity: SessionIdentity {
                    owner_key: PlayerKey::new("offline:bob"),
                    ..before.identity()
                },
            },
        );
        assert_eq!(decision.rejection, Some(SessionRejection::InvalidIdentity));
        assert_eq!(record, before);
        assert_eq!(decision.claim_effect, ClaimEffect::None);
    }

    #[test]
    fn checkpointed_disconnect_requires_durable_commit_and_restore_guard() {
        let mut record = SessionRecord::new(
            "craft-1",
            "offline:alice",
            SessionDurability::Checkpointed,
            BusyClaim::player("offline:alice"),
        );
        let identity = identity(&record);
        let failed = reduce_session(
            &mut record,
            SessionEvent::Disconnect {
                identity: identity.clone(),
                suspension: SuspensionResult::Failed,
            },
        );
        assert_eq!(
            failed.rejection,
            Some(SessionRejection::DurableCheckpointFailed)
        );
        assert_eq!(record.state, SessionState::Running);

        let committed_guard = guard(&record);
        let committed = reduce_session(
            &mut record,
            SessionEvent::Disconnect {
                identity,
                suspension: SuspensionResult::Committed(committed_guard.clone()),
            },
        );
        assert!(committed.accepted);
        assert_eq!(record.state, SessionState::Suspended);
        assert_eq!(
            committed.checkpoint_effect,
            CheckpointEffect::PersistReconnectGuard(committed_guard.clone())
        );
        let before_invalid_restore = record.clone();
        let mut wrong_token = committed_guard.clone();
        wrong_token.restore_token = "forged-token".to_string();
        let invalid_restore_identity = record.identity();
        let invalid_restore_revision = record.phase_revision;
        let rejected_restore = reduce_session(
            &mut record,
            SessionEvent::Restore {
                identity: invalid_restore_identity,
                guard: wrong_token,
                phase_revision: invalid_restore_revision + 1,
            },
        );
        assert_eq!(
            rejected_restore.rejection,
            Some(SessionRejection::InvalidRestore)
        );
        assert_eq!(record, before_invalid_restore);
        let restore_identity = record.identity();
        let restore_revision = record.phase_revision;
        let restored = reduce_session(
            &mut record,
            SessionEvent::Restore {
                identity: restore_identity,
                guard: committed_guard,
                phase_revision: restore_revision + 1,
            },
        );
        assert!(restored.accepted);
        assert_eq!(record.state, SessionState::Paused);
        assert!(record.restore_token.is_none());
    }

    #[test]
    fn terminal_handoff_commit_releases_claim_and_cannot_reopen() {
        let mut record = SessionRecord::new(
            "craft-1",
            "offline:alice",
            SessionDurability::Volatile,
            BusyClaim::player("offline:alice"),
        );
        let identity = identity(&record);
        let prepared = reduce_session(
            &mut record,
            SessionEvent::DimensionChange {
                identity: identity.clone(),
            },
        );
        assert_eq!(
            prepared.handoff,
            HandoffEffect::Prepare(TerminationCause::DimensionChange)
        );
        let committed = reduce_session(&mut record, SessionEvent::HandoffCommitted { identity });
        assert_eq!(record.state, SessionState::Ended);
        assert_eq!(
            committed.claim_effect,
            ClaimEffect::Release(BusyClaim::player("offline:alice"))
        );
        let ended_identity = record.identity();
        let late = reduce_session(
            &mut record,
            SessionEvent::Resume {
                identity: ended_identity,
            },
        );
        assert_eq!(late.rejection, Some(SessionRejection::InvalidState));
    }

    #[test]
    fn startup_epoch_rebases_guard_before_suspension_restore() {
        let mut record = SessionRecord::new(
            "craft-1",
            "offline:alice",
            SessionDurability::Checkpointed,
            BusyClaim::player("offline:alice"),
        );
        record.phase_revision = 3;
        let identity = record.identity();
        let old_guard = guard(&record);
        let decision = reduce_session(
            &mut record,
            SessionEvent::StartupEpochDetected {
                identity,
                guard: old_guard.clone(),
            },
        );
        assert!(decision.accepted);
        assert_eq!(record.state, SessionState::Suspended);
        assert_eq!(record.generation, 1);
        let CheckpointEffect::NormalizeSuspended(normalized_guard) = decision.checkpoint_effect
        else {
            panic!("startup normalization must persist a rebased reconnect guard");
        };
        assert_eq!(normalized_guard.generation, record.generation);
        assert_eq!(normalized_guard.phase_revision, record.phase_revision);
        assert_eq!(normalized_guard.restore_token, old_guard.restore_token);

        let restore_identity = record.identity();
        let restore_revision = record.phase_revision;
        let restored = reduce_session(
            &mut record,
            SessionEvent::Restore {
                identity: restore_identity,
                guard: normalized_guard,
                phase_revision: restore_revision + 1,
            },
        );
        assert!(restored.accepted);
        assert_eq!(record.state, SessionState::Paused);
    }

    #[test]
    fn startup_epoch_rejects_a_guard_from_another_generation_without_mutation() {
        let mut record = SessionRecord::new(
            "craft-1",
            "offline:alice",
            SessionDurability::Checkpointed,
            BusyClaim::player("offline:alice"),
        );
        let before = record.clone();
        let mut mismatched_guard = guard(&record);
        mismatched_guard.generation = 4;
        let decision = reduce_session(
            &mut record,
            SessionEvent::StartupEpochDetected {
                identity: before.identity(),
                guard: mismatched_guard,
            },
        );
        assert_eq!(decision.rejection, Some(SessionRejection::InvalidRestore));
        assert_eq!(record, before);
    }

    #[test]
    fn admission_correlates_only_valid_open_request_ids() {
        let request = AdmissionRequest {
            request_id: Some("open-1".to_string()),
            session_key: SessionKey::new("craft-1"),
            owner_key: PlayerKey::new("offline:alice"),
            durability: SessionDurability::Checkpointed,
            busy_claim: BusyClaim::player("offline:alice"),
        };
        let rejected = admit_session(request.clone(), AdmissionOutcome::Rejected);
        assert_eq!(
            rejected
                .correlated_rejection
                .as_ref()
                .map(|r| r.request_id.as_str()),
            Some("open-1")
        );
        let missing = admit_session(
            AdmissionRequest {
                request_id: None,
                ..request.clone()
            },
            AdmissionOutcome::Rejected,
        );
        assert_eq!(missing.rejection, Some(SessionRejection::MissingRequestId));
        assert!(missing.correlated_rejection.is_none());

        let admitted = admit_session(request, AdmissionOutcome::ReservedClaimWon);
        assert!(admitted.accepted);
        assert_eq!(admitted.next_state, SessionState::Running);
        assert_eq!(
            admitted
                .admitted_session
                .as_ref()
                .map(|session| session.session_key.as_str()),
            Some("craft-1")
        );

        let invalid = admit_session(
            AdmissionRequest {
                request_id: Some("open-2".to_string()),
                session_key: SessionKey::new(" "),
                owner_key: PlayerKey::new("offline:bob"),
                durability: SessionDurability::Checkpointed,
                busy_claim: BusyClaim::player("offline:bob"),
            },
            AdmissionOutcome::ReservedClaimWon,
        );
        assert_eq!(invalid.rejection, Some(SessionRejection::InvalidIdentity));
        assert_eq!(
            invalid
                .correlated_rejection
                .as_ref()
                .map(|rejection| rejection.request_id.as_str()),
            Some("open-2")
        );
        assert!(invalid.admitted_session.is_none());
    }

    #[test]
    fn craft_adapter_projection_carries_initial_request_and_phase_revision() {
        let adapter = CraftSessionAdapter::new(
            SessionRecord::new(
                "craft-1",
                "offline:alice",
                SessionDurability::Checkpointed,
                BusyClaim::player("offline:alice"),
            ),
            "open-1".to_string(),
        );
        let projection = adapter.project();
        assert!(projection.active);
        assert_eq!(projection.open_request_id.as_deref(), Some("open-1"));
        assert_eq!(
            projection.session_transition,
            CraftSessionTransition::Initial
        );
        assert_eq!(projection.phase_revision, 0);
    }
}
