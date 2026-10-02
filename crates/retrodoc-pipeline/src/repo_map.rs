//! Repo map (PLAN.md §2 step 2, roadmap phase 2): bottom-up summary per
//! file, then per module (folder), enriched with git history. Intermediate
//! pipeline artifact — the basis for the next phase (domain clustering,
//! PLAN.md §2 step 3).
//!
//! Bottom-up: each source file is summarized individually (probable role +
//! git history), then each folder is summarized from the summaries of its
//! direct files and already-summarized sub-folders, deepest first. The root
//! folder (empty `path`) thus carries a summary aggregating the whole repo.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use futures_util::stream::{self, StreamExt};
use retrodoc_ingest::{FileEntry, FileHistory, FileKind, IngestResult};
use retrodoc_llm::{ChatMessage, CompletionRequest, LlmProvider, Role};
use serde::{Deserialize, Serialize};

use crate::cache::{hash_content, RepoMapCache};
use crate::error::PipelineError;
use crate::progress::Progress;
use crate::response::complete_json;

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

/// A batch holds at most this many files.
const MAX_BATCH_FILES: usize = 8;

/// How the repo map pass talks to the LLM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepoMapOptions {
    /// Calls in flight at once (at least 1).
    pub concurrency: usize,
    /// Small files are summarized together, up to this many characters per
    /// request; a file over a quarter of it goes alone. 0: one call per file.
    pub batch_chars: usize,
}

impl Default for RepoMapOptions {
    /// Sequential, one call per file.
    fn default() -> Self {
        Self {
            concurrency: 1,
            batch_chars: 0,
        }
    }
}

