//! The read-only tools of the MCP server as plain functions over the
//! generated docs, answering in Markdown (the reader is an LLM). Every answer
//! gives the id to reuse, the confidence and the files it rests on; an
//! unknown id or an empty search answers "Not documented" as a normal result.

use std::path::Path;

use retrodoc_core::model::{BusinessRule, Domain, Feature, UseCase};
use retrodoc_ingest::existing_docs::ExistingDoc;
use retrodoc_pipeline::Glossary;

use crate::edits::Edits;
use crate::freshness::Freshness;
use crate::read_model::GeneratedDocs;
use crate::search::SearchIndex;
use crate::source::SourceAccess;

mod answer;
mod edit;
mod get_domain;
mod get_feature;
mod get_use_case;
mod git_log;
mod list_domains;
mod read_source;
mod review;
mod search_docs;
mod sources;

pub use sources::{Source, SourceKind, Warning};

/// The generated documentation, loaded once (the server re-applies the edits when they change), and the
/// bounded access to the
/// code it cites that `read_source` and `git_log` need.
#[derive(Clone)]
pub struct Docs {
    generated: GeneratedDocs,
    source: Option<SourceAccess>,
}

impl From<GeneratedDocs> for Docs {
    fn from(generated: GeneratedDocs) -> Self {
        Self {
            source: generated.source_access(),
            generated,
        }
    }
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
        GeneratedDocs::new(domains, features, use_cases, glossary.clone(), docs).into()
    }

    /// Sets the business rules, replacing any set before: they appear in `get_feature`,
    /// `get_use_case` and `get_domain`, are found by `search_docs`, and the code they cite can be
    /// read, whether the source access was given before or after.
    #[must_use]
    pub fn with_rules(mut self, rules: Vec<BusinessRule>) -> Self {
        self.generated = self.generated.with_rules(rules);
        self.source = self.generated.source_access();
        self
    }

    /// Lets `read_source` and `git_log` reach the files the docs cite, under
    /// `repo_root`.
    #[must_use]
    pub fn with_source_access(mut self, repo_root: &Path) -> Self {
        self.generated = self.generated.with_repo_root(repo_root);
        self.source = self.generated.source_access();
        self
    }

    /// Makes the answers warn when the code they cite changed since the docs
    /// were generated.
    #[must_use]
    pub fn with_freshness(mut self, freshness: Freshness) -> Self {
        self.generated = self.generated.with_freshness(freshness);
        self
    }

    /// Reads what the last `generate` saved (see [`GeneratedDocs::load`]), with
    /// access to the code it cites. `None` before a first `generate`.
    #[must_use]
    pub fn load(repo_root: &Path, docs: &[ExistingDoc]) -> Option<Self> {
        GeneratedDocs::load(repo_root, docs).map(Self::from)
    }

    /// Shows `edits` over the generated sections, in place of those shown before
    /// ([`GeneratedDocs::with_edits`]).
    #[must_use]
    pub fn with_edits(mut self, edits: Edits) -> Self {
        self.generated = self.generated.with_edits(edits);
        self
    }

    /// The edits shown over the sections.
    #[must_use]
    pub fn edits(&self) -> &Edits {
        self.generated.edits()
    }

    /// Where the docs were loaded from, `None` when they were not loaded from a repository.
    #[must_use]
    pub fn repo_root(&self) -> Option<&Path> {
        self.generated.repo_root()
    }

    #[must_use]
    pub fn index(&self) -> &SearchIndex {
        self.generated.index()
    }
}

#[cfg(test)]
mod tests;
