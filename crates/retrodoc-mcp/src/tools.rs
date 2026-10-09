//! The five read-only tools of the MCP server as plain functions over the
//! generated docs, answering in Markdown (the reader is an LLM). Every answer
//! gives the id to reuse, the confidence and the files it rests on; an
//! unknown id or an empty search answers "Not documented" as a normal result.

use std::fmt::Write as _;
use std::path::Path;

use retrodoc_core::model::{
    ConfidenceScore, Domain, Feature, SourceRef, UseCase, LOW_CONFIDENCE_THRESHOLD,
};
use retrodoc_ingest::existing_docs::ExistingDoc;
use retrodoc_pipeline::{domain_models, DomainMap, Glossary};

use crate::corpus::build_entries;
use crate::freshness::{FileState, Freshness};
use crate::search::SearchIndex;
use crate::source::SourceAccess;

/// The generated documentation, loaded once: the typed artifacts the tools
/// navigate and the search index over them.
pub struct Docs {
    domains: Vec<Domain>,
    features: Vec<Feature>,
    use_cases: Vec<UseCase>,
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
            index,
            freshness: None,
            source: None,
        }
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
        let cited: Vec<String> = features.chain(steps).cloned().collect();
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
        Some(
            Self::new(
                domain_models(&map, &features),
                features,
                use_cases,
                &glossary,
                docs,
            )
            .with_freshness(Freshness::load(repo_root))
            .with_source_access(repo_root),
        )
    }

    #[must_use]
    pub fn index(&self) -> &SearchIndex {
        &self.index
    }

    /// Every domain and sub-domain, with the number of features.
    #[must_use]
    pub fn list_domains(&self) -> String {
        if self.domains.is_empty() {
            return "Not documented: no domain was generated.\n".into();
        }
        let mut out = String::from("# Domains\n\n");
        for domain in &self.domains {
            let count = self
                .features
                .iter()
                .filter(|f| f.domain_slug == domain.slug)
                .count();
            let _ = writeln!(
                out,
                "- `{}` — {}: {} ({}, {count} feature(s))",
                domain.slug,
                domain.name,
                one_line(&domain.description),
                confidence_short(domain.confidence.as_ref()),
            );
            for sub in &domain.sub_domains {
                let _ = writeln!(
                    out,
                    "  - `{}/{}` — {}: {}",
                    domain.slug,
                    sub.slug,
                    sub.name,
                    one_line(&sub.description)
                );
            }
        }
        out.push_str("\nUse `get_domain` with an id to see its features.\n");
        out
    }

    /// A domain (`billing`) or sub-domain (`billing/payment`) and its features.
    #[must_use]
    pub fn get_domain(&self, id: &str) -> String {
        let (domain_slug, sub_slug) = match id.split_once('/') {
            Some((d, s)) => (d, Some(s)),
            None => (id, None),
        };
        let Some(domain) = self.domains.iter().find(|d| d.slug == domain_slug) else {
            return self.unknown("domain", id, "list_domains");
        };
        let (name, description, confidence) = match sub_slug {
            None => (
                &domain.name,
                &domain.description,
                domain.confidence.as_ref(),
            ),
            Some(slug) => match domain.sub_domains.iter().find(|s| s.slug == slug) {
                Some(sub) => (&sub.name, &sub.description, sub.confidence.as_ref()),
                None => return self.unknown("domain", id, "list_domains"),
            },
        };
        let mut out = format!(
            "# {name} (`{id}`)\n\n{description}\n\n{}\n",
            confidence_line(confidence)
        );
        let features: Vec<&Feature> = self
            .features
            .iter()
            .filter(|f| f.domain_slug == domain_slug)
            .filter(|f| sub_slug.is_none_or(|s| f.sub_domain_slug.as_deref() == Some(s)))
            .collect();
        if features.is_empty() {
            out.push_str("\nNo feature documented here.\n");
            return out;
        }
        out.push_str("\n## Features\n\n");
        for feature in features {
            let _ = writeln!(
                out,
                "- `{}` — {}: {} ({})",
                feature_id(feature),
                feature.name,
                one_line(&feature.description),
                confidence_short(feature.confidence.as_ref()),
            );
        }
        out.push_str("\nUse `get_feature` with an id to see its use cases.\n");
        out
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
        if use_cases.is_empty() {
            out.push_str("\nNo use case documented for this feature.\n");
            return out;
        }
        out.push_str("\n## Use cases\n\n");
        for use_case in use_cases {
            let _ = writeln!(
                out,
                "- `{}/{}` — {}: {} ({})",
                feature_id(feature),
                use_case.slug,
                use_case.name,
                one_line(&use_case.description),
                confidence_short(use_case.confidence.as_ref()),
            );
        }
        out.push_str("\nUse `get_use_case` with an id to see the steps.\n");
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
            let _ = writeln!(out, "Triggered by: {}", use_case.entry_points.join(", "));
        }
        out.push_str("\n## Steps\n\n");
        for step in &use_case.steps {
            let cited: Vec<String> = step.source_refs.iter().map(location).collect();
            let cited = if cited.is_empty() {
                " (no code cited)".to_string()
            } else {
                format!(" ({})", cited.join(", "))
            };
            let _ = writeln!(
                out,
                "{}. {} — {}: {}{cited}",
                step.order, step.actor.name, step.action, step.description
            );
        }
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
                "- [{}] {} (`{}`, {})",
                entry.kind.label(),
                entry.title,
                entry.id,
                confidence_short(
                    entry
                        .confidence
                        .map(|v| ConfidenceScore::new(v, None))
                        .as_ref()
                ),
            );
            if let Some(line) = entry
                .text
                .lines()
                .find(|l| *l != entry.title && !l.starts_with('#'))
            {
                let _ = writeln!(out, "  {}", truncate(line, SNIPPET_CHARS));
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

    /// The cited files that changed since generation, as `` `path` (modified) ``.
    fn stale_files<'a>(&self, paths: impl IntoIterator<Item = &'a str>) -> Vec<String> {
        let Some(freshness) = &self.freshness else {
            return Vec::new();
        };
        let mut seen = Vec::new();
        let paths = paths.into_iter().filter(|p| {
            let new = !seen.contains(p);
            seen.push(p);
            new
        });
        freshness
            .stale(paths)
            .into_iter()
            .map(|(path, state)| {
                let what = if state == FileState::Deleted {
                    "deleted"
                } else {
                    "modified"
                };
                format!("`{path}` ({what})")
            })
            .collect()
    }

    /// A warning paragraph when code cited by an answer changed since the
    /// docs were generated; empty otherwise.
    fn stale_notice<'a>(&self, paths: impl IntoIterator<Item = &'a str>) -> String {
        let stale = self.stale_files(paths);
        if stale.is_empty() {
            return String::new();
        }
        format!(
            "\n⚠ Stale: the code cited here changed since these docs were generated: {}. \
             Verify against the current code before relying on this.\n",
            stale.join(", ")
        )
    }

    fn unknown(&self, what: &str, id: &str, browse_with: &str) -> String {
        let mut out = format!("Not documented: no {what} `{id}`.");
        if what == "domain" {
            let known: Vec<&str> = self.domains.iter().map(|d| d.slug.as_str()).collect();
            let _ = write!(out, " Known domains: {}.", known.join(", "));
        }
        let _ = writeln!(
            out,
            " Use `{browse_with}` or `search_docs` to find the right id."
        );
        out
    }
}

