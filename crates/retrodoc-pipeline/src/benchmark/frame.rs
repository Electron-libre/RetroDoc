//! The global frame a replay gives the features or use cases pass: the product brief and the extracts of
//! the signals closest to each unit (ADR 0019), set per replay to compare the ways of injecting them
//! (`issues/done/brief_injection_experiments.md`).

use std::path::Path;

use retrodoc_core::config::Config;
use retrodoc_ingest::signals;

use crate::brief::{Evidence, ProductBrief};
use crate::error::PipelineError;
use crate::sources::saved_or_sniffed;

/// What frames the prompts of a replayed pass. The default is the frozen brief and no evidence: `generate`
/// also gives the evidence by default (`brief.evidence`), built from the tree with [`tree_evidence`].
#[derive(Debug, Clone, Default)]
pub struct Frame {
    /// Leave the frozen product brief out of the prompts.
    pub without_brief: bool,
    /// The extracts closest to each unit (`brief.evidence`); empty: none.
    pub evidence: Evidence,
}

impl Frame {
    /// The brief the pass is given: the frozen one in `work_root`, or none.
    pub(super) fn brief(&self, work_root: &Path) -> ProductBrief {
        if self.without_brief {
            ProductBrief::default()
        } else {
            ProductBrief::load(work_root).unwrap_or_default()
        }
    }
}

fn unusable(tree: &Path, reason: String) -> PipelineError {
    PipelineError::InvalidReference {
        path: tree.to_path_buf(),
        reason,
    }
}

/// The extracts `generate` would give the units of the repository at `tree` with `brief.evidence` on:
/// the signals of its existing docs (the generated ones left out), commits, tests and data, with the
/// signal sources saved in its own `.retrodoc/cache` (the run the frozen inputs come from).
///
/// # Errors
///
/// Returns an error if `retrodoc.toml` of the tree is invalid, or the tree can't be ingested.
pub fn tree_evidence(tree: &Path) -> Result<Evidence, PipelineError> {
    let config = if tree.join(retrodoc_core::config::CONFIG_FILE_NAME).exists() {
        // A saved run may name settings removed since (ADR 0035): only what it read is needed.
        Config::load_ingest_and_output(tree).map_err(|e| unusable(tree, e.to_string()))?
    } else {
        Config::default()
    };
    let mut ingest =
        retrodoc_ingest::run(tree, &config.ingest).map_err(|e| unusable(tree, e.to_string()))?;
    let generated = config.output.generated_dirs();
    ingest
        .files
        .retain(|file| !generated.iter().any(|dir| file.path.starts_with(dir)));
    ingest
        .existing_docs
        .retain(|doc| !generated.iter().any(|dir| doc.path.starts_with(dir)));
    let sources = saved_or_sniffed(tree, &ingest.files);
    let all =
        signals::collect(tree, &ingest, &sources).map_err(|e| unusable(tree, e.to_string()))?;
    Ok(Evidence::new(&all))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, file: &str, content: &str) {
        let path = root.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn the_evidence_of_a_tree_holds_its_docs_but_not_the_generated_ones() {
        let tree = tempfile::tempdir().unwrap();
        git2::Repository::init(tree.path()).unwrap();
        write(
            tree.path(),
            "docs/billing.md",
            "# Billing\n\nInvoices are paid monthly by the tenant.\n",
        );
        write(
            tree.path(),
            "docs/functional/invoices.md",
            "# Invoices\n\nGenerated invoices page, paid by nobody.\n",
        );

        let section = tree_evidence(tree.path()).unwrap().section("invoices paid");

        assert!(section.contains("paid monthly by the tenant"), "{section}");
        assert!(!section.contains("Generated invoices page"), "{section}");
    }

    #[test]
    fn a_saved_run_of_the_removed_files_path_still_gives_its_evidence() {
        let tree = tempfile::tempdir().unwrap();
        git2::Repository::init(tree.path()).unwrap();
        write(
            tree.path(),
            "retrodoc.toml",
            "[llm]\nbatch_chars = 6000\n\n[ingest]\nexisting_docs_paths = [\"notes\"]\n\n\
             [pipeline]\ndomains_from = \"files\"\n",
        );
        write(
            tree.path(),
            "notes/billing.md",
            "# Billing\n\nInvoices are paid monthly by the tenant.\n",
        );

        let section = tree_evidence(tree.path()).unwrap().section("invoices paid");

        assert!(section.contains("paid monthly by the tenant"), "{section}");
    }
}
