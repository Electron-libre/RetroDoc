//! The calls and tokens of each LLM pass of `generate` per unit of its input, measured on saved runs with no
//! LLM call (`issues/pass_ratios_from_usage_history.md`), for the pre-run estimate.
//!
//! A run is a directory whose `.retrodoc/cache/` holds the `usage.json` of its `generate` and the artifacts
//! of the same run. Only a first run measures a ratio (a rerun reuses its cached units and calls nothing for
//! them): its first `generate` must have called `sources`, and no pass of the removed `repo-map`. A run resumed
//! over several `generate` merges the first [`RunEntry::generates`] of them.
//!
//! Each pass scales with one unit, counted from the artifacts ([`Unit`]); a pass that scales with nothing is a
//! fixed cost, measured per run. The ratios of an application are its sums over its runs divided by its units;
//! the table gives their median and range across the applications not marked `indicative`, then the yields the
//! estimate extrapolates (entry points per file read, features per domain, use cases per feature…).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_yaml::Value;

use crate::usage_log::{load_history, RunUsage};

/// The runs to measure (a file kept next to them, outside git: it names their paths).
#[derive(Debug, Clone, Deserialize)]
pub struct RunManifest {
    pub runs: Vec<RunEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RunEntry {
    /// The run directory (relative to the manifest's directory, or absolute).
    pub path: PathBuf,
    /// A generic description of the application (no private name).
    pub application: String,
    /// The commit of `RetroDoc` that made the run.
    pub commit: String,
    /// How many `generate` of the history, from the first, make the run (more than one: a resumed run).
    #[serde(default = "one")]
    pub generates: usize,
    /// Shown per application but left out of the median and the range.
    #[serde(default)]
    pub indicative: bool,
}

fn one() -> usize {
    1
}

impl RunManifest {
    /// Reads the manifest and resolves the run paths against its directory.
    ///
    /// # Errors
    ///
    /// Returns a message when the file can't be read or parsed.
    pub fn load(path: &Path) -> Result<Self, String> {
        let raw = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut manifest: Self =
            serde_yaml::from_str(&raw).map_err(|e| format!("{}: {e}", path.display()))?;
        let base = path.parent().unwrap_or(Path::new("."));
        for run in &mut manifest.runs {
            run.path = base.join(&run.path);
        }
        Ok(manifest)
    }
}

/// What a pass's calls scale with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Unit {
    /// One `generate`: a fixed cost.
    Run,
    /// 1,000 characters of the business files read (`glossary.yaml`).
    BusinessKchars,
    /// 1,000 characters of the entry point files read outside the business files.
    EntryPointKchars,
    /// A domain of `domains.yaml`, `uncategorized` included.
    Domain,
    /// A feature of `features.yaml`.
    Feature,
    /// A use case of `use-cases.yaml`.
    UseCase,
    /// An item of `reconciliation.yaml` (an item matched to a section of the project's docs).
    ReconcileItem,
}

impl Unit {
    /// The unit of a pass, by its name in the usage history.
    #[must_use]
    pub fn of_pass(pass: &str) -> Self {
        match pass {
            "business-read" => Self::BusinessKchars,
            "entry-points" => Self::EntryPointKchars,
            "features" => Self::Domain,
            "use-cases" | "rules" => Self::Feature,
            "confidence" => Self::UseCase,
            "reconcile" => Self::ReconcileItem,
            _ => Self::Run,
        }
    }

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Run => "run",
            Self::BusinessKchars => "1,000 characters of business files read",
            Self::EntryPointKchars => "1,000 characters of entry point files read",
            Self::Domain => "domain",
            Self::Feature => "feature",
            Self::UseCase => "use case",
            Self::ReconcileItem => "reconciled item",
        }
    }
}

/// The units of one run, counted from its artifacts.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Units {
    pub business_files: u64,
    pub business_chars: u64,
    pub entry_point_files: u64,
    pub entry_point_chars: u64,
    /// Files of the two reads missing from the run's tree: their characters are not counted.
    pub missing_files: u64,
    pub entry_points: u64,
    pub domains: u64,
    pub features: u64,
    pub use_cases: u64,
    pub reconcile_items: u64,
}

