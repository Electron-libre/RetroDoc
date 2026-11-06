//! The ask page of `retrodoc ui --ask` (`specs/web-ask.md`, ADR 0048): a question posted to `/ask`
//! is answered by the agent of `retrodoc-agent` in the conversation of the browser session, whose id
//! a cookie carries; the sessions live in memory, at most [`MAX_SESSIONS`].

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::{Arc, Mutex, PoisonError};

use askama::Template;
use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Form, Router};
use retrodoc_agent::{Answer, Conversation};
use retrodoc_llm::LlmProvider;
use retrodoc_mcp::{Docs, Source, SourceKind};
use serde::Deserialize;

use crate::collected::markdown_html;
use crate::pages::render;
use crate::texts::{AskTexts, WebTexts};
use crate::view::{code_href, Shell, Site};
use crate::LiveSite;

/// The sessions kept in memory; a new one past them drops the one used least recently (WAK-20).
pub const MAX_SESSIONS: usize = 32;

/// The exchanges a session keeps for its page (WAK-21).
pub const MAX_EXCHANGES: usize = 50;

/// The cookie that carries the id of the session.
const COOKIE: &str = "retrodoc_ask";

/// What `retrodoc ui --ask` asks with: the provider of the pass `ask` and the rounds of the agent.
pub struct Asking {
    pub llm: Arc<dyn LlmProvider>,
    pub max_rounds: usize,
}

/// A question and what came of it.
struct Exchange {
    question: String,
    outcome: Result<Answer, String>,
}

/// The conversation of a browser session and the exchanges its page shows.
#[derive(Default)]
struct Session {
    conversation: Conversation,
    exchanges: VecDeque<Exchange>,
    /// A question of the session is running (WAK-29).
    asking: bool,
}

/// The sessions, the one used last at the end.
#[derive(Default)]
struct Sessions(Vec<(String, Session)>);

impl Sessions {
    /// The session kept under `id`, moved to the end as the one used last.
    fn get(&mut self, id: Option<&str>) -> Option<&mut Session> {
        let at = self
            .0
            .iter()
            .position(|(kept, _)| Some(kept.as_str()) == id)?;
        let entry = self.0.remove(at);
        self.0.push(entry);
        self.0.last_mut().map(|(_, session)| session)
    }

    /// The id of the session kept under `id`, used last, or of a new one when none is, the session
    /// used least recently dropped past [`MAX_SESSIONS`]; `true` when the session is new.
    fn open(&mut self, id: Option<&str>) -> (String, bool) {
        if let Some(id) = id.filter(|_| self.get(id).is_some()) {
            return (id.to_string(), false);
        }
        if self.0.len() >= MAX_SESSIONS {
            self.0.remove(0);
        }
        let id = new_id();
        self.0.push((id.clone(), Session::default()));
        (id, true)
    }
}

/// 128 random bits in hexadecimal (WAK-18).
fn new_id() -> String {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).expect("the system gives no random bytes");
    bytes.iter().fold(String::new(), |mut id, byte| {
        let _ = write!(id, "{byte:02x}");
        id
    })
}

/// The docs of the agent and the site they were made from.
type SiteDocs = (Arc<Site>, Arc<Docs>);

/// The state of the ask routes.
#[derive(Clone)]
struct AskState {
    site: LiveSite,
    asking: Arc<Asking>,
    sessions: Arc<Mutex<Sessions>>,
    /// The docs the agent reads, made once per site (rebuilt with the edits).
    docs: Arc<Mutex<Option<SiteDocs>>>,
}

impl AskState {
    fn sessions(&self) -> std::sync::MutexGuard<'_, Sessions> {
        self.sessions.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The docs of `site` for the agent, `None` before a first run.
    fn docs(&self, site: &Arc<Site>) -> Option<Arc<Docs>> {
        let mut cached = self.docs.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some((_, docs)) = cached
            .as_ref()
            .filter(|(built, _)| Arc::ptr_eq(built, site))
        {
            return Some(Arc::clone(docs));
        }
        let docs = Arc::new(Docs::from(site.docs()?.clone()));
        *cached = Some((Arc::clone(site), Arc::clone(&docs)));
        Some(docs)
    }
}

/// The value of the session cookie a request carries.
fn cookie(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .find_map(|pair| {
            let (name, value) = pair.trim().split_once('=')?;
            (name == COOKIE).then(|| value.to_string())
        })
}

