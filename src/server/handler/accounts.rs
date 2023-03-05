//! All handlers for the account endpoints live in here

use actix_toolbox::tb_middleware::Session;
use actix_web::web::{Data, Json};
use actix_web::{get, post, HttpResponse};
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHasher};
use rand::thread_rng;
use rorm::{insert, query, Database, Model};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::models::{Account, AccountInsert};
use crate::server::handler::{ApiError, ApiResult};

/// The content to register a new account
#[derive(Debug, Deserialize, ToSchema)]
pub struct AccountRegistrationRequest {
    #[schema(example = "user123")]
    username: String,
    #[schema(example = "Herbert")]
    display_name: String,
    #[schema(example = "super-secure-password")]
    password: String,
}

/// Register a new account
#[utoipa::path(
    tag = "Accounts",
    responses(
        (status = 200, description = "Account got created"),
        (status = 400, description = "Client error", body = ApiErrorResponse),
        (status = 500, description = "Server error", body = ApiErrorResponse),
    ),
    request_body = AccountRegistrationRequest,
)]
#[post("/api/v2/accounts/register")]
pub async fn register_account(
    req: Json<AccountRegistrationRequest>,
    db: Data<Database>,
) -> ApiResult<HttpResponse> {
    let mut tx = db.start_transaction().await?;

    if query!(&db, (Account::F.uuid,))
        .transaction(&mut tx)
        .condition(Account::F.username.equals(&req.username))
        .optional()
        .await?
        .is_some()
    {
        return Err(ApiError::UsernameAlreadyOccupied);
    }

    let salt = SaltString::generate(&mut thread_rng());
    let password_hash = Argon2::default()
        .hash_password(req.password.as_bytes(), &salt)?
        .to_string();

    let uuid = Uuid::new_v4();
    insert!(&db, AccountInsert)
        .transaction(&mut tx)
        .single(&AccountInsert {
            uuid: uuid.as_bytes().to_vec(),
            username: req.username.clone(),
            display_name: req.display_name.clone(),
            password_hash,
            last_login: None,
        })
        .await?;

    tx.commit().await?;

    Ok(HttpResponse::Ok().finish())
}

/// The account data
#[derive(Serialize, ToSchema)]
pub struct AccountResponse {
    #[schema(example = "user123")]
    username: String,
    #[schema(example = "Herbert")]
    display_name: String,
}

/// Returns the account that is currently logged-in
#[utoipa::path(
    tag = "Accounts",
    context_path = "/api/v2",
    responses(
        (status = 200, description = "Returns the account data of the current user", body = AccountResponse),
        (status = 400, description = "Client error", body = ApiErrorResponse),
        (status = 500, description = "Server error", body = ApiErrorResponse),
    ),
    security(("api_key" = []))
)]
#[get("/accounts/me")]
pub async fn get_me(db: Data<Database>, session: Session) -> ApiResult<Json<AccountResponse>> {
    let uuid: Vec<u8> = session.get("uuid")?.ok_or(ApiError::SessionCorrupt)?;

    let account = query!(&db, Account)
        .condition(Account::F.uuid.equals(&uuid))
        .optional()
        .await?
        .ok_or(ApiError::SessionCorrupt)?;

    Ok(Json(AccountResponse {
        username: account.username,
        display_name: account.display_name,
    }))
}
