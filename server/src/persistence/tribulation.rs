//! Active tribulation and ascension quota persistence.

use super::*;

pub fn persist_active_tribulation(
    settings: &PersistenceSettings,
    record: &ActiveTribulationRecord,
) -> io::Result<()> {
    let wall_clock = current_unix_seconds();
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    upsert_active_tribulation(&transaction, record, wall_clock)?;
    transaction.commit().map_err(io::Error::other)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn load_active_tribulation(
    settings: &PersistenceSettings,
    char_id: &str,
) -> io::Result<Option<ActiveTribulationRecord>> {
    let connection = open_persistence_connection(settings)?;
    load_active_tribulation_from_connection(&connection, char_id)
}

pub fn load_active_tribulation_count(settings: &PersistenceSettings) -> io::Result<u32> {
    let connection = open_persistence_connection(settings)?;
    let count: i64 = connection
        .query_row(
            "
            SELECT COUNT(*) FROM tribulations_active
            WHERE kind = ?1
               OR (kind = ?2 AND source = ?3)
            ",
            params![
                TRIBULATION_KIND_DU_XU,
                TRIBULATION_KIND_JUE_BI,
                JUEBI_SOURCE_VOID_QUOTA_EXCEEDED
            ],
            |row| row.get(0),
        )
        .map_err(io::Error::other)?;
    sql_to_u32(count)
}

pub fn delete_active_tribulation(settings: &PersistenceSettings, char_id: &str) -> io::Result<()> {
    let connection = open_persistence_connection(settings)?;
    connection
        .execute(
            "DELETE FROM tribulations_active WHERE char_id = ?1",
            params![char_id],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn load_ascension_quota(settings: &PersistenceSettings) -> io::Result<AscensionQuotaRecord> {
    let connection = open_persistence_connection(settings)?;
    load_ascension_quota_from_connection(&connection)
}

pub fn complete_tribulation_ascension(
    settings: &PersistenceSettings,
    char_id: &str,
) -> io::Result<AscensionQuotaRecord> {
    let wall_clock = current_unix_seconds();
    let mut connection = open_persistence_connection(settings)?;
    // r1-P5 fix：改用 IMMEDIATE 事务，起手即取写锁。
    //
    // 原来的 DEFERRED 事务在 WAL 模式下先读后写：两个并发 DuXu 完成各自在
    // SHARED 锁下读到相同的 occupied_slots（如 1），然后都写 2，丢失一次增量
    // （lost update）。IMMEDIATE 在 BEGIN 时就拿 RESERVED 写锁，保证
    // read-check-write 相对于其他 IMMEDIATE/EXCLUSIVE writer 是原子串行的。
    // 这是 worldview §三:78 化虚稀缺不变量在 SQLite 层面的硬保证。
    // 与 try_complete_tribulation_ascension（:2831）保持一致。
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(io::Error::other)?;
    let mut quota = load_ascension_quota_from_transaction(&transaction)?;
    let active_kind_source: Option<(String, String)> = transaction
        .query_row(
            "SELECT kind, source FROM tribulations_active WHERE char_id = ?1",
            params![char_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(io::Error::other)?;
    let occupies_quota = matches!(
        active_kind_source
            .as_ref()
            .map(|(kind, source)| (kind.as_str(), source.as_str())),
        Some((TRIBULATION_KIND_DU_XU, _))
            | Some((TRIBULATION_KIND_JUE_BI, JUEBI_SOURCE_VOID_QUOTA_EXCEEDED))
    );
    if occupies_quota {
        quota.occupied_slots = quota.occupied_slots.saturating_add(1);
    }

    transaction
        .execute(
            "DELETE FROM tribulations_active WHERE char_id = ?1",
            params![char_id],
        )
        .map_err(io::Error::other)?;
    upsert_ascension_quota(&transaction, &quota, wall_clock)?;
    transaction.commit().map_err(io::Error::other)?;
    Ok(quota)
}

/// plan-halfstep-buff-v1 P2 atomic ascension grant 四态决策。
///
/// 演进历史：
/// - P3 review #4：把"缺 active row"从 `granted=true` 中拆出（避免误升 Realm）→ `MissingActive`
/// - P4 review #2：把"占额成功"和"非占额仅结算"也拆开（独立 JueBi 不应升 Realm）→ `SettledOnly`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AscensionGrant {
    /// 占额路径（`du_xu` / `jue_bi + void_quota_exceeded`）+ 事务内 quota 校验通过：
    /// `occupied_slots` 已 +1，caller **应升 Realm 到 Void**
    Granted,
    /// 非占额路径（独立 JueBi，如 `void_action_explode_zone`）幸存：
    /// active row 已删但 `occupied_slots` **未增**；caller **不升 Realm**（化虚老怪扛过
    /// 额外天劫不算升格冲刺），仅作 settlement-success 标志
    SettledOnly,
    /// quota 已满（`occupied == limit`）或 `limit=0`（灵气枯竭），caller 回退 HalfStep
    Denied,
    /// `tribulations_active` 找不到 char_id（重复结算 / 状态错乱 / 已被另一进程 settle）。
    /// 本分支**不增量** `quota.occupied_slots`，但仍 commit transaction（保 idempotency +
    /// 清理 active 行）。caller 应 warn + 回退 HalfStep，绝不升 Realm
    MissingActive,
}

/// plan-halfstep-buff-v1 P2 atomic ascension grant outcome。
///
/// `quota` 是事务 commit 后最终的 [`AscensionQuotaRecord`]；`grant` 是 4 态决策
/// [`AscensionGrant::Granted`] / [`AscensionGrant::SettledOnly`] /
/// [`AscensionGrant::Denied`] / [`AscensionGrant::MissingActive`]，caller 必须 match
/// 全部 4 分支（典型用法：仅 `Granted` 升 Realm；其余 3 态均回退 HalfStep / 不升 Realm）；
/// `limit_used` / `occupied_before` 便于追踪并发情况和测试断言。
///
/// **事务行为**：两者均使用 IMMEDIATE 事务（起手即取写锁），无论 `grant` 何种状态，
/// 事务都会删除 `tribulations_active` 行 + commit quota 行（保 idempotency）；
/// 区别在只有 `Granted` 路径会 `occupied_slots += 1`，其他 3 态保持 quota 不变。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtomicAscensionOutcome {
    pub quota: AscensionQuotaRecord,
    pub grant: AscensionGrant,
    pub limit_used: u32,
    pub occupied_before: u32,
}

/// plan-halfstep-buff-v1 P2：事务内原子校验 quota 限额后再决定是否授予 ascension。
///
/// 与 `complete_tribulation_ascension` 的区别：本函数在 transaction 内额外检查
/// `quota.occupied_slots < quota_limit`；如果已满，**不增量、不破坏 DB 状态**，仅返回
/// `AscensionGrant::Denied`。即使如此，仍然删除 `tribulations_active` 行（entity 渡劫
/// 流程已完成，不该留下孤儿 active 记录）+ commit quota 行（保持 idempotent）。返回值
/// 见 [`AtomicAscensionOutcome`] 的 4 态枚举说明。
///
/// 这是 worldview §三:78 化虚稀缺性的硬保证 —— 即使多人同 tick 渡虚劫成功也不会突破名额上限。
///
/// **并发语义**（P5 review #5 澄清）：IMMEDIATE 事务保证 select-check-update 的
/// **原子串行化**（atomic serialization），即任何两个并发调用不会同时读到相同 quota
/// 然后都增量；SQLite **不承诺公平/FIFO 顺序**——多个 BEGIN IMMEDIATE 的获取顺序由
/// SQLite 内部锁队列决定，不一定按调用次序。worldview §三:78 关心的是"不突破名额上限"
/// （原子性保证），不是"谁先谁后"（公平性），所以这里的语义足够。
pub fn try_complete_tribulation_ascension(
    settings: &PersistenceSettings,
    char_id: &str,
    quota_limit: u32,
) -> io::Result<AtomicAscensionOutcome> {
    let wall_clock = current_unix_seconds();
    let mut connection = open_persistence_connection(settings)?;
    // plan-halfstep-buff-v1 P2 fix：用 IMMEDIATE 事务而非默认 DEFERRED。
    //
    // DEFERRED 在 WAL 模式下先读后写：另一个 writer 在我们 BEGIN 之后、UPDATE 之前提交了
    // 自己的写入，会让我们的 commit 失败为 `SQLITE_BUSY_SNAPSHOT` 或 `SQLITE_BUSY`，而不是
    // 把 read-check-write 序列化。IMMEDIATE 立即拿写锁，保证 `quota.occupied_slots <
    // quota_limit` 检查与 UPDATE 之间没有并发 writer 插队。这是 §三:78 化虚稀缺底线在
    // SQLite 层面的硬保证。
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(io::Error::other)?;
    let mut quota = load_ascension_quota_from_transaction(&transaction)?;
    let occupied_before = quota.occupied_slots;

    let active_kind_source: Option<(String, String)> = transaction
        .query_row(
            "SELECT kind, source FROM tribulations_active WHERE char_id = ?1",
            params![char_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(io::Error::other)?;

    let grant = match active_kind_source.as_ref().map(|(kind, source)| {
        (
            kind.as_str(),
            source.as_str(),
            matches!(
                (kind.as_str(), source.as_str()),
                (TRIBULATION_KIND_DU_XU, _)
                    | (TRIBULATION_KIND_JUE_BI, JUEBI_SOURCE_VOID_QUOTA_EXCEEDED)
            ),
        )
    }) {
        None => {
            // active row 缺失 → 状态错乱或重复结算；不增量，让 caller 走 warn + HalfStep
            AscensionGrant::MissingActive
        }
        Some((_, _, true)) => {
            // 占额路径（du_xu / jue_bi+void_quota_exceeded）→ 名额校验
            if quota_limit > 0 && quota.occupied_slots < quota_limit {
                quota.occupied_slots = quota.occupied_slots.saturating_add(1);
                AscensionGrant::Granted
            } else {
                AscensionGrant::Denied
            }
        }
        Some((_, _, false)) => {
            // 非占额路径（独立 JueBi 如 VoidActionExplodeZone）→ 不增不减
            // 用 `SettledOnly` 而非 `Granted` 让 caller 显式不升 Realm
            AscensionGrant::SettledOnly
        }
    };

    transaction
        .execute(
            "DELETE FROM tribulations_active WHERE char_id = ?1",
            params![char_id],
        )
        .map_err(io::Error::other)?;
    upsert_ascension_quota(&transaction, &quota, wall_clock)?;
    transaction.commit().map_err(io::Error::other)?;
    Ok(AtomicAscensionOutcome {
        quota,
        grant,
        limit_used: quota_limit,
        occupied_before,
    })
}

pub fn release_ascension_quota_slot(
    settings: &PersistenceSettings,
) -> io::Result<AscensionQuotaRelease> {
    let wall_clock = current_unix_seconds();
    let mut connection = open_persistence_connection(settings)?;
    // r3-P2 fix：改用 IMMEDIATE 事务，起手即取写锁。
    //
    // 原来的 DEFERRED 事务在 WAL 模式下先读后写：两个并发 release 各自在
    // SHARED 锁下读到相同的 occupied_slots（如 2），然后都写 1，丢失一次减量
    // （lost update）。IMMEDIATE 在 BEGIN 时就拿 RESERVED 写锁，保证
    // read-check-write 相对于其他 IMMEDIATE/EXCLUSIVE writer 是原子串行的。
    // 与 complete_tribulation_ascension（:2736）保持一致。
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(io::Error::other)?;
    let mut quota = load_ascension_quota_from_transaction(&transaction)?;
    let opened_slot = quota.occupied_slots > 0;
    quota.occupied_slots = quota.occupied_slots.saturating_sub(1);
    upsert_ascension_quota(&transaction, &quota, wall_clock)?;
    transaction.commit().map_err(io::Error::other)?;
    Ok(AscensionQuotaRelease { quota, opened_slot })
}

pub(super) fn upsert_active_tribulation(
    transaction: &rusqlite::Transaction<'_>,
    record: &ActiveTribulationRecord,
    wall_clock: i64,
) -> io::Result<()> {
    transaction
        .execute(
            "
            INSERT INTO tribulations_active (
                char_id,
                kind,
                source,
                origin_dimension,
                wave_current,
                waves_total,
                started_tick,
                epicenter_x,
                epicenter_y,
                epicenter_z,
                intensity,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            ON CONFLICT(char_id) DO UPDATE SET
                kind = excluded.kind,
                source = excluded.source,
                origin_dimension = excluded.origin_dimension,
                wave_current = excluded.wave_current,
                waves_total = excluded.waves_total,
                started_tick = excluded.started_tick,
                epicenter_x = excluded.epicenter_x,
                epicenter_y = excluded.epicenter_y,
                epicenter_z = excluded.epicenter_z,
                intensity = excluded.intensity,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                record.char_id.as_str(),
                record.kind.as_str(),
                record.source.as_str(),
                record.origin_dimension.as_deref(),
                i64::from(record.wave_current),
                i64::from(record.waves_total),
                tick_to_sql(record.started_tick)?,
                record.epicenter[0],
                record.epicenter[1],
                record.epicenter[2],
                f64::from(record.intensity),
                CURRENT_SCHEMA_VERSION,
                wall_clock,
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

pub(super) fn upsert_ascension_quota(
    transaction: &rusqlite::Transaction<'_>,
    record: &AscensionQuotaRecord,
    wall_clock: i64,
) -> io::Result<()> {
    transaction
        .execute(
            "
            INSERT INTO ascension_quota (
                row_id,
                occupied_slots,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(row_id) DO UPDATE SET
                occupied_slots = excluded.occupied_slots,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                ASCENSION_QUOTA_ROW_ID,
                i64::from(record.occupied_slots),
                CURRENT_SCHEMA_VERSION,
                wall_clock,
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

pub(super) fn load_active_tribulation_from_connection(
    connection: &Connection,
    char_id: &str,
) -> io::Result<Option<ActiveTribulationRecord>> {
    type ActiveTribulationRow = (
        String,
        String,
        Option<String>,
        i64,
        i64,
        i64,
        f64,
        f64,
        f64,
        f64,
    );
    let row: Option<ActiveTribulationRow> = connection
        .query_row(
            "
            SELECT kind, source, origin_dimension, wave_current, waves_total, started_tick, epicenter_x, epicenter_y, epicenter_z, intensity
            FROM tribulations_active
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
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get(9)?,
                ))
            },
        )
        .optional()
        .map_err(io::Error::other)?;
    let Some((
        kind,
        source,
        origin_dimension,
        wave_current,
        waves_total,
        started_tick,
        x,
        y,
        z,
        intensity,
    )) = row
    else {
        return Ok(None);
    };

    Ok(Some(ActiveTribulationRecord {
        char_id: char_id.to_string(),
        kind,
        source,
        origin_dimension,
        wave_current: sql_to_u32(wave_current)?,
        waves_total: sql_to_u32(waves_total)?,
        started_tick: sql_to_tick(started_tick)?,
        epicenter: [x, y, z],
        intensity: intensity as f32,
    }))
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn load_ascension_quota_from_connection(
    connection: &Connection,
) -> io::Result<AscensionQuotaRecord> {
    let row: Option<i64> = connection
        .query_row(
            "SELECT occupied_slots FROM ascension_quota WHERE row_id = ?1",
            params![ASCENSION_QUOTA_ROW_ID],
            |row| row.get(0),
        )
        .optional()
        .map_err(io::Error::other)?;

    Ok(AscensionQuotaRecord {
        occupied_slots: match row {
            Some(occupied_slots) => sql_to_u32(occupied_slots)?,
            None => 0,
        },
    })
}

pub(super) fn load_ascension_quota_from_transaction(
    transaction: &rusqlite::Transaction<'_>,
) -> io::Result<AscensionQuotaRecord> {
    let row: Option<i64> = transaction
        .query_row(
            "SELECT occupied_slots FROM ascension_quota WHERE row_id = ?1",
            params![ASCENSION_QUOTA_ROW_ID],
            |row| row.get(0),
        )
        .optional()
        .map_err(io::Error::other)?;

    Ok(AscensionQuotaRecord {
        occupied_slots: match row {
            Some(occupied_slots) => sql_to_u32(occupied_slots)?,
            None => 0,
        },
    })
}
