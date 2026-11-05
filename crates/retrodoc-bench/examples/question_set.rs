//! Checks the question set of `retrodoc ask` on a benchmark repository against its reference and a
//! checkout of the repository, with no LLM call (spec `ask-question-set`, `issues/ask_question_set.md`).
//!
//! Usage:
//! `cargo run -q -p retrodoc-bench --example question_set -- <questions.yaml> <reference.yaml> <tree>`
//!
//! - `questions.yaml`: `benchmark/<repo>/questions.yaml`.
//! - `reference.yaml`: `benchmark/<repo>/reference.yaml`.
//! - `tree`: a checkout of the repository at the commit of the reference.
//!
//! Prints the number of documented and undocumented questions, then each problem: a feature the
//! reference lacks or disputes, an absent term the tree holds, a tree at another commit. Exits with 1
//! when there is one.

use std::path::PathBuf;
use std::process::ExitCode;

use retrodoc_bench::benchmark::{check_questions, QuestionKind, QuestionSet, Reference};

fn main() -> ExitCode {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let [questions, reference, tree] = args.as_slice() else {
        eprintln!("usage: question_set <questions.yaml> <reference.yaml> <tree>");
        return ExitCode::from(2);
    };
    let loaded = QuestionSet::load(questions).and_then(|q| Ok((q, Reference::load(reference)?)));
    let (set, reference) = match loaded {
        Ok(loaded) => loaded,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
    };
    let documented = set
        .questions
        .iter()
        .filter(|q| matches!(q.kind(), Some(QuestionKind::Documented(_))))
        .count();
    println!(
        "{} questions: {documented} documented, {} undocumented",
        set.questions.len(),
        set.questions.len() - documented
    );
    let problems = check_questions(&set, &reference, tree);
    for problem in &problems {
        println!("{problem}");
    }
    if problems.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
