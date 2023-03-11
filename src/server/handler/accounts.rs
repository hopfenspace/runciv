//! All handlers for the account endpoints live in here

use actix_toolbox::tb_middleware::Session;
use actix_web::web::{Data, Json, Path};
use actix_web::{delete, get, post, put, HttpResponse};
use argon2::password_hash::{Error, SaltString};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use log::error;
use rand::thread_rng;
use rorm::{insert, query, update, Database, Model};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::chan::{WsManagerChan, WsManagerMessage};
use crate::models::{Account, AccountInsert};
use crate::server::handler::{ApiError, ApiResult, PathUuid};

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

    if req.username.is_empty() {
        return Err(ApiError::InvalidUsername);
    }

    if req.display_name.is_empty() {
        return Err(ApiError::InvalidDisplayName);
    }

    if query!(&mut tx, (Account::F.uuid,))
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
    insert!(&mut tx, AccountInsert)
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
#[derive(Serialize, Deserialize, ToSchema, Eq, Ord, PartialOrd, PartialEq, Clone, Debug)]
pub struct AccountResponse {
    pub(crate) uuid: Uuid,
    #[schema(example = "user123")]
    pub(crate) username: String,
    #[schema(example = "Herbert")]
    pub(crate) display_name: String,
}

/// The account data
#[derive(Serialize, ToSchema)]
pub struct OnlineAccountResponse {
    pub(crate) online: bool,
    pub(crate) uuid: Uuid,
    #[schema(example = "user123")]
    pub(crate) username: String,
    #[schema(example = "Herbert")]
    pub(crate) display_name: String,
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
    security(("session_cookie" = []))
)]
#[get("/accounts/me")]
pub async fn get_me(db: Data<Database>, session: Session) -> ApiResult<Json<AccountResponse>> {
    let uuid: Vec<u8> = session.get("uuid")?.ok_or(ApiError::SessionCorrupt)?;

    let account = query!(db.as_ref(), Account)
        .condition(Account::F.uuid.equals(&uuid))
        .optional()
        .await?
        .ok_or(ApiError::SessionCorrupt)?;

    Ok(Json(AccountResponse {
        uuid: Uuid::from_slice(&account.uuid).map_err(|_| ApiError::InternalServerError)?,
        username: account.username,
        display_name: account.display_name,
    }))
}

/// Deletes the currently logged-in account
#[utoipa::path(
    tag = "Accounts",
    context_path = "/api/v2",
    responses(
        (status = 200, description = "Deleted the currently logged-in account"),
        (status = 400, description = "Client error", body = ApiErrorResponse),
        (status = 500, description = "Server error", body = ApiErrorResponse),
    ),
    security(("session_cookie" = []))
)]
#[delete("/accounts/me")]
pub async fn delete_me(
    db: Data<Database>,
    session: Session,
    ws_manager_chan: Data<WsManagerChan>,
) -> ApiResult<HttpResponse> {
    let uuid: Vec<u8> = session.get("uuid")?.ok_or(ApiError::SessionCorrupt)?;

    rorm::delete!(db.as_ref(), Account)
        .condition(Account::F.uuid.equals(&uuid))
        .await?;

    // Clear the current session
    session.purge();

    // Close open websocket connections
    if let Err(err) = ws_manager_chan
        .send(WsManagerMessage::CloseSocket(uuid))
        .await
    {
        error!("Could not send to ws manager chan: {err}");
    }

    Ok(HttpResponse::Ok().finish())
}

/// The set password request data
///
/// The parameter `new_password` must not be empty
#[derive(Deserialize, ToSchema)]
pub struct SetPasswordRequest {
    #[schema(example = "super-secure-password")]
    old_password: String,
    #[schema(example = "ultra-secure-password!!11!")]
    new_password: String,
}

/// Sets a new password for the currently logged-in account
#[utoipa::path(
    tag = "Accounts",
    context_path = "/api/v2",
    responses(
        (status = 200, description = "New password has been set"),
        (status = 400, description = "Client error", body = ApiErrorResponse),
        (status = 500, description = "Server error", body = ApiErrorResponse),
    ),
    request_body = SetPasswordRequest,
    security(("session_cookie" = []))
)]
#[post("/accounts/me/setPassword")]
pub async fn set_password(
    req: Json<SetPasswordRequest>,
    db: Data<Database>,
    session: Session,
) -> ApiResult<HttpResponse> {
    let uuid: Vec<u8> = session.get("uuid")?.ok_or(ApiError::SessionCorrupt)?;

    if req.new_password.is_empty() {
        return Err(ApiError::InvalidPassword);
    }

    let mut tx = db.start_transaction().await?;

    let (pw_hash,) = query!(&mut tx, (Account::F.password_hash,))
        .condition(Account::F.uuid.equals(&uuid))
        .optional()
        .await?
        .ok_or(ApiError::SessionCorrupt)?;

    Argon2::default()
        .verify_password(req.old_password.as_bytes(), &PasswordHash::new(&pw_hash)?)
        .map_err(|e| match e {
            Error::Password => ApiError::LoginFailed,
            _ => ApiError::InvalidHash(e),
        })?;

    let salt = SaltString::generate(&mut thread_rng());
    let password_hash = Argon2::default()
        .hash_password(req.new_password.as_bytes(), &salt)?
        .to_string();

    update!(&mut tx, Account)
        .condition(Account::F.uuid.equals(&uuid))
        .set(Account::F.password_hash, &password_hash)
        .exec()
        .await?;

    tx.commit().await?;

    Ok(HttpResponse::Ok().finish())
}

