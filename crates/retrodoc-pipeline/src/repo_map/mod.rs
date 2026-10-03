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
use retrodoc_llm::LlmProvider;
use serde::{Deserialize, Serialize};

use crate::artifact::warn_on_error;
use crate::cache::{hash_content, RepoMapCache};
use crate::error::PipelineError;
use crate::progress::Progress;
use crate::response::{complete_json, complete_text};

mod summarize;
#[cfg(test)]
mod tests;

use self::summarize::{build_module_summaries, summarize_files};

/// Files larger than this are truncated before being sent to the LLM, to
/// stay within a reasonable token budget (PLAN.md §6 "cost/volume").
const MAX_FILE_CHARS: usize = 6000;

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

    let loaded = load_sources(repo_root, ingest, &cache, &progress);

    let sizes: Vec<(usize, usize)> = (0..loaded.len())
        .filter(|&i| loaded[i].summary.is_none())
        .map(|i| (i, loaded[i].content.chars().count().min(MAX_FILE_CHARS)))
        .collect();
    let batches = plan_batches(&sizes, options.batch_chars);
    let pass = FilePass {
        llm,
        ingest,
        concurrency,
        progress: &progress,
    };
    let mut results = summarize_pending(&pass, &loaded, &batches, &mut cache, repo_root).await?;

    let files = file_summaries(ingest, &loaded, &mut results);

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

/// Reads every source file first: cached summaries are served right away
/// (and counted as skipped), unreadable files are left out with a warning.
fn load_sources<'a>(
    repo_root: &Path,
    ingest: &'a IngestResult,
    cache: &RepoMapCache,
    progress: &RefCell<Progress>,
) -> Vec<Loaded<'a>> {
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
    loaded
}

/// Summarizes the files without a cached summary, `concurrency` batches at a
/// time, and records each summary in the cache. Returns them by index in
/// `loaded`.
async fn summarize_pending(
    pass: &FilePass<'_>,
    loaded: &[Loaded<'_>],
    batches: &[Vec<usize>],
    cache: &mut RepoMapCache,
    repo_root: &Path,
) -> Result<BTreeMap<usize, String>, PipelineError> {
    let FilePass {
        llm,
        ingest,
        concurrency,
        progress,
    } = *pass;
    let mut results: BTreeMap<usize, String> = BTreeMap::new();
    let jobs = batches.iter().map(|batch| async move {
        let first = loaded[batch[0]].entry.path.display().to_string();
        progress.borrow().start(&if batch.len() > 1 {
            format!("{first} and {} more", batch.len() - 1)
        } else {
            first
        });
        summarize_files(llm, ingest, loaded, batch).await
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
                warn_on_error(cache.save(repo_root));
                return Err(err);
            }
        }
    }
    Ok(results)
}

/// The file entries of the repo map: fresh summary, else the cached one,
/// with the git history counts.
fn file_summaries(
    ingest: &IngestResult,
    loaded: &[Loaded<'_>],
    results: &mut BTreeMap<usize, String>,
) -> Vec<FileSummary> {
    loaded
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
        .collect()
}

/// What the file summaries are asked with.
#[derive(Clone, Copy)]
struct FilePass<'a> {
    llm: &'a dyn LlmProvider,
    ingest: &'a IngestResult,
    concurrency: usize,
    progress: &'a RefCell<Progress>,
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