const NO_SOURCE_ACCESS: &str =
    "Not available: this server has no access to the repository's code.\n";

/// Characters of a description shown in a list or a search hit.
const SNIPPET_CHARS: usize = 160;

fn feature_id(feature: &Feature) -> String {
    match &feature.sub_domain_slug {
        Some(sub) => format!("{}/{sub}/{}", feature.domain_slug, feature.slug),
        None => format!("{}/{}", feature.domain_slug, feature.slug),
    }
}

fn last_segment(id: &str) -> &str {
    id.rsplit('/').next().unwrap_or(id)
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let cut: String = text.chars().take(max).collect();
    format!("{cut}…")
}

fn one_line(text: &str) -> String {
    truncate(
        text.lines().next().unwrap_or_default().trim(),
        SNIPPET_CHARS,
    )
}

fn percent(value: f32) -> String {
    format!("{:.0}%", value * 100.0)
}

/// `confidence 80%`, `confidence 30% (low)` or `not scored`.
fn confidence_short(score: Option<&ConfidenceScore>) -> String {
    match score {
        None => "not scored".into(),
        Some(s) if s.value < LOW_CONFIDENCE_THRESHOLD => {
            format!("confidence {} (low)", percent(s.value))
        }
        Some(s) => format!("confidence {}", percent(s.value)),
    }
}

