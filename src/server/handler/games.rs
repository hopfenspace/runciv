use std::path::Path as StdPath;

use actix_toolbox::tb_middleware::Session;
use actix_web::web::{Data, Json, Path};
use actix_web::{get, put};
use chrono::{DateTime, Utc};
use log::{debug, error, warn};
use rorm::{and, query, update, Database, Model};
use serde::{Deserialize, Serialize};
use tokio::fs::{read_to_string, remove_file, write};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::chan::{WsManagerChan, WsManagerMessage, WsMessage};
use crate::models::Game;
use crate::server::handler::{AccountResponse, ApiError, ApiResult, PathUuid};
use crate::server::RuntimeSettings;

/// A single game state identified by its Uuid and state identifier
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
    #[schema(example = 7)]
    max_players: i16,
    last_activity: DateTime<Utc>,
    last_player: AccountResponse,
    chat_room_uuid: Uuid,
}

/// A shortened game state identified by its ID and state identifier
///
/// If the state (`game_data_id`) of a known game differs from the last known
/// identifier, the server has a newer state of the game. The `last_activity`
/// field is a convenience attribute and shouldn't be used for update checks.
#[derive(Serialize, ToSchema)]
pub struct GameOverviewResponse {
    game_uuid: Uuid,
    #[schema(example = 1337)]
    game_data_id: u64,
    #[schema(example = "Herbert's game")]
    name: String,
    #[schema(example = 7)]
    max_players: i16,
    last_activity: DateTime<Utc>,
    last_player: AccountResponse,
    chat_room_uuid: Uuid,
}

/// An overview of games a player participates in
#[derive(Serialize, ToSchema)]
pub struct GetGameOverviewResponse {
    games: Vec<GameOverviewResponse>,
}

/// Retrieves an overview of all open games of a player
///
/// The response does not contain any full game state, but rather
/// a shortened game state identified by its ID and state identifier.
/// If the state (`game_data_id`) of a known game differs from the last known
/// identifier, the server has a newer state of the game. The `last_activity`
/// field is a convenience attribute and shouldn't be used for update checks.
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
    db: Data<Database>,
    session: Session,
) -> ApiResult<Json<GetGameOverviewResponse>> {
    let uuid: Uuid = session.get("uuid")?.ok_or(ApiError::SessionCorrupt)?;

    let open_games = query!(
        db.as_ref(),
        (
            Game::F.uuid,
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
            game_uuid,
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
                game_uuid,
                game_data_id: data_id as u64,
                name,
                max_players,
                last_activity: DateTime::from_utc(updated_at, Utc),
                last_player: AccountResponse {
                    uuid: updated_by_uuid,
                    username: updated_by_username,
                    display_name: updated_by_display_name,
                },
                chat_room_uuid: *chat_room.key(),
            }
        },
    )
    .collect();

    Ok(Json(GetGameOverviewResponse { games: open_games }))
}

