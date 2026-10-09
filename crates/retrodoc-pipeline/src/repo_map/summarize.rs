use super::*;

pub(super) const FILE_SUMMARY_SYSTEM_PROMPT: &str =
    "You summarize in one or two concise sentences the \
probable role of a source code file, based on its path, its git history, and its content. Reply \
with only the summary, in English, with no preamble and no Markdown formatting.";

pub(super) const MODULE_SUMMARY_SYSTEM_PROMPT: &str =
    "You summarize in one or two concise sentences the \
probable role of a module (folder) of a software project, based on the summaries of its direct \
files and its sub-modules. Reply with only the summary, in English, with no preamble and no \
Markdown formatting.";

pub(super) const BATCH_SUMMARY_SYSTEM_PROMPT: &str =
    "You summarize in one or two concise sentences the \
probable role of each of several source code files, based on its path, its git history, and its \
content. Reply with only a JSON object of the form {\"summaries\": [{\"path\": \"<the path as \
given>\", \"summary\": \"<the summary, in English, no Markdown>\"}]}, one entry per file.";

#[derive(Deserialize, schemars::JsonSchema)]
pub(super) struct RawBatch {
    #[serde(default)]
    pub(super) summaries: Vec<RawFileSummary>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub(super) struct RawFileSummary {
    pub(super) path: String,
    pub(super) summary: String,
}

/// Summarizes the `batch` files (indices into `loaded`): one request for
/// several small files, one per file otherwise. A file the batched answer
/// misses (or an unparseable answer) falls back to its own request.
pub(super) async fn summarize_files(
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

pub(super) async fn summarize_file(
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

    let response = complete_text(llm, FILE_SUMMARY_SYSTEM_PROMPT, &prompt).await?;
    Ok(response.trim().to_string())
}

/// The user prompt of a folder summary. Its hash is the cache key: it holds
/// everything the answer depends on.
pub(super) fn module_prompt(
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

pub(super) async fn summarize_module(
    llm: &dyn LlmProvider,
    prompt: String,
) -> Result<String, PipelineError> {
    let response = complete_text(llm, MODULE_SUMMARY_SYSTEM_PROMPT, &prompt).await?;
    Ok(response.trim().to_string())
}

/// A folder summary to obtain: from the cache, or from the LLM.
pub(super) struct ModuleJob {
    pub(super) dir: PathBuf,
    pub(super) file_count: u32,
    pub(super) prompt: String,
    pub(super) input_hash: String,
    pub(super) summary: Option<String>,
}

/// Asks the LLM for the folders of one level not served by the cache,
/// `concurrency` at a time, and records the answers in `todo` and the cache.
pub(super) async fn summarize_level(
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
pub(super) async fn build_module_summaries(
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
