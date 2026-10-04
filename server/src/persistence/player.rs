//! Player cultivation and durable dropped-loot persistence helpers.

use super::*;

type PlayerRuntimeAttachQueryItem<'a> = (Entity, &'a Username);
type PlayerRuntimeAttachQueryFilter = (
    With<Client>,
    With<crate::player::state::PlayerState>,
    Without<PlayerRuntimeSlicesLoaded>,
);
type PlayerRuntimeStateQueryItem<'a> = (
    &'a Username,
    Option<&'a crate::world::tiandao_hunt::TiandaoAttention>,
    Option<&'a crate::cultivation::realm_taint::RealmTaintState>,
    Option<&'a PlayerRuntimeSlicesLoadFailed>,
    &'a PlayerRuntimeSlicesLoaded,
);

pub(super) struct PlayerRuntimePersistenceSlice;

impl PersistenceSlice for PlayerRuntimePersistenceSlice {
    fn descriptor() -> &'static SliceDescriptor {
        &PLAYER_RUNTIME_SLICE_DESCRIPTOR
    }
}

const PLAYER_RUNTIME_SLICE_DESCRIPTOR: SliceDescriptor = SliceDescriptor {
    id: SliceId::new("player.runtime_state"),
    scope: SliceScope::PlayerEntity,
    order: 20,
    load_failure: LoadFailurePolicy::BlockWrites,
    time_basis: TimeBasis::None,
    write_binding: WriteBinding::new(
        WriteDomain::new("player.runtime_state"),
        WriteAuthority::new("persistence.player_runtime_state"),
    ),
    write_ordering: WriteOrdering::Serialized,
    autosave: AutosavePolicy::EveryTicks(60 * crate::combat::components::TICKS_PER_SECOND),
    hydrate: None,
    reconnect_preflight: None,
    reconnect_cleanup: None,
    rebase: None,
    disconnect_save: None,
    shutdown_flush: Some(flush_player_runtime_slice),
};

#[derive(Component, Debug, Default)]
pub(super) struct PlayerRuntimeSlicesLoaded;

#[derive(Component, Debug, Default)]
pub(super) struct PlayerRuntimeSlicesLoadFailed;

pub(super) fn hydrate_player_runtime_slices(
    mut commands: Commands,
    settings: Res<PersistenceSettings>,
    players: Query<PlayerRuntimeAttachQueryItem<'_>, PlayerRuntimeAttachQueryFilter>,
) {
    for (entity, username) in &players {
        let mut failed = false;
        match load_player_runtime_slice::<crate::world::tiandao_hunt::TiandaoAttention>(
            &settings,
            username.0.as_str(),
            "player.tiandao_attention",
        ) {
            Ok(Some(attention)) if valid_tiandao_attention(&attention) => {
                commands.entity(entity).insert(attention);
            }
            Ok(Some(_)) => {
                failed = true;
                tracing::error!(
                    "[bong][persistence] refusing to attach invalid TiandaoAttention for `{}`",
                    username.0
                );
            }
            Ok(None) => {}
            Err(error) => {
                failed = true;
                tracing::error!(
                    "[bong][persistence] refusing to overwrite TiandaoAttention for `{}` after load failure: {error}",
                    username.0
                );
            }
        }
        match load_player_runtime_slice::<crate::cultivation::realm_taint::RealmTaintState>(
            &settings,
            username.0.as_str(),
            "player.realm_taint",
        ) {
            Ok(Some(taint)) if valid_realm_taint_state(&taint) => {
                commands.entity(entity).insert(taint);
            }
            Ok(Some(_)) => {
                failed = true;
                tracing::error!(
                    "[bong][persistence] refusing to attach invalid RealmTaintState for `{}`",
                    username.0
                );
            }
            Ok(None) => {}
            Err(error) => {
                failed = true;
                tracing::error!(
                    "[bong][persistence] refusing to overwrite RealmTaintState for `{}` after load failure: {error}",
                    username.0
                );
            }
        }
        let mut entity_commands = commands.entity(entity);
        entity_commands.insert(PlayerRuntimeSlicesLoaded);
        if failed {
            entity_commands.insert(PlayerRuntimeSlicesLoadFailed);
        }
    }
}

fn valid_tiandao_attention(attention: &crate::world::tiandao_hunt::TiandaoAttention) -> bool {
    attention.level.is_finite()
        && attention.level >= 0.0
        && attention.accumulation_rate.is_finite()
        && attention.accumulation_rate >= 0.0
        && attention.peak_level.is_finite()
        && attention.peak_level >= 0.0
}

