//! Where the business lives in the repository (`issues/locate_business_files.md`).
//!
//! The roles pass classes files by layer (`model`, `logic`, `entrypoint`…)
//! and its answer for "where are the domain objects and the rules" changes
//! from run to run; many applications have no model layer at all. This pass
//! asks that one question: one LLM call over the file tree, the stack, the
//! product brief and cheap evidence per directory (files, commits, the
//! names of the most changed files) with the titles of the docs and the
//! vocabulary of the tests. It answers a ranked, bounded list of files and
//! directories with a reason each. Paths that are no source file nor a
//! folder holding one are dropped.
//!
//! The list is saved as `.retrodoc/cache/business-files.yaml`, reviewable
//! and hand-editable, with three hashes: the shape of the tree and the
//! brief it was inferred from, and the entries as inferred. An edited file
//! (entries differ from the third hash) is kept as is; an unedited one is
//! inferred again when the tree shape or the brief changed, or with `force`.
//! An LLM that fails or gives nothing usable leaves an empty map, not saved
//! so that a later run asks again: callers must have a way to go on without.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use retrodoc_ingest::signals::test_phrases;
use retrodoc_ingest::{FileEntry, FileKind, IngestResult};
use retrodoc_llm::LlmProvider;
use serde::{Deserialize, Serialize};

use crate::artifact::{load_yaml, save_yaml, Artifact};
use crate::brief::ProductBrief;
use crate::cache::hash_content;
use crate::error::PipelineError;
use crate::response::complete_json;
use crate::roles::{render_tree, MANIFESTS, MAX_MANIFEST_CHARS};

/// Entries kept from the LLM's answer.
const MAX_ENTRIES: usize = 40;
/// Files a map gives out, directories expanded.
pub const MAX_FILES: usize = 60;
/// Directories described in the evidence (the most changed first).
const MAX_EVIDENCE_DIRS: usize = 60;
/// File names listed for each directory of the evidence.
const MAX_NAMES_PER_DIR: usize = 12;
const MAX_DOC_TITLES: usize = 40;
const MAX_TEST_PHRASES: usize = 40;
const MAX_TEST_FILES_READ: usize = 30;
const MAX_PHRASE_CHARS: usize = 80;

const SYSTEM_PROMPT: &str = "You are locating, in a software repository of any stack, where the \
business lives: the files that hold the domain data (what the product manipulates: entities, \
records, value objects, their fields) and the business rules (what is allowed, computed, decided \
or validated). Many applications have no `models` folder: plain classes under `lib/`, services \
holding the rules, a functional core or a hexagonal layout are as good. You are given the file \
tree, the stack, what the product is for, evidence per directory (files, commits, the most \
changed file names), the titles of the docs and the words of the tests. Answer a ranked list, \
most important first, of at most 40 files or directories (a directory ends with `/`), each with \
a short reason. Prefer one directory to many files when all of them hold business. Leave out \
what is technical: migrations (history, not the current schema), HTTP clients, framework \
configuration and settings, templates, assets, generated code, pure plumbing, and the \
presentation layers (views, controllers, serializers, API routes, front-end components) unless a \
file there itself decides or computes something the business cares about. Never answer tests, nor \
the root folder or a whole source tree: name the folders or files that carry the business. Use \
only paths of the tree. \
Reply with ONLY a single JSON object, no prose and no Markdown code fence, matching this shape: \
{\"business\":[{\"path\":\"lib/shop/order.rb\",\"reason\":\"an order and its total\"},\
{\"path\":\"lib/shop/pricing/\",\"reason\":\"discount rules\"}]}.";

/// A file or a directory (ending with `/`) that holds business data or rules.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BusinessEntry {
    pub path: String,
    #[serde(default)]
    pub reason: String,
}

/// The saved artifact: the ranked entries and what tells whether they were
/// edited or are out of date.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BusinessMap {
    /// Shape of the file tree the entries were inferred from.
    #[serde(default)]
    pub tree_hash: String,
    /// Fingerprint of the product brief they were inferred under.
    #[serde(default)]
    pub brief_hash: String,
    /// Hash of the entries as inferred: a different hash on load means the
    /// file was edited by hand.
    #[serde(default)]
    pub content_hash: String,
    pub entries: Vec<BusinessEntry>,
}

impl BusinessMap {
    fn new(tree_hash: String, brief_hash: String, entries: Vec<BusinessEntry>) -> Self {
        Self {
            tree_hash,
            brief_hash,
            content_hash: hash_entries(&entries),
            entries,
        }
    }

    /// Loads `.retrodoc/cache/business-files.yaml`. Missing or unreadable:
    /// `None`.
    #[must_use]
    pub fn load(repo_root: &Path) -> Option<Self> {
        load_yaml(&Artifact::BusinessFiles.path(repo_root))
    }

