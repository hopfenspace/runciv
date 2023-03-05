use actix_toolbox::ws;
use actix_toolbox::ws::Message;
use actix_web::web::Payload;
use actix_web::{get, HttpRequest, HttpResponse};
use bytes::Bytes;

#[get("/ws")]
pub(crate) async fn start_ws(request: HttpRequest, payload: Payload) -> HttpResponse {
    let (tx, mut rx, response) = ws::start(&request, payload).unwrap();

    let tx_heartbeat = tx.clone();
    tokio::spawn(async move {
        tx_heartbeat
            .send(Message::Ping(Bytes::from("")))
            .await
            .unwrap()
    });

    tokio::spawn(async move {
        while let Some(Ok(res)) = rx.recv().await {
            if let Message::Text(str) = res {
                tx.send(Message::Text(str)).await.unwrap()
            }
        }
    });

    response
}
