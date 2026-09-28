//! 跨重启共享运行时 tick 的持久化。
//!
//! `Freshness.created_at_tick` 是库存 JSON 中的绝对 tick。旧实现只在内存里递增
//! `GameplayTick`、`CombatClock` 和 `ShelflifeSweepTick`，重启后新值从零开始，导致
//! `now_tick.saturating_sub(created_at_tick)` 把已有物品的年龄截断。这里保存一个共享
//! tick 与墙钟快照；启动时把停机期间经过的墙钟秒数换算成 tick，再灌回三个生产时钟。

use serde_json::Value;

use super::*;

const RUNTIME_CLOCK_ROW_ID: i64 = 1;
const TICKS_PER_SECOND: u64 = 20;

// 这里只列出能够证明是“过去某时刻”的运行时绝对 tick。未来截止时间（例如
// `ready_at_tick`、`invite_block_until_tick`）和时长累计值不能用来重建时钟。
const PERSISTED_RUNTIME_TICK_COLUMNS: &[(&str, &str)] = &[
    ("bootstrap_events", "game_tick"),
    ("life_events", "game_tick"),
    ("death_registry", "last_death_tick"),
    ("lifespan_events", "game_tick"),
    ("deceased_snapshots", "died_at_tick"),
    ("npc_state", "last_death_tick"),
    ("npc_state", "last_revive_tick"),
    ("membership", "joined_at_tick"),
    ("relationships", "since_tick"),
    ("archetype_registry", "since_tick"),
    ("npc_deceased_index", "died_at_tick"),
    ("tribulations_active", "started_tick"),
    ("agent_eras", "since_tick"),
    ("agent_eras", "observed_at_tick"),
    ("agent_decisions", "observed_at_tick"),
    ("player_lifespan", "born_at_tick"),
    ("player_identities", "last_switch_tick"),
    ("social_relationships", "since_tick"),
    ("social_exposures", "at_tick"),
    ("social_spirit_niches", "placed_at_tick"),
    ("high_renown_milestones", "emitted_at_tick"),
    ("spirit_treasure_world", "spawned_at_tick"),
    ("spirit_treasure_dialogue_log", "tick"),
    ("pending_dormant_relics", "created_tick"),
    ("zone_influence", "last_activity_tick"),
    ("zone_influence", "established_tick"),
    ("epitaphs", "death_tick"),
    ("heartbeat_pseudo_veins", "spawned_at_tick"),
    ("heartbeat_pseudo_veins", "last_tick"),
    ("player_lifecycle", "combat_clock_tick_at_save"),
    ("dormant_terminal_commits", "at_tick"),
    ("runtime_clock", "tick"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeClockRecord {
    pub tick: u64,
    pub snapshot_wall: i64,
}

#[derive(Debug, Default)]
pub(super) struct RuntimeClockSnapshotState {
    pub(super) last_snapshot_wall: i64,
}

impl Resource for RuntimeClockSnapshotState {}

pub(super) fn load_runtime_clock(settings: &PersistenceSettings) -> io::Result<u64> {
    load_runtime_clock_at(settings, current_unix_seconds())
}

fn load_runtime_clock_at(settings: &PersistenceSettings, now_wall: i64) -> io::Result<u64> {
    let connection = open_persistence_connection(settings)?;
    let persisted = connection
        .query_row(
            "SELECT tick, snapshot_wall FROM runtime_clock WHERE clock_id = ?1",
            params![RUNTIME_CLOCK_ROW_ID],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
        .map_err(io::Error::other)?;

    let Some((tick, snapshot_wall)) = persisted else {
        return legacy_inventory_tick(&connection, now_wall);
    };
    let record = RuntimeClockRecord {
        tick: sql_to_tick(tick)?,
        snapshot_wall,
    };

    Ok(record
        .tick
        .saturating_add(elapsed_wall_ticks(record.snapshot_wall, now_wall)))
}

pub(super) fn persist_runtime_clock(
    settings: &PersistenceSettings,
    tick: u64,
    snapshot_wall: i64,
) -> io::Result<()> {
    let connection = open_persistence_connection(settings)?;
    connection
        .execute(
            "
            INSERT INTO runtime_clock (clock_id, tick, snapshot_wall, schema_version)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(clock_id) DO UPDATE SET
                tick = excluded.tick,
                snapshot_wall = excluded.snapshot_wall,
                schema_version = excluded.schema_version
            ",
            params![
                RUNTIME_CLOCK_ROW_ID,
                tick_to_sql(tick)?,
                snapshot_wall,
                CURRENT_SCHEMA_VERSION,
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

pub(super) fn persist_runtime_clock_system(
    settings: Res<PersistenceSettings>,
    clock: Res<crate::cultivation::tick::CultivationClock>,
    mut state: ResMut<RuntimeClockSnapshotState>,
) {
    let now_wall = current_unix_seconds();
    if state.last_snapshot_wall > 0 && now_wall.saturating_sub(state.last_snapshot_wall) < 5 {
        return;
    }

    match persist_runtime_clock(&settings, clock.tick, now_wall) {
        Ok(()) => state.last_snapshot_wall = now_wall,
        Err(error) => tracing::warn!(
            "[bong][persistence] failed to persist runtime clock at {}: {error}",
            settings.db_path().display()
        ),
    }
}

fn elapsed_wall_ticks(snapshot_wall: i64, now_wall: i64) -> u64 {
    if snapshot_wall <= 0 {
        return 0;
    }
    let elapsed_seconds: u64 = now_wall
        .saturating_sub(snapshot_wall)
        .try_into()
        .unwrap_or(0);
    elapsed_seconds.saturating_mul(TICKS_PER_SECOND)
}

/// v44 及更早数据库没有共享 tick 行，也没有旧 tick 与 Unix 墙钟的对应关系。首次
/// 迁移只能使用数据库里仍可证明的运行时 tick：所有持久化 tick 的最大值作为旧进程
/// 的运行时下界，再加上从库存快照墙钟得到的停机间隔。墙钟只参与计算间隔，绝不能
/// 直接换算成运行时 epoch，否则 Unix 秒数会把相对 tick 放大到数十亿。
fn legacy_inventory_tick(connection: &Connection, now_wall: i64) -> io::Result<u64> {
    let mut rebased_tick = max_persisted_runtime_tick(connection)?;
    let mut statement = connection
        .prepare("SELECT inventory_json, last_updated_wall FROM inventories")
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(io::Error::other)?;

    for row in rows {
        let (inventory_json, snapshot_wall) = row.map_err(io::Error::other)?;
        let value = match serde_json::from_str::<Value>(&inventory_json) {
            Ok(value) => value,
            Err(error) => {
                tracing::warn!(
                    "[bong][persistence] ignored malformed legacy inventory while rebasing runtime clock: {error}"
                );
                continue;
            }
        };
        let Some(max_inventory_tick) = max_created_at_tick_in_json(&value) else {
            continue;
        };
        let candidate =
            max_inventory_tick.saturating_add(elapsed_wall_ticks(snapshot_wall, now_wall));
        rebased_tick = Some(rebased_tick.map_or(candidate, |current| current.max(candidate)));
    }

    Ok(rebased_tick.unwrap_or(0))
}

fn max_persisted_runtime_tick(connection: &Connection) -> io::Result<Option<u64>> {
    let mut maximum = None;
    for &(table, column) in PERSISTED_RUNTIME_TICK_COLUMNS {
        let quoted_table = quote_sql_identifier(&table);
        let quoted_column = quote_sql_identifier(column);
        let query =
            format!("SELECT MAX({quoted_column}) FROM {quoted_table} WHERE {quoted_column} >= 0");
        let value: Option<i64> = connection
            .query_row(&query, [], |row| row.get(0))
            .map_err(io::Error::other)?;
        let Some(value) = value else {
            continue;
        };
        let tick = sql_to_tick(value)?;
        maximum = Some(maximum.map_or(tick, |current: u64| current.max(tick)));
    }

    Ok(maximum)
}

fn quote_sql_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('\"', "\"\""))
}

fn max_created_at_tick_in_json(value: &Value) -> Option<u64> {
    match value {
        Value::Object(fields) => fields.iter().fold(None, |current, (key, child)| {
            let own = (key == "created_at_tick").then(|| child.as_u64()).flatten();
            [current, own, max_created_at_tick_in_json(child)]
                .into_iter()
                .flatten()
                .max()
        }),
        Value::Array(values) => values.iter().filter_map(max_created_at_tick_in_json).max(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shelflife::{
        compute::compute_track_state, DecayFormula, DecayProfile, DecayProfileId, Freshness,
        TrackState,
    };
    use std::path::PathBuf;

    fn settings(test_name: &str) -> (PersistenceSettings, PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "bong-runtime-clock-{test_name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let db_path = root.join("bong.db");
        (PersistenceSettings::with_db_path(&db_path, test_name), root)
    }

    #[test]
    fn persisted_tick_carries_wall_clock_gap_across_restart() {
        let (settings, root) = settings("wall-gap");
        bootstrap_sqlite(settings.db_path(), settings.server_run_id()).unwrap();
        persist_runtime_clock(&settings, 100, 1_000).unwrap();
        assert_eq!(load_runtime_clock_at(&settings, 1_005).unwrap(), 200);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn legacy_inventory_rebase_reads_nested_freshness_without_panicking() {
        let (settings, root) = settings("legacy-json");
        bootstrap_sqlite(settings.db_path(), settings.server_run_id()).unwrap();
        let connection = Connection::open(settings.db_path()).unwrap();
        connection
            .execute(
                "INSERT INTO inventories (username, inventory_json, schema_version, last_updated_wall) VALUES (?1, ?2, 1, ?3)",
                params![
                    "Azure",
                    r#"{"containers":[{"items":[{"instance":{"freshness":{"created_at_tick":42}}}]}]}"#,
                    1_000_i64,
                ],
            )
            .unwrap();
        assert_eq!(load_runtime_clock_at(&settings, 1_005).unwrap(), 142);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn malformed_legacy_inventory_does_not_block_valid_freshness_rebase() {
        let (settings, root) = settings("legacy-malformed-json");
        bootstrap_sqlite(settings.db_path(), settings.server_run_id()).unwrap();
        let connection = Connection::open(settings.db_path()).unwrap();
        connection
            .execute(
                "INSERT INTO inventories (username, inventory_json, schema_version, last_updated_wall) VALUES (?1, ?2, 1, ?3)",
                params!["Broken", "{", 1_000_i64],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO inventories (username, inventory_json, schema_version, last_updated_wall) VALUES (?1, ?2, 1, ?3)",
                params![
                    "Valid",
                    r#"{"items":[{"freshness":{"created_at_tick":42}}]}"#,
                    1_000_i64
                ],
            )
            .unwrap();

        assert_eq!(load_runtime_clock_at(&settings, 1_005).unwrap(), 142);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn legacy_rebase_ignores_future_inventory_deadline_tick() {
        let (settings, root) = settings("legacy-future-deadline");
        bootstrap_sqlite(settings.db_path(), settings.server_run_id()).unwrap();
        let connection = Connection::open(settings.db_path()).unwrap();
        connection
            .execute(
                "INSERT INTO inventories (username, inventory_json, schema_version, last_updated_wall) VALUES (?1, ?2, 1, ?3)",
                params![
                    "Legacy",
                    r#"{"freshness":{"created_at_tick":100,"expires_at_tick":1000000}}"#,
                    1_000_i64
                ],
            )
            .unwrap();

        assert_eq!(
            load_runtime_clock_at(&settings, 1_005).unwrap(),
            200,
            "future inventory deadlines must not move the migrated runtime clock"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn legacy_rebase_uses_per_row_tick_and_wall_max() {
        let (settings, root) = settings("legacy-per-row-rebase");
        bootstrap_sqlite(settings.db_path(), settings.server_run_id()).unwrap();
        let connection = Connection::open(settings.db_path()).unwrap();
        for (username, created_at_tick, snapshot_wall) in [
            ("OlderSnapshot", 100_i64, 1_000_i64),
            ("NewerSnapshot", 150_i64, 995_i64),
        ] {
            connection
                .execute(
                    "INSERT INTO inventories (username, inventory_json, schema_version, last_updated_wall) VALUES (?1, ?2, 1, ?3)",
                    params![
                        username,
                        format!(r#"{{"freshness":{{"created_at_tick":{created_at_tick}}}}}"#),
                        snapshot_wall
                    ],
                )
                .unwrap();
        }

        assert_eq!(
            load_runtime_clock_at(&settings, 1_005).unwrap(),
            350,
            "each inventory row must add its own known downtime before taking the maximum"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn legacy_inventory_with_unknown_wall_clock_does_not_jump_from_unix_epoch() {
        let (settings, root) = settings("legacy-zero-wall");
        bootstrap_sqlite(settings.db_path(), settings.server_run_id()).unwrap();
        let connection = Connection::open(settings.db_path()).unwrap();
        connection
            .execute(
                "INSERT INTO inventories (username, inventory_json, schema_version, last_updated_wall) VALUES (?1, ?2, 1, ?3)",
                params![
                    "Legacy",
                    r#"{"freshness":{"created_at_tick":42}}"#,
                    0_i64
                ],
            )
            .unwrap();

        assert_eq!(load_runtime_clock_at(&settings, 1_005).unwrap(), 42);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn legacy_freshness_created_at_zero_still_ages_during_known_downtime() {
        let (settings, root) = settings("legacy-zero-created-tick");
        bootstrap_sqlite(settings.db_path(), settings.server_run_id()).unwrap();
        let connection = Connection::open(settings.db_path()).unwrap();
        connection
            .execute(
                "INSERT INTO inventories (username, inventory_json, schema_version, last_updated_wall) VALUES (?1, ?2, 1, ?3)",
                params![
                    "Legacy",
                    r#"{"freshness":{"created_at_tick":0}}"#,
                    1_000_i64
                ],
            )
            .unwrap();

        assert_eq!(load_runtime_clock_at(&settings, 1_005).unwrap(), 100);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn legacy_rebase_uses_maximum_persisted_runtime_tick_for_known_age() {
        let (settings, root) = settings("legacy-age-monotonic");
        bootstrap_sqlite(settings.db_path(), settings.server_run_id()).unwrap();
        let connection = Connection::open(settings.db_path()).unwrap();
        connection
            .execute(
                "INSERT INTO inventories (username, inventory_json, schema_version, last_updated_wall) VALUES (?1, ?2, 1, ?3)",
                params![
                    "Legacy",
                    r#"{"freshness":{"created_at_tick":100}}"#,
                    1_000_i64
                ],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO life_events (event_id, char_id, event_type, payload_json, payload_version, game_tick, wall_clock, schema_version) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    "legacy-runtime-tick",
                    "char:legacy",
                    "test",
                    "{}",
                    1_i64,
                    10_000_i64,
                    1_000_i64,
                    1_i64
                ],
            )
            .unwrap();

        let migrated_now = load_runtime_clock_at(&settings, 1_005).unwrap();
        let migrated_age = migrated_now.saturating_sub(100);
        let known_pre_migration_age = 10_000_u64.saturating_sub(100);
        assert!(
            migrated_age >= known_pre_migration_age,
            "legacy rebase must not make known age younger: migrated={migrated_age}, known={known_pre_migration_age}"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn legacy_rebase_does_not_make_recent_item_immediately_expired() {
        let (settings, root) = settings("legacy-no-unix-epoch");
        bootstrap_sqlite(settings.db_path(), settings.server_run_id()).unwrap();
        let connection = Connection::open(settings.db_path()).unwrap();
        connection
            .execute(
                "INSERT INTO inventories (username, inventory_json, schema_version, last_updated_wall) VALUES (?1, ?2, 1, ?3)",
                params![
                    "Legacy",
                    r#"{"freshness":{"created_at_tick":100}}"#,
                    1_000_i64
                ],
            )
            .unwrap();

        let migrated_now = load_runtime_clock_at(&settings, 1_005).unwrap();
        assert_eq!(migrated_now, 200);

        let profile = DecayProfile::Spoil {
            id: DecayProfileId::new("legacy-test"),
            formula: DecayFormula::Exponential {
                half_life_ticks: 10_000,
            },
            spoil_threshold: 10.0,
        };
        let freshness = Freshness::new(100, 100.0, &profile);
        assert_eq!(
            compute_track_state(&freshness, &profile, migrated_now, 1.0),
            TrackState::Fresh,
            "legacy migration must not turn a recently created item into an immediately expired item"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn bootstrap_hydrates_all_freshness_clock_resources_from_one_epoch() {
        let (settings, root) = settings("bootstrap-hydrates-clocks");
        bootstrap_sqlite(settings.db_path(), settings.server_run_id()).unwrap();
        let persisted_wall = current_unix_seconds().saturating_sub(5);
        persist_runtime_clock(&settings, 100, persisted_wall).unwrap();

        let mut app = App::new();
        app.insert_resource(settings.clone());
        app.insert_resource(DailyBackupState::default());
        app.insert_resource(CultivationClock::default());
        app.insert_resource(crate::combat::CombatClock::default());
        app.insert_resource(crate::player::gameplay::GameplayTick::default());
        app.insert_resource(crate::shelflife::sweep::ShelflifeSweepTick::default());
        app.insert_resource(WorldQiAccount::default());
        app.add_systems(Startup, bootstrap_persistence_system);
        app.update();

        let expected_minimum = 100 + 5 * TICKS_PER_SECOND;
        let cultivation_tick = app.world().resource::<CultivationClock>().tick;
        assert!(
            cultivation_tick >= expected_minimum,
            "restart hydration must include the wall-clock gap, got {cultivation_tick}"
        );
        assert_eq!(
            app.world().resource::<crate::combat::CombatClock>().tick,
            cultivation_tick
        );
        assert_eq!(
            app.world()
                .resource::<crate::player::gameplay::GameplayTick>()
                .current_tick(),
            cultivation_tick
        );
        assert_eq!(
            app.world()
                .resource::<crate::shelflife::sweep::ShelflifeSweepTick>()
                .0,
            cultivation_tick
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
