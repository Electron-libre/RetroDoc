//! The read-only tools of the MCP server as plain functions over the
//! generated docs, answering in Markdown (the reader is an LLM). Every answer
//! gives the id to reuse, the confidence and the files it rests on; an
//! unknown id or an empty search answers "Not documented" as a normal result.

use std::path::Path;

use retrodoc_core::model::{BusinessRule, Domain, Feature, RuleEvidence, UseCase};
use retrodoc_ingest::existing_docs::ExistingDoc;
use retrodoc_pipeline::{domain_models, DomainMap, Glossary};

use crate::corpus::{build_entries, rule_entries};
use crate::freshness::Freshness;
use crate::search::SearchIndex;
use crate::source::SourceAccess;

mod answer;
mod get_domain;
mod get_feature;
mod get_use_case;
mod git_log;
mod list_domains;
mod read_source;
mod search_docs;

/// The generated documentation, loaded once: the typed artifacts the tools
/// navigate and the search index over them.
pub struct Docs {
    domains: Vec<Domain>,
    features: Vec<Feature>,
    use_cases: Vec<UseCase>,
    rules: Vec<BusinessRule>,
    index: SearchIndex,
    freshness: Option<Freshness>,
    source: Option<SourceAccess>,
}

impl Docs {
    #[must_use]
    pub fn new(
        domains: Vec<Domain>,
        features: Vec<Feature>,
        use_cases: Vec<UseCase>,
        glossary: &Glossary,
        docs: &[ExistingDoc],
    ) -> Self {
        let index = SearchIndex::new(build_entries(
            &domains, &features, &use_cases, glossary, docs,
        ));
        Self {
            domains,
            features,
            use_cases,
            rules: Vec::new(),
            index,
            freshness: None,
            source: None,
        }
    }

    /// Adds the business rules: they appear in `get_feature`, `get_use_case` and `get_domain`, are
    /// found by `search_docs`, and the code they cite can be read.
    #[must_use]
    pub fn with_rules(mut self, rules: Vec<BusinessRule>) -> Self {
        let mut entries = self.index.entries().to_vec();
        entries.extend(rule_entries(&rules, &self.features));
        self.index = SearchIndex::new(entries);
        self.rules = rules;
        self
    }

    /// Lets `read_source` and `git_log` reach the files the docs cite, under
    /// `repo_root`.
    #[must_use]
    pub fn with_source_access(mut self, repo_root: &Path) -> Self {
        let features = self.features.iter().flat_map(|f| f.source_paths.iter());
        let steps = self
            .use_cases
            .iter()
            .flat_map(|u| &u.steps)
            .flat_map(|s| s.source_refs.iter().map(|r| &r.path));
        let code = self
            .rules
            .iter()
            .flat_map(|r| &r.evidence)
            .filter_map(|e| match e {
                RuleEvidence::Code { source } => Some(&source.path),
                _ => None,
            });
        let cited: Vec<String> = features.chain(steps).chain(code).cloned().collect();
        self.source = Some(SourceAccess::new(repo_root, cited));
        self
    }

    /// Makes the answers warn when the code they cite changed since the docs
    /// were generated.
    #[must_use]
    pub fn with_freshness(mut self, freshness: Freshness) -> Self {
        self.freshness = Some(freshness);
        self
    }

    /// Reads what the last `generate` saved under `.retrodoc/cache/` plus the
    /// collected `docs`. `None` before a first `generate` (no features saved);
    /// use cases not saved yet count as none.
    #[must_use]
    pub fn load(repo_root: &Path, docs: &[ExistingDoc]) -> Option<Self> {
        let features = retrodoc_pipeline::load_features(repo_root)?;
        let use_cases = retrodoc_pipeline::load_use_cases(repo_root).unwrap_or_default();
        let map = DomainMap::load(repo_root).unwrap_or_default();
        let glossary = Glossary::load(repo_root).unwrap_or_default();
        let rules = retrodoc_pipeline::load_rules(repo_root).unwrap_or_default();
        Some(
            Self::new(
                domain_models(&map, &features),
                features,
                use_cases,
                &glossary,
                docs,
            )
            .with_rules(rules)
            .with_freshness(Freshness::load(repo_root))
            .with_source_access(repo_root),
        )
    }

    #[must_use]
    pub fn index(&self) -> &SearchIndex {
        &self.index
    }
}

#[cfg(test)]
mod tests;
