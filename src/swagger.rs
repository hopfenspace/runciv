//! This module holds the definition of the swagger declaration

use utoipa::OpenApi;

/// Helper struct for the openapi definitions.
#[derive(OpenApi)]
#[openapi(paths(), components(schemas()))]
pub struct ApiDoc;
