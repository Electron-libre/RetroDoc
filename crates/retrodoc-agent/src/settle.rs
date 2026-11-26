//! The settling of a disputed section (`specs/section-settle.md`, ADR 0057): the first message
//! built from the docs, the system prompt of verification, and `propose_verdict`, the tool that
//! ends a question on a proposed review and correction, written nowhere.

use std::fmt::Write as _;

use retrodoc_docs::reviews::{Section, SectionKind, Status};
use retrodoc_docs::Docs;
use retrodoc_llm::{ToolCall, ToolSpec};
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
propose_verdict with the status, a comment and the evidence apart, and the corrected text of each \
field that is wrong; leave out a field that is right. Write the comment for a reader who is not a \
developer: a few sentences in the words of the business, what is right or wrong in the section and \
why it matters to them, with no file, commit or code name; put those in the evidence, with the \
reader's answers. The status judges the section as the reader will save it, with your corrections: \
validated when it is right as it stands or once corrected (then give the corrected text), wrong when \
a claim of it is wrong that you cannot correct (the next run writes it again; give the corrections \
you can), to_review when it could not be settled.";

/// The longest comment of a proposal, in characters: the evidence goes apart (STL-13).
pub const MAX_COMMENT_CHARS: usize = 400;

/// The demand that ends the last request of a settling question, which offers `propose_verdict`
/// alone (STL-18).
pub(crate) const SETTLING_LAST_ROUND: &str = "You have used all your rounds of tool calls: \
propose_verdict is the only tool left. If the section is settled, call propose_verdict now, \
correcting the call if an earlier one was refused; otherwise say what is left to settle and ask \
the reader your questions.";

/// The sentence of the settling prompt on the rounds of a question, `{rounds}` their number.
const ROUNDS_LEFT: &str = "You have {rounds} of tool calls for each message of the reader: spend \
them on what settles the section, and call propose_verdict, or ask your questions, before they \
run out; after the last round only propose_verdict is left.";

/// The settling system prompt for `section`, whose questions have `max_rounds` rounds of tool calls
/// (STL-5): it names the section, which the later turns of the conversation, trimmed to its memory,
/// may no longer hold.
pub(crate) fn settling_prompt(section: &Section, max_rounds: usize) -> String {
    let tool = match section.kind {
        SectionKind::Domain => "get_domain",
        SectionKind::Feature => "get_feature",
        SectionKind::UseCase => "get_use_case",
    };
    let rounds = match max_rounds {
        0 => "You have no round of tool calls: only propose_verdict is offered.".to_string(),
        1 => ROUNDS_LEFT.replace("{rounds}", "1 round"),
        n => ROUNDS_LEFT.replace("{rounds}", &format!("{n} rounds")),
    };
    format!(
        "{SETTLING_PROMPT} {rounds} The section being settled is the {} `{}`: read it again with \
         {tool} when the conversation no longer shows it.",
        kind_name(section.kind),
        section.id
    )
}

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
    /// The reason of the verdict, for the reader.
    pub comment: String,
    /// What the verdict rests on: files, commits, the reader's answers.
    pub evidence: String,
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
            "Proposed verdict: {}\nComment: {}\nEvidence: {}",
            status_name(self.status),
            self.comment,
            self.evidence
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

/// The tool of a settling question of `section` (STL-6), its arguments as STL-13 lists them:
/// `narrative` for a use case only.
pub(crate) fn propose_verdict_spec(section: &Section) -> ToolSpec {
    let mut parameters = json!({
        "type": "object",
        "properties": {
            "status": {
                "type": "string",
                "enum": ["validated", "wrong", "to_review"],
                "description": "validated: the section is right as it stands, or once saved with \
                    the corrected fields given; wrong: a claim of it is wrong and left uncorrected, \
                    with the corrected fields given if any, so the next run writes it again; to_review: it could not be settled."
            },
            "comment": {
                "type": "string",
                "maxLength": MAX_COMMENT_CHARS,
                "description": format!("Why, in a few sentences in the words of the business, \
                    at most {MAX_COMMENT_CHARS} characters: no file, commit or code name.")
            },
            "evidence": {
                "type": "string",
                "description": "What the verdict rests on: files, commits, the reader's answers."
            },
            "name": {"type": "string", "description": "The corrected name, if it is wrong."},
            "description": {"type": "string", "description": "The corrected description, if it is wrong."}
        },
        "required": ["status", "comment", "evidence"]
    });
    if section.kind == SectionKind::UseCase {
        parameters["properties"]["narrative"] =
            json!({"type": "string", "description": "The corrected narrative, if it is wrong."});
    }
    ToolSpec {
        name: PROPOSE_VERDICT.to_string(),
        description: "Propose the verdict on the section being settled, once it is settled: its \
            review status, a short comment for a reader who is not a developer, the evidence \
            apart, and the corrected text of each wrong field. Saves nothing: the reader checks it \
            and saves it."
            .to_string(),
        parameters,
    }
}

