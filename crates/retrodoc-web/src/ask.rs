//! The ask page of `retrodoc ui --ask` (`specs/web-ask.md`, ADR 0048): a question posted to `/ask`
//! is answered by the agent of `retrodoc-agent` in the conversation of the browser session, whose id
//! a cookie carries; the sessions live in memory, at most [`MAX_SESSIONS`]. A question runs in a
//! task of its own (ADR 0049): its running exchange follows its steps with server-sent events, or
//! asks for the exchange once the question has ended when the stream fails.

use std::collections::VecDeque;
use std::convert::Infallible;
use std::fmt::Write as _;
use std::sync::{Arc, Mutex, PoisonError};

use askama::Template;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Form, Router};
use retrodoc_agent::{Answer, Conversation, Step};
use retrodoc_llm::{CallTotals, LlmProvider, UsageProvider, UsageTracker};
use retrodoc_mcp::{Docs, GeneratedDocs, Source, SourceKind};
use serde::Deserialize;
use tokio::sync::watch;

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
    /// Its number in its session.
    number: u64,
    question: String,
    steps: Vec<Step>,
    outcome: Result<Answer, String>,
    /// The calls of the question and of its session up to it (WAK-36).
    cost: CallTotals,
    session_cost: CallTotals,
}

/// What a question has done so far.
#[derive(Default)]
struct Progress {
    steps: Vec<Step>,
    /// Its exchange, once it has ended.
    ended: Option<Arc<Exchange>>,
}

/// A question running in a task of the server.
struct Running {
    number: u64,
    question: String,
    progress: watch::Sender<Progress>,
}

