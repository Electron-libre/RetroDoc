//! The handlers of the pages and their templates (`templates/`, escaped as
//! HTML: names and narratives are LLM output).

use std::sync::Arc;

use askama::Template;
use axum::extract::{Path, RawQuery, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use retrodoc_mcp::reviews::Reviews;
use retrodoc_mcp::GeneratedDocs;
use tokio::task::JoinError;

use crate::actors::ActorsPage;
use crate::assets::MERMAID;
use crate::audit::{audit_page, AuditPage};
use crate::code::{code_page, Asked, CodePage};
use crate::collected::{collected_page, CollectedPage, NarrativeLinks};
use crate::debt::{debt_page, DebtFilter, DebtPage};
use crate::edit::EditView;
use crate::edit_form::EditFormView;
use crate::files::FilesPage;
use crate::glossary::GlossaryPage;
use crate::history::{history_page, HistoryPage};
use crate::map::{DomainMapPage, MapPage};
use crate::review::{self, Place, ReviewView, SavedForm};
use crate::rules::{rules_page, RuleFilter, RulesPage};
use crate::search::{panel_answers, query_value, search_page, Results, SearchPage};
use crate::texts::WebTexts;
use crate::view::{
    domain_page, domain_rows, feature_page, use_case_page, DomainPage, DomainRow, FeaturePage,
    Shell, Site, StartLink, UseCasePage,
};

#[derive(Template)]
#[template(path = "home.html")]
struct Home<'a> {
    t: &'static WebTexts,
    shell: &'a Shell,
    purpose: Option<&'a str>,
    users: &'a [String],
    main_action: Option<MainAction>,
    start: Vec<StartLink>,
    domains: Vec<DomainRow>,
}

/// The primary button of the home page: the debt while it has items, else the map; none with no
/// domain, whose empty state says what to run (WEB-116).
struct MainAction {
    href: &'static str,
    label: String,
}

impl MainAction {
    fn of(start: &[StartLink], domains: &[DomainRow], texts: &WebTexts) -> Option<Self> {
        if domains.is_empty() {
            return None;
        }
        Some(match start.iter().find(|link| link.page.href == "/debt") {
            Some(debt) if debt.count > 0 => Self {
                href: debt.page.href,
                label: (texts.review_debt)(debt.count),
            },
            _ => Self {
                href: "/map",
                label: texts.read_map.to_owned(),
            },
        })
    }
}

#[derive(Template)]
#[template(path = "domain.html")]
struct DomainTemplate<'a> {
    t: &'static WebTexts,
    shell: &'a Shell,
    edit: Option<EditView>,
    review: ReviewView,
    edit_form: EditFormView,
    page: DomainPage,
}

#[derive(Template)]
#[template(path = "feature.html")]
struct FeatureTemplate<'a> {
    t: &'static WebTexts,
    shell: &'a Shell,
    edit: Option<EditView>,
    review: ReviewView,
    edit_form: EditFormView,
    page: FeaturePage,
}

#[derive(Template)]
#[template(path = "use_case.html")]
struct UseCaseTemplate<'a> {
    t: &'static WebTexts,
    shell: &'a Shell,
    edit: Option<EditView>,
    review: ReviewView,
    edit_form: EditFormView,
    page: UseCasePage,
    mermaid: &'static str,
}

#[derive(Template)]
#[template(path = "code.html")]
struct CodeTemplate<'a> {
    t: &'static WebTexts,
    shell: &'a Shell,
    page: CodePage,
}

#[derive(Template)]
#[template(path = "files.html")]
struct FilesTemplate<'a> {
    t: &'static WebTexts,
    shell: &'a Shell,
    page: &'a FilesPage,
}

#[derive(Template)]
#[template(path = "collected.html")]
struct CollectedTemplate<'a> {
    t: &'static WebTexts,
    shell: &'a Shell,
    page: CollectedPage,
}

