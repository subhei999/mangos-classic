use serde::{Deserialize, Serialize};
use sqlx::mysql::MySqlPool;
use sqlx::FromRow;

use crate::pool::{DbError, DbResult};

// ---------------------------------------------------------------------------
// Models
// ---------------------------------------------------------------------------

/// A row from the `account` table in the `realmd` database.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Account {
    pub id: u32,
    pub username: String,
    pub sha_pass_hash: String,
    pub gmlevel: u8,
    pub sessionkey: String,
    pub v: String,
    pub s: String,
    pub email: String,
    pub last_ip: String,
    pub locked: u8,
    pub expansion: u8,
    pub locale: u8,
    pub os: String,
}

/// A row from the `account_banned` table.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct AccountBanned {
    pub id: u32,
    pub bandate: i64,
    pub unbandate: i64,
    pub bannedby: String,
    pub banreason: String,
    pub active: u8,
}

/// A row from the `ip_banned` table.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct IpBanned {
    pub ip: String,
    pub bandate: i64,
    pub unbandate: i64,
    pub bannedby: String,
    pub banreason: String,
}

// ---------------------------------------------------------------------------
// Queries
// ---------------------------------------------------------------------------

/// Normalize account names the same way the auth handshake expects them.
pub fn normalize_username(username: &str) -> String {
    username.trim().to_uppercase()
}

/// Look up an account by username (case-insensitive).
pub async fn get_account_by_username(
    pool: &MySqlPool,
    username: &str,
) -> Result<Option<Account>, DbError> {
    let username = normalize_username(username);
    let account = sqlx::query_as::<_, Account>(
        "SELECT id, username, sha_pass_hash, gmlevel, sessionkey, v, s, \
                email, last_ip, locked, expansion, locale, os \
         FROM account WHERE username = ?",
    )
    .bind(&username)
    .fetch_optional(pool)
    .await?;

    Ok(account)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_username_trims_and_uppercases() {
        assert_eq!(normalize_username("  testUser  "), "TESTUSER");
    }
}

/// Persist the SRP session key and verifier/salt for an account after a
/// successful login handshake.
pub async fn update_session_key(
    pool: &MySqlPool,
    account_id: u32,
    session_key: &str,
    v: &str,
    s: &str,
) -> DbResult<()> {
    sqlx::query("UPDATE account SET sessionkey = ?, v = ?, s = ? WHERE id = ?")
        .bind(session_key)
        .bind(v)
        .bind(s)
        .bind(account_id)
        .execute(pool)
        .await?;

    Ok(())
}

/// Check whether an account is currently banned.
/// Returns the *active* ban row if one exists.
pub async fn get_account_banned(
    pool: &MySqlPool,
    account_id: u32,
) -> Result<Option<AccountBanned>, DbError> {
    let row = sqlx::query_as::<_, AccountBanned>(
        "SELECT id, bandate, unbandate, bannedby, banreason, active \
         FROM account_banned \
         WHERE id = ? AND active = 1 AND (unbandate > UNIX_TIMESTAMP() OR unbandate = bandate) \
         LIMIT 1",
    )
    .bind(account_id)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

/// Check whether an IP address is currently banned.
pub async fn get_ip_banned(
    pool: &MySqlPool,
    ip: &str,
) -> Result<Option<IpBanned>, DbError> {
    let row = sqlx::query_as::<_, IpBanned>(
        "SELECT ip, bandate, unbandate, bannedby, banreason \
         FROM ip_banned \
         WHERE ip = ? AND (unbandate > UNIX_TIMESTAMP() OR unbandate = bandate) \
         LIMIT 1",
    )
    .bind(ip)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}
