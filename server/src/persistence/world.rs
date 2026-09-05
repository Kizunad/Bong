//! World, zone, heartbeat, overlay, influence, and runtime persistence.

use super::*;

#[derive(Debug, Default)]
pub(super) struct ZoneRuntimeSnapshotState {
    pub(super) last_snapshot_wall: i64,
}

impl Resource for ZoneRuntimeSnapshotState {}

/// plan-territory-v1 P0：zone_influence snapshot 的节流状态。
#[derive(Debug, Default)]
pub(super) struct ZoneInfluenceSnapshotState {
    last_snapshot_wall: i64,
}

impl Resource for ZoneInfluenceSnapshotState {}

pub(super) struct ZoneRuntimePersistenceSlice;

impl PersistenceSlice for ZoneRuntimePersistenceSlice {
    fn descriptor() -> &'static SliceDescriptor {
        &ZONE_RUNTIME_SLICE_DESCRIPTOR
    }
}

const ZONE_RUNTIME_SLICE_DESCRIPTOR: SliceDescriptor = SliceDescriptor {
    id: SliceId::new("world.zone_runtime"),
    scope: SliceScope::WorldResource,
    order: 100,
    load_failure: LoadFailurePolicy::RefuseStartup,
    time_basis: TimeBasis::None,
    write_binding: WriteBinding::new(
        WriteDomain::new("world.zone_runtime"),
        WriteAuthority::new("persistence.zone_runtime"),
    ),
    write_ordering: WriteOrdering::Serialized,
    autosave: AutosavePolicy::Disabled,
    hydrate: None,
    reconnect_preflight: None,
    reconnect_cleanup: None,
    rebase: None,
    disconnect_save: None,
    shutdown_flush: Some(flush_zone_runtime_slice),
};

pub(super) fn persist_zone_runtime_system(
    settings: Res<PersistenceSettings>,
    mut snapshot_state: ResMut<ZoneRuntimeSnapshotState>,
    zones: Option<Res<crate::world::zone::ZoneRegistry>>,
    heartbeat: Option<Res<WorldHeartbeat>>,
    qi_ledger: Res<WorldQiAccount>,
    clock: Res<CultivationClock>,
) {
    let Some(zone_registry) = zones else {
        return;
    };

    let wall_clock = current_unix_seconds();
    if snapshot_state.last_snapshot_wall > 0
        && wall_clock.saturating_sub(snapshot_state.last_snapshot_wall)
            < ZONE_RUNTIME_SNAPSHOT_INTERVAL_SECS
    {
        return;
    }

    match persist_zone_runtime_snapshot_with_heartbeat_at_tick(
        &settings,
        &zone_registry,
        heartbeat.as_deref(),
        &qi_ledger,
        clock.tick,
    ) {
        Ok(_) => {
            snapshot_state.last_snapshot_wall = wall_clock;
        }
        Err(error) => tracing::warn!(
            "[bong][persistence] failed to persist zone runtime snapshot at {}: {error}",
            settings.db_path().display()
        ),
    }
}

pub(super) fn flush_zone_runtime_slice(
    world: &mut World,
    _context: &SliceRunContext,
) -> SliceRunResult {
    if !world.contains_resource::<crate::world::zone::ZoneRegistry>() {
        return Ok(SliceRunOutcome::Clean);
    }
    if !world.contains_resource::<PersistenceSettings>() {
        return Err(SliceRunError::new("PersistenceSettings is unavailable"));
    }
    world.resource_scope(
        |world, settings: valence::prelude::Mut<PersistenceSettings>| {
            world.resource_scope(
                |world, zones: valence::prelude::Mut<crate::world::zone::ZoneRegistry>| {
                    let heartbeat = world.get_resource::<WorldHeartbeat>();
                    let qi_ledger = world
                        .get_resource::<WorldQiAccount>()
                        .ok_or_else(|| SliceRunError::new("WorldQiAccount is unavailable"))?;
                    let clock_tick = world
                        .get_resource::<CultivationClock>()
                        .ok_or_else(|| SliceRunError::new("CultivationClock is unavailable"))?
                        .tick;

                    persist_zone_runtime_snapshot_with_heartbeat_at_tick(
                        &settings, &zones, heartbeat, qi_ledger, clock_tick,
                    )
                    .map(|_| SliceRunOutcome::Flushed)
                    .map_err(|error| SliceRunError::new(error.to_string()))
                },
            )
        },
    )
}

