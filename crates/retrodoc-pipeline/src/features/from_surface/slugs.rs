//! The saved slugs of the features a slice writes again (`specs/rerun.md` RER-13 to RER-15): a
//! feature of the answer holding the entry points of the slice a saved feature of the domain held
//! takes its slug back. A saved feature over several slices is matched on its entry points of the
//! slice, so its part written again merges with the rest by slug, as the saved run had it.

use std::collections::BTreeSet;

use retrodoc_core::model::Feature;

use super::Entry;
use crate::saved_slugs::keep_saved_slugs;

/// Gives the features `produced` by a slice of the domain `domain_slug` the slugs of the saved
/// features of `previous` with the same entry points in `slice`: those the answer leaves free (an
/// entry point the placement had put into a feature) left out. A slug stays taken when a feature
/// of another domain (`others`) holds it, or a feature of an earlier slice (`earlier`) that is not
/// a part of the saved feature: one holding an entry point the saved feature did not.
pub(super) fn keep_saved(
    produced: &mut [Feature],
    previous: &[Feature],
    (domain_slug, slice): (&str, &[Entry<'_>]),
    (others, earlier): (&[Feature], &[Feature]),
) {
    let claimed: BTreeSet<&str> = produced
        .iter()
        .flat_map(|f| &f.entry_points)
        .map(String::as_str)
        .collect();
    let in_slice: BTreeSet<&str> = slice
        .iter()
        .map(|e| e.id.as_str())
        .filter(|id| claimed.contains(id))
        .collect();
    let of_domain: Vec<&Feature> = previous
        .iter()
        .filter(|f| f.domain_slug == domain_slug)
        .collect();
    let saved: Vec<Feature> = of_domain
        .iter()
        .filter_map(|f| {
            let entry_points: Vec<String> = f
                .entry_points
                .iter()
                .filter(|id| in_slice.contains(id.as_str()))
                .cloned()
                .collect();
            (!entry_points.is_empty()).then(|| Feature {
                entry_points,
                ..(*f).clone()
            })
        })
        .collect();
    let part_of_saved = |holder: &Feature| {
        of_domain.iter().any(|f| {
            f.slug == holder.slug
                && holder
                    .entry_points
                    .iter()
                    .all(|id| f.entry_points.contains(id))
        })
    };
    keep_saved_slugs(produced, &saved, |slug| {
        others.iter().any(|f| f.slug == slug)
            || earlier.iter().any(|f| f.slug == slug && !part_of_saved(f))
    });
}
