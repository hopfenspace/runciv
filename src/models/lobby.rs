use rorm::fields::types::{BackRef, ForeignModel};
use rorm::{field, Model, Patch};
use uuid::Uuid;

use crate::models::{Account, ChatRoom};

/// The lobby is the game state in which the game has not started yet.
///
/// If the game has started, the lobby should be deleted.
#[derive(Model)]
pub struct Lobby {
    /// Primary key of the lobby
    #[rorm(primary_key)]
    pub uuid: Uuid,

    /// Name of the lobby
    #[rorm(max_length = 255)]
    pub name: String,

    /// The owner of this lobby
    #[rorm(on_delete = "Cascade", on_update = "Cascade")]
    pub owner: ForeignModel<Account>,

    /// Optional password of the lobby
    #[rorm(max_length = 255)]
    pub password_hash: Option<String>,

    /// The player that are currently in this lobby
    pub current_player: BackRef<field!(LobbyAccount::F.lobby)>,

    /// The maximum count of players
    pub max_player: i16,

    /// The chatroom of the lobby
    #[rorm(on_update = "Cascade", on_delete = "Cascade")]
    pub chat_room: ForeignModel<ChatRoom>,

    /// The point in time, the lobby was created
    #[rorm(auto_create_time)]
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Patch)]
#[rorm(model = "Lobby")]
pub(crate) struct LobbyInsert {
    pub(crate) uuid: Uuid,
    pub(crate) name: String,
    pub(crate) owner: ForeignModel<Account>,
    pub(crate) password_hash: Option<String>,
    pub(crate) chat_room: ForeignModel<ChatRoom>,
    pub(crate) max_player: i16,
}

/// The m2m relation between lobby and accounts
#[derive(Model)]
pub struct LobbyAccount {
    /// Primary key of a lobby player
    #[rorm(primary_key)]
    pub uuid: Uuid,

    /// The lobby
    #[rorm(on_delete = "Cascade", on_update = "Cascade")]
    pub lobby: ForeignModel<Lobby>,

    /// The account in the lobby
    #[rorm(on_delete = "Cascade", on_update = "Cascade")]
    pub player: ForeignModel<Account>,
}

#[derive(Patch)]
#[rorm(model = "LobbyAccount")]
pub(crate) struct LobbyAccountInsert {
    pub(crate) uuid: Uuid,
    pub(crate) lobby: ForeignModel<Lobby>,
    pub(crate) player: ForeignModel<Account>,
}