#[derive(Template)]
#[template(path = "glossary.html")]
struct GlossaryTemplate<'a> {
    t: &'static WebTexts,
    shell: &'a Shell,
    page: &'a GlossaryPage,
}

#[derive(Template)]
#[template(path = "map.html")]
struct MapTemplate<'a> {
    t: &'static WebTexts,
    shell: &'a Shell,
    page: &'a MapPage,
    mermaid: &'static str,
}

#[derive(Template)]
#[template(path = "domain_map.html")]
struct DomainMapTemplate<'a> {
    t: &'static WebTexts,
    shell: &'a Shell,
    page: &'a DomainMapPage,
    mermaid: &'static str,
}

#[derive(Template)]
#[template(path = "actors.html")]
struct ActorsTemplate<'a> {
    t: &'static WebTexts,
    shell: &'a Shell,
    page: &'a ActorsPage,
}

#[derive(Template)]
#[template(path = "rules.html")]
struct RulesTemplate<'a> {
    t: &'static WebTexts,
    shell: &'a Shell,
    page: RulesPage,
}

#[derive(Template)]
#[template(path = "audit.html")]
struct AuditTemplate<'a> {
    t: &'static WebTexts,
    shell: &'a Shell,
    page: AuditPage,
}

#[derive(Template)]
#[template(path = "debt.html")]
struct DebtTemplate<'a> {
    t: &'static WebTexts,
    shell: &'a Shell,
    page: DebtPage,
}

/// The list alone, swapped under the form by `htmx`.
#[derive(Template)]
#[template(path = "debt_list.html")]
struct DebtList {
    t: &'static WebTexts,
    page: DebtPage,
}

/// The list alone, swapped under the form by `htmx`.
#[derive(Template)]
#[template(path = "rules_list.html")]
struct RulesList {
    t: &'static WebTexts,
    page: RulesPage,
}

#[derive(Template)]
#[template(path = "search.html")]
struct SearchTemplate<'a> {
    t: &'static WebTexts,
    shell: &'a Shell,
    page: SearchPage,
}

/// The hits panel alone, swapped under the box by `htmx` as the query is typed.
#[derive(Template)]
#[template(path = "search_panel.html")]
struct SearchPanel {
    t: &'static WebTexts,
    page: SearchPage,
}

#[derive(Template)]
#[template(path = "no_run.html")]
struct NoRun<'a> {
    t: &'static WebTexts,
    shell: &'a Shell,
}

/// The page that says to run `generate`, the answer of a page before a first run.
fn no_run_page(site: &Site) -> Response {
    render(
        StatusCode::OK,
        &NoRun {
            t: site.texts(),
            shell: site.shell(),
        },
    )
}

#[derive(Template)]
#[template(path = "not_found.html")]
struct NotFound<'a> {
    t: &'static WebTexts,
    shell: &'a Shell,
}

pub async fn home(State(site): State<Arc<Site>>) -> Response {
    if site.docs().is_none() {
        return no_run_page(&site);
    }
    match blocking(&site, |site| {
        (
            site.docs()
                .map(|docs| domain_rows(docs, &review::load(docs).unwrap_or_default()))
                .unwrap_or_default(),
            site.start_links(),
        )
    })
    .await
    {
        Ok((domains, start)) => render(
            StatusCode::OK,
            &Home {
                t: site.texts(),
                shell: site.shell(),
                purpose: site.purpose(),
                users: site.users(),
                main_action: MainAction::of(&start, &domains, site.texts()),
                start,
                domains,
            },
        ),
        Err(err) => read_failed(&err),
    }
}

/// Runs `build` off the async workers: the stale badges read the cited files.
pub(crate) async fn blocking<T: Send + 'static>(
    site: &Arc<Site>,
    build: impl FnOnce(&Site) -> T + Send + 'static,
) -> Result<T, JoinError> {
    let site = Arc::clone(site);
    tokio::task::spawn_blocking(move || build(&site)).await
}

