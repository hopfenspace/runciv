use rorm::fields::types::BackRef;
use rorm::{field, Model, Patch};
use uuid::Uuid;

use crate::models::ChatRoomMember;

/// A user account
#[derive(Model)]
pub struct Account {
    /// The primary key of a user.
    ///
    /// This will be a uuid.
    #[rorm(primary_key)]
    pub uuid: Uuid,

    /// The username of the client
    #[rorm(max_length = 255, unique)]
    pub username: String,

    /// The name that is displayed for this user
    #[rorm(max_length = 255)]
    pub display_name: String,

    /// The password hash of the user.
    #[rorm(max_length = 1024)]
    pub password_hash: String,

    /// The last time the user has logged in
    pub last_login: Option<chrono::NaiveDateTime>,

    /// The chat rooms this account is part of
    pub chat_rooms: BackRef<field!(ChatRoomMember::F.member)>,
}

#[derive(Patch)]
#[rorm(model = "Account")]
pub(crate) struct AccountInsert {
    pub(crate) uuid: Uuid,
    pub(crate) username: String,
    pub(crate) display_name: String,
    pub(crate) password_hash: String,
    pub(crate) last_login: Option<chrono::NaiveDateTime>,
}
