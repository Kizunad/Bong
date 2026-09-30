//! R3 持久化侧的 checkpoint / reconnect guard seam。
//!
//! 本模块只负责把 R1 生成的 Suspended checkpoint、一次性 `ReconnectGuard` 和
//! R6/R2 消费的 `CraftRestoreGuard` 放进同一个 SQLite 事务；不拥有 session
//! reducer、control-frame 传输或 client store 状态。

use super::*;

use crate::player::state::{open_player_connection, PlayerStatePersistence};

const SESSION_CHECKPOINT_SCHEMA_VERSION: i32 = 1;
type StoredCraftRestoreGuardRow = (String, i64, i64, String, String);

/// S-07 在 durable commit 时生成的一次性恢复能力。
///
/// `generation` 与 `phase_revision` 绑定 checkpoint 的版本；调用方不得只凭
/// `owner_key` 或 `session_key` 恢复。`restore_token` 只用于一次 guarded restore，
/// 成功消费后不能再次匹配。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReconnectGuard {
    pub owner_key: String,
    pub session_key: String,
    pub generation: u64,
    pub phase_revision: u64,
    pub restore_token: String,
}

/// 下发给 client bridge 的独立 control frame 内容。
///
/// 该类型与 `ReconnectGuard` 保持字段逐字一致，但故意分成两个类型，避免
/// client 侧把 server-side capability 当成可自行生成的恢复凭证。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CraftRestoreGuard {
    pub owner_key: String,
    pub session_key: String,
    pub generation: u64,
    pub phase_revision: u64,
    pub restore_token: String,
}

impl From<&ReconnectGuard> for CraftRestoreGuard {
    fn from(guard: &ReconnectGuard) -> Self {
        Self {
            owner_key: guard.owner_key.clone(),
            session_key: guard.session_key.clone(),
            generation: guard.generation,
            phase_revision: guard.phase_revision,
            restore_token: guard.restore_token.clone(),
        }
    }
}

/// S-07 的 Suspended durable snapshot。
///
/// `checkpoint_json` 是 R1 session owner 生成的 canonical snapshot；R3 不解析其中
/// 的 gameplay 字段，也不把运行期 Entity 当成持久化身份。需要重新绑定工作台时，
/// 只使用稳定的 `placed_id`，不保存 runtime `workbench_key`。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SuspendedSessionCheckpoint {
    pub owner_key: String,
    pub session_key: String,
    pub generation: u64,
    pub phase_revision: u64,
    pub placed_id: Option<String>,
    pub checkpoint_json: String,
}

/// checkpoint、server-side guard 与待发送 control frame 的一致读视图。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuspendedSessionBundle {
    pub checkpoint: SuspendedSessionCheckpoint,
    pub reconnect_guard: ReconnectGuard,
    pub craft_restore_guard: CraftRestoreGuard,
}

/// 持久化 checkpoint 时的版本裁决结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuspendedCheckpointPersistOutcome {
    /// 该 session 首次落盘。
    Inserted,
    /// 新 snapshot 的 generation/revision 不低于当前行，已替换旧 snapshot。
    Updated,
    /// 现有行更新更快，旧 checkpoint 被保留。
    IgnoredStale,
}

/// 把 Suspended checkpoint 与两个 guard 投影放进同一个 SQLite 事务。
///
/// 事务提交前任何一张表失败都会整体回滚，不能出现“checkpoint 已写而 control
/// frame 缺失”的半状态。R1/R6 尚未接入生产 reducer/transport 时，也只能通过
/// 这个 seam 写入，禁止各自另开连接造成双写。
pub fn persist_suspended_session_checkpoint(
    persistence: &PlayerStatePersistence,
    checkpoint: &SuspendedSessionCheckpoint,
    reconnect_guard: &ReconnectGuard,
) -> io::Result<SuspendedCheckpointPersistOutcome> {
    validate_checkpoint_and_guard(checkpoint, reconnect_guard)?;
    let mut connection = open_player_connection(persistence)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    let outcome = persist_suspended_session_checkpoint_in_transaction(
        &transaction,
        checkpoint,
        reconnect_guard,
    )?;
    transaction.commit().map_err(io::Error::other)?;
    Ok(outcome)
}

