//! The corpus of the reconciliation: the sections of the project's own docs (REC-1).

use retrodoc_ingest::existing_docs::ExistingDoc;
use retrodoc_ingest::signals::doc_sections;

use crate::reconcile::{Section, SectionRef};

/// The sections of `docs`, which must not hold the generated docs (the caller drops them). Sections
/// of one file under the same heading are one section: an origin names one text.
#[must_use]
pub fn corpus(docs: &[ExistingDoc]) -> Vec<Section> {
    let mut sections: Vec<Section> = Vec::new();
    for signal in doc_sections(docs) {
        let at = SectionRef::parse(&signal.origin);
        match sections.iter_mut().find(|s| s.at == at) {
            Some(same) => {
                same.text.push_str("\n\n");
                same.text.push_str(&signal.text);
            }
            None => sections.push(Section {
                at,
                text: signal.text,
            }),
        }
    }
    sections
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: REC-2 (internal: the pass needs an LLM through the binary; the corpus is its first step)
    #[test]
    fn two_sections_of_one_file_under_the_same_heading_are_one_section() {
        let docs = [ExistingDoc {
            path: "docs/guide.md".into(),
            content: "# Example\nPay by card.\n\n# Other\nNothing.\n\n# Example\nRefund by wire.\n"
                .into(),
        }];
        let sections = corpus(&docs);
        let examples: Vec<&Section> = sections
            .iter()
            .filter(|s| s.at.origin() == "docs/guide.md#Example")
            .collect();
        assert_eq!(examples.len(), 1);
        assert!(
            examples[0].text.contains("Pay by card."),
            "{}",
            examples[0].text
        );
        assert!(
            examples[0].text.contains("Refund by wire."),
            "{}",
            examples[0].text
        );
    }
}
