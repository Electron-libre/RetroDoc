//! Mechanical checks of one use case step against the code it cites, run
//! before any LLM call (`issues/deterministic_confidence_checks.md`). A step
//! that passes needs no verdict from the model; one that fails on a fact (the
//! cited code is not there) is unsupported without asking; the rest is
//! doubtful and goes to the model.

use std::path::Path;

use retrodoc_core::model::Step;

/// Names shorter than this are too common to prove anything.
const MIN_ENTITY_CHARS: usize = 3;

/// What the checks say about a step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum StepCheck {
    /// The cited code exists and shows what the step names.
    Pass,
    /// The step cites no code.
    Ungrounded,
    /// A name the step uses is not in the cited code: the model decides.
    Doubtful(String),
    /// A cited file can't be read or a cited line does not exist.
    Broken(String),
}

/// Checks `step` against the files and lines it cites: they exist, the
/// identifiers its text names appear at the cited lines, and the `entities`
/// it names appear in the cited files.
pub(super) fn check_step(repo_root: &Path, step: &Step, entities: &[String]) -> StepCheck {
    if step.source_refs.is_empty() {
        return StepCheck::Ungrounded;
    }
    let mut cited = String::new();
    let mut files = String::new();
    for reference in &step.source_refs {
        let Ok(bytes) = std::fs::read(repo_root.join(&reference.path)) else {
            return StepCheck::Broken(format!("cannot read {}", reference.path));
        };
        let text = String::from_utf8_lossy(&bytes);
        let lines: Vec<&str> = text.lines().collect();
        if let Some(start) = reference.start_line.or(reference.end_line) {
            let end = reference.end_line.unwrap_or(start).max(start);
            if start == 0 || start as usize > lines.len() {
                return StepCheck::Broken(format!("{} has no line {start}", reference.path));
            }
            for line in &lines[start as usize - 1..(end as usize).min(lines.len())] {
                cited.push_str(line);
                cited.push('\n');
            }
        } else {
            cited.push_str(&text);
        }
        files.push_str(&text);
    }
    let cited = cited.to_lowercase();
    let files = files.to_lowercase();
    let step_text = format!("{} {}", step.action, step.description);
    if let Some(missing) = identifiers(&step_text)
        .into_iter()
        .find(|id| !cited.contains(&id.to_lowercase()))
    {
        return StepCheck::Doubtful(format!("`{missing}` is not at the cited lines"));
    }
    let lowered = step_text.to_lowercase();
    if let Some(missing) = entities
        .iter()
        .filter(|e| e.chars().count() >= MIN_ENTITY_CHARS)
        .filter(|e| lowered.contains(&e.to_lowercase()))
        .find(|e| !files.contains(&e.to_lowercase()))
    {
        return StepCheck::Doubtful(format!("entity {missing} is not in the cited files"));
    }
    StepCheck::Pass
}

/// The code identifiers a text names: words in backticks, `snake_case`,
/// `camelCase` / `CamelCase` words and calls like `foo(`. Plain words,
/// capitalized or in capitals, are prose. Each identifier once, in order.
fn identifiers(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let mut found: Vec<String> = Vec::new();
    let mut in_backticks = false;
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '`' {
            in_backticks = !in_backticks;
            i += 1;
            continue;
        }
        if !is_word(chars[i]) {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && is_word(chars[i]) {
            i += 1;
        }
        let word: String = chars[start..i].iter().collect();
        let word = word.trim_matches('_');
        let is_call = chars.get(i) == Some(&'(');
        if !word.is_empty()
            && (in_backticks || is_call || looks_like_identifier(word))
            && !found.iter().any(|f| f == word)
        {
            found.push(word.to_string());
        }
    }
    found
}

fn looks_like_identifier(word: &str) -> bool {
    let chars: Vec<char> = word.chars().collect();
    let snake = chars.contains(&'_') && chars.iter().any(|c| c.is_alphabetic());
    let camel = chars
        .windows(2)
        .any(|w| w[0].is_lowercase() && w[1].is_uppercase());
    snake || camel
}

#[cfg(test)]
mod tests {
    use super::*;
    use retrodoc_core::model::{Actor, ActorKind, SourceRef};

    fn step(action: &str, refs: Vec<(&str, Option<u32>, Option<u32>)>) -> Step {
        Step {
            order: 1,
            description: String::new(),
            actor: Actor {
                name: "API".to_string(),
                kind: ActorKind::System,
            },
            action: action.to_string(),
            source_refs: refs
                .into_iter()
                .map(|(path, start_line, end_line)| SourceRef {
                    path: path.to_string(),
                    start_line,
                    end_line,
                })
                .collect(),
        }
    }

    fn repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("a.rs"),
            "fn create_invoice() {}\nfn send_mail() {}\nstruct Invoice;\n",
        )
        .unwrap();
        dir
    }

    #[test]
    fn extracts_code_identifiers_and_not_prose() {
        assert_eq!(
            identifiers("The API calls `save` then create_invoice, UserAccount and run() Done."),
            ["save", "create_invoice", "UserAccount", "run"]
        );
        assert_eq!(
            identifiers("The User sends an Invoice to the API"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn passes_when_names_are_at_the_cited_lines() {
        let dir = repo();
        let s = step("Calls create_invoice", vec![("a.rs", Some(1), Some(1))]);
        assert_eq!(check_step(dir.path(), &s, &[]), StepCheck::Pass);
    }

    #[test]
    fn a_step_without_reference_is_ungrounded() {
        let dir = repo();
        let s = step("Calls create_invoice", vec![]);
        assert_eq!(check_step(dir.path(), &s, &[]), StepCheck::Ungrounded);
    }

    #[test]
    fn a_missing_file_or_line_is_broken() {
        let dir = repo();
        let gone = step("x", vec![("nope.rs", None, None)]);
        assert!(matches!(
            check_step(dir.path(), &gone, &[]),
            StepCheck::Broken(_)
        ));
        let far = step("x", vec![("a.rs", Some(40), Some(45))]);
        assert!(matches!(
            check_step(dir.path(), &far, &[]),
            StepCheck::Broken(_)
        ));
    }

    #[test]
    fn an_identifier_elsewhere_in_the_file_is_doubtful() {
        let dir = repo();
        let s = step("Calls send_mail", vec![("a.rs", Some(1), Some(1))]);
        assert!(matches!(
            check_step(dir.path(), &s, &[]),
            StepCheck::Doubtful(_)
        ));
        let whole = step("Calls send_mail", vec![("a.rs", None, None)]);
        assert_eq!(check_step(dir.path(), &whole, &[]), StepCheck::Pass);
    }

    #[test]
    fn an_entity_absent_from_the_cited_files_is_doubtful() {
        let dir = repo();
        let s = step("Creates a Payment", vec![("a.rs", None, None)]);
        let entities = vec!["Payment".to_string(), "Invoice".to_string()];
        assert!(matches!(
            check_step(dir.path(), &s, &entities),
            StepCheck::Doubtful(_)
        ));
        let ok = step("Creates an Invoice", vec![("a.rs", None, None)]);
        assert_eq!(check_step(dir.path(), &ok, &entities), StepCheck::Pass);
    }
}