/// The full line of a detail answer, with the rationale and, when low, the
/// advice to check the code.
fn confidence_line(score: Option<&ConfidenceScore>) -> String {
    let Some(score) = score else {
        return "Confidence: not scored — treat as unverified.".into();
    };
    let mut line = format!("Confidence: {}", percent(score.value));
    if let Some(rationale) = &score.rationale {
        let _ = write!(line, " ({rationale})");
    }
    if score.value < LOW_CONFIDENCE_THRESHOLD {
        line.push_str(" — low: verify against the code before relying on it.");
    }
    line
}

fn sources_section<'a>(title: &str, paths: impl Iterator<Item = &'a str>) -> String {
    let paths: Vec<&str> = paths.collect();
    if paths.is_empty() {
        return String::new();
    }
    format!("\n{title}: {}\n", paths.join(", "))
}

fn location(source: &SourceRef) -> String {
    match (source.start_line, source.end_line) {
        (Some(start), Some(end)) if start != end => format!("`{}:{start}-{end}`", source.path),
        (Some(line), _) => format!("`{}:{line}`", source.path),
        _ => format!("`{}`", source.path),
    }
}

#[cfg(test)]
mod tests {
    use retrodoc_core::model::ConfidenceScore;

    use super::*;
    use crate::corpus::fixtures::*;

    fn docs() -> Docs {
        let mut weak = use_case(
            "pay-late",
            "pay-invoice",
            "Pay late",
            "Settled after the due date",
        );
        weak.confidence = Some(ConfidenceScore::new(
            0.3,
            "no code found for step 2".to_string(),
        ));
        Docs::new(
            vec![domain("billing", "Billing", "Invoices and payments")],
            vec![
                feature("pay-invoice", "Pay an invoice", "Settle what is due"),
                feature("cancel-plan", "Cancel a plan", "Stop a subscription"),
            ],
            vec![
                use_case(
                    "pay-by-card",
                    "pay-invoice",
                    "Pay by card",
                    "The accountant settles it",
                ),
                weak,
            ],
            &Glossary::default(),
            &[],
        )
    }

    #[test]
    fn domains_are_listed_with_their_sub_domains_and_feature_counts() {
        let out = docs().list_domains();
        assert!(out.contains(
            "- `billing` — Billing: Invoices and payments (confidence 80%, 2 feature(s))"
        ));
        assert!(out.contains("  - `billing/payment` — Payment: Collecting the money"));
    }

    #[test]
    fn a_domain_lists_its_features_with_ids_to_follow() {
        let out = docs().get_domain("billing");
        assert!(out.starts_with("# Billing (`billing`)"));
        assert!(out.contains("`billing/pay-invoice` — Pay an invoice"));
        assert!(out.contains("`billing/cancel-plan`"));
    }

    #[test]
    fn a_sub_domain_without_feature_says_so() {
        let out = docs().get_domain("billing/payment");
        assert!(out.contains("Collecting the money"));
        assert!(out.contains("No feature documented here."));
    }

