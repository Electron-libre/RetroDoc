//! Replays the placement of entry points into features with two answer formats on the same questions
//! (`issues/done/closed_answer_decisions_with_semif.md`, step 3), against a hand-labelled sample:
//!
//! - `json`: the answer of the pipeline (`features/from_surface/repair.rs`): one call lists the features and
//!   the entry points, the model writes a JSON object; same system prompt, same provider and schema;
//! - `readout (bm25)`: the closed-answer readout of `SemIf`: one question per entry point, at most 16
//!   lettered options (the best features by BM25), one generated token, the probability of each letter
//!   read from the top logprobs;
//! - `readout (tournament)`: the same over every feature, in groups of 12 then a final between the winners.
//!
//! Usage:
//! `OPENROUTER_API_KEY=x cargo run -q -p retrodoc-pipeline --example placement_replay -- <model> <base url> <run dir> <sample.yaml>`
//!
//! - `base url`: an OpenAI-compatible server that returns logprobs, without the `/v1/chat/completions` path.
//! - `run dir`: a saved run with its `.retrodoc/cache` (`features.yaml`, `entry-points.yaml`).
//! - `sample.yaml`: `benchmark/<repo>/placement-sample.yaml`, the entry points with the features each can go into.
//!
//! The brief of the product is left out of the JSON prompt. Prints one table: entry points placed, placed in
//! an acceptable feature, features that do not exist, agreement between two passes, time and tokens. The
//! cost of a readout is that of the questions the method asks; the questions in the reverse order only check
//! the steadiness and are not counted.

#![allow(clippy::cast_precision_loss)] // counts of questions: far below 2^52

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::time::Instant;

use retrodoc_core::config::LlmConfig;
use retrodoc_core::model::Feature;
use retrodoc_llm::{
    ChatMessage, CompletionRequest, LlmProvider, OpenRouterProvider, ResponseSchema, Role,
};
use retrodoc_pipeline::{entry_point_id, load_features, Bm25, EntryPoint, EntryPoints};
use serde::Deserialize;
use serde_json::{json, Value};

const LETTERS: &str = "ABCDEFGHIJKLMNOP";
const TOP_LOGPROBS: u32 = 20;
const MAX_DESCRIPTION_CHARS: usize = 160;
const SHORTLIST: usize = 16;
const GROUP: usize = 12;
/// Entry points per call of the pipeline (`REPAIR_BATCH`).
const BATCH: usize = 60;

/// The system message of `SemIf`'s direct mode (`semif_phase1/core.py`, prompt `direct-options-v1`).
const SYSTEM: &str =
    "Apply the supplied criterion to the supplied evidence. Choose exactly one listed option. \
Respond with only its uppercase letter, with no explanation or reasoning.";

const CRITERION: &str = "Which feature does this entry point belong to?";

/// The system prompt of the pipeline's placement (`PLACEMENT_SYSTEM_PROMPT`).
const PLACEMENT_SYSTEM_PROMPT: &str = "You are completing the functional (business) documentation \
of a software project. Some entry points (what users and systems can do) were left out of its \
features. For each entry point listed, pick the existing feature it belongs to, from the ones \
given. Copy the entry point's short id (E1, E2…, the first word of its line) and the feature slug \
verbatim. Leave an entry point out of your \
answer if no feature fits it; never invent a feature. Reply with ONLY a single JSON object, no \
prose and no Markdown code fence, matching this shape: \
{\"placements\":[{\"entry_point\":\"id\",\"feature\":\"slug\"}]}.";

