//! The mark of a human edit on the page of its section (`specs/section-edit.md` SED-38): the line
//! under the heading that says which fields a person rewrote, who and when, in the words of the web
//! page (WEB-91). The render never sees the edits themselves, only these marks: the caller lays the
//! edited text over the sections before.

use std::collections::BTreeMap;

use crate::doc_links::ItemRef;
use crate::texts::Texts;

/// A field of a section a human can edit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditedField {
    Name,
    Description,
    Narrative,
}

/// What the page of an edited section says of its edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditMark {
    /// The fields the edit changes, in the order the page names them; never empty.
    pub fields: Vec<EditedField>,
    pub author: Option<String>,
    /// The date of the edit, not of the render, so that the same edits render the same bytes.
    pub edited_at: String,
}

/// The mark of each edited domain, feature and use case.
pub type EditMarks = BTreeMap<ItemRef, EditMark>;

impl EditMark {
    /// The line under the heading: `**Human edit** name *by A, 2026-11-05T10:00:00Z*`.
    pub(crate) fn line(&self, texts: &Texts) -> String {
        let fields: Vec<&str> = self
            .fields
            .iter()
            .map(|field| texts.edited_field(*field))
            .collect();
        let by = self
            .author
            .as_deref()
            .map(str::trim)
            .filter(|author| !author.is_empty())
            .map(|author| format!("{} {author}, ", texts.edited_by))
            .unwrap_or_default();
        format!(
            "**{}** {} *{by}{}*",
            texts.human_edit,
            fields.join(", "),
            self.edited_at
        )
    }
}

/// The mark line of `item`, followed by a blank line, or nothing when it is not edited.
pub(crate) fn mark_of(marks: &EditMarks, item: &ItemRef, texts: &Texts) -> String {
    marks
        .get(item)
        .filter(|mark| !mark.fields.is_empty())
        .map(|mark| format!("{}\n\n", mark.line(texts)))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: SED-38 (internal: the CLI test renders in English; the French table is checked here)
    #[test]
    fn the_mark_names_the_fields_the_author_and_the_date_in_the_docs_language() {
        let mark = EditMark {
            fields: vec![EditedField::Name, EditedField::Narrative],
            author: Some("A".to_string()),
            edited_at: "2026-11-05T10:00:00Z".to_string(),
        };
        let marks = EditMarks::from([(ItemRef::Feature("pay".into()), mark)]);

        assert_eq!(
            mark_of(&marks, &ItemRef::Feature("pay".into()), &Texts::FRENCH),
            "**Correction humaine** nom, récit *par A, 2026-11-05T10:00:00Z*\n\n"
        );
        assert_eq!(
            mark_of(&marks, &ItemRef::Domain("pay".into()), &Texts::FRENCH),
            ""
        );
    }
}
