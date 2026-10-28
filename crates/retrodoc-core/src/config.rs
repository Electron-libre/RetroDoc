//! `retrodoc.toml` configuration.
//!
//! The file lives at the root of the analyzed repo and describes the LLM
//! provider (`OpenRouter` in v1) as well as the ingestion settings (paths
//! ignored in addition to `.gitignore`, doc output folder).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const CONFIG_FILE_NAME: &str = "retrodoc.toml";

/// Keys `retrodoc.toml` no longer takes, as `(section, key, why)`: a file that still names one is refused with a
/// message saying so, rather than a bare unknown-field error or a key silently ignored
/// (`specs/pipeline-settings.md`).
const REMOVED_KEYS: &[(&str, &str, &str)] = &[
    (
        "pipeline",
        "body_lines",
        "the reading passes read the files whole (ADR 0027)",
    ),
    ("llm", "batch_chars", FILES_PATH_REMOVED),
];

/// Why the settings of the `files` path of `generate` are refused.
const FILES_PATH_REMOVED: &str =
    "the domains are built from the surface only, the file summaries are gone (ADR 0035)";

/// Default model proposed at init. Can be changed in `retrodoc.toml`.
pub const DEFAULT_MODEL: &str = "anthropic/claude-sonnet-4.5";

/// The passes that can have their own `[llm.passes.<name>]` section: the
/// names `UsageTracker` counts them under.
pub const PASS_NAMES: &[&str] = &[
    "sources",
    "brief",
    "roles",
    "business-files",
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
    "benchmark judge",
];

/// The passes asked at temperature 0 unless the configuration sets one (ADR 0031,
/// `issues/done/sampling_temperature_for_steadier_runs.md`): the ones that read facts out of the code and the
/// repository, and the ones that write the docs. Left to the server's default: `contexts` (not measured) and the
/// benchmark judge.
pub const ZERO_TEMPERATURE_PASSES: &[&str] = &[
    "sources",
    "brief",
    "roles",
    "business-files",
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub llm: LlmConfig,
    #[serde(default)]
    pub ingest: IngestConfig,
    #[serde(default)]
    pub output: OutputConfig,
    #[serde(default)]
    pub brief: BriefConfig,
    #[serde(default)]
    pub pipeline: PipelineConfig,
    #[serde(default, skip_serializing_if = "RulesConfig::is_empty")]
    pub rules: RulesConfig,
}

/// `[rules]`: what the user puts back of what the rules pass set aside on a judgement of the LLM
/// (`specs/rules-set-aside.md`). In `retrodoc.toml`, not in the cache, so `generate --force` keeps it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RulesConfig {
    /// Slugs of the features not to take for features about tests: their rules are stated.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keep_features: Vec<String>,
    /// Slugs (`<feature-slug>-<n>`) of the rules the judgement dropped as noise, published anyway.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keep_rules: Vec<String>,
}

impl RulesConfig {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.keep_features.is_empty() && self.keep_rules.is_empty()
    }
}

/// `[pipeline]`: which path `generate` takes (ADR 0020).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PipelineConfig {
    /// What the domains are built from. `surface` (default, ADR 0034): the entities and
    /// the entry points grouped by resource (ADR 0020). `contexts`: the
    /// bounded contexts of `retrodoc contexts` (ADR 0029), each a domain, documented a slice at a time.
    /// `files`, the LLM summaries of the files and directories, was removed (ADR 0035).
    #[serde(default)]
    pub domains_from: DomainsFrom,
    /// `generate` reads the business files once for the glossary and the entry
    /// points (one call answers both, ADR 0028). `false`: each pass reads them
    /// on its own, as before; for a small library, where the joint reading finds
    /// fewer entry points.
    #[serde(default = "default_true")]
    pub business_read: bool,
    /// `generate` states the business rules of each feature after its use cases (one LLM call per
    /// feature, ADR 0021). `false`: no rules, for a cheaper run (the pass is about 40% of the input
    /// tokens of a run).
    #[serde(default = "default_true")]
    pub rules: bool,
    /// `generate` ends by asking the LLM whether the doc sections that match an item cover or
    /// contradict it (`specs/docs-reconciliation.md`, ADR 0033); no call when nothing matches.
    /// `false`: the report only says which items no doc matches.
    #[serde(default = "default_true")]
    pub reconcile: bool,
}

