//! All handlers for the account endpoints live in here

use actix_web::web::{Data, Json};
use actix_web::{post, HttpResponse};
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHasher};
use rand::thread_rng;
use rorm::{insert, query, Database, Model};
use serde::Deserialize;
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
