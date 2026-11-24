//! The human corrections of the docs a pass is given (`specs/human-corrections.md`, ADR 0056): the
//! edits and the reviews of the sections a unit writes, shown in its request by slug. The command
//! loads them from `.retrodoc/edits.json` and `.retrodoc/reviews.json`; the pipeline never reads
//! nor writes those files.
//!
//! Only a review `wrong` asks its unit again, once: the reviews a unit answered are kept in
//! `fingerprints.json` ([`Fingerprints::answered`]), apart from its print, which leaves the
//! corrections out. An edit or a review `validated` reaches the request the next time the unit is
//! asked for another reason. The fixed text of the corrections is out of the prints too
//! (HCR-12): a change of it needs `--force`.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use retrodoc_core::cache::hash_content;
use retrodoc_core::text::one_line;

use crate::fingerprints::Fingerprints;

/// What a human said of a section in a review that guides the run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// The section is wrong: the comment says what to fix (empty when none was given).
    Wrong {
        comment: String,
        reviewed_at: String,
    },
    /// The section is right as the docs show it.
    Validated,
}

/// The corrections of one section.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Correction {
    /// The slug of the section among those of its unit (a use case's own slug, not
    /// `<feature>/<use case>`).
    pub slug: String,
    /// The fields of its edit, a field the edit leaves out or blank absent.
    pub name: Option<String>,
    pub description: Option<String>,
    pub narrative: Option<String>,
    pub verdict: Option<Verdict>,
    /// The name and the description the docs show, the edit applied: what a review `validated`
    /// vouched for.
    pub shown_name: String,
    pub shown_description: String,
}

impl Correction {
    /// Whether it says anything to a pass.
    fn is_empty(&self) -> bool {
        self.verdict.is_none()
            && self.name.is_none()
            && self.description.is_none()
            && self.narrative.is_none()
    }

    /// The key a review `wrong` is answered under: its section, its date and its comment.
    fn wrong_key(&self) -> Option<String> {
        match &self.verdict {
            Some(Verdict::Wrong {
                comment,
                reviewed_at,
            }) => Some(format!(
                "{}@{reviewed_at}#{}",
                self.slug,
                hash_content(comment)
            )),
            _ => None,
        }
    }
}

/// The corrections of a run, grouped by the unit that writes their sections.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Corrections {
    /// The features, by the slug of their domain.
    features: BTreeMap<String, Vec<Correction>>,
    /// The use cases, by the slug of their feature.
    use_cases: BTreeMap<String, Vec<Correction>>,
}

impl Corrections {
    /// Adds the correction of a feature of the domain `domain`; one that says nothing is left out.
    pub fn add_feature(&mut self, domain: &str, correction: Correction) {
        if !correction.is_empty() {
            self.features
                .entry(domain.to_string())
                .or_default()
                .push(correction);
        }
    }

    /// The corrections of the features of the domain `domain` (HCR-6, HCR-7).
    pub(crate) fn features_of(&self, domain: &str) -> &[Correction] {
        self.features.get(domain).map_or(&[], Vec::as_slice)
    }

    /// Adds the correction of a use case of the feature `feature`; one that says nothing is left out.
    pub fn add_use_case(&mut self, feature: &str, correction: Correction) {
        if !correction.is_empty() {
            self.use_cases
                .entry(feature.to_string())
                .or_default()
                .push(correction);
        }
    }

    /// The corrections of the use cases of the feature `feature` (HCR-6, HCR-7).
    pub(crate) fn use_cases_of(&self, feature: &str) -> &[Correction] {
        self.use_cases.get(feature).map_or(&[], Vec::as_slice)
    }
}

/// The keys of the reviews `wrong` of `corrections`.
pub(crate) fn wrong_keys(corrections: &[Correction]) -> Vec<String> {
    corrections
        .iter()
        .filter_map(Correction::wrong_key)
        .collect()
}

/// The key of the unit `key` of the pass `pass` in [`Fingerprints::answered`].
pub(crate) fn answered_key(pass: &str, key: &str) -> String {
    format!("{pass}:{key}")
}

/// Whether a review `wrong` of `wrong` was not answered by the unit `key` yet (HCR-4, HCR-10).
pub(crate) fn pending(prints: &Fingerprints, key: &str, wrong: &[String]) -> bool {
    let answered = prints.answered.get(key);
    wrong
        .iter()
        .any(|review| answered.is_none_or(|answered| !answered.contains(review)))
}

