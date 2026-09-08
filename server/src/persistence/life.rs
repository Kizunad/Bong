//! Life, death, revival, lifespan, and deceased archive persistence.

use super::*;

pub fn persist_near_death_transition(
    settings: &PersistenceSettings,
    lifecycle: &Lifecycle,
    life_record: &LifeRecord,
    cause: &str,
    lifespan_event: Option<&LifespanEventRecord>,
) -> io::Result<()> {
    let entry = latest_biography_entry(life_record)?;
    let wall_clock = current_unix_seconds();
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;

    upsert_life_record(&transaction, life_record, wall_clock)?;
    append_life_event(
        &transaction,
        life_record.character_id.as_str(),
        entry,
        wall_clock,
    )?;
    upsert_death_registry(
        &transaction,
        life_record.character_id.as_str(),
        lifecycle,
        cause,
        wall_clock,
    )?;
    if let Some(lifespan_event) = lifespan_event {
        append_lifespan_event(
            &transaction,
            life_record.character_id.as_str(),
            lifespan_event,
            wall_clock,
        )?;
    }

    transaction.commit().map_err(io::Error::other)
}

#[allow(clippy::too_many_arguments)]
pub fn persist_revival_qi_transaction(
    settings: &PersistenceSettings,
    username: &str,
    cultivation: &Cultivation,
    meridians: &MeridianSystem,
    contamination: &Contamination,
    life_record: &LifeRecord,
    zones: Option<&crate::world::zone::ZoneRegistry>,
    qi_ledger: &WorldQiAccount,
    release_void_quota: bool,
) -> io::Result<Option<AscensionQuotaRelease>> {
    let entry = latest_biography_entry(life_record)?;
    if !matches!(entry, BiographyEntry::Rebirth { .. }) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "revival qi transaction requires the latest biography entry to be Rebirth",
        ));
    }

    let wall_clock = current_unix_seconds();
    let mut connection = open_persistence_connection(settings)?;
    // Revival owns the actor bundle, biography, signed zone pressure, stable ownerless qi pools
    // and the optional 化虚 quota release as one durable transition. IMMEDIATE serializes the
    // quota read-modify-write with all other quota writers; every write below rolls back if a
    // later owner fails validation or persistence.
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(io::Error::other)?;
    let updated_bundle = prepare_revival_player_cultivation_bundle(
        &transaction,
        username,
        cultivation,
        meridians,
        contamination,
        life_record,
    )?;

    update_revival_player_cultivation_bundle(&transaction, username, &updated_bundle, wall_clock)?;
    upsert_life_record(&transaction, life_record, wall_clock)?;
    append_life_event(
        &transaction,
        life_record.character_id.as_str(),
        entry,
        wall_clock,
    )?;
    if let Some(zones) = zones {
        persist_zone_runtime_records(&transaction, zones, wall_clock)?;
    }
    upsert_runtime_qi_account_balances(&transaction, qi_ledger, wall_clock)?;

    let quota_release = if release_void_quota {
        let mut quota = load_ascension_quota_from_transaction(&transaction)?;
        let opened_slot = quota.occupied_slots > 0;
        quota.occupied_slots = quota.occupied_slots.saturating_sub(1);
        upsert_ascension_quota(&transaction, &quota, wall_clock)?;
        Some(AscensionQuotaRelease { quota, opened_slot })
    } else {
        None
    };

    transaction.commit().map_err(io::Error::other)?;
    Ok(quota_release)
}

