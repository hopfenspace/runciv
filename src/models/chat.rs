use rorm::fields::types::{BackRef, ForeignModel};
use rorm::{field, Model, Patch};
use uuid::Uuid;

use crate::models::Account;

/// This represents a chatroom in the database
#[derive(Model)]
pub struct ChatRoom {
    /// The primary key of a chat
    #[rorm(primary_key)]
    pub uuid: Uuid,

    /// A backref to the members of a specific chatroom
    pub members: BackRef<field!(ChatRoomMember::F.chat_room)>,

    /// A backref to the members of a specific chatroom
    pub messages: BackRef<field!(ChatRoomMessage::F.chat_room)>,

    /// The uuid of the most recent message
    pub last_message_uuid: Option<Uuid>,
}

#[derive(Patch)]
#[rorm(model = "ChatRoom")]
pub(crate) struct ChatRoomInsert {
    pub(crate) uuid: Uuid,
    pub(crate) last_message_uuid: Option<Uuid>,
}

/// The member <-> chatroom relation
#[derive(Model)]
pub struct ChatRoomMember {
    /// The primary key of a chatroom
    #[rorm(primary_key)]
    pub uuid: Uuid,

    /// The relation to a chatroom
    #[rorm(on_delete = "Cascade", on_update = "Cascade")]
    pub chat_room: ForeignModel<ChatRoom>,

    /// The relation to the member
    #[rorm(on_delete = "Cascade", on_update = "Cascade")]
    pub member: ForeignModel<Account>,

    /// The creation time of the member in a chat aka:
    /// When has the account joined the chat
    #[rorm(auto_create_time)]
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Patch)]
#[rorm(model = "ChatRoomMember")]
pub(crate) struct ChatRoomMemberInsert {
    pub(crate) uuid: Uuid,
    pub(crate) chat_room: ForeignModel<ChatRoom>,
    pub(crate) member: ForeignModel<Account>,
}

/// A message of a chatroom
#[derive(Model)]
pub struct ChatRoomMessage {
    /// The primary key of a chatroom message
    #[rorm(primary_key)]
    pub uuid: Uuid,

    /// The account that send the message
    #[rorm(on_delete = "Cascade", on_update = "Cascade")]
    pub sender: ForeignModel<Account>,

    /// The relation to the chat room
    #[rorm(on_delete = "Cascade", on_update = "Cascade")]
    pub chat_room: ForeignModel<ChatRoom>,

    /// The maximum length of a message
    #[rorm(max_length = 2048)]
    pub message: String,

    /// The timestamp when the message was received
    #[rorm(auto_create_time)]
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Patch)]
#[rorm(model = "ChatRoomMessage")]
pub(crate) struct ChatRoomMessageInsert {
    pub(crate) uuid: Uuid,
    pub(crate) chat_room: ForeignModel<ChatRoom>,
    pub(crate) sender: ForeignModel<Account>,
    pub(crate) message: String,
}
