//! Compares the mechanical checks of the confidence pass with the scores a model gave, on saved runs and
//! with no LLM call (`issues/deterministic_confidence_checks.md`, step 3).
//!
//! Usage: `cargo run -q -p retrodoc-pipeline --example confidence_agreement -- <run dir>...`
//!
//! A run dir is a checkout of the repository with its `.retrodoc/cache/` from a `generate` made before the
//! checks existed (or with `--confidence-sample` covering every use case), so the saved `use-cases.yaml`
//! scores are the model's. Only the use-case score is saved, not the verdict of each step, so the
//! comparison is per use case: for those the checks settle alone, the mechanical score against the saved
//! one (same side of the 50% flag, mean gap), plus how the steps are judged. One row per run, Markdown.

#![allow(clippy::cast_precision_loss)] // counts of use cases and steps: far below 2^52

use std::fmt::Write as _;
use std::path::Path;

use retrodoc_pipeline::{load_use_cases, mechanical_score, Glossary, StepTally};

/// The flag threshold of the report and the MCP server.
const FLAG: f32 = 0.5;

#[derive(Debug, Default, PartialEq)]
struct Agreement {
    use_cases: usize,
    scored_by_model: usize,
    settled: usize,
    same_side: usize,
    gap_sum: f32,
    steps: StepTally,
    disagreements: Vec<String>,
}

impl Agreement {
    fn add(&mut self, name: &str, tally: StepTally, mechanical: Option<f32>, model: Option<f32>) {
        self.use_cases += 1;
        self.steps.passing += tally.passing;
        self.steps.ungrounded += tally.ungrounded;
        self.steps.doubtful += tally.doubtful;
        self.steps.broken += tally.broken;
        let Some(model) = model else { return };
        self.scored_by_model += 1;
        let Some(mechanical) = mechanical else { return };
        self.settled += 1;
        self.gap_sum += (mechanical - model).abs();
        if (mechanical >= FLAG) == (model >= FLAG) {
            self.same_side += 1;
        } else {
            self.disagreements
                .push(format!("{name}: checks {mechanical:.2}, model {model:.2}"));
        }
    }

    fn table_row(&self, run: &str) -> String {
        let steps =
            (self.steps.passing + self.steps.ungrounded + self.steps.doubtful + self.steps.broken)
                .max(1);
        let pct = |n: usize, d: usize| 100.0 * n as f32 / d.max(1) as f32;
        let mut row = format!(
            "| {run} | {} | {:.0}% | {:.0}% | {:.0}% | {:.0}% | {:.0}% | {:.2} |",
            self.use_cases,
            pct(self.settled, self.scored_by_model),
            pct(self.same_side, self.settled),
            pct(self.steps.passing, steps),
            pct(self.steps.doubtful, steps),
            pct(self.steps.broken, steps),
            self.gap_sum / self.settled.max(1) as f32,
        );
        for d in &self.disagreements {
            let _ = write!(row, "\n|   | {d} |||||||");
        }
        row
    }
}

fn measure(run: &Path) -> Option<Agreement> {
    let use_cases = load_use_cases(run)?;
    let entities: Vec<String> = Glossary::load(run)
        .map(|g| g.entities().map(|(_, e)| e.name.clone()).collect())
        .unwrap_or_default();
    let mut agreement = Agreement::default();
    for use_case in &use_cases {
        let (tally, score) = mechanical_score(run, use_case, &entities);
        agreement.add(
            &use_case.slug,
            tally,
            score.map(|s| s.value),
            use_case.confidence.as_ref().map(|c| c.value),
        );
    }
    Some(agreement)
}

fn main() {
    let runs: Vec<String> = std::env::args().skip(1).collect();
    if runs.is_empty() {
        eprintln!("usage: confidence_agreement <run dir>...");
        std::process::exit(2);
    }
    println!(
        "| run | use cases | settled by checks | same side of 50% | steps passing | steps doubtful | steps broken | mean gap |"
    );
    println!("|---|---|---|---|---|---|---|---|");
    for run in &runs {
        match measure(Path::new(run)) {
            Some(a) => println!("{}", a.table_row(run)),
            None => println!("| {run} | no use-cases.yaml |||||||"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_agreement_by_side_of_the_flag() {
        let t = StepTally {
            passing: 1,
            ..StepTally::default()
        };
        let mut a = Agreement::default();
        a.add("agree", t, Some(1.0), Some(0.9));
        a.add("differ", t, Some(1.0), Some(0.2));
        a.add("open", t, None, Some(0.8));
        a.add("unscored", t, Some(1.0), None);
        assert_eq!((a.use_cases, a.scored_by_model, a.settled), (4, 3, 2));
        assert_eq!(a.same_side, 1);
        assert_eq!(a.disagreements, ["differ: checks 1.00, model 0.20"]);
        assert!((a.gap_sum - 0.9).abs() < 1e-6);
    }
}
