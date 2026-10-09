//! One HTTP call to check what a server does with a strict JSON schema (`response_format`), without
//! a full `generate` run: is it rejected, ignored, or obeyed?
//!
//! ```sh
//! cargo run -p retrodoc-llm --example schema_probe -- <preset> [--model M] [--base-url U] [--key-env V] [--json-object]
//! ```
//!
//! Presets: `ollama` (local, no key), `deepseek`, `gemini`, `openrouter`. The key comes from the
//! environment or the `.env` of the current directory. The prompt does not mention the JSON shape, so a
//! conforming answer comes from the schema, not from the prompt. The request is the one
//! `OpenRouterProvider` sends (strict `json_schema`, `require_parameters` on `OpenRouter`), sent
//! raw, so a refusal is seen here and not hidden by the provider's fallback.
//!
//! `--json-object` probes the weaker `response_format: {"type": "json_object"}` instead (valid JSON,
//! no schema): the prompt then spells the shape out, as the servers that offer only this mode require
//! the word JSON in it, and the verdict says whether the answer is valid JSON of that shape.

use std::time::Duration;

use serde_json::{json, Value};

struct Target {
    base_url: &'static str,
    model: &'static str,
    key_env: Option<&'static str>,
    reasoning_effort: Option<&'static str>,
    require_parameters: bool,
}

fn preset(name: &str) -> Option<Target> {
    Some(match name {
        "ollama" => Target {
            base_url: "http://localhost:11435/v1/chat/completions",
            model: "qwen3.6:35b-a3b",
            key_env: None,
            reasoning_effort: Some("none"),
            require_parameters: false,
        },
        "deepseek" => Target {
            base_url: "https://api.deepseek.com/chat/completions",
            model: "deepseek-chat",
            key_env: Some("DEEPSEEK_API_KEY"),
            reasoning_effort: None,
            require_parameters: false,
        },
        "gemini" => Target {
            base_url: "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
            model: "gemini-3.8-flash",
            key_env: Some("GEMINI_API_KEY"),
            reasoning_effort: Some("none"),
            require_parameters: false,
        },
        "openrouter" => Target {
            base_url: "https://openrouter.ai/api/v1/chat/completions",
            model: "anthropic/claude-sonnet-4.5",
            key_env: Some("OPENROUTER_API_KEY"),
            reasoning_effort: None,
            require_parameters: true,
        },
        _ => return None,
    })
}

/// A schema in the shape `response_schema` produces: every property required, no extra property,
/// a nullable field as `anyOf`, a nested array of objects.
fn schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["city", "population_millions", "landmarks", "mayor_note"],
        "properties": {
            "city": {"type": "string"},
            "population_millions": {"type": "integer"},
            "landmarks": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["name", "year_built"],
                    "properties": {
                        "name": {"type": "string"},
                        "year_built": {"type": "integer"}
                    }
                }
            },
            "mayor_note": {"anyOf": [{"type": "string"}, {"type": "null"}]}
        }
    })
}

#[derive(Debug, PartialEq, Eq)]
enum Verdict {
    /// The answer follows the schema.
    Obeyed,
    /// Answered, but the content does not follow the schema.
    Ignored(String),
    /// The server refused the request.
    Rejected(String),
    /// Rate limit or server error: says nothing about the schema, try again later.
    Unavailable(String),
}

/// Whether `value` follows [`schema`]: the keys, the types, nothing more.
fn conforms(value: &Value) -> Result<(), String> {
    let object = value.as_object().ok_or("not a JSON object")?;
    let expected = ["city", "population_millions", "landmarks", "mayor_note"];
    for key in expected {
        if !object.contains_key(key) {
            return Err(format!("missing `{key}`"));
        }
    }
    if let Some(extra) = object.keys().find(|k| !expected.contains(&k.as_str())) {
        return Err(format!("unexpected `{extra}`"));
    }
    if !object["city"].is_string() {
        return Err("`city` is not a string".into());
    }
    if !(object["population_millions"].is_i64() || object["population_millions"].is_u64()) {
        return Err("`population_millions` is not an integer".into());
    }
    let landmarks = object["landmarks"]
        .as_array()
        .ok_or("`landmarks` is not an array")?;
    for landmark in landmarks {
        let ok = landmark.as_object().is_some_and(|l| {
            l.len() == 2
                && l.get("name").is_some_and(Value::is_string)
                && l.get("year_built")
                    .is_some_and(|y| y.is_i64() || y.is_u64())
        });
        if !ok {
            return Err(format!("a landmark does not follow the schema: {landmark}"));
        }
    }
    if !(object["mayor_note"].is_string() || object["mayor_note"].is_null()) {
        return Err("`mayor_note` is neither a string nor null".into());
    }
    Ok(())
}

