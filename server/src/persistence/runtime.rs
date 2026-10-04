//! Generic JSON storage for runtime persistence slices.
//!
//! The table is deliberately boring: domain modules own the payload schema and
//! lifecycle, while this module owns connection setup, atomic replacement, and
//! fail-closed JSON decoding. No Bevy entity identifier is persisted here.

use super::*;

const RUNTIME_SLICE_SCHEMA_VERSION: i32 = 1;

pub(crate) fn save_world_runtime_slice<T: Serialize>(
    settings: &PersistenceSettings,
    slice_id: &str,
    payload: &T,
) -> io::Result<()> {
    let payload_json = serde_json::to_string(payload).map_err(io::Error::other)?;
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    transaction
        .execute(
            "
            INSERT INTO world_runtime_slices
                (slice_id, payload_json, schema_version, last_updated_wall)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(slice_id) DO UPDATE SET
                payload_json = excluded.payload_json,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                slice_id,
                payload_json,
                RUNTIME_SLICE_SCHEMA_VERSION,
                current_unix_seconds()
            ],
        )
        .map_err(io::Error::other)?;
    transaction.commit().map_err(io::Error::other)
}

pub(crate) fn load_world_runtime_slice<T: DeserializeOwned>(
    settings: &PersistenceSettings,
    slice_id: &str,
) -> io::Result<Option<T>> {
    let connection = open_persistence_connection(settings)?;
    load_world_runtime_slice_from_connection(&connection, slice_id)
}

pub(crate) fn load_world_runtime_slice_from_connection<T: DeserializeOwned>(
    connection: &Connection,
    slice_id: &str,
) -> io::Result<Option<T>> {
    let payload: Option<(String, i32)> = connection
        .query_row(
            "SELECT payload_json, schema_version FROM world_runtime_slices WHERE slice_id = ?1",
            params![slice_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(io::Error::other)?;
    payload
        .map(|(payload, schema_version)| {
            if schema_version != RUNTIME_SLICE_SCHEMA_VERSION {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "world runtime slice `{slice_id}` has unsupported schema version {schema_version}"
                    ),
                ));
            }
            serde_json::from_str(&payload).map_err(io::Error::other)
        })
        .transpose()
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn save_player_runtime_slice<T: Serialize>(
    settings: &PersistenceSettings,
    username: &str,
    slice_id: &str,
    payload: &T,
) -> io::Result<()> {
    let payload_json = serde_json::to_string(payload).map_err(io::Error::other)?;
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    save_player_runtime_slice_in_transaction(
        &transaction,
        username,
        slice_id,
        &payload_json,
        current_unix_seconds(),
    )?;
    transaction.commit().map_err(io::Error::other)
}

pub(crate) fn save_player_runtime_slice_in_transaction(
    transaction: &rusqlite::Transaction<'_>,
    username: &str,
    slice_id: &str,
    payload_json: &str,
    wall_clock: i64,
) -> io::Result<()> {
    transaction
        .execute(
            "
            INSERT INTO player_runtime_slices
                (username, slice_id, payload_json, schema_version, last_updated_wall)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(username, slice_id) DO UPDATE SET
                payload_json = excluded.payload_json,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                username,
                slice_id,
                payload_json,
                RUNTIME_SLICE_SCHEMA_VERSION,
                wall_clock
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

pub(crate) fn load_player_runtime_slice<T: DeserializeOwned>(
    settings: &PersistenceSettings,
    username: &str,
    slice_id: &str,
) -> io::Result<Option<T>> {
    let connection = open_persistence_connection(settings)?;
    let payload: Option<(String, i32)> = connection
        .query_row(
            "
            SELECT payload_json, schema_version
            FROM player_runtime_slices
            WHERE username = ?1 AND slice_id = ?2
            ",
            params![username, slice_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(io::Error::other)?;
    payload
        .map(|(payload, schema_version)| {
            if schema_version != RUNTIME_SLICE_SCHEMA_VERSION {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "player runtime slice `{slice_id}` for `{username}` has unsupported schema version {schema_version}"
                    ),
                ));
            }
            serde_json::from_str(&payload).map_err(io::Error::other)
        })
        .transpose()
}

pub(crate) fn delete_player_runtime_slice_in_transaction(
    transaction: &rusqlite::Transaction<'_>,
    username: &str,
    slice_id: &str,
) -> io::Result<()> {
    transaction
        .execute(
            "DELETE FROM player_runtime_slices WHERE username = ?1 AND slice_id = ?2",
            params![username, slice_id],
        )
        .map(|_| ())
        .map_err(io::Error::other)
}
