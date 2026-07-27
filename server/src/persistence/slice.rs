//! Cross-domain persistence contracts.
//!
//! P0 intentionally keeps this module free of production slice registrations. The
//! descriptors and state machines below pin the invariants that later migrations
//! must preserve without changing the existing SQLite ownership model.

use std::fmt;

use valence::prelude::{Resource, World};

use crate::time::MILLIS_PER_TICK;

/// Stable identifier used in logs, reports, and registry ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SliceId(&'static str);

impl SliceId {
    pub const fn new(value: &'static str) -> Self {
        Self(value)
    }

    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for SliceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

/// Runtime owner of a persisted slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliceScope {
    PlayerEntity,
    WorldResource,
}

/// Action taken when a loader cannot establish the persisted row's state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadFailurePolicy {
    BlockWrites,
    RefuseStartup,
}

/// Meaning of time fields persisted by a slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeBasis {
    None,
    RemainingLogicalTicks,
    WallDeadline,
    ObservedAgeWithElapsed,
}

/// How a slice is eligible for runtime autosave.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutosavePolicy {
    Disabled,
    OnChange,
    EveryTicks(u64),
}

/// Ordering guarantee chosen by the owner of a write domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteOrdering {
    /// A single owner serializes all writes for this domain.
    Serialized,
    /// The durable writer rejects revisions older than the persisted revision.
    PersistedRevisionCas,
}

/// Stable name for fields that must share one write authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WriteDomain(&'static str);

impl WriteDomain {
    pub const fn new(value: &'static str) -> Self {
        Self(value)
    }

    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

/// Stable identity of the only writer allowed to commit one write domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WriteAuthority(&'static str);

impl WriteAuthority {
    pub const fn new(value: &'static str) -> Self {
        Self(value)
    }

    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

/// Domain and authority identity carried by load guards, dirty snapshots, and receipts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WriteBinding {
    domain: WriteDomain,
    authority: WriteAuthority,
}

impl WriteBinding {
    pub const fn new(domain: WriteDomain, authority: WriteAuthority) -> Self {
        Self { domain, authority }
    }

    pub const fn domain(self) -> WriteDomain {
        self.domain
    }

    pub const fn authority(self) -> WriteAuthority {
        self.authority
    }
}

/// Lifecycle reason supplied to a slice hook.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliceRunReason {
    Hydrate,
    Rebase,
    Autosave,
    DisconnectSave,
    ReconnectLoad,
    Shutdown,
}

/// Runtime and wall time source injected into persistence dispatchers.
///
/// Framework tests use deterministic implementations; persistence contracts never
/// read process wall time directly.
pub trait SliceClock {
    fn runtime_tick(&self) -> u64;
    fn wall_unix_millis(&self) -> u64;
}

/// Value-only context shared with exclusive-world slice adapters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SliceRunContext {
    pub reason: SliceRunReason,
    pub runtime_tick: u64,
    pub wall_unix_millis: u64,
    /// Stable persisted subject (for example a player identity) during handoff.
    pub handoff_key: Option<String>,
}

/// Successful observable result of a slice hook.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliceRunOutcome {
    Clean,
    Flushed,
    SkippedBlocked,
}

/// Error returned by one slice without aborting the remaining shutdown flushes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SliceRunError {
    message: String,
}

impl SliceRunError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for SliceRunError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for SliceRunError {}

pub type SliceRunResult = Result<SliceRunOutcome, SliceRunError>;
pub type SliceHook = fn(&mut World, &SliceRunContext) -> SliceRunResult;

/// Static lifecycle declaration for one persistence slice.
#[derive(Debug)]
pub struct SliceDescriptor {
    pub id: SliceId,
    pub scope: SliceScope,
    pub order: u16,
    pub load_failure: LoadFailurePolicy,
    pub time_basis: TimeBasis,
    pub write_binding: WriteBinding,
    pub write_ordering: WriteOrdering,
    pub autosave: AutosavePolicy,
    pub hydrate: Option<SliceHook>,
    pub rebase: Option<SliceHook>,
    pub disconnect_save: Option<SliceHook>,
    pub shutdown_flush: Option<SliceHook>,
}

/// Compile-time owner of a static slice descriptor.
pub trait PersistenceSlice {
    fn descriptor() -> &'static SliceDescriptor;
}

/// Registry construction errors are startup contract violations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SliceRegistryError {
    InvalidSliceId(SliceId),
    InvalidWriteDomain {
        slice_id: SliceId,
    },
    InvalidWriteAuthority {
        slice_id: SliceId,
    },
    DuplicateSliceId(SliceId),
    ConflictingWriteAuthority {
        domain: WriteDomain,
        first: WriteAuthority,
        conflicting: WriteAuthority,
    },
    ConflictingWriteOrdering {
        domain: WriteDomain,
        first: WriteOrdering,
        conflicting: WriteOrdering,
    },
    ZeroAutosaveCadence {
        slice_id: SliceId,
    },
    MissingRebaseHook {
        slice_id: SliceId,
    },
}

impl fmt::Display for SliceRegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSliceId(id) => write!(formatter, "invalid persistence slice id `{id}`"),
            Self::InvalidWriteDomain { slice_id } => {
                write!(formatter, "slice `{slice_id}` has an invalid write domain")
            }
            Self::InvalidWriteAuthority { slice_id } => {
                write!(
                    formatter,
                    "slice `{slice_id}` has an invalid write authority"
                )
            }
            Self::DuplicateSliceId(id) => {
                write!(formatter, "duplicate persistence slice id `{id}`")
            }
            Self::ConflictingWriteAuthority {
                domain,
                first,
                conflicting,
            } => write!(
                formatter,
                "write domain `{}` has conflicting authorities `{}` and `{}`",
                domain.as_str(),
                first.as_str(),
                conflicting.as_str()
            ),
            Self::ConflictingWriteOrdering {
                domain,
                first,
                conflicting,
            } => write!(
                formatter,
                "write domain `{}` has conflicting ordering {:?} and {:?}",
                domain.as_str(),
                first,
                conflicting
            ),
            Self::ZeroAutosaveCadence { slice_id } => {
                write!(formatter, "slice `{slice_id}` has a zero autosave cadence")
            }
            Self::MissingRebaseHook { slice_id } => {
                write!(
                    formatter,
                    "slice `{slice_id}` declares a time basis without a rebase hook"
                )
            }
        }
    }
}

