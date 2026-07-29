//! Cross-domain persistence contracts.
//!
//! P0 intentionally keeps this module free of production slice registrations. The
//! descriptors and state machines below pin the invariants that later migrations
//! must preserve without changing the existing SQLite ownership model.

use std::{
    collections::HashMap,
    fmt,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex, Weak,
    },
};

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
    ReconnectTeardown,
    ReconnectLoad,
    ReconnectAbort,
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
#[derive(Debug, Clone, Copy)]
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
    /// Drops this slice's old runtime activation after every save has succeeded.
    ///
    /// Required for every player slice that participates in reconnect save or
    /// hydrate. The hook must synchronously release that slice's
    /// `GuardedSlice`, `DirtyTracker`, and `PersistedRevisionFence` state. It must
    /// be idempotent because the dispatcher invokes it again with
    /// `ReconnectAbort` to roll back a partial hydrate or rebase failure.
    pub reconnect_teardown: Option<SliceHook>,
    pub rebase: Option<SliceHook>,
    pub disconnect_save: Option<SliceHook>,
    pub shutdown_flush: Option<SliceHook>,
}

/// Compile-time owner of a static slice descriptor.
///
/// Registry construction remains outside the public API. This compile-fail example
/// is attached to a public item so `cargo test --doc` actually collects it and pins
/// the trust boundary:
///
/// ```compile_fail
/// use bong_server::persistence::slice::PersistenceSliceRegistry;
///
/// let _shadow = PersistenceSliceRegistry::empty();
/// ```
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
    MissingHydrateHook {
        slice_id: SliceId,
    },
    MissingReconnectTeardownHook {
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
            Self::MissingHydrateHook { slice_id } => {
                write!(
                    formatter,
                    "slice `{slice_id}` declares a time basis without a hydrate hook"
                )
            }
            Self::MissingReconnectTeardownHook { slice_id } => {
                write!(
                    formatter,
                    "player slice `{slice_id}` declares reconnect hooks without a teardown hook"
                )
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
///
/// The type itself, its construction, and descriptor-token issuance are restricted to
/// the `crate::persistence` trust boundary. Code outside that boundary cannot create,
/// insert, or remove a second registry and use it to downgrade the application's
/// canonical write policy.
#[derive(Debug)]
pub(in crate::persistence) struct PersistenceSliceRegistry {
    descriptors: Vec<&'static SliceDescriptor>,
    active_subjects: Mutex<HashMap<(WriteDomain, PersistenceSubjectKey), Weak<()>>>,
}

impl Resource for PersistenceSliceRegistry {}

/// Descriptor proven to come from a registry lookup inside the persistence boundary.
///
/// Both this token and `SliceLoad::activate` are persistence-private. Gameplay code
/// can declare slice data, but only a persistence adapter can construct the canonical
/// registry, resolve its policy, and activate writable state.
pub(in crate::persistence) struct RegisteredSliceDescriptor<'registry> {
    descriptor: &'static SliceDescriptor,
    _registry: std::marker::PhantomData<&'registry PersistenceSliceRegistry>,
}

impl PersistenceSliceRegistry {
    pub(in crate::persistence) fn empty() -> Self {
        Self {
            descriptors: Vec::new(),
            active_subjects: Mutex::new(HashMap::new()),
        }
    }

    pub(in crate::persistence) fn register_slice<S: PersistenceSlice>(
        &mut self,
    ) -> Result<(), SliceRegistryError> {
        self.register(S::descriptor())
    }

    pub(in crate::persistence) fn register(
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
        if matches!(descriptor.autosave, AutosavePolicy::EveryTicks(0)) {
            return Err(SliceRegistryError::ZeroAutosaveCadence {
                slice_id: descriptor.id,
            });
        }
        if descriptor.time_basis != TimeBasis::None
            && (descriptor.hydrate.is_none() || descriptor.rebase.is_none())
        {
            if descriptor.hydrate.is_none() {
                return Err(SliceRegistryError::MissingHydrateHook {
                    slice_id: descriptor.id,
                });
            }
            return Err(SliceRegistryError::MissingRebaseHook {
                slice_id: descriptor.id,
            });
        }
        if descriptor.scope == SliceScope::PlayerEntity
            && (descriptor.disconnect_save.is_some() || descriptor.hydrate.is_some())
            && descriptor.reconnect_teardown.is_none()
        {
            return Err(SliceRegistryError::MissingReconnectTeardownHook {
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

        self.descriptors.push(descriptor);
        self.descriptors
            .sort_by_key(|registered| (registered.order, registered.id));
        Ok(())
    }

    pub fn descriptors(&self) -> impl Iterator<Item = &'static SliceDescriptor> + '_ {
        self.descriptors.iter().copied()
    }

    pub(in crate::persistence) fn active_subject_domain(
        &self,
        subject_key: &PersistenceSubjectKey,
        domain: WriteDomain,
    ) -> bool {
        let Ok(mut active_subjects) = self.active_subjects.lock() else {
            return true;
        };
        active_subjects.retain(|_, subject| subject.strong_count() > 0);
        active_subjects.contains_key(&(domain, subject_key.clone()))
    }

    pub(in crate::persistence) fn registered_descriptor(
        &self,
        slice_id: SliceId,
    ) -> Option<RegisteredSliceDescriptor<'_>> {
        self.descriptors
            .iter()
            .copied()
            .find(|descriptor| descriptor.id == slice_id)
            .map(|descriptor| RegisteredSliceDescriptor {
                descriptor,
                _registry: std::marker::PhantomData,
            })
    }

    pub(in crate::persistence) fn activate<T, E>(
        &self,
        load: SliceLoad<T, E>,
        slice_id: SliceId,
        subject_key: PersistenceSubjectKey,
        initial_revision: DirtyRevision,
        on_missing: impl FnOnce() -> T,
        on_failed: impl FnOnce(&E) -> T,
    ) -> Result<GuardedSlice<T, E>, SliceActivationError<E>> {
        let registered = self
            .registered_descriptor(slice_id)
            .expect("persistence adapter must activate a registered slice descriptor");
        let descriptor = registered.descriptor;
        if matches!(load, SliceLoad::Failed(_))
            && descriptor.load_failure == LoadFailurePolicy::RefuseStartup
        {
            return load.refuse_startup(descriptor.id);
        }

        let mut active_subjects = self.active_subjects.lock().map_err(|_| {
            SliceActivationError::PoisonedSubjectRegistry {
                slice_id: descriptor.id,
            }
        })?;
        let lease_key = (descriptor.write_binding.domain(), subject_key.clone());
        active_subjects.retain(|_, subject| subject.strong_count() > 0);
        if active_subjects.contains_key(&lease_key) {
            return Err(SliceActivationError::DuplicateSubject {
                slice_id: descriptor.id,
                domain: descriptor.write_binding.domain(),
            });
        }
        let subject = SliceSubject::new();
        active_subjects.insert(lease_key, Arc::downgrade(&subject.0));
        drop(active_subjects);

        Ok(load.activate(
            registered,
            subject_key,
            subject,
            initial_revision,
            on_missing,
            on_failed,
        ))
    }
}

fn valid_stable_name(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.first().is_some_and(|byte| byte.is_ascii_lowercase())
        && bytes.last().is_some_and(|byte| !is_name_separator(*byte))
        && bytes.iter().copied().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || is_name_separator(byte)
        })
        && !bytes
            .windows(2)
            .any(|pair| is_name_separator(pair[0]) && is_name_separator(pair[1]))
}

const fn is_name_separator(byte: u8) -> bool {
    matches!(byte, b'.' | b'_' | b'-')
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

/// Dispatch cannot proceed without the canonical persistence registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliceDispatchError {
    MissingCanonicalRegistry,
    DuplicateSubject {
        slice_id: SliceId,
        domain: WriteDomain,
    },
}

impl fmt::Display for SliceDispatchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCanonicalRegistry => {
                formatter.write_str("canonical persistence slice registry is not installed")
            }
            Self::DuplicateSubject { slice_id, domain } => write!(
                formatter,
                "slice `{slice_id}` cannot hydrate while subject domain `{}` remains active",
                domain.as_str()
            ),
        }
    }
}

impl std::error::Error for SliceDispatchError {}

/// Runs shutdown hooks in registry order while isolating failures.
///
/// The descriptor list is copied before invoking hooks so the registry's shared
/// `World` borrow never overlaps a hook's exclusive `&mut World` borrow.
pub fn dispatch_shutdown_flushes(
    world: &mut World,
    request: ShutdownFlushRequest,
    clock: &impl SliceClock,
) -> Result<ShutdownFlushReport, SliceDispatchError> {
    if request == ShutdownFlushRequest::NotRequested {
        return Ok(ShutdownFlushReport::default());
    }

    let descriptors: Vec<_> = world
        .get_resource::<PersistenceSliceRegistry>()
        .ok_or(SliceDispatchError::MissingCanonicalRegistry)?
        .descriptors()
        .collect();
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
    Ok(report)
}

/// One-shot authority to run a disconnect/reconnect handoff for one stable subject.
///
/// Only persistence adapters can mint this token. Dispatch consumes it, so the
/// save/teardown/hydrate/rebase lifecycle cannot run twice for one generation.
#[derive(Debug)]
pub(in crate::persistence) struct ReconnectHandoffToken {
    generation: u64,
    subject_key: PersistenceSubjectKey,
}

static NEXT_HANDOFF_GENERATION: AtomicU64 = AtomicU64::new(1);

#[allow(dead_code)]
pub(in crate::persistence) fn reconnect_handoff_token(
    handoff_key: impl Into<String>,
) -> ReconnectHandoffToken {
    ReconnectHandoffToken {
        generation: NEXT_HANDOFF_GENERATION.fetch_add(1, Ordering::Relaxed),
        subject_key: PersistenceSubjectKey::new(handoff_key),
    }
}

/// Failed hook in one disconnect/reconnect handoff phase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconnectHandoffFailure {
    pub slice_id: SliceId,
    pub reason: SliceRunReason,
    pub error: SliceRunError,
}