fn valid_realm_taint_state(state: &crate::cultivation::realm_taint::RealmTaintState) -> bool {
    state.qi_taint_severity.is_finite() && (0.0..=1.0).contains(&state.qi_taint_severity)
}

pub(super) fn autosave_player_runtime_slices(
    settings: Res<PersistenceSettings>,
    timer: Option<Res<crate::player::state::PlayerStateAutosaveTimer>>,
    players: Query<PlayerRuntimeStateQueryItem<'_>, With<Client>>,
) {
    const RUNTIME_SLICE_FLUSH_INTERVAL_TICKS: u64 =
        60 * crate::combat::components::TICKS_PER_SECOND;
    let Some(timer) = timer else {
        return;
    };
    if !timer
        .ticks
        .is_multiple_of(RUNTIME_SLICE_FLUSH_INTERVAL_TICKS)
    {
        return;
    }
    for (username, attention, taint, load_failed, _loaded) in &players {
        if load_failed.is_some() {
            continue;
        }
        if let Some(attention) = attention {
            if let Err(error) = save_player_runtime_slice(
                &settings,
                username.0.as_str(),
                "player.tiandao_attention",
                attention,
            ) {
                tracing::warn!(
                    "[bong][persistence] attention autosave failed for `{}`: {error}",
                    username.0
                );
            }
        }
        if let Some(taint) = taint {
            if let Err(error) = save_player_runtime_slice(
                &settings,
                username.0.as_str(),
                "player.realm_taint",
                taint,
            ) {
                tracing::warn!(
                    "[bong][persistence] realm taint autosave failed for `{}`: {error}",
                    username.0
                );
            }
        } else if let Err(error) =
            delete_player_runtime_slice(&settings, username.0.as_str(), "player.realm_taint")
        {
            tracing::warn!(
                "[bong][persistence] realm taint deletion failed for `{}`: {error}",
                username.0
            );
        }
    }
}

pub(super) fn persist_disconnected_player_runtime_slices(
    settings: Res<PersistenceSettings>,
    mut disconnected: RemovedComponents<Client>,
    players: Query<PlayerRuntimeStateQueryItem<'_>>,
) {
    for entity in disconnected.read() {
        let Ok((username, attention, taint, load_failed, _loaded)) = players.get(entity) else {
            continue;
        };
        if load_failed.is_some() {
            continue;
        }
        if let Err(error) =
            persist_player_runtime_components(&settings, username.0.as_str(), attention, taint)
        {
            tracing::warn!(
                "[bong][persistence] disconnected player runtime flush failed for `{}`: {error}",
                username.0
            );
        }
    }
}

fn persist_player_runtime_components(
    settings: &PersistenceSettings,
    username: &str,
    attention: Option<&crate::world::tiandao_hunt::TiandaoAttention>,
    taint: Option<&crate::cultivation::realm_taint::RealmTaintState>,
) -> io::Result<()> {
    let mut failures = Vec::new();
    if let Some(attention) = attention {
        if let Err(error) =
            save_player_runtime_slice(settings, username, "player.tiandao_attention", attention)
        {
            failures.push(format!("TiandaoAttention: {error}"));
        }
    }
    let taint_result = match taint {
        Some(taint) => save_player_runtime_slice(settings, username, "player.realm_taint", taint),
        None => delete_player_runtime_slice(settings, username, "player.realm_taint"),
    };
    if let Err(error) = taint_result {
        failures.push(format!("RealmTaintState: {error}"));
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(io::Error::other(failures.join("; ")))
    }
}

fn flush_player_runtime_slice(world: &mut World, _context: &SliceRunContext) -> SliceRunResult {
    let Some(settings) = world.get_resource::<PersistenceSettings>().cloned() else {
        return Err(SliceRunError::new("PersistenceSettings is unavailable"));
    };
    let mut query = world.query::<PlayerRuntimeStateQueryItem<'_>>();
    let mut failures = Vec::new();
    for (username, attention, taint, load_failed, _loaded) in query.iter(world) {
        if load_failed.is_some() {
            continue;
        }
        if let Err(error) =
            persist_player_runtime_components(&settings, username.0.as_str(), attention, taint)
        {
            failures.push(format!("{}: {error}", username.0));
        }
    }
    if failures.is_empty() {
        Ok(SliceRunOutcome::Flushed)
    } else {
        Err(SliceRunError::new(format!(
            "player runtime flush failed: {}",
            failures.join(", ")
        )))
    }
}