/// 在已有玩家/库存/制作 checkpoint 事务中写入 S-07/M-12 投影。
///
/// 该函数保持 `pub(crate)`，让 R1 的 session adapter 能把 gameplay snapshot、
/// inventory/escrow 与 guard 绑定到同一个 `rusqlite::Transaction`，而不会把
/// `Connection` ownership 暴露给 gameplay。
pub(crate) fn persist_suspended_session_checkpoint_in_transaction(
    transaction: &rusqlite::Transaction<'_>,
    checkpoint: &SuspendedSessionCheckpoint,
    reconnect_guard: &ReconnectGuard,
) -> io::Result<SuspendedCheckpointPersistOutcome> {
    validate_checkpoint_and_guard(checkpoint, reconnect_guard)?;
    let generation = sqlite_u64("generation", reconnect_guard.generation)?;
    let phase_revision = sqlite_u64("phase_revision", reconnect_guard.phase_revision)?;
    let existing = load_existing_checkpoint_version(transaction, &reconnect_guard.session_key)?;
    let was_existing = existing.is_some();
    if let Some((existing_owner, existing_generation, existing_revision)) = existing {
        if existing_owner != reconnect_guard.owner_key {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "session `{}` already belongs to `{existing_owner}`, refusing owner `{}`",
                    reconnect_guard.session_key, reconnect_guard.owner_key
                ),
            ));
        }
        if is_stale_version(
            existing_generation,
            existing_revision,
            generation,
            phase_revision,
        ) {
            return Ok(SuspendedCheckpointPersistOutcome::IgnoredStale);
        }
    }

    let last_updated_wall = current_unix_seconds();
    upsert_checkpoint_row(
        transaction,
        checkpoint,
        reconnect_guard,
        generation,
        phase_revision,
        last_updated_wall,
    )?;
    upsert_craft_restore_guard_row(
        transaction,
        reconnect_guard,
        generation,
        phase_revision,
        last_updated_wall,
    )?;

    Ok(if was_existing {
        SuspendedCheckpointPersistOutcome::Updated
    } else {
        SuspendedCheckpointPersistOutcome::Inserted
    })
}

/// 读取仍带有未消费 guard 的 Suspended checkpoint。
///
/// checkpoint 存在但 guard 缺失或 frame 字段不一致时返回错误，而不是把损坏状态
/// 当成“没有 session”；这样重连路径会 fail closed，交给上层保留旧 durable 行。
pub fn load_suspended_session_bundle(
    persistence: &PlayerStatePersistence,
    session_key: &str,
) -> io::Result<Option<SuspendedSessionBundle>> {
    validate_non_empty_key("session_key", session_key)?;
    let connection = open_player_connection(persistence)?;
    let checkpoint = load_checkpoint_row(&connection, session_key)?;
    let Some(checkpoint) = checkpoint else {
        return Ok(None);
    };

    let guard_row = load_guard_row(&connection, session_key)?.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Suspended checkpoint `{session_key}` has no reconnect guard"),
        )
    })?;

    let (owner_key, generation, phase_revision, restore_token, frame_json) = guard_row;
    let reconnect_guard = ReconnectGuard {
        owner_key,
        session_key: session_key.to_string(),
        generation: u64_from_sql("generation", generation).map_err(io::Error::other)?,
        phase_revision: u64_from_sql("phase_revision", phase_revision).map_err(io::Error::other)?,
        restore_token,
    };
    validate_checkpoint_and_guard(&checkpoint, &reconnect_guard)?;
    let craft_restore_guard: CraftRestoreGuard = serde_json::from_str(&frame_json)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if craft_restore_guard != CraftRestoreGuard::from(&reconnect_guard) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("CraftRestoreGuard `{session_key}` does not match reconnect guard"),
        ));
    }

    Ok(Some(SuspendedSessionBundle {
        checkpoint,
        reconnect_guard,
        craft_restore_guard,
    }))
}

