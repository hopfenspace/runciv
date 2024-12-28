use rorm::fields::types::ForeignModel;
use rorm::{Model, Patch};
use uuid::Uuid;

use crate::models::{Account, Lobby};

/// Representation of an invite to a lobby.
///
/// With an invitation you don't to enter the password set to the lobby.
#[derive(Model)]
pub struct Invite {
    /// The primary key of an invite
    #[rorm(primary_key)]
    pub uuid: Uuid,

    /// The user that has invoked the invite
    #[rorm(on_delete = "Cascade", on_update = "Cascade")]
    pub from: ForeignModel<Account>,

    /// The invitee
    #[rorm(on_delete = "Cascade", on_update = "Cascade")]
    pub to: ForeignModel<Account>,

    /// The lobby
    #[rorm(on_delete = "Cascade", on_update = "Cascade")]
    pub lobby: ForeignModel<Lobby>,

    /// The point in time the invite was created
    #[rorm(auto_create_time)]
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Patch)]
#[rorm(model = "Invite")]
pub(crate) struct InviteInsert {
    pub(crate) uuid: Uuid,
    pub(crate) from: ForeignModel<Account>,
    pub(crate) to: ForeignModel<Account>,
    pub(crate) lobby: ForeignModel<Lobby>,
}
