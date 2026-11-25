//! The one form of a section page (`specs/section-form.md`): a `POST` to the page of a domain, a
//! feature or a use case saves its edit, then its review, through `retrodoc_docs::corrections`,
//! and answers `htmx` with a reload of the page, else a redirect to it; a refusal or a failure
//! answers `htmx` with the form alone, as sent, the error in it.

use std::sync::Arc;

use askama::Template;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Form;
use retrodoc_docs::corrections::{self, Correction, CorrectionError, EditChange};
use retrodoc_docs::edits::{self, EditsError, Field, Fields};
use retrodoc_docs::reviews::Status;
use retrodoc_docs::GeneratedDocs;
use serde::Deserialize;

use crate::ask::Probe;
use crate::collected::NarrativeLinks;
use crate::edit::is_edited;
use crate::edit_form::{is_outdated, shown_in, EditFormView};
use crate::pages::{blocking, form_answer, not_found, read_failed};
use crate::review::{self as review_view, Place, ReviewView};
use crate::texts::WebTexts;
use crate::view::Site;

/// What the form sends. A field of the text it leaves out is left as the page shows it, so that
/// a form sending a status alone keeps the edit; the narrative only the form of a use case sends.
#[derive(Debug, Clone, Deserialize)]
pub struct SectionForm {
    #[serde(default)]
    status: String,
    #[serde(default)]
    comment: String,
    name: Option<String>,
    description: Option<String>,
    narrative: Option<String>,
    /// The text the page showed, its hidden fields: what the fields sent are compared with
    /// (SFM-3); one left out is the text shown now.
    shown_name: Option<String>,
    shown_description: Option<String>,
    shown_narrative: Option<String>,
    /// Sent by the checkbox that confirms an outdated edit (SFM-17).
    #[serde(default)]
    confirm: String,
    /// `revert` for the button that brings the generated text back (SFM-5).
    #[serde(default)]
    action: String,
}

/// The review and the form of a section, answered to `htmx` when a save is refused or fails,
/// swapped in place of the former ones by `forms.js`, as any HTML error.
#[derive(Template)]
#[template(path = "section_form.html")]
struct FormFragment {
    t: &'static WebTexts,
    review: ReviewView,
    edit_form: EditFormView,
}

pub async fn domain(
    State(site): State<Arc<Site>>,
    State(probe): State<Probe>,
    Path(slug): Path<String>,
    headers: HeaderMap,
    Form(form): Form<SectionForm>,
) -> Response {
    save(site, &probe, Place::Domain(slug), &headers, form).await
}

pub async fn feature(
    State(site): State<Arc<Site>>,
    State(probe): State<Probe>,
    Path((domain, slug)): Path<(String, String)>,
    headers: HeaderMap,
    Form(form): Form<SectionForm>,
) -> Response {
    save(
        site,
        &probe,
        Place::Feature { domain, slug },
        &headers,
        form,
    )
    .await
}

pub async fn use_case(
    State(site): State<Arc<Site>>,
    State(probe): State<Probe>,
    Path((domain, feature, slug)): Path<(String, String, String)>,
    headers: HeaderMap,
    Form(form): Form<SectionForm>,
) -> Response {
    let place = Place::UseCase {
        domain,
        feature,
        slug,
    };
    save(site, &probe, place, &headers, form).await
}

/// What a save came to.
enum Saved {
    Done,
    NotFound,
    /// The status, and the review as it is saved with the form as sent, the error in it.
    Refused(StatusCode, Box<(ReviewView, EditFormView)>),
}

/// A field as the form sent it, trimmed and with `\n` line ends (SFM-10).
fn clean(text: &str) -> String {
    text.replace("\r\n", "\n").trim().to_string()
}

/// A field as a browser sends it back: cleaned, and a name without its line breaks, which an
/// `<input>` drops.
fn as_sent(text: Option<&str>, single_line: bool) -> String {
    let text = clean(text.unwrap_or_default());
    if single_line {
        text.replace('\n', "")
    } else {
        text
    }
}

impl SectionForm {
    /// The text the page that sent the form showed, the text shown now for a field it left out.
    fn shown(&self, now: Fields) -> Fields {
        Fields {
            name: self.shown_name.clone().or(now.name),
            description: self.shown_description.clone().or(now.description),
            narrative: self.shown_narrative.clone().or(now.narrative),
        }
    }

    /// The fields of the text it sends, a field it leaves out as `shown`.
    fn fields(&self, shown: &Fields) -> Fields {
        Fields {
            name: self.name.clone().or_else(|| shown.name.clone()),
            description: self
                .description
                .clone()
                .or_else(|| shown.description.clone()),
            narrative: self.narrative.clone().or_else(|| shown.narrative.clone()),
        }
    }
}