fn default_true() -> bool {
    true
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            domains_from: DomainsFrom::default(),
            business_read: true,
            rules: true,
            reconcile: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DomainsFrom {
    #[default]
    Surface,
    /// The bounded contexts of `contexts.yaml` (ADR 0029), documented a slice at a time.
    Contexts,
}

/// How the product brief (ADR 0019) is used.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BriefConfig {
    /// Also give the features and use cases passes, per unit, the few doc
    /// sections, test descriptions and commit subjects closest to it. On by
    /// default: the replays of `issues/done/brief_injection_experiments.md` showed
    /// merged duplicate features and more business-like use cases, for about
    /// a tenth more prompt tokens.
    #[serde(default = "default_true")]
    pub evidence: bool,
}

impl Default for BriefConfig {
    fn default() -> Self {
        Self { evidence: true }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LlmConfig {
    /// LLM provider: "openrouter" (default) or "deepseek" (its own API; the key
    /// is then read from `DEEPSEEK_API_KEY` unless `api_key_env` names another).
    #[serde(default = "LlmConfig::default_provider")]
    pub provider: String,
    /// Name of the environment variable holding the API key (never the key
    /// itself: this file is versioned alongside the analyzed repo).
    #[serde(default = "LlmConfig::default_api_key_env")]
    pub api_key_env: String,
    /// `OpenRouter` model to use (e.g. "anthropic/claude-sonnet-4.5").
    #[serde(default = "LlmConfig::default_model")]
    pub model: String,
    /// Overrides the chat-completions endpoint `OpenRouterProvider` calls.
    /// Unset: `OpenRouter`'s own endpoint. Set: any server speaking the
    /// same OpenAI-compatible chat-completions wire format — e.g. a local
    /// Ollama/LM Studio/llama.cpp instance — since `OpenRouterProvider`'s
    /// request/response shapes are that same protocol, not `OpenRouter`-
    /// specific. Still not "multi-provider support" (out of scope for v1,
    /// PLAN.md §1): the auth model and client stay `OpenRouterProvider`'s.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// Sent as `reasoning_effort` with every request when set (e.g.
    /// `"none"`, `"low"`). Lets a "thinking" model (Qwen3, …) skip its
    /// internal reasoning, which is far too slow for `RetroDoc`'s many short
    /// structured calls. Unset: nothing is sent, the server's default applies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
    /// Sampling temperature sent with every request when set (0 gives the
    /// steadiest runs). Unset: the passes of [`ZERO_TEMPERATURE_PASSES`] use
    /// 0 and the others send nothing, so the server's default applies. A
    /// server that refuses it is asked again without it. Not part of the
    /// fingerprints: a unit already saved is not redone when it changes
    /// (`generate --force` does).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Per-request HTTP timeout in seconds. Unset: 900 when `base_url` names
    /// a loopback host (`localhost`, `127.0.0.0/8`, `::1`), 120 otherwise. A
    /// slow local model on another machine writing a long structured answer
    /// (several thousand tokens at a few tokens per second) needs it set; a
    /// timeout is not retried, the same prompt would take as long again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,
    /// Maximum number of LLM calls in flight at once for the passes that
    /// can run them in parallel (use cases, rules, confidence). Unset: 1.
    /// A single local model gains little; a hosted one (`OpenRouter`) a lot,
    /// within its rate limits (429s are retried with backoff).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub concurrency: Option<usize>,
    /// Ask the server to constrain JSON answers to a schema (`response_format`).
    /// Unset: `true`. A server that rejects it is detected (the request is sent
    /// again without the schema); `false` never sends it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured_output: Option<bool>,
    /// Settings of single passes (`[llm.passes.<name>]`, names in
    /// [`PASS_NAMES`]), each key falling back to this section (ADR 0023).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub passes: BTreeMap<String, PassLlmConfig>,
}

/// `[llm.passes.<name>]`: the keys of `[llm]`, each optional, that one pass
/// changes. Spend a strong model on the few framing passes and a cheap or
/// local one on the many extraction passes.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PassLlmConfig {
    pub provider: Option<String>,
    pub api_key_env: Option<String>,
    pub model: Option<String>,
    /// An empty string drops the `[llm]` `base_url`: the provider's own
    /// endpoint is used.
    pub base_url: Option<String>,
    pub reasoning_effort: Option<String>,
    pub temperature: Option<f32>,
    pub timeout_secs: Option<u64>,
    pub concurrency: Option<usize>,
    pub structured_output: Option<bool>,
}

impl LlmConfig {
    /// The settings of the pass `name`: `[llm]` with the keys of
    /// `[llm.passes.<name>]` on top. A pass that changes `provider` without
    /// naming `api_key_env` uses the key variable of that provider, not the
    /// inherited one. The result has no `passes` of its own.
    #[must_use]
    pub fn for_pass(&self, name: &str) -> LlmConfig {
        let mut config = LlmConfig {
            passes: BTreeMap::new(),
            ..self.clone()
        };
        if config.temperature.is_none() && ZERO_TEMPERATURE_PASSES.contains(&name) {
            config.temperature = Some(0.0);
        }
        let Some(pass) = self.passes.get(name) else {
            return config;
        };
        if let Some(provider) = &pass.provider {
            config.provider.clone_from(provider);
            if pass.api_key_env.is_none() {
                config.api_key_env = Self::default_api_key_env();
            }
        }
        if let Some(value) = &pass.api_key_env {
            config.api_key_env.clone_from(value);
        }
        if let Some(value) = &pass.model {
            config.model.clone_from(value);
        }
        match pass.base_url.as_deref() {
            Some("") => config.base_url = None,
            Some(url) => config.base_url = Some(url.to_string()),
            None => {}
        }
        if pass.reasoning_effort.is_some() {
            config.reasoning_effort.clone_from(&pass.reasoning_effort);
        }
        config.temperature = pass.temperature.or(config.temperature);
        config.timeout_secs = pass.timeout_secs.or(config.timeout_secs);
        config.concurrency = pass.concurrency.or(config.concurrency);
        config.structured_output = pass.structured_output.or(config.structured_output);
        config
    }

