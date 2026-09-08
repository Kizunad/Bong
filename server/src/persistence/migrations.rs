//! SQLite schema migrations and migration-time compatibility checks.

use super::*;

pub(super) fn apply_migrations(connection: &mut Connection) -> rusqlite::Result<()> {
    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    let initial_version = current_version;
    if current_version > CURRENT_USER_VERSION {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            io::Error::other(format!(
                "sqlite user_version {current_version} is newer than supported {CURRENT_USER_VERSION}; refusing to open without modifying database"
            )),
        )));
    }

    if current_version < 1 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS bootstrap_events (
                event_id TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                game_tick INTEGER NOT NULL CHECK (game_tick >= 0),
                wall_clock INTEGER NOT NULL CHECK (wall_clock >= 0),
                server_run_id TEXT NOT NULL,
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0),
                payload_json TEXT NOT NULL
            );
            PRAGMA user_version = 1;
            ",
        )?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 2 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE INDEX IF NOT EXISTS idx_bootstrap_events_wall_clock
            ON bootstrap_events (wall_clock, event_id);
            PRAGMA user_version = 2;
            ",
        )?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 3 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS player_core (
                username TEXT PRIMARY KEY,
                current_char_id TEXT NOT NULL,
                realm TEXT NOT NULL,
                spirit_qi REAL NOT NULL,
                spirit_qi_max REAL NOT NULL,
                karma REAL NOT NULL,
                experience INTEGER NOT NULL,
                inventory_score REAL NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            CREATE TABLE IF NOT EXISTS player_slow (
                username TEXT PRIMARY KEY,
                pos_x REAL NOT NULL,
                pos_y REAL NOT NULL,
                pos_z REAL NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            CREATE TABLE IF NOT EXISTS inventories (
                username TEXT PRIMARY KEY,
                inventory_json TEXT NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            CREATE TABLE IF NOT EXISTS player_ui_prefs (
                username TEXT PRIMARY KEY,
                prefs_json TEXT NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            PRAGMA user_version = 3;
            ",
        )?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 4 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS life_records (
                char_id TEXT PRIMARY KEY,
                life_record_json TEXT NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            CREATE TABLE IF NOT EXISTS life_events (
                event_id TEXT PRIMARY KEY,
                char_id TEXT NOT NULL,
                event_type TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                payload_version INTEGER NOT NULL CHECK (payload_version >= 1),
                game_tick INTEGER NOT NULL CHECK (game_tick >= 0),
                wall_clock INTEGER NOT NULL CHECK (wall_clock >= 0),
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1)
            );
            CREATE INDEX IF NOT EXISTS idx_life_events_char_tick
            ON life_events (char_id, game_tick, event_id);
            CREATE TABLE IF NOT EXISTS death_registry (
                char_id TEXT PRIMARY KEY,
                death_count INTEGER NOT NULL CHECK (death_count >= 0),
                last_death_tick INTEGER NOT NULL CHECK (last_death_tick >= 0),
                last_death_cause TEXT NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            CREATE TABLE IF NOT EXISTS lifespan_events (
                event_id TEXT PRIMARY KEY,
                char_id TEXT NOT NULL,
                event_type TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                payload_version INTEGER NOT NULL CHECK (payload_version >= 1),
                game_tick INTEGER NOT NULL CHECK (game_tick >= 0),
                wall_clock INTEGER NOT NULL CHECK (wall_clock >= 0),
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1)
            );
            CREATE INDEX IF NOT EXISTS idx_lifespan_events_char_tick
            ON lifespan_events (char_id, game_tick, event_id);
            CREATE TABLE IF NOT EXISTS deceased_snapshots (
                char_id TEXT PRIMARY KEY,
                snapshot_json TEXT NOT NULL,
                died_at_tick INTEGER NOT NULL CHECK (died_at_tick >= 0),
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            PRAGMA user_version = 4;
            ",
        )?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 5 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS npc_state (
                char_id TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                archetype TEXT NOT NULL,
                pos_x REAL NOT NULL,
                pos_y REAL NOT NULL,
                pos_z REAL NOT NULL,
                state TEXT NOT NULL,
                blackboard_json TEXT NOT NULL,
                home_zone TEXT NOT NULL,
                patrol_anchor_index INTEGER NOT NULL CHECK (patrol_anchor_index >= 0),
                patrol_target_x REAL NOT NULL,
                patrol_target_y REAL NOT NULL,
                patrol_target_z REAL NOT NULL,
                movement_mode TEXT NOT NULL,
                can_sprint INTEGER NOT NULL,
                can_dash INTEGER NOT NULL,
                sprint_ready_at INTEGER NOT NULL CHECK (sprint_ready_at >= 0),
                dash_ready_at INTEGER NOT NULL CHECK (dash_ready_at >= 0),
                lifecycle_state TEXT NOT NULL,
                death_count INTEGER NOT NULL CHECK (death_count >= 0),
                last_death_tick INTEGER,
                last_revive_tick INTEGER,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            CREATE TABLE IF NOT EXISTS npc_digests (
                char_id TEXT PRIMARY KEY,
                archetype TEXT NOT NULL,
                realm TEXT NOT NULL,
                faction_id TEXT,
                recent_summary TEXT NOT NULL,
                last_referenced_wall INTEGER NOT NULL CHECK (last_referenced_wall >= 0),
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            CREATE INDEX IF NOT EXISTS idx_npc_digests_last_referenced_wall
            ON npc_digests (last_referenced_wall, char_id);
            CREATE TABLE IF NOT EXISTS factions (
                faction_id TEXT PRIMARY KEY,
                display_name TEXT NOT NULL,
                doctrine TEXT NOT NULL,
                metadata_json TEXT NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            CREATE TABLE IF NOT EXISTS reputation (
                faction_id TEXT NOT NULL,
                target_faction_id TEXT NOT NULL,
                score INTEGER NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0),
                PRIMARY KEY (faction_id, target_faction_id)
            );
            CREATE TABLE IF NOT EXISTS membership (
                faction_id TEXT NOT NULL,
                char_id TEXT NOT NULL,
                role TEXT NOT NULL,
                joined_at_tick INTEGER NOT NULL CHECK (joined_at_tick >= 0),
                metadata_json TEXT NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0),
                PRIMARY KEY (faction_id, char_id)
            );
            CREATE TABLE IF NOT EXISTS relationships (
                char_id TEXT NOT NULL,
                peer_char_id TEXT NOT NULL,
                relationship_type TEXT NOT NULL,
                since_tick INTEGER NOT NULL CHECK (since_tick >= 0),
                metadata_json TEXT NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0),
                PRIMARY KEY (char_id, peer_char_id, relationship_type)
            );
            CREATE TABLE IF NOT EXISTS archetype_registry (
                char_id TEXT NOT NULL,
                archetype TEXT NOT NULL,
                since_tick INTEGER NOT NULL CHECK (since_tick >= 0),
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0),
                PRIMARY KEY (char_id, since_tick, archetype)
            );
            CREATE INDEX IF NOT EXISTS idx_archetype_registry_char_tick
            ON archetype_registry (char_id, since_tick, archetype);
            CREATE TABLE IF NOT EXISTS npc_deceased_index (
                char_id TEXT PRIMARY KEY,
                archetype TEXT NOT NULL,
                died_at_tick INTEGER NOT NULL CHECK (died_at_tick >= 0),
                path TEXT NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            PRAGMA user_version = 5;
            ",
        )?;
        transaction.commit()?;
    }

    ensure_agent_world_model_table(connection)?;

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 6 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS tribulations_active (
                char_id TEXT PRIMARY KEY,
                wave_current INTEGER NOT NULL CHECK (wave_current >= 0),
                waves_total INTEGER NOT NULL CHECK (waves_total > 0),
                started_tick INTEGER NOT NULL CHECK (started_tick >= 0),
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            PRAGMA user_version = 6;
            ",
        )?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 7 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS ascension_quota (
                row_id INTEGER PRIMARY KEY CHECK (row_id = 1),
                occupied_slots INTEGER NOT NULL CHECK (occupied_slots >= 0),
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            PRAGMA user_version = 7;
            ",
        )?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 8 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS zones_runtime (
                zone_id TEXT PRIMARY KEY,
                spirit_qi REAL NOT NULL,
                danger_level INTEGER NOT NULL CHECK (danger_level >= 0),
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            PRAGMA user_version = 8;
            ",
        )?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 9 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS zone_overlays (
                zone_id TEXT NOT NULL,
                overlay_kind TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                since_wall INTEGER NOT NULL CHECK (since_wall >= 0),
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0),
                PRIMARY KEY (zone_id, overlay_kind, since_wall)
            );
            PRAGMA user_version = 9;
            ",
        )?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 10 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            ALTER TABLE zone_overlays
            ADD COLUMN payload_version INTEGER NOT NULL DEFAULT 1 CHECK (payload_version >= 1);
            PRAGMA user_version = 10;
            ",
        )?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 11 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS agent_eras (
                event_id TEXT PRIMARY KEY,
                envelope_id TEXT NOT NULL,
                source TEXT NOT NULL,
                era_name TEXT NOT NULL,
                since_tick INTEGER NOT NULL,
                global_effect TEXT NOT NULL,
                observed_at_tick INTEGER,
                observed_at_wall INTEGER NOT NULL CHECK (observed_at_wall >= 0),
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1)
            );
            CREATE INDEX IF NOT EXISTS idx_agent_eras_envelope_id
            ON agent_eras (envelope_id, observed_at_wall, event_id);
            CREATE TABLE IF NOT EXISTS agent_decisions (
                event_id TEXT PRIMARY KEY,
                envelope_id TEXT NOT NULL,
                source TEXT NOT NULL,
                agent_name TEXT NOT NULL,
                reasoning TEXT NOT NULL,
                command_count INTEGER NOT NULL CHECK (command_count >= 0),
                narration_count INTEGER NOT NULL CHECK (narration_count >= 0),
                payload_json TEXT NOT NULL,
                observed_at_tick INTEGER,
                observed_at_wall INTEGER NOT NULL CHECK (observed_at_wall >= 0),
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1)
            );
            CREATE INDEX IF NOT EXISTS idx_agent_decisions_envelope_agent
            ON agent_decisions (envelope_id, agent_name, observed_at_wall, event_id);
            PRAGMA user_version = 11;
            ",
        )?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 12 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS player_lifespan (
                username TEXT PRIMARY KEY,
                born_at_tick INTEGER NOT NULL CHECK (born_at_tick >= 0),
                years_lived REAL NOT NULL CHECK (years_lived >= 0),
                cap_by_realm INTEGER NOT NULL CHECK (cap_by_realm > 0),
                offline_pause_wall INTEGER NOT NULL CHECK (offline_pause_wall >= 0),
                in_coffin INTEGER NOT NULL DEFAULT 0 CHECK (in_coffin IN (0, 1)),
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            CREATE TABLE IF NOT EXISTS player_skills (
                username TEXT PRIMARY KEY,
                skill_set_json TEXT NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            PRAGMA user_version = 12;
            ",
        )?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 13 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS player_shrine (
                username TEXT PRIMARY KEY,
                anchor_x REAL NOT NULL,
                anchor_y REAL NOT NULL,
                anchor_z REAL NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            ",
        )?;
        let has_column: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('player_slow') WHERE name = 'last_dimension'",
            [],
            |row| row.get(0),
        )?;
        if has_column == 0 {
            transaction.execute_batch(
                "
                ALTER TABLE player_slow
                ADD COLUMN last_dimension TEXT NOT NULL DEFAULT 'overworld'
                CHECK (last_dimension IN ('overworld', 'tsy'));
                ",
            )?;
        }
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS player_cultivation (
                username TEXT PRIMARY KEY,
                cultivation_json TEXT NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            ",
        )?;

        // SQLite might already have a pruned player_core schema (e.g. older
        // dev databases). Drop columns only when they exist.
        let player_core_columns: Vec<String> = {
            let mut stmt = transaction.prepare("PRAGMA table_info(player_core)")?;
            let columns = stmt
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<Result<Vec<_>, _>>()?;
            columns
        };
        backfill_legacy_player_cultivation(&transaction, &player_core_columns)?;
        for legacy_col in ["realm", "spirit_qi", "spirit_qi_max", "experience"] {
            if player_core_columns.iter().any(|name| name == legacy_col) {
                transaction.execute(
                    &format!("ALTER TABLE player_core DROP COLUMN {legacy_col}"),
                    [],
                )?;
            }
        }

        transaction.execute_batch("PRAGMA user_version = 13;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 14 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS social_anonymity (
                char_id TEXT PRIMARY KEY,
                displayed_name TEXT,
                exposed_to_json TEXT NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            CREATE TABLE IF NOT EXISTS social_relationships (
                char_id TEXT NOT NULL,
                peer_char_id TEXT NOT NULL,
                relationship_type TEXT NOT NULL,
                since_tick INTEGER NOT NULL CHECK (since_tick >= 0),
                metadata_json TEXT NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0),
                PRIMARY KEY (char_id, peer_char_id, relationship_type)
            );
            CREATE TABLE IF NOT EXISTS social_exposures (
                event_id TEXT PRIMARY KEY,
                char_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                witnesses_json TEXT NOT NULL,
                at_tick INTEGER NOT NULL CHECK (at_tick >= 0),
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            CREATE INDEX IF NOT EXISTS idx_social_exposures_char_tick
            ON social_exposures (char_id, at_tick, event_id);
            CREATE TABLE IF NOT EXISTS social_renown (
                char_id TEXT PRIMARY KEY,
                fame INTEGER NOT NULL,
                notoriety INTEGER NOT NULL,
                tags_json TEXT NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            CREATE TABLE IF NOT EXISTS social_spirit_niches (
                owner TEXT PRIMARY KEY,
                pos_x INTEGER NOT NULL,
                pos_y INTEGER NOT NULL,
                pos_z INTEGER NOT NULL,
                placed_at_tick INTEGER NOT NULL CHECK (placed_at_tick >= 0),
                revealed INTEGER NOT NULL CHECK (revealed IN (0, 1)),
                revealed_by TEXT,
                is_damaged INTEGER NOT NULL DEFAULT 0 CHECK (is_damaged IN (0, 1)),
                defense_mode TEXT,
                guardians_json TEXT NOT NULL DEFAULT '[]',
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            PRAGMA user_version = 14;
            ",
        )?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 15 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS social_faction_memberships (
                char_id TEXT PRIMARY KEY,
                faction TEXT,
                named_faction TEXT,
                rank INTEGER NOT NULL CHECK (rank >= 0),
                loyalty INTEGER NOT NULL,
                betrayal_count INTEGER NOT NULL CHECK (betrayal_count >= 0),
                invite_block_until_tick INTEGER,
                permanently_refused INTEGER NOT NULL CHECK (permanently_refused IN (0, 1)),
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            PRAGMA user_version = 15;
            ",
        )?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 16 {
        let transaction = connection.transaction()?;
        let columns = table_columns(&transaction, "social_spirit_niches")?;
        if !columns.iter().any(|column| column == "guardians_json") {
            transaction.execute_batch(
                "
                ALTER TABLE social_spirit_niches
                ADD COLUMN guardians_json TEXT NOT NULL DEFAULT '[]';
                ",
            )?;
        }
        transaction.execute_batch("PRAGMA user_version = 16;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 17 {
        let transaction = connection.transaction()?;
        identity::migrate_v17(&transaction)?;
        // 防 user_version 升级但表不存在的 silent regression：在 PRAGMA 前显式 assert
        let table_exists: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'player_identities'",
            [],
            |row| row.get(0),
        )?;
        if table_exists != 1 {
            return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                io::Error::other("v17 migration completed but player_identities table missing"),
            )));
        }
        transaction.execute_batch("PRAGMA user_version = 17;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 19 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS void_action_cooldowns (
                character_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                ready_at_tick INTEGER NOT NULL CHECK (ready_at_tick >= 0),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0),
                PRIMARY KEY (character_id, kind)
            );
            ",
        )?;
        assert_void_action_cooldowns_schema_ready(&transaction)?;
        transaction.execute_batch("PRAGMA user_version = 19;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 20 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS high_renown_milestones (
                player_uuid TEXT NOT NULL,
                char_id TEXT NOT NULL,
                identity_id INTEGER NOT NULL CHECK (identity_id >= 0),
                milestone INTEGER NOT NULL CHECK (milestone >= 0),
                emitted_at_tick INTEGER NOT NULL CHECK (emitted_at_tick >= 0),
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0),
                PRIMARY KEY (player_uuid, identity_id, milestone)
            );
            CREATE INDEX IF NOT EXISTS idx_high_renown_milestones_char
            ON high_renown_milestones (char_id, identity_id, milestone);
            ",
        )?;
        assert_high_renown_milestones_schema_ready(&transaction)?;
        transaction.execute_batch("PRAGMA user_version = 20;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 21 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS tribulations_active (
                char_id TEXT PRIMARY KEY,
                kind TEXT NOT NULL DEFAULT 'du_xu',
                source TEXT NOT NULL DEFAULT '',
                wave_current INTEGER NOT NULL CHECK (wave_current >= 0),
                waves_total INTEGER NOT NULL CHECK (waves_total > 0),
                started_tick INTEGER NOT NULL CHECK (started_tick >= 0),
                epicenter_x REAL NOT NULL DEFAULT 0.0,
                epicenter_y REAL NOT NULL DEFAULT 64.0,
                epicenter_z REAL NOT NULL DEFAULT 0.0,
                intensity REAL NOT NULL DEFAULT 0.0,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            ",
        )?;
        let columns = table_columns(&transaction, "tribulations_active")?;
        if !columns.iter().any(|column| column == "kind") {
            transaction.execute_batch(
                "
                ALTER TABLE tribulations_active
                ADD COLUMN kind TEXT NOT NULL DEFAULT 'du_xu';
                ",
            )?;
        }
        if !columns.iter().any(|column| column == "source") {
            transaction.execute_batch(
                "
                ALTER TABLE tribulations_active
                ADD COLUMN source TEXT NOT NULL DEFAULT '';
                ",
            )?;
        }
        if !columns.iter().any(|column| column == "epicenter_x") {
            transaction.execute_batch(
                "
                ALTER TABLE tribulations_active
                ADD COLUMN epicenter_x REAL NOT NULL DEFAULT 0.0;
                ",
            )?;
        }
        if !columns.iter().any(|column| column == "epicenter_y") {
            transaction.execute_batch(
                "
                ALTER TABLE tribulations_active
                ADD COLUMN epicenter_y REAL NOT NULL DEFAULT 64.0;
                ",
            )?;
        }
        if !columns.iter().any(|column| column == "epicenter_z") {
            transaction.execute_batch(
                "
                ALTER TABLE tribulations_active
                ADD COLUMN epicenter_z REAL NOT NULL DEFAULT 0.0;
                ",
            )?;
        }
        if !columns.iter().any(|column| column == "intensity") {
            transaction.execute_batch(
                "
                ALTER TABLE tribulations_active
                ADD COLUMN intensity REAL NOT NULL DEFAULT 0.0;
                ",
            )?;
        }
        transaction.execute_batch("PRAGMA user_version = 21;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 22 {
        let transaction = connection.transaction()?;
        let columns = table_columns(&transaction, "tribulations_active")?;
        if !columns.iter().any(|column| column == "origin_dimension") {
            transaction.execute_batch(
                "
                ALTER TABLE tribulations_active
                ADD COLUMN origin_dimension TEXT;
                ",
            )?;
        }
        transaction.execute_batch("PRAGMA user_version = 22;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 23 {
        let transaction = connection.transaction()?;
        let columns = table_columns(&transaction, "player_lifespan")?;
        if columns.is_empty() {
            transaction.execute_batch(
                "
                CREATE TABLE IF NOT EXISTS player_lifespan (
                    username TEXT PRIMARY KEY,
                    born_at_tick INTEGER NOT NULL CHECK (born_at_tick >= 0),
                    years_lived REAL NOT NULL CHECK (years_lived >= 0),
                    cap_by_realm INTEGER NOT NULL CHECK (cap_by_realm > 0),
                    offline_pause_wall INTEGER NOT NULL CHECK (offline_pause_wall >= 0),
                    in_coffin INTEGER NOT NULL DEFAULT 0 CHECK (in_coffin IN (0, 1)),
                    schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                    last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
                );
                ",
            )?;
        } else if !columns.iter().any(|column| column == "in_coffin") {
            transaction.execute_batch(
                "
                ALTER TABLE player_lifespan
                ADD COLUMN in_coffin INTEGER NOT NULL DEFAULT 0 CHECK (in_coffin IN (0, 1));
                ",
            )?;
        }
        transaction.execute_batch("PRAGMA user_version = 23;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 24 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS spirit_treasure_world (
                template_id TEXT PRIMARY KEY,
                instance_id INTEGER NOT NULL CHECK (instance_id >= 0),
                holder_kind TEXT NOT NULL CHECK (holder_kind IN ('player', 'ground', 'lost')),
                holder_id TEXT,
                holder_pos_x REAL,
                holder_pos_y REAL,
                holder_pos_z REAL,
                affinity REAL NOT NULL DEFAULT 0.5 CHECK (affinity >= 0.0 AND affinity <= 1.0),
                dialogue_count INTEGER NOT NULL DEFAULT 0 CHECK (dialogue_count >= 0),
                sleeping INTEGER NOT NULL DEFAULT 0 CHECK (sleeping IN (0, 1)),
                spawned_at_tick INTEGER NOT NULL CHECK (spawned_at_tick >= 0),
                schema_version INTEGER NOT NULL DEFAULT 1 CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL DEFAULT 0 CHECK (last_updated_wall >= 0)
            );

            CREATE TABLE IF NOT EXISTS spirit_treasure_dialogue_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                template_id TEXT NOT NULL,
                character_id TEXT NOT NULL,
                tick INTEGER NOT NULL CHECK (tick >= 0),
                speaker TEXT NOT NULL CHECK (speaker IN ('player', 'spirit')),
                content TEXT NOT NULL,
                affinity_delta REAL NOT NULL DEFAULT 0.0,
                schema_version INTEGER NOT NULL DEFAULT 1 CHECK (schema_version >= 1)
            );

            CREATE INDEX IF NOT EXISTS idx_spirit_treasure_dialogue_log_character
            ON spirit_treasure_dialogue_log (character_id, template_id, tick);
            ",
        )?;
        assert_spirit_treasure_schema_ready(&transaction)?;
        transaction.execute_batch("PRAGMA user_version = 24;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 25 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS player_known_techniques (
                username TEXT PRIMARY KEY,
                known_techniques_json TEXT NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            ",
        )?;
        assert_player_known_techniques_schema_ready(&transaction)?;
        transaction.execute_batch("PRAGMA user_version = 25;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 26 {
        // plan-offscreen-war-v1 P3：克制式战场遗物（deferred-on-hydrate）的待物化持久层。
        // 不走 worldgen 静态布局（战场 chunk 未加载），改用 sqlite + TTL sweep（§10.1 #6
        // 决议：选 sqlite 而非 Resource，避免遗物与已 remove 的死者生命周期耦合成孤儿）。
        // relic_id = UUID TEXT PK（同 char_id 款）；position 拆三列 REAL（同 npc_state pos_x/y/z）；
        // loot_seed 是 u64 deterministic 种子，sqlite 无 u64 → 以 i64 位投影存（读回再投影回来）；
        // created_tick 给 deferred-on-hydrate 时序校验；created_wall 给 TTL sweep（墙钟，不依赖逻辑 tick）。
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS pending_dormant_relics (
                relic_id     TEXT PRIMARY KEY,
                char_id      TEXT NOT NULL,
                zone         TEXT NOT NULL,
                pos_x        REAL NOT NULL,
                pos_y        REAL NOT NULL,
                pos_z        REAL NOT NULL,
                archetype    TEXT NOT NULL,
                loot_seed    INTEGER NOT NULL,
                created_tick INTEGER NOT NULL CHECK (created_tick >= 0),
                created_wall INTEGER NOT NULL CHECK (created_wall >= 0),
                schema_version INTEGER NOT NULL DEFAULT 1 CHECK (schema_version >= 1)
            );
            CREATE INDEX IF NOT EXISTS idx_pending_dormant_relics_zone
            ON pending_dormant_relics (zone, created_wall);
            CREATE INDEX IF NOT EXISTS idx_pending_dormant_relics_created_wall
            ON pending_dormant_relics (created_wall);
            ",
        )?;
        assert_pending_dormant_relics_schema_ready(&transaction)?;
        transaction.execute_batch("PRAGMA user_version = 26;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 27 {
        // plan-coffin-tiers-v1 P0：player_lifespan 加 coffin_grade 列（TEXT，default 'mundane'）
        let transaction = connection.transaction()?;
        let columns = table_columns(&transaction, "player_lifespan")?;
        if !columns.is_empty() && !columns.iter().any(|col| col == "coffin_grade") {
            transaction.execute_batch(
                "
                ALTER TABLE player_lifespan
                ADD COLUMN coffin_grade TEXT NOT NULL DEFAULT 'mundane';
                ",
            )?;
        }
        transaction.execute_batch("PRAGMA user_version = 27;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 28 {
        // plan-niche-craft-fix-v1 P1：灵龛本体增加单一受损态。
        let transaction = connection.transaction()?;
        let columns = table_columns(&transaction, "social_spirit_niches")?;
        if !columns.is_empty() && !columns.iter().any(|col| col == "is_damaged") {
            transaction.execute_batch(
                "
                ALTER TABLE social_spirit_niches
                ADD COLUMN is_damaged INTEGER NOT NULL DEFAULT 0 CHECK (is_damaged IN (0, 1));
                ",
            )?;
        }
        transaction.execute_batch("PRAGMA user_version = 28;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 29 {
        // plan-territory-v1 P0：区域影响力持久化表（zone_influence）。
        // key=(zone_id, char_id)；dominant=1 标记当前霸主行。
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS zone_influence (
                zone_id               TEXT    NOT NULL,
                char_id               TEXT    NOT NULL,
                value                 REAL    NOT NULL DEFAULT 0.0,
                meditation_ticks      INTEGER NOT NULL DEFAULT 0,
                combat_wins           INTEGER NOT NULL DEFAULT 0,
                player_kills          INTEGER NOT NULL DEFAULT 0,
                gather_count          INTEGER NOT NULL DEFAULT 0,
                continuous_sessions   INTEGER NOT NULL DEFAULT 0,
                last_activity_tick    INTEGER NOT NULL DEFAULT 0,
                dominant              INTEGER NOT NULL DEFAULT 0,
                established_tick      INTEGER NOT NULL DEFAULT 0,
                public_known          INTEGER NOT NULL DEFAULT 0,
                schema_version        INTEGER NOT NULL DEFAULT 1,
                last_updated_wall     INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (zone_id, char_id)
            );
            CREATE INDEX IF NOT EXISTS idx_zone_influence_zone_id
            ON zone_influence (zone_id);
            ",
        )?;
        transaction.execute_batch("PRAGMA user_version = 29;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 30 {
        // plan-faction-expansion-v1 P0：social_faction_memberships.named_faction 列迁移。
        //
        // 迁移目标：social_faction_memberships.faction 列存 "attack"/"defend"/"neutral"（v15 建，
        // FactionId 真持久化数据）→ 按 zone_anchor 归属回填到具名势力。
        // 映射依据：
        //   attack  → qingyun_hunters    （青云外门猎杀型主动出击）
        //   defend  → cangyuan_merchants  （血谷守矿型防守）
        //   neutral → north_waste_drifters（流窜无归属→北荒漂流者）
        //   此为 P0 既有数据合理 zone_anchor 归属，非凭空映射；正典依据见 NamedFactionId 注释。
        //
        // (a) 新增 named_faction 列（保留旧 faction 列不破坏 social-v2 读路径）。
        // (b) 三条 UPDATE 按 faction 值回填 named_faction（WHERE named_faction IS NULL 防幂等覆盖）。
        // (c) 表不存在时跳过列/UPDATE（早期 fixture 库，social v15 之前），直接升版本。
        let transaction = connection.transaction()?;
        let existing_columns = table_columns(&transaction, "social_faction_memberships")?;
        if !existing_columns.is_empty() {
            // 表存在：(a) 按需加列，(b) 三条 UPDATE 回填。
            if !existing_columns.iter().any(|c| c == "named_faction") {
                transaction.execute_batch(
                    "ALTER TABLE social_faction_memberships ADD COLUMN named_faction TEXT;",
                )?;
            }
            transaction.execute_batch(
                "
                UPDATE social_faction_memberships
                SET named_faction = 'qingyun_hunters'
                WHERE faction = 'attack' AND named_faction IS NULL;

                UPDATE social_faction_memberships
                SET named_faction = 'cangyuan_merchants'
                WHERE faction = 'defend' AND named_faction IS NULL;

                UPDATE social_faction_memberships
                SET named_faction = 'north_waste_drifters'
                WHERE faction = 'neutral' AND named_faction IS NULL;
                ",
            )?;
        }
        // 无论表是否存在，均升版本号（前置 migration 会在 v15 建表，本 migration 幂等）。
        transaction.execute_batch("PRAGMA user_version = 30;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 31 {
        // plan-life-record-epitaph-v1 P0：碑刻持久化表。
        //
        // epitaph_id TEXT PRIMARY KEY（UUID v7，时序可排序）；
        // entry_json TEXT NOT NULL（EpitaphEntry serde_json 全量序列化）；
        // death_tick / schema_version / last_updated_wall 便于运维查询与版本迁移。
        // 永久保留语义：内存 WorldEpitaphRegistry 超 cap 淘汰时不删除本表行。
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS epitaphs (
                epitaph_id          TEXT PRIMARY KEY,
                character_id        TEXT NOT NULL,
                entry_json          TEXT NOT NULL,
                death_tick          INTEGER NOT NULL CHECK (death_tick >= 0),
                schema_version      INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall   INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            CREATE INDEX IF NOT EXISTS idx_epitaphs_character_id
            ON epitaphs (character_id);
            PRAGMA user_version = 31;
            ",
        )?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 32 {
        // plan-faction-expansion-v1 P3：玩家 ↔ 具名势力声望持久化。
        //
        // key=(char_id, named_faction)，score 取 plan P3 的 -100..=100 区间；
        // 不挂到 social_faction_memberships，避免把“当前挂靠”与“历史信誉”混为一列。
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS social_faction_reputations (
                char_id             TEXT    NOT NULL,
                named_faction       TEXT    NOT NULL,
                score               INTEGER NOT NULL CHECK (score >= -100 AND score <= 100),
                schema_version      INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall   INTEGER NOT NULL CHECK (last_updated_wall >= 0),
                PRIMARY KEY (char_id, named_faction)
            );
            CREATE INDEX IF NOT EXISTS idx_social_faction_reputations_char_id
            ON social_faction_reputations (char_id);
            ",
        )?;
        assert_social_faction_reputations_schema_ready(&transaction)?;
        transaction.execute_batch("PRAGMA user_version = 32;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 33 {
        // plan-bughunt-ao-worldgen-state-pseudo-vein-restart-loss-v1：
        // heartbeat 生成的伪灵脉是动态 zone + lifecycle 双状态，不能只靠 zones_runtime 三列恢复。
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS heartbeat_pseudo_veins (
                zone_id             TEXT PRIMARY KEY,
                dimension           TEXT NOT NULL CHECK (dimension IN ('overworld', 'tsy')),
                min_x               REAL NOT NULL,
                min_y               REAL NOT NULL,
                min_z               REAL NOT NULL,
                max_x               REAL NOT NULL,
                max_y               REAL NOT NULL,
                max_z               REAL NOT NULL,
                danger_level        INTEGER NOT NULL CHECK (danger_level >= 0),
                active_events_json  TEXT NOT NULL,
                patrol_anchors_json TEXT NOT NULL,
                center_x            REAL NOT NULL,
                center_z            REAL NOT NULL,
                spawned_at_tick     INTEGER NOT NULL CHECK (spawned_at_tick >= 0),
                last_tick           INTEGER NOT NULL CHECK (last_tick >= 0),
                qi_current          REAL NOT NULL,
                total_qi_consumed   REAL NOT NULL,
                warning_sent        INTEGER NOT NULL CHECK (warning_sent IN (0, 1)),
                dissipated          INTEGER NOT NULL CHECK (dissipated IN (0, 1)),
                season_at_spawn     TEXT NOT NULL CHECK (
                    season_at_spawn IN (
                        'summer',
                        'summer_to_winter',
                        'winter',
                        'winter_to_summer'
                    )
                ),
                schema_version      INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall   INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            PRAGMA user_version = 33;
            ",
        )?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 34 {
        // pending inflow 没有对应 ECS component / zone 字段可在重启时重建，必须单独
        // 落盘；其余 WorldQiAccount 条目仍由各自物理权威字段恢复，不持久化审计轨迹。
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS qi_runtime_accounts (
                account_id         TEXT PRIMARY KEY,
                balance            REAL NOT NULL CHECK (balance >= 0),
                schema_version     INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall  INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            ",
        )?;
        // 只有全新 v0 数据库能证明历史 pending inflow 必为 0。v33 升级库没有
        // 旧账本可供重建，故意保留缺行，Startup 会 fail-closed，禁止把未知余额
        // 静默解释为 0 并在首帧覆盖。
        if initial_version == 0 {
            transaction.execute(
                "
                INSERT INTO qi_runtime_accounts (
                    account_id, balance, schema_version, last_updated_wall
                ) VALUES (?1, 0.0, ?2, 0)
                ON CONFLICT(account_id) DO NOTHING
                ",
                params![PENDING_INFLOW_ACCOUNT_ID, CURRENT_SCHEMA_VERSION],
            )?;
        }
        transaction.execute_batch("PRAGMA user_version = 34;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 35 {
        let transaction = connection.transaction()?;
        for (column, definition) in [
            (
                "observed_age_ticks",
                "INTEGER NOT NULL DEFAULT 0 CHECK (observed_age_ticks >= 0)",
            ),
            (
                "pending_runtime_ticks",
                "INTEGER NOT NULL DEFAULT 0 CHECK (pending_runtime_ticks >= 0)",
            ),
            (
                "pending_offline_ticks",
                "INTEGER NOT NULL DEFAULT 0 CHECK (pending_offline_ticks >= 0)",
            ),
            (
                "occupant_count",
                "INTEGER NOT NULL DEFAULT 0 CHECK (occupant_count >= 0)",
            ),
            (
                "eval_elapsed_ticks",
                "INTEGER NOT NULL DEFAULT 0 CHECK (eval_elapsed_ticks >= 0)",
            ),
        ] {
            let columns = table_columns(&transaction, "heartbeat_pseudo_veins")?;
            if !columns.iter().any(|existing| existing == column) {
                transaction.execute_batch(&format!(
                    "ALTER TABLE heartbeat_pseudo_veins ADD COLUMN {column} {definition};"
                ))?;
            }
        }
        // v33/v34 没有保存快照发生在 heartbeat 200-tick 周期中的精确位置。
        // 迁移时按最保守的 interval-1 回填，宁可最多提早 199 tick，也不延长生命周期。
        let conservative_elapsed = HEARTBEAT_EVAL_INTERVAL_TICKS.saturating_sub(1);
        transaction.execute(
            "
            UPDATE heartbeat_pseudo_veins
            SET observed_age_ticks =
                    CASE
                        WHEN last_tick >= spawned_at_tick
                        THEN last_tick - spawned_at_tick + ?1
                        ELSE ?1
                    END,
                pending_runtime_ticks = ?1,
                pending_offline_ticks = 0,
                occupant_count = 0,
                eval_elapsed_ticks = ?1
            ",
            params![i64::try_from(conservative_elapsed).unwrap_or(i64::MAX)],
        )?;
        transaction.execute_batch("PRAGMA user_version = 35;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 36 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS player_craft_sessions (
                username          TEXT PRIMARY KEY,
                session_json      TEXT NOT NULL,
                schema_version    INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            PRAGMA user_version = 36;
            ",
        )?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 37 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS dropped_loot (
                instance_id       INTEGER PRIMARY KEY CHECK (instance_id >= 0),
                entry_json        TEXT NOT NULL,
                schema_version    INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
            );
            PRAGMA user_version = 37;
            ",
        )?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 38 {
        let transaction = connection.transaction()?;
        // 两个垂死大能 overflow 池在 v38 首次成为稳定持久账户；旧版本从未有可恢复的
        // 聚合余额，因此升级时显式初始化为已知 0。pending inflow 仍保留 v34 的严格
        // unknown 语义，绝不在这里补零。
        for account_id in [
            DYING_ELDER_DAN_EXCESS_ACCOUNT_ID,
            DYING_ELDER_RELEASE_OVERFLOW_ACCOUNT_ID,
        ] {
            transaction.execute(
                "
                INSERT INTO qi_runtime_accounts (
                    account_id, balance, schema_version, last_updated_wall
                ) VALUES (?1, 0.0, ?2, 0)
                ON CONFLICT(account_id) DO NOTHING
                ",
                params![account_id, CURRENT_SCHEMA_VERSION],
            )?;
        }
        transaction.execute_batch("PRAGMA user_version = 38;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 39 {
        let transaction = connection.transaction()?;
        // bughunt player-lifecycle-relog-death-consequence-wipe：`combat::components::
        // Lifecycle`（死亡/复活状态机：state/fortune_remaining/awaiting_decision/各 deadline
        // tick）此前从未落盘，断线重连时 `attach_combat_bundle_to_joined_clients` 只能盲插
        // `Lifecycle::default()`，把 NearDeath/AwaitingRevival 玩家的运气次数与渡劫决策全部
        // 抹回满状态"新角色"。单 JSON 列镜像整个组件（同 `player_known_techniques` 的
        // `known_techniques_json` 模式），键仍是 `username`（与其余 per-character slice 表一致，
        // 代表"当前存活角色"）。
        //
        // `combat_clock_tick_at_save` 是 OPUS 返工要求的跨重启锚点：`CombatClock` 每次进程
        // 重启都从 0 重新计数（`combat::mod::register` 里 `insert_resource(CombatClock::
        // default())`），而 `near_death_deadline_tick`/`revival_decision_deadline_tick`/
        // `weakened_until_tick` 都是"绝对 tick"——落盘时刻的 CombatClock.tick 值。跨重启直接
        // 复用这些绝对值毫无意义（新进程 tick=0 时，旧 deadline 动辄百万级，等价于几十小时后
        // 才会被 near_death_tick/auto_confirm_revival_decisions 结算，期间玩家卡在
        // AwaitingRevival 无敌状态）。这里把落盘时刻的 CombatClock.tick 一并记录，配合既有的
        // `last_updated_wall`，在读档时按真实流逝墙钟秒数把 deadline 折算到读档当刻的 tick
        // 空间（见 `player::state::translate_lifecycle_deadline_tick_across_restart`），
        // 镜像 `player_lifespan.offline_pause_wall` 的按墙钟折算模式。
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS player_lifecycle (
                username TEXT PRIMARY KEY,
                lifecycle_json TEXT NOT NULL,
                schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
                last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0),
                combat_clock_tick_at_save INTEGER NOT NULL DEFAULT 0
            );
            PRAGMA user_version = 39;
            ",
        )?;
        transaction.commit()?;
    }

    if current_version < 40 {
        let transaction = connection.transaction()?;
        // R5 的固定 overflow 池此前已经承载真实余额，但没有对应 ECS/zone 字段可从旧
        // 存档重建；从此 migration 起以已知 0 建立行，之后每次 snapshot/hydrate 都走
        // 与其它稳定 runtime pool 相同的完整 whitelist。
        transaction.execute(
            "
            INSERT INTO qi_runtime_accounts (
                account_id, balance, schema_version, last_updated_wall
            ) VALUES (?1, 0.0, ?2, 0)
            ON CONFLICT(account_id) DO NOTHING
            ",
            params![QI_FLOW_OVERFLOW_ACCOUNT_ID, CURRENT_SCHEMA_VERSION],
        )?;
        transaction.execute_batch("PRAGMA user_version = 40;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 41 {
        let transaction = connection.transaction()?;
        transaction.execute(
            "
            INSERT INTO qi_runtime_accounts (
                account_id, balance, schema_version, last_updated_wall
            ) VALUES (?1, 0.0, ?2, 0)
            ON CONFLICT(account_id) DO NOTHING
            ",
            params![RIFT_DRAIN_ACCOUNT_ID, CURRENT_SCHEMA_VERSION],
        )?;
        transaction.execute_batch("PRAGMA user_version = 41;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 42 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS dormant_terminal_commits (
                char_id                 TEXT PRIMARY KEY,
                cause                   TEXT NOT NULL,
                at_tick                 INTEGER NOT NULL CHECK (at_tick >= 0),
                zone                    TEXT NOT NULL,
                winner                  TEXT,
                winner_group            INTEGER,
                loser_group             INTEGER,
                zone_accepted           REAL NOT NULL CHECK (zone_accepted >= 0),
                cleanup_revision        INTEGER CHECK (cleanup_revision >= 0),
                created_wall            INTEGER NOT NULL CHECK (created_wall >= 0),
                schema_version          INTEGER NOT NULL CHECK (schema_version >= 1)
            );
            ",
        )?;
        assert_dormant_terminal_commits_schema_ready(&transaction)?;
        transaction.execute_batch("PRAGMA user_version = 42;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 43 {
        let transaction = connection.transaction()?;
        assert_dormant_terminal_commits_schema_ready(&transaction)?;
        if table_exists(&transaction, "deceased_snapshots")? {
            let columns = table_columns(&transaction, "deceased_snapshots")?;
            if columns.iter().any(|column| column == "public_path") {
                transaction
                    .execute_batch("ALTER TABLE deceased_snapshots DROP COLUMN public_path;")?;
            }
            assert_deceased_snapshots_schema_ready(&transaction)?;
        }
        transaction.execute_batch("PRAGMA user_version = 43;")?;
        transaction.commit()?;
    }

    let current_version: i32 =
        connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if current_version < 44 {
        let transaction = connection.transaction()?;
        // 传承死信箱已完全退役；旧数据库中的表和索引必须一并删除。
        transaction.execute_batch(
            "
            DROP INDEX IF EXISTS idx_legacy_letterbox_inheritor;
            DROP TABLE IF EXISTS legacy_letterbox;
            PRAGMA user_version = 44;
            ",
        )?;
        transaction.commit()?;
    }

    let deceased_schema_transaction = connection.transaction()?;
    if table_exists(&deceased_schema_transaction, "deceased_snapshots")? {
        assert_deceased_snapshots_schema_ready(&deceased_schema_transaction)?;
    }
    deceased_schema_transaction.commit()?;

    let terminal_schema_transaction = connection.transaction()?;
    assert_dormant_terminal_commits_schema_ready(&terminal_schema_transaction)?;
    terminal_schema_transaction.commit()?;

    let final_version: i32 = connection.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    if final_version != CURRENT_USER_VERSION {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            io::Error::other(format!(
                "sqlite user_version mismatch after migrations: expected {}, got {}",
                CURRENT_USER_VERSION, final_version
            )),
        )));
    }

    Ok(())
}

pub(super) fn backfill_legacy_player_cultivation(
    transaction: &rusqlite::Transaction<'_>,
    player_core_columns: &[String],
) -> rusqlite::Result<()> {
    let has_column = |column: &str| player_core_columns.iter().any(|name| name == column);
    if !(has_column("username")
        && has_column("realm")
        && has_column("spirit_qi")
        && has_column("spirit_qi_max"))
    {
        return Ok(());
    }

    let legacy_rows = {
        let mut stmt = transaction.prepare(
            "
            SELECT username, realm, spirit_qi, spirit_qi_max
            FROM player_core
            ",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, f64>(2)?,
                row.get::<_, f64>(3)?,
            ))
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    let wall_clock = current_unix_seconds();
    for (username, realm, spirit_qi, spirit_qi_max) in legacy_rows {
        let mut cultivation = Cultivation::default();
        if let Some(restored_realm) = legacy_player_realm_to_cultivation(realm.as_str()) {
            cultivation.realm = restored_realm;
        }
        if spirit_qi.is_finite() {
            cultivation.qi_current = spirit_qi.max(0.0);
        }
        if spirit_qi_max.is_finite() && spirit_qi_max > 0.0 {
            cultivation.qi_max = spirit_qi_max;
        }

        let persisted_cultivation =
            crate::cultivation::components::encode_persisted_cultivation(&cultivation);
        let bundle = serde_json::json!({
            // plan-race-system-v1 P1a：写入的是当前形态 `MeridianSystem::default()`
            // （snake_case channel id），必须标当前 bundle 版本号，否则加载时会误走
            // legacy 迁移分支去解析本就不是 legacy 形态的数据。
            "v": crate::cultivation::legacy_meridian_bundle::CURRENT_BUNDLE_VERSION,
            "cultivation": persisted_cultivation,
            "meridians": crate::cultivation::components::MeridianSystem::default(),
            "qi_color": crate::cultivation::components::QiColor::default(),
            "karma": crate::cultivation::components::Karma::default(),
            "contamination": crate::cultivation::components::Contamination::default(),
            "life_record": crate::cultivation::life_record::LifeRecord::new(
                canonical_player_id(username.as_str()),
            ),
            "practice_log": crate::cultivation::color::PracticeLog::default(),
            "insight_quota": crate::cultivation::insight::InsightQuota::default(),
            "unlocked_perceptions": crate::cultivation::insight_apply::UnlockedPerceptions::default(),
            "insight_modifiers": crate::cultivation::insight_apply::InsightModifiers::new(),
        });
        let cultivation_json = serde_json::to_string(&bundle)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;

        transaction.execute(
            "
            INSERT INTO player_cultivation (
                username,
                cultivation_json,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(username) DO NOTHING
            ",
            params![
                username,
                cultivation_json,
                CURRENT_SCHEMA_VERSION,
                wall_clock,
            ],
        )?;
    }

    Ok(())
}

pub(super) fn table_columns(
    transaction: &rusqlite::Transaction<'_>,
    table: &str,
) -> rusqlite::Result<Vec<String>> {
    let mut statement = transaction.prepare(&format!("PRAGMA table_info({table})"))?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub(super) fn table_exists(
    transaction: &rusqlite::Transaction<'_>,
    table: &str,
) -> rusqlite::Result<bool> {
    transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        params![table],
        |row| row.get(0),
    )
}

pub(super) fn assert_spirit_treasure_schema_ready(
    transaction: &rusqlite::Transaction<'_>,
) -> rusqlite::Result<()> {
    for (table, required) in [
        (
            "spirit_treasure_world",
            &[
                "template_id",
                "instance_id",
                "holder_kind",
                "holder_id",
                "holder_pos_x",
                "holder_pos_y",
                "holder_pos_z",
                "affinity",
                "dialogue_count",
                "sleeping",
                "spawned_at_tick",
                "schema_version",
                "last_updated_wall",
            ][..],
        ),
        (
            "spirit_treasure_dialogue_log",
            &[
                "id",
                "template_id",
                "character_id",
                "tick",
                "speaker",
                "content",
                "affinity_delta",
                "schema_version",
            ][..],
        ),
    ] {
        let columns = table_columns(transaction, table)?;
        if let Some(missing) = required
            .iter()
            .find(|column| !columns.iter().any(|candidate| candidate == *column))
        {
            return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                io::Error::other(format!("{table} missing required column {missing}")),
            )));
        }
    }

    Ok(())
}

