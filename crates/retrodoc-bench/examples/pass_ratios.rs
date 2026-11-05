//! Measures the calls and tokens of each LLM pass of `generate` per unit of its input on saved runs, with
//! no LLM call (`retrodoc_bench::pass_ratios`, `issues/done/pass_ratios_from_usage_history.md`).
//!
//! Usage:
//! `cargo run -q -p retrodoc-bench --example pass_ratios -- <runs.yaml> [<out.yaml>]`
//!
//! - `runs.yaml`: the runs to measure, kept next to them outside git (it names their paths):
//!   `runs: [{path, application, commit, generates?, indicative?}]`, `application` a generic description.
//! - `out.yaml`: where to write the table (default: printed), e.g.
//!   `crates/retrodoc-pipeline/data/pass-ratios.yaml`.
//!
//! Exits with 2 when a run is no first complete `generate` or can't be read.

use std::path::PathBuf;
use std::process::ExitCode;

use retrodoc_bench::pass_ratios::{measure_run, ratio_table, RunManifest};

const HEADER: &str = "# The calls and tokens of each LLM pass of `generate` per unit of its input, measured on \
saved runs\n# (issues/done/pass_ratios_from_usage_history.md). Generated, do not edit: rebuild with\n# `cargo run -q -p \
retrodoc-bench --example pass_ratios -- <runs.yaml> <this file>`.\n# Calls without usage make the token sums a lower \
bound; `indicative` applications are out of the spread.\n";

fn main() -> ExitCode {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let (manifest, out) = match args.as_slice() {
        [manifest] => (manifest, None),
        [manifest, out] => (manifest, Some(out)),
        _ => {
            eprintln!("usage: pass_ratios <runs.yaml> [<out.yaml>]");
            return ExitCode::from(2);
        }
    };
    let manifest = match RunManifest::load(manifest) {
        Ok(manifest) => manifest,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
    };
    let mut runs = Vec::new();
    for entry in &manifest.runs {
        match measure_run(entry) {
            Ok(run) => {
                if run.units.missing_files > 0 {
                    eprintln!(
                        "{}: {} file(s) read missing from the tree, their characters not counted",
                        entry.path.display(),
                        run.units.missing_files
                    );
                }
                runs.push(run);
            }
            Err(error) => {
                eprintln!("{error}");
                return ExitCode::from(2);
            }
        }
    }
    let yaml = match ratio_table(&runs).to_yaml() {
        Ok(yaml) => format!("{HEADER}{yaml}"),
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
    };
    match out {
        Some(out) => {
            if let Err(error) = std::fs::write(out, yaml) {
                eprintln!("{}: {error}", out.display());
                return ExitCode::from(2);
            }
        }
        None => print!("{yaml}"),
    }
    ExitCode::SUCCESS
}