    /// The names under `[llm.passes]` that are no pass.
    fn unknown_passes(&self) -> Vec<&str> {
        self.passes
            .keys()
            .map(String::as_str)
            .filter(|name| !PASS_NAMES.contains(name))
            .collect()
    }
}

impl LlmConfig {
    fn default_provider() -> String {
        "openrouter".to_string()
    }
    fn default_api_key_env() -> String {
        "OPENROUTER_API_KEY".to_string()
    }
    fn default_model() -> String {
        DEFAULT_MODEL.to_string()
    }
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: Self::default_provider(),
            api_key_env: Self::default_api_key_env(),
            model: Self::default_model(),
            base_url: None,
            reasoning_effort: None,
            temperature: None,
            timeout_secs: None,
            concurrency: None,
            structured_output: None,
            passes: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestConfig {
    /// Extra patterns to ignore, in addition to `.gitignore` (gitignore syntax).
    #[serde(default)]
    pub extra_ignore: Vec<String>,
    /// Existing Markdown documentation folders to take as input.
    #[serde(default = "IngestConfig::default_existing_docs_paths")]
    pub existing_docs_paths: Vec<String>,
    /// Analyse at most this many source files (`generate --max-files`
    /// overrides): the best ranked by file role, git history and how many
    /// files mention them; the rest is left out and listed in the report.
    /// Unset: every source file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_files: Option<usize>,
}

impl IngestConfig {
    fn default_existing_docs_paths() -> Vec<String> {
        vec!["docs".to_string(), "README.md".to_string()]
    }
}

impl Default for IngestConfig {
    fn default() -> Self {
        Self {
            extra_ignore: Vec::new(),
            existing_docs_paths: Self::default_existing_docs_paths(),
            max_files: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    /// Output folder for the generated docs, relative to the repo root.
    #[serde(default = "OutputConfig::default_docs_dir")]
    pub docs_dir: String,
}

/// Folder of the functional documentation, under the docs dir.
pub const FUNCTIONAL_DIR: &str = "functional";
/// Folder of `RetroDoc`'s own files (coverage report, run metadata), under
/// the docs dir. Neither it nor [`FUNCTIONAL_DIR`] must be fed back as
/// existing documentation on the next run.
pub const META_DIR: &str = "_retrodoc";

impl OutputConfig {
    fn default_docs_dir() -> String {
        "docs".to_string()
    }

    /// The directories `RetroDoc` writes its docs into, relative to the repo root.
    #[must_use]
    pub fn generated_dirs(&self) -> [PathBuf; 2] {
        let docs_dir = Path::new(&self.docs_dir);
        [docs_dir.join(FUNCTIONAL_DIR), docs_dir.join(META_DIR)]
    }
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            docs_dir: Self::default_docs_dir(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("config file not found: {0}")]
    NotFound(String),
    #[error("error reading {path}: {source}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("error writing {path}: {source}")]
    Write {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid TOML config: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("{setting} was removed: {why}; {fix} in retrodoc.toml")]
    Removed {
        setting: String,
        why: &'static str,
        fix: &'static str,
    },
    #[error("unknown pass in [llm.passes]: {}; valid names: {}", .0.join(", "), PASS_NAMES.join(", "))]
    UnknownPass(Vec<String>),
    #[error("could not serialize TOML: {0}")]
    Serialize(#[from] toml::ser::Error),
}

/// The error for the first removed setting `raw` still names, if any: a key of [`REMOVED_KEYS`], or a
/// setting of the removed `files` path (ADR 0035).
fn removed_setting(raw: &str) -> Option<ConfigError> {
    let table: toml::Table = toml::from_str(raw).ok()?;
    let section = |name: &str| table.get(name).and_then(toml::Value::as_table);
    let removed = |setting: String, why, fix| ConfigError::Removed { setting, why, fix };
    if let Some(&(name, key, why)) = REMOVED_KEYS
        .iter()
        .find(|(name, key, _)| section(name).is_some_and(|t| t.contains_key(*key)))
    {
        return Some(removed(format!("[{name}] {key}"), why, "delete the line"));
    }
    if section("pipeline")
        .and_then(|t| t.get("domains_from"))
        .and_then(toml::Value::as_str)
        == Some("files")
    {
        return Some(removed(
            "[pipeline] domains_from = \"files\"".to_string(),
            FILES_PATH_REMOVED,
            "delete the line or set domains_from = \"surface\"",
        ));
    }
    let passes = section("llm")
        .and_then(|t| t.get("passes"))
        .and_then(toml::Value::as_table)?;
    if passes.contains_key("repo-map") {
        return Some(removed(
            "[llm.passes.repo-map]".to_string(),
            FILES_PATH_REMOVED,
            "delete the section",
        ));
    }
    passes
        .iter()
        .find(|(_, pass)| {
            pass.as_table()
                .is_some_and(|t| t.contains_key("batch_chars"))
        })
        .map(|(name, _)| {
            removed(
                format!("[llm.passes.{name}] batch_chars"),
                FILES_PATH_REMOVED,
                "delete the line",
            )
        })
}

impl Config {
    /// Loads the config from `<repo_root>/retrodoc.toml`.
    ///
    /// # Errors
    ///
    /// Returns an error if the file is missing, unreadable, or its TOML
    /// content is invalid or names a removed setting.
    pub fn load(repo_root: &Path) -> Result<Self, ConfigError> {
        let raw = Self::read(repo_root)?;
        if let Some(error) = removed_setting(&raw) {
            return Err(error);
        }
        let config: Config = toml::from_str(&raw)?;
        let unknown = config.llm.unknown_passes();
        if !unknown.is_empty() {
            return Err(ConfigError::UnknownPass(
                unknown.into_iter().map(str::to_string).collect(),
            ));
        }
        Ok(config)
    }

    /// The `[ingest]` and `[output]` of `<repo_root>/retrodoc.toml`, the other sections left to their
    /// defaults: how a saved run is read again (its files and its docs), whatever settings were removed
    /// since it was made (ADR 0035).
    ///
    /// # Errors
    ///
    /// Returns an error if the file is missing, unreadable, or its TOML content or those two sections are
    /// invalid.
    pub fn load_ingest_and_output(repo_root: &Path) -> Result<Self, ConfigError> {
        let table: toml::Table = toml::from_str(&Self::read(repo_root)?)?;
        let kept: toml::Table = table
            .into_iter()
            .filter(|(section, _)| section == "ingest" || section == "output")
            .collect();
        Ok(toml::Value::Table(kept).try_into()?)
    }

    /// The text of `<repo_root>/retrodoc.toml`.
    fn read(repo_root: &Path) -> Result<String, ConfigError> {
        let path = repo_root.join(CONFIG_FILE_NAME);
        if !path.exists() {
            return Err(ConfigError::NotFound(path.display().to_string()));
        }
        fs::read_to_string(&path).map_err(|source| ConfigError::Read {
            path: path.display().to_string(),
            source,
        })
    }

    /// Writes the default config to `<repo_root>/retrodoc.toml`.
    /// Fails if the file already exists (see `force` on the CLI side to overwrite).
    ///
    /// # Errors
    ///
    /// Returns an error if the file already exists without `force`, or if
    /// writing to disk fails.
    pub fn write_default(repo_root: &Path, force: bool) -> Result<Self, ConfigError> {
        let path = repo_root.join(CONFIG_FILE_NAME);
        if path.exists() && !force {
            return Err(ConfigError::Write {
                path: path.display().to_string(),
                source: std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    "retrodoc.toml already exists (use --force to overwrite)",
                ),
            });
        }
        let config = Config::default();
        let raw = toml::to_string_pretty(&config)?;
        fs::write(&path, raw).map_err(|source| ConfigError::Write {
            path: path.display().to_string(),
            source,
        })?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_roundtrips_through_toml() {
        let config = Config::default();
        let raw = toml::to_string_pretty(&config).unwrap();
        let parsed: Config = toml::from_str(&raw).unwrap();
        assert_eq!(parsed.llm.provider, config.llm.provider);
        assert_eq!(parsed.llm.model, config.llm.model);
        assert_eq!(parsed.output.docs_dir, config.output.docs_dir);
    }

    // spec: RSA-6
    #[test]
    fn the_corrections_of_the_rules_pass_are_read_from_retrodoc_toml_and_init_writes_none() {
        let raw = r#"
            [rules]
            keep_features = ["email-verification"]
            keep_rules = ["orders-2"]
        "#;
        let parsed: Config = toml::from_str(raw).unwrap();
        assert_eq!(parsed.rules.keep_features, ["email-verification"]);
        assert_eq!(parsed.rules.keep_rules, ["orders-2"]);
        // The config file is not under `.retrodoc/cache/`, the only folder `--force` clears.
        assert!(!CONFIG_FILE_NAME.contains(".retrodoc"));
        assert!(!toml::to_string_pretty(&Config::default())
            .unwrap()
            .contains("[rules]"));
        assert!(toml::from_str::<Config>("[rules]\nkeep = []\n").is_err());
    }

    #[test]
    fn partial_toml_falls_back_to_defaults() {
        let raw = r#"
            [llm]
            model = "openai/gpt-4o"
        "#;
        let parsed: Config = toml::from_str(raw).unwrap();
        assert_eq!(parsed.llm.model, "openai/gpt-4o");
        assert_eq!(parsed.llm.provider, "openrouter");
        assert_eq!(parsed.output.docs_dir, "docs");
    }

    // spec: SET-1
    #[test]
    fn the_rules_pass_is_on_unless_turned_off_and_has_its_own_llm_section() {
        assert!(Config::default().pipeline.rules);
        let none: Config = toml::from_str("[pipeline]\ndomains_from = \"surface\"").unwrap();
        assert!(none.pipeline.rules);
        let off: Config = toml::from_str("[pipeline]\nrules = false").unwrap();
        assert!(!off.pipeline.rules);
        assert!(Config::default().pipeline.reconcile);
        assert!(
            !toml::from_str::<Config>("[pipeline]\nreconcile = false")
                .unwrap()
                .pipeline
                .reconcile
        );
        let on: Config =
            toml::from_str("[pipeline]\nrules = true\n[llm.passes.rules]\nmodel = \"big\"")
                .unwrap();
        assert!(on.pipeline.rules);
        assert_eq!(on.llm.for_pass("rules").model, "big");
    }

    // spec: SET-6 (internal: the default is a parse of retrodoc.toml, no command shows it without an LLM run)
    #[test]
    fn the_domains_come_from_the_surface_unless_another_path_is_named() {
        assert_eq!(
            Config::default().pipeline.domains_from,
            DomainsFrom::Surface
        );
        let none: Config = toml::from_str("[llm]\nmodel = \"m\"").unwrap();
        assert_eq!(none.pipeline.domains_from, DomainsFrom::Surface);
        let contexts: Config = toml::from_str("[pipeline]\ndomains_from = \"contexts\"").unwrap();
        assert_eq!(contexts.pipeline.domains_from, DomainsFrom::Contexts);
        // Kept when the config is written back.
        let raw = toml::to_string_pretty(&contexts).unwrap();
        let again: Config = toml::from_str(&raw).unwrap();
        assert_eq!(again.pipeline.domains_from, DomainsFrom::Contexts);
        // `init` writes the key, so a user sees which path the run takes and where to change it.
        let dir = tempfile::tempdir().unwrap();
        Config::write_default(dir.path(), false).unwrap();
        let written = fs::read_to_string(dir.path().join(CONFIG_FILE_NAME)).unwrap();
        assert!(written.contains("domains_from = \"surface\""), "{written}");
    }

    // spec: SET-2
    #[test]
    fn init_writes_the_default_settings_with_the_rules_on() {
        let dir = tempfile::tempdir().unwrap();
        Config::write_default(dir.path(), false).unwrap();
        let read = Config::load(dir.path()).unwrap();
        assert_eq!(read.pipeline, PipelineConfig::default());
        assert!(read.pipeline.rules);
    }

    // spec: SET-4
    #[test]
    fn a_removed_key_stops_the_load_with_a_message_naming_it() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(CONFIG_FILE_NAME),
            "[pipeline]\nrules = true\nbody_lines = 5\n",
        )
        .unwrap();
        let message = Config::load(dir.path()).unwrap_err().to_string();
        assert!(
            message.contains("[pipeline] body_lines was removed"),
            "{message}"
        );
        assert!(message.contains("delete the line"), "{message}");
    }

    /// The message `Config::load` gives for `raw`, which must fail.
    fn load_error(raw: &str) -> String {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(CONFIG_FILE_NAME), raw).unwrap();
        Config::load(dir.path()).unwrap_err().to_string()
    }

