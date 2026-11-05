//! The agent of `retrodoc ask` (ADR 0047, `specs/ask.md`): a model answers a question about the
//! application by navigating the generated docs with the read-only tools of the MCP server, in a
//! bounded number of rounds. Nothing here writes; the tools answer from [`Docs`].

use std::fmt::Write as _;
use std::sync::Arc;

use retrodoc_llm::{
    ChatMessage, CompletionRequest, LlmError, LlmProvider, Role, ToolCall, ToolSpec,
};
use retrodoc_mcp::{read_tools, Docs, Source, Warning};

/// Rounds of tool calls before the agent asks for the answer without tools, unless set otherwise:
/// enough to go from a search to a use case and its code, few enough for a local model's context.
pub const DEFAULT_MAX_ROUNDS: usize = 8;

const SYSTEM_PROMPT_START: &str = "You answer questions about how an application works, from its \
functional documentation, which you read with the tools. Find your way with search_docs or \
list_domains, then read the features and use cases with get_feature and get_use_case. ";

/// The sentence of the system prompt that sends the model to the code tools, left out without them.
const CODE_TOOLS_HINT: &str = "Go down to \
the code with read_source and git_log only when the question is technical, or when the docs have \
a low confidence or say the code changed since they were written. ";

const SYSTEM_PROMPT_END: &str = "Answer from what the tools \
showed, never from what you assume. Name each domain, feature, use case and file you rely on by \
its id or path in backticks, as the tools show it (e.g. `billing/pay-invoice`, `app/invoice.rb`). \
When the tools show nothing relevant, answer \"Not documented\" and say what you looked for. \
Answer in the language of the question.";

/// Calls of one round answered by their tool; the next ones get an error (ASK-23).
pub const MAX_CALLS_PER_ROUND: usize = 6;

/// Characters of the tool results of a question, errors left out, past which a result is refused
/// (ASK-24): about 15k tokens, half the 32k context of the local model, leaving room for the
/// prompts, the history and the answer.
pub const MAX_RESULT_CHARS: usize = 60_000;

const LAST_ROUND: &str =
    "You have used all your tool calls. Answer the question now from what the \
tools showed, naming the ids and paths you rely on in backticks; answer \"Not documented\" if \
they showed nothing relevant.";

/// The tools that read the code and its history, which [`Tools::DocsOnly`] leaves out.
const CODE_TOOLS: [&str; 2] = ["read_source", "git_log"];

/// The tools offered to the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tools {
    /// The seven read-only tools of the MCP server.
    #[default]
    All,
    /// The tools of the docs only, without `read_source` and `git_log`: to measure what reading the
    /// code brings (`issues/done/ask_question_set.md`).
    DocsOnly,
}

impl Tools {
    fn offers(self, name: &str) -> bool {
        self == Tools::All || !CODE_TOOLS.contains(&name)
    }

    fn system_prompt(self) -> String {
        let hint = if self == Tools::All {
            CODE_TOOLS_HINT
        } else {
            ""
        };
        format!("{SYSTEM_PROMPT_START}{hint}{SYSTEM_PROMPT_END}")
    }
}

/// What the agent answered, with the tool results it read to answer.
#[derive(Debug, Clone, Default)]
pub struct Answer {
    /// The text of the model's answer.
    pub text: String,
    /// The text of each tool result of the question, in order.
    pub tool_results: Vec<String>,
    /// The rounds that held tool calls.
    pub rounds: usize,
    /// What the answer rests on, checked against the tool results.
    pub sources: Vec<Source>,
    /// What the sources warn of: low confidence, code changed since the docs.
    pub warnings: Vec<Warning>,
}

/// The note of an answer with no source.
pub const NO_SOURCE: &str = "No source of the docs backs this answer.";

/// The text shown for an answer whose text is blank.
pub const NO_ANSWER: &str = "The model gave no answer.";

impl Answer {
    /// The answer as the user reads it: its text, then its sources, then their warnings.
    #[must_use]
    pub fn render(&self) -> String {
        let text = self.text.trim_end();
        let mut out = format!("{}\n", if text.is_empty() { NO_ANSWER } else { text });
        if self.sources.is_empty() {
            let _ = write!(out, "\n{NO_SOURCE}\n");
        } else {
            out.push_str("\nSources:\n");
            for source in &self.sources {
                let _ = writeln!(out, "- {source}");
            }
        }
        if !self.warnings.is_empty() {
            out.push_str("\nWarnings:\n");
            for warning in &self.warnings {
                let _ = writeln!(out, "- {warning}");
            }
        }
        out
    }
}