/// Save-before-load report for one disconnect/reconnect handoff.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ReconnectHandoffReport {
    pub generation: u64,
    pub saves_attempted: usize,
    pub saves_completed: usize,
    pub blocked_saves: Vec<SliceId>,
    pub loads_attempted: usize,
    pub loads_completed: usize,
    pub blocked_loads: Vec<SliceId>,
    pub rebases_attempted: usize,
    pub rebases_completed: usize,
    pub blocked_rebases: Vec<SliceId>,
    pub teardown_attempted: usize,
    pub teardowns_completed: usize,
    pub blocked_teardowns: Vec<SliceId>,
    pub aborts_attempted: usize,
    pub aborts_completed: usize,
    pub blocked_aborts: Vec<SliceId>,
    pub failures: Vec<ReconnectHandoffFailure>,
}

fn abort_reconnect_activations(
    world: &mut World,
    descriptors: &[&'static SliceDescriptor],
    runtime_tick: u64,
    wall_unix_millis: u64,
    handoff_key: &Option<String>,
    report: &mut ReconnectHandoffReport,
) {
    for descriptor in descriptors.iter().rev() {
        let Some(abort) = descriptor.reconnect_teardown else {
            continue;
        };
        report.aborts_attempted += 1;
        let context = SliceRunContext {
            reason: SliceRunReason::ReconnectAbort,
            runtime_tick,
            wall_unix_millis,
            handoff_key: handoff_key.clone(),
        };
        match abort(world, &context) {
            Ok(SliceRunOutcome::Clean | SliceRunOutcome::Flushed) => {
                report.aborts_completed += 1;
            }
            Ok(SliceRunOutcome::SkippedBlocked) => {
                report.blocked_aborts.push(descriptor.id);
            }
            Err(error) => report.failures.push(ReconnectHandoffFailure {
                slice_id: descriptor.id,
                reason: SliceRunReason::ReconnectAbort,
                error,
            }),
        }
    }
}

/// Enforces all disconnect saves before teardown, global lease preflight,
/// same-tick hydrate, and one rebase pass.
///
/// Hooks run synchronously in registry order through exclusive `World` access. Any
/// blocked or failed phase prevents all later phases. Every old activation lease
/// must be gone before the first hydrate. A failed hydrate or rebase invokes the
/// idempotent teardown hook in reverse order with `ReconnectAbort`, so no partial
/// activation survives and the stable subject can be retried. Consuming a one-shot
/// token makes the lifecycle exactly once for that handoff generation.
pub(in crate::persistence) fn dispatch_reconnect_handoff(
    world: &mut World,
    token: ReconnectHandoffToken,
    clock: &impl SliceClock,
) -> Result<ReconnectHandoffReport, SliceDispatchError> {
    let descriptors: Vec<_> = world
        .get_resource::<PersistenceSliceRegistry>()
        .ok_or(SliceDispatchError::MissingCanonicalRegistry)?
        .descriptors()
        .filter(|descriptor| descriptor.scope == SliceScope::PlayerEntity)
        .collect();
    let ReconnectHandoffToken {
        generation,
        subject_key,
    } = token;
    let handoff_key = Some(subject_key.0.clone());
    let runtime_tick = clock.runtime_tick();
    let wall_unix_millis = clock.wall_unix_millis();
    let mut report = ReconnectHandoffReport {
        generation,
        ..ReconnectHandoffReport::default()
    };

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
            Ok(SliceRunOutcome::Clean | SliceRunOutcome::Flushed) => {
                report.saves_completed += 1;
            }
            Ok(SliceRunOutcome::SkippedBlocked) => report.blocked_saves.push(descriptor.id),
            Err(error) => report.failures.push(ReconnectHandoffFailure {
                slice_id: descriptor.id,
                reason: SliceRunReason::DisconnectSave,
                error,
            }),
        }
    }

    if !report.blocked_saves.is_empty() || !report.failures.is_empty() {
        return Ok(report);
    }

    for descriptor in &descriptors {
        let Some(teardown) = descriptor.reconnect_teardown else {
            continue;
        };
        report.teardown_attempted += 1;
        let context = SliceRunContext {
            reason: SliceRunReason::ReconnectTeardown,
            runtime_tick,
            wall_unix_millis,
            handoff_key: handoff_key.clone(),
        };
        match teardown(world, &context) {
            Ok(SliceRunOutcome::Clean | SliceRunOutcome::Flushed) => {
                report.teardowns_completed += 1;
            }
            Ok(SliceRunOutcome::SkippedBlocked) => {
                report.blocked_teardowns.push(descriptor.id);
            }
            Err(error) => report.failures.push(ReconnectHandoffFailure {
                slice_id: descriptor.id,
                reason: SliceRunReason::ReconnectTeardown,
                error,
            }),
        }
    }

    if !report.blocked_teardowns.is_empty() || !report.failures.is_empty() {
        return Ok(report);
    }

    let active_subject = {
        let registry = world
            .get_resource::<PersistenceSliceRegistry>()
            .ok_or(SliceDispatchError::MissingCanonicalRegistry)?;
        descriptors.iter().find_map(|descriptor| {
            registry
                .active_subject_domain(&subject_key, descriptor.write_binding.domain())
                .then_some(SliceDispatchError::DuplicateSubject {
                    slice_id: descriptor.id,
                    domain: descriptor.write_binding.domain(),
                })
        })
    };
    if let Some(error) = active_subject {
        return Err(error);
    }

    let mut hydrated_descriptors = Vec::new();
    for descriptor in &descriptors {
        let Some(load) = descriptor.hydrate else {
            continue;
        };
        hydrated_descriptors.push(*descriptor);
        report.loads_attempted += 1;
        let context = SliceRunContext {
            reason: SliceRunReason::ReconnectLoad,
            runtime_tick,
            wall_unix_millis,
            handoff_key: handoff_key.clone(),
        };
        match load(world, &context) {
            Ok(SliceRunOutcome::Clean | SliceRunOutcome::Flushed) => {
                report.loads_completed += 1;
            }
            Ok(SliceRunOutcome::SkippedBlocked) => report.blocked_loads.push(descriptor.id),
            Err(error) => report.failures.push(ReconnectHandoffFailure {
                slice_id: descriptor.id,
                reason: SliceRunReason::ReconnectLoad,
                error,
            }),
        }
    }

    if !report.blocked_loads.is_empty() || !report.failures.is_empty() {
        abort_reconnect_activations(
            world,
            &hydrated_descriptors,
            runtime_tick,
            wall_unix_millis,
            &handoff_key,
            &mut report,
        );
        return Ok(report);
    }

    for descriptor in &descriptors {
        let Some(rebase) = descriptor.rebase else {
            continue;
        };
        report.rebases_attempted += 1;
        let context = SliceRunContext {
            reason: SliceRunReason::Rebase,
            runtime_tick,
            wall_unix_millis,
            handoff_key: handoff_key.clone(),
        };
        match rebase(world, &context) {
            Ok(SliceRunOutcome::Clean | SliceRunOutcome::Flushed) => {
                report.rebases_completed += 1;
            }
            Ok(SliceRunOutcome::SkippedBlocked) => report.blocked_rebases.push(descriptor.id),
            Err(error) => report.failures.push(ReconnectHandoffFailure {
                slice_id: descriptor.id,
                reason: SliceRunReason::Rebase,
                error,
            }),
        }
    }

    if !report.blocked_rebases.is_empty() || !report.failures.is_empty() {
        abort_reconnect_activations(
            world,
            &hydrated_descriptors,
            runtime_tick,
            wall_unix_millis,
            &handoff_key,
            &mut report,
        );
    }

    Ok(report)
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

/// Activation failures preserve either load provenance or canonical subject ownership.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SliceActivationError<E> {
    LoadFailed {
        slice_id: SliceId,
        cause: E,
    },
    DuplicateSubject {
        slice_id: SliceId,
        domain: WriteDomain,
    },
    PoisonedSubjectRegistry {
        slice_id: SliceId,
    },
}

impl<E> SliceActivationError<E> {
    pub const fn slice_id(&self) -> SliceId {
        match self {
            Self::LoadFailed { slice_id, .. }
            | Self::DuplicateSubject { slice_id, .. }
            | Self::PoisonedSubjectRegistry { slice_id } => *slice_id,
        }
    }

    pub fn cause(&self) -> Option<&E> {
        match self {
            Self::LoadFailed { cause, .. } => Some(cause),
            Self::DuplicateSubject { .. } | Self::PoisonedSubjectRegistry { .. } => None,
        }
    }

    pub fn into_cause(self) -> Option<E> {
        match self {
            Self::LoadFailed { cause, .. } => Some(cause),
            Self::DuplicateSubject { .. } | Self::PoisonedSubjectRegistry { .. } => None,
        }
    }
}

/// Stable durable identity inside one write domain.
///
/// Construction is persistence-private so adapters must derive it from canonical
/// player/world identity rather than a transient entity or activation instance.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(in crate::persistence) struct PersistenceSubjectKey(String);

impl PersistenceSubjectKey {
    pub(in crate::persistence) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

/// Opaque identity shared only by state derived from one active durable subject.
#[derive(Debug, Clone)]
struct SliceSubject(Arc<()>);

impl SliceSubject {
    fn new() -> Self {
        Self(Arc::new(()))
    }

    fn is_same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl PartialEq for SliceSubject {
    fn eq(&self, other: &Self) -> bool {
        self.is_same(other)
    }
}

impl Eq for SliceSubject {}

/// Runtime value plus the write barrier implied by its load result.
#[derive(Debug)]
pub struct GuardedSlice<T, E> {
    value: T,
    load_state: SliceLoadState<E>,
    binding: WriteBinding,
    write_ordering: WriteOrdering,
    subject_key: PersistenceSubjectKey,
    subject: SliceSubject,
    initial_revision: DirtyRevision,
    persistence_state_issued: bool,
}

impl<T, E> SliceLoad<T, E> {
    fn refuse_startup<R>(self, slice_id: SliceId) -> Result<R, SliceActivationError<E>> {
        match self {
            Self::Failed(cause) => Err(SliceActivationError::LoadFailed { slice_id, cause }),
            Self::Missing | Self::Loaded(_) => {
                unreachable!("refuse_startup is only called for a failed load")
            }
        }
    }

