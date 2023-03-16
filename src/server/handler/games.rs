use actix_toolbox::tb_middleware::Session;
use actix_web::get;
use actix_web::web::{Data, Json, Path};
use chrono::{DateTime, Utc};
use itertools::Itertools;
use rorm::fields::ForeignModelByField;
use rorm::{and, insert, query, Database, Model};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::config::Config;
use crate::models::{
    Account, ChatRoom, ChatRoomInsert, Game, GameAccount, GameAccountInsert, GameInsert,
};
use crate::server::handler::{AccountResponse, ApiError, ApiErrorResponse, ApiResult};
use crate::server::RuntimeSettings;

/// A single game state identified by its ID and state identifier
///
/// If the state (`game_data_id`) of a known game differs from the last known
/// identifier, the server has a newer state of the game. The `last_activity`
/// field is a convenience attribute and shouldn't be used for update checks.
#[derive(Serialize, ToSchema)]
pub struct GameStateResponse {
    game_data: String,
    #[schema(example = 1337)]
    game_data_id: u64,
    #[schema(example = "Herbert's game")]
    name: String,
    max_player: i16,
    last_activity: DateTime<Utc>,
    last_player: AccountResponse,
    #[schema(example = 1337)]
    chat_room_id: u64,
}

/// A shortened game state identified by its ID and state identifier
///
/// If the state (`game_data_id`) of a known game differs from the last known
/// identifier, the server has a newer state of the game. The `last_activity`
/// field is a convenience attribute and shouldn't be used for update checks.
#[derive(Serialize, ToSchema)]
pub struct GameOverviewResponse {
    #[schema(example = 1337)]
    game_id: u64,
    #[schema(example = 1337)]
    game_data_id: u64,
    #[schema(example = "Herbert's game")]
    name: String,
    max_player: i16,
    last_activity: DateTime<Utc>,
    last_player: AccountResponse,
    #[schema(example = 1337)]
    chat_room_id: u64,
}

/// An overview of games a player participates in
#[derive(Serialize, ToSchema)]
pub struct GetGameOverviewResponse {
    games: Vec<GameOverviewResponse>,
}

/// Retrieves an overview of all open games of a player
///
/// The response does not contain any full game state.
#[utoipa::path(
    tag = "Games",
    context_path = "/api/v2",
    responses(
        (status = 200, description = "Returns all currently open games of a player", body = GetGameOverviewResponse),
        (status = 400, description = "Client error", body = ApiErrorResponse),
        (status = 500, description = "Server error", body = ApiErrorResponse),
    ),
    security(("session_cookie" = []))
)]
#[get("/games")]
pub async fn get_open_games(
    settings: Data<RuntimeSettings>,
    db: Data<Database>,
    session: Session,
) -> ApiResult<Json<GetGameOverviewResponse>> {
    let uuid: Uuid = session.get("uuid")?.ok_or(ApiError::SessionCorrupt)?;

    let open_games = query!(
        db.as_ref(),
        (
            Game::F.id,
            Game::F.data_id,
            Game::F.name,
            Game::F.max_players,
            Game::F.updated_at,
            Game::F.updated_by.uuid,
            Game::F.updated_by.username,
            Game::F.updated_by.display_name,
            Game::F.chat_room,
        )
    )
    .condition(Game::F.current_players.player.equals(uuid.as_ref()))
    .all()
    .await?
    .into_iter()
    .map(
        |(
            id,
            data_id,
            name,
            max_players,
            updated_at,
            updated_by_uuid,
            updated_by_username,
            updated_by_display_name,
            chat_room,
        )| {
            GameOverviewResponse {
                game_id: id as u64,
                game_data_id: data_id as u64,
                name,
                max_players,
                last_activity: DateTime::from_utc(updated_at, Utc),
                last_player: AccountResponse {
                    uuid: updated_by_uuid,
                    username: updated_by_username.to_string(),
                    display_name: updated_by_display_name.to_string(),
                },
                chat_room_id: *chat_room.key() as u64,
            }
        },
    )
    .collect();

    Ok(Json(GetGameOverviewResponse { games: open_games }))
}

/// The ID of a game
#[derive(Deserialize, IntoParams)]
pub struct GameId {
    #[param(example = 1337)]
    id: u64,
}

/// Retrieves a single game which is currently open (actively played)
///
/// If the game has been completed or aborted, it
/// will respond with a `NotFound` in `ApiErrorResponse`.
#[utoipa::path(
    tag = "Games",
    context_path = "/api/v2",
    responses(
        (status = 200, description = "Returns a single game state", body = GameStateResponse),
        (status = 400, description = "Client error", body = ApiErrorResponse),
        (status = 500, description = "Server error", body = ApiErrorResponse),
    ),
    params(GameId),
    security(("session_cookie" = []))
)]
#[get("/games/{id}")]
pub async fn get_game(
    path: Path<GameId>,
    db: Data<Database>,
    session: Session,
) -> ApiResult<Json<GameStateResponse>> {
    // TODO: Implement this endpoint
    Ok(Json(GameStateResponse {
        game_data: "".to_string(),
        game_data_id: 0,
        name: "".to_string(),
        max_players: 0,
        last_activity: Default::default(),
        last_player: AccountResponse {
            uuid: Default::default(),
            username: "".to_string(),
            display_name: "".to_string(),
        },
        chat_room_id: 0,
    }))
}
