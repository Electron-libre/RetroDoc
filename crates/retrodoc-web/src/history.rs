//! What the history page shows (`specs/history-page.md`): the runs the history saved, newest
//! first, and the difference of two of them from `retrodoc_mcp::history::diff`, each section linked
//! to its page when the docs served hold it.

use std::collections::{HashMap, HashSet};

use retrodoc_core::model::{Feature, UseCase};
use retrodoc_mcp::history::{
    diff, set_apart_reason, Fall, SavedRun, Score, SectionKind, SectionScores, Snapshot,
};
use retrodoc_mcp::GeneratedDocs;

use crate::texts::WebTexts;
use crate::view::{domain_href, feature_href, use_case_href};

pub struct HistoryPage {
    /// Newest first.
    pub runs: Vec<RunView>,
    /// The values of `?from=` and `?to=` that name no saved run.
    pub unknown: Vec<String>,
    /// `None` with fewer than two runs.
    pub compared: Option<Compared>,
}

pub struct RunView {
    /// The name of its file, the value of `?from=` and `?to=`.
    pub name: String,
    /// `YYYY-MM-DD HH:MM:SS`.
    pub finished: String,
    /// The first 7 characters, `None` with no commit.
    pub commit: Option<String>,
    pub from: bool,
    pub to: bool,
}

/// The difference of the two compared runs, by group; a group with no section is left out.
pub struct Compared {
    pub groups: Vec<ChangeGroup>,
}

pub struct ChangeGroup {
    /// The `id` of its section in the page.
    pub id: &'static str,
    pub heading: &'static str,
    pub sections: Vec<ChangedSection>,
}

pub struct ChangedSection {
    pub kind: &'static str,
    pub name: String,
    /// Its page in the docs served, `None` when they don't hold it.
    pub href: Option<String>,
    /// The scores that fell, or the reasons it is set apart.
    pub detail: Option<String>,
}

/// The history page of `runs` (oldest first, as `history::load` gives them), comparing the runs
/// `from` and `to` name, each else its default.
#[must_use]
pub fn history_page(
    runs: &[SavedRun],
    docs: Option<&GeneratedDocs>,
    from: Option<&str>,
    to: Option<&str>,
    texts: &WebTexts,
) -> HistoryPage {
    let mut unknown = Vec::new();
    let mut find = |asked: Option<&str>| {
        let asked = asked.filter(|name| !name.is_empty())?;
        let found = runs.iter().position(|run| run.name == asked);
        if found.is_none() {
            unknown.push(asked.to_string());
        }
        found
    };
    let (from, to) = (find(from), find(to));
    let picked =
        (runs.len() >= 2).then(|| (from.unwrap_or(runs.len() - 2), to.unwrap_or(runs.len() - 1)));
    let compared = picked.map(|(from, to)| Compared {
        groups: groups(&runs[from], &runs[to], docs, texts),
    });
    HistoryPage {
        runs: runs
            .iter()
            .enumerate()
            .rev()
            .map(|(at, run)| RunView {
                name: run.name.clone(),
                finished: label(run),
                commit: short_commit(&run.snapshot),
                from: picked.is_some_and(|(from, _)| from == at),
                to: picked.is_some_and(|(_, to)| to == at),
            })
            .collect(),
        unknown,
        compared,
    }
}

/// The first 7 characters of the commit `snapshot` read, `None` with no commit.
pub(crate) fn short_commit(snapshot: &Snapshot) -> Option<String> {
    snapshot
        .commit
        .as_ref()
        .map(|c| c.chars().take(7).collect())
}

/// When `run` ended, with the number of its file when it is not the first of its second.
pub(crate) fn label(run: &SavedRun) -> String {
    let when = when(&run.snapshot.finished_at);
    match run.name.split_once('_') {
        Some((_, n)) => format!("{when} ({n})"),
        None => when,
    }
}

/// `2026-11-04T10:00:00.123+00:00` → `2026-11-04 10:00:00 UTC`; another offset is kept as written.
fn when(finished_at: &str) -> String {
    let (time, rest) = finished_at.split_at(finished_at.len().min(19));
    let zone = rest.trim_start_matches(|c: char| c == '.' || c.is_ascii_digit());
    let zone = match zone {
        "Z" | "+00:00" => "UTC",
        other => other,
    };
    format!("{} {zone}", time.replace('T', " "))
        .trim_end()
        .to_string()
}

