//! What a run cost, for the user: the recap printed at the end of every
//! command that calls the LLM ([`recap`]) and its history in
//! `.retrodoc/cache/usage.json` ([`record_run`]), so pass timings and token
//! counts survive the terminal log. It lives apart from the generated docs
//! on purpose: figures that change on every run would make the rendered
//! `run-metadata.json` differ each time and break the no-op rerun.
//!
//! Tokens are what the server reported (`retrodoc_llm::Usage`); calls
//! without it are counted apart and the recap says the sums are then a
//! lower bound.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;
use std::time::Duration;

use retrodoc_llm::{CallTotals, UsageReport};
use serde::{Deserialize, Serialize};

use crate::artifact::{self, Artifact};
use crate::error::PipelineError;
use crate::progress::format_duration;

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
}

fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

impl RunUsage {
    #[must_use]
    pub fn new(command: &str, finished_at: &str, report: &UsageReport) -> Self {
        Self {
            command: command.to_string(),
            finished_at: finished_at.to_string(),
            wall_ms: millis(report.wall()),
            passes: report
                .passes
                .iter()
                .map(|pass| PassRecord {
                    name: pass.name.clone(),
                    wall_ms: millis(pass.wall),
                    unparseable: pass.unparseable,
                    skipped: pass.skipped,
                    models: pass
                        .models
                        .iter()
                        .map(|(model, totals)| ModelRecord {
                            model: model.clone(),
                            calls: totals.calls,
                            calls_without_usage: totals.calls_without_usage,
                            prompt_tokens: totals.prompt_tokens,
                            completion_tokens: totals.completion_tokens,
                        })
                        .collect(),
                })
                .collect(),
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct History {
    runs: Vec<RunUsage>,
}

/// The saved runs, oldest first. Missing or unreadable: none.
#[must_use]
pub fn load_history(repo_root: &Path) -> Vec<RunUsage> {
    artifact::load_json::<History>(&Artifact::Usage.path(repo_root))
        .unwrap_or_default()
        .runs
}

/// Appends `run` to the history, keeping the last [`KEPT_RUNS`].
///
/// # Errors
///
/// Returns an error if the history file can't be written.
pub fn record_run(repo_root: &Path, run: RunUsage) -> Result<(), PipelineError> {
    let mut runs = load_history(repo_root);
    runs.push(run);
    if runs.len() > KEPT_RUNS {
        runs.drain(..runs.len() - KEPT_RUNS);
    }
    artifact::save_json(&Artifact::Usage.path(repo_root), &History { runs })
}

/// `12400` → `12,400`.
fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// The text printed at the end of a command: the total, one row per pass
/// that called the LLM, the models when there are several, and a warning when
/// some calls reported no tokens.
#[must_use]
pub fn recap(report: &UsageReport) -> String {
    let total = report.total();
    if total.calls == 0 {
        return "LLM usage: no call (everything was reused from the cache).\n".to_string();
    }
    let mut out = format!(
        "LLM usage: {} call(s), {} tokens in, {} tokens out, {}.\n",
        thousands(total.calls),
        thousands(total.prompt_tokens),
        thousands(total.completion_tokens),
        format_duration(report.wall()),
    );
    let rows: Vec<_> = report
        .passes
        .iter()
        .filter(|pass| pass.total().calls > 0 || pass.unparseable > 0)
        .collect();
    let name_width = rows.iter().map(|p| p.name.len()).max().unwrap_or(0);
    for pass in rows {
        let t = pass.total();
        let _ = writeln!(
            out,
            "  {:<name_width$}  {:>7} call(s)  {:>12} in  {:>11} out  {:>7}",
            pass.name,
            thousands(t.calls),
            thousands(t.prompt_tokens),
            thousands(t.completion_tokens),
            format_duration(pass.wall),
        );
        if pass.unparseable > 0 {
            let _ = writeln!(
                out,
                "  {:<name_width$}  {} unparseable answer(s), {} skipped",
                "",
                thousands(pass.unparseable),
                thousands(pass.skipped),
            );
        }
    }

    let mut models: BTreeMap<&str, CallTotals> = BTreeMap::new();
    for pass in &report.passes {
        for (model, totals) in &pass.models {
            let entry = models.entry(model).or_default();
            entry.calls += totals.calls;
            entry.prompt_tokens += totals.prompt_tokens;
            entry.completion_tokens += totals.completion_tokens;
        }
    }
    if models.len() > 1 {
        out.push_str("  By model:\n");
        for (model, t) in &models {
            let _ = writeln!(
                out,
                "    {model}: {} call(s), {} in, {} out",
                thousands(t.calls),
                thousands(t.prompt_tokens),
                thousands(t.completion_tokens),
            );
        }
    }
    if total.calls_without_usage > 0 {
        let _ = writeln!(
            out,
            "  {} call(s) reported no token usage: the token counts above are a lower bound.",
            thousands(total.calls_without_usage),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use retrodoc_llm::PassUsage;

    use super::*;

    fn pass(name: &str, secs: u64, models: &[(&str, u64, u64, u64, u64)]) -> PassUsage {
        PassUsage {
            name: name.to_string(),
            wall: Duration::from_secs(secs),
            models: models
                .iter()
                .map(|&(model, calls, without, prompt, completion)| {
                    (
                        model.to_string(),
                        CallTotals {
                            calls,
                            calls_without_usage: without,
                            prompt_tokens: prompt,
                            completion_tokens: completion,
                        },
                    )
                })
                .collect(),
            unparseable: 0,
            skipped: 0,
        }
    }

    fn report() -> UsageReport {
        UsageReport {
            passes: vec![
                pass("roles", 45, &[("big", 3, 0, 12_400, 1_800)]),
                pass("repo-map", 600, &[("big", 100, 0, 1_000_000, 90_000)]),
                pass("domains", 2, &[]),
            ],
        }
    }

    #[test]
    fn thousands_are_grouped() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1_000), "1,000");
        assert_eq!(thousands(1_203_400), "1,203,400");
    }

    #[test]
    fn recap_lists_the_total_and_the_passes_that_called_the_llm() {
        let text = recap(&report());
        assert!(
            text.starts_with(
                "LLM usage: 103 call(s), 1,012,400 tokens in, 91,800 tokens out, 10m47s.\n"
            ),
            "{text}"
        );
        assert!(text.contains("roles"), "{text}");
        assert!(text.contains("repo-map"), "{text}");
        // A pass that reused its cache has no row.
        assert!(!text.contains("domains"), "{text}");
        // One model: no per-model section; every call reported its tokens.
        assert!(!text.contains("By model"), "{text}");
        assert!(!text.contains("lower bound"), "{text}");
    }

    #[test]
    fn recap_shows_the_unparseable_answers_of_a_pass() {
        let mut flaky = pass("features", 5, &[("m", 4, 0, 10, 10)]);
        flaky.unparseable = 3;
        flaky.skipped = 1;
        let text = recap(&UsageReport {
            passes: vec![flaky, pass("glossary", 5, &[("m", 2, 0, 10, 10)])],
        });
        assert!(
            text.contains("3 unparseable answer(s), 1 skipped"),
            "{text}"
        );
        assert_eq!(text.matches("unparseable").count(), 1, "{text}");
    }

    #[test]
    fn the_counts_are_saved_and_an_old_history_without_them_still_loads() {
        let dir = tempfile::tempdir().unwrap();
        let mut flaky = pass("features", 1, &[("m", 1, 0, 1, 1)]);
        flaky.unparseable = 2;
        flaky.skipped = 1;
        let run = RunUsage::new(
            "generate",
            "2026-10-09T00:00:00Z",
            &UsageReport {
                passes: vec![flaky],
            },
        );
        record_run(dir.path(), run).unwrap();
        let back = load_history(dir.path());
        assert_eq!(
            (back[0].passes[0].unparseable, back[0].passes[0].skipped),
            (2, 1)
        );

        let old = r#"{"runs":[{"command":"generate","finished_at":"x","wall_ms":1,"passes":[{"name":"p","wall_ms":1,"models":[]}]}]}"#;
        let path = dir.path().join(".retrodoc/cache/usage.json");
        std::fs::write(&path, old).unwrap();
        assert_eq!(load_history(dir.path())[0].passes[0].unparseable, 0);
    }

    #[test]
    fn recap_splits_by_model_when_there_are_several() {
        let report = UsageReport {
            passes: vec![
                pass("features", 5, &[("big", 2, 0, 200, 40)]),
                pass(
                    "confidence",
                    5,
                    &[("big", 1, 0, 100, 20), ("small", 4, 0, 40, 8)],
                ),
            ],
        };
        let text = recap(&report);
        assert!(text.contains("By model:"), "{text}");
        assert!(text.contains("big: 3 call(s), 300 in, 60 out"), "{text}");
        assert!(text.contains("small: 4 call(s), 40 in, 8 out"), "{text}");
    }

    #[test]
    fn recap_says_when_the_token_counts_are_a_lower_bound() {
        let report = UsageReport {
            passes: vec![pass("roles", 5, &[("local", 4, 3, 10, 2)])],
        };
        let text = recap(&report);
        assert!(text.contains("3 call(s) reported no token usage"), "{text}");
        assert!(text.contains("lower bound"), "{text}");
    }

    #[test]
    fn recap_of_a_run_without_call_says_so() {
        let text = recap(&UsageReport::default());
        assert_eq!(
            text,
            "LLM usage: no call (everything was reused from the cache).\n"
        );
    }

    #[test]
    fn a_run_is_saved_and_read_back_with_its_passes_and_models() {
        let dir = tempfile::tempdir().unwrap();
        let run = RunUsage::new("generate", "2026-10-06T10:00:00+00:00", &report());
        record_run(dir.path(), run.clone()).unwrap();

        assert_eq!(load_history(dir.path()), vec![run.clone()]);
        assert_eq!(run.wall_ms, 647_000);
        assert_eq!(run.passes[1].name, "repo-map");
        assert_eq!(run.passes[1].models[0].prompt_tokens, 1_000_000);
    }

    #[test]
    fn the_history_keeps_the_last_runs_and_survives_an_unreadable_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = Artifact::Usage.path(dir.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "not json").unwrap();
        assert_eq!(load_history(dir.path()), Vec::<RunUsage>::new());

        for i in 0..KEPT_RUNS + 3 {
            let run = RunUsage::new("roles", &format!("run-{i}"), &report());
            record_run(dir.path(), run).unwrap();
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
