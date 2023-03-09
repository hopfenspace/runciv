//! This module holds the server definition

use std::collections::HashMap;
use std::net::SocketAddr;

use actix_toolbox::tb_middleware::{
    setup_logging_mw, DBSessionStore, LoggingMiddlewareConfig, PersistentSession, SessionMiddleware,
};
use actix_web::cookie::time::Duration;
use actix_web::cookie::Key;
use actix_web::http::StatusCode;
use actix_web::middleware::{Compress, ErrorHandlers};
use actix_web::web::{scope, Data, JsonConfig, PayloadConfig};
use actix_web::{App, HttpServer};
use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use log::info;
use rorm::Database;
use tokio::sync::Mutex;
use utoipa::OpenApi;
use utoipa_swagger_ui::{SwaggerUi, Url};

use crate::chan::WsManagerChan;
use crate::config::Config;
use crate::server::error::StartServerError;
use crate::server::handler::{
    accept_friend_request, create_friend_request, create_lobby, delete_friend, delete_me,
    get_friends, get_lobbies, get_me, health, login, logout, lookup_account_by_uuid,
    register_account, set_password, update_me, version, websocket, welcome_page,
};
use crate::server::middleware::{
    handle_not_found, json_extractor_error, AuthenticationRequired, TokenRequired,
};
use crate::server::swagger::{AdminApiDoc, ApiDoc};

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
    let key = Key::try_from(
        BASE64_STANDARD
            .decode(&config.server.secret_key)?
            .as_slice(),
    )?;

    let s_addr = SocketAddr::new(config.server.listen_address, config.server.listen_port);
    info!("Starting to listen on {}", s_addr);

    let file_data: FileData = Data::new(Mutex::new(HashMap::new()));
    let admin_token = config.server.admin_token.clone();

    if admin_token.is_empty() {
        return Err(StartServerError::InvalidSecretKey);
    }

    HttpServer::new(move || {
        App::new()
            .app_data(PayloadConfig::default())
            .app_data(JsonConfig::default().error_handler(json_extractor_error))
            .app_data(file_data.clone())
            .app_data(Data::new(db.clone()))
            .app_data(Data::new(ws_manager_chan.clone()))
            .wrap(setup_logging_mw(LoggingMiddlewareConfig::default()))
            .wrap(Compress::default())
            .wrap(
                SessionMiddleware::builder(DBSessionStore::new(db.clone()), key.clone())
                    .session_lifecycle(PersistentSession::session_ttl(
                        PersistentSession::default(),
                        Duration::hours(24),
                    ))
                    .build(),
            )
            .wrap(ErrorHandlers::new().handler(StatusCode::NOT_FOUND, handle_not_found))
            .service(welcome_page)
            .service(SwaggerUi::new("/docs/{_:.*}").urls(vec![
                (
                    Url::new("user-api", "/api-doc/userapi.json"),
                    ApiDoc::openapi(),
                ),
                (
                    Url::new("admin-api", "/api-doc/adminapi.json"),
                    AdminApiDoc::openapi(),
                ),
            ]))
            .service(register_account)
            .service(version)
            .service(scope("/api/v2/auth").service(login).service(logout))
            .service(
                scope("/api/v2/admin")
                    .wrap(TokenRequired(admin_token.clone()))
                    .service(health),
            )
            .service(
                scope("/api/v2")
                    .wrap(AuthenticationRequired)
                    .service(websocket)
                    .service(get_me)
                    .service(delete_me)
                    .service(update_me)
                    .service(set_password)
                    .service(lookup_account_by_uuid)
                    .service(create_friend_request)
                    .service(accept_friend_request)
                    .service(get_friends)
                    .service(delete_friend)
                    .service(get_lobbies)
                    .service(create_lobby),
            )
    })
    .bind(s_addr)?
    .run()
    .await?;

    Ok(())
}
