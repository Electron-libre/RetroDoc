use super::*;

/// Per-file truncation and overall budget of code sent for one feature
/// (PLAN.md §6 "cost/volume"); files beyond the budget are left out of the
/// prompt, and so can't be cited.
pub(super) const MAX_CHARS_PER_FILE: usize = 4_000;

/// Characters of code in one request; the cited files share it.
pub(super) const MAX_CHARS_PER_PROMPT: usize = 30_000;

/// Added to the system prompt when the feature has known entry points.
pub(super) const ENTRY_POINTS_ADDENDUM: &str =
    " The prompt lists the feature's entry points with their \
observable outputs, then the files that define them and the files those reference. Build each use \
case around one entry point, or a few closely related ones: the actor's goal, then the steps from \
the trigger to the observable outputs, grounded on that code. Add to each use case an \
`entry_points` array with the names of the entry points it covers, copied verbatim from the list. \
Prefer business wording (what happens to the contract, the company, the user) over method names.";

/// Added to the system prompt when the application's business actors are
/// known: they are listed in the prompt and must be used by name.
pub(super) const ACTORS_ADDENDUM: &str =
    " The prompt lists the application's known actors. Name each human \
actor with one of them, copied verbatim, choosing the one that fits the step. Name a software \
actor after the real component or external system involved (e.g. an e-signature provider, a mail \
service), and use \"System\" only for the application itself. Give each use case a \
`primary_actor`: the known human actor who triggers it, and begin its steps with the step in \
which that actor acts (submits the request, opens the page, confirms).";

/// Asks for the business-level account of each use case, next to its
/// technical steps (the two output levels).
pub(super) const NARRATIVE_ADDENDUM: &str =
    " Also give each use case a `narrative`: two to four sentences \
for a reader who does not know the code, saying who does what and why, which business objects are \
created or changed, and what the observable result is. Use the application's business vocabulary \
(its entities and actors) and do not mention classes, methods, files or HTTP details; those belong \
in the steps.";

pub(super) const USE_CASES_SYSTEM_PROMPT: &str =
    "You are documenting a software project from a functional \
point of view. Given a feature and the source code implementing it, describe its use cases: \
concrete scenarios in which an actor achieves a goal with this feature. For each use case give \
the ordered steps, each with its actor (kind \"human\" for a person, \"system\" for a software \
component, service or external system), the action performed, and the file (with line range when \
you can tell) the step is grounded on. Only describe behavior visible in the code provided; do not \
invent steps. Reply with ONLY a single JSON object, no prose and no Markdown code fence, matching \
this shape: {\"use_cases\":[{\"slug\":\"kebab-case\",\"name\":\"...\",\"description\":\"...\",\
\"steps\":[{\"description\":\"...\",\"actor\":{\"name\":\"...\",\"kind\":\"human|system\"},\
\"action\":\"short verb phrase\",\"source_refs\":[{\"path\":\"...\",\"start_line\":1,\
\"end_line\":10}]}]}]}.";

/// The system prompt: the base, the narrative request, and the parts that
/// only apply when the feature has entry points / the actors are known.
pub(super) fn system_prompt(has_entry_points: bool, has_actors: bool) -> String {
    let mut prompt = format!("{USE_CASES_SYSTEM_PROMPT}{NARRATIVE_ADDENDUM}");
    if has_entry_points {
        prompt.push_str(ENTRY_POINTS_ADDENDUM);
    }
    if has_actors {
        prompt.push_str(ACTORS_ADDENDUM);
    }
    prompt
}

/// What frames the prompt of a feature besides the feature itself.
pub(super) struct Framing<'a> {
    pub actors: &'a Actors,
    pub vocabulary: &'a [String],
    pub brief: &'a ProductBrief,
    /// The extracts of the project closest to the feature (may be empty).
    pub evidence: &'a str,
}

