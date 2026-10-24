//! Frozen inputs of a replay (`issues/done/benchmark_passes_on_fixed_inputs.md`): the artifacts of one good
//! run, kept outside the clone, from which one pass is replayed alone so that the passes upstream do not
//! vary between the runs that are compared.
//!
//! A frozen directory holds `manifest.yaml` (the repository, the commit, the model that wrote the
//! artifacts) and `cache/`, a copy of the `.retrodoc/cache/` of the run without what the replays redo.
//! [`FrozenInputs::prepare`] copies it into a work directory, leaving out the artifact of the replayed
//! pass and of the passes after it, and the fingerprints, so the pass runs from scratch.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::artifact::{load_yaml, save_yaml, Artifact};
use crate::error::PipelineError;

/// A pass that can be replayed alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayPass {
    Domains,
    Features,
    UseCases,
    Rules,
}

impl ReplayPass {
    /// What the pass reads from the passes before it: it must be frozen.
    fn needs(self) -> &'static [Artifact] {
        match self {
            ReplayPass::Domains => &[],
            ReplayPass::Features => &[Artifact::Domains],
            ReplayPass::UseCases => &[Artifact::Domains, Artifact::Features],
            ReplayPass::Rules => &[Artifact::Domains, Artifact::Features, Artifact::UseCases],
        }
    }

    /// What the pass writes, and what the passes after it wrote from it: left out of the work directory.
    fn redone(self) -> &'static [Artifact] {
        const FROM_RULES: &[Artifact] = &[
            Artifact::Rules,
            Artifact::RulesStated,
            Artifact::RulesSetAside,
        ];
        match self {
            ReplayPass::Domains => &[
                Artifact::Domains,
                Artifact::Features,
                Artifact::UseCases,
                Artifact::Rules,
                Artifact::RulesStated,
                Artifact::RulesSetAside,
                Artifact::BehaviourCoverage,
            ],
            ReplayPass::Features => &[
                Artifact::Features,
                Artifact::UseCases,
                Artifact::Rules,
                Artifact::RulesStated,
                Artifact::RulesSetAside,
                Artifact::BehaviourCoverage,
            ],
            ReplayPass::UseCases => &[
                Artifact::UseCases,
                Artifact::Rules,
                Artifact::RulesStated,
                Artifact::RulesSetAside,
            ],
            ReplayPass::Rules => FROM_RULES,
        }
    }
}

/// Artifacts every replay starts from.
const BASE: [Artifact; 7] = [
    Artifact::Roles,
    Artifact::SignalSources,
    Artifact::Product,
    Artifact::BusinessFiles,
    Artifact::Glossary,
    Artifact::EntryPoints,
    Artifact::Actors,
];

/// What `manifest.yaml` says about the frozen artifacts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub repo: String,
    /// The commit of the repository the run read.
    pub commit: String,
    /// The model that wrote the artifacts: they are only as good as it, say which.
    pub model: String,
    /// The run they were taken from.
    pub source_run: String,
    /// Where `domains.yaml` comes from when it is not the one of the run (see
    /// [`FrozenInputs::replace_domains`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domains_origin: Option<String>,
    /// The same for `features.yaml` (see [`FrozenInputs::replace_features`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub features_origin: Option<String>,
    /// The same for `use-cases.yaml` (see [`FrozenInputs::replace_use_cases`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub use_cases_origin: Option<String>,
}

/// The frozen artifacts of one repository.
#[derive(Debug, Clone)]
pub struct FrozenInputs {
    dir: PathBuf,
    pub manifest: Manifest,
}

fn io_error(path: &Path, source: std::io::Error) -> PipelineError {
    PipelineError::ArtifactIo {
        path: path.to_path_buf(),
        source,
    }
}

fn unusable(dir: &Path, reason: String) -> PipelineError {
    PipelineError::InvalidReference {
        path: dir.to_path_buf(),
        reason,
    }
}

impl FrozenInputs {
    fn cache(&self) -> PathBuf {
        self.dir.join("cache")
    }

    /// Copies the artifacts of the run at `run_root` (its `.retrodoc/cache/`) into `dir`, with the
    /// manifest. The artifacts that are not inputs of a replay (fingerprints, usage, hashes) stay behind.
    ///
    /// # Errors
    ///
    /// Returns an error if an artifact of the run is missing or a file can't be copied.
    pub fn freeze(
        run_root: &Path,
        dir: &Path,
        manifest: &Manifest,
    ) -> Result<FrozenInputs, PipelineError> {
        let cache = dir.join("cache");
        std::fs::create_dir_all(&cache).map_err(|e| io_error(&cache, e))?;
        let kept = BASE.into_iter().chain([
            Artifact::RepoMap,
            Artifact::Contexts,
            Artifact::Domains,
            Artifact::Features,
            Artifact::UseCases,
        ]);
        for artifact in kept {
            let from = artifact.path(run_root);
            if from.exists() {
                std::fs::copy(&from, cache.join(artifact.file_name()))
                    .map_err(|e| io_error(&from, e))?;
            }
        }
        save_yaml(&dir.join("manifest.yaml"), manifest)?;
        FrozenInputs::open(dir)
    }

