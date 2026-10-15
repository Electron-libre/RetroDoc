//! Measures, with no LLM call, how many use cases of a saved run only show something on a
//! resource that another use case changes (`issues/merge_form_screens_into_their_use_case.md`,
//! step 1): the candidates to be merged with the use case that submits.
//!
//! Usage: `cargo run -q -p retrodoc-pipeline --example form_pairs_measure -- [--names] <run dir>...`
//!
//! A run dir is a checkout of the repository with its `.retrodoc/cache` (`use-cases.yaml`,
//! `entry-points.yaml`). The pairing does not read action names nor paths, so it holds for any
//! framework: an entry point *acts* when one of its outputs is a database write, an email, an
//! event or an external call; a use case *shows* when it cites HTTP routes and none acts. Then:
//! * paired: the use case shows a resource that an acting HTTP route of another use case changes
//!   (the opening of a form, or a plain consultation: the measure cannot tell them apart);
//! * alone: it shows a resource nothing changes (a legitimate use case).
//!
//! `--names` also prints the names of the paired use cases, to read a sample by hand (never to
//! be copied in a committed file).

#![allow(clippy::cast_precision_loss)] // counts of use cases: far below 2^52

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use retrodoc_core::model::UseCase;
use retrodoc_pipeline::{load_use_cases, EntryKind, EntryPoint, EntryPoints, OutputKind};

/// Words that open a screen rather than do something.
const SCREEN_VERBS: [&str; 6] = ["open", "show", "view", "edit", "display", "new"];

/// What a use case is, as far as its entry points tell.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Kind {
    /// No entry point of it is known.
    Unknown,
    /// Cites something other than HTTP routes only, or an entry point that acts.
    Acts,
    /// Shows a resource that another use case changes.
    Paired,
    /// Shows a resource nothing changes.
    Alone,
}

/// A use case with its kind, for the report.
#[derive(Debug)]
struct Verdict {
    name: String,
    kind: Kind,
    /// The name starts with a verb that opens a screen.
    screen_name: bool,
    /// An entry point it cites talks of a form.
    form_text: bool,
}

fn acts(entry: &EntryPoint) -> bool {
    entry.outputs.iter().any(|o| {
        matches!(
            o.kind,
            OutputKind::DbWrite | OutputKind::Email | OutputKind::Event | OutputKind::ExternalCall
        )
    })
}

/// The entry point `<file>#<name>` of the inventory.
fn find<'a>(entry_points: &'a EntryPoints, id: &str) -> Option<&'a EntryPoint> {
    let (file, name) = id.split_once('#')?;
    entry_points
        .files
        .get(Path::new(file))?
        .entry_points
        .iter()
        .find(|e| e.name == name)
}

fn judge(use_cases: &[UseCase], entry_points: &EntryPoints) -> Vec<Verdict> {
    let cited: Vec<Vec<&EntryPoint>> = use_cases
        .iter()
        .map(|uc| {
            uc.entry_points
                .iter()
                .filter_map(|id| find(entry_points, id))
                .collect()
        })
        .collect();
    // The resources an acting route of each use case changes, to ask "by another use case".
    let changed_by: Vec<BTreeSet<&str>> = cited
        .iter()
        .map(|entries| {
            entries
                .iter()
                .filter(|e| e.kind == EntryKind::HttpRoute && acts(e) && !e.resource.is_empty())
                .map(|e| e.resource.as_str())
                .collect()
        })
        .collect();
    use_cases
        .iter()
        .enumerate()
        .map(|(i, uc)| {
            let entries = &cited[i];
            let shows = !entries.is_empty()
                && entries
                    .iter()
                    .all(|e| e.kind == EntryKind::HttpRoute && !acts(e));
            let kind = if entries.is_empty() {
                Kind::Unknown
            } else if !shows {
                Kind::Acts
            } else if entries.iter().any(|e| {
                changed_by
                    .iter()
                    .enumerate()
                    .any(|(j, set)| j != i && set.contains(e.resource.as_str()))
            }) {
                Kind::Paired
            } else {
                Kind::Alone
            };
            Verdict {
                name: uc.name.clone(),
                kind,
                screen_name: uc
                    .name
                    .split_whitespace()
                    .next()
                    .is_some_and(|w| SCREEN_VERBS.contains(&w.to_lowercase().as_str())),
                form_text: entries.iter().any(|e| {
                    format!("{} {}", e.name, e.description)
                        .to_lowercase()
                        .contains("form")
                }),
            }
        })
        .collect()
}

fn percent(part: usize, whole: usize) -> String {
    if whole == 0 {
        "0%".into()
    } else {
        format!("{:.0}%", part as f64 * 100.0 / whole as f64)
    }
}

