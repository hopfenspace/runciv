//! This module holds the definition of the swagger declaration

use utoipa::openapi::security::{ApiKey, ApiKeyValue, SecurityScheme};
use utoipa::{Modify, OpenApi};

use crate::server::handler;

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "api_key",
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
    )),
    modifiers(&SecurityAddon)
)]
pub struct ApiDoc;
