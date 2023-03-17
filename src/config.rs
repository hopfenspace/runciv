//! This module holds the configuration for the server

use std::net::IpAddr;

use actix_toolbox::logging::LoggingConfig;
use serde::{Deserialize, Serialize};

/// Configuration regarding the server
#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct ServerConfig {
    /// The directory on the local filesystem where to store game data files
    pub game_data_path: String,
    /// The address the server should bind to
    pub listen_address: IpAddr,
    /// The port the server should bind to
    pub listen_port: u16,
    /// Base64 encoded secret key
    ///
    /// The key is used to sign and verify sessions.
    ///
    /// Do not expose this key!
    pub secret_key: String,
    /// The token to access the admin API.
    pub admin_token: String,
}

/// Configuration regarding the database
#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct DBConfig {
    /// Host the database is located on
    pub host: String,
    /// Port the database is located on
    pub port: u16,
    /// The name of the database to connect to.
    pub name: String,
    /// The username to use for the database connection
    pub user: String,
    /// The password to use for the database connection
    pub password: String,
}

/// This struct can be parsed from the configuration file
#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct Config {
    /// Configuration regarding the server
    pub server: ServerConfig,
    /// The logging configuration
    pub logging: LoggingConfig,
    /// The database configuration
    pub database: DBConfig,
}
