//! Token accounting of the commands that call the LLM: the provider they
//! share (heartbeat over the usage counter over `OpenRouter`) and the recap
//! printed and saved when the command ends.

use std::path::Path;
use std::sync::Arc;

use anyhow::Context;
use retrodoc_core::config::LlmConfig;
use retrodoc_llm::{
    HeartbeatProvider, LlmProvider, OpenRouterProvider, UsageProvider, UsageTracker,
};
use retrodoc_pipeline::usage_log::{self, RunUsage};

/// The LLM provider of a command, counting its calls in `tracker`.
pub fn provider(
    config: &LlmConfig,
    tracker: &UsageTracker,
) -> anyhow::Result<HeartbeatProvider<UsageProvider<OpenRouterProvider>>> {
    let inner = OpenRouterProvider::from_config(config)
        .context("could not initialize the LLM provider (missing API key?)")?;
    Ok(HeartbeatProvider::new(UsageProvider::new(
        inner,
        tracker.clone(),
    )))
}

/// The provider of every pass, sharing one when two passes have the same
/// settings (`[llm.passes.<name>]` over `[llm]`, ADR 0023).
pub struct PassProviders {
    opened: Vec<(String, Arc<dyn LlmProvider>)>,
}

impl PassProviders {
    /// Builds the providers of `passes` now, so a missing API key stops the
    /// command before any call, naming the pass that needs it.
    ///
    /// # Errors
    ///
    /// Returns an error if a provider cannot be initialized.
    pub fn open(
        config: &LlmConfig,
        tracker: &UsageTracker,
        passes: &[&str],
    ) -> anyhow::Result<Self> {
        let mut built: Vec<(LlmConfig, Arc<dyn LlmProvider>)> = Vec::new();
        let mut opened = Vec::new();
        for &pass in passes {
            let pass_config = config.for_pass(pass);
            let shared = built.iter().find(|(known, _)| *known == pass_config);
            let provider = if let Some((_, provider)) = shared {
                Arc::clone(provider)
            } else {
                let provider: Arc<dyn LlmProvider> = Arc::new(
                    provider(&pass_config, tracker)
                        .with_context(|| format!("LLM settings of the `{pass}` pass"))?,
                );
                built.push((pass_config, Arc::clone(&provider)));
                provider
            };
            opened.push((pass.to_string(), provider));
        }
        Ok(Self { opened })
    }

    /// Providers given by name, for tests that script the answers of the
    /// LLM.
    #[cfg(test)]
    pub fn from_providers(opened: Vec<(&str, Arc<dyn LlmProvider>)>) -> Self {
        Self {
            opened: opened
                .into_iter()
                .map(|(name, provider)| (name.to_string(), provider))
                .collect(),
        }
    }

    /// The provider of `pass`.
    ///
    /// # Errors
    ///
    /// Returns an error if `pass` was not given to [`PassProviders::open`].
    pub fn get(&self, pass: &str) -> anyhow::Result<&Arc<dyn LlmProvider>> {
        self.opened
            .iter()
            .find(|(name, _)| name == pass)
            .map(|(_, provider)| provider)
            .with_context(|| format!("no LLM provider opened for the `{pass}` pass"))
    }
}

/// The provider of the single pass `pass`, for the commands that run one.
///
/// # Errors
///
/// Returns an error if the provider cannot be initialized.
pub fn pass_provider(
    config: &LlmConfig,
    tracker: &UsageTracker,
    pass: &str,
) -> anyhow::Result<Arc<dyn LlmProvider>> {
    let providers = PassProviders::open(config, tracker, &[pass])?;
    Ok(Arc::clone(providers.get(pass)?))
}

/// Prints the recap of a command run on `path` and adds the run to
/// `.retrodoc/cache/usage.json`. A failed command that never reached the LLM
/// prints nothing (there is nothing to report); a failed one that did still
/// shows what it spent. A run without any call is not saved. Saving is
/// best effort: it never turns a finished command into a failure.
pub fn finish(path: &Path, command: &str, tracker: &UsageTracker, succeeded: bool) {
    tracker.end_pass();
    let report = tracker.report();
    let calls = report.total().calls;
    if calls == 0 && !succeeded {
        return;
    }
    print!("\n{}", usage_log::recap(&report));
    if calls == 0 {
        return;
    }
    let Ok(repo_root) = path.canonicalize() else {
        return;
    };
    let run = RunUsage::new(command, &chrono::Utc::now().to_rfc3339(), &report);
    if let Err(error) = usage_log::record_run(&repo_root, run) {
        tracing::warn!("could not save the usage history: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use retrodoc_core::config::Config;

    fn config(raw: &str) -> LlmConfig {
        toml::from_str::<Config>(raw).unwrap().llm
    }

    #[test]
    fn passes_with_the_same_settings_share_a_provider() {
        std::env::set_var("RETRODOC_TEST_PASS_KEY", "unused");
        let llm = config(
            r#"
            [llm]
            api_key_env = "RETRODOC_TEST_PASS_KEY"
            [llm.passes.domains]
            model = "strong"
            [llm.passes.features]
            model = "strong"
            "#,
        );
        let passes = ["glossary", "domains", "features", "roles"];
        let providers = PassProviders::open(&llm, &UsageTracker::new(), &passes).unwrap();
        let get = |name| providers.get(name).unwrap();
        assert!(Arc::ptr_eq(get("domains"), get("features")));
        assert!(Arc::ptr_eq(get("glossary"), get("roles")));
        assert!(!Arc::ptr_eq(get("glossary"), get("domains")));
        assert!(providers.get("actors").is_err());
    }

    #[test]
    fn a_missing_key_names_the_pass_that_needs_it() {
        std::env::set_var("RETRODOC_TEST_PASS_KEY", "unused");
        std::env::remove_var("RETRODOC_TEST_PASS_MISSING");
        let llm = config(
            r#"
            [llm]
            api_key_env = "RETRODOC_TEST_PASS_KEY"
            [llm.passes.domains]
            api_key_env = "RETRODOC_TEST_PASS_MISSING"
            "#,
        );
        let error = PassProviders::open(&llm, &UsageTracker::new(), &["glossary", "domains"])
            .err()
            .expect("the key of `domains` is missing");
        assert!(format!("{error:#}").contains("`domains`"), "{error:#}");
    }
}
