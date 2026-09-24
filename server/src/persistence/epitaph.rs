//! Epitaph persistence.

use super::*;

// ─── plan-life-record-epitaph-v1 P0：碑刻持久化 ───────────────────────────────

/// 持久化一条碑刻到 SQLite epitaphs 表（幂等 ON CONFLICT DO UPDATE）。
///
/// 仿 `upsert_deceased_snapshot` 的 INSERT...ON CONFLICT 模式（:5926-:5963）。
/// SQLite epitaphs 表永久保留——即使 WorldEpitaphRegistry 内存 cap 淘汰也不删表行。
pub fn persist_epitaph(
    settings: &PersistenceSettings,
    entry: &crate::cultivation::epitaph::EpitaphEntry,
) -> io::Result<()> {
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(io::Error::other)?;
    let entry_json = serde_json::to_string(entry)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let wall_clock = current_unix_seconds();
    transaction
        .execute(
            "
            INSERT INTO epitaphs (
                epitaph_id,
                character_id,
                entry_json,
                death_tick,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(epitaph_id) DO UPDATE SET
                character_id      = excluded.character_id,
                entry_json        = excluded.entry_json,
                death_tick        = excluded.death_tick,
                schema_version    = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                entry.id.0,
                entry.character_id,
                entry_json,
                tick_to_sql(entry.death_tick)?,
                EVENT_SCHEMA_VERSION,
                wall_clock
            ],
        )
        .map_err(io::Error::other)?;
    transaction.commit().map_err(io::Error::other)?;
    Ok(())
}

/// 按 epitaph_id 读回单条碑刻（用于测试 round-trip 与运维查询）。
#[allow(dead_code)]
pub fn load_epitaph(
    settings: &PersistenceSettings,
    epitaph_id: &str,
) -> io::Result<Option<crate::cultivation::epitaph::EpitaphEntry>> {
    let connection = open_persistence_connection(settings)?;
    let result = connection
        .query_row(
            "SELECT entry_json FROM epitaphs WHERE epitaph_id = ?1",
            params![epitaph_id],
            |row| {
                let json: String = row.get(0)?;
                Ok(json)
            },
        )
        .optional()
        .map_err(io::Error::other)?;
    match result {
        None => Ok(None),
        Some(json) => {
            let entry: crate::cultivation::epitaph::EpitaphEntry = serde_json::from_str(&json)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            Ok(Some(entry))
        }
    }
}
