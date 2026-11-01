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
    if let Some(warning) = exposure_warning(bind) {
        tracing::warn!("{warning}");
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

/// What to tell the user when `bind` is reachable from other machines: the
/// server has no authentication.
fn exposure_warning(bind: SocketAddr) -> Option<String> {
    (!bind.ip().is_loopback()).then(|| {
        format!(
            "{bind} is not a loopback address: the docs are reachable from other machines, with no authentication"
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: WEB-14 (internal: listening on every interface in a test would expose the machine)
    #[test]
    fn a_bind_address_other_than_loopback_is_warned_of() {
        assert!(exposure_warning("127.0.0.1:8730".parse().unwrap()).is_none());
        assert!(exposure_warning("[::1]:8730".parse().unwrap()).is_none());
        let warning = exposure_warning("0.0.0.0:8730".parse().unwrap()).unwrap();
        assert!(warning.contains("other machines"), "{warning}");
    }
}