impl std::error::Error for SliceRegistryError {}

/// Sorted registry of persistence lifecycle descriptors.
#[derive(Debug, Default)]
pub struct PersistenceSliceRegistry {
    descriptors: Vec<&'static SliceDescriptor>,
}

impl Resource for PersistenceSliceRegistry {}

impl PersistenceSliceRegistry {
    pub fn register_slice<S: PersistenceSlice>(&mut self) -> Result<(), SliceRegistryError> {
        self.register(S::descriptor())
    }

    pub fn register(
        &mut self,
        descriptor: &'static SliceDescriptor,
    ) -> Result<(), SliceRegistryError> {
        if !valid_stable_name(descriptor.id.as_str()) {
            return Err(SliceRegistryError::InvalidSliceId(descriptor.id));
        }
        if !valid_stable_name(descriptor.write_binding.domain.as_str()) {
            return Err(SliceRegistryError::InvalidWriteDomain {
                slice_id: descriptor.id,
            });
        }
        if !valid_stable_name(descriptor.write_binding.authority.as_str()) {
            return Err(SliceRegistryError::InvalidWriteAuthority {
                slice_id: descriptor.id,
            });
        }
        if self
            .descriptors
            .iter()
            .any(|registered| registered.id == descriptor.id)
        {
            return Err(SliceRegistryError::DuplicateSliceId(descriptor.id));
        }
        if let Some(registered) = self
            .descriptors
            .iter()
            .find(|registered| registered.write_binding.domain == descriptor.write_binding.domain)
        {
            if registered.write_binding.authority != descriptor.write_binding.authority {
                return Err(SliceRegistryError::ConflictingWriteAuthority {
                    domain: descriptor.write_binding.domain,
                    first: registered.write_binding.authority,
                    conflicting: descriptor.write_binding.authority,
                });
            }
            if registered.write_ordering != descriptor.write_ordering {
                return Err(SliceRegistryError::ConflictingWriteOrdering {
                    domain: descriptor.write_binding.domain,
                    first: registered.write_ordering,
                    conflicting: descriptor.write_ordering,
                });
            }
        }
        if matches!(descriptor.autosave, AutosavePolicy::EveryTicks(0)) {
            return Err(SliceRegistryError::ZeroAutosaveCadence {
                slice_id: descriptor.id,
            });
        }
        if descriptor.time_basis != TimeBasis::None && descriptor.rebase.is_none() {
            return Err(SliceRegistryError::MissingRebaseHook {
                slice_id: descriptor.id,
            });
        }

        self.descriptors.push(descriptor);
        self.descriptors
            .sort_by_key(|registered| (registered.order, registered.id));
        Ok(())
    }

    pub fn descriptors(&self) -> impl Iterator<Item = &'static SliceDescriptor> + '_ {
        self.descriptors.iter().copied()
    }
}

fn valid_stable_name(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
}

/// Whether the shutdown lifecycle requested a registry dispatch this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownFlushRequest {
    NotRequested,
    Requested,
}

/// One failed slice in an otherwise continuing shutdown dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShutdownFlushFailure {
    pub slice_id: SliceId,
    pub error: SliceRunError,
}

/// Aggregate result emitted once after all registered shutdown hooks are attempted.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ShutdownFlushReport {
    pub attempted: usize,
    pub clean: usize,
    pub flushed: usize,
    pub blocked: usize,
    pub failures: Vec<ShutdownFlushFailure>,
}

/// Runs shutdown hooks in registry order while isolating failures.
///
/// The descriptor list is copied before invoking hooks so the registry's shared
/// `World` borrow never overlaps a hook's exclusive `&mut World` borrow.
pub fn dispatch_shutdown_flushes(
    world: &mut World,
    request: ShutdownFlushRequest,
    clock: &impl SliceClock,
) -> ShutdownFlushReport {
    if request == ShutdownFlushRequest::NotRequested {
        return ShutdownFlushReport::default();
    }

    let descriptors: Vec<_> = world
        .get_resource::<PersistenceSliceRegistry>()
        .map(|registry| registry.descriptors().collect())
        .unwrap_or_default();
    let context = SliceRunContext {
        reason: SliceRunReason::Shutdown,
        runtime_tick: clock.runtime_tick(),
        wall_unix_millis: clock.wall_unix_millis(),
        handoff_key: None,
    };
    let mut report = ShutdownFlushReport::default();

    for descriptor in descriptors {
        let Some(flush) = descriptor.shutdown_flush else {
            continue;
        };
        report.attempted += 1;
        match flush(world, &context) {
            Ok(SliceRunOutcome::Clean) => report.clean += 1,
            Ok(SliceRunOutcome::Flushed) => report.flushed += 1,
            Ok(SliceRunOutcome::SkippedBlocked) => report.blocked += 1,
            Err(error) => report.failures.push(ShutdownFlushFailure {
                slice_id: descriptor.id,
                error,
            }),
        }
    }
    report
}

/// Failed hook in one disconnect/reconnect handoff phase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconnectHandoffFailure {
    pub slice_id: SliceId,
    pub error: SliceRunError,
}

/// Save-before-load report for one disconnect/reconnect handoff.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ReconnectHandoffReport {
    pub saves_attempted: usize,
    pub saves_completed: usize,
    pub loads_attempted: usize,
    pub loads_completed: usize,
    pub failures: Vec<ReconnectHandoffFailure>,
}

