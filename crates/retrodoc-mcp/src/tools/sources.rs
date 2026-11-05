//! What a name an answer gives refers to in the docs: a domain, a feature, a use case or a file
//! they cite. The `ask` agent lists as sources only the names that resolve here (ADR 0047).

use std::fmt;

use retrodoc_core::model::LOW_CONFIDENCE_THRESHOLD;

use super::answer::percent;
use super::Docs;
use crate::corpus::{feature_id, use_case_id};
use crate::freshness::FileState;

/// The kind of a source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SourceKind {
    Domain,
    Feature,
    UseCase,
    File,
}

impl SourceKind {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            SourceKind::Domain => "domain",
            SourceKind::Feature => "feature",
            SourceKind::UseCase => "use case",
            SourceKind::File => "file",
        }
    }
}

/// A domain, a feature, a use case (by its full id, as the tools show it) or a file the docs cite
/// (by its path).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Source {
    pub kind: SourceKind,
    pub id: String,
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} `{}`", self.kind.label(), self.id)
    }
}

impl Docs {
    /// What `name` refers to: a file the docs cite (a `./` prefix and a `:line` or `:start-end`
    /// suffix allowed), a domain slug, a feature id or its slug alone, or a use case id with or
    /// without its domain. `None` for anything else.
    #[must_use]
    pub fn source_named(&self, name: &str) -> Option<Source> {
        let name = name.trim();
        let name = name.strip_prefix("./").unwrap_or(name);
        self.file_named(name)
            .or_else(|| self.file_named(without_lines(name)?))
            .or_else(|| self.section_named(name))
    }

    fn file_named(&self, path: &str) -> Option<Source> {
        self.generated
            .cited_paths()
            .iter()
            .any(|cited| cited == path)
            .then(|| Source {
                kind: SourceKind::File,
                id: path.to_string(),
            })
    }

    fn section_named(&self, name: &str) -> Option<Source> {
        let source = |kind, id| Some(Source { kind, id });
        if self.generated.domains().iter().any(|d| d.slug == name) {
            return source(SourceKind::Domain, name.to_string());
        }
        let features = self.generated.features();
        if let Some(feature) = features
            .iter()
            .find(|f| feature_id(f) == name || f.slug == name)
        {
            return source(SourceKind::Feature, feature_id(feature));
        }
        self.generated
            .use_cases()
            .iter()
            .map(|u| use_case_id(u, features))
            .find(|id| id == name || (name.contains('/') && id.ends_with(&format!("/{name}"))))
            .and_then(|id| source(SourceKind::UseCase, id))
    }
}

/// What a source of an answer warns of.
#[derive(Debug, Clone, PartialEq)]
pub enum Warning {
    /// A feature or a use case whose confidence is below [`LOW_CONFIDENCE_THRESHOLD`].
    LowConfidence { source: Source, confidence: f32 },
    /// A file changed (or deleted) since the docs were generated.
    Stale { path: String, deleted: bool },
}

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Warning::LowConfidence { source, confidence } => write!(
                f,
                "low confidence ({}) in {source}: the code may not back it",
                percent(*confidence)
            ),
            Warning::Stale { path, deleted } => write!(
                f,
                "`{path}` was {} since the docs were generated",
                if *deleted { "deleted" } else { "modified" }
            ),
        }
    }
}

impl Docs {
    /// The warnings of `sources`: each feature and use case below the low-confidence threshold,
    /// then each file changed since the docs were generated that is among them or that one of
    /// their features and use cases cites, each once.
    #[must_use]
    pub fn warnings_of(&self, sources: &[Source]) -> Vec<Warning> {
        let features = self.generated.features();
        let mut low = Vec::new();
        let mut files: Vec<&str> = Vec::new();
        for source in sources {
            let (confidence, cited): (_, Vec<&str>) = match source.kind {
                SourceKind::Domain => continue,
                SourceKind::File => (None, vec![source.id.as_str()]),
                SourceKind::Feature => {
                    let Some(feature) = features.iter().find(|f| feature_id(f) == source.id) else {
                        continue;
                    };
                    let cited = feature.source_paths.iter().map(String::as_str).collect();
                    (feature.confidence.as_ref(), cited)
                }
                SourceKind::UseCase => {
                    let Some(use_case) = self
                        .generated
                        .use_cases()
                        .iter()
                        .find(|u| use_case_id(u, features) == source.id)
                    else {
                        continue;
                    };
                    let cited = use_case
                        .steps
                        .iter()
                        .flat_map(|s| &s.source_refs)
                        .map(|r| r.path.as_str())
                        .collect();
                    (use_case.confidence.as_ref(), cited)
                }
            };
            if let Some(score) = confidence.filter(|c| c.value < LOW_CONFIDENCE_THRESHOLD) {
                low.push(Warning::LowConfidence {
                    source: source.clone(),
                    confidence: score.value,
                });
            }
            for path in cited {
                if !files.contains(&path) {
                    files.push(path);
                }
            }
        }
        let stale = self
            .generated
            .freshness()
            .map_or_else(Vec::new, |freshness| {
                freshness
                    .stale(files)
                    .into_iter()
                    .map(|(path, state)| Warning::Stale {
                        path: path.to_string(),
                        deleted: state == FileState::Deleted,
                    })
                    .collect()
            });
        low.into_iter().chain(stale).collect()
    }
}

/// `path` without a `:12` or `:12-30` suffix, `None` when it has none.
fn without_lines(name: &str) -> Option<&str> {
    let (path, lines) = name.rsplit_once(':')?;
    let digits = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    let ranged = lines
        .split_once('-')
        .map_or_else(|| digits(lines), |(a, b)| digits(a) && digits(b));
    ranged.then_some(path)
}

