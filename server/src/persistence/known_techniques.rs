//! Production persistence adapter for the KnownTechniques player slice.

use super::*;

pub(super) struct KnownTechniquesPersistenceSlice;

impl PersistenceSlice for KnownTechniquesPersistenceSlice {
    fn descriptor() -> &'static SliceDescriptor {
        &KNOWN_TECHNIQUES_SLICE_DESCRIPTOR
    }
}

pub(super) const KNOWN_TECHNIQUES_SLICE_ID: SliceId = SliceId::new("player.known_techniques");
pub(super) const KNOWN_TECHNIQUES_SLICE_DESCRIPTOR: SliceDescriptor = SliceDescriptor {
    id: KNOWN_TECHNIQUES_SLICE_ID,
    scope: SliceScope::PlayerEntity,
    order: 10,
    load_failure: LoadFailurePolicy::BlockWrites,
    time_basis: TimeBasis::None,
    write_binding: WriteBinding::new(
        WriteDomain::new("player.known_techniques"),
        WriteAuthority::new("persistence.known_techniques"),
    ),
    write_ordering: WriteOrdering::Serialized,
    autosave: AutosavePolicy::Disabled,
    hydrate: Some(hydrate_known_techniques_slice),
    reconnect_preflight: Some(preflight_known_techniques_slice),
    reconnect_cleanup: Some(cleanup_known_techniques_slice),
    rebase: None,
    disconnect_save: Some(save_known_techniques_disconnect_slice),
    shutdown_flush: Some(flush_known_techniques_shutdown_slice),
};

#[derive(Debug)]
pub(super) struct KnownTechniquesActivation {
    pub(super) entity: Entity,
    pub(super) guarded: GuardedSlice<KnownTechniques, String>,
    pub(super) tracker: DirtyTracker,
    pub(super) fence: PersistedRevisionFence,
}

#[derive(Debug, Default)]
pub(super) struct KnownTechniquesActivations(pub(super) HashMap<String, KnownTechniquesActivation>);

impl Resource for KnownTechniquesActivations {}

#[derive(Debug, Default)]
pub(super) struct PendingKnownTechniquesHandoffs(pub(super) HashMap<String, Entity>);

impl Resource for PendingKnownTechniquesHandoffs {}

#[derive(Debug, Default)]
pub(super) struct PendingKnownTechniquesCandidates(pub(super) HashMap<String, Vec<Entity>>);

impl Resource for PendingKnownTechniquesCandidates {}

#[derive(Debug, Default)]
pub(super) struct KnownTechniquesRetryEntry {
    pub(super) attempts: u8,
    pub(super) next_attempt_frame: u64,
    pub(super) next_log_frame: u64,
}

#[derive(Debug, Default)]
pub(super) struct KnownTechniquesReconnectState {
    pub(super) frame: u64,
    pub(super) retries: HashMap<String, KnownTechniquesRetryEntry>,
    pub(super) preflight_loads: Mutex<HashMap<String, Result<Option<KnownTechniques>, String>>>,
}

impl Resource for KnownTechniquesReconnectState {}

pub(super) const KNOWN_TECHNIQUES_RETRY_MAX_ATTEMPTS: u8 = 8;
pub(super) const KNOWN_TECHNIQUES_RETRY_MAX_BACKOFF_FRAMES: u64 = 64;
pub(super) const KNOWN_TECHNIQUES_RETRY_LOG_INTERVAL_FRAMES: u64 = 64;

pub(super) fn begin_known_techniques_retry(
    state: &mut KnownTechniquesReconnectState,
    subject: &str,
    frame: u64,
) -> bool {
    let entry = state.retries.entry(subject.to_string()).or_default();
    if frame < entry.next_attempt_frame {
        return false;
    }
    entry.attempts = entry
        .attempts
        .saturating_add(1)
        .min(KNOWN_TECHNIQUES_RETRY_MAX_ATTEMPTS);
    true
}

