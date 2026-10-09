//! Whether the code the docs cite is still the code they were generated from:
//! the current hash of each cited file against the one the last `generate`
//! recorded in `source-hashes.json` (written once a run has gone through every
//! pass, unlike the repo map cache), checked at every call so a long-running
//! server notices an edit made meanwhile.

use std::path::{Path, PathBuf};

use retrodoc_pipeline::cache::hash_content;
use retrodoc_pipeline::SourceHashes;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileState {
    /// Same content as when the docs were generated.
    Fresh,
    Modified,
    Deleted,
    /// No hash was recorded (a doc, a file left out by the file budget): not judged.
    Unknown,
}

pub struct Freshness {
    repo_root: PathBuf,
    recorded: SourceHashes,
}

impl Freshness {
    /// Reads the hashes recorded by the last complete `generate`; with none (never
    /// generated) every file is [`FileState::Unknown`].
    #[must_use]
    pub fn load(repo_root: &Path) -> Self {
        Self {
            repo_root: repo_root.to_path_buf(),
            recorded: SourceHashes::load(repo_root),
        }
    }

    #[must_use]
    pub fn state(&self, path: &str) -> FileState {
        let Some(recorded) = self.recorded.get(path) else {
            return FileState::Unknown;
        };
        match std::fs::read(self.repo_root.join(path)) {
            // Hashed as the repo map does (`SourceHashes::capture`): lossy UTF-8.
            Ok(bytes) if hash_content(&String::from_utf8_lossy(&bytes)) == recorded => {
                FileState::Fresh
            }
            Ok(_) => FileState::Modified,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => FileState::Deleted,
            Err(_) => FileState::Unknown,
        }
    }

    /// The files among `paths` that are modified or deleted, in order, with
    /// their state.
    #[must_use]
    pub fn stale<'a>(&self, paths: impl IntoIterator<Item = &'a str>) -> Vec<(&'a str, FileState)> {
        paths
            .into_iter()
            .map(|path| (path, self.state(path)))
            .filter(|(_, state)| matches!(state, FileState::Modified | FileState::Deleted))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A repo with `a.rb`, `b.rb`, `c.rb` and the hashes of their content
    /// recorded, as `generate` leaves them.
    fn generated_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let mut cache = SourceHashes::default();
        for name in ["a.rb", "b.rb", "c.rb"] {
            let content = format!("content of {name}");
            std::fs::write(dir.path().join(name), &content).unwrap();
            cache.put(name, &hash_content(&content));
        }
        cache.save(dir.path()).unwrap();
        dir
    }

    #[test]
    fn untouched_files_are_fresh() {
        let dir = generated_repo();
        let freshness = Freshness::load(dir.path());
        assert_eq!(freshness.state("a.rb"), FileState::Fresh);
        assert_eq!(freshness.stale(["a.rb", "b.rb"]), vec![]);
    }

    #[test]
    fn an_edit_or_a_deletion_after_generation_is_noticed_at_the_next_call() {
        let dir = generated_repo();
        let freshness = Freshness::load(dir.path());
        std::fs::write(dir.path().join("b.rb"), "edited").unwrap();
        std::fs::remove_file(dir.path().join("c.rb")).unwrap();
        assert_eq!(freshness.state("b.rb"), FileState::Modified);
        assert_eq!(freshness.state("c.rb"), FileState::Deleted);
        assert_eq!(
            freshness.stale(["a.rb", "b.rb", "c.rb"]),
            [("b.rb", FileState::Modified), ("c.rb", FileState::Deleted)]
        );
    }

    #[test]
    fn a_file_without_recorded_hash_is_not_judged() {
        let dir = generated_repo();
        std::fs::write(dir.path().join("README.md"), "docs").unwrap();
        let freshness = Freshness::load(dir.path());
        assert_eq!(freshness.state("README.md"), FileState::Unknown);
        assert_eq!(freshness.state("gone.rb"), FileState::Unknown);
        assert_eq!(freshness.stale(["README.md"]), vec![]);
    }

    #[test]
    fn a_repo_never_generated_has_nothing_to_compare() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rb"), "x").unwrap();
        assert_eq!(
            Freshness::load(dir.path()).state("a.rb"),
            FileState::Unknown
        );
    }

    #[test]
    fn a_repo_map_updated_by_a_failed_run_does_not_make_an_edit_look_fresh() {
        use retrodoc_pipeline::cache::RepoMapCache;
        let dir = generated_repo();
        // The next `generate` edits the file, rebuilds the repo map (which
        // records the new hash), then fails before the docs are rewritten:
        // `source-hashes.json` is left as the last complete run wrote it.
        std::fs::write(dir.path().join("a.rb"), "edited").unwrap();
        let mut repo_map = RepoMapCache::default();
        repo_map.put(Path::new("a.rb"), &hash_content("edited"), "summary");
        repo_map.save(dir.path()).unwrap();
        assert_eq!(
            Freshness::load(dir.path()).state("a.rb"),
            FileState::Modified
        );
    }

    /// Cost of the check, for `issues/mcp_freshness_cost.md`. Not part of the
    /// suite; run it optimized, the hash is slow in a debug build:
    /// `FRESHNESS_FILE_KB=20 cargo test -p retrodoc-mcp --release cost_of_the_check -- --ignored --nocapture`
    #[test]
    #[ignore = "measurement, not a check"]
    fn cost_of_the_check() {
        let kb: usize = std::env::var("FRESHNESS_FILE_KB")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(20);
        let dir = tempfile::tempdir().unwrap();
        let mut recorded = SourceHashes::default();
        let names: Vec<String> = (0..2000).map(|i| format!("f{i}.rb")).collect();
        for (i, name) in names.iter().enumerate() {
            let content = format!("{i} {}", "x".repeat(kb * 1024));
            std::fs::write(dir.path().join(name), &content).unwrap();
            recorded.put(name, &hash_content(&content));
        }
        recorded.save(dir.path()).unwrap();
        let freshness = Freshness::load(dir.path());
        println!("files of {kb} KiB");
        // A feature citing n files (get_feature), a search with `hits` hits of
        // 5 cited files each (search_docs), each hit checked on its own.
        for (label, calls, per_call) in [
            ("get_feature, 5 files", 1, 5),
            ("get_feature, 50 files", 1, 50),
            ("search_docs, 10 hits x 5 files", 10, 5),
            ("search_docs, 20 hits x 20 files", 20, 20),
        ] {
            let start = std::time::Instant::now();
            for call in 0..calls {
                let first = call * per_call;
                let paths = names[first..first + per_call].iter().map(String::as_str);
                assert_eq!(freshness.stale(paths), vec![]);
            }
            println!(
                "{label}: {} file(s), {} KiB hashed, {:.2} ms",
                calls * per_call,
                calls * per_call * kb,
                start.elapsed().as_secs_f64() * 1000.0
            );
        }
    }
}
