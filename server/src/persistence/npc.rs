//! NPC runtime, dormant relic, archive, and persistence-domain operations.

use super::*;

#[derive(Debug, Default)]
pub(super) struct NpcSnapshotTracker {
    last_snapshot_tick: u32,
}

impl Resource for NpcSnapshotTracker {}

#[derive(Debug, Default)]
pub(super) struct NpcDigestSweepState {
    last_sweep_wall: i64,
}

impl Resource for NpcDigestSweepState {}

/// plan-offscreen-war-v1 P3：战场遗物 TTL sweep 的手动限频状态（仿 [`NpcDigestSweepState`]）。
#[derive(Debug, Default)]
pub(super) struct DormantRelicSweepState {
    pub(super) last_sweep_wall: i64,
}

impl Resource for DormantRelicSweepState {}

#[derive(Debug, Default, Component)]
pub(super) struct NpcArchivedPersistence;

#[derive(Debug, Default, Component)]
pub(super) struct NpcLivePersistenceSnapshot;

#[allow(clippy::too_many_arguments)]
pub fn capture_npc_persistence(
    entity: Entity,
    position: &Position,
    kind: EntityKind,
    state: NpcStateKind,
    blackboard: &NpcBlackboard,
    nearest_player_id: Option<&str>,
    loadout: &NpcCombatLoadout,
    patrol: &NpcPatrol,
    movement: &MovementController,
    cooldowns: &MovementCooldowns,
    lifecycle: &Lifecycle,
    cultivation: Option<&Cultivation>,
    life_record: Option<&LifeRecord>,
) -> NpcPersistenceCapture {
    let char_id = if lifecycle.character_id != "unbound:character" {
        lifecycle.character_id.clone()
    } else {
        canonical_npc_id(entity)
    };
    let archetype = npc_archetype_label(loadout.melee_archetype).to_string();
    let blackboard_snapshot = build_npc_blackboard_snapshot(blackboard, nearest_player_id);
    let since_tick = life_record
        .map(|record| record.created_at)
        .unwrap_or_else(|| lifecycle.last_revive_tick.unwrap_or_default());
    let digest = NpcDigestRecord {
        char_id: char_id.clone(),
        archetype: archetype.clone(),
        realm: cultivation
            .map(|cultivation| format!("{:?}", cultivation.realm).to_ascii_lowercase())
            .unwrap_or_else(|| "unknown".to_string()),
        faction_id: None,
        recent_summary: life_record
            .map(|record| record.recent_summary_text(3))
            .filter(|summary| !summary.is_empty())
            .unwrap_or_else(|| format!("{}:{}", char_id, state_label(&state))),
        last_referenced_wall: current_unix_seconds(),
    };

    NpcPersistenceCapture {
        state: NpcStateRecord {
            char_id: char_id.clone(),
            kind: entity_kind_label(kind).to_string(),
            pos: vec3_to_array(position.get()),
            state: state_label(&state).to_string(),
            blackboard: blackboard_snapshot,
            archetype: archetype.clone(),
            home_zone: patrol.home_zone.clone(),
            patrol_anchor_index: patrol.anchor_index,
            patrol_target: vec3_to_array(patrol.current_target),
            movement_mode: movement_mode_label(&movement.mode).to_string(),
            can_sprint: loadout.movement_capabilities.can_sprint,
            can_dash: loadout.movement_capabilities.can_dash,
            sprint_ready_at: cooldowns.sprint_ready_at,
            dash_ready_at: cooldowns.dash_ready_at,
            lifecycle_state: lifecycle_state_label(&lifecycle.state).to_string(),
            death_count: lifecycle.death_count,
            last_death_tick: lifecycle.last_death_tick,
            last_revive_tick: lifecycle.last_revive_tick,
        },
        digest,
        archetype_entry: ArchetypeRegistryEntry {
            char_id,
            archetype,
            since_tick,
        },
        captured_at_wall: current_unix_seconds(),
    }
}

