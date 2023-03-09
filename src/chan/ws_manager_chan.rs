use std::collections::HashMap;

use actix_toolbox::ws;
use actix_toolbox::ws::Message;
use log::error;
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use tokio::sync::mpsc::Sender;
use tokio::sync::{mpsc, oneshot};
use tokio::task;
use uuid::Uuid;

use crate::server::handler::ChatMessage;

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
    /// This variant is sent from the client that has finished its turn
    FinishedTurn {
        /// Identifier of the game
        game_id: u64,
        /// Data of the game
        game_data: Box<RawValue>,
    },
    /// An update of the game data.
    ///
    /// This variant is sent from the server to all accounts that are in the game.
    UpdateGameData {
        /// Identifier of the game
        game_id: u64,
        /// Data of the game
        game_data: Box<RawValue>,
        /// A unique counter that is incremented every time a [FinishedTurn] is received from
        /// the same `game_id`.
        ///
        /// This can be used by clients to check for updates on a long running game via API.
        game_data_id: u64,
    },
    /// Notification for clients if a client in their game disconnected
    ClientDisconnected {
        /// Identifier of the game
        game_id: u64,
        /// The identifier of the client that disconnected
        uuid: Uuid,
    },
    /// Notification for clients if a client in their game reconnected
    ClientReconnected {
        /// Identifier of the game
        game_id: u64,
        /// The identifier of the client that disconnected
        uuid: Uuid,
    },
    /// A new chat message is sent to the client.
    IncomingChatMessage {
        /// Identifier of the chat, the message originated from
        chat_id: u64,
        /// The new message
        message: ChatMessage,
    },
}

/// This type is a sender to the websocket manager
pub type WsManagerChan = Sender<WsManagerMessage>;

/// Messages to control the websocket manager
pub enum WsManagerMessage {
    /// Close the socket from the server side
    CloseSocket(Vec<u8>),
    /// Client with given uuid initialized a websocket
    OpenedSocket(Vec<u8>, ws::Sender),
    /// Send a message to given uuid
    SendMessage(Vec<u8>, WsMessage),
    /// Retrieve the current websocket count by sending this
    /// message to the ws manager.
    ///
    /// It will respond through the provided channel
    RetrieveWsCount(oneshot::Sender<u64>),
    /// Retrieve the online state of the requested accounts by sending this
    /// message to the ws manager
    ///
    /// It will respond through the provided channel.
    RetrieveOnlineState(Vec<Vec<u8>>, oneshot::Sender<Vec<bool>>),
}

/// Start the websocket manager
///
/// It will return a channel to this manager
pub async fn start_ws_manager() -> Result<WsManagerChan, String> {
    let mut lookup: HashMap<Vec<u8>, Vec<Sender<WsMessage>>> = HashMap::new();

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
                WsManagerMessage::RetrieveOnlineState(accounts, tx) => {
                    let online_state = accounts
                        .into_iter()
                        .map(|a| lookup.contains_key(&a))
                        .collect();

                    if tx.send(online_state).is_err() {
                        error!("Could not send through callback channel");
                    }
                }
            }
        }
    });

    Ok(tx)
}