pub(crate) fn upsert_player_cultivation_slice(
    transaction: &rusqlite::Transaction<'_>,
    username: &str,
    cultivation: &Cultivation,
    wall_clock: i64,
) -> io::Result<()> {
    let existing: Option<String> = transaction
        .query_row(
            "SELECT cultivation_json FROM player_cultivation WHERE username = ?1",
            params![username],
            |row| row.get(0),
        )
        .optional()
        .map_err(io::Error::other)?;
    let mut bundle = existing
        .map(|json| {
            serde_json::from_str::<serde_json::Value>(&json)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
        })
        .transpose()?
        .unwrap_or_else(|| serde_json::json!({ "v": 1 }));
    let object = bundle.as_object_mut().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("player_cultivation for `{username}` must be a JSON object"),
        )
    })?;
    object.insert(
        "cultivation".to_string(),
        serde_json::to_value(
            crate::cultivation::components::encode_persisted_cultivation(cultivation),
        )
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
    );
    let cultivation_json = serde_json::to_string(&bundle)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    transaction
        .execute(
            "
            INSERT INTO player_cultivation (
                username, cultivation_json, schema_version, last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(username) DO UPDATE SET
                cultivation_json = excluded.cultivation_json,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                username,
                cultivation_json,
                CURRENT_SCHEMA_VERSION,
                wall_clock
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

pub(crate) fn upsert_dropped_loot_entries(
    transaction: &rusqlite::Transaction<'_>,
    entries: &[DroppedLootEntry],
    wall_clock: i64,
) -> io::Result<()> {
    for entry in entries {
        if entry.instance_id != entry.item.instance_id || entry.instance_id > JS_SAFE_INTEGER_MAX {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "invalid durable dropped loot id={} item_id={} max={JS_SAFE_INTEGER_MAX}",
                    entry.instance_id, entry.item.instance_id
                ),
            ));
        }
        let entry_json = serde_json::to_string(entry)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        transaction
            .execute(
                "
                INSERT INTO dropped_loot (
                    instance_id, entry_json, schema_version, last_updated_wall
                ) VALUES (?1, ?2, ?3, ?4)
                ON CONFLICT(instance_id) DO UPDATE SET
                    entry_json = excluded.entry_json,
                    schema_version = excluded.schema_version,
                    last_updated_wall = excluded.last_updated_wall
                ",
                params![
                    i64::try_from(entry.instance_id).map_err(io::Error::other)?,
                    entry_json,
                    CURRENT_SCHEMA_VERSION,
                    wall_clock
                ],
            )
            .map_err(io::Error::other)?;
    }
    Ok(())
}

pub(crate) fn delete_dropped_loot_entry(
    transaction: &rusqlite::Transaction<'_>,
    instance_id: u64,
) -> io::Result<()> {
    transaction
        .execute(
            "DELETE FROM dropped_loot WHERE instance_id = ?1",
            params![i64::try_from(instance_id).map_err(io::Error::other)?],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

pub fn load_durable_dropped_loot(
    settings: &PersistenceSettings,
) -> io::Result<HashMap<u64, DroppedLootEntry>> {
    let connection = open_persistence_connection(settings)?;
    let mut statement = connection
        .prepare("SELECT instance_id, entry_json FROM dropped_loot ORDER BY instance_id")
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(io::Error::other)?;
    let mut entries = HashMap::new();
    for row in rows {
        let (stored_id, entry_json) = row.map_err(io::Error::other)?;
        let stored_id = u64::try_from(stored_id).map_err(io::Error::other)?;
        let entry: DroppedLootEntry = serde_json::from_str(&entry_json)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        if stored_id != entry.instance_id
            || entry.instance_id != entry.item.instance_id
            || stored_id > JS_SAFE_INTEGER_MAX
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "durable dropped loot id mismatch row={stored_id} entry={} item={}",
                    entry.instance_id, entry.item.instance_id
                ),
            ));
        }
        entries.insert(stored_id, entry);
    }
    Ok(entries)
}

