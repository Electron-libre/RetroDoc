//! The history of the runs in `.retrodoc/cache/usage.json` (ADR 0043): what each command that called the LLM
//! cost, pass by pass and model by model, with the estimate a `generate` printed before it. The pipeline builds
//! a run from the usage report of the LLM (`retrodoc_pipeline::usage_log::run_usage`), the CLI saves it with
//! [`record_run`]; the audit and history pages read them.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::estimate::RunEstimate;
use crate::cache::{load_json, save_json, Artifact, CacheError};

/// Runs kept in the history, the oldest dropped first.
pub const KEPT_RUNS: usize = 20;

/// One run of one command, as saved in the history.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunUsage {
    /// The subcommand (`generate`, `roles`…).
    pub command: String,
    /// RFC 3339.
    pub finished_at: String,
    pub wall_ms: u64,
    pub passes: Vec<PassRecord>,
    /// The estimate printed before the run (`generate` only); absent from an older history, and dropped when
    /// it can't be read (written by a binary whose estimate differs), so that the run still loads.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "lenient_estimate"
    )]
    pub estimate: Option<RunEstimate>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PassRecord {
    pub name: String,
    pub wall_ms: u64,
    pub models: Vec<ModelRecord>,
    /// Answers the pass could not parse, and among them those it gave up on.
    #[serde(default)]
    pub unparseable: u64,
    #[serde(default)]
    pub skipped: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelRecord {
    pub model: String,
    pub calls: u64,
    pub calls_without_usage: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    /// Among `prompt_tokens`, those served from the server's prompt cache.
    #[serde(default)]
    pub cached_tokens: u64,
}

fn lenient_estimate<'de, D>(deserializer: D) -> Result<Option<RunEstimate>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(serde_json::from_value(value).ok())
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct History {
    runs: Vec<RunUsage>,
}

/// The saved runs, oldest first. Missing or unreadable: none.
#[must_use]
pub fn load_history(repo_root: &Path) -> Vec<RunUsage> {
    load_json::<History>(&Artifact::Usage.path(repo_root))
        .unwrap_or_default()
        .runs
}

/// Appends `run` to the history, keeping the last [`KEPT_RUNS`].
///
/// # Errors
///
/// Returns an error if the history file can't be written.
pub fn record_run(repo_root: &Path, run: RunUsage) -> Result<(), CacheError> {
    let mut runs = load_history(repo_root);
    runs.push(run);
    if runs.len() > KEPT_RUNS {
        runs.drain(..runs.len() - KEPT_RUNS);
    }
    save_json(&Artifact::Usage.path(repo_root), &History { runs })
}

#[cfg(test)]
mod tests {
    use super::super::estimate::{Basis, PassEstimate, Range};
    use super::*;

    fn run(finished_at: &str) -> RunUsage {
        RunUsage {
            command: "generate".to_string(),
            finished_at: finished_at.to_string(),
            wall_ms: 647_000,
            passes: vec![PassRecord {
                name: "features".to_string(),
                wall_ms: 600_000,
                models: vec![ModelRecord {
                    model: "big".to_string(),
                    calls: 100,
                    calls_without_usage: 0,
                    prompt_tokens: 1_000_000,
                    completion_tokens: 90_000,
                    cached_tokens: 500_000,
                }],
                unparseable: 2,
                skipped: 1,
            }],
            estimate: None,
        }
    }

    fn an_estimate() -> RunEstimate {
        let range = |low, high| Range { low, high };
        RunEstimate {
            source_files: 12,
            changed_files: Some(3),
            passes: vec![PassEstimate {
                pass: "use-cases".to_string(),
                basis: Basis::PerSourceFile,
                calls: range(2, 9),
                prompt_tokens: range(1_000, 9_000),
                completion_tokens: range(100, 900),
            }],
            calls: range(2, 9),
            prompt_tokens: range(1_000, 9_000),
            completion_tokens: range(100, 900),
        }
    }

    fn write(root: &Path, json: &str) {
        let path = Artifact::Usage.path(root);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, json).unwrap();
    }

    #[test]
    fn a_run_is_saved_and_read_back_with_its_passes_and_models() {
        let dir = tempfile::tempdir().unwrap();
        record_run(dir.path(), run("2026-10-06T10:00:00+00:00")).unwrap();
        assert_eq!(
            load_history(dir.path()),
            vec![run("2026-10-06T10:00:00+00:00")]
        );
    }

    // spec: TOK-5 (internal: an older history is a file no binary of today writes)
    #[test]
    fn an_old_history_without_the_cached_tokens_loads_with_zero() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            r#"{"runs":[{"command":"generate","finished_at":"x","wall_ms":1,"passes":[{"name":"p","wall_ms":1,"models":[{"model":"m","calls":1,"calls_without_usage":0,"prompt_tokens":5,"completion_tokens":2}]}]}]}"#,
        );
        let back = load_history(dir.path());
        assert_eq!(back[0].passes[0].models[0].prompt_tokens, 5);
        assert_eq!(back[0].passes[0].models[0].cached_tokens, 0);
    }

    #[test]
    fn an_old_history_without_the_unparseable_counts_still_loads() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            r#"{"runs":[{"command":"generate","finished_at":"x","wall_ms":1,"passes":[{"name":"p","wall_ms":1,"models":[]}]}]}"#,
        );
        assert_eq!(load_history(dir.path())[0].passes[0].unparseable, 0);
    }

    // spec: TOK-6 (internal: an older history is a file no binary of today writes)
    #[test]
    fn the_estimate_is_saved_with_its_run_and_an_old_history_loads_without_one() {
        let dir = tempfile::tempdir().unwrap();
        let mut estimated = run("2026-11-05T00:00:00Z");
        estimated.estimate = Some(an_estimate());
        record_run(dir.path(), estimated.clone()).unwrap();
        assert_eq!(load_history(dir.path()), vec![estimated]);

        write(
            dir.path(),
            r#"{"runs":[{"command":"generate","finished_at":"x","wall_ms":1,"passes":[]}]}"#,
        );
        let back = load_history(dir.path());
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].estimate, None);
    }

    // spec: TOK-6 (internal: a history written by another binary)
    #[test]
    fn a_history_whose_estimate_can_not_be_read_still_loads_its_runs() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            r#"{"runs":[{"command":"generate","finished_at":"x","wall_ms":1,"passes":[],
            "estimate":{"source_files":1,"passes":[{"pass":"p","basis":"guessed"}]}}]}"#,
        );
        let back = load_history(dir.path());
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].estimate, None);
    }

    #[test]
    fn the_history_keeps_the_last_runs_and_survives_an_unreadable_file() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "not json");
        assert_eq!(load_history(dir.path()), Vec::<RunUsage>::new());

        for i in 0..KEPT_RUNS + 3 {
            record_run(dir.path(), run(&format!("run-{i}"))).unwrap();
        }
        let runs = load_history(dir.path());
        assert_eq!(runs.len(), KEPT_RUNS);
        assert_eq!(runs[0].finished_at, "run-3");
        assert_eq!(
            runs[KEPT_RUNS - 1].finished_at,
            format!("run-{}", KEPT_RUNS + 2)
        );
    }
}
