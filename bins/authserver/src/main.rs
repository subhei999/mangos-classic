use anyhow::Context;
use tracing::info;
use tracing_subscriber::EnvFilter;

use wow_config::AuthServerConfig;
use wow_db::create_pool;
use wow_network::AuthServer;

// ---------------------------------------------------------------------------
// Banner
// ---------------------------------------------------------------------------

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn print_banner() {
    println!(
        r#"
   ____  __  __       _   _  ____  ___  ____
  / ___|/ _|/ _| __ _| \ | |/ ___|/ _ \/ ___|
 | |   | |_| |_ / _` |  \| | |  _| | | \___ \
 | |___|  _|  _| (_| | |\  | |_| | |_| |___) |
  \____|_| |_|  \__,_|_| \_|\____|\___/|____/
                       Rust Auth Server v{}
"#,
        VERSION,
    );
    println!("  World of Warcraft 1.12.x (Classic) authentication server");
    println!();
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a MySQL connection URL from the config's DatabaseConfig fields.
fn database_url(cfg: &wow_config::DatabaseConfig) -> String {
    format!(
        "mysql://{}:{}@{}:{}/{}",
        cfg.user, cfg.password, cfg.host, cfg.port, cfg.database,
    )
}

fn config_path_from_args() -> anyhow::Result<String> {
    let mut args = std::env::args().skip(1);
    let mut config_path = "config/authserver.toml".to_string();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" | "-c" => {
                config_path = args.next().context("--config requires a path argument")?;
            }
            "--help" | "-h" => {
                println!("Usage: authserver [--config <path>]");
                std::process::exit(0);
            }
            other => anyhow::bail!("Unknown argument: {other}"),
        }
    }

    Ok(config_path)
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // --- Parse CLI args ---------------------------------------------------
    let config_path = config_path_from_args()?;

    // --- Tracing ----------------------------------------------------------
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    // --- Banner -----------------------------------------------------------
    print_banner();

    // --- Configuration ----------------------------------------------------
    info!("Loading configuration from {}", config_path);
    let config =
        AuthServerConfig::load(&config_path).context("Failed to load auth server configuration")?;

    info!(
        bind = %format!("{}:{}", config.bind_address, config.bind_port),
        database = %config.database.database,
        "Configuration loaded",
    );

    // --- Database ---------------------------------------------------------
    let url = database_url(&config.database);
    info!("Connecting to realmd database...");
    let pool = create_pool(&url, 10)
        .await
        .context("Failed to connect to realmd database")?;
    info!("Database connection established.");

    // --- Server -----------------------------------------------------------
    let bind_addr: std::net::SocketAddr = format!("{}:{}", config.bind_address, config.bind_port)
        .parse()
        .context("Invalid bind address")?;

    let server = AuthServer::new(bind_addr, pool);

    info!("Auth server is ready. Waiting for connections...");
    server.run().await
}
