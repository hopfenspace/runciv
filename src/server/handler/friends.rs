use actix_toolbox::tb_middleware::Session;
use actix_web::web::{Data, Json, Path};
use actix_web::{delete, get, post, put, HttpResponse};
use log::error;
use rorm::fields::ForeignModelByField;
use rorm::{and, insert, or, query, update, Database, Model};
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::chan::{WsManagerChan, WsManagerMessage};
use crate::models::{
    Account, ChatRoomInsert, ChatRoomMemberInsert, Friend, FriendInsert, FriendWithChatInsert,
};
use crate::server::handler::{
    AccountResponse, ApiError, ApiResult, OnlineAccountResponse, PathUuid,
};

/// A single friend
#[derive(Serialize, ToSchema)]
pub struct FriendResponse {
    uuid: Uuid,
    chat_uuid: Uuid,
    friend: OnlineAccountResponse,
}

/// A single friend request
#[derive(Serialize, ToSchema)]
pub struct FriendRequestResponse {
    uuid: Uuid,
    from: AccountResponse,
    to: AccountResponse,
}

/// A list of your friends and friend requests
///
/// `friends` is a list of already established friendships
/// `friend_requests` is a list of friend requests (ingoing and outgoing)
#[derive(Serialize, ToSchema)]
pub struct GetFriendResponse {
    friends: Vec<FriendResponse>,
    friend_requests: Vec<FriendRequestResponse>,
}

/// Retrieve your friends and friend requests.
///
/// `friends` is a list of already established friendships
/// `friend_requests` is a list of friend requests (ingoing and outgoing)
///
/// Regarding `friend_requests`:
///
/// If you have a request with `from.uuid` equal to your username, it means you have requested a
/// friendship, but the destination hasn't accepted yet.
///
/// In the other case, if your username is in `to.uuid`, you have received a friend request.
#[utoipa::path(
    tag = "Friends",
    context_path = "/api/v2",
    responses(
        (status = 200, description = "Returns all friends and friend requests", body = GetFriendResponse),
        (status = 400, description = "Client error", body = ApiErrorResponse),
        (status = 500, description = "Server error", body = ApiErrorResponse),
    ),
    security(("session_cookie" = []))
)]
#[get("/friends")]
pub async fn get_friends(
    db: Data<Database>,
    session: Session,
    ws_manager_chan: Data<WsManagerChan>,
) -> ApiResult<Json<GetFriendResponse>> {
    let uuid: Uuid = session.get("uuid")?.ok_or(ApiError::SessionCorrupt)?;

    let mut tx = db.start_transaction().await?;

    let mut friend_requests = vec![];

    let friends_raw = query!(
        &mut tx,
        (
            Friend::F.uuid,
            Friend::F.to.uuid,
            Friend::F.to.username,
            Friend::F.to.display_name,
            Friend::F.chat_room,
        )
    )
    .condition(and!(
        Friend::F.from.equals(uuid.as_ref()),
        Friend::F.is_request.equals(false)
    ))
    .all()
    .await?;

    let (oneshot_tx, oneshot_rx) = oneshot::channel();
    let online_state = tokio::spawn(async move { oneshot_rx.await });
    if let Err(err) = ws_manager_chan
        .send(WsManagerMessage::RetrieveOnlineState(
            friends_raw.iter().map(|raw| raw.1).collect(),
            oneshot_tx,
        ))
        .await
    {
        error!("Could not send to ws manager chan: {err}");
        return Err(ApiError::InternalServerError);
    }

    let online_state = match online_state.await {
        Ok(res) => match res {
            Ok(state) => state,
            Err(err) => {
                error!("Error receiving online state from ws manager chan: {err}");
                return Err(ApiError::InternalServerError);
            }
        },
        Err(err) => {
            error!("Error joining task: {err}");
            return Err(ApiError::InternalServerError);
        }
    };

    // Retrieve all friendships
    let friends = Vec::from_iter(friends_raw.into_iter().zip(online_state).map(
        |((uuid, to_uuid, to_username, to_display_name, chat_room), online)| {
            // As all friend that are not in request state should have a chat room, this should be
            // fine unless the database is in an invalid state
            #[allow(clippy::unwrap_used)]
            FriendResponse {
                uuid,
                chat_uuid: *chat_room.unwrap().key(),
                friend: OnlineAccountResponse {
                    uuid: to_uuid,
                    username: to_username,
                    display_name: to_display_name,
                    online,
                },
            }
        },
    ));

    // Retrieve all incoming requests
    friend_requests.extend(
        query!(
            &mut tx,
            (
                Friend::F.uuid,
                Friend::F.from.uuid,
                Friend::F.from.username,
                Friend::F.from.display_name,
                Friend::F.to.uuid,
                Friend::F.to.username,
                Friend::F.to.display_name,
            )
        )
        .condition(and!(
            Friend::F.to.equals(uuid.as_ref()),
            Friend::F.is_request.equals(true)
        ))
        .all()
        .await?
        .into_iter()
        .map(
            |(
                uuid,
                from_uuid,
                from_username,
                from_display_name,
                to_uuid,
                to_username,
                to_display_name,
            )| FriendRequestResponse {
                uuid,
                from: AccountResponse {
                    uuid: from_uuid,
                    username: from_username,
                    display_name: from_display_name,
                },
                to: AccountResponse {
                    uuid: to_uuid,
                    username: to_username,
                    display_name: to_display_name,
                },
            },
        ),
    );

    // Retrieve all outgoing requests
    friend_requests.extend(
        query!(
            &mut tx,
            (
                Friend::F.uuid,
                Friend::F.from.uuid,
                Friend::F.from.username,
                Friend::F.from.display_name,
                Friend::F.to.uuid,
                Friend::F.to.username,
                Friend::F.to.display_name,
            )
        )
        .condition(and!(
            Friend::F.from.equals(uuid.as_ref()),
            Friend::F.is_request.equals(true)
        ))
        .all()
        .await?
        .into_iter()
        .map(
            |(
                uuid,
                from_uuid,
                from_username,
                from_display_name,
                to_uuid,
                to_username,
                to_display_name,
            )| FriendRequestResponse {
                uuid,
                from: AccountResponse {
                    uuid: from_uuid,
                    username: from_username,
                    display_name: from_display_name,
                },
                to: AccountResponse {
                    uuid: to_uuid,
                    username: to_username,
                    display_name: to_display_name,
                },
            },
        ),
    );

    tx.commit().await?;

    Ok(Json(GetFriendResponse {
        friends,
        friend_requests,
    }))
}

