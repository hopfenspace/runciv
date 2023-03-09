use std::cmp::Ordering;

use actix_toolbox::tb_middleware::Session;
use actix_web::get;
use actix_web::web::{Data, Json, Path};
use chrono::{DateTime, Utc};
use itertools::Itertools;
use rorm::{and, query, Database, Model};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::models::{ChatRoom, ChatRoomMember, ChatRoomMessage};
use crate::server::handler::{AccountResponse, ApiError, ApiResult};

/// The message of a chatroom
///
/// The parameter `id` should be used to uniquely identify a message
#[derive(Serialize, ToSchema, Eq, Deserialize, Clone, Debug)]
pub struct ChatMessage {
    #[schema(example = 1337)]
    id: i64,
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
        self.id == other.id
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

/// The id of a chat
#[derive(Deserialize, IntoParams)]
pub struct ChatId {
    #[param(example = 1337)]
    id: u64,
}

/// Retrieve the messages of a chatroom
///
/// `messages` should be sorted by the datetime of `message.created_at`.
/// `message.id` should be used to uniquely identify chat messages.
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
    params(ChatId),
    security(("session_cookie" = []))
)]
#[get("/chats/{id}")]
pub async fn get_chat(
    path: Path<ChatId>,
    db: Data<Database>,
    session: Session,
) -> ApiResult<Json<GetChatResponse>> {
    let uuid: Vec<u8> = session.get("uuid")?.ok_or(ApiError::SessionCorrupt)?;

    let mut tx = db.start_transaction().await?;

    query!(&db, (ChatRoom::F.id,))
        .transaction(&mut tx)
        .condition(ChatRoom::F.id.equals(path.id as i64))
        .optional()
        .await?
        .ok_or(ApiError::InvalidId)?;

    // Check if user is allowed to access chat data
    let user_count = query!(&db, (ChatRoomMember::F.id.count(),))
        .transaction(&mut tx)
        .condition(and!(
            ChatRoomMember::F.chat_room.equals(path.id as i64),
            ChatRoomMember::F.member.f().uuid.equals(&uuid)
        ))
        .one()
        .await?
        .0
        // This unwrap is fine as count always return a value
        .unwrap();

    if user_count == 0 {
        return Err(ApiError::MissingPrivileges);
    }

    let members = query!(
        &db,
        (
            ChatRoomMember::F.created_at,
            ChatRoomMember::F.member.f().uuid,
            ChatRoomMember::F.member.f().username,
            ChatRoomMember::F.member.f().display_name
        )
    )
    .transaction(&mut tx)
    .condition(ChatRoomMember::F.chat_room.equals(path.id as i64))
    .all()
    .await?;

    let messages = query!(
        &db,
        (
            ChatRoomMessage::F.id,
            ChatRoomMessage::F.message,
            ChatRoomMessage::F.created_at,
            ChatRoomMessage::F.sender.f().uuid,
            ChatRoomMessage::F.sender.f().username,
            ChatRoomMessage::F.sender.f().display_name
        )
    )
    .transaction(&mut tx)
    .condition(ChatRoomMessage::F.chat_room.equals(path.id as i64))
    .all()
    .await?;

    tx.commit().await?;

    Ok(Json(GetChatResponse {
        messages: messages
            .into_iter()
            .map(
                |(id, message, created_at, sender_uuid, sender_username, sender_display_name)| {
                    ChatMessage {
                        id,
                        message,
                        created_at: DateTime::from_utc(created_at, Utc),
                        sender: AccountResponse {
                            uuid: Uuid::from_slice(&sender_uuid).unwrap(),
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
                        uuid: Uuid::from_slice(&m_uuid).unwrap(),
                        username: m_username,
                        display_name: m_display_name,
                    },
                },
            )
            .collect(),
    }))
}