fn groups(
    from: &SavedRun,
    to: &SavedRun,
    docs: Option<&GeneratedDocs>,
    texts: &WebTexts,
) -> Vec<ChangeGroup> {
    let diff = diff(&from.snapshot, &to.snapshot);
    let pages = docs.map(Pages::of);
    let page = &texts.history_page;
    let changed = |section: &SectionScores, detail: Option<String>| ChangedSection {
        kind: match section.kind {
            SectionKind::Domain => texts.domain,
            SectionKind::Feature => texts.feature,
            SectionKind::UseCase => texts.use_case,
        },
        name: section.name.clone(),
        href: pages.as_ref().and_then(|pages| pages.href(section)),
        detail,
    };
    let plain = |sections: Vec<&SectionScores>| -> Vec<ChangedSection> {
        sections.into_iter().map(|s| changed(s, None)).collect()
    };
    let reasons = |section: &SectionScores| {
        section
            .set_apart
            .iter()
            .map(|key| set_apart_reason(key).map_or(key.as_str(), |r| texts.set_apart_reason(r)))
            .collect::<Vec<_>>()
            .join(", ")
    };
    [
        ("history-added", page.added, plain(diff.added)),
        ("history-removed", page.removed, plain(diff.removed)),
        (
            "history-degraded",
            page.degraded,
            diff.degraded
                .iter()
                .map(|(section, falls)| {
                    let detail = falls
                        .iter()
                        .map(|fall| fell(fall, texts))
                        .collect::<Vec<_>>();
                    changed(section, Some(detail.join("; ")))
                })
                .collect(),
        ),
        (
            "history-set-apart",
            page.set_apart,
            diff.set_apart
                .iter()
                .map(|section| changed(section, Some(reasons(section))))
                .collect(),
        ),
        (
            "history-no-longer-set-apart",
            page.no_longer_set_apart,
            plain(diff.no_longer_set_apart),
        ),
    ]
    .into_iter()
    .filter(|(_, _, sections)| !sections.is_empty())
    .map(|(id, heading, sections)| ChangeGroup {
        id,
        heading,
        sections,
    })
    .collect()
}

/// `Confidence 90% → 50% (−40)`, the fall rounded on its own.
fn fell(fall: &Fall, texts: &WebTexts) -> String {
    let score = match fall.score {
        Score::Confidence => texts.history_page.confidence,
        Score::BusinessLanguage => texts.history_page.business_language,
    };
    let percent = |value: f32| (value * 100.0).round();
    format!(
        "{score} {}% → {}% (−{})",
        percent(fall.from),
        percent(fall.to),
        percent(fall.by())
    )
}

/// The pages of the docs served, looked up by the id of a section.
struct Pages<'a> {
    domains: HashSet<&'a str>,
    features: HashMap<&'a str, &'a Feature>,
    use_cases: HashMap<(&'a str, &'a str), &'a UseCase>,
}

impl<'a> Pages<'a> {
    fn of(docs: &'a GeneratedDocs) -> Self {
        Self {
            domains: docs.domains().iter().map(|d| d.slug.as_str()).collect(),
            features: docs
                .features()
                .iter()
                .map(|f| (f.slug.as_str(), f))
                .collect(),
            use_cases: docs
                .use_cases()
                .iter()
                .map(|u| ((u.feature_slug.as_str(), u.slug.as_str()), u))
                .collect(),
        }
    }

    /// The page of `section`, `None` when the docs don't hold it.
    fn href(&self, section: &SectionScores) -> Option<String> {
        match section.kind {
            SectionKind::Domain => self
                .domains
                .contains(section.id.as_str())
                .then(|| domain_href(&section.id)),
            SectionKind::Feature => self
                .features
                .get(section.id.as_str())
                .map(|f| feature_href(f)),
            SectionKind::UseCase => {
                let (feature, slug) = section.id.split_once('/')?;
                let use_case = self.use_cases.get(&(feature, slug))?;
                Some(use_case_href(self.features.get(feature)?, use_case))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: HPG-8 (internal: a fall whose rounded percents differ by less than the rounded fall)
    #[test]
    fn the_fall_shown_is_the_rounded_fall() {
        let fall = Fall {
            score: Score::Confidence,
            from: 0.894_99,
            to: 0.695,
        };

        assert_eq!(
            fell(&fall, &WebTexts::ENGLISH),
            "Confidence 89% → 70% (−20)"
        );
    }

    // spec: HPG-1 (internal: a time with an offset, which `generate` never writes)
    #[test]
    fn a_time_keeps_its_zone() {
        assert_eq!(
            when("2026-11-04T10:00:00.123+00:00"),
            "2026-11-04 10:00:00 UTC"
        );
        assert_eq!(when("2026-11-04T10:00:00Z"), "2026-11-04 10:00:00 UTC");
        assert_eq!(
            when("2026-11-04T12:00:00+02:00"),
            "2026-11-04 12:00:00 +02:00"
        );
    }
}
