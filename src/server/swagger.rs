//! This module holds the definition of the swagger declaration

use utoipa::OpenApi;

use crate::server::handler;

/// Helper struct for the openapi definitions.
#[derive(OpenApi)]
#[openapi(
    paths(handler::register_account),
    components(schemas(
        handler::AccountRegistrationRequest,
        handler::ApiErrorResponse,
        handler::ApiStatusCode
    ))
)]
pub struct ApiDoc;
