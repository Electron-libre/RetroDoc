//! Use cases pass (PLAN.md §2 step 5, roadmap phase 4): for each feature,
//! one LLM call reads the actual code of the feature's files and describes
//! its use cases as ordered steps, each with an actor, an action and the
//! code locations it is grounded on. Saved as the intermediate
//! `.retrodoc/cache/use-cases.yaml` artifact.
//!
//! Grounding is enforced rather than trusted: a step's source reference to a
//! file outside the feature is dropped, and steps are renumbered from 1 in
//! the order given. As in the features pass, a malformed answer for one
//! feature is logged and that feature skipped.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use retrodoc_core::model::{Actor, ActorKind, Feature, SourceRef, Step, UseCase};
use retrodoc_llm::LlmProvider;
use serde::Deserialize;

use crate::error::PipelineError;
use crate::features::unique_slug;
use crate::response::complete_json;

const USE_CASES_RELATIVE_PATH: &str = ".retrodoc/cache/use-cases.yaml";

/// Per-file truncation and overall budget of code sent for one feature
/// (PLAN.md §6 "cost/volume"); files beyond the budget are left out of the
/// prompt, and so can't be cited.
const MAX_CHARS_PER_FILE: usize = 4000;
const MAX_CHARS_PER_PROMPT: usize = 30_000;

const USE_CASES_SYSTEM_PROMPT: &str = "You are documenting a software project from a functional \
point of view. Given a feature and the source code implementing it, describe its use cases: \
concrete scenarios in which an actor achieves a goal with this feature. For each use case give \
the ordered steps, each with its actor (kind \"human\" for a person, \"system\" for a software \
component, service or external system), the action performed, and the file (with line range when \
you can tell) the step is grounded on. Only describe behavior visible in the code provided; do not \
invent steps. Reply with ONLY a single JSON object, no prose and no Markdown code fence, matching \
this shape: {\"use_cases\":[{\"slug\":\"kebab-case\",\"name\":\"...\",\"description\":\"...\",\
\"steps\":[{\"description\":\"...\",\"actor\":{\"name\":\"...\",\"kind\":\"human|system\"},\
\"action\":\"short verb phrase\",\"source_refs\":[{\"path\":\"...\",\"start_line\":1,\
\"end_line\":10}]}]}]}.";

#[derive(Debug, Deserialize)]
struct RawUseCases {
    #[serde(default)]
    use_cases: Vec<RawUseCase>,
}

#[derive(Debug, Deserialize)]
struct RawUseCase {
    slug: String,
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    steps: Vec<RawStep>,
}

/// Steps are parsed leniently: small models emit empty objects, steps
/// without an actor, or "references" that are just notes. Incomplete steps
/// are dropped and incomplete references ignored, rather than rejecting the
/// whole answer.
#[derive(Debug, Deserialize)]
struct RawStep {
    #[serde(default)]
    description: String,
    actor: Option<RawActor>,
    action: Option<String>,
    #[serde(default)]
    source_refs: Vec<RawSourceRef>,
}

#[derive(Debug, Deserialize)]
struct RawActor {
    name: String,
    #[serde(default)]
    kind: String,
}

#[derive(Debug, Deserialize)]
struct RawSourceRef {
    path: Option<String>,
    start_line: Option<u32>,
    end_line: Option<u32>,
}

/// Loads a previously saved `use-cases.yaml`. Missing or unreadable: `None`
/// (first run), not an error.
#[must_use]
pub fn load_use_cases(repo_root: &Path) -> Option<Vec<UseCase>> {
    let raw = std::fs::read_to_string(repo_root.join(USE_CASES_RELATIVE_PATH)).ok()?;
    serde_yaml::from_str(&raw).ok()
}

/// Persists `use_cases` as `.retrodoc/cache/use-cases.yaml`; also used to
/// re-save once diagrams are attached.
///
/// # Errors
///
/// Returns an error if the file can't be written or serialization fails.
pub fn save_use_cases(repo_root: &Path, use_cases: &[UseCase]) -> Result<(), PipelineError> {
    let path = repo_root.join(USE_CASES_RELATIVE_PATH);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| PipelineError::ArtifactIo {
            path: path.clone(),
            source,
        })?;
    }
    let raw = serde_yaml::to_string(use_cases)?;
    std::fs::write(&path, raw).map_err(|source| PipelineError::ArtifactIo { path, source })
}

