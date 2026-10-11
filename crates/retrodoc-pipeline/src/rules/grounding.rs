//! What the LLM said about the rules of a feature, checked: a rule is kept only with a citation of
//! code that exists (a file shown to the LLM, lines inside it); the tests and documentation it
//! cites are ids of the prompt, a quoted test phrase must be in the extract; entities and use
//! cases are those of the lists; confidence follows from what backs the rule.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use retrodoc_core::model::{
    BusinessRule, ConfidenceScore, Feature, RuleEvidence, RuleKind, SourceRef, UseCase,
};
use serde::Deserialize;

use super::prompt::evidence_id;
use super::RuleInput;
use crate::brief::ExtractKind;
use crate::naming::normalize;
use crate::use_cases::resolve_cited_path;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(super) struct RawRules {
    #[serde(default)]
    pub(super) rules: Vec<RawRule>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(super) struct RawRule {
    #[serde(default)]
    pub(super) statement: String,
    #[serde(default)]
    pub(super) kind: String,
    #[serde(default)]
    pub(super) entities: Vec<String>,
    #[serde(default)]
    pub(super) use_cases: Vec<String>,
    #[serde(default)]
    pub(super) code: Vec<RawCode>,
    #[serde(default)]
    pub(super) tests: Vec<RawTest>,
    #[serde(default)]
    pub(super) docs: Vec<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(super) struct RawCode {
    pub(super) path: Option<String>,
    pub(super) start_line: Option<u32>,
    pub(super) end_line: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(super) struct RawTest {
    #[serde(default)]
    pub(super) id: String,
    #[serde(default)]
    pub(super) phrase: String,
}

/// Confidence of a rule from what backs it: code alone is a hypothesis, a test description or a
/// documentation section that states it too is stronger, both more so.
fn score(tests: bool, docs: bool) -> ConfidenceScore {
    match (tests, docs) {
        (false, false) => ConfidenceScore::new(0.4, "backed by code alone".to_string()),
        (true, true) => {
            ConfidenceScore::new(0.85, "stated by code, a test and a document".to_string())
        }
        (true, false) => ConfidenceScore::new(0.7, "stated by code and a test".to_string()),
        (false, true) => ConfidenceScore::new(0.7, "stated by code and a document".to_string()),
    }
}

fn parse_kind(raw: &str) -> Option<RuleKind> {
    match raw.trim().to_lowercase().replace(['-', ' '], "_").as_str() {
        "validation" => Some(RuleKind::Validation),
        "permission" => Some(RuleKind::Permission),
        "state_transition" | "state" | "transition" => Some(RuleKind::StateTransition),
        "limit" => Some(RuleKind::Limit),
        "computation" => Some(RuleKind::Computation),
        _ => None,
    }
}

/// Whether `rule` is one of the rules of the feature `slug` (`<slug>-<number>`).
pub(super) fn belongs_to(rule: &BusinessRule, slug: &str) -> bool {
    rule.slug
        .strip_prefix(slug)
        .and_then(|rest| rest.strip_prefix('-'))
        .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
}

/// Number of lines of a file, read once.
struct LineCounts<'a> {
    repo_root: &'a Path,
    counts: BTreeMap<String, Option<u32>>,
}

impl LineCounts<'_> {
    fn of(&mut self, path: &str) -> Option<u32> {
        *self.counts.entry(path.to_string()).or_insert_with(|| {
            let bytes = std::fs::read(self.repo_root.join(path)).ok()?;
            u32::try_from(String::from_utf8_lossy(&bytes).lines().count()).ok()
        })
    }
}

/// The rules of `feature` that the answer states and the code, tests and documents support.
pub(super) fn grounded_rules(
    raw: RawRules,
    repo_root: &Path,
    feature: &Feature,
    input: &RuleInput,
    cited_files: &BTreeSet<String>,
    use_cases: &[&UseCase],
) -> Vec<BusinessRule> {
    let mut lines = LineCounts {
        repo_root,
        counts: BTreeMap::new(),
    };
    let entity_names: Vec<&str> = input.entities.iter().map(String::as_str).collect();
    let use_case_slugs: Vec<&str> = use_cases.iter().map(|u| u.slug.as_str()).collect();
    let mut rules: Vec<BusinessRule> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for r in raw.rules {
        let statement = r.statement.trim().to_string();
        let Some(kind) = parse_kind(&r.kind) else {
            tracing::info!(kind = %r.kind, "rule dropped: unknown kind");
            continue;
        };
        let statement_key = normalize(&statement);
        if statement.is_empty() || seen.contains(&statement_key) {
            continue;
        }
        let mut evidence: Vec<RuleEvidence> = r
            .code
            .iter()
            .filter_map(|c| cited_code(c, cited_files, &mut lines))
            .map(|source| RuleEvidence::Code { source })
            .collect();
        if evidence.is_empty() {
            tracing::info!(rule = %statement, "rule dropped: no code citation that exists");
            continue;
        }
        seen.insert(statement_key);
        let tests = cited_tests(&r.tests, input);
        let docs = cited_docs(&r.docs, input);
        let confidence = score(!tests.is_empty(), !docs.is_empty());
        evidence.extend(tests);
        evidence.extend(docs);
        rules.push(BusinessRule {
            slug: format!("{}-{}", feature.slug, rules.len() + 1),
            statement,
            kind,
            entities: known(&r.entities, &entity_names),
            use_cases: known(&r.use_cases, &use_case_slugs),
            evidence,
            confidence: Some(confidence),
        });
    }
    rules
}

/// A code citation on a file shown to the LLM, with lines inside it; a citation without lines
/// stands for the file, one beyond its end is false and dropped.
fn cited_code(
    cited: &RawCode,
    files: &BTreeSet<String>,
    lines: &mut LineCounts<'_>,
) -> Option<SourceRef> {
    let path = resolve_cited_path(cited.path.as_deref()?, files)?;
    let (start, end) = match (cited.start_line, cited.end_line) {
        (Some(s), Some(e)) => (Some(s.min(e)), Some(s.max(e))),
        (Some(l), None) | (None, Some(l)) => (Some(l), Some(l)),
        (None, None) => (None, None),
    };
    if let (Some(start), Some(end)) = (start, end) {
        if start == 0 || end > lines.of(&path)? {
            return None;
        }
    }
    Some(SourceRef {
        path,
        start_line: start,
        end_line: end,
    })
}

/// The extract an id of the prompt designates.
fn extract<'a>(
    input: &'a RuleInput,
    id: &str,
    kind: ExtractKind,
) -> Option<&'a crate::brief::Extract> {
    let id = id.trim().trim_matches(|c| c == '[' || c == ']');
    input
        .evidence
        .iter()
        .filter(|e| e.kind == kind)
        .enumerate()
        .find(|(n, _)| evidence_id(kind, *n).eq_ignore_ascii_case(id))
        .map(|(_, e)| e)
}