/// The names the text gives in code spans that resolve in `docs` and whose full id or path a tool
/// result showed, in the order of the text, each once.
fn checked_sources(text: &str, tool_results: &[String], docs: &Docs) -> Vec<Source> {
    let shown: Vec<&str> = tool_results.iter().filter_map(|r| shown(r)).collect();
    let mut sources: Vec<Source> = Vec::new();
    for name in code_spans(text) {
        let Some(source) = docs.source_named(name) else {
            continue;
        };
        if shown.iter().any(|r| r.contains(&source.id)) && !sources.contains(&source) {
            sources.push(source);
        }
    }
    sources
}

/// What a tool result showed of the docs: nothing for an error, a "Not documented" or a "Not
/// available" answer, which repeat the call; a search without the query it repeats as its title.
fn shown(result: &str) -> Option<&str> {
    const NOTHING: [&str; 3] = ["Error:", "Not documented", "Not available"];
    if NOTHING.iter().any(|start| result.starts_with(start)) {
        return None;
    }
    Some(match result.split_once('\n') {
        Some((title, rest)) if title.starts_with("# Results for `") => rest,
        _ => result,
    })
}

/// What `sources` warn of; hashing the files they cite reads them, off the async threads.
async fn warnings_of(docs: &Arc<Docs>, sources: Vec<Source>) -> Vec<Warning> {
    let docs = Arc::clone(docs);
    tokio::task::spawn_blocking(move || docs.warnings_of(&sources))
        .await
        .unwrap_or_default()
}

/// The text of each inline code span of `text` that holds no line break, trimmed: a run of
/// backticks opens a span that only a run of as many closes (a span of two backticks may hold one,
/// a fenced block is skipped whole).
fn code_spans(text: &str) -> impl Iterator<Item = &str> {
    let mut rest = text;
    std::iter::from_fn(move || loop {
        let open = rest.find('`')?;
        let run = backticks_at(rest, open);
        let body = &rest[open + run..];
        let Some(close) = closing_run(body, run) else {
            rest = body;
            continue;
        };
        let span = body[..close].trim();
        rest = &body[close + run..];
        if !span.is_empty() && !span.contains('\n') {
            return Some(span);
        }
    })
}

/// The length of the run of backticks that starts at `at` in `text`.
fn backticks_at(text: &str, at: usize) -> usize {
    text[at..].len() - text[at..].trim_start_matches('`').len()
}

/// Where the next run of exactly `run` backticks starts in `text`.
fn closing_run(text: &str, run: usize) -> Option<usize> {
    let mut from = 0;
    while let Some(at) = text[from..].find('`') {
        let start = from + at;
        let length = backticks_at(text, start);
        if length == run {
            return Some(start);
        }
        from = start + length;
    }
    None
}

/// Asks `question` to `llm`, which answers with the read-only tools over `docs`, in at most
/// `max_rounds` rounds of tool calls, the calls and results bounded by [`MAX_CALLS_PER_ROUND`] and
/// [`MAX_RESULT_CHARS`]; then one last request, without tools nor tool history (the calls and
/// results folded into text), asks for the answer.
///
/// # Errors
///
/// Returns the error of a call to the LLM.
pub async fn ask(
    llm: &dyn LlmProvider,
    docs: Arc<Docs>,
    question: &str,
    max_rounds: usize,
) -> Result<Answer, LlmError> {
    ask_with(llm, docs, question, max_rounds, Tools::All).await
}