/// Enforces all disconnect saves before any same-tick reconnect hydrate.
///
/// Hooks run synchronously in registry order through exclusive `World` access.
/// Hydration is skipped entirely when any disconnect save fails, so a reconnect
/// cannot combine freshly loaded slices with stale durable rows from failed saves.
pub fn dispatch_reconnect_handoff(
    world: &mut World,
    handoff_key: impl Into<String>,
    clock: &impl SliceClock,
) -> ReconnectHandoffReport {
    let descriptors: Vec<_> = world
        .get_resource::<PersistenceSliceRegistry>()
        .map(|registry| {
            registry
                .descriptors()
                .filter(|descriptor| descriptor.scope == SliceScope::PlayerEntity)
                .collect()
        })
        .unwrap_or_default();
    let handoff_key = Some(handoff_key.into());
    let runtime_tick = clock.runtime_tick();
    let wall_unix_millis = clock.wall_unix_millis();
    let mut report = ReconnectHandoffReport::default();

    for descriptor in &descriptors {
        let Some(save) = descriptor.disconnect_save else {
            continue;
        };
        report.saves_attempted += 1;
        let context = SliceRunContext {
            reason: SliceRunReason::DisconnectSave,
            runtime_tick,
            wall_unix_millis,
            handoff_key: handoff_key.clone(),
        };
        match save(world, &context) {
            Ok(_) => report.saves_completed += 1,
            Err(error) => report.failures.push(ReconnectHandoffFailure {
                slice_id: descriptor.id,
                error,
            }),
        }
    }

    if !report.failures.is_empty() {
        return report;
    }

    for descriptor in descriptors {
        let Some(load) = descriptor.hydrate else {
            continue;
        };
        report.loads_attempted += 1;
        let context = SliceRunContext {
            reason: SliceRunReason::ReconnectLoad,
            runtime_tick,
            wall_unix_millis,
            handoff_key: handoff_key.clone(),
        };
        match load(world, &context) {
            Ok(_) => report.loads_completed += 1,
            Err(error) => report.failures.push(ReconnectHandoffFailure {
                slice_id: descriptor.id,
                error,
            }),
        }
    }

    report
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SliceLoad<T, E> {
    Missing,
    Loaded(T),
    Failed(E),
}

/// Durable provenance retained beside the runtime value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SliceLoadState<E> {
    Missing,
    Loaded,
    Failed(E),
}

/// Startup refusal emitted when a descriptor declares fail-closed hydration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SliceActivationError<E> {
    slice_id: SliceId,
    cause: E,
}

impl<E> SliceActivationError<E> {
    pub const fn slice_id(&self) -> SliceId {
        self.slice_id
    }

    pub fn cause(&self) -> &E {
        &self.cause
    }

    pub fn into_cause(self) -> E {
        self.cause
    }
}

/// Runtime value plus the write barrier implied by its load result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardedSlice<T, E> {
    value: T,
    load_state: SliceLoadState<E>,
    binding: WriteBinding,
}

impl<T, E> SliceLoad<T, E> {
    /// Activates a loaded value according to the descriptor's executable failure policy.
    ///
    /// A `RefuseStartup` descriptor never constructs a fallback runtime value. A
    /// `BlockWrites` descriptor may construct one, but retains failed provenance so
    /// no durable outlet can obtain a write permit.
    pub fn activate(
        self,
        descriptor: &SliceDescriptor,
        on_missing: impl FnOnce() -> T,
        on_failed: impl FnOnce(&E) -> T,
    ) -> Result<GuardedSlice<T, E>, SliceActivationError<E>> {
        match self {
            Self::Missing => Ok(GuardedSlice {
                value: on_missing(),
                load_state: SliceLoadState::Missing,
                binding: descriptor.write_binding,
            }),
            Self::Loaded(value) => Ok(GuardedSlice {
                value,
                load_state: SliceLoadState::Loaded,
                binding: descriptor.write_binding,
            }),
            Self::Failed(error) if descriptor.load_failure == LoadFailurePolicy::RefuseStartup => {
                Err(SliceActivationError {
                    slice_id: descriptor.id,
                    cause: error,
                })
            }
            Self::Failed(error) => {
                let value = on_failed(&error);
                Ok(GuardedSlice {
                    value,
                    load_state: SliceLoadState::Failed(error),
                    binding: descriptor.write_binding,
                })
            }
        }
    }
}

/// Every durable outlet must consume the same load guard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteOutlet {
    Changed,
    Autosave,
    Disconnect,
    Shutdown,
    Export,
    Transaction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SliceWriteBlocked {
    pub outlet: WriteOutlet,
}

impl fmt::Display for SliceWriteBlocked {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "persistence write through {:?} is blocked after slice load failure",
            self.outlet
        )
    }
}

impl std::error::Error for SliceWriteBlocked {}

/// Non-forgeable borrow that durable writer adapters must consume.
#[derive(Debug)]
pub struct SliceWritePermit<'a, T> {
    value: &'a T,
    binding: WriteBinding,
    outlet: WriteOutlet,
}

impl<T> SliceWritePermit<'_, T> {
    pub fn value(&self) -> &T {
        self.value
    }

    pub const fn binding(&self) -> WriteBinding {
        self.binding
    }

    pub const fn outlet(&self) -> WriteOutlet {
        self.outlet
    }
}

impl<T, E> GuardedSlice<T, E> {
    pub fn value(&self) -> &T {
        &self.value
    }

    pub fn value_mut(&mut self) -> &mut T {
        &mut self.value
    }

    pub fn load_state(&self) -> &SliceLoadState<E> {
        &self.load_state
    }

    pub const fn binding(&self) -> WriteBinding {
        self.binding
    }

    pub fn write_permit(
        &self,
        outlet: WriteOutlet,
    ) -> Result<SliceWritePermit<'_, T>, SliceWriteBlocked> {
        if matches!(self.load_state, SliceLoadState::Failed(_)) {
            return Err(SliceWriteBlocked { outlet });
        }
        Ok(SliceWritePermit {
            value: &self.value,
            binding: self.binding,
            outlet,
        })
    }
}

/// Monotonic revision attached to one write domain.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DirtyRevision(u64);

impl DirtyRevision {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Snapshot minted only by the tracker bound to the guarded slice's writer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirtySnapshot {
    binding: WriteBinding,
    revision: DirtyRevision,
}

impl DirtySnapshot {
    pub const fn binding(self) -> WriteBinding {
        self.binding
    }