/// What `form` does to the edit of the section at `place` of `docs` (SFM-3, SFM-11, SFM-17): the
/// revert removes it; fields sent as the page showed them keep it, unless an outdated edit is
/// confirmed; else the fields that change the generated text, cleaned, none removing it, written
/// confirmed when an outdated edit is. A field sent as the page showed it is taken as `docs` show
/// it now, so that it keeps an edit saved since by another page and the base of its text (SED-15).
fn edit_change(docs: &GeneratedDocs, place: &Place, form: &SectionForm) -> EditChange {
    if form.action == "revert" {
        return EditChange::Remove;
    }
    let now = shown_in(docs, place);
    let shown = form.shown(now.clone());
    let sent = form.fields(&shown);
    let generated = edits::fields_in(docs.generated(), &place.section()).unwrap_or_default();
    let untouched = |field: Field| {
        let single_line = field == Field::Name;
        as_sent(sent.get(field), single_line) == as_sent(shown.get(field), single_line)
    };
    let unchanged = Field::ALL.into_iter().all(untouched);
    let confirmed = !form.confirm.is_empty() && is_outdated(docs, place);
    if unchanged && !confirmed {
        return EditChange::Keep;
    }
    let kept = |field: Field| {
        let text = if untouched(field) { &now } else { &sent }.get(field);
        let text = clean(text.unwrap_or_default());
        (!text.is_empty() && text != clean(generated.get(field).unwrap_or_default()))
            .then_some(text)
    };
    let fields = Fields {
        name: kept(Field::Name),
        description: kept(Field::Description),
        narrative: kept(Field::Narrative),
    };
    if fields == Fields::default() {
        EditChange::Remove
    } else if confirmed {
        EditChange::Confirm(fields)
    } else {
        EditChange::Set(fields)
    }
}

/// Saves `form` of the section at `place` of `docs`.
fn write(site: &Site, docs: &GeneratedDocs, place: &Place, form: &SectionForm) -> Saved {
    let errors = &site.texts().errors;
    let sent = review_view::parse(form.status.trim());
    let comment = form.comment.trim().to_string();
    let refused = |code, error: String| {
        let current = ReviewView::of(docs, &review_view::load(docs), place);
        let review = current.refused(error, sent.flatten(), comment.clone());
        let fields = sent_form(
            site,
            place,
            form,
            (is_edited(docs, place), is_outdated(docs, place)),
            shown_in(docs, place),
        );
        Saved::Refused(code, Box::new((review, fields)))
    };
    let Some(status) = sent else {
        return refused(StatusCode::BAD_REQUEST, errors.unknown_status.into());
    };
    if form.narrative.is_some() && !matches!(place, Place::UseCase { .. }) {
        return refused(
            StatusCode::BAD_REQUEST,
            errors.narrative_of_use_case_only.into(),
        );
    }
    let Some(root) = docs.repo_root() else {
        return refused(
            StatusCode::INTERNAL_SERVER_ERROR,
            errors.no_repository.into(),
        );
    };
    let correction = Correction {
        edit: edit_change(docs, place, form),
        review: status.map(|status: Status| (status, comment.clone())),
    };
    match corrections::save(root, docs, &place.section(), &correction) {
        Ok(_) => Saved::Done,
        Err(err) if err.is_unknown_section() => Saved::NotFound,
        Err(CorrectionError::Edits(EditsError::NoNarrative(_))) => refused(
            StatusCode::BAD_REQUEST,
            errors.narrative_of_use_case_only.into(),
        ),
        Err(err) => refused(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

/// The fields of the text as `form` sent them, unfolded to show them, and its confirmation, offered
/// when the edit is outdated; a field it left out, and the hidden text shown, as `now`, the text
/// the page shows now (SFM-15).
fn sent_form(
    site: &Site,
    place: &Place,
    form: &SectionForm,
    (edited, outdated): (bool, bool),
    now: Fields,
) -> EditFormView {
    let links = NarrativeLinks::of(site, place);
    let shown = form.shown(now);
    let state = (edited, outdated, !form.confirm.is_empty());
    EditFormView::sent(place, form.fields(&shown), shown, state, &links)
}

async fn save(
    site: Arc<Site>,
    probe: &Probe,
    place: Place,
    headers: &HeaderMap,
    form: SectionForm,
) -> Response {
    let htmx = headers.contains_key("HX-Request");
    let (at, sent) = (place.clone(), form.clone());
    let saved = blocking(&site, move |site| {
        match site.docs().filter(|docs| at.in_docs(docs)) {
            None => Saved::NotFound,
            Some(docs) => write(site, docs, &at, &sent),
        }
    })
    .await;
    let refused = |code, review: ReviewView, edit_form| {
        if htmx {
            let fragment = FormFragment {
                t: site.texts(),
                review,
                edit_form,
            };
            form_answer(code, &fragment)
        } else {
            (code, review.form_error.unwrap_or_default()).into_response()
        }
    };
    match saved {
        Ok(Saved::Done) if htmx => {
            (StatusCode::OK, [("HX-Redirect", place.saved_href())]).into_response()
        }
        Ok(Saved::Done) => (
            StatusCode::SEE_OTHER,
            [(header::LOCATION, place.saved_href())],
        )
            .into_response(),
        // The form to `htmx`, which swaps an error it names the swap of; the not-found page
        // otherwise.
        Ok(Saved::NotFound) if htmx => {
            let error = site.texts().errors.no_section.to_string();
            let sent = review_view::parse(form.status.trim()).flatten();
            let review = ReviewView::absent(&place, error, sent, form.comment.trim().into());
            let edit_form = sent_form(&site, &place, &form, (false, false), Fields::default());
            refused(StatusCode::NOT_FOUND, review, edit_form)
        }
        Ok(Saved::NotFound) => not_found(State(Arc::clone(&site))).await,
        Ok(Saved::Refused(code, form)) => {
            let (review, edit_form) = *form;
            let (review, edit_form) = probe.form(
                headers,
                None,
                &place,
                review,
                edit_form,
                &NarrativeLinks::of(&site, &place),
            );
            refused(code, review, edit_form)
        }
        Err(err) => read_failed(&err),
    }
}
