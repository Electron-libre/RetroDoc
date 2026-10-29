//! The read-only tools of the MCP server as plain functions over the
//! generated docs, answering in Markdown (the reader is an LLM). Every answer
//! gives the id to reuse, the confidence and the files it rests on; an
//! unknown id or an empty search answers "Not documented" as a normal result.

use std::fmt::Write as _;
use std::path::Path;

use retrodoc_core::model::{BusinessRule, ConfidenceScore, Domain, Feature, RuleEvidence, UseCase};
use retrodoc_ingest::existing_docs::ExistingDoc;
use retrodoc_pipeline::{domain_models, DomainMap, Glossary};

use crate::corpus::{build_entries, feature_id, rule_entries, EntryKind};
use crate::freshness::Freshness;
use crate::search::SearchIndex;
use crate::source::SourceAccess;

mod answer;
mod get_domain;
mod list_domains;

use answer::{
    confidence_line, confidence_short, last_segment, location, one_line, set_apart_tag,
    sources_section, truncate, NO_SOURCE_ACCESS, SET_APART_NOTE, SNIPPET_CHARS,
};

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

    /// A feature, by its id (`billing/pay-invoice`) or its slug alone, with
    /// its use cases and the files it is grounded on.
    #[must_use]
    pub fn get_feature(&self, id: &str) -> String {
        let slug = last_segment(id);
        let Some(feature) = self.features.iter().find(|f| f.slug == slug) else {
            return self.unknown("feature", id, "get_domain");
        };
        let mut out = format!(
            "# {} (`{}`)\n\n{}\n\n{}\n",
            feature.name,
            feature_id(feature),
            feature.description,
            confidence_line(feature.confidence.as_ref())
        );
        if feature.back_office {
            let _ = writeln!(out, "{SET_APART_NOTE}");
        }
        out.push_str(&self.stale_notice(feature.source_paths.iter().map(String::as_str)));
        out.push_str(&sources_section(
            "Grounded on",
            feature.source_paths.iter().map(String::as_str),
        ));
        let use_cases: Vec<&UseCase> = self
            .use_cases
            .iter()
            .filter(|u| u.feature_slug == feature.slug)
            .collect();
        let rules: Vec<&BusinessRule> = self
            .rules
            .iter()
            .filter(|r| r.belongs_to_feature(&feature.slug))
            .collect();
        if use_cases.is_empty() {
            out.push_str("\nNo use case documented for this feature.\n");
            out.push_str(&self.rules_section(&rules));
            return out;
        }
        out.push_str("\n## Use cases\n\n");
        for use_case in use_cases {
            let _ = writeln!(
                out,
                "- `{}/{}` — {}: {} ({}){}",
                feature_id(feature),
                use_case.slug,
                use_case.name,
                one_line(&use_case.description),
                confidence_short(use_case.confidence.as_ref()),
                set_apart_tag(self.use_case_set_apart(use_case)),
            );
        }
        out.push_str("\nUse `get_use_case` with an id to see the steps.\n");
        out.push_str(&self.rules_section(&rules));
        out
    }

    /// A use case, by its id (`billing/pay-invoice/pay-by-card`) or the last
    /// two segments, with its business narrative, steps and cited code.
    #[must_use]
    pub fn get_use_case(&self, id: &str) -> String {
        let mut segments = id.rsplit('/');
        let slug = segments.next().unwrap_or_default();
        let feature_slug = segments.next();
        let found = self
            .use_cases
            .iter()
            .find(|u| u.slug == slug && feature_slug.is_none_or(|f| u.feature_slug == f));
        let Some(use_case) = found else {
            return self.unknown("use case", id, "get_feature");
        };
        let parent = self
            .features
            .iter()
            .find(|f| f.slug == use_case.feature_slug)
            .map_or_else(|| use_case.feature_slug.clone(), feature_id);
        let mut out = format!(
            "# {} (`{parent}/{}`)\n\n{}\n",
            use_case.name,
            use_case.slug,
            confidence_line(use_case.confidence.as_ref())
        );
        if self.use_case_set_apart(use_case) {
            let _ = writeln!(out, "{SET_APART_NOTE}");
        }
        let cited = use_case
            .steps
            .iter()
            .flat_map(|step| step.source_refs.iter().map(|r| r.path.as_str()));
        out.push_str(&self.stale_notice(cited));
        if let Some(narrative) = &use_case.narrative {
            let _ = writeln!(out, "\n{narrative}");
        }
        if !use_case.description.is_empty() {
            let _ = writeln!(out, "\n{}", use_case.description);
        }
        if let Some(actor) = &use_case.primary_actor {
            let _ = writeln!(out, "\nPrimary actor: {actor}");
        }
        if !use_case.entry_points.is_empty() {
            let _ = writeln!(
                out,
                "Triggered by: {}",
                use_case.entry_point_names().join(", ")
            );
        }
        out.push_str("\n## Steps\n\n");
        for step in &use_case.steps {
            let cited: Vec<String> = step.source_refs.iter().map(location).collect();
            let cited = if cited.is_empty() {
                " (no code cited)".to_string()
            } else {
                format!(" ({})", cited.join(", "))
            };
            let text = match step.detail() {
                Some(detail) => format!("{}: {detail}", step.label()),
                None => step.label().to_string(),
            };
            let _ = writeln!(out, "{}. {} — {text}{cited}", step.order, step.actor.name);
        }
        let rules: Vec<&BusinessRule> = self
            .rules
            .iter()
            .filter(|r| r.belongs_to_feature(&use_case.feature_slug))
            .filter(|r| r.use_cases.contains(&use_case.slug))
            .collect();
        out.push_str(&self.rules_section(&rules));
        out
    }

    /// A window of a file the docs cite, to check a claim against the code
    /// (`start_line` and `end_line` are 1-based and inclusive).
    #[must_use]
    pub fn read_source(
        &self,
        path: &str,
        start_line: Option<usize>,
        end_line: Option<usize>,
    ) -> String {
        let Some(source) = &self.source else {
            return NO_SOURCE_ACCESS.into();
        };
        match source.read_source(path, start_line, end_line) {
            Ok(excerpt) => {
                let notice = self.stale_notice([path.strip_prefix("./").unwrap_or(path)]);
                format!("{}\n{notice}\n{}", excerpt.header, excerpt.body)
            }
            Err(reason) => format!("{reason}\n"),
        }
    }

    /// The recent commits that changed a file the docs cite.
    #[must_use]
    pub fn git_log(&self, path: &str, limit: usize) -> String {
        let Some(source) = &self.source else {
            return NO_SOURCE_ACCESS.into();
        };
        source
            .git_log(path, limit)
            .unwrap_or_else(|reason| format!("{reason}\n"))
    }

    /// The best `limit` matches for `query`, or "Not documented".
    #[must_use]
    pub fn search_docs(&self, query: &str, limit: usize) -> String {
        let hits = self.index.search(query, limit);
        if hits.is_empty() {
            return format!(
                "Not documented: nothing among {} entries matches `{query}`. \
                 Try other words, or `list_domains` to browse.\n",
                self.index.len()
            );
        }
        let mut out = format!("# Results for `{query}`\n\n");
        for hit in hits {
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
                    .rules
                    .iter()
                    .find(|r| r.slug == entry.id)
                    .and_then(|r| self.features.iter().find(|f| r.belongs_to_feature(&f.slug)))
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

#[cfg(test)]
mod tests;
