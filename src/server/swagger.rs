//! This module holds the definition of the swagger declaration

use utoipa::openapi::security::{ApiKey, ApiKeyValue, HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{Modify, OpenApi};

use crate::server::handler;

struct CookieSecurity;

impl Modify for CookieSecurity {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "session_cookie",
                SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::new("id"))),
            )
        }
    }
}

/// Helper struct for the openapi definitions.
#[derive(OpenApi)]
#[openapi(
    paths(
        handler::register_account,
        handler::get_me,
        handler::delete_me,
        handler::update_me,
        handler::set_password,
        handler::login,
        handler::logout,
        handler::websocket,
        handler::version,
        handler::create_friend_request,
        handler::accept_friend_request,
        handler::get_friends,
        handler::delete_friend,
        handler::get_lobbies,
        handler::create_lobby,
        handler::lookup_account_by_uuid,
        handler::lookup_account_by_username,
        handler::get_chat,
        handler::get_all_chats,
        handler::create_invite,
        handler::get_invites,
        handler::get_open_games,
        handler::get_game,
        handler::push_game_update,
        handler::start_game,
        handler::send_message,
        handler::join_lobby,
        handler::delete_invite,
        handler::close_lobby,
        handler::leave_lobby,
    ),
    components(schemas(
        handler::AccountRegistrationRequest,
        handler::ApiErrorResponse,
        handler::ApiStatusCode,
        handler::LoginRequest,
        handler::AccountResponse,
        handler::SetPasswordRequest,
        handler::UpdateAccountRequest,
        handler::VersionResponse,
        handler::CreateFriendRequest,
        handler::GetFriendResponse,
        handler::FriendResponse,
        handler::LobbyResponse,
        handler::GetLobbiesResponse,
        handler::CreateLobbyResponse,
        handler::CreateLobbyRequest,
        handler::OnlineAccountResponse,
        handler::FriendRequestResponse,
        handler::LookupAccountUsernameRequest,
        handler::GetChatResponse,
        handler::ChatMessage,
        handler::ChatMember,
        handler::GetAllChatsResponse,
        handler::CreateInviteRequest,
        handler::GetInvitesResponse,
        handler::GetInvite,
        handler::GameStateResponse,
        handler::GameOverviewResponse,
        handler::GetGameOverviewResponse,
        handler::GameUploadResponse,
        handler::GameUploadRequest,
        handler::StartGameResponse,
        handler::SendMessageRequest,
        handler::JoinLobbyRequest,
    )),
    modifiers(&CookieSecurity)
)]
pub struct ApiDoc;

struct TokenSecurity;

impl Modify for TokenSecurity {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "admin_token",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .description(Some(
                            "The token is set in the configuration file in the server.",
                        ))
                        .build(),
                ),
            )
        }
    }
}

/// Helper struct for the admin openapi definitions.
#[derive(OpenApi)]
#[openapi(
    paths(
        handler::health,
    ),
    components(schemas(
        handler::ApiErrorResponse,
        handler::ApiStatusCode,
        handler::HealthResponse,
    )),
    modifiers(&TokenSecurity)
)]
pub struct AdminApiDoc;
