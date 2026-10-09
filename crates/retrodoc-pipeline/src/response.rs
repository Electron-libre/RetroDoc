//! Helpers shared by the passes that ask the LLM for a JSON answer.
//!
//! Small or local models often fail to follow "reply with ONLY a JSON
//! object": they add prose, wrap the answer in code fences, emit several
//! fenced blocks, append trailing text, or repeat a field. The helpers here
//! accept the first JSON value found and the last of a repeated field, and
//! [`complete_json`] retries once before giving up.

use retrodoc_llm::{ChatMessage, CompletionRequest, LlmProvider, ResponseSchema, Role};
use schemars::{generate::SchemaSettings, JsonSchema};
use serde::de::DeserializeOwned;

use crate::error::PipelineError;

/// Parses the first JSON value of type `T` found in an LLM answer,
/// tolerating code fences, surrounding prose and trailing content, and a
/// field written twice in the same object (the last one wins).
pub(crate) fn parse_json_response<T: DeserializeOwned>(raw: &str) -> Result<T, PipelineError> {
    let json = strip_code_fence(raw);
    serde_json::Deserializer::from_str(json)
        .into_iter::<T>()
        .next()
        .unwrap_or_else(|| serde_json::from_str(json))
        .or_else(|err| {
            // `serde_json` rejects a duplicated field when deserializing
            // straight into a struct, but a `Value` keeps the last one.
            if !err.to_string().contains("duplicate field") {
                return Err(err);
            }
            let value = serde_json::Deserializer::from_str(json)
                .into_iter::<serde_json::Value>()
                .next()
                .ok_or_else(|| {
                    <serde_json::Error as serde::de::Error>::custom("no JSON value")
                })??;
            serde_json::from_value(value)
        })
        .map_err(|source| PipelineError::ResponseParse {
            raw: raw.to_string(),
            source,
        })
}

/// Narrows an LLM answer down to the text where its JSON starts: the body
/// of the first Markdown code fence if there is one (several fenced blocks
/// are common, only the first is kept), otherwise everything from the first
/// `{`.
pub(crate) fn strip_code_fence(raw: &str) -> &str {
    let trimmed = raw.trim();
    if let Some((_, after_open)) = trimmed.split_once("```") {
        // Skip the language tag (`json`) up to the end of the fence line.
        let body = after_open
            .split_once('\n')
            .map_or(after_open, |(_, rest)| rest);
        return body.split("```").next().unwrap_or(body).trim();
    }
    trimmed.find('{').map_or(trimmed, |start| &trimmed[start..])
}

/// One system + user exchange with the default model, answer as is.
///
/// # Errors
///
/// Returns an error if the LLM call fails.
pub(crate) async fn complete_text(
    llm: &dyn LlmProvider,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String, PipelineError> {
    complete_with_schema(llm, system_prompt, user_prompt, None).await
}

async fn complete_with_schema(
    llm: &dyn LlmProvider,
    system_prompt: &str,
    user_prompt: &str,
    json_schema: Option<&ResponseSchema>,
) -> Result<String, PipelineError> {
    let response = llm
        .complete(CompletionRequest {
            messages: vec![
                ChatMessage {
                    role: Role::System,
                    content: system_prompt.to_string(),
                },
                ChatMessage {
                    role: Role::User,
                    content: user_prompt.to_string(),
                },
            ],
            model: None,
            json_schema: json_schema.cloned(),
        })
        .await?;
    Ok(response.content)
}

/// The schema of the answer type `T`, in the shape strict structured outputs
/// accept: subschemas inline, every property required (a field with a
/// `#[serde(default)]` is still always asked for), no extra property, no
/// `default` and `anyOf` in place of `oneOf`.
pub(crate) fn response_schema<T: JsonSchema>() -> ResponseSchema {
    let generator = SchemaSettings::draft2020_12()
        .with(|s| s.inline_subschemas = true)
        .into_generator();
    let mut schema = generator.into_root_schema_for::<T>().to_value();
    if let Some(root) = schema.as_object_mut() {
        root.remove("$schema");
    }
    make_strict(&mut schema);
    let name: String = T::schema_name()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    ResponseSchema { name, schema }
}

fn make_strict(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(serde_json::Value::Object(properties)) = map.get("properties") {
                let required: Vec<serde_json::Value> = properties
                    .keys()
                    .map(|k| serde_json::Value::String(k.clone()))
                    .collect();
                map.insert("required".to_string(), required.into());
                map.insert("additionalProperties".to_string(), false.into());
            }
            // Strict mode takes neither `default` nor `oneOf`, only `anyOf`.
            map.remove("default");
            if let Some(one_of) = map.remove("oneOf") {
                map.insert("anyOf".to_string(), one_of);
            }
            for (key, child) in map.iter_mut() {
                if key == "properties" {
                    // Its keys are field names, not keywords: only the schemas
                    // they hold are visited.
                    if let serde_json::Value::Object(fields) = child {
                        fields.values_mut().for_each(make_strict);
                    }
                } else {
                    make_strict(child);
                }
            }
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(make_strict),
        _ => {}
    }
}

