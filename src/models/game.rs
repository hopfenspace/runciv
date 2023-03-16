use rorm::fields::{BackRef, ForeignModel};
use rorm::{field, Model, Patch};

use crate::models::{Account, ChatRoom};

/// A game identified by its ID
///
/// The game data itself should be stored in a file on disk,
/// use `id` and `data_id` to create a filename to store it.
#[derive(Model)]
pub struct Game {
    /// Primary key of the game
    #[rorm(id)]
    pub id: i64,

    /// Unique identifier of the state of the data
    #[rorm(default = 0)]
    pub data_id: i64,

    /// Name of the game
    #[rorm(max_length = 255)]
    pub name: String,

    /// The users that are currently playing this game
    pub current_players: BackRef<field!(GameAccount::F.game)>,

    /// The maximum count of players
    pub max_players: i16,

    /// The point in time, the game was updated
    #[rorm(auto_create_time, auto_update_time)]
    pub updated_at: chrono::NaiveDateTime,

    /// The player who uploaded the most recent game state
    #[rorm(on_delete = "Cascade", on_update = "Cascade")]
    pub updated_by: ForeignModel<Account>,

    /// The chatroom of the game
    #[rorm(on_update = "Cascade", on_delete = "Cascade")]
    pub chat_room: ForeignModel<ChatRoom>,
}

#[derive(Patch)]
#[rorm(model = "Game")]
pub(crate) struct GameInsert {
    pub(crate) name: String,
    pub(crate) max_players: i16,
    pub(crate) updated_by: ForeignModel<Account>,
    pub(crate) chat_room: ForeignModel<ChatRoom>,
}

/// The m2m relation between games and accounts
#[derive(Model)]
pub struct GameAccount {
    /// Primary key of a game account
    #[rorm(id)]
    pub id: i64,

    /// The game
    #[rorm(on_delete = "Cascade", on_update = "Cascade")]
    pub game: ForeignModel<Game>,

    /// The player account in the game
    #[rorm(on_delete = "Cascade", on_update = "Cascade")]
    pub player: ForeignModel<Account>,
}

#[derive(Patch)]
#[rorm(model = "GameAccount")]
pub(crate) struct GameAccountInsert {
    pub game: ForeignModel<Game>,
    pub player: ForeignModel<Account>,
}