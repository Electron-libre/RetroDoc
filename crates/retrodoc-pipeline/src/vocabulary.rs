//! Business-language criterion (PLAN.md §7.1, phase 7 step 4e): "is this
//! business language?", scored next to the confidence. The confidence pass
//! rewards closeness to the code, which favours literal paraphrase; this one
//! looks at the other side, whether a use case could be read by someone who
//! does not know the code.
//!
//! It is deterministic (no LLM call, so it is recomputed on every run for
//! free) and explainable, built from what the narrative says:
//! - no narrative at all scores 0;
//! - each code-level token in the narrative (an identifier like
//!   `contract_signer`, a path, a call `foo()`, an HTTP verb in capitals, a
//!   multi-word class name that is not a known entity, a word such as
//!   "controller" or "params") costs 0.15, up to 0.6;
//! - naming no known entity or actor costs 0.3;
//! - when the actors are known, a use case with neither entry point nor
//!   primary actor (an internal helper, not something someone does) costs 0.2.
//!
//! It is a hint to review, like the glossary: a method name can be perfectly
//! good business wording, and a bland narrative can still pass.

use std::collections::BTreeSet;

use retrodoc_core::model::{ConfidenceScore, UseCase};

use crate::actors::{words, Actors};

/// Score removed for each code-level word found in the narrative.
const LEAK_PENALTY: f32 = 0.15;
/// Most that the code-level words together can remove.
const MAX_LEAK_PENALTY: f32 = 0.6;
/// Removed when the narrative names no known entity or actor.
const NO_VOCABULARY_PENALTY: f32 = 0.3;
/// Removed when a use case has neither an entry point nor a primary actor.
const NO_TRIGGER_PENALTY: f32 = 0.2;
/// Code-level words quoted in the reason shown to the reader.
const MAX_LEAKS_REPORTED: usize = 4;

/// Lowercase words that talk about the implementation, not the business.
const TECHNICAL_WORDS: &[&str] = &[
    "controller",
    "controllers",
    "endpoint",
    "endpoints",
    "database",
    "sql",
    "json",
    "params",
    "callback",
    "callbacks",
    "middleware",
    "payload",
    "boolean",
    "nil",
    "null",
    "method",
    "methods",
    "function",
    "functions",
    "class",
    "classes",
    "redirect",
    "redirects",
    "redirected",
    "renders",
    "rendered",
    "partial",
    "http",
    "parameter",
    "parameters",
    "instance",
    "variable",
    "variables",
];

const HTTP_VERBS: &[&str] = &["GET", "POST", "PUT", "PATCH", "DELETE"];
const CODE_EXTENSIONS: &[&str] = &[
    ".rb", ".rs", ".js", ".ts", ".erb", ".yml", ".yaml", ".py", ".go", ".java", ".php",
];

/// Scores every use case's business language, in place.
pub fn score_business_language(use_cases: &mut [UseCase], vocabulary: &[String], actors: &Actors) {
    for use_case in use_cases {
        use_case.business_language = Some(business_language_score(use_case, vocabulary, actors));
    }
}

/// The business-language score of one use case (see the module docs).
#[must_use]
pub fn business_language_score(
    use_case: &UseCase,
    vocabulary: &[String],
    actors: &Actors,
) -> ConfidenceScore {
    let Some(narrative) = use_case
        .narrative
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
    else {
        return ConfidenceScore::new(0.0, "no business narrative".to_string());
    };

    let names: Vec<&str> = vocabulary
        .iter()
        .map(String::as_str)
        .chain(actors.actors.iter().map(|a| a.name.as_str()))
        .collect();
    let known: BTreeSet<String> = names.iter().map(|n| n.to_lowercase()).collect();

    let mut score = 1.0_f32;
    let mut reasons: Vec<String> = Vec::new();

    let leaks = code_leaks(narrative, &known);
    if !leaks.is_empty() {
        #[allow(clippy::cast_precision_loss)]
        let penalty = (LEAK_PENALTY * leaks.len() as f32).min(MAX_LEAK_PENALTY);
        score -= penalty;
        let shown: Vec<&str> = leaks
            .iter()
            .take(MAX_LEAKS_REPORTED)
            .map(String::as_str)
            .collect();
        reasons.push(format!(
            "code-level wording in the narrative: {}",
            shown.join(", ")
        ));
    }

    if !names.is_empty() && !mentions_any(narrative, &names) {
        score -= NO_VOCABULARY_PENALTY;
        reasons.push("names no business entity or actor".to_string());
    }

    if !actors.is_empty() && use_case.entry_points.is_empty() && use_case.primary_actor.is_none() {
        score -= NO_TRIGGER_PENALTY;
        reasons
            .push("no entry point and no primary actor: reads like an internal helper".to_string());
    }

    let rationale = (!reasons.is_empty()).then(|| reasons.join("; "));
    ConfidenceScore::new(score.max(0.0), rationale)
}

/// Distinct code-level tokens of `text` (lowercased), in order of appearance.
/// `known` holds the lowercased entity and actor names, which are allowed to
/// look like class names.
fn code_leaks(text: &str, known: &BTreeSet<String>) -> Vec<String> {
    let mut leaks: Vec<String> = Vec::new();
    for raw in text.split_whitespace() {
        let token = raw.trim_matches(|c: char| ".,;:!?\"'()[]{}".contains(c));
        if token.is_empty() {
            continue;
        }
        let lower = token.to_lowercase();
        let is_leak = (raw.contains("()"))
            || token.contains("::")
            || (token.contains('_') && token.chars().any(char::is_alphabetic))
            || CODE_EXTENSIONS.iter().any(|e| lower.ends_with(e))
            || (token.contains('/') && token.len() > 5 && lower != "and/or")
            || HTTP_VERBS.contains(&token)
            || TECHNICAL_WORDS.contains(&lower.as_str())
            || (is_multi_word_class_name(token) && !known.contains(&lower));
        if is_leak && !leaks.contains(&lower) {
            leaks.push(lower);
        }
    }
    leaks
}

