use std::net::SocketAddr;
use std::path::Path;

use anyhow::Context;
use retrodoc_mcp::GeneratedDocs;
use retrodoc_web::Project;

use super::brief;
use super::docs::{item_doc_links, project_docs};
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
    let links = item_doc_links(&workspace.repo_root, &workspace.config, &docs);
    let project = project(&workspace);
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
    retrodoc_web::serve(listener, project, generated, links, docs)
        .await
        .context("the web server stopped")
}

/// The application the pages name, with the purpose of the saved product
/// brief (none, or one that can't be read and is warned of: no purpose).
fn project(workspace: &Workspace) -> Project {
    Project {
        name: workspace
            .config
            .project
            .name_or_directory(&workspace.repo_root),
        purpose: Some(brief::saved(&workspace.repo_root).purpose.text)
            .filter(|purpose| !purpose.trim().is_empty()),
    }
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

    // spec: WEB-62, WEB-64 (internal: `run` serves until stopped)
    #[test]
    fn the_project_is_named_by_the_config_and_carries_the_saved_purpose() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("shop");
        std::fs::create_dir_all(root.join(".retrodoc/cache")).unwrap();
        std::fs::write(root.join("retrodoc.toml"), "").unwrap();
        let unnamed = project(&Workspace::open(&root).unwrap());
        assert_eq!(unnamed.name, "shop");
        assert_eq!(unnamed.purpose, None);

        std::fs::write(
            root.join("retrodoc.toml"),
            "[project]\nname = \"Shop and Co\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join(".retrodoc/cache/product.yaml"),
            "purpose:\n  text: Sells goods online\n",
        )
        .unwrap();
        let named = project(&Workspace::open(&root).unwrap());
        assert_eq!(named.name, "Shop and Co");
        assert_eq!(named.purpose.as_deref(), Some("Sells goods online"));
    }

    // spec: WEB-14 (internal: listening on every interface in a test would expose the machine)
    #[test]
    fn a_bind_address_other_than_loopback_is_warned_of() {
        assert!(exposure_warning("127.0.0.1:8730".parse().unwrap()).is_none());
        assert!(exposure_warning("[::1]:8730".parse().unwrap()).is_none());
        let warning = exposure_warning("0.0.0.0:8730".parse().unwrap()).unwrap();
        assert!(warning.contains("other machines"), "{warning}");
    }
}
