use std::path::Path;

use retrodoc_pipeline::DomainMap;

use super::workspace::repo_root;

/// Prints the documentation debt report from the artifacts saved by the last
/// `generate` run (`.retrodoc/cache/`); makes no LLM call.
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

    let mut report = retrodoc_pipeline::build_report(&domains, &features, &use_cases);
    report.skipped_files = retrodoc_pipeline::Scope::load(&repo_root)
        .map(|s| s.skipped)
        .unwrap_or_default();
    report.with_behaviour_coverage(retrodoc_pipeline::BehaviourCoverage::load(&repo_root));
    super::docs::add_rule_debt(&mut report, &repo_root, &config, &features);
    print!("{}", report.to_markdown());
    Ok(())
}
