use std::path::Path;

use super::docs::load_docs;
use super::workspace::Workspace;

/// Characters of the matched text shown under each hit.
const SNIPPET_CHARS: usize = 160;

/// Prints the entries (domains, features, use cases, glossary, docs) that
/// best match `query`, from the artifacts of the last `generate` run and the
/// collected docs; makes no LLM call. Meant to judge the retrieval quality
/// the MCP server will have.
pub fn run(path: &Path, query: &str, limit: usize) -> anyhow::Result<()> {
    let workspace = Workspace::open(path)?;
    let docs = load_docs(&workspace)?;
    let index = docs.index();

    let hits = index.search(query, limit);
    if hits.is_empty() {
        println!(
            "not documented: nothing among {} entries matches",
            index.len()
        );
        return Ok(());
    }
    for hit in hits {
        let entry = hit.entry;
        let confidence = entry
            .confidence
            .map_or_else(String::new, |c| format!(", confidence {c:.2}"));
        println!(
            "[{}] {} ({}{confidence}, score {:.2})",
            entry.kind.label(),
            entry.title,
            entry.id,
            hit.score
        );
        let snippet: String = entry
            .text
            .lines()
            .find(|l| *l != entry.title && !l.starts_with('#'))
            .unwrap_or_default()
            .chars()
            .take(SNIPPET_CHARS)
            .collect();
        if !snippet.is_empty() {
            println!("    {snippet}");
        }
        if !entry.sources.is_empty() {
            println!("    files: {}", entry.sources.join(", "));
        }
    }
    Ok(())
}
