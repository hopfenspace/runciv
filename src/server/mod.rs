//! This module holds the server definition

use std::collections::HashMap;
use std::net::SocketAddr;

use actix_toolbox::tb_middleware::{setup_logging_mw, LoggingMiddlewareConfig};
use actix_web::middleware::Compress;
use actix_web::web::{scope, Data, JsonConfig, PayloadConfig};
use actix_web::{App, HttpServer};
use log::info;
use rorm::Database;
use tokio::sync::Mutex;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::config::Config;
use crate::server::error::StartServerError;
use crate::server::handler::register_account;
use crate::server::swagger::ApiDoc;

pub mod error;
pub mod handler;
pub mod swagger;

/// This type holds the file data of the game.
///
/// In the original implementation this was written to disk
pub type FileData = Data<Mutex<HashMap<String, Vec<u8>>>>;

/// Start the runciv server
///
/// **Parameter**:
/// - `config`: Reference to a [Config] struct
pub async fn start_server(config: &Config, db: Database) -> Result<(), StartServerError> {
    let s_addr = SocketAddr::new(config.server.listen_address, config.server.listen_port);

    info!("Starting to listen on {}", s_addr);

    let file_data: FileData = Data::new(Mutex::new(HashMap::new()));

    HttpServer::new(move || {
        App::new()
            .app_data(PayloadConfig::default())
            .app_data(JsonConfig::default())
            .app_data(file_data.clone())
            .app_data(Data::new(db.clone()))
            .wrap(setup_logging_mw(LoggingMiddlewareConfig::default()))
            .wrap(Compress::default())
            .service(SwaggerUi::new("/docs/{_:.*}").url("/api-doc/openapi.json", ApiDoc::openapi()))
            .service(scope("/api/v2").service(register_account))
    })
    .bind(s_addr)?
    .run()
    .await?;

    Ok(())
}