    pub const fn revision(self) -> DirtyRevision {
        self.revision
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RevisionExhausted;

impl fmt::Display for RevisionExhausted {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("persistence dirty revision exhausted u64")
    }
}

impl std::error::Error for RevisionExhausted {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WriteBindingMismatch {
    expected: WriteBinding,
    actual: WriteBinding,
}

impl WriteBindingMismatch {
    pub const fn expected(self) -> WriteBinding {
        self.expected
    }

    pub const fn actual(self) -> WriteBinding {
        self.actual
    }
}

impl fmt::Display for WriteBindingMismatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "write binding mismatch: expected domain `{}` authority `{}`, got domain `{}` authority `{}`",
            self.expected.domain.as_str(),
            self.expected.authority.as_str(),
            self.actual.domain.as_str(),
            self.actual.authority.as_str()
        )
    }
}

impl std::error::Error for WriteBindingMismatch {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirtyAcknowledgement {
    Acknowledged,
    Stale,
    WrongBinding(WriteBindingMismatch),
}

/// In-memory dirty acknowledgement state for one write domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirtyTracker {
    binding: WriteBinding,
    current: DirtyRevision,
    acknowledged: DirtyRevision,
}

impl DirtyTracker {
    /// Restores the in-memory tracker at the durable revision observed during hydrate.
    pub const fn clean_at(binding: WriteBinding, revision: DirtyRevision) -> Self {
        Self {
            binding,
            current: revision,
            acknowledged: revision,
        }
    }

    pub const fn binding(self) -> WriteBinding {
        self.binding
    }

    pub fn mark_dirty(&mut self) -> Result<DirtyRevision, RevisionExhausted> {
        let next = self.current.0.checked_add(1).ok_or(RevisionExhausted)?;
        self.current = DirtyRevision(next);
        Ok(self.current)
    }

    pub fn is_dirty(self) -> bool {
        self.current != self.acknowledged
    }

    pub fn current_revision(self) -> DirtyRevision {
        self.current
    }

    pub fn begin_snapshot<T>(
        self,
        permit: &SliceWritePermit<'_, T>,
    ) -> Result<Option<DirtySnapshot>, WriteBindingMismatch> {
        if permit.binding != self.binding {
            return Err(WriteBindingMismatch {
                expected: self.binding,
                actual: permit.binding,
            });
        }
        Ok(self.is_dirty().then_some(DirtySnapshot {
            binding: self.binding,
            revision: self.current,
        }))
    }

    /// Clears dirty state only with a receipt minted after a successful durable write.
    pub fn acknowledge(&mut self, receipt: DurableWriteReceipt) -> DirtyAcknowledgement {
        if receipt.binding != self.binding {
            return DirtyAcknowledgement::WrongBinding(WriteBindingMismatch {
                expected: self.binding,
                actual: receipt.binding,
            });
        }
        if receipt.revision != self.current {
            return DirtyAcknowledgement::Stale;
        }
        self.acknowledged = receipt.revision;
        DirtyAcknowledgement::Acknowledged
    }
}

/// Request passed to the only durable writer adapter for a domain.
pub struct DurableWriteRequest<'a, T> {
    value: &'a T,
    binding: WriteBinding,
    expected_persisted_revision: DirtyRevision,
    write_revision: DirtyRevision,
    outlet: WriteOutlet,
    ordering: WriteOrdering,
}

impl<T> DurableWriteRequest<'_, T> {
    pub fn value(&self) -> &T {
        self.value
    }

    pub const fn binding(&self) -> WriteBinding {
        self.binding
    }

    pub const fn expected_persisted_revision(&self) -> DirtyRevision {
        self.expected_persisted_revision
    }

    pub const fn write_revision(&self) -> DirtyRevision {
        self.write_revision
    }

    pub const fn outlet(&self) -> WriteOutlet {
        self.outlet
    }

    pub const fn ordering(&self) -> WriteOrdering {
        self.ordering
    }
}

/// Receipt cannot be directly constructed outside this module.
#[derive(Debug, PartialEq, Eq)]
pub struct DurableWriteReceipt {
    binding: WriteBinding,
    revision: DirtyRevision,
}

impl DurableWriteReceipt {
    pub const fn binding(&self) -> WriteBinding {
        self.binding
    }

    pub const fn revision(&self) -> DirtyRevision {
        self.revision
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum DurableCommitError<E> {
    WrongBinding(WriteBindingMismatch),
    StaleRevision {
        persisted: DirtyRevision,
        attempted: DirtyRevision,
    },
    WriteFailed(E),
}

/// Durable revision fence and receipt minter for one registered write authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PersistedRevisionFence {
    binding: WriteBinding,
    ordering: WriteOrdering,
    persisted: DirtyRevision,
}

impl PersistedRevisionFence {
    /// Restores the durable fence from the revision stored alongside the slice.
    pub const fn new(
        binding: WriteBinding,
        ordering: WriteOrdering,
        persisted: DirtyRevision,
    ) -> Self {
        Self {
            binding,
            ordering,
            persisted,
        }
    }

    pub const fn binding(self) -> WriteBinding {
        self.binding
    }

    pub fn persisted_revision(self) -> DirtyRevision {
        self.persisted
    }

