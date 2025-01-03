//! Websocket handler

use std::sync::Arc;
use std::time::{Duration, Instant};

use actix_toolbox::tb_middleware::Session;
use actix_toolbox::ws;
use actix_toolbox::ws::{MailboxError, Message};
use actix_web::web::{Data, Payload};
use actix_web::{get, HttpRequest, HttpResponse};
use bytes::Bytes;
use bytestring::ByteString;
use log::{debug, error, warn};
use once_cell::sync::Lazy;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::chan::{WsManagerChan, WsManagerMessage, WsMessage};
use crate::invalid_msg;
use crate::server::handler::{ApiError, ApiErrorResponse};

struct CommonMessages {
    invalid_message: ByteString,
}

static COMMON: Lazy<CommonMessages> = Lazy::new(|| CommonMessages {
    // Fine as we can't do anything here, if [WsMessage] does not want to serialize anymore
    #[allow(clippy::unwrap_used)]
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
    let hb_ws_manager = ws_manager_chan.clone();
    let hb_uuid = uuid;
    tokio::spawn(async move {
        loop {
            if Instant::now().duration_since(*hb_time.lock().await) > CLIENT_TIMEOUT
                && hb_tx.close().await.is_ok()
            {
                debug!("Closed websocket due to missing heartbeat responses");
            }

            tokio::time::sleep(Duration::from_secs(10)).await;

            if let Err(err) = hb_tx.send(Message::Ping(Bytes::from(""))).await {
                if let MailboxError::Closed = err {
                    debug!("Could not send ping to ws: ws closed");
                    if let Err(err) = hb_ws_manager
                        .send(WsManagerMessage::WebsocketClosed(hb_uuid))
                        .await
                    {
                        warn!("Could not send to ws_manager_chan: {err}");
                    }
                    break;
                }
                debug!("Sending to ran into tx timeout");
            };
        }
    });

    let rx_tx = tx.clone();
    let rx_ws_manager = ws_manager_chan.clone();
    let rx_uuid = uuid;
    tokio::spawn(async move {
        while let Some(res) = rx.recv().await {
            match res {
                Ok(msg) => match msg {
                    Message::Ping(req) => {
                        if let Err(err) = rx_tx.send(Message::Pong(req)).await {
                            if let MailboxError::Closed = err {
                                debug!("Could not pong send to ws: websocket closed");
                                break;
                            }
                            debug!("Sending to ran into tx timeout");
                        }
                    }
                    Message::Pong(_) => {
                        let mut r = last_hb.lock().await;
                        *r = Instant::now();
                    }
                    Message::Close(_) => {
                        debug!("Client closed websocket");
                        break;
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

        debug!("Websocket closed");
        if let Err(err) = rx_ws_manager
            .send(WsManagerMessage::WebsocketClosed(rx_uuid))
            .await
        {
            warn!("Could not send to ws_manager_chan: {err}");
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
