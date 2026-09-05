//! Agent world-model snapshot and append-only history persistence.

use super::*;

pub fn bootstrap_agent_world_model_mirror(
    settings: &PersistenceSettings,
) -> io::Result<Option<AgentWorldModelSnapshotRecord>> {
    let snapshot = load_agent_world_model_snapshot(settings)?;
    Ok(snapshot)
}

pub fn world_model_snapshot_to_mirror_fields(
    snapshot: &AgentWorldModelSnapshotRecord,
) -> io::Result<BTreeMap<String, String>> {
    let mut fields = BTreeMap::new();
    fields.insert(
        WORLD_MODEL_STATE_FIELD_CURRENT_ERA.to_string(),
        serde_json::to_string(&snapshot.current_era)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
    );
    fields.insert(
        WORLD_MODEL_STATE_FIELD_ZONE_HISTORY.to_string(),
        serde_json::to_string(&snapshot.zone_history)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
    );
    fields.insert(
        WORLD_MODEL_STATE_FIELD_LAST_DECISIONS.to_string(),
        serde_json::to_string(&snapshot.last_decisions)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
    );
    fields.insert(
        WORLD_MODEL_STATE_FIELD_PLAYER_FIRST_SEEN_TICK.to_string(),
        serde_json::to_string(&snapshot.player_first_seen_tick)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
    );
    fields.insert(
        WORLD_MODEL_STATE_FIELD_NEG_DOMAIN_PENDING_TRIBULATIONS.to_string(),
        serde_json::to_string(&snapshot.neg_domain_pending_tribulations)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
    );
    fields.insert(
        WORLD_MODEL_STATE_FIELD_NEG_DOMAIN_ESCAPE_TELEMETRY.to_string(),
        serde_json::to_string(&snapshot.neg_domain_escape_telemetry)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
    );
    fields.insert(
        WORLD_MODEL_STATE_FIELD_NEG_DOMAIN_ESCAPE_SESSIONS.to_string(),
        serde_json::to_string(&snapshot.neg_domain_escape_sessions)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
    );
    fields.insert(
        WORLD_MODEL_STATE_FIELD_LAST_TICK.to_string(),
        snapshot
            .last_tick
            .map(|value| value.to_string())
            .unwrap_or_default(),
    );
    fields.insert(
        WORLD_MODEL_STATE_FIELD_LAST_STATE_TS.to_string(),
        snapshot
            .last_state_ts
            .map(|value| value.to_string())
            .unwrap_or_default(),
    );
    Ok(fields)
}

pub(super) fn ensure_agent_world_model_table(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS agent_world_model (
            row_id INTEGER PRIMARY KEY CHECK (row_id = 1),
            snapshot_json TEXT NOT NULL,
            schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
            last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
        );
        ",
    )?;
    Ok(())
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn persist_agent_world_model_snapshot(
    settings: &PersistenceSettings,
    snapshot: &AgentWorldModelSnapshotRecord,
) -> io::Result<()> {
    let wall_clock = current_unix_seconds();
    let snapshot_json = serde_json::to_string(snapshot)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    transaction
        .execute(
            "
            INSERT INTO agent_world_model (
                row_id,
                snapshot_json,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(row_id) DO UPDATE SET
                snapshot_json = excluded.snapshot_json,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                AGENT_WORLD_MODEL_ROW_ID,
                snapshot_json,
                CURRENT_SCHEMA_VERSION,
                wall_clock
            ],
        )
        .map_err(io::Error::other)?;
    transaction.commit().map_err(io::Error::other)
}

pub fn persist_agent_world_model_authority_state(
    settings: &PersistenceSettings,
    envelope_id: &str,
    source: &str,
    snapshot: &AgentWorldModelSnapshotRecord,
) -> io::Result<()> {
    let wall_clock = current_unix_seconds();
    let snapshot_json = serde_json::to_string(snapshot)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    upsert_agent_world_model_snapshot(&transaction, &snapshot_json, wall_clock)?;
    if let Some(era) = snapshot.current_era.as_ref() {
        append_agent_era(
            &transaction,
            envelope_id,
            source,
            era,
            snapshot.last_tick,
            wall_clock,
        )?;
    }
    for (agent_name, decision) in &snapshot.last_decisions {
        append_agent_decision(
            &transaction,
            envelope_id,
            source,
            agent_name,
            decision,
            snapshot.last_tick,
            wall_clock,
        )?;
    }
    prune_agent_world_model_append_only(&transaction, wall_clock)?;
    transaction.commit().map_err(io::Error::other)
}

