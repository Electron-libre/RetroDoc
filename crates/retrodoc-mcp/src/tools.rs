//! The read-only tools of the MCP server as plain functions over the
//! generated docs, answering in Markdown (the reader is an LLM). Every answer
//! gives the id to reuse, the confidence and the files it rests on; an
//! unknown id or an empty search answers "Not documented" as a normal result.

use std::fmt::Write as _;
use std::path::Path;

use retrodoc_core::model::{
    BusinessRule, ConfidenceScore, Domain, Feature, RuleEvidence, RuleKind, SourceRef, UseCase,
    LOW_CONFIDENCE_THRESHOLD,
};
use retrodoc_ingest::existing_docs::ExistingDoc;
use retrodoc_pipeline::{domain_models, DomainMap, Glossary};

use crate::corpus::{build_entries, rule_entries, Entry, EntryKind};
use crate::freshness::{FileState, Freshness};
use crate::search::SearchIndex;
use crate::source::SourceAccess;

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
        let rules: Vec<&BusinessRule> = self
            .rules
            .iter()
            .filter(|r| features.iter().any(|f| r.belongs_to_feature(&f.slug)))
            .collect();
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
        if !rules.is_empty() {
            out.push_str("\n## Business rules\n\n");
            for rule in rules {
                let _ = writeln!(out, "{}", rule_item(rule));
            }
            out.push_str("\nUse `get_feature` with a feature id to see their evidence.\n");
        }
        out
    }

    /// Whether the docs set a use case apart under "Administration and technical".
    fn use_case_set_apart(&self, use_case: &UseCase) -> bool {
        let back_office = self
            .features
            .iter()
            .any(|f| f.slug == use_case.feature_slug && f.back_office);
        use_case.is_technical(back_office)
    }

    /// Whether a search entry is a back-office feature or a use case set apart.
    fn entry_set_apart(&self, entry: &Entry) -> bool {
        match entry.kind {
            EntryKind::Feature => self
                .features
                .iter()
                .any(|f| f.back_office && feature_id(f) == entry.id),
            EntryKind::UseCase => self.use_cases.iter().any(|u| {
                self.features.iter().any(|f| {
                    f.slug == u.feature_slug && format!("{}/{}", feature_id(f), u.slug) == entry.id
                }) && self.use_case_set_apart(u)
            }),
            _ => false,
        }
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
            let text = if step.action.trim().is_empty() {
                step.description.clone()
            } else {
                format!("{}: {}", step.action, step.description)
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

    /// The `## Business rules` section of a feature or a use case: each rule with its id, kind,
    /// confidence and evidence, and a stale warning for the code it cites. Empty without rules.
    fn rules_section(&self, rules: &[&BusinessRule]) -> String {
        if rules.is_empty() {
            return String::new();
        }
        let mut out = String::from("\n## Business rules\n\n");
        for rule in rules {
            let _ = writeln!(out, "{}", rule_item(rule));
            if !rule.entities.is_empty() {
                let _ = writeln!(out, "  - Entities: {}", rule.entities.join(", "));
            }
            for evidence in &rule.evidence {
                let _ = writeln!(out, "  - {}", evidence_line(evidence));
            }
            let code = rule.evidence.iter().filter_map(|e| match e {
                RuleEvidence::Code { source } => Some(source.path.as_str()),
                _ => None,
            });
            let stale = self.stale_files(code);
            if !stale.is_empty() {
                let _ = writeln!(out, "  - ⚠ stale: {}", stale.join(", "));
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

/// What an answer says of a feature or use case the docs set apart under "Administration and
/// technical": an agent can still read it, and knows it is not the core of the business.
const SET_APART_NOTE: &str = "Administration and technical: a back-office tool, or a behaviour that reads as code or that the code backs weakly (not the core of the business).";

fn set_apart_tag(set_apart: bool) -> &'static str {
    if set_apart {
        " [administration/technical]"
    } else {
        ""
    }
}

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

fn kind_label(kind: RuleKind) -> &'static str {
    match kind {
        RuleKind::Validation => "validation",
        RuleKind::Permission => "permission",
        RuleKind::StateTransition => "state transition",
        RuleKind::Limit => "limit",
        RuleKind::Computation => "computation",
    }
}

/// `- `id` — statement (kind, confidence 85%)`; below the threshold the confidence reads `(low)`.
fn rule_item(rule: &BusinessRule) -> String {
    format!(
        "- `{}` — {} ({}, {})",
        rule.slug,
        one_line(&rule.statement),
        kind_label(rule.kind),
        confidence_short(rule.confidence.as_ref())
    )
}

fn evidence_line(evidence: &RuleEvidence) -> String {
    match evidence {
        RuleEvidence::Code { source } => format!("Code: {}", location(source)),
        RuleEvidence::Test { phrase, path } => {
            format!("Test: \"{}\" (`{path}`)", one_line(phrase))
        }
        RuleEvidence::Doc { path, heading } if heading.is_empty() => {
            format!("Documentation: `{path}`")
        }
        RuleEvidence::Doc { path, heading } => {
            format!("Documentation: `{path}`, section \"{}\"", one_line(heading))
        }
    }
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
    fn what_the_docs_set_apart_is_marked_in_the_answers() {
        // `pay-late` has a confidence of 30%, `pay-by-card` 90%.
        let feature = docs().get_feature("billing/pay-invoice");
        let late = feature.lines().find(|l| l.contains("Pay late")).unwrap();
        let card = feature.lines().find(|l| l.contains("Pay by card")).unwrap();
        assert!(late.contains("[administration/technical]"));
        assert!(!card.contains("[administration/technical]"));
        assert!(docs()
            .get_use_case("pay-invoice/pay-late")
            .contains("Administration and technical"));
        assert!(!docs()
            .get_use_case("billing/pay-invoice/pay-by-card")
            .contains("Administration and technical"));
        let found = docs().search_docs("late", 10);
        let hit = found.lines().find(|l| l.contains("Pay late")).unwrap();
        assert!(hit.contains("[administration/technical]"), "{found}");
    }

    #[test]
    fn a_back_office_feature_is_marked_with_its_use_cases() {
        let mut features = docs().features.clone();
        features
            .iter_mut()
            .find(|f| f.slug == "pay-invoice")
            .unwrap()
            .back_office = true;
        let mut docs = docs();
        docs.features = features;
        let out = docs.get_use_case("billing/pay-invoice/pay-by-card");
        assert!(out.contains("Administration and technical"));
        assert!(docs
            .get_feature("billing/pay-invoice")
            .contains("Administration and technical"));
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
                ..retrodoc_pipeline::DomainCluster::default()
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

    fn rules() -> Vec<BusinessRule> {
        vec![
            BusinessRule {
                slug: "pay-invoice-1".into(),
                statement: "A paid invoice cannot be paid again.".into(),
                kind: RuleKind::StateTransition,
                entities: vec!["Invoice".into()],
                use_cases: vec!["pay-by-card".into()],
                evidence: vec![
                    RuleEvidence::Code {
                        source: SourceRef {
                            path: "app/mailer.rb".into(),
                            start_line: Some(3),
                            end_line: Some(9),
                        },
                    },
                    RuleEvidence::Test {
                        phrase: "refuses a second payment".into(),
                        path: "spec/pay_spec.rb".into(),
                    },
                ],
                confidence: Some(ConfidenceScore::new(0.7, None)),
            },
            BusinessRule {
                slug: "pay-invoice-2".into(),
                statement: "Amounts above the credit ceiling need approval.".into(),
                kind: RuleKind::Limit,
                entities: Vec::new(),
                use_cases: Vec::new(),
                evidence: vec![RuleEvidence::Code {
                    source: SourceRef {
                        path: "app/invoice.rb".into(),
                        start_line: None,
                        end_line: None,
                    },
                }],
                confidence: Some(ConfidenceScore::new(0.4, None)),
            },
            BusinessRule {
                slug: "cancel-plan-1".into(),
                statement: "A cancelled plan stops renewing.".into(),
                kind: RuleKind::Computation,
                entities: Vec::new(),
                use_cases: Vec::new(),
                evidence: Vec::new(),
                confidence: None,
            },
        ]
    }

    #[test]
    fn rules_are_in_the_feature_use_case_and_domain_answers() {
        let docs = docs().with_rules(rules());

        let feature = docs.get_feature("pay-invoice");
        assert!(feature.contains("## Business rules"), "{feature}");
        assert!(
            feature.contains(
                "- `pay-invoice-1` — A paid invoice cannot be paid again. (state transition, confidence 70%)"
            ),
            "{feature}"
        );
        assert!(feature.contains("  - Entities: Invoice"), "{feature}");
        assert!(
            feature.contains("  - Code: `app/mailer.rb:3-9`"),
            "{feature}"
        );
        assert!(
            feature.contains("  - Test: \"refuses a second payment\" (`spec/pay_spec.rb`)"),
            "{feature}"
        );
        assert!(
            feature.contains("(limit, confidence 40% (low))"),
            "{feature}"
        );
        assert!(!feature.contains("cancelled plan"), "{feature}");

        let use_case = docs.get_use_case("billing/pay-invoice/pay-by-card");
        assert!(
            use_case.contains("paid invoice cannot be paid again"),
            "{use_case}"
        );
        assert!(!use_case.contains("credit ceiling"), "{use_case}");

        let domain = docs.get_domain("billing");
        assert!(domain.contains("## Business rules"), "{domain}");
        assert!(domain.contains("`pay-invoice-2`"), "{domain}");
        assert!(domain.contains("`cancel-plan-1`"), "{domain}");
    }

    #[test]
    fn a_feature_without_use_case_still_shows_its_rules() {
        let docs = docs().with_rules(rules());
        let out = docs.get_feature("cancel-plan");
        assert!(out.contains("No use case documented"), "{out}");
        assert!(out.contains("A cancelled plan stops renewing."), "{out}");
    }

    #[test]
    fn without_rules_the_answers_have_no_rules_section() {
        let docs = docs();
        for out in [
            docs.get_feature("pay-invoice"),
            docs.get_use_case("pay-by-card"),
            docs.get_domain("billing"),
        ] {
            assert!(!out.contains("Business rules"), "{out}");
        }
    }

    #[test]
    fn a_search_finds_a_rule_by_what_it_says_and_by_its_tests() {
        let docs = docs().with_rules(rules());
        let out = docs.search_docs("credit ceiling approval", 3);
        assert!(
            out.contains("[rule] Amounts above the credit ceiling need approval."),
            "{out}"
        );
        assert!(out.contains("`pay-invoice-2`"), "{out}");
        let out = docs.search_docs("second payment", 3);
        assert!(out.contains("`pay-invoice-1`"), "{out}");
        assert!(
            out.contains("feature: `billing/pay-invoice` (see `get_feature`)"),
            "{out}"
        );
        assert!(out.contains("files: app/mailer.rb"), "{out}");
    }

    #[test]
    fn a_rule_warns_when_the_code_it_cites_changed_and_its_code_can_be_read() {
        let (dir, docs) = docs_over_a_generated_repo();
        let docs = docs.with_rules(rules()).with_source_access(dir.path());
        assert!(!docs.get_feature("pay-invoice").contains("tale"));
        std::fs::write(dir.path().join("app/mailer.rb"), "edited").unwrap();
        let out = docs.get_feature("pay-invoice");
        assert!(
            out.contains("  - ⚠ stale: `app/mailer.rb` (modified)"),
            "{out}"
        );
        assert!(docs
            .read_source("app/mailer.rb", None, None)
            .starts_with("# `app/mailer.rb`"));
    }

    #[test]
    fn the_saved_rules_are_loaded_with_the_docs() {
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
        retrodoc_pipeline::save_rules(dir.path(), &rules()[..1]).unwrap();
        let docs = Docs::load(dir.path(), &[]).unwrap();
        assert!(docs.get_feature("pay-invoice").contains("`pay-invoice-1`"));
        assert_eq!(docs.index().len(), 2);
    }
}
