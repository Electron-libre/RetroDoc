use std::path::Path;

use anyhow::Context;
use retrodoc_pipeline::DomainMap;

/// Prints the documentation debt report from the artifacts saved by the last
/// `generate` run (`.retrodoc/cache/`); makes no LLM call.
pub fn run(path: &Path) -> anyhow::Result<()> {
    let repo_root = path
        .canonicalize()
        .with_context(|| format!("path not found: {}", path.display()))?;

    let (Some(features), Some(use_cases)) = (
        retrodoc_pipeline::load_features(&repo_root),
        retrodoc_pipeline::load_use_cases(&repo_root),
    ) else {
        anyhow::bail!("no features/use cases found — run `retrodoc generate` first");
    };
    if use_cases.iter().all(|u| u.confidence.is_none()) {
        anyhow::bail!("no confidence scores found — run `retrodoc generate` again");
    }
    let domains = DomainMap::load(&repo_root).unwrap_or_default();

    let report = retrodoc_pipeline::build_report(&domains, &features, &use_cases);
    print!("{}", report.to_markdown());
    Ok(())
}
