use rorm::{ForeignModel, Model, Patch};

use crate::models::Account;

/// The representation of friends
///
/// This model has to be created 2 times for every relation.
#[derive(Model, Debug)]
pub struct Friend {
    /// Primary key of this friend pair
    #[rorm(id)]
    pub id: i64,

    /// This field is true, if the friendship is not confirmed yet.
    pub is_request: bool,

    /// The originating user
    #[rorm(on_update = "Cascade", on_delete = "Cascade")]
    pub from: ForeignModel<Account>,

    /// The other user
    #[rorm(on_update = "Cascade", on_delete = "Cascade")]
    pub to: ForeignModel<Account>,
}

#[derive(Patch)]
#[rorm(model = "Friend")]
pub(crate) struct FriendInsert {
    pub from: ForeignModel<Account>,
    pub to: ForeignModel<Account>,
}