/// Builds the user prompt for `feature` from the code of its files, within
/// the prompt budget. Returns it with the set of files actually included.
pub(super) fn use_cases_prompt(
    repo_root: &Path,
    feature: &Feature,
    input: &FeatureInput,
    framing: &Framing<'_>,
    splitter: &Splitter,
) -> (String, BTreeSet<String>) {
    let Framing {
        actors,
        vocabulary,
        brief,
        evidence,
    } = *framing;
    let focus = focus_for(feature, &input.entries);
    let mut prompt = format!(
        "{}Feature: {} — {}\n",
        brief.prompt_head(),
        feature.name,
        feature.description
    );
    if !evidence.is_empty() {
        let _ = write!(prompt, "\n{evidence}");
    }
    if !vocabulary.is_empty() {
        let _ = write!(
            prompt,
            "\nBusiness vocabulary (main entities): {}\n",
            vocabulary.join(", ")
        );
    }
    if !actors.is_empty() {
        let _ = write!(prompt, "\nKnown actors:\n{}", actors.prompt_section());
    }
    if !input.entries.is_empty() {
        prompt.push_str("\nEntry points:\n");
        for (file, entry) in &input.entries {
            let _ = write!(
                prompt,
                "- {} ({}): {}",
                entry.name,
                file.display(),
                entry.description
            );
            let outputs: Vec<String> = entry
                .outputs
                .iter()
                .map(|o| format!("{:?} {}", o.kind, o.description))
                .collect();
            if !outputs.is_empty() {
                let _ = write!(prompt, " → outputs: {}", outputs.join("; "));
            }
            prompt.push('\n');
        }
    }
    prompt.push_str("\nSource files:\n");
    let mut included = BTreeSet::new();
    let mut budget = MAX_CHARS_PER_PROMPT;

    for path in &input.files {
        if budget == 0 {
            break;
        }
        let Ok(bytes) = std::fs::read(repo_root.join(path)) else {
            tracing::warn!(path = %path, "could not read a feature file, left out of the prompt");
            continue;
        };
        let content = String::from_utf8_lossy(&bytes);
        let excerpt = splitter.excerpt(
            Path::new(path),
            &content,
            MAX_CHARS_PER_FILE.min(budget),
            &focus,
        );
        budget = budget.saturating_sub(excerpt.len());
        let _ = write!(prompt, "\n=== {path} ===\n{excerpt}\n");
        included.insert(path.clone());
    }
    (prompt, included)
}

/// What a long file's excerpt should favour for `feature`: the identifiers
/// its entry points are named after (`send_contract` in `POST
/// /contracts/:id/send_contract`), then the words of its own text.
pub(super) fn focus_for(feature: &Feature, entries: &[(PathBuf, EntryPoint)]) -> Focus {
    /// Too common to point at any code.
    const NOISE: &[&str] = &["post", "patch", "delete", "head", "implied", "callback"];
    let tokens = |text: &str, min_len: usize| -> Vec<String> {
        text.split(|c: char| !(c.is_alphanumeric() || c == '_'))
            .filter(|w| w.len() >= min_len)
            .map(str::to_lowercase)
            .filter(|w| !NOISE.contains(&w.as_str()))
            .collect()
    };
    let mut terms: Vec<String> = Vec::new();
    for (_, entry) in entries {
        for term in tokens(&entry.name, 4)
            .into_iter()
            .chain(tokens(&entry.verb, 4))
        {
            if !terms.contains(&term) {
                terms.push(term);
            }
        }
    }
    let mut words: Vec<String> = Vec::new();
    for word in tokens(&format!("{} {}", feature.name, feature.description), 5) {
        if !terms.contains(&word) && !words.contains(&word) {
            words.push(word);
        }
    }
    Focus {
        terms,
        words,
        lines: Vec::new(),
    }
}

/// Prefixes each line with its 1-based number (so the LLM can cite line
/// ranges) and stops once `max_chars` are reached.
pub(crate) fn numbered_excerpt(content: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (n, line) in content.lines().enumerate() {
        if out.len() >= max_chars {
            out.push_str("… (truncated)\n");
            break;
        }
        let _ = writeln!(out, "{:>4} | {line}", n + 1);
    }
    out
}
