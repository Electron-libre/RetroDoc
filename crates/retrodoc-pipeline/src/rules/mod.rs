//! Business rules pass (ADR 0021, `issues/done/business_rules.md`): what is allowed, refused, limited,
//! and in which states, stated in business language with its evidence. Saved as
//! `.retrodoc/cache/rules.yaml`.
//!
//! No pattern of any language or framework is written here: the LLM judges what is a rule on the
//! code it is shown. What is mechanical is the choice of that code and of the evidence around it
//! ([`RuleInput`]) and the check of the answer: a rule is kept only with a citation of code that
//! exists, tests and documents are cited by the ids of the prompt, and the confidence follows from
//! what backs the rule (code alone 0.4, with a test or a document 0.7, with both 0.85).
//!
//! One call per feature. Incremental: a feature whose text, code, evidence, use cases and model are
//! unchanged keeps its saved rules without a call; an answer that can't be parsed is retried on the
//! next run.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use futures_util::stream::{self, StreamExt};
use retrodoc_core::model::{BusinessRule, Feature, UseCase};
use retrodoc_llm::LlmProvider;

use crate::artifact::{load_yaml, save_yaml, warn_on_error, Artifact};
use crate::cache::hash_content;
use crate::chunks::Splitter;
use crate::error::PipelineError;
use crate::fingerprints::{fingerprint, model_part, Fingerprints};
use crate::progress::Progress;
use crate::response::complete_json;
use crate::roles::load_splitter;
use crate::use_cases::UseCaseContext;

mod dedup;
mod grounding;
mod input;
mod judge;
mod prompt;
#[cfg(test)]
mod tests;

use self::dedup::{merge_duplicates, Reach};
use self::grounding::{grounded_rules, RawRules};
pub use self::input::RuleInput;
use self::judge::{judge_prompt, keep_judged, RawVerdict, JUDGE_SYSTEM_PROMPT};
use self::prompt::{rules_prompt, RULES_SYSTEM_PROMPT};

/// Loads a previously saved `rules.yaml`. Missing or unreadable: `None` (first run), not an error.
#[must_use]
pub fn load_rules(repo_root: &Path) -> Option<Vec<BusinessRule>> {
    load_yaml(&Artifact::Rules.path(repo_root))
}

/// Persists `rules` as `.retrodoc/cache/rules.yaml`.
///
/// # Errors
///
/// Returns an error if the file can't be written or serialization fails.
pub fn save_rules(repo_root: &Path, rules: &[BusinessRule]) -> Result<(), PipelineError> {
    save_yaml(&Artifact::Rules.path(repo_root), rules)
}

/// The rules as each feature stated them, else (a run from before this artifact) the saved rules.
fn load_stated(repo_root: &Path) -> Option<Vec<BusinessRule>> {
    load_yaml(&Artifact::RulesStated.path(repo_root)).or_else(|| load_rules(repo_root))
}

/// Saves the rules as stated, then the same rule of several features kept once; returns the latter.
fn save_published(
    repo_root: &Path,
    stated: &[BusinessRule],
    reach: &BTreeMap<String, Reach>,
) -> Result<Vec<BusinessRule>, PipelineError> {
    save_yaml(&Artifact::RulesStated.path(repo_root), stated)?;
    let published = merge_duplicates(stated.to_vec(), reach);
    save_rules(repo_root, &published)?;
    Ok(published)
}

