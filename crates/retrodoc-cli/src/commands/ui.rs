use std::net::SocketAddr;
use std::path::Path;

use anyhow::Context;
use retrodoc_llm::UsageTracker;
use retrodoc_mcp::GeneratedDocs;
use retrodoc_web::{Asking, Project};

use super::brief;
use super::docs::{item_doc_links, project_docs, NO_DOCS};
use super::workspace::Workspace;
use super::{passes, usage};

/// Where `retrodoc ui` listens without `--bind`: this machine only.
pub const DEFAULT_BIND: &str = "127.0.0.1:8730";

/// Serves the generated docs as a web interface on `bind` until Ctrl-C;
/// makes no LLM call, unless `ask` gives the rounds of the ask page
/// (`specs/web-ask.md`), and writes nothing but the reviews and edits its
/// forms send. The docs and the git history that ranks their debt are read
/// once, here, the docs rebuilt when the human edits saved beside them change:
/// a repository with no run yet gets a page saying to run `generate`, but
/// with `ask` it fails, as `ask` does, before it listens; so does a missing
/// key of the pass `ask`, whose calls `tracker` counts.
pub async fn run(
    path: &Path,
    bind: SocketAddr,
    ask: Option<usize>,
    tracker: &UsageTracker,
) -> anyhow::Result<()> {
    let workspace = Workspace::open(path)?;
    let docs = project_docs(&workspace.repo_root, &workspace.config)?;
    let generated = GeneratedDocs::load(&workspace.repo_root, &docs)
        .map(|generated| generated.reconciled(&docs).with_history());
    let asking = match ask {
        Some(max_rounds) => {
            anyhow::ensure!(generated.is_some(), NO_DOCS);
            let llm = usage::pass_provider(&workspace.config, tracker, passes::ASK)?;
            tracker.set_pass(passes::ASK);
            Some(Asking { llm, max_rounds })
        }
        None => None,
    };
    let links = item_doc_links(generated.as_ref(), &workspace.config);
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
    let interrupt = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    retrodoc_web::serve(listener, project, generated, links, docs, asking, interrupt)
        .await
        .context("the web server stopped")
}

/// The application the pages name, with the purpose and the users of the
/// saved product brief (none, or one that can't be read and is warned of:
/// neither).
fn project(workspace: &Workspace) -> Project {
    let brief = brief::saved(&workspace.repo_root);
    Project {
        name: workspace
            .config
            .project
            .name_or_directory(&workspace.repo_root),
        purpose: Some(brief.purpose.text).filter(|purpose| !purpose.trim().is_empty()),
        users: brief.users.into_iter().map(|user| user.text).collect(),
        lang: Project::lang_of(&workspace.config.output),
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

    // spec: WEB-62, WEB-64, WEB-65 (internal: `run` serves until stopped)
    #[test]
    fn the_project_is_named_by_the_config_and_carries_the_saved_purpose() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("shop");
        std::fs::create_dir_all(root.join(".retrodoc/cache")).unwrap();
        std::fs::write(root.join("retrodoc.toml"), "").unwrap();
        let unnamed = project(&Workspace::open(&root).unwrap());
        assert_eq!(unnamed.name, "shop");
        assert_eq!(unnamed.purpose, None);
        assert_eq!(unnamed.users, Vec::<String>::new());

        std::fs::write(
            root.join("retrodoc.toml"),
            "[project]\nname = \"Shop and Co\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join(".retrodoc/cache/product.yaml"),
            "purpose:\n  text: Sells goods online\nusers:\n  - text: Buyers\n",
        )
        .unwrap();
        let named = project(&Workspace::open(&root).unwrap());
        assert_eq!(named.name, "Shop and Co");
        assert_eq!(named.purpose.as_deref(), Some("Sells goods online"));
        assert_eq!(named.users, ["Buyers"]);
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
