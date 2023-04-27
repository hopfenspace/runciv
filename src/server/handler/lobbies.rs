use std::iter;

use actix_toolbox::tb_middleware::Session;
use actix_web::web::{Data, Json, Path};
use actix_web::{delete, get, post, HttpResponse};
use argon2::password_hash::{Error, SaltString};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use chrono::{DateTime, Utc};
use log::{error, warn};
use rand::thread_rng;
use rorm::fields::{BackRef, ForeignModelByField};
use rorm::{and, insert, query, update, Database, Model};
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::chan::{WsManagerChan, WsManagerMessage, WsMessage};
use crate::models::{
    Account, ChatRoomInsert, ChatRoomMember, ChatRoomMemberInsert, ChatRoomMessage,
    GameAccountInsert, GameInsert, Lobby, LobbyAccount, LobbyAccountInsert, LobbyInsert,
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
pub async fn get_all_lobbies(db: Data<Database>) -> ApiResult<Json<GetLobbiesResponse>> {
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
            Lobby::F.chat_room,
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
                chat_room: ForeignModelByField::Key(*chat_room_uuid.key()),
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

/// A single lobby
#[derive(Serialize, ToSchema)]
pub struct GetLobbyResponse {
    uuid: Uuid,
    #[schema(example = "Herbert's lobby")]
    name: String,
    #[schema(example = 4)]
    max_players: u8,
    created_at: DateTime<Utc>,
    password: bool,
    owner: AccountResponse,
    current_players: Vec<AccountResponse>,
    chat_room_uuid: Uuid,
}

/// Retrieves an open lobbies.
///
/// If `password` is `true`, the lobby is secured by a user-set password
#[utoipa::path(
    tag = "Lobbies",
    context_path = "/api/v2",
    responses(
        (status = 200, description = "Returns all currently open lobbies", body = GetLobbyResponse),
        (status = 400, description = "Client error", body = ApiErrorResponse),
        (status = 500, description = "Server error", body = ApiErrorResponse),
    ),
    params(PathUuid),
    security(("session_cookie" = []))
)]
#[get("/lobbies/{uuid}")]
pub async fn get_lobby(
    path: Path<PathUuid>,
    db: Data<Database>,
) -> ApiResult<Json<GetLobbyResponse>> {
    let mut tx = db.start_transaction().await?;

    let (
        uuid,
        owner_uuid,
        owner_username,
        owner_display_name,
        name,
        created_at,
        max_player,
        password_hash,
        chat_room_uuid,
    ) = query!(
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
    .condition(Lobby::F.uuid.equals(path.uuid.as_ref()))
    .optional()
    .await?
    .ok_or(ApiError::InvalidUuid)?;

    let current_players = query!(
        &mut tx,
        (
            LobbyAccount::F.player.uuid,
            LobbyAccount::F.player.username,
            LobbyAccount::F.player.display_name,
        )
    )
    .condition(LobbyAccount::F.lobby.equals(uuid.as_ref()))
    .all()
    .await?;

    tx.commit().await?;

    // Ok as current_player is populated before
    #[allow(clippy::unwrap_used)]
    Ok(Json(GetLobbyResponse {
        uuid,
        name,
        owner: AccountResponse {
            uuid: owner_uuid,
            username: owner_username,
            display_name: owner_display_name,
        },
        current_players: current_players
            .into_iter()
            .map(|(uuid, username, display_name)| AccountResponse {
                uuid,
                username,
                display_name,
            })
            .collect(),
        max_players: max_player as u8,
        password: password_hash.is_some(),
        created_at: DateTime::from_utc(created_at, Utc),
        chat_room_uuid,
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
/// If you are not connected via websocket, an error is returned.
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
    ws_manager_chan: Data<WsManagerChan>,
) -> ApiResult<Json<CreateLobbyResponse>> {
    let uuid: Uuid = session.get("uuid")?.ok_or(ApiError::SessionCorrupt)?;

    let mut tx = db.start_transaction().await?;

    // Check if the request is valid
    if req.max_players < 2 || req.max_players > 34 {
        return Err(ApiError::InvalidMaxPlayersCount);
    }

    // Check if the websocket of the executing user is connected
    let (sender, receiver) = oneshot::channel();
    if let Err(err) = ws_manager_chan
        .send(WsManagerMessage::RetrieveOnlineState(uuid, sender))
        .await
    {
        warn!("Could not send to ws manager chan: {err}");
        return Err(ApiError::InternalServerError);
    }

    match receiver.await {
        Ok(online) => {
            if !online {
                return Err(ApiError::WsNotConnected);
            }
        }
        Err(err) => {
            warn!("Error receiving online state: {err}");
            return Err(ApiError::InternalServerError);
        }
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
    rorm::delete!(&mut tx, Lobby)
        .condition(Lobby::F.uuid.equals(path.uuid.as_ref()))
        .await?;

    tx.commit().await?;

    // Send notifications to all remaining players
    let msg = WsMessage::GameStarted {
        game_uuid,
        game_chat_uuid,
        lobby_uuid: lobby.uuid,
        lobby_chat_uuid: *lobby.chat_room.key(),
    };

    for p in player.into_iter().filter(|x| *x != uuid) {
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

/// The request to join a lobby
#[derive(Deserialize, ToSchema)]
pub struct JoinLobbyRequest {
    #[schema(example = "super-secure-password")]
    password: Option<String>,
}

/// Join an existing lobby
///
/// The executing user must not be the owner of a lobby or member of a lobby.
/// To be placed in a lobby, a active websocket connection is required.
///
/// As a lobby might be protected by password, the optional parameter `password` may be specified.
/// If the provided password was incorrect, the error [ApiError::MissingPrivileges] is returned.
/// If the lobby isn't protected, but a password was found in the request, it is ignored.
///
/// If the lobby is already full, a [ApiError::LobbyFull] error is returned.
///
/// On success, all players that were in the lobby before, are notified about the new player with a
/// [WsMessage::LobbyJoin] message.
#[utoipa::path(
    tag = "Lobbies",
    context_path = "/api/v2",
    responses(
        (status = 200, description = "Joined lobby successfully"),
        (status = 400, description = "Client error", body = ApiErrorResponse),
        (status = 500, description = "Server error", body = ApiErrorResponse),
    ),
    params(PathUuid),
    request_body = JoinLobbyRequest,
    security(("session_cookie" = []))
)]
#[post("/lobbies/{uuid}/join")]
pub async fn join_lobby(
    path: Path<PathUuid>,
    req: Json<JoinLobbyRequest>,
    db: Data<Database>,
    session: Session,
    ws_manager_chan: Data<WsManagerChan>,
) -> ApiResult<HttpResponse> {
    let uuid: Uuid = session.get("uuid")?.ok_or(ApiError::SessionCorrupt)?;

    let mut tx = db.start_transaction().await?;

    // Check if lobby exists
    let mut lobby = query!(&mut tx, Lobby)
        .condition(Lobby::F.uuid.equals(path.uuid.as_ref()))
        .optional()
        .await?
        .ok_or(ApiError::InvalidUuid)?;

    Lobby::F
        .current_player
        .populate(&mut tx, &mut lobby)
        .await?;

    // Ok as current_player is populated before
    #[allow(clippy::unwrap_used)]
    let current_player: Vec<LobbyAccount> = lobby.current_player.cached.unwrap();

    // Check if the lobby is already full

    if lobby.max_player as usize == current_player.len() + 1 {
        return Err(ApiError::LobbyFull);
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

    // If the lobby is password protected, check the hash
    if let Some(password_hash) = lobby.password_hash {
        let req_pw = req.password.clone().ok_or(ApiError::MissingPrivileges)?;
        Argon2::default()
            .verify_password(req_pw.as_bytes(), &PasswordHash::new(&password_hash)?)
            .map_err(|e| match e {
                Error::Password => ApiError::MissingPrivileges,
                _ => ApiError::InvalidHash(e),
            })?;
    }

    // Check if the websocket is connected
    let (sender, rx) = oneshot::channel();

    let msg = WsManagerMessage::RetrieveOnlineState(uuid, sender);
    if let Err(err) = ws_manager_chan.send(msg).await {
        warn!("Could not send to ws manager chan: {err}");
        return Err(ApiError::InternalServerError);
    }

    match rx.await {
        Ok(is_online) => {
            if !is_online {
                return Err(ApiError::WsNotConnected);
            }
        }
        Err(err) => {
            warn!("Error while receiving from oneshot channel: {err}");
            return Err(ApiError::InternalServerError);
        }
    }

    // Add player to lobby
    insert!(&mut tx, LobbyAccountInsert)
        .return_nothing()
        .single(&LobbyAccountInsert {
            uuid: Uuid::new_v4(),
            lobby: ForeignModelByField::Key(lobby.uuid),
            player: ForeignModelByField::Key(uuid),
        })
        .await?;

    let (uuid, username, display_name) = query!(
        &mut tx,
        (
            Account::F.uuid,
            Account::F.username,
            Account::F.display_name
        )
    )
    .condition(Account::F.uuid.equals(uuid.as_ref()))
    .optional()
    .await?
    .ok_or(ApiError::SessionCorrupt)?;

    // Add player to chatroom
    insert!(&mut tx, ChatRoomMemberInsert)
        .single(&ChatRoomMemberInsert {
            uuid: Uuid::new_v4(),
            member: ForeignModelByField::Key(uuid),
            chat_room: ForeignModelByField::Key(*lobby.chat_room.key()),
        })
        .await?;

    tx.commit().await?;

    let players: Vec<Uuid> = iter::once(*lobby.owner.key())
        .chain(current_player.into_iter().map(|x| *x.player.key()))
        .collect();

    let msg = WsMessage::LobbyJoin {
        lobby_uuid: lobby.uuid,
        player: AccountResponse {
            uuid,
            username,
            display_name,
        },
    };

    // Notify other players
    for player in players {
        if let Err(err) = ws_manager_chan
            .send(WsManagerMessage::SendMessage(player, msg.clone()))
            .await
        {
            warn!("Could not send to ws manager chan: {err}");
        }
    }

    Ok(HttpResponse::Ok().finish())
}

/// Close an open lobby
///
/// This endpoint can only be used by the lobby owner.
/// For joined users, see `POST /lobbies/{uuid}/leave`.
///
/// On success, all joined players will receive a [WsMessage::LobbyClosed] message via websocket.
#[utoipa::path(
    tag = "Lobbies",
    context_path = "/api/v2",
    responses(
        (status = 200, description = "Lobby closed"),
        (status = 400, description = "Client error", body = ApiErrorResponse),
        (status = 500, description = "Server error", body = ApiErrorResponse),
    ),
    params(PathUuid),
    security(("session_cookie" = []))
)]
#[delete("/lobbies/{uuid}")]
pub async fn close_lobby(
    path: Path<PathUuid>,
    db: Data<Database>,
    session: Session,
    ws_manager_chan: Data<WsManagerChan>,
) -> ApiResult<HttpResponse> {
    let uuid: Uuid = session.get("uuid")?.ok_or(ApiError::SessionCorrupt)?;

    let mut tx = db.start_transaction().await?;

    // Check if lobby exists
    let mut lobby = query!(&mut tx, Lobby)
        .condition(Lobby::F.uuid.equals(path.uuid.as_ref()))
        .optional()
        .await?
        .ok_or(ApiError::InvalidUuid)?;

    Lobby::F
        .current_player
        .populate(&mut tx, &mut lobby)
        .await?;

    // Ok as current_player is populated before
    #[allow(clippy::unwrap_used)]
    let current_player: Vec<LobbyAccount> = lobby.current_player.cached.unwrap();

    // Check if user has the privileges to close the lobby
    if *lobby.owner.key() != uuid {
        return Err(ApiError::MissingPrivileges);
    }

    rorm::delete!(&mut tx, Lobby)
        .condition(Lobby::F.uuid.equals(lobby.uuid.as_ref()))
        .await?;

    tx.commit().await?;

    let msg = WsMessage::LobbyClosed {
        lobby_uuid: lobby.uuid,
    };

    // Notify other players
    for player in current_player.into_iter().map(|x| *x.player.key()) {
        if let Err(err) = ws_manager_chan
            .send(WsManagerMessage::SendMessage(player, msg.clone()))
            .await
        {
            warn!("Error while sending message to ws manager chan: {err}");
        }
    }

    Ok(HttpResponse::Ok().finish())
}

/// Leave an open lobby
///
/// This endpoint can only be used by joined users.
/// For the lobby owner, you want to use `DELETE /lobbies/{uuid}`.
///
/// All players in the lobby will receive a [WsMessage::LobbyLeave] message via websocket on success.
#[utoipa::path(
    tag = "Lobbies",
    context_path = "/api/v2",
    responses(
        (status = 200, description = "Left the lobby"),
        (status = 400, description = "Client error", body = ApiErrorResponse),
        (status = 500, description = "Server error", body = ApiErrorResponse),
    ),
    params(PathUuid),
    security(("session_cookie" = []))
)]
#[post("/lobbies/{uuid}/leave")]
pub async fn leave_lobby(
    path: Path<PathUuid>,
    db: Data<Database>,
    session: Session,
    ws_manager_chan: Data<WsManagerChan>,
) -> ApiResult<HttpResponse> {
    let uuid: Uuid = session.get("uuid")?.ok_or(ApiError::SessionCorrupt)?;

    let mut tx = db.start_transaction().await?;

    // Check if lobby exists
    let mut lobby = query!(&mut tx, Lobby)
        .condition(Lobby::F.uuid.equals(path.uuid.as_ref()))
        .optional()
        .await?
        .ok_or(ApiError::InvalidUuid)?;

    Lobby::F
        .current_player
        .populate(&mut tx, &mut lobby)
        .await?;

    // Ok as current_player is populated before
    #[allow(clippy::unwrap_used)]
    let current_player: Vec<LobbyAccount> = lobby.current_player.cached.unwrap();

    // Check if executing user is in the lobby
    if !current_player.iter().any(|x| *x.player.key() == uuid) {
        return Err(ApiError::MissingPrivileges);
    }

    rorm::delete!(&mut tx, LobbyAccount)
        .condition(and!(
            LobbyAccount::F.lobby.equals(lobby.uuid.as_ref()),
            LobbyAccount::F.player.equals(uuid.as_ref())
        ))
        .await?;

    rorm::delete!(&mut tx, ChatRoomMember)
        .condition(and!(
            ChatRoomMember::F
                .chat_room
                .equals(lobby.chat_room.key().as_ref()),
            ChatRoomMember::F.member.equals(uuid.as_ref())
        ))
        .await?;

    let (uuid, username, display_name) = query!(
        &mut tx,
        (
            Account::F.uuid,
            Account::F.username,
            Account::F.display_name
        )
    )
    .condition(Account::F.uuid.equals(uuid.as_ref()))
    .optional()
    .await?
    .ok_or(ApiError::SessionCorrupt)?;

    tx.commit().await?;

    let msg = WsMessage::LobbyLeave {
        lobby_uuid: lobby.uuid,
        player: AccountResponse {
            uuid,
            username,
            display_name,
        },
    };

    let players =
        iter::once(*lobby.owner.key()).chain(current_player.into_iter().map(|x| *x.player.key()));

    // Notify other players
    for player in players {
        if let Err(err) = ws_manager_chan
            .send(WsManagerMessage::SendMessage(player, msg.clone()))
            .await
        {
            warn!("Error while sending message to ws manager chan: {err}");
        }
    }

    Ok(HttpResponse::Ok().finish())
}

/// The path parameter to kick a player
#[derive(Deserialize, IntoParams)]
pub struct PlayerKickPath {
    lobby_uuid: Uuid,
    player_uuid: Uuid,
}

/// Kick a player from an open lobby
///
/// This endpoint can only be used by the lobby owner.
///
/// All players in the lobby as well as the kick player will receive a [WsMessage::LobbyKick]
/// message via websocket on success.
#[utoipa::path(
    tag = "Lobbies",
    context_path = "/api/v2",
    responses(
        (status = 200, description = "Player was kicked"),
        (status = 400, description = "Client error", body = ApiErrorResponse),
        (status = 500, description = "Server error", body = ApiErrorResponse),
    ),
    params(PlayerKickPath),
    security(("session_cookie" = []))
)]
#[delete("/lobbies/{lobby_uuid}/{player_uuid}")]
pub async fn kick_player_from_lobby(
    path: Path<PlayerKickPath>,
    db: Data<Database>,
    session: Session,
    ws_manager_chan: Data<WsManagerChan>,
) -> ApiResult<HttpResponse> {
    let uuid: Uuid = session.get("uuid")?.ok_or(ApiError::SessionCorrupt)?;

    let mut tx = db.start_transaction().await?;

    // Check if lobby exists
    let mut lobby = query!(&mut tx, Lobby)
        .condition(Lobby::F.uuid.equals(path.lobby_uuid.as_ref()))
        .optional()
        .await?
        .ok_or(ApiError::InvalidUuid)?;

    Lobby::F
        .current_player
        .populate(&mut tx, &mut lobby)
        .await?;

    // Ok as current_player is populated before
    #[allow(clippy::unwrap_used)]
    let current_player: Vec<LobbyAccount> = lobby.current_player.cached.unwrap();

    // Check if executing user owns the lobby
    if *lobby.owner.key() != uuid {
        return Err(ApiError::MissingPrivileges);
    }

    // Check if the user to kick is in the lobby
    if !current_player
        .iter()
        .any(|x| *x.player.key() == path.player_uuid)
    {
        return Err(ApiError::InvalidPlayerUuid);
    }

    rorm::delete!(&mut tx, LobbyAccount)
        .condition(and!(
            LobbyAccount::F.lobby.equals(lobby.uuid.as_ref()),
            LobbyAccount::F.player.equals(path.player_uuid.as_ref())
        ))
        .await?;

    rorm::delete!(&mut tx, ChatRoomMember)
        .condition(and!(
            ChatRoomMember::F
                .chat_room
                .equals(lobby.chat_room.key().as_ref()),
            ChatRoomMember::F.member.equals(path.player_uuid.as_ref()),
        ))
        .await?;

    let (uuid, username, display_name) = query!(
        &mut tx,
        (
            Account::F.uuid,
            Account::F.username,
            Account::F.display_name
        )
    )
    .condition(Account::F.uuid.equals(path.player_uuid.as_ref()))
    .optional()
    .await?
    .ok_or(ApiError::SessionCorrupt)?;

    tx.commit().await?;

    let msg = WsMessage::LobbyKick {
        lobby_uuid: lobby.uuid,
        player: AccountResponse {
            uuid,
            username,
            display_name,
        },
    };

    // Notify joined players and kicked player
    for player in current_player.into_iter().map(|x| *x.player.key()) {
        if let Err(err) = ws_manager_chan
            .send(WsManagerMessage::SendMessage(player, msg.clone()))
            .await
        {
            warn!("Error while sending message to ws manager chan: {err}");
        }
    }

    Ok(HttpResponse::Ok().finish())
}