/// Tests cited by id whose phrase is in the extract (a quote that is not there is no evidence).
fn cited_tests(cited: &[RawTest], input: &RuleInput) -> Vec<RuleEvidence> {
    let mut found: Vec<RuleEvidence> = Vec::new();
    for t in cited {
        let Some(e) = extract(input, &t.id, ExtractKind::Test) else {
            continue;
        };
        let phrase = t.phrase.trim();
        if phrase.is_empty() || !e.text.to_lowercase().contains(&phrase.to_lowercase()) {
            tracing::info!(id = %t.id, "test citation dropped: the phrase is not in the extract");
            continue;
        }
        let evidence = RuleEvidence::Test {
            phrase: phrase.to_string(),
            path: e.origin.clone(),
        };
        if !found.contains(&evidence) {
            found.push(evidence);
        }
    }
    found
}

fn cited_docs(cited: &[String], input: &RuleInput) -> Vec<RuleEvidence> {
    let mut found: Vec<RuleEvidence> = Vec::new();
    for id in cited {
        let Some(e) = extract(input, id, ExtractKind::Doc) else {
            continue;
        };
        let (path, heading) = e.origin.split_once('#').unwrap_or((&e.origin, ""));
        let evidence = RuleEvidence::Doc {
            path: path.to_string(),
            heading: heading.to_string(),
        };
        if !found.contains(&evidence) {
            found.push(evidence);
        }
    }
    found
}

/// The names of `cited` that designate one of `allowed` (case-insensitive), spelled as `allowed`.
fn known(cited: &[String], allowed: &[&str]) -> Vec<String> {
    let mut kept: Vec<String> = Vec::new();
    for name in cited {
        if let Some(found) = allowed.iter().find(|a| a.eq_ignore_ascii_case(name.trim())) {
            if !kept.iter().any(|k| k == found) {
                kept.push((*found).to_string());
            }
        }
    }
    kept
}
