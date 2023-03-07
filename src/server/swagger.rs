//! This module holds the definition of the swagger declaration

use utoipa::openapi::security::{ApiKey, ApiKeyValue, HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{Modify, OpenApi};

use crate::server::handler;

struct CookieSecurity;

impl Modify for CookieSecurity {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "session_cookie",
                SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::new("id"))),
            )
        }
    }
}

/// Helper struct for the openapi definitions.
#[derive(OpenApi)]
#[openapi(
    paths(
        handler::register_account,
        handler::get_me,
        handler::delete_me,
        handler::update_me,
        handler::set_password,
        handler::login,
        handler::logout,
        handler::websocket,
        handler::version,
        handler::create_friend_request,
        handler::accept_friend_request,
        handler::get_friends,
        handler::delete_friend,
        handler::get_lobbies,
    ),
    components(schemas(
        handler::AccountRegistrationRequest,
        handler::ApiErrorResponse,
        handler::ApiStatusCode,
        handler::LoginRequest,
        handler::AccountResponse,
        handler::SetPasswordRequest,
        handler::UpdateAccountRequest,
        handler::VersionResponse,
        handler::CreateFriendRequest,
        handler::GetFriendResponse,
        handler::FriendResponse,
        handler::LobbyResponse,
        handler::GetLobbiesResponse,
    )),
    modifiers(&CookieSecurity)
)]
pub struct ApiDoc;

struct TokenSecurity;

impl Modify for TokenSecurity {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "admin_token",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .description(Some(
                            "The token is set in the configuration file in the server.",
                        ))
                        .build(),
                ),
            )
        }
    }
}

/// Helper struct for the admin openapi definitions.
#[derive(OpenApi)]
#[openapi(
    paths(
        handler::health,
    ),
    components(schemas(
        handler::ApiErrorResponse,
        handler::ApiStatusCode,
        handler::HealthResponse,
    )),
    modifiers(&TokenSecurity)
)]
pub struct AdminApiDoc;
