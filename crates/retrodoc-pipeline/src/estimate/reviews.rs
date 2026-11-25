//! The units the reviews `wrong` not yet answered ask again at the next `generate` (HCR-4), and their cost in the
//! estimate (`specs/run-estimate.md`, EST-12, EST-13). The reviews are read as the passes read them: the corrections
//! the command loaded, and the reviews each saved unit answered ([`Fingerprints::answered`]).
//!
//! A domain reviewed asks the domains call again; a feature reviewed, the features call of its domain (one call per
//! domain: the estimate doesn't cut a domain into the slices of the pass; a feature with no entry point is asked by
//! no slice); a use case reviewed, the use cases call of
//! its feature. A feature whose use cases are written again cascades into its rules and the confidence of its use
//! cases; the domains call cascades into nothing counted, as when a changed file asks it.

use std::collections::BTreeSet;
use std::path::Path;

use retrodoc_core::artifacts::estimate::{Basis, PassEstimate, Range};

use super::{scaled, EstimateInputs, RatioTable};
use crate::corrections::{Correction, Corrections};
use crate::domains::UNCATEGORIZED_SLUG;
use crate::fingerprints::Fingerprints;

/// The names the passes record their answered reviews under, in [`Fingerprints::answered`].
const DOMAINS: &str = "domains";
const FEATURES: &str = "features";
const USE_CASES: &str = "use cases";

/// The units the reviews `wrong` not yet answered ask again, and those they cascade into.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReviewedUnits {
    /// A domain reviewed: the domains call is asked again.
    pub domains: bool,
    /// The domains whose features call a reviewed feature asks again.
    pub feature_domains: usize,
    /// The features whose use cases and rules are asked again: those reviewed, and those of a reviewed use case.
    pub features: usize,
    /// The use cases saved for those features, which the confidence reads again.
    pub use_cases: usize,
}

impl ReviewedUnits {
    /// The units `corrections` ask again in the repository `repo_root`; `use_cases_of` gives the number of use
    /// cases saved for a feature, by its slug.
    #[must_use]
    pub fn pending(
        repo_root: &Path,
        corrections: &Corrections,
        use_cases_of: impl Fn(&str) -> usize,
    ) -> Self {
        Self::of(&Fingerprints::load(repo_root), corrections, use_cases_of)
    }

    fn of(
        prints: &Fingerprints,
        corrections: &Corrections,
        use_cases_of: impl Fn(&str) -> usize,
    ) -> Self {
        // The uncategorized domain is gathered by the code, not written by the model.
        let domains = corrections
            .domains()
            .iter()
            .filter(|c| c.slug != UNCATEGORIZED_SLUG)
            .any(|c| unanswered(prints, DOMAINS, c, |unit| unit == "-"));
        let mut features: BTreeSet<&str> = BTreeSet::new();
        let mut feature_domains = 0;
        for (domain, fixes) in corrections.features() {
            // A slice of the domain: `<domain>/-` asked in one call, `<domain>/slice-<n>` otherwise.
            let in_domain = |unit: &str| {
                unit.strip_prefix(domain)
                    .is_some_and(|rest| rest.starts_with('/'))
            };
            let before = features.len();
            features.extend(
                fixes
                    .iter()
                    // With no entry point, no slice shows it: the pass never asks it again.
                    .filter(|c| !c.entry_points.is_empty())
                    .filter(|c| unanswered(prints, FEATURES, c, in_domain))
                    .map(|c| c.slug.as_str()),
            );
            feature_domains += usize::from(features.len() > before);
        }
        for (feature, fixes) in corrections.use_cases() {
            let of_feature = format!("/{feature}");
            if fixes
                .iter()
                .any(|c| unanswered(prints, USE_CASES, c, |unit| unit.ends_with(&of_feature)))
            {
                features.insert(feature);
            }
        }
        Self {
            domains,
            feature_domains,
            features: features.len(),
            use_cases: features.iter().map(|feature| use_cases_of(feature)).sum(),
        }
    }

    /// The units of the pass `pass` they ask again.
    fn of_pass(&self, pass: &str) -> usize {
        match pass {
            "domains" => usize::from(self.domains),
            "features" => self.feature_domains,
            "use-cases" | "rules" => self.features,
            "confidence" => self.use_cases,
            _ => 0,
        }
    }
}

/// Whether `correction` is a review `wrong` that no unit of the pass `pass` whose key `unit` accepts has answered
/// (HCR-4, HCR-10).
fn unanswered(
    prints: &Fingerprints,
    pass: &str,
    correction: &Correction,
    unit: impl Fn(&str) -> bool,
) -> bool {
    let Some(review) = correction.wrong_key() else {
        return false;
    };
    let prefix = format!("{pass}:");
    !prints.answered.iter().any(|(key, reviews)| {
        key.strip_prefix(&prefix).is_some_and(&unit) && reviews.contains(&review)
    })
}

/// `pass` with the units the reviews ask again added, at its measured cost per unit: on a rerun without `--force`
/// only (a first run or `--force` reads every file already).
pub(super) fn add(
    mut pass: PassEstimate,
    inputs: &EstimateInputs,
    table: &RatioTable,
) -> PassEstimate {
    if inputs.changed_files.is_none()
        || !matches!(
            pass.basis,
            Basis::Cached | Basis::PerRun | Basis::PerSourceFile
        )
    {
        return pass;
    }
    let units = inputs.reviewed.of_pass(&pass.pass);
    let Some(spread) = table
        .pass(&pass.pass)
        .and_then(|measured| measured.per_unit)
    else {
        return pass;
    };
    if units == 0 {
        return pass;
    }
    let asked = scaled(spread, units);
    let basis = pass.basis;
    let before = pass.calls.low;
    if basis == Basis::Cached {
        pass.basis = Basis::Reviewed;
    }
    for (range, asked) in [
        (&mut pass.calls, asked.calls),
        (&mut pass.prompt_tokens, asked.prompt_tokens),
        (&mut pass.completion_tokens, asked.completion_tokens),
    ] {
        match basis {
            Basis::Cached => *range = asked,
            // A pass made once a run: the review makes its call certain, not a second one.
            Basis::PerRun => {
                range.low = range.low.max(asked.low);
                range.high = range.high.max(asked.high);
            }
            _ => Range::add(range, asked),
        }
    }
    // One call a unit, whatever the lowest yield of the table: the review makes it certain.
    if matches!(pass.pass.as_str(), "features" | "use-cases") {
        pass.calls.low = pass.calls.low.max(before + units as u64);
    }
    pass
}

#[cfg(test)]
#[path = "reviews_tests.rs"]
mod tests;
