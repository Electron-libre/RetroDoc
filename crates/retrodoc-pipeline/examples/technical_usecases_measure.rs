//! Measures, with no LLM call, how many use cases of a saved run are not the business
//! (`issues/done/technical_and_back_office_use_cases.md`, step 1).
//!
//! Usage: `cargo run -q -p retrodoc-pipeline --example technical_usecases_measure -- <run dir>...`
//!
//! A run dir is a checkout of the repository with its `.retrodoc/cache` (`use-cases.yaml`,
//! `features.yaml`, `entry-points.yaml`). Prints, per run, how many use cases each signal flags
//! and, for the signals that could say "administration", how well they agree with the use cases
//! read from an `admin` folder (a reference for the measure only, never a rule of the pipeline).

#![allow(clippy::cast_precision_loss)] // counts of use cases: far below 2^52

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use retrodoc_core::model::{
    ActorKind, Feature, UseCase, MIN_BUSINESS_LANGUAGE, MIN_USE_CASE_CONFIDENCE,
};
use retrodoc_pipeline::{load_features, load_use_cases, EntryKind, EntryPoints};

/// Words that open a screen rather than do something.
const SCREEN_VERBS: [&str; 6] = ["open", "show", "view", "edit", "display", "new"];

/// One flag per signal: a table of independent yes/no answers, not a state.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Default, PartialEq, Eq)]
struct Signals {
    /// The feature is mostly `public_api` entry points.
    public_api_feature: bool,
    /// The feature has no HTTP route nor command, yet a human actor is named.
    human_without_human_entry: bool,
    /// A named human actor whose first step is done by the system.
    system_first_step: bool,
    /// The sources are in an `admin` folder (the reference).
    admin_path: bool,
    /// The primary actor is named an administrator.
    admin_actor: bool,
    /// The name or description of the feature says administration.
    admin_feature: bool,
    low_business_language: bool,
    low_confidence: bool,
    /// The name starts with a verb that opens a screen.
    screen: bool,
}

impl Signals {
    /// What the docs would set apart (step 2): administration or under a threshold.
    fn set_apart(&self) -> bool {
        self.admin_feature || self.low_business_language || self.low_confidence
    }
}

fn mentions_admin(text: &str) -> bool {
    let text = text.to_lowercase();
    text.contains("administrat")
        || text
            .split(|c: char| !c.is_alphanumeric())
            .any(|w| w == "admin")
}

fn in_admin_folder(path: &str) -> bool {
    path.split('/').any(|part| part == "admin")
}

/// The kinds of the entry points of a feature (`<file>#<name>`), unknown ones skipped.
fn kinds_of(feature: &Feature, entry_points: &EntryPoints) -> Vec<EntryKind> {
    feature
        .entry_points
        .iter()
        .filter_map(|id| {
            let (file, name) = id.split_once('#')?;
            let entry = entry_points
                .files
                .get(Path::new(file))?
                .entry_points
                .iter()
                .find(|e| e.name == name)?;
            Some(entry.kind)
        })
        .collect()
}

fn signals(uc: &UseCase, feature: Option<&Feature>, kinds: &[EntryKind]) -> Signals {
    let public_api = kinds.iter().filter(|k| **k == EntryKind::PublicApi).count();
    let human_entry = kinds
        .iter()
        .any(|k| matches!(k, EntryKind::HttpRoute | EntryKind::CliCommand));
    let human_actor = uc.steps.iter().any(|s| s.actor.kind == ActorKind::Human);
    let first_is_system = uc
        .steps
        .iter()
        .min_by_key(|s| s.order)
        .is_some_and(|s| s.actor.kind == ActorKind::System);
    let feature_text = feature.map_or(String::new(), |f| format!("{} {}", f.name, f.description));
    let in_admin = |p: &str| in_admin_folder(p);
    Signals {
        public_api_feature: !kinds.is_empty() && public_api * 2 > kinds.len(),
        human_without_human_entry: human_actor && !human_entry,
        system_first_step: uc.primary_actor.is_some() && human_actor && first_is_system,
        admin_path: uc
            .steps
            .iter()
            .flat_map(|s| &s.source_refs)
            .any(|r| in_admin(&r.path))
            || feature.is_some_and(|f| f.source_paths.iter().any(|p| in_admin(p))),
        admin_actor: uc.primary_actor.as_deref().is_some_and(mentions_admin),
        admin_feature: mentions_admin(&feature_text),
        low_business_language: uc
            .business_language
            .as_ref()
            .is_some_and(|s| s.value < MIN_BUSINESS_LANGUAGE),
        low_confidence: uc
            .confidence
            .as_ref()
            .is_some_and(|s| s.value < MIN_USE_CASE_CONFIDENCE),
        screen: uc
            .name
            .split_whitespace()
            .next()
            .is_some_and(|w| SCREEN_VERBS.contains(&w.to_lowercase().as_str())),
    }
}

fn percent(part: usize, whole: usize) -> String {
    if whole == 0 {
        "0%".into()
    } else {
        format!("{:.0}%", part as f64 * 100.0 / whole as f64)
    }
}

fn table(rows: &[(&str, usize)], total: usize) -> String {
    let mut out = String::from("| Signal | Use cases | Share |\n|---|---|---|\n");
    for (name, count) in rows {
        let _ = writeln!(out, "| {name} | {count} | {} |", percent(*count, total));
    }
    out
}

