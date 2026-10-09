//! What every command starts with: the canonical repo root, its
//! `retrodoc.toml`, and the ingestion of the repo. One place, so the error
//! messages the user sees don't drift from one command to the next.

use std::path::{Path, PathBuf};

use anyhow::Context;
use retrodoc_core::config::Config;
use retrodoc_ingest::{FileKind, IngestResult};

/// The canonical root of the repo at `path`.
pub fn repo_root(path: &Path) -> anyhow::Result<PathBuf> {
    path.canonicalize()
        .with_context(|| format!("path not found: {}", path.display()))
}

/// A repo that `retrodoc init` has been run on.
pub struct Workspace {
    pub repo_root: PathBuf,
    pub config: Config,
}

impl Workspace {
    /// Opens the repo at `path` with its `retrodoc.toml`.
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let repo_root = repo_root(path)?;
        let config = Config::load(&repo_root)
            .with_context(|| "config not found — run `retrodoc init` first".to_string())?;
        Ok(Self { repo_root, config })
    }

    /// Walks the repo: files, git history and existing Markdown docs.
    /// The docs `generate` wrote are left out of the files, so a run that
    /// wrote them doesn't change the tree the passes saw before it.
    pub fn ingest(&self) -> anyhow::Result<IngestResult> {
        let mut ingest = retrodoc_ingest::run(&self.repo_root, &self.config.ingest)
            .with_context(|| format!("ingestion of {} failed", self.repo_root.display()))?;
        let generated = generated_dirs(&self.config);
        ingest
            .files
            .retain(|file| !generated.iter().any(|dir| file.path.starts_with(dir)));
        Ok(ingest)
    }
}

/// The directories `RetroDoc` writes its docs into, relative to the repo root.
pub fn generated_dirs(config: &Config) -> [PathBuf; 2] {
    let docs_dir = Path::new(&config.output.docs_dir);
    [
        docs_dir.join(retrodoc_render::FUNCTIONAL_DIR),
        docs_dir.join(retrodoc_render::META_DIR),
    ]
}

/// The paths of the files classified as source.
pub fn source_paths(ingest: &IngestResult) -> impl Iterator<Item = &Path> {
    ingest
        .files
        .iter()
        .filter(|f| f.kind == FileKind::Source)
        .map(|f| f.path.as_path())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git(dir: &Path, args: &[&str]) {
        let status = std::process::Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@t"])
            .args(args)
            .current_dir(dir)
            .status()
            .unwrap();
        assert!(status.success());
    }

    #[test]
    fn a_missing_path_is_reported_with_its_name() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope");
        let error = Workspace::open(&missing).err().unwrap();
        assert!(error.to_string().starts_with("path not found: "));
        assert!(error.to_string().contains("nope"));
    }

    #[test]
    fn a_repo_without_config_asks_for_init() {
        let dir = tempfile::tempdir().unwrap();
        let error = Workspace::open(dir.path()).err().unwrap();
        assert_eq!(
            error.to_string(),
            "config not found — run `retrodoc init` first"
        );
    }

    #[test]
    fn an_initialised_repo_is_ingested_and_its_sources_listed() {
        let dir = tempfile::tempdir().unwrap();
        Config::write_default(dir.path(), false).unwrap();
        std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
        std::fs::write(dir.path().join("notes.md"), "# Notes").unwrap();
        git(dir.path(), &["init", "-q"]);
        git(dir.path(), &["add", "main.rs", "notes.md"]);
        git(dir.path(), &["commit", "-q", "-m", "init"]);

        let workspace = Workspace::open(dir.path()).unwrap();
        let ingest = workspace.ingest().unwrap();

        assert_eq!(workspace.repo_root, dir.path().canonicalize().unwrap());
        assert_eq!(
            source_paths(&ingest).collect::<Vec<_>>(),
            [Path::new("main.rs")]
        );
    }

    #[test]
    fn the_docs_generate_wrote_are_not_among_the_files() {
        let dir = tempfile::tempdir().unwrap();
        Config::write_default(dir.path(), false).unwrap();
        std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
        let docs = dir.path().join("docs");
        std::fs::create_dir_all(docs.join("functional")).unwrap();
        std::fs::create_dir_all(docs.join("_retrodoc")).unwrap();
        std::fs::write(docs.join("functional/a.md"), "# A").unwrap();
        std::fs::write(docs.join("_retrodoc/run-metadata.json"), "{}").unwrap();
        std::fs::write(docs.join("guide.md"), "# Guide").unwrap();
        git(dir.path(), &["init", "-q"]);
        git(dir.path(), &["add", "."]);
        git(dir.path(), &["commit", "-q", "-m", "init"]);

        let ingest = Workspace::open(dir.path()).unwrap().ingest().unwrap();

        let paths: Vec<_> = ingest.files.iter().map(|f| f.path.as_path()).collect();
        assert!(paths.contains(&Path::new("main.rs")));
        assert!(paths.contains(&Path::new("docs/guide.md")));
        assert!(!paths.iter().any(|p| p.starts_with("docs/functional")));
        assert!(!paths.iter().any(|p| p.starts_with("docs/_retrodoc")));
    }
}
