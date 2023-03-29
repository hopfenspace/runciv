use std::cmp::Ordering;

use actix_toolbox::tb_middleware::Session;
use actix_web::get;
use actix_web::web::{Data, Json, Path};
use chrono::{DateTime, Utc};
use itertools::Itertools;
use rorm::{and, query, Database, Model};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::models::{ChatRoom, ChatRoomMember, ChatRoomMessage, Friend, GameAccount, LobbyAccount};
use crate::server::handler::{AccountResponse, ApiError, ApiResult, PathUuid};

/// The message of a chatroom
///
/// The parameter `uuid` is used to uniquely identify a message
#[derive(Serialize, ToSchema, Eq, Deserialize, Clone, Debug)]
pub struct ChatMessage {
    uuid: Uuid,
    sender: AccountResponse,
    #[schema(example = "Hello there!")]
    message: String,
    created_at: DateTime<Utc>,
}

impl Ord for ChatMessage {
    fn cmp(&self, other: &Self) -> Ordering {
        self.created_at.cmp(&other.created_at)
    }
}

impl PartialOrd for ChatMessage {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.created_at.partial_cmp(&other.created_at)
    }
}

impl PartialEq for ChatMessage {
    fn eq(&self, other: &Self) -> bool {
        self.uuid == other.uuid
    }
}

/// A member of a chatroom
#[derive(Serialize, ToSchema)]
pub struct ChatMember {
    #[serde(flatten)]
    account: AccountResponse,
    joined_at: DateTime<Utc>,
}

/// The response to a get chat
///
/// `messages` should be sorted by the datetime of `message.created_at`.
#[derive(Serialize, ToSchema)]
pub struct GetChatResponse {
    members: Vec<ChatMember>,
    messages: Vec<ChatMessage>,
}

/// Retrieve the messages of a chatroom
///
/// `messages` should be sorted by the datetime of `message.created_at`.
/// `message.uuid` should be used to uniquely identify chat messages.
/// This is needed as new messages are delivered via websocket
///
/// `members` holds information about all members that are currently in the chat room (including
/// yourself)
#[utoipa::path(
    tag = "Chats",
    context_path = "/api/v2",
    responses(
        (status = 200, description = "Returns the messages of the chat room", body = GetChatResponse),
        (status = 400, description = "Client error", body = ApiErrorResponse),
        (status = 500, description = "Server error", body = ApiErrorResponse),
    ),
    params(PathUuid),
    security(("session_cookie" = []))
)]
#[get("/chats/{uuid}")]
pub async fn get_chat(
    path: Path<PathUuid>,
    db: Data<Database>,
    session: Session,
) -> ApiResult<Json<GetChatResponse>> {
    let uuid: Uuid = session.get("uuid")?.ok_or(ApiError::SessionCorrupt)?;

    let mut tx = db.start_transaction().await?;

    query!(&mut tx, (ChatRoom::F.uuid,))
        .condition(ChatRoom::F.uuid.equals(path.uuid.as_ref()))
        .optional()
        .await?
        .ok_or(ApiError::InvalidUuid)?;

    // Check if user is allowed to access chat data
    let user_count = query!(&mut tx, (ChatRoomMember::F.uuid.count(),))
        .condition(and!(
            ChatRoomMember::F.chat_room.equals(path.uuid.as_ref()),
            ChatRoomMember::F.member.uuid.equals(uuid.as_ref())
        ))
        .one()
        .await?
        .0;

    if user_count == 0 {
        return Err(ApiError::MissingPrivileges);
    }

    let members = query!(
        &mut tx,
        (
            ChatRoomMember::F.created_at,
            ChatRoomMember::F.member.uuid,
            ChatRoomMember::F.member.username,
            ChatRoomMember::F.member.display_name
        )
    )
    .condition(ChatRoomMember::F.chat_room.equals(path.uuid.as_ref()))
    .all()
    .await?;

    let messages = query!(
        &mut tx,
        (
            ChatRoomMessage::F.uuid,
            ChatRoomMessage::F.message,
            ChatRoomMessage::F.created_at,
            ChatRoomMessage::F.sender.uuid,
            ChatRoomMessage::F.sender.username,
            ChatRoomMessage::F.sender.display_name
        )
    )
    .condition(ChatRoomMessage::F.chat_room.equals(path.uuid.as_ref()))
    .all()
    .await?;

    tx.commit().await?;

    Ok(Json(GetChatResponse {
        messages: messages
            .into_iter()
            .map(
                |(uuid, message, created_at, sender_uuid, sender_username, sender_display_name)| {
                    ChatMessage {
                        uuid,
                        message,
                        created_at: DateTime::from_utc(created_at, Utc),
                        sender: AccountResponse {
                            uuid: sender_uuid,
                            username: sender_username,
                            display_name: sender_display_name,
                        },
                    }
                },
            )
            .sorted()
            .collect(),
        members: members
            .into_iter()
            .map(
                |(created_at, m_uuid, m_username, m_display_name)| ChatMember {
                    joined_at: DateTime::from_utc(created_at, Utc),
                    account: AccountResponse {
                        uuid: m_uuid,
                        username: m_username,
                        display_name: m_display_name,
                    },
                },
            )
            .collect(),
    }))
}

/// All chat rooms your user has access to
#[derive(Serialize, ToSchema)]
pub struct GetAllChatsResponse {
    friend_chat_rooms: Vec<Uuid>,
    lobby_chat_rooms: Vec<Uuid>,
    game_chat_rooms: Vec<Uuid>,
}

/// Retrieve all chats the executing user has access to.
///
/// In the response, you will find different categories.
#[utoipa::path(
    tag = "Chats",
    context_path = "/api/v2",
    responses(
        (status = 200, description = "Returns the messages of the chat room", body = GetAllChatsResponse),
        (status = 400, description = "Client error", body = ApiErrorResponse),
        (status = 500, description = "Server error", body = ApiErrorResponse),
    ),
    security(("session_cookie" = []))
)]
#[get("/chats")]
pub async fn get_all_chats(
    db: Data<Database>,
    session: Session,
) -> ApiResult<Json<GetAllChatsResponse>> {
    let uuid: Uuid = session.get("uuid")?.ok_or(ApiError::SessionCorrupt)?;

    let mut tx = db.start_transaction().await?;

    let friend_chat_room_uuids = query!(&mut tx, (Friend::F.chat_room.uuid,))
        .condition(and!(
            Friend::F.is_request.equals(false),
            Friend::F.from.uuid.equals(uuid.as_ref())
        ))
        .all()
        .await?;

    let lobby_chat_room_uuids = query!(&mut tx, (LobbyAccount::F.lobby.chat_room.uuid))
        .condition(LobbyAccount::F.player.uuid.equals(uuid.as_ref()))
        .all()
        .await?;

    let game_chat_room_uuids = query!(&mut tx, (GameAccount::F.game.chat_room.uuid,))
        .condition(GameAccount::F.uuid.equals(uuid.as_ref()))
        .all()
        .await?;

    tx.commit().await?;

    Ok(Json(GetAllChatsResponse {
        lobby_chat_rooms: lobby_chat_room_uuids.into_iter().map(|x| x.0).collect(),
        friend_chat_rooms: friend_chat_room_uuids.into_iter().map(|x| x.0).collect(),
        game_chat_rooms: game_chat_room_uuids.into_iter().map(|x| x.0).collect(),
    }))
}
