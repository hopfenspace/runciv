//! This module holds the handler of runciv

use std::fmt::{Display, Formatter};

use actix_toolbox::tb_middleware::actix_session;
use actix_web::body::BoxBody;
use actix_web::HttpResponse;
use log::{debug, error, info, trace, warn};
use serde::{Deserialize, Serialize};
use serde_repr::Serialize_repr;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

pub use crate::server::handler::accounts::*;
pub use crate::server::handler::auth::*;
pub use crate::server::handler::chats::*;
pub use crate::server::handler::friends::*;
pub use crate::server::handler::health::*;
pub use crate::server::handler::lobbies::*;
pub use crate::server::handler::version::*;
pub use crate::server::handler::websocket::*;
pub use crate::server::handler::welcome_page::*;

mod accounts;
mod auth;
mod chats;
mod friends;
mod health;
mod lobbies;
mod version;
mod websocket;
mod welcome_page;

/// The result that is used throughout the complete api.
pub type ApiResult<T> = Result<T, ApiError>;

/// The status code represents a unique identifier for an error.
///
/// Error codes in the range of 1000..2000 represent client errors
/// that could be handled by the client.
/// Error codes in the range of 2000..3000 represent server errors.
#[derive(Serialize_repr, ToSchema)]
#[repr(u16)]
pub(crate) enum ApiStatusCode {
    Unauthenticated = 1000,
    NotFound = 1001,
    InvalidContentType = 1002,
    InvalidJson = 1003,
    PayloadOverflow = 1004,

    LoginFailed = 1005,
    UsernameAlreadyOccupied = 1006,
    InvalidPassword = 1007,
    EmptyJson = 1008,
    InvalidUsername = 1009,
    InvalidDisplayName = 1010,
    FriendshipAlreadyRequested = 1011,
    AlreadyFriends = 1012,
    InvalidId = 1013,
    MissingPrivileges = 1014,
    InvalidMaxPlayersCount = 1017,
    AlreadyInALobby = 1018,
    InvalidUuid = 1019,

    InternalServerError = 2000,
    DatabaseError = 2001,
    SessionError = 2002,
}

/// Parameter for accessing resources by path via uuid
#[derive(Deserialize, IntoParams)]
pub struct PathUuid {
    pub(crate) uuid: Uuid,
}

/// The Response that is returned in case of an error
///
/// For client errors the HTTP status code will be 400,
/// for server errors the 500 will be used.
#[derive(Serialize, ToSchema)]
pub(crate) struct ApiErrorResponse {
    #[schema(example = "Error message is here")]
    message: String,
    #[schema(example = 1000)]
    status_code: ApiStatusCode,
}

impl ApiErrorResponse {
    fn new(status_code: ApiStatusCode, message: String) -> Self {
        Self {
            message,
            status_code,
        }
    }
}

/// This enum holds all possible error types that can occur in the API
#[derive(Debug)]
pub enum ApiError {
    /// The user is not allowed to access the resource
    Unauthenticated,
    /// The resource was not found
    NotFound,
    /// Invalid content type sent
    InvalidContentType,
    /// Json error
    InvalidJson(serde_json::Error),
    /// Payload overflow
    PayloadOverflow(String),

    /// Login was not successful. Can be caused by incorrect username / password
    LoginFailed,
    /// The username is already occupied
    UsernameAlreadyOccupied,
    /// Invalid password (e.g. empty)
    InvalidPassword,
    /// Found an empty json
    EmptyJson,
    /// Invalid username was specified (e.g. empty)
    InvalidUsername,
    /// Invalid display name was specified (e.g. empty)
    InvalidDisplayName,
    /// Friendship was already requested, but is not accepted yet
    FriendshipAlreadyRequested,
    /// Users are already friends
    AlreadyFriends,
    /// Invalid id specified
    InvalidId,
    /// Missing privileges to execute this operation
    MissingPrivileges,
    /// Invalid max_players count
    InvalidMaxPlayersCount,
    /// The executing user is already in a lobby
    AlreadyInALobby,
    /// The provided uuid was not valid
    InvalidUuid,

