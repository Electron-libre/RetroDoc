//! How much the glossary and the entry points passes read twice, from saved runs, with no LLM call
//! (`issues/single_read_per_business_file.md`, step 1).
//!
//! Usage: `cargo run -q -p retrodoc-pipeline --example read_overlap -- <run dir>...`
//!
//! A run dir is a checkout with the `.retrodoc/cache/` of a `generate` (or of the `glossary` and
//! `entry-points` commands). A file is read by a pass when it has an entry in its artifact
//! (`glossary.yaml` models, `entry-points.yaml` files). The characters are those of the files on disk, so
//! this is the most a joint reading could save in input (the prompts' fixed part is not counted).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use retrodoc_pipeline::{EntryPoints, Glossary};

/// Files each pass read, and the characters of the shared ones.
#[derive(Debug, Default, PartialEq, Eq)]
struct Overlap {
    glossary_files: usize,
    entry_files: usize,
    shared_files: usize,
    glossary_chars: usize,
    entry_chars: usize,
    shared_chars: usize,
}

impl Overlap {
    fn of(
        glossary: &BTreeSet<PathBuf>,
        entries: &BTreeSet<PathBuf>,
        size: impl Fn(&Path) -> usize,
    ) -> Self {
        let shared: BTreeSet<&PathBuf> = glossary.intersection(entries).collect();
        Overlap {
            glossary_files: glossary.len(),
            entry_files: entries.len(),
            shared_files: shared.len(),
            glossary_chars: glossary.iter().map(|p| size(p)).sum(),
            entry_chars: entries.iter().map(|p| size(p)).sum(),
            shared_chars: shared.iter().map(|p| size(p)).sum(),
        }
    }

    /// Share of the input of the two passes that a joint reading would not send twice.
    #[allow(clippy::cast_precision_loss)] // character counts: far below 2^52
    fn saved_share(&self) -> f64 {
        let total = self.glossary_chars + self.entry_chars;
        100.0 * self.shared_chars as f64 / total.max(1) as f64
    }

    fn row(&self, run: &str) -> String {
        format!(
            "| {run} | {} | {} | {} | {} | {} | {} | {:.0}% |",
            self.glossary_files,
            self.entry_files,
            self.shared_files,
            self.glossary_chars,
            self.entry_chars,
            self.shared_chars,
            self.saved_share()
        )
    }
}

fn main() {
    let runs: Vec<String> = std::env::args().skip(1).collect();
    if runs.is_empty() {
        eprintln!("usage: read_overlap <run dir>...");
        std::process::exit(2);
    }
    println!(
        "| run | glossary files | entry points files | shared | glossary chars | entry points chars | shared chars | input saved |"
    );
    println!("|---|---|---|---|---|---|---|---|");
    for run in &runs {
        let root = Path::new(run);
        let (Some(glossary), Some(entries)) = (Glossary::load(root), EntryPoints::load(root))
        else {
            println!("| {run} | no glossary.yaml or entry-points.yaml |||||||");
            continue;
        };
        let glossary_files = glossary.models.keys().cloned().collect();
        let entry_files = entries.files.keys().cloned().collect();
        let size =
            |p: &Path| std::fs::read_to_string(root.join(p)).map_or(0, |c| c.chars().count());
        println!(
            "{}",
            Overlap::of(&glossary_files, &entry_files, size).row(run)
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_only_the_files_both_passes_read() {
        let set = |names: &[&str]| names.iter().map(PathBuf::from).collect::<BTreeSet<_>>();
        let overlap = Overlap::of(&set(&["a.rb", "b.rb"]), &set(&["b.rb", "c.rb"]), |p| {
            if p == Path::new("b.rb") {
                100
            } else {
                50
            }
        });
        assert_eq!(
            overlap,
            Overlap {
                glossary_files: 2,
                entry_files: 2,
                shared_files: 1,
                glossary_chars: 150,
                entry_chars: 150,
                shared_chars: 100,
            }
        );
        assert!((overlap.saved_share() - 100.0 / 3.0).abs() < 1e-9);
    }
}
