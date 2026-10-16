//! Prototype of the closed-answer readout of `SemIf` on the placement of the entry points
//! (`issues/closed_answer_decisions_with_semif.md`, step 1): one forward pass, one generated token, and the
//! probability of each lettered option read from the top logprobs of an OpenAI-compatible server (Ollama).
//!
//! Usage:
//! `cargo run -q -p retrodoc-pipeline --example closed_answer_readout -- <model> <base url> <run dir> [questions] [options]`
//!
//! - `run dir`: a saved run, a checkout with its `.retrodoc/cache` (`features.yaml`, `entry-points.yaml`).
//! - Each question is an entry point a feature holds, asked against that feature and up to `options - 1`
//!   others (16 at most, the letters A to P of `SemIf`), twice: in one order and in the reverse one.
//!
//! Prints, per run: agreement with the placement the pipeline made, agreement between the two orders,
//! letters missing from the returned top logprobs, time and tokens per question. Nothing is written.

#![allow(clippy::cast_precision_loss)] // counts of questions: far below 2^52

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Instant;

use retrodoc_core::model::Feature;
use retrodoc_pipeline::{entry_point_id, load_features, EntryPoint, EntryPoints};
use serde_json::{json, Value};

const LETTERS: &str = "ABCDEFGHIJKLMNOP";
const TOP_LOGPROBS: u32 = 20;
const MAX_DESCRIPTION_CHARS: usize = 200;

/// The system message of `SemIf`'s direct mode (`semif_phase1/core.py`, prompt `direct-options-v1`).
const SYSTEM: &str =
    "Apply the supplied criterion to the supplied evidence. Choose exactly one listed option. \
Respond with only its uppercase letter, with no explanation or reasoning.";

const CRITERION: &str = "Which feature does this entry point belong to?";

/// The chat messages of one decision: a fixed system message and the JSON of evidence, criterion and options.
fn messages(evidence: &str, criterion: &str, options: &[String]) -> Value {
    let options: Vec<Value> = options
        .iter()
        .zip(LETTERS.chars())
        .map(|(description, letter)| json!({"letter": letter.to_string(), "description": description}))
        .collect();
    let payload = json!({"evidence": evidence, "criterion": criterion, "options": options});
    json!([
        {"role": "system", "content": SYSTEM},
        {"role": "user", "content": payload.to_string()},
    ])
}

/// The probability of each of the first `count` letters, and how many of them the server did not return.
#[derive(Debug, PartialEq)]
struct LetterProbs {
    probs: Vec<f64>,
    missing: usize,
}

/// Softmax of the letters over the top logprobs `(token, logprob)` of the first generated token. A variant
/// such as `" A"` counts for `A`; a letter absent from the top has a mass of zero. `None` when none is there.
fn letter_probs(top: &[(String, f64)], count: usize) -> Option<LetterProbs> {
    let mut mass = vec![0.0_f64; count];
    for (token, logprob) in top {
        let token = token.trim();
        if let Some(index) = LETTERS
            .chars()
            .take(count)
            .position(|l| token == l.to_string())
        {
            mass[index] += logprob.exp();
        }
    }
    let total: f64 = mass.iter().sum();
    if total <= 0.0 {
        return None;
    }
    let missing = mass.iter().filter(|m| **m == 0.0).count();
    Some(LetterProbs {
        probs: mass.iter().map(|m| m / total).collect(),
        missing,
    })
}

fn argmax(values: &[f64]) -> usize {
    values
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .map_or(0, |(i, _)| i)
}

/// What one question cost.
struct Answer {
    letters: LetterProbs,
    seconds: f64,
    prompt_tokens: u64,
    completion_tokens: u64,
}

async fn ask(
    client: &reqwest::Client,
    base_url: &str,
    model: &str,
    messages: &Value,
    count: usize,
) -> Result<Option<Answer>, reqwest::Error> {
    let body = json!({
        "model": model,
        "messages": messages,
        "max_tokens": 1,
        "temperature": 0,
        "logprobs": true,
        "top_logprobs": TOP_LOGPROBS,
        "reasoning_effort": "none",
    });
    let started = Instant::now();
    let reply: Value = client
        .post(format!("{base_url}/v1/chat/completions"))
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let seconds = started.elapsed().as_secs_f64();
    let top: Vec<(String, f64)> = reply["choices"][0]["logprobs"]["content"][0]["top_logprobs"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|t| Some((t["token"].as_str()?.to_string(), t["logprob"].as_f64()?)))
                .collect()
        })
        .unwrap_or_default();
    Ok(letter_probs(&top, count).map(|letters| Answer {
        letters,
        seconds,
        prompt_tokens: reply["usage"]["prompt_tokens"].as_u64().unwrap_or(0),
        completion_tokens: reply["usage"]["completion_tokens"].as_u64().unwrap_or(0),
    }))
}

