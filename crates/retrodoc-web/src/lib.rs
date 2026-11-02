//! The local web interface over the generated docs (ADR 0039): an `axum`
//! router that renders HTML pages on the server from `askama` templates, with
//! its scripts and stylesheet embedded in the binary. It calls no LLM and
//! writes nothing.
//!
//! The pages are built from [`GeneratedDocs`], the read model `retrodoc-mcp`
//! loads, turned into the plain view structs of [`view`]; only [`router`] and
//! [`serve`] know HTTP. The docs are read once, at start.

pub mod actors;
mod assets;
pub mod code;
pub mod collected;
pub mod files;
pub mod glossary;
mod pages;
pub mod rules;
pub mod search;
pub mod view;

use std::net::IpAddr;
use std::sync::Arc;

use axum::extract::Request;
use axum::http::{header, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use retrodoc_mcp::{ExistingDoc, GeneratedDocs};
use retrodoc_render::DocLinks;
use tokio::net::TcpListener;

use view::Site;

/// The documented application, as every page names it.
#[derive(Debug, Clone, Default)]
pub struct Project {
    pub name: String,
    /// The purpose of the product brief of the last run, shown on the home
    /// page; `None` with no brief.
    pub purpose: Option<String>,
    /// The users of the product brief, shown on the home page.
    pub users: Vec<String>,
}

/// The routes of the interface over the docs of `project`, `docs`, `None`
/// before a first `generate`, with the sections of the project's own docs
/// behind each item and those docs, `collected`, served as pages.
pub fn router(
    project: Project,
    docs: Option<GeneratedDocs>,
    links: DocLinks,
    collected: Vec<ExistingDoc>,
) -> Router {
    Router::new()
        .route("/", get(pages::home))
        .route("/domains/{domain}", get(pages::domain))
        .route("/domains/{domain}/{feature}", get(pages::feature))
        .route(
            "/domains/{domain}/{feature}/{use_case}",
            get(pages::use_case),
        )
        .route("/files", get(pages::files))
        .route("/glossary", get(pages::glossary))
        .route("/rules", get(pages::rules))
        .route("/actors", get(pages::actors))
        .route("/search", get(pages::search))
        .route("/code/{*path}", get(pages::code))
        .route("/docs/{*path}", get(pages::collected_doc))
        .route("/assets/{name}", get(assets::serve))
        .fallback(pages::not_found)
        .layer(middleware::from_fn(refuse_foreign_host))
        .with_state(Arc::new(Site::new(project, docs, links, collected)))
}

/// Refuses a request whose `Host` is a name other than `localhost`: a page of
/// another site that resolves its own name to this machine (DNS rebinding)
/// would otherwise read the docs. An IP address can't be rebound.
async fn refuse_foreign_host(request: Request, next: Next) -> Response {
    let foreign = request
        .headers()
        .get(header::HOST)
        .is_some_and(|host| !host.to_str().is_ok_and(trusted_host));
    if foreign {
        return (StatusCode::FORBIDDEN, "unexpected Host header").into_response();
    }
    next.run(request).await
}

/// `localhost` or an IP address, with or without a port.
fn trusted_host(host: &str) -> bool {
    let name = match host.strip_prefix('[') {
        Some(rest) => rest.split(']').next().unwrap_or_default(),
        None => host.rsplit_once(':').map_or(host, |(name, _)| name),
    };
    name.eq_ignore_ascii_case("localhost") || name.parse::<IpAddr>().is_ok()
}

/// Serves the interface on `listener` until the process stops.
///
/// # Errors
///
/// When the server fails to accept connections.
pub async fn serve(
    listener: TcpListener,
    project: Project,
    docs: Option<GeneratedDocs>,
    links: DocLinks,
    collected: Vec<ExistingDoc>,
) -> std::io::Result<()> {
    axum::serve(listener, router(project, docs, links, collected)).await
}
