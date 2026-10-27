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
//! One statement call per feature, asked again on two halves of its files (recursively, down to one
//! file) when the server cuts the answer at its completion limit (`split.rs`).
//!
//! Incremental: a feature whose text, code, evidence, use cases and model are unchanged keeps its
//! saved rules without a call; an answer that can't be parsed is retried on the next run.
//!
//! What the LLM set aside, the user can put back in `[rules]` of `retrodoc.toml` (`specs/rules-set-aside.md`).

use std::collections::BTreeMap;
use std::path::Path;

use futures_util::stream::{self, StreamExt};
use retrodoc_core::config::RulesConfig;
use retrodoc_core::model::{BusinessRule, Feature, UseCase};
use retrodoc_llm::LlmProvider;

use crate::artifact::{load_yaml, save_yaml, warn_on_error, Artifact};
use crate::brief::ProductBrief;
use crate::cache::hash_content;
use crate::chunks::Splitter;
use crate::error::PipelineError;
use crate::fingerprints::{fingerprint, model_part, Fingerprints};
use crate::progress::Progress;
use crate::response::complete_json;
use crate::roles::load_splitter;
use crate::use_cases::UseCaseContext;

mod aside;
mod dedup;
mod grounding;
mod input;
mod judge;
mod prompt;
mod split;
#[cfg(test)]
mod tests;

pub use self::aside::{AsideFeature, AsideRule, SetAside};
use self::dedup::{merge_duplicates, Reach};
pub use self::input::RuleInput;
use self::judge::{
    feature_prompt, judge_prompt, keep_judged, RawFeatureKind, RawVerdict, FEATURE_SYSTEM_PROMPT,
    JUDGE_SYSTEM_PROMPT,
};
use self::prompt::{rules_prompt, RULES_SYSTEM_PROMPT};
use self::split::stated_rules;

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

/// What the rules pass set aside on a judgement of the LLM, to check the doubtful cases. Missing or
/// unreadable: `None`.
#[must_use]
pub fn load_set_aside(repo_root: &Path) -> Option<SetAside> {
    load_yaml(&Artifact::RulesSetAside.path(repo_root))
}

/// The rules as each feature stated them, else (a run from before this artifact) the saved rules.
fn load_stated(repo_root: &Path) -> Option<Vec<BusinessRule>> {
    load_yaml(&Artifact::RulesStated.path(repo_root)).or_else(|| load_rules(repo_root))
}

/// Saves the rules as stated, then publishes them with the rules `put_back` by hand, the same rule of
/// several features kept once; returns the latter.
fn save_published(
    repo_root: &Path,
    stated: &[BusinessRule],
    aside: &SetAside,
    put_back: &RulesConfig,
    reach: &BTreeMap<String, Reach>,
) -> Result<Vec<BusinessRule>, PipelineError> {
    save_yaml(&Artifact::RulesStated.path(repo_root), stated)?;
    let published = merge_duplicates(with_put_back(stated, aside, put_back), reach);
    save_rules(repo_root, &published)?;
    Ok(published)
}

/// The rules `stated` and those of `aside` that `keep_rules` of `put_back` lists, each among the rules
/// of its feature by its number. The rules put back are never saved as stated: a run takes them
/// from what was set aside, so taking one off the list sets it aside again.
fn with_put_back(
    stated: &[BusinessRule],
    aside: &SetAside,
    put_back: &RulesConfig,
) -> Vec<BusinessRule> {
    let mut all = stated.to_vec();
    for entry in &aside.rules {
        let Some(rule) = entry
            .rule
            .as_ref()
            .filter(|_| put_back.keep_rules.contains(&entry.slug))
        else {
            continue;
        };
        let feature = entry.feature.as_str();
        let number = rule.number_in(feature);
        let at = all
            .iter()
            .rposition(|r| r.number_in(feature).is_some_and(|n| Some(n) < number))
            .map(|i| i + 1)
            .or_else(|| all.iter().position(|r| r.belongs_to_feature(feature)))
            .unwrap_or(all.len());
        all.insert(at, rule.clone());
    }
    all
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
    // Without the saved rules, a fingerprint can't tell "none" from "lost"; nor can it, without the
    // file of what was set aside (a run from before it), tell a feature about tests from the rest.
    let known = if saved.is_some() && Artifact::RulesSetAside.path(repo_root).exists() {
        std::mem::take(&mut prints.rules)
    } else {
        BTreeMap::new()
    };
    let previous = saved.unwrap_or_default();
    let previous_aside = load_set_aside(repo_root).unwrap_or_default();
    warn_unknown_features(&context.put_back.keep_features, features);
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

    let (brief, splitter) = (&context.brief, &splitter);
    let mut answers = stream::iter(items)
        .map(|item| async move {
            let answer = answer_for(repo_root, llm, &item, brief, splitter).await;
            (item, answer)
        })
        .buffered(context.concurrency.max(1));

    // What a rerun starts from when the features from the rank `from` on are not done.
    let checkpoint =
        |rules: &[BusinessRule], aside: &SetAside, prints: &Fingerprints, from: usize| {
            save_partial(
                repo_root,
                Snapshot { rules, aside },
                Snapshot {
                    rules: &previous,
                    aside: &previous_aside,
                },
                &features[from..],
                prints,
                &known,
                &context.put_back,
            );
        };
    let mut rules: Vec<BusinessRule> = Vec::new();
    let mut aside = SetAside::default();
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
                aside.extend(previous_aside.of_feature(&feature.slug));
                prints.rules.insert(key, print);
                progress.skip();
            }
            Plan::Unreadable => {
                progress.begin(&key);
                tracing::warn!(feature = %feature.slug, "rules skipped: none of its files is readable");
                rules.extend(kept(&previous, &feature.slug));
                aside.extend(previous_aside.of_feature(&feature.slug));
            }
            Plan::Ask { .. } => {
                progress.begin(&key);
                match answer.unwrap_or(Ok(None)) {
                    Err(err) => {
                        checkpoint(&rules, &aside, &prints, position - 1);
                        return Err(err);
                    }
                    // Unparseable: the saved rules stay, and it is asked again on the next run.
                    Ok(None) => {
                        rules.extend(kept(&previous, &feature.slug));
                        aside.extend(previous_aside.of_feature(&feature.slug));
                    }
                    Ok(Some(asked)) => {
                        rules.extend(asked.rules);
                        aside.extend(asked.aside);
                        prints.rules.insert(key, print);
                    }
                }
                // After each answer: a run killed later loses the call in flight, nothing more.
                checkpoint(&rules, &aside, &prints, position);
            }
        }
    }
    drop(answers);

    warn_unknown_rules(&context.put_back.keep_rules, &aside, &rules);
    aside.forget_features(&context.put_back.keep_features);
    // Rules first: a fingerprint without its rules would freeze a feature as "none".
    let published = save_published(repo_root, &rules, &aside, &context.put_back, &reach)?;
    save_yaml(&Artifact::RulesSetAside.path(repo_root), &aside)?;
    prints.save(repo_root)?;
    Ok(published)
}

