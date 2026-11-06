//! The scripts, stylesheet and logo, vendored under `assets/` and embedded in the
//! binary: no CDN, so the interface works offline and leaks nothing.

use axum::extract::Path;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};

/// `htmx` 2.0.11, from `https://unpkg.com/htmx.org@2.0.11/dist/htmx.min.js`.
const HTMX: &str = include_str!("../assets/htmx.min.js");
/// The SSE extension of `htmx`, 2.2.4, `dist/sse.min.js` of the npm package `htmx-ext-sse`: the
/// running exchange of the ask page follows its question with it (ADR 0049).
const HTMX_SSE: &str = include_str!("../assets/htmx-ext-sse.min.js");
/// `mermaid` 12.2.0, `dist/mermaid.min.js` of the npm package, gzipped
/// (`gzip -9 -n`): 5.5 MB plain. Its name carries the version, so a browser
/// keeps it until the version changes.
const MERMAID_GZ: &[u8] = include_bytes!("../assets/mermaid-12.2.0.min.js.gz");
/// The name `mermaid` is served under, loaded only by the pages with a diagram.
pub const MERMAID: &str = "mermaid-12.2.0.min.js";
const DIAGRAMS: &str = include_str!("../assets/diagrams.js");
/// The answers of the forms sent with `htmx`: an HTML error swapped, any other in a notice.
const FORMS: &str = include_str!("../assets/forms.js");
/// The search box as a combobox over its hits panel: `/`, the arrows, Enter, Escape.
const SEARCH: &str = include_str!("../assets/search.js");
/// The composer of the ask page: the keys, the growth of the field, the focus after an answer.
const ASK: &str = include_str!("../assets/ask.js");
const STYLE: &str = include_str!("../assets/style.css");
/// The logo mark of Meta Lambda, from `static/assets/img/logo-mark.svg` of the
/// meta-lambda.com site, shown by the credit at the foot of every page.
const LOGO: &str = include_str!("../assets/logo-mark.svg");

const JAVASCRIPT: &str = "text/javascript; charset=utf-8";

pub async fn serve(Path(name): Path<String>, headers: HeaderMap) -> Response {
    if name == MERMAID {
        // Every browser accepts gzip; the plain script would weigh 5.5 MB more
        // in the binary, so a client that doesn't is refused.
        if !accepts_gzip(&headers) {
            return (
                StatusCode::NOT_ACCEPTABLE,
                "this script is only served gzip-compressed",
            )
                .into_response();
        }
        return (
            [
                (header::CONTENT_TYPE, JAVASCRIPT),
                (header::CONTENT_ENCODING, "gzip"),
                (header::VARY, "accept-encoding"),
                (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
            ],
            MERMAID_GZ,
        )
            .into_response();
    }
    let (content_type, body) = match name.as_str() {
        "htmx.min.js" => (JAVASCRIPT, HTMX),
        "htmx-ext-sse.min.js" => (JAVASCRIPT, HTMX_SSE),
        "diagrams.js" => (JAVASCRIPT, DIAGRAMS),
        "forms.js" => (JAVASCRIPT, FORMS),
        "ask.js" => (JAVASCRIPT, ASK),
        "search.js" => (JAVASCRIPT, SEARCH),
        "style.css" => ("text/css; charset=utf-8", STYLE),
        "logo-mark.svg" => ("image/svg+xml", LOGO),
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

/// Whether `Accept-Encoding` accepts gzip: named with a weight above 0, or,
/// when not named, `*` with a weight above 0 (RFC 9110: a named coding
/// overrides `*`).
fn accepts_gzip(headers: &HeaderMap) -> bool {
    let codings: Vec<(String, bool)> = headers
        .get_all(header::ACCEPT_ENCODING)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .map(|coding| {
            let mut parts = coding.split(';').map(str::trim);
            let name = parts.next().unwrap_or_default().to_ascii_lowercase();
            let refused = parts.any(|p| {
                p.strip_prefix("q=")
                    .and_then(|q| q.parse::<f32>().ok())
                    .is_some_and(|q| q <= 0.0)
            });
            (name, !refused)
        })
        .collect();
    let accepted = |name: &str| codings.iter().find(|(n, _)| n == name).map(|(_, ok)| *ok);
    accepted("gzip").or_else(|| accepted("*")).unwrap_or(false)
}
