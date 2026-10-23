//! The same rule stated by several features, kept once. Each feature is asked on its own, so two
//! features that read the same code state the same rule in their own words. No LLM and no pattern
//! of a language or framework: two rules of different features are the same when their statements
//! share most of their words and they cite overlapping lines of the same file, or share nearly all
//! of them. Measured by hand on two repositories (`issues/done/rules_deduplication_and_noise.md`): that
//! test never joined two different rules, and found about half of the repeated ones; the others
//! are worded too differently, and are left alone.
//!
//! Rules are joined by decreasing similarity, a group never holding two rules of one feature (a
//! feature does not state a rule twice), and two groups are joined only if every rule of one is
//! similar to every rule of the other. So "unknown customer" and "unknown restaurant", whose
//! statements differ by one word, are not joined through a third rule.
//!
//! The copy that stays is the one of the feature that owns the code: the one with an entry point in
//! the cited file, then the one backed by a test or a document, then the one that reads the fewest
//! files (the most specific), then the first by slug.

use std::collections::{BTreeMap, BTreeSet};

use retrodoc_core::model::{BusinessRule, ConfidenceScore, RuleEvidence};

/// Words in common, over all the words, from which two statements are alike.
const ALIKE: f64 = 0.6;
/// From which two statements are the same rule even if they cite no common line.
const SAME: f64 = 0.75;

/// What a feature reads, to tell which one owns the code a rule cites.
#[derive(Debug, Clone, Default)]
pub(super) struct Reach {
    /// The files that hold its entry points.
    pub(super) entry_files: BTreeSet<String>,
    /// The number of files whose code it reads.
    pub(super) files: usize,
}

fn words(statement: &str) -> BTreeSet<String> {
    statement
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() > 3)
        .map(str::to_string)
        .collect()
}

#[allow(clippy::cast_precision_loss)] // counts of words
fn jaccard(a: &BTreeSet<String>, b: &BTreeSet<String>) -> f64 {
    let all = a.union(b).count();
    if all == 0 {
        0.0
    } else {
        a.intersection(b).count() as f64 / all as f64
    }
}

/// The cited ranges of code: path, first and last line (a missing line stands for the whole file).
fn code(rule: &BusinessRule) -> Vec<(&str, u32, u32)> {
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
    let (a, b) = (code(a), code(b));
    a.iter().any(|(pa, sa, ea)| {
        b.iter()
            .any(|(pb, sb, eb)| pa == pb && sa <= eb && sb <= ea)
    })
}

/// `rules` with, of the rules several features state alike, the copy of the feature that owns the
/// code. `reach` is by feature slug; a rule of no feature of `reach` is never joined.
pub(super) fn merge_duplicates(
    rules: Vec<BusinessRule>,
    reach: &BTreeMap<String, Reach>,
) -> Vec<BusinessRule> {
    let feature: Vec<Option<&str>> = rules
        .iter()
        .map(|r| {
            reach
                .keys()
                .find(|f| r.belongs_to_feature(f))
                .map(String::as_str)
        })
        .collect();
    let tokens: Vec<_> = rules.iter().map(|r| words(&r.statement)).collect();

    let mut pairs: Vec<(f64, usize, usize)> = Vec::new();
    for i in 0..rules.len() {
        for j in i + 1..rules.len() {
            if feature[i].is_none() || feature[j].is_none() || feature[i] == feature[j] {
                continue;
            }
            let score = jaccard(&tokens[i], &tokens[j]);
            if score >= ALIKE && (score >= SAME || overlap(&rules[i], &rules[j])) {
                pairs.push((score, i, j));
            }
        }
    }
    if pairs.is_empty() {
        return rules;
    }
    pairs.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    let alike: BTreeSet<(usize, usize)> = pairs.iter().map(|&(_, i, j)| (i, j)).collect();
    let is_alike = |a: usize, b: usize| alike.contains(&(a.min(b), a.max(b)));

    let mut group: Vec<usize> = (0..rules.len()).collect();
    let mut members: BTreeMap<usize, Vec<usize>> = (0..rules.len()).map(|i| (i, vec![i])).collect();
    for (_, i, j) in pairs {
        let (gi, gj) = (group[i], group[j]);
        if gi == gj {
            continue;
        }
        let (a, b) = (&members[&gi], &members[&gj]);
        let features: BTreeSet<_> = a.iter().map(|&m| feature[m]).collect();
        let disjoint = b.iter().all(|&m| !features.contains(&feature[m]));
        if disjoint && a.iter().all(|&x| b.iter().all(|&y| is_alike(x, y))) {
            let moved = members.remove(&gj).unwrap_or_default();
            for &m in &moved {
                group[m] = gi;
            }
            members.entry(gi).or_default().extend(moved);
        }
    }

    let mut rules = rules;
    let mut gone = BTreeSet::new();
    for copies in members.values().filter(|m| m.len() > 1) {
        let keeper = copies
            .iter()
            .copied()
            .min_by_key(|&m| owner_key(&rules[m], feature[m], reach))
            .unwrap_or(copies[0]);
        for &copy in copies.iter().filter(|&&m| m != keeper) {
            let copy_rule = rules[copy].clone();
            absorb(&mut rules[keeper], copy_rule);
            gone.insert(copy);
        }
    }
    tracing::info!(
        merged = gone.len(),
        "rules stated by several features kept once"
    );
    rules
        .into_iter()
        .enumerate()
        .filter(|(i, _)| !gone.contains(i))
        .map(|(_, r)| r)
        .collect()
}

