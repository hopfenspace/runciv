use rorm::{BackRef, ForeignModel, Model, Patch};

use crate::models::Account;

/// This represents a chatroom in the database
#[derive(Model)]
pub struct ChatRoom {
    /// The primary key of a chat
    #[rorm(id)]
    pub id: i64,

    /// A backref to the members of a specific chatroom
    #[rorm(field = "ChatRoomMember::F.chat_room")]
    pub members: BackRef<ChatRoomMember>,

    /// A backref to the members of a specific chatroom
    #[rorm(field = "ChatRoomMessage::F.chat_room")]
    pub messages: BackRef<ChatRoomMessage>,
}

#[derive(Patch)]
#[rorm(model = "ChatRoom")]
pub(crate) struct ChatRoomInsert {}

/// The member <-> chatroom relation
#[derive(Model)]
pub struct ChatRoomMember {
    /// The primary key of a chatroom
    #[rorm(id)]
    pub id: i64,

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
    pub chat_room: ForeignModel<ChatRoom>,
    pub member: ForeignModel<Account>,
}

/// A message of a chatroom
#[derive(Model)]
pub struct ChatRoomMessage {
    /// The primary key of a chatroom message
    #[rorm(id)]
    pub id: i64,

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
    pub chat_room: ForeignModel<ChatRoom>,
    pub message: String,
}
