//! The calls and tokens a `generate` will cost, estimated before its first LLM call
//! (`specs/run-estimate.md`, `issues/pre_run_estimate.md`). No LLM call: a pure function of what the run knows
//! before it (the source files to read, those changed since the last complete run, the passes it turns off) and
//! of the ratios measured on saved runs (`data/pass-ratios.yaml`, embedded, built by the example `pass_ratios` of
//! `retrodoc-bench`).
//!
//! A pass that scales with its input costs its measured cost per source file times the files to read; a fixed
//! pass its cost per run. The low and high bounds are the lowest and the highest application of the table, never
//! the yields chained (their extremes multiply).

use std::sync::LazyLock;

use serde::Deserialize;

/// The table of measured ratios, as `retrodoc-bench` writes it (the fields the estimate reads).
#[derive(Debug, Clone, Deserialize)]
pub struct RatioTable {
    runs: Vec<TableRun>,
    passes: Vec<TablePass>,
    #[serde(default)]
    no_call: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct TableRun {
    models: Vec<String>,
    #[serde(default)]
    indicative: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct TablePass {
    pass: String,
    fixed: bool,
    per_unit: Option<PerUnit<Spread>>,
    per_source_file: Option<PerUnit<Spread>>,
    applications: Vec<TableApplication>,
}

#[derive(Debug, Clone, Deserialize)]
struct TableApplication {
    #[serde(default)]
    indicative: bool,
    calls: u64,
    prompt_tokens: u64,
    completion_tokens: u64,
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct PerUnit<T> {
    calls: T,
    prompt_tokens: T,
    completion_tokens: T,
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct Spread {
    min: f64,
    max: f64,
}

const EMBEDDED: &str = include_str!("../data/pass-ratios.yaml");

static TABLE: LazyLock<RatioTable> = LazyLock::new(|| {
    RatioTable::parse(EMBEDDED)
        .expect("data/pass-ratios.yaml is checked by the tests of `estimate`")
});

impl RatioTable {
    /// The table built into the binary.
    #[must_use]
    pub fn embedded() -> &'static Self {
        &TABLE
    }

    /// Reads a table written by `retrodoc-bench`.
    ///
    /// # Errors
    ///
    /// Returns the message of the YAML reader.
    pub fn parse(yaml: &str) -> Result<Self, String> {
        serde_yaml::from_str(yaml).map_err(|e| e.to_string())
    }

    /// The models the ratios were measured with (the indicative runs left out).
    #[must_use]
    pub fn measured_models(&self) -> Vec<String> {
        let mut models: Vec<String> = self
            .runs
            .iter()
            .filter(|run| !run.indicative)
            .flat_map(|run| run.models.iter().cloned())
            .collect();
        models.sort();
        models.dedup();
        models
    }

    fn pass(&self, name: &str) -> Option<&TablePass> {
        self.passes.iter().find(|pass| pass.pass == name)
    }
}

/// What the run knows before its first LLM call.
#[derive(Debug, Clone, Default)]
pub struct EstimateInputs<'a> {
    /// The LLM passes the run goes through, in their order.
    pub passes: &'a [&'a str],
    /// The source files the run reads (within the file budget).
    pub source_files: usize,
    /// On a rerun without `--force`, the source files changed, added or removed since the last complete run;
    /// `None` on a first run or with `--force`: every file is read again.
    pub changed_files: Option<usize>,
    /// The fixed-cost passes whose saved artifact exists (a rerun reuses it).
    pub saved_fixed: &'a [&'a str],
    /// The passes the run turns off (`--no-confidence`, `[pipeline] rules = false`, reconcile with no docs).
    pub off: &'a [&'a str],
    /// `--confidence-sample N`: the use cases asked as a second opinion besides those the checks don't settle.
    pub confidence_sample: Option<usize>,
}

/// A low and a high bound.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Range {
    pub low: u64,
    pub high: u64,
}

impl Range {
    fn add(&mut self, other: Self) {
        self.low += other.low;
        self.high += other.high;
    }
}