/// `response` with the cookie of the session `id` set when the session is `new`.
fn with_cookie(mut response: Response, id: &str, new: bool) -> Response {
    if new {
        let cookie = format!("{COOKIE}={id}; HttpOnly; SameSite=Strict; Path=/");
        if let Ok(value) = HeaderValue::from_str(&cookie) {
            response.headers_mut().insert(header::SET_COOKIE, value);
        }
    }
    response
}

/// The routes of the ask page over `site`.
pub(crate) fn routes(site: LiveSite, asking: Asking) -> Router {
    let state = AskState {
        site,
        asking: Arc::new(asking),
        sessions: Arc::default(),
        docs: Arc::default(),
    };
    Router::new()
        .route("/ask", get(page).post(question))
        .route("/ask/reset", post(reset))
        .with_state(state)
}

/// A source of an answer as the page shows it.
pub struct SourceLink {
    pub kind: &'static str,
    pub id: String,
    /// Its page, `None` for a use case whose id names no domain (WAK-11).
    pub href: Option<String>,
}

/// An exchange as the page shows it.
#[derive(Default)]
pub struct ExchangeView {
    pub question: String,
    /// The text of the answer rendered, safe to insert as is; `None` for a question that failed.
    pub html: Option<String>,
    pub error: Option<String>,
    pub sources: Vec<SourceLink>,
    pub warnings: Vec<String>,
}

impl ExchangeView {
    fn of(exchange: &Exchange, texts: &AskTexts) -> Self {
        let question = exchange.question.clone();
        match &exchange.outcome {
            Err(error) => Self {
                question,
                error: Some(error.clone()),
                ..Self::default()
            },
            Ok(answer) => {
                let text = Some(answer.text.trim()).filter(|text| !text.is_empty());
                let text = text.unwrap_or(texts.no_answer);
                Self {
                    question,
                    html: Some(markdown_html(text)),
                    error: None,
                    sources: answer
                        .sources
                        .iter()
                        .map(|source| source_link(source, texts))
                        .collect(),
                    warnings: answer
                        .warnings
                        .iter()
                        .map(|warning| (texts.warning)(warning, texts))
                        .collect(),
                }
            }
        }
    }
}

/// The link of `source` to its page (WAK-10).
fn source_link(source: &Source, texts: &AskTexts) -> SourceLink {
    let href = match source.kind {
        SourceKind::Domain | SourceKind::Feature => Some(format!("/domains/{}", source.id)),
        SourceKind::UseCase => {
            (source.id.matches('/').count() == 2).then(|| format!("/domains/{}", source.id))
        }
        SourceKind::File => Some(code_href(&source.id, None, None)),
    };
    SourceLink {
        kind: texts.kind(source.kind),
        id: source.id.clone(),
        href,
    }
}

#[derive(Template)]
#[template(path = "ask.html")]
struct AskPage<'a> {
    t: &'static WebTexts,
    shell: &'a Shell,
    exchanges: Vec<ExchangeView>,
}

/// One exchange alone, added after the former ones by `htmx`.
#[derive(Template)]
#[template(path = "exchange.html")]
struct ExchangeFragment {
    t: &'static WebTexts,
    exchange: ExchangeView,
}

async fn page(State(state): State<AskState>, headers: HeaderMap) -> Response {
    let site: Arc<Site> = state.site.0.current();
    let texts = site.texts();
    // A page with no session shows none and keeps none: only a question starts one.
    let exchanges = state
        .sessions()
        .get(cookie(&headers).as_deref())
        .map(|session| {
            session
                .exchanges
                .iter()
                .map(|exchange| ExchangeView::of(exchange, &texts.ask))
                .collect()
        })
        .unwrap_or_default();
    let page = AskPage {
        t: texts,
        shell: site.shell(),
        exchanges,
    };
    render(StatusCode::OK, &page)
}

/// What the form sends; a field it leaves out is empty.
#[derive(Debug, Deserialize)]
struct QuestionForm {
    #[serde(default)]
    question: String,
}