/// Build the revival replacement blob only from an existing, fully decodable player bundle.
///
/// This is intentionally stricter than `upsert_player_cultivation_slice`: revival is a durable
/// owner transfer, so it may not manufacture a partial bundle or overwrite corrupt sibling state.
/// Only the four staged owner slices (and the bundle version needed for the current meridian
/// wire shape) change; every other sibling value is retained bit-for-bit in the JSON object.
pub(super) fn prepare_revival_player_cultivation_bundle(
    transaction: &rusqlite::Transaction<'_>,
    username: &str,
    cultivation: &Cultivation,
    meridians: &MeridianSystem,
    contamination: &Contamination,
    life_record: &LifeRecord,
) -> io::Result<String> {
    let existing: Option<String> = transaction
        .query_row(
            "SELECT cultivation_json FROM player_cultivation WHERE username = ?1",
            params![username],
            |row| row.get(0),
        )
        .optional()
        .map_err(io::Error::other)?;
    let existing = existing.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("revival requires an existing player_cultivation bundle for `{username}`"),
        )
    })?;
    let mut bundle: serde_json::Value = serde_json::from_str(&existing)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let object = bundle.as_object_mut().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("player_cultivation for `{username}` must be a JSON object"),
        )
    })?;
    let bundle_version = match object.get("v") {
        Some(version) => version.as_i64().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("player_cultivation for `{username}` has a non-integer bundle version"),
            )
        })?,
        None => 1,
    };
    if bundle_version < 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "player_cultivation for `{username}` has invalid bundle version {bundle_version}"
            ),
        ));
    }

    {
        let required_slice = |name: &str| {
            object.get(name).cloned().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "player_cultivation for `{username}` is missing required `{name}` slice"
                    ),
                )
            })
        };
        crate::cultivation::components::decode_persisted_cultivation(required_slice(
            "cultivation",
        )?)
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "player_cultivation for `{username}` has invalid cultivation slice: {error}"
                ),
            )
        })?;
        crate::cultivation::legacy_meridian_bundle::decode_meridian_system(
            required_slice("meridians")?,
            bundle_version,
        )
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("player_cultivation for `{username}` has invalid meridians slice: {error}"),
            )
        })?;
        serde_json::from_value::<Contamination>(required_slice("contamination")?).map_err(
            |error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "player_cultivation for `{username}` has invalid contamination slice: {error}"
                    ),
                )
            },
        )?;
        let persisted_life_record = serde_json::from_value::<LifeRecord>(required_slice(
            "life_record",
        )?)
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "player_cultivation for `{username}` has invalid life_record slice: {error}"
                ),
            )
        })?;
        if persisted_life_record.character_id != life_record.character_id {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "player_cultivation identity mismatch for `{username}`: persisted={} staged={}",
                    persisted_life_record.character_id, life_record.character_id
                ),
            ));
        }
    }

    let staged_cultivation = serde_json::to_value(
        crate::cultivation::components::encode_persisted_cultivation(cultivation),
    )
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    crate::cultivation::components::decode_persisted_cultivation(staged_cultivation.clone())
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("staged revival cultivation is invalid: {error}"),
            )
        })?;
    let staged_meridians = serde_json::to_value(meridians)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    crate::cultivation::legacy_meridian_bundle::decode_meridian_system(
        staged_meridians.clone(),
        crate::cultivation::legacy_meridian_bundle::CURRENT_BUNDLE_VERSION,
    )
    .map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("staged revival meridians are invalid: {error}"),
        )
    })?;
    let staged_contamination = serde_json::to_value(contamination)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    serde_json::from_value::<Contamination>(staged_contamination.clone()).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("staged revival contamination is invalid: {error}"),
        )
    })?;
    let staged_life_record = serde_json::to_value(life_record)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    serde_json::from_value::<LifeRecord>(staged_life_record.clone()).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("staged revival life_record is invalid: {error}"),
        )
    })?;

    object.insert(
        "v".to_string(),
        serde_json::json!(crate::cultivation::legacy_meridian_bundle::CURRENT_BUNDLE_VERSION),
    );
    object.insert("cultivation".to_string(), staged_cultivation);
    object.insert("meridians".to_string(), staged_meridians);
    object.insert("contamination".to_string(), staged_contamination);
    object.insert("life_record".to_string(), staged_life_record);
    serde_json::to_string(&bundle)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

pub(super) fn update_revival_player_cultivation_bundle(
    transaction: &rusqlite::Transaction<'_>,
    username: &str,
    cultivation_json: &str,
    wall_clock: i64,
) -> io::Result<()> {
    let updated = transaction
        .execute(
            "
            UPDATE player_cultivation
            SET cultivation_json = ?2,
                schema_version = ?3,
                last_updated_wall = ?4
            WHERE username = ?1
            ",
            params![
                username,
                cultivation_json,
                CURRENT_SCHEMA_VERSION,
                wall_clock
            ],
        )
        .map_err(io::Error::other)?;
    if updated != 1 {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("revival player_cultivation row disappeared for `{username}`"),
        ));
    }
    Ok(())
}

pub fn persist_revival_transition(
    settings: &PersistenceSettings,
    life_record: &LifeRecord,
) -> io::Result<()> {
    let entry = latest_biography_entry(life_record)?;
    let wall_clock = current_unix_seconds();
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;

    upsert_life_record(&transaction, life_record, wall_clock)?;
    append_life_event(
        &transaction,
        life_record.character_id.as_str(),
        entry,
        wall_clock,
    )?;

    transaction.commit().map_err(io::Error::other)
}