    /// Executes the writer adapter and mints a receipt only after it reports success.
    ///
    /// CAS adapters receive both the expected durable revision and the new revision
    /// in `DurableWriteRequest`; serialized adapters receive the same values for
    /// auditability. A failed or stale write never advances the fence and never
    /// yields a receipt that could clear dirty state.
    /// This stays crate-private so external consumers cannot turn an arbitrary
    /// `Ok(())` callback into a durable receipt. Production adapters live inside
    /// the persistence crate boundary and remain responsible for reporting storage
    /// success only after the serialized write or revision CAS commits.
    #[allow(dead_code)]
    pub(crate) fn commit<T, E>(
        &mut self,
        permit: SliceWritePermit<'_, T>,
        snapshot: DirtySnapshot,
        write: impl FnOnce(DurableWriteRequest<'_, T>) -> Result<(), E>,
    ) -> Result<DurableWriteReceipt, DurableCommitError<E>> {
        if permit.binding != self.binding {
            return Err(DurableCommitError::WrongBinding(WriteBindingMismatch {
                expected: self.binding,
                actual: permit.binding,
            }));
        }
        if snapshot.binding != self.binding {
            return Err(DurableCommitError::WrongBinding(WriteBindingMismatch {
                expected: self.binding,
                actual: snapshot.binding,
            }));
        }
        if snapshot.revision <= self.persisted {
            return Err(DurableCommitError::StaleRevision {
                persisted: self.persisted,
                attempted: snapshot.revision,
            });
        }

        write(DurableWriteRequest {
            value: permit.value,
            binding: self.binding,
            expected_persisted_revision: self.persisted,
            write_revision: snapshot.revision,
            outlet: permit.outlet,
            ordering: self.ordering,
        })
        .map_err(DurableCommitError::WriteFailed)?;

        self.persisted = snapshot.revision;
        Ok(DurableWriteReceipt {
            binding: self.binding,
            revision: snapshot.revision,
        })
    }
}

/// Whether a deadline advances while the server is offline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfflineTimePolicy {
    Pause,
    Continue,
}

/// Persisted representation of a runtime deadline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemainingDeadline {
    pub remaining_ticks: u64,
    pub saved_at_wall_millis: u64,
    pub offline_policy: OfflineTimePolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TickRebaseError {
    DeadlineOverflow,
}

impl fmt::Display for TickRebaseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeadlineOverflow => formatter.write_str("rebased runtime deadline exceeds u64"),
        }
    }
}

impl std::error::Error for TickRebaseError {}