/// What `copy` knew that `keeper` does not: the entities and use cases it applies to, the tests and
/// documents that back it (and the confidence that goes with them).
fn absorb(keeper: &mut BusinessRule, copy: BusinessRule) {
    for entity in copy.entities {
        if !keeper.entities.contains(&entity) {
            keeper.entities.push(entity);
        }
    }
    for use_case in copy.use_cases {
        if !keeper.use_cases.contains(&use_case) {
            keeper.use_cases.push(use_case);
        }
    }
    for evidence in copy.evidence {
        if !matches!(evidence, RuleEvidence::Code { .. }) && !keeper.evidence.contains(&evidence) {
            keeper.evidence.push(evidence);
        }
    }
    let best = |c: &Option<ConfidenceScore>| c.as_ref().map_or(0.0, |c| c.value);
    if best(&copy.confidence) > best(&keeper.confidence) {
        keeper.confidence = copy.confidence;
    }
}

/// Smaller is better: owns a cited file, is backed by a test or a document, reads fewer files.
fn owner_key(
    rule: &BusinessRule,
    feature: Option<&str>,
    reach: &BTreeMap<String, Reach>,
) -> (bool, bool, usize, String) {
    let reach = feature
        .and_then(|f| reach.get(f))
        .cloned()
        .unwrap_or_default();
    let owns = code(rule)
        .iter()
        .any(|(path, _, _)| reach.entry_files.contains(*path));
    // A feature that reads no file states its rules from a previous run: the least authoritative.
    let files = if reach.files == 0 {
        usize::MAX
    } else {
        reach.files
    };
    (!owns, !rule.has_corroboration(), files, rule.slug.clone())
}

#[cfg(test)]
mod tests {
    use retrodoc_core::model::{RuleKind, SourceRef};

    use super::*;

    fn rule(slug: &str, statement: &str, path: &str, lines: (u32, u32)) -> BusinessRule {
        BusinessRule {
            slug: slug.into(),
            statement: statement.into(),
            kind: RuleKind::Limit,
            entities: Vec::new(),
            use_cases: Vec::new(),
            evidence: vec![RuleEvidence::Code {
                source: SourceRef {
                    path: path.into(),
                    start_line: Some(lines.0),
                    end_line: Some(lines.1),
                },
            }],
            confidence: None,
        }
    }

    fn reach(features: &[(&str, &[&str], usize)]) -> BTreeMap<String, Reach> {
        features
            .iter()
            .map(|(slug, entries, files)| {
                (
                    (*slug).to_string(),
                    Reach {
                        entry_files: entries.iter().map(ToString::to_string).collect(),
                        files: *files,
                    },
                )
            })
            .collect()
    }

    const LIMIT: &str =
        "An order is rejected if its calculated delivery time is 60 minutes or more.";

    fn slugs(rules: &[BusinessRule]) -> Vec<&str> {
        rules.iter().map(|r| r.slug.as_str()).collect()
    }

    #[test]
    fn the_same_rule_is_kept_on_the_feature_that_owns_the_code() {
        let rules = vec![
            rule("wide-1", LIMIT, "lib/router.rb", (3, 5)),
            rule(
                "orders-1",
                "An order is rejected if its calculated delivery time is 60 minutes or more",
                "lib/router.rb",
                (4, 6),
            ),
        ];
        let reach = reach(&[("wide", &[], 4), ("orders", &["lib/router.rb"], 4)]);
        assert_eq!(slugs(&merge_duplicates(rules, &reach)), ["orders-1"]);
    }

    #[test]
    fn the_most_specific_feature_keeps_it_when_none_owns_the_code() {
        let rules = vec![
            rule("wide-1", LIMIT, "lib/router.rb", (3, 5)),
            rule("narrow-1", LIMIT, "lib/router.rb", (3, 5)),
        ];
        let reach = reach(&[("wide", &[], 9), ("narrow", &[], 2)]);
        assert_eq!(slugs(&merge_duplicates(rules, &reach)), ["narrow-1"]);
    }

