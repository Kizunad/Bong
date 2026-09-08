//! Persistent void-action cooldown state.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct VoidActionCooldownRecord {
    character_id: String,
    kind: VoidActionKind,
    ready_at_tick: u64,
}

pub fn persist_void_action_cooldown(
    settings: &PersistenceSettings,
    character_id: &str,
    kind: VoidActionKind,
    ready_at_tick: u64,
) -> io::Result<()> {
    if kind.cooldown_ticks() == 0 {
        return Ok(());
    }
    let ready_at_tick = i64::try_from(ready_at_tick).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("void-action cooldown tick overflows sqlite INTEGER: {error}"),
        )
    })?;
    let connection = open_persistence_connection(settings)?;
    connection
        .execute(
            "
            INSERT INTO void_action_cooldowns (
                character_id,
                kind,
                ready_at_tick,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(character_id, kind) DO UPDATE SET
                ready_at_tick = excluded.ready_at_tick,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                character_id,
                kind.wire_name(),
                ready_at_tick,
                current_unix_seconds(),
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

pub(super) fn load_void_action_cooldown_records(
    settings: &PersistenceSettings,
) -> io::Result<Vec<VoidActionCooldownRecord>> {
    let connection = open_persistence_connection(settings)?;
    let mut statement = connection
        .prepare(
            "
            SELECT character_id, kind, ready_at_tick
            FROM void_action_cooldowns
            ORDER BY character_id, kind
            ",
        )
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map([], |row| {
            let character_id: String = row.get(0)?;
            let kind_name: String = row.get(1)?;
            let kind = VoidActionKind::from_wire_name(kind_name.as_str()).ok_or_else(|| {
                rusqlite::Error::FromSqlConversionFailure(
                    1,
                    Type::Text,
                    Box::new(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("unknown void-action kind `{kind_name}`"),
                    )),
                )
            })?;
            let ready_at_tick: i64 = row.get(2)?;
            Ok(VoidActionCooldownRecord {
                character_id,
                kind,
                ready_at_tick: ready_at_tick as u64,
            })
        })
        .map_err(io::Error::other)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(io::Error::other)
}

pub(super) fn hydrate_void_action_cooldowns(
    settings: &PersistenceSettings,
    cooldowns: &mut VoidActionCooldowns,
) -> io::Result<usize> {
    let records = load_void_action_cooldown_records(settings)?;
    let count = records.len();
    for record in records {
        cooldowns.force_ready_at(
            record.character_id.as_str(),
            record.kind,
            record.ready_at_tick,
        );
    }
    Ok(count)
}

pub(super) fn legacy_player_realm_to_cultivation(realm: &str) -> Option<Realm> {
    match realm {
        "mortal" => Some(Realm::Awaken),
        "qi_refining_1" => Some(Realm::Induce),
        "qi_refining_2" => Some(Realm::Condense),
        "qi_refining_3" | "foundation_establishment_1" => Some(Realm::Spirit),
        "Awaken" => Some(Realm::Awaken),
        "Induce" => Some(Realm::Induce),
        "Condense" => Some(Realm::Condense),
        "Solidify" => Some(Realm::Solidify),
        "Spirit" => Some(Realm::Spirit),
        "Void" => Some(Realm::Void),
        _ => None,
    }
}
