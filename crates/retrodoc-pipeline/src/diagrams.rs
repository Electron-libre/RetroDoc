//! Diagrams pass (PLAN.md §2 step 6, roadmap phase 4): Mermaid
//! `sequenceDiagram` per use case, generated *deterministically* from its
//! steps (no LLM call). The steps already carry actor and action, so asking
//! a model to re-draw them would only add cost and the risk of invalid
//! Mermaid syntax.
//!
//! Each step is drawn as a message from the previous step's actor to this
//! step's actor (a self-message for the first step), labeled with the step
//! number and action.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use retrodoc_core::model::{Step, UseCase};

use crate::use_cases::is_human;

/// Attaches a Mermaid sequence diagram to every use case that has steps.
pub fn attach_diagrams(use_cases: &mut [UseCase]) {
    for use_case in use_cases {
        use_case.diagram_mermaid = sequence_diagram(&use_case.steps);
    }
}

/// Renders `steps` as a Mermaid `sequenceDiagram`; `None` when empty.
#[must_use]
pub fn sequence_diagram(steps: &[Step]) -> Option<String> {
    if steps.is_empty() {
        return None;
    }

    // Participants in order of first appearance, keyed by actor name; an
    // opaque alias (`P1`…) keeps odd actor names from breaking the syntax.
    let mut aliases: BTreeMap<&str, usize> = BTreeMap::new();
    let mut diagram = String::from("sequenceDiagram\n");
    for step in steps {
        if aliases.contains_key(step.actor.name.as_str()) {
            continue;
        }
        let alias = aliases.len() + 1;
        aliases.insert(&step.actor.name, alias);
        let keyword = if is_human(&step.actor) {
            "actor"
        } else {
            "participant"
        };
        let _ = writeln!(
            diagram,
            "    {keyword} P{alias} as {}",
            sanitize(&step.actor.name)
        );
    }

    let mut previous = &steps[0].actor.name;
    for step in steps {
        let from = aliases[previous.as_str()];
        let to = aliases[step.actor.name.as_str()];
        let _ = writeln!(
            diagram,
            "    P{from}->>P{to}: {}. {}",
            step.order,
            sanitize(&step.action)
        );
        previous = &step.actor.name;
    }
    Some(diagram)
}

/// Strips the characters that terminate or corrupt a Mermaid message/label
/// (`;` and `#` are statement/entity markers, newlines break the line).
fn sanitize(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            ';' => ',',
            '#' | '\n' | '\r' => ' ',
            c => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    use retrodoc_core::model::{Actor, ActorKind};

    fn step(order: u32, actor: &str, kind: ActorKind, action: &str) -> Step {
        Step {
            order,
            description: String::new(),
            actor: Actor {
                name: actor.to_string(),
                kind,
            },
            action: action.to_string(),
            source_refs: Vec::new(),
        }
    }

    #[test]
    fn sequence_diagram_declares_each_actor_once_and_chains_steps() {
        let steps = [
            step(1, "Customer", ActorKind::Human, "submits payment"),
            step(2, "API", ActorKind::System, "validates; stores #1"),
            step(3, "Customer", ActorKind::Human, "sees receipt"),
        ];

        let diagram = sequence_diagram(&steps).unwrap();

        assert_eq!(
            diagram,
            "sequenceDiagram\n\
             \x20   actor P1 as Customer\n\
             \x20   participant P2 as API\n\
             \x20   P1->>P1: 1. submits payment\n\
             \x20   P1->>P2: 2. validates, stores  1\n\
             \x20   P2->>P1: 3. sees receipt\n"
        );
    }

    #[test]
    fn sequence_diagram_is_none_without_steps() {
        assert!(sequence_diagram(&[]).is_none());
    }

    #[test]
    fn attach_diagrams_fills_every_use_case() {
        let mut use_cases = vec![UseCase {
            entry_points: Vec::new(),
            primary_actor: None,
            narrative: None,
            business_language: None,
            slug: "u".to_string(),
            feature_slug: "f".to_string(),
            name: "U".to_string(),
            description: String::new(),
            steps: vec![step(1, "A", ActorKind::System, "runs")],
            diagram_mermaid: None,
            confidence: None,
        }];

        attach_diagrams(&mut use_cases);

        assert!(use_cases[0]
            .diagram_mermaid
            .as_deref()
            .unwrap()
            .starts_with("sequenceDiagram"));
    }
}
