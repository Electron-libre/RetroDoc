//! The settling of a disputed section from its form (`specs/section-settle.md`, ADR 0057): the
//! button of the form, the start of the settling in the ask session of the browser, and the
//! section the ask page names while it lasts.

use std::sync::{Arc, Mutex, PoisonError};

use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Form;
use retrodoc_agent::{Conversation, Disagreement, Disputed};
use retrodoc_core::reconcile::ItemKind;
use retrodoc_docs::reviews::{Section, SectionKind};
use retrodoc_docs::GeneratedDocs;
use serde::Deserialize;
use tokio::sync::watch;

use super::{cookie, refused, with_cookie, AskState, Progress, Running, Session, Sessions};
use crate::pages::read_failed;
use crate::view::{domain_href, feature_href, use_case_href, Site};

/// The sessions of the ask page as the pages of the sections see them: `None` without `--ask`.
#[derive(Clone, Default)]
pub(crate) struct Probe(pub(super) Option<Arc<Mutex<Sessions>>>);

impl Probe {
    /// The probe of `ui --ask`, over sessions shared with the ask routes.
    pub(crate) fn asking() -> Self {
        Self(Some(Arc::default()))
    }

    /// The button that settles `section` from its form, for the session of `headers`; none
    /// without `--ask` (STL-1). It asks a confirmation when the session holds an exchange or a
    /// question running (STL-12).
    pub(crate) fn settle_button(
        &self,
        headers: &HeaderMap,
        section: &Section,
    ) -> Option<SettleButton> {
        let sessions = self.0.as_ref()?;
        let mut sessions = sessions.lock().unwrap_or_else(PoisonError::into_inner);
        let confirm = sessions
            .get(cookie(headers).as_deref())
            .is_some_and(|session| !session.exchanges.is_empty() || session.running.is_some());
        Some(SettleButton {
            kind: kind_key(section.kind),
            id: section.id.clone(),
            confirm,
        })
    }
}

/// The button of the form of a section that starts its settling.
#[derive(Debug, Clone)]
pub struct SettleButton {
    /// `domain`, `feature` or `use_case`, as `/ask/settle` takes it.
    pub kind: &'static str,
    pub id: String,
    /// Whether it asks a confirmation before the exchanges of the session are dropped.
    pub confirm: bool,
}

fn kind_key(kind: SectionKind) -> &'static str {
    match kind {
        SectionKind::Domain => "domain",
        SectionKind::Feature => "feature",
        SectionKind::UseCase => "use_case",
    }
}

/// What the button sends.
#[derive(Debug, Deserialize)]
pub(super) struct SettleForm {
    #[serde(default)]
    kind: String,
    #[serde(default)]
    id: String,
}

/// A section of the last run, with its name and its page.
pub(super) struct Found {
    pub section: Section,
    pub name: String,
    pub href: String,
    /// What the reconciliation calls it: `<domain>[/<feature>[/<use case>]]`.
    label: String,
}

/// The section `id` of `kind` in `docs`, `None` when the run has none.
pub(super) fn find(docs: &GeneratedDocs, section: &Section) -> Option<Found> {
    let (name, href, label) = match section.kind {
        SectionKind::Domain => {
            let domain = docs.domains().iter().find(|d| d.slug == section.id)?;
            (
                domain.name.clone(),
                domain_href(&domain.slug),
                domain.slug.clone(),
            )
        }
        SectionKind::Feature => {
            let feature = docs.features().iter().find(|f| f.slug == section.id)?;
            let label = format!("{}/{}", feature.domain_slug, feature.slug);
            (feature.name.clone(), feature_href(feature), label)
        }
        SectionKind::UseCase => {
            let (feature_slug, slug) = section.id.split_once('/')?;
            let feature = docs.features().iter().find(|f| f.slug == feature_slug)?;
            let use_case = docs.use_cases_of(feature_slug).find(|u| u.slug == slug)?;
            let label = format!("{}/{}/{}", feature.domain_slug, feature.slug, use_case.slug);
            (
                use_case.name.clone(),
                use_case_href(feature, use_case),
                label,
            )
        }
    };
    Some(Found {
        section: section.clone(),
        name,
        href,
        label,
    })
}

