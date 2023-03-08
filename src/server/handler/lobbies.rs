use actix_toolbox::tb_middleware::Session;
use actix_web::web::{Data, Json};
use actix_web::{get, post};
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHasher};
use chrono::{DateTime, Utc};
use rand::thread_rng;
use rorm::internal::field::foreign_model::ForeignModelByField;
use rorm::{insert, query, BackRef, Database, Model};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::models::{Account, Lobby, LobbyAccount, LobbyInsert};
use crate::server::handler::{AccountResponse, ApiError, ApiResult};

/// A single lobby
#[derive(Serialize, ToSchema)]
pub struct LobbyResponse {
    #[schema(example = "Herbert's lobby")]
    name: String,
    #[schema(example = 4)]
    max_players: u8,
    #[schema(example = 3)]
    current_players: u8,
    created_at: DateTime<Utc>,
    password: bool,
    owner: AccountResponse,
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
    let lobbies = query!(
        &db,
        (
            Lobby::F.id,
            Lobby::F.owner.f().uuid,
            Lobby::F.owner.f().username,
            Lobby::F.owner.f().display_name,
            Lobby::F.name,
            Lobby::F.created_at,
            Lobby::F.max_player,
            Lobby::F.password_hash,
        )
    )
    .all()
    .await?;

    let mut lobbies: Vec<Lobby> = lobbies
        .into_iter()
        .map(
            |(
                id,
                o_uuid,
                o_username,
                o_display_name,
                name,
                created_at,
                max_player,
                password_hash,
            )| Lobby {
                id,
                name,
                current_player: BackRef { cached: None },
                owner: ForeignModelByField::Instance(Box::new(Account {
                    uuid: o_uuid,
                    username: o_username,
                    display_name: o_display_name,
                    last_login: None,
                    password_hash: String::new(),
                })),
                created_at,
                max_player,
                password_hash,
            },
        )
        .collect();

    for lobby in &mut lobbies {
        Lobby::F.current_player.populate(&db, lobby).await?;
    }

    Ok(Json(GetLobbiesResponse {
        lobbies: lobbies
            .into_iter()
            .map(|l| {
                let Some(owner) = l.owner.instance() else {
                    unreachable!("Owner should be queried!")
                };
                LobbyResponse {
                    name: l.name,
                    owner: AccountResponse {
                        uuid: Uuid::from_slice(&owner.uuid).unwrap(),
                        username: owner.username.clone(),
                        display_name: owner.display_name.clone(),
                    },
                    current_players: l.current_player.cached.unwrap().len() as u8 + 1,
                    max_players: l.max_player as u8,
                    password: l.password_hash.is_some(),
                    created_at: DateTime::from_utc(l.created_at, Utc),
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
/// It contains the id of the created lobby
#[derive(Serialize, ToSchema)]
pub struct CreateLobbyResponse {
    lobby_id: u64,
}

/// Create a new lobby
///
/// If you are already in another lobby, an error is returned.
/// `max_players` must be between 2 and 34 (inclusive).
/// If `password` is an empty string, an error is returned.
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
    let uuid: Vec<u8> = session.get("uuid")?.ok_or(ApiError::SessionCorrupt)?;

    let mut tx = db.start_transaction().await?;

    // Check if the request is valid
    if req.max_players < 2 || req.max_players > 34 {
        return Err(ApiError::InvalidMaxPlayersCount);
    }

    // Check if the executing account is already in a lobby
    if query!(&db, (LobbyAccount::F.id,))
        .transaction(&mut tx)
        .condition(LobbyAccount::F.player.equals(&uuid))
        .optional()
        .await?
        .is_some()
    {
        return Err(ApiError::AlreadyInALobby);
    }

    if query!(&db, (Lobby::F.id,))
        .transaction(&mut tx)
        .condition(Lobby::F.owner.equals(&uuid))
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

    // Create lobby
    let id = insert!(&db, LobbyInsert)
        .transaction(&mut tx)
        .single(&LobbyInsert {
            name: req.name.clone(),
            password_hash: pw_hash,
            max_player: req.max_players as i16,
            owner: ForeignModelByField::Key(uuid),
        })
        .await?;

    tx.commit().await?;

    Ok(Json(CreateLobbyResponse {
        lobby_id: id as u64,
    }))
}