/// `ContractSigner` but not `Contract`, `PATCH` or `iPhone`.
fn is_multi_word_class_name(token: &str) -> bool {
    token.chars().next().is_some_and(char::is_uppercase)
        && token.chars().all(char::is_alphanumeric)
        && words(token).len() > 1
        && token.chars().any(char::is_lowercase)
}

fn singular(word: &str) -> &str {
    match word.strip_suffix('s') {
        Some(stem) if stem.len() > 3 => stem,
        _ => word,
    }
}

/// Whether `text` names one of `names` (an entity or an actor), as the same
/// words in a row, ignoring case and plural `s`: "signature transactions"
/// names `SignatureTransaction`, "contract manager" names `Contract Manager`.
fn mentions_any(text: &str, names: &[&str]) -> bool {
    let text_words: Vec<String> = words(text)
        .iter()
        .map(|w| singular(w).to_string())
        .collect();
    names.iter().any(|name| {
        let name_words: Vec<String> = words(name)
            .iter()
            .map(|w| singular(w).to_string())
            .collect();
        !name_words.is_empty()
            && text_words
                .windows(name_words.len())
                .any(|window| window == name_words.as_slice())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use retrodoc_core::model::ActorKind;

    use crate::actors::BusinessActor;

    fn use_case(narrative: Option<&str>) -> UseCase {
        UseCase {
            slug: "u".to_string(),
            feature_slug: "f".to_string(),
            name: "U".to_string(),
            description: "d".to_string(),
            steps: Vec::new(),
            entry_points: vec!["PATCH /x".to_string()],
            primary_actor: Some("Contract Manager".to_string()),
            narrative: narrative.map(str::to_string),
            business_language: None,
            diagram_mermaid: None,
            confidence: None,
        }
    }

    fn vocabulary() -> Vec<String> {
        vec!["Contract".to_string(), "SignatureTransaction".to_string()]
    }

    fn actors() -> Actors {
        Actors {
            input_hash: String::new(),
            actors: vec![BusinessActor {
                name: "Contract Manager".to_string(),
                kind: ActorKind::Human,
                description: String::new(),
                evidence: Vec::new(),
            }],
        }
    }

    fn score(narrative: Option<&str>) -> ConfidenceScore {
        business_language_score(&use_case(narrative), &vocabulary(), &actors())
    }

    #[test]
    fn a_business_narrative_naming_entities_and_actors_scores_full() {
        let result = score(Some(
            "The Contract Manager replaces the signatory of a contract. Ongoing signature \
             transactions are cancelled and the parties are notified.",
        ));
        assert!((result.value - 1.0).abs() < f32::EPSILON, "{result:?}");
        assert!(result.rationale.is_none());
    }

    #[test]
    fn code_level_wording_costs_points_and_is_reported() {
        let result = score(Some(
            "The controller calls ContractSigner.update() with params from app/services/signer.rb \
             and a PATCH request, then the contract_signer redirects.",
        ));
        assert!(result.value <= 0.4 + f32::EPSILON, "{result:?}");
        let why = result.rationale.unwrap();
        assert!(
            why.contains("controller") && why.contains("app/services/signer.rb"),
            "{why}"
        );
    }

    #[test]
    fn known_entities_may_look_like_class_names_unknown_ones_may_not() {
        // `SignatureTransaction` is a known entity; `ContractSigner` is not.
        assert!(
            (score(Some("The Contract Manager cancels a SignatureTransaction.")).value - 1.0).abs()
                < f32::EPSILON
        );
        assert!(score(Some("The Contract Manager asks the ContractSigner.")).value < 1.0);
    }

    #[test]
    fn missing_narrative_and_missing_vocabulary_are_penalized() {
        assert!(score(None).value.abs() < f32::EPSILON);
        assert_eq!(
            score(None).rationale.as_deref(),
            Some("no business narrative")
        );

        let bland = score(Some(
            "Something is updated and then something else happens.",
        ));
        assert!((bland.value - 0.7).abs() < 1e-6, "{bland:?}");
        assert!(bland
            .rationale
            .unwrap()
            .contains("no business entity or actor"));
    }

    #[test]
    fn a_use_case_nobody_triggers_reads_like_a_helper() {
        let mut helper = use_case(Some("The contract is checked."));
        helper.entry_points.clear();
        helper.primary_actor = None;
        let result = business_language_score(&helper, &vocabulary(), &actors());
        assert!((result.value - 0.8).abs() < 1e-6, "{result:?}");
        assert!(result.rationale.unwrap().contains("internal helper"));
        // Without known actors the check does not apply (no way to tell).
        let result = business_language_score(&helper, &vocabulary(), &Actors::default());
        assert!((result.value - 1.0).abs() < 1e-6);
    }

    #[test]
    fn scoring_fills_every_use_case() {
        let mut list = vec![
            use_case(Some("The Contract Manager signs a contract.")),
            use_case(None),
        ];
        score_business_language(&mut list, &vocabulary(), &actors());
        assert!(list.iter().all(|u| u.business_language.is_some()));
    }
}