impl Units {
    fn of(&self, unit: Unit) -> f64 {
        #[allow(clippy::cast_precision_loss)]
        match unit {
            Unit::Run => 1.0,
            Unit::BusinessKchars => self.business_chars as f64 / 1000.0,
            Unit::EntryPointKchars => self.entry_point_chars as f64 / 1000.0,
            Unit::Domain => self.domains as f64,
            Unit::Feature => self.features as f64,
            Unit::UseCase => self.use_cases as f64,
            Unit::ReconcileItem => self.reconcile_items as f64,
        }
    }

    fn add(&mut self, other: &Self) {
        self.business_files += other.business_files;
        self.business_chars += other.business_chars;
        self.entry_point_files += other.entry_point_files;
        self.entry_point_chars += other.entry_point_chars;
        self.missing_files += other.missing_files;
        self.entry_points += other.entry_points;
        self.domains += other.domains;
        self.features += other.features;
        self.use_cases += other.use_cases;
        self.reconcile_items += other.reconcile_items;
    }
}

/// The calls and tokens of one pass, every model summed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct PassCost {
    pub calls: u64,
    /// Calls the server reported no usage for: the token sums are then a lower bound.
    pub calls_without_usage: u64,
    pub prompt_tokens: u64,
    pub cached_tokens: u64,
    pub completion_tokens: u64,
}

impl PassCost {
    fn add(&mut self, other: &Self) {
        self.calls += other.calls;
        self.calls_without_usage += other.calls_without_usage;
        self.prompt_tokens += other.prompt_tokens;
        self.cached_tokens += other.cached_tokens;
        self.completion_tokens += other.completion_tokens;
    }
}

/// One measured run.
#[derive(Debug, Clone, PartialEq)]
pub struct RunMeasure {
    pub application: String,
    pub commit: String,
    pub indicative: bool,
    pub generates: usize,
    pub finished_at: String,
    pub models: Vec<String>,
    /// In the order of the pipeline.
    pub passes: Vec<(String, PassCost)>,
    pub units: Units,
}

/// Measures one run.
///
/// # Errors
///
/// Returns a message when the run is no first complete `generate` (no history, a rerun, a `repo-map` pass)
/// or an artifact can't be read.
pub fn measure_run(entry: &RunEntry) -> Result<RunMeasure, String> {
    let at = entry.path.display();
    let generates: Vec<RunUsage> = load_history(&entry.path)
        .into_iter()
        .filter(|run| run.command == "generate")
        .take(entry.generates.max(1))
        .collect();
    let Some(first) = generates.first() else {
        return Err(format!("{at}: no generate in its usage history"));
    };
    let sources_called = first
        .passes
        .iter()
        .any(|pass| pass.name == "sources" && pass.models.iter().any(|m| m.calls > 0));
    if !sources_called {
        return Err(format!(
            "{at}: its first generate called no `sources`: a rerun, not a first run"
        ));
    }
    let mut passes: Vec<(String, PassCost)> = Vec::new();
    let mut models = BTreeSet::new();
    for run in &generates {
        for pass in &run.passes {
            if pass.name == "repo-map" {
                return Err(format!("{at}: it ran the removed `repo-map` pass"));
            }
            let mut cost = PassCost::default();
            for model in &pass.models {
                models.insert(model.model.clone());
                cost.add(&PassCost {
                    calls: model.calls,
                    calls_without_usage: model.calls_without_usage,
                    prompt_tokens: model.prompt_tokens,
                    cached_tokens: model.cached_tokens,
                    completion_tokens: model.completion_tokens,
                });
            }
            match passes.iter_mut().find(|(name, _)| *name == pass.name) {
                Some((_, total)) => total.add(&cost),
                None => passes.push((pass.name.clone(), cost)),
            }
        }
    }
    Ok(RunMeasure {
        application: entry.application.clone(),
        commit: entry.commit.clone(),
        indicative: entry.indicative,
        generates: generates.len(),
        finished_at: generates
            .last()
            .map(|run| run.finished_at.clone())
            .unwrap_or_default(),
        models: models.into_iter().collect(),
        passes,
        units: count_units(&entry.path)?,
    })
}