/// Rebuilds a process-local deadline from a persisted remaining duration.
pub fn rebase_remaining_deadline(
    snapshot: RemainingDeadline,
    current_runtime_tick: u64,
    restored_at_wall_millis: u64,
) -> Result<u64, TickRebaseError> {
    let remaining_ticks = match snapshot.offline_policy {
        OfflineTimePolicy::Pause => snapshot.remaining_ticks,
        OfflineTimePolicy::Continue => {
            let saved_at_wall_millis = snapshot.saved_at_wall_millis;
            let elapsed_millis = restored_at_wall_millis.saturating_sub(saved_at_wall_millis);
            let elapsed_ticks = elapsed_millis / MILLIS_PER_TICK;
            snapshot.remaining_ticks.saturating_sub(elapsed_ticks)
        }
    };

    current_runtime_tick
        .checked_add(remaining_ticks)
        .ok_or(TickRebaseError::DeadlineOverflow)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_BINDING: WriteBinding = WriteBinding::new(
        WriteDomain::new("test.player"),
        WriteAuthority::new("test.player.writer"),
    );

    #[derive(Debug, Clone, Copy)]
    struct FixedClock {
        runtime_tick: u64,
        wall_unix_millis: u64,
    }

    impl SliceClock for FixedClock {
        fn runtime_tick(&self) -> u64 {
            self.runtime_tick
        }

        fn wall_unix_millis(&self) -> u64 {
            self.wall_unix_millis
        }
    }

    const NO_HOOK_DESCRIPTOR: SliceDescriptor = SliceDescriptor {
        id: SliceId::new("test.no_hook"),
        scope: SliceScope::WorldResource,
        order: 20,
        load_failure: LoadFailurePolicy::BlockWrites,
        time_basis: TimeBasis::None,
        write_binding: WriteBinding::new(
            WriteDomain::new("test.no_hook"),
            WriteAuthority::new("test.no_hook.writer"),
        ),
        write_ordering: WriteOrdering::Serialized,
        autosave: AutosavePolicy::Disabled,
        hydrate: None,
        rebase: None,
        disconnect_save: None,
        shutdown_flush: None,
    };

    fn noop_rebase(_world: &mut World, _context: &SliceRunContext) -> SliceRunResult {
        Ok(SliceRunOutcome::Clean)
    }

    fn basic_descriptor(id: &'static str, order: u16) -> SliceDescriptor {
        SliceDescriptor {
            id: SliceId::new(id),
            scope: SliceScope::PlayerEntity,
            order,
            load_failure: LoadFailurePolicy::BlockWrites,
            time_basis: TimeBasis::None,
            write_binding: TEST_BINDING,
            write_ordering: WriteOrdering::Serialized,
            autosave: AutosavePolicy::OnChange,
            hydrate: None,
            rebase: None,
            disconnect_save: None,
            shutdown_flush: None,
        }
    }

    #[test]
    fn registry_rejects_duplicate_and_invalid_descriptors() {
        let first = Box::leak(Box::new(basic_descriptor("player.core", 10)));
        let duplicate = Box::leak(Box::new(basic_descriptor("player.core", 20)));
        let invalid = Box::leak(Box::new(basic_descriptor("Player Core", 30)));
        let zero_cadence = Box::leak(Box::new(SliceDescriptor {
            autosave: AutosavePolicy::EveryTicks(0),
            ..basic_descriptor("player.zero_cadence", 40)
        }));
        let missing_rebase = Box::leak(Box::new(SliceDescriptor {
            time_basis: TimeBasis::RemainingLogicalTicks,
            ..basic_descriptor("player.missing_rebase", 50)
        }));
        let invalid_domain = Box::leak(Box::new(SliceDescriptor {
            write_binding: WriteBinding::new(
                WriteDomain::new("Player Core"),
                WriteAuthority::new("test.player.writer"),
            ),
            ..basic_descriptor("player.invalid_domain", 55)
        }));
        let valid_rebase = Box::leak(Box::new(SliceDescriptor {
            time_basis: TimeBasis::RemainingLogicalTicks,
            rebase: Some(noop_rebase),
            ..basic_descriptor("player.valid_rebase", 60)
        }));
        let mut registry = PersistenceSliceRegistry::default();

        assert_eq!(registry.register(first), Ok(()));
        assert_eq!(
            registry.register(duplicate),
            Err(SliceRegistryError::DuplicateSliceId(SliceId::new(
                "player.core"
            )))
        );
        assert!(matches!(
            registry.register(invalid),
            Err(SliceRegistryError::InvalidSliceId(_))
        ));
        assert!(matches!(
            registry.register(zero_cadence),
            Err(SliceRegistryError::ZeroAutosaveCadence { .. })
        ));
        assert!(matches!(
            registry.register(missing_rebase),
            Err(SliceRegistryError::MissingRebaseHook { .. })
        ));
        assert!(matches!(
            registry.register(invalid_domain),
            Err(SliceRegistryError::InvalidWriteDomain { .. })
        ));
        assert_eq!(registry.register(valid_rebase), Ok(()));
    }

    #[test]
    fn registry_rejects_conflicting_authority_or_ordering_for_one_domain() {
        let first = Box::leak(Box::new(basic_descriptor("player.core", 10)));
        let wrong_authority = Box::leak(Box::new(SliceDescriptor {
            write_binding: WriteBinding::new(
                TEST_BINDING.domain(),
                WriteAuthority::new("test.player.competing_writer"),
            ),
            ..basic_descriptor("player.inventory", 20)
        }));
        let wrong_ordering = Box::leak(Box::new(SliceDescriptor {
            write_ordering: WriteOrdering::PersistedRevisionCas,
            ..basic_descriptor("player.craft", 30)
        }));
        let mut registry = PersistenceSliceRegistry::default();
        registry.register(first).unwrap();

        assert!(matches!(
            registry.register(wrong_authority),
            Err(SliceRegistryError::ConflictingWriteAuthority { .. })
        ));
        assert!(matches!(
            registry.register(wrong_ordering),
            Err(SliceRegistryError::ConflictingWriteOrdering { .. })
        ));
    }

    #[test]
    fn registry_orders_by_order_then_slice_id() {
        let later = Box::leak(Box::new(basic_descriptor("world.later", 20)));
        let same_order_b = Box::leak(Box::new(basic_descriptor("world.b", 10)));
        let same_order_a = Box::leak(Box::new(basic_descriptor("world.a", 10)));
        let mut registry = PersistenceSliceRegistry::default();
        registry.register(later).unwrap();
        registry.register(same_order_b).unwrap();
        registry.register(same_order_a).unwrap();

        assert_eq!(
            registry
                .descriptors()
                .map(|descriptor| descriptor.id.as_str())
                .collect::<Vec<_>>(),
            vec!["world.a", "world.b", "world.later"]
        );
    }

    #[derive(Debug, Default)]
    struct FlushTrace(Vec<&'static str>);

    impl Resource for FlushTrace {}

    fn clean_hook(world: &mut World, context: &SliceRunContext) -> SliceRunResult {
        assert_eq!(context.reason, SliceRunReason::Shutdown);
        world.resource_mut::<FlushTrace>().0.push("clean");
        Ok(SliceRunOutcome::Clean)
    }

    fn failed_hook(world: &mut World, _context: &SliceRunContext) -> SliceRunResult {
        world.resource_mut::<FlushTrace>().0.push("failed");
        Err(SliceRunError::new("disk unavailable"))
    }

    fn flushed_hook(world: &mut World, _context: &SliceRunContext) -> SliceRunResult {
        world.resource_mut::<FlushTrace>().0.push("flushed");
        Ok(SliceRunOutcome::Flushed)
    }

    fn blocked_hook(world: &mut World, _context: &SliceRunContext) -> SliceRunResult {
        world.resource_mut::<FlushTrace>().0.push("blocked");
        Ok(SliceRunOutcome::SkippedBlocked)
    }

    #[test]
    fn shutdown_dispatch_is_ordered_and_failure_isolated() {
        let descriptors = [
            Box::leak(Box::new(SliceDescriptor {
                id: SliceId::new("shutdown.flushed"),
                order: 30,
                shutdown_flush: Some(flushed_hook),
                ..basic_descriptor("shutdown.flushed", 30)
            })),
            Box::leak(Box::new(SliceDescriptor {
                id: SliceId::new("shutdown.failed"),
                order: 20,
                shutdown_flush: Some(failed_hook),
                ..basic_descriptor("shutdown.failed", 20)
            })),
            Box::leak(Box::new(SliceDescriptor {
                id: SliceId::new("shutdown.clean"),
                order: 10,
                shutdown_flush: Some(clean_hook),
                ..basic_descriptor("shutdown.clean", 10)
            })),
            Box::leak(Box::new(SliceDescriptor {
                id: SliceId::new("shutdown.blocked"),
                order: 40,
                shutdown_flush: Some(blocked_hook),
                ..basic_descriptor("shutdown.blocked", 40)
            })),
        ];
        let mut registry = PersistenceSliceRegistry::default();
        for descriptor in descriptors {
            registry.register(descriptor).unwrap();
        }
        registry.register(&NO_HOOK_DESCRIPTOR).unwrap();
        let mut world = World::new();
        world.insert_resource(registry);
        world.insert_resource(FlushTrace::default());

        let clock = FixedClock {
            runtime_tick: 77,
            wall_unix_millis: 1_000,
        };
        let report = dispatch_shutdown_flushes(&mut world, ShutdownFlushRequest::Requested, &clock);

        assert_eq!(
            world.resource::<FlushTrace>().0,
            vec!["clean", "failed", "flushed", "blocked"]
        );
        assert_eq!(report.attempted, 4);
        assert_eq!(report.clean, 1);
        assert_eq!(report.flushed, 1);
        assert_eq!(report.blocked, 1);
        assert_eq!(report.failures.len(), 1);
        assert_eq!(report.failures[0].slice_id.as_str(), "shutdown.failed");
    }

    #[test]
    fn absent_shutdown_request_does_not_invoke_hooks() {
        let descriptor = Box::leak(Box::new(SliceDescriptor {
            shutdown_flush: Some(flushed_hook),
            ..basic_descriptor("shutdown.not_requested", 10)
        }));
        let mut registry = PersistenceSliceRegistry::default();
        registry.register(descriptor).unwrap();
        let mut world = World::new();
        world.insert_resource(registry);
        world.insert_resource(FlushTrace::default());

        let clock = FixedClock {
            runtime_tick: 0,
            wall_unix_millis: 0,
        };
        let report =
            dispatch_shutdown_flushes(&mut world, ShutdownFlushRequest::NotRequested, &clock);

        assert_eq!(report, ShutdownFlushReport::default());
        assert!(world.resource::<FlushTrace>().0.is_empty());
    }

    #[derive(Debug, Default)]
    struct HandoffTrace {
        events: Vec<(SliceRunReason, u64, u64, String)>,
        fail_save: bool,
    }

    impl Resource for HandoffTrace {}

    fn handoff_save(world: &mut World, context: &SliceRunContext) -> SliceRunResult {
        let mut trace = world.resource_mut::<HandoffTrace>();
        trace.events.push((
            context.reason,
            context.runtime_tick,
            context.wall_unix_millis,
            context.handoff_key.clone().unwrap(),
        ));
        if trace.fail_save {
            Err(SliceRunError::new("disconnect save failed"))
        } else {
            Ok(SliceRunOutcome::Flushed)
        }
    }

    fn handoff_load(world: &mut World, context: &SliceRunContext) -> SliceRunResult {
        world.resource_mut::<HandoffTrace>().events.push((
            context.reason,
            context.runtime_tick,
            context.wall_unix_millis,
            context.handoff_key.clone().unwrap(),
        ));
        Ok(SliceRunOutcome::Clean)
    }

    #[test]
    fn reconnect_handoff_enforces_same_tick_all_saves_before_any_load() {
        let first = Box::leak(Box::new(SliceDescriptor {
            hydrate: Some(handoff_load),
            disconnect_save: Some(handoff_save),
            ..basic_descriptor("player.handoff_first", 10)
        }));
        let second = Box::leak(Box::new(SliceDescriptor {
            hydrate: Some(handoff_load),
            disconnect_save: Some(handoff_save),
            ..basic_descriptor("player.handoff_second", 20)
        }));
        let mut registry = PersistenceSliceRegistry::default();
        registry.register(second).unwrap();
        registry.register(first).unwrap();
        let clock = FixedClock {
            runtime_tick: 400,
            wall_unix_millis: 49_999,
        };
        let mut world = World::new();
        world.insert_resource(registry);
        world.insert_resource(HandoffTrace::default());

        assert_eq!(
            dispatch_reconnect_handoff(&mut world, "offline:test", &clock),
            ReconnectHandoffReport {
                saves_attempted: 2,
                saves_completed: 2,
                loads_attempted: 2,
                loads_completed: 2,
                failures: Vec::new(),
            }
        );
        assert_eq!(
            world
                .resource::<HandoffTrace>()
                .events
                .iter()
                .map(|event| event.0)
                .collect::<Vec<_>>(),
            vec![
                SliceRunReason::DisconnectSave,
                SliceRunReason::DisconnectSave,
                SliceRunReason::ReconnectLoad,
                SliceRunReason::ReconnectLoad,
            ]
        );
        assert!(world
            .resource::<HandoffTrace>()
            .events
            .iter()
            .all(|event| { event.1 == 400 && event.2 == 49_999 && event.3 == "offline:test" }));

        world.resource_mut::<HandoffTrace>().events.clear();
        world.resource_mut::<HandoffTrace>().fail_save = true;
        let report = dispatch_reconnect_handoff(&mut world, "offline:test", &clock);
        assert_eq!(report.saves_attempted, 2);
        assert_eq!(report.saves_completed, 0);
        assert_eq!(report.loads_attempted, 0);
        assert_eq!(report.loads_completed, 0);
        assert_eq!(report.failures.len(), 2);
        assert_eq!(world.resource::<HandoffTrace>().events.len(), 2);
        assert!(world
            .resource::<HandoffTrace>()
            .events
            .iter()
            .all(|event| event.0 == SliceRunReason::DisconnectSave));
    }

    #[test]
    fn load_failure_default_remains_blocked_for_every_write_outlet() {
        let descriptor = basic_descriptor("player.failed", 10);
        let guarded = SliceLoad::<u32, _>::Failed("invalid json")
            .activate(&descriptor, || 1, |_error| 0)
            .unwrap();
        assert_eq!(*guarded.value(), 0);
        assert_eq!(
            guarded.load_state(),
            &SliceLoadState::Failed("invalid json")
        );

        for outlet in [
            WriteOutlet::Changed,
            WriteOutlet::Autosave,
            WriteOutlet::Disconnect,
            WriteOutlet::Shutdown,
            WriteOutlet::Export,
            WriteOutlet::Transaction,
        ] {
            assert_eq!(
                guarded.write_permit(outlet).unwrap_err(),
                SliceWriteBlocked { outlet }
            );
        }
    }

    #[test]
    fn refuse_startup_never_constructs_a_failed_load_fallback() {
        let descriptor = SliceDescriptor {
            load_failure: LoadFailurePolicy::RefuseStartup,
            ..basic_descriptor("world.ledger", 10)
        };
        let mut fallback_called = false;
        let result = SliceLoad::<u32, _>::Failed("corrupt ledger").activate(
            &descriptor,
            || 1,
            |_error| {
                fallback_called = true;
                0
            },
        );

        let refusal = result.unwrap_err();
        assert_eq!(refusal.slice_id(), SliceId::new("world.ledger"));
        assert_eq!(refusal.cause(), &"corrupt ledger");
        assert!(!fallback_called);
    }

    #[test]
    fn missing_and_loaded_slices_are_writable() {
        let descriptor = basic_descriptor("player.writable", 10);
        let missing = SliceLoad::<u32, &str>::Missing
            .activate(&descriptor, || 7, |_| 0)
            .unwrap();
        let loaded = SliceLoad::<u32, &str>::Loaded(9)
            .activate(&descriptor, || 0, |_| 0)
            .unwrap();

        assert_eq!(*missing.value(), 7);
        assert_eq!(missing.load_state(), &SliceLoadState::Missing);
        assert!(missing.write_permit(WriteOutlet::Autosave).is_ok());
        assert_eq!(*loaded.value(), 9);
        assert_eq!(loaded.load_state(), &SliceLoadState::Loaded);
        assert!(loaded.write_permit(WriteOutlet::Shutdown).is_ok());
    }

    #[test]
    fn failed_durable_write_and_stale_receipt_never_clear_dirty_state() {
        let descriptor = basic_descriptor("player.dirty", 10);
        let guarded = SliceLoad::<u32, &str>::Loaded(9)
            .activate(&descriptor, || 0, |_| 0)
            .unwrap();
        let mut tracker = DirtyTracker::clean_at(TEST_BINDING, DirtyRevision::default());
        let mut fence = PersistedRevisionFence::new(
            TEST_BINDING,
            WriteOrdering::Serialized,
            DirtyRevision::default(),
        );

        tracker.mark_dirty().unwrap();
        let first_permit = guarded.write_permit(WriteOutlet::Autosave).unwrap();
        let first = tracker.begin_snapshot(&first_permit).unwrap().unwrap();
        tracker.mark_dirty().unwrap();
        let failed_permit = guarded.write_permit(WriteOutlet::Autosave).unwrap();
        let failed = tracker.begin_snapshot(&failed_permit).unwrap().unwrap();
        let result = fence.commit(failed_permit, failed, |_request| Err("disk unavailable"));
        assert_eq!(
            result,
            Err(DurableCommitError::WriteFailed("disk unavailable"))
        );
        assert!(tracker.is_dirty());

        let stale_permit = guarded.write_permit(WriteOutlet::Autosave).unwrap();
        let stale_receipt = fence
            .commit(stale_permit, first, |_request| Ok::<_, &str>(()))
            .unwrap();
        assert_eq!(
            tracker.acknowledge(stale_receipt),
            DirtyAcknowledgement::Stale
        );
        assert!(tracker.is_dirty());

        let latest_permit = guarded.write_permit(WriteOutlet::Autosave).unwrap();
        let latest = tracker.begin_snapshot(&latest_permit).unwrap().unwrap();
        let receipt = fence
            .commit(latest_permit, latest, |_request| Ok::<_, &str>(()))
            .unwrap();
        assert_eq!(
            tracker.acknowledge(receipt),
            DirtyAcknowledgement::Acknowledged
        );
        assert!(!tracker.is_dirty());
    }

    #[test]
    fn durable_write_is_bound_to_domain_authority_and_monotonic_revision() {
        const OTHER_BINDING: WriteBinding = WriteBinding::new(
            WriteDomain::new("test.other"),
            WriteAuthority::new("test.other.writer"),
        );
        let descriptor = basic_descriptor("player.bound", 10);
        let guarded = SliceLoad::<u32, &str>::Loaded(9)
            .activate(&descriptor, || 0, |_| 0)
            .unwrap();
        let permit = guarded.write_permit(WriteOutlet::Shutdown).unwrap();
        let mut wrong_tracker = DirtyTracker::clean_at(OTHER_BINDING, DirtyRevision::default());
        wrong_tracker.mark_dirty().unwrap();
        assert_eq!(
            wrong_tracker.begin_snapshot(&permit).unwrap_err(),
            WriteBindingMismatch {
                expected: OTHER_BINDING,
                actual: TEST_BINDING,
            }
        );

        let mut tracker = DirtyTracker::clean_at(TEST_BINDING, DirtyRevision::new(41));
        let mut fence = PersistedRevisionFence::new(
            TEST_BINDING,
            WriteOrdering::PersistedRevisionCas,
            DirtyRevision::new(41),
        );
        tracker.mark_dirty().unwrap();
        let permit = guarded.write_permit(WriteOutlet::Shutdown).unwrap();
        let snapshot = tracker.begin_snapshot(&permit).unwrap().unwrap();
        let receipt = fence
            .commit(permit, snapshot, |request| {
                assert_eq!(request.binding(), TEST_BINDING);
                assert_eq!(
                    request.expected_persisted_revision(),
                    DirtyRevision::new(41)
                );
                assert_eq!(request.write_revision(), DirtyRevision::new(42));
                assert_eq!(request.ordering(), WriteOrdering::PersistedRevisionCas);
                Ok::<_, &str>(())
            })
            .unwrap();
        assert_eq!(fence.persisted_revision(), DirtyRevision::new(42));
        assert_eq!(receipt.revision(), DirtyRevision::new(42));
        assert_eq!(
            tracker.acknowledge(receipt),
            DirtyAcknowledgement::Acknowledged
        );
    }

    #[test]
    fn dirty_revision_overflow_fails_without_wrapping_clean_state() {
        let mut tracker = DirtyTracker {
            binding: TEST_BINDING,
            current: DirtyRevision::new(u64::MAX),
            acknowledged: DirtyRevision::new(u64::MAX - 1),
        };

        assert_eq!(tracker.mark_dirty(), Err(RevisionExhausted));
        assert!(tracker.is_dirty());
        assert_eq!(tracker.current_revision(), DirtyRevision::new(u64::MAX));
    }

    #[test]
    fn deadline_rebase_pins_pause_continue_and_boundaries() {
        let paused = RemainingDeadline {
            remaining_ticks: 200,
            saved_at_wall_millis: 1_000_000,
            offline_policy: OfflineTimePolicy::Pause,
        };
        let advancing = RemainingDeadline {
            offline_policy: OfflineTimePolicy::Continue,
            ..paused
        };

        assert_eq!(
            rebase_remaining_deadline(paused, 10, 1_005_000),
            Ok(210),
            "online-only deadlines preserve all remaining ticks"
        );
        assert_eq!(
            rebase_remaining_deadline(advancing, 10, 1_005_000),
            Ok(110),
            "five offline seconds consume exactly 100 ticks at 50ms/tick"
        );
        assert_eq!(
            rebase_remaining_deadline(advancing, 10, 999_000),
            Ok(210),
            "a wall clock moving backwards must not create negative elapsed time"
        );
        assert_eq!(
            rebase_remaining_deadline(advancing, 10, 2_000_000),
            Ok(10),
            "offline elapsed beyond the remaining duration clamps the deadline to now"
        );
        assert_eq!(
            rebase_remaining_deadline(
                RemainingDeadline {
                    remaining_ticks: 1,
                    ..paused
                },
                u64::MAX,
                1_000_000,
            ),
            Err(TickRebaseError::DeadlineOverflow)
        );
    }
}
