//! Persistence bootstrap, lifecycle systems, SQLite setup, and backup orchestration.

use super::*;

#[derive(Debug, Default)]
pub(super) struct DailyBackupState {
    pub(super) last_backup_day: Option<i64>,
}

impl Resource for DailyBackupState {}

#[derive(Debug, Default)]
pub(super) struct PersistenceShutdownReader(ManualEventReader<AppExit>);

impl Resource for PersistenceShutdownReader {}

#[allow(clippy::too_many_arguments)]
pub(super) fn bootstrap_persistence_system(
    settings: valence::prelude::Res<PersistenceSettings>,
    mut daily_backup_state: valence::prelude::ResMut<DailyBackupState>,
    mut zones: Option<ResMut<crate::world::zone::ZoneRegistry>>,
    mut heartbeat: Option<ResMut<WorldHeartbeat>>,
    clock: Res<CultivationClock>,
    mut qi_ledger: ResMut<WorldQiAccount>,
    mut void_action_cooldowns: Option<ResMut<VoidActionCooldowns>>,
    mut zone_influence_map: Option<ResMut<crate::world::territory::ZoneInfluenceMap>>,
) {
    let wall_clock = current_unix_seconds();
    daily_backup_state.last_backup_day = Some(utc_day_from_unix_seconds(wall_clock));
    match run_startup_backup(&settings, wall_clock) {
        Ok(Some(path)) => tracing::info!(
            "[bong][persistence] created startup sqlite backup at {}",
            path.display()
        ),
        Ok(None) => {}
        Err(error) => tracing::warn!(
            "[bong][persistence] failed to create startup sqlite backup at {}: {error}",
            settings.db_path().display()
        ),
    }

    match prune_startup_backups(&settings, STARTUP_BACKUP_KEEP_COUNT) {
        Ok(pruned) if !pruned.is_empty() => tracing::info!(
            "[bong][persistence] pruned {} stale startup backup(s) under {}",
            pruned.len(),
            resolve_persistence_relative_path(&settings, STARTUP_BACKUP_DIR).display()
        ),
        Ok(_) => {}
        Err(error) => tracing::warn!(
            "[bong][persistence] failed to prune startup backups under {}: {error}",
            resolve_persistence_relative_path(&settings, STARTUP_BACKUP_DIR).display()
        ),
    }

    if let Err(error) = bootstrap_sqlite(settings.db_path(), settings.server_run_id()) {
        panic!(
            "[bong][persistence] failed to bootstrap sqlite at {}: {error}",
            settings.db_path().display()
        );
    }

    hydrate_runtime_qi_accounts(&settings, &mut qi_ledger).unwrap_or_else(|error| {
        panic!(
            "[bong][persistence] cannot safely hydrate runtime qi accounts at {}: {error}",
            settings.db_path().display()
        )
    });

    if let Err(error) = scan_orphaned_npc_archives(&settings) {
        tracing::warn!(
            "[bong][persistence] failed to scan orphaned npc archives at {}: {error}",
            settings.db_path().display()
        );
    }

    if let Some(cooldowns) = void_action_cooldowns.as_deref_mut() {
        match hydrate_void_action_cooldowns(&settings, cooldowns) {
            Ok(count) if count > 0 => tracing::info!(
                "[bong][persistence] hydrated {count} void-action cooldown(s) from sqlite"
            ),
            Ok(_) => {}
            Err(error) => panic!(
                "[bong][persistence] failed to hydrate void-action cooldowns at {}: {error}",
                settings.db_path().display()
            ),
        }
    }

    if let Some(zone_registry) = zones.as_deref_mut() {
        if let Some(heartbeat) = heartbeat.as_deref_mut() {
            match hydrate_heartbeat_pseudo_veins(
                &settings,
                heartbeat,
                zone_registry,
                clock.tick,
                wall_clock,
            ) {
                Ok(count) if count > 0 => {
                    tracing::info!(
                        "[bong][persistence] hydrated {count} heartbeat pseudo-vein runtime record(s) from sqlite"
                    );
                }
                Ok(_) => {}
                Err(error) => panic!(
                    "[bong][persistence] refusing startup after heartbeat pseudo-vein hydrate failure at {}: {error}",
                    settings.db_path().display()
                ),
            }
        }
        if let Err(error) = hydrate_zone_runtime(&settings, zone_registry) {
            panic!(
                "[bong][persistence] refusing startup after zone runtime hydrate failure at {}: {error}",
                settings.db_path().display()
            );
        }
        if let Some(heartbeat) = heartbeat.as_deref_mut() {
            heartbeat.sync_active_pseudo_vein_qi_from_zones(zone_registry);
        }
        // Zone balances are restored only into Zone.spirit_qi. Dynamic pseudo-veins use the same
        // external owner and settle through typed Zone↔stable-pool transactions; recreating a
        // `zone:*` ledger balance here would double-count every restored pseudo-vein.
        if let Err(error) = hydrate_zone_overlays(&settings, zone_registry) {
            tracing::warn!(
                "[bong][persistence] failed to hydrate zone overlays from sqlite at {}: {error}",
                settings.db_path().display()
            );
        }
    }

    // plan-territory-v1 P0：hydrate 区域影响力（照 zones_runtime hydrate 模式）
    if let Some(influence_map) = zone_influence_map.as_deref_mut() {
        match hydrate_zone_influence(&settings, influence_map) {
            Ok(count) if count > 0 => tracing::info!(
                "[bong][persistence] hydrated {count} zone-influence record(s) from sqlite"
            ),
            Ok(_) => {}
            Err(error) => tracing::warn!(
                "[bong][persistence] failed to hydrate zone influence from sqlite at {}: {error}",
                settings.db_path().display()
            ),
        }
    }
}

