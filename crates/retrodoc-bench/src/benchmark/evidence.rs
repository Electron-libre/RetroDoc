//! The evidence of a reference (spec `benchmark-reference-evidence`): checks that the files and headings
//! its features cite exist in a checkout of the repository, counts what backs each feature, and re-reads
//! saved run reports with the features marked `disputed` taken out. Example `reference_evidence`.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Component, Path};
use std::process::Command;

use super::matching::Score;
use super::reference::{EvidenceKind, Reference, ReferenceFeature};
use super::table::RunReport;

/// What [`check_evidence`] found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EvidenceCheck {
    /// One line per `doc` or `code` item whose path or heading is missing in the tree.
    pub problems: Vec<String>,
    /// Features backed by a `doc` or `url` item.
    pub doc_backed: usize,
    /// Features backed by `code` items only.
    pub code_only: usize,
    /// Features with no evidence.
    pub none: usize,
    pub disputed: usize,
}

impl EvidenceCheck {
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.problems.is_empty()
    }
}

/// Checks the evidence of every feature of `reference` against `tree`, a checkout at its commit.
#[must_use]
pub fn check_evidence(reference: &Reference, tree: &Path) -> EvidenceCheck {
    let mut check = EvidenceCheck::default();
    if let Some(head) = git_head(tree) {
        if !head.starts_with(reference.commit.trim()) {
            check.problems.push(format!(
                "the tree is at `{head}`, the reference at `{}`",
                reference.commit.trim()
            ));
        }
    }
    for feature in features(reference) {
        let kinds: Vec<_> = feature.evidence.iter().filter_map(|e| e.kind()).collect();
        if kinds.is_empty() {
            check.none += 1;
        } else if kinds.iter().any(|(kind, _)| *kind != EvidenceKind::Code) {
            check.doc_backed += 1;
        } else {
            check.code_only += 1;
        }
        if feature.disputed.is_some() {
            check.disputed += 1;
        }
        for (kind, value) in kinds {
            if kind == EvidenceKind::Url {
                continue;
            }
            let (path, heading) = match (kind, value.split_once('#')) {
                (EvidenceKind::Doc, Some((path, heading))) => (path, Some(heading)),
                _ => (value, None),
            };
            if Path::new(path)
                .components()
                .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
            {
                check.problems.push(format!(
                    "`{}`: `{path}` is outside the repository",
                    feature.name
                ));
                continue;
            }
            if !tree.join(path).exists() {
                check
                    .problems
                    .push(format!("`{}`: `{path}` does not exist", feature.name));
                continue;
            }
            if let Some(heading) = heading {
                let text = std::fs::read_to_string(tree.join(path)).unwrap_or_default();
                let wanted = one_line(heading);
                if !headings(&text).any(|h| h.eq_ignore_ascii_case(&wanted)) {
                    check.problems.push(format!(
                        "`{}`: `{path}` has no heading `{heading}`",
                        feature.name
                    ));
                }
            }
        }
    }
    check
}

fn features(reference: &Reference) -> impl Iterator<Item = &ReferenceFeature> {
    reference.domains.iter().flat_map(|d| &d.features)
}

/// The `HEAD` commit of `tree` when it is a Git checkout.
pub(super) fn git_head(tree: &Path) -> Option<String> {
    if !tree.join(".git").exists() {
        return None;
    }
    let output = Command::new("git")
        .arg("-C")
        .arg(tree)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// `text` with its runs of spaces and line breaks taken as one space, trimmed.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The Markdown headings (outside the front matter and the fenced code blocks) and the HTML `h1` to `h6`
/// of a file, on one line, inner tags left out.
fn headings(text: &str) -> impl Iterator<Item = String> + '_ {
    let mut markdown = Vec::new();
    let mut lines = text.lines().peekable();
    if lines.peek().is_some_and(|l| l.trim_end() == "---") {
        lines.next();
        for line in lines.by_ref() {
            if line.trim_end() == "---" {
                break;
            }
        }
    }
    let mut fence: Option<&str> = None;
    for line in lines {
        let line = line.trim_start();
        if let Some(open) = fence {
            if line.starts_with(open) {
                fence = None;
            }
            continue;
        }
        if line.starts_with("```") || line.starts_with("~~~") {
            fence = Some(&line[..3]);
            continue;
        }
        let rest = line.trim_start_matches('#');
        let level = line.len() - rest.len();
        if (1..=6).contains(&level) && rest.starts_with([' ', '\t']) {
            let rest = rest.trim();
            let closed = rest.trim_end_matches('#');
            let rest = if closed.is_empty() || closed.ends_with([' ', '\t']) {
                closed
            } else {
                rest
            };
            markdown.push(one_line(rest));
        }
    }
    let mut html = Vec::new();
    let mut rest = text;
    while let Some(start) = find_open_heading(rest) {
        let after = &rest[start..];
        let Some(close) = after.find("</h") else {
            break;
        };
        let inner = after[..close]
            .split_once('>')
            .map_or("", |(_, inner)| inner);
        html.push(one_line(&strip_tags(inner)));
        rest = &after[close + 3..];
    }
    markdown.into_iter().chain(html)
}

