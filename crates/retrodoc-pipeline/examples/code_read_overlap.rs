//! How much code the use cases prompt and the rules prompt of a feature share, from frozen inputs, with
//! no LLM call (`issues/one_code_reading_for_use_cases_and_rules.md`, step 3).
//!
//! Usage: `cargo run -q -p retrodoc-pipeline --example code_read_overlap -- <frozen dir> <benchmark dir>`
//!
//! Both passes are replayed on the frozen features against a provider that records the prompts and
//! answers with an empty list. A prompt is split into its `=== path ===` blocks (the code, the last
//! part of both prompts); for each feature the characters of the blocks of the same file are the code
//! the two prompts share (the smaller of the two excerpts). Printed: one row per feature, then the
//! totals: the share of each prompt that the other repeats.
//!
//! Limits: only the first prompt that shows code is counted per feature (the kind question and the
//! judgement of the rules carry no code; a statement call cut at the completion limit is asked again on
//! halves, which this does not see, so the figures are a floor for such a feature), and a source line
//! that looks like `=== x ===` would be read as a header.
//!
//! The benchmark dir (`benchmark/<repo>`) is only there because the rules replay asks for its reference.
//! The tree read is `manifest.source_run`, as for `pass_replay`. Feature names are printed to read the
//! table: never copy them in a committed file when the repository is confidential.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use retrodoc_llm::{
    CompletionRequest, CompletionResponse, LlmError, LlmProvider, Role, UsageTracker,
};
use retrodoc_pipeline::benchmark::{replay_rules, replay_use_cases, FrozenInputs, Reference};
use std::sync::Mutex;

type Failure = Box<dyn std::error::Error>;

/// What one prompt holds: its characters, and the characters of each file's excerpt.
#[derive(Debug, Default, PartialEq, Eq)]
struct Prompt {
    chars: usize,
    code: BTreeMap<String, usize>,
}

/// The feature a prompt is about (the name after `Feature: `) and its code blocks. A block starts with a
/// line `=== path ===` or `=== [F1] path ===` and ends at the next one.
fn parse(prompt: &str) -> Option<(String, Prompt)> {
    let name = prompt
        .lines()
        .find_map(|l| l.strip_prefix("Feature: "))?
        .split(" — ")
        .next()?
        .to_string();
    let mut parsed = Prompt {
        chars: prompt.chars().count(),
        code: BTreeMap::new(),
    };
    let mut current: Option<String> = None;
    for line in prompt.lines() {
        if let Some(header) = line
            .strip_prefix("=== ")
            .and_then(|l| l.strip_suffix(" ==="))
        {
            let path = match header.strip_prefix('[') {
                Some(rest) => rest.split_once("] ").map_or(rest, |(_, p)| p),
                None => header,
            };
            current = Some(path.to_string());
            parsed.code.entry(path.to_string()).or_default();
        } else if let Some(path) = &current {
            *parsed.code.get_mut(path).expect("inserted above") += line.chars().count() + 1;
        }
    }
    Some((name, parsed))
}

/// Records the prompts and answers `answer`, an empty list of the shape the pass reads.
struct Recorder {
    answer: &'static str,
    prompts: Mutex<Vec<String>>,
}

impl Recorder {
    fn answering(answer: &'static str) -> Self {
        Self {
            answer,
            prompts: Mutex::default(),
        }
    }
}

#[async_trait]
impl LlmProvider for Recorder {
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, LlmError> {
        let user = request
            .messages
            .iter()
            .find(|m| m.role == Role::User)
            .map(|m| m.content.clone())
            .unwrap_or_default();
        self.prompts.lock().unwrap().push(user);
        Ok(CompletionResponse {
            content: self.answer.to_string(),
            ..CompletionResponse::default()
        })
    }
}

/// One feature seen by the two passes.
#[derive(Debug, Default, PartialEq, Eq)]
struct Row {
    use_cases: usize,
    rules: usize,
    shared_code: usize,
}

fn rows(use_case_prompts: &[String], rule_prompts: &[String]) -> BTreeMap<String, Row> {
    let mut by_feature: BTreeMap<String, (Prompt, Prompt)> = BTreeMap::new();
    for (index, prompts) in [use_case_prompts, rule_prompts].into_iter().enumerate() {
        for (name, prompt) in prompts.iter().filter_map(|p| parse(p)) {
            let entry = by_feature.entry(name).or_default();
            let side = if index == 0 {
                &mut entry.0
            } else {
                &mut entry.1
            };
            // The empty answers of the recorder make a pass ask again, and a pass may ask a short
            // question first: count the first prompt that shows code, once per feature.
            if side.code.is_empty() {
                *side = prompt;
            }
        }
    }
    by_feature
        .into_iter()
        .map(|(name, (uc, rules))| {
            let shared_code = uc
                .code
                .iter()
                .filter_map(|(path, n)| rules.code.get(path).map(|m| (*n).min(*m)))
                .sum();
            (
                name,
                Row {
                    use_cases: uc.chars,
                    rules: rules.chars,
                    shared_code,
                },
            )
        })
        .collect()
}

