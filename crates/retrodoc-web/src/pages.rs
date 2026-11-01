//! The handlers of the pages and their templates (`templates/`, escaped as
//! HTML: names and narratives are LLM output).

use std::sync::Arc;

use askama::Template;
use axum::extract::{Path, RawQuery, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use tokio::task::JoinError;

use retrodoc_render::{TECHNICAL_HEADING, TECHNICAL_NOTE};

use crate::assets::MERMAID;
use crate::code::{code_page, Asked, CodePage};
use crate::collected::{collected_page, CollectedPage};
use crate::files::FilesPage;
use crate::glossary::GlossaryPage;
use crate::search::{query_value, search_page, Results, SearchPage};
use crate::view::{
    domain_page, domain_rows, feature_page, use_case_page, DomainPage, DomainRow, FeaturePage,
    Site, TreeDomain, UseCasePage,
};

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
#[template(path = "feature.html")]
struct FeatureTemplate<'a> {
    tree: &'a [TreeDomain],
    page: FeaturePage,
    technical_heading: &'static str,
    technical_note: &'static str,
}

#[derive(Template)]
#[template(path = "use_case.html")]
struct UseCaseTemplate<'a> {
    tree: &'a [TreeDomain],
    page: UseCasePage,
    mermaid: &'static str,
}

#[derive(Template)]
#[template(path = "code.html")]
struct CodeTemplate<'a> {
    tree: &'a [TreeDomain],
    page: CodePage,
}

#[derive(Template)]
#[template(path = "files.html")]
struct FilesTemplate<'a> {
    tree: &'a [TreeDomain],
    page: &'a FilesPage,
}

#[derive(Template)]
#[template(path = "collected.html")]
struct CollectedTemplate<'a> {
    tree: &'a [TreeDomain],
    page: CollectedPage,
}

#[derive(Template)]
#[template(path = "glossary.html")]
struct GlossaryTemplate<'a> {
    tree: &'a [TreeDomain],
    page: &'a GlossaryPage,
}

#[derive(Template)]
#[template(path = "search.html")]
struct SearchTemplate<'a> {
    tree: &'a [TreeDomain],
    page: SearchPage,
}

/// The results alone, swapped under the box by `htmx`.
#[derive(Template)]
#[template(path = "search_results.html")]
struct SearchResults {
    page: SearchPage,
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
    match blocking(&site, |site| {
        site.docs().map(domain_rows).unwrap_or_default()
    })
    .await
    {
        Ok(domains) => render(
            StatusCode::OK,
            &Home {
                tree: site.tree(),
                domains,
            },
        ),
        Err(err) => read_failed(&err),
    }
}

/// Runs `build` off the async workers: the stale badges read the cited files.
async fn blocking<T: Send + 'static>(
    site: &Arc<Site>,
    build: impl FnOnce(&Site) -> T + Send + 'static,
) -> Result<T, JoinError> {
    let site = Arc::clone(site);
    tokio::task::spawn_blocking(move || build(&site)).await
}

fn read_failed(err: &JoinError) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("could not read the docs: {err}"),
    )
        .into_response()
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

pub async fn feature(
    State(site): State<Arc<Site>>,
    Path((domain, slug)): Path<(String, String)>,
) -> Response {
    let page = blocking(&site, move |site| {
        site.docs()
            .and_then(|docs| feature_page(docs, site.links(), &domain, &slug))
    })
    .await;
    match page {
        Ok(Some(page)) => render(
            StatusCode::OK,
            &FeatureTemplate {
                tree: site.tree(),
                page,
                technical_heading: TECHNICAL_HEADING,
                technical_note: TECHNICAL_NOTE,
            },
        ),
        Ok(None) => not_found(State(site)).await,
        Err(err) => read_failed(&err),
    }
}

pub async fn use_case(
    State(site): State<Arc<Site>>,
    Path((domain, feature, slug)): Path<(String, String, String)>,
) -> Response {
    let page = blocking(&site, move |site| {
        site.docs()
            .and_then(|docs| use_case_page(docs, site.links(), &domain, &feature, &slug))
    })
    .await;
    match page {
        Ok(Some(page)) => render(
            StatusCode::OK,
            &UseCaseTemplate {
                tree: site.tree(),
                page,
                mermaid: MERMAID,
            },
        ),
        Ok(None) => not_found(State(site)).await,
        Err(err) => read_failed(&err),
    }
}

pub async fn files(State(site): State<Arc<Site>>) -> Response {
    let Some(page) = site.files() else {
        return render(StatusCode::OK, &NoRun { tree: site.tree() });
    };
    render(
        StatusCode::OK,
        &FilesTemplate {
            tree: site.tree(),
            page,
        },
    )
}

pub async fn glossary(State(site): State<Arc<Site>>) -> Response {
    let Some(page) = site.glossary() else {
        return render(StatusCode::OK, &NoRun { tree: site.tree() });
    };
    render(
        StatusCode::OK,
        &GlossaryTemplate {
            tree: site.tree(),
            page,
        },
    )
}

/// The results `htmx` swaps under the box before a first `generate`.
const NO_RUN_RESULTS: &str = "<section class=\"results\"><p>Nothing to search yet: run \
    <code>retrodoc generate</code>, then restart <code>retrodoc ui</code>.</p></section>";

pub async fn search(
    State(site): State<Arc<Site>>,
    headers: HeaderMap,
    RawQuery(query): RawQuery,
) -> Response {
    // The same URL answers a fragment or a page: a cache must tell them apart.
    let vary = [(header::VARY, "HX-Request")];
    let htmx = headers.contains_key("HX-Request");
    let Some(docs) = site.docs() else {
        if htmx {
            return (vary, Html(NO_RUN_RESULTS)).into_response();
        }
        return (vary, render(StatusCode::OK, &NoRun { tree: site.tree() })).into_response();
    };
    let page = search_page(
        docs,
        &query_value(query.as_deref(), "q").unwrap_or_default(),
    );
    if htmx {
        return (vary, render(StatusCode::OK, &SearchResults { page })).into_response();
    }
    let page = SearchTemplate {
        tree: site.tree(),
        page,
    };
    (vary, render(StatusCode::OK, &page)).into_response()
}

pub async fn code(
    State(site): State<Arc<Site>>,
    Path(path): Path<String>,
    RawQuery(query): RawQuery,
) -> Response {
    let asked = Asked::of(query.as_deref());
    let page = blocking(&site, move |site| {
        let source = site.source()?;
        code_page(site.docs()?, source, &path, asked)
    })
    .await;
    match page {
        Ok(Some(page)) => render(
            StatusCode::OK,
            &CodeTemplate {
                tree: site.tree(),
                page,
            },
        ),
        Ok(None) => not_found(State(site)).await,
        Err(err) => read_failed(&err),
    }
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

pub async fn collected_doc(State(site): State<Arc<Site>>, Path(path): Path<String>) -> Response {
    let Some(page) = collected_page(site.collected(), &path) else {
        return not_found(State(site)).await;
    };
    render(
        StatusCode::OK,
        &CollectedTemplate {
            tree: site.tree(),
            page,
        },
    )
}