fn load_yaml(cache: &Path, name: &str) -> Result<Option<Value>, String> {
    let path = cache.join(name);
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_yaml::from_str(&raw)
        .map(Some)
        .map_err(|e| format!("{}: {e}", path.display()))
}

fn keys(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_mapping)
        .map(|map| {
            map.keys()
                .filter_map(|k| k.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn list_len(value: Option<&Value>) -> u64 {
    value
        .and_then(Value::as_sequence)
        .map_or(0, |items| items.len() as u64)
}

/// Counts the units of a run from its `.retrodoc/cache/` and the characters of the files read from its tree.
fn count_units(root: &Path) -> Result<Units, String> {
    let cache = root.join(".retrodoc").join("cache");
    let glossary = load_yaml(&cache, "glossary.yaml")?;
    let entry_points = load_yaml(&cache, "entry-points.yaml")?;
    let business: BTreeSet<String> = keys(glossary.as_ref().and_then(|g| g.get("models")))
        .into_iter()
        .collect();
    let entry_point_files = entry_points.as_ref().and_then(|e| e.get("files"));
    let outside: Vec<String> = keys(entry_point_files)
        .into_iter()
        .filter(|file| !business.contains(file))
        .collect();
    let mut units = Units {
        business_files: business.len() as u64,
        entry_point_files: outside.len() as u64,
        ..Units::default()
    };
    for file in &business {
        match std::fs::read_to_string(root.join(file)) {
            Ok(text) => units.business_chars += text.chars().count() as u64,
            Err(_) => units.missing_files += 1,
        }
    }
    for file in &outside {
        match std::fs::read_to_string(root.join(file)) {
            Ok(text) => units.entry_point_chars += text.chars().count() as u64,
            Err(_) => units.missing_files += 1,
        }
    }
    units.entry_points = entry_point_files
        .and_then(Value::as_mapping)
        .map_or(0, |files| {
            files
                .values()
                .map(|file| list_len(file.get("entry_points")))
                .sum()
        });
    units.domains = list_len(
        load_yaml(&cache, "domains.yaml")?
            .as_ref()
            .and_then(|d| d.get("domains")),
    );
    units.features = list_len(load_yaml(&cache, "features.yaml")?.as_ref());
    units.use_cases = list_len(load_yaml(&cache, "use-cases.yaml")?.as_ref());
    units.reconcile_items = list_len(
        load_yaml(&cache, "reconciliation.yaml")?
            .as_ref()
            .and_then(|r| r.get("items")),
    );
    Ok(units)
}

/// The median and the range of a ratio across applications.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Spread {
    pub median: f64,
    pub min: f64,
    pub max: f64,
}

impl Spread {
    fn of(values: &[f64]) -> Option<Self> {
        let mut sorted: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
        if sorted.is_empty() {
            return None;
        }
        sorted.sort_by(f64::total_cmp);
        let mid = sorted.len() / 2;
        let median = if sorted.len().is_multiple_of(2) {
            f64::midpoint(sorted[mid - 1], sorted[mid])
        } else {
            sorted[mid]
        };
        Some(Self {
            median: round(median),
            min: round(sorted[0]),
            max: round(sorted[sorted.len() - 1]),
        })
    }
}

/// Three decimals: the table is rebuilt identical from the same runs.
fn round(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

/// Calls and tokens per unit.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct PerUnit<T> {
    pub calls: T,
    pub prompt_tokens: T,
    pub cached_tokens: T,
    pub completion_tokens: T,
}

/// One application's sums for one pass.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ApplicationRatio {
    pub application: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub indicative: bool,
    pub runs: usize,
    pub units: f64,
    #[serde(flatten)]
    pub cost: PassCost,
    /// `None` when the application has none of the unit.
    pub per_unit: Option<PerUnit<f64>>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PassRatio {
    pub pass: String,
    pub unit: &'static str,
    pub fixed: bool,
    /// Across the applications not marked indicative; `None` when none has the unit.
    pub per_unit: Option<PerUnit<Spread>>,
    pub applications: Vec<ApplicationRatio>,
}

/// A yield of the artifacts the estimate extrapolates, per application then across them.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct YieldRatio {
    pub name: &'static str,
    pub spread: Option<Spread>,
    pub applications: BTreeMap<String, f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RunSource {
    pub application: String,
    pub commit: String,
    pub models: Vec<String>,
    pub finished_at: String,
    #[serde(skip_serializing_if = "is_one")]
    pub generates: usize,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub indicative: bool,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_one(n: &usize) -> bool {
    *n == 1
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RatioTable {
    pub runs: Vec<RunSource>,
    pub passes: Vec<PassRatio>,
    /// Passes every run recorded with no call.
    pub no_call: Vec<String>,
    pub yields: Vec<YieldRatio>,
}

struct Application<'a> {
    name: &'a str,
    indicative: bool,
    runs: Vec<&'a RunMeasure>,
}

fn applications(runs: &[RunMeasure]) -> Vec<Application<'_>> {
    let mut apps: Vec<Application> = Vec::new();
    for run in runs {
        match apps.iter_mut().find(|app| app.name == run.application) {
            Some(app) => {
                app.indicative &= run.indicative;
                app.runs.push(run);
            }
            None => apps.push(Application {
                name: &run.application,
                indicative: run.indicative,
                runs: vec![run],
            }),
        }
    }
    apps
}

#[allow(clippy::cast_precision_loss)]
fn per_unit(cost: &PassCost, units: f64) -> Option<PerUnit<f64>> {
    (units > 0.0).then(|| PerUnit {
        calls: round(cost.calls as f64 / units),
        prompt_tokens: round(cost.prompt_tokens as f64 / units),
        cached_tokens: round(cost.cached_tokens as f64 / units),
        completion_tokens: round(cost.completion_tokens as f64 / units),
    })
}

fn spread_of(rows: &[&ApplicationRatio]) -> Option<PerUnit<Spread>> {
    let pick = |f: fn(&PerUnit<f64>) -> f64| -> Option<Spread> {
        let values: Vec<f64> = rows
            .iter()
            .filter_map(|r| r.per_unit.as_ref().map(f))
            .collect();
        Spread::of(&values)
    };
    Some(PerUnit {
        calls: pick(|p| p.calls)?,
        prompt_tokens: pick(|p| p.prompt_tokens)?,
        cached_tokens: pick(|p| p.cached_tokens)?,
        completion_tokens: pick(|p| p.completion_tokens)?,
    })
}

#[allow(clippy::cast_precision_loss)]
fn ratio(numerator: u64, denominator: u64) -> Option<f64> {
    (denominator > 0).then(|| round(numerator as f64 / denominator as f64))
}

/// A yield from the units of an application's runs summed, and the number of those runs.
type YieldOf = fn(&Units, u64) -> Option<f64>;

const YIELDS: &[(&str, YieldOf)] = &[
    ("characters per business file", |u, _| {
        ratio(u.business_chars, u.business_files)
    }),
    ("characters per entry point file", |u, _| {
        ratio(u.entry_point_chars, u.entry_point_files)
    }),
    ("entry points per file read", |u, _| {
        ratio(u.entry_points, u.business_files + u.entry_point_files)
    }),
    ("domains per run", |u, runs| ratio(u.domains, runs)),
    ("entry points per domain", |u, _| {
        ratio(u.entry_points, u.domains)
    }),
    ("features per domain", |u, _| ratio(u.features, u.domains)),
    ("use cases per feature", |u, _| {
        ratio(u.use_cases, u.features)
    }),
    ("reconciled items per feature", |u, _| {
        if u.reconcile_items == 0 {
            None
        } else {
            ratio(u.reconcile_items, u.features)
        }
    }),
];

/// Builds the table from measured runs, in their order.
#[must_use]
pub fn ratio_table(runs: &[RunMeasure]) -> RatioTable {
    let apps = applications(runs);
    let mut names: Vec<&str> = Vec::new();
    for run in runs {
        for (name, _) in &run.passes {
            if !names.contains(&name.as_str()) {
                names.push(name);
            }
        }
    }
    let mut passes = Vec::new();
    let mut no_call = Vec::new();
    for name in names {
        let called = runs
            .iter()
            .flat_map(|run| &run.passes)
            .any(|(pass, cost)| pass == name && cost.calls > 0);
        if !called {
            no_call.push(name.to_string());
            continue;
        }
        let unit = Unit::of_pass(name);
        // Only the runs that ran the pass count: one turned off (`rules = false`) is absent from the history.
        let rows: Vec<ApplicationRatio> = apps
            .iter()
            .filter_map(|app| {
                let mut cost = PassCost::default();
                let mut units = 0.0;
                let mut ran = 0;
                for run in &app.runs {
                    if let Some((_, c)) = run.passes.iter().find(|(pass, _)| pass == name) {
                        cost.add(c);
                        units += run.units.of(unit);
                        ran += 1;
                    }
                }
                (ran > 0).then(|| ApplicationRatio {
                    application: app.name.to_string(),
                    indicative: app.indicative,
                    runs: ran,
                    units: round(units),
                    per_unit: per_unit(&cost, units),
                    cost,
                })
            })
            .collect();
        let counted: Vec<&ApplicationRatio> = rows.iter().filter(|r| !r.indicative).collect();
        passes.push(PassRatio {
            pass: name.to_string(),
            unit: unit.label(),
            fixed: unit == Unit::Run,
            per_unit: spread_of(&counted),
            applications: rows,
        });
    }
    let yields = YIELDS
        .iter()
        .map(|(name, of)| {
            let mut per_app = BTreeMap::new();
            let mut counted = Vec::new();
            for app in &apps {
                let mut units = Units::default();
                for run in &app.runs {
                    units.add(&run.units);
                }
                let value = of(&units, app.runs.len() as u64);
                if let Some(value) = value {
                    per_app.insert(app.name.to_string(), value);
                    if !app.indicative {
                        counted.push(value);
                    }
                }
            }
            YieldRatio {
                name,
                spread: Spread::of(&counted),
                applications: per_app,
            }
        })
        .collect();
    RatioTable {
        runs: runs
            .iter()
            .map(|run| RunSource {
                application: run.application.clone(),
                commit: run.commit.clone(),
                models: run.models.clone(),
                finished_at: run.finished_at.clone(),
                generates: run.generates,
                indicative: run.indicative,
            })
            .collect(),
        passes,
        no_call,
        yields,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn pass(
        name: &str,
        calls: u64,
        without: u64,
        prompt: u64,
        cached: u64,
        out: u64,
    ) -> serde_json::Value {
        json!({"name": name, "wall_ms": 1, "models": [{"model": "m", "calls": calls,
            "calls_without_usage": without, "prompt_tokens": prompt, "completion_tokens": out,
            "cached_tokens": cached}]})
    }

    fn generate(passes: &[serde_json::Value]) -> serde_json::Value {
        json!({"command": "generate", "finished_at": "2026-11-03T11:00:00Z", "wall_ms": 1, "passes": passes})
    }

    /// A run of 2 business files (4,000 characters), 1 entry point file outside them (1,000), 3 entry
    /// points, 2 domains, 4 features, 8 use cases, 5 reconciled items.
    fn run_dir(history: &[serde_json::Value]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join(".retrodoc/cache");
        std::fs::create_dir_all(&cache).unwrap();
        std::fs::create_dir_all(dir.path().join("app")).unwrap();
        std::fs::write(dir.path().join("app/a.rb"), "a".repeat(3000)).unwrap();
        std::fs::write(dir.path().join("app/b.rb"), "é".repeat(1000)).unwrap();
        std::fs::write(dir.path().join("app/c.rb"), "c".repeat(1000)).unwrap();
        std::fs::write(
            cache.join("usage.json"),
            json!({"runs": history}).to_string(),
        )
        .unwrap();
        std::fs::write(
            cache.join("glossary.yaml"),
            "models:\n  app/a.rb:\n    entities: []\n  app/b.rb:\n    entities: []\ntests: {}\n",
        )
        .unwrap();
        std::fs::write(
            cache.join("entry-points.yaml"),
            "files:\n  app/a.rb:\n    entry_points: [{name: x}, {name: y}]\n  app/c.rb:\n    entry_points: [{name: z}]\n",
        )
        .unwrap();
        std::fs::write(
            cache.join("domains.yaml"),
            "domains: [{slug: a}, {slug: uncategorized}]\n",
        )
        .unwrap();
        std::fs::write(
            cache.join("features.yaml"),
            "[{slug: 1}, {slug: 2}, {slug: 3}, {slug: 4}]\n",
        )
        .unwrap();
        std::fs::write(cache.join("use-cases.yaml"), "[1, 2, 3, 4, 5, 6, 7, 8]\n").unwrap();
        std::fs::write(
            cache.join("reconciliation.yaml"),
            "items: [1, 2, 3, 4, 5]\n",
        )
        .unwrap();
        dir
    }

    fn entry(dir: &Path, application: &str) -> RunEntry {
        RunEntry {
            path: dir.to_path_buf(),
            application: application.to_string(),
            commit: "abc1234".to_string(),
            generates: 1,
            indicative: false,
        }
    }

    fn first_run() -> Vec<serde_json::Value> {
        vec![generate(&[
            pass("sources", 1, 0, 100, 0, 10),
            pass("business-read", 4, 1, 8000, 2000, 400),
            pass("glossary", 0, 0, 0, 0, 0),
            pass("entry-points", 1, 0, 900, 0, 90),
            pass("features", 3, 0, 600, 0, 60),
            pass("use-cases", 4, 0, 4000, 400, 800),
            pass("confidence", 2, 0, 800, 0, 16),
            pass("reconcile", 1, 0, 500, 0, 25),
        ])]
    }

    #[test]
    fn counts_the_units_of_a_run_from_its_artifacts_and_tree() {
        let dir = run_dir(&first_run());
        let run = measure_run(&entry(dir.path(), "app")).unwrap();
        assert_eq!(
            run.units,
            Units {
                business_files: 2,
                business_chars: 4000,
                entry_point_files: 1,
                entry_point_chars: 1000,
                missing_files: 0,
                entry_points: 3,
                domains: 2,
                features: 4,
                use_cases: 8,
                reconcile_items: 5,
            }
        );
    }

    #[test]
    fn gives_each_pass_its_cost_per_unit_and_marks_the_fixed_ones() {
        let dir = run_dir(&first_run());
        let table = ratio_table(&[measure_run(&entry(dir.path(), "app")).unwrap()]);
        let of = |name: &str| table.passes.iter().find(|p| p.pass == name).unwrap();

        let read = of("business-read");
        assert_eq!(read.unit, "1,000 characters of business files read");
        assert!(!read.fixed);
        let spread = read.per_unit.unwrap();
        assert_eq!(spread.calls.median, 1.0);
        assert_eq!(spread.prompt_tokens.median, 2000.0);
        assert_eq!(spread.cached_tokens.median, 500.0);
        assert_eq!(spread.completion_tokens.median, 100.0);
        assert_eq!(read.applications[0].cost.calls_without_usage, 1);

        assert_eq!(of("entry-points").per_unit.unwrap().calls.median, 1.0);
        assert_eq!(of("features").per_unit.unwrap().calls.median, 1.5);
        assert_eq!(of("use-cases").per_unit.unwrap().calls.median, 1.0);
        assert_eq!(of("confidence").per_unit.unwrap().calls.median, 0.25);
        assert_eq!(of("reconcile").per_unit.unwrap().calls.median, 0.2);
        let sources = of("sources");
        assert!(sources.fixed);
        assert_eq!(sources.per_unit.unwrap().prompt_tokens.median, 100.0);
        assert_eq!(table.no_call, vec!["glossary".to_string()]);

        let yield_of = |name: &str| {
            table
                .yields
                .iter()
                .find(|y| y.name == name)
                .unwrap()
                .spread
                .unwrap()
                .median
        };
        assert_eq!(yield_of("entry points per file read"), 1.0);
        assert_eq!(yield_of("features per domain"), 2.0);
        assert_eq!(yield_of("use cases per feature"), 2.0);
    }

    #[test]
    fn refuses_a_rerun_and_a_run_of_the_removed_repo_map() {
        let rerun = run_dir(&[generate(&[pass("use-cases", 1, 0, 10, 0, 1)])]);
        let error = measure_run(&entry(rerun.path(), "app")).unwrap_err();
        assert!(error.contains("a rerun"), "{error}");

        let mut passes = first_run()[0]["passes"].as_array().unwrap().clone();
        passes.push(pass("repo-map", 2, 0, 10, 0, 1));
        let old = run_dir(&[generate(&passes)]);
        let error = measure_run(&entry(old.path(), "app")).unwrap_err();
        assert!(error.contains("repo-map"), "{error}");
    }

    #[test]
    fn merges_a_resumed_run_and_ignores_the_later_reruns() {
        let mut history = first_run();
        history.push(generate(&[pass("use-cases", 4, 0, 4000, 0, 800)]));
        history.push(generate(&[pass("use-cases", 9, 0, 9, 0, 9)]));
        let dir = run_dir(&history);
        let mut resumed = entry(dir.path(), "app");
        resumed.generates = 2;
        let run = measure_run(&resumed).unwrap();
        let use_cases = run
            .passes
            .iter()
            .find(|(name, _)| name == "use-cases")
            .unwrap();
        assert_eq!(use_cases.1.calls, 8);
        assert_eq!(run.generates, 2);
    }

    #[test]
    fn takes_the_median_across_applications_without_the_indicative_one() {
        let small = run_dir(&first_run());
        let mut doubled = first_run();
        doubled[0]["passes"][4] = pass("features", 6, 0, 1200, 0, 120);
        let large = run_dir(&doubled);
        let mut huge = first_run();
        huge[0]["passes"][4] = pass("features", 60, 0, 1, 0, 1);
        let indicative = run_dir(&huge);
        let mut last = entry(indicative.path(), "large");
        last.indicative = true;
        let runs = [
            measure_run(&entry(small.path(), "small")).unwrap(),
            measure_run(&entry(large.path(), "medium")).unwrap(),
            measure_run(&last).unwrap(),
        ];
        let table = ratio_table(&runs);
        let features = table.passes.iter().find(|p| p.pass == "features").unwrap();
        let calls = features.per_unit.unwrap().calls;
        assert_eq!((calls.median, calls.min, calls.max), (2.25, 1.5, 3.0));
        assert_eq!(features.applications.len(), 3);
        assert!(features.applications[2].indicative);
        assert_eq!(features.applications[2].per_unit.unwrap().calls, 30.0);
    }

    #[test]
    fn leaves_out_of_a_pass_the_runs_that_turned_it_off() {
        let mut with_rules = first_run();
        with_rules[0]["passes"]
            .as_array_mut()
            .unwrap()
            .push(pass("rules", 8, 0, 800, 0, 80));
        let on = run_dir(&with_rules);
        let off = run_dir(&first_run());
        let runs = [
            measure_run(&entry(on.path(), "app")).unwrap(),
            measure_run(&entry(off.path(), "app")).unwrap(),
            measure_run(&entry(off.path(), "other")).unwrap(),
        ];
        let table = ratio_table(&runs);
        let rules = table.passes.iter().find(|p| p.pass == "rules").unwrap();
        assert_eq!(rules.applications.len(), 1);
        assert_eq!(rules.applications[0].runs, 1);
        assert_eq!(rules.per_unit.unwrap().calls.median, 2.0);
    }
}
