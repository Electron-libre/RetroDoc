//! The language of the generated docs (ADR 0041, `specs/docs-language.md`): with `[output] language`
//! set, the provider of each pass of [`WRITING_PASSES`] is wrapped in an [`InLanguage`], which ends the
//! system prompt of every request with one instruction naming the language. Its model is that of the
//! provider it wraps followed by the language, so the passes that print their model ask their units
//! again when the language changes; the brief, whose print holds no model, prints the language itself.

use std::sync::Arc;

use async_trait::async_trait;
use retrodoc_llm::{CompletionRequest, CompletionResponse, LlmError, LlmProvider, Role};

/// The passes that write text a reader sees. The others (`sources`, `roles`, `business-files`, the
/// benchmark judge) read the repository and keep the plain provider.
pub const WRITING_PASSES: &[&str] = &[
    "brief",
    "contexts",
    "glossary",
    "entry-points",
    "actors",
    "domains",
    "features",
    "use-cases",
    "rules",
    "confidence",
    "reconcile",
];

/// The instruction that ends the system prompt of a writing pass asked in `language`.
#[must_use]
pub fn instruction(language: &str) -> String {
    format!(
        "Write every free text of your answer (names, descriptions, narratives, steps, rules, \
         definitions, reasons) in {language}. Keep exactly as given the code identifiers, the file \
         paths and the names this prompt gives you."
    )
}

/// A provider whose requests are asked in a language.
pub struct InLanguage {
    inner: Arc<dyn LlmProvider>,
    language: String,
    instruction: String,
    model: String,
}

impl InLanguage {
    #[must_use]
    pub fn new(inner: Arc<dyn LlmProvider>, language: &str) -> Self {
        let model = format!("{} in {language}", inner.model());
        Self {
            inner,
            language: language.to_string(),
            instruction: instruction(language),
            model,
        }
    }
}

#[async_trait]
impl LlmProvider for InLanguage {
    async fn complete(
        &self,
        mut request: CompletionRequest,
    ) -> Result<CompletionResponse, LlmError> {
        if let Some(system) = request
            .messages
            .iter_mut()
            .find(|message| message.role == Role::System)
        {
            system.content = format!("{}\n\n{}", system.content, self.instruction);
        }
        self.inner.complete(request).await
    }

    fn note_unparseable_answer(&self, skipped: bool) {
        self.inner.note_unparseable_answer(skipped);
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn language(&self) -> Option<&str> {
        Some(&self.language)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::response::complete_json;
    use crate::testing::FakeLlm;

    #[derive(serde::Deserialize, schemars::JsonSchema)]
    struct Answer {
        #[allow(dead_code)]
        n: u32,
    }

    // spec: DLG-3 (internal: the exact system prompt sent is only visible to a scripted provider)
    #[tokio::test]
    async fn a_request_asked_in_a_language_ends_its_system_prompt_with_the_instruction() {
        let fake = Arc::new(FakeLlm::answering(r#"{"n": 1}"#));
        let llm = InLanguage::new(fake.clone(), "French");

        complete_json::<Answer>(&llm, "Name the domains.", "Files:", "test")
            .await
            .unwrap();

        let system = &fake.system_prompts()[0];
        assert!(system.starts_with("Name the domains."), "{system}");
        assert!(system.ends_with(&instruction("French")), "{system}");
        assert!(instruction("French").contains("in French"));
        assert_eq!(fake.prompts()[0], "Files:");
    }

    // spec: DLG-4 (internal: the passes print the model of their provider, which carries the language)
    #[test]
    fn the_model_of_a_provider_in_a_language_names_it() {
        let fake: Arc<dyn LlmProvider> = Arc::new(FakeLlm::answering("{}"));
        let french = InLanguage::new(Arc::clone(&fake), "French");
        let german = InLanguage::new(Arc::clone(&fake), "German");
        assert_eq!(french.model(), "test-model in French");
        assert_ne!(french.model(), german.model());
        assert_eq!(french.language(), Some("French"));
        assert_eq!(fake.language(), None);
    }
}