fn report(verdicts: &[Verdict], names: bool) -> String {
    let total = verdicts.len();
    let count = |kind: &Kind| verdicts.iter().filter(|v| &v.kind == kind).count();
    let mut out = String::new();
    for (label, kind) in [
        (
            "paired (shows a resource another use case changes)",
            Kind::Paired,
        ),
        ("alone (shows a resource nothing changes)", Kind::Alone),
        (
            "act (cite an entry point that acts, or not only routes)",
            Kind::Acts,
        ),
        ("without a known entry point", Kind::Unknown),
    ] {
        let n = count(&kind);
        let _ = writeln!(out, "{label}: {n} ({})", percent(n, total));
    }
    let paired: Vec<&Verdict> = verdicts.iter().filter(|v| v.kind == Kind::Paired).collect();
    let screen = paired.iter().filter(|v| v.screen_name).count();
    let form = paired.iter().filter(|v| v.form_text).count();
    let _ = writeln!(
        out,
        "\nOf the {} paired: {screen} have a name that opens a screen, {form} cite an entry point that speaks of a form.",
        paired.len()
    );
    let all_screen = verdicts.iter().filter(|v| v.screen_name).count();
    let _ = writeln!(
        out,
        "Use cases with a name that opens a screen, all kinds: {all_screen} ({}).",
        percent(all_screen, total)
    );
    if names {
        let _ = writeln!(out, "\nPaired:");
        for v in paired {
            let _ = writeln!(out, "- {}", v.name);
        }
    }
    out
}

fn run(repo_root: &Path, names: bool) -> Option<String> {
    let use_cases = load_use_cases(repo_root)?;
    let entry_points = EntryPoints::load(repo_root).unwrap_or_default();
    let verdicts = judge(&use_cases, &entry_points);
    Some(format!(
        "## {}\n\n{} use cases.\n\n{}",
        repo_root.display(),
        verdicts.len(),
        report(&verdicts, names)
    ))
}

fn main() {
    let mut names = false;
    let mut dirs: Vec<String> = Vec::new();
    for arg in std::env::args().skip(1) {
        if arg == "--names" {
            names = true;
        } else {
            dirs.push(arg);
        }
    }
    if dirs.is_empty() {
        eprintln!("usage: form_pairs_measure [--names] <run dir>...");
        std::process::exit(2);
    }
    for dir in dirs {
        match run(Path::new(&dir), names) {
            Some(text) => println!("{text}"),
            None => eprintln!("{dir}: no use-cases.yaml"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use retrodoc_pipeline::{EntryFile, Output};

    fn entry(name: &str, resource: &str, outputs: &[OutputKind]) -> EntryPoint {
        EntryPoint {
            kind: EntryKind::HttpRoute,
            name: name.into(),
            verb: String::new(),
            resource: resource.into(),
            description: String::new(),
            outputs: outputs
                .iter()
                .map(|kind| Output {
                    kind: *kind,
                    description: String::new(),
                })
                .collect(),
        }
    }

    fn inventory(entries: Vec<EntryPoint>) -> EntryPoints {
        let mut inv = EntryPoints::default();
        inv.files.insert(
            "routes.rb".into(),
            EntryFile {
                content_hash: String::new(),
                entry_points: entries,
            },
        );
        inv
    }

    fn use_case(name: &str, cites: &[&str]) -> UseCase {
        UseCase {
            slug: name.into(),
            feature_slug: "f".into(),
            name: name.into(),
            description: String::new(),
            steps: Vec::new(),
            entry_points: cites.iter().map(|n| format!("routes.rb#{n}")).collect(),
            primary_actor: None,
            narrative: None,
            business_language: None,
            diagram_mermaid: None,
            confidence: None,
        }
    }

    #[test]
    fn tells_paired_alone_acting_and_unknown() {
        let inv = inventory(vec![
            entry("GET a/new", "a", &[OutputKind::Response]),
            entry("POST a", "a", &[OutputKind::DbWrite, OutputKind::Response]),
            entry("GET b", "b", &[OutputKind::Response]),
        ]);
        let ucs = vec![
            use_case("Open a form", &["GET a/new"]),
            use_case("Create a", &["POST a"]),
            use_case("Show b", &["GET b"]),
            use_case("Something", &["not in the inventory"]),
        ];
        let kinds: Vec<Kind> = judge(&ucs, &inv).into_iter().map(|v| v.kind).collect();
        assert_eq!(
            kinds,
            [Kind::Paired, Kind::Acts, Kind::Alone, Kind::Unknown]
        );
    }

    #[test]
    fn a_use_case_does_not_pair_with_itself() {
        let inv = inventory(vec![
            entry("GET a/new", "a", &[OutputKind::Response]),
            entry("POST a", "a", &[OutputKind::DbWrite]),
        ]);
        // One use case that cites both already is the merged one: it acts, it is not a screen.
        let ucs = vec![use_case("Create a", &["GET a/new", "POST a"])];
        assert_eq!(judge(&ucs, &inv)[0].kind, Kind::Acts);
    }

    #[test]
    fn the_name_and_the_text_are_hints_not_the_rule() {
        let mut open = entry("GET a/new", "a", &[]);
        open.description = "Shows the creation form.".into();
        let inv = inventory(vec![open, entry("POST a", "a", &[OutputKind::Email])]);
        let ucs = vec![
            use_case("Open the creation form", &["GET a/new"]),
            use_case("Create a", &["POST a"]),
        ];
        let v = judge(&ucs, &inv);
        assert_eq!(v[0].kind, Kind::Paired);
        assert!(v[0].screen_name && v[0].form_text);
        assert!(!v[1].screen_name);
    }
}
