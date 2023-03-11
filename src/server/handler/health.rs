use actix_web::get;
use actix_web::web::{Data, Json};
use log::error;
use rorm::{query, Database, Model};
use serde::Serialize;
use tokio::sync::oneshot;
use utoipa::ToSchema;

use crate::chan::{WsManagerChan, WsManagerMessage};
use crate::models::Account;
use crate::server::handler::{ApiError, ApiResult};

/// The health data of this server
#[derive(Serialize, ToSchema)]
pub struct HealthResponse {
    #[schema(example = 1337)]
    registered_accounts: u64,
    #[schema(example = 31337)]
    open_connections: u64,
}

/// Request health data from this server.
///
/// `registered_accounts` are the currently registered user accounts on the server
/// `open_connections` are the currently open connections
#[utoipa::path(
    tag = "Server status",
    context_path = "/api/v2/admin",
    responses(
        (status = 200, description = "Health data of this server", body = HealthResponse),
        (status = 400, description = "Client error", body = ApiErrorResponse),
        (status = 500, description = "Server error", body = ApiErrorResponse),
    ),
    security(("admin_token" = []))
)]
#[get("/health")]
pub async fn health(
    db: Data<Database>,
    ws_manager_chan: Data<WsManagerChan>,
) -> ApiResult<Json<HealthResponse>> {
    let accounts = query!(db.as_ref(), (Account::F.uuid.count(),))
        .one()
        .await?
        .0 as u64;

    let (tx, rx) = oneshot::channel();

    let socket_count = tokio::spawn(async move { rx.await });

    if let Err(err) = ws_manager_chan
        .send(WsManagerMessage::RetrieveWsCount(tx))
        .await
    {
        error!("Could not send to ws manager chan: {err}");
        return Err(ApiError::InternalServerError);
    }

    let connections = socket_count
        .await
        .map_err(|err| {
            error!("Unable to join task: {err}");
            ApiError::InternalServerError
        })?
        .map_err(|err| {
            error!("Error receiving message from ws manager chan: {err}");
            ApiError::InternalServerError
        })?;

    Ok(Json(HealthResponse {
        registered_accounts: accounts,
        open_connections: connections,
    }))
}