pub(super) fn daily_midnight_backup_system(
    settings: Res<PersistenceSettings>,
    mut daily_backup_state: ResMut<DailyBackupState>,
) {
    let wall_clock = current_unix_seconds();
    match run_daily_backup_cycle(&settings, &mut daily_backup_state, wall_clock) {
        Ok(run) if !run.triggered => {}
        Ok(run) => {
            if let Some(path) = run.backup_path {
                tracing::info!(
                    "[bong][persistence] created daily sqlite backup at {}",
                    path.display()
                );
            }
            if !run.pruned_paths.is_empty() {
                tracing::info!(
                    "[bong][persistence] pruned {} stale daily backup(s) under {}",
                    run.pruned_paths.len(),
                    resolve_persistence_relative_path(&settings, STARTUP_BACKUP_DIR).display()
                );
            }
        }
        Err(error) => tracing::warn!(
            "[bong][persistence] daily backup cycle failed at {}: {error}",
            settings.db_path().display()
        ),
    }
}

pub(super) fn dispatch_persistence_shutdown_flushes(world: &mut World) {
    let requested = world.resource_scope(
        |world, mut reader: valence::prelude::Mut<PersistenceShutdownReader>| {
            world
                .get_resource::<Events<AppExit>>()
                .is_some_and(|events| reader.0.read(events).next().is_some())
        },
    );
    let request = if requested {
        ShutdownFlushRequest::Requested
    } else {
        ShutdownFlushRequest::NotRequested
    };
    let runtime_tick = world
        .get_resource::<CultivationClock>()
        .map_or(0, |clock| clock.tick);
    let wall_unix_millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64);
    let clock = ProductionSliceClock {
        runtime_tick,
        wall_unix_millis,
    };

    match slice::dispatch_shutdown_flushes(world, request, &clock) {
        Ok(report) => {
            for failure in report.failures {
                tracing::warn!(
                    "[bong][persistence] shutdown slice `{}` failed: {}",
                    failure.slice_id,
                    failure.error
                );
            }
        }
        Err(error) => {
            tracing::error!("[bong][persistence] shutdown slice dispatch failed closed: {error}")
        }
    }
}

pub fn bootstrap_sqlite(db_path: &Path, server_run_id: &str) -> rusqlite::Result<()> {
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    }

    let mut connection = Connection::open(db_path)?;
    configure_connection(&connection)?;
    run_integrity_check(&connection)?;
    apply_migrations(&mut connection)?;
    record_bootstrap_event(&connection, server_run_id)?;
    Ok(())
}

pub(super) fn configure_connection(connection: &Connection) -> rusqlite::Result<()> {
    let journal_mode: String =
        connection.query_row("PRAGMA journal_mode = WAL;", [], |row| row.get(0))?;
    if !journal_mode.eq_ignore_ascii_case("wal") {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            io::Error::other(format!(
                "sqlite journal_mode must be WAL, got `{journal_mode}`"
            )),
        )));
    }

    connection.execute_batch("PRAGMA foreign_keys = ON;")?;
    connection.busy_timeout(Duration::from_millis(SQLITE_BUSY_TIMEOUT_MS))?;
    Ok(())
}

pub(super) fn run_integrity_check(connection: &Connection) -> rusqlite::Result<()> {
    let integrity: String =
        connection.query_row("PRAGMA integrity_check;", [], |row| row.get(0))?;
    if integrity != "ok" {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            io::Error::other(format!("sqlite integrity_check returned `{integrity}`")),
        )));
    }
    Ok(())
}

