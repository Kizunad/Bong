//! Durable runtime qi-account persistence.

use super::*;

use crate::qi_physics::ledger::{is_anqi_carrier_account, ANQI_CARRIER_ACCOUNT_PREFIX};

fn upsert_runtime_qi_account_balance(
    transaction: &rusqlite::Transaction<'_>,
    qi_ledger: &WorldQiAccount,
    account: &QiAccountId,
    wall_clock: i64,
) -> io::Result<()> {
    let balance = qi_ledger.balance(account);
    if !balance.is_finite() || balance < 0.0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid runtime qi balance account={account} balance={balance}"),
        ));
    }
    transaction
        .execute(
            "
        INSERT INTO qi_runtime_accounts (
            account_id,
            balance,
            schema_version,
            last_updated_wall
        ) VALUES (?1, ?2, ?3, ?4)
        ON CONFLICT(account_id) DO UPDATE SET
            balance = excluded.balance,
            schema_version = excluded.schema_version,
            last_updated_wall = excluded.last_updated_wall
        ",
            params![
                account.id.as_str(),
                balance,
                CURRENT_SCHEMA_VERSION,
                wall_clock,
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

pub(crate) fn upsert_runtime_qi_account_balances(
    transaction: &rusqlite::Transaction<'_>,
    qi_ledger: &WorldQiAccount,
    wall_clock: i64,
) -> io::Result<()> {
    // Main credits TSY drain into the fixed `rift_drain_account()`, which is already in this
    // whitelist. Sync every durable account through one path; do not recreate the PR's obsolete
    // zone-specific `rift:*` row scan.
    for account in persistent_runtime_qi_accounts() {
        upsert_runtime_qi_account_balance(transaction, qi_ledger, &account, wall_clock)?;
    }
    // Carrier accounts are dynamic, but their prefix is stable. Read the durable rows before
    // touching them: a positive database balance with no corresponding ledger owner is
    // unknowable, so saving must fail closed rather than silently deleting the only copy.
    let mut statement = transaction
        .prepare(
            "
            SELECT account_id, balance
            FROM qi_runtime_accounts
            WHERE account_id LIKE ?1
            ORDER BY account_id
            ",
        )
        .map_err(io::Error::other)?;
    let existing_rows = statement
        .query_map(params![format!("{ANQI_CARRIER_ACCOUNT_PREFIX}%")], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
        })
        .map_err(io::Error::other)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(io::Error::other)?;
    drop(statement);
    let mut removable_accounts = Vec::new();
    for (account_id, balance) in &existing_rows {
        if !balance.is_finite() || *balance < 0.0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "invalid persisted carrier qi balance account={account_id} balance={balance}"
                ),
            ));
        }
        let account = QiAccountId::container(account_id.clone());
        if *balance <= f64::EPSILON {
            removable_accounts.push(account_id.clone());
            continue;
        }
        if !qi_ledger.has_account(&account) {
            if qi_ledger.is_retired_carrier_account(&account) {
                removable_accounts.push(account_id.clone());
                continue;
            }
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "persisted carrier qi account={account_id} has positive balance {balance} but is missing from the current ledger"
                ),
            ));
        }
        if qi_ledger.balance(&account) <= f64::EPSILON {
            removable_accounts.push(account_id.clone());
        }
    }
    // A zero row, or a row whose ledger account is explicitly present at zero (or retired after
    // a completed release), is safe to remove. Never use a prefix-wide DELETE, because that would
    // erase an active carrier whose ledger was not hydrated or whose owner was accidentally omitted.
    for account_id in removable_accounts {
        transaction
            .execute(
                "DELETE FROM qi_runtime_accounts WHERE account_id = ?1",
                params![account_id],
            )
            .map_err(io::Error::other)?;
    }
    for (account, balance) in qi_ledger.iter_balances() {
        if is_anqi_carrier_account(account) && balance > f64::EPSILON {
            upsert_runtime_qi_account_balance(transaction, qi_ledger, account, wall_clock)?;
        }
    }
    Ok(())
}

pub(crate) fn load_runtime_qi_account_balances(
    settings: &PersistenceSettings,
) -> io::Result<Vec<(QiAccountId, f64)>> {
    let connection = open_persistence_connection(settings)?;
    let mut balances = Vec::new();
    for account in persistent_runtime_qi_accounts() {
        let balance = connection
            .query_row(
                "
            SELECT balance
            FROM qi_runtime_accounts
            WHERE account_id = ?1
            ",
                params![account.id.as_str()],
                |row| row.get::<_, f64>(0),
            )
            .optional()
            .map_err(io::Error::other)?;
        match balance {
            Some(value) if value.is_finite() && value >= 0.0 => balances.push((account, value)),
            Some(value) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "invalid persisted runtime qi balance account={} balance={value}",
                        account.id
                    ),
                ));
            }
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "runtime qi balance account={} is unknown; refusing to invent zero",
                        account.id
                    ),
                ));
            }
        }
    }
    let mut statement = connection
        .prepare(
            "
            SELECT account_id, balance
            FROM qi_runtime_accounts
            WHERE account_id LIKE ?1
            ORDER BY account_id
            ",
        )
        .map_err(io::Error::other)?;
    let dynamic_accounts = statement
        .query_map(params![format!("{ANQI_CARRIER_ACCOUNT_PREFIX}%")], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
        })
        .map_err(io::Error::other)?;
    for row in dynamic_accounts {
        let (account_id, value) = row.map_err(io::Error::other)?;
        if !value.is_finite() || value < 0.0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "invalid persisted runtime qi balance account={account_id} balance={value}"
                ),
            ));
        }
        balances.push((QiAccountId::container(account_id), value));
    }
    Ok(balances)
}

pub(crate) fn hydrate_runtime_qi_accounts(
    settings: &PersistenceSettings,
    qi_ledger: &mut WorldQiAccount,
) -> io::Result<usize> {
    let balances = load_runtime_qi_account_balances(settings)?;
    for (account, balance) in &balances {
        qi_ledger
            .set_balance(account.clone(), *balance)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    }
    Ok(balances.len())
}

#[cfg(test)]
pub(crate) fn load_pending_inflow_balance(settings: &PersistenceSettings) -> io::Result<f64> {
    load_runtime_qi_account_balances(settings)?
        .into_iter()
        .find(|(account, _)| *account == pending_inflow_account())
        .map(|(_, balance)| balance)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "pending inflow account missing from persistent runtime whitelist",
            )
        })
}