/// The conversation of a browser session and the exchanges its page shows.
#[derive(Default)]
struct Session {
    conversation: Conversation,
    exchanges: VecDeque<Arc<Exchange>>,
    /// The question of the session running, if any (WAK-29).
    running: Option<Arc<Running>>,
    /// The questions sent in the session, which number them; not reset, so a number names one
    /// question only.
    asked: u64,
    /// The calls of the questions of the session since it started or was reset.
    spent: CallTotals,
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
        let id = new_id();
        self.push(id.clone(), Session::default());
        (id, true)
    }

    /// The session `id`, dropped while it ran `running`, started again as running it.
    fn reopen(&mut self, id: &str, running: &Arc<Running>) -> &mut Session {
        let session = Session {
            asked: running.number,
            running: Some(Arc::clone(running)),
            ..Session::default()
        };
        self.push(id.to_string(), session)
    }

    /// Keeps `session` under `id` as the one used last, past [`MAX_SESSIONS`] the session used
    /// least recently dropped, one whose question runs only when all of them run (WAK-20).
    fn push(&mut self, id: String, session: Session) -> &mut Session {
        if self.0.len() >= MAX_SESSIONS {
            let idle = self.0.iter().position(|(_, kept)| kept.running.is_none());
            self.0.remove(idle.unwrap_or(0));
        }
        self.0.push((id, session));
        &mut self.0.last_mut().expect("a session was just pushed").1
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

/// The questions running in their tasks, which the end of [`crate::serve`] waits for (WAK-23).
#[derive(Clone, Default)]
pub(crate) struct InFlight(Arc<watch::Sender<usize>>);

impl InFlight {
    /// Counts one more question running until the guard it gives is dropped.
    fn start(&self) -> Started {
        self.0.send_modify(|n| *n += 1);
        Started(self.clone())
    }

    /// Once no question runs.
    pub(crate) async fn idle(&self) {
        let _ = self.0.subscribe().wait_for(|n| *n == 0).await;
    }
}

/// A question counted in [`InFlight`] while it lives.
struct Started(InFlight);

impl Drop for Started {
    fn drop(&mut self) {
        self.0 .0.send_modify(|n| *n -= 1);
    }
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
    in_flight: InFlight,
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

    /// The progress of the question `number` of the session of `headers`, running or among its
    /// exchanges (WAK-34).
    fn follow(&self, headers: &HeaderMap, number: u64) -> Option<watch::Receiver<Progress>> {
        let mut sessions = self.sessions();
        let session = sessions.get(cookie(headers).as_deref())?;
        if let Some(running) = session.running.as_ref().filter(|r| r.number == number) {
            return Some(running.progress.subscribe());
        }
        let exchange = session.exchanges.iter().find(|e| e.number == number)?;
        let (_, ended) = watch::channel(Progress {
            steps: exchange.steps.clone(),
            ended: Some(Arc::clone(exchange)),
        });
        Some(ended)
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
pub(crate) fn routes(site: LiveSite, asking: Asking, in_flight: InFlight) -> Router {
    let state = AskState {
        site,
        asking: Arc::new(asking),
        sessions: Arc::default(),
        docs: Arc::default(),
        in_flight,
    };
    Router::new()
        .route("/ask", get(page).post(question))
        .route("/ask/reset", post(reset))
        .route("/ask/questions/{number}", get(ended))
        .route("/ask/questions/{number}/events", get(events))
        .with_state(state)
}

/// A source of an answer as the page shows it.
pub struct SourceLink {
    pub kind: &'static str,
    pub id: String,
    /// Its page, `None` for a use case whose id names no domain (WAK-11).
    pub href: Option<String>,
}

/// A step of a question as the page shows it.
#[derive(Template)]
#[template(path = "step.html")]
pub struct StepView<'a> {
    pub step: &'a Step,
}

/// An exchange as the page shows it.
#[derive(Default)]
pub struct ExchangeView {
    pub question: String,
    pub steps: Vec<Step>,
    /// The text of the answer rendered, safe to insert as is; `None` for a question that failed.
    pub html: Option<String>,
    pub error: Option<String>,
    pub sources: Vec<SourceLink>,
    pub warnings: Vec<String>,
    /// The cost of the question, then of the session up to it.
    pub cost: String,
    pub session_cost: String,
}

impl ExchangeView {
    fn of(exchange: &Exchange, texts: &AskTexts) -> Self {
        let question = exchange.question.clone();
        let steps = exchange.steps.clone();
        let cost = format!("{} {}", texts.question_cost, (texts.cost)(&exchange.cost));
        let session_cost = format!(
            "{} {}",
            texts.session_cost,
            (texts.cost)(&exchange.session_cost)
        );
        match &exchange.outcome {
            Err(error) => Self {
                question,
                steps,
                error: Some(error.clone()),
                cost,
                session_cost,
                ..Self::default()
            },
            Ok(answer) => {
                let text = Some(answer.text.trim()).filter(|text| !text.is_empty());
                let text = text.unwrap_or(texts.no_answer);
                Self {
                    question,
                    steps,
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
                    cost,
                    session_cost,
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
    running: Option<RunningView>,
    /// The questions suggested to a conversation with no exchange (WAK-43); empty otherwise.
    suggestions: Vec<String>,
}

/// How many domains of the last run the suggested questions name (WAK-43).
const SUGGESTED_DOMAINS: usize = 3;

/// The questions suggested to an empty conversation: the purpose of the application, then the
/// first domains of the run, in the order of the docs (WAK-43).
fn suggestions(site: &Site) -> Vec<String> {
    let texts = &site.texts().ask;
    let domains = site.docs().map(GeneratedDocs::domains).unwrap_or_default();
    std::iter::once(texts.purpose_question.to_owned())
        .chain(
            domains
                .iter()
                .take(SUGGESTED_DOMAINS)
                .map(|domain| (texts.domain_question)(&domain.name)),
        )
        .collect()
}

/// A question running, as the page shows it.
pub struct RunningView {
    pub number: u64,
    pub question: String,
}

impl RunningView {
    fn of(running: &Running) -> Self {
        Self {
            number: running.number,
            question: running.question.clone(),
        }
    }
}

/// The exchange of a question running alone, added after the former ones by `htmx`.
#[derive(Template)]
#[template(path = "running.html")]
struct RunningFragment {
    t: &'static WebTexts,
    running: RunningView,
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
    let (exchanges, running) = state
        .sessions()
        .get(cookie(&headers).as_deref())
        .map(|session| {
            let exchanges: Vec<ExchangeView> = session
                .exchanges
                .iter()
                .map(|exchange| ExchangeView::of(exchange, &texts.ask))
                .collect();
            (exchanges, session.running.as_deref().map(RunningView::of))
        })
        .unwrap_or_default();
    let suggestions = if exchanges.is_empty() && running.is_none() {
        suggestions(&site)
    } else {
        Vec::new()
    };
    let page = AskPage {
        t: texts,
        shell: site.shell(),
        exchanges,
        running,
        suggestions,
    };
    render(StatusCode::OK, &page)
}

/// What the form sends; a field it leaves out is empty.
#[derive(Debug, Deserialize)]
struct QuestionForm {
    #[serde(default)]
    question: String,
}

/// Why a question was refused, put by `htmx` in place of the error slot of the form, beside the
/// question it keeps (WAK-40).
#[derive(Template)]
#[template(
    source = r#"<p class="form-error" id="ask-error" role="alert">{{ error }}</p>"#,
    ext = "html"
)]
struct AskError {
    error: &'static str,
}

/// A question refused with `code` because of `error`: the error alone to `htmx`, retargeted to
/// the slot of the form; the text otherwise.
fn refused(code: StatusCode, error: &'static str, htmx: bool) -> Response {
    if !htmx {
        return (code, error).into_response();
    }
    let mut response = render(code, &AskError { error });
    let headers = response.headers_mut();
    headers.insert("HX-Retarget", HeaderValue::from_static("#ask-error"));
    headers.insert("HX-Reswap", HeaderValue::from_static("outerHTML"));
    response
}

async fn question(
    State(state): State<AskState>,
    headers: HeaderMap,
    Form(form): Form<QuestionForm>,
) -> Response {
    let site: Arc<Site> = state.site.0.current();
    let htmx = headers.contains_key("HX-Request");
    let errors = &site.texts().errors;
    let question = form.question.trim().to_string();
    if question.is_empty() {
        return refused(StatusCode::BAD_REQUEST, errors.blank_question, htmx);
    }
    let Some(docs) = state.docs(&site) else {
        return refused(StatusCode::NOT_FOUND, errors.no_docs, htmx);
    };
    // The question runs in a task of its own, so the page and the other sessions are served while
    // the model answers; it runs to its end whether a page follows it or not (WAK-35).
    let (id, new, running, conversation) = {
        let mut sessions = state.sessions();
        let (id, new) = sessions.open(cookie(&headers).as_deref());
        let Some(session) = sessions.get(Some(&id)) else {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        };
        // Two questions asked from the same history would each drop the other from it.
        if session.running.is_some() {
            return refused(StatusCode::CONFLICT, errors.question_running, htmx);
        }
        session.asked += 1;
        let running = Arc::new(Running {
            number: session.asked,
            question,
            progress: watch::Sender::new(Progress::default()),
        });
        session.running = Some(Arc::clone(&running));
        (id, new, running, session.conversation.clone())
    };
    let mut progress = running.progress.subscribe();
    let started = state.in_flight.start();
    tokio::spawn(run(
        started,
        state,
        id.clone(),
        Arc::clone(&running),
        conversation,
        docs,
    ));
    let response = if htmx {
        render(
            StatusCode::OK,
            &RunningFragment {
                t: site.texts(),
                running: RunningView::of(&running),
            },
        )
    } else {
        drop(running);
        let _ = ended_exchange(&mut progress).await;
        (StatusCode::SEE_OTHER, [(header::LOCATION, "/ask")]).into_response()
    };
    with_cookie(response, &id, new)
}

/// Asks the question `running` of the session `id` after its `conversation`, telling its progress
/// each step, then keeps the exchange and the conversation ([`Free::keep`]).
async fn run(
    _started: Started,
    state: AskState,
    id: String,
    running: Arc<Running>,
    mut conversation: Conversation,
    docs: Arc<Docs>,
) {
    let mut free = Free {
        sessions: Arc::clone(&state.sessions),
        id,
        running: Arc::clone(&running),
        counted: UsageTracker::new(),
    };
    let progress = &running.progress;
    let tell = |step: Step| progress.send_modify(|p| p.steps.push(step));
    // The calls of the question alone, counted as the recap counts them (WAK-37).
    let llm = UsageProvider::new(Arc::clone(&state.asking.llm), free.counted.clone());
    let outcome = conversation
        .ask_observed(
            &llm,
            docs,
            &running.question,
            state.asking.max_rounds,
            &tell,
        )
        .await
        .map_err(|error| error.to_string());
    free.keep(outcome, Some(conversation));
}

/// The question of a session running until its task ends: dropped before [`Free::keep`] (a panic,
/// the shutdown of the server), it keeps an exchange that says the question stopped (WAK-38).
struct Free {
    sessions: Arc<Mutex<Sessions>>,
    id: String,
    running: Arc<Running>,
    /// The calls of the question.
    counted: UsageTracker,
}

/// The error of a question whose task stopped without an answer.
const STOPPED: &str = "the question stopped before its answer";

impl Free {
    /// Ends the question with `outcome`: its exchange goes to its session, with `conversation`
    /// when given, the session started again under its id when it was dropped meanwhile (WAK-35);
    /// a session reset meanwhile runs it no longer and keeps neither. Then the followers get the
    /// exchange.
    fn keep(&mut self, outcome: Result<Answer, String>, conversation: Option<Conversation>) {
        let running = &self.running;
        let cost = self.counted.report().total();
        let mut exchange = Exchange {
            number: running.number,
            question: running.question.clone(),
            steps: running.progress.borrow().steps.clone(),
            outcome,
            cost,
            session_cost: cost,
        };
        let exchange = {
            let mut sessions = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
            let session = match sessions.get(Some(&self.id)) {
                Some(session) => session,
                None => sessions.reopen(&self.id, running),
            };
            let runs = session
                .running
                .as_ref()
                .is_some_and(|kept| Arc::ptr_eq(kept, running));
            if runs {
                session.spent.add(&cost);
                exchange.session_cost = session.spent;
                session.running = None;
                if let Some(conversation) = conversation {
                    session.conversation = conversation;
                }
                let exchange = Arc::new(exchange);
                session.exchanges.push_back(Arc::clone(&exchange));
                while session.exchanges.len() > MAX_EXCHANGES {
                    session.exchanges.pop_front();
                }
                exchange
            } else {
                Arc::new(exchange)
            }
        };
        running.progress.send_modify(|p| p.ended = Some(exchange));
    }
}

impl Drop for Free {
    fn drop(&mut self) {
        if self.running.progress.borrow().ended.is_none() {
            self.keep(Err(STOPPED.to_string()), None);
        }
    }
}

/// The exchange of the question `progress` follows, once it has ended; `None` when its task
/// stopped without one.
async fn ended_exchange(progress: &mut watch::Receiver<Progress>) -> Option<Arc<Exchange>> {
    let ended = progress.wait_for(|p| p.ended.is_some()).await.ok()?;
    ended.ended.clone()
}

/// The exchange of the question `number` alone, once it has ended: the fallback of a running
/// exchange whose stream failed (WAK-33).
async fn ended(
    State(state): State<AskState>,
    Path(number): Path<u64>,
    headers: HeaderMap,
) -> Response {
    let Some(mut progress) = state.follow(&headers, number) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some(exchange) = ended_exchange(&mut progress).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let texts = state.site.0.current().texts();
    render(
        StatusCode::OK,
        &ExchangeFragment {
            t: texts,
            exchange: ExchangeView::of(&exchange, &texts.ask),
        },
    )
}

/// The steps of the question `number`, from the first, then its exchange, as server-sent events
/// (WAK-31).
async fn events(
    State(state): State<AskState>,
    Path(number): Path<u64>,
    headers: HeaderMap,
) -> Response {
    let Some(progress) = state.follow(&headers, number) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let texts = state.site.0.current().texts();
    let stream = futures_util::stream::unfold(
        (progress, 0, false),
        move |(mut progress, sent, done)| async move {
            if done {
                return None;
            }
            loop {
                {
                    let now = progress.borrow_and_update();
                    if let Some(step) = now.steps.get(sent) {
                        let event = Ok::<_, Infallible>(event("step", &StepView { step }));
                        drop(now);
                        return Some((event, (progress, sent + 1, false)));
                    }
                    if let Some(exchange) = now.ended.clone() {
                        drop(now);
                        let exchange = ExchangeFragment {
                            t: texts,
                            exchange: ExchangeView::of(&exchange, &texts.ask),
                        };
                        let event = Ok(event("answer", &exchange));
                        return Some((event, (progress, sent, true)));
                    }
                }
                // The task stopped without an exchange: the stream ends, the page falls back.
                progress.changed().await.ok()?;
            }
        },
    );
    // A comment every 15 seconds keeps the connection alive while the model answers (WAK-39).
    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

/// The event `name` whose data is `template` rendered, without the carriage returns an event
/// cannot carry.
fn event(name: &str, template: &impl Template) -> Event {
    let html = template.render().unwrap_or_default().replace('\r', "");
    Event::default().event(name).data(html.trim())
}

/// Empties the exchanges and the conversation of the session (WAK-22).
async fn reset(State(state): State<AskState>, headers: HeaderMap) -> Response {
    if let Some(session) = state.sessions().get(cookie(&headers).as_deref()) {
        // A question still running ends outside of the session (`run`); the count goes on.
        *session = Session {
            asked: session.asked,
            ..Session::default()
        };
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
