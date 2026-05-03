use anyhow::Context;
use tracing::info;
use tracing_subscriber::EnvFilter;

use wow_config::WorldServerConfig;
use wow_db::create_pool;
use wow_db::{CharacterDeleteMethod, CharacterDeleteOptions};
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

    info!(
        bind = %bind_addr,
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

    let server = WorldServer::new(
        bind_addr,
        login_pool,
        character_pool,
        world_pool,
        delete_options,
        config.data_dir,
    )
    .await
    .context("Failed to initialize world runtime")?;
    info!("World server skeleton is ready. Waiting for connections...");
    server.run().await
}
