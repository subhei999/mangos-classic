use figment::providers::{Env, Format, Toml};
use figment::Figment;
use serde::Deserialize;

// ---------------------------------------------------------------------------
// DatabaseConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    #[serde(default = "default_db_host")]
    pub host: String,
    #[serde(default = "default_db_port")]
    pub port: u16,
    #[serde(default = "default_db_user")]
    pub user: String,
    #[serde(default = "default_db_password")]
    pub password: String,
    pub database: String,
}

fn default_db_host() -> String {
    "127.0.0.1".to_string()
}
fn default_db_port() -> u16 {
    3306
}
fn default_db_user() -> String {
    "mangos".to_string()
}
fn default_db_password() -> String {
    "mangos".to_string()
}

// ---------------------------------------------------------------------------
// WorldConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct WorldConfig {
    #[serde(default = "default_update_interval_ms")]
    pub update_interval_ms: u32,
    #[serde(default = "default_max_players")]
    pub max_players: u32,
    #[serde(default = "default_player_save_interval_secs")]
    pub player_save_interval_secs: u32,
    #[serde(default = "default_visibility_distance")]
    pub visibility_distance: f32,
    #[serde(default = "default_motd")]
    pub motd: String,
}

fn default_update_interval_ms() -> u32 {
    50
}
fn default_max_players() -> u32 {
    100
}
fn default_player_save_interval_secs() -> u32 {
    900
}
fn default_visibility_distance() -> f32 {
    90.0
}
fn default_motd() -> String {
    "Welcome to CMaNGOS Rust!".to_string()
}

// ---------------------------------------------------------------------------
// AuthServerConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct AuthServerConfig {
    #[serde(default = "default_bind_address")]
    pub bind_address: String,
    #[serde(default = "default_auth_port")]
    pub bind_port: u16,
    pub database: DatabaseConfig,
}

fn default_bind_address() -> String {
    "0.0.0.0".to_string()
}
fn default_auth_port() -> u16 {
    3724
}

// ---------------------------------------------------------------------------
// WorldServerConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct WorldServerConfig {
    #[serde(default = "default_bind_address")]
    pub bind_address: String,
    #[serde(default = "default_world_port")]
    pub bind_port: u16,
    #[serde(default = "default_data_dir")]
    pub data_dir: String,
    pub world_database: DatabaseConfig,
    pub character_database: DatabaseConfig,
    pub login_database: DatabaseConfig,
    #[serde(default)]
    pub world: WorldConfig,
}

fn default_world_port() -> u16 {
    8085
}
fn default_data_dir() -> String {
    "./data".to_string()
}

// Provide a Default impl for WorldConfig so #[serde(default)] works on the
// nested struct inside WorldServerConfig.
impl Default for WorldConfig {
    fn default() -> Self {
        Self {
            update_interval_ms: default_update_interval_ms(),
            max_players: default_max_players(),
            player_save_interval_secs: default_player_save_interval_secs(),
            visibility_distance: default_visibility_distance(),
            motd: default_motd(),
        }
    }
}

// ---------------------------------------------------------------------------
// Load helpers
// ---------------------------------------------------------------------------

impl AuthServerConfig {
    /// Load configuration from a TOML file at `path`, with environment
    /// variable overrides using the `AUTH_` prefix (double-underscore maps to
    /// nested keys, e.g. `AUTH_DATABASE__HOST`).
    pub fn load(path: &str) -> Result<Self, figment::Error> {
        Figment::new()
            .merge(Toml::file(path))
            .merge(Env::prefixed("AUTH_").split("__"))
            .extract()
    }
}

impl WorldServerConfig {
    /// Load configuration from a TOML file at `path`, with environment
    /// variable overrides using the `WORLD_` prefix (double-underscore maps
    /// to nested keys, e.g. `WORLD_WORLD_DATABASE__HOST`).
    pub fn load(path: &str) -> Result<Self, figment::Error> {
        Figment::new()
            .merge(Toml::file(path))
            .merge(Env::prefixed("WORLD_").split("__"))
            .extract()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_config_defaults_from_toml() {
        // Minimal TOML: only the required field (database.database) is set.
        let toml_content = r#"
[database]
database = "realmd"
"#;
        let config: AuthServerConfig = Figment::new()
            .merge(Toml::string(toml_content))
            .extract()
            .expect("should parse minimal auth config");

        assert_eq!(config.bind_address, "0.0.0.0");
        assert_eq!(config.bind_port, 3724);
        assert_eq!(config.database.host, "127.0.0.1");
        assert_eq!(config.database.port, 3306);
        assert_eq!(config.database.user, "mangos");
        assert_eq!(config.database.database, "realmd");
    }

    #[test]
    fn world_config_defaults() {
        let wc = WorldConfig::default();
        assert_eq!(wc.update_interval_ms, 50);
        assert_eq!(wc.max_players, 100);
        assert_eq!(wc.player_save_interval_secs, 900);
        assert!((wc.visibility_distance - 90.0).abs() < f32::EPSILON);
        assert_eq!(wc.motd, "Welcome to CMaNGOS Rust!");
    }
}