/// Groups consecutive files (`(index, chars)`, in tree order) into batches
/// of at most [`MAX_BATCH_FILES`] files and `batch_chars` characters in
/// total; a file over `batch_chars / 4` is never batched.
fn plan_batches(files: &[(usize, usize)], batch_chars: usize) -> Vec<Vec<usize>> {
    let big = batch_chars / 4;
    let mut batches: Vec<Vec<usize>> = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    let mut current_chars = 0;
    for &(index, chars) in files {
        if batch_chars == 0 || chars > big {
            if !current.is_empty() {
                batches.push(std::mem::take(&mut current));
                current_chars = 0;
            }
            batches.push(vec![index]);
            continue;
        }
        if !current.is_empty()
            && (current.len() >= MAX_BATCH_FILES || current_chars + chars > batch_chars)
        {
            batches.push(std::mem::take(&mut current));
            current_chars = 0;
        }
        current.push(index);
        current_chars += chars;
    }
    if !current.is_empty() {
        batches.push(current);
    }
    batches
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
    options: RepoMapOptions,
) -> Result<RepoMap, PipelineError> {
    let mut cache = RepoMapCache::load(repo_root);
    let concurrency = options.concurrency.max(1);
    let progress = RefCell::new(Progress::new(
        "repo map",
        ingest
            .files
            .iter()
            .filter(|f| f.kind == FileKind::Source)
            .count(),
    ));

    // Read everything first: cached summaries are served right away, the
    // others are summarized `concurrency` at a time.
    let mut loaded: Vec<Loaded> = Vec::new();
    for entry in ingest.files.iter().filter(|f| f.kind == FileKind::Source) {
        let content = match read_file_lossy(repo_root, &entry.path) {
            Ok(content) => content,
            Err(err) => {
                tracing::warn!(
                    path = %entry.path.display(),
                    error = %err,
                    "file skipped in the repo map (could not read it)"
                );
                progress.borrow_mut().skip();
                continue;
            }
        };
        let hash = hash_content(&content);
        let summary = cache.get(&entry.path, &hash).map(str::to_string);
        if summary.is_some() {
            progress.borrow_mut().skip();
        }
        loaded.push(Loaded {
            entry,
            content,
            hash,
            summary,
        });
    }

    let sizes: Vec<(usize, usize)> = (0..loaded.len())
        .filter(|&i| loaded[i].summary.is_none())
        .map(|i| (i, loaded[i].content.chars().count().min(MAX_FILE_CHARS)))
        .collect();
    let batches = plan_batches(&sizes, options.batch_chars);
    let mut results: BTreeMap<usize, String> = BTreeMap::new();
    {
        let jobs = batches.iter().map(|batch| {
            let progress = &progress;
            let loaded = &loaded;
            async move {
                let first = loaded[batch[0]].entry.path.display().to_string();
                progress.borrow().start(&if batch.len() > 1 {
                    format!("{first} and {} more", batch.len() - 1)
                } else {
                    first
                });
                summarize_files(llm, ingest, loaded, batch).await
            }
        });
        let mut stream = stream::iter(jobs).buffer_unordered(concurrency);
        while let Some(result) = stream.next().await {
            match result {
                Ok(summaries) => {
                    progress.borrow_mut().finish_many(summaries.len());
                    for (i, summary) in summaries {
                        cache.put(&loaded[i].entry.path, &loaded[i].hash, &summary);
                        results.insert(i, summary);
                    }
                }
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
        }
    }

    let files: Vec<FileSummary> = loaded
        .iter()
        .enumerate()
        .map(|(i, file)| {
            let history = ingest.history_for(&file.entry.path);
            FileSummary {
                path: file.entry.path.clone(),
                role_summary: results
                    .remove(&i)
                    .or_else(|| file.summary.clone())
                    .unwrap_or_default(),
                commit_count: history.map_or(0, |h| h.commit_count),
                author_count: history.map_or(0, author_count),
            }
        })
        .collect();

    // Save once the whole file loop succeeds too (belt and suspenders): a
    // failure further down the pipeline (module summaries) shouldn't lose
    // the file-level work already done.
    cache.save(repo_root)?;

    let modules = build_module_summaries(llm, &files, &mut cache, concurrency).await;
    // Saved even when a module call failed: the folders done so far are kept.
    cache.save(repo_root)?;
    let modules = modules?;

    Ok(RepoMap { files, modules })
}

/// A source file read for the repo map, with its cached summary if any.
struct Loaded<'a> {
    entry: &'a FileEntry,
    content: String,
    hash: String,
    summary: Option<String>,
}

/// What the repo map pass is about to send to the LLM, worked out from the
/// caches without any call (see [`estimate_repo_map`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepoMapEstimate {
    /// Readable source files.
    pub files: usize,
    /// Files without an up-to-date cached summary.
    pub files_to_summarize: usize,
    /// LLM calls for those files (fewer when small files are batched).
    pub file_calls: usize,
    /// Characters of those files that will be sent (after truncation).
    pub chars_to_send: usize,
    /// Folders holding source files, sub-folders included.
    pub directories: usize,
    /// Folders to summarize: a changed file invalidates its ancestors, a
    /// folder never summarized is new. A lower bound (a changed summary can
    /// invalidate more of them), exact on a first run.
    pub directories_to_summarize: usize,
}

impl RepoMapEstimate {
    /// LLM calls of the pass.
    #[must_use]
    pub fn calls(&self) -> usize {
        self.file_calls + self.directories_to_summarize
    }
}