pub fn persist_lifespan_event(
    settings: &PersistenceSettings,
    char_id: &str,
    event: &LifespanEventRecord,
) -> io::Result<()> {
    let wall_clock = current_unix_seconds();
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;

    append_lifespan_event(&transaction, char_id, event, wall_clock)?;

    transaction.commit().map_err(io::Error::other)
}

pub fn persist_life_record_death_insight(
    settings: &PersistenceSettings,
    life_record: &LifeRecord,
) -> io::Result<()> {
    let Some(death_insight) = life_record.death_insights.last() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "life_record must contain at least one death insight before persistence",
        ));
    };

    let wall_clock = current_unix_seconds();
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;

    upsert_life_record(&transaction, life_record, wall_clock)?;
    append_death_insight_event(
        &transaction,
        life_record.character_id.as_str(),
        death_insight,
        wall_clock,
    )?;
    update_deceased_snapshot_life_record(
        &transaction,
        life_record.character_id.as_str(),
        life_record,
        wall_clock,
    )?;

    transaction.commit().map_err(io::Error::other)
}

pub fn persist_termination_transition(
    settings: &PersistenceSettings,
    lifecycle: &Lifecycle,
    life_record: &LifeRecord,
) -> io::Result<()> {
    persist_termination_transition_inner(settings, lifecycle, life_record, None, None, None)
}

pub fn persist_termination_transition_with_death_context(
    settings: &PersistenceSettings,
    lifecycle: &Lifecycle,
    life_record: &LifeRecord,
    death_registry_cause: Option<&str>,
    lifespan_event: Option<&LifespanEventRecord>,
) -> io::Result<()> {
    persist_termination_transition_inner(
        settings,
        lifecycle,
        life_record,
        death_registry_cause,
        lifespan_event,
        None,
    )
}

pub fn persist_npc_termination_with_qi_snapshot(
    settings: &PersistenceSettings,
    lifecycle: &Lifecycle,
    life_record: &LifeRecord,
    death_registry_cause: &str,
    lifespan_event: Option<&LifespanEventRecord>,
    zones: &crate::world::zone::ZoneRegistry,
    qi_ledger: &WorldQiAccount,
) -> io::Result<()> {
    persist_termination_transition_inner(
        settings,
        lifecycle,
        life_record,
        Some(death_registry_cause),
        lifespan_event,
        Some((zones, qi_ledger)),
    )
}

pub(super) fn persist_termination_transition_inner(
    settings: &PersistenceSettings,
    lifecycle: &Lifecycle,
    life_record: &LifeRecord,
    death_registry_cause: Option<&str>,
    lifespan_event: Option<&LifespanEventRecord>,
    qi_snapshot: Option<(&crate::world::zone::ZoneRegistry, &WorldQiAccount)>,
) -> io::Result<()> {
    let entry = latest_biography_entry(life_record)?;
    let wall_clock = current_unix_seconds();
    let died_at_tick = biography_tick(entry);
    let termination_category = termination_category_from_entry(entry);
    let social = load_deceased_social_snapshot(settings, life_record.character_id.as_str())?;
    let snapshot = DeceasedSnapshot {
        char_id: life_record.character_id.clone(),
        died_at_tick,
        termination_category: termination_category.clone(),
        lifecycle: lifecycle.clone(),
        life_record: life_record.clone(),
        social,
    };
    let snapshot_json = serde_json::to_string_pretty(&snapshot)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    upsert_life_record(&transaction, life_record, wall_clock)?;
    append_life_event(
        &transaction,
        life_record.character_id.as_str(),
        entry,
        wall_clock,
    )?;
    if let Some(death_registry_cause) = death_registry_cause {
        upsert_death_registry(
            &transaction,
            life_record.character_id.as_str(),
            lifecycle,
            death_registry_cause,
            wall_clock,
        )?;
    }
    if let Some(lifespan_event) = lifespan_event {
        append_lifespan_event(
            &transaction,
            life_record.character_id.as_str(),
            lifespan_event,
            wall_clock,
        )?;
    }
    if let Some((zones, qi_ledger)) = qi_snapshot {
        persist_zone_runtime_records(&transaction, zones, wall_clock)?;
        upsert_runtime_qi_account_balances(&transaction, qi_ledger, wall_clock)?;
    }
    upsert_deceased_snapshot(
        &transaction,
        life_record.character_id.as_str(),
        snapshot_json.as_str(),
        died_at_tick,
        wall_clock,
    )?;

    transaction.commit().map_err(io::Error::other)
}

