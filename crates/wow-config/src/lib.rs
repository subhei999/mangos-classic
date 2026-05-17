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
    #[serde(
        default = "default_map_update_interval_ms",
        alias = "update_interval_ms",
        alias = "MapUpdateInterval"
    )]
    pub map_update_interval_ms: u32,
    #[serde(default = "default_max_players")]
    pub max_players: u32,
    #[serde(default = "default_player_save_interval_secs")]
    pub player_save_interval_secs: u32,
    #[serde(default = "default_visibility_distance")]
    pub visibility_distance: f32,
    #[serde(default = "default_motd")]
    pub motd: String,
    #[serde(default = "default_char_delete_method")]
    pub char_delete_method: u8,
    #[serde(default = "default_char_delete_min_level")]
    pub char_delete_min_level: u8,
    #[serde(default)]
    pub experimental_movement_actor: bool,
    #[serde(default = "default_experimental_movement_actor_queue_capacity")]
    pub experimental_movement_actor_queue_capacity: usize,
    #[serde(default = "default_experimental_movement_actor_max_batch_size")]
    pub experimental_movement_actor_max_batch_size: usize,
}

// ---------------------------------------------------------------------------
// ObservabilityConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct ObservabilityConfig {
    #[serde(default = "default_observability_enabled")]
    pub enabled: bool,
    #[serde(default = "default_observability_bind_address")]
    pub bind_address: String,
    #[serde(default = "default_observability_port")]
    pub bind_port: u16,
}

// ---------------------------------------------------------------------------
// PlayerbotsConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PlayerbotsConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_playerbot_combat_enabled")]
    pub combat_enabled: bool,
    #[serde(default)]
    pub local_roam_only: bool,
    #[serde(default)]
    pub force_active: bool,
    #[serde(default)]
    pub random: PlayerbotRandomConfig,
    #[serde(default)]
    pub travel: PlayerbotTravelConfig,
    #[serde(default)]
    pub bots: Vec<PlayerbotConfig>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PlayerbotTravelConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub map: u32,
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    #[serde(default)]
    pub z: f32,
    #[serde(default)]
    pub radius: f32,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PlayerbotRandomConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub count: u32,
    #[serde(default = "default_playerbot_random_start_guid")]
    pub start_guid: u32,
    #[serde(default = "default_playerbot_random_name_prefix")]
    pub name_prefix: String,
    #[serde(default = "default_playerbot_race")]
    pub race: u8,
    #[serde(default = "default_playerbot_class")]
    pub class: u8,
    #[serde(default)]
    pub gender: u8,
    #[serde(default = "default_playerbot_level")]
    pub level: u8,
    #[serde(default)]
    pub map: u32,
    #[serde(default)]
    pub center_x: f32,
    #[serde(default)]
    pub center_y: f32,
    #[serde(default)]
    pub center_z: f32,
    #[serde(default)]
    pub radius: f32,
    #[serde(default)]
    pub distribution: PlayerbotRandomDistribution,
    #[serde(default = "default_playerbot_random_seed")]
    pub seed: u64,
    #[serde(default)]
    pub player_bytes: u32,
    #[serde(default)]
    pub player_bytes2: u32,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlayerbotRandomDistribution {
    #[default]
    Radius,
    CellScatter,
    GridScatter,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlayerbotConfig {
    pub guid: u32,
    pub name: String,
    #[serde(default = "default_playerbot_race")]
    pub race: u8,
    #[serde(default = "default_playerbot_class")]
    pub class: u8,
    #[serde(default)]
    pub gender: u8,
    #[serde(default = "default_playerbot_level")]
    pub level: u8,
    #[serde(default)]
    pub map: u32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    #[serde(default)]
    pub orientation: f32,
    #[serde(default)]
    pub player_bytes: u32,
    #[serde(default)]
    pub player_bytes2: u32,
}

fn default_playerbot_race() -> u8 {
    1
}
fn default_playerbot_class() -> u8 {
    1
}
fn default_playerbot_level() -> u8 {
    1
}
fn default_playerbot_combat_enabled() -> bool {
    true
}
fn default_playerbot_random_start_guid() -> u32 {
    9_010_000
}
fn default_playerbot_random_name_prefix() -> String {
    "Loadbot".to_string()
}
fn default_playerbot_random_seed() -> u64 {
    1
}

fn default_observability_enabled() -> bool {
    true
}
fn default_observability_bind_address() -> String {
    "127.0.0.1".to_string()
}
fn default_observability_port() -> u16 {
    9091
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            enabled: default_observability_enabled(),
            bind_address: default_observability_bind_address(),
            bind_port: default_observability_port(),
        }
    }
}