/// Estimates the repo map pass from the caches, so a long run can be sized
/// before it starts. Later passes depend on its output (one call per domain
/// unit, per feature, per use case and per scored use case).
#[must_use]
pub fn estimate_repo_map(
    repo_root: &Path,
    ingest: &IngestResult,
    batch_chars: usize,
) -> RepoMapEstimate {
    let cache = RepoMapCache::load(repo_root);
    let mut estimate = RepoMapEstimate {
        files: 0,
        files_to_summarize: 0,
        file_calls: 0,
        chars_to_send: 0,
        directories: 0,
        directories_to_summarize: 0,
    };
    let mut dirs: BTreeSet<PathBuf> = BTreeSet::new();
    let mut dirty: BTreeSet<PathBuf> = BTreeSet::new();
    let mut sizes: Vec<(usize, usize)> = Vec::new();
    for entry in ingest.files.iter().filter(|f| f.kind == FileKind::Source) {
        let Ok(content) = read_file_lossy(repo_root, &entry.path) else {
            continue;
        };
        estimate.files += 1;
        let cached = cache.get(&entry.path, &hash_content(&content)).is_some();
        if !cached {
            let chars = content.chars().count().min(MAX_FILE_CHARS);
            sizes.push((estimate.files_to_summarize, chars));
            estimate.files_to_summarize += 1;
            estimate.chars_to_send += chars;
        }
        if let Some(parent) = entry.path.parent() {
            for ancestor in parent.ancestors() {
                dirs.insert(ancestor.to_path_buf());
                if !cached {
                    dirty.insert(ancestor.to_path_buf());
                }
            }
        }
    }
    estimate.file_calls = plan_batches(&sizes, batch_chars).len();
    estimate.directories = dirs.len();
    estimate.directories_to_summarize = dirs
        .iter()
        .filter(|d| dirty.contains(*d) || !cache.has_module(d))
        .count();
    estimate
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

const BATCH_SUMMARY_SYSTEM_PROMPT: &str = "You summarize in one or two concise sentences the \
probable role of each of several source code files, based on its path, its git history, and its \
content. Reply with only a JSON object of the form {\"summaries\": [{\"path\": \"<the path as \
given>\", \"summary\": \"<the summary, in English, no Markdown>\"}]}, one entry per file.";

#[derive(Deserialize)]
struct RawBatch {
    #[serde(default)]
    summaries: Vec<RawFileSummary>,
}

#[derive(Deserialize)]
struct RawFileSummary {
    path: String,
    summary: String,
}

/// Summarizes the `batch` files (indices into `loaded`): one request for
/// several small files, one per file otherwise. A file the batched answer
/// misses (or an unparseable answer) falls back to its own request.
async fn summarize_files(
    llm: &dyn LlmProvider,
    ingest: &IngestResult,
    loaded: &[Loaded<'_>],
    batch: &[usize],
) -> Result<Vec<(usize, String)>, PipelineError> {
    let mut answers: BTreeMap<String, String> = BTreeMap::new();
    if batch.len() > 1 {
        let mut prompt = String::new();
        for &i in batch {
            let file = &loaded[i];
            let _ = write!(
                prompt,
                "File: {}\nHistory: {}\n\nContent:\n```\n{}\n```\n\n",
                file.entry.path.display(),
                history_line(ingest.history_for(&file.entry.path)),
                truncate_chars(&file.content, MAX_FILE_CHARS)
            );
        }
        let what = format!("summaries of {} files", batch.len());
        if let Some(raw) =
            complete_json::<RawBatch>(llm, BATCH_SUMMARY_SYSTEM_PROMPT, &prompt, &what).await?
        {
            for item in raw.summaries {
                let summary = item.summary.trim().to_string();
                if !summary.is_empty() {
                    answers.insert(item.path, summary);
                }
            }
        }
    }
    let mut summaries = Vec::new();
    for &i in batch {
        let file = &loaded[i];
        let summary = if let Some(summary) = answers.remove(&file.entry.path.display().to_string())
        {
            summary
        } else {
            let history = ingest.history_for(&file.entry.path);
            summarize_file(llm, file.entry, &file.content, history).await?
        };
        summaries.push((i, summary));
    }
    Ok(summaries)
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

/// The user prompt of a folder summary. Its hash is the cache key: it holds
/// everything the answer depends on.
fn module_prompt(
    dir: &Path,
    own_files: &[&FileSummary],
    child_modules: &[&ModuleSummary],
) -> String {
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

    format!("Folder: {dir_label}\n\nSummarized content:\n{listing}")
}

async fn summarize_module(llm: &dyn LlmProvider, prompt: String) -> Result<String, PipelineError> {
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

/// A folder summary to obtain: from the cache, or from the LLM.
struct ModuleJob {
    dir: PathBuf,
    file_count: u32,
    prompt: String,
    input_hash: String,
    summary: Option<String>,
}

/// Asks the LLM for the folders of one level not served by the cache,
/// `concurrency` at a time, and records the answers in `todo` and the cache.
async fn summarize_level(
    llm: &dyn LlmProvider,
    todo: &mut [ModuleJob],
    progress: &RefCell<Progress>,
    cache: &mut RepoMapCache,
    concurrency: usize,
) -> Result<(), PipelineError> {
    let jobs: Vec<_> = todo
        .iter()
        .enumerate()
        .filter(|(_, job)| job.summary.is_none())
        .map(|(i, job)| {
            let prompt = job.prompt.clone();
            let label = format!("{}/", job.dir.display());
            async move {
                progress.borrow().start(&label);
                (i, summarize_module(llm, prompt).await)
            }
        })
        .collect();
    let mut stream = stream::iter(jobs).buffer_unordered(concurrency);
    let mut done: Vec<(usize, String)> = Vec::new();
    while let Some((i, result)) = stream.next().await {
        done.push((i, result?));
        progress.borrow_mut().finish();
    }
    drop(stream);
    for (i, summary) in done {
        cache.put_module(&todo[i].dir, &todo[i].input_hash, &summary);
        todo[i].summary = Some(summary);
    }
    Ok(())
}

/// Synthesizes a per-folder summary, deepest to shallowest, feeding each
/// parent with the already-computed summaries of its direct children
/// (files + sub-modules).
async fn build_module_summaries(
    llm: &dyn LlmProvider,
    files: &[FileSummary],
    cache: &mut RepoMapCache,
    concurrency: usize,
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
    let progress = RefCell::new(Progress::new("directory summaries", ordered.len()));
    // Folders of the same depth don't depend on each other: each level runs
    // `concurrency` calls at a time, and only needs the deeper levels done.
    for level in ordered.chunk_by(|a, b| a.components().count() == b.components().count()) {
        let mut todo: Vec<ModuleJob> = Vec::new();
        for dir in level {
            let own_files = files_by_dir.get(dir).cloned().unwrap_or_default();
            let child_modules: Vec<&ModuleSummary> = children_by_dir
                .get(dir)
                .into_iter()
                .flatten()
                .filter_map(|child| computed.get(child))
                .collect();

            if own_files.is_empty() && child_modules.is_empty() {
                progress.borrow_mut().skip();
                continue;
            }

            // Saturated at `u32::MAX`: never reached in practice (no repo
            // with billions of files).
            let own_file_count = u32::try_from(own_files.len()).unwrap_or(u32::MAX);
            let file_count =
                own_file_count + child_modules.iter().map(|m| m.file_count).sum::<u32>();
            let prompt = module_prompt(dir, &own_files, &child_modules);
            let input_hash = hash_content(&prompt);
            let cached = cache.get_module(dir, &input_hash).map(str::to_string);
            if cached.is_some() {
                progress.borrow_mut().skip();
            }
            todo.push(ModuleJob {
                dir: dir.clone(),
                file_count,
                prompt,
                input_hash,
                summary: cached,
            });
        }

        summarize_level(llm, &mut todo, &progress, cache, concurrency).await?;

        for job in todo {
            computed.insert(
                job.dir.clone(),
                ModuleSummary {
                    path: job.dir,
                    role_summary: job.summary.unwrap_or_default(),
                    file_count: job.file_count,
                },
            );
        }
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

        let map = build_repo_map(dir.path(), &ingest, &provider, RepoMapOptions::default())
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
    async fn the_estimate_matches_the_calls_of_a_first_run_and_of_a_rerun() {
        let dir = tempfile::tempdir().unwrap();
        let ingest = ingest_with_nested_files(dir.path());
        let provider = CountingProvider {
            calls: AtomicUsize::new(0),
        };

        let first = estimate_repo_map(dir.path(), &ingest, 0);
        assert_eq!((first.files, first.calls()), (2, 5));
        build_repo_map(dir.path(), &ingest, &provider, RepoMapOptions::default())
            .await
            .unwrap();
        assert_eq!(provider.calls.load(Ordering::SeqCst), first.calls());

        assert_eq!(estimate_repo_map(dir.path(), &ingest, 0).calls(), 0);
        std::fs::write(dir.path().join("a/y.rs"), "fn y2() {}").unwrap();
        // y.rs, then "a" and the root; "a/b" is untouched.
        assert_eq!(estimate_repo_map(dir.path(), &ingest, 0).calls(), 3);
    }

    /// Records the highest number of calls in flight at the same time.
    struct PeakProvider {
        in_flight: AtomicUsize,
        peak: AtomicUsize,
    }

    #[async_trait]
    impl LlmProvider for PeakProvider {
        async fn complete(&self, _: CompletionRequest) -> Result<CompletionResponse, LlmError> {
            let now = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
            self.peak.fetch_max(now, Ordering::SeqCst);
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            self.in_flight.fetch_sub(1, Ordering::SeqCst);
            Ok(CompletionResponse {
                content: "summary".to_string(),
                model: "test-model".to_string(),
            })
        }
    }

    #[tokio::test]
    async fn summaries_run_concurrently_up_to_the_limit_and_keep_their_order() {
        let dir = tempfile::tempdir().unwrap();
        let mut ingest = ingest_with_nested_files(dir.path());
        for name in ["c", "d", "e"] {
            std::fs::write(dir.path().join(format!("a/{name}.rs")), name).unwrap();
            ingest.files.push(FileEntry {
                path: PathBuf::from(format!("a/{name}.rs")),
                kind: FileKind::Source,
                size_bytes: 1,
            });
        }
        let provider = PeakProvider {
            in_flight: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
        };

        let map = build_repo_map(
            dir.path(),
            &ingest,
            &provider,
            RepoMapOptions {
                concurrency: 3,
                batch_chars: 0,
            },
        )
        .await
        .unwrap();

        assert_eq!(provider.peak.load(Ordering::SeqCst), 3);
        let paths: Vec<_> = map.files.iter().map(|f| f.path.clone()).collect();
        let expected: Vec<_> = ingest.files.iter().map(|f| f.path.clone()).collect();
        assert_eq!(paths, expected);
        assert!(map.files.iter().all(|f| f.role_summary == "summary"));
        assert_eq!(map.modules.len(), 3);
    }

    #[test]
    fn plan_batches_groups_small_files_and_isolates_big_ones() {
        // 4000 chars per request: a file over 1000 is never batched.
        let files = [(0, 100), (1, 200), (2, 2000), (3, 300), (4, 900), (5, 900)];
        assert_eq!(
            plan_batches(&files, 4000),
            vec![vec![0, 1], vec![2], vec![3, 4, 5]]
        );
        let many: Vec<_> = (0..10).map(|i| (i, 10)).collect();
        assert_eq!(plan_batches(&many, 4000).len(), 2); // 8 + 2
        assert_eq!(plan_batches(&many, 0).len(), 10); // batching off
    }

    /// Answers a batched request with a summary for the first file only
    /// (the rest must fall back to their own request), counts its calls.
    struct BatchProvider {
        calls: AtomicUsize,
    }

    #[async_trait]
    impl LlmProvider for BatchProvider {
        async fn complete(
            &self,
            request: CompletionRequest,
        ) -> Result<CompletionResponse, LlmError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let system = &request.messages[0].content;
            let content = if system.contains("\"summaries\"") {
                r#"{"summaries":[{"path":"a/b/x.rs","summary":"batched x"}]}"#.to_string()
            } else {
                "single".to_string()
            };
            Ok(CompletionResponse {
                content,
                model: "test-model".to_string(),
            })
        }
    }

    #[tokio::test]
    async fn small_files_share_a_request_and_missing_ones_fall_back() {
        let dir = tempfile::tempdir().unwrap();
        let ingest = ingest_with_nested_files(dir.path());
        let provider = BatchProvider {
            calls: AtomicUsize::new(0),
        };
        let options = RepoMapOptions {
            concurrency: 1,
            batch_chars: 4000,
        };
        let estimate = estimate_repo_map(dir.path(), &ingest, options.batch_chars);
        assert_eq!(estimate.file_calls, 1);

        let map = build_repo_map(dir.path(), &ingest, &provider, options)
            .await
            .unwrap();

        let summary_of = |p: &str| {
            map.files
                .iter()
                .find(|f| f.path == Path::new(p))
                .unwrap()
                .role_summary
                .clone()
        };
        assert_eq!(summary_of("a/b/x.rs"), "batched x");
        assert_eq!(summary_of("a/y.rs"), "single");
        // 1 batched request + 1 fallback for y.rs + 3 folders.
        assert_eq!(provider.calls.load(Ordering::SeqCst), 5);
    }

    #[tokio::test]
    async fn unchanged_files_are_not_re_summarized_on_second_run() {
        let dir = tempfile::tempdir().unwrap();
        let ingest = ingest_with_nested_files(dir.path());
        let provider = CountingProvider {
            calls: AtomicUsize::new(0),
        };

        build_repo_map(dir.path(), &ingest, &provider, RepoMapOptions::default())
            .await
            .unwrap();
        // 2 files + 3 modules ("a/b", "a", root "") = 5 calls on the first run.
        let calls_after_first_run = provider.calls.load(Ordering::SeqCst);
        assert_eq!(calls_after_first_run, 5);

        build_repo_map(dir.path(), &ingest, &provider, RepoMapOptions::default())
            .await
            .unwrap();
        let calls_after_second_run = provider.calls.load(Ordering::SeqCst);

        // File and module summaries are both served from the cache
        // (unchanged content, hence unchanged module listings).
        assert_eq!(calls_after_second_run, calls_after_first_run);
    }

    #[tokio::test]
    async fn a_changed_file_only_invalidates_its_folder_and_ancestors() {
        let dir = tempfile::tempdir().unwrap();
        let ingest = ingest_with_nested_files(dir.path());
        std::fs::create_dir_all(dir.path().join("c")).unwrap();
        std::fs::write(dir.path().join("c/z.rs"), "fn z() {}").unwrap();
        let mut ingest = ingest;
        ingest.files.push(FileEntry {
            path: PathBuf::from("c/z.rs"),
            kind: FileKind::Source,
            size_bytes: 9,
        });
        let provider = FailAfterNProvider {
            succeed_calls: usize::MAX,
            calls: AtomicUsize::new(0),
        };
        build_repo_map(dir.path(), &ingest, &provider, RepoMapOptions::default())
            .await
            .unwrap();
        let first = provider.calls.load(Ordering::SeqCst);

        std::fs::write(dir.path().join("a/b/x.rs"), "fn x2() {}").unwrap();
        build_repo_map(dir.path(), &ingest, &provider, RepoMapOptions::default())
            .await
            .unwrap();
        // Each answer is unique, so a new x.rs summary changes its parents'
        // listings: x.rs, then "a/b", "a" and the root; "c" is untouched.
        assert_eq!(provider.calls.load(Ordering::SeqCst) - first, 4);
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

        let result =
            build_repo_map(dir.path(), &ingest, &provider, RepoMapOptions::default()).await;
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
