//! # runciv
//!
//! runciv is a server implementation for [unciv](https://github.com/yairm210/Unciv)
#![warn(missing_docs, unused_imports, clippy::unwrap_used, clippy::expect_used)]
#![cfg_attr(
    feature = "rorm-main",
    allow(dead_code, unused_variables, unused_imports)
)]

use std::fs::read_to_string;
use std::path::Path;

use actix_toolbox::logging::setup_logging;
use actix_web::cookie::Key;
use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use clap::{Parser, Subcommand};
use log::{error, info};
use rorm::cli::config as cli_config;
use rorm::{cli, Database, DatabaseConfiguration, DatabaseDriver};

use crate::chan::start_ws_manager;
use crate::config::Config;
use crate::server::start_server;

pub mod chan;
pub mod config;
pub mod models;
pub mod server;

/// The possible commands for runciv
#[derive(Subcommand)]
pub enum Command {
    /// Start the server
    Start,
    /// Generate a secret key
    Keygen,
    /// Run database migrations
    Migrate {
        /// The directory where the migrations are located
        migration_dir: String,
    },
}

/// The cli parser for runciv
#[derive(Parser)]
#[clap(version, about = "An unciv server")]
pub struct Cli {
    #[clap(long = "config-path")]
    #[clap(help = "Specify an alternative path to the config file")]
    #[clap(default_value_t = String::from("/etc/runciv/config.toml"))]
    config_path: String,

    #[clap(subcommand)]
    command: Command,
}

#[rorm::rorm_main]
#[tokio::main]
async fn main() -> Result<(), String> {
    let cli = Cli::parse();

    match cli.command {
        Command::Start => {
            let conf = get_conf(&cli.config_path)?;

            setup_logging(&conf.logging)?;

            let db = get_db(&conf).await?;
            info!("Connected to database");

            let ws_manager_chan = start_ws_manager(db.clone()).await?;

            if let Err(err) = start_server(&conf, db, ws_manager_chan).await {
                error!("Error while starting server: {err}");
                return Err(err.to_string());
            }
        }
        Command::Keygen => {
            let key = Key::generate();
            println!("{}", BASE64_STANDARD.encode(key.master()));
        }
        Command::Migrate { migration_dir } => {
            let conf = get_conf(&cli.config_path)?;
            cli::migrate::run_migrate_custom(
                cli_config::DatabaseConfig {
                    last_migration_table_name: None,
                    driver: DatabaseDriver::Postgres {
                        host: conf.database.host,
                        port: conf.database.port,
                        name: conf.database.name,
                        user: conf.database.user,
                        password: conf.database.password,
                    },
                },
                migration_dir,
                false,
                None,
            )
            .await
            .map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

/// Retrieve a [Config] by Path
///
/// **Parameter**:
/// - `config_path`: [&str]
fn get_conf(config_path: &str) -> Result<Config, String> {
    let path = Path::new(config_path);

    if !path.exists() {
        return Err(format!("File {config_path} does not exist"));
    }

    if !path.is_file() {
        return Err(format!("{config_path} is a directory"));
    }

    let config_str =
        read_to_string(path).map_err(|err| format!("Could not read config file: {err}"))?;

    let config: Config =
        toml::from_str(&config_str).map_err(|err| format!("Could not parse config file: {err}"))?;

    Ok(config)
}

/// Retrieves the database using the provided config.
///
/// If the connection fails, an error is returned
async fn get_db(config: &Config) -> Result<Database, String> {
    let c = DatabaseConfiguration {
        driver: DatabaseDriver::Postgres {
            host: config.database.host.clone(),
            port: config.database.port,
            name: config.database.name.clone(),
            user: config.database.user.clone(),
            password: config.database.password.clone(),
        },
        min_connections: 2,
        max_connections: 20,
        disable_logging: Some(true),
        statement_log_level: None,
        slow_statement_log_level: None,
    };

    Database::connect(c)
        .await
        .map_err(|e| format!("Error connecting to database: {e}"))
}
