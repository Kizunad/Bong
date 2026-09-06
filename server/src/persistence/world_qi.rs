//! Durable runtime qi-account persistence.

use super::*;

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
    Ok(())
}

pub(crate) fn load_runtime_qi_account_balances(
    settings: &PersistenceSettings,
) -> io::Result<Vec<(QiAccountId, f64)>> {
    let connection = open_persistence_connection(settings)?;
    let expected_accounts = persistent_runtime_qi_accounts();
    let expected_ids = expected_accounts
        .iter()
        .map(|account| account.id.as_str())
        .collect::<HashSet<_>>();
    let mut persisted = HashMap::new();
    let mut statement = connection
        .prepare("SELECT account_id, balance FROM qi_runtime_accounts")
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
        })
        .map_err(io::Error::other)?;
    for row in rows {
        let (account_id, value) = row.map_err(io::Error::other)?;
        if !expected_ids.contains(account_id.as_str()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "unknown persisted runtime qi account={account_id}; refusing to ignore durable balance"
                ),
            ));
        }
        if persisted.insert(account_id.clone(), value).is_some() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("duplicate persisted runtime qi account={account_id}"),
            ));
        }
    }

    let mut balances = Vec::with_capacity(expected_accounts.len());
    for account in expected_accounts {
        let Some(value) = persisted.remove(account.id.as_str()) else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "runtime qi balance account={} is unknown; refusing to invent zero",
                    account.id
                ),
            ));
        };
        if !value.is_finite() || value < 0.0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "invalid persisted runtime qi balance account={} balance={value}",
                    account.id
                ),
            ));
        }
        balances.push((account, value));
    }
    Ok(balances)
}

pub(crate) fn hydrate_runtime_qi_accounts(
    settings: &PersistenceSettings,
    qi_ledger: &mut WorldQiAccount,
) -> io::Result<usize> {
    let balances = load_runtime_qi_account_balances(settings)?;
    qi_ledger
        .restore_persistent_runtime_balances(&balances)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
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
