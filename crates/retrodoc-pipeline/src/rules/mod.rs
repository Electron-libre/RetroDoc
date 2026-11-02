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
//! Incremental: a feature whose text, code, evidence, use cases, requests and model are unchanged keeps
//! its saved rules without a call; an answer that can't be parsed is retried on the next run. The
//! print of a feature has two parts, the statement (with the question on the kind of feature) and the
//! judgement: when only the judgement part changed, the rules the last run stated are judged again,
//! with no statement call (`specs/rerun.md` RER-11).
//!
//! What the LLM set aside, the user can put back in `[rules]` of `retrodoc.toml` (`specs/rules-set-aside.md`).

use std::collections::BTreeMap;
use std::path::Path;

use retrodoc_core::config::RulesConfig;
use retrodoc_core::model::{BusinessRule, Feature, UseCase};
use retrodoc_llm::LlmProvider;

use crate::artifact::{load_yaml, save_yaml, warn_on_error, Artifact};
use crate::chunks::Splitter;
use crate::error::PipelineError;
use crate::fingerprints::{feature_key, file_part, fingerprint, model_part, Fingerprints};
use crate::per_feature::{self, FeaturePass, Plan, Unit};
use crate::roles::load_splitter;
use crate::text::feature_line;
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
pub(crate) use self::dedup::code as cited_code;
use self::dedup::{merge_duplicates, Reach};
pub use self::input::RuleInput;
pub(crate) use self::judge::judge_prompt;
use self::judge::{keep_judged, JUDGEMENT, KIND};
pub use self::judge::{JUDGE_SYSTEM_PROMPT, NOISE};
use self::prompt::{rules_prompt, STATEMENT};
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
/// `.retrodoc/cache/rules.yaml`. `context` is the one of the use cases pass; `concurrency` features
/// are asked at once, as there.
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
    concurrency: usize,
) -> Result<Vec<BusinessRule>, PipelineError> {
    let saved = load_stated(repo_root);
    // Without the saved rules, a fingerprint can't tell "none" from "lost"; nor can it, without the
    // file of what was set aside (a run from before it), tell a feature about tests from the rest.
    let (mut prints, known) = per_feature::open::<RulesPass>(
        Fingerprints::load(repo_root),
        saved.is_some() && Artifact::RulesSetAside.path(repo_root).exists(),
    );
    warn_unknown_features(&context.put_back.keep_features, features);
    let splitter = load_splitter(repo_root);
    let previous = saved.unwrap_or_default();
    let previous_aside = load_set_aside(repo_root).unwrap_or_default();
    let judgement = judgement_print(llm);
    let (units, reach): (Vec<_>, BTreeMap<_, _>) = features
        .iter()
        .map(|feature| {
            let last = Last {
                prints: &known,
                rules: &previous,
                aside: &previous_aside,
                judgement: &judgement,
            };
            plan_unit(
                repo_root, feature, use_cases, context, &last, &splitter, llm,
            )
        })
        .unzip();
    let pass = RulesPass {
        repo_root,
        llm,
        context,
        splitter: &splitter,
        previous,
        previous_aside,
    };
    let Stated { rules, mut aside } =
        per_feature::run(&pass, units, &mut prints, &known, concurrency).await?;

    warn_unknown_rules(&context.put_back.keep_rules, &aside, &rules);
    aside.forget_features(&context.put_back.keep_features);
    // Rules first: a fingerprint without its rules would freeze a feature as "none".
    let published = save_published(repo_root, &rules, &aside, &context.put_back, &reach)?;
    save_yaml(&Artifact::RulesSetAside.path(repo_root), &aside)?;
    prints.save(repo_root)?;
    Ok(published)
}

/// The rules pass as the loop of the passes per feature runs it.
struct RulesPass<'a> {
    repo_root: &'a Path,
    llm: &'a dyn LlmProvider,
    context: &'a UseCaseContext,
    splitter: &'a Splitter,
    /// The rules as stated by the last run.
    previous: Vec<BusinessRule>,
    /// What the last run set aside.
    previous_aside: SetAside,
}

impl<'a> FeaturePass for RulesPass<'a> {
    type Ask = Asking<'a>;
    type Answer = Result<Option<Asked>, PipelineError>;
    type Done = Stated;

    const NAME: &'static str = "rules";
    // An unparseable or unreadable feature keeps its saved rules until it is answered.
    const KEEP_UNANSWERED: bool = true;

    fn prints(prints: &mut Fingerprints) -> &mut BTreeMap<String, String> {
        &mut prints.rules
    }

