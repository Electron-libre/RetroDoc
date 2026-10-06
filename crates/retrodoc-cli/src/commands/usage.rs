//! Token accounting of the commands that call the LLM: the provider they
//! share (heartbeat over the usage counter over `OpenRouter`) and the recap
//! printed and saved when the command ends.

use std::path::Path;

use anyhow::Context;
use retrodoc_core::config::LlmConfig;
use retrodoc_llm::{HeartbeatProvider, OpenRouterProvider, UsageProvider, UsageTracker};
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