    #[test]
    fn a_feature_shows_its_use_cases_and_cited_files() {
        let out = docs().get_feature("billing/pay-invoice");
        assert!(out.contains("Grounded on: app/invoice.rb"));
        assert!(out.contains("`billing/pay-invoice/pay-by-card` — Pay by card"));
        assert!(out.contains("Pay late"));
        assert!(docs()
            .get_feature("pay-invoice")
            .contains("# Pay an invoice"));
        assert!(docs()
            .get_feature("cancel-plan")
            .contains("No use case documented"));
    }

    #[test]
    fn a_use_case_gives_narrative_steps_actor_and_code() {
        let out = docs().get_use_case("billing/pay-invoice/pay-by-card");
        assert!(out.contains("The accountant settles it"));
        assert!(out.contains("Primary actor: Accountant"));
        assert!(out.contains("Triggered by: POST /invoices/:id/pay"));
        assert!(out.contains("1. Accountant — records: Checks the amount (`app/invoice.rb`)"));
        assert!(out.contains("Confidence: 90%"));
        assert!(!out.contains("verify against the code"));
    }

    #[test]
    fn a_low_confidence_answer_tells_the_agent_to_check_the_code() {
        let out = docs().get_use_case("pay-invoice/pay-late");
        assert!(out
            .contains("Confidence: 30% (no code found for step 2) — low: verify against the code"));
        assert!(docs()
            .get_feature("pay-invoice")
            .contains("confidence 30% (low)"));
    }

    #[test]
    fn an_unknown_id_is_not_documented_and_points_to_the_way_out() {
        let docs = docs();
        let out = docs.get_domain("shipping");
        assert!(out.starts_with("Not documented: no domain `shipping`."));
        assert!(out.contains("Known domains: billing."));
        assert!(docs
            .get_feature("x")
            .contains("Not documented: no feature `x`"));
        assert!(docs
            .get_use_case("billing/pay-invoice/nope")
            .contains("Not documented: no use case"));
        assert!(docs
            .get_use_case("cancel-plan/pay-by-card")
            .contains("Not documented"));
    }

    #[test]
    fn a_search_lists_ids_confidence_and_files_or_says_not_documented() {
        let docs = docs();
        let out = docs.search_docs("cancel subscription", 5);
        assert!(out.contains("- [feature] Cancel a plan (`billing/cancel-plan`, not scored)"));
        let out = docs.search_docs("zzzz", 5);
        assert!(out.starts_with("Not documented: nothing among"));
    }

    #[test]
    fn nothing_is_loaded_before_a_first_generate() {
        let dir = tempfile::tempdir().unwrap();
        assert!(Docs::load(dir.path(), &[]).is_none());
    }

    #[test]
    fn the_saved_artifacts_are_loaded_and_navigable() {
        let dir = tempfile::tempdir().unwrap();
        retrodoc_pipeline::save_features(
            dir.path(),
            &[feature(
                "pay-invoice",
                "Pay an invoice",
                "Settle what is due",
            )],
        )
        .unwrap();
        retrodoc_pipeline::save_use_cases(
            dir.path(),
            &[use_case("pay-by-card", "pay-invoice", "Pay by card", "x")],
        )
        .unwrap();
        DomainMap {
            domains: vec![retrodoc_pipeline::DomainCluster {
                slug: "billing".into(),
                name: "Billing".into(),
                description: "Invoices".into(),
                paths: vec![],
                sub_domains: vec![],
            }],
        }
        .save(dir.path())
        .unwrap();

        let docs = Docs::load(dir.path(), &[]).unwrap();
        assert!(docs.get_domain("billing").contains("`billing/pay-invoice`"));
        assert!(docs.get_use_case("pay-by-card").contains("Pay by card"));
        assert_eq!(docs.index().len(), 3);
    }

