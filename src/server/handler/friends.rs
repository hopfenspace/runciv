use actix_toolbox::tb_middleware::Session;
use actix_web::web::{Data, Json, Path};
use actix_web::{delete, get, post, put, HttpResponse};
use log::error;
use rorm::internal::field::foreign_model::ForeignModelByField;
use rorm::{and, insert, or, query, update, Database, Model};
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::chan::{WsManagerChan, WsManagerMessage};
use crate::models::{Account, ChatRoomInsert, ChatRoomMemberInsert, Friend, FriendInsert};
use crate::server::handler::{AccountResponse, ApiError, ApiResult, OnlineAccountResponse};

/// A single friend
#[derive(Serialize, ToSchema)]
pub struct FriendResponse {
    #[schema(example = 1337)]
    id: u64,
    #[schema(example = 1337)]
    chat_id: u64,
    from: AccountResponse,
    to: OnlineAccountResponse,
}

/// A single friend request
#[derive(Serialize, ToSchema)]
pub struct FriendRequestResponse {
    #[schema(example = 1337)]
    id: u64,
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
    let uuid: Vec<u8> = session.get("uuid")?.ok_or(ApiError::SessionCorrupt)?;

    let mut tx = db.start_transaction().await?;

    let mut friend_requests = vec![];

    let friends_raw = query!(
        &db,
        (
            Friend::F.id,
            Friend::F.from.fields().uuid,
            Friend::F.from.fields().username,
            Friend::F.from.fields().display_name,
            Friend::F.to.fields().uuid,
            Friend::F.to.fields().username,
            Friend::F.to.fields().display_name,
            Friend::F.chat_room,
        )
    )
    .transaction(&mut tx)
    .condition(and!(
        Friend::F.from.equals(&uuid),
        Friend::F.is_request.equals(false)
    ))
    .all()
    .await?;

    let (oneshot_tx, oneshot_rx) = oneshot::channel();
    let online_state = tokio::spawn(async move { oneshot_rx.await });
    if let Err(err) = ws_manager_chan
        .send(WsManagerMessage::RetrieveOnlineState(
            friends_raw.iter().map(|raw| raw.4.clone()).collect(),
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
        |(
            (
                id,
                from_uuid,
                from_username,
                from_display_name,
                to_uuid,
                to_username,
                to_display_name,
                chat_room,
            ),
            online,
        )| FriendResponse {
            id: id as u64,
            chat_id: match chat_room {
                ForeignModelByField::Key(v) => v,
                ForeignModelByField::Instance(v) => v.id,
            } as u64,
            from: AccountResponse {
                uuid: Uuid::from_slice(&from_uuid).unwrap(),
                username: from_username,
                display_name: from_display_name,
            },
            to: OnlineAccountResponse {
                uuid: Uuid::from_slice(&to_uuid).unwrap(),
                username: to_username,
                display_name: to_display_name,
                online,
            },
        },
    ));

    // Retrieve all incoming requests
    friend_requests.extend(
        query!(
            &db,
            (
                Friend::F.id,
                Friend::F.from.fields().uuid,
                Friend::F.from.fields().username,
                Friend::F.from.fields().display_name,
                Friend::F.to.fields().uuid,
                Friend::F.to.fields().username,
                Friend::F.to.fields().display_name,
            )
        )
        .transaction(&mut tx)
        .condition(and!(
            Friend::F.to.equals(&uuid),
            Friend::F.is_request.equals(true)
        ))
        .all()
        .await?
        .into_iter()
        .map(
            |(
                id,
                from_uuid,
                from_username,
                from_display_name,
                to_uuid,
                to_username,
                to_display_name,
            )| FriendRequestResponse {
                id: id as u64,
                from: AccountResponse {
                    uuid: Uuid::from_slice(&from_uuid).unwrap(),
                    username: from_username,
                    display_name: from_display_name,
                },
                to: AccountResponse {
                    uuid: Uuid::from_slice(&to_uuid).unwrap(),
                    username: to_username,
                    display_name: to_display_name,
                },
            },
        ),
    );

