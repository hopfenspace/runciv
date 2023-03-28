use actix_toolbox::tb_middleware::Session;
use actix_web::web::{Data, Json};
use actix_web::{get, post};
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHasher};
use chrono::{DateTime, Utc};
use rand::thread_rng;
use rorm::fields::{BackRef, ForeignModelByField};
use rorm::{insert, query, Database, Model};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::models::{
    Account, ChatRoomInsert, ChatRoomMemberInsert, Lobby, LobbyAccount, LobbyInsert,
};
use crate::server::handler::{AccountResponse, ApiError, ApiResult};

/// A single lobby
#[derive(Serialize, ToSchema)]
pub struct LobbyResponse {
    uuid: Uuid,
    #[schema(example = "Herbert's lobby")]
    name: String,
    #[schema(example = 4)]
    max_players: u8,
    #[schema(example = 3)]
    current_players: u8,
    created_at: DateTime<Utc>,
    password: bool,
    owner: AccountResponse,
    chat_room_uuid: Uuid,
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
    let mut tx = db.start_transaction().await?;

    let lobbies = query!(
        &mut tx,
        (
            Lobby::F.uuid,
            Lobby::F.owner.uuid,
            Lobby::F.owner.username,
            Lobby::F.owner.display_name,
            Lobby::F.name,
            Lobby::F.created_at,
            Lobby::F.max_player,
            Lobby::F.password_hash,
            Lobby::F.chat_room.uuid,
        )
    )
    .all()
    .await?;

    let mut lobbies: Vec<Lobby> = lobbies
        .into_iter()
        .map(
            |(
                uuid,
                o_uuid,
                o_username,
                o_display_name,
                name,
                created_at,
                max_player,
                password_hash,
                chat_room_uuid,
            )| Lobby {
                uuid,
                name,
                current_player: BackRef { cached: None },
                owner: ForeignModelByField::Instance(Box::new(Account {
                    uuid: o_uuid,
                    username: o_username,
                    display_name: o_display_name,
                    last_login: None,
                    password_hash: String::new(),
                    chat_rooms: BackRef { cached: None },
                })),
                created_at,
                max_player,
                password_hash,
                chat_room: ForeignModelByField::Key(chat_room_uuid),
            },
        )
        .collect();

    Lobby::F
        .current_player
        .populate_bulk(&mut tx, &mut lobbies)
        .await?;

    Ok(Json(GetLobbiesResponse {
        lobbies: lobbies
            .into_iter()
            .map(|l| {
                let Some(owner) = l.owner.instance() else {
                    unreachable!("Owner should be queried!")
                };
                // Ok as current_player is populated before
                #[allow(clippy::unwrap_used)]
                LobbyResponse {
                    uuid: l.uuid,
                    name: l.name,
                    owner: AccountResponse {
                        uuid: owner.uuid,
                        username: owner.username.clone(),
                        display_name: owner.display_name.clone(),
                    },
                    current_players: l.current_player.cached.unwrap().len() as u8 + 1,
                    max_players: l.max_player as u8,
                    password: l.password_hash.is_some(),
                    created_at: DateTime::from_utc(l.created_at, Utc),
                    chat_room_uuid: *l.chat_room.key(),
                }
            })
            .collect(),
    }))
}

/// The parameters to create a lobby
///
/// `max_players` must be greater or equals 2
#[derive(Deserialize, ToSchema)]
pub struct CreateLobbyRequest {
    #[schema(example = "Herbert's lobby")]
    name: String,
    #[schema(example = "super-secure-password")]
    password: Option<String>,
    #[schema(example = 4)]
    max_players: u8,
}

/// The response of a create lobby request.
///
/// It contains the uuid of the created lobby and the uuid of the created chatroom for the lobby
#[derive(Serialize, ToSchema)]
pub struct CreateLobbyResponse {
    lobby_uuid: Uuid,
    lobby_chat_room_uuid: Uuid,
}

/// Create a new lobby
///
/// If you are already in another lobby, an error is returned.
/// `max_players` must be between 2 and 34 (inclusive).
/// If `password` is an empty string, an error is returned.
///
/// You are placed in
#[utoipa::path(
    tag = "Lobbies",
    context_path = "/api/v2",
    responses(
        (status = 200, description = "Lobby got created", body = CreateLobbyResponse),
        (status = 400, description = "Client error", body = ApiErrorResponse),
        (status = 500, description = "Server error", body = ApiErrorResponse),
    ),
    request_body = CreateLobbyRequest,
    security(("session_cookie" = []))
)]
#[post("/lobbies")]
pub async fn create_lobby(
    req: Json<CreateLobbyRequest>,
    db: Data<Database>,
    session: Session,
) -> ApiResult<Json<CreateLobbyResponse>> {
    let uuid: Uuid = session.get("uuid")?.ok_or(ApiError::SessionCorrupt)?;

    let mut tx = db.start_transaction().await?;

    // Check if the request is valid
    if req.max_players < 2 || req.max_players > 34 {
        return Err(ApiError::InvalidMaxPlayersCount);
    }

    // Check if the executing account is already in a lobby
    if query!(&mut tx, (LobbyAccount::F.uuid,))
        .condition(LobbyAccount::F.player.equals(uuid.as_ref()))
        .optional()
        .await?
        .is_some()
    {
        return Err(ApiError::AlreadyInALobby);
    }

    if query!(&mut tx, (Lobby::F.uuid,))
        .condition(Lobby::F.owner.equals(uuid.as_ref()))
        .optional()
        .await?
        .is_some()
    {
        return Err(ApiError::AlreadyInALobby);
    }

    // Hash the password
    // Yes its only a game password, but why not ¯\_(ツ)_/¯
    let pw_hash = if let Some(pw) = &req.password {
        if pw.is_empty() {
            return Err(ApiError::InvalidPassword);
        }

        let salt = SaltString::generate(&mut thread_rng());
        Some(
            Argon2::default()
                .hash_password(pw.as_bytes(), &salt)?
                .to_string(),
        )
    } else {
        None
    };

    // Create chatroom for lobby
    let chat_room_uuid = insert!(&mut tx, ChatRoomInsert)
        .return_primary_key()
        .single(&ChatRoomInsert {
            uuid: Uuid::new_v4(),
        })
        .await?;

    // Place current user in chat
    insert!(&mut tx, ChatRoomMemberInsert)
        .single(&ChatRoomMemberInsert {
            uuid: Uuid::new_v4(),
            chat_room: ForeignModelByField::Key(chat_room_uuid),
            member: ForeignModelByField::Key(uuid),
        })
        .await?;

    // Create lobby
    let uuid = insert!(&mut tx, LobbyInsert)
        .return_primary_key()
        .single(&LobbyInsert {
            uuid: Uuid::new_v4(),
            name: req.name.clone(),
            password_hash: pw_hash,
            max_player: req.max_players as i16,
            owner: ForeignModelByField::Key(uuid),
            chat_room: ForeignModelByField::Key(chat_room_uuid),
        })
        .await?;

    tx.commit().await?;

    Ok(Json(CreateLobbyResponse {
        lobby_uuid: uuid,
        lobby_chat_room_uuid: chat_room_uuid,
    }))
}
