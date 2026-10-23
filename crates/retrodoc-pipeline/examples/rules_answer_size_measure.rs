//! Measures, with no LLM call, how close the rules answer of each feature of saved runs comes to the
//! completion cap (`issues/done/rules_answer_truncated_on_large_features.md`, step 1).
//!
//! Usage: `cargo run -q -p retrodoc-pipeline --example rules_answer_size_measure -- <run dir>...`
//!
//! A run dir is a checkout of the repository with its `.retrodoc/cache` (`rules.yaml` and
//! `rules-set-aside.yaml`). The answer is rebuilt in the shape the prompt asks for (statement, kind,
//! entities, use cases, code, tests, docs) from the rules a feature kept and the ones set aside, so
//! it is an estimate: an answer cut by the cap was never saved (its feature has no rules), and the
//! rules the deduplication dropped are missing. Printed per run: the rules and the characters of the
//! answer per feature (most loaded first) with the share of the cap and of each field, then how many
//! features pass 50, 75 and 100 % of the cap.
//!
//! The cap is `MAX_COMPLETION_TOKENS` (8192) at about 3.1 characters per token, as observed on the
//! cut answers (25.5k to 25.8k characters).

#![allow(clippy::cast_precision_loss)] // counts of characters: far below 2^52

use std::collections::BTreeMap;
use std::path::Path;

use retrodoc_core::model::{BusinessRule, RuleEvidence};
use retrodoc_pipeline::{load_rules, load_set_aside};

const CAP_CHARS: usize = 25_400;
const TOP: usize = 12;

/// The feature slug of a rule slug: `<feature-slug>-<n>` without `-<n>`.
fn feature_of(slug: &str) -> &str {
    slug.rsplit_once('-')
        .filter(|(_, n)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
        .map_or(slug, |(feature, _)| feature)
}

/// Characters of each field of the answer of one feature.
#[derive(Default)]
struct Size {
    rules: usize,
    statement: usize,
    kind: usize,
    names: usize, // entities and use cases
    code: usize,
    tests: usize,
    docs: usize,
}

impl Size {
    fn total(&self) -> usize {
        // The keys and braces of each rule object, about as long as the keys of the shape.
        const SHAPE: usize = 120;
        self.statement
            + self.kind
            + self.names
            + self.code
            + self.tests
            + self.docs
            + self.rules * SHAPE
    }

    fn add(&mut self, rule: &BusinessRule) {
        self.rules += 1;
        self.statement += rule.statement.len();
        self.kind += format!("{:?}", rule.kind).len() + 4;
        self.names += rule.entities.iter().map(|e| e.len() + 3).sum::<usize>();
        self.names += rule.use_cases.iter().map(|u| u.len() + 3).sum::<usize>();
        for evidence in &rule.evidence {
            match evidence {
                RuleEvidence::Code { source } => {
                    // {"path":"...","start_line":N,"end_line":N},
                    self.code += source.path.len() + 55;
                }
                RuleEvidence::Test { phrase, .. } => {
                    // {"id":"T1","phrase":"..."},
                    self.tests += phrase.len() + 25;
                }
                RuleEvidence::Doc { .. } => self.docs += 6,
            }
        }
    }
}

fn percent(part: usize, total: usize) -> usize {
    (part * 100).checked_div(total).unwrap_or(0)
}

fn measure(dir: &Path) {
    let Some(rules) = load_rules(dir) else {
        println!("{}: no rules.yaml", dir.display());
        return;
    };
    let mut sizes: BTreeMap<String, Size> = BTreeMap::new();
    for rule in &rules {
        sizes
            .entry(feature_of(&rule.slug).to_string())
            .or_default()
            .add(rule);
    }
    // A rule set aside was in the answer too; only its statement is known.
    let mut aside = 0;
    for rule in load_set_aside(dir).unwrap_or_default().rules {
        let size = sizes.entry(rule.feature.clone()).or_default();
        size.rules += 1;
        size.statement += rule.statement.len();
        aside += 1;
    }
    let mut rows: Vec<(&String, &Size)> = sizes.iter().collect();
    rows.sort_by_key(|(_, size)| std::cmp::Reverse(size.total()));

    println!(
        "== {}: {} features, {} rules kept, {aside} set aside",
        dir.display(),
        rows.len(),
        rules.len()
    );
    for (feature, size) in rows.iter().take(TOP) {
        let total = size.total();
        println!(
            "{feature}: {} rules, {total} chars ({}% of the cap); statement {}%, code {}%, tests {}%, names {}%, docs+kind {}%",
            size.rules,
            percent(total, CAP_CHARS),
            percent(size.statement, total),
            percent(size.code, total),
            percent(size.tests, total),
            percent(size.names, total),
            percent(size.docs + size.kind, total),
        );
    }
    for share in [50, 75, 100] {
        let over = rows
            .iter()
            .filter(|(_, size)| percent(size.total(), CAP_CHARS) >= share)
            .count();
        println!("features at {share}% of the cap or more: {over}");
    }
    let (rules_count, chars): (usize, usize) = rows
        .iter()
        .fold((0, 0), |(r, c), (_, s)| (r + s.rules, c + s.total()));
    if let Some(mean) = chars.checked_div(rules_count) {
        println!("mean answer per rule: {mean} chars");
    }
}

fn main() {
    let dirs: Vec<String> = std::env::args().skip(1).collect();
    if dirs.is_empty() {
        eprintln!("usage: rules_answer_size_measure <run dir>...");
        std::process::exit(2);
    }
    for dir in &dirs {
        measure(Path::new(dir));
    }
}
