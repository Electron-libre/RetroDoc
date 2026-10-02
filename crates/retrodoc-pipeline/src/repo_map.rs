//! Repo map (PLAN.md §2 step 2, roadmap phase 2): bottom-up summary per
//! file, then per module (folder), enriched with git history. Intermediate
//! pipeline artifact — the basis for the next phase (domain clustering,
//! PLAN.md §2 step 3).
//!
//! Bottom-up: each source file is summarized individually (probable role +
//! git history), then each folder is summarized from the summaries of its
//! direct files and already-summarized sub-folders, deepest first. The root
//! folder (empty `path`) thus carries a summary aggregating the whole repo.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use retrodoc_ingest::{FileEntry, FileHistory, FileKind, IngestResult};
use retrodoc_llm::{ChatMessage, CompletionRequest, LlmProvider, Role};
use serde::{Deserialize, Serialize};

use crate::cache::{hash_content, RepoMapCache};
use crate::error::PipelineError;
use crate::progress::Progress;

/// Files larger than this are truncated before being sent to the LLM, to
/// stay within a reasonable token budget (PLAN.md §6 "cost/volume").
const MAX_FILE_CHARS: usize = 6000;

const FILE_SUMMARY_SYSTEM_PROMPT: &str = "You summarize in one or two concise sentences the \
probable role of a source code file, based on its path, its git history, and its content. Reply \
with only the summary, in English, with no preamble and no Markdown formatting.";

const MODULE_SUMMARY_SYSTEM_PROMPT: &str = "You summarize in one or two concise sentences the \
probable role of a module (folder) of a software project, based on the summaries of its direct \
files and its sub-modules. Reply with only the summary, in English, with no preamble and no \
Markdown formatting.";

/// Summary of a file's probable role, enriched with its git history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSummary {
    pub path: PathBuf,
    pub role_summary: String,
    pub commit_count: u32,
    pub author_count: u32,
}

/// Bottom-up summary of a module (folder), synthesized from the summaries
/// of its direct files and its sub-modules. Empty `path` == repo root.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleSummary {
    pub path: PathBuf,
    pub role_summary: String,
    /// Number of source files in this module, sub-modules included.
    pub file_count: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RepoMap {
    pub files: Vec<FileSummary>,
    pub modules: Vec<ModuleSummary>,
}

/// Builds the repo map from the ingestion result: summarizes each source
/// file (with a content-hash cache), then each module bottom-up. Files that
/// aren't readable as UTF-8 or aren't readable at all are skipped (logged
/// warning) rather than failing the whole run.
///
/// # Errors
///
/// Returns an error if an LLM call fails or the cache can't be saved to
/// disk.
pub async fn build_repo_map(
    repo_root: &Path,
    ingest: &IngestResult,
    llm: &dyn LlmProvider,
) -> Result<RepoMap, PipelineError> {
    let mut cache = RepoMapCache::load(repo_root);
    let mut files = Vec::new();
    let mut progress = Progress::new(
        "repo map",
        ingest
            .files
            .iter()
            .filter(|f| f.kind == FileKind::Source)
            .count(),
    );

    for entry in &ingest.files {
        if entry.kind != FileKind::Source {
            continue;
        }
        let content = match read_file_lossy(repo_root, &entry.path) {
            Ok(content) => content,
            Err(err) => {
                tracing::warn!(
                    path = %entry.path.display(),
                    error = %err,
                    "file skipped in the repo map (could not read it)"
                );
                progress.skip();
                continue;
            }
        };

        let hash = hash_content(&content);
        let history = ingest.history_for(&entry.path);

        let role_summary = if let Some(cached) = cache.get(&entry.path, &hash) {
            progress.skip();
            cached.to_string()
        } else {
            progress.begin(&entry.path.display().to_string());
            match summarize_file(llm, entry, &content, history).await {
                Ok(summary) => summary,
                Err(err) => {
                    // A transient failure partway through a long file list
                    // shouldn't discard the summaries already computed in
                    // this run: best-effort save before propagating (a
                    // rerun then only has to redo the files not yet
                    // cached, not the whole list).
                    let _ = cache.save(repo_root);
                    return Err(err);
                }
            }
        };
        cache.put(&entry.path, &hash, &role_summary);

        files.push(FileSummary {
            path: entry.path.clone(),
            role_summary,
            commit_count: history.map_or(0, |h| h.commit_count),
            author_count: history.map_or(0, author_count),
        });
    }

    // Save once the whole file loop succeeds too (belt and suspenders): a
    // failure further down the pipeline (module summaries) shouldn't lose
    // the file-level work already done.
    cache.save(repo_root)?;

    let modules = build_module_summaries(llm, &files).await?;

    Ok(RepoMap { files, modules })
}

