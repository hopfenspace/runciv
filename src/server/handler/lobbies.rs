use actix_toolbox::tb_middleware::Session;
use actix_web::web::{Data, Json, Path};
use actix_web::{get, post};
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHasher};
use chrono::{DateTime, Utc};
use log::error;
use rand::thread_rng;
use rorm::fields::{BackRef, ForeignModelByField};
use rorm::{delete, insert, query, update, Database, Model};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::chan::{WsManagerChan, WsManagerMessage, WsMessage};
use crate::models::{
    Account, ChatRoomInsert, ChatRoomMember, ChatRoomMemberInsert, ChatRoomMessage,
    GameAccountInsert, GameInsert, Lobby, LobbyAccount, LobbyInsert,
};
use crate::server::handler::{AccountResponse, ApiError, ApiResult, PathUuid};

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
/// You are placed in the lobby and in the corresponding chatroom
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

/// The response when starting a game
#[derive(Serialize, ToSchema)]
pub struct StartGameResponse {
    game_uuid: Uuid,
    game_chat_uuid: Uuid,
}

/// Start a game from an existing lobby.
///
/// The executing user must be the owner of the lobby.
///
/// The lobby is deleted in the process, a new chatroom is created and all messages from the
/// lobby chatroom are attached to the game chatroom.
///
/// This will invoke a [WsMessage::GameStarted] message that is sent via websocket to all
/// members of the lobby to inform them which lobby was started. It also contains the the new and
/// old chatroom uuids to make mapping for the clients easier.
///
/// After the game started, the lobby owner must use the `PUT /api/v2/games/{uuid}` endpoint to
/// upload the initial game state.
///
/// **Note**:
/// This behaviour is subject to change.
/// The server should be set the order in which players are allowed to make their turns.
/// This allows the server to detect malicious players trying to update the game state before
/// its their turn.
#[utoipa::path(
    tag = "Lobbies",
    context_path = "/api/v2",
    responses(
        (status = 200, description = "Lobby got created", body = StartGameResponse),
        (status = 400, description = "Client error", body = ApiErrorResponse),
        (status = 500, description = "Server error", body = ApiErrorResponse),
    ),
    params(PathUuid),
    security(("session_cookie" = []))
)]
#[post("/lobbies/{uuid}/start")]
pub async fn start_game(
    path: Path<PathUuid>,
    db: Data<Database>,
    session: Session,
    ws_manager_chan: Data<WsManagerChan>,
) -> ApiResult<Json<StartGameResponse>> {
    let uuid: Uuid = session.get("uuid")?.ok_or(ApiError::SessionCorrupt)?;

    let mut tx = db.start_transaction().await?;

    let mut lobby = query!(&mut tx, Lobby)
        .condition(Lobby::F.uuid.equals(path.uuid.as_ref()))
        .optional()
        .await?
        .ok_or(ApiError::InvalidUuid)?;

    Lobby::F
        .current_player
        .populate(&mut tx, &mut lobby)
        .await?;

    // Check if the executing user owns the lobby
    if *lobby.owner.key() != uuid {
        return Err(ApiError::MissingPrivileges);
    }

    // Create chatroom for the game
    let game_chat_uuid = insert!(&mut tx, ChatRoomInsert)
        .return_primary_key()
        .single(&ChatRoomInsert {
            uuid: Uuid::new_v4(),
        })
        .await?;

    // Move messages from lobby chat to game chat
    update!(&mut tx, ChatRoomMessage)
        .condition(
            ChatRoomMessage::F
                .chat_room
                .equals(lobby.chat_room.key().as_ref()),
        )
        .set(ChatRoomMessage::F.chat_room, game_chat_uuid.as_ref())
        .exec()
        .await?;

    // Move chatroom member to new chatroom
    update!(&mut tx, ChatRoomMember)
        .condition(
            ChatRoomMember::F
                .chat_room
                .equals(lobby.chat_room.key().as_ref()),
        )
        .set(ChatRoomMember::F.chat_room, game_chat_uuid.as_ref())
        .exec()
        .await?;

    // Create new game and attach lobby chat
    let game_uuid = insert!(&mut tx, GameInsert)
        .return_primary_key()
        .single(&GameInsert {
            uuid: Uuid::new_v4(),
            chat_room: ForeignModelByField::Key(game_chat_uuid),
            max_players: lobby.max_player,
            name: lobby.name,
            updated_by: ForeignModelByField::Key(uuid),
        })
        .await?;

    // Retrieve players from lobby
    let player: Vec<Uuid> = if let Some(lobby_player) = lobby.current_player.cached {
        lobby_player
            .into_iter()
            .map(|x: LobbyAccount| *x.player.key())
            .collect()
    } else {
        error!("Cache of populated field current_player was empty");
        return Err(ApiError::InternalServerError);
    };

    // Attach all players from lobby to game
    insert!(&mut tx, GameAccountInsert)
        .return_nothing()
        .bulk(
            &player
                .iter()
                .map(|x| GameAccountInsert {
                    uuid: Uuid::new_v4(),
                    game: ForeignModelByField::Key(game_uuid),
                    player: ForeignModelByField::Key(*x),
                })
                .collect::<Vec<_>>(),
        )
        .await?;

    // Attach owner
    insert!(&mut tx, GameAccountInsert)
        .return_nothing()
        .single(&GameAccountInsert {
            uuid: Uuid::new_v4(),
            game: ForeignModelByField::Key(game_uuid),
            player: ForeignModelByField::Key(*lobby.owner.key()),
        })
        .await?;

    // Delete lobby
    delete!(&mut tx, Lobby)
        .condition(Lobby::F.uuid.equals(uuid.as_ref()))
        .await?;

    tx.commit().await?;

    // Send notifications to all remaining players
    let msg = WsMessage::GameStarted {
        game_uuid,
        game_chat_uuid,
        lobby_uuid: lobby.uuid,
        lobby_chat_uuid: *lobby.chat_room.key(),
    };

    for p in player.into_iter().filter(|x| *x == uuid) {
        if let Err(err) = ws_manager_chan
            .send(WsManagerMessage::SendMessage(p, msg.clone()))
            .await
        {
            error!("Could not send to ws manager chan: {err}");
            return Err(ApiError::InternalServerError);
        }
    }

    Ok(Json(StartGameResponse {
        game_uuid,
        game_chat_uuid,
    }))
}
