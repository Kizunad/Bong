//! Shared persistence conversion, connection, clock, and archive helpers.

use super::*;

pub(super) fn current_unix_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_secs() as i64
}

pub(super) fn utc_day_from_unix_seconds(unix_seconds: i64) -> i64 {
    unix_seconds.div_euclid(86_400)
}

pub(crate) fn open_persistence_connection(
    settings: &PersistenceSettings,
) -> io::Result<Connection> {
    if let Some(parent) = settings.db_path().parent() {
        fs::create_dir_all(parent)?;
    }

    let connection = Connection::open(settings.db_path()).map_err(io::Error::other)?;
    configure_connection(&connection).map_err(io::Error::other)?;
    Ok(connection)
}

pub(super) fn tick_to_sql(tick: u64) -> io::Result<i64> {
    i64::try_from(tick).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

pub(super) fn read_optional_file(path: &Path) -> io::Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(contents) => Ok(Some(contents)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

pub(super) fn rollback_file(path: &Path, previous: Option<&[u8]>) -> io::Result<()> {
    match previous {
        Some(contents) => fs::write(path, contents),
        None => match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        },
    }
}

pub(super) fn default_termination_category() -> String {
    "横死".to_string()
}

pub(super) fn parse_enum_label<T>(label: &str) -> io::Result<T>
where
    T: DeserializeOwned,
{
    serde_json::from_value(serde_json::Value::String(label.to_string()))
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

pub(super) fn termination_category_from_entry(entry: &BiographyEntry) -> String {
    let BiographyEntry::Terminated { cause, .. } = entry else {
        return default_termination_category();
    };
    match cause.as_str() {
        "natural_end" => "善终",
        "voluntary_retire" => "自主归隐",
        "duo_she" => "夺舍者",
        _ => "横死",
    }
    .to_string()
}

pub(super) fn build_npc_blackboard_snapshot(
    blackboard: &NpcBlackboard,
    nearest_player_id: Option<&str>,
) -> HashMap<String, serde_json::Value> {
    let mut snapshot = HashMap::new();
    if let Some(player_id) = nearest_player_id {
        snapshot.insert(
            "nearest_player".to_string(),
            serde_json::Value::String(player_id.to_string()),
        );
    }
    if blackboard.player_distance.is_finite() {
        snapshot.insert(
            "player_distance".to_string(),
            serde_json::Value::from(f64::from(blackboard.player_distance)),
        );
    }
    if let Some(target_position) = blackboard.target_position {
        snapshot.insert(
            "target_position".to_string(),
            serde_json::json!(vec3_to_array(target_position)),
        );
    }
    snapshot.insert(
        "last_melee_tick".to_string(),
        serde_json::Value::from(blackboard.last_melee_tick),
    );
    snapshot
}

pub(super) fn vec3_to_array(position: DVec3) -> [f64; 3] {
    [position.x, position.y, position.z]
}

pub(super) fn state_label(state: &NpcStateKind) -> &'static str {
    match state {
        NpcStateKind::Idle => "idle",
        NpcStateKind::Fleeing => "fleeing",
        NpcStateKind::Attacking => "attacking",
        NpcStateKind::Patrolling => "patrolling",
    }
}

pub(super) fn lifecycle_state_label(state: &LifecycleState) -> &'static str {
    match state {
        LifecycleState::Alive => "alive",
        LifecycleState::NearDeath => "near_death",
        LifecycleState::AwaitingRevival => "awaiting_revival",
        LifecycleState::Terminated => "terminated",
    }
}

pub(super) fn movement_mode_label(mode: &MovementMode) -> &'static str {
    match mode {
        MovementMode::GroundNav => "ground_nav",
        MovementMode::Sprinting(_) => "sprinting",
        MovementMode::Override(crate::npc::movement::ActiveOverride::Dash(_)) => "override_dash",
        MovementMode::Override(crate::npc::movement::ActiveOverride::Knockback(_)) => {
            "override_knockback"
        }
    }
}