/// plan-territory-v1 P0：zone_influence 快照 system（节流 ZONE_RUNTIME_SNAPSHOT_INTERVAL_SECS）。
pub(super) fn persist_zone_influence_system(
    settings: Res<PersistenceSettings>,
    mut snapshot_state: ResMut<ZoneInfluenceSnapshotState>,
    influence_map: Option<Res<crate::world::territory::ZoneInfluenceMap>>,
) {
    let Some(influence_map) = influence_map else {
        return;
    };

    let wall_clock = current_unix_seconds();
    if snapshot_state.last_snapshot_wall > 0
        && wall_clock.saturating_sub(snapshot_state.last_snapshot_wall)
            < ZONE_RUNTIME_SNAPSHOT_INTERVAL_SECS
    {
        return;
    }

    match persist_zone_influence_snapshot(&settings, &influence_map) {
        Ok(_) => {
            snapshot_state.last_snapshot_wall = wall_clock;
        }
        Err(error) => tracing::warn!(
            "[bong][persistence] failed to persist zone influence snapshot at {}: {error}",
            settings.db_path().display()
        ),
    }
}

pub fn persist_zone_and_runtime_qi_snapshot(
    settings: &PersistenceSettings,
    zones: Option<&crate::world::zone::ZoneRegistry>,
    qi_ledger: &WorldQiAccount,
) -> io::Result<()> {
    let wall_clock = current_unix_seconds();
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    if let Some(zones) = zones {
        persist_zone_runtime_records(&transaction, zones, wall_clock)?;
    }
    upsert_runtime_qi_account_balances(&transaction, qi_ledger, wall_clock)?;
    transaction.commit().map_err(io::Error::other)
}

pub fn persist_zone_runtime_snapshot(
    settings: &PersistenceSettings,
    zones: &crate::world::zone::ZoneRegistry,
) -> io::Result<()> {
    let wall_clock = current_unix_seconds();
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    persist_zone_runtime_records(&transaction, zones, wall_clock)?;
    transaction.commit().map_err(io::Error::other)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn persist_zone_runtime_snapshot_with_heartbeat(
    settings: &PersistenceSettings,
    zones: &crate::world::zone::ZoneRegistry,
    heartbeat: Option<&WorldHeartbeat>,
    qi_ledger: &WorldQiAccount,
) -> io::Result<()> {
    let current_tick = heartbeat
        .map(|heartbeat| heartbeat.last_eval_tick)
        .unwrap_or_default();
    persist_zone_runtime_snapshot_with_heartbeat_at_tick(
        settings,
        zones,
        heartbeat,
        qi_ledger,
        current_tick,
    )
}

pub(super) fn persist_zone_runtime_snapshot_with_heartbeat_at_tick(
    settings: &PersistenceSettings,
    zones: &crate::world::zone::ZoneRegistry,
    heartbeat: Option<&WorldHeartbeat>,
    qi_ledger: &WorldQiAccount,
    current_tick: u64,
) -> io::Result<()> {
    let wall_clock = current_unix_seconds();
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    persist_zone_runtime_records(&transaction, zones, wall_clock)?;
    if let Some(heartbeat) = heartbeat {
        let pseudo_veins = heartbeat.active_pseudo_vein_records_at_tick(zones, current_tick);
        replace_heartbeat_pseudo_vein_records(&transaction, &pseudo_veins, wall_clock)?;
    }
    upsert_runtime_qi_account_balances(&transaction, qi_ledger, wall_clock)?;
    transaction.commit().map_err(io::Error::other)
}

pub(super) fn persist_zone_runtime_records(
    transaction: &rusqlite::Transaction<'_>,
    zones: &crate::world::zone::ZoneRegistry,
    wall_clock: i64,
) -> io::Result<()> {
    // heartbeat 动态 zone 会在消散后从 ZoneRegistry 删除。先在同一事务中清掉该命名域的
    // 旧行，再由下方当前 registry 全量重插仍活跃者，避免已结算余额的孤儿行永久残留。
    transaction
        .execute(
            "DELETE FROM zones_runtime WHERE zone_id GLOB 'pseudo_vein_heartbeat_*'",
            [],
        )
        .map_err(io::Error::other)?;
    for zone in &zones.zones {
        upsert_zone_runtime(
            transaction,
            &ZoneRuntimeRecord {
                zone_id: zone.name.clone(),
                spirit_qi: zone.spirit_qi,
                danger_level: zone.danger_level,
            },
            wall_clock,
        )?;
    }
    Ok(())
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn load_zone_runtime_snapshot(
    settings: &PersistenceSettings,
) -> io::Result<Vec<ZoneRuntimeRecord>> {
    let connection = open_persistence_connection(settings)?;
    load_zone_runtime_snapshot_from_connection(&connection)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn persist_heartbeat_pseudo_veins_snapshot(
    settings: &PersistenceSettings,
    heartbeat: &WorldHeartbeat,
    zones: &crate::world::zone::ZoneRegistry,
) -> io::Result<()> {
    let wall_clock = current_unix_seconds();
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    let records = heartbeat.active_pseudo_vein_records(zones);
    replace_heartbeat_pseudo_vein_records(&transaction, &records, wall_clock)?;
    transaction.commit().map_err(io::Error::other)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn load_heartbeat_pseudo_veins_snapshot(
    settings: &PersistenceSettings,
) -> io::Result<Vec<HeartbeatPseudoVeinRecord>> {
    let connection = open_persistence_connection(settings)?;
    load_heartbeat_pseudo_veins_from_connection(&connection)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn persist_zone_overlays(
    settings: &PersistenceSettings,
    overlays: &[ZoneOverlayRecord],
) -> io::Result<()> {
    let wall_clock = current_unix_seconds();
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    transaction
        .execute("DELETE FROM zone_overlays", [])
        .map_err(io::Error::other)?;
    for overlay in overlays {
        upsert_zone_overlay(&transaction, overlay, wall_clock)?;
    }
    transaction.commit().map_err(io::Error::other)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn load_zone_overlays(settings: &PersistenceSettings) -> io::Result<Vec<ZoneOverlayRecord>> {
    let connection = open_persistence_connection(settings)?;
    load_zone_overlays_from_connection(&connection)
}

pub fn export_zone_persistence(settings: &PersistenceSettings) -> io::Result<ZoneExportBundle> {
    Ok(ZoneExportBundle {
        schema_version: CURRENT_SCHEMA_VERSION,
        kind: "zones_export_v1".to_string(),
        zones_runtime: load_zone_runtime_snapshot(settings)?,
        zone_overlays: load_zone_overlays(settings)?,
    })
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn import_zone_persistence(
    settings: &PersistenceSettings,
    bundle: &ZoneExportBundle,
) -> io::Result<()> {
    if bundle.kind != "zones_export_v1" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unexpected zone export kind: {}", bundle.kind),
        ));
    }
    if bundle.schema_version > CURRENT_SCHEMA_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "zone export schema_version {} is newer than supported {}",
                bundle.schema_version, CURRENT_SCHEMA_VERSION
            ),
        ));
    }

    let wall_clock = current_unix_seconds();
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;

    transaction
        .execute("DELETE FROM zones_runtime", [])
        .map_err(io::Error::other)?;
    for runtime in &bundle.zones_runtime {
        upsert_zone_runtime(&transaction, runtime, wall_clock)?;
    }

    transaction
        .execute("DELETE FROM zone_overlays", [])
        .map_err(io::Error::other)?;
    for overlay in &bundle.zone_overlays {
        upsert_zone_overlay(&transaction, overlay, wall_clock)?;
    }

    transaction.commit().map_err(io::Error::other)
}

