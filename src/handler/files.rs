use actix_web::web::{BytesMut, Path, Payload};
use actix_web::{error, get, put, HttpResponse};
use futures_util::stream::StreamExt;
use serde::Deserialize;

use crate::server::FileData;

#[derive(Deserialize, Debug)]
pub(crate) struct FileRequest {
    pub(crate) filename: String,
}

#[put("/files/{filename}")]
pub(crate) async fn put_file(
    path: Path<FileRequest>,
    file_data: FileData,
    mut payload: Payload,
) -> actix_web::Result<HttpResponse> {
    let mut body = BytesMut::new();
    while let Some(chunk) = payload.next().await {
        let chunk = chunk?;
        // limit max size of in-memory payload
        if (body.len() + chunk.len()) > 5_000_000 {
            return Err(error::ErrorBadRequest("overflow"));
        }
        body.extend_from_slice(&chunk);
    }

    file_data
        .lock()
        .await
        .insert(path.filename.clone(), body.to_vec());

    Ok(HttpResponse::Ok().finish())
}

#[get("/files/{filename}")]
pub(crate) async fn get_file(path: Path<FileRequest>, file_data: FileData) -> HttpResponse {
    if let Some(content) = file_data.lock().await.get(&path.filename) {
        HttpResponse::Ok().body(content.clone())
    } else {
        HttpResponse::NotFound().finish()
    }
}
