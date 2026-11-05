//! The form of the human edit of a section (`specs/web-ui.md`, WEB-93 to WEB-99): a `POST` to
//! `/edit` followed by the URL of the page of the section records its edit through the store of
//! `retrodoc-mcp`, the fields it leaves as generated left out, or removes it, and answers `htmx`
//! with a reload of the page, else a redirect to it. The path sits under `/edit`, not after the
//! page's: a feature or a use case may be named `edit`.

use std::sync::Arc;

use askama::Template;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Form;
use retrodoc_mcp::edits::{self, EditsError, Fields};
use retrodoc_mcp::GeneratedDocs;
use serde::Deserialize;

use crate::pages::{blocking, not_found, read_failed, render};
use crate::review::Place;
use crate::texts::WebTexts;
use crate::view::Site;

/// What the form sends; a field it leaves out is empty, but for the narrative, which only the
/// form of a use case sends.
#[derive(Debug, Deserialize)]
pub struct EditForm {
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    narrative: Option<String>,
    /// `revert` for the button that brings the generated text back.
    #[serde(default)]
    action: String,
}

/// The edit form of a section, filled with the text its page shows.
#[derive(Debug)]
pub struct EditFormView {
    /// Where it posts.
    pub action: String,
    pub name: String,
    pub description: String,
    /// A use case only.
    pub narrative: Option<String>,
    /// Whether the section holds an edit, which the button "back to the generated text" removes.
    pub edited: bool,
    /// Why the last save failed, the fields kept as sent.
    pub error: Option<String>,
}

impl EditFormView {
    /// The form of the section at `place` of `docs`, filled with the text shown (SED-23).
    #[must_use]
    pub fn of(docs: &GeneratedDocs, place: &Place) -> Self {
        let section = place.section();
        let shown = edits::fields_in(docs.sections(), &section).unwrap_or_default();
        Self {
            action: action(place),
            name: shown.name.unwrap_or_default(),
            description: shown.description.unwrap_or_default(),
            narrative: is_use_case(place).then(|| shown.narrative.unwrap_or_default()),
            edited: docs.edits().get(&section).is_some(),
            error: None,
        }
    }

    fn failed(place: &Place, form: &EditForm, edited: bool, error: String) -> Self {
        Self {
            action: action(place),
            name: form.name.clone(),
            description: form.description.clone(),
            narrative: is_use_case(place).then(|| form.narrative.clone().unwrap_or_default()),
            edited,
            error: Some(error),
        }
    }
}

fn action(place: &Place) -> String {
    format!("/edit{}", place.href())
}

fn is_use_case(place: &Place) -> bool {
    matches!(place, Place::UseCase { .. })
}

/// The edit form alone, answered to `htmx` when a save failed and swapped in place of the former
/// one by the `before-swap` hook of its section.
#[derive(Template)]
#[template(path = "edit_form.html")]
struct EditFormFragment {
    t: &'static WebTexts,
    edit_form: EditFormView,
}

pub async fn domain(
    State(site): State<Arc<Site>>,
    Path(slug): Path<String>,
    headers: HeaderMap,
    Form(form): Form<EditForm>,
) -> Response {
    save(site, Place::Domain(slug), &headers, form).await
}

pub async fn feature(
    State(site): State<Arc<Site>>,
    Path((domain, slug)): Path<(String, String)>,
    headers: HeaderMap,
    Form(form): Form<EditForm>,
) -> Response {
    save(site, Place::Feature { domain, slug }, &headers, form).await
}

pub async fn use_case(
    State(site): State<Arc<Site>>,
    Path((domain, feature, slug)): Path<(String, String, String)>,
    headers: HeaderMap,
    Form(form): Form<EditForm>,
) -> Response {
    let place = Place::UseCase {
        domain,
        feature,
        slug,
    };
    save(site, place, &headers, form).await
}

/// What a save came to.
enum Saved {
    Done,
    NotFound,
    /// The status, the error, whether the section holds an edit.
    Failed(StatusCode, String, bool),
}

/// A field as the form sent it, trimmed and with `\n` line ends (WEB-94).
fn clean(text: &str) -> String {
    text.replace("\r\n", "\n").trim().to_string()
}

/// The fields of `form` that change the text generated (WEB-95): an empty one, or one equal to
/// the generated text, left out.
fn changed(form: &EditForm, generated: &Fields) -> Fields {
    let keep = |sent: Option<&str>, generated: Option<&str>| {
        let text = clean(sent?);
        (!text.is_empty() && text != generated.unwrap_or_default().trim()).then_some(text)
    };
    Fields {
        name: keep(Some(&form.name), generated.name.as_deref()),
        description: keep(Some(&form.description), generated.description.as_deref()),
        narrative: keep(form.narrative.as_deref(), generated.narrative.as_deref()),
    }
}

/// Records or removes the edit of the section at `place` of `docs`.
fn write(docs: &GeneratedDocs, place: &Place, form: &EditForm) -> Saved {
    let section = place.section();
    let edited = docs.edits().get(&section).is_some();
    let failed = |code, error: String| Saved::Failed(code, error, edited);
    // Only a use case has a narrative (SED-3): refused before the store is read, a revert
    // included.
    if form.narrative.is_some() && !is_use_case(place) {
        return failed(
            StatusCode::BAD_REQUEST,
            "only a use case has a narrative".into(),
        );
    }
    let Some(root) = docs.repo_root() else {
        return failed(
            StatusCode::INTERNAL_SERVER_ERROR,
            "the docs carry no repository to save the edits in".into(),
        );
    };
    let fields = if form.action == "revert" {
        Fields::default()
    } else {
        let generated = edits::fields_in(docs.generated(), &section).unwrap_or_default();
        changed(form, &generated)
    };
    let done = if fields == Fields::default() {
        edits::remove(root, &section)
    } else {
        edits::record(root, docs, &section, fields).map(|_| ())
    };
    match done {
        Ok(()) => Saved::Done,
        Err(EditsError::UnknownSection(_)) => Saved::NotFound,
        Err(err @ EditsError::NoNarrative(_)) => failed(StatusCode::BAD_REQUEST, err.to_string()),
        Err(err) => failed(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

async fn save(site: Arc<Site>, place: Place, headers: &HeaderMap, form: EditForm) -> Response {
    let htmx = headers.contains_key("HX-Request");
    let (at, sent) = (place.clone(), form);
    let saved = blocking(&site, move |site| {
        let saved = match site.docs().filter(|docs| at.in_docs(docs)) {
            None => Saved::NotFound,
            Some(docs) => write(docs, &at, &sent),
        };
        (saved, sent)
    })
    .await;
    let failed = |code, error: String, edited, form: &EditForm| {
        if htmx {
            let edit_form = EditFormView::failed(&place, form, edited, error);
            render(
                code,
                &EditFormFragment {
                    t: site.texts(),
                    edit_form,
                },
            )
        } else {
            (code, error).into_response()
        }
    };
    match saved {
        Ok((Saved::Done, _)) if htmx => {
            (StatusCode::OK, [("HX-Redirect", place.href())]).into_response()
        }
        Ok((Saved::Done, _)) => {
            (StatusCode::SEE_OTHER, [(header::LOCATION, place.href())]).into_response()
        }
        Ok((Saved::NotFound, form)) if htmx => failed(
            StatusCode::NOT_FOUND,
            "the last run has no such section".into(),
            false,
            &form,
        ),
        Ok((Saved::NotFound, _)) => not_found(State(Arc::clone(&site))).await,
        Ok((Saved::Failed(code, error, edited), form)) => failed(code, error, edited, &form),
        Err(err) => read_failed(&err),
    }
}
