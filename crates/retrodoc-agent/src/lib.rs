//! The agent of `retrodoc ask` (ADR 0047, `specs/ask.md`): a model answers a question about the
//! application by navigating the generated docs with the read-only tools of the MCP server, in a
//! bounded number of rounds. Nothing here writes; the tools answer from [`Docs`].

use std::sync::Arc;

use retrodoc_llm::{
    ChatMessage, CompletionRequest, LlmError, LlmProvider, Role, ToolCall, ToolSpec,
};
use retrodoc_mcp::{read_tools, Docs};

/// Rounds of tool calls before the agent asks for the answer without tools, unless set otherwise:
/// enough to go from a search to a use case and its code, few enough for a local model's context.
pub const DEFAULT_MAX_ROUNDS: usize = 8;

const SYSTEM_PROMPT: &str = "You answer questions about how an application works, from its \
functional documentation, which you read with the tools. Find your way with search_docs or \
list_domains, then read the features and use cases with get_feature and get_use_case. Go down to \
the code with read_source and git_log only when the question is technical, or when the docs have \
a low confidence or say the code changed since they were written. Answer from what the tools \
showed, never from what you assume. Name each domain, feature, use case and file you rely on by \
its id or path in backticks, as the tools show it (e.g. `billing/pay-invoice`, `app/invoice.rb`). \
When the tools show nothing relevant, answer \"Not documented\" and say what you looked for. \
Answer in the language of the question.";

const LAST_ROUND: &str =
    "You have used all your tool calls. Answer the question now from what the \
tools showed, naming the ids and paths you rely on in backticks; answer \"Not documented\" if \
they showed nothing relevant.";

/// What the agent answered, with the tool results it read to answer.
#[derive(Debug, Clone, Default)]
pub struct Answer {
    /// The text of the model's answer.
    pub text: String,
    /// The text of each tool result of the question, in order.
    pub tool_results: Vec<String>,
    /// The rounds that held tool calls.
    pub rounds: usize,
}

/// Asks `question` to `llm`, which answers with the read-only tools over `docs`, in at most
/// `max_rounds` rounds of tool calls; then one last request without tools asks for the answer.
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
    let tools = tool_specs();
    let mut messages = vec![
        ChatMessage::new(Role::System, SYSTEM_PROMPT),
        ChatMessage::new(Role::User, question),
    ];
    let mut answer = Answer::default();
    loop {
        let offer = answer.rounds < max_rounds;
        if !offer {
            messages.push(ChatMessage::new(Role::User, LAST_ROUND));
        }
        let response = llm
            .complete(CompletionRequest {
                messages: messages.clone(),
                tools: if offer { tools.clone() } else { Vec::new() },
                ..Default::default()
            })
            .await?;
        if !offer || response.tool_calls.is_empty() {
            answer.text = response.content;
            return Ok(answer);
        }
        answer.rounds += 1;
        messages.push(ChatMessage::assistant_calls(
            response.content,
            response.tool_calls.clone(),
        ));
        for call in response.tool_calls {
            let result = run_tool(&docs, &call).await;
            messages.push(ChatMessage::tool_result(call.id, result.clone()));
            answer.tool_results.push(result);
        }
    }
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