/// Retrieves a single game which is currently open (actively played)
///
/// If the game has been completed or aborted, it
/// will respond with a `GameNotFound` in `ApiErrorResponse`.
#[utoipa::path(
    tag = "Games",
    context_path = "/api/v2",
    responses(
        (status = 200, description = "Returns a single game state", body = GameStateResponse),
        (status = 400, description = "Client error", body = ApiErrorResponse),
        (status = 500, description = "Server error", body = ApiErrorResponse),
    ),
    params(PathUuid),
    security(("session_cookie" = []))
)]
#[get("/games/{uuid}")]
pub async fn get_game(
    path: Path<PathUuid>,
    settings: Data<RuntimeSettings>,
    db: Data<Database>,
    session: Session,
) -> ApiResult<Json<GameStateResponse>> {
    let uuid: Uuid = session.get("uuid")?.ok_or(ApiError::SessionCorrupt)?;
    let game_uuid = path.uuid;

    let (
        data_id,
        name,
        max_players,
        updated_at,
        updated_by_uuid,
        updated_by_username,
        updated_by_display_name,
        chat_room,
    ) = query!(
        db.as_ref(),
        (
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
    .condition(and!(
        Game::F.uuid.equals(game_uuid.as_ref()),
        Game::F.current_players.player.uuid.equals(uuid.as_ref())
    ))
    .optional()
    .await?
    .ok_or({
        debug!("Game not found since no database entry exists for the given search parameters");
        ApiError::GameNotFound
    })?;

    let filename = format!("game_{game_uuid}_{data_id}.txt");
    let path = StdPath::new(&settings.game_data_path).join(&filename);
    let content = read_to_string(&path).await.map_err(|e| {
        error!("Game data expected in '{filename}' couldn't be read: {e}");
        ApiError::InternalServerError
    })?;
    Ok(Json(GameStateResponse {
        game_data: content,
        game_data_id: data_id as u64,
        name,
        max_players,
        last_activity: DateTime::from_utc(updated_at, Utc),
        last_player: AccountResponse {
            uuid: updated_by_uuid,
            username: updated_by_username.to_string(),
            display_name: updated_by_display_name.to_string(),
        },
        chat_room_uuid: *chat_room.key(),
    }))
}

/// The response a user receives after uploading a new game state successfully
#[derive(Serialize, ToSchema)]
pub struct GameUploadResponse {
    #[schema(example = 1337)]
    game_data_id: u64,
}

/// The request a user sends to the server to upload a new game state
#[derive(Deserialize, ToSchema)]
pub struct GameUploadRequest {
    game_data: String,
}

/// Upload a new game state for an existing game
///
/// If the game can't be updated (maybe it has been already completed or
/// aborted), it will respond with a `GameNotFound` in `ApiErrorResponse`.
#[utoipa::path(
    tag = "Games",
    context_path = "/api/v2",
    responses(
        (status = 200, description = "Returns the new data identifier of the uploaded game state", body = GameUploadResponse),
        (status = 400, description = "Client error", body = ApiErrorResponse),
        (status = 500, description = "Server error", body = ApiErrorResponse),
    ),
    params(PathUuid),
    request_body = GameUploadRequest,
    security(("session_cookie" = []))
)]
#[put("/games/{uuid}")]
pub async fn push_game_update(
    path: Path<PathUuid>,
    req: Json<GameUploadRequest>,
    settings: Data<RuntimeSettings>,
    db: Data<Database>,
    session: Session,
    ws_manager_chan: Data<WsManagerChan>,
) -> ApiResult<Json<GameUploadResponse>> {
    let game_uuid = path.uuid;
    let uuid: Uuid = session.get("uuid")?.ok_or(ApiError::SessionCorrupt)?;

    let mut tx = db.start_transaction().await?;

    // Lookup the game and verify that the player is actually participating in it
    let mut game = query!(&mut tx, Game)
        .condition(and!(
            Game::F.uuid.equals(game_uuid.as_ref()),
            Game::F.current_players.player.uuid.equals(uuid.as_ref())
        ))
        .optional()
        .await?
        .ok_or(ApiError::GameNotFound)?;

    // Retrieve uuids of all players from the game
    Game::F.current_players.populate(&mut tx, &mut game).await?;
    let players: Vec<Uuid> = if let Some(current_players) = game.current_players.cached {
        current_players
            .into_iter()
            .map(|x| *x.player.key())
            .collect()
    } else {
        error!("Cache of populated field current_players was empty");
        return Err(ApiError::InternalServerError);
    };

    // Increment the data identifier used to determine whether a game state has changed
    let new_data_id = game.data_id + 1;

    // Save a new file with the updated game state to disk
    let new_filename = format!("game_{game_uuid}_{new_data_id}.txt");
    let new_path = StdPath::new(&settings.game_data_path).join(&new_filename);
    if let Err(e) = write(&new_path, &req.game_data).await {
        error!("Game data could not be saved to '{new_filename}': {e}");
        return Err(ApiError::InternalServerError);
    }

    // Update the game state identifier and last player in the database,
    // which also updates the last access time automatically
    update!(&mut tx, Game)
        .set(Game::F.data_id, new_data_id)
        .set(Game::F.updated_by, uuid.as_ref())
        .condition(Game::F.uuid.equals(game_uuid.as_ref()))
        .await?;

    tx.commit().await?;

    // Remove the old file from the filesystem
    let old_filename = format!("game_{game_uuid}_{old}.txt", old = game.data_id);
    let old_path = StdPath::new(&settings.game_data_path).join(&old_filename);
    if let Err(e) = remove_file(&old_path).await {
        warn!("Outdated data in '{old_filename}' could not be removed and may leak: {e}");
    }

    // Notify all remaining players about the new game data
    let msg = WsMessage::UpdateGameData {
        game_uuid: game.uuid,
        game_data_id: new_data_id as u64,
        game_data: req.game_data.clone(),
    };
    for player in players.into_iter().filter(|x| *x == uuid) {
        if let Err(err) = ws_manager_chan
            .send(WsManagerMessage::SendMessage(player, msg.clone()))
            .await
        {
            error!("Could not send to ws manager chan: {err}");
        }
    }

    Ok(Json(GameUploadResponse {
        game_data_id: new_data_id as u64,
    }))
}