pub(super) fn hydrate_zone_runtime(
    settings: &PersistenceSettings,
    zones: &mut crate::world::zone::ZoneRegistry,
) -> io::Result<()> {
    let runtime_rows = load_zone_runtime_snapshot(settings)?;
    for record in &runtime_rows {
        if !is_heartbeat_pseudo_vein_zone_namespace(record.zone_id.as_str()) {
            continue;
        }
        let zone = zones
            .find_zone_by_name(record.zone_id.as_str())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "orphan pseudo-vein zone runtime `{}` has no restored lifecycle",
                        record.zone_id
                    ),
                )
            })?;
        if !zone
            .active_events
            .iter()
            .any(|event| event == EVENT_PSEUDO_VEIN)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "pseudo-vein zone runtime `{}` is not backed by an active lifecycle",
                    record.zone_id
                ),
            ));
        }
    }
    zones.apply_runtime_records(&runtime_rows);
    Ok(())
}

pub(super) fn hydrate_heartbeat_pseudo_veins(
    settings: &PersistenceSettings,
    heartbeat: &mut WorldHeartbeat,
    zones: &mut crate::world::zone::ZoneRegistry,
    current_tick: u64,
    current_wall: i64,
) -> io::Result<usize> {
    let pseudo_veins = load_heartbeat_pseudo_veins_snapshot(settings)?;
    let runtime_rows = load_zone_runtime_snapshot(settings)?;
    validate_pseudo_vein_snapshot_pair(&pseudo_veins, &runtime_rows)?;
    let restored = heartbeat.restore_pseudo_vein_records_at_wall(
        zones,
        &pseudo_veins,
        current_tick,
        current_wall,
    );
    if restored != pseudo_veins.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "restored {restored} of {} validated pseudo-vein lifecycle rows",
                pseudo_veins.len()
            ),
        ));
    }
    Ok(restored)
}

