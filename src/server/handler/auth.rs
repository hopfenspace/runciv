//! This module holds all endpoints regarding authentication

use actix_toolbox::tb_middleware::Session;
use actix_web::web::{Data, Json};
use actix_web::{get, post, HttpResponse};
use argon2::password_hash::Error;
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use chrono::Utc;
use log::error;
use rorm::{query, update, Database, Model};
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::chan::{WsManagerChan, WsManagerMessage};
use crate::models::Account;
use crate::server::handler::{ApiError, ApiResult};

/// The request data of a login request
#[derive(ToSchema, Deserialize)]
pub struct LoginRequest {
    #[schema(example = "user123")]
    username: String,
    #[schema(example = "super-secure-password")]
    password: String,
}

/// Login to runciv
///
/// On successful login you will retrieve a cookie.
#[utoipa::path(
    tag = "Authentication",
    context_path = "/api/v2/auth",
    responses(
        (status = 200, description = "Login successful"),
        (status = 400, description = "Client error", body = ApiErrorResponse),
        (status = 500, description = "Server error", body = ApiErrorResponse)
    ),
    request_body = LoginRequest,
)]
#[post("/login")]
pub(crate) async fn login(
    req: Json<LoginRequest>,
    db: Data<Database>,
    session: Session,
) -> ApiResult<HttpResponse> {
    let mut tx = db.start_transaction().await?;

    let user = query!(&mut tx, Account)
        .condition(Account::F.username.equals(&req.username))
        .optional()
        .await?
        .ok_or(ApiError::LoginFailed)?;

    Argon2::default()
        .verify_password(
            req.password.as_bytes(),
            &PasswordHash::new(&user.password_hash)?,
        )
        .map_err(|e| match e {
            Error::Password => ApiError::LoginFailed,
            _ => ApiError::InvalidHash(e),
        })?;

    update!(&mut tx, Account)
        .condition(Account::F.uuid.equals(user.uuid.as_ref()))
        .set(Account::F.last_login, Some(Utc::now().naive_utc()))
        .exec()
        .await?;

    tx.commit().await?;

    session.insert("uuid", user.uuid)?;
    session.insert("logged_in", true)?;

    Ok(HttpResponse::Ok().finish())
}

/// Log out of this session
///
/// Logs a logged-in user out of his session.
#[utoipa::path(
    tag = "Authentication",
    context_path = "/api/v2/auth",
    responses(
        (status = 200, description = "Logout successful"),
        (status = 400, description = "Client error", body = ApiErrorResponse),
        (status = 500, description = "Server error", body = ApiErrorResponse)
    ),
)]
#[get("/logout")]
pub(crate) async fn logout(
    session: Session,
    ws_manager_chan: Data<WsManagerChan>,
) -> ApiResult<HttpResponse> {
    let uuid: Uuid = session.get("uuid")?.ok_or(ApiError::SessionCorrupt)?;

    session.purge();

    if let Err(err) = ws_manager_chan
        .send(WsManagerMessage::CloseSocket(uuid))
        .await
    {
        error!("Could not send to ws manager chan: {err}");
    }

    Ok(HttpResponse::Ok().finish())
}
