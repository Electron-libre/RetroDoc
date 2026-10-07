//! The table that compares runs: one [`RunReport`] per run (written by `retrodoc benchmark --out`),
//! grouped in series (one per configuration, e.g. user docs hidden or shown), summarized by mean and
//! range, and put next to the previous benchmark when there is one.
//!
//! Layout read by [`load_series`]: `<dir>/<series>/<anything>.json`, one directory per series.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::judge::{Judgement, NarrativeRating};
use super::matching::Comparison;
use super::metrics::RunMetrics;
use crate::error::PipelineError;
use crate::progress::format_duration;

/// Everything `retrodoc benchmark` measured for one run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunReport {
    pub metrics: RunMetrics,
    /// Hand-written pairs and equal names only.
    pub strict: Comparison,
    /// With the judge's proposals; `None` when the judge didn't run.
    #[serde(default)]
    pub judged: Option<Comparison>,
    #[serde(default)]
    pub narratives: Option<NarrativeRating>,
}

impl RunReport {
    #[must_use]
    pub fn new(
        metrics: RunMetrics,
        strict: Comparison,
        judged: Option<(Comparison, Judgement)>,
    ) -> Self {
        let (judged, narratives) = match judged {
            Some((comparison, judgement)) => (Some(comparison), Some(judgement.narratives)),
            None => (None, None),
        };
        Self {
            metrics,
            strict,
            judged,
            narratives,
        }
    }
}

/// The runs of one configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct Series {
    pub name: String,
    pub runs: Vec<RunReport>,
}

/// Reads `<dir>/<series>/*.json`, series and runs sorted by name.
///
/// # Errors
///
/// Returns an error if `dir` can't be read or a run file can't be parsed (the message names it).
pub fn load_series(dir: &Path) -> Result<Vec<Series>, PipelineError> {
    let io_error = |path: &Path| {
        let path = path.to_path_buf();
        move |source| PipelineError::ArtifactIo { path, source }
    };
    let mut series = Vec::new();
    for folder in sorted_entries(dir).map_err(io_error(dir))? {
        let hidden = folder
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with('.'));
        if !folder.is_dir() || hidden {
            continue;
        }
        let mut runs = Vec::new();
        for file in sorted_entries(&folder).map_err(io_error(&folder))? {
            if file.extension().is_none_or(|e| e != "json") {
                continue;
            }
            let raw = std::fs::read_to_string(&file).map_err(io_error(&file))?;
            let run =
                serde_json::from_str(&raw).map_err(|error| PipelineError::InvalidReference {
                    path: file.clone(),
                    reason: error.to_string(),
                })?;
            runs.push(run);
        }
        series.push(Series {
            name: folder
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            runs,
        });
    }
    Ok(series)
}

fn sorted_entries(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut entries = std::fs::read_dir(dir)?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    entries.sort();
    Ok(entries)
}

/// One Markdown table per series: mean, range and number of runs of each figure, and the change
/// against the same series of `previous` when it has one.
#[must_use]
pub fn table(current: &[Series], previous: Option<&[Series]>) -> String {
    let mut out = String::new();
    for series in current {
        let before = previous.and_then(|all| all.iter().find(|s| s.name == series.name));
        let _ = writeln!(out, "## {} ({} run(s))\n", series.name, series.runs.len());
        out.push_str("| Metric | Mean | Range | Runs | Previous | Change |\n");
        out.push_str("|---|---|---|---|---|---|\n");
        for figure in FIGURES {
            let now = Summary::of(&series.runs, figure.value);
            let was = before.map(|s| Summary::of(&s.runs, figure.value));
            let (previous, change) = match (&now.mean, was.and_then(|w| w.mean)) {
                (Some(now), Some(was)) => (figure.kind.show(was), figure.kind.change(*now - was)),
                _ => (String::new(), String::new()),
            };
            let (mean, range) = match (now.mean, now.min, now.max) {
                (Some(mean), Some(min), Some(max)) if now.count > 1 => (
                    figure.kind.show(mean),
                    format!("{}–{}", figure.kind.show(min), figure.kind.show(max)),
                ),
                (Some(mean), ..) => (figure.kind.show(mean), "—".to_string()),
                _ => ("n/a".to_string(), "—".to_string()),
            };
            let _ = writeln!(
                out,
                "| {} | {mean} | {range} | {} | {previous} | {change} |",
                figure.label, now.count
            );
        }
        out.push('\n');
    }
    out
}