pub(super) fn validate_pseudo_vein_snapshot_pair(
    pseudo_veins: &[HeartbeatPseudoVeinRecord],
    runtime_rows: &[ZoneRuntimeRecord],
) -> io::Result<()> {
    let heartbeat_by_id = pseudo_veins
        .iter()
        .map(|record| (record.zone_id.as_str(), record))
        .collect::<HashMap<_, _>>();
    let runtime_by_id = runtime_rows
        .iter()
        .filter(|record| is_heartbeat_pseudo_vein_zone_namespace(record.zone_id.as_str()))
        .map(|record| (record.zone_id.as_str(), record))
        .collect::<HashMap<_, _>>();

    for record in pseudo_veins {
        validate_persisted_pseudo_vein_record(record).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "invalid pseudo-vein lifecycle `{}`: {error}",
                    record.zone_id
                ),
            )
        })?;
        let runtime = runtime_by_id.get(record.zone_id.as_str()).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "pseudo-vein lifecycle `{}` has no matching zones_runtime row",
                    record.zone_id
                ),
            )
        })?;
        if !(0.0..=1.0).contains(&runtime.spirit_qi) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "pseudo-vein zone `{}` spirit_qi must be within [0, 1], actual {}",
                    runtime.zone_id, runtime.spirit_qi
                ),
            ));
        }
    }
    for runtime in runtime_by_id.values() {
        if !heartbeat_by_id.contains_key(runtime.zone_id.as_str()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "pseudo-vein zones_runtime row `{}` has no matching lifecycle row",
                    runtime.zone_id
                ),
            ));
        }
    }
    Ok(())
}

pub(super) fn hydrate_zone_overlays(
    settings: &PersistenceSettings,
    zones: &mut crate::world::zone::ZoneRegistry,
) -> io::Result<()> {
    let overlay_rows = load_zone_overlays(settings)?;
    zones
        .apply_overlay_records(&overlay_rows)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    Ok(())
}

/// plan-territory-v1 P0：持久化 ZoneInfluenceMap 快照到 SQLite。
/// 照 `persist_zone_runtime_snapshot` 范本。
#[cfg_attr(not(test), allow(dead_code))]
pub fn persist_zone_influence_snapshot(
    settings: &PersistenceSettings,
    influence_map: &crate::world::territory::ZoneInfluenceMap,
) -> io::Result<()> {
    let wall_clock = current_unix_seconds();
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    for (zone_id, entry) in &influence_map.zones {
        for (char_id, player_inf) in &entry.players {
            let is_dominant = entry
                .dominant
                .as_ref()
                .is_some_and(|d| d.char_id == *char_id);
            let (established_tick, public_known) = if let Some(dom) = &entry.dominant {
                if dom.char_id == *char_id {
                    (dom.established_tick, dom.public_known)
                } else {
                    (0u64, false)
                }
            } else {
                (0u64, false)
            };
            upsert_zone_influence(
                &transaction,
                &ZoneInfluenceRecord {
                    zone_id: zone_id.clone(),
                    char_id: char_id.clone(),
                    value: player_inf.value,
                    meditation_ticks: player_inf.source_breakdown.meditation_ticks,
                    combat_wins: player_inf.source_breakdown.combat_wins,
                    player_kills: player_inf.source_breakdown.player_kills,
                    gather_count: player_inf.source_breakdown.gather_count,
                    continuous_sessions: player_inf.source_breakdown.continuous_sessions,
                    last_activity_tick: player_inf.last_activity_tick,
                    dominant: is_dominant,
                    established_tick,
                    public_known,
                    schema_version: CURRENT_SCHEMA_VERSION,
                    last_updated_wall: wall_clock,
                },
            )?;
        }
    }
    transaction.commit().map_err(io::Error::other)
}

/// plan-territory-v1 P0：从 SQLite 读取所有 zone_influence 记录。
#[cfg_attr(not(test), allow(dead_code))]
pub fn load_zone_influence_snapshot(
    settings: &PersistenceSettings,
) -> io::Result<Vec<ZoneInfluenceRecord>> {
    let connection = open_persistence_connection(settings)?;
    load_zone_influence_snapshot_from_connection(&connection)
}

