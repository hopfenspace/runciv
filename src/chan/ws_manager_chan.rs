use std::collections::HashMap;

use actix_toolbox::ws;
use actix_toolbox::ws::Message;
use log::error;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::Sender;
use tokio::sync::{mpsc, oneshot};
use tokio::task;
use uuid::Uuid;

use crate::server::handler::{AccountResponse, ChatMessage};

pub(crate) async fn start_ws_sender(tx: ws::Sender, mut rx: mpsc::Receiver<WsMessage>) {
    while let Some(msg) = rx.recv().await {
        match msg {
            WsMessage::ServerQuitSocket => {
                if let Err(err) = tx.close().await {
                    error!("Error while closing ws sender: {err}");
                }
                break;
            }
            _ => {
                let txt = match serde_json::to_string(&msg) {
                    Ok(v) => v,
                    Err(err) => {
                        error!("Error serializing WsMessage: {err}");
                        continue;
                    }
                };

                if let Err(err) = tx.send(Message::Text(txt.into())).await {
                    error!("Error sending to client: {err}, closing socket");
                    if let Err(err) = tx.close().await {
                        error!("Error closing socket: {err}");
                    }
                }
            }
        }
    }
}

/// Message that is sent via websocket
///
/// The messages will get serialized and deserialized using JSON
#[derive(Deserialize, Serialize, Clone)]
#[serde(tag = "type", content = "content", rename_all = "camelCase")]
pub enum WsMessage {
    /// This variant is only used internally to signal a socket handler that it should
    /// shutdown
    #[serde(skip)]
    ServerQuitSocket,
    /// Response to the client if an invalid message was received.
    ///
    /// This can occur, if the server can not deserialize the message, the message has a wrong
    /// type or a message, that should only be sent from the server, is received
    InvalidMessage,
    /// The notification for the clients that a new game has started
    GameStarted {
        /// Identifier of the game
        game_uuid: Uuid,
        /// Chatroom for the game
        game_chat_uuid: Uuid,
        /// The lobby the game originated from
        lobby_uuid: Uuid,
        /// The lobby chatroom the game chat room originated from
        lobby_chat_uuid: Uuid,
    },
    /// An update of the game data.
    ///
    /// This variant is sent from the server to all accounts that are in the game.
    UpdateGameData {
        /// Identifier of the game
        game_uuid: Uuid,
        /// Data of the game
        game_data: String,
        /// A unique counter identifying a game state, which is changed every time a
        /// [FinishedTurn] is received from the same `game_id`.
        ///
        /// This can be used by clients to check for updates on a long running game via API.
        game_data_id: u64,
    },
    /// Notification for clients if a client in their game disconnected
    ClientDisconnected {
        /// Identifier of the game
        game_uuid: Uuid,
        /// The identifier of the client that disconnected
        client_uuid: Uuid,
    },
    /// Notification for clients if a client in their game reconnected
    ClientReconnected {
        /// Identifier of the game
        game_uuid: Uuid,
        /// The identifier of the client that disconnected
        client_uuid: Uuid,
    },
    /// A new chat message is sent to the client.
    IncomingChatMessage {
        /// Identifier of the chat, the message originated from
        chat_uuid: Uuid,
        /// The new message
        message: ChatMessage,
    },
    /// An invite is sent to the client.
    IncomingInvite {
        /// The uuid of the invite
        invite_uuid: Uuid,
        /// The user that invoked the invite
        from: AccountResponse,
        /// The lobby to join
        lobby_uuid: Uuid,
    },
    /// A friend request is sent to the client
    IncomingFriendRequest {
        /// The user that invoked the request
        from: AccountResponse,
    },
    /// A new player joined the lobby
    LobbyJoin {
        /// The lobby that was joined
        lobby_uuid: Uuid,
        /// The player that joined in the lobby
        player: AccountResponse,
    },
    /// A lobby closed in which the client was part of
    LobbyClosed {
        /// The uuid of the lobby
        lobby_uuid: Uuid,
    },
    /// A player has left the lobby
    LobbyLeave {
        /// The lobby
        lobby_uuid: Uuid,
        /// The player that has left the lobby
        player: AccountResponse,
    },
    /// The user account was updated.
    ///
    /// This might me especially useful for reflecting changes in the username, etc. in the
    /// frontend
    AccountUpdated {
        /// The new account data
        account: AccountResponse,
    },
}