    #[test]
    fn a_rule_backed_by_a_test_wins_over_one_that_is_not() {
        let mut backed = rule("b-1", LIMIT, "lib/router.rb", (3, 5));
        backed.evidence.push(RuleEvidence::Test {
            phrase: "refuses an hour".into(),
            path: "spec/router_spec.rb".into(),
        });
        let rules = vec![rule("a-1", LIMIT, "lib/router.rb", (3, 5)), backed];
        let reach = reach(&[("a", &[], 1), ("b", &[], 1)]);
        assert_eq!(slugs(&merge_duplicates(rules, &reach)), ["b-1"]);
    }

    #[test]
    fn similar_wording_without_a_common_line_is_kept_unless_nearly_identical() {
        let alike =
            "An order is accepted only if its calculated delivery time is under 60 minutes.";
        let rules = vec![
            rule("a-1", LIMIT, "lib/a.rb", (1, 2)),
            rule("b-1", alike, "lib/b.rb", (1, 2)),
        ];
        let reach = reach(&[("a", &[], 1), ("b", &[], 1)]);
        assert_eq!(merge_duplicates(rules.clone(), &reach).len(), 2);

        let same = vec![rules[0].clone(), rule("b-1", LIMIT, "lib/b.rb", (1, 2))];
        assert_eq!(merge_duplicates(same, &reach).len(), 1);
    }

    #[test]
    fn rules_that_differ_by_one_word_are_not_joined_through_a_third_one() {
        let customer =
            "An order cannot be created if the specified customer does not exist in the system.";
        let restaurant =
            "An order cannot be created if the specified restaurant does not exist in the system.";
        let rules = vec![
            rule("a-1", customer, "lib/r.rb", (1, 9)),
            rule("a-2", restaurant, "lib/r.rb", (1, 9)),
            rule("b-1", customer, "lib/r.rb", (1, 9)),
            rule("b-2", restaurant, "lib/r.rb", (1, 9)),
        ];
        let reach = reach(&[("a", &["lib/r.rb"], 1), ("b", &[], 1)]);
        assert_eq!(slugs(&merge_duplicates(rules, &reach)), ["a-1", "a-2"]);
    }

    #[test]
    fn the_copy_that_stays_keeps_what_the_others_knew() {
        let mut other = rule("b-1", LIMIT, "lib/r.rb", (1, 9));
        other.use_cases = vec!["place-an-order".into()];
        other.entities = vec!["Order".into()];
        other.evidence.push(RuleEvidence::Test {
            phrase: "refuses an hour".into(),
            path: "spec/r_spec.rb".into(),
        });
        other.confidence = Some(ConfidenceScore {
            value: 0.7,
            rationale: None,
        });
        let rules = vec![rule("a-1", LIMIT, "lib/r.rb", (1, 9)), other];
        let reach = reach(&[("a", &["lib/r.rb"], 1), ("b", &[], 1)]);

        let merged = merge_duplicates(rules, &reach);

        assert_eq!(slugs(&merged), ["a-1"]);
        assert_eq!(merged[0].use_cases, ["place-an-order"]);
        assert_eq!(merged[0].entities, ["Order"]);
        assert!(merged[0].has_corroboration());
        assert!((merged[0].confidence.as_ref().unwrap().value - 0.7).abs() < 1e-6);
    }

    #[test]
    fn a_feature_that_reads_no_file_does_not_win_the_tie() {
        let rules = vec![
            rule("empty-1", LIMIT, "lib/r.rb", (1, 9)),
            rule("reads-1", LIMIT, "lib/r.rb", (1, 9)),
        ];
        let reach = reach(&[("empty", &[], 0), ("reads", &[], 6)]);
        assert_eq!(slugs(&merge_duplicates(rules, &reach)), ["reads-1"]);
    }

    #[test]
    fn three_features_stating_one_rule_keep_one_copy() {
        let rules = vec![
            rule("a-1", LIMIT, "lib/r.rb", (1, 9)),
            rule("b-1", LIMIT, "lib/r.rb", (1, 9)),
            rule("c-1", LIMIT, "lib/r.rb", (1, 9)),
        ];
        let reach = reach(&[("a", &[], 5), ("b", &[], 2), ("c", &[], 7)]);
        assert_eq!(slugs(&merge_duplicates(rules, &reach)), ["b-1"]);
    }

    #[test]
    fn a_rule_of_an_unknown_feature_and_the_rules_of_one_feature_are_left_alone() {
        let rules = vec![
            rule("a-1", LIMIT, "lib/r.rb", (1, 9)),
            rule("a-2", LIMIT, "lib/r.rb", (1, 9)),
            rule("gone-1", LIMIT, "lib/r.rb", (1, 9)),
        ];
        let reach = reach(&[("a", &[], 1)]);
        assert_eq!(merge_duplicates(rules, &reach).len(), 3);
    }
}
