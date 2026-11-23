//! The `search_docs` tool.

use std::fmt::Write as _;

use retrodoc_core::model::ConfidenceScore;

use super::answer::{confidence_short, set_apart_tag, truncate, SNIPPET_CHARS};
use super::Docs;
use crate::corpus::{feature_id, EntryKind};

impl Docs {
    /// The best `limit` matches for `query`, or "Not documented".
    #[must_use]
    pub fn search_docs(&self, query: &str, limit: usize) -> String {
        let found = self.generated.index().search(query, limit);
        if found.hits.is_empty() {
            return format!(
                "Not documented: nothing among {} entries matches `{query}`{}. \
                 Try other words, or `list_domains` to browse.\n",
                self.generated.index().len(),
                found.why_not_found()
            );
        }
        let mut out = format!("# Results for `{query}`\n\n");
        for hit in found.hits {
            let entry = hit.entry;
            let _ = writeln!(
                out,
                "- [{}] {} (`{}`, {}){}",
                entry.kind.label(),
                entry.title,
                entry.id,
                confidence_short(
                    entry
                        .confidence
                        .map(|v| ConfidenceScore::new(v, None))
                        .as_ref()
                ),
                set_apart_tag(self.entry_set_apart(entry)),
            );
            if let Some(line) = entry
                .text
                .lines()
                .find(|l| *l != entry.title && !l.starts_with('#'))
            {
                let _ = writeln!(out, "  {}", truncate(line, SNIPPET_CHARS));
            }
            if entry.kind == EntryKind::Rule {
                if let Some(feature) = self
                    .generated
                    .rules()
                    .iter()
                    .find(|r| r.slug == entry.id)
                    .and_then(|r| {
                        self.generated
                            .features()
                            .iter()
                            .find(|f| r.belongs_to_feature(&f.slug))
                    })
                {
                    let _ = writeln!(
                        out,
                        "  feature: `{}` (see `get_feature`)",
                        feature_id(feature)
                    );
                }
            }
            if !entry.sources.is_empty() {
                let _ = writeln!(out, "  files: {}", entry.sources.join(", "));
            }
            let stale = self.stale_files(entry.sources.iter().map(String::as_str));
            if !stale.is_empty() {
                let _ = writeln!(out, "  ⚠ stale: {}", stale.join(", "));
            }
        }
        out
    }
}