pub(super) fn latest_biography_entry(life_record: &LifeRecord) -> io::Result<&BiographyEntry> {
    life_record.biography.last().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "life_record must contain at least one biography entry before persistence",
        )
    })
}

pub(super) fn biography_event_type(entry: &BiographyEntry) -> &'static str {
    match entry {
        BiographyEntry::BreakthroughStarted { .. } => "breakthrough_started",
        BiographyEntry::BreakthroughSucceeded { .. } => "breakthrough_succeeded",
        BiographyEntry::SpiritEyeBreakthrough { .. } => "spirit_eye_breakthrough",
        BiographyEntry::BreakthroughFailed { .. } => "breakthrough_failed",
        BiographyEntry::MeridianOpened { .. } => "meridian_opened",
        BiographyEntry::MeridianClosed { .. } => "meridian_closed",
        BiographyEntry::ForgedRate { .. } => "forged_rate",
        BiographyEntry::ForgedCapacity { .. } => "forged_capacity",
        BiographyEntry::ColorShift { .. } => "color_shift",
        BiographyEntry::InsightTaken { .. } => "insight_taken",
        BiographyEntry::InsightDiverge { .. } => "insight_diverge",
        BiographyEntry::Rebirth { .. } => "rebirth",
        BiographyEntry::CombatHit { .. } => "combat_hit",
        BiographyEntry::DuguPoisonInflicted { .. } => "dugu_poison_inflicted",
        BiographyEntry::JiemaiParry { .. } => "jiemai_parry",
        BiographyEntry::NearDeath { .. } => "near_death",
        BiographyEntry::Terminated { .. } => "terminated",
        BiographyEntry::LifespanExtended { .. } => "lifespan_extended",
        BiographyEntry::DuoShePerformed { .. } => "duoshe_performed",
        BiographyEntry::PossessedBy { .. } => "possessed_by",
        BiographyEntry::AlchemyAttempt { .. } => "alchemy_attempt",
        BiographyEntry::PlotHarvestedByOther { .. } => "plot_harvested_by_other",
        BiographyEntry::PlotHarvestedFromOther { .. } => "plot_harvested_from_other",
        BiographyEntry::PlotQiDrainedByOther { .. } => "plot_qi_drained_by_other",
        BiographyEntry::PlotQiDrainedFromOther { .. } => "plot_qi_drained_from_other",
        BiographyEntry::PlotDestroyedByOther { .. } => "plot_destroyed_by_other",
        BiographyEntry::TribulationIntercepted { .. } => "tribulation_intercepted",
        BiographyEntry::TribulationFled { .. } => "tribulation_fled",
        BiographyEntry::HeartDemonRecord { .. } => "heart_demon_record",
        BiographyEntry::TradeCompleted { .. } => "trade_completed",
        BiographyEntry::PvpEncounter { .. } => "pvp_encounter",
        BiographyEntry::PvpBetrayal { .. } => "pvp_betrayal",
        BiographyEntry::NicheIntrusion { .. } => "niche_intrusion",
        BiographyEntry::VortexProjectileDrained { .. } => "vortex_projectile_drained",
        BiographyEntry::VortexBackfired { .. } => "vortex_backfired",
        BiographyEntry::AnqiSniped { .. } => "anqi_sniped",
        BiographyEntry::FalseSkinShed { .. } => "false_skin_shed",
        BiographyEntry::SpawnTutorialCompleted { .. } => "spawn_tutorial_completed",
        BiographyEntry::VoidAction { .. } => "void_action",
        BiographyEntry::JueBiSurvived { .. } => "jue_bi_survived",
        BiographyEntry::JueBiKilled { .. } => "jue_bi_killed",
        BiographyEntry::MutationAdvanced { .. } => "mutation_advanced",
    }
}

