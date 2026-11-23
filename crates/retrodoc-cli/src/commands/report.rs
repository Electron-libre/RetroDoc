use std::path::Path;

use super::docs::NO_DOCS;
use super::workspace::repo_root;

/// Prints the documentation debt report from the artifacts saved by the last
/// `generate` run (`.retrodoc/cache/`), the one `render` writes (SED-39); makes no LLM call.
pub fn run(path: &Path) -> anyhow::Result<()> {
    let repo_root = repo_root(path)?;
    // The report needs no `retrodoc.toml`: without one, the default document paths. A broken one
    // is an error, not a silent change of which docs are compared.
    let config = if repo_root
        .join(retrodoc_core::config::CONFIG_FILE_NAME)
        .exists()
    {
        retrodoc_core::config::Config::load(&repo_root)?
    } else {
        retrodoc_core::config::Config::default()
    };

    let docs = retrodoc_mcp::GeneratedDocs::load(&repo_root, &[])?;
    let Some((docs, use_cases)) = docs.zip(retrodoc_core::artifacts::load_use_cases(&repo_root))
    else {
        anyhow::bail!(NO_DOCS);
    };
    if use_cases.iter().all(|u| u.confidence.is_none()) {
        anyhow::bail!("no confidence scores found — run `retrodoc generate` again");
    }
    let report = super::docs::debt_report(&docs, &repo_root, &config);
    print!("{}", report.to_markdown());
    Ok(())
}
