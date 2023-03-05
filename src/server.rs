//! This module holds the server definition

use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::io;
use std::net::SocketAddr;

use actix_toolbox::tb_middleware::{setup_logging_mw, LoggingMiddlewareConfig};
use actix_web::middleware::Compress;
use actix_web::web::{Data, JsonConfig, PayloadConfig};
use actix_web::{App, HttpServer};
use log::info;
use tokio::sync::Mutex;

use crate::config::Config;

/// The errors that can occur during server startup
#[derive(Debug)]
pub enum StartServerError {
    /// IO error that can occur
    IO(io::Error),
}

impl Display for StartServerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            StartServerError::IO(err) => write!(f, "{err}"),
        }
    }
}

impl From<io::Error> for StartServerError {
    fn from(value: io::Error) -> Self {
        Self::IO(value)
    }
}

/// This type holds the file data of the game.
///
/// In the original implementation this was written to disk
pub type FileData = Data<Mutex<HashMap<String, Vec<u8>>>>;

/// Start the runciv server
///
/// **Parameter**:
/// - `config`: Reference to a [Config] struct
pub async fn start_server(config: &Config) -> Result<(), StartServerError> {
    let s_addr = SocketAddr::new(config.server.listen_address, config.server.listen_port);

    info!("Starting to listen on {}", s_addr);

    let file_data: FileData = Data::new(Mutex::new(HashMap::new()));

    HttpServer::new(move || {
        App::new()
            .app_data(PayloadConfig::default())
            .app_data(JsonConfig::default())
            .app_data(file_data.clone())
            .wrap(setup_logging_mw(LoggingMiddlewareConfig::default()))
            .wrap(Compress::default())
    })
    .bind(s_addr)?
    .run()
    .await?;

    Ok(())
}
