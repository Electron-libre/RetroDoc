//! What the LLM said about the rules of a feature, checked: a rule is kept only with a citation of
//! code that exists (a file shown to the LLM, by its number or its path, lines inside it); the tests
//! and documentation it cites are ids of the prompt, a quoted test phrase must be in the extract;
//! use cases are those of the list and entities the ones the statement names; confidence follows from what backs the rule.

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
    #[serde(default, deserialize_with = "lenient_names")]
    #[schemars(with = "Vec<String>")]
    pub(super) use_cases: Vec<String>,
    /// `"F2:10-24"`: the number of a file of the prompt and a line range.
    #[serde(default, deserialize_with = "lenient_code")]
    #[schemars(with = "Vec<String>")]
    pub(super) code: Vec<RawCode>,
    #[serde(default, deserialize_with = "lenient_tests")]
    #[schemars(with = "Vec<RawTest>")]
    pub(super) tests: Vec<RawTest>,
    #[serde(default, deserialize_with = "lenient_names")]
    #[schemars(with = "Vec<String>")]
    pub(super) docs: Vec<String>,
}

/// A list of names read leniently: a hosted model often answers `{"id": "D1"}` or `{"name": ...}`
/// where a string is asked. An element is its string, else the first text among its `id`, `name`
/// and `slug`; anything else is skipped, so a wrong shape costs the element, not the feature.
fn lenient_names<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    let items = match value {
        Some(serde_json::Value::Array(items)) => items,
        Some(single @ (serde_json::Value::String(_) | serde_json::Value::Object(_))) => {
            vec![single]
        }
        _ => Vec::new(),
    };
    Ok(items.iter().filter_map(name_of).collect())
}

fn name_of(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) => Some(text.clone()),
        serde_json::Value::Object(map) => ["id", "name", "slug"]
            .iter()
            .find_map(|key| map.get(*key).and_then(serde_json::Value::as_str))
            .map(str::to_string),
        _ => None,
    }
}

/// The cited tests: `{"id", "phrase"}` objects, or a bare id (which quotes nothing, so it backs
/// nothing but does not spoil the answer).
fn lenient_tests<'de, D>(deserializer: D) -> Result<Vec<RawTest>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    let items = match value {
        Some(serde_json::Value::Array(items)) => items,
        Some(single @ (serde_json::Value::String(_) | serde_json::Value::Object(_))) => {
            vec![single]
        }
        _ => Vec::new(),
    };
    Ok(items
        .iter()
        .filter_map(|item| {
            let id = name_of(item)?;
            let phrase = item
                .get("phrase")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string();
            Some(RawTest { id, phrase })
        })
        .collect())
}

#[derive(Debug, Deserialize)]
pub(super) struct RawCode {
    /// A file number of the prompt (`F2`) or a path.
    pub(super) path: Option<String>,
    pub(super) start_line: Option<u32>,
    pub(super) end_line: Option<u32>,
}

/// The cited code: `"F2:10-24"`, `"F2:7"`, `"F2"` or `"lib/a.rb:3-4"` strings, or the `{"path",
/// "start_line", "end_line"}` objects an older answer (or a model that ignores the shape) gives;
/// anything else is skipped.
fn lenient_code<'de, D>(deserializer: D) -> Result<Vec<RawCode>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    let items = match value {
        Some(serde_json::Value::Array(items)) => items,
        Some(single @ (serde_json::Value::String(_) | serde_json::Value::Object(_))) => {
            vec![single]
        }
        _ => Vec::new(),
    };
    Ok(items.iter().filter_map(raw_code_of).collect())
}

fn raw_code_of(value: &serde_json::Value) -> Option<RawCode> {
    match value {
        serde_json::Value::String(text) => Some(parse_citation(text)),
        serde_json::Value::Object(_) => serde_json::from_value(value.clone()).ok(),
        _ => None,
    }
}

/// `file:start-end`, `file:line` or `file`; a suffix after the last `:` that is not lines belongs
/// to the file.
fn parse_citation(text: &str) -> RawCode {
    let text = text.trim();
    if let Some((file, lines)) = text.rsplit_once(':') {
        let (start, end) = lines.split_once('-').unwrap_or((lines, ""));
        let start = start.trim().parse::<u32>().ok();
        let end = end.trim().parse::<u32>().ok();
        if start.is_some() && (end.is_some() || !lines.contains('-')) {
            return RawCode {
                path: Some(file.trim().to_string()),
                start_line: start,
                end_line: end,
            };
        }
    }
    // A slip in the range (`F2:L10-24`) costs the lines, not the file, when the file is a number.
    let file = text.split_once(':').map_or(text, |(file, _)| file);
    RawCode {
        path: Some(
            if file_number(file).is_some() {
                file
            } else {
                text
            }
            .to_string(),
        ),
        start_line: None,
        end_line: None,
    }
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
    cited_files: &[String],
    use_cases: &[&UseCase],
) -> Vec<BusinessRule> {
    let mut lines = LineCounts {
        repo_root,
        counts: BTreeMap::new(),
    };
    let file_set: BTreeSet<String> = cited_files.iter().cloned().collect();
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
            .filter_map(|c| cited_code(c, cited_files, &file_set, &mut lines))
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
            entities: mentioned(&statement, &entity_names),
            statement,
            kind,
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
    numbered: &[String],
    files: &BTreeSet<String>,
    lines: &mut LineCounts<'_>,
) -> Option<SourceRef> {
    let reference = cited.path.as_deref()?;
    let path = match file_number(reference).filter(|_| !files.contains(reference.trim())) {
        Some(number) => numbered.get(number.checked_sub(1)?)?.clone(),
        None => resolve_cited_path(reference, files)?,
    };
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

/// The number of a file as the prompt shows it (`F2`, or `[F2]`), unless it is a path.
fn file_number(reference: &str) -> Option<usize> {
    let digits = reference
        .trim()
        .trim_matches(|c| c == '[' || c == ']')
        .strip_prefix(['F', 'f'])?;
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
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

/// Whether `word` is `name` or its plural (`s`, `es`, or `ies` for a name ending in `y`).
fn same_word(word: &str, name: &str) -> bool {
    word == name
        || word.strip_suffix('s') == Some(name)
        || word.strip_suffix("es") == Some(name)
        || name
            .strip_suffix('y')
            .is_some_and(|stem| word.strip_suffix("ies") == Some(stem))
}

/// The names of `allowed` the statement speaks of, as whole words (case-insensitive, singular or plural), in the order of `allowed`.
pub(super) fn mentioned(statement: &str, allowed: &[&str]) -> Vec<String> {
    let words: Vec<String> = statement
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect();
    allowed
        .iter()
        .filter(|name| {
            let name: Vec<String> = name
                .split(|c: char| !c.is_alphanumeric())
                .filter(|w| !w.is_empty())
                .map(str::to_lowercase)
                .collect();
            !name.is_empty()
                && words
                    .windows(name.len())
                    .any(|window| window.iter().zip(&name).all(|(w, n)| same_word(w, n)))
        })
        .map(|name| (*name).to_string())
        .collect()
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
