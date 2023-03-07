use actix_web::get;
use actix_web::web::{Data, Json};
use chrono::{DateTime, Utc};
use rorm::{query, Database, Model};
use serde::Serialize;
use utoipa::ToSchema;

use crate::models::Lobby;
use crate::server::handler::ApiResult;

/// A single lobby
#[derive(Serialize, ToSchema)]
pub struct LobbyResponse {
    #[schema(example = "Herbert's lobby")]
    name: String,
    #[schema(example = 4)]
    max_players: u8,
    #[schema(example = 3)]
    current_players: u8,
    created_at: DateTime<Utc>,
    password: bool,
}

/// The lobbies that are open
#[derive(Serialize, ToSchema)]
pub struct GetLobbiesResponse {
    lobbies: Vec<LobbyResponse>,
}

/// Retrieves all open lobbies.
///
/// If `password` is `true`, the lobby is secured by a user-set password
#[utoipa::path(
    tag = "Lobbies",
    context_path = "/api/v2",
    responses(
        (status = 200, description = "Returns all currently open lobbies", body = GetLobbiesResponse),
        (status = 400, description = "Client error", body = ApiErrorResponse),
        (status = 500, description = "Server error", body = ApiErrorResponse),
    ),
    security(("session_cookie" = []))
)]
#[get("/lobbies")]
pub async fn get_lobbies(db: Data<Database>) -> ApiResult<Json<GetLobbiesResponse>> {
    let mut lobbies = query!(&db, Lobby).all().await?;

    for lobby in &mut lobbies {
        Lobby::F.current_player.populate(&db, lobby).await?;
    }

    Ok(Json(GetLobbiesResponse {
        lobbies: lobbies
            .into_iter()
            .map(|l| LobbyResponse {
                name: l.name,
                current_players: l.current_player.cached.unwrap().len() as u8,
                max_players: l.max_player as u8,
                password: l.password_hash.is_some(),
                created_at: DateTime::from_utc(l.created_at, Utc),
            })
            .collect(),
    }))
}