    /// The docs of `docs()` over a repo where `generate` recorded the hashes
    /// of the two files they cite.
    fn docs_over_a_generated_repo() -> (tempfile::TempDir, Docs) {
        let dir = tempfile::tempdir().unwrap();
        let mut cache = retrodoc_pipeline::SourceHashes::default();
        for name in ["app/invoice.rb", "app/mailer.rb"] {
            let path = dir.path().join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, format!("content of {name}")).unwrap();
            cache.put(
                name,
                &retrodoc_pipeline::cache::hash_content(&format!("content of {name}")),
            );
        }
        cache.save(dir.path()).unwrap();
        let docs = docs().with_freshness(Freshness::load(dir.path()));
        (dir, docs)
    }

    #[test]
    fn untouched_code_gives_answers_without_warning() {
        let (_dir, docs) = docs_over_a_generated_repo();
        for out in [
            docs.get_feature("pay-invoice"),
            docs.get_use_case("billing/pay-invoice/pay-by-card"),
            docs.search_docs("settle invoice", 5),
        ] {
            assert!(!out.contains("tale"), "{out}");
        }
    }

    #[test]
    fn a_use_case_warns_about_the_cited_files_that_changed_and_only_those() {
        let (dir, docs) = docs_over_a_generated_repo();
        std::fs::write(dir.path().join("app/mailer.rb"), "edited").unwrap();
        let out = docs.get_use_case("billing/pay-invoice/pay-by-card");
        assert!(out.contains("⚠ Stale"), "{out}");
        assert!(out.contains("`app/mailer.rb` (modified)"), "{out}");
        assert!(!out.contains("`app/invoice.rb` (modified)"), "{out}");
        // The feature cites only the file that did not change.
        assert!(!docs.get_feature("pay-invoice").contains("Stale"));
    }

    #[test]
    fn a_feature_and_a_search_hit_warn_about_a_deleted_file() {
        let (dir, docs) = docs_over_a_generated_repo();
        std::fs::remove_file(dir.path().join("app/invoice.rb")).unwrap();
        let out = docs.get_feature("pay-invoice");
        assert!(out.contains("⚠ Stale"), "{out}");
        assert!(out.contains("`app/invoice.rb` (deleted)"), "{out}");
        let out = docs.search_docs("settle invoice", 5);
        assert!(out.contains("⚠ stale: `app/invoice.rb` (deleted)"), "{out}");
    }

    #[test]
    fn docs_loaded_without_a_recorded_state_never_warn() {
        assert!(!docs()
            .get_use_case("pay-invoice/pay-by-card")
            .contains("tale"));
    }

    #[test]
    fn the_cited_code_can_be_read_through_the_docs_and_nothing_else() {
        let (dir, docs) = docs_over_a_generated_repo();
        let docs = docs.with_source_access(dir.path());
        let out = docs.read_source("app/invoice.rb", None, None);
        assert!(
            out.starts_with("# `app/invoice.rb` (lines 1-1 of 1)"),
            "{out}"
        );
        assert!(out.contains("   1 | content of app/invoice.rb"), "{out}");
        assert!(!out.contains("Stale"), "{out}");
        std::fs::write(dir.path().join("README.md"), "docs").unwrap();
        assert!(docs
            .read_source("README.md", None, None)
            .starts_with("Not available:"));
        assert!(docs
            .read_source("../x", None, None)
            .starts_with("Not available:"));
        assert!(docs.git_log("README.md", 5).starts_with("Not available:"));
    }

    #[test]
    fn a_file_read_after_it_changed_carries_the_stale_notice() {
        let (dir, docs) = docs_over_a_generated_repo();
        let docs = docs.with_source_access(dir.path());
        std::fs::write(dir.path().join("app/mailer.rb"), "edited").unwrap();
        let out = docs.read_source("app/mailer.rb", None, None);
        assert!(out.contains("⚠ Stale"), "{out}");
        assert!(out.contains("   1 | edited"), "{out}");
    }

    #[test]
    fn without_access_to_the_repository_the_code_tools_say_so() {
        let out = docs().read_source("app/invoice.rb", None, None);
        assert!(
            out.starts_with("Not available: this server has no access"),
            "{out}"
        );
        assert!(docs()
            .git_log("app/invoice.rb", 5)
            .starts_with("Not available:"));
    }
}