    // spec: SET-8 (internal: the refusal is a parse of retrodoc.toml, the same for every command)
    #[test]
    fn a_setting_of_the_removed_files_path_stops_the_load_with_a_message_naming_it() {
        let message = load_error("[pipeline]\ndomains_from = \"files\"\nrules = true\n");
        assert!(
            message.contains("[pipeline] domains_from = \"files\" was removed"),
            "{message}"
        );
        assert!(message.contains("ADR 0035"), "{message}");
        assert!(message.contains("domains_from = \"surface\""), "{message}");
        let message = load_error("[llm]\nmodel = \"m\"\nbatch_chars = 1000\n");
        assert!(
            message.contains("[llm] batch_chars was removed"),
            "{message}"
        );
        assert!(message.contains("delete the line"), "{message}");
        let message = load_error("[llm.passes.repo-map]\nconcurrency = 8\n");
        assert!(
            message.contains("[llm.passes.repo-map] was removed"),
            "{message}"
        );
        assert!(message.contains("delete the section"), "{message}");
        let message = load_error("[llm.passes.domains]\nbatch_chars = 0\n");
        assert!(
            message.contains("[llm.passes.domains] batch_chars was removed"),
            "{message}"
        );
    }

    // spec: SET-4
    #[test]
    fn a_mistyped_key_keeps_the_position_of_the_parse_error() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(CONFIG_FILE_NAME),
            "[pipeline]\ndomains_from = \"surface\"\nrulez = true\n",
        )
        .unwrap();
        let message = Config::load(dir.path()).unwrap_err().to_string();
        assert!(message.contains("line 3"), "{message}");
    }

