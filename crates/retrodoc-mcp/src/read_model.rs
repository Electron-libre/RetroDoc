//! The generated documentation as typed data, independent of how it is
//! answered: the MCP tools turn it into Markdown for an LLM, the web interface
//! into HTML pages (ADR 0039). Loaded once from what the last `generate` saved.

use std::path::Path;

use retrodoc_core::model::{BusinessRule, Domain, Feature, RuleEvidence, UseCase};
use retrodoc_ingest::existing_docs::ExistingDoc;
use retrodoc_pipeline::{domain_models, DomainMap, Glossary};

use crate::corpus::{build_entries, rule_entries};
use crate::freshness::Freshness;
use crate::search::SearchIndex;

/// The artifacts of a run and the search index over them and the collected docs.
pub struct GeneratedDocs {
    domains: Vec<Domain>,
    features: Vec<Feature>,
    use_cases: Vec<UseCase>,
    rules: Vec<BusinessRule>,
    glossary: Glossary,
    index: SearchIndex,
    freshness: Option<Freshness>,
}

impl GeneratedDocs {
    #[must_use]
    pub fn new(
        domains: Vec<Domain>,
        features: Vec<Feature>,
        use_cases: Vec<UseCase>,
        glossary: Glossary,
        docs: &[ExistingDoc],
    ) -> Self {
        let index = SearchIndex::new(build_entries(
            &domains, &features, &use_cases, &glossary, docs,
        ));
        Self {
            domains,
            features,
            use_cases,
            rules: Vec::new(),
            glossary,
            index,
            freshness: None,
        }
    }

    /// Adds the business rules, found by the search too.
    #[must_use]
    pub fn with_rules(mut self, rules: Vec<BusinessRule>) -> Self {
        let mut entries = self.index.entries().to_vec();
        entries.extend(rule_entries(&rules, &self.features));
        self.index = SearchIndex::new(entries);
        self.rules = rules;
        self
    }

    /// Tells whether the code the docs cite changed since they were generated.
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
                glossary,
                docs,
            )
            .with_rules(rules)
            .with_freshness(Freshness::load(repo_root)),
        )
    }

    /// The files the docs cite: the source files of the features, the lines
    /// of the steps and the code evidence of the rules, repeats included.
    #[must_use]
    pub fn cited_paths(&self) -> Vec<String> {
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
        features.chain(steps).chain(code).cloned().collect()
    }

    #[must_use]
    pub fn domains(&self) -> &[Domain] {
        &self.domains
    }

    #[must_use]
    pub fn features(&self) -> &[Feature] {
        &self.features
    }

    #[must_use]
    pub fn use_cases(&self) -> &[UseCase] {
        &self.use_cases
    }

    #[cfg(test)]
    pub(crate) fn features_mut(&mut self) -> &mut [Feature] {
        &mut self.features
    }

    #[must_use]
    pub fn rules(&self) -> &[BusinessRule] {
        &self.rules
    }

    #[must_use]
    pub fn glossary(&self) -> &Glossary {
        &self.glossary
    }

    #[must_use]
    pub fn index(&self) -> &SearchIndex {
        &self.index
    }

    /// `None` when the docs were built without it (tests).
    #[must_use]
    pub fn freshness(&self) -> Option<&Freshness> {
        self.freshness.as_ref()
    }

    /// The features of the domain `slug`, in order.
    pub fn features_of<'a>(&'a self, slug: &'a str) -> impl Iterator<Item = &'a Feature> + 'a {
        self.features.iter().filter(move |f| f.domain_slug == slug)
    }

    /// The use cases of the feature `slug`, in order.
    pub fn use_cases_of<'a>(&'a self, slug: &'a str) -> impl Iterator<Item = &'a UseCase> + 'a {
        self.use_cases
            .iter()
            .filter(move |u| u.feature_slug == slug)
    }
}