fn option_text(feature: &Feature) -> String {
    let description: String = feature
        .description
        .chars()
        .take(MAX_DESCRIPTION_CHARS)
        .collect();
    format!("{}: {} - {description}", feature.slug, feature.name)
}

/// Deterministic pseudo-random number below `bound` (no `rand` dependency).
fn below(state: &mut u64, bound: usize) -> usize {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    usize::try_from((*state >> 33) % bound as u64).expect("below a usize")
}

/// The true feature and distractors among `total` features, in a shuffled order.
fn choose_options(state: &mut u64, truth: usize, total: usize, wanted: usize) -> Vec<usize> {
    let mut chosen = vec![truth];
    while chosen.len() < wanted.min(total) {
        let candidate = below(state, total);
        if !chosen.contains(&candidate) {
            chosen.push(candidate);
        }
    }
    for i in (1..chosen.len()).rev() {
        chosen.swap(i, below(state, i + 1));
    }
    chosen
}

#[derive(Default)]
struct Totals {
    questions: usize,
    answered: usize,
    correct: usize,
    stable: usize,
    missing_letters: usize,
    letters_asked: usize,
    seconds: f64,
    prompt_tokens: u64,
    completion_tokens: u64,
    calls: usize,
    /// Questions whose first token holds none of the letters (not a server error).
    no_letter: usize,
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [model, base_url, run, rest @ ..] = args.as_slice() else {
        eprintln!(
            "usage: closed_answer_readout <model> <base url> <run dir> [questions] [options]"
        );
        std::process::exit(2);
    };
    let questions: usize = rest.first().and_then(|v| v.parse().ok()).unwrap_or(30);
    let option_count: usize = rest
        .get(1)
        .and_then(|v| v.parse().ok())
        .unwrap_or(8)
        .clamp(2, LETTERS.len());

