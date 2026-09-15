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
    pub(in crate::persistence) reconnect_activation: Option<ReconnectActivationCapability>,
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
pub type SlicePreflightHook = fn(&mut World, &SliceRunContext) -> SliceRunResult;
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
    pub(in crate::persistence) fn activate_test_subject<T, E>(
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
#[path = "slice_tests.rs"]
mod tests;