/// The contradictions of the project's docs with `found` (REC-5).
fn disagreements(docs: &GeneratedDocs, found: &Found) -> Vec<Disagreement> {
    let kind = match found.section.kind {
        SectionKind::Domain => ItemKind::Domain,
        SectionKind::Feature => ItemKind::Feature,
        SectionKind::UseCase => ItemKind::UseCase,
    };
    docs.uncovered()
        .reconciliation
        .iter()
        .flat_map(|reconciliation| &reconciliation.items)
        .filter(|verdict| verdict.kind == kind && verdict.label == found.label)
        .flat_map(|verdict| &verdict.contradictions)
        .map(|c| Disagreement {
            doc_section: c.section.origin(),
            doc_states: c.doc_states.clone(),
            section_states: c.item_states.clone(),
        })
        .collect()
}

/// The section a settling session settles, as the ask page names it (STL-3).
pub struct SettledView {
    pub name: String,
    pub href: String,
}

impl SettledView {
    pub(super) fn of(site: &Site, session: &Session) -> Option<Self> {
        let found = find(site.docs()?, session.conversation.settled()?)?;
        Some(Self {
            name: found.name,
            href: found.href,
        })
    }
}

/// Starts the settling of the section the form names (STL-2, STL-10, STL-11): the session emptied,
/// then its first question sent; answered at once, to `htmx` with `HX-Redirect`, else with a
/// redirect `303`, to `/ask`.
pub(super) async fn settle(
    State(state): State<AskState>,
    headers: HeaderMap,
    Form(form): Form<SettleForm>,
) -> Response {
    let site: Arc<Site> = state.site.0.current();
    let htmx = headers.contains_key("HX-Request");
    let texts = site.texts();
    let kind = match form.kind.as_str() {
        "domain" => SectionKind::Domain,
        "feature" => SectionKind::Feature,
        "use_case" => SectionKind::UseCase,
        _ => return refused(StatusCode::BAD_REQUEST, texts.settle.unknown_kind, htmx),
    };
    let (Some(generated), Some(docs)) = (site.docs(), state.docs(&site)) else {
        return refused(StatusCode::NOT_FOUND, texts.errors.no_docs, htmx);
    };
    let Some(found) = find(generated, &Section::new(kind, form.id.trim())) else {
        return refused(StatusCode::NOT_FOUND, texts.errors.no_section, htmx);
    };
    let disputed = Disputed {
        disagreements: disagreements(generated, &found),
        section: found.section,
    };
    let message = {
        let docs = Arc::clone(&docs);
        let disputed = disputed.clone();
        match tokio::task::spawn_blocking(move || disputed.message(&docs)).await {
            Ok(message) => message,
            Err(err) => return read_failed(&err),
        }
    };
    let (id, new, running, conversation) = {
        let mut sessions = state.sessions();
        let (id, new) = sessions.open(cookie(&headers).as_deref());
        let Some(session) = sessions.get(Some(&id)) else {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        };
        if session.running.is_some() {
            return refused(StatusCode::CONFLICT, texts.errors.question_running, htmx);
        }
        *session = Session {
            conversation: Conversation::settling(disputed.section),
            asked: session.asked + 1,
            ..Session::default()
        };
        let running = Arc::new(Running {
            number: session.asked,
            question: (texts.settle.question)(&found.name),
            message,
            progress: watch::Sender::new(Progress::default()),
        });
        session.running = Some(Arc::clone(&running));
        (id, new, running, session.conversation.clone())
    };
    super::launch(state, id.clone(), running, conversation, docs);
    let response = if htmx {
        let mut response = StatusCode::OK.into_response();
        response
            .headers_mut()
            .insert("HX-Redirect", HeaderValue::from_static("/ask"));
        response
    } else {
        (StatusCode::SEE_OTHER, [(header::LOCATION, "/ask")]).into_response()
    };
    with_cookie(response, &id, new)
}
