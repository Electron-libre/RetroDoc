//! Checks the evidence of a benchmark reference against a checkout of its repository, and re-reads saved
//! run reports with the features marked `disputed` taken out, with no LLM call (spec
//! `benchmark-reference-evidence`, `issues/benchmark_references_anchored_in_evidence.md`).
//!
//! Usage:
//! `cargo run -q -p retrodoc-bench --example reference_evidence -- <reference.yaml> <tree> [<run.json>...]`
//!
//! - `reference.yaml`: `benchmark/<repo>/reference.yaml`.
//! - `tree`: a checkout of the repository at the commit of the reference.
//! - `run.json`: reports written by `retrodoc benchmark --out`, scored against this reference.
//!
//! Prints when the reference was checked, the features by what backs them, each cited path or heading
//! missing in the tree, then per report the feature recall and precision before and after the disputed
//! features are taken out. Exits with 1 when a path or heading is missing.

use std::path::PathBuf;
use std::process::ExitCode;

use retrodoc_bench::benchmark::evidence::{check_evidence, evidence_summary};
use retrodoc_bench::benchmark::{Reference, RunReport};

fn main() -> ExitCode {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let [reference, tree, reports @ ..] = args.as_slice() else {
        eprintln!("usage: reference_evidence <reference.yaml> <tree> [<run.json>...]");
        return ExitCode::from(2);
    };
    let reference = match Reference::load(reference) {
        Ok(reference) => reference,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
    };
    let mut loaded = Vec::new();
    for path in reports {
        let report = std::fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|raw| serde_json::from_str::<RunReport>(&raw).map_err(|e| e.to_string()));
        match report {
            Ok(report) => loaded.push((path.display().to_string(), report)),
            Err(error) => {
                eprintln!("{}: {error}", path.display());
                return ExitCode::from(2);
            }
        }
    }
    let check = check_evidence(&reference, tree);
    print!("{}", evidence_summary(&reference, &check, &loaded));
    if check.is_clean() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
