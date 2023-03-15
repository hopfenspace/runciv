use std::sync::Arc;
use std::time::{Duration, Instant};

use actix_toolbox::tb_middleware::Session;
use actix_toolbox::ws;
use actix_toolbox::ws::{MailboxError, Message};
use actix_web::web::{Data, Payload};
use actix_web::{get, HttpRequest, HttpResponse};
use bytes::Bytes;
use bytestring::ByteString;
use log::{debug, error};
use once_cell::sync::Lazy;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::chan::{WsManagerChan, WsManagerMessage, WsMessage};
use crate::server::handler::ApiError;
use crate::{invalid_msg, send_to_ws};

struct CommonMessages {
    invalid_message: ByteString,
}

static COMMON: Lazy<CommonMessages> = Lazy::new(|| CommonMessages {
    invalid_message: ByteString::from(serde_json::to_string(&WsMessage::InvalidMessage).unwrap()),
});

const CLIENT_TIMEOUT: Duration = Duration::from_secs(30);

/// Start a websocket connection
///
/// A heartbeat PING packet is sent constantly (every 10s).
/// If no response is retrieved within 30s of the last transmission, the socket
/// will be closed.
#[utoipa::path(
    tag = "Websocket",
    context_path = "/api/v2",
    responses(
        (status = 101, description = "Websocket is initialized"),
        (status = 400, description = "Client error", body = ApiErrorResponse),
        (status = 500, description = "Server error", body = ApiErrorResponse),
    ),
    security(("session_cookie" = []))
)]
#[get("/ws")]
pub async fn websocket(
    req: HttpRequest,
    payload: Payload,
    session: Session,
    ws_manager_chan: Data<WsManagerChan>,
) -> actix_web::Result<HttpResponse> {
    let uuid: Uuid = session.get("uuid")?.ok_or(ApiError::SessionCorrupt)?;

    let (tx, mut rx, response) = ws::start(&req, payload)?;

    debug!("Initializing websocket connection");
    let last_hb = Arc::new(Mutex::new(Instant::now()));

    // Heartbeat task
    let hb_tx = tx.clone();
    let hb_time = last_hb.clone();
    tokio::spawn(async move {
        loop {
            if Instant::now().duration_since(*hb_time.lock().await) > CLIENT_TIMEOUT
                && hb_tx.close().await.is_ok()
            {
                debug!("Closed websocket due to missing heartbeat responses");
            }

            tokio::time::sleep(Duration::from_secs(10)).await;
            send_to_ws!(hb_tx, Message::Ping(Bytes::from("")));
        }
    });

    let rx_tx = tx.clone();
    tokio::spawn(async move {
        while let Some(res) = rx.recv().await {
            match res {
                Ok(msg) => match msg {
                    Message::Text(data) => {
                        let message: WsMessage = match serde_json::from_str(&String::from(data)) {
                            Ok(v) => v,
                            Err(err) => {
                                debug!("Could not deserialize message: {err}");
                                invalid_msg!(rx_tx);
                                continue;
                            }
                        };

                        match message {
                            WsMessage::FinishedTurn { game_id, game_data } => {
                                debug!("Received Finished turn: {game_id}");
                            }
                            _ => invalid_msg!(rx_tx),
                        }
                    }
                    Message::Ping(req) => send_to_ws!(rx_tx, Message::Pong(req)),
                    Message::Pong(_) => {
                        let mut r = last_hb.lock().await;
                        *r = Instant::now();
                    }
                    _ => {
                        invalid_msg!(rx_tx);
                        debug!("Received invalid message type via websocket");
                    }
                },
                Err(err) => {
                    debug!("Protocol error: {err}");
                }
            }
        }
    });

    // Give sender to ws manager
    if let Err(err) = ws_manager_chan
        .send(WsManagerMessage::OpenedSocket(uuid, tx.clone()))
        .await
    {
        error!("Could not send ws tx to ws manager: {err}. Closing websocket");
        if let Err(err) = tx.close().await {
            if let MailboxError::Closed = err {
                debug!("Websocket closed");
            }
            debug!("Sending to ran into tx timeout");
        }
    }

    Ok(response)
}

/// This is a helper macro to send a INVALID_MESSAGE to the websocket via tx
#[macro_export]
macro_rules! invalid_msg {
    ($tx:expr) => {
        if let Err(err) = $tx
            .send(Message::Text(COMMON.invalid_message.clone()))
            .await
        {
            if let MailboxError::Closed = err {
                debug!("Websocket closed");
                break;
            }
            debug!("Sending to ran into tx timeout");
        }
    };
}

/// Use this macro to send arbitrary messages to the websocket
///
/// First argument is the websocket
/// Second argument the Message to send.
#[macro_export]
macro_rules! send_to_ws {
    ($tx:expr, $msg:expr) => {
        if let Err(err) = $tx.send($msg).await {
            if let MailboxError::Closed = err {
                debug!("Websocket closed");
                break;
            }
            debug!("Sending to ran into tx timeout");
        }
    };
}