/// Update account request data
///
/// All parameter are optional, but at least one of them is required.
#[derive(Deserialize, ToSchema)]
pub struct UpdateAccountRequest {
    #[schema(example = "user321")]
    username: Option<String>,
    #[schema(example = "Heeeerbeeeert")]
    display_name: Option<String>,
}

/// Updates the currently logged-in account
///
/// All parameter are optional, but at least one of them is required.
#[utoipa::path(
    tag = "Accounts",
    context_path = "/api/v2",
    responses(
        (status = 200, description = "Account has been updated"),
        (status = 400, description = "Client error", body = ApiErrorResponse),
        (status = 500, description = "Server error", body = ApiErrorResponse),
    ),
    request_body = UpdateAccountRequest,
    security(("session_cookie" = []))
)]
#[put("/accounts/me")]
pub async fn update_me(
    req: Json<UpdateAccountRequest>,
    db: Data<Database>,
    session: Session,
) -> ApiResult<HttpResponse> {
    let uuid: Vec<u8> = session.get("uuid")?.ok_or(ApiError::SessionCorrupt)?;

    let mut tx = db.start_transaction().await?;

    if let Some(username) = &req.username {
        if username.is_empty() {
            return Err(ApiError::InvalidUsername);
        }

        if query!(&mut tx, Account)
            .condition(Account::F.username.equals(username))
            .optional()
            .await?
            .is_some()
        {
            return Err(ApiError::UsernameAlreadyOccupied);
        }
    }

    if let Some(display_name) = &req.display_name {
        if display_name.is_empty() {
            return Err(ApiError::InvalidDisplayName);
        }
    }

    update!(&mut tx, Account)
        .condition(Account::F.uuid.equals(&uuid))
        .begin_dyn_set()
        .set_if(Account::F.username, req.username.as_ref())
        .set_if(Account::F.display_name, req.display_name.as_ref())
        .finish_dyn_set()
        .map_err(|_| ApiError::EmptyJson)?
        .exec()
        .await?;

    tx.commit().await?;

    Ok(HttpResponse::Ok().finish())
}

/// Retrieve details for an account by uuid
///
/// As usernames are changeable, accounts are identified by uuids, which are used throughout
/// the API.
///
/// To fetch `display_name` and `username` for a given `uuid`, this endpoint shall be used.
#[utoipa::path(
    tag = "Accounts",
    context_path = "/api/v2",
    responses(
        (status = 200, description = "Returns the requested account data", body = AccountResponse),
        (status = 400, description = "Client error", body = ApiErrorResponse),
        (status = 500, description = "Server error", body = ApiErrorResponse),
    ),
    params(PathUuid),
    security(("session_cookie" = [])))]
#[get("/accounts/{uuid}")]
pub async fn lookup_account_by_uuid(
    req: Path<PathUuid>,
    db: Data<Database>,
) -> ApiResult<Json<AccountResponse>> {
    let account = query!(db.as_ref(), Account)
        .condition(Account::F.uuid.equals(req.uuid.as_ref()))
        .optional()
        .await?
        .ok_or(ApiError::InvalidUuid)?;

    Ok(Json(AccountResponse {
        uuid: req.uuid,
        username: account.username,
        display_name: account.display_name,
    }))
}

/// The request to lookup an account by its username
#[derive(Deserialize, ToSchema)]
pub struct LookupAccountUsernameRequest {
    username: String,
}

/// Retrieve details for an account by its username
///
/// **Important note**:
///
/// Usernames can be changed, so don't assume you can cache them to do lookups for their
/// display names or uuids when necessary. They solely exist to provide a good user experience
/// when searching for friends, etc..
///
/// If you receive a username by a user, you should convert them with this endpoint to an uuid.
/// Those are used in the database to uniquely identify a user and can't be changed, just deleted.
#[utoipa::path(
    tag = "Accounts", 
    context_path = "/api/v2",    
    responses(
        (status = 200, description = "Returns the requested account data", body = AccountResponse),
        (status = 400, description = "Client error", body = ApiErrorResponse),
        (status = 500, description = "Server error", body = ApiErrorResponse),
    ),
    request_body = LookupAccountUsernameRequest,
    security(("session_cookie" = []))
)]
#[post("/accounts/lookup")]
pub async fn lookup_account_by_username(
    req: Json<LookupAccountUsernameRequest>,
    db: Data<Database>,
) -> ApiResult<Json<AccountResponse>> {
    let account = query!(db.as_ref(), Account)
        .condition(Account::F.username.equals(&req.username))
        .optional()
        .await?
        .ok_or(ApiError::InvalidUsername)?;

    Ok(Json(AccountResponse {
        uuid: Uuid::from_slice(&account.uuid).map_err(|err| {
            error!("Retrieved invalid uuid from db: {err}");
            ApiError::InternalServerError
        })?,
        username: account.username,
        display_name: account.display_name,
    }))
}
