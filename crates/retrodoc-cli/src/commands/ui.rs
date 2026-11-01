use std::net::SocketAddr;
use std::path::Path;

use anyhow::Context;
use retrodoc_mcp::GeneratedDocs;

use super::docs::project_docs;
use super::workspace::Workspace;

/// Where `retrodoc ui` listens without `--bind`: this machine only.
pub const DEFAULT_BIND: &str = "127.0.0.1:8730";

/// Serves the generated docs as a web interface on `bind` until stopped;
/// makes no LLM call and writes nothing. The docs are read once, here: a
/// repository with no run yet gets a page saying to run `generate`.
pub async fn run(path: &Path, bind: SocketAddr) -> anyhow::Result<()> {
    let workspace = Workspace::open(path)?;
    let docs = project_docs(&workspace.repo_root, &workspace.config)?;
    let generated = GeneratedDocs::load(&workspace.repo_root, &docs);
    if generated.is_none() {
        tracing::warn!("no features found — run `retrodoc generate`, then restart `retrodoc ui`");
    }
    let listener = tokio::net::TcpListener::bind(bind)
        .await
        .with_context(|| format!("could not listen on {bind}"))?;
    let address = listener.local_addr()?;
    println!("Serving the docs at http://{address}/ (Ctrl+C to stop)");
    retrodoc_web::serve(listener, generated)
        .await
        .context("the web server stopped")
}
