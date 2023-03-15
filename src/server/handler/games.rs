use actix_toolbox::tb_middleware::Session;
use actix_web::web::{Data, Json};
use actix_web::{get, post};
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHasher};
use chrono::{DateTime, Utc};
use rand::thread_rng;
use rorm::internal::field::foreign_model::ForeignModelByField;
use rorm::{insert, query, BackRef, Database, Model};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::models::{
    Account, ChatRoomInsert, ChatRoomMemberInsert, Lobby, LobbyAccount, LobbyInsert,
};
use crate::server::handler::{AccountResponse, ApiError, ApiResult};

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
