//! Where the schema, the migrations and the translations of the repository
//! are, and in which format (ADR 0019, `retrodoc_ingest::SourceMap`).
//!
//! `SourceMap::sniff` guesses it from file content with no LLM. One LLM call
//! over the file tree, the manifests and that guess does better on a stack
//! the sniffing doesn't know: it answers glob rules, which are checked
//! against the real files (a rule that selects nothing, or files its format
//! can't read, is sent back once for a fix, then dropped). The result keeps
//! the sniffed sources the LLM left out, so it is never worse than the guess.
//!
//! The rules are saved as `.retrodoc/cache/signal-sources.yaml`, reviewable
//! and hand-editable, with two hashes: the shape of the tree they were
//! inferred from, and the rules as inferred. A file whose rules differ from
//! the second hash was edited and is kept as is; an unedited one is inferred
//! again only when the shape of the tree changed (a new kind of folder or
//! file), not for each new file. An LLM that fails or answers nothing usable
//! leaves the sniffed map, with a warning: the signals are an input, never a
//! reason to stop `generate`.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use retrodoc_ingest::signals::{check_rule, SourceFormat, SourceKind, SourceMap, SourceRule};
use retrodoc_ingest::IngestResult;
use retrodoc_llm::LlmProvider;
use serde::{Deserialize, Serialize};

use crate::artifact::{load_yaml, save_yaml, Artifact};
use crate::cache::hash_content;
use crate::error::PipelineError;
use crate::response::complete_json;
use crate::roles::{render_tree, MANIFESTS, MAX_MANIFEST_CHARS};

/// Sniffed rules shown to the LLM as a starting point.
const MAX_HINT_RULES: usize = 60;
/// Rules kept from the LLM's answer.
const MAX_RULES: usize = 40;

const SYSTEM_PROMPT: &str = "You are locating, in a software repository of any stack, the files \
that tell what the product does through its data and its words: the database schema, the \
migrations and the translated texts (the labels and messages shown to users). You are given the \
file tree, the manifests and a first guess made by content sniffing. Answer glob rules relative \
to the repository root. `kind` is one of \"schema\", \"migrations\", \"i18n\". `format` is one of: \
\"sql_ddl\" (SQL CREATE TABLE statements), \"rails_schema\" (an ActiveRecord schema file), \
\"file_names\" (migrations: only the file names are read), \"yaml\", \"json\", \"properties\" \
(Java key=value files) and \"po\" (gettext) for translations. Only use these formats; leave out \
a source in another format. Do not list a file twice for one kind. Prefer one glob for a whole \
folder (\"src/main/resources/i18n/*.properties\") to one rule per file. Fix the guess where it \
is wrong and add what it missed. Reply with ONLY a single JSON object, no prose and no Markdown \
code fence, matching this shape: {\"rules\":[{\"kind\":\"i18n\",\"glob\":\"locales/**/*.yml\",\
\"format\":\"yaml\"}]}.";

/// The saved artifact: the rules and what tells whether they were edited or
/// are out of date.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceMapFile {
    /// Shape of the file tree the rules were inferred from.
    pub tree_hash: String,
    /// Hash of the rules as inferred: a different hash on load means the
    /// file was edited by hand.
    pub rules_hash: String,
    pub rules: Vec<SourceRule>,
}

impl SourceMapFile {
    fn new(tree_hash: String, rules: Vec<SourceRule>) -> Self {
        Self {
            tree_hash,
            rules_hash: hash_rules(&rules),
            rules,
        }
    }

    /// Loads `.retrodoc/cache/signal-sources.yaml`. Missing or unreadable:
    /// `None`.
    #[must_use]
    pub fn load(repo_root: &Path) -> Option<Self> {
        load_yaml(&Artifact::SignalSources.path(repo_root))
    }

    /// # Errors
    ///
    /// Returns an error if the cache folder or file can't be written, or
    /// serialization fails.
    pub fn save(&self, repo_root: &Path) -> Result<(), PipelineError> {
        save_yaml(&Artifact::SignalSources.path(repo_root), self)
    }

    fn edited(&self) -> bool {
        hash_rules(&self.rules) != self.rules_hash
    }
}