    async fn ask(&self, feature: &Feature, asking: &Asking<'a>) -> Self::Answer {
        match asking {
            Asking::State {
                item,
                prompt,
                cited_files,
            } => {
                ask_rules(
                    self.repo_root,
                    self.llm,
                    item,
                    prompt,
                    cited_files,
                    self.context,
                    self.splitter,
                )
                .await
            }
            Asking::Judge(rules) => judged(self.llm, feature, rules.clone()).await.map(Some),
        }
    }

    fn keep(&self, feature: &Feature, done: &mut Stated) {
        done.rules.extend(kept(&self.previous, &feature.slug));
        done.aside
            .extend(self.previous_aside.of_feature(&feature.slug));
    }

    fn take(
        &self,
        _feature: &Feature,
        _asking: Asking<'a>,
        answer: Self::Answer,
        done: &mut Stated,
    ) -> Result<bool, PipelineError> {
        let Some(asked) = answer? else {
            return Ok(false);
        };
        done.rules.extend(asked.rules);
        done.aside.extend(asked.aside);
        Ok(true)
    }

    /// Not merged: it costs a quadratic comparison and the final save does it.
    fn save_partial(&self, done: &Stated, prints: &Fingerprints) {
        let put_back = &self.context.put_back;
        let mut aside = done.aside.clone();
        aside.forget_features(&put_back.keep_features);
        warn_on_error(save_yaml(
            &Artifact::RulesStated.path(self.repo_root),
            &done.rules,
        ));
        warn_on_error(save_rules(
            self.repo_root,
            &with_put_back(&done.rules, &aside, put_back),
        ));
        warn_on_error(save_yaml(
            &Artifact::RulesSetAside.path(self.repo_root),
            &aside,
        ));
        warn_on_error(prints.save(self.repo_root));
    }
}

/// The rules as stated in a run and what it set aside.
#[derive(Clone, Default)]
struct Stated {
    rules: Vec<BusinessRule>,
    aside: SetAside,
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
    context: &UseCaseContext,
    splitter: &Splitter,
) -> Result<Option<Asked>, PipelineError> {
    let feature = item.feature;
    if item.put_back {
        tracing::info!(feature = %feature.slug, "feature put back by hand, not asked whether it is about tests");
    } else if let Some(kind) = KIND
        .ask(
            llm,
            &feature_line(feature),
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
        stated_rules(repo_root, llm, item, prompt, cited_files, context, splitter).await?
    else {
        return Ok(None);
    };
    judged(llm, feature, rules).await.map(Some)
}

/// The `rules` of `feature` judged: those kept, and those dropped set aside with their class. No call
/// for no rule; an unparseable verdict keeps them all.
async fn judged(
    llm: &dyn LlmProvider,
    feature: &Feature,
    rules: Vec<BusinessRule>,
) -> Result<Asked, PipelineError> {
    if rules.is_empty() {
        return Ok(Asked::of(rules));
    }
    let Some((rules, dropped)) = judge_rules(llm, feature, &rules).await? else {
        return Ok(Asked::of(rules));
    };
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
    Ok(asked)
}

/// The rules of `feature` judged under the pass's request ([`JUDGE_SYSTEM_PROMPT`]): those kept and
/// those dropped with their class; `None` when the answer can't be read, the rules then being kept as
/// stated.
async fn judge_rules(
    llm: &dyn LlmProvider,
    feature: &Feature,
    rules: &[BusinessRule],
) -> Result<Option<Judged>, PipelineError> {
    Ok(JUDGEMENT
        .ask(
            llm,
            &judge_prompt(feature, rules),
            &format!("judgement of the rules of {}", feature.slug),
        )
        .await?
        .map(|verdict| keep_judged(rules.to_vec(), &verdict)))
}

/// The rules kept and those dropped with their class.
pub(crate) type Judged = (Vec<BusinessRule>, Vec<(BusinessRule, String)>);

/// The judgement of `rules` (those of the feature `slug`) asked with `system` and `user`: the pass's
/// prompts, or a variant a replay measures.
///
/// # Errors
///
/// Returns an error if the LLM call fails.
pub(crate) async fn judge_rules_with(
    llm: &dyn LlmProvider,
    system: &str,
    user: &str,
    slug: &str,
    rules: &[BusinessRule],
) -> Result<Option<Judged>, PipelineError> {
    Ok(JUDGEMENT
        .ask_with_system(
            llm,
            system,
            user,
            &format!("judgement of the rules of {slug}"),
        )
        .await?
        .map(|verdict| keep_judged(rules.to_vec(), &verdict)))
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

/// The saved rules of the feature `slug`.
pub(crate) fn kept(previous: &[BusinessRule], slug: &str) -> Vec<BusinessRule> {
    previous
        .iter()
        .filter(|r| r.belongs_to_feature(slug))
        .cloned()
        .collect()
}

/// One feature of the pass: what it is derived from.
struct Item<'a> {
    feature: &'a Feature,
    input: RuleInput,
    use_cases: Vec<&'a UseCase>,
    /// Listed in `keep_features`: not asked whether it is about tests.
    put_back: bool,
}

/// The call of a feature.
enum Asking<'a> {
    /// Its rules stated from its statement prompt, then judged.
    State {
        item: Item<'a>,
        prompt: String,
        cited_files: Vec<String>,
    },
    /// The rules the last run stated, judged again.
    Judge(Vec<BusinessRule>),
}