pub(crate) fn read_failed(err: &JoinError) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("could not read the docs: {err}"),
    )
        .into_response()
}

pub async fn domain(
    State(site): State<Arc<Site>>,
    Path(slug): Path<String>,
    RawQuery(query): RawQuery,
) -> Response {
    let after = AfterSave::of(query.as_deref());
    let page = blocking(&site, move |site| {
        site.docs().and_then(|docs| {
            let place = Place::Domain(slug.clone());
            let (reviews, review) = reviews_of(docs, &place);
            let links = NarrativeLinks::of(site, &place);
            let edit = EditView::of(docs, &place, site.texts(), &links);
            let (review, edit_form) = after.shown(review, EditFormView::of(docs, &place, &links));
            domain_page(docs, site.links(), &reviews, &slug)
                .map(|page| (page, edit, review, edit_form))
        })
    })
    .await;
    match page {
        Ok(Some((page, edit, review, edit_form))) => render(
            StatusCode::OK,
            &DomainTemplate {
                t: site.texts(),
                shell: site.shell(),
                edit,
                review,
                edit_form,
                page,
            },
        ),
        Ok(None) => not_found(State(site)).await,
        Err(err) => read_failed(&err),
    }
}

/// What the forms of a section saved, as the redirect after a save names it (`?saved=review`,
/// `?saved=edit`), so that its page says so (WEB-104), and whether the page was asked to edit,
/// `?edit=1` (WEB-114).
struct AfterSave {
    saved: Option<SavedForm>,
    edit: bool,
}

impl AfterSave {
    fn of(query: Option<&str>) -> Self {
        Self {
            saved: query_value(query, "saved")
                .as_deref()
                .and_then(SavedForm::named),
            edit: query_value(query, "edit").as_deref() == Some("1"),
        }
    }

    /// `review` and `edit_form`, the one that was saved saying so; not a review whose reviews
    /// can't be read.
    fn shown(&self, review: ReviewView, edit_form: EditFormView) -> (ReviewView, EditFormView) {
        let review = ReviewView {
            saved: self.saved == Some(SavedForm::Review) && review.error.is_none(),
            ..review
        };
        let edit_form = EditFormView {
            saved: self.saved == Some(SavedForm::Edit),
            open: self.edit,
            ..edit_form
        };
        (review, edit_form)
    }
}

/// The reviews of the repository of `docs`, none when they can't be read, and the review of the
/// section at `place`, with the error when they can't.
fn reviews_of(docs: &GeneratedDocs, place: &Place) -> (Reviews, ReviewView) {
    let loaded = review::load(docs);
    let view = ReviewView::of(docs, &loaded, place);
    (loaded.unwrap_or_default(), view)
}

pub async fn feature(
    State(site): State<Arc<Site>>,
    Path((domain, slug)): Path<(String, String)>,
    RawQuery(query): RawQuery,
) -> Response {
    let after = AfterSave::of(query.as_deref());
    let page = blocking(&site, move |site| {
        site.docs().and_then(|docs| {
            let place = Place::Feature {
                domain: domain.clone(),
                slug: slug.clone(),
            };
            let (reviews, review) = reviews_of(docs, &place);
            let links = NarrativeLinks::of(site, &place);
            let edit = EditView::of(docs, &place, site.texts(), &links);
            let (review, edit_form) = after.shown(review, EditFormView::of(docs, &place, &links));
            feature_page(docs, site.links(), &reviews, &domain, &slug)
                .map(|page| (page, edit, review, edit_form))
        })
    })
    .await;
    match page {
        Ok(Some((page, edit, review, edit_form))) => render(
            StatusCode::OK,
            &FeatureTemplate {
                t: site.texts(),
                shell: site.shell(),
                edit,
                review,
                edit_form,
                page,
            },
        ),
        Ok(None) => not_found(State(site)).await,
        Err(err) => read_failed(&err),
    }
}