pub(super) fn assert_player_known_techniques_schema_ready(
    transaction: &rusqlite::Transaction<'_>,
) -> rusqlite::Result<()> {
    let columns = table_columns(transaction, "player_known_techniques")?;
    let required = [
        "username",
        "known_techniques_json",
        "schema_version",
        "last_updated_wall",
    ];
    if let Some(missing) = required
        .iter()
        .find(|column| !columns.iter().any(|name| name == **column))
    {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            io::Error::other(format!(
                "v25 migration completed but player_known_techniques column {missing} missing"
            )),
        )));
    }

    let mut statement = transaction.prepare("PRAGMA table_info(player_known_techniques)")?;
    let primary_key = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, i32>(5)?))
        })?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|(_, pk_ordinal)| *pk_ordinal > 0)
        .collect::<Vec<_>>();
    let expected_primary_key = [("username".to_owned(), 1)];
    if primary_key.as_slice() != expected_primary_key.as_slice() {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            io::Error::other(format!(
                "v25 migration completed but player_known_techniques primary key mismatch: expected username got {primary_key:?}"
            )),
        )));
    }
    Ok(())
}

/// plan-offscreen-war-v1 P3：v26 迁移后验，确保 `pending_dormant_relics` 列与 PK 完整。
/// 仿 [`assert_player_known_techniques_schema_ready`]——迁移完成但列 / PK 漂移则直接报错，
/// 不让一个残缺表静默上线（遗物 upsert/load 会在运行时撞列名错误反而更难定位）。
pub(super) fn assert_pending_dormant_relics_schema_ready(
    transaction: &rusqlite::Transaction<'_>,
) -> rusqlite::Result<()> {
    let columns = table_columns(transaction, "pending_dormant_relics")?;
    let required = [
        "relic_id",
        "char_id",
        "zone",
        "pos_x",
        "pos_y",
        "pos_z",
        "archetype",
        "loot_seed",
        "created_tick",
        "created_wall",
        "schema_version",
    ];
    if let Some(missing) = required
        .iter()
        .find(|column| !columns.iter().any(|name| name == **column))
    {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            io::Error::other(format!(
                "v26 migration completed but pending_dormant_relics column {missing} missing"
            )),
        )));
    }

    let mut statement = transaction.prepare("PRAGMA table_info(pending_dormant_relics)")?;
    let primary_key = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, i32>(5)?))
        })?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|(_, pk_ordinal)| *pk_ordinal > 0)
        .collect::<Vec<_>>();
    let expected_primary_key = [("relic_id".to_owned(), 1)];
    if primary_key.as_slice() != expected_primary_key.as_slice() {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            io::Error::other(format!(
                "v26 migration completed but pending_dormant_relics primary key mismatch: expected relic_id got {primary_key:?}"
            )),
        )));
    }
    Ok(())
}

