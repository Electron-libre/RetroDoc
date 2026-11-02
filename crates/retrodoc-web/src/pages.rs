//! The handlers of the pages and their templates (`templates/`, escaped as
//! HTML: names and narratives are LLM output).

use std::sync::Arc;

use askama::Template;
use axum::extract::{Path, RawQuery, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use tokio::task::JoinError;

use retrodoc_mcp::GeneratedDocs;
use retrodoc_render::{TECHNICAL_HEADING, TECHNICAL_NOTE};

use crate::actors::ActorsPage;
use crate::assets::MERMAID;
use crate::code::{code_page, Asked, CodePage};
use crate::collected::{collected_page, CollectedPage};
use crate::files::FilesPage;
use crate::glossary::GlossaryPage;
use crate::map::MapPage;
use crate::rules::{rules_page, RuleFilter, RulesPage};
use crate::search::{query_value, search_page, Results, SearchPage};
use crate::view::{
    domain_page, domain_rows, feature_page, use_case_page, DomainPage, DomainRow, FeaturePage,
    Shell, Site, StartLink, UseCasePage,
};

#[derive(Template)]
#[template(path = "home.html")]
struct Home<'a> {
    shell: &'a Shell,
    purpose: Option<&'a str>,
    users: &'a [String],
    start: &'a [StartLink],
    domains: Vec<DomainRow>,
}

#[derive(Template)]
#[template(path = "domain.html")]
struct DomainTemplate<'a> {
    shell: &'a Shell,
    page: DomainPage,
    technical_heading: &'static str,
    technical_note: &'static str,
}

#[derive(Template)]
#[template(path = "feature.html")]
struct FeatureTemplate<'a> {
    shell: &'a Shell,
    page: FeaturePage,
    technical_heading: &'static str,
    technical_note: &'static str,
}

#[derive(Template)]
#[template(path = "use_case.html")]
struct UseCaseTemplate<'a> {
    shell: &'a Shell,
    page: UseCasePage,
    mermaid: &'static str,
}

#[derive(Template)]
#[template(path = "code.html")]
struct CodeTemplate<'a> {
    shell: &'a Shell,
    page: CodePage,
}

#[derive(Template)]
#[template(path = "files.html")]
struct FilesTemplate<'a> {
    shell: &'a Shell,
    page: &'a FilesPage,
}

#[derive(Template)]
#[template(path = "collected.html")]
struct CollectedTemplate<'a> {
    shell: &'a Shell,
    page: CollectedPage,
}

#[derive(Template)]
#[template(path = "glossary.html")]
struct GlossaryTemplate<'a> {
    shell: &'a Shell,
    page: &'a GlossaryPage,
}

#[derive(Template)]
#[template(path = "map.html")]
struct MapTemplate<'a> {
    shell: &'a Shell,
    page: &'a MapPage,
    mermaid: &'static str,
}

#[derive(Template)]
#[template(path = "actors.html")]
struct ActorsTemplate<'a> {
    shell: &'a Shell,
    page: &'a ActorsPage,
}

#[derive(Template)]
#[template(path = "rules.html")]
struct RulesTemplate<'a> {
    shell: &'a Shell,
    page: RulesPage,
}

/// The list alone, swapped under the form by `htmx`.
#[derive(Template)]
#[template(path = "rules_list.html")]
struct RulesList {
    page: RulesPage,
}

#[derive(Template)]
#[template(path = "search.html")]
struct SearchTemplate<'a> {
    shell: &'a Shell,
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
    shell: &'a Shell,
}

/// The page that says to run `generate`, the answer of a page before a first run.
fn no_run_page(site: &Site) -> Response {
    render(
        StatusCode::OK,
        &NoRun {
            shell: site.shell(),
        },
    )
}

#[derive(Template)]
#[template(path = "not_found.html")]
struct NotFound<'a> {
    shell: &'a Shell,
}