    /// Unknown error occurred
    InternalServerError,
    /// All errors that are thrown by the database
    DatabaseError(rorm::Error),
    /// An invalid hash is retrieved from the database
    InvalidHash(argon2::password_hash::Error),
    /// Error inserting into a session
    SessionInsert(actix_session::SessionInsertError),
    /// Error retrieving data from a session
    SessionGet(actix_session::SessionGetError),
    /// Session is in a corrupt state
    SessionCorrupt,
}

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::LoginFailed => write!(f, "The login was not successful"),
            ApiError::DatabaseError(_) => write!(f, "Database error occurred"),
            ApiError::UsernameAlreadyOccupied => write!(f, "Username is already occupied"),
            ApiError::Unauthenticated => write!(f, "Unauthenticated"),
            ApiError::InvalidHash(_) => write!(f, "Internal server error"),
            ApiError::InternalServerError | ApiError::NotFound => {
                write!(f, "The resource was not found")
            }
            ApiError::InvalidContentType => write!(f, "Content type error"),
            ApiError::InvalidJson(err) => write!(f, "Json error: {err}"),
            ApiError::PayloadOverflow(err) => write!(f, "{err}"),
            ApiError::SessionInsert(_) | ApiError::SessionGet(_) => {
                write!(f, "Session error occurred")
            }
            ApiError::SessionCorrupt => write!(f, "Corrupt session"),
            ApiError::InvalidPassword => write!(f, "Invalid password"),
            ApiError::EmptyJson => write!(f, "Empty json found"),
            ApiError::InvalidUsername => write!(f, "Invalid username"),
            ApiError::InvalidDisplayName => write!(f, "Invalid display name"),
            ApiError::FriendshipAlreadyRequested => write!(f, "Friendship was already requested"),
            ApiError::AlreadyFriends => write!(f, "You are already friends"),
            ApiError::InvalidId => write!(f, "Invalid id specified"),
            ApiError::MissingPrivileges => {
                write!(f, "Missing privileges to execute this operation")
            }
            ApiError::InvalidMaxPlayersCount => write!(f, "Invalid max_players count"),
            ApiError::AlreadyInALobby => write!(f, "Already in a lobby"),
            ApiError::InvalidUuid => write!(f, "The provided uuid was not valid"),
        }
    }
}