#[derive(Clone, Copy)]
enum Kind {
    /// A share, between 0 and 1.
    Percent,
    Count,
    /// Milliseconds.
    Time,
}

impl Kind {
    fn show(self, value: f64) -> String {
        match self {
            Kind::Percent => format!("{:.0}%", value * 100.0),
            Kind::Count => format!("{value:.0}"),
            Kind::Time => format_duration(Duration::from_millis(millis(value))),
        }
    }

    /// A signed difference, in the unit of the figure; a difference that rounds to nothing has no sign.
    fn change(self, delta: f64) -> String {
        let (shown, unit) = match self {
            Kind::Percent => (format!("{:.0}", delta.abs() * 100.0), " pts"),
            Kind::Count => (format!("{:.0}", delta.abs()), ""),
            Kind::Time => (
                format_duration(Duration::from_millis(millis(delta.abs()))),
                "",
            ),
        };
        let nothing = shown.chars().all(|c| c == '0') || shown == "0s";
        let sign = match (nothing, delta < 0.0) {
            (true, _) => "",
            (false, true) => "-",
            (false, false) => "+",
        };
        format!("{sign}{shown}{unit}")
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn millis(value: f64) -> u64 {
    value.max(0.0).round() as u64
}

struct Figure {
    label: &'static str,
    kind: Kind,
    value: fn(&RunReport) -> Option<f64>,
}

fn share(value: Option<f32>) -> Option<f64> {
    value.map(f64::from)
}

/// A counter of the run, exact far beyond what a run can reach.
#[allow(clippy::cast_precision_loss)]
fn big(value: u64) -> f64 {
    value as f64
}

#[allow(clippy::cast_precision_loss)]
fn count(value: usize) -> f64 {
    value as f64
}

const FIGURES: &[Figure] = &[
    Figure {
        label: "Domains recall",
        kind: Kind::Percent,
        value: |r| share(r.strict.domains.recall),
    },
    Figure {
        label: "Domains precision",
        kind: Kind::Percent,
        value: |r| share(r.strict.domains.precision),
    },
    Figure {
        label: "Features recall",
        kind: Kind::Percent,
        value: |r| share(r.strict.features.recall),
    },
    Figure {
        label: "Features precision",
        kind: Kind::Percent,
        value: |r| share(r.strict.features.precision),
    },
    Figure {
        label: "Domains recall (judged)",
        kind: Kind::Percent,
        value: |r| r.judged.as_ref().and_then(|c| share(c.domains.recall)),
    },
    Figure {
        label: "Domains precision (judged)",
        kind: Kind::Percent,
        value: |r| r.judged.as_ref().and_then(|c| share(c.domains.precision)),
    },
    Figure {
        label: "Features recall (judged)",
        kind: Kind::Percent,
        value: |r| r.judged.as_ref().and_then(|c| share(c.features.recall)),
    },
    Figure {
        label: "Features precision (judged)",
        kind: Kind::Percent,
        value: |r| r.judged.as_ref().and_then(|c| share(c.features.precision)),
    },
    Figure {
        label: "Narratives in business language (judge)",
        kind: Kind::Percent,
        value: |r| r.narratives.and_then(|n| share(n.business_share())),
    },
    Figure {
        label: "Business-language score",
        kind: Kind::Percent,
        value: |r| share(r.metrics.business_language),
    },
    Figure {
        label: "Domains",
        kind: Kind::Count,
        value: |r| Some(count(r.metrics.domains)),
    },
    Figure {
        label: "Features",
        kind: Kind::Count,
        value: |r| Some(count(r.metrics.features)),
    },
    Figure {
        label: "Use cases",
        kind: Kind::Count,
        value: |r| Some(count(r.metrics.use_cases)),
    },
    Figure {
        label: "LLM calls",
        kind: Kind::Count,
        value: |r| r.metrics.cost.map(|c| big(c.calls)),
    },
    Figure {
        label: "Tokens in",
        kind: Kind::Count,
        value: |r| r.metrics.cost.map(|c| big(c.prompt_tokens)),
    },
    Figure {
        label: "Tokens out",
        kind: Kind::Count,
        value: |r| r.metrics.cost.map(|c| big(c.completion_tokens)),
    },
    Figure {
        label: "Time",
        kind: Kind::Time,
        value: |r| r.metrics.cost.map(|c| big(c.wall_ms)),
    },
];

/// Mean and range of a figure over the runs that have it.
struct Summary {
    count: usize,
    mean: Option<f64>,
    min: Option<f64>,
    max: Option<f64>,
}

impl Summary {
    #[allow(clippy::cast_precision_loss)]
    fn of(runs: &[RunReport], value: fn(&RunReport) -> Option<f64>) -> Self {
        let values: Vec<f64> = runs.iter().filter_map(value).collect();
        Self {
            count: values.len(),
            mean: (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64),
            min: values.iter().copied().reduce(f64::min),
            max: values.iter().copied().reduce(f64::max),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::benchmark::matching::Score;
    use crate::benchmark::metrics::Cost;

    fn score(recall: f32, precision: f32) -> Score {
        Score {
            recall: Some(recall),
            precision: Some(precision),
            matched: vec![],
            unmatched_generated: vec![],
            unmatched_reference: vec![],
        }
    }

    fn report(domain_recall: f32, features: usize, calls: u64, judged: bool) -> RunReport {
        let strict = Comparison {
            domains: score(domain_recall, 1.0),
            features: score(0.5, 0.5),
        };
        RunReport {
            metrics: RunMetrics {
                domains: 3,
                sub_domains: 0,
                features,
                use_cases: 10,
                uncategorized_files: 0,
                narrative_share: Some(1.0),
                business_language: Some(0.5),
                confidence: None,
                cost: Some(Cost {
                    calls,
                    prompt_tokens: 1000,
                    completion_tokens: 100,
                    wall_ms: 60_000,
                    calls_without_usage: 0,
                }),
            },
            judged: judged.then(|| strict.clone()),
            narratives: judged.then_some(NarrativeRating {
                total: 10,
                rated: 10,
                business: 9,
            }),
            strict,
        }
    }

    fn series(name: &str, runs: Vec<RunReport>) -> Series {
        Series {
            name: name.to_string(),
            runs,
        }
    }

    fn row<'a>(table: &'a str, label: &str) -> &'a str {
        table
            .lines()
            .find(|l| l.starts_with(&format!("| {label} |")))
            .unwrap_or_else(|| panic!("no row {label:?} in\n{table}"))
    }

    #[test]
    fn gives_mean_range_and_count_of_each_figure() {
        let runs = vec![
            report(0.5, 6, 10, true),
            report(0.75, 8, 20, true),
            report(1.0, 10, 30, false),
        ];

        let text = table(&[series("hidden", runs)], None);

        assert!(text.contains("## hidden (3 run(s))"), "{text}");
        assert_eq!(
            row(&text, "Domains recall"),
            "| Domains recall | 75% | 50%–100% | 3 |  |  |"
        );
        assert_eq!(row(&text, "Features"), "| Features | 8 | 6–10 | 3 |  |  |");
        assert_eq!(
            row(&text, "LLM calls"),
            "| LLM calls | 20 | 10–30 | 3 |  |  |"
        );
        assert_eq!(
            row(&text, "Time"),
            "| Time | 1m00s | 1m00s–1m00s | 3 |  |  |"
        );
        // The judge ran for two runs out of three.
        assert_eq!(
            row(&text, "Domains recall (judged)"),
            "| Domains recall (judged) | 62% | 50%–75% | 2 |  |  |"
        );
        assert_eq!(
            row(&text, "Narratives in business language (judge)"),
            "| Narratives in business language (judge) | 90% | 90%–90% | 2 |  |  |"
        );
    }

    #[test]
    fn a_single_run_has_no_range_and_a_missing_figure_is_not_available() {
        let mut only = report(0.5, 6, 10, false);
        only.metrics.confidence = None;

        let text = table(&[series("shown", vec![only])], None);

        assert!(text.contains("## shown (1 run(s))"), "{text}");
        assert_eq!(row(&text, "Features"), "| Features | 6 | — | 1 |  |  |");
        assert_eq!(
            row(&text, "Domains recall (judged)"),
            "| Domains recall (judged) | n/a | — | 0 |  |  |"
        );
    }

    #[test]
    fn shows_the_change_against_the_previous_benchmark() {
        let now = [series("hidden", vec![report(0.75, 8, 10, false)])];
        let before = [
            series("hidden", vec![report(0.5, 10, 25, false)]),
            series("other", vec![report(0.0, 1, 1, false)]),
        ];

        let text = table(&now, Some(&before));

        assert_eq!(
            row(&text, "Domains recall"),
            "| Domains recall | 75% | — | 1 | 50% | +25 pts |"
        );
        assert_eq!(row(&text, "Features"), "| Features | 8 | — | 1 | 10 | -2 |");
        assert_eq!(
            row(&text, "LLM calls"),
            "| LLM calls | 10 | — | 1 | 25 | -15 |"
        );
        assert!(
            !text.contains("other"),
            "only the series of this benchmark are shown: {text}"
        );
    }

    #[test]
    fn a_change_that_rounds_to_nothing_has_no_sign() {
        let now = [series("hidden", vec![report(0.499, 8, 10, false)])];
        let before = [series("hidden", vec![report(0.5, 8, 10, false)])];

        let text = table(&now, Some(&before));

        assert_eq!(
            row(&text, "Domains recall"),
            "| Domains recall | 50% | — | 1 | 50% | 0 pts |"
        );
        assert_eq!(row(&text, "Features"), "| Features | 8 | — | 1 | 8 | 0 |");
    }

    #[test]
    fn loads_the_series_of_a_directory_in_order() {
        let dir = tempfile::tempdir().unwrap();
        for (series, file, run) in [
            ("shown", "run-1.json", report(0.5, 6, 10, false)),
            ("hidden", "run-2.json", report(0.75, 8, 20, true)),
            ("hidden", "run-1.json", report(0.5, 6, 10, false)),
        ] {
            let folder = dir.path().join(series);
            std::fs::create_dir_all(&folder).unwrap();
            std::fs::write(folder.join(file), serde_json::to_string(&run).unwrap()).unwrap();
        }
        std::fs::write(dir.path().join("notes.txt"), "not a series").unwrap();
        std::fs::create_dir_all(dir.path().join(".hidden")).unwrap();

        let loaded = load_series(dir.path()).unwrap();

        let names: Vec<_> = loaded.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["hidden", "shown"]);
        assert_eq!(loaded[0].runs.len(), 2);
        assert_eq!(
            loaded[0].runs[0],
            report(0.5, 6, 10, false),
            "run-1 comes first"
        );
    }

    #[test]
    fn a_run_file_that_cannot_be_read_is_named_in_the_error() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("hidden");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("run-1.json"), "{not json").unwrap();

        let error = load_series(dir.path()).unwrap_err().to_string();

        assert!(error.contains("run-1.json"), "{error}");
        assert!(load_series(&dir.path().join("missing")).is_err());
    }
}
