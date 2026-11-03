//! The evidence of a reference (spec `benchmark-reference-evidence`): checks that the files and headings
//! its features cite exist in a checkout of the repository, counts what backs each feature, and re-reads
//! saved run reports with the features marked `disputed` taken out. Example `reference_evidence`.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

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
            if !tree.join(path).exists() {
                check
                    .problems
                    .push(format!("`{}`: `{path}` does not exist", feature.name));
                continue;
            }
            if let Some(heading) = heading {
                let text = std::fs::read_to_string(tree.join(path)).unwrap_or_default();
                if !headings(&text).any(|h| h.eq_ignore_ascii_case(heading.trim())) {
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

/// The Markdown headings and the HTML `h1` to `h6` of a file, trimmed, inner tags left out.
fn headings(text: &str) -> impl Iterator<Item = String> + '_ {
    let markdown = text.lines().filter_map(|line| {
        let line = line.trim_start();
        let rest = line.trim_start_matches('#');
        (line.len() != rest.len() && rest.starts_with(' '))
            .then(|| rest.trim().trim_end_matches('#').trim().to_string())
    });
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
        html.push(strip_tags(inner).trim().to_string());
        rest = &after[close + 3..];
    }
    markdown.chain(html)
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
        recall: ratio(matched_reference.len(), kept),
        precision: ratio(matched_generated.len(), generated.len()),
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
        let mut line = format!(
            "{name}: features strict {}",
            figures(reference, &report.strict.features)
        );
        if let Some(judged) = &report.judged {
            let _ = write!(line, ", judged {}", figures(reference, &judged.features));
        }
        let _ = writeln!(out, "{line}");
    }
    out
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
            recall: None,
            precision: None,
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
}
