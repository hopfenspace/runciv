//! This module holds the server definition

use std::collections::HashMap;
use std::net::SocketAddr;

use actix_toolbox::tb_middleware::{setup_logging_mw, LoggingMiddlewareConfig};
use actix_web::http::StatusCode;
use actix_web::middleware::{Compress, ErrorHandlers};
use actix_web::web::{scope, Data, JsonConfig, PayloadConfig};
use actix_web::{App, HttpServer};
use log::info;
use rorm::Database;
use tokio::sync::Mutex;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::chan::WsManagerChan;
use crate::config::Config;
use crate::server::error::StartServerError;
use crate::server::handler::{login, logout, register_account, websocket};
use crate::server::middleware::{handle_not_found, json_extractor_error, AuthenticationRequired};
use crate::server::swagger::ApiDoc;

pub mod error;
pub mod handler;
pub mod middleware;
pub mod swagger;

/// This type holds the file data of the game.
///
/// In the original implementation this was written to disk
pub type FileData = Data<Mutex<HashMap<String, Vec<u8>>>>;

/// Start the runciv server
///
/// **Parameter**:
/// - `config`: Reference to a [Config] struct
/// - `db`: [Database]
/// - `ws_manager_chan`: [WsManagerChan] : The channel to manage websocket connections
pub async fn start_server(
    config: &Config,
    db: Database,
    ws_manager_chan: WsManagerChan,
) -> Result<(), StartServerError> {
    let s_addr = SocketAddr::new(config.server.listen_address, config.server.listen_port);

    info!("Starting to listen on {}", s_addr);

    let file_data: FileData = Data::new(Mutex::new(HashMap::new()));

    HttpServer::new(move || {
        App::new()
            .app_data(PayloadConfig::default())
            .app_data(JsonConfig::default().error_handler(json_extractor_error))
            .app_data(file_data.clone())
            .app_data(Data::new(db.clone()))
            .app_data(Data::new(ws_manager_chan.clone()))
            .wrap(setup_logging_mw(LoggingMiddlewareConfig::default()))
            .wrap(Compress::default())
            .wrap(ErrorHandlers::new().handler(StatusCode::NOT_FOUND, handle_not_found))
            .service(SwaggerUi::new("/docs/{_:.*}").url("/api-doc/openapi.json", ApiDoc::openapi()))
            .service(register_account)
            .service(scope("/api/v2/auth").service(login).service(logout))
            .service(
                scope("/api/v1")
                    .wrap(AuthenticationRequired)
                    .service(websocket),
            )
    })
    .bind(s_addr)?
    .run()
    .await?;

    Ok(())
}
