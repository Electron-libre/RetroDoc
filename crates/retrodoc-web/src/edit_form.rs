//! The fields of the text of a section in its form (`specs/section-form.md` SFM-1): its name,
//! its description and, for a use case, its narrative, filled with the text its page shows; the
//! form saves them with the review (`section_form`). A `POST` to `/preview` followed by the URL of
//! the page of a use case answers the narrative it sends rendered as that page renders it
//! (WEB-119, WEB-120), and writes nothing.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse, Response};
use axum::Form;
use retrodoc_docs::edits::{self, Fields};
use retrodoc_docs::GeneratedDocs;
use serde::Deserialize;

use crate::collected::{narrative_html, NarrativeLinks};
use crate::edit::is_edited;
use crate::pages::not_found;
use crate::review::Place;
use crate::view::Site;

/// The fields of the text of a section in its form, filled with the text its page shows.
#[derive(Debug)]
pub struct EditFormView {
    /// Where the preview of its narrative is asked (WEB-119).
    pub preview: String,
    pub name: String,
    pub description: String,
    /// A use case only.
    pub narrative: Option<String>,
    /// The narrative rendered as its page renders it, safe to insert as is (WEB-119); empty but
    /// for a use case.
    pub narrative_preview: String,
    /// Whether the section holds an edit, which the button "back to the generated text" removes.
    pub edited: bool,
    /// Whether the fields are shown as a save sent them, which unfolds them (SFM-2).
    pub sent: bool,
    /// The text the page shows, sent back in hidden fields, so that a save compares the fields
    /// with what this page showed, not with an edit saved since by another page (SFM-3).
    pub shown: Fields,
}

impl EditFormView {
    /// The form of the section at `place` of `docs`, filled with the text shown (SED-23), its
    /// preview's links resolved by `links`.
    #[must_use]
    pub(crate) fn of(docs: &GeneratedDocs, place: &Place, links: &NarrativeLinks) -> Self {
        let shown = shown_in(docs, place);
        Self {
            sent: false,
            ..Self::new(place, shown.clone(), shown, is_edited(docs, place), links)
        }
    }

    /// The fields as a save sent them, kept in the form that says why it failed (SFM-15), with
    /// the text the page that sent them showed.
    #[must_use]
    pub(crate) fn sent(
        place: &Place,
        fields: Fields,
        shown: Fields,
        edited: bool,
        links: &NarrativeLinks,
    ) -> Self {
        Self::new(place, fields, shown, edited, links)
    }

    fn new(
        place: &Place,
        fields: Fields,
        shown: Fields,
        edited: bool,
        links: &NarrativeLinks,
    ) -> Self {
        let narrative = is_use_case(place).then(|| fields.narrative.unwrap_or_default());
        Self {
            preview: format!("/preview{}", place.href()),
            name: fields.name.unwrap_or_default(),
            description: fields.description.unwrap_or_default(),
            narrative_preview: preview_html(narrative.as_deref(), links),
            narrative,
            edited,
            sent: true,
            shown,
        }
    }
}

/// The text the page of the section at `place` of `docs` shows (SED-23), a narrative for a use
/// case only.
#[must_use]
pub(crate) fn shown_in(docs: &GeneratedDocs, place: &Place) -> Fields {
    let shown = edits::fields_in(docs.sections(), &place.section()).unwrap_or_default();
    Fields {
        narrative: is_use_case(place).then(|| shown.narrative.unwrap_or_default()),
        ..shown
    }
}

/// `narrative` rendered as the page of a use case renders it (WEB-118); empty when there is none.
fn preview_html(narrative: Option<&str>, links: &NarrativeLinks) -> String {
    narrative
        .and_then(|text| narrative_html(text, links))
        .unwrap_or_default()
}

/// What the preview sends: the narrative, the other fields of the form ignored.
#[derive(Debug, Deserialize)]
pub struct PreviewForm {
    narrative: Option<String>,
}

/// The narrative sent rendered as the narrative of the use case at the path, without the layout
/// of a page (WEB-120): nothing is read nor written; 404 when the run lacks that use case.
pub async fn preview(
    State(site): State<Arc<Site>>,
    Path((domain, feature, slug)): Path<(String, String, String)>,
    Form(form): Form<PreviewForm>,
) -> Response {
    let place = Place::UseCase {
        domain,
        feature,
        slug,
    };
    if !site.docs().is_some_and(|docs| place.in_docs(docs)) {
        return not_found(State(site)).await;
    }
    let links = NarrativeLinks::of(&site, &place);
    Html(preview_html(form.narrative.as_deref(), &links)).into_response()
}

fn is_use_case(place: &Place) -> bool {
    matches!(place, Place::UseCase { .. })
}
