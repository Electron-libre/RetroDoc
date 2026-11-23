//! The debt report of the docs as shown: the one `retrodoc report` prints and `render` and
//! `generate` write in `_retrodoc/coverage-report.md` (`specs/section-edit.md`, SED-39), built from
//! the loaded [`GeneratedDocs`] so that no reader assembles its own.

use retrodoc_core::config::RulesConfig;
use retrodoc_ingest::existing_docs::ExistingDoc;
use retrodoc_pipeline::reconcile::ItemKind;
use retrodoc_pipeline::{build_report, DebtReport, MarkedWrong};

use crate::debt::{self, DebtKind, Subject};
use crate::read_model::GeneratedDocs;
use crate::reviews::{Outdated, Reviews};

/// What the debt report reads besides the loaded docs.
#[derive(Debug, Clone, Copy, Default)]
pub struct ReportInputs<'a> {
    /// The project's own docs the items are compared with; `None` when they could not be read,
    /// and the items are then not compared.
    pub collected: Option<&'a [ExistingDoc]>,
    /// The rules `retrodoc.toml` puts back among those the rules pass set aside; `None` while the
    /// rules pass is off: a file left by an earlier run is not what the last one judged.
    pub put_back: Option<&'a RulesConfig>,
    /// The reviews of the sections; `None` when they could not be read.
    pub reviews: Option<&'a Reviews>,
}

impl GeneratedDocs {
    /// The debt report of the docs as shown: the edited names on the items compared with the
    /// project's docs, their verdicts as given on the generated text (SED-40), and the sections
    /// marked wrong (DBT-35). Docs not loaded from a repository report their sections only.
    #[must_use]
    pub fn debt_report(&self, inputs: &ReportInputs) -> DebtReport {
        let mut report = build_report(self.domain_map(), self.features(), self.use_cases());
        report.skipped_files.clone_from(&self.uncovered().skipped);
        report.with_behaviour_coverage(self.uncovered().coverage.clone());
        if let Some(reviews) = inputs.reviews {
            report.marked_wrong = marked_wrong(self, reviews);
        }
        let Some(root) = self.repo_root() else {
            return report;
        };
        report.with_technical_features(retrodoc_core::artifacts::load_technical_features(root));
        if let Some(put_back) = inputs.put_back {
            report.with_set_aside(
                retrodoc_core::artifacts::SetAside::load(root).unwrap_or_default(),
                put_back,
                self.rules(),
            );
        }
        if let Some(collected) = inputs.collected {
            report.with_reconciliation(self.reconcile(root, collected));
        }
        report
    }
}

/// The sections of the run marked wrong, as the debt view lists them (DBT-30).
fn marked_wrong(docs: &GeneratedDocs, reviews: &Reviews) -> Vec<MarkedWrong> {
    debt::reviewed(docs, reviews)
        .filter(|item| item.kind == DebtKind::MarkedWrong)
        .filter_map(|item| {
            let kind = match &item.subject {
                Subject::Domain(_) => ItemKind::Domain,
                Subject::Section(section) if section.use_case.is_some() => ItemKind::UseCase,
                Subject::Section(_) => ItemKind::Feature,
                _ => return None,
            };
            Some(MarkedWrong {
                kind,
                label: item.label(),
                comment: item.rationale.unwrap_or_default(),
                outdated: item.outdated.as_ref().map(Outdated::report),
            })
        })
        .collect()
}
