//! The form of the review of a section (`specs/web-ui.md`, WEB-81 to WEB-88): a `POST` to the
//! page of the section records its review through the store of `retrodoc-mcp`, or removes it, and
//! answers the review alone to `htmx`, else a redirect to the page.

use std::sync::Arc;

use askama::Template;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Form;
use retrodoc_mcp::reviews::{self, ReviewsError};
use serde::Deserialize;

use crate::pages::{blocking, form_answer, not_found, read_failed, render};
use crate::review::{self as review_view, Place, ReviewView, SavedForm};
use crate::texts::WebTexts;
use crate::view::Site;

/// What the form sends; a field it leaves out is empty.
#[derive(Debug, Deserialize)]
pub struct ReviewForm {
    #[serde(default)]
    status: String,
    #[serde(default)]
    comment: String,
}

/// The review of the section and its form, swapped in place of the former ones by `htmx`; an
/// error answered with it too, the error in the form, which `forms.js` swaps as any HTML error.
#[derive(Template)]
#[template(path = "review.html")]
struct ReviewFragment {
    t: &'static WebTexts,
    review: ReviewView,
}

pub async fn domain(
    State(site): State<Arc<Site>>,
    Path(slug): Path<String>,
    headers: HeaderMap,
    Form(form): Form<ReviewForm>,
) -> Response {
    save(site, Place::Domain(slug), &headers, form).await
}

pub async fn feature(
    State(site): State<Arc<Site>>,
    Path((domain, slug)): Path<(String, String)>,
    headers: HeaderMap,
    Form(form): Form<ReviewForm>,
) -> Response {
    save(site, Place::Feature { domain, slug }, &headers, form).await
}

pub async fn use_case(
    State(site): State<Arc<Site>>,
    Path((domain, feature, slug)): Path<(String, String, String)>,
    headers: HeaderMap,
    Form(form): Form<ReviewForm>,
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
    Done(ReviewView),
    NotFound,
    /// The status, and the review as it is saved, the error in its form.
    Refused(StatusCode, ReviewView),
}

async fn save(site: Arc<Site>, place: Place, headers: &HeaderMap, form: ReviewForm) -> Response {
    let htmx = headers.contains_key("HX-Request");
    let comment = form.comment.trim().to_string();
    let sent = review_view::parse(form.status.trim());
    let at = place.clone();
    let text = comment.clone();
    let saved = blocking(&site, move |site| {
        let Some(docs) = site.docs().filter(|docs| at.in_docs(docs)) else {
            return Saved::NotFound;
        };
        let errors = &site.texts().errors;
        let refused = |code, error: String| {
            let current = ReviewView::of(docs, &review_view::load(docs), &at);
            Saved::Refused(code, current.refused(error, sent.flatten(), text.clone()))
        };
        let Some(status) = sent else {
            return refused(StatusCode::BAD_REQUEST, errors.unknown_status.into());
        };
        let Some(root) = docs.repo_root() else {
            return refused(
                StatusCode::INTERNAL_SERVER_ERROR,
                errors.no_repository.into(),
            );
        };
        let section = at.section();
        // The view of what was saved, not of the file read again: another request may have
        // saved since.
        let done = match status {
            Some(status) => reviews::record(root, docs, &section, status, &text).map(Some),
            None => reviews::remove(root, &section).map(|()| None),
        };
        match done {
            Ok(review) => Saved::Done(ReviewView::shown(docs, &at, review.as_ref())),
            Err(ReviewsError::UnknownSection(_)) => Saved::NotFound,
            Err(err) => refused(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
        }
    })
    .await;
    let fragment = |review| ReviewFragment {
        t: site.texts(),
        review,
    };
    let refused = |code, review: ReviewView| {
        if htmx {
            form_answer(code, &fragment(review))
        } else {
            (code, review.form_error.unwrap_or_default()).into_response()
        }
    };
    match saved {
        Ok(Saved::Done(review)) if htmx => render(
            StatusCode::OK,
            &fragment(ReviewView {
                saved: true,
                ..review
            }),
        ),
        Ok(Saved::Done(_)) => (
            StatusCode::SEE_OTHER,
            [(header::LOCATION, place.saved_href(SavedForm::Review))],
        )
            .into_response(),
        // The form to `htmx`, which swaps an error it names the swap of; the not-found page
        // otherwise.
        Ok(Saved::NotFound) if htmx => {
            let error = site.texts().errors.no_section.to_string();
            let review = ReviewView::absent(&place, error, sent.flatten(), comment);
            refused(StatusCode::NOT_FOUND, review)
        }
        Ok(Saved::NotFound) => not_found(State(Arc::clone(&site))).await,
        Ok(Saved::Refused(code, review)) => refused(code, review),
        Err(err) => read_failed(&err),
    }
}