pub async fn use_case(
    State(site): State<Arc<Site>>,
    Path((domain, feature, slug)): Path<(String, String, String)>,
    RawQuery(query): RawQuery,
) -> Response {
    let after = AfterSave::of(query.as_deref());
    let page = blocking(&site, move |site| {
        site.docs().and_then(|docs| {
            let place = Place::UseCase {
                domain: domain.clone(),
                feature: feature.clone(),
                slug: slug.clone(),
            };
            let review = ReviewView::of(docs, &review::load(docs), &place);
            let links = NarrativeLinks::of(site, &place);
            let edit = EditView::of(docs, &place, site.texts(), &links);
            let (review, edit_form) = after.shown(review, EditFormView::of(docs, &place, &links));
            use_case_page(docs, site.links(), &domain, &feature, &slug, &links)
                .map(|page| (page, edit, review, edit_form))
        })
    })
    .await;
    match page {
        Ok(Some((page, edit, review, edit_form))) => render(
            StatusCode::OK,
            &UseCaseTemplate {
                t: site.texts(),
                shell: site.shell(),
                edit,
                review,
                edit_form,
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
            t: site.texts(),
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
            t: site.texts(),
            shell: site.shell(),
            page,
        },
    )
}

pub async fn map(State(site): State<Arc<Site>>) -> Response {
    let Some(page) = site.maps().map(|maps| &maps.overview) else {
        return no_run_page(&site);
    };
    render(
        StatusCode::OK,
        &MapTemplate {
            t: site.texts(),
            shell: site.shell(),
            page,
            mermaid: MERMAID,
        },
    )
}

pub async fn domain_map(State(site): State<Arc<Site>>, Path(slug): Path<String>) -> Response {
    let Some(maps) = site.maps() else {
        return no_run_page(&site);
    };
    let Some(page) = maps.domain(&slug) else {
        return not_found(State(site)).await;
    };
    render(
        StatusCode::OK,
        &DomainMapTemplate {
            t: site.texts(),
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
            t: site.texts(),
            shell: site.shell(),
            page,
        },
    )
}

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
        site.texts().no_run_rules,
        site.docs()
            .map(|docs| rules_page(docs, &filter, site.texts())),
        |page| RulesList {
            t: site.texts(),
            page,
        },
        |shell, page| RulesTemplate {
            t: site.texts(),
            shell,
            page,
        },
    )
}

pub async fn debt(
    State(site): State<Arc<Site>>,
    headers: HeaderMap,
    RawQuery(query): RawQuery,
) -> Response {
    let asked = |name| query_value(query.as_deref(), name).filter(|v: &String| !v.is_empty());
    let filter = DebtFilter {
        kind: asked("kind"),
        domain: asked("domain"),
    };
    // The stale items read the cited files.
    let built = blocking(&site, move |site| {
        site.docs()
            .map(|docs| debt_page(docs, &filter, site.texts()))
    })
    .await;
    match built {
        Ok(built) => fragment_or_page(
            &site,
            &headers,
            site.texts().debt.no_run,
            built,
            |page| DebtList {
                t: site.texts(),
                page,
            },
            |shell, page| DebtTemplate {
                t: site.texts(),
                shell,
                page,
            },
        ),
        Err(err) => read_failed(&err),
    }
}

pub async fn audit(State(site): State<Arc<Site>>) -> Response {
    if site.docs().is_none() {
        return no_run_page(&site);
    }
    // The stale share reads the cited files.
    let built = blocking(&site, |site| {
        site.docs()
            .map(|docs| audit_page(docs, site.history(), site.texts()))
    })
    .await;
    match built {
        Ok(Some(page)) => render(
            StatusCode::OK,
            &AuditTemplate {
                t: site.texts(),
                shell: site.shell(),
                page,
            },
        ),
        Ok(None) => no_run_page(&site),
        Err(err) => read_failed(&err),
    }
}