fn report(all: &[Signals]) -> String {
    let total = all.len();
    let count = |f: fn(&Signals) -> bool| all.iter().filter(|s| f(s)).count();
    let rows = [
        ("public_api feature", count(|s| s.public_api_feature)),
        (
            "human actor, no route nor command",
            count(|s| s.human_without_human_entry),
        ),
        (
            "named human actor, system first step",
            count(|s| s.system_first_step),
        ),
        ("admin folder (reference)", count(|s| s.admin_path)),
        ("admin actor", count(|s| s.admin_actor)),
        ("admin in the feature", count(|s| s.admin_feature)),
        (
            "business language < 0.8",
            count(|s| s.low_business_language),
        ),
        ("confidence < 0.6", count(|s| s.low_confidence)),
        ("open / show / view / edit screen", count(|s| s.screen)),
        (
            "set apart (admin feature or a threshold)",
            count(Signals::set_apart),
        ),
    ];
    let mut out = table(&rows, total);
    for (name, flag) in [
        (
            "admin actor",
            (|s: &Signals| s.admin_actor) as fn(&Signals) -> bool,
        ),
        ("admin in the feature", |s| s.admin_feature),
    ] {
        let both = all.iter().filter(|s| s.admin_path && flag(s)).count();
        let _ = writeln!(
            out,
            "\n{name} vs admin folder: {both} in both, {} signal only, {} folder only.",
            count(flag) - both,
            count(|s| s.admin_path) - both,
        );
    }
    out
}

fn run(repo_root: &Path) -> Option<String> {
    let use_cases = load_use_cases(repo_root)?;
    let features: BTreeMap<String, Feature> = load_features(repo_root)?
        .into_iter()
        .map(|f| (f.slug.clone(), f))
        .collect();
    let entry_points = EntryPoints::load(repo_root).unwrap_or_default();
    let all: Vec<Signals> = use_cases
        .iter()
        .map(|uc| {
            let feature = features.get(&uc.feature_slug);
            let kinds = feature.map_or_else(Vec::new, |f| kinds_of(f, &entry_points));
            signals(uc, feature, &kinds)
        })
        .collect();
    Some(format!(
        "## {}\n\n{} use cases.\n\n{}",
        repo_root.display(),
        all.len(),
        report(&all)
    ))
}

fn main() {
    let dirs: Vec<String> = std::env::args().skip(1).collect();
    if dirs.is_empty() {
        eprintln!("usage: technical_usecases_measure <run dir>...");
        std::process::exit(2);
    }
    for dir in dirs {
        match run(Path::new(&dir)) {
            Some(text) => println!("{text}"),
            None => eprintln!("{dir}: no use-cases.yaml or features.yaml"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use retrodoc_core::model::{Actor, ConfidenceScore, SourceRef, Step};

    fn step(order: u32, kind: ActorKind, path: &str) -> Step {
        Step {
            order,
            description: String::new(),
            actor: Actor {
                name: "Someone".into(),
                kind,
            },
            action: String::new(),
            source_refs: vec![SourceRef {
                path: path.into(),
                start_line: Some(1),
                end_line: Some(2),
            }],
        }
    }

    fn use_case(name: &str, actor: Option<&str>, steps: Vec<Step>) -> UseCase {
        UseCase {
            slug: "uc".into(),
            feature_slug: "f".into(),
            name: name.into(),
            description: String::new(),
            steps,
            entry_points: vec![],
            primary_actor: actor.map(String::from),
            narrative: None,
            business_language: None,
            diagram_mermaid: None,
            confidence: None,
        }
    }

    #[test]
    fn admin_words_need_a_whole_word() {
        assert!(mentions_admin("A System Administrator"));
        assert!(mentions_admin("the admin console"));
        assert!(!mentions_admin("an administered badge's"));
        assert!(!mentions_admin("Badminton club"));
    }

    #[test]
    fn a_human_actor_without_route_is_flagged() {
        let uc = use_case(
            "Generate the PDF",
            Some("Customer"),
            vec![step(1, ActorKind::Human, "app/pdf.rb")],
        );
        let s = signals(&uc, None, &[EntryKind::PublicApi, EntryKind::PublicApi]);
        assert!(s.public_api_feature);
        assert!(s.human_without_human_entry);
        let with_route = signals(&uc, None, &[EntryKind::HttpRoute]);
        assert!(!with_route.human_without_human_entry);
    }

    #[test]
    fn a_human_actor_with_a_system_first_step_is_flagged() {
        let uc = use_case(
            "Import",
            Some("Customer"),
            vec![
                step(2, ActorKind::Human, "a.rb"),
                step(1, ActorKind::System, "a.rb"),
            ],
        );
        assert!(signals(&uc, None, &[]).system_first_step);
    }

    #[test]
    fn thresholds_and_screens_and_admin_folder() {
        let mut uc = use_case(
            "Open the creation form",
            None,
            vec![step(1, ActorKind::Human, "app/controllers/admin/x.rb")],
        );
        uc.business_language = Some(ConfidenceScore::new(0.7, None));
        uc.confidence = Some(ConfidenceScore::new(0.5, None));
        let s = signals(&uc, None, &[]);
        assert!(s.low_business_language && s.low_confidence && s.screen && s.admin_path);
        assert!(s.set_apart());
        uc.business_language = Some(ConfidenceScore::new(0.9, None));
        uc.confidence = Some(ConfidenceScore::new(0.9, None));
        assert!(!signals(&uc, None, &[]).set_apart());
    }
}