/// Records that the unit `key` was answered with the reviews `wrong` before it.
pub(crate) fn record_answered(prints: &mut Fingerprints, key: String, wrong: Vec<String>) {
    if wrong.is_empty() {
        prints.answered.remove(&key);
    } else {
        prints.answered.insert(key, wrong);
    }
}

const HEADER: &str = "\nHuman corrections of the last docs: a human checked these {kind}s. Give \
each one back with the same slug, and follow what is said of it.\n";
const EDITED: &str = " edited by a human; keep this text unless the code contradicts it:";
const WRONG: &str = " marked wrong by a human; fix it:";
const WRONG_NO_COMMENT: &str = " marked wrong by a human; write it again from the code.";
const VALIDATED: &str = " validated by a human; keep it as it is:";

/// The part of a request that shows `corrections` of sections of the kind `kind` (`"use case"`),
/// none when there are none (HCR-1, HCR-2, HCR-3, HCR-8).
pub(crate) fn prompt_part(kind: &str, corrections: &[Correction]) -> Option<String> {
    if corrections.is_empty() {
        return None;
    }
    let mut part = HEADER.replace("{kind}", kind);
    for correction in corrections {
        let _ = write!(part, "- {kind} `{}`:", correction.slug);
        let edited: Vec<String> = [
            ("name", &correction.name),
            ("description", &correction.description),
            ("narrative", &correction.narrative),
        ]
        .into_iter()
        .filter_map(|(field, text)| {
            text.as_deref()
                .map(|text| format!(" {field} \"{}\"", one_line(text)))
        })
        .collect();
        if !edited.is_empty() {
            let _ = write!(part, "{EDITED}{};", edited.join(";"));
        }
        match &correction.verdict {
            Some(Verdict::Wrong { comment, .. }) if comment.trim().is_empty() => {
                part.push_str(WRONG_NO_COMMENT);
            }
            Some(Verdict::Wrong { comment, .. }) => {
                let _ = write!(part, "{WRONG} \"{}\".", one_line(comment));
            }
            Some(Verdict::Validated) => {
                let _ = write!(
                    part,
                    "{VALIDATED} name \"{}\"; description \"{}\".",
                    one_line(&correction.shown_name),
                    one_line(&correction.shown_description)
                );
            }
            None => {}
        }
        part.push('\n');
    }
    Some(part)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wrong(slug: &str, comment: &str, at: &str) -> Correction {
        Correction {
            slug: slug.to_string(),
            verdict: Some(Verdict::Wrong {
                comment: comment.to_string(),
                reviewed_at: at.to_string(),
            }),
            ..Correction::default()
        }
    }

    // spec: HCR-7 (internal: the application of the binary tests has a single feature)
    #[test]
    fn a_feature_is_given_the_corrections_of_its_own_use_cases_only() {
        let mut corrections = Corrections::default();
        corrections.add_use_case("pay", wrong("pay-online", "Cash only.", "t"));
        corrections.add_use_case("ship", wrong("ship-order", "By post.", "t"));
        corrections.add_use_case("ship", Correction::default());

        let shown: Vec<&str> = corrections
            .use_cases_of("ship")
            .iter()
            .map(|c| c.slug.as_str())
            .collect();
        assert_eq!(shown, ["ship-order"]);
        assert_eq!(corrections.use_cases_of("list"), []);
    }

    // spec: HCR-10 (internal: the record of fingerprints.json the binary tests see only through the calls)
    #[test]
    fn a_review_is_answered_by_its_section_date_and_comment() {
        let mut prints = Fingerprints::default();
        let first = wrong_keys(&[wrong("a", "Cash only.", "t1")]);
        assert!(pending(&prints, "use cases:d/f", &first));
        record_answered(&mut prints, "use cases:d/f".to_string(), first.clone());
        assert!(!pending(&prints, "use cases:d/f", &first));
        for other in [
            wrong("a", "By card.", "t1"),
            wrong("a", "Cash only.", "t2"),
            wrong("b", "Cash only.", "t1"),
        ] {
            assert!(pending(&prints, "use cases:d/f", &wrong_keys(&[other])));
        }
        // A review removed asks nothing.
        assert!(!pending(&prints, "use cases:d/f", &[]));
    }

    #[test]
    fn no_correction_adds_nothing_to_a_request() {
        assert_eq!(prompt_part("use case", &[]), None);
    }
}
