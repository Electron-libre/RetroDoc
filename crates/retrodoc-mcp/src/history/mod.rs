//! The run history (`specs/run-history.md`, ADR 0043): a snapshot of the figures and the sections
//! of each `generate` that changed the docs, one JSON file per run under
//! `.retrodoc/cache/history/`, all kept, so the pages of the history see how the docs moved after
//! the artifacts of the last run were rewritten. The figures are those of the audit page.

use std::collections::{BTreeMap, HashSet};
use std::io::{ErrorKind, Write as _};
use std::path::{Path, PathBuf};

use retrodoc_core::model::SetApartReason;
use retrodoc_pipeline::usage_log::RunUsage;
use retrodoc_pipeline::{Artifact, BehaviourCoverage};
use serde::{Deserialize, Serialize};

use crate::audit::{entry_coverage, EntryCoverage};
use crate::debt::{debt_of, docs_report, DebtKind};
use crate::read_model::GeneratedDocs;

mod diff;
pub use diff::{diff, Fall, RunDiff, Score, FALL_THRESHOLD};

/// The format of the snapshots written now; a reader skips a file it cannot read.
pub const VERSION: u32 = 1;

/// What a run left.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub version: u32,
    /// The commit of `HEAD` the run read, `None` with no commit.
    pub commit: Option<String>,
    /// RFC 3339.
    pub finished_at: String,
    /// `None` when the run called no LLM.
    pub cost: Option<Cost>,
    pub figures: Figures,
    /// The domains, then the features, then the use cases, each in the order of the run.
    pub sections: Vec<SectionScores>,
}

/// The cost of a run, summed over its passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cost {
    pub wall_ms: u64,
    pub calls: u64,
    pub prompt_tokens: u64,
    pub cached_tokens: u64,
    pub completion_tokens: u64,
}

/// The figures of the audit page known at the end of a `generate`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Figures {
    pub confidence: Option<f32>,
    pub business_language: Option<f32>,
    pub coverage: Option<EntryCoverage>,
    /// By kind in the order of the debt view, then by domain; no stale item.
    pub debt: Vec<DebtCount>,
}

/// The debt items of one kind in one domain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebtCount {
    /// The value of `?kind=` of the debt page.
    pub kind: String,
    /// `None` for the kinds of the code no doc covers.
    pub domain: Option<String>,
    pub count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SectionKind {
    Domain,
    Feature,
    UseCase,
}

/// The scores of a section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SectionScores {
    pub kind: SectionKind,
    /// The slug, `<feature>/<use case>` for a use case.
    pub id: String,
    pub name: String,
    pub confidence: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub business_language: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub narrative_check: Option<f32>,
    /// Why a use case is set apart under "Administration and technical".
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub set_apart: Vec<String>,
}

/// The snapshot of the run of `docs`, reconciled with the project's docs when it has some, that
/// read `commit`, ended at `finished_at` and cost `usage`.
#[must_use]
pub fn snapshot(
    docs: &GeneratedDocs,
    commit: Option<String>,
    finished_at: &str,
    usage: Option<&RunUsage>,
) -> Snapshot {
    let report = docs_report(docs);
    let (confidence, business_language) = (report.overall, report.overall_business);
    Snapshot {
        version: VERSION,
        commit,
        finished_at: finished_at.to_string(),
        cost: usage.map(cost),
        figures: Figures {
            confidence,
            business_language,
            coverage: docs
                .repo_root()
                .and_then(BehaviourCoverage::load)
                .and_then(|c| entry_coverage(docs, &c)),
            debt: debt_counts(docs, report),
        },
        sections: sections(docs),
    }
}

fn cost(usage: &RunUsage) -> Cost {
    let models = usage.passes.iter().flat_map(|p| &p.models);
    let sum = |field: fn(&retrodoc_pipeline::usage_log::ModelRecord) -> u64| {
        models.clone().map(field).sum()
    };
    Cost {
        wall_ms: usage.wall_ms,
        calls: sum(|m| m.calls),
        prompt_tokens: sum(|m| m.prompt_tokens),
        cached_tokens: sum(|m| m.cached_tokens),
        completion_tokens: sum(|m| m.completion_tokens),
    }
}

fn debt_counts(docs: &GeneratedDocs, report: retrodoc_pipeline::DebtReport) -> Vec<DebtCount> {
    let mut counts: BTreeMap<(DebtKind, Option<&str>), usize> = BTreeMap::new();
    let debt = debt_of(docs, report);
    for item in debt.items.iter().filter(|i| i.kind != DebtKind::Stale) {
        *counts.entry((item.kind, item.domain())).or_default() += 1;
    }
    counts
        .into_iter()
        .map(|((kind, domain), count)| DebtCount {
            kind: kind.key().to_string(),
            domain: domain.map(str::to_string),
            count,
        })
        .collect()
}