async fn question(
    State(state): State<AskState>,
    headers: HeaderMap,
    Form(form): Form<QuestionForm>,
) -> Response {
    let question = form.question.trim().to_string();
    if question.is_empty() {
        return (StatusCode::BAD_REQUEST, "the question is empty").into_response();
    }
    let site: Arc<Site> = state.site.0.current();
    let Some(docs) = state.docs(&site) else {
        return (
            StatusCode::NOT_FOUND,
            "no docs to ask: run `retrodoc generate`",
        )
            .into_response();
    };
    // The conversation is asked out of the lock, so the page and the other sessions are served
    // while the model answers.
    let (id, new, mut conversation) = {
        let mut sessions = state.sessions();
        let (id, new) = sessions.open(cookie(&headers).as_deref());
        let Some(session) = sessions.get(Some(&id)) else {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        };
        // Two questions asked from the same history would each drop the other from it.
        if session.asking {
            return (
                StatusCode::CONFLICT,
                "a question of this conversation is running",
            )
                .into_response();
        }
        session.asking = true;
        (id, new, session.conversation.clone())
    };
    let _running = Running {
        sessions: Arc::clone(&state.sessions),
        id: id.clone(),
    };
    let outcome = conversation
        .ask(
            state.asking.llm.as_ref(),
            docs,
            &question,
            state.asking.max_rounds,
        )
        .await
        .map_err(|error| error.to_string());
    let exchange = Exchange { question, outcome };
    let view = ExchangeView::of(&exchange, &site.texts().ask);
    // A session dropped while the model answered gets its answer in a session of its own.
    let (id, new) = {
        let mut sessions = state.sessions();
        let (kept, created) = sessions.open(Some(&id));
        if let Some(session) = sessions.get(Some(&kept)) {
            session.conversation = conversation;
            session.exchanges.push_back(exchange);
            while session.exchanges.len() > MAX_EXCHANGES {
                session.exchanges.pop_front();
            }
        }
        (kept, new || created)
    };
    let response = if headers.contains_key("HX-Request") {
        render(
            StatusCode::OK,
            &ExchangeFragment {
                t: site.texts(),
                exchange: view,
            },
        )
    } else {
        (StatusCode::SEE_OTHER, [(header::LOCATION, "/ask")]).into_response()
    };
    with_cookie(response, &id, new)
}

/// The question of a session running: the session can take another when it ends, answered or
/// dropped with its request (a tab closed while the model answers).
struct Running {
    sessions: Arc<Mutex<Sessions>>,
    id: String,
}

impl Drop for Running {
    fn drop(&mut self) {
        let mut sessions = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(session) = sessions.get(Some(&self.id)) {
            session.asking = false;
        }
    }
}

/// Empties the exchanges and the conversation of the session (WAK-22).
async fn reset(State(state): State<AskState>, headers: HeaderMap) -> Response {
    if let Some(session) = state.sessions().get(cookie(&headers).as_deref()) {
        *session = Session::default();
    }
    (StatusCode::SEE_OTHER, [(header::LOCATION, "/ask")]).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: WAK-11 (internal: a use case outside the run is named only by artifacts of an older run)
    #[test]
    fn a_use_case_whose_id_names_no_domain_has_no_link() {
        let texts = &WebTexts::ENGLISH.ask;
        let source = |id: &str| Source {
            kind: SourceKind::UseCase,
            id: id.into(),
        };

        let orphan = source_link(&source("pay-invoice/by-card"), texts);
        let placed = source_link(&source("billing/pay-invoice/by-card"), texts);

        assert_eq!(orphan.href, None);
        assert_eq!(
            placed.href.as_deref(),
            Some("/domains/billing/pay-invoice/by-card")
        );
    }

    // spec: WAK-20 (internal: 33 sessions through the router would only count cookies)
    #[test]
    fn past_the_bound_the_session_used_least_recently_is_dropped() {
        let mut sessions = Sessions::default();
        let (first, _) = sessions.open(None);
        let (second, _) = sessions.open(None);
        for _ in 2..MAX_SESSIONS {
            sessions.open(None);
        }
        // The first, used again, is no longer the least recent: the second goes.
        assert!(sessions.get(Some(&first)).is_some());
        sessions.open(None);

        assert!(sessions.get(Some(&first)).is_some());
        assert!(sessions.get(Some(&second)).is_none());
        assert!(sessions.open(Some(&second)).1);
        assert_eq!(sessions.0.len(), MAX_SESSIONS);
    }

    // spec: WAK-27 (internal: the link builder alone decides the encoding of a cited path)
    #[test]
    fn a_file_source_links_to_its_path_percent_encoded() {
        let file = Source {
            kind: SourceKind::File,
            id: "app/my order#1.rb".into(),
        };

        let link = source_link(&file, &WebTexts::ENGLISH.ask);

        assert_eq!(link.href.as_deref(), Some("/code/app/my%20order%231.rb"));
    }
}
