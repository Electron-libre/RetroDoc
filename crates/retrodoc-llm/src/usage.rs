//! Token accounting: what a completion cost, as the server reports it.
//! Nothing here estimates tokens; a call whose server sent no (or an
//! incomplete) `usage` block simply has no [`Usage`].

use serde::Deserialize;

/// Token counts of one completion, as reported by the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Usage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
}

/// The `usage` block of an OpenAI-compatible chat completion. Both counters
/// are optional so an incomplete block parses; it is then dropped by
/// [`ApiUsage::into_usage`].
#[derive(Debug, Deserialize)]
struct ApiUsage {
    #[serde(default)]
    prompt_tokens: Option<u64>,
    #[serde(default)]
    completion_tokens: Option<u64>,
}

impl ApiUsage {
    fn into_usage(self) -> Option<Usage> {
        Some(Usage {
            prompt_tokens: self.prompt_tokens?,
            completion_tokens: self.completion_tokens?,
        })
    }
}

/// Reads the raw `usage` value of a response. Accounting must never cost a
/// valid answer: a block that is missing, incomplete or malformed (float,
/// negative or textual counters) is `None`, not a parse error.
pub(crate) fn parse(raw: Option<serde_json::Value>) -> Option<Usage> {
    // Serde would read a JSON array as the struct's fields in order.
    let raw = raw.filter(serde_json::Value::is_object)?;
    serde_json::from_value::<ApiUsage>(raw).ok()?.into_usage()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str) -> Option<Usage> {
        super::parse(Some(serde_json::from_str(json).unwrap()))
    }

    #[test]
    fn both_counters_make_a_usage() {
        assert_eq!(
            parse(r#"{"prompt_tokens":120,"completion_tokens":35,"total_tokens":155}"#),
            Some(Usage {
                prompt_tokens: 120,
                completion_tokens: 35
            })
        );
    }

    #[test]
    fn a_malformed_block_is_absent_not_an_error() {
        for json in [
            r#"{"prompt_tokens":12.5,"completion_tokens":3}"#,
            r#"{"prompt_tokens":-1,"completion_tokens":3}"#,
            r#"{"prompt_tokens":"12","completion_tokens":3}"#,
            r#""12 tokens""#,
            "[1,2]",
        ] {
            assert_eq!(parse(json), None, "{json}");
        }
        assert_eq!(super::parse(None), None);
    }

    #[test]
    fn a_block_missing_a_counter_is_not_trusted() {
        // Some local servers send `usage: {}` or null counters; treat the
        // whole block as absent rather than counting a half-known call.
        for json in [
            "{}",
            r#"{"prompt_tokens":10}"#,
            r#"{"completion_tokens":10}"#,
            r#"{"prompt_tokens":null,"completion_tokens":3}"#,
            "null",
        ] {
            assert_eq!(parse(json), None, "{json}");
        }
    }
}
