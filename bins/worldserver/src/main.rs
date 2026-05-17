use anyhow::Context;
use std::time::Duration;
use tracing::info;
use tracing_subscriber::EnvFilter;

use wow_common::position::WorldPosition;
use wow_config::WorldServerConfig;
use wow_db::create_pool;
use wow_db::{CharacterDeleteMethod, CharacterDeleteOptions};
use wow_network::world::PlayerbotSpawnConfig;
use wow_network::world::WorldServerOptions;
use wow_network::WorldServer;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn print_banner() {
    println!(
        r#"
   ____  __  __       _   _  ____  ___  ____
  / ___|/ _|/ _| __ _| \ | |/ ___|/ _ \/ ___|
 | |   | |_| |_ / _` |  \| | |  _| | | \___ \
 | |___|  _|  _| (_| | |\  | |_| | |_| |___) |
  \____|_| |_|  \__,_|_| \_|\____|\___/|____/
                    Rust World Server v{}
"#,
        VERSION,
    );
    println!("  World of Warcraft 1.12.x (Classic) world server skeleton");
    println!();
}

fn database_url(cfg: &wow_config::DatabaseConfig) -> String {
    format!(
        "mysql://{}:{}@{}:{}/{}",
        cfg.user, cfg.password, cfg.host, cfg.port, cfg.database,
    )
}

fn config_path_from_args() -> anyhow::Result<String> {
    let mut args = std::env::args().skip(1);
    let mut config_path = "config/worldserver.toml".to_string();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" | "-c" => {
                config_path = args.next().context("--config requires a path argument")?;
            }
            "--help" | "-h" => {
                println!("Usage: worldserver [--config <path>]");
                std::process::exit(0);
            }
            other => anyhow::bail!("Unknown argument: {other}"),
        }
    }

    Ok(config_path)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config_path = config_path_from_args()?;

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    print_banner();

    info!("Loading configuration from {}", config_path);
    let config =
        WorldServerConfig::load(&config_path).context("Failed to load world server config")?;

    let bind_addr: std::net::SocketAddr = format!("{}:{}", config.bind_address, config.bind_port)
        .parse()
        .context("Invalid bind address")?;
    let map_update_interval_ms = config.world.map_update_interval_ms;
    if map_update_interval_ms == 0 {
        anyhow::bail!("world.map_update_interval_ms / MapUpdateInterval must be greater than 0");
    }

    info!(
        bind = %bind_addr,
        map_update_interval_ms = map_update_interval_ms,
        observability_enabled = config.observability.enabled,
        observability_bind = %format!("{}:{}", config.observability.bind_address, config.observability.bind_port),
        login_database = %config.login_database.database,
        character_database = %config.character_database.database,
        world_database = %config.world_database.database,
        "Configuration loaded",
    );

    if config.observability.enabled {
        let metrics_bind: std::net::SocketAddr = format!(
            "{}:{}",
            config.observability.bind_address, config.observability.bind_port
        )
        .parse()
        .context("Invalid observability bind address")?;
        tokio::spawn(async move {
            if let Err(error) = wow_network::observability::run_metrics_endpoint(metrics_bind).await
            {
                tracing::error!(%metrics_bind, "Observability metrics endpoint stopped: {error}");
            }
        });
    }

    let login_url = database_url(&config.login_database);
    let login_pool = create_pool(&login_url, 10)
        .await
        .context("Failed to connect to login database")?;

    let character_url = database_url(&config.character_database);
    let character_pool = create_pool(&character_url, 10)
        .await
        .context("Failed to connect to character database")?;

    let world_url = database_url(&config.world_database);
    let world_pool = create_pool(&world_url, 10)
        .await
        .context("Failed to connect to world database")?;

    let delete_options = CharacterDeleteOptions {
        method: match config.world.char_delete_method {
            1 => CharacterDeleteMethod::Unlink,
            _ => CharacterDeleteMethod::HardDelete,
        },
        min_level_for_unlink: config.world.char_delete_min_level,
        force_hard_delete: false,
    };

    let playerbots = playerbot_spawn_configs(&config)?;
    let server = WorldServer::new(
        bind_addr,
        login_pool,
        character_pool,
        world_pool,
        delete_options,
        WorldServerOptions {
            data_dir: config.data_dir.into(),
            world_tick_interval: Duration::from_millis(u64::from(map_update_interval_ms)),
            movement_actor_enabled: config.world.experimental_movement_actor,
            playerbots,
        },
    )
    .await
    .context("Failed to initialize world runtime")?;
    info!("World server skeleton is ready. Waiting for connections...");
    server.run().await
}

fn playerbot_spawn_configs(
    config: &WorldServerConfig,
) -> anyhow::Result<Vec<PlayerbotSpawnConfig>> {
    if !config.playerbots.enabled {
        return Ok(Vec::new());
    }
    if config.playerbots.travel.enabled
        && (!config.playerbots.travel.radius.is_finite() || config.playerbots.travel.radius < 0.0)
    {
        anyhow::bail!("playerbots.travel.radius must be finite and non-negative");
    }

    let mut spawns: Vec<PlayerbotSpawnConfig> = config
        .playerbots
        .bots
        .iter()
        .map(|bot| PlayerbotSpawnConfig {
            guid: bot.guid,
            name: bot.name.clone(),
            race: bot.race,
            class: bot.class,
            gender: bot.gender,
            level: bot.level,
            position: WorldPosition::new(bot.map, bot.x, bot.y, bot.z, bot.orientation),
            travel_destination: playerbot_travel_destination(config, bot.guid),
            player_bytes: bot.player_bytes,
            player_bytes2: bot.player_bytes2,
        })
        .collect();

    let random = &config.playerbots.random;
    if !random.enabled || random.count == 0 {
        return Ok(spawns);
    }

    if !random.radius.is_finite() || random.radius < 0.0 {
        anyhow::bail!("playerbots.random.radius must be finite and non-negative");
    }
    let end_guid = random
        .start_guid
        .checked_add(random.count.saturating_sub(1))
        .context("playerbots.random start_guid + count overflows u32")?;

    let mut rng = DeterministicPlayerbotRng::new(random.seed);
    for guid in random.start_guid..=end_guid {
        let distance = rng.next_unit_f32().sqrt() * random.radius;
        let angle = rng.next_unit_f32() * std::f32::consts::TAU;
        let orientation = rng.next_unit_f32() * std::f32::consts::TAU;
        let index = guid - random.start_guid;
        spawns.push(PlayerbotSpawnConfig {
            guid,
            name: format!("{}{}", random.name_prefix, alphabetic_suffix(index)),
            race: random.race,
            class: random.class,
            gender: random.gender,
            level: random.level,
            position: WorldPosition::new(
                random.map,
                random.center_x + distance * angle.cos(),
                random.center_y + distance * angle.sin(),
                random.center_z,
                orientation,
            ),
            travel_destination: playerbot_travel_destination(config, guid),
            player_bytes: random.player_bytes,
            player_bytes2: random.player_bytes2,
        });
    }

    Ok(spawns)
}

fn playerbot_travel_destination(config: &WorldServerConfig, guid: u32) -> Option<WorldPosition> {
    let travel = &config.playerbots.travel;
    if !travel.enabled {
        return None;
    }
    let mut destination = WorldPosition::new(travel.map, travel.x, travel.y, travel.z, 0.0);
    if travel.radius > f32::EPSILON {
        let mut rng = DeterministicPlayerbotRng::new(playerbot_guid_seed(
            config.playerbots.random.seed,
            guid,
        ));
        let distance = rng.next_unit_f32().sqrt() * travel.radius;
        let angle = rng.next_unit_f32() * std::f32::consts::TAU;
        destination.x += distance * angle.cos();
        destination.y += distance * angle.sin();
    }
    Some(destination)
}

fn playerbot_guid_seed(seed: u64, guid: u32) -> u64 {
    seed ^ u64::from(guid).wrapping_mul(11_400_714_819_323_198_485)
}

struct DeterministicPlayerbotRng {
    state: u64,
}

impl DeterministicPlayerbotRng {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_unit_f32(&mut self) -> f32 {
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.state >> 40) as f32) / ((1u32 << 24) as f32)
    }
}

fn alphabetic_suffix(mut index: u32) -> String {
    let mut suffix = String::new();
    loop {
        let letter = b'a' + (index % 26) as u8;
        suffix.push(char::from(letter));
        index /= 26;
        if index == 0 {
            break;
        }
        index -= 1;
    }
    suffix.chars().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use figment::providers::{Format, Toml};
    use figment::Figment;

    fn test_config(toml_content: &str) -> WorldServerConfig {
        Figment::new()
            .merge(Toml::string(toml_content))
            .extract()
            .expect("should parse world config")
    }

    #[test]
    fn playerbot_spawn_configs_adds_deterministic_random_area() {
        let config = test_config(
            r#"
[login_database]
database = "realmd"

[world_database]
database = "mangos"

[character_database]
database = "characters"

[playerbots]
enabled = true

[playerbots.random]
enabled = true
count = 3
start_guid = 9010000
name_prefix = "Loadbot"
map = 0
center_x = -8949.0
center_y = -132.0
center_z = 83.5
radius = 80.0
seed = 42

[playerbots.travel]
enabled = true
map = 0
x = -9095.620
y = 422.026
z = 92.0445
radius = 10.0
"#,
        );

        let spawns = playerbot_spawn_configs(&config).expect("random bot spawns");

        assert_eq!(spawns.len(), 3);
        assert_eq!(spawns[0].guid, 9_010_000);
        assert_eq!(spawns[0].name, "Loadbota");
        assert_eq!(spawns[1].name, "Loadbotb");
        assert_eq!(spawns[2].name, "Loadbotc");
        assert!(spawns.iter().all(|spawn| spawn.position.map_id == 0));
        assert!(spawns
            .iter()
            .all(|spawn| (spawn.position.z - 83.5).abs() < f32::EPSILON));
        assert!(spawns.iter().all(|spawn| {
            let dx = spawn.position.x + 8949.0;
            let dy = spawn.position.y + 132.0;
            (dx * dx + dy * dy).sqrt() <= 80.0
        }));
        assert!(spawns.iter().all(|spawn| {
            spawn.travel_destination.is_some_and(|destination| {
                if destination.map_id != 0 {
                    return false;
                }
                let dx = destination.x + 9_095.62;
                let dy = destination.y - 422.026;
                (dx * dx + dy * dy).sqrt() <= 10.0
            })
        }));
        assert!(spawns.iter().any(|spawn| {
            spawn.travel_destination.is_some_and(|destination| {
                let dx = destination.x + 9_095.62;
                let dy = destination.y - 422.026;
                (dx * dx + dy * dy).sqrt() > 0.01
            })
        }));

        let second_pass = playerbot_spawn_configs(&config).expect("random bot spawns");
        assert_eq!(spawns[0].position.x, second_pass[0].position.x);
        assert_eq!(spawns[0].position.y, second_pass[0].position.y);
        assert_eq!(
            spawns[0].position.orientation,
            second_pass[0].position.orientation
        );
        assert_eq!(
            spawns[0].travel_destination,
            second_pass[0].travel_destination
        );
    }

    #[test]
    fn playerbot_spawn_configs_rejects_invalid_random_radius() {
        let config = test_config(
            r#"
[login_database]
database = "realmd"

[world_database]
database = "mangos"

[character_database]
database = "characters"

[playerbots]
enabled = true

[playerbots.random]
enabled = true
count = 1
radius = -1.0
"#,
        );

        let error = playerbot_spawn_configs(&config).expect_err("invalid radius");
        assert!(error
            .to_string()
            .contains("playerbots.random.radius must be finite and non-negative"));
    }

    #[test]
    fn playerbot_spawn_configs_rejects_invalid_travel_radius() {
        let config = test_config(
            r#"
[login_database]
database = "realmd"

[world_database]
database = "mangos"

[character_database]
database = "characters"

[playerbots]
enabled = true

[playerbots.travel]
enabled = true
radius = -1.0
"#,
        );

        let error = playerbot_spawn_configs(&config).expect_err("invalid radius");
        assert!(error
            .to_string()
            .contains("playerbots.travel.radius must be finite and non-negative"));
    }
}
