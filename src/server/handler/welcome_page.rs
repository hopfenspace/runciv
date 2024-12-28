//! Static file serving

use actix_web::{get, HttpResponse};

#[get("/")]
pub async fn welcome_page() -> HttpResponse {
    let b = include_str!("../../../static/index.html");

    HttpResponse::Ok().body(b)
}