fn find_open_heading(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    (0..bytes.len().saturating_sub(2)).find(|&i| {
        bytes[i] == b'<'
            && bytes[i + 1].eq_ignore_ascii_case(&b'h')
            && (b'1'..=b'6').contains(&bytes[i + 2])
    })
}

fn strip_tags(text: &str) -> String {
    let mut out = String::new();
    let mut depth = 0usize;
    for c in text.chars() {
        match c {
            '<' => depth += 1,
            '>' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

/// `score` (the features of a run) with the features of `reference` marked `disputed` taken out: a
/// generated name stays matched when one of its reference features is not disputed.
#[must_use]
pub fn without_disputed(reference: &Reference, score: &Score) -> Score {
    let disputed: BTreeSet<&str> = features(reference)
        .filter(|f| f.disputed.is_some())
        .map(|f| f.name.as_str())
        .collect();
    let kept = features(reference).filter(|f| f.disputed.is_none()).count();
    let matched: Vec<(String, String)> = score
        .matched
        .iter()
        .filter(|(_, r)| !disputed.contains(r.as_str()))
        .cloned()
        .collect();
    let mut generated: Vec<&str> = Vec::new();
    for name in score
        .matched
        .iter()
        .map(|(g, _)| g.as_str())
        .chain(score.unmatched_generated.iter().map(String::as_str))
    {
        if !generated.contains(&name) {
            generated.push(name);
        }
    }
    let matched_generated: BTreeSet<&str> = matched.iter().map(|(g, _)| g.as_str()).collect();
    let matched_reference: BTreeSet<&str> = matched.iter().map(|(_, r)| r.as_str()).collect();
    #[allow(clippy::cast_precision_loss)] // counts of features: far below 2^23
    let ratio = |n: usize, d: usize| (d > 0).then(|| n as f32 / d as f32);
    Score {
        recall: score
            .recall
            .and_then(|_| ratio(matched_reference.len(), kept)),
        precision: score
            .precision
            .and_then(|_| ratio(matched_generated.len(), generated.len())),
        unmatched_generated: generated
            .iter()
            .filter(|g| !matched_generated.contains(*g))
            .map(|g| (*g).to_string())
            .collect(),
        unmatched_reference: score
            .unmatched_reference
            .iter()
            .filter(|r| !disputed.contains(r.as_str()))
            .cloned()
            .collect(),
        matched,
    }
}

/// The text the example prints: the check, its counts and problems, then one line per saved report.
#[must_use]
pub fn evidence_summary(
    reference: &Reference,
    check: &EvidenceCheck,
    reports: &[(String, RunReport)],
) -> String {
    let mut out = String::new();
    match &reference.checked {
        Some(checked) => {
            let _ = writeln!(out, "Checked: {} ({})", checked.date, checked.method);
        }
        None => out.push_str("Checked: never\n"),
    }
    let _ = writeln!(
        out,
        "Features: {}; backed by docs {}, by code only {}, no evidence {}; disputed {}",
        reference.feature_count(),
        check.doc_backed,
        check.code_only,
        check.none,
        check.disputed
    );
    for problem in &check.problems {
        let _ = writeln!(out, "- {problem}");
    }
    for (name, report) in reports {
        let judged = report.judged.as_ref().map(|j| &j.features);
        let _ = writeln!(
            out,
            "{}",
            report_line(reference, name, &report.strict.features, judged)
        );
    }
    out
}

/// The figures of one saved report, or why they are skipped: a report that pairs a feature the reference
/// does not hold was scored against another reference.
fn report_line(
    reference: &Reference,
    name: &str,
    strict: &Score,
    judged: Option<&Score>,
) -> String {
    let known: BTreeSet<&str> = features(reference).map(|f| f.name.as_str()).collect();
    for score in std::iter::once(strict).chain(judged) {
        let named: BTreeSet<&str> = score
            .matched
            .iter()
            .map(|(_, r)| r.as_str())
            .chain(score.unmatched_reference.iter().map(String::as_str))
            .collect();
        let odd = named
            .difference(&known)
            .map(|r| format!("`{r}` is not in it"))
            .chain(
                known
                    .difference(&named)
                    .map(|r| format!("`{r}` is missing from the report")),
            )
            .next();
        if let Some(odd) = odd {
            return format!("{name}: scored against another reference ({odd}), skipped");
        }
    }
    let mut line = format!("{name}: features strict {}", figures(reference, strict));
    if let Some(judged) = judged {
        let _ = write!(line, ", judged {}", figures(reference, judged));
    }
    line
}

/// `recall/precision -> recall/precision without the disputed features`, in percent.
fn figures(reference: &Reference, score: &Score) -> String {
    let after = without_disputed(reference, score);
    format!(
        "{}/{} -> {}/{}",
        percent(score.recall),
        percent(score.precision),
        percent(after.recall),
        percent(after.precision)
    )
}

fn percent(value: Option<f32>) -> String {
    value.map_or_else(|| "-".to_string(), |v| format!("{:.0}%", v * 100.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::benchmark::reference::{Evidence, ReferenceDomain};

    fn feature(name: &str, evidence: &[(&str, &str)], disputed: bool) -> ReferenceFeature {
        ReferenceFeature {
            name: name.into(),
            description: String::new(),
            use_cases: Vec::new(),
            evidence: evidence
                .iter()
                .map(|(kind, value)| {
                    let value = Some((*value).to_string());
                    match *kind {
                        "doc" => Evidence {
                            doc: value,
                            url: None,
                            code: None,
                        },
                        "url" => Evidence {
                            doc: None,
                            url: value,
                            code: None,
                        },
                        _ => Evidence {
                            doc: None,
                            url: None,
                            code: value,
                        },
                    }
                })
                .collect(),
            disputed: disputed.then(|| "only one draft held it".into()),
        }
    }

    fn reference(features: Vec<ReferenceFeature>) -> Reference {
        Reference {
            repository: "https://example.com/shop".into(),
            commit: "abc".into(),
            purpose: String::new(),
            actors: Vec::new(),
            domains: vec![ReferenceDomain {
                name: "Shop".into(),
                description: String::new(),
                features,
            }],
            rules: Vec::new(),
            checked: None,
        }
    }

    fn tree(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (path, text) in files {
            let path = dir.path().join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        dir
    }

    // spec: BRE-4, BRE-7 (internal: a measuring tool run as a cargo example, not through the binary)
    #[test]
    fn reports_the_paths_missing_in_the_tree() {
        let dir = tree(&[("app/pay.py", "def pay(): ..."), ("app/admin/x.py", "")]);
        let reference = reference(vec![feature(
            "Checkout",
            &[
                ("code", "app/admin"),
                ("code", "app/pay.py"),
                ("code", "app/gone.py"),
                ("doc", "docs/pay.md"),
            ],
            false,
        )]);
        let check = check_evidence(&reference, dir.path());
        assert_eq!(
            check.problems,
            [
                "`Checkout`: `app/gone.py` does not exist",
                "`Checkout`: `docs/pay.md` does not exist"
            ]
        );
        assert!(!check.is_clean());
    }

    // spec: BRE-5, BRE-7 (internal: a measuring tool run as a cargo example, not through the binary)
    #[test]
    fn checks_the_headings_of_markdown_and_html_docs() {
        let dir = tree(&[
            (
                "docs/search.md",
                "---\ntitle: Search\n---\n\n## Search Syntax ##\n",
            ),
            (
                "app/views/about.html.erb",
                "<h2 id=\"merging\">Story <em>merging</em></h2>\n<h3>Flags</h3>",
            ),
        ]);
        let reference = reference(vec![
            feature(
                "Search",
                &[("doc", "docs/search.md# search syntax ")],
                false,
            ),
            feature(
                "Merge",
                &[("doc", "app/views/about.html.erb#Story merging")],
                false,
            ),
            feature("Flag", &[("doc", "app/views/about.html.erb#Flags")], false),
            feature("Vote", &[("doc", "app/views/about.html.erb#Voting")], false),
            feature("Url", &[("url", "https://example.com/gone#Nothing")], false),
        ]);
        let check = check_evidence(&reference, dir.path());
        assert_eq!(
            check.problems,
            ["`Vote`: `app/views/about.html.erb` has no heading `Voting`"]
        );

        let clean = reference
            .domains
            .iter()
            .flat_map(|d| &d.features)
            .filter(|f| f.name != "Vote")
            .cloned()
            .collect();
        assert!(check_evidence(&self::reference(clean), dir.path()).is_clean());
    }

    // spec: BRE-6 (internal: a measuring tool run as a cargo example, not through the binary)
    #[test]
    fn counts_the_features_by_what_backs_them() {
        let dir = tree(&[("a.md", "# A"), ("b.py", "")]);
        let reference = reference(vec![
            feature("Doc", &[("doc", "a.md"), ("code", "b.py")], false),
            feature("Url", &[("url", "https://example.com")], true),
            feature("Code", &[("code", "b.py")], false),
            feature("None", &[], true),
        ]);
        let check = check_evidence(&reference, dir.path());
        assert_eq!(
            (
                check.doc_backed,
                check.code_only,
                check.none,
                check.disputed
            ),
            (2, 1, 1, 2)
        );
        assert!(check.is_clean());
    }

    fn score(matched: &[(&str, &str)], unmatched_generated: &[&str]) -> Score {
        Score {
            recall: Some(0.0),
            precision: Some(0.0),
            matched: matched
                .iter()
                .map(|(g, r)| ((*g).to_string(), (*r).to_string()))
                .collect(),
            unmatched_generated: unmatched_generated
                .iter()
                .map(|g| (*g).to_string())
                .collect(),
            unmatched_reference: Vec::new(),
        }
    }

    // spec: BRE-8 (internal: a measuring tool run as a cargo example, not through the binary)
    #[test]
    fn takes_the_disputed_features_out_of_a_score() {
        let mut features: Vec<_> = (0..8)
            .map(|i| feature(&format!("F{i}"), &[], false))
            .collect();
        features.push(feature("Save a bookmark", &[], false));
        features.push(feature("Read it later", &[], true));
        let reference = reference(features);
        let after = without_disputed(
            &reference,
            &score(
                &[("Bookmarks", "Save a bookmark"), ("Queue", "Read it later")],
                &[],
            ),
        );
        assert_eq!(after.recall, Some(1.0 / 9.0));
        assert_eq!(after.precision, Some(0.5));
        assert_eq!(after.unmatched_generated, ["Queue"]);

        let both = without_disputed(
            &reference,
            &score(
                &[
                    ("Bookmarks", "Save a bookmark"),
                    ("Bookmarks", "Read it later"),
                ],
                &["Other"],
            ),
        );
        assert_eq!(both.precision, Some(0.5));
        assert_eq!(
            both.matched,
            [("Bookmarks".into(), "Save a bookmark".into())]
        );
    }

    // spec: BRE-9 (internal: a measuring tool run as a cargo example, not through the binary)
    #[test]
    fn the_summary_says_when_the_reference_was_checked() {
        let mut reference = reference(vec![feature("Checkout", &[], false)]);
        let check = EvidenceCheck::default();
        assert!(evidence_summary(&reference, &check, &[]).starts_with("Checked: never\n"));
        reference.checked = Some(super::super::reference::Checked {
            date: "2026-11-04".into(),
            method: "blind second draft".into(),
        });
        assert!(evidence_summary(&reference, &check, &[])
            .starts_with("Checked: 2026-11-04 (blind second draft)\n"));
    }

    // spec: BRE-10 (internal: a measuring tool run as a cargo example, not through the binary)
    #[test]
    fn reports_the_paths_outside_the_repository() {
        let dir = tree(&[("app/pay.py", "")]);
        let outside = dir.path().join("app/pay.py").display().to_string();
        let reference = reference(vec![feature(
            "Checkout",
            &[
                ("code", &outside),
                ("doc", "../x/README.md#Intro"),
                ("code", "app/../app/pay.py"),
            ],
            false,
        )]);
        let check = check_evidence(&reference, dir.path());
        assert_eq!(
            check.problems,
            [
                format!("`Checkout`: `{outside}` is outside the repository"),
                "`Checkout`: `../x/README.md` is outside the repository".to_string(),
                "`Checkout`: `app/../app/pay.py` is outside the repository".to_string(),
            ]
        );
    }

    // spec: BRE-11 (internal: a measuring tool run as a cargo example, not through the binary)
    #[test]
    fn refuses_a_tree_at_another_commit() {
        let dir = tree(&[("a.py", "")]);
        let repo = git2::Repository::init(dir.path()).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("a.py")).unwrap();
        let tree_id = index.write_tree().unwrap();
        let signature = git2::Signature::now("t", "t@example.com").unwrap();
        let head = repo
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                "first",
                &repo.find_tree(tree_id).unwrap(),
                &[],
            )
            .unwrap()
            .to_string();
        let mut reference = reference(vec![feature("Checkout", &[("code", "a.py")], false)]);
        reference.commit = "0123456".into();
        let check = check_evidence(&reference, dir.path());
        assert_eq!(
            check.problems,
            [format!(
                "the tree is at `{head}`, the reference at `0123456`"
            )]
        );
        reference.commit = head[..7].to_string();
        assert!(check_evidence(&reference, dir.path()).is_clean());
        assert!(check_evidence(&reference, tree(&[("a.py", "")]).path()).is_clean());
    }

    // spec: BRE-12 (internal: a measuring tool run as a cargo example, not through the binary)
    #[test]
    fn skips_a_report_scored_against_another_reference() {
        let reference = reference(vec![feature("Checkout", &[], false)]);
        let stale = score(&[("Pay", "Save a PDF")], &[]);
        let mut current = score(&[("Pay", "Checkout")], &[]);
        let text = report_line(&reference, "old.json", &stale, Some(&current));
        assert_eq!(
            text,
            "old.json: scored against another reference (`Save a PDF` is not in it), skipped"
        );
        let mut unmatched = score(&[], &[]);
        unmatched.unmatched_reference = vec!["Save a PDF".into()];
        assert_eq!(
            report_line(&reference, "old.json", &unmatched, None),
            "old.json: scored against another reference (`Save a PDF` is not in it), skipped"
        );
        assert_eq!(
            report_line(&reference, "old.json", &score(&[], &[]), None),
            "old.json: scored against another reference (`Checkout` is missing from the report), skipped"
        );
        current.unmatched_generated = vec!["Other".into()];
        let text = report_line(&reference, "new.json", &current, None);
        assert!(text.contains("new.json: features strict"), "{text}");
    }

    // spec: BRE-13 (internal: a measuring tool run as a cargo example, not through the binary)
    #[test]
    fn an_unjudged_score_stays_unjudged() {
        let reference = reference(vec![feature("Checkout", &[], false)]);
        let unjudged = Score {
            recall: None,
            precision: None,
            ..score(&[("Pay", "Checkout")], &["Other"])
        };
        let after = without_disputed(&reference, &unjudged);
        assert_eq!((after.recall, after.precision), (None, None));
    }

    // spec: BRE-15 (internal: a measuring tool run as a cargo example, not through the binary)
    #[test]
    fn code_blocks_and_front_matter_hold_no_heading() {
        let dir = tree(&[
            (
                "docs/x.md",
                "---\n# Front\n---\n```sh\n# install the extension\n```\n## Using C#\n## Story\n",
            ),
            ("about.html.erb", "<h2 id=\"m\">Story\n    merging</h2>"),
        ]);
        let reference = reference(vec![
            feature("Code", &[("doc", "docs/x.md#install the extension")], false),
            feature("Front", &[("doc", "docs/x.md#Front")], false),
            feature("Sharp", &[("doc", "docs/x.md#Using C#")], false),
            feature("Wrapped", &[("doc", "about.html.erb#Story merging")], false),
        ]);
        let check = check_evidence(&reference, dir.path());
        assert_eq!(
            check.problems,
            [
                "`Code`: `docs/x.md` has no heading `install the extension`",
                "`Front`: `docs/x.md` has no heading `Front`",
            ]
        );
    }
}
