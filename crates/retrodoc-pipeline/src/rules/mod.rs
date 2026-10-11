//! Business rules pass (ADR 0021, `issues/business_rules.md`): what is allowed, refused, limited,
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

use std::collections::BTreeMap;
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

mod grounding;
mod input;
mod prompt;
#[cfg(test)]
mod tests;

use self::grounding::{grounded_rules, RawRules};
pub use self::input::RuleInput;
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
    let saved = load_rules(repo_root);
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

    let mut answers = stream::iter(items)
        .map(|item| async move {
            let answer = match &item.plan {
                Plan::Ask { prompt, .. } => Some(
                    complete_json::<RawRules>(
                        llm,
                        RULES_SYSTEM_PROMPT,
                        prompt,
                        &format!("rules of {}", item.feature.slug),
                    )
                    .await,
                ),
                Plan::Reuse | Plan::Unreadable => None,
            };
            (item, answer)
        })
        .buffered(context.concurrency.max(1));

    let mut rules: Vec<BusinessRule> = Vec::new();
    let mut position = 0;
    while let Some((item, answer)) = answers.next().await {
        position += 1;
        let Item {
            feature,
            key,
            print,
            input,
            use_cases: own_use_cases,
            plan,
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
            Plan::Ask { cited_files, .. } => {
                progress.begin(&key);
                match answer.unwrap_or(Ok(None)) {
                    Err(err) => {
                        save_partial(
                            repo_root,
                            rules,
                            &previous,
                            &features[position - 1..],
                            &prints,
                        );
                        return Err(err);
                    }
                    // Unparseable: the saved rules stay, and it is asked again on the next run.
                    Ok(None) => rules.extend(kept(&previous, &feature.slug)),
                    Ok(Some(raw)) => {
                        rules.extend(grounded_rules(
                            raw,
                            repo_root,
                            feature,
                            &input,
                            &cited_files,
                            &own_use_cases,
                        ));
                        prints.rules.insert(key, print);
                    }
                }
            }
        }
    }
    drop(answers);

    // Rules first: a fingerprint without its rules would freeze a feature as "none".
    save_rules(repo_root, &rules)?;
    prints.save(repo_root)?;
    Ok(rules)
}

/// The saved rules of the feature `slug`.
fn kept(previous: &[BusinessRule], slug: &str) -> Vec<BusinessRule> {
    previous
        .iter()
        .filter(|r| r.belongs_to_feature(slug))
        .cloned()
        .collect()
}

/// After a failed call: the rules done so far and the previous ones of the features not reached.
fn save_partial(
    repo_root: &Path,
    mut rules: Vec<BusinessRule>,
    previous: &[BusinessRule],
    remaining: &[Feature],
    prints: &Fingerprints,
) {
    rules.extend(
        previous
            .iter()
            .filter(|r| remaining.iter().any(|f| r.belongs_to_feature(&f.slug)))
            .cloned(),
    );
    warn_on_error(save_rules(repo_root, &rules));
    warn_on_error(prints.save(repo_root));
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
        cited_files: std::collections::BTreeSet<String>,
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
    let key = format!("{}/{}", feature.domain_slug, feature.slug);
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
/// its entities, the evidence, its use cases, the brief and the model.
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
            .chain(std::iter::once(model_part(llm))),
    )
}
