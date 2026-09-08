//! Faction and social persistence bundles.

use super::*;

#[cfg_attr(not(test), allow(dead_code))]
pub fn replace_faction_social_state(
    settings: &PersistenceSettings,
    bundle: &FactionSocialBundle,
) -> io::Result<()> {
    let wall_clock = current_unix_seconds();
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    (|| -> io::Result<()> {
        transaction
            .execute("DELETE FROM factions", [])
            .map_err(io::Error::other)?;
        transaction
            .execute("DELETE FROM reputation", [])
            .map_err(io::Error::other)?;
        transaction
            .execute("DELETE FROM membership", [])
            .map_err(io::Error::other)?;
        transaction
            .execute("DELETE FROM relationships", [])
            .map_err(io::Error::other)?;

        for faction in &bundle.factions {
            upsert_faction(&transaction, faction, wall_clock)?;
        }
        for reputation in &bundle.reputations {
            upsert_faction_reputation(&transaction, reputation, wall_clock)?;
        }
        for membership in &bundle.memberships {
            upsert_faction_membership(&transaction, membership, wall_clock)?;
        }
        for relationship in &bundle.relationships {
            upsert_relationship(&transaction, relationship, wall_clock)?;
        }

        transaction.commit().map_err(io::Error::other)
    })()
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn load_faction_social_state(
    settings: &PersistenceSettings,
) -> io::Result<FactionSocialBundle> {
    let connection = open_persistence_connection(settings)?;
    Ok(FactionSocialBundle {
        factions: load_factions_from_connection(&connection)?,
        reputations: load_reputations_from_connection(&connection)?,
        memberships: load_memberships_from_connection(&connection)?,
        relationships: load_relationships_from_connection(&connection)?,
    })
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn upsert_faction(
    transaction: &rusqlite::Transaction<'_>,
    faction: &FactionRecord,
    wall_clock: i64,
) -> io::Result<()> {
    transaction
        .execute(
            "
            INSERT INTO factions (
                faction_id, display_name, doctrine, metadata_json, schema_version, last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(faction_id) DO UPDATE SET
                display_name = excluded.display_name,
                doctrine = excluded.doctrine,
                metadata_json = excluded.metadata_json,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                faction.faction_id,
                faction.display_name,
                faction.doctrine,
                faction.metadata_json,
                NPC_ROW_SCHEMA_VERSION,
                wall_clock,
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn upsert_faction_reputation(
    transaction: &rusqlite::Transaction<'_>,
    reputation: &FactionReputationRecord,
    wall_clock: i64,
) -> io::Result<()> {
    transaction
        .execute(
            "
            INSERT INTO reputation (
                faction_id, target_faction_id, score, schema_version, last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(faction_id, target_faction_id) DO UPDATE SET
                score = excluded.score,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                reputation.faction_id,
                reputation.target_faction_id,
                reputation.score,
                NPC_ROW_SCHEMA_VERSION,
                wall_clock,
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn upsert_faction_membership(
    transaction: &rusqlite::Transaction<'_>,
    membership: &FactionMembershipRecord,
    wall_clock: i64,
) -> io::Result<()> {
    transaction
        .execute(
            "
            INSERT INTO membership (
                faction_id, char_id, role, joined_at_tick, metadata_json, schema_version, last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(faction_id, char_id) DO UPDATE SET
                role = excluded.role,
                joined_at_tick = excluded.joined_at_tick,
                metadata_json = excluded.metadata_json,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                membership.faction_id,
                membership.char_id,
                membership.role,
                tick_to_sql(membership.joined_at_tick)?,
                membership.metadata_json,
                NPC_ROW_SCHEMA_VERSION,
                wall_clock,
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn upsert_relationship(
    transaction: &rusqlite::Transaction<'_>,
    relationship: &RelationshipRecord,
    wall_clock: i64,
) -> io::Result<()> {
    transaction
        .execute(
            "
            INSERT INTO relationships (
                char_id, peer_char_id, relationship_type, since_tick, metadata_json, schema_version, last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(char_id, peer_char_id, relationship_type) DO UPDATE SET
                since_tick = excluded.since_tick,
                metadata_json = excluded.metadata_json,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                relationship.char_id,
                relationship.peer_char_id,
                relationship.relationship_type,
                tick_to_sql(relationship.since_tick)?,
                relationship.metadata_json,
                NPC_ROW_SCHEMA_VERSION,
                wall_clock,
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn load_factions_from_connection(
    connection: &Connection,
) -> io::Result<Vec<FactionRecord>> {
    let mut statement = connection
        .prepare(
            "SELECT faction_id, display_name, doctrine, metadata_json FROM factions ORDER BY faction_id ASC",
        )
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map([], |row| {
            Ok(FactionRecord {
                faction_id: row.get(0)?,
                display_name: row.get(1)?,
                doctrine: row.get(2)?,
                metadata_json: row.get(3)?,
            })
        })
        .map_err(io::Error::other)?;
    collect_rows(rows)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn load_reputations_from_connection(
    connection: &Connection,
) -> io::Result<Vec<FactionReputationRecord>> {
    let mut statement = connection
        .prepare(
            "SELECT faction_id, target_faction_id, score FROM reputation ORDER BY faction_id ASC, target_faction_id ASC",
        )
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map([], |row| {
            Ok(FactionReputationRecord {
                faction_id: row.get(0)?,
                target_faction_id: row.get(1)?,
                score: row.get(2)?,
            })
        })
        .map_err(io::Error::other)?;
    collect_rows(rows)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn load_memberships_from_connection(
    connection: &Connection,
) -> io::Result<Vec<FactionMembershipRecord>> {
    let mut statement = connection
        .prepare(
            "
            SELECT faction_id, char_id, role, joined_at_tick, metadata_json
            FROM membership
            ORDER BY faction_id ASC, char_id ASC
            ",
        )
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map([], |row| {
            Ok(FactionMembershipRecord {
                faction_id: row.get(0)?,
                char_id: row.get(1)?,
                role: row.get(2)?,
                joined_at_tick: sql_to_tick(row.get::<_, i64>(3)?).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        3,
                        rusqlite::types::Type::Integer,
                        Box::new(error),
                    )
                })?,
                metadata_json: row.get(4)?,
            })
        })
        .map_err(io::Error::other)?;
    collect_rows(rows)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn load_relationships_from_connection(
    connection: &Connection,
) -> io::Result<Vec<RelationshipRecord>> {
    let mut statement = connection
        .prepare(
            "
            SELECT char_id, peer_char_id, relationship_type, since_tick, metadata_json
            FROM relationships
            ORDER BY char_id ASC, peer_char_id ASC, relationship_type ASC
            ",
        )
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map([], |row| {
            Ok(RelationshipRecord {
                char_id: row.get(0)?,
                peer_char_id: row.get(1)?,
                relationship_type: row.get(2)?,
                since_tick: sql_to_tick(row.get::<_, i64>(3)?).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        3,
                        rusqlite::types::Type::Integer,
                        Box::new(error),
                    )
                })?,
                metadata_json: row.get(4)?,
            })
        })
        .map_err(io::Error::other)?;
    collect_rows(rows)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn collect_rows<T>(
    rows: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>>,
) -> io::Result<Vec<T>> {
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(io::Error::other)?);
    }
    Ok(out)
}
