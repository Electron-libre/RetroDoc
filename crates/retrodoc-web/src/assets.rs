//! The scripts and stylesheet, vendored under `assets/` and embedded in the
//! binary: no CDN, so the interface works offline and leaks nothing.

use axum::extract::Path;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};

/// `htmx` 2.0.11, from `https://unpkg.com/htmx.org@2.0.11/dist/htmx.min.js`.
const HTMX: &str = include_str!("../assets/htmx.min.js");
const STYLE: &str = include_str!("../assets/style.css");

pub async fn serve(Path(name): Path<String>) -> Response {
    let (content_type, body) = match name.as_str() {
        "htmx.min.js" => ("text/javascript; charset=utf-8", HTMX),
        "style.css" => ("text/css; charset=utf-8", STYLE),
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    // Embedded in the binary: they change only with it.
    (
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "public, max-age=3600"),
        ],
        body,
    )
        .into_response()
}
