//! The MCP server: [`Docs`] behind the `rmcp` SDK, one tool per
//! [`Docs`] function. Read-only and LLM-free, it answers over stdio for the
//! agent that launched it (`retrodoc mcp`).

use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_handler, tool_router, ServerHandler, ServiceExt};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::source::DEFAULT_LOG;
use crate::tools::Docs;

/// Hits returned by `search_docs` when the agent gives no limit, and the most
/// it can ask for.
const DEFAULT_LIMIT: usize = 5;
const MAX_LIMIT: usize = 20;

#[derive(Debug, thiserror::Error)]
pub enum McpError {
    #[error("could not start the MCP server: {0}")]
    Start(String),
    #[error("the MCP server stopped abnormally: {0}")]
    Stopped(String),
}

#[derive(Clone)]
pub struct DocsServer {
    docs: Arc<Docs>,
}

#[derive(Deserialize, JsonSchema)]
struct IdParams {
    /// The id as shown by the other tools (e.g. `billing/pay-invoice`).
    id: String,
}

#[derive(Deserialize, JsonSchema)]
struct SourceParams {
    /// A file cited by the docs, relative to the repository root (e.g. `app/invoice.rb`).
    path: String,
    /// First line to show, 1-based (default 1).
    start_line: Option<usize>,
    /// Last line to show, inclusive (default: 200 lines from `start_line`, at most 400).
    end_line: Option<usize>,
}

#[derive(Deserialize, JsonSchema)]
struct LogParams {
    /// A file cited by the docs, relative to the repository root.
    path: String,
    /// Maximum number of commits (default 10, at most 50).
    limit: Option<usize>,
}

#[derive(Deserialize, JsonSchema)]
struct SearchParams {
    /// What to look for, in the application's own words.
    query: String,
    /// Maximum number of results (default 5, at most 20).
    limit: Option<usize>,
}

impl DocsServer {
    #[must_use]
    pub fn new(docs: Docs) -> Self {
        Self {
            docs: Arc::new(docs),
        }
    }
}

/// Runs file and git work off the async threads, so that a slow history walk
/// doesn't keep the server from answering other requests.
async fn blocking(work: impl FnOnce() -> String + Send + 'static) -> String {
    tokio::task::spawn_blocking(work)
        .await
        .unwrap_or_else(|_| "Not available: the request could not be completed.\n".to_string())
}

#[tool_router]
impl DocsServer {
    #[tool(
        description = "List the functional domains of the application (and their sub-domains) \
                       with a description and a confidence score. Start here to find your way."
    )]
    async fn list_domains(&self) -> String {
        self.docs.list_domains()
    }

    #[tool(
        description = "Show one domain or sub-domain (e.g. `billing`, `billing/payment`) and \
                       its features."
    )]
    async fn get_domain(&self, Parameters(IdParams { id }): Parameters<IdParams>) -> String {
        self.docs.get_domain(&id)
    }

    #[tool(
        description = "Show one feature (e.g. `billing/pay-invoice`): what it does, the files it \
                       rests on and its use cases."
    )]
    async fn get_feature(&self, Parameters(IdParams { id }): Parameters<IdParams>) -> String {
        self.docs.get_feature(&id)
    }

    #[tool(
        description = "Show one use case (e.g. `billing/pay-invoice/pay-by-card`): who does what \
                       and why, the steps, and the code each step cites. A low confidence means \
                       the claim is not backed by the code: verify before relying on it."
    )]
    async fn get_use_case(&self, Parameters(IdParams { id }): Parameters<IdParams>) -> String {
        self.docs.get_use_case(&id)
    }

    #[tool(
        description = "Show lines of a source file that the docs cite, with line numbers, to \
                       check a claim against the code. Only cited files can be read; use the \
                       paths given by get_feature and get_use_case."
    )]
    async fn read_source(
        &self,
        Parameters(SourceParams {
            path,
            start_line,
            end_line,
        }): Parameters<SourceParams>,
    ) -> String {
        let docs = Arc::clone(&self.docs);
        blocking(move || docs.read_source(&path, start_line, end_line)).await
    }

    #[tool(
        description = "Show the recent commits (hash, date, subject) that changed a source file \
                       the docs cite, to see how and why a behavior evolved. Only cited files."
    )]
    async fn git_log(
        &self,
        Parameters(LogParams { path, limit }): Parameters<LogParams>,
    ) -> String {
        let docs = Arc::clone(&self.docs);
        blocking(move || docs.git_log(&path, limit.unwrap_or(DEFAULT_LOG))).await
    }

    #[tool(
        description = "Search the generated documentation and the project's own docs with the \
                       words of a question. Answers `Not documented` when nothing matches: say so \
                       rather than guess."
    )]
    async fn search_docs(
        &self,
        Parameters(SearchParams { query, limit }): Parameters<SearchParams>,
    ) -> String {
        let limit = limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
        self.docs.search_docs(&query, limit)
    }
}

