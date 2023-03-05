use actix_web::get;
use actix_web::web::Json;
use serde::Serialize;
use utoipa::ToSchema;

/// The version data for clients
#[derive(Serialize, ToSchema)]
pub struct VersionResponse {
    #[schema(example = 2)]
    version: u8,
}

/// This endpoint is for clients to detect which version this server currently supports
#[utoipa::path(
    tag = "Version",
    responses(
        (status = 200, description = "Login successful", body = VersionResponse)
    ),
)]
#[get("/api/version")]
pub async fn version() -> Json<VersionResponse> {
    Json(VersionResponse { version: 2 })
}
