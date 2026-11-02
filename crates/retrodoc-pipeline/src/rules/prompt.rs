//! The prompt of the rules of one feature: its entities, its use cases, the evidence with a short id
//! each (so a rule cites `T1` or `D2`, never a path it could invent) and the numbered code.

use std::fmt::Write as _;
use std::path::Path;

use retrodoc_core::model::{Feature, UseCase};

use super::grounding::RawRules;
use super::RuleInput;
use crate::brief::{ExtractKind, ProductBrief};
use crate::chunks::Splitter;
use crate::request::Request;
use crate::text::{feature_line, names_line, one_line, FEATURE_LABEL};
use crate::use_cases::{focus_for, MAX_CHARS_PER_FILE, MAX_CHARS_PER_PROMPT};
use crate::walked::WalkedFiles;

pub(super) const RULES_SYSTEM_PROMPT: &str = "You are documenting a software project from a \
functional point of view. Given a feature and the source code implementing it, state the business \
rules the code enforces: what is allowed or refused, who may do what, which limits apply, which \
states an object can move through and when, how an amount, a choice or a date is worked out. A \
business reader must be able to ask about each of them (\"can a signed contract be edited?\"). \
Write each rule as one sentence in the application's own vocabulary, without class, method or \
file names. Leave out technical constraints without business meaning (field lengths, required \
fields, null handling, framework errors) and anything the code does not show. A rule may be a \
refusal returned as a value as well as a raised error, or a constant that a check uses. Cite the \
code it comes from with the number of the file shown before its excerpt and the line range of the \
numbered lines, as \"F2:10-24\". When a test \
description or a documentation extract of the prompt states the same rule, cite it by its id and \
copy the words of the test that state it. An empty list is a valid answer. Reply with ONLY a single \
JSON object, no prose and no Markdown code fence, matching this shape: {\"rules\":[{\"statement\":\
\"...\",\"kind\":\"validation|permission|state_transition|limit|computation\",\
\"use_cases\":[\"use-case-slug\"],\"code\":[\"F1:10-24\"],\
\"tests\":[{\"id\":\"T1\",\"phrase\":\"...\"}],\"docs\":[\"D1\"]}]}. `use_cases` is copied \
from the list of the prompt and may be empty.";

const ENTITIES_LABEL: &str = "\nEntities: ";
const USE_CASES_HEADER: &str = "\nUse cases:\n";
const TESTS_HEADER: &str = "\nTest descriptions (they may be partly unrelated):\n";
const DOCS_HEADER: &str = "\nDocumentation extracts (they may be partly unrelated):\n";
const SOURCE_FILES_HEADER: &str = "\nSource files:\n";

/// The request of the statement of the rules of a feature.
pub(super) static STATEMENT: Request<RawRules> = Request::new(
    RULES_SYSTEM_PROMPT,
    &[
        FEATURE_LABEL,
        ENTITIES_LABEL,
        USE_CASES_HEADER,
        TESTS_HEADER,
        DOCS_HEADER,
        SOURCE_FILES_HEADER,
    ],
);

/// Id of the `n`th (from 0) test extract or documentation extract.
pub(super) fn evidence_id(kind: ExtractKind, n: usize) -> String {
    let letter = match kind {
        ExtractKind::Test => 'T',
        ExtractKind::Doc => 'D',
    };
    format!("{letter}{}", n + 1)
}

/// The prompt for `feature`, with the files whose code it shows, in the order of their numbers
/// (`F1`, …): the only ones a rule can cite.
pub(super) fn rules_prompt(
    walked: &WalkedFiles,
    feature: &Feature,
    input: &RuleInput,
    use_cases: &[&UseCase],
    brief: &ProductBrief,
    splitter: &Splitter,
) -> (String, Vec<String>) {
    let mut prompt = format!("{}{}", brief.prompt_head(), feature_line(feature));
    if !input.entities.is_empty() {
        let _ = writeln!(prompt, "{ENTITIES_LABEL}{}", names_line(&input.entities));
    }
    if !use_cases.is_empty() {
        prompt.push_str(USE_CASES_HEADER);
        for use_case in use_cases {
            let _ = writeln!(prompt, "- {}: {}", use_case.slug, one_line(&use_case.name));
        }
    }
    for kind in [ExtractKind::Test, ExtractKind::Doc] {
        let extracts: Vec<&str> = input
            .evidence
            .iter()
            .filter(|e| e.kind == kind)
            .map(|e| e.text.as_str())
            .collect();
        if extracts.is_empty() {
            continue;
        }
        prompt.push_str(match kind {
            ExtractKind::Test => TESTS_HEADER,
            ExtractKind::Doc => DOCS_HEADER,
        });
        for (n, text) in extracts.iter().enumerate() {
            let _ = writeln!(prompt, "[{}] {text}", evidence_id(kind, n));
        }
    }
    prompt.push_str(SOURCE_FILES_HEADER);
    let focus = focus_for(feature, &input.entries);
    let mut included: Vec<String> = Vec::new();
    let mut budget = MAX_CHARS_PER_PROMPT;
    for path in &input.files {
        if budget == 0 {
            break;
        }
        let content = match walked.read(Path::new(path)) {
            Ok(content) => content,
            Err(err) => {
                tracing::warn!(path = %path, error = %err, "could not read a feature file, left out of the rules prompt");
                continue;
            }
        };
        let excerpt = splitter.excerpt(
            Path::new(path),
            &content,
            MAX_CHARS_PER_FILE.min(budget),
            &focus,
        );
        budget = budget.saturating_sub(excerpt.len());
        included.push(path.clone());
        let _ = write!(
            prompt,
            "\n=== [F{}] {path} ===\n{excerpt}\n",
            included.len()
        );
    }
    (prompt, included)
}