    /// Activates a loaded value according to canonical registry policy and subject lease.
    ///
    /// `PersistenceSliceRegistry::activate` rejects duplicate durable subjects before
    /// calling this helper. A `BlockWrites` fallback retains failed provenance so no
    /// durable outlet can obtain a snapshot.
    fn activate(
        self,
        registered: RegisteredSliceDescriptor<'_>,
        subject_key: PersistenceSubjectKey,
        subject: SliceSubject,
        initial_revision: DirtyRevision,
        on_missing: impl FnOnce() -> T,
        on_failed: impl FnOnce(&E) -> T,
    ) -> GuardedSlice<T, E> {
        let descriptor = registered.descriptor;
        let (value, load_state) = match self {
            Self::Missing => (on_missing(), SliceLoadState::Missing),
            Self::Loaded(value) => (value, SliceLoadState::Loaded),
            Self::Failed(error) => {
                let value = on_failed(&error);
                (value, SliceLoadState::Failed(error))
            }
        };
        GuardedSlice {
            value,
            load_state,
            binding: descriptor.write_binding,
            write_ordering: descriptor.write_ordering,
            subject_key,
            subject,
            initial_revision,
            persistence_state_issued: false,
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
    subject_key: PersistenceSubjectKey,
    subject: SliceSubject,
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

    pub fn load_state(&self) -> &SliceLoadState<E> {
        &self.load_state
    }

    pub const fn binding(&self) -> WriteBinding {
        self.binding
    }

    /// Restores the unique dirty tracker and durable revision fence for this subject.
    ///
    /// Both capabilities are issued together exactly once. Reissuing either one could
    /// manufacture a clean acknowledgement state after a failed write or fork dirty
    /// ownership between autosave and shutdown paths. The fence always inherits the
    /// descriptor's registered ordering rather than accepting a caller override.
    pub fn restore_persistence_state(
        &mut self,
    ) -> Result<(DirtyTracker, PersistedRevisionFence), PersistenceStateAlreadyIssued> {
        if self.persistence_state_issued {
            return Err(PersistenceStateAlreadyIssued);
        }
        self.persistence_state_issued = true;
        let revision = self.initial_revision;
        Ok((
            DirtyTracker {
                binding: self.binding,
                subject: self.subject.clone(),
                current: revision,
                acknowledged: revision,
            },
            PersistedRevisionFence {
                binding: self.binding,
                subject: self.subject.clone(),
                ordering: self.write_ordering,
                persisted: revision,
            },
        ))
    }

    /// Applies one mutation only after proving the tracker belongs to this subject.
    ///
    /// Revision overflow and wrong-subject errors are reported before the closure
    /// receives mutable access, so an untrackable mutation can never occur.
    pub fn mutate<R>(
        &mut self,
        tracker: &mut DirtyTracker,
        mutate: impl FnOnce(&mut T) -> R,
    ) -> Result<(DirtyRevision, R), GuardedSliceMutationError> {
        if matches!(self.load_state, SliceLoadState::Failed(_)) {
            return Err(GuardedSliceMutationError::LoadFailed);
        }
        tracker.ensure_subject(self.binding, &self.subject)?;
        let revision = tracker
            .mark_dirty()
            .map_err(|_| GuardedSliceMutationError::RevisionExhausted)?;
        let result = mutate(&mut self.value);
        Ok((revision, result))
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
            subject_key: self.subject_key.clone(),
            subject: self.subject.clone(),
            outlet,
        })
    }
}

/// Returned when code tries to fork persistence state for one guarded subject.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PersistenceStateAlreadyIssued;

impl fmt::Display for PersistenceStateAlreadyIssued {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("persistence state was already restored for this guarded subject")
    }
}

impl std::error::Error for PersistenceStateAlreadyIssued {}

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
///
/// The owned payload is captured atomically with the dirty revision. A later
/// mutation therefore cannot pair an old revision with a newer runtime value.
#[derive(Debug, PartialEq, Eq)]
pub struct DirtySnapshot<P> {
    payload: P,
    binding: WriteBinding,
    subject_key: PersistenceSubjectKey,
    subject: SliceSubject,
    revision: DirtyRevision,
    outlet: WriteOutlet,
}

impl<P> DirtySnapshot<P> {
    pub const fn binding(&self) -> WriteBinding {
        self.binding
    }

    pub const fn revision(&self) -> DirtyRevision {
        self.revision
    }

    pub fn payload(&self) -> &P {
        &self.payload
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
pub enum GuardedSliceMutationError {
    LoadFailed,
    WrongBinding(WriteBindingMismatch),
    WrongSubject,
    RevisionExhausted,
}

impl fmt::Display for GuardedSliceMutationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LoadFailed => formatter.write_str("slice mutation is blocked after load failure"),
            Self::WrongBinding(error) => error.fmt(formatter),
            Self::WrongSubject => formatter.write_str("dirty tracker belongs to another subject"),
            Self::RevisionExhausted => {
                formatter.write_str("persistence dirty revision exhausted u64")
            }
        }
    }
}

impl std::error::Error for GuardedSliceMutationError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotProvenanceError {
    WrongBinding(WriteBindingMismatch),
    WrongSubject,
}

impl fmt::Display for SnapshotProvenanceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongBinding(error) => error.fmt(formatter),
            Self::WrongSubject => formatter.write_str("write permit belongs to another subject"),
        }
    }
}

impl std::error::Error for SnapshotProvenanceError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirtyAcknowledgement {
    Acknowledged,
    Stale,
    WrongBinding(WriteBindingMismatch),
    WrongSubject,
}

/// In-memory dirty acknowledgement state for one write domain.
#[derive(Debug, PartialEq, Eq)]
pub struct DirtyTracker {
    binding: WriteBinding,
    subject: SliceSubject,
    current: DirtyRevision,
    acknowledged: DirtyRevision,
}

impl DirtyTracker {
    pub const fn binding(&self) -> WriteBinding {
        self.binding
    }

    fn ensure_subject(
        &self,
        binding: WriteBinding,
        subject: &SliceSubject,
    ) -> Result<(), GuardedSliceMutationError> {
        if binding != self.binding {
            return Err(GuardedSliceMutationError::WrongBinding(
                WriteBindingMismatch {
                    expected: self.binding,
                    actual: binding,
                },
            ));
        }
        if !self.subject.is_same(subject) {
            return Err(GuardedSliceMutationError::WrongSubject);
        }
        Ok(())
    }

    fn mark_dirty(&mut self) -> Result<DirtyRevision, RevisionExhausted> {
        let next = self.current.0.checked_add(1).ok_or(RevisionExhausted)?;
        self.current = DirtyRevision(next);
        Ok(self.current)
    }

    pub fn is_dirty(&self) -> bool {
        self.current != self.acknowledged
    }

    pub fn current_revision(&self) -> DirtyRevision {
        self.current
    }

