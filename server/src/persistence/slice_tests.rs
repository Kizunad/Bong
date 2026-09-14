#![allow(dead_code, unused_imports)]

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

fn noop_preflight(_world: &mut World, context: &SliceRunContext) -> SliceRunResult {
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
        dispatch_shutdown_flushes(&mut world, ShutdownFlushRequest::NotRequested, &clock).unwrap();

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

fn handoff_load_activation_first(world: &mut World, context: &SliceRunContext) -> SliceRunResult {
    trace_handoff_load(world, context, SliceId::new("player.activation_first"))
}

fn handoff_load_activation_second(world: &mut World, context: &SliceRunContext) -> SliceRunResult {
    trace_handoff_load(world, context, SliceId::new("player.activation_second"))
}

fn handoff_preflight(world: &mut World, context: &SliceRunContext) -> SliceRunResult {
    assert_eq!(context.reason, SliceRunReason::ReconnectPreflight);
    {
        let mut trace = world.resource_mut::<HandoffTrace>();
        trace.events.push((
            context.reason,
            context.runtime_tick,
            context.wall_unix_millis,
            context.handoff_key.clone().unwrap(),
        ));
    }
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
        .all(|event| { event.1 == 400 && event.2 == 49_999 && event.3 == "offline:clock_once" }));
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
            SliceRunReason::ReconnectPreflight,
            SliceRunReason::ReconnectPreflight,
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

    for (name, fail_second, block_second) in [("error", true, false), ("blocked", false, true)] {
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

fn atomic_preflight_first(_world: &mut World, context: &SliceRunContext) -> SliceRunResult {
    assert_eq!(context.reason, SliceRunReason::ReconnectPreflight);
    Ok(SliceRunOutcome::Clean)
}

fn atomic_preflight_second(world: &mut World, context: &SliceRunContext) -> SliceRunResult {
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
        let mut world = atomic_reconnect_world(first, second, injected, InjectedHookResult::Clean);
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
        let retry = dispatch_reconnect_handoff(&mut world, token("player:atomic"), &clock).unwrap();
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
        let subject = subject_key("player:atomic");
        let registry = world.resource::<PersistenceSliceRegistry>();
        assert!(
            !registry.active_subject_domain(&subject, first.write_binding.domain()),
            "rebase failure/block must release the first attempted reconnect lease"
        );
        assert!(
            !registry.active_subject_domain(&subject, second.write_binding.domain()),
            "rebase failure/block must release the second attempted reconnect lease"
        );
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
        let mut world = atomic_reconnect_world(first, second, InjectedHookResult::Clean, injected);
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
        let retry = dispatch_reconnect_handoff(&mut world, token("player:atomic"), &clock).unwrap();
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
        let error =
            dispatch_reconnect_handoff(&mut world, token("player:activation"), &clock).unwrap_err();
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