fn load_existing_checkpoint_version(
    transaction: &rusqlite::Transaction<'_>,
    session_key: &str,
) -> io::Result<Option<(String, i64, i64)>> {
    transaction
        .query_row(
            "
            SELECT owner_key, generation, phase_revision
            FROM suspended_session_checkpoints
            WHERE session_key = ?1
            ",
            params![session_key],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .optional()
        .map_err(io::Error::other)
}

fn upsert_checkpoint_row(
    transaction: &rusqlite::Transaction<'_>,
    checkpoint: &SuspendedSessionCheckpoint,
    reconnect_guard: &ReconnectGuard,
    generation: i64,
    phase_revision: i64,
    last_updated_wall: i64,
) -> io::Result<()> {
    transaction
        .execute(
            "
            INSERT INTO suspended_session_checkpoints (
                session_key,
                owner_key,
                generation,
                phase_revision,
                placed_id,
                checkpoint_json,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(session_key) DO UPDATE SET
                owner_key = excluded.owner_key,
                generation = excluded.generation,
                phase_revision = excluded.phase_revision,
                placed_id = excluded.placed_id,
                checkpoint_json = excluded.checkpoint_json,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                reconnect_guard.session_key,
                reconnect_guard.owner_key,
                generation,
                phase_revision,
                checkpoint.placed_id,
                checkpoint.checkpoint_json,
                SESSION_CHECKPOINT_SCHEMA_VERSION,
                last_updated_wall,
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

fn upsert_craft_restore_guard_row(
    transaction: &rusqlite::Transaction<'_>,
    reconnect_guard: &ReconnectGuard,
    generation: i64,
    phase_revision: i64,
    last_updated_wall: i64,
) -> io::Result<()> {
    let craft_restore_guard = CraftRestoreGuard::from(reconnect_guard);
    let frame_json = serde_json::to_string(&craft_restore_guard)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    transaction
        .execute(
            "
            INSERT INTO craft_restore_guards (
                session_key,
                owner_key,
                generation,
                phase_revision,
                restore_token,
                frame_json,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(session_key) DO UPDATE SET
                owner_key = excluded.owner_key,
                generation = excluded.generation,
                phase_revision = excluded.phase_revision,
                restore_token = excluded.restore_token,
                frame_json = excluded.frame_json,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                reconnect_guard.session_key,
                reconnect_guard.owner_key,
                generation,
                phase_revision,
                reconnect_guard.restore_token,
                frame_json,
                SESSION_CHECKPOINT_SCHEMA_VERSION,
                last_updated_wall,
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

fn load_checkpoint_row(
    connection: &Connection,
    session_key: &str,
) -> io::Result<Option<SuspendedSessionCheckpoint>> {
    connection
        .query_row(
            "
            SELECT owner_key, generation, phase_revision, placed_id, checkpoint_json
            FROM suspended_session_checkpoints
            WHERE session_key = ?1
            ",
            params![session_key],
            |row| {
                Ok(SuspendedSessionCheckpoint {
                    owner_key: row.get(0)?,
                    session_key: session_key.to_string(),
                    generation: u64_from_sql("generation", row.get(1)?)?,
                    phase_revision: u64_from_sql("phase_revision", row.get(2)?)?,
                    placed_id: row.get(3)?,
                    checkpoint_json: row.get(4)?,
                })
            },
        )
        .optional()
        .map_err(io::Error::other)
}

fn load_guard_row(
    connection: &Connection,
    session_key: &str,
) -> io::Result<Option<StoredCraftRestoreGuardRow>> {
    connection
        .query_row(
            "
            SELECT owner_key, generation, phase_revision, restore_token, frame_json
            FROM craft_restore_guards
            WHERE session_key = ?1
            ",
            params![session_key],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            },
        )
        .optional()
        .map_err(io::Error::other)
}

/// 原子消费一次性 `ReconnectGuard`，防止旧 control frame 重放。
///
/// checkpoint 本体暂时保留，供 R1 成功 restore 后写入新的 Paused snapshot；只有
/// owner/session/generation/revision/token 全字段匹配时才会删除 guard 行。
pub fn consume_reconnect_guard(
    persistence: &PlayerStatePersistence,
    reconnect_guard: &ReconnectGuard,
) -> io::Result<bool> {
    validate_guard(reconnect_guard)?;
    let mut connection = open_player_connection(persistence)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    let deleted = transaction
        .execute(
            "
            DELETE FROM craft_restore_guards
            WHERE session_key = ?1
              AND owner_key = ?2
              AND generation = ?3
              AND phase_revision = ?4
              AND restore_token = ?5
            ",
            params![
                reconnect_guard.session_key,
                reconnect_guard.owner_key,
                sqlite_u64("generation", reconnect_guard.generation)?,
                sqlite_u64("phase_revision", reconnect_guard.phase_revision)?,
                reconnect_guard.restore_token,
            ],
        )
        .map_err(io::Error::other)?;
    transaction.commit().map_err(io::Error::other)?;
    Ok(deleted == 1)
}

fn validate_checkpoint_and_guard(
    checkpoint: &SuspendedSessionCheckpoint,
    reconnect_guard: &ReconnectGuard,
) -> io::Result<()> {
    validate_non_empty_key("checkpoint.owner_key", &checkpoint.owner_key)?;
    validate_non_empty_key("checkpoint.session_key", &checkpoint.session_key)?;
    validate_non_empty_key("guard.owner_key", &reconnect_guard.owner_key)?;
    validate_non_empty_key("guard.session_key", &reconnect_guard.session_key)?;
    if checkpoint.owner_key != reconnect_guard.owner_key
        || checkpoint.session_key != reconnect_guard.session_key
        || checkpoint.generation != reconnect_guard.generation
        || checkpoint.phase_revision != reconnect_guard.phase_revision
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Suspended checkpoint and ReconnectGuard version/identity fields must match",
        ));
    }
    if let Some(placed_id) = checkpoint.placed_id.as_deref() {
        validate_non_empty_key("checkpoint.placed_id", placed_id)?;
    }
    if checkpoint.checkpoint_json.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Suspended checkpoint JSON cannot be empty",
        ));
    }
    serde_json::from_str::<serde_json::Value>(&checkpoint.checkpoint_json).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Suspended checkpoint JSON is invalid: {error}"),
        )
    })?;
    validate_guard(reconnect_guard)
}