    pub fn begin_snapshot<T, P>(
        &self,
        permit: SliceWritePermit<'_, T>,
        capture: impl FnOnce(&T) -> P,
    ) -> Result<Option<DirtySnapshot<P>>, SnapshotProvenanceError> {
        if permit.binding != self.binding {
            return Err(SnapshotProvenanceError::WrongBinding(
                WriteBindingMismatch {
                    expected: self.binding,
                    actual: permit.binding,
                },
            ));
        }
        if !permit.subject.is_same(&self.subject) {
            return Err(SnapshotProvenanceError::WrongSubject);
        }
        Ok(self.is_dirty().then(|| DirtySnapshot {
            payload: capture(permit.value),
            binding: self.binding,
            subject_key: permit.subject_key,
            subject: self.subject.clone(),
            revision: self.current,
            outlet: permit.outlet,
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
        if !receipt.subject.is_same(&self.subject) {
            return DirtyAcknowledgement::WrongSubject;
        }
        if receipt.revision != self.current {
            return DirtyAcknowledgement::Stale;
        }
        self.acknowledged = receipt.revision;
        DirtyAcknowledgement::Acknowledged
    }
}

/// Request passed to the only durable writer adapter for a domain.
pub struct DurableWriteRequest<'a, P> {
    payload: &'a P,
    subject_key: &'a PersistenceSubjectKey,
    binding: WriteBinding,
    expected_persisted_revision: DirtyRevision,
    write_revision: DirtyRevision,
    outlet: WriteOutlet,
    ordering: WriteOrdering,
}

impl<P> DurableWriteRequest<'_, P> {
    pub fn payload(&self) -> &P {
        self.payload
    }

    pub(in crate::persistence) fn subject_key(&self) -> &PersistenceSubjectKey {
        self.subject_key
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
    subject: SliceSubject,
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
    WrongSubject,
    StaleRevision {
        persisted: DirtyRevision,
        attempted: DirtyRevision,
    },
    WriteFailed(E),
}

/// Durable revision fence and receipt minter for one registered write authority.
#[derive(Debug, PartialEq, Eq)]
pub struct PersistedRevisionFence {
    binding: WriteBinding,
    subject: SliceSubject,
    ordering: WriteOrdering,
    persisted: DirtyRevision,
}

/// Capability issued only inside persistence after a durable transaction commits.
pub(in crate::persistence) mod durable_writer {
    #[derive(Debug)]
    pub struct Capability {
        _private: (),
    }

    #[allow(dead_code)]
    pub(in crate::persistence) const fn acquire() -> Capability {
        Capability { _private: () }
    }
}

impl PersistedRevisionFence {
    pub const fn binding(&self) -> WriteBinding {
        self.binding
    }

    pub fn persisted_revision(&self) -> DirtyRevision {
        self.persisted
    }

    /// Executes the writer adapter and mints a receipt only after it returns the
    /// persistence-private proof produced after its storage transaction or CAS.
    ///
    /// Subject checks happen before the adapter runs, so one player's permit or
    /// snapshot can never advance another player's durable fence.
    #[allow(dead_code)]
    pub(in crate::persistence) fn commit<P, E>(
        &mut self,
        snapshot: DirtySnapshot<P>,
        write: impl FnOnce(DurableWriteRequest<'_, P>) -> Result<durable_writer::Capability, E>,
    ) -> Result<DurableWriteReceipt, DurableCommitError<E>> {
        if snapshot.binding != self.binding {
            return Err(DurableCommitError::WrongBinding(WriteBindingMismatch {
                expected: self.binding,
                actual: snapshot.binding,
            }));
        }
        if !snapshot.subject.is_same(&self.subject) {
            return Err(DurableCommitError::WrongSubject);
        }
        if snapshot.revision <= self.persisted {
            return Err(DurableCommitError::StaleRevision {
                persisted: self.persisted,
                attempted: snapshot.revision,
            });
        }

        let _capability = write(DurableWriteRequest {
            payload: &snapshot.payload,
            subject_key: &snapshot.subject_key,
            binding: self.binding,
            expected_persisted_revision: self.persisted,
            write_revision: snapshot.revision,
            outlet: snapshot.outlet,
            ordering: self.ordering,
        })
        .map_err(DurableCommitError::WriteFailed)?;

        self.persisted = snapshot.revision;
        Ok(DurableWriteReceipt {
            binding: self.binding,
            subject: self.subject.clone(),
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
    use std::cell::Cell;

    use super::*;

    const TEST_BINDING: WriteBinding = WriteBinding::new(
        WriteDomain::new("test.player"),
        WriteAuthority::new("test.player.writer"),
    );
    const SECOND_TEST_BINDING: WriteBinding = WriteBinding::new(
        WriteDomain::new("test.player.second"),
        WriteAuthority::new("test.player.second.writer"),
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

    #[derive(Debug)]
    struct CountingClock {
        runtime_tick: u64,
        wall_unix_millis: u64,
        runtime_reads: Cell<usize>,
        wall_reads: Cell<usize>,
    }

    impl CountingClock {
        fn new(runtime_tick: u64, wall_unix_millis: u64) -> Self {
            Self {
                runtime_tick,
                wall_unix_millis,
                runtime_reads: Cell::new(0),
                wall_reads: Cell::new(0),
            }
        }

        fn reads(&self) -> (usize, usize) {
            (self.runtime_reads.get(), self.wall_reads.get())
        }
    }

    impl SliceClock for CountingClock {
        fn runtime_tick(&self) -> u64 {
            let reads = self.runtime_reads.get();
            self.runtime_reads.set(reads + 1);
            self.runtime_tick + reads as u64
        }

        fn wall_unix_millis(&self) -> u64 {
            let reads = self.wall_reads.get();
            self.wall_reads.set(reads + 1);
            self.wall_unix_millis + reads as u64
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
        reconnect_teardown: None,
        rebase: None,
        disconnect_save: None,
        shutdown_flush: None,
    };

    fn noop_rebase(_world: &mut World, _context: &SliceRunContext) -> SliceRunResult {
        Ok(SliceRunOutcome::Clean)
    }

    fn noop_teardown(_world: &mut World, _context: &SliceRunContext) -> SliceRunResult {
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
            reconnect_teardown: None,
            rebase: None,
            disconnect_save: None,
            shutdown_flush: None,
        }
    }

    fn subject_key(value: &str) -> PersistenceSubjectKey {
        PersistenceSubjectKey::new(value)
    }

    fn activate<T, E: fmt::Debug>(
        descriptor: &SliceDescriptor,
        load: SliceLoad<T, E>,
        subject: &str,
        revision: DirtyRevision,
        on_missing: impl FnOnce() -> T,
        on_failed: impl FnOnce(&E) -> T,
    ) -> (Box<PersistenceSliceRegistry>, GuardedSlice<T, E>) {
        let descriptor = Box::leak(Box::new(*descriptor));
        let mut registry = Box::new(PersistenceSliceRegistry::empty());
        registry.register(descriptor).unwrap();
        let guarded = registry
            .activate(
                load,
                descriptor.id,
                subject_key(subject),
                revision,
                on_missing,
                on_failed,
            )
            .unwrap();
        (registry, guarded)
    }

    #[test]
    fn registry_rejects_duplicate_and_invalid_descriptors() {
        let first = Box::leak(Box::new(basic_descriptor("player.core", 10)));
        let duplicate = Box::leak(Box::new(basic_descriptor("player.core", 20)));
        let invalid = Box::leak(Box::new(basic_descriptor("Player Core", 30)));
        let invalid_leading_separator = Box::leak(Box::new(basic_descriptor(".player.core", 31)));
        let invalid_leading_digit = Box::leak(Box::new(basic_descriptor("9player.core", 32)));
        let invalid_trailing_separator = Box::leak(Box::new(basic_descriptor("player.core-", 33)));
        let invalid_adjacent_separators = Box::leak(Box::new(basic_descriptor("player..core", 34)));
        let zero_cadence = Box::leak(Box::new(SliceDescriptor {
            autosave: AutosavePolicy::EveryTicks(0),
            ..basic_descriptor("player.zero_cadence", 40)
        }));
        let missing_hydrate = Box::leak(Box::new(SliceDescriptor {
            time_basis: TimeBasis::RemainingLogicalTicks,
            rebase: Some(noop_rebase),
            ..basic_descriptor("player.missing_hydrate", 45)
        }));
        let missing_rebase = Box::leak(Box::new(SliceDescriptor {
            time_basis: TimeBasis::RemainingLogicalTicks,
            hydrate: Some(noop_rebase),
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
            hydrate: Some(noop_rebase),
            reconnect_teardown: Some(noop_teardown),
            rebase: Some(noop_rebase),
            ..basic_descriptor("player.valid_rebase", 60)
        }));
        let world_missing_hydrate = Box::leak(Box::new(SliceDescriptor {
            scope: SliceScope::WorldResource,
            time_basis: TimeBasis::WallDeadline,
            rebase: Some(noop_rebase),
            ..basic_descriptor("world.missing_hydrate", 65)
        }));
        let missing_teardown = Box::leak(Box::new(SliceDescriptor {
            time_basis: TimeBasis::None,
            hydrate: Some(noop_rebase),
            disconnect_save: Some(noop_rebase),
            ..basic_descriptor("player.missing_teardown", 70)
        }));
        let mut registry = PersistenceSliceRegistry::empty();

        assert_eq!(registry.register(first), Ok(()));
        assert_eq!(
            registry.register(duplicate),
            Err(SliceRegistryError::DuplicateSliceId(SliceId::new(
                "player.core"
            )))
        );
        for invalid_name in [
            invalid,
            invalid_leading_separator,
            invalid_leading_digit,
            invalid_trailing_separator,
            invalid_adjacent_separators,
        ] {
            assert!(matches!(
                registry.register(invalid_name),
                Err(SliceRegistryError::InvalidSliceId(_))
            ));
        }
        assert!(matches!(
            registry.register(zero_cadence),
            Err(SliceRegistryError::ZeroAutosaveCadence { .. })
        ));
        assert!(matches!(
            registry.register(missing_hydrate),
            Err(SliceRegistryError::MissingHydrateHook { .. })
        ));
        assert!(matches!(
            registry.register(missing_rebase),
            Err(SliceRegistryError::MissingRebaseHook { .. })
        ));
        assert!(matches!(
            registry.register(invalid_domain),
            Err(SliceRegistryError::InvalidWriteDomain { .. })
        ));
        assert!(matches!(
            registry.register(world_missing_hydrate),
            Err(SliceRegistryError::MissingHydrateHook { .. })
        ));
        assert!(matches!(
            registry.register(missing_teardown),
            Err(SliceRegistryError::MissingReconnectTeardownHook { .. })
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
        let mut registry = PersistenceSliceRegistry::empty();
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
        let mut registry = PersistenceSliceRegistry::empty();
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
                shutdown_flush: Some(flushed_hook),
                ..basic_descriptor("shutdown.flushed", 30)
            })),
            Box::leak(Box::new(SliceDescriptor {
                shutdown_flush: Some(failed_hook),
                ..basic_descriptor("shutdown.failed", 20)
            })),
            Box::leak(Box::new(SliceDescriptor {
                shutdown_flush: Some(clean_hook),
                ..basic_descriptor("shutdown.clean", 10)
            })),
            Box::leak(Box::new(SliceDescriptor {
                shutdown_flush: Some(blocked_hook),
                ..basic_descriptor("shutdown.blocked", 40)
            })),
        ];
        let mut registry = PersistenceSliceRegistry::empty();
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
        let report =
            dispatch_shutdown_flushes(&mut world, ShutdownFlushRequest::Requested, &clock).unwrap();

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
        let mut registry = PersistenceSliceRegistry::empty();
        registry.register(descriptor).unwrap();
        let mut world = World::new();
        world.insert_resource(registry);
        world.insert_resource(FlushTrace::default());

        let clock = FixedClock {
            runtime_tick: 0,
            wall_unix_millis: 0,
        };
        let report =
            dispatch_shutdown_flushes(&mut world, ShutdownFlushRequest::NotRequested, &clock)
                .unwrap();

        assert_eq!(report, ShutdownFlushReport::default());
        assert!(world.resource::<FlushTrace>().0.is_empty());
    }

    #[test]
    fn missing_canonical_registry_fails_closed_but_explicit_empty_registry_is_valid() {
        let clock = FixedClock {
            runtime_tick: 0,
            wall_unix_millis: 0,
        };
        let mut missing = World::new();
        assert_eq!(
            dispatch_shutdown_flushes(&mut missing, ShutdownFlushRequest::Requested, &clock,),
            Err(SliceDispatchError::MissingCanonicalRegistry)
        );
        assert_eq!(
            dispatch_reconnect_handoff(
                &mut missing,
                reconnect_handoff_token("player:missing"),
                &clock,
            ),
            Err(SliceDispatchError::MissingCanonicalRegistry)
        );

        let mut explicit_empty = World::new();
        explicit_empty.insert_resource(PersistenceSliceRegistry::empty());
        assert_eq!(
            dispatch_shutdown_flushes(&mut explicit_empty, ShutdownFlushRequest::Requested, &clock,),
            Ok(ShutdownFlushReport::default())
        );
        let report = dispatch_reconnect_handoff(
            &mut explicit_empty,
            reconnect_handoff_token("player:empty"),
            &clock,
        )
        .unwrap();
        assert_eq!(report.saves_attempted, 0);
        assert_eq!(report.loads_attempted, 0);
        assert_eq!(report.rebases_attempted, 0);
    }

    #[derive(Debug, Default)]
    struct HandoffTrace {
        events: Vec<(SliceRunReason, u64, u64, String)>,
        fail_save: bool,
        block_save: bool,
        fail_load: bool,
        block_load: bool,
        fail_teardown: bool,
        block_teardown: bool,
        fail_abort: bool,
        block_abort: bool,
        fail_rebase: bool,
        block_rebase: bool,
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
        } else if trace.block_save {
            Ok(SliceRunOutcome::SkippedBlocked)
        } else {
            Ok(SliceRunOutcome::Flushed)
        }
    }

    fn handoff_load(world: &mut World, context: &SliceRunContext) -> SliceRunResult {
        let mut trace = world.resource_mut::<HandoffTrace>();
        trace.events.push((
            context.reason,
            context.runtime_tick,
            context.wall_unix_millis,
            context.handoff_key.clone().unwrap(),
        ));
        if trace.fail_load {
            Err(SliceRunError::new("reconnect hydrate failed"))
        } else if trace.block_load {
            Ok(SliceRunOutcome::SkippedBlocked)
        } else {
            Ok(SliceRunOutcome::Clean)
        }
    }

    fn handoff_teardown(world: &mut World, context: &SliceRunContext) -> SliceRunResult {
        let mut trace = world.resource_mut::<HandoffTrace>();
        trace.events.push((
            context.reason,
            context.runtime_tick,
            context.wall_unix_millis,
            context.handoff_key.clone().unwrap(),
        ));
        if context.reason == SliceRunReason::ReconnectAbort {
            if trace.fail_abort {
                Err(SliceRunError::new("reconnect abort failed"))
            } else if trace.block_abort {
                Ok(SliceRunOutcome::SkippedBlocked)
            } else {
                Ok(SliceRunOutcome::Clean)
            }
        } else if trace.fail_teardown {
            Err(SliceRunError::new("reconnect teardown failed"))
        } else if trace.block_teardown {
            Ok(SliceRunOutcome::SkippedBlocked)
        } else {
            Ok(SliceRunOutcome::Clean)
        }
    }

    fn handoff_rebase(world: &mut World, context: &SliceRunContext) -> SliceRunResult {
        let mut trace = world.resource_mut::<HandoffTrace>();
        trace.events.push((
            context.reason,
            context.runtime_tick,
            context.wall_unix_millis,
            context.handoff_key.clone().unwrap(),
        ));
        if trace.fail_rebase {
            Err(SliceRunError::new("rebase failed"))
        } else if trace.block_rebase {
            Ok(SliceRunOutcome::SkippedBlocked)
        } else {
            Ok(SliceRunOutcome::Clean)
        }
    }

    fn token(handoff_key: &str) -> ReconnectHandoffToken {
        reconnect_handoff_token(handoff_key)
    }

    #[test]
    fn dispatch_samples_each_injected_clock_anchor_once() {
        let shutdown_descriptor = Box::leak(Box::new(SliceDescriptor {
            shutdown_flush: Some(clean_hook),
            ..basic_descriptor("shutdown.clock_once", 10)
        }));
        let mut shutdown_registry = PersistenceSliceRegistry::empty();
        shutdown_registry.register(shutdown_descriptor).unwrap();
        let mut shutdown_world = World::new();
        shutdown_world.insert_resource(shutdown_registry);
        shutdown_world.insert_resource(FlushTrace::default());
        let shutdown_clock = CountingClock::new(70, 4_000);

        dispatch_shutdown_flushes(
            &mut shutdown_world,
            ShutdownFlushRequest::Requested,
            &shutdown_clock,
        )
        .unwrap();
        assert_eq!(
            shutdown_clock.reads(),
            (1, 1),
            "one shutdown dispatch must reuse a single injected time snapshot"
        );

        let handoff_descriptor = Box::leak(Box::new(SliceDescriptor {
            time_basis: TimeBasis::RemainingLogicalTicks,
            hydrate: Some(handoff_load),
            reconnect_teardown: Some(handoff_teardown),
            rebase: Some(handoff_rebase),
            disconnect_save: Some(handoff_save),
            ..basic_descriptor("player.clock_once", 10)
        }));
        let mut handoff_registry = PersistenceSliceRegistry::empty();
        handoff_registry.register(handoff_descriptor).unwrap();
        let mut handoff_world = World::new();
        handoff_world.insert_resource(handoff_registry);
        handoff_world.insert_resource(HandoffTrace::default());
        let handoff_clock = CountingClock::new(400, 49_999);

        dispatch_reconnect_handoff(
            &mut handoff_world,
            token("offline:clock_once"),
            &handoff_clock,
        )
        .unwrap();
        assert_eq!(
            handoff_clock.reads(),
            (1, 1),
            "save, hydrate, and rebase must share one injected time snapshot"
        );
        assert!(handoff_world
            .resource::<HandoffTrace>()
            .events
            .iter()
            .all(|event| event.1 == 400 && event.2 == 49_999));
    }

    #[test]
    fn reconnect_handoff_enforces_same_tick_all_saves_before_any_load() {
        let first = Box::leak(Box::new(SliceDescriptor {
            time_basis: TimeBasis::RemainingLogicalTicks,
            rebase: Some(handoff_rebase),
            hydrate: Some(handoff_load),
            reconnect_teardown: Some(handoff_teardown),
            disconnect_save: Some(handoff_save),
            ..basic_descriptor("player.handoff_first", 10)
        }));
        let second = Box::leak(Box::new(SliceDescriptor {
            time_basis: TimeBasis::RemainingLogicalTicks,
            rebase: Some(handoff_rebase),
            hydrate: Some(handoff_load),
            reconnect_teardown: Some(handoff_teardown),
            disconnect_save: Some(handoff_save),
            write_binding: SECOND_TEST_BINDING,
            ..basic_descriptor("player.handoff_second", 20)
        }));
        let mut registry = PersistenceSliceRegistry::empty();
        registry.register(second).unwrap();
        registry.register(first).unwrap();
        let clock = FixedClock {
            runtime_tick: 400,
            wall_unix_millis: 49_999,
        };
        let mut world = World::new();
        world.insert_resource(registry);
        world.insert_resource(HandoffTrace::default());

        let handoff_token = token("offline:test");
        let generation = handoff_token.generation;
        assert_eq!(
            dispatch_reconnect_handoff(&mut world, handoff_token, &clock).unwrap(),
            ReconnectHandoffReport {
                generation,
                saves_attempted: 2,
                saves_completed: 2,
                blocked_saves: Vec::new(),
                loads_attempted: 2,
                loads_completed: 2,
                blocked_loads: Vec::new(),
                rebases_attempted: 2,
                rebases_completed: 2,
                blocked_rebases: Vec::new(),
                teardown_attempted: 2,
                teardowns_completed: 2,
                blocked_teardowns: Vec::new(),
                aborts_attempted: 0,
                aborts_completed: 0,
                blocked_aborts: Vec::new(),
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
                SliceRunReason::ReconnectTeardown,
                SliceRunReason::ReconnectTeardown,
                SliceRunReason::ReconnectLoad,
                SliceRunReason::ReconnectLoad,
                SliceRunReason::Rebase,
                SliceRunReason::Rebase,
            ]
        );
        assert!(world
            .resource::<HandoffTrace>()
            .events
            .iter()
            .all(|event| { event.1 == 400 && event.2 == 49_999 && event.3 == "offline:test" }));

        world.resource_mut::<HandoffTrace>().events.clear();
        world.resource_mut::<HandoffTrace>().fail_save = true;
        let report = dispatch_reconnect_handoff(&mut world, token("offline:test"), &clock).unwrap();
        assert_eq!(report.saves_attempted, 2);
        assert_eq!(report.saves_completed, 0);
        assert!(report.blocked_saves.is_empty());
        assert_eq!(report.loads_attempted, 0);
        assert_eq!(report.loads_completed, 0);
        assert_eq!(report.failures.len(), 2);
        assert_eq!(world.resource::<HandoffTrace>().events.len(), 2);
        assert!(world
            .resource::<HandoffTrace>()
            .events
            .iter()
            .all(|event| event.0 == SliceRunReason::DisconnectSave));

        world.resource_mut::<HandoffTrace>().events.clear();
        world.resource_mut::<HandoffTrace>().fail_save = false;
        world.resource_mut::<HandoffTrace>().block_save = true;
        let report = dispatch_reconnect_handoff(&mut world, token("offline:test"), &clock).unwrap();
        assert_eq!(report.saves_attempted, 2);
        assert_eq!(report.saves_completed, 0);
        assert_eq!(
            report.blocked_saves,
            vec![
                SliceId::new("player.handoff_first"),
                SliceId::new("player.handoff_second"),
            ]
        );
        assert_eq!(report.loads_attempted, 0);
        assert_eq!(report.loads_completed, 0);
        assert!(report.failures.is_empty());
        assert_eq!(world.resource::<HandoffTrace>().events.len(), 2);
        assert!(world
            .resource::<HandoffTrace>()
            .events
            .iter()
            .all(|event| event.0 == SliceRunReason::DisconnectSave));
        assert_eq!(report.rebases_attempted, 0);

        world.resource_mut::<HandoffTrace>().events.clear();
        world.resource_mut::<HandoffTrace>().block_save = false;
        world.resource_mut::<HandoffTrace>().block_teardown = true;
        let report = dispatch_reconnect_handoff(&mut world, token("offline:test"), &clock).unwrap();
        assert_eq!(report.teardown_attempted, 2);
        assert_eq!(report.teardowns_completed, 0);
        assert_eq!(
            report.blocked_teardowns,
            vec![
                SliceId::new("player.handoff_first"),
                SliceId::new("player.handoff_second"),
            ]
        );
        assert_eq!(report.loads_attempted, 0);
        assert!(report.failures.is_empty());

        world.resource_mut::<HandoffTrace>().events.clear();
        world.resource_mut::<HandoffTrace>().block_teardown = false;
        world.resource_mut::<HandoffTrace>().fail_teardown = true;
        let report = dispatch_reconnect_handoff(&mut world, token("offline:test"), &clock).unwrap();
        assert_eq!(report.teardown_attempted, 2);
        assert_eq!(report.teardowns_completed, 0);
        assert_eq!(report.loads_attempted, 0);
        assert_eq!(report.failures.len(), 2);
        assert!(report
            .failures
            .iter()
            .all(|failure| failure.reason == SliceRunReason::ReconnectTeardown));

        world.resource_mut::<HandoffTrace>().events.clear();
        world.resource_mut::<HandoffTrace>().fail_teardown = false;
        world.resource_mut::<HandoffTrace>().block_load = true;
        let report = dispatch_reconnect_handoff(&mut world, token("offline:test"), &clock).unwrap();
        assert_eq!(report.loads_attempted, 2);
        assert_eq!(report.loads_completed, 0);
        assert_eq!(
            report.blocked_loads,
            vec![
                SliceId::new("player.handoff_first"),
                SliceId::new("player.handoff_second"),
            ]
        );
        assert_eq!(report.rebases_attempted, 0);
        assert_eq!(report.aborts_attempted, 2);
        assert_eq!(report.aborts_completed, 2);
        assert_eq!(world.resource::<HandoffTrace>().events.len(), 8);
        assert_eq!(
            world
                .resource::<HandoffTrace>()
                .events
                .iter()
                .rev()
                .take(2)
                .map(|event| event.0)
                .collect::<Vec<_>>(),
            vec![
                SliceRunReason::ReconnectAbort,
                SliceRunReason::ReconnectAbort,
            ]
        );

        world.resource_mut::<HandoffTrace>().events.clear();
        world.resource_mut::<HandoffTrace>().block_load = false;
        world.resource_mut::<HandoffTrace>().fail_load = true;
        let report = dispatch_reconnect_handoff(&mut world, token("offline:test"), &clock).unwrap();
        assert_eq!(report.loads_attempted, 2);
        assert_eq!(report.loads_completed, 0);
        assert_eq!(report.rebases_attempted, 0);
        assert_eq!(report.aborts_attempted, 2);
        assert_eq!(report.aborts_completed, 2);
        assert_eq!(report.failures.len(), 2);
        assert!(report
            .failures
            .iter()
            .all(|failure| failure.reason == SliceRunReason::ReconnectLoad));

        world.resource_mut::<HandoffTrace>().events.clear();
        world.resource_mut::<HandoffTrace>().fail_load = false;
        world.resource_mut::<HandoffTrace>().block_rebase = true;
        let report = dispatch_reconnect_handoff(&mut world, token("offline:test"), &clock).unwrap();
        assert_eq!(report.rebases_attempted, 2);
        assert_eq!(report.rebases_completed, 0);
        assert_eq!(
            report.blocked_rebases,
            vec![
                SliceId::new("player.handoff_first"),
                SliceId::new("player.handoff_second"),
            ]
        );
        assert_eq!(report.aborts_attempted, 2);
        assert_eq!(report.aborts_completed, 2);
        assert!(report.failures.is_empty());

        world.resource_mut::<HandoffTrace>().events.clear();
        world.resource_mut::<HandoffTrace>().block_rebase = false;
        world.resource_mut::<HandoffTrace>().fail_rebase = true;
        let report = dispatch_reconnect_handoff(&mut world, token("offline:test"), &clock).unwrap();
        assert_eq!(report.rebases_attempted, 2);
        assert_eq!(report.rebases_completed, 0);
        assert_eq!(report.aborts_attempted, 2);
        assert_eq!(report.aborts_completed, 2);
        assert_eq!(report.failures.len(), 2);
        assert!(report
            .failures
            .iter()
            .all(|failure| failure.reason == SliceRunReason::Rebase));

        world.resource_mut::<HandoffTrace>().events.clear();
        {
            let mut trace = world.resource_mut::<HandoffTrace>();
            trace.fail_rebase = false;
            trace.fail_load = true;
            trace.fail_abort = true;
        }
        let report = dispatch_reconnect_handoff(&mut world, token("offline:test"), &clock).unwrap();
        assert_eq!(report.aborts_attempted, 2);
        assert_eq!(report.aborts_completed, 0);
        assert!(report.blocked_aborts.is_empty());
        assert_eq!(
            report
                .failures
                .iter()
                .filter(|failure| failure.reason == SliceRunReason::ReconnectAbort)
                .count(),
            2
        );

        world.resource_mut::<HandoffTrace>().events.clear();
        {
            let mut trace = world.resource_mut::<HandoffTrace>();
            trace.fail_load = false;
            trace.fail_abort = false;
            trace.block_load = true;
            trace.block_abort = true;
        }
        let report = dispatch_reconnect_handoff(&mut world, token("offline:test"), &clock).unwrap();
        assert_eq!(report.aborts_attempted, 2);
        assert_eq!(report.aborts_completed, 0);
        assert_eq!(
            report.blocked_aborts,
            vec![
                SliceId::new("player.handoff_second"),
                SliceId::new("player.handoff_first"),
            ]
        );
        assert!(report.failures.is_empty());
    }

    #[derive(Debug)]
    struct HydratedActivation {
        _guarded: GuardedSlice<u32, &'static str>,
        _tracker: DirtyTracker,
        _fence: PersistedRevisionFence,
    }

    #[derive(Debug, Default)]
    struct PartialHydrateState {
        first: Option<HydratedActivation>,
        second: Option<HydratedActivation>,
        fail_second: bool,
        block_second: bool,
        transitions: Vec<(SliceRunReason, SliceId)>,
    }

    impl Resource for PartialHydrateState {}

    fn activate_partial_handoff_slice(
        world: &mut World,
        slice_id: SliceId,
    ) -> Result<HydratedActivation, SliceRunError> {
        world.resource_scope(
            |_world, registry: valence::prelude::Mut<PersistenceSliceRegistry>| {
                let mut guarded = registry
                    .activate(
                        SliceLoad::<u32, &'static str>::Loaded(9),
                        slice_id,
                        subject_key("player:partial"),
                        DirtyRevision::new(4),
                        || 0,
                        |_| 0,
                    )
                    .map_err(|_| SliceRunError::new("partial activation rejected"))?;
                let (tracker, fence) = guarded
                    .restore_persistence_state()
                    .map_err(|_| SliceRunError::new("partial persistence state already issued"))?;
                Ok(HydratedActivation {
                    _guarded: guarded,
                    _tracker: tracker,
                    _fence: fence,
                })
            },
        )
    }

    fn hydrate_partial_first(world: &mut World, context: &SliceRunContext) -> SliceRunResult {
        assert_eq!(context.reason, SliceRunReason::ReconnectLoad);
        let slice_id = SliceId::new("player.partial_first");
        let activation = activate_partial_handoff_slice(world, slice_id)?;
        let mut state = world.resource_mut::<PartialHydrateState>();
        state.transitions.push((context.reason, slice_id));
        state.first = Some(activation);
        Ok(SliceRunOutcome::Clean)
    }

    fn hydrate_partial_second(world: &mut World, context: &SliceRunContext) -> SliceRunResult {
        assert_eq!(context.reason, SliceRunReason::ReconnectLoad);
        let slice_id = SliceId::new("player.partial_second");
        let activation = activate_partial_handoff_slice(world, slice_id)?;
        let mut state = world.resource_mut::<PartialHydrateState>();
        state.transitions.push((context.reason, slice_id));
        state.second = Some(activation);
        if state.fail_second {
            Err(SliceRunError::new("second hydrate failed after activation"))
        } else if state.block_second {
            Ok(SliceRunOutcome::SkippedBlocked)
        } else {
            Ok(SliceRunOutcome::Clean)
        }
    }

    fn teardown_partial_activation(
        world: &mut World,
        context: &SliceRunContext,
        slice_id: SliceId,
        first: bool,
    ) -> SliceRunResult {
        assert!(matches!(
            context.reason,
            SliceRunReason::ReconnectTeardown | SliceRunReason::ReconnectAbort
        ));
        let mut state = world.resource_mut::<PartialHydrateState>();
        state.transitions.push((context.reason, slice_id));
        if first {
            state.first = None;
        } else {
            state.second = None;
        }
        Ok(SliceRunOutcome::Clean)
    }

    fn teardown_partial_first(world: &mut World, context: &SliceRunContext) -> SliceRunResult {
        teardown_partial_activation(world, context, SliceId::new("player.partial_first"), true)
    }

    fn teardown_partial_second(world: &mut World, context: &SliceRunContext) -> SliceRunResult {
        teardown_partial_activation(world, context, SliceId::new("player.partial_second"), false)
    }

    fn rebase_partial_activation(_world: &mut World, context: &SliceRunContext) -> SliceRunResult {
        assert_eq!(context.reason, SliceRunReason::Rebase);
        Ok(SliceRunOutcome::Clean)
    }

    #[test]
    fn reconnect_handoff_aborts_partial_hydrate_and_allows_clean_retry() {
        let first = Box::leak(Box::new(SliceDescriptor {
            hydrate: Some(hydrate_partial_first),
            reconnect_teardown: Some(teardown_partial_first),
            rebase: Some(rebase_partial_activation),
            disconnect_save: Some(handoff_save),
            ..basic_descriptor("player.partial_first", 10)
        }));
        let second = Box::leak(Box::new(SliceDescriptor {
            hydrate: Some(hydrate_partial_second),
            reconnect_teardown: Some(teardown_partial_second),
            rebase: Some(rebase_partial_activation),
            disconnect_save: Some(handoff_save),
            write_binding: SECOND_TEST_BINDING,
            ..basic_descriptor("player.partial_second", 20)
        }));
        let clock = FixedClock {
            runtime_tick: 400,
            wall_unix_millis: 49_999,
        };

        for (name, fail_second, block_second) in [("error", true, false), ("blocked", false, true)]
        {
            let mut registry = PersistenceSliceRegistry::empty();
            registry.register(first).unwrap();
            registry.register(second).unwrap();
            let mut world = World::new();
            world.insert_resource(registry);
            world.insert_resource(HandoffTrace::default());
            world.insert_resource(PartialHydrateState {
                fail_second,
                block_second,
                ..PartialHydrateState::default()
            });

            let report =
                dispatch_reconnect_handoff(&mut world, token("player:partial"), &clock).unwrap();
            assert_eq!(report.loads_attempted, 2, "{name}");
            assert_eq!(report.loads_completed, 1, "{name}");
            assert_eq!(report.rebases_attempted, 0, "{name}");
            assert_eq!(report.aborts_attempted, 2, "{name}");
            assert_eq!(report.aborts_completed, 2, "{name}");
            let transitions = &world.resource::<PartialHydrateState>().transitions;
            assert_eq!(
                &transitions[transitions.len() - 2..],
                &[
                    (SliceRunReason::ReconnectAbort, second.id),
                    (SliceRunReason::ReconnectAbort, first.id),
                ],
                "{name}"
            );
            let subject = subject_key("player:partial");
            {
                let registry = world.resource::<PersistenceSliceRegistry>();
                assert!(!registry.active_subject_domain(&subject, first.write_binding.domain()));
                assert!(!registry.active_subject_domain(&subject, second.write_binding.domain()));
            }

            {
                let mut state = world.resource_mut::<PartialHydrateState>();
                state.fail_second = false;
                state.block_second = false;
            }
            let retry =
                dispatch_reconnect_handoff(&mut world, token("player:partial"), &clock).unwrap();
            assert_eq!(retry.loads_completed, 2, "{name}");
            assert_eq!(retry.rebases_completed, 2, "{name}");
            assert_eq!(retry.aborts_attempted, 0, "{name}");
        }
    }

    #[derive(Debug, Default)]
    struct HandoffActivationState {
        guarded: Option<GuardedSlice<u32, &'static str>>,
        tracker: Option<DirtyTracker>,
        fence: Option<PersistedRevisionFence>,
        release_guarded: bool,
        release_tracker: bool,
        release_fence: bool,
    }

    impl Resource for HandoffActivationState {}

    fn teardown_activation(world: &mut World, context: &SliceRunContext) -> SliceRunResult {
        assert!(matches!(
            context.reason,
            SliceRunReason::ReconnectTeardown | SliceRunReason::ReconnectAbort
        ));
        let mut state = world.resource_mut::<HandoffActivationState>();
        if state.release_guarded {
            state.guarded = None;
        }
        if state.release_tracker {
            state.tracker = None;
        }
        if state.release_fence {
            state.fence = None;
        }
        Ok(SliceRunOutcome::Clean)
    }

    struct RetainedLeaseCase {
        name: &'static str,
        release_guarded: bool,
        release_tracker: bool,
        release_fence: bool,
    }

    fn handoff_world_with_retained_activation(
        first: &'static SliceDescriptor,
        second: &'static SliceDescriptor,
        case: &RetainedLeaseCase,
    ) -> World {
        let mut registry = PersistenceSliceRegistry::empty();
        registry.register(first).unwrap();
        registry.register(second).unwrap();
        let mut guarded = registry
            .activate(
                SliceLoad::<u32, &'static str>::Loaded(9),
                second.id,
                subject_key("player:activation"),
                DirtyRevision::new(4),
                || 0,
                |_| 0,
            )
            .unwrap();
        let (tracker, fence) = guarded.restore_persistence_state().unwrap();
        let mut world = World::new();
        world.insert_resource(registry);
        world.insert_resource(HandoffTrace::default());
        world.insert_resource(HandoffActivationState {
            guarded: Some(guarded),
            tracker: Some(tracker),
            fence: Some(fence),
            release_guarded: case.release_guarded,
            release_tracker: case.release_tracker,
            release_fence: case.release_fence,
        });
        world
    }

    #[test]
    fn reconnect_handoff_requires_all_old_activation_leases_released_before_any_hydrate() {
        let first = Box::leak(Box::new(SliceDescriptor {
            time_basis: TimeBasis::RemainingLogicalTicks,
            rebase: Some(handoff_rebase),
            hydrate: Some(handoff_load),
            reconnect_teardown: Some(teardown_activation),
            disconnect_save: Some(handoff_save),
            ..basic_descriptor("player.activation_first", 10)
        }));
        let second = Box::leak(Box::new(SliceDescriptor {
            time_basis: TimeBasis::RemainingLogicalTicks,
            rebase: Some(handoff_rebase),
            hydrate: Some(handoff_load),
            reconnect_teardown: Some(teardown_activation),
            disconnect_save: Some(handoff_save),
            write_binding: SECOND_TEST_BINDING,
            ..basic_descriptor("player.activation_second", 20)
        }));
        let clock = FixedClock {
            runtime_tick: 400,
            wall_unix_millis: 49_999,
        };
        let retained_cases = [
            RetainedLeaseCase {
                name: "guarded slice",
                release_guarded: false,
                release_tracker: true,
                release_fence: true,
            },
            RetainedLeaseCase {
                name: "dirty tracker",
                release_guarded: true,
                release_tracker: false,
                release_fence: true,
            },
            RetainedLeaseCase {
                name: "persisted revision fence",
                release_guarded: true,
                release_tracker: true,
                release_fence: false,
            },
        ];

        for case in &retained_cases {
            let mut world = handoff_world_with_retained_activation(first, second, case);
            let error = dispatch_reconnect_handoff(&mut world, token("player:activation"), &clock)
                .unwrap_err();
            assert_eq!(
                error,
                SliceDispatchError::DuplicateSubject {
                    slice_id: second.id,
                    domain: SECOND_TEST_BINDING.domain(),
                },
                "a retained {} must keep the durable subject active",
                case.name
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
                ],
                "a retained {} must prevent every hydrate",
                case.name
            );
        }

        let all_released = RetainedLeaseCase {
            name: "none",
            release_guarded: true,
            release_tracker: true,
            release_fence: true,
        };
        let mut world = handoff_world_with_retained_activation(first, second, &all_released);
        let report =
            dispatch_reconnect_handoff(&mut world, token("player:activation"), &clock).unwrap();
        assert_eq!(report.saves_completed, 2);
        assert_eq!(report.teardowns_completed, 2);
        assert_eq!(report.loads_completed, 2);
        assert_eq!(report.rebases_completed, 2);
        assert!(report.failures.is_empty());
    }

    #[test]
    fn failed_load_fallback_is_read_only_and_never_becomes_dirty() {
        let descriptor = basic_descriptor("player.failed", 10);
        let (_registry, mut guarded) = activate(
            &descriptor,
            SliceLoad::<u32, _>::Failed("invalid json"),
            "player:failed",
            DirtyRevision::new(7),
            || 1,
            |_error| 0,
        );
        let (mut tracker, _fence) = guarded.restore_persistence_state().unwrap();
        let mut mutation_called = false;

        assert_eq!(
            guarded.load_state(),
            &SliceLoadState::Failed("invalid json")
        );
        assert_eq!(
            guarded.mutate(&mut tracker, |value| {
                mutation_called = true;
                *value = 99;
            }),
            Err(GuardedSliceMutationError::LoadFailed)
        );
        assert!(!mutation_called);
        assert_eq!(*guarded.value(), 0);
        assert_eq!(tracker.current_revision(), DirtyRevision::new(7));
        assert!(!tracker.is_dirty());

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
        let descriptor = Box::leak(Box::new(descriptor));
        let mut registry = PersistenceSliceRegistry::empty();
        registry.register(descriptor).unwrap();
        let mut fallback_called = false;
        let result = registry.activate(
            SliceLoad::<u32, _>::Failed("corrupt ledger"),
            descriptor.id,
            subject_key("world:ledger"),
            DirtyRevision::default(),
            || 1,
            |_error| {
                fallback_called = true;
                0
            },
        );

        let refusal = result.unwrap_err();
        assert_eq!(refusal.slice_id(), SliceId::new("world.ledger"));
        assert_eq!(refusal.cause(), Some(&"corrupt ledger"));
        assert!(!fallback_called);
    }

    #[test]
    fn missing_and_loaded_slices_are_writable() {
        let descriptor = basic_descriptor("player.writable", 10);
        let (_missing_registry, missing) = activate(
            &descriptor,
            SliceLoad::<u32, &str>::Missing,
            "player:missing",
            DirtyRevision::default(),
            || 7,
            |_| 0,
        );
        let (_loaded_registry, loaded) = activate(
            &descriptor,
            SliceLoad::<u32, &str>::Loaded(9),
            "player:loaded",
            DirtyRevision::default(),
            || 0,
            |_| 0,
        );

        assert_eq!(*missing.value(), 7);
        assert_eq!(missing.load_state(), &SliceLoadState::Missing);
        assert!(missing.write_permit(WriteOutlet::Autosave).is_ok());
        assert_eq!(*loaded.value(), 9);
        assert_eq!(loaded.load_state(), &SliceLoadState::Loaded);
        assert!(loaded.write_permit(WriteOutlet::Shutdown).is_ok());
    }

    #[test]
    fn stable_subject_activation_rejects_duplicate_domain_writer_until_release() {
        let first_descriptor = Box::leak(Box::new(basic_descriptor("player.subject.first", 10)));
        let second_descriptor = Box::leak(Box::new(basic_descriptor("player.subject.second", 20)));
        let mut registry = PersistenceSliceRegistry::empty();
        registry.register(first_descriptor).unwrap();
        registry.register(second_descriptor).unwrap();

        let mut first = registry
            .activate(
                SliceLoad::<u32, &str>::Loaded(9),
                first_descriptor.id,
                subject_key("player:stable"),
                DirtyRevision::new(4),
                || 0,
                |_| 0,
            )
            .unwrap();
        let duplicate = registry.activate(
            SliceLoad::<u32, &str>::Loaded(10),
            second_descriptor.id,
            subject_key("player:stable"),
            DirtyRevision::new(4),
            || 0,
            |_| 0,
        );
        assert!(matches!(
            duplicate,
            Err(SliceActivationError::DuplicateSubject {
                slice_id,
                domain,
            }) if slice_id == second_descriptor.id && domain == TEST_BINDING.domain()
        ));

        let other_subject = registry
            .activate(
                SliceLoad::<u32, &str>::Loaded(11),
                second_descriptor.id,
                subject_key("player:other"),
                DirtyRevision::new(4),
                || 0,
                |_| 0,
            )
            .unwrap();
        assert_eq!(*other_subject.value(), 11);
        let (tracker, fence) = first.restore_persistence_state().unwrap();
        drop(first);

        let retained_tracker = registry.activate(
            SliceLoad::<u32, &str>::Loaded(12),
            second_descriptor.id,
            subject_key("player:stable"),
            DirtyRevision::new(7),
            || 0,
            |_| 0,
        );
        assert!(matches!(
            retained_tracker,
            Err(SliceActivationError::DuplicateSubject { .. })
        ));
        drop(tracker);

        let retained_fence = registry.activate(
            SliceLoad::<u32, &str>::Loaded(12),
            second_descriptor.id,
            subject_key("player:stable"),
            DirtyRevision::new(7),
            || 0,
            |_| 0,
        );
        assert!(matches!(
            retained_fence,
            Err(SliceActivationError::DuplicateSubject { .. })
        ));
        drop(fence);

        let reactivated = registry
            .activate(
                SliceLoad::<u32, &str>::Loaded(12),
                second_descriptor.id,
                subject_key("player:stable"),
                DirtyRevision::new(7),
                || 0,
                |_| 0,
            )
            .unwrap();
        assert_eq!(*reactivated.value(), 12);
    }

    #[test]
    fn mutation_and_durable_receipts_remain_bound_to_one_guarded_subject() {
        let descriptor = basic_descriptor("player.subject", 10);
        let (_first_registry, mut first) = activate(
            &descriptor,
            SliceLoad::<u32, &str>::Loaded(9),
            "player:first",
            DirtyRevision::default(),
            || 0,
            |_| 0,
        );
        let (_second_registry, mut second) = activate(
            &descriptor,
            SliceLoad::<u32, &str>::Loaded(9),
            "player:second",
            DirtyRevision::default(),
            || 0,
            |_| 0,
        );
        let (mut first_tracker, mut first_fence) = first.restore_persistence_state().unwrap();
        let (mut second_tracker, _second_fence) = second.restore_persistence_state().unwrap();

        let (revision, ()) = first
            .mutate(&mut first_tracker, |value| *value = 10)
            .unwrap();
        assert_eq!(revision, DirtyRevision::new(1));
        assert_eq!(*first.value(), 10);
        assert!(first_tracker.is_dirty());

        let mut wrong_subject_closure_called = false;
        assert_eq!(
            second.mutate(&mut first_tracker, |_| {
                wrong_subject_closure_called = true;
            }),
            Err(GuardedSliceMutationError::WrongSubject)
        );
        assert!(!wrong_subject_closure_called);
        assert_eq!(*second.value(), 9);

        let second_permit = second.write_permit(WriteOutlet::Autosave).unwrap();
        assert_eq!(
            first_tracker.begin_snapshot(second_permit, |value| *value),
            Err(SnapshotProvenanceError::WrongSubject)
        );

        second
            .mutate(&mut second_tracker, |value| *value = 11)
            .unwrap();
        let second_permit = second.write_permit(WriteOutlet::Autosave).unwrap();
        let second_snapshot = second_tracker
            .begin_snapshot(second_permit, |value| *value)
            .unwrap()
            .unwrap();
        let mut wrong_subject_writer_called = false;
        assert_eq!(
            first_fence.commit(second_snapshot, |_request| {
                wrong_subject_writer_called = true;
                Ok::<_, &str>(durable_writer::acquire())
            }),
            Err(DurableCommitError::WrongSubject)
        );
        assert!(!wrong_subject_writer_called);
        assert_eq!(first_fence.persisted_revision(), DirtyRevision::default());

        let first_permit = first.write_permit(WriteOutlet::Autosave).unwrap();
        let first_snapshot = first_tracker
            .begin_snapshot(first_permit, |value| *value)
            .unwrap()
            .unwrap();
        let first_receipt = first_fence
            .commit(first_snapshot, |_request| {
                Ok::<_, &str>(durable_writer::acquire())
            })
            .unwrap();
        assert_eq!(
            second_tracker.acknowledge(first_receipt),
            DirtyAcknowledgement::WrongSubject
        );
        assert!(second_tracker.is_dirty());
    }

    #[test]
    fn failed_durable_write_and_stale_receipt_never_clear_dirty_state() {
        let descriptor = basic_descriptor("player.dirty", 10);
        let (_registry, mut guarded) = activate(
            &descriptor,
            SliceLoad::<u32, &str>::Loaded(9),
            "player:dirty",
            DirtyRevision::default(),
            || 0,
            |_| 0,
        );
        let (mut tracker, mut fence) = guarded.restore_persistence_state().unwrap();

        guarded.mutate(&mut tracker, |value| *value = 10).unwrap();
        let first_permit = guarded.write_permit(WriteOutlet::Autosave).unwrap();
        let first = tracker
            .begin_snapshot(first_permit, |value| *value)
            .unwrap()
            .unwrap();
        assert_eq!(*first.payload(), 10);

        guarded.mutate(&mut tracker, |value| *value = 11).unwrap();
        let failed_permit = guarded.write_permit(WriteOutlet::Autosave).unwrap();
        let failed = tracker
            .begin_snapshot(failed_permit, |value| *value)
            .unwrap()
            .unwrap();
        let result = fence.commit(failed, |_request| {
            Err::<durable_writer::Capability, _>("disk unavailable")
        });
        assert_eq!(
            result,
            Err(DurableCommitError::WriteFailed("disk unavailable"))
        );
        assert!(tracker.is_dirty());
        assert_eq!(
            guarded.restore_persistence_state(),
            Err(PersistenceStateAlreadyIssued),
            "a failed writer must not be bypassed by restoring a new clean tracker/fence"
        );

        let stale_receipt = fence
            .commit(first, |request| {
                assert_eq!(*request.payload(), 10);
                assert_eq!(request.write_revision(), DirtyRevision::new(1));
                Ok::<_, &str>(durable_writer::acquire())
            })
            .unwrap();
        assert_eq!(
            tracker.acknowledge(stale_receipt),
            DirtyAcknowledgement::Stale
        );
        assert!(tracker.is_dirty());

        let latest_permit = guarded.write_permit(WriteOutlet::Autosave).unwrap();
        let latest = tracker
            .begin_snapshot(latest_permit, |value| *value)
            .unwrap()
            .unwrap();
        let receipt = fence
            .commit(latest, |request| {
                assert_eq!(*request.payload(), 11);
                assert_eq!(request.write_revision(), DirtyRevision::new(2));
                Ok::<_, &str>(durable_writer::acquire())
            })
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
        let descriptor = Box::leak(Box::new(SliceDescriptor {
            write_ordering: WriteOrdering::PersistedRevisionCas,
            ..basic_descriptor("player.bound", 10)
        }));
        let unregistered_downgrade = SliceDescriptor {
            write_ordering: WriteOrdering::Serialized,
            ..*descriptor
        };
        let mut registry = PersistenceSliceRegistry::empty();
        registry.register(descriptor).unwrap();
        let mut guarded = registry
            .activate(
                SliceLoad::<u32, &str>::Loaded(9),
                descriptor.id,
                subject_key("player:bound"),
                DirtyRevision::new(41),
                || 0,
                |_| 0,
            )
            .unwrap();
        assert_eq!(
            unregistered_downgrade.write_ordering,
            WriteOrdering::Serialized
        );
        let other_descriptor = SliceDescriptor {
            write_binding: OTHER_BINDING,
            ..basic_descriptor("player.other", 20)
        };
        let (_other_registry, mut other) = activate(
            &other_descriptor,
            SliceLoad::<u32, &str>::Loaded(9),
            "player:other-binding",
            DirtyRevision::default(),
            || 0,
            |_| 0,
        );
        let (mut wrong_tracker, _wrong_fence) = other.restore_persistence_state().unwrap();
        let mut wrong_subject_mutation_called = false;
        assert!(matches!(
            guarded.mutate(&mut wrong_tracker, |_| {
                wrong_subject_mutation_called = true;
            }),
            Err(GuardedSliceMutationError::WrongBinding(
                WriteBindingMismatch {
                    expected: OTHER_BINDING,
                    actual: TEST_BINDING,
                }
            ))
        ));
        assert!(!wrong_subject_mutation_called);
        let permit = guarded.write_permit(WriteOutlet::Shutdown).unwrap();
        assert!(matches!(
            wrong_tracker.begin_snapshot(permit, |value| *value),
            Err(SnapshotProvenanceError::WrongBinding(
                WriteBindingMismatch {
                    expected: OTHER_BINDING,
                    actual: TEST_BINDING,
                }
            ))
        ));

        let (mut tracker, mut fence) = guarded.restore_persistence_state().unwrap();
        guarded.mutate(&mut tracker, |value| *value = 10).unwrap();
        let permit = guarded.write_permit(WriteOutlet::Shutdown).unwrap();
        let snapshot = tracker
            .begin_snapshot(permit, |value| *value)
            .unwrap()
            .unwrap();
        let receipt = fence
            .commit(snapshot, |request| {
                assert_eq!(request.binding(), TEST_BINDING);
                assert_eq!(request.subject_key(), &subject_key("player:bound"));
                assert_eq!(*request.payload(), 10);
                assert_eq!(
                    request.expected_persisted_revision(),
                    DirtyRevision::new(41)
                );
                assert_eq!(request.write_revision(), DirtyRevision::new(42));
                assert_eq!(request.ordering(), WriteOrdering::PersistedRevisionCas);
                Ok::<_, &str>(durable_writer::acquire())
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
    fn dirty_revision_overflow_rejects_mutation_without_changing_value() {
        let descriptor = basic_descriptor("player.overflow", 10);
        let (_registry, mut guarded) = activate(
            &descriptor,
            SliceLoad::<u32, &str>::Loaded(9),
            "player:overflow",
            DirtyRevision::default(),
            || 0,
            |_| 0,
        );
        let mut tracker = DirtyTracker {
            binding: TEST_BINDING,
            subject: guarded.subject.clone(),
            current: DirtyRevision::new(u64::MAX),
            acknowledged: DirtyRevision::new(u64::MAX - 1),
        };
        let mut closure_called = false;

        assert_eq!(
            guarded.mutate(&mut tracker, |value| {
                closure_called = true;
                *value = 10;
            }),
            Err(GuardedSliceMutationError::RevisionExhausted)
        );
        assert!(!closure_called);
        assert_eq!(*guarded.value(), 9);
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
            rebase_remaining_deadline(advancing, 10, 1_000_049),
            Ok(210),
            "49ms is below one logical tick and must not reduce the deadline"
        );
        assert_eq!(
            rebase_remaining_deadline(advancing, 10, 1_000_050),
            Ok(209),
            "50ms is exactly one logical tick"
        );
        let one_tick = RemainingDeadline {
            remaining_ticks: 1,
            offline_policy: OfflineTimePolicy::Continue,
            ..paused
        };
        assert_eq!(
            rebase_remaining_deadline(one_tick, 10, 1_000_049),
            Ok(11),
            "one remaining tick survives until the 50ms boundary"
        );
        assert_eq!(
            rebase_remaining_deadline(one_tick, 10, 1_000_050),
            Ok(10),
            "one remaining tick expires exactly at the 50ms boundary"
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
