//! The local web interface over the generated docs (ADR 0039): an `axum`
//! router that renders HTML pages on the server from `askama` templates, with
//! its scripts and stylesheet embedded in the binary. It calls no LLM, but
//! for the ask page of `retrodoc ui --ask` ([`Asking`], ADR 0048), and writes
//! nothing but the human reviews and edits its forms send ([`review_form`]).
//!
//! The pages are built from [`GeneratedDocs`], the read model `retrodoc-mcp`
//! loads, turned into the plain view structs of [`view`]; only [`router`] and
//! [`serve`] know HTTP. The docs are read once, at start, and rebuilt when the
//! edits saved beside them change ([`retrodoc_mcp::edits::Live`]); the reviews
//! and the edits are read at each request.

pub mod actors;
mod ask;
mod assets;
pub mod audit;
pub mod code;
pub mod collected;
pub mod debt;
pub mod edit;
pub mod edit_form;
pub mod files;
pub mod glossary;
pub mod history;
pub mod map;
mod pages;
pub mod review;
mod review_form;
pub mod rules;
pub mod search;
pub mod texts;
pub mod trends;
pub mod view;

use std::future::Future;
use std::net::IpAddr;
use std::path::Path;
use std::sync::Arc;

use axum::extract::{FromRef, Request};
use axum::http::{header, HeaderMap, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use retrodoc_core::config::OutputConfig;
use retrodoc_mcp::edits::Live;
use retrodoc_mcp::{ExistingDoc, GeneratedDocs};
use retrodoc_render::DocLinks;
use tokio::net::TcpListener;

pub use ask::{Asking, MAX_EXCHANGES, MAX_SESSIONS};
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
    /// The `lang` of the pages ([`Project::lang_of`]), which also picks the table of their fixed
    /// text; `None` for a language with no tag.
    pub lang: Option<&'static str>,
}

impl Project {
    /// The `lang` of the pages for the docs language of `output`: `en` when none is set, the tag
    /// of the language when it has one, else `None`.
    #[must_use]
    pub fn lang_of(output: &OutputConfig) -> Option<&'static str> {
        if output.language_name().is_none() {
            return Some("en");
        }
        output.language_tag()
    }
}

/// The site the pages are built from, rebuilt when the edits saved beside its docs change
/// (`specs/section-edit.md` SED-27): a handler that asks for the `Arc<Site>` state gets the site
/// with the edits saved at its request.
#[derive(Clone)]
pub(crate) struct LiveSite(Arc<Live<Site>>);

impl FromRef<LiveSite> for Arc<Site> {
    fn from_ref(live: &LiveSite) -> Self {
        live.0.current()
    }
}

impl LiveSite {
    fn new(
        project: Project,
        docs: Option<GeneratedDocs>,
        links: DocLinks,
        collected: Vec<ExistingDoc>,
        ask: bool,
    ) -> Self {
        let root = docs
            .as_ref()
            .and_then(GeneratedDocs::repo_root)
            .map(Path::to_path_buf);
        let site = Site::new(project.clone(), docs, links, collected).with_ask_link(ask);
        let rebuild = move |site: &Site, edits| {
            Site::new(
                project.clone(),
                site.docs().cloned().map(|docs| docs.with_edits(edits)),
                site.links().clone(),
                site.collected().to_vec(),
            )
            .with_ask_link(ask)
        };
        Self(Arc::new(Live::new(
            site,
            root.as_deref(),
            |site: &Site| site.docs().map(GeneratedDocs::edits),
            rebuild,
        )))
    }
}

/// The routes of the interface over the docs of `project`, `docs`, `None`
/// before a first `generate`, with the sections of the project's own docs
/// behind each item and those docs, `collected`, served as pages; no ask page.
pub fn router(
    project: Project,
    docs: Option<GeneratedDocs>,
    links: DocLinks,
    collected: Vec<ExistingDoc>,
) -> Router {
    router_with(project, docs, links, collected, None)
}

/// [`router`], with the ask page when `asking` is given (`specs/web-ask.md`).
pub fn router_with(
    project: Project,
    docs: Option<GeneratedDocs>,
    links: DocLinks,
    collected: Vec<ExistingDoc>,
    asking: Option<Asking>,
) -> Router {
    let live = LiveSite::new(project, docs, links, collected, asking.is_some());
    let pages = Router::new()
        .route("/", get(pages::home))
        .route(
            "/domains/{domain}",
            get(pages::domain).post(review_form::domain),
        )
        .route(
            "/domains/{domain}/{feature}",
            get(pages::feature).post(review_form::feature),
        )
        .route(
            "/domains/{domain}/{feature}/{use_case}",
            get(pages::use_case).post(review_form::use_case),
        )
        .route("/edit/domains/{slug}", post(edit_form::domain))
        .route("/edit/domains/{domain}/{slug}", post(edit_form::feature))
        .route(
            "/edit/domains/{domain}/{feature}/{slug}",
            post(edit_form::use_case),
        )
        .route("/files", get(pages::files))
        .route("/glossary", get(pages::glossary))
        .route("/rules", get(pages::rules))
        .route("/debt", get(pages::debt))
        .route("/audit", get(pages::audit))
        .route("/history", get(pages::history))
        .route("/map", get(pages::map))
        .route("/map/{domain}", get(pages::domain_map))
        .route("/actors", get(pages::actors))
        .route("/search", get(pages::search))
        .route("/code/{*path}", get(pages::code))
        .route("/docs/{*path}", get(pages::collected_doc))
        .route("/assets/{name}", get(assets::serve))
        .fallback(pages::not_found)
        .with_state(live.clone());
    let pages = match asking {
        Some(asking) => pages.merge(ask::routes(live, asking)),
        None => pages,
    };
    pages
        .layer(middleware::from_fn(refuse_cross_site))
        .layer(middleware::from_fn(refuse_foreign_host))
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

/// Refuses a request that may change something (any method but `GET` and
/// `HEAD`) unless it comes from a page of this server: its `Origin`, else its
/// `Referer`, is `http://` and its `Host`. A form of another site would
/// otherwise save a review in the user's name.
async fn refuse_cross_site(request: Request, next: Next) -> Response {
    let safe = matches!(*request.method(), Method::GET | Method::HEAD);
    if !safe && !same_origin(request.headers()) {
        return (StatusCode::FORBIDDEN, "cross-site request").into_response();
    }
    next.run(request).await
}

/// Whether the `Origin` of a request, else its `Referer`, is the server
/// itself, as its `Host` names it.
fn same_origin(headers: &HeaderMap) -> bool {
    // A header that is not text is kept as such: an `Origin` present never
    // falls back to the `Referer`.
    let value = |name| headers.get(name).map(|v| v.to_str().unwrap_or_default());
    let Some(host) = value(header::HOST).filter(|host| !host.is_empty()) else {
        return false;
    };
    let own = format!("http://{host}");
    match (value(header::ORIGIN), value(header::REFERER)) {
        (Some(origin), _) => origin == own,
        (None, Some(referer)) => referer
            .strip_prefix(&own)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('/')),
        (None, None) => false,
    }
}

/// Serves the interface on `listener`, with the ask page when `asking` is given, until
/// `shutdown` completes; the requests in flight are answered first.
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
    asking: Option<Asking>,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> std::io::Result<()> {
    axum::serve(
        listener,
        router_with(project, docs, links, collected, asking),
    )
    .with_graceful_shutdown(shutdown)
    .await
}
