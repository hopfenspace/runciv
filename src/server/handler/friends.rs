use actix_toolbox::tb_middleware::Session;
use actix_web::web::{Data, Json};
use actix_web::{post, HttpResponse};
use rorm::internal::field::foreign_model::ForeignModelByField;
use rorm::{and, insert, query, Database, Model};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::models::{Account, Friend, FriendInsert};
use crate::server::handler::{ApiError, ApiResult};

/// The request of a new friendship
#[derive(Deserialize, ToSchema)]
pub struct CreateFriendRequest {
    /// The username of the new friend
    #[schema(example = "user321")]
    username: String,
}

/// Create a new friend request
#[utoipa::path(
    tag = "Friends",
    context_path = "/api/v2",
    responses(
        (status = 202, description = "Friend request has been created"),
        (status = 400, description = "Client error", body = ApiErrorResponse),
        (status = 500, description = "Server error", body = ApiErrorResponse),
    ),
    request_body = CreateFriendRequest,
    security(("api_key" = []))
)]
#[post("/friends/request")]
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
        .condition(Account::F.username.equals(&req.username))
        .optional()
        .await?
        .ok_or(ApiError::InvalidUsername)?;

    // Check if users are already in a friendship
    if let Some(friendship) = query!(&db, Friend)
        .transaction(&mut tx)
        .condition(and!(
            Friend::F.from.equals(&uuid),
            Friend::F.to.equals(&target.uuid)
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
            from: ForeignModelByField::Key(uuid),
            to: ForeignModelByField::Key(target.uuid),
        })
        .await?;

    tx.commit().await?;

    Ok(HttpResponse::Created().finish())
}
