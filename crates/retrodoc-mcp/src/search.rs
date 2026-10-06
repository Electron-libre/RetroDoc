//! Lexical search over the [`Entry`] list: BM25 with the title counted
//! three times, so a name beats a passing mention.

use std::path::Path;

use retrodoc_ingest::existing_docs::ExistingDoc;
use retrodoc_pipeline::{domain_models, DomainMap, Glossary};

use crate::bm25::Bm25;
use crate::corpus::{build_entries, Entry};

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

    /// Indexes what the last `generate` saved under `.retrodoc/cache/` plus
    /// the collected `docs`. `None` before a first `generate` (no features
    /// saved); use cases not saved yet count as none.
    #[must_use]
    pub fn load(repo_root: &Path, docs: &[ExistingDoc]) -> Option<Self> {
        let features = retrodoc_pipeline::load_features(repo_root)?;
        let use_cases = retrodoc_pipeline::load_use_cases(repo_root).unwrap_or_default();
        let map = DomainMap::load(repo_root).unwrap_or_default();
        let glossary = Glossary::load(repo_root).unwrap_or_default();
        Some(Self::new(build_entries(
            &domain_models(&map, &features),
            &features,
            &use_cases,
            &glossary,
            docs,
        )))
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

    use retrodoc_core::model::ConfidenceScore;
    use retrodoc_pipeline::{Artifact, DomainCluster};

    use super::*;
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

    #[test]
    fn nothing_is_loaded_before_a_first_generate() {
        let dir = tempfile::tempdir().unwrap();
        assert!(SearchIndex::load(dir.path(), &[]).is_none());
    }

    #[test]
    fn saved_features_are_searchable_before_any_use_case_exists() {
        let dir = tempfile::tempdir().unwrap();
        retrodoc_pipeline::save_features(
            dir.path(),
            &[feature("pay-invoice", "Pay an invoice", "x")],
        )
        .unwrap();
        let index = SearchIndex::load(dir.path(), &[]).unwrap();
        assert_eq!(index.search("invoice", 5).len(), 1);
    }

    #[test]
    fn the_saved_artifacts_are_loaded_and_searched() {
        let dir = tempfile::tempdir().unwrap();
        let mut feature = feature("pay-invoice", "Pay an invoice", "Settle what is due");
        feature.confidence = Some(ConfidenceScore::new(0.7, None));
        retrodoc_pipeline::save_features(dir.path(), &[feature]).unwrap();
        retrodoc_pipeline::save_use_cases(
            dir.path(),
            &[use_case("pay-by-card", "pay-invoice", "Pay by card", "x")],
        )
        .unwrap();
        DomainMap {
            domains: vec![DomainCluster {
                slug: "billing".into(),
                name: "Billing".into(),
                description: "Invoices".into(),
                paths: vec![],
                sub_domains: vec![],
            }],
        }
        .save(dir.path())
        .unwrap();
        assert!(Artifact::Features.path(dir.path()).exists());

        let index = SearchIndex::load(dir.path(), &[]).unwrap();
        let hits = index.search("invoice", 10);
        let feature = hits
            .iter()
            .find(|h| h.entry.kind == EntryKind::Feature)
            .unwrap();
        assert_eq!(feature.entry.confidence, Some(0.7));
        assert_eq!(index.len(), 3);
    }
}
