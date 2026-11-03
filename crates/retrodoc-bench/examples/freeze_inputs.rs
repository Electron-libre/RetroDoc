//! Freezes the artifacts of a good run as the inputs of the replays of one pass
//! (`issues/done/benchmark_passes_on_fixed_inputs.md`), outside the clone.
//!
//! Usage:
//! `cargo run -q -p retrodoc-pipeline --example freeze_inputs -- <run dir> <frozen dir> <repo> <commit> <model>`
//!
//! - `run dir`: a checkout with its `.retrodoc/cache` (a complete `generate`).
//! - `frozen dir`: e.g. `~/retrodoc-runs/frozen/<repo>`; gets `manifest.yaml` and `cache/`.
//! - `commit`, `model`: written in the manifest: the model that wrote the artifacts is part of the measure.

use std::path::Path;

use retrodoc_pipeline::benchmark::{FrozenInputs, Manifest};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [run, frozen, repo, commit, model] = args.as_slice() else {
        eprintln!("usage: freeze_inputs <run dir> <frozen dir> <repo> <commit> <model>");
        std::process::exit(2);
    };
    let manifest = Manifest {
        repo: repo.clone(),
        commit: commit.clone(),
        model: model.clone(),
        source_run: run.clone(),
        domains_origin: None,
        features_origin: None,
        use_cases_origin: None,
    };
    match FrozenInputs::freeze(Path::new(run), Path::new(frozen), &manifest) {
        Ok(frozen) => println!("{frozen:#?}"),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
