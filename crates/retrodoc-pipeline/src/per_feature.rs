//! The loop of the passes that take a feature at a time and keep a print per feature (use cases,
//! rules): each feature is planned before any call (unchanged since the last run, unreadable, or a
//! call to make), the calls go out `concurrency` at a time, the answers are taken in the order of the
//! features, and the run is saved after each answer so that a killed or failed run resumes where it
//! stopped.
//!
//! What differs between the passes stays in them ([`FeaturePass`]): what a call is and how its
//! answer is grounded, what is kept of a feature, how a run in progress is written. The features
//! pass (the earlier slices are in the prompt of the later ones) and the confidence pass (no print)
//! have loops of their own.

use std::collections::BTreeMap;

use futures_util::stream::{self, StreamExt};
use retrodoc_core::model::Feature;

use crate::error::PipelineError;
use crate::fingerprints::{feature_key, Fingerprints};
use crate::progress::Progress;

/// What is done about a feature, decided before any call.
pub(crate) enum Plan<A> {
    /// Unchanged since the last run: its saved results are kept.
    Reuse,
    /// None of its files can be read: nothing to ask.
    Unreadable,
    /// What the call needs.
    Ask(A),
}

/// One feature of a pass: its key and print, and its plan.
pub(crate) struct Unit<'f, A> {
    pub feature: &'f Feature,
    pub key: String,
    pub print: String,
    pub plan: Plan<A>,
}

/// What a pass gives the loop.
pub(crate) trait FeaturePass {
    /// What one call needs.
    type Ask;
    /// What one call gives, before it is grounded.
    type Answer;
    /// The results of the run so far.
    type Done: Default + Clone;

    /// The name of the pass in the progress line and the logs.
    const NAME: &'static str;
    /// Whether a feature that could not be read or answered keeps its saved results (and the run is
    /// saved then too), or loses them until it is answered.
    const KEEP_UNANSWERED: bool;

    /// The prints of the pass among those of every pass.
    fn prints(prints: &mut Fingerprints) -> &mut BTreeMap<String, String>;

    /// The call, run concurrently with those of other features.
    async fn ask(&self, feature: &Feature, ask: &Self::Ask) -> Self::Answer;

    /// Adds the saved results of `feature` to `done`.
    fn keep(&self, feature: &Feature, done: &mut Self::Done);

    /// Grounds the answer and adds it to `done`; `false` when it can't be used (the feature is asked
    /// again on the next run).
    ///
    /// # Errors
    ///
    /// The error of the call, which ends the pass.
    fn take(
        &self,
        feature: &Feature,
        ask: Self::Ask,
        answer: Self::Answer,
        done: &mut Self::Done,
    ) -> Result<bool, PipelineError>;

    /// Writes a run in progress, best effort.
    fn save_partial(&self, done: &Self::Done, prints: &Fingerprints);
}

/// The saved prints, and those of the pass taken out of them when they can be trusted (`trusted`:
/// the results they describe were saved too; else a print can't tell "none" from "lost").
pub(crate) fn open<P: FeaturePass>(
    mut prints: Fingerprints,
    trusted: bool,
) -> (Fingerprints, BTreeMap<String, String>) {
    let known = if trusted {
        std::mem::take(P::prints(&mut prints))
    } else {
        BTreeMap::new()
    };
    (prints, known)
}

/// Runs `units`, the plans of `features` in the same order, and returns the results with `prints`
/// holding those of the features done.
///
/// # Errors
///
/// The first error of a call: the run is saved with the features not reached as they were.
pub(crate) async fn run<P: FeaturePass>(
    pass: &P,
    features: &[Feature],
    units: Vec<Unit<'_, P::Ask>>,
    prints: &mut Fingerprints,
    known: &BTreeMap<String, String>,
    concurrency: usize,
) -> Result<P::Done, PipelineError> {
    let mut progress = Progress::new(P::NAME, features.len());
    let mut done = P::Done::default();
    // What a rerun starts from when the features from the rank `from` on are not done.
    let checkpoint = |done: &P::Done, prints: &Fingerprints, from: usize| {
        let mut done = done.clone();
        let mut prints = prints.clone();
        for feature in &features[from..] {
            pass.keep(feature, &mut done);
            let key = feature_key(feature);
            if let Some(print) = known.get(&key) {
                P::prints(&mut prints).insert(key, print.clone());
            }
        }
        pass.save_partial(&done, &prints);
    };

    // The calls go out `concurrency` at a time but the answers come back in the order of the
    // features: the result is the sequential one.
    let mut answers = stream::iter(units.into_iter().enumerate())
        .map(|(position, unit)| async move {
            let answer = match &unit.plan {
                Plan::Ask(ask) => Some(pass.ask(unit.feature, ask).await),
                Plan::Reuse | Plan::Unreadable => None,
            };
            (position, unit, answer)
        })
        .buffered(concurrency.max(1));

    while let Some((position, unit, answer)) = answers.next().await {
        let Unit {
            feature,
            key,
            print,
            plan,
        } = unit;
        match (plan, answer) {
            (Plan::Reuse, _) => {
                // A saved print without results is a remembered "none".
                tracing::info!(feature = %key, "{} unchanged, reused", P::NAME);
                pass.keep(feature, &mut done);
                P::prints(prints).insert(key, print);
                progress.skip();
            }
            (Plan::Unreadable, _) => {
                progress.begin(&key);
                tracing::warn!(feature = %feature.slug, "{} skipped: none of its files is readable", P::NAME);
                if P::KEEP_UNANSWERED {
                    pass.keep(feature, &mut done);
                }
            }
            (Plan::Ask(ask), Some(answer)) => {
                progress.begin(&key);
                match pass.take(feature, ask, answer, &mut done) {
                    Err(err) => {
                        checkpoint(&done, prints, position);
                        return Err(err);
                    }
                    Ok(true) => {
                        P::prints(prints).insert(key, print);
                    }
                    // Asked again on the next run.
                    Ok(false) if P::KEEP_UNANSWERED => pass.keep(feature, &mut done),
                    Ok(false) => continue,
                }
                // After each answer: a run killed later loses the call in flight, nothing more.
                checkpoint(&done, prints, position + 1);
            }
            (Plan::Ask(_), None) => unreachable!("a feature to ask is always asked"),
        }
    }
    Ok(done)
}