pub(super) fn assert_dormant_terminal_commits_schema_ready(
    transaction: &rusqlite::Transaction<'_>,
) -> rusqlite::Result<()> {
    let columns = table_columns(transaction, "dormant_terminal_commits")?;
    let required = [
        "char_id",
        "cause",
        "at_tick",
        "zone",
        "winner",
        "winner_group",
        "loser_group",
        "zone_accepted",
        "cleanup_revision",
        "created_wall",
        "schema_version",
    ];
    if let Some(missing) = required
        .iter()
        .find(|column| !columns.iter().any(|name| name == **column))
    {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            io::Error::other(format!(
                "v42 migration completed but dormant_terminal_commits column {missing} missing"
            )),
        )));
    }

    let mut statement = transaction.prepare("PRAGMA table_info(dormant_terminal_commits)")?;
    let primary_key = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, i32>(5)?))
        })?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|(_, pk_ordinal)| *pk_ordinal > 0)
        .collect::<Vec<_>>();
    let expected_primary_key = [("char_id".to_owned(), 1)];
    if primary_key.as_slice() != expected_primary_key.as_slice() {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            io::Error::other(format!(
                "v42 migration completed but dormant_terminal_commits primary key mismatch: expected char_id got {primary_key:?}"
            )),
        )));
    }
    Ok(())
}