#[allow(clippy::cast_precision_loss)] // character counts: far below 2^52
fn percent(part: usize, whole: usize) -> f64 {
    100.0 * part as f64 / whole.max(1) as f64
}

fn report(rows: &BTreeMap<String, Row>) -> String {
    let mut out =
        String::from("| Feature | Use cases chars | Rules chars | Shared code | In use cases | In rules |\n|---|---|---|---|---|---|\n");
    let mut total = Row::default();
    for (name, row) in rows {
        let _ = writeln!(
            out,
            "| {name} | {} | {} | {} | {:.0}% | {:.0}% |",
            row.use_cases,
            row.rules,
            row.shared_code,
            percent(row.shared_code, row.use_cases),
            percent(row.shared_code, row.rules),
        );
        total.use_cases += row.use_cases;
        total.rules += row.rules;
        total.shared_code += row.shared_code;
    }
    let _ = writeln!(
        out,
        "| **{} features** | {} | {} | {} | {:.0}% | {:.0}% |",
        rows.len(),
        total.use_cases,
        total.rules,
        total.shared_code,
        percent(total.shared_code, total.use_cases),
        percent(total.shared_code, total.rules),
    );
    let _ = write!(
        out,
        "\nBoth passes together send {} characters; the shared code is {:.0}% of them.\n",
        total.use_cases + total.rules,
        percent(total.shared_code, total.use_cases + total.rules),
    );
    out
}

async fn run(frozen_dir: &Path, benchmark_dir: &Path) -> Result<String, Failure> {
    let frozen = FrozenInputs::open(frozen_dir)?;
    let tree = PathBuf::from(&frozen.manifest.source_run);
    let work = tempfile::tempdir()?;
    let reference = Reference::load(&benchmark_dir.join("reference.yaml"))?;

    let use_cases = Recorder::answering(r#"{"use_cases":[]}"#);
    let rules = Recorder::answering(r#"{"rules":[]}"#);
    replay_use_cases(
        &frozen,
        &tree,
        &work.path().join("use-cases"),
        &use_cases,
        None,
        &UsageTracker::new(),
    )
    .await?;
    replay_rules(
        &frozen,
        &tree,
        &work.path().join("rules"),
        &rules,
        None,
        &UsageTracker::new(),
        &reference,
    )
    .await?;
    let use_cases = use_cases.prompts.into_inner().unwrap();
    let rules = rules.prompts.into_inner().unwrap();
    Ok(report(&rows(&use_cases, &rules)))
}

#[tokio::main]
async fn main() -> Result<(), Failure> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [frozen, benchmark] = args.as_slice() else {
        return Err("usage: code_read_overlap <frozen dir> <benchmark dir>".into());
    };
    print!("{}", run(Path::new(frozen), Path::new(benchmark)).await?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn use_cases_prompt() -> String {
        "Brief\nFeature: Refund — money back\n\nSource files:\n\n=== a.rs ===\n1 x\n2 y\n\n=== b.rs ===\n1 z\n"
            .to_string()
    }

    fn rules_prompt() -> String {
        "Feature: Refund — money back\n\nSource files:\n\n=== [F1] a.rs ===\n1 x\n2 y\n\n=== [F2] c.rs ===\n1 w\n"
            .to_string()
    }

    #[test]
    fn a_prompt_gives_its_feature_and_the_size_of_each_file_block() {
        let (name, prompt) = parse(&use_cases_prompt()).unwrap();
        assert_eq!(name, "Refund");
        assert_eq!(prompt.code.keys().collect::<Vec<_>>(), ["a.rs", "b.rs"]);
        assert!(prompt.code["a.rs"] > prompt.code["b.rs"]);
        assert!(parse("no feature here").is_none());
    }

    #[test]
    fn only_the_files_both_prompts_show_are_shared() {
        let rows = rows(&[use_cases_prompt()], &[rules_prompt()]);
        let row = &rows["Refund"];
        let (_, uc) = parse(&use_cases_prompt()).unwrap();
        assert_eq!(row.shared_code, uc.code["a.rs"]);
        assert!(row.shared_code < row.use_cases);
        assert!(report(&rows).contains("1 features"));
    }
}