/// Returns the sources of the repo: the saved `signal-sources.yaml` when it
/// was edited, or is current; otherwise the rules the LLM infers (see the
/// module doc), saved for next time. `force` infers again whatever the file
/// holds, edits included (as `roles --force`). When the LLM fails or never
/// gives a parseable answer the sniffed map is returned, not saved, so a
/// later run asks again. A saved file that can't be read is left alone.
///
/// # Errors
///
/// Returns an error only if the inferred rules can't be saved.
pub async fn infer_sources(
    repo_root: &Path,
    ingest: &IngestResult,
    llm: &dyn LlmProvider,
    force: bool,
) -> Result<SourceMap, PipelineError> {
    let tree_hash = tree_shape_hash(ingest);
    let sniffed = SourceMap::sniff(repo_root, &ingest.files);
    if !force {
        if Artifact::SignalSources.path(repo_root).exists()
            && SourceMapFile::load(repo_root).is_none()
        {
            // Never overwrite what a person may be in the middle of editing.
            tracing::warn!(
                "{} can't be read, using a guess from the file content; fix it or run `retrodoc brief --force`",
                Artifact::SignalSources.relative_path()
            );
            return Ok(sniffed);
        }
        if let Some(saved) = SourceMapFile::load(repo_root) {
            if saved.edited() {
                tracing::info!("using the hand-edited signal-sources.yaml");
                return Ok(SourceMap { rules: saved.rules });
            }
            if saved.tree_hash == tree_hash {
                return Ok(SourceMap { rules: saved.rules });
            }
        }
    }

    let rules = match ask(repo_root, ingest, llm, &sniffed).await {
        Ok(Some(rules)) => rules,
        Ok(None) => return Ok(sniffed),
        Err(error) => {
            tracing::warn!("could not infer where the schema and the translations are ({error}), using a guess from the file content");
            return Ok(sniffed);
        }
    };
    let rules = with_missing_sniffed(rules, &sniffed, &ingest.files);
    SourceMapFile::new(tree_hash, rules.clone()).save(repo_root)?;
    Ok(SourceMap { rules })
}

/// The LLM's rules, checked and fixed once (none at all is an answer: there
/// may be no schema nor translations). `None` when it never gave a parseable
/// answer.
async fn ask(
    repo_root: &Path,
    ingest: &IngestResult,
    llm: &dyn LlmProvider,
    sniffed: &SourceMap,
) -> Result<Option<Vec<SourceRule>>, PipelineError> {
    let mut prompt = String::from("File tree (one line per directory):\n");
    prompt.push_str(&render_tree(&ingest.files));
    for name in MANIFESTS {
        if let Ok(content) = std::fs::read_to_string(repo_root.join(name)) {
            let excerpt: String = content.chars().take(MAX_MANIFEST_CHARS).collect();
            let _ = write!(prompt, "\n--- {name} ---\n{excerpt}\n");
        }
    }
    prompt.push_str("\nFirst guess from the file content (one file per line):\n");
    for rule in sniffed.rules.iter().take(MAX_HINT_RULES) {
        let _ = writeln!(prompt, "{}", describe(rule));
    }
    if sniffed.rules.len() > MAX_HINT_RULES {
        let _ = writeln!(
            prompt,
            "… and {} more",
            sniffed.rules.len() - MAX_HINT_RULES
        );
    }

    let what = "source rules";
    let Some(response) = complete_json::<Response>(llm, SYSTEM_PROMPT, &prompt, what).await? else {
        return Ok(None);
    };
    let mut rules = response.rules();
    let (mut good, bad) = split_by_check(repo_root, &ingest.files, rules);
    if !bad.is_empty() {
        let mut retry = prompt.clone();
        retry.push_str("\nYour previous rules, with problems:\n");
        for (rule, reason) in &bad {
            let _ = writeln!(retry, "{} — {reason}", describe(rule));
        }
        retry.push_str("\nGive the complete corrected list of rules.\n");
        rules = match complete_json::<Response>(llm, SYSTEM_PROMPT, &retry, what).await? {
            Some(response) => response.rules(),
            None => Vec::new(),
        };
        let (fixed, still_bad) = split_by_check(repo_root, &ingest.files, rules);
        for (rule, reason) in still_bad {
            tracing::warn!("dropping source rule {}: {reason}", describe(&rule));
        }
        // The rules that were good the first time stay, whatever the retry says.
        for rule in fixed {
            if !good.contains(&rule) {
                good.push(rule);
            }
        }
    }
    good.truncate(MAX_RULES);
    Ok(Some(good))
}

#[derive(Debug, Deserialize)]
struct Response {
    #[serde(default)]
    rules: Vec<RawRule>,
}

