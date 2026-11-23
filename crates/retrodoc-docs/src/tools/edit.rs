//! What the detail answers say of the human edit of their section (`specs/section-edit.md`): that
//! the text is a human edit and, when the edit is outdated, why and what the last run generated in
//! its place. The edits are those the docs show, kept current by the server ([`crate::edits::Live`]).

use std::fmt::Write as _;

use retrodoc_core::text::one_line;

use super::Docs;
use crate::edits::{self, Edit, Field};
use crate::reviews::Section;

/// The edit of a section as an answer shows it; both parts empty with no edit.
#[derive(Default)]
pub(super) struct EditNote {
    /// `Edit: …` and its line break, on the line after the review line.
    pub line: String,
    /// The generated text of each field the edit changes, ending the answer of an outdated edit.
    pub generated: String,
}

impl Docs {
    /// The edit of `section`.
    pub(super) fn edit_note(&self, section: &Section) -> EditNote {
        let Some(edit) = self.generated.edits().get(section) else {
            return EditNote::default();
        };
        let changed = edit.changed();
        if changed.is_empty() {
            return EditNote::default();
        }
        let mut line = format!("Edit: human edit of {}", field_names(&changed));
        match edit
            .author
            .as_deref()
            .map(one_line)
            .filter(|a| !a.is_empty())
        {
            Some(author) => {
                let _ = write!(line, " ({author}, {})", one_line(&edit.edited_at));
            }
            None => {
                let _ = write!(line, " ({})", one_line(&edit.edited_at));
            }
        }
        let Some(outdated) = edits::outdated(&self.generated, edit) else {
            line.push('\n');
            return EditNote {
                line,
                generated: String::new(),
            };
        };
        let _ = writeln!(line, ", outdated: {}", outdated.edit_reasons());
        EditNote {
            line,
            generated: self.generated_text(edit),
        }
    }

    /// The generated text of each field `edit` changes, under a heading of its own.
    fn generated_text(&self, edit: &Edit) -> String {
        // The answer found the section: the docs hold it.
        let Some(fields) = edits::generated(&self.generated, edit) else {
            return String::new();
        };
        let mut out = String::from(
            "\n## Generated text\n\nThe edit is outdated: the last run generated, in place of the \
             edited fields:\n",
        );
        for (field, text) in fields {
            let key = field.key();
            let heading = format!("{}{}", key[..1].to_uppercase(), &key[1..]);
            let _ = write!(out, "\n### {heading}\n\n{}\n", text.trim_end());
        }
        out
    }
}

/// `the name`, `the name and the description`, `the name, the description and the narrative`.
fn field_names(fields: &[Field]) -> String {
    let names: Vec<String> = fields
        .iter()
        .map(|field| format!("the {}", field.key()))
        .collect();
    match names.as_slice() {
        [] => String::new(),
        [one] => one.clone(),
        [first @ .., last] => format!("{} and {last}", first.join(", ")),
    }
}