    // spec: SET-3
    #[test]
    fn the_business_files_are_read_once_unless_turned_off() {
        assert!(Config::default().pipeline.business_read);
        let other: Config = toml::from_str("[pipeline]\nrules = false").unwrap();
        assert!(other.pipeline.business_read);
        let off: Config = toml::from_str("[pipeline]\nbusiness_read = false").unwrap();
        assert!(!off.pipeline.business_read);
    }

    #[test]
    fn an_unknown_domains_from_or_pipeline_key_is_a_config_error() {
        assert!(toml::from_str::<Config>("[pipeline]\ndomains_from = \"magic\"").is_err());
        assert!(toml::from_str::<Config>("[pipeline]\ndomain_from = \"surface\"").is_err());
    }

    // spec: SET-5 (internal: the default is a parse of retrodoc.toml, no command shows it without an LLM run)
    #[test]
    fn the_evidence_of_each_unit_is_on_unless_turned_off() {
        assert!(Config::default().brief.evidence);
        let none: Config = toml::from_str("").unwrap();
        assert!(none.brief.evidence);
        let other: Config = toml::from_str("[brief]\n").unwrap();
        assert!(other.brief.evidence);
        let off: Config = toml::from_str("[brief]\nevidence = false").unwrap();
        assert!(!off.brief.evidence);
    }

