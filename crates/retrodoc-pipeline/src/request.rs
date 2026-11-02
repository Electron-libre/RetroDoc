//! What an LLM pass sends besides its input (ADR 0040): its system prompt, the JSON schema of its
//! answer type and the fixed text of its user prompt, declared once per pass as a [`Request`]. The
//! call and the fingerprint both take it, so what is sent and what is compared come from the same
//! place: a change of any of them redoes the units of the pass once.

use std::marker::PhantomData;
use std::sync::OnceLock;

use retrodoc_llm::LlmProvider;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;

use crate::error::PipelineError;
use crate::fingerprints::prompt_part;
use crate::response::{complete_json, response_schema};

/// The request of a pass whose answer is of type `R`. A text of the user prompt is one of
/// `user_texts`, never written inline where the prompt is built, so that it is in the print.
pub(crate) struct Request<R> {
    pub system: &'static str,
    /// The fixed text of the user prompt, in any order.
    pub user_texts: &'static [&'static str],
    part: OnceLock<String>,
    answer: PhantomData<fn() -> R>,
}

impl<R> Request<R> {
    pub(crate) const fn new(system: &'static str, user_texts: &'static [&'static str]) -> Self {
        Self {
            system,
            user_texts,
            part: OnceLock::new(),
            answer: PhantomData,
        }
    }
}

impl<R: DeserializeOwned + JsonSchema> Request<R> {
    /// The part of a fingerprint the request makes: the hash of the system prompt, of the schema
    /// sent with it and of the user prompt texts, computed once per process.
    pub(crate) fn part(&self) -> &str {
        self.part.get_or_init(|| {
            let schema = response_schema::<R>();
            let schema = format!("schema {} {}", schema.name, schema.schema);
            let texts: Vec<&str> = [self.system, schema.as_str()]
                .into_iter()
                .chain(self.user_texts.iter().copied())
                .collect();
            prompt_part(&texts)
        })
    }

    /// Asks the request with `user` as its user prompt ([`complete_json`]).
    ///
    /// # Errors
    ///
    /// Returns an error only if the LLM call itself fails.
    pub(crate) async fn ask(
        &self,
        llm: &dyn LlmProvider,
        user: &str,
        what: &str,
    ) -> Result<Option<R>, PipelineError> {
        complete_json::<R>(llm, self.system, user, what).await
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    #[derive(Deserialize, JsonSchema)]
    struct Answer {
        #[allow(dead_code)]
        n: u32,
    }

    #[derive(Deserialize, JsonSchema)]
    #[schemars(rename = "Answer")]
    struct Widened {
        #[allow(dead_code)]
        n: u32,
        #[allow(dead_code)]
        back_office: bool,
    }

    // spec: RER-10 (internal: the request of a pass is a constant of the binary, a test can't change it)
    #[test]
    fn the_part_of_a_request_changes_with_what_it_sends() {
        let base = Request::<Answer>::new("system", &["Files:\n"]);
        assert_eq!(
            base.part(),
            Request::<Answer>::new("system", &["Files:\n"]).part()
        );
        for other in [
            Request::<Widened>::new("system", &["Files:\n"])
                .part()
                .to_string(),
            Request::<Answer>::new("system", &["Model files:\n"])
                .part()
                .to_string(),
            Request::<Answer>::new("other system", &["Files:\n"])
                .part()
                .to_string(),
        ] {
            assert_ne!(base.part(), other);
        }
    }
}
