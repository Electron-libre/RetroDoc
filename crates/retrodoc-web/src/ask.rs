//! The ask page of `retrodoc ui --ask` (`specs/web-ask.md`, ADR 0048): a question posted to `/ask`
//! is answered by the agent of `retrodoc-agent` in the conversation of the browser session, whose id
//! a cookie carries; the sessions live in memory.

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::{Arc, Mutex, PoisonError};

use askama::Template;
use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Form, Router};
use retrodoc_agent::{Answer, Conversation};
use retrodoc_llm::LlmProvider;
use retrodoc_mcp::{Docs, Source, SourceKind};
use serde::Deserialize;

use crate::collected::markdown_html;
use crate::pages::render;
use crate::texts::{AskTexts, WebTexts};
use crate::view::{Shell, Site};
use crate::LiveSite;

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
}

/// The sessions, the one used last at the end.
#[derive(Default)]
struct Sessions(Vec<(String, Arc<tokio::sync::Mutex<Session>>)>);

impl Sessions {
    /// The session of `id`, moved to the end as the one used last, or a new one with a new id when
    /// none is kept under it; the id is given back with `true` when the session is new.
    fn open(&mut self, id: Option<&str>) -> (String, Arc<tokio::sync::Mutex<Session>>, bool) {
        if let Some(at) = id.and_then(|id| self.0.iter().position(|(kept, _)| kept == id)) {
            let entry = self.0.remove(at);
            self.0.push(entry.clone());
            return (entry.0, entry.1, false);
        }
        let id = new_id();
        let session = Arc::new(tokio::sync::Mutex::new(Session::default()));
        self.0.push((id.clone(), Arc::clone(&session)));
        (id, session, true)
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

/// The state of the ask routes.
#[derive(Clone)]
struct AskState {
    site: LiveSite,
    asking: Arc<Asking>,
    sessions: Arc<Mutex<Sessions>>,
}

impl AskState {
    fn open(&self, headers: &HeaderMap) -> (String, Arc<tokio::sync::Mutex<Session>>, bool) {
        self.sessions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .open(cookie(headers).as_deref())
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
    };
    Router::new()
        .route("/ask", get(page).post(question))
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
        SourceKind::File => Some(format!("/code/{}", source.id)),
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
    let (id, session, new) = state.open(&headers);
    let texts = site.texts();
    let exchanges = session
        .lock()
        .await
        .exchanges
        .iter()
        .map(|exchange| ExchangeView::of(exchange, &texts.ask))
        .collect();
    let page = AskPage {
        t: texts,
        shell: site.shell(),
        exchanges,
    };
    with_cookie(render(StatusCode::OK, &page), &id, new)
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
    let Some(generated) = site.docs().cloned() else {
        return (
            StatusCode::NOT_FOUND,
            "no docs to ask: run `retrodoc generate`",
        )
            .into_response();
    };
    let (id, session, new) = state.open(&headers);
    let docs = Arc::new(Docs::from(generated));
    let mut session = session.lock().await;
    let outcome = session
        .conversation
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
    session.exchanges.push_back(exchange);
    drop(session);
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
}