/// Asks the LLM for a JSON answer and parses it, retrying once if the
/// answer can't be parsed. `Ok(None)` means both attempts were unparseable
/// (logged as a warning for `what`): the caller skips that unit instead of
/// aborting the run.
///
/// # Errors
///
/// Returns an error only if the LLM call itself fails.
pub(crate) async fn complete_json<T: DeserializeOwned + JsonSchema>(
    llm: &dyn LlmProvider,
    system_prompt: &str,
    user_prompt: &str,
    what: &str,
) -> Result<Option<T>, PipelineError> {
    const ATTEMPTS: u32 = 2;
    let schema = response_schema::<T>();
    for attempt in 1..=ATTEMPTS {
        let response = complete_with_schema(llm, system_prompt, user_prompt, Some(&schema)).await?;
        match parse_json_response(&response) {
            Ok(parsed) => return Ok(Some(parsed)),
            Err(err) => {
                // The error's Display embeds the whole raw answer: keep the
                // warning short and leave the raw text to the debug level.
                let reason = match &err {
                    PipelineError::ResponseParse { source, .. } => source.to_string(),
                    other => other.to_string(),
                };
                tracing::debug!(what, raw = %response, "unparseable LLM response (raw)");
                tracing::warn!(
                    what,
                    attempt,
                    error = %reason,
                    answer_chars = response.chars().count(),
                    "unparseable LLM response{}",
                    if attempt < ATTEMPTS { ", retrying" } else { ", skipped" }
                );
            }
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde::Deserialize;

    use crate::testing::FakeLlm;

    #[derive(Debug, Deserialize, PartialEq, schemars::JsonSchema)]
    struct Answer {
        n: u32,
    }

    #[test]
    fn takes_the_first_fenced_block_and_ignores_the_rest() {
        let raw = "```json\n{\"n\":1}\n```\n```json\n{\"n\":2}\n```";
        assert_eq!(parse_json_response::<Answer>(raw).unwrap(), Answer { n: 1 });
    }

    #[test]
    fn tolerates_prose_and_trailing_content() {
        let raw = "Sure! Here you go: {\"n\":3} hope that helps {oops";
        assert_eq!(parse_json_response::<Answer>(raw).unwrap(), Answer { n: 3 });
    }

    #[test]
    fn tolerates_an_unclosed_fence() {
        let raw = "```json\n{\"n\":4}";
        assert_eq!(parse_json_response::<Answer>(raw).unwrap(), Answer { n: 4 });
    }

    #[test]
    fn keeps_the_last_of_a_duplicated_field() {
        let raw = "{\"n\":1,\"n\":2}";
        assert_eq!(parse_json_response::<Answer>(raw).unwrap(), Answer { n: 2 });
    }

    #[tokio::test]
    async fn complete_json_does_not_retry_on_a_duplicated_field() {
        let provider = FakeLlm::answering("{\"n\":1,\"n\":2}");
        let got = complete_json::<Answer>(&provider, "s", "u", "test")
            .await
            .unwrap();
        assert_eq!(got, Some(Answer { n: 2 }));
        assert_eq!(provider.calls(), 1);
    }

    #[test]
    fn rejects_an_answer_without_json() {
        assert!(parse_json_response::<Answer>("I cannot do that").is_err());
    }

    #[tokio::test]
    async fn complete_json_retries_once_then_succeeds() {
        let provider = FakeLlm::sequence(&["garbage", "{\"n\":5}"]);
        let got = complete_json::<Answer>(&provider, "s", "u", "test")
            .await
            .unwrap();
        assert_eq!(got, Some(Answer { n: 5 }));
        assert_eq!(provider.calls(), 2);
    }

    #[tokio::test]
    async fn complete_json_gives_up_after_two_attempts() {
        let provider = FakeLlm::answering("garbage");
        let got = complete_json::<Answer>(&provider, "s", "u", "test")
            .await
            .unwrap();
        assert_eq!(got, None);
        assert_eq!(provider.calls(), 2);
    }

    #[derive(Debug, Deserialize, schemars::JsonSchema)]
    #[allow(dead_code)] // only its schema is read
    struct Nested {
        label: String,
        #[serde(default)]
        tags: Vec<String>,
        #[serde(default)]
        note: Option<String>,
        #[serde(default)]
        items: Vec<Answer>,
    }

    #[tokio::test]
    async fn complete_json_sends_the_schema_of_the_answer_type() {
        let provider = FakeLlm::answering("{\"n\":1}");
        complete_json::<Answer>(&provider, "s", "u", "test")
            .await
            .unwrap();
        let schemas = provider.schemas();
        let schema = schemas[0].as_ref().expect("a schema is sent");
        assert_eq!(schema["properties"]["n"]["type"], "integer");
    }

    #[tokio::test]
    async fn a_retry_sends_the_schema_again() {
        let provider = FakeLlm::sequence(&["not json", "{\"n\":1}"]);
        complete_json::<Answer>(&provider, "s", "u", "test")
            .await
            .unwrap();
        assert!(provider.schemas().iter().all(Option::is_some));
        assert_eq!(provider.schemas().len(), 2);
    }

    #[test]
    fn the_schema_is_strict_and_self_contained() {
        let schema = response_schema::<Nested>();
        assert_eq!(schema.name, "Nested");
        let text = schema.schema.to_string();
        assert!(
            !text.contains("$ref") && !text.contains("$schema"),
            "{text}"
        );

        // Defaulted fields are asked for too, and nothing else is allowed.
        let root = &schema.schema;
        assert_eq!(root["additionalProperties"], false);
        let mut required: Vec<&str> = root["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        required.sort_unstable();
        assert_eq!(required, ["items", "label", "note", "tags"]);
        // The nested object is strict as well.
        let item = &root["properties"]["items"]["items"];
        assert_eq!(item["additionalProperties"], false);
        assert_eq!(item["required"], serde_json::json!(["n"]));
    }
}