pub fn persisted_inventory_instance_id_high_water(
    settings: &PersistenceSettings,
) -> io::Result<Option<u64>> {
    fn visit(value: &serde_json::Value, high_water: &mut Option<u64>) -> io::Result<()> {
        match value {
            serde_json::Value::Object(fields) => {
                for (key, value) in fields {
                    if key == "instance_id" {
                        let id = value.as_u64().ok_or_else(|| {
                            io::Error::new(
                                io::ErrorKind::InvalidData,
                                format!("persisted `{key}` must be an unsigned integer"),
                            )
                        })?;
                        if id > JS_SAFE_INTEGER_MAX {
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                format!(
                                    "persisted inventory instance id {id} exceeds JS safe integer max {JS_SAFE_INTEGER_MAX}"
                                ),
                            ));
                        }
                        *high_water = Some(high_water.map_or(id, |current| current.max(id)));
                    }
                    visit(value, high_water)?;
                }
            }
            serde_json::Value::Array(values) => {
                for value in values {
                    visit(value, high_water)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    let connection = open_persistence_connection(settings)?;
    let mut high_water = None;
    let mut statement = connection
        .prepare("SELECT inventory_json FROM inventories")
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(io::Error::other)?;
    for row in rows {
        let json = row.map_err(io::Error::other)?;
        let value: serde_json::Value = serde_json::from_str(&json)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        visit(&value, &mut high_water)?;
    }
    let durable = load_durable_dropped_loot(settings)?;
    for id in durable.keys().copied() {
        high_water = Some(high_water.map_or(id, |current| current.max(id)));
    }
    Ok(high_water)
}

#[allow(clippy::too_many_arguments)]
pub fn persist_player_cultivation_bundle(
    settings: &PersistenceSettings,
    username: &str,
    cultivation: &crate::cultivation::components::Cultivation,
    meridians: &crate::cultivation::components::MeridianSystem,
    qi_color: &crate::cultivation::components::QiColor,
    karma: &crate::cultivation::components::Karma,
    contamination: &crate::cultivation::components::Contamination,
    life_record: &crate::cultivation::life_record::LifeRecord,
    practice_log: &crate::cultivation::color::PracticeLog,
    insight_quota: &crate::cultivation::insight::InsightQuota,
    unlocked_perceptions: &crate::cultivation::insight_apply::UnlockedPerceptions,
    insight_modifiers: &crate::cultivation::insight_apply::InsightModifiers,
    tutorial_state: Option<&crate::world::spawn_tutorial::TutorialState>,
    meridian_severed: &crate::cultivation::meridian::severed::MeridianSeveredPermanent,
    poison_toxicity: Option<&crate::cultivation::poison_trait::PoisonToxicity>,
    digestion_load: Option<&crate::cultivation::poison_trait::DigestionLoad>,
) -> io::Result<()> {
    let wall_clock = current_unix_seconds();
    let persisted_cultivation =
        crate::cultivation::components::encode_persisted_cultivation(cultivation);
    let bundle = serde_json::json!({
        // plan-race-system-v1 P1a —— bump 1→2：`meridians`/`meridian_severed` 子字段
        // channel id 从 `MeridianId` PascalCase 枚举名换轨为 humanoid.json 声明的
        // snake_case `MeridianChannelId`（见
        // `crate::cultivation::legacy_meridian_bundle`）。旧存档（v1 或缺失 `"v"`）
        // 载入时在该模块显式迁移，此处只负责新写入必须标最新版本号。
        "v": crate::cultivation::legacy_meridian_bundle::CURRENT_BUNDLE_VERSION,
        "cultivation": persisted_cultivation,
        "meridians": meridians,
        "qi_color": qi_color,
        "karma": karma,
        "contamination": contamination,
        "life_record": life_record,
        "practice_log": practice_log,
        "insight_quota": insight_quota,
        "unlocked_perceptions": unlocked_perceptions,
        "insight_modifiers": insight_modifiers,
        "tutorial_state": tutorial_state,
        "meridian_severed": meridian_severed,
        "poison_toxicity": poison_toxicity,
        "digestion_load": digestion_load,
    });
    let cultivation_json = serde_json::to_string(&bundle)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    let connection = open_persistence_connection(settings)?;
    connection
        .execute(
            "
            INSERT INTO player_cultivation (
                username,
                cultivation_json,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(username) DO UPDATE SET
                cultivation_json = excluded.cultivation_json,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                username,
                cultivation_json,
                CURRENT_SCHEMA_VERSION,
                wall_clock
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn load_player_cultivation_bundle(
    settings: &PersistenceSettings,
    username: &str,
) -> io::Result<Option<serde_json::Value>> {
    let connection = open_persistence_connection(settings)?;
    let row: Option<String> = connection
        .query_row(
            "
            SELECT cultivation_json
            FROM player_cultivation
            WHERE username = ?1
            ",
            params![username],
            |row| row.get(0),
        )
        .optional()
        .map_err(io::Error::other)?;
    let Some(json) = row else {
        return Ok(None);
    };
    let decoded = serde_json::from_str(&json)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    Ok(Some(decoded))
}