    let run = Path::new(run);
    let features = load_features(run).expect("features.yaml of the run");
    let entry_points = EntryPoints::load(run).expect("entry-points.yaml of the run");
    let points: BTreeMap<String, &EntryPoint> = entry_points
        .iter()
        .map(|(file, point)| (entry_point_id(file, point), point))
        .collect();
    // (entry point id, index of the feature holding it)
    let held: Vec<(&String, usize)> = features
        .iter()
        .enumerate()
        .flat_map(|(i, f)| f.entry_points.iter().map(move |id| (id, i)))
        .filter(|(id, _)| points.contains_key(id.as_str()))
        .collect();
    assert!(
        features.len() >= 2 && !held.is_empty() && questions > 0,
        "nothing to ask"
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .expect("http client");
    let mut state = 42_u64;
    let mut totals = Totals::default();
    let asked = questions.min(held.len());
    for n in 0..asked {
        let (id, truth) = held[n * held.len() / asked];
        let point = points[id.as_str()];
        let evidence = format!(
            "{} \"{}\": {} on \"{}\": {}",
            point.kind.label(),
            point.name,
            point.verb,
            point.resource,
            point.description
        );
        let chosen = choose_options(&mut state, truth, features.len(), option_count);
        let forward: Vec<String> = chosen.iter().map(|&i| option_text(&features[i])).collect();
        let reversed: Vec<String> = forward.iter().rev().cloned().collect();

        totals.questions += 1;
        let count = chosen.len();
        let first = ask(
            &client,
            base_url,
            model,
            &messages(&evidence, CRITERION, &forward),
            count,
        )
        .await
        .unwrap_or_else(|e| panic!("the server failed: {e}"));
        let second = ask(
            &client,
            base_url,
            model,
            &messages(&evidence, CRITERION, &reversed),
            count,
        )
        .await
        .unwrap_or_else(|e| panic!("the server failed: {e}"));
        let (Some(first), Some(second)) = (first, second) else {
            totals.no_letter += 1;
            continue;
        };
        totals.answered += 1;
        let pick_first = chosen[argmax(&first.letters.probs)];
        let pick_second = chosen[count - 1 - argmax(&second.letters.probs)];
        totals.correct += usize::from(pick_first == truth);
        totals.stable += usize::from(pick_first == pick_second);
        for answer in [&first, &second] {
            totals.missing_letters += answer.letters.missing;
            totals.letters_asked += count;
            totals.seconds += answer.seconds;
            totals.prompt_tokens += answer.prompt_tokens;
            totals.completion_tokens += answer.completion_tokens;
            totals.calls += 1;
        }
    }

    print_totals(model, option_count, &totals);
}

fn print_totals(model: &str, option_count: usize, totals: &Totals) {
    let answered = totals.answered.max(1) as f64;
    let calls = totals.calls.max(1) as f64;
    println!(
        "model {model}, {option_count} options, {} questions",
        totals.questions
    );
    println!("| measure | value |\n|---|---|");
    println!(
        "| questions with no letter in the first token | {} |",
        totals.no_letter
    );
    println!(
        "| questions answered | {}/{} |",
        totals.answered, totals.questions
    );
    println!(
        "| agrees with the pipeline's placement | {:.0}% |",
        100.0 * totals.correct as f64 / answered
    );
    println!(
        "| same feature in both orders | {:.0}% |",
        100.0 * totals.stable as f64 / answered
    );
    println!(
        "| letters missing from the top {TOP_LOGPROBS} | {:.1}% |",
        100.0 * totals.missing_letters as f64 / totals.letters_asked.max(1) as f64
    );
    println!("| seconds per call | {:.2} |", totals.seconds / calls);
    println!(
        "| prompt tokens per call | {:.0} |",
        totals.prompt_tokens as f64 / calls
    );
    println!(
        "| completion tokens per call | {:.1} |",
        totals.completion_tokens as f64 / calls
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn top(items: &[(&str, f64)]) -> Vec<(String, f64)> {
        items.iter().map(|(t, l)| ((*t).to_string(), *l)).collect()
    }

    #[test]
    fn probabilities_sum_to_one_and_follow_the_logprobs() {
        let probs = letter_probs(&top(&[("A", -0.1), ("B", -2.3)]), 2).unwrap();
        assert!((probs.probs.iter().sum::<f64>() - 1.0).abs() < 1e-9);
        assert!(probs.probs[0] > probs.probs[1]);
        assert_eq!(probs.missing, 0);
    }

    #[test]
    fn a_variant_with_a_space_counts_for_its_letter() {
        let probs = letter_probs(&top(&[("A", -1.0), (" A", -1.0), ("B", -1.0)]), 2).unwrap();
        assert!((probs.probs[0] - 2.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn a_letter_out_of_the_top_has_no_mass_and_is_counted() {
        let probs = letter_probs(&top(&[("A", -0.5), ("Z", -1.0)]), 3).unwrap();
        assert_eq!(probs.probs, vec![1.0, 0.0, 0.0]);
        assert_eq!(probs.missing, 2);
    }

    #[test]
    fn letters_beyond_the_options_are_ignored() {
        let probs = letter_probs(&top(&[("A", -1.0), ("C", -0.1)]), 2).unwrap();
        assert_eq!(probs.probs, vec![1.0, 0.0]);
    }

    #[test]
    fn no_letter_in_the_top_gives_no_answer() {
        assert_eq!(letter_probs(&top(&[("The", -0.1)]), 2), None);
    }

    #[test]
    fn the_prompt_letters_the_options_in_order() {
        let m = messages("evidence", "question", &["first".into(), "second".into()]);
        assert_eq!(m[0]["content"], SYSTEM);
        let payload: Value = serde_json::from_str(m[1]["content"].as_str().unwrap()).unwrap();
        assert_eq!(
            payload["options"][0],
            json!({"letter": "A", "description": "first"})
        );
        assert_eq!(
            payload["options"][1],
            json!({"letter": "B", "description": "second"})
        );
        assert_eq!(payload["evidence"], "evidence");
    }

    #[test]
    fn argmax_picks_the_largest() {
        assert_eq!(argmax(&[0.1, 0.7, 0.2]), 1);
    }

    #[test]
    fn options_hold_the_truth_once_and_no_duplicate() {
        let mut state = 7;
        for truth in 0..10 {
            let chosen = choose_options(&mut state, truth, 10, 6);
            assert_eq!(chosen.len(), 6);
            assert!(chosen.contains(&truth));
            let mut sorted = chosen.clone();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(sorted.len(), 6);
        }
    }

    #[test]
    fn options_are_capped_by_the_features() {
        assert_eq!(choose_options(&mut 1, 0, 3, 8).len(), 3);
    }
}