// The code the macro generates is async without awaiting.
#[allow(clippy::unused_async_trait_impl)]
#[tool_handler(
    name = "retrodoc",
    instructions = "Functional documentation of an application, generated from its code and \
                    history. Browse with list_domains, get_domain, get_feature and get_use_case, \
                    or use search_docs. Each answer carries a confidence score and the files it \
                    cites, and read_source and git_log show that code and its history; \"Not documented\" means the docs say nothing about it."
)]
impl ServerHandler for DocsServer {}

/// Serves `docs` on stdin/stdout until the client closes the connection.
/// Nothing else may write to stdout meanwhile: it carries the protocol.
///
/// # Errors
///
/// Returns an error if the handshake with the client fails or the
/// connection breaks.
pub async fn serve_stdio(docs: Docs) -> Result<(), McpError> {
    let running = DocsServer::new(docs)
        .serve(rmcp::transport::stdio())
        .await
        .map_err(|e| McpError::Start(e.to_string()))?;
    running
        .waiting()
        .await
        .map(|_| ())
        .map_err(|e| McpError::Stopped(e.to_string()))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use retrodoc_pipeline::Glossary;
    use rmcp::model::CallToolRequestParams;
    use serde_json::json;

    use super::*;
    use crate::corpus::fixtures::*;

    /// A client connected to the server through an in-memory pipe.
    async fn connect() -> rmcp::service::RunningService<rmcp::RoleClient, ()> {
        let docs = Docs::new(
            vec![domain("billing", "Billing", "Invoices and payments")],
            vec![feature(
                "pay-invoice",
                "Pay an invoice",
                "Settle what is due",
            )],
            vec![use_case(
                "pay-by-card",
                "pay-invoice",
                "Pay by card",
                "The accountant settles it",
            )],
            &Glossary::default(),
            &[],
        );
        let (server_end, client_end) = tokio::io::duplex(8192);
        tokio::spawn(async move {
            if let Ok(running) = DocsServer::new(docs).serve(server_end).await {
                let _ = running.waiting().await;
            }
        });
        ().serve(client_end).await.unwrap()
    }

    async fn call(
        client: &rmcp::service::RunningService<rmcp::RoleClient, ()>,
        tool: &'static str,
        arguments: serde_json::Value,
    ) -> String {
        let mut params = CallToolRequestParams::new(tool);
        if let Some(arguments) = arguments.as_object() {
            params = params.with_arguments(arguments.clone());
        }
        let result = client.call_tool(params).await.unwrap();
        result
            .content
            .first()
            .and_then(|c| c.as_text())
            .map(|t| t.text.clone())
            .expect("a text answer")
    }

    #[tokio::test]
    async fn the_client_discovers_the_seven_tools_and_server_identity() {
        let client = connect().await;
        let tools = client.list_all_tools().await.unwrap();
        let names: BTreeSet<String> = tools.iter().map(|t| t.name.to_string()).collect();
        assert_eq!(
            names,
            BTreeSet::from(
                [
                    "list_domains",
                    "get_domain",
                    "get_feature",
                    "get_use_case",
                    "search_docs",
                    "read_source",
                    "git_log"
                ]
                .map(String::from)
            )
        );
        let info = client.peer_info().unwrap();
        assert_eq!(info.server_info.as_ref().unwrap().name, "retrodoc");
        client.cancel().await.unwrap();
    }

    #[tokio::test]
    async fn the_tools_answer_from_the_docs() {
        let client = connect().await;
        assert!(call(&client, "list_domains", json!({}))
            .await
            .contains("`billing`"));
        assert!(call(&client, "get_domain", json!({"id": "billing"}))
            .await
            .contains("`billing/pay-invoice`"));
        assert!(
            call(&client, "get_feature", json!({"id": "billing/pay-invoice"}))
                .await
                .contains("`billing/pay-invoice/pay-by-card`")
        );
        assert!(call(
            &client,
            "get_use_case",
            json!({"id": "billing/pay-invoice/pay-by-card"})
        )
        .await
        .contains("Confidence: 90%"));
        assert!(call(
            &client,
            "search_docs",
            json!({"query": "settle invoice", "limit": 3})
        )
        .await
        .contains("Pay an invoice"));
        client.cancel().await.unwrap();
    }

    #[tokio::test]
    async fn the_code_tools_are_wired_and_refuse_without_repository_access() {
        let client = connect().await;
        let read = call(
            &client,
            "read_source",
            json!({"path": "app/invoice.rb", "start_line": 1}),
        )
        .await;
        assert!(read.starts_with("Not available:"), "{read}");
        let log = call(&client, "git_log", json!({"path": "app/invoice.rb"})).await;
        assert!(log.starts_with("Not available:"), "{log}");
        client.cancel().await.unwrap();
    }

    #[tokio::test]
    async fn an_unknown_id_or_empty_search_is_a_normal_not_documented_answer() {
        let client = connect().await;
        assert!(call(&client, "get_domain", json!({"id": "shipping"}))
            .await
            .starts_with("Not documented"));
        assert!(call(&client, "search_docs", json!({"query": "zzzz"}))
            .await
            .starts_with("Not documented"));
        client.cancel().await.unwrap();
    }
}