#[derive(Template)]
#[template(path = "history_body.html")]
struct HistoryBody {
    t: &'static WebTexts,
    page: HistoryPage,
}

#[derive(Template)]
#[template(path = "history.html")]
struct HistoryTemplate<'a> {
    t: &'static WebTexts,
    shell: &'a Shell,
    page: HistoryPage,
}

pub async fn history(
    State(site): State<Arc<Site>>,
    headers: HeaderMap,
    RawQuery(query): RawQuery,
) -> Response {
    let from = query_value(query.as_deref(), "from");
    let to = query_value(query.as_deref(), "to");
    let page = history_page(
        site.history(),
        site.docs(),
        from.as_deref(),
        to.as_deref(),
        site.texts(),
    );
    fragment_or_page(
        &site,
        &headers,
        site.texts().history_page.fewer,
        Some(page),
        |page| HistoryBody {
            t: site.texts(),
            page,
        },
        |shell, page| HistoryTemplate {
            t: site.texts(),
            shell,
            page,
        },
    )
}

pub async fn search(
    State(site): State<Arc<Site>>,
    headers: HeaderMap,
    RawQuery(query): RawQuery,
) -> Response {
    let query = query_value(query.as_deref(), "q").unwrap_or_default();
    if is_htmx(&headers) && !panel_answers(&query) {
        return ([(header::VARY, FRAGMENT_VARY)], Html("")).into_response();
    }
    fragment_or_page(
        &site,
        &headers,
        site.texts().no_run_results,
        site.docs().map(|docs| search_page(docs, &query)),
        |page| SearchPanel {
            t: site.texts(),
            page,
        },
        |shell, page| SearchTemplate {
            t: site.texts(),
            shell,
            page,
        },
    )
}

/// The headers a page that also answers a fragment varies on.
const FRAGMENT_VARY: &str = "HX-Request, HX-History-Restore-Request";

/// Whether `htmx` asks for a fragment: a history restore asks for the page.
fn is_htmx(headers: &HeaderMap) -> bool {
    headers.contains_key("HX-Request") && !headers.contains_key("HX-History-Restore-Request")
}

/// The part of a page `htmx` swaps, `fragment`, for its requests, else the
/// whole page; before a first `generate`, `no_run` or the no-run page. A
/// history restore asks for the whole page: `htmx` swaps it into the body.
fn fragment_or_page<'a, T, F: Template, P: Template>(
    site: &'a Site,
    headers: &HeaderMap,
    no_run: &'static str,
    built: Option<T>,
    fragment: impl FnOnce(T) -> F,
    page: impl FnOnce(&'a Shell, T) -> P,
) -> Response {
    // The same URL answers a fragment or a page: a cache must tell them apart.
    let vary = [(header::VARY, FRAGMENT_VARY)];
    let htmx = is_htmx(headers);
    let response = match (built, htmx) {
        (None, true) => Html(no_run).into_response(),
        (None, false) => no_run_page(site),
        (Some(built), true) => render(StatusCode::OK, &fragment(built)),
        (Some(built), false) => render(StatusCode::OK, &page(site.shell(), built)),
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
        code_page(site.docs()?, source, &path, asked, site.texts())
    })
    .await;
    match page {
        Ok(Some(page)) => render(
            StatusCode::OK,
            &CodeTemplate {
                t: site.texts(),
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
            t: site.texts(),
            shell: site.shell(),
        },
    )
}

/// A form answered to `htmx` after a refusal, the error in it: `HX-Reswap` asks `forms.js` to swap
/// it though its status is an error (WEB-105, WEB-106).
pub(crate) fn form_answer(status: StatusCode, form: &impl Template) -> Response {
    let mut response = render(status, form);
    response
        .headers_mut()
        .insert("HX-Reswap", HeaderValue::from_static("outerHTML"));
    response
}

pub(crate) fn render(status: StatusCode, page: &impl Template) -> Response {
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
            t: site.texts(),
            shell: site.shell(),
            page,
        },
    )
}
