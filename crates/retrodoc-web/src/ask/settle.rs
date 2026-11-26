//! The settling of a disputed section from its form (`specs/section-settle.md`, ADR 0057): the
//! button of the form, the start of the settling in the ask session of the browser, and the
//! section the ask page names while it lasts.

use std::sync::{Arc, Mutex, PoisonError};

use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Form;
use retrodoc_agent::{Answer, Conversation, Disagreement, Disputed, Proposal, Refusal, Step};
use retrodoc_core::reconcile::ItemKind;
use retrodoc_docs::reviews::{Section, SectionKind, Status};
use retrodoc_docs::GeneratedDocs;
use retrodoc_llm::CallTotals;
use serde::Deserialize;
use tokio::sync::watch;

use super::{cookie, with_cookie, AskState, Exchange, Progress, Running, Session, Sessions};
use crate::collected::{markdown_html, NarrativeLinks};
use crate::edit_form::EditFormView;
use crate::pages::read_failed;
use crate::review::{Place, ReviewView};
use crate::texts::WebTexts;
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

    /// The review and the form of the section at `place` as its page shows them for the session
    /// of `headers`: with the button of a settling (STL-1), and filled with the proposal of the
    /// exchange `number` the query `proposal` names when it proposes for that section (STL-8,
    /// STL-16), the fields of the text it corrects unfolded, its narrative previewed by `links`, and
    /// nothing saved.
    pub(crate) fn form(
        &self,
        headers: &HeaderMap,
        number: Option<&str>,
        place: &Place,
        review: ReviewView,
        edit_form: EditFormView,
        links: &NarrativeLinks,
    ) -> (ReviewView, EditFormView) {
        let section = place.section();
        let review = ReviewView {
            settle: self.settle_button(headers, &section),
            ..review
        };
        let Some(proposal) = self.proposal(headers, number, &section) else {
            return (review, edit_form);
        };
        let review = ReviewView {
            selected: Some(proposal.status),
            draft: Some(proposal.comment),
            proposed: true,
            ..review
        };
        let edit_form = edit_form.proposed(
            proposal.name,
            proposal.description,
            proposal.narrative,
            links,
        );
        (review, edit_form)
    }

    /// The proposal for `section` of the exchange `number` of the session of `headers`; none for
    /// any other (STL-16).
    fn proposal(
        &self,
        headers: &HeaderMap,
        number: Option<&str>,
        section: &Section,
    ) -> Option<Proposal> {
        let number: u64 = number?.parse().ok()?;
        let mut sessions = self
            .0
            .as_ref()?
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let session = sessions.get(cookie(headers).as_deref())?;
        let exchange = session.exchanges.iter().find(|e| e.number == number)?;
        let proposal = exchange.outcome.as_ref().ok()?.proposal.as_ref()?;
        (proposal.section == *section).then(|| proposal.clone())
    }
}

/// A field of the text a proposal corrects, as its exchange shows it.
pub struct ProposedField {
    pub label: &'static str,
    pub value: String,
}

/// The proposal of an exchange, as the ask page shows it (STL-7).
pub struct ProposalView {
    pub status: &'static str,
    pub comment: String,
    /// What it rests on, folded under the comment, rendered from its Markdown: safe to insert.
    pub evidence: String,
    pub fields: Vec<ProposedField>,
    /// The form of the section filled with it, `None` when the run no longer has the section.
    pub href: Option<String>,
}

impl ProposalView {
    pub(super) fn of(proposal: &Proposal, href: Option<&str>, texts: &WebTexts) -> Self {
        let labels = texts.edit.labels;
        let fields = [
            (labels[0], &proposal.name),
            (labels[1], &proposal.description),
            (labels[2], &proposal.narrative),
        ]
        .into_iter()
        .filter_map(|(label, value)| {
            value.as_ref().map(|value| ProposedField {
                label,
                value: value.clone(),
            })
        })
        .collect();
        Self {
            status: texts.review.status(proposal.status),
            comment: proposal.comment.clone(),
            evidence: markdown_html(&proposal.evidence),
            fields,
            href: href.map(str::to_string),
        }
    }
}

/// The form of the section of `proposal` filled with it, as the link of its exchange `number`
/// names it; `None` when the run of `site` no longer has the section.
pub(super) fn proposal_href(site: &Site, proposal: &Proposal, number: u64) -> Option<String> {
    let found = find(site.docs()?, &proposal.section)?;
    Some(format!("{}?proposal={number}#review", found.href))
}

/// Logs the usage of the question `running` that ended on `exchange`, then its tool calls, when it
/// is a question of a settling, also once its session was reset or dropped meanwhile, then the
/// calls of `propose_verdict` refused in a question that answered (STL-17, STL-19, STL-20).
pub(super) fn log_usage(running: &Running, exchange: &Exchange) {
    let Some(section) = &running.settled else {
        return;
    };
    let mut settling = running.spent_before;
    settling.add(&exchange.cost);
    let number = running.number + 1 - running.settling_from;
    let line = usage_line(
        section,
        number,
        &exchange.outcome,
        exchange.steps.len(),
        &exchange.cost,
        &settling,
    );
    tracing::info!("{line}");
    tracing::info!("{}", tools_line(section, number, &exchange.steps));
    if let Ok(answer) = &exchange.outcome {
        tracing::info!("{}", refusals_line(section, number, &answer.refusals));
    }
}