fn judge(status: u16, body: &str) -> Verdict {
    if !(200..300).contains(&status) {
        let shown: String = body.split_whitespace().collect::<Vec<_>>().join(" ");
        let shown: String = shown.chars().take(300).collect();
        let why = format!("HTTP {status}: {shown}");
        return if status == 429 || status >= 500 {
            Verdict::Unavailable(why)
        } else {
            Verdict::Rejected(why)
        };
    }
    let content = serde_json::from_str::<Value>(body).ok().and_then(|v| {
        v["choices"][0]["message"]["content"]
            .as_str()
            .map(str::to_string)
    });
    let Some(content) = content else {
        return Verdict::Ignored("no content in the response".into());
    };
    match serde_json::from_str::<Value>(&content) {
        Ok(value) => match conforms(&value) {
            Ok(()) => Verdict::Obeyed,
            Err(why) => Verdict::Ignored(why),
        },
        Err(err) => {
            let shown: String = content.chars().take(120).collect();
            Verdict::Ignored(format!("the content is not JSON ({err}): {shown}"))
        }
    }
}

/// Variables of a `.env` in the current directory, those already set winning.
fn load_dotenv() {
    let Ok(text) = std::fs::read_to_string(".env") else {
        return;
    };
    for line in text.lines() {
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            if !key.starts_with('#') && std::env::var_os(key).is_none() {
                std::env::set_var(key, value.trim().trim_matches('"'));
            }
        }
    }
}

