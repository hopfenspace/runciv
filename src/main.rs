//! # runciv
//!
//! runciv is a server implementation for [unciv](https://github.com/yairm210/Unciv)
#![warn(missing_docs)]

use std::fs::read_to_string;
use std::path::Path;

use actix_toolbox::logging::setup_logging;
use clap::{Parser, Subcommand};
use log::error;

use crate::config::Config;
use crate::server::start_server;

pub mod config;
pub mod handler;
pub mod server;

/// The possible commands for runciv
#[derive(Subcommand)]
pub enum Command {
    /// Start the server
    Start,
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

#[tokio::main]
async fn main() -> Result<(), String> {
    let cli = Cli::parse();

    match cli.command {
        Command::Start => {
            let conf = get_conf(&cli.config_path)?;

            setup_logging(&conf.logging)?;

            if let Err(err) = start_server(&conf).await {
                error!("Error while starting server: {err}");
                return Err(err.to_string());
            }
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