/// The line the log of `ui` gets after the tool line of the question `number` of the settling of
/// `section`: each call of `propose_verdict` refused in order, its status, the length of its
/// comment and its error (STL-20).
fn refusals_line(section: &Section, number: u64, refusals: &[Refusal]) -> String {
    let refused = if refusals.is_empty() {
        "none".to_string()
    } else {
        refusals
            .iter()
            .map(|refusal| {
                let status = refusal
                    .status
                    .as_ref()
                    .map_or("no status".to_string(), |status| {
                        format!("status `{status}`")
                    });
                format!(
                    "{status}, comment of {} characters, {}",
                    refusal.comment_chars, refusal.error
                )
            })
            .collect::<Vec<_>>()
            .join(" | ")
    };
    format!(
        "settling of the {} `{}`, question {number}, refused proposals: {refused}",
        kind_name(section.kind),
        section.id
    )
}

/// The line the log of `ui` gets after the usage line of the question `number` of the settling
/// of `section`: each tool call of the question in order, what it reads after it (STL-19).
pub(super) fn tools_line(section: &Section, number: u64, steps: &[Step]) -> String {
    let calls = if steps.is_empty() {
        "none".to_string()
    } else {
        steps
            .iter()
            .map(|step| match &step.target {
                Some(target) => format!("{} `{target}`", step.tool),
                None => step.tool.clone(),
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!(
        "settling of the {} `{}`, question {number}, tool calls: {calls}",
        kind_name(section.kind),
        section.id
    )
}

fn kind_name(kind: SectionKind) -> &'static str {
    match kind {
        SectionKind::Domain => "domain",
        SectionKind::Feature => "feature",
        SectionKind::UseCase => "use case",
    }
}

/// The line the log of `ui` gets when the question `number` of the settling of `section` ends
/// with `outcome` after `tool_calls` calls of tools, its LLM calls `cost` and those of the
/// settling so far `settling` (STL-17).
pub(super) fn usage_line(
    section: &Section,
    number: u64,
    outcome: &Result<Answer, String>,
    tool_calls: usize,
    cost: &CallTotals,
    settling: &CallTotals,
) -> String {
    let answer = outcome.as_ref().ok();
    // A failed question ended on a call that failed: each call answered before held tool calls.
    let rounds = answer.map_or(cost.calls, |answer| answer.rounds as u64);
    let proposed = match answer.and_then(|answer| answer.proposal.as_ref()) {
        Some(proposal) => match proposal.status {
            Status::Validated => "validated",
            Status::Wrong => "wrong",
            Status::ToReview => "to_review",
        },
        None => "none",
    };
    format!(
        "settling of the {} `{}`, question {number}: {}, {}, {}, proposed {proposed}; \
         settling: {}",
        kind_name(section.kind),
        section.id,
        counted(rounds, "round"),
        counted(tool_calls as u64, "tool call"),
        calls(cost),
        calls(settling),
    )
}

/// The LLM calls of `totals` and their tokens, as the log line of a settling says them.
fn calls(totals: &CallTotals) -> String {
    format!(
        "{} ({} without usage), {} tokens in ({} cached), {} out",
        counted(totals.calls, "LLM call"),
        totals.calls_without_usage,
        totals.prompt_tokens,
        totals.cached_tokens,
        totals.completion_tokens
    )
}

/// `n` `what`, in the plural but for one.
fn counted(n: u64, what: &str) -> String {
    if n == 1 {
        format!("1 {what}")
    } else {
        format!("{n} {what}s")
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
/// redirect `303`, to `/ask`. A refusal is its reason as plain text, which the notice of errors of
/// the section page shows (`forms.js`): that page has no slot of the ask page to swap it into.
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
        _ => return (StatusCode::BAD_REQUEST, texts.settle.unknown_kind).into_response(),
    };
    let (Some(generated), Some(docs)) = (site.docs(), state.docs(&site)) else {
        return (StatusCode::NOT_FOUND, texts.errors.no_docs).into_response();
    };
    let Some(found) = find(generated, &Section::new(kind, form.id.trim())) else {
        return (StatusCode::NOT_FOUND, texts.errors.no_section).into_response();
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
            return (StatusCode::CONFLICT, texts.errors.question_running).into_response();
        }
        *session = Session {
            conversation: Conversation::settling(disputed.section.clone()),
            asked: session.asked + 1,
            settling_from: session.asked + 1,
            ..Session::default()
        };
        let running = Arc::new(Running {
            number: session.asked,
            question: (texts.settle.question)(&found.name),
            message,
            settled: Some(disputed.section.clone()),
            settling_from: session.asked,
            spent_before: CallTotals::default(),
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