/// Why a pass is estimated as it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Basis {
    /// Its measured cost per source file, times the files to read.
    PerSourceFile,
    /// Its measured cost per run.
    PerRun,
    /// The run turns it off.
    Off,
    /// The cache saves it: no file changed, or its artifact is saved.
    Cached,
    /// Measured with no call on every run.
    NoCall,
    /// The table has no ratio for it: estimated at zero.
    Unmeasured,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PassEstimate {
    pub pass: String,
    pub basis: Basis,
    pub calls: Range,
    pub prompt_tokens: Range,
    pub completion_tokens: Range,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunEstimate {
    pub passes: Vec<PassEstimate>,
    pub calls: Range,
    pub prompt_tokens: Range,
    pub completion_tokens: Range,
}

impl RunEstimate {
    /// The passes the table has no ratio for.
    #[must_use]
    pub fn unmeasured(&self) -> Vec<&str> {
        self.passes
            .iter()
            .filter(|pass| pass.basis == Basis::Unmeasured)
            .map(|pass| pass.pass.as_str())
            .collect()
    }
}

/// Estimates a run from what it knows before its first call and the measured `table`.
#[must_use]
pub fn estimate(inputs: &EstimateInputs, table: &RatioTable) -> RunEstimate {
    let files = inputs.changed_files.unwrap_or(inputs.source_files);
    let passes: Vec<PassEstimate> = inputs
        .passes
        .iter()
        .map(|&name| estimate_pass(name, files, inputs, table))
        .collect();
    let (mut calls, mut prompt_tokens, mut completion_tokens) = Default::default();
    for pass in &passes {
        Range::add(&mut calls, pass.calls);
        Range::add(&mut prompt_tokens, pass.prompt_tokens);
        Range::add(&mut completion_tokens, pass.completion_tokens);
    }
    RunEstimate {
        passes,
        calls,
        prompt_tokens,
        completion_tokens,
    }
}

fn estimate_pass(
    name: &str,
    files: usize,
    inputs: &EstimateInputs,
    table: &RatioTable,
) -> PassEstimate {
    let zero = |basis| PassEstimate {
        pass: name.to_string(),
        basis,
        calls: Range::default(),
        prompt_tokens: Range::default(),
        completion_tokens: Range::default(),
    };
    if inputs.off.contains(&name) {
        return zero(Basis::Off);
    }
    if table.no_call.iter().any(|pass| pass == name) {
        return zero(Basis::NoCall);
    }
    let Some(measured) = table.pass(name) else {
        return zero(Basis::Unmeasured);
    };
    let (basis, spread, times) = if measured.fixed {
        let reused = inputs.changed_files.is_some() && inputs.saved_fixed.contains(&name);
        (Basis::PerRun, measured.per_unit, usize::from(!reused))
    } else {
        (Basis::PerSourceFile, measured.per_source_file, files)
    };
    let Some(spread) = spread else {
        return zero(Basis::Unmeasured);
    };
    if times == 0 {
        return zero(Basis::Cached);
    }
    #[allow(clippy::cast_precision_loss)]
    let times = times as f64;
    // Rounded to a millionth first, so that a product such as 0.7 x 100 is not taken past 70.
    let exact = |value: f64| (value * times * 1e6).round() / 1e6;
    let range = |s: Spread| Range {
        low: bound(exact(s.min).floor()),
        high: bound(exact(s.max).ceil()),
    };
    let mut pass = PassEstimate {
        pass: name.to_string(),
        basis,
        calls: range(spread.calls),
        prompt_tokens: range(spread.prompt_tokens),
        completion_tokens: range(spread.completion_tokens),
    };
    if name == "confidence" {
        if let Some(sample) = inputs.confidence_sample {
            let (prompt, completion) = costliest_call(measured);
            let sample = sample as u64;
            pass.calls.high += sample;
            pass.prompt_tokens.high += sample * prompt;
            pass.completion_tokens.high += sample * completion;
        }
    }
    pass
}

/// The prompt and completion tokens of one call of the application whose calls cost the most (the indicative
/// ones left out).
fn costliest_call(pass: &TablePass) -> (u64, u64) {
    let per_call = |tokens: u64, calls: u64| tokens.div_ceil(calls.max(1));
    pass.applications
        .iter()
        .filter(|app| !app.indicative && app.calls > 0)
        .fold((0, 0), |(prompt, completion), app| {
            (
                prompt.max(per_call(app.prompt_tokens, app.calls)),
                completion.max(per_call(app.completion_tokens, app.calls)),
            )
        })
}

/// A non-negative bound as a count.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn bound(value: f64) -> u64 {
    value.max(0.0) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two applications: `use-cases` from 0.5 to 1.5 calls per source file, `brief` 1 to 2 calls a run,
    /// `confidence` 0.1 to 0.2 per file (100 to 300 prompt tokens per call), `glossary` with no call.
    const TABLE_YAML: &str = r#"
runs:
- {"application":"a","commit":"c","models":["m1"],"finished_at":"t"}
- {"application":"b","commit":"c","models":["m1"],"finished_at":"t"}
- {"application":"x","commit":"c","models":["m2"],"finished_at":"t","indicative":true}
passes:
- pass: "brief"
  unit: "run"
  fixed: true
  per_unit: {"calls":{"median":1.5,"min":1.0,"max":2.0},"prompt_tokens":{"median":1500.0,"min":1000.0,"max":2000.0},"cached_tokens":{"median":0.0,"min":0.0,"max":0.0},"completion_tokens":{"median":150.0,"min":100.0,"max":200.0}}
  per_source_file: null
  applications: []
- pass: "use-cases"
  unit: "feature"
  fixed: false
  per_unit: null
  per_source_file: {"calls":{"median":1.0,"min":0.5,"max":1.5},"prompt_tokens":{"median":1000.0,"min":500.0,"max":1500.0},"cached_tokens":{"median":0.0,"min":0.0,"max":0.0},"completion_tokens":{"median":100.0,"min":50.0,"max":150.0}}
  applications: []
- pass: "confidence"
  unit: "use case"
  fixed: false
  per_unit: null
  per_source_file: {"calls":{"median":0.15,"min":0.1,"max":0.2},"prompt_tokens":{"median":30.0,"min":10.0,"max":60.0},"cached_tokens":{"median":0.0,"min":0.0,"max":0.0},"completion_tokens":{"median":3.0,"min":1.0,"max":6.0}}
  applications:
  - {"application":"a","runs":1,"units":1.0,"calls":10,"calls_without_usage":0,"prompt_tokens":1000,"cached_tokens":0,"completion_tokens":50,"per_unit":null,"per_source_file":null}
  - {"application":"b","runs":1,"units":1.0,"calls":10,"calls_without_usage":0,"prompt_tokens":3000,"cached_tokens":0,"completion_tokens":200,"per_unit":null,"per_source_file":null}
  - {"application":"x","indicative":true,"runs":1,"units":1.0,"calls":1,"calls_without_usage":0,"prompt_tokens":9000,"cached_tokens":0,"completion_tokens":900,"per_unit":null,"per_source_file":null}
no_call: ["glossary"]
yields: []
"#;

    const PASSES: &[&str] = &["brief", "glossary", "use-cases", "confidence", "contexts"];

    fn table() -> RatioTable {
        RatioTable::parse(TABLE_YAML).unwrap()
    }

    fn first_run(source_files: usize) -> EstimateInputs<'static> {
        EstimateInputs {
            passes: PASSES,
            source_files,
            ..EstimateInputs::default()
        }
    }

    fn of<'a>(estimate: &'a RunEstimate, pass: &str) -> &'a PassEstimate {
        estimate.passes.iter().find(|p| p.pass == pass).unwrap()
    }

    // spec: EST-10 (internal: the table is a file of the crate, read at build time)
    #[test]
    fn the_embedded_table_reads_and_holds_every_measured_pass() {
        let table = RatioTable::parse(EMBEDDED).unwrap();
        for pass in [
            "sources",
            "brief",
            "use-cases",
            "rules",
            "confidence",
            "reconcile",
        ] {
            assert!(table.pass(pass).is_some(), "{pass}");
        }
        assert!(table.passes.iter().all(|p| if p.fixed {
            p.per_unit.is_some()
        } else {
            p.per_source_file.is_some()
        }));
        assert_eq!(table.measured_models(), vec!["deepseek-flash".to_string()]);
    }

    // spec: EST-4 (internal: a pure function; the CLI tests cover its display)
    #[test]
    fn costs_a_scaling_pass_per_source_file_and_a_fixed_one_per_run() {
        let estimate = estimate(&first_run(200), &table());
        let use_cases = of(&estimate, "use-cases");
        assert_eq!(use_cases.basis, Basis::PerSourceFile);
        assert_eq!(
            use_cases.calls,
            Range {
                low: 100,
                high: 300
            }
        );
        assert_eq!(
            use_cases.prompt_tokens,
            Range {
                low: 100_000,
                high: 300_000
            }
        );
        assert_eq!(
            use_cases.completion_tokens,
            Range {
                low: 10_000,
                high: 30_000
            }
        );
        let brief = of(&estimate, "brief");
        assert_eq!(brief.basis, Basis::PerRun);
        assert_eq!(brief.calls, Range { low: 1, high: 2 });
        assert_eq!(
            brief.prompt_tokens,
            Range {
                low: 1000,
                high: 2000
            }
        );
        assert_eq!(of(&estimate, "glossary").basis, Basis::NoCall);
        assert_eq!(of(&estimate, "glossary").calls, Range::default());
        assert_eq!(
            estimate.calls,
            Range {
                low: 121,
                high: 342
            }
        );
        assert_eq!(
            estimate
                .passes
                .iter()
                .map(|p| p.pass.as_str())
                .collect::<Vec<_>>(),
            PASSES
        );
    }

    // spec: EST-4 (internal: a pure function; the CLI tests cover its display)
    #[test]
    fn takes_no_bound_past_an_exact_product() {
        // In floating point 0.57 x 100 is 56.99999999999999 and 0.07 x 100 is 7.000000000000001.
        let yaml = TABLE_YAML.replace(
            r#""calls":{"median":0.15,"min":0.1,"max":0.2}"#,
            r#""calls":{"median":0.3,"min":0.57,"max":0.07}"#,
        );
        let table = RatioTable::parse(&yaml).unwrap();
        let estimate = estimate(&first_run(100), &table);
        assert_eq!(
            of(&estimate, "confidence").calls,
            Range { low: 57, high: 7 }
        );
    }

    // spec: EST-9 (internal: a pure function; the CLI tests cover its display)
    #[test]
    fn names_the_passes_the_table_has_no_ratio_for() {
        let estimate = estimate(&first_run(200), &table());
        assert_eq!(of(&estimate, "contexts").calls, Range::default());
        assert_eq!(estimate.unmeasured(), vec!["contexts"]);
    }

    // spec: EST-5 (internal: a pure function; the CLI tests cover its display)
    #[test]
    fn puts_a_pass_the_run_turns_off_at_zero() {
        let inputs = EstimateInputs {
            off: &["confidence"],
            ..first_run(200)
        };
        let estimate = estimate(&inputs, &table());
        let confidence = of(&estimate, "confidence");
        assert_eq!(confidence.basis, Basis::Off);
        assert_eq!(confidence.calls, Range::default());
        assert_eq!(confidence.prompt_tokens, Range::default());
    }

    // spec: EST-6 (internal: a pure function; the CLI tests cover its display)
    #[test]
    fn adds_the_confidence_sample_to_the_high_bound() {
        let inputs = EstimateInputs {
            confidence_sample: Some(5),
            ..first_run(100)
        };
        let confidence = of(&estimate(&inputs, &table()), "confidence").clone();
        assert_eq!(confidence.calls, Range { low: 10, high: 25 });

        // The costliest call of the applications not indicative: 300 prompt and 20 completion tokens.
        assert_eq!(
            confidence.prompt_tokens,
            Range {
                low: 1000,
                high: 6000 + 1500
            }
        );
        assert_eq!(
            confidence.completion_tokens,
            Range {
                low: 100,
                high: 600 + 100
            }
        );
    }

    // spec: EST-7 (internal: a pure function; the CLI tests cover the rerun)
    #[test]
    fn a_rerun_with_nothing_changed_costs_nothing() {
        let inputs = EstimateInputs {
            changed_files: Some(0),
            saved_fixed: &["brief"],
            ..first_run(200)
        };
        let estimate = estimate(&inputs, &table());
        assert_eq!(estimate.calls, Range::default());
        assert_eq!(estimate.prompt_tokens, Range::default());
        assert_eq!(estimate.completion_tokens, Range::default());
        assert_eq!(of(&estimate, "use-cases").basis, Basis::Cached);
        assert_eq!(of(&estimate, "brief").basis, Basis::Cached);
    }

    // spec: EST-8 (internal: a pure function; the CLI tests cover the rerun)
    #[test]
    fn a_rerun_costs_the_scaling_passes_on_the_changed_files_only() {
        let inputs = EstimateInputs {
            changed_files: Some(10),
            saved_fixed: &["brief"],
            ..first_run(200)
        };
        let estimate = estimate(&inputs, &table());
        assert_eq!(of(&estimate, "use-cases").calls, Range { low: 5, high: 15 });
        assert_eq!(of(&estimate, "brief").calls, Range::default());

        let unsaved = EstimateInputs {
            changed_files: Some(10),
            ..first_run(200)
        };
        assert_eq!(
            of(&estimate_of(&unsaved), "brief").calls,
            Range { low: 1, high: 2 }
        );
    }

    fn estimate_of(inputs: &EstimateInputs) -> RunEstimate {
        estimate(inputs, &table())
    }
}