pub fn load_agent_world_model_snapshot(
    settings: &PersistenceSettings,
) -> io::Result<Option<AgentWorldModelSnapshotRecord>> {
    let connection = open_persistence_connection(settings)?;
    let snapshot_json: Option<String> = connection
        .query_row(
            "SELECT snapshot_json FROM agent_world_model WHERE row_id = ?1",
            params![AGENT_WORLD_MODEL_ROW_ID],
            |row| row.get(0),
        )
        .optional()
        .map_err(io::Error::other)?;

    let Some(snapshot_json) = snapshot_json else {
        return Ok(None);
    };

    let snapshot = serde_json::from_str::<AgentWorldModelSnapshotRecord>(&snapshot_json)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    Ok(Some(snapshot))
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn load_agent_eras(settings: &PersistenceSettings) -> io::Result<Vec<AgentEraRecord>> {
    let connection = open_persistence_connection(settings)?;
    load_agent_eras_from_connection(&connection)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn load_agent_decisions(
    settings: &PersistenceSettings,
) -> io::Result<Vec<AgentDecisionRecord>> {
    let connection = open_persistence_connection(settings)?;
    load_agent_decisions_from_connection(&connection)
}

pub(super) fn prune_agent_world_model_append_only(
    transaction: &rusqlite::Transaction<'_>,
    now_wall: i64,
) -> io::Result<()> {
    let threshold = now_wall.saturating_sub(AGENT_WORLD_MODEL_APPEND_ONLY_RETENTION_SECS);
    transaction
        .execute(
            "DELETE FROM agent_eras WHERE observed_at_wall < ?1",
            params![threshold],
        )
        .map_err(io::Error::other)?;
    transaction
        .execute(
            "DELETE FROM agent_decisions WHERE observed_at_wall < ?1",
            params![threshold],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

pub(super) fn upsert_agent_world_model_snapshot(
    transaction: &rusqlite::Transaction<'_>,
    snapshot_json: &str,
    wall_clock: i64,
) -> io::Result<()> {
    transaction
        .execute(
            "
            INSERT INTO agent_world_model (
                row_id,
                snapshot_json,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(row_id) DO UPDATE SET
                snapshot_json = excluded.snapshot_json,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                AGENT_WORLD_MODEL_ROW_ID,
                snapshot_json,
                CURRENT_SCHEMA_VERSION,
                wall_clock
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

pub(super) fn append_agent_era(
    transaction: &rusqlite::Transaction<'_>,
    envelope_id: &str,
    source: &str,
    era: &serde_json::Value,
    observed_at_tick: Option<i64>,
    wall_clock: i64,
) -> io::Result<()> {
    let era_name = era
        .get("name")
        .and_then(|value| value.as_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "agent era missing name"))?;
    let since_tick = era
        .get("since_tick")
        .and_then(|value| value.as_i64())
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "agent era missing since_tick")
        })?;
    let global_effect = era
        .get("global_effect")
        .and_then(|value| value.as_str())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "agent era missing global_effect",
            )
        })?;
    transaction
        .execute(
            "
            INSERT INTO agent_eras (
                event_id,
                envelope_id,
                source,
                era_name,
                since_tick,
                global_effect,
                observed_at_tick,
                observed_at_wall,
                schema_version
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ",
            params![
                Uuid::now_v7().to_string(),
                envelope_id,
                source,
                era_name,
                since_tick,
                global_effect,
                observed_at_tick,
                wall_clock,
                EVENT_SCHEMA_VERSION,
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

pub(super) fn append_agent_decision(
    transaction: &rusqlite::Transaction<'_>,
    envelope_id: &str,
    source: &str,
    agent_name: &str,
    decision: &AgentWorldModelDecisionRecord,
    observed_at_tick: Option<i64>,
    wall_clock: i64,
) -> io::Result<()> {
    let payload_json = serde_json::to_string(decision)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    transaction
        .execute(
            "
            INSERT INTO agent_decisions (
                event_id,
                envelope_id,
                source,
                agent_name,
                reasoning,
                command_count,
                narration_count,
                payload_json,
                observed_at_tick,
                observed_at_wall,
                schema_version
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ",
            params![
                Uuid::now_v7().to_string(),
                envelope_id,
                source,
                agent_name,
                decision.reasoning,
                i64::try_from(decision.commands.len())
                    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
                i64::try_from(decision.narrations.len())
                    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
                payload_json,
                observed_at_tick,
                wall_clock,
                EVENT_SCHEMA_VERSION,
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

pub(super) fn load_agent_eras_from_connection(
    connection: &Connection,
) -> io::Result<Vec<AgentEraRecord>> {
    let mut statement = connection
        .prepare(
            "
            SELECT event_id, envelope_id, source, era_name, since_tick, global_effect,
                   observed_at_tick, observed_at_wall
            FROM agent_eras
            ORDER BY observed_at_wall ASC, event_id ASC
            ",
        )
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map([], |row| {
            Ok(AgentEraRecord {
                event_id: row.get(0)?,
                envelope_id: row.get(1)?,
                source: row.get(2)?,
                era_name: row.get(3)?,
                since_tick: row.get(4)?,
                global_effect: row.get(5)?,
                observed_at_tick: row.get(6)?,
                observed_at_wall: row.get(7)?,
            })
        })
        .map_err(io::Error::other)?;

    let mut records = Vec::new();
    for row in rows {
        records.push(row.map_err(io::Error::other)?);
    }
    Ok(records)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn load_agent_decisions_from_connection(
    connection: &Connection,
) -> io::Result<Vec<AgentDecisionRecord>> {
    let mut statement = connection
        .prepare(
            "
            SELECT event_id, envelope_id, source, agent_name, reasoning, command_count,
                   narration_count, payload_json, observed_at_tick, observed_at_wall
            FROM agent_decisions
            ORDER BY observed_at_wall ASC, event_id ASC
            ",
        )
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, Option<i64>>(8)?,
                row.get::<_, i64>(9)?,
            ))
        })
        .map_err(io::Error::other)?;

    let mut records = Vec::new();
    for row in rows {
        let (
            event_id,
            envelope_id,
            source,
            agent_name,
            reasoning,
            command_count,
            narration_count,
            payload_json,
            observed_at_tick,
            observed_at_wall,
        ) = row.map_err(io::Error::other)?;
        records.push(AgentDecisionRecord {
            event_id,
            envelope_id,
            source,
            agent_name,
            reasoning,
            command_count: sql_to_u32(command_count)?,
            narration_count: sql_to_u32(narration_count)?,
            payload_json,
            observed_at_tick,
            observed_at_wall,
        });
    }
    Ok(records)
}
