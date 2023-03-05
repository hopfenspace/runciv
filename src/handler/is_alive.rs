use actix_web::get;
use actix_web::web::Json;
use serde::Serialize;

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct IsAliveResponse {
    auth_version: u8,
}

#[get("/isalive")]
pub(crate) async fn is_alive() -> Json<IsAliveResponse> {
    Json(IsAliveResponse { auth_version: 0 })
}
