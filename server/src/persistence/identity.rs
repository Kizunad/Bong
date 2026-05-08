//! `player_identities` SQLite 表与 save / load helpers（plan-identity-v1 P0）。
//!
//! 表 schema 在 `persistence/mod.rs` 的 v17 migration 块创建；本文件提供：
//! - [`migrate_v17`]：表创建（在 mod.rs 的 migration 链中调用，bump user_version=17）
//! - [`save_player_identities`] / [`load_player_identities`]：单玩家 slice 读写
//!
//! 表结构：
//! ```sql
//! CREATE TABLE IF NOT EXISTS player_identities (
//!     char_id TEXT PRIMARY KEY,
//!     identities_json TEXT NOT NULL,           -- serde_json(Vec<IdentityProfile>)
//!     active_identity_id INTEGER NOT NULL CHECK (active_identity_id >= 0),
//!     last_switch_tick INTEGER NOT NULL CHECK (last_switch_tick >= 0),
//!     schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
//!     last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
//! );
//! ```

use std::io;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, OptionalExtension};

use crate::identity::{IdentityId, IdentityProfile, PlayerIdentities};
use crate::persistence::{open_persistence_connection, PersistenceSettings};

const IDENTITY_ROW_SCHEMA_VERSION: i32 = 1;

fn current_unix_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

/// v17 migration：建 `player_identities` 表（在 `persistence::mod` 的 migration 链中调用）。
pub(crate) fn migrate_v17(transaction: &rusqlite::Transaction<'_>) -> rusqlite::Result<()> {
    transaction.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS player_identities (
            char_id TEXT PRIMARY KEY,
            identities_json TEXT NOT NULL,
            active_identity_id INTEGER NOT NULL CHECK (active_identity_id >= 0),
            last_switch_tick INTEGER NOT NULL CHECK (last_switch_tick >= 0),
            schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
            last_updated_wall INTEGER NOT NULL CHECK (last_updated_wall >= 0)
        );
        ",
    )?;
    Ok(())
}

/// 写入 / 更新单玩家的 identity 集合。
///
/// `char_id` 用 [`crate::player::state::canonical_player_id`] 计算（`offline:<username>`）。
pub fn save_player_identities(
    settings: &PersistenceSettings,
    char_id: &str,
    identities: &PlayerIdentities,
) -> io::Result<()> {
    let identities_json = serde_json::to_string(&identities.identities)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    transaction
        .execute(
            "
            INSERT INTO player_identities (
                char_id,
                identities_json,
                active_identity_id,
                last_switch_tick,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(char_id) DO UPDATE SET
                identities_json = excluded.identities_json,
                active_identity_id = excluded.active_identity_id,
                last_switch_tick = excluded.last_switch_tick,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                char_id,
                identities_json,
                identities.active_identity_id.0,
                identities.last_switch_tick,
                IDENTITY_ROW_SCHEMA_VERSION,
                current_unix_seconds(),
            ],
        )
        .map_err(io::Error::other)?;
    transaction.commit().map_err(io::Error::other)?;
    Ok(())
}

