//! The slugs a unit asked again gives back (`specs/rerun.md`, RER-13 to RER-16): an edit or a
//! review names its section by slug, and a model asked again for a whole unit renames sections it
//! writes the same. A section of the answer that holds the entry points (the resources, for a
//! domain) of a saved section of its unit takes back the saved slug, one to one: the match renames,
//! it never merges two sections the model split on purpose.

use std::collections::BTreeSet;

use retrodoc_core::artifacts::DomainCluster;
use retrodoc_core::model::{Feature, UseCase};

use crate::naming::normalize;

/// A section of the docs a unit writes, as the match reads it.
pub(crate) trait Section {
    fn slug(&self) -> &str;
    fn set_slug(&mut self, slug: String);
    /// What tells the section apart in its unit: its entry points, or a domain's resources.
    fn held(&self) -> BTreeSet<String>;
}

impl Section for UseCase {
    fn slug(&self) -> &str {
        &self.slug
    }
    fn set_slug(&mut self, slug: String) {
        self.slug = slug;
    }
    fn held(&self) -> BTreeSet<String> {
        self.entry_points.iter().cloned().collect()
    }
}

impl Section for Feature {
    fn slug(&self) -> &str {
        &self.slug
    }
    fn set_slug(&mut self, slug: String) {
        self.slug = slug;
    }
    fn held(&self) -> BTreeSet<String> {
        self.entry_points.iter().cloned().collect()
    }
}

impl Section for DomainCluster {
    fn slug(&self) -> &str {
        &self.slug
    }
    fn set_slug(&mut self, slug: String) {
        self.slug = slug;
    }
    fn held(&self) -> BTreeSet<String> {
        self.resources.iter().map(|r| normalize(r)).collect()
    }
}

/// Gives the sections of `produced` the slug of the section of `saved` (the saved sections of the
/// same unit) that holds the same entry points (RER-13). A saved slug is given back only when no
/// section of the answer nor any section `held_elsewhere` says holds it, when the section of the
/// answer has no saved slug of the unit itself, and when no other section, of the answer or saved,
/// holds the same entry points (RER-14, RER-15).
pub(crate) fn keep_saved_slugs<P: Section, S: Section>(
    produced: &mut [P],
    saved: &[S],
    held_elsewhere: impl Fn(&str) -> bool,
) {
    let saved_slugs: BTreeSet<&str> = saved.iter().map(Section::slug).collect();
    let saved_held: Vec<BTreeSet<String>> = saved.iter().map(Section::held).collect();
    let produced_held: Vec<BTreeSet<String>> = produced.iter().map(Section::held).collect();
    for (section, held) in saved.iter().zip(&saved_held) {
        let slug = section.slug();
        if held.is_empty()
            || held_elsewhere(slug)
            || produced.iter().any(|p| p.slug() == slug)
            || saved_held.iter().filter(|h| *h == held).count() > 1
        {
            continue;
        }
        let mut same = (0..produced.len()).filter(|&i| produced_held[i] == *held);
        let (Some(found), None) = (same.next(), same.next()) else {
            continue;
        };
        if saved_slugs.contains(produced[found].slug()) {
            continue;
        }
        tracing::info!(from = %produced[found].slug(), to = %slug, "section written again keeps its saved slug");
        produced[found].set_slug(slug.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn use_case(slug: &str, entry_points: &[&str]) -> UseCase {
        UseCase {
            slug: slug.to_string(),
            feature_slug: "f".to_string(),
            name: String::new(),
            description: String::new(),
            steps: Vec::new(),
            entry_points: entry_points.iter().map(ToString::to_string).collect(),
            primary_actor: None,
            narrative: None,
            business_language: None,
            diagram_mermaid: None,
            confidence: None,
            narrative_check: None,
        }
    }

    fn slugs(sections: &[UseCase]) -> Vec<&str> {
        sections.iter().map(|s| s.slug.as_str()).collect()
    }

    fn kept(produced: &[(&str, &[&str])], saved: &[(&str, &[&str])]) -> Vec<String> {
        let mut produced: Vec<UseCase> = produced.iter().map(|(s, e)| use_case(s, e)).collect();
        let saved: Vec<UseCase> = saved.iter().map(|(s, e)| use_case(s, e)).collect();
        keep_saved_slugs(&mut produced, &saved, |_| false);
        slugs(&produced).into_iter().map(String::from).collect()
    }

    // spec: RER-13 (internal: the match itself, each unit's binary path tested apart)
    #[test]
    fn a_section_with_the_entry_points_of_a_saved_one_takes_its_slug() {
        assert_eq!(
            kept(
                &[
                    ("estimate-a", &["E1"]),
                    ("estimate-b", &["E2"]),
                    ("c", &["E3"])
                ],
                &[
                    ("compute-a", &["E1"]),
                    ("compute-b", &["E2"]),
                    ("c", &["E3"])
                ],
            ),
            ["compute-a", "compute-b", "c"]
        );
        // The order of the entry points does not matter.
        assert_eq!(
            kept(&[("new", &["E2", "E1"])], &[("old", &["E1", "E2"])]),
            ["old"]
        );
    }

    // spec: RER-14 (internal: a split the fake application of the binary tests can't answer)
    #[test]
    fn a_section_split_merged_or_with_no_entry_point_keeps_its_slug() {
        // Split in two.
        assert_eq!(
            kept(&[("x", &["E1"]), ("y", &["E2"])], &[("old", &["E1", "E2"])]),
            ["x", "y"]
        );
        // Two merged in one.
        assert_eq!(
            kept(&[("x", &["E1", "E2"])], &[("a", &["E1"]), ("b", &["E2"])]),
            ["x"]
        );
        // No entry point, saved or written again.
        assert_eq!(kept(&[("x", &[])], &[("old", &[])]), ["x"]);
    }

    // spec: RER-15 (internal: the answers the fake application of the binary tests can't give)
    #[test]
    fn a_saved_slug_held_or_ambiguous_is_not_given_back() {
        // The answer moves a saved slug to another section: both keep the answer's slugs.
        assert_eq!(
            kept(
                &[("b", &["E1"]), ("c", &["E2"])],
                &[("a", &["E1"]), ("b", &["E2"])]
            ),
            ["b", "c"]
        );
        // Two sections of the answer hold the entry points of the saved one.
        assert_eq!(
            kept(&[("x", &["E1"]), ("y", &["E1"])], &[("a", &["E1"])]),
            ["x", "y"]
        );
        // Two saved sections hold the same entry points.
        assert_eq!(
            kept(&[("x", &["E1"])], &[("a", &["E1"]), ("b", &["E1"])]),
            ["x"]
        );
        // Another section outside the answer holds the slug.
        let mut produced = vec![use_case("x", &["E1"])];
        keep_saved_slugs(&mut produced, &[use_case("a", &["E1"])], |slug| slug == "a");
        assert_eq!(slugs(&produced), ["x"]);
    }
}
