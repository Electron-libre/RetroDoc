//! The human corrections of the features a slice of a domain writes (`specs/human-corrections.md`):
//! the corrections of the features that held an entry point of the slice when they were read.

use super::Entry;
use crate::corrections::{self, Correction, Corrections};
use crate::fingerprints::Fingerprints;

/// The pass, as [`Fingerprints::answered`] names its units.
const PASS: &str = "features";

/// What a slice is told of the features it writes, and the reviews `wrong` it answers.
pub(super) struct SliceCorrections {
    /// The part of the prompt that shows them, none when there are none.
    pub shown: Option<String>,
    /// The keys of the reviews `wrong` it answers.
    reviews: Vec<String>,
    /// Its key in [`Fingerprints::answered`].
    key: String,
}

impl SliceCorrections {
    /// The corrections of the features of `domain_slug` that held an entry point of `slice` when the
    /// corrections were read, the slice whose print is under `key` (HCR-6).
    pub(super) fn of(
        corrections: &Corrections,
        (domain_slug, key): (&str, &str),
        slice: &[Entry<'_>],
    ) -> Self {
        let fixes: Vec<Correction> = corrections
            .features_of(domain_slug)
            .iter()
            .filter(|c| {
                c.entry_points
                    .iter()
                    .any(|id| slice.iter().any(|e| &e.id == id))
            })
            .cloned()
            .collect();
        Self {
            shown: corrections::prompt_part("feature", &fixes),
            reviews: corrections::wrong_keys(&fixes),
            key: corrections::answered_key(PASS, key),
        }
    }

    /// Whether a review `wrong` asks the slice again (HCR-4).
    pub(super) fn pending(&self, prints: &Fingerprints) -> bool {
        corrections::pending(prints, &self.key, &self.reviews)
    }

    /// Records that the slice was answered (HCR-10).
    pub(super) fn answered(self, prints: &mut Fingerprints) {
        corrections::record_answered(prints, self.key, self.reviews);
    }
}
