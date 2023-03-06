use actix_web::get;
use actix_web::web::{Data, Json};
use rorm::{query, Database, Model};
use serde::Serialize;
use utoipa::ToSchema;

use crate::models::Account;
use crate::server::handler::ApiResult;

/// The health data of this server
#[derive(Serialize, ToSchema)]
pub struct HealthResponse {
    #[schema(example = 1337)]
    accounts: u64,
}

/// Request health data from this server.
#[utoipa::path(
    tag = "Server status",
    context_path = "/api/v2/admin",
    responses(
        (status = 200, description = "Health data of this server", body = HealthResponse),
        (status = 400, description = "Client error", body = ApiErrorResponse),
        (status = 500, description = "Server error", body = ApiErrorResponse),
    ),
    security(("admin_token" = []))
)]
#[get("/health")]
pub async fn health(db: Data<Database>) -> ApiResult<Json<HealthResponse>> {
    let accounts = query!(&db, (Account::F.uuid.count(),))
        .one()
        .await?
        .0
        .unwrap() as u64;

    Ok(Json(HealthResponse { accounts }))
}
