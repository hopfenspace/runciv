//! This module holds the configuration for the server

use std::net::IpAddr;

use actix_toolbox::logging::LoggingConfig;
use serde::{Deserialize, Serialize};

/// Configuration regarding the server
#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct ServerConfig {
    /// The address the server should bind to
    pub listen_address: IpAddr,
    /// The port the server should bind to
    pub listen_port: u16,
}

/// This struct can be parsed from the configuration file
#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct Config {
    /// Configuration regarding the server
    pub server: ServerConfig,
    /// The logging configuration
    pub logging: LoggingConfig,
}
