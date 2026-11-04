//! What the history page shows (`specs/history-page.md`): the runs the history saved, newest
//! first, and the difference of two of them from `retrodoc_mcp::history::diff`, each section linked
//! to its page when the docs served hold it.

use retrodoc_mcp::history::{diff, Fall, SavedRun, Score, SectionKind, SectionScores};
use retrodoc_mcp::GeneratedDocs;

use crate::texts::WebTexts;

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
    let mut pick = |asked: Option<&str>, default: usize| {
        let asked = asked.filter(|name| !name.is_empty());
        let found = asked.and_then(|name| runs.iter().position(|run| run.name == name));
        if found.is_none() {
            unknown.extend(asked.map(str::to_string));
        }
        found.unwrap_or(default)
    };
    let picked = (runs.len() >= 2).then(|| {
        let from = pick(from, runs.len() - 2);
        (from, pick(to, runs.len() - 1))
    });
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
                finished: run
                    .snapshot
                    .finished_at
                    .chars()
                    .take(19)
                    .collect::<String>()
                    .replace('T', " "),
                commit: run
                    .snapshot
                    .commit
                    .as_ref()
                    .map(|c| c.chars().take(7).collect()),
                from: picked.is_some_and(|(from, _)| from == at),
                to: picked.is_some_and(|(_, to)| to == at),
            })
            .collect(),
        unknown,
        compared,
    }
}

fn groups(
    from: &SavedRun,
    to: &SavedRun,
    docs: Option<&GeneratedDocs>,
    texts: &WebTexts,
) -> Vec<ChangeGroup> {
    let diff = diff(&from.snapshot, &to.snapshot);
    let page = &texts.history_page;
    let changed = |section: &SectionScores, detail: Option<String>| ChangedSection {
        kind: match section.kind {
            SectionKind::Domain => texts.domain,
            SectionKind::Feature => texts.feature,
            SectionKind::UseCase => texts.use_case,
        },
        name: section.name.clone(),
        href: docs.and_then(|docs| href(docs, section)),
        detail,
    };
    let plain = |sections: Vec<&SectionScores>| -> Vec<ChangedSection> {
        sections.into_iter().map(|s| changed(s, None)).collect()
    };
    let reasons = |section: &SectionScores| {
        section
            .set_apart
            .iter()
            .map(|key| reason(key, texts))
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

/// `Confidence 90% → 50% (−40)`.
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
        percent(fall.from) - percent(fall.to)
    )
}

/// The name of a reason a use case is set apart, as the snapshot keys it.
fn reason(key: &str, texts: &WebTexts) -> String {
    let at = match key {
        "back_office" => 0,
        "business_language" => 1,
        "confidence" => 2,
        other => return other.to_string(),
    };
    texts.debt.reasons[at].to_string()
}

/// The page of `section` in `docs`, `None` when they don't hold it.
fn href(docs: &GeneratedDocs, section: &SectionScores) -> Option<String> {
    let feature_href = |slug: &str| {
        docs.features()
            .iter()
            .find(|f| f.slug == slug)
            .map(|f| format!("/domains/{}/{}", f.domain_slug, f.slug))
    };
    match section.kind {
        SectionKind::Domain => docs
            .domains()
            .iter()
            .any(|d| d.slug == section.id)
            .then(|| format!("/domains/{}", section.id)),
        SectionKind::Feature => feature_href(&section.id),
        SectionKind::UseCase => {
            let (feature, slug) = section.id.split_once('/')?;
            docs.use_cases()
                .iter()
                .any(|u| u.feature_slug == feature && u.slug == slug)
                .then(|| feature_href(feature).map(|f| format!("{f}/{slug}")))
                .flatten()
        }
    }
}