/// A call of `propose_verdict` the table of STL-13 refused, as the log of `ui` names it (STL-20).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// The `status` the call gives, if a string.
    pub status: Option<String>,
    /// The length in characters of the `comment` it gives, 0 for none.
    pub comment_chars: usize,
    /// The error its result gave the model.
    pub error: String,
}

impl Refusal {
    /// The refusal of the call whose `arguments` got `error`.
    pub(crate) fn of(arguments: &str, error: String) -> Self {
        let arguments = serde_json::from_str::<Value>(arguments).ok();
        let text = |key: &str| arguments.as_ref()?.get(key)?.as_str().map(str::to_string);
        Self {
            status: text("status"),
            comment_chars: text("comment").map_or(0, |comment| comment.chars().count()),
            error,
        }
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
    let comment = text("comment").ok_or_else(|| {
        "Error: `comment` is required: give the reason of the verdict.".to_string()
    })?;
    let length = comment.chars().count();
    if length > MAX_COMMENT_CHARS {
        return Err(format!(
            "Error: `comment` is longer than {MAX_COMMENT_CHARS} characters ({length} characters): \
             say the verdict in a few sentences for the reader and put the files, commits and \
             answers in `evidence`."
        ));
    }
    let evidence = text("evidence").ok_or_else(|| {
        "Error: `evidence` is required: the files, commits and answers the verdict rests on."
            .to_string()
    })?;
    let (name, description, narrative) = (text("name"), text("description"), text("narrative"));
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
        evidence,
        name,
        description,
        narrative,
    })
}

/// What the last request of a settling question ended on (STL-18).
pub(crate) enum LastAnswer {
    /// A proposal the table accepts, with the text of the message that made it and the index of
    /// its call.
    Proposed {
        proposal: Proposal,
        text: String,
        call: usize,
    },
    /// No proposal: the text of the message.
    Text(String),
}

/// What the last request of the settling of `section` ended on, its message holding `calls` and
/// `text`: the proposal of its first call of `propose_verdict` the table accepts, no round
/// following to correct a refused one; else the text, blank replaced by the comment of the first
/// refused call, else by its error. Each call refused before the accepted one is pushed to
/// `refusals` (STL-20).
pub(crate) fn last_answer(
    section: &Section,
    calls: &[ToolCall],
    text: String,
    refusals: &mut Vec<Refusal>,
) -> LastAnswer {
    let mut refused = None;
    for (index, call) in calls.iter().enumerate() {
        if call.name != PROPOSE_VERDICT {
            continue;
        }
        match proposal_of(section, &call.arguments) {
            Ok(proposal) => {
                return LastAnswer::Proposed {
                    proposal,
                    text,
                    call: index,
                }
            }
            Err(error) => {
                refusals.push(Refusal::of(&call.arguments, error.clone()));
                refused.get_or_insert((call, error));
            }
        }
    }
    match refused {
        Some((call, error)) if text.trim().is_empty() => {
            let comment = serde_json::from_str::<Value>(&call.arguments)
                .ok()
                .and_then(|arguments| Some(arguments.get("comment")?.as_str()?.trim().to_string()))
                .filter(|comment| !comment.is_empty());
            LastAnswer::Text(comment.unwrap_or(error))
        }
        _ => LastAnswer::Text(text),
    }
}

#[cfg(test)]
mod tests;
