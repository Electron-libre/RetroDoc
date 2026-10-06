use std::path::Path;

use super::docs::load_docs;
use super::workspace::Workspace;

/// Serves the generated docs to an MCP client over stdin/stdout until it
/// disconnects; makes no LLM call. Stdout carries the protocol, so the logs
/// of this command go to stderr (see `main`).
pub async fn run(path: &Path) -> anyhow::Result<()> {
    let workspace = Workspace::open(path)?;
    let docs = load_docs(&workspace)?;
    tracing::info!(
        entries = docs.index().len(),
        "serving the docs over MCP (stdio)"
    );
    retrodoc_mcp::serve_stdio(docs).await?;
    Ok(())
}