#[cfg(test)]
mod tests {
    use retrodoc_pipeline::Glossary;

    use retrodoc_core::model::ConfidenceScore;

    use super::*;
    use crate::corpus::fixtures::*;
    use crate::freshness::Freshness;

    fn docs() -> Docs {
        Docs::new(
            vec![domain("billing", "Billing", "Invoices and payments")],
            vec![feature(
                "pay-invoice",
                "Pay an invoice",
                "Settle what is due",
            )],
            vec![use_case(
                "pay-by-card",
                "pay-invoice",
                "Pay by card",
                "The accountant settles it",
            )],
            &Glossary::default(),
            &[],
        )
    }

    #[allow(clippy::unnecessary_wraps)] // compared with what `source_named` returns
    fn named(kind: SourceKind, id: &str) -> Option<Source> {
        Some(Source {
            kind,
            id: id.into(),
        })
    }

    // spec: ASK-7 (internal: the names are resolved against a read model built in the test)
    #[test]
    fn a_name_resolves_to_its_kind_and_full_id() {
        let docs = docs();
        let use_case = "billing/pay-invoice/pay-by-card";
        assert_eq!(
            docs.source_named("billing"),
            named(SourceKind::Domain, "billing")
        );
        for name in ["billing/pay-invoice", "pay-invoice"] {
            assert_eq!(
                docs.source_named(name),
                named(SourceKind::Feature, "billing/pay-invoice"),
                "{name}"
            );
        }
        for name in [use_case, "pay-invoice/pay-by-card"] {
            assert_eq!(
                docs.source_named(name),
                named(SourceKind::UseCase, use_case),
                "{name}"
            );
        }
        for name in [
            "app/mailer.rb",
            "./app/mailer.rb",
            "app/mailer.rb:12",
            "app/mailer.rb:3-9",
        ] {
            assert_eq!(
                docs.source_named(name),
                named(SourceKind::File, "app/mailer.rb"),
                "{name}"
            );
        }
    }

    // spec: ASK-8 (internal: the names are resolved against a read model built in the test)
    #[test]
    fn a_name_the_docs_do_not_hold_resolves_to_nothing() {
        let docs = docs();
        for name in [
            "shipping",
            "billing/refund",
            "pay-by-card",
            "app/other.rb",
            "app/mailer.rb:x",
            "",
        ] {
            assert_eq!(docs.source_named(name), None, "{name}");
        }
    }
    /// The docs of `docs()`, low confidence on the use case, over a repo where `generate`
    /// recorded the hashes of the two files they cite, then `app/mailer.rb` was modified.
    fn docs_with_changed_code() -> (tempfile::TempDir, Docs) {
        let dir = tempfile::tempdir().unwrap();
        let mut cache = retrodoc_pipeline::SourceHashes::default();
        for name in ["app/invoice.rb", "app/mailer.rb"] {
            let path = dir.path().join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, name).unwrap();
            cache.put(name, &retrodoc_pipeline::cache::hash_content(name));
        }
        cache.save(dir.path()).unwrap();
        std::fs::write(dir.path().join("app/mailer.rb"), "changed").unwrap();
        let mut low = use_case("pay-by-card", "pay-invoice", "Pay by card", "Settles it");
        low.confidence = Some(ConfidenceScore::new(0.3, None));
        let docs = Docs::new(
            vec![domain("billing", "Billing", "Invoices and payments")],
            vec![feature(
                "pay-invoice",
                "Pay an invoice",
                "Settle what is due",
            )],
            vec![low],
            &Glossary::default(),
            &[],
        )
        .with_freshness(Freshness::load(dir.path()));
        (dir, docs)
    }

    // spec: ASK-10 (internal: the confidence is set on a read model built in the test)
    #[test]
    fn a_source_below_the_threshold_is_warned_of_with_its_confidence() {
        let (_dir, docs) = docs_with_changed_code();
        let use_case = Source {
            kind: SourceKind::UseCase,
            id: "billing/pay-invoice/pay-by-card".into(),
        };
        let feature = Source {
            kind: SourceKind::Feature,
            id: "billing/pay-invoice".into(),
        };

        let warnings = docs.warnings_of(std::slice::from_ref(&feature));
        assert_eq!(warnings, []);
        let warnings = docs.warnings_of(std::slice::from_ref(&use_case));
        assert_eq!(
            warnings.first(),
            Some(&Warning::LowConfidence {
                source: use_case,
                confidence: 0.3
            })
        );
        assert_eq!(
            warnings[0].to_string(),
            "low confidence (30%) in use case `billing/pay-invoice/pay-by-card`: the code may not back it"
        );
    }

    // spec: ASK-11 (internal: the hashes of the generation are written by the test)
    #[test]
    fn a_changed_file_among_or_cited_by_the_sources_is_warned_of_once() {
        let (_dir, docs) = docs_with_changed_code();
        let file = |path: &str| Source {
            kind: SourceKind::File,
            id: path.into(),
        };
        let stale = Warning::Stale {
            path: "app/mailer.rb".into(),
            deleted: false,
        };

        assert_eq!(
            docs.warnings_of(&[file("app/mailer.rb")]),
            std::slice::from_ref(&stale)
        );
        assert_eq!(docs.warnings_of(&[file("app/invoice.rb")]), []);
        let use_case = Source {
            kind: SourceKind::UseCase,
            id: "billing/pay-invoice/pay-by-card".into(),
        };
        let warnings = docs.warnings_of(&[use_case, file("app/mailer.rb")]);
        assert_eq!(warnings[1..], *std::slice::from_ref(&stale));
        assert_eq!(
            stale.to_string(),
            "`app/mailer.rb` was modified since the docs were generated"
        );
    }
}
