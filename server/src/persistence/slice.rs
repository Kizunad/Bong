//! Cross-domain persistence contracts and canonical production registry support.
//!
//! The framework owns descriptor validation and dispatch. Production domains migrate
//! into the canonical registry one at a time so an old lifecycle hook and its slice
//! adapter are never registered concurrently.

use std::{
    collections::{HashMap, HashSet},
    fmt,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex, Weak,
    },
};

use rusqlite::{ffi, Connection, Params, Transaction, TransactionBehavior};
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
    ReconnectPreflight,
    ReconnectCleanup,
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
///
/// The context is intentionally not cloneable: a reconnect hydrate may borrow its
/// subject-bound activation capability only for the synchronous hook invocation and
/// cannot retain authority for later replay.
#[derive(Debug)]
pub struct SliceRunContext {
    pub reason: SliceRunReason,
    pub runtime_tick: u64,
    pub wall_unix_millis: u64,
    /// Stable persisted subject (for example a player identity) during handoff.
    pub handoff_key: Option<String>,
    reconnect_activation: Option<ReconnectActivationCapability>,
}

impl SliceRunContext {
    /// Persistence-private subject authority for a reconnect hydrate.
    ///
    /// The capability is minted by the one-shot handoff and cannot be constructed by
    /// gameplay. Registry activation derives the stable key from it instead of accepting
    /// an adapter-selected key.
    pub(in crate::persistence) fn reconnect_activation(
        &self,
    ) -> Result<&ReconnectActivationCapability, SliceRunError> {
        self.reconnect_activation
            .as_ref()
            .ok_or_else(|| SliceRunError::new("reconnect activation capability is unavailable"))
    }
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
/// Read-only reconnect check run before any old activation is destroyed.
pub type SlicePreflightHook = fn(&World, &SliceRunContext) -> SliceRunResult;
/// Destructive reconnect cleanup with no blocked/error return channel.
///
/// A panic is a descriptor contract violation and remains fail-fast.
pub type SliceCleanupHook = fn(&mut World, &SliceRunContext);

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
    /// Checks whether this slice can release its old reconnect activation.
    ///
    /// The hook must be non-destructive: the dispatcher runs every preflight before
    /// committing any cleanup, and a blocked or failed preflight preserves every old
    /// `GuardedSlice`, `DirtyTracker`, and `PersistedRevisionFence` activation.
    pub reconnect_preflight: Option<SlicePreflightHook>,
    /// Drops this slice's activation after reconnect preflight commits.
    ///
    /// Required together with `reconnect_preflight` for every player slice that
    /// participates in reconnect. There is deliberately no blocked/error return
    /// channel; a panic is a descriptor contract violation and remains fail-fast.
    /// After the hook returns, the dispatcher verifies that the subject/domain lease
    /// was actually released. The dispatcher also invokes it in reverse order with
    /// `ReconnectAbort` for every attempted hydrate descriptor, including a hook that
    /// activated state before returning blocked or failed. It must be idempotent.
    pub reconnect_cleanup: Option<SliceCleanupHook>,
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
    DuplicateWriteDomain {
        domain: WriteDomain,
        first_slice_id: SliceId,
        duplicate_slice_id: SliceId,
    },
    ZeroAutosaveCadence {
        slice_id: SliceId,
    },
    MissingHydrateHook {
        slice_id: SliceId,
    },
    MissingReconnectPreflightHook {
        slice_id: SliceId,
    },
    MissingReconnectCleanupHook {
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
            Self::DuplicateWriteDomain {
                domain,
                first_slice_id,
                duplicate_slice_id,
            } => write!(
                formatter,
                "write domain `{}` is already owned by slice `{first_slice_id}` and cannot be registered by `{duplicate_slice_id}`",
                domain.as_str()
            ),
            Self::ZeroAutosaveCadence { slice_id } => {
                write!(formatter, "slice `{slice_id}` has a zero autosave cadence")
            }
            Self::MissingHydrateHook { slice_id } => {
                write!(
                    formatter,
                    "slice `{slice_id}` declares a time basis or rebase hook without a hydrate hook"
                )
            }
            Self::MissingReconnectPreflightHook { slice_id } => {
                write!(
                    formatter,
                    "player slice `{slice_id}` participates in reconnect without a non-destructive preflight hook"
                )
            }
            Self::MissingReconnectCleanupHook { slice_id } => {
                write!(
                    formatter,
                    "player slice `{slice_id}` participates in reconnect without a cleanup hook that has no blocked/error return channel"
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LeaseMutationKind {
    Acquired,
    Released,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LeaseMutation {
    domain: WriteDomain,
    subject: PersistenceSubjectKey,
    kind: LeaseMutationKind,
}

#[derive(Debug, Default)]
struct LeaseBook {
    active_subjects: Mutex<HashMap<PersistenceSubjectKey, HashMap<WriteDomain, Weak<LeaseToken>>>>,
    audit: Mutex<Option<Vec<LeaseMutation>>>,
    audit_poisoned: AtomicBool,
}

impl LeaseBook {
    fn begin_audit(&self) -> Result<(), SliceDispatchError> {
        let mut audit = self
            .audit
            .lock()
            .map_err(|_| SliceDispatchError::PoisonedSubjectRegistry)?;
        if audit.is_some() {
            return Err(SliceDispatchError::LeaseAuditAlreadyActive);
        }
        *audit = Some(Vec::new());
        self.audit_poisoned.store(false, Ordering::Release);
        Ok(())
    }

    fn record(
        &self,
        domain: WriteDomain,
        subject: &PersistenceSubjectKey,
        kind: LeaseMutationKind,
    ) {
        let Ok(mut audit) = self.audit.lock() else {
            self.audit_poisoned.store(true, Ordering::Release);
            return;
        };
        if let Some(events) = audit.as_mut() {
            events.push(LeaseMutation {
                domain,
                subject: subject.clone(),
                kind,
            });
        }
    }

    fn finish_audit(&self) -> Result<Vec<LeaseMutation>, SliceDispatchError> {
        if self.audit_poisoned.load(Ordering::Acquire) {
            return Err(SliceDispatchError::PoisonedSubjectRegistry);
        }
        let mut audit = self
            .audit
            .lock()
            .map_err(|_| SliceDispatchError::PoisonedSubjectRegistry)?;
        audit.take().ok_or(SliceDispatchError::LeaseAuditNotActive)
    }
}

/// Sorted registry of persistence lifecycle descriptors.
///
/// The type itself, its construction, and descriptor-token issuance are restricted to
/// the `crate::persistence` trust boundary. Code outside that boundary cannot create,
/// insert, or remove a second registry and use it to downgrade the application's
/// canonical write policy.
#[derive(Debug)]
pub(in crate::persistence) struct PersistenceSliceRegistry {
    descriptors: Vec<&'static SliceDescriptor>,
    leases: Arc<LeaseBook>,
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
            leases: Arc::new(LeaseBook::default()),
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
        if descriptor.hydrate.is_none()
            && (descriptor.time_basis != TimeBasis::None || descriptor.rebase.is_some())
        {
            return Err(SliceRegistryError::MissingHydrateHook {
                slice_id: descriptor.id,
            });
        }
        if descriptor.time_basis != TimeBasis::None && descriptor.rebase.is_none() {
            return Err(SliceRegistryError::MissingRebaseHook {
                slice_id: descriptor.id,
            });
        }
        if player_reconnect_participant(descriptor) && descriptor.reconnect_preflight.is_none() {
            return Err(SliceRegistryError::MissingReconnectPreflightHook {
                slice_id: descriptor.id,
            });
        }
        if player_reconnect_participant(descriptor) && descriptor.reconnect_cleanup.is_none() {
            return Err(SliceRegistryError::MissingReconnectCleanupHook {
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
            return Err(SliceRegistryError::DuplicateWriteDomain {
                domain: descriptor.write_binding.domain,
                first_slice_id: registered.id,
                duplicate_slice_id: descriptor.id,
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

    pub(in crate::persistence) fn active_subject_domain(
        &self,
        subject_key: &PersistenceSubjectKey,
        domain: WriteDomain,
    ) -> bool {
        let Ok(mut subjects) = self.leases.active_subjects.lock() else {
            return true;
        };
        let Some(domains) = subjects.get_mut(subject_key) else {
            return false;
        };
        domains.retain(|_, subject| subject.strong_count() > 0);
        let active = domains.contains_key(&domain);
        if domains.is_empty() {
            subjects.remove(subject_key);
        }
        active
    }

    pub(in crate::persistence) fn active_subject_leases(
        &self,
    ) -> Result<HashSet<(WriteDomain, PersistenceSubjectKey)>, SliceDispatchError> {
        let mut subjects = self
            .leases
            .active_subjects
            .lock()
            .map_err(|_| SliceDispatchError::PoisonedSubjectRegistry)?;
        let mut leases = HashSet::new();
        subjects.retain(|subject, domains| {
            domains.retain(|_, lease| lease.strong_count() > 0);
            for domain in domains.keys().copied() {
                leases.insert((domain, subject.clone()));
            }
            !domains.is_empty()
        });
        Ok(leases)
    }

    pub(in crate::persistence) fn active_subject_domains(
        &self,
        subject_key: &PersistenceSubjectKey,
    ) -> Result<HashSet<WriteDomain>, SliceDispatchError> {
        let mut subjects = self
            .leases
            .active_subjects
            .lock()
            .map_err(|_| SliceDispatchError::PoisonedSubjectRegistry)?;
        let Some(domains) = subjects.get_mut(subject_key) else {
            return Ok(HashSet::new());
        };
        domains.retain(|_, lease| lease.strong_count() > 0);
        let result = domains.keys().copied().collect();
        if domains.is_empty() {
            subjects.remove(subject_key);
        }
        Ok(result)
    }

    fn begin_lease_audit(&self) -> Result<(), SliceDispatchError> {
        self.leases.begin_audit()
    }

    fn finish_lease_audit(&self) -> Result<Vec<LeaseMutation>, SliceDispatchError> {
        self.leases.finish_audit()
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
        subject: &ReconnectActivationCapability,
        initial_revision: DirtyRevision,
        on_missing: impl FnOnce() -> T,
        on_failed: impl FnOnce(&E) -> T,
    ) -> Result<GuardedSlice<T, E>, SliceActivationError<E>> {
        self.activate_subject(
            load,
            slice_id,
            subject.subject_key().clone(),
            initial_revision,
            on_missing,
            on_failed,
        )
    }

    #[cfg(test)]
    fn activate_test_subject<T, E>(
        &self,
        load: SliceLoad<T, E>,
        slice_id: SliceId,
        subject_key: PersistenceSubjectKey,
        initial_revision: DirtyRevision,
        on_missing: impl FnOnce() -> T,
        on_failed: impl FnOnce(&E) -> T,
    ) -> Result<GuardedSlice<T, E>, SliceActivationError<E>> {
        self.activate_subject(
            load,
            slice_id,
            subject_key,
            initial_revision,
            on_missing,
            on_failed,
        )
    }

    fn activate_subject<T, E>(
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
        if load.is_failed() && descriptor.load_failure == LoadFailurePolicy::RefuseStartup {
            return load.refuse_startup(descriptor.id);
        }

        let domain = descriptor.write_binding.domain();
        let mut subjects = self.leases.active_subjects.lock().map_err(|_| {
            SliceActivationError::PoisonedSubjectRegistry {
                slice_id: descriptor.id,
            }
        })?;
        let domains = subjects.entry(subject_key.clone()).or_default();
        domains.retain(|_, lease| lease.strong_count() > 0);
        if domains.contains_key(&domain) {
            return Err(SliceActivationError::DuplicateSubject {
                slice_id: descriptor.id,
                domain,
            });
        }
        let subject = SliceSubject::new(self.leases.clone(), subject_key.clone(), domain);
        domains.insert(domain, Arc::downgrade(&subject.0));
        self.leases
            .record(domain, &subject_key, LeaseMutationKind::Acquired);
        drop(subjects);

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

fn player_reconnect_participant(descriptor: &SliceDescriptor) -> bool {
    descriptor.scope == SliceScope::PlayerEntity
        && (descriptor.disconnect_save.is_some()
            || descriptor.hydrate.is_some()
            || descriptor.rebase.is_some())
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
    PoisonedSubjectRegistry,
    LeaseAuditAlreadyActive,
    LeaseAuditNotActive,
    MissingHydrateLease {
        slice_id: SliceId,
        domain: WriteDomain,
    },
    UnexpectedHydrateLease {
        slice_id: SliceId,
        domain: WriteDomain,
    },
    UnexpectedHydrateSubject {
        slice_id: SliceId,
        domain: WriteDomain,
    },
    UnexpectedRebaseLease {
        slice_id: SliceId,
        domain: WriteDomain,
    },
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
            Self::PoisonedSubjectRegistry => {
                formatter.write_str("canonical persistence subject registry is poisoned")
            }
            Self::LeaseAuditAlreadyActive => {
                formatter.write_str("canonical persistence lease audit is already active")
            }
            Self::LeaseAuditNotActive => {
                formatter.write_str("canonical persistence lease audit is not active")
            }
            Self::MissingHydrateLease { slice_id, domain } => write!(
                formatter,
                "slice `{slice_id}` returned hydrate success without activating subject domain `{}`",
                domain.as_str()
            ),
            Self::UnexpectedHydrateLease { slice_id, domain } => write!(
                formatter,
                "slice `{slice_id}` changed an unexpected reconnect lease in domain `{}`",
                domain.as_str()
            ),
            Self::UnexpectedHydrateSubject { slice_id, domain } => write!(
                formatter,
                "slice `{slice_id}` activated an unexpected reconnect subject in domain `{}`",
                domain.as_str()
            ),
            Self::UnexpectedRebaseLease { slice_id, domain } => write!(
                formatter,
                "slice `{slice_id}` changed reconnect leases during rebase in domain `{}`",
                domain.as_str()
            ),
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
        reconnect_activation: None,
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

/// One-shot subject-bound authority borrowed only during one reconnect hydrate hook.
///
/// No constructor, subject getter, `Clone`, or public path crosses the persistence
/// boundary. The canonical registry can derive the subject but an adapter cannot
/// choose, retain, or replay it.
#[derive(Debug)]
pub(in crate::persistence) struct ReconnectActivationCapability {
    subject_key: PersistenceSubjectKey,
}

impl ReconnectActivationCapability {
    fn subject_key(&self) -> &PersistenceSubjectKey {
        &self.subject_key
    }
}

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
    let generation = NEXT_HANDOFF_GENERATION.fetch_add(1, Ordering::Relaxed);
    ReconnectHandoffToken {
        generation,
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
    pub preflights_attempted: usize,
    pub preflights_completed: usize,
    pub blocked_preflights: Vec<SliceId>,
    pub cleanups_completed: usize,
    pub aborts_completed: usize,
    pub failures: Vec<ReconnectHandoffFailure>,
}

fn active_reconnect_lease(
    world: &World,
    descriptors: &[&'static SliceDescriptor],
    subject_key: &PersistenceSubjectKey,
) -> Result<Option<SliceDispatchError>, SliceDispatchError> {
    let registry = world
        .get_resource::<PersistenceSliceRegistry>()
        .ok_or(SliceDispatchError::MissingCanonicalRegistry)?;
    Ok(descriptors.iter().find_map(|descriptor| {
        registry
            .active_subject_domain(subject_key, descriptor.write_binding.domain())
            .then_some(SliceDispatchError::DuplicateSubject {
                slice_id: descriptor.id,
                domain: descriptor.write_binding.domain(),
            })
    }))
}

fn reconnect_lease_audit_begin(world: &World) -> Result<(), SliceDispatchError> {
    world
        .get_resource::<PersistenceSliceRegistry>()
        .ok_or(SliceDispatchError::MissingCanonicalRegistry)?
        .begin_lease_audit()
}

fn reconnect_lease_audit_finish(world: &World) -> Result<Vec<LeaseMutation>, SliceDispatchError> {
    world
        .get_resource::<PersistenceSliceRegistry>()
        .ok_or(SliceDispatchError::MissingCanonicalRegistry)?
        .finish_lease_audit()
}

fn audit_successful_hydrate_lease(
    world: &World,
    descriptor: &'static SliceDescriptor,
    expected_subject: &PersistenceSubjectKey,
    mutations: &[LeaseMutation],
) -> Result<(), SliceDispatchError> {
    let expected_domain = descriptor.write_binding.domain();
    if let Some(mutation) = mutations
        .iter()
        .find(|mutation| mutation.subject != *expected_subject)
    {
        return Err(SliceDispatchError::UnexpectedHydrateSubject {
            slice_id: descriptor.id,
            domain: mutation.domain,
        });
    }
    if let Some(mutation) = mutations
        .iter()
        .find(|mutation| mutation.domain != expected_domain)
    {
        return Err(SliceDispatchError::UnexpectedHydrateLease {
            slice_id: descriptor.id,
            domain: mutation.domain,
        });
    }

    let active = world
        .get_resource::<PersistenceSliceRegistry>()
        .ok_or(SliceDispatchError::MissingCanonicalRegistry)?
        .active_subject_domains(expected_subject)?;
    if mutations.is_empty() {
        return if active.contains(&expected_domain) {
            Err(SliceDispatchError::DuplicateSubject {
                slice_id: descriptor.id,
                domain: expected_domain,
            })
        } else {
            Err(SliceDispatchError::MissingHydrateLease {
                slice_id: descriptor.id,
                domain: expected_domain,
            })
        };
    }
    if mutations.len() != 1
        || mutations[0].domain != expected_domain
        || mutations[0].subject != *expected_subject
        || mutations[0].kind != LeaseMutationKind::Acquired
    {
        let domain = mutations
            .first()
            .map_or(expected_domain, |mutation| mutation.domain);
        return Err(SliceDispatchError::UnexpectedHydrateLease {
            slice_id: descriptor.id,
            domain,
        });
    }
    if !active.contains(&expected_domain) {
        return Err(SliceDispatchError::MissingHydrateLease {
            slice_id: descriptor.id,
            domain: expected_domain,
        });
    }
    Ok(())
}

fn audit_aborted_hydrate_leases(
    world: &World,
    mutations: &[LeaseMutation],
    descriptors: &[&'static SliceDescriptor],
    expected_subject: &PersistenceSubjectKey,
    failed_descriptor: &'static SliceDescriptor,
) -> Result<(), SliceDispatchError> {
    let expected_domains: HashSet<_> = descriptors
        .iter()
        .map(|descriptor| descriptor.write_binding.domain())
        .collect();
    let mut balances: HashMap<WriteDomain, i32> = HashMap::new();
    for mutation in mutations {
        if mutation.subject != *expected_subject {
            return Err(SliceDispatchError::UnexpectedHydrateSubject {
                slice_id: failed_descriptor.id,
                domain: mutation.domain,
            });
        }
        if !expected_domains.contains(&mutation.domain) {
            return Err(SliceDispatchError::UnexpectedHydrateLease {
                slice_id: failed_descriptor.id,
                domain: mutation.domain,
            });
        }
        let balance = balances.entry(mutation.domain).or_default();
        match mutation.kind {
            LeaseMutationKind::Acquired => *balance += 1,
            LeaseMutationKind::Released => *balance -= 1,
        }
    }
    let active = world
        .get_resource::<PersistenceSliceRegistry>()
        .ok_or(SliceDispatchError::MissingCanonicalRegistry)?
        .active_subject_domains(expected_subject)?;
    if let Some(domain) = active
        .iter()
        .copied()
        .find(|domain| expected_domains.contains(domain))
    {
        let slice_id = descriptors
            .iter()
            .find(|descriptor| descriptor.write_binding.domain() == domain)
            .map_or(failed_descriptor.id, |descriptor| descriptor.id);
        return Err(SliceDispatchError::DuplicateSubject { slice_id, domain });
    }
    if let Some(domain) = active.iter().copied().next() {
        return Err(SliceDispatchError::UnexpectedHydrateLease {
            slice_id: failed_descriptor.id,
            domain,
        });
    }
    if let Some((domain, _)) = balances.iter().find(|(_, balance)| **balance != 0) {
        return Err(SliceDispatchError::UnexpectedHydrateLease {
            slice_id: failed_descriptor.id,
            domain: *domain,
        });
    }
    Ok(())
}

fn audit_rebase_leases(
    descriptor: &'static SliceDescriptor,
    mutations: &[LeaseMutation],
) -> Result<(), SliceDispatchError> {
    if let Some(mutation) = mutations.first() {
        return Err(SliceDispatchError::UnexpectedRebaseLease {
            slice_id: descriptor.id,
            domain: mutation.domain,
        });
    }
    Ok(())
}

fn reconnect_cleanup_audit(
    world: &mut World,
    descriptors: &[&'static SliceDescriptor],
    runtime_tick: u64,
    wall_unix_millis: u64,
    handoff_key: &Option<String>,
) -> Result<(usize, Vec<LeaseMutation>), SliceDispatchError> {
    reconnect_lease_audit_begin(world)?;
    let completed = cleanup_reconnect_activations(
        world,
        descriptors,
        SliceRunReason::ReconnectAbort,
        runtime_tick,
        wall_unix_millis,
        handoff_key,
    );
    let mutations = reconnect_lease_audit_finish(world)?;
    Ok((completed, mutations))
}

fn cleanup_reconnect_activations(
    world: &mut World,
    descriptors: &[&'static SliceDescriptor],
    reason: SliceRunReason,
    runtime_tick: u64,
    wall_unix_millis: u64,
    handoff_key: &Option<String>,
) -> usize {
    let context = SliceRunContext {
        reason,
        runtime_tick,
        wall_unix_millis,
        handoff_key: handoff_key.clone(),
        reconnect_activation: None,
    };
    let mut completed = 0;
    match reason {
        SliceRunReason::ReconnectCleanup => {
            for descriptor in descriptors {
                let cleanup = descriptor.reconnect_cleanup.expect(
                    "player reconnect descriptors are registry-validated with cleanup hooks",
                );
                cleanup(world, &context);
                completed += 1;
            }
        }
        SliceRunReason::ReconnectAbort => {
            for descriptor in descriptors.iter().rev() {
                let cleanup = descriptor.reconnect_cleanup.expect(
                    "player reconnect descriptors are registry-validated with cleanup hooks",
                );
                cleanup(world, &context);
                completed += 1;
            }
        }
        _ => unreachable!("reconnect cleanup helper received a non-cleanup reason"),
    }
    completed
}

/// Enforces all disconnect saves before a non-destructive global cleanup
/// preflight, cleanup commit, same-tick hydrate, and one rebase pass.
///
/// Hooks run synchronously in registry order. Saves, cleanup, hydrate, and rebase use
/// exclusive `World` access; preflights receive only `&World`, statically preventing
/// mutation of old activation state through the dispatcher contract. Any blocked or
/// failed save/preflight prevents all later phases while preserving every old
/// activation. Only after every preflight succeeds does cleanup irrevocably drop all
/// old activation leases. Hydrate and rebase fail fast; on either failure, every
/// attempted hydrate descriptor is cleaned in reverse order with `ReconnectAbort`,
/// including a hook that activated state before returning blocked or failed. Cleanup
/// has no blocked/error return channel (a panic is a descriptor contract violation),
/// and the dispatcher verifies all subject/domain leases after cleanup and abort.
/// Consuming a one-shot token makes the lifecycle exactly once for that generation.
pub(in crate::persistence) fn dispatch_reconnect_handoff(
    world: &mut World,
    token: ReconnectHandoffToken,
    clock: &impl SliceClock,
) -> Result<ReconnectHandoffReport, SliceDispatchError> {
    let descriptors: Vec<_> = world
        .get_resource::<PersistenceSliceRegistry>()
        .ok_or(SliceDispatchError::MissingCanonicalRegistry)?
        .descriptors()
        .filter(|descriptor| player_reconnect_participant(descriptor))
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
            reconnect_activation: None,
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
        let preflight = descriptor
            .reconnect_preflight
            .expect("player reconnect descriptors are registry-validated with preflight hooks");
        report.preflights_attempted += 1;
        let context = SliceRunContext {
            reason: SliceRunReason::ReconnectPreflight,
            runtime_tick,
            wall_unix_millis,
            handoff_key: handoff_key.clone(),
            reconnect_activation: None,
        };
        match preflight(world, &context) {
            Ok(SliceRunOutcome::Clean | SliceRunOutcome::Flushed) => {
                report.preflights_completed += 1;
            }
            Ok(SliceRunOutcome::SkippedBlocked) => {
                report.blocked_preflights.push(descriptor.id);
            }
            Err(error) => report.failures.push(ReconnectHandoffFailure {
                slice_id: descriptor.id,
                reason: SliceRunReason::ReconnectPreflight,
                error,
            }),
        }
    }

    if !report.blocked_preflights.is_empty() || !report.failures.is_empty() {
        return Ok(report);
    }

    report.cleanups_completed = cleanup_reconnect_activations(
        world,
        &descriptors,
        SliceRunReason::ReconnectCleanup,
        runtime_tick,
        wall_unix_millis,
        &handoff_key,
    );

    if let Some(error) = active_reconnect_lease(world, &descriptors, &subject_key)? {
        return Err(error);
    }

    let mut hydrated_descriptors = Vec::new();
    let mut handoff_mutations = Vec::new();
    for descriptor in &descriptors {
        let Some(load) = descriptor.hydrate else {
            continue;
        };
        hydrated_descriptors.push(*descriptor);
        report.loads_attempted += 1;
        reconnect_lease_audit_begin(world)?;
        let activation = ReconnectActivationCapability {
            subject_key: subject_key.clone(),
        };
        let context = SliceRunContext {
            reason: SliceRunReason::ReconnectLoad,
            runtime_tick,
            wall_unix_millis,
            handoff_key: handoff_key.clone(),
            reconnect_activation: Some(activation),
        };
        let outcome = load(world, &context);
        drop(context);
        let mutations = reconnect_lease_audit_finish(world)?;
        handoff_mutations.extend(mutations.iter().cloned());
        let lease_error = match &outcome {
            Ok(SliceRunOutcome::Clean | SliceRunOutcome::Flushed) => {
                audit_successful_hydrate_lease(world, descriptor, &subject_key, &mutations).err()
            }
            Ok(SliceRunOutcome::SkippedBlocked) | Err(_) => None,
        };
        match outcome {
            Ok(SliceRunOutcome::Clean | SliceRunOutcome::Flushed) if lease_error.is_none() => {
                report.loads_completed += 1;
            }
            Ok(SliceRunOutcome::Clean | SliceRunOutcome::Flushed) => {}
            Ok(SliceRunOutcome::SkippedBlocked) => report.blocked_loads.push(descriptor.id),
            Err(error) => report.failures.push(ReconnectHandoffFailure {
                slice_id: descriptor.id,
                reason: SliceRunReason::ReconnectLoad,
                error,
            }),
        }
        if lease_error.is_some() || !report.blocked_loads.is_empty() || !report.failures.is_empty()
        {
            reconnect_lease_audit_begin(world)?;
            report.aborts_completed = cleanup_reconnect_activations(
                world,
                &hydrated_descriptors,
                SliceRunReason::ReconnectAbort,
                runtime_tick,
                wall_unix_millis,
                &handoff_key,
            );
            handoff_mutations.extend(reconnect_lease_audit_finish(world)?);
            audit_aborted_hydrate_leases(
                world,
                &handoff_mutations,
                &hydrated_descriptors,
                &subject_key,
                descriptor,
            )?;
            if let Some(error) = lease_error {
                return Err(error);
            }
            return Ok(report);
        }
    }

    for descriptor in &descriptors {
        let Some(rebase) = descriptor.rebase else {
            continue;
        };
        report.rebases_attempted += 1;
        reconnect_lease_audit_begin(world)?;
        let context = SliceRunContext {
            reason: SliceRunReason::Rebase,
            runtime_tick,
            wall_unix_millis,
            handoff_key: handoff_key.clone(),
            reconnect_activation: None,
        };
        let outcome = rebase(world, &context);
        let mutations = reconnect_lease_audit_finish(world)?;
        handoff_mutations.extend(mutations.iter().cloned());
        let lease_error = audit_rebase_leases(descriptor, &mutations).err();
        match outcome {
            Ok(SliceRunOutcome::Clean | SliceRunOutcome::Flushed) if lease_error.is_none() => {
                report.rebases_completed += 1;
            }
            Ok(SliceRunOutcome::Clean | SliceRunOutcome::Flushed) => {}
            Ok(SliceRunOutcome::SkippedBlocked) => report.blocked_rebases.push(descriptor.id),
            Err(error) => report.failures.push(ReconnectHandoffFailure {
                slice_id: descriptor.id,
                reason: SliceRunReason::Rebase,
                error,
            }),
        }
        if let Some(error) = lease_error {
            let (completed, cleanup_mutations) = reconnect_cleanup_audit(
                world,
                &hydrated_descriptors,
                runtime_tick,
                wall_unix_millis,
                &handoff_key,
            )?;
            report.aborts_completed = completed;
            handoff_mutations.extend(cleanup_mutations);
            audit_aborted_hydrate_leases(
                world,
                &handoff_mutations,
                &hydrated_descriptors,
                &subject_key,
                descriptor,
            )?;
            return Err(error);
        }
        if !report.blocked_rebases.is_empty() || !report.failures.is_empty() {
            let (completed, cleanup_mutations) = reconnect_cleanup_audit(
                world,
                &hydrated_descriptors,
                runtime_tick,
                wall_unix_millis,
                &handoff_key,
            )?;
            report.aborts_completed = completed;
            handoff_mutations.extend(cleanup_mutations);
            audit_aborted_hydrate_leases(
                world,
                &handoff_mutations,
                &hydrated_descriptors,
                &subject_key,
                descriptor,
            )?;
            return Ok(report);
        }
    }

    Ok(report)
}

pub struct SliceLoad<T, E> {
    state: SliceLoadInner<T, E>,
}

enum SliceLoadInner<T, E> {
    Missing,
    Loaded(T),
    Failed(E),
}

impl<T, E> fmt::Debug for SliceLoad<T, E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SliceLoad")
            .field("status", &self.status())
            .finish()
    }
}

/// Payload-free read-only view of durable load provenance.
///
/// The wrapper deliberately exposes no pattern-matchable payload or consuming getter:
///
/// ```compile_fail
/// use bong_server::persistence::slice::SliceLoad;
///
/// let load = SliceLoad::<u32, String>::failed("corrupt".to_owned());
/// let SliceLoad::Failed(cause) = load;
/// drop(cause);
/// ```
///
/// ```compile_fail
/// use bong_server::persistence::slice::SliceLoad;
///
/// let load = SliceLoad::<u32, String>::failed("corrupt".to_owned());
/// let _cause = load.state;
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliceLoadStatus {
    Missing,
    Loaded,
    Failed,
}

impl<T, E> SliceLoad<T, E> {
    pub const fn missing() -> Self {
        Self {
            state: SliceLoadInner::Missing,
        }
    }

    pub const fn loaded(value: T) -> Self {
        Self {
            state: SliceLoadInner::Loaded(value),
        }
    }

    pub const fn failed(error: E) -> Self {
        Self {
            state: SliceLoadInner::Failed(error),
        }
    }

    pub const fn status(&self) -> SliceLoadStatus {
        match &self.state {
            SliceLoadInner::Missing => SliceLoadStatus::Missing,
            SliceLoadInner::Loaded(_) => SliceLoadStatus::Loaded,
            SliceLoadInner::Failed(_) => SliceLoadStatus::Failed,
        }
    }

    pub const fn is_missing(&self) -> bool {
        matches!(&self.state, SliceLoadInner::Missing)
    }

    pub const fn is_loaded(&self) -> bool {
        matches!(&self.state, SliceLoadInner::Loaded(_))
    }

    pub const fn is_failed(&self) -> bool {
        matches!(&self.state, SliceLoadInner::Failed(_))
    }
}

/// Durable provenance retained beside the runtime value.
#[derive(Debug, Clone, PartialEq, Eq)]
enum GuardedLoadState<E> {
    Missing,
    Loaded,
    Failed(E),
}

/// Activation failures returned only by persistence-private registry transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::persistence) enum SliceActivationError<E> {
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
    pub(in crate::persistence) const fn slice_id(&self) -> SliceId {
        match self {
            Self::LoadFailed { slice_id, .. }
            | Self::DuplicateSubject { slice_id, .. }
            | Self::PoisonedSubjectRegistry { slice_id } => *slice_id,
        }
    }

    pub(in crate::persistence) fn cause(&self) -> Option<&E> {
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

    pub(in crate::persistence) fn as_str(&self) -> &str {
        &self.0
    }
}

/// Opaque identity shared only by state derived from one active durable subject.
#[derive(Debug)]
struct LeaseToken {
    lease_book: Arc<LeaseBook>,
    subject: PersistenceSubjectKey,
    domain: WriteDomain,
    released: AtomicBool,
}

impl LeaseToken {
    fn release(&self) {
        if self.released.swap(true, Ordering::AcqRel) {
            return;
        }
        let Ok(mut subjects) = self.lease_book.active_subjects.lock() else {
            return;
        };
        let Some(domains) = subjects.get_mut(&self.subject) else {
            return;
        };
        domains.remove(&self.domain);
        if domains.is_empty() {
            subjects.remove(&self.subject);
        }
        self.lease_book
            .record(self.domain, &self.subject, LeaseMutationKind::Released);
    }
}

impl Drop for LeaseToken {
    fn drop(&mut self) {
        self.release();
    }
}

#[derive(Debug, Clone)]
struct SliceSubject(Arc<LeaseToken>);

impl SliceSubject {
    fn new(
        lease_book: Arc<LeaseBook>,
        subject: PersistenceSubjectKey,
        domain: WriteDomain,
    ) -> Self {
        Self(Arc::new(LeaseToken {
            lease_book,
            subject,
            domain,
            released: AtomicBool::new(false),
        }))
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
pub struct GuardedSlice<T, E> {
    value: T,
    load_state: GuardedLoadState<E>,
    binding: WriteBinding,
    write_ordering: WriteOrdering,
    subject_key: PersistenceSubjectKey,
    subject: SliceSubject,
    initial_revision: DirtyRevision,
    persistence_state_issued: bool,
}

impl<T: fmt::Debug, E> fmt::Debug for GuardedSlice<T, E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GuardedSlice")
            .field("value", &self.value)
            .field("load_status", &self.load_status())
            .field("binding", &self.binding)
            .field("write_ordering", &self.write_ordering)
            .field("initial_revision", &self.initial_revision)
            .field("persistence_state_issued", &self.persistence_state_issued)
            .finish()
    }
}

impl<T, E> SliceLoad<T, E> {
    fn refuse_startup<R>(self, slice_id: SliceId) -> Result<R, SliceActivationError<E>> {
        match self.state {
            SliceLoadInner::Failed(cause) => {
                Err(SliceActivationError::LoadFailed { slice_id, cause })
            }
            SliceLoadInner::Missing | SliceLoadInner::Loaded(_) => {
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
        let (value, load_state) = match self.state {
            SliceLoadInner::Missing => (on_missing(), GuardedLoadState::Missing),
            SliceLoadInner::Loaded(value) => (value, GuardedLoadState::Loaded),
            SliceLoadInner::Failed(error) => {
                let value = on_failed(&error);
                (value, GuardedLoadState::Failed(error))
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

    pub fn load_status(&self) -> SliceLoadStatus {
        match &self.load_state {
            GuardedLoadState::Missing => SliceLoadStatus::Missing,
            GuardedLoadState::Loaded => SliceLoadStatus::Loaded,
            GuardedLoadState::Failed(_) => SliceLoadStatus::Failed,
        }
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
        if matches!(&self.load_state, GuardedLoadState::Failed(_)) {
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
        if matches!(&self.load_state, GuardedLoadState::Failed(_)) {
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

    pub(in crate::persistence) fn subject_key(&self) -> &PersistenceSubjectKey {
        &self.subject_key
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
///
/// It owns the only transaction reference exposed to the adapter. The fence checks
/// `executed` after the callback returns, so a callback cannot report success or
/// replay evidence from an earlier rolled-back request.
pub struct DurableWriteRequest<'transaction, 'connection, 'snapshot, P> {
    transaction: &'transaction Transaction<'connection>,
    payload: &'snapshot P,
    subject_key: &'snapshot PersistenceSubjectKey,
    binding: WriteBinding,
    expected_persisted_revision: DirtyRevision,
    write_revision: DirtyRevision,
    outlet: WriteOutlet,
    ordering: WriteOrdering,
    executed: std::cell::Cell<bool>,
}

impl<P> DurableWriteRequest<'_, '_, '_, P> {
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

    fn requires_canonical_database(&self) -> Result<(), DurableWriteExecuteError> {
        let schemas = self
            .transaction
            .prepare("PRAGMA database_list")
            .map_err(DurableWriteExecuteError::Sql)?
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(DurableWriteExecuteError::Sql)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(DurableWriteExecuteError::Sql)?;
        if schemas == ["main"] {
            Ok(())
        } else {
            Err(DurableWriteExecuteError::Proof(
                DurableWriteProofError::NonCanonicalDatabase,
            ))
        }
    }

    fn schema_version(&self) -> Result<i64, DurableWriteExecuteError> {
        self.transaction
            .query_row("PRAGMA schema_version", [], |row| row.get(0))
            .map_err(DurableWriteExecuteError::Sql)
    }

    fn requires_single_statement(&self, sql: &str) -> Result<(), DurableWriteExecuteError> {
        if sql.is_empty() {
            return Err(DurableWriteExecuteError::Proof(
                DurableWriteProofError::ReadOnlyStatement,
            ));
        }
        if sql.as_bytes().contains(&0) {
            return Err(DurableWriteExecuteError::Proof(
                DurableWriteProofError::MultipleStatements,
            ));
        }

        // SAFETY: the fence owns this live connection while it preflights SQL without stepping it.
        let database = unsafe { self.transaction.handle() };
        let mut remaining = sql.as_bytes();
        let mut found_statement = false;
        loop {
            let (has_statement, tail_offset) = Self::prepare_sql_tail(database, remaining)?;
            if has_statement {
                if found_statement {
                    return Err(DurableWriteExecuteError::Proof(
                        DurableWriteProofError::MultipleStatements,
                    ));
                }
                found_statement = true;
            }
            if tail_offset == remaining.len() {
                break;
            }
            remaining = &remaining[tail_offset..];
        }

        if found_statement {
            Ok(())
        } else {
            Err(DurableWriteExecuteError::Proof(
                DurableWriteProofError::ReadOnlyStatement,
            ))
        }
    }

    fn prepare_sql_tail(
        database: *mut ffi::sqlite3,
        sql: &[u8],
    ) -> Result<(bool, usize), DurableWriteExecuteError> {
        let length = std::os::raw::c_int::try_from(sql.len()).map_err(|_| {
            DurableWriteExecuteError::Sql(rusqlite::Error::SqliteFailure(
                ffi::Error::new(ffi::SQLITE_TOOBIG),
                None,
            ))
        })?;
        let mut statement = std::ptr::null_mut();
        let mut tail: *const std::os::raw::c_char = std::ptr::null();
        // SAFETY: `sql` stays live throughout prepare, and the returned statement is finalized
        // before this helper returns without being stepped or bound.
        let result = unsafe {
            ffi::sqlite3_prepare_v2(
                database,
                sql.as_ptr().cast(),
                length,
                &mut statement,
                &mut tail,
            )
        };
        let has_statement = !statement.is_null();
        if has_statement {
            // SAFETY: SQLite returned this statement for `database` in the preceding call.
            unsafe { ffi::sqlite3_finalize(statement) };
        }
        if result != ffi::SQLITE_OK {
            return Err(DurableWriteExecuteError::Sql(
                rusqlite::Error::SqliteFailure(ffi::Error::new(result), None),
            ));
        }

        let tail_offset = if tail.is_null() {
            sql.len()
        } else {
            (tail as usize)
                .checked_sub(sql.as_ptr() as usize)
                .filter(|offset| *offset <= sql.len())
                .ok_or_else(|| {
                    DurableWriteExecuteError::Sql(rusqlite::Error::SqliteFailure(
                        ffi::Error::new(ffi::SQLITE_MISUSE),
                        None,
                    ))
                })?
        };
        Ok((
            has_statement,
            if tail_offset == 0 || tail_offset >= sql.len() {
                sql.len()
            } else {
                tail_offset
            },
        ))
    }

    fn execute_durable<Q: Params>(
        &self,
        sql: &str,
        params: Q,
    ) -> Result<(usize, u64), DurableWriteExecuteError> {
        self.requires_single_statement(sql)?;
        self.requires_canonical_database()?;
        let schema_version_before = self.schema_version()?;
        let statement = self
            .transaction
            .prepare(sql)
            .map_err(DurableWriteExecuteError::Sql)?;
        if statement.readonly() {
            return Err(DurableWriteExecuteError::Proof(
                DurableWriteProofError::ReadOnlyStatement,
            ));
        }
        drop(statement);
        // SAFETY: the fence owns this live connection for the read-only SQLite query.
        let total_changes_before =
            unsafe { ffi::sqlite3_total_changes64(self.transaction.handle()) as u64 };
        let affected_rows = self
            .transaction
            .execute(sql, params)
            .map_err(DurableWriteExecuteError::Sql)?;
        let schema_version_after = self.schema_version()?;
        if schema_version_after != schema_version_before {
            return Err(DurableWriteExecuteError::Proof(
                DurableWriteProofError::SchemaMutationDetected,
            ));
        }
        self.requires_canonical_database()?;
        Ok((affected_rows, total_changes_before))
    }

    /// Executes a serialized durable statement through the fence-owned transaction.
    pub(in crate::persistence) fn execute_serialized<Q: Params>(
        &self,
        sql: &str,
        params: Q,
    ) -> Result<(), DurableWriteExecuteError> {
        if self.ordering != WriteOrdering::Serialized {
            return Err(DurableWriteExecuteError::Proof(
                DurableWriteProofError::WrongOrdering {
                    expected: WriteOrdering::Serialized,
                    actual: self.ordering,
                },
            ));
        }
        let (affected_rows, total_changes_before) = self.execute_durable(sql, params)?;
        if affected_rows == 0 {
            return Err(DurableWriteExecuteError::Proof(
                DurableWriteProofError::SerializedWriteRejected,
            ));
        }
        // SAFETY: the fence owns this live connection for the read-only SQLite query.
        let total_changes_after =
            unsafe { ffi::sqlite3_total_changes64(self.transaction.handle()) as u64 };
        if total_changes_after <= total_changes_before {
            return Err(DurableWriteExecuteError::Proof(
                DurableWriteProofError::MissingCurrentTransactionWrite,
            ));
        }
        self.executed.set(true);
        Ok(())
    }

    /// Executes one revision-CAS durable statement through the fence-owned transaction.
    pub(in crate::persistence) fn execute_cas<Q: Params>(
        &self,
        sql: &str,
        params: Q,
    ) -> Result<(), DurableWriteExecuteError> {
        if self.ordering != WriteOrdering::PersistedRevisionCas {
            return Err(DurableWriteExecuteError::Proof(
                DurableWriteProofError::WrongOrdering {
                    expected: WriteOrdering::PersistedRevisionCas,
                    actual: self.ordering,
                },
            ));
        }
        let (affected_rows, total_changes_before) = self.execute_durable(sql, params)?;
        if affected_rows != 1 {
            return Err(DurableWriteExecuteError::Proof(
                DurableWriteProofError::CasRejected { affected_rows },
            ));
        }
        // SAFETY: the fence owns this live connection for the read-only SQLite query.
        let total_changes_after =
            unsafe { ffi::sqlite3_total_changes64(self.transaction.handle()) as u64 };
        if total_changes_after <= total_changes_before {
            return Err(DurableWriteExecuteError::Proof(
                DurableWriteProofError::MissingCurrentTransactionWrite,
            ));
        }
        self.executed.set(true);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::persistence) enum DurableWriteProofError {
    WrongOrdering {
        expected: WriteOrdering,
        actual: WriteOrdering,
    },
    SerializedWriteRejected,
    ReadOnlyStatement,
    MissingCurrentTransactionWrite,
    NonCanonicalDatabase,
    MultipleStatements,
    SchemaMutationDetected,
    CasRejected {
        affected_rows: usize,
    },
}

#[derive(Debug, PartialEq)]
pub(in crate::persistence) enum DurableWriteExecuteError {
    Sql(rusqlite::Error),
    Proof(DurableWriteProofError),
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

#[derive(Debug, PartialEq)]
pub enum DurableCommitError<E> {
    WrongBinding(WriteBindingMismatch),
    WrongSubject,
    StaleRevision {
        persisted: DirtyRevision,
        attempted: DirtyRevision,
    },
    BeginTransaction(rusqlite::Error),
    WriteFailed(E),
    MissingDurableWrite,
    CommitFailed(rusqlite::Error),
}

/// Durable revision fence and receipt minter for one registered write authority.
#[derive(Debug, PartialEq, Eq)]
pub struct PersistedRevisionFence {
    binding: WriteBinding,
    subject: SliceSubject,
    ordering: WriteOrdering,
    persisted: DirtyRevision,
}

impl PersistedRevisionFence {
    pub const fn binding(&self) -> WriteBinding {
        self.binding
    }

    pub fn persisted_revision(&self) -> DirtyRevision {
        self.persisted
    }

    /// The adapter can only use request methods against this transaction; the fence
    /// requires one such method to succeed before it commits and mints a receipt.
    #[allow(dead_code)]
    pub(in crate::persistence) fn commit<P, E>(
        &mut self,
        connection: &mut Connection,
        snapshot: DirtySnapshot<P>,
        write: impl FnOnce(&DurableWriteRequest<'_, '_, '_, P>) -> Result<(), E>,
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

        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(DurableCommitError::BeginTransaction)?;
        let request = DurableWriteRequest {
            transaction: &transaction,
            payload: &snapshot.payload,
            subject_key: &snapshot.subject_key,
            binding: self.binding,
            expected_persisted_revision: self.persisted,
            write_revision: snapshot.revision,
            outlet: snapshot.outlet,
            ordering: self.ordering,
            executed: std::cell::Cell::new(false),
        };
        write(&request).map_err(DurableCommitError::WriteFailed)?;
        if !request.executed.get() {
            return Err(DurableCommitError::MissingDurableWrite);
        }
        transaction
            .commit()
            .map_err(DurableCommitError::CommitFailed)?;

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
    const EXTRA_TEST_BINDING: WriteBinding = WriteBinding::new(
        WriteDomain::new("test.player.extra"),
        WriteAuthority::new("test.player.extra.writer"),
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
        reconnect_preflight: None,
        reconnect_cleanup: None,
        rebase: None,
        disconnect_save: None,
        shutdown_flush: None,
    };

    fn noop_rebase(_world: &mut World, _context: &SliceRunContext) -> SliceRunResult {
        Ok(SliceRunOutcome::Clean)
    }

    fn noop_preflight(_world: &World, context: &SliceRunContext) -> SliceRunResult {
        assert_eq!(context.reason, SliceRunReason::ReconnectPreflight);
        Ok(SliceRunOutcome::Clean)
    }

    fn noop_cleanup(_world: &mut World, _context: &SliceRunContext) {}

    fn basic_descriptor(id: &'static str, order: u16) -> SliceDescriptor {
        SliceDescriptor {
            id: SliceId::new(id),
            scope: SliceScope::PlayerEntity,
            order,
            load_failure: LoadFailurePolicy::BlockWrites,
            time_basis: TimeBasis::None,
            write_binding: WriteBinding::new(WriteDomain::new(id), WriteAuthority::new(id)),
            write_ordering: WriteOrdering::Serialized,
            autosave: AutosavePolicy::OnChange,
            hydrate: None,
            reconnect_preflight: None,
            reconnect_cleanup: None,
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
            .activate_test_subject(
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
    fn lease_audit_rejects_nested_and_unmatched_lifecycle() {
        let registry = PersistenceSliceRegistry::empty();

        assert_eq!(
            registry.finish_lease_audit(),
            Err(SliceDispatchError::LeaseAuditNotActive),
            "finishing without begin must fail closed instead of fabricating an empty journal"
        );
        registry.begin_lease_audit().unwrap();
        assert_eq!(
            registry.begin_lease_audit(),
            Err(SliceDispatchError::LeaseAuditAlreadyActive),
            "a second begin must not overwrite the first journal"
        );
        assert!(registry
            .finish_lease_audit()
            .expect("the active journal must finish exactly once")
            .is_empty());
        assert_eq!(
            registry.finish_lease_audit(),
            Err(SliceDispatchError::LeaseAuditNotActive),
            "finishing twice must expose a residual lifecycle violation"
        );

        registry.begin_lease_audit().unwrap();
        assert!(registry
            .finish_lease_audit()
            .expect("a fresh journal must remain usable after a clean finish")
            .is_empty());
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
        let one_cadence = Box::leak(Box::new(SliceDescriptor {
            autosave: AutosavePolicy::EveryTicks(1),
            ..basic_descriptor("player.one_cadence", 41)
        }));
        let max_cadence = Box::leak(Box::new(SliceDescriptor {
            autosave: AutosavePolicy::EveryTicks(u64::MAX),
            ..basic_descriptor("player.max_cadence", 42)
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
        let rebase_without_hydrate = Box::leak(Box::new(SliceDescriptor {
            time_basis: TimeBasis::None,
            reconnect_preflight: Some(noop_preflight),
            reconnect_cleanup: Some(noop_cleanup),
            rebase: Some(noop_rebase),
            ..basic_descriptor("player.rebase_without_hydrate", 51)
        }));
        let invalid_domain = Box::leak(Box::new(SliceDescriptor {
            write_binding: WriteBinding::new(
                WriteDomain::new("Player Core"),
                WriteAuthority::new("test.player.writer"),
            ),
            ..basic_descriptor("player.invalid_domain", 55)
        }));
        let invalid_authority = Box::leak(Box::new(SliceDescriptor {
            write_binding: WriteBinding::new(
                WriteDomain::new("player.core"),
                WriteAuthority::new("Player Writer"),
            ),
            ..basic_descriptor("player.invalid_authority", 56)
        }));
        let valid_rebase = Box::leak(Box::new(SliceDescriptor {
            time_basis: TimeBasis::RemainingLogicalTicks,
            hydrate: Some(noop_rebase),
            reconnect_preflight: Some(noop_preflight),
            reconnect_cleanup: Some(noop_cleanup),
            rebase: Some(noop_rebase),
            ..basic_descriptor("player.valid_rebase", 60)
        }));
        let world_missing_hydrate = Box::leak(Box::new(SliceDescriptor {
            scope: SliceScope::WorldResource,
            time_basis: TimeBasis::WallDeadline,
            rebase: Some(noop_rebase),
            ..basic_descriptor("world.missing_hydrate", 65)
        }));
        let missing_reconnect_hooks = Box::leak(Box::new(SliceDescriptor {
            time_basis: TimeBasis::None,
            hydrate: Some(noop_rebase),
            disconnect_save: Some(noop_rebase),
            ..basic_descriptor("player.missing_reconnect_hooks", 70)
        }));
        let missing_cleanup = Box::leak(Box::new(SliceDescriptor {
            time_basis: TimeBasis::None,
            hydrate: Some(noop_rebase),
            disconnect_save: Some(noop_rebase),
            reconnect_preflight: Some(noop_preflight),
            ..basic_descriptor("player.missing_cleanup", 71)
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
        assert_eq!(
            registry.register(one_cadence),
            Ok(()),
            "the smallest positive autosave cadence must be accepted"
        );
        assert_eq!(
            registry.register(max_cadence),
            Ok(()),
            "u64::MAX must remain a valid positive autosave cadence"
        );
        assert!(matches!(
            registry.register(missing_hydrate),
            Err(SliceRegistryError::MissingHydrateHook { .. })
        ));
        assert!(matches!(
            registry.register(missing_rebase),
            Err(SliceRegistryError::MissingRebaseHook { .. })
        ));
        assert_eq!(
            registry.register(rebase_without_hydrate),
            Err(SliceRegistryError::MissingHydrateHook {
                slice_id: rebase_without_hydrate.id,
            })
        );
        assert!(matches!(
            registry.register(invalid_domain),
            Err(SliceRegistryError::InvalidWriteDomain { .. })
        ));
        assert!(matches!(
            registry.register(invalid_authority),
            Err(SliceRegistryError::InvalidWriteAuthority { .. })
        ));
        assert!(matches!(
            registry.register(world_missing_hydrate),
            Err(SliceRegistryError::MissingHydrateHook { .. })
        ));
        assert!(matches!(
            registry.register(missing_reconnect_hooks),
            Err(SliceRegistryError::MissingReconnectPreflightHook { .. })
        ));
        assert!(matches!(
            registry.register(missing_cleanup),
            Err(SliceRegistryError::MissingReconnectCleanupHook { .. })
        ));
        assert_eq!(registry.register(valid_rebase), Ok(()));
    }

    #[test]
    fn registry_rejects_every_second_descriptor_for_one_write_domain() {
        let first = Box::leak(Box::new(SliceDescriptor {
            write_binding: TEST_BINDING,
            ..basic_descriptor("player.core", 10)
        }));
        let same_authority_and_ordering = Box::leak(Box::new(SliceDescriptor {
            write_binding: TEST_BINDING,
            ..basic_descriptor("player.inventory", 20)
        }));
        let different_authority = Box::leak(Box::new(SliceDescriptor {
            write_binding: WriteBinding::new(
                TEST_BINDING.domain(),
                WriteAuthority::new("test.player.competing_writer"),
            ),
            ..basic_descriptor("player.craft", 30)
        }));
        let different_ordering = Box::leak(Box::new(SliceDescriptor {
            write_binding: TEST_BINDING,
            write_ordering: WriteOrdering::PersistedRevisionCas,
            ..basic_descriptor("player.mail", 40)
        }));
        let mut registry = PersistenceSliceRegistry::empty();
        registry.register(first).unwrap();

        for duplicate in [
            same_authority_and_ordering,
            different_authority,
            different_ordering,
        ] {
            assert_eq!(
                registry.register(duplicate),
                Err(SliceRegistryError::DuplicateWriteDomain {
                    domain: TEST_BINDING.domain(),
                    first_slice_id: first.id,
                    duplicate_slice_id: duplicate.id,
                })
            );
            assert_eq!(
                registry
                    .descriptors()
                    .map(|descriptor| descriptor.id)
                    .collect::<Vec<_>>(),
                vec![first.id],
                "a rejected duplicate must not remain registered"
            );
        }
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

    #[derive(Debug)]
    struct HandoffActivation {
        _guarded: GuardedSlice<u32, &'static str>,
        _tracker: DirtyTracker,
        _fence: PersistedRevisionFence,
    }

    #[derive(Debug, Default)]
    struct HandoffTrace {
        events: Vec<(SliceRunReason, u64, u64, String)>,
        activations: HashMap<SliceId, HandoffActivation>,
        fail_save: bool,
        block_save: bool,
        fail_load: bool,
        block_load: bool,
        fail_preflight: bool,
        block_preflight: bool,
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

    fn trace_handoff_load(
        world: &mut World,
        context: &SliceRunContext,
        slice_id: SliceId,
    ) -> SliceRunResult {
        {
            let mut trace = world.resource_mut::<HandoffTrace>();
            trace.events.push((
                context.reason,
                context.runtime_tick,
                context.wall_unix_millis,
                context.handoff_key.clone().unwrap(),
            ));
            if trace.fail_load {
                return Err(SliceRunError::new("reconnect hydrate failed"));
            }
            if trace.block_load {
                return Ok(SliceRunOutcome::SkippedBlocked);
            }
        }
        let activation = make_trace_activation(world, context, slice_id)?;
        world
            .resource_mut::<HandoffTrace>()
            .activations
            .insert(slice_id, activation);
        Ok(SliceRunOutcome::Clean)
    }

    fn make_trace_activation(
        world: &mut World,
        context: &SliceRunContext,
        slice_id: SliceId,
    ) -> Result<HandoffActivation, SliceRunError> {
        world.resource_scope(
            |_world, registry: valence::prelude::Mut<PersistenceSliceRegistry>| {
                let mut guarded = registry
                    .activate(
                        SliceLoad::<u32, &'static str>::loaded(9),
                        slice_id,
                        context.reconnect_activation()?,
                        DirtyRevision::new(4),
                        || 0,
                        |_| 0,
                    )
                    .map_err(|_| SliceRunError::new("handoff activation rejected"))?;
                let (tracker, fence) = guarded
                    .restore_persistence_state()
                    .map_err(|_| SliceRunError::new("handoff persistence state already issued"))?;
                Ok(HandoffActivation {
                    _guarded: guarded,
                    _tracker: tracker,
                    _fence: fence,
                })
            },
        )
    }

    fn handoff_load_clock(world: &mut World, context: &SliceRunContext) -> SliceRunResult {
        trace_handoff_load(world, context, SliceId::new("player.clock_once"))
    }

    fn handoff_load_participant(world: &mut World, context: &SliceRunContext) -> SliceRunResult {
        trace_handoff_load(world, context, SliceId::new("player.reconnect"))
    }

    fn handoff_load_first(world: &mut World, context: &SliceRunContext) -> SliceRunResult {
        trace_handoff_load(world, context, SliceId::new("player.handoff_first"))
    }

    fn handoff_load_second(world: &mut World, context: &SliceRunContext) -> SliceRunResult {
        trace_handoff_load(world, context, SliceId::new("player.handoff_second"))
    }

    fn handoff_load_activation_first(
        world: &mut World,
        context: &SliceRunContext,
    ) -> SliceRunResult {
        trace_handoff_load(world, context, SliceId::new("player.activation_first"))
    }

    fn handoff_load_activation_second(
        world: &mut World,
        context: &SliceRunContext,
    ) -> SliceRunResult {
        trace_handoff_load(world, context, SliceId::new("player.activation_second"))
    }

    fn handoff_preflight(world: &World, context: &SliceRunContext) -> SliceRunResult {
        assert_eq!(context.reason, SliceRunReason::ReconnectPreflight);
        let trace = world.resource::<HandoffTrace>();
        if trace.fail_preflight {
            Err(SliceRunError::new("reconnect preflight failed"))
        } else if trace.block_preflight {
            Ok(SliceRunOutcome::SkippedBlocked)
        } else {
            Ok(SliceRunOutcome::Clean)
        }
    }

    fn handoff_cleanup_clock(world: &mut World, context: &SliceRunContext) {
        handoff_cleanup_slice(world, context, SliceId::new("player.clock_once"));
    }

    fn handoff_cleanup_participant(world: &mut World, context: &SliceRunContext) {
        handoff_cleanup_slice(world, context, SliceId::new("player.reconnect"));
    }

    fn handoff_cleanup_first(world: &mut World, context: &SliceRunContext) {
        handoff_cleanup_slice(world, context, SliceId::new("player.handoff_first"));
    }

    fn handoff_cleanup_second(world: &mut World, context: &SliceRunContext) {
        handoff_cleanup_slice(world, context, SliceId::new("player.handoff_second"));
    }

    fn handoff_cleanup_slice(world: &mut World, context: &SliceRunContext, slice_id: SliceId) {
        assert!(matches!(
            context.reason,
            SliceRunReason::ReconnectCleanup | SliceRunReason::ReconnectAbort
        ));
        let mut trace = world.resource_mut::<HandoffTrace>();
        trace.events.push((
            context.reason,
            context.runtime_tick,
            context.wall_unix_millis,
            context.handoff_key.clone().unwrap(),
        ));
        trace.activations.remove(&slice_id);
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
            hydrate: Some(handoff_load_clock),
            reconnect_preflight: Some(handoff_preflight),
            reconnect_cleanup: Some(handoff_cleanup_clock),
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
            .all(|event| {
                event.1 == 400 && event.2 == 49_999 && event.3 == "offline:clock_once"
            }));
    }

    #[test]
    fn reconnect_dispatch_skips_non_participating_and_world_scoped_descriptors() {
        let non_participant = Box::leak(Box::new(SliceDescriptor {
            shutdown_flush: Some(clean_hook),
            ..basic_descriptor("player.shutdown_only", 5)
        }));
        let world_scoped = Box::leak(Box::new(SliceDescriptor {
            scope: SliceScope::WorldResource,
            hydrate: Some(handoff_load_participant),
            reconnect_preflight: Some(noop_preflight),
            reconnect_cleanup: Some(handoff_cleanup_participant),
            rebase: Some(handoff_rebase),
            disconnect_save: Some(handoff_save),
            write_binding: EXTRA_TEST_BINDING,
            ..basic_descriptor("world.reconnect_shaped", 7)
        }));
        let participant = Box::leak(Box::new(SliceDescriptor {
            hydrate: Some(handoff_load_participant),
            reconnect_preflight: Some(noop_preflight),
            reconnect_cleanup: Some(handoff_cleanup_participant),
            disconnect_save: Some(handoff_save),
            ..basic_descriptor("player.reconnect", 10)
        }));
        let mut registry = PersistenceSliceRegistry::empty();
        registry.register(non_participant).unwrap();
        registry.register(world_scoped).unwrap();
        registry.register(participant).unwrap();
        let mut world = World::new();
        world.insert_resource(registry);
        world.insert_resource(HandoffTrace::default());
        let clock = FixedClock {
            runtime_tick: 400,
            wall_unix_millis: 49_999,
        };

        let report =
            dispatch_reconnect_handoff(&mut world, token("player:participant"), &clock).unwrap();

        assert_eq!(report.saves_attempted, 1);
        assert_eq!(report.preflights_attempted, 1);
        assert_eq!(report.cleanups_completed, 1);
        assert_eq!(report.loads_attempted, 1);
        assert_eq!(report.rebases_attempted, 0);
    }

    #[test]
    fn reconnect_handoff_enforces_same_tick_all_saves_before_any_load() {
        let first = Box::leak(Box::new(SliceDescriptor {
            time_basis: TimeBasis::RemainingLogicalTicks,
            rebase: Some(handoff_rebase),
            hydrate: Some(handoff_load_first),
            reconnect_preflight: Some(handoff_preflight),
            reconnect_cleanup: Some(handoff_cleanup_first),
            disconnect_save: Some(handoff_save),
            ..basic_descriptor("player.handoff_first", 10)
        }));
        let second = Box::leak(Box::new(SliceDescriptor {
            time_basis: TimeBasis::RemainingLogicalTicks,
            rebase: Some(handoff_rebase),
            hydrate: Some(handoff_load_second),
            reconnect_preflight: Some(handoff_preflight),
            reconnect_cleanup: Some(handoff_cleanup_second),
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
                preflights_attempted: 2,
                preflights_completed: 2,
                blocked_preflights: Vec::new(),
                cleanups_completed: 2,
                aborts_completed: 0,
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
                SliceRunReason::ReconnectCleanup,
                SliceRunReason::ReconnectCleanup,
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
        world.resource_mut::<HandoffTrace>().block_preflight = true;
        let report = dispatch_reconnect_handoff(&mut world, token("offline:test"), &clock).unwrap();
        assert_eq!(report.preflights_attempted, 2);
        assert_eq!(report.preflights_completed, 0);
        assert_eq!(
            report.blocked_preflights,
            vec![
                SliceId::new("player.handoff_first"),
                SliceId::new("player.handoff_second"),
            ]
        );
        assert_eq!(report.cleanups_completed, 0);
        assert_eq!(report.loads_attempted, 0);
        assert!(report.failures.is_empty());

        world.resource_mut::<HandoffTrace>().events.clear();
        world.resource_mut::<HandoffTrace>().block_preflight = false;
        world.resource_mut::<HandoffTrace>().fail_preflight = true;
        let report = dispatch_reconnect_handoff(&mut world, token("offline:test"), &clock).unwrap();
        assert_eq!(report.preflights_attempted, 2);
        assert_eq!(report.preflights_completed, 0);
        assert_eq!(report.cleanups_completed, 0);
        assert_eq!(report.loads_attempted, 0);
        assert_eq!(report.failures.len(), 2);
        assert!(report
            .failures
            .iter()
            .all(|failure| failure.reason == SliceRunReason::ReconnectPreflight));

        world.resource_mut::<HandoffTrace>().events.clear();
        world.resource_mut::<HandoffTrace>().fail_preflight = false;
        world.resource_mut::<HandoffTrace>().block_load = true;
        let report = dispatch_reconnect_handoff(&mut world, token("offline:test"), &clock).unwrap();
        assert_eq!(report.loads_attempted, 1);
        assert_eq!(report.loads_completed, 0);
        assert_eq!(
            report.blocked_loads,
            vec![SliceId::new("player.handoff_first")]
        );
        assert_eq!(report.rebases_attempted, 0);
        assert_eq!(report.aborts_completed, 1);
        assert_eq!(
            world
                .resource::<HandoffTrace>()
                .events
                .last()
                .map(|event| event.0),
            Some(SliceRunReason::ReconnectAbort)
        );

        world.resource_mut::<HandoffTrace>().events.clear();
        world.resource_mut::<HandoffTrace>().block_load = false;
        world.resource_mut::<HandoffTrace>().fail_load = true;
        let report = dispatch_reconnect_handoff(&mut world, token("offline:test"), &clock).unwrap();
        assert_eq!(report.loads_attempted, 1);
        assert_eq!(report.loads_completed, 0);
        assert_eq!(report.rebases_attempted, 0);
        assert_eq!(report.aborts_completed, 1);
        assert_eq!(report.failures.len(), 1);
        assert_eq!(report.failures[0].reason, SliceRunReason::ReconnectLoad);

        world.resource_mut::<HandoffTrace>().events.clear();
        world.resource_mut::<HandoffTrace>().fail_load = false;
        world.resource_mut::<HandoffTrace>().block_rebase = true;
        let report = dispatch_reconnect_handoff(&mut world, token("offline:test"), &clock).unwrap();
        assert_eq!(report.rebases_attempted, 1);
        assert_eq!(report.rebases_completed, 0);
        assert_eq!(
            report.blocked_rebases,
            vec![SliceId::new("player.handoff_first")]
        );
        assert_eq!(report.aborts_completed, 2);
        assert!(report.failures.is_empty());

        world.resource_mut::<HandoffTrace>().events.clear();
        world.resource_mut::<HandoffTrace>().block_rebase = false;
        world.resource_mut::<HandoffTrace>().fail_rebase = true;
        let report = dispatch_reconnect_handoff(&mut world, token("offline:test"), &clock).unwrap();
        assert_eq!(report.rebases_attempted, 1);
        assert_eq!(report.rebases_completed, 0);
        assert_eq!(report.aborts_completed, 2);
        assert_eq!(report.failures.len(), 1);
        assert_eq!(report.failures[0].reason, SliceRunReason::Rebase);
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
        context: &SliceRunContext,
        slice_id: SliceId,
    ) -> Result<HydratedActivation, SliceRunError> {
        world.resource_scope(
            |_world, registry: valence::prelude::Mut<PersistenceSliceRegistry>| {
                let mut guarded = registry
                    .activate(
                        SliceLoad::<u32, &'static str>::loaded(9),
                        slice_id,
                        context.reconnect_activation()?,
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
        let activation = activate_partial_handoff_slice(world, context, slice_id)?;
        let mut state = world.resource_mut::<PartialHydrateState>();
        state.transitions.push((context.reason, slice_id));
        state.first = Some(activation);
        Ok(SliceRunOutcome::Clean)
    }

    fn hydrate_partial_second(world: &mut World, context: &SliceRunContext) -> SliceRunResult {
        assert_eq!(context.reason, SliceRunReason::ReconnectLoad);
        let slice_id = SliceId::new("player.partial_second");
        let activation = activate_partial_handoff_slice(world, context, slice_id)?;
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

    fn cleanup_partial_activation(
        world: &mut World,
        context: &SliceRunContext,
        slice_id: SliceId,
        first: bool,
    ) {
        assert!(matches!(
            context.reason,
            SliceRunReason::ReconnectCleanup | SliceRunReason::ReconnectAbort
        ));
        let mut state = world.resource_mut::<PartialHydrateState>();
        state.transitions.push((context.reason, slice_id));
        if first {
            state.first = None;
        } else {
            state.second = None;
        }
    }

    fn cleanup_partial_first(world: &mut World, context: &SliceRunContext) {
        cleanup_partial_activation(world, context, SliceId::new("player.partial_first"), true);
    }

    fn cleanup_partial_second(world: &mut World, context: &SliceRunContext) {
        cleanup_partial_activation(world, context, SliceId::new("player.partial_second"), false);
    }

    fn rebase_partial_activation(_world: &mut World, context: &SliceRunContext) -> SliceRunResult {
        assert_eq!(context.reason, SliceRunReason::Rebase);
        Ok(SliceRunOutcome::Clean)
    }

    #[test]
    fn reconnect_handoff_aborts_partial_hydrate_and_allows_clean_retry() {
        let first = Box::leak(Box::new(SliceDescriptor {
            hydrate: Some(hydrate_partial_first),
            reconnect_preflight: Some(noop_preflight),
            reconnect_cleanup: Some(cleanup_partial_first),
            rebase: Some(rebase_partial_activation),
            disconnect_save: Some(handoff_save),
            ..basic_descriptor("player.partial_first", 10)
        }));
        let second = Box::leak(Box::new(SliceDescriptor {
            hydrate: Some(hydrate_partial_second),
            reconnect_preflight: Some(noop_preflight),
            reconnect_cleanup: Some(cleanup_partial_second),
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
            assert_eq!(retry.aborts_completed, 0, "{name}");
        }
    }

    #[derive(Debug)]
    struct RealReconnectActivation {
        guarded: GuardedSlice<u32, &'static str>,
        tracker: DirtyTracker,
        fence: PersistedRevisionFence,
    }

    #[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
    enum InjectedHookResult {
        #[default]
        Clean,
        Blocked,
        Error,
    }

    #[derive(Debug, Default)]
    struct AtomicReconnectState {
        old_first: Option<RealReconnectActivation>,
        old_second: Option<RealReconnectActivation>,
        new_first: Option<RealReconnectActivation>,
        new_second: Option<RealReconnectActivation>,
        second_save: InjectedHookResult,
        second_preflight: InjectedHookResult,
        second_hydrate: InjectedHookResult,
        second_rebase: InjectedHookResult,
        preserve_new_first_on_abort: bool,
        release_first_during_rebase: bool,
        hydrate_attempts: usize,
        abort_order: Vec<SliceId>,
    }

    impl Resource for AtomicReconnectState {}

    fn make_test_activation(
        registry: &PersistenceSliceRegistry,
        slice_id: SliceId,
        subject: &str,
    ) -> RealReconnectActivation {
        let mut guarded = registry
            .activate_test_subject(
                SliceLoad::<u32, &'static str>::loaded(9),
                slice_id,
                subject_key(subject),
                DirtyRevision::new(4),
                || 0,
                |_| 0,
            )
            .expect("test activation must be unique");
        let (tracker, fence) = guarded
            .restore_persistence_state()
            .expect("test persistence state must be issued once");
        RealReconnectActivation {
            guarded,
            tracker,
            fence,
        }
    }

    fn make_handoff_activation(
        registry: &PersistenceSliceRegistry,
        context: &SliceRunContext,
        slice_id: SliceId,
    ) -> Result<RealReconnectActivation, SliceRunError> {
        let mut guarded = registry
            .activate(
                SliceLoad::<u32, &'static str>::loaded(9),
                slice_id,
                context.reconnect_activation()?,
                DirtyRevision::new(4),
                || 0,
                |_| 0,
            )
            .map_err(|_| SliceRunError::new("handoff activation rejected"))?;
        let (tracker, fence) = guarded
            .restore_persistence_state()
            .map_err(|_| SliceRunError::new("handoff persistence state already issued"))?;
        Ok(RealReconnectActivation {
            guarded,
            tracker,
            fence,
        })
    }

    fn activation_keeps_all_leases(activation: &RealReconnectActivation) -> bool {
        activation
            .guarded
            .subject
            .is_same(&activation.tracker.subject)
            && activation
                .guarded
                .subject
                .is_same(&activation.fence.subject)
    }

    fn atomic_save_first(_world: &mut World, context: &SliceRunContext) -> SliceRunResult {
        assert_eq!(context.reason, SliceRunReason::DisconnectSave);
        Ok(SliceRunOutcome::Flushed)
    }

    fn atomic_save_second(world: &mut World, context: &SliceRunContext) -> SliceRunResult {
        assert_eq!(context.reason, SliceRunReason::DisconnectSave);
        match world.resource::<AtomicReconnectState>().second_save {
            InjectedHookResult::Clean => Ok(SliceRunOutcome::Flushed),
            InjectedHookResult::Blocked => Ok(SliceRunOutcome::SkippedBlocked),
            InjectedHookResult::Error => Err(SliceRunError::new("second save failed")),
        }
    }

    fn atomic_preflight_first(_world: &World, context: &SliceRunContext) -> SliceRunResult {
        assert_eq!(context.reason, SliceRunReason::ReconnectPreflight);
        Ok(SliceRunOutcome::Clean)
    }

    fn atomic_preflight_second(world: &World, context: &SliceRunContext) -> SliceRunResult {
        assert_eq!(context.reason, SliceRunReason::ReconnectPreflight);
        match world.resource::<AtomicReconnectState>().second_preflight {
            InjectedHookResult::Clean => Ok(SliceRunOutcome::Clean),
            InjectedHookResult::Blocked => Ok(SliceRunOutcome::SkippedBlocked),
            InjectedHookResult::Error => Err(SliceRunError::new("second preflight failed")),
        }
    }

    fn atomic_cleanup_first(world: &mut World, context: &SliceRunContext) {
        let mut state = world.resource_mut::<AtomicReconnectState>();
        match context.reason {
            SliceRunReason::ReconnectCleanup => state.old_first = None,
            SliceRunReason::ReconnectAbort if !state.preserve_new_first_on_abort => {
                state.abort_order.push(SliceId::new("player.atomic_first"));
                state.new_first = None;
            }
            SliceRunReason::ReconnectAbort => {
                state.abort_order.push(SliceId::new("player.atomic_first"));
            }
            _ => panic!("unexpected reconnect cleanup reason"),
        }
    }

    fn atomic_cleanup_second(world: &mut World, context: &SliceRunContext) {
        let mut state = world.resource_mut::<AtomicReconnectState>();
        match context.reason {
            SliceRunReason::ReconnectCleanup => state.old_second = None,
            SliceRunReason::ReconnectAbort => {
                state.abort_order.push(SliceId::new("player.atomic_second"));
                state.new_second = None;
            }
            _ => panic!("unexpected reconnect cleanup reason"),
        }
    }

    fn atomic_hydrate_first(world: &mut World, context: &SliceRunContext) -> SliceRunResult {
        assert_eq!(context.reason, SliceRunReason::ReconnectLoad);
        let activation = world.resource_scope(
            |_world, registry: valence::prelude::Mut<PersistenceSliceRegistry>| {
                make_handoff_activation(&registry, context, SliceId::new("player.atomic_first"))
            },
        )?;
        let mut state = world.resource_mut::<AtomicReconnectState>();
        state.hydrate_attempts += 1;
        state.new_first = Some(activation);
        Ok(SliceRunOutcome::Clean)
    }

    fn atomic_hydrate_second(world: &mut World, context: &SliceRunContext) -> SliceRunResult {
        assert_eq!(context.reason, SliceRunReason::ReconnectLoad);
        let activation = world.resource_scope(
            |_world, registry: valence::prelude::Mut<PersistenceSliceRegistry>| {
                make_handoff_activation(&registry, context, SliceId::new("player.atomic_second"))
            },
        )?;
        let mut state = world.resource_mut::<AtomicReconnectState>();
        state.hydrate_attempts += 1;
        state.new_second = Some(activation);
        match state.second_hydrate {
            InjectedHookResult::Clean => Ok(SliceRunOutcome::Clean),
            InjectedHookResult::Blocked => Ok(SliceRunOutcome::SkippedBlocked),
            InjectedHookResult::Error => Err(SliceRunError::new(
                "second hydrate failed after real activation",
            )),
        }
    }

    fn atomic_rebase_first(world: &mut World, context: &SliceRunContext) -> SliceRunResult {
        assert_eq!(context.reason, SliceRunReason::Rebase);
        let mut state = world.resource_mut::<AtomicReconnectState>();
        if state.release_first_during_rebase {
            state.new_first = None;
        }
        Ok(SliceRunOutcome::Clean)
    }

    fn atomic_rebase_second(world: &mut World, context: &SliceRunContext) -> SliceRunResult {
        assert_eq!(context.reason, SliceRunReason::Rebase);
        match world.resource::<AtomicReconnectState>().second_rebase {
            InjectedHookResult::Clean => Ok(SliceRunOutcome::Clean),
            InjectedHookResult::Blocked => Ok(SliceRunOutcome::SkippedBlocked),
            InjectedHookResult::Error => Err(SliceRunError::new("second rebase failed")),
        }
    }

    fn atomic_descriptors() -> (&'static SliceDescriptor, &'static SliceDescriptor) {
        let first = Box::leak(Box::new(SliceDescriptor {
            hydrate: Some(atomic_hydrate_first),
            reconnect_preflight: Some(atomic_preflight_first),
            reconnect_cleanup: Some(atomic_cleanup_first),
            rebase: Some(atomic_rebase_first),
            disconnect_save: Some(atomic_save_first),
            ..basic_descriptor("player.atomic_first", 10)
        }));
        let second = Box::leak(Box::new(SliceDescriptor {
            hydrate: Some(atomic_hydrate_second),
            reconnect_preflight: Some(atomic_preflight_second),
            reconnect_cleanup: Some(atomic_cleanup_second),
            rebase: Some(atomic_rebase_second),
            disconnect_save: Some(atomic_save_second),
            write_binding: SECOND_TEST_BINDING,
            ..basic_descriptor("player.atomic_second", 20)
        }));
        (first, second)
    }

    fn atomic_reconnect_world_with_failures(
        first: &'static SliceDescriptor,
        second: &'static SliceDescriptor,
        second_save: InjectedHookResult,
        second_preflight: InjectedHookResult,
        second_hydrate: InjectedHookResult,
        second_rebase: InjectedHookResult,
    ) -> World {
        let mut registry = PersistenceSliceRegistry::empty();
        registry.register(first).unwrap();
        registry.register(second).unwrap();
        let old_first = make_test_activation(&registry, first.id, "player:atomic");
        let old_second = make_test_activation(&registry, second.id, "player:atomic");
        let mut world = World::new();
        world.insert_resource(registry);
        world.insert_resource(HandoffTrace::default());
        world.insert_resource(AtomicReconnectState {
            old_first: Some(old_first),
            old_second: Some(old_second),
            second_save,
            second_preflight,
            second_hydrate,
            second_rebase,
            ..AtomicReconnectState::default()
        });
        world
    }

    fn atomic_reconnect_world(
        first: &'static SliceDescriptor,
        second: &'static SliceDescriptor,
        second_preflight: InjectedHookResult,
        second_hydrate: InjectedHookResult,
    ) -> World {
        atomic_reconnect_world_with_failures(
            first,
            second,
            InjectedHookResult::Clean,
            second_preflight,
            second_hydrate,
            InjectedHookResult::Clean,
        )
    }

    #[test]
    fn reconnect_preflight_failure_preserves_all_old_real_activation_leases_until_clean_retry() {
        let (first, second) = atomic_descriptors();
        let clock = FixedClock {
            runtime_tick: 400,
            wall_unix_millis: 49_999,
        };

        for injected in [InjectedHookResult::Blocked, InjectedHookResult::Error] {
            let mut world =
                atomic_reconnect_world(first, second, injected, InjectedHookResult::Clean);
            let report =
                dispatch_reconnect_handoff(&mut world, token("player:atomic"), &clock).unwrap();

            assert_eq!(report.preflights_attempted, 2, "{injected:?}");
            assert_eq!(report.preflights_completed, 1, "{injected:?}");
            assert_eq!(report.cleanups_completed, 0, "{injected:?}");
            assert_eq!(report.loads_attempted, 0, "{injected:?}");
            {
                let state = world.resource::<AtomicReconnectState>();
                assert_eq!(state.hydrate_attempts, 0, "{injected:?}");
                assert!(activation_keeps_all_leases(
                    state
                        .old_first
                        .as_ref()
                        .expect("first old activation retained")
                ));
                assert!(activation_keeps_all_leases(
                    state
                        .old_second
                        .as_ref()
                        .expect("second old activation retained")
                ));
                assert!(state.new_first.is_none() && state.new_second.is_none());
            }

            world
                .resource_mut::<AtomicReconnectState>()
                .second_preflight = InjectedHookResult::Clean;
            let retry =
                dispatch_reconnect_handoff(&mut world, token("player:atomic"), &clock).unwrap();
            assert_eq!(retry.cleanups_completed, 2, "{injected:?}");
            assert_eq!(retry.loads_completed, 2, "{injected:?}");
            assert_eq!(retry.rebases_completed, 2, "{injected:?}");
            let state = world.resource::<AtomicReconnectState>();
            assert!(state.old_first.is_none() && state.old_second.is_none());
            assert!(activation_keeps_all_leases(
                state.new_first.as_ref().expect("first retry activation")
            ));
            assert!(activation_keeps_all_leases(
                state.new_second.as_ref().expect("second retry activation")
            ));
        }
    }

    #[test]
    fn reconnect_rebase_lease_mutation_fails_closed_and_aborts_hydrated_state() {
        let (first, second) = atomic_descriptors();
        let mut world = atomic_reconnect_world(
            first,
            second,
            InjectedHookResult::Clean,
            InjectedHookResult::Clean,
        );
        world
            .resource_mut::<AtomicReconnectState>()
            .release_first_during_rebase = true;
        let clock = FixedClock {
            runtime_tick: 400,
            wall_unix_millis: 49_999,
        };

        let error = dispatch_reconnect_handoff(&mut world, token("player:atomic"), &clock)
            .expect_err("rebase must not change the active reconnect lease set");
        assert_eq!(
            error,
            SliceDispatchError::UnexpectedRebaseLease {
                slice_id: first.id,
                domain: first.write_binding.domain(),
            }
        );
        let state = world.resource::<AtomicReconnectState>();
        assert!(state.old_first.is_none() && state.old_second.is_none());
        assert!(state.new_first.is_none() && state.new_second.is_none());
        let subject = subject_key("player:atomic");
        let registry = world.resource::<PersistenceSliceRegistry>();
        assert!(!registry.active_subject_domain(&subject, first.write_binding.domain()));
        assert!(!registry.active_subject_domain(&subject, second.write_binding.domain()));
    }

    #[test]
    fn reconnect_save_failure_preserves_real_activation_leases() {
        let (first, second) = atomic_descriptors();
        let mut world = atomic_reconnect_world_with_failures(
            first,
            second,
            InjectedHookResult::Error,
            InjectedHookResult::Clean,
            InjectedHookResult::Clean,
            InjectedHookResult::Clean,
        );
        let clock = FixedClock {
            runtime_tick: 400,
            wall_unix_millis: 49_999,
        };

        let report = dispatch_reconnect_handoff(&mut world, token("player:atomic"), &clock)
            .expect("save failure should be reported without destructive cleanup");
        assert_eq!(report.saves_attempted, 2);
        assert_eq!(report.saves_completed, 1);
        assert_eq!(report.loads_attempted, 0);
        assert_eq!(report.failures.len(), 1);
        let state = world.resource::<AtomicReconnectState>();
        assert!(state.old_first.is_some() && state.old_second.is_some());
        assert!(activation_keeps_all_leases(
            state.old_first.as_ref().unwrap()
        ));
        assert!(activation_keeps_all_leases(
            state.old_second.as_ref().unwrap()
        ));
        assert!(world
            .resource::<PersistenceSliceRegistry>()
            .active_subject_domain(&subject_key("player:atomic"), first.write_binding.domain()));
        assert!(world
            .resource::<PersistenceSliceRegistry>()
            .active_subject_domain(&subject_key("player:atomic"), second.write_binding.domain()));
    }
    #[test]
    fn later_save_failure_or_block_preserves_all_old_activations() {
        let (first, second) = atomic_descriptors();
        let clock = FixedClock {
            runtime_tick: 400,
            wall_unix_millis: 49_999,
        };

        for injected in [InjectedHookResult::Blocked, InjectedHookResult::Error] {
            let mut world = atomic_reconnect_world_with_failures(
                first,
                second,
                injected,
                InjectedHookResult::Clean,
                InjectedHookResult::Clean,
                InjectedHookResult::Clean,
            );
            let report =
                dispatch_reconnect_handoff(&mut world, token("player:atomic"), &clock).unwrap();

            assert_eq!(report.saves_attempted, 2, "{injected:?}");
            assert_eq!(report.saves_completed, 1, "{injected:?}");
            assert_eq!(report.cleanups_completed, 0, "{injected:?}");
            assert_eq!(report.loads_attempted, 0, "{injected:?}");
            let state = world.resource::<AtomicReconnectState>();
            assert!(state.old_first.is_some() && state.old_second.is_some());
            assert!(state.new_first.is_none() && state.new_second.is_none());
        }
    }

    #[test]
    fn later_rebase_failure_or_block_aborts_all_hydrated_activations() {
        let (first, second) = atomic_descriptors();
        let clock = FixedClock {
            runtime_tick: 400,
            wall_unix_millis: 49_999,
        };

        for injected in [InjectedHookResult::Blocked, InjectedHookResult::Error] {
            let mut world = atomic_reconnect_world_with_failures(
                first,
                second,
                InjectedHookResult::Clean,
                InjectedHookResult::Clean,
                InjectedHookResult::Clean,
                injected,
            );
            let report =
                dispatch_reconnect_handoff(&mut world, token("player:atomic"), &clock).unwrap();

            assert_eq!(report.rebases_attempted, 2, "{injected:?}");
            assert_eq!(report.rebases_completed, 1, "{injected:?}");
            assert_eq!(report.aborts_completed, 2, "{injected:?}");
            let state = world.resource::<AtomicReconnectState>();
            assert_eq!(
                state.abort_order,
                vec![second.id, first.id],
                "rebase {injected:?} must abort hydrated descriptors in reverse registry order"
            );
            assert!(state.old_first.is_none() && state.old_second.is_none());
            assert!(state.new_first.is_none() && state.new_second.is_none());
        }
    }

    #[test]
    fn reconnect_hydrate_failure_after_real_activation_rolls_back_all_and_retries_cleanly() {
        let (first, second) = atomic_descriptors();
        let clock = FixedClock {
            runtime_tick: 400,
            wall_unix_millis: 49_999,
        };

        for injected in [InjectedHookResult::Blocked, InjectedHookResult::Error] {
            let mut world =
                atomic_reconnect_world(first, second, InjectedHookResult::Clean, injected);
            let report =
                dispatch_reconnect_handoff(&mut world, token("player:atomic"), &clock).unwrap();

            assert_eq!(report.loads_attempted, 2, "{injected:?}");
            assert_eq!(report.loads_completed, 1, "{injected:?}");
            assert_eq!(report.aborts_completed, 2, "{injected:?}");
            assert_eq!(report.rebases_attempted, 0, "{injected:?}");
            {
                let state = world.resource::<AtomicReconnectState>();
                assert!(state.old_first.is_none() && state.old_second.is_none());
                assert!(state.new_first.is_none() && state.new_second.is_none());
            }
            let subject = subject_key("player:atomic");
            {
                let registry = world.resource::<PersistenceSliceRegistry>();
                assert!(!registry.active_subject_domain(&subject, first.write_binding.domain()));
                assert!(!registry.active_subject_domain(&subject, second.write_binding.domain()));
            }

            world.resource_mut::<AtomicReconnectState>().second_hydrate = InjectedHookResult::Clean;
            let retry =
                dispatch_reconnect_handoff(&mut world, token("player:atomic"), &clock).unwrap();
            assert_eq!(retry.loads_completed, 2, "{injected:?}");
            assert_eq!(retry.rebases_completed, 2, "{injected:?}");
            assert_eq!(retry.aborts_completed, 0, "{injected:?}");
        }
    }
    #[test]
    fn reconnect_abort_rejects_cleanup_that_retains_a_real_activation_lease() {
        let (first, second) = atomic_descriptors();
        let clock = FixedClock {
            runtime_tick: 400,
            wall_unix_millis: 49_999,
        };
        let mut world = atomic_reconnect_world(
            first,
            second,
            InjectedHookResult::Clean,
            InjectedHookResult::Error,
        );
        world
            .resource_mut::<AtomicReconnectState>()
            .preserve_new_first_on_abort = true;

        let error = dispatch_reconnect_handoff(&mut world, token("player:atomic"), &clock)
            .expect_err("a returned cleanup hook must still release every attempted activation");

        assert_eq!(
            error,
            SliceDispatchError::DuplicateSubject {
                slice_id: first.id,
                domain: first.write_binding.domain(),
            }
        );
        let state = world.resource::<AtomicReconnectState>();
        assert!(state.new_first.is_some());
        assert!(state.new_second.is_none());
    }

    #[derive(Debug, Default)]
    struct HydrateLeaseAuditState {
        activations: HashMap<SliceId, RealReconnectActivation>,
        aborts: Vec<SliceId>,
    }

    impl Resource for HydrateLeaseAuditState {}

    fn empty_hydrate(_world: &mut World, context: &SliceRunContext) -> SliceRunResult {
        assert_eq!(context.reason, SliceRunReason::ReconnectLoad);
        Ok(SliceRunOutcome::Clean)
    }

    fn hydrate_extra_domain(world: &mut World, context: &SliceRunContext) -> SliceRunResult {
        assert_eq!(context.reason, SliceRunReason::ReconnectLoad);
        let activation = world.resource_scope(
            |_world, registry: valence::prelude::Mut<PersistenceSliceRegistry>| {
                make_handoff_activation(&registry, context, SliceId::new("player.audit_extra"))
            },
        )?;
        world
            .resource_mut::<HydrateLeaseAuditState>()
            .activations
            .insert(SliceId::new("player.audit_extra"), activation);
        Ok(SliceRunOutcome::Clean)
    }

    fn hydrate_audit_first(world: &mut World, context: &SliceRunContext) -> SliceRunResult {
        assert_eq!(context.reason, SliceRunReason::ReconnectLoad);
        let slice_id = SliceId::new("player.audit_first");
        let activation = world.resource_scope(
            |_world, registry: valence::prelude::Mut<PersistenceSliceRegistry>| {
                make_handoff_activation(&registry, context, slice_id)
            },
        )?;
        world
            .resource_mut::<HydrateLeaseAuditState>()
            .activations
            .insert(slice_id, activation);
        Ok(SliceRunOutcome::Clean)
    }

    fn cleanup_audit_slice(world: &mut World, context: &SliceRunContext, slice_id: SliceId) {
        let mut state = world.resource_mut::<HydrateLeaseAuditState>();
        if context.reason == SliceRunReason::ReconnectAbort {
            state.aborts.push(slice_id);
        }
        state.activations.remove(&slice_id);
    }

    fn cleanup_audit_first(world: &mut World, context: &SliceRunContext) {
        cleanup_audit_slice(world, context, SliceId::new("player.audit_first"));
    }

    fn cleanup_audit_empty(world: &mut World, context: &SliceRunContext) {
        cleanup_audit_slice(world, context, SliceId::new("player.audit_empty"));
    }

    fn cleanup_audit_extra(world: &mut World, context: &SliceRunContext) {
        let mut state = world.resource_mut::<HydrateLeaseAuditState>();
        if context.reason == SliceRunReason::ReconnectAbort {
            state.aborts.push(SliceId::new("player.audit_target"));
        }
        state
            .activations
            .remove(&SliceId::new("player.audit_extra"));
    }

    #[test]
    fn reconnect_empty_hydrate_fails_closed_without_completion_or_rebase() {
        let descriptor = Box::leak(Box::new(SliceDescriptor {
            hydrate: Some(empty_hydrate),
            reconnect_preflight: Some(noop_preflight),
            reconnect_cleanup: Some(cleanup_audit_empty),
            rebase: Some(rebase_partial_activation),
            disconnect_save: Some(handoff_save),
            time_basis: TimeBasis::RemainingLogicalTicks,
            ..basic_descriptor("player.audit_empty", 10)
        }));
        let mut registry = PersistenceSliceRegistry::empty();
        registry.register(descriptor).unwrap();
        let mut world = World::new();
        world.insert_resource(registry);
        world.insert_resource(HandoffTrace::default());
        world.insert_resource(HydrateLeaseAuditState::default());
        let clock = FixedClock {
            runtime_tick: 400,
            wall_unix_millis: 49_999,
        };

        let error = dispatch_reconnect_handoff(&mut world, token("player:audit"), &clock)
            .expect_err("a Clean result without the descriptor lease must fail closed");

        assert_eq!(
            error,
            SliceDispatchError::MissingHydrateLease {
                slice_id: descriptor.id,
                domain: descriptor.write_binding.domain(),
            }
        );
        let state = world.resource::<HydrateLeaseAuditState>();
        assert_eq!(state.aborts, vec![descriptor.id]);
        assert!(state.activations.is_empty());
        let registry = world.resource::<PersistenceSliceRegistry>();
        assert!(registry.active_subject_leases().unwrap().is_empty());
    }

    #[test]
    fn reconnect_empty_second_hydrate_aborts_every_attempt_in_reverse_order() {
        let first = Box::leak(Box::new(SliceDescriptor {
            hydrate: Some(hydrate_audit_first),
            reconnect_preflight: Some(noop_preflight),
            reconnect_cleanup: Some(cleanup_audit_first),
            rebase: Some(rebase_partial_activation),
            disconnect_save: Some(handoff_save),
            time_basis: TimeBasis::RemainingLogicalTicks,
            ..basic_descriptor("player.audit_first", 10)
        }));
        let second = Box::leak(Box::new(SliceDescriptor {
            hydrate: Some(empty_hydrate),
            reconnect_preflight: Some(noop_preflight),
            reconnect_cleanup: Some(cleanup_audit_empty),
            rebase: Some(rebase_partial_activation),
            disconnect_save: Some(handoff_save),
            time_basis: TimeBasis::RemainingLogicalTicks,
            write_binding: SECOND_TEST_BINDING,
            ..basic_descriptor("player.audit_empty", 20)
        }));
        let mut registry = PersistenceSliceRegistry::empty();
        registry.register(first).unwrap();
        registry.register(second).unwrap();
        let mut world = World::new();
        world.insert_resource(registry);
        world.insert_resource(HandoffTrace::default());
        world.insert_resource(HydrateLeaseAuditState::default());
        let clock = FixedClock {
            runtime_tick: 400,
            wall_unix_millis: 49_999,
        };

        let error = dispatch_reconnect_handoff(&mut world, token("player:audit"), &clock)
            .expect_err("a later empty hydrate must roll back every attempted descriptor");

        assert_eq!(
            error,
            SliceDispatchError::MissingHydrateLease {
                slice_id: second.id,
                domain: second.write_binding.domain(),
            }
        );
        let state = world.resource::<HydrateLeaseAuditState>();
        assert_eq!(state.aborts, vec![second.id, first.id]);
        assert!(state.activations.is_empty());
        assert!(world
            .resource::<PersistenceSliceRegistry>()
            .active_subject_leases()
            .unwrap()
            .is_empty());
    }

    #[test]
    fn reconnect_hydrate_rejects_an_extra_domain_lease_and_cleans_it() {
        let target = Box::leak(Box::new(SliceDescriptor {
            hydrate: Some(hydrate_extra_domain),
            reconnect_preflight: Some(noop_preflight),
            reconnect_cleanup: Some(cleanup_audit_extra),
            rebase: Some(rebase_partial_activation),
            disconnect_save: Some(handoff_save),
            time_basis: TimeBasis::RemainingLogicalTicks,
            ..basic_descriptor("player.audit_target", 10)
        }));
        let extra = Box::leak(Box::new(SliceDescriptor {
            write_binding: EXTRA_TEST_BINDING,
            ..basic_descriptor("player.audit_extra", 20)
        }));
        let mut registry = PersistenceSliceRegistry::empty();
        registry.register(target).unwrap();
        registry.register(extra).unwrap();
        let mut world = World::new();
        world.insert_resource(registry);
        world.insert_resource(HandoffTrace::default());
        world.insert_resource(HydrateLeaseAuditState::default());
        let clock = FixedClock {
            runtime_tick: 400,
            wall_unix_millis: 49_999,
        };

        let error = dispatch_reconnect_handoff(&mut world, token("player:audit"), &clock)
            .expect_err("a hydrate must not activate a different write domain");

        assert_eq!(
            error,
            SliceDispatchError::UnexpectedHydrateLease {
                slice_id: target.id,
                domain: extra.write_binding.domain(),
            }
        );
        assert!(world
            .resource::<PersistenceSliceRegistry>()
            .active_subject_leases()
            .unwrap()
            .is_empty());
    }

    #[derive(Debug, Default)]
    struct ForeignHydrateState {
        retained: Option<RealReconnectActivation>,
        retain_on_abort: bool,
        hydrate_attempts: usize,
    }

    impl Resource for ForeignHydrateState {}

    fn foreign_subject_hydrate(world: &mut World, context: &SliceRunContext) -> SliceRunResult {
        assert_eq!(context.reason, SliceRunReason::ReconnectLoad);
        let activation = world.resource_scope(
            |_world, registry: valence::prelude::Mut<PersistenceSliceRegistry>| {
                make_test_activation(
                    &registry,
                    SliceId::new("player.foreign_subject"),
                    "player:foreign",
                )
            },
        );
        let mut state = world.resource_mut::<ForeignHydrateState>();
        state.hydrate_attempts += 1;
        state.retained = Some(activation);
        Ok(SliceRunOutcome::Clean)
    }

    fn foreign_subject_cleanup(world: &mut World, context: &SliceRunContext) {
        assert!(matches!(
            context.reason,
            SliceRunReason::ReconnectCleanup | SliceRunReason::ReconnectAbort
        ));
        let mut state = world.resource_mut::<ForeignHydrateState>();
        if context.reason == SliceRunReason::ReconnectAbort && !state.retain_on_abort {
            state.retained = None;
        }
    }

    #[test]
    fn reconnect_hydrate_cannot_escape_handoff_subject_or_report_clean_retry() {
        let descriptor = Box::leak(Box::new(SliceDescriptor {
            hydrate: Some(foreign_subject_hydrate),
            reconnect_preflight: Some(noop_preflight),
            reconnect_cleanup: Some(foreign_subject_cleanup),
            ..basic_descriptor("player.foreign_subject", 10)
        }));
        let mut registry = PersistenceSliceRegistry::empty();
        registry.register(descriptor).unwrap();
        let mut world = World::new();
        world.insert_resource(registry);
        world.insert_resource(ForeignHydrateState {
            retain_on_abort: true,
            ..ForeignHydrateState::default()
        });
        let clock = FixedClock {
            runtime_tick: 400,
            wall_unix_millis: 49_999,
        };

        let error = dispatch_reconnect_handoff(&mut world, token("player:stable"), &clock)
            .expect_err("a hydrate cannot choose a subject outside its handoff capability");
        assert_eq!(
            error,
            SliceDispatchError::UnexpectedHydrateSubject {
                slice_id: descriptor.id,
                domain: descriptor.write_binding.domain(),
            }
        );
        let state = world.resource::<ForeignHydrateState>();
        assert_eq!(state.hydrate_attempts, 1);
        assert!(state.retained.is_some());
        let registry = world.resource::<PersistenceSliceRegistry>();
        assert!(registry.active_subject_domain(
            &subject_key("player:foreign"),
            descriptor.write_binding.domain(),
        ));
        assert!(!registry.active_subject_domain(
            &subject_key("player:stable"),
            descriptor.write_binding.domain(),
        ));
    }

    #[test]
    fn reconnect_foreign_subject_attempt_is_aborted_without_leaking_a_writer() {
        let descriptor = Box::leak(Box::new(SliceDescriptor {
            hydrate: Some(foreign_subject_hydrate),
            reconnect_preflight: Some(noop_preflight),
            reconnect_cleanup: Some(foreign_subject_cleanup),
            ..basic_descriptor("player.foreign_subject", 10)
        }));
        let mut registry = PersistenceSliceRegistry::empty();
        registry.register(descriptor).unwrap();
        let mut world = World::new();
        world.insert_resource(registry);
        world.insert_resource(ForeignHydrateState::default());
        let clock = FixedClock {
            runtime_tick: 400,
            wall_unix_millis: 49_999,
        };

        let error = dispatch_reconnect_handoff(&mut world, token("player:stable"), &clock)
            .expect_err("a foreign hydrate must fail closed even when abort releases its lease");
        assert_eq!(
            error,
            SliceDispatchError::UnexpectedHydrateSubject {
                slice_id: descriptor.id,
                domain: descriptor.write_binding.domain(),
            }
        );
        assert!(world.resource::<ForeignHydrateState>().retained.is_none());
        let registry = world.resource::<PersistenceSliceRegistry>();
        assert!(!registry.active_subject_domain(
            &subject_key("player:foreign"),
            descriptor.write_binding.domain(),
        ));
        assert!(!registry.active_subject_domain(
            &subject_key("player:stable"),
            descriptor.write_binding.domain(),
        ));
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

    fn cleanup_activation(world: &mut World, context: &SliceRunContext) {
        assert!(matches!(
            context.reason,
            SliceRunReason::ReconnectCleanup | SliceRunReason::ReconnectAbort
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
            .activate_test_subject(
                SliceLoad::<u32, &'static str>::loaded(9),
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
            hydrate: Some(handoff_load_activation_first),
            reconnect_preflight: Some(noop_preflight),
            reconnect_cleanup: Some(cleanup_activation),
            disconnect_save: Some(handoff_save),
            ..basic_descriptor("player.activation_first", 10)
        }));
        let second = Box::leak(Box::new(SliceDescriptor {
            time_basis: TimeBasis::RemainingLogicalTicks,
            rebase: Some(handoff_rebase),
            hydrate: Some(handoff_load_activation_second),
            reconnect_preflight: Some(noop_preflight),
            reconnect_cleanup: Some(cleanup_activation),
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
        assert_eq!(report.cleanups_completed, 2);
        assert_eq!(report.loads_completed, 2);
        assert_eq!(report.rebases_completed, 2);
        assert!(report.failures.is_empty());
    }

    #[test]
    fn slice_load_status_predicates_cover_every_state() {
        let cases = [
            (
                SliceLoad::<u32, &str>::missing(),
                SliceLoadStatus::Missing,
                (true, false, false),
            ),
            (
                SliceLoad::<u32, &str>::loaded(17),
                SliceLoadStatus::Loaded,
                (false, true, false),
            ),
            (
                SliceLoad::<u32, &str>::failed("secret failure"),
                SliceLoadStatus::Failed,
                (false, false, true),
            ),
        ];

        for (load, status, predicates) in cases {
            assert_eq!(load.status(), status);
            assert_eq!(
                (load.is_missing(), load.is_loaded(), load.is_failed()),
                predicates,
                "each SliceLoad state must report one and only one matching predicate"
            );
        }
    }

    #[test]
    fn slice_load_debug_reports_status_without_payloads() {
        let missing = format!("{:?}", SliceLoad::<String, String>::missing());
        let loaded = format!(
            "{:?}",
            SliceLoad::<String, String>::loaded("private player payload".to_string())
        );
        let failed = format!(
            "{:?}",
            SliceLoad::<String, String>::failed("database credential leaked".to_string())
        );

        assert!(missing.contains("status: Missing"));
        assert!(loaded.contains("status: Loaded"));
        assert!(failed.contains("status: Failed"));
        assert!(!loaded.contains("private player payload"));
        assert!(!failed.contains("database credential leaked"));
    }

    #[test]
    fn failed_load_fallback_is_read_only_and_never_becomes_dirty() {
        let descriptor = basic_descriptor("player.failed", 10);
        let (_registry, mut guarded) = activate(
            &descriptor,
            SliceLoad::<u32, _>::failed("invalid json"),
            "player:failed",
            DirtyRevision::new(7),
            || 1,
            |_error| 0,
        );
        let (mut tracker, _fence) = guarded.restore_persistence_state().unwrap();
        let mut mutation_called = false;

        assert_eq!(guarded.load_status(), SliceLoadStatus::Failed);
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

        let debug = format!("{guarded:?}");
        assert!(debug.contains("load_status: Failed"));
        assert!(
            !debug.contains("invalid json"),
            "GuardedSlice Debug must not disclose failed-load provenance: {debug}"
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
        let descriptor = Box::leak(Box::new(descriptor));
        let mut registry = PersistenceSliceRegistry::empty();
        registry.register(descriptor).unwrap();
        let mut fallback_called = false;
        let result = registry.activate_test_subject(
            SliceLoad::<u32, _>::failed("corrupt ledger"),
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
            SliceLoad::<u32, &str>::missing(),
            "player:missing",
            DirtyRevision::default(),
            || 7,
            |_| 0,
        );
        let (_loaded_registry, loaded) = activate(
            &descriptor,
            SliceLoad::<u32, &str>::loaded(9),
            "player:loaded",
            DirtyRevision::default(),
            || 0,
            |_| 0,
        );

        assert_eq!(*missing.value(), 7);
        assert_eq!(missing.load_status(), SliceLoadStatus::Missing);
        assert!(missing.write_permit(WriteOutlet::Autosave).is_ok());
        assert_eq!(*loaded.value(), 9);
        assert_eq!(loaded.load_status(), SliceLoadStatus::Loaded);
        assert!(loaded.write_permit(WriteOutlet::Shutdown).is_ok());
    }

    #[test]
    fn stable_subject_activation_rejects_reactivation_until_every_lease_holder_releases() {
        let descriptor = Box::leak(Box::new(SliceDescriptor {
            write_binding: TEST_BINDING,
            ..basic_descriptor("player.subject", 10)
        }));
        let mut registry = PersistenceSliceRegistry::empty();
        registry.register(descriptor).unwrap();

        let mut first = registry
            .activate_test_subject(
                SliceLoad::<u32, &str>::loaded(9),
                descriptor.id,
                subject_key("player:stable"),
                DirtyRevision::new(4),
                || 0,
                |_| 0,
            )
            .unwrap();
        let duplicate = registry.activate_test_subject(
            SliceLoad::<u32, &str>::loaded(10),
            descriptor.id,
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
            }) if slice_id == descriptor.id && domain == TEST_BINDING.domain()
        ));

        let other_subject = registry
            .activate_test_subject(
                SliceLoad::<u32, &str>::loaded(11),
                descriptor.id,
                subject_key("player:other"),
                DirtyRevision::new(4),
                || 0,
                |_| 0,
            )
            .unwrap();
        assert_eq!(*other_subject.value(), 11);
        let (tracker, fence) = first.restore_persistence_state().unwrap();
        drop(first);

        let retained_tracker = registry.activate_test_subject(
            SliceLoad::<u32, &str>::loaded(12),
            descriptor.id,
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

        let retained_fence = registry.activate_test_subject(
            SliceLoad::<u32, &str>::loaded(12),
            descriptor.id,
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
            .activate_test_subject(
                SliceLoad::<u32, &str>::loaded(12),
                descriptor.id,
                subject_key("player:stable"),
                DirtyRevision::new(7),
                || 0,
                |_| 0,
            )
            .unwrap();
        assert_eq!(*reactivated.value(), 12);
    }

    #[test]
    fn durable_writer_rejects_cross_ordering_and_serialized_zero_row() {
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE durable_ordering (subject TEXT PRIMARY KEY, value INTEGER NOT NULL)",
            )
            .unwrap();
        let transaction = connection.transaction().unwrap();
        let payload = 10_u32;
        let subject_key = subject_key("player:ordering");
        let request = |ordering| DurableWriteRequest {
            transaction: &transaction,
            payload: &payload,
            subject_key: &subject_key,
            binding: TEST_BINDING,
            expected_persisted_revision: DirtyRevision::default(),
            write_revision: DirtyRevision::new(1),
            outlet: WriteOutlet::Autosave,
            ordering,
            executed: Cell::new(false),
        };

        assert_eq!(
            request(WriteOrdering::Serialized).execute_cas(
                "INSERT INTO durable_ordering (subject, value) VALUES (?1, ?2)",
                ("player:ordering", 10),
            ),
            Err(DurableWriteExecuteError::Proof(
                DurableWriteProofError::WrongOrdering {
                    expected: WriteOrdering::PersistedRevisionCas,
                    actual: WriteOrdering::Serialized,
                }
            ))
        );
        assert_eq!(
            request(WriteOrdering::PersistedRevisionCas).execute_serialized(
                "INSERT INTO durable_ordering (subject, value) VALUES (?1, ?2)",
                ("player:ordering", 10),
            ),
            Err(DurableWriteExecuteError::Proof(
                DurableWriteProofError::WrongOrdering {
                    expected: WriteOrdering::Serialized,
                    actual: WriteOrdering::PersistedRevisionCas,
                }
            ))
        );
        assert_eq!(
            request(WriteOrdering::Serialized).execute_serialized(
                "UPDATE durable_ordering SET value = ?1 WHERE subject = ?2",
                (10, "missing"),
            ),
            Err(DurableWriteExecuteError::Proof(
                DurableWriteProofError::SerializedWriteRejected
            ))
        );
    }

    #[test]
    fn mutation_and_durable_receipts_remain_bound_to_one_guarded_subject() {
        let descriptor = SliceDescriptor {
            write_binding: TEST_BINDING,
            ..basic_descriptor("player.subject", 10)
        };
        let (_first_registry, mut first) = activate(
            &descriptor,
            SliceLoad::<u32, &str>::loaded(9),
            "player:first",
            DirtyRevision::default(),
            || 0,
            |_| 0,
        );
        let (_second_registry, mut second) = activate(
            &descriptor,
            SliceLoad::<u32, &str>::loaded(9),
            "player:second",
            DirtyRevision::default(),
            || 0,
            |_| 0,
        );
        let (mut first_tracker, mut first_fence) = first.restore_persistence_state().unwrap();
        let (mut second_tracker, _second_fence) = second.restore_persistence_state().unwrap();
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE durable_subject (subject TEXT PRIMARY KEY, value INTEGER NOT NULL)",
            )
            .unwrap();

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
            first_fence.commit(&mut connection, second_snapshot, |_request| {
                wrong_subject_writer_called = true;
                Err::<(), _>("wrong-subject writer must not run")
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
            .commit(&mut connection, first_snapshot, |request| {
                request
                    .execute_serialized(
                        "INSERT INTO durable_subject (subject, value) VALUES (?1, ?2)",
                        (request.subject_key().0.as_str(), *request.payload()),
                    )
                    .map_err(|_| "insert failed")
            })
            .unwrap();
        assert_eq!(
            second_tracker.acknowledge(first_receipt),
            DirtyAcknowledgement::WrongSubject
        );
        assert!(second_tracker.is_dirty());
        assert_eq!(
            connection
                .query_row(
                    "SELECT value FROM durable_subject WHERE subject = 'player:first'",
                    [],
                    |row| row.get::<_, u32>(0),
                )
                .unwrap(),
            10
        );
    }

    #[test]
    fn durable_commit_rolls_back_adapter_failure_and_preserves_dirty_state() {
        let descriptor = SliceDescriptor {
            write_binding: TEST_BINDING,
            ..basic_descriptor("player.rollback", 10)
        };
        let (_registry, mut guarded) = activate(
            &descriptor,
            SliceLoad::<u32, &str>::loaded(9),
            "player:rollback",
            DirtyRevision::default(),
            || 0,
            |_| 0,
        );
        let (mut tracker, mut fence) = guarded.restore_persistence_state().unwrap();
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch("CREATE TABLE durable_rollback (value INTEGER NOT NULL)")
            .unwrap();

        guarded.mutate(&mut tracker, |value| *value = 10).unwrap();
        let permit = guarded.write_permit(WriteOutlet::Autosave).unwrap();
        let snapshot = tracker
            .begin_snapshot(permit, |value| *value)
            .unwrap()
            .unwrap();
        let result = fence.commit(&mut connection, snapshot, |request| {
            request
                .execute_serialized("INSERT INTO durable_rollback (value) VALUES (?1)", [10])
                .unwrap();
            Err::<(), _>("disk unavailable")
        });

        assert_eq!(
            result,
            Err(DurableCommitError::WriteFailed("disk unavailable"))
        );
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM durable_rollback", [], |row| {
                    row.get::<_, u32>(0)
                })
                .unwrap(),
            0,
            "the fence-owned transaction must roll back adapter writes on callback failure"
        );
        assert_eq!(fence.persisted_revision(), DirtyRevision::default());
        assert!(tracker.is_dirty());
        assert_eq!(
            guarded.restore_persistence_state(),
            Err(PersistenceStateAlreadyIssued),
            "a failed writer must not be bypassed by restoring a new clean tracker/fence"
        );
    }

    #[test]
    fn durable_commit_begin_transaction_failure_preserves_writer_and_dirty_state() {
        let descriptor = SliceDescriptor {
            write_binding: TEST_BINDING,
            ..basic_descriptor("player.begin_transaction_failure", 10)
        };
        let (_registry, mut guarded) = activate(
            &descriptor,
            SliceLoad::<u32, &str>::loaded(9),
            "player:begin_transaction_failure",
            DirtyRevision::default(),
            || 0,
            |_| 0,
        );
        let (mut tracker, mut fence) = guarded.restore_persistence_state().unwrap();
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch("BEGIN DEFERRED TRANSACTION")
            .expect("the outer transaction must be established before the fence commit");

        guarded.mutate(&mut tracker, |value| *value = 10).unwrap();
        let snapshot = tracker
            .begin_snapshot(
                guarded.write_permit(WriteOutlet::Autosave).unwrap(),
                |value| *value,
            )
            .unwrap()
            .unwrap();
        let mut writer_calls = 0;
        let result = fence.commit(&mut connection, snapshot, |_request| {
            writer_calls += 1;
            Ok::<(), &str>(())
        });

        assert!(matches!(
            result,
            Err(DurableCommitError::BeginTransaction(_))
        ));
        assert_eq!(
            writer_calls, 0,
            "the writer must not run before a transaction exists"
        );
        assert_eq!(
            fence.persisted_revision(),
            DirtyRevision::default(),
            "a transaction-open failure must not advance the durable revision"
        );
        assert!(
            tracker.is_dirty(),
            "the dirty snapshot must remain retryable after transaction-open failure"
        );
        connection
            .execute_batch("ROLLBACK")
            .expect("the injected outer transaction must be cleaned up");
    }

    #[test]
    fn durable_commit_rejects_success_without_a_current_transaction_write() {
        let descriptor = SliceDescriptor {
            write_binding: TEST_BINDING,
            ..basic_descriptor("player.no_write", 10)
        };
        let (_registry, mut guarded) = activate(
            &descriptor,
            SliceLoad::<u32, &str>::loaded(9),
            "player:no_write",
            DirtyRevision::default(),
            || 0,
            |_| 0,
        );
        let (mut tracker, mut fence) = guarded.restore_persistence_state().unwrap();
        let mut connection = Connection::open_in_memory().unwrap();

        guarded.mutate(&mut tracker, |value| *value = 10).unwrap();
        let permit = guarded.write_permit(WriteOutlet::Autosave).unwrap();
        let snapshot = tracker
            .begin_snapshot(permit, |value| *value)
            .unwrap()
            .unwrap();
        let result = fence.commit(&mut connection, snapshot, |_request| Ok::<(), &str>(()));

        assert_eq!(result, Err(DurableCommitError::MissingDurableWrite));
        assert_eq!(fence.persisted_revision(), DirtyRevision::default());
        assert!(tracker.is_dirty());
    }

    #[test]
    fn durable_commit_rejects_write_to_attached_memory_database() {
        let descriptor = SliceDescriptor {
            write_binding: TEST_BINDING,
            ..basic_descriptor("player.attached_database", 10)
        };
        let (_registry, mut guarded) = activate(
            &descriptor,
            SliceLoad::<u32, &str>::loaded(9),
            "player:attached_database",
            DirtyRevision::default(),
            || 0,
            |_| 0,
        );
        let (mut tracker, mut fence) = guarded.restore_persistence_state().unwrap();
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "ATTACH ':memory:' AS receipt_only;\
                 CREATE TABLE receipt_only.rows (value INTEGER NOT NULL)",
            )
            .unwrap();

        guarded.mutate(&mut tracker, |value| *value = 10).unwrap();
        let permit = guarded.write_permit(WriteOutlet::Autosave).unwrap();
        let snapshot = tracker
            .begin_snapshot(permit, |value| *value)
            .unwrap()
            .unwrap();
        let result = fence.commit(&mut connection, snapshot, |request| {
            request
                .execute_serialized(
                    "INSERT INTO receipt_only.rows (value) VALUES (?1)",
                    [*request.payload()],
                )
                .map_err(|_| "attached database must not mint a durable receipt")
        });

        assert_eq!(
            result,
            Err(DurableCommitError::WriteFailed(
                "attached database must not mint a durable receipt"
            ))
        );
        assert_eq!(fence.persisted_revision(), DirtyRevision::default());
        assert!(tracker.is_dirty());
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM receipt_only.rows", [], |row| {
                    row.get::<_, u32>(0)
                })
                .unwrap(),
            0,
            "the rejected callback must not write even the attached in-memory database"
        );
    }

    #[test]
    fn durable_commit_rejects_write_to_temp_database() {
        let descriptor = SliceDescriptor {
            write_binding: TEST_BINDING,
            ..basic_descriptor("player.temp_database", 10)
        };
        let (_registry, mut guarded) = activate(
            &descriptor,
            SliceLoad::<u32, &str>::loaded(9),
            "player:temp_database",
            DirtyRevision::default(),
            || 0,
            |_| 0,
        );
        let (mut tracker, mut fence) = guarded.restore_persistence_state().unwrap();
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch("CREATE TEMP TABLE receipt_only (value INTEGER NOT NULL)")
            .unwrap();

        guarded.mutate(&mut tracker, |value| *value = 10).unwrap();
        let permit = guarded.write_permit(WriteOutlet::Autosave).unwrap();
        let snapshot = tracker
            .begin_snapshot(permit, |value| *value)
            .unwrap()
            .unwrap();
        let result = fence.commit(&mut connection, snapshot, |request| {
            request
                .execute_serialized(
                    "INSERT INTO receipt_only (value) VALUES (?1)",
                    [*request.payload()],
                )
                .map_err(|_| "temp database must not mint a durable receipt")
        });

        assert_eq!(
            result,
            Err(DurableCommitError::WriteFailed(
                "temp database must not mint a durable receipt"
            ))
        );
        assert_eq!(fence.persisted_revision(), DirtyRevision::default());
        assert!(tracker.is_dirty());
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM receipt_only", [], |row| {
                    row.get::<_, u32>(0)
                })
                .unwrap(),
            0,
            "the rejected callback must not write even the temporary database"
        );
    }

    #[test]
    fn durable_commit_rejects_stale_sqlite_change_count_from_non_dml_statement() {
        let descriptor = SliceDescriptor {
            write_binding: TEST_BINDING,
            ..basic_descriptor("player.stale_change_count", 10)
        };
        let (_registry, mut guarded) = activate(
            &descriptor,
            SliceLoad::<u32, &str>::loaded(9),
            "player:stale_change_count",
            DirtyRevision::default(),
            || 0,
            |_| 0,
        );
        let (mut tracker, mut fence) = guarded.restore_persistence_state().unwrap();
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE durable_change_count (value INTEGER NOT NULL);\
                 INSERT INTO durable_change_count (value) VALUES (9)",
            )
            .unwrap();

        guarded.mutate(&mut tracker, |value| *value = 10).unwrap();
        let permit = guarded.write_permit(WriteOutlet::Autosave).unwrap();
        let snapshot = tracker
            .begin_snapshot(permit, |value| *value)
            .unwrap()
            .unwrap();
        let result = fence.commit(&mut connection, snapshot, |request| {
            request
                .execute_serialized("SAVEPOINT receipt_only", [])
                .map_err(|_| "non-DML statement must not mint a durable receipt")
        });

        assert_eq!(
            result,
            Err(DurableCommitError::WriteFailed(
                "non-DML statement must not mint a durable receipt"
            ))
        );
        assert_eq!(fence.persisted_revision(), DirtyRevision::default());
        assert!(tracker.is_dirty());
        assert_eq!(
            connection
                .query_row("SELECT value FROM durable_change_count", [], |row| {
                    row.get::<_, u32>(0)
                })
                .unwrap(),
            9,
            "a stale connection-level change count must not acknowledge this snapshot"
        );
    }

    #[test]
    fn durable_commit_accepts_single_statement_with_trivia_tail() {
        let descriptor = SliceDescriptor {
            write_binding: TEST_BINDING,
            ..basic_descriptor("player.sql_trivia", 10)
        };
        let (_registry, mut guarded) = activate(
            &descriptor,
            SliceLoad::<u32, &str>::loaded(9),
            "player:sql_trivia",
            DirtyRevision::default(),
            || 0,
            |_| 0,
        );
        let (mut tracker, mut fence) = guarded.restore_persistence_state().unwrap();
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE durable_trivia (subject TEXT PRIMARY KEY, value INTEGER NOT NULL)",
            )
            .unwrap();

        guarded.mutate(&mut tracker, |value| *value = 10).unwrap();
        let permit = guarded.write_permit(WriteOutlet::Autosave).unwrap();
        let snapshot = tracker
            .begin_snapshot(permit, |value| *value)
            .unwrap()
            .unwrap();
        let receipt = fence
            .commit(&mut connection, snapshot, |request| {
                request
                    .execute_serialized(
                        "INSERT INTO durable_trivia (subject, value) VALUES (?1, ?2); -- audit-safe trailing comment\n",
                        (request.subject_key().0.as_str(), *request.payload()),
                    )
                    .map_err(|_| "one statement plus trivia must remain durable")
            })
            .unwrap();

        assert_eq!(fence.persisted_revision(), DirtyRevision::new(1));
        assert_eq!(
            tracker.acknowledge(receipt),
            DirtyAcknowledgement::Acknowledged
        );
        assert!(!tracker.is_dirty());
        assert_eq!(
            connection
                .query_row(
                    "SELECT value FROM durable_trivia WHERE subject = 'player:sql_trivia'",
                    [],
                    |row| row.get::<_, u32>(0),
                )
                .unwrap(),
            10
        );
    }

    #[test]
    fn durable_commit_rejects_dml_with_nontrivial_sql_tail() {
        let descriptor = SliceDescriptor {
            write_binding: TEST_BINDING,
            ..basic_descriptor("player.sql_tail", 10)
        };
        let (_registry, mut guarded) = activate(
            &descriptor,
            SliceLoad::<u32, &str>::loaded(9),
            "player:sql_tail",
            DirtyRevision::default(),
            || 0,
            |_| 0,
        );
        let (mut tracker, mut fence) = guarded.restore_persistence_state().unwrap();
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE durable_tail (subject TEXT PRIMARY KEY, value INTEGER NOT NULL);\
                 CREATE TABLE unrelated_tail (value INTEGER NOT NULL);\
                 INSERT INTO unrelated_tail (value) VALUES (9)",
            )
            .unwrap();

        guarded.mutate(&mut tracker, |value| *value = 10).unwrap();
        let permit = guarded.write_permit(WriteOutlet::Autosave).unwrap();
        let snapshot = tracker
            .begin_snapshot(permit, |value| *value)
            .unwrap()
            .unwrap();
        let result = fence.commit(&mut connection, snapshot, |request| {
            let error = request
                .execute_serialized(
                    "INSERT INTO durable_tail (subject, value) VALUES (?1, ?2); DELETE FROM unrelated_tail",
                    (request.subject_key().0.as_str(), *request.payload()),
                )
                .unwrap_err();
            assert_eq!(
                error,
                DurableWriteExecuteError::Proof(DurableWriteProofError::MultipleStatements)
            );
            Err::<(), _>("nontrivial SQL tail must not mint a durable receipt")
        });

        assert_eq!(
            result,
            Err(DurableCommitError::WriteFailed(
                "nontrivial SQL tail must not mint a durable receipt"
            ))
        );
        assert_eq!(fence.persisted_revision(), DirtyRevision::default());
        assert!(tracker.is_dirty());
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM durable_tail", [], |row| {
                    row.get::<_, u32>(0)
                })
                .unwrap(),
            0,
            "the rejected multi-statement request must roll back its DML prefix"
        );
        assert_eq!(
            connection
                .query_row("SELECT value FROM unrelated_tail", [], |row| {
                    row.get::<_, u32>(0)
                })
                .unwrap(),
            9,
            "the ignored tail must not become an alternate write path"
        );
    }

    #[test]
    fn durable_commit_rejects_schema_mutating_cas_statement_with_cascade_changes() {
        let descriptor = SliceDescriptor {
            write_binding: TEST_BINDING,
            write_ordering: WriteOrdering::PersistedRevisionCas,
            ..basic_descriptor("player.schema_cascade", 10)
        };
        let (_registry, mut guarded) = activate(
            &descriptor,
            SliceLoad::<u32, &str>::loaded(9),
            "player:schema_cascade",
            DirtyRevision::default(),
            || 0,
            |_| 0,
        );
        let (mut tracker, mut fence) = guarded.restore_persistence_state().unwrap();
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 CREATE TABLE parent (id INTEGER PRIMARY KEY);
                 CREATE TABLE child (
                     parent_id INTEGER NOT NULL REFERENCES parent(id) ON DELETE CASCADE
                 );
                 INSERT INTO parent (id) VALUES (1);
                 INSERT INTO child (parent_id) VALUES (1);
                 CREATE TABLE durable_schema_cascade (
                     subject TEXT PRIMARY KEY,
                     revision INTEGER NOT NULL,
                     value INTEGER NOT NULL
                 );
                 INSERT INTO durable_schema_cascade (subject, revision, value)
                 VALUES ('player:schema_cascade', 0, 9);",
            )
            .unwrap();

        guarded.mutate(&mut tracker, |value| *value = 10).unwrap();
        let permit = guarded.write_permit(WriteOutlet::Autosave).unwrap();
        let snapshot = tracker
            .begin_snapshot(permit, |value| *value)
            .unwrap()
            .unwrap();
        let result = fence.commit(&mut connection, snapshot, |request| {
            request
                .execute_cas("DROP TABLE parent", [])
                .map_err(|_| "schema mutation must not mint a durable receipt")
        });

        assert_eq!(
            result,
            Err(DurableCommitError::WriteFailed(
                "schema mutation must not mint a durable receipt"
            ))
        );
        assert_eq!(fence.persisted_revision(), DirtyRevision::default());
        assert!(tracker.is_dirty());
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM parent", [], |row| {
                    row.get::<_, u32>(0)
                })
                .unwrap(),
            1,
            "a schema-mutating statement must roll back before it can acknowledge the snapshot"
        );
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM child", [], |row| row.get::<_, u32>(0))
                .unwrap(),
            1,
            "foreign-key cascade side effects must roll back with the rejected DDL"
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT revision, value FROM durable_schema_cascade WHERE subject = 'player:schema_cascade'",
                    [],
                    |row| Ok((row.get::<_, u64>(0)?, row.get::<_, u32>(1)?)),
                )
                .unwrap(),
            (0, 9),
            "the durable snapshot row must remain unchanged after DDL rejection"
        );
    }

    #[test]
    fn durable_commit_failure_mints_no_receipt_and_rolls_back_write() {
        let descriptor = SliceDescriptor {
            write_binding: TEST_BINDING,
            ..basic_descriptor("player.commit_failure", 10)
        };
        let (_registry, mut guarded) = activate(
            &descriptor,
            SliceLoad::<u32, &str>::loaded(9),
            "player:commit_failure",
            DirtyRevision::default(),
            || 0,
            |_| 0,
        );
        let (mut tracker, mut fence) = guarded.restore_persistence_state().unwrap();
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch("PRAGMA foreign_keys = ON")
            .unwrap();
        connection
            .execute_batch(
                "CREATE TABLE parent (id INTEGER PRIMARY KEY);\
                 CREATE TABLE child (parent_id INTEGER NOT NULL REFERENCES parent(id) DEFERRABLE INITIALLY DEFERRED)",
            )
            .unwrap();

        guarded.mutate(&mut tracker, |value| *value = 10).unwrap();
        let permit = guarded.write_permit(WriteOutlet::Autosave).unwrap();
        let snapshot = tracker
            .begin_snapshot(permit, |value| *value)
            .unwrap()
            .unwrap();
        let result = fence.commit(&mut connection, snapshot, |request| {
            request
                .execute_serialized(
                    "INSERT INTO child (parent_id) VALUES (?1)",
                    [*request.payload()],
                )
                .map_err(|_| "child insert failed")
        });

        assert!(matches!(result, Err(DurableCommitError::CommitFailed(_))));
        assert_eq!(fence.persisted_revision(), DirtyRevision::default());
        assert!(tracker.is_dirty());
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM child", [], |row| row.get::<_, u32>(0))
                .unwrap(),
            0,
            "a failed SQLite commit must not leave a durable row or receipt"
        );
    }

    #[test]
    fn durable_revision_cas_rejection_keeps_dirty_until_one_row_write_commits() {
        let descriptor = SliceDescriptor {
            write_binding: TEST_BINDING,
            write_ordering: WriteOrdering::PersistedRevisionCas,
            ..basic_descriptor("player.cas", 10)
        };
        let (_registry, mut guarded) = activate(
            &descriptor,
            SliceLoad::<u32, &str>::loaded(9),
            "player:cas",
            DirtyRevision::new(41),
            || 0,
            |_| 0,
        );
        let (mut tracker, mut fence) = guarded.restore_persistence_state().unwrap();
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE durable_cas (subject TEXT PRIMARY KEY, revision INTEGER NOT NULL, value INTEGER NOT NULL);\
                 INSERT INTO durable_cas (subject, revision, value) VALUES ('player:cas', 40, 9);\
                 INSERT INTO durable_cas (subject, revision, value) VALUES ('player:other', 40, 7)",
            )
            .unwrap();

        guarded.mutate(&mut tracker, |value| *value = 10).unwrap();
        let rejected_permit = guarded.write_permit(WriteOutlet::Autosave).unwrap();
        let rejected_snapshot = tracker
            .begin_snapshot(rejected_permit, |value| *value)
            .unwrap()
            .unwrap();
        let rejected = fence.commit(&mut connection, rejected_snapshot, |request| {
            request
                .execute_cas(
                    "UPDATE durable_cas SET revision = ?1, value = ?2 WHERE subject = ?3 AND revision = ?4",
                    (
                        request.write_revision().get() as i64,
                        *request.payload() as i64,
                        request.subject_key().0.as_str(),
                        request.expected_persisted_revision().get() as i64,
                    ),
                )
                .map_err(|_| "cas rejected")
        });
        assert_eq!(
            rejected,
            Err(DurableCommitError::WriteFailed("cas rejected"))
        );
        assert_eq!(fence.persisted_revision(), DirtyRevision::new(41));
        assert!(tracker.is_dirty());
        assert_eq!(
            connection
                .query_row(
                    "SELECT revision, value FROM durable_cas WHERE subject = 'player:cas'",
                    [],
                    |row| Ok((row.get::<_, u64>(0)?, row.get::<_, u32>(1)?)),
                )
                .unwrap(),
            (40, 9)
        );

        connection
            .execute(
                "UPDATE durable_cas SET revision = ?1 WHERE subject = 'player:cas'",
                [DirtyRevision::new(41).get() as i64],
            )
            .unwrap();

        let multi_row_permit = guarded.write_permit(WriteOutlet::Autosave).unwrap();
        let multi_row_snapshot = tracker
            .begin_snapshot(multi_row_permit, |value| *value)
            .unwrap()
            .unwrap();
        let multi_row = fence.commit(&mut connection, multi_row_snapshot, |request| {
            request
                .execute_cas(
                    "UPDATE durable_cas SET revision = ?1, value = ?2 WHERE revision >= ?3",
                    (
                        request.write_revision().get() as i64,
                        *request.payload() as i64,
                        DirtyRevision::new(40).get() as i64,
                    ),
                )
                .map_err(|_| "cas matched multiple rows")
        });
        assert_eq!(
            multi_row,
            Err(DurableCommitError::WriteFailed("cas matched multiple rows"))
        );
        assert_eq!(fence.persisted_revision(), DirtyRevision::new(41));
        assert!(tracker.is_dirty());
        assert_eq!(
            connection
                .query_row(
                    "SELECT revision, value FROM durable_cas WHERE subject = 'player:other'",
                    [],
                    |row| Ok((row.get::<_, u64>(0)?, row.get::<_, u32>(1)?)),
                )
                .unwrap(),
            (40, 7),
            "a CAS predicate matching multiple rows must roll the whole transaction back"
        );

        let accepted_permit = guarded.write_permit(WriteOutlet::Autosave).unwrap();
        let accepted_snapshot = tracker
            .begin_snapshot(accepted_permit, |value| *value)
            .unwrap()
            .unwrap();
        let receipt = fence
            .commit(&mut connection, accepted_snapshot, |request| {
                request
                    .execute_cas(
                        "UPDATE durable_cas SET revision = ?1, value = ?2 WHERE subject = ?3 AND revision = ?4",
                        (
                            request.write_revision().get() as i64,
                            *request.payload() as i64,
                            request.subject_key().0.as_str(),
                            request.expected_persisted_revision().get() as i64,
                        ),
                    )
                    .map_err(|_| "cas rejected")
            })
            .unwrap();
        assert_eq!(fence.persisted_revision(), DirtyRevision::new(42));
        assert_eq!(
            tracker.acknowledge(receipt),
            DirtyAcknowledgement::Acknowledged
        );
        assert!(!tracker.is_dirty());
        assert_eq!(
            connection
                .query_row(
                    "SELECT revision, value FROM durable_cas WHERE subject = 'player:cas'",
                    [],
                    |row| Ok((row.get::<_, u64>(0)?, row.get::<_, u32>(1)?)),
                )
                .unwrap(),
            (42, 10)
        );
    }

    #[test]
    fn durable_write_is_bound_to_domain_authority_and_monotonic_revision() {
        const OTHER_BINDING: WriteBinding = WriteBinding::new(
            WriteDomain::new("test.other"),
            WriteAuthority::new("test.other.writer"),
        );
        let descriptor = Box::leak(Box::new(SliceDescriptor {
            write_binding: TEST_BINDING,
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
            .activate_test_subject(
                SliceLoad::<u32, &str>::loaded(9),
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
            SliceLoad::<u32, &str>::loaded(9),
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
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE durable_bound (subject TEXT PRIMARY KEY, revision INTEGER NOT NULL, value INTEGER NOT NULL);\
                 INSERT INTO durable_bound (subject, revision, value) VALUES ('player:bound', 41, 9)",
            )
            .unwrap();
        guarded.mutate(&mut tracker, |value| *value = 10).unwrap();
        let permit = guarded.write_permit(WriteOutlet::Shutdown).unwrap();
        let snapshot = tracker
            .begin_snapshot(permit, |value| *value)
            .unwrap()
            .unwrap();
        let receipt = fence
            .commit(&mut connection, snapshot, |request| {
                assert_eq!(request.binding(), TEST_BINDING);
                assert_eq!(request.subject_key(), &subject_key("player:bound"));
                assert_eq!(*request.payload(), 10);
                assert_eq!(
                    request.expected_persisted_revision(),
                    DirtyRevision::new(41)
                );
                assert_eq!(request.write_revision(), DirtyRevision::new(42));
                assert_eq!(request.ordering(), WriteOrdering::PersistedRevisionCas);
                request
                    .execute_cas(
                        "UPDATE durable_bound SET revision = ?1, value = ?2 WHERE subject = ?3 AND revision = ?4",
                        (
                            request.write_revision().get() as i64,
                            *request.payload() as i64,
                            request.subject_key().0.as_str(),
                            request.expected_persisted_revision().get() as i64,
                        ),
                    )
                    .map_err(|_| "cas rejected")
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
    fn stale_snapshot_and_receipt_ordering_matrix_preserves_newest_dirty_revision() {
        let descriptor = SliceDescriptor {
            write_binding: TEST_BINDING,
            ..basic_descriptor("player.stale_matrix", 10)
        };
        let (_registry, mut guarded) = activate(
            &descriptor,
            SliceLoad::<u32, &str>::loaded(9),
            "player:stale-matrix",
            DirtyRevision::new(40),
            || 0,
            |_| 0,
        );
        let (mut tracker, mut fence) = guarded.restore_persistence_state().unwrap();
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE durable_stale_matrix (subject TEXT PRIMARY KEY, value INTEGER NOT NULL);\
                 INSERT INTO durable_stale_matrix (subject, value) VALUES ('player:stale-matrix', 9)",
            )
            .unwrap();

        guarded.mutate(&mut tracker, |value| *value = 10).unwrap();
        let equal_first = tracker
            .begin_snapshot(
                guarded.write_permit(WriteOutlet::Autosave).unwrap(),
                |value| *value,
            )
            .unwrap()
            .unwrap();
        let equal_replay = tracker
            .begin_snapshot(
                guarded.write_permit(WriteOutlet::Autosave).unwrap(),
                |value| *value,
            )
            .unwrap()
            .unwrap();
        let first_receipt = fence
            .commit(&mut connection, equal_first, |request| {
                request
                    .execute_serialized(
                        "UPDATE durable_stale_matrix SET value = ?1 WHERE subject = ?2",
                        (*request.payload(), request.subject_key().0.as_str()),
                    )
                    .map_err(|_| "initial write failed")
            })
            .unwrap();
        assert_eq!(
            tracker.acknowledge(first_receipt),
            DirtyAcknowledgement::Acknowledged
        );
        let mut equal_replay_writer_called = false;
        assert_eq!(
            fence.commit(&mut connection, equal_replay, |_request| {
                equal_replay_writer_called = true;
                Ok::<(), &str>(())
            }),
            Err(DurableCommitError::StaleRevision {
                persisted: DirtyRevision::new(41),
                attempted: DirtyRevision::new(41),
            })
        );
        assert!(!equal_replay_writer_called);

        guarded.mutate(&mut tracker, |value| *value = 11).unwrap();
        let older_snapshot = tracker
            .begin_snapshot(
                guarded.write_permit(WriteOutlet::Autosave).unwrap(),
                |value| *value,
            )
            .unwrap()
            .unwrap();
        guarded.mutate(&mut tracker, |value| *value = 12).unwrap();
        let newer_snapshot = tracker
            .begin_snapshot(
                guarded.write_permit(WriteOutlet::Autosave).unwrap(),
                |value| *value,
            )
            .unwrap()
            .unwrap();
        let newer_receipt = fence
            .commit(&mut connection, newer_snapshot, |request| {
                request
                    .execute_serialized(
                        "UPDATE durable_stale_matrix SET value = ?1 WHERE subject = ?2",
                        (*request.payload(), request.subject_key().0.as_str()),
                    )
                    .map_err(|_| "newest write failed")
            })
            .unwrap();
        let mut older_writer_called = false;
        assert_eq!(
            fence.commit(&mut connection, older_snapshot, |_request| {
                older_writer_called = true;
                Ok::<(), &str>(())
            }),
            Err(DurableCommitError::StaleRevision {
                persisted: DirtyRevision::new(43),
                attempted: DirtyRevision::new(42),
            })
        );
        assert!(!older_writer_called);

        guarded.mutate(&mut tracker, |value| *value = 13).unwrap();
        assert_eq!(
            tracker.acknowledge(newer_receipt),
            DirtyAcknowledgement::Stale
        );
        assert!(tracker.is_dirty());
        assert_eq!(tracker.current_revision(), DirtyRevision::new(44));
        assert_eq!(fence.persisted_revision(), DirtyRevision::new(43));
        assert_eq!(
            connection
                .query_row(
                    "SELECT value FROM durable_stale_matrix WHERE subject = 'player:stale-matrix'",
                    [],
                    |row| row.get::<_, u32>(0),
                )
                .unwrap(),
            12
        );
    }

    #[test]
    fn dirty_revision_overflow_rejects_mutation_without_changing_value() {
        let descriptor = SliceDescriptor {
            write_binding: TEST_BINDING,
            ..basic_descriptor("player.overflow", 10)
        };
        let (_registry, mut guarded) = activate(
            &descriptor,
            SliceLoad::<u32, &str>::loaded(9),
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