fn default_map_update_interval_ms() -> u32 {
    100
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
fn default_char_delete_method() -> u8 {
    0
}
fn default_char_delete_min_level() -> u8 {
    0
}
fn default_experimental_movement_actor_queue_capacity() -> usize {
    1024
}
fn default_experimental_movement_actor_max_batch_size() -> usize {
    64
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
    #[serde(default)]
    pub observability: ObservabilityConfig,
    #[serde(default)]
    pub playerbots: PlayerbotsConfig,
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
            map_update_interval_ms: default_map_update_interval_ms(),
            max_players: default_max_players(),
            player_save_interval_secs: default_player_save_interval_secs(),
            visibility_distance: default_visibility_distance(),
            motd: default_motd(),
            char_delete_method: default_char_delete_method(),
            char_delete_min_level: default_char_delete_min_level(),
            experimental_movement_actor: false,
            experimental_movement_actor_queue_capacity:
                default_experimental_movement_actor_queue_capacity(),
            experimental_movement_actor_max_batch_size:
                default_experimental_movement_actor_max_batch_size(),
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
    pub fn load(path: &str) -> Result<Self, Box<figment::Error>> {
        Figment::new()
            .merge(Toml::file(path))
            .merge(Env::prefixed("AUTH_").split("__"))
            .extract()
            .map_err(Box::new)
    }
}

impl WorldServerConfig {
    /// Load configuration from a TOML file at `path`, with environment
    /// variable overrides using the `WORLD_` prefix (double-underscore maps
    /// to nested keys, e.g. `WORLD_WORLD_DATABASE__HOST`).
    pub fn load(path: &str) -> Result<Self, Box<figment::Error>> {
        Figment::new()
            .merge(Toml::file(path))
            .merge(Env::prefixed("WORLD_").split("__"))
            .extract()
            .map_err(Box::new)
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
        assert_eq!(wc.map_update_interval_ms, 100);
        assert_eq!(wc.max_players, 100);
        assert_eq!(wc.player_save_interval_secs, 900);
        assert!((wc.visibility_distance - 90.0).abs() < f32::EPSILON);
        assert_eq!(wc.motd, "Welcome to CMaNGOS Rust!");
        assert_eq!(wc.char_delete_method, 0);
        assert_eq!(wc.char_delete_min_level, 0);
        assert!(!wc.experimental_movement_actor);
        assert_eq!(wc.experimental_movement_actor_queue_capacity, 1024);
        assert_eq!(wc.experimental_movement_actor_max_batch_size, 64);
    }

    #[test]
    fn world_config_accepts_cmangos_map_update_interval_name() {
        let toml_content = r#"
[world_database]
database = "mangos"

[character_database]
database = "characters"

[login_database]
database = "realmd"

[world]
MapUpdateInterval = 75
"#;
        let config: WorldServerConfig = Figment::new()
            .merge(Toml::string(toml_content))
            .extract()
            .expect("should parse CMaNGOS-shaped map update interval");

        assert_eq!(config.world.map_update_interval_ms, 75);
    }

    #[test]
    fn world_config_accepts_movement_actor_tuning() {
        let toml_content = r#"
[world_database]
database = "mangos"

[character_database]
database = "characters"

[login_database]
database = "realmd"

[world]
experimental_movement_actor = true
experimental_movement_actor_queue_capacity = 2048
experimental_movement_actor_max_batch_size = 128
"#;
        let config: WorldServerConfig = Figment::new()
            .merge(Toml::string(toml_content))
            .extract()
            .expect("should parse movement actor tuning");

        assert!(config.world.experimental_movement_actor);
        assert_eq!(
            config.world.experimental_movement_actor_queue_capacity,
            2048
        );
        assert_eq!(config.world.experimental_movement_actor_max_batch_size, 128);
    }

    #[test]
    fn observability_config_defaults_to_local_metrics_endpoint() {
        let config = ObservabilityConfig::default();
        assert!(config.enabled);
        assert_eq!(config.bind_address, "127.0.0.1");
        assert_eq!(config.bind_port, 9091);
    }

    #[test]
    fn world_config_accepts_playerbot_roster() {
        let toml_content = r#"
bind_address = "127.0.0.1"

[login_database]
database = "realmd"

[world_database]
database = "mangos"

[character_database]
database = "characters"

[playerbots]
enabled = true
combat_enabled = false
local_roam_only = true
force_active = true

[playerbots.random]
enabled = true
count = 511
start_guid = 9010000
name_prefix = "Loadbot"
map = 0
center_x = 0.0
center_y = 0.0
center_z = 83.5
radius = 80.0
distribution = "grid_scatter"
seed = 42

[playerbots.travel]
enabled = true
map = 0
x = -9095.620
y = 422.026
z = 92.0445
radius = 10.0

[[playerbots.bots]]
guid = 9000001
name = "Scoutbot"
x = -8949.0
y = -132.0
z = 83.5
"#;
        let config: WorldServerConfig = Figment::new()
            .merge(Toml::string(toml_content))
            .extract()
            .expect("should parse playerbot roster");

        assert!(config.playerbots.enabled);
        assert!(!config.playerbots.combat_enabled);
        assert!(config.playerbots.local_roam_only);
        assert!(config.playerbots.force_active);
        assert!(config.playerbots.random.enabled);
        assert_eq!(config.playerbots.random.count, 511);
        assert_eq!(config.playerbots.random.start_guid, 9_010_000);
        assert_eq!(config.playerbots.random.name_prefix, "Loadbot");
        assert!((config.playerbots.random.radius - 80.0).abs() < f32::EPSILON);
        assert_eq!(
            config.playerbots.random.distribution,
            PlayerbotRandomDistribution::GridScatter
        );
        assert!(config.playerbots.travel.enabled);
        assert!((config.playerbots.travel.x + 9_095.62).abs() < f32::EPSILON);
        assert!((config.playerbots.travel.radius - 10.0).abs() < f32::EPSILON);
        assert_eq!(config.playerbots.bots.len(), 1);
        assert_eq!(config.playerbots.bots[0].name, "Scoutbot");
        assert_eq!(config.playerbots.bots[0].race, 1);
        assert_eq!(config.playerbots.bots[0].class, 1);
        assert_eq!(config.playerbots.bots[0].level, 1);
    }
}
