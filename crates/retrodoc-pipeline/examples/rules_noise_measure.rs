//! Measures, with no LLM call, the duplicates of a saved `rules.yaml` and draws a sample to label by
//! hand (`issues/rules_deduplication_and_noise.md`, step 1).
//!
//! Usage: `cargo run -q -p retrodoc-pipeline --example rules_noise_measure -- [--sample N] [--seed S] <run dir>`
//!
//! A run dir is a checkout of the repository with its `.retrodoc/cache` (`rules.yaml`). Printed:
//! * the rules by kind and by what backs them (code alone, with a test or a doc);
//! * the rules per feature (the slug without its `-<n>`), most loaded first;
//! * the duplicate pairs among rules of different features: *worded* when the statements share most
//!   of their words (Jaccard at least 0.6), *same code* when they also cite overlapping lines and
//!   share at least 0.4 of their words;
//! * a sample of `N` rules (default 60) in a fixed order for a given seed (default 1), one per
//!   line, to be labelled by hand: good, UI noise, test noise, plumbing, duplicate, wrong kind.
//!
//! The statements and the paths are printed to read a sample by hand, never to be copied in a
//! committed file when the repository is confidential.

#![allow(clippy::cast_precision_loss)] // counts of rules: far below 2^52

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use retrodoc_core::model::{BusinessRule, RuleEvidence};
use retrodoc_pipeline::load_rules;

const WORDED: f64 = 0.6;
const SAME_CODE_WORDS: f64 = 0.4;

/// The feature slug of a rule slug: `<feature-slug>-<n>` without `-<n>`.
fn feature_of(rule: &BusinessRule) -> &str {
    rule.slug
        .rsplit_once('-')
        .filter(|(_, n)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
        .map_or(rule.slug.as_str(), |(feature, _)| feature)
}

fn words(statement: &str) -> BTreeSet<String> {
    statement
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 2)
        .map(str::to_string)
        .collect()
}

fn jaccard(a: &BTreeSet<String>, b: &BTreeSet<String>) -> f64 {
    let common = a.intersection(b).count();
    let all = a.union(b).count();
    if all == 0 {
        0.0
    } else {
        common as f64 / all as f64
    }
}

/// Cited code ranges: `(path, start, end)`; a missing line stands for the whole file.
fn code_ranges(rule: &BusinessRule) -> Vec<(&str, u32, u32)> {
    rule.evidence
        .iter()
        .filter_map(|e| match e {
            RuleEvidence::Code { source } => Some((
                source.path.as_str(),
                source.start_line.unwrap_or(0),
                source.end_line.unwrap_or(u32::MAX),
            )),
            _ => None,
        })
        .collect()
}

fn overlap(a: &BusinessRule, b: &BusinessRule) -> bool {
    let (a, b) = (code_ranges(a), code_ranges(b));
    a.iter().any(|(pa, sa, ea)| {
        b.iter()
            .any(|(pb, sb, eb)| pa == pb && sa <= eb && sb <= ea)
    })
}

fn backing(rule: &BusinessRule) -> &'static str {
    let test = rule
        .evidence
        .iter()
        .any(|e| matches!(e, RuleEvidence::Test { .. }));
    let doc = rule
        .evidence
        .iter()
        .any(|e| matches!(e, RuleEvidence::Doc { .. }));
    match (test, doc) {
        (false, false) => "code",
        (true, false) => "code+test",
        (false, true) => "code+doc",
        (true, true) => "code+test+doc",
    }
}

/// Fixed-seed shuffle (xorshift), so the same seed draws the same sample.
fn shuffled<T>(mut items: Vec<T>, seed: u64) -> Vec<T> {
    let mut state = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    for i in (1..items.len()).rev() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let j = usize::try_from(state % (i as u64 + 1)).unwrap_or(0);
        items.swap(i, j);
    }
    items
}

fn main() {
    let mut sample = 60usize;
    let mut seed = 1u64;
    let mut dir = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--sample" => sample = args.next().and_then(|v| v.parse().ok()).unwrap_or(sample),
            "--seed" => seed = args.next().and_then(|v| v.parse().ok()).unwrap_or(seed),
            _ => dir = Some(arg),
        }
    }
    let Some(dir) = dir else {
        eprintln!("usage: rules_noise_measure [--sample N] [--seed S] <run dir>");
        std::process::exit(2);
    };
    let Some(rules) = load_rules(Path::new(&dir)) else {
        eprintln!("no readable rules.yaml under {dir}/.retrodoc/cache");
        std::process::exit(1);
    };

    println!("rules: {}", rules.len());
    let mut by_kind: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_backing: BTreeMap<&str, usize> = BTreeMap::new();
    let mut by_feature: BTreeMap<&str, usize> = BTreeMap::new();
    for rule in &rules {
        *by_kind.entry(format!("{:?}", rule.kind)).or_default() += 1;
        *by_backing.entry(backing(rule)).or_default() += 1;
        *by_feature.entry(feature_of(rule)).or_default() += 1;
    }
    println!("by kind: {by_kind:?}");
    println!("by backing: {by_backing:?}");
    let mut loaded: Vec<_> = by_feature.iter().collect();
    loaded.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
    println!(
        "features with rules: {}; most loaded: {:?}",
        by_feature.len(),
        &loaded[..loaded.len().min(5)]
    );

    let tokens: Vec<_> = rules.iter().map(|r| words(&r.statement)).collect();
    let (mut worded, mut same_code) = (0usize, 0usize);
    for i in 0..rules.len() {
        for j in i + 1..rules.len() {
            if feature_of(&rules[i]) == feature_of(&rules[j]) {
                continue;
            }
            let score = jaccard(&tokens[i], &tokens[j]);
            let is_worded = score >= WORDED;
            let is_same_code = score >= SAME_CODE_WORDS && overlap(&rules[i], &rules[j]);
            worded += usize::from(is_worded);
            same_code += usize::from(is_same_code);
            if is_worded || is_same_code {
                println!(
                    "PAIR {score:.2} {} | {} :: {} || {}",
                    rules[i].slug, rules[j].slug, rules[i].statement, rules[j].statement
                );
            }
        }
    }
    println!("duplicate pairs: worded {worded}, same code {same_code}");

    println!("\nSAMPLE (seed {seed}): slug | kind | backing | statement");
    for rule in shuffled(rules.iter().collect(), seed)
        .into_iter()
        .take(sample)
    {
        println!(
            "{} | {:?} | {} | {}",
            rule.slug,
            rule.kind,
            backing(rule),
            rule.statement
        );
    }
}