#[derive(Debug, Deserialize)]
struct RawRule {
    #[serde(default)]
    kind: String,
    #[serde(default)]
    glob: String,
    #[serde(default)]
    format: String,
}

impl Response {
    /// The rules whose kind and format are known; the others are dropped with
    /// a warning.
    fn rules(self) -> Vec<SourceRule> {
        self.rules
            .into_iter()
            .filter_map(|raw| {
                let rule =
                    parse_kind(&raw.kind)
                        .zip(parse_format(&raw.format))
                        .map(|(kind, format)| SourceRule {
                            kind,
                            glob: raw.glob.trim().to_string(),
                            format,
                        });
                if rule.is_none() || raw.glob.trim().is_empty() {
                    tracing::warn!(
                        "ignoring source rule with kind {:?}, format {:?} and glob {:?}",
                        raw.kind,
                        raw.format,
                        raw.glob
                    );
                    return None;
                }
                rule
            })
            .collect()
    }
}

fn parse_kind(text: &str) -> Option<SourceKind> {
    serde_yaml::from_str(&format!("\"{}\"", text.trim().to_lowercase())).ok()
}

fn parse_format(text: &str) -> Option<SourceFormat> {
    serde_yaml::from_str(&format!("\"{}\"", text.trim().to_lowercase())).ok()
}

/// Rules that select readable files, and those that don't with why.
fn split_by_check(
    repo_root: &Path,
    files: &[retrodoc_ingest::FileEntry],
    rules: Vec<SourceRule>,
) -> (Vec<SourceRule>, Vec<(SourceRule, String)>) {
    let mut good = Vec::new();
    let mut bad = Vec::new();
    for rule in rules {
        let check = check_rule(repo_root, files, &rule);
        if check.matched == 0 {
            bad.push((rule, "the glob selects no file".to_string()));
        } else if check.usable == 0 {
            let reason = format!(
                "it selects {} file(s) but none can be read in that format",
                check.matched
            );
            bad.push((rule, reason));
        } else {
            good.push(rule);
        }
    }
    (good, bad)
}

/// The inferred rules plus the sniffed ones for files they don't select.
fn with_missing_sniffed(
    mut rules: Vec<SourceRule>,
    sniffed: &SourceMap,
    files: &[retrodoc_ingest::FileEntry],
) -> Vec<SourceRule> {
    let inferred = SourceMap {
        rules: rules.clone(),
    };
    for kind in [SourceKind::Schema, SourceKind::Migrations, SourceKind::I18n] {
        let covered: BTreeSet<_> = inferred
            .files(kind, files)
            .into_iter()
            .map(|(file, _)| file.path.clone())
            .collect();
        let sniffed_kind = SourceMap {
            rules: sniffed
                .rules
                .iter()
                .filter(|r| r.kind == kind)
                .cloned()
                .collect(),
        };
        for (file, format) in sniffed_kind.files(kind, files) {
            if !covered.contains(&file.path) {
                rules.push(SourceRule {
                    kind,
                    glob: globset_escape(&file.path),
                    format,
                });
            }
        }
    }
    rules
}

fn globset_escape(path: &Path) -> String {
    retrodoc_ingest::signals::escape_glob(&path.to_string_lossy().replace('\\', "/"))
}

fn describe(rule: &SourceRule) -> String {
    let kind = serde_yaml::to_string(&rule.kind).unwrap_or_default();
    let format = serde_yaml::to_string(&rule.format).unwrap_or_default();
    format!("{} {} {}", kind.trim(), format.trim(), rule.glob)
}

fn hash_rules(rules: &[SourceRule]) -> String {
    hash_content(&serde_yaml::to_string(rules).unwrap_or_default())
}

/// The shape of the tree: which kinds of file sit in which folder, not how
/// many, so adding a file doesn't make the rules out of date.
fn tree_shape_hash(ingest: &IngestResult) -> String {
    let shapes: BTreeSet<String> = ingest
        .files
        .iter()
        .map(|file| {
            let dir = file.path.parent().unwrap_or(Path::new(""));
            let ext = file
                .path
                .extension()
                .map(|e| e.to_string_lossy().into_owned())
                .unwrap_or_default();
            format!("{}|{ext}", dir.display())
        })
        .collect();
    hash_content(&shapes.into_iter().collect::<Vec<_>>().join("\n"))
}

#[cfg(test)]
mod tests;
