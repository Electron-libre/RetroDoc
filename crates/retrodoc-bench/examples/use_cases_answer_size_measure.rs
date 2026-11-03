//! Where the characters of the use cases answer go, and how much shorter candidate shapes would be, from
//! saved runs with no LLM call (`issues/done/shorter_use_case_answers.md`, step 1).
//!
//! Usage: `cargo run -q -p retrodoc-pipeline --example use_cases_answer_size_measure -- <run>...`
//!
//! A run is a `use-cases.yaml`, or a dir holding one under `.retrodoc/cache/` or `cache/` (frozen inputs).
//! The answer is rebuilt as compact JSON in the shape the prompt asks for, from the use cases that were
//! saved: it is an estimate (steps the grounding dropped are missing, entry point tags are counted as
//! `E1`). Printed per run: use cases per feature, steps per use case, the share of system steps, the share
//! of the characters of each field (the keys and braces are the rest), and the size of the answer under
//! each candidate shape against the current one:
//!
//! - (a) a citation as `[file number, start, end]`, the files numbered in the prompt;
//! - (b) one text per step (the description, no action) and the actor as a name;
//! - (c) at most `MAX_STEPS` steps per use case (the first ones);
//! - (a+b+c) all three.

#![allow(clippy::cast_precision_loss)] // counts of characters: far below 2^52

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use retrodoc_core::model::{ActorKind, SourceRef, Step, UseCase};
use serde_json::{json, Value};

/// The cap of steps of candidate (c).
const MAX_STEPS: usize = 5;

/// What the answer shape keeps of the steps.
#[derive(Clone, Copy)]
struct Shape {
    compact_refs: bool,
    one_text: bool,
    max_steps: Option<usize>,
}

const CURRENT: Shape = Shape {
    compact_refs: false,
    one_text: false,
    max_steps: None,
};

fn refs(refs: &[SourceRef], compact: bool) -> Value {
    if compact {
        // The number of a file in the prompt: two digits at most.
        refs.iter()
            .map(|r| json!([12, r.start_line, r.end_line]))
            .collect()
    } else {
        refs.iter()
            .map(|r| json!({"path": r.path, "start_line": r.start_line, "end_line": r.end_line}))
            .collect()
    }
}

fn step(step: &Step, shape: Shape) -> Value {
    let refs_key = if shape.compact_refs {
        "refs"
    } else {
        "source_refs"
    };
    let refs = refs(&step.source_refs, shape.compact_refs);
    if shape.one_text {
        json!({"text": step.description, "actor": step.actor.name, refs_key: refs})
    } else {
        let kind = match step.actor.kind {
            ActorKind::Human => "human",
            ActorKind::System => "system",
        };
        json!({
            "description": step.description,
            "actor": {"name": step.actor.name, "kind": kind},
            "action": step.action,
            refs_key: refs,
        })
    }
}

/// The use case as the LLM writes it in `shape`.
fn answer(use_case: &UseCase, shape: Shape) -> Value {
    let kept = shape.max_steps.unwrap_or(usize::MAX);
    let steps: Vec<Value> = use_case
        .steps
        .iter()
        .take(kept)
        .map(|s| step(s, shape))
        .collect();
    json!({
        "slug": use_case.slug.as_str(),
        "name": use_case.name,
        "description": use_case.description,
        "steps": steps,
        "entry_points": vec!["E1"; use_case.entry_points.len()],
        "primary_actor": use_case.primary_actor,
        "narrative": use_case.narrative,
    })
}

fn chars(value: &Value) -> usize {
    value.to_string().chars().count()
}

/// Characters of the whole answer of `use_cases` in `shape`.
fn size(use_cases: &[UseCase], shape: Shape) -> usize {
    use_cases.iter().map(|u| chars(&answer(u, shape))).sum()
}

/// Characters of each field of the current shape, the values only.
#[derive(Debug, Default, PartialEq, Eq)]
struct Fields {
    step_description: usize,
    citations: usize,
    paths: usize,
    narrative: usize,
    head: usize, // slug, name, description
    actor: usize,
    action: usize,
}

impl Fields {
    fn of(use_cases: &[UseCase]) -> Self {
        let mut fields = Fields::default();
        for use_case in use_cases {
            let text = |s: &str| chars(&json!(s));
            fields.narrative += chars(&json!(use_case.narrative));
            fields.head +=
                text(use_case.slug.as_str()) + text(&use_case.name) + text(&use_case.description);
            for s in &use_case.steps {
                let Value::Object(map) = step(s, CURRENT) else {
                    continue;
                };
                fields.step_description += chars(&map["description"]);
                fields.actor += chars(&map["actor"]);
                fields.action += chars(&map["action"]);
                fields.citations += chars(&map["source_refs"]);
                fields.paths += s.source_refs.iter().map(|r| text(&r.path)).sum::<usize>();
            }
        }
        fields
    }
}

fn percent(part: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        part as f64 * 100.0 / total as f64
    }
}