pub(super) fn record_bootstrap_event(
    connection: &Connection,
    server_run_id: &str,
) -> rusqlite::Result<()> {
    let event_id = Uuid::now_v7().to_string();
    let wall_clock = current_unix_seconds();
    let payload = BootstrapPayload {
        id: event_id.clone(),
        schema_version: CURRENT_SCHEMA_VERSION,
        note: "sqlite bootstrap ready".to_string(),
    };
    let payload_json = serde_json::to_string(&payload)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;

    connection.execute(
        "
        INSERT OR IGNORE INTO bootstrap_events (
            event_id,
            kind,
            schema_version,
            game_tick,
            wall_clock,
            server_run_id,
            last_updated_wall,
            payload_json
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        ",
        params![
            event_id,
            "bootstrap_ready",
            CURRENT_SCHEMA_VERSION,
            0_i64,
            wall_clock,
            server_run_id,
            wall_clock,
            payload_json
        ],
    )?;

    Ok(())
}

#[derive(Debug, Default)]
pub(super) struct DailyBackupRun {
    pub(super) triggered: bool,
    pub(super) backup_path: Option<PathBuf>,
    pub(super) pruned_paths: Vec<PathBuf>,
}

pub(super) fn run_daily_backup_cycle(
    settings: &PersistenceSettings,
    state: &mut DailyBackupState,
    wall_clock: i64,
) -> io::Result<DailyBackupRun> {
    let current_day = utc_day_from_unix_seconds(wall_clock);
    if state
        .last_backup_day
        .is_some_and(|last_backup_day| current_day <= last_backup_day)
    {
        return Ok(DailyBackupRun::default());
    }

    state.last_backup_day = Some(current_day);
    let backup_path = run_startup_backup(settings, wall_clock)?;
    let pruned_paths = prune_startup_backups(settings, STARTUP_BACKUP_KEEP_COUNT)?;
    Ok(DailyBackupRun {
        triggered: true,
        backup_path,
        pruned_paths,
    })
}

pub(super) fn run_startup_backup(
    settings: &PersistenceSettings,
    wall_clock: i64,
) -> io::Result<Option<PathBuf>> {
    if !settings.db_path().exists() {
        return Ok(None);
    }

    let backup_path = startup_backup_path(settings, wall_clock);
    snapshot_existing_sqlite(settings.db_path(), &backup_path)?;
    Ok(Some(backup_path))
}

pub(super) fn startup_backup_path(settings: &PersistenceSettings, wall_clock: i64) -> PathBuf {
    resolve_persistence_relative_path(settings, STARTUP_BACKUP_DIR).join(format!(
        "{STARTUP_BACKUP_FILE_PREFIX}{}{STARTUP_BACKUP_FILE_SUFFIX}",
        format_startup_backup_stamp(wall_clock),
    ))
}

pub(super) fn format_startup_backup_stamp(unix_seconds: i64) -> String {
    let days = unix_seconds.div_euclid(86_400);
    let seconds_of_day = unix_seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;

    format!("{year:04}{month:02}{day:02}-{hour:02}{minute:02}{second:02}",)
}

pub(super) fn snapshot_existing_sqlite(db_path: &Path, backup_path: &Path) -> io::Result<()> {
    if let Some(parent) = backup_path.parent() {
        fs::create_dir_all(parent)?;
    }
    if backup_path.exists() {
        fs::remove_file(backup_path)?;
    }

    let connection = Connection::open(db_path).map_err(io::Error::other)?;
    configure_connection(&connection).map_err(io::Error::other)?;
    let escaped_path = backup_path.to_string_lossy().replace('\'', "''");
    let sql = format!("VACUUM main INTO '{escaped_path}';");
    connection.execute_batch(&sql).map_err(io::Error::other)
}

pub(super) fn prune_startup_backups(
    settings: &PersistenceSettings,
    keep: usize,
) -> io::Result<Vec<PathBuf>> {
    let backup_root = resolve_persistence_relative_path(settings, STARTUP_BACKUP_DIR);
    let mut backup_files = collect_files_with_suffix(&backup_root, STARTUP_BACKUP_FILE_SUFFIX)?;
    backup_files.retain(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| {
                name.starts_with(STARTUP_BACKUP_FILE_PREFIX)
                    && name.ends_with(STARTUP_BACKUP_FILE_SUFFIX)
            })
    });
    backup_files.sort_by(|left, right| {
        left.file_name()
            .cmp(&right.file_name())
            .then_with(|| left.cmp(right))
    });

    if backup_files.len() <= keep {
        return Ok(Vec::new());
    }

    let stale_count = backup_files.len() - keep;
    let stale_files = backup_files
        .into_iter()
        .take(stale_count)
        .collect::<Vec<_>>();
    for path in &stale_files {
        fs::remove_file(path)?;
    }

    Ok(stale_files)
}
