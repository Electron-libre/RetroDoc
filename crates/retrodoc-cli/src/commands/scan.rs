use std::path::Path;

use retrodoc_ingest::FileKind;

use super::workspace::Workspace;

pub fn run(path: &Path) -> anyhow::Result<()> {
    let workspace = Workspace::open(path)?;
    let repo_root = &workspace.repo_root;

    let result = workspace.ingest()?;

    let source_count = result
        .files
        .iter()
        .filter(|f| f.kind == FileKind::Source)
        .count();
    let markdown_count = result
        .files
        .iter()
        .filter(|f| f.kind == FileKind::Markdown)
        .count();
    let test_count = result
        .files
        .iter()
        .filter(|f| f.kind == FileKind::Test)
        .count();
    let other_count = result.files.len() - source_count - markdown_count - test_count;

    println!("Repo: {}", repo_root.display());
    println!(
        "Files: {} (source: {source_count}, test: {test_count}, markdown: {markdown_count}, other: {other_count})",
        result.files.len()
    );
    println!(
        "Git history: {} file(s) with at least one commit",
        result.history_by_path.len()
    );
    println!(
        "Existing Markdown docs taken as input: {}",
        result.existing_docs.len()
    );
    for doc in &result.existing_docs {
        println!("  - {}", doc.path.display());
    }

    Ok(())
}