    /// # Errors
    ///
    /// Returns an error if the cache folder or file can't be written, or
    /// serialization fails.
    pub fn save(&self, repo_root: &Path) -> Result<(), PipelineError> {
        save_yaml(&Artifact::BusinessFiles.path(repo_root), self)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn edited(&self) -> bool {
        hash_entries(&self.entries) != self.content_hash
    }

    /// The source files the entries designate, in rank order: a file as is,
    /// a directory as its source files in path order, without repeats, at
    /// most [`MAX_FILES`]. Entries that match nothing are skipped.
    #[must_use]
    pub fn files(&self, files: &[FileEntry]) -> Vec<PathBuf> {
        let sources: BTreeSet<&Path> = files
            .iter()
            .filter(|f| f.kind == FileKind::Source)
            .map(|f| f.path.as_path())
            .collect();
        let mut out: Vec<PathBuf> = Vec::new();
        let mut seen = BTreeSet::new();
        for entry in &self.entries {
            let path = entry.path.trim().trim_start_matches("./");
            let dir = path.trim_end_matches('/');
            let matched: Vec<&Path> = if !path.ends_with('/') && sources.contains(Path::new(path)) {
                vec![Path::new(path)]
            } else {
                sources
                    .iter()
                    .copied()
                    .filter(|file| !dir.is_empty() && file.starts_with(dir))
                    .collect()
            };
            for file in matched {
                if seen.insert(file) {
                    out.push(file.to_path_buf());
                }
            }
        }
        out.truncate(MAX_FILES);
        out
    }
}

/// Returns the business map of the repo: the saved `business-files.yaml` when
/// it was edited, or is current (same tree shape and brief); otherwise the
/// entries the LLM answers (see the module doc), saved for next time.
/// `force` asks again whatever the file holds, edits included. When the LLM
/// fails, never gives a parseable answer or only paths that don't exist, the
/// out of date saved entries are returned if there are any (else an empty
/// map), and nothing is saved. A saved file that can't be
/// read is left alone.
///
/// # Errors
///
/// Returns an error only if the inferred entries can't be saved.
pub async fn infer_business_files(
    repo_root: &Path,
    ingest: &IngestResult,
    stack: &str,
    brief: &ProductBrief,
    llm: &dyn LlmProvider,
    force: bool,
) -> Result<BusinessMap, PipelineError> {
    let tree_hash = source_shape_hash(ingest);
    let brief_hash = brief.fingerprint();
    let mut stale = None;
    if !force {
        if Artifact::BusinessFiles.path(repo_root).exists()
            && BusinessMap::load(repo_root).is_none()
        {
            // Never overwrite what a person may be in the middle of editing.
            tracing::warn!(
                "{} can't be read; fix it or run `retrodoc business-files --force`",
                Artifact::BusinessFiles.relative_path()
            );
            return Ok(BusinessMap::default());
        }
        if let Some(saved) = BusinessMap::load(repo_root) {
            if saved.edited() {
                tracing::info!("using the hand-edited business-files.yaml");
                return Ok(saved);
            }
            if saved.tree_hash == tree_hash && saved.brief_hash == brief_hash {
                return Ok(saved);
            }
            stale = Some(saved);
        }
    }
    // Out of date entries are still better than none when the LLM fails.
    let fallback = || stale.clone().unwrap_or_default();

    let prompt = build_prompt(repo_root, ingest, stack, brief);
    let answer =
        match complete_json::<Response>(llm, SYSTEM_PROMPT, &prompt, "business files").await {
            Ok(Some(answer)) => answer,
            Ok(None) => return Ok(fallback()),
            Err(error) => {
                tracing::warn!("could not locate the business files ({error})");
                return Ok(fallback());
            }
        };
    let entries = valid_entries(answer.business, &ingest.files);
    if entries.is_empty() {
        tracing::warn!("the LLM named no business file that exists in the repository");
        return Ok(fallback());
    }
    let map = BusinessMap::new(tree_hash, brief_hash, entries);
    map.save(repo_root)?;
    Ok(map)
}

#[derive(Debug, Deserialize)]
struct Response {
    #[serde(default)]
    business: Vec<RawEntry>,
}

#[derive(Debug, Deserialize)]
struct RawEntry {
    #[serde(default)]
    path: String,
    #[serde(default)]
    reason: String,
}

/// The entries that are a source file, or a folder holding one (written with
/// a trailing `/`), once each, in the LLM's order, at most [`MAX_ENTRIES`].
fn valid_entries(raw: Vec<RawEntry>, files: &[FileEntry]) -> Vec<BusinessEntry> {
    let sources: Vec<&Path> = files
        .iter()
        .filter(|f| f.kind == FileKind::Source)
        .map(|f| f.path.as_path())
        .collect();
    let mut seen = BTreeSet::new();
    let mut entries = Vec::new();
    for entry in raw {
        let path = entry.path.trim().trim_start_matches("./");
        let bare = path.trim_end_matches('/');
        let kept = if bare.is_empty() {
            None
        } else if !path.ends_with('/') && sources.contains(&Path::new(bare)) {
            Some(bare.to_string())
        } else if sources.iter().any(|file| file.starts_with(bare)) {
            Some(format!("{bare}/"))
        } else {
            None
        };
        match kept {
            Some(path) if seen.insert(path.clone()) => entries.push(BusinessEntry {
                path,
                reason: entry.reason.trim().to_string(),
            }),
            Some(_) => {}
            None => tracing::warn!("ignoring business path {:?}: no such source", entry.path),
        }
    }
    entries.truncate(MAX_ENTRIES);
    entries
}

fn build_prompt(
    repo_root: &Path,
    ingest: &IngestResult,
    stack: &str,
    brief: &ProductBrief,
) -> String {
    let mut prompt = brief.prompt_head();
    if !stack.trim().is_empty() {
        let _ = writeln!(prompt, "Stack: {}\n", stack.trim());
    }
    prompt.push_str("File tree (one line per directory):\n");
    prompt.push_str(&render_tree(&ingest.files));
    prompt.push_str("\nEvidence per directory (source files, commits, most changed files):\n");
    prompt.push_str(&directory_evidence(ingest));
    for name in MANIFESTS {
        if let Ok(content) = std::fs::read_to_string(repo_root.join(name)) {
            let excerpt: String = content.chars().take(MAX_MANIFEST_CHARS).collect();
            let _ = write!(prompt, "\n--- {name} ---\n{excerpt}\n");
        }
    }
    let titles = doc_titles(ingest);
    if !titles.is_empty() {
        let _ = write!(prompt, "\nTitles of the docs:\n{}", titles.join("\n"));
    }
    let phrases = test_vocabulary(repo_root, ingest);
    if !phrases.is_empty() {
        let _ = write!(prompt, "\n\nWhat the tests say:\n{}", phrases.join("\n"));
    }
    prompt
}

/// One line for each of the most changed directories holding source files.
fn directory_evidence(ingest: &IngestResult) -> String {
    struct Dir<'a> {
        commits: u32,
        files: Vec<(u32, &'a Path)>,
    }
    let mut dirs: BTreeMap<&Path, Dir> = BTreeMap::new();
    for file in ingest.files.iter().filter(|f| f.kind == FileKind::Source) {
        let commits = ingest
            .history_for(&file.path)
            .map_or(0, |history| history.commit_count);
        let dir = dirs
            .entry(file.path.parent().unwrap_or(Path::new("")))
            .or_insert_with(|| Dir {
                commits: 0,
                files: Vec::new(),
            });
        dir.commits += commits;
        dir.files.push((commits, file.path.as_path()));
    }
    let mut ranked: Vec<(&Path, Dir)> = dirs.into_iter().collect();
    ranked.sort_by(|a, b| b.1.commits.cmp(&a.1.commits).then(a.0.cmp(b.0)));
    ranked.truncate(MAX_EVIDENCE_DIRS);
    ranked.sort_by(|a, b| a.0.cmp(b.0));

    let mut out = String::new();
    for (path, mut dir) in ranked {
        dir.files.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(b.1)));
        let names: Vec<String> = dir
            .files
            .iter()
            .take(MAX_NAMES_PER_DIR)
            .map(|(commits, file)| {
                let name = file
                    .file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                format!("{name} ({commits})")
            })
            .collect();
        let label = if path.as_os_str().is_empty() {
            ".".to_string()
        } else {
            format!("{}/", path.display())
        };
        let _ = writeln!(
            out,
            "{label} {} file(s), {} commit(s): {}",
            dir.files.len(),
            dir.commits,
            names.join(", ")
        );
    }
    out
}