pub(super) fn npc_archetype_label(archetype: NpcMeleeArchetype) -> &'static str {
    match archetype {
        NpcMeleeArchetype::Brawler => "brawler",
        NpcMeleeArchetype::Sword => "sword",
        NpcMeleeArchetype::Spear => "spear",
    }
}

pub(super) fn entity_kind_label(kind: EntityKind) -> String {
    let debug = format!("{kind:?}");
    if let Some((_, label)) = debug.split_once(' ') {
        label
            .trim_start_matches('(')
            .trim_end_matches(')')
            .to_string()
    } else {
        debug
    }
}

pub(super) fn bool_to_sql(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn sql_to_bool(value: i64) -> bool {
    value != 0
}

pub(super) fn dimension_kind_to_sql(dimension: DimensionKind) -> &'static str {
    match dimension {
        DimensionKind::Overworld => "overworld",
        DimensionKind::Tsy => "tsy",
    }
}

pub(super) fn sql_to_dimension_kind(value: &str) -> io::Result<DimensionKind> {
    match value {
        "overworld" => Ok(DimensionKind::Overworld),
        "tsy" => Ok(DimensionKind::Tsy),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unknown dimension kind `{other}`"),
        )),
    }
}

pub(super) fn pseudo_vein_season_to_sql(season: PseudoVeinSeasonV1) -> &'static str {
    match season {
        PseudoVeinSeasonV1::Summer => "summer",
        PseudoVeinSeasonV1::SummerToWinter => "summer_to_winter",
        PseudoVeinSeasonV1::Winter => "winter",
        PseudoVeinSeasonV1::WinterToSummer => "winter_to_summer",
    }
}