pub(super) fn assert_deceased_snapshots_schema_ready(
    transaction: &rusqlite::Transaction<'_>,
) -> rusqlite::Result<()> {
    let columns = table_columns(transaction, "deceased_snapshots")?;
    let required = [
        "char_id",
        "snapshot_json",
        "died_at_tick",
        "schema_version",
        "last_updated_wall",
    ];
    if let Some(missing) = required
        .iter()
        .find(|column| !columns.iter().any(|candidate| candidate == *column))
    {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            io::Error::other(format!(
                "v43 migration completed but deceased_snapshots column {missing} missing"
            )),
        )));
    }
    if columns.iter().any(|column| column == "public_path") {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            io::Error::other(
                "v43 migration completed but retired deceased_snapshots.public_path remains",
            ),
        )));
    }
    Ok(())
}

pub(super) fn assert_social_faction_reputations_schema_ready(
    transaction: &rusqlite::Transaction<'_>,
) -> rusqlite::Result<()> {
    let columns = table_columns(transaction, "social_faction_reputations")?;
    let required = [
        "char_id",
        "named_faction",
        "score",
        "schema_version",
        "last_updated_wall",
    ];
    if let Some(missing) = required
        .iter()
        .find(|column| !columns.iter().any(|name| name == **column))
    {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            io::Error::other(format!(
                "v32 migration completed but social_faction_reputations column {missing} missing"
            )),
        )));
    }

    let mut statement = transaction.prepare("PRAGMA table_info(social_faction_reputations)")?;
    let table_info = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(1)?,
                row.get::<_, i32>(3)?,
                row.get::<_, i32>(5)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let primary_key = table_info
        .iter()
        .map(|(name, _, pk_ordinal)| (name.clone(), *pk_ordinal))
        .filter(|(_, pk_ordinal)| *pk_ordinal > 0)
        .collect::<Vec<_>>();
    let expected_primary_key = [("char_id".to_owned(), 1), ("named_faction".to_owned(), 2)];
    if primary_key.as_slice() != expected_primary_key.as_slice() {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            io::Error::other(format!(
                "v32 migration completed but social_faction_reputations primary key mismatch: expected (char_id, named_faction) got {primary_key:?}"
            )),
        )));
    }
    for required_not_null in ["char_id", "named_faction"] {
        let is_not_null = table_info
            .iter()
            .find(|(name, _, _)| name == required_not_null)
            .map(|(_, not_null, _)| *not_null != 0)
            .unwrap_or(false);
        if !is_not_null {
            return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                io::Error::other(format!(
                    "v32 migration completed but social_faction_reputations column {required_not_null} must be NOT NULL"
                )),
            )));
        }
    }

    let create_sql: Option<String> = transaction
        .query_row(
            "
            SELECT sql
            FROM sqlite_master
            WHERE type = 'table' AND name = 'social_faction_reputations'
            ",
            [],
            |row| row.get(0),
        )
        .optional()?;
    let Some(create_sql) = create_sql else {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            io::Error::other(
                "v32 migration completed but social_faction_reputations table missing",
            ),
        )));
    };
    for required_check in [
        "score >= -100",
        "score <= 100",
        "schema_version >= 1",
        "last_updated_wall >= 0",
    ] {
        if !create_sql.contains(required_check) {
            return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                io::Error::other(format!(
                    "v32 migration completed but social_faction_reputations CHECK `{required_check}` missing"
                )),
            )));
        }
    }

    Ok(())
}

