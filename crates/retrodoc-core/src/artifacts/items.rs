//! The items the passes find, as saved in `.retrodoc/cache/`: the features (`features.yaml`, and those set
//! aside as technical in `features-set-aside.yaml`), the use cases (`use-cases.yaml`) and the business
//! rules (`rules.yaml`). Each loader gives `None` for a missing or unreadable file (a first run), not an
//! error.

use std::path::Path;

use crate::cache::{load_yaml, save_yaml, Artifact, CacheError};
use crate::model::{BusinessRule, Feature, UseCase};

/// Loads a previously saved `features.yaml`.
#[must_use]
pub fn load_features(repo_root: &Path) -> Option<Vec<Feature>> {
    load_yaml(&Artifact::Features.path(repo_root))
}

/// Persists `features` as `.retrodoc/cache/features.yaml`; also used to re-save once confidence scores
/// are attached.
///
/// # Errors
///
/// Returns an error if the file can't be written or serialization fails.
pub fn save_features(repo_root: &Path, features: &[Feature]) -> Result<(), CacheError> {
    save_yaml(&Artifact::Features.path(repo_root), features)
}

/// Loads the features the features pass set aside as technical (`features-set-aside.yaml`).
#[must_use]
pub fn load_technical_features(repo_root: &Path) -> Option<Vec<Feature>> {
    load_yaml(&Artifact::FeaturesSetAside.path(repo_root))
}

/// Loads a previously saved `use-cases.yaml`.
#[must_use]
pub fn load_use_cases(repo_root: &Path) -> Option<Vec<UseCase>> {
    load_yaml(&Artifact::UseCases.path(repo_root))
}

/// Persists `use_cases` as `.retrodoc/cache/use-cases.yaml`; also used to re-save once diagrams are
/// attached.
///
/// # Errors
///
/// Returns an error if the file can't be written or serialization fails.
pub fn save_use_cases(repo_root: &Path, use_cases: &[UseCase]) -> Result<(), CacheError> {
    save_yaml(&Artifact::UseCases.path(repo_root), use_cases)
}

/// Loads a previously saved `rules.yaml`.
#[must_use]
pub fn load_rules(repo_root: &Path) -> Option<Vec<BusinessRule>> {
    load_yaml(&Artifact::Rules.path(repo_root))
}

/// Persists `rules` as `.retrodoc/cache/rules.yaml`.
///
/// # Errors
///
/// Returns an error if the file can't be written or serialization fails.
pub fn save_rules(repo_root: &Path, rules: &[BusinessRule]) -> Result<(), CacheError> {
    save_yaml(&Artifact::Rules.path(repo_root), rules)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_saved_loads_as_none() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load_features(dir.path()).is_none());
        assert!(load_technical_features(dir.path()).is_none());
        assert!(load_use_cases(dir.path()).is_none());
        assert!(load_rules(dir.path()).is_none());
    }

    #[test]
    fn saved_items_load_back() {
        let dir = tempfile::tempdir().unwrap();
        save_features(dir.path(), &[]).unwrap();
        save_use_cases(dir.path(), &[]).unwrap();
        save_rules(dir.path(), &[]).unwrap();
        assert!(load_features(dir.path()).is_some_and(|items| items.is_empty()));
        assert!(load_use_cases(dir.path()).is_some_and(|items| items.is_empty()));
        assert!(load_rules(dir.path()).is_some_and(|items| items.is_empty()));
    }
}
