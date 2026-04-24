use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use thiserror::Error;
use tracing::info;

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum DbError {
    #[error("database error: {0}")]
    Sqlx(#[from] sqlx::Error),
}

pub type DbResult<T> = Result<T, DbError>;

// ---------------------------------------------------------------------------
// Pool helper
// ---------------------------------------------------------------------------

/// Create a single MySQL connection pool for the given database URL.
pub async fn create_pool(url: &str, max_connections: u32) -> DbResult<MySqlPool> {
    let pool = MySqlPoolOptions::new()
        .max_connections(max_connections)
        .connect(url)
        .await?;
    Ok(pool)
}

// ---------------------------------------------------------------------------
// DatabasePools
// ---------------------------------------------------------------------------

/// Holds connection pools for the three CMaNGOS databases.
#[derive(Clone)]
pub struct DatabasePools {
    /// The `realmd` (login / auth) database pool.
    pub realmd_pool: MySqlPool,
    /// The `characters` database pool.
    pub characters_pool: MySqlPool,
    /// The `mangos` (world) database pool.
    pub world_pool: MySqlPool,
}

impl DatabasePools {
    /// Connect to all three databases. Each URL should be a valid MySQL
    /// connection string, e.g. `mysql://user:pass@host/dbname`.
    pub async fn new(
        realmd_url: &str,
        characters_url: &str,
        world_url: &str,
    ) -> DbResult<Self> {
        Self::with_max_connections(realmd_url, characters_url, world_url, 10).await
    }

    /// Same as [`new`](Self::new) but allows customising the maximum number of
    /// connections per pool.
    pub async fn with_max_connections(
        realmd_url: &str,
        characters_url: &str,
        world_url: &str,
        max_connections: u32,
    ) -> DbResult<Self> {
        info!("Connecting to realmd database...");
        let realmd_pool = create_pool(realmd_url, max_connections).await?;

        info!("Connecting to characters database...");
        let characters_pool = create_pool(characters_url, max_connections).await?;

        info!("Connecting to world (mangos) database...");
        let world_pool = create_pool(world_url, max_connections).await?;

        info!("All database pools connected successfully.");

        Ok(Self {
            realmd_pool,
            characters_pool,
            world_pool,
        })
    }
}