pub fn persist_npc_capture(
    settings: &PersistenceSettings,
    capture: &NpcPersistenceCapture,
) -> io::Result<()> {
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    (|| -> io::Result<()> {
        upsert_npc_state(&transaction, &capture.state, capture.captured_at_wall)?;
        upsert_npc_digest(&transaction, &capture.digest, capture.captured_at_wall)?;
        upsert_archetype_registry_entry(
            &transaction,
            &capture.archetype_entry,
            capture.captured_at_wall,
        )?;
        transaction.commit().map_err(io::Error::other)
    })()
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn load_npc_state(
    settings: &PersistenceSettings,
    char_id: &str,
) -> io::Result<Option<NpcStateRecord>> {
    let connection = open_persistence_connection(settings)?;
    load_npc_state_from_connection(&connection, char_id)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn load_npc_digest(
    settings: &PersistenceSettings,
    char_id: &str,
) -> io::Result<Option<NpcDigestRecord>> {
    let connection = open_persistence_connection(settings)?;
    load_npc_digest_from_connection(&connection, char_id)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn record_archetype_transition(
    settings: &PersistenceSettings,
    entry: &ArchetypeRegistryEntry,
) -> io::Result<()> {
    let wall_clock = current_unix_seconds();
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    upsert_archetype_registry_entry(&transaction, entry, wall_clock)?;
    transaction.commit().map_err(io::Error::other)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn load_archetype_registry(
    settings: &PersistenceSettings,
    char_id: &str,
) -> io::Result<Vec<ArchetypeRegistryEntry>> {
    let connection = open_persistence_connection(settings)?;
    load_archetype_registry_from_connection(&connection, char_id)
}

pub fn persist_npc_deceased_archive(
    settings: &PersistenceSettings,
    archive: &NpcDeceasedArchiveRecord,
) -> io::Result<()> {
    persist_npc_deceased_archive_with_connection(settings, archive, open_persistence_connection)
}

pub(super) fn persist_npc_deceased_archive_with_connection(
    settings: &PersistenceSettings,
    archive: &NpcDeceasedArchiveRecord,
    open_connection: impl FnOnce(&PersistenceSettings) -> io::Result<Connection>,
) -> io::Result<()> {
    persist_npc_deceased_archive_with_hooks(settings, archive, open_connection, write_zstd_bundle)
}

pub(super) fn persist_npc_deceased_archive_with_hooks(
    settings: &PersistenceSettings,
    archive: &NpcDeceasedArchiveRecord,
    open_connection: impl FnOnce(&PersistenceSettings) -> io::Result<Connection>,
    write_bundle: impl FnOnce(&Path, &[u8]) -> io::Result<()>,
) -> io::Result<()> {
    let archive_path = npc_deceased_archive_absolute_path(
        settings,
        archive.char_id.as_str(),
        archive.archived_at_wall,
    )?;
    let relative_path =
        npc_deceased_archive_relative_path(archive.char_id.as_str(), archive.archived_at_wall)?;
    let previous_archive = read_optional_file(&archive_path)?;
    let archive_json = serde_json::to_vec_pretty(archive)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    // `write_zstd_bundle` 的发布契约是失败时不改变最终路径：临时文件只会在
    // hard_link 成功后成为目标，目标已存在时 hard_link 只返回 AlreadyExists。
    // 若目标正是本次进程在 DB 提交前发布后崩溃留下的有效 bundle，可以复用它完成
    // index/hot-row reconciliation；不同内容或无法解码的目标仍然 fail-closed，不能
    // 通过覆盖文件来掩盖 ownership 冲突。
    let archive_published_by_call = match write_bundle(&archive_path, &archive_json) {
        Ok(()) => true,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            let existing_archive = match read_optional_file(&archive_path)? {
                Some(existing) => existing,
                None => return Err(error),
            };
            let existing_payload = zstd::stream::decode_all(existing_archive.as_slice())
                .map_err(|decode_error| io::Error::new(io::ErrorKind::InvalidData, decode_error))?;
            let existing_value: serde_json::Value = serde_json::from_slice(&existing_payload)
                .map_err(|decode_error| io::Error::new(io::ErrorKind::InvalidData, decode_error))?;
            let expected_value: serde_json::Value = serde_json::from_slice(&archive_json)
                .map_err(|decode_error| io::Error::new(io::ErrorKind::InvalidData, decode_error))?;
            if existing_value != expected_value {
                return Err(error);
            }
            false
        }
        Err(error) => return Err(error),
    };

    let persisted = (|| -> io::Result<()> {
        let mut connection = open_connection(settings)?;
        let transaction = connection.transaction().map_err(io::Error::other)?;
        upsert_npc_deceased_index(
            &transaction,
            &NpcDeceasedIndexRecord {
                char_id: archive.char_id.clone(),
                archetype: archive.archetype.clone(),
                died_at_tick: archive.died_at_tick,
                path: relative_path.clone(),
            },
            archive.archived_at_wall,
        )?;
        delete_npc_hot_rows(&transaction, archive.char_id.as_str())?;
        transaction.commit().map_err(io::Error::other)
    })();

    match persisted {
        Ok(()) => Ok(()),
        Err(error) if !archive_published_by_call => Err(error),
        Err(error) => match rollback_file(&archive_path, previous_archive.as_deref()) {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(combine_persistence_failure(
                "npc archive persistence",
                error,
                rollback_error,
            )),
        },
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn load_npc_deceased_archive(
    settings: &PersistenceSettings,
    char_id: &str,
) -> io::Result<Option<NpcDeceasedArchiveRecord>> {
    validate_archive_component(char_id)?;
    let connection = open_persistence_connection(settings)?;
    let path: Option<String> = connection
        .query_row(
            "SELECT path FROM npc_deceased_index WHERE char_id = ?1",
            params![char_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(io::Error::other)?;
    let Some(path) = path else {
        return Ok(None);
    };
    let bytes = read_zstd_bundle(settings.db_path(), path.as_str())?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

pub fn sweep_stale_npc_digests(
    settings: &PersistenceSettings,
    now_wall: i64,
) -> io::Result<Vec<NpcDigestRecord>> {
    sweep_stale_npc_digests_with_writer(settings, now_wall, write_zstd_bundle)
}

pub(super) fn sweep_stale_npc_digests_with_writer(
    settings: &PersistenceSettings,
    now_wall: i64,
    mut write_bundle: impl FnMut(&Path, &[u8]) -> io::Result<()>,
) -> io::Result<Vec<NpcDigestRecord>> {
    let threshold = now_wall - NPC_DIGEST_RETENTION_SECS;
    let mut connection = open_persistence_connection(settings)?;
    let stale_digests = load_stale_npc_digests(&connection, threshold)?;
    if stale_digests.is_empty() {
        return Ok(Vec::new());
    }

    for digest in &stale_digests {
        let archive_path =
            npc_digest_archive_absolute_path(settings, digest.char_id.as_str(), now_wall)?;
        let previous_archive = read_optional_file(&archive_path)?;
        let archive_json = serde_json::to_vec_pretty(digest)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let publish_result = match previous_archive.as_deref() {
            Some(existing) => {
                let decoded = zstd::stream::decode_all(existing)
                    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
                if decoded == archive_json {
                    Ok(())
                } else {
                    Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        format!(
                            "npc digest archive already contains different bytes for `{}`",
                            digest.char_id
                        ),
                    ))
                }
            }
            None => write_bundle(&archive_path, &archive_json),
        };
        // `write_zstd_bundle` 只会发布自己的临时文件，且以 hard_link 做 no-replace
        // 发布；失败时调用方没有最终目标的 ownership。特别是读取到 None 后，
        // 另一个 publisher 可能已在写入期间建立目标，不能用 rollback_file(None)
        // 把并发 publisher 的归档删除。
        publish_result?;
    }

    let transaction = connection.transaction().map_err(io::Error::other)?;
    for digest in &stale_digests {
        transaction
            .execute(
                "DELETE FROM npc_digests WHERE char_id = ?1",
                params![digest.char_id.as_str()],
            )
            .map_err(io::Error::other)?;
    }
    transaction.commit().map_err(io::Error::other)?;

    Ok(stale_digests)
}

pub(super) type NpcPersistenceQueryItem<'a> = (
    Entity,
    &'a Position,
    &'a EntityKind,
    &'a NpcBlackboard,
    &'a NpcCombatLoadout,
    &'a NpcPatrol,
    &'a MovementController,
    &'a MovementCooldowns,
    &'a Lifecycle,
    Option<&'a Cultivation>,
    Option<&'a LifeRecord>,
    Option<&'a NpcLivePersistenceSnapshot>,
    Option<&'a NpcArchivedPersistence>,
);

#[allow(clippy::too_many_arguments)]
pub(super) fn persist_npc_runtime_state_system(
    settings: Res<PersistenceSettings>,
    mut commands: Commands,
    mut snapshot_tracker: ResMut<NpcSnapshotTracker>,
    players: Query<(Entity, &Username), With<Client>>,
    npcs: Query<NpcPersistenceQueryItem<'_>, With<NpcMarker>>,
    flee_actions: Query<(&Actor, &ActionState), With<FleeAction>>,
    chase_actions: Query<(&Actor, &ActionState), With<ChaseAction>>,
    melee_actions: Query<(&Actor, &ActionState), With<MeleeAttackAction>>,
    dash_actions: Query<(&Actor, &ActionState), With<DashAction>>,
    game_tick: Option<Res<crate::npc::movement::GameTick>>,
) {
    let snapshot_due = game_tick.as_ref().is_none_or(|tick| {
        tick.0.wrapping_sub(snapshot_tracker.last_snapshot_tick) >= NPC_SNAPSHOT_INTERVAL_TICKS
    });
    let action_states =
        collect_npc_action_states(&flee_actions, &chase_actions, &melee_actions, &dash_actions);

    for (
        entity,
        position,
        kind,
        blackboard,
        loadout,
        patrol,
        movement,
        cooldowns,
        lifecycle,
        cultivation,
        life_record,
        live_snapshot,
        archived,
    ) in &npcs
    {
        let nearest_player_id = resolve_nearest_player_id(blackboard, &players);
        let effective_state = effective_npc_state(entity, lifecycle, &action_states);
        let is_terminated = lifecycle.state == LifecycleState::Terminated;
        let should_snapshot = if is_terminated {
            archived.is_none()
        } else {
            snapshot_due || live_snapshot.is_none()
        };
        if !should_snapshot {
            continue;
        }

        let capture = capture_npc_persistence(
            entity,
            position,
            *kind,
            effective_state,
            blackboard,
            nearest_player_id.as_deref(),
            loadout,
            patrol,
            movement,
            cooldowns,
            lifecycle,
            cultivation,
            life_record,
        );

        let result = if lifecycle.state == LifecycleState::Terminated {
            persist_npc_deceased_archive(
                &settings,
                &NpcDeceasedArchiveRecord {
                    char_id: capture.state.char_id.clone(),
                    archetype: capture.state.archetype.clone(),
                    died_at_tick: lifecycle.last_death_tick.unwrap_or_default(),
                    archived_at_wall: capture.captured_at_wall,
                    lifecycle_state: capture.state.lifecycle_state.clone(),
                    death_count: capture.state.death_count,
                    state: Some(capture.state.clone()),
                    digest: Some(capture.digest.clone()),
                    life_record: life_record.cloned(),
                },
            )
        } else {
            persist_npc_capture(&settings, &capture)
        };

        if let Err(error) = result {
            tracing::warn!(
                "[bong][persistence] failed to persist npc {}: {error}",
                capture.state.char_id
            );
            continue;
        }

        if is_terminated && archived.is_none() {
            commands.entity(entity).insert(NpcArchivedPersistence);
        } else if !is_terminated && live_snapshot.is_none() {
            commands.entity(entity).insert(NpcLivePersistenceSnapshot);
        }
    }

    if snapshot_due {
        if let Some(tick) = game_tick.as_ref() {
            snapshot_tracker.last_snapshot_tick = tick.0;
        }
    }
}

pub(super) fn sweep_npc_digest_retention_system(
    settings: Res<PersistenceSettings>,
    mut sweep_state: ResMut<NpcDigestSweepState>,
) {
    let now_wall = current_unix_seconds();
    if sweep_state.last_sweep_wall > 0
        && now_wall.saturating_sub(sweep_state.last_sweep_wall) < NPC_DIGEST_SWEEP_INTERVAL_SECS
    {
        return;
    }

    match sweep_stale_npc_digests(&settings, now_wall) {
        Ok(_) => {
            sweep_state.last_sweep_wall = now_wall;
        }
        Err(error) => {
            tracing::warn!("[bong][persistence] failed npc digest retention sweep: {error}");
        }
    }
}

/// plan-offscreen-war-v1 P3：消费 [`PendingDormantRelicCreated`](crate::npc::dormant::PendingDormantRelicCreated)
/// → 把待物化战场遗物落盘进 `pending_dormant_relics`。
///
/// 事件由 `run_dormant_combat_phase` 在败者真元**已守恒释放完毕**且克制判定通过时 emit
/// （严格在 `release_dormant_qi_to_zone` 之后、`store.remove` 之前——无吞真元窗口）。本 system
/// 只把 event 持久化，**不碰任何真元 / ledger**（遗物零真元，§10.1 #5 ④红线）。现开连接同步写
/// （仿 `persist_npc_runtime_state_system` 范式，无 deferred channel）。
///
/// `relic_id` 用**确定性**复合键（char_id + created_tick + loot_seed）而非随机 UUID（CodeRabbit）：
/// 一个逻辑战死对应唯一 (char_id, created_tick)，loot_seed 由 (char_id, tick, sim_seed) 确定，
/// 故同一逻辑死亡始终映射到同一 relic_id。配合 `upsert_pending_dormant_relic` 的
/// `ON CONFLICT(relic_id) DO UPDATE`，**重复 emit 同一遗物天然幂等**（覆盖而非插重复行），
/// 也让未来若加重试路径能靠 relic_id 去重。
/// 注：遗物是**零真元** telemetry/cosmetic loot 占位，持久化失败仅丢一处遗物 ground loot、
/// **不违反守恒**（不像 dormant qi 快照丢失=吞真元）；故此处失败 warn+drop 而非引入重型重试
/// 队列子系统——确定性 relic_id 已消除「随机 id 无法去重」这一真正的回归隐患。
/// 由战场遗物 event 的**逻辑标识字段**（char_id + created_tick + loot_seed）构造确定性
/// `relic_id`。同一逻辑战死无论 emit / persist 多少次都得到同一 id，配合 PK `ON CONFLICT`
/// upsert 实现幂等。created_wall（墙钟）**不**进 id（它随重试漂移、会破坏幂等）。
pub(super) fn deterministic_relic_id(
    event: &crate::npc::dormant::PendingDormantRelicCreated,
) -> String {
    format!(
        "relic:{}:{}:{:016x}",
        event.char_id, event.created_tick, event.loot_seed
    )
}

pub(super) fn persist_pending_dormant_relics_system(
    settings: Res<PersistenceSettings>,
    mut events: EventReader<crate::npc::dormant::PendingDormantRelicCreated>,
) {
    let pending: Vec<&crate::npc::dormant::PendingDormantRelicCreated> = events.read().collect();
    if pending.is_empty() {
        return;
    }
    let created_wall = current_unix_seconds();
    for event in pending {
        let record = PendingDormantRelicRecord {
            relic_id: deterministic_relic_id(event),
            char_id: event.char_id.clone(),
            zone: event.zone.clone(),
            pos_x: event.position[0],
            pos_y: event.position[1],
            pos_z: event.position[2],
            archetype: event.archetype.as_str().to_string(),
            loot_seed: event.loot_seed,
            created_tick: event.created_tick as i64,
            created_wall,
        };
        if let Err(error) = persist_pending_dormant_relic(&settings, &record) {
            tracing::warn!(
                "[bong][persistence] failed to persist pending dormant relic for {}: {error}",
                event.char_id
            );
            continue;
        }
        tracing::debug!(
            "[bong][persistence] persisted pending battlefield relic {} (char={} zone={} archetype={})",
            record.relic_id,
            record.char_id,
            record.zone,
            record.archetype,
        );
    }
}

/// plan-offscreen-war-v1 P3：战场遗物 TTL retention sweep（仿 [`sweep_npc_digest_retention_system`]）。
/// 墙钟手动限频（[`PENDING_RELIC_SWEEP_INTERVAL_SECS`]）；每次清掉 `created_wall` 早于
/// `now - PENDING_RELIC_RETENTION_SECS` 的陈旧遗物，避免无人到访的战场遗物永久堆积。
pub(super) fn sweep_dormant_relic_retention_system(
    settings: Res<PersistenceSettings>,
    mut sweep_state: ResMut<DormantRelicSweepState>,
) {
    let now_wall = current_unix_seconds();
    if sweep_state.last_sweep_wall > 0
        && now_wall.saturating_sub(sweep_state.last_sweep_wall) < PENDING_RELIC_SWEEP_INTERVAL_SECS
    {
        return;
    }

    match sweep_stale_dormant_relics(&settings, now_wall) {
        Ok(removed) => {
            sweep_state.last_sweep_wall = now_wall;
            if removed > 0 {
                tracing::debug!(
                    "[bong][persistence] swept {removed} stale battlefield relic(s) (older than {PENDING_RELIC_RETENTION_SECS}s)"
                );
            }
        }
        Err(error) => {
            tracing::warn!("[bong][persistence] failed dormant relic retention sweep: {error}");
        }
    }
}

pub(super) fn effective_npc_state(
    entity: Entity,
    lifecycle: &Lifecycle,
    action_states: &HashMap<Entity, NpcStateKind>,
) -> NpcStateKind {
    if lifecycle.state == LifecycleState::Terminated {
        return NpcStateKind::Idle;
    }
    action_states
        .get(&entity)
        .cloned()
        .unwrap_or(NpcStateKind::Idle)
}

pub(super) fn collect_npc_action_states(
    flee_actions: &Query<(&Actor, &ActionState), With<FleeAction>>,
    chase_actions: &Query<(&Actor, &ActionState), With<ChaseAction>>,
    melee_actions: &Query<(&Actor, &ActionState), With<MeleeAttackAction>>,
    dash_actions: &Query<(&Actor, &ActionState), With<DashAction>>,
) -> HashMap<Entity, NpcStateKind> {
    let mut states = HashMap::new();
    for (Actor(entity), action_state) in chase_actions.iter() {
        if matches!(action_state, ActionState::Executing) {
            states.insert(*entity, NpcStateKind::Patrolling);
        }
    }
    for (Actor(entity), action_state) in flee_actions.iter() {
        if matches!(action_state, ActionState::Executing) {
            states.insert(*entity, NpcStateKind::Fleeing);
        }
    }
    for (Actor(entity), action_state) in dash_actions.iter() {
        if matches!(action_state, ActionState::Executing) {
            states.insert(*entity, NpcStateKind::Attacking);
        }
    }
    for (Actor(entity), action_state) in melee_actions.iter() {
        if matches!(action_state, ActionState::Executing) {
            states.insert(*entity, NpcStateKind::Attacking);
        }
    }
    states
}

pub(super) fn resolve_nearest_player_id(
    blackboard: &NpcBlackboard,
    players: &Query<(Entity, &Username), With<Client>>,
) -> Option<String> {
    let player_entity = blackboard.nearest_player?;
    let Ok((_, username)) = players.get(player_entity) else {
        return None;
    };
    Some(canonical_player_id(username.0.as_str()))
}

pub(super) fn upsert_npc_state(
    transaction: &rusqlite::Transaction<'_>,
    state: &NpcStateRecord,
    wall_clock: i64,
) -> io::Result<()> {
    let blackboard_json = serde_json::to_string(&state.blackboard)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    transaction
        .execute(
            "
            INSERT INTO npc_state (
                char_id,
                kind,
                archetype,
                pos_x,
                pos_y,
                pos_z,
                state,
                blackboard_json,
                home_zone,
                patrol_anchor_index,
                patrol_target_x,
                patrol_target_y,
                patrol_target_z,
                movement_mode,
                can_sprint,
                can_dash,
                sprint_ready_at,
                dash_ready_at,
                lifecycle_state,
                death_count,
                last_death_tick,
                last_revive_tick,
                schema_version,
                last_updated_wall
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16,
                ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24
            )
            ON CONFLICT(char_id) DO UPDATE SET
                kind = excluded.kind,
                archetype = excluded.archetype,
                pos_x = excluded.pos_x,
                pos_y = excluded.pos_y,
                pos_z = excluded.pos_z,
                state = excluded.state,
                blackboard_json = excluded.blackboard_json,
                home_zone = excluded.home_zone,
                patrol_anchor_index = excluded.patrol_anchor_index,
                patrol_target_x = excluded.patrol_target_x,
                patrol_target_y = excluded.patrol_target_y,
                patrol_target_z = excluded.patrol_target_z,
                movement_mode = excluded.movement_mode,
                can_sprint = excluded.can_sprint,
                can_dash = excluded.can_dash,
                sprint_ready_at = excluded.sprint_ready_at,
                dash_ready_at = excluded.dash_ready_at,
                lifecycle_state = excluded.lifecycle_state,
                death_count = excluded.death_count,
                last_death_tick = excluded.last_death_tick,
                last_revive_tick = excluded.last_revive_tick,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                state.char_id,
                state.kind,
                state.archetype,
                state.pos[0],
                state.pos[1],
                state.pos[2],
                state.state,
                blackboard_json,
                state.home_zone,
                sql_usize(state.patrol_anchor_index)?,
                state.patrol_target[0],
                state.patrol_target[1],
                state.patrol_target[2],
                state.movement_mode,
                bool_to_sql(state.can_sprint),
                bool_to_sql(state.can_dash),
                i64::from(state.sprint_ready_at),
                i64::from(state.dash_ready_at),
                state.lifecycle_state,
                i64::from(state.death_count),
                optional_tick_to_sql(state.last_death_tick)?,
                optional_tick_to_sql(state.last_revive_tick)?,
                NPC_ROW_SCHEMA_VERSION,
                wall_clock,
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

pub(super) fn upsert_npc_digest(
    transaction: &rusqlite::Transaction<'_>,
    digest: &NpcDigestRecord,
    wall_clock: i64,
) -> io::Result<()> {
    transaction
        .execute(
            "
            INSERT INTO npc_digests (
                char_id,
                archetype,
                realm,
                faction_id,
                recent_summary,
                last_referenced_wall,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(char_id) DO UPDATE SET
                archetype = excluded.archetype,
                realm = excluded.realm,
                faction_id = excluded.faction_id,
                recent_summary = excluded.recent_summary,
                last_referenced_wall = excluded.last_referenced_wall,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                digest.char_id,
                digest.archetype,
                digest.realm,
                digest.faction_id,
                digest.recent_summary,
                digest.last_referenced_wall,
                NPC_ROW_SCHEMA_VERSION,
                wall_clock,
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

/// plan-offscreen-war-v1 P3：把一行待物化战场遗物 upsert 进 `pending_dormant_relics`
/// （仿 [`upsert_npc_digest`] 签名）。`loot_seed: u64` 经 `as i64` 位投影存（sqlite 无 u64）。
/// `relic_id` 是 UUID PK，正常情况下每个 event 唯一；用 upsert 是为幂等（同一 event 万一被
/// 重复消费也不双写）。**不碰 ledger / WorldQiAccount**——遗物零真元（§10.1 #5 ④）。
pub(super) fn upsert_pending_dormant_relic(
    transaction: &rusqlite::Transaction<'_>,
    record: &PendingDormantRelicRecord,
) -> io::Result<()> {
    transaction
        .execute(
            "
            INSERT INTO pending_dormant_relics (
                relic_id,
                char_id,
                zone,
                pos_x,
                pos_y,
                pos_z,
                archetype,
                loot_seed,
                created_tick,
                created_wall,
                schema_version
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ON CONFLICT(relic_id) DO UPDATE SET
                char_id = excluded.char_id,
                zone = excluded.zone,
                pos_x = excluded.pos_x,
                pos_y = excluded.pos_y,
                pos_z = excluded.pos_z,
                archetype = excluded.archetype,
                loot_seed = excluded.loot_seed,
                created_tick = excluded.created_tick,
                -- plan-offscreen-war-v1 P3 review-fix（CodeRabbit Major）：冲突时**保留更早的**
                -- created_wall。它是 TTL retention sweep 与 hydrate 排序的墙钟锚点；若覆盖成新事件
                -- 的墙钟，同一逻辑死亡重发 / 重试会刷新 TTL（陈旧遗物被无限续命）、并打乱 hydrate
                -- 排序。幂等必须对**可观察 TTL** 也成立，而不只对去重成立——故取两者的最小值。
                created_wall = MIN(pending_dormant_relics.created_wall, excluded.created_wall),
                schema_version = excluded.schema_version
            ",
            params![
                record.relic_id,
                record.char_id,
                record.zone,
                record.pos_x,
                record.pos_y,
                record.pos_z,
                record.archetype,
                record.loot_seed as i64,
                record.created_tick,
                record.created_wall,
                NPC_ROW_SCHEMA_VERSION,
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

/// 原子持久化 dormant 终局：staged zone sink、固定 runtime qi accounts、幂等 tombstone
/// 与可选零真元遗物在同一个 SQLite transaction 中提交。首次提交返回 `Committed`；同一
/// `char_id` 已有 tombstone 时返回 `AlreadyCommitted`，且绝不重写 sink 或终局上下文。
pub fn persist_dormant_terminal_commit(
    settings: &PersistenceSettings,
    record: &DormantTerminalCommitRecord,
    zones: &crate::world::zone::ZoneRegistry,
    qi_ledger: &WorldQiAccount,
    relic: Option<&crate::npc::dormant::PendingDormantRelicCreated>,
) -> io::Result<PersistDormantTerminalOutcome> {
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    let exists = transaction
        .query_row(
            "SELECT 1 FROM dormant_terminal_commits WHERE char_id = ?1",
            params![record.char_id],
            |_| Ok(()),
        )
        .optional()
        .map_err(io::Error::other)?
        .is_some();
    if exists {
        transaction.rollback().map_err(io::Error::other)?;
        return Ok(PersistDormantTerminalOutcome::AlreadyCommitted);
    }

    let wall_clock = current_unix_seconds();
    persist_zone_runtime_records(&transaction, zones, wall_clock)?;
    upsert_runtime_qi_account_balances(&transaction, qi_ledger, wall_clock)?;
    if let Some(event) = relic {
        let relic = PendingDormantRelicRecord {
            relic_id: deterministic_relic_id(event),
            char_id: event.char_id.clone(),
            zone: event.zone.clone(),
            pos_x: event.position[0],
            pos_y: event.position[1],
            pos_z: event.position[2],
            archetype: event.archetype.as_str().to_string(),
            loot_seed: event.loot_seed,
            created_tick: i64::try_from(event.created_tick).unwrap_or(i64::MAX),
            created_wall: wall_clock,
        };
        upsert_pending_dormant_relic(&transaction, &relic)?;
    }
    transaction
        .execute(
            "
            INSERT INTO dormant_terminal_commits (
                char_id, cause, at_tick, zone, winner, winner_group, loser_group,
                zone_accepted, cleanup_revision, created_wall, schema_version
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, ?9, ?10)
            ",
            params![
                record.char_id,
                record.cause,
                i64::try_from(record.at_tick).unwrap_or(i64::MAX),
                record.zone,
                record.winner,
                record.winner_group.map(|value| value as i64),
                record.loser_group.map(|value| value as i64),
                record.zone_accepted,
                wall_clock,
                NPC_ROW_SCHEMA_VERSION,
            ],
        )
        .map_err(io::Error::other)?;
    transaction.commit().map_err(io::Error::other)?;
    Ok(PersistDormantTerminalOutcome::Committed)
}

pub fn load_dormant_terminal_commits(
    settings: &PersistenceSettings,
) -> io::Result<Vec<DormantTerminalCommitRecord>> {
    let connection = open_persistence_connection(settings)?;
    let mut statement = connection
        .prepare(
            "
            SELECT char_id, cause, at_tick, zone, winner, winner_group, loser_group,
                   zone_accepted, cleanup_revision
            FROM dormant_terminal_commits
            ORDER BY char_id
            ",
        )
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map([], |row| {
            Ok(DormantTerminalCommitRecord {
                char_id: row.get(0)?,
                cause: row.get(1)?,
                at_tick: row.get::<_, i64>(2)? as u64,
                zone: row.get(3)?,
                winner: row.get(4)?,
                winner_group: row.get::<_, Option<i64>>(5)?.map(|value| value as u64),
                loser_group: row.get::<_, Option<i64>>(6)?.map(|value| value as u64),
                zone_accepted: row.get(7)?,
                cleanup_revision: row.get::<_, Option<i64>>(8)?.map(|value| value as u64),
            })
        })
        .map_err(io::Error::other)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(io::Error::other)
}

pub fn rearm_dormant_terminal_commits(
    settings: &PersistenceSettings,
) -> io::Result<Vec<DormantTerminalCommitRecord>> {
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    transaction
        .execute(
            "UPDATE dormant_terminal_commits SET cleanup_revision = NULL",
            [],
        )
        .map_err(io::Error::other)?;
    transaction.commit().map_err(io::Error::other)?;
    load_dormant_terminal_commits(settings)
}

pub fn bind_dormant_terminal_cleanup_revision(
    settings: &PersistenceSettings,
    char_ids: &[String],
    revision: u64,
) -> io::Result<()> {
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    for char_id in char_ids {
        transaction
            .execute(
                "
                UPDATE dormant_terminal_commits
                SET cleanup_revision = ?2
                WHERE char_id = ?1 AND cleanup_revision IS NULL
                ",
                params![char_id, i64::try_from(revision).unwrap_or(i64::MAX)],
            )
            .map_err(io::Error::other)?;
    }
    transaction.commit().map_err(io::Error::other)
}

pub fn clear_dormant_terminal_commits_through_revision(
    settings: &PersistenceSettings,
    revision: u64,
) -> io::Result<usize> {
    let connection = open_persistence_connection(settings)?;
    connection
        .execute(
            "
            DELETE FROM dormant_terminal_commits
            WHERE cleanup_revision IS NOT NULL AND cleanup_revision <= ?1
            ",
            params![i64::try_from(revision).unwrap_or(i64::MAX)],
        )
        .map_err(io::Error::other)
}

pub fn persist_pending_dormant_relic(
    settings: &PersistenceSettings,
    record: &PendingDormantRelicRecord,
) -> io::Result<()> {
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    upsert_pending_dormant_relic(&transaction, record)?;
    transaction.commit().map_err(io::Error::other)?;
    Ok(())
}

/// plan-offscreen-war-v1 P3：读出某个 zone 全部待物化战场遗物（按 created_wall 稳定排序，
/// 让 deferred-on-hydrate 物化顺序确定性）。`loot_seed` 从 i64 投影回 u64（无损往返）。
/// 消费方：`npc::dormant::relic_hydrate::hydrate_pending_dormant_relics_system`（交付物 3）。
pub fn load_pending_dormant_relics_for_zone(
    settings: &PersistenceSettings,
    zone: &str,
) -> io::Result<Vec<PendingDormantRelicRecord>> {
    let connection = open_persistence_connection(settings)?;
    let mut statement = connection
        .prepare(
            "
            SELECT relic_id, char_id, zone, pos_x, pos_y, pos_z, archetype,
                   loot_seed, created_tick, created_wall
            FROM pending_dormant_relics
            WHERE zone = ?1
            ORDER BY created_wall ASC, relic_id ASC
            ",
        )
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map(params![zone], |row| {
            Ok(PendingDormantRelicRecord {
                relic_id: row.get(0)?,
                char_id: row.get(1)?,
                zone: row.get(2)?,
                pos_x: row.get(3)?,
                pos_y: row.get(4)?,
                pos_z: row.get(5)?,
                archetype: row.get(6)?,
                loot_seed: row.get::<_, i64>(7)? as u64,
                created_tick: row.get(8)?,
                created_wall: row.get(9)?,
            })
        })
        .map_err(io::Error::other)?;

    let mut relics = Vec::new();
    for row in rows {
        relics.push(row.map_err(io::Error::other)?);
    }
    Ok(relics)
}

/// plan-offscreen-war-v1 P3：删一行已物化（hydrate 消费完）的战场遗物。消费后立刻删，
/// 保证同一遗物不被二次物化（玩家拾走后再次靠近不再凭空再生一份 loot）。
/// 消费方：`npc::dormant::relic_hydrate::hydrate_pending_dormant_relics_system`（交付物 3）。
pub fn delete_pending_dormant_relic(
    settings: &PersistenceSettings,
    relic_id: &str,
) -> io::Result<()> {
    let connection = open_persistence_connection(settings)?;
    connection
        .execute(
            "DELETE FROM pending_dormant_relics WHERE relic_id = ?1",
            params![relic_id],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

/// plan-offscreen-war-v1 P3：清掉 `created_wall` 早于 `now - PENDING_RELIC_RETENTION_SECS`
/// 的陈旧战场遗物（仿 [`sweep_stale_npc_digests`]，但无 zstd 归档——遗物只是 ground loot
/// 占位，过期即风化，不值得归档）。返回被清掉的行数（telemetry / 测试断言用）。
pub fn sweep_stale_dormant_relics(
    settings: &PersistenceSettings,
    now_wall: i64,
) -> io::Result<usize> {
    let threshold = now_wall.saturating_sub(PENDING_RELIC_RETENTION_SECS);
    let connection = open_persistence_connection(settings)?;
    let removed = connection
        .execute(
            "DELETE FROM pending_dormant_relics WHERE created_wall < ?1",
            params![threshold],
        )
        .map_err(io::Error::other)?;
    Ok(removed)
}

pub(super) fn upsert_archetype_registry_entry(
    transaction: &rusqlite::Transaction<'_>,
    entry: &ArchetypeRegistryEntry,
    wall_clock: i64,
) -> io::Result<()> {
    transaction
        .execute(
            "
            INSERT INTO archetype_registry (
                char_id,
                archetype,
                since_tick,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(char_id, since_tick, archetype) DO UPDATE SET
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                entry.char_id,
                entry.archetype,
                tick_to_sql(entry.since_tick)?,
                NPC_ROW_SCHEMA_VERSION,
                wall_clock,
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

pub(super) fn upsert_npc_deceased_index(
    transaction: &rusqlite::Transaction<'_>,
    entry: &NpcDeceasedIndexRecord,
    wall_clock: i64,
) -> io::Result<()> {
    transaction
        .execute(
            "
            INSERT INTO npc_deceased_index (
                char_id,
                archetype,
                died_at_tick,
                path,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(char_id) DO UPDATE SET
                archetype = excluded.archetype,
                died_at_tick = excluded.died_at_tick,
                path = excluded.path,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                entry.char_id,
                entry.archetype,
                tick_to_sql(entry.died_at_tick)?,
                entry.path,
                NPC_ROW_SCHEMA_VERSION,
                wall_clock,
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

pub(super) fn delete_npc_hot_rows(
    transaction: &rusqlite::Transaction<'_>,
    char_id: &str,
) -> io::Result<()> {
    transaction
        .execute("DELETE FROM npc_state WHERE char_id = ?1", params![char_id])
        .map_err(io::Error::other)?;
    transaction
        .execute(
            "DELETE FROM npc_digests WHERE char_id = ?1",
            params![char_id],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn load_npc_state_from_connection(
    connection: &Connection,
    char_id: &str,
) -> io::Result<Option<NpcStateRecord>> {
    let row: Option<NpcStateSqlRow> = connection
        .query_row(
            "
            SELECT kind, archetype, pos_x, pos_y, pos_z, state, blackboard_json, home_zone,
                   patrol_anchor_index, patrol_target_x, patrol_target_y, patrol_target_z,
                   movement_mode, can_sprint, can_dash, sprint_ready_at, dash_ready_at,
                   lifecycle_state, death_count, last_death_tick, last_revive_tick
            FROM npc_state
            WHERE char_id = ?1
            ",
            params![char_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get(9)?,
                    row.get(10)?,
                    row.get(11)?,
                    row.get(12)?,
                    row.get(13)?,
                    row.get(14)?,
                    row.get(15)?,
                    row.get(16)?,
                    row.get(17)?,
                    row.get(18)?,
                    row.get(19)?,
                    row.get(20)?,
                ))
            },
        )
        .optional()
        .map_err(io::Error::other)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let blackboard = serde_json::from_str(&row.6)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    Ok(Some(NpcStateRecord {
        char_id: char_id.to_string(),
        kind: row.0,
        archetype: row.1,
        pos: [row.2, row.3, row.4],
        state: row.5,
        blackboard,
        home_zone: row.7,
        patrol_anchor_index: sql_to_usize(row.8)?,
        patrol_target: [row.9, row.10, row.11],
        movement_mode: row.12,
        can_sprint: sql_to_bool(row.13),
        can_dash: sql_to_bool(row.14),
        sprint_ready_at: sql_to_u32(row.15)?,
        dash_ready_at: sql_to_u32(row.16)?,
        lifecycle_state: row.17,
        death_count: sql_to_u32(row.18)?,
        last_death_tick: optional_sql_to_tick(row.19)?,
        last_revive_tick: optional_sql_to_tick(row.20)?,
    }))
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn load_npc_digest_from_connection(
    connection: &Connection,
    char_id: &str,
) -> io::Result<Option<NpcDigestRecord>> {
    let row: Option<(String, String, Option<String>, String, i64)> = connection
        .query_row(
            "
            SELECT archetype, realm, faction_id, recent_summary, last_referenced_wall
            FROM npc_digests
            WHERE char_id = ?1
            ",
            params![char_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .optional()
        .map_err(io::Error::other)?;
    Ok(row.map(
        |(archetype, realm, faction_id, recent_summary, last_referenced_wall)| NpcDigestRecord {
            char_id: char_id.to_string(),
            archetype,
            realm,
            faction_id,
            recent_summary,
            last_referenced_wall,
        },
    ))
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn load_archetype_registry_from_connection(
    connection: &Connection,
    char_id: &str,
) -> io::Result<Vec<ArchetypeRegistryEntry>> {
    let mut statement = connection
        .prepare(
            "
            SELECT archetype, since_tick
            FROM archetype_registry
            WHERE char_id = ?1
            ORDER BY since_tick ASC, archetype ASC
            ",
        )
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map(params![char_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(io::Error::other)?;

    let mut registry = Vec::new();
    for row in rows {
        let (archetype, since_tick) = row.map_err(io::Error::other)?;
        registry.push(ArchetypeRegistryEntry {
            char_id: char_id.to_string(),
            archetype,
            since_tick: sql_to_tick(since_tick)?,
        });
    }
    Ok(registry)
}

pub(super) fn load_stale_npc_digests(
    connection: &Connection,
    threshold: i64,
) -> io::Result<Vec<NpcDigestRecord>> {
    let mut statement = connection
        .prepare(
            "
            SELECT char_id, archetype, realm, faction_id, recent_summary, last_referenced_wall
            FROM npc_digests
            WHERE last_referenced_wall < ?1
            ORDER BY last_referenced_wall ASC, char_id ASC
            ",
        )
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map(params![threshold], |row| {
            Ok(NpcDigestRecord {
                char_id: row.get(0)?,
                archetype: row.get(1)?,
                realm: row.get(2)?,
                faction_id: row.get(3)?,
                recent_summary: row.get(4)?,
                last_referenced_wall: row.get(5)?,
            })
        })
        .map_err(io::Error::other)?;

    let mut digests = Vec::new();
    for row in rows {
        digests.push(row.map_err(io::Error::other)?);
    }
    Ok(digests)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) type NpcStateSqlRow = (
    String,
    String,
    f64,
    f64,
    f64,
    String,
    String,
    String,
    i64,
    f64,
    f64,
    f64,
    String,
    i64,
    i64,
    i64,
    i64,
    String,
    i64,
    Option<i64>,
    Option<i64>,
);