/// plan-territory-v1 P0：从已有 Connection 读取 zone_influence 记录。
#[cfg_attr(not(test), allow(dead_code))]
pub fn load_zone_influence_snapshot_from_connection(
    connection: &Connection,
) -> io::Result<Vec<ZoneInfluenceRecord>> {
    let mut statement = connection
        .prepare(
            "
            SELECT zone_id, char_id, value,
                   meditation_ticks, combat_wins, player_kills,
                   gather_count, continuous_sessions, last_activity_tick,
                   dominant, established_tick, public_known,
                   schema_version, last_updated_wall
            FROM zone_influence
            ORDER BY zone_id ASC, char_id ASC
            ",
        )
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, f64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, i64>(8)?,
                row.get::<_, i64>(9)?,
                row.get::<_, i64>(10)?,
                row.get::<_, i64>(11)?,
                row.get::<_, i64>(12)?,
                row.get::<_, i64>(13)?,
            ))
        })
        .map_err(io::Error::other)?;

    let mut records = Vec::new();
    for row in rows {
        let (
            zone_id,
            char_id,
            value,
            meditation_ticks,
            combat_wins,
            player_kills,
            gather_count,
            continuous_sessions,
            last_activity_tick,
            dominant,
            established_tick,
            public_known,
            schema_version,
            last_updated_wall,
        ) = row.map_err(io::Error::other)?;
        records.push(ZoneInfluenceRecord {
            zone_id,
            char_id,
            value,
            meditation_ticks: u64::try_from(meditation_ticks.max(0)).unwrap_or(u64::MAX),
            combat_wins: sql_to_u32(combat_wins)?,
            player_kills: sql_to_u32(player_kills)?,
            gather_count: sql_to_u32(gather_count)?,
            continuous_sessions: sql_to_u32(continuous_sessions)?,
            last_activity_tick: u64::try_from(last_activity_tick.max(0)).unwrap_or(u64::MAX),
            dominant: dominant != 0,
            established_tick: u64::try_from(established_tick.max(0)).unwrap_or(u64::MAX),
            public_known: public_known != 0,
            schema_version: i32::try_from(schema_version).unwrap_or(CURRENT_SCHEMA_VERSION),
            last_updated_wall,
        });
    }
    Ok(records)
}

/// plan-territory-v1 P0：从 SQLite 记录 hydrate 到 ZoneInfluenceMap Resource。
/// 返回 hydrate 成功的记录数。
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn hydrate_zone_influence(
    settings: &PersistenceSettings,
    influence_map: &mut crate::world::territory::ZoneInfluenceMap,
) -> io::Result<usize> {
    use crate::world::territory::{InfluenceSources, PlayerInfluence, ZoneDominance};
    let records = load_zone_influence_snapshot(settings)?;
    let count = records.len();
    for record in records {
        let entry = influence_map
            .zones
            .entry(record.zone_id.clone())
            .or_default();
        entry.players.insert(
            record.char_id.clone(),
            PlayerInfluence {
                value: record.value,
                last_activity_tick: record.last_activity_tick,
                source_breakdown: InfluenceSources {
                    meditation_ticks: record.meditation_ticks,
                    combat_wins: record.combat_wins,
                    player_kills: record.player_kills,
                    gather_count: record.gather_count,
                    continuous_sessions: record.continuous_sessions,
                },
            },
        );
        // 恢复霸主状态（dominant=true 的那行）
        if record.dominant {
            entry.dominant = Some(ZoneDominance {
                char_id: record.char_id.clone(),
                influence: record.value,
                established_tick: record.established_tick,
                public_known: record.public_known,
                realm_band: None, // persistence 存量无境界段，P3 新增字段
            });
        }
    }
    Ok(count)
}

pub(super) fn normalize_zone_overlay_payload(
    record: ZoneOverlayRecord,
    supported_payload_version: i32,
) -> io::Result<Option<ZoneOverlayRecord>> {
    if record.payload_version < 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "zone overlay payload_version {} must be >= 1",
                record.payload_version
            ),
        ));
    }
    if record.payload_version > supported_payload_version {
        tracing::warn!(
            "[bong][persistence] preserve future zone overlay `{}`/`{}` at {}: payload_version {} is newer than supported {}",
            record.zone_id,
            record.overlay_kind,
            record.since_wall,
            record.payload_version,
            supported_payload_version
        );
        return Ok(Some(record));
    }

    let mut migrated = record;
    while migrated.payload_version < supported_payload_version {
        migrated = match migrated.payload_version {
            1 => migrate_zone_overlay_payload_v1_to_v2(migrated)?,
            unsupported => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("no zone overlay payload migration from version {unsupported}"),
                ));
            }
        };
    }

    Ok(Some(migrated))
}

pub(super) fn migrate_zone_overlay_payload_v1_to_v2(
    mut record: ZoneOverlayRecord,
) -> io::Result<ZoneOverlayRecord> {
    let mut payload: serde_json::Value = serde_json::from_str(record.payload_json.as_str())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let Some(payload_object) = payload.as_object_mut() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "zone overlay v1 payload must be a JSON object",
        ));
    };
    payload_object
        .entry("payload_schema".to_string())
        .or_insert_with(|| serde_json::Value::String("zone_overlay_v2".to_string()));
    record.payload_json = serde_json::to_string(&payload)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    record.payload_version = 2;
    Ok(record)
}