/// This type is a sender to the websocket manager
pub type WsManagerChan = Sender<WsManagerMessage>;

/// Messages to control the websocket manager
pub enum WsManagerMessage {
    /// Close the socket from the server side
    CloseSocket(Uuid),
    /// Client with given uuid initialized a websocket
    OpenedSocket(Uuid, ws::Sender),
    /// Send a message to given uuid
    SendMessage(Uuid, WsMessage),
    /// Retrieve the current websocket count by sending this
    /// message to the ws manager.
    ///
    /// It will respond through the provided channel
    RetrieveWsCount(oneshot::Sender<u64>),
    /// Retrieve the online state of the requested accounts by sending this
    /// message to the ws manager
    ///
    /// It will respond through the provided channel.
    RetrieveOnlineStates(Vec<Uuid>, oneshot::Sender<Vec<bool>>),
    /// Retrieve the online state of the single account by sending this
    /// message to the ws manager
    ///
    /// It will respond through the provided channel.
    RetrieveOnlineState(Uuid, oneshot::Sender<bool>),
}

/// Start the websocket manager
///
/// It will return a channel to this manager
pub async fn start_ws_manager() -> Result<WsManagerChan, String> {
    let mut lookup: HashMap<Uuid, Vec<Sender<WsMessage>>> = HashMap::new();

    let (tx, mut rx) = mpsc::channel(16);

    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            match msg {
                WsManagerMessage::CloseSocket(uuid) => {
                    // Trigger close for all websockets associated with uuid
                    if let Some(sockets) = lookup.get(&uuid) {
                        for s in sockets {
                            if !s.is_closed() {
                                if let Err(err) = s.send(WsMessage::ServerQuitSocket).await {
                                    error!("Couldn't send close to ws sender: {err}");
                                }
                            }
                        }
                    }

                    lookup.remove(&uuid);
                }
                WsManagerMessage::OpenedSocket(uuid, ws_tx) => {
                    let (tx, rx) = mpsc::channel(16);
                    task::spawn(start_ws_sender(ws_tx, rx));

                    // Add new client connection to state
                    if let Some(sockets) = lookup.get_mut(&uuid) {
                        sockets.push(tx);
                    }
                    // Insert new client connection
                    else {
                        lookup.insert(uuid, vec![tx]);
                    }
                }
                WsManagerMessage::SendMessage(uuid, msg) => {
                    if let Some(sender) = lookup.get(&uuid) {
                        for tx in sender {
                            if let Err(err) = tx.send(msg.clone()).await {
                                error!("Could not send to ws sender: {err}");
                            }
                        }
                    }
                }
                WsManagerMessage::RetrieveWsCount(tx) => {
                    let sum = lookup.values().map(|s| s.len() as u64).sum();
                    if tx.send(sum).is_err() {
                        error!("Could not send through callback channel");
                    }
                }
                WsManagerMessage::RetrieveOnlineStates(accounts, tx) => {
                    let online_state = accounts
                        .into_iter()
                        .map(|a| lookup.contains_key(&a))
                        .collect();

                    if tx.send(online_state).is_err() {
                        error!("Could not send through callback channel");
                    }
                }
                WsManagerMessage::RetrieveOnlineState(account, tx) => {
                    if tx.send(lookup.contains_key(&account)).is_err() {
                        error!("Could not send through callback channel");
                    }
                }
            }
        }
    });

    Ok(tx)
}