/// What the last run left: the prints of the pass, the rules as stated, what was set aside; and the
/// judgement part of the prints of this run.
struct Last<'l> {
    prints: &'l BTreeMap<String, String>,
    rules: &'l [BusinessRule],
    aside: &'l SetAside,
    judgement: &'l str,
}

/// The rules of `slug` as the last run stated them before its judgement: those it kept and those it
/// set aside, in the order of their numbers; `None` when a rule set aside was saved without its whole
/// rule (a run from before it was kept).
fn stated_before_judgement(last: &Last<'_>, slug: &str) -> Option<Vec<BusinessRule>> {
    let mut rules = kept(last.rules, slug);
    for aside in last.aside.rules.iter().filter(|r| r.feature == slug) {
        rules.push(aside.rule.clone()?);
    }
    rules.sort_by_key(|rule| {
        rule.slug
            .rsplit('-')
            .next()
            .and_then(|n| n.parse::<u64>().ok())
    });
    Some(rules)
}

/// The plan of a feature, and what it reads (to tell which feature owns the code a rule cites).
fn plan_unit<'a>(
    repo_root: &Path,
    feature: &'a Feature,
    use_cases: &'a [UseCase],
    context: &UseCaseContext,
    last: &Last<'_>,
    splitter: &Splitter,
    llm: &dyn LlmProvider,
) -> (Unit<'a, Asking<'a>>, (String, Reach)) {
    let key = feature_key(feature);
    let input = RuleInput::new(repo_root, feature, context);
    let reach = Reach {
        entry_files: input
            .entries
            .iter()
            .map(|(path, _)| path.to_string_lossy().into_owned())
            .collect(),
        files: input.files.len(),
    };
    let own: Vec<&UseCase> = use_cases
        .iter()
        .filter(|u| u.feature_slug == feature.slug)
        .collect();
    let put_back = context.put_back.keep_features.contains(&feature.slug);
    let statement = statement_print(feature, &input, &own, context, llm, put_back);
    let print = format!("{statement} {}", last.judgement);
    let saved = last.prints.get(&key);
    let judged_again = saved
        .and_then(|saved| saved.split_once(' '))
        .is_some_and(|(saved, _)| saved == statement)
        .then(|| stated_before_judgement(last, &feature.slug))
        .flatten();
    let plan = if saved == Some(&print) {
        Plan::Reuse
    } else if let Some(rules) = judged_again {
        // A feature about tests, or one with no rule, has nothing to judge.
        if rules.is_empty() {
            Plan::Reuse
        } else {
            Plan::Ask(Asking::Judge(rules))
        }
    } else {
        let (prompt, cited_files) = rules_prompt(
            &context.walked,
            feature,
            &input,
            &own,
            &context.brief,
            splitter,
        );
        if cited_files.is_empty() {
            Plan::Unreadable
        } else {
            Plan::Ask(Asking::State {
                item: Item {
                    feature,
                    input,
                    use_cases: own,
                    put_back,
                },
                prompt,
                cited_files,
            })
        }
    };
    (
        Unit {
            feature,
            key,
            print,
            plan,
        },
        (feature.slug.clone(), reach),
    )
}

/// The statement part of the print of a feature: its text, the content of its files, its entities,
/// the evidence, its use cases, the brief, the requests of the statement and of the question on the
/// kind of feature, the model, and whether it is put back by hand (only when it is, so that the other
/// features keep their print).
fn statement_print(
    feature: &Feature,
    input: &RuleInput,
    use_cases: &[&UseCase],
    context: &UseCaseContext,
    llm: &dyn LlmProvider,
    put_back: bool,
) -> String {
    let files = input
        .files
        .iter()
        .map(|path| file_part(&context.walked, path));
    let evidence = input
        .evidence
        .iter()
        .map(|e| format!("{:?} {}\n{}", e.kind, e.origin, e.text));
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
            .chain(context.brief.print_part())
            .chain([STATEMENT.part().to_string(), KIND.part().to_string()])
            .chain(std::iter::once(model_part(llm)))
            .chain(put_back.then(|| "put back by hand".to_string())),
    )
}

/// The judgement part of the print of a feature: its request and the model, the same for every
/// feature of a run.
fn judgement_print(llm: &dyn LlmProvider) -> String {
    fingerprint([JUDGEMENT.part().to_string(), model_part(llm)])
}
