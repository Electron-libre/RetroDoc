//! The human edit of a section on its page (`specs/web-ui.md` WEB-91, WEB-92): the fields it
//! changes, its author and its date, why it is outdated when it is, and the text the last run
//! generated for each of those fields, folded unless it is outdated. The edits are those the docs
//! show, kept current by `retrodoc ui` (`retrodoc_mcp::edits::Live`).

use retrodoc_mcp::edits::{self, Field};
use retrodoc_mcp::GeneratedDocs;

use crate::review::{OutdatedView, Place};
use crate::texts::{EditTexts, WebTexts};

/// The edit of a section as its page shows it.
#[derive(Debug)]
pub struct EditView {
    /// The fields it changes, named in the language of the page, joined.
    pub fields: String,
    pub author: Option<String>,
    pub date: String,
    pub outdated: Option<OutdatedView>,
    /// The text the last run generated for each field it changes, folded on the page unless the
    /// edit is outdated.
    pub generated: Vec<GeneratedField>,
}

/// A field as the last run generated it.
#[derive(Debug)]
pub struct GeneratedField {
    pub label: &'static str,
    pub text: String,
}

impl EditView {
    /// The edit of the section at `place` of `docs`, `None` when it has none.
    #[must_use]
    pub fn of(docs: &GeneratedDocs, place: &Place, t: &WebTexts) -> Option<Self> {
        let edit = docs.edits().get(&place.section())?;
        let changed = edit.changed();
        if changed.is_empty() {
            return None;
        }
        let outdated = edits::outdated(docs, edit);
        let generated = edits::generated(docs, edit).unwrap_or_default();
        Some(Self {
            fields: changed
                .into_iter()
                .map(|field| field_label(&t.edit, field))
                .collect::<Vec<_>>()
                .join(", "),
            author: edit
                .author
                .as_deref()
                .map(str::trim)
                .filter(|author| !author.is_empty())
                .map(str::to_string),
            date: edit.edited_at.clone(),
            outdated: outdated.as_ref().map(OutdatedView::of_edit),
            generated: generated
                .into_iter()
                .map(|(field, text)| GeneratedField {
                    label: field_label(&t.edit, field),
                    text,
                })
                .collect(),
        })
    }
}

/// Whether the section at `place` of `docs` shows a human edit: one that changes a field as the
/// readers show it ([`retrodoc_mcp::edits::Edit::changed`]), the mark of its row (WEB-100).
#[must_use]
pub fn is_edited(docs: &GeneratedDocs, place: &Place) -> bool {
    docs.edits()
        .get(&place.section())
        .is_some_and(|edit| !edit.changed().is_empty())
}

fn field_label(texts: &EditTexts, field: Field) -> &'static str {
    Field::ALL
        .iter()
        .position(|f| *f == field)
        .map_or("", |at| texts.fields[at])
}