/// 读单玩家的 identity 集合；不存在 → `Ok(None)`（让调用方走默认创建）。
pub fn load_player_identities(
    settings: &PersistenceSettings,
    char_id: &str,
) -> io::Result<Option<PlayerIdentities>> {
    let connection = open_persistence_connection(settings)?;
    let row = connection
        .query_row(
            "
            SELECT identities_json, active_identity_id, last_switch_tick
            FROM player_identities
            WHERE char_id = ?1
            ",
            params![char_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .optional()
        .map_err(io::Error::other)?;

    let Some((identities_json, active_id_raw, last_switch_tick_raw)) = row else {
        return Ok(None);
    };

    let identities: Vec<IdentityProfile> = serde_json::from_str(&identities_json)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    if identities.is_empty() {
        // 损坏行（有 row 但 list 为空）→ 不当作"已加载"，让调用方重建默认。
        return Ok(None);
    }

    let requested_active_id = IdentityId(active_id_raw.max(0) as u32);
    // 校验 active_identity_id 在 identities 列表里——脏数据下回退到首个可用 identity
    // 避免 PlayerIdentities::active() 返回 None 后调用方误判为"无 identity"。
    let active_identity_id = if identities.iter().any(|p| p.id == requested_active_id) {
        requested_active_id
    } else {
        identities[0].id
    };
    let last_switch_tick = last_switch_tick_raw.max(0) as u64;

    Ok(Some(PlayerIdentities {
        identities,
        active_identity_id,
        last_switch_tick,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::Realm;
    use crate::identity::{RevealedTag, RevealedTagKind};
    use crate::persistence::{bootstrap_sqlite, PersistenceSettings};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_db_path() -> PathBuf {
        let pid = std::process::id();
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!("bong-identity-test-{pid}-{counter}.db"))
    }

    fn fresh_settings() -> PersistenceSettings {
        let path = temp_db_path();
        let _ = std::fs::remove_file(&path);
        let parent = path.parent().expect("temp parent");
        let settings = PersistenceSettings::with_paths(
            path.clone(),
            parent.join("deceased"),
            "test-identity-run".to_string(),
        );
        bootstrap_sqlite(&path, "test-identity-run").expect("bootstrap sqlite");
        settings
    }

    #[test]
    fn save_then_load_round_trips() {
        let settings = fresh_settings();
        let mut pid = PlayerIdentities::with_default("kiz", 100);
        pid.identities
            .push(IdentityProfile::new(IdentityId(1), "alt", 200));
        pid.identities[1].renown.fame = 50;
        pid.identities[1].revealed_tags.push(RevealedTag {
            kind: RevealedTagKind::DuguRevealed,
            witnessed_at_tick: 250,
            witness_realm: Realm::Spirit,
            permanent: true,
        });
        pid.last_switch_tick = 12_345;
        pid.active_identity_id = IdentityId(1);

        save_player_identities(&settings, "offline:kiz", &pid).expect("save");
        let loaded = load_player_identities(&settings, "offline:kiz")
            .expect("load")
            .expect("Some");
        assert_eq!(loaded, pid);
    }

    #[test]
    fn load_returns_none_when_row_missing() {
        let settings = fresh_settings();
        let loaded = load_player_identities(&settings, "offline:nobody").expect("load");
        assert!(loaded.is_none());
    }

    #[test]
    fn load_falls_back_to_first_id_when_active_id_invalid() {
        // 防回归：脏数据 active_identity_id 不在 identities 列表 → 回退到 identities[0].id
        // 而不是返回不一致的 PlayerIdentities（CodeRabbit Major 反馈）。
        let settings = fresh_settings();
        let mut pid = PlayerIdentities::with_default("kiz", 0);
        pid.identities
            .push(IdentityProfile::new(IdentityId(7), "alt", 100));
        pid.active_identity_id = IdentityId(99); // 故意写一个不存在的 id
        save_player_identities(&settings, "offline:kiz", &pid).expect("save");

        let loaded = load_player_identities(&settings, "offline:kiz")
            .expect("load")
            .expect("Some");
        assert!(
            loaded
                .identities
                .iter()
                .any(|p| p.id == loaded.active_identity_id),
            "load 后 active_identity_id 必须是 identities 中真实存在的 id"
        );
        assert_eq!(
            loaded.active_identity_id,
            IdentityId(0),
            "回退到 identities[0].id"
        );
    }

    #[test]
    fn save_overwrites_existing_row() {
        let settings = fresh_settings();
        let pid_v1 = PlayerIdentities::with_default("kiz", 0);
        save_player_identities(&settings, "offline:kiz", &pid_v1).expect("save v1");

        let mut pid_v2 = PlayerIdentities::with_default("kiz", 0);
        pid_v2.identities[0].renown.fame = 999;
        save_player_identities(&settings, "offline:kiz", &pid_v2).expect("save v2");

        let loaded = load_player_identities(&settings, "offline:kiz")
            .expect("load")
            .expect("Some");
        assert_eq!(loaded.identities[0].renown.fame, 999);
    }
}
