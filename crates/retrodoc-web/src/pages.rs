//! The handlers of the pages and their templates (`templates/`, escaped as
//! HTML: names and narratives are LLM output).

use std::sync::Arc;

use askama::Template;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};

use retrodoc_render::{TECHNICAL_HEADING, TECHNICAL_NOTE};

use crate::view::{domain_page, domain_rows, DomainPage, DomainRow, Site, TreeDomain};

#[derive(Template)]
#[template(path = "home.html")]
struct Home<'a> {
    tree: &'a [TreeDomain],
    domains: Vec<DomainRow>,
}

#[derive(Template)]
#[template(path = "domain.html")]
struct DomainTemplate<'a> {
    tree: &'a [TreeDomain],
    page: DomainPage,
    technical_heading: &'static str,
    technical_note: &'static str,
}

#[derive(Template)]
#[template(path = "no_run.html")]
struct NoRun<'a> {
    tree: &'a [TreeDomain],
}

#[derive(Template)]
#[template(path = "not_found.html")]
struct NotFound<'a> {
    tree: &'a [TreeDomain],
}

pub async fn home(State(site): State<Arc<Site>>) -> Response {
    if site.docs().is_none() {
        return render(StatusCode::OK, &NoRun { tree: site.tree() });
    }
    // The stale badges read the cited files: off the async workers.
    let rows = {
        let site = Arc::clone(&site);
        tokio::task::spawn_blocking(move || site.docs().map(domain_rows).unwrap_or_default()).await
    };
    match rows {
        Ok(domains) => render(
            StatusCode::OK,
            &Home {
                tree: site.tree(),
                domains,
            },
        ),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("could not read the docs: {err}"),
        )
            .into_response(),
    }
}

pub async fn domain(State(site): State<Arc<Site>>, Path(slug): Path<String>) -> Response {
    let Some(page) = site
        .docs()
        .and_then(|docs| domain_page(docs, site.links(), &slug))
    else {
        return not_found(State(site)).await;
    };
    render(
        StatusCode::OK,
        &DomainTemplate {
            tree: site.tree(),
            page,
            technical_heading: TECHNICAL_HEADING,
            technical_note: TECHNICAL_NOTE,
        },
    )
}

pub async fn not_found(State(site): State<Arc<Site>>) -> Response {
    render(StatusCode::NOT_FOUND, &NotFound { tree: site.tree() })
}

fn render(status: StatusCode, page: &impl Template) -> Response {
    match page.render() {
        Ok(html) => (status, Html(html)).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("could not render the page: {err}"),
        )
            .into_response(),
    }
}
