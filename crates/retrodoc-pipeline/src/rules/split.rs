//! The statement call of a feature, asked again on smaller parts of its code when the server cuts
//! the answer at its completion limit (`issues/done/rules_answer_truncated_on_large_features.md`).
//!
//! A feature with many rules writes an answer longer than the limit; the cut JSON can't be read
//! and asking again would be cut at the same place. The files shown are then halved, in the order
//! of the prompt, and each half is asked on its own (and halved again if it is cut too), down to
//! one file. The parts keep the same tests, documents and use cases; their rules are grounded
//! against their own file numbers, merged in order, the repeated statements kept once, and
//! numbered `<feature-slug>-<n>` as if one call had written them. Any part that can't be read
//! leaves the whole feature to the next run: a feature missing a part is never remembered as done.

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, VecDeque};
use std::path::Path;

use retrodoc_core::model::BusinessRule;
use retrodoc_llm::LlmProvider;

use super::grounding::{grounded_rules, RawRules};
use super::prompt::{rules_prompt, RULES_SYSTEM_PROMPT};
use super::Item;
use crate::chunks::Splitter;
use crate::error::PipelineError;
use crate::naming::normalize;
use crate::response::{complete_json_or_cut, Answer};
use crate::use_cases::UseCaseContext;

/// The rules `item`'s feature states, grounded; `None` when an answer can't be read.
pub(super) async fn stated_rules(
    repo_root: &Path,
    llm: &dyn LlmProvider,
    item: &Item<'_>,
    prompt: &str,
    cited_files: &[String],
    context: &UseCaseContext,
    splitter: &Splitter,
) -> Result<Option<Vec<BusinessRule>>, PipelineError> {
    let feature = item.feature;
    let mut parts: VecDeque<(String, Vec<String>)> =
        VecDeque::from([(prompt.to_string(), cited_files.to_vec())]);
    let mut asked = 0;
    let mut rules: Vec<BusinessRule> = Vec::new();
    while let Some((prompt, files)) = parts.pop_front() {
        asked += 1;
        let answer = complete_json_or_cut::<RawRules>(
            llm,
            RULES_SYSTEM_PROMPT,
            &prompt,
            &format!("rules of {}", feature.slug),
            true,
        )
        .await?;
        match answer {
            Answer::Read(raw) => {
                let stated = raw.rules.len();
                let grounded = grounded_rules(
                    raw,
                    repo_root,
                    feature,
                    &item.input,
                    &files,
                    &item.use_cases,
                );
                tracing::info!(feature = %feature.slug, stated, grounded = grounded.len(), "rules answer read");
                rules.extend(grounded);
            }
            Answer::Cut if files.len() > 1 => {
                let (first, second) = files.split_at(files.len() / 2);
                tracing::info!(
                    feature = %feature.slug,
                    files = files.len(),
                    "rules answer cut by the completion limit, asked again on two halves of the files"
                );
                for half in [second, first] {
                    let mut input = item.input.clone();
                    input.files = half.to_vec();
                    let (prompt, included) = rules_prompt(
                        &context.walked,
                        feature,
                        &input,
                        &item.use_cases,
                        &context.brief,
                        splitter,
                    );
                    if included.is_empty() {
                        tracing::warn!(feature = %feature.slug, "rules skipped: a half of the files can't be shown");
                        return Ok(None);
                    }
                    parts.push_front((prompt, included));
                }
            }
            Answer::Cut | Answer::Unreadable => return Ok(None),
        }
    }
    if asked > 1 {
        rules = merged(rules, &feature.slug);
    }
    Ok(Some(rules))
}

/// `rules` of the parts in order, a repeated statement kept once (the copy with the higher
/// confidence, at the place of the first), numbered from 1.
pub(super) fn merged(rules: Vec<BusinessRule>, feature_slug: &str) -> Vec<BusinessRule> {
    let mut kept: Vec<BusinessRule> = Vec::new();
    let mut places: BTreeMap<String, usize> = BTreeMap::new();
    let confidence = |rule: &BusinessRule| rule.confidence.as_ref().map_or(0.0, |c| c.value);
    for rule in rules {
        match places.entry(normalize(&rule.statement)) {
            Entry::Vacant(place) => {
                place.insert(kept.len());
                kept.push(rule);
            }
            Entry::Occupied(place) => {
                let first = &mut kept[*place.get()];
                if confidence(&rule) > confidence(first) {
                    *first = rule;
                }
            }
        }
    }
    for (n, rule) in kept.iter_mut().enumerate() {
        rule.slug = format!("{feature_slug}-{}", n + 1);
    }
    kept
}