pub async fn home(State(site): State<Arc<Site>>) -> Response {
    if site.docs().is_none() {
        return no_run_page(&site);
    }
    match blocking(&site, |site| {
        site.docs().map(domain_rows).unwrap_or_default()
    })
    .await
    {
        Ok(domains) => render(
            StatusCode::OK,
            &Home {
                shell: site.shell(),
                purpose: site.purpose(),
                users: site.users(),
                start: site.start_links(),
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
            shell: site.shell(),
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
                shell: site.shell(),
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
                shell: site.shell(),
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
        return no_run_page(&site);
    };
    render(
        StatusCode::OK,
        &FilesTemplate {
            shell: site.shell(),
            page,
        },
    )
}

pub async fn glossary(State(site): State<Arc<Site>>) -> Response {
    let Some(page) = site.glossary() else {
        return no_run_page(&site);
    };
    render(
        StatusCode::OK,
        &GlossaryTemplate {
            shell: site.shell(),
            page,
        },
    )
}

pub async fn map(State(site): State<Arc<Site>>) -> Response {
    let Some(page) = site.map() else {
        return no_run_page(&site);
    };
    render(
        StatusCode::OK,
        &MapTemplate {
            shell: site.shell(),
            page,
            mermaid: MERMAID,
        },
    )
}

pub async fn actors(State(site): State<Arc<Site>>) -> Response {
    let Some(page) = site.actors() else {
        return no_run_page(&site);
    };
    render(
        StatusCode::OK,
        &ActorsTemplate {
            shell: site.shell(),
            page,
        },
    )
}

/// The list `htmx` swaps under the form before a first `generate`.
const NO_RUN_RULES: &str = "<section id=\"rule-list\"><p>No rule yet: run \
    <code>retrodoc generate</code>, then restart <code>retrodoc ui</code>.</p></section>";

pub async fn rules(
    State(site): State<Arc<Site>>,
    headers: HeaderMap,
    RawQuery(query): RawQuery,
) -> Response {
    let asked = |name| query_value(query.as_deref(), name).filter(|v: &String| !v.is_empty());
    let filter = RuleFilter {
        kind: asked("kind"),
        domain: asked("domain"),
        feature: asked("feature"),
    };
    fragment_or_page(
        &site,
        &headers,
        NO_RUN_RULES,
        |docs| rules_page(docs, &filter),
        |page| RulesList { page },
        |shell, page| RulesTemplate { shell, page },
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
    let query = query_value(query.as_deref(), "q").unwrap_or_default();
    fragment_or_page(
        &site,
        &headers,
        NO_RUN_RESULTS,
        |docs| search_page(docs, &query),
        |page| SearchResults { page },
        |shell, page| SearchTemplate { shell, page },
    )
}

/// The part of a page `htmx` swaps, `fragment`, for its requests, else the
/// whole page; before a first `generate`, `no_run` or the no-run page. A
/// history restore asks for the whole page: `htmx` swaps it into the body.
fn fragment_or_page<'a, T, F: Template, P: Template>(
    site: &'a Site,
    headers: &HeaderMap,
    no_run: &'static str,
    build: impl FnOnce(&GeneratedDocs) -> T,
    fragment: impl FnOnce(T) -> F,
    page: impl FnOnce(&'a Shell, T) -> P,
) -> Response {
    // The same URL answers a fragment or a page: a cache must tell them apart.
    let vary = [(header::VARY, "HX-Request, HX-History-Restore-Request")];
    let htmx =
        headers.contains_key("HX-Request") && !headers.contains_key("HX-History-Restore-Request");
    let response = match (site.docs(), htmx) {
        (None, true) => Html(no_run).into_response(),
        (None, false) => no_run_page(site),
        (Some(docs), true) => render(StatusCode::OK, &fragment(build(docs))),
        (Some(docs), false) => render(StatusCode::OK, &page(site.shell(), build(docs))),
    };
    (vary, response).into_response()
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
                shell: site.shell(),
                page,
            },
        ),
        Ok(None) => not_found(State(site)).await,
        Err(err) => read_failed(&err),
    }
}

pub async fn not_found(State(site): State<Arc<Site>>) -> Response {
    render(
        StatusCode::NOT_FOUND,
        &NotFound {
            shell: site.shell(),
        },
    )
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
            shell: site.shell(),
            page,
        },
    )
}
