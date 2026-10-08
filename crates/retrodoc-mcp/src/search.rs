//! Lexical search over the [`Entry`] list: BM25 with the title counted
//! three times, so a name beats a passing mention.

use crate::corpus::Entry;
use retrodoc_pipeline::Bm25;

const TITLE_WEIGHT: usize = 3;

pub struct SearchIndex {
    entries: Vec<Entry>,
    bm25: Bm25,
}

#[derive(Debug, Clone, Copy)]
pub struct Hit<'a> {
    pub entry: &'a Entry,
    pub score: f32,
}

impl SearchIndex {
    #[must_use]
    pub fn new(entries: Vec<Entry>) -> Self {
        let texts: Vec<String> = entries
            .iter()
            .map(|e| {
                format!(
                    "{}\n{}",
                    [e.title.as_str(); TITLE_WEIGHT].join("\n"),
                    e.text
                )
            })
            .collect();
        let bm25 = Bm25::new(&texts);
        Self { entries, bm25 }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The best `limit` entries for `query`, best first; none when nothing
    /// shares a word with it.
    #[must_use]
    pub fn search(&self, query: &str, limit: usize) -> Vec<Hit<'_>> {
        self.bm25
            .search(query)
            .into_iter()
            .take(limit)
            .map(|(i, score)| Hit {
                entry: &self.entries[i],
                score,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use retrodoc_ingest::existing_docs::ExistingDoc;
    use retrodoc_pipeline::Glossary;

    use super::*;
    use crate::corpus::build_entries;
    use crate::corpus::fixtures::*;
    use crate::corpus::EntryKind;

    fn index() -> SearchIndex {
        let docs = [ExistingDoc {
            path: PathBuf::from("docs/fees.md"),
            content: "# Late fees\nA customer owes extra after thirty days.".into(),
        }];
        SearchIndex::new(build_entries(
            &[domain("billing", "Billing", "Invoices and payments")],
            &[
                feature("pay-invoice", "Pay an invoice", "Settle what is due"),
                feature("cancel-plan", "Cancel a plan", "Stop a subscription"),
            ],
            &[use_case(
                "pay-by-card",
                "pay-invoice",
                "Pay by card",
                "The accountant settles the invoice",
            )],
            &Glossary::default(),
            &docs,
        ))
    }

    #[test]
    fn a_business_question_finds_the_feature_and_its_use_case() {
        let index = index();
        let hits = index.search("how to cancel a subscription", 5);
        assert_eq!(hits[0].entry.id, "billing/cancel-plan");
        let hits = index.search("settle the invoice", 5);
        let top: Vec<EntryKind> = hits.iter().take(2).map(|h| h.entry.kind).collect();
        assert!(top.contains(&EntryKind::Feature));
        assert!(top.contains(&EntryKind::UseCase));
    }

    #[test]
    fn the_collected_docs_are_searched_too() {
        let index = index();
        let hits = index.search("late fees", 3);
        assert_eq!(hits[0].entry.id, "docs/fees.md");
    }

    #[test]
    fn the_limit_caps_the_hits_and_nonsense_finds_nothing() {
        let index = index();
        assert_eq!(index.search("invoice", 1).len(), 1);
        assert!(index.search("zzzz", 5).is_empty());
    }
}
