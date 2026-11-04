//! The generated documentation as typed data, independent of how it is
//! answered: the MCP tools turn it into Markdown for an LLM, the web interface
//! into HTML pages (ADR 0039). Loaded once from what the last `generate` saved.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use retrodoc_core::model::{BusinessRule, Domain, Feature, RuleEvidence, UseCase};
use retrodoc_ingest::existing_docs::ExistingDoc;
use retrodoc_pipeline::reconcile::{reconcile, Judgements, Reconciliation};
use retrodoc_pipeline::{domain_models, BehaviourCoverage, DomainMap, Glossary, Scope};

use crate::corpus::{build_entries, rule_entries, EntryKind};
use crate::freshness::Freshness;
use crate::search::SearchIndex;
use crate::source::SourceAccess;

/// The artifacts of a run and the search index over them and the collected docs.
pub struct GeneratedDocs {
    domains: Vec<Domain>,
    features: Vec<Feature>,
    use_cases: Vec<UseCase>,
    rules: Vec<BusinessRule>,
    glossary: Glossary,
    index: SearchIndex,
    freshness: Option<Freshness>,
    uncovered: Uncovered,
    /// The commits of the history that changed each file; `None` when it was not read.
    churn: Option<HashMap<String, u32>>,
    /// Where the docs were loaded from, to read the code they cite.
    repo_root: Option<PathBuf>,
}

/// What the docs leave out, read once with them (`specs/debt-view.md`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Uncovered {
    /// Where the items stand against the collected docs, from the saved verdicts (no LLM call);
    /// `None` when there is no collected doc to compare them with.
    pub reconciliation: Option<Reconciliation>,
    /// Source files the clustering left in no domain.
    pub no_domain: Vec<PathBuf>,
    /// Source files the file budget left out, in path order.
    pub skipped: Vec<PathBuf>,
    /// The saved behaviour coverage, `None` but for a `surface` run.
    pub coverage: Option<BehaviourCoverage>,
}