pub(crate) fn read_file_lossy(repo_root: &Path, relative: &Path) -> Result<String, PipelineError> {
    let abs = repo_root.join(relative);
    let bytes = std::fs::read(&abs).map_err(|source| PipelineError::Read {
        path: relative.to_path_buf(),
        source,
    })?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

pub(crate) fn truncate_chars(content: &str, max_chars: usize) -> String {
    if content.chars().count() <= max_chars {
        return content.to_string();
    }
    let truncated: String = content.chars().take(max_chars).collect();
    format!("{truncated}\n… (truncated)")
}

/// Number of distinct authors, saturated at `u32::MAX` (never reached in
/// practice — a repo doesn't have billions of authors).
fn author_count(history: &FileHistory) -> u32 {
    u32::try_from(history.authors.len()).unwrap_or(u32::MAX)
}

fn history_line(history: Option<&FileHistory>) -> String {
    match history {
        Some(h) if h.commit_count > 0 => format!(
            "{} commit(s), {} author(s), last modified {}",
            h.commit_count,
            h.authors.len(),
            h.last_commit_at
                .map_or_else(|| "unknown".to_string(), |d| d.date_naive().to_string())
        ),
        _ => "no git history (file not versioned or never committed)".to_string(),
    }
}

async fn summarize_file(
    llm: &dyn LlmProvider,
    entry: &FileEntry,
    content: &str,
    history: Option<&FileHistory>,
) -> Result<String, PipelineError> {
    let prompt = format!(
        "File: {}\nHistory: {}\n\nContent:\n```\n{}\n```",
        entry.path.display(),
        history_line(history),
        truncate_chars(content, MAX_FILE_CHARS)
    );

    let response = llm
        .complete(CompletionRequest {
            messages: vec![
                ChatMessage {
                    role: Role::System,
                    content: FILE_SUMMARY_SYSTEM_PROMPT.to_string(),
                },
                ChatMessage {
                    role: Role::User,
                    content: prompt,
                },
            ],
            model: None,
        })
        .await?;

    Ok(response.content.trim().to_string())
}

async fn summarize_module(
    llm: &dyn LlmProvider,
    dir: &Path,
    own_files: &[&FileSummary],
    child_modules: &[&ModuleSummary],
) -> Result<String, PipelineError> {
    let dir_label = if dir.as_os_str().is_empty() {
        "repo root".to_string()
    } else {
        dir.display().to_string()
    };

    let mut listing = String::new();
    for file in own_files {
        // `write!` on a `String` can't fail.
        let _ = writeln!(
            listing,
            "- file {}: {}",
            file.path.display(),
            file.role_summary
        );
    }
    for module in child_modules {
        let _ = writeln!(
            listing,
            "- sub-module {}: {}",
            module.path.display(),
            module.role_summary
        );
    }

    let prompt = format!("Folder: {dir_label}\n\nSummarized content:\n{listing}");

    let response = llm
        .complete(CompletionRequest {
            messages: vec![
                ChatMessage {
                    role: Role::System,
                    content: MODULE_SUMMARY_SYSTEM_PROMPT.to_string(),
                },
                ChatMessage {
                    role: Role::User,
                    content: prompt,
                },
            ],
            model: None,
        })
        .await?;

    Ok(response.content.trim().to_string())
}

/// Synthesizes a per-folder summary, deepest to shallowest, feeding each
/// parent with the already-computed summaries of its direct children
/// (files + sub-modules).
async fn build_module_summaries(
    llm: &dyn LlmProvider,
    files: &[FileSummary],
) -> Result<Vec<ModuleSummary>, PipelineError> {
    let mut dirs: BTreeSet<PathBuf> = BTreeSet::new();
    for file in files {
        if let Some(parent) = file.path.parent() {
            for ancestor in parent.ancestors() {
                dirs.insert(ancestor.to_path_buf());
            }
        }
    }
    if dirs.is_empty() {
        return Ok(Vec::new());
    }

    let mut files_by_dir: BTreeMap<PathBuf, Vec<&FileSummary>> = BTreeMap::new();
    for file in files {
        let parent = file.path.parent().unwrap_or_else(|| Path::new(""));
        files_by_dir
            .entry(parent.to_path_buf())
            .or_default()
            .push(file);
    }

    let mut children_by_dir: BTreeMap<PathBuf, Vec<PathBuf>> = BTreeMap::new();
    for dir in &dirs {
        if let Some(parent) = dir.parent() {
            children_by_dir
                .entry(parent.to_path_buf())
                .or_default()
                .push(dir.clone());
        }
    }

    // Deepest to shallowest, so that each folder already has its
    // sub-folders' summaries by the time it's processed (bottom-up).
    let mut ordered: Vec<PathBuf> = dirs.into_iter().collect();
    ordered.sort_by_key(|d| std::cmp::Reverse(d.components().count()));

    let mut computed: BTreeMap<PathBuf, ModuleSummary> = BTreeMap::new();
    let mut progress = Progress::new("directory summaries", ordered.len());
    for dir in ordered {
        let own_files = files_by_dir.get(&dir).cloned().unwrap_or_default();
        let child_modules: Vec<&ModuleSummary> = children_by_dir
            .get(&dir)
            .into_iter()
            .flatten()
            .filter_map(|child| computed.get(child))
            .collect();

        if own_files.is_empty() && child_modules.is_empty() {
            progress.skip();
            continue;
        }

        // Saturated at `u32::MAX`: never reached in practice (no repo with
        // billions of files).
        let own_file_count = u32::try_from(own_files.len()).unwrap_or(u32::MAX);
        let file_count = own_file_count + child_modules.iter().map(|m| m.file_count).sum::<u32>();
        progress.begin(&format!("{}/", dir.display()));
        let role_summary = summarize_module(llm, &dir, &own_files, &child_modules).await?;

        computed.insert(
            dir.clone(),
            ModuleSummary {
                path: dir,
                role_summary,
                file_count,
            },
        );
    }

    Ok(computed.into_values().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use async_trait::async_trait;
    use retrodoc_ingest::{FileEntry, FileKind, IngestResult};
    use retrodoc_llm::{CompletionResponse, LlmError};

    /// Fake provider that counts its calls and returns a deterministic
    /// summary, to test bottom-up aggregation and the cache without
    /// depending on the network.
    struct CountingProvider {
        calls: AtomicUsize,
    }

    #[async_trait]
    impl LlmProvider for CountingProvider {
        async fn complete(
            &self,
            request: CompletionRequest,
        ) -> Result<CompletionResponse, LlmError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let last_user_message = request
                .messages
                .iter()
                .rev()
                .find(|m| m.role == Role::User)
                .map(|m| m.content.clone())
                .unwrap_or_default();
            let first_line = last_user_message.lines().next().unwrap_or("").to_string();
            Ok(CompletionResponse {
                content: format!("summary of: {first_line}"),
                model: "test-model".to_string(),
            })
        }
    }

    fn ingest_with_nested_files(root: &Path) -> IngestResult {
        std::fs::create_dir_all(root.join("a/b")).unwrap();
        std::fs::write(root.join("a/b/x.rs"), "fn x() {}").unwrap();
        std::fs::write(root.join("a/y.rs"), "fn y() {}").unwrap();

        IngestResult {
            files: vec![
                FileEntry {
                    path: PathBuf::from("a/b/x.rs"),
                    kind: FileKind::Source,
                    size_bytes: 9,
                },
                FileEntry {
                    path: PathBuf::from("a/y.rs"),
                    kind: FileKind::Source,
                    size_bytes: 9,
                },
            ],
            history_by_path: HashMap::new(),
            existing_docs: Vec::new(),
        }
    }

    #[tokio::test]
    async fn builds_bottom_up_modules_for_nested_directories() {
        let dir = tempfile::tempdir().unwrap();
        let ingest = ingest_with_nested_files(dir.path());
        let provider = CountingProvider {
            calls: AtomicUsize::new(0),
        };

        let map = build_repo_map(dir.path(), &ingest, &provider)
            .await
            .unwrap();

        assert_eq!(map.files.len(), 2);
        let module_paths: Vec<_> = map.modules.iter().map(|m| m.path.clone()).collect();
        assert!(module_paths.contains(&PathBuf::from("a")));
        assert!(module_paths.contains(&PathBuf::from("a/b")));

        // "a" aggregates its own file (y.rs) and sub-module "a/b" (x.rs).
        let module_a = map
            .modules
            .iter()
            .find(|m| m.path == Path::new("a"))
            .unwrap();
        assert_eq!(module_a.file_count, 2);
    }

    #[tokio::test]
    async fn unchanged_files_are_not_re_summarized_on_second_run() {
        let dir = tempfile::tempdir().unwrap();
        let ingest = ingest_with_nested_files(dir.path());
        let provider = CountingProvider {
            calls: AtomicUsize::new(0),
        };

        build_repo_map(dir.path(), &ingest, &provider)
            .await
            .unwrap();
        // 2 files + 3 modules ("a/b", "a", root "") = 5 calls on the first run.
        let calls_after_first_run = provider.calls.load(Ordering::SeqCst);
        assert_eq!(calls_after_first_run, 5);

        build_repo_map(dir.path(), &ingest, &provider)
            .await
            .unwrap();
        let calls_after_second_run = provider.calls.load(Ordering::SeqCst);

        // File summaries are served from the cache (unchanged content):
        // only the 3 modules, which aren't cached, trigger a new LLM call.
        assert_eq!(calls_after_second_run - calls_after_first_run, 3);
    }

    /// Fake provider that succeeds its first `succeed_calls` completions,
    /// then fails every one after that — simulates a provider that turns
    /// flaky partway through a long file list (e.g. a rate limit hit deep
    /// into the run).
    struct FailAfterNProvider {
        succeed_calls: usize,
        calls: AtomicUsize,
    }

    #[async_trait]
    impl LlmProvider for FailAfterNProvider {
        async fn complete(
            &self,
            _request: CompletionRequest,
        ) -> Result<CompletionResponse, LlmError> {
            let call_index = self.calls.fetch_add(1, Ordering::SeqCst);
            if call_index < self.succeed_calls {
                Ok(CompletionResponse {
                    content: format!("summary #{call_index}"),
                    model: "test-model".to_string(),
                })
            } else {
                Err(LlmError::Transport("simulated failure".to_string()))
            }
        }
    }

    #[tokio::test]
    async fn a_failure_partway_through_the_file_loop_keeps_earlier_summaries_cached() {
        let dir = tempfile::tempdir().unwrap();
        let ingest = ingest_with_nested_files(dir.path());
        // Only the first file summarization call succeeds; the second one
        // (and the run as a whole) fails.
        let provider = FailAfterNProvider {
            succeed_calls: 1,
            calls: AtomicUsize::new(0),
        };

        let result = build_repo_map(dir.path(), &ingest, &provider).await;
        assert!(result.is_err());

        // The summary computed before the failure was still persisted to
        // disk, not discarded along with the run.
        let cache = RepoMapCache::load(dir.path());
        let cached_paths: Vec<_> = [Path::new("a/b/x.rs"), Path::new("a/y.rs")]
            .into_iter()
            .filter(|p| {
                cache
                    .get(
                        p,
                        &hash_content(&std::fs::read_to_string(dir.path().join(p)).unwrap()),
                    )
                    .is_some()
            })
            .collect();
        assert_eq!(cached_paths.len(), 1);
    }
}