/// The request of a new friendship
#[derive(Deserialize, ToSchema)]
pub struct CreateFriendRequest {
    /// The uuid of the new friend
    uuid: Uuid,
}

/// Create a new friend request
#[utoipa::path(
    tag = "Friends",
    context_path = "/api/v2",
    responses(
        (status = 200, description = "Friend request has been created"),
        (status = 400, description = "Client error", body = ApiErrorResponse),
        (status = 500, description = "Server error", body = ApiErrorResponse),
    ),
    request_body = CreateFriendRequest,
    security(("session_cookie" = []))
)]
#[post("/friends")]
pub async fn create_friend_request(
    req: Json<CreateFriendRequest>,
    db: Data<Database>,
    session: Session,
) -> ApiResult<HttpResponse> {
    let uuid: Uuid = session.get("uuid")?.ok_or(ApiError::SessionCorrupt)?;

    let mut tx = db.start_transaction().await?;

    // Check if target exists
    let target = query!(&mut tx, Account)
        .condition(Account::F.uuid.equals(req.uuid.as_bytes().as_slice()))
        .optional()
        .await?
        .ok_or(ApiError::InvalidUuid)?;

    // Check if users are already in a friendship
    if let Some(friendship) = query!(&mut tx, Friend)
        .condition(or!(
            and!(
                Friend::F.from.equals(uuid.as_ref()),
                Friend::F.to.equals(target.uuid.as_ref())
            ),
            and!(
                Friend::F.from.equals(target.uuid.as_ref()),
                Friend::F.to.equals(uuid.as_ref())
            )
        ))
        .optional()
        .await?
    {
        return if friendship.is_request {
            Err(ApiError::FriendshipAlreadyRequested)
        } else {
            Err(ApiError::AlreadyFriends)
        };
    }

    // Create new friendship request
    insert!(&mut tx, FriendInsert)
        .single(&FriendInsert {
            uuid: Uuid::new_v4(),
            is_request: true,
            from: ForeignModelByField::Key(uuid),
            to: ForeignModelByField::Key(target.uuid),
        })
        .await?;

    tx.commit().await?;

    Ok(HttpResponse::Ok().finish())
}