fn validate_guard(guard: &ReconnectGuard) -> io::Result<()> {
    validate_non_empty_key("guard.owner_key", &guard.owner_key)?;
    validate_non_empty_key("guard.session_key", &guard.session_key)?;
    let token_bytes = guard.restore_token.as_bytes();
    if !(32..=128).contains(&token_bytes.len())
        || !token_bytes[0].is_ascii_alphanumeric()
        || token_bytes
            .iter()
            .skip(1)
            .any(|byte| !byte.is_ascii_alphanumeric() && !b"._~-".contains(byte))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "restore_token must be 32..=128 ASCII bytes with a word-character first byte",
        ));
    }
    Ok(())
}

fn validate_non_empty_key(field: &str, value: &str) -> io::Result<()> {
    if value.trim().is_empty() || value.as_bytes().contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{field} cannot be empty or contain NUL"),
        ));
    }
    Ok(())
}

fn sqlite_u64(field: &str, value: u64) -> io::Result<i64> {
    i64::try_from(value).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{field}={value} exceeds SQLite INTEGER range"),
        )
    })
}

fn u64_from_sql(field: &str, value: i64) -> rusqlite::Result<u64> {
    u64::try_from(value).map_err(|_| {
        rusqlite::Error::ToSqlConversionFailure(Box::new(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{field} stored a negative SQLite INTEGER"),
        )))
    })
}

fn is_stale_version(
    existing_generation: i64,
    existing_revision: i64,
    incoming_generation: i64,
    incoming_revision: i64,
) -> bool {
    existing_generation > incoming_generation
        || (existing_generation == incoming_generation && existing_revision > incoming_revision)
}