#[tokio::main]
async fn main() {
    load_dotenv();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(target) = args.first().and_then(|name| preset(name)) else {
        eprintln!("usage: schema_probe <ollama|deepseek|gemini|openrouter> [--model M] [--base-url U] [--key-env V]");
        std::process::exit(2);
    };
    let flag = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let model = flag("--model").unwrap_or_else(|| target.model.to_string());
    let base_url = flag("--base-url").unwrap_or_else(|| target.base_url.to_string());
    let key_env = flag("--key-env").or_else(|| target.key_env.map(str::to_string));
    let key = key_env.as_deref().map_or_else(
        || "unused".to_string(),
        |var| {
            std::env::var(var).unwrap_or_else(|_| {
                eprintln!("{var} is not set (environment or .env)");
                std::process::exit(2);
            })
        },
    );

    let json_object = args.iter().any(|a| a == "--json-object");
    let question = "Tell me about Paris: its population in millions, two landmarks with the year they were built, and a note about its mayor if you have one.";
    let (question, response_format) = if json_object {
        (
            format!("{question} Answer in JSON only, an object with the keys city (string), population_millions (integer), landmarks (array of objects with name (string) and year_built (integer)) and mayor_note (string or null), and no other key."),
            json!({"type": "json_object"}),
        )
    } else {
        (
            question.to_string(),
            json!({
                "type": "json_schema",
                "json_schema": {"name": "city_facts", "strict": true, "schema": schema()}
            }),
        )
    };
    let mut body = json!({
        "model": model,
        "messages": [
            {"role": "system", "content": "You answer questions about cities."},
            {"role": "user", "content": question}
        ],
        "max_tokens": 1024,
        "response_format": response_format
    });
    if let Some(effort) = target.reasoning_effort {
        body["reasoning_effort"] = json!(effort);
    }
    if target.require_parameters {
        body["provider"] = json!({"require_parameters": true});
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .expect("http client");
    let started = std::time::Instant::now();
    let response = client
        .post(&base_url)
        .bearer_auth(&key)
        .json(&body)
        .send()
        .await;
    let (status, text) = match response {
        Ok(response) => {
            let status = response.status().as_u16();
            (status, response.text().await.unwrap_or_default())
        }
        Err(err) => {
            println!("{model} @ {base_url}: no answer ({err})");
            std::process::exit(1);
        }
    };
    let verdict = judge(status, &text);
    println!(
        "{model} @ {base_url} ({:.1}s)",
        started.elapsed().as_secs_f32()
    );
    match &verdict {
        Verdict::Obeyed if json_object => {
            println!(
                "OBEYED  - json_object accepted, valid JSON of the shape the prompt asked for"
            );
        }
        Verdict::Obeyed => println!("OBEYED  - the answer follows the strict schema"),
        Verdict::Ignored(why) => println!("IGNORED - answered, but {why}"),
        Verdict::Rejected(why) => println!("REJECTED - {why}"),
        Verdict::Unavailable(why) => println!("UNAVAILABLE (no verdict, retry later) - {why}"),
    }
    std::process::exit(i32::from(verdict != Verdict::Obeyed));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(content: &str) -> String {
        json!({"choices": [{"message": {"content": content}}]}).to_string()
    }

    const GOOD: &str = r#"{"city":"Paris","population_millions":2,"landmarks":[{"name":"Eiffel Tower","year_built":1889}],"mayor_note":null}"#;

    #[test]
    fn a_conforming_answer_is_obeyed() {
        assert_eq!(judge(200, &response(GOOD)), Verdict::Obeyed);
    }

    #[test]
    fn a_fenced_or_prose_answer_is_ignored() {
        let fenced = format!("```json\n{GOOD}\n```");
        assert!(matches!(
            judge(200, &response(&fenced)),
            Verdict::Ignored(_)
        ));
        assert!(matches!(
            judge(200, &response("Paris is nice")),
            Verdict::Ignored(_)
        ));
    }

    #[test]
    fn a_wrong_shape_is_ignored_and_says_why() {
        let missing = r#"{"city":"Paris","population_millions":2,"landmarks":[]}"#;
        let Verdict::Ignored(why) = judge(200, &response(missing)) else {
            panic!("expected Ignored");
        };
        assert!(why.contains("mayor_note"), "{why}");
        let extra = GOOD.replace("\"city\"", "\"extra\":1,\"city\"");
        assert!(matches!(judge(200, &response(&extra)), Verdict::Ignored(_)));
        let wrong_type = GOOD.replace("\"population_millions\":2", "\"population_millions\":\"2\"");
        assert!(matches!(
            judge(200, &response(&wrong_type)),
            Verdict::Ignored(_)
        ));
        let landmark = GOOD.replace("\"year_built\":1889", "\"year_built\":\"1889\"");
        assert!(matches!(
            judge(200, &response(&landmark)),
            Verdict::Ignored(_)
        ));
    }

    #[test]
    fn an_error_status_is_a_rejection_with_the_body() {
        let Verdict::Rejected(why) = judge(400, r#"{"error":"unknown field response_format"}"#)
        else {
            panic!("expected Rejected");
        };
        assert!(
            why.contains("400") && why.contains("response_format"),
            "{why}"
        );
    }

    #[test]
    fn a_rate_limit_or_server_error_is_no_verdict_on_the_schema() {
        assert!(matches!(judge(503, "overloaded"), Verdict::Unavailable(_)));
        assert!(matches!(judge(429, "slow down"), Verdict::Unavailable(_)));
    }

    #[test]
    fn a_response_without_content_is_ignored() {
        assert!(matches!(
            judge(200, r#"{"choices":[]}"#),
            Verdict::Ignored(_)
        ));
    }

    #[test]
    fn the_schema_is_strict_in_the_shape_the_provider_sends() {
        let schema = schema();
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(schema["required"].as_array().unwrap().len(), 4);
        assert!(schema["properties"]["mayor_note"]["anyOf"].is_array());
    }
}