impl Uncovered {
    /// The source files of the code no doc covers, the files of the entry points included, repeats
    /// included.
    #[must_use]
    pub fn paths(&self) -> Vec<String> {
        let coverage = self.coverage.as_ref();
        let support = coverage.map(|c| c.technical_support.as_slice());
        let mut paths: Vec<String> = [
            &self.no_domain[..],
            &self.skipped[..],
            support.unwrap_or_default(),
        ]
        .into_iter()
        .flatten()
        .map(|p| p.display().to_string())
        .collect();
        paths.extend(coverage.into_iter().flat_map(|c| {
            c.unplaced_entry_points
                .iter()
                .filter_map(|id| crate::debt::entry_file(id).map(str::to_string))
        }));
        paths
    }
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
            uncovered: Uncovered::default(),
            churn: None,
            repo_root: None,
        }
    }

    /// Sets the business rules, found by the search too; a second call replaces the first rules.
    #[must_use]
    pub fn with_rules(mut self, rules: Vec<BusinessRule>) -> Self {
        let mut entries = self.index.entries().to_vec();
        entries.retain(|e| e.kind != EntryKind::Rule);
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

    /// Sets what the docs leave out.
    #[must_use]
    pub fn with_uncovered(mut self, uncovered: Uncovered) -> Self {
        self.uncovered = uncovered;
        self
    }

    /// Sets the commits of the history that changed each file, by path, which rank the debt.
    #[must_use]
    pub fn with_churn(mut self, churn: HashMap<String, u32>) -> Self {
        self.churn = Some(churn);
        self
    }

    /// Reads the churn of [`Self::with_churn`] from the history of `HEAD` under the repository the
    /// docs were loaded from (one walk); left unset, with a warning, when it can't be read.
    #[must_use]
    pub fn with_history(self) -> Self {
        let Some(root) = self.repo_root.clone() else {
            return self;
        };
        tracing::info!("reading the git history that ranks the debt");
        match retrodoc_ingest::git_history::collect_history(&root) {
            Ok(history) => self.with_churn(
                history
                    .into_iter()
                    .map(|(path, h)| (path.display().to_string(), h.commit_count))
                    .collect(),
            ),
            Err(error) => {
                tracing::warn!(%error, "no git history: the debt counts no commit");
                self
            }
        }
    }

    /// Reads what the last `generate` saved under `.retrodoc/cache/` plus the
    /// collected `docs`. `None` before a first `generate` (no features saved);
    /// use cases not saved yet count as none. The items are not reconciled with
    /// the docs: [`Self::reconciled`] does it.
    #[must_use]
    pub fn load(repo_root: &Path, docs: &[ExistingDoc]) -> Option<Self> {
        let features = retrodoc_pipeline::load_features(repo_root)?;
        let use_cases = retrodoc_pipeline::load_use_cases(repo_root).unwrap_or_default();
        let map = DomainMap::load(repo_root).unwrap_or_default();
        let glossary = Glossary::load(repo_root).unwrap_or_default();
        let rules = retrodoc_pipeline::load_rules(repo_root).unwrap_or_default();
        let uncovered = Uncovered {
            reconciliation: None,
            no_domain: map.uncategorized_paths().cloned().collect(),
            skipped: Scope::load(repo_root)
                .map(|s| s.skipped)
                .unwrap_or_default(),
            coverage: BehaviourCoverage::load(repo_root),
        };
        Some(
            Self::new(
                domain_models(&map, &features),
                features,
                use_cases,
                glossary,
                docs,
            )
            .with_rules(rules)
            .with_freshness(Freshness::load(repo_root))
            .with_uncovered(uncovered)
            .with_repo_root(repo_root),
        )
    }

    /// Reconciles the items with the collected `docs` from the verdicts saved under the repository
    /// the docs were loaded from (no LLM call): what [`Uncovered::reconciliation`] holds; none with no
    /// collected doc.
    #[must_use]
    pub fn reconciled(mut self, docs: &[ExistingDoc]) -> Self {
        let Some(root) = self.repo_root.as_deref().filter(|_| !docs.is_empty()) else {
            return self;
        };
        self.uncovered.reconciliation = Some(reconcile(
            &DomainMap::load(root).unwrap_or_default(),
            &self.features,
            &self.use_cases,
            &self.rules,
            docs,
            &Judgements::load(root),
        ));
        self
    }

    /// Lets [`Self::source_access`] read the code under `repo_root`.
    #[must_use]
    pub fn with_repo_root(mut self, repo_root: &Path) -> Self {
        self.repo_root = Some(repo_root.to_path_buf());
        self
    }

    /// Where the docs were loaded from, `None` when they were not loaded from a repository.
    #[must_use]
    pub fn repo_root(&self) -> Option<&Path> {
        self.repo_root.as_deref()
    }

    /// The commits of the history that changed each file, by path; `None` when the history was
    /// not read.
    #[must_use]
    pub fn churn(&self) -> Option<&HashMap<String, u32>> {
        self.churn.as_ref()
    }

    /// The bounded access to the files the docs cite, `None` when the docs
    /// were not loaded from a repository.
    #[must_use]
    pub fn source_access(&self) -> Option<SourceAccess> {
        let root = self.repo_root.as_deref()?;
        Some(SourceAccess::new(root, self.cited_paths()))
    }

    /// The bounded access to the files the docs cite and to those of the code no doc covers
    /// ([`Uncovered::paths`]), for a human auditing the debt; `None` when the docs were not loaded
    /// from a repository.
    #[must_use]
    pub fn source_access_with_debt(&self) -> Option<SourceAccess> {
        let root = self.repo_root.as_deref()?;
        let paths = self.cited_paths().into_iter().chain(self.uncovered.paths());
        Some(SourceAccess::new(root, paths))
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

    /// What the docs leave out; nothing when the docs were built without it (tests).
    #[must_use]
    pub fn uncovered(&self) -> &Uncovered {
        &self.uncovered
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use retrodoc_core::model::Feature;
    use retrodoc_ingest::existing_docs::ExistingDoc;
    use retrodoc_pipeline::reconcile::Status;
    use retrodoc_pipeline::{BehaviourCoverage, DomainCluster, DomainMap, Scope};

    use super::GeneratedDocs;

    fn saved_run() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        retrodoc_pipeline::save_features(
            root,
            &[Feature {
                back_office: false,
                technical: false,
                slug: "pay-invoice".into(),
                domain_slug: "billing".into(),
                name: "Pay an invoice".into(),
                description: String::new(),
                source_paths: vec!["app/pay.rb".into()],
                entry_points: Vec::new(),
                confidence: None,
            }],
        )
        .unwrap();
        let cluster = |slug: &str, paths: &[&str]| DomainCluster {
            slug: slug.into(),
            name: slug.into(),
            paths: paths.iter().map(PathBuf::from).collect(),
            ..DomainCluster::default()
        };
        DomainMap {
            domains: vec![
                cluster("billing", &["app/pay.rb"]),
                cluster("uncategorized", &["lib/odd.rb"]),
            ],
        }
        .save(root)
        .unwrap();
        Scope {
            max_files: 1,
            analysed: 1,
            skipped: vec!["vendor/big.rb".into()],
        }
        .save(root)
        .unwrap();
        BehaviourCoverage {
            unplaced_entry_points: vec!["app/admin.rb#purge".into()],
            technical_support: vec!["lib/util.rb".into()],
            ..BehaviourCoverage::default()
        }
        .save(root)
        .unwrap();
        dir
    }

    #[test]
    fn the_load_keeps_what_the_docs_leave_out() {
        let dir = saved_run();
        let docs = GeneratedDocs::load(dir.path(), &[]).unwrap();
        let uncovered = docs.uncovered();
        assert_eq!(uncovered.no_domain, [PathBuf::from("lib/odd.rb")]);
        assert_eq!(uncovered.skipped, [PathBuf::from("vendor/big.rb")]);
        let coverage = uncovered.coverage.as_ref().unwrap();
        assert_eq!(coverage.unplaced_entry_points, ["app/admin.rb#purge"]);
        assert_eq!(coverage.technical_support, [PathBuf::from("lib/util.rb")]);
        assert_eq!(uncovered.reconciliation, None, "no collected doc");
    }

    #[test]
    fn the_load_reconciles_the_items_with_the_collected_docs() {
        let dir = saved_run();
        let collected = [ExistingDoc {
            path: "README.md".into(),
            content: "# Shipping\n\nParcels leave on Monday.\n".into(),
        }];
        let docs = GeneratedDocs::load(dir.path(), &collected)
            .unwrap()
            .reconciled(&collected);
        let reconciliation = docs.uncovered().reconciliation.as_ref().unwrap();
        let feature = reconciliation
            .items
            .iter()
            .find(|i| i.label == "billing/pay-invoice")
            .unwrap();
        assert_eq!(feature.status, Status::Undocumented);
    }
}
