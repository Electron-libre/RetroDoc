//! The settling of a disputed section (`specs/section-settle.md`, ADR 0057): the first message
//! built from the docs, the system prompt of verification, and `propose_verdict`, the tool that
//! ends a question on a proposed review and correction, written nowhere.

use std::fmt::Write as _;

use retrodoc_docs::reviews::{Section, SectionKind, Status};
use retrodoc_docs::Docs;
use retrodoc_llm::ToolSpec;
use serde_json::{json, Value};

/// The name of the tool that proposes a verdict.
pub const PROPOSE_VERDICT: &str = "propose_verdict";

/// The paragraph the system prompt of a settling adds to the prompt of `ask` (STL-5).
pub(crate) const SETTLING_PROMPT: &str = "\n\nThe reader doubts a section of the documentation and \
asks you to help settle it. Check each claim of the section against the code (read_source), its \
history (git_log) and the rest of the docs, and against what the project's own docs state. Then say \
which claims you confirmed, which are wrong, and what you could not settle. When something only the \
reader can tell is left, ask them the few questions that would settle it, numbered, each with the \
answers you see, and stop there: they answer in the next message. Once the section is settled, call \
propose_verdict with the status, a comment giving your evidence (files, commits, the reader's \
answers) and the corrected text of each field that is wrong; leave out a field that is right.";

/// A doc section of the project that states something else than the section (REC-5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disagreement {
    /// `path#heading` of the doc section, or its path alone.
    pub doc_section: String,
    /// What the doc section states.
    pub doc_states: String,
    /// What the generated section states.
    pub section_states: String,
}

/// The section a settling is about, and the project's docs that contradict it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disputed {
    pub section: Section,
    pub disagreements: Vec<Disagreement>,
}

/// The verdict the agent proposes for the settled section: nothing is saved until the reader saves
/// the form it fills.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposal {
    pub section: Section,
    pub status: Status,
    pub comment: String,
    pub name: Option<String>,
    pub description: Option<String>,
    /// Only for a use case.
    pub narrative: Option<String>,
}

impl Proposal {
    /// The proposal as the kept answer holds it (STL-15).
    #[must_use]
    pub fn as_text(&self) -> String {
        let mut text = format!(
            "Proposed verdict: {}\nComment: {}",
            status_name(self.status),
            self.comment
        );
        for (label, field) in [
            ("Name", &self.name),
            ("Description", &self.description),
            ("Narrative", &self.narrative),
        ] {
            if let Some(value) = field {
                let _ = write!(text, "\n{label}: {value}");
            }
        }
        text
    }
}

fn status_name(status: Status) -> &'static str {
    match status {
        Status::Validated => "validated",
        Status::Wrong => "wrong",
        Status::ToReview => "to_review",
    }
}

fn kind_name(kind: SectionKind) -> &'static str {
    match kind {
        SectionKind::Domain => "domain",
        SectionKind::Feature => "feature",
        SectionKind::UseCase => "use case",
    }
}

impl Disputed {
    /// The first question of the settling (STL-4): the section as its detail tool answers it,
    /// then the contradictions of the project's docs. Reads the reviews and edits: call it off the
    /// async threads.
    #[must_use]
    pub fn message(&self, docs: &Docs) -> String {
        let Section { kind, id } = &self.section;
        let detail = match kind {
            SectionKind::Domain => docs.get_domain(id),
            SectionKind::Feature => docs.get_feature(id),
            SectionKind::UseCase => docs.get_use_case(id),
        };
        let mut message = format!(
            "Help me settle the {} `{id}`, which I doubt. Its documentation as I read it:\n\n{}\n\n",
            kind_name(*kind),
            detail.trim_end()
        );
        if self.disagreements.is_empty() {
            message.push_str("The project's docs contradict none of it.");
        } else {
            message.push_str("The project's docs contradict it:\n");
            for d in &self.disagreements {
                let _ = write!(
                    message,
                    "\n- `{}` states: {}\n  The section states: {}",
                    d.doc_section, d.doc_states, d.section_states
                );
            }
        }
        message
    }
}

/// The tool of a settling question (STL-6), its arguments as STL-13 lists them.
pub(crate) fn propose_verdict_spec() -> ToolSpec {
    ToolSpec {
        name: PROPOSE_VERDICT.to_string(),
        description: "Propose the verdict on the section being settled, once it is settled: its \
            review status, a comment with the evidence, and the corrected text of each wrong \
            field. Saves nothing: the reader checks it and saves it."
            .to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "status": {"type": "string", "enum": ["validated", "wrong", "to_review"]},
                "comment": {"type": "string", "description": "Why, with the evidence."},
                "name": {"type": "string", "description": "The corrected name, if wrong."},
                "description": {"type": "string", "description": "The corrected description, if wrong."},
                "narrative": {"type": "string", "description": "The corrected narrative of a use case, if wrong."}
            },
            "required": ["status", "comment"]
        }),
    }
}

/// The proposal a call of `propose_verdict` makes for `section`, or the error its result gives the
/// model (STL-13).
pub(crate) fn proposal_of(section: &Section, arguments: &str) -> Result<Proposal, String> {
    let arguments: Value = serde_json::from_str(arguments)
        .map_err(|err| format!("Error: the arguments are not a JSON object: {err}."))?;
    let text = |key: &str| {
        arguments
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    };
    let status = match text("status").as_deref() {
        Some("validated") => Status::Validated,
        Some("wrong") => Status::Wrong,
        Some("to_review") => Status::ToReview,
        _ => {
            return Err(
                "Error: `status` must be one of `validated`, `wrong` and `to_review`.".to_string(),
            )
        }
    };
    let comment = text("comment")
        .ok_or_else(|| "Error: `comment` is required: give the evidence.".to_string())?;
    let narrative = text("narrative");
    if narrative.is_some() && section.kind != SectionKind::UseCase {
        return Err(format!(
            "Error: a {} has no narrative; propose its name or description only.",
            kind_name(section.kind)
        ));
    }
    Ok(Proposal {
        section: section.clone(),
        status,
        comment,
        name: text("name"),
        description: text("description"),
        narrative,
    })
}

#[cfg(test)]
mod tests;
