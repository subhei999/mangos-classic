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
    pub gmlevel: u8,
    pub sessionkey: String,
    pub v: String,
    pub s: String,
    pub email: String,
    pub locked: u8,
    pub expansion: u8,
    pub locale: String,
    pub os: String,
}

/// A row from the `account_banned` table.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct AccountBanned {
    pub id: i32,
    pub account_id: i32,
    pub banned_at: i64,
    pub expires_at: i64,
    pub banned_by: String,
    pub reason: String,
    pub active: i8,
}

/// A row from the `ip_banned` table.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct IpBanned {
    pub ip: String,
    pub banned_at: i64,
    pub expires_at: i64,
    pub banned_by: String,
    pub reason: String,
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
        "SELECT id, username, gmlevel, sessionkey, v, s, \
                email, locked, expansion, locale, os \
         FROM account WHERE username = ?",
    )
    .bind(&username)
    .fetch_optional(pool)
    .await?;

    Ok(account)
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
        "SELECT id, account_id, banned_at, expires_at, banned_by, reason, active \
         FROM account_banned \
         WHERE account_id = ? AND active = 1 AND (expires_at > UNIX_TIMESTAMP() OR expires_at = banned_at) \
         LIMIT 1",
    )
    .bind(account_id)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

/// Check whether an IP address is currently banned.
pub async fn get_ip_banned(pool: &MySqlPool, ip: &str) -> Result<Option<IpBanned>, DbError> {
    let row = sqlx::query_as::<_, IpBanned>(
        "SELECT ip, banned_at, expires_at, banned_by, reason \
         FROM ip_banned \
         WHERE ip = ? AND (expires_at > UNIX_TIMESTAMP() OR expires_at = banned_at) \
         LIMIT 1",
    )
    .bind(ip)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_username_trims_and_uppercases() {
        assert_eq!(normalize_username("  testUser  "), "TESTUSER");
    }
}