    /// Replaces the frozen domains by those saved in the run at `run_root`, and says where they come from
    /// in the manifest. The domains of a run of the `files` path hold paths and no resources, so the
    /// features pass of the `surface` path finds no entry point in them: the replay of that pass freezes
    /// domains clustered from the surface.
    ///
    /// # Errors
    ///
    /// Returns an error if the run has no `domains.yaml` or a file can't be written.
    pub fn replace_domains(&mut self, run_root: &Path, origin: &str) -> Result<(), PipelineError> {
        self.copy_from(Artifact::Domains, run_root)?;
        self.manifest.domains_origin = Some(origin.to_string());
        save_yaml(&self.dir.join("manifest.yaml"), &self.manifest)
    }

    /// Replaces the frozen features by those saved in the run at `run_root`, as
    /// [`FrozenInputs::replace_domains`] does for the domains: the use cases replay starts from features
    /// of the `surface` path, the ones its domains hold.
    ///
    /// # Errors
    ///
    /// Returns an error if the run has no `features.yaml` or a file can't be written.
    pub fn replace_features(&mut self, run_root: &Path, origin: &str) -> Result<(), PipelineError> {
        self.copy_from(Artifact::Features, run_root)?;
        self.manifest.features_origin = Some(origin.to_string());
        save_yaml(&self.dir.join("manifest.yaml"), &self.manifest)
    }

    /// Replaces the frozen use cases by those saved in the run at `run_root`, as
    /// [`FrozenInputs::replace_features`] does for the features: the rules replay starts from use cases
    /// that belong to the frozen features.
    ///
    /// # Errors
    ///
    /// Returns an error if the run has no `use-cases.yaml` or a file can't be written.
    pub fn replace_use_cases(
        &mut self,
        run_root: &Path,
        origin: &str,
    ) -> Result<(), PipelineError> {
        self.copy_from(Artifact::UseCases, run_root)?;
        self.manifest.use_cases_origin = Some(origin.to_string());
        save_yaml(&self.dir.join("manifest.yaml"), &self.manifest)
    }

    fn copy_from(&self, artifact: Artifact, run_root: &Path) -> Result<(), PipelineError> {
        let from = artifact.path(run_root);
        std::fs::copy(&from, self.cache().join(artifact.file_name()))
            .map(|_| ())
            .map_err(|e| io_error(&from, e))
    }

    /// Opens a frozen directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the manifest is missing or unreadable, or if an artifact every replay starts
    /// from is missing.
    pub fn open(dir: &Path) -> Result<FrozenInputs, PipelineError> {
        let manifest: Manifest = load_yaml(&dir.join("manifest.yaml"))
            .ok_or_else(|| unusable(dir, "manifest.yaml is missing or unreadable".to_string()))?;
        let frozen = FrozenInputs {
            dir: dir.to_path_buf(),
            manifest,
        };
        for artifact in BASE {
            if !frozen.cache().join(artifact.file_name()).exists() {
                return Err(unusable(
                    dir,
                    format!("cache/{} is missing", artifact.file_name()),
                ));
            }
        }
        Ok(frozen)
    }

