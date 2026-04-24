use serde::{Deserialize, Serialize};
use sqlx::mysql::MySqlPool;
use sqlx::FromRow;

use crate::character::character_count_for_account;
use crate::pool::{DbError, DbResult};

// ---------------------------------------------------------------------------
// Models
// ---------------------------------------------------------------------------

/// A row from the `realmlist` table in the `realmd` database.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct RealmEntry {
    pub id: u32,
    pub name: String,
    pub address: String,
    pub port: i32,
    pub icon: u8,
    pub realmflags: u8,
    pub timezone: u8,
    pub population: f32,
    #[sqlx(rename = "allowedSecurityLevel")]
    pub allowed_security_level: u8,
}

/// Helper struct for the `realmcharacters` join.
#[derive(Debug, Clone, FromRow)]
pub struct RealmCharacterCount {
    pub realmid: u32,
    pub numchars: u8,
}

// ---------------------------------------------------------------------------
// Queries
// ---------------------------------------------------------------------------

/// Format a realm network endpoint in the form the 1.12.x client expects.
pub fn realm_address(address: &str, port: i32) -> String {
    format!("{address}:{port}")
}

/// Return all realm entries from the `realmlist` table.
pub async fn get_realm_list(pool: &MySqlPool) -> Result<Vec<RealmEntry>, DbError> {
    let rows = sqlx::query_as::<_, RealmEntry>(
        "SELECT id, name, address, port, icon, realmflags, timezone, \
                population, allowedSecurityLevel \
         FROM realmlist",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// Return the number of characters each realm has for a given account.
///
/// The result is a list of `(realm_id, num_chars)` tuples.
pub async fn get_realm_characters(
    pool: &MySqlPool,
    account_id: u32,
) -> Result<Vec<(u32, u8)>, DbError> {
    let rows = sqlx::query_as::<_, RealmCharacterCount>(
        "SELECT realmid, numchars FROM realmcharacters WHERE acctid = ?",
    )
    .bind(account_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| (r.realmid, r.numchars)).collect())
}

/// Update the population value for a realm.
pub async fn update_realm_population(
    pool: &MySqlPool,
    realm_id: u32,
    population: f32,
) -> DbResult<()> {
    sqlx::query("UPDATE realmlist SET population = ? WHERE id = ?")
        .bind(population)
        .bind(realm_id)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn set_realm_character_count(
    pool: &MySqlPool,
    account_id: u32,
    realm_id: u32,
    num_chars: u8,
) -> DbResult<()> {
    sqlx::query(
        "INSERT INTO realmcharacters (realmid, acctid, numchars) \
         VALUES (?, ?, ?) \
         ON DUPLICATE KEY UPDATE numchars = VALUES(numchars)",
    )
    .bind(realm_id)
    .bind(account_id)
    .bind(num_chars)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn refresh_realm_character_count(
    login_pool: &MySqlPool,
    character_pool: &MySqlPool,
    account_id: u32,
    realm_id: u32,
) -> Result<u8, DbError> {
    let count = character_count_for_account(character_pool, account_id).await?;
    set_realm_character_count(login_pool, account_id, realm_id, count).await?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn realm_address_includes_port() {
        assert_eq!(realm_address("127.0.0.1", 8085), "127.0.0.1:8085");
    }
}