    // Retrieve all outgoing requests
    friend_requests.extend(
        query!(
            &db,
            (
                Friend::F.id,
                Friend::F.from.fields().uuid,
                Friend::F.from.fields().username,
                Friend::F.from.fields().display_name,
                Friend::F.to.fields().uuid,
                Friend::F.to.fields().username,
                Friend::F.to.fields().display_name,
            )
        )
        .transaction(&mut tx)
        .condition(and!(
            Friend::F.from.equals(&uuid),
            Friend::F.is_request.equals(true)
        ))
        .all()
        .await?
        .into_iter()
        .map(
            |(
                id,
                from_uuid,
                from_username,
                from_display_name,
                to_uuid,
                to_username,
                to_display_name,
            )| FriendRequestResponse {
                id: id as u64,
                from: AccountResponse {
                    uuid: Uuid::from_slice(&from_uuid).unwrap(),
                    username: from_username,
                    display_name: from_display_name,
                },
                to: AccountResponse {
                    uuid: Uuid::from_slice(&to_uuid).unwrap(),
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
    let uuid: Vec<u8> = session.get("uuid")?.ok_or(ApiError::SessionCorrupt)?;

    let mut tx = db.start_transaction().await?;

    // Check if target exists
    let target = query!(&db, Account)
        .transaction(&mut tx)
        .condition(Account::F.uuid.equals(req.uuid.as_bytes().as_slice()))
        .optional()
        .await?
        .ok_or(ApiError::InvalidUsername)?;

    // Check if users are already in a friendship
    if let Some(friendship) = query!(&db, Friend)
        .transaction(&mut tx)
        .condition(or!(
            and!(
                Friend::F.from.equals(&uuid),
                Friend::F.to.equals(&target.uuid)
            ),
            and!(
                Friend::F.from.equals(&target.uuid),
                Friend::F.to.equals(&uuid)
            )
        ))
        .optional()
        .await?
    {
        if friendship.is_request {
            return Err(ApiError::FriendshipAlreadyRequested);
        } else {
            return Err(ApiError::AlreadyFriends);
        }
    }

    // Create new friendship request
    insert!(&db, FriendInsert)
        .transaction(&mut tx)
        .single(&FriendInsert {
            is_request: true,
            from: ForeignModelByField::Key(uuid),
            to: ForeignModelByField::Key(target.uuid),
        })
        .await?;

    tx.commit().await?;

    Ok(HttpResponse::Ok().finish())
}

/// The id of a friend or friend request
#[derive(Deserialize, IntoParams)]
pub struct FriendId {
    #[param(example = 1337)]
    id: u64,
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
    params(FriendId),
    security(("session_cookie" = []))
)]
#[delete("/friends/{id}")]
pub async fn delete_friend(
    path: Path<FriendId>,
    db: Data<Database>,
    session: Session,
) -> ApiResult<HttpResponse> {
    let uuid: Vec<u8> = session.get("uuid")?.ok_or(ApiError::SessionCorrupt)?;

    let mut tx = db.start_transaction().await?;

    // Check if friend exists
    let f = query!(&db, Friend)
        .transaction(&mut tx)
        .condition(Friend::F.id.equals(path.id as i64))
        .optional()
        .await?
        .ok_or(ApiError::InvalidId)?;

    let from = match &f.from {
        ForeignModelByField::Key(k) => k.clone(),
        _ => unreachable!("Not queried"),
    };

    let to = match &f.to {
        ForeignModelByField::Key(k) => k.clone(),
        _ => unreachable!("Not queried"),
    };

    // If executing user is neither from nor to, return permission denied
    if from != uuid && to != uuid {
        return Err(ApiError::MissingPrivileges);
    }

    rorm::delete!(&db, Friend)
        .transaction(&mut tx)
        .single(&f)
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
    params(FriendId),
    security(("session_cookie" = []))
)]
#[put("/friends/{id}")]
pub async fn accept_friend_request(
    path: Path<FriendId>,
    session: Session,
    db: Data<Database>,
) -> ApiResult<HttpResponse> {
    let uuid: Vec<u8> = session.get("uuid")?.ok_or(ApiError::SessionCorrupt)?;

    let mut tx = db.start_transaction().await?;

    // Check if friend request exists
    let f = query!(&db, Friend)
        .transaction(&mut tx)
        .condition(and!(
            Friend::F.id.equals(path.id as i64),
            Friend::F.is_request.equals(true)
        ))
        .optional()
        .await?
        .ok_or(ApiError::InvalidId)?;

    let from = match &f.from {
        ForeignModelByField::Key(k) => k.clone(),
        _ => unreachable!("Not queried"),
    };

    let to = match &f.to {
        ForeignModelByField::Key(k) => k.clone(),
        _ => unreachable!("Not queried"),
    };

    // If executing user is neither from nor to, return permission denied
    if from != uuid && to != uuid {
        return Err(ApiError::MissingPrivileges);
    }

    update!(&db, Friend)
        .transaction(&mut tx)
        .set(Friend::F.is_request, false)
        .exec()
        .await?;

    insert!(&db, FriendInsert)
        .transaction(&mut tx)
        .single(&FriendInsert {
            is_request: false,
            from: ForeignModelByField::Key(to.clone()),
            to: ForeignModelByField::Key(from.clone()),
        })
        .await?;

    let chat_room_id = insert!(&db, ChatRoomInsert)
        .transaction(&mut tx)
        .single(&ChatRoomInsert {})
        .await?;

    insert!(&db, ChatRoomMemberInsert)
        .transaction(&mut tx)
        .bulk(&[
            ChatRoomMemberInsert {
                chat_room: ForeignModelByField::Key(chat_room_id),
                member: ForeignModelByField::Key(to),
            },
            ChatRoomMemberInsert {
                chat_room: ForeignModelByField::Key(chat_room_id),
                member: ForeignModelByField::Key(from),
            },
        ])
        .await?;

    tx.commit().await?;

    Ok(HttpResponse::Ok().finish())
}