fn doc_titles(ingest: &IngestResult) -> Vec<String> {
    let mut titles = Vec::new();
    for doc in &ingest.existing_docs {
        for line in doc.content.lines() {
            if let Some(title) = line.strip_prefix('#') {
                let title = title.trim_start_matches('#').trim();
                if !title.is_empty() {
                    titles.push(format!("{}: {title}", doc.path.display()));
                }
            }
            if titles.len() >= MAX_DOC_TITLES {
                return titles;
            }
        }
    }
    titles
}

fn test_vocabulary(repo_root: &Path, ingest: &IngestResult) -> Vec<String> {
    let mut tests: Vec<&Path> = ingest
        .files
        .iter()
        .filter(|f| f.kind == FileKind::Test)
        .map(|f| f.path.as_path())
        .collect();
    tests.sort_unstable();
    let mut seen = BTreeSet::new();
    let mut phrases = Vec::new();
    for path in tests.into_iter().take(MAX_TEST_FILES_READ) {
        let Ok(content) = std::fs::read_to_string(repo_root.join(path)) else {
            continue;
        };
        for phrase in test_phrases(&content) {
            let phrase: String = phrase.chars().take(MAX_PHRASE_CHARS).collect();
            if phrases.len() < MAX_TEST_PHRASES && seen.insert(phrase.clone()) {
                phrases.push(phrase);
            }
        }
    }
    phrases
}

/// The shape of the source tree: which extensions sit in which folder, not
/// how many files. Only source files count: the docs `generate` writes, the
/// config `init` adds and the tests are no reason to ask again.
fn source_shape_hash(ingest: &IngestResult) -> String {
    let shapes: BTreeSet<String> = ingest
        .files
        .iter()
        .filter(|file| file.kind == FileKind::Source)
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

fn hash_entries(entries: &[BusinessEntry]) -> String {
    hash_content(&serde_yaml::to_string(entries).unwrap_or_default())
}

#[cfg(test)]
mod tests;