pub(crate) fn upsert_zone_runtime(
    transaction: &rusqlite::Transaction<'_>,
    record: &ZoneRuntimeRecord,
    wall_clock: i64,
) -> io::Result<()> {
    validate_zone_runtime_record(record)?;
    transaction
        .execute(
            "
            INSERT INTO zones_runtime (
                zone_id,
                spirit_qi,
                danger_level,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(zone_id) DO UPDATE SET
                spirit_qi = excluded.spirit_qi,
                danger_level = excluded.danger_level,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                record.zone_id,
                record.spirit_qi,
                i64::from(record.danger_level),
                CURRENT_SCHEMA_VERSION,
                wall_clock,
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

pub(super) fn validate_zone_runtime_record(record: &ZoneRuntimeRecord) -> io::Result<()> {
    if record.zone_id.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "zone runtime id must not be empty",
        ));
    }
    if !record.spirit_qi.is_finite() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "zone runtime `{}` spirit_qi must be finite, actual {}",
                record.zone_id, record.spirit_qi
            ),
        ));
    }
    if is_heartbeat_pseudo_vein_zone_namespace(record.zone_id.as_str())
        && !is_heartbeat_pseudo_vein_zone_id(record.zone_id.as_str())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "pseudo-vein zone runtime id `{}` must end in a decimal u64 index",
                record.zone_id
            ),
        ));
    }
    if is_heartbeat_pseudo_vein_zone_id(record.zone_id.as_str())
        && !(0.0..=1.0).contains(&record.spirit_qi)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "pseudo-vein zone runtime `{}` spirit_qi must be within [0, 1], actual {}",
                record.zone_id, record.spirit_qi
            ),
        ));
    }
    Ok(())
}

pub(super) fn replace_heartbeat_pseudo_vein_records(
    transaction: &rusqlite::Transaction<'_>,
    records: &[HeartbeatPseudoVeinRecord],
    wall_clock: i64,
) -> io::Result<()> {
    transaction
        .execute("DELETE FROM heartbeat_pseudo_veins", [])
        .map_err(io::Error::other)?;
    for record in records {
        upsert_heartbeat_pseudo_vein(transaction, record, wall_clock)?;
    }
    Ok(())
}

