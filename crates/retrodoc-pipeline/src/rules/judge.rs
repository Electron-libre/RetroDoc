//! The second look at the rules of a feature: one call that gives each rule a class, `business` or
//! one of four classes of noise (how data is stored or fetched, what a screen does, what a test of
//! a screen checks, a technical detail). The statement call finds what the code enforces; this one
//! judges what is worth a page, on the statements alone, so it is much shorter. A closed class per
//! rule, not a list of numbers to drop: measured on hand labels of a large repository, the list
//! lost 22% of the good rules, the classes 3% (`issues/rules_deduplication_and_noise.md`). A rule
//! the answer does not class, or classes in an unknown way, is kept. Like the rest of the pass, no
//! pattern of a language or framework: the LLM judges, the program applies the verdict.

use std::fmt::Write as _;

use retrodoc_core::model::{BusinessRule, Feature, RuleEvidence, RuleKind};
use serde::Deserialize;

pub(super) const JUDGE_SYSTEM_PROMPT: &str = "You are classifying the business rules found in \
the code of a software feature, for the documentation read by product owners, support and \
analysts. For each numbered rule, choose exactly one class:\n\
business: a decision of the business: what is allowed or refused, who may do what, a limit or \
quota, the states an object goes through and when, how an amount, a choice, a date or a search \
result is worked out, what is created, changed or removed as a consequence of an action.\n\
storage: how data is stored, fetched, listed or deleted, or how an object is built or \
initialised, with no decision of the business.\n\
screen: what a screen shows, hides, focuses or opens, or how it reacts to a key or a click.\n\
test: what a test checks about a screen or an end-to-end scenario. If the feature itself \
describes how the application is tested rather than what it does, every rule of it is test.\n\
technical: a technical detail no business reader asks about: a protocol or security header, a \
file or folder handled by the program, an internal cache or task, a database mode, the format of \
an identifier, the exact way a number or a list is cut for display.\n\
A rule that refuses or limits something is business even if it is simple. Reply with ONLY a \
single JSON object, no prose and no Markdown code fence: \
{\"rules\":[{\"n\":1,\"class\":\"business\"}]}.";

/// The classes that are noise.
const NOISE: [&str; 4] = ["storage", "screen", "test", "technical"];

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(super) struct RawVerdict {
    #[serde(default)]
    rules: Vec<RawClass>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct RawClass {
    /// Number of the rule in the prompt.
    #[serde(default)]
    #[schemars(with = "u32")]
    n: serde_json::Value,
    #[serde(default)]
    class: String,
}

/// The prompt: the feature and its numbered rules, with their kind and what backs them.
pub(super) fn judge_prompt(feature: &Feature, rules: &[BusinessRule]) -> String {
    let mut prompt = format!(
        "Feature: {} — {}\n\nRules:\n",
        feature.name, feature.description
    );
    for (n, rule) in rules.iter().enumerate() {
        let _ = writeln!(
            prompt,
            "{}. [{}, {}] {}",
            n + 1,
            kind_name(rule.kind),
            backing(rule),
            one_line(&rule.statement)
        );
    }
    prompt
}

/// `text` on one line, so that it can't forge a numbered line of the prompt.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `rules` split into those kept and those the verdict classes as noise.
pub(super) fn keep_judged(
    rules: Vec<BusinessRule>,
    verdict: &RawVerdict,
) -> (Vec<BusinessRule>, Vec<BusinessRule>) {
    let dropped: Vec<usize> = verdict
        .rules
        .iter()
        .filter(|c| NOISE.contains(&c.class.trim().to_lowercase().as_str()))
        .filter_map(|c| match &c.n {
            // `2.0` is a number too.
            serde_json::Value::Number(n) => {
                n.to_string().trim_end_matches(".0").parse::<u64>().ok()
            }
            serde_json::Value::String(s) => s.trim().parse().ok(),
            _ => None,
        })
        .filter_map(|n| usize::try_from(n).ok())
        .collect();
    let mut kept = Vec::new();
    let mut gone = Vec::new();
    for (i, rule) in rules.into_iter().enumerate() {
        if dropped.contains(&(i + 1)) {
            gone.push(rule);
        } else {
            kept.push(rule);
        }
    }
    (kept, gone)
}

fn kind_name(kind: RuleKind) -> &'static str {
    match kind {
        RuleKind::Validation => "validation",
        RuleKind::Permission => "permission",
        RuleKind::StateTransition => "state transition",
        RuleKind::Limit => "limit",
        RuleKind::Computation => "computation",
    }
}

fn backing(rule: &BusinessRule) -> &'static str {
    let has = |f: fn(&RuleEvidence) -> bool| rule.evidence.iter().any(f);
    match (
        has(|e| matches!(e, RuleEvidence::Test { .. })),
        has(|e| matches!(e, RuleEvidence::Doc { .. })),
    ) {
        (false, false) => "code alone",
        (true, false) => "code and test",
        (false, true) => "code and doc",
        (true, true) => "code, test and doc",
    }
}
