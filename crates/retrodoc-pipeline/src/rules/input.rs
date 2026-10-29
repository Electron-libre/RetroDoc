//! What the rules of one feature are read from: the code its use cases are read from (its entry
//! points and the code they run, else its files), the glossary entities that code mentions, and the
//! closest documentation sections and test descriptions, with their origin so that a rule can cite
//! them. No LLM, deterministic.

use std::path::Path;

use retrodoc_core::model::Feature;

use crate::brief::Extract;
use crate::entry_points::EntryPoint;
use crate::text::read_file_prefix_lossy;
use crate::use_cases::{feature_query, FeatureInput, UseCaseContext};

/// Of each file, the bytes searched for entity names.
const MAX_BYTES_PER_FILE: usize = 200_000;

/// The lowercase words of `text`, split at anything that is not a letter or digit and inside
/// `camelCase` and `PascalCase` identifiers (`OrderLine` gives `order`, `line`).
fn words_of(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut previous_lower = false;
    for c in text.chars() {
        if !c.is_alphanumeric() {
            previous_lower = false;
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
            continue;
        }
        if c.is_uppercase() && previous_lower && !word.is_empty() {
            words.push(std::mem::take(&mut word));
        }
        previous_lower = c.is_lowercase();
        word.extend(c.to_lowercase());
    }
    if !word.is_empty() {
        words.push(word);
    }
    words
}

#[derive(Debug, Clone)]
pub struct RuleInput {
    /// The entry points of the feature, with their file.
    pub entries: Vec<(std::path::PathBuf, EntryPoint)>,
    /// The files whose code is read, repo-relative: the same as for the use cases.
    pub files: Vec<String>,
    /// The entities of the glossary that the feature's text or code names.
    pub entities: Vec<String>,
    /// The documentation sections and test descriptions closest to the feature.
    pub evidence: Vec<Extract>,
}

impl RuleInput {
    #[must_use]
    pub fn new(repo_root: &Path, feature: &Feature, context: &UseCaseContext) -> Self {
        let input = FeatureInput::new(repo_root, feature, &context.entry_points, &context.index);
        let evidence = context
            .rule_evidence
            .extracts(&feature_query(feature, &input));
        let mut text = format!("{} {}", feature.name, feature.description);
        for path in &input.files {
            if let Ok(prefix) =
                read_file_prefix_lossy(repo_root, Path::new(path), MAX_BYTES_PER_FILE)
            {
                text.push('\n');
                text.push_str(&prefix);
            }
        }
        let words = words_of(&text);
        let entities = context
            .vocabulary_for(&feature.domain_slug)
            .iter()
            .filter(|name| {
                let wanted = words_of(name);
                !wanted.is_empty() && words.windows(wanted.len()).any(|w| w == wanted)
            })
            .cloned()
            .collect();
        Self {
            entries: input.entries,
            files: input.files,
            entities,
            evidence,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use retrodoc_ingest::signals::{Signal, SignalKind};

    use super::*;
    use crate::brief::{Evidence, ExtractKind};
    use crate::entry_points::{EntryFile, EntryKind, EntryPoints};
    use crate::slices::CodeIndex;

    fn feature(paths: &[&str]) -> Feature {
        Feature {
            back_office: false,
            slug: "refunds".into(),
            domain_slug: "billing".into(),
            sub_domain_slug: None,
            name: "Refund an order".into(),
            description: "A buyer gets money back".into(),
            source_paths: paths.iter().map(ToString::to_string).collect(),
            entry_points: Vec::new(),
            confidence: None,
        }
    }

    fn signal(kind: SignalKind, origin: &str, text: &str) -> Signal {
        Signal {
            kind,
            origin: origin.into(),
            text: text.into(),
        }
    }

    fn write(root: &Path, files: &[(&str, &str)]) {
        for (path, content) in files {
            std::fs::create_dir_all(root.join(path).parent().unwrap()).unwrap();
            std::fs::write(root.join(path), content).unwrap();
        }
    }

    #[test]
    fn gathers_the_code_the_entities_and_the_evidence_of_a_feature() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            &[
                (
                    "app/refunds_controller.rb",
                    "def create\n  Ledger.credit(order) unless order.paid?\nend\n",
                ),
                (
                    "app/ledger.rb",
                    "class Ledger\n  def self.credit(o); end\nend\n",
                ),
                ("app/unrelated.rb", "class Weather\nend\n"),
            ],
        );
        let entry_points = EntryPoints {
            files: BTreeMap::from([(
                PathBuf::from("app/refunds_controller.rb"),
                EntryFile {
                    content_hash: String::new(),
                    entry_points: vec![EntryPoint {
                        kind: EntryKind::HttpRoute,
                        name: "POST /refunds".into(),
                        verb: "create".into(),
                        resource: "refund".into(),
                        description: "Refund an order".into(),
                        outputs: Vec::new(),
                    }],
                },
            )]),
        };
        let context = UseCaseContext {
            entry_points,
            index: CodeIndex::new(
                [
                    "app/refunds_controller.rb",
                    "app/ledger.rb",
                    "app/unrelated.rb",
                ]
                .map(Path::new),
            ),
            vocabulary: vec!["Order".into(), "Ledger".into(), "Invoice".into()],
            rule_evidence: Evidence::new(&[
                signal(
                    SignalKind::TestDescriptions,
                    "spec/refund_spec.rb",
                    "refuses a refund when the order is not paid",
                ),
                signal(
                    SignalKind::DocSection,
                    "README.md#Refunds",
                    "A refund is only possible for a paid order.",
                ),
                signal(
                    SignalKind::DocSection,
                    "README.md#Install",
                    "Run the installer.",
                ),
            ]),
            ..UseCaseContext::default()
        };

        let input = RuleInput::new(
            dir.path(),
            &feature(&["app/refunds_controller.rb"]),
            &context,
        );

        assert_eq!(input.entries.len(), 1);
        assert_eq!(input.files, ["app/refunds_controller.rb", "app/ledger.rb"]);
        assert_eq!(input.entities, ["Order", "Ledger"]);
        let evidence: Vec<(ExtractKind, &str)> = input
            .evidence
            .iter()
            .map(|e| (e.kind, e.origin.as_str()))
            .collect();
        assert_eq!(
            evidence,
            [
                (ExtractKind::Doc, "README.md#Refunds"),
                (ExtractKind::Test, "spec/refund_spec.rb"),
            ]
        );
    }

    #[test]
    fn an_entity_is_a_word_of_the_code_not_a_piece_of_one() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            &[(
                "lib/a.rb",
                "border = itemize(OrderLine.new)\nclass Credit_Note; end\n",
            )],
        );
        let context = UseCaseContext {
            vocabulary: ["Order", "Item", "Order Line", "Credit Note", "Note book"]
                .map(String::from)
                .into(),
            ..UseCaseContext::default()
        };
        let input = RuleInput::new(dir.path(), &feature(&["lib/a.rb"]), &context);
        assert_eq!(input.entities, ["Order", "Order Line", "Credit Note"]);
    }

    #[test]
    fn a_feature_without_entry_points_reads_its_own_files() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), &[("lib/router.rb", "class Router; end\n")]);
        let input = RuleInput::new(
            dir.path(),
            &feature(&["lib/router.rb", "lib/gone.rb"]),
            &UseCaseContext::default(),
        );
        assert_eq!(input.files, ["lib/router.rb", "lib/gone.rb"]);
        assert!(input.entities.is_empty() && input.evidence.is_empty());
    }
}