/// The answer for `item`, when its plan is to ask.
async fn answer_for(
    repo_root: &Path,
    llm: &dyn LlmProvider,
    item: &Item<'_>,
    brief: &ProductBrief,
    splitter: &Splitter,
) -> Option<Result<Option<Asked>, PipelineError>> {
    match &item.plan {
        Plan::Ask {
            prompt,
            cited_files,
        } => Some(ask_rules(repo_root, llm, item, prompt, cited_files, brief, splitter).await),
        Plan::Reuse | Plan::Unreadable => None,
    }
}

/// The rules of one feature: a question on whether it is about tests (then it has no rules), the
/// statement call, the checks of its answer, then the judgement of what a business reader would ask
/// about. `Ok(None)` when the statement answer can't be parsed; an unparseable verdict keeps the
/// rules as they are (the call is logged), so the rules are not lost.
async fn ask_rules(
    repo_root: &Path,
    llm: &dyn LlmProvider,
    item: &Item<'_>,
    prompt: &str,
    cited_files: &[String],
    brief: &ProductBrief,
    splitter: &Splitter,
) -> Result<Option<Asked>, PipelineError> {
    let feature = item.feature;
    if item.put_back {
        tracing::info!(feature = %feature.slug, "feature put back by hand, not asked whether it is about tests");
    } else if let Some(kind) = complete_json::<RawFeatureKind>(
        llm,
        FEATURE_SYSTEM_PROMPT,
        &feature_prompt(feature),
        &format!("kind of {}", feature.slug),
    )
    .await?
    {
        if kind.is_about_tests() {
            tracing::info!(feature = %feature.slug, "feature about tests, no rules asked");
            return Ok(Some(Asked {
                rules: Vec::new(),
                aside: SetAside {
                    features: vec![AsideFeature {
                        slug: feature.slug.clone(),
                        name: feature.name.clone(),
                        description: feature.description.clone(),
                        reason: "describes how the application is tested, not what it does for \
                                 its users"
                            .to_string(),
                    }],
                    rules: Vec::new(),
                },
            }));
        }
    }
    let Some(rules) =
        stated_rules(repo_root, llm, item, prompt, cited_files, brief, splitter).await?
    else {
        return Ok(None);
    };
    if rules.is_empty() {
        return Ok(Some(Asked::of(rules)));
    }
    let Some(verdict) = complete_json::<RawVerdict>(
        llm,
        JUDGE_SYSTEM_PROMPT,
        &judge_prompt(feature, &rules),
        &format!("judgement of the rules of {}", feature.slug),
    )
    .await?
    else {
        return Ok(Some(Asked::of(rules)));
    };
    let (rules, dropped) = keep_judged(rules, &verdict);
    let mut asked = Asked::of(rules);
    for (rule, class) in dropped {
        tracing::info!(rule = %rule.slug, %class, statement = %rule.statement, "rule dropped by the judgement");
        asked.aside.rules.push(AsideRule {
            slug: rule.slug.clone(),
            feature: feature.slug.clone(),
            statement: rule.statement.clone(),
            class,
            rule: Some(rule),
        });
    }
    Ok(Some(asked))
}

