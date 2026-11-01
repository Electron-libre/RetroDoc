//! The local web interface over the generated docs (ADR 0039): an `axum`
//! router that renders HTML pages on the server from `askama` templates, with
//! its scripts and stylesheet embedded in the binary. It calls no LLM and
//! writes nothing.
//!
//! The pages are built from [`GeneratedDocs`], the read model `retrodoc-mcp`
//! loads, turned into the plain view structs of [`view`]; only [`router`] and
//! [`serve`] know HTTP. The docs are read once, at start.

mod assets;
mod pages;
pub mod view;

use std::sync::Arc;

use axum::routing::get;
use axum::Router;
use retrodoc_mcp::GeneratedDocs;
use tokio::net::TcpListener;

use view::Site;

/// The routes of the interface over `docs`, `None` before a first `generate`.
pub fn router(docs: Option<GeneratedDocs>) -> Router {
    Router::new()
        .route("/", get(pages::home))
        .route("/assets/{name}", get(assets::serve))
        .fallback(pages::not_found)
        .with_state(Arc::new(Site::new(docs)))
}

/// Serves the interface on `listener` until the process stops.
///
/// # Errors
///
/// When the server fails to accept connections.
pub async fn serve(listener: TcpListener, docs: Option<GeneratedDocs>) -> std::io::Result<()> {
    axum::serve(listener, router(docs)).await
}