/// The rules of every feature, from the code and the evidence of each, saved to
/// `.retrodoc/cache/rules.yaml`. `context` is the one of the use cases pass.
///
/// # Errors
///
/// Returns an error if an LLM call fails (the features done so far are saved) or the artifact
/// can't be saved.
pub async fn build_rules(
    repo_root: &Path,
    features: &[Feature],
    use_cases: &[UseCase],
    context: &UseCaseContext,
    llm: &dyn LlmProvider,
) -> Result<Vec<BusinessRule>, PipelineError> {
    let saved = load_stated(repo_root);
    let mut prints = Fingerprints::load(repo_root);
    // Without the saved rules, a fingerprint can't tell "none" from "lost".
    let known = if saved.is_some() {
        std::mem::take(&mut prints.rules)
    } else {
        BTreeMap::new()
    };
    let previous = saved.unwrap_or_default();
    let splitter = load_splitter(repo_root);
    let mut progress = Progress::new("rules", features.len());

    let items: Vec<Item<'_>> = features
        .iter()
        .map(|feature| {
            plan_item(
                repo_root, feature, use_cases, context, &known, &splitter, llm,
            )
        })
        .collect();

    let reach = reach_of(&items);

    let mut answers = stream::iter(items)
        .map(|item| async move {
            let answer = match &item.plan {
                Plan::Ask {
                    prompt,
                    cited_files,
                } => Some(ask_rules(repo_root, llm, &item, prompt, cited_files).await),
                Plan::Reuse | Plan::Unreadable => None,
            };
            (item, answer)
        })
        .buffered(context.concurrency.max(1));

    // What a rerun starts from when the features from the rank `from` on are not done.
    let checkpoint = |rules: &[BusinessRule], prints: &Fingerprints, from: usize| {
        save_partial(
            repo_root,
            rules,
            &previous,
            &features[from..],
            prints,
            &known,
        );
    };
    let mut rules: Vec<BusinessRule> = Vec::new();
    let mut position = 0;
    while let Some((item, answer)) = answers.next().await {
        position += 1;
        let Item {
            feature,
            key,
            print,
            plan,
            ..
        } = item;
        match plan {
            Plan::Reuse => {
                tracing::info!(feature = %key, "rules unchanged, reused");
                rules.extend(kept(&previous, &feature.slug));
                prints.rules.insert(key, print);
                progress.skip();
            }
            Plan::Unreadable => {
                progress.begin(&key);
                tracing::warn!(feature = %feature.slug, "rules skipped: none of its files is readable");
                rules.extend(kept(&previous, &feature.slug));
            }
            Plan::Ask { .. } => {
                progress.begin(&key);
                match answer.unwrap_or(Ok(None)) {
                    Err(err) => {
                        checkpoint(&rules, &prints, position - 1);
                        return Err(err);
                    }
                    // Unparseable: the saved rules stay, and it is asked again on the next run.
                    Ok(None) => rules.extend(kept(&previous, &feature.slug)),
                    Ok(Some(judged)) => {
                        rules.extend(judged);
                        prints.rules.insert(key, print);
                    }
                }
                // After each answer: a run killed later loses the call in flight, nothing more.
                checkpoint(&rules, &prints, position);
            }
        }
    }
    drop(answers);

    // Rules first: a fingerprint without its rules would freeze a feature as "none".
    let published = save_published(repo_root, &rules, &reach)?;
    prints.save(repo_root)?;
    Ok(published)
}

/// The rules of one feature: the statement call, the checks of its answer, then the judgement of
/// what a business reader would ask about. `Ok(None)` when the statement answer can't be parsed; an
/// unparseable verdict keeps the rules as they are (the call is logged), so the rules are not lost.
async fn ask_rules(
    repo_root: &Path,
    llm: &dyn LlmProvider,
    item: &Item<'_>,
    prompt: &str,
    cited_files: &BTreeSet<String>,
) -> Result<Option<Vec<BusinessRule>>, PipelineError> {
    let feature = item.feature;
    let Some(raw) = complete_json::<RawRules>(
        llm,
        RULES_SYSTEM_PROMPT,
        prompt,
        &format!("rules of {}", feature.slug),
    )
    .await?
    else {
        return Ok(None);
    };
    let rules = grounded_rules(
        raw,
        repo_root,
        feature,
        &item.input,
        cited_files,
        &item.use_cases,
    );
    if rules.is_empty() {
        return Ok(Some(rules));
    }
    let Some(verdict) = complete_json::<RawVerdict>(
        llm,
        JUDGE_SYSTEM_PROMPT,
        &judge_prompt(feature, &rules),
        &format!("judgement of the rules of {}", feature.slug),
    )
    .await?
    else {
        return Ok(Some(rules));
    };
    let (rules, dropped) = keep_judged(rules, &verdict);
    for rule in &dropped {
        tracing::info!(rule = %rule.slug, statement = %rule.statement, "rule dropped by the judgement");
    }
    Ok(Some(rules))
}