fn use_cases_file(run: &Path) -> PathBuf {
    if run.is_file() {
        return run.to_path_buf();
    }
    let saved = run.join(".retrodoc/cache/use-cases.yaml");
    if saved.is_file() {
        saved
    } else {
        run.join("cache/use-cases.yaml")
    }
}

fn measure(run: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let file = use_cases_file(run);
    let use_cases: Vec<UseCase> = serde_yaml::from_str(&std::fs::read_to_string(&file)?)?;
    let features: BTreeSet<&str> = use_cases.iter().map(|u| u.feature_slug.as_str()).collect();
    let steps: Vec<&Step> = use_cases.iter().flat_map(|u| &u.steps).collect();
    let system = steps
        .iter()
        .filter(|s| s.actor.kind == ActorKind::System)
        .count();
    let total = size(&use_cases, CURRENT);
    println!(
        "== {}: {} use cases, {:.1} per feature, {:.1} steps per use case, {:.0}% system steps, {total} chars",
        file.display(),
        use_cases.len(),
        use_cases.len() as f64 / features.len().max(1) as f64,
        steps.len() as f64 / use_cases.len().max(1) as f64,
        percent(system, steps.len()),
    );
    let fields = Fields::of(&use_cases);
    for (name, part) in [
        ("step description", fields.step_description),
        ("citations", fields.citations),
        ("  of which paths", fields.paths),
        ("narrative", fields.narrative),
        ("slug, name, description", fields.head),
        ("actor object", fields.actor),
        ("action", fields.action),
    ] {
        println!("  {name:24} {part:8} {:5.1}%", percent(part, total));
    }
    let candidates = [
        (
            "(a) compact citations",
            Shape {
                compact_refs: true,
                ..CURRENT
            },
        ),
        (
            "(b) one text, actor name",
            Shape {
                one_text: true,
                ..CURRENT
            },
        ),
        (
            "(c) at most 5 steps",
            Shape {
                max_steps: Some(MAX_STEPS),
                ..CURRENT
            },
        ),
        (
            "(a+b+c)",
            Shape {
                compact_refs: true,
                one_text: true,
                max_steps: Some(MAX_STEPS),
            },
        ),
    ];
    for (name, shape) in candidates {
        let candidate = size(&use_cases, shape);
        println!(
            "  {name:24} {candidate:8} {:+5.1}%",
            percent(candidate, total) - 100.0
        );
    }
    Ok(())
}

fn main() {
    let runs: Vec<String> = std::env::args().skip(1).collect();
    if runs.is_empty() {
        eprintln!("usage: use_cases_answer_size_measure <run>...");
        std::process::exit(2);
    }
    for run in &runs {
        if let Err(error) = measure(Path::new(run)) {
            eprintln!("{run}: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ARTIFACT: &str = r"
- slug: create-bookmark
  feature_slug: bookmarks
  name: Create Bookmark
  description: The user creates a bookmark.
  steps:
  - order: 1
    description: The user submits the form.
    actor: {name: User, kind: human}
    action: Submits the form
    source_refs: [{path: app/views.py, start_line: 7, end_line: 9}]
  - {order: 2, description: Saves it., actor: {name: System, kind: system}, action: Saves, source_refs: []}
  - {order: 3, description: Loads the favicon., actor: {name: System, kind: system}, action: Loads, source_refs: []}
  - {order: 4, description: Takes a snapshot., actor: {name: System, kind: system}, action: Snapshots, source_refs: []}
  - {order: 5, description: Indexes it., actor: {name: System, kind: system}, action: Indexes, source_refs: []}
  - {order: 6, description: Redirects., actor: {name: System, kind: system}, action: Redirects, source_refs: []}
  narrative: The user saves a link.
";

    fn use_cases() -> Vec<UseCase> {
        serde_yaml::from_str(ARTIFACT).unwrap()
    }

    #[test]
    fn counts_the_values_of_each_field() {
        let fields = Fields::of(&use_cases());
        assert_eq!(fields.paths, "\"app/views.py\"".len());
        assert_eq!(
            fields.action,
            [
                "Submits the form",
                "Saves",
                "Loads",
                "Snapshots",
                "Indexes",
                "Redirects"
            ]
            .iter()
            .map(|a| a.len() + 2)
            .sum::<usize>()
        );
        assert_eq!(fields.narrative, "\"The user saves a link.\"".len());
    }

    #[test]
    fn every_candidate_is_shorter_and_the_cap_keeps_the_first_steps() {
        let use_cases = use_cases();
        let current = size(&use_cases, CURRENT);
        let compact = Shape {
            compact_refs: true,
            ..CURRENT
        };
        let one_text = Shape {
            one_text: true,
            ..CURRENT
        };
        let capped = Shape {
            max_steps: Some(MAX_STEPS),
            ..CURRENT
        };
        for shape in [compact, one_text, capped] {
            assert!(size(&use_cases, shape) < current);
        }
        let steps = answer(&use_cases[0], capped)["steps"].clone();
        assert_eq!(steps.as_array().unwrap().len(), MAX_STEPS);
        assert_eq!(steps[0]["action"], "Submits the form");
        assert_eq!(answer(&use_cases[0], compact)["steps"][0]["refs"][0][1], 7);
    }
}
