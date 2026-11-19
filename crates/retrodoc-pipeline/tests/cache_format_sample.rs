//! A frozen sample of the artifacts the readers of the generated docs load, in the current format
//! (`specs/cache-format.md`, CFV-10): each is read by its loader and saved again to the same text, so a
//! renamed, removed or added field fails here until the change of format is decided.
//!
//! On a failure: if a reader of an older version would read the new shape wrong, bump
//! `FORMAT_VERSION` in `cache_format.rs` and write the sample under its new number; else refresh the
//! sample in place. `RETRODOC_BLESS=1 cargo test -p retrodoc-pipeline --test cache_format_sample`
//! writes the sample from what the loaders save, read from the latest earlier sample after a bump
//! (a field the bump adds then holds its default: fill it by hand).

use std::path::{Path, PathBuf};

use retrodoc_pipeline::cache_format::FORMAT_VERSION;
use retrodoc_pipeline::reconcile::Judgements;
use retrodoc_pipeline::{BehaviourCoverage, DomainMap, Glossary, Scope};

fn sample_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(format!("cache-format-{FORMAT_VERSION}"))
}

/// The sample to read: that of the current format, else, to bless the sample of a bumped version, the
/// sample of the latest format before it.
fn source_dir() -> PathBuf {
    let sample = sample_dir();
    if sample.is_dir() || !blessing() {
        assert!(
            sample.is_dir(),
            "no sample of format {FORMAT_VERSION} at {}: write it with RETRODOC_BLESS=1",
            sample.display()
        );
        return sample;
    }
    (1..FORMAT_VERSION)
        .rev()
        .map(|v| sample.with_file_name(format!("cache-format-{v}")))
        .find(|dir| dir.is_dir())
        .expect("no sample of an earlier format to start from")
}

fn blessing() -> bool {
    std::env::var_os("RETRODOC_BLESS").is_some()
}

/// A repository whose cache is the sample.
fn repo_of_the_sample() -> tempfile::TempDir {
    let sample = source_dir();
    let repo = tempfile::tempdir().unwrap();
    let cache = repo.path().join(".retrodoc/cache");
    std::fs::create_dir_all(&cache).unwrap();
    for entry in std::fs::read_dir(&sample).unwrap() {
        let path = entry.unwrap().path();
        std::fs::copy(&path, cache.join(path.file_name().unwrap())).unwrap();
    }
    repo
}

/// The text each artifact the readers load saves again, by file name, read by its loader.
fn saved_again(root: &Path) -> Vec<(&'static str, String)> {
    vec![
        (
            "features.yaml",
            yaml(&retrodoc_pipeline::load_features(root).expect("features.yaml")),
        ),
        (
            "use-cases.yaml",
            yaml(&retrodoc_pipeline::load_use_cases(root).expect("use-cases.yaml")),
        ),
        (
            "domains.yaml",
            yaml(&DomainMap::load(root).expect("domains.yaml")),
        ),
        (
            "glossary.yaml",
            yaml(&Glossary::load(root).expect("glossary.yaml")),
        ),
        (
            "rules.yaml",
            yaml(&retrodoc_pipeline::load_rules(root).expect("rules.yaml")),
        ),
        ("scope.yaml", yaml(&Scope::load(root).expect("scope.yaml"))),
        (
            "behaviour-coverage.yaml",
            yaml(&BehaviourCoverage::load(root).expect("behaviour-coverage.yaml")),
        ),
        ("reconciliation.yaml", yaml(&Judgements::load(root))),
    ]
}

/// `value` saved as the pipeline saves an artifact.
fn yaml<T: serde::Serialize>(value: &T) -> String {
    serde_yaml::to_string(value).unwrap()
}

// spec: CFV-10 (internal: the shape of the artifacts is a property of the types the loaders read, no command shows it)
#[test]
fn the_artifacts_of_the_current_format_read_back_and_save_to_the_same_text() {
    let repo = repo_of_the_sample();

    let saved = saved_again(repo.path());

    if blessing() {
        std::fs::create_dir_all(sample_dir()).unwrap();
        for (name, text) in &saved {
            std::fs::write(sample_dir().join(name), text).unwrap();
        }
        return;
    }
    for (name, text) in saved {
        let frozen = std::fs::read_to_string(sample_dir().join(name)).unwrap();
        assert_eq!(
            text, frozen,
            "the shape of {name} changed: bump FORMAT_VERSION if an older reader would read it wrong, \
             else refresh the sample (see the doc of this file)"
        );
    }
}

// spec: CFV-10 (internal: the shape of the artifacts is a property of the types the loaders read, no command shows it)
#[test]
fn a_renamed_field_of_the_sample_no_longer_saves_to_the_same_text() {
    let repo = repo_of_the_sample();
    let features = repo.path().join(".retrodoc/cache/features.yaml");
    let text = std::fs::read_to_string(&features).unwrap();
    std::fs::write(&features, text.replace("source_paths:", "source_files:")).unwrap();

    let saved = saved_again(repo.path());

    let (_, again) = saved.iter().find(|(n, _)| *n == "features.yaml").unwrap();
    assert_ne!(
        *again,
        std::fs::read_to_string(sample_dir().join("features.yaml")).unwrap()
    );
}