/// Don't want your friends anymore? Just delete them!
#[utoipa::path(
    tag = "Friends",
    context_path = "/api/v2",
    responses(
        (status = 200, description = "Friend has been deleted"),
        (status = 400, description = "Client error", body = ApiErrorResponse),
        (status = 500, description = "Server error", body = ApiErrorResponse),
    ),
    params(PathUuid),
    security(("session_cookie" = []))
)]
#[delete("/friends/{uuid}")]
pub async fn delete_friend(
    path: Path<PathUuid>,
    db: Data<Database>,
    session: Session,
) -> ApiResult<HttpResponse> {
    let uuid: Uuid = session.get("uuid")?.ok_or(ApiError::SessionCorrupt)?;

    let mut tx = db.start_transaction().await?;

    // Check if friend exists
    let f = query!(&mut tx, Friend)
        .condition(Friend::F.uuid.equals(path.uuid.as_ref()))
        .optional()
        .await?
        .ok_or(ApiError::InvalidUuid)?;

    // If executing user is neither from nor to, return permission denied
    if *f.from.key() != uuid && *f.to.key() != uuid {
        return Err(ApiError::MissingPrivileges);
    }

    rorm::delete!(&mut tx, Friend)
        .condition(Friend::F.uuid.equals(f.uuid.as_ref()))
        .await?;

    tx.commit().await?;

    Ok(HttpResponse::Ok().finish())
}

/// Accept a friend request
#[utoipa::path(
    tag = "Friends",
    context_path = "/api/v2",
    responses(
        (status = 200, description = "Friend request accepted"),
        (status = 400, description = "Client error", body = ApiErrorResponse),
        (status = 500, description = "Server error", body = ApiErrorResponse),
    ),
    params(PathUuid),
    security(("session_cookie" = []))
)]
#[put("/friends/{uuid}")]
pub async fn accept_friend_request(
    path: Path<PathUuid>,
    session: Session,
    db: Data<Database>,
) -> ApiResult<HttpResponse> {
    let uuid: Uuid = session.get("uuid")?.ok_or(ApiError::SessionCorrupt)?;

    let mut tx = db.start_transaction().await?;

    // Check if friend request exists
    let f = query!(&mut tx, Friend)
        .condition(and!(
            Friend::F.uuid.equals(path.uuid.as_ref()),
            Friend::F.is_request.equals(true)
        ))
        .optional()
        .await?
        .ok_or(ApiError::InvalidUuid)?;

    // If executing user is neither from nor to, return permission denied
    if *f.from.key() != uuid && *f.to.key() != uuid {
        return Err(ApiError::MissingPrivileges);
    }

    // Create the chat room for both users
    let chat_room_uuid = insert!(&mut tx, ChatRoomInsert)
        .return_primary_key()
        .single(&ChatRoomInsert {
            uuid: Uuid::new_v4(),
        })
        .await?;

    insert!(&mut tx, ChatRoomMemberInsert)
        .bulk(&[
            ChatRoomMemberInsert {
                uuid: Uuid::new_v4(),
                chat_room: ForeignModelByField::Key(chat_room_uuid),
                member: ForeignModelByField::Key(*f.to.key()),
            },
            ChatRoomMemberInsert {
                uuid: Uuid::new_v4(),
                chat_room: ForeignModelByField::Key(chat_room_uuid),
                member: ForeignModelByField::Key(*f.from.key()),
            },
        ])
        .await?;

    update!(&mut tx, Friend)
        .condition(Friend::F.uuid.equals(path.uuid.as_ref()))
        .set(Friend::F.is_request, false)
        .set(Friend::F.chat_room, Some(chat_room_uuid.as_ref()))
        .exec()
        .await?;

    insert!(&mut tx, FriendWithChatInsert)
        .single(&FriendWithChatInsert {
            uuid: Uuid::new_v4(),
            is_request: false,
            from: ForeignModelByField::Key(*f.to.key()),
            to: ForeignModelByField::Key(*f.from.key()),
            chat_room: Some(ForeignModelByField::Key(chat_room_uuid)),
        })
        .await?;

    tx.commit().await?;

    Ok(HttpResponse::Ok().finish())
}