/// Derives the use cases of every feature from the code it is grounded on
/// and persists them to `.retrodoc/cache/use-cases.yaml`. Diagrams are not
/// attached here (see [`crate::diagrams`]).
///
/// # Errors
///
/// Returns an error if an LLM call fails or the artifact can't be saved.
pub async fn build_use_cases(
    repo_root: &Path,
    features: &[Feature],
    llm: &dyn LlmProvider,
) -> Result<Vec<UseCase>, PipelineError> {
    let mut use_cases: Vec<UseCase> = Vec::new();

    for feature in features {
        let (prompt, cited_files) = use_cases_prompt(repo_root, feature);
        if cited_files.is_empty() {
            tracing::warn!(feature = %feature.slug, "feature skipped: none of its files is readable");
            continue;
        }
        let Some(raw) = complete_json::<RawUseCases>(
            llm,
            USE_CASES_SYSTEM_PROMPT,
            &prompt,
            &format!("use cases of {}", feature.slug),
        )
        .await?
        else {
            continue;
        };

        for raw_use_case in raw.use_cases {
            if raw_use_case.steps.is_empty() {
                tracing::warn!(use_case = %raw_use_case.slug, "use case dropped: no steps");
                continue;
            }
            let slug = unique_slug(
                &raw_use_case.slug,
                use_cases
                    .iter()
                    .filter(|u| u.feature_slug == feature.slug)
                    .map(|u| u.slug.as_str()),
            );
            use_cases.push(UseCase {
                slug,
                feature_slug: feature.slug.clone(),
                name: raw_use_case.name,
                description: raw_use_case.description,
                steps: ground_steps(raw_use_case.steps, &cited_files),
                diagram_mermaid: None,
                confidence: None,
            });
        }
    }

    save_use_cases(repo_root, &use_cases)?;
    Ok(use_cases)
}

/// Numbers steps from 1 (skipping incomplete ones) and keeps only the source
/// references that point to a file actually shown to the LLM for this
/// feature.
fn ground_steps(raw_steps: Vec<RawStep>, allowed: &BTreeSet<String>) -> Vec<Step> {
    let complete = raw_steps.into_iter().filter_map(|raw| {
        let (Some(actor), Some(action)) = (raw.actor, raw.action) else {
            tracing::warn!("step dropped: no actor or no action");
            return None;
        };
        Some((raw.description, actor, action, raw.source_refs))
    });
    complete
        .zip(1..)
        .map(|((description, actor, action, refs), order)| {
            let source_refs = refs
                .into_iter()
                .filter_map(|r| {
                    let path = r.path?;
                    if !allowed.contains(&path) {
                        tracing::warn!(path = %path, "step reference dropped: file not in the feature");
                        return None;
                    }
                    Some(SourceRef {
                        path,
                        start_line: r.start_line,
                        end_line: r.end_line,
                    })
                })
                .collect();
            Step {
                order,
                description,
                actor: Actor {
                    name: actor.name,
                    kind: if actor.kind.eq_ignore_ascii_case("human") {
                        ActorKind::Human
                    } else {
                        ActorKind::System
                    },
                },
                action,
                source_refs,
            }
        })
        .collect()
}

/// Builds the user prompt for `feature` from the code of its files, within
/// the prompt budget. Returns it with the set of files actually included.
fn use_cases_prompt(repo_root: &Path, feature: &Feature) -> (String, BTreeSet<String>) {
    let mut prompt = format!(
        "Feature: {} — {}\n\nSource files:\n",
        feature.name, feature.description
    );
    let mut included = BTreeSet::new();
    let mut budget = MAX_CHARS_PER_PROMPT;

    for path in &feature.source_paths {
        if budget == 0 {
            break;
        }
        let Ok(bytes) = std::fs::read(repo_root.join(path)) else {
            tracing::warn!(path = %path, "could not read a feature file, left out of the prompt");
            continue;
        };
        let content = String::from_utf8_lossy(&bytes);
        let excerpt = numbered_excerpt(&content, MAX_CHARS_PER_FILE.min(budget));
        budget = budget.saturating_sub(excerpt.len());
        let _ = write!(prompt, "\n=== {path} ===\n{excerpt}\n");
        included.insert(path.clone());
    }
    (prompt, included)
}

/// Prefixes each line with its 1-based number (so the LLM can cite line
/// ranges) and stops once `max_chars` are reached.
pub(crate) fn numbered_excerpt(content: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (n, line) in content.lines().enumerate() {
        if out.len() >= max_chars {
            out.push_str("… (truncated)\n");
            break;
        }
        let _ = writeln!(out, "{:>4} | {line}", n + 1);
    }
    out
}

