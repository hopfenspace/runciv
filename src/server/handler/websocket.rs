use std::sync::Arc;
use std::time::{Duration, Instant};

use actix_toolbox::tb_middleware::Session;
use actix_toolbox::ws;
use actix_toolbox::ws::{MailboxError, Message};
use actix_web::web::{Data, Payload};
use actix_web::{get, HttpRequest, HttpResponse};
use bytes::Bytes;
use log::{debug, error};
use tokio::sync::Mutex;

use crate::chan::WsManagerMessage::Message;
use crate::chan::{WsManagerChan, WsManagerMessage};
use crate::server::handler::ApiError;

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
    let uuid: Vec<u8> = session.get("uuid")?.ok_or(ApiError::SessionCorrupt)?;

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

            if let Err(err) = hb_tx.send(Message::Ping(Bytes::from(""))).await {
                match err {
                    MailboxError::Closed => {
                        debug!("Websocket was closed by another tx instance")
                    }
                    MailboxError::Timeout => {
                        debug!("Got timeout sending to client, trying to close socket");
                        if hb_tx.close().await.is_err() {
                            debug!("Error closing socket")
                        }
                    }
                }
                break;
            }
        }
    });

    let rx_tx = tx.clone();
    tokio::spawn(async move {
        while let Some(res) = rx.recv().await {
            match res {
                Ok(msg) => match msg {
                    Message::Pong(_) => {
                        let mut r = last_hb.lock().await;
                        *r = Instant::now();
                    }
                    Message::Ping(req) => {
                        if let Err(err) = rx_tx.send(Message::Pong(req)).await {
                            error!("Could not send to tx: {err}");
                            if let MailboxError::Closed = err {
                                debug!("Websocket closed");
                                break;
                            }
                        }
                    }
                    _ => {
                        debug!("msg: {msg:?}");
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
            error!("Couldn't close websocket: {err}");
        }
    }

    Ok(response)
}