/// [`ask`] with the tools `offered`; a call to a tool not offered is answered as unknown.
///
/// # Errors
///
/// Returns the error of a call to the LLM.
pub async fn ask_with(
    llm: &dyn LlmProvider,
    docs: Arc<Docs>,
    question: &str,
    max_rounds: usize,
    offered: Tools,
) -> Result<Answer, LlmError> {
    let tools: Vec<ToolSpec> = tool_specs()
        .into_iter()
        .filter(|tool| offered.offers(&tool.name))
        .collect();
    let mut messages = vec![
        ChatMessage::new(Role::System, offered.system_prompt()),
        ChatMessage::new(Role::User, question),
    ];
    let mut answer = Answer::default();
    let mut read: Vec<(ToolCall, String)> = Vec::new();
    let mut used = 0;
    loop {
        let offer = answer.rounds < max_rounds;
        let request = if offer {
            CompletionRequest {
                messages: messages.clone(),
                tools: tools.clone(),
                ..Default::default()
            }
        } else {
            CompletionRequest {
                messages: vec![
                    messages[0].clone(),
                    messages[1].clone(),
                    ChatMessage::new(Role::User, folded(&read)),
                ],
                ..Default::default()
            }
        };
        let response = llm.complete(request).await?;
        if !offer || response.tool_calls.is_empty() {
            answer.sources = checked_sources(&response.content, &answer.tool_results, &docs);
            answer.warnings = warnings_of(&docs, answer.sources.clone()).await;
            answer.text = response.content;
            return Ok(answer);
        }
        answer.rounds += 1;
        messages.push(
            ChatMessage::assistant_calls(response.content, response.tool_calls.clone())
                .with_reasoning(response.reasoning),
        );
        for (index, call) in response.tool_calls.into_iter().enumerate() {
            // All the tools: the tool's own error names an unknown one. Some: say which are offered.
            let result = if index >= MAX_CALLS_PER_ROUND {
                format!(
                    "Error: at most {MAX_CALLS_PER_ROUND} calls per round are run; make fewer \
                     calls per round, or answer from what the tools showed."
                )
            } else if offered == Tools::All || tools.iter().any(|t| t.name == call.name) {
                within_budget(run_tool(&docs, &call).await, &mut used)
            } else {
                let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
                format!(
                    "Error: unknown tool `{}`; the tools are {}.",
                    call.name,
                    names.join(", ")
                )
            };
            messages.push(ChatMessage::tool_result(call.id.clone(), result.clone()));
            answer.tool_results.push(result.clone());
            read.push((call, result));
        }
    }
}

/// `result`, counted in the characters `used` by the question, or the error that refuses it when
/// it would bring them past [`MAX_RESULT_CHARS`]; an error result is not counted.
fn within_budget(result: String, used: &mut usize) -> String {
    if result.starts_with("Error:") {
        return result;
    }
    let size = result.chars().count();
    let left = MAX_RESULT_CHARS - *used;
    if size > left {
        return format!(
            "Error: this result holds {size} characters and {left} are left of the budget of \
             the question; narrow the call (a shorter range of lines, a precise id) or answer \
             from what the tools showed."
        );
    }
    *used += size;
    result
}

/// The last request's message: each call of the question and its result as text, then the
/// demand of the answer, so the history holds no tool call a server could refuse without tools.
fn folded(read: &[(ToolCall, String)]) -> String {
    let mut text = String::new();
    if !read.is_empty() {
        text.push_str("Your tool calls and what they returned:\n\n");
        for (call, result) in read {
            let _ = write!(
                text,
                "## `{}` `{}`\n\n{}\n\n",
                call.name,
                call.arguments,
                result.trim_end()
            );
        }
    }
    text.push_str(LAST_ROUND);
    text
}

/// The read-only tools of the MCP server as tools of a request.
fn tool_specs() -> Vec<ToolSpec> {
    read_tools()
        .into_iter()
        .map(|tool| ToolSpec {
            name: tool.name.to_string(),
            description: tool.description.to_string(),
            parameters: tool.parameters,
        })
        .collect()
}

/// The result of one call: the answer of the tool, or the error that kept it from answering, for
/// the model to correct its call. The tools read files and git, off the async threads.
async fn run_tool(docs: &Arc<Docs>, call: &ToolCall) -> String {
    let docs = Arc::clone(docs);
    let (name, arguments) = (call.name.clone(), call.arguments.clone());
    tokio::task::spawn_blocking(move || docs.call_read_tool(&name, &arguments))
        .await
        .map_or_else(
            |_| "Error: the tool could not complete.".to_string(),
            |result| result.unwrap_or_else(|error| format!("Error: {error}.")),
        )
}

#[cfg(test)]
mod tests;
