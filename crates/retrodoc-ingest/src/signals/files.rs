//! Signals read from the content of the files the walker found: Gherkin
//! features and test descriptions.

use std::fmt::Write as _;
use std::path::Path;

use crate::walker::{FileEntry, FileKind};

use super::test_phrases::test_phrases;
use super::{Signal, SignalKind};

/// Gherkin keywords whose line is a title worth keeping.
const GHERKIN_TITLES: &[&str] = &[
    "Feature:",
    "Rule:",
    "Background:",
    "Scenario:",
    "Scenario Outline:",
    "Scenario Template:",
    "Example:",
];

/// One signal per `.feature` file: its feature, rule and scenario titles.
#[must_use]
pub fn feature_scenarios(repo_root: &Path, files: &[FileEntry]) -> Vec<Signal> {
    files
        .iter()
        .filter(|f| f.path.extension().and_then(|e| e.to_str()) == Some("feature"))
        .filter_map(|file| {
            let content = read_lossy(repo_root, file)?;
            let mut text = String::new();
            for line in content.lines().map(str::trim) {
                if GHERKIN_TITLES.iter().any(|k| line.starts_with(k)) {
                    let _ = writeln!(text, "{line}");
                }
            }
            (!text.is_empty()).then(|| Signal {
                kind: SignalKind::FeatureScenarios,
                origin: file.path.to_string_lossy().replace('\\', "/"),
                text: text.trim_end().to_string(),
            })
        })
        .collect()
}

/// One signal per test file (by directory or file-name convention) that
/// has described blocks: what the tests say the application does.
#[must_use]
pub fn test_descriptions(repo_root: &Path, files: &[FileEntry]) -> Vec<Signal> {
    files
        .iter()
        .filter(|f| f.kind == FileKind::Test)
        .filter_map(|file| {
            let phrases = test_phrases(&read_lossy(repo_root, file)?);
            (!phrases.is_empty()).then(|| Signal {
                kind: SignalKind::TestDescriptions,
                origin: file.path.to_string_lossy().replace('\\', "/"),
                text: phrases.join("\n"),
            })
        })
        .collect()
}

fn read_lossy(repo_root: &Path, file: &FileEntry) -> Option<String> {
    match std::fs::read(repo_root.join(&file.path)) {
        Ok(bytes) => Some(String::from_utf8_lossy(&bytes).into_owned()),
        Err(error) => {
            tracing::warn!("skipping {}: {error}", file.path.display());
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn entry(path: &str, kind: FileKind) -> FileEntry {
        FileEntry {
            path: PathBuf::from(path),
            kind,
            size_bytes: 1,
        }
    }

    #[test]
    fn keeps_the_titles_of_a_feature_file() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("features")).unwrap();
        fs::write(
            dir.path().join("features/orders.feature"),
            "Feature: Orders\n  As a buyer\n  Background:\n    Given a cart\n  Scenario: Pay an order\n    When I pay\n  Scenario Outline: Refund <kind>\n",
        )
        .unwrap();
        fs::write(dir.path().join("features/empty.feature"), "# nothing\n").unwrap();
        let files = [
            entry("features/orders.feature", FileKind::Other),
            entry("features/empty.feature", FileKind::Other),
            entry("features/missing.feature", FileKind::Other),
        ];
        let signals = feature_scenarios(dir.path(), &files);
        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].origin, "features/orders.feature");
        assert_eq!(signals[0].kind, SignalKind::FeatureScenarios);
        assert_eq!(
            signals[0].text,
            "Feature: Orders\nBackground:\nScenario: Pay an order\nScenario Outline: Refund <kind>"
        );
    }

    #[test]
    fn describes_the_test_files_only() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("spec")).unwrap();
        fs::write(
            dir.path().join("spec/order_spec.rb"),
            "describe Order do\n  it \"cannot be paid twice\" do\n",
        )
        .unwrap();
        fs::write(dir.path().join("spec/empty_spec.rb"), "# no block\n").unwrap();
        fs::write(dir.path().join("order.rb"), "it \"not a test file\"\n").unwrap();
        let files = [
            entry("spec/order_spec.rb", FileKind::Test),
            entry("spec/empty_spec.rb", FileKind::Test),
            entry("order.rb", FileKind::Source),
        ];
        let signals = test_descriptions(dir.path(), &files);
        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].origin, "spec/order_spec.rb");
        assert_eq!(signals[0].text, "cannot be paid twice");
    }
}