pub(super) fn upsert_heartbeat_pseudo_vein(
    transaction: &rusqlite::Transaction<'_>,
    record: &HeartbeatPseudoVeinRecord,
    wall_clock: i64,
) -> io::Result<()> {
    validate_persisted_pseudo_vein_record(record).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "invalid pseudo-vein lifecycle `{}`: {error}",
                record.zone_id
            ),
        )
    })?;
    let active_events_json = serde_json::to_string(&record.active_events)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let patrol_anchors_json = serde_json::to_string(&record.patrol_anchors)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    transaction
        .execute(
            "
            INSERT INTO heartbeat_pseudo_veins (
                zone_id,
                dimension,
                min_x,
                min_y,
                min_z,
                max_x,
                max_y,
                max_z,
                danger_level,
                active_events_json,
                patrol_anchors_json,
                center_x,
                center_z,
                spawned_at_tick,
                last_tick,
                qi_current,
                total_qi_consumed,
                warning_sent,
                dissipated,
                season_at_spawn,
                observed_age_ticks,
                pending_runtime_ticks,
                pending_offline_ticks,
                occupant_count,
                eval_elapsed_ticks,
                schema_version,
                last_updated_wall
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11,
                ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22,
                ?23, ?24, ?25, ?26, ?27
            )
            ON CONFLICT(zone_id) DO UPDATE SET
                dimension = excluded.dimension,
                min_x = excluded.min_x,
                min_y = excluded.min_y,
                min_z = excluded.min_z,
                max_x = excluded.max_x,
                max_y = excluded.max_y,
                max_z = excluded.max_z,
                danger_level = excluded.danger_level,
                active_events_json = excluded.active_events_json,
                patrol_anchors_json = excluded.patrol_anchors_json,
                center_x = excluded.center_x,
                center_z = excluded.center_z,
                spawned_at_tick = excluded.spawned_at_tick,
                last_tick = excluded.last_tick,
                qi_current = excluded.qi_current,
                total_qi_consumed = excluded.total_qi_consumed,
                warning_sent = excluded.warning_sent,
                dissipated = excluded.dissipated,
                season_at_spawn = excluded.season_at_spawn,
                observed_age_ticks = excluded.observed_age_ticks,
                pending_runtime_ticks = excluded.pending_runtime_ticks,
                pending_offline_ticks = excluded.pending_offline_ticks,
                occupant_count = excluded.occupant_count,
                eval_elapsed_ticks = excluded.eval_elapsed_ticks,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                record.zone_id.as_str(),
                dimension_kind_to_sql(record.dimension),
                record.bounds_min[0],
                record.bounds_min[1],
                record.bounds_min[2],
                record.bounds_max[0],
                record.bounds_max[1],
                record.bounds_max[2],
                i64::from(record.danger_level),
                active_events_json,
                patrol_anchors_json,
                record.center_xz[0],
                record.center_xz[1],
                tick_to_sql(record.spawned_at_tick)?,
                tick_to_sql(record.last_tick)?,
                record.qi_current,
                record.total_qi_consumed,
                bool_to_sql(record.warning_sent),
                bool_to_sql(record.dissipated),
                pseudo_vein_season_to_sql(record.season_at_spawn),
                tick_to_sql(record.observed_age_ticks)?,
                tick_to_sql(record.pending_runtime_ticks)?,
                tick_to_sql(record.pending_offline_ticks)?,
                i64::try_from(record.occupant_count).unwrap_or(i64::MAX),
                tick_to_sql(record.eval_elapsed_ticks)?,
                CURRENT_SCHEMA_VERSION,
                wall_clock,
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

/// plan-territory-v1 P0：upsert zone_influence 行（照 upsert_zone_runtime 范本）。
#[cfg_attr(not(test), allow(dead_code))]

pub(super) fn upsert_zone_influence(
    transaction: &rusqlite::Transaction<'_>,
    record: &ZoneInfluenceRecord,
) -> io::Result<()> {
    transaction
        .execute(
            "
            INSERT INTO zone_influence (
                zone_id,
                char_id,
                value,
                meditation_ticks,
                combat_wins,
                player_kills,
                gather_count,
                continuous_sessions,
                last_activity_tick,
                dominant,
                established_tick,
                public_known,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
            ON CONFLICT(zone_id, char_id) DO UPDATE SET
                value                = excluded.value,
                meditation_ticks     = excluded.meditation_ticks,
                combat_wins          = excluded.combat_wins,
                player_kills         = excluded.player_kills,
                gather_count         = excluded.gather_count,
                continuous_sessions  = excluded.continuous_sessions,
                last_activity_tick   = excluded.last_activity_tick,
                dominant             = excluded.dominant,
                established_tick     = excluded.established_tick,
                public_known         = excluded.public_known,
                schema_version       = excluded.schema_version,
                last_updated_wall    = excluded.last_updated_wall
            ",
            params![
                record.zone_id,
                record.char_id,
                record.value,
                i64::try_from(record.meditation_ticks).unwrap_or(i64::MAX),
                i64::from(record.combat_wins),
                i64::from(record.player_kills),
                i64::from(record.gather_count),
                i64::from(record.continuous_sessions),
                i64::try_from(record.last_activity_tick).unwrap_or(i64::MAX),
                i64::from(record.dominant),
                i64::try_from(record.established_tick).unwrap_or(i64::MAX),
                i64::from(record.public_known),
                record.schema_version,
                record.last_updated_wall,
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn upsert_zone_overlay(
    transaction: &rusqlite::Transaction<'_>,
    record: &ZoneOverlayRecord,
    wall_clock: i64,
) -> io::Result<()> {
    let record = normalize_zone_overlay_payload(record.clone(), ZONE_OVERLAY_PAYLOAD_VERSION)?
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "zone overlay payload_version {} is newer than supported {}",
                    record.payload_version, ZONE_OVERLAY_PAYLOAD_VERSION
                ),
            )
        })?;
    transaction
        .execute(
            "
            INSERT INTO zone_overlays (
                zone_id,
                overlay_kind,
                payload_json,
                payload_version,
                since_wall,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(zone_id, overlay_kind, since_wall) DO UPDATE SET
                payload_json = excluded.payload_json,
                payload_version = excluded.payload_version,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                record.zone_id,
                record.overlay_kind,
                record.payload_json,
                record.payload_version,
                record.since_wall,
                CURRENT_SCHEMA_VERSION,
                wall_clock,
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

pub(super) fn load_zone_overlays_from_connection(
    connection: &Connection,
) -> io::Result<Vec<ZoneOverlayRecord>> {
    let mut statement = connection
        .prepare(
            "
            SELECT zone_id, overlay_kind, payload_json, payload_version, since_wall
            FROM zone_overlays
            ORDER BY zone_id ASC, overlay_kind ASC, since_wall ASC
            ",
        )
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map([], |row| {
            Ok(ZoneOverlayRecord {
                zone_id: row.get(0)?,
                overlay_kind: row.get(1)?,
                payload_json: row.get(2)?,
                payload_version: row.get(3)?,
                since_wall: row.get(4)?,
            })
        })
        .map_err(io::Error::other)?;

    let mut overlays = Vec::new();
    for row in rows {
        let record = row.map_err(io::Error::other)?;
        if let Some(record) = normalize_zone_overlay_payload(record, ZONE_OVERLAY_PAYLOAD_VERSION)?
        {
            overlays.push(record);
        }
    }
    Ok(overlays)
}

pub(super) fn load_zone_runtime_snapshot_from_connection(
    connection: &Connection,
) -> io::Result<Vec<ZoneRuntimeRecord>> {
    let mut statement = connection
        .prepare(
            "
            SELECT zone_id, spirit_qi, danger_level
            FROM zones_runtime
            ORDER BY zone_id ASC
            ",
        )
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, f64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .map_err(io::Error::other)?;

    let mut records = Vec::new();
    for row in rows {
        let (zone_id, spirit_qi, danger_level) = row.map_err(io::Error::other)?;
        let record = ZoneRuntimeRecord {
            zone_id,
            spirit_qi,
            danger_level: sql_to_u8(danger_level)?,
        };
        validate_zone_runtime_record(&record)?;
        records.push(record);
    }
    Ok(records)
}

pub(super) fn load_heartbeat_pseudo_veins_from_connection(
    connection: &Connection,
) -> io::Result<Vec<HeartbeatPseudoVeinRecord>> {
    let mut statement = connection
        .prepare(
            "
            SELECT zone_id, dimension,
                   min_x, min_y, min_z,
                   max_x, max_y, max_z,
                   danger_level,
                   active_events_json,
                   patrol_anchors_json,
                   center_x, center_z,
                   spawned_at_tick,
                   last_tick,
                   qi_current,
                   total_qi_consumed,
                   warning_sent,
                   dissipated,
                   season_at_spawn,
                   observed_age_ticks,
                   pending_runtime_ticks,
                   pending_offline_ticks,
                   occupant_count,
                   eval_elapsed_ticks,
                   last_updated_wall
            FROM heartbeat_pseudo_veins
            ORDER BY zone_id ASC
            ",
        )
        .map_err(io::Error::other)?;
    let mut rows = statement.query([]).map_err(io::Error::other)?;

    let mut records = Vec::new();
    while let Some(row) = rows.next().map_err(io::Error::other)? {
        let dimension: String = row.get(1).map_err(io::Error::other)?;
        let active_events_json: String = row.get(9).map_err(io::Error::other)?;
        let patrol_anchors_json: String = row.get(10).map_err(io::Error::other)?;
        let season_at_spawn: String = row.get(19).map_err(io::Error::other)?;
        records.push(HeartbeatPseudoVeinRecord {
            zone_id: row.get(0).map_err(io::Error::other)?,
            dimension: sql_to_dimension_kind(dimension.as_str())?,
            bounds_min: [
                row.get(2).map_err(io::Error::other)?,
                row.get(3).map_err(io::Error::other)?,
                row.get(4).map_err(io::Error::other)?,
            ],
            bounds_max: [
                row.get(5).map_err(io::Error::other)?,
                row.get(6).map_err(io::Error::other)?,
                row.get(7).map_err(io::Error::other)?,
            ],
            danger_level: sql_to_u8(row.get(8).map_err(io::Error::other)?)?,
            active_events: serde_json::from_str(&active_events_json)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
            patrol_anchors: serde_json::from_str(&patrol_anchors_json)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
            center_xz: [
                row.get(11).map_err(io::Error::other)?,
                row.get(12).map_err(io::Error::other)?,
            ],
            spawned_at_tick: sql_to_tick(row.get(13).map_err(io::Error::other)?)?,
            last_tick: sql_to_tick(row.get(14).map_err(io::Error::other)?)?,
            qi_current: row.get(15).map_err(io::Error::other)?,
            total_qi_consumed: row.get(16).map_err(io::Error::other)?,
            warning_sent: sql_to_bool(row.get(17).map_err(io::Error::other)?),
            dissipated: sql_to_bool(row.get(18).map_err(io::Error::other)?),
            season_at_spawn: sql_to_pseudo_vein_season(season_at_spawn.as_str())?,
            observed_age_ticks: sql_to_tick(row.get(20).map_err(io::Error::other)?)?,
            pending_runtime_ticks: sql_to_tick(row.get(21).map_err(io::Error::other)?)?,
            pending_offline_ticks: sql_to_tick(row.get(22).map_err(io::Error::other)?)?,
            occupant_count: usize::try_from(sql_to_tick(row.get(23).map_err(io::Error::other)?)?)
                .map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "occupant_count overflow")
            })?,
            eval_elapsed_ticks: sql_to_tick(row.get(24).map_err(io::Error::other)?)?,
            snapshot_wall: row.get(25).map_err(io::Error::other)?,
        });
    }
    Ok(records)
}