    /// Fills `work_root/.retrodoc/cache/` with the frozen artifacts a replay of `pass` starts from, and
    /// none of what it redoes.
    ///
    /// # Errors
    ///
    /// Returns an error if an artifact the pass reads from the passes before it was not frozen, or a
    /// file can't be copied.
    pub fn prepare(&self, pass: ReplayPass, work_root: &Path) -> Result<(), PipelineError> {
        for artifact in pass.needs() {
            if !self.cache().join(artifact.file_name()).exists() {
                return Err(unusable(
                    &self.dir,
                    format!(
                        "cache/{} is missing, a replay of the {pass:?} pass needs it",
                        artifact.file_name()
                    ),
                ));
            }
        }
        let target = Artifact::Roles.path(work_root);
        let target = target.parent().unwrap_or(work_root);
        std::fs::create_dir_all(target).map_err(|e| io_error(target, e))?;
        let redone = pass.redone();
        for entry in std::fs::read_dir(self.cache()).map_err(|e| io_error(&self.cache(), e))? {
            let entry = entry.map_err(|e| io_error(&self.cache(), e))?;
            let name = entry.file_name();
            if redone.iter().any(|a| a.file_name() == name) {
                continue;
            }
            std::fs::copy(entry.path(), target.join(&name))
                .map_err(|e| io_error(&entry.path(), e))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, artifact: Artifact, content: &str) {
        let path = artifact.path(root);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    fn manifest() -> Manifest {
        Manifest {
            repo: "demo".to_string(),
            commit: "abc123".to_string(),
            model: "some-model".to_string(),
            source_run: "run-1".to_string(),
            domains_origin: None,
            features_origin: None,
            use_cases_origin: None,
        }
    }

    /// A run with every input and some results of the replayed passes.
    fn run(root: &Path) {
        for artifact in BASE {
            write(root, artifact, "x");
        }
        write(root, Artifact::Domains, "domains");
        write(root, Artifact::Features, "features");
        write(root, Artifact::UseCases, "use cases");
        write(root, Artifact::Rules, "rules");
        write(root, Artifact::Fingerprints, "{}");
        write(root, Artifact::Usage, "[]");
    }

    #[test]
    fn freezing_keeps_the_inputs_and_leaves_the_bookkeeping() {
        let (run_dir, frozen_dir) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        run(run_dir.path());

        let frozen = FrozenInputs::freeze(run_dir.path(), frozen_dir.path(), &manifest()).unwrap();

        assert_eq!(frozen.manifest, manifest());
        let cache = frozen_dir.path().join("cache");
        assert!(cache.join("features.yaml").exists());
        assert!(!cache.join("fingerprints.json").exists());
        assert!(!cache.join("usage.json").exists());
        assert!(!cache.join("rules.yaml").exists());
    }

    #[test]
    fn a_replay_starts_without_its_own_result_nor_the_later_ones() {
        let (run_dir, frozen_dir, work) = (
            tempfile::tempdir().unwrap(),
            tempfile::tempdir().unwrap(),
            tempfile::tempdir().unwrap(),
        );
        run(run_dir.path());
        let frozen = FrozenInputs::freeze(run_dir.path(), frozen_dir.path(), &manifest()).unwrap();

        frozen.prepare(ReplayPass::Features, work.path()).unwrap();

        assert!(Artifact::Glossary.path(work.path()).exists());
        assert!(Artifact::Domains.path(work.path()).exists());
        assert!(!Artifact::Features.path(work.path()).exists());
        assert!(!Artifact::UseCases.path(work.path()).exists());
    }

    #[test]
    fn a_replay_needs_the_artifacts_of_the_passes_before_it() {
        let (run_dir, frozen_dir, work) = (
            tempfile::tempdir().unwrap(),
            tempfile::tempdir().unwrap(),
            tempfile::tempdir().unwrap(),
        );
        run(run_dir.path());
        std::fs::remove_file(Artifact::Features.path(run_dir.path())).unwrap();
        let frozen = FrozenInputs::freeze(run_dir.path(), frozen_dir.path(), &manifest()).unwrap();

        assert!(frozen.prepare(ReplayPass::Domains, work.path()).is_ok());
        let error = frozen
            .prepare(ReplayPass::UseCases, work.path())
            .unwrap_err();
        assert!(error.to_string().contains("features.yaml"), "{error}");
    }

    #[test]
    fn the_frozen_domains_can_be_replaced_and_the_manifest_says_so() {
        let (run_dir, frozen_dir, other) = (
            tempfile::tempdir().unwrap(),
            tempfile::tempdir().unwrap(),
            tempfile::tempdir().unwrap(),
        );
        run(run_dir.path());
        write(other.path(), Artifact::Domains, "other domains");
        let mut frozen =
            FrozenInputs::freeze(run_dir.path(), frozen_dir.path(), &manifest()).unwrap();

        frozen
            .replace_domains(other.path(), "surface, run 1")
            .unwrap();

        let saved = std::fs::read_to_string(frozen_dir.path().join("cache/domains.yaml")).unwrap();
        assert_eq!(saved, "other domains");
        let reopened = FrozenInputs::open(frozen_dir.path()).unwrap();
        assert_eq!(
            reopened.manifest.domains_origin.as_deref(),
            Some("surface, run 1")
        );
    }

    #[test]
    fn a_directory_without_manifest_or_inputs_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        assert!(FrozenInputs::open(dir.path()).is_err());

        save_yaml(&dir.path().join("manifest.yaml"), &manifest()).unwrap();
        let error = FrozenInputs::open(dir.path()).unwrap_err();
        assert!(error.to_string().contains("roles.yaml"), "{error}");
    }
}