impl actix_web::ResponseError for ApiError {
    fn error_response(&self) -> HttpResponse<BoxBody> {
        match self {
            ApiError::SessionInsert(err) => {
                error!("Session insert error: {err}");

                HttpResponse::InternalServerError().json(ApiErrorResponse::new(
                    ApiStatusCode::SessionError,
                    self.to_string(),
                ))
            }
            ApiError::SessionGet(err) => {
                error!("Session get error: {err}");

                HttpResponse::InternalServerError().json(ApiErrorResponse::new(
                    ApiStatusCode::SessionError,
                    self.to_string(),
                ))
            }
            ApiError::Unauthenticated => {
                trace!("Unauthenticated");

                HttpResponse::BadRequest().json(ApiErrorResponse::new(
                    ApiStatusCode::Unauthenticated,
                    self.to_string(),
                ))
            }
            ApiError::LoginFailed => {
                debug!("Login request failed");

                HttpResponse::BadRequest().json(ApiErrorResponse::new(
                    ApiStatusCode::LoginFailed,
                    self.to_string(),
                ))
            }
            ApiError::DatabaseError(err) => {
                error!("Database error: {err}");

                HttpResponse::InternalServerError().json(ApiErrorResponse::new(
                    ApiStatusCode::DatabaseError,
                    self.to_string(),
                ))
            }
            ApiError::UsernameAlreadyOccupied => {
                debug!("Username is already occupied");

                HttpResponse::BadRequest().json(ApiErrorResponse::new(
                    ApiStatusCode::UsernameAlreadyOccupied,
                    self.to_string(),
                ))
            }
            ApiError::InvalidHash(err) => {
                error!("Got invalid password hash from db: {err}");

                HttpResponse::InternalServerError().json(ApiErrorResponse::new(
                    ApiStatusCode::InternalServerError,
                    self.to_string(),
                ))
            }
            ApiError::InternalServerError => HttpResponse::InternalServerError().json(
                ApiErrorResponse::new(ApiStatusCode::InternalServerError, self.to_string()),
            ),
            ApiError::NotFound => {
                info!("Not found");

                HttpResponse::BadRequest().json(ApiErrorResponse::new(
                    ApiStatusCode::NotFound,
                    self.to_string(),
                ))
            }
            ApiError::InvalidContentType => HttpResponse::BadRequest().json(ApiErrorResponse::new(
                ApiStatusCode::InvalidContentType,
                self.to_string(),
            )),
            ApiError::InvalidJson(err) => {
                debug!("Received invalid json: {err}");

                HttpResponse::BadRequest().json(ApiErrorResponse::new(
                    ApiStatusCode::InvalidJson,
                    self.to_string(),
                ))
            }
            ApiError::PayloadOverflow(err) => {
                debug!("Payload overflow: {err}");

                HttpResponse::BadRequest().json(ApiErrorResponse::new(
                    ApiStatusCode::PayloadOverflow,
                    self.to_string(),
                ))
            }
            ApiError::SessionCorrupt => {
                warn!("Corrupt session");

                HttpResponse::BadRequest().json(ApiErrorResponse::new(
                    ApiStatusCode::SessionError,
                    self.to_string(),
                ))
            }
            ApiError::InvalidPassword => {
                debug!("Invalid password specified");
                HttpResponse::BadRequest().json(ApiErrorResponse::new(
                    ApiStatusCode::InvalidPassword,
                    self.to_string(),
                ))
            }
            ApiError::EmptyJson => {
                debug!("Empty json found in request");
                HttpResponse::BadRequest().json(ApiErrorResponse::new(
                    ApiStatusCode::EmptyJson,
                    self.to_string(),
                ))
            }
            ApiError::InvalidUsername => {
                debug!("Invalid username specified");
                HttpResponse::BadRequest().json(ApiErrorResponse::new(
                    ApiStatusCode::InvalidUsername,
                    self.to_string(),
                ))
            }
            ApiError::InvalidDisplayName => {
                debug!("Invalid display name specified");
                HttpResponse::BadRequest().json(ApiErrorResponse::new(
                    ApiStatusCode::InvalidDisplayName,
                    self.to_string(),
                ))
            }
            ApiError::FriendshipAlreadyRequested => {
                debug!("Friendship was already requested");
                HttpResponse::BadRequest().json(ApiErrorResponse::new(
                    ApiStatusCode::FriendshipAlreadyRequested,
                    self.to_string(),
                ))
            }
            ApiError::AlreadyFriends => {
                debug!("Already friends");
                HttpResponse::BadRequest().json(ApiErrorResponse::new(
                    ApiStatusCode::AlreadyFriends,
                    self.to_string(),
                ))
            }
            ApiError::InvalidId => {
                debug!("Invalid id specified");
                HttpResponse::BadRequest().json(ApiErrorResponse::new(
                    ApiStatusCode::InvalidId,
                    self.to_string(),
                ))
            }
            ApiError::MissingPrivileges => {
                debug!("Missing privileges");
                HttpResponse::BadRequest().json(ApiErrorResponse::new(
                    ApiStatusCode::MissingPrivileges,
                    self.to_string(),
                ))
            }
            ApiError::InvalidMaxPlayersCount => {
                debug!("Invalid max_players count found");
                HttpResponse::BadRequest().json(ApiErrorResponse::new(
                    ApiStatusCode::InvalidMaxPlayersCount,
                    self.to_string(),
                ))
            }
            ApiError::AlreadyInALobby => {
                debug!("Already in a lobby");
                HttpResponse::BadRequest().json(ApiErrorResponse::new(
                    ApiStatusCode::AlreadyInALobby,
                    self.to_string(),
                ))
            }
            ApiError::InvalidUuid => HttpResponse::BadRequest().json(ApiErrorResponse::new(
                ApiStatusCode::InvalidUuid,
                self.to_string(),
            )),
        }
    }
}

impl From<rorm::Error> for ApiError {
    fn from(value: rorm::Error) -> Self {
        Self::DatabaseError(value)
    }
}

impl From<argon2::password_hash::Error> for ApiError {
    fn from(value: argon2::password_hash::Error) -> Self {
        Self::InvalidHash(value)
    }
}

impl From<actix_session::SessionInsertError> for ApiError {
    fn from(value: actix_session::SessionInsertError) -> Self {
        Self::SessionInsert(value)
    }
}

impl From<actix_session::SessionGetError> for ApiError {
    fn from(value: actix_session::SessionGetError) -> Self {
        Self::SessionGet(value)
    }
}