/// Whether the actor is a person rather than a software component.
#[must_use]
pub(crate) fn is_human(actor: &Actor) -> bool {
    actor.kind == ActorKind::Human
}

#[cfg(test)]
mod tests {
    use super::*;

    use async_trait::async_trait;
    use retrodoc_llm::{CompletionRequest, CompletionResponse, LlmError};

    struct CannedProvider {
        response: String,
    }

    #[async_trait]
    impl LlmProvider for CannedProvider {
        async fn complete(&self, _: CompletionRequest) -> Result<CompletionResponse, LlmError> {
            Ok(CompletionResponse {
                content: self.response.clone(),
                model: "test-model".to_string(),
            })
        }
    }

    fn feature(slug: &str, paths: &[&str]) -> Feature {
        Feature {
            slug: slug.to_string(),
            domain_slug: "billing".to_string(),
            sub_domain_slug: None,
            name: slug.to_string(),
            description: "d".to_string(),
            source_paths: paths.iter().map(ToString::to_string).collect(),
            confidence: None,
        }
    }

    #[test]
    fn numbered_excerpt_numbers_lines_and_truncates() {
        let content = "a\nb\nc\n";
        assert_eq!(
            numbered_excerpt(content, 1000),
            "   1 | a\n   2 | b\n   3 | c\n"
        );
        assert!(numbered_excerpt(content, 1).contains("(truncated)"));
    }

    #[tokio::test]
    async fn build_use_cases_renumbers_steps_and_drops_ungrounded_refs() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "fn a() {}\n").unwrap();
        let provider = CannedProvider {
            response: r#"{"use_cases":[
              {"slug":"Pay invoice","name":"Pay invoice","description":"d","steps":[
                {"description":"s1","actor":{"name":"Customer","kind":"human"},
                 "action":"submits payment",
                 "source_refs":[{"path":"a.rs","start_line":1,"end_line":1},
                                {"path":"ghost.rs","start_line":null,"end_line":null}]},
                {"description":"s2","actor":{"name":"API","kind":"system"},
                 "action":"records payment","source_refs":[]}]},
              {"slug":"empty","name":"Empty","description":"d","steps":[]}
            ]}"#
            .to_string(),
        };

        let use_cases = build_use_cases(dir.path(), &[feature("payment", &["a.rs"])], &provider)
            .await
            .unwrap();

        assert_eq!(use_cases.len(), 1);
        let uc = &use_cases[0];
        assert_eq!(uc.slug, "pay-invoice");
        assert_eq!(uc.feature_slug, "payment");
        assert_eq!(
            uc.steps.iter().map(|s| s.order).collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(uc.steps[0].source_refs.len(), 1);
        assert_eq!(uc.steps[0].source_refs[0].path, "a.rs");
        assert!(is_human(&uc.steps[0].actor));
        assert!(!is_human(&uc.steps[1].actor));

        assert_eq!(load_use_cases(dir.path()).unwrap().len(), 1);
    }

    #[tokio::test]
    async fn build_use_cases_skips_features_without_readable_files_and_bad_answers() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "fn a() {}\n").unwrap();
        let provider = CannedProvider {
            response: "not json".to_string(),
        };

        let use_cases = build_use_cases(
            dir.path(),
            &[
                feature("missing", &["nope.rs"]),
                feature("garbled", &["a.rs"]),
            ],
            &provider,
        )
        .await
        .unwrap();

        assert!(use_cases.is_empty());
    }

    #[tokio::test]
    async fn build_use_cases_tolerates_sloppy_steps_and_references() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "fn a() {}\n").unwrap();
        // Two fenced blocks (only the first counts), an empty step, a step
        // without actor, a note instead of a reference, a free-form actor kind.
        let provider = CannedProvider {
            response: r#"```json
            {"use_cases":[{"slug":"u","name":"U","description":"d","steps":[
              {},
              {"description":"no actor","action":"x"},
              {"description":"ok","actor":{"name":"Dev","kind":"Human"},"action":"does",
               "source_refs":[{"note":"inferred"},{"path":"a.rs"}]}]}]}
            ```
            ```json
            {"use_cases":[]}
            ```"#
                .to_string(),
        };

        let use_cases = build_use_cases(dir.path(), &[feature("f", &["a.rs"])], &provider)
            .await
            .unwrap();

        assert_eq!(use_cases.len(), 1);
        let steps = &use_cases[0].steps;
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].order, 1);
        assert!(is_human(&steps[0].actor));
        assert_eq!(steps[0].source_refs.len(), 1);
        assert_eq!(steps[0].source_refs[0].path, "a.rs");
    }
}