    #[test]
    fn a_pass_overrides_only_the_keys_it_names() {
        let raw = r#"
            [llm]
            model = "local"
            base_url = "http://localhost:11434/v1"
            concurrency = 2

            [llm.passes.domains]
            model = "strong"
            base_url = ""
            timeout_secs = 600

            [llm.passes.use-cases]
            concurrency = 8
        "#;
        let llm = toml::from_str::<Config>(raw).unwrap().llm;
        let domains = llm.for_pass("domains");
        assert_eq!(domains.model, "strong");
        assert_eq!(domains.base_url, None);
        assert_eq!(domains.timeout_secs, Some(600));
        assert_eq!(domains.concurrency, Some(2));
        assert!(domains.passes.is_empty());
        let use_cases = llm.for_pass("use-cases");
        assert_eq!(use_cases.model, "local");
        assert_eq!(
            use_cases.base_url.as_deref(),
            Some("http://localhost:11434/v1")
        );
        assert_eq!(use_cases.concurrency, Some(8));
        // A pass without a section changes nothing.
        assert_eq!(llm.for_pass("glossary").model, "local");
    }

    #[test]
    fn the_zero_temperature_passes_are_passes() {
        for pass in ZERO_TEMPERATURE_PASSES {
            assert!(PASS_NAMES.contains(pass), "{pass} is no pass");
        }
    }

