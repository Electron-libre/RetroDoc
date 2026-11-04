//! The form of the review of a section (`specs/web-ui.md`, WEB-81 to WEB-88): a `POST` to the
//! page of the section records its review through the store of `retrodoc-mcp`, or removes it, and
//! answers the review alone to `htmx`, else a redirect to the page.

use std::sync::Arc;

use askama::Template;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Form;
use retrodoc_mcp::reviews::{self, ReviewsError, Status};
use serde::Deserialize;

use crate::pages::{blocking, not_found, read_failed, render};
use crate::review::{self as review_view, Place, ReviewView};
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

/// The review of the section and its form, swapped in place of the former ones by `htmx`.
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

/// The status a form sends: `None` for none, `Err` for a value outside the four.
fn status(value: &str) -> Result<Option<Status>, ()> {
    match value {
        "" => Ok(None),
        "validated" => Ok(Some(Status::Validated)),
        "wrong" => Ok(Some(Status::Wrong)),
        "to_review" => Ok(Some(Status::ToReview)),
        _ => Err(()),
    }
}

/// What a save came to.
enum Saved {
    Done(ReviewView),
    NotFound,
    Failed(StatusCode, String),
}

async fn save(site: Arc<Site>, place: Place, headers: &HeaderMap, form: ReviewForm) -> Response {
    let comment = form.comment.trim().to_string();
    let failed = |code, error: &str| {
        if headers.contains_key("HX-Request") {
            let review = ReviewView::failed(place.href(), error.to_string(), comment.clone());
            render(
                code,
                &ReviewFragment {
                    t: site.texts(),
                    review,
                },
            )
        } else {
            (code, error.to_string()).into_response()
        }
    };
    let Ok(status) = status(form.status.trim()) else {
        return failed(StatusCode::BAD_REQUEST, "unknown review status");
    };
    let at = place.clone();
    let text = comment.clone();
    let saved = blocking(&site, move |site| {
        let Some(docs) = site.docs().filter(|docs| at.in_docs(docs)) else {
            return Saved::NotFound;
        };
        let Some(root) = docs.repo_root() else {
            return Saved::Failed(
                StatusCode::INTERNAL_SERVER_ERROR,
                "the docs carry no repository to save the reviews in".into(),
            );
        };
        let section = at.section();
        let done = match status {
            Some(status) => reviews::record(root, docs, &section, status, &text).map(|_| ()),
            None => reviews::remove(root, &section),
        };
        match done {
            Ok(()) => Saved::Done(ReviewView::of(&review_view::load(docs), &at)),
            Err(ReviewsError::UnknownSection(_)) => Saved::NotFound,
            Err(err) => Saved::Failed(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
        }
    })
    .await;
    let htmx = headers.contains_key("HX-Request");
    match saved {
        Ok(Saved::Done(review)) if htmx => render(
            StatusCode::OK,
            &ReviewFragment {
                t: site.texts(),
                review,
            },
        ),
        Ok(Saved::Done(_)) => {
            (StatusCode::SEE_OTHER, [(header::LOCATION, place.href())]).into_response()
        }
        // The fragment to `htmx`, which swaps an error too; the not-found page otherwise.
        Ok(Saved::NotFound) if htmx => {
            failed(StatusCode::NOT_FOUND, "the last run has no such section")
        }
        Ok(Saved::NotFound) => not_found(State(Arc::clone(&site))).await,
        Ok(Saved::Failed(code, error)) => failed(code, &error),
        Err(err) => read_failed(&err),
    }
}