pub(super) fn sql_to_pseudo_vein_season(value: &str) -> io::Result<PseudoVeinSeasonV1> {
    match value {
        "summer" => Ok(PseudoVeinSeasonV1::Summer),
        "summer_to_winter" => Ok(PseudoVeinSeasonV1::SummerToWinter),
        "winter" => Ok(PseudoVeinSeasonV1::Winter),
        "winter_to_summer" => Ok(PseudoVeinSeasonV1::WinterToSummer),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unknown pseudo-vein season `{other}`"),
        )),
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn sql_to_tick(value: i64) -> io::Result<u64> {
    u64::try_from(value).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

pub(super) fn optional_tick_to_sql(tick: Option<u64>) -> io::Result<Option<i64>> {
    tick.map(tick_to_sql).transpose()
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn optional_sql_to_tick(value: Option<i64>) -> io::Result<Option<u64>> {
    value.map(sql_to_tick).transpose()
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn sql_to_u32(value: i64) -> io::Result<u32> {
    u32::try_from(value).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

pub(super) fn sql_to_u8(value: i64) -> io::Result<u8> {
    u8::try_from(value).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn sql_to_usize(value: i64) -> io::Result<usize> {
    usize::try_from(value).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

pub(super) fn sql_usize(value: usize) -> io::Result<i64> {
    i64::try_from(value).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

pub(super) fn npc_deceased_archive_relative_path(char_id: &str, archived_at_wall: i64) -> String {
    format!(
        "data/archive/npc_deceased/{}/{}.json.zst",
        utc_year_from_unix_seconds(archived_at_wall),
        char_id
    )
}

pub(super) fn npc_deceased_archive_absolute_path(
    settings: &PersistenceSettings,
    char_id: &str,
    archived_at_wall: i64,
) -> PathBuf {
    resolve_persistence_relative_path(
        settings,
        npc_deceased_archive_relative_path(char_id, archived_at_wall).as_str(),
    )
}

pub(super) fn npc_digest_archive_relative_path(char_id: &str) -> String {
    format!("data/archive/npc_digests/{char_id}.json.zst")
}

pub(super) fn npc_digest_archive_absolute_path(
    settings: &PersistenceSettings,
    char_id: &str,
    _archived_at_wall: i64,
) -> PathBuf {
    resolve_persistence_relative_path(settings, npc_digest_archive_relative_path(char_id).as_str())
}

pub(super) fn resolve_persistence_relative_path(
    settings: &PersistenceSettings,
    relative_path: &str,
) -> PathBuf {
    let path = PathBuf::from(relative_path);
    if path.is_absolute() {
        return path;
    }

    let Some(data_dir) = settings.db_path().parent() else {
        return path;
    };
    if data_dir.file_name().is_some_and(|name| name == "data") {
        if let Some(root) = data_dir.parent() {
            return root.join(relative_path);
        }
        return path;
    }

    data_dir.join(relative_path)
}

pub(super) fn write_zstd_bundle(path: &Path, payload: &[u8]) -> io::Result<()> {
    write_zstd_bundle_with_writer(path, payload, |file, compressed| file.write_all(compressed))
}

pub(super) fn write_zstd_bundle_with_writer(
    path: &Path,
    payload: &[u8],
    write_temp: impl FnOnce(&mut fs::File, &[u8]) -> io::Result<()>,
) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let compressed = zstd::stream::encode_all(payload, 3).map_err(io::Error::other)?;
    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
    let filename = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "bundle".to_string());
    let temp_path = path.with_file_name(format!(
        ".{filename}.tmp-{}-{}",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let mut temp_file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp_path)?;
    let result = (|| {
        write_temp(&mut temp_file, &compressed)?;
        temp_file.sync_all()?;
        drop(temp_file);
        fs::rename(&temp_path, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn read_zstd_bundle(reference: &Path, relative_path: &str) -> io::Result<Vec<u8>> {
    let absolute_path = if Path::new(relative_path).is_absolute() {
        PathBuf::from(relative_path)
    } else {
        let settings = PersistenceSettings {
            db_path: reference.to_path_buf(),
            server_run_id: String::new(),
        };
        resolve_persistence_relative_path(&settings, relative_path)
    };
    let compressed = fs::read(absolute_path)?;
    zstd::stream::decode_all(compressed.as_slice())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

pub(super) fn utc_year_from_unix_seconds(unix_seconds: i64) -> i32 {
    let days = unix_seconds.div_euclid(86_400);
    civil_from_days(days).0
}

pub(super) fn civil_from_days(days_since_unix_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_unix_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if m <= 2 { 1 } else { 0 };

    (year as i32, m as u32, d as u32)
}

pub(super) fn find_orphaned_npc_archive_paths(
    settings: &PersistenceSettings,
) -> io::Result<Vec<PathBuf>> {
    let connection = open_persistence_connection(settings)?;
    let mut statement = connection
        .prepare("SELECT path FROM npc_deceased_index")
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(io::Error::other)?;
    let mut indexed_paths = HashSet::new();
    for row in rows {
        indexed_paths.insert(row.map_err(io::Error::other)?);
    }

    let archive_root = resolve_persistence_relative_path(settings, "data/archive/npc_deceased");
    let mut archive_files = collect_files_with_suffix(&archive_root, ".json.zst")?;
    archive_files.sort();
    let mut orphaned = Vec::new();
    for archive_file in archive_files {
        let Ok(relative_path) = archive_file.strip_prefix(
            archive_root
                .parent()
                .and_then(|parent| parent.parent())
                .unwrap_or(archive_root.as_path()),
        ) else {
            continue;
        };
        let normalized = relative_path.to_string_lossy().replace('\\', "/");
        let normalized = if normalized.starts_with("data/") {
            normalized
        } else {
            format!("data/{normalized}")
        };
        if !indexed_paths.contains(&normalized) {
            orphaned.push(archive_file);
        }
    }

    Ok(orphaned)
}

pub(super) fn scan_orphaned_npc_archives(settings: &PersistenceSettings) -> io::Result<()> {
    for archive_file in find_orphaned_npc_archive_paths(settings)? {
        tracing::warn!(
            "[bong][persistence] orphaned npc archive without sqlite index: {}",
            archive_file.display()
        );
    }

    Ok(())
}

pub(super) fn collect_files_with_suffix(root: &Path, suffix: &str) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    if !root.exists() {
        return Ok(files);
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            files.extend(collect_files_with_suffix(&path, suffix)?);
        } else if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(suffix))
        {
            files.push(path);
        }
    }
    Ok(files)
}
