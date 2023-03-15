use actix_toolbox::tb_middleware::Session;
use actix_web::web::{Data, Json};
use actix_web::{post, HttpResponse};
use log::error;
use rorm::internal::field::foreign_model::ForeignModelByField;
use rorm::{and, insert, query, Database, Model};
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::chan::{WsManagerChan, WsManagerMessage, WsMessage};
use crate::models::{Account, Friend, InviteInsert, Lobby, LobbyAccount};
use crate::server::handler::{AccountResponse, ApiError, ApiResult};

/// The request to invite a friend into a lobby
#[derive(Deserialize, ToSchema)]
pub struct CreateInviteRequest {
    friend: Uuid,
    #[schema(example = 1337)]
    lobby_id: u64,
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
#[post("/invite")]
pub async fn create_invite(
    req: Json<CreateInviteRequest>,
    session: Session,
    db: Data<Database>,
    ws_manager_chan: Data<WsManagerChan>,
) -> ApiResult<HttpResponse> {
    let uuid: Vec<u8> = session.get("uuid")?.ok_or(ApiError::SessionCorrupt)?;

    let mut tx = db.start_transaction().await?;

    // Check if lobby is currently open
    let lobby = query!(&db, Lobby)
        .transaction(&mut tx)
        .condition(Lobby::F.id.equals(req.lobby_id as i64))
        .optional()
        .await?
        .ok_or(ApiError::InvalidLobbyId)?;

    let is_owner = match lobby.owner {
        ForeignModelByField::Key(k) => k == uuid,
        ForeignModelByField::Instance(account) => account.uuid == uuid,
    };

    // Check if the executing account has the privileges to invite to the specified lobby
    if !is_owner
        && query!(&db, LobbyAccount)
            .transaction(&mut tx)
            .condition(and!(
                LobbyAccount::F.lobby.equals(lobby.id),
                LobbyAccount::F.player.equals(&uuid)
            ))
            .optional()
            .await?
            .is_none()
    {
        return Err(ApiError::MissingPrivileges);
    }

    // Check if specified friend is valid
    let friend_account = query!(&db, Account)
        .transaction(&mut tx)
        .condition(Account::F.uuid.equals(req.friend.as_bytes()))
        .optional()
        .await?
        .ok_or(ApiError::InvalidUuid)?;

    // Check if there's a valid friendship
    let friend = query!(&db, Friend)
        .transaction(&mut tx)
        .condition(and!(
            Friend::F.is_request.equals(false),
            Friend::F.from.equals(&uuid),
            Friend::F.to.equals(&friend_account.uuid)
        ))
        .optional()
        .await?
        .ok_or(ApiError::InvalidFriendState)?;

    let invite_id = insert!(&db, InviteInsert)
        .transaction(&mut tx)
        .single(&InviteInsert {
            from: ForeignModelByField::Key(uuid.clone()),
            to: friend.to,
            lobby: ForeignModelByField::Key(lobby.id),
        })
        .await?;

    let executing_account = query!(&db, Account)
        .transaction(&mut tx)
        .condition(Account::F.uuid.equals(&uuid))
        .optional()
        .await?
        .ok_or(ApiError::SessionCorrupt)?;

    tx.commit().await?;

    let invite = WsMessage::IncomingInvite {
        invite_id: invite_id as u64,
        lobby_id: lobby.id as u64,
        from: AccountResponse {
            uuid: Uuid::from_slice(&executing_account.uuid).unwrap(),
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