pub(super) fn assert_void_action_cooldowns_schema_ready(
    transaction: &rusqlite::Transaction<'_>,
) -> rusqlite::Result<()> {
    let columns = table_columns(transaction, "void_action_cooldowns")?;
    let required = ["character_id", "kind", "ready_at_tick", "last_updated_wall"];
    if let Some(missing) = required
        .iter()
        .find(|column| !columns.iter().any(|name| name == **column))
    {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            io::Error::other(format!(
                "v19 migration completed but void_action_cooldowns column {missing} missing"
            )),
        )));
    }
    let mut statement = transaction.prepare("PRAGMA table_info(void_action_cooldowns)")?;
    let primary_key = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, i32>(5)?))
        })?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|(_, pk_ordinal)| *pk_ordinal > 0)
        .collect::<Vec<_>>();
    let expected_primary_key = [("character_id".to_owned(), 1), ("kind".to_owned(), 2)];
    if primary_key.as_slice() != expected_primary_key.as_slice() {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            io::Error::other(format!(
                "v19 migration completed but void_action_cooldowns primary key mismatch: expected character_id,kind got {primary_key:?}"
            )),
        )));
    }
    Ok(())
}

pub(super) fn assert_high_renown_milestones_schema_ready(
    transaction: &rusqlite::Transaction<'_>,
) -> rusqlite::Result<()> {
    let columns = table_columns(transaction, "high_renown_milestones")?;
    let required = [
        "player_uuid",
        "char_id",
        "identity_id",
        "milestone",
        "emitted_at_tick",
        "schema_version",
        "last_updated_wall",
    ];
    if let Some(missing) = required
        .iter()
        .find(|column| !columns.iter().any(|name| name == **column))
    {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            io::Error::other(format!(
                "v20 migration completed but high_renown_milestones column {missing} missing"
            )),
        )));
    }
    let index_exists: i64 = transaction.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_high_renown_milestones_char'",
        [],
        |row| row.get(0),
    )?;
    if index_exists != 1 {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            io::Error::other("v20 migration completed but high_renown_milestones index missing"),
        )));
    }
    let mut statement = transaction.prepare("PRAGMA table_info(high_renown_milestones)")?;
    let primary_key = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, i32>(5)?))
        })?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|(_, pk_ordinal)| *pk_ordinal > 0)
        .collect::<Vec<_>>();
    let expected_primary_key = [
        ("player_uuid".to_owned(), 1),
        ("identity_id".to_owned(), 2),
        ("milestone".to_owned(), 3),
    ];
    if primary_key.as_slice() != expected_primary_key.as_slice() {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            io::Error::other(format!(
                "v20 migration completed but high_renown_milestones primary key mismatch: expected player_uuid,identity_id,milestone got {primary_key:?}"
            )),
        )));
    }
    Ok(())
}
