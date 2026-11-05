//! What a name an answer gives refers to in the docs: a domain, a feature, a use case or a file
//! they cite. The `ask` agent lists as sources only the names that resolve here (ADR 0047).

use std::fmt;

use super::Docs;
use crate::corpus::{feature_id, use_case_id};

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

    use super::*;
    use crate::corpus::fixtures::*;

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
}