fn sections(docs: &GeneratedDocs) -> Vec<SectionScores> {
    let section = |kind, id: String, name: &str, confidence: Option<f32>| SectionScores {
        kind,
        id,
        name: name.to_string(),
        confidence,
        business_language: None,
        narrative_check: None,
        set_apart: Vec::new(),
    };
    let domains = docs.domains().iter().map(|d| {
        section(
            SectionKind::Domain,
            d.slug.clone(),
            &d.name,
            d.confidence.as_ref().map(|c| c.value),
        )
    });
    let features = docs.features().iter().map(|f| {
        section(
            SectionKind::Feature,
            f.slug.clone(),
            &f.name,
            f.confidence.as_ref().map(|c| c.value),
        )
    });
    let back_office: HashSet<&str> = docs
        .features()
        .iter()
        .filter(|f| f.back_office)
        .map(|f| f.slug.as_str())
        .collect();
    let use_cases = docs.use_cases().iter().map(|u| {
        let back_office = back_office.contains(u.feature_slug.as_str());
        SectionScores {
            business_language: u.business_language.as_ref().map(|c| c.value),
            narrative_check: u.narrative_check.as_ref().map(|c| c.value),
            set_apart: u
                .set_apart_reasons(back_office)
                .into_iter()
                .map(|r| reason_key(r).to_string())
                .collect(),
            ..section(
                SectionKind::UseCase,
                format!("{}/{}", u.feature_slug, u.slug),
                &u.name,
                u.confidence.as_ref().map(|c| c.value),
            )
        }
    });
    domains.chain(features).chain(use_cases).collect()
}

const REASONS: [SetApartReason; 3] = [
    SetApartReason::BackOffice,
    SetApartReason::BusinessLanguage,
    SetApartReason::Confidence,
];

fn reason_key(reason: SetApartReason) -> &'static str {
    match reason {
        SetApartReason::BackOffice => "back_office",
        SetApartReason::BusinessLanguage => "business_language",
        SetApartReason::Confidence => "confidence",
    }
}

/// The reason a key of [`SectionScores::set_apart`] names, `None` for a key of no reason.
#[must_use]
pub fn set_apart_reason(key: &str) -> Option<SetApartReason> {
    REASONS.into_iter().find(|r| reason_key(*r) == key)
}

/// Where the snapshots of `repo_root` are saved.
#[must_use]
pub fn history_dir(repo_root: &Path) -> PathBuf {
    Artifact::Usage.path(repo_root).with_file_name("history")
}

/// Saves `snapshot` as a new file named by its end date, unless it equals the last one saved but
/// for its commit, date and cost; never rewrites a saved one, and never leaves a file cut short. The
/// file written, `None` when none is.
///
/// # Errors
///
/// The folder or the file can't be written.
pub fn save(repo_root: &Path, snapshot: &Snapshot) -> std::io::Result<Option<PathBuf>> {
    let dir = history_dir(repo_root);
    // The newest file only: an unreadable one is not the run to compare with, so it never hides a
    // change behind an older snapshot.
    if files(&dir)
        .last()
        .and_then(|path| read(path))
        .is_some_and(|last| last.figures == snapshot.figures && last.sections == snapshot.sections)
    {
        return Ok(None);
    }
    std::fs::create_dir_all(&dir)?;
    let stem = file_stem(&snapshot.finished_at);
    // Written whole aside, under a name of this process, then linked under a name no file holds yet.
    let draft = dir.join(format!(".{stem}.{}.tmp", std::process::id()));
    let mut file = std::fs::File::create(&draft)?;
    file.write_all(
        serde_json::to_string_pretty(snapshot)
            .map_err(std::io::Error::other)?
            .as_bytes(),
    )?;
    file.sync_all()?;
    let linked = link_aside(&draft, &dir, &stem);
    std::fs::remove_file(&draft)?;
    linked.map(Some)
}

/// Links `draft` under the first name of the runs of `stem` no file of `dir` holds.
fn link_aside(draft: &Path, dir: &Path, stem: &str) -> std::io::Result<PathBuf> {
    for n in 1..=u32::MAX {
        let path = dir.join(file_name(stem, n));
        match std::fs::hard_link(draft, &path) {
            Ok(()) => return Ok(path),
            Err(e) if e.kind() == ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
    }
    Err(ErrorKind::AlreadyExists.into())
}

/// The `n`-th file of the runs ended in the second of `stem`, from 1.
fn file_name(stem: &str, n: u32) -> String {
    if n == 1 {
        format!("{stem}.json")
    } else {
        format!("{stem}_{n}.json")
    }
}

/// `2026-11-04T10:00:00.123+00:00` → `20261104T100000Z`: the date and time to the second.
fn file_stem(finished_at: &str) -> String {
    let digits: String = finished_at
        .chars()
        .filter(char::is_ascii_digit)
        .take(14)
        .collect();
    let (date, time) = digits.split_at(digits.len().min(8));
    format!("{date}T{time}Z")
}

/// The snapshot files of `dir`, oldest first: by the date of their name, then by their number.
fn files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<(String, u32, PathBuf)> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter_map(|path| {
            let stem = path
                .file_name()?
                .to_str()?
                .strip_suffix(".json")?
                .to_string();
            Some(match stem.split_once('_') {
                Some((date, n)) => (date.to_string(), n.parse().ok()?, path),
                None => (stem, 1, path),
            })
        })
        .collect();
    files.sort();
    files.into_iter().map(|(_, _, path)| path).collect()
}

/// The snapshot of `path`, `None` when it cannot be read or is of a later format.
fn read(path: &Path) -> Option<Snapshot> {
    let snapshot: Snapshot = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    (snapshot.version <= VERSION).then_some(snapshot)
}

/// A saved snapshot and the name of its run.
#[derive(Debug, Clone, PartialEq)]
pub struct SavedRun {
    /// The name of its file without `.json`, unique among the saved runs.
    pub name: String,
    pub snapshot: Snapshot,
}

/// The saved snapshots, oldest first; a file that cannot be read, or of a later format, is left out.
#[must_use]
pub fn load(repo_root: &Path) -> Vec<SavedRun> {
    files(&history_dir(repo_root))
        .iter()
        .filter_map(|path| {
            Some(SavedRun {
                name: path.file_stem()?.to_str()?.to_string(),
                snapshot: read(path)?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests;