/// The chat messages of one decision: a fixed system message and the JSON of evidence, criterion and options.
/// With `options_first` the options come before the evidence, so questions over the same options start with
/// the same text and a server that caches the prefix reads it once (the shared mode of `SemIf`).
fn messages(evidence: &str, criterion: &str, options: &[String], options_first: bool) -> Value {
    let options: Vec<Value> = options
        .iter()
        .zip(LETTERS.chars())
        .map(|(description, letter)| json!({"letter": letter.to_string(), "description": description}))
        .collect();
    let content = if options_first {
        format!(
            "{{\"criterion\":{},\"options\":{},\"evidence\":{}}}",
            json!(criterion),
            json!(options),
            json!(evidence)
        )
    } else {
        json!({"evidence": evidence, "criterion": criterion, "options": options}).to_string()
    };
    json!([
        {"role": "system", "content": SYSTEM},
        {"role": "user", "content": content},
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

/// What a set of calls cost.
#[derive(Debug, Default, Clone, Copy)]
struct Cost {
    calls: usize,
    seconds: f64,
    prompt_tokens: u64,
    /// Prompt tokens the server read from its cache instead of computing (when it says so).
    cached_tokens: u64,
    completion_tokens: u64,
}

impl Cost {
    fn add(&mut self, other: Cost) {
        self.calls += other.calls;
        self.seconds += other.seconds;
        self.prompt_tokens += other.prompt_tokens;
        self.cached_tokens += other.cached_tokens;
        self.completion_tokens += other.completion_tokens;
    }
}

struct Server<'a> {
    client: reqwest::Client,
    base_url: &'a str,
    model: &'a str,
}

/// One question of the readout: the index (in `options`) of the most probable option, and its cost. `None`
/// when the first token holds none of the letters.
async fn ask(
    server: &Server<'_>,
    messages: &Value,
    count: usize,
) -> Result<Option<(usize, Cost)>, reqwest::Error> {
    let body = json!({
        "model": server.model,
        "messages": messages,
        "max_tokens": 1,
        "temperature": 0,
        "logprobs": true,
        "top_logprobs": TOP_LOGPROBS,
        "reasoning_effort": "none",
    });
    let started = Instant::now();
    let reply: Value = server
        .client
        .post(format!("{}/v1/chat/completions", server.base_url))
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
    let cost = Cost {
        calls: 1,
        seconds,
        prompt_tokens: reply["usage"]["prompt_tokens"].as_u64().unwrap_or(0),
        cached_tokens: reply["usage"]["prompt_tokens_details"]["cached_tokens"]
            .as_u64()
            .unwrap_or(0),
        completion_tokens: reply["usage"]["completion_tokens"].as_u64().unwrap_or(0),
    };
    Ok(letter_probs(&top, count).map(|letters| (argmax(&letters.probs), cost)))
}

/// The feature as an option: the same facts as a line of the JSON prompt.
fn option_text(feature: &Feature) -> String {
    let description: String = feature
        .description
        .chars()
        .take(MAX_DESCRIPTION_CHARS)
        .collect();
    format!(
        "{} ({}): {} — {description}",
        feature.slug, feature.domain_slug, feature.name
    )
}

/// The entry point as the evidence of a question.
fn evidence(file: &str, point: &EntryPoint) -> String {
    let description: String = point
        .description
        .chars()
        .take(MAX_DESCRIPTION_CHARS)
        .collect();
    format!(
        "{} \"{}\" ({file}): {} on \"{}\": {description}",
        point.kind.label(),
        point.name,
        point.verb,
        point.resource
    )
}

/// The prompt of the pipeline's placement call, without the brief of the product.
fn json_prompt(features: &[Feature], lines: &[&str]) -> String {
    let mut prompt = String::from("Features:\n");
    for f in features {
        let _ = writeln!(
            prompt,
            "- {} ({}): {} — {}",
            f.slug, f.domain_slug, f.name, f.description
        );
    }
    prompt.push_str("\nEntry points to place:\n");
    for (position, line) in lines.iter().enumerate() {
        let _ = writeln!(prompt, "- E{} — {line}", position + 1);
    }
    prompt
}

/// The schema of `{"placements":[{"entry_point","feature"}]}`, in the strict shape of the pipeline.
fn placement_schema() -> ResponseSchema {
    ResponseSchema {
        name: "RawPlacements".into(),
        schema: json!({
            "type": "object",
            "properties": {"placements": {"type": "array", "items": {
                "type": "object",
                "properties": {"entry_point": {"type": "string"}, "feature": {"type": "string"}},
                "required": ["entry_point", "feature"],
                "additionalProperties": false,
            }}},
            "required": ["placements"],
            "additionalProperties": false,
        }),
    }
}

/// What a format answered for the sample: the slug chosen per entry point (`None`: left out) and the cost.
#[derive(Default)]
struct Outcome {
    picks: Vec<Option<String>>,
    cost: Cost,
    /// Feature slugs cited that do not exist (JSON only).
    unknown: usize,
    /// Entry points whose shortlist held an acceptable feature (shortlist methods only).
    shortlist_hits: Option<usize>,
    /// Entry points answered the same in the reverse order of the options (readouts only).
    reversed_same: Option<usize>,
}

/// The first JSON object of a model's answer.
fn parse_object(text: &str) -> Option<Value> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    serde_json::from_str(text.get(start..=end)?).ok()
}

async fn json_format(
    provider: &OpenRouterProvider,
    features: &[Feature],
    lines: &[String],
) -> Outcome {
    let mut outcome = Outcome {
        picks: vec![None; lines.len()],
        ..Outcome::default()
    };
    for (batch_index, batch) in lines.chunks(BATCH).enumerate() {
        let refs: Vec<&str> = batch.iter().map(String::as_str).collect();
        let request = CompletionRequest {
            messages: vec![
                ChatMessage {
                    role: Role::System,
                    content: PLACEMENT_SYSTEM_PROMPT.into(),
                },
                ChatMessage {
                    role: Role::User,
                    content: json_prompt(features, &refs),
                },
            ],
            model: None,
            json_schema: Some(placement_schema()),
        };
        let started = Instant::now();
        let response = provider
            .complete(request)
            .await
            .expect("the server answers");
        outcome.cost.calls += 1;
        outcome.cost.seconds += started.elapsed().as_secs_f64();
        if let Some(usage) = response.usage {
            outcome.cost.prompt_tokens += usage.prompt_tokens;
            outcome.cost.completion_tokens += usage.completion_tokens;
        }
        let Some(answer) = parse_object(&response.content) else {
            let head: String = response.content.chars().take(300).collect();
            eprintln!("unparseable answer to batch {batch_index}: {head:?}");
            continue;
        };
        for placement in answer["placements"].as_array().into_iter().flatten() {
            let tag = placement["entry_point"].as_str().unwrap_or_default();
            let slug = placement["feature"].as_str().unwrap_or_default();
            let Some(position) = tag
                .trim()
                .strip_prefix(['E', 'e'])
                .and_then(|n| n.parse::<usize>().ok())
                .filter(|n| (1..=batch.len()).contains(n))
            else {
                continue;
            };
            if features.iter().any(|f| f.slug == slug) {
                outcome.picks[batch_index * BATCH + position - 1] = Some(slug.to_string());
            } else {
                outcome.unknown += 1;
            }
        }
    }
    outcome
}

/// The index (in `features`) of the option the readout picks among `candidates`, and the cost of the question.
/// `reversed` lists the options in the reverse order, as a check on the order of the options.
async fn choose(
    server: &Server<'_>,
    features: &[Feature],
    evidence: &str,
    candidates: &[usize],
    reversed: bool,
    options_first: bool,
) -> Option<(usize, Cost)> {
    let mut order: Vec<usize> = candidates.to_vec();
    if reversed {
        order.reverse();
    }
    let options: Vec<String> = order.iter().map(|&i| option_text(&features[i])).collect();
    let (picked, cost) = ask(
        server,
        &messages(evidence, CRITERION, &options, options_first),
        options.len(),
    )
    .await
    .expect("the server answers")?;
    Some((order[picked], cost))
}

/// The best features by BM25, at most `SHORTLIST`, completed to two when the text shares no word.
fn shortlist(index: &Bm25, features: &[Feature], text: &str) -> Vec<usize> {
    let mut candidates: Vec<usize> = index
        .search_any(text)
        .into_iter()
        .map(|(i, _)| i)
        .take(SHORTLIST)
        .collect();
    for i in 0..features.len() {
        if candidates.len() < 2 && !candidates.contains(&i) {
            candidates.push(i);
        }
    }
    candidates
}

/// The readout over a shortlist of the features closest to the entry point by BM25: one question per
/// entry point, asked in each order.
async fn bm25_format(
    server: &Server<'_>,
    features: &[Feature],
    sample: &[(String, Vec<String>)],
    texts: &[String],
) -> Outcome {
    let index = Bm25::new(
        &features
            .iter()
            .map(|f| format!("{} {} {}", f.slug, f.name, f.description))
            .collect::<Vec<_>>(),
    );
    let mut outcome = Outcome {
        shortlist_hits: Some(0),
        reversed_same: Some(0),
        ..Outcome::default()
    };
    for (text, (_, acceptable)) in texts.iter().zip(sample) {
        let candidates = shortlist(&index, features, text);
        if let Some(hits) = outcome.shortlist_hits.as_mut() {
            *hits += usize::from(
                candidates
                    .iter()
                    .any(|&i| acceptable.contains(&features[i].slug)),
            );
        }
        let forward = choose(server, features, text, &candidates, false, false).await;
        let backward = choose(server, features, text, &candidates, true, false).await;
        if let Some((_, cost)) = forward {
            outcome.cost.add(cost);
        }
        if let Some(same) = outcome.reversed_same.as_mut() {
            *same +=
                usize::from(forward.is_some() && forward.map(|f| f.0) == backward.map(|b| b.0));
        }
        outcome
            .picks
            .push(forward.map(|(i, _)| features[i].slug.clone()));
    }
    outcome
}

/// The readout over every feature: groups of `GROUP` features, one question per entry point and group, then
/// a final question between the winners. The groups are the same for every entry point, so with
/// `options_first` the start of a question is the same for the 50 questions of a group; they are asked group
/// by group so a server that keeps one prefix at a time keeps it.
async fn tournament_format(
    server: &Server<'_>,
    features: &[Feature],
    texts: &[String],
    options_first: bool,
) -> Outcome {
    assert!(
        features.len() <= LETTERS.len() * GROUP,
        "the final round holds {} letters at most",
        LETTERS.len()
    );
    let all: Vec<usize> = (0..features.len()).collect();
    let mut outcome = Outcome {
        reversed_same: Some(0),
        ..Outcome::default()
    };
    // winners[entry point][group], forward and reversed order
    let mut forward: Vec<Vec<Option<usize>>> = vec![Vec::new(); texts.len()];
    let mut backward = forward.clone();
    for group in all.chunks(GROUP) {
        for reversed in [false, true] {
            for (e, text) in texts.iter().enumerate() {
                let answer = choose(server, features, text, group, reversed, options_first).await;
                if !reversed {
                    if let Some((_, cost)) = answer {
                        outcome.cost.add(cost);
                    }
                }
                let winners = if reversed {
                    &mut backward
                } else {
                    &mut forward
                };
                winners[e].push(answer.map(|a| a.0));
            }
        }
    }
    let mut stable = vec![true; texts.len()];
    for (e, text) in texts.iter().enumerate() {
        stable[e] = forward[e].iter().all(Option::is_some) && forward[e] == backward[e];
        let finalists: Vec<usize> = forward[e].iter().flatten().copied().collect();
        let winner = if finalists.len() > 1 {
            let first = choose(server, features, text, &finalists, false, false).await;
            let second = choose(server, features, text, &finalists, true, false).await;
            stable[e] &= first.is_some() && first.map(|f| f.0) == second.map(|s| s.0);
            if let Some((_, cost)) = first {
                outcome.cost.add(cost);
            }
            first.map(|f| f.0)
        } else {
            finalists.first().copied()
        };
        outcome.picks.push(winner.map(|i| features[i].slug.clone()));
    }
    outcome.reversed_same = Some(stable.iter().filter(|s| **s).count());
    outcome
}

#[derive(Debug, Deserialize)]
struct Sample {
    entry_points: Vec<Label>,
}

#[derive(Debug, Deserialize)]
struct Label {
    id: String,
    acceptable: Vec<String>,
    pipeline: Option<String>,
}

fn pct(part: usize, whole: usize) -> String {
    format!("{:.0}%", 100.0 * part as f64 / whole.max(1) as f64)
}

fn row(name: &str, outcome: &Outcome, sample: &[(String, Vec<String>)], agreement: &str) {
    let total = sample.len();
    let placed = outcome.picks.iter().filter(|p| p.is_some()).count();
    let good = outcome
        .picks
        .iter()
        .zip(sample)
        .filter(|(pick, (_, ok))| pick.as_ref().is_some_and(|p| ok.contains(p)))
        .count();
    let hits = outcome
        .shortlist_hits
        .map_or_else(|| "-".into(), |h| pct(h, total));
    let cost = outcome.cost;
    println!(
        "| {name} | {placed}/{total} | {} | {} | {hits} | {agreement} | {} | {:.1} | {} | {} | {} |",
        pct(good, total),
        outcome.unknown,
        cost.calls,
        cost.seconds,
        cost.prompt_tokens,
        cost.cached_tokens,
        cost.completion_tokens
    );
}

/// Prints the table of the formats.
fn print_results(
    model: &str,
    features: usize,
    labels: &[Label],
    sample: &[(String, Vec<String>)],
    json_runs: [&Outcome; 2],
    readouts: [(&str, &Outcome); 3],
) {
    let total = sample.len();
    let same = json_runs[0]
        .picks
        .iter()
        .zip(&json_runs[1].picks)
        .filter(|(a, b)| a == b)
        .count();
    let pipeline = Outcome {
        picks: labels.iter().map(|l| l.pipeline.clone()).collect(),
        ..Outcome::default()
    };
    println!("model {model}, {total} entry points, {features} features\n");
    println!("| format | placed | in an acceptable feature | unknown feature | shortlist holds one | steadiness | calls | seconds | prompt tokens | of which cached | completion tokens |");
    println!("|---|---|---|---|---|---|---|---|---|---|---|");
    row("pipeline run (reference)", &pipeline, sample, "-");
    row("json (run 1)", json_runs[0], sample, &pct(same, total));
    row("json (run 2)", json_runs[1], sample, &pct(same, total));
    for (name, outcome) in readouts {
        let steady = pct(outcome.reversed_same.unwrap_or(0), total);
        row(name, outcome, sample, &steady);
    }
    println!("\nsteadiness: JSON, same answer in two runs; readout, same answer with the options in reverse order.");
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [model, base_url, run, sample_path] = args.as_slice() else {
        eprintln!("usage: placement_replay <model> <base url> <run dir> <sample.yaml>");
        std::process::exit(2);
    };
    let base_url = base_url.trim_end_matches('/');
    let run = std::path::Path::new(run);
    let features = load_features(run).expect("features.yaml of the run");
    let entry_points = EntryPoints::load(run).expect("entry-points.yaml of the run");
    let by_id: BTreeMap<String, (String, &EntryPoint)> = entry_points
        .iter()
        .map(|(file, point)| {
            (
                entry_point_id(file, point),
                (file.display().to_string(), point),
            )
        })
        .collect();
    let labels: Sample = serde_yaml::from_str(
        &std::fs::read_to_string(sample_path).expect("the sample file is readable"),
    )
    .expect("the sample file is YAML");
    let labels: Vec<Label> = labels
        .entry_points
        .into_iter()
        .filter(|l| by_id.contains_key(&l.id))
        .collect();
    assert!(
        !labels.is_empty(),
        "no entry point of the sample in the run"
    );
    let sample: Vec<(String, Vec<String>)> = labels
        .iter()
        .map(|l| (l.id.clone(), l.acceptable.clone()))
        .collect();
    let texts: Vec<String> = labels
        .iter()
        .map(|l| {
            let (file, point) = &by_id[&l.id];
            evidence(file, point)
        })
        .collect();

    let config: LlmConfig = serde_json::from_value(json!({
        "provider": "openrouter",
        "model": model,
        "base_url": format!("{base_url}/v1/chat/completions"),
        "reasoning_effort": "none",
        "timeout_secs": 900,
    }))
    .expect("llm config");
    let provider = OpenRouterProvider::from_config(&config).expect("OPENROUTER_API_KEY is set");
    let server = Server {
        client: reqwest::Client::builder()
            .timeout(std::time::Duration::from_mins(15))
            .build()
            .expect("http client"),
        base_url,
        model,
    };

    let first = json_format(&provider, &features, &texts).await;
    let second = json_format(&provider, &features, &texts).await;
    let bm25 = bm25_format(&server, &features, &sample, &texts).await;
    let tournament = tournament_format(&server, &features, &texts, false).await;
    let shared = tournament_format(&server, &features, &texts, true).await;

    print_results(
        model,
        features.len(),
        &labels,
        &sample,
        [&first, &second],
        [
            ("readout, bm25 shortlist of 16", &bm25),
            ("readout, tournament", &tournament),
            ("readout, tournament, options first", &shared),
        ],
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
        let m = messages(
            "evidence",
            "question",
            &["first".into(), "second".into()],
            false,
        );
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
    fn options_first_puts_the_evidence_last() {
        let options = ["first".to_string(), "second".to_string()];
        let m = messages("evidence", "question", &options, true);
        let content = m[1]["content"].as_str().unwrap();
        let payload: Value = serde_json::from_str(content).unwrap();
        assert_eq!(payload["evidence"], "evidence");
        assert_eq!(payload["options"][1]["letter"], "B");
        assert!(content.find("options").unwrap() < content.find("evidence").unwrap());
        // two questions over the same options share the text before the evidence
        let other = messages("another", "question", &options, true);
        let other = other[1]["content"].as_str().unwrap();
        let shared = content.find("\"evidence\"").unwrap();
        assert_eq!(content[..shared], other[..shared]);
    }

    #[test]
    fn argmax_picks_the_largest() {
        assert_eq!(argmax(&[0.1, 0.7, 0.2]), 1);
    }

    #[test]
    fn the_answer_is_found_in_prose_and_fences() {
        let v = parse_object("Sure:\n```json\n{\"placements\":[]}\n```").unwrap();
        assert_eq!(v["placements"], json!([]));
        assert!(parse_object("no object here").is_none());
    }

    #[test]
    fn the_json_prompt_tags_entry_points_from_e1() {
        let prompt = json_prompt(&[], &["first", "second"]);
        assert!(prompt.contains("- E1 — first\n- E2 — second\n"));
    }

    #[test]
    fn cost_adds_up() {
        let mut total = Cost::default();
        total.add(Cost {
            calls: 1,
            seconds: 0.5,
            prompt_tokens: 10,
            cached_tokens: 4,
            completion_tokens: 1,
        });
        total.add(total);
        assert_eq!((total.calls, total.prompt_tokens), (2, 20));
    }
}