/// What each feature reads, to tell which one owns the code a rule cites.
fn reach_of(items: &[Item<'_>]) -> BTreeMap<String, Reach> {
    items
        .iter()
        .map(|item| {
            let entry_files = item
                .input
                .entries
                .iter()
                .map(|(path, _)| path.to_string_lossy().into_owned())
                .collect();
            (
                item.feature.slug.clone(),
                Reach {
                    entry_files,
                    files: item.input.files.len(),
                },
            )
        })
        .collect()
}

/// The saved rules of the feature `slug`.
fn kept(previous: &[BusinessRule], slug: &str) -> Vec<BusinessRule> {
    previous
        .iter()
        .filter(|r| r.belongs_to_feature(slug))
        .cloned()
        .collect()
}

/// Best-effort save of a run in progress or failed: the rules done so far, and the previous rules
/// and fingerprints of the features not reached, so a rerun resumes where this one stopped.
fn save_partial(
    repo_root: &Path,
    done: &[BusinessRule],
    previous: &[BusinessRule],
    remaining: &[Feature],
    prints: &Fingerprints,
    known: &BTreeMap<String, String>,
) {
    let mut rules = done.to_vec();
    rules.extend(
        previous
            .iter()
            .filter(|r| remaining.iter().any(|f| r.belongs_to_feature(&f.slug)))
            .cloned(),
    );
    let mut prints = prints.clone();
    for feature in remaining {
        let key = feature_key(feature);
        if let Some(print) = known.get(&key) {
            prints.rules.insert(key, print.clone());
        }
    }
    // Not merged: it costs a quadratic comparison and the final save does it.
    warn_on_error(save_yaml(&Artifact::RulesStated.path(repo_root), &rules));
    warn_on_error(save_rules(repo_root, &rules));
    warn_on_error(prints.save(repo_root));
}

/// The key of a feature in the fingerprints of the pass.
fn feature_key(feature: &Feature) -> String {
    format!("{}/{}", feature.domain_slug, feature.slug)
}

/// One feature of the pass: what it is derived from and what is done about it.
struct Item<'a> {
    feature: &'a Feature,
    key: String,
    print: String,
    input: RuleInput,
    use_cases: Vec<&'a UseCase>,
    plan: Plan,
}

enum Plan {
    /// Unchanged since the last run: its saved rules are kept.
    Reuse,
    /// None of its files can be read: nothing to ask.
    Unreadable,
    Ask {
        prompt: String,
        cited_files: BTreeSet<String>,
    },
}

fn plan_item<'a>(
    repo_root: &Path,
    feature: &'a Feature,
    use_cases: &'a [UseCase],
    context: &UseCaseContext,
    known: &BTreeMap<String, String>,
    splitter: &Splitter,
    llm: &dyn LlmProvider,
) -> Item<'a> {
    let key = feature_key(feature);
    let input = RuleInput::new(repo_root, feature, context);
    let own: Vec<&UseCase> = use_cases
        .iter()
        .filter(|u| u.feature_slug == feature.slug)
        .collect();
    let print = rules_fingerprint(repo_root, feature, &input, &own, context, llm);
    let plan = if known.get(&key) == Some(&print) {
        Plan::Reuse
    } else {
        let (prompt, cited_files) =
            rules_prompt(repo_root, feature, &input, &own, &context.brief, splitter);
        if cited_files.is_empty() {
            Plan::Unreadable
        } else {
            Plan::Ask {
                prompt,
                cited_files,
            }
        }
    };
    Item {
        feature,
        key,
        print,
        input,
        use_cases: own,
        plan,
    }
}

/// Hash of everything the rules of a feature are derived from: its text, the content of its files,
/// its entities, the evidence, its use cases, the brief, the judgement prompt and the model.
fn rules_fingerprint(
    repo_root: &Path,
    feature: &Feature,
    input: &RuleInput,
    use_cases: &[&UseCase],
    context: &UseCaseContext,
    llm: &dyn LlmProvider,
) -> String {
    let files = input.files.iter().map(|path| {
        let hash = std::fs::read(repo_root.join(path)).map_or_else(
            |_| "unreadable".to_string(),
            |bytes| hash_content(&String::from_utf8_lossy(&bytes)),
        );
        format!("{path}\n{hash}")
    });
    let evidence = input
        .evidence
        .iter()
        .map(|e| format!("{:?} {}\n{}", e.kind, e.origin, e.text));
    let brief = context.brief.fingerprint();
    fingerprint(
        [feature.name.clone(), feature.description.clone()]
            .into_iter()
            .chain(files)
            .chain(std::iter::once(format!(
                "entities {}",
                input.entities.join(",")
            )))
            .chain(evidence)
            .chain(
                use_cases
                    .iter()
                    .map(|u| format!("use case {} {}", u.slug, u.name)),
            )
            .chain((!brief.is_empty()).then_some(brief))
            .chain(std::iter::once(format!(
                "judgement {}",
                hash_content(JUDGE_SYSTEM_PROMPT)
            )))
            .chain(std::iter::once(model_part(llm))),
    )
}