pub(super) fn record_known_techniques_retry_failure(
    state: &mut KnownTechniquesReconnectState,
    subject: &str,
    frame: u64,
) -> bool {
    let entry = state.retries.entry(subject.to_string()).or_default();
    let capped = entry.attempts >= KNOWN_TECHNIQUES_RETRY_MAX_ATTEMPTS;
    if capped {
        entry.attempts = KNOWN_TECHNIQUES_RETRY_MAX_ATTEMPTS;
    }
    let backoff_shift = entry.attempts.saturating_sub(1).min(6);
    let backoff = 1_u64 << backoff_shift;
    entry.next_attempt_frame =
        frame.saturating_add(backoff.min(KNOWN_TECHNIQUES_RETRY_MAX_BACKOFF_FRAMES));
    capped
}

pub(super) fn known_techniques_retry_log_allowed(
    state: &mut KnownTechniquesReconnectState,
    subject: &str,
    frame: u64,
) -> bool {
    let entry = state.retries.entry(subject.to_string()).or_default();
    if frame < entry.next_log_frame {
        return false;
    }
    entry.next_log_frame = frame.saturating_add(KNOWN_TECHNIQUES_RETRY_LOG_INTERVAL_FRAMES);
    true
}

pub(super) fn clear_known_techniques_retry(
    state: &mut KnownTechniquesReconnectState,
    subject: &str,
) {
    state.retries.remove(subject);
}

