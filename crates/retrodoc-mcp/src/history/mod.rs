//! The run history (`specs/run-history.md`, ADR 0043): a snapshot of the figures and the sections
//! of each `generate` that changed the docs, one JSON file per run under
//! `.retrodoc/cache/history/`, all kept, so the pages of the history see how the docs moved after
//! the artifacts of the last run were rewritten. The figures are those of the audit page.

use std::collections::BTreeMap;
use std::io::{ErrorKind, Write as _};
use std::path::{Path, PathBuf};

use retrodoc_core::model::SetApartReason;
use retrodoc_pipeline::usage_log::RunUsage;
use retrodoc_pipeline::{Artifact, BehaviourCoverage};
use serde::{Deserialize, Serialize};

use crate::audit::{entry_coverage, EntryCoverage};
use crate::debt::{debt_of, docs_report, DebtKind};
use crate::read_model::GeneratedDocs;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
    let mut counts: BTreeMap<(usize, Option<&str>), usize> = BTreeMap::new();
    let debt = debt_of(docs, report);
    for item in debt.items.iter().filter(|i| i.kind != DebtKind::Stale) {
        let kind = DebtKind::ALL.iter().position(|k| *k == item.kind);
        *counts
            .entry((kind.unwrap_or(usize::MAX), item.domain()))
            .or_default() += 1;
    }
    counts
        .into_iter()
        .map(|((kind, domain), count)| DebtCount {
            kind: DebtKind::ALL[kind].key().to_string(),
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
    let use_cases = docs.use_cases().iter().map(|u| {
        let back_office = docs
            .features()
            .iter()
            .any(|f| f.slug == u.feature_slug && f.back_office);
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

fn reason_key(reason: SetApartReason) -> &'static str {
    match reason {
        SetApartReason::BackOffice => "back_office",
        SetApartReason::BusinessLanguage => "business_language",
        SetApartReason::Confidence => "confidence",
    }
}

/// Where the snapshots of `repo_root` are saved.
#[must_use]
pub fn history_dir(repo_root: &Path) -> PathBuf {
    Artifact::Usage.path(repo_root).with_file_name("history")
}

/// Saves `snapshot` as a new file named by its end date, unless it equals the last one saved but
/// for its commit, date and cost; never rewrites a saved one. The file written, `None` when none is.
///
/// # Errors
///
/// The folder or the file can't be written.
pub fn save(repo_root: &Path, snapshot: &Snapshot) -> std::io::Result<Option<PathBuf>> {
    if load(repo_root)
        .last()
        .is_some_and(|last| last.figures == snapshot.figures && last.sections == snapshot.sections)
    {
        return Ok(None);
    }
    let dir = history_dir(repo_root);
    std::fs::create_dir_all(&dir)?;
    let json = serde_json::to_string_pretty(snapshot).map_err(std::io::Error::other)?;
    let stem = file_stem(&snapshot.finished_at);
    for n in 1.. {
        let path = if n == 1 {
            dir.join(format!("{stem}.json"))
        } else {
            dir.join(format!("{stem}_{n}.json"))
        };
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                file.write_all(json.as_bytes())?;
                return Ok(Some(path));
            }
            Err(e) if e.kind() == ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
    }
    unreachable!("an unbounded range never ends")
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

/// The saved snapshots, oldest first; a file that cannot be read is left out.
#[must_use]
pub fn load(repo_root: &Path) -> Vec<Snapshot> {
    let Ok(dir) = std::fs::read_dir(history_dir(repo_root)) else {
        return Vec::new();
    };
    let mut files: Vec<(Snapshot, String)> = dir
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| {
            let text = std::fs::read_to_string(e.path()).ok()?;
            let snapshot: Snapshot = serde_json::from_str(&text).ok()?;
            Some((snapshot, e.file_name().to_string_lossy().into_owned()))
        })
        .collect();
    // Two runs of the same second: `X.json` before `X_2.json`, whatever the clock said.
    files.sort_by(|(a, a_name), (b, b_name)| {
        (&a.finished_at, a_name.len(), a_name).cmp(&(&b.finished_at, b_name.len(), b_name))
    });
    files.into_iter().map(|(snapshot, _)| snapshot).collect()
}

#[cfg(test)]
mod tests;
