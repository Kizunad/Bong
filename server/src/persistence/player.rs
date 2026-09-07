//! Player cultivation and durable dropped-loot persistence helpers.

use super::*;

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
    inventory: &crate::inventory::PlayerInventory,
    instance_id: u64,
) -> io::Result<()> {
    let instance_id_sql = i64::try_from(instance_id).map_err(io::Error::other)?;
    let entry_json: Option<String> = transaction
        .query_row(
            "SELECT entry_json FROM dropped_loot WHERE instance_id = ?1",
            params![instance_id_sql],
            |row| row.get(0),
        )
        .optional()
        .map_err(io::Error::other)?;
    let Some(entry_json) = entry_json else {
        // 没有 durable row 时没有发生删除；保留原有幂等语义，让同一进程内刚生成但
        // 尚未落盘的掉落仍可完成正常 inventory checkpoint。
        return Ok(());
    };

    let entry: DroppedLootEntry = serde_json::from_str(&entry_json)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if entry.instance_id != instance_id
        || entry.instance_id != entry.item.instance_id
        || entry.instance_id > JS_SAFE_INTEGER_MAX
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "durable dropped loot id mismatch row={instance_id} entry={} item={}",
                entry.instance_id, entry.item.instance_id
            ),
        ));
    }

    let carried = crate::inventory::inventory_item_by_instance_borrow(inventory, instance_id)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "refusing to delete dropped loot instance {instance_id} without inventory ownership proof"
                ),
            )
        })?;
    if !carried.spirit_quality.is_finite()
        || carried.spirit_quality < 0.0
        || carried.spirit_quality > entry.item.spirit_quality
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "dropped loot instance {instance_id} has invalid carried spirit_quality={} for durable value {}",
                carried.spirit_quality, entry.item.spirit_quality
            ),
        ));
    }
    // Pickup attrition is the only legal mutation between attaching the durable item and this
    // checkpoint: it lowers `spirit_quality` and records the released qi against the zone. All
    // other fields must remain byte-for-byte equivalent, otherwise a caller could delete one
    // durable item while persisting a different instance under the same id.
    let mut expected_item = entry.item;
    expected_item.spirit_quality = carried.spirit_quality;
    if carried != &expected_item {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "refusing to delete dropped loot instance {instance_id}: carried item does not match durable payload"
            ),
        ));
    }

    transaction
        .execute(
            "DELETE FROM dropped_loot WHERE instance_id = ?1 AND entry_json = ?2",
            params![instance_id_sql, entry_json],
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
