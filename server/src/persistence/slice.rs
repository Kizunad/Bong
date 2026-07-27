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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WriteDomain(&'static str);

impl WriteDomain {
    pub const fn new(value: &'static str) -> Self {
        Self(value)
    }

    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

/// Lifecycle reason supplied to a slice hook.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliceRunReason {
    Hydrate,
    Rebase,
    Autosave,
    Shutdown,
}

/// Value-only context shared with exclusive-world slice adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SliceRunContext {
    pub reason: SliceRunReason,
    pub runtime_tick: u64,
    pub wall_unix_millis: u64,
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
    pub write_domain: WriteDomain,
    pub write_ordering: WriteOrdering,
    pub autosave: AutosavePolicy,
    pub hydrate: Option<SliceHook>,
    pub rebase: Option<SliceHook>,
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
    InvalidWriteDomain { slice_id: SliceId },
    DuplicateSliceId(SliceId),
    ZeroAutosaveCadence { slice_id: SliceId },
    MissingRebaseHook { slice_id: SliceId },
}

impl fmt::Display for SliceRegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSliceId(id) => write!(formatter, "invalid persistence slice id `{id}`"),
            Self::InvalidWriteDomain { slice_id } => {
                write!(formatter, "slice `{slice_id}` has an invalid write domain")
            }
            Self::DuplicateSliceId(id) => {
                write!(formatter, "duplicate persistence slice id `{id}`")
            }
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
        if !valid_stable_name(descriptor.write_domain.as_str()) {
            return Err(SliceRegistryError::InvalidWriteDomain {
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
    runtime_tick: u64,
    wall_unix_millis: u64,
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
        runtime_tick,
        wall_unix_millis,
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

/// Result of reading a durable slice. `Missing` is never an error fallback.
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

/// Runtime value plus the write barrier implied by its load result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardedSlice<T, E> {
    value: T,
    load_state: SliceLoadState<E>,
}

impl<T, E> SliceLoad<T, E> {
    pub fn into_guarded(
        self,
        on_missing: impl FnOnce() -> T,
        on_failed: impl FnOnce(&E) -> T,
    ) -> GuardedSlice<T, E> {
        match self {
            Self::Missing => GuardedSlice {
                value: on_missing(),
                load_state: SliceLoadState::Missing,
            },
            Self::Loaded(value) => GuardedSlice {
                value,
                load_state: SliceLoadState::Loaded,
            },
            Self::Failed(error) => {
                let value = on_failed(&error);
                GuardedSlice {
                    value,
                    load_state: SliceLoadState::Failed(error),
                }
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

    pub fn write_guard(&self, outlet: WriteOutlet) -> Result<(), SliceWriteBlocked> {
        if matches!(self.load_state, SliceLoadState::Failed(_)) {
            Err(SliceWriteBlocked { outlet })
        } else {
            Ok(())
        }
    }

    pub fn into_parts(self) -> (T, SliceLoadState<E>) {
        (self.value, self.load_state)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirtySnapshot {
    revision: DirtyRevision,
}

impl DirtySnapshot {
    pub const fn new(revision: DirtyRevision) -> Self {
        Self { revision }
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
pub enum PersistenceAttempt {
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirtyAcknowledgement {
    Acknowledged,
    Stale,
    Failed,
}

/// In-memory dirty acknowledgement state for one write domain.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DirtyTracker {
    current: DirtyRevision,
    acknowledged: DirtyRevision,
}

impl DirtyTracker {
    /// Restores the in-memory tracker at the durable revision observed during hydrate.
    pub const fn clean_at(revision: DirtyRevision) -> Self {
        Self {
            current: revision,
            acknowledged: revision,
        }
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

    pub fn begin_snapshot(self) -> Option<DirtySnapshot> {
        self.is_dirty().then_some(DirtySnapshot {
            revision: self.current,
        })
    }

    pub fn acknowledge(
        &mut self,
        snapshot: DirtySnapshot,
        attempt: PersistenceAttempt,
    ) -> DirtyAcknowledgement {
        if attempt == PersistenceAttempt::Failed {
            return DirtyAcknowledgement::Failed;
        }
        if snapshot.revision != self.current {
            return DirtyAcknowledgement::Stale;
        }
        self.acknowledged = snapshot.revision;
        DirtyAcknowledgement::Acknowledged
    }
}

/// Durable revision fence used by CAS-style write domains.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PersistedRevisionFence {
    persisted: DirtyRevision,
}

impl PersistedRevisionFence {
    /// Restores the durable CAS fence from the revision stored alongside the slice.
    pub const fn new(persisted: DirtyRevision) -> Self {
        Self { persisted }
    }

    pub fn persisted_revision(self) -> DirtyRevision {
        self.persisted
    }

    pub fn accepts(self, snapshot: DirtySnapshot) -> bool {
        snapshot.revision > self.persisted
    }

    pub fn record(&mut self, snapshot: DirtySnapshot) -> bool {
        if snapshot.revision <= self.persisted {
            return false;
        }
        self.persisted = snapshot.revision;
        true
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

    const NO_HOOK_DESCRIPTOR: SliceDescriptor = SliceDescriptor {
        id: SliceId::new("test.no_hook"),
        scope: SliceScope::WorldResource,
        order: 20,
        load_failure: LoadFailurePolicy::BlockWrites,
        time_basis: TimeBasis::None,
        write_domain: WriteDomain::new("test.no_hook"),
        write_ordering: WriteOrdering::Serialized,
        autosave: AutosavePolicy::Disabled,
        hydrate: None,
        rebase: None,
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
            write_domain: WriteDomain::new("test.player"),
            write_ordering: WriteOrdering::Serialized,
            autosave: AutosavePolicy::OnChange,
            hydrate: None,
            rebase: None,
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
            write_domain: WriteDomain::new("Player Core"),
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

        let report =
            dispatch_shutdown_flushes(&mut world, ShutdownFlushRequest::Requested, 77, 1_000);

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

        let report =
            dispatch_shutdown_flushes(&mut world, ShutdownFlushRequest::NotRequested, 0, 0);

        assert_eq!(report, ShutdownFlushReport::default());
        assert!(world.resource::<FlushTrace>().0.is_empty());
    }

    #[test]
    fn load_failure_default_remains_blocked_for_every_write_outlet() {
        let guarded = SliceLoad::<u32, _>::Failed("invalid json").into_guarded(|| 1, |_error| 0);
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
                guarded.write_guard(outlet),
                Err(SliceWriteBlocked { outlet })
            );
        }
    }

    #[test]
    fn missing_and_loaded_slices_are_writable() {
        let missing = SliceLoad::<u32, &str>::Missing.into_guarded(|| 7, |_| 0);
        let loaded = SliceLoad::<u32, &str>::Loaded(9).into_guarded(|| 0, |_| 0);

        assert_eq!(*missing.value(), 7);
        assert_eq!(missing.load_state(), &SliceLoadState::Missing);
        assert!(missing.write_guard(WriteOutlet::Autosave).is_ok());
        assert_eq!(*loaded.value(), 9);
        assert_eq!(loaded.load_state(), &SliceLoadState::Loaded);
        assert!(loaded.write_guard(WriteOutlet::Shutdown).is_ok());
    }

    #[test]
    fn stale_or_failed_acknowledgement_never_clears_new_dirty_state() {
        let mut tracker = DirtyTracker::default();
        tracker.mark_dirty().unwrap();
        let first = tracker.begin_snapshot().unwrap();
        tracker.mark_dirty().unwrap();

        assert_eq!(
            tracker.acknowledge(first, PersistenceAttempt::Succeeded),
            DirtyAcknowledgement::Stale
        );
        assert!(tracker.is_dirty());
        let latest = tracker.begin_snapshot().unwrap();
        assert_eq!(
            tracker.acknowledge(latest, PersistenceAttempt::Failed),
            DirtyAcknowledgement::Failed
        );
        assert!(tracker.is_dirty());
        assert_eq!(
            tracker.acknowledge(latest, PersistenceAttempt::Succeeded),
            DirtyAcknowledgement::Acknowledged
        );
        assert!(!tracker.is_dirty());
    }

    #[test]
    fn persisted_revision_fence_rejects_late_old_snapshot() {
        let mut tracker = DirtyTracker::default();
        tracker.mark_dirty().unwrap();
        let old = tracker.begin_snapshot().unwrap();
        tracker.mark_dirty().unwrap();
        let new = tracker.begin_snapshot().unwrap();
        let mut fence = PersistedRevisionFence::default();

        assert!(fence.record(new));
        assert!(!fence.record(new));
        assert!(!fence.record(old));
        assert_eq!(fence.persisted_revision(), new.revision());
    }

    #[test]
    fn hydrated_revision_rejects_pre_restart_snapshots_and_advances_monotonically() {
        let durable = DirtyRevision::new(41);
        let mut tracker = DirtyTracker::clean_at(durable);
        let mut fence = PersistedRevisionFence::new(durable);

        assert!(!tracker.is_dirty());
        assert!(!fence.accepts(DirtySnapshot::new(DirtyRevision::new(40))));
        assert!(!fence.accepts(DirtySnapshot::new(durable)));

        let next = tracker.mark_dirty().unwrap();
        let snapshot = tracker.begin_snapshot().unwrap();
        assert_eq!(next, DirtyRevision::new(42));
        assert!(fence.record(snapshot));
        assert_eq!(fence.persisted_revision(), DirtyRevision::new(42));
        assert_eq!(
            tracker.acknowledge(snapshot, PersistenceAttempt::Succeeded),
            DirtyAcknowledgement::Acknowledged
        );
        assert!(!tracker.is_dirty());
    }

    #[test]
    fn dirty_revision_overflow_fails_without_wrapping_clean_state() {
        let mut tracker = DirtyTracker {
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