pub(super) fn clear_stale_known_techniques_retries(
    state: &mut KnownTechniquesReconnectState,
    pending_subjects: &HashSet<String>,
) {
    let stale_subjects = state
        .retries
        .keys()
        .filter(|subject| !pending_subjects.contains(subject.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    for subject in stale_subjects {
        clear_known_techniques_retry(state, subject.as_str());
    }
}

pub(super) fn known_techniques_live_activation(world: &World, subject: &str) -> Option<Entity> {
    world
        .resource::<KnownTechniquesActivations>()
        .0
        .get(subject)
        .filter(|activation| world.get::<Client>(activation.entity).is_some())
        .map(|activation| activation.entity)
}

pub(super) fn reconnect_report_is_live_duplicate(report: &ReconnectHandoffReport) -> bool {
    report.failures.iter().any(|failure| {
        failure.reason == SliceRunReason::ReconnectPreflight
            && failure.error.message() == "known techniques subject already has a live activation"
    })
}

pub(super) const KNOWN_TECHNIQUES_UPSERT: &str = "
    INSERT INTO player_known_techniques (
        username,
        known_techniques_json,
        schema_version,
        last_updated_wall
    ) VALUES (?1, ?2, ?3, ?4)
    ON CONFLICT(username) DO UPDATE SET
        known_techniques_json = excluded.known_techniques_json,
        schema_version = excluded.schema_version,
        last_updated_wall = excluded.last_updated_wall
";

pub(super) fn persist_known_techniques_activation(
    activation: &mut KnownTechniquesActivation,
    persistence: &PlayerStatePersistence,
    outlet: WriteOutlet,
) -> Result<SliceRunOutcome, SliceRunError> {
    let permit = activation
        .guarded
        .write_permit(outlet)
        .map_err(|error| SliceRunError::new(error.to_string()))?;
    let Some(snapshot) = activation
        .tracker
        .begin_snapshot(permit, Clone::clone)
        .map_err(|error| SliceRunError::new(error.to_string()))?
    else {
        return Ok(SliceRunOutcome::Clean);
    };
    let username = player_username_from_character_id(snapshot.subject_key().as_str())
        .ok_or_else(|| SliceRunError::new("known techniques subject is not a player identity"))?
        .to_string();
    let known_techniques_json = serde_json::to_string(snapshot.payload())
        .map_err(|error| SliceRunError::new(error.to_string()))?;
    let mut connection = open_player_connection(persistence)
        .map_err(|error| SliceRunError::new(error.to_string()))?;
    let receipt = activation
        .fence
        .commit(&mut connection, snapshot, |request| {
            request.execute_serialized(
                KNOWN_TECHNIQUES_UPSERT,
                params![
                    username,
                    known_techniques_json,
                    PLAYER_ROW_SCHEMA_VERSION,
                    current_unix_seconds()
                ],
            )
        })
        .map_err(|error| SliceRunError::new(format!("{error:?}")))?;
    match activation.tracker.acknowledge(receipt) {
        DirtyAcknowledgement::Acknowledged => Ok(SliceRunOutcome::Flushed),
        acknowledgement => Err(SliceRunError::new(format!(
            "known techniques durable receipt was not acknowledged: {acknowledgement:?}"
        ))),
    }
}

pub(super) fn sync_known_techniques_activation(
    world: &World,
    subject: &str,
    activation: &mut KnownTechniquesActivation,
) -> Result<(), SliceRunError> {
    let Some(current) = world.get::<KnownTechniques>(activation.entity) else {
        return Ok(());
    };
    if current != activation.guarded.value() {
        activation
            .guarded
            .mutate(&mut activation.tracker, |value| *value = current.clone())
            .map_err(|error| SliceRunError::new(format!("{subject}: {error}")))?;
    }
    Ok(())
}

pub(super) fn hydrate_known_techniques_slice(
    world: &mut World,
    context: &SliceRunContext,
) -> SliceRunResult {
    let subject = context
        .handoff_key
        .as_deref()
        .ok_or_else(|| SliceRunError::new("known techniques hydrate has no subject"))?;
    let entity = world
        .resource::<PendingKnownTechniquesHandoffs>()
        .0
        .get(subject)
        .copied()
        .ok_or_else(|| SliceRunError::new("known techniques reconnect target is unavailable"))?;
    if world.get_entity(entity).is_none() {
        cleanup_stale_known_techniques_pending(world);
        return Err(SliceRunError::new(
            "known techniques reconnect target entity is gone",
        ));
    }
    validate_known_techniques_reconnect_target(world, subject, entity)?;
    let loaded = world
        .resource::<KnownTechniquesReconnectState>()
        .preflight_loads
        .lock()
        .map_err(|_| SliceRunError::new("known techniques preflight cache is poisoned"))?
        .remove(subject)
        .ok_or_else(|| SliceRunError::new("known techniques preflight load is unavailable"))?;
    let load = match loaded {
        Ok(Some(value)) => SliceLoad::loaded(value),
        Ok(None) => SliceLoad::missing(),
        Err(error) => SliceLoad::failed(error),
    };
    let activation = context.reconnect_activation()?;
    let missing_default = world
        .get_resource::<TechniqueRegistry>()
        .map_or_else(KnownTechniques::default, KnownTechniques::progression_reset);
    let missing_default_for_rebase = missing_default.clone();
    let mut guarded = world
        .resource_scope(
            |_, registry: valence::prelude::Mut<PersistenceSliceRegistry>| {
                registry.activate(
                    load,
                    KNOWN_TECHNIQUES_SLICE_ID,
                    activation,
                    DirtyRevision::default(),
                    || missing_default,
                    |_| missing_default_for_rebase,
                )
            },
        )
        .map_err(|error| SliceRunError::new(format!("activation failed: {error:?}")))?;
    let failed = guarded.load_status() == slice::SliceLoadStatus::Failed;
    let value = guarded.value().clone();
    let (tracker, fence) = guarded
        .restore_persistence_state()
        .map_err(|error| SliceRunError::new(error.to_string()))?;
    {
        let Some(mut target) = world.get_entity_mut(entity) else {
            cleanup_stale_known_techniques_pending(world);
            return Err(SliceRunError::new(
                "known techniques reconnect target entity disappeared during hydrate",
            ));
        };
        target.insert(value);
        if failed {
            target.insert(KnownTechniquesLoadFailed);
        } else {
            target.remove::<KnownTechniquesLoadFailed>();
        }
    }
    world.resource_mut::<KnownTechniquesActivations>().0.insert(
        subject.to_string(),
        KnownTechniquesActivation {
            entity,
            guarded,
            tracker,
            fence,
        },
    );
    world
        .resource_mut::<PendingKnownTechniquesHandoffs>()
        .0
        .remove(subject);
    Ok(SliceRunOutcome::Clean)
}

pub(super) fn cleanup_stale_known_techniques_pending(world: &mut World) {
    let pending_subjects = world
        .resource::<PendingKnownTechniquesHandoffs>()
        .0
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    let stale_subjects = pending_subjects
        .iter()
        .filter(|subject| {
            let Some(entity) = world
                .resource::<PendingKnownTechniquesHandoffs>()
                .0
                .get(*subject)
                .copied()
            else {
                return true;
            };
            !known_techniques_reconnect_candidate_is_live(world, subject, entity)
        })
        .cloned()
        .collect::<Vec<_>>();
    for subject in stale_subjects {
        let entity = world
            .resource_mut::<PendingKnownTechniquesHandoffs>()
            .0
            .remove(&subject);
        if let Some(entity) = entity {
            if let Some(mut target) = world.get_entity_mut(entity) {
                target.remove::<KnownTechniquesReconnectBlocked>();
                target.remove::<KnownTechniquesReconnectFailed>();
                target.remove::<KnownTechniquesReconnectReady>();
            }
        }
        let mut state = world.resource_mut::<KnownTechniquesReconnectState>();
        state.retries.remove(&subject);
        if let Ok(mut loads) = state.preflight_loads.lock() {
            loads.remove(&subject);
        };
    }

    let subjects = world
        .resource::<PendingKnownTechniquesCandidates>()
        .0
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    let mut stale_candidates = Vec::new();
    for subject in subjects {
        let candidates = world
            .resource::<PendingKnownTechniquesCandidates>()
            .0
            .get(&subject)
            .cloned()
            .unwrap_or_default();
        let live = candidates
            .into_iter()
            .filter(|entity| known_techniques_reconnect_candidate_is_live(world, &subject, *entity))
            .collect::<Vec<_>>();
        if live.is_empty() {
            stale_candidates.push(subject);
        } else {
            world
                .resource_mut::<PendingKnownTechniquesCandidates>()
                .0
                .insert(subject, live);
        }
    }
    for subject in stale_candidates {
        world
            .resource_mut::<PendingKnownTechniquesCandidates>()
            .0
            .remove(&subject);
    }
}

pub(super) fn known_techniques_reconnect_candidate_is_live(
    world: &World,
    subject: &str,
    entity: Entity,
) -> bool {
    let Some(target) = world.get_entity(entity) else {
        return false;
    };
    let Some(username) = target.get::<Username>() else {
        return false;
    };
    target.get::<Client>().is_some()
        && target.get::<Despawned>().is_none()
        && player_username_from_character_id(subject).is_some_and(|expected| username.0 == expected)
}

pub(super) fn promote_known_techniques_candidate(world: &mut World, subject: &str) {
    if world
        .resource::<PendingKnownTechniquesHandoffs>()
        .0
        .contains_key(subject)
        || world
            .resource::<KnownTechniquesActivations>()
            .0
            .contains_key(subject)
    {
        return;
    }
    let candidate = world
        .resource::<PendingKnownTechniquesCandidates>()
        .0
        .get(subject)
        .and_then(|candidates| {
            candidates
                .iter()
                .copied()
                .min_by_key(|entity| entity.index())
        });
    let Some(entity) = candidate else {
        return;
    };
    world
        .resource_mut::<PendingKnownTechniquesCandidates>()
        .0
        .entry(subject.to_string())
        .and_modify(|candidates| candidates.retain(|candidate| *candidate != entity));
    if let Some(mut target) = world.get_entity_mut(entity) {
        target.remove::<KnownTechniquesReconnectBlocked>();
        target.remove::<KnownTechniquesReconnectFailed>();
    }
    world
        .resource_mut::<PendingKnownTechniquesHandoffs>()
        .0
        .insert(subject.to_string(), entity);
}

pub(super) fn validate_known_techniques_reconnect_target(
    world: &World,
    subject: &str,
    entity: Entity,
) -> Result<(), SliceRunError> {
    let Some(target) = world.get_entity(entity) else {
        return Err(SliceRunError::new(
            "known techniques reconnect target entity is gone",
        ));
    };
    let Some(client) = target.get::<Client>() else {
        return Err(SliceRunError::new(
            "known techniques reconnect target is disconnected",
        ));
    };
    let username = target
        .get::<Username>()
        .ok_or_else(|| SliceRunError::new("known techniques reconnect target has no username"))?;
    let expected = player_username_from_character_id(subject)
        .ok_or_else(|| SliceRunError::new("known techniques subject is not a player identity"))?;
    if username.0 != expected {
        return Err(SliceRunError::new(
            "known techniques reconnect target identity mismatch",
        ));
    }
    let _ = client;
    if target.get::<Despawned>().is_some() {
        return Err(SliceRunError::new(
            "known techniques reconnect target is despawned",
        ));
    }
    Ok(())
}

pub(super) fn preflight_known_techniques_slice(
    world: &mut World,
    context: &SliceRunContext,
) -> SliceRunResult {
    let subject = context
        .handoff_key
        .as_deref()
        .ok_or_else(|| SliceRunError::new("known techniques preflight has no subject"))?;
    let target = world
        .resource::<PendingKnownTechniquesHandoffs>()
        .0
        .get(subject)
        .copied()
        .ok_or_else(|| SliceRunError::new("known techniques reconnect target is unavailable"))?;
    validate_known_techniques_reconnect_target(world, subject, target)?;
    if known_techniques_live_activation(world, subject).is_some() {
        return Err(SliceRunError::new(
            "known techniques subject already has a live activation",
        ));
    }
    let has_activation = world
        .resource::<KnownTechniquesActivations>()
        .0
        .contains_key(subject);
    let persistence = world
        .get_resource::<PlayerStatePersistence>()
        .cloned()
        .ok_or_else(|| SliceRunError::new("PlayerStatePersistence is unavailable"))?;
    let username = player_username_from_character_id(subject)
        .ok_or_else(|| SliceRunError::new("known techniques subject is not a player identity"))?;
    let loaded = load_player_known_techniques_slice(&persistence, username);
    let cached = match loaded {
        Ok(value) => Ok(value),
        Err(error) if !has_activation => Err(error.to_string()),
        Err(error) => return Err(SliceRunError::new(error.to_string())),
    };
    world
        .resource::<KnownTechniquesReconnectState>()
        .preflight_loads
        .lock()
        .map_err(|_| SliceRunError::new("known techniques preflight cache is poisoned"))?
        .insert(subject.to_string(), cached);
    Ok(SliceRunOutcome::Clean)
}

pub(super) fn cleanup_known_techniques_slice(world: &mut World, context: &SliceRunContext) {
    let Some(subject) = context.handoff_key.as_deref() else {
        return;
    };
    if let Some(activation) = world
        .resource_mut::<KnownTechniquesActivations>()
        .0
        .remove(subject)
    {
        if let Some(mut entity) = world.get_entity_mut(activation.entity) {
            entity.remove::<KnownTechniques>();
            entity.remove::<KnownTechniquesLoadFailed>();
        }
    }
}

pub(super) fn save_known_techniques_disconnect_slice(
    world: &mut World,
    context: &SliceRunContext,
) -> SliceRunResult {
    let subject = context
        .handoff_key
        .as_deref()
        .ok_or_else(|| SliceRunError::new("known techniques disconnect save has no subject"))?;
    let persistence = world
        .get_resource::<PlayerStatePersistence>()
        .cloned()
        .ok_or_else(|| SliceRunError::new("PlayerStatePersistence is unavailable"))?;
    world.resource_scope(
        |world, mut activations: valence::prelude::Mut<KnownTechniquesActivations>| {
            let Some(activation) = activations.0.get_mut(subject) else {
                return Ok(SliceRunOutcome::Clean);
            };
            if activation.guarded.load_status() == slice::SliceLoadStatus::Failed {
                return Ok(SliceRunOutcome::Clean);
            }
            sync_known_techniques_activation(world, subject, activation)?;
            persist_known_techniques_activation(activation, &persistence, WriteOutlet::Disconnect)
        },
    )
}

pub(super) fn flush_known_techniques_shutdown_slice(
    world: &mut World,
    _context: &SliceRunContext,
) -> SliceRunResult {
    let persistence = world
        .get_resource::<PlayerStatePersistence>()
        .cloned()
        .ok_or_else(|| SliceRunError::new("PlayerStatePersistence is unavailable"))?;
    world.resource_scope(
        |world, mut activations: valence::prelude::Mut<KnownTechniquesActivations>| {
            let mut subjects = activations.0.keys().cloned().collect::<Vec<_>>();
            subjects.sort();
            let mut flushed = false;
            let mut failures = Vec::new();
            for subject in subjects {
                let Some(activation) = activations.0.get_mut(&subject) else {
                    continue;
                };
                if activation.guarded.load_status() == slice::SliceLoadStatus::Failed {
                    continue;
                }
                let result = (|| -> Result<SliceRunOutcome, SliceRunError> {
                    sync_known_techniques_activation(world, &subject, activation)?;
                    persist_known_techniques_activation(
                        activation,
                        &persistence,
                        WriteOutlet::Shutdown,
                    )
                })();
                match result {
                    Ok(SliceRunOutcome::Flushed) => flushed = true,
                    Ok(SliceRunOutcome::Clean | SliceRunOutcome::SkippedBlocked) => {}
                    Err(error) => failures.push(format!("{subject}: {error}")),
                }
            }
            if failures.is_empty() {
                Ok(if flushed {
                    SliceRunOutcome::Flushed
                } else {
                    SliceRunOutcome::Clean
                })
            } else {
                Err(SliceRunError::new(format!(
                    "known techniques shutdown flush failed: {}",
                    failures.join("; ")
                )))
            }
        },
    )
}

pub(super) fn production_slice_clock(world: &World) -> ProductionSliceClock {
    ProductionSliceClock {
        runtime_tick: world
            .get_resource::<CultivationClock>()
            .map_or(0, |clock| clock.tick),
        wall_unix_millis: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_millis() as u64),
    }
}

pub(crate) fn dispatch_known_techniques_reconnects(world: &mut World) {
    let frame = {
        let mut state = world.resource_mut::<KnownTechniquesReconnectState>();
        state.frame = state.frame.saturating_add(1);
        state.frame
    };

    cleanup_stale_known_techniques_pending(world);

    let mut added_query = world.query_filtered::<(Entity, &Username), Added<Client>>();
    let added = added_query
        .iter(world)
        .map(|(entity, username)| (canonical_player_id(username.0.as_str()), entity))
        .collect::<Vec<_>>();
    for (subject, entity) in added {
        let already_pending = world
            .resource::<PendingKnownTechniquesHandoffs>()
            .0
            .contains_key(&subject);
        if already_pending {
            world
                .entity_mut(entity)
                .insert(KnownTechniquesReconnectBlocked);
            world
                .resource_mut::<PendingKnownTechniquesCandidates>()
                .0
                .entry(subject.clone())
                .or_default()
                .push(entity);
            tracing::warn!(
                "[bong][persistence] rejecting duplicate known techniques reconnect target for `{subject}`"
            );
            continue;
        }
        world
            .resource_mut::<PendingKnownTechniquesHandoffs>()
            .0
            .insert(subject, entity);
    }

    let candidate_subjects = world
        .resource::<PendingKnownTechniquesCandidates>()
        .0
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    for subject in candidate_subjects {
        promote_known_techniques_candidate(world, &subject);
    }

    let disconnected_subjects = world
        .resource::<KnownTechniquesActivations>()
        .0
        .iter()
        .filter(|(_, activation)| world.get::<Client>(activation.entity).is_none())
        .map(|(subject, _)| subject.clone())
        .collect::<Vec<_>>();

    let persistence = world.get_resource::<PlayerStatePersistence>().cloned();
    let save_subjects = disconnected_subjects
        .into_iter()
        .filter(|subject| {
            !world
                .resource::<PendingKnownTechniquesHandoffs>()
                .0
                .contains_key(subject)
        })
        .collect::<Vec<_>>();
    for subject in save_subjects {
        let should_attempt = {
            let mut state = world.resource_mut::<KnownTechniquesReconnectState>();
            begin_known_techniques_retry(&mut state, &subject, frame)
        };
        if !should_attempt {
            continue;
        }
        let result = persistence.as_ref().map_or_else(
            || Err(SliceRunError::new("PlayerStatePersistence is unavailable")),
            |persistence| {
                world.resource_scope(
                    |world, mut activations: valence::prelude::Mut<KnownTechniquesActivations>| {
                        let Some(activation) = activations.0.get_mut(&subject) else {
                            return Ok(SliceRunOutcome::Clean);
                        };
                        if activation.guarded.load_status() == slice::SliceLoadStatus::Failed {
                            return Ok(SliceRunOutcome::Clean);
                        }
                        sync_known_techniques_activation(world, &subject, activation)?;
                        persist_known_techniques_activation(
                            activation,
                            persistence,
                            WriteOutlet::Disconnect,
                        )
                    },
                )
            },
        );
        match result {
            Ok(_) => {
                world
                    .resource_mut::<KnownTechniquesActivations>()
                    .0
                    .remove(&subject);
                clear_known_techniques_retry(
                    &mut world.resource_mut::<KnownTechniquesReconnectState>(),
                    &subject,
                );
            }
            Err(error) => {
                let (at_retry_cap, should_log) = {
                    let mut state = world.resource_mut::<KnownTechniquesReconnectState>();
                    let at_retry_cap =
                        record_known_techniques_retry_failure(&mut state, &subject, frame);
                    let should_log =
                        known_techniques_retry_log_allowed(&mut state, &subject, frame);
                    (at_retry_cap, should_log)
                };
                if should_log {
                    if at_retry_cap {
                        tracing::error!(
                            "[bong][persistence] known techniques disconnect flush remains unavailable at the retry cap for `{subject}`; retry scheduled: {error}"
                        );
                    } else {
                        tracing::warn!(
                            "[bong][persistence] known techniques disconnect flush failed for `{subject}`; retry scheduled: {error}"
                        );
                    }
                }
            }
        }
    }

    let candidate_subjects = world
        .resource::<PendingKnownTechniquesCandidates>()
        .0
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    for subject in candidate_subjects {
        promote_known_techniques_candidate(world, &subject);
    }
    let pending_subjects = world
        .resource::<PendingKnownTechniquesHandoffs>()
        .0
        .keys()
        .cloned()
        .collect::<Vec<_>>();

    // A subject that is still pending reconnect is saved by the handoff dispatcher below;
    // keeping its retry entry here would double-count attempts and alter the handoff gate.
    let pending_subject_set = pending_subjects.iter().cloned().collect::<HashSet<_>>();
    clear_stale_known_techniques_retries(
        &mut world.resource_mut::<KnownTechniquesReconnectState>(),
        &pending_subject_set,
    );

    for subject in pending_subjects {
        if !world
            .resource::<PendingKnownTechniquesHandoffs>()
            .0
            .contains_key(&subject)
        {
            continue;
        }
        if known_techniques_live_activation(world, &subject).is_some() {
            if let Some(entity) = world
                .resource::<PendingKnownTechniquesHandoffs>()
                .0
                .get(&subject)
                .copied()
            {
                let was_blocked = world
                    .get::<KnownTechniquesReconnectBlocked>(entity)
                    .is_some();
                world
                    .entity_mut(entity)
                    .insert(KnownTechniquesReconnectBlocked);
                if !was_blocked {
                    tracing::warn!(
                        "[bong][persistence] rejecting live duplicate known techniques reconnect target for `{subject}`"
                    );
                }
            }
            clear_known_techniques_retry(
                &mut world.resource_mut::<KnownTechniquesReconnectState>(),
                &subject,
            );
            continue;
        }

        let should_attempt = {
            let mut state = world.resource_mut::<KnownTechniquesReconnectState>();
            begin_known_techniques_retry(&mut state, &subject, frame)
        };
        if !should_attempt {
            continue;
        }

        let clock = production_slice_clock(world);
        let (succeeded, stable_live_duplicate) = match dispatch_reconnect_handoff(
            world,
            reconnect_handoff_token(subject.clone()),
            &clock,
        ) {
            Ok(report)
                if report.failures.is_empty()
                    && report.blocked_saves.is_empty()
                    && report.blocked_loads.is_empty()
                    && report.blocked_preflights.is_empty()
                    && report.blocked_rebases.is_empty() =>
            {
                (true, false)
            }
            Ok(report) => {
                let stable_live_duplicate = reconnect_report_is_live_duplicate(&report);
                let pending_entity = world
                    .resource::<PendingKnownTechniquesHandoffs>()
                    .0
                    .get(&subject)
                    .copied();
                if let Some(entity) = pending_entity {
                    world
                        .entity_mut(entity)
                        .insert(KnownTechniquesReconnectFailed);
                }
                let should_log = known_techniques_retry_log_allowed(
                    &mut world.resource_mut::<KnownTechniquesReconnectState>(),
                    &subject,
                    frame,
                );
                if should_log {
                    tracing::error!(
                        "[bong][persistence] known techniques reconnect handoff failed closed for `{subject}`: {report:?}"
                    );
                }
                (false, stable_live_duplicate)
            }
            Err(error) => {
                if let Some(entity) = world
                    .resource::<PendingKnownTechniquesHandoffs>()
                    .0
                    .get(&subject)
                    .copied()
                {
                    world
                        .entity_mut(entity)
                        .insert(KnownTechniquesReconnectFailed);
                }
                let should_log = known_techniques_retry_log_allowed(
                    &mut world.resource_mut::<KnownTechniquesReconnectState>(),
                    &subject,
                    frame,
                );
                if should_log {
                    tracing::error!(
                        "[bong][persistence] known techniques reconnect dispatch failed for `{subject}`: {error}"
                    );
                }
                (false, false)
            }
        };
        if succeeded {
            if let Some(entity) = world
                .resource::<KnownTechniquesActivations>()
                .0
                .get(&subject)
                .map(|activation| activation.entity)
            {
                let load_failed = world
                    .resource::<KnownTechniquesActivations>()
                    .0
                    .get(&subject)
                    .is_some_and(|activation| {
                        activation.guarded.load_status() == slice::SliceLoadStatus::Failed
                    });
                if let Some(mut target) = world.get_entity_mut(entity) {
                    target.remove::<KnownTechniquesReconnectBlocked>();
                    if load_failed {
                        target.remove::<KnownTechniquesReconnectReady>();
                        target.insert(KnownTechniquesReconnectFailed);
                    } else {
                        target.remove::<KnownTechniquesReconnectFailed>();
                        target.insert(KnownTechniquesReconnectReady);
                    }
                }
            }
            world
                .resource_mut::<KnownTechniquesReconnectState>()
                .retries
                .remove(&subject);
        } else if stable_live_duplicate {
            clear_known_techniques_retry(
                &mut world.resource_mut::<KnownTechniquesReconnectState>(),
                &subject,
            );
        } else {
            let mut state = world.resource_mut::<KnownTechniquesReconnectState>();
            record_known_techniques_retry_failure(&mut state, &subject, frame);
        }
    }
}

pub(super) fn flush_changed_known_techniques_slices(world: &mut World) {
    let mut query = world.query_filtered::<(Entity, &Username, &KnownTechniques), (
        With<Client>,
        Changed<KnownTechniques>,
        Without<KnownTechniquesLoadFailed>,
    )>();
    let changed = query
        .iter(world)
        .map(|(entity, username, value)| {
            (
                entity,
                canonical_player_id(username.0.as_str()),
                value.clone(),
            )
        })
        .collect::<Vec<_>>();
    let Some(persistence) = world.get_resource::<PlayerStatePersistence>().cloned() else {
        return;
    };
    for (entity, subject, value) in changed {
        let result = world.resource_scope(
            |_, mut activations: valence::prelude::Mut<KnownTechniquesActivations>| {
                let Some(activation) = activations.0.get_mut(&subject) else {
                    return Ok(SliceRunOutcome::Clean);
                };
                if activation.entity != entity || activation.guarded.value() == &value {
                    return Ok(SliceRunOutcome::Clean);
                }
                activation
                    .guarded
                    .mutate(&mut activation.tracker, |guarded| *guarded = value)
                    .map_err(|error| SliceRunError::new(error.to_string()))?;
                persist_known_techniques_activation(activation, &persistence, WriteOutlet::Changed)
            },
        );
        if let Err(error) = result {
            tracing::warn!(
                "[bong][persistence] immediate known techniques flush failed for `{subject}`: {error}"
            );
        }
    }
}