    #[test]
    fn the_temperature_is_set_in_llm_and_overridden_by_a_pass() {
        let raw = r"
            [llm]
            temperature = 0.0

            [llm.passes.domains]
            temperature = 0.7
        ";
        let llm = toml::from_str::<Config>(raw).unwrap().llm;
        assert_eq!(llm.for_pass("glossary").temperature, Some(0.0));
        assert_eq!(llm.for_pass("domains").temperature, Some(0.7));
        // Unset: 0 for the extraction and writing passes, the server's default for the others.
        let unset = LlmConfig::default();
        assert_eq!(unset.for_pass("entry-points").temperature, Some(0.0));
        assert_eq!(unset.for_pass("domains").temperature, Some(0.0));
        assert_eq!(unset.for_pass("contexts").temperature, None);
        let pass: Config = toml::from_str("[llm.passes.glossary]\ntemperature = 0.5").unwrap();
        assert_eq!(pass.llm.for_pass("glossary").temperature, Some(0.5));
    }

    #[test]
    fn a_pass_changing_the_provider_takes_its_key_variable() {
        let raw = r#"
            [llm.passes.brief]
            provider = "deepseek"
            [llm.passes.roles]
            provider = "deepseek"
            api_key_env = "MY_KEY"
        "#;
        let llm = toml::from_str::<Config>(raw).unwrap().llm;
        assert_eq!(llm.for_pass("brief").api_key_env, "OPENROUTER_API_KEY");
        assert_eq!(llm.for_pass("brief").provider, "deepseek");
        assert_eq!(llm.for_pass("roles").api_key_env, "MY_KEY");
    }

    #[test]
    fn an_unknown_pass_or_key_is_a_config_error() {
        let dir = tempfile::tempdir().unwrap();
        let write = |raw: &str| fs::write(dir.path().join(CONFIG_FILE_NAME), raw).unwrap();
        write("[llm.passes.domain]\nmodel = \"x\"");
        let error = Config::load(dir.path()).unwrap_err().to_string();
        assert!(
            error.contains("domain") && error.contains("use-cases"),
            "{error}"
        );
        write("[llm.passes.domains]\nmodle = \"x\"");
        assert!(matches!(
            Config::load(dir.path()),
            Err(ConfigError::Parse(_))
        ));
        write("[llm.passes.domains]\nmodel = \"x\"");
        assert!(Config::load(dir.path()).is_ok());
    }

    #[test]
    fn a_config_without_passes_serializes_none() {
        let raw = toml::to_string_pretty(&Config::default()).unwrap();
        assert!(!raw.contains("passes"), "{raw}");
    }
}
