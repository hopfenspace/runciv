use rorm::{BackRef, ForeignModel, Model, Patch};

use crate::models::Account;

/// The lobby is the game state in which the game has not started yet.
///
/// If the game has started, the lobby should be deleted.
#[derive(Model, Debug)]
pub struct Lobby {
    /// Primary key of the lobby
    #[rorm(id)]
    pub id: i64,

    /// Name of the lobby
    #[rorm(max_length = 255)]
    pub name: String,

    /// Optional password of the lobby
    #[rorm(max_length = 255)]
    pub password_hash: Option<String>,

    /// The player that are currently in this lobby
    #[rorm(field = "LobbyAccount::F.lobby")]
    pub current_player: BackRef<LobbyAccount>,

    /// The maximum count of players
    pub max_player: i16,

    /// The point in time, the lobby was created
    #[rorm(auto_create_time)]
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Patch)]
#[rorm(model = "Lobby")]
pub(crate) struct LobbyInsert {
    pub(crate) name: String,
    pub(crate) password_hash: Option<String>,
    pub(crate) max_player: i16,
}

/// The m2m relation between lobby and accounts
#[derive(Model, Debug)]
pub struct LobbyAccount {
    /// Primary key of a lobby player
    #[rorm(id)]
    pub id: i64,

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
    pub lobby: ForeignModel<Lobby>,
    pub player: ForeignModel<Account>,
}
