use actix_toolbox::tb_middleware::Session;
use actix_web::web::{Data, Json, Path};
use actix_web::{delete, get, post, HttpResponse};
use chrono::{DateTime, Utc};
use log::error;
use rorm::fields::ForeignModelByField;
use rorm::{and, insert, query, Database, Model};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::chan::{WsManagerChan, WsManagerMessage, WsMessage};
use crate::models::{Account, Friend, Invite, InviteInsert, Lobby, LobbyAccount};
use crate::server::handler::{AccountResponse, ApiError, ApiResult, PathUuid};

/// The request to invite a friend into a lobby
#[derive(Deserialize, ToSchema)]
pub struct CreateInviteRequest {
    friend_uuid: Uuid,
    lobby_uuid: Uuid,
}

/// Invite a friend to a lobby.
///
/// The executing user must be in the specified open lobby.
/// The invited `friend` must not be in a friend request state.
#[utoipa::path(
    tag = "Invites",
    context_path = "/api/v2",
    responses(
        (status = 200, description = "Friend got invited"),
        (status = 400, description = "Client error", body = ApiErrorResponse),
        (status = 500, description = "Server error", body = ApiErrorResponse),
    ),
    request_body = CreateInviteRequest,
    security(("session_cookie" = []))
)]
#[post("/invites")]
pub async fn create_invite(
    req: Json<CreateInviteRequest>,
    session: Session,
    db: Data<Database>,
    ws_manager_chan: Data<WsManagerChan>,
) -> ApiResult<HttpResponse> {
    let uuid: Uuid = session.get("uuid")?.ok_or(ApiError::SessionCorrupt)?;

    let mut tx = db.start_transaction().await?;

    // Check if lobby is currently open
    let lobby = query!(&mut tx, Lobby)
        .condition(Lobby::F.uuid.equals(req.lobby_uuid.as_ref()))
        .optional()
        .await?
        .ok_or(ApiError::InvalidLobbyUuid)?;

    // Check if the executing account has the privileges to invite to the specified lobby
    if *lobby.owner.key() != uuid
        && query!(&mut tx, LobbyAccount)
            .condition(and!(
                LobbyAccount::F.lobby.equals(lobby.uuid.as_ref()),
                LobbyAccount::F.player.equals(uuid.as_ref())
            ))
            .optional()
            .await?
            .is_none()
    {
        return Err(ApiError::MissingPrivileges);
    }

    // Check if specified friend is valid
    let friend_account = query!(&mut tx, Account)
        .condition(Account::F.uuid.equals(req.friend_uuid.as_ref()))
        .optional()
        .await?
        .ok_or(ApiError::InvalidUuid)?;

    // Check if there's a valid friendship
    let friend = query!(&mut tx, Friend)
        .condition(and!(
            Friend::F.is_request.equals(false),
            Friend::F.from.equals(uuid.as_ref()),
            Friend::F.to.equals(friend_account.uuid.as_ref())
        ))
        .optional()
        .await?
        .ok_or(ApiError::InvalidFriendState)?;

    let invite_uuid = insert!(&mut tx, InviteInsert)
        .return_primary_key()
        .single(&InviteInsert {
            uuid: Uuid::new_v4(),
            from: ForeignModelByField::Key(uuid),
            to: friend.to,
            lobby: ForeignModelByField::Key(lobby.uuid),
        })
        .await?;

    let executing_account = query!(&mut tx, Account)
        .condition(Account::F.uuid.equals(uuid.as_ref()))
        .optional()
        .await?
        .ok_or(ApiError::SessionCorrupt)?;

    tx.commit().await?;

    let invite = WsMessage::IncomingInvite {
        invite_uuid,
        lobby_uuid: lobby.uuid,
        from: AccountResponse {
            uuid: executing_account.uuid,
            username: executing_account.username,
            display_name: executing_account.display_name,
        },
    };

    if let Err(err) = ws_manager_chan
        .send(WsManagerMessage::SendMessage(uuid, invite))
        .await
    {
        error!("Could not send to ws manager chan: {err}");
    }

    Ok(HttpResponse::Ok().finish())
}

/// A single invite
#[derive(Serialize, ToSchema)]
pub struct GetInvite {
    uuid: Uuid,
    created_at: DateTime<Utc>,
    from: AccountResponse,
    lobby_uuid: Uuid,
}

/// The invites that an account has received
#[derive(Serialize, ToSchema)]
pub struct GetInvitesResponse {
    invites: Vec<GetInvite>,
}

/// Retrieve all invites for the executing user
#[utoipa::path(
    tag = "Invites",
    context_path = "/api/v2",
    responses(
        (status = 200, description = "Retrieve all invites", body = GetInvitesResponse),
        (status = 400, description = "Client error", body = ApiErrorResponse),
        (status = 500, description = "Server error", body = ApiErrorResponse),
    ),
    security(("session_cookie" = []))
)]
#[get("/invites")]
pub async fn get_invites(
    db: Data<Database>,
    session: Session,
) -> ApiResult<Json<GetInvitesResponse>> {
    let uuid: Uuid = session.get("uuid")?.ok_or(ApiError::SessionCorrupt)?;

    let invites = query!(
        db.as_ref(),
        (
            Invite::F.uuid,
            Invite::F.from.uuid,
            Invite::F.from.username,
            Invite::F.from.display_name,
            Invite::F.lobby.uuid,
            Invite::F.created_at
        )
    )
    .condition(Invite::F.to.equals(uuid.as_ref()))
    .all()
    .await?;

    Ok(Json(GetInvitesResponse {
        invites: invites
            .into_iter()
            .map(
                |(uuid, from_uuid, from_username, from_display_name, lobby_uuid, created_at)| {
                    GetInvite {
                        uuid,
                        lobby_uuid,
                        created_at: DateTime::from_utc(created_at, Utc),
                        from: AccountResponse {
                            uuid: from_uuid,
                            username: from_username,
                            display_name: from_display_name,
                        },
                    }
                },
            )
            .collect(),
    }))
}

/// Reject or retract an invite to a lobby
///
/// This endpoint can be used either by the sender of the invite to retract the invite or
/// by the receiver to reject the invite.
#[utoipa::path(
    tag = "Invites",
    context_path = "/api/v2",
    responses(
        (status = 200, description = "Invite was rejected"),
        (status = 400, description = "Client error", body = ApiErrorResponse),
        (status = 500, description = "Server error", body = ApiErrorResponse),
    ),
    params(PathUuid),
    security(("session_cookie" = []))
)]
#[delete("/invites/{uuid}")]
pub async fn delete_invite(
    path: Path<PathUuid>,
    db: Data<Database>,
    session: Session,
) -> ApiResult<HttpResponse> {
    let uuid: Uuid = session.get("uuid")?.ok_or(ApiError::SessionCorrupt)?;

    let mut tx = db.start_transaction().await?;

    let invite = query!(&mut tx, Invite)
        .condition(Invite::F.uuid.equals(path.uuid.as_ref()))
        .optional()
        .await?
        .ok_or(ApiError::InvalidUuid)?;

    // Check if the executing user has the privileges to delete the invite
    if *invite.to.key() != uuid && *invite.from.key() != uuid {
        return Err(ApiError::MissingPrivileges);
    }

    rorm::delete!(&mut tx, Invite).single(&invite).await?;

    tx.commit().await?;

    Ok(HttpResponse::Ok().finish())
}