pub(super) fn append_death_insight_event(
    transaction: &rusqlite::Transaction<'_>,
    char_id: &str,
    death_insight: &DeathInsightRecord,
    wall_clock: i64,
) -> io::Result<()> {
    let payload_json = serde_json::to_string(&DeathInsightEventPayload {
        death_insight: death_insight.clone(),
    })
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let event_id = format!(
        "{}:death_insight:{}:{}",
        char_id, death_insight.tick, wall_clock
    );

    transaction
        .execute(
            "
            INSERT OR IGNORE INTO life_events (
                event_id,
                char_id,
                event_type,
                payload_json,
                payload_version,
                game_tick,
                wall_clock,
                schema_version
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ",
            params![
                event_id,
                char_id,
                "death_insight",
                payload_json,
                EVENT_PAYLOAD_VERSION,
                tick_to_sql(death_insight.tick)?,
                wall_clock,
                EVENT_SCHEMA_VERSION
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

pub(super) fn biography_tick(entry: &BiographyEntry) -> u64 {
    match entry {
        BiographyEntry::BreakthroughStarted { tick, .. }
        | BiographyEntry::BreakthroughSucceeded { tick, .. }
        | BiographyEntry::SpiritEyeBreakthrough { tick, .. }
        | BiographyEntry::BreakthroughFailed { tick, .. }
        | BiographyEntry::MeridianOpened { tick, .. }
        | BiographyEntry::MeridianClosed { tick, .. }
        | BiographyEntry::ForgedRate { tick, .. }
        | BiographyEntry::ForgedCapacity { tick, .. }
        | BiographyEntry::ColorShift { tick, .. }
        | BiographyEntry::InsightTaken { tick, .. }
        | BiographyEntry::InsightDiverge { tick, .. }
        | BiographyEntry::Rebirth { tick, .. }
        | BiographyEntry::CombatHit { tick, .. }
        | BiographyEntry::DuguPoisonInflicted { tick, .. }
        | BiographyEntry::JiemaiParry { tick, .. }
        | BiographyEntry::NearDeath { tick, .. }
        | BiographyEntry::Terminated { tick, .. }
        | BiographyEntry::LifespanExtended { tick, .. }
        | BiographyEntry::DuoShePerformed { tick, .. }
        | BiographyEntry::PossessedBy { tick, .. }
        | BiographyEntry::AlchemyAttempt { tick, .. }
        | BiographyEntry::PlotHarvestedByOther { tick, .. }
        | BiographyEntry::PlotHarvestedFromOther { tick, .. }
        | BiographyEntry::PlotQiDrainedByOther { tick, .. }
        | BiographyEntry::PlotQiDrainedFromOther { tick, .. }
        | BiographyEntry::PlotDestroyedByOther { tick, .. }
        | BiographyEntry::TribulationIntercepted { tick, .. }
        | BiographyEntry::TribulationFled { tick, .. }
        | BiographyEntry::HeartDemonRecord { tick, .. }
        | BiographyEntry::TradeCompleted { tick, .. }
        | BiographyEntry::PvpEncounter { tick, .. }
        | BiographyEntry::PvpBetrayal { tick, .. }
        | BiographyEntry::NicheIntrusion { tick, .. }
        | BiographyEntry::VortexProjectileDrained { tick, .. }
        | BiographyEntry::VortexBackfired { tick, .. }
        | BiographyEntry::AnqiSniped { tick, .. }
        | BiographyEntry::FalseSkinShed { tick, .. }
        | BiographyEntry::SpawnTutorialCompleted { tick, .. }
        | BiographyEntry::VoidAction { tick, .. }
        | BiographyEntry::JueBiSurvived { tick, .. }
        | BiographyEntry::JueBiKilled { tick, .. }
        | BiographyEntry::MutationAdvanced { tick, .. } => *tick,
    }
}

pub(super) fn upsert_life_record(
    transaction: &rusqlite::Transaction<'_>,
    life_record: &LifeRecord,
    wall_clock: i64,
) -> io::Result<()> {
    let life_record_json = serde_json::to_string(life_record)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    transaction
        .execute(
            "
            INSERT INTO life_records (
                char_id,
                life_record_json,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(char_id) DO UPDATE SET
                life_record_json = excluded.life_record_json,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                life_record.character_id,
                life_record_json,
                EVENT_SCHEMA_VERSION,
                wall_clock
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

pub(super) fn append_life_event(
    transaction: &rusqlite::Transaction<'_>,
    char_id: &str,
    entry: &BiographyEntry,
    wall_clock: i64,
) -> io::Result<()> {
    let payload_json = serde_json::to_string(&LifeEventPayload {
        biography_entry: entry.clone(),
    })
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    transaction
        .execute(
            "
            INSERT INTO life_events (
                event_id,
                char_id,
                event_type,
                payload_json,
                payload_version,
                game_tick,
                wall_clock,
                schema_version
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ",
            params![
                Uuid::now_v7().to_string(),
                char_id,
                biography_event_type(entry),
                payload_json,
                EVENT_PAYLOAD_VERSION,
                tick_to_sql(biography_tick(entry))?,
                wall_clock,
                EVENT_SCHEMA_VERSION
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

pub(super) fn upsert_death_registry(
    transaction: &rusqlite::Transaction<'_>,
    char_id: &str,
    lifecycle: &Lifecycle,
    cause: &str,
    wall_clock: i64,
) -> io::Result<()> {
    transaction
        .execute(
            "
            INSERT INTO death_registry (
                char_id,
                death_count,
                last_death_tick,
                last_death_cause,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(char_id) DO UPDATE SET
                death_count = excluded.death_count,
                last_death_tick = excluded.last_death_tick,
                last_death_cause = excluded.last_death_cause,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                char_id,
                i64::from(lifecycle.death_count),
                tick_to_sql(lifecycle.last_death_tick.unwrap_or_default())?,
                cause,
                EVENT_SCHEMA_VERSION,
                wall_clock
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

pub(super) fn append_lifespan_event(
    transaction: &rusqlite::Transaction<'_>,
    char_id: &str,
    event: &LifespanEventRecord,
    wall_clock: i64,
) -> io::Result<()> {
    let payload_json = serde_json::to_string(event)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    transaction
        .execute(
            "
            INSERT INTO lifespan_events (
                event_id,
                char_id,
                event_type,
                payload_json,
                payload_version,
                game_tick,
                wall_clock,
                schema_version
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ",
            params![
                Uuid::now_v7().to_string(),
                char_id,
                event.kind,
                payload_json,
                EVENT_PAYLOAD_VERSION,
                tick_to_sql(event.at_tick)?,
                wall_clock,
                EVENT_SCHEMA_VERSION
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

pub(super) fn upsert_deceased_snapshot(
    transaction: &rusqlite::Transaction<'_>,
    char_id: &str,
    snapshot_json: &str,
    died_at_tick: u64,
    wall_clock: i64,
) -> io::Result<()> {
    transaction
        .execute(
            "
            INSERT INTO deceased_snapshots (
                char_id,
                snapshot_json,
                died_at_tick,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(char_id) DO UPDATE SET
                snapshot_json = excluded.snapshot_json,
                died_at_tick = excluded.died_at_tick,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                char_id,
                snapshot_json,
                tick_to_sql(died_at_tick)?,
                EVENT_SCHEMA_VERSION,
                wall_clock
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

pub(super) fn load_deceased_social_snapshot(
    settings: &PersistenceSettings,
    char_id: &str,
) -> io::Result<Option<DeceasedSocialSnapshot>> {
    let connection = open_persistence_connection(settings)?;
    let renown = load_deceased_renown(&connection, char_id)?;
    let relationships = load_deceased_relationships(&connection, char_id)?;
    let exposure_log = load_deceased_exposure_log(&connection, char_id)?;
    let faction_membership = load_deceased_faction_membership(&connection, char_id)?;

    if renown == DeceasedRenownSnapshot::default()
        && relationships.is_empty()
        && exposure_log.is_empty()
        && faction_membership.is_none()
    {
        return Ok(None);
    }

    Ok(Some(DeceasedSocialSnapshot {
        renown,
        relationships,
        exposure_log,
        faction_membership,
    }))
}

pub(super) fn load_deceased_renown(
    connection: &Connection,
    char_id: &str,
) -> io::Result<DeceasedRenownSnapshot> {
    let row: Option<(i32, i32, String)> = connection
        .query_row(
            "SELECT fame, notoriety, tags_json FROM social_renown WHERE char_id = ?1",
            params![char_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(io::Error::other)?;
    let Some((fame, notoriety, tags_json)) = row else {
        return Ok(DeceasedRenownSnapshot::default());
    };
    let tags = serde_json::from_str::<Vec<RenownTagV1>>(tags_json.as_str())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    Ok(DeceasedRenownSnapshot {
        fame,
        notoriety,
        tags,
    })
}

pub(super) fn load_deceased_relationships(
    connection: &Connection,
    char_id: &str,
) -> io::Result<Vec<RelationshipSnapshotV1>> {
    let mut statement = connection
        .prepare(
            "
            SELECT peer_char_id, relationship_type, since_tick, metadata_json
            FROM social_relationships
            WHERE char_id = ?1
            ORDER BY peer_char_id ASC, relationship_type ASC
            ",
        )
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map(params![char_id], |row| {
            let kind_label: String = row.get(1)?;
            let metadata_json: String = row.get(3)?;
            let kind =
                parse_enum_label::<RelationshipKindV1>(kind_label.as_str()).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        1,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
            let metadata = serde_json::from_str(metadata_json.as_str()).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    3,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?;
            Ok(RelationshipSnapshotV1 {
                peer: row.get(0)?,
                kind,
                since_tick: sql_to_tick(row.get::<_, i64>(2)?).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        2,
                        rusqlite::types::Type::Integer,
                        Box::new(error),
                    )
                })?,
                metadata,
            })
        })
        .map_err(io::Error::other)?;

    let mut relationships = Vec::new();
    for row in rows {
        relationships.push(row.map_err(io::Error::other)?);
    }
    Ok(relationships)
}

pub(super) fn load_deceased_exposure_log(
    connection: &Connection,
    char_id: &str,
) -> io::Result<Vec<DeceasedExposureSnapshot>> {
    let mut statement = connection
        .prepare(
            "
            SELECT kind, witnesses_json, at_tick
            FROM social_exposures
            WHERE char_id = ?1
            ORDER BY at_tick ASC, event_id ASC
            ",
        )
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map(params![char_id], |row| {
            let kind_label: String = row.get(0)?;
            let witnesses_json: String = row.get(1)?;
            let kind =
                parse_enum_label::<ExposureKindV1>(kind_label.as_str()).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
            let witnesses = serde_json::from_str(witnesses_json.as_str()).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    1,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?;
            Ok(DeceasedExposureSnapshot {
                kind,
                witnesses,
                tick: sql_to_tick(row.get::<_, i64>(2)?).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        2,
                        rusqlite::types::Type::Integer,
                        Box::new(error),
                    )
                })?,
            })
        })
        .map_err(io::Error::other)?;

    let mut exposure_log = Vec::new();
    for row in rows {
        exposure_log.push(row.map_err(io::Error::other)?);
    }
    Ok(exposure_log)
}

pub(super) fn load_deceased_faction_membership(
    connection: &Connection,
    char_id: &str,
) -> io::Result<Option<FactionMembershipSnapshotV1>> {
    let row: Option<DeceasedFactionMembershipSqlRow> = connection
        .query_row(
            "
            SELECT faction, rank, loyalty, betrayal_count, invite_block_until_tick, permanently_refused
            FROM social_faction_memberships
            WHERE char_id = ?1
            ",
            params![char_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .optional()
        .map_err(io::Error::other)?;
    let Some((
        faction,
        rank,
        loyalty,
        betrayal_count,
        invite_block_until_tick,
        permanently_refused,
    )) = row
    else {
        return Ok(None);
    };

    Ok(Some(FactionMembershipSnapshotV1 {
        faction: faction.unwrap_or_else(|| "neutral".to_string()),
        rank: u8::try_from(rank)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
        loyalty: i32::try_from(loyalty)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
        betrayal_count: u8::try_from(betrayal_count)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
        invite_block_until_tick: invite_block_until_tick.map(sql_to_tick).transpose()?,
        permanently_refused: permanently_refused != 0,
    }))
}

pub(super) fn update_deceased_snapshot_life_record(
    transaction: &rusqlite::Transaction<'_>,
    char_id: &str,
    life_record: &LifeRecord,
    wall_clock: i64,
) -> io::Result<()> {
    let Some(existing_snapshot_json) = transaction
        .query_row(
            "SELECT snapshot_json FROM deceased_snapshots WHERE char_id = ?1",
            params![char_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(io::Error::other)?
    else {
        return Ok(());
    };

    let mut snapshot: DeceasedSnapshot = serde_json::from_str(existing_snapshot_json.as_str())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    snapshot.life_record = life_record.clone();
    let snapshot_json = serde_json::to_string_pretty(&snapshot)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    transaction
        .execute(
            "
            UPDATE deceased_snapshots
            SET snapshot_json = ?2,
                last_updated_wall = ?3
            WHERE char_id = ?1
            ",
            params![char_id, snapshot_json, wall_clock],
        )
        .map_err(io::Error::other)?;
    Ok(())
}
