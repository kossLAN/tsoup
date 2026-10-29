#![expect(
    clippy::same_name_method,
    reason = "the RustEmbed derive generates inherent methods named like its trait methods"
)]

use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse as _, Response};
use rust_embed::RustEmbed;

const FALLBACK: &str = "index.html";

#[derive(RustEmbed)]
#[folder = "$OUT_DIR/web"]
struct Assets;

pub async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    if path.starts_with("api/") {
        return StatusCode::NOT_FOUND.into_response();
    }

    let Some(file) = Assets::get(path).or_else(|| Assets::get(FALLBACK)) else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let content_type = file.metadata.mimetype().to_owned();
    ([(header::CONTENT_TYPE, content_type)], file.data).into_response()
}