/// Warns about each slug of `keep_features` that names none of `features`.
fn warn_unknown_features(keep_features: &[String], features: &[Feature]) {
    for slug in keep_features {
        if !features.iter().any(|f| &f.slug == slug) {
            tracing::warn!(
                "`{slug}` of `keep_features` in `[rules]` of retrodoc.toml names no feature: \
                 nothing to put back (was it renamed?)"
            );
        }
    }
}

/// Warns about each slug of `keep_rules` that names no rule, set aside or `stated`, and about one set
/// aside by a run that did not keep the rule whole.
fn warn_unknown_rules(keep_rules: &[String], aside: &SetAside, stated: &[BusinessRule]) {
    for slug in keep_rules.iter().collect::<std::collections::BTreeSet<_>>() {
        match aside.rules.iter().find(|r| &r.slug == slug) {
            Some(AsideRule { rule: Some(_), .. }) => {}
            Some(_) => tracing::warn!(
                "`{slug}` of `keep_rules` in `[rules]` of retrodoc.toml was set aside by an older run \
                 that did not keep it whole: it comes back when its feature is stated again"
            ),
            None if stated.iter().any(|r| &r.slug == slug) => {}
            None => tracing::warn!(
                "`{slug}` of `keep_rules` in `[rules]` of retrodoc.toml names no rule: nothing to \
                 put back (was its feature stated again?)"
            ),
        }
    }
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

/// The answer for one feature: its rules, and what was set aside on the way.
struct Asked {
    rules: Vec<BusinessRule>,
    aside: SetAside,
}

impl Asked {
    fn of(rules: Vec<BusinessRule>) -> Self {
        Self {
            rules,
            aside: SetAside::default(),
        }
    }
}

/// The rules and the set-aside entries of a run, as saved.
#[derive(Clone, Copy)]
struct Snapshot<'a> {
    rules: &'a [BusinessRule],
    aside: &'a SetAside,
}

/// The saved rules of the feature `slug`.
fn kept(previous: &[BusinessRule], slug: &str) -> Vec<BusinessRule> {
    previous
        .iter()
        .filter(|r| r.belongs_to_feature(slug))
        .cloned()
        .collect()
}

/// Best-effort save of a run in progress or failed: the rules done so far, and the previous rules,
/// set-aside entries and fingerprints of the features not reached, so a rerun resumes where this one
/// stopped.
fn save_partial(
    repo_root: &Path,
    done: Snapshot<'_>,
    previous: Snapshot<'_>,
    remaining: &[Feature],
    prints: &Fingerprints,
    known: &BTreeMap<String, String>,
    put_back: &RulesConfig,
) {
    let mut rules = done.rules.to_vec();
    rules.extend(
        previous
            .rules
            .iter()
            .filter(|r| remaining.iter().any(|f| r.belongs_to_feature(&f.slug)))
            .cloned(),
    );
    let mut aside = done.aside.clone();
    for feature in remaining {
        aside.extend(previous.aside.of_feature(&feature.slug));
    }
    aside.forget_features(&put_back.keep_features);
    let mut prints = prints.clone();
    for feature in remaining {
        let key = feature_key(feature);
        if let Some(print) = known.get(&key) {
            prints.rules.insert(key, print.clone());
        }
    }
    // Not merged: it costs a quadratic comparison and the final save does it.
    warn_on_error(save_yaml(&Artifact::RulesStated.path(repo_root), &rules));
    warn_on_error(save_rules(
        repo_root,
        &with_put_back(&rules, &aside, put_back),
    ));
    warn_on_error(save_yaml(&Artifact::RulesSetAside.path(repo_root), &aside));
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
    /// Listed in `keep_features`: not asked whether it is about tests.
    put_back: bool,
    plan: Plan,
}

enum Plan {
    /// Unchanged since the last run: its saved rules are kept.
    Reuse,
    /// None of its files can be read: nothing to ask.
    Unreadable,
    Ask {
        prompt: String,
        cited_files: Vec<String>,
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
    let put_back = context.put_back.keep_features.contains(&feature.slug);
    let print = rules_fingerprint(repo_root, feature, &input, &own, context, llm, put_back);
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
        put_back,
        plan,
    }
}

/// Hash of everything the rules of a feature are derived from: its text, the content of its files,
/// its entities, the evidence, its use cases, the brief, the three prompts (statement, kind, judgement), the model,
/// and whether it is put back by hand (only when it is, so that the other features keep their print).
fn rules_fingerprint(
    repo_root: &Path,
    feature: &Feature,
    input: &RuleInput,
    use_cases: &[&UseCase],
    context: &UseCaseContext,
    llm: &dyn LlmProvider,
    put_back: bool,
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
                hash_content(&format!(
                    "{RULES_SYSTEM_PROMPT}\0{FEATURE_SYSTEM_PROMPT}\0{JUDGE_SYSTEM_PROMPT}"
                ))
            )))
            .chain(std::iter::once(model_part(llm)))
            .chain(put_back.then(|| "put back by hand".to_string())),
    )
}
