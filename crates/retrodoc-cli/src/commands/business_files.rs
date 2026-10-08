use std::path::Path;

use anyhow::Context;
use retrodoc_ingest::IngestResult;
use retrodoc_llm::{LlmProvider, UsageTracker};
use retrodoc_pipeline::{Artifact, BusinessMap, ProductBrief, RoleRules};

use super::workspace::Workspace;

/// Locates the business files (one LLM call, or the saved
/// `.retrodoc/cache/business-files.yaml`) and prints them with their
/// reasons. The stack comes from the saved role rules when there are some,
/// the brief from the saved one. `force` asks again, edits included.
pub async fn run(path: &Path, force: bool, tracker: &UsageTracker) -> anyhow::Result<()> {
    let workspace = Workspace::open(path)?;
    let repo_root = &workspace.repo_root;
    let config = &workspace.config;
    let mut ingest = workspace.ingest()?;
    ingest.existing_docs = super::docs::without_generated(ingest.existing_docs, config);
    let rules = RoleRules::load(repo_root);
    if let Some(rules) = &rules {
        rules.promote_sources(&mut ingest.files);
    }
    let stack = rules.map(|r| r.stack).unwrap_or_default();

    let llm = super::usage::provider(&config.llm, tracker)?;
    let brief = super::brief::saved(repo_root);
    let map = locate(repo_root, &ingest, &stack, &brief, &llm, force, tracker).await?;
    if map.is_empty() {
        anyhow::bail!(
            "no business file located (the LLM answer was unusable, see the warnings above) — try again"
        );
    }
    println!("\nBusiness files, most important first (editable):");
    for entry in &map.entries {
        println!("  {}  {}", entry.path, entry.reason);
    }
    println!(
        "\n{} file(s) in all, saved to {}.",
        map.files(&ingest.files).len(),
        Artifact::BusinessFiles.relative_path()
    );
    Ok(())
}

/// The pass as `generate` and the standalone command run it: counted under
/// its own name in the usage recap. An empty map is a warning, not an error:
/// the passes that read it go on without.
pub async fn locate(
    repo_root: &Path,
    ingest: &IngestResult,
    stack: &str,
    brief: &ProductBrief,
    llm: &dyn LlmProvider,
    force: bool,
    tracker: &UsageTracker,
) -> anyhow::Result<BusinessMap> {
    println!("Locating the business files…");
    let map = tracker
        .in_pass(
            "business-files",
            retrodoc_pipeline::infer_business_files(repo_root, ingest, stack, brief, llm, force),
        )
        .await
        .context("failed to locate the business files")?;
    if map.is_empty() {
        tracing::warn!("no business file located");
    } else {
        println!(
            "{} business file(s) or folder(s) located.",
            map.entries.len()
        );
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use retrodoc_ingest::{FileEntry, FileKind};
    use retrodoc_llm::{CompletionRequest, CompletionResponse, LlmError};

    use super::*;

    struct Counting(AtomicUsize);

    #[async_trait::async_trait]
    impl LlmProvider for Counting {
        async fn complete(&self, _: CompletionRequest) -> Result<CompletionResponse, LlmError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(CompletionResponse {
                content: r#"{"business":[{"path":"lib/order.rb","reason":"an order"}]}"#
                    .to_string(),
                model: "test".to_string(),
                usage: None,
            })
        }
    }

    #[tokio::test]
    async fn the_second_run_reads_the_saved_file_and_calls_nothing() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("lib")).unwrap();
        std::fs::write(dir.path().join("lib/order.rb"), "class Order; end").unwrap();
        let ingest = IngestResult {
            files: vec![FileEntry {
                path: PathBuf::from("lib/order.rb"),
                kind: FileKind::Source,
                size_bytes: 16,
            }],
            history_by_path: std::collections::HashMap::new(),
            existing_docs: Vec::new(),
            commits: Vec::new(),
        };
        let llm = Counting(AtomicUsize::new(0));
        let tracker = UsageTracker::new();
        let brief = ProductBrief::default();

        let first = locate(dir.path(), &ingest, "Ruby", &brief, &llm, false, &tracker)
            .await
            .unwrap();
        let second = locate(dir.path(), &ingest, "Ruby", &brief, &llm, false, &tracker)
            .await
            .unwrap();

        assert_eq!(llm.0.load(Ordering::SeqCst), 1);
        assert_eq!(first, second);
        assert_eq!(first.entries[0].path, "lib/order.rb");
    }
}
